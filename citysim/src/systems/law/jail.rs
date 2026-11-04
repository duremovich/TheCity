use super::*;

/// Prisoners get one meal a day from the Market nearest the Jail, paid by the
/// Treasury at the city's mean price to the Market's owner (M11 D46; the
/// coins used to vanish), and a full night's sleep. Pressure pass round 3,
/// S3 (`[justice]`): the meal from the first of the nearest
/// `kitchen_markets` Markets with food (a bare kitchen is a `Cells` line for
/// a body on screen), and the night's sleep is the off-screen hold's alone
/// (a body sleeps at lights-out, `in_lights_out`).
pub(super) fn jail_upkeep(world: &mut World) {
    let lights = world.config.justice.is_some();
    // Round 8 X6 (`OPEN-217`): the meal at `[justice] meal_hour` (`jail_meals`);
    // 0 (or no `[justice]`) at midnight, as before.
    if world.config.justice.as_ref().is_none_or(|c| c.meal_hour == 0) {
        serve_meals(world, false);
    }
    for who in world.with::<Sentence>() {
        if lights && !crate::systems::lod::is_held(world, who) {
            continue;
        }
        if let Some(n) = world.comp_mut::<Needs>(who) {
            n.energy = 1.0;
        }
    }
}

/// Round 8 X6 (`OPEN-217`), hourly: the cells' meal at `[justice] meal_hour`
/// (not 0), every prisoner's line its own ("ate the cells' meal"; V16's jail
/// days had no line at all).
pub(super) fn jail_meals(world: &mut World) {
    let Some(h) = world.config.justice.as_ref().map(|c| c.meal_hour).filter(|&h| h > 0 && h < 24) else { return };
    if world.tick_of_day() / crate::time::TICKS_PER_HOUR as u16 != u16::from(h) {
        return;
    }
    serve_meals(world, true);
}

/// The cells' meal for every prisoner (`jail_meal`); `lined`: each fed
/// prisoner's line too (the kitchen it came from).
fn serve_meals(world: &mut World, lined: bool) {
    let price = world.mean_price();
    let kitchens = jail_kitchens(world);
    let cfg = world.config.needs.clone();
    let lights = world.config.justice.is_some();
    // Lives at work phase 3 (§ 2.2, jailer): the meal and the count are the
    // jailer's: a fed prisoner is an `Inmate` encounter (civic only).
    let jail = world.building_of_kind(BuildingKind::Jail).filter(|_| crate::systems::service::civic_on(world));
    for who in world.with::<Sentence>() {
        if let Some(market) = jail_meal(world, who, &kitchens, price, &cfg) {
            // Lives at work phase 7: a Station's held are fed, but the
            // Precinct's jailer serves only the Precinct's.
            let cell = world.comp::<Position>(who).and_then(|p| p.building);
            let in_station = cell.is_some_and(|b| crate::systems::stations::is_station(world, b));
            if let Some(j) = jail.filter(|_| !in_station) {
                crate::systems::service::arrive(world, who, j, crate::components::ServeKind::Inmate, 0);
            }
            if lined {
                let at = cell.map_or_else(|| "the cells".to_string(), |b| format!("the cells of {}", world.name_of(b)));
                let text = format!("{} ate the meal in {at} (food from {})", world.name_of(who), world.name_of(market));
                let mut actors = vec![who];
                actors.extend(cell);
                world.push_event(EventKind::Cells, &actors, text);
                crate::systems::ownership::jobs2_row(world).cell_meals += 1;
            }
        } else if lights && (lined || !crate::systems::lod::is_held(world, who)) {
            let text = format!("{} went without: the cells' kitchen had no food", world.name_of(who));
            world.push_event(EventKind::Cells, &[who], text);
        }
    }
}

/// Pressure pass round 3, S3: is it lights-out in the cells (`[justice]`
/// `lights_out_hour`..`wake_hour`)? A sentenced body sleeps then.
pub fn in_lights_out(world: &World) -> bool {
    let Some(c) = world.config.justice.as_ref() else { return false };
    let h = (u64::from(world.tick_of_day()) / crate::time::TICKS_PER_HOUR) as u8;
    if c.lights_out_hour <= c.wake_hour {
        h >= c.lights_out_hour && h < c.wake_hour
    } else {
        h >= c.lights_out_hour || h < c.wake_hour
    }
}

/// S3: is `id` a prisoner asleep in the cells now (sentenced, inside, at
/// lights-out)? Read by `needs::run` (the bed's restore) and the shadow tool.
pub fn asleep_in_cell(world: &World, id: EntityId) -> bool {
    in_lights_out(world)
        && world.has::<Sentence>(id)
        && world.comp::<Position>(id).is_some_and(|p| p.building.is_some())
}

/// The cells' kitchens: the Market nearest the Jail's door; with
/// `[justice]`, its nearest `kitchen_markets`, nearest first.
fn jail_kitchens(world: &World) -> Vec<EntityId> {
    let Some(door) = world.building_of_kind(BuildingKind::Jail).and_then(|j| world.comp::<Building>(j).map(|b| b.door))
    else {
        return Vec::new();
    };
    let Some(n) = world.config.justice.as_ref().map(|c| c.kitchen_markets.max(1)) else {
        return world.nearest_of_kind(BuildingKind::Market, door).into_iter().collect();
    };
    let mut all: Vec<(u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .filter_map(|&b| {
            world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| (bd.door.manhattan(door), b))
        })
        .collect();
    all.sort_unstable();
    all.into_iter().take(n).map(|(_, b)| b).collect()
}

/// One Jail meal for a prisoner from the first of `kitchens` with food on
/// the shelf, the Treasury paying (`Flow::JailFood`): the Market it came
/// from. `None` when every shelf is empty.
fn jail_meal(
    world: &mut World,
    who: EntityId,
    kitchens: &[EntityId],
    price: i64,
    cfg: &crate::config::NeedsCfg,
) -> Option<EntityId> {
    let market = kitchens.iter().copied().find(|&m| world.comp::<Building>(m).is_some_and(|b| b.stock_food > 0))?;
    if let Some(b) = world.comp_mut::<Building>(market) {
        b.stock_food -= 1;
    }
    if crate::systems::lod::is_held(world, who) {
        world.stats.current.budget.held_fed += 1;
    }
    let owner = world.owner_of(market);
    let paid = crate::systems::ownership::charge(world, None, owner, price, crate::systems::ownership::Flow::JailFood);
    crate::systems::ownership::credit(world, market, paid);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        crate::needs::eat(n, cfg);
    }
    world.mark_day(who, crate::components::trace_flags::ATE);
    Some(market)
}

/// L2 (L21): the held prisoners' day, at midnight before `jail_upkeep`
/// feeds them: the decay not yet settled and a starvation check. By intent
/// both also run every hour (`held_hourly`, as the Statistical tier's hourly
/// loop: `needs::starvation` kills on `starving_since` plus the grace and
/// remembers once a day), so this pass settles at most the midnight tick.
/// Then, for each held prisoner who arrived in the last day,
/// `jail_meet_max` first meetings among the held (and as many gang members
/// with `jail_pitch`), drawn on `WordNs::Held` keyed `(day, id.index)`
/// (`stat_meet`'s first meeting: a bare edge, `MetInJail` both ways); and
/// the cells' pitch: each held member drifts with every other held
/// prisoner by a day of the Jail's co-location drift.
pub fn held_daily(world: &mut World) {
    use crate::systems::{lod, social};
    if !lod::held_on(world) {
        return;
    }
    let held: Vec<EntityId> = world.sentenced().iter().copied().filter(|&p| lod::is_held(world, p)).collect();
    // Entries for prisoners no longer held (dead, released) go.
    world.held_since.retain(|p, _| held.binary_search(p).is_ok());
    if held.is_empty() {
        return;
    }
    for &id in &held {
        settle_held(world, id);
        crate::needs::starvation(world, id);
    }
    let held: Vec<EntityId> = held.into_iter().filter(|&p| living(world, p)).collect();
    let now = world.tick;
    let today = world.day();
    let cap = world.config.life.jail_meet_max;
    let pitch = world.config.life.jail_pitch;
    let member = |w: &World, x: EntityId| pitch && w.has::<crate::components::GangMember>(x);
    let arrivals: Vec<EntityId> = held
        .iter()
        .copied()
        .filter(|&p| world.comp::<Position>(p).is_some_and(|pos| pos.entered + TICKS_PER_DAY >= now))
        // One arrival, one round of meetings: a prisoner who met the cells
        // as a body (`social::colocation`'s arrival) meets nobody again.
        .filter(|&p| {
            let since = world.comp::<Position>(p).map_or(now, |pos| pos.entered);
            !world
                .comp::<crate::components::Memory>(p)
                .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::MetInJail && e.tick >= since))
        })
        .collect();
    for a in arrivals {
        let mut rng = world.rng.word(crate::word::WordNs::Held, today, u64::from(a.index));
        let mut strangers: Vec<(bool, u64, EntityId)> = held
            .iter()
            .copied()
            .filter(|&b| b != a && world.edge(a, b).is_none())
            .map(|b| (member(world, b), rng.random::<u64>(), b))
            .collect();
        strangers.sort_unstable();
        let (mut plain, mut gang) = (0usize, 0usize);
        for (m, _, b) in strangers {
            let n = if m { &mut gang } else { &mut plain };
            if *n >= cap {
                continue;
            }
            *n += 1;
            let sa = world.comp::<Personality>(a).map_or(0.5, |p| p.sociability);
            let sb = world.comp::<Personality>(b).map_or(0.5, |p| p.sociability);
            let (u1, u2): (f32, f32) = (rng.random(), rng.random());
            social::first_meeting(world, a, b, social::first_affinity(sa, sb, u1, u2));
            world.remember(a, MemoryKind::MetInJail, Some(b), 0.5, 0.0, false);
            world.remember(b, MemoryKind::MetInJail, Some(a), 0.5, 0.0, false);
        }
    }
    if pitch {
        let hours = 24.0;
        let members: Vec<EntityId> = held.iter().copied().filter(|&m| member(world, m)).collect();
        for &m in &members {
            for &p in &held {
                // Each pair once: a member with a non-member, or the lower id of two members.
                if p == m || (member(world, p) && p < m) {
                    continue;
                }
                // Pressure pass round 3, S3 (OPEN-223): the met only (see
                // `social::colocation`'s jail drift): a member's day drifted
                // him with the whole hold, an edge with each.
                if world.edge(m, p).is_none() {
                    continue;
                }
                let step = world.config.social.affinity_per_hour * social::similarity_mult(world, m, p);
                social::adjust(world, m, p, hours * step, hours * 0.02);
            }
        }
    }
}

/// L2 (L21): hourly, every held prisoner's jailed decay since its last
/// settlement and the starvation check, hourly by intent (the Statistical
/// tier's hourly step; ~150 prisoners an hour).
pub(super) fn held_hourly(world: &mut World) {
    if !crate::systems::lod::held_on(world) {
        return;
    }
    let held: Vec<EntityId> =
        world.sentenced().iter().copied().filter(|&p| crate::systems::lod::is_held(world, p)).collect();
    let mut kitchen: Option<(Vec<EntityId>, i64, crate::config::NeedsCfg)> = None;
    for id in held {
        settle_held(world, id);
        // L2 phase 5: a held prisoner who is starving (an arrest of the
        // starving: thieves of food arrive at hunger 0) gets the cell's
        // meal now, not at midnight: on 42-47 one or two a run died within
        // the hour of arrival (the gate's "no held prisoner starves").
        if world.comp::<Needs>(id).is_some_and(|n| n.starving_since.is_some()) {
            let (kitchens, price, cfg) = kitchen
                .get_or_insert_with(|| (jail_kitchens(world), world.mean_price(), world.config.needs.clone()))
                .clone();
            jail_meal(world, id, &kitchens, price, &cfg);
        }
        crate::needs::starvation(world, id);
    }
    // Pressure pass round 3, S3: a prisoner on screen (a pinned body in the
    // cells) starving gets the same meal now (the hold's rule was the held's
    // alone: `jailed_75054` starved from d26 at Full).
    if world.config.justice.is_some() {
        let bodies: Vec<EntityId> = world
            .sentenced()
            .iter()
            .copied()
            .filter(|&p| !crate::systems::lod::is_held(world, p))
            .filter(|&p| world.comp::<Needs>(p).is_some_and(|n| n.starving_since.is_some()))
            .collect();
        for id in bodies {
            let (kitchens, price, cfg) = kitchen
                .get_or_insert_with(|| (jail_kitchens(world), world.mean_price(), world.config.needs.clone()))
                .clone();
            jail_meal(world, id, &kitchens, price, &cfg);
        }
    }
}

/// L2 (L21): a held prisoner's jailed decay for the ticks since it was last
/// settled (`World::held_since`: its demotion into the hold, or the last
/// midnight pass), at most a day; the body's per-tick decay covered the
/// rest of the day. Restarts the clock.
pub fn settle_held(world: &mut World, id: EntityId) {
    let now = world.tick;
    let since = world.held_since.get(&id).copied().unwrap_or(now.saturating_sub(TICKS_PER_DAY));
    let ticks = now.saturating_sub(since).min(TICKS_PER_DAY) as u32;
    world.held_since.insert(id, now);
    if ticks == 0 {
        return;
    }
    if let Some(log) = world.shadow_notes.as_mut() {
        log.push(crate::word::ShadowNote::Settled { tick: now, id, ticks });
    }
    let cfg = world.config.needs.clone();
    let season_energy_mult = world.config.economy.energy_decay_mult[world.season().index()];
    let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
    let under_18 = !crate::systems::demography::is_adult(world, id);
    if let Some(n) = world.comp_mut::<Needs>(id) {
        let ctx = crate::needs::DecayCtx {
            jailed: true,
            season_energy_mult,
            sociability,
            under_18,
            ..crate::needs::DecayCtx::plain()
        };
        crate::needs::decay(n, &cfg, &ctx, ticks);
    }
    // L2 L13 (phase 2): the hold's `fun` decay too (a no-op with leisure off).
    let execs = crate::systems::classes::exec_set(world);
    let rate = crate::systems::leisure::fun_per_hour(world, id, &execs);
    if let Some(n) = world.comp_mut::<Needs>(id) {
        crate::systems::leisure::decay_fun(n, rate, ticks as f32 / crate::time::TICKS_PER_HOUR as f32);
    }
}

pub(super) fn releases(world: &mut World) {
    let tick = world.tick;
    let due: Vec<EntityId> = world
        .sentenced()
        .iter()
        .copied()
        .filter(|&p| world.comp::<Sentence>(p).is_some_and(|s| tick >= s.until_tick))
        .collect();
    for p in due {
        release(world, p, true);
    }
}

/// Player `Arrest`: no witness roll, straight to the Jail for `3 × sentence_mult` days.
pub fn player_arrest(world: &mut World, who: EntityId) -> Result<(), String> {
    if !world.is_alive(who) || !world.has::<Brain>(who) {
        return Err("no such agent".into());
    }
    if world.has::<Sentence>(who) {
        return Err("already jailed".into());
    }
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return Err("no Precinct".into()) };
    if crate::systems::stations::precinct_bound(world) >= usize::from(world.config.buildings.jail.capacity) {
        return Err("the Precinct is full".into());
    }
    let until = world.tick + sentence_ticks(world, Crime::Theft);
    sentence(world, who, Crime::Theft, until, jail);
    resolve_reports(world, who);
    world.stats.current.arrests += 1;
    Ok(())
}

/// Player `Release`.
pub fn player_release(world: &mut World, who: EntityId) -> Result<(), String> {
    if !world.has::<Sentence>(who) {
        return Err("not jailed".into());
    }
    release(world, who, false);
    Ok(())
}
