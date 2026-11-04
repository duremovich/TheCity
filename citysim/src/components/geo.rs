use super::*;

// ---------------------------------------------------------------------------
// Core value types
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct TilePos {
    pub x: u16,
    pub y: u16,
}

impl TilePos {
    pub const fn new(x: u16, y: u16) -> Self {
        TilePos { x, y }
    }

    pub fn manhattan(self, other: TilePos) -> u32 {
        (i32::from(self.x) - i32::from(other.x)).unsigned_abs()
            + (i32::from(self.y) - i32::from(other.y)).unsigned_abs()
    }
}

impl fmt::Display for TilePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.x, self.y)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    /// Exclusive right and bottom edges, in `u32` so a rect reaching column 65,535 cannot overflow.
    fn x1(&self) -> u32 {
        u32::from(self.x) + u32::from(self.w)
    }

    fn y1(&self) -> u32 {
        u32::from(self.y) + u32::from(self.h)
    }

    pub fn contains(&self, p: TilePos) -> bool {
        p.x >= self.x && u32::from(p.x) < self.x1() && p.y >= self.y && u32::from(p.y) < self.y1()
    }

    pub fn on_perimeter(&self, p: TilePos) -> bool {
        self.contains(p)
            && (p.x == self.x || u32::from(p.x) + 1 == self.x1() || p.y == self.y || u32::from(p.y) + 1 == self.y1())
    }

    pub fn centre(&self) -> (f32, f32) {
        (f32::from(self.x) + f32::from(self.w) / 2.0, f32::from(self.y) + f32::from(self.h) / 2.0)
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        u32::from(self.x) < o.x1()
            && u32::from(o.x) < self.x1()
            && u32::from(self.y) < o.y1()
            && u32::from(o.y) < self.y1()
    }
}

/// A coarse district of the map (M10): the second grid of a v2 map file.
/// A v1 map has no zone grid and reads `Mid` everywhere.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Zone {
    Spire,
    Civic,
    Vats,
    #[default]
    Mid,
    Sump,
}

impl Zone {
    pub const ALL: [Zone; 5] = [Zone::Spire, Zone::Civic, Zone::Vats, Zone::Mid, Zone::Sump];

    /// Position in `Zone::ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The map-file letter.
    pub fn letter(self) -> char {
        match self {
            Zone::Spire => 'S',
            Zone::Civic => 'C',
            Zone::Vats => 'V',
            Zone::Mid => 'M',
            Zone::Sump => 'U',
        }
    }

    pub fn parse(c: char) -> Option<Zone> {
        Zone::ALL.into_iter().find(|z| z.letter() == c)
    }
}

impl fmt::Display for Zone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum TileKind {
    Ground,
    Wall,
    Road,
    Door,
    Farmland,
    Water,
    /// Scale S3c (docs/IMPORT_GRAPHICS.md): land outside the urban limit
    /// (an import's hills): walkable, slower to cross, never built on.
    Mountain,
}

impl TileKind {
    pub fn walkable(self) -> bool {
        !matches!(self, TileKind::Wall | TileKind::Water)
    }

    /// Step cost for pathfinding.
    pub fn move_cost(self) -> f32 {
        match self {
            TileKind::Ground | TileKind::Door => 1.0,
            TileKind::Road => 0.7,
            TileKind::Farmland => 1.2,
            TileKind::Mountain => 1.5,
            TileKind::Wall | TileKind::Water => f32::INFINITY,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    Home,
    Farm,
    Market,
    Bar,
    Jail,
    Cemetery,
    Hall,
    Hideout,
    Warehouse,
    /// M10: two in the Vats, loaded and drawn, inert until M11.
    SecurityOffice,
    /// M10: an empty walled-off plot (no walls, one door), inert until M11.
    Lot,
    /// M12 D20: beds by the night for the homeless who can pay.
    Hotel,
    /// M13 D16: the Ripperdoc: chrome, Therapy, Detox (inert until phase 3).
    Clinic,
    /// M13 D16: vehicles, repairs, secure parking (inert until phase 2).
    Garage,
    /// M14 D16 (plan V16): makes Data in its focus track.
    Lab,
    /// M15 W36: a news outlet; its Reporters turn the day's deed records
    /// into story records posted into the pools it covers.
    Feed,
    /// L2 (plan L1): a night club (Hosts; `Enjoy`, `Drink`).
    Club,
    /// L2 (plan L1): a braindance parlour (Attendants; `Enjoy`).
    Arcade,
    /// L2 (plan L1): the street food stall (Cooks; `EatOut`).
    NoodleBar,
    /// L2 (plan L1): the fight pit (Fighters; `Enjoy`, bouts).
    FightPit,
    /// L2 (plan L1): the gambling den (Croupiers; `Gamble`, `Drink`).
    Den,
    /// L2 (plan L1): the Spire lounge (Concierges; `Enjoy`).
    Lounge,
    /// L2 (plan L1): the parts fab (Fabbers; `FabWork` makes Parts).
    Fab,
    /// M16a (plan C8): a Fixer's office: its owner keeps a contract record
    /// book (`Broker`); a game abstraction, never a Den.
    Fixer,
    /// Real economy E26 (docs/ECONOMY_V2.md § 4): a Mission, a kitchen on
    /// donations ("Soup Kitchen"); `Building.charity` holds its purse.
    Mission,
    /// Real economy E37 (addendum 19): a work camp; `Building.camp` holds
    /// the children housed there. Owned by a corp or the city, never an agent.
    Camp,
    /// M16b phase 2 (spec § 4): a corp's private prison, where its
    /// captives are held; built by a corp, never by an agent.
    Prison,
    // --- Jobs & lives phase 1 (docs/JOBS_AND_LIVES.md § 2), only with
    // `[jobs2] enabled`: none stands with the switch off.
    /// Packers turn Farm Food into Rations sold to the World.
    PackingPlant,
    /// Loaders and Drivers: the handling fee on every crossing.
    FreightYard,
    /// Plant Operators: the city's power, paid by every building.
    UtilityPlant,
    /// Builders: every build, refit and floor is paid here.
    ConstructionYard,
    /// A corp's head office in the Spire (Office Workers).
    Hq,
    // --- Jobs & lives phase 3 (§ 4), only with `[weapons] enabled`.
    /// Gunsmiths turn Parts into Weapons (a Security corp's shop).
    Gunsmith,
    // --- Jobs & lives phase 4 (§ 5.1, § 9): venues, only with `[vice]` or
    // `[fashion]` enabled. Display names live in `[buildings.*]`'s rows.
    /// A Vice venue with dancers (a `Venue`; Mid/Sump).
    StripClub,
    /// A Vice venue with its workers and a madam (a `Venue`; Sump/Mid).
    Brothel,
    /// The tailor's and barber's venue (a `Venue`; Mid/Spire): clothing and
    /// grooming, the `status` need's satisfier.
    Boutique,
    // --- Jobs & lives phase 5 (§ 5.2, § 5.3), only with `[vice] chip` or
    // `[vice] bd` on.
    /// A Vice venue on the high rung, staffed under chip contracts (a `Venue`; Mid/Spire).
    Dollhouse,
    /// A studio whose staff make Braindance units (a workplace; Mid).
    BdStudio,
    // --- Jobs & lives phase 6 (§ 7), only with `[gray] enabled` on.
    /// A gang's unlicensed clinic: its stock holds the organs, its staff
    /// are struck-off docs and brokers (a workplace).
    BlackClinic,
    // --- Jobs & lives phase 7 (§ 6.1), only with `[ages] enabled` on.
    /// A School on fees (its teachers; Mid and up).
    School,
    // --- Lives at work phase 7 (§ 5.2), only with `[law] stations` on.
    /// A district's police station: a booking desk and a few cells, the
    /// city's; its guards muster there and the night van takes the held
    /// to the Precinct (the Jail).
    Station,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 41] = [
        BuildingKind::Home,
        BuildingKind::Farm,
        BuildingKind::Market,
        BuildingKind::Bar,
        BuildingKind::Jail,
        BuildingKind::Cemetery,
        BuildingKind::Hall,
        BuildingKind::Hideout,
        BuildingKind::Warehouse,
        BuildingKind::SecurityOffice,
        BuildingKind::Lot,
        BuildingKind::Hotel,
        BuildingKind::Clinic,
        BuildingKind::Garage,
        BuildingKind::Lab,
        BuildingKind::Feed,
        BuildingKind::Club,
        BuildingKind::Arcade,
        BuildingKind::NoodleBar,
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::Lounge,
        BuildingKind::Fab,
        BuildingKind::Fixer,
        BuildingKind::Mission,
        BuildingKind::Camp,
        BuildingKind::Prison,
        BuildingKind::PackingPlant,
        BuildingKind::FreightYard,
        BuildingKind::UtilityPlant,
        BuildingKind::ConstructionYard,
        BuildingKind::Hq,
        BuildingKind::Gunsmith,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Boutique,
        BuildingKind::Dollhouse,
        BuildingKind::BdStudio,
        BuildingKind::BlackClinic,
        BuildingKind::School,
        BuildingKind::Station,
    ];

    /// Jobs & lives phase 1: the kinds that stand only with `[jobs2] enabled`
    /// (phase 3's Gunsmith only with `[weapons]` on too, phase 4's venues
    /// only with `[vice]` or `[fashion]`).
    pub const JOBS2: [BuildingKind; 13] = [
        BuildingKind::PackingPlant,
        BuildingKind::FreightYard,
        BuildingKind::UtilityPlant,
        BuildingKind::ConstructionYard,
        BuildingKind::Hq,
        BuildingKind::Gunsmith,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Boutique,
        BuildingKind::Dollhouse,
        BuildingKind::BdStudio,
        BuildingKind::BlackClinic,
        BuildingKind::School,
    ];

    /// Jobs & lives phase 4: every venue kind, L2's six and phase 4's three
    /// (a loop over it meets no building of the new kinds with their
    /// switches off). The CSV's per-kind columns keep `LEISURE`. Phase 5
    /// appends the Dollhouse.
    pub const LEISURE_ALL: [BuildingKind; 10] = [
        BuildingKind::Club,
        BuildingKind::Arcade,
        BuildingKind::NoodleBar,
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::Lounge,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Boutique,
        BuildingKind::Dollhouse,
    ];

    /// L2 (plan L1): the six leisure kinds (a `Venue` each; the Fab has none).
    pub const LEISURE: [BuildingKind; 6] = [
        BuildingKind::Club,
        BuildingKind::Arcade,
        BuildingKind::NoodleBar,
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::Lounge,
    ];

    /// L2: one of the six leisure kinds (Jobs & lives phase 4: or one of
    /// its three venues).
    pub fn is_leisure(self) -> bool {
        BuildingKind::LEISURE_ALL.contains(&self)
    }

    pub fn parse(s: &str) -> Option<BuildingKind> {
        Some(match s {
            "Home" => BuildingKind::Home,
            "Farm" => BuildingKind::Farm,
            "Market" => BuildingKind::Market,
            "Bar" => BuildingKind::Bar,
            "Jail" => BuildingKind::Jail,
            "Cemetery" => BuildingKind::Cemetery,
            "Hall" => BuildingKind::Hall,
            "Hideout" => BuildingKind::Hideout,
            "Warehouse" => BuildingKind::Warehouse,
            "SecurityOffice" => BuildingKind::SecurityOffice,
            "Lot" => BuildingKind::Lot,
            "Hotel" => BuildingKind::Hotel,
            "Clinic" => BuildingKind::Clinic,
            "Garage" => BuildingKind::Garage,
            "Lab" => BuildingKind::Lab,
            "Feed" => BuildingKind::Feed,
            "Club" => BuildingKind::Club,
            "Arcade" => BuildingKind::Arcade,
            "NoodleBar" => BuildingKind::NoodleBar,
            "FightPit" => BuildingKind::FightPit,
            "Den" => BuildingKind::Den,
            "Lounge" => BuildingKind::Lounge,
            "Fab" => BuildingKind::Fab,
            "Fixer" => BuildingKind::Fixer,
            "Mission" => BuildingKind::Mission,
            "Camp" => BuildingKind::Camp,
            "Prison" => BuildingKind::Prison,
            "PackingPlant" => BuildingKind::PackingPlant,
            "FreightYard" => BuildingKind::FreightYard,
            "UtilityPlant" => BuildingKind::UtilityPlant,
            "ConstructionYard" => BuildingKind::ConstructionYard,
            "Hq" => BuildingKind::Hq,
            "Gunsmith" => BuildingKind::Gunsmith,
            "StripClub" => BuildingKind::StripClub,
            "Brothel" => BuildingKind::Brothel,
            "Boutique" => BuildingKind::Boutique,
            "Dollhouse" => BuildingKind::Dollhouse,
            "BdStudio" => BuildingKind::BdStudio,
            "BlackClinic" => BuildingKind::BlackClinic,
            "School" => BuildingKind::School,
            "Station" => BuildingKind::Station,
            _ => return None,
        })
    }

    /// The display name (M11 section 1). Identifiers and `Display` stay the v1 names.
    pub fn label(self) -> &'static str {
        match self {
            BuildingKind::Home => "Block",
            BuildingKind::Farm => "Vat Farm",
            BuildingKind::Market => "Street Market",
            BuildingKind::Bar => "Bar",
            BuildingKind::Jail => "Precinct",
            BuildingKind::Cemetery => "Recycler",
            BuildingKind::Hall => "Civic Hall",
            BuildingKind::Hideout => "Hideout",
            BuildingKind::Warehouse => "Reserve Depot",
            BuildingKind::SecurityOffice => "Security Office",
            BuildingKind::Lot => "Lot",
            BuildingKind::Hotel => "Capsule Hotel",
            BuildingKind::Clinic => "Ripperdoc",
            BuildingKind::Garage => "Garage",
            BuildingKind::Lab => "Lab",
            BuildingKind::Feed => "Feed",
            BuildingKind::Club => "Club",
            BuildingKind::Arcade => "Braindance Parlour",
            BuildingKind::NoodleBar => "Noodle Bar",
            BuildingKind::FightPit => "Fight Pit",
            BuildingKind::Den => "Gambling Den",
            BuildingKind::Lounge => "Spire Lounge",
            BuildingKind::Fab => "Parts Fab",
            BuildingKind::Fixer => "Fixer",
            BuildingKind::Mission => "Soup Kitchen",
            BuildingKind::Camp => "Work Camp",
            BuildingKind::Prison => "Private Prison",
            BuildingKind::PackingPlant => "Packing Plant",
            BuildingKind::FreightYard => "Freight Yard",
            BuildingKind::UtilityPlant => "Utility Plant",
            BuildingKind::ConstructionYard => "Construction Yard",
            BuildingKind::Hq => "Corp HQ",
            BuildingKind::Gunsmith => "Gunsmith",
            BuildingKind::StripClub => "Strip Club",
            BuildingKind::Brothel => "Brothel",
            BuildingKind::Boutique => "Boutique",
            BuildingKind::Dollhouse => "Dollhouse",
            BuildingKind::BdStudio => "BD Studio",
            BuildingKind::BlackClinic => "Back Room",
            BuildingKind::School => "School",
            BuildingKind::Station => "Station",
        }
    }

    /// The letter drawn at the building's centre.
    pub fn letter(self) -> char {
        match self {
            BuildingKind::Home => 'H',
            BuildingKind::Farm => 'F',
            BuildingKind::Market => 'M',
            BuildingKind::Bar => 'B',
            BuildingKind::Jail => 'J',
            BuildingKind::Cemetery => 'C',
            BuildingKind::Hall => 'T',
            BuildingKind::Hideout => 'G',
            BuildingKind::Warehouse => 'W',
            BuildingKind::SecurityOffice => 'O',
            BuildingKind::Lot => 'L',
            BuildingKind::Hotel => 'N',
            BuildingKind::Clinic => 'R',
            BuildingKind::Garage => 'V',
            BuildingKind::Lab => 'Q',
            BuildingKind::Feed => 'P',
            // L2 (plan L1): `X` and `Z` are claimed by M16 (Fixer, Prison).
            BuildingKind::Club => 'K',
            BuildingKind::Arcade => 'A',
            BuildingKind::NoodleBar => 'S',
            BuildingKind::FightPit => 'I',
            BuildingKind::Den => 'D',
            BuildingKind::Lounge => 'U',
            BuildingKind::Fab => 'E',
            // M16a (plan C8): the Fixer's office.
            BuildingKind::Fixer => 'X',
            // Real economy E26, E37: `M` is the Market's and `Y` the one
            // free capital, so the Mission draws `+` (render only).
            BuildingKind::Mission => '+',
            BuildingKind::Camp => 'Y',
            // M16b phase 2: the letter L2 left it.
            BuildingKind::Prison => 'Z',
            // Jobs & lives phase 1 (JOBS_V2 J33): every capital is taken.
            BuildingKind::PackingPlant => '&',
            BuildingKind::FreightYard => '%',
            BuildingKind::UtilityPlant => '*',
            BuildingKind::ConstructionYard => '^',
            BuildingKind::Hq => '$',
            // Jobs & lives phase 3.
            BuildingKind::Gunsmith => '!',
            // Jobs & lives phase 4.
            BuildingKind::StripClub => '<',
            BuildingKind::Brothel => '>',
            BuildingKind::Boutique => '=',
            // Jobs & lives phase 5.
            BuildingKind::Dollhouse => '~',
            BuildingKind::BdStudio => '#',
            // Jobs & lives phase 6.
            BuildingKind::BlackClinic => '@',
            // Jobs & lives phase 7.
            BuildingKind::School => '?',
            // Lives at work phase 7.
            BuildingKind::Station => '|',
        }
    }
}

impl fmt::Display for BuildingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
