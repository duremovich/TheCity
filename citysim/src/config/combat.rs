use super::*;

/// Jobs & lives phase 3 `[weapons]` (docs/JOBS_AND_LIVES.md § 4; Dylan's
/// answer 1): weapons as the first kind of the owned-item store, the
/// fight's tier term, Gunsmiths, armouries, dealers, crates, licences and
/// carry sightings, and the petty theft of carried and kept items. Live
/// only under `[jobs2] enabled`. Every number is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeaponsCfg {
    pub enabled: bool,
    // --- The fight.
    /// A raid's pairing strength and a Hunt's might, per tier.
    pub might_w: f32,
    /// Condition a fight takes off the winner's weapon.
    pub wear_per_fight: u8,
    /// An armed suspect contesting the cuffs: the death chance × this.
    pub armed_arrest_mult: f32,
    // --- The store.
    /// Weapon entities alive at most, per 1,000 residents (R4).
    pub weapons_per_1000: f32,
    // --- Gunsmiths.
    /// `[corps]` rows (Security) given a Gunsmith per copy at seed.
    pub gunsmith_rows: Vec<usize>,
    pub parts_per_weapon: u32,
    /// Weapons a gunsmith makes an on-duty hour, × (0.5 + knowledge).
    pub make_rate: f32,
    /// Weapons a Gunsmith keeps in stock at most.
    pub gunsmith_stock: u32,
    // --- The law and the guards.
    /// The tier the Precinct and the Security Offices buy.
    pub guard_tier: u8,
    /// The Precinct's target stock per city guard, and a Security
    /// Office's per private guard.
    pub stock_per_guard: f32,
    /// The Treasury's arms line: coins a day at most.
    pub arms_budget: i64,
    /// A licence from the Civic Clerk (`Flow::Licence`).
    pub licence_fee: i64,
    /// The concealment contest's step per tier.
    pub conceal_step: f32,
    /// `UnlicensedArms` reports a day at most, per 1,000 residents (R3).
    pub ua_per_1000_day: f32,
    /// Carry sightings are rolled this many times in a district cracking
    /// down on the carrier's gang.
    pub crackdown_rolls: u8,
    // --- Owners.
    /// The stable share of each class (`Corp`, `Street`, `Dreg`) who want
    /// a weapon of their own when they can afford one.
    pub want_p_by_class: [f32; 3],
    /// The chance a day an owner with no other reason carries it (else it
    /// stays at home), by class.
    pub carry_p_by_class: [f32; 3],
    /// Lawfulness under this is a reason to want (and carry) one.
    pub want_lawless: f32,
    /// A theft or an assault suffered within this many days is a reason.
    pub robbed_days: u32,
    /// Coins a buyer keeps after the purchase.
    pub coin_reserve: i64,
    // --- Gangs and dealers.
    /// A gang's armoury target per member (the dealer sells the surplus).
    pub armoury_per_member: f32,
    /// A dealer's price × the tier's list.
    pub dealer_markup: f32,
    /// The dealer's cut of each sale (`Flow::Arms`).
    pub dealer_cut: i64,
    // --- Smuggling.
    /// Weapons a crate holds at most, their tier, and the price of each
    /// abroad; a gang ships one while its armoury is under its target plus
    /// `deal_float` (its dealer's stock).
    pub crate_size: u32,
    pub crate_tier: u8,
    pub crate_unit: i64,
    pub deal_float: u32,
    /// Days between a gang's crates.
    pub crate_days: u32,
    /// A crate is paid only from the coins above this many days of the
    /// gang's stipends (the roster and one recruit).
    pub crate_reserve_days: u32,
    /// The minute of the day crates land.
    pub crate_hour: u16,
    pub p_seize: f32,
    /// Coins to a Loader on shift at the Yard, and the seizure chance ×
    /// `bribe_mult` when one takes it.
    pub bribe: i64,
    pub bribe_mult: f32,
    // --- Petty theft (Dylan's answer 1).
    /// A Market thief also lifts a carried item from a body within
    /// `petty_reach` on this chance (a concealment contest after).
    pub p_pickpocket: f32,
    pub petty_reach: u32,
    /// ... or empties the till of a stall in the Market's district of up to
    /// this many coins.
    pub stall_take_max: i64,
    /// Round 6 V3 (`OPEN-238`): ... the nearest stall to the thief, not one
    /// emptied in the last this many hours (its owner minds the till after;
    /// V14 `vendor_9648 d23`: ten Market thieves across the Sump emptied the
    /// district's lowest owner id's till in six hours).
    pub stall_rob_gap_hours: u32,
    /// Petty item thefts a day at most, per 1,000 residents (R3).
    pub petty_per_1000_day: f32,
    /// What a fence pays of an item's value.
    pub fence_frac: f32,
}

impl Default for WeaponsCfg {
    fn default() -> Self {
        WeaponsCfg {
            enabled: false,
            might_w: 0.15,
            wear_per_fight: 2,
            armed_arrest_mult: 1.5,
            weapons_per_1000: 100.0,
            gunsmith_rows: vec![7],
            parts_per_weapon: 3,
            make_rate: 0.15,
            gunsmith_stock: 12,
            guard_tier: 1,
            stock_per_guard: 0.5,
            arms_budget: 100,
            licence_fee: 60,
            conceal_step: 0.25,
            ua_per_1000_day: 1.0,
            crackdown_rolls: 2,
            want_p_by_class: [0.08, 0.05, 0.03],
            carry_p_by_class: [0.3, 0.6, 0.8],
            want_lawless: 0.25,
            robbed_days: 7,
            coin_reserve: 20,
            armoury_per_member: 0.3,
            dealer_markup: 1.5,
            dealer_cut: 10,
            crate_size: 4,
            crate_tier: 1,
            crate_unit: 40,
            deal_float: 6,
            crate_days: 3,
            crate_reserve_days: 7,
            crate_hour: 120,
            p_seize: 0.15,
            bribe: 15,
            bribe_mult: 0.5,
            p_pickpocket: 0.3,
            petty_reach: 3,
            stall_take_max: 10,
            stall_rob_gap_hours: 24,
            petty_per_1000_day: 2.0,
            fence_frac: 0.4,
        }
    }
}

/// L2 § 3 `[fviolence]` (phase 4): the ledger's victim side and the daily
/// pass that applies it to the Statistical tier. Every number a placeholder
/// (phase 5 calibrates; `fv_mult` is the first knob).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FviolenceCfg {
    /// Days of the cells' windows the rates read (at most `ledger::RATE_DAYS`).
    pub rate_days: usize,
    /// Body-days of evidence the prior is worth.
    pub prior_weight: f32,
    pub fv_mult: f32,
    pub day_cap: FvKindsCfg,
    pub prior: FvPriorCfg,
    /// L2 phase 5: a prior per victim class, as a multiplier per kind
    /// (Killed, Assaulted, Robbed, Abducted) on every source's prior.
    pub class_mult: FvClassCfg,
    /// The island pass phase 10 `[fviolence.hands]` (OPEN-185): a source
    /// strikes off screen only with its off-screen hands (`fviolence::daily`).
    /// `None` (not written into a save) with the key absent:
    /// the victim-side rolls limited only by `day_cap`, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hands: Option<FvHandsCfg>,
}

/// `[fviolence.hands]`: the actor side's prior, per member-day of a
/// faction's members under a source (an order or a vendetta), Killed,
/// Assaulted, Robbed, Abducted, worth `prior_days` member-days of evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvHandsCfg {
    pub prior: [f32; 4],
    pub prior_days: f32,
}

impl Default for FvHandsCfg {
    fn default() -> Self {
        FvHandsCfg { prior: [0.002, 0.02, 0.005, 0.001], prior_days: 30.0 }
    }
}

/// L2 phase 5 (calibration (b)): per `VictimClass`, the per-kind
/// multipliers of the source priors (1.0: the spec's priors as they are).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvClassCfg {
    pub civilian: [f32; 4],
    pub member: [f32; 4],
    pub watch: [f32; 4],
}

impl Default for FvClassCfg {
    fn default() -> Self {
        FvClassCfg { civilian: [1.0; 4], member: [1.0; 4], watch: [1.0; 4] }
    }
}

/// Per-kind day caps, city-wide (Killed, Assaulted, Robbed, Abducted).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvKindsCfg {
    pub killed: u32,
    pub assaulted: u32,
    pub robbed: u32,
    pub abducted: u32,
}

impl Default for FvKindsCfg {
    fn default() -> Self {
        FvKindsCfg { killed: 3, assaulted: 12, robbed: 20, abducted: 2 }
    }
}

impl FvKindsCfg {
    /// In the ledger's order: Killed, Assaulted, Robbed, Abducted.
    pub fn get(&self, k: usize) -> u32 {
        [self.killed, self.assaulted, self.robbed, self.abducted][k.min(3)]
    }
}

/// The per-source priors per agent-day, `[Killed, Assaulted, Robbed,
/// Abducted]`; `harvest_abducted` is `Order(Harvest)`'s Abducted prior.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FvPriorCfg {
    pub order: [f32; 4],
    pub vendetta: [f32; 4],
    pub riot: [f32; 4],
    pub episode: [f32; 4],
    pub harvest_abducted: f32,
}

impl Default for FvPriorCfg {
    fn default() -> Self {
        FvPriorCfg {
            order: [0.0003, 0.002, 0.003, 0.0],
            vendetta: [0.0004, 0.002, 0.0, 0.0],
            riot: [0.001, 0.01, 0.01, 0.0],
            episode: [0.0005, 0.003, 0.0, 0.0],
            harvest_abducted: 0.0004,
        }
    }
}

impl Default for FviolenceCfg {
    fn default() -> Self {
        FviolenceCfg {
            rate_days: 14,
            prior_weight: 48.0,
            fv_mult: 1.0,
            day_cap: FvKindsCfg::default(),
            prior: FvPriorCfg::default(),
            class_mult: FvClassCfg::default(),
            hands: None,
        }
    }
}

/// `[fixers] gun_lawfulness`: the illegal kinds' lawfulness ceilings (a
/// missing kind reads 1.0; M16b appends its kinds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GunLawfulnessCfg {
    pub hit: f32,
    pub beat: f32,
    /// M16b phase 2.
    pub abduct: f32,
    pub steal: f32,
    pub rescue: f32,
    /// M16b phase 4.
    pub tag: f32,
    pub vote: f32,
    /// M17 phase 3.
    pub retake: f32,
}

impl Default for GunLawfulnessCfg {
    fn default() -> Self {
        GunLawfulnessCfg { hit: 0.3, beat: 0.4, abduct: 0.3, steal: 0.4, rescue: 0.7, tag: 0.5, vote: 0.6, retake: 0.6 }
    }
}

impl GunLawfulnessCfg {
    pub fn get(&self, k: crate::contract::ContractKind) -> f32 {
        use crate::contract::ContractKind;
        match k {
            ContractKind::Hit => self.hit,
            ContractKind::Beat => self.beat,
            ContractKind::Abduct => self.abduct,
            ContractKind::Steal => self.steal,
            ContractKind::Rescue => self.rescue,
            ContractKind::Tag => self.tag,
            ContractKind::Vote => self.vote,
            ContractKind::Retake => self.retake,
            ContractKind::Guard
            | ContractKind::Locate
            | ContractKind::Persuade
            | ContractKind::Escort
            | ContractKind::Deliver
            | ContractKind::Spin
            | ContractKind::Trauma => 1.0,
        }
    }
}

/// `[hostage]` (M16b phase 2; spec § 4 "Hostages and the private prison",
/// with the plan keys marked). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostageCfg {
    /// The switch in flight (off: the phase-1 city).
    pub enabled: bool,
    pub hostage_days: u32,
    /// Coins a day the holder pays for a captive's meal.
    pub hostage_food: i64,
    pub p_hostage_kill: f32,
    /// Plan deviation (spec `[lod] captive_quota`): captives held at once,
    /// city-wide (per capita, up only).
    pub captive_quota: usize,
    /// Plan key: a ransom's coins.
    pub ransom: i64,
    /// Plan key (spec `found_cost.prison`): what a corp pays to build one.
    pub prison_cost: i64,
    /// Plan key (spec `upkeep.prison`): a Prison's daily upkeep.
    pub prison_upkeep: i64,
    /// Plan key: a Steal on a building takes this share of its owner's purse ...
    pub steal_share: f32,
    /// ... at most this.
    pub steal_max: i64,
    /// Plan key: a corpse unburied this long may be bought a Deliver.
    pub bury_hours: u32,
    /// Spec `[contracts] abduct_share`: a Harvest's Abduct price, × the chrome's value.
    pub abduct_share: f32,
}

impl Default for HostageCfg {
    fn default() -> Self {
        HostageCfg {
            enabled: true,
            hostage_days: 7,
            hostage_food: 3,
            p_hostage_kill: 0.3,
            captive_quota: 6,
            ransom: 200,
            prison_cost: 800,
            prison_upkeep: 20,
            steal_share: 0.2,
            steal_max: 300,
            bury_hours: 24,
            abduct_share: 0.3,
        }
    }
}

/// `[combat]` (the combat model C1, docs/COMBAT.md § 1-5; `systems/combat.rs`):
/// the synchronous beat loop (`combat::resolve_now`), wounds and the blood
/// meter, the three skills and the stances. Tables by severity are `[graze,
/// wound, grave]`; by stance `[all out, defensive, subdue, hold back]`; by
/// region `[head, torso, left arm, right arm, left leg, right leg]`.
/// Placeholders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CombatCfg {
    /// The most beats a fight runs (about 10 s each); at the end the side
    /// with the lower morale breaks.
    pub max_beats: u8,
    /// C3a: a live engagement (every party Full) runs this many beats a tick
    /// (`combat::step`, after exec).
    pub beats_per_tick: u8,
    /// C3a: a live duel's two within this many tiles (or in one building) at
    /// its start, one moved out of it, has left the fight (it fled).
    pub reach_tiles: u32,
    /// C3a: the ring log's length, in beats (the view reads it in C4).
    pub log_beats: usize,
    /// `p_hit = hit_base + attack_w x skill + accuracy x quality + stance -
    /// evade_w x evasion`, clamped to `[hit_min, hit_max]`.
    pub hit_base: f32,
    pub attack_w: f32,
    pub evade_w: f32,
    pub hit_min: f32,
    pub hit_max: f32,
    /// Kit reflex adds this x reflex to evasion (and the higher reflex acts
    /// first in a beat).
    pub reflex_evade: f32,
    /// The region a hit lands on (weights, by region).
    pub region_weights: [f32; 6],
    /// A melee hit's grave weight x `(1 + strength_grave x Kit.strength)`.
    pub strength_grave: f32,
    /// Quality by the weapon's tier (1-3): accuracy x it, the grave weight
    /// x it and the graze weight / it.
    pub quality: [f32; 3],
    /// A guard's vest: armour on the torso (Skin chrome covers every region
    /// at `Kit.armour`; the better of the two holds).
    pub vest_torso: f32,
    /// Armour x this is the chance a hit goes down a severity (a graze stopped).
    pub armour_w: f32,
    /// A head hit knocks the body out on this (by severity), x
    /// `blunt_ko_mult` from a blunt kind, plus an old head wound's term.
    pub ko_p: [f32; 3],
    pub blunt_ko_mult: f32,
    /// Morale: `morale_base + courage_w x (courage - 0.5) + stance_morale +
    /// the caller's crew bonus`; less `morale_hit` per wound taken, plus
    /// `morale_landed` per hit landed. Below `break_at` + the stance's
    /// `break_shift` the body breaks: it flees, else yields.
    pub morale_base: f32,
    pub courage_w: f32,
    pub morale_hit: [f32; 3],
    pub morale_landed: f32,
    pub stance_morale: [f32; 4],
    pub stance_attack: [f32; 4],
    pub stance_evade: [f32; 4],
    pub break_at: f32,
    pub break_shift: [f32; 4],
    /// A break flees on `(flee_p + flee_evade_w x evasion)` x the legs'
    /// effect; a failed flight yields when the other side takes yields.
    pub flee_p: f32,
    pub flee_evade_w: f32,
    /// The blood a fresh wound takes at once and an hour of bleeding takes
    /// (by severity); a body not bleeding gets `regen_hour` back.
    pub blood_hit: [f32; 3],
    pub bleed_hour: [f32; 3],
    pub regen_hour: f32,
    /// A bleeding Wound stops on this roll each hour (first aid comes in
    /// C2); a Graze never bleeds; a Grave bleeds until a Clinic stabilises it.
    pub stop_p_wound: f32,
    /// C1's stub of C2's healing: a wound is gone this many days after it
    /// was dealt (a Grave only once it has stopped bleeding).
    pub heal_days: [u32; 3],
    /// Effects (`health::effects`), by the worst wound's severity there: an
    /// arm x the attack skill (each arm), a leg x evasion and the flight
    /// (each leg) and the worse leg x the step cost (`Kit.walk_mult`), the
    /// head adds to a later knockout and x work.
    pub arm_mult: [f32; 3],
    pub leg_mult: [f32; 3],
    pub leg_walk: [f32; 3],
    pub head_ko: [f32; 3],
    pub head_work: [f32; 3],
    /// Growth: a melee hit landed `+melee_gain`, a shot `+shooting_gain`, a
    /// dodge `+evasion_gain` (replacing the flat +0.01 to the winner).
    pub melee_gain: f32,
    pub shooting_gain: f32,
    pub evasion_gain: f32,
    /// `combat::power`: the better of melee and (with a gun carried)
    /// shooting, plus this share of evasion.
    pub power_evasion: f32,
    /// Shooting's seed: a base by role (`Role` names; else a gang member
    /// `shooting_gang`, else `shooting_base`) x a keyed draw in `shooting_draw`.
    pub shooting_base: f32,
    pub shooting_gang: f32,
    pub shooting_posts: std::collections::BTreeMap<String, f32>,
    pub shooting_draw: [f32; 2],
    /// Evasion's seed: `U([world] skill_min, skill_max)` x the age curve (1
    /// to `evasion_peak` years, then down `evasion_decline` a year, to
    /// `evasion_floor`).
    pub evasion_peak: f32,
    pub evasion_decline: f32,
    pub evasion_floor: f32,
    /// C1's wrong-read (a): the fight's age term. Past `age_from` years the
    /// attack and evasion fall `age_fight` a year (to `age_fight_floor` x),
    /// morale `age_morale` a year, and a hit's grave weight on the old body
    /// rises `age_grave` a year.
    pub age_from: f32,
    pub age_fight: f32,
    pub age_fight_floor: f32,
    pub age_morale: f32,
    pub age_grave: f32,
    /// C1's wrong-read (b, d): a post's training. At midnight a body on a
    /// post (a `shooting_posts` role, a gang member) is brought up to its
    /// shooting seed (the role's base x its keyed draw); these posts (Role
    /// names) to these floors of melee and evasion.
    pub melee_posts: std::collections::BTreeMap<String, f32>,
    pub evasion_posts: std::collections::BTreeMap<String, f32>,
    /// A trained post's nerve: these roles' (Role names) morale + this in a
    /// fight (a guard making an arrest holds where a brawler breaks).
    pub morale_posts: std::collections::BTreeMap<String, f32>,
    /// C2 (docs/COMBAT.md § 2, wounds that last): a treated wound heals in
    /// this share of the days it had left.
    pub treated_heal_mult: f32,
    /// The Treat goal: an untreated wound of `treat_from` (a severity index)
    /// or worse sends the body to a Clinic (or the back room); the price is
    /// `[customers] patch_price` (`back_price`) x `treat_price_mult` by the
    /// worst; the goal's hurt term is at least `treat_urgency` by the worst.
    pub treat_from: usize,
    pub treat_price_mult: [f32; 3],
    pub treat_urgency: [f32; 3],
    /// The hours after a wound a body that could not get it seen to says so
    /// (the price, or no doctor in reach).
    pub untreated_note_hours: u32,
    /// First aid: a friend, kin or a gang-mate (an adult on its feet) within
    /// `aid_tiles` (or in the same building) stops a bleeding Wound at once;
    /// an aided Grave bleeds at `aid_grave_mult` (it needs a Clinic).
    pub aid_tiles: u32,
    pub aid_grave_mult: f32,
    /// Manual posts (Role names or `[[trades]]` ids): an arm or leg Wound or
    /// worse turns the worker away, the shift and its pay lost. A Grave head
    /// wound turns any worker away.
    pub manual_posts: Vec<String>,
    /// At the heal: a scar on `scar_p` by severity; a Grave left untreated
    /// heals badly (a maim on the region, a Wound for good) on `maim_p`, a
    /// treated one on `maim_p_treated`; an untreated Grave limb is lost
    /// instead on `rot_p`.
    pub scar_p: [f32; 3],
    pub maim_p: f32,
    pub maim_p_treated: f32,
    pub rot_p: f32,
    /// Off screen (`lod::stat_hit`): a beating writes `stat_hits` hits (a
    /// keyed count in the range) from the `stat_kind` row's damage table on
    /// `region_weights`; a killing survived (`wounded::stat_kill_p`'s split)
    /// adds one Grave from `stat_grave_kind`.
    pub stat_hits: [u32; 2],
    pub stat_kind: String,
    pub stat_grave_kind: String,
    /// C3b (docs/COMBAT.md "C3b as built"), numbers count: every standing
    /// member of a side strikes the other side's front each beat; the front
    /// set upon by several splits its defence: its evasion against the
    /// second and later attacks in a beat x `(1 - split_evade x (n - 1))`,
    /// at least 0.
    pub split_evade: f32,
    /// Melee bodies that can reach one body in a beat (its front among
    /// them); the rest wait their turn. Ranged helpers all shoot. Geometry,
    /// not a cap on the body set upon (it helps the lone body).
    pub crowd_reach: usize,
    /// Morale reads the head count: `+ numbers_morale x ln(own standing /
    /// theirs)` (1 against 3: -1.1 x this).
    pub numbers_morale: f32,
    /// Fatigue (0 fresh, 1 spent): `tire_beat` a beat fought, `tire_wound`
    /// a wound taken (by severity), x `(1 - tire_strength x Kit strength)`
    /// (at least 0.2); attack x `(1 - tire_attack x fatigue)`, evasion x
    /// `(1 - tire_evade x fatigue)`; `rest_hour` back an hour out of fights.
    /// A robot never tires.
    pub tire_beat: f32,
    pub tire_wound: [f32; 3],
    pub tire_strength: f32,
    pub tire_attack: f32,
    pub tire_evade: f32,
    pub rest_hour: f32,
    /// Joiners (§ 1): a Full adult within `join_tiles` with a reason (a
    /// gang-mate, kin, a friend at `join_bond` affinity, a guard on duty
    /// seeing an assault: the victim's side) joins on `join_base +
    /// join_courage_w x (courage - 0.5)` (+ `join_guard` on duty, +
    /// `join_kin` kin), a keyed roll per fight; at most `join_max` a fight.
    pub join_tiles: u32,
    pub join_bond: f32,
    pub join_base: f32,
    pub join_courage_w: f32,
    pub join_guard: f32,
    pub join_kin: f32,
    pub join_max: usize,
    /// A joiner joins no other fight for this many minutes (ticks).
    pub join_cooldown_min: u32,
    /// The flight holds: a body that broke and fled lies low this many
    /// hours (only Eat, Sleep, Flee, Idle and, hurt, Treat score).
    pub low_hours: u32,
    /// Chrome (§ 4) on the `Implant(kind)` tiers. Nerves: a Kerenzikov at
    /// any tier (a dodge charge at the first attack and again when first
    /// hurt, taken on `keren_p` by tier), a Sandevistan from `sandy_tier`
    /// (`sandy_actions` extra strikes a beat by tier for `sandy_beats`
    /// beats, then `sandy_cooldown_hours`, `sandy_sanity` sanity a use).
    pub keren_p: [f32; 3],
    pub sandy_tier: u8,
    pub sandy_actions: [u8; 3],
    pub sandy_beats: u8,
    pub sandy_cooldown_hours: u32,
    pub sandy_sanity: f32,
    /// Arms: from `mantis_tier` the blades (the `mantis_kind` row, at the
    /// implant's tier quality; never taken off the body, sheathed under a
    /// subdue stance) in place of the bare hand or a melee weapon; below it
    /// gorilla arms: the bare hand's grave weight x `gorilla_grave` and its
    /// lethality at least `gorilla_lethality`.
    pub mantis_tier: u8,
    pub mantis_kind: String,
    pub gorilla_grave: f32,
    pub gorilla_lethality: f32,
    /// A hit on a chrome limb (Arms, Legs) wounds no flesh: a Graze is
    /// turned, a Wound wears the implant `chrome_wear` condition, a Grave
    /// wrecks it (`assets::wreck`, the ripperdoc's repair).
    pub chrome_wear: u8,
    /// Robots in the beats: a Wound counts 1, a Grave `robot_wreck_hits`;
    /// at `robot_wreck_hits` the robot is wrecked.
    pub robot_wreck_hits: u8,
    /// The `[[weapon_kinds]]` row a robot fights with, by tier.
    pub robot_kinds: [String; 3],
    /// C2's leftovers: the pit's referee stops a bout at the first wound of
    /// this severity (index); the Treat goal's hurt term grows
    /// `treat_urgency_day` a day the worst wound goes unseen (to 1); the back
    /// room sees to wounds up to `back_room_max` (a Grave needs a Clinic).
    pub bout_stop: usize,
    pub treat_urgency_day: f32,
    pub back_room_max: usize,
    /// C4 (docs/COMBAT.md § 6): abstract cover (`combat::cover_at`), a ranged
    /// hit's chance x (1 - cover): the open street or a road `cover_open`, a
    /// doorway `cover_door`, a street tile `cover_wall` for each walled side,
    /// inside a building `cover_inside`, at most `cover_max`. Take cover
    /// moves the body to the best tile within `cover_tiles`.
    pub cover_open: f32,
    pub cover_door: f32,
    pub cover_wall: f32,
    pub cover_inside: f32,
    pub cover_max: f32,
    pub cover_tiles: u16,
    /// C5: the body the human drives decides when to break (its morale break
    /// is waived); under its break line it shakes: attack and evasion x (1 -
    /// `shake_attack` / `shake_evade` x how far under), at least `shake_floor`.
    pub shake_attack: f32,
    pub shake_evade: f32,
    pub shake_floor: f32,
    /// C5: a duel with the human's body runs to this many beats (not
    /// `max_beats`): the human ends a stand-off, then the lower morale gives way.
    pub human_max_beats: u8,
    /// C5 (`OPEN-295`): a knockout lasts `ko_minutes`, + `ko_head_minutes` by
    /// the worst head wound (graze, wound, grave): down and still, robbable.
    pub ko_minutes: u16,
    pub ko_head_minutes: [u16; 3],
    /// C5: the winner of a fight goes through a knocked-out loser's pockets
    /// when its lawfulness is under `ko_rob_lawful` (no guard, no arrest, no
    /// bout), taking `ko_rob_share` of the coins.
    pub ko_rob_lawful: f32,
    pub ko_rob_share: f32,
    /// C5: a flight whose refuge is the building it fled (or none) runs out to
    /// a street tile this far from the door.
    pub flee_tiles: u16,
    /// C5: a subdue stance goes for the knockout: any blow it lands puts the
    /// body down on this x (1 + the body's fatigue).
    pub subdue_ko_p: f32,
    /// C4: the odds on an Attack row are this many mock fights
    /// (`combat::odds`, the beat's rules without the world's writes).
    pub odds_trials: u32,
    /// C4: the fight HUD's morale hint: a body this close above its break
    /// line is wavering.
    pub hint_waver: f32,
}

impl Default for CombatCfg {
    fn default() -> Self {
        CombatCfg {
            max_beats: 10,
            beats_per_tick: 6,
            reach_tiles: 4,
            log_beats: 96,
            hit_base: 0.4,
            attack_w: 0.6,
            evade_w: 0.5,
            hit_min: 0.05,
            hit_max: 0.95,
            reflex_evade: 0.5,
            region_weights: [0.1, 0.4, 0.125, 0.125, 0.125, 0.125],
            strength_grave: 0.5,
            quality: [0.85, 1.0, 1.2],
            vest_torso: 0.4,
            armour_w: 1.0,
            ko_p: [0.05, 0.3, 0.6],
            blunt_ko_mult: 1.5,
            morale_base: 0.55,
            courage_w: 0.5,
            morale_hit: [0.08, 0.22, 0.4],
            morale_landed: 0.04,
            stance_morale: [0.15, 0.05, 0.15, -0.15],
            stance_attack: [0.1, -0.1, 0.0, -0.05],
            stance_evade: [-0.05, 0.15, 0.0, 0.05],
            break_at: 0.2,
            break_shift: [0.0, -0.1, 0.0, 0.15],
            flee_p: 0.55,
            flee_evade_w: 0.4,
            blood_hit: [0.0, 0.1, 0.35],
            bleed_hour: [0.0, 0.1, 0.3],
            regen_hour: 0.05,
            stop_p_wound: 0.35,
            heal_days: [1, 6, 20],
            arm_mult: [0.95, 0.75, 0.45],
            leg_mult: [0.95, 0.7, 0.4],
            leg_walk: [1.0, 1.4, 2.0],
            head_ko: [0.0, 0.1, 0.25],
            head_work: [1.0, 0.85, 0.6],
            melee_gain: 0.004,
            shooting_gain: 0.004,
            evasion_gain: 0.003,
            power_evasion: 0.25,
            shooting_base: 0.05,
            shooting_gang: 0.35,
            shooting_posts: std::collections::BTreeMap::from([
                ("Guard".to_string(), 0.55),
                ("Fighter".to_string(), 0.15),
            ]),
            shooting_draw: [0.4, 1.0],
            evasion_peak: 30.0,
            evasion_decline: 0.012,
            evasion_floor: 0.4,
            age_from: 45.0,
            age_fight: 0.015,
            age_fight_floor: 0.4,
            age_morale: 0.006,
            age_grave: 0.03,
            melee_posts: std::collections::BTreeMap::from([("Guard".to_string(), 0.35)]),
            evasion_posts: std::collections::BTreeMap::from([("Guard".to_string(), 0.3)]),
            morale_posts: std::collections::BTreeMap::from([("Guard".to_string(), 0.15)]),
            treated_heal_mult: 0.5,
            treat_from: 1,
            treat_price_mult: [1.0, 2.0, 6.0],
            treat_urgency: [0.2, 0.6, 1.0],
            untreated_note_hours: 12,
            aid_tiles: 2,
            aid_grave_mult: 0.5,
            manual_posts: Vec::new(),
            scar_p: [0.0, 0.3, 1.0],
            maim_p: 0.5,
            maim_p_treated: 0.05,
            rot_p: 0.15,
            stat_hits: [1, 3],
            stat_kind: "unarmed".to_string(),
            stat_grave_kind: "knife".to_string(),
            split_evade: 0.2,
            crowd_reach: 3,
            numbers_morale: 0.15,
            tire_beat: 0.025,
            tire_wound: [0.02, 0.08, 0.2],
            tire_strength: 0.5,
            tire_attack: 0.5,
            tire_evade: 0.6,
            rest_hour: 0.5,
            join_tiles: 6,
            join_bond: 0.3,
            join_base: 0.35,
            join_courage_w: 0.8,
            join_guard: 0.4,
            join_kin: 0.2,
            join_max: 6,
            join_cooldown_min: 30,
            low_hours: 6,
            keren_p: [0.5, 0.65, 0.8],
            sandy_tier: 2,
            sandy_actions: [1, 1, 2],
            sandy_beats: 3,
            sandy_cooldown_hours: 2,
            sandy_sanity: 0.02,
            mantis_tier: 2,
            mantis_kind: "mantis".to_string(),
            gorilla_grave: 2.5,
            gorilla_lethality: 0.3,
            chrome_wear: 25,
            robot_wreck_hits: 3,
            robot_kinds: ["club".to_string(), "club".to_string(), "pistol".to_string()],
            bout_stop: 1,
            treat_urgency_day: 0.15,
            back_room_max: 1,
            cover_open: 0.0,
            cover_door: 0.3,
            cover_wall: 0.15,
            cover_inside: 0.3,
            cover_max: 0.5,
            cover_tiles: 2,
            shake_attack: 0.8,
            shake_evade: 0.8,
            shake_floor: 0.35,
            human_max_beats: 30,
            ko_minutes: 20,
            ko_head_minutes: [0, 20, 60],
            ko_rob_lawful: 0.4,
            ko_rob_share: 1.0,
            flee_tiles: 10,
            subdue_ko_p: 0.08,
            odds_trials: 200,
            hint_waver: 0.12,
        }
    }
}

/// A weapon kind's family: which skill it is used with.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeaponFamily {
    #[default]
    Melee,
    Ranged,
}

/// A `[[weapon_kinds]]` row (docs/COMBAT.md § 4). The tier stays as
/// quality (`[combat] quality`). `tiers` and `classes` map the stock: a
/// weapon of a tier owned by a class (`law`, `gang`, `civil`) takes a kind
/// among the rows listing both (a keyed pick at the spawn).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeaponKindCfg {
    pub id: String,
    pub family: WeaponFamily,
    /// Added to the attack roll (x quality).
    pub accuracy: f32,
    /// Weights over `[graze, wound, grave]` for a hit.
    pub damage: [f32; 3],
    /// The death chance on a Grave head or torso wound.
    pub lethality: f32,
    /// A blunt kind knocks out more (`[combat] blunt_ko_mult`); a subdue
    /// stance strikes with the first blunt row when armed.
    pub blunt: bool,
    /// The beats a defender takes to ready it (the attacker comes drawn).
    pub draw: u8,
    /// Its term in the concealment contest (`weapons::notice_p`), in place
    /// of the tier's `tier - 1`.
    pub conceal: f32,
    pub tiers: Vec<u8>,
    pub classes: Vec<String>,
    /// C3b: a Grave limb hit from it (edged or ranged) takes the limb on
    /// this (C2's flat `[combat] sever_p`).
    pub sever_p: f32,
}

type KindRow<'a> = (&'a str, WeaponFamily, f32, [f32; 3], f32, bool, u8, f32, &'a [u8], &'a [&'a str]);

/// The shipped `[[weapon_kinds]]` (a config without the rows).
pub fn default_weapon_kinds() -> Vec<WeaponKindCfg> {
    use WeaponFamily::{Melee, Ranged};
    let rows: [KindRow; 10] = [
        ("unarmed", Melee, 0.0, [0.75, 0.22, 0.03], 0.08, true, 0, 0.0, &[], &[]),
        ("knife", Melee, 0.05, [0.4, 0.42, 0.18], 0.35, false, 1, 0.0, &[1], &["gang", "civil"]),
        ("blade", Melee, 0.05, [0.3, 0.42, 0.28], 0.45, false, 1, 1.0, &[2], &["gang"]),
        ("club", Melee, 0.05, [0.5, 0.4, 0.1], 0.15, true, 1, 0.5, &[1], &["law", "gang", "civil"]),
        ("pistol", Ranged, 0.12, [0.3, 0.42, 0.28], 0.5, false, 1, 0.5, &[1, 2], &["law", "gang", "civil"]),
        ("smg", Ranged, 0.12, [0.25, 0.42, 0.33], 0.55, false, 1, 1.5, &[2, 3], &["law", "gang"]),
        ("shotgun", Ranged, 0.15, [0.2, 0.4, 0.4], 0.65, false, 2, 2.0, &[2, 3], &["law", "civil", "gang"]),
        ("rifle", Ranged, 0.12, [0.2, 0.38, 0.42], 0.7, false, 2, 2.5, &[3], &["law", "gang"]),
        ("sniper", Ranged, 0.0, [0.1, 0.3, 0.6], 0.85, false, 3, 3.0, &[], &[]),
        ("mantis", Melee, 0.1, [0.2, 0.42, 0.38], 0.5, false, 0, 3.0, &[], &[]),
    ];
    rows.iter()
        .map(|&(id, family, accuracy, damage, lethality, blunt, draw, conceal, tiers, classes)| WeaponKindCfg {
            id: id.to_string(),
            family,
            accuracy,
            damage,
            lethality,
            blunt,
            draw,
            conceal,
            tiers: tiers.to_vec(),
            classes: classes.iter().map(|c| c.to_string()).collect(),
            sever_p: match id {
                "knife" | "smg" => 0.1,
                "blade" => 0.25,
                "pistol" => 0.08,
                "shotgun" => 0.2,
                "rifle" => 0.15,
                "sniper" | "mantis" => 0.3,
                _ => 0.0,
            },
        })
        .collect()
}
