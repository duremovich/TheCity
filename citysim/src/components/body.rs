use super::*;

/// Derived, never saved: rebuilt on load and by `assets::rekit` after any
/// change (plan D3).
#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    pub vehicle: Option<EntityId>,
    pub fighting: f32,
    pub stealth: f32,
    pub reflex: f32,
    pub strength: f32,
    pub sight: u8,
    pub armour: f32,
    pub walk_mult: f32,
    /// Σ sanity_cost of installed chrome (bricked and failed included).
    pub load: f32,
    pub chrome_value: i64,
    /// Installed tiers in Arms, Eyes, Skin.
    pub visible: u8,
    /// Coarse GotoTimed multiplier of the vehicle (phase 2; 1.0 until then).
    pub timed_mult: f32,
    /// `min(1, (visible + vehicle tier, a flyer as 3) / 9)`.
    pub flash: f32,
    /// Plan D3: the kind of the vehicle being driven, cached from `World::trips`.
    pub driving: Option<AssetKind>,
    /// Plan D3: any implant installed (bricked or failed included).
    pub chrome: bool,
    /// M14 V36: the deck carried (lowest id, condition > 0).
    pub deck: Option<EntityId>,
    /// M14 V36: its effective tier (`assets::eff_tier`), 0 without one.
    pub deck_tier: u8,
    /// Jobs & lives phase 3 (§ 4): the carried weapon's effective tier
    /// (the best one, condition > 0), 0 unarmed.
    pub weapon: u8,
    /// The combat model C1: that weapon's `[[weapon_kinds]]` row (0 with
    /// `weapon` 0: read only when armed).
    pub weapon_kind: u8,
}

/// The combat model C1 (docs/COMBAT.md § 2): a body region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Region {
    Head,
    Torso,
    ArmL,
    ArmR,
    LegL,
    LegR,
}

impl Region {
    pub const ALL: [Region; 6] = [Region::Head, Region::Torso, Region::ArmL, Region::ArmR, Region::LegL, Region::LegR];

    pub fn is_arm(self) -> bool {
        matches!(self, Region::ArmL | Region::ArmR)
    }

    pub fn is_leg(self) -> bool {
        matches!(self, Region::LegL | Region::LegR)
    }

    pub fn label(self) -> &'static str {
        match self {
            Region::Head => "head",
            Region::Torso => "body",
            Region::ArmL => "left arm",
            Region::ArmR => "right arm",
            Region::LegL => "left leg",
            Region::LegR => "right leg",
        }
    }
}

/// C1: a wound's severity (`[combat]` tables are indexed by it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Graze,
    Wound,
    Grave,
}

impl Severity {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Graze => "grazed",
            Severity::Wound => "wounded",
            Severity::Grave => "gravely wounded",
        }
    }

    pub fn down(self) -> Option<Severity> {
        match self {
            Severity::Graze => None,
            Severity::Wound => Some(Severity::Graze),
            Severity::Grave => Some(Severity::Wound),
        }
    }
}

/// C1: one wound on a body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wound {
    pub region: Region,
    pub severity: Severity,
    /// Draining the blood meter (`combat::hourly`).
    pub bleeding: bool,
    pub since: Tick,
    /// Stabilised at a Clinic (the M16b Stabilise): it stops bleeding.
    #[serde(default)]
    pub treated: bool,
    pub by: Option<EntityId>,
    /// A bleed-out from it counts as a Murder by `by` (an attacker who is
    /// no guard dealt it to the one he attacked).
    #[serde(default)]
    pub murder: bool,
    /// C2: the tick it heals (`since` + `heal_days`; treatment shortens what
    /// is left by `treated_heal_mult`).
    #[serde(default)]
    pub heal_at: Tick,
    /// C2: first aid given (a friend, kin or a gang-mate beside the body).
    #[serde(default)]
    pub aided: bool,
}

/// C2 (docs/COMBAT.md § 2): what a healed wound leaves on a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Mark {
    /// A scar: sim state, seen on the body; no effect.
    Scar,
    /// A Grave wound that healed badly: the region reads as a Wound for good.
    Maimed,
    /// A lost limb: the region reads as a Grave until a ripperdoc fits chrome.
    Lost,
}

impl Mark {
    pub fn label(self) -> &'static str {
        match self {
            Mark::Scar => "scarred",
            Mark::Maimed => "maimed",
            Mark::Lost => "lost",
        }
    }
}

/// C1 (docs/COMBAT.md § 2): the wounds a body carries and its blood meter
/// (1 full; at 0 the body goes down, the M16b `wounded` state). Removed
/// when the last wound heals and the blood is full.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wounds {
    pub list: Vec<Wound>,
    pub blood: f32,
    /// C2: what healed wounds left for good (scars, maims, lost limbs); the
    /// component stays while any is held.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<(Region, Mark)>,
}

impl Default for Wounds {
    fn default() -> Self {
        Wounds { list: Vec::new(), blood: 1.0, marks: Vec::new() }
    }
}

impl Wounds {
    /// The worst severity on `region`, if any: its open wounds, a maim (a
    /// Wound for good) and a lost limb (a Grave until chrome is fitted).
    pub fn worst(&self, region: Region) -> Option<Severity> {
        let marks = self.marks.iter().filter(|m| m.0 == region).filter_map(|m| match m.1 {
            Mark::Scar => None,
            Mark::Maimed => Some(Severity::Wound),
            Mark::Lost => Some(Severity::Grave),
        });
        self.list.iter().filter(|w| w.region == region).map(|w| w.severity).chain(marks).max()
    }

    /// The open wounds' worst on `region` (the marks apart).
    pub fn open(&self, region: Region) -> Option<Severity> {
        self.list.iter().filter(|w| w.region == region).map(|w| w.severity).max()
    }

    pub fn has_mark(&self, region: Region, mark: Mark) -> bool {
        self.marks.contains(&(region, mark))
    }
}

impl Default for Kit {
    fn default() -> Self {
        Kit {
            vehicle: None,
            fighting: 0.0,
            stealth: 0.0,
            reflex: 0.0,
            strength: 0.0,
            sight: 0,
            armour: 0.0,
            walk_mult: 1.0,
            load: 0.0,
            chrome_value: 0,
            visible: 0,
            timed_mult: 1.0,
            flash: 0.0,
            driving: None,
            chrome: false,
            deck: None,
            deck_tier: 0,
            weapon: 0,
            weapon_kind: 0,
        }
    }
}

impl Kit {
    /// No vehicle, no implant, not driving: every Kit term is skipped.
    pub fn is_bare(&self) -> bool {
        self.vehicle.is_none() && !self.chrome && self.driving.is_none()
    }
}

/// Body stats (plan D4) on every agent, all tiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    /// `U(0.2, 0.6)`; children inherit the parents' mean ± 0.05.
    pub strength: f32,
    pub reflex: f32,
    /// 0..=1, default 1.
    pub sanity: f32,
    /// 0..=1.
    pub addiction: f32,
    /// The last stim.
    pub last_use: Option<Tick>,
    /// In a cyberpsychotic episode (phase 3).
    pub episode_until: Option<Tick>,
    /// Plan D4: the last purchase (the Shop cooldown).
    #[serde(default)]
    pub last_shop: Option<Tick>,
}

/// For M15: stored daily, read by nobody in M13 (plan D5).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub dress: u8,
    pub chrome: u8,
    pub colours: Option<EntityId>,
    /// M15 (god `GrantSkill` with `suit`): a dress the daily appearance
    /// pass keeps instead of deriving it from class and wealth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dress_pin: Option<u8>,
}

/// What a corpse carried (plan D13).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Loot {
    pub coins: i64,
    pub food: u32,
    pub stims: u16,
    pub parts: u16,
}

impl Loot {
    pub fn is_empty(&self) -> bool {
        self.coins == 0 && self.food == 0 && self.stims == 0 && self.parts == 0
    }
}

/// What a shopper is buying, fixed at bind time (plan D29).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShopPick {
    pub kind: AssetKind,
    pub tier: u8,
    /// A used asset in the seller's stock, if the pick is one.
    pub used: Option<EntityId>,
    /// M14 V36: an `UpgradeDeck` of the carried deck to `tier`.
    #[serde(default)]
    pub upgrade: bool,
}

/// A vehicle trip in progress (plan D20; filled from phase 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trip {
    pub vehicle: EntityId,
    pub start: Tick,
    pub from: TilePos,
    pub road_tiles: u16,
    pub steps: u16,
    pub half: u16,
    pub mid: Option<TilePos>,
    pub chase: bool,
}

// ---------------------------------------------------------------------------
// Hunger as sickness (docs/HUNGER_SICKNESS.md § 2-3)
// ---------------------------------------------------------------------------

/// What a meal was, for the day's tally.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MealKind {
    Real,
    Synth,
    /// Scraps from a bin.
    Dive,
    /// A Mission's synth meal.
    Alms,
}

/// The day's meals and hungry hours (reset at midnight by `health::daily`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MealTally {
    pub real: u8,
    pub synth: u8,
    pub dive: u8,
    pub alms: u8,
    pub hungry_hours: u8,
}

/// Class pay (docs/CLASS_PAY.md): an adult's background, made only with
/// class pay on. `origin` is the class it was seeded, born or arrived into
/// (0 Dreg, 1 Sump, 2 Mid, 3 Spire, by Home tier); `education` (0..=1) is
/// the origin's mean plus a pure-hash spread, a child's its parents' mean.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schooling {
    pub origin: u8,
    pub education: f32,
}

/// A cancer rolled from high toxicity (§ 3).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cancer {
    /// 0..=1; death at 1.
    pub stage: f32,
    pub since: Tick,
    pub diagnosed: bool,
}

/// § 2: the meters, kept on every citizen while `[sickness]` is on (the
/// store is empty, and not saved, while it is off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Health {
    pub malnutrition: f32,
    pub toxicity: f32,
    pub cancer: Option<Cancer>,
    /// Rolled at midnight: no shift, no wage today.
    pub sick_today: bool,
    /// The last 14 days' sick days, bit 0 today (decision 4: 5 cost the job).
    pub sick_days: u16,
    /// § 4: the day a gang last shook this diver down at a bin (once a day).
    pub shaken_day: Option<u64>,
    pub today: MealTally,
    /// A child's last pantry unit was synth (review fix: a child's half-unit
    /// day eats no unit and carries the last one's quality).
    pub child_last_synth: bool,
}
