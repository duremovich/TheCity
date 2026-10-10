# Living City Simulator: Build Spec v1

Oct 3, 2026 · @Dylan

A hand-off spec for an implementing agent: one city, 2D top-down squares, utility goal selection over a GOAP planner, with economy, law, social graph and demography as world systems. Companion to the feasibility doc.

## Decisions made for v1

These settle the open questions from the feasibility doc so an agent can start. Each is a one-line change if you disagree; the rest of the spec is written against them.

| Decision | Choice for v1 | Why | To overrule |
| --- | --- | --- | --- |
| Player role | God/mayor with levers (no player character) | Removes a whole control scheme; the sim is the product. A walking character can be added later as one more agent with a human brain | Add a `PlayerControlled` component and an input system; everything else stays |
| Language and engine | Rust, workspace of three crates: `citysim` (headless library) and `citysim-app` (macroquad renderer) and citysim-cli (headless runner and calibration) | Fast enough for 2,000 agents without tricks; `cargo test` lets the agent verify behaviour headlessly; macroquad draws squares in \~50 lines with no editor needed | Same architecture in C# (`citysim` class library + MonoGame) is a straight port; keep the crate split |
| ECS | Hand-rolled: `Vec<Option<Component>>` per component type indexed by `EntityId` (generational index) | Avoids Bevy's API churn for an agent; the sim needs \~15 component types, not a general ECS | Swap to `hecs` or `bevy_ecs` if iteration speed becomes the bottleneck |
| Map | Fixed, hand-authored 96×64 tile grid loaded from a text file | Generated maps add nothing to the AI and cost weeks | Replace the loader; nothing downstream knows the map was authored |
| Time | Fixed timestep: 1 tick = 1 in-game minute; 1 in-game day = 1,440 ticks; default speed 1 day per 3 real minutes (8 ticks per real second), range 1×–1,000× | Deterministic, replayable; day length is a single constant | Change `TICKS_PER_DAY` and the speed table |
| Determinism | Seeded `ChaCha8` RNG, no wall-clock reads, no hash-map iteration in sim logic (use `BTreeMap` or sorted `Vec`) | Replayable bug reports; same seed = same city | None advised |
| Core scarcity | Food | Matches the target emergent chain; one scarcity is enough to prove the loop | Add goods to the economy table |
| Population | 300 citizens at start, 50 full-fidelity budget, rest coarse or statistical | Small enough to inspect, large enough for gangs and famine | Constants in `config.toml` |
| Scripting | None; all behaviour in Rust data tables (`const` arrays or TOML) | An agent can change a TOML row faster than a script runtime can be built | A Lua layer can be added over the same tables later |

## Scope

v1 is done when a 300-citizen city runs unattended for 120 in-game days and the log shows the chain hunger → theft → arrest → jail → gang recruitment occurring without scripting, and the player can change the outcome with a lever.

**Must do**

- Citizens with needs, personality, mood, jobs, wallets, homes and families.
- Utility goal selection over a GOAP planner; F.E.A.R.-style three-state execution.
- Economy with one good (food), prices that respond to stock, wages from a treasury, seasons.
- Law: witnesses, memories, crime reports, guards who arrest, a jail with capacity and sentences.
- Social graph with affinity/trust/debt; friendship, courtship, marriage, one gang with recruitment and income.
- Demography: ageing, birth, death (starvation, violence, age), corpses, burial.
- Three LOD levels with a 50-agent full-fidelity budget and 1×–1,000× speed.
- Player levers: release food reserve, set tax, set sentence length, hire/fire guards, give coins to a citizen, arrest/release a citizen, demolish/build Home.
- Inspector for any agent (needs, scores, plan, memories, edges) and a filterable event log.
- Deterministic save/load as plain data (RON or JSON).

**Must not (v1)**

- No second city, no travel, no procedural map.
- No combat simulation beyond a single contested roll.
- No dialogue, no text generation, no LLMs.
- No audio, sprites, animation; squares and text only.
- No multiplayer, no mod loading, no scripting runtime.

## World model

A fixed 96×64 tile map with 69 buildings, one food good, integer coins, and a 1,440-tick day in four phases.

### Map file format

`assets/map.txt`, UTF-8, exactly 64 lines of 96 characters (row 0 = top, `y` down, `x` right), then one blank line, then one building line per building. The loader panics on any dimension or legend violation.

| Char | TileKind | Walkable | Notes |
| --- | --- | --- | --- |
| `.` | Ground | yes | move cost 1.0 |
| `#` | Wall | no |  |
| `=` | Road | yes | move cost 0.7 |
| `D` | Door | yes | must lie on a building rect's perimeter; exactly one per building |
| `f` | Farmland | yes | must lie inside a Farm rect |
| `~` | Water | no |  |

Building line: `B <Kind> <x> <y> <w> <h>` (rect = tiles `x..x+w`, `y..y+h`; perimeter must be `#` except one `D``, and the tile outside that D must be Road). The door tile is derived from the grid. Building EntityIds are assigned in file order. The implementing agent authors the map in M0 with a small generator script (kept in the repo as assets/gen_map.py): a road grid every 8 tiles, 5×4 Homes in rows along the roads, the two Farms on the east edge, Market and Hall at the centre crossing, Bar beside the Market, Jail beside the Hall, Cemetery and Hideout in opposite corners, Warehouse beside the Farms, a Water strip along the south edge; the script output is committed as map.txt`.

```
B Home 3 3 5 4
B Farm 40 10 12 9
```

#### Map format v2 (M10)

Since M10 `assets/map.txt` is the 256 x 192 v2 map and the v1 map lives on as `assets/map_v1.txt` (used by `Config::v1_profile()` in unit tests). v2 layout: a first line `<w> <h>` (each at most 256, because `TilePos` is `u8`), then `h` tile rows of `w` characters, a blank line, then `h` zone rows of `w` characters from `S` Spire, `C` Civic, `V` Vats, `M` Mid, `U` Sump, a blank line, then the building lines `B <Kind> <x> <y> <w> <h> [<tier>]` (tier defaults to 1). A file without the `<w> <h>` header is read as v1: width and height from the rows, every tile `Zone::Mid`, every tier 1. `BuildingKind` gains `Lot` (a wall-less rect with one door, inert until M11) and `SecurityOffice` (inert). The generator is `assets/gen_map.py`. The full table of the v2 city is in [M10_SCALE.md](M10_SCALE.md) section 2.

### Building definition table

Capacity = max agents inside at once (excess queue at the door for up to 30 ticks, then the plan step fails). Advertised actions are GOAP actions the building offers to any agent, as The Sims' smart objects do. `dur` in ticks.

| Kind | Count | Footprint | Capacity | Stocks / produces | Advertised actions (effects) |
| --- | --- | --- | --- | --- | --- |
| Home | 60 | 5×4 | 6 residents | `stock_food` pantry, cap 30 | `Sleep` (dur 480, energy → 1.0, safety +0.3); `EatAtHome` (dur 15, −1 pantry, hunger +0.5); `StoreFood` (dur 5, inventory → pantry); `Rest` (dur 60, energy +0.1) |
| Farm | 2 | 12×9 (≥60 `f` tiles) | 12 workers | produces Food into `stock_food`, cap 400 | `FarmWork` (job only, dur 60, +1.25 × season\_mult food, farming skill +0.001); `HaulToMarket` (job only, dur 30, moves min(50, stock) to Market); `StealFood` (dur 20, −min(3, stock), +3 inventory, crime Theft) |
| Market | 1 | 8×6 | 20 | `stock_food` cap 2000, `price_food` | `BuyFood` (dur 5, −price coins, +1 food, qty 1–3); `SellFood` (dur 5, +floor(price × 0.6) coins, −1 food); `ClerkWork` (job only, dur 60); `StealFood` (dur 15, −2 stock, +2 inventory, Theft); `Fence` (gang only, sells stolen food at price × 0.8, dur 10) |
| Bar | 1 | 8×6 | 24 | none | `Drink` (dur 60, −2 coins, belonging +0.15, memory valence +0.2); `Chat` (dur 30, belonging +0.3 both, affinity +0.05); `Flirt` (dur 30, intimacy +0.1, affinity +0.1 if accepted); `BartendWork` (job only, dur 60) |
| Jail | 1 | 6×6 | 16 inmates | none | `ServeTime` (inmate only, until `Sentence.until_tick`; hunger decays at half rate; fed 1 food/day from treasury); `GuardJail` (guard job, dur 60) |
| Cemetery | 1 | 8×8 | 8 | graves counter | `BuryCorpse` (dur 60, `Corpse.buried = true`, burier belonging +0.1, memory Grief salience 0.5 if kin) |
| Hall | 1 | 10×6 | 20 | `Treasury.coins` | `CollectWage` (job only, `+wage from treasury, tax withheld); CollectDole (unemployed adults, once per day); ReportCrime (creates CrimeReport`) |
| Hideout | 1 | 7×5 | 12 | `Gang.treasury`, `stock_food` cap 200 | `JoinGang` (dur 30, adds `GangMember`); `Extort` (gang only, target Home`, −5 coins from victims into the actor's wallet, crime Extortion); SplitLoot (50% of the day's haul into Gang.treasury); see the Action table for durations`; `HideFromLaw` (dur 120, safety +0.4) |
| Warehouse | 1 | 8×6 | 6 | `stock_food` cap 3000, city reserve | `ReleaseReserve` (player lever only, moves N to Market); `StealFood` (dur 30, −5 stock, +5 inventory, Theft, requires stealth ≥ 0.4) |

The Hall treasury pays wages for Guard, Clerk, Bartender and Gravedigger; farms are city-owned in v1 so Farmer wages come from the treasury too.

### Time

| Constant | Value |
| --- | --- |
| `TICKS_PER_DAY` | 1440 |
| `TICKS_PER_HOUR` | 60 |
| `DAYS_PER_SEASON` | 30 |
| `DAYS_PER_YEAR` | 120 |
| `tick_of_day` | `tick % 1440` |
| `day` | `tick / 1440` |
| `is_dark` | \`tick\_of\_day < 360 or tick\_of\_day >= 1260 (21:00–06:00) |

| DayPhase | tick\_of\_day | Clock |
| --- | --- | --- |
| Night | 0–359 | 00:00–05:59 |
| Morning | 360–539 | 06:00–08:59 |
| Work | 540–1079 | 09:00–17:59 |
| Evening | 1080–1439 | 18:00–23:59 |

Default shift for all jobs: Work phase, 6 days in 7 (`day % 7 != 6`). Half the guards (even `EntityId.index`) work a night shift 1260–1439 plus 0–359 instead.

### Seasons

| Season | Days in year | `farm_yield_mult` | `energy_decay_mult` |
| --- | --- | --- | --- |
| Spring | 0–29 | 1.0 | 1.0 |
| Summer | 30–59 | 1.4 | 1.0 |
| Autumn | 60–89 | 1.1 | 1.0 |
| Winter | 90–119 | 0.3 | 1.15 |

Season = `(day % 120) / 30`. Expected farm output: 24 farmers × 9 h × 1.4 × (0.6 + 0.8 × farming) × mult ≈ 300 food/day in Spring at farming 0.5 and ≈ 90 in Winter, against \~300/day consumption at one meal per citizen, so the city is balanced in Spring and Summer and short in Winter. The Warehouse reserve is released only by the player.

### Resources

| Resource | Type | Where held |
| --- | --- | --- |
| Food | `u32`; 1 unit = one meal | `Inventory.food`, `Building.stock_food`, `Market.stock_food` |
| Coins | `i64` | `Wallet.coins`, `Treasury.coins`, `Gang.treasury` |

Market price rule (Economy system, once per day at `tick_of_day == 0`): `price_food = clamp(round(3.0 × sqrt(600 / max(stock_food, 7))), 1, 30), giving stock 7 → 28, 50 → 10, 600 → 3, 2400 → 2, 10000 → 1`.

### Initial world state

| Item | Value |
| --- | --- |
| Population | 300 agents, age `U(18, 60)` years (`age_days = years × 120`), sex 50/50 |
| Homes | 60, 5 residents each (seeded shuffle); in each home one pair is flagged Spouse with probability 0.6 |
| Jobs | 24 Farmer, 10 Guard, 3 Clerk, 2 Bartender, 1 Gravedigger; the other 260 have no `Job` component and draw the dole (3 coins/day at the Hall, see Economy) |
| Coins per citizen | `U_int(10, 40)` |
| Food | `U_int(0, 2)` per citizen in `Inventory`; each Home pantry starts at 10 |
| Market stock | 600 |
| Warehouse stock | 1500 |
| Farm stock | 0 each |
| Treasury | 5000 coins |
| Gang | one `Gang` entity "The Hollow", 0 members, treasury 50, territory `[Hideout]`, leader `None` |
| Price | `price_food = 3` |

## Data schema

Every component is a plain struct stored as `Vec<Option<T>>` indexed by `EntityId.index`; the relationship graph is a `BTreeMap` on the `World`, not a component.

### Core types

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EntityId { pub index: u32, pub generation: u32 }

pub type Tick = u64;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct TilePos { pub x: u8, pub y: u8 }          // 0..96, 0..64

#[derive(Copy, Clone, Debug)]
pub struct Rect { pub x: u8, pub y: u8, pub w: u8, pub h: u8 }

pub enum TileKind { Ground, Wall, Road, Door, Farmland, Water }
pub enum BuildingKind { Home, Farm, Market, Bar, Jail, Cemetery, Hall, Hideout, Warehouse }
pub enum Sex { Male, Female }
pub enum Lod { Full, Coarse, Statistical }
pub enum Role { Farmer, Guard, Clerk, Bartender, Gravedigger }
pub enum Crime { Theft, Extortion, Assault, Murder }
pub enum DeathCause { Starvation, Violence, OldAge, Execution }
pub enum RelKind { Acquaintance, Friend, Family, Spouse, Parent, Rival, Enemy }
pub enum MemoryKind { Ate, Starved, WasRobbed, SawCrime, WasArrested, Fought, Won, Lost,
                      Socialised, Rejected, Courted, Married, Grief, SawCorpse, Paid, Unpaid }
pub enum GoalKind { Eat, Sleep, Work, Earn, Socialise, Court, Flee, Fight, ReportCrime,
                    Patrol, Arrest, JoinGang, GangWork, Bury, Idle }
```

### Agent components

```rust
pub struct Position {
    pub tile: TilePos,
    pub building: Option<EntityId>,    // Some if inside a building rect (incl. door)
}

pub struct Identity {
    pub name: String,                  // "<First> <Last>" from name tables
    pub age_days: u32,                 // 0..=12000 (100 years); +1 per day
    pub sex: Sex,
    pub born_tick: i64,                // negative for the initial population
}

pub struct Needs {                     // all f32 in 0.0..=1.0; 1.0 = fully satisfied, 0.0 = critical
    pub hunger: f32,                   // default 0.8
    pub energy: f32,                   // default 0.9
    pub safety: f32,                   // default 0.8
    pub wealth: f32,                   // derived hourly: clamp(coins / (7*price_food*(1+greed)), 0, 1)
    pub belonging: f32,                // default 0.6
    pub intimacy: f32,                 // default 0.5
    pub starving_since: Option<Tick>,  // Some(t) while hunger == 0.0
}

pub struct Personality {               // all f32 in 0.0..=1.0
    pub lawfulness: f32,
    pub greed: f32,
    pub pride: f32,
    pub sociability: f32,
    pub courage: f32,
    pub loyalty: f32,
}

pub struct Mood {
    pub value: f32,                    // -1.0..=1.0, default 0.0
    pub low_since: Option<Tick>,       // Some while value < -0.8
    pub last_computed: Tick,
}

pub struct Wallet { pub coins: i64 }   // >= 0 for agents; debts live on edges
pub struct Inventory { pub food: u32, pub stolen_food: u32 } // food 0..=20 (carry cap 20); stolen_food <= food

pub struct Job {
    pub employer: Option<EntityId>,    // building entity
    pub role: Role,
    pub wage_per_day: i64,             // Farmer 6, Guard 8, Clerk 6, Bartender 5, Gravedigger 5
    pub shifts: Vec<(u16, u16)>,       // tick_of_day ranges [start, end); default [(540, 1080)]
    pub days_unpaid: u8,               // quits at 7
    pub tax_accum: f32,
}

pub struct Household { pub home: Option<EntityId> }  // None = homeless

pub struct Brain {
    pub lod: Lod,                      // default Coarse
    pub current_goal: Option<GoalKind>,
    pub goal_since: Tick,
    pub plan: Option<Plan>,            // see the GOAP section; Plan holds goal, target, steps, started_tick
    pub plan_step: u8,
    pub cooldowns: BTreeMap<GoalKind, Tick>,
    pub pinned: bool,
    pub betraying: bool,
    pub last_think: Option<ThinkTrace>, // top-5 goals and consideration outputs, for the inspector
    pub last_think_tick: Tick,
    pub action_until: Tick,            // Coarse: when the current action resolves
}

pub struct MemoryEntry {
    pub kind: MemoryKind,
    pub subject: Option<EntityId>,
    pub tick: Tick,
    pub salience: f32,                 // 0.0..=1.0 magnitude
    pub valence: f32,                  // -1.0..=1.0 sign of mood impact
}
pub struct Memory { pub entries: Vec<MemoryEntry> }  // cap 24; evict lowest salience*recency

pub struct Skills {                    // f32 0.0..=1.0, initial U(0.1, 0.4)
    pub stealth: f32,
    pub fighting: f32,
    pub farming: f32,
}

pub struct Sentence { pub until_tick: Tick, pub crime: Crime }       // present only while jailed
pub struct GangMember { pub gang: EntityId, pub rank: u8 }           // 0 grunt, 1 lieutenant, 2 leader
pub struct Corpse { pub died_tick: Tick, pub cause: DeathCause, pub buried: bool }

pub struct ActionInstance {
    pub action: ActionKind,            // see the GOAP section
    pub target: Option<EntityId>,      // building or agent
    pub tile: Option<TilePos>,
}
```

### Non-agent entities

```rust
pub struct Building {
    pub kind: BuildingKind,
    pub production_accum: f32,         // Farm only
    pub extort_count: u8,              // Home only
    pub rect: Rect,
    pub door: TilePos,
    pub stock_food: u32,               // 0..=cap per kind table
    pub capacity: u8,
    pub owner: Option<EntityId>,       // None = city-owned
    pub occupants: Vec<EntityId>,      // sorted
}

pub struct Gang {
    pub name: String,
    pub members: Vec<EntityId>,        // sorted
    pub treasury: i64,
    pub territory: Vec<EntityId>,      // building ids, sorted
    pub leader: Option<EntityId>,
    pub empty_since: Option<Tick>,
}

pub struct Market {
    pub price_food: i64,               // 1..=30; stock lives on the Market building's Building.stock_food
    pub price_history: VecDeque<i64>,  // cap 120, one per day
}

pub struct Treasury { pub coins: i64 } // may go negative; wages unpaid while < 0

pub struct CrimeReport {               // stored in World::crime_reports
    pub crime: Crime,
    pub suspect: EntityId,
    pub witness: Option<EntityId>,      // None for a Statistical-tick theft
    pub tick: Tick,
    pub resolved: bool,
}
```

### Relationships

```rust
pub struct Edge {
    pub affinity: f32,                 // -1.0..=1.0, default 0.0
    pub trust: f32,                    // 0.0..=1.0, default 0.3
    pub debt: i32,                     // coins the lower id owes the higher id; negative = reverse
    pub kind: RelKind,                 // default Acquaintance
    pub last_interaction: Tick,
    pub last_birth_tick: Option<Tick>, // Spouse edges only
}
// Key is always (min(a, b), max(a, b)).
// Edges with |affinity| < 0.05 and no interaction for 30 days are pruned daily.
```

### World skeleton

```rust
pub struct World {
    pub tick: Tick,
    pub rng: ChaCha8Rng,
    pub map: Map,                                   // [[TileKind; 96]; 64] + nav grid
    // entity allocator
    pub generations: Vec<u32>,
    pub free_list: Vec<u32>,
    pub alive: Vec<bool>,
    // agent components (index = EntityId.index)
    pub position: Vec<Option<Position>>,
    pub identity: Vec<Option<Identity>>,
    pub needs: Vec<Option<Needs>>,
    pub personality: Vec<Option<Personality>>,
    pub mood: Vec<Option<Mood>>,
    pub wallet: Vec<Option<Wallet>>,
    pub inventory: Vec<Option<Inventory>>,
    pub job: Vec<Option<Job>>,
    pub household: Vec<Option<Household>>,
    pub brain: Vec<Option<Brain>>,
    pub memory: Vec<Option<Memory>>,
    pub skills: Vec<Option<Skills>>,
    pub sentence: Vec<Option<Sentence>>,
    pub gang_member: Vec<Option<GangMember>>,
    pub corpse: Vec<Option<Corpse>>,
    // non-agent components
    pub building: Vec<Option<Building>>,
    pub gang: Vec<Option<Gang>>,
    pub market: Vec<Option<Market>>,
    pub treasury: Vec<Option<Treasury>>,
    // graph + blackboard
    pub edges: BTreeMap<(EntityId, EntityId), Edge>,
    pub crime_reports: Vec<CrimeReport>,
    pub buildings_by_kind: BTreeMap<BuildingKind, Vec<EntityId>>,
    pub agents_by_tile: BTreeMap<TilePos, Vec<EntityId>>,  // rebuilt each tick for Full agents
    pub levers: Levers,                                     // player settings
    pub stats: DailyStats,                                  // births, deaths, crimes, prices
    pub events: VecDeque<Event>,                            // ring of 50,000 since M10 (`EVENT_RING_CAP`; ids are contiguous, so `event_mut(id)` is O(1)), never drained; the UI keeps a read cursor
    pub plan_queue: BTreeMap<(Reverse<OrderedFloat<f32>>, EntityId), Tick>, // value = enqueue tick
    pub reservations: BTreeMap<EntityId, Vec<Reservation>>,
    pub door_queue: BTreeMap<TilePos, u8>,
    pub flow_fields: BTreeMap<EntityId, FlowField>,         // #[serde(skip)]
    pub last_seen: BTreeMap<EntityId, (TilePos, Tick)>,
    pub colocation: BTreeMap<(EntityId, EntityId), u16>,
    pub vacancies: BTreeMap<EntityId, Vec<Role>>,
    pub edge_roads: Vec<TilePos>,
    pub view_rect: Option<Rect>,
    pub command_queue: Vec<PlayerCommand>,
    pub command_log: Vec<(Tick, PlayerCommand)>,
}

pub fn tick(world: &mut World);
```

## Needs, personality and mood

Convention: every need is `f32` in `0.0..=1.0` where **1.0 = fully satisfied, 0.0 = critical**. Goal selection uses urgency `U(need) = 1.0 − need`.

### Decay

Applied every tick to Full and Coarse agents; Statistical agents apply 60× once per hour.

| Need | Decay per tick | Derivation | Modifiers |
| --- | --- | --- | --- |
| hunger | 0.000347 | 1.0 → 0.0 in 2,880 ticks (2 days) | ×0.5 while jailed or asleep; ×1.3 during `FarmWork` |
| energy | 0.000926 | 1.0 → 0.0 in 1,080 ticks (18 h awake) | ×`energy_decay_mult` (Winter 1.15); ×1.5 while fleeing or fighting; 0 while sleeping |
| safety | no passive decay; recovers +0.0010/tick toward 1.0 when no threat within 6 tiles | — | event drops: witnessed crime −0.2; robbed −0.4; assaulted −0.6; saw corpse −0.1; a guard within 6 tiles doubles recovery |
| wealth | recomputed hourly: `clamp(coins / (7 × price_food × (1 + greed)), 0, 1)` | 7 days of food = secure | greed raises the bar |
| belonging | 0.000139 | 1.0 → 0.0 in 7,200 ticks (5 days) | ×(0.5 + sociability) |
| intimacy | 0.0000694 | 1.0 → 0.0 in 14,400 ticks (10 days) | ×0.5 under 18 years (`age_days < 2160`) |

### Satisfiers

| Action | Effect |
| --- | --- |
| Eat 1 food, any source | hunger +0.5 (clamped); memory `Ate` salience 0.1, valence +0.2 |
| Sleep | energy +1/480 per tick (8 h = full); at Home: safety +0.3 on completion; on the street: safety −0.1, energy gain ×0.6 |
| Rest 60 ticks | energy +0.1 |
| Chat 30 ticks | belonging +0.3 both parties; affinity +0.05 |
| Drink 60 ticks | belonging +0.15; −2 coins |
| Flirt accepted | intimacy +0.1, affinity +0.1 each; rejected: memory `Rejected` valence −0.3 |
| Sleep in the same Home as Spouse | intimacy +0.4 on completion |
| Court success (edge → Spouse) | intimacy +0.5; memory `Married` salience 0.8, valence +0.8 |
| Receive wage | wealth recomputed; memory `Paid` valence +0.1 |
| HideFromLaw / inside own Home | safety +0.4 / +0.002 per tick |
| Guard patrol passes within 6 tiles | safety +0.05 to citizens, once per guard per hour |

### Starvation and age

- When `hunger` reaches 0.0: set `starving_since = Some(tick)` if `None`; add memory `Starved` (salience 0.6, valence −0.6) once per day.
- When `hunger > 0.0`: `starving_since = None`.
- When `tick − starving_since >= 4320` (3 days): the agent dies with `DeathCause::Starvation`. All components are removed except `Position` and `Identity`; a `Corpse` is added.
- From `age_days >= 10800` (90 years) each day carries a 0.01 chance of death by `OldAge`.

### Personality

Six axes, `f32 0.0..=1.0`: `lawfulness, greed, pride, sociability, courage, loyalty`.

- Initial population: each axis = median of three independent `U(0,1)` draws (this is Beta(2,2)).
- Birth: `clamp((mother + father) / 2 + N(0, 0.15), 0, 1)` per axis; an unknown parent contributes 0.5.

Trait drift is applied by whichever system raises the event, clamped to 0..1:

| Event | Drift |
| --- | --- |
| Arrested | lawfulness −0.05; pride −0.03 |
| Served full sentence | lawfulness +0.02 |
| Starving (per day with `starving_since` set) | lawfulness −0.02; courage +0.01 |
| Crime unpunished after 30 days | lawfulness −0.03; greed +0.02 |
| Joined gang | loyalty +0.10; lawfulness −0.10 |
| Left gang or gang disbanded | loyalty −0.15 |
| Won fight | courage +0.03; pride +0.02 |
| Lost fight | courage −0.05 |
| Robbed or extorted | lawfulness +0.02; courage −0.02 |
| Married | loyalty +0.05; sociability +0.02 |
| Spouse died | sociability −0.05 |
| Paid on time 7 days running | lawfulness +0.01 |
| Wage unpaid (per day) | lawfulness −0.01; loyalty −0.02 |
| Reported a crime | lawfulness +0.01 |
| Each birthday after 40 years | greed −0.005; courage −0.005 |

### Mood

Computed once per hour (`tick_of_day % 60 == 0`) for all agents; `h, e, s, w, b, i` are the six needs.

```
need_term   = 0.30*(2h-1) + 0.15*(2e-1) + 0.20*(2s-1) + 0.15*(2w-1) + 0.12*(2b-1) + 0.08*(2i-1)
memory_term = clamp( sum over memories m with age_days(m) <= 7 of
                     m.valence * m.salience * 0.5^(age_days(m)/3), -1, 1 )
raw         = clamp( 0.7*need_term + 0.3*memory_term, -1, 1 )
delta       = 0.2 * (raw - mood.value)
if delta < 0 { delta *= 1 + 0.5*pride }        // the proud take setbacks harder
mood.value  = clamp( mood.value + delta, -1, 1 )
```

| Threshold | Consequence |
| --- | --- |
| `mood < -0.3` | Socialise consideration ×1.2; Chat affinity gain halved |
| `mood < -0.6` | Fight goal gets a flat +0.15 after compensation; Assault cost ignores lawfulness above 0.7; 2% hourly chance of a `Tantrum` memory for witnesses (valence −0.2) |
| `mood < -0.8` for 7 continuous days (`low_since`) | Agent leaves the city: walks to a map-edge Road tile, is despawned, logged `Emigrated`; household and edges cleaned up |
| `mood > 0.5` | Court and Socialise ×1.2; birth probability for Spouse pairs ×1.5 |

## Goal selection

Utility scoring runs every 30 ticks, staggered by `tick % 30 == id.index % 30`, for Full and Coarse agents, and immediately on plan failure. Statistical agents skip it and use calibrated hourly probabilities (see Level of detail).

### Response curves

Inputs are normalised to `x ∈ [0,1]`; outputs are clamped to `[0,1]`.

| Curve | Params | Formula |
| --- | --- | --- |
| `Linear{m, b}` | slope, intercept | `y = m*x + b` |
| `Quadratic{k, m, c, b}` | exponent, slope, x-shift, y-shift | `y = m*(x − c)^k + b` |
| `Logistic{k, mid}` | steepness, midpoint | `y = 1 / (1 + e^(−k*(x − mid)))` |
| `Step{t, lo, hi}` | threshold, below, at-or-above | `y = x >= t ? hi : lo` |

Input shorthands: `U(need) = 1 − need`; `Can(cond)` = 1 or 0; `Phase(set)` = 1 if `tick_of_day` falls in a listed phase, else the listed `lo`.

### Scoring

```
raw   = product over i of curve_i(x_i)        // n considerations; any 0 makes the goal unavailable
mod   = (1 − raw) * (1 − 1/n)
score = raw + mod * raw                        // Dave Mark's compensation factor
if goal == brain.current_goal { score += 0.10 } // hysteresis
score += goal.flat_bonus                        // e.g. Fight's mood bonus
```

Highest score wins; ties break by table order. A goal whose required components are missing is skipped before scoring. If the winner differs from `current_goal`, the planner replans for the new goal state.

### Goal table

| Goal | Requires | Considerations (input → curve) | GOAP goal state |
| --- | --- | --- | --- |
| Eat | Needs, Inventory, Wallet | `U(hunger)` → Quadratic{2,1,0,0}; `Can(food>0 ∨ coins≥price ∨ home pantry>0 ∨ lawfulness<0.3)` → Step{1,0,1} | `{hunger_satisfied: true}` |
| Sleep | Needs (Street if homeless) | `U(energy)` → Logistic{10,0.`75}; Phase: Night → 1, Evening → 0.7, else 0.4` | `{energy_satisfied: true}` |
| Work | Job, Needs, no Sentence | `Can(in shift now)` → Step{1,0,1}; `energy` → Linear{1,0}; `U(wealth)` → Logistic{8,0.5}; `Can(days_unpaid<7)` → Step{1,0,1} | `{shift_done: true, has_wage_due: false} (plan: GoTo(workplace) → work action → GoTo(Hall) → CollectWage)` |
| Earn | Needs, Wallet, Skills | `U(wealth)` → Quadratic{2,1,0,0}; `U(hunger)` → Linear{1,0}; `Can(not in shift)` → Step{1,0.3,1}; `greed` → Linear{0.5,0.5} | `{has_savings: true} (coins ≥ 7 × price; produced by CollectWage, CollectDole, SellFood, Fence, SplitLoot, Beg)` |
| Socialise | Needs, Personality | `U(belonging)` → Logistic{6,0.5}; `(mood+1)/2` → Linear{1,0} (×1.2 if mood<−0.3); `sociability` → Linear{1,0}; `Phase(Evening)` → Step{1,0.4,1} | `{belonging_satisfied: true}` |
| Court | Needs, age ≥ 18 y, no Spouse edge | `U(intimacy)` → Quadratic{2,1,0,0}; `Can(known candidate with affinity≥0.3)` → Step{1,0,1}; `courage` → Linear{0.6,0.4}; `Phase(Evening)` → Step{1,0.5,1} | `{has_spouse: true}` |
| Flee | Needs | `U(safety)` → Logistic{12,0.5}; `Can(hostile within 6 tiles)` → Step{1,0,1}; `1−courage` → Linear{0.8,0.2} | `{is_safe: true}` |
| Fight | Needs, Skills | `Can(enemy/rival within 4 tiles ∨ being robbed)` → Step{1,0,1}; `courage` → Linear{1,0}; `fighting` → Linear{0.5,0.5}; `U(safety)` → Linear{0.5,0.5}; flat +0.15 if `mood<−0.6` | `{threat_removed: true}` |
| ReportCrime | unreported `SawCrime` memory ≤ 2 days old; not GangMember unless Brain.betraying is set (see Gang › Betrayal) | `lawfulness` → Logistic{8,0.5}; `memory.salience` → Linear{1,0}; `Can(Hall open: Morning/Work/Evening)` → Step{1,0,1}; `1−affinity(suspect)` → Linear{1,0} | `{crime_reported: true}` |
| Patrol | Job{Guard}, no Sentence | `Can(in shift)` → Step{1,0,1}; `energy` → Linear{0.8,0.2}; `Can(no open warrant)` → Step{1,0.3,1} | `{patrol_leg_done: true}` |
| Arrest | Job{Guard} | `Can(open CrimeReport with suspect located)` → Step{1,0,1}; `courage` → Linear{0.5,0.5}; `Can(in shift)` → Step{1,0.5,1} | `{suspect_jailed: true}` |
| JoinGang | not GangMember, not Guard | `1−lawfulness` → Quadratic{2,1,0,0}; `U(wealth)` → Linear{1,0}; `Can(eligible per Gang › Eligibility, which is authoritative)` → Step{1,0,1}; `courage` → Linear{0.5,0.5} | `{in_gang: true}` |
| GangWork | GangMember | `U(wealth)` → Linear{0.7,0.3}; `greed` → Linear{0.7,0.3}; `Can(not in shift)` → Step{1,0.5,1}; `Can(no guard within 8 tiles of target)` → Step{1,0.2,1} | `{gang_task_done: true}` |
| Bury | `SawCorpse` memory for an unburied corpse | `Can(corpse unburied)` → Step{1,0,1}; kin → 1, Gravedigger → 0.9, else 0.25 → Linear{1,0}; `lawfulness` → Linear{0.5,0.5}; `U(safety)` → Linear{−0.5,1} | `{corpse_buried: true}` |
| Idle | — | constant → Step{0,0.05,0.05} | `bypasses the planner: installs [Rest] if at Home or Bar, else [Wander]` |

Agents with a `Sentence` skip utility and planning entirely: the law system feeds them (+0.5 hunger per day, one unit taken from Market stock and paid for by the Treasury at price\_food) and sets energy to 1.0 nightly. Corpses have no Brain.

### Worked example

A Farmer in the Work phase, in shift, with `hunger = 0.15`, `energy = 0.6`, `belonging = 0.5`, `coins = 4`, `price_food = 3`, `food = 0`, `greed = 0.5`, `sociability = 0.5`, `mood = −0.3`, `lawfulness = 0.6`, `days_unpaid = 0`, `current_goal = Work`.

`wealth = clamp(4 / (7 × 3 × 1.5), 0, 1) = 0.127`, so `U(wealth) = 0.873` and `U(hunger) = 0.85`.

| Goal | Considerations | raw | n | mod | score |
| --- | --- | --- | --- | --- | --- |
| Eat | 0.85² = 0.7225; can afford (4 ≥ 3) → 1 | 0.7225 | 2 | 0.2775 × 0.5 = 0.139 | 0.7225 + 0.139 × 0.7225 = **0.823** |
| Work | in shift 1; energy 0.6; Logistic(0.873) = 0.952; paid 1 | 0.571 | 4 | 0.429 × 0.75 = 0.322 | 0.571 + 0.322 × 0.571 = 0.755, +0.10 hysteresis = **0.855** |
| Earn | 0.873² = 0.762; hunger 0.85; in shift → 0.3; greed 0.75 | 0.146 | 4 | 0.854 × 0.75 = 0.641 | 0.146 + 0.641 × 0.146 = **0.240** |
| Socialise | Logistic(0.5) = 0.5; (−0.3+1)/2 = 0.35, ×1.2 = 0.42; sociability 0.5; not Evening 0.4 | 0.042 | 4 | 0.958 × 0.75 = 0.719 | 0.042 + 0.719 × 0.042 = **0.072** |

Work (0.855) narrowly beats Eat (0.823) because of hysteresis, so the agent finishes the hour. Thirty ticks later hunger is \~0.14 and Eat is 0.838; at hunger 0.10 Eat reaches 0.886 and wins, and the planner resolves it as `GoTo(Market) → BuyFood(1) → EatFromInventory`. With `coins = 2` the affordability Step returns 0 (unless `lawfulness < 0.3`, which enables `StealFood`), Eat is unavailable, and Work continues until `CollectWage` at shift end.

## GOAP planner

Forward A\* over a small `Copy` world-state struct, with \~35 actions whose costs are scaled by personality, a 200-expansion cap and a 12-plans-per-tick budget.

### WorldState

`WorldState` is a symbolic snapshot derived from components by `WorldState::observe(world, agent, plan_target)` at plan time and again at each step boundary during execution.

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum LocationKey {
    #[default] Anywhere,  // only in goal states / preconditions, never observed
    Home, Farm, Market, Bar, Jail, Cemetery, Hall, Hideout, Warehouse,
    Street,         // on a non-building tile
    TargetHome,     // the Home bound to Plan.target (StealFood(Home), Extort)
    SuspectTile,    // last known tile of Plan.target suspect
    CorpseTile,     // tile of Plan.target corpse
    PatrolWaypoint, // next patrol leg
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct WorldState {
    pub at: LocationKey,
    pub hunger_satisfied: bool,       // needs.hunger >= 0.6
    pub energy_satisfied: bool,       // needs.energy >= 0.6
    pub belonging_satisfied: bool,    // needs.belonging >= 0.5
    pub has_food: bool,               // inventory.food >= 1
    pub has_coins: bool,              // wallet.coins >= market.price_food
    pub has_savings: bool,            // wallet.coins >= 7 * market.price_food
    pub coin_bucket: u8,              // 0: < price, 1: < 2*price, 2: more; planner effects update this
    pub food_count: u8,               // inventory.food saturating at 3; planner effects update this
    pub has_wage_due: bool,           // job.days_unpaid >= 1
    pub shift_done: bool,             // today's shift completed
    pub has_spouse: bool,
    pub has_partner_candidate: bool,  // an edge with affinity >= 0.6 && trust >= 0.5, both unmarried
    pub is_safe: bool,                // needs.safety >= 0.4
    pub threat_removed: bool,         // no hostile within 4 tiles
    pub crime_reported: bool,         // no unreported first-hand SawCrime with salience >= 0.5
    pub suspect_jailed: bool,         // Plan.target has Sentence
    pub suspect_cuffed: bool,         // intermediate between Arrest and Escort
    pub in_gang: bool,
    pub gang_task_done: bool,
    pub corpse_buried: bool,
    pub carrying_corpse: bool,
    pub carrying_stolen: bool,        // inventory.stolen_food >= 1
    pub is_dark: bool,
    pub known_corpse: bool,           // a SawCorpse memory whose subject is still an unburied Corpse
    pub known_suspect_location: bool, // open CrimeReport whose suspect was seen in the last 240 ticks
    pub patrol_leg_done: bool,
    pub food_source_available: bool,  // Market stock > reserved, or own pantry > 0
    pub forage_available: bool,       // Summer or Autumn, and not dark
}
```

`at` is symbolic. The planner never reasons about tiles: `at: Market` means inside or at the door of the Market; `at: TargetHome` means at the building bound to `Plan.target`. Only `GoTo` changes `at`, and only the execution layer turns it into a tile path via `world.resolve_location(agent, key, plan.target)`. This keeps the search space to a handful of locations and makes plans identical for every citizen regardless of where their Home is.

Goal states are partial: `GoalState = Vec<(Key, Value)>` with at most 3 entries; `satisfied(ws, goal)` compares only listed keys. `Inventory` gains a `stolen_food: u32` sub-count so `carrying_stolen` can be observed.

### ActionKind

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ActionKind {
    GoTo(LocationKey), EatFromInventory, EatAtHome, BuyFood, StealFood(StealSource),
    Forage, Beg, Sleep, Rest, FarmWork, HaulToMarket, ClerkWork, BartendWork,
    CollectWage, SellFood, Fence, Chat, Drink, Flirt, Propose, ReportCrime,
    PatrolLeg, Arrest, Escort, HideFromLaw, FleeToHome, Attack, JoinGang,
    Extort, SplitLoot, BuryCorpse, CarryCorpse, Wander,
    CollectDole,   // at Hall, unemployed adult, once per day: has_coins, has_savings if coins >= 7*price; dur 5, cost 2
    GuardJail,     // Guard, at Jail: shift_done, has_wage_due; runs to shift end
    ServeTime,     // the forced plan of a sentenced agent; never chosen by the planner
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum StealSource { Market, Home, Warehouse }
```

Enum order is the tie-break order (lower variant wins on equal f-cost).

### Action table

Abbreviations: `L` lawfulness, `G` greed, `C` courage, `S` sociability, `st` stealth, `fi` fighting, `guard8` = a guard within 8 tiles, `starving` = `hunger < 0.15`. Memories are `(kind, salience, valence)`; W = witnesses who pass the notice roll. Work actions run to the end of the shift (up to 540 ticks) and are interruptible; farm yield accrues per 60 ticks worked, so a partial shift still produces. This Action table is authoritative for preconditions, effects, costs and durations wherever the Building table differs.

| Action | Preconditions | Effects | Base | Cost modifiers | Dur | Who | Memories | Crime |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GoTo(k) | at ≠ k | at = k | 2 | + door-to-door manhattan / 16; +3 if dark and k = Street | by path | all | – | – |
| EatFromInventory | has\_food | hunger\_satisfied; has\_food = (food ≥ 2) | 1 | – | 10 | all | actor Ate 0.2 +0.3 | – |
| EatAtHome | at Home, pantry > 0 | hunger\_satisfied | 2 | +4 if pantry ≤ 2 | 15 | residents | actor Ate 0.2 +0.3 | – |
| BuyFood | at Market, has\_coins, food\_source\_available | has\_food; has\_coins = coins ≥ 2 × price after purchase | 3 | + G × 2 | 10 | all | actor Paid 0.1 0 | – |
| StealFood(Market) | at Market, not carrying\_stolen | has\_food, carrying\_stolen | 8 | + L × 20; +6 if guard8; −4 if starving; −3 if dark; − st × 4 | 20 | all | W SawCrime 0.5 −0.4; clerk on duty WasRobbed 0.6 −0.6 | Theft |
| StealFood(Home) | at TargetHome, pantry > 0, not carrying\_stolen | has\_food, carrying\_stolen | 7 | as Market; +5 if any occupant present | 20 | all | W SawCrime 0.5 −0.4; residents WasRobbed 0.6 −0.6 | Theft |
| StealFood(Warehouse) | at Warehouse, stock > 0, stealth ≥ 0.4 | has\_food, carrying\_stolen | 9 | as Market; +4 | 25 | all | as Market | Theft |
| Forage | at Farm, forage\_available | has\_food | 6 | +2 if hunger < 0.3 | 45 | all | – | – |
| Beg | at Market or Bar | has\_coins (probabilistic; step fails if nothing gained) | 5 | + pride × 10 + (1 − S) × 4 | 30 | all but Guard | actor Rejected 0.3 −0.3 on failure | – |
| Sleep | at Home (Street if homeless) | energy\_satisfied | 1 | +6 if at Street | until energy ≥ 0.9, cap 480 | all | – | – |
| Rest | at Bar or Home | energy\_satisfied | 4 | – | 90 | all | – | – |
| FarmWork | at Farm, not shift\_done, phase Work | shift\_done, has\_wage\_due | 3 | − farming × 2 | to shift end, ≤ 300 | Farmer | – | – |
| HaulToMarket | at Farm, farm stock ≥ 10 | at = Market; farm −min(50, stock), market +same | 3 | – | 40 + path | Farmer | – | – |
| ClerkWork | at Market, not shift\_done, phase Work | shift\_done, has\_wage\_due | 3 | – | to shift end | Clerk | – | – |
| BartendWork | at Bar, not shift\_done, phase Work or Evening | shift\_done, has\_wage\_due | 3 | – | to shift end | Bartender | – | – |
| CollectWage | at Hall, has\_wage\_due | has\_coins (if paid ≥ price), has\_wage\_due = false | 2 | +4 if treasury < 0 | 5 | employed | actor Paid 0.3 +0.3, or Unpaid 0.5 −0.5 | – |
| SellFood | at Market, has\_food, not carrying\_stolen | has\_coins, has\_food = false | 3 | − G × 2 | 10 | all | actor Paid 0.1 +0.1 | – |
| Fence | at Hideout, carrying\_stolen | has\_coins, carrying\_stolen = false, has\_food = false | 3 | + L × 8 | 15 | gang members, or anyone with affinity > 0 to a member | – | – |
| Chat | at Market, Bar, Home or Farm; partner reservable | belonging\_satisfied | 3 | − S × 2 | 30 | all | both Socialised 0.2 +0.2 | – |
| Drink | at Bar, has\_coins | belonging\_satisfied; coins −2 | 4 | − S × 2 | 45 | all | actor Socialised 0.2 +0.3 | – |
| Flirt | at Bar or Market; partner reservable; not has\_spouse | apply() sets has\_partner\_candidate = true for planning; at execution the affinity roll may leave it false, so Propose then fails with PreconditionLost | 4 | + (1 − S) × 4 | 30 | unmarried adults | both Courted 0.4 +0.3, or actor Rejected 0.4 −0.3 | – |
| Propose | has\_partner\_candidate, co-located with candidate | has\_spouse (roll) | 5 | − C × 2 | 10 | unmarried adults | both Married 1.0 +0.9, or actor Rejected 0.6 −0.5 | – |
| ReportCrime | at Hall, not crime\_reported | crime\_reported | 2 | + (1 − L) × 10; +8 if in\_gang | 10 | all | – | – |
| PatrolLeg | at PatrolWaypoint | patrol\_leg\_done | 2 | – | 20 | Guard | – | – |
| Arrest | known\_suspect\_location, at SuspectTile, adjacent | suspect\_cuffed (contested if suspect courage > 0.7) | 4 | + (1 − C) × 6 | 10 | Guard | suspect WasArrested 0.7 −0.5; both Fought on contest | – |
| Escort | suspect\_cuffed | at = Jail, suspect\_jailed | 3 | – | path | Guard | – | – |
| HideFromLaw | at Hideout or Home; open warrant on self | is\_safe | 4 | − (1 − L) × 2 | 120 | all | – | – |
| FleeToHome | not is\_safe | at = Home, is\_safe | 2 | − (1 − C) × 3 | path | all | – | – |
| Attack | hostile within 4 or Plan.target set | threat\_removed (roll) | 10 | + (1 − C) × 15 + L × 10 − fi × 6; −5 if WasRobbed by target | 15 | all | both Fought 0.7 −0.5; winner Won 0.6 +0.4; loser Lost 0.6 −0.6; W SawCrime 0.8 −0.6 | Assault; Murder if the loser dies |
| JoinGang | at Hideout, not in\_gang, eligible (see Social graph) | in\_gang | 6 | + L × 15 − (1 − hunger) × 4 | 30 | adults, not Guard | actor Socialised 0.4 +0.2 | – |
| Extort | in\_gang, at TargetHome, not guard8 | gang\_task\_done; actor coins +5 | 6 | + L × 12; +4 if an occupant has courage > 0.6 | 20 | GangMember | victims WasRobbed 0.7 −0.7; W SawCrime 0.6 −0.5 | Extortion |
| SplitLoot | in\_gang, at Hideout, gang\_task\_done | has\_coins | 2 | – | 10 | GangMember | actor Paid 0.2 +0.2 | – |
| BuryCorpse | at Cemetery, carrying\_corpse | corpse\_buried, carrying\_corpse = false | 2 | – | 60 | Gravedigger; any adult if none alive | actor Grief 0.3 −0.2 | – |
| CarryCorpse | at CorpseTile, known\_corpse | carrying\_corpse | 3 | – | 20 | as above | actor SawCorpse 0.5 −0.4 | – |
| Wander | – | at = Street | 1 | – | 30 | all | – | – |

Beg detail: at completion, take up to 4 passers-by within 3 tiles (lowest `EntityId` first); each with `edge.affinity > 0` and `coins > 10` rolls `rng < 0.3 + sociability × 0.4`; the first success gives `1 + (rng < 0.5) as i64` coins and records a debt on the edge.

`fn cost(kind, world, agent, ws) -> f32` is clamped to `[0.5, 60.0]`.

### Planner

Forward A\* over `WorldState`. The state is \~25 keys and `Copy`, forward search evaluates real costs against the agent's current context, and it avoids the unbound-variable problem of backward GOAP.

```rust
pub struct Plan { pub goal: GoalKind, pub target: Option<EntityId>, pub steps: Vec<ActionInstance>, pub started_tick: Tick }

pub const PLANNER_MAX_EXPANSIONS: usize = 200;
pub const PLAN_MAX_LEN: usize = 6;
pub const PLANNER_BUDGET_PER_TICK: usize = 12;
pub const PLAN_TIMEOUT_TICKS: Tick = 900;
pub const PLANNER_EXPANSION_BUDGET_PER_TICK: usize = 600;  // a search past the budget resumes next tick
pub const GOAL_COOLDOWN_TICKS: Tick = 120;

struct Node { f: f32, g: f32, state: WorldState, depth: u8, parent: Option<usize>, via: Option<ActionKind> }
```

1. `open: BinaryHeap<Reverse<(OrderedFloat(f), action_order, node_idx)>>`, `closed: BTreeSet<WorldState>`.
2. Pop the lowest `f`. If `satisfied(state, goal)`, reconstruct and return. Skip if `depth == PLAN_MAX_LEN`. Return `Err(PlanFailed)` past 200 expansions.
3. For each `ActionKind` in enum order whose preconditions hold and whose role gate passes: `next = apply(state, kind)`, `g' = g + cost(kind)`, `h = 3.0 × (number of goal keys unsatisfied in next)`; push if not closed.
4. The heuristic is slightly inadmissible (min action cost is 0.5) and that is accepted for speed.

Budget: agents whose goal changed or whose plan is empty push `(urgency = winning utility score, EntityId)` into `world.plan_queue: BTreeMap<(Reverse<OrderedFloat<f32>>, EntityId), ()>`. The planner pops at most 12 per tick; the rest stay queued and the agent stands still (needs still decay). Entries older than 90 ticks are dropped and the agent idles until its next think.

Replan triggers: (a) utility picks a different `GoalKind`; (b) at step start, `observe()` shows a precondition of the next step false; (c) a step returns `StepResult::Failed`; (d) `tick − plan.started_tick > 600`. On (b) or (c) the same goal is replanned once; a second failure within 60 ticks cools the goal: `Brain.cooldowns: BTreeMap<GoalKind, Tick>` with `until = tick + 120`, and utility multiplies a cooled goal's score by 0.

**Target binding.** `Plan.target` is bound once, at plan time, by goal: `Eat` binds the nearest other Home with pantry > 0 (for `StealFood(Home)`); `Socialise` and `Court` bind the co-located agent with the highest affinity who has no `Partner` reservation (if none, Chat/Flirt preconditions are false); `GangWork` binds the Extort target per the Gang section; `Arrest` binds the open warrant's suspect; `Bury` binds the nearest known unburied corpse; `Fight` binds the hostile or the revenge subject. `apply()` updates `coin_bucket` and `food_count` so that effects such as `has_food = (food ≥ 2)` and `has_coins` after BuyFood are computable from `WorldState` alone.

### Worked trace 1: broke, hungry, lawfulness 0.3

Home resident, coins 0, inventory 0, pantry 0, hunger 0.1 (starving), at Street, Market stock 400, no guard within 8, stealth 0.2, pride 0.5, sociability 0.5, daytime, Spring. Goal `Eat → {hunger_satisfied}`; `h` = 3 per unsatisfied key.

| # | Popped state | g | h | f | Expansions pushed |
| --- | --- | --- | --- | --- | --- |
| 0 | at Street, no food, no coins | 0 | 3 | 3 | GoTo(Home) g 2.5, GoTo(Market) g 2.9, GoTo(Bar) g 3.0, GoTo(Farm) g 3.1, GoTo(Hall) g 3.4, Wander g 1 |
| 1 | Wander → at Street | 1 | 3 | 4 | same GoTos at higher g; closed |
| 2 | at Home | 2.5 | 3 | 5.5 | EatAtHome blocked (pantry 0); GoTo(Market) g 5.3 |
| 3 | at Market | 2.9 | 3 | 5.9 | BuyFood blocked (no coins); StealFood(Market) cost 8 + 0.3×20 − 4 − 0.8 = 9.2 → g 12.1; Beg cost 5 + 5 + 2 = 12 → g 14.9 |
| 4 | at Bar | 3.0 | 3 | 6.0 | Beg g 15.0 |
| 5 | at Farm | 3.1 | 3 | 6.1 | Forage blocked (Spring) |
| 6 | at Market, has\_food, carrying\_stolen | 12.1 | 3 | 15.1 | EatFromInventory g 13.1, h 0 |
| 7 | hunger\_satisfied | 13.1 | 0 | 13.1 | goal satisfied |

Plan (the table shows the nodes on the winning path; A\* also pops the cheaper GoTo permutations first, about 20 nodes in all): `GoTo(Market) [2.9] → StealFood(Market) [9.2] → EatFromInventory [1.0]`, total 13.1. The Beg branch (f 17.9) is never popped. At execution StealFood raises `Theft`, witnesses roll, and `carrying_stolen` stays true after eating if 2+ units were taken.

### Worked trace 2: lawfulness 0.9

Same scenario, lawfulness 0.9, pride 0.4, sociability 0.6, Autumn.

| # | Popped | g | f | Note |
| --- | --- | --- | --- | --- |
| 0 | Street | 0 | 3 | push GoTos |
| 1–2 | Wander, Home |  |  | nothing useful |
| 3 | Market | 2.9 | 5.9 | StealFood 8 + 18 − 4 − 0.8 = 21.2 → g 24.1; Beg 5 + 4 + 1.6 = 10.6 → g 13.5 |
| 4 | Bar | 3.0 | 6.0 | Beg g 13.6 |
| 5 | Farm | 3.1 | 6.1 | Forage 6 + 2 = 8 → g 11.1, f 14.1 |
| 6 | Farm, has\_food | 11.1 | 14.1 | EatFromInventory → g 12.1 |
| 7 | hunger\_satisfied | 12.1 | 12.1 | goal |

Plan: `GoTo(Farm) [3.1] → Forage [8.0] → EatFromInventory [1.0]` = 12.1. Beg → BuyFood → Eat would have cost at least 17.5 and Steal 25.1.

Winter variant (no forage), nobody generous at the Market: the plan is `GoTo(Market) → Beg → BuyFood → EatFromInventory`. Beg completes with no coins → `Failed`; the replan is identical and fails again inside 60 ticks → `Eat` cools for 120 ticks and utility picks the next goal (usually `Earn` via Work and CollectWage). When the cooldown ends, starving has lowered StealFood to 21.2 against Beg's 10.6, so this agent will keep begging and may starve rather than steal, which is the intended personality-driven outcome.

## Execution layer

Three executor states as in F.E.A.R.: Goto, Use, Wait. Effects apply at completion, money at start, and shared resources are reserved at plan time.

```rust
pub enum ExecState {
    Idle,
    Goto { path: Vec<TilePos>, next_move_tick: Tick, dest: LocationKey },
    GotoTimed { arrive_tick: Tick, dest: LocationKey },        // Coarse LOD
    Use { kind: ActionKind, until: Tick, started: Tick },
    Wait { until: Tick },
}
pub enum StepResult { Running, Done, Failed(FailReason) }
pub enum FailReason { NoSuchPlace, PreconditionLost, StockGone, PartnerLeft, Timeout }

pub const MOVE_TICKS_FULL: Tick = 2;        // 1 tile per 2 ticks
pub const INTERRUPT_CHECK_TICKS: Tick = 30;
pub const DOOR_CAPACITY_PER_TICK: u8 = 2;
pub const DOOR_QUEUE_MAX_TICKS: Tick = 30;
pub const RESERVATION_TTL: Tick = 120;
```

**Goto.** `GoTo(key)` resolves to a door tile via `world.resolve_location(agent, key, plan.target) -> Option<TilePos>` (`None` → `Failed(NoSuchPlace)`). Pathing is grid A\* on 4-connected walkable tiles (Ground, Road, Door, Farmland; Wall and Water blocked) with step costs Ground 1.0, Road 0.7, Door 1.0, Farmland 1.2 and a Manhattan heuristic × 0.7. For the common case, `world.flow_fields: BTreeMap<EntityId, FlowField>` holds one direction field per building door, built by Dijkstra from the door at world creation and invalidated only by `BuildHome`/`DemolishHome`; an agent heading to a building follows the field in O(1) per step. A\* is used only for non-building destinations (SuspectTile, CorpseTile, wander, map edge), capped at 4,000 node expansions; on cap the agent waits 10 ticks and retries. The agent advances one tile every `MOVE_TICKS_FULL` ticks. Stepping onto a Door tile consumes one of that door's `DOOR_CAPACITY_PER_TICK` slots in `world.door_queue: BTreeMap<TilePos, u8>` (reset each tick); if none is free the agent waits in place, and after `DOOR_QUEUE_MAX_TICKS` the step fails. Entering sets `Position.building = Some(id)` and pushes to `Building.occupants`; leaving pops it. Goto completes when the agent is at `dest` or inside the building.

**Use.** On entry: re-run `observe()`; if a precondition is false → `Failed(PreconditionLost)`. Money transfers (BuyFood pays, Drink pays, Extort takes, Fence and SellFood pay out, CollectWage) happen at the start so an interruption cannot duplicate them. All other effects (food into inventory, needs, memories, crime reports, stock decrements) apply at `until`. At completion the action re-checks its resource: BuyFood, StealFood, EatAtHome, Forage and HaulToMarket fail with `Failed(StockGone)` if the source is empty (BuyFood's payment is refunded). Chat, Flirt and Propose fail with `PartnerLeft` if the partner left the building. Work actions run until the shift end (max 300 ticks) and apply their yield per 60 ticks elapsed.

**Wait.** Used by Sleep-until-energy, door queueing, and agents whose plan request is still in `plan_queue`. Needs decay continues.

**Interruption.** Every 30 ticks (`tick % 30 == index % 30`) a Full or Coarse agent re-runs utility. If the winner differs from `current_goal` and the current step is not uninterruptible, the step is aborted: no completion effects, start-of-action money is not refunded (except BuyFood), reservations are released, and the new goal is queued. Uninterruptible steps: `Sleep` once ≥ 60 ticks have elapsed, `Arrest`, `Escort`, `ServeTime` (the pseudo-action a sentenced agent is locked into), `BuryCorpse`. An agent with a `Sentence` has its plan forcibly replaced by `[ServeTime]` until release.

**Coarse LOD.** `GoTo` becomes `GotoTimed { arrive_tick: tick + manhattan(door_from, door_to) × 2 }`; on arrival `Position.tile = door_to` and `Position.building` is set. `Use` resolves at `tick + duration` with full effects. Witness rolls still run for Coarse agents by building co-location. Statistical agents never execute; systems act on them directly.

**Reservations.**

```rust
pub enum ReservationKind {
    FoodUnits { building: EntityId, units: u32 },
    Bed { home: EntityId },
    Partner { other: EntityId },
    Corpse { corpse: EntityId },
    Suspect { suspect: EntityId },
}
pub struct Reservation { pub kind: ReservationKind, pub expires: Tick }
// World.reservations: BTreeMap<EntityId /* holder */, Vec<Reservation>>
```

When a plan is returned, each step that consumes a shared resource calls `reserve(world, holder, kind, tick + RESERVATION_TTL)`. `observe()` reports `food_source_available` as `stock − reserved_units > 0`, partner reservability as the absence of a `Partner` reservation on that entity, and likewise for corpses and suspects. A reservation is released on step completion, step failure, plan abort, or expiry (swept once per tick). Reserved units leave stock only at action completion; because planning is sequential within a tick, two reservations cannot exceed stock in the same tick, and a later completion that finds no stock fails with `StockGone`.

## World systems

Five systems tick for every citizen at every LOD. `World::tick` runs them in the fixed order `commands, time, lod, needs, memory (daily decay), think, plan, exec, economy, law, social, gang, demography, stats`.

### Ownership, rent, corps and classes (M11)

Since M11 the tick order is `commands, lod, needs, memory, mood, think, plan, exec, ownership, classes, economy, [bind], law, social, gang, corp_brain, demography, stats` (`ownership::run` and `classes::run` act at midnight, before the price step; `corp_brain::run` checks shocks every tick and rescores at midnight). Every building has an owner: `None` is the city (the Treasury), else an agent (their wallet), a gang or a corp (its treasury). The money model, as built:

| Flow | From | To |
| --- | --- | --- |
| Food (`BuyFood`), Drink | buyer | the Market's or Bar's owner (taxed) |
| Wages | the employer's owner, collected at the workplace (`LocationKey::Workplace`; the Hall for a city job) | worker (taxed) |
| Rent | each adult's share of their Block's `rent_per_day`, at midnight | the Block's owner (taxed) |
| Upkeep | every non-city owner, per building | the Treasury |
| Wholesale | a Market's owner for a Farm's food (different owners); the City for Farm overflow; a Market's owner for restock | the Farm's owner; the Farm's owner; the Treasury |
| Security contracts | a client building's owner, daily | the Security corp |
| Exec wage | a corp | its exec |
| Found, Sale, Subsidy, Bribe, SellFood, Precinct meals | as named | as named |
| Dole | the Treasury | an unemployed adult |

The rules are in [M11_OWNERSHIP.md](M11_OWNERSHIP.md): ownership and purses § 3, rent and eviction § 4, corps and the corp brain § 5, founding and incorporation § 6, classes § 7, the levers § 8, and what the build changed under "Implemented: deviations". The Economy text below is the v1 city, where the Treasury owns everything; `Config::v1_profile()` still runs it that way (no rent, no corps).

### Districts, the street and riots (M12)

Since M12 the tick order is `commands, lod, needs, memory, mood, think, plan, exec, ownership, classes, districts, economy, [bind], law, social, gang, corp_brain, demography, stats`. The v2 map's five zones are cut into eight districts on road lines (`[districts]`: Spire, Civic, Vats, Mid West, Mid East, Sump West, Sump Central, Sump East); `World::district_of(tile)` is one byte read from a per-tile grid whose bit 7 marks a street tile (walkable, inside no building), the litter and sweep domain. `districts::run` does the daily pass at midnight (crime counters, the street's day, litter decay, the sweepers and owners cleaning, aggregates, control, unrest, the riot trigger, district strikes) and the street's nightly pass at 03:00 (Hotel bookings, squats, Vagrancy). Litter is a byte per tile in bands: 0-31 clean, 32-95 littered, 96-191 trashed, 192-254 heaped, 255 rubble (the Damage hook, off); a district's litter is the share of its street tiles at 32 or above. The captain deals the patrol guards to districts and sets a stance per district beside the M9 Jail posture.

The rules are in [M12_DISTRICTS.md](M12_DISTRICTS.md): districts § 1, the law in districts § 2, litter § 3, the street § 4, gangs § 5, unrest, riots, crossfire and strikes § 6, the levers § 7, UI, events and CSV § 8, and what the build changed under § 14 and "Implemented: deviations". `Config::v1_profile()` turns districts into one and litter, the street and riots off.

### Assets (M13)

Since M13 the tick order is `commands, lod, needs, memory, mood, think, plan, exec, ownership, assets, classes, districts, economy, [bind], law, social, gang, corp_brain, demography, stats`. `assets::run` does nothing per tick except end episodes on the hour; at midnight it runs upkeep, finance, repossession and impound, wear, repairs (vehicles at a Garage, robots at a Security Office), Garage rent, sanity and episodes, the corpse window and scav strips, the parts market, abandoned-vehicle recovery and the fleet recall, off-screen vehicle theft and abduction, the Statistical addict pass and shop, and appearance.

An asset is an entity carrying only `Asset` (kind, tier, owner, loc, condition, value, upkeep, finance, flags, keeper, list price), so no agent scan sees it. Its place is an `AssetLoc`: `Parked(building)` (at the door; Homes, workplaces, Garages, Hideouts), `InUse(agent)` (driven), `Carried(agent)` (a pack or bridge), `Installed(body)` (chrome, in a living agent or a corpse), `Posted(building)` (a robot), `Stock(building)` (for sale at a Clinic, Garage or Security Office, or a gang's take at its Hideout), `Limbo(hole)` (taken by an unbound Abducted hole). Only `assets::set_loc` and `set_owner` write them, keeping the indices (`assets_at`, `assets_by_owner`, `vehicles`, `limbo`, `loot_corpses`) and every touched agent's derived `Kit` in step.

**Step quarters.** A Full mover's step durations are counted in quarter ticks: `ExecState::Goto` carries `carry_q`, and each executor call takes steps while `next_move_tick × 4 + carry_q` is below the next tick's quarter, at most `[vehicles] max_steps_per_tick`, stopping at the door. A step costs `step_q = max(1, round(4 × move_ticks_full × mult))` quarters, `mult` the driven road vehicle's entry for the tile entered (road, ground, farmland) or the walker's `Kit.walk_mult` (Legs chrome), plus M12's litter delay. An unchromed walker costs 8 quarters a step: one step every two ticks, the v1 arithmetic exactly. Coarse movers take a timed hop scaled by `Kit.timed_mult`; the flyer is a timed Chebyshev hop over walls at every tier (`ExecState::Fly`). Paths and flow fields stay pedestrian.

The rules are in [M13_ASSETS.md](M13_ASSETS.md): the asset model § 1, vehicles § 2, chrome § 3, stims § 4, the robot § 5, the Shop § 6, the Statistical tier § 7, the levers § 8, UI, events and CSV § 9, and what the build changed under "Implemented: deviations". `Config::v1_profile()` and the calibration city turn `[assets]` off: no pass, goal, order, Kit term or Body read.

### Data and Virt (M14)

Since M14 the tick order is `commands, lod, needs, memory, mood, think, plan, exec, virt, ownership, assets, tech, classes, districts, economy, [bind], law, social, gang, corp_brain, demography, stats`. `virt::run` relinks the plane when a hook marked it dirty and pops the run steps that are due; nothing else runs per tick. `tech::run` is the plane's midnight pass: relink, Lab production from the shift ledger, ICE upkeep, research upkeep and decay, research under the Research order, gang Data sales, the Statistical hack pass, the expiries (alarms, hacks, run orders, sightings, turned robots) and the 30-day ICE-spend roll.

The Virt plane is a board game laid over the map, not a model of anything technical: `World::virt` holds abstract nodes (a Public node per district, a node per faction building, Lab and Hideout, the Precinct, a Ledger per living corp and the Treasury) and links with a tier (Street and Access 1, Trunk 2). A run is an event chain in `World::run_queue`, keyed by its id so its dice never touch the world stream:

| Step | Does | Contest |
| --- | --- | --- |
| order | a freelancer's Hack goal, a gang or corp order, the Raid prelude, the Statistical pass or god writes a `RunOrder`; the runner plans `GoTo(Chair) → JackIn` | — |
| `JackIn` | at the chair (home, Hideout, its Lab, or a Bar or Hotel terminal for a fee): route from the chair's portal node over links of tier ≤ the deck's, cheapest by `Σ −ln p`; the body sits `JackedIn` (never Statistical) | — |
| hop | each node on the route, `hop_ticks[deck]` apart | every node owned by someone other than the patron with `def = ICE + alarm > 0` |
| break-in, act, extract | the target, contested twice around `act_ticks[purpose]` | the target, when guarded |
| out | the effect: Data stolen into the patron's store or the deck, a store wiped (a corp with no backup left drops the tier), coins from a Ledger (`Flow::Hack`), `DoorOpen` until the raid's window closes, a robot turned | — |
| loss | the run ends: fry (reflex saves), death by flatline at ICE 3 or the kill roll, a trace to the chair (a corp or city owner files a report and the law knows where the body sits; a gang takes a grudge), the payload back on an extract loss; the body is dazed | — |

The **tier contest** has a fourth user: M13's `security::contest` (a thief's tier against a lock, a robot sensor, a camera) is `contest_f(att, def, step)` over floats, `p = clamp(0.5 + contest_step × (att − def), 0.05, 0.95)`, and a run's contest is `att = deck tier + hack_w × Skills.hacking` against `def = ICE + alarm`, the same draw.

Labs make Data in three tracks (Chrome, Deck, Industry) from the shift ledger; corps hold a tech tree of tiers 1-3 per track that costs Data a day to hold, lapses after `decay_days` short days or at once on a wipe with no backup, and gates what their sellers sell; an asset's effect reads `min(tier, its maker's tier)`. ICE sits on `Building.security` (and a corp's Ledger, the Treasury's on the Hall), is bought from a Security corp or self-installed under Secure and on the node robbed last week, and lowered under Hunker.

The rules are in [M14_VIRT.md](M14_VIRT.md): the plane § 1, decks § 2, runs, ICE and the contest § 3, Data and Labs § 4, the tech tree § 5, goals, orders and the law § 6, watching live § 7, the Statistical tier § 8, levers § 9, UI, events and CSV § 10, and what the build changed under "Implemented: deviations". `Config::v1_profile()` and the calibration city turn every M14 section off (`--virt-off` does the same for a run): no relink, run, upkeep, production or tier cap.

### Word and blood (M15)

Since M15 the tick order is `commands, lod, needs, memory, mood, think, plan, exec, virt, ownership, assets, tech, classes, districts, economy, [bind], word, law, social, gang, corp_brain, demography, stats`: `word` runs after the binder (so a bound hole names its rumours before the pools decay) and before the law, gang and corp brains, which read heat, fear, vendettas and honour. `word::run` does `hunt::tick` every tick (the Hunts' validity, at most `max_hunts`) and, at midnight, the **daily word chain**:

| Step | Does |
| --- | --- |
| `gossip::decay_and_leak` | every district pool's reach × `pool_decay`, the strong entries leaked to adjacent districts, entries under `reach_min` dropped |
| `news::daily` | the Feeds' reach from yesterday's Reporter shifts, Spin (plants and buries), the stories (posted into the covered pools at hops 1 with a slant), the ads |
| `gossip::hear` | each Statistical adult hears one entry of its district's pool (`hear_p × (0.5 + sociability)`, picked by reach); bodies hear one story; the post-back of first-hand deeds |
| `gossip::kin` | a killing's or beating's kin are told (`kin_p` by hops), at any tier |
| `reputation::rebuild` | the four axes and `known_by` of every adult and faction from the held deed memories and the pools, the regard matrix, the kill-watch sample, each corp's honour history |
| `competence::daily` | knowledge from yesterday's Lab, Feed and Hall shifts, rust, every corp's and the Law's competence, `TalentLost` |
| `grudges::daily` | decay, settlement, `kill_chain` expiry, the vendettas opened and closed |
| `hunt::daily` | Hunts abandoned after `hunt_days`, the Statistical hunters' pass |
| expiry | old sightings, renamed anonymous rumours |

On screen a deed travels at Chat (`gossip::exchange` after `social::gossip`), at a Bar's Drink and at a Hunt's AskAround; every roll of the word is on a keyed word stream (`SimRng::word`), never the world or agent streams. A grudge forms when an adult learns a deed whose object is itself or someone close (`deed_sev × rel_w × conf ≥ grudge_min`). With `[gossip] enabled = false` (`--word-off`) nothing above runs.

**The fifth dice rule: the social move.** After the witness's notice roll, the fight (`law::resolve_fight`), the tier contest (`security::contest_f`, shared by M13's locks and M14's runs) and the Statistical hourly table, `moves::resolve` is the one roll behind every social move (Intimidate in a Shakedown, Persuade in a poach, Deceive in an AskAround answer, Charm):

`p = clamp(logistic(move_k × (bias[kind] + skill_a − resist_t + w_m × (might_a − might_t) + w_r × rep_gap + w_l × taste(target → actor))), p_min, p_max)`

where `skill_a` is the actor's social skill for the kind, `resist_t` the target's (Intimidate: courage + 0.3 × fighting; Persuade by the stake: 1 − greed for coins, loyalty and the tie to its exec for a job; Deceive: knowledge; Charm: 0.5 − affinity), `might` is fighting plus visible chrome plus allies within 8 tiles (capped), `rep_gap` reads dread (Intimidate), standing (Persuade) or honour (Deceive, Charm). One draw on the Move word stream keyed by the tick, the two agents and the kind, so the callers' order never changes an outcome. A failed Intimidate on a brave target backlashes (a Fight bonus for an hour, an Enemy edge); a failed Deceive zeroes trust and is told as Betrayed; a Persuade across too wide a standing gap is refused at the door.

The rules are in [M15_WORD_AND_BLOOD.md](M15_WORD_AND_BLOOD.md): deeds, rumours and sightings § 1, reputation § 2, grudges § 3, the Hunt § 4, social stats § 5, the social move § 6, the news and propaganda § 7, the psychological LOD table § 8, levers § 9, UI, events and CSV § 10, and what the build changed under "Implemented: deviations". `Config::v1_profile()` and the calibration city turn every M15 section off.

### The living city (Life pass L2)

Since L2 the tick order is `commands, lod, needs, memory, mood, think, plan, exec, virt, ownership, assets, tech, classes, districts, economy, [bind], word, law, social, gang, corp_brain, living, demography, stats`. `living::run` holds L2's own passes: at midnight the venues' prices and visit windows, `jobs::daily` (scrap into Recycler Parts, the Noodle Bars' restock from the nearest Market, the import tally), `jobs::top_up` (one vacancy a day at every Market and Bar below `[jobs] market_staff`/`bar_staff`), `budget::daily` (the Treasury band: public works and `upkeep_mult`), `outside::export_daily` (the World account, `[export]`), deferred venue seeding after a pre-L2 load, `leisure::spots` (the day's HangOut spots); every hour the bout at `bout_hour`; at 21:00 `leisure::stat_daily` (the Statistical evening: one rung per agent below `fun_satisfied`, paid as on screen); at 18:00 an off-screen leader's Collect and Call. Elsewhere: `needs::run`'s hourly branch decays `fun` for bodies (`run_statistical` for the Statistical tier, `law::held_hourly` for held prisoners); `gang::daily_economy` builds fronts and `gang::desist` lets ordinary members walk away; `law::run` settles held prisoners hourly (decay, a cell meal for a starving one, the starvation check) before `jail_upkeep`; `lod::run` rolls the ledger's actor side at midnight (`fviolence::daily_actor`), rebuilds the active sources at 01:00 and tallies body exposure after each hourly assignment; **the daily faction pass `fviolence::daily` runs at the top of `bind::run`'s midnight branch**, before the binder (its holes are bound the next midnight).

**The Statistical tier's second roll.** Beside the table's hourly outcome (the M10 victim and actor rolls), each Statistical adult (not jailed, emigrating or pinned, not already a victim since the last midnight) in a district a source touches (a gang's order over its held districts, the rival's for Contest, the door's for a raid, a god `FactionStrike`, each side of a vendetta where both hold Homes, a live riot, an episode) draws once a day on `WordNs::FViolence` keyed `(day, id.index)`, in the fixed order Killed, Assaulted, Robbed, Abducted (Harvest, kitted adults only), against `rate[k] = fv_mult × (Σ victims[k] + prior[k] × prior_weight) ÷ (Σ exposure ÷ 24 + prior_weight)` summed over the sources that may strike it: the victims and body-hours the on-screen tier recorded in that `(source, district, victim class)` cell over the last `rate_days`, shrunk toward the source's prior times the class's `class_mult`. At most one hit per agent, city-wide `day_cap` per kind; a hit opens a consequential hole naming the source and its faction, and the binder draws only among that faction's members (else Unknown). The sixth dice rule: like the brawl, the lock, the run and the move, one seeded draw over counters.

The rules are in [LIFE_L2.md](LIFE_L2.md) (§ 1 jobs and venues, § 2 fun and the street, § 3 faction violence, § 4 the LOD budget and the churn, § 5 levers, § 6 the year run) and what the build changed under "Implemented: deviations". `Config::v1_profile()` and the calibration city call `living_off()`; `--l2-off` (`[living] enabled = false`) reproduces the M15 city.

### The contract board (M16a)

Since M16a the tick order is `commands, lod, needs, memory, mood, think, plan, exec, virt, ownership, assets, tech, classes, districts, economy, [bind], word, law, social, gang, corp_brain, living, contracts, demography, stats`. A contract is a record in `World::contracts` (buyer, placing agent, kind `Hit | Beat | Guard | Locate`, target, price, escrow, broker, deadline, origin, taker and crew), matched by a utility score and resolved by one seeded roll; every coin on a brokered record sits in escrow, counted inside `ownership::total_coins`, and moves only through `escrow_in`/`escrow_out` (the probe `escrow_leak`: Σ `Contract.escrow` − `World::escrow_held`, 0 every day). `contracts::run` does, every tick, the ledger draws that fall due (`ledger_due`) and the live runs' validity (a taker jailed or gone fails the attempt), then `missions::check` (the crews' march windows); every hour it starts queued live records under `[missions] max_missions`; at `[fixers] match_hour` (18:00) `match_day` offers each open Fixer's book to its regulars and gangs (by price and age, `offers_per_day`) and every direct record to its buyer's edges, gang and staff, then the Fixers' run orders; at midnight `contracts::daily` (deferred Fixer seeding, the median wage, buyers gone, deadlines, Guard posts kept, regulars pruned and the Statistical regulars stamped, the Hunt's hire pass, Fixer heat with the bust and the bribe, guard corruption, the vendettas' Locates, the Fixers' income ring, closed records pruned after `closed_keep_days`). The brains post through the same `contracts::post`: the Hunt's hire pass, a weak gang's `Raid` or `Retaliate` on the rival's leader (`faction::rescore`), a corp's `Secure`, `Lobby` and culprit Locate (`corp_brain`), the law's public bounties (`law_brain`). The law's `accessory_check` runs in `law::run`'s midnight branch.

**The status machine.** `Open → Taken → Fulfilled | Failed | Expired | Reneged | Cancelled`, and `Taken → Open` on a failed attempt below `max_attempts`. `contracts::set_status` is the one writer: it stamps `closed`, unhooks every index (the ledger queue, the Locate targets, the Fixer's book, the live run and its plan), counts the CSV columns and logs the event; `settle` pays the taker (price less the Fixer's cut, the cut credited to the Fixer) or refunds the buyer, so a record never closes holding escrow (`escrow_stuck`, 0). A direct record whose buyer reneges pays nothing and posts `Betrayed` with a grudge.

**Renders.** A record is `LIVE` when its taker or target is a body (pinned or on screen), a squad or a gang's job, or a Guard taken by an agent: the taker works it through the scripted Contract goal (`GoTo(Intel) → [AskAround] → StakeOut → Attack` on `hunt::chase`'s intel; a Guard stands its post); otherwise `LEDGER`: one draw on the contract stream at `due` (the strike decision first, then `p_win` against the target's habit tile), promoted to `LIVE` if a party becomes a body first.

**Missions are the raid machinery's third expedition.** `raid::Expedition` is `Gang(gang) | Riot(id) | Mission(contract)`: a gang's job (`Order::Job`) or a weak gun's squad (recruited while the solo estimate is under `squad_below`) musters, marches and brawls through the same `depart`, `fight_out` and `wait_for_crew` arms as a gang raid and a riot (a mission's `fight_out` matches a raid's for the same lists and seed). At the muster `missions::decide` computes the strike: `p_win`, the political term in a district a faction holds (its share, regard and fear of the buyer's faction, doubled for a member of the holder), and `EV = p_win − loss_w × (1 − p_win) − pol_w × political`: Strike, Hold (a day, up to `hold_days`), Fail (`StrikeDeclined`) or, for the holder of the district, `SoldOut` (it takes the job over at 1.5 × the price, the buyer topping up the escrow); a strike in a held district pushes `Shock::Trespass`/`CorpShock::Trespass` on the holder.

**The ledger's contract holes are a binder rule.** A ledger Hit or Beat opens its hole through `lod::stat_hit` with `ViolenceSource::Contract(id)` and the taker as the hole's `faction`, naming nobody. At the binder (`bind::bind`) a contract hole is **pre-bound** to that faction (the taker; the target for a taker killed by its target): the Unknown and candidate draws are made and discarded, so the witness roll reads the same stream offset an ordinary hole binding one actor reads, and saves, loads and re-binds give the same answer (`contract_hole_wrong`, a hole bound to anyone but its faction: 0).

**The law.** A contract killing is a Murder like any other. The placing agent is reached only through the `Hired` deed: first-hand in the buyer, the Fixer's owner and the taker; told by a Fixer to its regulars at a Drink (`fixer_talk × (1 − heat)`); named by an arrested taker the law interrogates (an Intimidate move). At midnight a city guard or a witness of the strike holding `Hired` about a fulfilled Hit or Beat files `Crime::Conspiracy` against the placing agent (`Accessory`, sentence `accessory_mult` × Murder's); the law's own records are never charged, and the record's own side (taker, crew, the broker's owner) never files. A Fixer named to the law warms; at `warrant_heat` the law closes it for `close_days` (`FixerBusted`, the open book refunded), and its owner may bribe the captain from its own purse (`Payer::Owner`).

The rules are in [M16_CONTRACTS.md](M16_CONTRACTS.md) (§ 1 the record, § 2 Fixers and hired guns, § 3 missions, squads and the law, § 7 the Statistical tier and the quotas, § 8 the levers and god commands, § 9 the UI) and what the build changed under "Implemented: deviations (M16a)". Contracts are always on.

### Economy

Daily at `tick_of_day == 0` plus per-event hooks. Inputs: building stocks, Market, Treasury, Jobs, lever `tax_rate`, season. Outputs: stock changes, `price_food`, `price_history` (cap 120), Wallet changes, `days_unpaid`.

- **Production.** For every 60 ticks of a Farmer's work action, `Farm.production_accum += 1.25 × (0.6 + 0.8 × farming) × farm_yield_mult`; whole units move from the accumulator into `stock_food` (cap 400; surplus lost). Farming skill +0.002 per completed shift (cap 1.0). At farming 0.5 this gives the 240 food/day in Spring stated in the world model.
- **Hauling.** At shift end, a Farmer whose Farm has `stock_food ≥ 10` gets `HaulToMarket` appended to their plan: `min(50, stock)` units move to `Market.stock_food` (cap 2000; overflow to Warehouse, cap 3000, then lost).
- **Price.** Daily: `price_food = clamp(round(3 × sqrt(600 / max(stock, 7))), 1, 30)`; pushed to `price_history`.
- **Trades.** `BuyFood` takes `min(3, floor(coins / price), 20 − inventory.food)` units and pays `units × price` into the Treasury (the Market is city-owned). `SellFood` pays `units × floor(price × 0.6)` from the Treasury and fails if the Treasury cannot cover it. `Fence` pays `units × floor(price × 0.8)` from the gang treasury.
- **Wages.** At `CollectWage`: `due = wage_per_day × days_unpaid`. `Job.tax_accum += due × tax_rate at every collection; tax = floor(tax_accum) is withheld into the Treasury and subtracted from tax_accum, so small wages are still taxed over time`. If the Treasury can pay `due − tax`: pay, `days_unpaid = 0`, memory `Paid`. Otherwise pay what it can down to 0, leave the remainder as `days_unpaid = ceil(remainder / wage)`, memory `Unpaid` (0.5, −0.5), `needs.wealth −= 0.1`. A worker with `days_unpaid ≥ 7`, or three `Unpaid` memories in 7 days, quits (Job removed, vacancy posted). Shift end sets `days_unpaid += 1`, meaning days owed. Dole: an unemployed adult may run CollectDole at the Hall once per day (dur 5) for dole\_per\_day coins (lever, default 3) while Treasury.coins ≥ 0; the Statistical tick pays it directly. The dole is what keeps 260 jobless citizens fed in good seasons; treasury pressure, not day-one poverty, is meant to cause the first famine.
- **Restock.** Market stock changes only through HaulToMarket, BuyFood, StealFood, SellFood, and `the player's ReleaseReserve command. There is no automatic Warehouse → Market transfer; the reserve exists so the lever matters`.
- **Spoilage.** Daily: Home pantries and Inventories lose `floor(food × 0.05)`; Market and Warehouse lose `floor(stock × 0.01)`.
- **Vacancies.** `world.vacancies: BTreeMap<EntityId, Vec<Role>>` is filled by `kill`, quitting, and jailing (a sentenced worker keeps the job only if the sentence is ≤ 3 days).

### Law and jail

Per crime event (`raise_crime`), per Arrest/Escort action, and a daily sweep. Inputs: crime events, Positions, Skills, Jobs, Memories, `crime_reports`, lever `sentence_mult`. Outputs: CrimeReports, Sentences, memories, Jail occupancy.

- **Witnessing.** `raise_crime(world, actor, victim, crime, tile)`: radius `r = 4 if is_dark else 8` (plain perception uses `SIGHT = 6`). Every Brain-bearing agent within `r` tiles (Chebyshev) or in the same building, excluding the actor, rolls `notice = 0.6 − 0.5 × actor.stealth + 0.3 × (witness is Guard)`; on success they get `SawCrime{subject: actor}` with salience Theft 0.5 / Extortion 0.6 / Assault 0.8 / Murder 1.0 and valence `−salience`. A Guard witness files `CrimeReport{crime, suspect: actor, witness, tick, resolved: false}` immediately; a citizen files only through the `ReportCrime` goal at the Hall. Victims always get `WasRobbed` or `Fought` regardless of the roll. The actor gains `stealth += 0.01` per unwitnessed crime (cap 1).
- **Warrants.** Open reports are those with `resolved == false`. One report per `(suspect, crime)`; a duplicate refreshes `tick`. Unresolved warrants expire after 30 days.
- **Suspect location.** Any Guard perceiving a warrant suspect writes `world.last_seen: BTreeMap<EntityId, (TilePos, Tick)>`; `known_suspect_location` is true within 240 ticks of a sighting.
- **Arrest.** The guard must be Chebyshev-adjacent. If `suspect.courage > 0.7` the arrest is contested with `resolve_fight`; a guard win cuffs, a loss gives the guard `Lost` and the suspect `safety −0.3` (so Flee follows). Uncontested: the suspect's plan becomes `[ServeTime]`, `suspect_cuffed` is set, and the suspect's Position copies the guard's each tick. **Escort** walks to the Jail door; on arrival `Sentence{until_tick: tick + days × 1440, crime}` is attached, the report is resolved, `Position.building = Jail`, and the suspect gets `WasArrested` (0.7, −0.5) plus the arrest trait drift.
- **Fight resolution** (Attack and contested Arrest): `p_win = clamp(0.5 + 0.4 × (fi_a − fi_b) + 0.1 × (C_a − C_b), 0.1, 0.9)`. Loser: `safety −0.4`, `mood −0.3`; with probability `0.15 × (1 + winner.fighting)` the loser dies (`kill(…, Violence)`) and the crime escalates to Murder. Winner `fighting += 0.01`.
- **Sentences.** Theft 3, Extortion 6, Assault 10, Murder 30 days, × `sentence_mult`, rounded up, min 1.
- **Capacity.** If the Jail holds 16 at Escort arrival: Theft becomes a fine of `2 × price_food` paid to the Treasury if affordable (report resolved, memory `Paid` 0.4 −0.4), else release at the Jail door with `lawfulness −0.05` and an `Unpunished` event. For other crimes the Theft prisoner with the longest remaining sentence is released early; if there is none, the arrestee is released as `Unpunished`.
- **Release.** When `tick ≥ until_tick`: remove `Sentence`, place at the Jail door, memory `WasArrested` (0.7, −0.5) again as the release memory, `belonging = 0.2`, `safety = 0.5`, `lawfulness +0.02` for a full sentence served. A vacated job is gone.
- **Guard duty.** Guards on shift with no warrant alternate: shifts where `guard.index % 2 == day % 2` run `GuardJail` (counts as the shift), the rest `Patrol`. A patrol loop is `[Market, Bar, Hall, Home(rng), Home(rng)]` with the Homes re-rolled per loop; `patrol_leg_done` resets after each leg and five legs complete a shift.

### Social graph

Per interaction, per-tick co-location, and daily. Inputs: Positions, Personality, Memory, action completions, Gang. Outputs: `world.edges`, Gang state, memories.

- **Edge creation.** `world.colocation: BTreeMap<(EntityId, EntityId), u16>` counts ticks two Brain agents share a building (reset on separation). At 30 ticks an edge is created: `affinity = (S_a + S_b)/2 × 0.1 + N(0, 0.05)` clamped to ±0.15, `trust = 0.3`, `debt = 0`, `kind = Acquaintance`. If the shared building is the Jail, both also get memory `MetInJail{subject: other}` (0.5, 0.0), which the acceptance scenario checks for.
- **Drift.** Chat, Drink and Flirt completion, and each full 60 ticks of co-work or co-jail: `affinity += 0.05 × m` with `m = 1.5 if |L_a − L_b| < 0.2 else 1.0`, `trust += 0.02`. Being robbed by someone (StealFood(Home) resident, Extort victim): `affinity −0.6`, `trust = 0`, `kind = Enemy`. Witnessing their crime: `affinity −0.2`, `trust −0.1`. Second-hand gossip: `affinity −0.1`. Beg success records `debt`; repayment (automatic at CollectWage when `coins > 20`, paying `min(debt, coins − 10)`) gives `affinity +0.1, trust +0.1`; a debt older than 14 days costs `affinity −0.15` on the donor's side. A fight: both `affinity −0.3`, loser's `trust −0.2`.
- **Promotion.** After every change: Acquaintance → Friend at `affinity ≥ 0.4`; Friend or Acquaintance → Rival at `≤ −0.3`; any → Enemy at `≤ −0.6`; Enemy → Rival above −0.3; Rival → Acquaintance above 0. Family, Parent and Spouse never change by threshold.
- **Propose.** Both unmarried, `affinity ≥ 0.6`, `trust ≥ 0.5`. Roll `rng < 0.5 + 0.5 × affinity`. Success: `kind = Spouse`, both `Married`, the target moves into the proposer's Home if it has room, else the proposer moves. Failure: proposer `Rejected` (0.6, −0.5), `affinity −0.1`.
- **Daily decay.** Edges with no interaction for 7 days: `affinity ×= 0.98`. Prune Acquaintance edges with `|affinity| < 0.05` and no interaction for 30 days.

### Gang

Daily plus per action. One gang in v1; the structure supports more.

- **Eligibility for JoinGang:** an edge to any member with `affinity ≥ 0.2`; or `hunger < 0.2 && lawfulness < 0.3`; or the gang is empty and the agent holds a `WasArrested` memory.
- **Ranks.** Daily, `leader = argmax(loyalty + days_in_gang / 100)`; `rank = 2` for the leader, 0 otherwise. Leader arrested or killed → recomputed immediately.
- **Stipend.** Daily, each member receives 4 coins from `Gang.treasury` while it holds ≥ 4, leader first.
- **Extort.** Target: the nearest Home not already in territory with no guard within 8 tiles. Takes `min(5, occupants' coins)` proportionally into the actor's wallet; `Gang.extort_count[home]++`, and at 3 the Home joins `territory`. Territory Homes pay 2 coins/day into the gang treasury when occupants can. `SplitLoot` pays `floor(gained × 0.5)` into the treasury; `gang_task_done` resets daily.
- **Disband.** `Gang.empty_since: Option<Tick>`; after 30 days with no members, treasury and territory are cleared and the name is kept.
- **Betrayal.** Daily, a member with `loyalty < 0.3` and an open warrant on themselves gets `ReportCrime +0.3` targeting the leader's most salient crime in their memory; on filing, the suspect is the leader, the betrayer leaves the gang, every remaining member gets an Enemy edge (`affinity −0.7`) to them, and the betrayer's own warrant is resolved.

### Demography

Daily at `tick_of_day == 0`; `kill` per event. Inputs: Identity, Needs, Mood, Household, edges. Outputs: new entities, Corpses, component removal, Treasury, vacancies.

```rust
// Identity gains one field:
pub spouse_died_tick: Option<Tick>,

pub const ADULT_AGE_DAYS: u32 = 2160;     // 18 years
pub const BIRTH_P: f32 = 0.004;
pub const CHILD_FOOD_PER_DAY: f32 = 0.5;
pub const OLD_AGE_DAYS: u32 = 10800;      // 90 years
pub const OLD_AGE_P_PER_DAY: f32 = 0.01;
```

- **Ageing.** `age_days += 1`. At 2160 the agent gains Brain, Needs and job eligibility with `lod = Coarse`. From 10,800 days each day carries a 0.01 chance of `OldAge` death.
- **Children.** Statistical only, no Brain, `Household.home` set. Each Home deducts `0.5 × children` food per day from the pantry (accumulated in `child_food_debt: f32`). If the pantry is empty a child's `hunger_days += 1`; at 3 the child dies of starvation.
- **Birth.** For each Spouse edge with both aged 18–45 (2160–5400 days), same `Household.home`, both `intimacy ≥ 0.6`: `p = 0.004 × (1.5 if both mood > 0.5 else 1.0)`, at most one birth per couple per 120 days (`Edge.last_birth_tick`). On success spawn a child next day: `age_days = 0`, personality per the blending rule, skills 0, Parent edges from each parent (`affinity 0.6, trust 0.8`), Family edges to siblings (`affinity 0.3`); a birth may exceed the Home's capacity of 6.
- **kill.**

```rust
pub fn kill(world: &mut World, id: EntityId, cause: DeathCause) {
    // 1. snapshot tile, home, job, spouse, children, coins
    // 2. remove Brain, Needs, Job (post vacancy), Household (occupants −1), GangMember,
    //    Sentence, reservations, plan_queue entry
    // 3. insert Corpse { died_tick, cause, buried: false }; keep Position and Identity
    // 4. SawCorpse (0.6, −0.5) to Brain agents within 6 tiles or in the same building
    // 5. Grief (0.9, −0.9) to every Spouse / Parent / Family edge partner anywhere;
    //    spouse.identity.spouse_died_tick = Some(tick); the Spouse edge kind is unchanged
    // 6. inheritance: coins → living Spouse, else split equally among living children
    //    (remainder to the first), else Treasury
    // 7. push Event::Death { id, cause }
}
```

- **Corpses.** Daily, an unburied corpse older than 2 days gives agents within 6 tiles `safety −0.1` and `SawCorpse` (0.4, −0.4). At 10 days the entity is freed and `Rotted` is logged. `BuryCorpse` sets `buried = true`, moves the corpse to the Cemetery, and the entity is freed a day later. The Gravedigger is pushed a `SawCorpse` (0.5) for every unburied corpse daily so the Bury goal can fire.
- **Job search.** Daily, for each vacancy in BTreeMap order: candidates are unemployed adults with a Brain, no Sentence, and `lawfulness ≥ 0.4` for Guard; the candidate with the smallest Manhattan distance from home door to workplace door wins (ties by `EntityId`).
- **Emigration.** From the mood section: `mood < −0.8` for 7 days → the agent walks to a map-edge Road tile and is despawned with an `Emigrated` event.

### Immigration

Weekly at `day % 7 == 0`, `tick_of_day == 0`, lever `immigration_per_week` (default 2). Each immigrant: age `U(2160, 12000)` days, uniform random Personality, Skills 0.1, `Wallet{15}`, `Inventory{0}`, `lod = Coarse`, placed at a random map-edge Road tile from the precomputed `world.edge_roads`. Housing: the Home with the fewest residents among those under 6 (ties by `EntityId`); if none, `Household.home = None` (homeless: Sleep on the Street, no `EatAtHome`, `safety −0.05/day`). No edges on arrival; job search the next day.

## Memory and perception

Perception is consumed immediately and never stored; memory is a bounded, decaying list of salient events per agent.

**Perception.** Each tick a Full agent perceives every entity within `SIGHT = 6` Chebyshev tiles (crime witnessing uses 8 by day / 4 in the dark) plus all occupants of its current building. Consumers: the law system (`last_seen` for warrant suspects, guards only), corpse noticing (first sight of an unburied Corpse writes `SawCorpse` 0.6 −0.5 once per corpse per agent), threat detection (`threat_removed = false` when an Enemy-edge partner or a currently fighting agent is within 4 tiles), and partner availability for reservations. Coarse agents perceive only building co-occupants. Statistical agents perceive nothing.

**Memory API.**

```rust
pub struct MemoryEntry {
    pub kind: MemoryKind, pub subject: Option<EntityId>, pub tick: Tick,
    pub salience: f32, pub valence: f32, pub second_hand: bool,
}
pub const MEMORY_CAP: usize = 24;
pub const MEMORY_HALF_LIFE_DAYS: f32 = 7.0;
pub const MEMORY_DROP_BELOW: f32 = 0.05;

pub fn remember(world: &mut World, who: EntityId, entry: MemoryEntry) {
    let Some(mem) = world.memory_mut(who) else { return };
    // merge rapid duplicates: same kind + subject within 60 ticks keeps the higher salience
    if let Some(e) = mem.entries.iter_mut()
        .find(|e| e.kind == entry.kind && e.subject == entry.subject && entry.tick - e.tick < 60) {
        e.salience = e.salience.max(entry.salience); e.tick = entry.tick; return;
    }
    if mem.entries.len() >= MEMORY_CAP {
        let now = world.tick;
        let weight = |e: &MemoryEntry| e.salience * 0.5f32.powf((now - e.tick) as f32 / 1440.0 / 7.0);
        let i = (0..mem.entries.len()).min_by(|&a, &b| weight(&mem.entries[a]).total_cmp(&weight(&mem.entries[b]))).unwrap_or(0);
        mem.entries.swap_remove(i);
    }
    mem.entries.push(entry);
}
```

`MemoryKind` gains `MetInJail` (written on jail co-location, see Social graph) and `Tantrum` (see Mood).

**Decay.** Daily at `tick_of_day == 0`: `salience ×= 0.5^(1/7)` (≈ 0.9057); entries under 0.05 are removed. Decay runs before any system that could hit the cap.

**Readers.**

| System | Kinds read | Use |
| --- | --- | --- |
| Utility `ReportCrime` | first-hand `SawCrime`, salience ≥ 0.5, subject not in an open report | score input `memory.salience` |
| Utility `Bury` | `SawCorpse` whose subject is still an unburied Corpse | nearest becomes `Plan.target` |
| Mood | all, age ≤ 7 days | `memory_term` in the mood formula |
| Utility `JoinGang` | own `WasArrested`, `MetInJail`; edges to members | eligibility; +0.2 if any `WasArrested` |
| Utility `Fight` (revenge) | `WasRobbed` with a subject within 4 tiles | +0.2; `Plan.target = subject`; Attack cost −5 |
| Utility `Flee` | `Fought` or `Lost` in the last 2 days with the subject within 8 | +0.15 |
| Utility `Court` | `Courted`, `Rejected` | +0.1 per `Courted` toward the same subject; `Rejected` by a subject blocks Flirt at them for 7 days |
| Social | `Grief` | a widowed spouse with `Grief` under 14 days old has `Court` × 0 |
| Law `HideFromLaw` | `WasArrested` + open warrant on self | `is_safe` false while a guard is within 8 |
| Economy quit | `Unpaid` | three in 7 days → quit |
| Gossip | `SawCrime`, `WasRobbed` | below |

**Gossip.** On `Chat` completion each party selects (seeded RNG) one own memory with `kind ∈ {SawCrime, WasRobbed}`, `salience ≥ 0.5` and a subject, and copies it to the partner via `remember` with `salience × 0.6`, `second_hand = true`. Second-hand entries never satisfy `ReportCrime` and never create reports; on receipt the receiver's edge to the subject (created at affinity 0 if absent) gets `affinity −0.1`. A memory is not re-gossiped to someone who already holds the same kind and subject.

**Statistical agents.** No perception, and `Memory` is attached lazily with cap 8 by `remember_statistical`, which accepts only `Grief` (from `kill` on kin), `WasRobbed` (household extorted) and `MetInJail`. On promotion the entries are kept and the cap becomes 24.

## Level of detail and scheduler

Every 60 ticks the 50 highest-priority agents become Full, the next 100 Coarse, the rest Statistical; a calibrated hourly table stands in for the brain of a Statistical agent.

### Assignment

`systems::lod::run` executes when `tick % 60 == 0`. For every living, unjailed adult:

```
priority = (on_screen ? 3 : 0) + (pinned ? 2 : 0) + (story_relevant_step ? 1 : 0)
```

- `on_screen`: `Position` lies inside `world.view_rect` expanded by 8 tiles. The app `issues SetView(rect) as a PlayerCommand whenever the view rect changes, so it is in the command log and replays deterministically`; headless runs leave it `None`.
- `story_relevant_step`: the plan's current action is one of `StealFood, Arrest, Attack, Propose, JoinGang, Extort, BuryCorpse`.
- Tie-break: ascending Manhattan distance to the view centre (map centre `(48, 32)` headless), then ascending `EntityId.index`.

Stable-sort by `(priority desc, dist asc, index asc)`. First `MAX_FULL = 50` → Full, next `MAX_COARSE = 100` → Coarse, rest → Statistical. Jailed agents are always Coarse and do not consume slots. Hysteresis: a Full agent ranked 51–60 stays Full and the agent that would displace it is held at Coarse; the same 10-rank band applies at the Coarse/Statistical boundary.

Since M10 the priority adds a rank class (`lod::class_with`) and the caps are `[lod] max_full` 50 and `max_coarse` 150. Since Life L2 (`[lod] budget`) each forced class has a quota inside them and held agents cost no body:

| Class | Who | Quota / rule | Slots |
| --- | --- | --- | --- |
| 5 | pinned | uncapped | ranked (Full first) |
| 4 | runners, hunters, hunted | `set_lod` refuses their demotion | ranked |
| 3 | public guards and gravediggers | all (`watch_on_shift_only = false` keeps M10 D20: off shift too) | ranked |
| 2 | per gang: leader, two lieutenants, members with a story step, the order's front line (`front_tiles`), then a daily rotation | `gang_quota` 25, `raid_quota` 40 from `raid_promote_hours` before a raid muster | ranked |
| 2 | private guards | the first `private_quota` 12 by id | ranked |
| 2 | rioters, abductees in tow, Harvest targets, episodes | as M12/M13 | ranked |
| 0 | civilians, members over their quota | screen distance, story, M11 bonus | ranked |
| held | sentenced, unpinned, not release-soon, cuffed or in a BreakOut window | Statistical in the cells, decayed hourly, fed at midnight (and at once when starving) | none |
| outside the rank | release within `release_soon_hours`, cuffed, a BreakOut gang's prisoners, emigrating | Coarse | none (few) |

So the ranked bodies are exactly `max_full + max_coarse`; Coarse outside the held class exceeds `max_coarse` only by the pinned, class-4 refusals and the bodies promoted after the assignment in the same tick (a released prisoner's walk out, a runner given an order, a week's immigrants, a thief caught by the Statistical pass), which the next assignment ranks.

### Transitions

| From → To | Rule |
| --- | --- |
| Full → Coarse | Keep goal, plan and step. A `Goto` becomes `GotoTimed { arrive_tick: now + manhattan × 2 }`; `Use` and `Wait` keep their remaining duration |
| Coarse → Full | Keep plan. A `GotoTimed` becomes a path request; the agent waits in place until served |
| Coarse → Statistical | Abort plan (`Event::PlanAborted { reason: LodDemotion }`). Snap Position to the Home door at Night or Morning; workplace door in the Work phase if employed, else Home; Market door if homeless |
| Statistical → Coarse or Full | Place at the door for the current phase: Night or Morning → Home; Work → workplace if employed, else Home; Evening → Bar if `sociability ≥ 0.5`, else Market. Needs, wallet, inventory, memories and edges are untouched; the agent thinks at its next slot |

### Statistical hourly tick

`systems::lod::run_statistical` runs every tick, for the Statistical agents whose `id.index % 60 == tick % 60` (`lod::due_this_tick`; the slot is fixed per agent, so each runs 24 times a day and the load is spread over the hour), after assignment. Since M10 the Statistical ids are bucketed by slot (`World::stat_slots`), so finding the due agents is O(due), not O(tier). The outcome list below is the M0-M9 table; M10 adds off-screen victim and actor rolls (robbed, assaulted, killed, steal, flirt), holes and the binder, described in [M10_SCALE.md](M10_SCALE.md) section 3. For each such agent:

1. Apply 60 ticks of need decay in one step, clamped to `[0,1]`; recompute `wealth`.
2. Load the `StatTable` row for the current phase.
3. Draw one `u` from the agent's own RNG stream (`world.rng.agent(id)`, a ChaCha8 stream seeded from `(world_seed, id.index)`) and walk the cumulative distribution in the fixed order `p_eat, p_work, p_social, p_sleep, idle`. Exactly one outcome fires.

| Outcome | Resolution |
| --- | --- |
| eat | If `inventory.food ≥ 1`: eat (+0.5 hunger). Else if `coins ≥ price` and `market.stock > 0`: pay price into the Treasury, `market.stock −= 1`, eat. Else if `lawfulness < 0.3` and `market.stock > 0`: `Event::Theft`, `market.stock −= 1`, eat, then roll `p_caught = 0.25`; if caught, call `law::file_report(crime: Theft, suspect, witness: None, certain: true)` so the normal arrest path runs (the nearest on-shift guard is dispatched; the agent is promoted to Coarse). Otherwise nothing; hunger keeps falling |
| work | Only if employed and in the Work phase, else idle. `at shift end the Treasury pays wage_per_day less tax_accum directly into the wallet and days_unpaid = 0 (the Hall visit is skipped at this LOD); unemployed adults receive the dole the same way`. Farmer: `farm.production_accum += 1.25 × (0.6 + 0.8 × farming) × season_mult` |
| social | `belonging = min(1, belonging + 0.1)`; one existing edge of the agent gets `affinity +0.02`; with no edges, create one with a random Statistical agent in the same Home |
| sleep | Only at Night, else idle. `energy = 1.0` |
| idle | nothing |

Starvation, old age, release from jail and emigration are handled by the demography, law and mood systems, which iterate all agents regardless of LOD.

### StatTable and calibration

```rust
#[derive(Deserialize, Serialize)]
pub struct StatRow { pub p_eat: f32, pub p_work: f32, pub p_social: f32, pub p_sleep: f32 }
pub struct StatTable { pub morning: StatRow, pub work: StatRow, pub evening: StatRow, pub night: StatRow }
```

Values are never hand-written. `cargo run -p citysim-cli -- calibrate`:

1. Builds a world with seed 1000, 200 agents, all forced Full, with the pathfinder replaced by a straight-line stub (cost = manhattan × 2 ticks) and no renderer.
2. Runs 30 days. On every hour boundary records, per agent, which of `{ate, worked, socialised, slept, idle}` dominated the past 60 ticks by exec state.
3. Writes per-phase frequencies to `assets/stat_table.toml` with the header `# generated by calibrate, seed 1000, 30 days, DO NOT EDIT`.

**M10:** the table is StatTable v2 (24 rows keyed on phase, lawfulness bucket and hunger bucket, with the victim-side `p_robbed`, `p_assaulted`, `p_killed`), regenerated by `citysim-cli calibrate` v2 (500 agents on the real map, gangless, three seeds pooled). See [M10_SCALE.md](M10_SCALE.md) section 3 "StatTable v2". The description above is the v1 four-row table; an old four-row file is rejected with the calibrate message.

`World::new` loads the table and panics with `run cargo run -p citysim-cli -- calibrate` if it is missing. Re-run calibration whenever the planner, action costs or need rates change.

Acceptance test `test_full_vs_statistical_within_15pct` (`citysim/tests/lod.rs`, `#[ignore]`, run in CI with `--ignored`): seed 2000, 200 agents, 30 days, once with `config.lod.force = Some(Full)` and once `Some(Statistical)`. Compare thefts, hunger-days (agent-days with `hunger < 0.2`) and arrests per 100 agent-days; each Statistical metric must be within ±15% of Full, with an absolute floor of 0.5 per 100 agent-days so near-zero rates do not fail on noise.

### Think, plan and path budgets

- Think slot `id.index % 30`; goal selection runs when `tick % 30 == slot`, plus one urgent think per 30 ticks when a plan ends or fails.
- Plan queue as in the GOAP section: 12 plans per tick, ordered by utility score, entries dropped after 90 ticks.
- Path queue: `PATH_BUDGET = 20` requests per tick, FIFO. Building-door destinations use cached flow fields and cost no A\*; only agent-targeted paths (Arrest, Attack, Court) run A\*. Coarse agents never request paths.

### Performance targets

| Metric | Condition | Target |
| --- | --- | --- |
| Headless throughput | 300 agents, 1000×, `run --days 30`, release build | ≥ 8,000 ticks/s |
| Sim time per frame | 8×, 50 Full + 100 Coarse + 150 Statistical, release | < 4 ms |
| Frame time | same, 1280×800 | < 16.6 ms |
| Resident memory | 300 agents, 120 days | < 100 MB |
| Save file | 300 agents | < 5 MB |
| Save/load round trip | 300 agents | < 200 ms |

`cargo bench -p citysim` has `bench_tick_300_agents` (criterion); CI fails if the median tick exceeds 125 µs.

### CLI

Binary `citysim-cli` (clap):

```
citysim-cli run --days N --seed S [--speed 1000] [--report] [--lever "day=90:release_reserve=1500"]... [--save-at T] [--load FILE] [--force-lod full|coarse|stat]
citysim-cli calibrate [--days 30] [--agents 200] [--out assets/stat_table.toml]
```

M10 adds `run --population N --map FILE` and `--lever law_posture=<name|auto>`; `calibrate` defaults to `--agents 500 --map assets/map.txt --seeds 3` and takes `--straight-lines` as an opt-in. The report adds the hole, tier and violence columns (see the README).

`--report` prints `DailyStats` as CSV to stdout, one row per day, header first:

```
day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,ticks_per_sec
```

Exit code 0 on completion, 101 on panic. `--lever` entries become `PlayerCommand`s scheduled at `day × 1440`.

## Player levers and debug UI

Every lever is a `PlayerCommand` applied at the start of the next tick and recorded with its tick, so a save plus its command log replays deterministically.

### PlayerCommand and Levers

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlayerCommand {
    ReleaseReserve { amount: u32 },
    SetTaxRate(f32),
    SetSentenceMult(f32),
    SetGuardCount(u8),
    SetImmigrationPerWeek(u8),
    GrantCoins { agent: EntityId, amount: i64 },
    Arrest(EntityId),
    Release(EntityId),
    DemolishHome(EntityId),
    BuildHome { rect: Rect },
    Pin(EntityId, bool),
    SetView(Option<Rect>),      // issued by the app whenever the view rect changes; makes LOD replayable
    SetDolePerDay(u8),          // 0..=10, default 3
    SetSpeed(Speed),            // app-only; logged so replays know the speed, no sim effect
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Speed { X1, X2, X4, X8, X32, X128, X1000 }

// Levers (stored lever values) follow.

#[derive(Clone, Serialize, Deserialize)]
pub struct Levers {
    pub tax_rate: f32,            // 0.0..=0.3, default 0.05
    pub sentence_mult: f32,       // 0.5..=3.0, default 1.0
    pub guard_count: u8,          // 0..=30, default 10
    pub immigration_per_week: u8, // 0..=10, default 2
    pub dole_per_day: u8,         // 0..=10, default 3
}
```

`World::tick` drains `command_queue` first, applies each in order, appends `Event::PlayerAction`, and pushes `(tick, cmd)` onto `command_log`, which is serialised in saves.

| Command | Range / default | Takes effect |
| --- | --- | --- |
| `ReleaseReserve` | 1..=warehouse stock, clamped | Immediately: Warehouse → Market. Price recomputes at the next daily tick |
| `SetTaxRate` | 0.0–0.3, default 0.05 | Stored; applied by Economy at the next weekly withholding |
| `SetSentenceMult` | 0.5–3.0, default 1.0 | Stored; used at sentencing. Existing sentences unchanged |
| `SetGuardCount` | 0–30, default 10 | Reconciled at the next `tick_of_day == 0`. Hire: unemployed, unjailed, `lawfulness ≥ 0.4`, highest lawfulness first. Fire: the guard with the lowest loyalty. Up to 5 changes per day |
| `SetImmigrationPerWeek` | 0–10, default 2 | Stored; used by the weekly immigration tick |
| `GrantCoins` | 1..=treasury | Immediately: Treasury → wallet. `PlayerActionFailed` if the Treasury is short |
| `Arrest` | — | Immediately, no witness roll: teleport to the Jail door, `Sentence` of `3 × sentence_mult` days, plan aborted, memory `WasArrested` with subject `None`. Fails if the Jail is full |
| `Release` | — | Immediately: `Sentence` removed, placed at the Jail door |
| `DemolishHome` | must be a Home | Immediately: tiles become Ground, residents' `Household.home = None`, one event per resident, flow field dropped |
| `BuildHome` | rect 4×4 to 6×6 of Ground, not adjacent to Water, cost 200 | Immediately if the Treasury has 200: walls on the perimeter, door on the south edge, capacity 6; the lowest-index homeless move in. Otherwise `PlayerActionFailed { reason }` |
| `Pin` | — | Sets `Brain.pinned`; applied at the next LOD assignment (≤ 60 ticks) |
| `SetSpeed` | 1, 2, 4, 8, 32, 128, 1000 | App accumulator only |

### Inspector panel (egui, right, 360 px)

Shown when `app.selected: Option<EntityId>` is set by clicking an agent or a log row. Collapsible sections, all open by default:

1. **Identity**: name, age (years, days), sex, id `index:generation`, LOD with a Pin toggle, job (`Farmer @ Farm#1`), home (`Home#23` or homeless), status chips `Jailed until day N`, `Gang`, `Corpse`.
2. **Needs**: six horizontal bars with the value, red under 0.25.
3. **Personality**: six bars, read-only.
4. **Mood**: one bar −1..1 plus `need_term` and `memory_term` as text.
5. **Goal**: current goal and score, then the top-5 goals from the last think with one row per consideration showing input → curve output. Data comes from `Brain.last_think: Option<ThinkTrace>`, kept only for Full and Coarse agents.
6. **Plan**: steps with `>` on the current index, each `Action(target) cost = N`; exec state line (`Goto (12,40) 7/23`, `Use BuyFood 4 left`, `Wait 30`).
7. **Memories**: by salience desc, columns kind, subject, age, salience, second-hand flag.
8. **Edges**: by `|affinity|` desc, columns other, kind, affinity, trust, debt.
9. **Wallet and inventory**: coins, food, stolen food, days unpaid.
10. Buttons: `Grant 50`, `Arrest` / `Release`, `Centre camera`, `Follow`.

### City panel (left, 300 px)

Population, employed, homeless, jailed, gang members; food as Market / Warehouse / pantries and the sum; price with a 30-day sparkline from `stats.history`; treasury; deaths by cause, crimes by kind, births, immigrants and emigrants over the last 30 days. Below, the lever widgets: `tax_rate` slider, `sentence_mult` slider, `guard_count` and `immigration_per_week` drag values, `Release reserve` amount (default 500) with a button, `Build home` mode (next drag on the map draws the rect, green if valid, red if not), `Demolish` mode (click a Home). Sliders issue one command on release, not per frame.

### Event log (bottom, 220 px, resizable)

```rust
pub struct Event { pub tick: Tick, pub kind: EventKind, pub actors: SmallVec<[EntityId; 3]>, pub text: String }
pub enum EventKind {
    Theft, Extortion, Assault, Murder, Witness, Report, Arrest, Sentence, Release, Unpunished,
    Birth, Death, Burial, Rotted, Proposal, Marriage, GangJoin, GangLeave, Betrayal,
    Hire, Fire, Quit, Immigration, Emigration, Starving, Homeless, PlanAborted,
    PriceChange, Inheritance, PlayerAction, PlayerActionFailed,
}
```

`world.events` is a `VecDeque<Event>` capped at 5,000. The panel shows newest first with columns `day hh:mm | kind | text`, a kind multi-select, and an `Only selected` checkbox that filters to events whose `actors` contain the selected agent. Clicking a row selects `actors[0]` and centres the camera (on its building if the agent is not Full).

### Time controls

HUD bar, right end: pause/resume (Space), speed radio `1 2 4 8 32 128 1000` (keys 1–7), step one tick (`.`), step one hour (`,`). Stepping while paused calls `tick()` directly.

### Save and load

F5 saves `saves/<seed>-<tick>.ron`; F9 loads the newest for the current seed (or the `--load` file). `save.rs` serialises the whole `World` with serde + ron, including the RNG state, every component vector, the event ring, stats history, levers, `command_log` and LOD state. Flow fields are `#[serde(skip)]` and rebuilt lazily. Camera, selection and speed go to a sidecar `saves/<seed>-<tick>.app.ron`.

Test `test_save_load_bit_identical`: seed 42, run to tick 3000, save, run the original and a freshly loaded world 1,000 ticks each, compare `blake3` of the serialised `World`; they must match.

## Rendering

macroquad, 1280×800, squares and letters only; the renderer reads `&World` and never mutates it.

### Camera and loop

```rust
pub struct Camera { pub centre: Vec2 /* tile units */, pub px_per_tile: f32 /* 8.0..=48.0, default 16 */ }
```

Pan with WASD at 20 tiles/s or left-drag when not over a panel; zoom with the wheel at ×1.25 per notch, about the cursor, clamped 8–48. `Camera::view_rect()` is passed to `world.set_view` every frame.

```rust
let mut acc = 0.0f64;
loop {
    let dt = get_frame_time() as f64;
    if !paused { acc += dt * 8.0 * speed_mult; }   // speed_mult in {1,2,4,8,32,128,1000}
    let mut n = acc.floor() as u32; acc -= n as f64;
    n = n.min(4000);                                 // drop ticks at 1000x on slow frames
    world.push_commands(&mut ui_cmds);
    for _ in 0..n { citysim::tick(&mut world); }
    render::draw(&world, &camera, &app_state);       // &World only
    ui::draw(&mut ui_cmds, &world, &mut app_state);
    next_frame().await;
}
```

### Draw order and colours

Back to front, in tile space via `set_camera(&Camera2D)`.

1. **Tiles**, culled to the view: Ground `#2e2a24`, Road `#4a4440`, Wall `#6b6660`, Door `#9a7b4f`, Farmland `#3f5a2a`, Water `#27485e`.
2. **Buildings**: 2 px outline `#c8c0b0` and one letter at the centre (H F M B J C T G W for Home, Farm, Market, Bar, Jail, Cemetery, Town hall, Gang hideout, Warehouse); demolished Homes get a dashed outline and no letter. Labels only at `px_per_tile ≥ 12`.
3. **Count badges**: per building, the number of Coarse and Statistical agents inside, as a black 80% pill at the top-right with white text. Built once per frame into a `Vec<u16>` indexed by building.
4. **Corpses**: a dark X (`#1a1a1a`, two 2 px lines).
5. **Agents (Full only)**: a 1-tile square inset 1 px, coloured by current action:

| State | Colour |
| --- | --- |
| idle, no plan, wander | `#8a8a8a` |
| working | `#3d7bd9` |
| eating | `#4caf50` |
| sleeping | `#1f2f6b` |
| stealing, extorting, attacking | `#d92f2f` |
| fleeing | `#f08c1e` |
| guard patrolling or arresting | `#f5f5f5` |
| socialising, courting | `#d9a23d` |
| jailed | `#555555` at 60% alpha |

Gang members get a 2 px `#8e44ad` border; guards a 1 px white border when not arresting.

6. **Selection**: 2 px `#ffd700` outline, the remaining path as a 1 px polyline through tile centres, and a 6-tile sight circle at 40% alpha. For a non-Full agent, outline its building instead.
7. **Day/night**: a screen-space overlay `#0a0f2a` at alpha 0.35 during Night, ramping linearly over the 60 ticks on each side of the phase boundary.
8. **HUD top bar** (28 px, black at 70%): `Day 12 · Autumn · Work 09:40 · 8x · Pop 298 · Food 1840 · Price 4 · Treasury 2310` plus the time controls; `PAUSED` in orange when paused.

### Performance rules

- Cull tiles to the view; the full map at 8 px/tile is 6,144 rects, which is within budget.
- No allocations in the agent loop except the one selected agent's path polyline.
- Target 60 fps at 8× with 300 agents (50 drawn); a `--fps` flag shows the rolling average in the HUD.
- `render.rs` takes `&World` only; `#![deny(clippy::needless_pass_by_ref_mut)]`.

## Milestones and acceptance tests

Eight milestones, each a git commit with its test list green and the 30-day CLI run exiting 0. Build the world rules before the brains and the statistical layer last.

### M0 — Repo, headless world, save/load, CLI

Deliverables: workspace; `entity.rs` (`EntityId`, generational arena); `world.rs`; `map.rs` (parse `assets/map.txt`, build buildings); `time.rs`; `rng.rs`; `config.rs`; `events.rs`; `stats.rs`; `save.rs`; `citysim-cli run`. Agents exist with Position, Identity, Job and Household but do nothing. The app opens a window and draws tiles, buildings and static agents.

| Test | Asserts |
| --- | --- |
| `test_map_parses_expected_counts` | 60 Home, 2 Farm, 1 of each other kind; every building has exactly one Door tile adjacent to a Road |
| `test_spawn_300_with_jobs` | population 300; job counts 24/10/3/2/1/260; everyone has a home |
| `test_determinism_same_seed_same_hash` | two seed-42 worlds run 2,000 ticks → identical `blake3` of the serialised World |
| `test_different_seed_different_hash` | seeds 42 and 43 differ |
| `test_entity_generation_invalidates_stale_id` | despawn then respawn reuses the slot; the old id returns `None` |
| `test_save_load_bit_identical` | as specified under Save and load |
| `test_cli_report_csv_header` | `run --days 1 --seed 1 --report` prints the exact header line |

Scenario: `run --days 10 --seed 42 --report` → 10 rows, population 300, no panic. Demo: window opens, pan and zoom work, F5 writes a file into `saves/`.

### M1 — Needs, economy, scheduled behaviour

Deliverables: `needs.rs`; `systems/economy.rs` (production, hauling, price, wages, tax, spoilage, warehouse transfer); a hard-coded routine in `exec/` (Night: go home and sleep; Work: go to the workplace; Evening: Market if hungry and able to pay, else Bar if `sociability ≥ 0.5`; eat at home); `exec/pathfind.rs` A\*; `exec/flowfield.rs`.

| Test | Asserts |
| --- | --- |
| `test_price_formula_table` | stock 7 → 28, 50 → 10, 600 → 3, 2400 → 2, 10000 → 1 |
| `test_price_rises_when_stock_falls` | draining the Market from 600 to 100 raises the price at the next daily tick |
| `test_farmer_produces_food` | 24 farmers at farming 0.5, one Spring day, hauling disabled → farm stock +302 ± 10 |
| `test_wage_taxed_to_treasury` | one weekly withholding at 0.05 → Treasury gains `floor(due × 0.05)` |
| `test_hunger_decays_and_eating_restores` | from hunger 1.0, 1,440 ticks without food → hunger ≈ 0.5; one meal → +0.5 clamped |
| `test_flowfield_matches_astar_length` | 20 random starts: flow-field descent length equals A\* length |
| `test_pathfinder_never_crosses_wall_or_water` | 200 random paths |

Scenario: `run --days 30 --seed 42 --report` → `mean_hunger ≥ 0.4` every day, ≤ 5 starvation deaths, price 2–8 throughout, Market stock never 0 on more than 2 consecutive days. Demo: at 8× agents visibly cluster in Homes at night and on the Farms by day.

### M2 — Utility goal selection, inspector

Deliverables: `personality.rs`; `mood.rs`; `utility/` (curves, considerations, compensation, hysteresis, cooldowns); `ThinkTrace`; staggered think scheduling; `ui/inspector.rs`. Goals drive the M1 routines as actions.

| Test | Asserts |
| --- | --- |
| `test_curve_outputs_in_unit_range` | all four curves over 0..1 give 0..1 |
| `test_hungry_agent_picks_eat` | hunger 0.1, other needs 1.0 → top goal Eat |
| `test_tired_at_night_picks_sleep` | energy 0.2, Night → Sleep |
| `test_hysteresis_prevents_flip` | a goal within 0.10 of the current one does not replace it |
| `test_think_runs_every_30_ticks_staggered` | `last_think_tick` advances by exactly 30 |
| `test_worked_example_scores` | the goal-selection worked example reproduces 0.855 / 0.823 / 0.240 / 0.072 within 0.005 |
| `test_mood_in_range` | over 10 days every mood stays in −1..1 |

Scenario: M1 bounds plus `goal_changes_per_agent` between 3 and 12 per day. Demo: the inspector shows top-5 goals and consideration outputs moving; needs bars move.

### M3 — GOAP, first emergent theft

Deliverables: `goap/` (WorldState, actions, forward A\*, plan queue, replanning, cooldowns, reservations); `exec/` three-state executor replacing the M1 routine.

| Test | Asserts |
| --- | --- |
| `test_planner_finds_buy_and_eat` | hungry with coins → `[GoTo(Market), BuyFood, EatFromInventory]` |
| `test_planner_respects_limits` | an unsatisfiable goal returns `Err` within 200 expansions and 6 steps |
| `test_hungry_broke_low_lawfulness_steals` | the worked trace 1 inputs → plan contains `StealFood` with total cost 13.1 ± 0.1 |
| `test_hungry_broke_high_lawfulness_forages` | worked trace 2 inputs → `Forage`, not `StealFood` |
| `test_failed_goal_cools_for_120_ticks` | two failures in 60 ticks → goal score 0 until `tick + 120` |
| `test_plan_budget_12_per_tick` | 100 agents queued → exactly 12 planned on the first tick |
| `test_reservation_prevents_overcommit` | 3 agents, 2 food units → the third's `food_source_available` is false |

Scenario: `run --days 30 --seed 42 --report` → thefts ≥ 1, arrests 0 (no law yet), population 300, no panic. Demo: filter the log to Theft, click the row, the inspector shows a plan containing `StealFood`.

### M4 — Memory, witnesses, law, jail

Deliverables: `systems/memory.rs`; `systems/law.rs` (witness roll, reports, warrants, Arrest and Escort, sentences, capacity, release, guard duty); `Arrest` and `Release` commands.

| Test | Asserts |
| --- | --- |
| `test_witness_notice_formula` | stealth 0.2 citizen witness → 0.5; guard witness → 0.8 |
| `test_theft_witnessed_by_guard_leads_to_jail_within_1_day` | a scripted theft adjacent to a guard → `Sentence` within 1,440 ticks |
| `test_theft_unwitnessed_no_arrest` | nobody within sight → no report in 3 days |
| `test_memory_cap_24_evicts_lowest_weight` | 30 memories → 24 remain, the lowest salience × recency gone |
| `test_memory_half_life` | salience 1.0 → 0.5 ± 0.01 after 7 daily decays |
| `test_sentence_mult_scales_release_tick` | mult 2.0 → 6 days for Theft |
| `test_jail_full_theft_becomes_fine` | 16 jailed, 17th thief with coins → fine paid, no Sentence |
| `test_player_arrest_bypasses_witness` | `Arrest(id)` → `Sentence` on the next tick |

Scenario: `run --days 30 --seed 42 --report` → thefts ≥ 3, arrests ≥ 1, jailed ≤ 16 every day. Demo: a white guard chases a red thief; the thief's inspector shows `WasArrested` and the Jail badge count rises.

### M5 — Social graph, gangs

Deliverables: `systems/social.rs`; `systems/gang.rs`; Court, Flirt, Propose; JoinGang, Extort, SplitLoot, Fence; gossip.

| Test | Asserts |
| --- | --- |
| `test_jail_cellmates_gain_affinity_and_met_in_jail` | two agents jailed 2 days → edge affinity ≥ 0.4 and both hold `MetInJail` |
| `test_colocation_at_bar_creates_edge` | two agents at the Bar for 30 ticks → an Acquaintance edge exists |
| `test_propose_requires_thresholds` | affinity 0.6 / trust 0.5 → allowed; 0.5 / 0.5 → not |
| `test_join_gang_requires_contact_or_desperation` | lawfulness 0.2, hunger 0.9, no edge → no JoinGang; add an edge at 0.25 → JoinGang |
| `test_extort_moves_coins_and_adds_memory` | victims −5 total, actor +5, victims hold `WasRobbed` |
| `test_gossip_copies_second_hand` | after Chat the partner holds the SawCrime at 0.6 × salience, `second_hand = true` |
| `test_edges_deterministic_order` | iteration order identical across two seed-42 runs |

Scenario: `run --days 60 --seed 42 --report` → `gang_members ≥ 2` by day 60, ≥ 1 GangJoin, ≥ 1 Marriage. Demo: purple-bordered agents exist; one has `MetInJail` with a current member as subject.

### M6 — Demography

Deliverables: `systems/demography.rs` (ageing, children, birth, old age, `kill`, corpses, burial, inheritance, job search, emigration, immigration); `BuildHome` and `DemolishHome`.

| Test | Asserts |
| --- | --- |
| `test_starvation_kills_after_3_days_at_zero` | hunger 0 for 4,320 ticks → `Death { Starvation }` and a Corpse |
| `test_old_age_death_probability` | 10,000 agents at 95 years for one day → 70–130 deaths |
| `test_corpse_buried_by_gravedigger_within_2_days` | a corpse in a Home → Burial event, corpse at the Cemetery |
| `test_inheritance_spouse_then_children_then_treasury` | three cases |
| `test_birth_blends_traits_and_links_parents` | child traits within the parents' mean ± 0.45; Parent edges exist |
| `test_immigration_two_per_week` | 4 weeks → 8 immigrants, each housed or homeless |
| `test_demolish_home_makes_residents_homeless` | `home == None` for each resident, one event each |
| `test_build_home_rejects_invalid_rect` | on Water, overlapping, or Treasury short → `PlayerActionFailed` |

Scenario: `run --days 120 --seed 42 --report` → births ≥ 1, deaths ≥ 1, burials ≥ 1, population 200–400. Demo: a dark X appears, the gravedigger carries it to C, deaths-by-cause updates.

### M7 — LOD, calibration, performance, full levers

Deliverables: `systems/lod.rs`; Statistical tick; `StatTable`; `citysim-cli calibrate`; benches; remaining levers, city panel, badges.

| Test | Asserts |
| --- | --- |
| `test_lod_assignment_counts` | Full ≤ 50, Coarse ≤ 100 after assignment |
| `test_pinned_agent_is_full` | pin → Full at the next assignment |
| `test_demote_full_to_coarse_keeps_plan` | plan and step preserved, Goto timed |
| `test_demote_to_statistical_snaps_home_at_night` | position equals the Home door |
| `test_statistical_hourly_decay_equals_60_ticks` | needs match a Full agent idling 60 ticks within 1e-5 |
| `test_full_vs_statistical_within_15pct` | thefts, hunger-days and arrests per 100 agent-days, `#[ignore]` |
| `test_headless_throughput_8000_tps` | release only, 300 agents, 10 days |
| `test_command_log_replay_matches` | save with 5 commands applied, replay → same hash |

Scenario: `run --days 120 --seed 7 --speed 1000 --report` → `ticks_per_sec ≥ 8000` on the last row and the M6 bounds hold. Demo: at 1000× the day counter advances ≈ 1 day per 1.5 s; badges appear; a pinned agent stays drawn at any zoom.

### v1 acceptance (`citysim/tests/scenario.rs::test_v1_acceptance`, `#[ignore]`, run in CI)

Run A: seed 7, 120 days, headless, no view rect. Assert from `DailyStats` totals and the event log:

- no panic;
- thefts ≥ 10; arrests ≥ 5;
- ≥ 1 GangJoin whose recruit holds a `MetInJail` memory whose subject was a gang member at the join tick (the event text records `cites mem#<n>`);
- births ≥ 1; ≥ 1 `Death { Starvation }` in Winter; burials ≥ 1;
- population on day 120 in 200..=400.

Run B: same, plus `--lever "day=90:release_reserve=1500"`. Assert starvation deaths over days 90–120 are fewer in B than in A, and that A and B are bit-identical through tick `90 × 1440 − 1`.

### Repo layout

```
Cargo.toml                     # [workspace] members = ["citysim", "citysim-app", "citysim-cli"]
citysim/
  Cargo.toml                   # rand, rand_chacha (serde), serde, ron, toml, smallvec, ordered-float; dev: blake3, criterion
  src/lib.rs world.rs entity.rs components.rs map.rs time.rs needs.rs personality.rs mood.rs
  src/utility/{mod.rs,curves.rs,goals.rs}
  src/goap/{mod.rs,world_state.rs,actions.rs,planner.rs}
  src/exec/{mod.rs,pathfind.rs,flowfield.rs,reservations.rs}
  src/systems/{mod.rs,economy.rs,law.rs,social.rs,gang.rs,demography.rs,memory.rs,lod.rs}
  src/events.rs levers.rs stats.rs save.rs rng.rs config.rs
  tests/{determinism.rs,economy.rs,utility.rs,goap.rs,law.rs,social.rs,demography.rs,lod.rs,save.rs,scenario.rs}
  benches/tick.rs
citysim-app/
  Cargo.toml                   # citysim, macroquad, egui-macroquad, egui_plot
  src/{main.rs,render.rs,camera.rs,input.rs,ui/{mod.rs,inspector.rs,city.rs,log.rs}}
citysim-cli/
  Cargo.toml                   # citysim, clap
  src/main.rs                  # subcommands run, calibrate
assets/{map.txt,names.txt,stat_table.toml,config.toml}
docs/SPEC.md                   # this document, exported
saves/                         # gitignored
```

### Coding rules for the implementing agent

- `cargo clippy --workspace --all-targets -- -D warnings` clean at every commit.
- Every system is `pub fn run(world: &mut World)` in its own module, called by `World::tick` in the fixed order given under World systems.
- No `.unwrap()` or `.expect()` on `Option<Component>` inside `systems/`, `utility/`, `goap/`, `exec/`; use `let Some(x) = world.comp::<T>(id) else { continue };`. Enforced with `#![deny(clippy::unwrap_used)]` in those modules.
- No `HashMap` or `HashSet` in `citysim` (`clippy.toml` `disallowed-types`); no `std::time` in `citysim`.
- Every constant lives in `assets/config.toml`, loaded once by `Config::load()` into a plain struct passed to `World::new(seed, config)`; no magic numbers in systems.
- Every emergent event appends exactly one `Event`.
- One commit per milestone, message `M<n>: <title>`, with the milestone's tests green and `cargo run -p citysim-cli -- run --days 30 --seed 42 --report` exiting 0.
- When the spec and a test disagree, the test is wrong until this document is changed; propose the change in the commit message rather than silently diverging.

## Tuning defaults

Every row is a key in `assets/config.toml`; the values match the sections above and are the starting point, not the answer.

| Key | Default | Unit | Raise when | Lower when |
| --- | --- | --- | --- | --- |
| `hunger_decay_per_tick` | 0.000347 | need/tick | nobody is ever hungry, Market stock never moves | many starvation deaths |
| `energy_decay_per_tick` | 0.000926 | need/tick | agents never sleep | agents sleep through shifts |
| `safety_recover_per_tick` | 0.0010 | need/tick | Flee lingers for hours | Flee never fires |
| `belonging_decay_per_tick` | 0.000139 | need/tick | the Bar is always empty | Socialise crowds out work |
| `intimacy_decay_per_tick` | 0.0000694 | need/tick | no marriages | Court dominates |
| `wealth_days_secure` | 7 | days of food | nobody seeks Earn | everyone works evenings |
| `food_satisfy` | 0.5 | need per unit | agents eat 3+ times a day | one meal lasts all day |
| `sleep_ticks_full` | 480 | ticks | agents wake tired | agents oversleep |
| `chat_belonging` | 0.3 | need | belonging never recovers | one chat satisfies a week |
| `starvation_grace_ticks` | 4320 | ticks at hunger 0 | starvation too sudden | nobody starves |
| `price_base` | 3.0 | coins | price low, stock drains | hunger-days rise |
| `price_ref_stock` | 600 | units | price rises too early | price flat in scarcity |
| `price_min_stock` | 50 | units | price explodes on tiny stock | cap never reached (the default is 7 after review; the 50 in this row is superseded) |
| `price_cap` | 30 | coins | famine not painful | the poor can never buy |
| `wage_farmer`, `wage_guard`, `wage_clerk`, `wage_bartender`, `wage_gravedigger` | 6, 8, 6, 5, 5 | coins/day | workers steal | treasury drains |
| `initial_coins_min`, `initial_coins_max` | 10, 40 | coins | week-1 theft spike | no theft for months |
| `treasury_initial` | 2000 | coins | wages unpaid in month 1 | levers trivially cheap |
| `farm_yield_base` | 1.25 | food per 60 ticks | chronic scarcity | Warehouse overflows |
| `farm_skill_floor`, `farm_skill_slope` | 0.6, 0.8 | multiplier | skill irrelevant | novices produce nothing |
| `season_mult` | 1.0, 1.4, 1.1, 0.3 | multiplier | Winter never bites | famine every Winter |
| `spoilage_pantry`, `spoilage_market` | 0.05, 0.01 | fraction/day | hoarding | pantries always empty |
| `warehouse_initial` | 1500 | food | the release lever is meaningless | starvation unavoidable |
| `tax_rate` | 0.05 | fraction | treasury drains | citizens stay broke |
| `steal_base_cost` | 8 | plan cost | theft too common | theft never planned |
| `steal_lawfulness_factor` | 20 | cost × lawfulness | lawful agents steal | only lawfulness < 0.1 steal |
| `steal_guard_penalty`, `steal_starving_bonus`, `steal_dark_bonus` | 6, 4, 3 | plan cost | guards irrelevant / nobody steals at night | theft only at night |
| `beg_base_cost`, `beg_pride_factor` | 5, 10 | plan cost | begging dominates | nobody begs |
| `forage_base_cost` | 6 | plan cost | foraging dominates | nobody forages |
| `witness_base`, `witness_stealth_factor`, `witness_guard_bonus` | 0.6, 0.5, 0.3 | probability | thefts never seen | every theft caught |
| `sight`, `sight_day_crime`, `sight_night_crime` | 6, 8, 4 | tiles | crimes unseen | no crime survives |
| `sentence_days` | 3, 6, 10, 30 | days | re-offence high | jail always full |
| `jail_capacity` | 16 | agents | arrests become fines | jail empty, crime high |
| `fight_death_p` | 0.15 | probability | fights are harmless | murders dominate deaths |
| `edge_create_ticks` | 30 | ticks co-located | no acquaintances | everyone knows everyone |
| `affinity_per_hour` | 0.05 | affinity | no friendships form | all friends in a week |
| `friend_threshold`, `rival_threshold`, `enemy_threshold` | 0.4, −0.3, −0.6 | affinity | no friends / no feuds | too many of either |
| `propose_affinity`, `propose_trust` | 0.6, 0.5 | thresholds | no marriages | marriages in days |
| `gang_stipend` | 4 | coins/day | nobody joins | gang outgrows the guards |
| `extort_amount` | 5 | coins | extortion pointless | victims starve |
| `join_gang_affinity` | 0.2 | affinity | no recruits | recruits without contact |
| `join_gang_desperation_hunger`, `join_gang_desperation_lawfulness` | 0.2, 0.3 | thresholds | no desperate recruits | everyone hungry joins |
| `birth_p_per_day` | 0.004 | probability | population declines | population explodes |
| `old_age_days`, `old_age_p_per_day` | 10800, 0.01 | days, probability | nobody dies old | elders vanish at once |
| `memory_cap`, `memory_half_life_days` | 24, 7 | entries, days | grudges forgotten | ancient memories drive goals |
| `goal_hysteresis` | 0.10 | score | goal flapping | agents stuck on stale goals |
| `goal_cooldown_ticks` | 120 | ticks | agents retry the impossible | agents give up on real needs |
| `think_interval_ticks` | 30 | ticks | agents react late | plan budget starved |
| `plan_budget_per_tick`, `plan_max_expansions`, `plan_max_len` | 12, 200, 6 | per tick, nodes, steps | queue age > 90 ticks | CPU idle |
| `path_budget_per_tick` | 20 | paths | agents wait for paths | CPU spikes |
| `max_full`, `max_coarse` | 50, 100 | agents | detail thin | frame > 4 ms |
| `stat_theft_caught_p` | 0.25 | probability | Statistical crime unpunished | arrests spike off-screen |
| `immigration_per_week` | 2 | agents | population shrinks | homeless pile up |
| `mood_w_need`, `mood_w_memory` | 0.7, 0.3 | weights | mood ignores crime | mood ignores hunger |
| `emigrate_mood`, `emigrate_days` | −0.8, 7 | mood, days | nobody leaves | mass exodus |
| `build_home_cost` | 200 | coins | the mayor spams homes | the homeless never get housed |

## Review notes

An independent reviewer read the full draft cold and flagged 41 items, 17 blocking. All blocking items and most important ones are resolved in the text above; this table records what changed so the implementing agent can see the reasoning.

| # | Finding | Resolution |
| --- | --- | --- |
| 1 | Price formula could never exceed 10, so the cap of 30 and the M1 test were unreachable | `price_min_stock` is now 7 (stock 7 → 28); test table updated |
| 2 | Farm output of 240/day contradicted the 300-tick shift cap, the 9-hour shift and the skill multiplier; the M1 test ignored hauling | Shifts run to 540 ticks, `farm_yield_base` 1.4, expected ≈ 300/day at farming 0.5; test runs with hauling disabled and expects +302 |
| 3 | 260 jobless citizens had no income path except begging and theft; the city starved in Spring | Added a dole: `CollectDole` at the Hall, lever `dole_per_day` default 3, suspended when the Treasury is negative; Treasury starts at 5000; M1 scenario allows ≤ 5 starvation deaths |
| 4 | A daily Warehouse → Market auto-transfer drained the reserve before Winter, making the acceptance lever a no-op | Auto-transfer removed; only `ReleaseReserve` moves reserve food |
| 5 | Workers never collected wages because `Earn → has_coins` was already satisfied; everyone quit at day 7 | `Work` goal state is `{shift_done, has_wage_due: false}`; `Earn` targets a new `has_savings` key (7 days of food); `PLAN_TIMEOUT_TICKS` raised to 900 |
| 6 | Statistical workers accumulated fractional `days_unpaid` into a `u8` and were never paid | Statistical shift end pays wages and dole directly |
| 7 | Extortion proceeds were specified three incompatible ways | Actor's wallet, then `SplitLoot` sends 50% to the gang treasury; Building table updated |
| 8 | `JoinGang` gate in the goal table omitted the empty-gang bootstrap | Gate now defers to Gang › Eligibility |
| 9 | Betrayal was dead code because `ReportCrime` excluded gang members | `Brain.betraying` flag lifts the exclusion |
| 10 | `Household.home` could not be `None` for the homeless; Sleep required Household | `Option<EntityId>`; Sleep requires only Needs |
| 11 | `CrimeReport.witness` could not be `None` for Statistical thefts | `Option<EntityId>` |
| 12 | Market stock existed on two structs | `Building.stock_food` is authoritative; `Market` holds price data only |
| 13 | Durations and effects differed between the Building and Action tables | Action table declared authoritative |
| 14 | `ServeTime`, `GuardJail`, `CollectDole` were used but not in `ActionKind` | Added |
| 15 | `is_dark` row was truncated | Restored |
| 16 | Camera position changed LOD, so replays diverged from app runs | `SetView` is a logged `PlayerCommand` |
| 17 | Weekly `floor(due × tax_rate)` was always 0 on daily wages | `Job.tax_accum` carries fractions |
| 18–19 | Think slot defined two ways; sentenced agents both evaluated goals and had a forced plan | One slot formula; sentenced agents skip utility, the law system feeds them |
| 20 | 12 plans × 200 expansions could not fit the 125 µs tick target | Expansion budget of 600 per tick; searches resume next tick |
| 21 | Memory decay ran after the systems that fill memory | Decay moved to right after needs |
| 22 | Sleep's curve won every Evening, emptying the Bar | Logistic midpoint 0.75, Evening factor 0.7 |
| 23–26 | Idle produced an empty plan; symbolic effects needed numeric state; target binding and Flirt's effect were unspecified | Idle bypasses the planner; `coin_bucket`/`food_count` added to `WorldState`; target-binding paragraph added; Flirt's planning effect is deterministic |
| 27–28 | Two plan representations; many fields introduced only by usage | `Brain.plan: Option<Plan>`; fields added to the schema structs and `World` |
| 29 | No map supplied and no road-adjacency rule | Door must open onto Road; a generator recipe is specified and committed with its output |
| 30 | Event log was both drained and a ring | Ring of 5,000, never drained |
| 31–33 | Worked-trace node counts and Beg cost; Statistical eat paid nobody; two test tolerances too tight | Corrected |
| 34–41 | Minor naming and wording | Corrected where found; the agent should treat the Action table, Gang section and Data schema as authoritative on any remaining conflict |

Still open, deliberately: the economy's first 30 days are tuned on paper only. The M1 scenario is the gate; if it fails, adjust `dole_per_day`, `farm_yield_base` and `treasury_initial` in that order before touching anything else.
