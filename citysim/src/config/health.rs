use super::*;

/// Pressure pass round 2, R3 `[care]` (docs/PRESSURE_PASS.md § R3,
/// OPEN-216; `systems/care.rs`): the wounded and the sick. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CareCfg {
    // --- the downed: the people at hand, the call, the back room
    /// A stranger within this Chebyshev radius of a downed body may carry
    /// it to a doctor (the people at hand) ...
    pub help_radius: u32,
    /// ... the Rescue tie's term: this × (sociability + lawfulness) / 2.
    pub stranger_term: f32,
    /// A body going down wakes the `help_wake` nearest off-screen adults
    /// within `help_radius` (a body each while it bleeds: the street hears).
    pub help_wake: u32,
    /// The first helper binds the wound when he lifts the body: the bleed
    /// clock gets this many more minutes (once), so a far doctor is in reach.
    pub aid_min: u32,
    /// A city guard on shift within `[wounded] rescue_radius` carries anyone
    /// down (a guard on the beat), at this term.
    pub beat_term: f32,
    /// The downed calls one tied body (kin, a Friend, a loyal gang-mate, a
    /// colleague) who can reach it and a doctor before it bleeds out; again
    /// every this many minutes while nobody has come.
    pub call_every_min: u32,
    /// Pressure pass round 7, W3 (`OPEN-244`): a callee who has not come by
    /// the next call is passed over; with no tied body able, the call reaches
    /// the nearest stranger who can: a city guard on shift or the doctor's
    /// own staff on duty within this many tiles, or a passer-by within
    /// `help_radius` (0: tied bodies only, as before).
    pub call_radius: u32,
    /// Round 7 W3: the Rescue tie's term for a called stranger ...
    pub call_term: f32,
    /// ... and the flat the callee's Rescue carries (a colleague called three
    /// times by a bleeding 61-y went on with his shift).
    pub call_flat: f32,
    /// A back room stabilises a carried downed body for this (the nearest
    /// staffed doctor, a Clinic or a back room, is the carrier's).
    pub back_stabilise_price: i64,
    // --- the fee
    /// The doctor on the table takes this share of a Clinic's fee (a patch,
    /// a treatment) into his own wallet; the rest to the owner's till.
    pub doctor_cut: f32,
    // --- the short: the slate, the pawn, begging for it
    /// Care on the slate: the doctor treats a diagnosed patient (or patches
    /// a wound) whose pool falls short for this down, the rest on his own
    /// book (an edge debt: repaid from the wallet as any debt, a Debt Beat
    /// from a lawless doctor) ...
    pub slate_down: i64,
    /// ... marked up by this ...
    pub slate_rate: f32,
    /// ... while the patient owes this doctor less than this (barred past it).
    pub slate_cap: i64,
    /// A pawn: an owned deck, weapon or vehicle (not chrome, financed or
    /// stolen) handed to the doctor covers this × its value of the fee.
    pub pawn_mult: f32,
    /// Begging for it: a diagnosed patient short of the fee asks the warmest
    /// kin or Friend at or above this affinity ...
    pub ask_affinity: f32,
    /// ... once in this many days ...
    pub ask_days: u32,
    /// ... who lends what he holds above this, up to the shortfall (a debt
    /// between them).
    pub ask_keep: i64,
    /// Round 3 S4 (OPEN-216): the asks a night, warmest first (the Friends at
    /// or above `ask_affinity`), until the shortfall is covered; one line.
    pub ask_tries: u32,
    /// ... a Friend who turned him down is not asked again for this many days
    /// (one with nothing above `ask_keep` is asked again at the next ask).
    pub ask_refused_days: u32,
    // --- the sick worker's post
    /// A sick body's post is covered at midnight by a jobless adult living
    /// in the employer's district (a day hire to the end of the shift) when
    /// the owner's purse holds this many days of the role's wage; else the
    /// absence is written.
    pub cover_purse_days: i64,
    /// A cover is lawful enough to hire: lawfulness at or above this.
    pub cover_lawful: f32,
    /// M18 P3: a cover picked who is a body a human drives is offered the
    /// shift instead of hired; the offer stands this many hours.
    pub cover_offer_hours: u64,
    // --- round 5 U6 (OPEN-217): the clinic's day
    /// A patient goes to a doctor's body on duty with room first (the
    /// Clinic's off-screen colleagues, present by the clock, took the pair
    /// hash).
    pub clinic_body_first: bool,
    // --- round 6 V6 (OPEN-217): empty rooms beside full ones
    /// A Clinic is open for a patch-up with any of its patient staff on
    /// duty (an orderly on the night's split shift), not its Ripperdoc
    /// alone (a sale, an install, a treatment still wait for him).
    pub orderly_opens: bool,
    /// A patient goes to the nearest Clinic (or back room) whose doctor on
    /// duty has room this hour, else the nearest open (the nearest alone
    /// took every patient whatever the queue).
    pub clinic_spill: bool,
    /// A back room's doc on call is the pair hash of the patient among the
    /// free docs with room, not the first found.
    pub on_call_spread: bool,
    /// The off-screen chrome bought at midnight is booked for an hour from
    /// the first to the last of these (hours of the day, inclusive; the
    /// Ripperdoc's day shift), kept when the doctor is at the counter.
    pub book_hours: [u16; 2],
    /// The wash: an off-screen adult carrying this much toxicity or more ...
    pub wash_min_tox: f32,
    /// ... and holding `wash_price` above `wash_keep` books one on this a
    /// day (a hash), at the Clinic nearest his Home ...
    pub p_wash: f32,
    pub wash_price: i64,
    pub wash_keep: i64,
    /// ... which takes this share of his toxicity out. `p_wash` 0 off.
    pub wash_frac: f32,
    // --- round 7 W4 (OPEN-245): the install's doer
    /// The `[[serves]]` keys of a Clinic's (or back room's) patient staff who
    /// cut chrome: an install is done (named, paid) by one of them on duty,
    /// never the orderly at the triage. Empty: the patient queue's server.
    pub install_rows: Vec<String>,
    /// The doctor who cut it takes this share of the implant's value from
    /// the owner's till into his wallet (`Flow::Treatment`).
    pub install_cut: f32,
    // --- round 7 W6 (OPEN-217): the booked trade spreads by the book
    /// A booked visit (a wash, the off-screen chrome) goes to the Clinic
    /// with a Ripperdoc on staff and the fewest bookings today per doctor
    /// among those within this many ticks of travel more than the nearest
    /// from the patient's Home (the nearest on ties); 0: the nearest.
    pub book_slack_ticks: u64,
    /// ... weighing this many staffed Clinics nearest his Home (by tiles).
    pub book_cands: usize,
    /// Round 8 X6 (`OPEN-217`): a walk-in patch-up goes to the Clinic open
    /// for it with the fewest patients today (seen and booked) per doctor
    /// among the `book_cands` nearest within this many ticks of travel more
    /// than the nearest (the nearest on ties); 0: the nearest with room.
    pub walk_slack_ticks: u64,
}

impl Default for CareCfg {
    fn default() -> Self {
        CareCfg {
            help_radius: 6,
            stranger_term: 0.5,
            help_wake: 3,
            aid_min: 180,
            beat_term: 0.9,
            call_every_min: 30,
            call_radius: 40,
            call_term: 0.8,
            call_flat: 0.3,
            back_stabilise_price: 30,
            doctor_cut: 0.25,
            slate_down: 10,
            slate_rate: 0.3,
            slate_cap: 400,
            pawn_mult: 0.5,
            ask_affinity: 0.3,
            ask_days: 3,
            ask_keep: 20,
            ask_tries: 3,
            ask_refused_days: 7,
            cover_purse_days: 3,
            cover_lawful: 0.3,
            cover_offer_hours: 8,
            clinic_body_first: true,
            orderly_opens: true,
            clinic_spill: true,
            on_call_spread: true,
            book_hours: [9, 17],
            wash_min_tox: 0.4,
            p_wash: 0.05,
            wash_price: 8,
            wash_keep: 10,
            wash_frac: 0.5,
            install_rows: Vec::new(),
            install_cut: 0.0,
            book_slack_ticks: 0,
            book_cands: 6,
            walk_slack_ticks: 0,
        }
    }
}

/// M13 chrome (docs/M13_ASSETS.md § 3). Phase 1 carries the Kit-table keys
/// (plan "Kit tables"), the sanity cost and the used-sale fraction; phase 3
/// adds the rest of the spec's `[chrome]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromeCfg {
    /// Arms: `fighting + arms_fighting × tier`.
    pub arms_fighting: f32,
    /// Arms: `strength + arms_strength × tier`.
    pub arms_strength: f32,
    /// Legs: `walk_mult` per tier.
    pub legs_walk: Vec<f32>,
    /// Nerves: `reflex + nerves_reflex × tier`.
    pub nerves_reflex: f32,
    /// Eyes: `sight + eyes_sight × tier` tiles.
    pub eyes_sight: u8,
    /// Eyes: `stealth + eyes_stealth × tier`.
    pub eyes_stealth: f32,
    /// Skin: `armour = skin_armour × tier`.
    pub skin_armour: f32,
    /// Sanity load per installed implant, per tier.
    pub sanity_cost: Vec<f32>,
    /// A used asset sells at `used_frac × list × condition / 100` (plan D29).
    pub used_frac: f32,
    /// Plan D28: `security::contest` moves the odds this much per tier.
    pub contest_step: f32,
    /// Spec § 3: a Clinic or Garage buys a used asset at this share of its
    /// value (phase 2: a hunkering corp's fleet sale, D44).
    pub buyback_frac: f32,
    /// Phase 3 (spec § 3): the fee to install a gang's own implant, per tier.
    pub install_fee: Vec<i64>,
    /// Sanity lost at once on an install.
    pub install_shock: f32,
    /// Daily: sanity moves toward `1 − Kit.load` by at most this.
    pub sanity_drift: f32,
    /// Daily: an extra sanity loss per installed implant whose upkeep is unpaid.
    pub unmedicated_drift: f32,
    /// Below this sanity: `−edgy_mood` mood, `+edgy_fight` on the Fight flat.
    pub edgy: f32,
    pub edgy_mood: f32,
    pub edgy_fight: f32,
    /// Below this sanity an episode can start (daily roll).
    pub psycho: f32,
    /// `p_episode = episode_base × (psycho − sanity) ÷ psycho`.
    pub episode_base: f32,
    /// × on `p_episode` within `[stims] stim_hours` of a stim (phase 4 reads it).
    pub stim_episode_mult: f32,
    pub episode_hours: u32,
    /// An episode's `Attack` kills at `fight_death_p × this`.
    pub berserk_kill_mult: f32,
    /// A guard's contested arrest of an episode kills at this.
    pub psycho_kill: f32,
    pub therapy_price: i64,
    pub therapy_gain: f32,
    /// Harvest sees chrome at `Kit.visible ≥` this.
    pub harvest_min_visible: u8,
    /// Daily, while a gang holds Harvest: a Statistical target's roll
    /// `abduct_base × chrome_value ÷ 1000 × (2 − coverage)`.
    pub abduct_base: f32,
    /// A ripped live abductee dies with this chance.
    pub p_rip_kill: f32,
    /// Plan D35: the Loot goal sees unsettled bodies within this many tiles
    /// (Manhattan; a plan key the spec leaves unnamed).
    pub loot_reach: u32,
    /// Plan D44 (phase 3 deviation): a gang buys its members' Arms with the
    /// treasury at this (the spec's `gang_buy_floor` 600 is never reached
    /// before phase 4's dealing money; seed 42's gangs hold 0-600).
    pub gang_chrome_floor: i64,
    /// Plan D34 (phase 3 deviation): the Treat goal is considered below this
    /// sanity (1.0: any body that has lost some; a whole one has nothing to
    /// treat, else the Treat curve's floor sent the calm to Therapy whenever
    /// they idled).
    pub treat_below: f32,
    /// Round 5 U7 (`OPEN-237`): an heir who did not see a body stripped of
    /// coins learns it when the body is settled, an unnamed `Stripped` memory
    /// of this salience (the one in sight remembers the stripper at 0.7).
    pub strip_heir_salience: f32,
}

impl Default for ChromeCfg {
    fn default() -> Self {
        ChromeCfg::off()
    }
}

impl ChromeCfg {
    /// The spec's values (assets off reads none of them).
    pub fn off() -> ChromeCfg {
        ChromeCfg {
            arms_fighting: 0.1,
            arms_strength: 0.25,
            legs_walk: vec![0.85, 0.75, 0.65],
            nerves_reflex: 0.15,
            eyes_sight: 2,
            eyes_stealth: 0.05,
            skin_armour: 0.15,
            sanity_cost: vec![0.08, 0.15, 0.25],
            used_frac: 0.6,
            contest_step: 0.25,
            buyback_frac: 0.4,
            install_fee: vec![30, 80, 200],
            install_shock: 0.05,
            sanity_drift: 0.05,
            unmedicated_drift: 0.02,
            edgy: 0.5,
            edgy_mood: 0.2,
            edgy_fight: 0.1,
            psycho: 0.25,
            episode_base: 0.05,
            stim_episode_mult: 2.0,
            episode_hours: 3,
            berserk_kill_mult: 4.0,
            psycho_kill: 0.3,
            therapy_price: 60,
            therapy_gain: 0.15,
            harvest_min_visible: 3,
            abduct_base: 0.002,
            p_rip_kill: 0.5,
            loot_reach: 16,
            gang_chrome_floor: 300,
            treat_below: 1.0,
            strip_heir_salience: 0.5,
        }
    }
}

/// M13 § 4 stims (plan phase 4, D38-D40). Read only while `[assets]
/// enabled`; `off()` carries the plan's values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StimsCfg {
    /// Coins of precursors per dose cooked (`Flow::Import`); also a legal
    /// Market's restock cost a dose (D40).
    pub cook_cost: i64,
    /// Doses a gang cooks per member a day.
    pub cook_per_member: u32,
    /// A gang cooks its Hideout stock up to this.
    pub cook_target: u32,
    /// A dealer picks up this many doses; the gang deals only with this much in stock.
    pub deal_batch: u32,
    /// A dealer's dose sells at `deal_price × (0.8 + 0.4 × leader greed)`.
    pub deal_price: i64,
    /// Coins a dose the dealer keeps.
    pub dealer_cut: i64,
    /// A legal Market's dose sells at `stim_price × the owner's Food price_level`.
    pub stim_price: i64,
    /// Legal Markets restock Stims to this at midnight.
    pub stim_restock_floor: u32,
    pub stim_energy: f32,
    /// Mood bias for `stim_hours` after a dose.
    pub stim_mood: f32,
    pub stim_hours: u32,
    /// `addiction += addict_per_use × (1.5 − lawfulness)` a dose.
    pub addict_per_use: f32,
    /// A day with no dose lowers addiction by this.
    pub addiction_decay: f32,
    /// Hooked at `addiction ≥` this.
    pub hooked: f32,
    /// Hooked with no dose for this long: withdrawal.
    pub withdrawal_hours: u32,
    pub withdrawal_mood: f32,
    /// Energy decay × this in withdrawal.
    pub withdrawal_energy: f32,
    /// `StealFood` (and `StealVehicle`) cost − this in withdrawal.
    pub withdrawal_steal_bonus: f32,
    /// A Statistical agent's `p_steal` × this in withdrawal.
    pub withdrawal_steal_mult: f32,
    /// A dose taken at `addiction ≥ 0.8` kills with this chance.
    pub p_overdose: f32,
    /// Doses a hooked Statistical adult buys a day.
    pub uses_per_day: u32,
    pub detox_price: i64,
    /// Detox: `addiction × detox_mult`.
    pub detox_mult: f32,
    /// A hooked Statistical adult with the coins and the will detoxes with this chance a day.
    pub p_stat_detox: f32,
    /// The GetHigh goal's flat.
    pub get_high_flat: f32,
    /// Lives at work phase 4 (§ 2.3): a dealer stands a corner (a HangOut
    /// spot of the gang's most-held district) when the gang has no venue,
    /// instead of the nearest Bar. Read with `[service] street`; off in a
    /// save at false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deal_corner: bool,
}

impl Default for StimsCfg {
    fn default() -> Self {
        StimsCfg::off()
    }
}

impl StimsCfg {
    /// The plan's values (assets off reads none of them).
    pub fn off() -> StimsCfg {
        StimsCfg {
            cook_cost: 1,
            cook_per_member: 2,
            cook_target: 200,
            deal_batch: 10,
            deal_price: 5,
            dealer_cut: 1,
            stim_price: 6,
            stim_restock_floor: 300,
            stim_energy: 0.4,
            stim_mood: 0.3,
            stim_hours: 6,
            addict_per_use: 0.06,
            addiction_decay: 0.02,
            hooked: 0.5,
            withdrawal_hours: 24,
            withdrawal_mood: 0.4,
            withdrawal_energy: 1.3,
            withdrawal_steal_bonus: 3.0,
            withdrawal_steal_mult: 1.5,
            p_overdose: 0.002,
            uses_per_day: 2,
            detox_price: 80,
            detox_mult: 0.3,
            p_stat_detox: 0.05,
            deal_corner: false,
            get_high_flat: 0.0,
        }
    }
}

/// `[charity]` (spec § 4 verbatim plus the plan keys of E26-E29).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharityCfg {
    /// Missions at seed (the Sump first); 0 seeds none.
    pub seed: u32,
    /// A Mission serves the hungry poor whose door (Home, else tile) is within this.
    pub reach_tiles: u32,
    /// A meal costs the kitchen `wholesale + meal_markup` a unit.
    pub meal_markup: i64,
    /// A cot's ledger-only cost a night (the purse pays nobody).
    pub cot_price: i64,
    /// A clinic hour's cost (tier ≥ 1; no tier-0 Therapy target at HEAD: unused).
    pub clinic_price: i64,
    /// E28: `p = donate_base × (lawfulness + loyalty) ÷ 2 × (1 + creed)`.
    pub donate_base: f32,
    /// E28: the share of the surplus a donor gives.
    pub donate_frac: f32,
    /// E28, E30: a corp under Lobby gives this to its district's Mission.
    pub lobby_gift: i64,
    /// Spec: standing per 100 coins given; carried by the `Gave` deed's
    /// honour weight (E29), read by nothing directly.
    pub rep_gift: f32,
    /// E27 (plan key): served when hunger is below this.
    pub hunger_below: f32,
    /// E27 (plan key): the hourly service cap (doubled by a Volunteer on shift).
    pub meals_per_hour: u16,
    /// E28 (plan key): a donor keeps this many days of rent and meals.
    pub give_floor_days: i64,
    /// E28 (plan key): a gang under Expand with a front gives this a week.
    pub gang_gift: i64,
    /// E29 (plan key): a gift of at least this writes the `Gave` deed.
    pub rep_gift_min: i64,
    /// E26 (plan key): `Register`'s per-capita target of Missions is
    /// `population ÷ this` (the founding tie-break's denominator).
    pub residents_per_mission: u32,
    /// Round 5 U3 (OPEN-235): a starving adult (`[life] starving_hunger`)
    /// walks to a kitchen this far, in travel ticks (`distance::trip`: the
    /// walk, or the El for a body that would board it), from where he sets
    /// out; both tiers. Others keep `reach_tiles`.
    pub starving_reach_ticks: u64,
    /// Round 5 U3: the kitchen's pot, the bowls it cooks a day: the nightly
    /// restock buys the shelf units that make this many meals (at the diet's
    /// synth stretch), capped by `[buildings.mission] stock_cap`, the purse
    /// and the Market's shelf. When it is gone the line is turned away.
    pub servings_per_day: u32,
    /// Round 5 U3: the hour of the soup line off screen: the Statistical
    /// hungry poor, hungriest first, are served from the pots in reach.
    pub soup_hour: u16,
    /// Round 5 U3: a kitchen keeps this many days of its own pots; the rest
    /// of its purse goes, at midnight, to the kitchens whose pot ran dry
    /// (`charity::share_purses`).
    pub share_keep_days: i64,
    /// Round 5 U3: a district counting this many off-screen starving out of
    /// every kitchen's reach at the soup hour gets a soup line, paid from
    /// the richest kitchen's purse past its keep (`charity::found_far`; one
    /// a day). 0: none.
    pub found_far_min: u32,
    /// Round 5 U3: at most this many such soup lines open a day.
    pub found_far_per_day: u32,
}

impl Default for CharityCfg {
    fn default() -> Self {
        CharityCfg {
            seed: 1,
            reach_tiles: 40,
            meal_markup: 1,
            cot_price: 1,
            clinic_price: 6,
            donate_base: 0.02,
            donate_frac: 0.05,
            lobby_gift: 40,
            rep_gift: 0.02,
            hunger_below: 0.4,
            meals_per_hour: 10,
            give_floor_days: 14,
            gang_gift: 10,
            rep_gift_min: 20,
            residents_per_mission: 1000,
            starving_reach_ticks: 240,
            servings_per_day: 150,
            soup_hour: 12,
            share_keep_days: 7,
            found_far_min: 20,
            found_far_per_day: 3,
        }
    }
}

/// `[sickness]` (docs/HUNGER_SICKNESS.md § 8; starting values). Always on
/// since P10 (its `enabled` switch retired); `diet`, `bins` and `cancer`
/// are its parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SicknessCfg {
    /// § 1-2: synth and real meals, the meters, the debuffs.
    pub diet: bool,
    /// § 4: the bins and the dive.
    pub bins: bool,
    /// § 3: cancer and its treatment.
    pub cancer: bool,
    /// Synth meals a shelf unit is cut into (each at a `1 / stretch` price).
    pub synth_stretch: u32,
    /// Buy real at coins >= this many real meals (coordinator call on P2:
    /// 10 kept the employed on synth for good) ...
    pub real_meal_reserve: i64,
    /// ... or this malnourished with two real meals' coins.
    pub mal_seek_real: f32,
    /// An hour below this hunger is a hungry hour.
    pub mal_hungry_edge: f32,
    /// Malnutrition per fully hungry day.
    pub mal_hungry: f32,
    /// Malnutrition per day at an all-synth (or scraps) diet.
    pub mal_poor_diet: f32,
    /// A synth (or Mission) diet raises malnutrition only to here: run-down,
    /// not sick; hunger and bin scraps push past it (coordinator call on P2).
    pub synth_malnut_cap: f32,
    /// Malnutrition off per real meal (two a day at most).
    pub mal_real: f32,
    pub mal_onset: f32,
    /// Toxicity per synth or Mission meal.
    pub tox_synth: f32,
    /// Toxicity per bin meal.
    pub tox_dive: f32,
    /// The share of toxicity cleared a day (after P5: a flat clearance gave
    /// no resting level). The steady level of a diet of S synth meals a day
    /// is `tox_synth x S / tox_clear_frac`.
    pub tox_clear_frac: f32,
    /// World creation (and an arrival): an adult's toxicity approaches its
    /// diet's steady level by `1 - exp(-(age - 16) / tox_seed_years)`.
    pub tox_seed_years: f32,
    /// The seeded levels' spread: x `(1 - j/2 + j u)`, `u` a hash.
    pub tox_seed_jitter: f32,
    pub tox_onset: f32,
    pub mal_energy: f32,
    pub tox_energy: f32,
    pub cancer_energy: f32,
    pub mal_work: f32,
    pub cancer_work: f32,
    /// A day's sick chance per unit of malnutrition past the onset (scaled).
    pub sick_mal: f32,
    /// Sick days in the last 14 that cost the job (Dylan, decision 4).
    pub sick_fire_days: u32,
    pub mal_mood: f32,
    pub tox_mood: f32,
    pub cancer_mood: f32,
    /// The old-age hazard x (1 + this x (m + t)).
    pub frail_old: f32,
    /// The fight's skill term less this x (m' + c).
    pub frail_fight: f32,
    /// No conception at or above this malnutrition.
    pub fertile_mal_max: f32,
    /// The cancer's daily hazard: p = this x toxicity x age factor (Dylan,
    /// after P5: a stationary hazard; no dose term, which compounded).
    pub cancer_hazard: f32,
    /// The age factor `1 + max(0, age - cancer_age_from) / cancer_age_span`
    /// (sim years).
    pub cancer_age_from: f32,
    pub cancer_age_span: f32,
    pub cancer_growth: f32,
    pub cancer_symptom: f32,
    pub cancer_treat_price: i64,
    pub cure_stage: f32,
    pub remission: f32,
    pub treat_tox_mult: f32,
    pub p_stat_treat: f32,
    pub bin_cap: u16,
    pub bin_meals_per_unit: u16,
    pub venue_scrap: f32,
    /// Scrap meals into a Market's bin per shelf unit sold (retail waste).
    pub market_waste: f32,
    /// Scrap meals into a Home's bin per pantry meal eaten (household waste).
    pub home_scrap: f32,
    pub bin_rot: f32,
    pub dive_reach_tiles: u32,
    pub dive_satisfy: f32,
    pub dive_sick_p: f32,
    pub regular_days: u8,
    /// A held gang Home within this many tiles makes the bin the gang's.
    pub bin_turf_tiles: u32,
    pub bin_shake_p: f32,
    pub bin_toll: i64,
    /// Days at hunger 0 before a death by starvation.
    pub starve_grace_days: u64,
}

impl Default for SicknessCfg {
    fn default() -> Self {
        SicknessCfg {
            diet: true,
            bins: true,
            cancer: true,
            synth_stretch: 3,
            real_meal_reserve: 3,
            mal_seek_real: 0.5,
            mal_hungry_edge: 0.25,
            mal_hungry: 0.08,
            mal_poor_diet: 0.012,
            synth_malnut_cap: 0.45,
            mal_real: 0.02,
            mal_onset: 0.3,
            tox_synth: 0.005,
            tox_dive: 0.012,
            tox_clear_frac: 0.03,
            tox_seed_years: 8.0,
            tox_seed_jitter: 0.3,
            tox_onset: 0.4,
            mal_energy: 0.5,
            tox_energy: 0.3,
            cancer_energy: 1.0,
            mal_work: 0.4,
            cancer_work: 0.5,
            sick_mal: 0.3,
            sick_fire_days: 5,
            mal_mood: 0.2,
            tox_mood: 0.15,
            cancer_mood: 0.3,
            frail_old: 2.0,
            frail_fight: 0.2,
            fertile_mal_max: 0.5,
            cancer_hazard: 0.00017,
            cancer_age_from: 30.0,
            cancer_age_span: 10.0,
            cancer_growth: 0.015,
            cancer_symptom: 0.25,
            cancer_treat_price: 240,
            cure_stage: 0.6,
            remission: 0.4,
            treat_tox_mult: 0.5,
            p_stat_treat: 0.2,
            bin_cap: 40,
            bin_meals_per_unit: 2,
            venue_scrap: 0.3,
            market_waste: 0.2,
            home_scrap: 0.2,
            bin_rot: 0.5,
            dive_reach_tiles: 40,
            dive_satisfy: 0.6,
            dive_sick_p: 0.05,
            regular_days: 3,
            bin_turf_tiles: 15,
            bin_shake_p: 0.3,
            bin_toll: 1,
            starve_grace_days: 7,
        }
    }
}

/// `[wounded]` (M16b phase 3; spec § 5, with the plan keys marked). Every
/// value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WoundedCfg {
    /// The switch in flight (off: the phase-2 city).
    pub enabled: bool,
    /// A fight's death becomes a wound with `wound_share × (1 − armour)`.
    pub wound_share: f32,
    pub bleed_hours: u32,
    pub stabilise_price: i64,
    pub recover_days: u32,
    pub rescue_radius: u32,
    pub rescue_loyalty: f32,
    /// Spec `[lod] wounded_quota`: downed at once (per capita, up only).
    pub wounded_quota: usize,
    /// Plan key (the Statistical split of `p_killed`): the share of
    /// wounds a Full run saves; off screen an hour's killing survives as
    /// an assault with `wound_share × stat_saved`.
    pub stat_saved: f32,
    /// Plan key: the Rescue goal's flat (a Trauma crew's call adds
    /// `trauma_flat`).
    pub rescue_flat: f32,
    pub trauma_flat: f32,
    /// Review finding 6: a stabilised patient's daily meal (off a Market's
    /// shelf, as a captive's), paid by its carer, else itself.
    pub care_food: i64,
    /// Round 9 Y1 (`OPEN-249`): a stabilised patient under this hunger is
    /// fed within the hour (its carer, itself, its kin, else the clinic's
    /// owner pays `care_food`), not only at midnight.
    pub care_feed_hunger: f32,
}

impl Default for WoundedCfg {
    fn default() -> Self {
        WoundedCfg {
            enabled: true,
            wound_share: 0.6,
            bleed_hours: 3,
            stabilise_price: 80,
            recover_days: 3,
            rescue_radius: 12,
            rescue_loyalty: 0.5,
            wounded_quota: 12,
            stat_saved: 0.16,
            rescue_flat: 0.0,
            trauma_flat: 0.5,
            care_food: 3,
            care_feed_hunger: 0.25,
        }
    }
}

/// `[trauma]` (M16b phase 3; spec § 5 "Trauma Team").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraumaCfg {
    pub fee_weekly: i64,
    pub trauma_crew: u32,
    pub subscribe_coins: i64,
    pub subscribe_heat: f32,
    pub p_subscribe: f32,
    pub trauma_stat_save: f32,
}

impl Default for TraumaCfg {
    fn default() -> Self {
        TraumaCfg {
            fee_weekly: 35,
            trauma_crew: 2,
            subscribe_coins: 400,
            subscribe_heat: 0.3,
            p_subscribe: 0.2,
            trauma_stat_save: 0.5,
        }
    }
}
