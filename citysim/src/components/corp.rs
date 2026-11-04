use super::*;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Niche {
    Food,
    Housing,
    Security,
    /// M13 D17: Clinics and Garages (Zetatech).
    Tech,
    /// Jobs & lives (§ 1, § 5): strip clubs, brothels, dollhouses, BD
    /// studios. Appended in phase 1; no kind sits in it until phase 4, and
    /// it stays out of `ALL` (the corp brain's niche loops) until then.
    Vice,
}

impl Niche {
    pub const ALL: [Niche; 4] = [Niche::Food, Niche::Housing, Niche::Security, Niche::Tech];

    /// Every niche, the appended `Vice` included (parsing).
    pub const ALL_JOBS2: [Niche; 5] = [Niche::Food, Niche::Housing, Niche::Security, Niche::Tech, Niche::Vice];

    /// Jobs & lives phase 4: the niches the corp loops read: `ALL`, and
    /// `Vice` last with `[vice]` on (`systems::vice::on`).
    pub fn live(vice: bool) -> &'static [Niche] {
        if vice {
            &Niche::ALL_JOBS2
        } else {
            &Niche::ALL
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Niche::Food => "Food",
            Niche::Housing => "Housing",
            Niche::Security => "Security",
            Niche::Tech => "Tech",
            Niche::Vice => "Vice",
        }
    }

    pub fn parse(s: &str) -> Option<Niche> {
        Niche::ALL_JOBS2.into_iter().find(|n| n.label() == s)
    }
}

impl fmt::Display for Niche {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A corp's standing order (phase 3's brain; phase 2 seeds every corp at `Hunker`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum CorpOrder {
    Grow,
    Squeeze,
    Undercut,
    Acquire,
    Secure,
    #[default]
    Hunker,
    Lobby,
    /// M14 (plan V31): research the focus track, build and staff Labs.
    Research,
    /// M14 (plan V31): one run a day from a Lab against the niche rival's
    /// Data (a dice contest on the Virt plane). Spends: a tier-2 fleet deck
    /// for the Lab when it has none (above the fleet reserve), logged as
    /// `AssetBought` "for {Lab} (VirtRaid)".
    VirtRaid,
}

impl CorpOrder {
    pub const ALL: [CorpOrder; 9] = [
        CorpOrder::Grow,
        CorpOrder::Squeeze,
        CorpOrder::Undercut,
        CorpOrder::Acquire,
        CorpOrder::Secure,
        CorpOrder::Hunker,
        CorpOrder::Lobby,
        CorpOrder::Research,
        CorpOrder::VirtRaid,
    ];
}

impl fmt::Display for CorpOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Something that happened to a corp since its last rescoring.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CorpShock {
    Robbed(i64),
    Extorted,
    EmployeeKilled,
    Bankrupt(EntityId),
    Strike,
    Undercut,
    BuildingLost,
    /// M12 D8: lost control of a district.
    LostDistrict,
    /// M12 D39: a gang raid on one of its buildings was won.
    Raided,
    /// M12 D32: a riot hit one of its buildings ...
    Rioted,
    /// ... or another building in a district where it owns some (half severity).
    RiotNearby,
    /// M14 (plan V34): a tech track dropped a tier.
    TechLost,
    /// M15 W28: competence fell by `talent_drop` in a day.
    TalentLost,
    /// M15 W29: a rival corp poached one of its staff.
    Poached,
    /// M16a (plan C24, phase 2): a contract strike in one of its districts.
    Trespass,
}

impl CorpShock {
    pub fn severity(self) -> f32 {
        match self {
            CorpShock::Robbed(c) if c > 100 => 0.6,
            CorpShock::Robbed(_) => 0.3,
            CorpShock::Extorted => 0.4,
            CorpShock::EmployeeKilled => 0.5,
            CorpShock::Bankrupt(_) => 0.5,
            CorpShock::Strike => 0.8,
            CorpShock::Undercut => 0.3,
            CorpShock::BuildingLost => 0.5,
            CorpShock::LostDistrict => 0.4,
            CorpShock::Raided => 0.7,
            CorpShock::Rioted => 0.9,
            CorpShock::RiotNearby => 0.45,
            CorpShock::TechLost => 0.6,
            CorpShock::TalentLost => 0.4,
            CorpShock::Poached => 0.3,
            CorpShock::Trespass => 0.3,
        }
    }
}

/// One `(order, niche)` score from the last rescoring, for the Corp panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CorpOrderScore {
    pub order: CorpOrder,
    pub niche: Niche,
    pub score: f32,
    pub considerations: Vec<Consideration>,
}

/// D20: a crime loss on an owned building, and the gang behind it if any.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CorpLoss {
    pub tick: Tick,
    pub coins: i64,
    pub gang: Option<EntityId>,
    /// The building it happened at (`Secure` contracts the newest first).
    #[serde(default)]
    pub building: Option<EntityId>,
}

pub(super) fn one_f32() -> f32 {
    1.0
}

/// A corporation: a faction that owns buildings, earns through them and pays
/// their upkeep. Its own entity, carrying only this component.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corp {
    pub name: String,
    pub niches: BTreeSet<Niche>,
    pub treasury: i64,
    /// The agent whose Personality the brain reads; `None` = a default.
    pub exec: Option<EntityId>,
    /// Sorted; redundant with `Building.owner`, kept for O(1) listing.
    pub buildings: Vec<EntityId>,
    #[serde(default)]
    pub order: CorpOrder,
    #[serde(default)]
    pub order_niche: Option<Niche>,
    #[serde(default)]
    pub order_since: Tick,
    #[serde(skip)]
    pub order_trace: Vec<CorpOrderScore>,
    /// Pending since the last rescoring. M15 phase 2 (determinism): saved,
    /// as `Law.shocks` (with `World::corp_rethink`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shocks: Vec<CorpShock>,
    /// Per niche: Food markup, Housing rent level, Security contract price.
    #[serde(default)]
    pub price_level: BTreeMap<Niche, f32>,
    /// Net coins per day, last 14, newest last.
    #[serde(default)]
    pub cashflow: VecDeque<i64>,
    /// D20: crime losses on owned buildings, last 14 days.
    #[serde(default)]
    pub loss_log: VecDeque<CorpLoss>,
    #[serde(default)]
    pub negative_since: Option<Tick>,
    /// M11 phase 5: the treasury at the last midnight close, before that
    /// night's upkeep: what solvency (`negative_since`) and restocking read,
    /// so a corp with a day's profit but less than a day's upkeep in the
    /// bank is not in the red (the upkeep lump is the intra-day trough).
    #[serde(default)]
    pub closing: i64,
    /// M11 phase 5: no upkeep before this tick (a newly incorporated corp).
    #[serde(default)]
    pub upkeep_grace_until: Option<Tick>,
    /// Security only: `(client building, until)`, sorted (phase 3).
    #[serde(default)]
    pub contracts: Vec<(EntityId, Tick)>,
    #[serde(default)]
    pub lobby_until: Option<Tick>,
    #[serde(default)]
    pub last_acquisition_tick: Option<Tick>,
    /// The Hit market (H18, plan D11): the last rival corp that wronged this
    /// one (a hostile buyout, a poached hand, an undercut), how and when.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wronged: Option<(EntityId, Tick, crate::contract::Wrong)>,
    /// D38: the seeding row (CSV columns); spinoffs and incorporations `None`.
    #[serde(default)]
    pub slot: Option<u16>,
    /// The `cash` denominator: `treasury_initial`, or the treasury at founding.
    #[serde(default)]
    pub treasury_ref: i64,
    /// Net coins through `World::purse_add` today; rolled into `cashflow`.
    #[serde(default)]
    pub cashflow_today: i64,
    /// D22: wages paid × this (Food Squeeze 0.9).
    #[serde(default = "one_f32")]
    pub wage_mult: f32,
    /// D22: Housing Squeeze evicts sooner.
    #[serde(default)]
    pub evict_days_override: Option<u8>,
    #[serde(default)]
    pub share_hist: BTreeMap<Niche, VecDeque<f32>>,
    /// D23.
    #[serde(default)]
    pub last_build_tick: Option<Tick>,
    /// D41: a branch of an outside parent (no behaviour in M11).
    #[serde(default)]
    pub parent: Option<ParentId>,
    #[serde(default)]
    pub outside_treasury: i64,
    /// D49: `Dictator` = `exec` decides; no Board in M11.
    #[serde(default)]
    pub governance: Governance,
    /// God `SetCorpOrder`: the order holds until this tick (the rescores
    /// keep the trace but do not switch).
    #[serde(default)]
    pub pinned_until: Option<Tick>,
    /// M12 fix pass: the tick of the last raid or riot that took from one of
    /// its buildings; for `[corps] raided_days` the Secure and Lobby input
    /// `losses` floors at `raided_losses` (the raided corp hardens).
    #[serde(default)]
    pub raided_at: Option<Tick>,
    /// M14 (plan V21): the tech tree.
    #[serde(default = "crate::virt::Tech::unset")]
    pub tech: crate::virt::Tech,
    /// M14 (plan V25): the ICE on this corp's Ledger node.
    #[serde(default)]
    pub ledger_ice: crate::virt::SecurityProfile,
    /// M14 (plan V27): ICE spend per day, last 30, newest last.
    #[serde(default)]
    pub ice_spend: VecDeque<i64>,
    #[serde(default)]
    pub ice_spend_today: i64,
    /// M14 (plan V27, phase 3): Virt losses on its nodes, `(tick, node)`,
    /// last 14 days, newest last (Secure raises those first).
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub virt_losses: VecDeque<(Tick, crate::virt::NodeId)>,
    /// M14 phase 3 (procurement budget): coins spent on Data today.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub data_bought_today: i64,
    /// M15 W28: the exec's and staff's skills against the city means
    /// (`competence::daily`); `comp_ref` until the first pass.
    #[serde(default = "default_competence")]
    pub competence: f32,
    /// M15 W40: the corp's honour at the last 14 midnights, newest last
    /// (Spin's `honour_drop`).
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub honour_hist: VecDeque<f32>,
    /// M15 W40 (orchestrator deviation: Spin is a side spend, not a
    /// corp-wide order): spinning since this midnight, while the corp's
    /// Spin score clears `[news] spin_min` (`news::rescore_spin`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spin_since: Option<Tick>,
    /// The last midnight's Spin score and its considerations (the panel).
    #[serde(default, skip_serializing_if = "is_zero_f32_c")]
    pub spin_score: f32,
    #[serde(skip)]
    pub spin_trace: Vec<Consideration>,
    // --- The Real economy phase 2 (docs/ECONOMY_V2.md § 5, plan E15-E18):
    // written only behind `econ::wages_on`.
    /// E15: the revenue rule's wage multiplier (`wages::daily`), on top of
    /// M11's Squeeze `wage_mult`.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub wage_rev: f32,
    /// E16: today's revenue (taxed inflows net of tax, exports) and payroll
    /// (gross wages, the exec's), rolled into 7-day windows at midnight.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub rev_today: i64,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub pay_today: i64,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub rev: VecDeque<i64>,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub pay: VecDeque<i64>,
    /// E18: days in a row under `hire_below × P*`, and over `fire_above × P*`.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub hire_days: u8,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub fire_days: u8,
    /// The jobs round: asset imports fronted today (`[economy2]
    /// asset_import_per_day`), zeroed at the midnight roll; wages on only.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub import_today: i64,
    /// Class pay (docs/CLASS_PAY.md P3): today's graded premium (the part of
    /// a graded wage above the labour rate), paid out of capital and profit,
    /// not the labour pool; rolled into a 7-day window at midnight. Written
    /// only with class pay on.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub prem_today: i64,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub prem: VecDeque<i64>,
    /// Jobs & lives phase 1 (§ 2, the Hq): the corp's head office where one
    /// stands (`industry::daily` keeps it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub office: Option<EntityId>,
    /// Round 4 T6: the standing order the exec's last told call kept
    /// (`"keep Grow in Tech"`); a shock rescore's same call is not news
    /// again until the order changes (`corp_brain::rescore`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept_call: Option<String>,
}

pub(super) fn is_zero_f32_c(v: &f32) -> bool {
    *v == 0.0
}

pub(super) fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

impl Corp {
    /// The highest `price_level` the corp charges in any niche; 0 with none
    /// (M12 review: one fold for the strike target and the riot grievance; a
    /// corp with no price levels never outranks a pricing one as a strike
    /// target, and the riot reads it through `max(1.0)`, no grievance).
    pub fn max_price_level(&self) -> f32 {
        self.price_level.values().copied().fold(0.0f32, f32::max)
    }

    pub fn new(name: String, niches: BTreeSet<Niche>, treasury: i64, exec: Option<EntityId>) -> Corp {
        let price_level = niches.iter().map(|&n| (n, 1.0)).collect();
        Corp {
            name,
            niches,
            treasury,
            exec,
            buildings: Vec::new(),
            order: CorpOrder::Hunker,
            order_niche: None,
            order_since: 0,
            order_trace: Vec::new(),
            shocks: Vec::new(),
            price_level,
            cashflow: VecDeque::new(),
            loss_log: VecDeque::new(),
            negative_since: None,
            closing: treasury,
            upkeep_grace_until: None,
            contracts: Vec::new(),
            lobby_until: None,
            last_acquisition_tick: None,
            last_wronged: None,
            slot: None,
            treasury_ref: treasury.max(1000),
            cashflow_today: 0,
            wage_mult: 1.0,
            evict_days_override: None,
            share_hist: BTreeMap::new(),
            last_build_tick: None,
            parent: None,
            outside_treasury: 0,
            governance: Governance::Dictator,
            pinned_until: None,
            raided_at: None,
            tech: crate::virt::Tech::default(),
            ledger_ice: crate::virt::SecurityProfile::default(),
            ice_spend: VecDeque::new(),
            virt_losses: VecDeque::new(),
            data_bought_today: 0,
            ice_spend_today: 0,
            competence: default_competence(),
            honour_hist: VecDeque::new(),
            spin_since: None,
            spin_score: 0.0,
            spin_trace: Vec::new(),
            wage_rev: 1.0,
            rev_today: 0,
            pay_today: 0,
            rev: VecDeque::new(),
            pay: VecDeque::new(),
            hire_days: 0,
            fire_days: 0,
            import_today: 0,
            prem_today: 0,
            prem: VecDeque::new(),
            office: None,
            kept_call: None,
        }
    }

    /// The niche's price level (1.0 when unset).
    pub fn level(&self, niche: Niche) -> f32 {
        self.price_level.get(&niche).copied().unwrap_or(1.0)
    }
}
