use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingCfg {
    pub capacity: u16,
    pub stock_cap: u32,
    /// M11 D17: full staff for an owned niche building (Grow tops up to it;
    /// an agent-owned Bar posts a vacancy below it). 0 = no rule.
    #[serde(default)]
    pub staff: u32,
    // --- Jobs & lives phase 1 (D4): the `KindCfg` columns. Each is
    // optional: an absent key keeps the kind's built-in value (the M11-M17
    // constants), so an older config reads as before.
    /// What founding or building the kind costs (the new kinds; the older
    /// kinds keep `[corps] found_cost`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_cost: Option<i64>,
    /// What a building of the kind sells for (the new kinds; the older
    /// kinds keep `[corps] value`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    /// Floors at seed (the new kinds; a map building's own count otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors_seed: Option<u8>,
    /// The most floors the kind may have, ahead of `[floors] floors_max`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors_max: Option<u8>,
    /// The corp niche the kind sits in (`corp_brain::niche_of_kind`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub niche: Option<crate::components::Niche>,
    /// An agent may found it: the tie-break rank (1 first), 0 never
    /// (`founding::foundable`, which was the `FOUNDABLE` list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foundable: Option<u8>,
}

/// Jobs & lives phase 1 (D4): a kind's row of the `[buildings.<kind>]`
/// table, read through [`BuildingsCfg::for_kind`] and the `KindCfg`
/// readers ([`Config::kind_niche`], [`Config::kind_foundable`]).
pub type KindCfg = BuildingCfg;

impl BuildingCfg {
    /// The `KindCfg` columns absent (the struct-update base of the
    /// constructors below).
    pub const BASE: BuildingCfg = BuildingCfg {
        capacity: 0,
        stock_cap: 0,
        staff: 0,
        found_cost: None,
        value: None,
        floors_seed: None,
        floors_max: None,
        niche: None,
        foundable: None,
    };

    /// Jobs & lives phase 1 (§ 2): the industries and the Hq.
    pub fn packing_plant() -> BuildingCfg {
        BuildingCfg {
            capacity: 20,
            stock_cap: 300,
            staff: 0,
            found_cost: Some(900),
            value: Some(900),
            floors_seed: Some(2),
            floors_max: Some(4),
            ..BuildingCfg::BASE
        }
    }
    pub fn freight_yard() -> BuildingCfg {
        BuildingCfg {
            capacity: 24,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(800),
            value: Some(800),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    pub fn utility_plant() -> BuildingCfg {
        BuildingCfg {
            capacity: 14,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(1000),
            value: Some(1000),
            floors_seed: Some(2),
            floors_max: Some(2),
            ..BuildingCfg::BASE
        }
    }
    pub fn construction_yard() -> BuildingCfg {
        BuildingCfg {
            capacity: 14,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(700),
            value: Some(700),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 3 (§ 4): a Gunsmith (Parts in stock).
    /// Jobs & lives phase 4 (§ 5.1): a StripClub (dancers, bouncers, a barkeep).
    pub fn strip_club() -> BuildingCfg {
        BuildingCfg {
            capacity: 24,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(700),
            value: Some(700),
            floors_seed: Some(1),
            floors_max: Some(2),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 4 (§ 5.1): a Brothel (its workers, a barkeep, a madam).
    pub fn brothel() -> BuildingCfg {
        BuildingCfg {
            capacity: 16,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(600),
            value: Some(600),
            floors_seed: Some(1),
            floors_max: Some(2),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 4 (§ 9): a Boutique (tailors, barbers).
    pub fn boutique() -> BuildingCfg {
        BuildingCfg {
            capacity: 10,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(500),
            value: Some(500),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 5 (§ 5.2): a Dollhouse (its contract hosts).
    pub fn dollhouse() -> BuildingCfg {
        BuildingCfg {
            capacity: 12,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(900),
            value: Some(900),
            floors_seed: Some(1),
            floors_max: Some(2),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 7 (§ 6.1): a School (its teachers).
    /// Lives at work phase 7: the Station's row with `[buildings.station]`
    /// absent (stations off: none stands).
    pub const STATION_OFF: BuildingCfg = BuildingCfg { capacity: 0, stock_cap: 0, staff: 0, ..BuildingCfg::BASE };

    pub fn school() -> BuildingCfg {
        BuildingCfg {
            capacity: 40,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(600),
            value: Some(600),
            floors_seed: Some(1),
            floors_max: Some(2),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 6 (§ 7): a gang's BlackClinic (organs in stock).
    pub fn black_clinic() -> BuildingCfg {
        BuildingCfg {
            capacity: 8,
            stock_cap: 40,
            staff: 0,
            found_cost: Some(500),
            value: Some(500),
            floors_seed: Some(1),
            floors_max: Some(1),
            ..BuildingCfg::BASE
        }
    }
    /// Jobs & lives phase 5 (§ 5.3): a BdStudio (its staff make units).
    pub fn bd_studio() -> BuildingCfg {
        BuildingCfg {
            capacity: 10,
            stock_cap: 60,
            staff: 0,
            found_cost: Some(700),
            value: Some(700),
            floors_seed: Some(1),
            floors_max: Some(1),
            niche: Some(crate::components::Niche::Vice),
            ..BuildingCfg::BASE
        }
    }
    pub fn gunsmith() -> BuildingCfg {
        BuildingCfg {
            capacity: 8,
            stock_cap: 60,
            staff: 0,
            found_cost: Some(600),
            value: Some(600),
            floors_seed: Some(1),
            floors_max: Some(1),
            niche: Some(crate::components::Niche::Security),
            ..BuildingCfg::BASE
        }
    }
    pub fn hq() -> BuildingCfg {
        BuildingCfg {
            capacity: 30,
            stock_cap: 0,
            staff: 0,
            found_cost: Some(1500),
            value: Some(1500),
            floors_seed: Some(3),
            floors_max: Some(8),
            ..BuildingCfg::BASE
        }
    }

    /// No room, no stock: the M10 kinds that do nothing until M11.
    pub fn inert() -> BuildingCfg {
        BuildingCfg { capacity: 0, stock_cap: 0, staff: 0, ..BuildingCfg::BASE }
    }

    /// M12 D20: a Hotel holds its guests (capacity is capped by `[street]
    /// hotel_beds` and the interior when built).
    pub fn hotel() -> BuildingCfg {
        BuildingCfg { capacity: 12, stock_cap: 0, staff: 0, ..BuildingCfg::BASE }
    }

    /// M14 (plan V16): a Lab of four Researchers.
    pub fn lab() -> BuildingCfg {
        BuildingCfg { capacity: 8, stock_cap: 0, staff: 4, ..BuildingCfg::BASE }
    }

    /// M15 W36: a Feed of one Reporter (phase 4: `assets/config.toml`).
    pub fn feed() -> BuildingCfg {
        BuildingCfg { capacity: 6, stock_cap: 0, staff: 1, ..BuildingCfg::BASE }
    }

    /// L2 § 1 (spec values; `[buildings.club]` … in `assets/config.toml`).
    pub fn club() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 0, staff: 10, ..BuildingCfg::BASE }
    }
    pub fn arcade() -> BuildingCfg {
        BuildingCfg { capacity: 16, stock_cap: 0, staff: 3, ..BuildingCfg::BASE }
    }
    pub fn noodle_bar() -> BuildingCfg {
        BuildingCfg { capacity: 10, stock_cap: 60, staff: 3, ..BuildingCfg::BASE }
    }
    pub fn fight_pit() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 0, staff: 4, ..BuildingCfg::BASE }
    }
    pub fn den() -> BuildingCfg {
        BuildingCfg { capacity: 20, stock_cap: 0, staff: 4, ..BuildingCfg::BASE }
    }
    pub fn lounge() -> BuildingCfg {
        BuildingCfg { capacity: 16, stock_cap: 0, staff: 6, ..BuildingCfg::BASE }
    }
    pub fn fab() -> BuildingCfg {
        BuildingCfg { capacity: 14, stock_cap: 400, staff: 12, ..BuildingCfg::BASE }
    }

    /// M16a (spec § 2): a Fixer's office of one Fixer.
    pub fn fixer() -> BuildingCfg {
        BuildingCfg { capacity: 8, stock_cap: 0, staff: 1, ..BuildingCfg::BASE }
    }

    /// Real economy E26: a Mission's kitchen (12 cots, 60 meals of stock,
    /// two Volunteers).
    pub fn mission() -> BuildingCfg {
        BuildingCfg { capacity: 12, stock_cap: 60, staff: 2, ..BuildingCfg::BASE }
    }

    /// Real economy E37: a work camp (no staff role: the children run the
    /// shift and the owner is the employer of record).
    pub fn camp() -> BuildingCfg {
        BuildingCfg { capacity: 40, stock_cap: 120, staff: 0, ..BuildingCfg::BASE }
    }

    /// M16b phase 2 (spec § 4): a private prison (plan deviation: no staff
    /// role; its guards are its corp's posted ones).
    pub fn prison() -> BuildingCfg {
        BuildingCfg { capacity: 20, stock_cap: 100, staff: 0, ..BuildingCfg::BASE }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingsCfg {
    pub home: BuildingCfg,
    pub farm: BuildingCfg,
    pub market: BuildingCfg,
    pub bar: BuildingCfg,
    pub jail: BuildingCfg,
    pub cemetery: BuildingCfg,
    pub hall: BuildingCfg,
    pub hideout: BuildingCfg,
    pub warehouse: BuildingCfg,
    /// M10, inert until M11.
    #[serde(default = "BuildingCfg::inert")]
    pub security_office: BuildingCfg,
    /// M10, inert until M11.
    #[serde(default = "BuildingCfg::inert")]
    pub lot: BuildingCfg,
    /// M12 D20: a Capsule Hotel (beds are `[street] hotel_beds`).
    #[serde(default = "BuildingCfg::hotel")]
    pub hotel: BuildingCfg,
    /// M13 D16: the Ripperdoc.
    #[serde(default = "BuildingCfg::inert")]
    pub clinic: BuildingCfg,
    /// M13 D16.
    #[serde(default = "BuildingCfg::inert")]
    pub garage: BuildingCfg,
    /// M14 (plan V16).
    #[serde(default = "BuildingCfg::lab")]
    pub lab: BuildingCfg,
    /// M15 W36.
    #[serde(default = "BuildingCfg::feed")]
    pub feed: BuildingCfg,
    /// L2 § 1: the seven new kinds (none stands with L2 off).
    #[serde(default = "BuildingCfg::club")]
    pub club: BuildingCfg,
    #[serde(default = "BuildingCfg::arcade")]
    pub arcade: BuildingCfg,
    #[serde(default = "BuildingCfg::noodle_bar")]
    pub noodle_bar: BuildingCfg,
    #[serde(default = "BuildingCfg::fight_pit")]
    pub fight_pit: BuildingCfg,
    #[serde(default = "BuildingCfg::den")]
    pub den: BuildingCfg,
    #[serde(default = "BuildingCfg::lounge")]
    pub lounge: BuildingCfg,
    #[serde(default = "BuildingCfg::fab")]
    pub fab: BuildingCfg,
    /// M16a (plan C8): a Fixer's office (none stands with contracts off).
    #[serde(default = "BuildingCfg::fixer")]
    pub fixer: BuildingCfg,
    /// Real economy E26 (none stands with `[charity]` off).
    #[serde(default = "BuildingCfg::mission")]
    pub mission: BuildingCfg,
    /// Real economy E37 (none stands with `[camp]` off).
    #[serde(default = "BuildingCfg::camp")]
    pub camp: BuildingCfg,
    /// M16b phase 2: a corp's private prison.
    #[serde(default = "BuildingCfg::prison")]
    pub prison: BuildingCfg,
    /// Jobs & lives phase 1 (§ 2): none stands with `[jobs2]` off.
    #[serde(default = "BuildingCfg::packing_plant")]
    pub packing_plant: BuildingCfg,
    #[serde(default = "BuildingCfg::freight_yard")]
    pub freight_yard: BuildingCfg,
    #[serde(default = "BuildingCfg::utility_plant")]
    pub utility_plant: BuildingCfg,
    #[serde(default = "BuildingCfg::construction_yard")]
    pub construction_yard: BuildingCfg,
    #[serde(default = "BuildingCfg::hq")]
    pub hq: BuildingCfg,
    /// Jobs & lives phase 3 (§ 4): none stands with `[weapons]` off.
    #[serde(default = "BuildingCfg::gunsmith")]
    pub gunsmith: BuildingCfg,
    /// Jobs & lives phase 4: none stands with `[vice]` / `[fashion]` off.
    #[serde(default = "BuildingCfg::strip_club")]
    pub strip_club: BuildingCfg,
    #[serde(default = "BuildingCfg::brothel")]
    pub brothel: BuildingCfg,
    #[serde(default = "BuildingCfg::boutique")]
    pub boutique: BuildingCfg,
    /// Jobs & lives phase 5: none stands with `[vice] chip` / `bd` off.
    #[serde(default = "BuildingCfg::dollhouse")]
    pub dollhouse: BuildingCfg,
    #[serde(default = "BuildingCfg::bd_studio")]
    pub bd_studio: BuildingCfg,
    /// Jobs & lives phase 6: none stands with `[gray]` off.
    #[serde(default = "BuildingCfg::black_clinic")]
    pub black_clinic: BuildingCfg,
    /// Jobs & lives phase 7: none stands with `[ages]` off.
    #[serde(default = "BuildingCfg::school")]
    pub school: BuildingCfg,
    /// Lives at work phase 7 (§ 5.2): the Station's seats (its guards and
    /// its held; the cells are `[law] station_cells`). Absent (and not
    /// written) with `[law] stations` off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<BuildingCfg>,
}

impl BuildingsCfg {
    pub fn for_kind(&self, kind: BuildingKind) -> &BuildingCfg {
        match kind {
            BuildingKind::Home => &self.home,
            BuildingKind::Farm => &self.farm,
            BuildingKind::Market => &self.market,
            BuildingKind::Bar => &self.bar,
            BuildingKind::Jail => &self.jail,
            BuildingKind::Cemetery => &self.cemetery,
            BuildingKind::Hall => &self.hall,
            BuildingKind::Hideout => &self.hideout,
            BuildingKind::Warehouse => &self.warehouse,
            BuildingKind::SecurityOffice => &self.security_office,
            BuildingKind::Lot => &self.lot,
            BuildingKind::Hotel => &self.hotel,
            BuildingKind::Clinic => &self.clinic,
            BuildingKind::Garage => &self.garage,
            BuildingKind::Lab => &self.lab,
            BuildingKind::Feed => &self.feed,
            BuildingKind::Club => &self.club,
            BuildingKind::Arcade => &self.arcade,
            BuildingKind::NoodleBar => &self.noodle_bar,
            BuildingKind::FightPit => &self.fight_pit,
            BuildingKind::Den => &self.den,
            BuildingKind::Lounge => &self.lounge,
            BuildingKind::Fab => &self.fab,
            BuildingKind::Fixer => &self.fixer,
            BuildingKind::Mission => &self.mission,
            BuildingKind::Camp => &self.camp,
            BuildingKind::Prison => &self.prison,
            BuildingKind::PackingPlant => &self.packing_plant,
            BuildingKind::FreightYard => &self.freight_yard,
            BuildingKind::UtilityPlant => &self.utility_plant,
            BuildingKind::ConstructionYard => &self.construction_yard,
            BuildingKind::Hq => &self.hq,
            BuildingKind::Gunsmith => &self.gunsmith,
            BuildingKind::StripClub => &self.strip_club,
            BuildingKind::Brothel => &self.brothel,
            BuildingKind::Boutique => &self.boutique,
            BuildingKind::Dollhouse => &self.dollhouse,
            BuildingKind::BdStudio => &self.bd_studio,
            BuildingKind::BlackClinic => &self.black_clinic,
            BuildingKind::School => &self.school,
            BuildingKind::Station => self.station.as_ref().unwrap_or(&BuildingCfg::STATION_OFF),
        }
    }
}

/// Jobs and room § 2.3 (plan J7, J8) `[floors]`: what a floor costs, when a
/// corp adds one, and how high each kind may go.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorsCfg {
    /// J8: a floor costs this × the kind's `found_cost`.
    pub floor_cost_frac: f32,
    /// J8: midnights at the ceiling with room for a floor's staff before a corp adds one.
    pub floor_days: u8,
    /// J8: a corp adds a floor only with no vacant Lot within this many
    /// tiles (Manhattan, door to door) of the building.
    pub lot_reach: u32,
    /// J8: the highest a kind may go, keyed by the map's kind names
    /// (`BuildingKind::parse`); a kind not listed stays at one floor.
    pub floors_max: std::collections::BTreeMap<String, u8>,
}

impl Default for FloorsCfg {
    fn default() -> Self {
        let max = [
            ("Farm", 3),
            ("Fab", 3),
            ("Home", 6),
            ("Club", 2),
            ("Arcade", 2),
            ("NoodleBar", 2),
            ("FightPit", 2),
            ("Den", 2),
            ("Lounge", 2),
        ];
        FloorsCfg {
            floor_cost_frac: 0.6,
            floor_days: 7,
            lot_reach: 48,
            floors_max: max.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }
}

impl FloorsCfg {
    /// J8: the most floors a `kind` may have (1 when not listed). Jobs &
    /// lives phase 1: a `[buildings.<kind>] floors_max` comes first
    /// ([`Config::floors_max`]).
    pub fn max_for(&self, kind: BuildingKind) -> u8 {
        self.floors_max.iter().find(|(k, _)| BuildingKind::parse(k) == Some(kind)).map_or(1, |(_, &v)| v.max(1))
    }
}
