# M17: The outside world — parents, brothers, the state, the ledger beyond the wall

## Revised 2026-10-12: M17 against HEAD

This section overrides everything below it. The body was drafted on M12 phase 2 at 2,000 residents. Since then the tree has gained:

- **The Real economy** (`docs/ECONOMY_V2.md`): the World account, books per good, imports and exports, wages from revenue.
- **No safety net** (addenda 17-19, `docs/P10_POLICY.md`): the dole and public works are funded policy levers, and they are off.
- **M13-M16b as built**: see the "deviations" section in each doc.
- **Scale** (`docs/SCALE_20K.md`): 20k residents on generated or imported maps, `[scale]`, `Config::scaled_to`/`fit_to_map`, the Ambient tier of flats.

The acceptance table at the end of this section (docs/TESTING.md) supersedes the body's bands, its six-seed and pinned-run devices, and its scenario gate.

### What exists, what is reused, what is new

| M17 item (body §) | At HEAD | M17 does |
| --- | --- | --- |
| `World::outside`, `Outside`, `OutsideFaction`, `OutsideKind`, `OutsideId` (§ 1) | `citysim/src/outside.rs` (L2 L11, Real economy E4) holds the World account only, `WORLD_ACCOUNT` = 1069. `OutsideFaction.books` is the World's per-good book. | **Reuse.** Append the parent fields with `#[serde(default)]`. `books` is taken, so the body's 30-day rings are named `OutsideFaction.ledger: OutsideLedger`. `Outside` gains `sanctions`, `scrip`, `promoted` and `blackwall`. |
| The World account (§ 1) | Each midnight it refills to `[world_market] treasury_ref` both ways (`minted += refill`: the city's net trade). It buys Food at the haul, and Parts and Data at sloped tranche bids. It sells imports (`ownership::import`, already re-pointed abroad) and takes inputs (`Flow::Inputs`). Immigrants' and emigrants' purses and the fence credit cross through it. | **Reuse as the money-supply account.** *Superseded*: the body's World row (income = upkeep, treasury 1M), `import_to_outside` (always on), and "the World never decides". M17 adds two things to it: the **Ambient book** (below) and the Blackwall (§ 6). |
| Conservation identity (§ 1, § 11) | `econ::identity` = `total_coins + Σ outside treasuries − minted`. `core_sanity`, `core_year` and `core_20k` assert it every day. | **Reuse unchanged.** A parent's daily `income − upkeep`, `body_cost`, a sanction's running cost and the treasury absorbed at death all book to `minted`. Coins cross only through `ownership::cross_in`/`cross_out`. No new assert. |
| `market` walk, `SimRng::outside` (§ 1) | No `outside` stream exists, and `WordNs` is full (M16 § 13). The World's appetite walks are pure hashes (`systems::econ::hash_normal`, E49). | **New, as pure hashes**: `hash_unit`/`hash_normal(seed, PURPOSE_OUTSIDE_*, account, day)` for each parent's walk and for every M17 roll (the Abroad draw, promotion, a breach). No stream is used, so no other system's draws move. |
| `Corp.parent`, `Corp.outside_treasury` (M11 D41) | Set at seed on megacorp rows (`parent = 1000 + row`, `outside_treasury = 100000`). Only the app's corp panel reads them. At 20k the roster is dealt k times ("Arasaka", "Arasaka 2", …), and each copy gets its own parent id. | **Re-pointed.** One account per **brand** (the row's `template`): every Arasaka copy is a branch of Arasaka Global. `Corp.parent` is rewritten to the brand's account at seed and when a pre-M17 save loads. `outside_treasury` is zeroed and kept for serde. A treasury is `treasury_per_branch × branches`, so a parent scales with the map. |
| `Gang.parent`, `Law.parent` (§ 1) | absent | **New**, `#[serde(default)]`. One syndicate per seeded gang template (The Hollow, Ninefold); none for splinters or The Unplugged. The State's branch is the Law entity. |
| Flows `Remit`, `ParentFund`, `Tribute`, `Grant`, `Scrip` (§ 9) | `Flow::Tribute` already exists (L2: a gang's weekly cut to its members). | **New**: `Remit` (corp → parent and gang → syndicate; the body's syndicate `Tribute` folds into it), `ParentFund` (inbound), `Grant` (State → Treasury), `Scrip` and `AmbientSale`. `Flow::capital()` gains `Remit` and `ParentFund`: `wage_rev` never reads a subsidy as revenue, and `cashflow` never reads a remittance as a trading loss. |
| Subsidies and remittances vs the export-driven money supply | Money enters only through the World's purchases and migrants' purses. It leaves through imports, inputs and emigrants (ECONOMY_V2 § 1). Food corps hoard 83-113k by day 120. | A parent is an **outside purse with finite reserves**. Its income abroad is minted, like the World's refill. A subsidy is capital paid into a branch's treasury: it keeps the branch and its jobs alive, but it never raises a wage (wages follow revenue) and never reaches a wallet. Remittances take a share of positive cashflow above `remit_floor`, so the megacorps ship the city's profit abroad out of the hoards. The balance of trade gains `flow_remit`, `flow_parent_fund` and `flow_ambient`. The net crossing is a printed finding, not a band. |
| No safety net | P10: `dole_per_day`, `public_works`, `school_meals` and `reserve_release` are levers, all off. The Treasury runs on taxes, the property rate and customs. | **Nothing in M17 pays a wallet from outside.** The State funds guards only (`Flow::Grant`, a week of `state_guards`' wages), booked outside `EconState.receipts` so the tax band does not cut taxes on it. No State dole, no relief. Scrip is wages, not aid. |
| The State's guards (§ 4) | The Treasury sets `levers.guard_count` (Jobs J17, `treasury::daily`), and `law.rs` reconciles it daily. | `levers.state_guards` (new, `u16`) is added on top of the civic count in the reconcile. `state_guards_max` is per capita. A `Mandate` is a `LawShock` (appended) and adds `mandate_attention` to the Law's M16b attention pool. |
| Reinforcement bodies (§ 2) | `demography::spawn_immigrant_for(b, role)` (J20: an immigrant with a signed job at a building, housed). | **Reuse**, plus an `Outsider` component. Outsiders count as none of `immigrants`, J19's pull or the Ambient arrivals: they come from abroad, not from the island's flats. A tour ends through `demography::emigrate`. |
| `Retake`, buy-back, Guard (§ 2) | M16b's `Rescue` (a mission on a building), `leverage`'s `Demand::Sell(b)`, `corps::acquire`, M16a's funded `Guard`. | `ContractKind::Retake` (appended after `Tag`) runs on the Rescue mission machinery, and a win ends in `coerce(Threat, Sell(b))` (M16b). `Origin::Parent` is appended. Buy-back goes through `corps::acquire`. |
| Persona, rival (§ 2) | M16b boards hold a branch's aggregate personality. M15 `Regard` exists. | The parent's `Persona` comes from config (the board abroad); the branch's own board is untouched. The rival comes from `word::Regard`. |
| Uplink (§ 3) | M14 `NodeKind` {Public, Building, Ledger}, `virt::relink`. M16b `Steal` on a Lab runs a heist. | `NodeKind::Uplink(OutsideId)` (appended), one per brand with a living branch: at most 12 nodes at any scale. Campaign heists use M16b `Steal`. |
| Scrip (§ 4) | absent | `Wallet` is **not** widened (20k agents). Holdings go in a side table, `Outside.scrip: BTreeMap<EntityId, SmallVec<[(OutsideId, i64); 2]>>`, cleared on death and emigration. A purchase checks it only at an issuer's building (one map lookup). |
| Economic warfare inputs (§ 3) | M11 shocks and estates, M12 strikes and districts, M16b protection and `Sell` coercions. | **Reuse as reads.** A sanction also cuts the World's appetite for the sanctioned branch's goods (ECONOMY_V2 § 6). |
| Abroad, cast, promotion (§ 5) | M16a's contract entity and settlement. M16 decided that `Abroad` stays M17's own kind. | `ContractKind::Abroad` (appended) and the `Away` component, kept as records only. The draw is a pure hash. |
| The tick (§ 1) | At midnight: `corp_brain::run` (`corps::daily`), then `living::run` (`wages::daily`, `world_market::daily`, …), …, then `demography::run`. | `systems::outside::run` runs at midnight after `living::run` and before `demography::run`, which spawns the bodies. The Ambient book's sales are hourly (below). |
| Save, switches | SAVE_VERSION 20. The off switches were retired 2026-10-09. | SAVE_VERSION +1 per phase. `[outside] enabled` (on) with `CITYSIM_OUTSIDE_OFF=1` stays in flight until M17 closes. Off turns off the parents and the Ambient book, and is byte-identical to main on seeds 42-44 × 30 (events, and the CSV on shared columns). The World market belongs to the Real economy and stays on. *Superseded*: `decisions = false`, and `v1_profile`'s `enabled = false` (the v1 city keeps every feature since 2026-10-09). |
| Scale | `scaled_to` (`up_*` per person), `fit_to_map` (per building, gang, corp, district), `per_capita_count`. | These keys go in `scaled_to`: `state_guards_max`, `outsiders_cap`, `ambient_food_cap`, `ambient_visit_cap`, `promoted_max`. `subsidy_cap_day` and the other per-brand keys follow the branch count. Nothing runs per agent per tick. The daily pass is O(accounts + branches + Outsiders + sanctions), the Ambient hour is O(districts + Markets + venues), and nothing is O(n²). |

### Dylan's decision: Ambient consumption is outside demand that creates jobs

The Ambient flats' households live on the island beyond the simulated 20k, so their purses belong to the World. When they shop at a simulated business, World money is buying a city good: **export demand at retail**. That money enters as revenue and funds wages through `wage_rev` (not magic wages). It also moves the Grow inputs, so corps hire and build on it.

- **The book.** The World account gains `ambient: AmbientBook` (`#[serde(default)]`). It holds an `appetite`, a pure-hash walk like the goods' with the lever `SetAmbientAppetite { mult }` to pin or release it. It also holds today's units, visits and coins, plus 30-day rings. It is built in **phase 1** for three reasons: it needs only the World account, it is the change to the money supply, and the parents can wait.
- **Demand.**
  - Food: `ambient::daily` already estimates each district's Ambient food (its adults × yesterday's meals per simulated resident). Demand is `units_d = AmbientDistrict.food × ambient_capture × appetite`, where `ambient_capture` is the share of those meals bought at simulated Markets. The city's total is capped at `ambient_food_cap`, in units a day per 2,000 residents (a `scaled_to` key).
  - Leisure: `visits_d = adults_d × yesterday's simulated venue visits per resident × ambient_capture × appetite`, capped at `ambient_visit_cap`.
- **Scaling.** Demand grows linearly with the Ambient adults (flats × occupancy × (1 + `couple_p`)) until the per-capita cap binds. The cap is proportional to the simulated population, because the simulated businesses are sized for it.
  - On the k = 1 import (~900k Ambient adults) and on gen20k at 50 flats a slot (~230k), the cap binds.
  - On a test map with a few flats a slot, `ambient_capture` sets the demand.
  - The shipped 2k map has no Ambient flats. Demand is zero there, so the 2k city is unchanged and its off identity is trivial.
- **Supply is real.** Each hour carries a share of the day's demand, following the Statistical tier's meal and evening profile.
  - Food goes to the district's Markets first, then the nearest. A Market sells to the Ambient book only the stock above `ambient_keep` × its own 7-day mean daily sales, so the book never empties a shelf a resident needs, and no goods are created.
  - The price is the Market's shelf price. The coins cross in from the World as `Flow::AmbientSale`, and the owner pays the sale's tax, as for `Export`. The units count in `Market.sales`.
  - Leisure visits are ledger visits: no bodies, no seats. Each pays the venue's entry price into its owner's revenue, up to `ambient_visit_share` of the venue's own 7-day visits.
- **How it creates jobs (no new hiring rule).**
  - The sale is revenue, so `wages::daily`'s `wage_rev` rises and corps under target post vacancies.
  - `Market.sales` raises Food sell-through, which raises `corp_brain::demand_of` and the Grow score, and Grow builds.
  - The vacancies fill from the simulated jobless first, then from the Ambient labour pool (S5, `hires_per_day`).
  - A drain is the lever's other side: lower appetite means layoffs.
- **Not in M17:** Ambient rent (a city landlord paid by World money); Ambient Clinic, Hotel or asset purchases; dematerialisation (S5's "later").

### Revised phases (one commit each; the orchestrator merges each)

1. **Ambient demand and the switch.**
   - `[outside] enabled` with `CITYSIM_OUTSIDE_OFF`.
   - The `AmbientBook` and its hourly sales (Food and leisure), `Flow::AmbientSale` and the per-capita caps.
   - `SetAmbientAppetite`, the CSV columns (`ambient_units`, `ambient_visits`, `flow_ambient`, `appetite_ambient`) and a city-panel line.
   - `core_20k` gains Ambient flats (gen20k at S5's 50 a slot).
   - SAVE 21.
2. **The ledger and the parents.**
   - Accounts per brand (megacorps), per gang template (syndicates) and the State, seeded from `[outside] factions`, with the pre-M17 save migration.
   - The daily tick: market, income, upkeep, remittance (the syndicate's share weekly), health, rep, attitude and the failure flags. No death yet.
   - The Hold and Subsidise orders.
   - `Embargo`, `SetTariff`, the Outside panel and the CSV.
   - SAVE 22.
3. **Bodies: reinforcement, retaking, brothers, the State.**
   - The considerations table (Reinforce, Retake, FoundBranch, Abandon) and `Outsider` tours.
   - Buy-back, `Retake` with `Origin::Parent`, the Hit on the raider, Guard funding, and a local corp founding a parent.
   - The syndicates' brothers and refounds.
   - The State's Fund, Mandate and Withdraw with `state_guards`.
   - `ForceReinforce` and `SetOutsideOrder`.
   - SAVE 23.
4. **The weapons that reach the ledger, and scrip.**
   - The uplink, edge, the tech ceiling and Data shipments.
   - Sanction (with the cut to the World's appetite) and Campaign.
   - Death and the branch bankruptcies.
   - Scrip: issuance, spending, `ConvertScrip`, the wipe and `BanScrip`.
   - `KillParent`, `WipeParentTreasury`, `ForceUplinkRun` and `SetParentEdge`.
   - SAVE 24.
5. **Abroad, the Blackwall, the gate.**
   - `Abroad`/`Away`, the cast and promotion.
   - The Blackwall and traces.
   - The remaining levers, god commands and the overlay.
   - The god scenarios, an Outside section in `tools/analyze_run.py`, and the docs ("Implemented: deviations").
   - Then `/code-review high` and a fix commit.

Every phase:
- runs the standing gates one at a time: the trio, `core_year`, `shadow --assert`, the god suites, parity and `core_20k`;
- measures seeds 42-47 × 120 at 2k against main, and seed 42 × 30 at 20k with ticks/s;
- checks the off identity.

### Acceptance (docs/TESTING.md doctrine)

| tier | M17 items |
| --- | --- |
| `core_sanity` bullets (must fire on 4+ of seeds 42-47, else demoted to a unit test by the TESTING.md rule) | Candidates: a remittance (P2), a Subsidised (P2), a syndicate remittance (P2), a StateGrant (P3). The identity, the collapse bounds and population (Outsiders included) are already asserted. |
| `core_20k` | An Ambient sale on every day from day 2. This is an existence assert; it cannot be a `core_sanity` bullet because the 2k city has no flats. Also the identity, and the employment collapse floor (unchanged). |
| Unit tests (for mechanisms that fire on 2 or fewer of 6 seeds, or that are mechanism-level) | **P1:** demand is linear in Ambient adults up to the cap, and the cap scales with `per_capita`; no Ambient sale goes below `ambient_keep`; an Ambient sale raises `Market.sales` and the owner's revenue, and its coins cross from the World (identity); a corp whose revenue rose posts a vacancy; zero flats means zero demand. **P2:** income applies every factor and books `minted`; `branch_loss` rises on a sale, a lost district and a remittance gap; a subsidy is capital (no `wage_rev` move); `Embargo` stops every crossing; a brand account collects every "Arasaka N". **P3:** Reinforce caps (per day, per call, the living cap), each body arriving with a job and a cohort; an accepted buy-back pays the premium, and two refusals post a `Retake` offered to the branch first; a `Retake` win forces the sale; FoundBranch; a local corp founds a parent; brothers arrive after the delay and not inside the cooldown; no refound before `refound_days`; the State funds above `mandate_heat` and withdraws on a corrupt captain. **P4:** the uplink needs tier 3 and its ICE reads the parent's Deck; a wipe drops the tier and caps the branch; edge recovers only above `data_floor`; death comes at `death_days` exactly, bankrupts the branches and wipes the scrip; scrip below par spends at health; a conversion at a Fixer. **P5:** the Abroad draw is stable across save, load and re-run, kills only abroad and promotes only past the thresholds; the Blackwall cooldown; a pre-M17 save loads with the accounts built. |
| Person probes (`tests/person.rs`) | An Outsider with no kin leaves at `tour_until`, and one with a Spouse stays (P3). A brother whose leader was killed holds an Enemy edge to the known killer (P3). A broke scrip holder converts at the nearest open Fixer within 3 days (P4). |
| Shadow | `outsider` as a diaries-only archetype (P3). Bounds only after a calibration ensemble has read it. |
| God scenarios (60 days, each against its control) | `god_ambient_boom` (P1: Ambient appetite 3.0 on the shipped map with 4 flats a slot; Ambient revenue and vacancies above the control's; employed printed). `god_raze_arasaka` (P3: a branch owns a building again within 30 days). `god_decapitate_twice` (P3). `god_murder_wave` (P3: a StateGrant within 14 days). `god_kill_militech` and its control (P4). `god_abroad` (P5). |
| Printed bands (`tools/analyze_run.py`, never asserted) | Ambient units ÷ Food sales; Ambient coins a day per 1,000 residents; vacancies posted by corps with Ambient revenue; employed at 20k against main; outside in ÷ `total_coins`; the net crossing including remittances; subsidies and reinforcements per parent; Outsiders living (max); retakes ÷ branch buildings lost; parent deaths (0 expected on the baseline); edge on day 120; State guards (mean); scrip's share of branch employees' wealth; Abroad contracts per 120 days. |

### Open for Dylan

1. Should the 2k testing city get Ambient flats, so Ambient demand runs in `core_sanity` and the 2k gates? *Recommendation: no.* The 2k city stays the closed core the gates were calibrated on. Ambient demand is a big-map mechanism, tested by unit tests on the shipped map with flats, by `god_ambient_boom` and by `core_20k`.
2. Should the megacorps be net extractors? Remittances could make the parents a net outflow, with Ambient demand and exports as the inflow. *Recommendation: yes, extraction is the dystopian tone.* The body's "net crossing within ±5 %" becomes a printed reading.
3. Ambient rent (World money paying city landlords for the flats) would be a large inflow to Housing corps. *Recommendation: not in M17; revisit at the late calibration.*

The coordinator took all three recommendations as working defaults (2026-10-12), flagged to Dylan. Each one flips by config: `[ambient] flats_per_slot` on the 2k map, the remittance keys, and a future Ambient rent key.

### Phase 1 status (2026-10-12): Ambient demand and the switch

Built as designed above. Files:
- `citysim/src/systems/outside.rs`: the book, its hourly pass after `living::run`, and the lever.
- `outside.rs`: `AmbientBook` on the World account.
- `ambient.rs`: the day's counters in `AmbientDay`.
- `[outside]` and `[outside.ambient]` in config.toml, with `food_cap` and `visit_cap` in `scaled_to`.
- `Flow::AmbientSale`, `PlayerCommand::SetAmbientAppetite` (CLI `ambient_appetite=<mult|auto>`), seven columns appended to the Ambient CSV group, and a city-panel line.
- SAVE 21.

Deviations from the design:
- **Food demand is computed fresh at midnight.** It is the district's Ambient adults × yesterday's meals per simulated resident. The aggregate's own `food` estimate is refreshed later in the same tick, so reading it would lag a day.
- **The trade balance includes the book's purchases.** `trade_balance` now counts them as World money in, which is 0 without flats.
- **Ambient visits do not count in a venue's `visits` ring.** A venue's own visits set its cap, so counting them would feed the cap back into itself. NoodleBar visits take a meal off the shelf.
- **Unsold demand is lost**, not carried to the next hour.

Measured:
- **2k city.** The shipped city is **byte-identical to main 9dd4f19 with the switch on and off**: events and CSV without `ticks_per_sec`, seeds 42-44 × 30, and with it on, seeds 42-47 × 120. There are no flats, so there is no demand.
- **20k off identity.** On gen20k with 50 flats a slot, `CITYSIM_OUTSIDE_OFF=1` is byte-identical to main with the same flats: events, and the CSV on shared columns, seed 42 × 60.

Seed 42 × 60 on gen20k, 50 flats a slot, against main (and main with the same flats):

| reading | main (no flats) | main (flats) | M17 p1 |
| --- | --- | --- | --- |
| employed d30 / d60 | 9,454 / 9,918 | 9,454 / 9,918 | 9,911 / 10,636 |
| employed share d60 | 0.510 | 0.509 | 0.542 |
| homeless d30 / d60 | 1,989 / 3,940 | 1,989 / 3,983 | 1,858 / 3,773 |
| Murders, Assault/day (60 d) | 288, 91.6 | 315, 91.8 | 275, 79.8 |
| starvation (60 d) | 3 | 2 | 1 |
| Ambient paid/day, share of wages | — | — | 7.9k, 14.0 % (food 1,475 units at the cap, 455 visits) |
| laid off, foundings (60 d) | 922, 84 | 895, 87 | 719, 75 |
| `grow_world`, Grow orders (60 d) | 38, 48 | 38, 49 | 48, 58 |
| total coins d1 → d60 | 1.29M → 1.93M | → 1.92M | 1.39M → 2.32M |
| food price | 4 | 4 | 4 |
| ticks/s d11-60 (two runs in parallel) | 159 | 168 | 167 |

- `core_20k` with flats passes:
  - J24 is 0.508 on day 30.
  - Ambient sells on every day.
  - Ambient's share of wages falls from 18.9 % on day 2 to 10.2 % on day 30, as venue visits shrink to the venues' own falling visits.
  - 210 ticks/s.
- **`god_ambient_boom`** (shipped map, 4 flats a slot, appetite 3.0 from day 10) against its control over days 30-60:
  - Ambient coins 425 a day against 131;
  - employed 1,086 against 1,071;
  - layoffs 4 against 18;
  - foundings 7 against 3.

**Finding on the money stock.** It grows in main's 20k city already: +49 % in 60 days, with the trade balance at +400k over the last 30 days. Ambient demand adds a bounded, linear ~8k a day at the cap, and prices hold at 4. This is not a broken signal from the book, but the stock has no drain at 20k.

### Phase 2 status (2026-10-13): the ledger and the parents

Built as designed above. Files:
- `citysim/src/systems/parents.rs`: seeding, the midnight pass, remittance, the orders, the books and the CSV group.
- `outside.rs`: `Parent`, `OutsideOrder`, `Persona`, `OutsideLedger`, and `Outside.{tariff, inbound_mark, outbound_mark}`.
- `[outside.parents]` in config.toml.
- `Flow::{Remit, ParentFund}` (both capital), `EventKind::{Subsidised, ParentFailing, OutsideOrder}`, `PlayerCommand::{Embargo, SetTariff}` (CLI `embargo=<id>:on|off`, `tariff=<rate>`).
- The Outside CSV group, after the Ambient group: 8 columns plus 7 per parent.
- An Outside section in the city panel; the corp panel names its parent.
- SAVE 22. A save before 22 builds the accounts on load (minted, so the identity carries over).

Deviations:
- **A parent's state hangs off `OutsideFaction.parent: Option<Parent>`.** The World account carries `None`.
- **`Gang.parent` and `Law.parent` are not added.** Branches live in the account's `branches` list; only `Corp.parent` is re-pointed.
- **Syndicates by seeded order.** Seeded Hideout gang `i` (ascending id) is a branch of syndicate row `i mod n`. The shipped city's two gangs map to its two rows; the 20k city's generated gangs are dealt over them. A creed's gang (The Unplugged) has no syndicate.
- **`branch_loss` has no district term yet.** Phase 2 reads buildings and remittances only, `0.6 × (1 − value ÷ peak) + 0.4 × remit_gap`; the district term arrives with economic warfare in phase 4.
- **The State holds in phase 2.** Its own orders are phase 3's.
- **The parents run at each midnight but tick 0.** As `ownership`'s cashflow roll, nothing has traded yet at tick 0.
- **`trade_balance` now includes the parents' crossings** (subsidies in, less remittances out), so Σ `trade_balance` = Δ `total_coins` holds.

Identity: with `CITYSIM_OUTSIDE_OFF=1` the 2k city is byte-identical to c6d6ebe (events and CSV), seeds 42-44 × 30.

Measured on seeds 42-47 × 120 at 2k, c6d6ebe → phase 2:
- Murders 587 → 543; Assault/day (Σ over the six seeds) 83.6 → 86.5.
- Employed: mean 7,043 → 6,978, d120 9,003 → 8,801.
- Homeless d120 1,051 → 1,265 (+20 %, on 6 of 6 seeds); starvation 3 → 3.
- Total coins d120 1,583k → 1,552k.
- Remittances 82k on 520 seed-days (6 of 6 seeds); subsidies 287 paying 39k (6 of 6).
- Net crossing +734k, against a trade balance of +765k on c6d6ebe.
- `core_year`: population 1,863 on day 365 (1,892 on c6d6ebe).
- A parent failing on day 120 on 4 of 6 seeds (a syndicate).
- `core_sanity` gains two bullets, a day with a remittance and a Subsidised.

**Money stock at 20k** (seed 42 × 60, gen20k, 50 Ambient flats a slot; remittances on, then `remit_share` and `tribute_share` 0):

| | d10 | d30 | d60 |
|---|---|---|---|
| total coins, remittances on (k) | 1,373 | 1,734 | 2,294 |
| total coins, remittances off (k) | 1,401 | 1,769 | 2,333 |
| homeless, remittances on | 366 | 1,836 | 3,387 |
| homeless, remittances off | 367 | 1,755 | 3,221 |

- **The parents are not the missing drain.** Over 60 days remittances average 1.6k a day out and subsidies 1.2k a day in, against a trade balance of +15.6k a day.
- **Nothing mints at 20k that does not mint at 2k.** Per 2,000 residents, the 2k and 20k cities run the same circuit: exports ~2.5k a day, imports and inputs ~1.3k a day, so a trade surplus of ~1.1-1.4k a day. Total coins grow +42 % (2k) and +46 % (20k) in 60 days without the parents.
- **The surplus is the World market's export leg exceeding the import and input legs at both scales.** Coins pile up in the Treasury (16k → 70k per 2,000 residents; the tax band cuts the rate to 0 by day 30) and in the corps.
- **No 2k constant is mis-scaled on this path.**
- **The homeless doubling is per capita the same at 2k:** 177 → 325 from d30 to d60 on seed 42 at 2k, against 199 → 394 per 2,000 at 20k. Evictions run ~10 a day per 2,000 at both scales.

Test fixtures:
- `leverage.rs` and `tags.rs` build their seeded worlds with `[outside] enabled = false`. Their dice and picks read the M16b city's trajectory at day ~40 (which gang is the rival, an equal-tier Patrol sweep).
- The shadow tier's d19 ripperdoc row is re-calibrated by the one-row rule. It is a ten-agent row: 2.78 known met and 0.05 aborts on 42-44, inside the range under two butterflies with the parents on.

Found and fixed (outside phase 2's scope): `transit::board` did not check `CutLine`. A ride filed before the cut boarded the cut line after it: `god_line_cut` caught one on day 20 at 00:35. `board` now requires the line open, as it requires the stop.

**The phase 2 homeless trace** (2026-10-13, seeds 42-47 × 120, each lever zeroed alone against phase 2). Homeless on d120 (mean over the run):

| run | homeless d120 (mean) | evictions |
|---|---|---|
| c6d6ebe | 1,051 (1,222) | 5,212 |
| phase 2 | 1,265 (1,328) | 5,442 |
| `remit_share = 0` | 1,188 (1,281) | 5,259 |
| `tribute_share = 0` | 1,109 (1,233) | 5,255 |
| subsidies off | 1,259 (1,279) | 5,337 |

- **Most of the rise is the syndicates' tribute.** With tribute off, the mean homeless count is back to c6d6ebe's, and gang members on d120 recover from 697 to 741 (768 on c6d6ebe). The weekly 10 % takes the coins a gang pays its members beyond the promise, so fewer stay in the gang, and the ones who leave lose that income and land on the street or in squats (squatters 920 → 1,132; 978 with tribute off).
- **The rest is Arasaka's remittances.** Arasaka is a Security corp with no Housing; its mean treasury falls from 27k to 15.6k (21.7k with remittances off), and the city loses some of its hires (employed mean 7,043 → 6,978; 7,093 with remittances off).
- **Not Militech's Housing.** Militech's treasury is unchanged (40k → 42k): its closing rarely clears `remit_floor`, so it remits little.

Both effects are extraction by design, so whether they are wanted is a question for Dylan, not a tune.

### Phase 3 status (2026-10-13): bodies

Built as designed above in `systems/parents.rs` (one file with phase 2), `outside.rs` and `[outside.parents]`'s phase 3 keys:
- **Reinforce:** Outsiders, each with a job at a branch post, a tour and a cohort.
- **Retake:** a buy-back, then a `Retake` record (`ContractKind::Retake`, `Origin::Parent`) offered first to the branch's own Outsiders, a Hit on a raider gang's leader, and Guards on the remaining buildings.
- **FoundBranch, Abandon,** and a local corp founding a parent.
- **The syndicates:** brothers after a decapitation (leader dead or jailed) with their delay and cooldown, and refounds of an emptied gang. M12's bootstrap re-formation is off for a syndicate's gang.
- **The State:** Fund, Mandate and Withdraw. `Outside.state_guards` sits on top of `guard_count` in the reconcile; the Mandate tilts the law's posture scores.
- **The remit-gap fix:** `branch_loss`'s gap is now the share of corp branches a whole week in the red, so sporadic remittances read no loss.
- **Commands and save:** `ForceReinforce` and `SetOutsideOrder` (CLI `force_reinforce=<id>:<n>`, `outside_order=<id>:<order>:<days>`), 17 Outside CSV columns, 14 events, SAVE 23.

Deviations:
- **A Retake is struck on the ledger.** It is one keyed draw at the missions' `p_win` over the crew's strength against the building's posted guards and the holder gang's strongest members, as Tag and Vote are ledger records. It is not a live mission: the mission machinery needs an agent target. A win forces the sale at value, funded by the parent.
- **The buy-back's seller rule is simplified.** A corp sells when short of cash or its exec is greedy; an agent when greedy or poorer than the building; a gang never; the city always.
- **Outsiders live in a side table** (`Outside.outsiders`), not a component. They spawn through demography's migrant arrival (the World's 15 coins and the arrival savings cross in), are not counted as immigrants, and get cohort edges as Acquaintances (there is no Colleague kind). The parent pays `body_cost` abroad and `body_fund` into the branch.
- **A new branch survives without a building during its grace.** A Security Office cannot be built on a Lot, so with the parents on, M11's no-building bankruptcy waits out the new branch's upkeep grace while it buys or takes a building back.
- **Brothers' triggers:** a leader dead or jailed since the last midnight (killer unknown); a Sack does not trigger them. A syndicate's cooldown is per syndicate, shared over its gangs.
- **Emptied gangs:** an emptied gang waiting on its refound is out of `branch_loss` and gets no subsidy. `refound_attitude` is −0.5 (placeholder): a syndicate's attitude drifts below 0 without remittances.
- **The State's Fund scores at the cap,** so a hot city keeps its guards (it oscillated Fund/Withdraw otherwise). The Mandate is read at the law's rescore: no `LawShock` is appended and the attention pool is not touched. `SetStateGuards` waits for phase 5.
- **`flat_retake` is 0.15 (placeholder):** Retake's five considerations compensate to about 0.3 on a razed branch, under Hold's 0.3 plus hysteresis.
- **A founded parent has no CSV columns.** The header is fixed at the seeded parents.

Identity: with `CITYSIM_OUTSIDE_OFF=1` the 2k city is byte-identical to 4592dc1 with the same switch (events and CSV), seeds 42-44 × 30.

Seeds 42-47 × 120 at 2k, 4592dc1 → phase 3:
- Murders 543 → 622 (+15 %); Assault/day (Σ over the six seeds) 86.5 → 91.3.
- Employed: mean 6,978 → 6,901, d120 8,801 → 8,438.
- Homeless d120 1,265 → 1,339 (mean 1,328 → 1,336); starvation 3 → 2.
- Population d120 11,449 → 11,423; gang members d120 697 → 770 (+10 %).
- Total coins 1,552k → 1,650k.
- Outsiders: 42 OutsidersArrived events, 8 left.
- BrothersArrived events: 27 (3 bodies each), on 6 of 6 seeds.
- Refounds 0.
- The State: grants 115, mandates 7, withdrawals 18.
- ParentFounded 6 (Nutrix on every seed, days 42-58, moving 16-24k abroad).
- Abandoned 7 (the syndicates).
- Retakes and buy-backs 0 (no branch building lost to a holder in the window). The spiral runs in `god_raze_arasaka`: FoundBranch, Reinforce, two refusals, a Retake and a Hit, BranchRetook on day 27.

20k (seed 42 × 60, gen20k, 50 flats a slot, a single run):
- 6 Outsiders arrived; 12 brothers.
- State guards at the cap (120) by day 60; 9 grants.
- Gang members d60 1,079 (1,053 in phase 2); population 19,753; employed 10,921.
- Homeless d60 3,244 (3,387 in phase 2).
- Murders 310, Assault/day 96.7.
- 185 ticks/s (a single run; phase 2's 170 was one of two in parallel).

Shadow: the d19 homeless paid rows are re-calibrated by the rule (`shadow.rs`, five readings).

### Phase 4 status (2026-10-13): the weapons that reach the ledger, and scrip

Built in `systems/parents.rs`, with hooks in `systems/virt.rs`, `ownership.rs` and `world_market.rs`; SAVE 24.

- **The uplink.** `NodeKind::Uplink(OutsideId)` is one node per living megacorp parent with a corp branch, at that branch's Ledger, behind one tier-3 Trunk. It has no owner, so the parent's Data is never a branch's holding. Its ICE is `uplink_ice` capped by the parent's Deck ceiling, and its store mirrors the parent's Data.
  - A Data run takes from the parent and cuts `edge` by `edge_per_unit` a unit.
  - A wipe zeroes the touched tracks, drops each from tier 2 or more by one, and cuts `edge_wipe`. The branches' effective tiers (`virt::maker_tier`) are capped by the parent's ceiling.
  - A Ledger run crosses `min(uplink_frac × treasury, uplink_cap)` to the patron.
  - Natural runners see the uplink as a Data target, but only a tier-3 deck routes to it.
- **Edge.** It recovers `edge_recover × (1 − edge)` a day while every track holds `data_floor`, and Data arrives at `data_per_day`. Income now reads edge and one combined cut: the sanctions' and the Embargo's.
- **Data shipments.** A branch whose tech upkeep would lapse for want of Data gets `ship_data[tier]` units in its first Lab.
- **Sanctions.**
  - A rival parent's Sanction order needs the M15 Regard between branches under `sanction_regard`.
  - The State sanctions a hot branch's parent, but only in a hot city (`city_heat ≥ mandate_heat`; see the deviations).
  - A sanction cuts income, costs the sanctioner `sanction_cost` a day, cuts what the World buys from the sanctioned branch, and, when the State's, stops subsidies and bodies crossing.
- **Campaign.** A disliked rival only. It sends a weekly run on the rival's uplink from a branch Lab with a tier-3 fleet deck, or else, when the parent is lawless, a Beat on the rival branch's exec.
- **Economic warfare.** The districts the corp branches control add `0.3 ×` the share lost since the 30-day peak to `branch_loss`.
- **Death.**
  - A parent dies on `death_days` failing (with its reserves under `treasury_ref`; see the deviations), or on `insolvent_days` with a treasury under 0.
  - Its corp branches go bankrupt (M11), its gangs are on their own, its Outsiders' tours end and its scrip is wiped (district unrest by `scrip_unrest`).
  - Its uplink drops at the relink and its treasury is absorbed abroad.
- **Scrip.**
  - `Outside.scrip` is a side table. `scrip_wage_share` of a parented corp branch's wage is scrip; its coins cross to the parent at issue (`Flow::Scrip`).
  - Food or rent paid to the issuer's branch uses scrip first: the parent redeems it into the payer's purse at par, or at health below `scrip_par_min`.
  - A daily pass converts a broke holder's scrip at a Fixer at health less `fixer_cut`.
  - `BanScrip` redeems every holder. Abandon redeems at health, or wipes below par.
  - Scrip is never coins: `econ::identity` holds every day. `trade_balance` now reads every crossing with a parent (`parents_in − parents_out`).
- **God commands.** `ForceUplinkRun` (the effect without a runner), `KillParent`, `WipeParentTreasury`, `SetParentEdge`, `SanctionParent`, `BanScrip`.

Deviations:
- **Conversion is a daily pass, not a `ConvertScrip` walk.** The Fixer's owner takes the cut. The walk is an M18 hook.
- **Not built:** the scrip memories and the wipe's Betrayed grudges.
- **Campaign needs a disliked rival.** On the plan's gate every healthy parent campaigned: 0.52 against Hold's 0.3.
- **The State's sanction is a rule, not an order,** and only in a hot city: M15's corp heat sat near 0.7, and Militech was sanctioned on day 3.
- **A failing parent dies only with its reserves under `treasury_ref`.** In a year run, Nutrix Holdings (a locally founded parent) died on a treasury fat with remittances and took Nutrix, 34 buildings, with it.
- **`branch_loss`'s district term is additive (0.3 × the share lost),** keeping phase 2's 0.6/0.4 split.
- **`ForceUplinkRun` applies the run's effect without a runner.** Natural runs go through the run machinery.

Identity: with `CITYSIM_OUTSIDE_OFF=1` the 2k city is byte-identical to a07544b with the same switch, seeds 42-44 × 30.

Seeds 42-47 × 120 at 2k, a07544b → phase 4:
- Murders 622 → 523; Assault/day (Σ) 91.3 → 89.1.
- Employed mean 6,901 → 6,870; homeless d120 1,339 → 1,353; starvation 2 → 2.
- Population 11,423 → 11,503; gang members 770 → 778.
- Total coins 1,650k → 1,640k.
- Parents dead 0.
- Uplink runs 0: no natural runner reaches a tier-3 Trunk in 120 days.
- Data shipped 64; sanctions 14; campaigns 0.
- Scrip: 485k paid, 223k spent at the issuers, 99k converted, 0 wiped; 91k out on day 120 (13-19k a seed).
- `core_year`: population 1,834, corps alive 8/9.
- `god_kill_militech`: Militech International dies on day 33 (wiped on days 10 and 25, failing from day 12), and its scrip of 1,497 units is wiped. The control lives, at edge 1.0 against 0.64.

20k (seed 42 × 60, gen20k, 50 flats a slot, a single run):
- Scrip: 97k paid, 1.2k spent, 39k converted; 29k out on day 60.
- 50 Data shipments, 2 sanctions, 0 uplink runs, 0 deaths.
- Employed 10,827; homeless 3,293; Murders 315; Assault/day 104.7.
- 183 ticks/s.

### Phase 4b status (2026-10-13): the succession

Dylan, reviewing the brothers: "A good possible event, but a little weird as an always occupancy. Gangs probably should promote from within or even have a civil war to settle who the new boss is." Built in `systems/succession.rs`, with hooks in `gang.rs` (`leader_scores`, the crown in `recompute_leader_after`, D36's split as `split_led`) and `parents.rs`; `[outside.succession]`; SAVE 25.

- **Promotion is the default.** A decapitated gang's new leader is the top of `leader_ranking`, as before; a `Succession` event names the promoted, the gang and the fallen. A leader jailed leaves an acting leader ("runs The Hollow while X sits inside"; nothing for a Vagrancy night) and never a war: the boss walks back in on release.
- **A close succession is a civil war.** When the leader died, the runner-up stands within `war_margin` (0.02) of the heir and the gang has `war_min_members` (8) free members:
  - every free member takes a side: the challenger's pull (their edge's affinity, plus `regard_w × (standing + dread × (1 − courage))` of the claimant) against the heir's pull plus `loyalty_w × loyalty`; a `TookSides` event per member for the diaries;
  - the feud runs on M15: each claimant holds a `war_grudge` (0.9, lethal) grudge on the other and takes up the Hunt at once (a Statistical claimant is promoted to Coarse; a weak one may hire instead), the two strongest seconds of each side hold `war_grudge_second` (0.8) on the rival claimant, and the sides are Enemies (30 pairs at most);
  - the heir holds the chair while it runs (`Gang.crowned`);
  - a claimant killed or jailed loses at once; after `war_days` (14) the sides' summed `raid::strength` settles it on a keyed roll (the heir wins under its share);
  - the winner is crowned; the loser is dead, sat in a cell, leads a splinter of its following through D36's split (`split_led`: the structure tests only), or leaves the gang and the city.
- **Brothers are a rare event.** A syndicate sends them only to a gang sacked since the last midnight with fewer than `brothers_gutted_below` (6) free members, on a keyed roll under `brothers_p` (0.3), or on the god `SendBrothers(gang)` (the next midnight, the cooldown ignored). The phase 3 scheduler is kept as `schedule_brothers`.
- **CSV:** `successions` (promotions after a death), `civil_wars`, `wars_active`, `war_loser_dead`, `war_loser_fled`, `war_loser_splinter`, `war_loser_jailed`. Events `Succession`, `CivilWar`, `TookSides`, `CivilWarEnded`.

Deviations:
- **Wars follow deaths only.** Leaders are jailed about 70 times a seed in 120 days; a war on each read as noise (85 wars on 42-47 in the first build). A jailing leaves an acting leader.
- **Regard is the claimants' reputation.** M15's `Regard` is faction × faction, the same for both claimants inside one gang, so a member's regard reads each claimant's standing and dread.
- **The succession rides the M17 switch** (`CITYSIM_OUTSIDE_OFF=1` turns it off with the parents), so the off identity holds against main.
- **A jailed loser stays a member** (nobody's boss); the war's grudges live on and decay.
- **The god scenario's lieutenants are staged**: their days in the gang set to the leader's and their loyalty 0.001 and 0.006 under it.

Identity: with `CITYSIM_OUTSIDE_OFF=1` the 2k city is byte-identical to 19e17d0 with the same switch (events, and the CSV on its 654 shared columns), seeds 42-44 × 30.

Seeds 42-47 × 120 at 2k, 19e17d0 → phase 4b (printed findings):
- Leader deaths 12: 8 promoted, 4 civil wars (seeds 43, 44, 45 twice; 98 members took sides). Every war ended inside 1-3 days with the loser jailed (Dealing, Assault): the law settles them before the strength roll. Fled and splinter fired on 0 of 6 (unit-tested). Acting leaders 424.
- Brothers: 27 BrothersArrived (81 bodies) → 1 (3 bodies, seed 47).
- Murders 523 → 565 (per seed 80/62/82/78/83/138 → 54/77/104/118/144/68: the noise between builds is wider than the change); Assault/day (Σ) 89.1 → 82.2.
- Gang members d120 778 → 725 (mean 698 → 666). Per gang on day 120, summed over the seeds: The Hollow 213 → 166, Ninefold 205 → 247, Rust Saints 160 → 153, The Unplugged 200 → 159.
- Employed mean 6,870 → 6,807 (d120 8,407 → 8,222); homeless d120 1,353 → 1,546; starvation 2 → 1; population d120 11,503 → 11,469.

20k (seed 42 × 60, gen20k, 50 flats a slot, a single run; phase 4's reading in brackets): 7 promotions, 176 acting leaders, 1 civil war (Low Choir, day 51, still running on day 60), 0 brothers (12 in phase 3); employed 10,943 (10,827), homeless 3,242 (3,293), gang members 996, Murders 316 (315), Assault/day 99.4 (104.7); 210 ticks/s (183).

The shadow d19 homeless rows `PaidOfWorked` / `WagedWorkdays` are restored to their pre-phase-3 bounds (Min 0.7 / 0.68): the shipped city reads 0.84 / 0.80.

Companion to `SPEC.md`, `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M15_WORD_AND_BLOOD.md`, `M16_CONTRACTS.md`, `ROADMAP_POST_M14.md` and `VISION.md`. M16 put a price on everything inside the city. M17 gives the city **an outside that pays some of those prices**: a ledger nobody in the city can walk to, which sends coins, bodies and contracts in and takes remittances, tribute and Data out. Ninefold raids Arasaka's Security Office in the Sump twice and its last two clients walk; within the week a dozen hard-faced guards nobody has seen before step off the edge road, sleep in a Hotel on Arasaka's account, and a `Retake` contract appears on the Stack Bar Fixer's book with Ninefold's leader as the door. Meanwhile a Zetatech runner on a tier-3 deck goes through Militech's uplink three nights in a month and wipes its Deck Data abroad; Militech's outside income falls under its upkeep, its creditors call it in on day 97, Militech's Blocks go to the estate, and every Militech tenant holding scrip wakes up poor. Mid West riots on day 98. Where this document and the earlier ones disagree, this one wins for M17.

The roadmap row (`ROADMAP_POST_M14.md`): *parents outside the city at ledger LOD: megacorps, syndicates, the state; remittance, reinforcement, collapse; Virt and economic warfare as the only weapons that reach a parent.* M17 also takes what the addenda and `VISION.md` assign to it: the outside as combat maps with a transient cast promotable to persistence (governance addendum; hook only), scrip valued by the outside ledger (addendum 9), the Blackwall as the daemon tier cap (addendum 11, candidate), bunkers that the outside can still trace (addendum 7; a sighting source only), the `Corp.parent` and `outside_treasury` hooks of M11 D41, the uplink nodes and the `edge` cut from Data theft that M14 deferred, `Flow::Import` re-pointed at the outside (M13; a switch, off by default), and the cross-boundary contracts M16 deferred. The player, maps and daemons stay out (§ 14).

Scale: 2,000 residents on the 256 × 192 map; every number assumes M10–M16 have landed. At the time of writing the tree holds M12 phase 2; M13–M16 are specified, not built, and M17 reads them as specified.

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What an outside faction is | A struct in `World::outside.factions`, not an entity: a treasury, an income model, a Data stock, a tech ceiling, a reputation, an attitude toward its branches, a `Persona` and a standing order. ≤ `max_accounts` (12). No map, no buildings, no agents abroad. The roadmap's `Parent` is renamed `OutsideFaction`; `OutsideId` is M11's `ParentId`, numbered from `OUTSIDE_PARENT_BASE`. |
| Who has one | Seeded: Arasaka Global and Militech International (megacorps, M11 D41), the **State** (behind the Law), a **syndicate** for The Hollow and one for Ninefold, and **the World** (the generic counterparty). A city corp that grows big enough founds a parent (§ 2); M12 splinters and The Unplugged have none. Six at seed. |
| The tick | **Daily**, `systems::outside::run` at `tick_of_day == 0`, after `corp_brain::run` (whose `corps::daily` closes the books and bankrupts) and before `demography::run` (which spawns the bodies the outside sends). O(accounts + branches); nothing per agent. |
| How the outside enters | Only as **coins** (cross-boundary flows), **bodies** (immigrants through `demography::spawn_immigrant` with a job, an `Outsider` component and a cohort), **contracts** (posted on the M16 board with the branch as buyer of record) and **Data** (shipped to a branch's Lab). |
| Conservation | The outside is a purse the conservation test can see: `ownership::total_coins(world) + Σ treasury − outside.minted` is constant, where `minted` accumulates each account's daily `income − upkeep` (the outside economy's own creation and destruction of money). Scrip is not coins (§ 4). |
| How a parent decides | The M11 brain template: a considerations table over eight orders (Hold, Subsidise, Reinforce, Retake, Sanction, Campaign, FoundBranch, Abandon), rescored daily with hysteresis, reading a three-trait `Persona` (the board abroad as one aggregate, D49). |
| Reinforcement is immigration with a destination | Bodies arrive at an edge road through the weekly-immigration code path, on top of the lever, with a job at the branch already signed, housed by the parent, on a `tour_days` tour; they leave at the end of it unless they have settled. |
| Retaking | Money first, then force: a buy-back offer at a premium, then a `Retake` contract (a mission on the building, M16 § 3) first offered to the branch's own Outsiders, and `Hit` on the raider's leader when the parent's lawfulness is low. A dead branch is refounded (`FoundBranch`). |
| How a parent dies | **Income below upkeep for `death_days`** (its creditors call it in), or its treasury below zero for `insolvent_days`. Losing every city building hurts through `exposure` but never kills it alone; Virt (the uplink) and economic warfare are what push income under upkeep. |
| What reaches the ledger | An **uplink node** per parent with a branch (`NodeKind::Uplink`), behind a tier-3 Trunk from the branch's Ledger node, ICE at the parent's Deck tier. Data theft there cuts `edge`; a wipe drops the parent's tech tier, which caps every branch; a Ledger siphon takes coins from abroad. Economic warfare counts through `branch_loss` and remittances. |
| Scrip | Lands narrow: only parented branches issue it, as `scrip_wage_share` of wages; **the coins go to the parent at issue**, so scrip is a parent liability. Accepted at par by the issuer's own shops and landlords while the parent is healthy; converted at a Fixer at `health`; wiped when the parent dies. |
| The hydra gap | M12's "no instant hydra" stays for gangs without brothers. A syndicate turns re-formation into a **bought, delayed, capped** event: brothers after a decapitation or a sack, a refound of an emptied gang after `refound_days`, each paid from a small treasury with a cooldown. |
| The State | An account that **funds extra guards** (wages paid as a grant to the Treasury) and **mandates** a harder posture when the city's heat is high, and **withdraws** both when the city is calm or its captain is known corrupt. It never commands a guard and never collapses. |
| The outside as combat maps | Hook only: `ContractKind::Abroad` on an outside asset, always resolved at the Ledger LOD (one draw), with a transient cast of hash-derived NPCs stored as records, not entities; one who ends Friend or Enemy of a crew member may be **promoted** and arrive later as an immigrant with that edge. No maps. |
| The Blackwall | Owned by the World account: `blackwall_cap` (3) is the daemon tier ceiling and the M14 deck ceiling; a tier-3 run on an uplink rolls a rare `BlackwallBreach`. Nothing else of daemons. |

## Goals and acceptance

(2026-10-09: how these bullets are judged is docs/TESTING.md: existence bullets, sanity bounds, person probes, behaviour bounds and god reactions; every band, majority or six-seed device below is a printed report, not an assert. See *Acceptance, revised 2026-10-09* under the testing section.)

The city should read as a branch office of something larger: corps that bleed and are topped up, guards with no history in the city who leave after two months, a gang that loses its leader and gets three brothers from the next city a week later and no more, a law that swells when the murders do, and wages that are worth what a distant board is worth. The target spiral: **a gang raids or squats a megacorp branch's building → the branch's `loss` rises and the parent scores Reinforce, then Retake → Outsiders arrive, the parent offers to buy the building back, is refused, and posts `Retake` → the branch's Outsiders take it as a mission and win → the gang's syndicate, after the raid kills its leader, sends brothers → a rival runner breaches the parent's uplink and its edge falls → the parent's income nears its upkeep and its scrip trades below par at the Fixers.** On seed 42, 2,000 residents, 120 days:

- the ledger: every account's treasury and order move at least once; both megacorp parents and both syndicates are alive on day 120; the conservation identity holds to the coin every day; outside coins entering the city are **3–15 % of `total_coins`** on day 120 and the net crossing (in − out) is within ±5 % of it;
- reinforcement: **≥ 1 Subsidise** and **≥ 1 Reinforce** whose ≥ 3 Outsiders hold jobs at the branch within a day of arrival; ≥ 1 buy-back offer or `Retake` posted; no branch is bankrupted while its parent is alive, not Abandoned and above `max_spend_frac` headroom;
- gangs: **≥ 1 `BrothersArrived`** after a decapitation or a sack, never sooner than `brother_delay_days`; no emptied gang with a syndicate re-forms before `refound_days`;
- the State: ≥ 1 `StateGrant` (guards funded) and ≥ 1 `StateMandate` or `StateWithdrew`; living guards never exceed `guard_count + state_guards_max`;
- scrip: ≥ 1 branch paying scrip wages, ≥ 1 conversion at a Fixer, Militech scrip spent on rent;
- abroad: ≥ 1 `Abroad` contract resolved;
- **god scenarios** (§ 11): a razed branch is retaken (owns a building again) within 30 days; a parent dies through uplink runs plus economic warfare within 120 days, its scrip is wiped and a district riots, while the same parent survives the same razing without the runs; a decapitated gang is reinforced and a second decapitation inside the cooldown is not; a heat spike draws State guards and a Crackdown; a promoted NPC from an Abroad mission arrives with an edge;
- Assault events per day stay ≤ 42.7, Murders per 120 days rise by ≤ 10 % over the M16 run on the same seed, starvation deaths and population stay within the scaled v1 bounds (Outsiders included), and the v1, M8–M16 and Full-vs-Statistical parity gates still pass;
- throughput is reported per gate; the shared floor is 4,000 ticks/s (2026-10-06: relaxed from 8,000 while systems are still being built; optimisation is a later pass).

## 1. The ledger

### Data model

```rust
pub type OutsideId = ParentId;           // u32 from OUTSIDE_PARENT_BASE (M11 D41)

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OutsideKind { Megacorp, Syndicate, State, World }

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub enum OutsideOrder { #[default] Hold, Subsidise, Reinforce, Retake, Sanction, Campaign, FoundBranch, Abandon }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Persona { pub greed: f32, pub pride: f32, pub lawfulness: f32 }   // the board abroad, aggregated

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Sanction { pub by: Option<OutsideId>, pub on: OutsideId, pub until: Tick, pub cut: f32 } // by None = the city

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutsideBooks {                 // 30-day rings, newest last
    pub income: VecDeque<i64>, pub remitted: VecDeque<i64>, pub spent: VecDeque<i64>,
    pub branch_value: VecDeque<i64>,      // Σ value of branch buildings at the close
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutsideFaction {
    pub id: OutsideId,
    pub name: String,                     // "Arasaka Global"
    pub kind: OutsideKind,
    pub persona: Persona,
    pub treasury: i64, pub treasury_ref: i64,
    pub base_income: i64, pub base_upkeep: i64,
    pub income_today: i64,                // after every factor, last tick
    pub market: f32,                      // the seeded outside market, mean 1.0
    pub edge: f32,                        // 0.2..=1.5 competitive edge
    pub rep: f32,                         // 0..=1 the outside's view of it
    pub data: DataStore,                  // M14, per track, held abroad
    pub tech: [u8; 3],                    // the ceiling on its branches' Tech (M14 § 5)
    pub exposure: f32,                    // share of income riding on the city
    pub attitude: f32,                    // −1..=1 toward its branches
    pub branches: Vec<EntityId>,          // corps, gangs, or the Law's entity; sorted
    pub branch_name: String,              // the name a refounded branch takes
    pub order: OutsideOrder, pub order_since: Tick,
    pub order_target: Option<EntityId>,   // the building, the rival branch, the gang
    #[serde(skip)] pub order_trace: Vec<OutsideOrderScore>,
    pub books: OutsideBooks,
    pub failing_since: Option<Tick>, pub insolvent_since: Option<Tick>,
    pub abandoned_until: Option<Tick>,
    pub last_reinforce: Option<Tick>, pub reinforce_left: u8,
    pub uplink: Option<NodeId>,
    pub scrip_out: i64,                   // scrip held in city wallets
    pub dead: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outside {
    pub factions: Vec<OutsideFaction>,    // ≤ max_accounts, by id
    pub sanctions: Vec<Sanction>,
    pub minted: i64,                      // Σ (income − upkeep) over all accounts since day 0
    pub inbound: i64, pub outbound: i64,  // cumulative coins across the boundary
    pub promoted: Vec<Promoted>,          // § 5
    pub blackwall: Blackwall,             // § 6
    pub next_id: OutsideId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outsider { pub from: OutsideId, pub arrived: Tick, pub tour_until: Tick, pub cohort: u32 } // component
```

`World::outside: Outside` is saved. `Gang.parent: Option<OutsideId>` and `Law.parent: Option<OutsideId>` are added (`#[serde(default)]`); `Corp.parent` exists. `Corp.outside_treasury` (M11 D41) is read once, on seed or on load of a pre-M17 save, to initialise its parent's `treasury`, then zeroed and never read again (kept for save compatibility). Branch lists are rebuilt from the `parent` fields on load.

### The daily tick

For every living account, in id order:

1. **Market.** `market = 1 + (market − 1) × (1 − market_revert) + N(0, market_sd)` from `SimRng::outside(id, day)`, clamped to 0.7–1.3. Ledgers drift; a parent with a 15 % margin does not drift into death by itself.
2. **Income.** `income_today = base_income × market × edge × (0.8 + 0.4 × rep) × (1 − exposure × branch_loss) × Π(1 − sanction.cut)`. `branch_loss` (0..=1) is `0.5 × (1 − branch_value ÷ max(30-day peak, 1)) + 0.3 × districts lost in 30 days ÷ max(districts held at the peak, 1) + 0.2 × remit_gap`, where `remit_gap = 1 − remitted_7d ÷ max(remitted_30d ÷ 30 × 7, 1)`: buildings, territory and the flow of money home.
3. **Upkeep.** `treasury += income_today − base_upkeep`; `minted += income_today − base_upkeep`. Spend on branches (§ 2) is separate, from the treasury, and booked in `books.spent`. A cost paid abroad and never crossing (a body's transport, a sanction's running cost) is booked `minted −= cost`, so the identity holds.
4. **Remittance.** Each corp branch whose `closing ≥ remit_floor × treasury_ref` and whose yesterday's cashflow was positive pays `remit_share` of it (`Flow::Remit`, capital: kept out of the branch brain's `flow`), less the city's `tariff` (to the Treasury, `Flow::Tax`). A syndicate's gang pays `Flow::Tribute`, weekly, `tribute_share` of its treasury above `tribute_floor`.
5. **Health.** `health = clamp(0.5 + 2 × (income_7d − 7 × base_upkeep) ÷ (7 × base_upkeep), 0, 1) × min(1, treasury ÷ (0.25 × treasury_ref))`. It values scrip (§ 4) and reads as the panel's single gauge.
6. **Reputation.** `rep` moves `rep_rate` a day toward `0.5 × mean branch honour + 0.3 × mean branch standing + 0.2 × (1 − mean branch heat)` (M15 axes); a branch's `Exposed` fabrication or a known `Bribe` by its exec costs `rep_scandal` at once. The State's `rep` is the Law's honour.
7. **Attitude.** `attitude = clamp(attitude × (1 − att_decay) + att_remit × U(remitted_7d ÷ (7 × base_income × exposure)) − att_cost × U(spent_7d ÷ (7 × base_income)) − att_loss × branch_loss, −1, 1)`, `U` the M11 saturating unit (`x ÷ (1 + x)`). Money home makes a parent fond of its branch; money sent and buildings lost make it tired of it.
8. **Failure.** `income_7d < 7 × base_upkeep` sets `failing_since` (cleared when it recovers); `treasury < 0` sets `insolvent_since`. At `death_days` failing or `insolvent_days` insolvent the parent **dies** (§ 3).
9. **Orders** (§ 2), then the order's daily action.

### The World account

`OutsideKind::World` has no branch and never decides. It is the counterparty for what M13 sends abroad when `import_to_outside` is on (off by default: imports stay customs to the Treasury, M11 D3), owns the Blackwall (§ 6), and is the owner of record of `Abroad` assets with no faction (§ 5). Its `base_income` equals its `base_upkeep` so it never mints.

```toml
[outside]
enabled = true
decisions = true                   # false: ledgers tick, nobody orders (the phase-1 byte-identical check)
max_accounts = 12
market_sd = 0.01
market_revert = 0.1
remit_floor = 0.5                  # branch closing ÷ treasury_ref
remit_share = 0.3
tariff = 0.0                       # lever
rep_rate = 0.05
rep_scandal = 0.1
att_decay = 0.05
att_remit = 0.3
att_cost = 0.4
att_loss = 0.3
death_days = 21
insolvent_days = 7
import_to_outside = false
factions = [                       # persona = [greed, pride, lawfulness]; data per track; tech = [chrome, deck, industry]
  { name = "Arasaka Global", kind = "Megacorp", branch = "Arasaka", treasury = 100000, income = 3000, upkeep = 2500, exposure = 0.15, persona = [0.6, 0.8, 0.3], data = [400, 1500, 400], tech = [2, 3, 2] },
  { name = "Militech International", kind = "Megacorp", branch = "Militech", treasury = 100000, income = 2800, upkeep = 2400, exposure = 0.2, persona = [0.7, 0.6, 0.4], data = [400, 1200, 600], tech = [2, 3, 2] },
  { name = "The State", kind = "State", branch = "Law", treasury = 60000, income = 900, upkeep = 900, exposure = 0.0, persona = [0.2, 0.5, 0.8], data = [0, 0, 0], tech = [1, 2, 1] },
  { name = "Hollow Brothers", kind = "Syndicate", branch = "The Hollow", treasury = 3000, income = 40, upkeep = 30, exposure = 0.3, persona = [0.6, 0.7, 0.1], data = [0, 0, 0], tech = [1, 1, 1] },
  { name = "Ninefold Lodge", kind = "Syndicate", branch = "Ninefold", treasury = 3000, income = 40, upkeep = 30, exposure = 0.3, persona = [0.5, 0.8, 0.1], data = [0, 0, 0], tech = [1, 1, 1] },
  { name = "The World", kind = "World", branch = "", treasury = 1000000, income = 0, upkeep = 0, exposure = 0.0, persona = [0.5, 0.5, 0.5], data = [0, 0, 0], tech = [3, 3, 3] },
]
```

The megacorp rows' `treasury` defaults to M11's `[corps] outside_treasury_initial` (100000) when omitted.

## 2. Orders, reinforcement and retaking

### The considerations table

Inputs per parent (megacorp and syndicate; the State has its own, § 4): `cash` = the weakest living branch's M11 `cash` (treasury ÷ `treasury_ref`); `wealth = U(treasury ÷ treasury_ref)`; `loss = branch_loss`; `margin = (income_7d − 7 × base_upkeep) ÷ (7 × base_upkeep)`; `att = (attitude + 1) ÷ 2`; `recent` = a `BuildingLost`, `LostDistrict`, `EmployeeKilled ≥ 2`, `Raided` or `Sacked` shock on a branch in 14 days; `rival` = the living rival parent's branch with the lowest M15 `Regard(own branch, rival branch).value` in a shared niche. Rescored daily, `hysteresis` 0.10, ties to the current order; an order holds at least `min_hold_days`. The daily action is capped by `max_spend_frac × treasury` across all orders.

| Order | Considerations | Daily action |
| --- | --- | --- |
| Hold | flat `order_flat.hold` (0.3) | nothing |
| Subsidise | `Can(a living branch ∧ cash < subsidise_below)` → GATE; `1 − cash` → Linear{0.8,0.2}; `wealth` → Linear{0.5,0.5}; `att` → Linear{0.6,0.4}; `exposure` → Linear{0.5,0.5} | pays `min(subsidy_cap_day, (subsidise_to × treasury_ref − closing))` to the weakest branch (`Flow::ParentFund`, capital); `Subsidised` |
| Reinforce | `Can(recent ∧ last_reinforce + reinforce_cooldown_days ≤ today ∧ treasury ≥ body_cost × bodies_per_call)` → GATE; `loss` → Logistic{8,0.3}; `pride` → Linear{0.6,0.4}; `att` → Linear{0.5,0.5}; `wealth` → Linear{0.5,0.5} | sends `bodies_per_day` bodies until `bodies_per_call` (below); `Reinforced` |
| Retake | `Can(a building lost in retake_window_days with a living holder ∨ a district lost ∧ wealth ≥ retake_wealth_min)` → GATE; `loss` → Linear{0.7,0.3}; `pride` → Quadratic{2,1,0,0}; `exposure` → Linear{0.5,0.5}; `att` → Linear{0.6,0.4} | buy-back, then contracts (below) |
| Sanction | `Can(rival with Regard value < sanction_regard ∧ margin ≥ 0)` → GATE; `1 − Regard value` → Linear{0.7,0.3}; `pride` → Linear{0.5,0.5}; `margin` → Linear{0.5,0.5} | sanctions the rival's parent (§ 3) |
| Campaign | `Can(rival ∧ margin ≥ campaign_margin ∧ no open campaign contract)` → GATE; `1 − Regard value` → Linear{0.6,0.4}; `1 − lawfulness` → Linear{0.6,0.4}; `greed` → Linear{0.5,0.5}; `wealth` → Linear{0.5,0.5} | posts one contract against the rival (§ 3) |
| FoundBranch | `Can(no living corp branch ∧ abandoned_until passed ∧ treasury ≥ branch_seed × 3)` → GATE; `exposure` → Linear{0.7,0.3}; `pride` → Linear{0.6,0.4}; `days since the last branch died ÷ 14` → Logistic{8,0.5}; `wealth` → Linear{0.5,0.5} | founds a branch (below) |
| Abandon | `Can(a living branch ∧ attitude < abandon_attitude)` → GATE; `U(spent_30d ÷ (30 × income))` → Logistic{8,0.5}; `greed` → Linear{0.6,0.4}; `1 − exposure` → Linear{0.6,0.4}; `1 − att` → Linear{0.7,0.3} | cuts every branch loose (below) |

### Reinforcement: the agents from abroad

A body is one call to `demography::spawn_immigrant` with an `Arrival` override: `Skills.fighting` and the niche's role skill at `outsider_skill` (0.6), `loyalty ≥ outsider_loyalty`, `lawfulness` drawn from `persona.lawfulness ± 0.2`, `outsider_coins` in the Wallet, Coarse, at the edge road nearest the branch. It gets an `Outsider { from, arrived, tour_until = arrived + tour_days, cohort }`, **a signed `Job`** at the branch building most short of staff (the role `ownership::role_for` its kind; a Security branch's bodies are guards, M11 `security_guards` raised by the count), Colleague edges (affinity 0.4) to its cohort, and M15's `own_bias` toward the branch doubled. Housing: a vacant bed in a branch-owned Block, else a Hotel night paid daily by the parent (`Flow::ParentFund` to the guest, then `Flow::Hotel`), else the emptiest Home. Each body costs the parent `body_cost` (paid abroad, `minted −= body_cost`) plus `outsider_coins` and `wage_fund_days` of the role's wage, paid into the branch (`Flow::ParentFund`). Caps: `bodies_per_day`, `bodies_per_call` per Reinforce episode, `outsiders_cap` living per parent.

Bodies count toward population and the immigration stats as `outsiders_arrived`, not against `immigration_per_week`. At `tour_until` an Outsider with no Spouse, child or owned building emigrates through `demography`'s emigration path (`OutsiderLeft`); one who has them loses the component and stays (`Settled`). A body killed in the city opens nothing special: its death, holes and reports are any agent's (M10 § 3 applies unchanged).

### Retaking

- **Buy-back.** Daily, for the newest lost building (from the branch's `loss_log` and `Acquired`/estate events) with a living holder: an offer at `retake_premium × value`, funded by the parent into the branch, then `corps::acquire`. A corp holder accepts under M11's seller rule; an agent holder with `greed ≥ 0.5` or purse < value accepts; a gang never sells. Two refusals move to force.
- **Force.** The branch posts (buyer of record: the branch; `Origin::Parent`) a **`Retake`** contract (`ContractKind::Retake`, appended; `Target::Building(b)`, price `price_base.retake` × risk as M16 § 1): a mission on the building (M16 § 3) whose win clears the occupants, squatters and the holder's guards, and then forces the sale by `leverage::coerce(Threat, Sell(b))` with `lev_bias.threat + retake_bias`; a squat (`owner = None`) is bought from the city at value. It is **first offered to the branch itself**, crew its Outsiders (a corp is an M16 taker, scored by its best member); unaccepted after one match day it goes to the board. When the loss was a raid, riot or squat by a named gang and `persona.lawfulness < hit_lawfulness`, a `Hit` on that gang's leader is posted alongside (the M16 strike gate and political cost apply).
- **Guard.** Every remaining branch building without a guard contract gets one, funded (`Guard`, M16).
- **District.** A lost district is retaken through its buildings: the order targets the lost district's buildings first, and M12's control recomputes from presence; `BranchRetook` fires when the branch controls it again.

### Founding a branch, and a local corp founding a parent

**FoundBranch** spawns a corp named `branch_name` (suffixed " II", " III" when the old name lived), the parent's niche set, treasury `branch_seed` (`Flow::ParentFund`), `upkeep_grace_days` of grace (M11's incorporation grace), and `parent = Some(id)`; on the same day it buys the cheapest estate or Lot building of its niche (`founding::build_on_lot` for a Security Office) and the next Reinforce is armed with `bodies_per_call` at no cooldown. `BranchFounded` event.

**A local corp founds a parent** when it has held `treasury ≥ found_parent_treasury` and a top niche share ≥ `found_parent_share` for `found_parent_days`, with no parent and `accounts < max_accounts`: `found_parent_frac` of its treasury moves abroad (`Flow::Remit`) as the new account's treasury, `base_income = parent_income_rate × moved`, `base_upkeep = parent_upkeep_rate × moved`, persona from its exec, tech from its own `Tech`, exposure `0.5`. `ParentFounded` event. Expected 0–1 per 120 days.

**Abandon**: every branch's `parent = None`; each Outsider's tour ends at once; branch scrip is redeemed by the parent at `health` into the holders' wallets (`Flow::Scrip`); `abandoned_until = today + abandon_days`; `Abandoned` event, `CorpShock::Abandoned` (0.7, new) on the branch. The branch lives or dies by M11 alone from then on.

```toml
[outside.orders]
hysteresis = 0.10
min_hold_days = 3
max_spend_frac = 0.05              # of treasury per day, all orders
subsidise_below = 0.3
subsidise_to = 0.5
subsidy_cap_day = 1500
reinforce_cooldown_days = 21
bodies_per_day = 2
bodies_per_call = 6
outsiders_cap = 24
body_cost = 300
outsider_coins = 60
wage_fund_days = 30
outsider_skill = 0.6
outsider_loyalty = 0.7
tour_days = 60
retake_window_days = 30
retake_wealth_min = 0.2
retake_premium = 1.5
retake_bias = 0.3
hit_lawfulness = 0.4
sanction_regard = -0.4
campaign_margin = 0.1
branch_seed = 4000
upkeep_grace_days = 14
abandon_attitude = -0.5
abandon_days = 60
found_parent_treasury = 20000
found_parent_share = 0.5
found_parent_days = 30
found_parent_frac = 0.5
parent_income_rate = 0.02
parent_upkeep_rate = 0.017
order_flat = { hold = 0.3, subsidise = 0.0, reinforce = 0.0, retake = 0.0, sanction = -0.1, campaign = -0.1, found_branch = 0.0, abandon = -0.2 }
[contracts]                        # additions (M16)
price_base = { retake = 500, abroad = 800 }
deadline_days = { retake = 10, abroad = 14 }
```

## 3. Killing a megacorp

### The uplink

`NodeKind::Uplink(OutsideId)` (appended, M14 § 1) exists for each living parent with a living corp branch: `owner` the branch, `pos` the branch's first building's door, one `Trunk` link of tier `uplink_link_tier` (3) to the branch's Ledger node, so only a tier-3 deck reaches it and only through the branch. Its ICE is `uplink_ice` capped by the parent's Deck tier (`ice_maker` = the branch; the cap reads `tech[Deck]`, not the branch's). Its `store` mirrors the parent's `data` (the node is a window, the units live on the account). `virt::relink` adds and drops it on branch founding, death and abandonment. Purposes against it are M14's, with outside effects:

| Purpose | Effect at Out |
| --- | --- |
| `Data { wipe: false }` | takes `min(data[t], steal_units[deck_eff])` per track, largest first, as M14; then `edge −= edge_per_unit × units` (`UplinkBreached`) |
| `Data { wipe: true }` | the parent's store to 0 in the touched tracks; `edge −= edge_wipe`; each touched track with `tech[t] ≥ 2` drops one tier at once; every branch's effective tier in that track is capped at the new ceiling (M14's re-kit of every asset the branch made) |
| `Ledger` | `min(uplink_frac × treasury, uplink_cap)` from the parent's treasury to the patron (`Flow::Hack`, inbound across the boundary) |

`edge` recovers `edge_recover × (1 − edge)` a day while every track's `data[t] ≥ data_floor`, and gains `data_per_day[t]` a day per track from abroad. A parent's Data is also how its branches keep their tiers: when a branch's tech upkeep would lapse for want of Data, the parent ships `upkeep_data[tier]` units into its Lab (`DataShipped`, no coins). A branch's `TechGained` lifts `edge` by `edge_tech`; `TechLost` cuts it by the same.

Who runs it: the M14 Hack goal and the gang and corp `VirtRaid` orders score an uplink as one more target with `EV = value_per_edge × edge cut × base_income ÷ 100` for patrons whose branch is a rival in a shared niche, and the parent's Campaign (below) posts M16 `Steal { Node(uplink) }` heists against a rival.

### Economic warfare

No new action: the ledger reads what M11–M16 already do.

- **Undercut, strikes, a supply cut.** Anything that shrinks the branch's cashflow cuts remittances, and `remit_gap` enters `branch_loss`. A district strike (M12) against the branch, a rival's `Undercut` to the floor, a Farm haul cut, a protection racket (M16) on its shops: each bleeds the parent through `exposure`.
- **Lost buildings and territory.** Bankruptcy sales, `Acquired`, raids, squats and M16 `Sell` coercions enter `branch_value`; `LostDistrict` enters the district term.
- **Sanctions.** A Sanction order by a rival parent adds `Sanction { by, on, until = today + sanction_days, cut = sanction_cut }` and costs the sanctioner `sanction_cost` a day while it runs. The State (§ 4) sanctions a parent whose branch's heat ≥ `state_sanction_heat`: no Subsidise or Reinforce crosses for `sanction_days`, and its cut applies. The city's `Embargo(id)` lever is a sanction by `None` with no expiry while set: no remittance leaves and no Subsidise, Reinforce or scrip redemption enters.
- **Campaign.** The order posts one contract a day, buyer of record its branch, until `campaign_contracts` are open: a `Steal { Node(rival uplink) }` heist when the branch holds no tier-3 runner, else its own `VirtRaid` against the uplink; an `Abroad` on the rival parent's depot (§ 5); a `Beat` on the rival branch's best staff (M15 poaching's target list) when `lawfulness < 0.4`; a `Spin` against the rival exec. A campaign is a string of contracts, not a war.

### Death

At death: `dead = true`, `ParentDied` event (red, logged); every corp branch gets `CorpShock::ParentDied` (1.0, new) and is **bankrupted by the M11 rule at once** (`corps::bankrupt`: estate sales under the estate rule, foreclosure, `dissolve`); every gang branch loses its `parent` and every Outsider's tour ends; all its scrip is wiped (§ 4); its uplink node is dropped; its treasury, positive or not, is absorbed by the outside (`minted −= treasury`, then `treasury = 0`). A dead account stays in the list for the panel and the CSV; its id is never reused.

```toml
[outside.virt]
uplink_link_tier = 3
uplink_ice = 3
edge_per_unit = 0.0003
edge_wipe = 0.15
edge_recover = 0.01
edge_tech = 0.02
edge_min = 0.2
edge_max = 1.5
data_floor = 100
data_per_day = [5, 10, 5]
uplink_frac = 0.01
uplink_cap = 2000
value_per_edge = 1.0
sanction_days = 30
sanction_cut = 0.08
sanction_cost = 60
campaign_contracts = 2
```

## 4. Scrip, gangs and the State outside

### Scrip

`Wallet.scrip: SmallVec<[(OutsideId, i64); 2]>` (`#[serde(default)]`). A corp branch with a living, non-sanctioned parent pays `scrip_wage_share` of every wage in its parent's scrip; the coin equivalent goes to the parent at once (`Flow::Remit`), and `scrip_out` rises. **Spending**: an agent buying food at, or paying rent to, a building owned by a branch of the issuer pays scrip first, at par while `health ≥ scrip_par_min`, else at `health`; the branch is paid the coin value by the parent (`Flow::Scrip`, inbound), and the scrip is retired. **Conversion**: `ConvertScrip` (new `ActionKind`, dur 10, at an open Fixer) pays `scrip × health × (1 − fixer_cut)` to the agent and `scrip × health × fixer_cut` to the Fixer's owner, both from the parent's treasury (`Flow::Scrip`); the Shop goal adds it for an agent with scrip ≥ `convert_min` and coins < one day's food. Statistical agents convert in the M15 daily pass on one hash-picked day in seven. Health below `scrip_par_min` pushes a `ScripDiscount` memory (salience 0.3, valence −0.3) on holders weekly.

**The wipe.** On a parent's death or Abandon at `health < scrip_par_min`, every holder's scrip in it goes to 0 (`ScripWiped` event with the total; memory `ScripWiped`, salience 0.8, valence −0.7; employees of the branch form a `Grudge { cause: Betrayed }` on its last exec at `scrip_grudge` × share of their wealth lost). Each district's M12 unrest rises by `scrip_unrest × wiped ÷ max(district coins, 1)`, so a big wipe in a branch's housing districts is a riot by the existing streak rule. `BanScrip` (lever) stops issuance and redeems every holder at `health` through the parent.

### Syndicates: the brothers

A syndicate's branch is one gang (`Gang.parent`). Its orders are a subset of § 2's (Hold, Subsidise, Reinforce, Abandon), with these rules in place of the corp ones:

- **Subsidise** pays `syndicate_subsidy` into the gang treasury after a `Shock::Sacked` or `Shock::Raided` (`Flow::ParentFund`).
- **Reinforce** fires on `Shock::LeaderChanged` from a death or arrest, or `Shock::Sacked`, and sends `brothers` bodies (members, not employees: `JoinGang` at arrival, fighting `outsider_skill`, an Enemy edge to the killer when one is known) after `brother_delay_days`, at `brother_cost` each, then nothing until `syndicate_cooldown_days`. `BrothersArrived`.
- **Refound.** An emptied gang with a living syndicate re-forms **only** this way, overriding M12's arrested-bootstrap: after `refound_days`, if the syndicate's treasury ≥ `refound_cost` and `attitude ≥ refound_attitude`, `brothers` bodies arrive and claim the old Hideout (unless sacked or inside a district held by another gang, in which case they found on a vacant Lot or derelict in Sump Central, else nothing). The first refound costs `refound_cost`; each later one within `refound_window_days` needs `attitude ≥ refound_attitude + 0.3`. `GangRefounded`.
- **Tribute** keeps the account fed (§ 1 step 4); a gang paying none for `tribute_lapse_days` lowers attitude by `att_cost` a day, and an Abandoned gang is on its own (M12 rules).

### The State

The Law's account decides daily among **Hold**, **Fund**, **Mandate**, **Withdraw** and **Sanction**, from `city_heat = 0.4 × U(murders_7d ÷ heat_murders) + 0.3 × U(riots_30d) + 0.3 × max gang heat` (M15) and `corrupt = 1 − Law.honour` (M15):

| Order | Considerations | Daily action |
| --- | --- | --- |
| Hold | flat `order_flat.hold` (0.3) | nothing |
| Fund | `Can(state_guards < state_guards_max ∧ treasury ≥ grant_week)` → GATE; `city_heat` → Logistic{8,0.4}; `1 − corrupt` → Linear{0.6,0.4}; `wealth` → Linear{0.5,0.5} | raises `levers.state_guards` by `state_guards_step` (the reconcile hires to `guard_count + state_guards`), pays a week of their wages ahead to the Treasury (`Flow::Grant`) each Monday; `StateGrant` |
| Mandate | `Can(city_heat ≥ mandate_heat ∧ no mandate running)` → GATE; `city_heat` → Linear{0.8,0.2}; `persona.pride` → Linear{0.5,0.5} | pushes `LawShock::Mandate` (0.6, new): Crackdown and Garrison scores + `mandate_bias` for `mandate_days`; `StateMandate` |
| Withdraw | `Can(state_guards > 0)` → GATE; `1 − city_heat` → Linear{0.7,0.3}; `corrupt` → Logistic{8,0.5} | lowers `state_guards` by `state_guards_step`; the reconcile dismisses by `economy::dismiss`; `StateWithdrew` |
| Sanction | `Can(a megacorp branch with heat ≥ state_sanction_heat)` → GATE; `heat` → Linear{0.7,0.3}; `persona.lawfulness` → Linear{0.6,0.4} | sanctions its parent (§ 3) |

The State never pays the city's own guards, never pins a posture (the M9 pin stays the player's), and never dies: its income equals its upkeep and a negative treasury only forces Withdraw.

```toml
[outside.scrip]
enabled = true
scrip_wage_share = 0.3
scrip_par_min = 0.3
convert_min = 20
scrip_grudge = 0.6
scrip_unrest = 0.5
[outside.syndicate]
syndicate_subsidy = 150
brothers = 3
brother_delay_days = 5
brother_cost = 150
syndicate_cooldown_days = 30
refound_days = 21
refound_cost = 600
refound_attitude = 0.0
refound_window_days = 60
tribute_share = 0.1
tribute_floor = 100
tribute_lapse_days = 14
[outside.state]
state_guards_max = 12
state_guards_step = 4
heat_murders = 10
mandate_heat = 0.6
mandate_bias = 0.2
mandate_days = 14
state_sanction_heat = 0.7
```

## 5. The outside as combat maps (hook)

`ContractKind::Abroad` (appended) with `Target::Outside { on: OutsideId, asset: OutsideAsset }`, `OutsideAsset { Depot, DataCentre, Convoy }`. Posted by a Campaign, by a corp buyer whose rival has a parent, or by god (and M18's player). It is **always Ledger**: a taker and crew who accept are marked `Away { contract, back_at }` (a component: off the map, not drawn, no needs decay, skipped by exec and by every co-location rule, never Statistical-scanned, never promoted) for `abroad_days`, then resolved by one draw from `SimRng::contract(id)`: `p = logistic(strike_k × (S_crew ÷ asset_def − 1))` with `asset_def = abroad_def[asset] × (0.5 + 0.5 × parent's edge)`. Success: Depot moves `abroad_loot` from the parent's treasury to the buyer (`Flow::Payout`, inbound, the crew paid as M16); DataCentre takes `abroad_data` units per track into the buyer's store and cuts `edge` as an uplink theft; Convoy adds a `Sanction { cut: convoy_cut, until: +7 days }`. Each crew member dies abroad with `abroad_death_p × (1 − p)` (`DeathCause::Violence`, no body, no report in the city; kin learn it by a first-hand `Killed` deed memory with actor `None`). Survivors return at an edge road. `AbroadResolved` event.

**The transient cast.** The draw also produces `cast_size` records, never entities:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CastMember { pub seed: u64, pub name: String, pub role: CastRole, pub personality: Personality, pub disposition: f32 } // −1..=1 toward the crew
pub enum CastRole { Guard, Clerk, Runner, Fixer, Bystander }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Promoted { pub cast: CastMember, pub toward: EntityId, pub rel: RelKind, pub arrive_on: u32 }
```

Names and personalities are hash-derived from `(contract, i)`; `disposition` from the outcome, the crew leader's honour and a hash draw. A cast member with `disposition ≥ promote_friend` or `≤ promote_enemy` is **promoted** with `p = promote_p` (× `promote_pinned_mult` when a crew member is pinned: M18's player): pushed to `outside.promoted` with `toward` the crew member they met (the leader), `rel` Friend or Enemy, and `arrive_on = today + hash % promote_delay_days`. On that day `demography::spawn_immigrant` creates them with their stored name and personality, a `RelKind::Friend` (affinity 0.6) or `Enemy` (affinity −0.6, and a `Grudge { cause: Abroad, weight: 0.7 }` that M15's Hunt reads) edge to `toward`, and `Promoted` fires. Promotion caps at `promoted_max` living. No map is generated; M18 renders the mission as a map from the same records.

```toml
[outside.abroad]
abroad_days = 3
abroad_def = { depot = 3.0, data_centre = 4.0, convoy = 2.0 }
abroad_loot = 1500
abroad_data = 150
convoy_cut = 0.05
abroad_death_p = 0.3
cast_size = 6
promote_friend = 0.6
promote_enemy = -0.6
promote_p = 0.3
promote_pinned_mult = 3.0
promote_delay_days = 30
promoted_max = 8
```

## 6. The Blackwall, bunkers, attribution

- **The Blackwall** (`Blackwall { cap: u8, breach_p: f32, last_breach: Option<Tick> }`, owned by the World account). `blackwall_cap` (3) is the ceiling of every deck, ICE and future daemon tier; M14's tier arrays already stop there and M17 makes the number a config key the daemon milestone reads. A successful tier-3 run on an uplink rolls `breach_p`: a `BlackwallBreach` (red) wipes the target's whole store and the patron's own Lab stores, flatlines the runner, and sets every node of the patron's to `alarm_until + breach_alarm_days`. Never more than one per `breach_cooldown_days`. Rare by design: the one boogeyman.
- **Bunkers.** The outside traces: a parent's `FactionDb` (M14) receives a sighting of every agent who breached its uplink or killed one of its Outsiders, conf `trace_conf`, through the trace rule M14 already has; a hardened building does not hide its owner from a ledger. The bunker goal itself is M18's.
- **Attribution.** The outside opens no holes and its actions are never statistical guesses: coins, bodies and Data move when it decides. Contracts it causes carry the branch as buyer of record, so the M16 accessory rule names the branch's exec, never the parent; an Outsider's crimes bind lazily like anyone's (M10 § 3).

```toml
[outside.blackwall]
blackwall_cap = 3
breach_p = 0.002
breach_alarm_days = 7
breach_cooldown_days = 60
trace_conf = 0.8
```

## 7. The Statistical tier and LOD

| Data and behaviour | Full | Coarse | Statistical |
| --- | --- | --- | --- |
| The ledger, orders, sanctions, the State | faction-side, daily | same | same |
| Outsiders | arrive Coarse, exec | exec | demoted by the normal LOD rule after `outsider_coarse_days` (7); tours end by date at any tier |
| Brothers | arrive Coarse, join at once | same | as Outsiders |
| Scrip | paid and spent by exec | same | spent in the economy's Statistical food and rent steps by the same rule; converted in the daily pass |
| Abroad crews | `Away` at any tier; one draw | same | same |
| Uplink runs | M14 runs (Coarse at least) | same | promoted for the run, as M14 |

The StatTable is unchanged; `calibrate` is re-run for `ConvertScrip`.

## 8. Player levers and god commands

| Command | Effect |
| --- | --- |
| `Embargo(OutsideId, bool)` | A city sanction without expiry: no remittance out, no Subsidise, Reinforce or scrip redemption in. |
| `SetTariff(f32)` | The share of every remittance kept by the Treasury. |
| `BanScrip(bool)` | No issuance; holders redeemed at `health`. |
| `SetStateGuards(u8)` | Caps `state_guards` (0 refuses the State's men). |

CLI: `--lever day:Embargo:1006:true`. **God commands** (logged): `WipeParentTreasury(id)` (`minted −= treasury`, treasury to 0); `SanctionParent { id, days, cut }`; `ForceReinforce { id, bodies, coins }` (ignores cooldown and caps but not `outsiders_cap`); `KillParent(id)` (death at once, § 3); `SetParentEdge { id, edge }`; `SetOutsideOrder { id, order, days }`; `PostAbroad { buyer, on, asset, crew }`; `ForceUplinkRun { runner, parent, purpose }` (a deck at `blackwall_cap`, the run queued at once); `BlackwallBreach(node)`; `FoundParent(corp)`.

## 9. UI, events, CSV

New `EventKind`s (violet): `Subsidised`, `Reinforced`, `OutsidersArrived`, `OutsiderLeft`, `Settled`, `BuyBackOffered`, `BranchRetook`, `BranchFounded`, `ParentFounded`, `Abandoned`, `Sanctioned`, `CampaignPosted`, `ParentFailing`, `ParentDied`, `UplinkBreached`, `DataShipped`, `ScripWiped`, `BrothersArrived`, `GangRefounded`, `StateGrant`, `StateMandate`, `StateWithdrew`, `AbroadResolved`, `Promoted`, `BlackwallBreach`. New `ContractKind`s `Retake`, `Abroad`; `Target::Outside`; `Origin::Parent`; `NodeKind::Uplink`; `ActionKind::ConvertScrip`; `CorpShock`s `Abandoned`, `ParentDied`; `LawShock::Mandate`; `GrudgeCause::Abroad`; `MemoryKind`s `ScripDiscount`, `ScripWiped`; components `Outsider`, `Away`; `Flow`s `Remit`, `ParentFund`, `Tribute`, `Grant`, `Scrip`.

- **Outside panel** (new, `O`): one row per account (kind, treasury with a 30-day sparkline, income vs upkeep, health gauge, edge, rep, attitude, tech ceiling, Data, order and its trace, sanctions, branches, Outsiders living, scrip out, failing or dead); click a row for its books and the last 20 events.
- **Corp and gang panels**: parent, remitted and received in 30 days, Outsiders on staff, scrip issued.
- **Inspector**: the `Outsider` block (from, tour end, cohort), scrip in the Wallet with its value at health, `Away` with the mission, a promoted NPC's origin mission.
- **City panel**: an Outside section (inbound and outbound today and in 30 days, outside share of coins, the State's guards and mandate) and the levers.
- **Board panel** (M16): `Retake` and `Abroad` rows; an Abroad row shows the asset and the parent, never a map.
- **Map overlay** (`O`): edge-road arrival markers for 24 hours after a Reinforce, an Outsider ring on agents, the uplink as a node glyph on M14's Virt overlay.
- **CSV** (`--report`): per account `o{i}_treasury`, `o{i}_income`, `o{i}_health`, `o{i}_edge`, `o{i}_attitude`, `o{i}_order`, `o{i}_scrip_out`; `outside_in`, `outside_out`, `outside_share`, `outsiders_living`, `outsiders_arrived`, `outsiders_left`, `subsidies`, `reinforcements`, `buybacks`, `retakes_posted`, `retakes_won`, `branches_founded`, `parents_dead`, `uplink_runs`, `uplink_breached`, `scrip_paid`, `scrip_converted`, `scrip_wiped`, `brothers_arrived`, `gangs_refounded`, `state_guards`, `mandates`, `abroad_done`, `promoted`, and ledger columns `flow_remit`, `flow_parent_fund`, `flow_tribute`, `flow_grant`, `flow_scrip`.

## 10. Save compatibility

Every new field is `#[serde(default)]`: `Gang.parent`, `Law.parent`, `Wallet.scrip`, `levers.state_guards`; `World::outside` defaults empty. `Outsider` and `Away` are new component stores. Every enum variant above is appended. **A pre-M17 save** builds `World::outside` on load from `[outside] factions`, matching rows to corps and gangs by name and to the Law by `branch = "Law"`; a megacorp row takes its corp's `outside_treasury` (then zeroed) when nonzero; uplinks are added by `virt::relink` on load. `Config::v1_profile` sets `[outside] enabled = false`, which turns off every account, the uplink, scrip, brothers (M12's bootstrap rule is untouched) and the State, so the v1, M8 and M9 tests run unchanged. With `decisions = false` the ledgers tick and remittances flow but nothing is sent in; with `enabled = false` a 120-day CSV is byte-identical to M16 outside the new columns.

## 11. Testing and calibration

**Unit tests** (`citysim/tests/outside.rs`, `reinforce.rs`, `uplink.rs`, `scrip.rs`, `state.rs`): the conservation identity holds over a daily pass with remittance, subsidy, a scrip wage, a scrip rent payment, a Fixer conversion and an uplink siphon; income applies every factor in order and `minted` books it; `branch_loss` is 0 for a whole branch and rises on a sale, a lost district and a remittance gap; a parent with income under upkeep dies on day `death_days` and not one before; a dead parent bankrupts its corp branches through `corps::bankrupt` and wipes its scrip; Reinforce sends at most `bodies_per_day` a day and `bodies_per_call` an episode, each with a job at the branch and a cohort; an Outsider without kin leaves at `tour_until` and one with a Spouse stays; a buy-back accepted transfers at the premium, two refusals post `Retake` first offered to the branch; a `Retake` win forces the sale; FoundBranch makes a corp with the parent and the seed treasury; a local corp past the thresholds founds a parent; the uplink needs a tier-3 deck and ICE reads the parent's Deck tier; a wipe drops the parent's tier and caps the branch's assets; `edge` recovers only above `data_floor`; a Sanction cuts income and costs its sanctioner; `Embargo` stops every crossing; scrip at health below par spends at health; brothers arrive after the delay, not before, and not again inside the cooldown; an emptied gang with a syndicate does not re-form before `refound_days`; the State funds guards above `mandate_heat` and withdraws on a corrupt captain; an Abroad draw is the same on save, load and re-run, kills only abroad and promotes only past the thresholds; a promoted NPC arrives with the stored name and the edge; a Blackwall breach respects its cooldown; a pre-M17 save loads with the outside built and the treasuries carried.

**Acceptance, revised 2026-10-09 (docs/TESTING.md; supersedes the scenario gate below and the bands, majority, six-seed and pinned-run devices in the Goals bullets).** No milestone scenario gate: `tests/scenario.rs` is gone. The Goals bullets map onto the tiers: (1) every mechanism that must exist is a `core_sanity` bullet (`citysim/tests/core.rs`, `mechanisms()`: fired on at least one of seeds 42-44 in 120 days), or, when it fires on 2 or fewer of seeds 42-47, a unit test in a seeded world; (2) the sanity bounds (coin identity, population, starvation, assaults a day, Treasury, corps alive, held prisoners fed) are `core_sanity`'s and `core_year`'s and need nothing new; (3) every bullet about what a person does becomes a person probe (`citysim/tests/person.rs`: one pinned agent, one stimulus) or a `shadow --assert` bound on an archetype (`citysim-cli/src/shadow.rs`, a new archetype where the milestone adds one, its bounds measured with a margin); (4) each shock question is one god scenario per reaction class, 60 days, asserting that the world reacted. Calibration bands, Murders against a pinned run and per-seed trajectories are printed by `tools/analyze_run.py`, never asserted. For M17 the conservation identity is `core_sanity`'s already (extend `econ::identity` with the parents' treasuries); the `core_sanity` bullets are a remittance, a Reinforce arrival and a FoundBranch; the person probe an Outsider without kin who leaves at `tour_until`.

*Superseded:* **Scenario** (`citysim/tests/scenario.rs`, `#[ignore]` `test_m17_outside_seed_42`): the bullets under *Goals and acceptance*. **God scenarios** (`tests/god.rs`):

- `god_raze_arasaka`: on day 10 every Arasaka building is sold to Ninefold or the city (god `Bankrupt` with the estate barred to Arasaka). Gate: FoundBranch within 7 days, an Arasaka branch owns a building again within 30.
- `god_kill_militech`: Zetatech gets a tier-3 runner, `ForceUplinkRun` wipe on Militech's Deck and Industry tracks on days 10, 25 and 40 plus two thefts, and `Embargo` from day 30; Habitat is pinned to Undercut. Gate: `ParentDied` by day 120, Militech's Blocks in the estate, `ScripWiped` > 0, a riot in a Militech housing district within 10 days. **Control**: the same razing without runs or embargo leaves Militech's parent alive on day 120.
- `god_decapitate_twice`: kill The Hollow's leader on day 20 and its successor on day 35. Gate: brothers arrive between days 25 and 30 and nothing arrives for the second.
- `god_murder_wave`: `PostContract Hit` on ten agents on day 10 (M16). Gate: `StateGrant` and a Crackdown within 14 days; `StateWithdrew` within 45 days of the wave ending.
- `god_abroad`: `PostAbroad` on Arasaka's depot with a crew of four from Ninefold, `promote_p = 1`. Gate: a promoted NPC arrives with an edge to the crew leader within `promote_delay_days`.
- `KillParent Arasaka` on day 60 with M15 on: do Arasaka's guards' Hunt the Zetatech exec, does the unrest riot, does the city's Security niche refill.

**Calibration**, tuned via `[outside]` and its sub-tables only:

| Target | Band |
| --- | --- |
| Outside coins in ÷ `total_coins`, day 120 | 3–15 % |
| Net crossing (in − out) ÷ `total_coins` | within ±5 % |
| Subsidies per megacorp parent per 120 days | 1–8 |
| Reinforce episodes per 120 days (all parents) | 1–6 |
| Outsiders living at once, max | 6–40 |
| Retakes posted ÷ buildings lost by branches | 20–80 % |
| Megacorp parent deaths, baseline 120 days | 0 |
| Megacorp parent lifetime under `god_kill_militech` | 60–110 days |
| Syndicate reinforcements per 120 days | 1–4 |
| State guards living, mean | 0–8 |
| Scrip ÷ coins in branch employees' wealth | 10–35 % |
| Mean megacorp `edge`, day 120, baseline | 0.85–1.15 |
| Abroad contracts per 120 days | 1–6 |

**Throughput.** Nothing runs per agent per tick. The outside is ≤ 12 accounts with one daily tick of O(accounts + branches + sanctions); orders score ≤ 8 options each; bodies are ≤ 2 spawns a day per parent; scrip rides the existing wage, food and rent paths (one extra wallet check) and a daily conversion pass for Statistical holders on one day in seven; uplinks are ≤ 3 more nodes; Abroad is one draw per contract; promotions are ≤ 8. The pass must cost < 0.1 ms a day.

## 12. Phases

1. **The ledger**: § 1 (`Outside`, `OutsideFaction`, seeding and the pre-M17 migration from `outside_treasury`, the daily tick, remittance, tribute, health, attitude, failure and death without branches' reactions, the cross-boundary flows and the conservation identity), Hold and Subsidise, `Embargo` and `SetTariff`, the Outside panel and CSV. With `enabled = false` a 120-day CSV is byte-identical to M16; with `decisions = false` the conservation test passes for 120 days. Commit.
2. **Reinforcement and retaking**: § 2 (the considerations table, `Outsider` bodies with jobs, housing and tours, buy-back, `ContractKind::Retake` and `Origin::Parent`, Hit the raider, Guard funding, FoundBranch, a local corp founding a parent, Abandon), `ForceReinforce`, `SetOutsideOrder`, `god_raze_arasaka`. Commit. The first `OutsidersArrived` on seed 42 is the first visible M17.
3. **The weapons that reach the ledger**: § 3 (`NodeKind::Uplink`, the outside purposes, edge, tech ceiling and Data shipments, Sanction, Campaign, economic warfare inputs, death and the branch bankruptcies) and § 4's scrip (issuance, spending, `ConvertScrip`, the wipe, `BanScrip`), `god_kill_militech` and its control. Commit.
4. **Brothers, the State and abroad**: § 4's syndicates and the State, § 5 (`Abroad`, `Away`, the cast, promotion), § 6 (the Blackwall, traces), the remaining levers and god commands, § 9's panels and overlay, the remaining god scenarios. Commit. Watch it live: gang sizes and the Crackdown share must stay inside the M12 bands.
5. **Gate and polish**: the M17 scenario and god scenarios, calibration, README and docs, then `/code-review high <base>..HEAD` and a fix commit, then push.

## 13. Risks

- **Outside money swamps or drains the city.** Every crossing changes `total_coins`, which M11–M16 calibrated as a closed economy: subsidies inflate it, remittances and scrip issuance drain it. The bands on the outside share and the net crossing are the gate, `max_spend_frac`, `remit_share` and `scrip_wage_share` the brakes; phase 1 runs 120 days with `decisions = false` and checks the Treasury and the corps' bankruptcy dates against M16 before anything is sent in.
- **Immortal or fragile megacorps.** A parent that subsidises forever makes its branch unkillable; one that drifts under upkeep dies on noise. The baseline-zero deaths band, the `god_kill_militech` lifetime band and its control are three sides of the same tuning; `market_sd` stays small and death needs `death_days` of sustained failure.
- **Bodies and brothers re-open the hydra.** Outsiders and brothers raise population, gang sizes and fights. `outsiders_cap`, `tour_days`, the brother cooldown and `refound_days` bound them; the scenario's population, Assault and Murder bounds include them, and phase 4 checks gang sizes against M12's bands.

## 14. Out of scope (M18 and later)

The player in person, dialogue, a player taking `Abroad` and `Retake` jobs (the same call), the bunker goal and escape plans, the player meeting a promoted NPC (M18); generated combat maps for `Abroad` and anything rendered of the outside beyond records; other cities as places, travel by agents, outside wars and parent-to-parent mergers; daemons beyond the Blackwall cap and the breach event; scrip beyond megacorp branches, floating exchange rates and corp scrip for local corps; Power and Water as goods and outside shipments of them; imports re-pointed to the World account by default; map layers and interiors.
