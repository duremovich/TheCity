//! The gang: eligibility, joining, ranks and the leader, the daily stipend,
//! extortion and territory, loot splitting, fencing, disbanding, betrayal.

use crate::components::{
    Brain, Building, BuildingKind, Gang, GangMember, Household, MemoryKind, Needs, Personality, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::time::TICKS_PER_DAY;
use crate::world::World;

/// Eligibility for JoinGang: an edge to any member with affinity >= join_gang_affinity; or
/// `hunger < 0.2 && lawfulness < 0.3`; or the gang is empty and the agent
/// holds a WasArrested memory.
pub fn eligible(world: &World, id: EntityId) -> bool {
    if world.has::<GangMember>(id) {
        return false;
    }
    let Some(gang) = world.gang_id().and_then(|g| world.comp::<Gang>(g)) else { return false };
    let cfg = &world.config.social;
    let contact = gang.members.iter().any(|&m| world.edge(id, m).is_some_and(|e| e.affinity >= cfg.join_gang_affinity));
    let desperate = world.comp::<Needs>(id).is_some_and(|n| n.hunger < cfg.join_gang_desperation_hunger)
        && world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < cfg.join_gang_desperation_lawfulness);
    let bootstrap = gang.members.is_empty()
        && world
            .comp::<crate::components::Memory>(id)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::WasArrested));
    // A gang recruits while it can pay: the treasury must cover a day's stipend
    // for everyone including the recruit. Loot funds growth; a broke gang
    // stops growing. A gang of fewer than three recruits on promise.
    let funded = gang.members.len() < 3 || gang.treasury >= cfg.gang_stipend * (gang.members.len() as i64 + 1);
    bootstrap || ((contact || desperate) && funded)
}

/// JoinGang at the Hideout.
pub fn join(world: &mut World, id: EntityId) -> bool {
    let Some(gang) = world.gang_id() else { return false };
    if !eligible(world, id) {
        return false;
    }
    let tick = world.tick;
    world.insert(id, GangMember { gang, rank: 0, joined_tick: tick });
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if let Err(i) = g.members.binary_search(&id) {
            g.members.insert(i, id);
        }
        g.empty_since = None;
    }
    if let Some(p) = world.comp_mut::<Personality>(id) {
        p.drift(Drift::JoinedGang);
    }
    world.remember(id, MemoryKind::Socialised, None, 0.4, 0.2, false);
    // A recruit who met a current member in jail cites that memory.
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let cites = world.comp::<crate::components::Memory>(id).and_then(|m| {
        m.entries
            .iter()
            .position(|e| e.kind == MemoryKind::MetInJail && e.subject.is_some_and(|s| s != id && members.contains(&s)))
    });
    let name = world.name_of(id);
    let text = match cites {
        Some(n) => format!("{name} joined the gang (cites mem#{n})"),
        None => format!("{name} joined the gang"),
    };
    world.push_event(EventKind::GangJoin, &[id], text);
    recompute_leader(world);
    true
}

/// Leave the gang (betrayal, death handled in kill).
pub fn leave(world: &mut World, id: EntityId, reason: &str) {
    let Some(gm) = world.remove::<GangMember>(id) else { return };
    let tick = world.tick;
    if let Some(g) = world.comp_mut::<Gang>(gm.gang) {
        g.members.retain(|&m| m != id);
        if g.leader == Some(id) {
            g.leader = None;
        }
        if g.members.is_empty() {
            g.empty_since = Some(tick);
        }
    }
    if let Some(p) = world.comp_mut::<Personality>(id) {
        p.drift(Drift::LeftGang);
    }
    let name = world.name_of(id);
    world.push_event(EventKind::GangLeave, &[id], format!("{name} left the gang ({reason})"));
    recompute_leader(world);
}

/// `leader = argmax(loyalty + days_in_gang / 100)`; rank 2, others 0.
pub fn recompute_leader(world: &mut World) {
    let Some(gang) = world.gang_id() else { return };
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let tick = world.tick;
    let leader = members
        .iter()
        .copied()
        .filter(|&m| world.has::<Brain>(m) && !world.has::<crate::components::Sentence>(m))
        .map(|m| {
            let loyalty = world.comp::<Personality>(m).map_or(0.0, |p| p.loyalty);
            let days = world
                .comp::<GangMember>(m)
                .map_or(0.0, |g| tick.saturating_sub(g.joined_tick) as f32 / TICKS_PER_DAY as f32);
            (m, loyalty + days / 100.0)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(m, _)| m);
    for &m in &members {
        if let Some(gm) = world.comp_mut::<GangMember>(m) {
            gm.rank = if Some(m) == leader { 2 } else { 0 };
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.leader = leader;
    }
}

/// The nearest Home not in territory with no guard within 8 tiles of it.
pub fn extort_target(world: &World, id: EntityId) -> Option<EntityId> {
    let gang = world.gang_id()?;
    let territory = world.comp::<Gang>(gang)?.territory.clone();
    let tile = world.comp::<crate::components::Position>(id)?.tile;
    let r = world.config.crime.sight_day_crime;
    let own = world.comp::<Household>(id).and_then(|h| h.home);
    let guards: Vec<crate::components::TilePos> = world
        .citizens()
        .into_iter()
        .filter(|&g| crate::systems::law::is_guard(world, g))
        .filter_map(|g| world.comp::<crate::components::Position>(g).map(|p| p.tile))
        .collect();
    world
        .buildings_by_kind
        .get(&BuildingKind::Home)?
        .iter()
        .copied()
        .filter(|h| !territory.contains(h) && Some(*h) != own)
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && !b.occupants.is_empty())
        .filter(|(_, b)| !guards.iter().any(|&g| crate::systems::law::chebyshev(g, b.door) <= r))
        .min_by_key(|(h, b)| (b.door.manhattan(tile), h.index))
        .map(|(h, _)| h)
}

/// Extort at a Home: take `min(5, occupants' coins)` proportionally into the
/// actor's wallet; the Home's extort_count rises and at 3 it joins the
/// territory. Victims remember WasRobbed and become enemies.
pub fn extort(world: &mut World, actor: EntityId, home: EntityId) -> i64 {
    let amount = world.config.social.extort_amount;
    let occupants: Vec<EntityId> = world
        .comp::<Building>(home)
        .map(|b| b.occupants.iter().copied().filter(|&o| o != actor && world.has::<Wallet>(o)).collect())
        .unwrap_or_default();
    let total: i64 = occupants.iter().map(|&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0))).sum();
    let take = amount.min(total);
    let mut taken = 0;
    for &o in &occupants {
        let coins = world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0));
        let share = if total > 0 { (take * coins) / total } else { 0 };
        if let Some(w) = world.comp_mut::<Wallet>(o) {
            w.coins -= share;
        }
        taken += share;
        world.remember(o, MemoryKind::WasRobbed, Some(actor), 0.7, -0.7, false);
        crate::systems::social::robbed_by(world, o, actor);
    }
    // The rounding remainder comes a coin at a time from whoever still has one.
    let mut guard = 0;
    while taken < take && guard < 64 {
        guard += 1;
        let Some(&payer) = occupants.iter().max_by_key(|&&o| world.comp::<Wallet>(o).map_or(0, |w| w.coins)) else {
            break;
        };
        if world.comp::<Wallet>(payer).is_none_or(|w| w.coins <= 0) {
            break;
        }
        if let Some(w) = world.comp_mut::<Wallet>(payer) {
            w.coins -= 1;
        }
        taken += 1;
    }
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins += taken;
    }
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.loot_today += taken;
        b.gang_task_day = Some(today);
    }
    let mut joined_territory = false;
    if let Some(b) = world.comp_mut::<Building>(home) {
        b.extort_count = b.extort_count.saturating_add(1);
        joined_territory = b.extort_count >= 3;
    }
    if joined_territory {
        if let Some(g) = world.gang_id().and_then(|g| world.comp_mut::<Gang>(g)) {
            if let Err(i) = g.territory.binary_search(&home) {
                g.territory.insert(i, home);
            }
        }
    }
    let name = world.name_of(actor);
    world.push_event(
        EventKind::Extortion,
        &[actor, home],
        format!("{name} extorted {taken} coins from Home#{}", home.index),
    );
    let tile =
        world.comp::<crate::components::Position>(actor).map_or(crate::components::TilePos::default(), |p| p.tile);
    crate::systems::law::raise_crime(world, actor, None, crate::components::Crime::Extortion, tile);
    taken
}

/// SplitLoot at the Hideout: half the day's haul into the gang treasury.
pub fn split_loot(world: &mut World, actor: EntityId) -> i64 {
    let loot = world.comp::<Brain>(actor).map_or(0, |b| b.loot_today);
    let share = (loot as f32 * 0.5).floor() as i64;
    let share = share.min(world.comp::<Wallet>(actor).map_or(0, |w| w.coins)).max(0);
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins -= share;
    }
    if let Some(g) = world.gang_id().and_then(|g| world.comp_mut::<Gang>(g)) {
        g.treasury += share;
    }
    if let Some(b) = world.comp_mut::<Brain>(actor) {
        b.loot_today = 0;
    }
    world.remember(actor, MemoryKind::Paid, None, 0.2, 0.2, false);
    share
}

/// Fence at the Hideout: stolen food sold to the gang at floor(price × 0.8)
/// each, paid from the gang treasury.
pub fn fence(world: &mut World, actor: EntityId) -> i64 {
    let price = world.market().map_or(0, |m| m.price_food);
    let each = (price as f32 * 0.8).floor() as i64;
    let units = world.comp::<crate::components::Inventory>(actor).map_or(0, |i| i.stolen_food);
    let treasury = world.gang_id().and_then(|g| world.comp::<Gang>(g)).map_or(0, |g| g.treasury);
    let affordable = if each > 0 { (treasury / each).max(0) as u32 } else { units };
    let sold = units.min(affordable);
    if sold == 0 {
        return 0;
    }
    let pay = each * i64::from(sold);
    if let Some(i) = world.comp_mut::<crate::components::Inventory>(actor) {
        i.food -= sold;
        i.stolen_food -= sold;
    }
    if let Some(g) = world.gang_id().and_then(|g| world.comp_mut::<Gang>(g)) {
        g.treasury -= pay;
        g.treasury += i64::from(sold) * price; // the gang resells at market price
    }
    if let Some(w) = world.comp_mut::<Wallet>(actor) {
        w.coins += pay;
    }
    let cap = world.config.buildings.hideout.stock_cap;
    if let Some(m) = world.building_of_kind(BuildingKind::Hideout).and_then(|h| world.comp_mut::<Building>(h)) {
        m.stock_food = (m.stock_food + sold).min(cap);
    }
    pay
}

/// Daily: ranks, stipend, territory tribute, disbanding, betrayal.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    let Some(gang) = world.gang_id() else { return };
    recompute_leader(world);
    // Task flags reset daily.
    let today = world.day();
    for id in world.citizens() {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            if b.gang_task_day != Some(today) {
                b.loot_today = 0;
            }
        }
    }
    // Territory tribute: 2 coins/day per Home when the occupants can pay.
    let territory = world.comp::<Gang>(gang).map(|g| g.territory.clone()).unwrap_or_default();
    for home in territory {
        let residents: Vec<EntityId> = world
            .citizens()
            .into_iter()
            .filter(|&c| world.comp::<Household>(c).and_then(|h| h.home) == Some(home))
            .collect();
        let mut owed = 2;
        for r in residents {
            if owed == 0 {
                break;
            }
            let coins = world.comp::<Wallet>(r).map_or(0, |w| w.coins);
            let pay = owed.min(coins.max(0));
            if pay > 0 {
                if let Some(w) = world.comp_mut::<Wallet>(r) {
                    w.coins -= pay;
                }
                owed -= pay;
            }
        }
        let paid = 2 - owed;
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.treasury += paid;
        }
    }
    // Stipend: 4 coins each while the treasury holds >= 4, leader first.
    let stipend = world.config.social.gang_stipend;
    let (mut members, leader) = world.comp::<Gang>(gang).map(|g| (g.members.clone(), g.leader)).unwrap_or_default();
    if let Some(l) = leader {
        members.retain(|&m| m != l);
        members.insert(0, l);
    }
    for m in members {
        let can = world.comp::<Gang>(gang).is_some_and(|g| g.treasury >= stipend);
        if !can {
            break;
        }
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.treasury -= stipend;
        }
        if let Some(w) = world.comp_mut::<Wallet>(m) {
            w.coins += stipend;
        }
    }
    // Disband after 30 days with no members: treasury and territory go, the name stays.
    let empty_since = world.comp::<Gang>(gang).and_then(|g| if g.members.is_empty() { g.empty_since } else { None });
    if let Some(since) = empty_since {
        let now = world.tick;
        if now.saturating_sub(since) >= 30 * TICKS_PER_DAY {
            if let Some(g) = world.comp_mut::<Gang>(gang) {
                g.treasury = 0;
                g.territory.clear();
                g.empty_since = Some(now);
            }
        }
    }
    // Betrayal: a member with loyalty < 0.3 and an open warrant on themselves.
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    for m in members {
        let disloyal = world.comp::<Personality>(m).is_some_and(|p| p.loyalty < 0.3);
        let wanted = crate::systems::law::wanted(world, m);
        if let Some(b) = world.comp_mut::<Brain>(m) {
            b.betraying = disloyal && wanted;
        }
    }
}

/// A betrayer filed a report naming the leader: they leave, every remaining
/// member becomes their enemy, and their own warrant is resolved.
pub fn betrayal_filed(world: &mut World, betrayer: EntityId) {
    let Some(gang) = world.gang_id() else { return };
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    leave(world, betrayer, "betrayal");
    for m in members.into_iter().filter(|&m| m != betrayer) {
        let tick = world.tick;
        let e = world.edge_entry(m, betrayer);
        e.affinity = (e.affinity - 0.7).max(-1.0);
        e.kind = crate::components::RelKind::Enemy;
        e.last_interaction = tick;
        crate::systems::social::reindex_kind(world, m, betrayer);
    }
    for r in world.crime_reports.iter_mut().filter(|r| r.suspect == betrayer) {
        r.resolved = true;
    }
    if let Some(b) = world.comp_mut::<Brain>(betrayer) {
        b.betraying = false;
    }
    let name = world.name_of(betrayer);
    world.push_event(EventKind::Betrayal, &[betrayer], format!("{name} betrayed the gang"));
}
