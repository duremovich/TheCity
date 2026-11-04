use super::*;

/// `[view]` (the city milestone C2): the view's snapshot and its rate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewCfg {
    /// A tile's side in metres on the island maps (2 m x `k`, `k` = 2).
    pub tile_m: f64,
    /// The city's north edge in metres (the import's y1: terrain.json `size_m`).
    pub north_m: f64,
    /// The human walk a walk-matched view shows, m/s (`view::walk_matched_rate`).
    pub walk_m_s: f64,
    /// The camera ring's half side in tiles.
    pub ring_half_tiles: u16,
    /// A Coarse walk is placed on its way (one road search) only when its
    /// start-to-target box grown by this many tiles meets the ring.
    pub way_margin_tiles: u16,
    /// The view milestone V1 (OPEN-262): real seconds between two looks at the config file's mtime (cityview's
    /// reload on a file change).
    pub reload_poll_s: f64,
    /// The view milestone V3 (cityview `people.rs`): a body within this many
    /// metres of the subject is drawn skinned and animated (walk / idle);
    /// past it, a static posed mesh.
    pub people_anim_m: f32,
    /// The most bodies drawn skinned and animated at once (the nearest).
    pub people_anim_max: u16,
    /// The view milestone V4b (cityview `cam.rs`): the orbit follow-cam. Its
    /// vertical FOV, the starting yaw (the camera's bearing from the subject,
    /// 0 = south) and pitch, the pitch's clamp, the zoom's range (the frame's
    /// width at the subject, metres), the aim's ease (half-life, the snap
    /// distance, the aim's height over the feet), the controls' speeds and
    /// the far plane (the stream's reach too).
    pub cam_fov_deg: f32,
    pub cam_yaw_deg: f32,
    pub cam_pitch_deg: f32,
    pub cam_pitch_min_deg: f32,
    pub cam_pitch_max_deg: f32,
    pub cam_zoom_min_m: f32,
    pub cam_zoom_max_m: f32,
    pub cam_follow_half_life_s: f32,
    pub cam_snap_m: f32,
    pub cam_look_up_m: f32,
    pub cam_rotate_deg_s: f32,
    pub cam_pitch_deg_s: f32,
    pub cam_drag_deg_px: f32,
    pub cam_zoom_step: f32,
    pub cam_far_m: f32,
    /// V10: the light follows the sim's clock: day from `light_dawn_h` to
    /// `light_dusk_h`, night between, each change blended over
    /// `light_twilight_h` hours centred on its hour (cityview `cam.rs`; L
    /// overrides: day, night, the clock).
    pub light_dawn_h: f32,
    pub light_dusk_h: f32,
    pub light_twilight_h: f32,
    /// The day: the sky's ambient (cd/m^2-ish, Bevy's brightness), the sun
    /// (lux) and the exposure (EV100).
    pub light_day_sky: f32,
    pub light_day_sun: f32,
    pub light_day_ev100: f32,
    /// The night with the street lamps loaded: the sky glow's ambient, the
    /// moon (lux) and the exposure (EV100); `light_bare_*`: without lamps (an
    /// old asset tree, the overview), brighter so the silhouettes read.
    pub light_night_sky: f32,
    pub light_night_moon: f32,
    pub light_night_ev100: f32,
    pub light_bare_sky: f32,
    pub light_bare_moon: f32,
    /// V4b (glclient `occlusion.rs`): the cutaway. A box on the camera's
    /// side of the subject: its width across the camera-to-subject line, how
    /// far behind the feet it starts, the height over the feet above which
    /// it cuts, the dither band at its edges, the cut edge's glow, and the
    /// widest zoom it runs at (an establishing shot keeps the city whole).
    pub cut_width_m: f32,
    pub cut_behind_m: f32,
    pub cut_above_m: f32,
    pub cut_band_m: f32,
    pub cut_rim: f32,
    pub cut_max_zoom_m: f32,
    /// V4b: the subject indoors, its own building is cut this far over its
    /// feet (its ceiling: the room shows).
    pub cut_ceiling_m: f32,
    /// A child's drawn height over an adult's at age 0; it grows linearly to
    /// 1 at `child_full_age`.
    pub child_scale_min: f32,
    pub child_full_age: u32,
    /// The people's atlas as emission, this strong (the night street: an
    /// unlit body is lost in the dark).
    pub people_glow: f32,
    /// The view milestone V4 (cityview `sim.rs`): an El train drawn as
    /// `el_cars` cars of `el_car_m` (gap `el_car_gap_m`), `el_car_w_m` wide
    /// and `el_car_h_m` tall, its floor `el_floor_m` over the rail deck, on
    /// the track `el_track_m` left of the deck's centre line (the way it runs).
    pub el_cars: u8,
    pub el_car_m: f32,
    pub el_car_gap_m: f32,
    pub el_car_w_m: f32,
    pub el_car_h_m: f32,
    pub el_floor_m: f32,
    pub el_track_m: f32,
    /// A rider's place in a car: along the walls every this many metres.
    pub el_slot_m: f32,
    /// V4: the lift car a lift rider stands in (side, height), metres.
    pub lift_cab_m: f32,
    pub lift_cab_h_m: f32,
    /// V4: a stairs climber walks round the core at this radius, a turn a storey.
    pub stair_radius_m: f32,
    /// V10: a storey whose floor is at z stands on the lot's massing box whose height holds z + this (its slab):
    /// a tower storey on the tower's plate, not on the whole lot.
    pub storey_slab_m: f32,
    /// V4: bodies standing on one tile spread over the walkable ground
    /// within this many metres, about `crowd_gap_m` apart.
    pub crowd_spread_m: f32,
    pub crowd_gap_m: f32,
    /// The view milestone V6 (`OPEN-270`): `shadow --record`'s ring half side in tiles (each pick's surroundings
    /// a tick, as the shadow's world has them).
    pub record_half_tiles: u16,
    /// The recording's gzip level (0-9; flate2's `Compression`).
    pub record_gzip_level: u32,
    /// M18 P1 (cityview's player HUD): the play speeds, multiples of the
    /// walk-matched rate (1x: a body on screen walks `walk_m_s`), the `,` /
    /// `.` keys step through (T2, docs/TIME_MODEL.md § 2).
    pub play_speeds: Vec<f64>,
    /// M18 P1: a click farther than this many metres along the camera ray
    /// hits nothing.
    pub pick_reach_m: f32,
    /// M18 P1: the ray's march step, metres (a thin wall is missed past it).
    pub pick_step_m: f32,
    /// M18 P1: a click on a roof or a wall walks into the sim's building
    /// nearest the hit within this many tiles (the lots and the rects
    /// disagree at their edges).
    pub pick_snap_tiles: u16,
    /// M18 P2 (`systems/player.rs`): the kinds whose door opens to anyone
    /// from a click (the player's own home, workplace, squat, booked bed and
    /// gang's Hideout open too); any other door is walked to, not through.
    pub play_public_kinds: Vec<BuildingKind>,
    /// M18 P2: a click within this many pixels of a drawn body (its chest
    /// projected to the screen) picks the body, not what is behind it.
    pub pick_body_px: f32,
    /// M18 P2: the HUD's width, pixels (bottom right, out of the far
    /// field's clicks at the top of the frame).
    pub hud_width_px: f32,
    /// M18 P4 (consequences, `systems/player_news.rs`): the game minutes
    /// between two reads of what the city knows of the player (a read scans
    /// every body's memory).
    pub news_scan_minutes: u32,
    /// M18 P4: "word travels" (more bodies know a deed of the player's)
    /// is said at most once in this many game minutes.
    pub news_word_minutes: u32,
    /// M18 P4: the player's own log keeps this many lines.
    pub news_log_lines: usize,
    /// M18 P4: "who knows about you" lists at most this many bodies.
    pub news_knowers_max: usize,
    /// M18 P4: the day-end's "your block": the events within this many
    /// tiles of the player's home door (where it stands, homeless).
    pub block_tiles: u16,
    /// C5 (docs/COMBAT.md § 5, the combat clock): while the body the human
    /// drives is in a live fight the view steps its engagement a beat every
    /// this many real seconds inside the tick (orders land between beats),
    /// then runs the rest of the tick: `beats_per_tick` beats a game minute,
    /// as an NPC's fight; normal speed when the fight ends.
    pub beat_s: f64,
    /// C4: a beat's cue (a word over a body, a flash) floats this many real
    /// seconds.
    pub cue_s: f32,
    /// C5: the replay's procedural poses on the existing rig: a swing lunges
    /// `lunge_m` toward the struck and leans `lunge_deg` forward, a hit of a
    /// Wound or worse staggers the struck back as far; each over `lunge_s`
    /// real seconds.
    pub lunge_s: f32,
    pub lunge_m: f32,
    pub lunge_deg: f32,
    /// C5 (`OPEN-297`): indoors, the bodies of the subject's fight stand
    /// round it this far (metres), facing it (the view's indoor place is a
    /// jitter across the room).
    pub fight_ring_m: f32,
    /// M18 P5: the auto-pause's start (the HUD toggles it): the clock stops
    /// when the possessed body is attacked or a fight starts within
    /// `autopause_tiles` of it while it is on autopilot or the human is away
    /// (no key or click for `away_s` real seconds).
    pub autopause: bool,
    pub autopause_tiles: u16,
    pub away_s: f64,
    /// P5b: a slice game (a new one, a load) starts at this index of `play_speeds` (T2: 0, 1x).
    pub start_speed_i: usize,
    /// T2 (docs/TIME_MODEL.md § 3): auto-speed. Through a stretch of the possessed body's day with nothing to watch
    /// (asleep, on shift inside, riding, queued, serving time, an autopilot indoor activity) the clock runs at
    /// `auto_speed` x the walk-matched rate (capped by what the sim thread manages), ramped over `auto_ramp_s` real
    /// seconds each way; `auto_speed_on` is the toggle's start.
    pub auto_speed: f64,
    pub auto_ramp_s: f64,
    pub auto_speed_on: bool,
    /// T2 (§ 4): when the render thread falls behind the sim thread, a published frame carries the ways walked over
    /// at most this many ticks (then it is published anyway).
    pub trail_max_ticks: u32,
    /// T2 (§ 4): a body whose way ahead does not reach its next drawn place (a walk begun inside the tick, a door
    /// left) has the street way between the two searched when they lie within this many tiles; farther it is a
    /// jump in the sim (a placement), drawn as one.
    pub trail_repair_tiles: u16,
    /// M18 P5: a play script's `provoke lethal` (a proof's cheat, never the
    /// player's): the fight's kill chance on a loss.
    pub provoke_kill_p: f32,
}

impl Default for ViewCfg {
    fn default() -> Self {
        ViewCfg {
            tile_m: 4.0,
            north_m: 3750.0,
            walk_m_s: 1.4,
            ring_half_tiles: 48,
            way_margin_tiles: 64,
            reload_poll_s: 1.0,
            people_anim_m: 40.0,
            people_anim_max: 48,
            cam_fov_deg: 30.0,
            cam_yaw_deg: 45.0,
            cam_pitch_deg: 50.0,
            cam_pitch_min_deg: 35.0,
            cam_pitch_max_deg: 65.0,
            cam_zoom_min_m: 12.0,
            cam_zoom_max_m: 1000.0,
            cam_follow_half_life_s: 0.25,
            cam_snap_m: 40.0,
            cam_look_up_m: 1.0,
            cam_rotate_deg_s: 90.0,
            cam_pitch_deg_s: 45.0,
            cam_drag_deg_px: 0.25,
            cam_zoom_step: 1.15,
            cam_far_m: 3000.0,
            light_dawn_h: 6.0,
            light_dusk_h: 19.0,
            light_twilight_h: 1.0,
            light_day_sky: 6000.0,
            light_day_sun: 12000.0,
            light_day_ev100: 13.0,
            light_night_sky: 600.0,
            light_night_moon: 900.0,
            light_night_ev100: 8.8,
            light_bare_sky: 250.0,
            light_bare_moon: 400.0,
            cut_width_m: 36.0,
            cut_behind_m: 1.5,
            cut_above_m: 1.8,
            cut_band_m: 1.2,
            cut_rim: 0.6,
            cut_max_zoom_m: 250.0,
            cut_ceiling_m: 2.8,
            child_scale_min: 0.5,
            child_full_age: 17,
            people_glow: 0.5,
            el_cars: 4,
            el_car_m: 20.0,
            el_car_gap_m: 1.0,
            el_car_w_m: 3.0,
            el_car_h_m: 3.2,
            el_floor_m: 0.4,
            el_track_m: 2.5,
            el_slot_m: 1.4,
            lift_cab_m: 2.0,
            lift_cab_h_m: 2.6,
            stair_radius_m: 1.5,
            storey_slab_m: 0.5,
            crowd_spread_m: 10.0,
            crowd_gap_m: 0.9,
            record_half_tiles: 48,
            record_gzip_level: 6,
            play_speeds: vec![1.0, 2.0, 5.0, 20.0, 60.0],
            pick_reach_m: 3000.0,
            pick_step_m: 0.5,
            pick_snap_tiles: 2,
            play_public_kinds: {
                use BuildingKind as K;
                vec![
                    K::Market,
                    K::Bar,
                    K::Hotel,
                    K::Clinic,
                    K::Hall,
                    K::Club,
                    K::Arcade,
                    K::NoodleBar,
                    K::FightPit,
                    K::Den,
                    K::Lounge,
                    K::Fixer,
                    K::Mission,
                    K::Garage,
                    K::Gunsmith,
                    K::StripClub,
                    K::Brothel,
                    K::Boutique,
                    K::Cemetery,
                    K::Station,
                ]
            },
            pick_body_px: 14.0,
            hud_width_px: 300.0,
            news_scan_minutes: 10,
            news_word_minutes: 120,
            news_log_lines: 300,
            news_knowers_max: 40,
            block_tiles: 12,
            beat_s: 1.0,
            cue_s: 1.2,
            lunge_s: 0.45,
            lunge_m: 0.45,
            lunge_deg: 18.0,
            fight_ring_m: 1.3,
            autopause: true,
            autopause_tiles: 8,
            away_s: 30.0,
            start_speed_i: 0,
            auto_speed: 600.0,
            auto_ramp_s: 1.0,
            auto_speed_on: true,
            trail_max_ticks: 60,
            trail_repair_tiles: 64,
            provoke_kill_p: 1.0,
        }
    }
}

/// `[perf]` (the perf pass phase 8, OPEN-184): the host's threads; phase
/// 7b: the midnight stagger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerfCfg {
    /// Threads scoring the tick's think slice (`systems::think::run`); 1
    /// scores it on the calling thread. Every value gives the same city.
    /// Never saved.
    #[serde(default = "PerfCfg::one_thread", skip_serializing)]
    pub threads: usize,
    /// The perf pass phase 7b (R8): ticks between two slots of the
    /// staggered midnight (`systems::midnight`); 0 (absent): every daily
    /// pass on tick 0, the base.
    #[serde(default)]
    pub stagger_midnight: u16,
}

impl Default for PerfCfg {
    fn default() -> Self {
        PerfCfg { threads: 1, stagger_midnight: 0 }
    }
}

impl PerfCfg {
    fn one_thread() -> usize {
        1
    }

    /// Nothing of the world's in it (no stagger): left out of a save.
    pub fn host_only(&self) -> bool {
        self.stagger_midnight == 0
    }
}

/// Scale S4 (docs/SCALE_20K.md § S4): the reference city the config's
/// absolute keys were tuned on, the shipped map (`assets/map.txt`) at 2,000
/// residents. `Config::scaled_to` scales the per-person keys by `population
/// / this population`; `Config::fit_to_map` (at `World::new`) the
/// per-building keys by the map's count of the kind over the count here.
/// On the shipped map at 2,000 every ratio is 1. `enabled = false` (the v1
/// profile, whose map is its own reference) leaves the map's keys alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScaleCfg {
    pub enabled: bool,
    /// Residents the absolute keys assume.
    pub population: u32,
    /// The reference map's building counts.
    pub farms: u32,
    pub markets: u32,
    pub bars: u32,
    pub hideouts: u32,
    pub offices: u32,
    pub lots: u32,
    /// The reference map's districts (`[districts]` rows).
    pub districts: u32,
    /// Review finding 4: `fit_to_map` has run on this config (it is
    /// idempotent: a world's own config, fitted and saved, handed back to
    /// `World::new` by a replay is not fitted twice). Not a config.toml key;
    /// written into a save only when set.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fitted: bool,
    /// Pressure pass F2: the copies of the opening roster whose businesses
    /// stand on Lots (`fit_to_map`: the map's Lots over `lots`, at most the
    /// roster's copies; 0 before fitting: the roster's). Not a config.toml
    /// key; written into a save only when set.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lot_copies: u32,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl Default for ScaleCfg {
    fn default() -> Self {
        ScaleCfg {
            enabled: true,
            population: 2000,
            farms: 12,
            markets: 3,
            bars: 3,
            hideouts: 2,
            offices: 2,
            lots: 160,
            districts: 8,
            fitted: false,
            lot_copies: 0,
        }
    }
}

impl ScaleCfg {
    /// Off: `fit_to_map` changes nothing (the v1 profile).
    pub fn off() -> Self {
        ScaleCfg { enabled: false, ..ScaleCfg::default() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrainCfg {
    pub memory_cap: usize,
    pub memory_half_life_days: f32,
    pub goal_hysteresis: f32,
    pub goal_cooldown_ticks: u64,
    /// Round 3 S2 (OPEN-222): each cool of a goal in a row (no plan of it
    /// finished between) lasts this many times the last one, from
    /// `goal_cooldown_ticks`, at most `cool_backoff_max_ticks` (0: off, every
    /// cool `goal_cooldown_ticks`); `GoalKind::backs_off` goals only.
    #[serde(default)]
    pub cool_backoff: f32,
    #[serde(default)]
    pub cool_backoff_max_ticks: u64,
    /// The streak lapses when the goal has not cooled for this long after
    /// its last cool ended.
    #[serde(default)]
    pub cool_backoff_reset_ticks: u64,
    pub think_interval_ticks: u64,
    pub plan_budget_per_tick: usize,
    pub plan_max_expansions: usize,
    pub plan_max_len: usize,
    pub plan_timeout_ticks: u64,
    /// A search past this many expansions in one tick resumes next tick.
    pub plan_expansion_budget_per_tick: usize,
    /// Shadow V4 bug 20: a queue entry this old is served before the more
    /// urgent ones (oldest first), so it is planned before it goes stale
    /// (0 = urgency alone).
    #[serde(default)]
    pub plan_age_promote_ticks: u64,
    pub path_budget_per_tick: usize,
    pub mood_w_need: f32,
    pub mood_w_memory: f32,
    /// Round 9 Y1 (`OPEN-249`): a starving body's food source that failed
    /// (a bin emptied before it got there, a kitchen out of bowls) is passed
    /// over this long while it tries the next.
    #[serde(default = "default_food_tried_ticks")]
    pub food_tried_ticks: u64,
    /// Round 9 Y1: the failed sources a starving body moves past before its
    /// Eat cools as any goal's (0: cools at once, the old rule).
    #[serde(default = "default_food_tries")]
    pub food_tries: u32,
    /// Round 9 Y1: a starving adult with no other source steals a meal
    /// when its lawfulness is under this (the fed thief's line is `0.3`).
    #[serde(default = "default_starving_steal_lawfulness")]
    pub starving_steal_lawfulness: f32,
}
fn default_food_tried_ticks() -> u64 {
    240
}
fn default_food_tries() -> u32 {
    4
}
fn default_starving_steal_lawfulness() -> f32 {
    0.7
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecCfg {
    /// The walking pace's time side: a walker crosses `walk_tiles_per_tick`
    /// tiles every `move_ticks_full` ticks (round 9 Y4; before it, one tile
    /// per this many ticks).
    pub move_ticks_full: u64,
    /// Round 9 Y4 (Dylan 2026-10-30): the walking pace's distance side, the
    /// tiles a walker crosses in `move_ticks_full` ticks (Y4's 21 of the
    /// island's 4 m tiles a minute was ~5 km/h in game time; the time model
    /// T1 makes it 7 every 2 ticks, 3.5 a game minute, the 1.4 m/s on screen
    /// at the 6x-compressed day). Absent: 1, the pace before Y4.
    #[serde(default = "default_walk_tiles_per_tick")]
    pub walk_tiles_per_tick: u64,
    /// Round 9 Y4: the Full executor's sub-tick resolution, units of one
    /// walker's step (a vehicle's `[vehicles] mult` resolves to 1 /
    /// `step_res` of it). Absent: 4, the M13 quarter ticks.
    #[serde(default = "default_step_res")]
    pub step_res: u64,
    pub interrupt_check_ticks: u64,
    pub door_capacity_per_tick: u8,
    pub door_queue_max_ticks: u64,
    pub reservation_ttl: u64,
    pub astar_max_expansions: usize,
    /// Calibration only: every walk is a timed arrival (manhattan x 2 ticks).
    #[serde(default)]
    pub straight_line_paths: bool,
    /// Flow fields kept (LRU); each is one byte per tile. Eviction never
    /// changes results: a field is a pure function of the map and its door.
    #[serde(default = "default_flow_cache")]
    pub flow_field_cache: usize,
    /// Scale S3a: the flow fields' byte budget in MiB (caps
    /// `flow_field_cache` by the map's size: `FlowCache::cap_for`).
    #[serde(default = "default_flow_budget_mb")]
    pub flow_field_budget_mb: usize,
    /// Scale S3a: maps of more tiles than this walk by exact A* paths
    /// (`exec::bigpath`) instead of per-door flow fields.
    #[serde(default = "default_flow_max_tiles")]
    pub flow_field_max_tiles: usize,
    /// Workplace room: an agent refused at a full building
    /// (`FailReason::BuildingFull`) passes it over in `World::local` for
    /// this many ticks, so the replan goes elsewhere (another Market, the
    /// home pantry) instead of back to the same door (0: off).
    #[serde(default)]
    pub refused_ticks: u64,
    /// Workplace room: a building's own staff are let in when it is full
    /// (the staff door; the trades' J13 rule for every role), so customers
    /// never lock the shift out (false: the trades and the sweepers only).
    #[serde(default)]
    pub staff_door: bool,
}

fn default_walk_tiles_per_tick() -> u64 {
    1
}

fn default_step_res() -> u64 {
    4
}

impl ExecCfg {
    /// Round 9 Y4: the ticks to walk `tiles` on foot at the pace, rounded
    /// up: `ceil(tiles x move_ticks_full / walk_tiles_per_tick)` (0 for 0
    /// tiles, at least 1 for any step). The one walk estimate every reader
    /// of the pace goes through; [`ExecCfg::walk_tiles`] is its inverse.
    pub fn walk_ticks(&self, tiles: u64) -> crate::time::Tick {
        tiles.saturating_mul(self.move_ticks_full).div_ceil(self.walk_tiles_per_tick.max(1))
    }

    /// Round 9 Y4: [`ExecCfg::walk_ticks`] scaled by `mult` (a vehicle's
    /// share of the walk, litter, chrome legs), rounded to the nearest tick
    /// as the M12/M13 arithmetic was (at the pace before Y4 exactly
    /// `round(tiles x move_ticks_full x mult)`).
    pub fn walk_ticks_f(&self, tiles: u64, mult: f32) -> crate::time::Tick {
        let t = tiles as f64 * self.move_ticks_full as f64 * f64::from(mult) / self.walk_tiles_per_tick.max(1) as f64;
        t.round().max(0.0) as crate::time::Tick
    }

    /// Round 9 Y4: the tiles walked in `ticks` at the pace (floor): a tile
    /// is within `ticks` on foot exactly when `walk_ticks(tiles) <= ticks`.
    pub fn walk_tiles(&self, ticks: crate::time::Tick) -> u64 {
        ticks.saturating_mul(self.walk_tiles_per_tick.max(1)) / self.move_ticks_full.max(1)
    }

    /// Round 9 Y4: the Full executor's step units in one tick
    /// (`step_res x walk_tiles_per_tick`).
    pub fn units_per_tick(&self) -> u64 {
        self.step_res.max(1) * self.walk_tiles_per_tick.max(1)
    }

    /// Round 9 Y4: an unchromed walker's step in units (`step_res x
    /// move_ticks_full`): `units_per_tick / step_units` steps a tick.
    pub fn step_units(&self) -> u64 {
        self.step_res.max(1) * self.move_ticks_full.max(1)
    }
}

fn default_flow_cache() -> usize {
    512
}

fn default_flow_budget_mb() -> usize {
    64
}

fn default_flow_max_tiles() -> usize {
    131_072
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LodCfg {
    pub max_full: usize,
    pub max_coarse: usize,
    /// Set by the CLI's `--force-lod`; never in the file.
    #[serde(default)]
    pub force: Option<Lod>,
    /// M10: days of `Trace` kept per adult.
    #[serde(default = "default_trace_days")]
    pub trace_days: usize,
    /// M10: multiplies the table's `p_robbed`, `p_assaulted` and `p_killed`
    /// at use (D26); the parity test pins it to 1.0.
    #[serde(default = "default_stat_violence_mult")]
    pub stat_violence_mult: f32,
    /// Experiment: the Statistical tier's policy, `"table"` (the calibrated
    /// 24-row table) or `"mlp"` (`assets/stat_mlp.toml`; see
    /// `docs/history/EXPERIMENT_LEARNED_STAT_POLICY.md`).
    #[serde(default = "default_stat_policy")]
    pub policy: String,
    /// M13 D46: a Statistical kitted agent's `p_robbed × (1 + flash_w × Kit.flash)`.
    #[serde(default)]
    pub flash_w: f32,
    /// M13 D37: the binder weights an Assaulted or Killed candidate by
    /// `1 + chrome_bind_w × Kit.fighting`.
    #[serde(default)]
    pub chrome_bind_w: f32,
    /// L2 (L21): sentenced, unpinned agents are held Statistical.
    #[serde(default = "LodCfg::default_true")]
    pub held_prisoners: bool,
    /// L2 (L22): class-2 bodies per gang, and in a raid window.
    #[serde(default = "LodCfg::default_gang_quota")]
    pub gang_quota: usize,
    #[serde(default = "LodCfg::default_private_quota")]
    pub private_quota: usize,
    #[serde(default = "LodCfg::default_raid_quota")]
    pub raid_quota: usize,
    /// L2 (L21, L22): the raid window opens this many hours before the muster.
    #[serde(default = "LodCfg::default_raid_promote_hours")]
    pub raid_promote_hours: u16,
    /// L2 (L22): members this close to the order's target rank in the front line.
    #[serde(default = "LodCfg::default_front_tiles")]
    pub front_tiles: u32,
    /// L2 (L21): a prisoner this close to release holds a body (the walk out).
    #[serde(default = "LodCfg::default_release_soon_hours")]
    pub release_soon_hours: u16,
    /// L2 (L22, spec § 4): the public watch ranks 3 only on shift (off
    /// shift it ranks as a civilian); `false` keeps M10 D20 (always 3).
    #[serde(default = "LodCfg::default_true")]
    pub watch_on_shift_only: bool,
    /// M16a (plan C35, phase 4): live contract parties' bodies (class 4).
    #[serde(default = "LodCfg::default_contract_quota")]
    pub contract_quota: usize,
    /// M16a (plan C35, phase 4): Fixer owners' bodies (class 2) in the
    /// matching window.
    #[serde(default = "LodCfg::default_fixer_quota")]
    pub fixer_quota: usize,
    /// Scale S1: a Statistical worker's hour of co-work drifts it with its
    /// crew, the `coworker_crew` co-workers of its role and employer nearest
    /// it in id order (half each side, wrapping), not with every one: the
    /// pairs grew with the square of a workplace's staff (a 20,000 city's
    /// Farms hold ~300). 0 is every co-worker (the pre-S1 rule); a group of
    /// at most `coworker_crew + 1` is every co-worker either way.
    #[serde(default)]
    pub coworker_crew: usize,
    /// The view milestone V5 (OPEN-149): the camera ring's own budget, outside `max_full` / `max_coarse`: bodies
    /// held Full in the ring (the view + `lod::VIEW_MARGIN`), nearest the camera first; the rest of the ring Coarse.
    #[serde(default = "LodCfg::default_ring_full_max")]
    pub ring_full_max: usize,
    /// Bodies held Coarse in the band around the ring (and the ring's overflow).
    #[serde(default = "LodCfg::default_ring_coarse_max")]
    pub ring_coarse_max: usize,
    /// The band's width in tiles past the ring.
    #[serde(default = "LodCfg::default_ring_band_tiles")]
    pub ring_band_tiles: u16,
    /// A held body keeps its ring tier this many tiles past the ring (Full) or the band (Coarse): no flap at the edge.
    #[serde(default = "LodCfg::default_ring_hold_tiles")]
    pub ring_hold_tiles: u16,
    /// A view wider than this (tiles) is a map overview, not a camera: the ring holds nothing.
    #[serde(default = "LodCfg::default_ring_max_side_tiles")]
    pub ring_max_side_tiles: u16,
    /// The ring reaches ahead by the camera's last move (the frame is drawn after the tick, where the subject is
    /// then); a move past this many tiles is a cut, no lead.
    #[serde(default = "LodCfg::default_ring_lead_max_tiles")]
    pub ring_lead_max_tiles: u16,
}

impl LodCfg {
    fn default_true() -> bool {
        true
    }
    fn default_ring_full_max() -> usize {
        3000
    }
    fn default_ring_coarse_max() -> usize {
        4000
    }
    fn default_ring_band_tiles() -> u16 {
        24
    }
    fn default_ring_hold_tiles() -> u16 {
        8
    }
    fn default_ring_max_side_tiles() -> u16 {
        256
    }
    fn default_ring_lead_max_tiles() -> u16 {
        96
    }
    fn default_contract_quota() -> usize {
        24
    }
    fn default_fixer_quota() -> usize {
        4
    }
    fn default_gang_quota() -> usize {
        25
    }
    fn default_private_quota() -> usize {
        12
    }
    fn default_raid_quota() -> usize {
        40
    }
    fn default_raid_promote_hours() -> u16 {
        4
    }
    fn default_front_tiles() -> u32 {
        30
    }
    fn default_release_soon_hours() -> u16 {
        2
    }
}

fn default_stat_policy() -> String {
    "table".to_string()
}

fn default_trace_days() -> usize {
    120
}

fn default_stat_violence_mult() -> f32 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeversCfg {
    pub tax_rate: f32,
    pub sentence_mult: f32,
    pub guard_count: u16,
    pub immigration_per_week: u16,
    /// P10: the dole's rate while the dole is enacted (`[policy] dole` at
    /// seed, the `safety_net` lever); a city without it seeds 0.
    pub dole_per_day: u8,
    /// M12 D23: city Sanitation workers (0 in a pre-M12 save: nobody sweeps).
    #[serde(default)]
    pub sanitation_count: u16,
    /// M13 D40: Markets sell Stims legally from day 0.
    #[serde(default)]
    pub stims_legal: bool,
    /// M14 (plan V26/V42): the ICE seeded on the Treasury and the Precinct.
    #[serde(default = "default_city_ice")]
    pub city_ice: u8,
}

fn default_city_ice() -> u8 {
    2
}

/// P10 (docs/P10_POLICY.md): the city's policy at world creation. It seeds
/// the saved policy levers (`levers::Levers`); after day 0 only the levers
/// are read (`--lever day=D:<key>=<value>`, `PlayerCommand::SetPolicy`).
/// The default is the shipped city: the market wage law and no safety net.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyCfg {
    /// The levy (the M11 economy, the dole city's): the config wage
    /// schedule, the corps' upkeep levy to the Treasury and L2's budget band
    /// (public works sized by the surplus, the levy cut), the Treasury's
    /// scavenge coin. Off: wages from revenue, inputs and the property rate,
    /// the tax band and civic budget, the Recycler's till (the Real economy).
    pub levy: bool,
    /// The dole at `[levers] dole_per_day` a day, paid by the Treasury.
    pub dole: bool,
    /// Public works: Sanitation posts the Treasury band opens above its top.
    pub public_works: bool,
    /// School meals: a child whose pantry is empty eats from the Reserve.
    pub school_meals: bool,
    /// The Reserve's free release to a stranded Market (L2 phase 5).
    pub reserve_release: bool,
    /// The dole and public works lapse after this many days in a row on
    /// which the Treasury could not pay (0: never).
    pub lapse_days: u16,
}

impl Default for PolicyCfg {
    fn default() -> Self {
        PolicyCfg {
            levy: false,
            dole: false,
            public_works: false,
            school_meals: false,
            reserve_release: false,
            lapse_days: 7,
        }
    }
}

impl PolicyCfg {
    /// The dole city's policy (the old default before the flip): the levy
    /// and the whole safety net.
    pub fn dole_city() -> PolicyCfg {
        PolicyCfg {
            levy: true,
            dole: true,
            public_works: true,
            school_meals: true,
            reserve_release: true,
            ..PolicyCfg::default()
        }
    }
}

/// M18 P5 (docs/M18_PLAYER.md, start and loop): the start menu's nobodies
/// (`player::nobodies`), the fresh arrival (`player::arrive_fresh`) and the
/// witness's demand as an offer (`consequence::blackmailed`). The Default
/// mirrors `assets/config.toml` (the slice save was written before the
/// section).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SliceCfg {
    /// "Pick a nobody" lists at most this many adults (one per kind).
    pub nobody_count: usize,
    /// A nobody holds at most this many coins (broke).
    pub nobody_coins: i64,
    /// A nobody is 18 up to this age in years.
    pub nobody_age_max: u32,
    /// P5b: a nobody has no job or one paying at most this a day, and no gang post.
    pub nobody_wage_max: i64,
    /// P5b: at most this many nobodies of one district on the list while another district has a candidate.
    pub nobody_per_district: usize,
    /// The witness's demand on a body the human drives (or its autopilot)
    /// stands as an offer this many game minutes (pay, refuse, go for him);
    /// let lapse it is refused.
    pub blackmail_offer_minutes: u64,
}

impl Default for SliceCfg {
    fn default() -> Self {
        SliceCfg {
            nobody_count: 8,
            nobody_coins: 25,
            nobody_age_max: 60,
            nobody_wage_max: 5,
            nobody_per_district: 2,
            blackmail_offer_minutes: 30,
        }
    }
}

/// L2 § 1 `[export]`: what is left of L2's export hook under the World
/// market (2026-10-09: the flat-price hook and its switch retired).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportCfg {
    /// The World account's opening treasury when a donation creates it first.
    pub treasury_ref: i64,
    /// Plan key: a Fab's Parts kept back.
    pub parts_floor: u32,
    /// Plan key (deviation: M14 has no Data sell floor): a Lab's Data kept back.
    pub data_floor: u32,
}

impl Default for ExportCfg {
    fn default() -> Self {
        ExportCfg::off()
    }
}

impl ExportCfg {
    pub fn off() -> ExportCfg {
        ExportCfg { treasury_ref: 20_000, parts_floor: 20, data_floor: 100 }
    }
}
