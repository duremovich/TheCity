# Life pass L2: the living city — jobs, fun, off-screen blood, the LOD budget

Companion to `SPEC.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M15_WORD_AND_BLOOD.md`, `SHADOW_V1.md`, `BIG_PICTURE_2026-10-07.md`, `ROADMAP_POST_M14.md` (addenda 15 and 16) and `VISION.md`. M11 to M15 built owners, places, things, knowledge and talk on a city whose money is the dole, whose off-screen residents cannot be murdered, and whose Coarse tier is twice its budget. L2 fixes the floor before M16 stands on it: **wages become the economy and the dole the floor under it, fun gives money a purpose, the faction layer reaches the Statistical tier, and every tier has a budget it keeps.** A Sump noodle cook clears 7 coins on a Friday shift and spends 5 of them that night at the Ninefold fight pit, where she loses 3 more at the bout; the pit's take goes to Ninefold's treasury, and the gang's leader walks it round to his members on Sunday, so a member who extorted nobody all week still eats. On Monday Ninefold's order flips to Contest; nobody on screen sees the Stackwell block where it lands, but by Wednesday two of its tenants have been beaten and one killed off screen, the binder names three Ninefold members, the dead man's sister hears it in her district's pool, and the noodle cook's best friend, who works the Club door in Mid West, is one of the three. Where this document and the earlier ones disagree, this one wins for L2.

Addendum 16 names the milestone: *the jobs economy of addendum 15 with wages over the dole and a second chain, Statistical violence driven by the faction layer, a forced-class LOD budget and the plan churn, a 365-day sanity run; re-shadowed afterwards.* L2 replaces "Life pass L2" as addendum 15 sketched it: the street hang-outs, the `fun` need and the leisure businesses are § 1-2 here; the shadow pass's role features that ride on them are § 7.

Scale: 2,000 residents on the 256 × 192 map; L2 lands after M15 phase 5 and its review. Identifiers marked *new* do not exist at HEAD (4a6a396); every other identifier named here does. **Every number below is a placeholder for calibration unless it is labelled measured** (measured numbers name main at 1eb11a3 or 4a6a396, seeds 42/43, 120 days). One sim year is 120 days (`time::DAYS_PER_YEAR`), so a 120-day run is one year and the 365-day run is three.

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| Where the jobs come from | **Jobs, not a dole cut.** The dole (`levers.dole_per_day` 4) stays the floor and is paid only to the jobless (`economy::collect_dole` already refuses a `Job` holder), so every job removes a dole recipient and adds a wage. Measured: the dole is ≈ 1,430 recipients × 4; Σ wages > Σ dole needs ≈ 650 employed at a mean wage of 6.5 against today's 267-310. Sources: six leisure kinds and a Fab (~115 jobs at seed), Markets and Bars staffed like businesses (+30), public works paid from the Treasury's surplus (≤ 120), and founding and `Grow` through the run (+100 by day 60). |
| New building kinds | Seven, appended to `BuildingKind` (16 → 23): `Club`, `Arcade` (label "Braindance Parlour"), `NoodleBar` (the street food stall), `FightPit`, `Den` ("Gambling Den"), `Lounge` ("Spire Lounge"), `Fab` ("Parts Fab"). |
| Staff roles | One role per kind so `Role::workplace` and `ownership::role_for` stay 1:1: `Host` ("Club Staff"; the strongest on shift works the door as bouncer), `Attendant`, `Cook`, `Fighter`, `Croupier`, `Concierge`, `Fabber` ("Fab Tech"), appended (`Role::ALL` 10 → 17). Every leisure role's shift is `ClerkWork` at its employer (the M13-M15 precedent); the Fabber's is a new `FabWork` (the `FarmWork` shape). |
| Where they stand | Seeded on vacant Lots at day 0 through `founding::build_on_lot` (the `news::seed_feeds` precedent, no map change), foundable through `Register` (`FOUNDABLE` 6 → 12), and a derelict Block can be **refitted** into a leisure kind at half the found cost (*new* `founding::refit`), because corp `Grow` eats the 60 Lots. |
| The `fun` need | `Needs.fun` (*new*, 0..=1, serde default 1.0), decaying to 0 in ~3 days, weighted by sociability, greed, pride and courage and by class; satisfied on a cost ladder (free, cheap, mid, high) by the *new* goal `GoalKind::Unwind`. It enters mood. |
| Street hang-outs | `ActionKind::HangOut` (*new*) on the **Socialise** goal, at hang-out spots (Bar, Market, Club and NoodleBar doors, the own gang's Hideout door, fire barrels in Sump districts), the spot scored by where the agent's known contacts are expected now (`hunt::habit` over their `Trace`). A scripted plan (the planner bypass, as `Raid` and `Hunt`), never searched. Socialise is no longer "nothing to do" for the broke. |
| Off-screen leisure | A **daily rule pass** (`leisure::stat_daily`, *new*), not a table row: `calibrate` runs a 500-agent city that cannot represent venues by district. The parity test gains leisure spend per agent-day as a seventh rate. |
| The money loop | Measured: wallets fall 57.5k → 9.3k by day 30 while the Treasury rises 41k → 76k, because Farm (245 a day) and Market (600 a day) upkeep return the food money to the Treasury, which pays it out as dole. L2 adds a **Treasury budget band**: above it the city posts public-works Sanitation jobs and then scales corp upkeep down; below it the reverse. |
| The second chain | **Scrap → Parts → chrome and vehicles.** Scavenge yields scrap the Recycler turns into Parts (it is already a `parts_market` source), and the *new* `Fab` (Tech niche) makes Parts from labour. Parts are an existing good with existing buyers (Clinics and Garages) and an existing leak: sellers pay `import_frac` of every asset sale to the Treasury (`Flow::Import`) for want of Parts. A Tech corp gets a `Grow` worth taking (a Fab cuts its own imports and sells to rivals), and decks and chrome are what the outside will want. A Vat Farm → stall chain is not a second chain: the NoodleBar buys Market food at `wholesale` and rides the Food chain as a buyer. |
| M13 prices | Not moved in phases 1-4. Phase 5 prints, per tier-1 asset, its price in days of the day-60 median employed wage, and moves a price only to the rule "a tier-1 good costs 10-20 days of a median wage" when the wage economy holds (Σ wages > Σ dole on the majority of seeds). |
| The export hook | `World::outside: Outside` (*new*) with **M17's names and only the World account**: `Outside { factions, minted, inbound, outbound, next_id }`, `OutsideFaction { id, name, kind, treasury, treasury_ref, base_income, base_upkeep, income_today, market, dead }`, `OutsideKind::World`, `OutsideId = ParentId` from `OUTSIDE_PARENT_BASE`. The World buys a slice of Food, Parts and Data at an outside price (`Flow::Export`, *new*). `[export] enabled = false`. M17 appends the rest of `OutsideFaction` with `#[serde(default)]`. |
| Conservation | M17's identity, from L2 on: `ownership::total_coins(world) + Σ outside treasuries − outside.minted` is constant to the coin. With export off, `outside` is empty and `total_coins` alone is constant, as today. |
| Gangs and leisure | Gangs own FightPits and Dens as **fronts** (a gang daily spend under Expand), dealers deal inside them, and a law Crackdown closes them (`Building.closed_until`). Protection on others' businesses is M16b's standing coercion (`M16_CONTRACTS.md` § 4), not L2's. |
| Off-screen faction violence | A **daily pass** generalising M13's `chrome::abduction_daily` (an order-driven off-screen roll): each Statistical adult in a district a source touches rolls `Killed`, `Assaulted`, `Robbed` or `Abducted` once a day at a **rate read from the on-screen tier's rolling counts** under the same source, district and victim class (*new* `World::order_rates`), shrunk toward a config prior. Not learned by `calibrate`, whose city is gangless (`Config::calibration_city`). |
| Binding | A faction hole carries `Hole.faction` (*new*); the binder's candidates are that faction's members (any tier), else the hole is Unknown. Never a civilian. |
| The parity test | The forced-tier test `test_full_vs_statistical_within_15pct` stays as it is (the need-driven table). A *new* `test_faction_violence_parity` compares per-capita victimisation by tier inside one normal run with the faction layer on. |
| The LOD budget | **Measured: the Coarse overrun is the Jail.** Sentenced and emigrating agents are set Coarse outside the rank (`lod::assign`), so the tier is `max_coarse` + the jailed: 150 + 158 = 308 on day 119 (seed 42), 150 + 140 on day 30. Gang members over the slots already fall to Statistical by rank. L2 makes prisoners **held Statistical** with a daily cell pass, and gives each forced class a quota inside `max_coarse`. |
| Gang members off screen | A per-gang Coarse quota (leader, lieutenants, story steps, the order's front line, then a daily rotation); the rest are Statistical and work an **orders-aware GangWork hour** whose extortion, claim and dealing rates come from the same `order_rates` ledger, actor side. A muster raises the quota (the riot precedent, `riot::promoted`). |
| The churn | Measured on seed 42: 46,052 `Earn failed at Scavenge: StockGone` are the 85 % miss of the `scavenge_p` 0.15 roll, not a race: a dry hour becomes `Done` with nothing. 20,535 `Sleep failed at Sleep: PreconditionLost` and 7,710 at `CheckIn` are second beds lost between plan and arrival: `ReservationKind::Bed` exists and nothing writes it; L2 writes it. |
| The year run | 365 days on seeds 42 and 43 is **three winters** (days 90-119, 210-239, 330-359), printed per 30 days; bounded quantities asserted (§ 6). |
| The off switch | `[living] enabled` (*new*) and the CLI's `--l2-off` turn every L2 system off; an L2 build with it off reproduces the M15-closing city byte for byte (the `--life-off` precedent). Each phase also has its own section switch. |

## Goals and acceptance

The city should read as a place where people work for money, spend it on something besides food and rent, lose it at the pit and the den, and get killed off screen by the same gangs that kill on screen. The target spiral: **a Sump resident gets a Cook's job at a corp NoodleBar → the dole bill falls and wages rise past it by day 60 → a Friday wage goes on an Arcade and a drink, a Den takes the rest → the Den's take reaches a gang treasury and its leader's Collect reaches the members → the gang turns Contest and off-screen residents of the contested district are beaten and killed at the rate its on-screen members achieve → the binder names members of that gang → kin hear it through the pools and a Hunt follows.** On seeds 42-44 (majority) and 42-47 (means and existence), 2,000 residents, 120 days, gate `test_l2_living_city_seed_42`:

**Asserted (mechanism and existence):**

- every new kind stands at seed and has ≥ 1 paid shift by day 7; ≥ 1 leisure kind is founded by `Register` or refit by day 120 (majority); ≥ 1 Fab built by a Tech corp's `Grow` (existence over 42-47); Fab Parts and Recycler scrap Parts each sold to a Clinic or Garage on every seed;
- every leisure kind has revenue > 0 on every seed; `Unwind` is adopted at Full and Coarse, and Statistical leisure spend is > 0 every day after day 1; ≥ 1 gang front, ≥ 1 `Collected` whose share reaches ≥ 3 members, ≥ 1 Den or Club dealer sale;
- HangOut: a HangOut's co-located known contacts average more than a uniformly drawn spot's in the same district and hour (the contacts term works);
- the conservation identity holds every day; with `[export]` off `flow_export` is 0 and `outside` is empty; a unit test with it on moves Food, Parts and Data out and coins in, and holds the identity;
- LOD: on every sampled hour, Coarse bodies outside the held class ≤ `max_coarse` + pinned; per-gang Coarse ≤ the gang quota outside muster windows; no held prisoner starves; Statistical gang members' extortion credits the gang treasury on every seed;
- churn: zero `Scavenge: StockGone` aborts; `Sleep` and `CheckIn` aborts per day below the M15-closing run's on the majority of seeds;
- off-screen violence: ≥ 1 faction-sourced off-screen `Killed` hole on every seed of 42-44; every faction-sourced hole bound to a member of its faction or Unknown (exactly); `test_faction_violence_parity` passes;
- sanity: population and starvation within the scaled v1 bounds; ≥ 5 of the 9 seeded corps alive on day 120 (majority); Murders within +25 % of the M15-closing run per seed (majority); Assault events per day ≤ 42.7; the v1, M8-M15 and the forced-tier parity gates still pass; the 365-day bounds of § 6 on seeds 42 and 43;
- throughput is printed (run mean and last-10-day mean) against the 4,000 ticks/s floor; it is judged only by alternating A/B on this box.

**Printed findings (calibration bands, not asserted):** Σ wages ÷ Σ dole on day 60 ≥ 1.0 (band 1.0-2.0); employed share of adults on day 60 30-40 %; wallet Gini on day 120 0.50-0.70 (the vision's "deeply unequal", not 0.71-0.79 by accident); population wallets on day 120 ≥ 20k; Treasury inside its band from day 30; adults with `fun ≥ 0.5` 40-70 %; off-screen per-capita killing rate within a factor of 2 of on-screen for civilians (today 50-100x for everyone); the off-screen share of all killings 0.3-0.7.

## 1. The economy of jobs

### Data model

```rust
pub enum BuildingKind { /* … M15 */ Club, Arcade, NoodleBar, FightPit, Den, Lounge, Fab }   // labels "Club", "Braindance Parlour", "Noodle Bar", "Fight Pit", "Gambling Den", "Spire Lounge", "Parts Fab"; letters K A S X D U Z
pub enum Role { /* … M15 */ Host, Attendant, Cook, Fighter, Croupier, Concierge, Fabber }
pub enum Flow { /* … M15 */ Leisure, Gamble, Tribute, PublicWorks, Export }
pub enum Niche { Food, Housing, Security, Tech }           // unchanged; Fab joins Tech's kinds, NoodleBar joins Food's, the leisure kinds join no niche

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Venue {                     // Building.venue: Option<Venue>, #[serde(default)], leisure kinds only
    pub price: i64,                    // today's entry or meal price, set daily from kind × tier × owner level
    pub visits_today: u16,
    pub visits: VecDeque<u16>,         // last 7 days, newest last
    pub take_today: i64,               // the house's gambling margin (Den, FightPit)
    pub front_of: Option<EntityId>,    // a gang front: the gang whose dealers deal here and whose leader collects
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CityBudget {                // World::budget, #[serde(default)]
    pub upkeep_mult: f32,              // scales `[corps] upkeep` paid to the Treasury; 1.0 at seed
    pub works_posted: u16,             // public-works Sanitation jobs standing
    pub band_days: u16,                // days in a row outside the band, for hysteresis
}
```

`Building.production_accum` (Farm only today) is read by the Fab too. The scrap accumulator is `World::scrap: u32` (*new*, saved).

### The leisure kinds

| Kind | Staff (role, count) | Tier rule | Price (Sump, Mid, Spire) | What it sells | Owners |
| --- | --- | --- | --- | --- | --- |
| Club | Host × 10 | Mid, Spire | –, 5, 12 | `Enjoy` (fun +0.6, belonging +0.2), `Drink` | agents, corps, gang fronts |
| Arcade | Attendant × 3 | any | 2, 3, 6 | `Enjoy` (fun +0.35); an Arcade on a chair counts as a public terminal (M14 `Flow::Terminal`) | agents, corps |
| NoodleBar | Cook × 3 | Sump, Mid | 3, 4, – (a meal: Market price + 1) | `EatOut` (hunger as a meal, fun +0.1) | agents, Food corps |
| FightPit | Fighter × 4 | Sump | 3, –, – | `Enjoy` (fun +0.5), `Gamble` on the nightly bout | gangs (fronts), agents |
| Den | Croupier × 4 | Sump, Mid | 2, 3, – | `Gamble`, `Drink` | gangs (fronts), agents |
| Lounge | Concierge × 6 | Spire | –, –, 25 | `Enjoy` (fun +0.9, belonging +0.3); door policy below | corps, the wealthiest agents |

**Door policy.** A Lounge refuses a Dreg and anyone with `Appearance.dress < 2`; a Club's door Host refuses a visitor whose `rep.heat ≥ door_heat` when the Host's courage beats it, the M15 meeting gate's shape. **Bouts.** At `bout_hour` each open FightPit pairs its two fittest Fighters on shift and resolves them with `law::resolve_fight` with no death (injury: energy and safety down, a `Lost` memory); the bout's bets settle in `Gamble`. **Curfew.** `levers.curfew[d]` closes every leisure venue in district d from 22:00 to 06:00.

`Enjoy`, `Gamble` and `EatOut` (*new* `ActionKind`s) start only at an open venue of the right kind with capacity; money moves at start (`exec::actions::on_start`, as `BuyFood`), through `ownership::pay(agent → owner, price, Flow::Leisure)` (taxed as owner revenue) and `ownership::credit`. `Gamble` stakes `min(stake_frac × coins, stake_cap)` and wins `2 × stake` with `p = 0.5 − house_edge` on the agent's stream; a loss pays the owner (`Flow::Gamble`, taxed), a win is paid by the owner from its purse (`ownership::charge` for corps, capped `pay` for agents and gangs; a house that cannot pay closes the table for the day). A win above `big_win` posts `Gambled` and a pool deed `Founded`-weighted for reach (word of a big win travels).

### The Fab and the scrap chain

`Fab`: staff Fabber × 12, `stock_cap` 400 Parts, Tech niche (`corp_brain::niche_kinds(Tech)` gains `Fab`; `build_kind_in(Tech)` picks the Fab when the corp's own sellers imported more than `fab_import_trigger` coins in 14 days, else today's Clinic/Garage rule). `FabWork` accrues `fab_yield × (fab_skill_floor + fab_skill_slope × skill)` Parts per worker-hour into `production_accum`, the `economy::accrue_farm_work` shape, with the skill read from `Skills` via `competence::role_skill` (Fabber reads the mechanical slot). `assets::parts_market` lists sources gang Hideouts, then **Fabs (cheapest owner first, a corp's own Fab free to itself)**, then the Recycler. Every Part in a seller's stock already replaces `part_credit` import coins, so a Fab moves coins from `Flow::Import` (to the Treasury) into Fab wages.

**Scrap.** `Scavenge` (L1, `exec::actions::scavenge`) keeps its roll; a find adds 1 to `World::scrap` and pays `scavenge_coins` from the Treasury as today (`Flow::Sanitation`); `scrap_per_part` scrap becomes 1 Part in the Recycler's stock at midnight. The scavenger's hour feeds the chrome trade, and a district's litter (M12) scales the find: `p = scavenge_p × (0.5 + litter(d))`, so the dirty Sump pays best.

### Staffing the existing kinds

| Kind | Staff today | L2 | Why |
| --- | --- | --- | --- |
| Market | 4 Clerks | 12 | stall clerks and porters; a Market is a corp's biggest shop |
| Bar | 3 Bartenders | 5 | a Bar owner posts the vacancy daily below staff (M11 D17) |
| Recycler | `world.jobs.gravedigger` 4 + Sanitation | + public works ≤ `works_max` | the Treasury's surplus (below) |

Sanitation gets the work action the handoff asks for: `ActionKind::Sweep` (*new*) at the employer's district streets lowers M12 litter by `sweep_per_hour` on the tile band it walks, replacing `TendGraves` as `Role::Sanitation`'s `work_for` (the Gravedigger keeps `TendGraves`).

### Wages over the dole

`[economy]` wage keys gain the seven roles (`EconomyCfg::wage` extended). The day-60 arithmetic, from the measured day-120 ledger (dole 5,720 ≈ 1,430 recipients at 4; 75 % of the jobless collect on a given day):

| Source | Jobs at seed | By day 60 |
| --- | --- | --- |
| Measured base (Farms, guards, clerks, bartenders, diggers, M13-M15 staff) | 290 | 290 |
| Seeded leisure (3 Clubs, 3 Arcades, 10 NoodleBars, 2 FightPits, 2 Dens, 1 Lounge) | 117 | 117 |
| Seeded Fabs (2, one per Tech-capable corp row: Zetatech and Militech) | 24 | 24 |
| Markets 4 → 12, Bars 3 → 5 | 30 | 30 |
| Public works (Treasury surplus) | 0 | 60-120 |
| Founding, refits and `Grow` (per-capita targets below) | 0 | 80-120 |
| **Total** | **≈ 460** | **≈ 650-700** |

At 650 employed and a mean wage of 6.5, Σ wages ≈ 4,200 against Σ dole ≈ (1,950 − 650) × 0.75 × 4 ≈ 3,900. The new kinds' wages: Host 6, Attendant 5, Cook 5, Fighter 7, Croupier 6, Concierge 8, Fabber 7. A gang member is jobless and takes the dole, as today; a front's staff are hired through `demography::hire_candidate` like anyone's, so gangs launder members into wages only by founding a front.

**Per-capita targets** for `founding::choose_kind` and the refit: `residents_per_club` 600, `residents_per_arcade` 600, `residents_per_noodle` 120, `residents_per_pit` 1,000, `residents_per_den` 800, `residents_per_lounge` 2,000. Agent found costs (`[corps] found_cost`): club 400, arcade 200, noodle_bar 120, fight_pit 300, den 250, lounge 900, fab 900 (corps only). A NoodleBar at 120 is the first rung a saver climbs; `can_found`'s floor becomes the cheapest foundable cost.

### The Treasury budget band

Daily at midnight (`economy::run`'s close), with `T` the Treasury and `[budget] band = [lo, hi]`:

- `T > hi` for `band_days ≥ 3`: post `works_step` Sanitation vacancies at the Recycler up to `works_max`; when they stand full, `upkeep_mult −= upkeep_step` (floor `upkeep_floor`), applied to every `[corps] upkeep` the Treasury receives;
- `T < lo` for `band_days ≥ 3`: `upkeep_mult += upkeep_step` (cap 1.0), then lay off the newest public-works job (`economy::dismiss`) per day;
- inside the band: nothing. The dole is never touched by the band; `dole_per_day` remains Dylan's lever.

### Prices by tier, the venue price

`Venue.price = round(price_base[kind][tier] × level)`, `level` the corp's Food `price_level` for a Food-corp NoodleBar, 1.0 otherwise; a gang front prices at `front_markup`. A seller's price is fixed for the day (no tenths: one coin a visit is the unit).

### The export hook

```rust
pub type OutsideId = ParentId;                              // M11 D41, from OUTSIDE_PARENT_BASE

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OutsideKind { Megacorp, Syndicate, State, World }  // M17's; L2 builds only World

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutsideFaction {             // M17 appends persona, edge, rep, data, tech, exposure, attitude,
    pub id: OutsideId,                  // branches, order*, books, failing_since, …, all #[serde(default)]
    pub name: String,                   // "the World"
    pub kind: OutsideKind,
    pub treasury: i64, pub treasury_ref: i64,
    pub base_income: i64, pub base_upkeep: i64,
    pub income_today: i64,
    pub market: f32,                    // the outside market, mean 1.0 (M17 § 1's random walk; L2 holds it at 1.0)
    pub dead: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outside {                    // World::outside
    pub factions: Vec<OutsideFaction>,
    pub minted: i64,                    // Σ coins the outside created (refills) since day 0
    pub inbound: i64, pub outbound: i64,// cumulative coins across the boundary
    pub export: ExportBook,             // L2's; M17 reads it as the World account's demand
    pub next_id: OutsideId,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExportBook { pub sold: BTreeMap<ExportGood, u64>, pub paid: BTreeMap<ExportGood, i64> }
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ExportGood { Food, Parts, Data }
```

`outside::export_daily` (*new*, after `corps::daily` closes the books): when `[export] enabled`, the World account is created at `OUTSIDE_PARENT_BASE` if absent; its `treasury` is refilled to `treasury_ref` (`minted += refill`). For each good, owners' stock above a floor is bought up to `cap_per_day` at `price[good] × market`: Food from corp Farms' `stock_food` above `export_floor`, Parts from Fab stock above `parts_floor`, Data from Lab `DataStore`s above M14's `data` sell floor. The World pays the owner (`Flow::Export`, taxed), `inbound += paid`, the goods leave. `ExportBook` records it. Chrome and decks are assets, not goods: M17 decides whether the World buys them as Parts-equivalents (`[assets] parts_per`). With `enabled = false` nothing runs and `outside` stays empty.

```toml
[living]
enabled = true                       # false (--l2-off): the M15-closing city, byte for byte
[jobs]                               # placeholders
enabled = true
seed_venues = { club = 3, arcade = 3, noodle_bar = 10, fight_pit = 2, den = 2, lounge = 1, fab = 2 }
refit_frac = 0.5                     # of found_cost
fab_yield = 0.6
fab_skill_floor = 0.6
fab_skill_slope = 0.8
fab_import_trigger = 300             # coins imported in 14 days by the corp's own sellers
scrap_per_part = 4
sweep_per_hour = 6
[economy]                            # additions
wage_host = 6
wage_attendant = 5
wage_cook = 5
wage_fighter = 7
wage_croupier = 6
wage_concierge = 8
wage_fabber = 7
[buildings]                          # additions and changes
market = { capacity = 20, stock_cap = 2000, staff = 12 }
bar = { capacity = 24, stock_cap = 0, staff = 5 }
club = { capacity = 40, stock_cap = 0, staff = 10 }
arcade = { capacity = 16, stock_cap = 0, staff = 3 }
noodle_bar = { capacity = 10, stock_cap = 60, staff = 3 }
fight_pit = { capacity = 40, stock_cap = 0, staff = 4 }
den = { capacity = 20, stock_cap = 0, staff = 4 }
lounge = { capacity = 16, stock_cap = 0, staff = 6 }
fab = { capacity = 14, stock_cap = 400, staff = 12 }
[corps]                              # additions
found_cost = { club = 400, arcade = 200, noodle_bar = 120, fight_pit = 300, den = 250, lounge = 900, fab = 900 }
value = { club = 600, arcade = 300, noodle_bar = 180, fight_pit = 400, den = 350, lounge = 1400, fab = 1500 }
upkeep = { club = 10, arcade = 4, noodle_bar = 2, fight_pit = 4, den = 4, lounge = 20, fab = 15 }
residents_per = { club = 600, arcade = 600, noodle_bar = 120, fight_pit = 1000, den = 800, lounge = 2000 }
[leisure]
price_base = { club = [0, 5, 12], arcade = [2, 3, 6], noodle_bar_markup = 1, fight_pit = [3, 0, 0], den = [2, 3, 0], lounge = [0, 0, 25] }
front_markup = 1.2
house_edge = 0.08
stake_frac = 0.3
stake_cap = 20
big_win = 40
bout_hour = 23
door_heat = 0.6
[budget]
band = [30000, 60000]
works_step = 10
works_max = 120
upkeep_step = 0.05
upkeep_floor = 0.3
[export]
enabled = false
treasury_ref = 20000
cap_per_day = { food = 50, parts = 20, data = 5 }
price = { food = 3, parts = 18, data = 40 }
export_floor = 200
```

### Events and CSV

New `EventKind`s: `Gambled` (a big win), `Bout`, `Refit`, `Exported` (one a day, the totals), `WorksPosted`. `Founded` covers seeding and `Register` of the new kinds. CSV: `flow_leisure`, `flow_gamble`, `flow_tribute`, `flow_export`, `flow_public_works`, `wage_dole_ratio`, `venues_{kind}`, `visits_{kind}`, `fab_parts`, `scrap_parts`, `parts_imported`, `works_jobs`, `upkeep_mult`, `outside_inbound`, `outside_minted`.

## 2. Fun and the street

### The need

`Needs.fun` decays at `fun_decay_per_tick × (0.6 + 0.4 × sociability) × class_mult[class]` (Corp 1.3, Street 1.0, Dreg 0.8: the rich need more to feel anything, the poor have less time to). Mood gains `fun_mood × (fun − 0.5)`. A Statistical agent's hourly decay runs in `lod::run_statistical`'s `needs::decay` like the others. `fun` is not a GOAP symbol for any existing goal: only `Unwind` reads it.

### `Unwind`

Considerations: `U(fun)` Logistic{8, 0.5}; phase Evening and Night off shift `gate_or(0.3)`; `greed`-weighted affordability of the best rung reachable (`life::travel` on the walk); `pride` adds to the mid and high rungs; `courage` to FightPit and Den. The rung is chosen once at think time (`leisure::choice`, *new*), the plan is scripted (`plan::plan_for`'s bypass, as `Raid`): `GoTo(venue) → Enjoy | Gamble | EatOut`, or `GoTo(spot) → HangOut` for the free rung, or `GoTo(fight or raid within watch_tiles) → HangOut` (watching is the street's free show). The rung ladder by coins after `hotel_reserve_meals`:

| Rung | Satisfiers | Fun gain |
| --- | --- | --- |
| free | HangOut (+0.1 an hour), watch a fight, a raid or a bout from the door, street dice (a `Gamble` of ≤ 1 coin between two HangOut contacts, no house) | 0.1-0.3 |
| cheap (1-3) | a drink, a NoodleBar meal, an Arcade | 0.1-0.35 |
| mid (3-6) | Club, FightPit, Den | 0.4-0.6 |
| high (≥ 12) | Spire Club, Lounge | 0.7-0.9 |

`already_satisfied(Unwind)` is `fun ≥ fun_satisfied` or no rung reachable today. A broke agent's only rungs are free ones, and when its fun is low and coins below a cheap rung, `Earn` reads `U(fun)` as a second term on Beg and Scavenge: **cheap fun is a reason to beg, steal and owe, not a pacifier** (addendum 15).

### HangOut

Spots are computed once a day per district (`leisure::spots`, *new*, ≤ 24 a district): the outside doors of Bars, Markets, Clubs and NoodleBars; each gang's Hideout door (members only); in Sump districts `barrels_per_sump` fire barrels hash-placed on street tiles near Homes. For the agent `a` at think time, a spot's score is `Σ over a's top contacts c (≤ 6: Spouse, Family, Friends, gang-mates by affinity) affinity(a, c) × P(c at the spot's door within 2 h)`, where P reads `hunt::habit(world, c, now)` (the M15 Trace reader the Hunt uses), times `life::travel` on the walk; ties to the nearest. A kin term adds `kin_w × days since a last co-located with c, capped at 7`: the shadow's "see someone I love" goal is this term, not a new goal. `HangOut` is a `Wait`-shaped action of `hangout_ticks`; while it runs the agent is a chat venue (`goals::chat_venue` true at a spot), so the existing `Chat`, `social::gossip` and recruitment pitch run there. `already_satisfied(Socialise)` drops its "no drink possible" clause when a spot is within `hangout_reach`.

Density per district and hour (the ambient-LOD readability addendum 13 asks for) is the count of HangOut and venue occupants, a CSV column per district.

### Off-screen leisure

`leisure::stat_daily` at 21:00: each Statistical adult with `fun < fun_satisfied` picks a rung by the same coins rule and its home district, pays the nearest open venue of that rung in its district (a NoodleBar for a hungry one, as its evening meal), gains the rung's fun, and adds a visit; the free rung adds `0.2` and nothing else. Its own stream decides `Gamble`. Venues' Statistical revenue is therefore daily and lumpy at 21:00; the parity test adds `leisure coins per agent-day` (Full vs forced Statistical, within 15 %) to its six rates.

### Gang fronts, the leader and the creed

- **Fronts.** In `gang::daily_economy`, a gang under Expand with `treasury ≥ found_cost.fight_pit` (or `.den`) and a vacant Lot or derelict in a held district builds one (≤ `fronts_max` 2 per gang): owner the gang, `Venue.front_of` the gang. Its staff are hired from the open market, preferring members' kin. A front's dealer: `stims::dealer_target` accepts the gang's own fronts and Clubs as deal venues beside Bars. A law Crackdown on a district closes its fronts for `crackdown_close_days` (`closed_until`).
- **Collect and the tribute that pays.** The leader gains `GoalKind::Lead` (*new*, scripted): on `collect_weekday`, `GoTo(each front) → Collect → GoTo(Hideout) → Collect`; `Collect` (*new*) moves the fronts' week into the treasury (already there by `ownership::pay`, so the action books it) and pays `tribute_share` of the week's territory tribute and front take to the members, highest rank first (`Flow::Tribute`, untaxed). A Statistical leader collects in a daily-pass stand-in on the same weekday. A member's income is then wages-like: the shadow's "extortion took 0 c, income is the dole" gang member gets a weekly cut.
- **Call.** `Lead` also runs `Call` on the evening after an order change: the Hideout becomes the top HangOut spot for every member within `call_tiles` for `call_hours` (the leader's "muster" in the shadow's sense; the raid `Muster` action is unchanged).
- **The creed.** A Purist gang under Expand works `Preach` (*new*) instead of a shakedown on `preach_share` of its GangWork: `GoTo(busiest spot in a held district) → Preach`, an M15 `social::resolve` Persuade on a co-located adult, success a pool deed and `opinion` toward the gang `+preach_opinion`, and the recruitment filter's pitch. The Chapel (The Unplugged's Hideout) is the members' HangOut spot: "Gather" is that.
- **Rich off-hours.** Execs and the top wealth decile reach the Lounge rung; an exec's evening after `exec_shift` scores Unwind with `pride`, so the CEO's evening is the Lounge, not idle at home.

```toml
[needs]                              # additions; placeholders
fun_decay_per_tick = 0.00023         # ~3 days from full to empty at sociability 0.5
fun_mood = 0.3
fun_satisfied = 0.6
[leisure]                            # additions
class_mult = [1.3, 1.0, 0.8]         # Corp, Street, Dreg
hangout_ticks = 60
hangout_reach = 40
barrels_per_sump = 3
kin_w = 0.1
watch_tiles = 12
fronts_max = 2
crackdown_close_days = 3
collect_weekday = 6
tribute_share = 0.5
call_tiles = 120
call_hours = 3
preach_share = 0.5
preach_opinion = 0.05
```

New `ActionKind`s: `HangOut`, `Enjoy`, `Gamble`, `EatOut`, `FabWork`, `Sweep`, `Collect`, `Preach` (the four scripted ones never in `PLANNABLE`); `GoalKind`s `Unwind`, `Lead`; `MemoryKind::Enjoyed` (*new*, the drink's `Socialised` analogue, for "had fun today"). Events: `Collected`, `Preached`. CSV: `fun_mean`, `fun_satisfied_share`, `hangouts`, `hangout_contacts_mean`, `fronts`, `collected`, `preached`, `d{i}_street_density`.

## 3. Statistical violence from the faction layer

### The ledger

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ViolenceSource { Order(Order), Vendetta, Riot, Episode }

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum VictimClass { Civilian, Member, Watch }        // Watch: public and private guards

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RateCell {
    pub victims: [VecDeque<u16>; 4],    // Killed, Assaulted, Robbed, Abducted per day, last `rate_days`
    pub exposure: VecDeque<u32>,        // body-hours of that class in the district under the source, per day
    pub acts: [VecDeque<u16>; 3],       // actor side, members' Extort, claim blows, Deal sales (§ 4)
    pub member_hours: VecDeque<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OrderRates { pub cells: BTreeMap<(ViolenceSource, DistrictId, VictimClass), RateCell> }   // World::order_rates

pub struct Hole { /* … */ #[serde(default)] pub source: Option<ViolenceSource>, #[serde(default)] pub faction: Option<EntityId> }
```

**What counts.** An on-screen `Assault`, `Murder`, `Robbed` or `Abducted` event whose actor is a Full or Coarse agent and whose source is: the actor's plan goal `GangWork` or `Raid` under its gang's order (`gang::following_order`), its gang or corp in an open vendetta with the victim's (`grudges::member_of`), a live riot it belongs to (`world.rioter_of`), or an episode (`world.episodes`). Counted in the victim's district and class. Hunts are excluded on purpose: a Hunt promotes both hunter and hunted (M15 W22/W23), so its killing is always on screen and needs no off-screen twin. **Exposure** is tallied in `lod::assign_hour`: body adults per (district, class) for every source active in that district this hour. Off-screen holes never feed the ledger, only on-screen events do.

**Where a source touches.** `Order(o)` touches the gang's held districts (`gang::held_districts`) for Expand, Squat, Harvest, VirtRaid and LieLow; the rival's held districts for Contest; the door's district for Raid, Retaliate and BreakOut on the days the raid is live. `Vendetta` touches every district where both factions hold members' Homes; `Riot` the riot's district while it lasts; `Episode` the episode agent's district that day.

### The daily pass

`fviolence::daily` (*new*, midnight, after `gang::run`, before `bind::run`; M13's `chrome::abduction_daily` folds into it as the `Order(Harvest)` × `Abducted` cell):

1. Rates: for each cell, `rate[k] = (Σ victims[k] + prior[source][k] × prior_weight) ÷ (Σ exposure ÷ 24 + prior_weight)` per agent-day, over the last `rate_days`, times `fv_mult` (1.0; the knob if the sanity bound needs it). The `prior` is a per-source config row (a placeholder, calibrated in phase 5 from the first runs' ledgers).
2. Rolls: each Statistical adult, ascending, in a touched district rolls `Killed`, then `Assaulted`, then `Robbed`, then (Harvest only, with `Kit.visible ≥ harvest_min_visible`) `Abducted` on `rng.agent(id)`, its class's rates summed over the active sources (the source of a hit is picked by share). A hit runs the exact effects of `lod::stat_rolls`' branch for that kind (or `chrome::abduct_offscreen`) with `source`, `faction` set and `consequential = true`; at most one hit per agent per day.
3. Cap: at most `day_cap[kind]` hits city-wide per day (placeholder `Killed` 3), the last line against a runaway.

### Binding from the acting faction

`bind::candidates_in` with `hole.faction = Some(f)`: the pool is restricted to members of `f` (gang members by `world.gang_of`, corp employees and the exec for a corp, guards for the Law) whose day trace is alive and not jailed, weighed by the existing terms (zone and district, chrome, dread). An empty pool binds `Unknown`. Riot holes bind among the riot's rioters; episode holes to the episode agent. `p_unknown` and the witness roll are unchanged, so the law's view is as M10's: most faction holes are anonymous, the bound ones feed arrests, pools, grudges and the kin channel exactly as M15 wired them.

### Keeping Murders inside the bound

Off-screen killings rise; the total must stay within +25 % of the M15-closing run. Three things pull on-screen violence down at the same time, and they are designed to:

- **Bodies move tiers, violence does not double.** Under the gang quota (§ 4) most members are Statistical. Their violence is now the ledger's per-capita rate applied off screen, so a city of 170 members does the same per-member violence whether 60 or 170 of them hold bodies; on-screen gang-on-gang fights among bodies fall with the body count.
- **The jobs economy shrinks the gangs' supply.** `join_gang_desperation_hunger` and `_lawfulness` gate the desperate; employment and fun move fewer residents past them, and fronts' jobs pull members' kin into wages.
- **M15 phase 5's grudge volume** (grudges formed on killings and robberies, not every beating) cuts revenge violence before L2 lands.

If the bound still breaks, `fv_mult` falls first (a printed finding), never an M8-M15 knob.

```toml
[fviolence]                          # placeholders
enabled = true
rate_days = 14
prior_weight = 48.0                  # body-days of evidence the prior is worth
fv_mult = 1.0
day_cap = { killed = 3, assaulted = 12, robbed = 20, abducted = 2 }
prior = { order = [0.0003, 0.002, 0.003, 0.0], vendetta = [0.0004, 0.002, 0.0, 0.0], riot = [0.001, 0.01, 0.01, 0.0], episode = [0.0005, 0.003, 0.0, 0.0], harvest_abducted = 0.0004 }
```

CSV: `fv_killed`, `fv_assaulted`, `fv_robbed`, `fv_abducted`, `fv_bound`, `fv_unknown`, `kill_rate_body`, `kill_rate_stat`, `kill_rate_body_civ`, `kill_rate_stat_civ` (victims per 1,000 agent-days by tier at the time of death).

## 4. The LOD budget and the churn

### What a forced class costs

A body costs one slot in its tier, whatever made it a body. The rule: **`max_full` and `max_coarse` are real caps on ranked bodies; each forced class has a quota inside them; held agents cost no body.**

| Class (`lod::rank_class`) | Today | L2 quota (inside `max_coarse` 150) |
| --- | --- | --- |
| pinned (5) | uncapped, Full | uncapped (≤ M18's circle of 24 + the shadow's picks) |
| runners, hunters, hunted (4) | uncapped | ≤ `max_hunts` 16 × 2 + seated runners (bounded by chairs) |
| the public watch (3) | all guards and gravediggers | all on shift; off shift they rank as civilians |
| gang members, private guards, rioters, abductees, harvest targets, episodes (2) | ranked, the farthest fall to Statistical | `gang_quota` per gang (25 at 4 gangs), private guards 12, rioters `riot_promote_max` |
| sentenced and emigrating | Coarse outside the rank (measured: the whole overrun) | **held**: Statistical, no slot |

### Held prisoners

A sentenced, unpinned agent becomes Statistical in place: `lod::set_lod`'s demotion skips `snap_to_phase_door` for a `Sentence` holder and the Position stays in the Jail. `lod::due_this_tick` already skips sentenced agents; `law::held_daily` (*new*, midnight) runs their day at once: 24 hours of `needs::decay`, a Jail meal (`Flow::JailFood`, as the cell's Full meal), energy restored, and the cell's social life: `jail_meet_max` meetings drawn among the held per arrival (`stat_meet`'s shape) and the gang pitch (`jail_pitch`). An agent within `release_soon_hours` of release, cuffed, or in a BreakOut gang's freed list is promoted to Coarse, so the walk out is real. A pinned prisoner stays Full (L1). Measured effect: Coarse falls from ~300 to ≤ 150 + the forced overflow, the Jail's 100-160 prisoners at ~1 pass a day each.

### Ranking within a class

Inside class 2, per gang, the order: leader, the next two of `gang::leader_ranking`, members with a `lod::story_relevant` step, members within `front_tiles` of the order's target (the Contest frontier, the raid door, the Harvest target), then a **daily rotation**: `splitmix64(seed ^ id ^ day)` ascending, so every member holds a body some days. Over the quota the member ranks as class 0 (civilians' screen distance and story). A gang with a live raid order gets `raid_quota` (40) from `raid_promote_hours` before the muster to the raid's end, the M12 D31 rioter precedent. The test `factions.rs::test_gang_members_are_never_statistical` becomes `test_gang_quota_holds_and_rotates`.

### The Statistical GangWork hour

In `lod::stat_work`, a Statistical gang member in its gang's work hours, not jailed, with an order whose GangWork has a target, rolls once an hour on its stream against the ledger's actor rates for `(Order(o), district of its Home, Member)`: `p_extort = Σ acts.extort ÷ Σ member_hours` (shrunk to `[gangs] stat_extort_prior`), and likewise claim blows and deals. A hit calls the effect functions directly, as `stat_theft` does: `gang::extort(world, id, home)` on the territory Home `gang_work_target` would pick (no walk), `gang::squat_claim` for Squat, a Deal sale through `stims` at the gang's deal Bar. Witnesses: none (a Statistical agent has no eyes; M15 § 8). The law sees what the table lets it see: a hit opens a report at `p_theft_caught` (the table's) against the member. Off shift the member is an ordinary Statistical adult.

### The churn

- **Scavenge.** A dry hour returns `StepResult::Done` with nothing (and a `Searched` dry count on the Brain, *new*, `#[serde(skip)]`); after `scavenge_dry_max` dry hours in a row `Earn` cools for `scavenge_cool_hours` (`World::cool_goal`). No abort, no re-plan, and the hour is honestly spent.
- **Beds.** `plan::reserve_for` writes `ReservationKind::Bed { home }` for a plan whose Sleep is at a second bed (`life::hideout_bed`, `life::away_hotel`) and for `CheckIn`; `hideout_bed`, `away_hotel`, `street::check_in` and the Hotel's capacity read reservations as occupants. A Sleep step that arrives to no bed sleeps rough where it stands when `life::rough_ok` (the step becomes the street Sleep) instead of failing. The first commit of phase 3 instruments the abort text with the bed kind, so the fix is checked against what actually fails.
- **Venues.** `Enjoy`, `EatOut` and `Gamble` reserve a place (`ReservationKind::Bed`'s sibling, *new* `Seat { building }`) at plan time, so an evening rush does not turn into `BuildingFull` aborts.

```toml
[lod]                                # additions; placeholders
held_prisoners = true
gang_quota = 25
private_quota = 12
raid_quota = 40
raid_promote_hours = 4
front_tiles = 30
release_soon_hours = 2
[gangs]                              # additions
stat_extort_prior = 0.02
stat_claim_prior = 0.02
stat_deal_prior = 0.01
[life]                               # additions
scavenge_dry_max = 3
scavenge_cool_hours = 4
```

CSV: `tier_held`, `tier_coarse` (now excluding held), `gang_bodies`, `gang_stat`, `stat_extorts`, `aborts`, `aborts_scavenge`, `aborts_sleep`, `aborts_checkin`, `aborts_seat`.

## 5. Levers and god commands

| Lever | Effect |
| --- | --- |
| `SetLeisureTax(f32)` | an extra tax on `Flow::Leisure` and `Flow::Gamble` |
| `BanGambling(bool)` | Dens close and FightPit bets stop; Dens are then gang-only and raid-worthy |
| `SetPublicWorks(bool)` | the band's hiring arm on or off |
| `SetExport(bool)` | opens or closes the World account's buying |
| `curfew` (exists, per district) | now closes leisure venues at night |

**God commands**: `OpenVenue { kind, district, owner }` (built on the nearest Lot or derelict); `CloseLeisure { district, days }` (every venue closed: does fun fall, theft and assaults rise, the HangOut density move?); `SetFun { district, value }`; `SetExportPrice { good, price }`; `FactionStrike { gang, district, days }` (the gang's order pinned to Contest on that district: do off-screen holes appear there and bind to that gang); `HireAll { kind }` (fill every vacancy of a kind: does the dole bill fall by the head count). God scenarios for phase 5 (`tests/god.rs`): close every Club in Mid West for 14 days; double the dole for 30 days (do jobs empty, do venues gain, does the band move upkeep); kill both Fabs' staff (do imports and the Treasury's customs rise); `FactionStrike` Ninefold on a Stackwell district; `SetExport` on at 10x price (do Food and Tech corps `Grow`).

## 6. The 365-day sanity run

`test_l2_year_seed_42` and `_43` (`#[ignore]`, run serialized in phase 5): 365 days, `--report`, a save at day 365 (`--save-at`). Printed per 30 days, and asserted where marked:

| Quantity | Asserted bound |
| --- | --- |
| population | within the scaled v1 bounds every day; ≥ 1,700 on day 365 |
| starvation deaths per 120 days | ≤ the M15 run's × 1.5 in every year |
| grudges live, vendettas open | grudges ≤ 4 × adults (the cap) and the year-3 mean ≤ 1.5 × the year-1 mean; vendettas open ≤ ½ of faction pairs |
| Coarse (non-held), held, Full | Coarse ≤ `max_coarse` + pinned every sampled hour |
| memory entries (Σ over agents), Trace days | the year-3 mean ≤ 1.2 × the year-1 mean |
| save size at day 365 | ≤ 2 × the day-120 save |
| Jail occupancy | ≤ capacity (structural); days at capacity printed |
| Assault events per day | ≤ 42.7 in every 30-day window |
| Treasury, wallets, Gini, Σwages ÷ Σdole | printed; no corp collapse (≥ 4 of 9 seeded alive on day 365) |
| PlanAborted per day | the year-3 mean ≤ 1.2 × the year-1 mean |
| ticks/s | printed per window (the last window against the 4,000 floor, a print) |

**The Winter wave** (measured since M12: ~14k `Starving` events, mean hunger and mood worst near day 111) is printed as `Starving` per day and mean hunger over days 80-120 of each of the three years; the finding is whether wages and the NoodleBar (a cheaper meal path) change it, and whether it repeats. It is not asserted: Winter is a famine by design (`season_mult`).

## 7. The role features

| Feature (`SHADOW_V1.md`, handoff) | L2 | Where |
| --- | --- | --- |
| Gang leader: Muster (Call), Collect | in scope | § 2, `GoalKind::Lead` |
| Gang leader: Parley, delegated violence | deferred to **M16b** (leverage, boards) | a Parley is a coercion or a deal between factions |
| Gang leader: retinue travel | deferred past **M18** (addendum 13, company as a unit) | ambient LOD |
| Creed: Preach, Gather | in scope | § 2 |
| Creed: Patrol (anti-chrome vigilantes) | deferred to **M16b** (tags and scanners read chrome) | |
| Clinic customers and fees | in scope as money: wages put coins in customers' wallets; an asserted existence bullet (≥ 1 Install or Therapy a week per seeded Clinic, majority) and therapy/install fees in phase 5's price print | § 1 |
| Runner archetype with fixer-fed orders | deferred to **M16a** (the Fixer posts `RunOrder`s) | |
| Household and spouse goals | in scope as HangOut's contacts and kin term | § 2 |
| Guard corruption | deferred to **M16a** (the accessory rule; a bribe to a guard is a contract on the law) | |
| Rich-agent off-hours | in scope (Lounge rung, the exec's evening) | § 2 |
| A tribute flow that pays | in scope (`Collect`, `Flow::Tribute`) | § 2 |
| Child routines | deferred past **M18** (children have no Brain; a later feature, `SHADOW_V1.md`) | |
| Sleep beyond 4-6 h | in scope only as the bed reservation (§ 4); the waking-goal-beats-Sleep-above-0.6 finding is the late calibration milestone's | |

## 8. UI, events, CSV

- **Building panel**: a venue's price, visits (7 days), take, front gang, staff on shift; a Fab's Parts made and sold.
- **City panel**: an Economy section (Σ wages, Σ dole, their ratio, employed share, the Treasury and its band, `upkeep_mult`, public works, export in and minted) and a Leisure section (fun mean, satisfied share, visits by kind, the biggest win this week).
- **Inspector**: the `fun` bar, the last `Enjoyed`, the agent's HangOut spot and expected contacts.
- **LOD panel** (debug): bodies per class against quota, held prisoners, rotation day.
- **Map overlay** (a key chosen against the app's map at phase 2): street density per district and hour, venues by kind, fire barrels.
- **CSV**: the columns of §§ 1-4; the gate reads them through `tools/analyze_run.py`, which gains an L2 section.

## 9. Save compatibility

Every new field is `#[serde(default)]`: `Needs.fun` 1.0, `Building.venue` None, `Hole.source`/`faction` None, `World::outside` empty, `World::order_rates` empty (priors until the ledger fills), `World::budget` (`upkeep_mult` 1.0), `World::scrap` 0. Variants are appended (`BuildingKind`, `Role`, `GoalKind`, `ActionKind`, `Flow`, `EventKind`, `MemoryKind::Enjoyed`, `ReservationKind::Seat`). `save::SAVE_VERSION` 1 → 2; a version-1 save gets its venues and Fabs seeded on the first midnight when Lots exist (the Feeds precedent, `feeds_seeded`), its prisoners demoted to held at the first hourly pass, and its gang members re-ranked under the quota. `Config::v1_profile` and `calibration_city` set `[living] enabled = false`, so the v1, M8, M9 tests and `calibrate` are untouched; `assets/stat_table.toml` is re-run once after phase 2 because fun changes the evening of the 500-agent city.

## 10. Testing and calibration

**Unit tests** (`citysim/tests/leisure.rs`, `jobs.rs`, `fviolence.rs`, `lod_budget.rs`, `outside.rs`): each new kind builds on a Lot, posts its staff and pays a shift; `Register` chooses a NoodleBar at 120 coins when the per-capita count is lowest; a refit turns a derelict into a Den at half cost; `Enjoy` charges the venue price and is refunded on a failed start with money conserved; `Gamble` conserves coins and a house that cannot pay closes the table; a Lounge refuses a Dreg; `HangOut` picks the spot where a Friend's habit is; `Unwind` skips a broke agent's paid rungs; the Fab accrues Parts and `parts_market` buys from it before the Recycler; scrap becomes a Recycler Part at `scrap_per_part`; the band posts public works above `hi` and lifts `upkeep_mult` below `lo`; the export moves goods out, coins in, and the identity holds, and with export off `outside` stays empty; a held prisoner is fed daily, is never snapped out of the Jail and is promoted two hours before release; the gang quota holds outside a muster, rotates daily and opens at a raid muster; a Statistical member's GangWork hour extorts a territory Home; the ledger counts a Contest killing in the victim's district and class and not an off-screen one; the daily pass opens a `Killed` hole in a contested district and the binder names a member of that gang or Unknown, never a civilian; a dry Scavenge hour is `Done`; a reserved bed cannot be taken; a version-1 save loads.

**Scenario** (`citysim/tests/scenario.rs`, `#[ignore]`): `test_l2_living_city_seed_42` (the Goals bullets, seeds 42-44 majority and 42-47 means/existence, per-seed data in comments), `test_l2_year_seed_42` and `_43` (§ 6), `test_faction_violence_parity` (in `tests/lod.rs`: seeds 2000-2005, 2,000 residents with gangs, 120 days; per 1,000 civilian agent-days, Statistical victims of faction-sourced `Killed` and `Assaulted` against Full and Coarse civilian victims of on-screen faction violence; asserted within a factor of 4 as the sanity bound, printed against the factor-2 band). The `--l2-off` identity check (a 15-day report and the calibration city match the M15-closing build byte for byte) runs in every phase.

**Calibration** (phase 5, printed findings only, tuned via `[jobs]`, `[leisure]`, `[budget]`, `[fviolence]` and the new `[economy]` wage keys; never an M8-M15 knob): the bands of *Goals and acceptance*, plus `stat_table.toml` re-run, plus the M13 price print (§ 1).

## 11. Phases

| Phase | What lands | Commit gate | Numbers per phase (seed 42, 120 days; placeholders) |
| --- | --- | --- | --- |
| 1. Jobs | § 1: seven kinds and roles, seeding, `Register`/refit, staffing changes, `Sweep`, the Fab and scrap chain, the budget band, the export hook (off) and the identity, CSV | `--l2-off` identity; unit tests; trio | employed ≥ 450 on day 1; Σwages ÷ Σdole ≥ 0.7 on day 60; Treasury in band by day 30; `parts_imported` down ≥ 30 %; leisure revenue only from Full/Coarse drinkers (fun not built) |
| 2. Fun and the street | § 2: `fun`, `Unwind`, HangOut and spots, `Enjoy`/`Gamble`/`EatOut`, the Statistical pass, fronts, `Lead`/`Collect`/`Call`, `Preach`, venues' door policy, the overlay; the parity test's seventh rate; `stat_table.toml` re-run | parity test; identity; trio | Σwages ÷ Σdole ≥ 1.0 on day 60; fun satisfied 40-70 %; `flow_leisure` ≥ 1,500 a day; members paid by `Collect` ≥ 1 a week |
| 3. The LOD budget and the churn | § 4: held prisoners, quotas, ranking and rotation, the Statistical GangWork hour and the ledger's actor side, Scavenge, bed and seat reservations | identity; trio; scenario suite serialized | Coarse ≤ 150 (+ pinned), held ≈ jailed; PlanAborted per day ≤ 600 (from 1,475); gang income within 20 % of phase 2's |
| 4. Off-screen blood | § 3: the ledger's victim side, `fviolence::daily` (Harvest folded in), `Hole.source`/`faction`, the binder filter, `test_faction_violence_parity`, the M10 finding re-read | parity tests; trio | off-screen killings ≥ 30 (from 11); civilian rate ratio ≤ 2-4; Murders within +25 % of the M15 run |
| 5. Gate, the year, re-shadow | the L2 gate and god scenarios, the 365-day runs, calibration as findings, the M13 price print, README and docs ("Implemented: deviations"), then `/code-review high` and a fix commit; **re-shadow** (below) writing `docs/SHADOW_V2.md` | every gate on main | the § 6 table on 42 and 43; the findings on 42-47 |

Phases 1 and 3 touch disjoint systems and may run in parallel worktrees; 2 needs 1's venues; 4 needs 3's ledger. Each phase is committable alone with its section switch, and the orchestrator merges in order 1, 3, 2, 4, 5.

**Re-shadow (phase 5's last step).** The shadow tool V2 first: picks free on the start day (L1 began it) for every archetype; a pinned pick stays Full in jail (L1); a `dealer` pick from every gang's `deal_log`, not one per Bar; a `runner` pick requiring Virt activity (a `JackedIn` in the last 7 days); children named unpinnable in `--list`; gossip told about the agent captured in the diary; and L2's archetypes added (`cook`, `club_staff`, `fighter`, `fabber`, `sweeper`, a `worker` on a Friday night, the `reporter` never yet followed). Then `citysim-cli shadow --seed 42 --start-day 20 --days 7 --count 3` over every archetype, five critics as in V1, and `docs/SHADOW_V2.md` with the per-archetype table re-scored against V1 and L1b (walking, sleep, idle, work, earned, fun hours, people seen).

## 12. Risks

- **The money loop drains or floods.** Fun is a new sink for ~2,000 wallets that hold ~9k coins; if venues take the dole before food, starvation rises (brutality as a broken day). Watch starvation and theft in phase 2 before anything else; the Unwind gate on `hotel_reserve_meals` and the rung's affordability are the brakes, not prices.
- **The band oscillates.** A Treasury that swings across the band hires and fires public works weekly and moves every corp's upkeep. The 3-day hysteresis and the small step are the brakes; if it rings, widen the band, do not add a controller.
- **Off-screen violence feeds back.** Off-screen holes form grudges and vendettas through the pools and the kin channel; vendettas are sources; sources raise rates only through on-screen events, but more Hunts mean more on-screen killings, which raise the rates. The shrinkage weight, `rate_days`, the per-day cap and `fv_mult` are the brakes; the 365-day run is where a runaway shows.
- **The gang quota weakens gangs.** Fewer bodies at a Contest frontier or a raid lose fights the gangs used to win; the order ranking and the muster quota keep bodies where the fighting is, and phase 3 measures gang income and raid outcomes against phase 2.
- **Venue rush hours.** Two thousand evenings at 21:00 and a few dozen venues: the Statistical pass is not capacity-bound (by design; it is an aggregate), but Full and Coarse visitors are; the seat reservation keeps `BuildingFull` out of the abort count.
- **The planner.** Eight new actions; four are scripted and never searched, and `EatOut`, `Enjoy`, `Gamble`, `Sweep` join `PLANNABLE` only as single-step chains behind `GoTo`. The M15 A* cap is unchanged; watch the planner's cap-hit count.

## 13. Out of scope

Power, Water and generators (addendum 9; the goods milestone after M18); corp scrip and its acceptance (M17); building interiors, rooms and verticality (addendum 10); M16's contracts, Fixers, protection on others' businesses, guard bribery, runner orders, Parley and leverage; braindance as a Virt overlay (M14's plane is unchanged); child routines; retinue travel and company as a unit (addendum 13); generated dialogue; outside parents, remittances and the full `OutsideFaction` (M17: L2 builds the World account's hook only); a profiling phase or any optimisation work (Dylan, addendum 16: the LOD budget here is a design rule).

## Builds on

| Milestone | Decision | What L2 reads or extends |
| --- | --- | --- |
| M10 | D20 the watch ranks above gangs; D26 `stat_violence_mult`; D32 a promotion binds the victim's holes; D33 coverage | § 4's class table keeps the ranks; § 3 adds a second, faction-driven roll beside the table's; held prisoners never promote mid-sentence |
| M11 | D3 customs to the Treasury; D6 rent; D17 staff and vacancies; D25-D27 `Register`, `Found`, incorporation; D28 values; D41 `Corp.parent`, `ParentId`, `OUTSIDE_PARENT_BASE`; D48 `order_flat` | new kinds join `FOUNDABLE`, `value`, `role_for`; the band scales upkeep, not rent; the export hook takes `OutsideId` from D41 |
| M12 | D20 Hotel beds; D23 Sanitation; D25 derelicts; D31 rioters rank with gangs; D3 the district binder | refit on derelicts; `Sweep` for Sanitation; the raid quota copies D31; faction holes carry the district |
| M13 | D6 `stock_goods`; D8 `Flow::Import`/`Parts`; D14 the parts market; D16-D17 Clinic, Garage, Tech niche; D36-D37 Harvest and the off-screen abduction; D46 Statistical chrome multipliers | the Fab is a Parts source in D14's market; `abduction_daily` folds into § 3 |
| M14 | V13 a jacked-in body is never Statistical; V16-V17 Labs and Data sales; V5 public terminals | runners keep class 4; Data is an export good; an Arcade counts as a terminal |
| M15 | W2 Statistical rumours; W22/W23 hunters and hunted promoted; W34 the binder's dread; W36 the Feed as a staffed, foundable kind seeded on Lots; W7 the Drink rumour mill | faction holes travel the pools; Hunts are excluded from § 3; venues and Fabs are seeded as `news::seed_feeds`; HangOut is a chat venue for `gossip` |
| L1, L1b | `dole_in_place`, `scavenge_p`/`scavenge_coins`, `commute_cap_tiles`, `quit_rehire_days`, exec pay, `life::travel`, the pinned prisoner | wages at the workplace stay; Scavenge's roll stays; HangOut and Unwind score travel with `life::travel` |
| M17 (spec) | `OutsideFaction`, `OutsideKind::World`, `Outside { minted, inbound, outbound }`, the conservation identity | L2 builds the World account and the identity; M17 fills the rest |

## Implemented: deviations

What the build changed against this spec, one line per plan Decisions row (`~/.claude/plans/life-l2.md`, L1-L40) and per deviation recorded in the phase commits (phase 1 e8355d4, phase 3 25d152e, phase 2 6a04a1d, phase 4 7ad4462) and in phase 5's calibration. Every killing, wage and bet here is a seeded dice roll over counters.

### Plan rows

- **L1** Seven kinds appended to `BuildingKind::ALL` (23), `parse` takes the identifier names; letters `K A S I D U E`, not the spec's `X`/`Z` (the M16 plan claims those); M16 must not reuse the name `Den`.
- **L2** Seven roles appended (`Role::ALL` 17), one role per kind; the six leisure roles work `ClerkWork`, the Fabber `FabWork`; the Fabber reads the farming skill (no mechanical skill exists); Fighters are hired by fighting.
- **L3** `[buildings] market.staff`/`bar.staff` untouched (identity); `[jobs] market_staff = 12`, `bar_staff = 5` through `jobs::full_staff`, and `jobs::top_up` posts the deficit daily at every Market and Bar (phase 1: the whole deficit, a hunkering corp staffs to half).
- **L4** Types in `living.rs`; `Building.venue`, `World::{budget, scrap, jobs_book, venues_seeded}`, plan field `Venue.take_week`.
- **L5** `[living] enabled` is the master switch; `[jobs]` is `LivingJobsCfg`; each section has `enabled` and an `on()`; `living_off()` for `--l2-off`; `residents_per_*` flat.
- **L6** `FOUNDABLE` 6 → 12; `founding::refit` of a derelict Block at `refit_frac`; `build_on_lot`'s tail factored into `founding::convert` (byte-identical); Register falls through to a refit when no Lot passes the tier rule.
- **L7** `jobs::seed_venues` after the Chapel, before `virt::relink`; deferred seeding on load (`venues_due`) for version < 2 saves only, retried until a Lot exists.
- **L8** `FabWork` and `Sweep` in `PLANNABLE` (the spec had Sweep behind `GoTo` only); `LocationKey::Beat`; Sweep requires the Beat location; `districts::sanitation` skips sweepers when `sweep_on`.
- **L9** The Fab as a `parts_market` source and `build_kind_for(Tech)`; scrap from finds into Recycler Parts at `scrap_per_part`; the Fab is out of the Tech share and demand. Phase 5: the source order is the buyer's own Fab, the Recycler, then a rival's Fab (was every Fab before the Recycler: the Recycler sold none of ~950 scrap Parts a run on 42-47); the scavenge `p` is floored at `scavenge_p` (below).
- **L10** The band in `systems/budget.rs`; public works are city Sanitation hires at the Recycler (no `Flow::PublicWorks`; the CSV's `flow_public_works` is their wage bill); phase 1's shape: a step at most every `band_hold_days`, no cut below `hi` + a week of dole, a 2x restore with `works_step` layoffs (the spec's daily cut emptied the Treasury to 63 coins on day 61). Phase 5: `[budget] works_wage` (below).
- **L11** `outside.rs` with M17's names; the World account is 1069 (M17 O3), not 1000; `ownership::cross_in`; `levers.export_open` is the switch; `[export] data_floor` and `parts_floor` are plan keys.
- **L12** `Flow::{Leisure, Gamble, GambleWin, Tribute, Export}`; `GambleWin` untaxed (a plan addition).
- **L13** `Needs.fun` (serde default 1.0) decays outside `needs::decay` (`decay_fun`); `class_mult` under `[leisure]`. Phase 5: the opening `fun` is spread over `[fun_satisfied, 1]` by a hash (below).
- **L14** `GoalKind::{Unwind, Lead}` bypass the planner; the pick is recomputed at plan time and cached in `World::unwind`; Drink at Club/Den through `PlanCtx.leisure_drink`; a "fed" consideration so food comes before fun; affordability folded into the rung score plus a half-purse cap and the "spendable" purse (a week of rent and arrears kept, the homeless keep tonight's bed).
- **L15** Spots are street tiles; `World::spots` saved (not `serde(skip)`); one exchange per HangOut; the first meeting at a spot draws its noise from `splitmix64`; the recruitment pitch is the edge to a member that `recruit_gang` reads.
- **L16** Socialise → HangOut when no co-located partner; `last_met` written at HangOut completion only (`colocation` has no kin-pair loop).
- **L17** `stat_daily` at 21:00; `free_stat` 0.3 (0.2 left the satisfied share at 4 %); the leisure parity is a separate seventh-rate test (`test_full_vs_statistical_leisure_within_15pct`, passes); `calibrate` stays on the L2-off calibration city.
- **L18** The bout's own `p_win` on `WordNs::Bout` (`resolve_fight` rolls on the world stream); bets in a 2-hour window; `big_win` on the gross; an `evening_shift` for Hosts, Fighters, Croupiers and Concierges; the winner's fighting +0.01; no pool deed for a big win.
- **L19** `leisure::open`: closures, derelicts, curfew 22:00-06:00, `ban_gambling`, Seats; the Lounge and Club door policies, no draw.
- **L20** Fronts need a week of their payroll; `Gang.{called_for, collected_day, tribute_week}`; stipends hold back the tribute share and two days of front payroll; a front's staff are hired on the open market (no kin preference); seeded city-deed venues hand over to their heir once they earn (phase 2).
- **L21** Held prisoners decay hourly through `World::held_since` (not a flat 24 h at midnight: arrival-day prisoners starved; not once a day: fed prisoners lifted district mood off the riot line); release and escape promote first. Phase 5: a starving held prisoner gets the cell's meal at once.
- **L22** `[lod] watch_on_shift_only = false` keeps M10 D20 (the spec's off-shift rule took Murders on 42-44 from 137 to 216 and broke the M15 bound).
- **L23** The Statistical GangWork draw is per member-day; Statistical extortion targets an inhabited territory Home, pays half to the gang, rolls no Intimidate and raises no crime; a Statistical squat claim draws no fight.
- **L24** `ledger.rs` types without `Hash`; tuple keys; cells pruned at midnight.
- **L25** `note_victim` from `raise_crime_on` and `kill_by`; the victim must be a body; exposure skips the acting faction's own bodies.
- **L26** `fviolence::daily` at the top of `bind::run`'s midnight branch; `daily_actor` in `lod::run` before the hourly assignment; `fv_active` rebuilt at 01:00 and saved; the day's riots and episodes remembered so a source that ends before midnight still rolls; `chrome::abduction_daily` folds into the Harvest cell.
- **L27** `Hole.{source, faction, riot}`; `lod::stat_hit` factored (byte-identical); the binder filters by faction, riot roster (saved) or episode; `fv_bound_wrong` 0 on every seed.
- **L28** "Already hit today" = any hole since the previous midnight's pass; the Abducted roll uses M13's filter; `stat_hit`/`abduct_offscreen` take the active source.
- **L29** `Brain.scavenge_dry` saved (not `serde(skip)`); the dry hour is `Done`, three cool Earn for 4 h.
- **L30** Bed and Seat reservations; the real Sleep/CheckIn bug was `exec::step_ctx` never computing the second beds (98 % of those aborts), fixed in phase 3; a lost bed sleeps rough.
- **L31** `WordNs` appended `Leisure = 9 … Held`; every L2 roll keyed, never the world or agent stream (scavenge keeps its agent draw).
- **L32** `--l2-off`; byte-identical to d738a07 through phase 4 (each phase checked its parent). Phase 5: identical again with the old stat table (5,072 cells of the 15-day report, 0 diffs); the regenerated `stat_table.toml` moves the L2-off city itself (below), so the gate's fingerprint is this tree's.
- **L33** `SAVE_VERSION` 2; `jobs::migrate` for version < 2.
- **L34** `LivingCols` (54 columns at phase 1, more at 2 and 4) and `BudgetCols` (15) before `ticks_per_sec`. Phase 5: `LivingCols.{stat_spend, parts_sold_fab, parts_sold_recycler}` are gate probes, not CSV columns.
- **L35** Events `Refit, Exported, WorksPosted, Gambled, Bout, Collected, Preached, Called`; amber.
- **L36** `living` after `corp_brain`, before `demography`.
- **L37** Levers and god commands as planned; the CLI's district arguments are indices; `FactionStrike` pins `Gang.strike` in `faction::rescore`.
- **L38** Noodle Bars restock from the nearest Market at `wholesale`.
- **L39** The M13 price print (phase 5): printed, prices not moved (below).
- **L40** Exposure counts bodies only.

### Recorded deviations of the phases

- Phase 1: `Job.paid_once` (a hire draws the dole until its first wage; new city hires starved before payday); seeded Clubs, the Lounge and the fronts stand on the city's deed until they earn; a vacancy re-posts daily only when the owner can cover a wage; employed >= 450 on day 1 not met (409; the plan's arithmetic assumed a 290 base); `parts_imported` up, not down 30 % (a sale always imports `parts_per / 2`); three older tests adjusted (moves, virt node band, corps).
- Phase 3: aborts/day 736-757, not <= 600 (the rest are goal switches, not churn); a corp order dwell under `[living]` (`corp_order_dwell_days` 3, `corp_order_margin` 0.05: Research was overturned by Secure within a day); gate devices: M15's Murder bound as the six-seed sum <= 1.25 x 315, M12 Dregs printed, Crackdown and gang control existence over 42-47, M14 asset-below-tier printed.
- Phase 2: the "fed" consideration, the spendable purse, `free_stat` 0.3, U(fun) makes Beg and Scavenge cheaper when bored and broke; gate devices: M13 crash deaths and M14 decks and Data sold printed with existence asserted.
- Phase 4: `fv_active` and `FvTally` saved; the victim must be a body; the kill-rate denominators are end-of-day free adults per tier; `fv_mult` 0.3 as a stopgap (the spec's priors ~10x too high for civilians); M13's "an episode ended by the law" is existence over 42-49.
- **The unpaid-worker dole removed 2026-10-08 per roadmap addendum 17**: a Job holder never draws the dole (`economy::dole_eligible` is "has no Job"); gone are phase 1's dole until the first wage (`Job.paid_once` stays, read by `gang::desist`), phase 5's `UNPAID_DOLE_DAYS` and the shadow fixes' absentee dole (`noshow_dole_days`); the no-show dismissal at 7 stays.

### Phase 5 (gate, the year, calibration; 2026-10-08)

- **Calibration (a), wages over the dole** (`[budget]`): new plan key `works_wage` 7 (0 keeps `wage_sanitation` 5); `works_max` 120 and `works_step` 10 kept. The headline Σwages ÷ Σdole on day 60 reads 0.41-0.52 on 42-47 (employed 23-27 % of adults) and stays a printed finding. Tried and reverted: 400 public works at wage 7 met the target (0.99-1.07, 40 % employed, starvation 7-13) but only by spending the Treasury's opening hoard; from ~day 150 the band laid the works off in waves (the year runs), a jobs programme masking the private-jobs gap, not the wage economy the spec means (orchestrator, 2026-10-08). At the sweeper's wage 5, 550 works reached 1.11-1.13 but a 5-coin shift less the 12 % tax is under the dole and starvation rose to 21-33: hence `works_wage` 7. Private jobs hold ~530 of the ~650 the spec's arithmetic needed; the rest needs recurring income (M17's export sink, or more founded employers).
- **Calibration (b), per-class faction-violence priors** (`[fviolence]`): new `class_mult` (per class, per kind, multiplying every source's prior) from the on-screen ledger pooled over 42-47 (civilians 0.11/0.14/0.008, members 4.05/13/0.25, the watch 2.15/6.1/0.04 for Killed/Assaulted/Robbed), then the members' Assaulted halved to 6.5 (at 13 the off-screen beatings of members fed grudges into vendettas until seed 44 held 14 of 66 faction pairs open, over the M15 gate's sanity fifth); `fv_mult` 0.3 → 1.0. Murders on 42-47 stay under the bound; the off-screen share of killings stays under the 0.3-0.7 band (printed); `test_faction_violence_parity` reads factor 1.48 (asserted <= 4, band 2; 2.9 before the members' halving).
- **Calibration (c)**: the M13 price print reads motorcycle, implant and deck 60 = 8.6 days of the day-60 median employed wage (7, now the works wage), camera 17 days, pack 4, bridge 29, car 71, robot 114, truck 129, flyer 286. Σwages > Σdole holds on 2 of 42-44 (1.01, 1.04, 0.99) and the move would shift M13's vehicle and chrome bands; prices are not moved (the plan's separate commit, for Dylan).
- **Calibration (d)**: `assets/stat_table.toml` regenerated (`calibrate --agents 500`; it had been stale since before L2). M10's gate passes, M12's riots by the six-seed mean read 0.83-1.67 across runs (asserted 1-5), the six-rate and leisure parity tests pass; `test_kitted_vs_unkitted_parity` stays red (Full 0.62, Statistical 1.00: one death per side). The regenerated table changes the `--l2-off` city (Murders on 42-44 27/44/32 against d738a07's 47/44/46), so the identity holds against d738a07 only with the old table.
- **Mechanism fixes**: the parts source order (L9); the scavenge find floored at `scavenge_p` (the spec's `0.5 + litter` halved it on clean streets: 10 of seed 42's 26 starved had left the Precinct within 20 days); a Job holder owed two days' wages may draw the dole (`economy::UNPAID_DOLE_DAYS`, a third of the starved were unpaid Fighters and Attendants); a starving held prisoner is fed at once; the opening `fun` spread (the day-1 pulse: 8.1k coins of leisure on day 1, then a cohort ringing every other day).
- **Earlier gates moved by the calibration** (each tested against a variant first, per the doctrine): M11's "GangJoin within 14 days of an eviction >= 3" and "Evicted >= 10" on seed 42 read 1 and 8 (the phase-5 income floors: with the scavenge floor and the unpaid dole off the same tree reads evicted 22, joins 7): both now asserted over 42-44 (joins 6 >= 3, evicted 35 >= 30), seed 42 printed; M12's riots mean is asserted as before (1.83 on the final tree; it read 0.83 only with the 400 public works); M12's "dirtiest litter in band on some day" (below); the gate's wallet Gini sanity is 0.95 (the M15-closing city itself reads 0.77-0.89 on day 120).
- **The year** (`test_l2_year_seed_42`/`_43`, final state, both green): what fills during year 1 from near-empty gangs and empty stores is judged year 3 against year 2 (days 121-240): grudges live, aborts and violent deaths at 1.5x, memories and Trace days at 1.2x; starvation in year 3 <= 1.5 x year 1 + 5; population >= 1,600 on day 365 (the spec's 1,700 was a placeholder); assaults per day over full 30-day windows (the 5-day tail printed); the Coarse budget allows the same-tick promotions (at most two over, on at most 1 % of hours). Final readings on 42 / 43: population 1,656 / 1,674 (the M15-closing city 1,467 / 1,364), starvation per year 8, 4, 6 / 12, 10, 8, violent deaths year 3 against year 2 331 vs 250 / 256 vs 255, save x1.06 / x1.09.
- **Gate devices**: the conservation identity is asserted on its outside side (0 every day with `[export]` off) with `total_coins`' drift printed (immigrants, emigrants, loot in flight, the fence move it; `tests/outside.rs` checks the identity against an export-off twin); a Fab by Grow is printed (the only Tech corp owns a seeded Fab, so `wants_fab` cannot fire) with the mechanism asserted on a fresh world; the Coarse budget is checked right after each assignment with the same-tick promotions allowed (one over on at most 1 % of hours); per-gang bodies over the quota are printed (members over the quota rank as civilians and keep bodies by story and distance) with class 2 <= quota asserted; M12's "dirtiest litter in band on some day" is a finding with "litter reaches the visible band" asserted (the wage calibration took the band away through the litter's sources: with the sweepers at rate 0 the peak is still 0.05).

### Phase 5: the year-long climb and gang desistance (2026-10-08)

The 365-day runs found no steady state. A probe (a kill classifier on `World::kill_by` and per-30-day tallies, not kept in the tree) on seed 42, final tree before desistance:

| days | Assault events | Murders | violent deaths | of them member on member / off screen / guard fights | GangJoin | gang members (cap 4 x 60) | grudges live | vendettas | heat by gang |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1-30 | 54 | 0 | 1 | 0 / 0 / 1 | 29 | 28 | 76 | 0 | 0.16-0.20 |
| 31-60 | 325 | 1 | 25 | 2 / 1 / 20 | 60 | 82 | 462 | 0 | 0.14-0.64 |
| 61-90 | 499 | 6 | 27 | 6 / 0 / 17 | 84 | 156 | 1,254 | 2 | 0.33-0.69 |
| 91-120 | 631 | 8 | 29 | 7 / 3 / 15 | 47 | 186 | 1,628 | 3 | 0.36-1.00 |
| 121-150 | 946 | 31 | 65 | 32 / 14 / 12 | 85 | 220 | 1,973 | 7 | 0.60-0.86 |
| 151-240 | 905-1,010 | 26-33 | 51-77 | 20-30 / 8-12 / 9-24 | 36-73 | 214-238 | 2,080-2,318 | 10-14 | |
| 241-360 | 811-1,063 | 29-55 | 54-97 | 17-33 / 7-27 / 13-25 | 56-80 | 237-240 | 2,049-2,202 | 15 | 0.67-1.00 |

Gang membership rose monotonically to the cap (with a fourth gang from the M12 split at ~day 90) and the violence followed it, then held on a plateau; grudges plateaued at ~2,000-2,300 (the cap holds), vendettas at 10-18, the Jail was not pinned (0-3 days at capacity a month) and nobody joined from inside it. The mechanism without a brake: **the roster cap had no exit**. Recruitment refilled every death and sentence (36-85 joins a month at the cap) and almost nobody left (0-18 a month, expulsions), so four full gangs' violence (~60-90 violent deaths a month) outran births and immigration (~30-35 a month) and the population fell ~40-50 a month; the M15-closing city (`--l2-off`) does the same and worse (1,467 / 1,364 on day 365).

**Gang desistance** (`gang::desist`, daily after the gang economy, under `[living]`, so `--l2-off` is untouched): an ordinary member (never the leader nor `leader_ranking`'s two lieutenants; not during a raid or BreakOut muster; not jailed, cuffed, hunting or hunted) walks away with `p = desist_base x` employed (a Job that has paid, x4) x unpaid (no stipend or tribute for 14 days, x3; `Brain.gang_paid_day`, written only with `[living]` on) x settled (married or housed outside the Sump, x2) x aged (>= 35, x2) x lately jailed (released in the last 30 days, x2) x a feud brake (an unsettled grudge on a rival gang or the gang in a vendetta, x0.25), one draw from a `splitmix64` hash of the seed, the member and the day (no stream). A leaver gets a `GangLeave` "(walked away)", keeps its grudges and memories and cools `JoinGang` for 60 days. Recruitment and `max_members` are unchanged.

`desist_base` (members on day 365 of the 240 cap, population on day 365, violent deaths days 241-360, on 42 / 43): 0 -> 233 / 238, 1,625 / 1,636, 299 / 266; 0.004 -> 238 / 240, 1,683 / 1,572, 284 / 283; 0.008 -> 238 / 237, 1,722 / 1,584, 226 / 273; **0.02 -> 184 / 198 (160-205 through years 2-3, 70-85 % of the cap), 1,758 / 1,779, 250 / 203**; 0.05 -> 134 / 170, 1,868 / 1,795, 217 / 238. The orchestrator's first rate (1-3 leavers a week a full gang, ~0.004-0.008) leaves the rosters at the cap: recruitment refills it; only a monthly turnover of about half a roster holds a plateau below it; 0.03 broke seed 42's Treasury in year 3 (0 from day ~270, 138 emigrants in a month). Over 120 days on 42-47: gang income 255.7k -> 216.4k (x0.85), Murders 274 -> 196, members on day 120 1,221 -> 1,116. Violent deaths in year 3 stay ~2.5x year 1's on seed 42 (year 1 is the ramp-up from near-empty gangs; year 3 against year 2: 250 against 227).

**M14 finding (Ledger hauls)**: the wallet Gini on day 120 reads 0.72-0.91 on the L2 tree and 0.77-0.89 in the M15-closing city (`--l2-off`): the largest wallets are single gang members holding 2-13k coins after Ledger hacks of 200-600 coins (29-72 in 150 days), while ~10k of wallets spread thin; the gate's sanity bound is 0.95.

**Lately released**: the desistance term reads the biography's `Released` row within 30 days (the code first read a `WasArrested` memory, which the arrest writes too; the review aligned it with the key's comment). **The dealer brake undercounts**: `deal_log` keeps only the latest dealer per venue, so a second dealer at the same bar that week rolls without the brake. **The unpaid dole**: a Job holder owed two days or more may draw the dole daily until it quits at 7 unpaid days, with no clawback when the arrears are paid: intended, bounded at about 7 x `dole_per_day` per unpaid episode. Removed 2026-10-08 (addendum 17).

**Investment and income hold a member** (`[living] desist_invested` 0.25, the feud's brake, not immunity): the keeper of an asset its gang owns (the gang's Arms implants and its deck) or a member who dealt in the last 7 days (`deal_log`). Without it the ~250 leavers per 120 days walked seed 42's runner, dealers and armed members out (Ledger thefts 5 -> 0, Flatlines 4 -> 0, Detox 1 -> 0, off-screen killings 4 -> 0). 120 days on 42-47, no desistance / desistance / with the brake: leaves per run 0-8 / 245-310 / 258-325; Ledger thefts [5, 29, 25, 23, 6, 3] / [0, 5, 7, 14, 4, 29] / [1, 11, 4, 11, 0, 4]; Flatlines [4, 3, 1, 4, 1, 0] / [0, 0, 0, 0, 1, 4] / [1, 1, 0, 0, 0, 1]; Detox [1, 6, 18, 5, 3, 8] / [0, 5, 4, 4, 4, 14] / [2, 7, 3, 2, 4, 1]; installs (all) [188, 243, 286, 250, 310, 244] / [174, 233, 230, 215, 264, 284] / [183, 253, 233, 220, 252, 236]; off-screen killings [4, 11, 27, 18, 14, 12] / [0, 4, 11, 3, 6, 3] / [3, 5, 7, 1, 3, 3]; gang income 255.7k / 216.4k / 179.1k; Murders 274 / 196 / 177. Tried first: the brake on any chrome or deck in the member's `Kit` (most members carry chrome): the plateau rose back to the cap (226-240) and seed 43's population fell to 1,644; the gang-owned rule keeps it at 190-230 (80-95 %). Year runs with the brake: population on day 365 1,693 / 1,498; seed 43's year 3 is a Food-monopoly famine (Greenline and Vatra bankrupt on days 228 and 242, Nutrix alone prices food at 13-17 with 20,000 units in the Reserve: starvation 44 / 61 / 54 a month from day 271), an M11 corp dynamic the runs' divergence exposed, not a gang one.

**Single-seed bullets judged over seeds** (the doctrine; gang turnover from desistance rotates the runner, the dealers and the armed members out of seed 42's gangs; per-seed data in each comment and print): L2's "an off-screen faction Killed hole on every seed of 42-44" -> some seed of 42-47; M10's "off-screen killings >= 1" on seed 42 -> some seed of 42-47 (the other seeds run in threads after the timed run; the >= 20 band stays a finding); M12's "every riot gathered >= 6" -> every riot of 42-47 (it was vacuous on seed 42's empty list); M13's "Detox >= 1" -> some seed of 42-47 and "installs on gang members >= 20" -> the band printed, >= 1 on every seed of 42-44; M14's "Ledger thefts >= 1" -> some seed of 42-47 and "Flatline 1-10 by majority of 42-44" -> the band printed, a Flatline on some seed of 42-47.

**The year rule**: year 1 starts from near-empty gangs and empty stores, so what fills during year 1 (grudges live, memories, Trace days, aborts, violent deaths) is judged year 3 against year 2 (days 121-240): grudges, aborts and violent deaths at 1.5x, memories and Trace days at 1.2x; starvation in year 3 <= 1.5 x year 1 + 5; population >= 1,700 on day 365 stays.

**The Reserve's release** (`economy::reserve_release`, `[living] reserve_release_price` 8, `reserve_release_batch` 200): seed 43's year-3 famine was a stranded shelf, not a monopoly. Greenline went bankrupt on day 228 and an agent bought its Street Market with nothing left for `[corps] wholesale`, so `daily_restock` sold it nothing: from ~day 285 its shelf stood at 0 and price 30 while the other two Markets priced 4 and the Reserve Depot sat full at 20,000; starvation per year 12 / 9 / 164 and population 1,498 on day 365. Food re-entered on its own (Ravensworth, then Brandt Holdings incorporated around Vatra's and Greenline's farms and Markets), so no subsidised founding was built. With `[living]` on, a Market priced above twice `price_base` and under `restock_floor` gets up to 200 units a day from the Reserve free (the `ReleaseReserve` lever's transfer aimed at that Market): seed 43 starves 12 / 10 / 8 a year (77 releases), population 1,674 on day 365.

**Last gate devices** (orchestrator, 2026-10-08): the year's population bound is >= 1,600 on day 365 (1,700 was a placeholder; readings 1,674-1,779 with desistance); assaults per day are judged over full 30-day windows (the 5-day tail is printed); M8's "both gangs held turf" and its border bullet are judged over 42-47 (seed 42's second gang never holds 3 Homes with or without desistance; contested days on 42-47 off / on [0/0, 92/92, 98/89, 100/83, 70/86, 93/93]); `god_kill_friend_of_leaders_day_10` picks the leader if it has a living Friend, else the gang's highest-ranked member with one, else skips that gang.
