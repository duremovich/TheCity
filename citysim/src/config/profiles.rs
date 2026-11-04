use super::*;

impl Config {
    /// The v1 city (96 x 64, 300 residents) on top of the current tuning:
    /// only the scale keys are put back. Unit tests that hand-build small
    /// worlds use it; nothing else may load `map_v1.txt` (M10 D23).
    pub fn v1_profile(mut self) -> Config {
        self.world.map = "map_v1.txt".to_string();
        self.world.population = 300;
        // Scale S4: the v1 map is its own reference (nothing is fitted to it).
        self.scale = ScaleCfg::off();
        // Transit: the v1 map has no lines (a unit test builds its own).
        self.transit = TransitCfg::default();
        // V8 (`OPEN-256`): the hand-built unit worlds keep round 9's cells
        // (every term served, no bail, no crowding); a test reading the
        // cells' week sets its own `[cells]`.
        self.cells = None;
        self.world.jobs = JobsCfg { farmer: 24, guard: 10, clerk: 3, bartender: 2, gravedigger: 1, sanitation: 0 };
        self.world.market_initial = 600;
        self.world.warehouse_initial = 1500;
        self.world.treasury_initial = 5000;
        self.buildings.jail.capacity = 16;
        self.buildings.farm.capacity = 12;
        self.buildings.hideout.capacity = 12;
        self.buildings.warehouse.stock_cap = 3000;
        self.levers.guard_count = 10;
        self.levers.immigration_per_week = 2;
        self.gangs.max_members = 20;
        // M10 raised these for the 256 x 192 map and two 60-member gangs.
        self.gangs.recruit_on_promise = 5;
        self.gangs.raid_gather_hours = 3;
        self.economy.restock_floor = 400;
        self.economy.restock_batch = 200;
        self.lod.max_coarse = 100;
        // Not a D23 key: the v1 city had no pursuit limit.
        self.law.pursuit_radius = 512;
        // The M11 review's per-capita pressure: the v1 city keeps its absolute 6.
        self.law.crackdown_reports_per_1000 = 0.0;
        // M11 D9: a city-owned v1 city, no rent, no corps, equal wallets.
        self.rent.base = [0, 0, 0];
        self.corps = CorpsCfg::none();
        self.world.coins_by_tier = [1.0, 1.0, 1.0];
        self.classes.couple_immigration = false;
        self.classes.dreg_emigrate_days = 0;
        // M11 phase 5 recalibrated the 2,000 city's dole, tax, school meals and
        // prices; the v1 gates and unit tests keep the v1 economy.
        self.levers.dole_per_day = 3;
        self.levers.tax_rate = 0.05;
        self.economy.price_tenths = false;
        self.rent.rehouse_wait_days = 0;
        // M12 D1: the v1 city is one district.
        self.districts = DistrictsCfg::single();
        // M12 D46: the M11 law and binder.
        self.law.district_beats = false;
        self.bind.same_zone_weight = 1.0;
        self.bind.district_coverage = false;
        // M12 D46: no litter, no Hotels, derelicts or squats, nobody sweeps.
        self.litter = LitterCfg::off();
        self.street = StreetCfg::off();
        self.levers.sanitation_count = 0;
        // M12 D46 (phase 4): no riots or crossfire, the M11 gangs, no district
        // unrest terms, and no hoard tilt.
        self.riots = RiotsCfg::off();
        self.gangs.m11_behaviour();
        self.classes.rent_burden_w = 0.0;
        self.classes.curfew_fear = 0.0;
        self.corps.hoard_tilt = 0.0;
        // M13 D50: no assets, so the v1, M8 and M9 unit tests run unchanged.
        self.assets = AssetsCfg::off();
        self.vehicles = VehiclesCfg::off();
        self.shop = ShopCfg::off();
        self.stims = StimsCfg::off();
        self.robots = RobotsCfg::off();
        // L1b's food price is the 2,000 city's (the v1 economy prices at 3).
        // Since 2026-10-09 the life pass, the plane, the word, the living
        // city, contracts and the World market run here too: their off
        // switches are retired. P10 (docs/P10_POLICY.md): the v1 city starts
        // under the dole city's policy (the levy and the safety net) but for
        // its school meals; hunger as sickness is always on (the street,
        // and with it the shanties, is cut above); `[life] violence_fixes`
        // follows the shipped default.
        self.economy.price_base = 3.0;
        self.policy = PolicyCfg { school_meals: false, ..PolicyCfg::dole_city() };
        self
    }

    /// The flip (Dylan, 2026-10-10): the shipped city runs wages from
    /// revenue with no safety net. P10 (docs/P10_POLICY.md): this is the old
    /// default as a policy preset, the dole city, which most unit-test files
    /// pin: the city starts under the levy (the wage schedule and the corp
    /// upkeep levy) with the whole safety net enacted (`PolicyCfg::dole_city`).
    /// Hunger as sickness and the shanties run here too (their switches
    /// retired with P10). The housing catch-up (`[life]
    /// housing_catchup_days`, 2026-10-11) is off: its housing growth is the
    /// flat P5h rule (the pre-roll reads the market wage law and no net).
    /// No `[company_housing]` (the island pass phase 3a) and no
    /// `[commute_pressure]` (phase 4).
    pub fn dole_city(mut self) -> Config {
        self.policy = PolicyCfg::dole_city();
        // Transit phase 3: the old default had no El.
        self.transit.enabled = false;
        self.life.housing_catchup_days = 0.0;
        // The island pass phase 3a: no company housing (the old default;
        // with no El its walked commutes would seed blocks on the Lots the
        // pinned tests read).
        self.company_housing = None;
        // The island pass phase 4: no commute pressure (the old default's
        // relocation, which the pinned tests read).
        self.commute_pressure = None;
        // V8 (`OPEN-256`): the old default's cells (every term served: the
        // pinned tests book a thief with coins and read his sentence).
        self.cells = None;
        self
    }

    /// The Full city size the shipped planner budgets serve
    /// (`calibration_city` scales them past it).
    pub const PLAN_BUDGET_REF_AGENTS: u32 = 500;

    /// The city `calibrate` measures and the LOD parity test replays: the
    /// shipped city at `n` residents (`scaled_to`; both run the shipped
    /// population since the recalibration, see below) with only the cuts
    /// whose effect the Statistical tier models elsewhere. Gangless (the
    /// table never modelled gang actions, D26; off screen a gang's violence
    /// is `fviolence`'s ledger, L2 phase 4) and a full Warehouse (M10 phase
    /// 5b). Full farmers work about half the shift hours a Statistical
    /// farmer is credited with, so an all-Full city runs a food deficit the
    /// real, mostly Statistical city never sees: at 500 residents it emptied
    /// its Warehouse by day 24, the price reached 16, and the table learned
    /// famine theft as everyday theft.
    ///
    /// The recalibration (2026-10-09, `recal`; ported with hunger as
    /// sickness P5 onto the fixed planner): the table's rows condition on
    /// phase, lawfulness and hunger only, so every off-screen agent rolls the
    /// calibration city's average hour. That city was corp-less, rent-free,
    /// with equal wallets, no derelicts and 500 residents on the 2,000 map:
    /// on the flipped config about half its adults stayed jobless and broke
    /// while the shipped city is ~93 % employed (p_killed 5.7x, p_steal 3x),
    /// and at 500 residents a quarter of the police patrolled the same
    /// streets. The corps (their jobs, private guards, Markets and Blocks),
    /// rent, the tiered opening wallets, the class coupling and the seeded
    /// derelicts are the shipped city's now; three pooled seeds average one
    /// seed's landlords out.
    pub fn calibration_city(self, n: u32) -> Config {
        let mut c = self.scaled_to(n);
        c.gangs.max_members = 0;
        c.world.warehouse_initial = c.buildings.warehouse.stock_cap;
        // M12 D48 (phase 4): riots off, no splits, no crossfire (off screen
        // a riot is `fviolence`'s source).
        c.riots = RiotsCfg::off();
        c.gangs.split_base = 0.0;
        // M13 D46/D50: no assets. `lod::stat_probs` layers a kitted agent's
        // Kit and a stim withdrawal over the bare row, so the row must not
        // already hold them.
        c.assets = AssetsCfg::off();
        c.vehicles = VehiclesCfg::off();
        c.shop = ShopCfg::off();
        c.stims = StimsCfg::off();
        c.robots = RobotsCfg::off();
        // 2026-10-09 (the off switches retired): the life pass, the Virt
        // plane, the word, the living city, contracts and the World market
        // run here as in the shipped city. The starting policy (`[policy]`,
        // P10; the wage law and the safety net) follows the loaded config
        // too, as `[life] violence_fixes` does (the flip-readiness round: the
        // table and the parity test model the off-screen hours of the city
        // the config ships). `v1_profile` starts under the dole city's.
        // Hunger as sickness P5h (a harness change): the planner budgets were
        // tuned for a 500-agent Full city; the all-Full calibration and
        // parity cities scale them by n / 500 (never below the shipped
        // values). At 2,000 on budget 12 the queue held ~480 and 12% of
        // shift time was a Work goal waiting on a plan (1.1% at 4x).
        let scale = (f64::from(n) / f64::from(Self::PLAN_BUDGET_REF_AGENTS)).max(1.0);
        c.brain.plan_budget_per_tick = (c.brain.plan_budget_per_tick as f64 * scale).ceil() as usize;
        c.brain.plan_expansion_budget_per_tick =
            (c.brain.plan_expansion_budget_per_tick as f64 * scale).ceil() as usize;
        c
    }

    /// The same city at `n` residents: jobs, opening stocks, the Treasury, the
    /// guard and immigration levers and the gang cap scale by `n / population`
    /// (M10 D25; `calibrate` and the parity test run 500 on the 2,000 map).
    /// The per-Market restock floor and batch and `price_ref_stock` scale too,
    /// so a scaled Market's smaller shelf prices like the full city's.
    pub fn scaled_to(mut self, n: u32) -> Config {
        let from = self.world.population.max(1);
        let f = f64::from(n) / f64::from(from);
        let scale = |v: u32| (f64::from(v) * f).round() as u32;
        let job = |v: u32| if v == 0 { 0 } else { scale(v).max(1) };
        let j = &mut self.world.jobs;
        *j = JobsCfg {
            farmer: job(j.farmer),
            guard: job(j.guard),
            clerk: job(j.clerk),
            bartender: job(j.bartender),
            gravedigger: job(j.gravedigger),
            sanitation: job(j.sanitation),
        };
        self.world.population = n;
        self.world.market_initial = scale(self.world.market_initial);
        self.world.warehouse_initial = scale(self.world.warehouse_initial);
        self.world.treasury_initial = (self.world.treasury_initial as f64 * f).round() as i64;
        self.levers.guard_count = (f64::from(self.levers.guard_count) * f).round().clamp(0.0, 65535.0) as u16;
        self.levers.immigration_per_week =
            (f64::from(self.levers.immigration_per_week) * f).round().clamp(0.0, 65535.0) as u16;
        self.levers.sanitation_count = (f64::from(self.levers.sanitation_count) * f).round().clamp(0.0, 65535.0) as u16;
        self.gangs.max_members = (self.gangs.max_members as f64 * f).round() as usize;
        self.economy.restock_floor = scale(self.economy.restock_floor);
        self.economy.restock_batch = scale(self.economy.restock_batch);
        self.economy.price_ref_stock *= f;
        // M11: corp treasuries, Block holdings and the Bar owners' purse
        // scale; Farms, Markets and Offices do not (every one exists at any n).
        let c = &mut self.corps;
        for t in &mut c.treasury_initial {
            *t = (*t as f64 * f).round() as i64;
        }
        for b in &mut c.blocks {
            *b = (f64::from(*b) * f).round() as u32;
        }
        c.bar_owner_coins = (c.bar_owner_coins as f64 * f).round() as i64;
        // Flip readiness: the no-net Treasury's coin keys (read only with
        // the market wage law, P10) scale too. Unscaled, a
        // 300-resident city's Treasury sat under the 2,000 city's band, the
        // civic budget's target read 0 and the first midnight dismissed the
        // police (the district beats emptied a day later).
        let t = &mut self.treasury;
        t.treasury_initial = (t.treasury_initial as f64 * f).round() as i64;
        t.till_initial = (t.till_initial as f64 * f).round() as i64;
        for b in &mut t.band {
            *b = (*b as f64 * f).round() as i64;
        }
        t.sanitation_max = (f64::from(t.sanitation_max) * f).round().clamp(0.0, 65535.0) as u16;
        // Scale S4 (docs/SCALE_20K.md § S4): the city-wide counts, caps and
        // budgets that assumed 2,000 residents scale per capita, up only (a
        // city under 2,000 keeps the 2,000 city's value, as before S4: the
        // small test cities were calibrated on it).
        let up = f.max(1.0);
        let up_usize = |v: usize| (v as f64 * up).round() as usize;
        let up_u32 = |v: u32| (f64::from(v) * up).round() as u32;
        let up_i64 = |v: i64| (v as f64 * up).round() as i64;
        // The bodied tiers per capita (and the private guards' quota inside
        // Coarse): the forced classes, a gang's quota and the private guards,
        // grow with the city, a gang recruits only through its bodies, and
        // the Statistical tier is calibrated against a Full share of the
        // city. Measured on the generated 20k map (30 days; the 2,000 city
        // on day 30: gang members 46 and arrests 373 per 1,000, employed
        // 51-53 %): both at 50 / 150, gang members 0.8 and arrests 46 per
        // 1,000 (the gangs die without bodies) at 435-784 ticks/s; Coarse per
        // capita 28-30 and 160-220, employed 45-47 % at 117-235 ticks/s; with
        // Full per capita too (seeds 42-43, the S4 city) employed 48.0-48.3 %
        // vs 46.9-47.6 % at 114-125 ticks/s vs 125-128. A body's goal scoring
        // reads city-wide lists (Unwind's venues, Squat's derelicts), so the
        // bodies, not the residents, cost the run.
        let bodies = (self.lod.max_full + self.lod.max_coarse).max(1) as f64;
        self.lod.max_full = up_usize(self.lod.max_full);
        self.lod.max_coarse = up_usize(self.lod.max_coarse);
        self.lod.private_quota = up_usize(self.lod.private_quota);
        // R6b (OPEN-217): the live contract parties' and the Fixer owners'
        // bodies grow with the city, as `max_missions` does: unscaled, 24
        // bodies held ~8 runs city-wide at 20k against 120 run slots, and a
        // taken hit or locate queued to its deadline (20k seed 42 x 30: 238
        // of 261 live takes QUEUED, Hits done 4; V10's guns never hunted).
        self.lod.contract_quota = up_usize(self.lod.contract_quota);
        self.lod.fixer_quota = up_usize(self.lod.fixer_quota);
        if let Some(v) = self.verbs.as_mut() {
            v.hq_builds_per_day = up_u32(v.hq_builds_per_day);
        }
        // The planner's and the paths' per-tick budgets serve the bodies.
        let k = (self.lod.max_full + self.lod.max_coarse) as f64 / bodies;
        let b = &mut self.brain;
        b.plan_budget_per_tick = (b.plan_budget_per_tick as f64 * k).ceil() as usize;
        b.plan_expansion_budget_per_tick = (b.plan_expansion_budget_per_tick as f64 * k).ceil() as usize;
        b.path_budget_per_tick = (b.path_budget_per_tick as f64 * k).ceil() as usize;
        // Migration (a week's arrivals and the beds that read as room).
        let d = &mut self.demography;
        d.migrants_max = up_u32(d.migrants_max);
        d.beds_ref *= up as f32;
        d.offers_max = up_u32(d.offers_max);
        // City-wide caps on live records and daily events.
        self.hunt.max_hunts = up_usize(self.hunt.max_hunts);
        self.contracts.max_open = up_usize(self.contracts.max_open);
        self.missions.max_missions = up_usize(self.missions.max_missions);
        let h = &mut self.hitmarket;
        h.max_open = up_usize(h.max_open);
        h.posts_per_day = up_usize(h.posts_per_day);
        h.corp_year_cap = up_u32(h.corp_year_cap);
        // M16b: the standing coercions' cap; phase 2: the captives'.
        self.leverage.max_coercions = up_usize(self.leverage.max_coercions);
        self.hostage.captive_quota = up_usize(self.hostage.captive_quota);
        // Phase 3: the downed.
        self.wounded.wounded_quota = up_usize(self.wounded.wounded_quota);
        // Phase 4: the tags city-wide.
        self.tags.max_tags = up_usize(self.tags.max_tags);
        let fv = &mut self.fviolence.day_cap;
        *fv = FvKindsCfg {
            killed: up_u32(fv.killed),
            assaulted: up_u32(fv.assaulted),
            robbed: up_u32(fv.robbed),
            abducted: up_u32(fv.abducted),
        };
        // The island pass phase 10 (OPEN-185): `vendetta_norm` is the
        // grudge weight one faction's members hold on another's that reads
        // as a full feud; a faction's members (and so the grudges between
        // two factions) grow with the city, so the norm is per 2,000
        // residents (with the phase's `[fviolence.hands]`; absolute before).
        if self.fviolence.hands.is_some() {
            self.grudges.vendetta_norm *= up as f32;
        }
        self.life.relocate_per_day = up_usize(self.life.relocate_per_day);
        // The outside's demand: the World buys per resident (money enters at
        // the 2,000 city's per-capita rate) and its account refills to match.
        // M17 phase 1: the Ambient book's caps are per resident (the
        // simulated businesses are sized for the simulated city).
        let oa = &mut self.outside.ambient;
        oa.food_cap = up_u32(oa.food_cap);
        oa.visit_cap = up_u32(oa.visit_cap);
        // M17 phase 3: the bodies per capita (a parent's Outsiders, the
        // State's guards; the brand's own branches scale its money).
        let op = &mut self.outside.parents;
        op.outsiders_cap = up_u32(op.outsiders_cap);
        op.state_guards_max = (f64::from(op.state_guards_max) * up).round().clamp(0.0, 65535.0) as u16;
        op.state_guards_step = (f64::from(op.state_guards_step) * up).round().clamp(1.0, 65535.0) as u16;
        op.heat_murders *= up as f32;
        // M17 phase 5: the promoted cast living at most, per capita.
        self.outside.abroad.promoted_max = up_u32(self.outside.abroad.promoted_max);
        let wm = &mut self.world_market;
        wm.treasury_ref = up_i64(wm.treasury_ref);
        wm.food.cap = up_u32(wm.food.cap);
        wm.parts.cap = up_u32(wm.parts.cap);
        wm.data.cap = up_u32(wm.data.cap);
        // Jobs & lives phase 1 (R2): the appended books per capita, as Data's.
        wm.rations.cap = up_u32(wm.rations.cap);
        wm.braindance.cap = up_u32(wm.braindance.cap);
        wm.organs.cap = up_u32(wm.organs.cap);
        // The civic three's caps per capita (police and Sanitation's own above).
        let j2 = &mut self.jobs2;
        j2.clerks_max = (f64::from(j2.clerks_max) * up).round().clamp(0.0, 65535.0) as u16;
        j2.depot_hands_max = (f64::from(j2.depot_hands_max) * up).round().clamp(0.0, 65535.0) as u16;
        // Phase 2: the street's caps per capita (pitches a district, sweeps a day).
        let per_d = |v: u16| (f64::from(v) * up).round().clamp(0.0, 65535.0) as u16;
        j2.pitches_per_district = per_d(j2.pitches_per_district);
        j2.sweeps_per_day = per_d(j2.sweeps_per_day);
        for c in &mut wm.cap_wages {
            *c = up_u32(*c);
        }
        self.export.treasury_ref = up_i64(self.export.treasury_ref);
        // The Treasury's other coin keys (the L2 band, the public works, the
        // derelict floor), as `[treasury]` above.
        for b in &mut self.budget.band {
            *b = up_i64(*b);
        }
        self.budget.works_max = (f64::from(self.budget.works_max) * up).round().clamp(0.0, 65535.0) as u16;
        self.street.city_absorb_floor = up_i64(self.street.city_absorb_floor);
        // The opening businesses dealt per resident (the Hotels, the venues
        // and Fabs, the Missions); the name-keyed rows deal `World::seed_copies()`.
        self.street.seed_hotels = up_usize(self.street.seed_hotels);
        self.street.seed_derelict_blocks = up_usize(self.street.seed_derelict_blocks);
        let v = &mut self.jobs.seed_venues;
        *v = SeedVenuesCfg {
            club: up_u32(v.club),
            arcade: up_u32(v.arcade),
            noodle_bar: up_u32(v.noodle_bar),
            fight_pit: up_u32(v.fight_pit),
            den: up_u32(v.den),
            lounge: up_u32(v.lounge),
            fab: up_u32(v.fab),
        };
        self.charity.seed = up_u32(self.charity.seed);
        // The civic headcounts' daily cut (the reconcilers' daily moves are
        // `per_capita_count`).
        self.treasury.civic_pace = (f64::from(self.treasury.civic_pace) * up).round().clamp(0.0, 255.0) as u8;
        self
    }

    /// Scale S4: residents over the reference city's (`[scale] population`).
    pub fn per_capita(&self) -> f64 {
        f64::from(self.world.population) / f64::from(self.scale.population.max(1))
    }

    /// Scale S4: `round(per_capita)`, at least 1: the population's count of
    /// reference cities. The role-seeding passes read the roster's copies
    /// instead (`World::seed_copies`, review finding 7).
    pub fn copies(&self) -> usize {
        (self.per_capita().round() as usize).max(1)
    }

    /// Scale S4: a city-wide count in code that assumed 2,000 residents (the
    /// guard and Sanitation reconcilers' five hires or dismissals a day, the
    /// law's report rings), per capita, up only.
    pub fn per_capita_count(&self, at_2000: usize) -> usize {
        (at_2000 as f64 * self.per_capita().max(1.0)).round() as usize
    }

    /// Scale S4: a `PlayerCommand` headcount's ceiling (`SetGuardCount`,
    /// `SetSanitation`, `SetImmigrationPerWeek`), per capita, up only.
    pub fn lever_ceiling(&self, at_2000: u16) -> u16 {
        (f64::from(at_2000) * self.per_capita().max(1.0)).round().clamp(0.0, 65535.0) as u16
    }

    /// Scale S4 (`World::new`, after the map's districts and lines are in):
    /// the keys held per building, per gang, per corp or per district,
    /// fitted to the map's counts against `[scale]`'s (every ratio is 1 on
    /// the shipped map, and ratios under 1 read 1). `scaled_to` scaled the
    /// city totals by population; here they are spread over the map's
    /// holders:
    /// - per Market: the opening shelf, the restock floor and batch and
    ///   `price_ref_stock` (the double scaling: a 20k city's ten times the
    ///   Markets each held ten times the shelf);
    /// - per gang (one per Hideout): `max_members`, and `max_gangs` grows
    ///   with the Hideouts;
    /// - the corps: the roster dealt once per reference city's employers
    ///   (Farms, Markets and Security Offices) and re-dealt so every one is
    ///   owned in the shipped proportion ([`CorpsCfg::replicate`]); the
    ///   agent Bar owners and their purse per Bar;
    /// - the one-per-city buildings (the Jail, the Hall, the Recycler, the
    ///   Reserve Depot): seats and stock per capita;
    /// - per district: the riot crowd and its bodies, the gang-landlord and
    ///   sweep thresholds by residents per district; the concurrent riots and
    ///   crackdowns by districts;
    /// - per Lot: the shanty ceiling.
    ///
    /// Once per config (`[scale] fitted`): a fitted config fits to itself.
    pub fn fit_to_map(&mut self, map: &crate::map::Map) {
        if !self.scale.enabled || self.scale.fitted {
            return;
        }
        self.scale.fitted = true;
        let s = self.scale.clone();
        let mut count: std::collections::BTreeMap<BuildingKind, u32> = Default::default();
        for b in &map.buildings {
            *count.entry(b.kind).or_insert(0) += 1;
        }
        let n = |k: BuildingKind| count.get(&k).copied().unwrap_or(0);
        let ratio = |have: u32, reference: u32| (f64::from(have) / f64::from(reference.max(1))).max(1.0);
        let r_market = ratio(n(BuildingKind::Market), s.markets);
        let r_bar = ratio(n(BuildingKind::Bar), s.bars);
        let r_hideout = ratio(n(BuildingKind::Hideout), s.hideouts);
        let r_lot = ratio(n(BuildingKind::Lot), s.lots);
        let districts = self.districts.names.len().max(1) as u32;
        let r_districts = ratio(districts, s.districts);
        let up = self.per_capita().max(1.0);
        // Residents per district against the reference city's.
        let r_dsize = (up / (f64::from(districts) / f64::from(s.districts.max(1)))).max(1.0);
        // Per Market.
        let w = &mut self.world;
        w.market_initial = (f64::from(w.market_initial) / r_market).round() as u32;
        let e = &mut self.economy;
        e.restock_floor = (f64::from(e.restock_floor) / r_market).round() as u32;
        e.restock_batch = (f64::from(e.restock_batch) / r_market).round() as u32;
        e.price_ref_stock /= r_market;
        // Per gang.
        let g = &mut self.gangs;
        g.max_members = (g.max_members as f64 / r_hideout).round() as usize;
        g.max_gangs = (g.max_gangs as f64 * r_hideout).round() as usize;
        // The corps: the roster once per reference city's employers.
        let employers = |a: u32, b: u32, c: u32| f64::from(a + b + c);
        let r_emp = employers(n(BuildingKind::Farm), n(BuildingKind::Market), n(BuildingKind::SecurityOffice))
            / employers(s.farms, s.markets, s.offices).max(1.0);
        // More corps, not bigger ones (measured on the generated 20k map,
        // seeds 42-44, 60 days, day 60: the roster dealt ten times read
        // employed 47.7-49.4 %, homeless 246-260 per 1,000; the nine rows
        // owning ten times the buildings, with the per-corp coin keys
        // (`hoard_heat`, `lobby_min_treasury`, the estate caps, the outside
        // treasuries, `corp_raid_cap`) ten times over, 45.1-46.9 % and
        // 278-284). Copies keep every per-corp key as tuned, an exec per
        // ~2,000 residents and a bankruptcy's share of the economy.
        let k = (r_emp.round() as usize).max(1);
        // Pressure pass F2: the opening businesses that stand on Lots (the
        // venues, Labs, Clinics, Garages, Fixers, Feeds, the industries, the
        // Hq, the vice and fashion houses, the School, the Missions and
        // Hotels) are dealt per Lot, not per employer: the reference city
        // seeds ~40 % of its Lots and leaves the rest to grow on. The 100k
        // island holds 600 Farms (k = 50) on 1,415 Lots (8.8 x 160); the
        // passes dealt 50 copies each in a fixed order, the Lots ran out at
        // the Noodle Bars, and every kind seeded after (the Hq, the School,
        // the Brothel and StripClub, the Dollhouse, the FightPit, the
        // Stations) stood nowhere: Office Worker, Teacher, Madam, Contract
        // Host and Fighter were never hired (Shadow V9). At 2k and 20k the
        // two ratios agree and nothing moves.
        let k_lot = (r_lot.round() as usize).clamp(1, k);
        if k_lot < k {
            self.scale.lot_copies = k_lot as u32;
            let f = k_lot as f64 / up;
            let down = |v: u32| if v == 0 { 0 } else { ((f64::from(v) * f).round() as u32).max(1) };
            let v = &mut self.jobs.seed_venues;
            *v = SeedVenuesCfg {
                club: down(v.club),
                arcade: down(v.arcade),
                noodle_bar: down(v.noodle_bar),
                fight_pit: down(v.fight_pit),
                den: down(v.den),
                lounge: down(v.lounge),
                fab: down(v.fab),
            };
            self.street.seed_hotels = down(self.street.seed_hotels as u32) as usize;
            self.charity.seed = down(self.charity.seed);
        }
        let c = &mut self.corps;
        if c.row_count() > 0 {
            let total = |v: &Vec<u32>, r: f64| (f64::from(v.iter().sum::<u32>()) * r).round() as u32;
            let totals = [
                total(&c.farms, ratio(n(BuildingKind::Farm), s.farms)),
                total(&c.markets, r_market),
                total(&c.offices, ratio(n(BuildingKind::SecurityOffice), s.offices)),
                c.blocks.iter().sum(),
            ];
            let same = [c.farms.iter().sum(), c.markets.iter().sum(), c.offices.iter().sum(), totals[3]];
            if k > 1 || totals != same {
                c.replicate(k, totals);
            }
        }
        c.bar_owner_count = (c.bar_owner_count as f64 * r_bar).round() as usize;
        c.bar_owner_coins = (c.bar_owner_coins as f64 / r_bar).round() as i64;
        // The one-per-city buildings: seats and stock per capita.
        let per_city = |cfg: &mut BuildingCfg, have: u32| {
            let r = up / f64::from(have.max(1));
            if r > 1.0 {
                cfg.capacity = (f64::from(cfg.capacity) * r).round().clamp(0.0, 65535.0) as u16;
                cfg.stock_cap = (f64::from(cfg.stock_cap) * r).round() as u32;
            }
        };
        let bl = &mut self.buildings;
        per_city(&mut bl.jail, n(BuildingKind::Jail));
        per_city(&mut bl.hall, n(BuildingKind::Hall));
        per_city(&mut bl.cemetery, n(BuildingKind::Cemetery));
        per_city(&mut bl.warehouse, n(BuildingKind::Warehouse));
        // Per district.
        let r = &mut self.riots;
        r.riot_max = (r.riot_max as f64 * r_dsize).round() as usize;
        r.riot_promote_max = (r.riot_promote_max as f64 * r_dsize).round() as usize;
        r.max_active_riots = (r.max_active_riots as f64 * r_districts).round() as usize;
        let l = &mut self.law;
        l.gang_landlord_homes = (l.gang_landlord_homes as f64 * r_dsize).round() as usize;
        l.sweep_full = (f64::from(l.sweep_full) * r_dsize).round() as u32;
        l.max_crackdowns = (l.max_crackdowns as f64 * r_districts).round() as usize;
        // Per Lot (the opening derelicts are per resident, `scaled_to`: they empty
        // occupied Blocks, and a map of tall Blocks has many Homes per resident).
        let st = &mut self.street;
        st.shanty_lots_max = (st.shanty_lots_max as f64 * r_lot).round() as usize;
    }
}
