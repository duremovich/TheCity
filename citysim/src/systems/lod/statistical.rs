use super::*;

/// The agent's table row: the current phase, and its lawfulness and hunger
/// at the start of the hour (before the hour's decay), as `calibrate` buckets.
/// Under `[lod] policy = "mlp"` the learned policy's row instead
/// (`systems::stat_policy`); the draws are the same either way.
fn stat_row(world: &World, id: EntityId) -> Option<StatRow> {
    use crate::systems::stat_policy::{MlpPolicy, StatPolicy, TablePolicy};
    let t = world.stat_table.as_ref()?;
    Some(match world.stat_mlp.as_ref() {
        Some(m) => MlpPolicy(m).row(world, id),
        None => TablePolicy(t).row(world, id),
    })
}

/// Statistical agents whose hourly slot is this tick: `id.index % 60 == tick % 60`.
/// Each agent keeps a fixed minute of the hour, so it runs 24 times a day and
/// the load is spread evenly over the ticks.
/// Reads one slot bucket (`World::stat_slots`), so the cost is O(due agents).
pub fn due_this_tick(world: &World) -> Vec<EntityId> {
    // M17 phase 5: a crew away on a job abroad is skipped (no decay, no outcome).
    world
        .stat_slots
        .at(world.tick)
        .iter()
        .copied()
        .filter(|&id| !world.has::<Sentence>(id) && !crate::systems::abroad::is_away(world, id))
        .collect()
}

/// The hourly stand-in for a Statistical brain: an hour of need decay, then
/// one outcome drawn from the calibrated row with the agent's own stream. It
/// runs every tick for the agents due this tick (`due_this_tick`).
pub fn run_statistical(world: &mut World) {
    let agents = due_this_tick(world);
    if agents.is_empty() {
        return;
    }
    if world.stat_table.is_none() {
        if world.stat_table_legacy {
            panic!(
                "assets/stat_table.toml is the pre-M10 four-row table: run `cargo run --release -p citysim-cli -- calibrate`"
            );
        }
        panic!("assets/stat_table.toml is missing: run `cargo run --release -p citysim-cli -- calibrate`");
    }
    let cfg = world.config.needs.clone();
    let season = world.season().index();
    let season_energy_mult = world.config.economy.energy_decay_mult[season];
    let phase = world.phase();
    // L2 L13: the hour's `fun` decay off screen (the exec set once).
    let fun_execs = Some(crate::systems::classes::exec_set(world));
    // The recalibration (2026-10-09, ported with hunger as sickness P5): the
    // dole city reads the scavenge and beg path too, whenever the table
    // carries it (`calibrate` learns it in every city; without it the
    // off-screen broke of the dole city had the dole alone).
    let earn_path = world.stat_table.as_ref().is_some_and(|t| t.p_scavenge.is_some());
    for id in agents {
        let Some(row) = stat_row(world, id) else { continue };
        // Flip readiness (no safety net): the table row's index at the hour's
        // start, for the income path's per-row rates.
        let no_net_row = if earn_path { stat_row_index(world, id) } else { None };
        // Parity (arrests): the hour's row for the El's non-commute ride.
        let ride_row = stat_row_index(world, id);
        // § 4 (parity, after P5): the dive is its own hour, on the row's
        // `p_dive` (every eligible hour without a learned rate), as `calibrate`
        // learns it: dives begun per hour with a bin in reach at the hour's
        // start (before the hour's decay, as `calibrate` reads eligibility).
        if crate::systems::bins::on(world) && crate::systems::demography::is_adult(world, id) {
            let p = match (world.stat_table.as_ref().and_then(|t| t.p_dive), stat_row_index(world, id)) {
                (Some(rates), Some(r)) => rates[r],
                _ => 1.0,
            };
            crate::prof::timed!("stat::stat_dive", crate::systems::bins::stat_dive(world, id, p));
        }
        // 1. An hour of decay in one step.
        let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
        let under_18 = !crate::systems::demography::is_adult(world, id);
        let energy_mult = crate::systems::stims::energy_mult(world, id);
        let energy_mult = crate::systems::health::energy_mult(world, id, energy_mult);
        // Parity (after P5): a Full sleeper's hunger decays at half rate; off
        // screen the hour's sleep is the row's `p_sleep` share, so (with a
        // table holding `p_meal`, learned with it) the hunger lost is cut by
        // `0.5 x p_sleep`, as a Full agent sleeping that share would lose.
        let sleep_share = if world.stat_table.as_ref().is_some_and(|t| t.p_meal.is_some()) { row.p_sleep } else { 0.0 };
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            let ctx = crate::needs::DecayCtx {
                season_energy_mult,
                sociability,
                under_18,
                energy_mult,
                ..crate::needs::DecayCtx::plain()
            };
            let before = n.hunger;
            crate::needs::decay(n, &cfg, &ctx, TICKS_PER_HOUR as u32);
            if sleep_share > 0.0 {
                n.hunger = (n.hunger + (before - n.hunger) * 0.5 * sleep_share).min(1.0);
            }
        }
        // Diet parity (hunger fix item 11's off-screen side): the grocery
        // run. Round 7 W1 (OPEN-243): before the hour's leisure (the
        // household's food ahead of the adults' own wants).
        crate::prof::timed!("stat::stat_grocery", stat_grocery(world, id));
        if let Some(execs) = &fun_execs {
            crate::prof::timed!("stat::leisure::stat_hour", crate::systems::leisure::stat_hour(world, id, execs));
        }
        crate::prof::timed!("stat::starvation", crate::needs::starvation(world, id));
        if !world.has::<Brain>(id) {
            continue; // starved this hour
        }
        // Hunger as sickness § 2: the hour's hungry mark (bodies: needs::run).
        crate::systems::health::hourly_mark(world, id);
        // 2b. Work is not a gamble: an employed agent on shift works the hour
        // (a Full farmer works every hour of the shift), and the jobless draw
        // the dole once a day. The table's p_work mass then stands for the
        // idle hours of the employed.
        crate::prof::timed!("stat::stat_work", stat_work(world, id, phase, no_net_row));
        // Review item 1: a dodger caught at the commute's gate is on screen
        // now (the arrest path, `transit::stat_commute`): the rest of the
        // hour is its body's, as after the errand ride's catch below.
        if world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical) {
            continue;
        }
        // Parity (arrests): a non-commute ride on the El (`p_errand_ride`);
        // a dodger caught at the gate comes on screen (the arrest path).
        if let Some(r) = ride_row.filter(|_| crate::systems::demography::is_adult(world, id)) {
            if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical)
                && crate::systems::transit::stat_errand(world, id, r)
                && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
            {
                continue;
            }
        }
        // 3. One outcome. A Full agent eats when hungry, not by lottery, so a
        // hungry Statistical agent eats if it can and the draw covers the
        // discretionary meals of the fed.
        // ... and a sated one does not (a Full agent skips Eat above ~0.7).
        let hunger = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
        let (hungry, sated) = (hunger < 0.4, hunger >= 0.7);
        let u: f32 = world.rng.agent(id).random();
        // Parity (hunger as sickness, after P5): a Full agent neither eats
        // the hour it gets hungry (the walk, the queue, a goal ahead of Eat)
        // nor only when hungry (a meal bought on the way): with the table's
        // `p_meal` an agent eats on its phase's and hunger band's share of
        // hours (a pure hash: no stream moves); without it the M10 rule
        // (every hungry hour eats, `p_eat` the rest).
        let on_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(world.tick_of_day()));
        let meal_p = world.stat_table.as_ref().and_then(|t| t.meal_rate(phase, hunger, on_shift));
        // `p_meal` is the means' rate (food in hand, a pantry, a meal's
        // coins); the broke eat through their own hourly rolls: the bin
        // (`p_dive`, above), the noon Mission, and the hungry broke hour's
        // theft (`p_desperate`, in `stat_eat`), each learned per such hour.
        let means = meal_p.is_some() && has_means(world, id);
        let eats = match meal_p {
            Some(p) if means => {
                crate::systems::econ::hash_unit(world.seed(), 0x4e47_0001, u64::from(id.index), world.tick) < p
            }
            Some(_) => hungry,
            None => hungry || (u < row.p_eat && !sated),
        };
        let outcome = if eats {
            Outcome::Eat
        } else if u < row.p_eat + row.p_work {
            Outcome::Idle
        } else if u < row.p_eat + row.p_work + row.p_social {
            Outcome::Social
        } else if u < row.p_eat + row.p_work + row.p_social + row.p_sleep {
            Outcome::Sleep
        } else {
            Outcome::Idle
        };
        match outcome {
            Outcome::Eat => {
                crate::prof::timed!("stat::stat_eat", stat_eat(world, id));
                // L2 shadow fixes item 8: a hungry hour whose Eat came to
                // nothing sleeps at night (two Statistical workers read
                // energy 0.00 for 92 and 122 h: every night hour drew Eat).
                let after = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
                if hungry && phase == DayPhase::Night && after <= hunger {
                    stat_sleep_night(world, id);
                }
            }
            Outcome::Social => crate::prof::timed!("stat::stat_social", stat_social(world, id)),
            Outcome::Sleep => {
                if phase == DayPhase::Night {
                    stat_sleep_night(world, id);
                }
            }
            Outcome::Idle => {}
        }
        // 4. The independent rolls: the agent's own crimes and courtship, and
        // what is done to it by an actor nobody has drawn yet (a hole).
        crate::prof::timed!("stat::stat_rolls", stat_rolls(world, id, &row));
        if world.has::<Brain>(id) {
            world.recompute_wealth_for(id);
        }
    }
}

/// A Statistical night's Sleep: as a Full agent's Sleep at home: energy
/// back, and a cohabiting spouse keeps the marriage warm.
fn stat_sleep_night(world: &mut World, id: EntityId) {
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    if home.is_some() {
        world.mark_day(id, trace_flags::SLEPT_AT_HOME);
    } else if crate::systems::street::booked_hotel(world, id).is_some() {
        // M12 D21: a booked bed is a bed.
        world.mark_day(id, trace_flags::SLEPT_AT_HOME | trace_flags::HOTEL);
    } else if world.has::<crate::components::Squatter>(id) {
        world.mark_day(id, trace_flags::SQUAT);
    }
    let with_spouse =
        home.is_some() && world.spouse_of(id).is_some_and(|s| world.comp::<Household>(s).and_then(|h| h.home) == home);
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        n.energy = 1.0;
        if with_spouse {
            n.intimacy = (n.intimacy + 0.4).min(1.0);
        }
    }
}

/// A first meeting off screen (the `p_meet` roll): a stranger from the
/// agent's zone, at the v1 first-meeting affinity. No Chat's drift: a Full
/// first meeting (`social::colocation`) is the bare edge, Chats are the
/// separately calibrated `p_chat` roll, and drifting every new acquaintance
/// past the prune line kept them all (the 2,000 save grew 30 -> 48 MB).
fn stat_meet(world: &mut World, id: EntityId) {
    let Some(stranger) = stranger_in_zone(world, id) else { return };
    let sa = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
    let sb = world.comp::<Personality>(stranger).map_or(0.5, |p| p.sociability);
    let rng = world.rng.agent(id);
    let (u1, u2): (f32, f32) = (rng.random(), rng.random());
    let affinity = crate::systems::social::first_affinity(sa, sb, u1, u2);
    crate::systems::social::first_meeting(world, id, stranger, affinity);
}

/// A stranger for an off-screen meeting: up to `STRANGER_TRIES` uniform
/// draws from the Statistical tier on the agent's stream, the first standing
/// in the agent's zone taken (rejection sampling: uniform over that zone's
/// Statistical agents). `None` when no draw lands in the zone, or the one
/// that does is a child, the agent itself or someone it already knows: no
/// meeting this hour. Only Statistical strangers: bodies meet by co-location,
/// and drawing them gave the gang members and guards hundreds of
/// acquaintances that every think walks. A scan of the zone's Homes per
/// meeting cost 7 % of the 2,000 city's throughput.
pub(crate) fn stranger_in_zone(world: &mut World, id: EntityId) -> Option<EntityId> {
    const STRANGER_TRIES: usize = 16;
    let zone_of = |w: &World, o: EntityId| w.comp::<Position>(o).map(|p| w.map.zone(p.tile));
    let zone = zone_of(world, id)?;
    let n = world.tier(Lod::Statistical).len();
    if n == 0 {
        return None;
    }
    for _ in 0..STRANGER_TRIES {
        let k = world.rng.agent(id).random_range(0..n);
        let o = world.tier(Lod::Statistical)[k];
        if zone_of(world, o) != Some(zone) {
            continue;
        }
        // L2 (L21): a held prisoner is in the cells, nobody's stranger.
        let ok = o != id
            && crate::systems::demography::is_adult(world, o)
            && world.edge(id, o).is_none()
            && !world.has::<Sentence>(o);
        return ok.then_some(o);
    }
    None
}

/// The seven independent hourly rolls, always drawn from the agent's stream in
/// the fixed order steal, flirt, robbed, assaulted, killed, meet, chat (so the stream
/// advances identically whatever fires), then applied killed first. An agent
/// whose hour already took it off the tier (a caught thief) only draws.
/// The hour's violence and theft probabilities a Statistical agent rolls
/// against: `(p_killed, p_assaulted, p_robbed, p_steal)`, the row's times
/// `stat_violence_mult`, then (M13 D46) only for a kitted agent `p_killed`
/// and `p_assaulted × (1 − dodge_w × Kit.reflex) × (1 − Kit.armour)` and
/// `p_robbed × (1 + flash_w × Kit.flash)`. A bare agent's are the M12
/// products bit for bit (a branch, not a multiply by 1).
pub fn stat_probs(world: &World, id: EntityId, row: &StatRow) -> (f32, f32, f32, f32) {
    let m = world.config.lod.stat_violence_mult;
    let (mut killed, mut assaulted, mut robbed) = (row.p_killed * m, row.p_assaulted * m, row.p_robbed * m);
    if world.config.assets.enabled {
        if let Some((harm, flash)) = crate::systems::chrome::stat_multipliers(world, id) {
            killed *= harm;
            assaulted *= harm;
            robbed *= flash;
        }
    }
    // M13 D46: in withdrawal the hour's theft is likelier (a branch).
    let mut steal = match world.stat_table.as_ref().and_then(|t| t.p_steal_broke) {
        // Parity (the law1 actor share): with the table's `p_steal_broke`
        // the hour's theft is the broke adult's alone (below the meal price,
        // as `calibrate` reads it): a Full agent with a meal's coins all but
        // never steals.
        Some(rates) if world.stat_mlp.is_none() => {
            let broke = crate::systems::demography::is_adult(world, id)
                && world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins)
                    < crate::systems::life::meal_price(world, id);
            match (broke, stat_row_index(world, id)) {
                (true, Some(r)) => rates[r],
                _ => 0.0,
            }
        }
        _ => row.p_steal,
    };
    if crate::systems::stims::in_withdrawal(world, id) {
        steal *= world.config.stims.withdrawal_steal_mult;
    }
    // The pressure pass phase 1 (OPEN-178): misery steals sooner.
    if steal > 0.0 && world.config.misery.is_some() {
        steal *= crate::systems::misery::stat_steal_mult(world, id);
    }
    (killed, assaulted, robbed, steal)
}

fn stat_rolls(world: &mut World, id: EntityId, row: &StatRow) {
    let rng = world.rng.agent(id);
    let u_steal: f32 = rng.random();
    let u_flirt: f32 = rng.random();
    let u_robbed: f32 = rng.random();
    let u_assaulted: f32 = rng.random();
    let u_killed: f32 = rng.random();
    let u_meet: f32 = rng.random();
    let u_chat: f32 = rng.random();
    if world.comp::<Brain>(id).is_none_or(|b| b.lod != Lod::Statistical) {
        return;
    }
    let (p_killed, p_assaulted, p_robbed, p_steal) = stat_probs(world, id, row);
    let adult = crate::systems::demography::is_adult(world, id);
    if !world.has::<Position>(id) {
        return;
    }
    // M16b phase 3 (spec § 5, the Statistical split of `p_killed`): of the
    // hour's killing, `wound_share × stat_saved` is a wound survived (an
    // assault), and a Trauma client's is cut by `trauma_stat_save`; the same
    // draw decides (with the wounded off, M16a's roll exactly).
    if adult && u_killed < p_killed {
        if let Some(eff) = crate::systems::wounded::stat_kill_p(world, id, p_killed) {
            if u_killed >= eff {
                world.stats.current.wounded.stat_wounds += 1;
                stat_hit(world, id, HoleKind::Assaulted, None);
                // The combat model C2: a killing survived leaves a Grave.
                crate::systems::combat::stat_wounds(world, id, true);
                return;
            }
        }
        stat_hit(world, id, HoleKind::Killed, None);
        return;
    }
    if u_assaulted < p_assaulted {
        stat_hit(world, id, HoleKind::Assaulted, None);
    }
    if u_robbed < p_robbed {
        stat_hit(world, id, HoleKind::Robbed, None);
    }
    // Meet before courting: Full agents court the people they meet.
    if u_meet < row.p_meet {
        stat_meet(world, id);
    }
    if u_chat < row.p_chat {
        stat_chat(world, id, row);
    }
    if u_flirt < row.p_flirt {
        stat_flirt(world, id);
    }
    if u_steal < p_steal {
        stat_theft(world, id);
    }
}

/// One off-screen hit on a Statistical agent (the hourly table's three hole
/// branches, plan L27): Killed (the death, `deaths_violence_offscreen`),
/// Assaulted (the beating's memories and needs) or Robbed (up to
/// `extort_amount` coins as the hole's loot), each with its event and a hole
/// for the binder. `src`: L2's faction violence (`fviolence::daily`): the
/// hole carries the source, the acting faction and the riot, is filed in
/// the source's district and is consequential. `None`: the table's hole,
/// as M10-M15 opened it. Abducted is `chrome::abduct_offscreen`'s.
pub fn stat_hit(world: &mut World, id: EntityId, kind: HoleKind, src: Option<&crate::ledger::ActiveSource>) -> bool {
    let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else { return false };
    let (zone, district) = match src {
        Some(s) => (world.district(s.district).zone, s.district),
        None => (world.map.zone(tile), world.district_of(tile)),
    };
    let place = world.district_name(district).to_string();
    let tick = world.tick;
    let consequential =
        src.is_some() || crate::systems::law::is_guard(world, id) || world.has::<crate::components::GangMember>(id);
    let (source, faction, riot) = src.map_or((None, None, None), |s| (Some(s.source), s.faction, s.riot));
    let base = |kind: HoleKind, event_id: u64, consequential: bool, loot: i64, w: &World| Hole {
        id: hole_id(tick, id, kind),
        kind,
        victim: id,
        zone,
        district,
        tick,
        event_id,
        consequential,
        spouse: w.spouse_of(id),
        loot,
        home: w.comp::<Household>(id).and_then(|h| h.home),
        gang: w.gang_of(id),
        source,
        faction,
        riot,
        prebound: None,
    };
    match kind {
        HoleKind::Killed => {
            let hole = base(HoleKind::Killed, 0, true, 0, world);
            let name = world.name_of(id);
            let ev = world.push_event(
                EventKind::Murder,
                &[EntityId::NONE, id],
                format!("{name} was killed in {place} (assailant unknown)"),
            );
            world.kill_by(id, DeathCause::Violence, None);
            world.stats.current.deaths_violence_offscreen += 1;
            bind::open_hole(world, Hole { event_id: ev, ..hole });
            true
        }
        HoleKind::Assaulted => {
            world.remember(id, MemoryKind::Fought, None, 0.7, -0.7, false);
            world.remember(id, MemoryKind::Lost, None, 0.6, -0.6, false);
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                n.safety = (n.safety - 0.6).max(0.0);
                n.energy = (n.energy - 0.2).max(0.0);
            }
            // As `law::raise_crime` drifts a Full victim.
            if let Some(p) = world.comp_mut::<Personality>(id) {
                p.drift(crate::personality::Drift::Robbed);
            }
            let name = world.name_of(id);
            let ev =
                world.push_event(EventKind::Assaulted, &[EntityId::NONE, id], format!("{name} was beaten in {place}"));
            let hole = base(HoleKind::Assaulted, ev, consequential, 0, world);
            bind::open_hole(world, hole);
            // The combat model C2 (docs/COMBAT.md § 2): the beating's wounds.
            crate::systems::combat::stat_wounds(world, id, false);
            true
        }
        HoleKind::Robbed => {
            let extort = world.config.social.extort_amount;
            let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
            let loot = extort.min(coins.max(0));
            if let Some(w) = world.comp_mut::<crate::components::Wallet>(id) {
                w.coins -= loot;
            }
            world.remember(id, MemoryKind::WasRobbed, None, 0.6, -0.6, false);
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                n.safety = (n.safety - 0.4).max(0.0);
            }
            if let Some(p) = world.comp_mut::<Personality>(id) {
                p.drift(crate::personality::Drift::Robbed);
            }
            let name = world.name_of(id);
            let ev = world.push_event(
                EventKind::Robbed,
                &[EntityId::NONE, id],
                format!("{name} was robbed of {loot} in {place}"),
            );
            let hole = base(HoleKind::Robbed, ev, consequential, loot, world);
            bind::open_hole(world, hole);
            true
        }
        HoleKind::Abducted => false,
    }
}
