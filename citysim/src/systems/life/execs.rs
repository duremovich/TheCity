use super::*;

// ---------------------------------------------------------------------------
// Execs
// ---------------------------------------------------------------------------

/// An exec's day salary: `exec_pay_frac` of the corp's treasury, floored at
/// `[economy] wage_exec` and capped at `exec_pay_cap` (the flat wage when off).
pub fn exec_pay(world: &World, corp: EntityId) -> i64 {
    let floor = world.config.economy.wage_exec;
    let t = world.comp::<Corp>(corp).map_or(0, |c| c.treasury);
    let scaled = (t.max(0) as f32 * world.config.life.exec_pay_frac).round() as i64;
    scaled.clamp(floor, world.config.life.exec_pay_cap.max(floor))
}

/// The exec pick's score (higher is better): an adult of at least
/// `exec_min_age_years` (else ineligible), by wealth (coins), then
/// persuasion + knowledge, then age. `None` for the ineligible. Wealth
/// leads: picking by the exec skill first put a top-percentile exec at
/// every corp, and M15's competence (normalised on the old pick) rose ~10 %
/// city-wide, Farm output with it.
pub fn exec_score(world: &World, a: EntityId) -> Option<(i64, i64, u32)> {
    let ident = world.comp::<crate::components::Identity>(a)?;
    let min_days = (world.config.life.exec_min_age_years * crate::time::DAYS_PER_YEAR as f32) as u32;
    if ident.age_days < min_days {
        return None;
    }
    let skill = world.comp::<crate::components::Skills>(a).map_or(0.0, |s| s.persuasion + s.knowledge);
    Some((coins(world, a), (skill * 1000.0).round() as i64, ident.age_days))
}

/// L1b: the exec from `candidates`: among the eligible (`exec_score`), the
/// wealthiest tenth (at least 5), and of those the greediest (ties: the
/// exec skill, then the lower id). The corp brain reads its exec's greed
/// (Squeeze scores greed², Acquire greed): with wealth alone the execs were
/// a random draw of greed and no corp ever held Squeeze or bid hostile
/// (seeds 42-44: Squeeze 1 of 3, hostile Acquired 0).
pub fn pick_exec(world: &World, candidates: impl Iterator<Item = EntityId>) -> Option<EntityId> {
    let mut rich: Vec<((i64, i64, u32), EntityId)> =
        candidates.filter_map(|a| exec_score(world, a).map(|k| (k, a))).collect();
    rich.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
    let n = if world.config.life.exec_greed { (rich.len() / 10).max(5).min(rich.len()) } else { rich.len().min(1) };
    rich.truncate(n);
    let greed = |a: EntityId| world.comp::<crate::components::Personality>(a).map_or(0.0, |p| p.greed);
    rich.into_iter()
        .max_by(|x, y| greed(x.1).total_cmp(&greed(y.1)).then(x.0 .1.cmp(&y.0 .1)).then(y.1.cmp(&x.1)))
        .map(|(_, a)| a)
}

/// The corp's HQ for its exec's office hours: the corp's most senior
/// working building (its Hq first), the nearest the exec's Home of that
/// kind, then the lower id; a corp with none of the ranked kinds gives a
/// desk at any other working building it holds, never a Home or a Lot
/// (Shadow V4 review 7; bug 5: distance
/// came first and there was no Hq in the ranking, so CEOs held office at
/// Vat Farms and Arasaka's head moved to the Sump).
pub fn hq_of(world: &World, corp: EntityId, exec: EntityId) -> Option<EntityId> {
    // R6b (OPEN-217): the Market is a street market's stalls, no office (a
    // food corp's CEO sat at a stall with no Job, V10): it ranks with the
    // unranked kinds, after the packing plant's and the freight yard's
    // offices.
    const RANK: [BuildingKind; 12] = [
        BuildingKind::Hq,
        BuildingKind::Lab,
        BuildingKind::SecurityOffice,
        BuildingKind::Feed,
        BuildingKind::PackingPlant,
        BuildingKind::FreightYard,
        BuildingKind::Clinic,
        BuildingKind::Garage,
        BuildingKind::Hotel,
        BuildingKind::Bar,
        BuildingKind::Farm,
        BuildingKind::Warehouse,
    ];
    let c = world.comp::<Corp>(corp)?;
    let from = home_of(world, exec).and_then(|h| world.comp::<Building>(h)).map(|b| b.door);
    c.buildings
        .iter()
        .copied()
        .filter_map(|b| {
            let bd = world.comp::<Building>(b).filter(|bd| {
                !bd.demolished && !bd.derelict && !matches!(bd.kind, BuildingKind::Home | BuildingKind::Lot)
            })?;
            let rank = RANK.iter().position(|&k| k == bd.kind).unwrap_or(RANK.len());
            Some((rank, from.map_or(0, |f| f.manhattan(bd.door)), b))
        })
        .min()
        .map(|(_, _, b)| b)
}

/// R6b (OPEN-207): the exec's office: the corp's Hq or its best working
/// building (`hq_of`), else, for a landlord corp (Homes and Lots only) under
/// `[verbs] landlord_desk`, a lettings desk in its standing Block nearest
/// the exec's Home (then the lower id). `None` for a corp with neither.
pub fn office_of(world: &World, corp: EntityId, exec: EntityId) -> Option<EntityId> {
    hq_of(world, corp, exec).or_else(|| lettings_desk(world, corp, exec))
}

/// R6b (OPEN-207): a landlord corp's lettings desk (`office_of`).
pub fn lettings_desk(world: &World, corp: EntityId, exec: EntityId) -> Option<EntityId> {
    if !world.config.verbs.as_ref().is_some_and(|v| v.landlord_desk) {
        return None;
    }
    let c = world.comp::<Corp>(corp)?;
    let from = home_of(world, exec).and_then(|h| world.comp::<Building>(h)).map(|b| b.door);
    c.buildings
        .iter()
        .copied()
        .filter_map(|b| {
            let bd = world
                .comp::<Building>(b)
                .filter(|bd| !bd.demolished && !bd.derelict && bd.kind == BuildingKind::Home)?;
            Some((from.map_or(0, |f| f.manhattan(bd.door)), b))
        })
        .min()
        .map(|(_, b)| b)
}

/// The corp `id` is the exec of.
pub fn exec_corp(world: &World, id: EntityId) -> Option<EntityId> {
    world.corp_slice().iter().copied().find(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(id)))
}

/// An exec's office day to keep: on, an exec of a corp with an HQ, not
/// jailed, today's office hours not yet kept, and the shift on (or about to
/// start within the walk).
pub fn exec_day_pending(world: &World, id: EntityId) -> Option<EntityId> {
    if world.has::<Sentence>(id) || world.has::<Job>(id) {
        return None;
    }
    let today = world.day();
    if world.comp::<Brain>(id).is_some_and(|b| b.office_day == Some(today)) {
        return None;
    }
    let corp = exec_corp(world, id)?;
    let hq = office_of(world, corp, id)?;
    let (s, e) = world.config.life.exec_shift;
    let tod = u64::from(world.tick_of_day());
    if !crate::exec::routine::is_workday(today as i64) || tod >= u64::from(e) {
        return None;
    }
    // L2 shadow fixes item 1: the walk to the office, once started today,
    // holds (the CEO set out and turned back up to four times a morning).
    let started = world
        .comp::<Brain>(id)
        .and_then(|b| b.plan.as_ref())
        .filter(|p| p.goal == GoalKind::Work && p.target == Some(hq))
        .is_some_and(|p| p.started_tick / TICKS_PER_DAY == today);
    if started {
        return Some(hq);
    }
    let door = world.comp::<Building>(hq).map_or_else(Default::default, |b| b.door);
    // Review item 6: the trip to arrive 30 ticks before the office opens,
    // read against that deadline (`transit::trip_due`, the departure
    // gate's), not at "now": a now-based estimate moves with the timetable
    // and the 05:00 start of service, and turned the exec back.
    let until = u64::from(s).saturating_sub(tod);
    crate::systems::transit::trip_due(world, id, door, until, 30).then_some(hq)
}

/// Lives at work phase 7 (§ 5.3): the Home tier a corp exec lives at or
/// above (`[life] exec_home_tier`); 0 off.
pub fn exec_home_tier(world: &World) -> u8 {
    if world.config.lives.enabled {
        world.config.life.exec_home_tier
    } else {
        0
    }
}

/// Lives at work phase 7 (§ 5.3), daily before the relocation: every corp
/// exec housed under `exec_home_tier` moves up when a Home there has room
/// ([`house_exec`]); nobody is evicted for it. An exec under a grown child's
/// roof stays (`ages::with_kin`: the first build moved an elder exec up
/// each midnight and the kin take-in moved her back the next). A no-op off.
pub fn spire_homes(world: &mut World) {
    if exec_home_tier(world) == 0 {
        return;
    }
    for e in crate::systems::classes::exec_set(world) {
        if crate::systems::law::living(world, e)
            && !world.has::<Sentence>(e)
            && !crate::systems::ages::with_kin(world, e)
        {
            house_exec(world, e);
        }
    }
}

/// Lives at work phase 7 (§ 5.3): the execs, those housed at
/// `exec_home_tier` or better, the beds of that tier, those free, and the
/// beds wanted (`execs × exec_home_spare`, rounded up). Printed at seed
/// (the 20k Spire's supply), never acted on.
pub fn spire_supply(world: &World) -> (usize, usize, usize, usize, usize) {
    let tier = exec_home_tier(world).max(1);
    let execs = crate::systems::classes::exec_set(world);
    let housed = execs
        .iter()
        .filter(|&&e| home_of(world, e).and_then(|h| world.comp::<Building>(h)).is_some_and(|b| b.tier >= tier))
        .count();
    let (mut beds, mut free) = (0usize, 0usize);
    for &h in world.buildings_of_kind(BuildingKind::Home) {
        if let Some(b) = world.comp::<Building>(h).filter(|b| b.tier >= tier && !b.demolished && !b.derelict) {
            beds += usize::from(b.capacity);
            free += usize::from(b.capacity).saturating_sub(world.residents_of(h).len());
        }
    }
    let want = (execs.len() as f32 * world.config.life.exec_home_spare).ceil() as usize;
    (execs.len(), housed, beds, free, want)
}

/// The day's end: the corp execs and those housed at `exec_home_tier` or
/// better (`execs`, `execs_spire`). A no-op off.
pub fn snapshot(world: &mut World) {
    if exec_home_tier(world) == 0 {
        return;
    }
    let (execs, housed, ..) = spire_supply(world);
    let row = crate::systems::ownership::jobs2_row(world);
    row.execs = execs as u32;
    row.execs_spire = housed as u32;
}

/// The perf pass phase 10 (I9): an exec's corp Hq door when
/// `[life] exec_home_reach` is on (`hq_of`, ranked from his Home); `None`
/// off, or for an exec with no corp or no standing workplace.
pub fn exec_hq_door(world: &World, exec: EntityId) -> Option<TilePos> {
    if world.config.life.exec_home_reach == 0 {
        return None;
    }
    let corp = exec_corp(world, exec)?;
    hq_of(world, corp, exec).and_then(|b| world.comp::<Building>(b)).map(|b| b.door)
}

/// Move an exec (with a spouse and children sharing the Home) into a free
/// Spire Home (tier 2; Lives at work phase 7: `exec_home_tier` when on)
/// when they live lower down and one has room. The perf pass phase 10
/// (ISLAND_PASS I9, `[life] exec_home_reach`): the free Home nearest his
/// corp's Hq (door to door, then the id) instead of the emptiest, and an
/// exec housed at the tier but further than the reach moves to a nearer
/// one within it when one has room.
pub fn house_exec(world: &mut World, exec: EntityId) {
    let Some(home) = home_of(world, exec) else { return };
    // OPEN-140: one move a body a midnight (an exec tied as a super was
    // moved into the Spire and back to the landlord's Block every night).
    if housed_now(world, exec) {
        return;
    }
    let tier = match exec_home_tier(world) {
        0 => 2,
        t => t,
    };
    let at_tier = world.comp::<Building>(home).is_some_and(|b| b.tier >= tier);
    let reach = u32::from(world.config.life.exec_home_reach);
    // I9: the Hq's door and how far the Home stands from it.
    let near = exec_hq_door(world, exec).map(|hq| {
        let d = world.comp::<Building>(home).map_or(u32::MAX, |b| b.door.manhattan(hq));
        (hq, d)
    });
    if at_tier && near.is_none_or(|(_, d)| d <= reach) {
        return;
    }
    let spouse = world.spouse_of(exec).filter(|&s| home_of(world, s) == Some(home) && world.has::<Brain>(s));
    let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, exec)
        .into_iter()
        .filter(|&c| world.has::<crate::components::Child>(c) && home_of(world, c) == Some(home))
        .collect();
    let need = 1 + usize::from(spouse.is_some()) + kids.len();
    let pick = world.buildings_of_kind(BuildingKind::Home).iter().copied().filter(|&h| {
        world.comp::<Building>(h).is_some_and(|b| {
            b.tier >= tier
                    && !b.demolished
                    && !b.derelict
                    && world.residents_of(h).len() + need <= usize::from(b.capacity)
                    // The island pass phase 3a (I6): a company bed is its staff's only.
                    && crate::systems::company::open_to_all(world, h)
        })
    });
    let pick = match near {
        // I9: the nearest to the Hq; housed at the tier, only a nearer Home
        // within the reach.
        Some((hq, d)) => pick
            .filter_map(|h| world.comp::<Building>(h).map(|b| (b.door.manhattan(hq), h)))
            .filter(|&(dh, _)| !at_tier || (dh <= reach && dh < d))
            .min()
            .map(|(_, h)| h),
        None => pick.min_by_key(|&h| (world.residents_of(h).len(), h)),
    };
    let Some(to) = pick else { return };
    let tick = world.tick;
    let place = world.name_of(to);
    for m in std::iter::once(exec).chain(spouse) {
        world.set_home(m, Some(to));
        let name = world.name_of(m);
        world.push_event(EventKind::Housed, &[m, to], format!("{name} moved into {place} (the Spire)"));
    }
    let door = world.comp::<Building>(to).map(|b| b.door).unwrap_or_default();
    for c in kids {
        world.set_home(c, Some(to));
        world.remove_from_building(c);
        if let Some(p) = world.comp_mut::<Position>(c) {
            p.tile = door;
            p.building = None;
            p.floor = 0;
            p.entered = tick;
        }
        world.enter_building(c, to);
    }
}

/// An exec's office hours at the HQ: `GoTo(Seller)` bound to the HQ, then
/// `Meeting` until the shift ends. `None` without an office day to keep.
pub fn exec_plan(world: &World, id: EntityId) -> Option<crate::goap::Plan> {
    let hq = exec_day_pending(world, id)?;
    let mut steps = Vec::new();
    if world.comp::<Position>(id).and_then(|p| p.building) != Some(hq) {
        steps.push(crate::components::ActionInstance {
            action: crate::goap::ActionKind::GoTo(LocationKey::Seller),
            target: Some(hq),
            tile: None,
        });
    }
    steps.push(crate::components::ActionInstance {
        action: crate::goap::ActionKind::Meeting,
        target: Some(hq),
        tile: None,
    });
    Some(crate::goap::Plan { goal: GoalKind::Work, target: Some(hq), steps, started_tick: world.tick })
}

/// Ticks until today's office shift ends (an early arrival waits for it; 0
/// when it is over).
pub fn exec_shift_left(world: &World) -> Tick {
    let (_, e) = world.config.life.exec_shift;
    Tick::from(e.saturating_sub(world.tick_of_day()))
}

/// Today's office shift is over.
pub fn exec_shift_over(world: &World) -> bool {
    world.tick_of_day() >= world.config.life.exec_shift.1
}

/// Days a quitter is not rehired by the employer it left.
pub fn quit_blocks(world: &World, a: EntityId, employer: EntityId) -> bool {
    // Shadow V4 bug 6: `EntityId::NONE` blocks every employer (a secure quit).
    world
        .comp::<Brain>(a)
        .and_then(|b| b.quit_from)
        .is_some_and(|(e, until)| (e == employer || e == EntityId::NONE) && world.tick < until)
        // Pressure pass F3: the idle days after a quit or a firing, the
        // firing employer's refusal, the reputation's pass-over.
        || crate::systems::misery::hire_blocked(world, a, employer)
}

/// Shadow V4 bug 6: a "no need of the wage" quit keeps the quitter out of
/// every hire for `quit_rehire_days` (another employer rehired him the
/// same midnight).
pub fn note_secure_quit(world: &mut World, a: EntityId) {
    let until = world.tick + world.config.life.quit_rehire_days * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(a) {
        b.quit_from = Some((EntityId::NONE, until));
    }
}

/// Record a quit (`quit_rehire_days` before that employer hires them again).
pub fn note_quit(world: &mut World, a: EntityId, employer: Option<EntityId>) {
    let until = world.tick + world.config.life.quit_rehire_days * TICKS_PER_DAY;
    if let (Some(e), Some(b)) = (employer, world.comp_mut::<Brain>(a)) {
        b.quit_from = Some((e, until));
    }
}

/// L1: the daily dole paid where the agent is, at the Work phase's start
/// (09:00), to every free jobless adult body (Full and Coarse; the
/// Statistical tier keeps its own daily roll) below its savings line, as
/// the Full Earn goal's gate reads it; execs draw a salary instead. Same
/// amount, same day: `economy::collect_dole` pays it (treasury not negative,
/// once a day). The Hall walk it replaces was 2-4 h a day for 4 coins.
pub fn dole_in_place(world: &mut World) {
    if !world.config.life.dole_in_place {
        return;
    }
    let execs: std::collections::BTreeSet<EntityId> =
        world.corps().into_iter().filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.exec)).collect();
    let today = world.day();
    let due: Vec<EntityId> = world
        .bodies()
        .into_iter()
        .filter(|&a| {
            world.comp::<Brain>(a).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical && !b.emigrating && b.last_dole_day != Some(today)
            }) && crate::systems::economy::dole_eligible(world, a)
                && !world.has::<Sentence>(a)
                && !execs.contains(&a)
                && crate::systems::demography::is_adult(world, a)
                && coins(world, a) < crate::goap::world_state::SAVINGS_DAYS.saturating_mul(meal_price(world, a))
        })
        .collect();
    for a in due {
        crate::systems::economy::collect_dole(world, a);
    }
}
