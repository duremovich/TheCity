use super::*;

pub(super) fn stat_work(world: &mut World, id: EntityId, phase: DayPhase, no_net_row: Option<usize>) {
    let tick = world.tick;
    let job = world.comp::<Job>(id).cloned();
    // Only the jobless decide the dole (`economy::dole_eligible`; addendum 17).
    if job.is_none() {
        // The dole, paid directly, decided once a day in the Work phase as a
        // Full agent decides it (M10; drawing it daily regardless drained the
        // Treasury and left the wages unpaid): its Earn goal is satisfied
        // while it holds `SAVINGS_DAYS` meals of coins, and below that it
        // makes the walk to the Hall on the table's `p_dole_day` of days.
        // An agent who cannot buy today's meal always goes (M10 phase 5b): a
        // broke Full agent collects on ~97 % of days, and each day a broke
        // Statistical agent lost the lottery became a desperation theft.
        let day = world.day();
        // L1: the dole accrues from the last collection (`life::dole_days`),
        // so a day decided without a visit is marked apart from it.
        let undecided = world.comp::<Brain>(id).is_some_and(|b| b.stat_dole_day != Some(day));
        // The view milestone V2b (OPEN-265): the Market's price is read only
        // on the hour the dole is decided (every jobless hour read it).
        let saving = || {
            let price = world.local(id, BuildingKind::Market).map_or(1, |m| world.price_for(m, id));
            let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
            (coins >= crate::goap::world_state::SAVINGS_DAYS.saturating_mul(price), coins, price)
        };
        let decide = phase == DayPhase::Work && undecided && crate::systems::demography::is_adult(world, id);
        if let Some((false, coins, price)) = decide.then(saving) {
            let p = world.stat_table.as_ref().map_or(1.0, |t| t.p_dole_day);
            let u: f32 = world.rng.agent(id).random();
            if u < p || coins < price {
                economy::collect_dole(world, id);
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.stat_dole_day = Some(day);
            }
        }
    }
    // L2 (L23): a jobless Statistical member's GangWork day.
    if job.is_none() && phase == DayPhase::Work {
        crate::prof::timed!("work::stat_gang_day", crate::systems::fviolence::stat_gang_day(world, id));
    }
    if let (None, Some(i)) = (&job, no_net_row) {
        crate::prof::timed!("work::stat_earn", stat_earn(world, id, i));
    }
    let Some(job) = job else { return };
    // Hunger as sickness § 2: a sick day is no shift and no wage (as Full's);
    // the combat model C2: nor a manual post with a bad arm or leg.
    if crate::systems::health::sick(world, id) || crate::systems::combat::unfit(world, id, &job).is_some() {
        return;
    }
    let key = job.shift_key_at(tick);
    // L2 shadow fixes item 10: a guard's own (staggered) workday.
    if !job.on_shift(world.tick_of_day()) || !crate::exec::routine::workday_of(world, id, &job, key) {
        return;
    }
    // This shift is already done, or struck (M11 D35): no work, no wage.
    if job.last_shift_day == Some(key) {
        return;
    }
    // P5f (wage parity): the hour is worked on the role's `p_attend` (a
    // pure hash of the agent and the hour; the wage below reads the same).
    let attend = stat_attend(world, id, job.role);
    // Transit T13 (phase 2): the shift's first hour pays the day's commute
    // rides when the El wins the trip (nothing with transit off). Review
    // item 2: attendance is rolled first; a no-show (no hour of the shift
    // worked) does not ride.
    // Jobs & lives phase 2 (§ 3): a cab or a rickshaw first (the El's leg
    // is skipped for a rider; nothing with the street off).
    let cab = crate::systems::streetwork::on(world)
        && attend.is_none_or(|p| shift_attended(world, id, &job, p))
        && crate::systems::streetwork::stat_cab(world, id, &job);
    if !cab {
        crate::prof::timed!("work::stat_commute", crate::systems::transit::stat_commute(world, id, &job, attend));
    }
    // A dodger caught at the gate is on screen now (the arrest path).
    if world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical) {
        return;
    }
    let worked = attend.is_none_or(|p| attend_roll(world, id, tick / TICKS_PER_HOUR) < p);
    if worked && job.role == Role::Farmer {
        if let Some(farm) = job.employer {
            economy::accrue_farm_work(world, id, farm, u64::from(TICKS_PER_HOUR as u32));
        }
    }
    // L2 L9: a Statistical Fab Tech's hour on shift.
    if worked && job.role == Role::Fabber {
        if let Some(fab) = job.employer {
            crate::systems::jobs::accrue_fab_work(world, id, fab, u64::from(TICKS_PER_HOUR as u32));
        }
    }
    // Jobs and room J13: a Statistical producing trade's hour on shift.
    if worked && job.role.is_trade() {
        if let Some(b) = job.employer {
            crate::systems::trades::accrue(world, id, b, u64::from(TICKS_PER_HOUR as u32));
        }
    }
    // Lives at work phase 3 (§ 6, the super's door rent): a Statistical
    // round worker credited for the shift walks its round world-side at the
    // shift's first hour: the same stop list, the same encounters.
    // Lives at work phase 4 (§ 2.3): the courier's parcels the same way.
    if (crate::systems::service::civic_on(world) || crate::systems::service::street_on(world))
        && crate::systems::trades::on_round(world, id)
        && attend.is_none_or(|p| shift_attended(world, id, &job, p))
    {
        let tod = world.tick_of_day();
        let first = job.first_hour(tod);
        if first && crate::systems::streetwork::haul_courier(world, id) {
            // The pre-view pass phase 3 (OPEN-141, the mirror): a haul
            // courier's own addresses, each handed over on the same presence
            // rule a body's unload reads (`courier_stop`).
            for b in crate::systems::streetwork::courier_addresses(world, id) {
                crate::systems::streetwork::courier_stop(world, id, b);
            }
        } else if first {
            let round = crate::systems::trades::round_for(world, id, key);
            let stops = round.len();
            for b in round {
                crate::systems::service::super_stop(world, id, b);
                crate::systems::streetwork::courier_stop(world, id, b);
            }
            crate::systems::service::notices_folded(world, id, Some(stops));
        }
    }
    // An hour of co-work drifts each pair of co-workers as `social`'s
    // co-location does for bodies (bodies never see a Statistical agent in
    // their building). Each Statistical pair once an hour: the lower id drives.
    // Scale S1: the employer's staff index, not every holder of the role,
    // and only the agent's crew (`[lod] coworker_crew`).
    if let Some(employer) = job.employer {
        let _timer = crate::prof::scope!("work::coworkers");
        let tod = world.tick_of_day();
        let crew = world.config.lod.coworker_crew;
        let mates: Vec<EntityId> = {
            let _mates = crate::prof::scope!("work::mates");
            crew_above(world.staff_of(employer), id, crew, |c| world.comp::<Job>(c).is_some_and(|j| j.role == job.role))
                .into_iter()
                .filter(|&c| {
                    world.comp::<Job>(c).is_some_and(|j| j.on_shift(tod))
                        && world.comp::<Brain>(c).is_some_and(|b| b.lod == Lod::Statistical)
                        && !world.has::<Sentence>(c)
                })
                .collect()
        };
        world.coworker_pairs += mates.len() as u64;
        // The perf pass phase 9 (OPEN-184): the bond in two steps, reads then
        // writes. Every pair's step is read first (the two temperaments) and
        // its edge looked up, each lookup independent of the others, so the
        // edge map's cache misses overlap instead of queueing one per write;
        // then the drifts are written in mate order. The pairs are distinct
        // and a drift writes only its own edge, the neighbour index and the
        // enemies index, none of which a step reads, so each write is
        // `social::interacted`'s for that mate exactly.
        let _drift = crate::prof::scope!("work::interacted");
        let mut looked = 0u32;
        let steps: Vec<(EntityId, f32)> = mates
            .iter()
            .map(|&c| {
                looked ^= world.edge(id, c).map_or(0, |e| e.affinity.to_bits());
                (c, crate::systems::social::interaction_step(world, id, c))
            })
            .collect();
        // The lookups are made for their cache lines alone.
        std::hint::black_box(looked);
        for (c, step) in steps {
            crate::systems::social::interacted_at(world, id, c, step);
        }
    }
    let shift_ends_soon = job.shift_end(tick).is_some_and(|end| end <= tick + TICKS_PER_HOUR);
    if shift_ends_soon && job.last_shift_day != Some(key) {
        // Lives at work phase 1 (§ 2.1): the shift-end rule as a body's `end_shift`.
        crate::systems::service::shift_end(world, id);
        // Lives at work phase 6 (§ 4.3): the profession's day (as a body's).
        crate::systems::careers::shift_end(world, id);
        if let Some(j) = world.comp_mut::<Job>(id) {
            j.last_shift_day = Some(key);
            j.days_unpaid = j.days_unpaid.saturating_add(1);
        }
        // P5f: pro rata on the shift's hours credited (the whole wage
        // without a `p_attend`).
        match attend {
            Some(p) => {
                let ratio = world.stat_table.as_ref().and_then(|t| t.p_wage_ratio).unwrap_or(1.0);
                let share = (stat_shift_share(world, id, &job, tick, p) * ratio).min(1.0);
                economy::collect_wage_scaled(world, id, share);
            }
            None => {
                economy::collect_wage(world, id);
            }
        }
        // A Full farmer hauls to the Market at shift end; so does this one.
        if job.role == Role::Farmer {
            if let Some(farm) = job.employer {
                let enough =
                    world.comp::<Building>(farm).is_some_and(|b| b.stock_food >= world.config.economy.haul_min_stock);
                if enough {
                    // M13 D23 (deviation): the Farm's fleet truck carries a
                    // Statistical farmer's haul too.
                    let mult = crate::systems::vehicles::stat_haul_mult(world, farm);
                    economy::haul(world, farm, mult);
                }
            }
        }
    }
}

/// P5f: the role's learned attendance (`StatTable::p_attend`), if any.
pub fn stat_attend(world: &World, id: EntityId, role: Role) -> Option<f32> {
    let t = world.stat_table.as_ref()?.p_attend.as_ref()?;
    // Transit T14 (plan D9): the role's row in the agent's commute band
    // (`"{role:?}/b{band}"`) when the table learned it; the role's row
    // otherwise (a table without banded rows keeps P5f's rule). The band is
    // measured only for a role with banded rows.
    if t.banded(role) {
        if let Some(p) = crate::systems::transit::commute_band(world, id).and_then(|b| t.get(role, Some(b))) {
            return Some(p);
        }
    }
    match t.get(role, None) {
        Some(p) => Some(p),
        // Class pay (docs/CLASS_PAY.md): a role the table has not learned
        // (the new graded trades, a Ripperdoc) works the table's mean
        // attendance (over the roles' own rows, not the banded ones) rather
        // than a whole paid shift, until `calibrate` learns it (a graded
        // wage would otherwise widen the tiers' gap).
        None if crate::systems::classpay::on(world) => t.mean(),
        None => None,
    }
}

/// P5f: the hour's attendance roll (a pure hash of the agent and the hour).
fn attend_roll(world: &World, id: EntityId, hour: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), 0xa77e_0001, u64::from(id.index), hour)
}

/// Review item 2: does `id` work any hour of the shift it is on, from this
/// hour (its slot's minute) to the shift's end, on attendance `p` (the
/// rolls `stat_work` makes, one per hour at the same minute)? A shift with
/// none is a no-show. `calibrate`'s commute ride share counts the Full
/// commutes that arrived at the work, so a commute is a shift shown up
/// for, whatever hour it began.
pub fn shift_attended(world: &World, id: EntityId, job: &Job, p: f32) -> bool {
    let tick = world.tick;
    let Some(end) = job.shift_end(tick) else { return true };
    let minute = tick % TICKS_PER_HOUR;
    (tick / TICKS_PER_HOUR..).take_while(|&h| h * TICKS_PER_HOUR + minute < end).any(|h| attend_roll(world, id, h) < p)
}

/// Scale S1: `id`'s crew above it, ascending: of `staff` (ascending ids)
/// the members `same` keeps (its role), half of `crew` (rounded up) nearest `id` on each
/// side in that order, wrapping (`crew` 0: every one), then those above `id`
/// (the lower id drives a pair). Crew membership is symmetric, so each pair
/// is driven once; a group of at most `crew + 1` is every member.
pub fn crew_above(staff: &[EntityId], id: EntityId, crew: usize, same: impl Fn(EntityId) -> bool) -> Vec<EntityId> {
    let above = staff.partition_point(|&c| c <= id);
    if crew == 0 {
        return staff[above..].iter().copied().filter(|&c| same(c)).collect();
    }
    let n = staff.len();
    let half = crew.div_ceil(2);
    let mut out: Vec<EntityId> = Vec::with_capacity(2 * half);
    // Forward from `id` (wrapping), then backward, `half` members each.
    let mut found = 0;
    for k in 0..n {
        let c = staff[(above + k) % n];
        if found == half || c == id {
            break;
        }
        if same(c) {
            out.push(c);
            found += 1;
        }
    }
    let below = staff.partition_point(|&c| c < id);
    found = 0;
    for k in 1..=n {
        let c = staff[(below + n - k) % n];
        if found == half || c == id {
            break;
        }
        if same(c) {
            out.push(c);
            found += 1;
        }
    }
    out.retain(|&c| c > id);
    out.sort_unstable();
    out.dedup();
    out
}

/// P5f: the share of the shift ending around `tick` whose hours the agent
/// worked (the same rolls `stat_work` made).
fn stat_shift_share(world: &World, id: EntityId, job: &Job, tick: Tick, p: f32) -> f32 {
    let Some(end) = job.shift_end(tick) else { return 1.0 };
    let tod = (tick % crate::time::TICKS_PER_DAY) as u16;
    let len = job.shifts.iter().find(|&&(s, e)| tod >= s && tod < e).map_or(0, |&(s, e)| Tick::from(e - s));
    let len = if job.shifts.iter().any(|&(_, e)| e == 1440) { len.max(end.saturating_sub(tick)) } else { len };
    let start = end.saturating_sub(len.max(TICKS_PER_HOUR));
    let (h0, h1) = (start / TICKS_PER_HOUR, end.div_ceil(TICKS_PER_HOUR));
    let n = h1.saturating_sub(h0).max(1);
    let worked = (h0..h1).filter(|&h| attend_roll(world, id, h) < p).count() as f32;
    worked / n as f32
}

/// The table row index of `id`'s hour (`calibrate`'s buckets: the phase,
/// lawfulness and hunger now), `None` without a table.
pub(super) fn stat_row_index(world: &World, id: EntityId) -> Option<usize> {
    let t = world.stat_table.as_ref()?;
    let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    let hunger = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
    Some(t.index(world.phase(), law, hunger))
}

/// Flip readiness (no safety net): the Full tier's broke jobless hour off
/// screen. Under the market wage law (P10) and a table holding `p_scavenge`/`p_beg`, a jobless
/// adult who is broke (`life::broke`, the Full Scavenge's and Beg's gate)
/// scavenges on its row's `p_scavenge` of hours (the same find, paid
/// from the Recycler's till: `jobs::scavenge_find`) and begs on its row's
/// `p_beg` (`stat_beg`), one draw on the agent's stream.
fn stat_earn(world: &mut World, id: EntityId, row: usize) {
    // Shadow V4 bug 1: the near-broke (short, not broke) on their own rates.
    let broke = crate::systems::life::broke(world, id);
    let Some((p_scav, p_beg)) = world.stat_table.as_ref().map(|t| {
        if broke {
            (t.p_scavenge.map_or(0.0, |p| p[row]), t.p_beg.map_or(0.0, |p| p[row]))
        } else {
            (t.p_scavenge_short.map_or(0.0, |p| p[row]), t.p_beg_short.map_or(0.0, |p| p[row]))
        }
    }) else {
        return;
    };
    if p_scav + p_beg <= 0.0 || !crate::systems::demography::is_adult(world, id) {
        return;
    }
    // Parity (the bin meals red): the Full Scavenge's and Beg's own gate,
    // `life::broke` (the planner's `ctx.poor`: below a meal, and homeless a
    // Hotel night too), not the Earn goal's savings line. A Full jobless
    // adult with a meal's coins all but never scavenges or begs (seed 2000,
    // all-Full: 0.196 scavenges an hour at 0 coins, 0.012 at 1, under 0.01
    // above); the savings-line gate spread the broke's rate over everyone
    // under seven meals, so the broke off screen stayed broke (half the
    // off-screen homeless broke against a tenth Full). Shadow V4 bug 1:
    // with no dole the Full jobless under `earn_meals_no_net` meals earn on
    // (`life::earn_short`), at the near-broke's own learned rates (one rate
    // over the wider gate is the same spread).
    if !broke && !crate::systems::life::earn_short_if(world, id, false) {
        return;
    }
    let u: f32 = world.rng.agent(id).random();
    if u < p_scav {
        crate::systems::jobs::scavenge_find(world, id);
    } else if u < p_scav + p_beg {
        stat_beg(world, id);
    }
}

/// An off-screen Beg (the no-net income path): up to four Statistical
/// adults drawn in the agent's zone (from up to `STRANGER_TRIES` uniform
/// draws, as `stranger_in_zone`) are the passers a Full beggar asks
/// (`exec::actions` Beg takes four within three tiles), each as a Full
/// passer gives: not an enemy (affinity > 0, or a stranger), more than 10
/// coins, on `0.3 + 0.4 × sociability` (half for a stranger), one or two
/// coins, the first who gives ends it. A known edge carries the debt as
/// Full's; a stranger's gift opens no edge (the off-screen street keeps no
/// ledger of passers).
fn stat_beg(world: &mut World, id: EntityId) {
    const STRANGER_TRIES: usize = 16;
    const PASSERS: usize = 4;
    let mut asked = 0;
    let zone_of = |w: &World, o: EntityId| w.comp::<Position>(o).map(|p| w.map.zone(p.tile));
    let Some(zone) = zone_of(world, id) else { return };
    let n = world.tier(Lod::Statistical).len();
    if n == 0 {
        return;
    }
    for _ in 0..STRANGER_TRIES {
        let k = world.rng.agent(id).random_range(0..n);
        let o = world.tier(Lod::Statistical)[k];
        if o == id
            || zone_of(world, o) != Some(zone)
            || world.has::<Sentence>(o)
            || !crate::systems::demography::is_adult(world, o)
        {
            continue;
        }
        if asked == PASSERS {
            return;
        }
        asked += 1;
        let edge = world.edge(id, o).map(|e| e.affinity);
        let generous =
            edge.is_none_or(|a| a > 0.0) && world.comp::<crate::components::Wallet>(o).is_some_and(|w| w.coins > 10);
        if !generous {
            continue;
        }
        let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
        let odds = (0.3 + sociability * 0.4) * if edge.is_none() { 0.5 } else { 1.0 };
        let rng = world.rng.agent(id);
        let (roll, bonus): (f32, f32) = (rng.random(), rng.random());
        if roll >= odds {
            continue;
        }
        let coins = 1 + i64::from(bonus < 0.5);
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(o) {
            w.coins -= coins;
        }
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(id) {
            w.coins += coins;
        }
        if edge.is_some() {
            // The lower id owes the higher id, as Full's Beg books it.
            let lo = crate::components::edge_key(id, o).0;
            let tick = world.tick;
            let e = world.edge_entry(id, o);
            e.debt += if lo == id { coins as i32 } else { -(coins as i32) };
            e.debt_since.get_or_insert(tick);
            e.last_interaction = tick;
        }
        return;
    }
}

/// An off-screen Social hour: belonging. The Chat it used to carry is the
/// `p_chat` roll.
pub(super) fn stat_social(world: &mut World, id: EntityId) {
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        n.belonging = (n.belonging + 0.1).min(1.0);
    }
}

/// A Chat off screen (the `p_chat` roll): on the row's `p_chat_home`
/// (always drawn) with the housemate a Full agent at home would pick (the
/// highest affinity, ties to the lower id, as `best_colocated_partner`),
/// else with a random known edge, else (nobody known yet) a Statistical
/// housemate met.
pub(super) fn stat_chat(world: &mut World, id: EntityId, row: &StatRow) {
    let u_home: f32 = world.rng.agent(id).random();
    if u_home < row.p_chat_home {
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let best = home
            .map(|h| world.residents_of(h))
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|&o| o != id && world.has::<Brain>(o) && !world.has::<Sentence>(o))
            .map(|o| (o, world.edge(id, o).map_or(0.0, |e| e.affinity)))
            .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
            .map(|(o, _)| o);
        if let Some(mate) = best {
            crate::systems::social::interacted(world, id, mate);
            return;
        }
    }
    let neighbours: Vec<EntityId> = world.neighbours(id).collect();
    if neighbours.is_empty() {
        // Nobody known yet: meet a Statistical housemate.
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let mates: Vec<EntityId> = home
            .map(|h| world.residents_of(h))
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|&o| o != id && world.comp::<Brain>(o).is_some_and(|b| b.lod == Lod::Statistical))
            // L2 (L21): a held housemate is in the cells.
            .filter(|&o| !world.has::<Sentence>(o))
            .collect();
        if !mates.is_empty() {
            let k = world.rng.agent(id).random_range(0..mates.len());
            let tick = world.tick;
            let e = world.edge_entry(id, mates[k]);
            e.last_interaction = tick;
        }
        return;
    }
    // A Chat's drift (`exec/actions.rs` Chat): affinity by similarity, trust +0.02.
    let k = world.rng.agent(id).random_range(0..neighbours.len());
    crate::systems::social::interacted(world, id, neighbours[k]);
}
