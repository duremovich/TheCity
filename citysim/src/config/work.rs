use super::*;

/// M18 W (docs/M18_PLAYER.md § W): `[work_beat]`, the draw of the
/// `[[work_beats]]` deck at a shift's seams (`systems::workbeats`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkBeatCfg {
    /// A station block's end (not the shift's last) draws a beat on this.
    pub p_block: f32,
    /// The shift's break draws a beat on this (`at = "break"` rows).
    pub p_break: f32,
    /// A corp exec's office hour (`service::meeting_hour`) draws on this.
    pub p_hour: f32,
    /// At most this many beats a body a shift (an exec a day).
    pub max_per_shift: u8,
    /// A beat memory's salience, both sides.
    pub salience: f32,
}

impl Default for WorkBeatCfg {
    fn default() -> Self {
        WorkBeatCfg { p_block: 0.5, p_break: 0.8, p_hour: 0.5, max_per_shift: 6, salience: 0.3 }
    }
}

/// M18 W: where in the day a `[[work_beats]]` row is drawn.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BeatAt {
    /// A station block's end (`shift::block_done`).
    #[default]
    Block,
    /// The shift's break (`shift::stations_plan`, `haul_plan`).
    Break,
    /// A corp exec's office hour (`service::meeting_hour`).
    Hour,
}

/// M18 W: whom a beat reaches (all on shift, adults, not the body itself).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BeatWho {
    /// A coworker present (`service::present`), picked by a keyed hash.
    #[default]
    Colleague,
    /// The coworker present with the warmest edge to the body (ties the
    /// lower id); none with an edge, a hashed colleague.
    Partner,
    /// The foreman present, else the present coworker of the highest grade
    /// above the body's.
    Superior,
    /// The workplace's person owner, else its corp's exec, else the foreman.
    Boss,
    /// An exec's beat: a staffer present at the Hq, hashed.
    Staff,
}

/// M18 W: what a beat does (only through `social`, memories and gossip).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BeatEffect {
    /// The co-work drift (`social::interacted`) plus the row's deltas.
    #[default]
    Drift,
    /// The row's affinity and trust deltas only (`social::adjust`).
    Nudge,
    /// The deltas and a tick of the body's trade skill (`skill_gain`).
    Skill,
    /// The counterpart tells the body one deed it holds (`gossip::tell`);
    /// nothing to tell, no beat.
    Rumour,
}

/// M18 W: one `[[work_beats]]` row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkBeatRow {
    pub id: String,
    /// The jobs it reaches: `[[trades]]` ids, built-in `Role` names,
    /// `"exec"` (a corp exec at the Hq), `"*"` (every shaped shift).
    pub jobs: Vec<String>,
    pub at: BeatAt,
    /// `[from, to)` minutes of the day (`from > to` wraps midnight).
    pub hours: (u16, u16),
    /// The row's weight among the rows open at the seam.
    pub weight: f32,
    pub who: BeatWho,
    pub effect: BeatEffect,
    /// The edge's affinity and trust step (both ways: edges are undirected).
    pub affinity: f32,
    pub trust: f32,
    /// The memory both sides keep, its valence.
    pub memory: crate::components::MemoryKind,
    pub valence: f32,
    /// `Skill`: the trade skill's tick.
    pub skill_gain: f32,
    /// At most once a shift.
    pub once: bool,
    /// The line: `{me}`, `{them}`, `{at}` (the workplace), `{about}` (a rumour's deed).
    pub text: String,
}

impl Default for WorkBeatRow {
    fn default() -> Self {
        WorkBeatRow {
            id: String::new(),
            jobs: Vec::new(),
            at: BeatAt::Block,
            hours: (0, 1440),
            weight: 1.0,
            who: BeatWho::Colleague,
            effect: BeatEffect::Drift,
            affinity: 0.0,
            trust: 0.0,
            memory: crate::components::MemoryKind::Socialised,
            valence: 0.1,
            skill_gain: 0.0,
            once: false,
            text: String::new(),
        }
    }
}

/// The island pass phase 8 `[island_trades]` (docs/ISLAND_PASS.md I4;
/// OPEN-168 part). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IslandTradesCfg {
    /// A corp's parcels go through the open Yard (with room) that has the
    /// most of the corp's staffed buildings within this travel of its door
    /// (`distance::travel`), those buildings the recipients; with none in
    /// reach of any Yard, the base pick (the Yard nearest the corp's first
    /// building, `[jobs2] site_reach`).
    pub parcel_cap_ticks: u64,
    /// The recipient buildings one corp's parcels go to (the base's six).
    pub parcel_stops_max: u8,
    /// A haul trip's estimate reads `distance::travel` for the carry (the
    /// base: the doors' Manhattan distance at the walk's speed).
    pub haul_travel: bool,
    /// A Construction Yard's Builder whose Yard has open sites but none in
    /// `[shift.haul] haul_reach` carries one load out to the nearest site and
    /// works the rest of the shift there (the base: the trips ran inside the
    /// Yard).
    pub site_day: bool,
    /// The labour corner stands outside every open Construction and Freight
    /// Yard (one within `[jobs2] corner_tiles` of another shares it), not
    /// one a district (the island's Yards sit 380 tiles from the Vats'
    /// lowest-id one).
    pub corner_per_yard: bool,
    /// An off-screen broke jobless past `[jobs2] corner_stat_reach` tiles of
    /// every corner queues at the corner nearest his Home by travel within
    /// this (on `p_corner`, as the base's within the tiles).
    pub corner_stat_ticks: u64,
    /// The cab drivers on shift at each Garage that keep a body (ranked with
    /// the gangs, `lod::class_with`), lowest id first.
    pub cab_quota: u8,
    /// A corner dealer whose gang holds an armoury surplus sells it at the
    /// gang's deal Bar (`stims::deal_bar`, the nearest Bar when the gang
    /// deals at a corner) as the base's bar dealer does.
    pub corner_arms: bool,
}

/// The pressure pass phase 2 `[customers]` (docs/PRESSURE_PASS.md § Phase 2;
/// `systems/customers.rs`). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomersCfg {
    /// The door: a visitor with a cause (drunk or high, a brawl within
    /// `door_brawl_days`, a rival gang's colours, a dress under
    /// `door_dress_min`) is turned away by a bouncer on duty on `door_p`.
    pub door_brawl_days: u32,
    pub door_dress_min: u8,
    /// A visitor carrying a weapon is a cause.
    pub door_armed: bool,
    /// Round 6 V6 (OPEN-217): a visitor short of the door's price now (his
    /// coins in hand under `leisure::price_of`) is a cause, turned away
    /// every time ("broke"), both tiers; the off-screen visitor meets the
    /// door before his purse does (`leisure::stat_visit`).
    #[serde(default)]
    pub door_broke: bool,
    /// Round 6 V6: an off-screen drink leaves the mark the door reads
    /// (`Misery.drank_at`; the Statistical memory keeps no drink).
    #[serde(default)]
    pub door_stat_drink: bool,
    /// Round 7 W6 (OPEN-217): a visitor whose name runs hot (`Reputation.heat`
    /// at or above this: the deeds the word carries, both tiers) is trouble
    /// at a kept door ("trouble", on `door_p`); 0 off.
    #[serde(default)]
    pub door_trouble_heat: f32,
    /// Round 7 W6: an off-screen face barred from a house comes back to its
    /// door anyway on this (a hash of him, the venue and the day; drunk,
    /// always) and is turned away "barred"; else he stays away (round 4 T7's
    /// `door_seen`). 0: the barred off screen always stay away.
    #[serde(default)]
    pub door_stat_barred_p: f32,
    pub door_p: f32,
    /// A turned-away drunk or brawler swings at the bouncer on
    /// `door_swing_p × (1 − lawfulness)`; the turned-away holds `door_grudge`.
    pub door_swing_p: f32,
    pub door_grudge: f32,
    /// Clients: intimacy under `lonely_intimacy` is lonely (0..1 below it);
    /// a venue with a companion, a host or a dancer on duty scores `× (1 +
    /// lonely_pull × lonely + misery_pull × misery)`; Unwind's need is
    /// `max(U(fun), lonely_unwind × lonely)` for such a pick, with
    /// `client_coins` in the purse.
    pub lonely_intimacy: f32,
    pub lonely_pull: f32,
    pub misery_pull: f32,
    pub lonely_unwind: f32,
    pub client_coins: i64,
    /// Arms: safety under `fear_safety` is a reason to want a gun (the
    /// frightened buy one at a dealer's Bar or, licensed, a Gunsmith).
    pub fear_safety: f32,
    /// Care: a lost fight within `hurt_hours` is a wound to have seen to: a
    /// Clinic's patch-up at `patch_price`, or the back room's at
    /// `back_price` for the wanted, a gang member, or one short of the
    /// Clinic's price; `patch_min` minutes on the table.
    pub hurt_hours: u32,
    pub patch_price: i64,
    pub back_price: i64,
    pub patch_min: u32,
    /// Care: a diagnosed cancer whose pool falls short of the Clinic's
    /// treatment buys the back room's at `back_onco_price`: a remission of
    /// `back_onco_mult ×` the Clinic's, never a cure.
    pub back_onco_price: i64,
    pub back_onco_mult: f32,
    /// The back room's founder: while fewer stand than one per
    /// `back_room_adults` adults, a struck-off doc (else the least lawful
    /// licensed Ripperdoc under `rogue_lawful`, who leaves his Clinic for
    /// it) opens one: a vacant Lot he can pay for, else a derelict refitted.
    pub back_room_adults: u32,
    pub rogue_lawful: f32,
    /// A back room is walked to from within `back_reach_ticks` of travel.
    pub back_reach_ticks: u64,
    /// The cab: a body's GoTo of `hail_min_tiles` or more, not to work,
    /// hails a cab once a day on `p_hail` when it holds `hail_rich_coins`
    /// or is hurried (Treat, Flee, Rescue), with the fare.
    pub hail_min_tiles: u32,
    pub hail_rich_coins: i64,
    pub p_hail: f32,
    /// A cab driver's body is paid `min(1, fares / cab_fares_full)` of the
    /// day's wage (no fare, no pay for Rest at the Garage).
    pub cab_fares_full: u16,
    /// The shape-up: every open Construction and Freight Yard posts
    /// `shape_up_posts` day posts at the dawn pick; a broke jobless body
    /// walks to a corner within `shape_up_reach` tiles (`[jobs2] corner_reach` off).
    pub shape_up_posts: u8,
    pub shape_up_reach: u32,
    /// The vendor: a working stall sells up to `vendor_per_hour` meals an
    /// hour to bodies on the street within `vendor_tiles` with hunger under
    /// `vendor_hunger`.
    pub vendor_tiles: u32,
    pub vendor_per_hour: u8,
    pub vendor_hunger: f32,
    /// The scav's day: a scav gang's member walks `scav_reach` tiles to a
    /// Market's bin on the gang's round (off hours), or to a fresh body.
    pub scav_reach: u32,
}

/// Pressure pass round 2, R6a `[trade_verbs]` (docs/PRESSURE_PASS.md §
/// R6a, OPEN-217). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeVerbsCfg {
    /// The pimp's round: a pitch collected at within this many ticks is
    /// visited after the pitches owed a take and the ones not yet seen.
    pub pimp_revisit_ticks: u64,
    /// ... and a pitch farther than this many tiles from him only when no
    /// nearer one is due.
    pub pimp_round_tiles: u32,
    /// The pit: an off-screen visitor of a FightPit in the bet window with
    /// `[leisure] gamble_rung_courage` stakes on tonight's bout (not a table
    /// game), at most this many bettors a pit a night (the house's book).
    pub pit_bettors_max: u32,
    /// The madam books every Brothel visit served while she is on duty: her
    /// night's bookings are told in one line at her shift's end, naming at
    /// most this many of the house's workers.
    pub madam_names: usize,
    /// A booked client turns rough on this x (1 - lawfulness) (a hash of
    /// the pair and the night): the worker hurt, the madam's grudge and the
    /// house's door shut to him for `rough_ban_days`.
    pub rough_p: f32,
    pub rough_ban_days: u64,
    /// The cab: an off-screen rider waits this many ticks for a driver's
    /// body (a body waits `[legs] fare_wait_ticks`); the Garage's off-screen
    /// cab past it.
    pub stat_fare_wait_ticks: u64,
    /// The Dollhouse's off-screen evening rate (visits per adult a day, as
    /// the learned table's `p_venue`, which has no Dollhouse row).
    pub dollhouse_p_venue: f32,
    /// Round 5 U6 (OPEN-217): the pimp's body at a dry pitch (nothing taken
    /// since his last collection) draws this many off-screen residents of
    /// the district for a client to send her (the first adult with the
    /// street's wants and price); 0 off.
    pub pimp_book_draws: u32,
    /// Round 5 U6: a named pimp holding a day post quits it for the round
    /// (and the job market passes him over while he holds the street).
    pub pimp_quits: bool,
    /// Round 5 U6: the round's trip bound (with `[distance.choice]`): a
    /// pitch past this from where the pimp stands is no round this hour, and
    /// nobody living past it from the district's nearest pitch is named.
    pub pimp_trip_ticks: u64,
}

impl Default for TradeVerbsCfg {
    fn default() -> Self {
        TradeVerbsCfg {
            pimp_revisit_ticks: 180,
            pimp_round_tiles: 120,
            pit_bettors_max: 12,
            madam_names: 3,
            rough_p: 0.03,
            rough_ban_days: 14,
            stat_fare_wait_ticks: 120,
            dollhouse_p_venue: 0.002,
            pimp_book_draws: 8,
            pimp_quits: true,
            pimp_trip_ticks: 120,
        }
    }
}

/// The pre-view pass phase 5 `[legs]` (docs/PREVIEW_PASS.md § 2 "20k
/// legs"). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegsCfg {
    /// A Full or Coarse walking `GoTo` farther than this (Manhattan tiles)
    /// walks in segments of this length, each end re-planning the rest
    /// (the ride re-checked).
    pub leg_replan_tiles: u32,
    /// A segment's end snaps to a walkable Road tile within this radius.
    pub segment_snap: u32,
    /// P12: an escort walks (at walking speed) up to this many tiles; past
    /// it the van.
    pub escort_walk_tiles: u32,
    /// P12: the van's arrival at the arrest, in ticks, before the drive.
    pub van_call_ticks: u32,
    /// OPEN-140 remainder: an exec driven to his Hq is driven home too.
    pub exec_ride_home: bool,
    /// OPEN-140 remainder: the away-from-home bed reads the trip home by
    /// ride (the El, the exec's car), not the walk.
    pub bed_by_ride: bool,
    /// OPEN-156: the sweeper's walk back to the Recycler reads the ride.
    pub sweep_by_ride: bool,
    /// The `fares` shape (OPEN-115): a booked cab rider waits at his door
    /// for a driver's body that can reach him within this (cab speed);
    /// farther, the Garage's off-screen cab.
    pub fare_wait_ticks: u32,
    /// The `fares` shape: a commute of `[jobs2] cab_min_tiles` or more that
    /// the El does not shorten takes the cab when the purse allows (no
    /// `p_cab` roll), both tiers (`streetwork::cab_roll`).
    pub fares_el_missed: bool,
    /// The island pass phase 4 (OPEN-164): the plan timeout (`[brain]
    /// plan_timeout_ticks`) spares a leg in motion (a timed walk, a flight
    /// or a ride before its arrival, a walk with its path left and not
    /// blocked). False (and not written into a save) with
    /// the key absent: the base timeout, which cut a
    /// 931-tick ride at 900 and left the body at the board stop.
    #[serde(default, skip_serializing_if = "is_false")]
    pub timeout_spares_legs: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// The pre-view pass phase 4 `[presence]` (docs/PREVIEW_PASS.md § 0 P3):
/// an encounter posts when, and only when, both bodies are present. Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresenceCfg {
    /// OPEN-154: a round's stops at most this many tiles from the post's door.
    pub round_reach_tiles: u32,
    /// A Statistical counterpart at home (not on shift, not the night) is
    /// drawn present at this rate (purpose-keyed).
    pub stat_home_p: f32,
    /// P8: the sweeper's round: street stops a shift (dirtiest first, within
    /// `round_reach_tiles` of the first).
    pub sweep_stops: u8,
    /// P8: minutes swept at each stop.
    pub sweep_stop_min: u16,
    /// P8: a stop cleans the street tiles within this Chebyshev radius.
    pub sweep_radius: u8,
    /// P8: a round's walk is estimated at the stops' Manhattan tiles times
    /// this (the street grid is no straight line).
    pub sweep_walk_factor: f32,
    /// An off-screen arms buyer's meeting waits this many hours for the dealer's body at the Bar.
    pub arms_wait_hours: u16,
    /// Round 3 S4 (OPEN-216): a super's round with no other Block of the
    /// landlord's in reach is his own post's Block (its tenants served:
    /// the notices, the door rent), not no round at all.
    #[serde(default)]
    pub round_post_stop: bool,
    /// Round 3 S4: the jobs a super's legs at the post are (the rest of his
    /// shift past the round, worked in stations blocks: `shape = "stations"`
    /// on his trade row), one drawn per block (purpose-keyed): a repair in a
    /// tenant's flat, a tenant's complaint in the hall, the stairs walked.
    /// Empty: the blocks are bare work.
    #[serde(default)]
    pub super_legs: Vec<SuperLeg>,
    /// Round 3 S4: what a repair leg fixes (drawn), the diary's words.
    #[serde(default)]
    pub super_repairs: Vec<String>,
    /// Round 3 S4: what a tenant's complaint is about (drawn).
    #[serde(default)]
    pub super_complaints: Vec<String>,
    /// Round 3 S4: a tenant a super saw to (a repair, a complaint) is not
    /// the next leg's for this many days.
    #[serde(default)]
    pub super_leg_days: u32,
}

/// Round 3 S4 (OPEN-216): a super's leg at his post (`[presence] super_legs`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuperLeg {
    /// A repair in a tenant's flat (any tenant: he has the keys).
    Repair,
    /// A tenant at home stops him in the hall with a complaint.
    Complaint,
    /// The stairs and halls walked, floor by floor.
    Stairs,
}

/// The pre-view pass phase 2 (docs/PREVIEW_PASS.md § 0 P1): the shape a
/// shift is worked in. Phases 3-6 append `haul`, `counter` (and work
/// `round`/`fare` through `systems/shift.rs`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShiftShape {
    /// The one work block at the post, the shift's whole length (the old shift).
    #[default]
    Post,
    /// § 1 P2: 2-`station_spots_max` spots inside the workplace, worked in
    /// `station_block_min`..`station_block_max` minute blocks, one break.
    /// Saved as `spots` (an off `[law] stations` city's save holds no
    /// `station`); `stations` in the file.
    #[serde(rename = "spots", alias = "stations")]
    Stations,
    /// The pre-view pass phase 3 (§ 1): trips from the workplace's stock
    /// tile to a destination (a Market, a Warehouse, an open site, a
    /// parcel's address) and back, `[shift.haul]`; the courier's parcels.
    Haul,
    /// The pre-view pass phase 3 (§ 1): the counter or the door tile stood,
    /// arrivals served as they arrive, `[shift.counter]`.
    Counter,
}

/// The pre-view pass phase 3 `[shift.haul]` (docs/PREVIEW_PASS.md § 1).
/// `None` drops the shape. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HaulCfg {
    /// Units a trip carries (the device row; pay and output do not move, P5).
    pub haul_load: u16,
    /// No new trip starts within this many minutes (its estimate added) of
    /// the shift's end: the rest is worked at the post, where it closes.
    pub haul_cutoff_min: u16,
    /// The load at the stock tile and the unload at the destination, minutes each.
    pub haul_handle_min: u16,
    /// A destination's door at most this many tiles from the post's door.
    pub haul_reach: u32,
    /// The most destinations a shift's trips turn through.
    pub haul_dests_max: u8,
    /// The trade ids that drive the post's fleet truck when one is parked
    /// there (`vehicles::claim_haul`); the rest take what the GoTo finds.
    #[serde(default)]
    pub drive: Vec<String>,
}

/// The pre-view pass phase 3 `[shift.counter]` (docs/PREVIEW_PASS.md § 1).
/// `None` drops the shape. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CounterCfg {
    /// An off-screen visit to a venue with a counter body is served in the
    /// `counter_settle_window`-minute slot it arrives in, through the body's
    /// shift, not in the 21:00 evening batch.
    pub counter_settle_window: u16,
    /// The built-in roles (by the `Role`'s name) that stand the counter,
    /// over `[shift.roles]` (the Cook, OPEN-145). `counter_staff` in a save
    /// (an off `[careers]` city's save holds no `roles:`).
    #[serde(default, rename = "counter_staff", alias = "roles")]
    pub roles: Vec<String>,
}

/// The pre-view pass phase 2 `[shift]` (docs/PREVIEW_PASS.md § 1). Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShiftCfg {
    /// P2: a stations block's length in minutes (a trade row may override both).
    // Saved as `block_min`, `block_max`, `spots_max`, `move_max` (an off
    // `[law] stations` city's save holds no `station`); the file's names read.
    #[serde(rename = "block_min", alias = "station_block_min")]
    pub station_block_min: u16,
    #[serde(rename = "block_max", alias = "station_block_max")]
    pub station_block_max: u16,
    /// P2: the most distinct spots a shift works (at least 2, at most the
    /// workplace's interior tiles).
    #[serde(rename = "spots_max", alias = "station_spots_max")]
    pub station_spots_max: u8,
    /// P2: the walk across the room between two spots, at most this many
    /// minutes (the tiles at `[exec] move_ticks_full` otherwise).
    #[serde(rename = "move_max", alias = "station_move_max")]
    pub station_move_max: u16,
    /// P7: the mid-shift break in minutes.
    pub break_min: u16,
    /// P7: the break's start, staggered per body per shift by up to this
    /// many minutes either side of mid-shift.
    pub break_stagger: u16,
    /// P7: a hungry worker's meal break walks to a food venue at most this
    /// many minutes away (one way); farther, the break is outside the door.
    pub break_walk_max: u16,
    /// P1: the shape of a trade whose `duty` is `post`.
    pub trade_post: ShiftShape,
    /// P1: the built-in roles' shapes, keyed by the `Role`'s name (absent:
    /// `post`); `[shift.roles]` in the file, `role_shapes` in a save (an off
    /// `[careers]` city's save holds no `roles:`).
    #[serde(default, rename = "role_shapes", alias = "roles")]
    pub roles: std::collections::BTreeMap<String, ShiftShape>,
    /// The pre-view pass phase 3: the `haul` shape's numbers (`None` off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haul: Option<HaulCfg>,
    /// The pre-view pass phase 3: the `counter` shape's numbers (`None` off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<CounterCfg>,
}

/// Lives at work phase 6 `[careers]` (docs/LIVES_AT_WORK.md § 4.1-4.4).
/// Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CareersCfg {
    pub enabled: bool,
    /// § 4.2: a seeded Job's seniority is `min(years over the row's
    /// min_age, this) × DAYS_PER_YEAR × u` days.
    pub seniority_cap_years: u32,
    /// § 4.2: the foreman is the most experienced worker this old or older.
    pub foreman_min_age: u32,
    /// § 4.2: the foreman's wage multiplier.
    pub foreman_premium: f32,
    /// § 4.3: shifts worked in a trade before it is the profession.
    pub profession_min_days: u32,
    /// § 4.3: a candidate holding the vacancy's profession ranks this many
    /// tiles nearer (the hiring key's offset, capped with skill and the
    /// rehire bonus at 512).
    pub profession_w: u32,
    /// § 4.4: a worker this old retires on savings or kin ...
    pub retire_from_years: u32,
    /// ... when the household's coins cover this many days of its costs.
    pub retire_days: u32,
    /// § 4.4: a worker whose `ages::work_mult` falls under this is let go.
    pub frail_exit_mult: f32,
    /// § 4.4: a retired agent under this many coins is retired no more.
    pub unretire_coins: i64,
    /// Round 8 X3 (`OPEN-247`): a sentence this many days or longer is a
    /// record (`Misery.record_at`); a night in the cells is none.
    pub record_min_days: u32,
    /// Round 8 X3: a post that reads a record (`[[trades]] reads_record`,
    /// `[roles.<Name>] reads_record`) turns away anyone sentenced within
    /// this many days (0: for good).
    pub record_days: u32,
    /// Pressure pass round 9, Y2 (`OPEN-250`): ... and anyone put out of
    /// his Home within this many days (the arrears on the record; 0 off).
    pub evicted_days: u32,
}

impl Default for CareersCfg {
    fn default() -> Self {
        CareersCfg {
            enabled: false,
            seniority_cap_years: 30,
            foreman_min_age: 30,
            foreman_premium: 1.25,
            profession_min_days: 14,
            profession_w: 400,
            retire_from_years: 62,
            retire_days: 60,
            frail_exit_mult: 0.6,
            unretire_coins: 20,
            record_min_days: 2,
            record_days: 365,
            evicted_days: 14,
        }
    }
}

impl CareersCfg {
    /// Not written into a save at its defaults (off).
    pub fn is_default(&self) -> bool {
        *self == CareersCfg::default()
    }
}

/// Lives at work phase 6 `[roles.<Name>]` (§ 4.1): a legacy role's age band
/// in years (0 none).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoleCfg {
    pub min_age: u32,
    pub max_age: u32,
    /// Round 8 X3 (`OPEN-247`): the `[[trades]] no_foreman` of a legacy role
    /// (the Fighter under the pit).
    pub no_foreman: bool,
    /// Round 8 X3: the hire reads a record (`[careers] record_days`): the
    /// law's posts.
    pub reads_record: bool,
    /// Pressure pass round 9, Y2 (`OPEN-250`): the hire reads the body: a
    /// diagnosed cancer or a man off sick today is turned away (the Guard).
    pub reads_body: bool,
}

/// Jobs & lives `[jobs2]` (docs/JOBS_AND_LIVES.md §§ 1-2): the master
/// switch in flight and phase 1's keys (the industries, the civic three,
/// the Hq). Every number is a placeholder. Off (the default when the table
/// is absent), no new kind seeds, no new trade row posts, no new flow books.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Jobs2Cfg {
    pub enabled: bool,
    /// § 2: Markets cut their synth shelf from Rations stock (off: it moves
    /// the diet rates and parity; no code reads it before the gate).
    pub rations_to_markets: bool,
    /// § 1: the appended goods' stock cap at a building (at least the
    /// kind's `stock_cap`).
    pub goods_cap: u32,
    // --- Packing (§ 2).
    /// Food units a Packer packs an on-duty hour (× 0.5 + her farming skill).
    pub pack_rate: f32,
    /// Rations made per Food unit packed.
    pub pack_ratio: f32,
    /// The World's Rations bid is this × its Food bid (`world_market::bid_ref`).
    pub packed_premium: f32,
    /// Rations a plant keeps back at the midnight sale.
    pub rations_floor: u32,
    /// A Food corp's Grow builds a Packing Plant while the World's Food
    /// fill is above this ...
    pub pack_fill: f32,
    /// ... and it owns fewer plants than its Farms ÷ this.
    pub farms_per_plant: u32,
    // --- Freight (§ 2).
    /// Coins a unit sold to the World pays the nearest Freight Yard.
    pub handling_per_unit: f32,
    /// Of an import's coins, to the nearest Freight Yard (an import names
    /// no units; JOBS_V2 J23's rate on value).
    pub handling_rate: f32,
    /// Coins a unit of a cross-owner Farm -> Market haul pays the Yard (the Drivers).
    pub haul_per_unit: f32,
    /// A Yard with no Loader on the last night shift passes the fee to the
    /// World (the leak JOBS_V2 named) instead of taking it.
    pub loader_gate: bool,
    /// Per Loader shift: a Ration walks out with the Loader (`Pilfered`).
    pub p_pilfer: f32,
    // --- Power (§ 2).
    /// Of the power a Utility Plant takes, to the World (its fuel).
    pub fuel_share: f32,
    /// Hours with no Plant Operator on shift before its districts go dark.
    pub outage_hours: u8,
    // --- Construction (§ 2).
    /// Days a build stands closed with the Yard's full crew.
    pub crew_days: f32,
    /// The slowest build: `crew_days` × this (a Yard with no Builders).
    pub crew_slow_max: f32,
    /// A Builder's round visits the Yard's sites within this many tiles
    /// (Manhattan, door to door); a farther site is built from the Yard.
    pub site_reach: u32,
    // --- The civic three (§ 2), after police and Sanitation.
    /// One Jailer per this many held.
    pub held_per_jailer: u32,
    pub clerks_max: u16,
    pub depot_hands_max: u16,
    /// A Jailer short (fewer than the held need) multiplies a breach's odds.
    pub jailer_short_mult: f32,
    // --- Seeding (row indices into `[corps] names`, never names).
    /// One Packing Plant per copy of each row.
    pub packing_rows: Vec<usize>,
    pub freight_rows: Vec<usize>,
    pub utility_rows: Vec<usize>,
    pub construction_rows: Vec<usize>,
    /// An Hq per copy of each megacorp row (`[corps] megacorp`) at seed.
    pub hq_megacorps: bool,
    // --- The Hq (§ 2).
    /// A local corp owning this many standing buildings builds an Hq.
    pub hq_min_buildings: usize,
    /// ... keeping this much of its treasury after the build.
    pub hq_reserve: i64,
    /// JOBS_V2 J30: an Hq's office costs at most this share of its corp's
    /// 7-day mean revenue a day (the overhead rows' places, `industry::hq_places`).
    pub admin_share: f32,
    // --- Phase 2, the street (§ 3): `street` is its sub-switch (read only
    // under `enabled`; `streetwork::on`).
    pub street: bool,
    /// Open Pitches a district holds at most, per 2,000 residents (scaled).
    pub pitches_per_district: u16,
    /// Coins a jobless adult holds to open a Pitch ...
    pub pitch_stake: i64,
    /// Shadow V4 bug 6: a knowledge row's hire has at least this knowledge.
    pub hire_knowledge_floor: f32,
    /// ... after this many days without a post ...
    pub pitch_after_days: u32,
    /// ... on this chance a day (a pure hash of the day and the agent).
    pub p_pitch_open: f32,
    /// A Pitch with no take for this many days closes.
    pub pitch_fail_days: u8,
    /// A body stands its Pitch within this many tiles of the spot.
    pub pitch_reach: u32,
    /// The share of a Pitch's day's take paid to the gang holding its
    /// district (`Flow::StreetCut`).
    pub cut_frac: f32,
    /// Knowledge at which a pitcher tinkers, fighting at which he pulls a rickshaw.
    pub tinker_skill: f32,
    pub rickshaw_skill: f32,
    /// Vendors: the meals bought at the nearest Market at the day's open
    /// (shelf units of synth, `synth_stretch` meals each) ...
    pub vendor_batch: u32,
    /// ... sold at the synth meal's price × this ...
    pub vendor_markup: f32,
    /// ... to an off-screen synth buyer in the district on this chance.
    pub p_vendor_pick: f32,
    /// A Pitch in a `Sweep` district is swept on this chance a day: a fine
    /// to the Treasury and closed for `sweep_close_days`; at most
    /// `sweeps_per_day` a day per 2,000 residents (scaled).
    pub p_pitch_sweep: f32,
    pub pitch_fine: i64,
    pub sweep_close_days: u8,
    pub sweeps_per_day: u16,
    /// The labour corner: the dawn pick's tick of day ...
    pub corner_hour: u16,
    /// ... from the jobless with fewer coins than this, at least this lawful ...
    pub corner_broke: i64,
    pub corner_lawful: f32,
    /// ... standing within this many tiles (a body) or living within
    /// `corner_stat_reach` (off screen, on `p_corner` a day) ...
    pub corner_tiles: u32,
    pub corner_stat_reach: u32,
    pub p_corner: f32,
    /// ... a body walks there from this many tiles at most.
    pub corner_reach: u32,
    /// Day posts: per open site a Construction Yard posts, per Farm in
    /// the harvest seasons, per Freight Yard that handled `backlog_units`
    /// yesterday.
    pub day_per_site: u8,
    pub harvest_posts: u8,
    pub harvest_seasons: Vec<crate::time::Season>,
    pub yard_posts: u8,
    pub backlog_units: u32,
    /// Couriers: parcels a day per standing building of a corp with two or
    /// more, a courier's day's parcels, and each parcel's fee.
    pub parcels_per_building: f32,
    pub parcels_per_courier: u32,
    pub parcel_fee: i64,
    /// Cabs and rickshaws (off screen, the shift's first hour): a commute of
    /// at least `cab_min_tiles` with `fare + cab_reserve` coins takes one on
    /// `p_cab`; the fare `fare_base + fare_per_tile × tiles` (a rickshaw's
    /// × `rickshaw_mult`); a driver's and a rickshaw's rides a day.
    pub cab_min_tiles: u32,
    pub cab_reserve: i64,
    /// Round 6 V4 (OPEN-240): the commute's cab (the roll, the ride home,
    /// the El-missed ride) takes a fare of at most this share of the day's
    /// wage, unless the rider holds `cab_flush_coins` (`fabber_84414`: a 7c
    /// leg each way on a 7c wage). 0 reads no wage.
    #[serde(default = "default_cab_wage_share")]
    pub cab_wage_share: f32,
    #[serde(default = "default_cab_flush_coins")]
    pub cab_flush_coins: i64,
    pub p_cab: f32,
    pub fare_base: i64,
    pub fare_per_tile: f32,
    pub rickshaw_mult: f32,
    pub rides_per_driver: u32,
    pub rides_per_rickshaw: u32,
    /// Tinkers: an agent's vehicle due a repair within `tinker_reach` of an
    /// open tinker is fixed at `tinker_discount` × the Garage's price, on
    /// `tinker_fail_p` wrecked instead; `tinker_jobs` a day each.
    pub tinker_reach: u32,
    pub tinker_discount: f32,
    pub tinker_fail_p: f32,
    pub tinker_jobs: u8,
    /// A corp's Utility Plant dark on this many days running is taken by
    /// the city (the Nationalise path; 0 never).
    pub seize_after_days: u8,
}

impl Default for Jobs2Cfg {
    fn default() -> Self {
        Jobs2Cfg {
            enabled: false,
            rations_to_markets: false,
            goods_cap: 300,
            pack_rate: 4.0,
            pack_ratio: 1.0,
            packed_premium: 1.4,
            rations_floor: 0,
            pack_fill: 0.9,
            farms_per_plant: 6,
            handling_per_unit: 0.25,
            handling_rate: 0.08,
            haul_per_unit: 0.25,
            loader_gate: true,
            p_pilfer: 0.02,
            fuel_share: 0.4,
            outage_hours: 6,
            crew_days: 3.0,
            crew_slow_max: 4.0,
            site_reach: 48,
            held_per_jailer: 25,
            clerks_max: 8,
            depot_hands_max: 6,
            jailer_short_mult: 1.5,
            packing_rows: Vec::new(),
            freight_rows: Vec::new(),
            utility_rows: Vec::new(),
            construction_rows: Vec::new(),
            hq_megacorps: false,
            hq_min_buildings: 6,
            hq_reserve: 2000,
            admin_share: 0.05,
            street: false,
            pitches_per_district: 4,
            pitch_stake: 20,
            hire_knowledge_floor: 0.05,
            pitch_after_days: 5,
            p_pitch_open: 0.05,
            pitch_fail_days: 4,
            pitch_reach: 3,
            cut_frac: 0.1,
            tinker_skill: 0.5,
            rickshaw_skill: 0.4,
            vendor_batch: 5,
            vendor_markup: 1.25,
            p_vendor_pick: 0.5,
            p_pitch_sweep: 0.1,
            pitch_fine: 5,
            sweep_close_days: 1,
            sweeps_per_day: 4,
            corner_hour: 360,
            corner_broke: 20,
            corner_lawful: 0.2,
            corner_tiles: 6,
            corner_stat_reach: 160,
            p_corner: 0.3,
            corner_reach: 40,
            day_per_site: 1,
            harvest_posts: 1,
            harvest_seasons: vec![crate::time::Season::Summer, crate::time::Season::Autumn],
            yard_posts: 2,
            backlog_units: 300,
            parcels_per_building: 0.5,
            parcels_per_courier: 12,
            parcel_fee: 1,
            cab_min_tiles: 60,
            cab_reserve: 30,
            cab_wage_share: 0.5,
            cab_flush_coins: 80,
            p_cab: 0.03,
            fare_base: 3,
            fare_per_tile: 0.05,
            rickshaw_mult: 0.6,
            rides_per_driver: 8,
            rides_per_rickshaw: 6,
            tinker_reach: 40,
            tinker_discount: 0.6,
            tinker_fail_p: 0.1,
            tinker_jobs: 3,
            seize_after_days: 5,
        }
    }
}

/// J9: a trade's shift (`demography::hire`, J12).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeShift {
    /// `[world] shift_day`.
    Day,
    /// `[leisure] evening_shift`.
    Evening,
    /// `[world] shift_night`.
    Night,
    /// Night for even ids, day for odd (the guards' rule).
    Split,
}

/// J9, J13: what pays a trade and what its shift makes (docs/JOBS_V2.md
/// § 1.2). The wage is paid by the workplace's owner at the shift's end
/// like any staff; `produce` accrues units per on-duty hour
/// (`trades::accrue`); `service` staff count as the building's on-duty
/// staff where its service reads them; the others name the flow that pays.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeModel {
    Produce,
    Service,
    Handling,
    Contract,
    Civic,
    Overhead,
    Resale,
    Day,
}

/// J9: the skill a trade's hiring ranks on and competence reads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeSkill {
    #[default]
    None,
    Farming,
    Fighting,
    Persuasion,
    Knowledge,
    Hacking,
}

/// J9: one `[[trades]]` row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeCfg {
    /// The stable key (saves store it, not the index).
    pub id: String,
    pub label: String,
    /// The kinds that employ it.
    pub workplace: Vec<BuildingKind>,
    /// Places per floor of a workplace (per group with `per_owned`).
    pub staff_per_floor: u8,
    pub wage: i64,
    pub shift: TradeShift,
    #[serde(default)]
    pub skill: TradeSkill,
    pub model: TradeModel,
    /// J14 (the Super): with `n > 0`, a corp owner posts `staff_per_floor`
    /// places per `n` of its standing buildings of the workplace kind (a
    /// group started counts), at the lowest-id building of each group of
    /// `n` (`trades::owned_places`); no other owner posts any. 0: every
    /// building posts its places per floor.
    #[serde(default)]
    pub per_owned: u16,
    /// J14: how the shift is worked (`post`, or the Super's `round`).
    #[serde(default)]
    pub duty: TradeDuty,
    /// Class pay (docs/CLASS_PAY.md): the post's grade (0 labour, 1
    /// skilled, 2 professional, 3 management), read only with class pay on.
    #[serde(default)]
    pub grade: u8,
    /// Class pay: the row exists only with class pay on
    /// (`classpay::trade_live`); off, it posts nothing anywhere.
    #[serde(default)]
    pub classpay: bool,
    /// Class pay: only a corp-owned workplace posts the row's places.
    #[serde(default)]
    pub corp_only: bool,
    /// J14 (a `round` duty): the doors a shift's round visits.
    #[serde(default)]
    pub round_stops: u8,
    /// The pre-view pass phase 2 (§ 1): this row's stations block in
    /// minutes (0: `[shift] station_block_min` / `station_block_max`).
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub station_block_min: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub station_block_max: u16,
    /// The pre-view pass phase 3 (§ 1): the row's shift shape when it is not
    /// its `duty`'s (`haul`, `counter`); the duty keeps its hiring and its
    /// Statistical path. `None` drops it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ShiftShape>,
    /// Jobs & lives phase 1 (§ 1): the row exists only with `[jobs2]
    /// enabled` (`classpay::trade_live`); off, it posts nothing anywhere.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub jobs2: bool,
    /// Jobs & lives phase 2: the row exists only with `[jobs2] street` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub street: bool,
    /// Jobs & lives phase 3: the row exists only with `[weapons]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub weapons: bool,
    /// Jobs & lives phase 4: the row exists only with `[vice]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vice: bool,
    /// Jobs & lives phase 4: the row exists only with `[fashion]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fashion: bool,
    /// Jobs & lives phase 5: the row exists only with `[vice] chip` on; its
    /// posts are filled only by the chip contract (never the hiring pass).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub chip: bool,
    /// Jobs & lives phase 5: the row exists only with `[vice] bd` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bd: bool,
    /// Jobs & lives phase 6: the row exists only with `[gray]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gray: bool,
    /// Jobs & lives phase 6: hired only from the struck-off (`[gray]` on).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub struck_off: bool,
    /// Jobs & lives phase 7: the row exists only with `[ages]` on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ages: bool,
    /// Jobs & lives phase 4: the row's on-duty staff take a visit's tip
    /// (`[vice] tip_share`) at a Vice venue or the Lounge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tipped: bool,
    /// Jobs & lives phase 4 (§ 9): the dress a hire needs (read with
    /// `[fashion]` on; 0 none).
    #[serde(default, skip_serializing_if = "is_zero_u8_cfg")]
    pub min_dress: u8,
    /// Lives at work phase 3 (§ 2.2, the super): a `round` duty's stops are
    /// the `round_stops` Blocks of the pool nearest the worker's Home (ties
    /// by id), not the id-ordered rotation. Read with `[service] civic`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub round_near: bool,
    /// Lives at work phase 3 (§ 2.2, the super): the worker and his
    /// household are housed rent-free in a Block of his employer's owner
    /// (`ownership::collect_rent`; the midnight move-in
    /// `household::tied_housing`). Read with `[service] civic`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tied_housing: bool,
    /// Lives at work phase 6 (§ 4.1): the row's hiring age band in years
    /// (0 none). Read with `[careers] enabled`; 0 (not written) off.
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub min_age: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub max_age: u32,
    /// Round 7 W4 (OPEN-245): a holder of this trade is never the foreman
    /// who picks a hire or names a layoff (the orderly under the doctor, the
    /// forger under the fixer, the door under the house).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_foreman: bool,
    /// Round 8 X3 (`OPEN-247`): the hire reads a record (`[careers]
    /// record_days`): the law's and the city's posts, the doors, the books.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reads_record: bool,
}

pub(super) fn is_zero_usize_cfg(v: &usize) -> bool {
    *v == 0
}

pub(super) fn is_zero_u8_cfg(v: &u8) -> bool {
    *v == 0
}

/// J14: how a trade works its shift.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeDuty {
    /// At the workplace for the shift (the generic `Work`).
    #[default]
    Post,
    /// The Super's round: `round_stops` of the employer's Blocks in the
    /// post's district, door to door (`trades::round_plan`), then the post.
    /// Jobs & lives phase 1: a Builder's round walks his Yard's open sites.
    Round,
    // --- Jobs & lives (§ 1): appended, worked from phase 2 (the street).
    /// Wait at a rank, carry a rider (cabs, rickshaws).
    Fare,
    /// The dawn pick at the labour corner.
    Corner,
    /// Preach at a spot (preachers).
    Preach,
    /// Sell at a Pitch (vendors).
    Sell,
}

/// J9: the `trades` field's reader: the rows, checked (unique keys, at most
/// 255), and their ids made the keys a save on this thread resolves against.
pub(super) fn trades_de<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<TradeCfg>, D::Error> {
    let rows = Vec::<TradeCfg>::deserialize(d)?;
    if rows.len() > usize::from(u8::MAX) {
        return Err(serde::de::Error::custom("[[trades]]: at most 255 rows"));
    }
    for (i, r) in rows.iter().enumerate() {
        if rows[..i].iter().any(|o| o.id == r.id) {
            return Err(serde::de::Error::custom(format!("[[trades]]: duplicate id {:?}", r.id)));
        }
    }
    crate::components::TradeId::set_keys(trade_ids_of(&rows));
    Ok(rows)
}

pub(super) fn trade_ids_of(rows: &[TradeCfg]) -> Vec<crate::components::TradeId> {
    rows.iter().enumerate().map(|(i, r)| crate::components::TradeId::new(i as u8, &r.id, &r.label)).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobsCfg {
    pub farmer: u32,
    pub guard: u32,
    pub clerk: u32,
    pub bartender: u32,
    pub gravedigger: u32,
    /// M12 D23: hired by `levers.sanitation_count`, not seeded.
    #[serde(default)]
    pub sanitation: u32,
}

impl JobsCfg {
    pub fn count(&self, role: Role) -> u32 {
        match role {
            Role::Farmer => self.farmer,
            Role::Guard => self.guard,
            Role::Clerk => self.clerk,
            Role::Bartender => self.bartender,
            Role::Gravedigger => self.gravedigger,
            Role::Sanitation => self.sanitation,
            // M13 D16: hired by the Clinics and Garages that are built, never seeded.
            // M14: likewise the Labs' Researchers.
            // M15 W36: and the Feeds' Reporters.
            Role::Ripperdoc | Role::Mechanic | Role::Researcher | Role::Reporter => 0,
            // L2 (plan L2): the venues' and Fabs' staff are hired, never seeded.
            Role::Host
            | Role::Attendant
            | Role::Cook
            | Role::Fighter
            | Role::Croupier
            | Role::Concierge
            | Role::Fabber => 0,
            // M16a (plan C8): a Fixer office's staff are hired, never seeded.
            Role::Fixer => 0,
            // Real economy E26: a Mission's Volunteers are hired, never seeded.
            Role::Volunteer => 0,
            Role::Trade(_) => 0,
        }
    }
}

/// M15 § 6: competence and poaching (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompetenceCfg {
    pub exec_w: f32,
    pub comp_w: f32,
    pub comp_ref: f32,
    pub comp_min: f32,
    pub comp_max: f32,
    pub talent_drop: f32,
    pub poach_min: f32,
    pub poach_gap: f32,
    pub poach_premium: f32,
    /// Phase 2 review: a skill's competence term is `2 ×` its percentile
    /// among the city's adults at seed (the median corp at multiplier 1);
    /// false: the plan's `skill ÷ city mean` (W28; the median corp near 0.85
    /// of `comp_ref`, the exec term being a heavy-tailed skill).
    pub rank_norm: bool,
    /// Jobs & lives phase 1 (§ 2, the Hq): the admin term's weight in a
    /// corp's competence (`[jobs2] enabled` only) ...
    pub admin_w: f32,
    /// ... and `admin = clamp(Σ office staff skill ÷ admin_ref, 0, 1)`.
    pub admin_ref: f32,
}

impl Default for CompetenceCfg {
    fn default() -> Self {
        CompetenceCfg {
            exec_w: 0.4,
            comp_w: 1.0,
            comp_ref: 0.25,
            comp_min: 0.75,
            comp_max: 1.35,
            talent_drop: 0.05,
            poach_min: 0.5,
            poach_gap: 0.2,
            poach_premium: 1.3,
            rank_norm: true,
            admin_w: 0.1,
            admin_ref: 10.0,
        }
    }
}

/// `[economy2] labour_share` (spec § 5, plan E17): the share of a niche's
/// revenue its payroll targets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabourShareCfg {
    pub food: f32,
    pub housing: f32,
    pub security: f32,
    pub tech: f32,
}

impl Default for LabourShareCfg {
    fn default() -> Self {
        LabourShareCfg { food: 0.6, housing: 0.4, security: 0.7, tech: 0.55 }
    }
}

impl LabourShareCfg {
    pub fn of(&self, niche: crate::components::Niche) -> f32 {
        use crate::components::Niche;
        match niche {
            Niche::Food => self.food,
            Niche::Housing => self.housing,
            Niche::Security => self.security,
            Niche::Tech => self.tech,
            // Jobs & lives: the Vice venues (phase 4) pay as the Food trade's
            // counters until they have their own share.
            Niche::Vice => self.food,
        }
    }
}

/// `[classpay]` (docs/CLASS_PAY.md; placeholders, not tuned). Every key is
/// read only with `classpay::on` (the market wage law and
/// `enabled`): the dole city is the city before it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassPayCfg {
    pub enabled: bool,
    /// A post's wage multiple by grade: labour, skilled, professional, management.
    pub grade_mult: [f32; 4],
    /// The bespoke roles' base grade by `{role:?}` (others labour).
    pub role_grade: std::collections::BTreeMap<String, u8>,
    /// Of a corp building's labour staff in a role, the skilled posts (floor).
    pub skilled_share: f32,
    /// Days at the corp before a promotion.
    pub promote_days: u32,
    pub merit_edu: f32,
    pub merit_skill: f32,
    pub merit_tenure: f32,
    /// Tenure at the same corp counts in full at this many days.
    pub tenure_full_days: u32,
    /// Merit lost per tile of commute (Home door to workplace door).
    pub commute_w: f32,
    /// Education's mean by origin: Dreg, Sump, Mid, Spire.
    pub education: [f32; 4],
    /// The seeded and immigrant spread (+- uniform, a pure hash).
    pub education_spread: f32,
    /// A child's spread around its parents' mean.
    pub child_spread: f32,
    /// P3, the thin ladder: education gained a day in a skilled or graded post.
    pub edu_gain_per_day: f32,
    /// P3: the night school's cap.
    pub edu_cap: f32,
    /// P3, the status sink: coins an adult keeps before spending on status.
    pub status_floor: i64,
    /// P3: the share of the excess over `status_floor` spent a day.
    pub status_frac: f32,
    /// P3: the split: a Spire night out, fashion (a corp Market), cosmetic chrome (a corp Clinic).
    pub status_split: [f32; 3],
    /// P3 (Dylan): the most of the capital payout the graded premium may
    /// draw on; the rest always goes into labour's P*.
    pub premium_payout_share: f32,
}

impl Default for ClassPayCfg {
    fn default() -> Self {
        ClassPayCfg {
            enabled: false,
            grade_mult: [1.0, 1.3, 2.0, 3.0],
            role_grade: std::collections::BTreeMap::new(),
            skilled_share: 0.25,
            promote_days: 30,
            merit_edu: 0.5,
            merit_skill: 0.3,
            merit_tenure: 0.2,
            tenure_full_days: 180,
            commute_w: 0.0005,
            education: [0.10, 0.25, 0.50, 0.80],
            education_spread: 0.20,
            child_spread: 0.10,
            edu_gain_per_day: 0.0003,
            edu_cap: 0.75,
            status_floor: 60,
            status_frac: 0.05,
            status_split: [0.5, 0.25, 0.25],
            premium_payout_share: 0.5,
        }
    }
}

fn default_cab_wage_share() -> f32 {
    0.5
}

fn default_cab_flush_coins() -> i64 {
    80
}
