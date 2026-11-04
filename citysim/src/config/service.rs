use super::*;

/// Lives at work phase 1 `[service]` (docs/LIVES_AT_WORK.md § 1, § 2.1):
/// the encounter's sub-switches (one per phase; `civic`, `street` and
/// `tower` are the later phases') and numbers. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceCfg {
    /// Phase 1: venues, vice and the Clinic.
    pub venues: bool,
    /// Phase 3 (§ 2.2, § 3.5): the civic counterparts (teacher, camp
    /// warden, jailer, super), the near round, tied housing and the parent's
    /// answer to a taken child (`[service] civic = false`).
    pub civic: bool,
    /// Phase 4 (§ 2.3): the street and freight: the cab roll shared by both
    /// tiers and the driver's body, the parcel, the lift, the client, the
    /// pimp's round, the dealer's corner and regulars
    /// (`[service] street = false`; not Jobs & lives phase 2's `[jobs2]
    /// street` switch, the pitches and rounds themselves).
    pub street: bool,
    /// Phase 5 (§ 2.4, § 2.2's civic clerk, § 3.6's `Decided`): the
    /// exec's Meeting and decision, the reporter's beat and byline, the
    /// Fixer's walk-in and contract encounters, the owner's collection, the
    /// clerk named (`[service] tower = false`).
    pub tower: bool,
    /// § 2.1: the madam's share of her worker's tips at her Brothel (worker -> madam), of the shift's total at shift end (8d, OPEN-123).
    pub madam_cut: f32,
    /// § 2.1: an Assault inside a kept venue is an ejection on this × (the
    /// bouncer's fighting − the attacker's), the attacker put at the door.
    pub p_eject: f32,
    /// § 2.1: a Cook takes one meal from the NoodleBar's stock at shift end
    /// (every tier).
    pub staff_meal: bool,
    /// D5: a `Served` memory's salience (its mood weight is 0 regardless).
    pub served_salience: f32,
    // --- Phase 3 (civic), each left out of a save at its default so the
    // off save is byte-identical (`ServiceCfg::strip_civic`).
    /// § 2.2 (warden): a Camp child-day's chance of an escape (the child
    /// back to a parent's Home with room, else the street).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub p_camp_escape: f32,
    /// § 3.5: the buy-out's fee to the Camp's owner (`Flow::Buyout`).
    #[serde(skip_serializing_if = "is_zero_i64_cfg")]
    pub buyout_fee: i64,
    /// § 3.5: the Home's pantry must hold this many days of the child's
    /// food (`demography::CHILD_FOOD_PER_DAY`) for the buy-out.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub buyout_food_days: f32,
    /// § 3.5: a gang member's break-out needs this many of his gang alive
    /// and free.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub breakout_crew: u8,
    /// § 3.5: a parent of a Camp child with no `last_met` with it in this
    /// many days is counted neglecting (printed, never a camp-take term).
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub neglect_days: u16,
    /// § 2.2 (teacher): a taught School day adds `teacher_floor + (1 −
    /// teacher_floor) × knowledge` to `Child.edu_taught`.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub teacher_floor: f32,
    // --- Phase 4 (the street), each left out of a save at its default
    // (`ServiceCfg::strip_street`).
    /// § 2.3 (cab): a rider waits this long at the door for a driver's body;
    /// farther, the Garage's off-screen cab takes the ride (the leg timed).
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub cab_wait_ticks: u32,
    /// C9a: a driver's body booked this long ago and not yet at the door
    /// leaves the ride to the Garage's off-screen cab (the hourly pass);
    /// 0 waits for him all shift (C8b: a 364-min cab, the rider at his door).
    #[serde(default, skip_serializing_if = "is_zero_u32_cfg")]
    pub cab_no_show_ticks: u32,
    /// § 2.3 (street worker): a client is a body out walking within this
    /// many tiles of the pitch before the district's hashed draw.
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub client_tiles: u32,
    /// § 2.3 (pimp): a pitch is protected by a pimp's body within this many
    /// tiles (a Statistical pimp credited by the clock in pitch hours).
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub pimp_tiles: u32,
    /// § 2.3 (pimp): the share of his cut the pimp passes up to the gang.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub pimp_pass_up: f32,
    /// § 2.3 (pimp): a pimp absent from every pitch of his for this many
    /// days loses the district (`name_pimps`).
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub pimp_absent_days: u16,
    /// § 2.3 (loader): after a pilfer, the owner reports a Theft on this
    /// chance (a hash of the loader and the day).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub p_short_report: f32,
    /// § 2.3 (dealer): a lookout at the corner or in the Bar multiplies the
    /// Dealing witness roll by this.
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub witness_lookout_mult: f32,
    /// § 2.3 (cab): a `split` fare-duty trade's shifts under the street
    /// switch, one a driver in turn by id (the commute hours: the `split`
    /// day/night pair is on duty at neither). Empty: the `split` rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cab_shifts: Vec<(u16, u16)>,
    /// OPEN-111 (§ 14 phase 1, "Who is behind the counter"), Shadow V5 bug
    /// 1: the legacy venue roles whose staff cover the venue's trade hours,
    /// split at each venue between `[world] shift_day` and `[leisure]
    /// evening_shift` (the thinner first, the evening on a tie), instead of
    /// all on `[world] shift_day`. Read under `[service] venues`
    /// (`service::venue_shift`); empty, every legacy role's shift as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evening_staff: Vec<crate::components::Role>,
    // --- Phase 5 (the tower), each left out of a save at its default
    // (`ServiceCfg::strip_tower`).
    /// § 2.4 (exec): the graded staff present at the Hq an exec meets in a
    /// Meeting hour, at most.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub meeting_attendees: u8,
    /// § 2.4 (exec): the two best corp orders within this of each other are
    /// the exec's call (the picked one gains it).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub decision_margin: f32,
    /// § 2.4 (exec): the orders a brave exec leans to ...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expansive: Vec<crate::components::CorpOrder>,
    /// ... and those a loyal one leans to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub defensive: Vec<crate::components::CorpOrder>,
    /// § 2.4 (reporter): the scenes of yesterday's deeds a reporter walks a shift.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub beat_stops: u8,
    /// § 2.4 (reporter): every this many bylines the reporter's grade rises one step.
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub byline_promote: u16,
    /// § 2.4 (reporter): a witness is a body within this many tiles of the scene.
    #[serde(skip_serializing_if = "is_zero_u32_cfg")]
    pub witness_tiles: u32,
    // --- 8d (the gate's last rows), each left out of a save at its default.
    /// 8d (OPEN-126, Shadow V5 bug 17): a homeless worker moving into a
    /// Home (`ownership::rehouse`, the hunger fix's first-rent rule) must
    /// hold this many days of its rent and pays them up front to its owner
    /// (`Flow::Rent`), held as prepaid rent; the jobless keep
    /// `rehouse_coins_mult`. 0: the old rule (one day's rent, nothing paid).
    #[serde(skip_serializing_if = "is_zero_u16_cfg")]
    pub move_in_rent_days: u16,
    /// 8d (OPEN-129): a corp exec homed at `[life] exec_home_tier` (the
    /// Spire) whose Hq stands in another district is driven to work: the
    /// leg timed at the Car's speed, in a corp vehicle when the corp owns a
    /// road one, else the nearest Garage's off-screen cab, its fare paid by
    /// the corp (`streetwork::exec_ride`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub exec_ride: bool,
    /// Review fix 8 (doctrine 3): the phase's memory valences, griefs, the
    /// fare dodger's grudge, the break-out's thresholds and the pimp's round
    /// curves. Left out of a save at its defaults (the shipped values).
    #[serde(default, skip_serializing_if = "ServeTuneCfg::is_default")]
    pub tune: ServeTuneCfg,
}

/// `[service.tune]`: the numbers the service systems read beside their
/// rows (each a `Served` memory's valence unless named otherwise).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServeTuneCfg {
    /// § 2.1 (bouncer): a refusal at the door, the bouncer's side and the refused's.
    pub refused_bouncer: f32,
    pub refused: f32,
    /// § 2.1 (bouncer): an ejection, the bouncer's side and the attacker's.
    pub eject_bouncer: f32,
    pub ejected: f32,
    /// § 2.2 (super): the tenant in arrears served a notice.
    pub notice: f32,
    /// § 2.2 (jailer): each jailer on duty when the Jail is broken.
    pub jailbreak: f32,
    /// § 2.3 (cab): the driver's `Robbed` grudge on a rider who skipped his fare.
    pub fare_dodge_grudge: f32,
    /// § 3.5: a parent breaks his child out of a Camp alone when lawfulness
    /// is under `breakout_lawfulness` and courage at least `breakout_courage`.
    pub breakout_lawfulness: f32,
    pub breakout_courage: f32,
    /// § 2.3 (pimp): the round's GangWork curves `(m, b)` on energy and loyalty.
    pub pimp_energy: (f32, f32),
    pub pimp_loyalty: (f32, f32),
}

impl Default for ServeTuneCfg {
    fn default() -> Self {
        ServeTuneCfg {
            refused_bouncer: -0.1,
            refused: -0.3,
            eject_bouncer: -0.1,
            ejected: -0.4,
            notice: -0.3,
            jailbreak: -0.3,
            fare_dodge_grudge: 0.4,
            breakout_lawfulness: 0.3,
            breakout_courage: 0.6,
            pimp_energy: (0.6, 0.4),
            pimp_loyalty: (0.3, 0.7),
        }
    }
}

impl ServeTuneCfg {
    pub fn is_default(&self) -> bool {
        *self == ServeTuneCfg::default()
    }
}

pub(super) fn is_zero_u32_cfg(v: &u32) -> bool {
    *v == 0
}

pub(super) fn is_zero_f32_cfg(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_i64_cfg(v: &i64) -> bool {
    *v == 0
}

pub(super) fn is_zero_u16_cfg(v: &u16) -> bool {
    *v == 0
}

impl Default for ServiceCfg {
    fn default() -> Self {
        ServiceCfg {
            venues: false,
            civic: false,
            street: false,
            tower: false,
            madam_cut: 0.3,
            p_eject: 0.5,
            staff_meal: true,
            served_salience: 0.15,
            p_camp_escape: 0.0,
            buyout_fee: 0,
            buyout_food_days: 0.0,
            breakout_crew: 0,
            neglect_days: 0,
            teacher_floor: 0.0,
            cab_wait_ticks: 0,
            cab_no_show_ticks: 0,
            client_tiles: 0,
            pimp_tiles: 0,
            pimp_pass_up: 0.0,
            pimp_absent_days: 0,
            p_short_report: 0.0,
            witness_lookout_mult: 0.0,
            cab_shifts: Vec::new(),
            evening_staff: Vec::new(),
            meeting_attendees: 0,
            decision_margin: 0.0,
            expansive: Vec::new(),
            defensive: Vec::new(),
            beat_stops: 0,
            byline_promote: 0,
            witness_tiles: 0,
            move_in_rent_days: 0,
            exec_ride: false,
            tune: ServeTuneCfg::default(),
        }
    }
}

impl ServiceCfg {
    /// Not written into a save at its defaults (every switch off).
    pub fn is_default(&self) -> bool {
        *self == ServiceCfg::default()
    }

    /// Lives at work phase 3: the civic switch off and its keys at their
    /// defaults (the off switches; nothing reads them off).
    pub fn strip_civic(&mut self) {
        let d = ServiceCfg::default();
        self.civic = false;
        self.p_camp_escape = d.p_camp_escape;
        self.buyout_fee = d.buyout_fee;
        self.buyout_food_days = d.buyout_food_days;
        self.breakout_crew = d.breakout_crew;
        self.neglect_days = d.neglect_days;
        self.teacher_floor = d.teacher_floor;
    }

    /// Lives at work phase 4: the street switch off and its keys at their
    /// defaults (the off switches; nothing reads them off).
    pub fn strip_street(&mut self) {
        let d = ServiceCfg::default();
        self.street = false;
        self.cab_wait_ticks = d.cab_wait_ticks;
        self.cab_no_show_ticks = d.cab_no_show_ticks;
        self.client_tiles = d.client_tiles;
        self.pimp_tiles = d.pimp_tiles;
        self.pimp_pass_up = d.pimp_pass_up;
        self.pimp_absent_days = d.pimp_absent_days;
        self.p_short_report = d.p_short_report;
        self.witness_lookout_mult = d.witness_lookout_mult;
        self.cab_shifts = d.cab_shifts;
    }

    /// Lives at work phase 5: the tower switch off and its keys at their
    /// defaults (the off switches; nothing reads them off).
    pub fn strip_tower(&mut self) {
        let d = ServiceCfg::default();
        self.tower = false;
        self.meeting_attendees = d.meeting_attendees;
        self.decision_margin = d.decision_margin;
        self.expansive = d.expansive;
        self.defensive = d.defensive;
        self.beat_stops = d.beat_stops;
        self.byline_promote = d.byline_promote;
        self.witness_tiles = d.witness_tiles;
    }
}

/// Lives at work phase 1: an encounter's outcome a `[[serves]]` row's
/// `event_on` may name (§ 1.4: events only for the notable).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServeOutcome {
    Done,
    Refused,
    Ejected,
    Regular,
    /// Lives at work phase 3 (§ 2.2, super): a tenant in arrears served notice.
    Notice,
}

/// Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1.1): one `[[serves]]`
/// row. `key` is a `[[trades]]` id or a legacy `Role` name (resolved like
/// the names `Config::unresolved_names` checks).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServesCfg {
    pub key: String,
    /// What the trade serves (its queue's source: § 2).
    pub kind: crate::components::ServeKind,
    /// Encounters one server takes an hour.
    pub per_hour: u8,
    /// The encounter's share of the hour, minutes (the diary's reading).
    pub minutes: u8,
    /// The `Skills` field a meeting trains, by this much.
    pub skill: TradeSkill,
    pub skill_gain: f32,
    /// A served becomes a regular (an Acquaintance edge, `regulars_made`)
    /// at this many encounters within `regular_days` (0: never).
    pub regular_after: u8,
    pub regular_days: u8,
    /// The `Served` memory's valence.
    pub valence: f32,
    /// The outcomes that print an `EventKind::Served`.
    pub event_on: Vec<ServeOutcome>,
    /// Lives at work phase 3: a civic row (teacher, camp warden, jailer,
    /// super), live only with `[service] civic` and stripped from the
    /// config off (`Config::strip_civic`), so the off save matches main.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub civic: bool,
    /// Lives at work phase 4: a street row (cab driver, courier, loader,
    /// street worker, pimp, dealer, vendor, rickshaw, tinker), live only with
    /// `[service] street` and stripped off (`Config::strip_street`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub street: bool,
    /// Lives at work phase 5: a tower row (the exec, the reporter, the
    /// Fixer's staff, the forger, the civic clerk), live only with `[service]
    /// tower` and stripped off (`Config::strip_tower`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tower: bool,
}

impl Default for ServesCfg {
    fn default() -> Self {
        ServesCfg {
            key: String::new(),
            kind: crate::components::ServeKind::Visit,
            per_hour: 6,
            minutes: 10,
            skill: TradeSkill::None,
            skill_gain: 0.0,
            regular_after: 0,
            regular_days: 14,
            valence: 0.1,
            event_on: Vec::new(),
            civic: false,
            street: false,
            tower: false,
        }
    }
}
