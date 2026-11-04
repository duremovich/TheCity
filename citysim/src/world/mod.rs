//! The `World`: entity arena, component stores, graph, blackboard and the
//! fixed-order tick.

use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::components::*;
use crate::config::Config;
use crate::entity::{Component, EntityId};
use crate::events::Event;
use crate::exec::{FlowCache, Reservation};
use crate::levers::{Levers, PlayerCommand};
use crate::map::Map;
use crate::rng::SimRng;
use crate::stats::DailyStats;
use crate::systems;
use crate::time::{self, Tick, DAYS_PER_YEAR, TICKS_PER_DAY};

mod indices;
mod mutations;
mod owners;
mod queries;
mod spawn;
mod stat_table;
mod tick;

pub use spawn::*;
pub use stat_table::*;

/// Position of a role in [`World::roles`]: a bespoke role's in `Role::ALL`,
/// a trade's `19 + its row` (J11).
fn role_index(role: Role) -> usize {
    match role {
        Role::Trade(t) => Role::ALL.len() + t.index(),
        _ => Role::ALL.iter().position(|&r| r == role).expect("every bespoke role is in Role::ALL"),
    }
}

/// Plan-queue key: highest urgency first, then lowest id. `Reverse` has no
/// serde impl, so the ordering is flipped by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Urgency(pub OrderedFloat<f32>);

impl PartialOrd for Urgency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Urgency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.cmp(&self.0)
    }
}

/// The view milestone V7: one watched step onto a crossing (`World::crossing_watch`): the tick, the tile,
/// its junction, the ms waited at the kerb (`u32::MAX`: a jaywalk onto a carriageway).
pub type CrossingSeen = (Tick, TilePos, TilePos, u32);

/// The `World` struct and its component stores, from one declaration: the
/// plain fields as written, then `components { name: Type, }`, each the store
/// `pub name: Vec<Option<Type>>` (index = `EntityId.index`; docs and serde
/// attributes kept). A component is one line there: the macro writes its
/// `Component` impl, its slot in `grow_components` / `clear_components` (so
/// spawn and despawn can never miss a store), in `resize_components` (a load
/// sizes every store to the arena) and its empty store in `World::assemble`
/// (`World::new` builds the plain fields as a `WorldParts`).
macro_rules! world {
    (
        $(#[$sattr:meta])*
        pub struct World {
            $( $(#[$pattr:meta])* $pvis:vis $pfield:ident : $pty:ty ),* $(,)?
        }
        components {
            $( $(#[$cattr:meta])* $cfield:ident : $cty:ty ),* $(,)?
        }
    ) => {
        $(#[$sattr])*
        pub struct World {
            $( $(#[$pattr])* $pvis $pfield: $pty, )*
            $( $(#[$cattr])* pub $cfield: Vec<Option<$cty>>, )*
        }

        /// `World`'s plain fields, as `World::new` builds them (`World::assemble`
        /// adds the empty component stores).
        struct WorldParts {
            $( $pfield: $pty, )*
        }

        $(
            impl Component for $cty {
                fn store(world: &World) -> &Vec<Option<Self>> { &world.$cfield }
                fn store_mut(world: &mut World) -> &mut Vec<Option<Self>> { &mut world.$cfield }
            }
        )*

        impl World {
            fn assemble(p: WorldParts) -> World {
                World { $( $pfield: p.$pfield, )* $( $cfield: Vec::new(), )* }
            }

            pub(crate) fn grow_components(&mut self) {
                $( self.$cfield.push(None); )*
            }

            pub(crate) fn clear_components(&mut self, i: usize) {
                $( self.$cfield[i] = None; )*
            }

            /// Every store at least the arena's length (a save leaves empty stores out).
            fn resize_components(&mut self) {
                let n = self.alive.len();
                $( self.$cfield.resize_with(n.max(self.$cfield.len()), || None); )*
            }
        }
    };
}

world! {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct World {
        pub tick: Tick,
        pub rng: SimRng,
        pub config: Config,
        pub map: Map,
        // entity allocator
        pub generations: Vec<u32>,
        /// Freed slots; `spawn` always reuses the lowest.
        pub free_list: BTreeSet<u32>,
        pub alive: Vec<bool>,
        /// M13 D2: assets by the building or agent their `loc` names, each
        /// list ascending; kept by `assets::set_loc`, rebuilt on load.
        #[serde(skip)]
        pub assets_at: BTreeMap<EntityId, SmallVec<[EntityId; 4]>>,
        /// M13 D2: assets by owner (the city under `EntityId::NONE`); kept by `assets::set_owner`.
        #[serde(skip)]
        pub assets_by_owner: BTreeMap<EntityId, SmallVec<[EntityId; 4]>>,
        /// M13 D2: every vehicle, ascending.
        #[serde(skip)]
        pub vehicles: Vec<EntityId>,
        /// M13 D2: assets held by an unbound Abducted hole.
        #[serde(skip)]
        pub limbo: BTreeMap<HoleId, SmallVec<[EntityId; 4]>>,
        /// M13 D2/D13: corpses whose loot is not settled, ascending.
        #[serde(skip)]
        pub loot_corpses: Vec<EntityId>,
        /// M13 D20: vehicle trips in progress (phase 2 fills it).
        #[serde(default)]
        pub trips: BTreeMap<EntityId, Trip>,
        /// M13 D25: drivers the god `Chase` pinned (phase 2).
        #[serde(default)]
        pub chase_pins: BTreeSet<EntityId>,
        /// M13 D33 (phase 3): agents in a cyberpsychotic episode (ended hourly
        /// at `Body.episode_until`, at arrest or at death).
        #[serde(default)]
        pub episodes: BTreeSet<EntityId>,
        /// M13 D38: Bar -> registered dealers (a `Deal` in progress), ascending.
        #[serde(default)]
        pub dealers: BTreeMap<EntityId, Vec<EntityId>>,
        /// M13 D39: Bar -> the last dealer registered there and the day it was
        /// (the Statistical addict pass buys from yesterday's dealers).
        #[serde(default)]
        pub deal_log: BTreeMap<EntityId, (EntityId, u64)>,
        /// Lives at work phase 4b (§ 2.3): corner -> the dealers standing it (a
        /// corner `Deal` in progress), ascending; `stims::deal_corner`. Written
        /// only with `[stims] deal_corner` on.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub corner_dealers: BTreeMap<TilePos, Vec<EntityId>>,
        /// Lives at work phase 4b: corner -> its last dealer and the day (the
        /// Statistical buyers' pass reads it beside `deal_log`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub corner_log: BTreeMap<TilePos, (EntityId, u64)>,
        /// Pressure pass round 7, W5 (`OPEN-237`): dealer -> his stand in
        /// progress (`stims::Stand`: the sales' witnesses and his cut, told and
        /// paid at the stand's end). Written while it holds one.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub stands: BTreeMap<EntityId, crate::systems::stims::Stand>,
        /// M13 D42: robot -> the Security corp that sold it (a robot in service
        /// counts in its seller's Security share as a contract does).
        #[serde(default)]
        pub robot_sellers: BTreeMap<EntityId, EntityId>,
        /// M13 D49: a `GoTo` toward the agent's workplace in progress: (start
        /// tick, Manhattan tiles door to door).
        #[serde(default)]
        pub commute_start: BTreeMap<EntityId, (Tick, u32)>,
        /// M13 D49: today's commute sums: walked ticks, walked tiles, driven
        /// ticks, driven tiles (`commute_tpt_*` at the day's close).
        #[serde(default)]
        pub commute_acc: [u64; 4],
        /// Transit (docs/TRANSIT.md): the El's derived network, its closures and
        /// day ledger (not saved while idle).
        #[serde(default, skip_serializing_if = "systems::transit::TransitState::is_idle")]
        pub transit: systems::transit::TransitState,
        /// Transit T12: rides in progress, by rider (not saved while empty).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub rides: BTreeMap<EntityId, systems::transit::Ride>,
        /// The pre-view pass phase 5 (OPEN-139): a walking `GoTo` on a segment,
        /// by walker: the plan step it belongs to (`exec::legs`; not saved while
        /// empty).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub segments: BTreeMap<EntityId, (Tick, u8)>,
        /// The island pass phase 5 (I3, `systems/motion.rs`): the vehicles in
        /// motion, by vehicle (not saved while empty).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub motions: BTreeMap<EntityId, systems::motion::Motion>,
        /// The island pass phase 5: rider -> the vehicle he is aboard (not
        /// saved while empty).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub aboard: BTreeMap<EntityId, EntityId>,
        /// The island pass phase 7 (OPEN-166): the prisoner transfers on the
        /// road, by van (`stations.rs` § the van; not saved while empty).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub van_runs: BTreeMap<EntityId, systems::stations::VanRun>,
        /// Measurement only (never saved, read by nothing in the sim): when
        /// `Some`, every commute arrival `(ticks, Manhattan tiles, mode)` with
        /// mode 0 walked, 1 drove, 2 rode.
        #[serde(skip)]
        pub commute_log: Option<Vec<(u32, u32, u8)>>,
        /// Measurement only (never saved, read by nothing in the sim): the
        /// pre-view pass phase 5's GoTo leg log (`exec::leglog`, `run --legs`).
        #[serde(skip)]
        pub leg_log: Option<crate::exec::leglog::LegLog>,
        /// Measurement only (never saved, read by nothing in the sim): the view
        /// milestone V7's crossings of the watched bodies (the shadow's picks):
        /// by body, each step onto a marked crossing (tick, its tile, its
        /// junction, the ms waited at the kerb) or onto a carriageway off one
        /// (`u32::MAX` waited: a jaywalk), drained by the reader.
        #[serde(skip)]
        pub crossing_watch: Option<std::collections::BTreeMap<EntityId, Vec<CrossingSeen>>>,
        // graph + blackboard
        pub edges: crate::edge_map::EdgeMap,
        /// Private so every write goes through `reports_mut`, which drops the
        /// per-suspect index; read with `crime_reports()`.
        crime_reports: Vec<CrimeReport>,
        /// Per suspect: (open reports, latest report tick). Built lazily from
        /// `crime_reports` on the first read after a write (M10 phase 5b: the
        /// warrant checks walked every report per citizen per hour).
        #[serde(skip)]
        report_index: std::sync::OnceLock<ReportIndex>,
        /// Every gang, ascending by id; kept by the Gang insert/remove hooks and
        /// rebuilt on load (`gangs()` was a full entity scan).
        #[serde(skip)]
        gang_ids: Vec<EntityId>,
        /// Every agent with a `Sentence`, ascending; kept by the Sentence
        /// insert/remove hooks and rebuilt on load (`releases` scanned every
        /// entity each tick).
        #[serde(skip)]
        sentenced_ids: Vec<EntityId>,
        /// Every corp, ascending by id; kept by the Corp insert/remove hooks and
        /// rebuilt on load (a Corp store scan is ~600 bytes a slot, and the corp
        /// brain asks several times a rescoring).
        #[serde(skip)]
        corp_ids: Vec<EntityId>,
        /// Residents by Home, each list ascending; kept by the Household hooks
        /// and `set_home`, rebuilt on load (M10 review: the per-Home
        /// `citizens()` scans). `resident_home` is the reverse, so a removed
        /// Household can be unfiled.
        #[serde(skip)]
        residents: BTreeMap<EntityId, Vec<EntityId>>,
        #[serde(skip)]
        resident_home: BTreeMap<EntityId, EntityId>,
        /// `mean_price` for the tick it was read at, for the per-agent wealth
        /// refresh; `economy::daily_price` drops it.
        #[serde(skip)]
        pub(crate) mean_price_cache: Option<(Tick, i64)>,
        /// M13 review: the Shop offer a think scored (the tick, the offer), read
        /// by the same tick's `plan_for` instead of computing it twice more;
        /// cleared at each think pass.
        #[serde(skip)]
        pub shop_offers: BTreeMap<EntityId, (Tick, Option<crate::systems::assets::ShopOffer>)>,
        /// M11: a corp's pending shocks reached `[corps] shock_severity_rethink`
        /// (set by `ownership::push_corp_shock`), so `corp_brain::run` scans the
        /// corps this tick. M15 phase 2 (determinism): saved with the corps'
        /// pending shocks.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub corp_rethink: bool,
        /// Every building by kind, ascending. Private since the perf pass
        /// phase 2: read through `buildings_by_kind()` / `buildings_of_kind`,
        /// written only through `buildings_by_kind_mut()`, which bumps
        /// `kinds_gen` (the venue index's invalidation point).
        buildings_by_kind: BTreeMap<BuildingKind, Vec<EntityId>>,
        /// Rebuilt each tick for Full agents.
        pub agents_by_tile: BTreeMap<TilePos, Vec<EntityId>>,
        pub levers: Levers,
        pub stats: DailyStats,
        /// Ring of 50,000, never drained; the UI keeps a read cursor.
        pub events: VecDeque<Event>,
        /// The last `DEBUG_RING_CAP` `PlanAborted` events; not saved.
        #[serde(skip)]
        pub debug_events: VecDeque<Event>,
        /// The id the next `push_event` assigns (M10 D12).
        #[serde(default)]
        pub next_event_id: u64,
        /// Value = enqueue tick.
        pub plan_queue: BTreeMap<(Urgency, EntityId), Tick>,
        /// Shadow V4 review 8: `plan_queue` by enqueue tick, for bug 20's aging
        /// (`plan::run` read the oldest by scanning the queue on every pop). An
        /// entry is live while `plan_queue` holds its key at its tick; removals
        /// elsewhere (`retain`) leave it to be skipped. Not saved: rebuilt on
        /// load (`rebuild_indices`).
        #[serde(skip)]
        pub plan_queue_age: BTreeSet<(Tick, Urgency, EntityId)>,
        /// Hunger as sickness P5h (a printed finding): queue entries dropped
        /// stale (`plan::PLAN_QUEUE_MAX_AGE`), counted since world creation.
        #[serde(default, skip_serializing_if = "is_zero_u64")]
        pub plan_stale_drops: u64,
        pub reservations: BTreeMap<EntityId, Vec<Reservation>>,
        /// Scale S4 perf: per building, every holder of a reservation there
        /// (`FoodUnits`, `Bed`, `Seat`, `OpenMeals`), ascending; kept by
        /// `reserve`, `release_all` and `sweep_reservations`, rebuilt on load.
        #[serde(skip)]
        pub(crate) res_by_building: BTreeMap<EntityId, BTreeSet<EntityId>>,
        /// The perf pass phase 4: per agent held by a `Partner` reservation,
        /// each holder and how many it holds on them; kept by `reserve`,
        /// `release_all`, `sweep_reservations` and a death, rebuilt on load
        /// (`social::LazyReserved` reads it, not every reservation in the city).
        #[serde(skip)]
        pub(crate) res_partners: BTreeMap<EntityId, BTreeMap<EntityId, u32>>,
        pub door_queue: BTreeMap<TilePos, u8>,
        /// Pressure pass round 4, T3 (`OPEN-222`): per guard, the chase another
        /// guard handed him (`law::chase_dropped`): `(suspect, tick)`, his collar
        /// for `[justice] collar_hours` (`law::collar_of`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub handoffs: BTreeMap<EntityId, (EntityId, Tick)>,
        /// Lazily built, LRU-evicted (`[exec] flow_field_cache`); pure functions of map and door.
        #[serde(skip)]
        pub flow_fields: FlowCache,
        /// Review finding 10: the tiles the big-map landmarks were measured on
        /// while a closure left them older than the map's (`FlowCache::
        /// stale_base`), so a load measures the same landmarks. `None` on the
        /// 2,000 city (flow fields, no landmarks) and whenever they are current.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub landmark_base: Option<crate::exec::bigpath::BaseTiles>,
        /// Where and when the law last saw each suspect (read with
        /// `last_seen()`, written with `set_last_seen` / `remove_last_seen`).
        last_seen: BTreeMap<EntityId, (TilePos, Tick)>,
        /// Scale S1: `last_seen` by its tick, `(tick, suspect)` (not saved;
        /// rebuilt on load). The law's "located" window reads the recent end
        /// instead of every open warrant. Written only by `set_last_seen` and
        /// `remove_last_seen`.
        #[serde(skip)]
        last_seen_by_tick: BTreeSet<(Tick, EntityId)>,
        /// Scale S1: bumped by every write to `last_seen` and to the reports
        /// (the key of `located_memo`; not saved).
        #[serde(skip)]
        seen_gen: u64,
        /// Scale S1: `located_candidates`' last answer, keyed by tick, window
        /// and `seen_gen` (every guard's think reads it; not saved, not cloned).
        #[serde(skip)]
        located_memo: Memo<LocatedMemo>,
        /// Scale S1: every Position by tile cell and building (`agents_near`;
        /// not saved, built on its first query).
        #[serde(skip)]
        pub(crate) pos_index: crate::spatial::PosIndex,
        /// Scale S4 perf: bumped whenever a Building is inserted or removed (the
        /// key of `building_grid`; not saved).
        #[serde(skip)]
        pub(crate) building_gen: u64,
        /// Scale S4 perf: every building by door tile (`World::building_grid`),
        /// with the `building_gen` it was built at (not saved, not cloned).
        #[serde(skip)]
        building_memo: Memo<(u64, std::sync::Arc<crate::spatial::BuildingGrid>)>,
        /// Pressure pass round 5, U8 (OPEN-229): the Homes with a chromed
        /// resident by door (`World::chromed_homes`), with the hour they were
        /// read in (`tick / TICKS_PER_HOUR`; not saved, not cloned).
        #[serde(skip)]
        chromed_memo: Memo<(Tick, std::sync::Arc<crate::spatial::BuildingGrid>)>,
        /// Pressure pass round 5, U8 (OPEN-229): the guards' tiles on a grid
        /// (`World::guard_grid`), with the tick they were read at (not saved,
        /// not cloned).
        #[serde(skip)]
        guard_memo: Memo<(Tick, std::sync::Arc<GuardGrid>)>,
        /// The perf pass phase 2: bumped by every `buildings_by_kind_mut()` (a
        /// building founded, converted, re-kinded or dropped from its kind's
        /// list; not saved).
        #[serde(skip)]
        kinds_gen: u64,
        /// The perf pass phase 2 (the venue index): one grid per kind over
        /// `buildings_of_kind(kind)` by door (`World::kind_grid`), all keyed by
        /// the `(building_gen, kinds_gen)` they were built at (not saved, not
        /// cloned: a loaded world rebuilds on its first query).
        #[serde(skip)]
        kind_memo: Memo<KindGrids>,
        /// The perf pass phase 4: bumped by every write path of the Building,
        /// Corp and Market stores (`comp_mut`, `insert`, `remove`, `despawn`):
        /// the key of the price-keyed caches (`shop_memo`; not saved).
        #[serde(skip)]
        pub(crate) shop_gen: u64,
        /// The perf pass phase 4: `assets::lowest_level` per seller kind and
        /// `mean_price`, keyed by the `(shop_gen, kinds_gen)` they were read at
        /// (not saved, not cloned).
        #[serde(skip)]
        pub(crate) shop_memo: Memo<ShopMemo>,
        /// The view milestone V2b (OPEN-265): `World::venue_occupants`, keyed
        /// by the `(shop_gen, kinds_gen)` it was read at (an occupant list is a
        /// Building write; not saved, not cloned).
        #[serde(skip)]
        venue_memo: Memo<VenueOccupants>,
        /// The perf pass phase 4: one [`crate::spatial::NearTable`] per kind
        /// (`World::near_table`), keyed by `(building_gen, kinds_gen)` as
        /// `kind_memo` is (not saved, not cloned).
        #[serde(skip)]
        near_memo: Memo<NearTables>,
        /// The perf pass phase 5: the suspects each guard claims (an escort, an
        /// Arrest plan's target), kept by marks (`World::claimed_by_other`; not
        /// saved, not cloned: built on its first query).
        #[serde(skip)]
        claims: Memo<ClaimIndex>,
        /// The perf pass phase 7: `districts::rebuild`'s grid half with the
        /// inputs it was built from (not saved, not cloned: a loaded world
        /// builds it on its first rebuild).
        #[serde(skip)]
        pub(crate) district_geo: Memo<crate::systems::districts::DistrictGeo>,
        pub vacancies: BTreeMap<EntityId, Vec<Role>>,
        pub edge_roads: Vec<TilePos>,
        pub view_rect: Option<Rect>,
        /// The view milestone V5 (OPEN-149): the camera ring's held bodies (`systems::ring`); not saved.
        #[serde(skip)]
        pub ring: crate::systems::ring::Ring,
        pub command_queue: Vec<PlayerCommand>,
        pub command_log: Vec<(Tick, PlayerCommand)>,
        /// `None` until `citysim-cli calibrate` has produced `assets/stat_table.toml`.
        /// Not saved (M10 D28): reloaded from the assets on load, like the names.
        #[serde(skip)]
        pub stat_table: Option<StatTable>,
        /// The assets hold the pre-M10 four-row table: `run_statistical` panics
        /// with the calibrate message.
        #[serde(skip)]
        pub stat_table_legacy: bool,
        /// Experiment: the learned Statistical policy, loaded only under
        /// `[lod] policy = "mlp"` (`systems::stat_policy`). Not saved.
        #[serde(skip)]
        pub stat_mlp: Option<crate::systems::stat_policy::StatMlp>,
        /// M10: open holes, oldest first (the key packs the tick on top).
        #[serde(default)]
        pub holes: BTreeMap<HoleId, Hole>,
        /// Open holes by victim; rebuilt on load.
        #[serde(skip)]
        pub holes_by_agent: BTreeMap<EntityId, SmallVec<[HoleId; 4]>>,
        /// Victim holes of agents promoted off the Statistical tier, bound at the
        /// end of `lod::run` (M10 D32).
        #[serde(default)]
        pub bind_queue: Vec<HoleId>,
        /// Guard-on-shift ticks per zone, for `bind::zone_law_coverage`.
        #[serde(default)]
        pub zone_watch: ZoneWatch,
        /// Today's ATE and SLEPT_AT_HOME bits per agent, folded into the trace at day end.
        #[serde(default)]
        pub day_marks: BTreeMap<EntityId, u16>,
        /// BuyFood in progress: `(units, coins paid)` so a lost stock can be refunded.
        #[serde(default)]
        pub pending_purchase: BTreeMap<EntityId, (u32, i64)>,
        /// M11 D2: the fractional tax owed per payee, withheld whole coins at a time.
        #[serde(default)]
        pub tax_accum: BTreeMap<EntityId, f32>,
        /// M11: eviction ticks over the last 60 days, oldest first.
        #[serde(default)]
        pub eviction_log: VecDeque<Tick>,
        /// M11 § 7: the class aggregates (`Class::index`), recomputed daily by
        /// `classes::run` after rent and before the brains (D42).
        #[serde(default)]
        pub classes: [crate::components::ClassAggregate; 3],
        /// M11 D35: the last Strike (one per `[classes] strike_cooldown_days`).
        #[serde(default)]
        pub last_strike: Option<Tick>,
        /// M11 D34: guard-hours near each Home, rolled with `zone_watch`.
        #[serde(default)]
        pub home_watch: crate::components::HomeWatch,
        /// M12 § 1: the districts, `[districts]` row order; aggregates recomputed
        /// daily by `systems::districts::daily`. A pre-M12 save gets them from
        /// `districts::rebuild` on load.
        #[serde(default)]
        pub districts: Vec<crate::components::District>,
        /// M12 D1: one byte per tile, row-major like `Map::tiles`: the low nibble
        /// is the `DistrictId`, `districts::STREET_BIT` marks a walkable tile
        /// inside no building. Rebuilt on load and on every wall change.
        #[serde(skip)]
        pub district_grid: Vec<u8>,
        /// M12 D1: per district, a bitmask of the districts sharing a 4-neighbour edge.
        #[serde(skip)]
        pub district_adjacent: Vec<crate::components::DistrictMask>,
        /// M12 D3: guard-on-shift ticks per district, rolled with `zone_watch`.
        #[serde(default)]
        pub district_watch: crate::components::DistrictWatch,
        /// M12 D13 (phase 2 fills it): `(tick, gang, district)` of each report on a gang member.
        #[serde(default)]
        pub report_places: VecDeque<(Tick, EntityId, crate::components::DistrictId)>,
        /// M12 D29: `(tick, district, owner)` per evicted adult, 30-day window.
        #[serde(default)]
        pub eviction_places: VecDeque<(Tick, crate::components::DistrictId, Option<EntityId>)>,
        /// M12 D15: where each reported Full or Coarse vagrant was swept, so the
        /// sentence counts there and not at the Precinct the escort ends in;
        /// taken by `law::sentence`, dropped with the agent.
        #[serde(default)]
        pub vagrancy_places: BTreeMap<EntityId, crate::components::DistrictId>,
        /// M12 D16: litter, one byte per tile, row-major like `Map::tiles`
        /// (`systems::litter`); saved run-length encoded.
        #[serde(default, with = "crate::systems::litter::rle")]
        pub litter: Vec<u8>,
        /// M12 D20: Hotel bookings, guest -> (hotel, the tick the bed is theirs until).
        #[serde(default)]
        pub hotel_beds: BTreeMap<EntityId, (EntityId, Tick)>,
        /// M18 P5h: the housing errand's books: the city's list, the
        /// landlords' refusals, the capsules held, the address history
        /// (`systems::housing`).
        #[serde(default)]
        pub housing: crate::systems::housing::HousingState,
        /// M12 D23: each Sanitation worker's district today.
        #[serde(default)]
        pub sweep_beats: BTreeMap<EntityId, crate::components::DistrictId>,
        /// M12 D27: squatters by building, each list ascending; kept by the
        /// Squatter hooks, rebuilt on load.
        #[serde(skip)]
        pub squat_index: BTreeMap<EntityId, Vec<EntityId>>,
        /// M12 D30: live riots, at most `[riots] max_active_riots`.
        #[serde(default)]
        pub riots: Vec<crate::components::Riot>,
        #[serde(default)]
        pub next_riot_id: u32,
        /// M12 D30: rioter -> riot id, rebuilt from `riots` (on load and by `riot`).
        #[serde(skip)]
        pub rioter_of: BTreeMap<EntityId, u32>,
        /// Adjacency index over `edges`, kept in step by `edge_entry` / `remove_edge`;
        /// rebuilt on load.
        #[serde(skip)]
        pub neighbours: BTreeMap<EntityId, BTreeSet<EntityId>>,
        /// The perf pass phase 6 (the slope): the kin candidates, a superset of
        /// each agent's Spouse, Family and Parent edges (`World::kin_candidates`).
        /// Filed by `note_kin` where an edge is made kin (a marriage, a birth, an
        /// immigrant child, a grown child, an exile's edge, an ambient household),
        /// unfiled with the edge; an edge that stops being kin (`make_enemy`)
        /// stays filed, so a reader filters by kind as its neighbour walk did.
        /// Not saved: rebuilt exact on load.
        #[serde(skip)]
        kin_index: BTreeMap<EntityId, BTreeSet<EntityId>>,
        /// The perf pass phase 6 (the slope; `OPEN-203`): social edges created
        /// (`edge_entry`'s new pairs), since the world was built or loaded.
        /// Read by the CLI's `edges_new` column as a day's difference; not saved,
        /// read by no system.
        #[serde(skip)]
        pub edges_made: u64,
        /// The perf pass phase 6: Statistical co-worker pair-hours bonded
        /// (`lod`'s hour of co-work, one per pair an hour), since the world was
        /// built or loaded: the CLI's `coworker_pairs` column. Not saved, read by
        /// no system.
        #[serde(skip)]
        pub coworker_pairs: u64,
        /// Spouse lookup, both directions; written only by `set_spouse` / death.
        #[serde(skip)]
        pub spouses: BTreeMap<EntityId, EntityId>,
        /// Enemy edges by agent, both directions; kept by `social::reindex_kind`.
        #[serde(skip)]
        pub enemies: BTreeMap<EntityId, BTreeSet<EntityId>>,
        /// Agents by LOD (`Lod as usize`), ascending. Kept by the Brain hooks and `retier`; rebuilt on load.
        #[serde(skip)]
        pub by_tier: [Vec<EntityId>; 3],
        /// Job holders by `Role` (index in [`World::roles`]: `Role::ALL`, then
        /// the configured trades, J11), ascending. Kept by the Job hooks; rebuilt on load.
        #[serde(skip)]
        pub by_role: Vec<Vec<EntityId>>,
        /// The Statistical tier bucketed by hourly slot (`id.index % 60`), each
        /// ascending: the spread tick reads one bucket per tick. Kept with `by_tier`.
        #[serde(skip)]
        pub stat_slots: StatSlots,
        /// Name tables for births and immigrants; reloaded from assets on load.
        #[serde(skip)]
        pub names: NameTables,
        /// M14 V1: the Virt plane (nodes, links, stores; indices rebuilt on load).
        #[serde(default)]
        pub virt: crate::virt::VirtPlane,
        /// M14 V4: a hook changed an owner or a kind; `virt::run` relinks.
        /// Saved (M14 review): a save taken after a hook and before the next
        /// tick's relink would otherwise load clean and skip it.
        #[serde(default)]
        pub virt_dirty: bool,
        /// M14 V28: faction databases (the city under `EntityId::NONE`).
        #[serde(default)]
        pub db: BTreeMap<EntityId, crate::virt::FactionDb>,
        /// M14 V10: runs in progress and their due steps (phase 2).
        #[serde(default)]
        pub runs: BTreeMap<crate::virt::RunId, crate::virt::Run>,
        #[serde(default)]
        pub run_queue: BTreeSet<(Tick, crate::virt::RunId)>,
        /// M14 V11: one order per runner (phase 2).
        #[serde(default)]
        pub run_orders: BTreeMap<EntityId, crate::virt::RunOrder>,
        /// M14 V10: the last 64 finished runs.
        #[serde(default)]
        pub run_log: VecDeque<crate::virt::Run>,
        /// M14 V15: each runner's newest trace `(chair, tick)`.
        #[serde(default)]
        pub last_trace: BTreeMap<EntityId, (EntityId, Tick)>,
        /// M14 V10: runner -> run, rebuilt from `runs`.
        #[serde(skip)]
        pub runner_of: BTreeMap<EntityId, crate::virt::RunId>,
        /// Hunger as sickness § 4: buildings whose bin holds scraps (rebuilt on load).
        #[serde(skip)]
        pub bins: BTreeSet<EntityId>,
        /// M15 W6: one rumour pool per district (`DistrictId` order).
        #[serde(default)]
        pub rumours: Vec<crate::word::RumourPool>,
        /// M15 W14: faction x faction regard, rebuilt daily.
        #[serde(default)]
        pub regard: BTreeMap<(EntityId, EntityId), crate::word::Regard>,
        /// M15 W48: `(tick, actor)` of each noticed Murder and bound killing (cap 256).
        #[serde(default)]
        pub kill_watch: VecDeque<(Tick, EntityId)>,
        /// M15 W48: `known_by` of each watched killer seven days on (the gate's whole-run median).
        #[serde(default)]
        pub kill_known: Vec<u16>,
        /// M15 § 2 `law_heat`: `(tick, suspect)` of each sentence, 30 days (the law's own records).
        #[serde(default)]
        pub arrest_log: VecDeque<(Tick, EntityId)>,
        /// M15: `(deed, object, deed day)` of every unnamed rumour handed to a
        /// heard store, so `gossip::name_hole` walks the heard stores only when
        /// there is something to rename; pruned daily past the hole TTL. Saved
        /// (a bind after a load must rename the same copies).
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        pub anon_heard: BTreeSet<(crate::word::Deed, EntityId, u64)>,
        /// M15 W28: the adult city means of the skills competence reads
        /// (`moves::MEAN_*` order), computed at seed,
        /// never updated (so drift and deaths move competence).
        #[serde(default)]
        pub skill_means: [f32; 8],
        /// The `shadow` tool's notes: `None` unless the tool turned it on.
        #[serde(skip)]
        pub shadow_notes: Option<Vec<crate::word::ShadowNote>>,
        /// Phase 2 review: per `MEAN_*` slot, the adults' 101 percentiles at
        /// seed (`competence::norm`); computed at seed.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub skill_quantiles: Vec<Vec<f32>>,
        /// M15 W28: per corp or the Law, today's departure with the largest
        /// competence share (`kill_by`, `vacate_job`), read by the TalentLost text.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub talent_gone: BTreeMap<EntityId, crate::word::TalentGone>,
        /// M15 W27: per extortion attempt `(actor dread >= 0.5, target has an
        /// ally within 8, succeeded)`, newest last, cap 4,096 (the gate's two
        /// comparisons).
        #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
        pub extort_log: VecDeque<(bool, bool, bool)>,
        /// M15 W20: the Hunts under way, by hunter (at most `[hunt] max_hunts`
        /// but for god Hunts).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub hunts: BTreeMap<EntityId, crate::word::HuntState>,
        /// M15 W22: hunted -> hunter for every Hunt in `Watch` (never
        /// Statistical while here); rebuilt from `hunts` (`hunt::reindex`).
        #[serde(skip)]
        pub hunted_by: BTreeMap<EntityId, EntityId>,
        /// M15 W15: a Hunt's victim -> (the chain its avenger's grudge had + 1,
        /// when); a grudge over that victim starts at this chain. Pruned after
        /// 60 days.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub kill_chain: BTreeMap<EntityId, (u8, Tick)>,
        /// M15 W18: the open feuds between factions.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub vendettas: Vec<crate::word::Vendetta>,
        /// The Hit market (decision 4): the ticks corps posted a Rival Hit on an
        /// exec, the last 365 days (the city-wide cap).
        #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
        pub hm_corp_hits: std::collections::VecDeque<Tick>,
        /// The Hit market (H8, H9; phase 2): agent buyers walking to a Fixer
        /// to commission a Hit or a Beat, by buyer.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub wants: BTreeMap<EntityId, crate::contract::Want>,
        /// The Hit market (Debt, phase 2): `(creditor, debtor)` -> the debt's
        /// clock (first seen at `debt_min`, the last Debt Beat).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub hm_debts: BTreeMap<(EntityId, EntityId), crate::contract::DebtClock>,
        /// The Hit market (gaps pass, decision 3's grace): `(taker, buyer)` ->
        /// (the tick the buyer was stiffed, the grudge weight it swore): the
        /// taker's grudge on the buyer counts toward no Hunt until `debt_days`
        /// run out with the balance unpaid (`hitmarket::stiff_sweep`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub hm_stiffs: BTreeMap<(EntityId, EntityId), (Tick, f32)>,
        /// M15 W35: a body -> the agents standing over it (GuardBody's wait);
        /// rebuilt from the guards' plans on load.
        #[serde(skip)]
        pub guards_of_corpse: BTreeMap<EntityId, SmallVec<[EntityId; 2]>>,
        /// M15 W37: the last 256 stories the Feeds ran, oldest first.
        #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
        pub stories: VecDeque<crate::word::Story>,
        /// M15 W37: the next `Story.id`.
        #[serde(default)]
        pub next_story_id: u32,
        /// L2 (L24): the order-rates ledger (phase 3 the actor side).
        #[serde(default, skip_serializing_if = "crate::ledger::OrderRates::is_empty")]
        pub order_rates: crate::ledger::OrderRates,
        /// L2 (L24): the order and vendetta sources touching each district
        /// today (phase 4: rebuilt at 01:00, after the midnight rescore; live
        /// riots and episodes are read live). Saved (deviation from L24's
        /// `serde(skip)`): rebuilt from a loaded mid-day state it would read
        /// that state's orders and territory, not the 01:00 ones, and the
        /// save/load identity would break.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub fv_active: Vec<crate::ledger::ActiveSource>,
        /// L2 phase 4: the day's kill tallies by tier and the parity tallies
        /// (`ledger::FvTally`); zero with `[fviolence]` off.
        #[serde(default, skip_serializing_if = "crate::ledger::FvTally::is_zero")]
        pub fv_tally: crate::ledger::FvTally,
        /// L2 (L21): when each held prisoner's needs were last settled (it was
        /// demoted into the hold, or the last midnight pass).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub held_since: BTreeMap<EntityId, Tick>,
        /// M15 W41: the opening Feeds were seeded (at `World::new`, or by
        /// `news::migrate` on the first load of an older save).
        #[serde(default)]
        pub feeds_seeded: bool,
        /// Review fix: the save format this world was last written or migrated
        /// at (`save::SAVE_VERSION`); 0 in a save from before the field, which
        /// `save::from_ron` reads to pick the migrations to run, rather than a
        /// sentinel on the data.
        #[serde(default)]
        pub save_version: u16,
        // --- Life pass L2 phase 1 (plan L4, L10, L11): every field below is
        // written only behind an L2 `on()`; at its default it is not saved.
        /// L2 L10: the Treasury band's state.
        #[serde(default, skip_serializing_if = "crate::living::CityBudget::is_default")]
        pub budget: crate::living::CityBudget,
        /// L2 L11: the outside world's account book (the World account).
        #[serde(default, skip_serializing_if = "crate::outside::Outside::is_empty")]
        pub outside: crate::outside::Outside,
        /// L2 L9: scrap picked by scavengers, turned into Recycler Parts at midnight.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub scrap: u32,
        /// L2 L4: imports by corp, public-works hires, today's sweepers.
        #[serde(default, skip_serializing_if = "crate::living::JobsBook::is_empty")]
        pub jobs_book: crate::living::JobsBook,
        /// L2 L10: open public-works vacancies at the Recycler (a hire into one
        /// joins `jobs_book.works`).
        #[serde(default, skip_serializing_if = "is_zero_u16")]
        pub works_vacancies: u16,
        /// L2 L7: the venues and Fabs were seeded (at `World::new`, or at the
        /// first midnight after loading an older save with L2 on).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub venues_seeded: bool,
        /// L2 L7: seeding deferred to the next midnight (`living::run`).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub venues_due: bool,
        // --- Life pass L2 phase 2 (plan L14-L16): written only with `leisure::on`.
        /// L2 L14: the leisure pick an `Unwind`, HangOut or Preach plan was
        /// built for (its spot is `LocationKey::Spot`); pruned at midnight.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub unwind: BTreeMap<EntityId, crate::living::UnwindPick>,
        /// L2 L15: who is hanging out (or preaching) at each spot now; rebuilt
        /// in `rebuild_indices` from the bodies' running steps.
        #[serde(skip)]
        pub hangouts: BTreeMap<TilePos, SmallVec<[EntityId; 8]>>,
        /// Lives at work phase 1 (§ 1.1): each server's room this hour (the
        /// encounters taken against its row's `per_hour`), written by
        /// `service::arrive`, pruned by `service::hourly`. Saved (OPEN-113,
        /// SAVE_VERSION 44): a save inside an hour keeps the room taken, so a
        /// mid-hour save resumes byte for byte. Empty off.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub service_room: BTreeMap<EntityId, crate::systems::service::Room>,
        /// The pre-view pass phase 3 (OPEN-145, `[shift.counter]`): off-screen
        /// venue visits drawn at the 21:00 evening into a slot of a counter
        /// body's shift, paid and served then (`leisure::counter_due`). Saved
        /// (SAVE_VERSION 48); written only when non-empty, so an off save holds none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub counter_due: Vec<crate::systems::leisure::CounterVisit>,
        /// The pre-view pass phase 4 (P3, OPEN-119, `[presence]`): off-screen
        /// buyers' arms sales booked at midnight with a dealer who is a body,
        /// made when his body is at the Bar (`weapons::arms_meetings`). Saved
        /// (SAVE_VERSION 49); written only when non-empty.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub arms_due: Vec<crate::systems::weapons::ArmsMeet>,
        /// The pre-view pass phase 4 (P3, OPEN-119): the Fixer owner's cut of
        /// papers sold at the handover, held by the member of staff who made the
        /// sale until the owner collects in person (`service::collected`): by
        /// Fixer, (holder, coins). Saved; written only when non-empty.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub fixer_owed: BTreeMap<EntityId, Vec<(EntityId, i64)>>,
        /// The pre-view pass phase 1 (OPEN-140, the housing churn): the
        /// bodies a `Housed` line named at this tick (the midnight's first
        /// move wins; `life::housed_now`). Not saved: only the tick that wrote
        /// it reads it.
        #[serde(skip)]
        pub housed_now: (Tick, BTreeSet<EntityId>),
        /// OPEN-140: a body moved into a new Home -> that Home, until it first
        /// walks in (`enter_building`): it idles and sleeps there, not where
        /// the move found it (`life::moving_in`). Saved (SAVE_VERSION 46);
        /// empty off.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub moving_in: BTreeMap<EntityId, EntityId>,
        /// Lives at work phase 1 (§ 1.1): each server's regulars (at most
        /// `service::REGULARS_CAP`; the oldest falls off). Empty off, so the
        /// off save is byte-identical.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub regulars: BTreeMap<EntityId, SmallVec<[crate::systems::service::Regular; 8]>>,
        /// Lives at work phase 2 (§ 3.5): the visits due today, by visitor
        /// (`household::visits_due` at midnight; a Full visitor's `Visit` goal,
        /// a Statistical one credited at the hour). Empty off.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub visits: BTreeMap<EntityId, crate::systems::household::PendingVisit>,
        /// Lives at work phase 3 (§ 2.2, super): the tenants served notice today
        /// (`(tenant, day)`), one notice a tenant a day; cleared at midnight
        /// (`service::hourly`). Saved while it holds anything (a resumed save
        /// serves the same notices), empty off.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        pub notices_today: BTreeSet<(EntityId, u64)>,
        /// The pressure pass R4 (OPEN-219): the tenants a super has served
        /// notice on in the round he is walking, by super; one `Notice` line a
        /// round names them (`service::notices_folded`). Saved while it holds anything.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub notice_fold: BTreeMap<EntityId, Vec<EntityId>>,
        /// Lives at work phase 5 (§ 2.4, exec): each corp's last Meeting table
        /// (the attendees of its exec's last Meeting hour; the decision names
        /// them). Saved while it holds anything (deviation: the brief's unsaved
        /// map would name nobody after a resume); empty off.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub meeting_tables: BTreeMap<EntityId, SmallVec<[EntityId; 6]>>,
        /// Round 4 T6: each ledger's books, by owner (a corp, `EntityId::NONE`
        /// the city's): the runs they caught, `(tick, runner)`, `[hack]
        /// ledger_audit_days`, newest last (`virt::audit_ledger`). Saved while
        /// it holds anything.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub ledger_books: BTreeMap<EntityId, std::collections::VecDeque<(Tick, EntityId)>>,
        /// Lives at work phase 5 (§ 2.4, reporter): each reporter's beat this
        /// shift (`news::beat_plan`): the scenes and the deeds interviewed.
        /// Saved while it holds anything; empty off.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub beats: BTreeMap<EntityId, crate::systems::news::Beat>,
        /// Round 3 S5 (OPEN-217): the crime desk: the last day's first tellings
        /// of a deed with a `deed_sev` above 0 (`gossip::post_deed_at`), which
        /// the beat reads beside the pools (`news::beat_stops`): a district's
        /// pool holds `pool_cap` entries at any population, and at 100k the
        /// first-hand ones were shed for hearsay by day 4 (no beat stop).
        /// Pruned at midnight (`news::daily`); empty with `[verbs]` absent.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub beat_desk: Vec<crate::word::PoolEntry>,
        /// Lives at work phase 5 (§ 2.4, reporter): bylines a reporter has had.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub bylines: BTreeMap<EntityId, u16>,
        /// Lives at work phase 5 (§ 2.4, fixer): the paper buyers queued at
        /// midnight for a Fixer's open hours (`gray::papers_daily`), served by
        /// `service::hourly` or at a body's arrival. Saved while pending; empty off.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub walk_ins: Vec<crate::systems::service::WalkIn>,
        /// The pressure pass phase 6 (`systems::underwater`): the businesses
        /// under water, the loans against them, the stiffed workers, the shut
        /// businesses for sale and the bought fronts. Saved while it holds anything.
        #[serde(default, skip_serializing_if = "crate::systems::underwater::State::is_empty")]
        pub underwater: crate::systems::underwater::State,
        /// The pressure pass phase 5a (`systems::consequence`): the open
        /// blackmails, the hit reposts, the prisoners' runs, today's services.
        /// Saved while it holds anything.
        #[serde(default, skip_serializing_if = "crate::systems::consequence::State::is_empty")]
        pub consequence: crate::systems::consequence::State,
        /// The pressure pass phase 5b (`systems::crew`): the gang members' duty,
        /// books (standing, bail owed, the talk), failed courts, reckonings.
        #[serde(default, skip_serializing_if = "crate::systems::crew::State::is_empty")]
        pub crew: crate::systems::crew::State,
        /// Pressure pass round 6, V2 (`systems::turf`): the cells, their
        /// holders, corners and windows of presence, the neighbours.
        #[serde(default, skip_serializing_if = "crate::systems::turf::State::is_empty")]
        pub turf: crate::systems::turf::State,
        /// Pressure pass round 2, R1 (`systems::fares`): the fare dodgers'
        /// records (caught, banned) and the beaten down on the platform.
        #[serde(default, skip_serializing_if = "crate::systems::fares::State::is_empty")]
        pub fares: crate::systems::fares::State,
        /// Pressure pass round 2, R3 (`systems::care`): the asks for treatment
        /// money, the sick workers' covers. Saved while it holds anything.
        #[serde(default, skip_serializing_if = "crate::systems::care::State::is_empty")]
        pub care: crate::systems::care::State,
        /// Pressure pass round 8, X2 (`systems::grief`): the mourners, the
        /// funerals due, the men lying low. Saved while it holds anything.
        #[serde(default, skip_serializing_if = "crate::systems::grief::State::is_empty")]
        pub grief: crate::systems::grief::State,
        /// Pressure pass round 8, X5 (`OPEN-248`): the live courtships, `(lo,
        /// hi)` (`social::dated`; a proposal waits for one's end,
        /// `social::courtship_ripe`). Pruned at the Social midnight.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub courting: BTreeMap<(EntityId, EntityId), crate::systems::social::Courtship>,
        /// L2 L15: the HangOut spots per district (`leisure::spots`, midnight).
        /// Saved (deviation: the plan's `serde(skip)` would rebuild them on load
        /// from a later state than the midnight's).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub spots: Vec<Vec<crate::living::Spot>>,
        /// L2 L16: the day two kin last met (HangOut, co-location), `(lo, hi)`.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub last_met: BTreeMap<(EntityId, EntityId), u64>,
        /// L2 shadow fixes item 17: the tick a teller last told a listener a
        /// deed, keyed `(teller, listener, deed hash)` (`fixes::told_key`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub told: BTreeMap<(EntityId, EntityId, u64), Tick>,
        /// M18 W (`systems::workbeats`): each body's beats in its current shift
        /// (an exec's: its day), one entry a body, overwritten at the next shift.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub shift_beats: BTreeMap<EntityId, crate::systems::workbeats::ShiftBeats>,
        /// L2 L14: the 90th-percentile wallet at the last midnight (the Lounge
        /// rung's "top wealth decile").
        #[serde(default, skip_serializing_if = "is_zero_i64")]
        pub wealth_p90: i64,
        // --- M16a (plan C2): contract records, a game abstraction (records
        // with a price and a deadline, matched by a score, resolved by a seeded
        // roll). Written only behind `contracts::on`; at its default not saved.
        /// C2: every record, open, taken and recently closed, by id.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub contracts: BTreeMap<crate::contract::ContractId, crate::contract::Contract>,
        /// C22 (phase 2): live missions by record.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub missions: BTreeMap<crate::contract::MissionId, crate::contract::Mission>,
        /// C14: live takers' chases, by taker.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub contract_runs: BTreeMap<EntityId, crate::contract::ContractRun>,
        /// C2: one-line outcomes, newest last (cap 512).
        #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
        pub contract_log: VecDeque<(Tick, crate::contract::ContractId, String)>,
        /// C2: the next record id (ids from 1).
        #[serde(default, skip_serializing_if = "is_zero_u64")]
        pub next_contract: u64,
        /// M16b § 4: the coercions, standing and recently closed (dropped
        /// `retry_days` after closing).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub coercions: BTreeMap<crate::contract::CoercionId, crate::contract::Coercion>,
        /// M16b § 4: the next coercion id (ids from 1).
        #[serde(default, skip_serializing_if = "is_zero_u64")]
        pub next_coercion: u64,
        /// M16b § 4: `(actor, target, purpose)` -> the last try of a leverage
        /// move between them (one try per `retry_days`), pruned past it.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub lev_tried: BTreeMap<(EntityId, EntityId, u8), Tick>,
        /// M16b § 4 Fraud: fake jobs sold, until their victims learn it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub fake_jobs: Vec<crate::contract::FakeJob>,
        /// M16b phase 2: the held agents, by captive.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub captives: BTreeMap<EntityId, crate::contract::Captive>,
        /// M16b phase 2: `(buyer corp, seller corp)` -> Acquire offers refused
        /// (two make a dirty exec reach for a hostage).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub refusals: BTreeMap<(EntityId, EntityId), u8>,
        /// M16b phase 2: a Steal heist's runner -> its record.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub steal_runs: BTreeMap<EntityId, crate::contract::ContractId>,
        /// M16b phase 2: a Feed caught running a fabrication -> its reach is
        /// cut until the tick.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub feed_shamed: BTreeMap<EntityId, Tick>,
        /// M16b phase 3: the downed, by agent.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub wounded: BTreeMap<EntityId, crate::contract::Wounded>,
        /// The combat model C1: the bodies with a `Wounds` component (the
        /// hourly bleed and heal pass reads it, not every citizen); C2: with an
        /// open wound or short of blood (one with only its marks, scars, maims
        /// and lost limbs, keeps the component out of it).
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        pub hurt: BTreeSet<EntityId>,
        /// C1: the last fight's outcome (`combat::resolve_now`), beside the
        /// `(winner, loser, died)` every caller reads. Not saved.
        #[serde(skip)]
        pub last_fight: Option<crate::systems::combat::FightOutcome>,
        /// C3a: the live engagements and their indices (`combat::Live`); not
        /// saved (a save resolves them first, `combat::settle_all`).
        #[serde(skip)]
        pub combat: Box<crate::systems::combat::Live>,
        /// M16b phase 3: client -> its Trauma corp, from the `Standing` Trauma
        /// records (rebuilt on load, `wounded::reindex`).
        #[serde(skip)]
        pub trauma_clients: BTreeMap<EntityId, EntityId>,
        /// M16b phase 4: the tags on agents.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub tags: Vec<crate::contract::Tag>,
        #[serde(default, skip_serializing_if = "is_zero_u64")]
        pub next_tag: u64,
        /// M16b phase 4: the Purist gangs that bought a Patrol's scanner.
        #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
        pub gang_scanners: std::collections::BTreeSet<EntityId>,
        /// M16b phase 4: each faction's attention (the law at `EntityId::NONE`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub attention: BTreeMap<EntityId, crate::contract::Attention>,
        /// C9: the Fixers were seeded (at `World::new`, or at the first midnight
        /// after loading an older save with contracts on).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub fixers_seeded: bool,
        /// C40: seeding deferred to the next midnight (`contracts::daily`).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub fixers_due: bool,
        /// C34 (phase 3): runner -> (Fixer, offered).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub fixer_runs: BTreeMap<EntityId, (EntityId, Tick)>,
        /// C5: the coins `escrow_in`/`escrow_out` moved into escrow, net (the
        /// `escrow_leak` probe compares it with Σ `Contract.escrow`).
        #[serde(default, skip_serializing_if = "is_zero_i64")]
        pub escrow_held: i64,
        /// C13: the median Job wage at the last midnight (the gun gate's
        /// "paid below 0.7 × the median").
        #[serde(default, skip_serializing_if = "is_zero_i64")]
        pub median_wage: i64,
        /// C14: chased target -> taker for every contract run in `Watch`;
        /// rebuilt from `contract_runs` (`hunt::reindex`).
        #[serde(skip)]
        pub chased_by: BTreeMap<EntityId, EntityId>,
        /// C2: every record by party (buyer, agent, taker, crew, target),
        /// rebuilt from `contracts` (`contracts::rebuild_index`).
        #[serde(skip)]
        pub by_party: BTreeMap<EntityId, SmallVec<[crate::contract::ContractId; 4]>>,
        /// C18: the ledger records' due ticks.
        #[serde(skip)]
        pub ledger_due: BTreeSet<(Tick, crate::contract::ContractId)>,
        /// C22 (phase 2): crew member -> mission.
        #[serde(skip)]
        pub mission_of: BTreeMap<EntityId, crate::contract::MissionId>,
        /// C27: Open or Taken Locate records by target.
        #[serde(skip)]
        pub locate_targets: BTreeMap<EntityId, SmallVec<[crate::contract::ContractId; 2]>>,
        /// C31 (phase 3): a taker -> its Taken Guard records without a broker
        /// (`contracts::on_take_of` reads which make it a city guard on the take).
        #[serde(skip)]
        pub on_take: BTreeMap<EntityId, SmallVec<[crate::contract::ContractId; 2]>>,
        /// C32: client (agent or building) -> the agents standing a Guard post on it now.
        #[serde(skip)]
        pub contract_guards: BTreeMap<EntityId, SmallVec<[EntityId; 2]>>,
        // --- The Real economy (docs/ECONOMY_V2.md, plan E3): written only behind
        // an economy `on()`; at its default it is not saved.
        #[serde(default, skip_serializing_if = "crate::econ::EconState::is_default")]
        pub econ: crate::econ::EconState,
        /// Plan 1.2: the coin census's probe counters (never saved).
        #[serde(skip)]
        pub probe: crate::econ::CoinProbe,
        // --- Jobs and room P1 (docs/JOBS_V2.md; plan J3, J4).
        /// J3: workplace -> its staff (every Job holder whose `employer` is the
        /// building), each list ascending, no empty lists. Kept by the Job hooks
        /// (`index_job`/`unindex_job`) with `employer_of`; rebuilt on load and
        /// checked by `check_indices`. Read through [`World::staff_of`].
        #[serde(skip)]
        pub employers: BTreeMap<EntityId, Vec<EntityId>>,
        /// J3: the reverse of `employers` (the Job hook runs after the old Job
        /// is gone, so the old workplace is looked up here).
        #[serde(skip)]
        pub(crate) employer_of: BTreeMap<EntityId, EntityId>,
        /// J4: agent -> (role, tick) of its last layoff (`economy::dismiss_as`),
        /// written only with wages on, pruned after `demography::REHIRE_DAYS`;
        /// `pick_candidate`'s rehire bonus reads it.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub laid_off: BTreeMap<EntityId, (Role, Tick)>,
        /// Hunger fix item 10 (P5h): employer -> consecutive midnights its
        /// housing demand went unmet (`life::grow_housing`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub housing_demand: BTreeMap<EntityId, u16>,
        /// The island pass phase 3a (I6, `systems/company.rs`): a company
        /// block -> the workplace whose staff its beds are reserved for.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub company_homes: BTreeMap<EntityId, EntityId>,
        /// The city milestone C6b (`systems/floors.rs`): host -> its tenancies
        /// and the vacant units on it; not saved, rebuilt at the seed, the load
        /// and every founding or vacated floor (`floors::reindex`).
        #[serde(skip)]
        pub floor_index: crate::systems::floors::FloorIndex,
        /// The city milestone C7 (`systems/levels.rs`): a lift core -> its
        /// lobby's calls and C8's hooks (cut, held).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub lifts: BTreeMap<EntityId, crate::systems::levels::LiftState>,
        /// The city milestone C9b (`systems/vertical.rs`): a gang's Hideout
        /// short on its floor rent -> the landlord's answer so far.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub floor_answers: BTreeMap<EntityId, crate::systems::vertical::FloorAnswer>,
        /// The island pass phase 3a: a company block's resident whose job at
        /// its workplace ended -> the tick it ended (evicted `grace_days` on).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub company_lapsed: BTreeMap<EntityId, Tick>,
        /// The island pass phase 3b: the `company_lapsed` residents put on
        /// notice by a buyout (not by a job lost), for the eviction's line.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        pub company_notice: BTreeSet<EntityId>,
        /// The island pass phase 4 (I2, `systems/commute.rs`): a worker -> his
        /// commute pressure (the far days, the next search, the cooldown).
        /// Written only when non-empty, so an off save holds none.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pub commute_pressure: BTreeMap<EntityId, crate::systems::commute::Pressure>,
        /// Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier (empty, and not
        /// saved, on a map without Ambient flats).
        #[serde(default, skip_serializing_if = "crate::ambient::AmbientState::is_empty")]
        pub ambient: crate::ambient::AmbientState,
        /// Jobs & lives phase 1 (docs/JOBS_AND_LIVES.md § 2): the industries'
        /// and the civic three's state (empty, and not saved, with `[jobs2]` off).
        #[serde(default, skip_serializing_if = "crate::systems::industry::Jobs2State::is_default")]
        pub jobs2: crate::systems::industry::Jobs2State,
    }

    // The component stores, one line each: `name: Type` is the store `name: Vec<Option<Type>>`
    // (index = `EntityId.index`), wired by `world!`. Docs and serde attributes as on a field.
    components {
        position: Position,
        identity: Identity,
        needs: Needs,
        personality: Personality,
        mood: Mood,
        wallet: Wallet,
        inventory: Inventory,
        job: Job,
        household: Household,
        brain: Brain,
        memory: Memory,
        skills: Skills,
        sentence: Sentence,
        gang_member: GangMember,
        corpse: Corpse,
        child: Child,
        /// M12 D27.
        #[serde(default)]
        squatter: Squatter,
        building: Building,
        gang: Gang,
        market: Market,
        treasury: Treasury,
        /// M9: on the Jail.
        #[serde(default)]
        law: Law,
        /// M10: per adult, kept after death.
        #[serde(default)]
        trace: Trace,
        /// M10: per agent, kept after death.
        #[serde(default)]
        life: Life,
        /// M11: corps, each its own entity.
        #[serde(default)]
        corp: Corp,
        /// M13 D1: assets, each its own entity carrying only this.
        #[serde(default)]
        asset: Asset,
        /// M13 D4: every agent's Body.
        #[serde(default)]
        body: Body,
        /// M13 D5: stored daily for M15.
        #[serde(default)]
        appearance: Appearance,
        /// M13 D3: derived from the assets, never saved; rebuilt on load.
        #[serde(skip)]
        kit: Kit,
        /// M15 W13: every living adult's and faction's reputation, rebuilt at
        /// midnight by `reputation::rebuild`; saved, so a load mid-day reads
        /// what the last midnight wrote.
        #[serde(default)]
        reputation: crate::word::Reputation,
        /// M15 W15: grudges (phase 3); empty stores are not written.
        #[serde(default, skip_serializing_if = "all_none")]
        grudges: crate::word::Grudges,
        /// M16a (plan C2): a Fixer office's record book; empty stores are not
        /// written (`resize_stores` sizes it on load).
        #[serde(default, skip_serializing_if = "all_none")]
        broker: crate::contract::Broker,
        /// Real economy E41: the `CampRaised` trait of adults raised at a work
        /// camp; empty stores are not written.
        #[serde(default, skip_serializing_if = "all_none")]
        camp_raised: crate::econ::CampRaised,
        /// Hunger as sickness § 2: the meters (only with `[sickness]` on; an
        /// empty store is not written).
        #[serde(default, skip_serializing_if = "all_none")]
        health: crate::components::Health,
        /// Class pay (docs/CLASS_PAY.md): backgrounds (only with class pay on;
        /// an empty store is not written).
        #[serde(default, skip_serializing_if = "all_none")]
        schooling: crate::components::Schooling,
        /// Jobs & lives phase 5 (§ 5.2, D9): the `Chipped` trait of agents whose
        /// chip came out (only with `[vice] chip` on; an empty store is not written).
        #[serde(default, skip_serializing_if = "all_none")]
        chipped: crate::components::Chipped,
        /// Lives at work phase 6 (§ 4.3, § 4.4): the profession and the retired
        /// mark (only with `[careers]` on; empty stores are not written).
        #[serde(default, skip_serializing_if = "all_none")]
        profession: crate::components::Profession,
        #[serde(default, skip_serializing_if = "all_none")]
        retired: crate::components::Retired,
        /// The pressure pass phase 1 (OPEN-178): misery's bookkeeping (only
        /// with `[misery]`; an empty store is not written).
        #[serde(default, skip_serializing_if = "all_none")]
        misery: crate::components::Misery,
        /// The combat model C1 (docs/COMBAT.md § 2): the wounds a body carries
        /// (an empty store is not written; `hurt` indexes it).
        #[serde(default, skip_serializing_if = "all_none")]
        wounds: crate::components::Wounds,
    }
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

/// `skip_serializing_if` for a component store with nothing in it.
fn all_none<T>(v: &[Option<T>]) -> bool {
    v.iter().all(Option::is_none)
}

use crate::components::{Law, LawShock};

/// Scale S1: a memo cell on the world: skipped by serde and empty in a
/// clone (a cache of a pure function of the world, never state).
#[derive(Debug)]
pub struct Memo<T>(std::sync::Mutex<Option<T>>);

impl<T> Default for Memo<T> {
    fn default() -> Self {
        Memo(std::sync::Mutex::new(None))
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Memo(std::sync::Mutex::new(None))
    }
}

impl<T> Memo<T> {
    /// The perf pass phase 7: the cell itself, for a writer holding the
    /// world mutably (no lock taken).
    pub(crate) fn slot(&mut self) -> &mut Option<T> {
        self.0.get_mut().unwrap_or_else(|e| e.into_inner())
    }
}

/// Pressure pass round 5, U8: every guard's tile and the grid over them
/// (`World::guard_grid`).
pub type GuardGrid = (Vec<TilePos>, crate::spatial::TileGrid);

/// The perf pass phase 2: `((building_gen, kinds_gen), each kind's grid)`.
type KindGrids = ((u64, u64), BTreeMap<BuildingKind, std::sync::Arc<crate::spatial::BuildingGrid>>);

/// The perf pass phase 4: `((building_gen, kinds_gen), each kind's table)`.
type NearTables = ((u64, u64), BTreeMap<BuildingKind, std::sync::Arc<crate::spatial::NearTable>>);

/// The view milestone V2b: `((shop_gen, kinds_gen), (the Bars' and Markets'
/// occupant lists' summed length, their occupants ascending))`.
type VenueOccupants = ((u64, u64), (usize, std::sync::Arc<Vec<EntityId>>));

/// The perf pass phase 4: what `shop_memo` holds for one `(shop_gen,
/// kinds_gen)`: each seller kind's lowest price level once read, and the
/// mean food price once read.
#[derive(Debug, Default)]
pub(crate) struct ShopMemo {
    pub key: (u64, u64),
    pub lowest: BTreeMap<BuildingKind, Option<f32>>,
    pub mean_price: Option<i64>,
}

/// The perf pass phase 5: every guard's claims on suspects (the L1 rule:
/// one guard per suspect), so `law::chaseable` asks "does another guard
/// chase `s`" without walking every guard per scoring call
/// (`life::claimed_suspects`, the scan it replaces).
///
/// Kept current by marks, as `PosIndex` is: a guard's claims are its
/// Brain's `escorting` and its Arrest plan's `target`, and the guards are
/// `World::guards()`. Every write to a Brain goes through `comp_mut`,
/// `insert`, `remove` or `despawn`, and each marks the entity when it is a
/// filed guard; `guards()` changes only in `index_job` / `unindex_job`
/// (a Job inserted, removed or re-posted, a despawn), which mark the entity
/// whatever it was. A query re-files the marked first, so an Arrest plan
/// installed, an escort begun, a guard hired or killed between two reads in
/// one tick is seen by the second.
#[derive(Debug, Default)]
pub(crate) struct ClaimIndex {
    /// Per entity index: the guard filed there and its claims `[escort,
    /// Arrest target]`, while it is in `guards()`.
    by_guard: Vec<Option<(EntityId, [Option<EntityId>; 2])>>,
    /// Per suspect: the guards claiming them, once per claim.
    claimers: BTreeMap<EntityId, SmallVec<[EntityId; 2]>>,
    dirty: Vec<u32>,
    flagged: Vec<bool>,
}

impl ClaimIndex {
    fn mark(&mut self, index: u32) {
        let i = index as usize;
        if self.flagged.len() <= i {
            self.flagged.resize(i + 1, false);
        }
        if !self.flagged[i] {
            self.flagged[i] = true;
            self.dirty.push(index);
        }
    }

    /// A Brain written: only a filed guard's claims can change.
    fn mark_brain(&mut self, index: u32) {
        if self.by_guard.get(index as usize).is_some_and(Option::is_some) {
            self.mark(index);
        }
    }

    /// The claims of guard `g` as the world holds them now.
    fn claims_of(world: &World, g: EntityId) -> [Option<EntityId>; 2] {
        world.comp::<Brain>(g).map_or([None, None], |b| {
            [b.escorting, b.plan.as_ref().filter(|p| p.goal == GoalKind::Arrest).and_then(|p| p.target)]
        })
    }

    fn file(&mut self, i: usize, now: Option<(EntityId, [Option<EntityId>; 2])>) {
        if self.by_guard.len() <= i {
            self.by_guard.resize(i + 1, None);
        }
        if self.by_guard[i] == now {
            return;
        }
        if let Some((g, old)) = self.by_guard[i].take() {
            for s in old.into_iter().flatten() {
                if let Some(v) = self.claimers.get_mut(&s) {
                    if let Some(k) = v.iter().position(|&x| x == g) {
                        v.swap_remove(k);
                    }
                    if v.is_empty() {
                        self.claimers.remove(&s);
                    }
                }
            }
        }
        if let Some((g, new)) = now {
            for s in new.into_iter().flatten() {
                self.claimers.entry(s).or_default().push(g);
            }
        }
        self.by_guard[i] = now;
    }

    fn build(world: &World) -> ClaimIndex {
        let mut ix = ClaimIndex::default();
        for &g in world.guards() {
            ix.file(g.index as usize, Some((g, Self::claims_of(world, g))));
        }
        ix
    }

    /// Re-file the marked: the live entity at each index, with its claims
    /// if it is in `guards()`.
    fn refresh(&mut self, world: &World) {
        let dirty = std::mem::take(&mut self.dirty);
        for &i in &dirty {
            self.flagged[i as usize] = false;
            let id = EntityId { index: i, generation: world.generations.get(i as usize).copied().unwrap_or(0) };
            let now = (world.is_alive(id) && world.guards().binary_search(&id).is_ok())
                .then(|| (id, Self::claims_of(world, id)));
            self.file(i as usize, now);
        }
        self.dirty = dirty;
        self.dirty.clear();
    }

    fn claimed_by_other(&self, guard: EntityId, s: EntityId) -> bool {
        self.claimers.get(&s).is_some_and(|v| v.iter().any(|&g| g != guard))
    }
}

/// `(tick, window, seen_gen)` and the open suspects seen inside the window
/// then, ascending, each with its `last_seen` entry.
type LocatedMemo = (Tick, Tick, u64, std::sync::Arc<Located>);

/// Scale S1: the open-warrant suspects seen inside the law's window,
/// ascending, each with its `last_seen` entry, and a grid of their tiles.
#[derive(Debug, PartialEq)]
pub struct Located {
    pub list: Vec<(EntityId, (TilePos, Tick))>,
    grid: crate::spatial::TileGrid,
}

impl Located {
    /// The cell of `grid`, in tiles (a city guard's reach is 64, a private one's 16).
    const CELL: u32 = 16;

    fn new(list: Vec<(EntityId, (TilePos, Tick))>, w: usize, h: usize) -> Located {
        let grid = crate::spatial::TileGrid::build(w, h, Self::CELL, list.iter().map(|&(_, (t, _))| t));
        Located { list, grid }
    }

    /// The entries last seen within the Chebyshev square of radius `r`
    /// around `t` (and maybe a few beyond it), ascending: a superset of
    /// those within `r` Manhattan tiles.
    pub fn near(&self, t: TilePos, r: u32) -> Vec<(EntityId, (TilePos, Tick))> {
        let mut idx = Vec::new();
        self.grid.near(t, r, &mut idx);
        idx.sort_unstable();
        idx.into_iter().map(|i| self.list[i as usize]).collect()
    }
}

/// Per suspect `(open reports, latest report tick)`, and the suspects with an
/// open report, ascending.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportIndex {
    pub by_suspect: BTreeMap<EntityId, (u32, Tick)>,
    pub open: Vec<EntityId>,
    /// Scale S1: per suspect, its open reports for anything but Vagrancy
    /// (only suspects with one; `law::vagrant_at_work` read every report).
    pub open_not_vagrancy: BTreeMap<EntityId, u32>,
}

/// Statistical agents by hourly slot: `slots[s]` holds the ids with
/// `index % 60 == s`, ascending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatSlots(pub Vec<Vec<EntityId>>);

impl Default for StatSlots {
    fn default() -> Self {
        StatSlots(vec![Vec::new(); time::TICKS_PER_HOUR as usize])
    }
}

impl StatSlots {
    fn slot_of(id: EntityId) -> usize {
        (u64::from(id.index) % time::TICKS_PER_HOUR) as usize
    }

    /// The bucket for `tick % 60`.
    pub fn at(&self, tick: Tick) -> &[EntityId] {
        &self.0[(tick % time::TICKS_PER_HOUR) as usize]
    }
}
