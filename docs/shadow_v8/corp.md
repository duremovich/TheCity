# Shadow V8: corp (ceo, exec, office_worker, guard, reporter)

Critic: corp. `W` = `D:/TheCity/target/shadow_v8`. Windows: A `isle_s42_d0`, B `isle_s43_d30`, C `isle_s44_d90`, G `gen20k_s43_d30`, K `k2_s42_d26`.
Cites are `window/person dNN HH:MM`. Read: `ceo_21278` d30-d36 in full; `ceo_17908`, `exec_6400`, `exec_22432`, `exec_19673`, `guard_8630`,
`reporter_14524` by slices; per-day numbers from each window's `camera.tsv`; leg stats from `W/isle20k/v8_43.legs.tsv` (the leg log of the same
seed, 20k island, a different run from the pinned diaries, so I never mix a diary line into a tsv row).

**Verdict: the CEO's "wrong-district ends" are a misread. `(ended)` legs are cut by the plan, not mis-routed. The real bug is that a Spire home and a
Spire Hq are 150-350 tiles apart inside one "district", so a Spire-to-Spire commute is 4-14 h on the El or on foot and the plan timeout cuts it. The same
geometry explains the 0.5 h execs and the corp guards. Fixed since V7: the exec ride is drawn, Hq tables are staffed, ceo_17908 arrives.**

## 1. The top item: ceo_21278's legs

**The `-> X` in `ACTION GoTo ... -> X, N min (ended)` is where the leg was cut, not where it was headed.** `(ended)` = the plan was interrupted; `(done)` = arrived.
The 668-min leg at `B/ceo_21278 d30 06:00` is cut by `d30 17:08 plan interrupted: Work -> Eat` (06:00 + 668 min = 17:08), hence the Neon Quarter street. The same for
`d31 11:08` (cut at `17:08 Eat`) and `d32 05:53` (675 min). The target in all of them is `Corp HQ#4689 (D5 The Spire)`. So the legs do not end in the wrong
district: the body is still travelling when hunger interrupts it. SHADOW_V8's "16 legs of 300 min+ ending in the wrong district" counts cut legs (and
`diary_rows.md:49` reads them the same way). Phase 4's fix works as built, but it spares a leg only at a segment end; these are El rides and walks.

The trip, minute by minute (home Block#338 and Hq#4689, both labelled D5):
- `d30 06:01` leaves the flat, `06:18` already in D4 Mid-Levels. No fare, no vehicle: on foot. `06:18-12:50` 6.5 h with no `[move]` line, `12:50` the Sump, `12:52 Fare -1c` (El boarded),
  `14:19` D4, `14:38 D2, 14:40 D4, 14:56 D2, 15:04 D4` (a line along a border), `16:07` D3, `17:08` cut. His kit lists `vehicle Flyer T1`; it is not used.
- The one that arrives, `d35 02:53 GoTo ... 689 min (done)` from a D2 street: `05:24 Fare`, D3 `05:34`, Sump `07:37`, Vats `09:16`, D4 `11:41`, D5 `12:07`, Hq `14:22`. 2.5 h to the stop, 6.5 h round a loop line, 2.25 h on foot inside the Spire.
- `d36 11:12` from the Vats: `Fare 13:05`, D5 `13:37`, **2 h 15 min on foot inside D5**, Hq `15:52`.

**Why (the geometry, from the leg log).** Destination tiles labelled D5 span x 200-1252, y 250-658. Same-district D5-to-D5 legs by ceo/exec run 150-350 tiles
(`walk 859,521 -> 563,463`, 354 tiles, 716 min `done`; the El on the same pair 435 min). The El covers about 1 tile/min, the walk 2. Of exec/ceo legs, 107 of 149 El and 64 of 74
`exec_ride` legs end `aborted`, most at the 359-360 min timeout.

**Code sites** (read-only, not run):
- `systems/streetwork.rs` `exec_ride`: returns `None` when `district_of_building(home) == district_of_building(hq)` (the "same district" gate). For a Spire home and a Spire Hq
  300 tiles apart the exec gets no car. This is the main cause for ceo_21278 and exec_6400.
- `exec/mod.rs` `start_step`: after the exec ride, the El (`transit::try_ride`) then the walk segments, for a body with no `begin_trip` vehicle; the Flyer is not chosen on this path.
- `transit::choose`: accepts a 2.5 h access walk and a loop route longer than the walk itself (no cap vs the walk).
- Placement: nothing keeps an exec's home near his Hq inside a district (`life::hq_of`, relocation to `exec_home_tier`).

**Fix ideas (bugs only):** gate `exec_ride` on tile distance (the `leg_replan_tiles` scale), not the district label; refuse an El route longer than the walk or with an access walk
above N min; or place the exec's home next to his Hq. Treat the Spire's district label as non-spatial in any "same district" test.

## 2. Per archetype

**CEO.** Fixed: `A/ceo_17908` commutes 195 min Spire-to-Spire and arrives (`d2 05:28`, `d3 11:38`, `d4 06:21`, `d5 05:26`, all `(done)`), meets 7-8 h on d2, d4, d5
(`d4 DAY SUMMARY meeting 7.4h`). V7's 901-min wrong-way ride is gone; the midnight "moved nearer work" lines read 0 in the week. Still broken: `A/ceo_17908 d0 00:00 GoTo(Seller)
974 min (done)` and `d0 17:28 GoTo(Home) 971 min (done)` (travel 22.8 h on d0), and street 8.6 h a day all week. Alive: `B/ceo_7362` (398 table lines, work 6.9 h, 38 settlements a day) and
`C/ceo_6504` (395, work 7.1 h): V7's empty Hq table is fixed for CEOs. `B/ceo_21278` is not at work: work 0.6 h a day, street 12.1 h, 2 settlements in the week, two Meetings (`d35 14:22` 158 min, `d36 15:52` 68 min).
His other life is intact (`d34` home 12.8 h, a bar, a hotel), so the failure is the commute, not the brain.

**Exec.** Fixed: exec_ride is a drawn vehicle ride, not a statue (`B/exec_19673` vehicle 6.3 h; the 228-min Spire-to-Vats leg every day is `(done)`, then a Club leg of 53-55 min), no body jumps.
Hq table staffed for `exec_21630` (259 lines) and `exec_19673` (50); `A/exec_19004` has 0 table lines in a week of 7.0 h work (V7 bug 5, unchanged). Still broken:
- `C/exec_6400` (Vasquez Holdings, Spire home, Spire Hq): `d91 01:47 GoTo(Seller) 823 min (done)`, `d91 DAY SUMMARY travel 20.6h, meeting 1.5h`; d93 travel 20.7 h, meeting 2.2 h; d95 meeting 2.3 h. The off days are home and crime (`d92 crime 0.9h, net +734c`).
  **This is the "0.5 h of work" row: a long commute, not an idle brain.**
- `C/exec_22432` (Stackwell 5, treasury 431c, order Hunker): home 24.0 h all 7 days, 113 plans, **0 `Work` lines in the whole diary**, idle 17-20 h a day, 0 settlements. No `Quit` or reason line anywhere on the page.

**Office worker.** `B/office_worker_12419`, `23739`: work 7.7-7.8 h, street 2.3-2.7 h a day; `G/25531` 8.1 h, `K/1141` 7.2 h. Solid shifts, a better street than V7. Open: `C/office_worker_16207` work 3.8 h, street 9.9 h (not read in depth).

**Guard.** `GuardJail` is still one long action: `B/guard_15370 d31 21:02 ACTION GuardJail @ Precinct#4460, 538 min (ended)`, `d32 21:02` (6 `GuardJail` lines in B and in `A/guard_22478`): unchanged from V7.
`B/guard_8630` (home D4, posted in the Vats): work 0.3 h a day, travel 17.0 h on d30 (`02:12 Escort 364 min (ended)`, `10:42 GoTo(Home) 274 min`, `21:02 GoTo(PatrolWaypoint) 78 min`); `C/guard_13955` work 0.5 h, `K/guard_2232` 0.3 h.
It is V7's `guard_8523` unchanged. An escort is a 364-min stand with no jump, still not the van.

**Reporter.** Shift alive (`B/22655`, `25681`: work 7.6-7.7 h, street 2.7 h). The beat is thin: an interview in `B/reporter_22655 d35 10:09`, 5 in `G/20574`, none in A. A `Story` exists:
`C/reporter_14524 d92 00:00 Story: Kaneda Feed: Ignatius Oakes arrested by Cordelia Nakamura`, a news item that names the arrester, not the author (V7 0 bylines; V8 1, no author line). `G/reporter_17515`: work 0.0 h, street 14.9 h a day, the exec's shape.
`SETTLED APART` lines: 2 in `G/17515`, 1 in `G/20574` (V7 bug 11, unchanged).

## 3. Ranked bugs (a broken mechanism)

1. **Same-district commutes of 150-350 tiles on a slow El or on foot.** Evidence: section 1; `C/exec_6400 d91 01:47 823 min`; `A/ceo_17908 d0 974 min`; leg log 107 of 149 El and 64 of 74 `exec_ride` legs `aborted`.
   Site: the same-district gate in `streetwork::exec_ride`; the route choice in `transit::choose`. Fix: a tile-distance gate, an El-vs-walk comparison, or home-near-Hq placement. It also drives the 0.5 h execs, the 360-min timeouts and the corp guards' street.
2. **The report's `-> X (ended)` reads as a destination.** It is the cut point; V8's wrong-district table and `diary_rows.md:49` count timeouts as mis-routes. Fix: print `cut at X` for an `ended` leg, and recount with `(done)` legs only.
3. **An exec who never plans Work** (`C/exec_22432`, 7 days, 0 `Work` lines, 431c, Hunker) with no reason on the page. Site unread (the goal score or `life::hq_of` returning `None`); fix: log "no Hq" on the goal line.
4. **`GuardJail` 538-min single action** (`B/guard_15370 d31 21:02`, `d32 21:02`). Unchanged since V6; split it into blocks as the ClerkWork loop was.
5. **Corp guards travel the shift** (`B/guard_8630`, `C/guard_13955`): post and home in far districts, escort as a timed walk of 364 min. Same geometry as 1; `escort_van_ticks` should also bound the walk.
6. **Reporter beat**: 3 `SETTLED APART` interviews in G, and the byline names the arrester.
7. **`A/exec_19004`: an Hq with 0 table lines in a 7 h/day week.**

## 4. FINDINGs (calibration, not knobs)

- A 4-14 h commute is a plausible price of a sprawling Spire, at a strange rate: the island's Spire execs spend 40-90 % of the day travelling (`B/exec_19673` veh 6.3 h; `C/exec_6400` travel 20.7 h on work days).
- Exec work hours by window: B 5.8-6.9 h, C 0.0-7.1, G 7.1-8.2, K 7.3.
- Hq tables: `ceo_7362` 398, `ceo_6504` 395, `exec_21630` 259 lines; the V7 empty table is fixed for 5 of 6 Hq holders.
- Reporter work hours: 7.6 h (B), 0.3-6.2 (C), 0.0 (`G/17515`).
- `B/ceo_21278 d35 02:52` was fined 8c mid-commute: a street-cut trip exposes a CEO to crime. A story if it is meant.
