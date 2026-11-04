use super::*;

/// First/last name tables from `assets/names.txt`.
#[derive(Clone, Debug, Default)]
pub struct NameTables {
    pub(super) first_male: Vec<String>,
    pub(super) first_female: Vec<String>,
    pub(super) last: Vec<String>,
}

impl NameTables {
    /// Scale S5: the first-name table for `sex`.
    pub fn first_names(&self, sex: Sex) -> &[String] {
        match sex {
            Sex::Male => &self.first_male,
            Sex::Female => &self.first_female,
        }
    }

    /// Scale S5: the surname table.
    pub fn last_names(&self) -> &[String] {
        &self.last
    }

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

/// Scale S4: the `j`-th generated gang name past `[gangs] names` (one gang
/// per Hideout: twenty at 20,000), "<adjective> <noun>", 144 distinct, then
/// numbered. Disjoint from `[gangs] splinter_names`' words.
fn gang_name(j: usize) -> String {
    const ADJ: [&str; 12] =
        ["Neon", "Ash", "Static", "Iron", "Cinder", "Red", "Grey", "Glass", "Wire", "Salt", "Black", "Pale"];
    const NOUN: [&str; 12] = [
        "Dogs", "Wolves", "Rats", "Knives", "Crows", "Brothers", "Lanterns", "Teeth", "Hands", "Widows", "Ghosts",
        "Hounds",
    ];
    let (a, n) = (j % ADJ.len(), (j + j / ADJ.len()) % NOUN.len());
    let base = format!("The {} {}", ADJ[a], NOUN[n]);
    match j / (ADJ.len() * NOUN.len()) {
        0 => base,
        k => format!("{base} {}", k + 1),
    }
}

impl World {
    /// Build the initial city: map, buildings, gang, 300 citizens with homes
    /// and jobs. Deterministic for a given `(seed, config)`.
    pub fn new(seed: u64, mut config: Config) -> World {
        // P10: the starting policy (`[policy]`) seeds the policy levers in
        // `Levers::from_config` (it was `econ::apply`'s config rewrite).
        // Scale S3b: the map from its source (a text map by default, or the
        // generator); a map's own districts and El lines replace the config's.
        let map = crate::map::MapSource::from_config(&config).load();
        map.apply_to(&mut config);
        // Scale S4: the per-building, per-gang, per-corp and per-district
        // keys fitted to this map's counts (nothing moves on the shipped map).
        config.fit_to_map(&map);
        let names = NameTables::load(&config);
        // Absent until `citysim-cli calibrate` has written it; any other read
        // failure is a broken checkout and must not pass silently.
        let (stat_table, stat_table_legacy) = load_stat_table(&config);
        let stat_mlp = load_stat_mlp(&config);
        // J11: `by_role` holds the 19 bespoke roles and the configured trades.
        let config_trades = config.trades.len();

        let edge_roads = map.edge_roads();
        let levers = Levers::from_config(&config);
        let mut w = World::assemble(WorldParts {
            tick: 0,
            rng: SimRng::new(seed),
            config,
            map,
            generations: Vec::new(),
            free_list: BTreeSet::new(),
            alive: Vec::new(),
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
            transit: Default::default(),
            rides: BTreeMap::new(),
            segments: BTreeMap::new(),
            motions: BTreeMap::new(),
            aboard: BTreeMap::new(),
            van_runs: BTreeMap::new(),
            commute_log: None,
            leg_log: None,
            crossing_watch: None,
            dealers: BTreeMap::new(),
            deal_log: BTreeMap::new(),
            corner_dealers: BTreeMap::new(),
            corner_log: BTreeMap::new(),
            stands: BTreeMap::new(),
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
            plan_queue_age: BTreeSet::new(),
            plan_stale_drops: 0,
            reservations: BTreeMap::new(),
            res_by_building: BTreeMap::new(),
            res_partners: BTreeMap::new(),
            door_queue: BTreeMap::new(),
            handoffs: BTreeMap::new(),
            flow_fields: FlowCache::default(),
            landmark_base: None,
            last_seen: BTreeMap::new(),
            last_seen_by_tick: BTreeSet::new(),
            seen_gen: 0,
            located_memo: Memo::default(),
            pos_index: Default::default(),
            building_gen: 0,
            building_memo: Memo::default(),
            chromed_memo: Memo::default(),
            guard_memo: Memo::default(),
            kinds_gen: 0,
            kind_memo: Memo::default(),
            shop_gen: 0,
            shop_memo: Memo::default(),
            venue_memo: Memo::default(),
            near_memo: Memo::default(),
            claims: Memo::default(),
            district_geo: Memo::default(),
            vacancies: BTreeMap::new(),
            edge_roads,
            view_rect: None,
            ring: Default::default(),
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
            housing: Default::default(),
            sweep_beats: BTreeMap::new(),
            squat_index: BTreeMap::new(),
            riots: Vec::new(),
            next_riot_id: 0,
            rioter_of: BTreeMap::new(),
            neighbours: BTreeMap::new(),
            kin_index: BTreeMap::new(),
            edges_made: 0,
            coworker_pairs: 0,
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
            hurt: BTreeSet::new(),
            last_fight: None,
            combat: Box::default(),
            bins: BTreeSet::new(),
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
            hm_corp_hits: std::collections::VecDeque::new(),
            wants: BTreeMap::new(),
            hm_debts: BTreeMap::new(),
            hm_stiffs: BTreeMap::new(),
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
            service_room: BTreeMap::new(),
            counter_due: Vec::new(),
            arms_due: Vec::new(),
            fixer_owed: BTreeMap::new(),
            housed_now: (0, BTreeSet::new()),
            moving_in: BTreeMap::new(),
            regulars: BTreeMap::new(),
            visits: BTreeMap::new(),
            notices_today: BTreeSet::new(),
            notice_fold: BTreeMap::new(),
            meeting_tables: BTreeMap::new(),
            ledger_books: BTreeMap::new(),
            beats: BTreeMap::new(),
            beat_desk: Vec::new(),
            bylines: BTreeMap::new(),
            walk_ins: Vec::new(),
            underwater: Default::default(),
            consequence: Default::default(),
            crew: Default::default(),
            turf: Default::default(),
            fares: Default::default(),
            care: Default::default(),
            grief: Default::default(),
            courting: BTreeMap::new(),
            spots: Vec::new(),
            last_met: BTreeMap::new(),
            told: BTreeMap::new(),
            shift_beats: BTreeMap::new(),
            wealth_p90: 0,
            contracts: BTreeMap::new(),
            missions: BTreeMap::new(),
            contract_runs: BTreeMap::new(),
            contract_log: VecDeque::new(),
            next_contract: 0,
            coercions: BTreeMap::new(),
            next_coercion: 0,
            lev_tried: BTreeMap::new(),
            fake_jobs: Vec::new(),
            captives: BTreeMap::new(),
            refusals: BTreeMap::new(),
            steal_runs: BTreeMap::new(),
            feed_shamed: BTreeMap::new(),
            wounded: BTreeMap::new(),
            trauma_clients: BTreeMap::new(),
            tags: Vec::new(),
            next_tag: 0,
            gang_scanners: std::collections::BTreeSet::new(),
            attention: BTreeMap::new(),
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
            housing_demand: BTreeMap::new(),
            company_homes: BTreeMap::new(),
            floor_index: Default::default(),
            lifts: Default::default(),
            floor_answers: Default::default(),
            company_lapsed: BTreeMap::new(),
            company_notice: BTreeSet::new(),
            commute_pressure: BTreeMap::new(),
            ambient: Default::default(),
            jobs2: Default::default(),
        });
        crate::prof::timed!("new::spawn_buildings", w.spawn_buildings());
        // The city milestone C7: the walkway portals as a graph (before any path).
        systems::levels::init_portals(&mut w);
        w.litter = vec![0; w.map.w() * w.map.h()];
        crate::prof::timed!("new::districts::rebuild", systems::districts::rebuild(&mut w));
        // Transit: the El's network from the config and the map (no RNG;
        // nothing with transit off), before anything reads a commute
        // (phase 2: `life::commute_cost` rides; the staffing and the
        // housing pre-roll read it).
        crate::prof::timed!("new::transit::build", systems::transit::build(&mut w));
        w.rumours = vec![Default::default(); w.districts.len()];
        crate::prof::timed!("new::spawn_gangs", w.spawn_gangs());
        crate::prof::timed!("new::spawn_population", w.spawn_population(&names));
        // M15 W25/W28: the social skills from each agent's Skill word stream
        // (the world stream is untouched), then the city means competence reads.
        crate::prof::timed!("new::moves::seed_population", systems::moves::seed_population(&mut w));
        // M12 D26: the derelicts are cut after the population is dealt (the
        // world stream is untouched) and before ownership skips them; the
        // Hotels go up on Lots once the Bar owners are dealt.
        crate::prof::timed!("new::street::seed_derelicts", systems::street::seed_derelicts(&mut w));
        crate::prof::timed!("new::ownership::seed", systems::ownership::seed(&mut w));
        // Real economy (plan E4, Seeding): the World account and its books
        // (no RNG), before any purchase can cross.
        crate::prof::timed!("new::world_market::seed", systems::world_market::seed(&mut w));
        crate::prof::timed!("new::street::seed_hotels", systems::street::seed_hotels(&mut w));
        // M18 P5h: the city's capsule hostels in the poor districts.
        crate::prof::timed!("new::housing::seed_hostels", systems::housing::seed_hostels(&mut w));
        // M13 D18: the Garages go up last, on the Lots the Hotels left.
        crate::prof::timed!("new::assets::seed_sellers", systems::assets::seed_sellers(&mut w));
        // M14 (Seeding table, V21, V26, V47): the corps' trees by name (no
        // RNG), then, with the plane on, the Labs on the Lots left, the
        // plane and its opening ICE.
        crate::prof::timed!("new::tech::seed_corps", systems::tech::seed_corps(&mut w, false));
        crate::prof::timed!("new::virt::seed_labs", systems::virt::seed_labs(&mut w));
        // M15 W41: the Civic Wire and Nutrix Now on the Lots left (no RNG),
        // before the plane links, so each has a node from the start.
        crate::prof::timed!("new::news::seed_feeds", systems::news::seed_feeds(&mut w));
        // M15 W31: The Unplugged on a Sump Central derelict (no RNG), before
        // the plane links, so its Chapel has a node from the start.
        crate::prof::timed!("new::creeds::seed_unplugged", systems::creeds::seed_unplugged(&mut w));
        // L2 (plan L7): the venues and Fabs on the Lots left (no RNG), after
        // the Chapel and before the plane links, so each has a node.
        crate::prof::timed!("new::jobs::seed_venues", systems::jobs::seed_venues(&mut w));
        // M16a (plan C9): the seeded Fixer on the Lots left (no RNG), after
        // the venues and before the plane links, so each has a node.
        crate::prof::timed!("new::contracts::seed_fixers", systems::contracts::seed_fixers(&mut w));
        // Real economy E26, E37 (Seeding table): the Sump Mission and the
        // Chapel's kitchen, then the city's Camp (no RNG), before the plane links.
        crate::prof::timed!("new::charity::seed", systems::charity::seed(&mut w));
        crate::prof::timed!("new::camp::seed", systems::camp::seed(&mut w));
        // Jobs & lives phase 1 (§ 2): the industries and the megacorps'
        // Hqs on the Lots left (no RNG; nothing with `[jobs2]` off), before
        // the trades' places are posted.
        crate::prof::timed!("new::industry::seed", systems::industry::seed(&mut w));
        // Jobs & lives phase 3 (section 4): the Gunsmiths and the opening
        // stocks (no RNG; nothing with `[weapons]` off).
        crate::prof::timed!("new::weapons::seed", systems::weapons::seed(&mut w));
        // Jobs & lives phase 4 (§ 5.1, § 9): the Vice venues and the
        // Boutiques (no RNG; nothing with `[vice]` / `[fashion]` off).
        crate::prof::timed!("new::vice::seed", systems::vice::seed(&mut w));
        crate::prof::timed!("new::fashion::seed", systems::fashion::seed(&mut w));
        crate::prof::timed!("new::dollhouse::seed", systems::dollhouse::seed(&mut w));
        crate::prof::timed!("new::braindance::seed", systems::braindance::seed(&mut w));
        crate::prof::timed!("new::ages::seed", systems::ages::seed(&mut w));
        // Lives at work phase 7 (§ 5.2): the district Stations on the Lots
        // left and the seeded guards posted (no RNG; nothing with `[law]
        // stations` off).
        crate::prof::timed!("new::stations::seed", systems::stations::seed(&mut w));
        // Jobs and room J15: the trade places of every standing building the
        // map and the seeding passes put up (no RNG).
        crate::prof::timed!("new::trades::seed", systems::trades::seed(&mut w));
        // L2 phase 5 (the day-1 leisure pulse): opening fun spread (no RNG).
        crate::prof::timed!("new::leisure::seed_fun", systems::leisure::seed_fun(&mut w));
        crate::prof::timed!("new::virt::relink", systems::virt::relink(&mut w));
        crate::prof::timed!("new::virt::seed_ice", systems::virt::seed_ice(&mut w));
        // M15 W28: every corp's and the Law's opening competence.
        crate::prof::timed!("new::competence::seed", systems::competence::seed(&mut w));
        // Flip readiness (the transition wave; the market wage law): the Recycler's
        // opening float, out of the opening hoard (a purse move, no RNG).
        crate::prof::timed!("new::treasury::seed_till", systems::treasury::seed_till(&mut w));
        // Real economy phase 2 (plan E14, Seeding): the hoard into the
        // corps' working capital, last (no RNG; a plain purse move).
        crate::prof::timed!("new::econ::seed_capital", systems::econ::seed_capital(&mut w));
        // Flip readiness (the transition wave; wages on): the no-dole city's
        // staffing before day 0, from the capital just seeded (no RNG).
        // Class pay (docs/CLASS_PAY.md): the backgrounds, then the staffing
        // (graded posts first, by merit), then the posts the graded hires
        // left and the skilled grade (no RNG).
        crate::prof::timed!("new::classpay::seed_schooling", systems::classpay::seed_schooling(&mut w));
        crate::prof::timed!("new::wages::seed_staff", systems::wages::seed_staff(&mut w));
        crate::prof::timed!("new::classpay::seed_after_staff", systems::classpay::seed_after_staff(&mut w));
        // Hunger fix item 9 (P5g): the seeded staff housed near work where
        // Homes allow (the relocation pass, uncapped; no RNG). The pre-roll
        // (items 8-10, wages and no safety net on): Homes and floors toward
        // the jobs, unpaid as the seed buildings, until the far workers have
        // beds in range or no Lot or floor is left there (no RNG).
        if systems::fixes::hire_near(&w) {
            crate::prof::timed!("new::life::preroll_housing", systems::life::preroll_housing(&mut w));
        }
        // The island pass phase 3a (I6), after the pre-roll housed whom it
        // could and its job search settled the posts: company blocks beside
        // the workplaces whose far posts outnumber the beds near them, on
        // the Lots left or open ground, and their far staff moved in (no RNG,
        // no coin; nothing with `[company_housing]` off).
        crate::prof::timed!("new::company::seed", systems::company::seed(&mut w));
        // Hunger as sickness (after P5): the adults' histories (pure
        // hashes, no stream draw), once they are housed.
        crate::prof::timed!("new::health::seed_history", systems::health::seed_history(&mut w));
        // Scale S5: the Ambient flats beyond the residents' beds (no RNG;
        // none on a map without them).
        crate::prof::timed!("new::ambient::seed", systems::ambient::seed(&mut w));
        // M17 phase 2: the parents' accounts, minted (no RNG; nothing with
        // `[outside]` off).
        crate::prof::timed!("new::outside::seed_parents", systems::outside::seed_parents(&mut w));
        // Lives at work phase 6 (§ 4.2, § 4.3): the seeded Jobs' seniority
        // and professions (a hash, no RNG; nothing with `[careers]` off).
        crate::prof::timed!("new::careers::seed", systems::careers::seed(&mut w));
        // The pressure pass S6 (OPEN-224): the seeded couples' children, once
        // the households are settled (keyed streams, no world draw; nothing
        // with `[ages]` off).
        crate::prof::timed!("new::ages::seed_children", systems::ages::seed_children(&mut w));
        // Round 8 X5 (OPEN-248): the courtships in progress when the story
        // starts (pure hashes, no RNG), once the kin are linked.
        crate::prof::timed!("new::social::seed_courting", systems::social::seed_courting(&mut w));
        // Round 5 U1 (OPEN-232): the Schools topped up to the school-age
        // count, by reach, their teachers hired (no RNG, no coins).
        crate::prof::timed!("new::ages::seed_by_children", systems::ages::seed_by_children(&mut w));
        // The city milestone C6a: the seeded households' flats dealt again
        // once the kin are linked (kin share a flat), and every body indoors
        // on its storey (nothing on a map without spans).
        // The city milestone C8: the gangs' seats up to their hosts' top storeys first.
        crate::prof::timed!("new::vertical::seed", systems::vertical::seed(&mut w));
        crate::prof::timed!("new::floors::seed", systems::floors::seed(&mut w));
        // OPEN-140: the seed city's housing is not the first midnight's move.
        w.housed_now.1.clear();
        // The combat model C1: shooting and evasion, once the posts are
        // dealt (shooting's base is by role), then the city means again.
        systems::combat::seed_all(&mut w);
        w.skill_means = systems::moves::compute_means(&w);
        w.skill_quantiles = systems::moves::compute_quantiles(&w);
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
        // C6a: each line's entity, for the tenancies' `in=` (a host may
        // follow its tenancy in file order).
        let mut ids: Vec<EntityId> = Vec::with_capacity(defs.len());
        let mut hosted: Vec<(EntityId, u32)> = Vec::new();
        for def in defs {
            let cfg = self.config.buildings.for_kind(def.kind);
            let stock_food = match def.kind {
                BuildingKind::Home => self.config.world.home_pantry_initial,
                BuildingKind::Market => self.config.world.market_initial,
                BuildingKind::Warehouse => self.config.world.warehouse_initial,
                _ => 0,
            };
            // Jobs and room J7: a map building's seats × its floors.
            let capacity = crate::systems::founding::with_floors(self, def.kind, cfg.capacity, def.floors);
            let id = self.spawn();
            ids.push(id);
            if let Some(h) = def.within {
                hosted.push((id, h));
            }
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
                    synth_food: 0,
                    bin: 0,
                    bin_accum: 0.0,
                    spoil_carry: 0.0,
                    bin_diver: None,
                    bin_regular: None,
                    under_construction_until: None,
                    stock_x: [0; 3],
                    within: None,
                    span: def.span,
                    units: def.units,
                    floor_rent: 0,
                    floor_arrears: 0,
                    hours: None,
                    lift: def.lift,
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
            self.buildings_by_kind_mut().entry(def.kind).or_default().push(id);
        }
        for (id, h) in hosted {
            let host = ids.get(h as usize).copied();
            if let Some(b) = self.comp_mut::<Building>(id) {
                b.within = host;
            }
        }
    }

    /// One gang per Hideout, in map order, named and funded from `[gangs]`
    /// (Scale S4: past the configured names, a generated one).
    fn spawn_gangs(&mut self) {
        let hideouts = self.buildings_by_kind().get(&BuildingKind::Hideout).cloned().unwrap_or_default();
        let cfg = self.config.gangs.clone();
        for (i, hideout) in hideouts.into_iter().enumerate() {
            let id = self.spawn();
            let name = cfg.names.get(i).cloned().unwrap_or_else(|| gang_name(i - cfg.names.len()));
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
            // Jobs & lives phase 7 (Dylan's answer 5): the seeded ages to
            // `[ages] seed_max_years` under the switch.
            let max =
                if systems::ages::on_cfg(&self.config) { self.config.ages.seed_max_years } else { wc.age_max_years };
            // The pressure pass S6 (Dylan 2026-10-27): a seeded adult (a Brain,
            // a body every victim pick reads as fair) is never under the
            // protected age, whatever `age_min_years` says.
            let min = wc.age_min_years.max(self.config.ages.protected_under_years as f32);
            let rng = self.rng.world();
            let years: f32 = rng.random_range(min..max.max(min + 1.0));
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
                    status: 1.0,
                },
            );
            self.insert(id, personality);
            self.insert(id, Mood::default());
            self.insert(id, Wallet { coins });
            self.insert(id, Inventory { food, stolen_food: 0, stims: 0, parts: 0, synth: 0 });
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
        // Scale S3b: a Home of `floors` storeys holds a household a floor
        // (a map Home with no floors field is one slot: the 2k city's dealing
        // is unchanged).
        let mut homes: Vec<EntityId> = self
            .buildings_by_kind()
            .get(&BuildingKind::Home)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .flat_map(|h| {
                let floors = self.comp::<Building>(h).map_or(1, |b| usize::from(b.floors.max(1)));
                std::iter::repeat_n(h, floors)
            })
            .collect();
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
                self.insert(id, Position { tile: door, building: None, entered: 0, floor: 0 });
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
        for h in self.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default() {
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
                self.insert(id, Position { tile: street, building: None, entered: 0, floor: 0 });
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
        // Scale S1: the untaken residents by home door, each door's in pool
        // order (the homeless apart), so a slot reads one candidate per door
        // (its first untaken: the lowest `i` at that distance), not the whole
        // pool: the same `(distance, i)` minimum as the scan.
        let mut by_door: BTreeMap<TilePos, std::collections::VecDeque<usize>> = BTreeMap::new();
        let mut doorless: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for (i, id) in pool.iter().enumerate() {
            match home_door.get(id) {
                Some(&h) => by_door.entry(h).or_default().push_back(i),
                None => doorless.push_back(i),
            }
        }
        // The island pass phase 1 (OPEN-163): a slot whose pick is past
        // `[distance] job_cap_ticks` (`distance::travel`) waits until every
        // other slot has picked, then takes the nearest left (none off: the
        // base order, role by role).
        let cap = self.config.distance.as_ref().map(|d| d.job_cap_ticks);
        let mut slots: Vec<(Role, usize)> =
            Role::ALL.iter().flat_map(|&role| (0..wc.jobs.count(role) as usize).map(move |k| (role, k))).collect();
        let mut deferred: Vec<(Role, usize)> = Vec::new();
        for round in 0..2 {
            // A role with no candidate left takes no later slot (the base's `break`).
            let mut stopped: Option<Role> = None;
            for &(role, k) in &slots {
                if stopped == Some(role) {
                    continue;
                }
                let workplaces =
                    role.workplace().and_then(|k| self.buildings_by_kind().get(&k)).cloned().unwrap_or_default();
                // Lives at work phase 6 (§ 4.1): with careers on, a resident
                // outside the role's age band is passed over (seeded into
                // another row, or none); off, every door's first untaken.
                let band = systems::careers::on_cfg(&self.config)
                    .then(|| systems::careers::band_cfg(&self.config, role))
                    .filter(|&b| b != (0, 0));
                let fits = |w: &World, i: usize| {
                    band.is_none_or(|b| {
                        systems::careers::fits(w.comp::<Identity>(pool[i]).map_or(0, |x| x.age_years()), b)
                    })
                };
                let employer = workplaces.get(k % workplaces.len().max(1)).copied();
                let work_door = employer.and_then(|e| self.comp::<Building>(e)).map(|b| b.door);
                // `(distance, i, door)`: a resident with a home door and a
                // workplace door is nearer than `u32::MAX`; else the lowest `i`.
                let first = |list: &std::collections::VecDeque<usize>| list.iter().copied().find(|&i| fits(self, i));
                let mut pick: Option<(u32, usize, Option<TilePos>)> = first(&doorless).map(|i| (u32::MAX, i, None));
                for (&door, list) in &by_door {
                    let Some(i) = first(list) else { continue };
                    let d = work_door.map_or(u32::MAX, |w| door.manhattan(w));
                    if pick.is_none_or(|(pd, pi, _)| (d, i) < (pd, pi)) {
                        pick = Some((d, i, Some(door)));
                    }
                }
                let Some((_, i, door)) = pick else {
                    stopped = Some(role);
                    continue;
                };
                if round == 0 {
                    if let (Some(c), Some(d), Some(w)) = (cap, door, work_door) {
                        if systems::distance::travel(self, d, w) > c {
                            deferred.push((role, k));
                            continue;
                        }
                    }
                }
                match door {
                    Some(door) => {
                        let list = by_door.get_mut(&door).expect("the picked door");
                        if let Some(at) = list.iter().position(|&x| x == i) {
                            list.remove(at);
                        }
                        if list.is_empty() {
                            by_door.remove(&door);
                        }
                    }
                    None => {
                        if let Some(at) = doorless.iter().position(|&x| x == i) {
                            doorless.remove(at);
                        }
                    }
                }
                let id = pool[i];
                let shifts = if role == Role::Guard && id.index % 2 == 0 {
                    wc.shift_night.clone()
                } else if let Some(s) = systems::service::venue_shift(self, employer, role) {
                    // OPEN-111, Shadow V5 bug 1: a venue's legacy staff split its day and evening.
                    s
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
                        grade: 0,
                        until: None,
                        seniority_days: 0,
                        foreman: false,
                        shift_tips: 0,
                    },
                );
            }
            if round == 1 || deferred.is_empty() {
                break;
            }
            slots = std::mem::take(&mut deferred);
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
}
