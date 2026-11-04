use super::*;

// ---------------------------------------------------------------------------
// M13 assets (docs/M13_ASSETS.md § 1, plan phase 1)
// ---------------------------------------------------------------------------

/// An implant's place in the body: one implant per slot.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Slot {
    Arms,
    Legs,
    Nerves,
    Eyes,
    Skin,
    /// Jobs & lives phase 5 (§ 5.2, D8): the behaviour chip's slot: its
    /// load only (`[vice] chip_load`), never sold at a Clinic's counter.
    Behaviour,
}

impl Slot {
    pub const ALL: [Slot; 6] = [Slot::Arms, Slot::Legs, Slot::Nerves, Slot::Eyes, Slot::Skin, Slot::Behaviour];

    pub fn label(self) -> &'static str {
        match self {
            Slot::Arms => "Arms",
            Slot::Legs => "Legs",
            Slot::Nerves => "Nerves",
            Slot::Eyes => "Eyes",
            Slot::Skin => "Skin",
            Slot::Behaviour => "Behaviour",
        }
    }
}

/// What an asset is. Appended only (saves name the variants).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum AssetKind {
    Motorcycle,
    Car,
    Truck,
    Flyer,
    Implant(Slot),
    Robot,
    Pack,
    /// M14 (plan D52): a Virt bridge, carried then planted at a building.
    /// Only its asset entry exists in M13.
    Bridge,
    /// M14 V36: a runner's deck (carried; a corp's fleet deck is posted at
    /// a Lab). Its tier is the run's `deck_eff` (effective tier).
    Deck,
    /// M14 V28: a camera posted at a building (inert until phase 3).
    Camera,
    /// Jobs & lives phase 3 (§ 4, Dylan's answer 1): a weapon, the first
    /// kind of the owned-item store (carried, kept at home, or stocked at
    /// a Gunsmith, a Hideout's armoury or the Precinct); its tier is the
    /// fight's term.
    Weapon,
    /// Jobs & lives phase 4 (§ 9, Dylan's answer 1): a clothing item of the
    /// store; the best one carried is worn (`Appearance.dress` reads its
    /// tier and condition), the rest kept at home.
    Clothing,
    /// Jobs & lives phase 5 (§ 5.3): a recorder carried by a gang member;
    /// a gang-mate's fight in its sight is a recorded deed.
    Recorder,
}

/// The lever and CSV grouping of an `AssetKind` (implants by class, not slot).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum AssetClass {
    Motorcycle,
    Car,
    Truck,
    Flyer,
    Implant,
    Robot,
    Pack,
    /// Plan D52.
    Bridge,
    /// M14 V36.
    Deck,
    /// M14 V28.
    Camera,
    /// Jobs & lives phase 3.
    Weapon,
    /// Jobs & lives phase 4.
    Clothing,
    /// Jobs & lives phase 5.
    Recorder,
}

impl AssetClass {
    pub const ALL: [AssetClass; 13] = [
        AssetClass::Motorcycle,
        AssetClass::Car,
        AssetClass::Truck,
        AssetClass::Flyer,
        AssetClass::Implant,
        AssetClass::Robot,
        AssetClass::Pack,
        AssetClass::Bridge,
        AssetClass::Deck,
        AssetClass::Camera,
        AssetClass::Weapon,
        AssetClass::Clothing,
        AssetClass::Recorder,
    ];

    /// Position in `ALL` (indexes `Levers::asset_tax`).
    pub fn index(self) -> usize {
        self as usize
    }
}

impl AssetKind {
    pub fn class(self) -> AssetClass {
        match self {
            AssetKind::Motorcycle => AssetClass::Motorcycle,
            AssetKind::Car => AssetClass::Car,
            AssetKind::Truck => AssetClass::Truck,
            AssetKind::Flyer => AssetClass::Flyer,
            AssetKind::Implant(_) => AssetClass::Implant,
            AssetKind::Robot => AssetClass::Robot,
            AssetKind::Pack => AssetClass::Pack,
            AssetKind::Bridge => AssetClass::Bridge,
            AssetKind::Deck => AssetClass::Deck,
            AssetKind::Camera => AssetClass::Camera,
            AssetKind::Weapon => AssetClass::Weapon,
            AssetKind::Clothing => AssetClass::Clothing,
            AssetKind::Recorder => AssetClass::Recorder,
        }
    }

    /// Jobs & lives phase 3 (Dylan's answer 1): an owned item (carried on
    /// the body, kept at home, stocked): weapons and (phase 4) clothing.
    pub fn is_item(self) -> bool {
        matches!(self, AssetKind::Weapon | AssetKind::Clothing)
    }

    /// Motorcycle, Car, Truck, Flyer.
    pub fn is_vehicle(self) -> bool {
        matches!(self, AssetKind::Motorcycle | AssetKind::Car | AssetKind::Truck | AssetKind::Flyer)
    }

    /// A vehicle that keeps to the ground (not the Flyer).
    pub fn is_road_vehicle(self) -> bool {
        self.is_vehicle() && self != AssetKind::Flyer
    }

    pub fn is_implant(self) -> bool {
        matches!(self, AssetKind::Implant(_))
    }

    pub fn label(self) -> &'static str {
        match self {
            AssetKind::Motorcycle => "Motorcycle",
            AssetKind::Car => "Car",
            AssetKind::Truck => "Truck",
            AssetKind::Flyer => "Flyer",
            AssetKind::Implant(Slot::Arms) => "Arms implant",
            AssetKind::Implant(Slot::Legs) => "Legs implant",
            AssetKind::Implant(Slot::Nerves) => "Nerves implant",
            AssetKind::Implant(Slot::Eyes) => "Eyes implant",
            AssetKind::Implant(Slot::Skin) => "Skin implant",
            AssetKind::Implant(Slot::Behaviour) => "Behaviour chip",
            AssetKind::Robot => "Security robot",
            AssetKind::Pack => "Pack",
            AssetKind::Bridge => "Bridge",
            AssetKind::Deck => "Deck",
            AssetKind::Camera => "Camera",
            AssetKind::Weapon => "Weapon",
            AssetKind::Clothing => "Clothing",
            AssetKind::Recorder => "Recorder",
        }
    }
}

/// Jobs & lives phase 3 (Dylan's answer 1): how an owned item came to its
/// holder, and who made, sold, took or seized it.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ProvKind {
    /// Made at a Gunsmith (`by` the workshop).
    Made,
    /// Sold legally (`by` the seller building: a Gunsmith, a Boutique).
    Sold,
    /// Taken by a pickpocket, a mugger, a burglar or a fight's winner
    /// (`by` the thief).
    Stolen,
    /// Landed in a crate (`by` the receiving gang).
    Smuggled,
    /// Sold by a gang's dealer (`by` the dealer).
    Dealt,
    /// Seized by the law (`by` the Precinct).
    Seized,
    /// A god's grant.
    Granted,
}

/// Jobs & lives phase 5 (§ 5.2, D9): the trait an agent keeps after its
/// behaviour chip came out (like `CampRaised`): the holder, the days the
/// chip was in, and whether it was a captive's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chipped {
    pub holder: Option<EntityId>,
    pub days: u32,
    pub captive: bool,
}

/// Lives at work phase 6 (§ 4.3): the trade an agent knows, written at
/// each worked shift: `days` worked in `trade` (the profession once at
/// `[careers] profession_min_days`), and a different role's run `next`
/// that replaces it on reaching the same. Survives a layoff, a term and a move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profession {
    pub trade: Role,
    pub days: u32,
    /// The day of the last shift counted (one a day).
    pub last_day: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<(Role, u16)>,
}

/// Lives at work phase 6 (§ 4.4; the spec's `Trait::Retired`: no `Trait`
/// enum exists): a retired agent, out of the hiring pool until his coins
/// fall under `[careers] unretire_coins`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Retired {
    pub since: Tick,
}

/// The pressure pass phase 1 (OPEN-178, `systems::misery`): a body's misery
/// bookkeeping, inserted when misery first touches it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Misery {
    /// Days of the shifts skipped, within `[misery] absence_window_days`.
    pub absences: Vec<u32>,
    /// Consecutive midnights below `[misery] quit_mood` while employed.
    pub low_days: u16,
    /// The day of the last brawl started (one a day).
    pub brawl_day: Option<u32>,
    /// Pressure pass F3: the shift key a midnight roll chose to skip, acted
    /// out at the shift's moment (`misery::hourly`: the body sets off or
    /// wakes, then does not go) for a body; `None` once acted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_key: Option<i64>,
    /// F3: the misery the midnight roll read (the moment's event names it).
    #[serde(default)]
    pub skip_misery: f32,
    /// F3: the skipped shift's end: the Unwind goal's skip flat until then.
    #[serde(default)]
    pub skip_until: Tick,
    /// F3: out of every hire until this tick (a firing's or a quit's idle days).
    #[serde(default)]
    pub idle_until: Tick,
    /// F3: a work reputation until this tick: a hire pass passes the
    /// candidate over on `stain_p` (a pure hash of the agent and the day).
    #[serde(default)]
    pub stain_until: Tick,
    #[serde(default)]
    pub stain_p: f32,
    /// F3: `(employer, until)`: the employer who fired this worker does not
    /// take him back before the tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fired_from: Option<(EntityId, Tick)>,
    /// The pressure pass phase 2 (care): the tick a lost fight's wound was
    /// last seen to (a Clinic's or a back room's patch-up).
    #[serde(default)]
    pub patched_at: Tick,
    /// Round 6 V6 (OPEN-217): the tick of an off-screen drink (a Bar's, a
    /// venue's bar), the mark the door reads where the Statistical tier
    /// keeps no `Socialised` memory (`misery::influence`); 0 none.
    #[serde(default)]
    pub drank_at: Tick,
    /// Round 7 W6 (OPEN-217): the tick a lost bout hurt him (a wound to
    /// have seen to, `customers::hurt`, as a lost fight's `Lost`; a bout is
    /// sanctioned, so no `Lost` memory); 0 none.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub hurt_at: Tick,
    /// Round 8 X3 (`OPEN-247`): the tick of the last sentence of `[careers]
    /// record_min_days` or more (the record a post that reads one reads,
    /// `careers::record_ok`); 0 none.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub record_at: Tick,
    /// Round 4 T5 (`systems::search`): the day's search (a body's): the day
    /// it is for, the doors asked, the door he is making for and the Friend
    /// who told him of it; `None` once no search has been made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<SearchDay>,
    /// Round 4 T5: the jobless spell (its first day) whose Friends were told
    /// "nobody knew of an opening" (a later day's ask is told only with a word).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_spell: Option<u64>,
}

/// Round 4 T5: one body's search day (`Misery.search`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchDay {
    pub day: u64,
    /// Sent home from the shape-up today: a search day whatever his idle days.
    #[serde(default)]
    pub sent_home: bool,
    /// The Friends were asked today (one line).
    #[serde(default)]
    pub asked: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tried: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead: Option<EntityId>,
    /// No door left today (the line said so).
    #[serde(default)]
    pub done: bool,
}

/// Jobs & lives phase 3: an item's provenance (its last change of hands).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Provenance {
    pub how: ProvKind,
    pub by: Option<EntityId>,
    pub at: Tick,
}

/// A traded good. Food lives in `Building.stock_food`; Stims and Parts in
/// `Building.stock_goods[good as usize - 1]` (plan D6).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Good {
    Food,
    Stims,
    Parts,
    /// Jobs & lives phase 1 (D6): packed Food, made at a Packing Plant
    /// (`Building.stock_x[0]`).
    Rations,
    /// Jobs & lives (D6): braindance units (phase 5; `stock_x[1]`).
    Braindance,
    /// Jobs & lives (D6, D10): organs (phase 6; `stock_x[2]`).
    Organs,
}

impl Good {
    pub const ALL: [Good; 3] = [Good::Food, Good::Stims, Good::Parts];

    /// Jobs & lives phase 1: every good, the appended ones included.
    pub const ALL_JOBS2: [Good; 6] =
        [Good::Food, Good::Stims, Good::Parts, Good::Rations, Good::Braindance, Good::Organs];

    pub fn label(self) -> &'static str {
        match self {
            Good::Food => "Food",
            Good::Stims => "Stims",
            Good::Parts => "Parts",
            Good::Rations => "Rations",
            Good::Braindance => "Braindance",
            Good::Organs => "Organs",
        }
    }
}

/// Where an asset is. Write it only through `assets::set_loc` (plan D2).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AssetLoc {
    /// At a building (its door): Homes, workplaces, Garages, Hideouts.
    Parked(EntityId),
    /// Being driven by an agent.
    InUse(EntityId),
    /// A pack (or a bridge) on an agent.
    Carried(EntityId),
    /// Chrome in a body (a living agent or a corpse).
    Installed(EntityId),
    /// A robot guarding a building.
    Posted(EntityId),
    /// For sale at a Clinic, Garage or Security Office, or a gang's take at its Hideout.
    Stock(EntityId),
    /// Taken by an unbound Abducted hole.
    Limbo(HoleId),
}

impl AssetLoc {
    /// The building or agent the loc names (`None` for Limbo).
    pub fn holder(self) -> Option<EntityId> {
        match self {
            AssetLoc::Parked(e)
            | AssetLoc::InUse(e)
            | AssetLoc::Carried(e)
            | AssetLoc::Installed(e)
            | AssetLoc::Posted(e)
            | AssetLoc::Stock(e) => Some(e),
            AssetLoc::Limbo(_) => None,
        }
    }
}

/// A finance plan (plan D10): `lender: None` is the city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finance {
    pub lender: Option<EntityId>,
    pub remaining: i64,
    pub per_day: i64,
    pub arrears: u8,
}

/// An owned thing (plan D1): an entity carrying only this component.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub kind: AssetKind,
    /// 1..=3; also the lock tier (vehicles) and the sensor tier (robots).
    pub tier: u8,
    /// M11 owner; `None` = the city. Write only through `assets::set_owner`.
    pub owner: Option<EntityId>,
    /// Write only through `assets::set_loc`.
    pub loc: AssetLoc,
    /// 0..=100; 0 = wrecked (vehicles, robots) or failed (chrome).
    pub condition: u8,
    /// `list × condition / 100`, recomputed daily.
    pub value: i64,
    pub upkeep_per_day: i64,
    pub upkeep_arrears: u8,
    pub finance: Option<Finance>,
    /// Chrome locked by its lender: modifiers off, load stays.
    pub bricked: bool,
    /// Set by theft or a rip; cleared by a chop, a fence or recovery.
    pub stolen: bool,
    pub bought: Tick,
    /// Plan D24: who drives a fleet vehicle.
    #[serde(default)]
    pub keeper: Option<EntityId>,
    /// Plan D1: the list price at purchase.
    #[serde(default)]
    pub list: i64,
    /// Phase 5: midnights a kept corp car has stood parked away from its
    /// owner's buildings (recalled at 2, `vehicles::fleet_recall`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub away_days: u8,
    /// M14 (plan V23): the corp whose tech tier caps this asset's effect;
    /// `None` for pre-M14 assets and god grants (uncapped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maker: Option<EntityId>,
    /// M14 V36: Data a deck carries, per track (a freelancer's take,
    /// sold by `SellData`; confiscated at an arrest).
    #[serde(default, skip_serializing_if = "is_zero_data")]
    pub data: [u32; 3],
    /// M14 V33 (phase 3 writes it): a robot turned by a run fights for
    /// this faction until the tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turned: Option<(EntityId, Tick)>,
    /// Jobs & lives phase 3 (Dylan's answer 1): an item's provenance
    /// (`None` for every pre-milestone asset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prov: Option<Provenance>,
    /// The combat model C1 (docs/COMBAT.md § 4): a weapon's kind, its
    /// `[[weapon_kinds]]` row + 1 (0: not mapped; `combat::map_kind` maps
    /// it by tier and owner class at the spawn).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub wkind: u8,
}

pub(super) fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

pub(super) fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

pub(super) fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

fn is_zero_data(v: &[u32; 3]) -> bool {
    *v == [0; 3]
}
