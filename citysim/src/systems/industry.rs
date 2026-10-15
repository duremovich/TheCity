//! Jobs & lives phase 1 (docs/JOBS_AND_LIVES.md §§ 1-2; JOBS_V2 P4, P5b,
//! P8 and the foreman): the industries, the civic three and the Hq, behind
//! the milestone's master switch `[jobs2] enabled` (`CITYSIM_JOBS2_OFF=1`).
//!
//! - **Packing** (`produce`): a Farm's haul toward the World stops first at
//!   a Packing Plant (its owner's free, another's at wholesale); Packers on
//!   shift turn the plant's Food into Rations ([`accrue_packing`]), sold at
//!   midnight to the World's capped Rations book at `packed_premium` × the
//!   Food bid ([`daily`]).
//! - **Freight** (`handling`): every unit sold to the World and every import
//!   pays a fee to the nearest Freight Yard whose staff were on shift in the
//!   last day ([`handle_export`], [`handle_import`]); a cross-owner Farm ->
//!   Market haul pays the Drivers' fee ([`haul_fee`]). With Yards standing
//!   and none staffed the fee crosses to the World (the leak); with none
//!   standing nothing is charged. A Loader's shift may walk out a Ration.
//! - **Power** (`contract`): a building's daily power goes to the nearest
//!   lit Utility Plant's owner ([`power`]), who pays `fuel_share` of it to
//!   the World ([`settle_fuel`]); a plant with no operator on shift for
//!   `outage_hours` darkens the districts it serves (`District.dark`).
//! - **Construction** (`contract`): a build, refit or floor pays the nearest
//!   Construction Yard (`Flow::Build`) instead of the Treasury
//!   ([`pay_build`]); a new workplace stands closed for `crew_days` scaled
//!   by the Yard's crew (`Building.under_construction_until`), the Builders'
//!   round walks the open sites, and the site opens with its staff posted.
//! - **Civic** (`civic`): the Treasury's allocation, after police and
//!   Sanitation, funds Jailers (one per `held_per_jailer` held), Clerks and
//!   Depot Hands ([`civic_allocate`], [`civic_places`]).
//! - **The Hq** (`overhead`): seeded per megacorp copy on a Spire Lot, built
//!   by a corp owning `hq_min_buildings`; its Office Workers add the admin
//!   term to their corp's competence ([`admin_term`]); `Corp.office`.
//! - **The foreman**: derived, no state ([`foreman`]).
//!
//! Every coin moves between purses through `ownership`; the one new mint is
//! the Rations book (capped per capita, printed). Hourly work is O(yards +
//! plants + sites), daily O(sites + new kinds); nothing reads per agent per
//! tick. Off, every entry point is a no-op and no state is written.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Corp, Good, Inventory, Job, Lod, Position, Role, Sentence, Skills, TilePos, Zone,
};
use crate::config::TradeModel;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{ExportGood, WORLD_ACCOUNT};
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// The hash purposes of this module's rolls (`systems::econ::hash_unit`).
const PURPOSE_PILFER: u64 = 0x10B2_0001;

/// The milestone's master switch (`[jobs2] enabled`, `CITYSIM_JOBS2_OFF=1`).
pub fn on(world: &World) -> bool {
    world.config.jobs2.enabled
}

/// Phase 1's state on the world (empty, and not saved, with the switch off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Jobs2State {
    /// Freight Yard -> the last tick a Loader or Driver was on shift there.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub yard_seen: BTreeMap<EntityId, Tick>,
    /// Utility Plant -> the last tick an operator was on shift there.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plant_seen: BTreeMap<EntityId, Tick>,
    /// Utility Plant -> the fuel it owes the World from today's power.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fuel_due: BTreeMap<EntityId, i64>,
    /// Open build sites: site -> (its Construction Yard, the tick it began).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub sites: BTreeMap<EntityId, (EntityId, Tick)>,
    /// The civic three's headcounts (Jail, Hall, Warehouse), the budget's.
    #[serde(skip_serializing_if = "is_zero3")]
    pub civic: [u16; 3],
    // --- Phase 2, the street (`systems::streetwork`).
    /// Open Pitches, by owner (one an owner).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pitches: BTreeMap<EntityId, crate::components::Pitch>,
    /// Jobless adults -> the day they were first seen without a post.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub idle_since: BTreeMap<EntityId, u64>,
    /// Freight Yard -> today's parcel stops for its couriers' round.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parcel_stops: BTreeMap<EntityId, Vec<EntityId>>,
    /// Garage -> cab rides given today.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub cab_trips: BTreeMap<EntityId, u32>,
    /// Freight Yard -> units it handled today (the backlog's day posts read
    /// yesterday's), and yesterday's.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub yard_units: BTreeMap<EntityId, (u32, u32)>,
    /// Utility Plants dark at some hour today, and their dark days running
    /// (the seizure, `seize_after_days`).
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub plant_dark_today: std::collections::BTreeSet<EntityId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plant_dark_days: BTreeMap<EntityId, u8>,
    /// The last dawn pick's posts offered (a body walks to the corner only
    /// after a dawn that offered some).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub last_posts: u32,
    // --- Phase 3, weapons (`systems::weapons`).
    #[serde(skip_serializing_if = "crate::systems::weapons::ArmsState::is_default")]
    pub arms: crate::systems::weapons::ArmsState,
    // --- Phase 4, vice and fashion (`systems::vice`, `systems::fashion`).
    #[serde(skip_serializing_if = "crate::systems::vice::ViceState::is_default")]
    pub vice: crate::systems::vice::ViceState,
    #[serde(skip_serializing_if = "crate::systems::fashion::FashionState::is_default")]
    pub fashion: crate::systems::fashion::FashionState,
    // --- Phase 5, the chip and braindance (`systems::dollhouse`, `systems::braindance`).
    #[serde(skip_serializing_if = "crate::systems::dollhouse::ChipState::is_default")]
    pub chip: crate::systems::dollhouse::ChipState,
    #[serde(skip_serializing_if = "crate::systems::braindance::BdState::is_default")]
    pub bd: crate::systems::braindance::BdState,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_zero3(v: &[u16; 3]) -> bool {
    *v == [0; 3]
}

impl Jobs2State {
    pub fn is_default(&self) -> bool {
        *self == Jobs2State::default()
    }
}

// ---------------------------------------------------------------------------
// Buildings
// ---------------------------------------------------------------------------

/// Not demolished, not derelict.
fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// A standing building that is not a closed site.
pub fn open(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict && !under_construction(bd, world.tick))
}

/// A site still closed at `now`.
pub fn under_construction(bd: &Building, now: Tick) -> bool {
    bd.under_construction_until.is_some_and(|t| t > now)
}

/// Is `b` a closed site now (no places, no door)? False with the switch off.
pub fn closed_site(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| under_construction(bd, world.tick))
}

fn door(world: &World, b: EntityId) -> TilePos {
    world.comp::<Building>(b).map(|bd| bd.door).unwrap_or_default()
}

/// Open buildings of `kind`, ascending.
fn open_of_kind(world: &World, kind: BuildingKind) -> Vec<EntityId> {
    world.buildings_of_kind(kind).iter().copied().filter(|&b| open(world, b)).collect()
}

/// The open building of `kind` nearest `from` passing `keep` (ties lower id).
fn nearest(world: &World, kind: BuildingKind, from: TilePos, keep: impl Fn(EntityId) -> bool) -> Option<EntityId> {
    world
        .buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| open(world, b) && keep(b))
        .map(|b| (door(world, b).manhattan(from), b))
        .min()
        .map(|(_, b)| b)
}

/// The trade row of `id`'s Job, if a trade.
fn row_of_agent(world: &World, id: EntityId) -> Option<&crate::config::TradeCfg> {
    world.comp::<Job>(id).and_then(|j| crate::systems::trades::row_of(world, j.role))
}

/// `b`'s trade staff of `model` on shift now and at work (a Statistical
/// one counts as there), not held.
pub fn on_duty(world: &World, b: EntityId, model: TradeModel) -> usize {
    let tod = world.tick_of_day();
    world
        .staff_of(b)
        .iter()
        .filter(|&&s| {
            world.comp::<Job>(s).is_some_and(|j| j.on_shift(tod))
                && row_of_agent(world, s).is_some_and(|r| r.model == model)
                && !world.has::<Sentence>(s)
                && world.comp::<Brain>(s).is_some_and(|br| {
                    br.lod == Lod::Statistical || world.comp::<Position>(s).is_some_and(|p| p.building == Some(b))
                })
        })
        .count()
}

/// Trade staff of `model` employed at `b` (on shift or not).
fn staff_of_model(world: &World, b: EntityId, model: TradeModel) -> Vec<EntityId> {
    world.staff_of(b).iter().copied().filter(|&s| row_of_agent(world, s).is_some_and(|r| r.model == model)).collect()
}

/// Phase 3 (§ 4): the Civic Clerks on the books (a `civic` trade at a
/// Hall), who sell the weapons licence.
pub fn clerks_on_books(world: &World) -> usize {
    world.buildings_of_kind(BuildingKind::Hall).iter().map(|&b| staff_of_model(world, b, TradeModel::Civic).len()).sum()
}

// ---------------------------------------------------------------------------
// The foreman (§ 2): derived, no state
// ---------------------------------------------------------------------------

/// The foreman of `b`: its longest-serving worker (the earliest
/// `hired_tick`, ties lower id), `exclude` left out (a hire or a layoff
/// being named). `None` for a building with no other staff.
pub fn foreman(world: &World, b: EntityId, exclude: Option<EntityId>) -> Option<EntityId> {
    world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| Some(s) != exclude)
        .filter_map(|s| world.comp::<Job>(s).map(|j| (j.hired_tick, s)))
        .min()
        .map(|(_, s)| s)
}

/// The event text's foreman clause (", picked by Ines Vo, the foreman"),
/// empty with the switch off or no foreman.
pub fn foreman_note(world: &World, b: EntityId, who: EntityId, verb: &str) -> String {
    if !on(world) {
        return String::new();
    }
    match foreman(world, b, Some(who)) {
        Some(f) => format!(" ({verb} by {}, the foreman)", world.name_of(f)),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Freight (§ 2, `handling`)
// ---------------------------------------------------------------------------

/// A Yard whose staff were on shift in the last day.
fn yard_staffed(world: &World, y: EntityId) -> bool {
    world.jobs2.yard_seen.get(&y).is_some_and(|&t| world.tick.saturating_sub(t) <= TICKS_PER_DAY)
}

/// Where a crossing's fee goes: `Some(Some(yard))` the nearest staffed Yard
/// to `from` (an import with no place: the staffed Yards in turn by the
/// tick), `Some(None)` the World (Yards stand, none staffed), `None` no fee
/// (no Yard stands, or the switch off).
fn fee_to(world: &World, from: Option<TilePos>, turn: u64) -> Option<Option<EntityId>> {
    if !on(world) {
        return None;
    }
    let yards = open_of_kind(world, BuildingKind::FreightYard);
    if yards.is_empty() {
        return None;
    }
    let staffed: Vec<EntityId> = if world.config.jobs2.loader_gate {
        yards.iter().copied().filter(|&y| yard_staffed(world, y)).collect()
    } else {
        yards
    };
    if staffed.is_empty() {
        return Some(None);
    }
    Some(Some(match from {
        Some(p) => *staffed.iter().min_by_key(|&&y| (door(world, y).manhattan(p), y)).expect("non-empty"),
        None => staffed[(turn % staffed.len() as u64) as usize],
    }))
}

/// Pay a handling fee of `fee` from `payer` (a corp or the city in full).
fn pay_fee(world: &mut World, payer: Option<EntityId>, to: Option<Option<EntityId>>, fee: i64) {
    let Some(to) = to else { return };
    if fee <= 0 {
        return;
    }
    match to {
        Some(yard) => {
            let owner = world.owner_of(yard);
            let paid = ownership::charge(world, payer, owner, fee, Flow::Handling);
            ownership::credit(world, yard, paid);
        }
        None => {
            let full = matches!(ownership::owner_kind(world, payer), OwnerKind::City | OwnerKind::Corp(_));
            let paid = ownership::cross_out(world, payer, WORLD_ACCOUNT, fee, Flow::Inputs, full);
            ownership::jobs2_row(world).handling_leak += paid;
        }
    }
}

/// § 2: `units` sold to the World from `building` cross a Freight Yard:
/// `handling_per_unit` each from the seller. A no-op with the switch off.
pub fn handle_export(world: &mut World, building: Option<EntityId>, owner: Option<EntityId>, units: u32) {
    if !on(world) || units == 0 {
        return;
    }
    let fee = (units as f32 * world.config.jobs2.handling_per_unit).round() as i64;
    let from = building.map(|b| door(world, b));
    let to = fee_to(world, from, world.tick);
    // Phase 2 (the backlog's day posts): the units the Yard handled.
    if let (Some(Some(y)), true) = (to, crate::systems::streetwork::on(world)) {
        world.jobs2.yard_units.entry(y).or_default().0 += units;
    }
    pay_fee(world, owner, to, fee);
}

/// § 2: an import of `coins` crosses a Freight Yard: `handling_rate` of
/// its value from the importer (an import names no units: JOBS_V2 J23's
/// rate). A no-op with the switch off.
pub fn handle_import(world: &mut World, from: Option<EntityId>, coins: i64) {
    if !on(world) || coins <= 0 {
        return;
    }
    let fee = (coins as f32 * world.config.jobs2.handling_rate).round() as i64;
    let turn = world.tick ^ from.map_or(0, |f| u64::from(f.index));
    let to = fee_to(world, None, turn);
    pay_fee(world, from, to, fee);
}

/// § 2 (the Drivers): a cross-owner Farm -> Market haul of `units` pays
/// `haul_per_unit` each from the Farm's owner to the nearest staffed Yard
/// (no leak: the haul is the city's own road).
pub fn haul_fee(world: &mut World, farm: EntityId, farm_owner: Option<EntityId>, units: u32) {
    if !on(world) || units == 0 {
        return;
    }
    let fee = (units as f32 * world.config.jobs2.haul_per_unit).round() as i64;
    let from = door(world, farm);
    if let Some(Some(y)) = fee_to(world, Some(from), world.tick) {
        pay_fee(world, farm_owner, Some(Some(y)), fee);
    }
}

// ---------------------------------------------------------------------------
// Packing (§ 2, `produce`)
// ---------------------------------------------------------------------------

/// § 2: the haul's packing leg, before the World's: the Farm owner's own
/// Packing Plants first (a free transfer), then the nearest other plant,
/// which buys at `wholesale` as far as its purse goes (`Flow::Wholesale`,
/// credited to the Farm); room is the plant's Food `stock_cap`. A no-op
/// with the switch off or the World closed (Rations sell only to it).
pub fn haul_to_packing(world: &mut World, farm: EntityId, farm_owner: Option<EntityId>, left: &mut u32) {
    if !on(world) || *left == 0 || !crate::systems::world_market::open(world) {
        return;
    }
    let cap = world.config.buildings.packing_plant.stock_cap;
    let wholesale = world.config.corps.wholesale;
    let from = door(world, farm);
    let room = |w: &World, p: EntityId| w.comp::<Building>(p).map_or(0, |bd| cap.saturating_sub(bd.stock_food));
    let mut plants: Vec<(bool, u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::PackingPlant)
        .iter()
        .copied()
        .filter(|&p| open(world, p))
        .map(|p| (world.owner_of(p) != farm_owner, door(world, p).manhattan(from), p))
        .collect();
    plants.sort_unstable();
    for (rival, _, p) in plants {
        if *left == 0 {
            break;
        }
        let mut take = (*left).min(room(world, p));
        if take == 0 {
            continue;
        }
        if rival && wholesale > 0 {
            let owner = world.owner_of(p);
            let afford = (world.purse(owner).max(0) / wholesale).min(i64::from(u32::MAX)) as u32;
            take = take.min(afford);
            if take == 0 {
                continue;
            }
            let paid = ownership::pay(world, owner, farm_owner, i64::from(take) * wholesale, Flow::Wholesale);
            ownership::credit(world, farm, paid);
        }
        if let Some(bd) = world.comp_mut::<Building>(p) {
            bd.stock_food += take;
        }
        *left -= take;
    }
}

/// § 2 (`trades::accrue`'s Packing Plant arm): a Packer's `ticks` on shift
/// pack `pack_rate × (0.5 + farming)` Food units an hour into
/// `production_accum`; whole units leave the plant's Food for Rations at
/// `pack_ratio`, as far as the Rations stock has room.
pub fn accrue_packing(world: &mut World, worker: EntityId, b: EntityId, ticks: u64) {
    if !on(world) || closed_site(world, b) {
        return;
    }
    let cfg = &world.config.jobs2;
    let (rate, ratio) = (cfg.pack_rate, cfg.pack_ratio.max(0.0));
    let farming = world.comp::<Skills>(worker).map_or(0.0, |s| s.farming);
    let units = rate * (0.5 + farming) * ticks as f32 / TICKS_PER_HOUR as f32;
    let whole = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.production_accum += units;
        let whole = bd.production_accum.floor().max(0.0);
        bd.production_accum -= whole;
        whole as u32
    };
    let food = world.comp::<Building>(b).map_or(0, |bd| bd.stock_food);
    let room = world.goods_cap(b, Good::Rations).saturating_sub(world.stock(b, Good::Rations));
    let per = ratio.max(1e-3);
    let fit = ((room as f32) / per).floor() as u32;
    let take = whole.min(food).min(fit);
    if take == 0 {
        return;
    }
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.stock_food -= take;
    }
    let made = (take as f32 * ratio).floor() as u32;
    let added = world.add_stock(b, Good::Rations, made);
    let row = ownership::jobs2_row(world);
    row.food_packed += take;
    row.rations_made += added;
}

/// § 2, midnight: each open Packing Plant sells its Rations above
/// `rations_floor` to the World's Rations book (capped; ascending id).
fn sell_rations(world: &mut World) {
    if !crate::systems::world_market::open(world) {
        return;
    }
    let floor = world.config.jobs2.rations_floor;
    for p in open_of_kind(world, BuildingKind::PackingPlant) {
        let have = world.stock(p, Good::Rations).saturating_sub(floor);
        if have == 0 {
            continue;
        }
        let owner = world.owner_of(p);
        let (n, paid) = crate::systems::world_market::sell(world, Some(p), owner, ExportGood::Rations, have);
        world.take_stock(p, Good::Rations, n);
        let row = ownership::jobs2_row(world);
        row.rations_sold += n;
        row.rations_paid += paid;
    }
}

/// § 2: does `corp` want a Packing Plant (its Grow in Food, on World
/// demand): the World's Food fill above `pack_fill` and fewer plants than
/// its Farms ÷ `farms_per_plant` (at least one).
pub fn wants_packing(world: &World, corp: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let cfg = &world.config.jobs2;
    if crate::systems::world_market::fill(world, ExportGood::Food) <= cfg.pack_fill {
        return false;
    }
    let owned = |k: BuildingKind| ownership::owned_of_kind(world, Some(corp), k).len() as u32;
    let farms = owned(BuildingKind::Farm);
    let want = (farms / cfg.farms_per_plant.max(1)).max(u32::from(farms > 0));
    owned(BuildingKind::PackingPlant) < want
}

// ---------------------------------------------------------------------------
// Power (§ 2, `contract`)
// ---------------------------------------------------------------------------

/// A plant whose operators were on shift within `outage_hours`.
fn plant_lit(world: &World, p: EntityId) -> bool {
    let hours = u64::from(world.config.jobs2.outage_hours.max(1));
    world.jobs2.plant_seen.get(&p).is_some_and(|&t| world.tick.saturating_sub(t) < hours * TICKS_PER_HOUR)
}

/// § 2 (`wages::upkeep`'s power leg): `b`'s daily power `p` goes to the
/// nearest lit Utility Plant's owner (`Flow::Power`, a corp or the city in
/// full), the plant owing `fuel_share` of `p` to the World at the pass's
/// end ([`settle_fuel`]); with the switch off, no plant lit, or none
/// standing, to the World as E13 (`wages::inputs`).
pub fn power(world: &mut World, b: EntityId, owner: Option<EntityId>, p: i64) {
    if p <= 0 {
        return;
    }
    let plant = if on(world) {
        let from = door(world, b);
        nearest(world, BuildingKind::UtilityPlant, from, |x| plant_lit(world, x))
    } else {
        None
    };
    let Some(plant) = plant else {
        crate::systems::wages::inputs(world, owner, p);
        return;
    };
    let to = world.owner_of(plant);
    let paid = ownership::charge(world, owner, to, p, Flow::Power);
    ownership::credit(world, plant, paid);
    // A plant powering its own owner's building moves no coins, but it
    // burns the fuel all the same.
    let fuel = (p as f32 * world.config.jobs2.fuel_share).round() as i64;
    *world.jobs2.fuel_due.entry(plant).or_default() += fuel;
}

/// § 2: each plant's owner pays the day's fuel to the World (`Flow::Inputs`).
pub fn settle_fuel(world: &mut World) {
    let due = std::mem::take(&mut world.jobs2.fuel_due);
    for (plant, fuel) in due {
        let owner = world.owner_of(plant);
        let paid = crate::systems::wages::inputs(world, owner, fuel);
        ownership::jobs2_row(world).power_fuel += paid;
    }
}

/// § 2, hourly: each district's plant (the nearest open one to its
/// centroid) dark or lit; a change writes `District.dark` and an event.
fn outages(world: &mut World) {
    let plants = open_of_kind(world, BuildingKind::UtilityPlant);
    // Phase 2 (the late-year blackouts): a plant dark at any hour of the
    // day is marked for the seizure count (`seize_dark_plants`).
    if crate::systems::streetwork::on(world) {
        for &p in &plants {
            if !plant_lit(world, p) {
                world.jobs2.plant_dark_today.insert(p);
            }
        }
    }
    for i in 0..world.districts.len() {
        let c = world.districts[i].centroid;
        let serving = plants.iter().copied().min_by_key(|&p| (door(world, p).manhattan(c), p));
        let dark = serving.is_some_and(|p| !plant_lit(world, p));
        if world.districts[i].dark == dark {
            continue;
        }
        world.districts[i].dark = dark;
        let name = world.districts[i].name.clone();
        let actors: Vec<EntityId> = serving.into_iter().collect();
        if dark {
            ownership::jobs2_row(world).outages += 1;
            world.push_event(
                EventKind::PowerOut,
                &actors,
                format!("the power went out in {name} (no operator at the plant)"),
            );
        } else {
            world.push_event(EventKind::PowerBack, &actors, format!("the power came back in {name}"));
        }
    }
}

/// Is the district of `b` dark (an outage)? False with the switch off.
pub fn dark_at(world: &World, b: EntityId) -> bool {
    on(world) && world.districts.get(world.district_of_building(b).index()).is_some_and(|d| d.dark)
}

// ---------------------------------------------------------------------------
// Construction (§ 2, `contract`)
// ---------------------------------------------------------------------------

/// A Construction Yard is open (with the switch on).
pub fn yard_open(world: &World) -> bool {
    on(world) && !open_of_kind(world, BuildingKind::ConstructionYard).is_empty()
}

/// Kinds whose builds close until they stand: the workplaces (a kind that
/// employs anyone) but the Blocks.
fn closes(world: &World, kind: BuildingKind) -> bool {
    kind != BuildingKind::Home && !ownership::roles_for(world, kind).is_empty()
}

/// § 2: `payer` pays `cost` for a build, refit or floor (capped by its
/// purse, as `ownership::pay`): with the switch on and a Construction Yard
/// open, to the nearest Yard's owner (`Flow::Build`), `site` (if any)
/// becoming one of its open sites; else to the Treasury (`Flow::Found`,
/// M11's path). `close`: a new workplace stands closed until the crew is
/// done ([`start_site`]). Returns the coins paid.
pub fn pay_build(world: &mut World, payer: Option<EntityId>, cost: i64, site: Option<EntityId>, close: bool) -> i64 {
    let yard = if on(world) {
        let from = site.map(|s| door(world, s)).unwrap_or_default();
        nearest(world, BuildingKind::ConstructionYard, from, |_| true)
    } else {
        None
    };
    let Some(yard) = yard else {
        return ownership::pay(world, payer, None, cost, Flow::Found);
    };
    let to = world.owner_of(yard);
    let before = world.purse(to);
    let paid = ownership::pay(world, payer, to, cost, Flow::Build);
    ownership::credit(world, yard, paid);
    // `Flow::Build` is capital for the payer (one build is not two weeks
    // of losses) but the Yard's trade: its corp's cashflow keeps the take.
    let gained = world.purse(to) - before;
    if let Some(c) = to.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.cashflow_today += gained;
    }
    if let Some(s) = site {
        start_site(world, s, yard, close);
    }
    paid
}

/// § 2: `site` joins `yard`'s open sites; a closing kind stands closed
/// until `now + crew_days × clamp(full crew ÷ the Yard's Builders, 1,
/// crew_slow_max)` days, its posted vacancies withdrawn (re-posted when it
/// opens).
fn start_site(world: &mut World, site: EntityId, yard: EntityId, close: bool) {
    let now = world.tick;
    world.jobs2.sites.insert(site, (yard, now));
    ownership::jobs2_row(world).builds_started += 1;
    let Some(kind) = world.comp::<Building>(site).map(|bd| bd.kind) else { return };
    if !close || !closes(world, kind) {
        return;
    }
    let cfg = &world.config.jobs2;
    let builders = staff_of_model(world, yard, TradeModel::Contract).len().max(1) as f32;
    let crew: usize = ownership::roles_for(world, BuildingKind::ConstructionYard)
        .iter()
        .map(|&(r, _)| crate::systems::jobs::places_of(world, yard, r))
        .sum();
    let slow = (crew.max(1) as f32 / builders).clamp(1.0, cfg.crew_slow_max.max(1.0));
    let days = cfg.crew_days.max(0.0) * slow;
    let until = world.tick + (days * TICKS_PER_DAY as f32).round() as Tick;
    if let Some(bd) = world.comp_mut::<Building>(site) {
        bd.under_construction_until = Some(until);
    }
    world.vacancies.remove(&site);
}

/// § 2, hourly: sites whose time is up open: their staff posted as a
/// conversion posts them, `BuildDone`; a demolished or vanished site drops.
fn finish_sites(world: &mut World) {
    let now = world.tick;
    let crew = (world.config.jobs2.crew_days.max(0.0) * TICKS_PER_DAY as f32) as Tick;
    let sites: Vec<(EntityId, EntityId, Tick)> = world.jobs2.sites.iter().map(|(&s, &(y, t))| (s, y, t)).collect();
    for (s, y, started) in sites {
        let Some((kind, demolished, until)) =
            world.comp::<Building>(s).map(|bd| (bd.kind, bd.demolished, bd.under_construction_until))
        else {
            world.jobs2.sites.remove(&s);
            continue;
        };
        if demolished {
            world.jobs2.sites.remove(&s);
            continue;
        }
        // A floor or a Block (no closure) is done after `crew_days`.
        if until.is_some_and(|t| t > now) {
            continue;
        }
        if until.is_none() && now.saturating_sub(started) < crew {
            continue;
        }
        world.jobs2.sites.remove(&s);
        if let Some(bd) = world.comp_mut::<Building>(s) {
            bd.under_construction_until = None;
        }
        if until.is_some() {
            post_full_staff(world, s, kind);
        }
        ownership::jobs2_row(world).builds_done += 1;
        let (what, yard) = (world.name_of(s), world.name_of(y));
        let owner = world.owner_of(s);
        let mut actors: Vec<EntityId> = owner.into_iter().collect();
        actors.push(s);
        world.push_event(EventKind::BuildDone, &actors, format!("{what} stands (built by {yard})"));
    }
}

/// The site's staff, as `founding::convert` posts them: the bespoke role's
/// `staff × floors` and each trade's places.
fn post_full_staff(world: &mut World, b: EntityId, kind: BuildingKind) {
    let floors = world.comp::<Building>(b).map_or(1, |bd| bd.floors.max(1));
    let staff = world.config.buildings.for_kind(kind).staff as usize;
    if let Some(role) = ownership::role_for(kind) {
        if staff > 0 {
            world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, staff * usize::from(floors)));
        }
    }
    for (role, _) in ownership::roles_for(world, kind) {
        if !role.is_trade() {
            continue;
        }
        let n = crate::systems::jobs::places_of(world, b, role);
        if n > 0 {
            world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, n));
        }
    }
}

/// § 2: `yard`'s open sites, ascending.
pub fn sites_of(world: &World, yard: EntityId) -> Vec<EntityId> {
    world.jobs2.sites.iter().filter(|&(_, &(y, _))| y == yard).map(|(&s, _)| s).collect()
}

/// § 2 (the Builders' round, `trades::new_round`): `yard`'s open sites
/// whose doors lie within `[jobs2] site_reach` tiles of the Yard's (a far
/// site is built from the Yard: a round across the city ate the shift and
/// its wage, the parity city's Full Builders walked 8 h and were never paid).
pub fn round_sites(world: &World, yard: EntityId) -> Vec<EntityId> {
    let (from, reach) = (door(world, yard), world.config.jobs2.site_reach);
    sites_of(world, yard).into_iter().filter(|&s| door(world, s).manhattan(from) <= reach).collect()
}

// ---------------------------------------------------------------------------
// The civic three (§ 2, `civic`)
// ---------------------------------------------------------------------------

/// The civic slot of a kind: the Jail's Jailers, the Hall's Clerks, the
/// Warehouse's Depot Hands.
fn civic_slot(kind: BuildingKind) -> Option<usize> {
    match kind {
        BuildingKind::Jail => Some(0),
        BuildingKind::Hall => Some(1),
        BuildingKind::Warehouse => Some(2),
        _ => None,
    }
}

/// § 2: a `civic` trade's places at `b`: the budget's headcount for its
/// kind dealt over the kind's standing buildings in id order (the first
/// ones take the remainder); a kind with no civic slot keeps the row's
/// `staff_per_floor × floors`.
pub fn civic_places(world: &World, b: EntityId, row: &crate::config::TradeCfg) -> Option<usize> {
    if row.model != TradeModel::Civic {
        return None;
    }
    let kind = world.comp::<Building>(b)?.kind;
    let slot = civic_slot(kind)?;
    let want = usize::from(world.jobs2.civic[slot]);
    let all: Vec<EntityId> = world.buildings_of_kind(kind).iter().copied().filter(|&x| standing(world, x)).collect();
    let i = all.iter().position(|&x| x == b)?;
    let n = all.len();
    Some(want / n + usize::from(i < want % n))
}

/// § 2 (`treasury::civic_daily`, after police and Sanitation): `left`
/// coins a day fund, in order, Jailers (one per `held_per_jailer` held,
/// rounded up), Clerks up to `clerks_max`, Depot Hands up to
/// `depot_hands_max`, each at its row's wage; each headcount moves up at
/// once and down at most `civic_pace` a day. A no-op with the switch off.
pub fn civic_allocate(world: &mut World, left: i64) {
    if !on(world) {
        return;
    }
    let cfg = world.config.jobs2.clone();
    let wage_of = |w: &World, kind: BuildingKind| -> i64 {
        w.config
            .trades
            .iter()
            .find(|r| r.model == TradeModel::Civic && r.workplace.contains(&kind))
            .map_or(i64::MAX, |r| r.wage.max(1))
    };
    let held = world.sentenced().len() as u32;
    let wants =
        [held.div_ceil(cfg.held_per_jailer.max(1)) as i64, i64::from(cfg.clerks_max), i64::from(cfg.depot_hands_max)];
    let kinds = [BuildingKind::Jail, BuildingKind::Hall, BuildingKind::Warehouse];
    let mut left = left.max(0);
    let pace = world.config.treasury.civic_pace;
    for i in 0..3 {
        let w = wage_of(world, kinds[i]);
        let n = if w == i64::MAX { 0 } else { (left / w).min(wants[i]).clamp(0, i64::from(u16::MAX)) };
        left -= n * if w == i64::MAX { 0 } else { w };
        let cur = world.jobs2.civic[i];
        world.jobs2.civic[i] = crate::systems::treasury::paced(cur, n as u16, pace);
    }
}

/// § 2, daily: a civic post over its places is laid off (the newest first)
/// and its extra vacancies withdrawn.
fn civic_reconcile(world: &mut World) {
    for kind in [BuildingKind::Jail, BuildingKind::Hall, BuildingKind::Warehouse] {
        for b in world.buildings_of_kind(kind).to_vec() {
            let roles: Vec<Role> = ownership::roles_for(world, kind)
                .into_iter()
                .map(|(r, _)| r)
                .filter(|&r| crate::systems::trades::row_of(world, r).is_some_and(|x| x.model == TradeModel::Civic))
                .collect();
            for role in roles {
                let places = crate::systems::jobs::places_of(world, b, role);
                let mut holders: Vec<(Tick, EntityId)> = world
                    .staff_of(b)
                    .iter()
                    .copied()
                    .filter_map(|s| world.comp::<Job>(s).filter(|j| j.role == role).map(|j| (j.hired_tick, s)))
                    .collect();
                holders.sort_unstable();
                if let Some(v) = world.vacancies.get_mut(&b) {
                    let open = v.iter().filter(|&&r| r == role).count();
                    let over = (holders.len() + open).saturating_sub(places).min(open);
                    for _ in 0..over {
                        if let Some(i) = v.iter().rposition(|&r| r == role) {
                            v.remove(i);
                        }
                    }
                    if v.is_empty() {
                        world.vacancies.remove(&b);
                    }
                }
                while holders.len() > places {
                    let Some((_, who)) = holders.pop() else { break };
                    let name = world.name_of(who);
                    let at = world.name_of(b);
                    let text = format!("{name} was laid off as {} at {at} (the civic budget)", role.label());
                    crate::systems::economy::dismiss_as(world, who, Some(b), text, EventKind::LaidOff);
                }
            }
        }
    }
}

/// § 2: the Jail is short of Jailers (fewer on its staff than the held
/// need): `factions::breach`'s odds × `jailer_short_mult`. False off.
pub fn jailers_short(world: &World) -> bool {
    if !on(world) {
        return false;
    }
    let need = (world.sentenced().len() as u32).div_ceil(world.config.jobs2.held_per_jailer.max(1)) as usize;
    let have: usize = world
        .buildings_of_kind(BuildingKind::Jail)
        .iter()
        .map(|&j| staff_of_model(world, j, TradeModel::Civic).len())
        .sum();
    have < need
}

// ---------------------------------------------------------------------------
// The Hq (§ 2, `overhead`)
// ---------------------------------------------------------------------------

/// § 2 (`competence::corp_competence`): the admin term `admin_w ×
/// clamp(Σ office staff skill ÷ admin_ref, 0, 1)`, the office staff being
/// the `overhead` trade staff at the corp's open Hqs; 0 with the switch off.
pub fn admin_term(world: &World, corp: EntityId) -> f32 {
    if !on(world) {
        return 0.0;
    }
    let cfg = &world.config.competence;
    let Some(c) = world.comp::<Corp>(corp) else { return 0.0 };
    let mut sum = 0.0f32;
    for &b in &c.buildings {
        if world.comp::<Building>(b).is_none_or(|bd| bd.kind != BuildingKind::Hq) || !open(world, b) {
            continue;
        }
        for s in staff_of_model(world, b, TradeModel::Overhead) {
            let Some(role) = world.comp::<Job>(s).map(|j| j.role) else { continue };
            sum += crate::systems::competence::role_term(world, s, role).unwrap_or(0.0);
        }
    }
    cfg.admin_w * (sum / cfg.admin_ref.max(1e-3)).clamp(0.0, 1.0)
}

/// § 2 (JOBS_V2 J30): the coins a day a corp's office may cost:
/// `admin_share` × its 7-day mean revenue (`Corp.rev`, wages on); without
/// the wage law (no revenue window) the office is not capped.
pub fn admin_budget(world: &World, corp: EntityId) -> Option<i64> {
    if !crate::systems::wages::on(world) {
        return None;
    }
    let c = world.comp::<Corp>(corp)?;
    let mean = if c.rev.is_empty() { 0.0 } else { c.rev.iter().sum::<i64>() as f32 / c.rev.len() as f32 };
    Some((world.config.jobs2.admin_share * mean.max(0.0)).floor() as i64)
}

/// § 2 (JOBS_V2 J30): an `overhead` trade's places at an Hq: its rows by
/// grade (then row order) share the owner's [`admin_budget`] (each takes up
/// to its `staff_per_floor × floors` at its base wage, the next row gets
/// what is left); `None` for any other building or row (the generic places).
pub fn hq_places(world: &World, b: EntityId, row: &crate::config::TradeCfg) -> Option<usize> {
    if row.model != TradeModel::Overhead {
        return None;
    }
    let bd = world.comp::<Building>(b)?;
    if bd.kind != BuildingKind::Hq {
        return None;
    }
    let floors = usize::from(bd.floors.max(1));
    let corp = world.corp_of_building(b)?;
    let Some(mut left) = admin_budget(world, corp) else {
        return Some(usize::from(row.staff_per_floor) * floors);
    };
    // The office floor first, the graded posts above it after (grade, then
    // row order): an office worker is the rung a manager is promoted from.
    let mut rows: Vec<(u8, usize, &crate::config::TradeCfg)> = world
        .config
        .trades
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.model == TradeModel::Overhead
                && r.workplace.contains(&BuildingKind::Hq)
                && crate::systems::classpay::trade_live(world, r)
        })
        .map(|(i, r)| (r.grade, i, r))
        .collect();
    rows.sort_by_key(|&(g, i, _)| (g, i));
    for (_, _, r) in rows {
        let wage = r.wage.max(1);
        let n = (left / wage).clamp(0, (usize::from(r.staff_per_floor) * floors) as i64);
        if r.id == row.id {
            return Some(n as usize);
        }
        left -= n * wage;
    }
    Some(0)
}

/// § 2, daily: `Corp.office` points at the corp's open Hq (the lowest id),
/// or none.
fn offices(world: &mut World) {
    for c in world.corps() {
        let office = world.comp::<Corp>(c).and_then(|cc| {
            cc.buildings.iter().copied().find(|&b| {
                world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Hq) && standing(world, b)
            })
        });
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.office = office;
        }
    }
}

/// § 2, daily: one corp a day (the lowest id first) with no Hq, owning
/// `hq_min_buildings` standing buildings and holding the Hq's cost plus
/// `hq_reserve`, builds one on the vacant Spire Lot nearest its first
/// building (paid as any build: [`pay_build`]).
fn grow_hq(world: &mut World) {
    let cfg = world.config.jobs2.clone();
    let Some(cost) = crate::systems::founding::found_cost(world, BuildingKind::Hq) else { return };
    for c in world.corps() {
        let Some(cc) = world.comp::<Corp>(c) else { continue };
        if cc.office.is_some() {
            continue;
        }
        let owned = cc.buildings.iter().filter(|&&b| standing(world, b)).count();
        if owned < cfg.hq_min_buildings || cc.treasury < cost + cfg.hq_reserve {
            continue;
        }
        // An office the corp's revenue could not staff a floor of is not built.
        let floor_cost = world
            .config
            .trades
            .iter()
            .filter(|r| r.model == TradeModel::Overhead && r.workplace == [BuildingKind::Hq])
            .map(|r| i64::from(r.staff_per_floor) * r.wage)
            .max()
            .unwrap_or(0);
        if admin_budget(world, c).is_some_and(|a| a < floor_cost) {
            continue;
        }
        if cc.buildings.iter().any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Hq)) {
            continue;
        }
        let from = cc.buildings.first().map(|&b| door(world, b)).unwrap_or_default();
        let Some(lot) = crate::systems::founding::nearest_lot_for(world, BuildingKind::Hq, from) else { return };
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Hq, Some(c)).is_err() {
            continue;
        }
        let paid = pay_build(world, Some(c), cost, Some(lot), true);
        if let Some(x) = world.comp_mut::<Corp>(c) {
            x.office = Some(lot);
        }
        let (who, what) = (world.owner_label(Some(c)), world.name_of(lot));
        world.push_event(EventKind::Founded, &[c, lot], format!("{who} built its Corp HQ, {what}, for {paid}"));
        return;
    }
}

// ---------------------------------------------------------------------------
// Seeding (§ 2)
// ---------------------------------------------------------------------------

/// The vacant Lot for a seeded `kind`: its zone first (the Spire for an Hq,
/// the Vats for the industries; any allowed tier otherwise), the district
/// with the fewest of the kind, then nearest the district's centroid.
fn seed_lot(world: &World, kind: BuildingKind) -> Option<EntityId> {
    // Phase 3: a Gunsmith stands in the Mid.
    let want = match kind {
        BuildingKind::Hq => Zone::Spire,
        BuildingKind::Gunsmith => Zone::Mid,
        _ => Zone::Vats,
    };
    seed_lot_in(world, kind, want)
}

/// [`seed_lot`] in the zone `want` first.
fn seed_lot_in(world: &World, kind: BuildingKind, want: Zone) -> Option<EntityId> {
    crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter_map(|l| {
            let bd = world.comp::<Building>(l)?;
            let tier = crate::systems::founding::door_tier(world, bd.door);
            if !crate::systems::founding::tier_ok(kind, tier) {
                return None;
            }
            let off_zone = world.map.zone(bd.door) != want;
            let d = world.district_of_building(l);
            let copies = world
                .buildings_of_kind(kind)
                .iter()
                .filter(|&&b| standing(world, b) && world.district_of_building(b) == d)
                .count();
            let centroid = world.districts.get(d.index()).map_or(bd.door, |x| x.centroid);
            Some((off_zone, copies, bd.door.manhattan(centroid), l))
        })
        .min()
        .map(|(_, _, _, l)| l)
}

/// One seeded `kind` for `owner` on a Lot at its seeding floors; a
/// `Founded` event. No RNG, no coins (the seed buildings are unpaid).
fn seed_one(world: &mut World, kind: BuildingKind, owner: Option<EntityId>) -> Option<EntityId> {
    let lot = seed_lot(world, kind)?;
    seed_on(world, lot, kind, owner)
}

/// Phase 4 (`fashion::seed`): one seeded `kind` for `owner` on a Lot of
/// the zone `want` first.
pub fn seed_kind_in(world: &mut World, kind: BuildingKind, owner: Option<EntityId>, want: Zone) -> Option<EntityId> {
    let lot = seed_lot_in(world, kind, want)?;
    seed_on(world, lot, kind, owner)
}

fn seed_on(world: &mut World, lot: EntityId, kind: BuildingKind, owner: Option<EntityId>) -> Option<EntityId> {
    let floors = world.config.buildings.for_kind(kind).floors_seed.unwrap_or(1).max(1);
    let floors = floors.min(world.config.floors_max(kind));
    if let Some(bd) = world.comp_mut::<Building>(lot) {
        bd.floors = floors;
    }
    if crate::systems::founding::build_on_lot(world, lot, kind, owner).is_err() {
        if let Some(bd) = world.comp_mut::<Building>(lot) {
            bd.floors = 1;
        }
        return None;
    }
    let at = world.districts.get(world.district_of_building(lot).index()).map_or("?", |x| x.name.as_str());
    let who = world.owner_label(owner);
    let text = format!("{who} opened a {} in {at} (seeded; Lot {})", kind.label(), lot.index);
    let mut actors: Vec<EntityId> = owner.into_iter().collect();
    actors.push(lot);
    world.push_event(EventKind::Founded, &actors, text);
    Some(lot)
}

/// Phase 3 (`weapons::seed`): one seeded `kind` for `owner` on the Lots left.
pub fn seed_kind(world: &mut World, kind: BuildingKind, owner: Option<EntityId>) -> Option<EntityId> {
    seed_one(world, kind, owner)
}

/// `World::new`, after the camps and before the trades' seeding: the
/// industries for every copy of their `[jobs2] *_rows`, and an Hq for every
/// megacorp copy (`hq_megacorps`), on the Lots left. No RNG; nothing with
/// the switch off. A plant's operators and a Yard's crew count as seen at
/// the seed (no opening outage).
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.jobs2.clone();
    let owners_of = |w: &World, rows: &[usize]| -> Vec<EntityId> {
        let mut out = Vec::new();
        let copies = w.seed_copies();
        for j in 0..copies {
            for &r in rows {
                let Some(t) = w.config.corps.row_template(r) else { continue };
                if let Some(&c) = w.corps_of_template(t).get(j) {
                    out.push(c);
                }
            }
        }
        out
    };
    let plan: [(BuildingKind, &Vec<usize>); 4] = [
        (BuildingKind::FreightYard, &cfg.freight_rows),
        (BuildingKind::UtilityPlant, &cfg.utility_rows),
        (BuildingKind::ConstructionYard, &cfg.construction_rows),
        (BuildingKind::PackingPlant, &cfg.packing_rows),
    ];
    for (kind, rows) in plan {
        for owner in owners_of(world, rows) {
            seed_one(world, kind, Some(owner));
        }
    }
    if cfg.hq_megacorps {
        let mut megas: Vec<(u16, EntityId)> = world
            .corps()
            .into_iter()
            .filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.slot).map(|s| (s, c)))
            .filter(|&(s, _)| world.config.corps.megacorp.get(usize::from(s)).copied().unwrap_or(false))
            .collect();
        megas.sort_unstable();
        for (_, c) in megas {
            if let Some(h) = seed_one(world, BuildingKind::Hq, Some(c)) {
                if let Some(cc) = world.comp_mut::<Corp>(c) {
                    cc.office = Some(h);
                }
            }
        }
    }
    let now = world.tick;
    for y in world.buildings_of_kind(BuildingKind::FreightYard).to_vec() {
        world.jobs2.yard_seen.insert(y, now);
    }
    for p in world.buildings_of_kind(BuildingKind::UtilityPlant).to_vec() {
        world.jobs2.plant_seen.insert(p, now);
    }
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Hourly (from `living::run`): the Yards' and plants' staff on shift, the
/// outages, the sites that stand, and at the night shift's end the
/// Loaders' pilfer roll. O(yards + plants + sites + districts × plants).
pub fn hourly(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let now = world.tick;
    for y in open_of_kind(world, BuildingKind::FreightYard) {
        if on_duty(world, y, TradeModel::Handling) + on_duty(world, y, TradeModel::Contract) > 0 {
            world.jobs2.yard_seen.insert(y, now);
        }
    }
    for p in open_of_kind(world, BuildingKind::UtilityPlant) {
        if on_duty(world, p, TradeModel::Contract) > 0 {
            world.jobs2.plant_seen.insert(p, now);
        }
    }
    outages(world);
    finish_sites(world);
    let night_end = world.config.world.shift_night.iter().map(|&(_, e)| e).filter(|&e| e < 1440).max();
    if night_end == Some(world.tick_of_day()) {
        pilfer(world);
    }
}

/// § 2: at the night shift's end each Loader (a `handling` trade at a Yard)
/// with a Packing Plant holding Rations rolls `p_pilfer` (a pure hash of
/// the day and the Loader): a hit moves one Ration from the nearest plant
/// with stock into the Loader's food (`Pilfered`).
fn pilfer(world: &mut World) {
    let p = world.config.jobs2.p_pilfer;
    if p <= 0.0 {
        return;
    }
    let (seed, day) = (world.seed(), world.day());
    for y in open_of_kind(world, BuildingKind::FreightYard) {
        for l in staff_of_model(world, y, TradeModel::Handling) {
            if world.has::<Sentence>(l) {
                continue;
            }
            if crate::systems::econ::hash_unit(seed, PURPOSE_PILFER, day, u64::from(l.index)) >= p {
                continue;
            }
            let from = door(world, y);
            let Some(plant) = nearest(world, BuildingKind::PackingPlant, from, |x| world.stock(x, Good::Rations) > 0)
            else {
                continue;
            };
            if world.take_stock(plant, Good::Rations, 1) == 0 {
                continue;
            }
            if let Some(inv) = world.comp_mut::<Inventory>(l) {
                inv.food += 1;
            }
            ownership::jobs2_row(world).pilfered += 1;
            let (who, what) = (world.name_of(l), world.name_of(plant));
            world.push_event(EventKind::Pilfered, &[l, plant], format!("{who} walked a Ration out of {what}"));
        }
    }
}

/// Midnight (from `living::run`, after the World's book refills): the
/// Rations sale, the civic posts reconciled, `Corp.office`, an Hq built.
pub fn daily(world: &mut World) {
    if !on(world) || world.tick_of_day() != 0 {
        return;
    }
    sell_rations(world);
    civic_reconcile(world);
    offices(world);
    grow_hq(world);
    seize_dark_plants(world);
}

/// Phase 2 (the coordinator's carry-over: a broke owner's plant left every
/// district dark every night): a corp-owned Utility Plant dark on
/// `[jobs2] seize_after_days` days running is taken by the city through
/// the Nationalise lever's path (`ownership::nationalise`: the Treasury
/// pays its value, the staff follow); the city then staffs it as any of
/// its buildings. Under `[jobs2] street` (the phase's switch); a Treasury
/// that cannot pay leaves it dark.
fn seize_dark_plants(world: &mut World) {
    if !crate::systems::streetwork::on(world) {
        return;
    }
    let today = std::mem::take(&mut world.jobs2.plant_dark_today);
    let limit = world.config.jobs2.seize_after_days;
    for p in world.buildings_of_kind(BuildingKind::UtilityPlant).to_vec() {
        let days = if today.contains(&p) { world.jobs2.plant_dark_days.get(&p).copied().unwrap_or(0) + 1 } else { 0 };
        if days == 0 {
            world.jobs2.plant_dark_days.remove(&p);
            continue;
        }
        world.jobs2.plant_dark_days.insert(p, days);
        if days < limit || limit == 0 || world.corp_of_building(p).is_none() {
            continue;
        }
        if ownership::nationalise(world, p).is_ok() {
            world.jobs2.plant_dark_days.remove(&p);
            ownership::jobs2_row(world).plants_seized += 1;
        }
    }
}

/// The day's snapshot columns (`systems::stats::snapshot`): headcounts by
/// model and by new kind, the civic three, the Hqs, sites and dark districts.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut by_model = [0u32; 8];
    for (i, role) in world.config.trade_roles().into_iter().enumerate() {
        let Some(row) = world.config.trades.get(i) else { continue };
        let k = match row.model {
            TradeModel::Produce => 0,
            TradeModel::Service => 1,
            TradeModel::Handling => 2,
            TradeModel::Contract => 3,
            TradeModel::Civic => 4,
            TradeModel::Overhead => 5,
            TradeModel::Resale => 6,
            TradeModel::Day => 7,
        };
        by_model[k] += world.workers(role).len() as u32;
    }
    let staff_at = |w: &World, kind: BuildingKind| -> u32 {
        w.buildings_of_kind(kind).iter().map(|&b| w.staff_of(b).len() as u32).sum()
    };
    let count = |w: &World, kind: BuildingKind| -> u32 {
        w.buildings_of_kind(kind).iter().filter(|&&b| standing(w, b)).count() as u32
    };
    let civic_at = |w: &World, kind: BuildingKind| -> u32 {
        w.buildings_of_kind(kind).iter().map(|&b| staff_of_model(w, b, TradeModel::Civic).len() as u32).sum()
    };
    let office: u32 = world
        .buildings_of_kind(BuildingKind::Hq)
        .iter()
        .map(|&b| staff_of_model(world, b, TradeModel::Overhead).len() as u32)
        .sum();
    let vals = (
        staff_at(world, BuildingKind::PackingPlant),
        staff_at(world, BuildingKind::FreightYard),
        staff_at(world, BuildingKind::UtilityPlant),
        staff_at(world, BuildingKind::ConstructionYard),
        staff_at(world, BuildingKind::Hq),
        [
            count(world, BuildingKind::PackingPlant),
            count(world, BuildingKind::FreightYard),
            count(world, BuildingKind::UtilityPlant),
            count(world, BuildingKind::ConstructionYard),
            count(world, BuildingKind::Hq),
        ],
        [
            civic_at(world, BuildingKind::Jail),
            civic_at(world, BuildingKind::Hall),
            civic_at(world, BuildingKind::Warehouse),
        ],
    );
    let dark = world.districts.iter().filter(|d| d.dark).count() as u32;
    let sites = world.jobs2.sites.len() as u32;
    let target: u32 = world.jobs2.civic.iter().map(|&x| u32::from(x)).sum();
    let row = ownership::jobs2_row(world);
    row.emp_produce = by_model[0];
    row.emp_service = by_model[1];
    row.emp_handling = by_model[2];
    row.emp_contract = by_model[3];
    row.emp_civic = by_model[4];
    row.emp_overhead = by_model[5];
    row.emp_resale = by_model[6];
    row.emp_day = by_model[7];
    row.emp_packing = vals.0;
    row.emp_freight = vals.1;
    row.emp_utility = vals.2;
    row.emp_construction = vals.3;
    row.emp_hq = vals.4;
    row.n_packing = vals.5[0];
    row.n_freight = vals.5[1];
    row.n_utility = vals.5[2];
    row.n_construction = vals.5[3];
    row.hqs = vals.5[4];
    row.jailers = vals.6[0];
    row.clerks = vals.6[1];
    row.depot_hands = vals.6[2];
    row.civic3_target = target;
    row.office_workers = office;
    row.dark_districts = dark;
    row.sites_open = sites;
}

// ---------------------------------------------------------------------------
// The `day` and `resale` models (§ 1): phase 2's users
// ---------------------------------------------------------------------------

/// § 1 (`day`): a one-day post's Job ends at its `until` (no layoff event:
/// the shift was the post). Daily from `jobs2::daily`'s caller in phase 2;
/// O(day posts) through `World::workers` of the `day` trades. Returns the
/// posts ended.
pub fn end_day_posts(world: &mut World) -> u32 {
    let now = world.tick;
    let roles: Vec<Role> = world
        .config
        .trade_roles()
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| world.config.trades.get(i).is_some_and(|r| r.model == TradeModel::Day))
        .map(|(_, r)| r)
        .collect();
    let mut ended = 0;
    for role in roles {
        for a in world.workers(role).to_vec() {
            if world.comp::<Job>(a).and_then(|j| j.until).is_some_and(|t| t <= now) {
                world.abort_plan(a);
                world.remove::<Job>(a);
                ended += 1;
            }
        }
    }
    ended
}

/// § 1 (`resale`): `worker` buys `units` of food at `market`'s price
/// (`Flow::Wholesale` to its owner, as far as the wallet goes) into its
/// own stock. Returns `(units bought, coins paid)`.
pub fn resale_buy(world: &mut World, worker: EntityId, market: EntityId, units: u32) -> (u32, i64) {
    let price = world.comp::<crate::components::Market>(market).map_or(0, |m| m.price_food).max(1);
    let stock = world.comp::<Building>(market).map_or(0, |bd| bd.stock_food);
    let afford = (world.purse(Some(worker)).max(0) / price).min(i64::from(u32::MAX)) as u32;
    let n = units.min(stock).min(afford);
    if n == 0 {
        return (0, 0);
    }
    let owner = world.owner_of(market);
    let paid = ownership::pay(world, Some(worker), owner, i64::from(n) * price, Flow::Wholesale);
    ownership::credit(world, market, paid);
    if let Some(bd) = world.comp_mut::<Building>(market) {
        bd.stock_food -= n;
    }
    if let Some(inv) = world.comp_mut::<Inventory>(worker) {
        inv.food += n;
    }
    (n, paid)
}

/// § 1 (`resale`): `worker` sells one of its units to `buyer` at `price ×
/// markup` (rounded, at least the price), `Flow::Food` (the buyer pays as
/// far as its wallet holds the whole price). Returns the coins paid.
pub fn resale_sell(world: &mut World, worker: EntityId, buyer: EntityId, price: i64, markup: f32) -> i64 {
    let ask = ((price as f32 * markup).round() as i64).max(price).max(1);
    if world.comp::<Inventory>(worker).is_none_or(|i| i.food == 0) || world.purse(Some(buyer)) < ask {
        return 0;
    }
    let paid = ownership::pay(world, Some(buyer), Some(worker), ask, Flow::Food);
    if paid > 0 {
        if let Some(inv) = world.comp_mut::<Inventory>(worker) {
            inv.food -= 1;
        }
        if let Some(inv) = world.comp_mut::<Inventory>(buyer) {
            inv.food += 1;
        }
    }
    paid
}
