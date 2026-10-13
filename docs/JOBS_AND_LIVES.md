# Jobs & lives: vice, the street, weapons, the Spire's workforce, ages, the gray market, faith, dress, and the JOBS_V2 leftovers

Companion to `JOBS_V2.md` (whose unbuilt half this absorbs: P4, P5b, P7, P8, the foreman, its probes and gods), `ECONOMY_V2.md`, `LIFE_L2.md`, `M16_CONTRACTS.md`, `M17_OUTSIDE.md`, `SCALE_20K.md`, `CLASS_PAY.md`, `TESTING.md`, `VISION.md`. Written 2026-10-13 against main `0c0baaf` (SAVE_VERSION 26). Identifiers marked *new* don't exist at HEAD. Every number is a *placeholder* unless it cites a measurement.

**Purpose.** The city has ~40 roles and a job for about half its adults, and almost every one of them is a shift at a counter, a vat or a door. Dylan's scope for this milestone is "add it all": the lives a cyberpunk city is actually made of (the vice trades, the street economy, guns, office work, children and the old, forgers and black clinics, churches and cults, dress) plus the industry and civic trades JOBS_V2 specified and never built. The point is not the job count. It is **ladders**: a dancer who becomes a club owner, a fixer or a junkie; a laid-off salaryman who drives a cab; a maimed fighter who begs, saves for a prosthetic and loses his mind to the load. Every wage below comes from revenue (residents' spending, the World's books, the Ambient book), every new venue can sell to Ambient demand the way a Bar does, and every feature sits behind an in-flight `enabled` switch with an env override for the byte-identical off check. Tone: systems-level; jobs, venues, coins, risk, sanity, law and reputation. Names (brands, venues, creeds) live in config and never in logic (the M17 rule).

---

## 0. Decisions

"Mine" = the spec author's, overturnable. "Dylan" = decided 2026-10-13.

| # | question | decision | why | whose |
|---|---|---|---|---|
| D1 | Scope | all nine areas, in eight phases | Dylan: add it all | Dylan |
| D2 | Trades as data or enum variants | every new trade is a `[[trades]]` row; the five `TradeModel`s without code (`handling`, `contract`, `civic`, `resale`, `day`) get it in phase 1; `TradeDuty` and `TradeModel` are appended, never reordered | JOBS_V2 § 1.3; 28 files match on `Role` today | mine |
| D3 | Self-employment | *new* `Pitch`: a venue without a building (a vendor, a street worker, a rickshaw, a tinker, a preacher), a registry per district with a per-capita cap, worked through one new `GoalKind::Pitch` | the street economy, street sex work and preachers all need the same shape; a Pitch has no room cap, so it is the overflow rung under the workplace room cap | mine |
| D4 | New building kinds | 14 appended to `BuildingKind` (§ 1-9); each gets a `[buildings.<kind>]` row loaded into one *new* `KindCfg` table (`found_cost`, `value`, roles, floors, draw letter, niche), so the per-kind `match` arms stop growing | 27 → 41 kinds; the match-arm sprawl is the real cost | mine |
| D5 | Weapons | an `AssetKind::Weapon` asset (tier 1-3, condition, stock, carried), not a `Good` unit; `Kit.weapon` is its tier; fights, raids, Hits and the law read the tier | assets already carry tier, theft, loot, stock, sale and the Kit pass; a unit count loses the tier | mine (Q1) |
| D6 | Goods appended | `Rations`, `Braindance`, `Organs` on `Good` and `ExportGood`; Power stays a flow (`Flow::Power`), not a stocked good | addendum 9's Power/Water good is the goods milestone; Rations/BD/Organs are what the new industries make and the World buys | mine |
| D7 | Legal status of vice | a `[vice]` policy block seeded into `World::levers` (P10 style): brothels licensed (property rate, tolerated), dollhouses legal for consenting contracts, street work illegal (sweeps), BDs legal except recorded crimes | levers, not switches: the utopia/dystopia game moves them | mine (Q2) |
| D8 | The chip | a `Slot::Behaviour` implant (appended) with a high `sanity_cost`, installed at hire, uninstalled at the contract's end; a captive chipped against a holding is `Crime::Trafficking` | rides `Kit.load`, `chrome::sanity_daily`, episodes and `chrome::uninstall` unchanged | mine |
| D9 | The chip's memory | shifts under a chip write no `MemoryEntry` and no mood term; `load` ticks; a `Chipped` trait (like `CampRaised`) stays after removal | the grim mechanic in one rule; the diary shows the gap | mine |
| D10 | Organs | a `Good` ripped at a black clinic or by Harvest, rotting daily, demanded by the World (`ExportGood::Organs`, capped like Data) and by Clinics for stabilisations | gives Harvest a market; the dystopian export | mine (Q3) |
| D11 | Children | no Brain; a daily **routine** (`School`, `Play`, `Chores`, `Street`, `Camp`) on `Child`, chosen from the household's state; a `School` kind on fees; a `public_school` lever off by default | no safety net; the camp stays the dystopian answer | mine (Q4) |
| D12 | The elderly | no pension; frailty from `frail_from_years` (60), output and hiring fall, kin take them in, then the Mission cot, then the street; seeding to 75 under the switch | 120-day years: nobody reaches `old_age_days` (90 y) in a run, so frailty must start earlier | mine (Q5) |
| D13 | Disability | `Body.maimed: u8` from a stabilised bleed-out, a ripped slot or a cancer; a prosthetic implant in the slot clears it (then the load) | rides `wounded`, `chrome::rip`, `health::effects` | mine |
| D14 | Creeds as data | `Creed::Config(CreedId)` appended; `[[creeds]]` rows (tolerance, tithe, recruit edges, apostasy); churches are Missions with a creed; cults are gangs with one | The Unplugged is the seed of both shapes; no creed name in code | mine |
| D15 | Fashion | `Needs.status` (`#[serde(default = one)]` like `fun`), satisfied at a `Boutique` (tailor, barber) and by the High rung; `Appearance.dress = max(derived, bought)` with the bought tier decaying; `[taste]`, `door_ok`, Hq hiring and the sighting roll read it | the brief: a dress/status need on the existing `Appearance` | mine |
| D16 | New crimes | seven appended: `Solicitation`, `Trafficking`, `ArmsDealing`, `UnlicensedArms`, `Forgery`, `Smuggling`, `BlackMarket`; petty ones sentence as Vagrancy | one per law story; no more (risk R3) | mine |
| D17 | Statistical mirror | new spending is a world-side choice in the Statistical hour (`stat_daily`'s pattern: where to eat, drink, ride), never a new table column, so no stat-table regen before the gate | a regen redraws rare rates (`thecity-calibration-noise`) | mine |
| D18 | Phase order | industry/civic/Hq/foreman → street → weapons → vice venues, the street and fashion → dollhouse and braindance → gray market and faith → life stages → gate | § 11 gives the dependency reasons | mine |

---

## 1. Shared machinery (phase 1 builds it; every area rides on it)

| piece | what | reuses |
|---|---|---|
| The switch | `[jobs2] enabled = true`, `CITYSIM_JOBS2_OFF=1`; off, no new kind seeds, no new row posts, no new flow books, no new need decays: byte-identical on 42-44 × 30 (events and the shared CSV columns). Per-area sub-switches (`[vice] enabled`, `[weapons] enabled`, `[gray] enabled`, `[faith] enabled`, `[ages] enabled`, `[fashion] enabled`) for the A/B of one area, all under the master | the `[outside] enabled` pattern |
| `TradeModel` code | `handling`: a fee per unit crossing a Yard (`Flow::Handling`); `contract`: paid per job by a client (a build, a ride, a parcel) into the workplace owner's purse; `civic`: the Treasury's payroll (`treasury::daily`'s allocation, the P5a path); `resale`: the worker buys stock at a Market and sells at `markup` (a Pitch or a stall); `day`: a one-day post (`Job.until`) | `trades.rs`; wages still through `economy::collect_wage` from the owner's purse |
| `TradeDuty` appends | `Fare` (wait at a rank, carry a rider), `Corner` (the dawn pick), `Preach`, `Sell` (a Pitch) | `Round` (the Super) is the model: a scripted bypass in `plan::plan_for` |
| The Pitch | *new* `components::Pitch { spot: TilePos, district, trade: TradeId, owner: EntityId, since, take_today, take_week, cut_to: Option<EntityId> }`, `World::pitches` (a registry per district, `pitches_per_district` per capita, `scaled_to`). A Pitch is a venue for the rung ladder (`leisure::choice` sees it as a Cheap satisfier within `pitch_reach`), for the Statistical evening and meals (`stat_daily`'s nearest-venue choice), and for the Ambient book (a district's food and visits may land on a Pitch, `ambient_keep` applies). Its day closes at midnight: `cut_frac` to `cut_to` (the gang holding the district, `District.control`) as `Flow::StreetCut` | `Venue` (`living.rs`), `Spot`/`SpotKind` (the HangOut registry), `AmbientBook` |
| `GoalKind::Pitch` | appended: walk to the pitch, stand the shift, sell, close. Scored for a jobless adult with `pitch_stake` coins, no post after `pitch_after_days`, in a district under the pitch cap; dropped when a post is held. One scripted plan like `Unwind` | `plan.rs`'s scripted plans |
| Goods | `Good::{Rations, Braindance, Organs}` appended; `stock_goods` grows; `ExportGood::{Rations, Braindance, Organs}` with their own `WorldBook`s (`[world_market]` caps per capita; the Organs cap like Data's) | `world_market.rs` |
| Flows | appended: `Handling`, `Power`, `Build`, `Tip`, `Fare`, `StreetCut`, `Licence`, `Gray`, `Tithe` | `ownership::Flow`; each goes in `flow_other` unless a column is named below |
| Kinds | 14 appended (§ 2-9); `KindCfg` from `[buildings.<kind>]`; `FOUNDABLE` becomes the table's `foundable` flag; `niche_of_kind` the table's `niche` | `founding.rs`, `corp_brain::niche_kinds` |
| Niche | `Niche::Vice` appended (strip clubs, brothels, dollhouses, BD studios); `Arms` is **not** a niche: Gunsmiths sit in `Security` | `corp_brain::demand_of` gets a Vice arm: own 7-day visits ÷ (7 × `vice_demand_ref` × own venues) |
| Scaling | every cap below is per capita through `Config::scaled_to` (`up_*`) or per building through `fit_to_map`; the hourly passes are O(pitches + venues + yards); the daily ones O(children + elders + pitches); no new per-agent per-tick read | `SCALE_20K.md` § S4 |
| Shadow | `trade_role(arch)` already maps a trade id to its role, so every new trade is an archetype for free; Pitch trades pick by `Pitch.owner` | `shadow.rs` |

---

## 2. Industry, civic and the foreman (JOBS_V2 P4, P5b, the foreman)

**Data.** Kinds `PackingPlant`, `FreightYard`, `UtilityPlant`, `ConstructionYard` (Vats, 2 floors at seed, `floors_max` 4/1/2/1). Trades: `packer` (produce, split), `loader` (handling, night), `driver` (contract, day; needs a Truck in the Yard's fleet), `plant_operator` (contract, split), `builder` (contract, day), `civic_clerk` (civic, Hall), `depot_hand` (civic, Warehouse "Reserve Depot"), `jailer` (civic, Jail, night). `Good::Rations`. Flows `Handling`, `Power`, `Build`. *new* `Building.under_construction_until: Option<Tick>`.

**Systems and hooks.**
- Packing: `trades::accrue` gains a `PackingPlant` arm: Food bought from Farms (`Flow::Wholesale`, the Farm→Yard haul) becomes Rations at `pack_ratio`; sold to the World's Rations book at `packed_premium × Food bid` (`world_market::sell`) and, under `[jobs2] rations_to_markets`, to Markets as the synth shelf's source (a Market with Rations stock cuts synth from them instead of `synth_stretch`; off by default: it moves the diet rates and parity).
- Freight: every `Flow::Export`/`Import` crossing pays `handling_per_unit` to the nearest standing Yard's owner (`world_market::sell`, `ownership::import`), and the Farm→Market haul pays `haul_per_unit` (today free). Loaders on shift gate the night crossing (no Loader, the day's exports wait; a corp with no Yard in reach pays the World's handling as today: the leak JOBS_V2 named). `p_pilfer` per Loader-shift moves a unit to the Loader (a theft the owner reports as `Crime::Theft` when `Market.sales`-style stock history shows the gap). `Hijack` is an event name only.
- Power: E13's power inputs (`wages::inputs` at the per-unit power rate) are re-pointed to the nearest UtilityPlant's owner as `Flow::Power`; the plant pays `fuel_share` of it to the World. A plant with no operator on shift for `outage_hours` writes `District.powered = false` on its districts (*new*, `#[serde(default = true)]`): `security` door tiers −1, ICE off (`virt`), night venues closed (`door_ok`), the El line slowed (`transit`). The sabotage target lands with addendum 9.
- Construction: `founding::build_on_lot`, `refit` and `add_floor` pay `Flow::Build` to the nearest ConstructionYard's owner instead of `Flow::Found` to the Treasury; the Lot holds `under_construction_until = now + crew_days × (full crew ÷ builders on site)`; Builders' `Work` targets the site (a `Round`-style duty over the Yard's open sites); day posts fill the crew at harvest and build peaks (§ 3).
- Civic: `treasury::daily`'s allocation gains Jailers (1 per `held_per_jailer` prisoners), Clerks (up to `clerks_max`), Depot Hands (`depot_hands_max`) after police and Sanitation, in that order; `law::reconcile_guards`'s pattern reconciles them. A Jailer short raises `factions::breach`'s odds (`jailer_short_mult`); the Clerk sells licences and IDs (§ 4, § 7) and is bribable (M16a's on-the-take record on a Clerk: a fake ID or a licence for `Flow::Bribe`).
- The foreman: derived, no state: `trades::foreman(b)` = the longest `hired_tick` worker at `b`; named in `Hire`/`Layoff` event text; the day-post pick ranks the foreman's known contacts first (`social` edges), which is the story ("Kessler's foreman picked his cousin").

**Owners and money.** Corps: the Food roster's copies build Packing Plants and Freight Yards (`corp_brain::build_kind_in(Food)` gains them when `demand_of` reads a World fill > `pack_fill`); a Security or Tech corp builds the Utility Plant and the Construction Yard (seeded one each per roster copy). Coins: World → Packing (Rations) → Farms (Food) → wages; exporters → Yard → wages; power payers → Plant → World (`fuel_share`) and wages; founders → Yard → wages. Civic trades from receipts (the fines-pay-the-cops story stays and prints).

**Law and risk.** Pilfering (Theft), a Loader bribed for a smuggler's crate (§ 7), an outage as a Crackdown's and a raid's opening (`attention`: a dark district distracts the Law).

**Ladders.** Day labourer → Builder/Loader (the rehire bonus, J-rules) → foreman (tenure). Packer → "a hand lost to the press": `p_press_maim` per 1,000 shifts → `Body.maimed` (§ 6).

**20k.** Yards per roster copy (`fit_to_map`); the crossing fee is O(crossings); outages O(plants).

**Out.** Water, generators by tier, the grid as a target (addendum 9); cargo hijacks as missions; strikes at the new kinds (M11's strike reads them as any workplace).

### Corp HQ and office workers (JOBS_V2 P8)

**Data.** Kind `Hq` (Spire, 3 floors at seed, `floors_max` 8), one per corp roster copy for megacorps, built by a local corp at `hq_min_buildings` (6) owned. Trade `office_worker` (overhead, day, knowledge, grade 1; `skilled_share` promotions to grade 2 per `classpay`); the `manager`, `accountant` and `broker` rows gain `Hq` in `workplace`. *new* `[competence] admin_w`, `admin_ref`.

**Systems.** `competence::` gains the `admin_share` term: `admin = clamp(Σ office staff skill ÷ admin_ref, 0, 1)`; the corp's competence (M15 W28, read by `corp_brain`, `law`, boards) adds `admin_w × admin`. `Corp.office` points at the Hq where one stands (the CEO's office hours, board votes, poaching, `Steal` heists on the Hq's node (`NodeKind::Building`) and its documents). Layoffs by `wages::shed_extra` as any overhead post (the newest first). Hiring gates on `id_tier ≥ 1` (§ 7) and `Appearance.dress ≥ hq_dress` (§ 8).

**Money.** Overhead from the corp treasury (`labour_share`; `wage_rev` reads it as cost). No new inflow: the Hq is where the hoards go to work.

**Ladders.** Office worker → manager (classpay promotion) → exec (`is_exec` succession, M17 4b's corp line) → poached (M15 W29) → laid off (hunker) → cab driver (§ 3). The Spire commute (El, cab) is the salaryman's diary.

**Out.** Rooms and elevators (addendum 10); Hq security tiers beyond the existing door tier and robots.

---

## 3. The street economy (JOBS_V2 P7 plus couriers, cabs, rickshaws, tinkers)

**Data.** Pitch trades: `vendor` (resale, day/evening, none), `rickshaw` (contract + `Fare`, day, none), `tinker` (service, day, knowledge), later `street_worker` and `preacher`. Building trades: `courier` (contract + `Round`, day, at `FreightYard` and `Fixer`), `cab_driver` (contract + `Fare`, split, at `Garage`; needs a Car in the Garage's fleet). `SpotKind::LabourCorner` appended; `Job.until: Option<Tick>`; `Trip.fare` on `World::trips`.

**Systems and hooks.**
- Vendors: the Pitch's `resale`: at open, buy `vendor_batch` Food at the nearest Market (`Flow::Wholesale`); sell meals at `price × vendor_markup` to Full agents (`EatOut` at the Cheap rung chooses a Pitch within `pitch_reach` when it is nearer than a NoodleBar), to the Statistical hour (`stat_daily`'s nearest-venue choice, no table change) and to the Ambient book (a district's food units land on Markets first, Pitches second). `Flow::StreetCut` to the holding gang. Chores: a child in the vendor's household adds `child_help` to the batch (§ 6).
- The labour corner: one spot per Yard district; at `corner_hour` (06:00) `trades::dawn_pick` fills the day's posts (Yards' open sites, Farms over `harvest_season`, the Freight Yard's crossing backlog) from adults standing at the corner, foreman's contacts first, then by `fighting`+`strength`, paid at shift end (`Job.until` ends the Job at the shift's close; the `Earn` goal gains a `GoToCorner` consideration: jobless, `coins < corner_broke`, lawfulness ≥ 0.2, a corner within `corner_reach`). Not picked = a diary line and a HangOut.
- Couriers: each corp with ≥ 2 buildings posts `parcels_per_building` a day (an abstract unit) into the nearest Freight Yard's book; a courier's `Round` walks the sender's buildings; the fee `parcel_fee` is `Flow::Handling` from the sender. A Fixer's courier carries `Deliver` records (M16b). A courier with a Motorcycle rides (`Kit.vehicle`, `life::travel`).
- Cabs and rickshaws: `life::travel` (which already picks walk vs the El) gains `Cab`: an agent with `coins ≥ fare + hotel_reserve` and a trip over `cab_min_ticks` books the nearest free driver on duty (`Fare` duty: the driver waits at a rank spot, `SpotKind::Rank`); the fare `base + per_tile` is `Flow::Fare` to the Garage's owner (or the rickshaw's own purse); at Full the ride is an M13 trip (crashes apply, `Crime::Manslaughter`); at Coarse/Statistical a timed ride (transit's `board` pattern). Ambient visitors arrive by cab at `ambient_cab_share` (a `fare` line in the book: World money for rides). Fare dodging by the lawless (`p_dodge`) → a fight at the kerb or a report.
- Tinkers: a repair site beside the Garage and the Office: chrome condition, vehicle wear and robot wear at `tinker_discount` with `tinker_fail_p` (a failed repair bricks or wrecks); buys Parts from scavengers at the scrap price (`Flow::Scavenge`'s rate, agent to agent).

**Owners and money.** Pitches are the worker's own; cabs the Garage's (Tech niche demand reads fares). Coins: residents' and Ambient spending → pitches/drivers → Markets/Garages and gang cuts. No mint.

**Law and risk.** A Pitch in a `Stance::Sweep` district is swept with the squats (`street::sweep_squats`' sibling `sweep_pitches`: Vagrancy-class, a fine, the pitch closed a day); the gang's cut is a shakedown the law can read (M16b protection records on a Pitch); a cabbie robbed by a lawless rider (`p_rob` → `Theft`, a fight).

**Ladders.** Jobless → corner → Builder; jobless + `pitch_stake` → vendor → NoodleBar owner (`Found`, `choose_kind_full` prices the NoodleBar for a vendor with `vendor_days ≥ 60` first) → corp (`incorporate_daily`); laid-off salaryman → cab driver (a Garage hire ranks `knowledge` and a clean record); gang member → pimp (§ 5) or arms dealer (§ 4).

**20k.** `pitches_per_district` per capita; the corner pick is O(standing at the corner); fares O(trips).

**Out.** Taxi fleets as corps of their own (the Garage owns them); traffic; a courier mission type beyond `Deliver`.

---

## 4. Weapons

**Data.** `AssetKind::Weapon` and `AssetClass::Weapon` appended (tier 1-3, condition, `Asset.provenance: Provenance { Clean, Stolen, Smuggled }` appended, `#[serde(default)]`); `Kit.weapon: u8` (the carried tier, the Kit pass in `assets.rs`); kind `Gunsmith` (Security niche, Mid/Civic; trade `gunsmith`, produce: Parts → a Weapon at `parts_per_weapon`, tier = the corp's tech tier, `Flow::Parts` in, `Flow::Asset` out); `Order::Smuggle` appended; `World::licences: BTreeSet<EntityId>` (*new*); crimes `ArmsDealing`, `UnlicensedArms` (severity between Dealing and Extortion; UnlicensedArms petty when no other crime is open). `[weapons]`: `weapon_w`, `kill_w`, `carry_p_by_class`, `licence_fee`, `conceal_step`, `armoury_per_member`, `weapons_per_1000` (the entity cap).

**Systems and hooks.**
- The fight: `law::fight_inner`: when either side has `Kit.weapon > 0`, `p_win += weapon_w × (w_a − w_b)` and the loser's death chance × `(1 + kill_w × w_winner)`; unarmed fights roll exactly as today (a branch, `tests/chrome.rs`'s pattern). `raid::brawl`'s side strength sums the tier; `hitmarket`'s gun credit and price read it; `captives`' Rescue and M17's Abroad cast read it as `fighting` does.
- Armouries: `weapons::gang_arms` (the `chrome::gang_arms` shape): a gang buys from a Gunsmith when legal for it (never: gangs buy from smugglers and dealers) or receives a `Smuggle` crate (below), stock at the Hideout (`AssetLoc::Stock`), issued to members at a muster (`Carried`), returned after; a raid's losers drop theirs (loot, `chrome::strip`'s path). The law's stock at the Precinct arms guards at `guard_tier`; Security corps buy for their guards (`Flow::Asset`, the Security niche's demand).
- Dealers: the stims dealer assignment in `stims.rs` is generalised over a good: a gang assigns `arms_dealers` of its loyal members to deal Weapons from the armoury at a front or a Bar; a buyer with `coins` and a `wants_weapon` consideration (lawless, hunted, in a vendetta, a gun on a Fixer's books, a street worker, a vendor who was robbed) comes to the dealer; `Crime::ArmsDealing` rolls against witnesses as Dealing does.
- Smuggling: `Order::Smuggle`: the syndicate (M17 parent) ships a crate of `crate_size` Weapons (or Stims, or BDs, § 7) to the Freight Yard at night for `crate_price` (`Flow::Remit`'s inverse: `ParentFund` in kind); a bribed Loader (`Flow::Bribe`, M16a's record) passes it; customs (`world_market::customs_rate`) seizes it with `p_seize` into the Precinct's stock (`Crime::Smuggling` on the gang's known members); corps import legally with customs.
- Carrying and the law: `law::sightings` (O(guards × near), unchanged shape): a guard who sees an armed agent without a licence files `UnlicensedArms` after a concealment contest (`security::contest_p(stealth tier + pack, guard sight, conceal_step)`); licence holders (guards, Security staff, Hq corp-class with a licence bought from the Civic Clerk for `licence_fee`, `Flow::Licence`) pass. An armed suspect raises `arrest_kill_mult` and `law_brain`'s crackdown pressure; a Crackdown doubles carry sightings.

**Owners and money.** Gunsmiths: Security corps (Militech's roster row gets `gunsmiths`); the Treasury's civic budget gains an `arms` line (police weapons); gangs' armouries from dealing and smuggling. Coins: Treasury and corps → Gunsmiths → Parts (Fabs) and wages; buyers → dealers → gang treasury (`Flow::Stims`'s untaxed shape, a `flow_arms` column); crates → the syndicate (outside).

**Risk.** Every raid, Hunt and Hit gets deadlier where guns are: Murders rise. Printed, with `weapon_w` as the lever; no earlier knob moves.

**Ladders.** Gang member → arms dealer → the gang's quartermaster (the longest-serving dealer, derived) → a Gunsmith's hire if he ever holds a clean record (never, mostly). Fabber → gunsmith (Parts skill). A street worker with a gun fights back (§ 5).

**20k.** `weapons_per_1000` caps the entities (a gang over cap issues from stock without new ones); dealer assignment is the gang's daily pass; sightings unchanged.

**Out.** Weapon types beyond tier; ammunition; ranged vs melee; armour as a separate asset (the Kit's armour term stays chrome's).

---

## 5. Vice

All of it is venues, shifts, visits, coins, risk, sanity, law and reputation. Nothing here describes an act.

### 5.1 The strip club, the brothel, the street, the companions

**Data.** Kinds `StripClub` (Mid/Sump, Vice niche, a venue: `Venue` with `front_of`), `Brothel` (Sump/Mid, Vice niche, a venue). Trades: `dancer` (exists at Club; `StripClub` added to `workplace`), `bouncer` (ditto), `barkeep` (service, evening, at StripClub/Brothel), `sex_worker` (service, split evening/night, persuasion, at Brothel), `madam` (overhead, grade 2, at Brothel), `companion` (service, High rung, persuasion, at Lounge; `dress ≥ 2` to be hired), Pitch trade `street_worker` (service, night; `cut_to` the pimp's gang). *new* `Venue.tips_today`, `Flow::Tip`, `Crime::Solicitation` (petty, just above FareEvasion). `[vice]`: `tip_share`, `visit_price` by kind, `p_assault_street`, `pimp_cut`, `sweep_pitches_per_day`, the policy block (`brothels_licensed`, `street_legal`, `dollhouse_contracts`) seeded into `World::levers`.

**Systems.**
- Venues: StripClub and Brothel join the rung ladder (`leisure::choice`: StripClub at Mid, Brothel at Cheap/Mid; the Lounge's High rung gains the companion's visit) as `Enjoy` satisfiers of `fun` and, for the Brothel and the Lounge's companion, `intimacy` (`Needs.intimacy` exists; its planner consideration gains the venue as a priced satisfier beside Court). A visit pays `price` to the owner (`Flow::Leisure`) and `tip_share × price` to the on-duty worker (`Flow::Tip`, customer → worker, untaxed): a wage top-up from the customer, not the owner. The Statistical evening and the Ambient book treat them as venues (visits by kind weights `[outside.ambient] visit_weights`).
- Fronts: `[leisure] front_kinds` gains StripClub and Brothel; `fronts_daily` builds them; dealers deal inside; the leader's `Collect` takes `take_week`.
- The street: a `street_worker` Pitch at night in a district; a pimp is a gang member assigned by the gang's daily pass (the dealer assignment again) to a held district's pitches: he stands at the spot (`GangWork` with a target), the pitches pay `pimp_cut` (`Flow::StreetCut`) and he "protects": per worker-shift `p_assault_street × district fear × (1 − pimp present) × (1 − Kit.weapon > 0)` rolls an assault by a Statistical stranger (`fviolence`'s cell off screen, `resolve_fight` with a materialised stranger at Full); a pimp present answers with the gang's fight (a grudge, a Hunt if it goes badly). A worker who was robbed or beaten buys a weapon (§ 4) or joins a Brothel (safer, a madam's cut instead of a pimp's).
- Sweeps: `street::sweep_pitches` in `Stance::Sweep` districts: `Crime::Solicitation`, a fine (`Flow::Fine`), the pitch closed `sweep_close_days`; `sweep_pitches_per_day` per capita caps the flood (R3). With `street_legal` on, no sweeps (the utopia lever).
- Companions: the Lounge's second role; a visit at the High rung; execs and the rich (`class Spire`, `wealth ≥ 0.8`); `tip_share` is the companion's income; the exec's "regular" becomes a known contact (`social` edges from shared venue time), which is where leverage lives (M16b blackmail on a secret: a companion knows things).

**Owners and money.** Agents found StripClubs and Brothels (`FOUNDABLE`; a founder who worked as dancer/sex_worker/madam for `vice_days` or holds persuasion ≥ 0.6: `founder_ok`); gangs as fronts; Vice-niche corps (a seeded roster row, placeholder brand in config) at the Mid tier. Coins: visitors' wallets and the Ambient book → owner (wages, rent, property rate when licensed) and workers (tips); pitches → pimps' gang.

**Law and reputation.** Licensed brothels pay the property rate and are swept only under Crackdown; unlicensed (a front, `brothels_licensed` off) are raided as fronts are (Crackdown closes fronts today). A worker's `Regard` in Purist and church taste is low (`[taste]` rows); a companion's high in Spire taste.

**Ladders.** Dancer → club owner (Found) / fixer (networks at a Fixer, M16a `Contract` goal: persuasion sells) / junkie (the front's dealer: `GetHigh` at the workplace, addiction, Detox or the street) / companion (dress ≥ 2, the Lounge). Street worker → brothel (a madam's hire: the safer rung) → madam (classpay promotion at `promote_days`) → owner; or → debt (`Debt` records from the Hit market's credit, leverage's demands) → the dollhouse contract (§ 5.2). Pimp → gang lieutenant (the cut counts as income the succession's merit reads, M17 4b).

**20k.** Venues per roster copy; pitches per capita; the assault roll is per worker-shift (hundreds a night at 20k, one hash each).

**Out.** Disease; romance edges from visits; any act.

### 5.2 The dollhouse and the chip

**Data.** Kind `Dollhouse` (Mid/Spire, Vice niche, a venue at the High rung; owned by a Vice corp, a gang front or a megacorp branch; never founded by an agent). Trade `joytoy` (service, split; hired under a *new* `Job.chip_until: Option<Tick>` contract of `chip_days`). `Slot::Behaviour` appended; an `AssetKind::Implant(Behaviour)` row in `[chrome]` with `sanity_cost` `chip_cost` (the highest of any slot), `install_fee` paid by the owner (`Flow::Asset` to the Clinic; a gang's at a black clinic, § 7), `visible` 0. *new* `Trait::Chipped` beside `CampRaised`. `Crime::Trafficking` (severity just above Abduction).

**Systems.**
- The hire: the venue posts joytoy places; the hire pool is the desperate (`relief_hire`'s broke and homeless, debtors, CampRaised adults, street workers) ranked by `appearance.dress`/`chrome` taste; a hire walks to the owner's Clinic and is chipped (`chrome::install` with `chip` provenance); the contract pays `wage × chip_mult` (above a dancer's: the lure) with `chip_days` fixed.
- The shift: `exec` writes no `MemoryEntry` for actions at a Dollhouse while `Job.chip_until` stands, and `mood`'s daily pass skips the day's work term; `Kit.load` carries the chip's cost into `chrome::sanity_daily` as any implant; episodes (`start_episode`) at the venue are the scandal (a Feed story, a bouncer's fight, the law) and end the contract early.
- The end: at `chip_until` the owner pays the uninstall (`chrome::uninstall`), the `Chipped` trait stays (a permanent `sanity` floor cut `chipped_sanity`, a `Grief`-class memory of the missing days, affinity −0.2 to the owner's faction as a grudge cause `Chipped`), and the worker walks away with the savings (the one rung up: a chipped year buys a Lot).
- Who gets chipped against their will: a captive (M16b `Abduct` with the Dollhouse as the holding) is chipped on arrival by a gang owner: that is `Crime::Trafficking` on the gang's members the law can name, a Rescue target (M16b missions), a Feed story when exposed, and the thing a Purist Patrol hates most (a Patrol scan at the door flags chipped staff). A corp dollhouse never holds captives (`dollhouse_contracts` lever; the corp's board can vote it off, § M16b governance).

**Owners and money.** Vice corps, megacorp branches (a Spire dollhouse for the execs), gang fronts. Coins: visitors (High rung), the Ambient book's High-rung visits, the installs to Clinics, the wages to the chipped, the Rescue and Hit market around captives.

**Ladders in and out.** Street worker/debtor/CampRaised → chipped → paid out (Lot, a NoodleBar) / episode (Treat, the Purists, Harvest victim: the chip is chrome) / rescued (a grudge, a Hunt on the owner).

**20k.** Dollhouses per roster copy; the memory skip is a flag read at the diary write, no new pass.

**Out.** The chip as a weapon on anyone else (a behaviour chip on a board member is the Daemons/leverage milestone's question); any act.

### 5.3 Braindance studios

**Data.** Kind `BdStudio` (Mid, Vice or Tech niche: config decides); trades `bd_editor` (produce, day, knowledge), `bd_actor` (service, evening, persuasion). `Good::Braindance`, `ExportGood::Braindance`. *new* `Deed.recorded: bool` on M15's deed records. `[vice] bd_per_session`, `arcade_needs_bd` (off), `black_bd_price_mult`.

**Systems.**
- Legal: an actor's shift is a session; editors turn sessions into `Braindance` units (`trades::accrue` arm); sold wholesale to Arcades (`Flow::Wholesale`; with `arcade_needs_bd` an Arcade's visits need stock, else today's priced `Enjoy` stands) and to the World's book (export demand; Ambient visits at Arcades rise with stock).
- Black-market: a gang studio (a front; `front_kinds`) records its members: a deed (`Murder`, `Assault`, a Hit fulfilled, a raid) by a member carrying a *new* `Recorder` (an `AssetKind` appended; carried) sets `Deed.recorded`, and the studio's editor turns it into a black `Braindance` unit (a second `stock_goods` slot? no: one good; `Building.black_bd: u16` counts the black share) sold by dealers (the generalised dealer, § 4) at `black_bd_price_mult`; `Crime::Dealing` applies, and the recording is evidence (the law's `Conspiracy` path reads a recorded deed as a witnessed one). The Hit market's buyers with `recorded` pay more (a `hitmarket` price term): a hook for M18's "snuff" story, kept abstract.

**Money.** Viewers and the World → studios → actors, editors; black units → gang treasuries.

**Ladders.** Dancer → actor → editor (knowledge); runner → editor; gang member → cameraman (the Recorder carrier: the one who never fights).

**Out.** Braindance as a Virt overlay (M14 unchanged); BD addiction (a stim-like meter) is the late calibration's if wanted.

---

## 6. Life stages

### 6.1 Children

**Data.** `Child.routine: Routine { School, Play, Chores, Street, Camp }` appended (`#[serde(default)]`), `Child.edu_days: u16`; kind `School` (Mid/Spire/Civic; trade `teacher`, contract, day, knowledge; fees `school_fee` a day paid by the household head, `Flow::Contract`); `levers.public_school` (a P10 lever, off: the Treasury pays the fees in a district when on). `[ages] child_theft_p`, `child_beg_coins`, `child_help`, `street_take_days`.

**Systems.** `demography::children` (daily) picks the routine: a household that can pay the fee and has a School in reach → School (`edu_days` → `classpay` education at `edu_gain_per_day × school_mult`); a household with a Pitch or a shop → Chores (`child_help` to the pitch's batch, the shop's service); a housed child otherwise → Play (the district's street density ledger gains children per hour, addendum 13); a homeless household's child → Street: begging (`child_beg_coins` a day from the district's Statistical passers-by, `Flow::Scavenge`'s shape, agent-side, no mint: the givers' wallets pay) and petty theft (`child_theft_p` → `Theft` reports; the law's answer is `camp::take` with a second trigger, `street_take_days`); a camp child → Camp (today's ledger shift). Coming of age at 18 keeps the routine's mark: School → education; Street → `CampRaised`-like `StreetRaised` (stealth +, lawfulness −); Camp as today.

**Money.** Fees are a sink for the housed (the Spire's school is the status good; `status` need, § 8); street children's coins come from wallets.

**Ladders.** School → Hq (education gates grade 2) ; Street → gang (`JoinGang` at 18 with the trait) / joytoy / the corner; Camp → the camp's owner's Fab.

**Out.** A child Brain; child labour outside the camp; schools as a corp business beyond fees.

### 6.2 The elderly

**Data.** `[ages] frail_from_years` 60, `frail_slope`, `seed_max_years` 75 (under the switch), `elder_fire_mult`, `kin_take_in_p`. `health::effects` gains `old_work` and `old_fight` from age (today `old_mult` scales frailty only).

**Systems.** From `frail_from_years`: work output × `(1 − frail_slope × years over)` (`trades::accrue`, the Farm/Fab accruals, service hours), hiring rank −, `sick_fire_days` reached sooner (`elder_fire_mult`); no pension, no dole: an elder keeps a post while it pays. Fired: Beg (exists), a preacher's Pitch (§ 7), a vendor's; kin: `demography`'s daily pass moves a jobless elder with an adult child who has a Home (`is_child_of`) into it with `kin_take_in_p` (a `Household` merge through `ownership::rehouse`; affinity reads it); then the Mission cot, the shanty, the street (`Vagrancy`); old-age death `old_age_p_per_day` from `old_age_days` as today, plus the cancer hazard's age term. The Trauma Team's price and the Clinic's are the same for everyone: no concession.

**Ladders.** Worker → fired → beggar/preacher → kin → cot → street. The rich elder: a Lounge regular until the money runs out (no story change; the exec's pension is his treasury).

**Out.** Inheritance beyond today's wallet pass; retirement as a choice.

### 6.3 Disability

**Data.** `Body.maimed: u8` (0-3; `#[serde(default)]`), set by `wounded::stabilise`'s survivor roll `p_maimed`, by `chrome::rip` on a living victim (the ripped slot), by a cancer in remission (1), by the press (§ 2). `health::effects` gains `maimed_work`, `maimed_fight`, `maimed_walk` per level.

**Systems.** `Kit.walk_mult`, `fighting` and work output read it; hiring ranks it; a prosthetic (`Implant` in the maimed slot) clears the level and adds `sanity_cost` (the chrome loop); Beg's giver roll × `maimed_beg_mult` (the bowl). The Clinic sells the prosthetic at the implant's price; the black clinic cheaper with its failure roll.

**Ladders.** Fighter → maimed → beggar → chrome → load → episode → the Purists or the Jail; a maimed loader → tinker (a Pitch needs no legs).

---

## 7. The gray market

**Data.** Trades: `forger` (service, day, knowledge+deception, 2nd role at `Fixer`), `black_doc` (service, split, knowledge, at *new* kind `BlackClinic`), `organ_broker` (resale, day, at BlackClinic). `World::ids: BTreeMap<EntityId, IdState { tier: u8, fake: bool }>` (*new*; absent = tier 0: no papers), `Good::Organs`, `ExportGood::Organs`, `Asset.provenance` (§ 4) with `Ripped`. Crimes `Forgery` (Fraud-level), `Smuggling`, `BlackMarket` (Dealing-level: unlicensed practice, organ brokerage). `[gray]`: `id_price[tier]`, `id_w`, `black_discount`, `black_fail_p`, `organs_per_body`, `organ_rot`, `p_struck_off`.

**Systems.**
- IDs: the Civic Clerk (§ 2) issues tier-1 papers to residents for `id_fee` (`Flow::Licence`); immigrants, Outsiders (M17), CampRaised and StreetRaised adults arrive at tier 0. What papers do: corp-only and Hq rows need `tier ≥ 1`; a licence (§ 4) needs tier 1; the law's sighting of a wanted agent passes with `p × (1 − id_w × tier)` when the papers are fake and unexposed (`law::sightings`' recognise roll). A forger sells fake tier-1/2 papers at `id_price` (`Flow::Gray`, untaxed); exposure: a Clerk's or employer's check is `security::contest_p(forger's knowledge tier, checker's knowledge, step)` at the hire, at a licence purchase and at an arrest; exposed = `Forgery` on the holder and (if the gossip names him) the forger; the papers are void. Buyers: the hunted (`hunt`'s hunted walk to the Fixer, the `Commission` plan's shape), the wanted, immigrants without a job after `id_wait_days`.
- Black clinics: a `BlackClinic` sells installs at `install_fee × black_discount` with `black_fail_p` (bricked, or `Body.maimed`), therapy and detox the same, pays no property rate and no treatment tax, and **buys ripped chrome** (`chrome::buy_back` at a licensed Clinic now refuses `Provenance::Ripped`, under the switch; the Harvest crews' sales move here) and organs. Staff: `black_doc` (a struck-off Ripperdoc: `p_struck_off` per patient death at a Clinic removes the licence (`Trait::StruckOff`), and the licensed Clinics no longer hire him) and the broker. Founded by a struck-off doc or a gang (`FOUNDABLE` for agents with the trait; a front kind for gangs). The law: `BlackMarket` when a raid or a sweep finds it (a Crackdown's fronts pass); a Purist Patrol's scan.
- Organs: `chrome::rip` on a body at a black clinic (or a Harvest's rip at the Hideout) yields `organs_per_body` units into the building's stock, rotting `organ_rot` a day; demand: the World's Organs book (export through a Freight Yard at night, `Smuggling` risk) and licensed Clinics (`wounded::stabilise` and the cancer cure consume one unit at `p_organ_needed`; a Clinic without stock fails the save: Trauma Team buys from brokers, quietly: a `Flow::Gray` the Feed can expose). `chrome::harvest_target`'s value adds the organ bid, so Harvest's score reads the market.
- Smuggling: § 4's `Order::Smuggle` over goods (Weapons, Stims, Braindance in; Organs out); the syndicate's crate, the Loader's bribe, customs.

**Money.** Buyers → forgers → Fixer cut; patients → black clinics; the World → brokers → gangs (a mint, like the fence's, printed as `flow_organs`); Clinics → brokers.

**Ladders.** Runner/reporter → forger (knowledge+deception); Ripperdoc → struck off → black doc → broker; gang member → Harvest crew → broker's supplier; an immigrant with fake papers → Hq (the exposure story).

**20k.** IDs are a map; organs a stock; nothing per agent.

**Out.** Identity theft of a named agent; a Soylent-style factory (addendum 2: a later goods milestone; the organ book is its hook).

---

## 8. Faith

**Data.** `Creed::Config(CreedId)` appended; `[[creeds]]` rows (`id`, `label`, `tolerance` (chrome, as the Purists'), `tithe_frac`, `recruit_edges`, `apostasy_grudge`, `violence_bias`, `drug_policy`, `chapel_district`, `church: bool`); kind `Church` (a Mission with `Building.creed`; `charity` purse; trade `priest`, contract paid from the purse, day); Pitch trade `preacher` (`Preach` duty, paid by givers); `Flow::Tithe`; `GrudgeCause::Apostasy`; `Brain.creed: Option<CreedId>` (*new*, a follower's faith; a gang member's is the gang's).

**Systems.**
- Churches: the Mission's machinery (meals, cots from the purse; `charity::`), plus `Preach` at the door daily (the Purists' `leisure::preach` generalised to any creed): attendance is a Free-rung `Unwind` satisfying `belonging` and a little `fun`, open to all; a follower (`Brain.creed`) tithes `tithe_frac` of each wage (`Flow::Tithe` into the purse at `collect_wage`), so the kitchen runs on the flock's wages and donations (addendum 17; a player's `Donate` is the lever the district feels: starving and Dregs fall where the purse is fat, printed per district as `charity::` already prints). Conversion: an attendee whose `belonging < convert_belonging` or who holds a `Grief` memory converts with `p_convert` keyed on the Preach draw. Priests from the followers (persuasion).
- Preachers: a Pitch at a HangOut spot: Preach draws givers' coins (`p_give × the spot's crowd`), converts at a lower rate, feeds the rumour mill (a Preach is a `Chat` for `gossip`).
- Cults: a gang with a creed row: recruits along `recruit_edges` (M15's Purist rule) and from the lonely and grieving at its Preach; tithes `tithe_frac` (1.0 for the brutal rows: the members are broke by design, housed at the chapel); expels on the row's taboos (chrome, stims, a job outside); an apostate (a member who leaves) gets a grudge (`Apostasy`) from the leader, which `hunt` reads like any grudge, so cults hunt leavers at `hunt_min`; `violence_bias` tilts the faction brain's `Contest`/`Raid` scores (the Purist Patrol is one row's duty). Seeded like The Unplugged (`creeds::seed_chapel` per row, per `seed_copies`); a row's `chapel_district` places it. The M17 Blackwall cult is a row (`blackwall = true`: its Preach targets runners; its breach is M17's).

**Money.** Wages → tithes → purses → meals, cots, priests. Donations as today. No mint.

**Law and reputation.** A church is lawful; a cult's violence is a gang's; `[taste]` rows per creed read `Appearance.chrome`.

**Ladders.** Grieving widow → follower → priest; CampRaised → cult → expelled → hunted; a cult member → the Purist Patrol's scanner.

**Out.** Theology; a creed's politics beyond `violence_bias`; a church as a board (governance) is M16b's if wanted.

---

## 9. Fashion and status

**Data.** `Needs.status: f32` (`#[serde(default = one)]`, decays `status_decay × pride × class_w[class]` daily); `Appearance.bought: u8`, `bought_until: Tick`, `groomed_until: Tick` (`#[serde(default)]`); kind `Boutique` (Mid/Spire; trades `tailor` (service, day, persuasion), `barber` (service, day, none)); `ShopPick` gains `Dress(tier)` and `Groom`; `[fashion]`: `dress_price[tier]`, `dress_days`, `groom_price`, `groom_days`, `hq_dress`, `lounge_dress`, `status_w`.

**Systems.** The daily appearance pass (M15) sets `dress = max(derived from class and wealth, bought while bought_until stands)`; `groomed` adds +1 to the taste read while it stands. Satisfiers: a Boutique visit (`Shop` with `Dress`/`Groom`: the Cheap rung is a barber, Mid a tailor, High the Spire tailor), a High-rung evening, visible chrome for gang taste (`[taste]`). Readers: `[taste]` (every social move's regard), `leisure::door_ok` (the Lounge's bouncer: `dress ≥ lounge_dress`), Hq and companion hiring (`hq_dress`), the sighting roll (a `dress 0` body in the Spire raises a guard's stop: `p_sight × spire_dress_mult`), the Harvest's target value (chrome already). `status` feeds mood like `fun` does (`status_w`), so the rich spend (the status-goods sink the class-pay memory asks for) and the poor's low status is one more pressure. The Ambient book gains a `grooming` line (visits at Boutiques).

**Money.** Wallets and the Ambient book → Boutiques → wages, Parts (a tailor buys cloth as Parts at the scrap rate: no new good).

**Ladders.** Dancer/companion → tailor (persuasion); a Boutique owner → Spire regular.

**Out.** Faction uniforms as disguises (the emergence doctrine's security tiers; the dress term is the hook); clothing as a good.

---

## 10. Lives: the ladders and the goals that climb them

The ladders are not scripted. Each rung is a goal an agent already scores (or one of the three appended here), with considerations over needs, wallet, skills, class, traits, district and what the agent knows. The table is the design contract for the behaviour tier.

| from | to | the goal and its considerations |
|---|---|---|
| jobless, broke | the labour corner | `Earn`'s `GoToCorner`: jobless, `coins < corner_broke`, a corner within reach, lawfulness ≥ 0.2; the dawn pick |
| jobless, `pitch_stake` coins | a vendor / rickshaw / tinker / preacher pitch | `Pitch` (appended): days jobless ≥ `pitch_after_days`, the district under cap, skill match (knowledge → tinker, persuasion → preacher, none → vendor); dropped on a hire |
| vendor 60 days, savings | NoodleBar owner | `Found` (`choose_kind_full` ranks the kind the agent worked) |
| day labourer | Builder / Loader | the Yard's hire (`rehire_bonus`, the foreman's contacts) |
| Builder | foreman | tenure (derived) |
| office worker | manager / exec | `classpay` promotion; succession |
| office worker laid off | cab driver | the Garage's hire ranks knowledge and a clean record; `Work` |
| dancer | club owner / fixer / junkie / companion | `Found` (persuasion, savings); `Contract` (networks at a Fixer); `GetHigh` (the front's dealer, addiction); the Lounge's hire (dress ≥ 2) |
| street worker | brothel / armed / dollhouse | a madam's hire (`relief_hire`'s pool); `Shop(Weapon)` after a robbery memory; the chip contract after a `Debt` |
| chipped | paid out / episode / rescued | the contract's end; `chrome::sanity_daily`; a Rescue record |
| gang member | pimp / arms dealer / cameraman / smuggler's loader | the gang's daily assignment (loyalty, lawfulness, `fighting`) |
| Ripperdoc | struck off → black doc → broker | `p_struck_off`; `Found(BlackClinic)` |
| runner / reporter | forger | the Fixer's 2nd role hire (knowledge + deception) |
| widow, lonely | follower / priest / cult member | `Unwind` Free rung at a Preach; `p_convert`; the cult's recruit edges |
| cult member | apostate, hunted | leaves on a taboo; the leader's grudge; `Hunt` |
| child (School / Street / Camp) | Hq / gang / the corner | education; `StreetRaised`; `CampRaised` |
| worker over 60 | fired → beggar → kin → cot → street | `sick_fire_days`; Beg; `kin_take_in_p`; the Mission; Vagrancy |
| fighter | maimed → beggar → chromed → episode | `p_maimed`; Beg × bowl; `Shop(Implant)`; the load |
| anyone with coins | dressed → the Lounge / the Hq | `Shop(Dress)` on `status`; `door_ok`; `hq_dress` |

Appended `GoalKind`s: `Pitch` only. `Shop`, `Earn`, `Work`, `Found`, `Unwind`, `GetHigh`, `Treat`, `Contract`, `Commission`, `Hunt`, `JoinGang`, `Squat` carry the rest. The job search at midnight (`jobs::top_up`'s hire pool, `life`'s search) ranks a Pitch below a post and above nothing, so a hire ends a pitch and a layoff opens one: the rung between employed and jobless that the city lacks today.

---

## 11. Phases (each a commit; the orchestrator merges each; every phase runs the trio, `core_year`, shadow, the gods, parity and `core_20k`, measures 42-47 × 120 at 2k and 42 × 30 at 20k, and checks the off identity)

| # | phase | builds | identity check | why here |
|---|---|---|---|---|
| 1 | **Industry, civic, the Hq, the foreman** | § 1's scaffolding (the switch, `TradeModel` code, `KindCfg`, Goods, Flows), § 2 (PackingPlant, FreightYard, UtilityPlant, ConstructionYard, Rations, the civic three, the foreman) and the Hq with `admin_share`; SAVE 28 | `[jobs2] enabled = false` byte-identical | the leftovers are specced and the scaffolding every later phase needs; the Yards give the corner (2) its posts and the smugglers (3, 7) their door |
| 2 | **The street** | the Pitch and `GoalKind::Pitch`, vendors, the labour corner (`Job.until`), couriers, cabs and rickshaws (`Fare`), tinkers; SAVE 29 | `[jobs2] street = false` | the Pitch is the shape vice (4), preachers (6) and children's chores (7) reuse |
| 3 | **Weapons** | the asset, the Kit term, the fight/raid/Hit terms, Gunsmiths, armouries, dealers over goods, `Smuggle`, licences, carry sightings; SAVE 30 | `[weapons] enabled = false` | the street worker's danger (4) and the gray market's crates (6) read it; the generalised dealer serves BDs (5) |
| 4 | **Vice venues, the street and fashion** | StripClub, Brothel, companions, tips, fronts, `street_worker` pitches, pimps, sweeps, the `[vice]` levers; `status`, Boutiques, the dress readers; SAVE 31 | `[vice] enabled = false`, `[fashion] enabled = false` | the Lounge's companion needs the dress gate, so fashion ships with it |
| 5 | **The dollhouse and braindance** | the chip (`Slot::Behaviour`, `Chipped`, the memory skip, contracts, captives, `Trafficking`), BdStudios, legal and black BDs, the Recorder; SAVE 32 | `[vice] chip = false`, `bd = false` | needs the venues (4), the dealer (3) and the black clinic's install path is optional until 6 |
| 6 | **The gray market and faith** | IDs and forgers, black clinics and struck-off docs, organs and the book, smuggling over goods; creeds as data, churches, preachers, cults, tithes, apostasy; SAVE 33 | `[gray] enabled = false`, `[faith] enabled = false` | reads weapons (3), pitches (2), the chip's black install (5) |
| 7 | **Life stages** | children's routines and the School, the elderly, disability; SAVE 34 | `[ages] enabled = false` | reads pitches (chores), Beg, kin, the Mission, chrome; last because it moves the seeded population (ages) and the diet rates |
| 8 | **The gate** | the `core_sanity` bullets and demotions, unit tests, probes, the 14 archetypes with diaries, the god scenarios, `analyze_run.py` sections, the one stat-table regen (D17) with the Murders check, docs ("Implemented: deviations"), then `/code-review high` and a fix commit | — | TESTING.md's cadence |

Reordering against the brief: the Hq moved from "Corp HQ and fashion" into phase 1 (it is JOBS_V2 P8, an overhead trade on the scaffolding, nothing else reads it); fashion moved into the vice phase (the companion's dress gate); the dollhouse and braindance are their own phase (the chip touches chrome, exec, mood and captives at once and deserves its own review).

---

## 12. Acceptance (TESTING.md form)

Asserts catch broken; behaviour judges alive; numbers are reports. Measure every candidate bullet on 42-47 first; a mechanism on 2 or fewer of 6 seeds gets a unit test, never a bullet.

### `core_sanity` bullets (candidates; the per-phase measurement decides)
- a Trade hire of each new `TradeModel` (`handling`, `contract`, `civic`, `resale`, `day`) — phase 1
- a Rations export, a handling fee, a power payment to a plant, a build paid to a Yard, a Jailer on shift, a licence sold — 1
- a Pitch opened, a vendor sale, a day post filled, a cab fare, a parcel delivered, a street cut paid — 2
- a weapon made, a weapon carried into a fight, an armoury issue at a muster, an `ArmsDealing` report, a crate landed — 3
- a tip paid, a StripClub visit, a Brothel visit, a companion's visit, a `Solicitation` sweep, a Boutique visit — 4
- a joytoy hired and chipped, a chip contract ended, a BD unit made, a BD sold wholesale — 5
- a tithe, a Preach attended by a non-member, a conversion, a fake ID sold, a black install, an organ unit made — 6
- a School day, a Street-routine child's coin, an elder taken in by kin, a `maimed` set — 7
- the coin identity and `escrow_leak` unchanged; the collapse bounds unchanged; the 4,000 tps floor; `core_20k` per capita with its printed floor

### Unit tests (rare or mechanism-level, in seeded worlds)
`trades.rs`: each model's conservation over a day (a handling fee is paid once per unit; a day post ends at its shift; a resale pitch's purchase and sale balance); the corner pick ranks the foreman's contacts; a Pitch under the district cap. `weapons.rs`: an unarmed fight rolls byte-identically to main; an armed one moves `p_win` by `weapon_w`; a seized crate lands in the Precinct; a licence passes the sighting; concealment contest at each tier. `vice.rs`: a tip is customer → worker and untaxed; a sweep in a Sweep district only; a pimp present zeroes the assault roll's pimp term; a chipped shift writes no memory and ticks `load`; the contract's end uninstalls and leaves `Chipped`; a captive chipped is `Trafficking`; a recorded deed makes a black unit. `gray.rs`: a fake ID passes a hire and fails a Clerk's check at the tier contest; a licensed Clinic refuses `Ripped`; organs rot; a Trauma save consumes a unit. `faith.rs`: a tithe at `collect_wage`; a conversion at the Preach draw; an apostate's grudge reaches `hunt`. `ages.rs`: the routine choice per household state; a School day raises education; `street_take_days` takes a child; an elder over `frail_from_years` loses output; kin take-in merges the household; `p_maimed` on a stabilisation; a prosthetic clears the level. `fashion.rs`: `dress = max(derived, bought)`; the bought tier expires; the Lounge door reads it. Saves: a round trip with every appended variant; a pre-milestone save loads with every default.

### Person probes (`tests/person.rs`)
- JOBS_V2's five (laid off finds work; a Night Porter and a Loader keep their shifts; the offer is real; the homeless hire is housed; the Super walks the Blocks), un-ignored.
- a jobless adult with 30 coins and no post for 5 days opens a vendor pitch within 2 days and is at it by 11:00; hired the next midnight, he closes it.
- a broke jobless adult beside a corner is in the queue at 06:00; picked, he is paid by the shift's end; not picked, he hangs out.
- a Spire office worker with the fare takes a cab home after 22:00 and arrives before the walker would.
- a street worker robbed last night buys a weapon from a dealer within 3 days (control: never robbed, buys none).
- a dancer with 400 coins and persuasion 0.7 founds a StripClub within 14 days (control: persuasion 0.2 saves).
- a debtor offered the chip contract takes it when `coins < debt`; the same agent with the debt paid refuses.
- a wanted agent with a fake ID walks past a guard's sighting more often than the same agent without (pooled over 20 runs of the roll).
- a widow attends a Preach within a week of the death and converts under the row's rate; her tithe shows at the next wage.
- a 64-year-old Farmer fired for sick days moves in with his adult daughter within 7 days (control: no daughter, the Mission cot).
- a child of a vendor is at the pitch (Chores) on a weekday; the same child with a School in reach and the fee paid is at the School.

### Shadow archetypes (diaries first; bounds only after the eight-run calibration ensemble)
`dancer` (bounds exist; re-read), `joytoy`, `street_worker`, `pimp`, `vendor`, `day_labourer`, `cabbie`, `courier`, `office_worker`, `arms_dealer`, `forger`, `preacher`, `elder`, `child` (the pick exists; its diary gains the routine), plus `loader`, `builder`, `foreman`, `madam`, `companion`, `black_doc`, `cultist` as diaries only. Metrics as today (`worked`, `paid/wkd`, `walk_h/d`, `known/wk`, `e0_h/wk`), plus printed `tips/wk`, `take/d` (pitches), `swept/wk`, `rides/d`.

### God scenarios (60 days, each against its control; one per reaction class)
- `god_kill_workers` (JOBS_V2: 200 employed killed on day 45): offers arrive, employment back to ≥ 90 % of the control's by day 60; the corner fills.
- `god_outside_boom` (JOBS_V2: `outside_wage` × 0.5): immigration rises.
- `god_power_cut` (every UtilityPlant's staff jailed 10 days at day 30): `powered` false in their districts, door tiers down, a raid or a run lands on a darkened building more than in the control.
- `god_flood_guns` (a crate of 200 tier-3 weapons to the weakest gang on day 20): its raid wins rise, Murders rise, the Law's crackdown pressure answers.
- `god_vice_legal` (`street_legal` on, `brothels_licensed` on, day 20): sweeps stop, pitches rise, the pimps' cut falls, the Treasury's fines fall (the utopia lever).
- `god_chip_everyone_in_debt` (every debtor offered the chip, day 20): chipped count, episodes, Rescues, Trafficking reports.
- `god_organ_boom` (the Organs appetite × 5, day 20): Harvest orders rise, black clinics found, Abduction reports rise.
- `god_cult_in_sump` (a `tithe 1.0` cult seeded with a charismatic leader, day 10): members, apostates, Hunts; the Purists react.
- `god_school_free` (`public_school` on, day 10): camp intakes fall, education rises, the Treasury pays.
- `god_kill_the_young` (every adult under 35 killed, day 30): elders work on, kin take-ins, the corner empties, the city's output falls and does not collapse.

### Printed calibration bands (`tools/analyze_run.py`, never asserted)
the employed share (J24) and the homeless share (J25) by phase; the J24/J25 effect of each sub-switch (A/B per phase); jobs by `TradeModel` and by kind; pitches open and their take; day posts filled ÷ offered; vice share of gang income (`StreetCut` + fronts ÷ gang inflow); tips ÷ vice revenue; weapons carried per 1,000, armed fights ÷ fights, Murders with and without `[weapons]`; licences sold; sweeps and `Solicitation` arrests per day; chipped living, contracts ended, episodes at venues; BD units legal ÷ black; fake IDs living, exposures; organs made, exported, used by Clinics; tithes ÷ wages, followers by creed, apostates hunted; children by routine, School days ÷ child-days, camp intakes; elders employed, taken in, on cots; maimed living; `status` mean by class, Boutique visits; each new Flow's daily mean and the net World crossing (Rations, BD, Organs books in; `fuel_share`, crates out); ticks/s at 2k and 20k per phase.

---

## 13. Risks

| risk | what could go wrong | the answer |
|---|---|---|
| R1 jobs vs the room cap | `wages::staff_room`'s workplace room cap (kept, Dylan 2026-10-11) means the new building trades add posts only with new buildings and floors; the Lots are finite (100 added in JOBS_V2 P2) | the industries seed on Vats ground (their own Lot quota in `gen_map`), the Hq on Spire Lots; Pitches have no room cap by design (the overflow rung); print places by kind and the Lots left; never lift the cap to pass a band |
| R2 money circuits | three new World books (Rations, BD, Organs) are mints on top of M17's finding (coins +45 % per 60 days); tips and fares are transfers, fine; `fuel_share` and crates are leaks | caps per capita on each book like Data's; print the net crossing per phase; the late calibration milestone sets the appetites; no earlier knob moves |
| R3 the hydra of crime | seven appended crimes could flood the Jail (160) and the law's attention; `Solicitation` sweeps and `UnlicensedArms` sightings are the volume | both petty (a night in the cells, a fine), capped per day per capita; the Jail's bump rule evicts them first; print arrests by crime; the attention pool (M16b) already starves the Law of sweeps during a vendetta |
| R4 throughput | new hourly passes, weapon entities, children's routines | O(pitches + venues + yards) hourly, O(children + elders) daily, entities capped per 1,000; `core_20k` prints tps per phase; the `prof` feature names each new site |
| R5 the parity test | new spending at the Statistical tier must mirror Full's (vendor meals, visits, cab rides, pitches worked by Statistical agents) or the 15 % check flips | D17: world-side choices in the Statistical hour, no new table columns; a Statistical pitch worker's hour is the one exception, added at the gate with one regen and the Murders check |
| R6 shadow rows | 14 new archetypes and the existing rows (worker's `paid/wkd` with day posts, dancer, homeless walking to corners) move | diaries first; bounds after the eight-run ensemble per TESTING.md; the pooled tier's cushions |
| R7 the seeded population | ages to 75, IDs, `status` from day 0 change the seeded city under the switch | all under `[jobs2]`; the off city is byte-identical; the on city's seeds move once, at phase 7, and the gate re-reads everything |
| R8 tone | realism fixes have made the city softer twice; the vice trades and the corner give the poor more income | by design the new income is small and taxed by cuts, fines and tips' volatility; the levers (`street_legal`, `public_school`) default to the brutal side; print J24/J25 and homeless per phase and restore pressure by rents and cuts, never by broken days |
| R9 tone | the vice phases touch sensitive themes | briefs and code stay systems-level and non-explicit: shifts, visits, coins, a timed flag on a Job, a sanity cost; no act is ever described |

---

## 14. Questions for Dylan

1. **Weapons as assets (entities) or a tier number on the Kit?** Assets give theft, loot, stock, provenance and sale for free at the cost of entities (capped per 1,000). *Recommendation: assets.*
2. **The vice levers' defaults.** Brothels licensed and tolerated, dollhouse contracts legal for corps, street work illegal and swept, captive chipping `Trafficking`. The utopia game moves them. *Recommendation: as stated; a lever each, seeded by `[vice]`.*
3. **Organs as a World export.** It is a mint (World coins for bodies) and the dystopian story; capped like Data. *Recommendation: yes, capped, printed.*
4. **Children's School.** Fees only (no safety net) with a `public_school` lever off by default, or fees only and no lever. *Recommendation: the lever, off; it is the utopia player's second lever after the dole.*
5. **The elderly's seeding.** Raising `seed_max_years` 60 → 75 under the switch puts elders in the city from day 0 (the on city's seeds move at phase 7). Without it no elder exists before year 2 of a run. *Recommendation: raise it under the switch.*
6. **Cults that kill.** Apostates hunted through the grudge machinery means a cult can kill leavers (rare, `hunt_min`). *Recommendation: yes; it is the Purist expulsion story one step further, and the Feeds carry it.*
7. **The Soylent factory and the chop shop** (addendum 2) are out of scope here; the organ book and the Packing Plant are their hooks. *Recommendation: a later goods milestone with Power and Water.*
8. **The one stat-table regen at the gate** (a Statistical pitch worker's hour) redraws the rare rates. *Recommendation: do it once, check Murders against main, keep main's table if it drifts past the noise.*
