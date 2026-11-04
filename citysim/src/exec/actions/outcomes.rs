use super::*;

/// C3a, the Attack's tail on a Guard taker (M16a, plan C32): the
/// attacker's Assault (or Murder) on the guard; true when the attacker won
/// and the victim is still there to be hit.
pub(crate) fn attack_guard_end(
    world: &mut World,
    id: EntityId,
    victim: EntityId,
    g: EntityId,
    c: &crate::systems::combat::Clash,
) -> bool {
    let (winner, died) = (c.winner, c.died);
    if died && winner != id {
        crate::systems::demography::defended(world, id);
    }
    let tile = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
    let text = format!("{} attacked {}'s guard {}", world.name_of(id), world.name_of(victim), world.name_of(g));
    world.push_event(crate::events::EventKind::Assault, &[id, g], text);
    if crate::systems::law::living(world, id) {
        let murder = died && winner == id;
        let crime = if murder { crate::components::Crime::Murder } else { crate::components::Crime::Assault };
        crate::systems::law::raise_crime_on(world, id, (!murder).then_some(g), Some(g), crime, tile);
    }
    if winner != id || !crate::systems::law::living(world, victim) {
        crate::systems::hunt::on_strike(world, id, victim, g, false);
        return false;
    }
    true
}

/// C3a, the Attack's tail: the attacker's crime (Murder only when the
/// victim died; an attacker who died resisting is charged with nothing),
/// the line, the bouncer, the word he owed, the Hunt's strike.
pub(crate) fn attack_end(world: &mut World, id: EntityId, victim: EntityId, c: &crate::systems::combat::Clash) {
    let (winner, loser, died) = (c.winner, c.loser, c.died);
    let murder = died && loser == victim;
    let crime = if murder { crate::components::Crime::Murder } else { crate::components::Crime::Assault };
    let tile = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
    let name = world.name_of(id);
    // Round 8 X1 (`OPEN-246`): a fight in the cells says where (V16
    // `pimp_14518 d26 06:18`: no place, no case).
    let cells = crate::systems::law::cells_of(world, id)
        .filter(|_| crate::systems::law::cells_of(world, victim).is_some())
        .map_or_else(String::new, |j| format!(" in the cells of {}", world.name_of(j)));
    let text = if died && loser == id {
        format!("{name} attacked {}{cells} and died", world.name_of(victim))
    } else {
        format!("{name} attacked {}{cells}", world.name_of(victim))
    };
    if died && loser == id {
        crate::systems::demography::defended(world, id);
    }
    world.push_event(
        if murder { crate::events::EventKind::Murder } else { crate::events::EventKind::Assault },
        &[id, victim],
        text,
    );
    if crate::systems::law::living(world, id) {
        crate::systems::law::raise_crime_on(world, id, (!murder).then_some(victim), Some(victim), crime, tile);
        // Lives at work phase 1 (§ 2.1): an Assault inside a kept
        // venue may end with the attacker thrown out by the bouncer.
        crate::systems::service::maybe_eject(world, id);
    }
    // Round 8 X1: he meant to tell the law of the man, and went for
    // him himself (V16 `gun_44357 d22 17:32`).
    if crate::systems::law::owes_word_on(world, id, victim) {
        let (he, him, _) = crate::systems::consequence::pro(world, id);
        let why = format!("{he} went for {} {him}self instead", world.name_of(victim));
        crate::systems::law::drop_word(world, id, victim, &why);
    }
    // C3b, the chase follows the purpose: a Hunt whose target broke and
    // fled keeps the Hunt (the hunter goes after him); a mugger lets him go.
    if c.end == crate::systems::combat::End::Fled
        && loser == victim
        && world.hunts.get(&id).is_some_and(|h| h.target == victim)
        && crate::systems::law::living(world, id)
    {
        // C4 (C3b's wrong-read): one line a chase, not one each time the
        // target breaks and runs again (`Live::chased`, over with the Hunt).
        if world.combat.chased.get(&id) != Some(&victim) {
            world.combat.chased.insert(id, victim);
            let text = format!("{name} gave chase after {}", world.name_of(victim));
            world.push_event(crate::events::EventKind::Fight, &[id, victim], text);
            crate::systems::ownership::jobs2_row(world).chases += 1;
        }
        return;
    }
    // M15 W22: the Hunt's strike (after the crime: street silence reads the Hunt).
    crate::systems::hunt::on_strike(world, id, victim, winner, died);
}

/// Theft: units leave the source into the thief's inventory as stolen food.
/// Witnesses, crime reports and arrests arrive with the law system (M4); for
/// now the event and the daily counter record it.
pub(super) fn steal_food(world: &mut World, id: EntityId, source: StealSource, target: Option<EntityId>) -> StepResult {
    let (building, take) = match source {
        StealSource::Market => (world.local(id, BuildingKind::Market), 2),
        StealSource::Home => (target, 2),
        StealSource::Warehouse => (world.building_of_kind(BuildingKind::Warehouse), 5),
    };
    let Some(b) = building else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let Some(bd) = world.comp_mut::<Building>(b) else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let units = take.min(bd.stock_food);
    if units == 0 {
        return StepResult::Failed(FailReason::StockGone);
    }
    bd.stock_food -= units;
    let kind = bd.kind;
    if let Some(inv) = world.comp_mut::<Inventory>(id) {
        inv.food = (inv.food + units).min(20);
        inv.stolen_food = (inv.stolen_food + units).min(inv.food);
    }
    world.stats.current.thefts += 1;
    // M11 D20: a corp-owned Market books the loss at its price.
    if kind == BuildingKind::Market {
        let coins = i64::from(units) * world.price_at(b);
        crate::systems::ownership::note_loss(world, b, coins, Some(id));
    }
    let name = world.name_of(id);
    world.push_event(
        crate::events::EventKind::Theft,
        &[id, b],
        format!(
            "{name} stole {units} food from the {}#{}{}",
            kind.label(),
            b.index,
            crate::systems::misery::theft_note(world, id).unwrap_or_default()
        ),
    );
    world.release_all(id);
    // Witnesses: the victims are the building's occupants (a Home's residents,
    // the clerk on duty); for the Market and Warehouse there is no single victim.
    let tile = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
    let victim = match source {
        // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
        // Round 5 U7 (`OPEN-237`): the robbed is a resident at home, never a
        // visitor there (a thief who had just stolen from the same Home was
        // "robbed" by the next thief, `homeless_71553 d21 16:26`).
        StealSource::Home => world.comp::<Building>(b).and_then(|bd| {
            let res = world.residents_of(b);
            bd.occupants
                .iter()
                .copied()
                .find(|&o| o != id && res.contains(&o) && !crate::systems::ages::is_protected(world, o))
        }),
        _ => None,
    };
    crate::systems::law::raise_crime(world, id, victim, crate::components::Crime::Theft, tile);
    // Jobs & lives phase 3 (Dylan's answer 1): the petty theft of items on
    // the same path (a body's carried item or a stall's till at a Market,
    // an item kept at a Home).
    match source {
        StealSource::Market => crate::systems::weapons::market_petty(world, id),
        StealSource::Home => crate::systems::weapons::home_petty(world, id, b),
        StealSource::Warehouse => {}
    }
    StepResult::Done
}

/// Beg: up to 4 passers-by within 3 tiles (lowest id first); each with
/// `edge.affinity > 0` and `coins > 10` rolls `rng < 0.3 + sociability × 0.4`;
/// the first success gives `1 + (rng < 0.5)` coins and records a debt. Fails
/// (memory Rejected) if nothing was gained.
pub(super) fn beg(world: &mut World, id: EntityId) -> StepResult {
    use rand::Rng;
    let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else {
        return StepResult::Failed(FailReason::NoSuchPlace);
    };
    let sociability = world.comp::<crate::components::Personality>(id).map_or(0.5, |p| p.sociability);
    // Scale S4 perf: the first four citizens by id within 3 tiles, from the
    // position index (ascending, every entity with a Position near the
    // tile, Statistical passers included), not a scan of every citizen.
    let passers: Vec<EntityId> = world
        .agents_near(tile, 3, None)
        .into_iter()
        .filter(|&o| world.has::<crate::components::Identity>(o) && !world.has::<crate::components::Corpse>(o))
        .filter(|&o| o != id)
        // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
        .filter(|&o| !crate::systems::ages::is_protected(world, o))
        .filter(|&o| world.comp::<Position>(o).is_some_and(|p| p.tile.manhattan(tile) <= 3))
        .take(4)
        .collect();
    // L1: a stranger may give too (the broke begged only from friends, and
    // the street is strangers), at half the odds.
    for other in passers {
        let key = crate::components::edge_key(id, other);
        let edge = world.edges.get(&key).map(|e| e.affinity);
        let generous = (edge.is_some_and(|a| a > 0.0) || (edge.is_none()))
            && world.comp::<Wallet>(other).is_some_and(|w| w.coins > 10);
        if !generous {
            continue;
        }
        let roll: f32 = world.rng.world().random();
        let mut odds = (0.3 + sociability * 0.4) * if edge.is_none() { 0.5 } else { 1.0 };
        // Jobs & lives phase 7 (§ 6.3): the maimed's bowl (1 off).
        let m = crate::systems::ages::beg_mult(world, id);
        if m != 1.0 {
            odds *= m;
        }
        if roll < odds {
            let bonus: f32 = world.rng.world().random();
            let coins = 1 + i64::from(bonus < 0.5);
            if let Some(w) = world.comp_mut::<Wallet>(other) {
                w.coins -= coins;
            }
            if let Some(w) = world.comp_mut::<Wallet>(id) {
                w.coins += coins;
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.beg_dry = 0;
            }
            {
                // the lower id owes the higher id
                let (lo, _) = key;
                let tick = world.tick;
                let e = world.edge_entry(id, other);
                e.debt += if lo == id { coins as i32 } else { -(coins as i32) };
                e.debt_since.get_or_insert(tick);
                e.last_interaction = tick;
            }
            return StepResult::Done;
        }
    }
    // Hunger fix G4: nobody gave; an hour honestly spent (Scavenge's dry
    // hour), retried, until `beg_dry_max` in a row cool Earn.
    if crate::systems::fixes::beg_retry(world) {
        let max = world.config.life.beg_dry_max.max(1);
        let cool = world.tick + world.config.life.beg_cool_hours * crate::time::TICKS_PER_HOUR;
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.beg_dry = b.beg_dry.saturating_add(1);
            if b.beg_dry >= max {
                b.beg_dry = 0;
                b.cooldowns.insert(crate::components::GoalKind::Earn, cool);
            }
        }
        return StepResult::Done;
    }
    world.remember(id, MemoryKind::Rejected, None, 0.3, -0.3, false);
    StepResult::Failed(FailReason::PreconditionLost)
}

/// Walks with a completion effect: Escort delivers the suspect, FleeToHome
/// settles the nerves.
pub fn on_arrive(world: &mut World, id: EntityId, step: &crate::components::ActionInstance) {
    match step.action {
        ActionKind::Escort => {
            let suspect = world.comp::<Brain>(id).and_then(|b| b.escorting).or(step.target);
            if let Some(s) = suspect {
                crate::systems::law::jail_suspect(world, id, s);
            }
        }
        ActionKind::FleeToHome => {
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.safety = n.safety.max(0.5);
            }
            // L2 shadow fixes item 6: the flight holds a while: no leisure
            // or company out on the street for `flee_clear_ticks`.
            let until = world.tick + world.config.life.flee_clear_ticks;
            if let Some(b) = world.comp_mut::<Brain>(id) {
                for g in [crate::components::GoalKind::Unwind, crate::components::GoalKind::Socialise] {
                    let c = b.cooldowns.entry(g).or_insert(0);
                    *c = (*c).max(until);
                }
            }
        }
        // L1: at the last-seen tile, chase a suspect still in sight; a
        // sighting gone stale is dropped (the Arrest step then fails).
        ActionKind::GoTo(crate::goap::LocationKey::SuspectTile) => {
            if let Some(s) = step.target {
                crate::systems::life::arrive_at_suspect(world, id, s);
            }
        }
        _ => {}
    }
}

/// `ReportCrime` at the Hall: file the most salient unreported first-hand
/// SawCrime as a report.
pub(super) fn report_crime(world: &mut World, id: EntityId, target: Option<EntityId>) -> StepResult {
    // Round 8 X1 (`OPEN-246`): the promise kept at the desk (`law::word_plan`).
    if let Some(s) = target.filter(|&s| crate::systems::law::owes_word_on(world, id, s)) {
        let desk = world.comp::<Position>(id).and_then(|p| p.building);
        crate::systems::law::told_at_desk(world, id, s, desk, false);
        return StepResult::Done;
    }
    let betraying = world.comp::<Brain>(id).is_some_and(|b| b.betraying);
    let leader = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)).and_then(|g| g.leader);
    let Some(mem) = world.comp::<crate::components::Memory>(id) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    let best = mem
        .entries
        .iter()
        .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.salience >= 0.5)
        .filter(|e| e.subject.is_some_and(|s| !crate::systems::law::reported_since(world, s, e.tick)))
        // A betrayer names the leader's most salient crime first.
        .max_by(|a, b| {
            (betraying && a.subject == leader)
                .cmp(&(betraying && b.subject == leader))
                .then(a.salience.total_cmp(&b.salience))
        })
        .map(|e| (e.subject, e.crime));
    let Some((Some(suspect), crime)) = best else { return StepResult::Failed(FailReason::PreconditionLost) };
    let crime = crime.unwrap_or(crate::components::Crime::Theft);
    crate::systems::law::file_report(world, crime, suspect, Some(id));
    if let Some(p) = world.comp_mut::<crate::components::Personality>(id) {
        p.drift(crate::personality::Drift::ReportedCrime);
    }
    if betraying && leader == Some(suspect) {
        crate::systems::gang::betrayal_filed(world, id);
    }
    StepResult::Done
}

/// Shift end: the shift is marked worked (so Work stays pending while it
/// runs and a resumed shift is not counted twice) and a day of wages is owed.
/// A guard's wages are owed by the shift clock instead
/// (`law::credit_guard_shifts`), whichever plan the guard ends the shift on.
pub fn end_shift(world: &mut World, id: EntityId) {
    // Lives at work phase 1 (§ 2.1): the shift-end rule shared with the
    // Statistical tier (the cook's staff meal, the staffed-shift count).
    crate::systems::service::shift_end(world, id);
    // Lives at work phase 6 (§ 4.3): the profession's day (both tiers).
    crate::systems::careers::shift_end(world, id);
    let tick = world.tick;
    let mut owed = false;
    if let Some(j) = world.comp_mut::<Job>(id) {
        j.last_shift_day = Some(j.shift_key_at(tick.saturating_sub(1)));
        if j.role != crate::components::Role::Guard {
            j.days_unpaid = j.days_unpaid.saturating_add(1);
            owed = true;
        }
    }
    // L1: paid at the workplace as the shift ends (as the Statistical tier
    // always was), not on a walk to the Hall; a short payment carries over
    // to the next shift's.
    if owed {
        // The pressure pass phase 2 (OPEN-176): a cab driver's body is paid
        // by the fares he drove, not for Rest at the Garage.
        match crate::systems::customers::fare_scale(world, id) {
            Some(scale) => economy::collect_wage_scaled(world, id, scale),
            None => economy::collect_wage(world, id),
        };
        return;
    }
    economy::maybe_quit(world, id);
}

/// L1 `Scavenge`: `[life] scavenge_coins` from the Treasury (the Recycler's
/// scrap price, `Flow::Sanitation`) when it is not negative; under the market wage law
/// from the Recycler's till (E33).
pub(super) fn scavenge(world: &mut World, id: EntityId) -> StepResult {
    // The find and its pay (`jobs::scavenge_find`, shared with the
    // Statistical tier's no-net hour since the flip-readiness round).
    if !crate::systems::jobs::scavenge_find(world, id) {
        // L2 (L29, `[lod] budget`): a dry hour is an hour honestly spent,
        // not an abort; `scavenge_dry_max` of them in a row cool Earn.
        let now = world.tick;
        let max = world.config.life.scavenge_dry_max.max(1);
        let cool = now + world.config.life.scavenge_cool_hours * crate::time::TICKS_PER_HOUR;
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.scavenge_dry = b.scavenge_dry.saturating_add(1);
            if b.scavenge_dry >= max {
                b.scavenge_dry = 0;
                b.cooldowns.insert(crate::components::GoalKind::Earn, cool);
            }
        }
        world.stats.current.budget.scavenge_dry += 1;
        return StepResult::Done;
    }
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.scavenge_dry = 0;
    }
    StepResult::Done
}

/// Sleeping at home with a spouse who lives there too. Checked by household
/// rather than by the spouse's position at wake-up, because one of them has
/// usually left for a shift by then.
pub(super) fn spouse_in_same_home(world: &World, id: EntityId) -> bool {
    let Some(here) = world.comp::<Position>(id).and_then(|p| p.building) else { return false };
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    home == Some(here) && world.spouse_of(id).is_some_and(|s| world.comp::<Household>(s).and_then(|h| h.home) == home)
}
