use super::*;

/// A Flirt off screen, as `exec/actions.rs` Flirt: the highest-affinity known
/// edge (>= 0.3) with a living, adult, unmarried counterpart not rejected this
/// week (ties: lower id). Ready to propose: the proposal (the second visit of
/// the v1 courtship). Else accepted at the v1 rate `0.3 + affinity + 0.2 x
/// sociability`, rolled on the agent's stream.
pub(super) fn stat_flirt(world: &mut World, id: EntityId) {
    use crate::systems::{demography, social};
    // Round 7 W5 (`OPEN-237`): nobody courts from a cell, or one in it.
    let held = |w: &World, o: EntityId| w.has::<crate::components::Sentence>(o);
    if !demography::is_adult(world, id) || social::has_spouse(world, id) || held(world, id) {
        return;
    }
    // The same candidate rule `calibrate` conditions `p_flirt` on.
    let Some(partner) = social::known_candidate(world, id, 0.3).filter(|&o| demography::is_adult(world, o)) else {
        return;
    };
    if held(world, partner) {
        return;
    }
    if social::propose_allowed(world, id, partner) {
        social::propose(world, id, partner);
        return;
    }
    let aff = world.edge(id, partner).map_or(0.0, |e| e.affinity);
    let sociable = world.comp::<Personality>(partner).map_or(0.5, |p| p.sociability);
    let roll: f32 = world.rng.agent(id).random();
    if roll < 0.3 + aff + 0.2 * sociable {
        for who in [id, partner] {
            if let Some(n) = world.comp_mut::<crate::components::Needs>(who) {
                n.intimacy = (n.intimacy + 0.1).min(1.0);
            }
            world.remember(who, MemoryKind::Courted, Some(if who == id { partner } else { id }), 0.4, 0.3, false);
        }
        social::adjust(world, id, partner, 0.1, 0.02);
        // Round 8 X5 (`OPEN-248`): a date in the pair's courtship.
        social::dated(world, id, partner);
    } else {
        world.remember(id, MemoryKind::Rejected, Some(partner), 0.4, -0.3, false);
    }
}

/// An off-screen Market theft: one unit from the agent's Market into its
/// inventory as stolen food, the `Theft` event and counter, and a
/// `stat_theft_caught_p` report with no witness (the thief then gets a body,
/// which the arrest path needs). Shared by the eat path and the `p_steal`
/// roll. Returns whether anything was taken. Flip readiness: with
/// the market wage law (P10: was `no_net`) the haul is a Full Market theft's (`exec::actions`
/// StealFood: two units where the shelf holds them): off screen it is the
/// only meal a broke thief gets, and one unit sent it back a few hours later.
pub(super) fn stat_theft(world: &mut World, id: EntityId) -> bool {
    let per = if crate::systems::econ::wages_on(world) { 2 } else { 1 };
    // Jobs and room P2 fix: an empty local Market sends the thief to a stocked one.
    let Some(market) = world.food_market(id) else { return false };
    let Some(b) = world.comp_mut::<Building>(market).filter(|b| b.stock_food > 0) else { return false };
    let units = per.min(b.stock_food);
    b.stock_food -= units;
    // M11 D20: a corp-owned Market books the unit at its price.
    let coins = world.price_at(market) * i64::from(units);
    crate::systems::ownership::note_loss(world, market, coins, Some(id));
    if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
        i.food += units;
        i.stolen_food += units;
    }
    world.stats.current.thefts += 1;
    // M12 D17: an off-screen theft's litter, in the Market's district, at a
    // tile drawn from its own key.
    let d = world.district_of_building(market);
    let key = crate::components::hole_id(world.tick, id, HoleKind::Robbed);
    crate::systems::litter::deposit_in_district(world, d, 6, 0, key);
    let name = world.name_of(id);
    let note = crate::systems::misery::theft_note(world, id).unwrap_or_default();
    world.push_event(EventKind::Theft, &[id, market], format!("{name} stole food (off screen){note}"));
    let caught: f64 = world.rng.agent(id).random();
    let p = world
        .stat_table
        .as_ref()
        .and_then(|t| t.p_theft_caught)
        .map_or(world.config.crime.stat_theft_caught_p, f64::from);
    if caught < p {
        crate::systems::law::file_report(world, Crime::Theft, id, None);
        // The normal arrest path needs a body on the map.
        set_lod(world, id, Lod::Coarse);
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Outcome {
    Eat,
    Social,
    Sleep,
    Idle,
}

/// Parity (after P5): an agent with the means of a meal now: food in hand,
/// a stocked pantry, or a meal's coins at its food Market (`p_meal`'s
/// hours; `calibrate` reads it too).
pub fn has_means(world: &World, id: EntityId) -> bool {
    if world.comp::<crate::components::Inventory>(id).is_some_and(|i| i.food >= 1) {
        return true;
    }
    let pantry = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .is_some_and(|h| world.comp::<Building>(h).is_some_and(|b| b.stock_food > 0));
    if pantry {
        return true;
    }
    crate::systems::life::can_buy_meal(world, id)
}

pub(super) fn stat_eat(world: &mut World, id: EntityId) {
    let cfg = world.config.needs.clone();
    let has_food = world.comp::<crate::components::Inventory>(id).is_some_and(|i| i.food >= 1);
    if has_food {
        let mut meal = crate::components::MealKind::Real;
        if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
            meal = crate::systems::health::eat_inventory_unit(i);
            i.stolen_food = i.stolen_food.min(i.food);
        }
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            crate::needs::eat(n, &cfg);
        }
        crate::systems::health::ate(world, id, meal);
        world.mark_day(id, trace_flags::ATE);
        return;
    }
    // The Home pantry feeds a Statistical resident as it feeds a Full one.
    let pantry = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .filter(|&h| crate::systems::camp::adults_share(world, id, h) > 0);
    if let Some(h) = pantry {
        let mut meal = crate::components::MealKind::Real;
        if let Some(b) = world.comp_mut::<Building>(h) {
            meal = crate::systems::health::eat_pantry_unit(b);
        }
        // Round 6 V1 (OPEN-231): off his household's stake first.
        crate::systems::shelf::ate(world, id, h, 1.0);
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            crate::needs::eat(n, &cfg);
        }
        crate::systems::health::ate(world, id, meal);
        world.mark_day(id, trace_flags::ATE);
        return;
    }
    // Jobs and room P2 fix: an empty local Market sends the shopper to a stocked one.
    let Some((market, stock, price)) = world.food_market(id).and_then(|m| {
        world.comp::<Building>(m).map(|b| (m, b.stock_food, crate::systems::diet::price_at(world, id, m)))
    }) else {
        return;
    };
    if stock == 0 {
        return;
    }
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
    // Jobs & lives phase 2 (§ 3): a synth buyer may take a vendor's meal on
    // the street instead (nothing with the street off).
    if coins >= price && crate::systems::streetwork::stat_vendor_meal(world, id, market) {
        return;
    }
    if coins >= price {
        // The purchase a Full agent makes (BuyFood: up to three units), one
        // eaten and the rest stored in the Home pantry (StoreFood), where the
        // household eats it. Buying one meal at a time left a broke
        // housemate nothing to eat once the opening pantry was gone (M10).
        let units = economy::buy_quantity(world, id, Some(market)).max(1);
        let q = economy::quality_at(world, id, Some(market));
        let paid = economy::pay_for_food_q(world, id, Some(market), units, q);
        if economy::take_food_q(world, id, Some(market), units, paid, q) {
            let mut meal = crate::components::MealKind::Real;
            if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
                meal = crate::systems::health::eat_inventory_unit(i);
            }
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
            crate::systems::health::ate(world, id, meal);
            world.mark_day(id, trace_flags::ATE);
            stat_store_food(world, id);
        }
        return;
    }
    // Desperation theft: a starving agent with no food and no coins steals,
    // whatever its lawfulness (with nothing else to eat that is a Full
    // agent's only Eat plan). Every other theft is the table's `p_steal`,
    // calibrated on all Full thefts but these (M10; replaces the v1
    // lawless-and-hungry gate, D29).
    let hunger = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
    // Flip readiness (no safety net): a hungry agent who cannot buy steals
    // the meal by lawfulness, as the Full planner's theft cost reads it
    // (`p_desperate`, calibrated on the Full tier's hungry thieves below the
    // meal price); a lawful agent mostly goes hungry (the noon Mission, the
    // next hour's scavenge or beg).
    // The recalibration (2026-10-09, ported
    // with hunger as sickness P5): the dole city reads it too (`calibrate`
    // learns it in every city), only where the city it was learned in holds:
    // a no-net city or a dole that pays. With the dole cut and no no-net path
    // the off-screen broke have no other meal, so the M10 rule stands. A
    // table without `p_desperate`: the M10 rule (starving, whatever the
    // lawfulness). Adults only; a child keeps the M10 rule.
    let dole_pays = world.levers.dole_per_day > 0 && world.treasury().map_or(0, |t| t.coins) >= 0;
    let learned_city = crate::systems::econ::wages_on(world) || dole_pays;
    let need = if learned_city && crate::systems::demography::is_adult(world, id) {
        let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
        world.stat_table.as_ref().and_then(|t| t.p_desperate.map(|d| (d[t.lawfulness_bucket(law)], t.hunger_edge)))
    } else {
        None
    };
    match need {
        Some((p, edge)) => {
            if hunger >= edge {
                return;
            }
            let u: f32 = world.rng.agent(id).random();
            if u >= p {
                return;
            }
        }
        None if hunger > 0.0 => return,
        None => {}
    }
    if !stat_theft(world, id) {
        return;
    }
    // ... and eaten at once.
    let mut meal = crate::components::MealKind::Real;
    if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
        meal = crate::systems::health::eat_inventory_unit(i);
        i.stolen_food = i.stolen_food.min(i.food);
    }
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    crate::systems::health::ate(world, id, meal);
    world.mark_day(id, trace_flags::ATE);
}

/// Diet parity (hunger fix item 11, P5i (b), off screen): an employed
/// agent off shift whose household's pantry and own carried meals are under
/// `routine::shift_meals_needed` per resident (capped at the pantry) and who
/// holds a meal's coins at its stocked food Market buys the shortfall (at
/// most three meals) into the pantry, as a Full worker's grocery run does. The Full city's
/// jobless ate from the pantries the workers' runs filled (hungry at
/// midnight 0.3-0.7 % against 3-3.6 % off screen on the flipped city). S6
/// (OPEN-224): the children's food on top, and a jobless parent by day.
pub(super) fn stat_grocery(world: &mut World, id: EntityId) {
    if !crate::systems::fixes::shift_meals(world) {
        return;
    }
    let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else { return };
    // The pressure pass S6 (OPEN-224): the children's food on top (the
    // learned level is the childless city's), and a jobless parent shops
    // for it by day, as the Full parent's run (`routine::grocery_plan`).
    // Round 6 V1 (OPEN-231): his household's children, residents and claim
    // in a shared Home (`shelf.rs`), as the Full run reads them.
    // The view milestone V2b (OPEN-265): the shelf read once (`shelf::Shelf`),
    // and not at all for a worker on shift.
    if world.comp::<Job>(id).is_some_and(|job| job.on_shift(world.tick_of_day())) {
        return;
    }
    let shelf = crate::systems::shelf::Shelf::of(world, home);
    let k = crate::systems::shelf::key(world, id);
    let kids = shelf.kids_need(world, k);
    let cap = world.config.buildings.home.stock_cap;
    let need = match world.comp::<Job>(id) {
        Some(job) => {
            let need_one = crate::exec::routine::shift_meals_needed(world, job);
            let residents = shelf.members(k);
            // The level: the Full city's learned pantry per resident (a run does
            // not always win the evening), else a shift's worth each.
            let per =
                world.stat_table.as_ref().and_then(|t| t.pantry_per_resident).map_or(need_one as f32, |p| p.max(0.0));
            (((per * residents as f32).round() as u32).min(need_one * residents) + kids).min(cap)
        }
        // Round 7 W1 (OPEN-243): or any free adult of the household while
        // its claim is short of the children's food (the household's purse).
        None if kids > 0
            && world.phase() != DayPhase::Night
            && (crate::systems::ages::feeds_children(world, id)
                || crate::systems::shelf::feeds_household(world, id)) =>
        {
            kids.min(cap)
        }
        None => return,
    };
    // Round 7 W1 (OPEN-243): short of the children's food on the
    // household's own units (`shelf::kids_short`), the run reads those.
    let kids_short = shelf.kids_short(world, k) > 0;
    let pantry = if kids_short { shelf.own_held(k) } else { shelf.held_by(k) };
    // Round 4 T1 (OPEN-231): on the service's warning the pantry alone
    // holds the children's food (the Full run's `routine::run_level`).
    // Round 7 W1 (OPEN-243): so does a claim short of the children's food.
    let carried = if crate::systems::camp::household_on_warning_in(world, id, home, shelf.shared()) || kids_short {
        0
    } else {
        world.comp::<crate::components::Inventory>(id).map_or(0, |i| i.food.saturating_sub(i.stolen_food))
    };
    if pantry + carried >= need {
        return;
    }
    let Some(market) = world.food_market(id) else { return };
    let price = crate::systems::diet::price_at(world, id, market);
    let stocked = world.comp::<Building>(market).is_some_and(|b| b.stock_food > 0);
    if !stocked || world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins) < price {
        return;
    }
    // Up to the shortfall only (a run of three meals each time stocked the
    // off-screen pantries at 0.84 meals a resident against 0.28 in Full),
    // and all of it in one run, as a Full grocery run buys it
    // (`routine::grocery_cap`: not three meals at a time).
    let short = need - (pantry + carried);
    let units = economy::buy_quantity_upto(world, id, Some(market), short);
    if units == 0 {
        return;
    }
    let q = economy::quality_at(world, id, Some(market));
    let paid = economy::pay_for_food_q(world, id, Some(market), units, q);
    if economy::take_food_q(world, id, Some(market), units, paid, q) {
        stat_store_food(world, id);
    }
}

/// StoreFood off screen: the agent's bought (not stolen) food goes into its
/// Home pantry up to the pantry's cap.
fn stat_store_food(world: &mut World, id: EntityId) {
    let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else { return };
    let cap = world.config.buildings.home.stock_cap;
    // Round 6 V1 (OPEN-231): his household's room, and his stake.
    let room = crate::systems::shelf::room(world, id, home, cap);
    let Some(inv) = world.comp_mut::<crate::components::Inventory>(id) else { return };
    let movable = inv.food.saturating_sub(inv.stolen_food).min(room);
    let synth = inv.synth_held().min(movable);
    inv.synth = inv.synth_held() - synth;
    inv.food -= movable;
    inv.synth = inv.synth.min(inv.food);
    if let Some(b) = world.comp_mut::<Building>(home) {
        b.synth_food = b.synth_food.min(b.stock_food) + synth;
        b.stock_food += movable;
    }
    crate::systems::shelf::stored(world, id, movable);
}
