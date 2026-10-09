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

/// Rows in a v2 table: 4 phases x 3 lawfulness buckets x 2 hunger buckets.
pub const STAT_ROWS: usize = 24;

/// One row of the calibrated hourly table (M10 v2). The first four are one
/// outcome drawn per hour, as in v1; the rest are independent hourly rolls.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct StatRow {
    /// "Morning law0 hunger1", for humans.
    #[serde(default)]
    pub label: String,
    pub p_eat: f32,
    pub p_work: f32,
    pub p_social: f32,
    pub p_sleep: f32,
    /// Actor side: Market theft beyond the eat path's desperation theft.
    pub p_steal: f32,
    /// Actor side: a Flirt with a known edge of affinity >= 0.3.
    pub p_flirt: f32,
    /// Victim side: the actor is a hole until bound.
    pub p_robbed: f32,
    pub p_assaulted: f32,
    pub p_killed: f32,
    /// Independent roll: the agent meets a stranger from its zone (a new
    /// edge). `calibrate`: half the Full agents' new non-co-worker edge ends
    /// per agent-hour (each meeting gives two agents an edge), pooled over
    /// hunger like the other rolls. Full agents meet in every phase (the
    /// Market, the street, home), not in Social hours: per Social hour the
    /// rate exceeded 1 in every row. 0 in a table without it.
    #[serde(default)]
    pub p_meet: f32,
    /// Independent roll: a Chat (a known edge's drift). `calibrate`: Full
    /// Chats begun per agent-hour, pooled over hunger. A Chat is a few
    /// minutes of an hour some other category dominates, so the Social
    /// outcome (dominant hours) undercounted them by about half.
    #[serde(default)]
    pub p_chat: f32,
    /// Given a Chat: it is with a housemate rather than a random known edge.
    /// `calibrate`: the share of Full Chats begun with someone of the same
    /// Home, pooled over hunger (a Full agent chats with whoever is there,
    /// and at home that is the household).
    #[serde(default)]
    pub p_chat_home: f32,
}

impl StatRow {
    /// A copy without the human label: the hourly tick reads only the
    /// numbers, and cloning the label allocated once per agent-hour.
    /// Exhaustive, so a new field fails to compile until it is copied.
    pub fn numbers(&self) -> StatRow {
        let StatRow {
            label: _,
            p_eat,
            p_work,
            p_social,
            p_sleep,
            p_steal,
            p_flirt,
            p_robbed,
            p_assaulted,
            p_killed,
            p_meet,
            p_chat,
            p_chat_home,
        } = *self;
        StatRow {
            label: String::new(),
            p_eat,
            p_work,
            p_social,
            p_sleep,
            p_steal,
            p_flirt,
            p_robbed,
            p_assaulted,
            p_killed,
            p_meet,
            p_chat,
            p_chat_home,
        }
    }
}

/// Calibrated hourly behaviour table for Statistical agents (`citysim-cli
/// calibrate`, v2). Rows by `phase x 6 + lawfulness bucket x 2 + hunger bucket`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatTable {
    /// 2.
    pub version: u32,
    pub seed: u64,
    pub days: u64,
    pub agents: u32,
    /// `[0.3, 0.7]`: lawfulness `< e0` is bucket 0, `< e1` bucket 1, else 2.
    pub lawfulness_edges: [f32; 2],
    /// `0.4`: hunger `< edge` is bucket 0 (hungry), else 1.
    pub hunger_edge: f32,
    /// The chance a jobless Full agent who starts the Work phase below its
    /// savings line (`SAVINGS_DAYS` meals) collects the dole that day; it
    /// has to walk to the Hall. 1 in a table without it.
    #[serde(default = "one_f32")]
    pub p_dole_day: f32,
    /// The chance an off-screen theft is reported and its thief given a body
    /// (`lod::stat_theft`): Full arrests per Full theft. Off screen that
    /// report is the only road to an arrest (an Assaulted hole files its
    /// witness's report only when bound, at `hole_ttl_days`), and nearly
    /// every one ends in one, so it carries the Full city's whole arrest
    /// rate; the Full share of thefts reported (0.53) put Statistical
    /// arrests 18 % over Full. A table without it falls back to
    /// `[crime] stat_theft_caught_p`.
    #[serde(default)]
    pub p_theft_caught: Option<f32>,
    pub rows: Vec<StatRow>,
}

fn one_f32() -> f32 {
    1.0
}

impl StatTable {
    /// Morning 0, Work 1, Evening 2, Night 3 (`DayPhase` declares Night first).
    pub fn phase_index(phase: time::DayPhase) -> usize {
        match phase {
            time::DayPhase::Morning => 0,
            time::DayPhase::Work => 1,
            time::DayPhase::Evening => 2,
            time::DayPhase::Night => 3,
        }
    }

    pub fn lawfulness_bucket(&self, lawfulness: f32) -> usize {
        if lawfulness < self.lawfulness_edges[0] {
            0
        } else if lawfulness < self.lawfulness_edges[1] {
            1
        } else {
            2
        }
    }

    pub fn hunger_bucket(&self, hunger: f32) -> usize {
        usize::from(hunger >= self.hunger_edge)
    }

    /// `phase x 6 + lawfulness bucket x 2 + hunger bucket`.
    pub fn index(&self, phase: time::DayPhase, lawfulness: f32, hunger: f32) -> usize {
        Self::phase_index(phase) * 6 + self.lawfulness_bucket(lawfulness) * 2 + self.hunger_bucket(hunger)
    }

    pub fn row(&self, phase: time::DayPhase, lawfulness: f32, hunger: f32) -> &StatRow {
        &self.rows[self.index(phase, lawfulness, hunger)]
    }

    /// "Morning law0 hunger1" for row `i`.
    pub fn label(i: usize) -> String {
        let phase = ["Morning", "Work", "Evening", "Night"][i / 6];
        format!("{phase} law{} hunger{}", (i % 6) / 2, i % 2)
    }
}

/// Read `assets/stat_table.toml`: `(table, legacy)`. Missing gives `(None,
/// false)`; the pre-M10 four-row table `(None, true)` (M10 D28); a v2 table
/// with the wrong row count, or anything unparsable, panics.
pub fn load_stat_table(config: &Config) -> (Option<StatTable>, bool) {
    let path = config.asset("stat_table.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (None, false),
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    };
    match toml::from_str::<StatTable>(&text) {
        Ok(t) => {
            assert!(
                t.rows.len() == STAT_ROWS,
                "{}: {} rows, expected {STAT_ROWS}: run `cargo run --release -p citysim-cli -- calibrate`",
                path.display(),
                t.rows.len()
            );
            (Some(t), false)
        }
        Err(_) if text.contains("[morning]") => (None, true),
        Err(e) => panic!("bad {}: {e}", path.display()),
    }
}

/// `assets/stat_mlp.toml` under `[lod] policy = "mlp"`, else `None`.
fn load_stat_mlp(config: &Config) -> Option<crate::systems::stat_policy::StatMlp> {
    match config.lod.policy.as_str() {
        "table" => None,
        "mlp" => Some(crate::systems::stat_policy::StatMlp::load(config)),
        other => panic!("[lod] policy = {other:?}: expected \"table\" or \"mlp\""),
    }
}

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
    // agent components (index = EntityId.index)
    pub position: Vec<Option<Position>>,
    pub identity: Vec<Option<Identity>>,
    pub needs: Vec<Option<Needs>>,
    pub personality: Vec<Option<Personality>>,
    pub mood: Vec<Option<Mood>>,
    pub wallet: Vec<Option<Wallet>>,
    pub inventory: Vec<Option<Inventory>>,
    pub job: Vec<Option<Job>>,
    pub household: Vec<Option<Household>>,
    pub brain: Vec<Option<Brain>>,
    pub memory: Vec<Option<Memory>>,
    pub skills: Vec<Option<Skills>>,
    pub sentence: Vec<Option<Sentence>>,
    pub gang_member: Vec<Option<GangMember>>,
    pub corpse: Vec<Option<Corpse>>,
    pub child: Vec<Option<Child>>,
    /// M12 D27.
    #[serde(default)]
    pub squatter: Vec<Option<Squatter>>,
    // non-agent components
    pub building: Vec<Option<Building>>,
    pub gang: Vec<Option<Gang>>,
    pub market: Vec<Option<Market>>,
    pub treasury: Vec<Option<Treasury>>,
    /// M9: on the Jail.
    #[serde(default)]
    pub law: Vec<Option<Law>>,
    /// M10: per adult, kept after death.
    #[serde(default)]
    pub trace: Vec<Option<Trace>>,
    /// M10: per agent, kept after death.
    #[serde(default)]
    pub life: Vec<Option<Life>>,
    /// M11: corps, each its own entity.
    #[serde(default)]
    pub corp: Vec<Option<Corp>>,
    /// M13 D1: assets, each its own entity carrying only this.
    #[serde(default)]
    pub asset: Vec<Option<Asset>>,
    /// M13 D4: every agent's Body.
    #[serde(default)]
    pub body: Vec<Option<Body>>,
    /// M13 D5: stored daily for M15.
    #[serde(default)]
    pub appearance: Vec<Option<Appearance>>,
    /// M13 D3: derived from the assets, never saved; rebuilt on load.
    #[serde(skip)]
    pub kit: Vec<Option<Kit>>,
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
    pub buildings_by_kind: BTreeMap<BuildingKind, Vec<EntityId>>,
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
    pub reservations: BTreeMap<EntityId, Vec<Reservation>>,
    pub door_queue: BTreeMap<TilePos, u8>,
    /// Lazily built, LRU-evicted (`[exec] flow_field_cache`); pure functions of map and door.
    #[serde(skip)]
    pub flow_fields: FlowCache,
    pub last_seen: BTreeMap<EntityId, (TilePos, Tick)>,
    pub vacancies: BTreeMap<EntityId, Vec<Role>>,
    pub edge_roads: Vec<TilePos>,
    pub view_rect: Option<Rect>,
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
    pub district_adjacent: Vec<u16>,
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
    /// M15 W13: every living adult's and faction's reputation, rebuilt at
    /// midnight by `reputation::rebuild`; saved, so a load mid-day reads
    /// what the last midnight wrote.
    #[serde(default)]
    pub reputation: Vec<Option<crate::word::Reputation>>,
    /// M15 W15: grudges (phase 3); empty stores are not written.
    #[serde(default, skip_serializing_if = "all_none")]
    pub grudges: Vec<Option<crate::word::Grudges>>,
    /// M16a (plan C2): a Fixer office's record book; empty stores are not
    /// written (`resize_stores` sizes it on load).
    #[serde(default, skip_serializing_if = "all_none")]
    pub broker: Vec<Option<crate::contract::Broker>>,
    /// Real economy E41: the `CampRaised` trait of adults raised at a work
    /// camp; empty stores are not written.
    #[serde(default, skip_serializing_if = "all_none")]
    pub camp_raised: Vec<Option<crate::econ::CampRaised>>,
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
    pub save_version: u8,
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
    /// C31 (phase 3): a city guard on the take -> its buyers.
    #[serde(skip)]
    pub on_take: BTreeMap<EntityId, SmallVec<[EntityId; 2]>>,
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

/// Wire every component store: the `Component` impl, plus `grow`/`clear`
/// so spawn/despawn can never miss a store.
macro_rules! components {
    ($($field:ident : $ty:ty),* $(,)?) => {
        $(
            impl Component for $ty {
                fn store(world: &World) -> &Vec<Option<Self>> { &world.$field }
                fn store_mut(world: &mut World) -> &mut Vec<Option<Self>> { &mut world.$field }
            }
        )*
        impl World {
            pub(crate) fn grow_components(&mut self) {
                $( self.$field.push(None); )*
            }
            pub(crate) fn clear_components(&mut self, i: usize) {
                $( self.$field[i] = None; )*
            }
        }
    };
}

components! {
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
    squatter: Squatter,
    building: Building,
    gang: Gang,
    market: Market,
    treasury: Treasury,
    law: Law,
    trace: Trace,
    life: Life,
    corp: Corp,
    asset: Asset,
    body: Body,
    appearance: Appearance,
    kit: Kit,
    reputation: crate::word::Reputation,
    grudges: crate::word::Grudges,
    broker: crate::contract::Broker,
    camp_raised: crate::econ::CampRaised,
}

/// Per suspect `(open reports, latest report tick)`, and the suspects with an
/// open report, ascending.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportIndex {
    pub by_suspect: BTreeMap<EntityId, (u32, Tick)>,
    pub open: Vec<EntityId>,
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

/// First/last name tables from `assets/names.txt`.
#[derive(Clone, Debug, Default)]
pub struct NameTables {
    first_male: Vec<String>,
    first_female: Vec<String>,
    last: Vec<String>,
}

impl NameTables {
    pub fn load(config: &Config) -> NameTables {
        let text =
            std::fs::read_to_string(config.asset("names.txt")).unwrap_or_else(|e| panic!("cannot read names.txt: {e}"));
        NameTables::parse(&text)
    }

    pub fn parse(text: &str) -> NameTables {
        let mut tables = NameTables { first_male: Vec::new(), first_female: Vec::new(), last: Vec::new() };
        let mut section = "";
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = match name {
                    "first_male" => "m",
                    "first_female" => "f",
                    "last" => "l",
                    other => panic!("names.txt: unknown section [{other}]"),
                };
                continue;
            }
            match section {
                "m" => tables.first_male.push(line.to_string()),
                "f" => tables.first_female.push(line.to_string()),
                "l" => tables.last.push(line.to_string()),
                _ => panic!("names.txt: name {line:?} before any section header"),
            }
        }
        assert!(
            !tables.first_male.is_empty() && !tables.first_female.is_empty() && !tables.last.is_empty(),
            "names.txt: every section needs at least one name"
        );
        tables
    }
}

impl World {
    /// Build the initial city: map, buildings, gang, 300 citizens with homes
    /// and jobs. Deterministic for a given `(seed, config)`.
    pub fn new(seed: u64, config: Config) -> World {
        // Real economy E22 (a), E36 (phase 3a): the dole and school meals
        // resolved off with `no_net` (a no-op otherwise).
        let config = crate::systems::econ::apply(config);
        let map = Map::load(&config.asset(&config.world.map));
        let names = NameTables::load(&config);
        // Absent until `citysim-cli calibrate` has written it; any other read
        // failure is a broken checkout and must not pass silently.
        let (stat_table, stat_table_legacy) = load_stat_table(&config);
        let stat_mlp = load_stat_mlp(&config);
        // J11: `by_role` holds the 19 bespoke roles and the configured trades.
        let config_trades = config.trades.len();

        let edge_roads = map.edge_roads();
        let levers = Levers::from_config(&config);
        let mut w = World {
            tick: 0,
            rng: SimRng::new(seed),
            config,
            map,
            generations: Vec::new(),
            free_list: BTreeSet::new(),
            alive: Vec::new(),
            position: Vec::new(),
            identity: Vec::new(),
            needs: Vec::new(),
            personality: Vec::new(),
            mood: Vec::new(),
            wallet: Vec::new(),
            inventory: Vec::new(),
            job: Vec::new(),
            household: Vec::new(),
            brain: Vec::new(),
            memory: Vec::new(),
            skills: Vec::new(),
            sentence: Vec::new(),
            gang_member: Vec::new(),
            corpse: Vec::new(),
            child: Vec::new(),
            squatter: Vec::new(),
            building: Vec::new(),
            gang: Vec::new(),
            market: Vec::new(),
            treasury: Vec::new(),
            law: Vec::new(),
            trace: Vec::new(),
            life: Vec::new(),
            corp: Vec::new(),
            asset: Vec::new(),
            body: Vec::new(),
            appearance: Vec::new(),
            kit: Vec::new(),
            assets_at: BTreeMap::new(),
            assets_by_owner: BTreeMap::new(),
            vehicles: Vec::new(),
            limbo: BTreeMap::new(),
            loot_corpses: Vec::new(),
            trips: BTreeMap::new(),
            chase_pins: BTreeSet::new(),
            episodes: BTreeSet::new(),
            commute_start: BTreeMap::new(),
            commute_acc: [0; 4],
            dealers: BTreeMap::new(),
            deal_log: BTreeMap::new(),
            robot_sellers: BTreeMap::new(),
            edges: crate::edge_map::EdgeMap::new(),
            crime_reports: Vec::new(),
            report_index: std::sync::OnceLock::new(),
            gang_ids: Vec::new(),
            sentenced_ids: Vec::new(),
            corp_ids: Vec::new(),
            residents: BTreeMap::new(),
            resident_home: BTreeMap::new(),
            mean_price_cache: None,
            shop_offers: BTreeMap::new(),
            corp_rethink: false,
            buildings_by_kind: BTreeMap::new(),
            agents_by_tile: BTreeMap::new(),
            levers,
            stats: DailyStats::new(),
            events: VecDeque::new(),
            debug_events: VecDeque::new(),
            next_event_id: 0,
            plan_queue: BTreeMap::new(),
            reservations: BTreeMap::new(),
            door_queue: BTreeMap::new(),
            flow_fields: FlowCache::default(),
            last_seen: BTreeMap::new(),
            vacancies: BTreeMap::new(),
            edge_roads,
            view_rect: None,
            command_queue: Vec::new(),
            command_log: Vec::new(),
            stat_table,
            stat_table_legacy,
            stat_mlp,
            holes: BTreeMap::new(),
            holes_by_agent: BTreeMap::new(),
            bind_queue: Vec::new(),
            zone_watch: ZoneWatch::default(),
            day_marks: BTreeMap::new(),
            pending_purchase: BTreeMap::new(),
            tax_accum: BTreeMap::new(),
            eviction_log: VecDeque::new(),
            classes: Default::default(),
            last_strike: None,
            home_watch: Default::default(),
            districts: Vec::new(),
            district_grid: Vec::new(),
            district_adjacent: Vec::new(),
            district_watch: Default::default(),
            report_places: VecDeque::new(),
            eviction_places: VecDeque::new(),
            vagrancy_places: BTreeMap::new(),
            litter: Vec::new(),
            hotel_beds: BTreeMap::new(),
            sweep_beats: BTreeMap::new(),
            squat_index: BTreeMap::new(),
            riots: Vec::new(),
            next_riot_id: 0,
            rioter_of: BTreeMap::new(),
            neighbours: BTreeMap::new(),
            spouses: BTreeMap::new(),
            enemies: BTreeMap::new(),
            by_tier: Default::default(),
            by_role: vec![Vec::new(); Role::ALL.len() + config_trades],
            stat_slots: StatSlots::default(),
            names: names.clone(),
            virt: Default::default(),
            virt_dirty: false,
            db: BTreeMap::new(),
            runs: BTreeMap::new(),
            run_queue: BTreeSet::new(),
            run_orders: BTreeMap::new(),
            run_log: VecDeque::new(),
            last_trace: BTreeMap::new(),
            runner_of: BTreeMap::new(),
            reputation: Vec::new(),
            grudges: Vec::new(),
            broker: Vec::new(),
            camp_raised: Vec::new(),
            rumours: Vec::new(),
            regard: BTreeMap::new(),
            kill_watch: VecDeque::new(),
            kill_known: Vec::new(),
            arrest_log: VecDeque::new(),
            anon_heard: BTreeSet::new(),
            skill_means: [0.0; 8],
            shadow_notes: None,
            skill_quantiles: Vec::new(),
            talent_gone: BTreeMap::new(),
            extort_log: VecDeque::new(),
            hunts: BTreeMap::new(),
            hunted_by: BTreeMap::new(),
            kill_chain: BTreeMap::new(),
            vendettas: Vec::new(),
            guards_of_corpse: BTreeMap::new(),
            stories: VecDeque::new(),
            next_story_id: 0,
            order_rates: Default::default(),
            fv_active: Vec::new(),
            fv_tally: Default::default(),
            held_since: BTreeMap::new(),
            feeds_seeded: false,
            save_version: crate::save::SAVE_VERSION,
            budget: Default::default(),
            outside: Default::default(),
            scrap: 0,
            jobs_book: Default::default(),
            works_vacancies: 0,
            venues_seeded: false,
            venues_due: false,
            unwind: BTreeMap::new(),
            hangouts: BTreeMap::new(),
            spots: Vec::new(),
            last_met: BTreeMap::new(),
            told: BTreeMap::new(),
            wealth_p90: 0,
            contracts: BTreeMap::new(),
            missions: BTreeMap::new(),
            contract_runs: BTreeMap::new(),
            contract_log: VecDeque::new(),
            next_contract: 0,
            fixers_seeded: false,
            fixers_due: false,
            fixer_runs: BTreeMap::new(),
            escrow_held: 0,
            median_wage: 0,
            chased_by: BTreeMap::new(),
            by_party: BTreeMap::new(),
            ledger_due: BTreeSet::new(),
            mission_of: BTreeMap::new(),
            locate_targets: BTreeMap::new(),
            on_take: BTreeMap::new(),
            contract_guards: BTreeMap::new(),
            econ: Default::default(),
            probe: Default::default(),
            employers: BTreeMap::new(),
            employer_of: BTreeMap::new(),
            laid_off: BTreeMap::new(),
        };
        w.spawn_buildings();
        w.litter = vec![0; w.map.w() * w.map.h()];
        systems::districts::rebuild(&mut w);
        w.rumours = vec![Default::default(); w.districts.len()];
        w.spawn_gangs();
        w.spawn_population(&names);
        // M15 W25/W28: the social skills from each agent's Skill word stream
        // (the world stream is untouched), then the city means competence reads.
        systems::moves::seed_population(&mut w);
        // M12 D26: the derelicts are cut after the population is dealt (the
        // world stream is untouched) and before ownership skips them; the
        // Hotels go up on Lots once the Bar owners are dealt.
        systems::street::seed_derelicts(&mut w);
        systems::ownership::seed(&mut w);
        // Real economy (plan E4, Seeding): the World account and its books
        // (no RNG), before any purchase can cross.
        systems::world_market::seed(&mut w);
        systems::street::seed_hotels(&mut w);
        // M13 D18: the Garages go up last, on the Lots the Hotels left.
        systems::assets::seed_sellers(&mut w);
        // M14 (Seeding table, V21, V26, V47): the corps' trees by name (no
        // RNG), then, with the plane on, the Labs on the Lots left, the
        // plane and its opening ICE.
        systems::tech::seed_corps(&mut w, false);
        systems::virt::seed_labs(&mut w);
        // M15 W41: the Civic Wire and Nutrix Now on the Lots left (no RNG),
        // before the plane links, so each has a node from the start.
        systems::news::seed_feeds(&mut w);
        // M15 W31: The Unplugged on a Sump Central derelict (no RNG), before
        // the plane links, so its Chapel has a node from the start.
        systems::creeds::seed_unplugged(&mut w);
        // L2 (plan L7): the venues and Fabs on the Lots left (no RNG), after
        // the Chapel and before the plane links, so each has a node.
        systems::jobs::seed_venues(&mut w);
        // M16a (plan C9): the seeded Fixer on the Lots left (no RNG), after
        // the venues and before the plane links, so each has a node.
        systems::contracts::seed_fixers(&mut w);
        // Real economy E26, E37 (Seeding table): the Sump Mission and the
        // Chapel's kitchen, then the city's Camp (no RNG), before the plane links.
        systems::charity::seed(&mut w);
        systems::camp::seed(&mut w);
        // L2 phase 5 (the day-1 leisure pulse): opening fun spread (no RNG).
        systems::leisure::seed_fun(&mut w);
        systems::virt::relink(&mut w);
        systems::virt::seed_ice(&mut w);
        // M15 W28: every corp's and the Law's opening competence.
        systems::competence::seed(&mut w);
        // Real economy phase 2 (plan E14, Seeding): the hoard into the
        // corps' working capital, last (no RNG; a plain purse move).
        systems::econ::seed_capital(&mut w);
        w
    }

    pub fn seed(&self) -> u64 {
        self.rng.seed()
    }

    // -----------------------------------------------------------------------
    // Initial world
    // -----------------------------------------------------------------------

    fn spawn_buildings(&mut self) {
        let defs = self.map.buildings.clone();
        for def in defs {
            let cfg = self.config.buildings.for_kind(def.kind);
            let stock_food = match def.kind {
                BuildingKind::Home => self.config.world.home_pantry_initial,
                BuildingKind::Market => self.config.world.market_initial,
                BuildingKind::Warehouse => self.config.world.warehouse_initial,
                _ => 0,
            };
            // Jobs and room J7: a map building's seats × its floors.
            let capacity = crate::systems::founding::with_floors(def.kind, cfg.capacity, def.floors);
            let id = self.spawn();
            self.insert(
                id,
                Building {
                    kind: def.kind,
                    production_accum: 0.0,
                    claim: None,
                    child_food_debt: 0.0,
                    rect: def.rect,
                    door: def.door,
                    stock_food,
                    capacity,
                    owner: None,
                    occupants: Vec::new(),
                    demolished: false,
                    tier: def.tier,
                    rent_per_day: 0,
                    revenue_today: 0,
                    revenue: VecDeque::new(),
                    secured_by: None,
                    derelict: false,
                    empty_since: None,
                    closed_until: None,
                    full_capacity: None,
                    stock_goods: [0; 2],
                    asset_sales_today: 0,
                    asset_sales: VecDeque::new(),
                    security: Default::default(),
                    focus: None,
                    hacked: None,
                    last_door_open: None,
                    label: None,
                    feed: None,
                    venue: None,
                    charity: None,
                    camp: None,
                    input_accum: 0.0,
                    floors: def.floors,
                    floor_days: 0,
                },
            );
            match def.kind {
                BuildingKind::Market => {
                    let price = self.config.world.price_initial;
                    self.insert(id, Market::new(price));
                }
                BuildingKind::Hall => {
                    let coins = self.config.world.treasury_initial;
                    self.insert(id, Treasury { coins });
                }
                BuildingKind::Jail => self.insert(id, Law::default()),
                _ => {}
            }
            self.buildings_by_kind.entry(def.kind).or_default().push(id);
        }
    }

    /// One gang per Hideout, in map order, named and funded from `[gangs]`.
    fn spawn_gangs(&mut self) {
        let hideouts = self.buildings_by_kind.get(&BuildingKind::Hideout).cloned().unwrap_or_default();
        let cfg = self.config.gangs.clone();
        for (i, hideout) in hideouts.into_iter().enumerate() {
            let id = self.spawn();
            let name = cfg.names.get(i).cloned().unwrap_or_else(|| format!("Gang {}", i + 1));
            let treasury = cfg.treasury_initial.get(i).or(cfg.treasury_initial.last()).copied().unwrap_or(0);
            self.insert(id, Gang::new(name, hideout, treasury));
        }
    }

    fn spawn_population(&mut self, names: &NameTables) {
        let wc = self.config.world.clone();
        let n = wc.population as usize;

        // --- individuals -------------------------------------------------
        let mut ids = Vec::with_capacity(n);
        for i in 0..n {
            let id = self.spawn();
            let sex = if i % 2 == 0 { Sex::Male } else { Sex::Female };
            let rng = self.rng.world();
            let first = match sex {
                Sex::Male => &names.first_male,
                Sex::Female => &names.first_female,
            };
            let first = first[rng.random_range(0..first.len())].clone();
            let last = names.last[rng.random_range(0..names.last.len())].clone();
            let years: f32 = rng.random_range(wc.age_min_years..wc.age_max_years);
            let age_days = (years * DAYS_PER_YEAR as f32) as u32;
            let born_tick = -(i64::from(age_days) * TICKS_PER_DAY as i64);
            let coins = rng.random_range(wc.initial_coins_min..=wc.initial_coins_max);
            let food = rng.random_range(wc.initial_food_min..=wc.initial_food_max);
            let personality = Personality::random(rng);
            let stealth = rng.random_range(wc.skill_min..wc.skill_max);
            let fighting = rng.random_range(wc.skill_min..wc.skill_max);
            let farming = rng.random_range(wc.skill_min..wc.skill_max);
            let skills = Skills::basic(stealth, fighting, farming, Skills::unset_hacking());

            self.insert(
                id,
                Identity { name: format!("{first} {last}"), age_days, sex, born_tick, spouse_died_tick: None },
            );
            self.insert(
                id,
                Needs {
                    hunger: wc.needs_initial.hunger,
                    energy: wc.needs_initial.energy,
                    safety: wc.needs_initial.safety,
                    wealth: 0.0,
                    belonging: wc.needs_initial.belonging,
                    intimacy: wc.needs_initial.intimacy,
                    starving_since: None,
                    fun: 1.0,
                },
            );
            self.insert(id, personality);
            self.insert(id, Mood::default());
            self.insert(id, Wallet { coins });
            self.insert(id, Inventory { food, stolen_food: 0, stims: 0, parts: 0 });
            self.insert(id, Brain::default());
            self.insert(id, Memory::default());
            self.insert(id, skills);
            // M14 V35: a keyed draw (the world stream is untouched).
            let hacking = systems::tech::draw_hacking(self, id);
            systems::tech::give_hacking(self, id, hacking);
            self.insert(id, Household::new(None));
            // M13 D4: keyed draws, so the world stream is untouched.
            let body = systems::assets::new_body(self, id);
            systems::assets::give_body(self, id, body);
            ids.push(id);
        }
        self.recompute_wealth();

        // --- homes: seeded shuffle, residents_per_home each ----------------
        let mut homes = self.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default();
        // Under capacity, spread the households over the whole map by even
        // stride (M10 D24), so a 500-agent world still spans every zone.
        let per = (wc.residents_per_home as usize).max(1);
        let needed = n.div_ceil(per);
        if needed < homes.len() {
            homes = (0..needed).map(|k| homes[k * homes.len() / needed]).collect();
        }
        let mut shuffled = ids.clone();
        shuffled.shuffle(self.rng.world());
        for (chunk, &home) in shuffled.chunks(wc.residents_per_home as usize).zip(homes.iter()) {
            let door = self.comp::<Building>(home).map(|b| b.door).unwrap_or_default();
            for &id in chunk {
                self.insert(id, Household::new(Some(home)));
                self.insert(id, Position { tile: door, building: None, entered: 0 });
                self.enter_building(id, home); // an interior slot each
            }
            if chunk.len() >= 2 && self.rng.world().random_bool(wc.spouse_p) {
                self.set_spouse(chunk[0], chunk[1]);
                let e = self.edge_entry(chunk[0], chunk[1]);
                e.affinity = 0.6;
                e.trust = 0.6;
            }
        }
        // Under capacity, an empty Home starts with an empty pantry: in the
        // 500-resident calibration city 300 unwatched pantries of opening food
        // were free loot, and the Full thieves the table learns from cleared
        // them (M10 phase 5b).
        let used: BTreeSet<EntityId> = homes.iter().copied().take(n.div_ceil(per)).collect();
        for h in self.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default() {
            if !used.contains(&h) {
                if let Some(b) = self.comp_mut::<Building>(h) {
                    b.stock_food = 0;
                }
            }
        }
        // M11: inequality from day one. The coin draw above is scaled by the
        // tier of the Home the agent was dealt (no new draws: the world
        // stream is untouched). The homeless keep the draw.
        let mult = wc.coins_by_tier;
        if mult != [1.0, 1.0, 1.0] {
            for &id in &ids {
                let Some(tier) = self
                    .comp::<Household>(id)
                    .and_then(|h| h.home)
                    .and_then(|h| self.comp::<Building>(h))
                    .map(|b| usize::from(b.tier.min(2)))
                else {
                    continue;
                };
                if let Some(w) = self.comp_mut::<Wallet>(id) {
                    w.coins = (w.coins as f32 * mult[tier]).round() as i64;
                }
            }
            self.recompute_wealth();
        }
        // Anyone left over (population > homes × residents) is homeless on the
        // road outside the Market door: on the street, so `building` is None.
        let market_door = self
            .building_of_kind(BuildingKind::Market)
            .and_then(|m| self.comp::<Building>(m).map(|b| (b.rect, b.door)))
            .unwrap_or_default();
        let street = self
            .map
            .neighbours4(market_door.1)
            .find(|&n| !market_door.0.contains(n) && self.map.tile_at(n) == TileKind::Road)
            .unwrap_or(market_door.1);
        for &id in &ids {
            if !self.has::<Position>(id) {
                self.insert(id, Position { tile: street, building: None, entered: 0 });
            }
        }

        // --- jobs: seeded shuffle, fixed role order ------------------------
        // M10: each slot goes to the unassigned resident living nearest the
        // workplace (home door to workplace door; ties in shuffle order), as
        // `job_search` already hires. On the 256 x 192 map a shuffled
        // assignment put commutes at 100-300 tiles, most of a shift.
        let mut pool = ids.clone();
        pool.shuffle(self.rng.world());
        let home_door: BTreeMap<EntityId, TilePos> = pool
            .iter()
            .filter_map(|&id| {
                let home = self.comp::<Household>(id).and_then(|h| h.home)?;
                Some((id, self.comp::<Building>(home)?.door))
            })
            .collect();
        let mut taken = vec![false; pool.len()];
        for role in Role::ALL {
            let count = wc.jobs.count(role) as usize;
            let workplaces = role.workplace().and_then(|k| self.buildings_by_kind.get(&k)).cloned().unwrap_or_default();
            for k in 0..count {
                let employer = workplaces.get(k % workplaces.len().max(1)).copied();
                let work_door = employer.and_then(|e| self.comp::<Building>(e)).map(|b| b.door);
                let pick = pool
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| !taken[i])
                    .map(|(i, id)| {
                        let d = match (home_door.get(id), work_door) {
                            (Some(h), Some(w)) => h.manhattan(w),
                            _ => u32::MAX,
                        };
                        (d, i)
                    })
                    .min();
                let Some((_, i)) = pick else { break };
                taken[i] = true;
                let id = pool[i];
                let shifts = if role == Role::Guard && id.index % 2 == 0 {
                    wc.shift_night.clone()
                } else {
                    wc.shift_day.clone()
                };
                let wage_per_day = self.config.wage(role);
                self.insert(
                    id,
                    Job {
                        employer,
                        role,
                        wage_per_day,
                        shifts,
                        days_unpaid: 0,
                        tax_accum: 0.0,
                        last_shift_day: None,
                        last_wage_attempt_day: None,
                        duty_ticks: 0,
                        hired_tick: 0,
                        struck_shift: None,
                        premium: 1.0,
                        paid_once: true,
                        duty_fixed: None,
                    },
                );
            }
        }
    }

    /// `wealth = clamp(coins / (7 × price × (1 + greed)), 0, 1)` for every citizen.
    pub fn recompute_wealth(&mut self) {
        for id in self.citizens() {
            // scan-ok: hourly
            self.recompute_wealth_for(id);
        }
    }

    /// `recompute_wealth` for one agent (the spread Statistical tick calls it
    /// per processed agent).
    pub fn recompute_wealth_for(&mut self, id: EntityId) {
        let tick = self.tick;
        let price = match self.mean_price_cache {
            Some((t, p)) if t == tick => p,
            _ => {
                let p = self.mean_price();
                self.mean_price_cache = Some((tick, p));
                p
            }
        };
        let price = price.max(1) as f32;
        let days = self.config.needs.wealth_days_secure;
        let greed = self.comp::<Personality>(id).map_or(0.5, |p| p.greed);
        let coins = self.comp::<Wallet>(id).map_or(0, |w| w.coins) as f32;
        if let Some(n) = self.comp_mut::<Needs>(id) {
            n.wealth = (coins / (days * price * (1.0 + greed))).clamp(0.0, 1.0);
        }
    }

    // -----------------------------------------------------------------------
    // Tier and role indices (kept incrementally; no per-tick scans)
    // -----------------------------------------------------------------------

    pub(crate) fn after_insert<T: Component>(&mut self, id: EntityId) {
        use std::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<Brain>() {
            self.index_brain(id);
        } else if TypeId::of::<T>() == TypeId::of::<Job>() {
            self.index_job(id);
        } else if TypeId::of::<T>() == TypeId::of::<Gang>() {
            Self::list_insert(&mut self.gang_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Sentence>() {
            Self::list_insert(&mut self.sentenced_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Corp>() {
            Self::list_insert(&mut self.corp_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Household>() {
            self.index_household(id);
        } else if TypeId::of::<T>() == TypeId::of::<Squatter>() {
            self.index_squatter(id);
        }
    }

    pub(crate) fn after_remove<T: Component>(&mut self, id: EntityId) {
        use std::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<Brain>() {
            self.unindex_brain(id);
        } else if TypeId::of::<T>() == TypeId::of::<Job>() {
            self.unindex_job(id);
        } else if TypeId::of::<T>() == TypeId::of::<Gang>() {
            self.unindex_gang(id);
        } else if TypeId::of::<T>() == TypeId::of::<Sentence>() {
            self.unindex_sentenced(id);
        } else if TypeId::of::<T>() == TypeId::of::<Corp>() {
            self.unindex_corp(id);
        } else if TypeId::of::<T>() == TypeId::of::<Household>() {
            self.unindex_household(id);
        } else if TypeId::of::<T>() == TypeId::of::<Squatter>() {
            self.unindex_squatter(id);
        }
    }

    /// File a squatter under its building (D27).
    fn index_squatter(&mut self, id: EntityId) {
        self.unindex_squatter(id);
        if let Some(b) = self.comp::<Squatter>(id).map(|s| s.building) {
            Self::list_insert(self.squat_index.entry(b).or_default(), id);
        }
    }

    /// Drop an agent from every squat list (its Squatter removed, or despawned).
    pub(crate) fn unindex_squatter(&mut self, id: EntityId) {
        self.squat_index.retain(|_, list| {
            Self::list_remove(list, id);
            !list.is_empty()
        });
    }

    /// The squatters of a building, ascending.
    pub fn squatters_of(&self, b: EntityId) -> &[EntityId] {
        self.squat_index.get(&b).map_or(&[], Vec::as_slice)
    }

    /// File the agent under its Household's Home.
    fn index_household(&mut self, id: EntityId) {
        self.unindex_household(id);
        if let Some(home) = self.comp::<Household>(id).and_then(|h| h.home) {
            Self::list_insert(self.residents.entry(home).or_default(), id);
            self.resident_home.insert(id, home);
        }
    }

    pub(crate) fn unindex_household(&mut self, id: EntityId) {
        if let Some(home) = self.resident_home.remove(&id) {
            if let Some(list) = self.residents.get_mut(&home) {
                Self::list_remove(list, id);
                if list.is_empty() {
                    self.residents.remove(&home);
                }
            }
        }
    }

    /// Move an agent into a Home (or out of any): the one write to
    /// `Household::home`, so `residents_of` stays in step.
    pub fn set_home(&mut self, id: EntityId, home: Option<EntityId>) {
        if let Some(h) = self.comp_mut::<Household>(id) {
            // M11 phase 5: losing a Home (eviction, demolition) ends its rent:
            // arrears kept after a DemolishHome barred re-housing for good.
            if home.is_none() {
                h.arrears = 0;
                h.rent_due = 0.0;
            } else {
                h.homeless_since = None;
            }
            h.home = home;
            self.index_household(id);
        }
        // Fix pass (phase 3 review): any path to a Home (re-housing, a
        // marriage, a god command) ends a squat and tonight's Hotel booking;
        // a housed agent kept the slot and the Sweep evicted them.
        if home.is_some() {
            self.remove::<crate::components::Squatter>(id);
            self.hotel_beds.remove(&id);
        }
    }

    /// Everyone whose Household names this Home, ascending (the living:
    /// a death removes the Household).
    pub fn residents_of(&self, home: EntityId) -> &[EntityId] {
        self.residents.get(&home).map_or(&[], Vec::as_slice)
    }

    /// Drop an agent from `sentenced_ids` (its Sentence removed, or despawned).
    pub(crate) fn unindex_sentenced(&mut self, id: EntityId) {
        Self::list_remove(&mut self.sentenced_ids, id);
    }

    /// Every agent with a `Sentence`, ascending by id (`with::<Sentence>()`
    /// without the entity scan).
    pub fn sentenced(&self) -> &[EntityId] {
        &self.sentenced_ids
    }

    /// Drop a gang from `gang_ids` (its Gang removed, or despawned).
    pub(crate) fn unindex_gang(&mut self, id: EntityId) {
        if self.gang_ids.binary_search(&id).is_ok() {
            // M14 V4: a gang gone (its Hideout node) relinks the plane.
            self.virt_dirty = true;
        }
        Self::list_remove(&mut self.gang_ids, id);
    }

    /// Drop a corp from `corp_ids` (its Corp removed, or despawned).
    pub(crate) fn unindex_corp(&mut self, id: EntityId) {
        if self.corp_ids.binary_search(&id).is_ok() {
            // M14 V4: a corp gone (its Ledger node) relinks the plane.
            self.virt_dirty = true;
        }
        Self::list_remove(&mut self.corp_ids, id);
    }

    fn list_remove(list: &mut Vec<EntityId>, id: EntityId) {
        if let Ok(i) = list.binary_search(&id) {
            list.remove(i);
        }
    }

    fn list_insert(list: &mut Vec<EntityId>, id: EntityId) {
        if let Err(i) = list.binary_search(&id) {
            list.insert(i, id);
        }
    }

    /// File the agent under its Brain's current LOD.
    pub fn index_brain(&mut self, id: EntityId) {
        self.unindex_brain(id);
        if let Some(b) = self.comp::<Brain>(id) {
            let lod = b.lod;
            Self::list_insert(&mut self.by_tier[lod as usize], id);
            if lod == Lod::Statistical {
                Self::list_insert(&mut self.stat_slots.0[StatSlots::slot_of(id)], id);
            }
        }
    }

    pub fn unindex_brain(&mut self, id: EntityId) {
        for list in &mut self.by_tier {
            Self::list_remove(list, id);
        }
        Self::list_remove(&mut self.stat_slots.0[StatSlots::slot_of(id)], id);
    }

    /// Call after any write to `Brain::lod`.
    pub fn retier(&mut self, id: EntityId) {
        self.index_brain(id);
    }

    /// File the agent under its Job's role (and, J3, its workplace).
    pub fn index_job(&mut self, id: EntityId) {
        self.unindex_job(id);
        if let Some(j) = self.comp::<Job>(id) {
            let (r, employer) = (role_index(j.role), j.employer);
            if r >= self.by_role.len() {
                self.by_role.resize(r + 1, Vec::new());
            }
            Self::list_insert(&mut self.by_role[r], id);
            if let Some(b) = employer {
                Self::list_insert(self.employers.entry(b).or_default(), id);
                self.employer_of.insert(id, b);
            }
        }
    }

    pub fn unindex_job(&mut self, id: EntityId) {
        for list in &mut self.by_role {
            Self::list_remove(list, id);
        }
        if let Some(b) = self.employer_of.remove(&id) {
            if let Some(list) = self.employers.get_mut(&b) {
                Self::list_remove(list, id);
                if list.is_empty() {
                    self.employers.remove(&b);
                }
            }
        }
    }

    /// J3: everyone employed at `building`, ascending (the employer index).
    pub fn staff_of(&self, building: EntityId) -> &[EntityId] {
        self.employers.get(&building).map_or(&[], Vec::as_slice)
    }

    /// Agents with a Brain at this LOD, ascending.
    pub fn tier(&self, lod: Lod) -> &[EntityId] {
        &self.by_tier[lod as usize]
    }

    /// Full and Coarse agents (the ones with bodies), ascending.
    pub fn bodies(&self) -> Vec<EntityId> {
        let (a, b) = (&self.by_tier[Lod::Full as usize], &self.by_tier[Lod::Coarse as usize]);
        let mut out = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            if a[i] < b[j] {
                out.push(a[i]);
                i += 1;
            } else {
                out.push(b[j]);
                j += 1;
            }
        }
        out.extend_from_slice(&a[i..]);
        out.extend_from_slice(&b[j..]);
        out
    }

    /// Job holders of one role, ascending.
    pub fn workers(&self, role: Role) -> &[EntityId] {
        self.by_role.get(role_index(role)).map_or(&[], Vec::as_slice)
    }

    /// J11: every role a Job can hold: the 19 bespoke roles (`Role::ALL`)
    /// and the configured trades, in [`World::workers`]' index order.
    pub fn roles(&self) -> Vec<Role> {
        let mut out = Role::ALL.to_vec();
        out.extend(self.config.trade_roles());
        out
    }

    /// Every guard (a Job holder with `Role::Guard`), ascending.
    pub fn guards(&self) -> &[EntityId] {
        self.workers(Role::Guard)
    }

    /// Rebuild both indices from the stores (after a load).
    pub fn rebuild_tiers_and_roles(&mut self) {
        let (tiers, roles, slots) = self.indices_from_stores();
        self.by_tier = tiers;
        self.by_role = roles;
        self.stat_slots = slots;
        (self.employers, self.employer_of) = self.employers_from_stores();
        self.gang_ids = self.with::<Gang>();
        self.sentenced_ids = self.with::<Sentence>();
        self.corp_ids = self.with::<Corp>();
        (self.residents, self.resident_home) = self.residents_from_stores();
        self.squat_index = self.squat_index_from_stores();
        self.rioter_of =
            self.riots.iter().flat_map(|r| r.rioters.iter().map(move |&a| (a, r.id))).collect::<BTreeMap<_, _>>();
        self.mean_price_cache = None;
    }

    fn squat_index_from_stores(&self) -> BTreeMap<EntityId, Vec<EntityId>> {
        let mut out: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        for id in self.entities() {
            if let Some(s) = self.comp::<Squatter>(id) {
                out.entry(s.building).or_default().push(id);
            }
        }
        out
    }

    /// J3: the employer index and its reverse from the Job store.
    #[allow(clippy::type_complexity)]
    fn employers_from_stores(&self) -> (BTreeMap<EntityId, Vec<EntityId>>, BTreeMap<EntityId, EntityId>) {
        let mut by_b: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        let mut back = BTreeMap::new();
        for id in self.entities() {
            if let Some(b) = self.comp::<Job>(id).and_then(|j| j.employer) {
                by_b.entry(b).or_default().push(id);
                back.insert(id, b);
            }
        }
        (by_b, back)
    }

    #[allow(clippy::type_complexity)]
    fn residents_from_stores(&self) -> (BTreeMap<EntityId, Vec<EntityId>>, BTreeMap<EntityId, EntityId>) {
        let mut residents: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        let mut back = BTreeMap::new();
        for id in self.entities() {
            if let Some(home) = self.comp::<Household>(id).and_then(|h| h.home) {
                residents.entry(home).or_default().push(id);
                back.insert(id, home);
            }
        }
        (residents, back)
    }

    fn indices_from_stores(&self) -> ([Vec<EntityId>; 3], Vec<Vec<EntityId>>, StatSlots) {
        let mut tiers: [Vec<EntityId>; 3] = Default::default();
        let mut roles: Vec<Vec<EntityId>> = vec![Vec::new(); Role::ALL.len() + self.config.trades.len()];
        let mut slots = StatSlots::default();
        for id in self.entities() {
            if let Some(b) = self.comp::<Brain>(id) {
                tiers[b.lod as usize].push(id);
                if b.lod == Lod::Statistical {
                    slots.0[StatSlots::slot_of(id)].push(id);
                }
            }
            if let Some(j) = self.comp::<Job>(id) {
                let r = role_index(j.role);
                if r >= roles.len() {
                    roles.resize(r + 1, Vec::new());
                }
                roles[r].push(id);
            }
        }
        (tiers, roles, slots)
    }

    /// Compare the incremental indices with a rebuild from the stores.
    pub fn check_indices(&self) -> Result<(), String> {
        let (tiers, roles, slots) = self.indices_from_stores();
        if slots != self.stat_slots {
            return Err("stat_slots out of sync with the Statistical tier".to_string());
        }
        if tiers != self.by_tier {
            return Err(format!(
                "by_tier out of sync: have {:?}, stores say {:?}",
                self.by_tier.iter().map(Vec::len).collect::<Vec<_>>(),
                tiers.iter().map(Vec::len).collect::<Vec<_>>()
            ));
        }
        if roles != self.by_role {
            return Err(format!(
                "by_role out of sync: have {:?}, stores say {:?}",
                self.by_role.iter().map(Vec::len).collect::<Vec<_>>(),
                roles.iter().map(Vec::len).collect::<Vec<_>>()
            ));
        }
        let (employers, employer_of) = self.employers_from_stores();
        if employers != self.employers || employer_of != self.employer_of {
            return Err(format!(
                "employers out of sync: have {} workplaces / {} workers, stores say {} / {}",
                self.employers.len(),
                self.employer_of.len(),
                employers.len(),
                employer_of.len()
            ));
        }
        let (residents, resident_home) = self.residents_from_stores();
        if residents != self.residents || resident_home != self.resident_home {
            return Err("residents out of sync with the Household stores".to_string());
        }
        if self.gang_ids != self.with::<Gang>() {
            return Err(format!("gang_ids out of sync: {:?}", self.gang_ids));
        }
        if self.sentenced_ids != self.with::<Sentence>() {
            return Err(format!("sentenced_ids out of sync: {:?}", self.sentenced_ids));
        }
        if self.corp_ids != self.with::<Corp>() {
            return Err(format!("corp_ids out of sync: {:?}", self.corp_ids));
        }
        if self.squat_index != self.squat_index_from_stores() {
            return Err("squat_index out of sync with the Squatter store".to_string());
        }
        if let Some(idx) = self.report_index.get() {
            if *idx != Self::build_report_index(&self.crime_reports) {
                return Err("report_index out of sync with crime_reports".to_string());
            }
        }
        systems::assets::check(self)
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Living citizens: entities with an `Identity` and no `Corpse`, ascending.
    pub fn citizens(&self) -> Vec<EntityId> {
        self.entities().filter(|&id| self.has::<Identity>(id) && !self.has::<Corpse>(id)).collect()
    }

    pub fn population(&self) -> usize {
        self.citizens().len()
    }

    /// The first building of a kind (the only one for singletons).
    pub fn building_of_kind(&self, kind: BuildingKind) -> Option<EntityId> {
        self.buildings_by_kind.get(&kind).and_then(|v| v.first().copied())
    }

    /// Every building of a kind, ascending (map order), demolished included.
    pub fn buildings_of_kind(&self, kind: BuildingKind) -> &[EntityId] {
        self.buildings_by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// The building of `kind` whose door is nearest `from` by Manhattan
    /// distance; ties to the lower index; demolished ones skipped.
    pub fn nearest_of_kind(&self, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
        self.buildings_of_kind(kind)
            .iter()
            .filter_map(|&b| {
                self.comp::<Building>(b)
                    .filter(|bd| !bd.demolished && !bd.derelict)
                    .map(|bd| (bd.door.manhattan(from), b))
            })
            .min()
            .map(|(_, b)| b)
    }

    /// The building of `kind` an agent uses: the one it is inside, else its
    /// employer if that is of `kind` (a clerk's own Market, a bartender's own
    /// Bar, as the Farm rule already did), else the nearest door to its tile
    /// (M10 D21). Stable between planning and execution because the agent
    /// stands on the same tile at both.
    pub fn local(&self, agent: EntityId, kind: BuildingKind) -> Option<EntityId> {
        let pos = self.comp::<Position>(agent)?;
        // M12 D33: a building a riot looted is closed for trade.
        let now = self.tick;
        let usable = |b: EntityId| {
            self.comp::<Building>(b).is_some_and(|bd| {
                bd.kind == kind && !bd.demolished && !bd.derelict && bd.closed_until.is_none_or(|t| t <= now)
            })
        };
        if let Some(b) = pos.building.filter(|&b| usable(b)) {
            return Some(b);
        }
        if let Some(e) = self.comp::<Job>(agent).and_then(|j| j.employer).filter(|&e| usable(e)) {
            return Some(e);
        }
        if kind == BuildingKind::Market && self.config.corps.shop_price_tiles > 0 {
            return self.market_by_price(pos.tile);
        }
        self.nearest_of_kind(kind, pos.tile)
    }

    /// M11 D15: the Market a shopper at `from` picks, every tier alike: the
    /// least `door distance + shop_price_tiles × price` (ties lower id), so a
    /// cheaper Market draws custom from further off. Prices change only at
    /// midnight, so the choice holds from planning to execution. The price is
    /// read in tenths (phase 5), so a 2.7 beats a 3.0 by 1.2 tiles.
    /// Jobs and room P2 fix: an empty shelf is passed over while any Market
    /// holds food (seed 44's wages city starved 345 off-screen shoppers and
    /// thieves next to an empty Market for 37 days while the other two held
    /// 1,200 each); with every Market empty, the old pick.
    pub fn market_by_price(&self, from: TilePos) -> Option<EntityId> {
        let per_coin = self.config.corps.shop_price_tiles;
        self.buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| {
                let b = self.comp::<Building>(m).filter(|b| !b.demolished && !self.is_closed(m))?;
                let cost = 10 * i64::from(b.door.manhattan(from)) + per_coin * self.price_tenths_at(m);
                Some((b.stock_food == 0, cost, m))
            })
            .min()
            .map(|(_, _, m)| m)
    }

    /// Jobs and room P2 fix: the Market `agent` eats from: [`World::local`]'s
    /// while it holds food, else the stocked one [`World::market_by_price`]
    /// picks from the agent's tile (a Clerk's own empty shop, the empty
    /// Market it stands in).
    pub fn food_market(&self, agent: EntityId) -> Option<EntityId> {
        let local = self.local(agent, BuildingKind::Market);
        if local.and_then(|m| self.comp::<Building>(m)).is_some_and(|b| b.stock_food > 0) {
            return local;
        }
        let tile = self.comp::<Position>(agent)?.tile;
        self.market_by_price(tile).filter(|&m| self.comp::<Building>(m).is_some_and(|b| b.stock_food > 0)).or(local)
    }

    // -----------------------------------------------------------------------
    // M11: corps and owners
    // -----------------------------------------------------------------------

    /// Every corp, ascending by id (an index kept by the Corp hooks).
    pub fn corps(&self) -> Vec<EntityId> {
        self.corp_ids.clone()
    }

    /// Position of a corp in `corps()`: picks its colour in the app.
    pub fn corp_index(&self, corp: EntityId) -> usize {
        self.corps().iter().position(|&c| c == corp).unwrap_or(0)
    }

    /// M12 D33: a riot closed this building and the closure has not run out.
    pub fn is_closed(&self, b: EntityId) -> bool {
        self.comp::<Building>(b).and_then(|bd| bd.closed_until).is_some_and(|t| t > self.tick)
    }

    /// A building's owner (`None` = the city).
    pub fn owner_of(&self, building: EntityId) -> Option<EntityId> {
        self.comp::<Building>(building).and_then(|b| b.owner)
    }

    /// The corp owning a building, if its owner is one.
    pub fn corp_of_building(&self, building: EntityId) -> Option<EntityId> {
        self.owner_of(building).filter(|&o| self.has::<Corp>(o))
    }

    /// The corp an agent works for: the one it is exec of, else the one
    /// owning its employer. A daily-or-rarer query (the exec lookup scans corps).
    pub fn corp_of_agent(&self, agent: EntityId) -> Option<EntityId> {
        self.corps()
            .into_iter()
            .find(|&c| self.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(agent)))
            .or_else(|| self.comp::<Job>(agent).and_then(|j| j.employer).and_then(|e| self.corp_of_building(e)))
    }

    /// M11 D13: where an agent collects wages: the Civic Hall for a city job
    /// (an employer the city owns, or none), else the employer building.
    pub fn wage_desk(&self, agent: EntityId) -> Option<EntityId> {
        let employer = self.comp::<Job>(agent).and_then(|j| j.employer);
        match employer {
            Some(e) if self.owner_of(e).is_some() => Some(e),
            _ => self.building_of_kind(BuildingKind::Hall),
        }
    }

    /// One Market's food price (`price_initial` if it has no `Market`): the
    /// rounded price fines, the fence, theft losses and the UI read.
    pub fn price_at(&self, market: EntityId) -> i64 {
        self.comp::<Market>(market).map_or(self.config.world.price_initial, |m| m.price_food)
    }

    /// A Market's price in tenths of a coin (`price_food x 10` when it
    /// carries no fraction).
    pub fn price_tenths_at(&self, market: EntityId) -> i64 {
        self.comp::<Market>(market).map_or(self.config.world.price_initial * 10, Market::tenths)
    }

    /// M11 phase 5: the whole-coin price `agent` pays at `market` today. A
    /// fractional price rounds up with probability equal to its fraction,
    /// from a hash of (agent, day, Market): no RNG draw, the same from
    /// planning to paying within a day, the right mean over many shoppers.
    /// Exactly `price_at` when the price is whole.
    pub fn price_for(&self, market: EntityId, agent: EntityId) -> i64 {
        let t = self.price_tenths_at(market);
        let (whole, frac) = (t / 10, t % 10);
        if frac == 0 {
            return whole.max(1);
        }
        let day = self.tick / crate::time::TICKS_PER_DAY;
        let mut h = (u64::from(agent.index) << 32) ^ day ^ (u64::from(market.index) << 48);
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        (whole + i64::from((h % 10) < frac as u64)).max(1)
    }

    /// The rounded mean price over every Market: the city-wide reading for
    /// wealth, stats, fines, Jail upkeep, the fence and the UI.
    pub fn mean_price(&self) -> i64 {
        let prices: Vec<i64> = self
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| self.comp::<Market>(m))
            .map(|m| m.price_food)
            .collect();
        if prices.is_empty() {
            return self.config.world.price_initial;
        }
        let n = prices.len() as i64;
        (prices.iter().sum::<i64>() + n / 2) / n
    }

    pub fn treasury(&self) -> Option<&Treasury> {
        self.building_of_kind(BuildingKind::Hall).and_then(|id| self.comp::<Treasury>(id))
    }

    pub fn treasury_mut(&mut self) -> Option<&mut Treasury> {
        self.building_of_kind(BuildingKind::Hall).and_then(|id| self.comp_mut::<Treasury>(id))
    }

    /// M9: the law, on the Jail.
    pub fn law(&self) -> Option<&Law> {
        self.building_of_kind(BuildingKind::Jail).and_then(|id| self.comp::<Law>(id))
    }

    pub fn law_mut(&mut self) -> Option<&mut Law> {
        self.building_of_kind(BuildingKind::Jail).and_then(|id| self.comp_mut::<Law>(id))
    }

    /// Every crime report, oldest first.
    pub fn crime_reports(&self) -> &[CrimeReport] {
        &self.crime_reports
    }

    /// The reports, for writing; drops the per-suspect index.
    pub fn reports_mut(&mut self) -> &mut Vec<CrimeReport> {
        self.report_index.take();
        &mut self.crime_reports
    }

    /// Refresh the open report on `(suspect, crime)`, if any: its tick to
    /// now, its witness if it had none. Keeps a built index in step instead of
    /// dropping it (a rebuild walks every report; perf). `false` if none.
    pub fn refresh_open_report(
        &mut self,
        crime: crate::components::Crime,
        suspect: EntityId,
        witness: Option<EntityId>,
    ) -> bool {
        let tick = self.tick;
        let Some(r) = self.crime_reports.iter_mut().find(|r| !r.resolved && r.suspect == suspect && r.crime == crime)
        else {
            return false;
        };
        r.tick = tick;
        if r.witness.is_none() {
            r.witness = witness;
        }
        if let Some(idx) = self.report_index.get_mut() {
            if let Some(e) = idx.by_suspect.get_mut(&suspect) {
                e.1 = e.1.max(tick);
            }
        }
        true
    }

    /// Resolve every report on `suspect` (an arrest), keeping a built index
    /// in step (as `refresh_open_report`).
    pub fn resolve_reports_of(&mut self, suspect: EntityId) {
        for r in self.crime_reports.iter_mut().filter(|r| r.suspect == suspect) {
            r.resolved = true;
        }
        if let Some(idx) = self.report_index.get_mut() {
            if let Some(e) = idx.by_suspect.get_mut(&suspect) {
                e.0 = 0;
            }
            if let Ok(i) = idx.open.binary_search(&suspect) {
                idx.open.remove(i);
            }
        }
    }

    /// Drop every report on `suspect` (removed from the world), keeping a
    /// built index in step.
    pub fn drop_reports_of(&mut self, suspect: EntityId) {
        self.crime_reports.retain(|r| r.suspect != suspect);
        if let Some(idx) = self.report_index.get_mut() {
            idx.by_suspect.remove(&suspect);
            if let Ok(i) = idx.open.binary_search(&suspect) {
                idx.open.remove(i);
            }
        }
    }

    /// Append a report, keeping a built index in step (as `refresh_open_report`).
    pub fn push_report(&mut self, r: CrimeReport) {
        if let Some(idx) = self.report_index.get_mut() {
            let e = idx.by_suspect.entry(r.suspect).or_insert((0, 0));
            e.0 += u32::from(!r.resolved);
            e.1 = e.1.max(r.tick);
            if e.0 > 0 {
                if let Err(i) = idx.open.binary_search(&r.suspect) {
                    idx.open.insert(i, r.suspect);
                }
            }
        }
        self.crime_reports.push(r);
    }

    fn build_report_index(reports: &[CrimeReport]) -> ReportIndex {
        let mut m: BTreeMap<EntityId, (u32, Tick)> = BTreeMap::new();
        for r in reports {
            let e = m.entry(r.suspect).or_insert((0, 0));
            e.0 += u32::from(!r.resolved);
            e.1 = e.1.max(r.tick);
        }
        // M11 phase 2: the suspects with an open report, kept apart so the
        // guards' warrant checks do not walk 60 days of closed ones.
        let open = m.iter().filter(|(_, &(open, _))| open > 0).map(|(&s, _)| s).collect();
        ReportIndex { by_suspect: m, open }
    }

    fn report_index(&self) -> &ReportIndex {
        self.report_index.get_or_init(|| Self::build_report_index(&self.crime_reports))
    }

    /// `(open reports, latest report tick)` on `suspect`, if any report names them.
    pub fn reports_on(&self, suspect: EntityId) -> Option<(u32, Tick)> {
        self.report_index().by_suspect.get(&suspect).copied()
    }

    /// Suspects with an open report, ascending, each once.
    pub fn open_suspects(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.report_index().open.iter().copied()
    }

    /// Every gang, ascending by id (map order of their Hideouts).
    pub fn gangs(&self) -> Vec<EntityId> {
        self.gang_ids.clone()
    }

    /// The gang ids, ascending, borrowed (no clone for read-only scans).
    pub fn gang_list(&self) -> &[EntityId] {
        &self.gang_ids
    }

    /// The gang an agent belongs to.
    pub fn gang_of(&self, agent: EntityId) -> Option<EntityId> {
        self.comp::<GangMember>(agent).map(|g| g.gang).filter(|&g| self.has::<Gang>(g))
    }

    /// A gang's Hideout, if the building still exists.
    pub fn hideout_of(&self, gang: EntityId) -> Option<EntityId> {
        self.comp::<Gang>(gang).map(|g| g.hideout).filter(|&h| self.has::<Building>(h))
    }

    /// The other gang; with more than two, the one holding the most territory.
    pub fn rival_of(&self, gang: EntityId) -> Option<EntityId> {
        self.gang_ids
            .iter()
            .copied()
            .filter(|&g| g != gang)
            .filter_map(|g| self.comp::<Gang>(g).map(|gg| (std::cmp::Reverse(gg.territory.len()), g)))
            .min()
            .map(|(_, g)| g)
    }

    /// Position of a gang in `gangs()`: picks its colour in the app.
    pub fn gang_index(&self, gang: EntityId) -> usize {
        self.gang_ids.iter().position(|&g| g == gang).unwrap_or(0)
    }

    /// The agent's name, a building's `Kind#index`, an asset's `"{label}
    /// T{tier}"` (M13 D1), or `#index` for anything else.
    pub fn name_of(&self, id: EntityId) -> String {
        match self.comp::<Identity>(id) {
            Some(i) => i.name.clone(),
            None => match self.comp::<Building>(id) {
                // M15 W36: a Feed goes by its name.
                Some(b) if b.feed.is_some() => b.feed.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
                Some(b) => format!("{}#{}", b.kind.label(), id.index),
                None => match self.comp::<Asset>(id) {
                    Some(a) => format!("{} T{}", a.kind.label(), a.tier),
                    None => format!("#{}", id.index),
                },
            },
        }
    }

    pub fn tick_of_day(&self) -> u16 {
        time::tick_of_day(self.tick)
    }

    pub fn day(&self) -> u64 {
        time::day(self.tick)
    }

    pub fn phase(&self) -> time::DayPhase {
        time::phase(self.tick)
    }

    pub fn season(&self) -> time::Season {
        time::season(self.tick)
    }

    pub fn is_dark(&self) -> bool {
        time::is_dark(self.tick)
    }

    /// Called by the app every frame; queues a `SetView` command only when the
    /// rect differs from the last one queued (or applied, if none is pending),
    /// so a paused app does not flood the command log. Headless runs leave it `None`.
    pub fn set_view(&mut self, rect: Option<Rect>) {
        let pending = self.command_queue.iter().rev().find_map(|c| match c {
            PlayerCommand::SetView(r) => Some(*r),
            _ => None,
        });
        if pending.unwrap_or(self.view_rect) != rect {
            self.push_command(PlayerCommand::SetView(rect));
        }
    }

    // -----------------------------------------------------------------------
    // Tick
    // -----------------------------------------------------------------------

    /// One in-game minute, systems in the fixed order
    /// `commands, time, lod, needs, memory, mood, think, plan, exec, virt,
    /// ownership, assets, tech, classes, districts, economy, bind, word, law,
    /// social, gang, corp_brain, living, demography, stats` (L2 L36: `living`
    /// holds every L2 daily and hourly pass; M14 V39: `virt` relinks
    /// when dirty and pops run steps; `tech` is the plane's midnight pass;
    /// M15 W47: `word`'s midnight chain after the binder names its holes). The assets pass (M13 D9) runs at
    /// midnight right after ownership's, so upkeep and finance see the same
    /// purses the rent pass left. The district aggregates read the class pass's
    /// midnight state (M12 D6). The binder runs
    /// before the law so a cold-case report reaches the captain's daily
    /// rescoring (M10 D32); ownership's daily pass (rent, evictions,
    /// re-housing, upkeep) runs before the economy's price step (M11 D42).
    pub fn tick(&mut self) {
        self.apply_commands();
        // time: the clock is `self.tick`; daily hooks live in the systems that need them.
        systems::lod::run(self);
        crate::needs::run(self);
        systems::memory::run(self);
        crate::mood::run(self);
        systems::think::run(self);
        systems::plan::run(self);
        crate::exec::run(self);
        systems::virt::run(self);
        systems::ownership::run(self);
        systems::assets::run(self);
        systems::tech::run(self);
        systems::classes::run(self);
        systems::districts::run(self);
        systems::economy::run(self);
        systems::bind::run(self);
        systems::word::run(self);
        systems::law::run(self);
        systems::social::run(self);
        systems::gang::run(self);
        systems::corp_brain::run(self);
        systems::living::run(self);
        systems::contracts::run(self);
        systems::demography::run(self);
        systems::stats::run(self);
        self.tick += 1;
    }

    // -----------------------------------------------------------------------
    // Shared mutations used by several systems
    // -----------------------------------------------------------------------

    /// Add a memory, evicting the lowest `salience × recency` entry past the cap.
    pub fn remember(
        &mut self,
        id: EntityId,
        kind: MemoryKind,
        subject: Option<EntityId>,
        salience: f32,
        valence: f32,
        second_hand: bool,
    ) {
        let tick = self.tick;
        let mut cap = self.config.brain.memory_cap;
        let half_life = self.config.brain.memory_half_life_days;
        // A Statistical agent keeps only what the hourly tick can act on, in
        // eight slots; the entries survive promotion, when the cap becomes 24.
        // M10 D30: the off-screen crimes and courtships the hourly table now
        // produces leave their memories too. Starved too: `needs::starvation`
        // drifts lawfulness once a day by checking for today's entry, and
        // without it a starving Statistical agent drifted every hour.
        if self.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
            if !matches!(
                kind,
                MemoryKind::Grief
                    | MemoryKind::Starved
                    | MemoryKind::WasRobbed
                    | MemoryKind::MetInJail
                    | MemoryKind::Fought
                    | MemoryKind::Lost
                    | MemoryKind::Won
                    // L2 shadow fixes item 9: a Statistical fighter's bout.
                    | MemoryKind::Bout
                    | MemoryKind::Courted
                    | MemoryKind::Rejected
                    | MemoryKind::RentShort
                    | MemoryKind::Evicted
                    // Real economy E38: a parent's child taken (any tier).
                    | MemoryKind::ChildTaken
            ) {
                return;
            }
            cap = 8;
        }
        let entry = MemoryEntry { subject, salience, valence, second_hand, ..MemoryEntry::blank(kind, tick) };
        // M15 W15: a new deed memory may leave a grudge (never on a merge).
        let deed = systems::memory::deed_of(id, &entry);
        let Some(m) = self.comp_mut::<Memory>(id) else { return };
        let fresh = systems::memory::insert(m, entry, tick, cap, half_life);
        if let (true, Some(r)) = (fresh, deed) {
            systems::grudges::on_learn(self, id, &r, 1.0, u8::from(second_hand));
        }
    }

    /// The edge between two agents, if any.
    pub fn edge(&self, a: EntityId, b: EntityId) -> Option<&Edge> {
        self.edges.get(&edge_key(a, b))
    }

    /// The edge between two agents, created as an Acquaintance at affinity 0
    /// if absent. Keeps the neighbour index in step.
    pub fn edge_entry(&mut self, a: EntityId, b: EntityId) -> &mut Edge {
        let key = edge_key(a, b);
        let tick = self.tick;
        // One tree search when the edge exists (the usual case; perf).
        match self.edges.entry(key) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => {
                self.neighbours.entry(a).or_default().insert(b);
                self.neighbours.entry(b).or_default().insert(a);
                v.insert(Edge::new(RelKind::Acquaintance, tick))
            }
        }
    }

    pub fn remove_edge(&mut self, a: EntityId, b: EntityId) {
        self.edges.remove(&edge_key(a, b));
        for (x, y) in [(a, b), (b, a)] {
            if let Some(s) = self.enemies.get_mut(&x) {
                s.remove(&y);
            }
        }
        if let Some(n) = self.neighbours.get_mut(&a) {
            n.remove(&b);
        }
        if let Some(n) = self.neighbours.get_mut(&b) {
            n.remove(&a);
        }
    }

    /// A first name for `sex` from the tables (seeded).
    pub fn random_first_name(&mut self, sex: Sex) -> String {
        let table = match sex {
            Sex::Male => &self.names.first_male,
            Sex::Female => &self.names.first_female,
        };
        if table.is_empty() {
            return "Nameless".to_string();
        }
        let i = self.rng.world().random_range(0..table.len());
        table[i].clone()
    }

    /// A full name from the tables (seeded).
    pub fn random_name(&mut self, sex: Sex) -> String {
        let first = self.random_first_name(sex);
        let last = if self.names.last.is_empty() {
            "Doe".to_string()
        } else {
            let i = self.rng.world().random_range(0..self.names.last.len());
            self.names.last[i].clone()
        };
        format!("{first} {last}")
    }

    /// After a load: the tables are not saved.
    pub fn reload_names(&mut self) {
        self.names = NameTables::load(&self.config);
        let (table, legacy) = load_stat_table(&self.config);
        self.stat_table = table;
        self.stat_table_legacy = legacy;
        self.stat_mlp = load_stat_mlp(&self.config);
    }

    /// Set a `trace_flags` bit (ATE, SLEPT_AT_HOME) for today.
    pub fn mark_day(&mut self, id: EntityId, bit: u16) {
        *self.day_marks.entry(id).or_default() |= bit;
    }

    /// Remove an entity from every index and free it (emigrants, rotted or
    /// buried corpses). Edges to it are dropped.
    pub fn remove_agent(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        // M13 D20: a trip ends first, so the vehicle is parked, not lost.
        systems::vehicles::end_trip(self, id, false);
        // M16a (plan C16): an emigrant's records settle as a death's (a
        // refund lands in the wallet it leaves with).
        systems::contracts::on_death(self, id, None);
        // M11 D45: an emigrant's buildings pass to their heirs.
        systems::ownership::on_owner_gone(self, id);
        // M13 D13/D45: its parked vehicles too; an unsettled corpse settles
        // (the heirs are read from edges dropped below); what it carries goes.
        systems::assets::on_owner_gone(self, id);
        systems::assets::on_removed(self, id);
        self.abort_plan(id);
        self.vacate_job(id);
        self.remove_from_building(id);
        if self.has::<GangMember>(id) {
            systems::gang::leave(self, id, "gone");
        }
        crate::systems::social::unlink(self, id);
        for o in self.neighbours(id).collect::<Vec<_>>() {
            self.remove_edge(id, o);
        }
        self.neighbours.remove(&id);
        self.release_all(id);
        self.reservations.retain(|_, rs| {
            rs.retain(|r| !matches!(r.kind, crate::exec::ReservationKind::Partner { other } | crate::exec::ReservationKind::Corpse { corpse: other } | crate::exec::ReservationKind::Suspect { suspect: other } if other == id));
            !rs.is_empty()
        });
        self.pending_purchase.remove(&id);
        self.plan_queue.retain(|&(_, who), _| who != id);
        self.last_seen.remove(&id);
        self.vagrancy_places.remove(&id);
        // M12 D27/D20: a squatter's slot and a guest's bed are freed.
        self.remove::<Squatter>(id);
        self.hotel_beds.remove(&id);
        self.sweep_beats.remove(&id);
        crate::systems::bind::drop_victim_holes(self, id);
        self.drop_reports_of(id);
        for id2 in self.citizens() {
            if let Some(b) = self.comp_mut::<Brain>(id2) {
                if b.escorting == Some(id) {
                    b.escorting = None;
                }
                if b.cuffed_by == Some(id) {
                    b.cuffed_by = None;
                }
                if b.carrying_corpse == Some(id) {
                    b.carrying_corpse = None;
                }
            }
        }
        self.despawn(id);
    }

    /// After a load: the arena stores a save leaves out when empty
    /// (`grudges`, `broker`, `camp_raised` are not written while all `None`)
    /// or defaults (`#[serde(default)]`) are sized to the arena, and the
    /// litter grid to the map.
    pub fn resize_stores(&mut self) {
        let n = self.alive.len();
        self.law.resize_with(n.max(self.law.len()), || None);
        self.reputation.resize_with(n.max(self.reputation.len()), || None);
        self.grudges.resize_with(n.max(self.grudges.len()), || None);
        self.broker.resize_with(n.max(self.broker.len()), || None);
        self.camp_raised.resize_with(n.max(self.camp_raised.len()), || None);
        self.trace.resize_with(n.max(self.trace.len()), || None);
        self.life.resize_with(n.max(self.life.len()), || None);
        self.corp.resize_with(n.max(self.corp.len()), || None);
        self.squatter.resize_with(n.max(self.squatter.len()), || None);
        self.asset.resize_with(n.max(self.asset.len()), || None);
        self.body.resize_with(n.max(self.body.len()), || None);
        self.appearance.resize_with(n.max(self.appearance.len()), || None);
        let tiles = self.map.w() * self.map.h();
        if self.litter.len() != tiles {
            self.litter.resize(tiles, 0);
        }
    }

    /// Rebuild `neighbours`, `spouses` and `enemies` from `edges` (after a
    /// load; the indices are not saved).
    pub fn rebuild_indices(&mut self) {
        self.neighbours.clear();
        self.spouses.clear();
        self.enemies.clear();
        let living = |w: &World, id: EntityId| w.has::<Identity>(id) && !w.has::<Corpse>(id);
        let edges: Vec<((EntityId, EntityId), RelKind)> = self.edges.iter().map(|(&k, e)| (k, e.kind)).collect();
        for ((a, b), kind) in edges {
            self.neighbours.entry(a).or_default().insert(b);
            self.neighbours.entry(b).or_default().insert(a);
            match kind {
                RelKind::Spouse if living(self, a) && living(self, b) => {
                    self.spouses.insert(a, b);
                    self.spouses.insert(b, a);
                }
                RelKind::Enemy => {
                    self.enemies.entry(a).or_default().insert(b);
                    self.enemies.entry(b).or_default().insert(a);
                }
                _ => {}
            }
        }
        self.rebuild_tiers_and_roles();
        self.holes_by_agent.clear();
        for (&hid, h) in &self.holes {
            self.holes_by_agent.entry(h.victim).or_default().push(hid);
        }
        // M13 D2/D3: the asset indices, the corpses to settle, every Kit.
        systems::assets::rebuild(self);
        // M14 V1/V10: the plane's indices and the runners.
        systems::virt::rebuild_index(self);
        self.runner_of = self.runs.iter().map(|(&id, r)| (r.runner, id)).collect();
        // M15 W22/W35: the hunted and the guarded bodies.
        systems::hunt::reindex(self);
        systems::grudges::rebuild_guards(self);
        // L2 L15: who hangs out where.
        systems::leisure::rebuild_hangouts(self);
        // M16a (plan C2): the record indices.
        systems::contracts::rebuild_index(self);
    }

    /// Everyone `id` has an edge with, ascending.
    pub fn neighbours(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.neighbours.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// Marry two agents: a Spouse edge and the lookup in both directions.
    pub fn set_spouse(&mut self, a: EntityId, b: EntityId) {
        let tick = self.tick;
        let e = self.edge_entry(a, b);
        e.kind = RelKind::Spouse;
        e.last_interaction = tick;
        self.spouses.insert(a, b);
        self.spouses.insert(b, a);
    }

    pub fn spouse_of(&self, id: EntityId) -> Option<EntityId> {
        self.spouses.get(&id).copied()
    }

    /// Everyone on an Enemy edge with `id`.
    pub fn enemies_of(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.enemies.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// A SawCrime memory that also records which crime was seen and (M15
    /// W5) on whom (`object`).
    pub fn remember_crime(
        &mut self,
        id: EntityId,
        subject: EntityId,
        crime: Crime,
        salience: f32,
        object: Option<EntityId>,
    ) {
        let tick = self.tick;
        let cap = self.config.brain.memory_cap;
        let half_life = self.config.brain.memory_half_life_days;
        let Some(m) = self.comp_mut::<Memory>(id) else { return };
        let entry = MemoryEntry {
            subject: Some(subject),
            salience,
            valence: -salience,
            crime: Some(crime),
            object,
            ..MemoryEntry::blank(MemoryKind::SawCrime, tick)
        };
        // M15 W15: a witness to a wrong done to someone close may hold a
        // grudge. A victim noticing its own wrong learns it once, through
        // its own `Lost` / `WasRobbed` (fix round: one deed, one grudge).
        let deed = object.filter(|&o| o != id).and_then(|_| systems::memory::deed_of(id, &entry));
        let fresh = systems::memory::insert(m, entry, tick, cap, half_life);
        if let (true, Some(r)) = (fresh, deed) {
            systems::grudges::on_learn(self, id, &r, 1.0, 0);
        }
    }

    /// Remove the Job, posting a vacancy at the employer. Returns the old Job.
    pub fn vacate_job(&mut self, id: EntityId) -> Option<Job> {
        // M15 W28: a departure's share of its group's competence.
        systems::competence::note_departure(self, id);
        let job = self.remove::<Job>(id)?;
        if let Some(employer) = job.employer {
            self.vacancies.entry(employer).or_default().push(job.role);
        }
        Some(job)
    }

    /// Drop the agent from its building's occupant list; the Position is left
    /// to the caller (a leaver moves to the street, a corpse stays put).
    /// M14 V12 (review): a seated runner taken off the list (put out at the
    /// door by a sack or a demolition, moved by an arrest) is out of the
    /// chair, so its run is dumped first; a no-op for anyone not on a run.
    pub fn remove_from_building(&mut self, id: EntityId) {
        if self.runner_of.contains_key(&id) {
            systems::virt::dump(self, id, "pulled from the chair");
        }
        let Some(here) = self.comp::<Position>(id).and_then(|p| p.building) else { return };
        if let Some(b) = self.comp_mut::<Building>(here) {
            if let Ok(i) = b.occupants.binary_search(&id) {
                b.occupants.remove(i);
            }
        }
    }

    /// Death: every component but `Position` and `Identity` goes, a `Corpse`
    /// is added, the building and household forget the agent, the job is
    /// vacated, and the event and daily counter are recorded.
    pub fn kill(&mut self, id: EntityId, cause: DeathCause) {
        self.kill_by(id, cause, None);
    }

    /// `kill` with the killer named, so a gang knows whether a rival did it.
    pub fn kill_by(&mut self, id: EntityId, cause: DeathCause, killer: Option<EntityId>) {
        if !self.has::<Identity>(id) || self.has::<Corpse>(id) {
            return;
        }
        // L2 phase 4 (plan L25): the ledger and the kill-rate tally read the
        // victim's tier, class and gang before anything is unlinked.
        systems::fviolence::note_death(self, id, cause, killer);
        // M16a (plan C16): the dead's records settle while its wallet is
        // still its own (a refund to a dead buyer is its estate's).
        systems::contracts::on_death(self, id, killer);
        // M14 V12: a body jacked in is dumped from its run first.
        systems::virt::dump(self, id, "died in the chair");
        let tick = self.tick;
        let name = self.name_of(id);
        let child = self.has::<Child>(id);
        // M13 D20 (phase 1 review): the trip ends before the heirs are dealt,
        // so a vehicle being driven is parked and inherited, not lost with
        // the corpse.
        systems::vehicles::end_trip(self, id, false);
        // M13 D38: a dead dealer deals no more.
        systems::stims::end_deal(self, id);
        // L2 L15 (review fix): nor hangs out at a spot.
        systems::leisure::leave_spot(self, id);
        // M12 D17: a violent death leaves its mark on the street.
        if cause == DeathCause::Violence {
            if let Some((t, b)) = self.comp::<Position>(id).map(|p| (p.tile, p.building)) {
                systems::litter::deposit_near(self, t, b, 40, 1);
            }
        }
        // Widowhood is recorded on the Death event, and `on_death` unlinks the pair.
        let spouse = self.spouse_of(id);
        // A dead guard lets their suspect go; a dead suspect frees their guard.
        if let Some(b) = self.comp::<Brain>(id) {
            let (escorting, cuffed_by) = (b.escorting, b.cuffed_by);
            if let Some(s) = escorting {
                if let Some(sb) = self.comp_mut::<Brain>(s) {
                    sb.cuffed_by = None;
                }
            }
            if let Some(g) = cuffed_by {
                if let Some(gb) = self.comp_mut::<Brain>(g) {
                    gb.escorting = None;
                }
            }
        }
        // Witnesses, grief, widowhood and inheritance read the living state.
        // M13 D13: with a loot window the coins and goods stay on the body.
        let loot = systems::demography::on_death(self, id);
        // M15 W10: the kin are read before `on_death` unlinks the dead, and
        // the killing goes into the death's pool unnamed; a noticing
        // witness (`law::raise_crime_on`) or the binder names it.
        if cause == DeathCause::Violence {
            systems::gossip::post_killing(self, id);
        }
        // M15 W16: grudges on the dead settle, the dead's pass to its heirs
        // (read before `on_death` unlinks the spouse), its Hunts end.
        systems::grudges::on_death(self, id, killer);
        crate::systems::social::on_death(self, id);
        if cause == DeathCause::Violence && systems::law::is_guard(self, id) {
            systems::law_brain::push_shock(self, LawShock::GuardKilled);
        }
        // M11 D20: a corp loses an employee to violence.
        if cause == DeathCause::Violence {
            if let Some(corp) = self.comp::<Job>(id).and_then(|j| j.employer).and_then(|e| self.corp_of_building(e)) {
                systems::ownership::push_corp_shock(self, corp, CorpShock::EmployeeKilled);
            }
        }
        // M15 W28: a corp exec or the captain takes their competence share along.
        systems::competence::note_exec_death(self, id);
        self.vacate_job(id);
        self.remove_from_building(id);
        if self.has::<GangMember>(id) {
            systems::gang::on_member_killed(self, id, killer);
        }
        self.remove::<Needs>(id);
        self.remove::<Personality>(id);
        self.remove::<Mood>(id);
        self.remove::<Wallet>(id);
        self.remove::<Inventory>(id);
        self.remove::<Household>(id);
        self.remove::<Brain>(id);
        self.remove::<Memory>(id);
        self.remove::<Skills>(id);
        self.remove::<Sentence>(id);
        self.remove::<GangMember>(id);
        self.remove::<Child>(id);
        self.remove::<Squatter>(id);
        self.hotel_beds.remove(&id);
        self.sweep_beats.remove(&id);
        self.release_all(id);
        self.pending_purchase.remove(&id);
        self.plan_queue.retain(|&(_, who), _| who != id);
        self.insert(
            id,
            Corpse { died_tick: tick, cause, buried: false, buried_tick: None, loot, stripped: false, settled: false },
        );
        systems::assets::on_corpse(self, id);
        match cause {
            DeathCause::Starvation => self.stats.current.deaths_starvation += 1,
            DeathCause::OldAge => self.stats.current.deaths_old_age += 1,
            DeathCause::Violence | DeathCause::Execution => self.stats.current.deaths_violence += 1,
            DeathCause::Accident => self.stats.current.crash_deaths += 1,
            DeathCause::Overdose => self.stats.current.overdoses += 1,
            // M14 V14: a Flatline is not violence.
            DeathCause::Flatline => self.stats.current.virt.flatlined += 1,
        }
        // `[dead, spouse?]`, or `[dead, spouse or NONE, killer]` when the
        // killer is known, so the biography names them (an attacker who
        // lost the fight has no Murder event saying who killed them).
        let actors: smallvec::SmallVec<[EntityId; 3]> = match (spouse, killer) {
            (s, Some(k)) => smallvec::smallvec![id, s.unwrap_or(EntityId::NONE), k],
            (Some(s), None) => smallvec::smallvec![id, s],
            (None, None) => smallvec::smallvec![id],
        };
        self.push_event(
            crate::events::EventKind::Death,
            &actors,
            format!("{name} died of {cause:?}{}", if child { " (child)" } else { "" }),
        );
    }

    /// Rebuild a world from its seed and command log: the commands are queued
    /// at the ticks they were applied, and the world runs to `end_tick`.
    pub fn replay(seed: u64, config: Config, log: &[(Tick, PlayerCommand)], end_tick: Tick) -> World {
        let mut w = World::new(seed, config);
        let mut next = 0;
        while w.tick < end_tick {
            while next < log.len() && log[next].0 == w.tick {
                w.push_command(log[next].1.clone());
                next += 1;
            }
            w.tick();
        }
        w
    }

    /// Run `n` ticks.
    pub fn run_ticks(&mut self, n: u64) {
        for _ in 0..n {
            self.tick();
        }
    }
}
