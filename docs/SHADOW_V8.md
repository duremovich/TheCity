# Shadow pass V8: the island pass gate (part 1, the runs and the device rows)

Read on 2026-10-23: main `468c635` (the island pass, phases 0-10) with the V7 tool plus two V8 lens fixes
(`bda32d3`, `272a213`, branch `worktree-agent-a0ea3cdf9af21a78a`). The spec is `docs/ISLAND_PASS.md`; the gate is
"Shadow V8 on the island (`isle_m_k2`, 20k, seeds 42-44) against `target/shadow_v7/`, V7's tool, windows and critics".
Part 2 (seven critics and a synthesis) follows from the critic brief at the end.

V7's diaries are kept in main's `D:/TheCity/target/shadow_v7/`, V8's beside them in `D:/TheCity/target/shadow_v8/`
(377 MB). Every V7 figure below is re-read from V7's tree with the same script that reads V8's, unless it says "V7 quoted".

## Setup

- **Windows**: V7's six, same arguments, same 48 archetypes (`gang_leader:4,sweeper:4`), same seeds and days:
  `isle_s42_d0`, `isle_s43_d30`, `isle_s44_d90` (island, `--population 20000 --map assets/maps/isle_m_k2.txt`),
  `gen20k_s43_d30` (the generated 20k city, the control), `k2_s42_d26` (2k), `isle_s43_d30_nopin` (12 picks, 3 days,
  unpinned). Plus the five `--list` runs, `shadow --assert`, and `--lives --population 20000` on the island and on gen.
- **Counters**: island 20k, seeds 42-44 x 30 (`run --report --events --legs`), V8 against V7's own runs
  (`target/shadow_v7/isle20k/cand_*`), and V8 with the ten island-pass switches off (`off`, the leg device's base).
  Seed 44 x 91 on V8 and on V7's binary (from `base-92c4a37`'s own root) for the d90 hunger columns.
  2k, 42-47 x 120, against V7's `measure2k`. Alternating A/B ticks/s, island seed 42 x 5 d.
- **Picks**: the people differ. Picks are chosen at the start day from a city the pass changed, so 24 of 409 picks
  are the same person in both (the corp critic's `ceo_17908` and `ceo_21278`, the retiree `worker_6720`; the list is
  `overlap.py`'s, § Critic brief). V8 compares archetypes one for one, not people.
- **Scripts** (in the run tree): `diary_rows.md` (the diary measures, both trees), `counters.md` (the counters and
  the leg device), `walls.txt`, `ab.txt`. The run and analysis scripts are in the runs report's scratchpad
  (`lane_a4.sh`, `lane_b.sh`, `v8_diaries.py`, `v8_counters.py`, `ab_v8.py`).

**Tool changes** (`citysim-cli/src/shadow.rs` only; read-only, no RNG). The island pass put riders aboard vehicles
in motion (I3), and V7's lens still used the old timed speeds. It read an El train's 7 tiles a minute as a jump
(444 body jumps in a week on `isle_s42_d0`), and a cab or car rider with an `Idle` exec as a street walk.
- `bda32d3`: a pick aboard a vehicle (`motion::vehicle_of`) is in a vehicle (the `veh` column), and the vehicle's own
  move each minute is jump allowance.
- `272a213`: the lens reads `Motion::pos`, not `motion::pos_of`, which runs a tick ahead of the riders. At the
  boarding minute the vehicle's own step is allowance too. What is left: 103 body jumps on the island in three weeks,
  every one of 8 tiles or fewer, at a car's kerb. V7 had 10 body jumps of 300 tiles or more.
- CLI unit tests 26 of 26 pass; fmt and clippy are clean.
- Every diary in `shadow_v8/` was written by `272a213`. An earlier pass with the first fix only was overwritten.

## The verdict in five lines

1. **Distance worked.** Island labour commute legs of 4 h or more fell from 20 % to 4 %, the median from 42 to
   31 min, and structural commutes of 240 min or more from 29.6 % to 8.7 %.
2. **Vehicles worked.** The body is no longer parked on a boarding tile: pick-days parked 180 min or more fell from
   241 to 0 on the island. Leg-device body-days parked or jumped fell from 103,461 to 9 over 3 seeds x 30 days, the
   Coarse walk to the El excepted.
3. **The dead trades' bugs are fixed** for the courier, the day labourer and foreman, the arms dealer and the cab, at
   low volume. The bouncer, the joytoy and the scav's side are still dead, as planned (`OPEN-176`).
4. **New or remaining:**
   - Murders are 3 x V7's on the island (16 → 45). That rise is not the pass's: V7's close main was at 156 on
     seed 42, and phase 10 brought it down. At 2k, Murders fell 79 → 32.
   - Idle grew by about 1.5 h a day on both 20k cities.
   - Island homelessness is up 55 %.
   - A Spire CEO still rides 668-1,020 min and ends in the wrong district.
5. **Hunger** at d90: the shadow `sick` pick fell 2,188 → 934, but 6,185 islanders are still malnourished (V7's
   binary: 7,015). `shadow --assert`: 114 asserted rows pass, 0 fail; 18 print as FINDING.

## The device rows, V7 → V8 (island 20k unless named)

### Commutes

| measure | V7 | V8 |
| --- | --- | --- |
| labour `GoTo(Workplace)` legs of 4 h or more, island pooled (diaries) | 20.3 % (111 of 548) | **4.4 %** (29 of 657) |
| the same, d30 / d90 windows (V7 quoted "24-25 %") | 25.3 % / 24.1 % | 4.2 % / 0.9 % |
| labour `GoTo(Workplace)` legs over 2 h | 26.5 % | 13.5 % |
| labour `GoTo(Workplace)` median, min | 42 | 31 |
| labour `GoTo(Home)` 4 h or more / median | 24.0 % / 81 min | 8.8 % / 39 min |
| structural home-to-work commutes of 240 min or more (`#commute`, all employed and housed, d30, 3 seeds; V7 column = V8 `off`) | 29.6 % | **8.7 %** |
| labour pick-days mostly travel: V7's measure (longest awake still is `travel`) | 126 of 378 | **3 of 406** |
| labour pick-days whose GoTo minutes exceed their work minutes | 223 of 378 | 158 of 406 |
| labour pick-days with no work at all | 81 of 378 | 49 of 406 |
| gen labour legs 4 h+ / 2k (controls) | 0.5 % / 0.0 % | 1.0 % / 2.1 % |
| `--lives` travel h/week, island / gen | 50.6 / 40.2 | 41.6 / 33.0 |

### Parked or jumped bodies

| measure | V7 | V8 |
| --- | --- | --- |
| pick-days parked on a travel stretch of 180 min or more, the three island windows | 241 of 1,709 | **0** of 1,755 |
| `[camera] JUMP` body jumps, island windows | 10 (all 300+ tiles; 9 one exec's `exec_ride`) | 103 (all ≤ 8 tiles: a car's kerb) |
| leg device (`#ride`), body-days parked or jumped, 3 seeds x 30 d, `off` → V8: El (Full / Coarse) | 39,802 / 41,934 | 0 / 9 |
| car drive (Coarse), cab (Full + Coarse), exec ride | 1,698; 1,077; 468 | 0; 0; 0 |
| escort (Full + Coarse), arrest hand-off (Full + Coarse) | 7,610; 7,679 | 0; 0 |
| van (new mode), transfer, flyer | (none); 6; 64 | 0 of 19,254; 0; 0 |
| **the Coarse walk to and from the El (`el_walk`, Coarse)** | 41,939 of 50,575 | **38,277 of 46,498** (left by design, `OPEN-165`) |
| `el_walk`, Full | 3,121 | 0 |
| Coarse timed walks (`#walk`, OPEN-181): parked, promotion jumps | 80k / 250 a seed | 0 / 4-9 |

The `off` column is V8 with its ten switches off. It is not V7 itself: the Statistical table was regenerated at V7's
close (`45aef8c`) and again in phase 6. V7's binary predates the device.

### Wrong-district segment ends (diaries; a GoTo that ended short of its target, in another district)

| window | V7: all / 300 min+ | V8: all / 300 min+ |
| --- | --- | --- |
| isle_s42_d0 | 5 / 3 | 9 / 1 |
| isle_s43_d30 | 33 / 8 | 45 / **16** |
| isle_s44_d90 | 61 / 22 | 36 / 6 |
| gen, 2k | 0 | 0 |

**Not fixed for the one CEO both reads followed.** `isle_s43_d30/ceo_21278` lives in a Spire Block, works at a Spire
Hq, and still runs `GoTo(Seller)` 668 min to a Neon Quarter street (`d30 06:00`), 360 min to Mid-Levels, 675 min
from the Sump, then `GoTo(Home)` 1,020 min. That makes 6 legs of 300 min or more in a week, the same shape as V7's
901 min. Phase 4 closed `OPEN-164` on the plan timeout; this is another path, and it is the corp critic's to read.

### Hunger (I5, `OPEN-167`)

| measure | V7 | V8 |
| --- | --- | --- |
| shadow `sick` candidates, seed 44 d90 (V7's measure) | 2,188 | **934** |
| `malnourished` column, seed 44 d90 (`run`, same seed and day; V7 = V7's binary) | 7,015 | 6,185 |
| starving at d90 / starvation deaths to d90 | 37 / 16 | 21 / 1 |
| `malnourished` at d30, 3 seeds mean | 579 | 344 |

The sick pick fell by 57 %. Malnutrition at 0.3 or more climbs from d30 to d90 in both binaries (`OPEN-167`: it predates
the pass and is untraced). The care critic re-reads the carers at d90.

### The dead trades (I4)

| trade | V7 | V8 |
| --- | --- | --- |
| courier parcels handed over on screen, island diaries | 0 | **83** (d0 28, d30 55, d90 0) |
| `parcels_delivered`, 30 d mean | 7,072 | 10,688 |
| builder `Unload` off the post, island diaries | 11 of 225 | 29 of 182; `site_days` 43 a seed (new) |
| foreman `Hire` lines, island diaries | 0 | 3 |
| day posts filled / `Hire as Day Labourer`, 30 d mean | 0 of 963 | **185** of 1,000 |
| day-labourer candidates, d30 / d90 | 0 / 0 | 41 / 2 |
| arms dealer candidates, d30 / d90 | 0 / 0 | 1 / 6 |
| `sold_dealer` (arms sold by the dealer), 30 d mean | 0 | 8.3 (2k: 0 → 28 / 120 d) |
| body-driven cab rides (`cab_rides_driven`) / `fares_driven` | 0.3 / 0 | **15 / 2**; `fares_no_body` 2,824 → 197 |
| cabbie fares on screen, island diaries | 0 | 8 |
| the bouncer's refusals / the joytoy's clients / scav harvests, island diaries | 0 / 0 / 0 | 0 / 0 / 0 (`OPEN-176`, the pressure pass) |
| Court / OrderGiven, island leader diaries | 29 / 6 | 23 / 2 |

### The one-block shifts and presence leaks (V7 bugs 6-7, `OPEN-169`/`OPEN-170`, re-read and not fixed)

| archetype, pooled pinned windows | median longest work action, min: V7 → V8 | work-days over 180 min |
| --- | --- | --- |
| fabber | 537 → 537 | 51 → 49 of 52 |
| super | 373 → **417** | 48 → 50 |
| sweeper | 271 → 196 | 51 → 43 |
| cook | 217 → 225 | 41 → 45 |
| **builder** | 90 → **296** | 4 → 32 (new: the site day, phase 8) |
| foreman | 89 → 91 | **0 → 19** |
| bouncer / madam | 186 / 184 → 184 / 176 | 32 / 28 → 29 / 23 |
| all archetypes | 89 → 89 | 480 → 525 of 1,864 |

Sweeper stands 186 → 201 a seed, rounds 325 → 504.

| presence | V7 | V8 |
| --- | --- | --- |
| settlements with the body apart, island windows | 0.4-1.5 % | 0.4-1.5 % |
| settlements with the pick asleep, island d0 / d30 / d90 | 3.0 / 0.8 / 1.1 % | 0.1 / 1.2 / **3.3 %** |
| gen, pick asleep | 11.1 % | 0.7 % |
| SETTLED APART lines, island windows | 88 | 80 |
| `presence_missed`, 30 d mean | 65,340 | 49,288 |

### Company housing (I6, new)

| measure (30 d, 3 seeds mean) | V7 | V8 |
| --- | --- | --- |
| company beds (last day) / inside the workplace | 0 | 6,148 / 5,221 |
| company-housed (last day) / moved in | 0 | 4,108 / 839 |
| buyouts / tenants evicted by a buyout | 0 | 20 / 323 |
| evicted after losing the job / notices | 0 | 489 / 364 |
| **homeless (last day)** | 193 | **300** (+55 %; `off` 234) |

### Murders and violence

| measure (island 30 d, 3 seeds mean) | V7 | V8 |
| --- | --- | --- |
| Murder events | 16 | **45** (`off` 44) |
| deaths by violence / off screen | 50 / 13 | 83 / 38 |
| `fv_killed` (the Statistical mirror's kills) | 2 | 0 |
| kill rate per 1,000 agent-days, body / Statistical | 0.61 / 0.025 | 0.75 / 0.071 |
| seed 44 to d90, Murder events (V7 binary / V8) | 149 | 164 |
| **2k, 42-47 x 120**: Murders / violent deaths | 79 / 122 | **32** / 74 |

**The island's rise is not the pass's.** Seed 42 x 30 on the cached bases gives:
- V7's binary (`92c4a37`): 15.
- V7's close (`a05a9de`, after the parity fix regenerated the Statistical table): 156.
- Phase 0 (`fa74b64`): 156.
- V8: 49.

`OPEN-185`'s amplifier arrived with that table, and phase 10 cut it to 49. The off-screen deaths that are left come
from the victim side (`OPEN-186`).

The 2k fall (-60 %) is new to this read. The standing decision of 2026-10-17 says not to tune it; it is a FINDING
for the synthesis.

### Ticks/s (print only)

Alternating A/B, island 20k, seed 42 x 5 d, V7's binary against V8: **248 → 271** (248/250/247 against 261/276/276).
On a loaded box the 30-day counters read 123 → 161, and 2k reads 2,472 → 3,501.

### The rest of the devices

- **`shadow --assert`**: 114 rows asserted (collapse bounds), **0 failed**; 18 rows print as FINDING. One calibration
  MISS: d19 ripperdoc KnownMetWeek 1.27 (range 2.78-9.47). V7's 132 of 132 was the pre-restructure tier, where every
  row asserted.
- **`--lives`, the `all` row**:
  - island: n 214, idle **4.48 h/d** (V7 2.93), named/wk 1.68 (1.38), empty 0.18 (0.21), Served-able 0.23 (0.29),
    met 0.22 (0.22), travel 41.6 h/wk (50.6).
  - gen: idle **4.89** (3.46), named/wk 0.85 (1.24), met 0.31 (0.29), travel 33.0 (40.2).
  - Both cities sit at home about 2 h a day more (home 6.9-8.3 → 8.9-10.3 h a pick-day in the camera). That is
    probably I1's exception (no hangout spot past the cap, so the person stays in, the street-life feature being
    `OPEN-109`), plus shorter legs. For the street and vice critics.
- **The camera, island means per pick-day**: home 6.9-8.3 → 8.9-10.3 h, work 3.6-5.2 → 4.0-5.3, street 7.4-8.8 →
  5.9-6.7, vehicle 1.9-2.0 → 1.3-1.7, awake still 176-295 → 142-201 min, GoTo over 120 min 1.09-1.20 → 0.76-0.86 a
  pick-day. The full table is in `diary_rows.md`.
- **Arrests** -15 % (3,146 → 2,670). Escort walks 748 → 100 a seed, now that the van carries them. Gang members
  218 → 303. Wallets +1 %.

## Anything that looks like a new failure

1. **The Spire CEO's 668-1,020 min legs that end in another district** (`isle_s43_d30/ceo_21278`). There are 16 legs of
   300 min or more ending in the wrong district at d30, against V7's 8.
2. **Idle up by about 1.5 h a day on both 20k cities** (`--lives`), and home time up by 2 h.
3. **Island homeless +55 %**, beside 489 job-loss evictions and 323 buyout evictions a seed in 30 days.
4. **New one-block shifts**: the builder's site day (median 296 min), the foreman (19 days over 180 min), and the
   super up to 417.
5. **2k Murders 79 → 32**, and the island at 3 x V7's base (explained above, but the number the camera will see).
6. **Day 90**: the island couriers hand over 0 parcels (120 unloads at their own Yard), the execs work 0.5 h a day,
   and 3.3 % of settlements happen with the pick asleep.
7. **The Coarse walk to the El** still parks and jumps on 82 % of its body-days (known, `OPEN-165` "left").

No crash and no nondeterminism in any run: every window, list, counter, assert and lives run exited 0. The two
non-zero exits in `walls.txt` are a superseded first-tool lane, whose directory the rerun removed.

## The diaries

`D:/TheCity/target/shadow_v8/<window>/<arch>_<index>.md` (and `.jsonl`), one `camera.tsv` per window, the stdout in
`<window>.log`, the candidates in `<window>_list.log`.

| window | V8 picks (V7) | archetypes with no candidate |
| --- | --- | --- |
| isle_s42_d0 | 58 (58) | gang_member, gang_leader, runner, purist, dealer, sweeper, gun, buyer, joytoy, street_worker, pimp, vendor, office_worker, arms_dealer, companion, black_doc, cultist, scav (the day labourer now exists) |
| isle_s43_d30 | 96 (92) | black_doc |
| isle_s44_d90 | 98 (95) | pimp |
| gen20k_s43_d30 | 97 (96) | see the log |
| k2_s42_d26 | 50 (48) | see the log |
| isle_s43_d30_nopin | 12 (12) | - |

Wall times (s, the box shared): k2 16, isle d0 81, isle d30 335, isle d90 1,038, gen 313, nopin 267, assert 72, lives
279 and 269.

## Critic brief

Each critic reads its archetypes in all six windows in `D:/TheCity/target/shadow_v8/`, and the V7 diary of the same
archetype in `D:/TheCity/target/shadow_v7/` for the before. They also read `diary_rows.md`, `counters.md`, their V7
critique and this file. The rules are V7's: a missing line is never evidence, and a Statistical or Coarse stretch is
never quoted except for lines that exist. Cite as `window/file dDD HH:MM`.

Same person in both reads: isle_s42_d0 `ceo_17908`, `elder_22703`, `elder_25682`, `exec_6168`, `fixer_8866`,
`guard_22478`, `worker_13907`, `worker_6720`; isle_s43_d30 `buyer_15947`, `ceo_21278`, `fighter_8413`, `guard_15370`,
`street_worker_21709`; isle_s44_d90 `night_shift_6958`, `sweeper_6405`; gen `fixer_6893`, `madam_23940`,
`street_worker_24759`, `teacher_21323`; k2 `loader_1179`, `sick_1919`, `teacher_1351`.

### street (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden)

- **V7 said**:
  - A same-district GoTo of half a day, with the body standing on one tile for the ride.
  - The sweeper's stand a district from the post.
  - The arrest jump.
  - A vendor selling while asleep.
  - Day-0 aimless idle.
  - Hired homeless walking 8 h to the job.
  - The 2k warden counting nobody.
  - "Hotel" landing in a Soup Kitchen or a Church, 33 times.
- **The pass claims**:
  - Distance in the free-time picks and the hotel (phase 2); the cot is its own place (`OPEN-171`).
  - Rides and arrests in motion (phases 5-7).
  - Coarse walkers on their way (phase 9).
- **Not claimed**: the sweeper's stand (`OPEN-169`), the vendor asleep (`OPEN-170`) and the warden (`OPEN-172`).
- **Read first**:
  - `isle_s44_d90/sweeper_*` and `sweeper_6405` (the same person).
  - The `isle_s44_d90/sick_*`, which d90 hunger lands on.
  - The homeless picks' `GoTo(Cot)`.
  - `k2_s42_d26/camp_warden_*`.
  - The street time down 2 h and home time up: is the street emptier?

### gangs (gang_member, gang_leader, purist, cultist, scav, dealer, runner)

- **V7 said**:
  - A walk of 12-15 h to the Hideout bed.
  - A free HangOut across the district.
  - The Harvest order unplannable (ExpansionCap).
  - Court a statue.
  - The Hideout/Home ping-pong.
  - The 2,250c LedgerHack.
  - Deal a 4 h statue.
  - Arrest jumps.
- **The pass claims**:
  - Distance in the Hideout bed (phase 1); the ping-pong gone after phase 1.
  - The arms dealer from a gang Bar (phase 8).
  - Off-screen strikes only by off-screen hands (phase 10).
- **Read**:
  - The leaders' nights.
  - The gang members' legs to the Hideout.
  - The new `arms_dealer` picks, with the crime critic.
  - Purist and runner, which went from 115 and 73 to 383 and 145 min.
  - Gang members are 303, up from 218.

### crime (forger, fixer, gun, buyer, pimp, black_doc, arms_dealer)

- **V7 said**:
  - A prisoner moved at 04:00 with no walk.
  - A 14-h statue then a jump.
  - A hunter dead "of Violence" with no attacker.
  - The pimp's 61-min collections.
  - Post-holders quitting unpaid.
  - The black doc with no patient.
  - No arms dealer.
- **The pass claims**:
  - The van transfer and hand-off (phase 7).
  - The arms dealer appointed (phase 8; 1 candidate at d30, 6 at d90, `sold_dealer` 8 a seed).
  - Distance.
- **Read**:
  - The `arms_dealer_*` picks at d30, d90 and gen.
  - The transfers.
  - The pimp: the median longest action went from 61 to 509 min.
  - The murders: 45 on the island, 32 at 2k. Do the guns' hunts still read?
  - `fixer_8866` and `buyer_15947` are the same people as in V7.

### corp (ceo, exec, office_worker, guard, reporter)

- **V7 said**:
  - A GoTo that ends in the wrong district (`ceo_21278`, `ceo_17908`).
  - `exec_ride` a 282-313 min statue.
  - The escort and transfer jumps.
  - A CEO who is a gang leader and a Vat Tech.
  - Midnight "moved nearer work".
  - The empty Hq table.
  - `GuardJail` 538 min.
  - Guards walking the shift.
- **The pass claims**:
  - The plan timeout spares a leg (phase 4, `OPEN-164`).
  - The exec ride in motion (phase 5).
  - Commute pressure (phase 4).
- **Read first**:
  - `isle_s43_d30/ceo_21278`: 668-1,020 min legs ending in D3 and D4 for a Spire-to-Spire trip. Is `OPEN-164` back
    by another path?
  - `isle_s42_d0/ceo_17908`.
  - The d90 execs at 0.5 h of work and 17 h at home.

### labour (worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer)

- **V7 said**:
  - Island commutes of 4-15 h; 126 of 378 pick-days travel.
  - The body on the boarding tile.
  - Foremen hiring nobody.
  - Four roles still one block.
  - The haul going nowhere.
  - The 21:00 platform settle.
  - The retiree's `move_in`.
  - The cab driver.
- **The pass claims**:
  - Distance, company housing and commute pressure (phases 1, 3a-4).
  - Vehicles (phases 5-6).
  - Courier, builder, corner and cab (phase 8).
- **Read**:
  - The day labourers, new: 28 work-days.
  - The foremen's dawn picks.
  - The builder's site day, a new 296-min block.
  - The courier's parcels (28 and 55 handed at d0 and d30, 0 at d90).
  - The cabbie's 8 fares.
  - A company-housed worker's eviction or move.
  - `worker_6720` and `worker_13907`, the same people.
  - `fabber` is unchanged at 537.

### vice (club_staff, bouncer, dancer, madam, companion, joytoy, fighter, cook)

- **V7 said**:
  - Free-time legs of 300-900 min, then a late arrival at work.
  - The foreman taking people off their trade.
  - A hotel 380 min away.
  - The bouncer a statue at a door that never opens.
  - A dancer who never works.
  - Customers that never reach the island s44 counter.
- **The pass claims**: distance in the free-time spot and the hotel (phase 2), and a hangout past its cap means
  staying in (I1's exception).
- **Not claimed**: the bouncer and the joytoy (`OPEN-176`, still 0 and 0).
- **Read**:
  - Whether venues fill now that free time stays home: lives idle is up about 1.5 h, and Served-able fell 0.29 →
    0.23 on the island.
  - The cook: still 225 min, with 34 days on one tile.
  - `madam_23940` and `fighter_8413`, the same people.

### care (ripperdoc, orderly, teacher, super)

- **V7 said**:
  - A carer starving on the wage, untreated (d90).
  - The teacher's door posted apart.
  - A super with no round standing the whole shift.
  - A doctor at the owner's table hourly.
  - An 8-h night walk home.
  - The orderlies' 00:00 batch.
- **The pass claims**:
  - Distance and vehicles, which `I5` expected to lift the hunger: the sick pick is 934, down from 2,188.
  - No clinic path (deferred to this read).
- **Read**:
  - The d90 carers' malnutrition and shifts.
  - The super: up to 417 min, with 12 days on one tile.
  - The teachers (`teacher_21323` and `teacher_1351` are the same people).
  - The pick-asleep share at d90, now 3.3 %.
