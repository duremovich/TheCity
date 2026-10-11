# Transit: the El

Companion to `JOBS_V2.md` (the jobs, the hiring key), `M13_ASSETS.md` § 2 (vehicles, trips, the flyer's off-map hop), `M12_DISTRICTS.md` (district crime, riots, curfews), `TESTING.md` (the tiers) and `VISION.md` ("Public transport rails can be damaged by accident or by terrorism"; roadmap addendum 6). Plan: `~/.claude/plans/transit.md`. Identifiers marked *new* don't exist at HEAD (`d771073`). Numbers marked *placeholder* are starting values; the rest are measured or computed from `assets/map.txt`.

**The problem (2026-10-10).** In the all-Full flipped city a worker is at the work for ~51 % of free, healthy shift minutes (Hunger as sickness P5f's diagnosis, seed 42, days 2-30): 16 % of shift minutes are the commute itself, and the Sleep (8-10 %) and the mid-shift meal (10 %) are partly the commute's too, because a 3-hour walk each way leaves no room for a night's sleep and a meal at home. The median home-to-work commute is ~90 Manhattan tiles (~3 h at the measured 2.05 ticks a tile); ~500 farmers walk a median 116 tiles to Farms on the east edge. Learned attendance (`StatTable.p_attend`): farmers 0.38, clerks 0.53, cooks 0.91. A parallel branch adds relocation and a hiring commute cap; this spec is the long-term answer the vision already promised: rails.

**The short version.** Three elevated lines (the El) run over the road grid from the Sump and Mid to the Vats, with stops at road intersections every 16 tiles. A ride is a deterministic timetable lookup (no train entities): walk to a stop, wait for the next departure, ride off-map like M13's flyer, walk from the stop. One pure function decides walk or ride for every tier, so Full, Coarse and Statistical agents commute by the same arithmetic. The rail is a corp concession: fares are its revenue, power is its cost, and the lines close when the street riots. Everything the vision hangs on rails (sabotage, gang turf, inspectors, station crime) gets a hook, not a mechanism.

---

## 0. What the El buys (computed from the map)

`net.py` (scratchpad; the plan's phase 1 ports it to `citysim-cli transit-report`): every Home door against every Farm door and against a fifth of all workplace doors, walking at 2 ticks a tile, the El at the config below (0.25 ticks a tile, 1-tick dwell, 5-tick average wait, 5-tick transfers, the 3 nearest stops at each end).

| network | stops | track tiles | access walk to the nearest stop, p50 / p90 tiles (Sump, Mid, Spire) | home → Farm, ticks p50 (walk → best) | home → any workplace, p50 (walk → best) | pairs where the ride wins |
|---|---|---|---|---|---|---|
| one line (Sump → Vats) | 24 | 360 | 19/35, 69/125, 93/133 | 440 → 173 | 304 → 201 | 63 % |
| two lines (Vat Line + Crosstown) | 38 | 592 | 19/43, 27/43, 37/53 | 440 → 154 | 304 → 159 | 86 % |
| **three lines (the proposal)** | **54** | **840** | **11/21, 21/35, 37/53** | **440 → 147** | **304 → 149** | **88 %** |

By commute length (three lines): a 70-100 tile walk (140-200 ticks) becomes ~130; a 100-130 tile walk (200-260) becomes ~142. **The access walk is the binding term, not the train**: at 0.15 ticks a tile instead of 0.25 the home → Farm median moves only 154 → 134. So the El alone cuts the measured median commute (~90 tiles, ~180 ticks) by about a quarter and the farmers' (~116 tiles, ~235 ticks) by about two fifths. The rest comes from where people live: the relocation branch should prefer Homes near a stop, and the hiring cap should read travel time, not tiles (§ 6). A static estimate; the plan measures the real one.

---

## 1. Decisions

| # | Question | Decision |
|---|---|---|
| T1 | What it is | **The El**: elevated rail over the road grid. Grade-separated, so road wreckage, crashes and traffic never touch it in v1 (rails get their own damage later, addendum 6). Not buses: buses on roads would need vehicle entities, road capacity and a crash model for 50-passenger vehicles. |
| T2 | Trains | **No train entities.** A line is a timetable: departures from each terminus every `headway` ticks, a stop's departure time is the terminus's plus the ride ticks to it. A ride is a lookup, never a moving body. Capacity is unlimited in v1 (the peak load per departure is printed). |
| T3 | Stops | A stop is a **road tile** at an intersection under the track (no building, no map change). The line's `waypoints` are road intersections; stops fall every `stop_spacing` track tiles from the line's first waypoint, plus every waypoint and every tile two lines share (the transfers). A nominal stop tile that is not walkable Road (a double-wide building overwrote the road) snaps to the nearest walkable Road tile, ties the lower `(y, x)`. Derived at world creation and on load from config and map; not saved. |
| T4 | Lines (*placeholder* network) | **Vat Line** `(7,127) → (231,127) → (231,15)`: Sump north to the Vat Farms. **Low Line** `(7,167) → (231,167) → (231,127)`: the deep Sump, joining the Vat Line at `(231,127)`. **Crosstown** `(39,15) → (39,63) → (231,63)`: Mid West, Civic, the Vats, crossing the Vat Line at `(231,63)`. The Spire is not served: the rich drive or fly. All coordinates are road lines (`7 + 8k`). |
| T5 | Ride time | `ride(s → t) = Σ track tiles × ride_ticks_per_tile + dwell_ticks × stops passed + transfer_ticks × transfers`, the minimum over the line graph, precomputed all-pairs (≤ 64 stops: a 64 × 64 `u16` table, 8 KB) at world creation and on load. |
| T6 | Wait | **Exact, from the timetable**: `next_departure(stop, line, dir, t)` = the first departure at or after `t`. Pure function of the tick. The wait at a transfer is not exact: `transfer_ticks` (half a headway) is folded into T5, the same for every tier. |
| T7 | The route | *new* `transit::route(world, who, from, to, t) -> Option<Route>`: over the `stop_candidates` stops nearest `from` (board) and nearest `to` (alight), the minimum of `walk(from, s) + wait(s, t + walk) + ride(s, t) + walk(t, to)`, where `walk` is `exec::timed_walk` (litter included). `Route { board, alight, line, dir, depart, arrive, total_ticks, fare }`. No RNG. ≤ 9 combinations of table lookups and two scans over ≤ 64 stops. |
| T8 | Walk or ride | Ride iff every test passes, in order (each refusal counted by cause): (1) `[transit] enabled`, the line open (T15) and in service hours; (2) Manhattan distance ≥ `min_ride_tiles`; (3) **time vs fare**: `walk_ticks − route.total_ticks ≥ min_saving_ticks + fare × ticks_per_coin`, where `ticks_per_coin` is `poor_ticks_per_coin` when the wallet holds less than `poor_meals` meals at the local price (a coin is worth more to the poor); (4) **can pay**: wallet ≥ fare, or the agent dodges (T10); (5) **danger**: at night (`night_from..night_to`), the worse of the two stops' districts by `crime_rate` above `danger_crime × (0.5 + courage)` refuses. An agent with a vehicle for the trip (`vehicles::vehicle_for_trip`) drives and never asks. |
| T9 | Fare | Flat `fare` per ride (transfers free), paid at boarding (Full, Coarse) or at the commute's hour (Statistical, T13), `Flow::Fare` (*new*, taxed like a Hotel night) from the rider to the operator. No passes in v1. |
| T10 | Fare dodging | A rider who can't pay (or whose `lawfulness < dodge_always`) jumps the gate if `lawfulness < dodge_lawfulness`; otherwise walks. Counted (`fare_dodges`), no crime filed in v1: the hook for ticket inspectors (T18). |
| T11 | The operator | A **corp concession**: `operator = "Arasaka"` (*placeholder*; a Security corp, so its guards are the later inspectors and its cameras the later station surveillance). Fares are the corp's revenue (wages from revenue count them). `power_per_line_day` per open line is paid at midnight to the World account (power imported until the Power milestone). The corp brain does not know it owns a rail in v1: no fare setting, no expansion. A bankrupt or dissolved operator's concession passes to the city (`operator` none: fares to the Treasury, power from the Treasury). `operator = "city"` is the city from day 0. |
| T12 | Full riders | The plan step stays `GoTo(key)` throughout; the exec chains the legs. *new* `World::rides: BTreeMap<EntityId, Ride>` holds the itinerary (as `World::trips` does for a vehicle). Leg 1 is an ordinary `walking_goto` to the board stop's tile. On arrival the agent stands on the platform (the stop tile) in *new* `ExecState::Ride { target, board, alight, depart, arrive_tick }` until `depart`, then is off the map (like `ExecState::Fly`: no co-location, no witness, uninterruptible) until `arrive_tick`, then stands at the alight stop and leg 3 starts as an ordinary goto to the original target. The fare is paid when the agent boards. A missed departure (arrived late) takes the next one. |
| T13 | Coarse and Statistical | **Coarse**: `timed_goto` reads `route` and arrives at `t + route.total_ticks` (litter on the walk legs only), pays at the start, counts the ride at the board stop. **Statistical**: no movement. On each workday, at the shift's first credited hour, the worker's home → work route (cached per agent, keyed by `(home door, workplace door, transit epoch)`) decides walk or ride; a ride pays `2 × fare` (out and back) and counts two rides in the stop ledger. Errands are not modelled off-screen (the parity rates count commute rides only). |
| T14 | Attendance (the parity term) | Today a Statistical worker's shift hours are credited at `StatTable.p_attend[role]` (Hunger as sickness P5f). **Re-key it by `(role, commute band)`**, the band from `route`'s door-to-door ticks (walk or ride, whichever wins): `< 30`, `30-60`, `60-120`, `120-180`, `≥ 180` (`p_attend` falls back to the role's row for an empty cell). `calibrate` learns it from the all-Full city with the El on. Then a line cut moves an off-screen worker into a longer band and his attendance falls, with no new rule. Needs P5f merged (§ 8). |
| T15 | Closures | A line or a stop is closed while `closed_until > tick`. **Riots** close every stop in a rioting district for the riot's span (trains run through); a **curfew** closes the district's stops from `night_from` to `night_to`. **God** `CutLine { line, days }` and `CloseStop { stop, days }` (*new* `PlayerCommand`s). A closed board or alight stop is not a candidate; a line cut leaves the riders to the rest of the network or to their feet. Nothing else closes a line in v1. |
| T16 | Departure gate | `routine::must_leave_for_work` reads *new* `transit::estimate(world, id, door)` (= the route's ticks if the agent would ride, else `fixes::walk_estimate`). A rider leaves later and sleeps longer. |
| T17 | Stops as places | Full riders waiting on a platform co-locate as on any street tile: the existing readers (social drift, gossip, sightings, witnesses, street encounters) see them, with no new code. *new* `TransitState.load[stop][hour]` counts boardings (all tiers) for the day: printed, read by nothing in v1 but the report; it is the hook for ambient platform density (addendum 13) and stop crime. |
| T18 | Hooks only (not v1) | Sabotage and power cuts close a line through T15's `closed_until` (one call). Gang turf: a district's controller is readable at each stop (a toll later). Ticket inspectors: the operator's guards stop dodgers (T10's count becomes a Crime). Station crime: pickpockets on the busiest platforms by `load`. Crime aboard. Rail damage as a `Damage` tile layer (addendum 6). |

---

## 2. Config (`[transit]`)

```toml
[transit]
# docs/TRANSIT.md. Every value a placeholder until the plan's phase 1 measures it.
enabled = true                 # CITYSIM_TRANSIT_OFF=1 turns it off (the vf_* pattern); off = today exactly
operator = "Arasaka"           # a corp name, or "city"
fare = 1                       # coins per ride, transfers free
headway = 10                   # ticks between departures from each terminus
ride_ticks_per_tile = 0.25     # 4 tiles a minute, ~8x walking
dwell_ticks = 1                # per stop passed
transfer_ticks = 5             # per transfer (half a headway)
stop_spacing = 16              # track tiles between stops
stop_candidates = 3            # nearest stops tried at each end
min_ride_tiles = 24            # no ride under this Manhattan distance
min_saving_ticks = 15
ticks_per_coin = 20            # what a coin of fare must save in time
poor_ticks_per_coin = 60       # ... for a wallet under poor_meals meals
poor_meals = 2
dodge_lawfulness = 0.25        # a rider who can't pay jumps the gate below this
dodge_always = 0.1             # ... and never pays below this
service_from = 300             # 05:00
service_to = 60                # 01:00 (no service 01:00-05:00)
night_from = 1260              # 21:00
night_to = 300                 # 05:00
danger_crime = 3.0             # crime_rate (per 100 residents a day) at which an average-courage rider refuses at night; set from phase 1's distribution
power_per_line_day = 150       # operator -> World account, per open line

[[transit.lines]]
name = "Vat Line"
waypoints = [[7, 127], [231, 127], [231, 15]]

[[transit.lines]]
name = "Low Line"
waypoints = [[7, 167], [231, 167], [231, 127]]

[[transit.lines]]
name = "Crosstown"
waypoints = [[39, 15], [39, 63], [231, 63]]
```

`Config::v1_profile` (unit tests on the 96 × 64 map) has `enabled = false` and no lines; a unit test builds its own line on the v1 map.

---

## 3. LOD: one function, three tiers

| | Full | Coarse | Statistical |
|---|---|---|---|
| decides | `route` + T8 at the GoTo's start | the same | the same, home → work, once a workday |
| moves | walks to the stop, stands on the platform, off-map aboard, walks on | `GotoTimed` for the route's total | none |
| pays | at boarding | at the start | `2 × fare` at the commute's hour |
| counts in `load` | the board stop, the departure hour | the board stop, the start hour | the home stop at the shift-start hour, the work stop at the shift-end hour |
| attendance | emerges (less walking, more sleep) | emerges | `p_attend[(role, band)]` learned from Full (T14) |

**Tier changes.** A demotion or promotion mid-ride ends the `Ride` (`transit::end_ride`, called where `vehicles::end_trip` is: `World::abort_plan`, `remove_agent`, `set_lod`); the fare is not refunded; the agent is snapped as today (`lod::snap_to_phase_door`). A Coarse agent promoted mid-`GotoTimed` keeps today's behaviour (re-plans its GoTo, which re-reads `route`).

**Parity.** The Full-vs-Statistical test (`lod.rs::test_full_vs_statistical_within_15pct`) gains: fares per worker-day (commute rides only on the Full side: a ride whose GoTo targets the agent's employer or Home on a workday), the ride share of commutes, and attendance by commute band. Full's errand rides are printed apart.

**Determinism.** No RNG anywhere in transit. `World::rides` and `TransitState`'s maps are `BTreeMap`s; the stop list is ordered by line then track position.

---

## 4. Data and columns

- *new* `systems/transit.rs`: `TransitState { lines: Vec<Line>, stops: Vec<Stop>, ride: Vec<u16> (all-pairs), nearest: per-district stop shortlist, closed_line: Vec<Option<Tick>>, closed_stop: Vec<Option<Tick>>, load: Vec<[u16; 24]>, epoch: u32 }`; `route`, `estimate`, `next_departure`, `board`, `end_ride`, `daily` (power, load reset, operator succession), `on_riot`, `on_curfew`.
- *new* `components::{Line, Stop, Ride}`, `ExecState::Ride`, `Flow::Fare`, `PlayerCommand::{CutLine, CloseStop}`.
- Saved: `World::rides`, `closed_line`, `closed_stop`, today's `load` and counters, the operator id. Derived on load: stops, the table, the shortlists. **`save::SAVE_VERSION` + 1** (6 → 7 at HEAD; the Hunger-as-sickness and Hit-market branches may bump it first: whoever merges last takes the next number). A pre-transit save loads with no rides and every line open.
- *new* `TransitCols` (the `JobsCols` pattern, its own header block, `is_zero` when off): `rides_full`, `rides_coarse`, `rides_stat`, `fares`, `fare_dodges`, `refused_fare`, `refused_danger`, `refused_closed`, `peak_stop_load`, `peak_stop` (index), `operator_fares`, `operator_power`, `commute_tpt_ride` (beside `commute_tpt_walk`/`_drive`: `World::commute_acc` grows two slots), `attend_rider`, `attend_walker` (Full: on-shift minutes at work over shift minutes, by how the agent came). `tools/analyze_run.py` gets a Transit section.
- UI (`citysim-app`): the track drawn over the roads, stops as markers, riders aboard drawn along the track between `depart` and `arrive_tick` (the flyer's lerp, along the polyline); the inspector shows "riding the Vat Line to X, arrives 07:42".

---

## 5. Costs

- **Ticks/s.** Per Full or Coarse GoTo: two scans over ≤ 64 stops and ≤ 9 table lookups (a few hundred ns). Per Statistical worker: one cached route a workday. Riders aboard take no path steps: a Full rider walks ~40 % of the tiles he walked before, so the exec does less. Expected neutral to slightly positive; the floor (4,000 on seed 42, `core_throughput`) stands and the plan A/Bs it.
- **Memory.** < 100 KB.
- **Save.** One version bump; the rides map is at most the number of agents aboard (~100s at peak).
- **Gates.** The behaviour tier re-measures (commute legs change every Full worker's day); shadow rows move by the doctrine (bounds re-fit from main plus the butterflies). The stat table regenerates once (with T14).

---

## 6. Hooks into work in flight

- **Relocation + the hiring commute cap** (parallel branch, 2026-10-10): every distance it reads should be `transit::commute_ticks(world, who, from_door, to_door)` (the route's ticks, or the walk's), not Manhattan tiles, so a Sump resident near a stop is hireable at the Vats and a mover prefers a Home near a stop. If that branch merges first, phase 2 swaps its distance; if this merges first, it calls the function.
- **Hunger as sickness P5f** (`p_attend`): T14 extends its key. If P5f is not merged when phase 3 starts, phase 3 waits for it (Statistical attendance is the parity term).
- **The flip** (P9a re-run, then P10): transit changes attendance and so wages; land it before the recalibration so one `calibrate` covers both (open question 7).
- **Ambient LOD** (addendum 13): `load[stop][hour]` is the platform density a promotion reads.

---

## 7. Acceptance (TESTING.md tiers)

**Unit** (`tests/transit.rs`, a hand-built line on a small map): the timetable (`next_departure` at, before and after a departure; both directions); `route` picks the minimum and refuses under `min_ride_tiles`; T8's five refusals each by a built case; the fare moves from wallet to operator and the coin identity holds; a dodger rides free and is counted; a closed stop is skipped and a cut line falls back to the other line or the walk; a mid-ride save loads and runs on byte for byte; a demotion mid-ride ends it; the operator's bankruptcy hands the concession to the city.

**Core** (`core_sanity`, seeds 42-44 × 120 days): mechanism bullets "a ride" and "a fare paid to the operator" (each on 3/3); sanity: no agent in `ExecState::Ride` past `arrive_tick + 1`, `World::rides` keys == agents in `Ride` or on a ride's walk legs, the coin identity (unchanged). `core_year` unchanged. The ticks floor unchanged.

**Behaviour.** The shadow diary shows the legs ("walked to Sump Central stop, 14 min; the Vat Line to Vats North, 31 min; walked 6 min"). New printed metrics per archetype: rides a week, commute minutes a workday, attendance. Rows re-measured. A critic read of the farmer and night-porter diaries after phase 2.

**Person probes** (`tests/person.rs`): a Sump farmer with coins rides and arrives before his walk would; a broke lawful worker walks; a broke lawless one jumps the gate; a timid worker walks rather than board at night in the worst district; a worker whose line is cut walks the next day.

**God** (`god_line_cut`, `docs/GOD_SCENARIOS_V*.md`): cut the Vat Line for 10 days at day 20 of a 60-day run; asserts: no ride on it during the cut, Vat Line commuters' commute ticks up, the operator's fares down; recovered after (rides resume). The riot close is unit-tested (riots are rare per seed).

**Printed findings, never asserted**: the ride share of commutes, commute minutes p50/p90 walk vs ride, attendance by role and band before vs after, `p_attend` by band, fares and the operator's treasury, peak stop loads, dodges, refusals by cause, Murders and thefts at stops' districts.

---

## 8. Not in v1

Train entities, capacity and platform queues; buses and road transit; sabotage, power cuts and rail damage (hooks only, T18); gang tolls on lines; ticket inspectors and a FareEvasion crime; crime aboard; station buildings with Security profiles and cameras (the Interiors milestone); the corp brain setting fares or building lines; passes, zone fares, concessions for children or the camps; errand rides off-screen; park-and-ride; a Spire express; rail to the outside (M17).

---

## 9. Map port

Dylan is building a much bigger city in a separate layout repo: a Hong Kong-like island, 6 × 3.75 km on 2 m tiles (~3,000 × 1,875 tiles), about 14.5k lots, with **an elevated rail layer of its own**. The three hand-placed lines of § 2 (waypoints on the 256 × 192 map's `7 + 8k` road lines) are scaffolding for this map only. When the city moves to the new layout, **lines and stops come from the map data**, not from `[[transit.lines]]` waypoints.

What keeps that swap cheap (phase 1 already holds to it):

- **One entry point.** `transit::build` turns *some* description of lines into the derived `Net` (stops, per-line stop order and ride offsets, the all-pairs table). Everything downstream (`route`, `choose`, the timetable, the legs, closures, columns, saves) reads only `Net` and stop indices, never waypoints or the config's line rows. The port replaces the front half of `build` (waypoints → track tiles → stops) with a reader of the layout's rail layer (track polylines and station positions as data); the back half (offsets, the table) stays.
- **Stops are data, not geometry rules.** Spacing every `stop_spacing` tiles, snapping to the nearest Road tile and "a shared tile is a transfer" are the hand-placed fallback. A map that names its stations (and their street access points) gives stops directly; `stop_spacing` and the snap then go unused.
- **Nothing saved depends on the layout.** The network is derived on creation and on load; saves hold only closures keyed by line and stop index, the day's load, and rides in progress. A map change invalidates those indices as it invalidates every saved position: a new map is a new world.
- **Scale.** The table is all-pairs over stops (64 stops: 8 KB). On the island, with a few hundred stations, it's still small (300² × 8 bytes ≈ 0.7 MB) and built once; past ~1,000 stations, switch to per-line tables plus a transfer graph searched per query. `candidates` scans all stops linearly; at that size it wants a spatial index (a coarse grid of stop lists). Neither changes the interfaces.
- **Ride time in metres.** `ride_ticks_per_tile`, `stop_spacing` and `min_ride_tiles` are per tile; on 2 m tiles they must be rescaled (or restated in metres and converted at load), as every distance knob in the config must.

---

## Open questions for Dylan

1. **Who owns the El?** Proposed: a corp concession (Arasaka: its guards become the inspectors, its cameras the station surveillance, its fares a raid prize). Alternatives: the city (fares fund police and sweepers through P5a's ring; the tax band falls), or a new transit corp (a tenth corp, a Niche the corp brain must learn: more work).
2. **The fare.** A flat 1 coin a ride is 2 a workday, a fifth to a third of a low wage on the flipped city. Brutal and real, and the time-vs-fare rule makes the poorest walk short trips. Keep it, or price it lower (a weekly pass; a free Sump line)?
3. **The El, not buses?** Grade-separated rail over the roads is the cheapest model and fits the look. Buses would share the roads with M13's cars (crashes, wreckage, impassable roads) and cost vehicle entities.
4. **The network.** Three lines serving the Sump and Mid, none in the Spire (the rich drive or fly). Two lines get most of the median gain (159 vs 149 ticks); the third halves the Sump's access walk (19 → 11 tiles p50). A fourth (another N-S spine) added nothing in the estimate.
5. **No service 01:00-05:00.** Night porters and stockers walk. Story, or a night service?
6. **Fare dodging with nobody checking.** In v1 a broke lawless rider rides free and nothing happens to him. OK until inspectors land, or file a Crime from day one?
7. **Sequencing.** Transit moves attendance and so wages and the money circuit: land it before the recalibration and the flip (one `calibrate`), or after the flip (a second recalibration)? Proposed: after Hunger as sickness (it needs P5f's `p_attend`), before the recalibration.
8. **Map.** v1 stops are street tiles: no map change. Station buildings (with a door, a Security profile, shops on the concourse) need map cells; take them from the free ground now, or wait for the Interiors milestone?

## Decisions (Dylan, 2026-10-10)

1. Sequencing: the flip goes first (hunger + relocation + recalibrate + flip), and transit lands after it with its own recalibration. Phases 1-2 may be built in parallel, merged off.
2. Owner: **the city**. Fares go to the Treasury, and power is paid from it. There is no concession and no corp revenue.
3. Fare: a flat 1 coin a ride.
4. Fare dodging is a petty crime record from day one, fed to the existing police and witness machinery. Inspectors come later.
5. (Orchestrator defaults) Elevated rail, not buses; three lines; no service 01:00-05:00, so night workers walk; stations are stops with no buildings until the Interiors milestone.

## Status (2026-10-11): phases 1-3 built, on by default

Phase 1 was ported onto main `ef83a2a` (the flipped city). Phases 2 and 3 followed on the same branch. **`[transit] enabled = true`** ships. `CITYSIM_TRANSIT=0` turns it off for A/B runs, and `Config::dole_city()` turns it off (the old default had no El). The measurement harness is `tests/transit.rs::measure_transit_tail` (ignored; `TRANSIT_SEEDS`, `TRANSIT_DAYS`, `TRANSIT_POOR_TPC`, `TRANSIT_POWER`).

**Seeds 42-47 x 120 days, shipped ON vs main OFF** (printed findings, never asserted):

| | main OFF | ON |
|---|---|---|
| far workers at day 120 (the agent's own trip > `commute_move_ticks` 80) | 245 | 197 |
| farmers' trip p90 (ticks) | 124-156 | 93-104 |
| all commutes p90 (ticks) | 192-206 | ~140 |
| employed at day 120 (mean) | 1,477 | 1,581 |
| Murders (sum) | 554 | 506 |
| private purses at day 120 (mean) | 44.4k | 42.0k |
| coins in the city (mean) | 254k | 241k |
| seeded corps alive | 51 | 53 |
| starvation | 0-1 | 0 |

- Ride share of commutes: about a third for bodies, 0.27-0.31 off screen.
- The far count by walking tiles doubles (~500). Housing and hiring now read the commute as ridden, so the city stops building Homes by the Farms for people the El carries.
- About 90 workers the shared distance calls near still walk far by their own choice. Most are the poor (the time-vs-fare rule's dearer coin); some are broke and lawful. The poor rule is kept: it is the brutal city working.

**Deviations from the spec and the plan**, with the reason for each:

1. **Ownership and money (decision 2).** The city owns the El. Fares go to the Treasury, which pays the power and pays **maintenance crews** from the fares: `TransitState.staff`, Sanitation posts on the city payroll, sized as (the 14-day mean of fares − a day's power) / a sweeper's wage, paced down like the civic headcount, and hired by `reconcile_sanitation` on top of it (the `el_staff` column). Why: parked in the Treasury (police at their ceiling, tax at its floor) the fares left purses 17 % lower. Station clerks and platform guards wait for station buildings (decision 5, Interiors).
2. **`power_per_line_day` 150 → 20** (E13's scale: a Market's power is 20 a day). Coins paid to the World displace export minting one for one. The placeholder sent 450 a day out of the city, about 45k over 120 days: purses −25 %, employed −5 %.
3. **The shared distance** (§ 6). `life::commute_cost` = `transit::commute_ticks`: the walk, or the ride over each door's nearest stops (untimed walks, half a headway's wait, the table) when it saves `min_saving_ticks` plus a fare's `ticks_per_coin`. No agent and no clock, so it is structural. The hiring band, relocation, the housing demand and the pre-roll read it, and so does the hire key's distance when the El is on. `Net::near` holds each tile's 8 nearest stops.
4. **The departure gate** (T16). It reads `transit::estimate_by`: the trip that arrives by the deadline, as `choose` would make it. It does not read "now". A now-based estimate fell when service began at 05:00 and turned riders back from commutes (tier commute aborts: sweeper 0.46, worker_friday 0.18). Bedtime (P5j) and leisure's shift lead read it too. `fixes::walk_estimate` is `transit::estimate` (now-based) for the other "how long to get there" readers.
5. **No per-agent route cache** (D8). An unsaved cache would make a loaded world choose differently. The choice is a few table lookups once per workday.
6. **Off-screen dodgers are caught** at `StatTable.p_dodge_caught`, learned by `calibrate` as Fare Evasion sentences per jumped gate (0.38). A caught dodger is reported and comes on screen for the arrest, as an off-screen theft does. Why: in the all-Full city Fare Evasion was 70 % of sentences, and parity's arrests went red. `calibrate`'s `p_theft_caught` no longer counts a thief sentenced at the gate. Without that fix the first table read 1.0, against 0.82 without the El.
7. **An arrest deters** (`dodge_deterred_days` 7). A rider arrested in the last week pays when it can, whatever its lawfulness. Why: a d90 gang leader with a full purse jumped the gate every morning, was arrested by 11:00 and spent ten hours a day in the cells, four days running (the tier's d90 gang leader `e0_h/wk` 4.03 against ≤ 3.1).
8. **Attendance by band** (T14, D9). `p_attend` gains `"{role}/b{n}"` cells (bands < 30, 30-60, 60-120, 120-180, ≥ 180 ticks of `commute_cost`), kept only past 54,000 shift minutes. The role's row is the fallback. Finding: in the all-Full calibration city the bands barely separate, because far workers leave earlier. Full farmer attendance rose from 0.83 to 0.91 with the El.
9. **The stale ride sweep.** `transit::run` drops, every tick, a ride whose plan step was replaced without an abort. `core_sanity`'s new check caught one on seed 43.
10. Unchanged from phase 1:
    - the aboard body keeps the board stop's tile;
    - a line cut mid-ride doesn't stop the ride;
    - `SAVE_VERSION` is not bumped: every new field is `#[serde(default)]` and skipped while empty or off, so an off world's save is byte-identical. With the El on, the save gains `transit`, `rides` and the config row; a pre-transit save loads with no rides.

**Gates.**
- Trio green: 920 passed.
- `core_sanity`:
  - two bullets with the El on: an El ride [81,882 / 84,121 / 87,740] and a fare paid [120 / 120 / 120] days;
  - the ride sanity (no stale ride, nobody aboard past arrival).
- `core_year` ok: seed 42, Murders 338, starvation 0, corps 8/9.
- Behaviour tier: 135/135 judged rows pass, no bound moved.
- God: 23 + 3 pass, including the new `god_line_cut` (v10).
- Parity (nightly, `lod.rs`): law1 actor share now passes (0.170 vs 0.167; red on main); bin meals stays red (1.43 vs 2.68, as on main); two new reds: arrests 1.25 vs 0.51 (main 0.58 vs 0.43: the Full city's Fare Evasion arrests come from errand rides too, and the Statistical tier models commute rides only, plan risk 1) and commute rides 28.5 vs 24.1 (tolerance 4.3; ride share 34.5 vs 31.0 passes). Next: learn an off-screen errand-ride rate, or `p_dodge_caught` per commute dodge.
- Five person probes (`probe_el_*`).
