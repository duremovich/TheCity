# TheCity

A cyberpunk city that runs by itself: 2,000 residents on a 256 x 192 map, living under owners. The end state is a place a player can go break, take over, or simply exist in, with a lot of little stories along the way. Corps own the Vat Farms, Street Markets and Blocks, gangs hold the Sump, the Precinct's guards answer to whoever bribes the captain, and districts decay as the money moves. Agents are utility-driven over a GOAP planner; off screen they live on cheaper tiers but still commit crimes and have a biography you can read. The sim is the product; the player comes after it stands on its own. See [docs/VISION.md](docs/VISION.md).

The full v1 build spec lives in [docs/SPEC.md](docs/SPEC.md).

## Layout

- `citysim` — headless simulation library
- `citysim-app` — macroquad renderer and debug UI
- `citysim-cli` — headless runner and calibration

## Running

```
cargo run -p citysim-cli -- run --days 30 --seed 42 --report
cargo run -p citysim-app
```

`citysim-cli run` flags: `--days N --seed S` (required), `--report` (daily CSV on stdout), `--events` (events on stderr), `--population N` (override `[world] population`), `--map FILE` (override the map), `--lever "day=D:name=value"` (repeatable, e.g. `day=30:law_posture=crackdown`, `law_posture=auto` releases the pin), `--save-at T`, `--load FILE`, `--force-lod full|coarse|stat`, `--diag` (with `--events`: a daily line per Street Market and per corp), `--v1-profile` (the 300-resident v1 city).

Levers: `release_reserve`, `tax_rate`, `sentence_mult`, `guard_count`, `immigration_per_week`, `dole_per_day`, `law_posture`, and since M11 `city_rent=1/2/4` (rent on city Blocks, Sump/Mid/Spire), `rent_cap=3` or `rent_cap=none`, `no_city_evictions=1`, `breakup=<corp slot>`. God levers (out of the rules, for the god suites): `kill_leader`, `kill_gang`, `seize_gang`, `jail_gang=<gang>:<days>`, `fund_gang=<gang>:<coins>`, `fire_guards`, `treasury=<coins>`, and for corps (by seeding slot 0-7; the CSV column corpN is slot N-1) `fund_corp=<slot>:<coins>`, `bankrupt_corp=<slot>`, `seize_corp=<slot>:city|gang<i>|<slot>`, `kill_exec=<slot>`, `kill_staff=<slot>`, `strike=<slot>`, `corp_order=<slot>:<Order>[:<Niche>]:<days>`, `wipe_corps=1`. Nationalise and Subsidise are app buttons (building and corp panels).

M12 levers (districts by index 0-7: Spire, Civic, Vats, Mid West, Mid East, Sump West, Sump Central, Sump East): `guard_weight=<d>:<0..5>` (0 withdraws the law), `stance=<d>:patrol|sweep|cordon|withdrawn|crackdown<gang>|auto`, `sanitation=<count>` (city sweepers), `sanitation_weight=<d>:<0..5>`, `curfew=<d>:on|off` (Vagrancy x2, submission up, happiness -0.05), `riot_response=contain|disperse|crush|auto`. M12 god levers: `riot=<d>` (a riot at the next muster hour, unrest ignored), `litter=<d>:<0..1>` (every street tile), `split_gang=<gang>` (the split rule minus the roll), `derelict=<building index>`, `buy_building=city|gang<i>|corp<slot>|<agent index>:<building index>:<price>`.

M13 levers: `stims_legal=on|off` (Markets restock and sell Stims; dealing stays a crime but nothing is confiscated), `asset_tax=<class>:<rate>` (upkeep x `1 + rate` for `moto|car|truck|flyer|implant|robot|pack|bridge`, rate 0..5), `impound=on|off` (the city impounds vehicles with unpaid upkeep; on by default). M13 god levers: `grant_asset=<agent index>:<kind>:<tier>` (kinds `moto|car|truck|flyer|arms|legs|nerves|eyes|skin|robot|pack|bridge`, free, no import), `wreck=<asset index>`, `chrome_everyone=<tier>` (every adult gets Arms and Nerves of that tier, filled slots kept), `flood_stims=<d>:<n>` (n doses into the Hideout stock of every gang dealing in, holding or hiding in district d), `brick=<corp slot>|agent:<index>` (every implant that lender financed is bricked and its loan called), `chase=<agent index>` (the agent's next trip counts as a chase for the crash roll).

M14 levers (the Virt plane, an abstract second board of nodes laid over the buildings; every run is one seeded dice contest per guarded node): `city_ice=<0..3>` (the Treasury's and the Precinct's ICE, installed or lowered one tier at a time, the upkeep the Treasury's), `data_tax=<0..1>` (an extra share of every Data sale to the Treasury, out of the seller's take), `hack_sentence=intrusion|data_theft:<days>` (replaces the base sentence; the sentence multiplier still applies). M14 god levers: `wipe_data=<building index>`, `set_tech=<corp slot>:<chrome|deck|industry>:<tier>`, `grant_data=<corp slot>|gang<i>:<track>:<units>`, `grant_deck=<agent index>:<tier>` (free), `set_ice=<building index>:<0..3>` (the city's own, free), `fry=<agent index>` (a jacked-in agent is fried at once, at command time, a lost contest at its next contested node with no save; the kill roll uses `p_flatline` only at ICE 3, else `p_fry_kill`), `run_now=<agent index>:<building index|corp<slot>>:<data|wipe|ledger|door>` (pins a run for an agent with a deck; the contest rules apply). In the app the City panel's Virt section has the city ICE and data tax sliders and the two hack sentences.

`citysim-app` flags: `--seed`, `--load`, `--map FILE`, `--fps`, and the screenshot helpers `--start-tick T`, `--screenshot out.png` (frames the whole map unless something is selected), `--fit`, `--select INDEX`, `--select-kind Hideout` (a Rust `BuildingKind` name: `Market` is shown as Street Market, `Jail` as Precinct), `--select-name "First Last"`, `--tab story`, `--tab corp` (with `--select-kind`: open the panel of the corp owning the first building of that kind), `--no-autobind`. The City panel shows tier counts, open holes and the sim rate, one row per corp with a share bar per niche, the three classes (count, happiness, loyalty, submission, unrest) and the M11 levers (city rent per tier, rent cap, no city evictions); the event log has an "Unattributed" toggle that lists open holes and binds one on click. Clicking a corp (City panel, a building's owner, an inspector's employer) opens the Corp panel: niches with price level, share and monopoly flag, governance, exec, treasury and a 14-day cashflow sparkline, the bankruptcy countdown, the order and its trace, buildings, employees, contracts, Subsidise and Break up. Building outlines take the owner's colour (a corp's, a gang's, white for an agent, grey for the city); Lots are dashed. M14: `N` draws the Virt plane over the ghosted city (nodes coloured by ICE, Data glyphs, links, live runs as dots; a click selects a node or a run), the Node, Run and raid (Mission) panels, and the screenshot flags `--overlay virt`, `--select-node <index>`, `--select-runner` (the first jacked-in agent and its run), `--select-raid` (the first streamed raid).

With `B` the app draws the eight districts (borders, a fill in the controller's colour, Contested hatched) and a click on a street tile opens the District panel (aggregates, control and its presences, the law's allocation, stance and trace, litter, unrest and riot state, the street, the district levers); `L` draws the litter heat. The City panel lists the districts (controller, guards, unrest, litter, crime) under the sanitation and riot-response levers. `citysim-app` also takes `--districts`, `--litter` and `--select-district <index>` for screenshots.

The CSV (`--report`) since M11 adds rent and evictions (`evictions, rent_paid, rent_short, housed`), the money ledger per flow (`flow_food, flow_drink, flow_wages, flow_rent, flow_upkeep, flow_wholesale, flow_overflow, flow_restock, flow_contract, flow_tax, flow_dole, flow_other`), wealth (`wallets, wallet_gini, wallet_top10`), each seeded corp's `corpN_treasury, corpN_order` (slots 1-8, `0,-` once dissolved), `acquisitions, bankruptcies, monopolies, foundings, incorporations, strikes`, and the classes (`unrest_corp, unrest_street, unrest_dreg, class_corp, class_street, class_dreg, happiness_street`), all before `ticks_per_sec`. M12 adds, per district (columns `d1_` to `d8_`, d1 = Spire; the levers number districts 0-7), `d{i}_coverage, d{i}_control` (0 Contested, 1 City, 2 Gang, 3 Corp), `d{i}_litter` (share of street tiles visibly littered), `d{i}_unrest, d{i}_crime, d{i}_guards`, then `dregs, hotel_nights, squatters, derelicts, vagrancy, riots, crossfire, gangs`, also before `ticks_per_sec`.

M13 gives the city's people things. An asset is an entity with an owner (an agent, a gang, a corp or the city), an upkeep, a condition and maybe a finance plan; it can be repossessed (towed, bricked, taken), impounded, stolen, fenced, chopped for Parts, looted from a corpse and inherited. **Vehicles** (motorcycle, car, truck, flyer) change how long a step takes (road, ground, farmland), haul six batches on a Vat Farm truck, crash at the end of a trip (worse in a chase; reflex is the save) and are stolen from the street; corps run truck and patrol-car fleets and buy their execs flyers, gangs buy bikes for members. **Chrome** (Arms, Legs, Nerves, Eyes, Skin) is bought and installed at a Ripperdoc (Clinic), changes fights, sight and stealth, and costs sanity: low sanity rolls a berserk episode the law hunts; Therapy treats it; scavs strip and rip corpses, and gangs under the Harvest order abduct the chromed. **Stims** are cooked by gangs and dealt at Bars (illegal by default, the `stims_legal` lever lets Markets sell them), with addiction, withdrawal, overdose and Detox. The **security robot** is posted by a corp's Secure order, defends in brawls and detains thieves. Garages and Clinics are a ninth corp's niche (Tech) and agents can found them.

The CSV since M13 adds, before `ticks_per_sec`: `vehicles_moto, vehicles_car, vehicles_truck, vehicles_flyer, truck_hauls, walk_hauls, commute_tpt_walk, commute_tpt_drive, chrome_installs, chrome_agents, mean_sanity, episodes, hooked, stims_dealt, stims_legal, dealing_reports, repos, impounds, crashes, crash_deaths, vehicle_thefts, chops, abductions, stripped, robots, flow_asset, flow_asset_upkeep, flow_finance, flow_import, flow_stims, flow_parts, flow_treatment, overdoses, harvests, stripped_window, gang_income, gang_income_dealing, episodes_by_law, treatments, detoxes`, and the ninth corp slot (`corp9_treasury, corp9_order`). The app's assets overlay heats the districts by hooked adults (`A` per the plan, `K` where `A` pans the camera: the UI phase decides).

Display names differ from code names: Block (`Home`), Vat Farm (`Farm`), Street Market (`Market`), Precinct (`Jail`), Civic Hall (`Hall`), Recycler (`Cemetery`), Reserve Depot (`Warehouse`); money is shown as ¢. The CSV columns and the Rust identifiers keep the v1 names.

## Roadmap

| Milestone | Scope | Spec |
| --- | --- | --- |
| M0-M7 | v1: one gang, economy, law, social graph, demography | [SPEC](docs/SPEC.md) |
| M8 Factions | two gangs with a brain: orders, turf, raids, sacks | [M8](docs/M8_FACTIONS.md) |
| M9 The law | the guards as a faction: captain, postures, breakouts, bribery | [M9](docs/M9_LAW.md) |
| M10 Scale (done) | 2,000 residents, five zones, Statistical tier, holes and the binder, Life | [M10](docs/M10_SCALE.md) |
| M11 Ownership (done) | theme labels, owners and purses, rent and eviction, eight corps with a brain, founding, classes | [M11](docs/M11_OWNERSHIP.md) |
| M12 Districts (done) | eight districts with owners and moods, per-district law and stances, litter and sweepers, Hotels, derelicts and squats, riots, crossfire, splits, corp raids | [M12](docs/M12_DISTRICTS.md) |
| M13 Assets (done) | vehicles, chrome at the Ripperdoc, stims and dealing, the security robot, loot and the Tech niche | [M13](docs/M13_ASSETS.md) |
| M14 Data and Virt | a second plane, decks, ICE, Data as a resource, the tech tree | [post-M14](docs/ROADMAP_POST_M14.md) |
| M15 Word and blood | gossip, reputation, grudges, revenge chains | [post-M14](docs/ROADMAP_POST_M14.md) |
| M16 Contracts | hits, fixers, leverage moves, the quest board | [post-M14](docs/ROADMAP_POST_M14.md) |
| M17 The outside world | megacorp, syndicate and state parents at ledger LOD | [post-M14](docs/ROADMAP_POST_M14.md) |
| M18 The player | one more pinned agent, dialogue as a renderer over social state | [post-M14](docs/ROADMAP_POST_M14.md) |

## Verify

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release --no-fail-fast
# long scenario gates (v1 acceptance, M8, M9, M10 scale, M11 ownership, M12 districts and the three-seed split,
# Full-vs-Statistical parity, binder, median tick);
# each scenario is 25-60 s at 2,000 residents, so run them one at a time if you want readable output
cargo test --release -p citysim --test scenario --test lod --test scale --test bind -- --ignored --nocapture
# the M10 scale gate alone: throughput (release only, gate 8,000 ticks/s, target 12,000), off-screen violence,
# hole ledger, Unknown share, save size and time
cargo test --release -p citysim --test scenario test_m10_scale_seed_42 -- --ignored --nocapture
# the M11 ownership gate alone: every corp changes order, Squeeze and Undercut held, a corp-payer bribe, evictions,
# evicted gang recruits, an NPC founding and an incorporation, a hostile acquisition, no monopoly before day 60,
# a strike, immigration that moves, the M10 bounds and throughput (release only); prints the calibration readings
cargo test --release -p citysim --test scenario test_m11_ownership_seed_42 -- --ignored --nocapture
# the M12 districts gate alone: district traces, control, allocation and Crackdowns, gang landlords, litter bands and
# the sweepers, Vagrancy, Hotel nights, squats, the Dreg share, riots with loot and crossfire, raids that muster,
# hit corps and never depart into cover, the M10 bounds and throughput; prints the calibration table (spec § 10).
# Seeds 42-47 (M13 phase 5): riots by the six-seed mean, gang control and sanitation on half the seeds, gang
# landlords pooled >= 2/3, the split bullet on any seed; seed 42 alone for the mechanism checks and ticks/s
cargo test --release -p citysim --test scenario test_m12_districts_seed_42 -- --ignored --nocapture
# the M13 assets gate alone: vehicles and trucks, the commute, chrome and harvests, an NPC Clinic or Garage,
# episodes and Therapy, dealing, addiction and Detox, repossessions, crashes, theft and the chop, Secure robots,
# strips, the M10 bounds and throughput; prints the calibration table (spec § 11). Seed 42, with the coin-flip
# bullets (crash deaths, a chop after a theft, episodes, one ended by the law, an NPC seller) by majority of 42-44
cargo test --release -p citysim --test scenario test_m13_assets_seed_42 -- --ignored --nocapture
# the god suites: the player-lever scenarios (v1, gangs and the law, and the M13 assets scenarios of
# docs/GOD_SCENARIOS_V4.md), v2 (corps, classes, the economy) and v3 (districts, the street, riots)
cargo test --release -p citysim --test god -- --ignored --nocapture
cargo test --release -p citysim --test god_corps -- --ignored --nocapture
cargo test --release -p citysim --test god_districts -- --ignored --nocapture
# M12 unit tests: districts, the law in districts, litter, the street, riots and splits
cargo test --release -p citysim --test districts --test street --test factions
# M11 unit tests: ownership and rent, the corp brain, founding and classes
cargo test --release -p citysim --test ownership --test corps --test classes
# criterion bench (tick_300_agents, tick_2000_agents); the median gate is a test in tests/scale.rs
cargo bench -p citysim --bench tick
# regenerate the Statistical tier's table (assets/stat_table.toml): calibrate v2, 500 agents, gangless, 3 seeds
cargo run --release -p citysim-cli -- calibrate
# debug run with the event log (events go to stderr)
cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv
```

## Tools

Stdlib-only Python scripts in `tools/` (see [tools/README.md](tools/README.md)):

```
python tools/analyze_run.py run.csv events.tsv   # one run: economy, law, holes, ownership, corps, districts, throughput, flags
python tools/compare_runs.py a.csv b.csv         # per-column means of two runs
```

`assets/gen_map.py` regenerates `assets/map.txt` (write bytes, LF line endings).
