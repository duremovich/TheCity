use super::*;

/// `[mood.housing]` (docs/ISLAND_PASS.md phase 3c, OPEN-177): a flat bias
/// on the hourly mood target from the agent's Home. The homeless read none
/// (sleeping rough already costs energy and safety). Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoodHousingCfg {
    /// By the Home's tier (Sump, Mid, Spire).
    pub tier: [f32; 3],
    /// Crowding: `crowd_w x min(excess, crowd_max) x factor`, the excess
    /// the residents a floor over `[buildings] home` capacity (a plain
    /// Block's beds a floor), less 1.
    pub crowd_w: f32,
    pub crowd_max: f32,
    /// The factor for company floors inside a workplace above it (dorm
    /// storeys, container bunks), and below it (`[company_housing.inside]
    /// rooms` `below`: the coffin racks). Any other Home's is 1.
    pub inside_factor: f32,
    pub below_factor: f32,
    /// A bed in company housing (the company town's grip): subtracted.
    pub company: f32,
}

/// The island pass phase 3a `[company_housing]` (docs/ISLAND_PASS.md I6):
/// company blocks seeded beside the far-commuting workplaces at world
/// build, their beds reserved for that workplace's staff, the rent docked
/// from the wage, the bed lost `grace_days` after the job. Every value a
/// placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyHousingCfg {
    /// The workplace kinds that put up company housing.
    pub kinds: Vec<BuildingKind>,
    /// A post is far when its holder's Home door (or the holder is
    /// homeless) is past this travel (`distance::travel`) from the
    /// workplace door; also the move-in rule for a new hire.
    pub far_ticks: u64,
    /// A company block's Lot: its door within this travel of the workplace
    /// door (the nearest first).
    pub lot_cap_ticks: u64,
    /// The share of a workplace's unmet far posts (far posts less the free
    /// beds within `far_ticks`) seeded as company beds at world build.
    pub seed_share: f32,
    /// No block for a workplace whose seeded want is below this many beds.
    pub min_beds: u32,
    /// At most this many blocks beside one workplace at world build.
    pub blocks_max: u32,
    /// With no vacant Lot within `lot_cap_ticks`, a block goes up on a
    /// plot of open ground this many tiles wide and high (walls included).
    pub plot_w: u16,
    pub plot_h: u16,
    /// A block's storeys: enough for the beds wanted, at most this (a Lot's
    /// own storeys at least).
    pub floors_max: u8,
    /// A company block's beds per floor (bunks; a plain Home's is
    /// `[buildings] home.capacity`), capped by the Lot's interior.
    pub beds_per_floor: u16,
    /// The company rent: this × `[rent] base[tier]` a day per housed
    /// worker, docked from the wage at collection.
    pub rent_mult: f32,
    /// A resident whose job at the workplace ended keeps the bed this many
    /// days, then is evicted.
    pub grace_days: u32,
    /// World build: the far posts' holders move into the seeded beds
    /// (the seed city lives where it is housed); off, only the new hires
    /// and the homeless staff fill them.
    pub fill_at_seed: bool,
    /// The island pass phase 3b (I6): the seeded beds as floors inside the
    /// workplace (`[company_housing.inside]`). `None` (not written into a
    /// save) with the key absent: 3a's blocks on a Lot,
    /// else a plot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<CompanyInsideCfg>,
    /// The island pass phase 3b (I6): a corp buying a nearby block for its
    /// far staff (`[company_housing.buyout]`). `None` (not written into a
    /// save) with the key absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buyout: Option<CompanyBuyoutCfg>,
}

/// C9b `[vertical.landlord]`: the landlord's answer to a gang's unpaid
/// floor rent. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LandlordCfg {
    /// A gang's Hideout this many midnights short in a row gets the
    /// landlord's answer ...
    pub answer_days: u8,
    /// ... and, still short, the next move on his ladder this many days on.
    pub again_days: u16,
    /// A person landlord this fearful (courage below it) whose tower's door
    /// cell the gang holds swallows the debt as protection.
    pub swallow_courage: f32,
    /// A landlord this lawful or more (a person's, a corp exec's) calls the
    /// law first; the city always does.
    pub law_lawfulness: f32,
    /// The muscle: a Beat on the floor's boss through the nearest Fixer, when
    /// the landlord's purse holds the quote and this much more.
    pub muscle_reserve: i64,
    /// The power to the lift cut this many days (the host's lift; 0: never).
    pub cut_days: u16,
    /// The tower sold to a buyer who will deal with the gang (its rival gang,
    /// else the richest Housing corp) at this share of its value.
    pub sell_mult: f32,
    /// The gang leader's grudge on the landlord for each move but the swallow.
    pub grudge: f32,
}

/// The city milestone C6b `[tenancy]`: the landlord, founding into a unit,
/// a lease of floors, a tenancy's hours. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TenancyCfg {
    /// A tenancy's floor rent a day per storey of its span, by its tier
    /// (Sump, Mid, Spire): a city-owned host's as is, a corp landlord's x its
    /// Housing level (the Home rent's rule), an agent's or a gang's as is.
    pub floor_rent: [i64; 3],
    /// And this share of the tenancy's mean daily revenue over its last
    /// seven days (a percentage rent), added to the base.
    pub revenue_share: f32,
    /// Days of floor rent short in a row before the landlord takes the
    /// floor back: a business to a vacant unit, a Home's lease to the
    /// landlord (its households stay, as his tenants).
    pub evict_days: u8,
    /// Founding takes a vacant unit within this walking travel (ticks) of
    /// the founder's Home door first, then a Lot.
    pub found_reach_ticks: u64,
    /// The kinds a unit takes (a shop front, a workshop, an office); the
    /// rest need a Lot.
    pub unit_kinds: Vec<BuildingKind>,
    /// A company buyout's lease of floors: x the storeys' value
    /// (`[company_housing.buyout] price_mult` applies on top), paid once to
    /// the landlord; the floors then pay floor rent.
    pub lease_mult: f32,
    /// The hours of a kind with no `hours` row, `[from, to)` minutes.
    pub default_hours: (u16, u16),
    /// Per kind: its opening hours (`to` below `from` runs past midnight).
    #[serde(default)]
    pub hours: Vec<TenancyHoursCfg>,
}

/// One `[tenancy] hours` row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TenancyHoursCfg {
    pub kind: BuildingKind,
    pub open: (u16, u16),
}

impl TenancyCfg {
    /// `kind`'s opening hours: its row, else `default_hours`.
    pub fn hours_of(&self, kind: BuildingKind) -> (u16, u16) {
        self.hours.iter().find(|r| r.kind == kind).map_or(self.default_hours, |r| r.open)
    }
}

/// The island pass phase 3b `[company_housing.inside]` (I6): the seeded
/// company beds are floors of the workplace building itself (a Home record
/// sharing the workplace's rect and door, no new footprint). Each kind's
/// floors have a display name, so the view can draw the building as both
/// workplace and warren. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyInsideCfg {
    /// Per workplace kind: the company floors' name and whether they sit
    /// below the workplace (a basement) or above it (storeys).
    pub rooms: Vec<CompanyRoomCfg>,
    /// The name for a kind with no `rooms` row.
    pub default_name: String,
}

/// One `[[company_housing.inside.rooms]]` row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyRoomCfg {
    pub kind: BuildingKind,
    pub name: String,
    /// The floors are a basement (else storeys on top).
    #[serde(default)]
    pub below: bool,
}

impl CompanyInsideCfg {
    /// The company floors' `(name, below)` inside a `kind` workplace.
    pub fn room(&self, kind: BuildingKind) -> (&str, bool) {
        self.rooms
            .iter()
            .find(|r| r.kind == kind)
            .map_or((self.default_name.as_str(), false), |r| (r.name.as_str(), r.below))
    }
}

/// The island pass phase 3b `[company_housing.buyout]` (I6): a corp whose
/// staff still commute far buys a nearby residential block and converts
/// it. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanyBuyoutCfg {
    /// The buyout pass runs at the midnight whose day is a multiple of this;
    /// each corp makes at most `per_corp` buyouts (or fallback builds) a pass.
    pub period_days: u32,
    pub per_corp: u32,
    /// A workplace buys when its far posts less its free company beds reach
    /// this many.
    pub min_want: u32,
    /// The block's door within this travel of the workplace door.
    pub cap_ticks: u64,
    /// Blocks of this tier and below only (0 Sump, 1 Mid, 2 Spire).
    pub tier_max: u8,
    /// The price: this x the block's value (`ownership::value(Home)` a
    /// floor, the first floor whole and each further one at `[floors]
    /// floor_cost_frac`).
    pub price_mult: f32,
    /// The corp buys only from what it holds above `treasury_ref` (its
    /// reserve), as a bankruptcy buyer.
    pub reserve: bool,
    /// The converted block's bunks a floor (the interior allowing; a block
    /// never loses beds by the conversion).
    pub bunks_per_floor: u16,
    /// With nothing for sale in reach, build a block on the nearest plain
    /// vacant Lot within `cap_ticks` (through `life::pay_housing`, the
    /// corp paying); false: no fallback.
    pub build_fallback: bool,
}

/// Island fix round F3 `[distance.free.stay_in]` (docs/ISLAND_PASS.md I1,
/// Dylan's D1 B; OPEN-190): with no HangOut spot within `spot_cap_ticks`
/// the Unwind is the own Home's HangOut (`leisure::stay_in_home`), with
/// its flatmates and Block-mates, else alone. A placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StayInCfg {
    /// The fun of an hour in alone (the screen, the deck, the window); in
    /// company, the larger of this and a HangOut's `gain.hangout_hour`.
    pub fun_hour: f32,
}

/// The island fix round F2 `[beds]` (docs/ISLAND_PASS.md I6, I8; OPEN-189,
/// OPEN-191, OPEN-193). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BedsCfg {
    /// A company bed is taken (or kept, by a corp's worker of another of its
    /// workplaces) only by staff whose post's door is within this travel of
    /// the bed's door (`distance::travel`).
    pub company_post_ticks: u64,
    /// Tonight's Hideout bed (the nearer bed, the watch, lying low) for a
    /// housed member: only within this travel from where the body stands;
    /// past it, Home.
    pub hideout_walk_ticks: u64,
    /// ... and, for a member with a post, only with the Hideout's door
    /// within this travel of the post's door (tomorrow's commute); past it,
    /// Home.
    pub hideout_work_ticks: u64,
    /// A Builder's far-site day ([island_trades] site_day) that starts away
    /// from the Yard walks straight to the site (the base: to the Yard to
    /// load, then out).
    pub site_from_home: bool,
}

/// M11 rent (docs/M11_OWNERSHIP.md § 4, plan D6/D7). Rent is per Home per day
/// by tier, split equally among the Home's adult residents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RentCfg {
    /// Per Home per day, by tier (0 Sump, 1 Mid, 2 Spire); a corp's Housing
    /// `price_level` multiplies it, the city's is `levers.city_rent`.
    pub base: [i64; 3],
    pub evict_days: u8,
    /// A homeless adult moves in with coins >= this x the Home's rent.
    pub rehouse_coins_mult: i64,
    /// An evicting owner refuses the evictee this long.
    pub refuse_days: u64,
    /// Tier-0 pantries spoil this much faster.
    pub sump_spoilage_mult: f32,
    /// Rent due is paid out of a wage or the dole the moment it is collected
    /// (before it can be spent on food), not only from what is left at
    /// midnight. Phase 2 measurement: see `assets/config.toml`.
    #[serde(default)]
    pub pay_from_income: bool,
    /// M11 phase 5: an evictee sleeps rough this many days before any owner
    /// takes them in (0 = re-housed the same night).
    #[serde(default)]
    pub rehouse_wait_days: u64,
}

impl RentCfg {
    /// A save from before M11: no rent and M9's spoilage (plan D40; the
    /// plan's `off()` keeps the 1.5 Sump spoilage, which would change an old
    /// save's behaviour, so it is 1.0 here).
    pub fn off() -> RentCfg {
        RentCfg {
            base: [0, 0, 0],
            evict_days: 7,
            rehouse_coins_mult: 3,
            refuse_days: 30,
            sump_spoilage_mult: 1.0,
            pay_from_income: false,
            rehouse_wait_days: 0,
        }
    }
}

/// M18 P5h (housing as an errand, `systems::housing`): a homeless adult's
/// FindHome goal walks to a letting desk and applies; the landlord's kind
/// decides. Every number of the errand, the refusals and the capsule tenure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HousingCfg {
    /// Off: no FindHome goal, no daily roll (nobody is housed off the
    /// street but by a marriage, tied housing or the company beds).
    pub enabled: bool,
    /// The city's waiting list: a place this many days after joining it.
    pub list_days: u64,
    /// A corp refuses a record (a sentence served or a warrant) within this many days.
    pub corp_record_days: u64,
    /// A corp refuses an applicant who has had no Home of his own within
    /// this many days (no address history; a capsule is no address).
    pub corp_address_days: u64,
    /// A gang landlord's protection rides on the rent: its Homes' rent is
    /// times `1 + gang_protection` (the gang's cut, to the gang).
    pub gang_protection: f32,
    /// A landlord who refused an applicant is not asked again for this many days.
    pub refusal_days: u64,
    /// The desks are open from this hour to `desk_close` (an applicant walks then).
    pub desk_open: u8,
    pub desk_close: u8,
    /// The Apply step's minutes at the desk.
    pub apply_mins: u64,
    /// The FindHome goal's flat score and its urgency's Linear curve (m, b)
    /// over the days homeless (`days / urgency_days`, capped at 1).
    pub flat: f32,
    pub curve: (f32, f32),
    pub urgency_days: f32,
    /// An applicant asks about a Home at most this many tiles from where he
    /// stands (the neighbourhood he looks in: a place on the map).
    pub reach_tiles: u32,
    /// T1 (`OPEN-304`): ... and goes to its letting desk only within this
    /// many travel ticks (`distance::trip`: the walk, or the El where it
    /// wins), so the city's one desk at the Civic Hall is ridden to.
    pub desk_reach_ticks: u64,
    /// The player's menu at a desk lists at most this many places.
    pub menu_rows: usize,
    /// The capsule hostel: a nightly chance a capsule holder's locker is
    /// robbed, and the share of his coins taken (to a fellow guest).
    pub locker_rob_p: f32,
    pub locker_rob_share: f32,
    /// The public capsule hostels the world build opens: one per this many
    /// residents (none below it), dealt round the districts of the zones in
    /// `hostel_zones`, on vacant lots, owned by the city. 0: none.
    pub hostel_residents: usize,
    pub hostel_zones: Vec<crate::components::Zone>,
}

impl Default for HousingCfg {
    fn default() -> Self {
        HousingCfg {
            enabled: true,
            list_days: 10,
            corp_record_days: 30,
            corp_address_days: 60,
            gang_protection: 0.5,
            refusal_days: 7,
            desk_open: 8,
            desk_close: 18,
            apply_mins: 20,
            flat: 0.55,
            curve: (0.4, 0.6),
            urgency_days: 7.0,
            reach_tiles: 400,
            desk_reach_ticks: 120,
            menu_rows: 6,
            locker_rob_p: 0.03,
            locker_rob_share: 0.5,
            hostel_residents: 6000,
            hostel_zones: vec![crate::components::Zone::Sump],
        }
    }
}
