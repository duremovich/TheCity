use super::*;

/// M15 § 2: reputation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReputationCfg {
    pub half_life_days: f32,
    pub hop_w: [f32; 4],
    pub pool_w: f32,
    pub dread_scale: f32,
    pub honour_scale: f32,
    pub heat_scale: f32,
    pub fame_scale: f32,
    pub heat_wanted: f32,
    pub heat_arrest: f32,
    pub bribe_honour: f32,
    pub dread_w: DeedTable,
    pub honour_w: DeedTable,
    pub heat_w: DeedTable,
    pub own_bias: f32,
    pub heat_pressure_w: f32,
    pub harvest_dread_max: f32,
    pub acquire_honour_min: f32,
    pub bind_dread_w: f32,
}

impl Default for ReputationCfg {
    fn default() -> Self {
        //                        kil  ass  rob   ext  str  arr  mar  evi   stk  rai  fou  ave  bet   rep  poa  hir  gav
        let dread_w =
            DeedTable::of([1.0, 0.4, 0.15, 0.3, 0.2, 0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.8, 0.0, 0.0, 0.0, 0.3, 0.0]);
        // M16b § 4: `blackmailed` honour_w -0.4.
        let honour_w = DeedTable {
            blackmailed: -0.4,
            ..DeedTable::of([
                0.0, 0.0, -0.3, -0.2, -0.6, 0.0, 0.1, -0.2, 0.0, 0.0, 0.0, 0.4, -1.0, 0.3, -0.1, -0.2, 0.2,
            ])
        };
        let heat_w =
            DeedTable::of([0.5, 0.2, 0.2, 0.1, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.4, 0.0]);
        ReputationCfg {
            half_life_days: 7.0,
            hop_w: [1.0, 0.8, 0.6, 0.4],
            pool_w: 2.0,
            dread_scale: 3.0,
            honour_scale: 3.0,
            heat_scale: 2.0,
            fame_scale: 20.0,
            heat_wanted: 0.6,
            heat_arrest: 0.2,
            bribe_honour: 0.5,
            dread_w,
            honour_w,
            heat_w,
            own_bias: 0.3,
            heat_pressure_w: 0.8,
            harvest_dread_max: 0.6,
            acquire_honour_min: 0.3,
            bind_dread_w: 0.5,
        }
    }
}

/// M15 § 2: appearance and taste per audience (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TasteCfg {
    pub chrome_ref: u8,
    pub first_look_w: f32,
    pub meet_gap: f32,
    pub meet_taste_w: f32,
    pub corp: crate::word::Taste,
    pub street: crate::word::Taste,
    pub dreg: crate::word::Taste,
    pub gang: crate::word::Taste,
    pub purist: crate::word::Taste,
}

impl Default for TasteCfg {
    fn default() -> Self {
        use crate::word::Taste;
        let t = |dress, chrome, own_colours, rival_colours| Taste { dress, chrome, own_colours, rival_colours };
        TasteCfg {
            chrome_ref: 3,
            first_look_w: 0.1,
            meet_gap: 0.5,
            meet_taste_w: 0.3,
            corp: t(0.6, 0.0, 0.3, -0.3),
            street: t(0.1, 0.2, 0.2, -0.2),
            dreg: t(-0.2, 0.1, 0.0, 0.0),
            gang: t(0.0, 0.3, 0.5, -0.8),
            purist: t(-0.2, -1.0, 0.5, -0.5),
        }
    }
}

/// M15 § 2: the Purist creed (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreedsCfg {
    pub seed_purist: bool,
    pub purist_name: String,
    pub purist_treasury: i64,
    pub creed_tolerance: u8,
    pub purist_chrome: f32,
    pub tithe_frac: f32,
    /// Plan deviation (phase 2 review): a creed spreads through people. A
    /// Purist gang takes a recruit only along a Friend, Family, Parent or
    /// Spouse edge to a member, except its first `founders` members, who
    /// may come as any gang's bootstrap recruits do (nearest Hideout).
    pub founders: usize,
}

impl Default for CreedsCfg {
    fn default() -> Self {
        CreedsCfg {
            seed_purist: false,
            purist_name: "The Unplugged".to_string(),
            purist_treasury: 50,
            creed_tolerance: 0,
            purist_chrome: 0.3,
            tithe_frac: 0.6,
            founders: 3,
        }
    }
}

/// M15 § 3 `rel_w` plus plan rows `leader`, `comrade` (W15).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RelWeights {
    pub spouse: f32,
    pub parent: f32,
    pub family: f32,
    pub friend: f32,
    pub own: f32,
    pub leader: f32,
    pub comrade: f32,
}

impl Default for RelWeights {
    fn default() -> Self {
        RelWeights { spouse: 1.0, parent: 1.0, family: 0.9, friend: 0.6, own: 0.8, leader: 0.7, comrade: 0.0 }
    }
}

/// M15 § 3: grudges (phase 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrudgesCfg {
    pub grudge_min: f32,
    /// Shadow V4 bug 15: the safety a holder loses on hearing of a Hit on
    /// himself.
    pub marked_safety_drop: f32,
    pub grudge_decay: f32,
    pub beat_settle: f32,
    pub inherit_min: f32,
    pub inherit_frac: f32,
    pub settle_keep_days: u32,
    pub rel_w: RelWeights,
    pub guard_hours: u32,
    pub vendetta_norm: f32,
    pub vendetta_open: f32,
    pub vendetta_close: f32,
    /// W17.
    pub fight_grudge_min: f32,
    /// M15 phase 5: days a god `DeclareVendetta` holds the feud open.
    pub declared_days: u32,
}

impl Default for GrudgesCfg {
    fn default() -> Self {
        GrudgesCfg {
            grudge_min: 0.15,
            marked_safety_drop: 0.4,
            grudge_decay: 0.01,
            beat_settle: 0.5,
            inherit_min: 0.4,
            inherit_frac: 0.6,
            settle_keep_days: 30,
            rel_w: RelWeights::default(),
            guard_hours: 6,
            vendetta_norm: 3.0,
            vendetta_open: 0.5,
            vendetta_close: 0.2,
            fight_grudge_min: 0.5,
            declared_days: 30,
        }
    }
}

/// M15 § 5: social skills (phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsCfg {
    pub skill_scale: f32,
    pub rarity_exp: f32,
    pub inherit_skill: f32,
    pub skill_drift: f32,
    pub skill_rust: f32,
    pub knowledge_work: f32,
    /// W25.
    pub secondary_scale: f32,
}

impl Default for SkillsCfg {
    fn default() -> Self {
        SkillsCfg {
            skill_scale: 1.0,
            rarity_exp: 4.0,
            inherit_skill: 0.5,
            skill_drift: 0.005,
            skill_rust: 0.001,
            knowledge_work: 0.0005,
            secondary_scale: 0.6,
        }
    }
}

/// M15 § 6: the move kinds' biases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoveBias {
    pub persuade: f32,
    pub intimidate: f32,
    pub deceive: f32,
    pub charm: f32,
}

impl Default for MoveBias {
    fn default() -> Self {
        MoveBias { persuade: 0.0, intimidate: 0.8, deceive: 0.0, charm: 0.2 }
    }
}

/// M15 § 6: the social move (phase 2; `contradict_conf` is read from phase 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MovesCfg {
    pub move_k: f32,
    pub bias: MoveBias,
    pub w_m: f32,
    pub w_r: f32,
    pub w_l: f32,
    pub p_min: f32,
    pub p_max: f32,
    pub chrome_might: f32,
    pub ally_might: f32,
    pub ally_cap: f32,
    pub backlash_courage: f32,
    pub backlash_fight: f32,
    pub contradict_conf: f32,
}

impl Default for MovesCfg {
    fn default() -> Self {
        MovesCfg {
            move_k: 4.0,
            bias: MoveBias::default(),
            w_m: 0.5,
            w_r: 0.6,
            w_l: 0.3,
            p_min: 0.05,
            p_max: 0.95,
            chrome_might: 0.2,
            ally_might: 0.1,
            ally_cap: 0.5,
            backlash_courage: 0.6,
            backlash_fight: 0.3,
            contradict_conf: 0.3,
        }
    }
}

/// M18 P3 (docs/M18_PLAYER.md, the board and talk): the talk moves a body
/// makes on another it stands beside (`player::talk`, rolled through
/// `moves::resolve`). The Default mirrors `assets/config.toml` (the slice
/// save was written before the section).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TalkCfg {
    /// Two bodies on the street this near (Chebyshev tiles) can talk (Chat
    /// and the moves); inside, the same building.
    pub street_tiles: u16,
    /// Befriend (a Charm): a success moves the pair's edge by these.
    pub befriend_affinity: f32,
    pub befriend_trust: f32,
    /// Any refused move: the pair's affinity moves by this.
    pub refused_affinity: f32,
    /// Ask for a loan (a Persuade, `Stake::Coins`): this many coins, out of
    /// what the lender holds above `loan_keep` (a debt on the edge).
    pub loan_coins: i64,
    pub loan_keep: i64,
    /// Lean on them (an Intimidate, `Stake::Coins`): up to this many coins.
    pub threat_coins: i64,
    /// A body asked (any move, either outcome) is not asked again by the
    /// same body for this many hours.
    pub again_hours: u64,
}

impl Default for TalkCfg {
    fn default() -> Self {
        TalkCfg {
            street_tiles: 2,
            befriend_affinity: 0.15,
            befriend_trust: 0.05,
            refused_affinity: -0.05,
            loan_coins: 5,
            loan_keep: 10,
            threat_coins: 8,
            again_hours: 12,
        }
    }
}

/// M15 § 7: Feeds, stories and Spin (phase 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewsCfg {
    pub reach_base: f32,
    pub reach_per_reporter: f32,
    pub story_reach: f32,
    pub stories_per_day: u8,
    /// Shadow V4 bug 21: an organic deed runs in at most this many Feeds
    /// within `RECENT_DAYS` (0 = any number).
    pub outlets_per_deed: u8,
    pub ad_rate: i64,
    pub plant_price: i64,
    pub bury_price: i64,
    pub bury_days: u32,
    pub press_w: f32,
    pub press_loyalty: f32,
    pub press_cap: f32,
    pub censor_lawfulness: f32,
    /// Plan key (W36): `Register`'s per-capita target for a Feed
    /// (`founding::choose_kind`), as `residents_per_bar` for a Bar.
    pub residents_per_feed: u32,
    /// Orchestrator deviation (W40): Spin is a side spend; a corp spins
    /// while its Spin score (the spec's considerations plus
    /// `order_flat.spin`) is at least this.
    pub spin_min: f32,
}

impl Default for NewsCfg {
    fn default() -> Self {
        NewsCfg {
            reach_base: 0.2,
            reach_per_reporter: 0.15,
            story_reach: 0.8,
            stories_per_day: 3,
            outlets_per_deed: 2,
            ad_rate: 3,
            plant_price: 120,
            bury_price: 60,
            bury_days: 3,
            press_w: 0.3,
            press_loyalty: 0.1,
            press_cap: 0.15,
            censor_lawfulness: 0.3,
            residents_per_feed: 1000,
            spin_min: 0.2,
        }
    }
}
