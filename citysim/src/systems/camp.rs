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
    Building, BuildingKind, Child, Corp, Household, Identity, MemoryKind, Niche, Personality, Position, Role, Skills,
    TilePos, Zone,
};
use crate::econ::{CampMove, CampRaised, CampState, RING_DAYS};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::demography::CHILD_FOOD_PER_DAY;
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::word::GrudgeCause;
use crate::world::World;

/// Why a child leaves a camp (`release`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ReleaseWhy {
    /// Came of age: the `CampRaised` trait.
    Adult,
    /// M16b's Rescue (the hook): out of the list, nothing else.
    Rescue,
    /// Lives at work phase 3 (§ 2.2): slipped out (`p_camp_escape`).
    Escaped,
    /// Lives at work phase 3 (§ 3.5): bought out by a parent.
    BoughtOut,
    /// Round 4 T1 (OPEN-231): broken out by a parent (was `BoughtOut`).
    BrokenOut,
    /// Round 4 T1: let home by the service (the parent's pantry held).
    LetHome,
}

impl ReleaseWhy {
    fn camp_move(self) -> CampMove {
        match self {
            ReleaseWhy::Adult => CampMove::CameOfAge,
            ReleaseWhy::Rescue | ReleaseWhy::BrokenOut => CampMove::BrokenOut,
            ReleaseWhy::Escaped => CampMove::Escaped,
            ReleaseWhy::BoughtOut => CampMove::BoughtOut,
            ReleaseWhy::LetHome => CampMove::LetHome,
        }
    }
}

/// Lives at work phase 3: a Camp child-day's escape roll.
const PURPOSE_ESCAPE: u64 = 0x4341_0002;
/// Round 4 T1 (OPEN-231): the day's try at the fence, its hour, the catch,
/// a warned parent's Friends' ask.
const PURPOSE_ABSCOND: u64 = 0x4341_0003;
const PURPOSE_ABSCOND_HOUR: u64 = 0x4341_0004;
const PURPOSE_CAUGHT: u64 = 0x4341_0005;
const PURPOSE_WARN_ASK: u64 = 0x4341_0006;

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
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

/// Round 4 T1 (OPEN-231): a coming or going for the warden's next
/// headcount (the last `MOVES_KEPT`).
fn note_move(world: &mut World, camp: EntityId, child: EntityId, m: CampMove) {
    if let Some(s) = state_mut(world, camp) {
        s.since_count.push((child, m));
        if s.since_count.len() > MOVES_KEPT {
            s.since_count.remove(0);
        }
    }
}

/// The comings and goings a Camp keeps between two headcounts.
const MOVES_KEPT: usize = 64;

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
        .map_or_else(|| TilePos { x: (world.map.w() / 2) as u16, y: (world.map.h() / 2) as u16 }, |d| d.centroid)
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
    let paid = crate::systems::industry::pay_build(world, Some(corp), cost, Some(b), true);
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
/// unfed; S6: hungry, never killed), then
/// corp camps before the city's, then the nearest (ties the lower id);
/// else the city camp over capacity (the first camp when none is the
/// city's); `(camp, over)`.
fn placement(world: &World, from: TilePos, parent: Option<EntityId>) -> Option<(EntityId, bool)> {
    let camps = standing_camps(world);
    let with_room = camps
        .iter()
        .copied()
        .filter(|&c| open(world, c) && children_of(world, c).len() < capacity(world, c))
        .filter_map(|c| {
            door(world, c).map(|d| {
                (
                    !can_feed_tonight(world, c, 1),
                    !in_reach(world, parent, from, d),
                    is_city(world, c),
                    d.manhattan(from),
                    c,
                )
            })
        })
        .min()
        .map(|(_, _, _, _, c)| c);
    if let Some(c) = with_room {
        return Some((c, false));
    }
    let city = camps.iter().copied().filter(|&c| open(world, c)).find(|&c| is_city(world, c));
    city.or_else(|| camps.iter().copied().find(|&c| open(world, c))).map(|c| (c, true))
}

/// Round 4 T1 (OPEN-231): a Camp door within `[distance.choice]
/// visit_ticks` of the child's Home by the parent's trip (S1's
/// `distance::trip_ahead`, the week's visit as `household::camp_too_far`
/// reads it). True with no parent or the choice off.
fn in_reach(world: &World, parent: Option<EntityId>, from: TilePos, to: TilePos) -> bool {
    let (Some(p), Some(c)) = (parent, crate::systems::distance::choice(world)) else { return true };
    crate::systems::distance::trip_ahead(world, p, from, to) <= c.visit_ticks
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
/// with `hunger_days ≥ take_days` is on the ladder, ascending id (round 4
/// T1, OPEN-231: the first neglect a warning at the door, a repeat inside
/// `warn_window_days` past `warn_grace_days` the take); before it the
/// Camp children whose parent's pantry has held are let home.
pub fn take_daily(world: &mut World) {
    let cfg = world.config.camp.clone();
    let take_days = cfg.take_days;
    for c in standing_camps(world) {
        prune(world, c);
    }
    let_home_daily(world);
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
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
    let mut warns: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for c in due {
        let home = world.comp::<Household>(c).and_then(|h| h.home);
        let warned = world.comp::<Child>(c).and_then(|k| k.warned_day);
        match (home, warned) {
            // No door to come to (a street child), or the ladder off: the take.
            (None, _) => {
                take(world, c);
            }
            _ if cfg.warn_window_days == 0 => {
                take(world, c);
            }
            (Some(_), Some(w)) if day.saturating_sub(w) <= u32::from(cfg.warn_window_days) => {
                if day.saturating_sub(w) >= u32::from(cfg.warn_grace_days) {
                    take(world, c);
                }
            }
            (Some(h), _) => warns.entry(h).or_default().push(c),
        }
    }
    for (h, kids) in warns {
        warn(world, h, &kids, day);
    }
}

/// Round 4 T1 (OPEN-231, Dylan 2026-10-27: neglect warns first): the
/// service at `home`'s door over `kids`' unfed days. Each child's
/// `warned_day` is today and its unfed count starts again; the parents
/// hear it (one `Custody` event, the grace and the window in it); the
/// first free parent short of the grace's food money (`warn_grace_days`
/// of the children's food at his meal price) asks his Friends for it
/// (`warn_ask_*`, an edge debt), and the line says how it went.
fn warn(world: &mut World, home: EntityId, kids: &[EntityId], day: u32) {
    let cfg = world.config.camp.clone();
    let mut parents: Vec<EntityId> = Vec::new();
    let mut unfed = 0u8;
    for &k in kids {
        if let Some(c) = world.comp_mut::<Child>(k) {
            unfed = unfed.max(c.hunger_days);
            c.warned_day = Some(day);
            c.hunger_days = 0;
        }
        parents.extend(parents_of(world, k));
    }
    parents.sort_unstable();
    parents.dedup();
    ownership::jobs2_row(world).camp_warnings += kids.len() as u32;
    let names: Vec<String> = kids
        .iter()
        .map(|&k| format!("{} ({} y)", world.name_of(k), world.comp::<Identity>(k).map_or(0, |i| i.age_years())))
        .collect();
    let who: Vec<String> = parents.iter().map(|&p| world.name_of(p)).collect();
    let heard = if who.is_empty() {
        "nobody was home to hear it".to_string()
    } else {
        format!("{} heard it", who.join(" and "))
    };
    let until = day + u32::from(cfg.warn_window_days);
    let mut text = format!(
        "the child protective service came to the door of {}: {} went {unfed} days unfed; {heard}: {} days' grace, and unfed again before d{until} the service takes them",
        world.name_of(home),
        names.join(", "),
        cfg.warn_grace_days,
    );
    let mut actors: Vec<EntityId> = parents.clone();
    if let Some(asker) = parents.iter().copied().find(|&p| {
        world.has::<crate::components::Brain>(p)
            && !world.has::<crate::components::Sentence>(p)
            && crate::systems::demography::is_adult(world, p)
            && crate::systems::law::living(world, p)
    }) {
        let kids_home = world.residents_of(home).iter().filter(|&&r| world.has::<Child>(r)).count().max(1) as f32;
        let units = (CHILD_FOOD_PER_DAY * kids_home * f32::from(cfg.warn_grace_days)).ceil() as i64;
        let cost = units * crate::systems::life::meal_price(world, asker);
        let held: i64 =
            parents.iter().map(|&p| world.comp::<crate::components::Wallet>(p).map_or(0, |w| w.coins.max(0))).sum();
        if held >= cost {
            text.push_str(&format!(" ({held}c between them covers the {cost}c of food)"));
        } else {
            let (bits, lenders) = warn_ask(world, asker, cost - held, &cfg, day);
            actors.extend(lenders);
            text.push_str(&format!("; {} has {held}c of the {cost}c of food and {bits}", world.name_of(asker)));
        }
    }
    actors.extend(kids.iter().copied());
    actors.push(home);
    world.push_event(EventKind::Custody, &actors, text);
}

/// Round 4 T1: the warned parent's ladder of Friends (care's ask, round 3
/// S4, with `[camp] warn_ask_*`): the line's tail and the Friends asked.
fn warn_ask(
    world: &mut World,
    id: EntityId,
    mut need: i64,
    cfg: &crate::config::CampCfg,
    day: u32,
) -> (String, Vec<EntityId>) {
    use crate::components::{MemoryKind, RelKind, Wallet};
    let mut friends: Vec<(f32, EntityId)> = world
        .neighbours(id)
        .filter(|&o| o != id && crate::systems::law::living(world, o))
        .filter_map(|o| {
            world
                .edge(id, o)
                .filter(|e| e.kind == RelKind::Friend && e.affinity >= cfg.warn_ask_affinity)
                .map(|e| (e.affinity, o))
        })
        .collect();
    friends.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.1.cmp(&q.1)));
    friends.truncate(usize::from(cfg.warn_ask_tries.max(1)));
    if friends.is_empty() {
        return ("no friend to ask".to_string(), Vec::new());
    }
    let mut bits: Vec<String> = Vec::new();
    let mut asked = Vec::new();
    for (k, (aff, f)) in friends.into_iter().enumerate() {
        if need <= 0 {
            break;
        }
        asked.push(f);
        let fname = world.name_of(f);
        let spare = world.comp::<Wallet>(f).map_or(0, |w| w.coins) - cfg.warn_ask_keep;
        if spare <= 0 {
            bits.push(format!("{fname} had nothing to spare"));
            continue;
        }
        let greed = world.comp::<Personality>(f).map_or(0.5, |p| p.greed);
        let roll = hash(world, PURPOSE_WARN_ASK, key(id), u64::from(day) * 8 + k as u64);
        if roll >= aff * (1.0 - greed / 2.0) {
            world.remember(id, MemoryKind::Rejected, Some(f), 0.5, -0.4, false);
            bits.push(format!("{fname} turned them down"));
            continue;
        }
        let lend = spare.min(need);
        if let Some(w) = world.comp_mut::<Wallet>(f) {
            w.coins -= lend;
        }
        if let Some(w) = world.comp_mut::<Wallet>(id) {
            w.coins += lend;
        }
        need -= lend;
        let (lo, _) = crate::components::edge_key(id, f);
        let tick = world.tick;
        let e = world.edge_entry(id, f);
        e.debt += if lo == id { lend as i32 } else { -(lend as i32) };
        e.debt_since.get_or_insert(tick);
        e.last_interaction = tick;
        world.remember(f, MemoryKind::Paid, Some(id), 0.5, 0.1, false);
        world.remember(id, MemoryKind::Paid, Some(f), 0.6, 0.3, false);
        bits.push(format!("{fname} lent {lend}c"));
    }
    let tail = if need <= 0 { ": covered".to_string() } else { format!(": still {need}c short") };
    (format!("asked friends for the rest: {}{tail}", bits.join("; ")), asked)
}

/// Round 4 T1 (OPEN-231): a child living in `home` is on the service's
/// warning (inside `warn_window_days` of it): the household's grocery run
/// fills the children's food from the pantry alone (`routine::grocery_plan`,
/// `lod::stat_grocery`: food carried for a shift feeds no child).
pub fn home_on_warning(world: &World, home: EntityId) -> bool {
    let window = u32::from(world.config.camp.warn_window_days);
    if window == 0 {
        return false;
    }
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    world
        .residents_of(home)
        .iter()
        .any(|&r| world.comp::<Child>(r).and_then(|c| c.warned_day).is_some_and(|w| day.saturating_sub(w) <= window))
}

/// Round 4 T1 (OPEN-231): the pantry units of `home` an adult may eat or
/// pack: on the service's warning the children's food
/// (`ages::children_food_need`) is kept back for the midnight meal (a
/// Block's adults ate the pantry bare before the children's turn); else
/// the whole stock.
pub fn adults_share(world: &World, home: EntityId) -> u32 {
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food);
    if !home_on_warning(world, home) {
        return stock;
    }
    stock.saturating_sub(crate::systems::ages::children_food_need(world, home))
}

/// Round 4 T1 (OPEN-231): `p` can have his Camp children home: alive,
/// adult, free, in a standing Home whose pantry holds `home_pantry_days` of
/// the children's food with his `away` Camp children counted in. The Home.
fn home_holds(world: &World, p: EntityId, away: usize, cfg: &crate::config::CampCfg) -> Option<EntityId> {
    if !crate::systems::law::living(world, p)
        || !crate::systems::demography::is_adult(world, p)
        || world.has::<crate::components::Sentence>(p)
    {
        return None;
    }
    let h = world.comp::<Household>(p).and_then(|x| x.home)?;
    let b = world.comp::<Building>(h).filter(|b| b.kind == BuildingKind::Home && !b.demolished && !b.derelict)?;
    let kids = (world.residents_of(h).iter().filter(|&&r| world.has::<Child>(r)).count() + away) as f32;
    (b.stock_food as f32 >= CHILD_FOOD_PER_DAY * kids * cfg.home_pantry_days).then_some(h)
}

/// Round 4 T1 (OPEN-231): midnight, before the ladder: each Camp child's
/// let-home count (a parent's Home held the food of him and his other Camp
/// children tonight: one more, else none); at `let_home_days` the service
/// lets the parent's children home together, on a warning (`warned_day`
/// today: unfed again inside the window and the service takes them back).
fn let_home_daily(world: &mut World) {
    let cfg = world.config.camp.clone();
    if cfg.let_home_days == 0 {
        return;
    }
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    // (child, its camp, its parents), every Camp child.
    let mut kids: Vec<(EntityId, EntityId, Vec<EntityId>)> = Vec::new();
    for camp in standing_camps(world) {
        for &k in children_of(world, camp) {
            kids.push((k, camp, parents_of(world, k)));
        }
    }
    let mut away_n: std::collections::BTreeMap<EntityId, usize> = std::collections::BTreeMap::new();
    for (_, _, ps) in &kids {
        for &p in ps {
            *away_n.entry(p).or_default() += 1;
        }
    }
    let away = |p: EntityId| away_n.get(&p).copied().unwrap_or(0);
    let picks: Vec<(EntityId, EntityId, Option<EntityId>)> = kids
        .iter()
        .map(|(k, camp, ps)| (*k, *camp, ps.iter().copied().find(|&p| home_holds(world, p, away(p), &cfg).is_some())))
        .collect();
    // (parent, camp) -> the children going home and the nights held.
    let mut going: std::collections::BTreeMap<(EntityId, EntityId), (Vec<EntityId>, u16)> =
        std::collections::BTreeMap::new();
    for (k, camp, pick) in picks {
        let Some(c) = world.comp_mut::<Child>(k) else { continue };
        if pick.is_some() {
            c.home_fed_days = c.home_fed_days.saturating_add(1);
        } else {
            c.home_fed_days = 0;
        }
        if let Some(p) = pick.filter(|_| c.home_fed_days >= cfg.let_home_days) {
            let n = c.home_fed_days;
            let e = going.entry((p, camp)).or_default();
            e.0.push(k);
            e.1 = e.1.max(n);
        }
    }
    for ((p, camp), (ks, n)) in going {
        let mut at = None;
        for &k in &ks {
            release(world, k, ReleaseWhy::LetHome);
            at = send_home(world, k, p);
            if let Some(c) = world.comp_mut::<Child>(k) {
                c.warned_day = Some(day);
                c.home_fed_days = 0;
                c.hunger_days = 0;
            }
        }
        let names: Vec<String> = ks.iter().map(|&k| world.name_of(k)).collect();
        let (pn, what) = (world.name_of(p), world.name_of(camp));
        let to = at.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
        let mut actors = ks.clone();
        actors.extend([p, camp]);
        actors.extend(at);
        world.push_event(
            EventKind::Custody,
            &actors,
            format!(
                "the service let {} go home from {what} to {pn} at {to}: the pantry has held their food {n} nights running (home on a warning: unfed again before d{} and they go back)",
                names.join(", "),
                day + u32::from(cfg.warn_window_days)
            ),
        );
        ownership::jobs2_row(world).camp_let_home += ks.len() as u32;
    }
}

/// The take: the camp with room nearest the child's Home (custody never
/// refuses: the city camp over capacity), the Household moved,
/// `ChildTaken` for the child and each living parent (a Grief-class
/// `ChildTaken` memory, no deed; a grudge on the Law), a Feed bulletin.
pub fn take(world: &mut World, child: EntityId) -> Option<EntityId> {
    if !world.has::<Child>(child) {
        return None;
    }
    let home = world.comp::<Household>(child).and_then(|h| h.home);
    let from =
        home.and_then(|h| door(world, h)).or_else(|| world.comp::<Position>(child).map(|p| p.tile)).unwrap_or_default();
    let parents = parents_of(world, child);
    let (camp, over) = placement(world, from, parents.first().copied())?;
    let day = world.day();
    if over {
        if let Some(s) = state_mut(world, camp) {
            s.last_overflow_day = Some(day);
        }
    }
    house(world, child, camp);
    note_move(world, camp, child, CampMove::Taken);
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
/// the meals (`CHILD_FOOD_PER_DAY` a child; the Reserve covers a shortfall
/// and an unfed day is counted, never a death: S6), the rings, the scandal and the closure, the
/// shift (Parts from the older children's days), the reopening.
pub fn daily(world: &mut World) {
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
    if crate::systems::service::civic_on(world) {
        warden_rolls(world);
    }
}

/// Lives at work phase 3 (§ 2.2, camp warden; civic only, midnight): an
/// escape on `p_camp_escape` (`release(Escaped)`, then the first living
/// parent's Home with room, else the street as `release` leaves it).
/// Hashed on (child, day): no world-stream draw. The pressure pass S6
/// (Dylan 2026-10-27, no child is harmed): the warden's beating is gone.
fn warden_rolls(world: &mut World) {
    let cfg = world.config.service.clone();
    let day = world.day();
    let kids: Vec<(EntityId, EntityId)> = standing_camps(world)
        .into_iter()
        .flat_map(|c| children_of(world, c).iter().map(move |&k| (k, c)).collect::<Vec<_>>())
        .collect();
    for (k, camp) in kids {
        if !world.is_alive(k) || world.has::<crate::components::Corpse>(k) {
            continue;
        }
        if hash(world, PURPOSE_ESCAPE, key(k), day) < cfg.p_camp_escape {
            escape(world, k, camp, "");
        }
    }
}

/// A Camp child out and away: `release(Escaped)`, then the first living
/// parent's Home, else the street; the event (`how` after the Camp's name).
fn escape(world: &mut World, k: EntityId, camp: EntityId, how: &str) {
    release(world, k, ReleaseWhy::Escaped);
    let home = parents_of(world, k).into_iter().find_map(|p| send_home(world, k, p));
    let (name, what) = (world.name_of(k), world.name_of(camp));
    let to = home.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
    let mut actors = vec![k, camp];
    actors.extend(home);
    world.push_event(EventKind::CampEscape, &actors, format!("{name} slipped out of {what}{how} to {to}"));
    ownership::jobs2_row(world).camp_escapes += 1;
}

/// Round 4 T1 (OPEN-231, the warden's day; civic only, hourly from
/// `service::civic_hour`): a Camp child's try at the fence on `p_abscond`
/// at its hashed hour inside `abscond_hours`; a warden on duty there
/// catches it on `p_abscond_caught` and walks it back in (no blow, no
/// fight: the under-18 rule; the child stays), else it gets away
/// (`escape`). Hashed on (child, day): no world-stream draw.
pub fn abscond_hour(world: &mut World) {
    let cfg = world.config.camp.clone();
    let [from, to] = cfg.abscond_hours;
    if cfg.p_abscond <= 0.0 || to <= from {
        return;
    }
    let day = world.day();
    let hour = (u64::from(world.tick_of_day()) / crate::time::TICKS_PER_HOUR) as u8;
    if !(from..to).contains(&hour) {
        return;
    }
    let span = f32::from(to - from);
    let mut tries: Vec<(EntityId, EntityId)> = Vec::new();
    for camp in standing_camps(world) {
        for &k in children_of(world, camp) {
            let h = from + ((hash(world, PURPOSE_ABSCOND_HOUR, key(k), day) * span) as u8).min(to - from - 1);
            if h == hour && hash(world, PURPOSE_ABSCOND, key(k), day) < cfg.p_abscond {
                tries.push((k, camp));
            }
        }
    }
    for (k, camp) in tries {
        if !world.is_alive(k) || world.has::<crate::components::Corpse>(k) {
            continue;
        }
        let wardens = crate::systems::service::servers_on_duty(world, camp, crate::components::ServeKind::Inmate);
        let w = wardens.first().copied();
        if let Some(w) = w.filter(|_| hash(world, PURPOSE_CAUGHT, key(k), day) < cfg.p_abscond_caught) {
            let (wn, cn, what) = (world.name_of(w), world.name_of(k), world.name_of(camp));
            let age = world.comp::<Identity>(k).map_or(0, |i| i.age_years());
            world.push_event(
                EventKind::Custody,
                &[w, k, camp],
                format!("{wn} caught {cn} ({age} y) at the fence of {what} and walked them back in"),
            );
            note_move(world, camp, k, CampMove::Caught);
            ownership::jobs2_row(world).camp_caught += 1;
        } else {
            let how = match w {
                Some(w) => format!(" past {} in the day", world.name_of(w)),
                None => " in the day, no warden on the gate,".to_string(),
            };
            escape(world, k, camp, &how);
        }
    }
}

/// Round 4 T1 (OPEN-231, the warden's day): the headcount at the first
/// hour of `wardens`' shift, one `Custody` line: the children counted, the
/// comings and goings since the last count (by name, three a kind), last
/// night's meals (the unfed, the Reserve's share). The ledger cleared.
pub fn headcount(world: &mut World, camp: EntityId, wardens: &[EntityId]) {
    if wardens.is_empty() {
        return;
    }
    let n = children_of(world, camp).len();
    let (moves, unfed, reserve) = match state_mut(world, camp) {
        Some(s) => (std::mem::take(&mut s.since_count), s.unfed_days.back().copied().unwrap_or(0), s.reserve_today),
        None => return,
    };
    let kinds = [
        (CampMove::Taken, "brought in"),
        (CampMove::Escaped, "slipped out"),
        (CampMove::Caught, "caught at the fence"),
        (CampMove::LetHome, "let home"),
        (CampMove::BoughtOut, "bought out"),
        (CampMove::BrokenOut, "broken out"),
        (CampMove::CameOfAge, "came of age and left"),
    ];
    let mut bits: Vec<String> = Vec::new();
    for (m, label) in kinds {
        let who: Vec<EntityId> = moves.iter().filter(|(_, x)| *x == m).map(|(c, _)| *c).collect();
        if who.is_empty() {
            continue;
        }
        let mut names: Vec<String> = who.iter().take(3).map(|&c| world.name_of(c)).collect();
        if who.len() > 3 {
            names.push(format!("{} more", who.len() - 3));
        }
        bits.push(format!("{} {label} ({})", who.len(), names.join(", ")));
    }
    let since = if bits.is_empty() {
        "; nobody in or out since the last count".to_string()
    } else {
        format!("; since the last count {}", bits.join(", "))
    };
    let meals = match (unfed, reserve) {
        (0, 0) => "; every child ate last night".to_string(),
        (0, r) => format!("; every child ate last night, {r} of the meals from the Reserve"),
        (u, _) => format!("; {u} went without last night"),
    };
    let mut names: Vec<String> = wardens.iter().take(2).map(|&x| world.name_of(x)).collect();
    if wardens.len() > 2 {
        names.push(format!("{} other wardens", wardens.len() - 2));
    }
    let text = format!("{} counted {n} children at {}{since}{meals}", names.join(" and "), world.name_of(camp));
    let mut actors = wardens.to_vec();
    actors.push(camp);
    world.push_event(EventKind::Custody, &actors, text);
}

/// Lives at work phase 3: a released child goes home to `parent`'s Home
/// when it stands: its Household and its body there. `None` with no such
/// Home (the street, as `release` leaves it). Deviation from the brief's
/// "with room": a child is its parents' dependant and never counted against
/// the flat's capacity (`demography::spawn_child` houses a newborn the same
/// way; most pairs live in a flat of two, so a room check sent every
/// returning child to the street).
pub fn send_home(world: &mut World, child: EntityId, parent: EntityId) -> Option<EntityId> {
    let h = world.comp::<Household>(parent).and_then(|x| x.home)?;
    let bd = world.comp::<Building>(h)?;
    if bd.demolished || bd.derelict {
        return None;
    }
    let d = bd.door;
    let tick = world.tick;
    world.set_home(child, Some(h));
    world.remove_from_building(child);
    if let Some(p) = world.comp_mut::<Position>(child) {
        p.tile = d;
        p.building = Some(h);
        p.entered = tick;
    }
    Some(h)
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
/// (ascending id), `hunger_days` as at home (S6: the Reserve covers the
/// shortfall first; an unfed child never dies).
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
    // The pressure pass S6 (Dylan 2026-10-27, no child is harmed): camp
    // children are fed: a shortfall the camp's stock cannot meet is eaten
    // from the Reserve (the Warehouse), as the school meal; what neither
    // holds is an unfed day (the scandal's count), never a death.
    let short = needed - take;
    let reserve = world.building_of_kind(BuildingKind::Warehouse);
    let covered = reserve.and_then(|r| world.comp::<Building>(r)).map_or(0, |b| b.stock_food.min(short));
    if covered > 0 {
        if let Some(b) = reserve.and_then(|r| world.comp_mut::<Building>(r)) {
            b.stock_food -= covered;
        }
        if let Some(b) = world.comp_mut::<Building>(camp) {
            b.child_food_debt -= covered as f32;
        }
    }
    if let Some(s) = state_mut(world, camp) {
        s.reserve_today = covered;
    }
    let take = take + covered;
    let fed_n = if take == needed { kids.len() } else { (take as usize * 2).min(kids.len()) };
    for (i, &k) in kids.iter().enumerate() {
        let fed = i < fed_n;
        let Some(c) = world.comp_mut::<Child>(k) else { continue };
        if fed {
            c.hunger_days = 0;
        } else {
            c.hunger_days = c.hunger_days.saturating_add(1);
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
    if unfed > 0 {
        // Review fix: a child the service could not feed is a scandal
        // naming the service (S6: hungry, never starved to death).
        let what = world.name_of(camp);
        crate::systems::news::bulletin(
            world,
            &[camp],
            format!("the child protective service could not feed {unfed} children at {what}"),
        );
    }
}

/// E40: the shift is a daily ledger (children have no Brain): per child
/// older than `work_age`, `camp_yield × (floor + slope × farming)` Parts
/// into the camp's accumulator, whole Parts into its stock, `camp_skill_gain`
/// to the child's farming. Phase 2 (plan E13, E40): `input_per_part` to the
/// World per Part that enters the stock (`wages::produce_inputs`, wages on),
/// so a camp is no mint.
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
    let per = world.config.economy2.input_per_part;
    crate::systems::wages::produce_inputs(world, camp, made, per);
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
    // Deviation: the third unfed day killed (as at home) before S6, so the
    // law closes the camp early: at `close_days − 1` unfed days (2 of the plan's
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
        let Some((to, _)) = placement(world, from, None) else { break };
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
    note_move(world, camp, id, why.camp_move());
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
    matches!(role, Role::Farmer | Role::Fabber) && world.has::<CampRaised>(id)
}
