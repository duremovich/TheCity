use super::*;

/// Who a guard can chase: `Some((tile, guard))` keeps the located suspects
/// last seen within `[law] pursuit_radius` tiles of `tile` (M10), and never
/// the guard themselves (one who chased their own warrant cuffed themselves,
/// with no escort to end it, and starved); `None` is the whole city.
#[derive(Clone, Copy, Debug)]
pub struct Pursuer {
    pub tile: TilePos,
    pub guard: EntityId,
}

/// M11 D18: a private guard chases only nearby warrants.
pub fn pursuit_radius_for(world: &World, guard: EntityId) -> u32 {
    if is_private_guard(world, guard) {
        world.config.corps.private_pursuit_radius
    } else {
        world.config.law.pursuit_radius
    }
}

/// M11 D18: a guard job at a Security Office (the one test behind
/// `is_private_guard`, `is_city_guard` and `routine::workplace_key_for`).
pub fn job_is_private_guard(world: &World, job: &Job) -> bool {
    job.role == Role::Guard
        && job.employer.and_then(|e| world.comp::<Building>(e)).is_some_and(|b| b.kind == BuildingKind::SecurityOffice)
}

/// M11 D18: a guard employed by a Security Office.
pub fn is_private_guard(world: &World, guard: EntityId) -> bool {
    world.comp::<Job>(guard).is_some_and(|j| job_is_private_guard(world, j))
}

/// A guard on the city payroll: a guard who is not a Security Office's
/// (employed at the Precinct, or by nobody). The complement of
/// `is_private_guard` among guards.
pub fn is_city_guard(world: &World, guard: EntityId) -> bool {
    world.comp::<Job>(guard).is_some_and(|j| j.role == Role::Guard && !job_is_private_guard(world, j))
}

/// Shadow V4 bug 11: a private guard polices his corp's beat: a suspect
/// last seen within `[corps] private_beat_tiles` of his office or one of its
/// clients (the corp guards chased warrants city-wide). A city guard's beat
/// is the city. Applied in `chaseable`, so every pursuer path (the Arrest
/// goal, its plan's bind, `any_located_suspect`) reads it.
pub fn on_beat(world: &World, guard: EntityId, s: EntityId) -> bool {
    if !is_private_guard(world, guard) {
        return true;
    }
    let Some(&(seen, _)) = world.last_seen().get(&s) else { return false };
    on_beat_at(world, guard, seen)
}

/// `on_beat` for a private guard and a sighting tile.
fn on_beat_at(world: &World, guard: EntityId, seen: TilePos) -> bool {
    let max = world.config.corps.private_beat_tiles;
    let Some(office) = world.comp::<Job>(guard).and_then(|j| j.employer) else { return false };
    let near = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| bd.door.manhattan(seen) <= max);
    if near(office) {
        return true;
    }
    let Some(corp) = world.corp_of_building(office) else { return false };
    world.comp::<crate::components::Corp>(corp).is_some_and(|c| c.contracts.iter().any(|&(b, _)| near(b)))
}

/// `claimed`: the suspects other guards already chase (L1), built once per
/// scoring call by `claim_set`, on its first read, with the pursuer's
/// `pursuit_radius_for`. `seen`: `s`'s `World::last_seen` entry.
fn chaseable(world: &World, s: EntityId, seen: (TilePos, Tick), by: Option<Pursuer>, claimed: &Claims<'_>) -> bool {
    let near = |p: Pursuer| {
        s != p.guard
            && seen.0.manhattan(p.tile) <= claimed.radius
            // L1: a stale sighting is chased only from near by, and a
            // suspect another guard is after is theirs.
            && crate::systems::life::sighting_fresh_at(world, seen, p.tile)
            && !claimed.contains(s)
            // Round 7 W3: a warrant handed to another guard (his collar) is his.
            && !handed_to_other(world, p.guard, s)
            // M16a (plan C31): never the buyer of a guard on the take.
            && !crate::systems::contracts::on_take_of(world, p.guard, s)
            // Shadow V4 bug 11: a corp guard's warrants on its beat only.
            && (!claimed.private || on_beat_at(world, p.guard, seen.0))
    };
    // The cheap reach test first (the same conjunction, reordered).
    by.is_none_or(near) && located(world, s)
}

/// Round 7 W3 (`OPEN-244`): `s` is a guard other than `guard`'s collar by
/// hand-on (`handoffs`, inside `[justice] collar_hours`): a warrant taken to
/// the door was served by whichever guard stood nearer.
fn handed_to_other(world: &World, guard: EntityId, s: EntityId) -> bool {
    if world.handoffs.is_empty() {
        return false;
    }
    let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
    let since = world.tick.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    world.handoffs.iter().any(|(&g, &(x, t))| g != guard && x == s && t >= since && living(world, g))
}

/// The L1 claim set for a pursuer (empty for the whole city). The perf
/// pass phase 5: read from the world's claim index
/// (`World::claimed_by_other`), kept current by marks, not built per
/// scoring call by walking every guard (`life::claimed_suspects`, which a
/// unit test's `spatial::scans()` still reads).
struct Claims<'a> {
    world: &'a World,
    guard: Option<EntityId>,
    /// The pursuer's `pursuit_radius_for` (0 for the whole city: unread).
    radius: u32,
    /// The pursuer is a private (corp) guard: `on_beat` applies.
    private: bool,
    /// The scan's set, built on the first read (`spatial::scans()` only).
    set: std::cell::OnceCell<BTreeSet<EntityId>>,
}

impl Claims<'_> {
    fn contains(&self, s: EntityId) -> bool {
        match self.guard {
            Some(g) if crate::spatial::scans() => {
                self.set.get_or_init(|| crate::systems::life::claimed_suspects(self.world, g)).contains(&s)
            }
            Some(g) => self.world.claimed_by_other(g, s),
            None => false,
        }
    }
}

fn claim_set(world: &World, by: Option<Pursuer>) -> Claims<'_> {
    let radius = by.map_or(0, |p| pursuit_radius_for(world, p.guard));
    let private = by.is_some_and(|p| is_private_guard(world, p.guard));
    Claims { world, guard: by.map(|p| p.guard), radius, private, set: std::cell::OnceCell::new() }
}

/// Scale S1: the open-warrant suspects `located` can pass for `by`,
/// ascending, with their `last_seen` entries: those last seen inside
/// `[crime] suspect_seen_ticks` (`World::located_candidates`, memoised per
/// tick) and, for a pursuer, inside its reach's square (the memo's grid),
/// not every open warrant in the city (a guard's every think walked them all).
fn located_candidates(world: &World, by: Option<Pursuer>, claimed: &Claims<'_>) -> Vec<(EntityId, (TilePos, Tick))> {
    let located = world.located_candidates(world.config.crime.suspect_seen_ticks);
    match by {
        Some(p) => located.near(p.tile, claimed.radius),
        None => located.list.clone(),
    }
}

/// Open-warrant suspects that are located (and in reach of `by`), ascending.
pub fn located_suspects(world: &World, by: Option<Pursuer>) -> Vec<EntityId> {
    let claimed = claim_set(world, by);
    located_candidates(world, by, &claimed)
        .into_iter()
        .filter(|&(s, t)| chaseable(world, s, t, by, &claimed))
        .map(|(s, _)| s)
        .collect()
}

/// Pressure pass round 3, S3 (`OPEN-223`, `[justice] collar_hours`): the
/// suspect `guard` saw commit a crime past petty himself (the open report
/// he filed in the last `collar_hours`), when it is located and chaseable
/// by him: his collar, the Arrest goal's target on shift or off (a guard
/// at 100k witnessed 132 crimes in a week and arrested none: off shift the
/// Arrest goal is shut, and on shift the nearest warrant won). The newest.
pub fn collar_of(world: &World, guard: EntityId) -> Option<EntityId> {
    let hours = world.config.justice.as_ref()?.collar_hours;
    if hours == 0 || !is_guard(world, guard) {
        return None;
    }
    let since = world.tick.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    let mut mine: Vec<(Tick, EntityId)> = world
        .crime_reports()
        .iter()
        .filter(|r| !r.resolved && r.witness == Some(guard) && r.tick >= since && !is_petty(r.crime))
        .map(|r| (r.tick, r.suspect))
        .collect();
    // Round 4 T3 (OPEN-222): a chase handed to him (`chase_dropped`) is his
    // collar for as long.
    if let Some(&(s, t)) = world.handoffs.get(&guard) {
        if t >= since {
            mine.push((t, s));
        }
    }
    if mine.is_empty() {
        return None;
    }
    let tile = world.comp::<Position>(guard)?.tile;
    let radius = pursuit_radius_for(world, guard);
    let private = is_private_guard(world, guard);
    // `chaseable` but for the claim: the witness's collar is his before any
    // guard who heard the report (another's chase from across the district
    // claimed it first and lost the thief, while the guard who saw it walked
    // on to his desk).
    mine.into_iter()
        .filter(|&(_, s)| {
            s != guard
                && wanted(world, s)
                && located(world, s)
                && world.last_seen().get(&s).is_some_and(|&seen| {
                    seen.0.manhattan(tile) <= radius
                        && crate::systems::life::sighting_fresh_at(world, seen, tile)
                        && (!private || on_beat_at(world, guard, seen.0))
                })
                && !crate::systems::contracts::on_take_of(world, guard, s)
        })
        .max()
        .map(|(_, s)| s)
}

/// Round 4 T3 (`OPEN-222`): guard `guard`'s Arrest plan ends without the
/// arrest (a re-think's `winner`, or a failed step with `None`): one
/// `Pursuit` line says why (the warrant closed, the trail cold, another
/// guard on him, out of his reach, the shift over, another want first; a
/// failed step with the chase still on says nothing: he re-plans it) and,
/// while the suspect is still wanted and located, hands the case to the
/// nearest free guard on shift who can chase him (`handoffs`: his collar
/// for `[justice] collar_hours`). V12's `guard_8440 d23 11:10` chased Zane
/// Oakes four legs, then `Arrest -> Unwind` at 13:10 with no line (his
/// collar lapsed off shift); 20k seed 42 x 30 had 347 such ends. A chase
/// that made its arrest (escorting) or whose suspect is dead says nothing.
pub fn chase_dropped(world: &mut World, guard: EntityId, winner: Option<GoalKind>) {
    let Some(b) = world.comp::<Brain>(guard) else { return };
    if b.escorting.is_some() {
        return;
    }
    let Some(s) = b.plan.as_ref().filter(|p| p.goal == GoalKind::Arrest).and_then(|p| p.target) else { return };
    if s == guard || !living(world, s) {
        return;
    }
    let now = world.tick;
    let held = world.has::<Sentence>(s) || world.comp::<Brain>(s).is_some_and(|x| x.cuffed_by.is_some());
    let seen = world.last_seen().get(&s).copied();
    let here = world.comp::<Position>(guard).map(|p| p.tile);
    let on = world.comp::<Job>(guard).is_some_and(|j| {
        j.on_shift(world.tick_of_day()) && crate::exec::routine::workday_of(world, guard, j, j.shift_key_at(now))
    });
    let open = wanted(world, s) && !held;
    let out_of_reach = seen.zip(here).is_some_and(|((t, _), h)| t.manhattan(h) > pursuit_radius_for(world, guard));
    // A failed step (the suspect gone from the tile he was seen on, or he
    // fought free) with the chase still on: the guard re-plans it, silent.
    if winner.is_none() && open && located(world, s) && !world.claimed_by_other(guard, s) && !out_of_reach {
        return;
    }
    let why = if held {
        "already in custody".to_string()
    } else if !wanted(world, s) {
        "the warrant is closed".to_string()
    } else if !located(world, s) {
        match seen {
            Some((_, t)) => {
                format!("the trail went cold: last seen {} h ago", now.saturating_sub(t) / crate::time::TICKS_PER_HOUR)
            }
            None => "the trail went cold".to_string(),
        }
    } else if world.claimed_by_other(guard, s) {
        "another guard has the chase".to_string()
    } else if out_of_reach {
        "out of reach".to_string()
    } else if !on {
        "the shift is over".to_string()
    } else {
        winner.map_or_else(|| "got away".to_string(), |w| format!("{w:?} came first"))
    };
    let free = open && located(world, s) && !world.claimed_by_other(guard, s);
    let receiver =
        if free { seen.and_then(|(at, _)| handoff_guard(world, guard, s, at).map(|r| (r, at))) } else { None };
    let (gname, sname) = (world.name_of(guard), world.name_of(s));
    let text = match receiver {
        Some((r, at)) => {
            let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
            let since = now.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
            world.handoffs.retain(|_, &mut (_, t)| t >= since);
            world.handoffs.insert(r, (s, now));
            let far = world.comp::<Position>(r).map_or(0, |p| p.tile.manhattan(at));
            format!(
                "{gname} gave up the chase of {sname} ({why}): handed to {} on shift, {far} tiles off",
                world.name_of(r)
            )
        }
        None if free => {
            format!("{gname} gave up the chase of {sname} ({why}): no guard on shift near; the warrant stays open")
        }
        None => format!("{gname} gave up the chase of {sname} ({why})"),
    };
    let mut actors = vec![guard, s];
    actors.extend(receiver.map(|(r, _)| r));
    world.push_event(EventKind::Pursuit, &actors, text);
}

/// Pressure pass round 7, W3 (`OPEN-244`; `[justice] door_from_hour`..
/// `door_to_hour`): on the hour at night, each open warrant whose suspect no
/// guard has seen within `[crime] suspect_seen_ticks` (the chase never starts
/// without a sighting: 296 warrants stood at day 30 of 20k x 30, a guard made
/// 0.65 arrests a week) and who is inside his own Home goes to the nearest
/// free city guard on shift within his pursuit radius (`handoff_guard`'s
/// tests, one warrant a guard): the address is the sighting
/// (`World::set_last_seen` at the door, he is in) and the warrant his collar
/// (`handoffs`), so the arrest is made at home. Never a minor
/// (`ages::is_protected`), nor a warrant for sleeping rough alone.
pub(super) fn serve_at_door(world: &mut World) {
    let Some((from, to)) = world.config.justice.as_ref().map(|c| (c.door_from_hour, c.door_to_hour)) else { return };
    if from == to {
        return;
    }
    let hour = (world.tick_of_day() / crate::time::TICKS_PER_HOUR as u16) as u8;
    let open = if from < to { hour >= from && hour < to } else { hour >= from || hour < to };
    if !open {
        return;
    }
    let now = world.tick;
    let window = world.config.crime.suspect_seen_ticks;
    let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
    let since = now.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    let tod = world.tick_of_day();
    // The free city guards on shift, with bodies, holding no warrant now.
    let mut free: Vec<(EntityId, TilePos)> = world
        .guards()
        .iter()
        .copied()
        .filter(|&g| is_city_guard(world, g) && living(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| !world.handoffs.get(&g).is_some_and(|&(_, t)| t >= since))
        .filter(|&g| {
            world.comp::<Brain>(g).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical
                    && b.cuffed_by.is_none()
                    && b.escorting.is_none()
                    && b.plan_goal() != Some(GoalKind::Arrest)
            })
        })
        .filter(|&g| {
            world
                .comp::<Job>(g)
                .is_some_and(|j| j.on_shift(tod) && crate::exec::routine::workday_of(world, g, j, j.shift_key_at(now)))
                && !on_leave(world, g)
        })
        .filter_map(|g| world.comp::<Position>(g).map(|p| (g, p.tile)))
        .collect();
    if free.is_empty() {
        return;
    }
    let suspects: Vec<EntityId> = world.open_suspects().collect();
    for s in suspects {
        if free.is_empty() {
            break;
        }
        if !living(world, s)
            || world.has::<Sentence>(s)
            || crate::systems::ages::is_protected(world, s)
            || world.comp::<Brain>(s).is_none_or(|b| b.cuffed_by.is_some())
            || world.last_seen().get(&s).is_some_and(|&(_, t)| now.saturating_sub(t) <= window)
            || world.wanted_only_for_vagrancy(s)
            || world.claimed_by_other(EntityId::NONE, s)
        {
            continue;
        }
        let Some(home) = world.comp::<crate::components::Household>(s).and_then(|h| h.home) else { continue };
        if world.comp::<Position>(s).and_then(|p| p.building) != Some(home) {
            continue;
        }
        let Some(door) = world.comp::<Building>(home).map(|b| b.door) else { continue };
        let Some(i) = free
            .iter()
            .enumerate()
            .filter(|&(_, &(g, t))| {
                g != s
                    && t.manhattan(door) <= pursuit_radius_for(world, g)
                    && !crate::systems::contracts::on_take_of(world, g, s)
            })
            .min_by_key(|&(_, &(g, t))| (t.manhattan(door), g))
            .map(|(i, _)| i)
        else {
            continue;
        };
        let (g, gt) = free.swap_remove(i);
        if world.comp::<Brain>(s).is_some_and(|b| b.lod == crate::components::Lod::Statistical) {
            crate::systems::lod::set_lod(world, s, crate::components::Lod::Coarse);
        }
        world.set_last_seen(s, (door, now));
        world.handoffs.insert(g, (s, now));
        crate::systems::ownership::jobs2_row(world).law_door_taken += 1;
        let charge = wanted_for(world, s).map_or_else(String::new, |c| format!(" for {}", charge_label(world, s, c)));
        let text = format!(
            "{} took the warrant on {}{charge} to the door of {}, {} tiles off: home tonight, never seen on the street",
            world.name_of(g),
            world.name_of(s),
            world.name_of(home),
            gt.manhattan(door)
        );
        world.push_event(EventKind::Pursuit, &[g, s], text);
    }
}

/// `chase_dropped`'s hand-on: the nearest guard to `at` (the suspect's last
/// sighting; ties the lower id), not `from`, of the same force (a private
/// guard's only on his beat), at work on shift, free (no chase, no escort,
/// not a Statistical one, not on the suspect's take), within his pursuit
/// radius of `at` with the sighting fresh to him.
pub(super) fn handoff_guard(world: &World, from: EntityId, s: EntityId, at: TilePos) -> Option<EntityId> {
    let private = is_private_guard(world, from);
    let tod = world.tick_of_day();
    let seen = world.last_seen().get(&s).copied()?;
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| g != from && g != s && is_private_guard(world, g) == private)
        .filter(|&g| living(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| {
            world.comp::<Brain>(g).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical
                    && b.cuffed_by.is_none()
                    && b.escorting.is_none()
                    && b.plan_goal() != Some(GoalKind::Arrest)
            })
        })
        .filter(|&g| {
            world.comp::<Job>(g).is_some_and(|j| {
                j.on_shift(tod) && crate::exec::routine::workday_of(world, g, j, j.shift_key_at(world.tick))
            }) && !on_leave(world, g)
        })
        .filter_map(|g| world.comp::<Position>(g).map(|p| (g, p.tile)))
        .filter(|&(g, t)| {
            t.manhattan(at) <= pursuit_radius_for(world, g)
                && crate::systems::life::sighting_fresh_at(world, seen, t)
                && (!private || on_beat_at(world, g, at))
                && !crate::systems::contracts::on_take_of(world, g, s)
        })
        .min_by_key(|&(g, t)| (t.manhattan(at), g))
        .map(|(g, _)| g)
}

/// Is `located_suspects(world, by)` non-empty?
pub fn any_located_suspect(world: &World, by: Option<Pursuer>) -> bool {
    let claimed = claim_set(world, by);
    located_candidates(world, by, &claimed).into_iter().any(|(s, t)| chaseable(world, s, t, by, &claimed))
}

/// The pre-S1 `located_suspects` (every open warrant), for the tests.
#[cfg(test)]
pub(super) fn located_suspects_scan(world: &World, by: Option<Pursuer>) -> Vec<EntityId> {
    let claimed = claim_set(world, by);
    world
        .open_suspects()
        .filter(|&s| world.last_seen().get(&s).is_some_and(|&seen| chaseable(world, s, seen, by, &claimed)))
        .collect()
}

/// Is `s` a wanted, located suspect (anywhere)?
pub fn is_located_suspect(world: &World, s: EntityId) -> bool {
    wanted(world, s) && located(world, s)
}

/// The fine settling a fineable report: Vagrancy's, the dodger's (Transit
/// T10), else `fine_mult` meals.
pub(super) fn fine_for(world: &World, crime: Crime) -> i64 {
    match crime {
        Crime::Vagrancy => world.config.law.vagrancy_fine,
        Crime::FareEvasion => world.config.transit.dodge_fine,
        _ => world.config.crime.fine_mult * world.mean_price(),
    }
}

/// A petty offence: a night in the cells (M12 D15: Vagrancy; Transit T10:
/// fare evasion; Jobs & lives phase 3, § 4: an unlicensed weapon; phase 4:
/// Solicitation), fined whenever the suspect can pay (Shadow V4 bug 4).
pub fn is_petty(crime: Crime) -> bool {
    matches!(crime, Crime::Vagrancy | Crime::FareEvasion | Crime::UnlicensedArms | Crime::Solicitation)
}

/// The worst open warrant on `suspect` (the one its sentence would be for).
pub fn wanted_for(world: &World, suspect: EntityId) -> Option<Crime> {
    world.crime_reports().iter().filter(|r| !r.resolved && r.suspect == suspect).map(|r| r.crime).max()
}

/// Pressure pass round 3, S3 (OPEN-223): the charge as a line reads it, the
/// crime and whom it was done to when the report's witness saw that
/// ("Assault on Thea Kemp"): an arrest of a sleeper read as the sleeper
/// attacked, with the warrant nowhere in his diary. The victim comes from
/// the newest report of `crime` on `suspect`, its witness's `SawCrime`
/// memory; none (an off-screen report, a memory gone): the crime alone.
pub fn charge_label(world: &World, suspect: EntityId, crime: Crime) -> String {
    let prep = match crime {
        Crime::Assault | Crime::Abduction => "on",
        Crime::Murder | Crime::Manslaughter => "of",
        Crime::Theft | Crime::GrandTheft | Crime::Extortion | Crime::Blackmail | Crime::Fraud => "from",
        _ => return crime.label().to_string(),
    };
    let victim = world
        .crime_reports()
        .iter()
        .filter(|r| r.suspect == suspect && r.crime == crime)
        .max_by_key(|r| (!r.resolved, r.tick))
        .and_then(|r| r.witness)
        .and_then(|w| world.comp::<crate::components::Memory>(w))
        .and_then(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(suspect) && e.crime == Some(crime))
                .max_by_key(|e| e.tick)
                .and_then(|e| e.object)
        })
        .filter(|&v| v != suspect && world.has::<crate::components::Identity>(v));
    match victim {
        Some(v) => format!("{} {prep} {}", crime.label(), world.name_of(v)),
        None => crime.label().to_string(),
    }
}

/// Sentence length in ticks for a crime at the current lever.
pub fn sentence_ticks(world: &World, crime: Crime) -> Tick {
    // M12 D15: a night in the cells, whatever the sentence lever.
    if is_petty(crime) {
        return world.config.law.vagrancy_sentence_ticks.max(1);
    }
    // M16a (plan C29): Conspiracy is `accessory_mult` (or the lever's) of Murder's.
    if crime == Crime::Conspiracy {
        let mult = world.levers.accessory_mult.unwrap_or(world.config.law.accessory_mult);
        let murder = sentence_ticks(world, Crime::Murder);
        return ((murder as f32 * mult).ceil() as Tick).max(TICKS_PER_DAY);
    }
    // M13 D47: the crimes appended after Vagrancy read `sentence_days_ext`.
    let ext = &world.config.crime.sentence_days_ext;
    let base = match crime {
        Crime::GrandTheft => ext.grand_theft,
        Crime::Manslaughter => ext.manslaughter,
        Crime::Abduction => ext.abduction,
        // Jobs & lives phase 5: Abduction's term.
        Crime::Trafficking => ext.abduction,
        // Jobs & lives phase 6: Fraud's and Dealing's terms.
        Crime::Forgery => ext.fraud,
        Crime::BlackMarket => ext.dealing,
        Crime::Dealing => ext.dealing,
        // M14 V18.
        // V42 `SetHackSentence` replaces the base.
        Crime::Intrusion => world.levers.hack_sentence_days[0].map_or(ext.intrusion, u32::from),
        Crime::DataTheft => world.levers.hack_sentence_days[1].map_or(ext.data_theft, u32::from),
        // M16b § 4.
        Crime::Fraud => ext.fraud,
        Crime::Blackmail => ext.blackmail,
        // Jobs & lives phase 3 (§ 4).
        Crime::ArmsDealing => ext.arms_dealing,
        Crime::Smuggling => ext.smuggling,
        // Pressure pass round 7, W5 (`OPEN-237`).
        Crime::CampEscape => ext.camp_escape,
        _ => world.config.crime.sentence_days[crime as usize],
    } as f32;
    let days = (base * world.levers.sentence_mult).ceil().max(1.0) as u64;
    days * TICKS_PER_DAY
}
