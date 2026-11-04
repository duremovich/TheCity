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

mod feeding;
mod fetch;
mod founding;
mod wardens;
mod warning;

pub use feeding::*;
pub use fetch::*;
pub use founding::*;
pub use wardens::*;
pub use warning::*;

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
        if s.since_count.len() < MOVES_KEPT {
            s.since_count.push((child, m));
        } else if let Some(t) = s.since_over.iter_mut().find(|(k, _)| *k == m) {
            t.1 += 1;
        } else {
            s.since_over.push((m, 1));
        }
    }
}

/// The comings and goings a Camp names between two headcounts (the rest are
/// only counted, `CampState::since_over`).
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

/// The take: the camp with room nearest the child's Home (custody never
/// refuses: the city camp over capacity), the Household moved,
/// `ChildTaken` for the child and each living parent (a Grief-class
/// `ChildTaken` memory, no deed; a grudge on the Law), a Feed bulletin.
pub fn take(world: &mut World, child: EntityId) -> Option<EntityId> {
    take_why(world, child, "")
}

/// Round 5 U2: the take with its reason on the line (empty: none).
pub fn take_why(world: &mut World, child: EntityId, why: &str) -> Option<EntityId> {
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
    // Round 5 U2: the warning is spent by the take (a let-home sets a new one).
    if let Some(c) = world.comp_mut::<Child>(child) {
        c.warned_day = None;
    }
    note_move(world, camp, child, CampMove::Taken);
    let (name, what) = (world.name_of(child), world.name_of(camp));
    let from_home = home.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
    let mut actors = vec![child];
    actors.extend(parents.iter().copied());
    world.push_event(
        EventKind::ChildTaken,
        &actors,
        if why.is_empty() {
            format!("the child protective service took {name} from {from_home} to {what}")
        } else {
            format!("the child protective service took {name} from {from_home} to {what}: {why}")
        },
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
