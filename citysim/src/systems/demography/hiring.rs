use super::*;

// ---------------------------------------------------------------------------
// Job search
// ---------------------------------------------------------------------------

/// Jobs and room J4: a layoff earns the rehire bonus for this many days.
pub const REHIRE_DAYS: u64 = 30;

/// J4: note a layoff (`economy::dismiss_as`, every employer-side dismissal)
/// for the rehire bonus. Written only with wages on.
pub fn note_laid_off(world: &mut World, id: EntityId, role: Role) {
    if crate::systems::wages::on(world) {
        let now = world.tick;
        world.laid_off.insert(id, (role, now));
    }
}

/// J4: was `id` laid off from `role` in the last [`REHIRE_DAYS`]?
pub fn laid_off_from(world: &World, id: EntityId, role: Role) -> bool {
    world.laid_off.get(&id).is_some_and(|&(r, t)| r == role && t + REHIRE_DAYS * TICKS_PER_DAY > world.tick)
}

fn ownership_row(world: &mut World) -> &mut crate::stats::Jobs2Cols {
    crate::systems::ownership::jobs2_row(world)
}

/// Daily, for each vacancy in BTreeMap order: the nearest unemployed adult
/// (home door to workplace door, ties by id) is hired.
pub(crate) fn job_search(world: &mut World) {
    // J4: layoffs older than the bonus's window are forgotten.
    if !world.laid_off.is_empty() {
        let now = world.tick;
        world.laid_off.retain(|_, &mut (_, t)| t + REHIRE_DAYS * TICKS_PER_DAY > now);
    }
    let vacancies: Vec<(EntityId, Vec<Role>)> = world.vacancies.iter().map(|(&e, r)| (e, r.clone())).collect();
    // Class pay (docs/CLASS_PAY.md): the graded vacancies (professional and
    // management) are filled first, by merit, then every other as before.
    let graded_first = crate::systems::classpay::on(world);
    let passes: &[Option<bool>] = if graded_first { &[Some(true), Some(false)] } else { &[None] };
    for &pass in passes {
        job_search_pass(world, &vacancies, pass);
    }
    // Scale S5: the labour pool. The ungraded places no simulated adult
    // took go to the Ambient household nearest the workplace, up to the
    // day's cap (nothing without the tier).
    if crate::systems::ambient::on(world) {
        ambient_pass(world);
    }
}

/// Scale S5 (docs/SCALE_20K.md § S5): [`job_search`]'s last pass, the open
/// ungraded places (in `vacancies` order) offered to the Ambient labour pool.
fn ambient_pass(world: &mut World) {
    use crate::systems::classpay;
    let open: Vec<(EntityId, Vec<Role>)> = world.vacancies.iter().map(|(&e, r)| (e, r.clone())).collect();
    for (employer, roles) in open {
        for role in roles {
            if !crate::systems::ambient::hires_left(world) {
                return;
            }
            if classpay::on(world) && classpay::base_grade(world, Some(employer), role) >= classpay::PROFESSIONAL {
                continue;
            }
            if !world.vacancies.get(&employer).is_some_and(|v| v.contains(&role)) {
                continue;
            }
            let Some(id) = crate::systems::ambient::hire_from_pool(world, employer, role) else { return };
            close_vacancy(world, id, employer, role);
        }
    }
}

/// A hire's place out of `world.vacancies` (and its works roster entry).
pub(crate) fn close_vacancy(world: &mut World, id: EntityId, employer: EntityId, role: Role) {
    crate::systems::budget::note_hire(world, id, employer, role);
    if let Some(v) = world.vacancies.get_mut(&employer) {
        if let Some(i) = v.iter().position(|&r| r == role) {
            v.remove(i);
        }
        if v.is_empty() {
            world.vacancies.remove(&employer);
        }
    }
}

/// One walk of [`job_search`]'s vacancy snapshot: every vacancy (`None`),
/// or only the graded (`Some(true)`) or ungraded (`Some(false)`) ones.
fn job_search_pass(world: &mut World, vacancies: &[(EntityId, Vec<Role>)], pass: Option<bool>) {
    job_search_pass_with(world, vacancies, pass, !crate::spatial::scans());
}

/// [`job_search_pass`]; `pooled` false reads `pick_candidate` for every
/// ungraded vacancy (the pre-S1 walk) and `classpay::pick_graded`'s scan
/// for every graded one (before the perf pass phase 3's index), for the
/// tests.
pub(super) fn job_search_pass_with(
    world: &mut World,
    vacancies: &[(EntityId, Vec<Role>)],
    pass: Option<bool>,
    pooled: bool,
) {
    // Scale S1: the labour pool, built on the pass's first ungraded pick.
    // The perf pass phase 3: and the graded picks' index, built on the pass's first.
    let mut pools = PassPools::default();
    // Lives at work phase 2 (§ 3.4): the heirs waiting for a household business (one pass).
    let holders = crate::systems::household::business_holders(world);
    // The island pass phase 1 (OPEN-163): a vacancy whose pick is past
    // `[distance] job_cap_ticks` waits until every other vacancy of the pass
    // has picked (so a nearer post takes a candidate first), then takes the
    // nearest left. Empty off: the base walk.
    let mut deferred: Vec<(EntityId, Vec<Role>)> = Vec::new();
    let defer = crate::systems::distance::on(world).then_some(&mut deferred);
    job_search_round(world, vacancies, pass, pooled, &mut pools, &holders, defer);
    if !deferred.is_empty() {
        job_search_round(world, &deferred, pass, pooled, &mut pools, &holders, None);
    }
}

/// One job-search pass's indexes, each built on its first use: the labour
/// pool (Scale S1) and the graded picks' (the perf pass phase 3).
#[derive(Default)]
struct PassPools {
    hire: Option<HirePool>,
    graded: Option<crate::systems::classpay::GradedIndex>,
}

/// One round of [`job_search_pass_with`]: `defer` (the first round, with
/// `[distance]` on) collects the vacancies whose pick is past the job's cap
/// instead of hiring.
fn job_search_round(
    world: &mut World,
    vacancies: &[(EntityId, Vec<Role>)],
    pass: Option<bool>,
    pooled: bool,
    pools: &mut PassPools,
    holders: &std::collections::BTreeSet<EntityId>,
    mut defer: Option<&mut Vec<(EntityId, Vec<Role>)>>,
) {
    let PassPools { hire: pool, graded: graded_ix } = pools;
    use crate::systems::classpay;
    for (employer, roles) in vacancies {
        let employer = *employer;
        let Some(workplace_door) = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door) else {
            world.vacancies.remove(&employer);
            continue;
        };
        for &role in roles {
            let graded = pass.is_some() && classpay::base_grade(world, Some(employer), role) >= classpay::PROFESSIONAL;
            if pass.is_some_and(|p| p != graded) {
                continue;
            }
            // Lives at work phase 2 (§ 3.4): the owner's household takes its
            // business's open post first (None off, or nobody jobless there).
            let household = crate::systems::household::owner_operator(world, employer, role);
            let pick = if let Some(m) = household {
                Some(m)
            } else if graded {
                // P3: a premium the corp cannot fund is not hired.
                if !classpay::premium_funded(world, employer, role) {
                    continue;
                }
                crate::prof::timed!("job::pick_graded", {
                    if pooled {
                        let ix = graded_ix.get_or_insert_with(|| classpay::GradedIndex::new(world));
                        classpay::pick_graded_in(world, employer, role, ix)
                    } else {
                        classpay::pick_graded(world, employer, role)
                    }
                })
            } else {
                if pooled {
                    let pool = pool.get_or_insert_with(|| HirePool::new(world));
                    crate::prof::timed!("job::pick_candidate", pool.pick(world, employer, workplace_door, role))
                } else {
                    pick_candidate(world, employer, workplace_door, role, holders)
                }
            };
            if let Some(n) = pool.as_mut().map(|p| std::mem::take(&mut p.refused)).filter(|&n| n > 0) {
                ownership_row(world).band_refusals += n;
            }
            let Some(id) = pick else { continue };
            // The island pass phase 1 (OPEN-163): past the job's cap, the
            // vacancy waits for the second round (the household's own post
            // excepted).
            if household.is_none()
                && crate::systems::distance::past_job_cap(world, hire_from(world, id), workplace_door)
            {
                if let Some(d) = defer.as_deref_mut() {
                    d.push((employer, vec![role]));
                    continue;
                }
                // Pressure pass round 3, S1 (OPEN-221): the nearest left is
                // still no hire past `[distance.choice] hire_max_ticks` (V11:
                // a 767-tile commute, 11.8 h a day on the road); the vacancy
                // waits for the next pass. Round 4 T5 (OPEN-228's follow-up):
                // a native who takes worse work (`search::takes_far`) takes
                // it up to `[search] far_ticks`, before the Ambient pool.
                let from = hire_from(world, id);
                if past_hire_max(world, from, workplace_door) {
                    if !crate::systems::search::takes_far(world, id, from, workplace_door) {
                        ownership_row(world).choice_hire_far += 1;
                        continue;
                    }
                    ownership_row(world).far_hires += 1;
                }
            }
            // Scale S5 (`[ambient] hire_far`): an Ambient household within
            // the commute cap before a simulated adult beyond it.
            if !graded && ambient_nearer(world, id, workplace_door) {
                if let Some(a) = crate::systems::ambient::hire_from_pool(world, employer, role) {
                    close_vacancy(world, a, employer, role);
                    continue;
                }
            }
            if let Some(pool) = pool.as_mut() {
                pool.note_hired(id);
            }
            let was = classpay::held(world, id);
            let moved = graded && classpay::leave_for_promotion(world, id);
            hire(world, id, employer, role);
            if moved {
                classpay::note_promoted(world, id, Some(employer), was);
            }
            // L2 L10: a public-works vacancy's hire joins the works roster.
            crate::systems::budget::note_hire(world, id, employer, role);
            if let Some(v) = world.vacancies.get_mut(&employer) {
                if let Some(i) = v.iter().position(|&r| r == role) {
                    v.remove(i);
                }
                if v.is_empty() {
                    world.vacancies.remove(&employer);
                }
            }
        }
    }
}

/// The adult `job_search` would hire for `role` at a workplace door: the
/// nearest unemployed, free adult (home door, else tile; ties by id), a
/// guard lawful (≥ 0.4); a Lab the best hacker, a Feed the most
/// knowledgeable (ties lower id). Jobs and room J4 (wages on): the default
/// key is `1024 + distance − round(skill_w × 100 × skill) − rehire_bonus`
/// (`camp_band + 512` for the 1024 on a map past 1,024 Manhattan tiles)
/// (the role's `competence::role_skill`; the bonus for an adult laid off
/// from the same role in the last 30 days).
pub(super) fn pick_candidate(
    world: &World,
    employer: EntityId,
    workplace_door: TilePos,
    role: Role,
    holders: &std::collections::BTreeSet<EntityId>,
) -> Option<EntityId> {
    // L2 shadow fixes item 13: a corp's exec is not in the labour pool
    // (Zetatech's exec was hired as a Militech Fab Tech and laid off).
    let execs = crate::systems::classes::exec_set(world);
    let pimps: Vec<EntityId> = world.jobs2.vice.pimps.values().copied().collect();
    let (skill_w, rehire) = if crate::systems::wages::on(world) {
        (world.config.economy2.skill_w.max(0.0), world.config.economy2.rehire_bonus)
    } else {
        (0.0, 0)
    };
    // A sweeper post's day is a beat assigned after the hire: no band.
    let sweeper_post = role == Role::Sanitation && crate::systems::jobs::sweep_on(world);
    let near_cap =
        (crate::systems::fixes::hire_near(world) && !sweeper_post).then_some(world.config.life.commute_hire_ticks);
    let band = crate::systems::careers::hire_band(world, role);
    world
        .citizens()
        .into_iter()
        .filter(|&id| world.comp::<Brain>(id).is_some_and(|b| !b.emigrating))
        // L1: not the worker who just quit this employer (rehired at midnight, daily).
        .filter(|&id| !crate::systems::life::quit_blocks(world, id, employer))
        .filter(|&id| !holders.contains(&id))
        .filter(|&id| !world.has::<Job>(id) && !world.has::<Sentence>(id))
        .filter(|&id| is_adult(world, id))
        .filter(|id| !execs.contains(id))
        // Round 5 U6 (OPEN-217): a named pimp's round is his work.
        .filter(|id| !pimps.contains(id))
        .filter(|&id| role != Role::Guard || world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.4))
        // Jobs & lives phase 4 (§ 9): a row's `min_dress` (fashion on).
        .filter(|&id| crate::systems::fashion::dress_ok(world, id, role))
        // Shadow V4 bug 6: a knowledge row's floor.
        .filter(|&id| crate::systems::trades::skill_ok(world, id, role))
        // Jobs & lives phase 5: a `chip` post is a contract's, never a hire's.
        .filter(|_| crate::systems::dollhouse::hire_ok(world, role))
        // Jobs & lives phase 6: the struck-off at a black clinic only.
        .filter(|&id| crate::systems::gray::hire_ok(world, id, role))
        // Round 8 X3 (`OPEN-247`): a post that reads a record.
        .filter(|&id| crate::systems::careers::record_ok(world, id, role))
        // Lives at work phase 6 (§ 4.1, § 4.4): the age band, the retired.
        .filter(|&id| crate::systems::careers::admits(world, id, band))
        .map(|id| -> (u64, EntityId) {
            let from = world
                .comp::<Household>(id)
                .and_then(|h| h.home)
                .and_then(|h| world.comp::<Building>(h))
                .map(|b| b.door)
                .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
                .unwrap_or_default();
            // M14 V16 / L2 L2 / M16a (plan C8) / M15 W36: a Lab hires the
            // best hacker, a FightPit the best fighter, a Fixer's office the
            // best talker, a Feed the most knowledgeable (ties lower id;
            // `skill_key`). The island pass phase 1 (OPEN-163): those within
            // `[distance] job_cap_ticks` before any past it (0 off).
            if skill_ranked(role) {
                return (skill_far(world, from, workplace_door) | u64::from(skill_key(world, id, role)), id);
            }
            // The Real economy E41: a `CampRaised` applicant comes first for
            // a Farm or Fab vacancy (the distance key halved past the map).
            let first = crate::systems::camp::applicant_first(world, id, role);
            if first {
                return (u64::from(from.manhattan(workplace_door) / 2), id);
            }
            // J4: skill and the rehire bonus come off the distance, capped
            // at 512 so the key stays above the `CampRaised` band (a halved
            // distance: review finding 8, the key's floor is `camp_band`,
            // past the map's largest halved distance; 512 on the 256 x 192
            // map, whose largest is 223).
            let skill = if skill_w > 0.0 {
                crate::systems::competence::role_skill(world, id, role).map_or(0.0, |(v, _, _)| v.clamp(0.0, 1.0))
            } else {
                0.0
            };
            let off = (skill_w * 100.0 * skill).round() as u32
                + if rehire > 0 && laid_off_from(world, id, role) { rehire } else { 0 }
                + profession_bonus(world, id, role);
            // Transit phase 2 (plan § 6): with the El on, the distance is the
            // commute (`life::commute_cost`, the ride when it wins) in walking
            // tiles, so a Sump resident by a stop ranks as near the Vats.
            let d = if crate::systems::transit::enabled(world) {
                u32::try_from(world.config.exec.walk_tiles(crate::systems::life::commute_cost(
                    world,
                    from,
                    workplace_door,
                )))
                .unwrap_or(u32::MAX)
            } else {
                from.manhattan(workplace_door)
            };
            // Hunger fix item 9 (P5g): a candidate within `commute_hire_ticks`
            // (`life::commute_cost`) before any beyond it (skill and the
            // rehire bonus rank inside each band).
            let band = if near_cap.is_some_and(|c| crate::systems::life::commute_cost(world, from, workplace_door) > c)
            {
                1024
            } else {
                0
            } + past_cap_band(world, from, workplace_door);
            (u64::from((camp_band(world) + 512 + band + d).saturating_sub(off.min(512))), id)
        })
        .min()
        .map(|(_, id)| id)
}

/// Scale S5 (`[ambient] hire_far`): `id`'s commute to `workplace_door` is
/// past `[life] commute_hire_ticks` and the nearest Ambient household's is
/// within it (the labour pool's cap not spent).
fn ambient_nearer(world: &World, id: EntityId, workplace_door: TilePos) -> bool {
    if !world.config.ambient.hire_far
        || !crate::systems::ambient::on(world)
        || !crate::systems::ambient::hires_left(world)
    {
        return false;
    }
    let cap = world.config.life.commute_hire_ticks;
    if crate::systems::life::commute_cost(world, hire_from(world, id), workplace_door) <= cap {
        return false;
    }
    crate::systems::ambient::nearest_home(world, workplace_door)
        .and_then(|hi| world.ambient.homes.get(hi))
        .and_then(|h| world.comp::<Building>(h.building))
        .is_some_and(|b| crate::systems::life::commute_cost(world, b.door, workplace_door) <= cap)
}

/// [`pick_candidate`]'s filters that hold for a whole `job_search` pass
/// (a hire takes only its own hire out of the pool): a free, unemployed,
/// staying adult who is not an exec.
pub(super) fn pool_member(world: &World, id: EntityId, execs: &std::collections::BTreeSet<EntityId>) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| !b.emigrating)
        && !world.has::<Job>(id)
        && !world.has::<Sentence>(id)
        && is_adult(world, id)
        && !execs.contains(&id)
        // Round 7 W6 (OPEN-217): a named pimp's round is his work, in the
        // pooled hire too (U6 kept him out of `pick_candidate` only: 20k
        // seed 42, a named pimp quit "to run the street" at 00:07 and was
        // hired back by the pool at 00:11, five nights running, then lost
        // the street "holding a day post" or "away from every pitch").
        && crate::systems::vice::pimp_district(world, id).is_none()
        // M18 P5: the player's body is hired only when it asks (P2's ask at
        // the door); the pool never writes it a job.
        && !crate::systems::player::possessed(world, id)
}

/// [`pick_candidate`]'s skill-ranked roles (the key has no distance).
fn skill_ranked(role: Role) -> bool {
    matches!(role, Role::Researcher | Role::Fighter | Role::Fixer | Role::Reporter)
}

/// [`pick_candidate`]'s key for a skill-ranked role.
fn skill_key(world: &World, id: EntityId, role: Role) -> u32 {
    let skill = |f: fn(&crate::components::Skills) -> f32| world.comp::<crate::components::Skills>(id).map_or(0.0, f);
    match role {
        Role::Researcher => u32::MAX - (skill(|s| s.hacking).clamp(0.0, 1.0) * 1_000_000.0) as u32,
        Role::Fighter => {
            u32::MAX
                - (world
                    .comp::<crate::components::Skills>(id)
                    .map_or(0.0, |s| crate::systems::combat::skill_power(world, s))
                    .clamp(0.0, 1.0)
                    * 1_000_000.0) as u32
        }
        Role::Fixer => u32::MAX - (skill(|s| s.persuasion + s.knowledge).clamp(0.0, 2.0) * 500_000.0) as u32,
        _ => u32::MAX - (skill(|s| s.knowledge).clamp(0.0, 1.0) * 1_000_000.0) as u32,
    }
}

/// [`pick_candidate`]'s home door (else tile) of a candidate.
/// S1 (OPEN-221): a candidate setting out from `from` is past
/// `[distance.choice] hire_max_ticks` of the workplace door (false off). The
/// structural trip (`distance::travel`, the El where it wins), not his own:
/// the broke candidate who walks today rides on his first wage.
fn past_hire_max(world: &World, from: TilePos, door: TilePos) -> bool {
    crate::systems::distance::choice(world)
        .is_some_and(|c| crate::systems::distance::travel(world, from, door) > c.hire_max_ticks)
}

pub(super) fn hire_from(world: &World, id: EntityId) -> TilePos {
    world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
        .unwrap_or_default()
}

/// [`pick_candidate`]'s skill and rehire offset for `id` in `role` (capped
/// at 512, as the key reads it).
fn hire_offset(world: &World, id: EntityId, role: Role, skill_w: f32, rehire: u32) -> u32 {
    let skill = if skill_w > 0.0 {
        crate::systems::competence::role_skill(world, id, role).map_or(0.0, |(v, _, _)| v.clamp(0.0, 1.0))
    } else {
        0.0
    };
    let off = (skill_w * 100.0 * skill).round() as u32
        + if rehire > 0 && laid_off_from(world, id, role) { rehire } else { 0 }
        + profession_bonus(world, id, role);
    off.min(512)
}

/// Lives at work phase 6 (§ 4.3): a holder of the vacancy's profession
/// ranks `[careers] profession_w` tiles nearer (0 off).
fn profession_bonus(world: &World, id: EntityId, role: Role) -> u32 {
    if crate::systems::careers::holds(world, id, role) {
        world.config.careers.profession_w
    } else {
        0
    }
}

/// The Real economy E41's band (review finding 8): every regular hiring key
/// is at least this (`camp_band + 512 + d - offset`, the offset capped at
/// 512), and a `CampRaised` applicant's key (its halved Manhattan distance)
/// is below it on any map: the largest halved distance plus one, at least
/// 512 (the shipped 256 x 192 map's 1,024 base, unchanged).
fn camp_band(world: &World) -> u32 {
    let (w, h) = (world.map.w() as u32, world.map.h() as u32);
    ((w.saturating_sub(1) + h.saturating_sub(1)) / 2 + 1).max(512)
}

/// [`pick_candidate`]'s distance part of the key from `from` to a
/// workplace door, `camp_band + 512 + band + d` (`near_cap`: the hire-near
/// band's cap).
///
/// The perf pass phase 5: the commute (`life::commute_cost`, which
/// `distance::travel` is too) read once for its three uses, the El's
/// distance, the hire-near band and the job cap's band (it was read up to
/// three times per `from` and door; a pure function of both and the world,
/// so the same key). `pool::bases` was most of the midnight tick at 60k.
fn hire_base(world: &World, from: TilePos, workplace_door: TilePos, near_cap: Option<u64>) -> u32 {
    let transit = crate::systems::transit::enabled(world);
    let job_cap = crate::systems::distance::job_cap(world);
    let cost = (transit || near_cap.is_some() || job_cap.is_some())
        .then(|| crate::systems::life::commute_cost(world, from, workplace_door));
    let past = |cap: Option<u64>| cap.zip(cost).is_some_and(|(c, cost)| cost > c);
    let d = match cost.filter(|_| transit) {
        Some(cost) => u32::try_from(world.config.exec.walk_tiles(cost)).unwrap_or(u32::MAX),
        None => from.manhattan(workplace_door),
    };
    let band = if past(near_cap) { 1024 } else { 0 } + if past(job_cap) { PAST_CAP_BAND } else { 0 };
    camp_band(world) + 512 + band + d
}

/// The island pass phase 1 (OPEN-163): a candidate past `[distance]
/// job_cap_ticks` (`distance::travel`) adds [`PAST_CAP_BAND`] to the
/// hiring key, so every candidate within the cap ranks before any past it
/// (the offsets come off at most 512, the hire-near band adds 1,024, and a
/// within-cap distance is far under 512 tiles); with none within, the key's
/// order decides, the nearest first (0 off).
fn past_cap_band(world: &World, from: TilePos, workplace_door: TilePos) -> u32 {
    if crate::systems::distance::past_job_cap(world, from, workplace_door) {
        PAST_CAP_BAND
    } else {
        0
    }
}

/// [`past_cap_band`]'s band.
const PAST_CAP_BAND: u32 = 2048;

/// [`past_cap_band`] for a skill-ranked post: the key's high word.
fn skill_far(world: &World, from: TilePos, workplace_door: TilePos) -> u64 {
    u64::from(crate::systems::distance::past_job_cap(world, from, workplace_door)) << 32
}

/// The groups of one role, by index into the pool's `froms` (home door,
/// else tile).
type HireGroups = Vec<HireGroup>;

/// One home door's candidates for one role: those [`HirePool::pick`]'s
/// `free` admits on every rule a pass cannot change, `(512 - offset, id)`
/// ascending (the group's best first). The perf pass phase 3: the rules a
/// hire cannot change are read once, when the role's groups are built (the
/// heirs, a guard's lawfulness, the dress, the knowledge floor, a chip post,
/// the struck-off, the age gate: each the candidate's own state or the
/// config's, and a pass hires only the candidate it takes); the walk reads
/// only the hired and the quit-blocked. The age refusals stay counted
/// where the walk meets them (`band_refusals`).
#[derive(Clone, Default)]
struct HireGroup {
    /// The entries before it are all hired.
    head: usize,
    entries: Vec<HireEntry>,
}

/// A candidate in a [`HireGroup`].
#[derive(Clone, Copy)]
struct HireEntry {
    rank: u32,
    id: EntityId,
    /// Its index in the pool's `members`.
    m: u32,
    /// `careers::refusal`: `None` admitted, `Some(true)` age, `Some(false)` retired.
    gate: Option<bool>,
}

/// Every candidate's home door by the key's distance part from one
/// workplace door, `(hire_base, index into froms)` ascending.
type HireBases = Vec<(u32, u32)>;

/// Scale S1: one `job_search` pass's labour pool. [`pick_candidate`] walked
/// every citizen for every vacancy (a 20,000 city's opening staffing read
/// ~14,000 adults for each of ~2,500 vacancies). The pool keeps the pass's
/// candidates (`pool_member`: the filters a hire cannot change) and, per
/// role, groups them by home door with each group ranked by its offset, so
/// a vacancy reads the groups in the order of the key's distance part from
/// its door, each group's best, and stops once no farther group can beat
/// the best found: `pick_candidate`'s pick (`test_hire_pool_matches_pick_candidate`).
pub(super) struct HirePool {
    /// The candidates, ascending.
    members: Vec<EntityId>,
    /// The `CampRaised` candidates (E41: first for a Farm or Fab vacancy).
    camp: Vec<EntityId>,
    /// Every candidate's home door (else tile), each once, ascending.
    froms: Vec<TilePos>,
    /// Each member's index in `froms`.
    member_from: Vec<u32>,
    /// Each member's quit block live at the pass's tick (`Brain::quit_from`):
    /// the employer it may not rejoin (`EntityId::NONE`: every one).
    member_quit: Vec<Option<EntityId>>,
    /// Hired this pass.
    hired: std::collections::BTreeSet<EntityId>,
    /// `hired`, by index in `members`.
    member_hired: Vec<bool>,
    /// Lives at work phase 2 (§ 3.4): the heirs waiting for a household business.
    holders: std::collections::BTreeSet<EntityId>,
    /// Lives at work phase 6 (§ 4.1): candidates passed over for age this
    /// pass (each time a pick reads one: `band_refusals`).
    refused: u32,
    groups: std::collections::BTreeMap<Role, HireGroups>,
    /// `(workplace door, near cap)` -> `froms` by `hire_base`, ascending.
    bases: std::collections::BTreeMap<(TilePos, Option<u64>), HireBases>,
}

impl HirePool {
    pub(super) fn new(world: &World) -> HirePool {
        let execs = crate::systems::classes::exec_set(world);
        let members: Vec<EntityId> = world
            .brained()
            .into_iter()
            .filter(|&id| world.has::<Identity>(id) && !world.has::<Corpse>(id) && pool_member(world, id, &execs))
            // Pressure pass F3: the idle days and the reputation's pass-over (`life::quit_blocks`).
            .filter(|&id| !crate::systems::misery::pool_out(world, id))
            .collect();
        let camp = members.iter().copied().filter(|&id| world.has::<crate::econ::CampRaised>(id)).collect();
        let from_of: Vec<TilePos> = members.iter().map(|&id| hire_from(world, id)).collect();
        let mut froms = from_of.clone();
        froms.sort_unstable();
        froms.dedup();
        let member_from = from_of.iter().map(|f| froms.binary_search(f).unwrap_or_default() as u32).collect();
        let tick = world.tick;
        let member_quit = members
            .iter()
            .map(|&id| {
                world.comp::<Brain>(id).and_then(|b| b.quit_from).filter(|&(_, until)| tick < until).map(|(e, _)| e)
            })
            .collect();
        let n = members.len();
        HirePool {
            members,
            camp,
            froms,
            member_from,
            member_quit,
            hired: std::collections::BTreeSet::new(),
            member_hired: vec![false; n],
            holders: crate::systems::household::business_holders(world),
            refused: 0,
            groups: std::collections::BTreeMap::new(),
            bases: std::collections::BTreeMap::new(),
        }
    }

    /// `id` hired this pass (out of every later pick).
    fn note_hired(&mut self, id: EntityId) {
        self.hired.insert(id);
        if let Ok(m) = self.members.binary_search(&id) {
            self.member_hired[m] = true;
        }
    }

    /// `pick_candidate(world, employer, workplace_door, role)`.
    pub(super) fn pick(
        &mut self,
        world: &World,
        employer: EntityId,
        workplace_door: TilePos,
        role: Role,
    ) -> Option<EntityId> {
        let free = |hired: &std::collections::BTreeSet<EntityId>, id: EntityId| {
            !hired.contains(&id)
                && !crate::systems::life::quit_blocks(world, id, employer)
                // Lives at work phase 2 (§ 3.4): an heir waits for the household's business.
                && !self.holders.contains(&id)
                && (role != Role::Guard || world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.4))
                && crate::systems::fashion::dress_ok(world, id, role)
                && crate::systems::trades::skill_ok(world, id, role)
                && crate::systems::dollhouse::hire_ok(world, role)
                && crate::systems::gray::hire_ok(world, id, role)
                && crate::systems::careers::record_ok(world, id, role)
        };
        // Lives at work phase 6 (§ 4.1, § 4.4): the age band and the retired
        // (every candidate admitted off); an age refusal is counted.
        let band = crate::systems::careers::hire_band(world, role);
        let mut refused = 0u32;
        let mut gate = |id: EntityId| match crate::systems::careers::refusal(world, id, band) {
            None => true,
            Some(age) => {
                refused += u32::from(age);
                false
            }
        };
        if skill_ranked(role) {
            let pick = self
                .members
                .iter()
                .copied()
                .filter(|&id| free(&self.hired, id) && gate(id))
                .map(|id| {
                    (skill_far(world, hire_from(world, id), workplace_door) | u64::from(skill_key(world, id, role)), id)
                })
                .min()
                .map(|(_, id)| id);
            self.refused += refused;
            return pick;
        }
        let (skill_w, rehire) = if crate::systems::wages::on(world) {
            (world.config.economy2.skill_w.max(0.0), world.config.economy2.rehire_bonus)
        } else {
            (0.0, 0)
        };
        let sweeper_post = role == Role::Sanitation && crate::systems::jobs::sweep_on(world);
        let near_cap =
            (crate::systems::fixes::hire_near(world) && !sweeper_post).then_some(world.config.life.commute_hire_ticks);
        let mut best: Option<(u32, EntityId)> = None;
        // The Real economy E41: the `CampRaised` applicants' halved distance.
        if matches!(role, Role::Farmer | Role::Fabber) {
            for &id in &self.camp {
                if crate::systems::camp::applicant_first(world, id, role) && free(&self.hired, id) && gate(id) {
                    let k = (hire_from(world, id).manhattan(workplace_door) / 2, id);
                    if best.is_none_or(|b| k < b) {
                        best = Some(k);
                    }
                }
            }
        }
        let (members, member_from, holders) = (&self.members, &self.member_from, &self.holders);
        let nfroms = self.froms.len();
        let groups = self.groups.entry(role).or_insert_with(|| {
            let _t = crate::prof::scope!("pool::groups");
            let mut g: HireGroups = vec![HireGroup::default(); nfroms];
            for (m, &id) in members.iter().enumerate() {
                if crate::systems::camp::applicant_first(world, id, role) {
                    continue;
                }
                // `free`'s rules a pass cannot change.
                let fixed = !holders.contains(&id)
                    && (role != Role::Guard || world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.4))
                    && crate::systems::fashion::dress_ok(world, id, role)
                    && crate::systems::trades::skill_ok(world, id, role)
                    && crate::systems::dollhouse::hire_ok(world, role)
                    && crate::systems::gray::hire_ok(world, id, role)
                    && crate::systems::careers::record_ok(world, id, role);
                if !fixed {
                    continue;
                }
                let off = hire_offset(world, id, role, skill_w, rehire);
                let gate = crate::systems::careers::refusal(world, id, band);
                g[member_from[m] as usize].entries.push(HireEntry { rank: 512 - off, id, m: m as u32, gate });
            }
            for grp in &mut g {
                grp.entries.sort_unstable_by_key(|e| (e.rank, e.id));
            }
            g
        });
        let froms = &self.froms;
        let bases = self.bases.entry((workplace_door, near_cap)).or_insert_with(|| {
            let _t = crate::prof::scope!("pool::bases");
            let mut b: Vec<(u32, u32)> = froms
                .iter()
                .enumerate()
                .map(|(i, &from)| (hire_base(world, from, workplace_door, near_cap), i as u32))
                .collect();
            b.sort_unstable();
            b
        });
        let _t = crate::prof::scope!("pool::walk");
        let (hired, quit) = (&self.member_hired, &self.member_quit);
        for &(base, fi) in bases.iter() {
            // Every key in this group and the ones after it is at least `base - 512`.
            if best.is_some_and(|(k, _)| base - 512 > k) {
                break;
            }
            let grp = &mut groups[fi as usize];
            // The hired leave a group for good; the quit-blocked only for this employer.
            while grp.entries.get(grp.head).is_some_and(|e| hired[e.m as usize]) {
                grp.head += 1;
            }
            for e in &grp.entries[grp.head..] {
                if hired[e.m as usize]
                    || quit[e.m as usize].is_some_and(|q| q == employer || q == EntityId::NONE)
                    || crate::systems::misery::fired_by(world, e.id, employer)
                {
                    continue;
                }
                match e.gate {
                    None => {
                        let k = (base - 512 + e.rank, e.id);
                        if best.is_none_or(|b| k < b) {
                            best = Some(k);
                        }
                        break;
                    }
                    Some(age) => refused += u32::from(age),
                }
            }
        }
        self.refused += refused;
        best.map(|(_, id)| id)
    }
}

/// M15 W29: the adult the vacancy at `employer` would hire for `role` now
/// (poaching compares a rival's worker with this one).
pub fn hire_candidate(world: &World, employer: EntityId, role: Role) -> Option<EntityId> {
    let door = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door)?;
    // Lives at work phase 2 (§ 3.4): the heirs waiting for a household business (one pass a call).
    let holders = crate::systems::household::business_holders(world);
    pick_candidate(world, employer, door, role, &holders)
}

pub fn hire(world: &mut World, id: EntityId, employer: EntityId, role: Role) {
    hire_with(world, id, employer, role, true);
}

/// Round 4 T5 (OPEN-217, `search::on_ask`): `id` may take the open `role`
/// at `employer` at its door: [`pick_candidate`]'s rules for one candidate
/// (a free unemployed adult who is no exec, not quit-blocked there, the
/// guard's lawfulness, the dress, the knowledge floor, a chip post, the
/// struck-off, the age band). A graded post (merit, `classpay`) and a
/// skill-ranked one (the best of the city) are the midnight's, not a walk-in's.
pub fn can_take(world: &World, id: EntityId, employer: EntityId, role: Role) -> bool {
    use crate::systems::classpay;
    if skill_ranked(role)
        || (classpay::on(world) && classpay::base_grade(world, Some(employer), role) >= classpay::PROFESSIONAL)
    {
        return false;
    }
    world.comp::<Brain>(id).is_some_and(|b| !b.emigrating)
        && !world.has::<Job>(id)
        && !world.has::<Sentence>(id)
        && is_adult(world, id)
        && !crate::systems::ages::is_protected(world, id)
        && !crate::systems::founding::is_exec(world, id)
        && crate::systems::vice::pimp_district(world, id).is_none()
        && !crate::systems::life::quit_blocks(world, id, employer)
        && (role != Role::Guard || world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.4))
        && crate::systems::fashion::dress_ok(world, id, role)
        && crate::systems::trades::skill_ok(world, id, role)
        && crate::systems::dollhouse::hire_ok(world, role)
        && crate::systems::gray::hire_ok(world, id, role)
        // Round 8 X3 (`OPEN-247`): a post that reads a record.
        && crate::systems::careers::record_ok(world, id, role)
        && crate::systems::careers::admits(world, id, crate::systems::careers::hire_band(world, role))
}

/// [`hire`], the foreman an actor of the Hire event or not (round 4 T4: a
/// Yard's shape-up is the foreman's one line a morning, `streetwork::dawn_pick`;
/// the hand's Hire still names him).
pub fn hire_with(world: &mut World, id: EntityId, employer: EntityId, role: Role, foreman_actor: bool) {
    // The pressure pass S6 (Dylan 2026-10-27): no job under 18 (every pick
    // takes an adult; the backstop).
    if crate::systems::ages::is_protected(world, id) {
        return;
    }
    // Round 4 T6: nor a corp's exec (the labour pool leaves him out; the
    // backstop for the picks beside it, `exec_78363 d26`'s sick cover).
    if crate::systems::founding::is_exec(world, id) {
        return;
    }
    let wc = &world.config.world;
    let mut shifts =
        if role == Role::Guard && id.index.is_multiple_of(2) { wc.shift_night.clone() } else { wc.shift_day.clone() };
    // L2 (plan key `[leisure] evening_shift`): the night venues' staff work
    // the evening, so a bout at `bout_hour` has its Fighters on shift.
    if matches!(role, Role::Host | Role::Fighter | Role::Croupier | Role::Concierge) {
        shifts = world.config.leisure.evening_shift.clone();
    }
    // OPEN-111: a venue's legacy staff work its trade hours.
    if let Some(s) = crate::systems::service::venue_shift(world, Some(employer), role) {
        shifts = s;
    }
    // Jobs and room J12: a trade's shift is its row's.
    if let Some(row) = crate::systems::trades::row_of(world, role) {
        shifts = crate::systems::trades::shifts_for(world, row, id);
    }
    // Workplace room: a Farm's or Fab's hire past its day shift's room
    // works the night shift.
    if crate::systems::wages::relief_hire(world, employer, role) {
        shifts = world.config.world.shift_night.clone();
    }
    // Class pay: the post's grade fixes its wage (the config wage at grade 0).
    let grade = crate::systems::classpay::base_grade(world, Some(employer), role);
    let wage_per_day = crate::systems::classpay::wage_at(world, role, grade);
    world.insert(
        id,
        Job {
            employer: Some(employer),
            role,
            wage_per_day,
            shifts,
            days_unpaid: 0,
            tax_accum: 0.0,
            last_shift_day: None,
            last_wage_attempt_day: None,
            duty_ticks: 0,
            hired_tick: world.tick,
            struck_shift: None,
            premium: 1.0,
            // L2 fix round: unpaid until the first wage (L2 on only).
            paid_once: false,
            duty_fixed: None,
            grade,
            until: None,
            seniority_days: 0,
            foreman: false,
            shift_tips: 0,
        },
    );
    world.abort_plan(id);
    // J4: a rehire spends the bonus.
    world.laid_off.remove(&id);
    let name = world.name_of(id);
    // Jobs & lives phase 1 (§ 2): the foreman named (nothing off).
    let foreman = crate::systems::industry::foreman_note(world, employer, id, "picked");
    // Lives at work phase 6 (§ 4.2, § 4.3): the foreman is an actor (his
    // diary holds the hire); a holder of the trade counted.
    let mut actors = vec![id, employer];
    if let Some(f) = crate::systems::careers::foreman_actor(world, employer, id) {
        if foreman_actor {
            actors.push(f);
        }
        ownership_row(world).foreman_hires += 1;
    }
    if crate::systems::careers::holds(world, id, role) {
        ownership_row(world).profession_rehires += 1;
    }
    world.push_event(EventKind::Hire, &actors, format!("{name} hired as {}{foreman}", role.label()));
    // Lives at work phase 2 (§ 3.6): the work deed (nothing off).
    if crate::systems::household::on(world) {
        let d = crate::systems::gossip::talk_district(world, id);
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::Employed, Some(id), Some(employer));
    }
}
