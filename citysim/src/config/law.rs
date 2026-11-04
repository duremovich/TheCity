use super::*;

/// Pressure pass round 3, S3 `[justice]` (docs/PRESSURE_PASS.md § S3;
/// `OPEN-223`): the cells keep the body alive, the watch acts on what it
/// V8 (`OPEN-256`) `[cells]`: the cells' week. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellsCfg {
    /// A sentence of this many days or fewer is bought out at booking for
    /// `fine_per_day` coins a day of it, paid from the purse (the convict's,
    /// then his kin's: a spouse, a parent, a grown child), to the city
    /// (`Flow::Fine`); 0: none.
    pub fine_max_days: u32,
    pub fine_per_day: i64,
    /// A prisoner with this many days or fewer left on a term of
    /// `bail_term_max_days` or fewer is bailed out at midnight for
    /// `bail_per_day` coins a day left, from the same purses (a gang's
    /// member is its gang's first, `[protection] bail_*`); 0: none.
    pub bail_max_days: u32,
    pub bail_term_max_days: u32,
    pub bail_per_day: i64,
    /// A purse pays a fine or bail only above this (the week's food).
    pub keep_coins: i64,
    /// Overcrowding: past this share of the Precinct's beds (`[buildings]
    /// jail capacity`) held at midnight, the least dangerous (the pettiest
    /// crime, then the soonest out) go early until `crowd_to` of them; a
    /// line each. 0: never.
    pub crowd_from: f32,
    pub crowd_to: f32,
    /// The work detail: at this hour a prisoner works on `work_p` (a draw a
    /// prisoner a day) one of `work_details` for `work_pay` coins from the
    /// city (`Flow::Wage`); 24: off.
    pub work_hour: u8,
    pub work_p: f32,
    pub work_pay: i64,
    pub work_details: Vec<String>,
    /// Cards in the yard (`[consequence] cell_yard_hour`): a prisoner with
    /// nothing else on plays a cellmate on `cards_p`, `cards_stake` coins
    /// changing hands (`Flow::Gamble`), the better hand by a draw.
    pub cards_p: f32,
    pub cards_stake: i64,
    /// BreakOut: an order a gang cannot march (fewer than `[gangs]
    /// raid_min_members` of its free adults within `[crew] duty_reach_ticks`
    /// of the Precinct) is dropped as it is chosen, a line saying why.
    pub breakout_reach: bool,
}

/// sees. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JusticeCfg {
    /// The cells' kitchen: the day's meal comes from the first of the
    /// Jail's nearest this-many Markets with food on the shelf (the nearest
    /// alone ran dry at 100k and the held starved).
    pub kitchen_markets: usize,
    /// Lights-out: a prisoner on screen sleeps in the cell from this hour
    /// to `wake_hour` (energy restores as in a bed; the midnight reset to
    /// 1.0 is the off-screen hold's alone).
    pub lights_out_hour: u8,
    pub wake_hour: u8,
    /// A guard who saw a crime past petty himself (the report he filed)
    /// takes the suspect as his collar for this many hours: the Arrest
    /// goal's target first, on shift or off (0: the nearest warrant, on
    /// shift only, as before).
    pub collar_hours: u64,
    /// ... and his Arrest goal carries this flat while he has one (a guard
    /// who saw a murder on his commute walked on to the desk: Work 0.99).
    pub collar_flat: f32,
    /// Pressure pass round 7, W3 (`OPEN-244`): a chase begun this many hours
    /// ago or less is finished past the shift's end (0: the shift's end drops it).
    pub chase_overtime_hours: u64,
    /// Round 7 W3: the warrant at the door. From `door_from_hour` to
    /// `door_to_hour` (across midnight when from > to; equal: off), a warrant
    /// whose suspect no guard has seen within `[crime] suspect_seen_ticks`
    /// and who is inside his own Home is taken by the nearest free city
    /// guard on shift within his pursuit radius: the address is the
    /// sighting, the arrest is made at home (a warrant the street never saw
    /// was never served).
    pub door_from_hour: u8,
    pub door_to_hour: u8,
    /// Round 8 X6 (`OPEN-217`): the cells' meal is served at this hour (the
    /// jailer's count, a line a prisoner); 0: at midnight with the hold's
    /// rest (as before).
    pub meal_hour: u8,
    /// Pressure pass round 9, Y2 (`OPEN-251`): a suspect who fights the
    /// cuffs faces the on-duty city guards within this Chebyshev radius of
    /// him too, each in turn (0: the arresting guard alone) ...
    pub backup_reach: u32,
    /// ... up to this many guards at the cuffs, the arresting one included.
    pub backup_max: usize,
}

impl Default for JusticeCfg {
    fn default() -> Self {
        JusticeCfg {
            kitchen_markets: 4,
            lights_out_hour: 22,
            wake_hour: 6,
            collar_hours: 2,
            collar_flat: 0.5,
            chase_overtime_hours: 2,
            door_from_hour: 22,
            door_to_hour: 6,
            meal_hour: 0,
            backup_reach: 6,
            backup_max: 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrimeCfg {
    pub steal_base_cost: f32,
    pub steal_lawfulness_factor: f32,
    pub steal_guard_penalty: f32,
    pub steal_starving_bonus: f32,
    pub steal_dark_bonus: f32,
    pub beg_base_cost: f32,
    pub beg_pride_factor: f32,
    pub forage_base_cost: f32,
    pub witness_base: f32,
    pub witness_stealth_factor: f32,
    pub witness_guard_bonus: f32,
    pub sight: u32,
    pub sight_day_crime: u32,
    pub sight_night_crime: u32,
    /// Theft, Extortion, Assault, Murder.
    pub sentence_days: [u32; 4],
    pub fight_death_p: f64,
    pub stat_theft_caught_p: f64,
    /// Unresolved warrants expire after this many days.
    pub warrant_expiry_days: u64,
    /// A sighting locates a suspect for this many ticks.
    pub suspect_seen_ticks: u64,
    /// A Theft arrest at a full Jail becomes a fine of this many × price_food.
    pub fine_mult: i64,
    /// M12 review: a guard answers an alarm at a door (a corp raid, a riot)
    /// only from within this many tiles of it (Chebyshev), on shift and
    /// awake; the brawl then stands it at the door.
    #[serde(default = "CrimeCfg::default_answer_radius")]
    pub answer_radius: u32,
    /// M13 D47: sentences for the crimes appended after Vagrancy (days).
    #[serde(default)]
    pub sentence_days_ext: SentenceDaysExt,
    /// Pressure pass F2: a crime's witnesses who note it (a `SawCrime`, an
    /// edge, a `Witness` line), at most; the guards on shift are rolled first
    /// and always count (a fare dodge on a 100k platform drew 26 witnesses
    /// in a minute, 641k `Witness` lines in 30 days). 0: no cap.
    #[serde(default = "CrimeCfg::default_witness_cap")]
    pub witness_cap: u32,
    /// Round 5 U4 (`OPEN-236`): the building kinds with no walls to see
    /// through (a market's stalls, a yard, an empty lot): a body in one sees
    /// the street and the street sees in, as on its door tile. Every other
    /// building's walls hold: who is inside sees inside, who is outside sees
    /// the street (`law::sees_scene`).
    #[serde(default = "CrimeCfg::default_open_air")]
    pub open_air: Vec<BuildingKind>,
    /// Round 6 V3 (`OPEN-238`): a body found and the law's case, and the
    /// victim who tells a guard on the street (`[crime.case]`).
    #[serde(default)]
    pub case: CaseCfg,
}

/// Round 6 V3 (`OPEN-238`, `[crime.case]`; `demography::find_bodies`,
/// `law::hue_and_cry`, `law::tell_the_beat`): violence with consequence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaseCfg {
    /// The find pass runs every this many ticks over the unfound bodies of
    /// violence (a body is found within this long of a passer-by's sight).
    pub find_every_ticks: u64,
    /// The law names the killer of a found body on this chance (the scene,
    /// the street asked) ...
    pub lead_p: f32,
    /// ... or this one when the killer and the dead knew each other (a
    /// motive, the people around them asked).
    pub tie_lead_p: f32,
    /// A victim's shout (the hue and cry) reaches an on-duty guard outdoors
    /// within this many tiles of a street crime (Chebyshev; sight is 4-8).
    pub hue_reach: u32,
    /// Hourly, a victim (or a murder's witness) holding an unreported
    /// first-hand sighting tells an on-duty guard outdoors within
    /// `hue_reach`, from a lawfulness this high.
    pub tell_lawful_min: f32,
    /// ... of a sighting no older than this many days (ReportCrime's two).
    pub tell_days: u64,
    /// Round 7 W2 (`OPEN-238`): a victim or witness under this courage
    /// keeps quiet about a man whose gang holds the block he lives on (fear:
    /// the holders are still there tomorrow).
    pub fear_courage_max: f32,
    /// Round 8 X1 (`OPEN-246`): a promise to tell the law ("means to tell")
    /// stands this many hours: given at the nearest open desk (a Hall, the
    /// Precinct, a Station within `word_reach_ticks`), to a
    /// guard on the street or in a shout, or dropped with the reason.
    pub word_hours: u64,
    /// ... and ReportCrime's flat bonus while it stands (on the trip's
    /// travel factor: a promise outranks the evening's leisure).
    pub word_flat: f32,
    /// ... and the desk it is walked to lies within this many travel ticks
    /// (`distance::trip_to`; a passing sighting's is `[distance.reach]
    /// report_ticks`).
    pub word_reach_ticks: u64,
    /// Round 8 X1: a crime in the cells (the held on a cellmate) adds this
    /// share of its term to the sentence (`law::cell_case`; a Murder adds
    /// its whole term).
    pub cell_term_share: f32,
}

impl Default for CaseCfg {
    fn default() -> Self {
        CaseCfg {
            find_every_ticks: 5,
            lead_p: 0.25,
            tie_lead_p: 0.6,
            hue_reach: 12,
            tell_lawful_min: 0.3,
            tell_days: 2,
            fear_courage_max: 0.5,
            word_hours: 24,
            word_flat: 0.35,
            word_reach_ticks: 240,
            cell_term_share: 0.3,
        }
    }
}

impl CrimeCfg {
    fn default_answer_radius() -> u32 {
        96
    }

    fn default_open_air() -> Vec<BuildingKind> {
        vec![
            BuildingKind::Market,
            BuildingKind::Cemetery,
            BuildingKind::Lot,
            BuildingKind::FreightYard,
            BuildingKind::ConstructionYard,
        ]
    }

    fn default_witness_cap() -> u32 {
        4
    }
}

/// M13 D47: `[crime] sentence_days_ext`, matched by `law::sentence_ticks`
/// before it indexes `sentence_days` (as M12 D15 did Vagrancy).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SentenceDaysExt {
    pub grand_theft: u32,
    pub dealing: u32,
    pub abduction: u32,
    pub manslaughter: u32,
    /// M14 V18: the spec's 48 h.
    pub intrusion: u32,
    /// M14 V18: the spec's 120 h.
    pub data_theft: u32,
    /// M16b § 4: the spec's 48 h (`[law.sentence_hours] fraud`).
    pub fraud: u32,
    /// M16b § 4: the spec's 72 h.
    pub blackmail: u32,
    /// Jobs & lives phase 3 (§ 4): a dealer's weapon sale, a seized crate.
    pub arms_dealing: u32,
    pub smuggling: u32,
    /// Pressure pass round 7, W5 (`OPEN-237`): a parent's break-out of his
    /// own child from a work Camp (`Crime::CampEscape`).
    pub camp_escape: u32,
}

impl Default for SentenceDaysExt {
    fn default() -> Self {
        SentenceDaysExt {
            grand_theft: 6,
            dealing: 5,
            abduction: 20,
            manslaughter: 10,
            intrusion: 2,
            data_theft: 5,
            fraud: 2,
            blackmail: 3,
            arms_dealing: 6,
            smuggling: 8,
            camp_escape: 2,
        }
    }
}

/// The law as a faction (M9, `docs/M9_LAW.md`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LawCfg {
    /// Reports against gang members counted for pressure.
    pub window_days: u64,
    /// Reports in the window that read as full pressure (absolute; used
    /// when `crackdown_reports_per_1000` is 0).
    pub crackdown_reports: u32,
    /// M11 review: full pressure per 1,000 living residents (18: ~36 at 2,000);
    /// 0 keeps the absolute `crackdown_reports` (v1_profile, older configs).
    #[serde(default)]
    pub crackdown_reports_per_1000: f32,
    /// A jailbreak holds the Jail this long.
    pub garrison_days: u64,
    pub hysteresis: f32,
    pub shock_severity_rethink: f32,
    /// Crackdown and Garrison need this many guards on the payroll.
    pub min_guards: usize,
    /// Captain lawfulness at or above this refuses bribes.
    pub incorruptible: f32,
    /// A bribe costs `bribe_base + bribe_per_guard x guards`.
    pub bribe_base: i64,
    pub bribe_per_guard: i64,
    /// A bribe (taken or refused) holds for this long.
    pub bribe_days: u64,
    /// The gang pays when its bribe score reaches this.
    pub bribe_threshold: f32,
    /// A Crackdown keeps its target unless a challenger leads it by this many reports.
    #[serde(default = "default_target_margin")]
    pub target_margin: usize,
    /// M10: a patrol loop draws its Homes from this many nearest its Market
    /// (60: the whole v1 city).
    #[serde(default = "default_patrol_beat_homes")]
    pub patrol_beat_homes: usize,
    /// S3 (`OPEN-318`): a shift's patrol starts at one of this many stops
    /// of its loop nearest the guard, picked by a keyed draw on the guard and
    /// the shift (1: the nearest, every guard of a Station the same first
    /// leg; 0 reads as 1).
    #[serde(default)]
    pub patrol_first_pick: usize,
    /// S3 (`OPEN-318`): a guard setting out on a shift's first patrol leg
    /// from indoors musters up to this many ticks after the plan's start, a
    /// keyed draw on the guard and the shift (0: all out of the door at once).
    #[serde(default)]
    pub patrol_muster_ticks: u64,
    /// M10: a guard chases only warrants last seen within this many tiles
    /// (Manhattan). The default reaches across any map.
    #[serde(default = "default_pursuit_radius")]
    pub pursuit_radius: u32,
    /// M10 phase 5c: a guard is owed a shift on law duty for at least this
    /// share of it (or completed: five legs, a held Jail).
    #[serde(default = "default_shift_duty_share")]
    pub shift_duty_share: f32,
    pub posture_flat: PostureFlatCfg,
    // --- M12 phase 2: the law in districts (docs/M12_DISTRICTS.md § 2, plan D9-D15) ---
    /// Plan D9-D11: per-district allocation, beats, stances and Vagrancy. `false`
    /// (older configs, `v1_profile`) keeps the M11 routes and nothing else runs.
    #[serde(default)]
    pub district_beats: bool,
    #[serde(default = "LawCfg::d_alloc_base")]
    pub alloc_base: f32,
    #[serde(default = "LawCfg::d_alloc_crime")]
    pub alloc_crime: f32,
    /// M12 phase 5 calibration: the crime term is `alloc_crime × min(3,
    /// rate ÷ mean) ^ alloc_crime_exp` (1.0 = the phase 2 linear term).
    #[serde(default = "LawCfg::d_alloc_crime_exp")]
    pub alloc_crime_exp: f32,
    #[serde(default = "LawCfg::d_alloc_paid")]
    pub alloc_paid: f32,
    #[serde(default = "LawCfg::d_alloc_gang_landlord")]
    pub alloc_gang_landlord: f32,
    #[serde(default = "LawCfg::d_alloc_riot")]
    pub alloc_riot: f32,
    #[serde(default = "LawCfg::d_gang_landlord_homes")]
    pub gang_landlord_homes: usize,
    #[serde(default = "LawCfg::d_max_crackdowns")]
    pub max_crackdowns: usize,
    /// Rough sleepers in a district that read as full Sweep pressure.
    #[serde(default = "LawCfg::d_sweep_full")]
    pub sweep_full: u32,
    #[serde(default = "LawCfg::d_private_fill_coverage")]
    pub private_fill_coverage: f32,
    #[serde(default = "LawCfg::d_private_fill_weight")]
    pub private_fill_weight: f32,
    /// Per rough night at coverage 1.0.
    #[serde(default = "LawCfg::d_vagrancy_base")]
    pub vagrancy_base: f32,
    #[serde(default = "LawCfg::d_sweep_mult")]
    pub sweep_mult: f32,
    #[serde(default = "LawCfg::d_curfew_mult")]
    pub curfew_mult: f32,
    #[serde(default = "LawCfg::d_vagrancy_fine")]
    pub vagrancy_fine: i64,
    /// One night.
    #[serde(default = "LawCfg::d_vagrancy_sentence_ticks")]
    pub vagrancy_sentence_ticks: u64,
    #[serde(default)]
    pub stance_flat: StanceFlatCfg,
    // --- M16a (spec § 3 "the law's view", plan C28-C31): read only behind
    // `contracts::on`, by phase 3. Spec values as defaults.
    /// C28: a taker jailed within this many days of a fulfilled Hit or Beat is interrogated.
    #[serde(default = "LawCfg::d_interrogate_days")]
    pub interrogate_days: u32,
    /// C28: added to the interrogation move's bonus.
    #[serde(default)]
    pub interrogate_bias: f32,
    /// C29: a `Hired` rumour this sure names the buyer to the law.
    #[serde(default = "LawCfg::d_accessory_conf")]
    pub accessory_conf: f32,
    /// C29: the Conspiracy sentence is this × Murder's.
    #[serde(default = "LawCfg::d_accessory_mult")]
    pub accessory_mult: f32,
    /// C30: a Fixer's heat per fulfilled brokered Hit.
    #[serde(default = "LawCfg::d_heat_per_hit")]
    pub heat_per_hit: f32,
    /// C30: a Fixer's heat per interrogation naming it.
    #[serde(default = "LawCfg::d_heat_per_named")]
    pub heat_per_named: f32,
    /// C30: a Fixer's heat decay per day.
    #[serde(default = "LawCfg::d_heat_decay")]
    pub heat_decay: f32,
    /// C30: the office is busted at this heat.
    #[serde(default = "LawCfg::d_warrant_heat")]
    pub warrant_heat: f32,
    /// C30: days a busted office stays closed.
    #[serde(default = "LawCfg::d_close_days")]
    pub close_days: u32,
    /// C30, C31: heat a taken bribe or a guard on the take takes off.
    #[serde(default = "LawCfg::d_bribe_cool")]
    pub bribe_cool: f32,
    /// C21: a Fixer passes `Hired` to a regular at `fixer_talk × (1 − heat)`.
    #[serde(default = "LawCfg::d_fixer_talk")]
    pub fixer_talk: f32,
    /// C29: the accessory window after a fulfilment.
    #[serde(default = "LawCfg::d_accessory_days")]
    pub accessory_days: u32,
    /// C31: a city guard below this lawfulness may take a Guard record on a buyer.
    #[serde(default = "LawCfg::d_corrupt_lawfulness")]
    pub corrupt_lawfulness: f32,
    /// C30, C31: a Fixer owner at this heat bribes and may buy a guard.
    #[serde(default = "LawCfg::d_corrupt_heat")]
    pub corrupt_heat: f32,
    // --- Lives at work phase 7 (§ 5.2): district stations, under the
    // `[lives]` master (`[law] stations = false`). Off, every key below is
    // 0 and none is written into a save (`LawCfg::strip_stations`).
    /// Seed a Station in the districts far from the Precinct; guards post
    /// there and the escort books at the nearest with a free cell.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stations: bool,
    /// A district gets a Station only when its Lot's door is at least this
    /// many tiles (Manhattan) from the Precinct's door and from every
    /// Station already placed.
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub station_min_tiles: u32,
    /// A Station's cells: the held it books before an escort is sent on.
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub station_cells: u32,
    /// The van's hour, in ticks of the day: every held body whose sentence
    /// runs past the next van goes to the Precinct.
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub transfer_hour: u16,
    /// The share of a Station's guards on its desk a shift (the rest patrol).
    #[serde(default, skip_serializing_if = "is_zero_f32_cfg")]
    pub station_desk_share: f32,
}

impl LawCfg {
    fn d_interrogate_days() -> u32 {
        30
    }
    fn d_accessory_conf() -> f32 {
        0.5
    }
    fn d_accessory_mult() -> f32 {
        0.75
    }
    fn d_heat_per_hit() -> f32 {
        0.15
    }
    fn d_heat_per_named() -> f32 {
        0.3
    }
    fn d_heat_decay() -> f32 {
        0.02
    }
    fn d_warrant_heat() -> f32 {
        0.8
    }
    fn d_close_days() -> u32 {
        7
    }
    fn d_bribe_cool() -> f32 {
        0.3
    }
    fn d_fixer_talk() -> f32 {
        0.3
    }
    fn d_accessory_days() -> u32 {
        60
    }
    fn d_corrupt_lawfulness() -> f32 {
        0.3
    }
    fn d_corrupt_heat() -> f32 {
        0.3
    }
}

fn default_target_margin() -> usize {
    2
}

fn default_patrol_beat_homes() -> usize {
    60
}

fn default_shift_duty_share() -> f32 {
    0.5
}

fn default_pursuit_radius() -> u32 {
    512
}

impl LawCfg {
    /// Lives at work phase 7 (§ 5.2): the stations' keys off (0, not written).
    pub fn strip_stations(&mut self) {
        self.stations = false;
        self.station_min_tiles = 0;
        self.station_cells = 0;
        self.transfer_hour = 0;
        self.station_desk_share = 0.0;
    }

    /// The `[law]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> LawCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).law
    }

    fn d_alloc_base() -> f32 {
        1.0
    }
    fn d_alloc_crime() -> f32 {
        1.0
    }
    fn d_alloc_crime_exp() -> f32 {
        1.0
    }
    fn d_alloc_paid() -> f32 {
        0.5
    }
    fn d_alloc_gang_landlord() -> f32 {
        1.5
    }
    fn d_alloc_riot() -> f32 {
        3.0
    }
    fn d_gang_landlord_homes() -> usize {
        20
    }
    fn d_max_crackdowns() -> usize {
        2
    }
    fn d_sweep_full() -> u32 {
        10
    }
    fn d_private_fill_coverage() -> f32 {
        0.7
    }
    fn d_private_fill_weight() -> f32 {
        0.5
    }
    fn d_vagrancy_base() -> f32 {
        0.04
    }
    fn d_sweep_mult() -> f32 {
        3.0
    }
    fn d_curfew_mult() -> f32 {
        2.0
    }
    fn d_vagrancy_fine() -> i64 {
        3
    }
    fn d_vagrancy_sentence_ticks() -> u64 {
        600
    }
}

/// `[camp]` (addendum 19; plan E37-E42; placeholders).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CampCfg {
    /// E38: `Child.hunger_days` (consecutive unfed days; S6: never a death)
    /// at which the service takes a child.
    pub take_days: u8,
    /// E40: sim years; younger children are housed and fed, not worked.
    pub work_age: u32,
    /// E40: Parts per child-day at skill 0.
    pub camp_yield: f32,
    pub camp_skill_floor: f32,
    pub camp_skill_slope: f32,
    /// E40: farming (the production skill) per shift.
    pub camp_skill_gain: f32,
    /// E37: corps found a camp when every camp is this full.
    pub found_full: f32,
    /// E42: unfed days before the law closes a camp.
    pub close_days: u8,
    pub close_ban_days: u32,
    /// E42: the owner's standing per unfed child-day in the last 30 days.
    pub scandal_standing: f32,
    /// E41: the released adult's affinity to each parent.
    pub family_affinity: f32,
    /// E41: fed share at or above which the camp earns loyalty.
    pub loyal_share: f32,
    /// E41: fed share below which the released adult holds a grudge.
    pub grudge_share: f32,
    /// Round 4 T1 (OPEN-231, Dylan 2026-10-27: neglect warns first): the
    /// first `take_days` of unfed days bring the service to the door (a
    /// warning); neglect again inside this many days of it takes the child.
    /// 0: no warning (the take at the first neglect, the base). Round 5 U2
    /// (OPEN-231, Dylan 2026-10-28: a second warning inside 30 days takes):
    /// 30, was `warn_window_days` 14.
    pub warn_repeat_days: u8,
    /// Round 4 T1: the warning's grace: no take before this many days
    /// after it, whatever the pantry does.
    pub warn_grace_days: u8,
    /// Round 4 T1: a warned parent short of the grace's food money asks up
    /// to this many Friends at or above `warn_ask_affinity`, warmest first;
    /// one with coins above `warn_ask_keep` lends on `affinity × (1 −
    /// greed / 2)` (an edge debt).
    pub warn_ask_tries: u8,
    pub warn_ask_affinity: f32,
    pub warn_ask_keep: i64,
    /// Round 4 T1: a Camp child goes home once a parent's Home has held
    /// this many days of its children's food (`CHILD_FOOD_PER_DAY`, the
    /// child counted in) ...
    pub home_pantry_days: f32,
    /// ... for this many midnights running (home on a warning).
    pub let_home_days: u16,
    /// Round 4 T1 (the warden's day): a Camp child-day's chance of a try
    /// at the fence in the day, at a hashed hour inside `abscond_hours`
    /// (the night's `[service] p_camp_escape` stands beside it) ...
    pub p_abscond: f32,
    pub abscond_hours: [u8; 2],
    /// ... and a warden on duty's chance of catching the child and walking
    /// it back in (never a blow: the under-18 rule).
    pub p_abscond_caught: f32,
    /// Round 6 V1 (OPEN-231, a report only): the day's `ch_warned_old`
    /// counts the children out of a Camp on a warning older than this many
    /// days (a warning standing; the ladder never reads it).
    pub warn_stale_days: u8,
    /// Round 7 W1 (OPEN-243): a fed adult's Eat goal (the grocery run)
    /// scores this while his household's claim on the pantry is short of
    /// its children's food (`shelf::kids_short`), above `[life]
    /// grocery_score`: the run for the children's share goes ahead of an
    /// errand.
    pub kids_grocery_score: f32,
    /// ... and this on the service's warning (the household's purse first:
    /// the run that day, ahead of the adults' own wants; no bedtime cut).
    pub warned_grocery_score: f32,
    /// ... and that run starts down to this energy at home (`[life]
    /// grocery_energy_floor` is the errand's), in a day worker's small
    /// hours too.
    pub kids_grocery_energy_floor: f32,
}

impl Default for CampCfg {
    fn default() -> Self {
        CampCfg {
            take_days: 2,
            work_age: 10,
            camp_yield: 0.15,
            camp_skill_floor: 0.6,
            camp_skill_slope: 0.8,
            camp_skill_gain: 0.002,
            found_full: 0.8,
            close_days: 3,
            close_ban_days: 30,
            scandal_standing: -0.05,
            family_affinity: -0.2,
            loyal_share: 0.9,
            grudge_share: 0.7,
            warn_repeat_days: 30,
            warn_grace_days: 3,
            warn_ask_tries: 3,
            warn_ask_affinity: 0.3,
            warn_ask_keep: 10,
            home_pantry_days: 3.0,
            let_home_days: 7,
            p_abscond: 0.004,
            abscond_hours: [9, 17],
            p_abscond_caught: 0.75,
            warn_stale_days: 14,
            kids_grocery_score: 0.7,
            warned_grocery_score: 0.9,
            kids_grocery_energy_floor: 0.15,
        }
    }
}

/// `[governance]` (M16b phase 4; spec § 6). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GovernanceCfg {
    /// The phase 4 switch (off: no boards, tags or attention).
    pub enabled: bool,
    /// The roster names whose corps (and their copies) have boards.
    pub boards: Vec<String>,
    pub board_size: usize,
    pub vote_days: u32,
    pub campaign_days: u32,
    pub ambition_min: f32,
    pub vote_bribe: i64,
}

impl Default for GovernanceCfg {
    fn default() -> Self {
        GovernanceCfg {
            enabled: true,
            boards: vec!["Habitat".to_string(), "Nutrix".to_string()],
            board_size: 5,
            vote_days: 45,
            campaign_days: 7,
            ambition_min: 0.12,
            vote_bribe: 150,
        }
    }
}

/// `[tags]` (M16b phase 4; spec § 3's `[bounty]` tag and scanner keys and
/// the Patrol's `patrol_share`, kept together).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TagsCfg {
    pub enabled: bool,
    pub tag_price: [i64; 4],
    pub tag_conf: [f32; 4],
    pub tag_battery_days: u32,
    /// City-wide (per capita, up only).
    pub max_tags: usize,
    pub scanner_price: [i64; 4],
    pub scan_price: i64,
    pub scan_heat: f32,
    pub patrol_share: f32,
    /// Plan keys: the scanner a Clinic and a Security Office keep.
    pub clinic_scanner: u8,
    pub office_scanner: u8,
    /// Plan key: a scanning adult goes once in this many days.
    pub scan_days: u32,
    /// Plan key: the Patrol's reach around its spot.
    pub patrol_radius: u32,
}

impl Default for TagsCfg {
    fn default() -> Self {
        TagsCfg {
            enabled: true,
            tag_price: [0, 150, 400, 900],
            tag_conf: [0.0, 0.7, 0.85, 0.95],
            tag_battery_days: 10,
            max_tags: 32,
            scanner_price: [0, 300, 800, 2000],
            scan_price: 40,
            scan_heat: 0.4,
            patrol_share: 0.3,
            clinic_scanner: 1,
            office_scanner: 2,
            scan_days: 7,
            patrol_radius: 6,
        }
    }
}
