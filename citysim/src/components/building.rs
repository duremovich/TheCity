use super::*;

// ---------------------------------------------------------------------------
// Non-agent entities
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub kind: BuildingKind,
    /// Farm only.
    pub production_accum: f32,
    /// Home only: which gang is working this Home and how far along
    /// (`systems::gang::CLAIM_HELD` holds it).
    #[serde(default)]
    pub claim: Option<Claim>,
    /// Homes: children's food owed to the pantry, `0.5 × children` per day.
    #[serde(default)]
    pub child_food_debt: f32,
    pub rect: Rect,
    pub door: TilePos,
    /// `0..=cap` per kind table.
    pub stock_food: u32,
    pub capacity: u16,
    /// `None` = city-owned.
    pub owner: Option<EntityId>,
    /// Sorted.
    pub occupants: Vec<EntityId>,
    /// Set by `DemolishHome`; drawn dashed, never used.
    pub demolished: bool,
    /// M10: 0 Sump, 1 Mid, 2 Spire (the map's seventh `B` field). Scales the
    /// Sleep safety bonus at home; rent is M11.
    #[serde(default = "default_tier")]
    pub tier: u8,
    /// M11 D6: a Home's daily rent, split among its adult residents; set by
    /// the owner at the rent pass.
    #[serde(default)]
    pub rent_per_day: i64,
    /// M11: coins earned through this building today (gross), and the last
    /// seven days, newest last.
    #[serde(default)]
    pub revenue_today: i64,
    #[serde(default)]
    pub revenue: VecDeque<i64>,
    /// M11 D19: the security corp holding a contract on this building (phase 3).
    #[serde(default)]
    pub secured_by: Option<EntityId>,
    /// M12 D25: abandoned: no owner, no rent, no staff; squatters may move in.
    #[serde(default)]
    pub derelict: bool,
    /// M12 fix pass: the capacity a derelict had before it went derelict (a
    /// demolished Block stands at half), restored when it returns to use.
    #[serde(default)]
    pub full_capacity: Option<u16>,
    /// M12 D25/D26: a Block with no residents since this tick (abandonment),
    /// and, once derelict, the tick it went derelict (re-letting).
    #[serde(default)]
    pub empty_since: Option<Tick>,
    /// M12 D33: looted by a riot; no trade or beds until this tick.
    #[serde(default)]
    pub closed_until: Option<Tick>,
    /// M13 D6: Stims and Parts in stock (`Good as usize - 1`); Food keeps `stock_food`.
    #[serde(default)]
    pub stock_goods: [u32; 2],
    /// M13 D17: assets sold here today, and the last seven days (newest last).
    #[serde(default)]
    pub asset_sales_today: u16,
    #[serde(default)]
    pub asset_sales: VecDeque<u16>,
    /// M14 (plan V25): the building node's ICE.
    #[serde(default, skip_serializing_if = "crate::virt::SecurityProfile::is_bare")]
    pub security: crate::virt::SecurityProfile,
    /// M14 (plan V16): a Lab's track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<crate::virt::Track>,
    /// M14 (plan V32/V33, phase 3 writes it): a hack on its node, until.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hacked: Option<(crate::virt::HackEffect, Tick)>,
    /// M14 V32: the last DoorOpen on it (one per `door_cooldown_days`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_door_open: Option<Tick>,
    /// M15 W31: a display label over the kind's ("Chapel" for The Unplugged's Hideout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// M15 W36: a Feed's state (name, reach, covered districts, buried actors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed: Option<crate::word::FeedState>,
    /// L2 (plan L4): a leisure venue's day (price, visits, take, front).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<crate::living::Venue>,
    /// Real economy E26: a Mission's (or the Chapel's) kitchen and purse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charity: Option<crate::econ::Charity>,
    /// Real economy E37: a work camp's children and ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camp: Option<crate::econ::CampState>,
    /// The Real economy phase 2 (plan E13): the fraction of a coin of
    /// inputs a producer still owes the World (whole coins cross out per
    /// unit produced, `wages::produce_inputs`).
    #[serde(default, skip_serializing_if = "is_zero_f32_c")]
    pub input_accum: f32,
    /// Jobs and room J6: storeys. Floors multiply a workplace's places
    /// (`jobs::places_of`) and a Home's, Hotel's or venue's seats
    /// (`capacity`); they cost no pathfinding (the door is the one portal).
    #[serde(default = "one_u8", skip_serializing_if = "is_one_u8")]
    pub floors: u8,
    /// Jobs and room J8: consecutive midnights this building stood at its
    /// staffing ceiling with its corp's room covering another floor's
    /// staff (`wages::floor_pass`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub floor_days: u8,
    /// Hunger as sickness § 1: the synth meals among a pantry's
    /// `stock_food` (a subset, clamped as `Inventory.synth`).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub synth_food: u32,
    /// Hunger as sickness § 4: meals of scraps in the bin out back.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub bin: u16,
    /// § 4: a venue's fraction of a scrap meal (`venue_scrap` a meal served).
    #[serde(default, skip_serializing_if = "is_zero_f32_c")]
    pub bin_accum: f32,
    /// § 4 (P5): a Home pantry's fraction of a spoiled unit, carried over.
    #[serde(default, skip_serializing_if = "is_zero_f32_c")]
    pub spoil_carry: f32,
    /// § 4: the diver holding the bin, until (one at a time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_diver: Option<(EntityId, Tick)>,
    /// § 4: the bin's regular (or the diver who may become one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_regular: Option<BinRegular>,
    /// Jobs & lives phase 1 (§ 2): a build paid to a Construction Yard
    /// stands closed (no places, no door) until this tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under_construction_until: Option<Tick>,
    /// Jobs & lives phase 1 (§ 1): the appended goods in stock
    /// (`Rations`, `Braindance`, `Organs`: `good as usize - 3`).
    #[serde(default, skip_serializing_if = "is_zero_stock_x")]
    pub stock_x: [u32; 3],
    /// The island pass phase 3b (I6): a Home that is company floors inside
    /// this workplace (the same rect and door; `World.company_homes` names
    /// it too, and `[company_housing.inside] rooms` the floors' name and
    /// whether they are a basement). The view draws the workplace as both.
    /// The city milestone C6a generalises it: a **tenancy**, a use on some
    /// floors of its **host** (the cut's main use, one structure on the
    /// tile map): the same rect, its own kind, owner, rent, capacity and
    /// door (its lot's street door on the host's wall, else the host's).
    /// Company floors are the tenancies `World.company_homes` names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<EntityId>,
    /// The city milestone C6a: the storeys this building stands on, `lo..=hi`
    /// (0 the ground): a tenancy's floors, a host's own (a Home's flat
    /// floors). `None`: the whole building from the ground (the 2k city, a
    /// building the importer gave no levels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<(u8, u8)>,
    /// C6a: a host's work units (shops, workshops, offices; a works floor
    /// one), the map's `units=`: `floors::vacant_units` less the uses on it.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub units: u16,
    /// The city milestone C6b: a tenancy's floor rent a day to its host's
    /// owner, the landlord (`Flow::FloorRent`), set at the rent pass
    /// (`floors::rent_pass`); 0 for a host, a vacant unit, a tenancy its
    /// landlord owns.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub floor_rent: i64,
    /// C6b: consecutive midnights its floor rent went short; at `[tenancy]
    /// evict_days` the landlord takes the floor back (`floors::vacate`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub floor_arrears: u8,
    /// C6b: its own opening hours, `[from, to)` minutes of the day (`to`
    /// below `from` runs past midnight); `None` reads its kind's row of
    /// `[tenancy] hours` (`floors::hours_of`), an owner's call later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hours: Option<(u16, u16)>,
    /// The city milestone C7: a host's lifts, `(cars, top storey)` from the
    /// map's `lift=` (the export's lift cores); `None`: its stairs only (a
    /// tenancy climbs its host's, `levels::core_of`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lift: Option<(u8, u8)>,
}

fn is_zero_stock_x(v: &[u32; 3]) -> bool {
    *v == [0; 3]
}

/// Hunger as sickness § 4: a bin's regular diver and the days (of the last
/// seven, bit 0 the last seen) it dove there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinRegular {
    pub who: EntityId,
    pub days: u8,
    pub last_day: u64,
}

impl BinRegular {
    pub fn new(who: EntityId, day: u64) -> BinRegular {
        BinRegular { who, days: 1, last_day: day }
    }

    /// The dive days of the 7 ending `day`.
    pub fn days_as_of(&self, day: u64) -> u8 {
        let gap = day.saturating_sub(self.last_day);
        if gap >= 7 {
            0
        } else {
            ((u16::from(self.days) << gap) & 0x7f) as u8
        }
    }

    /// A diver there on `need` of the last 7 days.
    pub fn established(&self, day: u64, need: u8) -> bool {
        self.days_as_of(day).count_ones() >= u32::from(need)
    }

    /// Another dive by the regular on `day`.
    pub fn saw(&mut self, day: u64) {
        self.days = self.days_as_of(day) | 1;
        self.last_day = day;
    }
}

pub fn default_tier() -> u8 {
    1
}

/// Jobs and room J6: the serde default of `floors`.
pub fn one_u8() -> u8 {
    1
}

pub fn is_one_u8(v: &u8) -> bool {
    *v == 1
}

impl Building {
    /// C6b: a fresh building of `kind` on `rect` (no owner, no stock, no
    /// staff): the founding of a unit's base, as the map's spawn.
    pub fn blank(kind: BuildingKind, rect: Rect, door: TilePos, tier: u8) -> Building {
        Building {
            kind,
            production_accum: 0.0,
            claim: None,
            child_food_debt: 0.0,
            rect,
            door,
            stock_food: 0,
            capacity: 0,
            owner: None,
            occupants: Vec::new(),
            demolished: false,
            tier,
            rent_per_day: 0,
            revenue_today: 0,
            revenue: VecDeque::new(),
            secured_by: None,
            derelict: false,
            full_capacity: None,
            empty_since: None,
            closed_until: None,
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
            floors: 1,
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
            span: None,
            units: 0,
            floor_rent: 0,
            floor_arrears: 0,
            hours: None,
            lift: None,
        }
    }

    /// Interior tiles (everything inside the perimeter), row-major.
    pub fn interior(&self) -> impl Iterator<Item = TilePos> + '_ {
        let r = self.rect;
        let (x0, y0) = (u32::from(r.x) + 1, u32::from(r.y) + 1);
        let (x1, y1) = (u32::from(r.x) + u32::from(r.w) - 1, u32::from(r.y) + u32::from(r.h) - 1);
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| TilePos { x: x as u16, y: y as u16 }))
    }

    pub fn is_full(&self) -> bool {
        self.occupants.len() >= usize::from(self.capacity)
    }
}
