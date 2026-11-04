use super::*;

/// The pre-view pass phase 6 `[leaders]` (docs/PREVIEW_PASS.md § 0 P9,
/// P13-P15). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeadersCfg {
    /// P13: the round starts from this hour and is not begun from `round_last_hour`.
    pub round_hour: u16,
    pub round_last_hour: u16,
    /// P13: stops a round (the owed protections first).
    pub round_stops_max: u8,
    /// P13: an off-screen owner is met at his business at this rate.
    pub owner_stat_p: f32,
    /// P14: court opens at this hour and runs `court_min` minutes.
    pub court_hour: u16,
    pub court_min: u16,
    /// P14: an off-screen lieutenant attends a court at this rate.
    pub court_stat_p: f32,
    /// P14: the leader sets out to be at court by `court_hour`, at most
    /// this many minutes ahead.
    pub court_walk_max: u64,
    /// P15: an order waits for a lieutenant at court this long, then the
    /// leader walks to one; past `order_wait_ticks + order_word_ticks` it
    /// goes by word (off screen).
    pub order_wait_ticks: u64,
    pub order_word_ticks: u64,
    /// P15: the order is given in the lieutenant's building or within this
    /// Chebyshev radius of him.
    pub order_reach: u32,
    /// P15: minutes the order takes, given at the lieutenant.
    pub order_give_min: u16,
}

/// M10 holes and the binder (`docs/M10_SCALE.md` § 3). A candidate's weight is
/// `(1 - lawfulness)^lawfulness_power x (1 + gang_claim_mult x gang x claims) x (1 + enemy_mult x enemy)
/// x (1 + statistical_mult x statistical)`, times `other_zone_weight` off the victim's zone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindCfg {
    /// A hole older than this binds (or goes Unknown) at the daily pass.
    pub hole_ttl_days: u64,
    /// Open holes per victim; the oldest expires to Unknown past it.
    pub max_open_per_agent: usize,
    /// Drawn first: nobody ever learns who did it (M10 D11).
    pub p_unknown: f64,
    /// Witness chance of a bound crime, times the zone's law coverage.
    pub p_witness: f32,
    pub coverage_min: f32,
    pub coverage_max: f32,
    pub gang_claim_mult: f64,
    pub enemy_mult: f64,
    pub statistical_mult: f64,
    pub other_zone_weight: f64,
    /// The exponent on `1 - lawfulness`; tuned by the parity test's actor shares.
    #[serde(default = "BindCfg::default_lawfulness_power")]
    pub lawfulness_power: f64,
    /// M12 D3: a candidate in the hole's zone but another district. 1.0 (the
    /// serde default, `v1_profile`) is the M11 binder: the whole zone weighs 1.
    #[serde(default = "BindCfg::default_same_zone_weight")]
    pub same_zone_weight: f64,
    /// M12 D3: the witness roll reads `District.coverage` and the witness
    /// pool is the hole's district; `false` is the M11 zone binder.
    #[serde(default)]
    pub district_coverage: bool,
}

impl BindCfg {
    fn default_lawfulness_power() -> f64 {
        2.0
    }

    fn default_same_zone_weight() -> f64 {
        1.0
    }

    /// The `[bind]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> BindCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).bind
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FoundCostCfg {
    pub bar: i64,
    pub home: i64,
    /// M12 D20: a Capsule Hotel on a Lot.
    #[serde(default = "FoundCostCfg::default_hotel")]
    pub hotel: i64,
    /// M13 D16.
    #[serde(default)]
    pub clinic: i64,
    #[serde(default)]
    pub garage: i64,
    /// M14 (spec § 4): a Lab, built only by the Research order.
    #[serde(default)]
    pub lab: i64,
    /// M15 § 7: a Feed (foundable through `Register` with `[news]` on).
    #[serde(default)]
    pub feed: i64,
    /// L2 § 1 (spec values as defaults; `founding::found_cost` prices them
    /// only with `jobs::on`).
    #[serde(default = "FoundCostCfg::default_club")]
    pub club: i64,
    #[serde(default = "FoundCostCfg::default_arcade")]
    pub arcade: i64,
    #[serde(default = "FoundCostCfg::default_noodle_bar")]
    pub noodle_bar: i64,
    #[serde(default = "FoundCostCfg::default_fight_pit")]
    pub fight_pit: i64,
    #[serde(default = "FoundCostCfg::default_den")]
    pub den: i64,
    #[serde(default = "FoundCostCfg::default_lounge")]
    pub lounge: i64,
    /// Corps only (the Fab is never `Register`ed).
    #[serde(default = "FoundCostCfg::default_fab")]
    pub fab: i64,
    /// M16a (plan C8; `founding::found_cost` prices it only with `contracts::on`).
    #[serde(default = "FoundCostCfg::default_fixer")]
    pub fixer: i64,
    /// Real economy E26 (priced only with `charity::on`).
    #[serde(default = "FoundCostCfg::default_mission")]
    pub mission: i64,
    /// Real economy E37 (corps only, with `camp::on`).
    #[serde(default = "FoundCostCfg::default_camp")]
    pub camp: i64,
}

impl FoundCostCfg {
    fn default_mission() -> i64 {
        150
    }
    fn default_camp() -> i64 {
        600
    }
    fn default_hotel() -> i64 {
        250
    }
    fn default_club() -> i64 {
        400
    }
    fn default_arcade() -> i64 {
        200
    }
    fn default_noodle_bar() -> i64 {
        120
    }
    fn default_fight_pit() -> i64 {
        300
    }
    fn default_den() -> i64 {
        250
    }
    fn default_lounge() -> i64 {
        900
    }
    fn default_fab() -> i64 {
        900
    }
    fn default_fixer() -> i64 {
        400
    }
}

/// D48: flat terms on each corp order's score.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorpOrderFlatCfg {
    pub grow: f32,
    pub squeeze: f32,
    pub undercut: f32,
    pub acquire: f32,
    pub secure: f32,
    pub hunker: f32,
    pub lobby: f32,
    /// M14 (plan V31).
    #[serde(default)]
    pub research: f32,
    /// M14 (plan V31, phase 3).
    #[serde(default)]
    pub virt_raid: f32,
    /// M15 W40.
    #[serde(default)]
    pub spin: f32,
}

/// M11 corps (docs/M11_OWNERSHIP.md § 5 and the plan's additions). One row
/// per corp in the row vectors; `none()` is the corp-free city.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CorpsCfg {
    pub names: Vec<String>,
    pub niches: Vec<Vec<String>>,
    pub farms: Vec<u32>,
    pub markets: Vec<u32>,
    pub blocks: Vec<u32>,
    pub offices: Vec<u32>,
    pub treasury_initial: Vec<i64>,
    pub bar_owner_coins: i64,
    pub hoard_heat: i64,
    pub acquire_premium: f32,
    pub acquire_cooldown_days: u64,
    pub undercut_floor: f32,
    pub hysteresis: f32,
    pub shock_severity_rethink: f32,
    pub bankrupt_days: u64,
    pub incorporate_buildings: usize,
    pub found_cost: FoundCostCfg,
    pub wholesale: i64,
    pub contract_per_guard_day: i64,
    pub security_guards: u32,
    pub lobby_min_treasury: i64,
    pub monopoly_markup_cap: f32,
    /// Squeeze's `price_level` ceiling without a monopoly (spec: 1.5).
    pub squeeze_cap: f32,
    pub bar_owner_count: usize,
    pub upkeep: UpkeepCfg,
    pub value: ValueCfg,
    pub shop_price_tiles: i64,
    pub grow_cooldown_days: u64,
    pub secure_per_day: usize,
    pub private_pursuit_radius: u32,
    /// Shadow V4 bug 11: a private guard's warrants lie within this many
    /// tiles of its office or a client of its corp.
    #[serde(default)]
    pub private_beat_tiles: u32,
    pub found_cooldown_days: u64,
    pub residents_per_bar: u32,
    /// M12 D20: `Register`'s target count of Hotels is `population ÷ this`.
    pub residents_per_hotel: u32,
    /// M13 D16: `Register`'s target count of Clinics is `population ÷ this`.
    pub residents_per_clinic: u32,
    /// M13 D16: likewise Garages.
    pub residents_per_garage: u32,
    /// L2 (plan L5): `Register`'s per-capita targets of the leisure kinds
    /// (the spec's `residents_per` table as flat keys), read only with
    /// `jobs::on`.
    pub residents_per_club: u32,
    pub residents_per_arcade: u32,
    pub residents_per_noodle: u32,
    pub residents_per_pit: u32,
    pub residents_per_den: u32,
    pub residents_per_lounge: u32,
    /// M11 phase 4: a flat term on the Found goal's score (calibration knob).
    #[serde(default)]
    pub found_flat: f32,
    pub hoard_tilt: f32,
    pub megacorp: Vec<bool>,
    pub outside_treasury_initial: i64,
    pub order_flat: CorpOrderFlatCfg,
    /// M11 phase 5 (estate rule): a bankruptcy estate skips corps whose share
    /// of the building's niche is at or above this.
    pub estate_share_cap: f32,
    /// M11 phase 5 (estate rule): an estate's building goes only to corps
    /// already in its niche (agents may still buy).
    pub estate_niche_only: bool,
    /// M11 phase 5 (estate rule): one buyer takes at most this many buildings
    /// of one estate (0 = no cap).
    pub estate_buyer_cap: usize,
    /// M11 review (estate settlement): the most of a dissolved corp's positive
    /// balance its exec takes as severance; the rest goes to the Treasury.
    pub estate_heir_cap: i64,
    /// M11 phase 5: a newly incorporated corp pays no upkeep for this many days.
    pub incorporate_grace_days: u64,
    /// Hunker never lays off Farm staff (phase 3 follow-up).
    pub hunker_spares_farms: bool,
    /// M11 phase 5: Housing Grow builds only when the corp's own Blocks are
    /// at least this full (0 = no gate).
    pub grow_min_occupancy: f32,
    /// M12 fix pass: for this many days after a raid or a riot took from one
    /// of its buildings, a corp's `losses` input reads at least
    /// `raided_losses` (it hardens: Secure, or Lobby against the gang).
    #[serde(default = "CorpsCfg::default_raided_days")]
    pub raided_days: u64,
    #[serde(default = "CorpsCfg::default_raided_losses")]
    pub raided_losses: f32,
    /// Scale S4: each row's template, the shipped row it copies (empty: every
    /// row is its own, `names`). `Config::fit_to_map` deals the roster once
    /// per ~2,000 residents' worth of employers ("Nutrix", "Nutrix 2", ...);
    /// the name-keyed seeding (the tech trees, the Labs, the Fabs, the Feeds)
    /// reads a row's template, so every copy of Zetatech is a Tech corp
    /// seeded as Zetatech.
    #[serde(default)]
    pub template: Vec<String>,
    /// M17 phase 5 (the rename hazard): the seeding roles by row index into
    /// `names` (a row's template at 20k), never by a corp's name, so a renamed
    /// row keeps its role. The Fab owners, dealt in turn (Zetatech, Militech).
    #[serde(default = "CorpsCfg::default_fab_rows")]
    pub fab_rows: Vec<usize>,
    /// The Food row the Noodle Bars' round-robin starts from (Nutrix).
    #[serde(default = "CorpsCfg::default_noodle_first_row")]
    pub noodle_first_row: Option<usize>,
    /// The seeded Labs (M14 V26): owner row, district by zone, focus track.
    #[serde(default = "CorpsCfg::default_seed_labs")]
    pub seed_labs: Vec<SeedSite>,
    /// The seeded corp Feeds (M15 W41, "<owner> Now"): owner row, district by zone.
    #[serde(default = "CorpsCfg::default_seed_feeds")]
    pub seed_feeds: Vec<SeedSite>,
}

/// M17 phase 5: a seeded building's owner row and seat (`seed_labs`, `seed_feeds`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeedSite {
    pub row: usize,
    pub zone: crate::components::Zone,
    #[serde(default)]
    pub track: crate::virt::Track,
}

impl Default for CorpsCfg {
    fn default() -> Self {
        CorpsCfg::none()
    }
}

impl CorpsCfg {
    fn default_fab_rows() -> Vec<usize> {
        vec![8, 7]
    }
    fn default_noodle_first_row() -> Option<usize> {
        Some(0)
    }
    fn default_seed_labs() -> Vec<SeedSite> {
        use crate::components::Zone;
        use crate::virt::Track;
        let site = |row, zone, track| SeedSite { row, zone, track };
        vec![
            site(8, Zone::Spire, Track::Chrome),
            site(8, Zone::Civic, Track::Industry),
            site(6, Zone::Vats, Track::Deck),
            site(7, Zone::Mid, Track::Deck),
        ]
    }
    fn default_seed_feeds() -> Vec<SeedSite> {
        vec![SeedSite { row: 0, zone: crate::components::Zone::Mid, track: crate::virt::Track::Industry }]
    }

    /// Row `i`'s template when `i` is a row of the shipped roster, else `None`.
    pub fn row_template(&self, i: usize) -> Option<&str> {
        (i < self.names.len()).then(|| self.template_of(i))
    }

    fn default_raided_days() -> u64 {
        7
    }
    fn default_raided_losses() -> f32 {
        0.5
    }
}

impl CorpsCfg {
    /// No corps and no agent Bar owners; every scalar at its documented default.
    pub fn none() -> CorpsCfg {
        CorpsCfg {
            names: Vec::new(),
            niches: Vec::new(),
            farms: Vec::new(),
            markets: Vec::new(),
            blocks: Vec::new(),
            offices: Vec::new(),
            treasury_initial: Vec::new(),
            bar_owner_coins: 200,
            hoard_heat: 5000,
            acquire_premium: 1.2,
            acquire_cooldown_days: 10,
            undercut_floor: 0.6,
            hysteresis: 0.10,
            shock_severity_rethink: 0.5,
            bankrupt_days: 14,
            incorporate_buildings: 2,
            found_cost: FoundCostCfg {
                bar: 300,
                home: 400,
                hotel: 250,
                clinic: 500,
                garage: 600,
                lab: 800,
                feed: 500,
                club: 400,
                arcade: 200,
                noodle_bar: 120,
                fight_pit: 300,
                den: 250,
                lounge: 900,
                fab: 900,
                fixer: 400,
                mission: 150,
                camp: 600,
            },
            wholesale: 2,
            contract_per_guard_day: 10,
            security_guards: 6,
            lobby_min_treasury: 400,
            monopoly_markup_cap: 2.0,
            squeeze_cap: 1.5,
            bar_owner_count: 0,
            upkeep: UpkeepCfg {
                farm: 120,
                market: 600,
                bar: 15,
                home: [3; 3],
                security_office: 60,
                hotel: 10,
                clinic: 10,
                garage: 10,
                lab: 0,
                feed: 0,
                club: 10,
                arcade: 4,
                noodle_bar: 2,
                fight_pit: 4,
                den: 4,
                lounge: 20,
                fab: 15,
                fixer: 8,
                mission: 0,
                camp: 2,
            },
            value: ValueCfg {
                farm: 1000,
                market: 1000,
                security_office: 500,
                clinic: 600,
                garage: 800,
                lab: 1200,
                feed: 800,
                club: 600,
                arcade: 300,
                noodle_bar: 180,
                fight_pit: 400,
                den: 350,
                lounge: 1400,
                fab: 1500,
                fixer: 500,
                mission: 300,
                camp: 800,
            },
            // A pre-M11 save (and v1_profile) shops at the nearest Market.
            shop_price_tiles: 0,
            grow_cooldown_days: 7,
            secure_per_day: 2,
            private_pursuit_radius: 16,
            private_beat_tiles: 40,
            found_cooldown_days: 10,
            residents_per_bar: 300,
            residents_per_hotel: 800,
            residents_per_clinic: 700,
            residents_per_garage: 700,
            residents_per_club: 600,
            residents_per_arcade: 600,
            residents_per_noodle: 120,
            residents_per_pit: 1000,
            residents_per_den: 800,
            residents_per_lounge: 2000,
            found_flat: 0.0,
            hoard_tilt: 0.1,
            megacorp: Vec::new(),
            outside_treasury_initial: 100_000,
            estate_share_cap: 0.5,
            estate_niche_only: true,
            estate_buyer_cap: 0,
            estate_heir_cap: 300,
            incorporate_grace_days: 0,
            hunker_spares_farms: true,
            grow_min_occupancy: 0.0,
            raided_days: 7,
            raided_losses: 0.5,
            order_flat: CorpOrderFlatCfg {
                grow: 0.0,
                squeeze: 0.0,
                undercut: 0.0,
                acquire: 0.0,
                secure: 0.0,
                hunker: 0.15,
                lobby: 0.0,
                research: 0.0,
                virt_raid: 0.0,
                spin: 0.0,
            },
            template: Vec::new(),
            fab_rows: Vec::new(),
            noodle_first_row: None,
            seed_labs: Vec::new(),
            seed_feeds: Vec::new(),
        }
    }

    /// Scale S4: row `i`'s template, the shipped row it copies (its own
    /// name when the roster was never dealt again).
    pub fn template_of(&self, i: usize) -> &str {
        self.template.get(i).or_else(|| self.names.get(i)).map_or("", String::as_str)
    }

    /// Scale S4: the roster dealt `k` times (row `i + j × n` copies row `i`,
    /// named "<name> <j + 1>" past the first copy), the treasuries split `k`
    /// ways and each holding column re-dealt over the copies so that they ask
    /// for `[farms, markets, offices, blocks]` of `totals` together (largest
    /// remainder by the row's own count, ties to the lower row). `k = 1`
    /// with totals equal to the columns' sums changes nothing.
    pub fn replicate(&mut self, k: usize, totals: [u32; 4]) {
        let n = self.row_count();
        let k = k.max(1);
        let rows = |v: &Vec<u32>| -> Vec<u32> { (0..k).flat_map(|_| v.iter().copied()).collect() };
        let deal = |weights: Vec<u32>, total: u32| -> Vec<u32> {
            let sum: u64 = weights.iter().map(|&w| u64::from(w)).sum();
            if sum == 0 {
                return weights;
            }
            let exact: Vec<(u64, u64)> = weights
                .iter()
                .map(|&w| (u64::from(w) * u64::from(total) / sum, u64::from(w) * u64::from(total) % sum))
                .collect();
            let mut out: Vec<u32> = exact.iter().map(|&(q, _)| q as u32).collect();
            let left = u64::from(total) - exact.iter().map(|&(q, _)| q).sum::<u64>();
            let mut order: Vec<usize> = (0..weights.len()).filter(|&i| weights[i] > 0).collect();
            order.sort_by_key(|&i| (std::cmp::Reverse(exact[i].1), i));
            for &i in order.iter().take(left as usize) {
                out[i] += 1;
            }
            out
        };
        let template: Vec<String> = (0..k).flat_map(|_| (0..n).map(|i| self.template_of(i).to_string())).collect();
        self.names = (0..k)
            .flat_map(|j| (0..n).map(move |i| (i, j)))
            .map(|(i, j)| if j == 0 { self.names[i].clone() } else { format!("{} {}", self.names[i], j + 1) })
            .collect();
        self.niches = (0..k).flat_map(|_| self.niches.iter().cloned()).collect();
        self.megacorp = (0..k).flat_map(|_| (0..n).map(|i| self.megacorp.get(i).copied().unwrap_or(false))).collect();
        self.farms = deal(rows(&self.farms), totals[0]);
        self.markets = deal(rows(&self.markets), totals[1]);
        self.offices = deal(rows(&self.offices), totals[2]);
        self.blocks = deal(rows(&self.blocks), totals[3]);
        let kk = k as f64;
        self.treasury_initial =
            (0..k).flat_map(|_| self.treasury_initial.iter().map(|&t| (t as f64 / kk).round() as i64)).collect();
        self.template = template;
    }

    /// The number of corp rows; panics when the row vectors disagree.
    pub fn row_count(&self) -> usize {
        let n = self.names.len();
        let lens = [
            ("niches", self.niches.len()),
            ("farms", self.farms.len()),
            ("markets", self.markets.len()),
            ("blocks", self.blocks.len()),
            ("offices", self.offices.len()),
            ("treasury_initial", self.treasury_initial.len()),
        ];
        for (name, len) in lens {
            assert!(len == n, "[corps] {name} has {len} rows, names has {n}");
        }
        n
    }

    /// Row `i`'s niches; panics on an unknown name.
    pub fn niches_of(&self, i: usize) -> std::collections::BTreeSet<Niche> {
        self.niches[i]
            .iter()
            .map(|s| Niche::parse(s).unwrap_or_else(|| panic!("[corps] row {i}: unknown niche {s:?}")))
            .collect()
    }
}

/// M11 classes (docs/M11_OWNERSHIP.md § 7; plan D34-D36).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassesCfg {
    /// Street unrest above this calls a strike.
    pub strike_threshold: f32,
    pub strike_cooldown_days: u64,
    /// D34: Chebyshev tiles from a Home's door that an on-shift guard watches.
    pub fear_radius: u32,
    /// Guard-hours a day near a Home that read as full fear.
    pub fear_hours_full: f32,
    /// Immigration = lever x Logistic{k, mid}(Street happiness); false = the lever.
    pub couple_immigration: bool,
    pub immigration_k: f32,
    pub immigration_mid: f32,
    pub dreg_emigrate_mood: f32,
    /// 0 = off.
    pub dreg_emigrate_days: u8,
    /// D36: an evictee this recent and this lawless is desperate for a gang.
    pub evicted_recruit_days: u64,
    pub evicted_recruit_lawfulness: f32,
    /// M12 D29: district unrest gains `rent_burden_w × burden_d` (0 = off).
    #[serde(default)]
    pub rent_burden_w: f32,
    /// M12 D29: submission gains this under a district curfew.
    #[serde(default)]
    pub curfew_fear: f32,
}

impl Default for ClassesCfg {
    fn default() -> Self {
        ClassesCfg::off()
    }
}

impl ClassesCfg {
    /// The documented defaults with the coupling and Dreg emigration off
    /// (pre-M11 saves, `v1_profile`: plan D9, D40).
    pub fn off() -> ClassesCfg {
        ClassesCfg {
            strike_threshold: 0.6,
            strike_cooldown_days: 7,
            fear_radius: 6,
            fear_hours_full: 1.0,
            couple_immigration: false,
            immigration_k: 8.0,
            immigration_mid: 0.5,
            dreg_emigrate_mood: -0.5,
            dreg_emigrate_days: 0,
            evicted_recruit_days: 14,
            evicted_recruit_lawfulness: 0.5,
            rent_burden_w: 0.0,
            curfew_fear: 0.0,
        }
    }
}

/// M12 control presence weights (plan D8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlWeights {
    pub held_home: f32,
    pub owned_home: f32,
    pub owned_other: f32,
    pub city_per_coverage: f32,
}

/// M12 districts (docs/M12_DISTRICTS.md § 1). One row per district: a name,
/// a zone letter and a half-open x range inside that zone. A tile is in the
/// first row whose zone matches its zone and whose range holds its x.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistrictsCfg {
    pub names: Vec<String>,
    pub zones: Vec<String>,
    pub x_from: Vec<u16>,
    pub x_to: Vec<u16>,
    /// Below this share of the total presence the district is Contested.
    pub control_min_share: f32,
    pub control_weights: ControlWeights,
}

impl DistrictsCfg {
    /// The `[districts]` block of `assets/config.toml`, for saves written before it existed.
    pub fn from_assets() -> DistrictsCfg {
        let dir = Config::find_assets_dir()
            .unwrap_or_else(|| panic!("assets/config.toml not found; set CITYSIM_ASSETS or run from the repo"));
        Config::load_from(&dir).districts
    }

    /// One district, "City", covering the whole map (the v1 city, plan D1).
    pub fn single() -> DistrictsCfg {
        DistrictsCfg {
            names: vec!["City".to_string()],
            zones: vec!["M".to_string()],
            x_from: vec![0],
            x_to: vec![256],
            control_min_share: 0.4,
            control_weights: ControlWeights {
                held_home: 1.0,
                owned_home: 1.0,
                owned_other: 3.0,
                city_per_coverage: 1.0,
            },
        }
    }

    /// The number of rows; panics when the row vectors disagree, a zone
    /// letter is unknown or there are more than `MAX_DISTRICTS` rows.
    pub fn row_count(&self) -> usize {
        let n = self.names.len();
        for (name, len) in [("zones", self.zones.len()), ("x_from", self.x_from.len()), ("x_to", self.x_to.len())] {
            assert!(len == n, "[districts] {name} has {len} rows, names has {n}");
        }
        assert!(
            (1..=crate::components::MAX_DISTRICTS).contains(&n),
            "[districts] has {n} rows; 1..={} allowed",
            crate::components::MAX_DISTRICTS
        );
        for i in 0..n {
            assert!(self.zone_of(i).is_some(), "[districts] row {i}: unknown zone {:?}", self.zones[i]);
        }
        n
    }

    /// Row `i`'s zone, from its letter.
    pub fn zone_of(&self, i: usize) -> Option<crate::components::Zone> {
        let mut chars = self.zones.get(i)?.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        crate::components::Zone::parse(c)
    }
}

/// M13 plan 1.2: one value per `AssetKind` class (implants share a row).
/// Every field defaults, so `parts_per` may omit `pack` and `bridge`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PerKind<T: Default> {
    #[serde(default)]
    pub motorcycle: T,
    #[serde(default)]
    pub car: T,
    #[serde(default)]
    pub truck: T,
    #[serde(default)]
    pub flyer: T,
    #[serde(default)]
    pub implant: T,
    #[serde(default)]
    pub robot: T,
    #[serde(default)]
    pub pack: T,
    /// Plan D52 (M14's Virt bridge).
    #[serde(default)]
    pub bridge: T,
    /// M14 V36.
    #[serde(default)]
    pub deck: T,
    /// M14 V28.
    #[serde(default)]
    pub camera: T,
    /// Jobs & lives phase 3.
    #[serde(default)]
    pub weapon: T,
    /// Jobs & lives phase 4.
    #[serde(default)]
    pub clothing: T,
    /// Jobs & lives phase 5.
    #[serde(default)]
    pub recorder: T,
}

impl AssetsCfg {
    /// The down payment share for `kind` (phase 5: implants have their own).
    pub fn down_frac_of(&self, kind: AssetKind) -> f32 {
        if kind.is_implant() {
            self.implant_down_frac
        } else {
            self.down_frac
        }
    }
}

impl<T: Default> PerKind<T> {
    pub fn get(&self, kind: AssetKind) -> &T {
        match kind.class() {
            AssetClass::Motorcycle => &self.motorcycle,
            AssetClass::Car => &self.car,
            AssetClass::Truck => &self.truck,
            AssetClass::Flyer => &self.flyer,
            AssetClass::Implant => &self.implant,
            AssetClass::Robot => &self.robot,
            AssetClass::Pack => &self.pack,
            AssetClass::Bridge => &self.bridge,
            AssetClass::Deck => &self.deck,
            AssetClass::Camera => &self.camera,
            AssetClass::Weapon => &self.weapon,
            AssetClass::Clothing => &self.clothing,
            AssetClass::Recorder => &self.recorder,
        }
    }
}

/// M13 assets (docs/M13_ASSETS.md § 1, plan D1-D15, D50). Tier lists read
/// `get(kind)[tier - 1]`; a missing tier is not sold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsCfg {
    /// `off()` = false: no pass, no Kit term, no Body read; the CSV columns stay 0.
    pub enabled: bool,
    pub price: PerKind<Vec<i64>>,
    pub upkeep: PerKind<Vec<i64>>,
    pub wear: PerKind<u8>,
    pub parts_per: PerKind<u32>,
    pub parts_price: i64,
    pub import_frac: f32,
    /// Import coins one Part from a seller's stock replaces.
    pub part_credit: i64,
    pub down_frac: f32,
    /// Phase 5 (the chrome finance spiral): an implant's down payment share
    /// (1.0 = cash only). `down_frac` for every other kind.
    pub implant_down_frac: f32,
    /// Phase 5: what an agent's `Register` pays for a Clinic or a Garage,
    /// overriding `[corps] found_cost` when set (seeded founders hold
    /// 200-300 coins against 500 and 600, so no NPC ever founded one).
    pub found_cost_clinic: Option<i64>,
    pub found_cost_garage: Option<i64>,
    /// Phase 5: an agent founder's per-capita target for a Clinic or a
    /// Garage (`founding::choose_kind`), overriding `[corps]
    /// residents_per_clinic`/`_garage` (which still cap the Tech corp): the
    /// seeded three Clinics and three Garages met the 700 target, so a
    /// founder always chose a Bar.
    pub founder_residents_per_seller: Option<u32>,
    pub interest: f32,
    pub term_days: u32,
    pub repo_days: u8,
    pub brick_repo_days: u8,
    pub impound_days: u8,
    pub impound_frac: f32,
    pub unpaid_wear_mult: u8,
    pub repair_below: u8,
    /// Coins per condition point.
    pub repair_price: i64,
    pub fail_shock: f32,
    /// Plan D13: 0 runs inheritance at death (M12); phase 3 sets 12.
    pub loot_window_hours: u32,
    pub rip_courage: f32,
    pub scav_strip_base: f32,
    pub carry_base: u32,
    pub carry_per_strength: f32,
    /// Carry added by a pack, per tier.
    pub pack: Vec<u32>,
    /// Plan D6: the Stims/Parts cap where a kind's `stock_cap` is lower.
    pub goods_cap: u32,
    /// Plan D14: a Clinic or Garage below this many Parts buys `parts_batch`.
    pub parts_floor: u32,
    pub parts_batch: u32,
    /// Phase 1 deviation (plan D16 grows `FOUNDABLE`), per kind from phase
    /// 2: `Register`, `choose_kind` and a Tech corp's `Grow` may build a
    /// Garage only while `found_garage`, a Clinic only while `found_clinic`
    /// (false until phase 3 makes the Clinic do something).
    pub found_garage: bool,
    pub found_clinic: bool,
}

impl Default for AssetsCfg {
    fn default() -> Self {
        AssetsCfg::off()
    }
}

impl AssetsCfg {
    /// Assets off (a pre-M13 save, `v1_profile`, the calibration city):
    /// every other key at its `assets/config.toml` value.
    pub fn off() -> AssetsCfg {
        AssetsCfg {
            enabled: false,
            price: PerKind {
                motorcycle: vec![300],
                car: vec![800, 1400],
                truck: vec![1500],
                flyer: vec![6000],
                implant: vec![150, 500, 1500],
                robot: vec![800, 2000, 5000],
                pack: vec![30, 80],
                bridge: vec![200, 600, 1800],
                deck: vec![400, 1500, 5000],
                camera: vec![120, 400, 1200],
                weapon: vec![60, 160, 400],
                clothing: vec![20, 60, 180],
                recorder: vec![120],
            },
            upkeep: PerKind {
                motorcycle: vec![1],
                car: vec![2, 3],
                truck: vec![4],
                flyer: vec![15],
                implant: vec![0, 1, 3],
                robot: vec![4, 8, 15],
                pack: vec![0, 0],
                bridge: vec![0, 0, 0],
                deck: vec![1, 3, 8],
                camera: vec![1, 2, 4],
                weapon: vec![0, 0, 0],
                clothing: vec![0, 0, 0],
                recorder: vec![0],
            },
            wear: PerKind {
                motorcycle: 1,
                car: 1,
                truck: 1,
                flyer: 1,
                implant: 0,
                robot: 1,
                pack: 0,
                bridge: 0,
                deck: 0,
                camera: 1,
                weapon: 0,
                clothing: 1,
                recorder: 0,
            },
            parts_per: PerKind {
                motorcycle: 4,
                car: 8,
                truck: 12,
                flyer: 30,
                implant: 3,
                robot: 10,
                pack: 0,
                bridge: 0,
                deck: 4,
                camera: 2,
                weapon: 3,
                clothing: 0,
                recorder: 0,
            },
            parts_price: 15,
            import_frac: 0.6,
            part_credit: 20,
            down_frac: 0.25,
            implant_down_frac: 0.25,
            found_cost_clinic: None,
            found_cost_garage: None,
            founder_residents_per_seller: None,
            interest: 0.2,
            term_days: 60,
            repo_days: 5,
            brick_repo_days: 10,
            impound_days: 10,
            impound_frac: 0.5,
            unpaid_wear_mult: 3,
            repair_below: 50,
            repair_price: 2,
            fail_shock: 0.1,
            loot_window_hours: 0,
            rip_courage: 0.5,
            scav_strip_base: 0.15,
            carry_base: 20,
            carry_per_strength: 10.0,
            pack: vec![10, 25],
            goods_cap: 1000,
            parts_floor: 20,
            parts_batch: 20,
            found_garage: false,
            found_clinic: false,
        }
    }
}

/// M13 § 6 the Shop goal, the Garage and fleets (plan D17, D18, D43, D44).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShopCfg {
    pub shop_cooldown_days: u32,
    /// Tiles of commute at which the vehicle term reads 0.5.
    pub commute_ref: u32,
    pub shop_flat: f32,
    pub gang_buy_floor: i64,
    /// Coins a day a vehicle parked in someone else's Garage pays its owner.
    pub garage_rent: i64,
    /// Plan D43: upkeep + finance a day at most this share of daily income.
    pub max_burden: f32,
    /// Plan D43: a Statistical shopper buys at this score or above.
    pub stat_shop_min: f32,
    /// Plan D18: a seeded Garage owner's wallet.
    pub seed_owner_coins: i64,
    /// Plan D44: Food corps buy trucks under any order but Hunker.
    pub fleet_any_order: bool,
    /// Plan D17: a Tech corp's demand reference (sales a day per building).
    pub tech_demand_ref: f32,
    /// M13 phase 5 (the chrome finance spiral): the D43 burden test reads
    /// income net of this many meals a day at the mean Market price
    /// (0 = gross income, the plan's rule).
    pub burden_meals: f32,
    /// Phase 5: a corp buys its exec a flyer with this many times its price
    /// in the treasury and the books (0 = never).
    pub exec_flyer_cash_mult: f32,
}

impl Default for ShopCfg {
    fn default() -> Self {
        ShopCfg::off()
    }
}

impl ShopCfg {
    /// The spec's and plan's values (assets off reads none of them).
    pub fn off() -> ShopCfg {
        ShopCfg {
            shop_cooldown_days: 7,
            commute_ref: 60,
            shop_flat: 0.02,
            gang_buy_floor: 600,
            garage_rent: 1,
            max_burden: 0.5,
            stat_shop_min: 0.3,
            seed_owner_coins: 300,
            fleet_any_order: false,
            tech_demand_ref: 1.0,
            burden_meals: 0.0,
            exec_flyer_cash_mult: 0.0,
        }
    }
}

/// `[robots] robot_tier_by_kind` (spec § 5): the tier `Secure` buys per
/// building kind; a kind left out buys tier 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RobotTiers {
    pub farm: u8,
    pub market: u8,
    pub home: u8,
    pub security_office: u8,
    pub clinic: u8,
    pub garage: u8,
}

impl Default for RobotTiers {
    fn default() -> Self {
        RobotTiers { farm: 2, market: 2, home: 1, security_office: 3, clinic: 2, garage: 2 }
    }
}

/// M13 § 5 the security robot (plan phase 4, D41, D42).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RobotsCfg {
    /// `law::fighting` of a robot, per tier.
    pub robot_fighting: Vec<f32>,
    /// D42: a robot's cost is its price plus this many days of upkeep,
    /// against as many days of a guard contract.
    pub robot_horizon_days: u32,
    /// D42: a corp buys a robot only with its treasury at this × the price.
    pub robot_cash_mult: f32,
    pub robot_tier_by_kind: RobotTiers,
    /// M14: a hacked robot changes owner for this long (nothing reads it in M13).
    pub hack_hours: u32,
}

impl Default for RobotsCfg {
    fn default() -> Self {
        RobotsCfg::off()
    }
}

impl RobotsCfg {
    /// The plan's values (assets off reads none of them).
    pub fn off() -> RobotsCfg {
        RobotsCfg {
            robot_fighting: vec![0.5, 0.75, 1.0],
            robot_horizon_days: 60,
            robot_cash_mult: 2.0,
            robot_tier_by_kind: RobotTiers::default(),
            hack_hours: 24,
        }
    }
}

// ---------------------------------------------------------------------------
// M16b § 4 leverage (docs/M16_CONTRACTS.md; phase 1). A coercion is a record
// on a struct and one seeded Intimidate roll between fictional agents.
// ---------------------------------------------------------------------------

/// `[leverage] lev_bias`: the logit's bias per leverage kind (phase 2
/// appends `hostage`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevBiasCfg {
    pub threat: f32,
    pub secret: f32,
    pub debt: f32,
    /// M16b phase 2.
    pub hostage: f32,
}

impl Default for LevBiasCfg {
    fn default() -> Self {
        LevBiasCfg { threat: 0.0, secret: 0.3, debt: 0.2, hostage: 0.5 }
    }
}

impl LevBiasCfg {
    pub fn get(&self, k: crate::contract::LeverageKind) -> f32 {
        use crate::contract::LeverageKind;
        match k {
            LeverageKind::Threat => self.threat,
            LeverageKind::Secret => self.secret,
            LeverageKind::Debt => self.debt,
            LeverageKind::Hostage => self.hostage,
        }
    }
}

/// `[leverage]` (spec § 4; the plan keys marked). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LeverageCfg {
    /// The switch in flight (the A/B and identity device; off: M16a's
    /// city, the shakedown M15's roll).
    pub enabled: bool,
    /// Standing coercions at once (per capita, up only).
    pub max_coercions: usize,
    pub lev_bias: LevBiasCfg,
    /// `demand_cost = demand_w × value ÷ max(purse + 7 × wage, 1)`.
    pub demand_w: f32,
    /// A Secret adds `secret_w × deed_sev × (1 − reach)`.
    pub secret_w: f32,
    /// A failed move is reported with `p = backlash_report × lawfulness`.
    pub backlash_report: f32,
    pub protection_weekly: i64,
    pub blackmail_weekly: i64,
    pub blackmail_lawfulness: f32,
    /// Plan key: the coerced's standing above the holder's by this much.
    pub blackmail_gap: f32,
    /// Plan key: a holder looks for a secret once in this many days.
    pub blackmail_days: u32,
    pub secret_known_max: u32,
    pub secret_public_reach: f32,
    pub silence_lawfulness: f32,
    pub poach_threat_lawfulness: f32,
    /// Plan key: a debt under this is not called (begging loans).
    pub debt_call_min: i64,
    pub fake_fee: i64,
    /// Plan key: a fake-job candidate tries on a day with this p.
    pub fake_p: f32,
    /// Plan key: the fake job's victim learns it after this many days.
    pub fake_days: u32,
    /// Round 4 T6: a mark who learned of a con (an unsettled `Betrayed`
    /// grudge) inside this many days buys no fake job (0 off).
    pub fake_wary_days: u32,
    pub parley_ratio: f32,
    pub parley_tribute: i64,
    /// Plan key: a fulfilled Persuade adds this to the courted's affinity.
    pub persuade_affinity: f32,
    /// Plan key: a suitor rejected this often by the same person (30 days).
    pub court_rejections: u32,
    /// Plan key: ... and under this sociability buys a Persuade.
    pub court_sociability: f32,
    /// Plan key: a pair refused, broken or failed is not tried again for this long.
    pub retry_days: u32,
    /// M16b phase 2: a Hostage adds `hostage_w × rel_w(over, captive)`.
    pub hostage_w: f32,
    /// M16b phase 2: a spinning corp's exec under this fabricates.
    pub fabricate_lawfulness: f32,
    /// M16b phase 2: a fabricated plant's price multiplier.
    pub fabricate_mult: f32,
    /// M16b phase 2: a contradicting holder exposes a fabrication a day with this p.
    pub expose_p: f32,
}

impl Default for LeverageCfg {
    fn default() -> Self {
        LeverageCfg {
            enabled: true,
            max_coercions: 128,
            lev_bias: LevBiasCfg::default(),
            demand_w: 1.0,
            secret_w: 1.0,
            backlash_report: 0.5,
            protection_weekly: 40,
            blackmail_weekly: 30,
            blackmail_lawfulness: 0.35,
            blackmail_gap: 0.2,
            blackmail_days: 30,
            secret_known_max: 5,
            secret_public_reach: 0.3,
            silence_lawfulness: 0.3,
            poach_threat_lawfulness: 0.3,
            debt_call_min: 3,
            fake_fee: 40,
            fake_p: 0.1,
            fake_days: 7,
            fake_wary_days: 0,
            parley_ratio: 0.3,
            parley_tribute: 30,
            persuade_affinity: 0.3,
            court_rejections: 2,
            court_sociability: 0.4,
            retry_days: 28,
            hostage_w: 1.5,
            fabricate_lawfulness: 0.35,
            fabricate_mult: 2.0,
            expose_p: 0.05,
        }
    }
}
