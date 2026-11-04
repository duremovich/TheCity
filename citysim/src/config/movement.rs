use super::*;

/// `[routes]` (the island fix round F1, OPEN-188): when a trip's route is
/// longer than walking, walk; and a label is not space (the Spire is one
/// district 150-350 tiles across). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutesCfg {
    /// The exec's car (`streetwork::exec_ride`, the ride home and its bed
    /// estimate) drives a trip of this many Manhattan tiles or more, home
    /// to Hq or from where he stands to his Home; the base drove only
    /// between two district labels.
    pub exec_ride_min_tiles: u32,
    /// The El (`transit::route`, `commute_ticks`) is not boarded at a stop
    /// whose walk from the trip's start is past this, nor left at one whose
    /// walk to its end is; 0, no cap.
    pub access_cap_ticks: u64,
    /// A stop pair is no route when the alight stop lies on the walk to
    /// the board stop (within this many tiles of a shortest Manhattan way):
    /// the rider would walk past his stop to ride back to it.
    pub pass_slack_tiles: u32,
}

/// The city milestone C7 `[levels]`: the climb inside a structure and on the
/// walkways. Ticks are game minutes, fractions allowed (a climb rounds up to
/// whole ticks, one at least for any storey). Every value a placeholder at
/// the real pace (`[exec] walk_tiles_per_tick`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelsCfg {
    /// Minutes a storey climbed up by the stairs.
    pub stair_ticks_per_floor: f32,
    /// A storey down by the stairs is this share of one up.
    pub stair_down_mult: f32,
    /// Minutes for a car to come to an empty lobby (the call, the doors).
    pub lift_wait_ticks: f32,
    /// Minutes a storey ridden (the ride, its stops spread over it).
    pub lift_ticks_per_floor: f32,
    /// Minutes the car stands at each end of its round trip: a car's cycle
    /// is up to the lift's top floor and back plus twice this.
    pub lift_dwell_ticks: f32,
    /// Bodies a car holds.
    pub lift_capacity: u16,
    /// A climb of more storeys than this takes the lift, when the host has
    /// one, it is not cut, and its wait and ride beat the stairs.
    pub lift_min_floors: u8,
    /// Minutes a storey of a walkway portal's stairs (a footbridge's, a
    /// station's, a landing into a lot) costs over its run, up or down alike
    /// (the path search's costs are symmetric).
    pub portal_stair_ticks_per_floor: f32,
    /// A station lift portal: the call, then `lift_ticks_per_floor` a storey;
    /// no queue (the station's own lifts).
    pub portal_lift_wait_ticks: f32,
    /// The escalator walked is this many times the walk on the street (the
    /// stair moves under the walker; the sim's ground is flat, so its lift
    /// up the hill is not the advantage, its speed is).
    pub escalator_speed: f32,
}

/// The view milestone V7 `[crossings]`: the junctions' signals and the
/// price of crossing a street on foot. Every value a placeholder at the real
/// pace (`[exec] walk_tiles_per_tick`; a tick a game minute).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossingsCfg {
    /// Seconds of one signal cycle at every junction (the walk phase first,
    /// each junction's cycle offset by its tile: `signals::offset_ms`).
    pub cycle_s: f32,
    /// The share of the cycle the walkers cross on (the walk phase; the
    /// rest don't-walk, the main road's traffic moving).
    pub walk_share: f32,
    /// Tenths of a step a walker's path pays to step onto a carriageway tile
    /// off every crossing (a Road step is 7): near-forbidden, never a wall,
    /// so a street without a crossing nearby is still crossed.
    pub jaywalk_tenths: u32,
    /// A timed (Coarse) walk on foot crosses this many signalled crossings a
    /// Manhattan tile, each at the expected wait (`signals::timed_wait_ticks`;
    /// the Full walkers' measured rate is the run report's
    /// `crossings_walked / walk_steps`).
    pub timed_crossings_per_tile: f32,
}

/// The city milestone C8 `[vertical]`: the vertical gangs. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerticalCfg {
    /// At the world's build a gang's seat (its first Hideout, the map's) moves
    /// up to the top storey of its host (the flats under it keep their
    /// storeys less that one): the upper floors over the shop.
    pub seat_up: bool,
    /// Hideouts a gang founds a day at most, one per district it holds turf
    /// in (a held cell) and has none.
    pub found_per_day: u8,
    /// The key money a district Hideout costs the gang's treasury, paid to
    /// the landlord (`Flow::Sale`); a gang holding less moves in on what it
    /// has (nobody evicts a gang).
    pub found_cost: i64,
    /// A district Hideout is founded only in a host of at least this many
    /// storeys (the upper floors over a front), else any host with a vacant
    /// unit in the district.
    pub min_storeys: u8,
    /// A gang holds the lift of a host its Hideout stands in while it holds
    /// the cell of the host's door: its members ride first, anyone else
    /// waits `held_wait_ticks` more in the lobby (or takes the stairs).
    pub hold_lifts: bool,
    pub held_wait_ticks: f32,
    /// A raid won or a Hideout sacked cuts the lift of the loser's host for
    /// this many days (the cables cut on the way out); 0: never.
    pub raid_cut_days: u16,
    /// A landlord short on a host's power cuts its lift for this many days
    /// (the power off to the cars); 0: never.
    pub power_cut_days: u16,
    /// A raid's column takes the lift up to its target's floor only with this
    /// many raiders or fewer (the lobby is the choke), the lift running and
    /// not held by the target; else the stairs.
    pub lift_max_raiders: u16,
    /// Tenants on the floors a raid's column climbs past who hear it (the
    /// noise is word), at most this many a raid.
    pub hear_cap: u16,
    /// Vertical turf: a gang's claim that would take a Home another gang
    /// holds, on a Home of this many storeys or more, takes its upper half
    /// instead (a Home tenancy carved from the span, its households with it):
    /// one tower holds two gangs' floors. 0: the whole Home, as before.
    pub split_min_storeys: u8,
    /// V8 (`OPEN-264`): a raider climbs the stairs at his own pace: past this
    /// age each year adds `stair_age_slow` of the stair time (0: every body
    /// at `[levels] stair_ticks_per_floor`).
    #[serde(default)]
    pub stair_age_from: u8,
    #[serde(default)]
    pub stair_age_slow: f32,
    /// V8: a district Hideout founded on a floor is walked into by this many
    /// of the members it is now nearest (free adults, nearest first), at
    /// `found_hour` (a Visit, the climb theirs); 0: the founding is a line.
    #[serde(default)]
    pub found_walkers: u8,
    #[serde(default)]
    pub found_hour: u8,
    /// C9b: the raid on the rival's nearest Hideout, a standing guard at the
    /// seat (absent: every raid on the seat, no guard).
    #[serde(default)]
    pub raids: Option<VerticalRaidsCfg>,
    /// C9b: the landlord's answer to a gang that will not pay its floor rent
    /// (absent: the arrears stand and nothing follows).
    #[serde(default)]
    pub landlord: Option<LandlordCfg>,
}

/// C9b `[vertical.raids]`: where a raid lands. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerticalRaidsCfg {
    /// The seat's standing guard: this many of tonight's watch (the members
    /// living within a walk of the seat) posted to it at `[crew] guard_hour`
    /// under any order but LieLow, kept out of a raid's muster.
    pub seat_guard: u8,
    /// A raid's target is the rival's Hideout nearest the raiders' own; the
    /// seat (the chest, the stash) when it is the nearest, or when the
    /// raiders' fit adults are this many times the rival's.
    pub seat_ratio: f32,
    /// A district Hideout won or sacked gives up this share of the rival's
    /// chest (the seat's: `[gangs] raid_prize_frac` won, all of it sacked).
    pub take_frac: f32,
}

/// The island pass phase 5 `[motion]` (docs/ISLAND_PASS.md I3): a vehicle
/// in motion advances along its way with its riders aboard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionCfg {
    /// A timed road vehicle's way follows the road path (the walk's A*);
    /// false: an L, along x then y (no search).
    pub road_route: bool,
    /// The island pass phase 6 `[motion.trains]` (OPEN-165): the El runs as
    /// trains on its timetable (`transit.rs` § Trains). `None` (not written
    /// into a save) with the key absent: the timed ride as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trains: Option<TrainsCfg>,
    /// The island pass phase 7 `[motion.vans]` (OPEN-166): the escort and
    /// the prisoner transfer ride a van (`stations.rs` § the van). `None`
    /// (not written into a save) with the key absent: the timed escort
    /// and the bodiless 04:00 transfer as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vans: Option<VansCfg>,
    /// The island pass phase 9 `[motion.walks]` (OPEN-181): a Coarse timed
    /// walk keeps its body on its way (`exec::walk_place`), and a promotion
    /// or an interrupt mid-walk sets it down there. `None` (not written into
    /// a save) with the key absent: the body stands at the
    /// walk's start until it arrives, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walks: Option<WalksCfg>,
}

/// `[motion.walks]`: a Coarse body's timed walk on foot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalksCfg {
    /// The walk's way follows the road path (the Full walk's A*); false: an
    /// L, along x then y (no search).
    pub road_route: bool,
    /// A walk cut short (its plan dropped or replaced) leaves the body where
    /// it stands on its way; false: back at the walk's start.
    pub settle_interrupt: bool,
}

/// `[motion.vans]`: the law's vans. The drive is at the Car's speed
/// (`[vehicles] mult`); the escort's walk-or-van distance and the van's call
/// are `[legs]`'s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VansCfg {
    /// Prisoners one transfer van carries; a Station holding more due sends
    /// another van.
    pub capacity: u32,
}

/// `[motion.trains]`: the El's trains. The headway, the dwell, the speed and
/// the transfer walk are `[transit]`'s; the train count is derived (each
/// line's round trip over the headway).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainsCfg {
    /// Riders a train carries; a full train leaves the rest on the platform
    /// for the next one.
    pub capacity: u32,
}

/// The island pass phase 4 `[commute_pressure]` (docs/ISLAND_PASS.md I2):
/// a worker whose commute (`distance::travel`, the Home door to the work
/// tile) is past `far_ticks` builds a day of pressure each midnight; past
/// `move_days` the household looks for a Home nearer work (any it can afford
/// within `[distance] home_cap_ticks`, the corp's company beds included),
/// and past `job_days` with none found, the worker a nearer post of the same
/// role. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommutePressureCfg {
    /// A commute past this travel builds a day of pressure.
    pub far_ticks: u64,
    /// Days of pressure before the household looks for a nearer Home.
    pub move_days: u16,
    /// Days of pressure before, with no Home found, the worker looks for a
    /// nearer post of the same role.
    pub job_days: u16,
    /// A search that found nothing waits this many days for the next.
    pub retry_days: u16,
    /// After a move or a job switch, no pressure builds for this many days.
    pub cool_days: u16,
    /// A nearer post saves at least this much travel from the Home door.
    pub job_margin_ticks: u64,
    /// A worker hired since the last midnight with a far Home looks at once
    /// (phase 3a's move-in of a far hire, on this one path).
    pub hire_now: bool,
}

/// The island pass phase 1 `[distance]` (docs/ISLAND_PASS.md I1): each
/// pick's hard cap on the travel estimate (`distance::travel`, ticks), past
/// which a candidate is dropped while one within it is left. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistanceCfg {
    /// The job: a candidate whose Home door (else tile) is past this from
    /// the workplace door.
    pub job_cap_ticks: u64,
    /// The bed a worker moves into: a Home whose door is past this from the
    /// workplace door.
    pub home_cap_ticks: u64,
    /// Tonight's bed (the Home or the gang's Hideout): past this from where
    /// the body stands.
    pub bed_cap_ticks: u64,
    /// The own Home's offer over a Hideout cot, in travel ticks: the Hideout
    /// is tonight's bed only when nearer by more than this.
    pub home_bed_ticks: u64,
    /// The island pass phase 2 (OPEN-163, OPEN-171): the free-time spot and
    /// the Hotel. `None` (not written into a save) with
    /// the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free: Option<FreeDistanceCfg>,
    /// The pressure pass phase 4 (OPEN-194): the reach of the non-urgent
    /// trips. `None` (not written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<ReachCfg>,
    /// Pressure pass round 3, S1 (OPEN-221): distance in the goals' place
    /// picks. `None` (not written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<ChoiceCfg>,
}

/// Pressure pass round 3, S1 `[distance.choice]` (docs/PRESSURE_PASS.md;
/// OPEN-221): the place a goal picks is picked by reach, on the body's own
/// trip (`distance::trip`: the El only for a body that would board it), and
/// a trip past its cap is not set out on: the nearer option, else another
/// goal, and the diary says so. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceCfg {
    /// A dealer's site today: the gang's spot (venue or corner) of each
    /// district it holds, nearest first, from his Home door (by the Hideout
    /// when he must pick up first); none within: no Deal today.
    pub deal_ticks: u64,
    /// A gang member's order work (`gang::gang_work_target`: the Home to
    /// shake down or claim, the derelict to squat): past this from where he
    /// stands he does not set out.
    pub gang_work_ticks: u64,
    /// The hire: a candidate whose Home door is past this from the
    /// workplace door is not hired even with nobody nearer (the vacancy
    /// waits); `[distance] job_cap_ticks` stays the soft cap.
    pub hire_max_ticks: u64,
    /// A Builder's (the day labourer's) far site day (`shift::far_site`):
    /// the open site past this from the Yard's door is not walked to (the
    /// shift is worked at the Yard).
    pub site_ticks: u64,
    /// A Visit (a Camp child, a hurt kin, an elder parent, an errand; not a
    /// gang duty): the place past this from where the visitor sets out is
    /// not walked to.
    pub visit_ticks: u64,
    /// C9a: a stims source (`stims::stim_source`, the nearest Bar with a
    /// dealer, else a legal Market): past this from where the body stands
    /// (his own trip) it is no source, and the craving waits for a nearer
    /// one or the corner. 0: any distance.
    #[serde(default)]
    pub stim_ticks: u64,
}

/// The pressure pass phase 4 `[distance.reach]` (docs/PRESSURE_PASS.md;
/// OPEN-194): how far, in travel ticks (`distance::travel`), a non-urgent
/// trip is walked. Past it the trip is not made (a nearer alternative where
/// the purpose has one). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReachCfg {
    /// ReportCrime: the Hall's door from where the witness sets out.
    pub report_ticks: u64,
    /// Found (Register): the Hall's door from where the founder sets out.
    pub register_ticks: u64,
    /// Treat (therapy, detox; never a diagnosed cancer): the nearest Clinic.
    pub clinic_ticks: u64,
    /// A Hunt's or a hit Contract's first `GoTo(Intel)`: the ask venue (past
    /// it, the hunter's own district's busiest Bar) or the stakeout.
    pub stakeout_ticks: u64,
    /// A sweeper's beat: the district's centroid from the post's door (past
    /// it, the nearest district with a slot left, else the post's own).
    pub beat_ticks: u64,
    /// The pressure pass R2 (`OPEN-215`): a city guard's patrol loop
    /// (`law::district_route`): its Market within this of the guard (none:
    /// the district's Homes nearest him), its Bar and the Hall within this
    /// of the Market. Absent: `[distance] job_cap_ticks`, the Bar anywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patrol_ticks: Option<u64>,
}

/// The island pass phase 2 `[distance.free]` (docs/ISLAND_PASS.md I1):
/// the caps of the free-time spot and the night's bed out (a Hotel or a
/// Mission's cot). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FreeDistanceCfg {
    /// The HangOut spot (a street corner, a Hideout's door, a friend's
    /// Home): past this from where the body sets out it is dropped, and with
    /// none within, no spot (the own Home is never past it).
    pub spot_cap_ticks: u64,
    /// Tonight's paid or charity bed (`street::hotel_for`): past this from
    /// where the body sets out it is dropped while one within it is left.
    pub hotel_cap_ticks: u64,
    /// A Mission's cot's offer over a paid Hotel bed, in travel ticks (it
    /// is free): the cot unless a Hotel is nearer by more than this.
    pub cot_bed_ticks: u64,
    /// Island fix round F3 (OPEN-190): the stay-in fallback. `None` (not
    /// written into a save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stay_in: Option<StayInCfg>,
}

/// Pressure pass round 2, R1 `[fares]` (docs/PRESSURE_PASS.md § R1;
/// `systems/fares.rs`): the fare dodger's ladder. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FaresCfg {
    /// The fine on the platform when the wallet holds it (to the city).
    pub fine: i64,
    /// A dodger who cannot pay is beaten by the guard who caught him: down
    /// on the platform this many minutes (he neither thinks nor walks) ...
    pub beat_down_minutes: u64,
    /// ... with this grudge on the guard's faction (the Law, or a private
    /// guard's corp; round 9 Y2: never the guard) ...
    pub beat_grudge: f32,
    /// ... and this off his mood and his safety.
    pub beat_mood: f32,
    pub beat_safety: f32,
    /// Caught `ban_after` times inside `ban_window_days`: banned from the El
    /// for `ban_days`, and he walks.
    pub ban_after: u8,
    pub ban_window_days: u64,
    pub ban_days: u64,
    /// A banned rider under this lawfulness boards anyway (the ban broken):
    /// caught, he is beaten whatever his wallet holds, and the ban starts again.
    pub ban_break_lawfulness: f32,
    /// Caught this many hours ago or less, he jumps no gate: he pays the
    /// fare or walks (a beaten dodger does not try the same gate again
    /// within the hour).
    pub wary_hours: u64,
    /// Pressure pass round 3, S3: beaten this many days ago or less, a
    /// caught dodger is not beaten again: the guard turns him off the
    /// platform and he walks (0: every catch past the fine is a beating).
    pub beat_cooldown_days: u64,
    /// Pressure pass round 7, W3 (`OPEN-244`): a guard handles one dodger at
    /// a time: a catch holds him this many minutes (the fine counted, the
    /// beating or the booking), and a gate jumped in front of him meanwhile
    /// goes through (0: every dodger he sees is caught at once; a guard beat
    /// 18 in 11 minutes).
    pub handle_minutes: u64,
    /// Round 7 W3: a broke dodger this old or more (or sick, or wounded) is
    /// booked, not beaten: the catch on his record toward the ban, no blow
    /// (0: the old are beaten like anyone).
    pub book_age: u32,
}

impl Default for FaresCfg {
    fn default() -> Self {
        FaresCfg {
            fine: 5,
            beat_down_minutes: 30,
            beat_grudge: 0.4,
            beat_mood: 0.3,
            beat_safety: 0.4,
            ban_after: 3,
            ban_window_days: 7,
            ban_days: 14,
            ban_break_lawfulness: 0.1,
            wary_hours: 24,
            beat_cooldown_days: 3,
            handle_minutes: 6,
            book_age: 60,
        }
    }
}

/// Transit (docs/TRANSIT.md § 2): the El's lines, timetable and the walk-or-
/// ride rule. Read only while `enabled`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransitCfg {
    pub enabled: bool,
    /// Coins per ride, transfers free (Dylan 2026-10-10: a flat 1).
    pub fare: i64,
    /// Ticks between departures from each terminus.
    pub headway: u32,
    pub ride_ticks_per_tile: f32,
    /// Ticks per stop passed. The view milestone V9: fractional, summed
    /// with the stretches' ride before the offset is rounded up.
    pub dwell_ticks: f32,
    /// Ticks per transfer (the wait at a transfer, folded into the table).
    pub transfer_ticks: u32,
    /// Track tiles between stops.
    pub stop_spacing: u32,
    /// The nearest stops tried at each end of a trip.
    pub stop_candidates: usize,
    /// No ride under this Manhattan distance.
    pub min_ride_tiles: u32,
    pub min_saving_ticks: u32,
    /// The view milestone V9 (OPEN-255): a minute on foot or on the
    /// platform weighs this many minutes aboard in the walk-or-ride choice
    /// (`transit::felt`). 1.0: a minute is a minute.
    pub foot_weight: f32,
    /// What a coin of fare must save in ticks ...
    pub ticks_per_coin: u32,
    /// ... and for a wallet under `poor_meals` meals.
    pub poor_ticks_per_coin: u32,
    pub poor_meals: i64,
    /// A rider who can't pay jumps the gate below this lawfulness ...
    pub dodge_lawfulness: f32,
    /// ... and never pays below this.
    pub dodge_always: f32,
    /// Phase 3: a rider arrested in the last this many days pays the fare
    /// when it can, whatever its lawfulness (the deterrent a night in the
    /// cells is; a broke rider still jumps).
    pub dodge_deterred_days: u32,
    /// Service hours, tick of day: from `service_from` to `service_to`
    /// (wrapping midnight).
    pub service_from: u16,
    pub service_to: u16,
    /// Night, for the danger test (wrapping midnight).
    pub night_from: u16,
    pub night_to: u16,
    /// `crime_rate` at a stop's district above which an average-courage
    /// rider (courage 0.5) refuses at night; the bar is
    /// `danger_crime × (0.5 + courage)`.
    pub danger_crime: f32,
    /// Coins per open line per day, the Treasury to the World (power).
    pub power_per_line_day: i64,
    /// The El's maintenance crews per open line: the cap on
    /// `transit::staff_target` (the fares past the power still bound it
    /// from above: no fares, no crews). A real metro staffs 0.2-0.8 % of
    /// its population; 4 a line on the shipped 3 lines is 12 posts (0.6 %
    /// of 2,000). Uncapped, the fares paid ~250 crews on seed 42. Missing
    /// (an old save's config, from before the cap): `u16::MAX`, uncapped,
    /// the behaviour it was saved under (review finding 5); the shipped
    /// config.toml sets 4.
    pub staff_per_line: u16,
    /// A fare dodger's fine when the Precinct is full (`Flow::Fine`).
    pub dodge_fine: i64,
    pub lines: Vec<TransitLineCfg>,
}

/// One `[[transit.lines]]` row: a name and its waypoints (road tiles, the
/// track runs straight between consecutive ones).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransitLineCfg {
    pub name: String,
    pub waypoints: Vec<[u16; 2]>,
    /// The pre-view pass phase 5: a line in flight (a map's `L+` line),
    /// dropped with `[legs]` off (`Map::apply_to`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_flight: bool,
    /// The view milestone V9 (OPEN-255): the waypoints (indices) that are
    /// stations, the line's stops; the rest are bends the train runs
    /// through. Empty: every waypoint a stop, and a stop every
    /// `stop_spacing` track tiles (the config's lines, a map without
    /// stations).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<u16>,
}

impl Default for TransitCfg {
    fn default() -> Self {
        TransitCfg {
            enabled: false,
            fare: 1,
            headway: 10,
            ride_ticks_per_tile: 0.25,
            dwell_ticks: 1.0,
            transfer_ticks: 5,
            stop_spacing: 16,
            stop_candidates: 3,
            min_ride_tiles: 24,
            min_saving_ticks: 15,
            foot_weight: 1.0,
            ticks_per_coin: 20,
            poor_ticks_per_coin: 60,
            poor_meals: 2,
            dodge_lawfulness: 0.25,
            dodge_always: 0.1,
            dodge_deterred_days: 7,
            service_from: 300,
            service_to: 60,
            night_from: 1260,
            night_to: 300,
            danger_crime: 3.0,
            power_per_line_day: 20,
            staff_per_line: u16::MAX,
            dodge_fine: 5,
            lines: Vec::new(),
        }
    }
}

impl TransitCfg {
    /// Not written into a save while off.
    pub fn is_off(&self) -> bool {
        !self.enabled
    }
}

/// M13 § 2 vehicles (plan phase 2, D19-D27). Read only while `[assets]
/// enabled`; `off()` carries the spec's values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VehiclesCfg {
    /// The road share a Coarse trip's `timed_mult` assumes (D21).
    pub road_share: f32,
    /// A Full driver's steps per executor call (D19).
    pub max_steps_per_tick: u16,
    /// A flyer's ticks per Chebyshev tile (D22).
    pub flyer_ticks_per_tile: f32,
    /// Step multipliers `[road, ground, farmland]` per road vehicle (a Door
    /// reads ground; 1.0 is walking pace).
    pub mult: PerKind<Vec<f32>>,
    /// `HaulToMarket` batch multiplier per vehicle (D23).
    pub haul: PerKind<u32>,
    pub crash_per_tile: f32,
    pub speed: PerKind<f32>,
    pub chase_mult: f32,
    pub crash_radius: u32,
    pub dodge_w: f32,
    pub dodge_cap: f32,
    pub p_crash_kill: f32,
    pub crash_wear: u8,
    pub crash_litter: u8,
    pub steal_vehicle_cost: f32,
    pub fence_frac: f32,
    pub vehicle_theft_base: f32,
    pub garage_mult: f32,
    pub bikes_max_share: f32,
    /// Plan D26: a thief binds a street-parked vehicle within this many tiles.
    pub steal_reach: u32,
}

impl Default for VehiclesCfg {
    fn default() -> Self {
        VehiclesCfg::off()
    }
}

impl VehiclesCfg {
    /// The spec's values (assets off reads none of them).
    pub fn off() -> VehiclesCfg {
        VehiclesCfg {
            road_share: 0.7,
            max_steps_per_tick: 4,
            flyer_ticks_per_tile: 0.3,
            mult: PerKind {
                motorcycle: vec![0.25, 0.6, 0.8],
                car: vec![0.2, 1.0, 1.0],
                truck: vec![0.3, 1.0, 0.6],
                ..PerKind::default()
            },
            haul: PerKind { motorcycle: 1, car: 2, truck: 6, flyer: 1, ..PerKind::default() },
            crash_per_tile: 0.00004,
            speed: PerKind { motorcycle: 1.5, car: 1.0, truck: 1.2, flyer: 0.0, ..PerKind::default() },
            chase_mult: 6.0,
            crash_radius: 2,
            dodge_w: 0.6,
            dodge_cap: 0.9,
            p_crash_kill: 0.3,
            crash_wear: 30,
            crash_litter: 24,
            steal_vehicle_cost: 10.0,
            fence_frac: 0.2,
            vehicle_theft_base: 0.004,
            garage_mult: 0.2,
            bikes_max_share: 0.5,
            steal_reach: 40,
        }
    }

    /// `mult[kind][column]` (0 road, 1 ground, 2 farmland); 1.0 when absent.
    pub fn mult_of(&self, kind: AssetKind, column: usize) -> f32 {
        self.mult.get(kind).get(column).copied().unwrap_or(1.0)
    }
}
