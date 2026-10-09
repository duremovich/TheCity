//! The Real economy phase 3c (docs/ECONOMY_V2.md addendum; roadmap
//! addendum 19; plan E37-E42): the child protective service and the work
//! camps. A daily pass moves a child whose `Child.hunger_days` reached
//! `take_days` to a `BuildingKind::Camp` (its `Household.home`), the camp
//! feeds the children it houses from food its owner buys, a daily ledger
//! shift turns the older children's days into Parts the owner sells, a camp
//! that cannot feed is a Feed story and the law closes it, and a child who
//! comes of age there leaves with the `CampRaised` trait.
//!
//! Every "take", "shift", "scandal" and "release" here is a counter, a list
//! membership and coins moved between integer purses, nothing more.

use crate::components::{
    Building, BuildingKind, Child, Corp, DeathCause, Household, Identity, MemoryKind, Niche, Personality, Position,
    Role, Skills, TilePos, Zone,
};
use crate::econ::{CampRaised, CampState, RING_DAYS};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::demography::CHILD_FOOD_PER_DAY;
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::word::GrudgeCause;
use crate::world::World;

/// E1: `[living] enabled && [economy2] enabled && [camp] enabled`.
pub fn on(world: &World) -> bool {
    world.config.living.enabled && world.config.economy2.enabled && world.config.camp.enabled
}

/// Why a child leaves a camp (`release`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ReleaseWhy {
    /// Came of age: the `CampRaised` trait.
    Adult,
    /// M16b's Rescue (the hook): out of the list, nothing else.
    Rescue,
}

// ---------------------------------------------------------------------------
// Camps
// ---------------------------------------------------------------------------

/// Every standing Camp (not demolished, with a `CampState`), ascending.
pub fn standing_camps(world: &World) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Camp)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.camp.is_some() && !bd.demolished))
        .collect()
}

fn state(world: &World, camp: EntityId) -> Option<&CampState> {
    world.comp::<Building>(camp).and_then(|bd| bd.camp.as_ref())
}

fn state_mut(world: &mut World, camp: EntityId) -> Option<&mut CampState> {
    world.comp_mut::<Building>(camp).and_then(|bd| bd.camp.as_mut())
}

/// E42 (the Rescue hook): the children housed at `camp`.
pub fn children_of(world: &World, camp: EntityId) -> &[EntityId] {
    state(world, camp).map_or(&[], |s| s.children.as_slice())
}

/// Is the camp open to a take (standing, not closed by the law)?
fn open(world: &World, camp: EntityId) -> bool {
    state(world, camp).is_some_and(|s| !s.closed_by_law) && !world.is_closed(camp)
}

fn is_city(world: &World, camp: EntityId) -> bool {
    world.owner_of(camp).is_none()
}

fn capacity(world: &World, camp: EntityId) -> usize {
    world.comp::<Building>(camp).map_or(0, |bd| usize::from(bd.capacity))
}

fn door(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

/// Drop dead or despawned children from a camp's lists.
fn prune(world: &mut World, camp: EntityId) {
    let alive: Vec<EntityId> =
        children_of(world, camp).iter().copied().filter(|&c| world.is_alive(c) && world.has::<Child>(c)).collect();
    if let Some(s) = state_mut(world, camp) {
        if s.children.len() != alive.len() {
            s.children = alive.clone();
            s.arrived.retain(|(c, _)| alive.contains(c));
            s.fed_by_child.retain(|(c, _)| alive.contains(c));
        }
    }
}

// ---------------------------------------------------------------------------
// Seeding and founding (E37)
// ---------------------------------------------------------------------------

/// The centroid of the Vats district (the first), else the map's middle.
fn vats_centroid(world: &World) -> TilePos {
    world
        .districts
        .iter()
        .find(|d| d.zone == Zone::Vats)
        .map_or_else(|| TilePos { x: (world.map.w() / 2) as u8, y: (world.map.h() / 2) as u8 }, |d| d.centroid)
}

/// A vacant Lot nearest `from` (a Vats Lot first), else a derelict Block
/// nearest it: `(site, on_lot)`.
fn site_near(world: &World, from: TilePos) -> Option<(EntityId, bool)> {
    let lots = crate::systems::founding::vacant_lots(world);
    let pick = |only_vats: bool| {
        lots.iter()
            .copied()
            .filter(|&l| !only_vats || world.comp::<Building>(l).is_some_and(|b| world.map.zone(b.door) == Zone::Vats))
            .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(from), l)))
            .min()
            .map(|(_, l)| l)
    };
    if let Some(l) = pick(true).or_else(|| pick(false)) {
        return Some((l, true));
    }
    crate::systems::street::derelicts(world)
        .into_iter()
        .filter_map(|x| {
            let bd = world.comp::<Building>(x).filter(|bd| bd.kind == BuildingKind::Home && !bd.demolished)?;
            Some((bd.door.manhattan(from), x))
        })
        .min()
        .map(|(_, x)| (x, false))
}

fn open_camp(world: &mut World, site: EntityId, on_lot: bool, owner: Option<EntityId>) -> Option<EntityId> {
    let made = if on_lot {
        crate::systems::founding::build_on_lot(world, site, BuildingKind::Camp, owner)
    } else {
        crate::systems::founding::refit_with(world, site, BuildingKind::Camp, owner, false)
    };
    let b = made.ok()?;
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.camp = Some(CampState::default());
    }
    Some(b)
}

/// `founding::convert`'s hook: a Camp opens with its state.
pub fn on_convert(world: &mut World, b: EntityId, kind: BuildingKind) {
    if kind == BuildingKind::Camp {
        if let Some(bd) = world.comp_mut::<Building>(b) {
            if bd.camp.is_none() {
                bd.camp = Some(CampState::default());
            }
        }
    }
}

/// E37 (`World::new`, after the Missions; no RNG): the service's own Camp
/// on the vacant Lot nearest the Vats (a Vats Lot first), else a derelict
/// refitted free, on the city's deed.
pub fn seed(world: &mut World) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let from = vats_centroid(world);
    let (site, on_lot) = site_near(world, from)?;
    let b = open_camp(world, site, on_lot, None)?;
    let at = world.district_name(world.district_of_building(b)).to_string();
    let how = if on_lot { "Lot" } else { "refit" };
    let text = format!("the city opened a Work Camp in {at} (seeded; the service's own; {how} {})", b.index);
    world.push_event(EventKind::Founded, &[b], text);
    Some(b)
}

/// E37 (midnight): when every standing Camp is at least `found_full` full,
/// or a taken child had no place in the last 7 days, the richest Tech or
/// Food corp with `3 × found_cost.camp` builds one (a Lot near its
/// buildings, else a derelict); not a corp order.
pub fn found_daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let camps = standing_camps(world);
    let full = world.config.camp.found_full;
    let all_full = !camps.is_empty()
        && camps.iter().all(|&c| {
            let cap = capacity(world, c).max(1) as f32;
            children_of(world, c).len() as f32 >= full * cap
        });
    let day = world.day();
    let overflow = camps
        .iter()
        .any(|&c| state(world, c).and_then(|s| s.last_overflow_day).is_some_and(|d| day.saturating_sub(d) <= 7));
    if !all_full && !overflow {
        return;
    }
    let cost = world.config.corps.found_cost.camp;
    let pick = world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (cc.treasury, c, cc)))
        .filter(|(t, _, cc)| *t >= 3 * cost && (cc.niches.contains(&Niche::Tech) || cc.niches.contains(&Niche::Food)))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, c, _)| c);
    let Some(corp) = pick else { return };
    let from = world
        .comp::<Corp>(corp)
        .and_then(|c| c.buildings.first().copied())
        .and_then(|b| door(world, b))
        .unwrap_or_else(|| vats_centroid(world));
    let Some((site, on_lot)) = site_near(world, from) else { return };
    let Some(b) = open_camp(world, site, on_lot, Some(corp)) else { return };
    let paid = ownership::pay(world, Some(corp), None, cost, Flow::Found);
    let (who, at) = (world.owner_label(Some(corp)), world.district_name(world.district_of_building(b)).to_string());
    world.push_event(
        EventKind::Founded,
        &[corp, b],
        format!("{who} built a Work Camp in {at} for {paid} (a work camp)"),
    );
    world.stats.current.foundings += 1;
}

// ---------------------------------------------------------------------------
// The service (E38)
// ---------------------------------------------------------------------------

/// The living parents of `child` (Parent edges to someone born earlier), ascending.
pub fn parents_of(world: &World, child: EntityId) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = world
        .neighbours(child)
        .filter(|&o| world.has::<Identity>(o) && crate::systems::law::living(world, o))
        .filter(|&o| crate::systems::demography::is_child_of(world, child, o))
        .collect();
    out.sort_unstable();
    out
}

/// The standing Market nearest `from` with at least `units` on its shelf
/// (ties the lower id); the city's kitchens buy from any Market with stock.
fn market_with_stock(world: &World, from: TilePos, units: u32) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter_map(|m| {
            let bd = world.comp::<Building>(m).filter(|bd| !bd.demolished && !bd.derelict)?;
            (bd.stock_food >= units.max(1)).then(|| (bd.door.manhattan(from), m))
        })
        .min()
        .map(|(_, m)| m)
}

/// Review fix (E38, E39): can `camp` feed its children plus `extra`
/// tonight: the shelf covers their rations, or the owner can buy the
/// shortfall (the city's purse is unbounded) from a Market with stock.
pub fn can_feed_tonight(world: &World, camp: EntityId, extra: usize) -> bool {
    let n = (children_of(world, camp).len() + extra) as u32;
    let need = n.div_ceil(2);
    let stock = world.comp::<Building>(camp).map_or(0, |b| b.stock_food);
    if stock >= need {
        return true;
    }
    let short = need - stock;
    let price = world.config.corps.wholesale.max(1);
    let owner = world.owner_of(camp);
    let afford = match ownership::owner_kind(world, owner) {
        ownership::OwnerKind::City => true,
        _ => world.purse(owner).max(0) >= i64::from(short) * price,
    };
    afford && door(world, camp).is_some_and(|d| market_with_stock(world, d, short).is_some())
}

/// The camp a take goes to: among the open camps with room, one that can
/// feed tonight first (review fix: a taken child arrives at `take_days`
/// unfed and the third unfed day kills, before the law's closure), then
/// corp camps before the city's, then the nearest (ties the lower id);
/// else the city camp over capacity (the first camp when none is the
/// city's); `(camp, over)`.
fn placement(world: &World, from: TilePos) -> Option<(EntityId, bool)> {
    let camps = standing_camps(world);
    let with_room = camps
        .iter()
        .copied()
        .filter(|&c| open(world, c) && children_of(world, c).len() < capacity(world, c))
        .filter_map(|c| {
            door(world, c).map(|d| (!can_feed_tonight(world, c, 1), is_city(world, c), d.manhattan(from), c))
        })
        .min()
        .map(|(_, _, _, c)| c);
    if let Some(c) = with_room {
        return Some((c, false));
    }
    let city = camps.iter().copied().filter(|&c| open(world, c)).find(|&c| is_city(world, c));
    city.or_else(|| camps.iter().copied().find(|&c| open(world, c))).map(|c| (c, true))
}

/// E38: a child moves into `camp`: its Household, the camp's lists, its
/// position inside.
fn house(world: &mut World, child: EntityId, camp: EntityId) {
    let day = world.day();
    world.set_home(child, Some(camp));
    world.remove_from_building(child);
    let tick = world.tick;
    let d = door(world, camp).unwrap_or_default();
    if let Some(p) = world.comp_mut::<Position>(child) {
        p.tile = d;
        p.building = Some(camp);
        p.entered = tick;
    }
    if let Some(s) = state_mut(world, camp) {
        if let Err(i) = s.children.binary_search(&child) {
            s.children.insert(i, child);
        }
        if !s.arrived.iter().any(|(c, _)| *c == child) {
            s.arrived.push((child, day));
        }
        if !s.fed_by_child.iter().any(|(c, _)| *c == child) {
            s.fed_by_child.push((child, 0));
        }
    }
}

/// Out of the camp's lists (the Household is the caller's).
fn unhouse(world: &mut World, child: EntityId, camp: EntityId) -> (u32, u32) {
    let day = world.day();
    let mut days = 0;
    let mut fed = 0;
    if let Some(s) = state_mut(world, camp) {
        s.children.retain(|&c| c != child);
        if let Some(i) = s.arrived.iter().position(|(c, _)| *c == child) {
            days = day.saturating_sub(s.arrived[i].1) as u32;
            s.arrived.remove(i);
        }
        if let Some(i) = s.fed_by_child.iter().position(|(c, _)| *c == child) {
            fed = s.fed_by_child[i].1;
            s.fed_by_child.remove(i);
        }
    }
    (days, fed)
}

/// E38 (`demography::run`, after `children`): every `Child` in a Block
/// with `hunger_days ≥ take_days` is taken, ascending id.
pub fn take_daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let take_days = world.config.camp.take_days;
    for c in standing_camps(world) {
        prune(world, c);
    }
    let mut due: Vec<EntityId> = world
        .with::<Child>()
        .into_iter()
        .filter(|&c| world.comp::<Child>(c).is_some_and(|k| k.hunger_days >= take_days))
        .filter(|&c| {
            world
                .comp::<Household>(c)
                .and_then(|h| h.home)
                .is_none_or(|h| world.comp::<Building>(h).is_some_and(|b| b.kind != BuildingKind::Camp))
        })
        .collect();
    due.sort_unstable();
    for c in due {
        take(world, c);
    }
}

/// The take: the camp with room nearest the child's Home (custody never
/// refuses: the city camp over capacity), the Household moved,
/// `ChildTaken` for the child and each living parent (a Grief-class
/// `ChildTaken` memory, no deed; a grudge on the Law), a Feed bulletin.
pub fn take(world: &mut World, child: EntityId) -> Option<EntityId> {
    if !on(world) || !world.has::<Child>(child) {
        return None;
    }
    let home = world.comp::<Household>(child).and_then(|h| h.home);
    let from =
        home.and_then(|h| door(world, h)).or_else(|| world.comp::<Position>(child).map(|p| p.tile)).unwrap_or_default();
    let (camp, over) = placement(world, from)?;
    let day = world.day();
    if over {
        if let Some(s) = state_mut(world, camp) {
            s.last_overflow_day = Some(day);
        }
    }
    let parents = parents_of(world, child);
    house(world, child, camp);
    let (name, what) = (world.name_of(child), world.name_of(camp));
    let from_home = home.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
    let mut actors = vec![child];
    actors.extend(parents.iter().copied());
    world.push_event(
        EventKind::ChildTaken,
        &actors,
        format!("the child protective service took {name} from {from_home} to {what}"),
    );
    world.stats.current.econ.children_taken += 1;
    let law = world.building_of_kind(BuildingKind::Jail).filter(|&j| world.has::<crate::components::Law>(j));
    for p in parents {
        world.remember(p, MemoryKind::ChildTaken, Some(child), 0.9, -0.9, false);
        if let Some(l) = law {
            crate::systems::grudges::add(world, p, l, GrudgeCause::ChildTaken, 0.6, 0);
        }
    }
    crate::systems::news::bulletin(world, &[child], format!("the service took a child from {from_home}"));
    Some(camp)
}

// ---------------------------------------------------------------------------
// The camp day (E39, E40, E42)
// ---------------------------------------------------------------------------

/// Midnight (from `living::run`, before `demography`): per standing camp,
/// the restock (the owner's purse, the Treasury for the city's, `Flow::CampFood`),
/// the meals (`CHILD_FOOD_PER_DAY` a child; an unfed day counts as at
/// home and the third kills), the rings, the scandal and the closure, the
/// shift (Parts from the older children's days), the reopening.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let now = world.tick;
    for camp in standing_camps(world) {
        prune(world, camp);
        if let Some(s) = state_mut(world, camp) {
            s.days_open += 1;
            s.unfed_today = 0;
            s.output_today = 0;
        }
        reopen(world, camp, now);
        restock(world, camp);
        feed(world, camp);
        shift(world, camp);
        scandal(world, camp);
    }
    let housed: u32 = standing_camps(world).iter().map(|&c| children_of(world, c).len() as u32).sum();
    world.stats.current.econ.camp_children = housed;
}

fn reopen(world: &mut World, camp: EntityId, now: u64) {
    let closed = state(world, camp).is_some_and(|s| s.closed_by_law);
    let until = world.comp::<Building>(camp).and_then(|b| b.closed_until);
    if closed && until.is_none_or(|t| t <= now) {
        if let Some(s) = state_mut(world, camp) {
            s.closed_by_law = false;
            s.scandal_days = 0;
        }
    }
}

/// E39: the owner buys from the nearest Market at `wholesale` up to
/// `stock_cap` and its purse; the city's Camp from the Treasury
/// (`Flow::CampFood`, the Jail meal's shape).
fn restock(world: &mut World, camp: EntityId) {
    let n = children_of(world, camp).len() as u32;
    if n == 0 {
        return;
    }
    let Some(cd) = door(world, camp) else { return };
    let cap = world.config.buildings.for_kind(BuildingKind::Camp).stock_cap;
    let stock = world.comp::<Building>(camp).map_or(0, |b| b.stock_food);
    // A week of meals is enough; the cap bounds it.
    let want = (n.div_ceil(2) * 7).min(cap).saturating_sub(stock);
    if want == 0 {
        return;
    }
    // Review fix: the nearest Market with stock (an empty shelf next door
    // is skipped; nothing in the city: nothing bought, no `CampFood` line).
    let Some(market) = market_with_stock(world, cd, 1) else { return };
    let price = world.config.corps.wholesale.max(1);
    let have = world.comp::<Building>(market).map_or(0, |b| b.stock_food);
    let owner = world.owner_of(camp);
    let afford = match ownership::owner_kind(world, owner) {
        ownership::OwnerKind::City => u32::MAX,
        _ => u32::try_from(world.purse(owner).max(0) / price).unwrap_or(u32::MAX),
    };
    let units = want.min(have).min(afford);
    if units == 0 {
        return;
    }
    let seller = world.owner_of(market);
    let amount = i64::from(units) * price;
    let paid = if owner.is_none() {
        let paid = ownership::charge(world, None, seller, amount, Flow::CampFood);
        if seller.is_none() {
            // The city buying from its own Market: no coin moves, the line shows.
            ownership::ledger_only(world, Flow::CampFood, amount);
            amount
        } else {
            paid
        }
    } else {
        ownership::pay(world, owner, seller, amount, Flow::Wholesale)
    };
    if paid <= 0 {
        return;
    }
    let bought = u32::try_from(paid / price).unwrap_or(0).min(units);
    world.take_stock(market, crate::components::Good::Food, bought);
    ownership::credit(world, market, paid);
    if let Some(b) = world.comp_mut::<Building>(camp) {
        b.stock_food += bought;
    }
}

/// E39: each child eats `CHILD_FOOD_PER_DAY` from the camp's stock (the
/// Home's debt shape); the children the stock could not cover go unfed
/// (ascending id), `hunger_days` as at home, the third day kills.
fn feed(world: &mut World, camp: EntityId) {
    let kids = children_of(world, camp).to_vec();
    if kids.is_empty() {
        return;
    }
    let (needed, take) = {
        let Some(b) = world.comp_mut::<Building>(camp) else { return };
        b.child_food_debt += CHILD_FOOD_PER_DAY * kids.len() as f32;
        let needed = b.child_food_debt.floor() as u32;
        let take = needed.min(b.stock_food);
        b.stock_food -= take;
        b.child_food_debt -= take as f32;
        (needed, take)
    };
    let fed_n = if take == needed { kids.len() } else { (take as usize * 2).min(kids.len()) };
    let mut starved = Vec::new();
    for (i, &k) in kids.iter().enumerate() {
        let fed = i < fed_n;
        let Some(c) = world.comp_mut::<Child>(k) else { continue };
        if fed {
            c.hunger_days = 0;
        } else {
            c.hunger_days = c.hunger_days.saturating_add(1);
            if c.hunger_days >= 3 {
                starved.push(k);
            }
        }
        if let Some(s) = state_mut(world, camp) {
            if fed {
                if let Some(e) = s.fed_by_child.iter_mut().find(|(c, _)| *c == k) {
                    e.1 += 1;
                }
            } else {
                s.unfed_today = s.unfed_today.saturating_add(1);
            }
        }
    }
    let unfed = state(world, camp).map_or(0, |s| s.unfed_today);
    if let Some(s) = state_mut(world, camp) {
        s.fed_days.push_back(u8::from(unfed == 0));
        while s.fed_days.len() > RING_DAYS {
            s.fed_days.pop_front();
        }
        s.unfed_days.push_back(unfed);
        while s.unfed_days.len() > RING_DAYS {
            s.unfed_days.pop_front();
        }
    }
    world.stats.current.econ.camp_unfed += u32::from(unfed);
    for k in starved {
        // Review fix: a child the service could not feed is a scandal
        // naming the service (no Treasury-to-pantry path exists to save it).
        let (name, what) = (world.name_of(k), world.name_of(camp));
        crate::systems::news::bulletin(
            world,
            &[k, camp],
            format!("the child protective service could not feed {name} at {what}, who starved"),
        );
        world.kill(k, DeathCause::Starvation);
    }
}

/// E40: the shift is a daily ledger (children have no Brain): per child
/// older than `work_age`, `camp_yield × (floor + slope × farming)` Parts
/// into the camp's accumulator, whole Parts into its stock, `camp_skill_gain`
/// to the child's farming. (Phase 2's `input_per_part` to the World applies
/// at its merge.)
fn shift(world: &mut World, camp: EntityId) {
    let cfg = world.config.camp.clone();
    let kids = children_of(world, camp).to_vec();
    let mut accum = 0.0f32;
    for k in kids {
        let years = world.comp::<Identity>(k).map_or(0, |i| i.age_years());
        if years < cfg.work_age {
            continue;
        }
        let farming = world.comp::<Skills>(k).map_or(0.0, |s| s.farming);
        accum += cfg.camp_yield * (cfg.camp_skill_floor + cfg.camp_skill_slope * farming);
        if let Some(s) = world.comp_mut::<Skills>(k) {
            s.farming = (s.farming + cfg.camp_skill_gain).min(1.0);
        }
    }
    if accum <= 0.0 {
        return;
    }
    let whole = {
        let Some(b) = world.comp_mut::<Building>(camp) else { return };
        b.production_accum += accum;
        let whole = b.production_accum.floor() as u32;
        b.production_accum -= whole as f32;
        whole
    };
    if whole == 0 {
        return;
    }
    let made = world.add_stock(camp, crate::components::Good::Parts, whole);
    if let Some(s) = state_mut(world, camp) {
        s.output_today += made;
    }
    world.stats.current.econ.camp_output += made;
}

/// E42: an unfed day adds a scandal day; the first is a Feed story and a
/// standing hit (read at the reputation rebuild); at `close_days` the law
/// closes the camp for `close_ban_days` and its children move.
fn scandal(world: &mut World, camp: EntityId) {
    let unfed = state(world, camp).map_or(0, |s| s.unfed_today);
    if unfed == 0 {
        if let Some(s) = state_mut(world, camp) {
            if !s.closed_by_law {
                s.scandal_days = 0;
            }
        }
        return;
    }
    let days = {
        let Some(s) = state_mut(world, camp) else { return };
        s.scandal_days = s.scandal_days.saturating_add(1);
        s.scandal_days
    };
    world.stats.current.econ.camp_scandals += 1;
    let owner = world.owner_of(camp);
    let (what, who) = (world.name_of(camp), world.owner_label(owner));
    // Deviation: the third unfed day kills (as at home), so the law closes
    // the camp before it: at `close_days − 1` unfed days (2 of the plan's
    // 3) the children move to the city's camp and eat there on the third.
    let close_days = world.config.camp.close_days.saturating_sub(1).max(1);
    if days == 1 || days == close_days {
        let mut actors = vec![camp];
        actors.extend(owner);
        crate::systems::news::bulletin(
            world,
            &actors,
            format!("{what} ({who}) could not feed {unfed} of its children"),
        );
    }
    if days >= close_days && state(world, camp).is_some_and(|s| !s.closed_by_law) {
        close(world, camp);
    }
}

/// E42: the law closes `camp`: `closed_by_law`, `closed_until`, its
/// children moved to the nearest other open camp (the city's last, over
/// capacity), `CampClosed`.
pub fn close(world: &mut World, camp: EntityId) {
    let ban = u64::from(world.config.camp.close_ban_days) * TICKS_PER_DAY;
    let now = world.tick;
    if let Some(s) = state_mut(world, camp) {
        s.closed_by_law = true;
    }
    if let Some(b) = world.comp_mut::<Building>(camp) {
        b.closed_until = Some(now + ban);
    }
    let owner = world.owner_of(camp);
    let kids = children_of(world, camp).to_vec();
    let from = door(world, camp).unwrap_or_default();
    let mut moved = 0;
    for k in kids {
        let Some((to, _)) = placement(world, from) else { break };
        if to == camp {
            break;
        }
        unhouse(world, k, camp);
        house(world, k, to);
        moved += 1;
    }
    let (what, who) = (world.name_of(camp), world.owner_label(owner));
    let mut actors = vec![camp];
    actors.extend(owner);
    world.push_event(EventKind::CampClosed, &actors, format!("the law closed {what} ({who}); {moved} children moved"));
}

/// E42: the owner's standing hit, `scandal_standing × unfed child-days in
/// the last 30 days` over its camps (negative; read at the reputation
/// rebuild: the store is rebuilt nightly, so a direct write would not last).
pub fn scandal_penalty(world: &World, owner: Option<EntityId>) -> f32 {
    if !on(world) {
        return 0.0;
    }
    let per = world.config.camp.scandal_standing;
    standing_camps(world)
        .into_iter()
        .filter(|&c| world.owner_of(c) == owner)
        .map(|c| state(world, c).map_or(0, |s| s.unfed_days.iter().map(|&u| u32::from(u)).sum::<u32>()))
        .sum::<u32>() as f32
        * per
}

// ---------------------------------------------------------------------------
// Release (E41)
// ---------------------------------------------------------------------------

/// E41 (`demography::mature`, after the adult's components are in): a
/// child whose home is a Camp is released: `true` when it was one.
pub fn on_mature(world: &mut World, id: EntityId) -> bool {
    let camp = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp));
    let Some(camp) = camp else { return false };
    release(world, id, ReleaseWhy::Adult);
    let _ = camp;
    true
}

/// E41, E42 (the Rescue hook): `id` leaves its camp: out of the lists,
/// homeless at the camp's door. `Adult`: the `CampRaised` trait, the
/// family's affinity, the loyalty or the grudge by the fed share, the
/// `CampRaised` event.
pub fn release(world: &mut World, id: EntityId, why: ReleaseWhy) {
    let Some(camp) = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp))
    else {
        return;
    };
    let (days, fed) = unhouse(world, id, camp);
    world.set_home(id, None);
    let now = world.tick;
    if let Some(h) = world.comp_mut::<Household>(id) {
        h.homeless_since = Some(now);
    }
    world.stand_at_door(id, camp);
    if why != ReleaseWhy::Adult {
        return;
    }
    let owner = world.owner_of(camp);
    let share = if days == 0 { 1.0 } else { (fed as f32 / days as f32).clamp(0.0, 1.0) };
    world.insert(id, CampRaised { camp, owner, days, fed_share: share });
    let cfg = world.config.camp.clone();
    let tick = world.tick;
    for p in parents_of(world, id) {
        let e = world.edge_entry(p, id);
        e.affinity = cfg.family_affinity;
        e.last_interaction = tick;
    }
    if share >= cfg.loyal_share {
        if let Some(pers) = world.comp_mut::<Personality>(id) {
            pers.loyalty = (pers.loyalty + 0.1).min(1.0);
        }
        if let Some(exec) = owner.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.exec) {
            let e = world.edge_entry(id, exec);
            e.affinity = (e.affinity + 0.2).clamp(-1.0, 1.0);
            e.last_interaction = tick;
        }
    } else if share < cfg.grudge_share {
        let target = owner
            .or_else(|| world.building_of_kind(BuildingKind::Jail).filter(|&j| world.has::<crate::components::Law>(j)));
        if let Some(t) = target {
            crate::systems::grudges::add(world, id, t, GrudgeCause::ChildTaken, 0.6, 0);
        }
    }
    let (name, what) = (world.name_of(id), world.name_of(camp));
    world.push_event(
        EventKind::CampRaised,
        &[id, camp],
        format!("{name} came of age at {what} after {days} days (fed {:.0} %)", share * 100.0),
    );
    world.stats.current.econ.camp_released += 1;
}

/// E41: a `CampRaised` applicant comes first for a Farm or Fab vacancy
/// (`demography::pick_candidate`'s key).
pub fn applicant_first(world: &World, id: EntityId, role: Role) -> bool {
    on(world) && matches!(role, Role::Farmer | Role::Fabber) && world.has::<CampRaised>(id)
}
