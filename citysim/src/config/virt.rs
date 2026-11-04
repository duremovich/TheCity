use super::*;

// ---------------------------------------------------------------------------
// M14 Data and Virt (docs/M14_VIRT.md, plan phase 1.2). Each section has an
// `off()`; `VirtCfg::off().enabled` is false and gates every M14 branch, the
// others hold the TOML's values (read only while `[virt]` is on).
// ---------------------------------------------------------------------------

/// M14 § 1: the plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VirtCfg {
    pub terminal_fee: i64,
    pub jacked_fight_mult: f32,
    pub dump_sanity: f32,
    /// Per deck tier.
    pub hop_ticks: Vec<u32>,
    pub contest_ticks: u32,
    pub hop_eps: f32,
    /// No free choice targets a node below this (orders may, plan V11).
    pub min_route_p: f32,
    pub public_links_tier: u8,
    pub access_links_tier: u8,
    pub trunk_links_tier: u8,
}

impl Default for VirtCfg {
    fn default() -> Self {
        VirtCfg {
            terminal_fee: 3,
            jacked_fight_mult: 0.3,
            dump_sanity: 0.1,
            hop_ticks: vec![8, 5, 3],
            contest_ticks: 5,
            hop_eps: 0.01,
            min_route_p: 0.25,
            public_links_tier: 1,
            access_links_tier: 1,
            trunk_links_tier: 2,
        }
    }
}

/// M14 § 2: decks and the hacking skill's seed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecksCfg {
    pub deck_shop_min: f32,
    /// Plan V35: `hacking = hack_seed_scale x U(0,1)^3`.
    pub hack_seed_scale: f32,
    pub hack_drift: f32,
    pub upgrade_frac: f32,
    pub gang_deck_floor: i64,
    pub gang_decks_max: u32,
}

impl Default for DecksCfg {
    fn default() -> Self {
        DecksCfg {
            deck_shop_min: 0.4,
            hack_seed_scale: 0.8,
            hack_drift: 0.01,
            upgrade_frac: 0.7,
            gang_deck_floor: 900,
            gang_decks_max: 3,
        }
    }
}

/// `[ice] ice_seed`: the ICE seeded on a building node by kind (plan V26).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceSeed {
    pub lab: u8,
    pub security_office: u8,
    pub farm: u8,
    pub market: u8,
    pub hideout: u8,
    pub clinic: u8,
    pub garage: u8,
    pub bar: u8,
    pub hotel: u8,
}

impl IceSeed {
    pub fn for_kind(&self, kind: BuildingKind) -> u8 {
        match kind {
            BuildingKind::Lab => self.lab,
            BuildingKind::SecurityOffice => self.security_office,
            BuildingKind::Farm => self.farm,
            BuildingKind::Market => self.market,
            BuildingKind::Hideout => self.hideout,
            BuildingKind::Clinic => self.clinic,
            BuildingKind::Garage => self.garage,
            BuildingKind::Bar => self.bar,
            BuildingKind::Hotel => self.hotel,
            _ => 0,
        }
    }
}

/// `[ice] act_ticks` per purpose (phase 2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActTicks {
    pub data: u32,
    pub ledger: u32,
    pub door: u32,
    pub robot: u32,
    pub camera: u32,
}

/// M14 § 3: ICE, its upkeep and the contest's outcomes. The contest step is
/// `[chrome] contest_step` (one value, plan 1.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceCfg {
    pub hack_w: f32,
    /// Per ICE tier 0..=3.
    pub ice_price: Vec<i64>,
    pub self_install_frac: f32,
    /// Per ICE tier 0..=3.
    pub ice_upkeep: Vec<i64>,
    pub ice_off_days: u8,
    pub ice_value_steps: Vec<i64>,
    pub ice_seed: IceSeed,
    pub alarm_hours: u32,
    pub fry_base: Vec<f32>,
    pub fry_gap: f32,
    pub fry_sanity: f32,
    pub fry_energy: f32,
    pub p_fry_kill: Vec<f32>,
    pub flatline_gap: f32,
    pub p_flatline: f32,
    pub fry_wear: u8,
    pub trace_p: Vec<f32>,
    pub stealth_w: f32,
    pub daze_ticks: u32,
    pub act_ticks: ActTicks,
    pub door_strip_cap: f32,
    pub door_cooldown_days: u32,
    /// M14 addendum (plan V65): a trace's attribution confidence after `h`
    /// hops between the portal and the target is `(1 - decay)^h`.
    pub trace_decay_per_hop: f32,
    /// Plan V65: below this confidence a trace names nobody (no Sighting,
    /// no report, no `last_seen`, the shock names no one).
    pub trace_floor: f32,
}

impl Default for IceCfg {
    fn default() -> Self {
        IceCfg {
            hack_w: 1.0,
            ice_price: vec![0, 150, 600, 2000],
            self_install_frac: 0.5,
            ice_upkeep: vec![0, 0, 1, 2],
            ice_off_days: 2,
            ice_value_steps: vec![200, 1500, 6000],
            ice_seed: IceSeed {
                lab: 2,
                security_office: 2,
                farm: 1,
                market: 1,
                hideout: 0,
                clinic: 1,
                garage: 1,
                bar: 0,
                hotel: 0,
            },
            alarm_hours: 48,
            fry_base: vec![0.0, 0.05, 0.25, 0.6],
            fry_gap: 1.0,
            fry_sanity: 0.3,
            fry_energy: 0.5,
            p_fry_kill: vec![0.0, 0.0, 0.05, 0.3],
            flatline_gap: 1.5,
            p_flatline: 0.8,
            fry_wear: 20,
            trace_p: vec![0.0, 0.2, 0.45, 0.7],
            stealth_w: 0.5,
            daze_ticks: 30,
            act_ticks: ActTicks { data: 30, ledger: 30, door: 10, robot: 15, camera: 5 },
            door_strip_cap: 0.5,
            door_cooldown_days: 3,
            trace_decay_per_hop: 0.35,
            trace_floor: 0.15,
        }
    }
}

impl IceCfg {
    /// `fry_base[ice]`, `p_fry_kill[ice]`, `trace_p[ice]` (0 past the table).
    pub fn at(v: &[f32], ice: u8) -> f32 {
        v.get(usize::from(ice)).copied().unwrap_or(0.0)
    }

    /// `ice_upkeep[tier]` (0 past the table).
    pub fn upkeep(&self, tier: u8) -> i64 {
        self.ice_upkeep.get(usize::from(tier)).copied().unwrap_or(0)
    }

    /// `ice_price[tier]` (`None` past the table).
    pub fn price(&self, tier: u8) -> Option<i64> {
        self.ice_price.get(usize::from(tier)).copied()
    }
}

/// M14 § 4: Data and Labs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataCfg {
    pub data_per_shift: f32,
    pub store_cap: u32,
    pub steal_units: Vec<u32>,
    pub data_price: i64,
    pub gang_data_keep: u32,
    pub backup_days: u32,
    pub ledger_frac: f32,
    pub ledger_cap: Vec<i64>,
    pub door_loss: i64,
    /// Plan: a seeded Lab's opening store in its focus track.
    pub seed_store: u32,
    /// M14 phase 3 (procurement budget): a corp's Data purchases in a day
    /// are capped at this × its closing treasury, and never take the
    /// treasury below `treasury_ref / 4` (M13's fleet-buy reserve).
    pub buy_budget_frac: f32,
}

impl Default for DataCfg {
    fn default() -> Self {
        DataCfg {
            data_per_shift: 3.0,
            store_cap: 2000,
            steal_units: vec![60, 150, 400],
            data_price: 3,
            gang_data_keep: 0,
            backup_days: 3,
            ledger_frac: 0.05,
            ledger_cap: vec![0, 400, 1500],
            door_loss: 100,
            seed_store: 300,
            buy_budget_frac: 0.05,
        }
    }
}

/// `[tech] requires`: the owner's tier in the kind's track a seller needs to
/// sell tier `t` (`requires[t - 1]`; a tier past the list is not gated).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TechRequires {
    pub implant: Vec<u8>,
    pub deck: Vec<u8>,
    pub robot: Vec<u8>,
    pub camera: Vec<u8>,
    pub motorcycle: Vec<u8>,
    pub car: Vec<u8>,
    pub truck: Vec<u8>,
    pub flyer: Vec<u8>,
    pub pack: Vec<u8>,
    pub bridge: Vec<u8>,
    /// Jobs & lives phase 3.
    pub weapon: Vec<u8>,
    /// Jobs & lives phase 4.
    pub clothing: Vec<u8>,
    /// Jobs & lives phase 5.
    pub recorder: Vec<u8>,
}

impl TechRequires {
    /// The list for an asset kind's class.
    pub fn of(&self, kind: AssetKind) -> &[u8] {
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

/// `[tech] focus_by_niche`: a corp's opening focus by its first niche.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusByNiche {
    pub food: String,
    pub housing: String,
    pub security: String,
    pub tech: String,
}

impl Default for FocusByNiche {
    fn default() -> Self {
        FocusByNiche {
            food: "Industry".into(),
            housing: "Industry".into(),
            security: "Deck".into(),
            tech: "Chrome".into(),
        }
    }
}

impl FocusByNiche {
    pub fn for_niche(&self, n: Niche) -> crate::virt::Track {
        let s = match n {
            Niche::Food => &self.food,
            Niche::Housing => &self.housing,
            Niche::Security => &self.security,
            // Jobs & lives: a Vice corp (phase 4) researches as Tech for now.
            Niche::Tech | Niche::Vice => &self.tech,
        };
        crate::virt::Track::parse(s).unwrap_or(crate::virt::Track::Industry)
    }
}

/// M14 § 5: the tech tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TechCfg {
    pub requires: TechRequires,
    /// Data to reach tier i.
    pub tier_cost: Vec<u32>,
    pub tier_coins: Vec<i64>,
    pub upkeep_data: Vec<u32>,
    pub upkeep_coins: Vec<i64>,
    pub decay_days: u8,
    pub research_rate: u32,
    pub orphan_cap: u8,
    pub industry_prod: Vec<f32>,
    pub focus_by_niche: FocusByNiche,
    /// `[chrome, deck, industry]` by `Corp.name`; every other corp `[1, 1, 1]`.
    pub seed: std::collections::BTreeMap<String, [u8; 3]>,
    /// Plan V47: coins per seeded Lab to its owner, out of nothing.
    pub seed_lab_grant: i64,
    /// M14 phase 5 (deviation from the spec's Research score): Research's
    /// tech gap reads the street tier (the best living corp's tier in the
    /// corp's focus track), not only niche rivals. Off = the spec.
    pub research_gap_street: bool,
    /// M14 phase 5: a floor under Research's `max lapse` consideration so a
    /// corp that is not lapsing can still score it. 0 = the spec.
    pub research_lapse_floor: f32,
    /// M14 phase 5: days a corp must have held Research before it builds a
    /// Lab (0 = the spec: on the first day).
    pub research_build_days: u32,
    /// M14 phase 5: Research is gated shut until the corp has this many days
    /// of cashflow on its books (0 = the spec: from day 0).
    pub research_min_days: u32,
}

impl Default for TechCfg {
    fn default() -> Self {
        TechCfg {
            requires: TechRequires {
                implant: vec![1, 2, 3],
                deck: vec![1, 2, 3],
                robot: vec![1, 2, 3],
                camera: vec![1, 2, 3],
                motorcycle: vec![1],
                car: vec![1, 2],
                truck: vec![2],
                flyer: vec![3],
                pack: vec![1, 1],
                bridge: Vec::new(),
                weapon: vec![1, 2, 3],
                clothing: Vec::new(),
                recorder: Vec::new(),
            },
            tier_cost: vec![0, 0, 400, 1200],
            tier_coins: vec![0, 0, 800, 2500],
            upkeep_data: vec![0, 0, 3, 8],
            upkeep_coins: vec![0, 0, 0, 0],
            decay_days: 7,
            research_rate: 40,
            orphan_cap: 2,
            industry_prod: vec![0.0, 0.0, 0.1, 0.2],
            focus_by_niche: FocusByNiche::default(),
            seed: [("Zetatech", [3, 1, 3]), ("Arasaka", [1, 3, 1]), ("Militech", [1, 2, 1])]
                .into_iter()
                .map(|(n, t)| (n.to_string(), t))
                .collect(),
            seed_lab_grant: 2000,
            research_gap_street: false,
            research_lapse_floor: 0.0,
            research_build_days: 0,
            research_min_days: 0,
        }
    }
}

impl TechCfg {
    pub fn upkeep_data_at(&self, tier: u8) -> u32 {
        self.upkeep_data.get(usize::from(tier)).copied().unwrap_or(0)
    }

    pub fn upkeep_coins_at(&self, tier: u8) -> i64 {
        self.upkeep_coins.get(usize::from(tier)).copied().unwrap_or(0)
    }
}

/// M14 § 6: the Hack goal and the orders (phases 2-3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HackCfg {
    pub hack_min: f32,
    pub hack_ref: i64,
    pub hack_flat: f32,
    pub hack_cooldown_days: u32,
    pub fried_cooldown_days: u32,
    pub virt_runners: u32,
    pub door_lead_hours: u32,
    pub door_hours: u32,
    pub blind_hours: u32,
    pub wipe_min: f32,
    pub stream_min: f32,
    pub order_score: f32,
    pub stat_hack_min: f32,
    /// Plan V66 (addendum's quiet and loud): a quiet run takes `quiet_take`
    /// of the units (or coins) and is traced at `quiet_trace` of the odds,
    /// and raises no alarm; a loud one `loud_take` and `loud_trace` (the
    /// trace capped at 1). `*_att` add to the runner's `att` (0: the
    /// contest table holds in both modes).
    pub quiet_take: f32,
    pub loud_take: f32,
    pub quiet_trace: f32,
    pub loud_trace: f32,
    pub quiet_att: f32,
    pub loud_att: f32,
    /// Plan V66: a freelancer runs loud when `(1 - lawfulness) x courage`
    /// reaches this.
    pub loud_min: f32,
    /// Round 4 T6: a corp's books keep the ledger runs on them this many
    /// days (0: no audit, the base's free money).
    pub ledger_audit_days: u32,
    /// Round 4 T6: the runner's run that is this many on the same books
    /// inside the window is matched and he is named (at least 2).
    pub ledger_audit_runs: u32,
}

impl Default for HackCfg {
    fn default() -> Self {
        HackCfg {
            hack_min: 0.3,
            hack_ref: 500,
            hack_flat: 0.0,
            hack_cooldown_days: 2,
            fried_cooldown_days: 10,
            virt_runners: 2,
            door_lead_hours: 2,
            door_hours: 6,
            blind_hours: 12,
            wipe_min: 0.6,
            stream_min: 0.2,
            order_score: 0.9,
            stat_hack_min: 0.5,
            quiet_take: 0.5,
            loud_take: 1.5,
            quiet_trace: 0.5,
            loud_trace: 2.0,
            quiet_att: 0.0,
            loud_att: 0.0,
            loud_min: 0.3,
            ledger_audit_days: 0,
            ledger_audit_runs: 2,
        }
    }
}

/// M14 § 6: faction databases (phases 2-3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DbCfg {
    pub db_cap: usize,
    pub sighting_days: u32,
}

impl Default for DbCfg {
    fn default() -> Self {
        DbCfg { db_cap: 32, sighting_days: 14 }
    }
}

// ---------------------------------------------------------------------------
// M15 Word and blood (docs/M15_WORD_AND_BLOOD.md; plan phase 1.2)
// ---------------------------------------------------------------------------

/// A value per `Deed` (spec deed-keyed tables); a missing key reads 0.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeedTable {
    pub killed: f32,
    pub assaulted: f32,
    pub robbed: f32,
    pub extorted: f32,
    pub stripped: f32,
    pub arrested: f32,
    pub married: f32,
    pub evicted: f32,
    pub struck: f32,
    pub raided: f32,
    pub founded: f32,
    pub avenged: f32,
    pub betrayed: f32,
    pub repaid: f32,
    pub poached: f32,
    /// M16a (plan C21): the `Hired` deed (a missing key reads 0).
    pub hired: f32,
    /// Real economy E29: the `Gave` deed (a missing key reads 0).
    pub gave: f32,
    /// M16b § 4: the `Blackmailed` deed (a missing key reads 0).
    pub blackmailed: f32,
    /// M16b phase 2: the `Rescued` deed (a missing key reads 0).
    pub rescued: f32,
    /// Lives at work phase 2 (§ 3.6): the work and household deeds (a
    /// missing key reads 0; left out of a save at 0, and zeroed by the off
    /// switches, so an off save is byte-identical).
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub employed: f32,
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub laid_off: f32,
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub born: f32,
    /// Lives at work phase 5 (§ 3.6): the exec's decision (left out of a
    /// save at 0; zeroed by the tower's off switches).
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub decided: f32,
    /// Pressure pass round 8 X4 (`OPEN-241`): a gang's new leader (left out
    /// of a save at 0).
    #[serde(skip_serializing_if = "is_zero_f32_deed")]
    pub took_over: f32,
}

fn is_zero_f32_deed(v: &f32) -> bool {
    *v == 0.0
}

impl DeedTable {
    /// Lives at work phase 2: the phase's keys at 0 (the off switches).
    pub fn without_household_deeds(&mut self) {
        self.employed = 0.0;
        self.laid_off = 0.0;
        self.born = 0.0;
    }
}

impl DeedTable {
    pub fn get(&self, d: crate::word::Deed) -> f32 {
        use crate::word::Deed;
        match d {
            Deed::Killed => self.killed,
            Deed::Assaulted => self.assaulted,
            Deed::Robbed => self.robbed,
            Deed::Extorted => self.extorted,
            Deed::Stripped => self.stripped,
            Deed::Arrested => self.arrested,
            Deed::Married => self.married,
            Deed::Evicted => self.evicted,
            Deed::Struck => self.struck,
            Deed::Raided => self.raided,
            Deed::Founded => self.founded,
            Deed::Avenged => self.avenged,
            Deed::Betrayed => self.betrayed,
            Deed::Repaid => self.repaid,
            Deed::Poached => self.poached,
            Deed::Hired => self.hired,
            Deed::Gave => self.gave,
            Deed::Blackmailed => self.blackmailed,
            Deed::Rescued => self.rescued,
            Deed::Employed => self.employed,
            Deed::LaidOff => self.laid_off,
            Deed::Born => self.born,
            Deed::Decided => self.decided,
            Deed::TookOver => self.took_over,
        }
    }

    /// The table from values in `Deed::ALL` order.
    pub fn of(v: [f32; 17]) -> DeedTable {
        let [killed, assaulted, robbed, extorted, stripped, arrested, married, evicted, struck, raided, founded, avenged, betrayed, repaid, poached, hired, gave] =
            v;
        DeedTable {
            killed,
            assaulted,
            robbed,
            extorted,
            stripped,
            arrested,
            married,
            evicted,
            struck,
            raided,
            founded,
            avenged,
            betrayed,
            repaid,
            poached,
            hired,
            gave,
            blackmailed: 0.0,
            rescued: 0.0,
            employed: 0.0,
            laid_off: 0.0,
            born: 0.0,
            decided: 0.0,
            took_over: 0.0,
        }
    }
}
