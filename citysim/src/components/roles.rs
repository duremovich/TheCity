use super::*;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Sex {
    Male,
    Female,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Lod {
    Full,
    Coarse,
    Statistical,
}

/// Jobs and room J9: a trade's interned key and label (one leaked pair per
/// distinct `(id, label)` row in the process, so a `Role` stays `Copy` and
/// labels itself without the world).
#[derive(Debug, PartialEq, Eq)]
pub struct TradeName {
    pub key: String,
    pub label: String,
}

/// The interned names, by `(key, label)`.
static TRADE_NAMES: std::sync::Mutex<Vec<&'static TradeName>> = std::sync::Mutex::new(Vec::new());

thread_local! {
    /// The trades a save being read resolves `Trade("key")` against: set by
    /// `Config`'s `trades` field as it is deserialised (the config precedes
    /// every Job in a `World`), cleared by `save::from_ron` first.
    static TRADE_KEYS: std::cell::RefCell<Vec<TradeId>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Jobs and room J9: a configured trade, the index of its `[[trades]]` row
/// in `Config::trades`. It carries the row's interned key and label: a save
/// writes the key (`Trade("night_porter")`) and a load looks the key up in
/// the config the save carries (an unknown key fails the load); labels and
/// events read the name with no world at hand. Equality and order are the
/// index's (one world's rows are one list).
#[derive(Copy, Clone)]
pub struct TradeId {
    idx: u8,
    name: &'static TradeName,
}

impl TradeId {
    /// The id of row `idx` keyed `key` and labelled `label` (interned).
    pub fn new(idx: u8, key: &str, label: &str) -> TradeId {
        let mut names = TRADE_NAMES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let name = match names.iter().find(|n| n.key == key && n.label == label) {
            Some(&n) => n,
            None => {
                let n: &'static TradeName =
                    Box::leak(Box::new(TradeName { key: key.to_string(), label: label.to_string() }));
                names.push(n);
                n
            }
        };
        TradeId { idx, name }
    }

    /// The row index in `Config::trades`.
    pub fn index(self) -> usize {
        usize::from(self.idx)
    }

    /// The row's stable key (`id`).
    pub fn key(self) -> &'static str {
        &self.name.key
    }

    /// The row's display label.
    pub fn label(self) -> &'static str {
        &self.name.label
    }

    /// The trades a save's `Trade("key")` resolves against on this thread
    /// (`Config`'s `trades` deserialiser; `save::from_ron` clears it).
    pub fn set_keys(ids: Vec<TradeId>) {
        TRADE_KEYS.with(|k| *k.borrow_mut() = ids);
    }
}

impl PartialEq for TradeId {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl Eq for TradeId {}

impl PartialOrd for TradeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TradeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.idx.cmp(&other.idx)
    }
}

impl fmt::Debug for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.key())
    }
}

impl Serialize for TradeId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for TradeId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<TradeId, D::Error> {
        let key = String::deserialize(d)?;
        TRADE_KEYS.with(|k| k.borrow().iter().copied().find(|t| t.key() == key)).ok_or_else(|| {
            serde::de::Error::custom(format!("unknown trade key {key:?} (not in the config's [[trades]])"))
        })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Role {
    Farmer,
    Guard,
    Clerk,
    Bartender,
    Gravedigger,
    /// M12 D23: the city's street sweepers, employed at the Recycler.
    Sanitation,
    /// M13 D16: staff of a Clinic.
    Ripperdoc,
    /// M13 D16: staff of a Garage.
    Mechanic,
    /// M14 (plan V16): staff of a Lab.
    Researcher,
    /// M15 W36: staff of a Feed.
    Reporter,
    /// L2 (plan L2): staff of a Club ("Club Staff").
    Host,
    /// L2: staff of an Arcade.
    Attendant,
    /// L2: staff of a NoodleBar.
    Cook,
    /// L2: staff of a FightPit.
    Fighter,
    /// L2: staff of a Den.
    Croupier,
    /// L2: staff of a Lounge.
    Concierge,
    /// L2: staff of a Fab ("Fab Tech").
    Fabber,
    /// M16a (plan C8): staff of a Fixer's office.
    Fixer,
    /// Real economy E26: a Mission's unpaid staff (wage 0; a staff meal
    /// from the kitchen; doubles the hourly service).
    Volunteer,
    /// Jobs and room J9: a configured trade (`[[trades]]`), appended.
    Trade(TradeId),
}

impl Role {
    pub const ALL: [Role; 19] = [
        Role::Farmer,
        Role::Guard,
        Role::Clerk,
        Role::Bartender,
        Role::Gravedigger,
        Role::Sanitation,
        Role::Ripperdoc,
        Role::Mechanic,
        Role::Researcher,
        Role::Reporter,
        Role::Host,
        Role::Attendant,
        Role::Cook,
        Role::Fighter,
        Role::Croupier,
        Role::Concierge,
        Role::Fabber,
        Role::Fixer,
        Role::Volunteer,
    ];

    /// The display name (M11 section 1).
    pub fn label(self) -> &'static str {
        match self {
            Role::Farmer => "Vat Tech",
            Role::Guard => "Guard",
            Role::Clerk => "Clerk",
            Role::Bartender => "Bartender",
            Role::Gravedigger => "Recycler Tech",
            Role::Sanitation => "Sanitation",
            Role::Ripperdoc => "Ripperdoc",
            Role::Mechanic => "Mechanic",
            Role::Researcher => "Researcher",
            Role::Reporter => "Reporter",
            Role::Host => "Club Staff",
            Role::Attendant => "Attendant",
            Role::Cook => "Cook",
            Role::Fighter => "Fighter",
            Role::Croupier => "Croupier",
            Role::Concierge => "Concierge",
            Role::Fabber => "Fab Tech",
            Role::Fixer => "Fixer",
            Role::Volunteer => "Volunteer",
            Role::Trade(t) => t.label(),
        }
    }

    /// Jobs and room J9: the trade, for a `Trade` role.
    pub fn trade(self) -> Option<TradeId> {
        match self {
            Role::Trade(t) => Some(t),
            _ => None,
        }
    }

    /// Jobs and room J9: is this a configured trade (not a bespoke role)?
    pub fn is_trade(self) -> bool {
        matches!(self, Role::Trade(_))
    }

    /// The building kind that employs a bespoke role; `None` for a trade
    /// (its row's `workplace` lists the kinds).
    pub fn workplace(self) -> Option<BuildingKind> {
        Some(match self {
            Role::Farmer => BuildingKind::Farm,
            Role::Guard => BuildingKind::Jail,
            Role::Clerk => BuildingKind::Market,
            Role::Bartender => BuildingKind::Bar,
            Role::Gravedigger => BuildingKind::Cemetery,
            Role::Sanitation => BuildingKind::Cemetery,
            Role::Ripperdoc => BuildingKind::Clinic,
            Role::Mechanic => BuildingKind::Garage,
            Role::Researcher => BuildingKind::Lab,
            Role::Reporter => BuildingKind::Feed,
            Role::Host => BuildingKind::Club,
            Role::Attendant => BuildingKind::Arcade,
            Role::Cook => BuildingKind::NoodleBar,
            Role::Fighter => BuildingKind::FightPit,
            Role::Croupier => BuildingKind::Den,
            Role::Concierge => BuildingKind::Lounge,
            Role::Fabber => BuildingKind::Fab,
            Role::Fixer => BuildingKind::Fixer,
            Role::Volunteer => BuildingKind::Mission,
            Role::Trade(_) => return None,
        })
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
