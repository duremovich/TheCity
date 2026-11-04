use super::*;

/// Island fix round F3 (OPEN-190; I1's stay-in, Dylan's D1 B): with
/// `[distance.free.stay_in]` and the household on, a standing Home (the
/// caller has found no spot within `spot_cap_ticks`): the Home and its
/// outside door: the person stays in, and the
/// evening runs inside it (its flatmates and Block-mates, else alone).
pub(super) fn stay_in_home(world: &World, id: EntityId) -> Option<(EntityId, TilePos)> {
    crate::systems::distance::stay_in(world)?;
    if !crate::systems::household::on(world) {
        return None;
    }
    let h = home_of(world, id)?;
    world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (h, world.outside_door(b)))
}

pub(super) fn home_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Household>(id).and_then(|h| h.home)
}

// ---------------------------------------------------------------------------
// Unwind: considerations and plans (plan L14, L16)
// ---------------------------------------------------------------------------

/// `[life] spot_enemy_penalty` (L2 shadow fixes item 5).
pub(super) fn cfg_life_penalty(world: &World) -> f32 {
    world.config.life.spot_enemy_penalty
}

fn adult_free(world: &World, id: EntityId) -> bool {
    crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// Spec § 2: Unwind is satisfied at `fun_satisfied` (and off for a child,
/// a prisoner, or with leisure off).
pub fn unwind_satisfied(world: &World, id: EntityId) -> bool {
    !adult_free(world, id)
        || world.comp::<Needs>(id).is_none_or(|n| n.fun >= world.config.needs.fun_satisfied)
            // The pressure pass phase 2: the lonely with coins look for company.
            && !crate::systems::customers::lonely_wants(world, id)
}

/// Day gaps 1 (2026-10-11): `job`'s shift is being worked now: on shift on
/// a working day (the agent's own: a guard's staggered rest day) that no
/// strike struck. A rest day's shift hours are free.
pub fn working_now(world: &World, id: EntityId, job: &Job) -> bool {
    let key = job.shift_key_at(world.tick);
    job.on_shift(world.tick_of_day())
        && crate::exec::routine::workday_of(world, id, job, key)
        && job.struck_shift != Some(key)
}

/// The agent's off hours. Day gaps 1 (2026-10-11): a job holder's are its
/// own: not working a shift now, and the next workday shift not due (a
/// rest day, a shift already worked or struck) or further off than the
/// walk to work plus `[leisure] shift_lead_ticks`. The wall clock's
/// Evening and Night left night crews, the venues' evening staff, night
/// guards and rest days at Unwind x 0.3 < Idle, at home for 6-18 h. The
/// jobless and execs keep the Evening and Night (an exec once the office
/// has shut).
pub fn off_hours(world: &World, id: EntityId) -> bool {
    if let Some(j) = world.comp::<Job>(id) {
        if working_now(world, id, j) {
            return false;
        }
        let key = j.next_shift_key(world.tick);
        if !crate::exec::routine::workday_of(world, id, j, key)
            || j.last_shift_day == Some(key)
            || j.struck_shift == Some(key)
        {
            return true;
        }
        let Some(door) = j.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door) else {
            let until = u64::from(j.ticks_until_shift(world.tick_of_day()));
            return until > world.config.leisure.shift_lead_ticks;
        };
        let until = u64::from(j.ticks_until_shift(world.tick_of_day()));
        // Transit T16: the trip as it would be made to arrive by the lead
        // (`transit::trip_due`, the departure gate's).
        return !crate::systems::transit::trip_due(world, id, door, until, world.config.leisure.shift_lead_ticks);
    }
    if !matches!(world.phase(), DayPhase::Evening | DayPhase::Night) {
        return false;
    }
    crate::systems::life::exec_corp(world, id).is_none() || crate::systems::life::exec_shift_over(world)
}

/// Plan L14 (spec § 2): `U(fun)` Logistic{8, 0.5}; off hours ([`off_hours`]:
/// the agent's own), else `gate_or(0.3)`; the best rung's score (gain × the walk ×
/// pride on the mid and high rungs × courage at the FightPit and Den ×
/// greed-weighted affordability). `None`: no rung reachable.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if unwind_satisfied(world, id) {
        return None;
    }
    // Shadow V4 bug 2: no leisure in a day worker's small hours (woken at
    // 23:30, it stood on a corner at 03:00; a soft 0.3 gate still beat Idle
    // with nothing else awake: the re-shadow read 606 such HangOuts against
    // 638): the night is Idle's, and Idle at Home at night is bed.
    if crate::exec::routine::day_worker_night(world, id) {
        return None;
    }
    let n = world.comp::<Needs>(id)?;
    // Hunger fix item 5 (P5f): a worker on shift stays at work (day gaps
    // 1: a shift being worked; a rest day's shift hours are free).
    if crate::systems::fixes::shift_first(world) && world.comp::<Job>(id).is_some_and(|j| working_now(world, id, j)) {
        return None;
    }
    let pick = choice(world, id, false)?;
    // The pressure pass phase 2 (OPEN-176): a lonely body whose pick is a
    // companion's, host's or dancer's venue unwinds for the company.
    let fun_u = Curve::Logistic { k: 8.0, mid: 0.5 }.eval(urgency(n.fun));
    let need = fun_u.max(crate::systems::customers::lonely_need(world, id, pick.venue));
    let cs = vec![
        Consideration::raw("U(fun) or lonely", urgency(n.fun), need),
        Consideration::new("off hours", can(off_hours(world, id)), gate_or(0.3)),
        Consideration::raw("rung", pick.score, (0.5 + pick.score).min(1.0)),
        // Deviation (measured): food before fun. Without it a hungry body
        // took the evening at a spot or a venue over its meal (fun decay off:
        // seed 42/43 starvation 22/16; on: 23-45; e8355d4 12/25).
        Consideration::new("fed", n.hunger, Curve::Logistic { k: 12.0, mid: 0.4 }),
    ];
    // L2 shadow fixes item 3: the tired go to bed, not to a corner.
    let mut cs = cs;
    if let Some(c) = crate::utility::goals::energy_brake(world, n) {
        cs.push(c);
    }
    Some((cs, 0.0))
}

pub(super) fn step(action: ActionKind, target: Option<EntityId>) -> ActionInstance {
    ActionInstance { action, target, tile: None }
}

/// The steps of a pick: the walk, then the satisfier (a Den's drink first,
/// a FightPit's bet in the bet window).
fn steps_for(world: &World, id: EntityId, pick: &UnwindPick) -> Vec<ActionInstance> {
    let mut steps = Vec::new();
    match (pick.venue, pick.spot) {
        // Lives at work phase 2 (§ 3.1): a HangOut at a Home (one's own, or
        // a friend's) walks to the Home and runs inside it.
        (Some(h), Some(_)) if pick.act == ActionKind::HangOut => {
            if Some(h) == home_of(world, id) {
                steps.push(step(ActionKind::GoTo(LocationKey::Home), None));
            } else {
                steps.push(step(ActionKind::GoTo(LocationKey::TargetHome), Some(h)));
            }
            steps.push(step(ActionKind::HangOut, None));
        }
        (Some(v), _) => {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(v)));
            let kind = kind_of(world, v);
            let purse = spendable(world, id) - price_of(world, v);
            let drink = kind == Some(BuildingKind::Den) && purse >= 3 && !crate::utility::goals::drank_today(world, id);
            // Round 6 V4 (OPEN-239): at a door a bouncer keeps, the drink
            // comes after the bet: the door's check runs at the venue step's
            // start (`exec::start_step`) and turns the drunk away, so the
            // plan's own Drink made its Gamble refused, nightly
            // (`office_worker_56309 d23 21:23`, `PreconditionLost`).
            let after = drink
                && kind.is_some_and(|k| {
                    crate::systems::customers::cfg(world).is_some() && crate::systems::customers::kept_kind(world, k)
                });
            if drink && !after {
                steps.push(step(ActionKind::Drink, Some(v)));
            }
            steps.push(step(pick.act, Some(v)));
            if after {
                steps.push(step(ActionKind::Drink, Some(v)));
            }
            // R6a (OPEN-217): the window read at the body's arrival, not at
            // the plan (a pit planned at 20:30 with a walk into the window
            // took no bet).
            let window = match world.config.trade_verbs.as_ref() {
                Some(_) => {
                    let walk = world
                        .comp::<Position>(id)
                        .zip(world.comp::<Building>(v))
                        .map_or(0, |(p, bd)| crate::exec::timed_walk(world, id, p.tile, bd.door));
                    bet_window_at(world, u64::from(world.tick_of_day()) + walk)
                }
                None => bet_window(world),
            };
            if kind == Some(BuildingKind::FightPit) && window && purse >= 1 {
                let brave = world
                    .comp::<Personality>(id)
                    .is_some_and(|p| p.courage >= world.config.leisure.gamble_rung_courage);
                if brave {
                    steps.push(step(ActionKind::Gamble, Some(v)));
                }
            }
        }
        (None, Some(_)) => {
            steps.push(step(ActionKind::GoTo(LocationKey::Spot), None));
            steps.push(step(pick.act, None));
        }
        (None, None) => {}
    }
    steps
}

/// Plan L14: the Unwind plan from the pick made now (deviation: the pick is
/// computed again at plan time with the spots scored, and cached in
/// `World::unwind` for the executor, rather than carried from the think).
pub fn unwind_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if unwind_satisfied(world, id) {
        return None;
    }
    let pick = choice(world, id, true)?;
    let steps = steps_for(world, id, &pick);
    if steps.is_empty() {
        return None;
    }
    // Round 6 V2: a contested corner passed over tonight is told.
    if let Some(from) = origin(world, id) {
        crate::systems::turf::note_avoided(world, id, from);
    }
    let target = pick.venue;
    // Lives at work phase 2 (§ 3.1): a Home has no seats to hold.
    let at_home = pick.act == ActionKind::HangOut;
    world.unwind.insert(id, pick);
    // Plan L30: with the LOD budget on, a Seat at the venue is held from the
    // plan to the arrival (`unwind_plan` installs without `reserve_for`).
    if let Some(v) = target.filter(|_| !at_home) {
        let expires = world.tick + world.config.exec.reservation_ttl;
        world.reserve(id, crate::exec::ReservationKind::Seat { building: v }, expires);
    }
    Some(Plan { goal: GoalKind::Unwind, target, steps, started_tick: world.tick })
}

/// Plan L16: a Socialise with nobody to talk to here goes to a HangOut
/// when broke (< 2 coins) or when the best spot outscores the local Bar's
/// walk-weighted 1.0. `None`: the planner as before.
pub fn hangout_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if crate::systems::social::has_colocated_partner(world, id) {
        return None;
    }
    let from = origin(world, id)?;
    let (spot, social) = best_spot_kind(world, id, from)?;
    let tile = spot.tile;
    let spot_score = (0.5 + social) * spot_travel(world, id, from, tile);
    let bar = world.local(id, BuildingKind::Bar).and_then(|b| crate::systems::life::tiles_to(world, id, b));
    let bar_score = bar.map_or(0.0, |t| travel(world, t));
    if coins(world, id) >= 2 && spot_score < bar_score {
        return None;
    }
    // Lives at work phase 2 (§ 3.1): a Home spot's HangOut runs inside the
    // Home (the pick's `venue`); the walk is to the Home itself.
    let home = match spot.kind {
        SpotKind::Home(h) => Some(h),
        _ => None,
    };
    let pick = UnwindPick {
        rung: Rung::Free,
        act: ActionKind::HangOut,
        venue: home,
        spot: Some(tile),
        score: spot_score,
        stay_in: false,
    };
    // Round 6 V2: a contested corner passed over tonight is told.
    crate::systems::turf::note_avoided(world, id, from);
    let steps = match home {
        Some(h) if Some(h) == home_of(world, id) => {
            vec![step(ActionKind::GoTo(LocationKey::Home), None), step(ActionKind::HangOut, None)]
        }
        Some(h) => vec![step(ActionKind::GoTo(LocationKey::TargetHome), Some(h)), step(ActionKind::HangOut, None)],
        None => vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::HangOut, None)],
    };
    world.unwind.insert(id, pick);
    Some(Plan { goal: GoalKind::Socialise, target: None, steps, started_tick: world.tick })
}

/// Plan L16: `already_satisfied(Socialise)`'s "no drink possible" clause is
/// dropped when a spot is within `hangout_reach`.
pub fn spot_in_reach(world: &World, id: EntityId) -> bool {
    origin(world, id).is_some_and(|from| {
        // The island pass phase 2: on the travel estimate, within the spot cap.
        if crate::systems::distance::free(world).is_some() {
            let cap = spot_cap(world);
            return spots_near(world, id, from).iter().any(|s| spot_dist(world, id, from, s.tile) <= cap);
        }
        spots_near(world, id, from).iter().any(|s| s.tile.manhattan(from) <= world.config.leisure.hangout_reach)
    })
}

/// The island pass phase 2 (`[distance.free]`): the Bar a Socialise would
/// walk to (`World::local`) is past `spot_cap_ticks` from where the body
/// sets out, so no drink is possible for it (the base walked to the nearest
/// Bar at any distance once no spot was left: 1,809 Socialise Bar legs on
/// the island at d30, 43 % at 2 h or more). `false` off.
pub fn bar_past_cap(world: &World, id: EntityId) -> bool {
    let Some(f) = crate::systems::distance::free(world) else { return false };
    let Some(from) = origin(world, id) else { return false };
    world
        .local(id, BuildingKind::Bar)
        .and_then(|b| world.comp::<Building>(b))
        .is_some_and(|bd| crate::systems::distance::trip(world, id, from, bd.door) > f.spot_cap_ticks)
}

/// `LocationKey::Spot`: the spot of the agent's pick.
pub fn spot_of(world: &World, id: EntityId) -> Option<TilePos> {
    // Jobs & lives phase 2: the Pitch goal's street tile (its stall, or the
    // labour corner at dawn).
    if world.comp::<crate::components::Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Pitch) {
        return crate::systems::streetwork::street_tile(world, id);
    }
    // Lives at work phase 4: a cab driver's leg (the rider's door, the work door).
    // Review fix 2: only in the Work plan (another goal's plan reads its own spot).
    if crate::systems::streetwork::driving(world, id) {
        if let Some(t) = crate::systems::streetwork::fare_tile(world, id) {
            return Some(t);
        }
    }
    // Round 3 S5 (OPEN-217): a cab driver standing by drives to his rank.
    if let Some(t) = crate::systems::streetwork::rank_spot(world, id) {
        return Some(t);
    }
    world.unwind.get(&id).and_then(|p| p.spot)
}

/// Plan L14 (spec § 2): `Earn` reads `U(fun)` as a second term on Beg and
/// Scavenge when fun is below 0.3 and the purse below the cheap rung (the
/// two actions' plan cost, `PlanCtx::fun_term`; review fix: not the goal).
pub fn earn_fun_term(world: &World, id: EntityId) -> Option<f32> {
    let fun = world.comp::<Needs>(id)?.fun;
    (fun < 0.3 && coins(world, id) < 2).then(|| urgency(fun))
}

// ---------------------------------------------------------------------------
// Execution (plan L14, L15, L18)
// ---------------------------------------------------------------------------

/// Plan L14: a leisure step may start (money is re-checked here, so a
/// refusal moves none).
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    let inside = here(world, id);
    let at_venue = |b: EntityId| inside == Some(b) && open(world, b) && door_ok(world, id, b);
    match kind {
        ActionKind::Enjoy => {
            let Some(b) = target else { return false };
            at_venue(b) && price_of(world, b) > 0 && coins(world, id) >= price_of(world, b)
        }
        ActionKind::EatOut => {
            let Some(b) = target else { return false };
            at_venue(b)
                && kind_of(world, b) == Some(BuildingKind::NoodleBar)
                && world.comp::<Building>(b).is_some_and(|bd| bd.stock_food > 0)
                && coins(world, id) >= price_of(world, b).max(1)
        }
        ActionKind::Gamble => {
            let Some(b) = target else { return false };
            let ok_kind = match kind_of(world, b) {
                Some(BuildingKind::Den) => table_open(world, b),
                Some(BuildingKind::FightPit) => bet_window(world),
                _ => false,
            };
            at_venue(b) && ok_kind && coins(world, id) >= 1
        }
        // Lives at work phase 2 (§ 3.1): a HangOut runs inside the Home it was picked for.
        ActionKind::HangOut => {
            world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
                || crate::systems::household::hanging_at_home(world, id).is_some()
        }
        ActionKind::Preach => world.comp::<Position>(id).is_some_and(|p| p.building.is_none()),
        ActionKind::Collect => {
            // Lives at work phase 4b (§ 2.3): a pimp's collection at a pitch.
            if let Some(ok) = crate::systems::vice::collect_can_start(world, id, target) {
                return ok;
            }
            let Some(gang) = world.gang_of(id) else { return false };
            world.comp::<Gang>(gang).is_some_and(|g| g.leader == Some(id)) && inside.is_some() && inside == target
        }
        _ => false,
    }
}

/// Plan L14, at start (money moves now): the entry or the meal (refunded on
/// abort through `World::pending_purchase`), the table game or the bet, the
/// HangOut registry.
pub fn on_start(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>) {
    match kind {
        ActionKind::Enjoy | ActionKind::EatOut => {
            let Some(b) = target else { return };
            let price = price_of(world, b);
            let paid = pay_venue(world, id, b, price, Flow::Leisure);
            let mut units = 0;
            if kind == ActionKind::EatOut && world.take_stock(b, crate::components::Good::Food, 1) == 1 {
                units = 1;
            }
            world.pending_purchase.insert(id, (units, paid));
        }
        ActionKind::Gamble => {
            let Some(b) = target else { return };
            let stake = stake_of(world, spendable(world, id).max(1).min(coins(world, id)));
            if kind_of(world, b) == Some(BuildingKind::FightPit) {
                let paid = pay_venue(world, id, b, stake, Flow::Gamble);
                if paid > 0 {
                    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
                        v.bets.push((id, paid, false));
                    }
                }
            } else {
                let now = world.tick;
                let u: f32 = world.rng.word(WordNs::Gamble, now, u64::from(id.index)).random();
                play_table(world, id, b, stake, u);
            }
        }
        ActionKind::HangOut | ActionKind::Preach => {
            let Some(t) = world.comp::<Position>(id).map(|p| p.tile) else { return };
            let list = world.hangouts.entry(t).or_default();
            if !list.contains(&id) {
                list.push(id);
            }
        }
        _ => {}
    }
}

/// Leave the HangOut registry (a step's end, an abort; review fix: also a
/// death and an emigration, which drop the Brain without `abort_plan`).
pub fn leave_spot(world: &mut World, id: EntityId) {
    if world.hangouts.is_empty() {
        return;
    }
    let t = world.comp::<Position>(id).map(|p| p.tile);
    let mut emptied = None;
    if let Some(list) = t.and_then(|t| world.hangouts.get_mut(&t)) {
        list.retain(|x| *x != id);
        if list.is_empty() {
            emptied = t;
        }
    } else {
        for list in world.hangouts.values_mut() {
            list.retain(|x| *x != id);
        }
        world.hangouts.retain(|_, l| !l.is_empty());
    }
    if let Some(t) = emptied {
        world.hangouts.remove(&t);
    }
}

/// Plan L14: an abandoned Enjoy or meal is refunded (`ownership::refund`),
/// the meal's unit back in stock; a HangOut leaves the registry.
pub fn on_abort(world: &mut World, id: EntityId, kind: ActionKind) {
    match kind {
        ActionKind::Enjoy | ActionKind::EatOut => {
            let Some((units, paid)) = world.pending_purchase.remove(&id) else { return };
            let b = world
                .comp::<Brain>(id)
                .and_then(|b| b.current_step())
                .and_then(|s| s.target)
                .or_else(|| here(world, id));
            let Some(b) = b else { return };
            let owner = world.owner_of(b);
            ownership::refund(world, id, owner, paid, Flow::Leisure);
            ownership::credit(world, b, -paid);
            book_take(world, b, -paid, false);
            if paid > 0 {
                if let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) {
                    let e = world.probe.venue_paid.entry(format!("{kind:?}")).or_default();
                    e.0 = e.0.saturating_sub(1);
                    e.1 -= paid;
                    note_unmirrored(world, id, kind, -1);
                }
            }
            // Review fix: the `SetLeisureTax` surcharge comes back too.
            refund_surcharge(world, owner, paid);
            if units > 0 {
                world.add_stock(b, crate::components::Good::Food, units);
            }
        }
        ActionKind::HangOut | ActionKind::Preach => leave_spot(world, id),
        _ => {}
    }
}

/// L2 shadow fixes item 4: an abandoned paid step keeps its purchase (no
/// refund): a meal out is eaten, an entry gives its fun for the share of
/// the visit had; a bet stands.
pub fn abort_paid(world: &mut World, id: EntityId, kind: ActionKind, started: Tick) {
    let b = world.comp::<Brain>(id).and_then(|b| b.current_step()).and_then(|s| s.target).or_else(|| here(world, id));
    match kind {
        ActionKind::EatOut => {
            let (units, _) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            if units > 0 {
                let cfg = world.config.needs.clone();
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    crate::needs::eat(n, &cfg);
                }
                crate::systems::health::ate(world, id, crate::components::MealKind::Real);
                world.mark_day(id, crate::components::trace_flags::ATE);
                world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            }
        }
        ActionKind::Enjoy => {
            world.pending_purchase.remove(&id);
            if let Some(b) = b {
                let share = (world.tick.saturating_sub(started) as f32 / 90.0).min(1.0);
                let gain = gain_at(world, b) * share;
                add_fun(world, id, gain);
            }
        }
        _ => {}
    }
}

/// Plan L14, L15, L20: completion effects.
pub fn on_complete(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>, started: Tick) {
    match kind {
        ActionKind::Enjoy => {
            world.pending_purchase.remove(&id);
            let Some(b) = target else { return };
            let gain = gain_at(world, b);
            add_fun(world, id, gain);
            add_belonging(world, id, belonging_at(world, b));
            world.remember(id, MemoryKind::Enjoyed, None, 0.2, 0.3, false);
            crate::systems::jobs::note_visit(world, b);
            phase4_visit(world, id, b);
            // Lives at work phase 1 (§ 2.1): the visit queues at the venue's counter.
            let price = price_of(world, b);
            crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, price);
        }
        ActionKind::EatOut => {
            let (units, _) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            let Some(b) = target else { return };
            if units > 0 {
                // Lives at work phase 1 (§ 2.1): the meal is the cook's.
                let price = price_of(world, b);
                crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, price);
                crate::systems::bins::venue_meal(world, b);
                let cfg = world.config.needs.clone();
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    crate::needs::eat(n, &cfg);
                }
                crate::systems::health::ate(world, id, crate::components::MealKind::Real);
                world.mark_day(id, crate::components::trace_flags::ATE);
                world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            }
            add_fun(world, id, gain_at(world, b));
            crate::systems::jobs::note_visit(world, b);
        }
        ActionKind::Gamble => {
            let Some(b) = target else { return };
            // A FightPit's bettor had the pit's evening already (its Enjoy).
            if kind_of(world, b) == Some(BuildingKind::Den) {
                add_fun(world, id, gain_at(world, b));
                world.remember(id, MemoryKind::Enjoyed, None, 0.2, 0.3, false);
                crate::systems::jobs::note_visit(world, b);
                // Lives at work phase 1 (§ 2.1): the table game names its croupier.
                crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, 0);
            }
        }
        ActionKind::HangOut => hangout_done(world, id, started),
        ActionKind::Preach => {
            preach_done(world, id);
            leave_spot(world, id);
        }
        // Lives at work phase 4b (§ 2.3): a pimp's collection, else the leader's.
        ActionKind::Collect if !crate::systems::vice::pimp_collect(world, id, target) => {
            collect_step(world, id, target)
        }
        _ => {}
    }
}

/// Jobs & lives phase 4: a paid venue visit's vice and fashion terms (the
/// tip, intimacy, status, a Boutique's clothing; nothing with both off).
pub(super) fn phase4_visit(world: &mut World, id: EntityId, b: EntityId) {
    if !crate::systems::vice::on(world) && !crate::systems::fashion::on(world) {
        return;
    }
    // Jobs & lives phase 5: a Dollhouse's visit, an Arcade's unit (nothing off).
    crate::systems::dollhouse::on_visit(world, b, price_of(world, b));
    if kind_of(world, b) == Some(BuildingKind::Arcade) {
        crate::systems::braindance::on_arcade_visit(world, b);
    }
    let price = price_of(world, b);
    let high = rung_of(world, b, price) == Rung::High;
    // The pressure pass phase 2: a lonely client's visit (counted before
    // the visit's intimacy lands).
    if crate::systems::customers::lonely(world, id) > 0.0 && crate::systems::customers::worker_on(world, b) {
        ownership::jobs2_row(world).lonely_visits += 1;
    }
    crate::systems::vice::on_visit(world, id, b, price);
    crate::systems::fashion::on_visit(world, id, b, high);
}

/// Plan L15: a HangOut ends: fun by the hour, `chat_belonging`, and one
/// exchange with the co-hanger of highest affinity (else the first present,
/// ties the lower id): `social::interacted` (a first meeting if strangers;
/// its noise from a `splitmix64` hash), the legacy `social::gossip` when it
/// is the live path, `gossip::exchange(.., Chat)`; kin note the day they
/// met; street dice between two bored co-hangers.
fn hangout_done(world: &mut World, id: EntityId, started: Tick) {
    let tile = world.comp::<Position>(id).map(|p| p.tile);
    let hours = world.tick.saturating_sub(started) as f32 / TICKS_PER_HOUR as f32;
    // Lives at work phase 2 (§ 3.1): inside a Home the company is the
    // household present, not the street registry.
    let at_home = crate::systems::household::hanging_at_home(world, id);
    let others: Vec<EntityId> = match at_home {
        Some(h) => crate::systems::household::residents_inside(world, h, id)
            .into_iter()
            .filter(|&o| world.has::<Brain>(o))
            .collect(),
        None => tile
            .and_then(|t| world.hangouts.get(&t))
            .map(|v| v.iter().copied().filter(|&o| o != id && world.has::<Brain>(o)).collect())
            .unwrap_or_default(),
    };
    let mut gain = world.config.leisure.gain.hangout_hour * hours.max(0.5);
    // Island fix round F3 (OPEN-190): the stay-in's hour (the own Home
    // past the spot cap) is `stay_in.fun_hour` alone, and never less in
    // company; it is not a spot left empty.
    let stay_in =
        world.unwind.get(&id).is_some_and(|p| p.stay_in) && at_home.is_some_and(|h| Some(h) == home_of(world, id));
    let stay_cfg = crate::systems::distance::stay_in(world).filter(|_| stay_in).cloned();
    if let Some(c) = &stay_cfg {
        ownership::jobs2_row(world).stay_in_hangouts += 1;
        gain = gain.max(c.fun_hour * hours.max(0.5));
    }
    // L2 shadow fixes item 5: an hour alone on the corner is `spot_lone_fun`
    // of one in company.
    if let (true, Some(c)) = (others.is_empty(), &stay_cfg) {
        gain = c.fun_hour * hours.max(0.5);
    } else if others.is_empty() {
        gain *= world.config.life.spot_lone_fun;
        // Lives at work phase 2 (§ 3.3): a spot left early is remembered empty.
        if world.tick.saturating_sub(started) < world.config.leisure.hangout_ticks {
            // Review fix 6: a Home's spot is keyed by its outside door (the
            // tile `SpotKind::Home` is scored by), not the interior tile.
            let spot = match at_home {
                Some(h) => world.comp::<Building>(h).map(|b| world.outside_door(b)),
                None => tile,
            };
            if let Some(t) = spot {
                crate::systems::household::remember_empty(world, id, t);
            }
            // The own Home left empty is counted apart (the fix round's device).
            if at_home.is_some_and(|h| Some(h) == home_of(world, id)) {
                ownership::jobs2_row(world).home_empty_leaves += 1;
            }
        }
    }
    if let Some(h) = at_home {
        let own = Some(h) == home_of(world, id);
        let row = ownership::jobs2_row(world);
        if own {
            row.home_hangouts += 1;
        } else {
            row.friend_hangouts += 1;
        }
    }
    add_fun(world, id, gain);
    let chat = world.config.needs.chat_belonging;
    add_belonging(world, id, chat);
    leave_spot(world, id);
    let known = others.iter().filter(|&&o| world.edge(id, o).is_some()).count();
    let row = &mut world.stats.current.living;
    // Running mean of known co-hangers per completed HangOut.
    let n = row.hangouts as f32;
    row.hangout_contacts_mean = (row.hangout_contacts_mean * n + known as f32) / (n + 1.0);
    row.hangouts += 1;
    let partner = others
        .iter()
        .copied()
        .map(|o| (o, world.edge(id, o).map_or(f32::NEG_INFINITY, |e| e.affinity)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(o, _)| o);
    let Some(p) = partner else { return };
    if world.edge(id, p).is_none() {
        let (sa, sb) = (
            world.comp::<Personality>(id).map_or(0.5, |x| x.sociability),
            world.comp::<Personality>(p).map_or(0.5, |x| x.sociability),
        );
        let (a, b) = pair(id, p);
        let h = splitmix64(world.seed() ^ world.tick.rotate_left(13) ^ (u64::from(a.index) << 32 | u64::from(b.index)));
        let u1 = ((h >> 40) as f32 + 0.5) / (1u64 << 24) as f32;
        let u2 = ((h & 0xFF_FFFF) as f32 + 0.5) / (1u64 << 24) as f32;
        let look = crate::systems::social::first_look(world, id, p);
        let aff = crate::systems::social::first_affinity_with(sa, sb, u1, u2, look);
        crate::systems::social::first_meeting(world, id, p, aff);
    } else {
        crate::systems::social::interacted(world, id, p);
    }
    crate::systems::gossip::exchange(world, id, p, crate::systems::gossip::Venue::Chat);
    if world.edge(id, p).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent)) {
        let today = world.day();
        world.last_met.insert(pair(id, p), today);
    }
    // Lives at work phase 2 (§ 3.1): an evening at home with the spouse is
    // counted; street dice indoors only under `home_dice`.
    if at_home.is_some() {
        if world.spouse_of(id) == Some(p) {
            ownership::jobs2_row(world).home_with_spouse += 1;
        }
        if !world.config.household.home_dice {
            return;
        }
    }
    street_dice(world, id, p);
}

/// Spec § 2: street dice: two co-hangers with fun below 0.5 and a coin each
/// play for 1 coin (`WordNs::Gamble`, key `(tick, a << 32 | b)`; agent to
/// agent, `Flow::StreetDice`).
fn street_dice(world: &mut World, a: EntityId, b: EntityId) {
    let bored = |w: &World, x: EntityId| w.comp::<Needs>(x).is_some_and(|n| n.fun < 0.5) && coins(w, x) >= 1;
    if !bored(world, a) || !bored(world, b) {
        return;
    }
    let (lo, hi) = pair(a, b);
    let u: f32 = world.rng.word(WordNs::Gamble, world.tick, u64::from(lo.index) << 32 | u64::from(hi.index)).random();
    let (winner, loser) = if u < 0.5 { (lo, hi) } else { (hi, lo) };
    ownership::pay(world, Some(loser), Some(winner), 1, Flow::StreetDice);
    for x in [a, b] {
        add_fun(world, x, 0.05);
    }
}

/// Plan L15: who hangs out where, from the bodies' running steps (on load).
pub fn rebuild_hangouts(world: &mut World) {
    world.hangouts.clear();
    let mut map: std::collections::BTreeMap<TilePos, SmallVec<[EntityId; 8]>> = Default::default();
    for id in world.bodies() {
        let running = world
            .comp::<Brain>(id)
            .is_some_and(|b| matches!(b.exec, ExecState::Use { kind: ActionKind::HangOut | ActionKind::Preach, .. }));
        if !running {
            continue;
        }
        if let Some(t) = world.comp::<Position>(id).map(|p| p.tile) {
            map.entry(t).or_default().push(id);
        }
    }
    world.hangouts = map;
}
