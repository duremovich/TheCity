use super::*;

/// D34: guard-hours per Home (on-shift guards within `fear_radius` of the
/// door, counted on the hour), today and yesterday.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HomeWatch {
    pub today: BTreeMap<EntityId, u16>,
    pub yesterday: BTreeMap<EntityId, u16>,
}

/// Guard-on-shift ticks per zone (`Zone::index`), today and yesterday (M10 D33).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZoneWatch {
    pub today: [u32; 5],
    pub yesterday: [u32; 5],
}

// ---------------------------------------------------------------------------
// M12: districts (docs/M12_DISTRICTS.md § 1)
// ---------------------------------------------------------------------------

/// Most districts a config may define (the `[u32; 32]` watch slots, the
/// low 6 bits of `World::district_grid`, the `u32` adjacency and Feed
/// masks, the 5-bit district of a `DayTrace`). Scale S2: 12 before; 32 is
/// serde's largest derived array.
pub const MAX_DISTRICTS: usize = 32;

/// A district bitset (bit `DistrictId::index`).
pub type DistrictMask = u32;

/// Scale S2: a `[T; MAX_DISTRICTS]` from a save written with fewer slots
/// (12 before S2), the rest `fill`; extra slots are dropped.
fn district_slots_padded<'de, D, T>(d: D, fill: T) -> Result<[T; MAX_DISTRICTS], D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Copy,
{
    struct V<T>(T);
    impl<'de, T: Deserialize<'de> + Copy> serde::de::Visitor<'de> for V<T> {
        type Value = [T; MAX_DISTRICTS];
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of per-district values")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = [self.0; MAX_DISTRICTS];
            let mut i = 0;
            while let Some(x) = seq.next_element::<T>()? {
                if let Some(slot) = out.get_mut(i) {
                    *slot = x;
                }
                i += 1;
            }
            Ok(out)
        }
    }
    d.deserialize_tuple(MAX_DISTRICTS, V(fill))
}

/// [`district_slots_padded`] with `T::default()`.
pub fn district_slots_any_len<'de, D, T>(d: D) -> Result<[T; MAX_DISTRICTS], D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Copy + Default,
{
    district_slots_padded(d, T::default())
}

/// [`district_slots_padded`] with 1.0 (a weight lever's neutral value).
pub fn district_weights_any_len<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[f32; MAX_DISTRICTS], D::Error> {
    district_slots_padded(d, 1.0)
}

/// A district's index in `World::districts` (`[districts]` row order).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct DistrictId(pub u8);

impl DistrictId {
    /// Not yet known (a district not set).
    pub const UNSET: DistrictId = DistrictId(255);

    pub fn unset() -> DistrictId {
        DistrictId::UNSET
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn is_unset(self) -> bool {
        self == DistrictId::UNSET
    }
}

impl District {
    /// M12 phase 2 (plan deviation on "inhabited = `adults > 0`"): the
    /// district has standing Blocks and adults binned to it. A few homeless
    /// adults sleeping in the Civic made it "inhabited" at a crime rate of
    /// 257 per 100, drew Crackdowns and six to eight guards.
    pub fn inhabited(&self) -> bool {
        self.adults > 0 && !self.homes.is_empty()
    }
}

/// Who controls a district (plan D8): the top presence with at least
/// `[districts] control_min_share` of the total, else Contested.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Controller {
    #[default]
    Contested,
    City,
    Gang(EntityId),
    Corp(EntityId),
}

impl Controller {
    /// The CSV code (plan D45): 0 Contested, 1 City, 2 Gang, 3 Corp.
    pub fn csv_code(self) -> u8 {
        match self {
            Controller::Contested => 0,
            Controller::City => 1,
            Controller::Gang(_) => 2,
            Controller::Corp(_) => 3,
        }
    }

    /// Tie order (plan D8): City, then gangs, then corps; then lower id.
    pub fn tie_rank(self) -> (u8, EntityId) {
        match self {
            Controller::Contested => (3, EntityId::NONE),
            Controller::City => (0, EntityId::NONE),
            Controller::Gang(g) => (1, g),
            Controller::Corp(c) => (2, c),
        }
    }

    /// The controlling entity, if the controller is one.
    pub fn entity(self) -> Option<EntityId> {
        match self {
            Controller::Gang(e) | Controller::Corp(e) => Some(e),
            Controller::Contested | Controller::City => None,
        }
    }
}

/// The captain's per-district stance (phase 2 scores it; Patrol until then).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Stance {
    #[default]
    Patrol,
    Crackdown(EntityId),
    Sweep,
    Cordon,
    Withdrawn,
}

/// M12 D30: what a riot marches on.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RiotTarget {
    /// A corp-owned Market, Block or Hotel in the district.
    Corp,
    /// The Precinct.
    Precinct,
    /// The controlling gang's Hideout or held squat.
    Gang,
}

/// M12 D34: how the law meets a riot.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RiotResponse {
    /// Cordon the district; the guards fight only at the door.
    Contain,
    /// Fight at the door, then report the stragglers.
    Disperse,
    /// Cordon, and fight to kill (`crush_kill_mult`); fear for 14 days.
    Crush,
}

/// M12 D30: a district riot from its trigger at midnight to its clash (or
/// its fizzling). Rioters run the Raid goal (`raid::Expedition::Riot`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Riot {
    pub id: u32,
    pub district: DistrictId,
    pub target: EntityId,
    pub kind: RiotTarget,
    /// The muster building (rioters gather at its door).
    pub muster: EntityId,
    pub muster_at: Tick,
    /// Most miserable first, ties the lower id (`riot::eligible`'s order;
    /// `riot::promoted` promotes the head of it).
    pub rioters: Vec<EntityId>,
    pub response: RiotResponse,
    pub started: Tick,
    /// The first rioter out of the muster stamps it.
    #[serde(default)]
    pub departed: Option<Tick>,
}

/// Guard-on-shift ticks per district (`DistrictId::index`), today and yesterday.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DistrictWatch {
    #[serde(deserialize_with = "district_slots_any_len")]
    pub today: [u32; MAX_DISTRICTS],
    #[serde(deserialize_with = "district_slots_any_len")]
    pub yesterday: [u32; MAX_DISTRICTS],
}

/// One district: a cut of a zone (plan "District cuts"), its daily
/// aggregates (recomputed at midnight by `systems::districts::daily`; `trace`
/// explains each), its controller and, from phase 2, its law.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct District {
    pub id: DistrictId,
    pub name: String,
    pub zone: Zone,
    /// Buildings whose door is inside, sorted; rebuilt on load and on any wall change.
    #[serde(skip)]
    pub buildings: Vec<EntityId>,
    /// Walkable tiles inside no building rect (the litter denominator).
    #[serde(skip)]
    pub walk_tiles: u32,
    /// M12 phase 3: those tiles' grid indices, ascending (litter draws,
    /// means and sweeping).
    #[serde(skip)]
    pub streets: Vec<u32>,
    /// Standing, non-derelict Blocks (Homes) whose door is inside, sorted.
    #[serde(skip)]
    pub homes: Vec<EntityId>,
    /// Living adults binned here by the last aggregate pass, ascending.
    /// M14 phase 1 fix: saved (it was skipped, so a resumed world read no
    /// residents until the next midnight and a crash's Statistical-victim
    /// fallback found nobody: save/load diverged on seed 3).
    #[serde(default)]
    pub residents: Vec<EntityId>,
    /// Mean walkable street tile, rounded.
    #[serde(skip)]
    pub centroid: TilePos,
    // --- daily aggregates ---
    /// Every resident binned here (adults and children).
    #[serde(default)]
    pub population: u32,
    /// Living adults with a Brain binned here.
    #[serde(default)]
    pub adults: u32,
    /// Corp, Street, Dreg adults (`Class::index`).
    #[serde(default)]
    pub classes: [u32; 3],
    /// Mean `(mood + 1) / 2` of the resident adults.
    #[serde(default)]
    pub happiness: f32,
    /// `bind::district_coverage`, normalised `coverage_min..=coverage_max`.
    #[serde(default)]
    pub coverage: f32,
    #[serde(default)]
    pub fear: f32,
    /// Phase 4: Street + Dreg residents only.
    #[serde(default)]
    pub unrest: f32,
    #[serde(default)]
    pub unrest_streak: u16,
    /// M12 D41: the same formula over its Street-class residents only (the
    /// strike trigger: Dregs hold no shifts to walk out of).
    #[serde(default)]
    pub street_unrest: f32,
    /// Phase 3: mean litter ÷ 255 over `walk_tiles`.
    #[serde(default)]
    pub litter: f32,
    /// Crimes per day, the last 7, newest last.
    #[serde(default)]
    pub crimes: VecDeque<u16>,
    /// Crimes today, rolled into `crimes` at midnight.
    #[serde(default)]
    pub crimes_today: u16,
    /// Crimes per 100 residents per day over the last 7 days.
    #[serde(default)]
    pub crime_rate: f32,
    #[serde(default)]
    pub control: Controller,
    #[serde(default)]
    pub control_share: f32,
    #[serde(default)]
    pub control_since: Tick,
    /// The first control computation after a seed or a load is silent.
    #[serde(default)]
    pub control_init: bool,
    /// The last non-Contested controller (phase 1 review): a faction is
    /// shocked only when another faction takes the district from it, not
    /// when its share wobbles across `control_min_share` into Contested.
    #[serde(default)]
    pub last_holder: Controller,
    // --- the law (phase 2) ---
    #[serde(default)]
    pub stance: Stance,
    #[serde(default)]
    pub stance_since: Tick,
    /// City guards allocated today.
    #[serde(default)]
    pub guards: u16,
    /// Sanitation workers allocated today (phase 3).
    #[serde(default)]
    pub sweepers: u16,
    #[serde(default)]
    pub curfew: bool,
    #[serde(default)]
    pub crush_until: Option<Tick>,
    /// Rough sleepers last night (phase 2/3).
    #[serde(default)]
    pub rough: u16,
    #[serde(default)]
    pub vagrancy_log: VecDeque<Tick>,
    #[serde(default)]
    pub shakedowns: VecDeque<(Tick, EntityId)>,
    #[serde(default)]
    pub last_riot: Option<Tick>,
    #[serde(default)]
    pub last_strike: Option<Tick>,
    /// The inputs of the last daily pass, by name, for the District panel.
    #[serde(skip)]
    pub trace: Vec<(&'static str, f32)>,
    /// M12 D12: every stance's score from the last rescoring, best first.
    #[serde(skip)]
    pub stance_trace: Vec<StanceScore>,
    /// M12 D10: the allocation weight's terms from the last allocation.
    #[serde(skip)]
    pub alloc_trace: Vec<(&'static str, f32)>,
    /// Jobs & lives phase 1 (§ 2): the district's Utility Plant has had no
    /// operator on shift for `[jobs2] outage_hours` (the spec's `powered =
    /// false`, inverted so the derived default is the lit district).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dark: bool,
}

impl District {
    /// Jobs & lives phase 1: the lights are on (no outage).
    pub fn powered(&self) -> bool {
        !self.dark
    }
}
