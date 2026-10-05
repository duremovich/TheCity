# M11: Ownership — corps, rent, businesses, classes

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md` and `M10_SCALE.md`. M11 is the first systems milestone of the cyberpunk direction agreed on 2026-10-05: a corporate dystopia with meatspace and Virt as two planes, vehicles and chrome as owned assets with upkeep, districts that decay into litter and open warfare, and a living economy in which corps rise, fall and monopolise. M11 lays the spine the rest hangs on: **a faction owning assets that make money and cost upkeep**. Where this document and the earlier ones disagree, this one wins for M11.

Roadmap agreed on 2026-10-05 (each a later spec):

| Milestone | Scope |
| --- | --- |
| M10 Scale | 2,000 residents on a 256 × 192 map, tier-indexed agent lists, the Statistical tier producing crime, violence and courtship at calibrated rates, deferred attribution of off-screen crimes, trace rings and biographies. `M10_SCALE.md`. |
| **M11 Ownership** (this) | Theme labels, a larger generated map with vacant lots, building ownership, wages and revenue through owners, rent and eviction, three asymmetric corps with a brain, bankruptcy and acquisition, NPC-founded businesses, per-class loyalty and submission, immigration coupled to happiness. |
| M12 Districts | District aggregates (law coverage, control, litter, class), per-district law allocation and private security, litter as an event-driven tile state with cleanup jobs, bystanders in brawls, riots and strikes. |
| M13 Assets | Owned assets with upkeep and repossession: vehicles (motorcycle, car, truck, flyer) as GoTo cost multipliers and haul capacity, chrome at a Ripperdoc as skill modifiers, stims and addiction, the security robot. |
| M14 Data and Virt | A second plane with portal pathing, nodes instead of buildings, decks as movement tiers, ICE as guards, Data as a faction resource produced by Labs and stolen through Virt, a three-track tech tree gating asset tiers. |

Scale: 2,000 residents on a 256 × 192 map, from `M10_SCALE.md`; every number here assumes it has landed.

Decisions taken with Dylan on 2026-10-05:

| Question | Decision |
| --- | --- |
| How many corps | Eight at seed, in direct competition inside each niche so no one corp drifts to monopoly by default: three Food, three Housing, two Security (the Arasaka/Militech pair), and two of the eight are conglomerates in two niches. New corps form by incorporation. |
| What a corp is | A business with a brain. A business is a building with an owner, a purse, employees and a revenue. A gang already is this shape. |
| Who can own | Agents, gangs and corps; `None` is the city. Founding a gang and founding a business are the same code path so the later player character gets both for free (see `thecity-player-character-later`). |
| Order of work | Ownership and corps (M11), districts and litter (M12), assets and vehicles (M13), Data and Virt (M14). Vehicles need the asset model; Virt needs Data to steal. |
| Map | Generated and larger (M10), with zoned blocks and vacant lots. The v1 rule that generated maps add nothing held only while nothing could be built. |
| Performance | Everything new is daily, hourly or event-driven. Nothing new runs per agent per tick. |

Decisions taken by the implementing agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| The niches | Three, each with several corps in it: **Food** (Vat Farms and Street Markets), **Housing** (Blocks and rent), **Security** (Security Offices and guard contracts). A corp holds a set of niches; a conglomerate holds two. Names are placeholders in config. |
| One brain, three parametrisations | Seven orders shared by every corp: `Grow`, `Squeeze`, `Undercut`, `Acquire`, `Secure`, `Hunker`, `Lobby`. The price lever each order pulls depends on the niche (markup, rent, contract price); a conglomerate scores per niche and acts on the one that scored. |
| Code identifiers | Not renamed. `BuildingKind::Farm` stays `Farm` in Rust; `label()` returns "Vat Farm" for the UI, events and CSV. Renaming 9k lines of identifiers is churn with no behaviour. New kinds get cyberpunk names directly. |
| Wages | Paid from the employer's purse at the workplace (`CollectWage` at `LocationKey::Workplace`), the Hall only for city jobs. The plan shape is unchanged; only the location and the purse differ. |
| Rent | A daily deduction at `tick_of_day == 0`, not an action, so every LOD tier pays it the same way and the parity test is unaffected. |
| Monopoly counterforces | Three, so a leader does not run away: rivals `Undercut`, the player can `BreakUp` a monopoly into two corps, and a corp treasury above `[corps] hoard_heat` counts as a raid prize for the gangs (Songs of Syx's hoarding rule; the raid itself is M12, the gang brain's `prize` term reads it now). |
| Classes | Derived daily, never stored on the agent: Corp (employed by a corp or its exec), Street (housed, not Corp), Dreg (homeless). Aggregates live on `World::classes`. |
| Riots | M12, with districts. M11 stops at aggregates, strikes and emigration, because a riot is spatial and needs a district to happen in. |
| Foundable kinds in M11 | Bar and Home only. With food as the only good, these are the two that make money. More kinds arrive with more goods in M13. |

## Goals and acceptance

The city should read as a market with owners: eight corps with intent, landlords evicting, NPCs opening bars and renting out Homes, one of the corps swallowing or losing something, and the law for sale to the highest bidder. The target failure spiral, written down the way Songs of Syx documents its own: **rent hike → evictions → gang recruitment → raids and theft on corp buildings → food price spike → strike → the food corp buys a Crackdown**. Concretely, on seed 42 over 120 days:

- every corp changes order at least once; `Squeeze` is held at least once; at least one `Lobby` resolves to a `Bribe` event with a corp as payer;
- at least 10 `Evicted` events; at least 3 `GangJoin` events whose recruit was evicted within the previous 14 days (the spiral's first link);
- at least one `Founded` event (an NPC opens a business), at least one `Incorporated` event, and at least one `Acquired` event from a hostile acquisition (buyer and seller both corps);
- at least one `Undercut` order held; no niche reaches a monopoly (`share == 1`) before day 60;
- at least one `Strike` event; immigration per week moves with Street happiness (not constant across the run);
- Assault events per day stay ≤ 6.4, starvation deaths and population stay within the v1 bounds scaled to the new population, and the v1 acceptance, M8 factions, M9 law and Full-vs-Statistical parity tests still pass on the new map;
- throughput stays ≥ 8k ticks/s with the Statistical tier.

## 1. Theme labels

`BuildingKind::label()`, `Role::label()` and `Crime::label()` return display names; every UI string, event message and CSV header uses them. Rust identifiers do not change.

| Kind | Label | Kind | Label |
| --- | --- | --- | --- |
| Home | Block | Hall | Civic Hall |
| Farm | Vat Farm | Hideout | Hideout |
| Market | Street Market | Warehouse | Reserve Depot |
| Bar | Bar | Cemetery | Recycler |
| Jail | Precinct | Lot (M10) | Lot |
| SecurityOffice (M10) | Security Office | | |

Coins are labelled "¢" in the UI. `assets/names.txt` gets a cyberpunk pass (handles and street names alongside given names); gang and corp names come from config. The README's first paragraph is rewritten for the new direction and links the roadmap table above.

## 2. Map

The map, zones, tiers, Lots, Security Offices and the 2,000-resident seeding are M10's (`M10_SCALE.md` § 2). M11 gives the inert pieces their behaviour: a `Lot` becomes buildable (§ 6), `tier` scales rent (§ 4), a Security Office employs private guards (§ 5). Nothing in the generator changes.

## 3. Ownership

### Owner and purse

`Building.owner: Option<EntityId>` keeps its type and gains meaning: `None` is the city; `Some(e)` is an agent (a `Wallet`), a gang entity (`Gang.treasury`) or a corp entity (`Corp.treasury`). One helper pair resolves it:

```rust
impl World {
    /// The coins behind an owner: an agent's Wallet, a gang's or corp's treasury, or the city Treasury.
    pub fn purse(&self, owner: Option<EntityId>) -> i64;
    pub fn purse_add(&mut self, owner: Option<EntityId>, delta: i64);   // may go negative for corps and the city
    pub fn owner_label(&self, owner: Option<EntityId>) -> String;
}
```

Initial ownership is dealt from `[corps]` (§ 5): the three Food corps split the twelve Vat Farms 6/4/2 and own one Street Market each; the three Housing corps own 300 of the 400 Blocks (all 60 Spire Blocks go to the largest); the two Security corps own one Security Office each; one conglomerate is Food + Housing (the 4-farm corp also holds 60 Blocks), the other Security + Housing (the smaller Security corp also holds 40 Blocks). Two of the three Bars are owned by named agents (the first business owners, seeded with `[corps] bar_owner_coins`); the third by the largest Food corp. The Hideouts are owned by their gangs; Civic Hall, Precinct, Recycler, Reserve Depot and the remaining 100 Blocks are city-owned.

### Money through owners

| Flow | Before M11 | M11 |
| --- | --- | --- |
| `BuyFood` payment | Treasury | the owner of the Market the agent buys at; `LocationKey::Market` resolves to the nearest Street Market for Coarse and Statistical agents and to the cheapest within 24 tiles for Full agents, so an `Undercut` wins customers |
| `Drink` 2 coins | sink | the Bar owner's purse |
| Wages | Treasury at the Hall | the employer building's owner's purse, collected at the workplace at shift end (`LocationKey::Workplace`); city jobs still collect at the Hall |
| Farm → Market haul | free | a Farm hauls to its owner's nearest Market; a Farm whose owner has no Market sells to the nearest Market at `wholesale` per unit, paid by that Market's owner |
| Jail food | Treasury pays the Market | unchanged (the city buys from the Market owner) |
| Rent | — | resident's Wallet → Home owner's purse, daily (§ 4) |
| Security contract | — | client's purse → security corp, daily per guard (§ 5) |
| Tax | wages only | wages and every owner revenue above, `tax_rate` withheld to the Treasury at the moment of the flow |

`collect_wage` takes the purse from the employer; a corp whose purse is negative pays nothing, exactly as the Treasury does today, so `Unpaid` memories, quitting and the loyalty drift all keep working. `WorldState.has_wage_due` and the Work goal are unchanged.

### Hiring

`World::vacancies` already posts open roles per building. M11 adds the owner's side: a corp's `Grow` order opens vacancies; `Hunker` closes them and fires the newest hire per building per day (`Fire` event). An agent-owned business hires through the same table with no brain: a Bar with fewer than `[buildings] bar.staff` bartenders posts one vacancy.

## 4. Rent and housing

| Field | Where | Meaning |
| --- | --- | --- |
| `tier: u8` | `Building` | 0 Sump, 1 standard, 2 Spire. |
| `rent_per_day: i64` | `Building` (Home) | Set by the owner: `[rent] base[tier]` for the city, the housing corp's `rent_level × base[tier]` (§ 5), the founder's choice for an agent landlord (`base[tier]`). |
| `arrears: u8` | `Household` | Days of rent unpaid. |

Daily at `tick_of_day == 0`, before the economy's price step, every housed adult pays `rent_per_day` from their Wallet to the owner's purse. City-owned Homes charge `base[tier]`; a city Home's rent is a lever (`SetCityRent`). A resident who cannot pay in full pays what they have, `arrears += 1`, remembers `RentShort` (salience 0.4, valence −0.4). At `arrears ≥ [rent] evict_days` the owner evicts: `Household.home = None`, the Home's occupant slot is freed, `Evicted` event, memory `Evicted` (salience 0.8, valence −0.8), `WasRobbed`-style affinity −0.3 to the owner if the owner is an agent. Children leave with a parent. Spouses are evicted together. An eviction pushes `Shock::Evicted` on the Street class aggregate (§ 7). The city never evicts while `levers.no_city_evictions` is set.

**Re-housing**, daily after rent: every homeless adult with `coins ≥ 3 × rent` of some Home with a free slot and `arrears == 0` moves in, nearest first by Manhattan distance from where they stand; an agent with an unpaid eviction from the same owner within 30 days is refused by that owner. A Home's `tier` scales the Sleep safety bonus (`+0.2 / +0.3 / +0.4`) and Street-tier Homes take `spoilage_pantry × 1.5`.

The dole stays at 3 and `[rent] base` defaults to `[1, 2, 4]`, so a Dreg on the dole can afford tier 0 and nothing else; the spread is what calibration tunes.

## 5. Corps

### Corp component

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corp {
    pub name: String,
    pub niches: BTreeSet<Niche>,      // one or two of Food, Housing, Security
    pub treasury: i64,
    pub exec: Option<EntityId>,       // the agent whose Personality the brain reads; None = a default
    pub buildings: Vec<EntityId>,     // sorted; redundant with Building.owner, kept for O(1) listing
    pub order: CorpOrder,             // default Hunker
    pub order_niche: Option<Niche>,   // the niche the order acts in (conglomerates)
    pub order_since: Tick,
    #[serde(skip)] pub order_trace: Vec<CorpOrderScore>,
    #[serde(skip)] pub shocks: Vec<CorpShock>,
    pub price_level: BTreeMap<Niche, f32>, // per niche: Food markup, Housing rent_level, Security contract price; 0.5..=2.0, default 1.0
    pub cashflow: VecDeque<i64>,      // net coins per day, last 14
    pub loss_log: VecDeque<(Tick, i64)>, // crime losses on owned buildings, last 14 days
    pub negative_since: Option<Tick>,
    pub contracts: Vec<(EntityId, Tick)>, // Security only: (client building, until)
    pub lobby_until: Option<Tick>,
    pub last_acquisition_tick: Option<Tick>,
}

pub enum Niche { Food, Housing, Security }
pub enum CorpOrder { Grow, Squeeze, Undercut, Acquire, Secure, Hunker, Lobby }
pub enum CorpShock { Robbed(i64), Extorted, EmployeeKilled, Bankrupt(EntityId), Strike, Undercut, BuildingLost }
```

`World::corp: Vec<Option<Corp>>` is a new component store (wired through the `components!` macro). `World::corps()`, `corp_of_building(b)`, `corp_of_agent(a)` (exec or employee), `exec` is the agent chosen at seeding (the highest-greed agent without a Job per corp) and later by incorporation.

### Seeding (`[corps]`)

```toml
[corps]
# placeholders; one row per corp, niches as a list, holdings as counts per niche kind
names    = ["Nutrix", "Vatra", "Greenline", "Habitat", "Stackwell", "Kessler", "Arasaka", "Militech"]
niches   = [["Food"], ["Food", "Housing"], ["Food"], ["Housing"], ["Housing"], ["Housing"], ["Security"], ["Security", "Housing"]]
farms    = [6, 4, 2, 0, 0, 0, 0, 0]
markets  = [1, 1, 1, 0, 0, 0, 0, 0]
blocks   = [0, 60, 0, 120, 60, 20, 0, 40]
offices  = [0, 0, 0, 0, 0, 0, 1, 1]
treasury_initial = [6000, 5000, 2500, 8000, 4000, 1500, 4000, 3000]
bar_owner_coins = 200
hoard_heat = 5000                  # a corp treasury above this is a raid prize in the gang brain
acquire_premium = 1.2              # offer = value × premium
acquire_cooldown_days = 10
undercut_floor = 0.6               # price_level cannot be undercut below this
hysteresis = 0.10
shock_severity_rethink = 0.5
bankrupt_days = 14
incorporate_buildings = 2         # an agent owning this many becomes a corp
found_cost = { bar = 300, home = 400 }
wholesale = 2                      # coins per unit Farm → Market when owners differ
contract_per_guard_day = 10
security_guards = 6
lobby_min_treasury = 400
monopoly_markup_cap = 2.0          # price_level ceiling with a monopoly; 1.5 without
```

### The corp brain (`systems/corp_brain.rs`)

Same template as the gang and law brains: daily rescore at `tick_of_day == 0` with hysteresis, an immediate rescore when pending shock severities sum past the threshold, scores via `utility::curves`, trace kept for the inspector. Severities: `Robbed` 0.3 (0.6 above 100 coins), `Extorted` 0.4, `EmployeeKilled` 0.5, `Bankrupt` of a rival 0.5, `Strike` 0.8, `Undercut` 0.3.

Inputs, once per rescoring:

| Symbol | Definition |
| --- | --- |
| `cash` | `treasury / treasury_initial`, clamped to 2 |
| `flow` | mean of `cashflow` ÷ `max(daily wage bill, 1)`, clamped −1..1 |
| `losses` | crime losses in 14 days ÷ `max(treasury, 1)`, clamped to 1 |
| `demand` | Food: Market sales ÷ stock over 7 days; Housing: occupancy of owned Homes; Security: contracted guards ÷ `security_guards` |
| `share` | per niche: Food = this corp's Market sales ÷ all Market sales over 7 days; Housing = owned occupied Blocks ÷ all occupied Blocks; Security = contracted guards ÷ all contracted guards (1.0 is a monopoly) |
| `rival_price` | the lowest `price_level` among rivals in the niche |
| `weakest` | the rival in the niche with `negative_since` set or `cash < 0.3`, if any, and the value of its cheapest building |
| `lots` | vacant Lots the corp can afford at `found_cost` |
| `unrest` | the Street class's `unrest` (§ 7) |
| `E` | exec's `greed`, `courage`, `lawfulness`; a corp without an exec uses 0.5 |

A conglomerate scores the table once per niche and takes the best `(order, niche)` pair; `order_niche` records which.

| Order | What it does daily while held | Considerations (input → curve) |
| --- | --- | --- |
| Grow | Buys the nearest Lot to an owned building and builds the niche kind (Home for Housing, Bar for Food as a second revenue, a second Security Office is never built); opens vacancies to full staff | `Can(lots > 0 ∧ cash ≥ 0.5)` → Step{1,0,1}; `demand` → Linear{0.8,0.2}; `flow` → Linear{0.5,0.5}; `E.greed` → Linear{0.5,0.5} |
| Squeeze | `price_level += 0.1/day` toward the cap (`monopoly_markup_cap` if `share == 1`, else 1.5); Housing also cuts `evict_days` by 1 (min 2); Food also cuts wages 10 % | `share` → Logistic{8,0.4}; `1 − flow` → Linear{0.7,0.3}; `E.greed` → Quadratic{2,1,0,0}; `1 − unrest` → Linear{0.6,0.4} |
| Undercut | `price_level −= 0.1/day` down to `max(undercut_floor, rival_price − 0.1)`; rivals in the niche get `CorpShock::Undercut` when their share drops 0.1 in a week | `Can(rivals in niche > 0 ∧ cash ≥ 0.4)` → Step{1,0,1}; `1 − share` → Linear{0.8,0.2}; `rival_price − own price_level` clamped 0..1 → Linear{0.5,0.5}; `flow` → Linear{0.4,0.6} |
| Acquire | Offers `value × acquire_premium` for the weakest rival's cheapest niche building; the rival accepts when `negative_since` is set or `cash < 0.3`; the building, its employees and contracts move; `Acquired` event; `CorpShock::BuildingLost` on the seller; one per `acquire_cooldown_days` | `Can(weakest exists ∧ treasury ≥ offer ∧ cooldown passed)` → Step{1,0,1}; `cash` → Linear{0.7,0.3}; `E.greed` → Linear{0.6,0.4}; `1 − share` → Linear{0.4,0.6} |
| Secure | Buys a security contract for each owned building without one, newest losses first, at `contract_per_guard_day` | `losses` → Logistic{8,0.1}; `cash` → Linear{0.5,0.5}; `E.courage` → Linear{0.3,0.7} |
| Hunker | `price_level` drifts to 1.0; closes vacancies; fires one per building per day down to half staff; cancels contracts | `1 − cash` → Logistic{6,0.6}; `1 − flow` → Linear{0.6,0.4}; flat 0.15 base so a quiet corp can always choose it |
| Lobby | Pays `bribe_price()` to the captain for a Crackdown on the gang that robbed or extorted it most in 14 days, through `faction::consider_bribe`'s machinery with the corp as payer; `lobby_until = now + bribe_days` | `Can(treasury ≥ lobby_min_treasury ∧ lobby_until passed ∧ a culprit gang exists)` → Step{1,0,1}; `losses` → Linear{0.9,0.1}; `1 − E.lawfulness` → Linear{0.6,0.4} |

Security corps' guards are `Role::Guard` with `employer = Security Office`. `law_brain::guards()` returns city guards only (employer kind Jail); private guards get their own patrol loop over contracted buildings and respond to crimes there exactly as city guards do (arrest, escort to the Precinct), but never take Jail duty. A contract ends when the client's purse cannot pay, or when a rival Security corp offers the client a lower `contract_per_guard_day × price_level` at renewal (contracts renew weekly); the shock is `Undercut` on the loser.

Lobby reuses M9 bribery end to end: the captain's `incorruptible` check, `hardened_until`, the `Bribe` event. A corp bribe sets `Law.target` to the culprit gang and the gang's `bribe_until` is untouched (the gang can out-bid next day, which is the war the design wants).

### Bankruptcy, acquisition, monopoly

- `negative_since` is set when `treasury < 0` and cleared when it recovers. At `bankrupt_days`: `Bankrupt` event; each owned building goes to the highest purse among the other corps and agents with `coins ≥ value(kind)` (value = `found_cost` for Bar and Home, 1000 for Farm and Market, 500 for the Security Office), else to the city at `value / 2` (the Treasury pays, which may push it negative); employees keep their jobs under the new owner; the corp entity is despawned and its exec drifts `greed +0.05`. `Acquired` event per building. Every other corp gets `CorpShock::Bankrupt(id)`.
- A corp with `share == 1.0` in a niche is a **monopoly** there: `monopoly_markup_cap` applies and the City panel says so. The player's `BreakUp` lever splits a monopoly's niche buildings in half into a new corp with a fresh exec (the second-greediest employee), `BrokenUp` event.
- A corp whose treasury exceeds `hoard_heat` is listed as a raid prize to the gang brain (`prize` in M8's Raid order reads the richest such corp as an alternative target). Raiding a corp building is specified in the districts milestone; until then the term only tilts gangs toward `Contest` of that corp's Blocks.
- A corp with every building sold or demolished is dissolved the same way.

## 6. Founding

A new goal, **Found**, for adults without a Job or with `wage_per_day ≤ dole`, not GangMember:

| Considerations | GOAP goal state |
| --- | --- |
| `Can(coins ≥ found_cost(kind) for some foundable kind ∧ a Lot exists)` → Step{1,0,1}; `greed` → Quadratic{2,1,0,0}; `U(wealth)` → Linear{0.3,0.7}; `lawfulness` → Linear{0.6,0.4}; `Can(not in shift)` → Step{1,0.3,1} | `{founded: true}` |

Plan: `GoTo(Hall) → Register`. `Register` (new `ActionKind`, dur 30, cost 20) picks the kind the agent can afford that has the fewest instances per capita (Bar vs Home), picks the Lot nearest the agent's Home (tie: lower index), pays `found_cost` to the Treasury, converts the Lot into a built `Building` of that kind with walls stamped onto the map (the flow fields for every building are invalidated), sets `owner = Some(agent)`, posts its vacancies and logs `Founded`. A founded Home charges `base[tier]` rent and the founder keeps their own Home. The founder of a Bar does not work there; it needs a bartender hired through the vacancy table.

**Incorporation.** An agent who owns `incorporate_buildings` or more becomes a corp: a `Corp` entity spawns with `niches` from the kinds owned (Bar → Food, Home → Housing), `exec = agent`, `treasury = 0`, the buildings move to it, and from then on revenue goes to the corp treasury and the exec draws a wage of `[economy] wage_exec` from it. `Incorporated` event. Founding a gang is the existing bootstrap path in `gang::join`; M11 does not change it, but `Register` and `JoinGang`'s bootstrap are the two "found a faction" entry points the player character will later call directly.

## 7. Classes

`World::classes: [ClassAggregate; 3]` indexed by `Class { Corp, Street, Dreg }`, recomputed daily after rent, before the brains:

```rust
pub struct ClassAggregate {
    pub count: u32,
    pub happiness: f32,   // mean (mood + 1) / 2
    pub employment: f32,  // employed adults / adults
    pub fear: f32,        // guard-hours within 6 tiles of this class's Homes last day ÷ count, clamped to 1
    pub loyalty: f32,     // happiness × min(employment + 0.5, 1) ; capped by employment, as Syx does
    pub submission: f32,  // fear-driven: 0.3 + 0.7 × fear
    pub unrest: f32,      // (1 − loyalty) × (1 − submission) + 0.1 × evictions_7d / count
    pub evictions_7d: u32,
    pub trace: Vec<(&'static str, f32)>,  // not saved
}
```

Class membership is derived: Corp = employed by a corp-owned building or an exec; Dreg = `Household.home == None`; Street = everyone else. Consequences, all daily:

- **Strike.** When Street `unrest > [classes] strike_threshold` (0.6), every Street worker of the corp with the highest `price_level` skips tomorrow's shift (`Job.last_shift_day` is advanced; no wage accrues), `Strike` event once, `CorpShock::Strike` on that corp. A strike cannot repeat within 7 days.
- **Immigration** per week becomes `levers.immigration_per_week × f(happiness)` where `f` is Logistic{8, 0.5} over Street happiness (Syx opens immigration above 80 %); the lever is the ceiling.
- **Emigration.** A Dreg with mood < −0.5 for 7 days and no Job leaves the city at the map edge (`Emigration` event, existing kind). A Corp-class agent never emigrates.
- **Fear** feeds `submission`; a crackdown or a Security contract near the Sump raises it, which is the Garrison-state equilibrium: unhappy but quiet. The M12 riot reads `unrest`; M11 only logs it.

## 8. Player levers

| Command | Effect |
| --- | --- |
| `SetCityRent(u8 per tier)` | Rent on city-owned Homes. |
| `SetRentCap(Option<i64>)` | A ceiling on any Home's rent; corps at the cap cannot Squeeze rent further. |
| `Nationalise(EntityId)` | The city buys a building at `value(kind)` from its owner (refused when the Treasury cannot pay). |
| `Subsidise { corp, amount }` | Treasury → corp treasury. |
| `NoCityEvictions(bool)` | The city never evicts. |
| `BreakUp(EntityId)` | Split a monopoly in a niche into two corps (§ 5). Refused when the corp is not a monopoly anywhere. |

All go through `PlayerCommand` and the command log as before.

## 9. UI and events

New `EventKind`s: `Evicted`, `RentShort`, `Founded`, `Incorporated`, `Bankrupt`, `Acquired`, `BrokenUp`, `CorpOrder`, `Strike`, `Contract` (corp orange).

- **Building panel**: owner (link; corp, gang, agent or "city"), rent and arrears per resident for a Home, revenue last 7 days, security contract and its guard, tier.
- **Corp panel** (new, from a building's owner link or the City panel): niches, exec, treasury, cashflow sparkline (14 days), order and since, the order trace (top three), `price_level`, buildings, employees, contracts, monopoly flag, bankruptcy countdown.
- **City panel**: a Corps section (one row per corp: name, niches, order, treasury, price levels, buildings) and a per-niche share bar, a Classes section (three rows: count, happiness, loyalty, submission, unrest), the new levers.
- **Inspector**: class, employer and owner, rent paid and arrears, "founded X" and "exec of Y".
- **Map**: a building's owner colours its outline (corp colours from the corp index, gang colours as in M8, city grey, agent white); Lots dotted; tier 2 Homes drawn a shade brighter.
- **CSV** (`--report`): columns for each corp's treasury and order, evictions, foundings, each class's unrest.

## 10. Save compatibility

All new fields are `#[serde(default)]`; `migrate_legacy` gives a pre-M11 save a `Corp` store with no corps, city ownership everywhere, rent 0 and empty classes, so every rule above degrades to M9 behaviour. A save made on the v1 map loads with its own dimensions because `Map` carries them. `LocationKey::Workplace` resolves to the Hall for a city job, so an old plan mid-flight still completes.

## 11. Testing and calibration

**Unit tests** (`citysim/tests/ownership.rs`, `citysim/tests/corps.rs`): a purchase pays the Market's owner; Drink pays the Bar owner; a corp with a negative treasury leaves a worker unpaid and they quit at 7 days; rent moves coins and short rent counts arrears; eviction at `evict_days` frees the slot and the evictee re-houses when they can pay; the brain picks Grow with demand and lots, Squeeze with a dominant share and a greedy exec, Undercut with a small share and a pricier rival, Acquire against a rival in the red, Secure after a big robbery, Hunker when broke, Lobby after extortion with a lawless exec; a conglomerate acts in the niche that scored; an acquisition moves the building, its employees and contracts; BreakUp halves a monopoly; a Strike shock forces a rescore; bankruptcy sells to the richest buyer and to the city when nobody can pay; a monopoly raises the markup cap; Register converts a Lot and stamps walls; owning two buildings incorporates; class aggregates compute the documented numbers on a hand-built world; a private guard arrests at a contracted building and never takes Jail duty; a pre-M11 save loads.

**Scenario** (`citysim/tests/scenario.rs`, new `#[ignore]` 120-day test on seed 42): the bullets under *Goals and acceptance*. The v1, M8 and M9 gates are re-pointed at the new map with population bounds scaled by 2000/300.

**Calibration** targets, tuned via `[rent]`, `[corps]` and `[classes]` only: evictions 10–40 per 120 days; Street unrest between 0.2 and 0.7 most days; one to three bankruptcies or acquisitions per 120 days on seed 42; no niche at monopoly before day 60; the food price stays within the v1 band in Summer. Re-run `calibrate` (Register is a new action, CollectWage's location changed, and the new map changes every distance).

**Throughput**: rent is O(agents) daily; the brains are O(buildings) daily; class aggregates are O(agents) daily; the fear term reuses the guard-patrol safety pass that already walks guards hourly. Nothing new per tick. The larger map lengthens walks, so the ticks-per-plan calibration will shift; that is expected and is what `calibrate` is for.

## 12. Phases

1. **Labels**: § 1, the README rewrite, the roadmap table. Commit.
2. **Ownership and rent**: § 3 and § 4 with the eight corps seeded as inert owners (no brain), the Bar owners, levers. Commit.
3. **The corp brain**: § 5 including Undercut, Acquire, bankruptcy, monopoly and BreakUp, private guards and Lobby. Commit.
4. **Founding and classes**: § 6 and § 7, strikes, immigration coupling, emigration. Commit.
5. **Gate and polish**: § 9 panels and CSV, the M11 scenario, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

This is a five-phase milestone, larger than M8 or M9; phase 2 is a gate on its own and is worth watching live in the app before phase 3 starts.

## 13. Out of scope (M12 and later)

Districts and anything per district (law allocation, litter, control), riots and crossfire, raids on corp buildings, assets with upkeep (vehicles, chrome, robots), a second good, Virt and Data, corp wars beyond Lobby, Undercut and Acquire, corps hiring gangs, a corp buying a gang's Hideout, trials and fines, and the player character.
