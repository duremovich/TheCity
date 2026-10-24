# Shadow pass V8: the island pass gate (part 1, the runs and the device rows)

Read on 2026-10-23: main `468c635` (the island pass, phases 0-10) with the V7 tool plus two V8 lens fixes
(`bda32d3`, `272a213`, branch `worktree-agent-a0ea3cdf9af21a78a`). The spec is `docs/ISLAND_PASS.md`; the gate is
"Shadow V8 on the island (`isle_m_k2`, 20k, seeds 42-44) against `target/shadow_v7/`, V7's tool, windows and critics".
Part 2 (seven critics and a synthesis) follows from the critic brief at the end.

V7's diaries are kept in main's `D:/TheCity/target/shadow_v7/`, V8's beside them in `D:/TheCity/target/shadow_v8/`
(417 MB). Every V7 figure below is re-read from V7's tree with the same script that reads V8's, unless it says "V7 quoted".

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
  the leg device), `walls.txt`, `ab.txt`. The run and analysis scripts are in `shadow_v8/scripts/` (`lane_a4.sh`,
  `lane_b.sh`, `v8_diaries.py`, `v8_counters.py`, `ab_v8.py`, `overlap.py`, `murders_bisect.sh`).

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

---

# Part 2: synthesis

Seven Sonnet critics read their archetypes in all six windows against V7's diaries (`docs/shadow_v8/{street,gangs,crime,corp,labour,vice,care}.md`).
This part dedups them, checks the orchestrator's cross-lens patterns against what the critics wrote, and ranks by how badly a
bug breaks "people act like people" on camera. Every cite is the critic's own; a missing line is never evidence.

## Verdict

1. **The island pass did what ISLAND_PASS.md set out to do on its own claims.** Distance: island commute legs of 4 h or more fell
   20.3 % to 4.4 %, structural commutes 29.6 % to 8.7 %, mostly-travel labour pick-days 126 of 378 to 3 of 406. Vehicles: pick-days parked
   180 min or more 241 to 0, body-days parked or jumped 103,461 to 9 (the Coarse El walk excepted). Trades: courier parcels on screen 0 to 83,
   day posts filled 0 to 185 of 1,000, body-driven cab rides 0.3 to 15, an arms dealer exists, the 04:00 prisoner jump is gone.
2. **The camera still sees one mechanism the pass never touched: route choice.** The 4-16 h trips that remain are mostly to *near* targets:
   a Spire CEO, a sweeper, a companion, a loader and an orderly each spend 5-16 h getting somewhere in their own or the next district (seven
   lenses, bug 1). The pass fixed the pick and the body, not the path.
3. **Part 1 over-read one thing.** The "16 legs of 300 min or more ending in the wrong district" and `ceo_21278`'s "wrong-district Neon Quarter" are
   legs *cut* by hunger or sleep (`-> X (ended)` prints the cut point, not the destination; corp § 1 and its bug 2). `OPEN-164` stays closed; the
   cause is bug 1.
4. **Two things are pass regressions:** I6 company beds go to the wrong people or the wrong place (a ripperdoc in a yard barracks, a 444-min
   dorm), and I1's stay-in rule has no fun fallback (zero-leisure vice diaries 0 to 9 of 70, fun 0.00, home Rest 1,358 to 2,454 h). Both are fixable and in scope.
5. **Gate: passed on the pass's claims, not clean for the camera.** Close it with a fix round of two phases (routes; beds) and a third
   conditional on Dylan's call on I1 (§ Proposed fix-round scope). No crash, no nondeterminism, 0 asserts failed. Hunger and Murders are not the pass's (§ FINDINGs).

## The rows, V7 to V8

| row | V7 on the island | V8 on the island |
| --- | --- | --- |
| 0 destinations without distance | failed, above all | **fixed in aggregate**; a tail remains: the Hideout bed (58 of 78 Hideout legs go to the Sump), company beds picked without the post (bugs 2, 4), non-urgent goals with no reach (bug 7) |
| 1 statue shift | one block for fabber, super, Court, Deal | **partly**: sweeper 271 to 196, cook flat; fabber 537 and super 417 unchanged (`OPEN-169`); builder 90 to 296 and foreman (19 days over 180 min) are new site-day blocks; `GuardJail` 538 min unchanged |
| 2 idle freeze | fixed | **fixed as a freeze, back as a stay-in**: elder home 24 h on 7 of 14 d0 pick-days (V7 0), idle +1.5 h/d on both 20k cities, 1-minute `Rest` loops in shift hours (bugs 3, 8) |
| 3 presence | leaks left | **unchanged, vendor worse**: vendors settle asleep 26 vs 5 at d90, orderlies' 00:00 batch 26 of 26, the teacher's door; pick asleep 3.3 % at d90 (`OPEN-170`) |
| 4 long legs | worse: 19-25 % of legs 4 h+, parked 3-11 h | **mostly fixed**: 4.4 %, 0 parked; the tail is same-district and adjacent-district rides of 5-16 h (bug 1) |
| 5 housing churn | 6 of 18 corp picks sleep away | **churn fixed, new misallocation**: no double `Housed`; homeless +55 % (193 to 300; `off` 234); company beds misgiven (bug 2) |
| 6 dead trades | all dead but leader, loader | **alive, thin**: courier, day labourer, foreman, arms dealer, cab (each at one fixed minute or one sale); bouncer, joytoy, scav still 0 (`OPEN-176`); pimp not a trade; black doc has no patient |
| 7 implant loop | fixed | **fixed, not re-read** (no lens contradicts it) |

## Cross-lens patterns, checked against the critiques

| orchestrator's pattern | verdict | note |
| --- | --- | --- |
| 1 same-district trips that never converge | **confirmed, and wider** | corp (107/149 El and 64/74 `exec_ride` legs aborted), street (sweepers 10-16 h), gangs (same-district GangWork 4-13 h), vice (56 of 165 legs of 200+ min), care (orderly D4 to D5 313 min, 12 of 71 legs), labour (loader 443 min inside D6). Crime's pimp is the same *shape* but a different bug (a worker in another district, bug 10). The root differs in kind: for the Spire it is district size (D5 spans x 200-1252, 150-350 tiles apart inside one label); for the rest it is the route taken |
| 2 company beds to non-staff or far | **confirmed** | care (ripperdoc), labour (loader, retiree) |
| 3 picks still keyed to district or corp | **confirmed** for the builder, the sanitation beat, the Hideout and the pimp | one Hideout per gang is a design (D2) |
| 4 1-minute `Rest` loops in shift hours | **confirmed** | crime (gun), gangs (scav 77 to 312), care (super), corp (`exec_22432`, 0 `Work` lines in 7 days) |
| 5 I1 stay-in rule | **confirmed; part of it is a bug** | the vice critic's joytoy has *no Unwind goal at all* (0 in 697 lines) and `distance.rs` says the own Home stays as the fallback, so the offer is gated before that fallback. The rule is a decision (D1), the fallback a fix (bug 3) |

## The ranked bug list

Ranked by how badly it breaks a followed person on camera. "Lenses" are the critics that saw it.

### 1. Short trips run half a day to a day: route choice, not the pick

- **Root cause.** The route, not the destination. Two sub-causes the critics separate:
  - *The Spire is one label.* Same-district D5-to-D5 legs run 150-350 tiles; the El is ~1 tile/min, the walk 2. `streetwork::exec_ride` returns `None` when
    home and Hq share a district label, so a Spire exec gets no car (corp § 1).
  - *The El loop is chosen when walking is shorter.* `transit::choose` accepts a 2.5 h access walk and a loop route longer than the walk (no cap versus the walk);
    a plan dropped mid-ride keeps the rider aboard past the alight stop (`motion::alight`, `held`), so the loop is ridden again (street bug 1; the 1,111-min estimate is its signature).
  - Unverified: the vice critic suspects a path that detours through a neighbour district (companion_7618 D5, D4, D5); the care critic's orderly earns 5c, which has no fare. The first step of the fix is a leg-log dump of the named cases below.
- **Lenses.** corp, street, gangs, vice, crime, care, labour.
- **Evidence.**
  - corp: `B/ceo_21278 d30 06:00` 668 min, cut at `17:08` by Eat; `C/exec_6400 d91 01:47` 823 min, travel 20.6 h, meeting 1.5 h; `A/ceo_17908 d0 00:00` 974 min.
  - street: `isle_s44_d90/sweeper_6405 d90 23:08` 1,111 min Block#90 D4 to Recycler#4461 D4, touring D6, D1, D3, D4, D2, then `Sleep @ street`; the same Recycler for `sweeper_10991`, `_7882`, `_8203`, `_8335`.
  - vice: `s44/companion_7618 d91 10:28` 438 min and 649 min home, work 0.1 h, street 17.5 h; `s44/club_staff_8280 d94` 601 and 371 min.
  - gangs: `isle_s43_d30/gang_member_7638 d35 00:16` 808 min, travel 20.1 h inside D1.
  - care: `isle_s43_d30/orderly_22353 d30-d36` 313-382 min each way, D4 to D5.
  - labour: `isle_s43_d30/loader_13401 d30 00:00` 443 and 444 min, dorm and yard both D6.
- **Suspected sites.** `streetwork::exec_ride` (district-label gate), `transit::choose` (access walk, loop versus walk), `motion::alight` (`held`).
- **OPEN.** New: `OPEN-188`. Folds `OPEN-163`'s remainder; `OPEN-164` and `OPEN-165` stay closed.
- **Owner.** Island fix round (F1). It also drives the 0.5 h execs and the corp guards' street time.

### 2. Company beds go to non-staff or far from the post (I6)

- **Root cause.** The I6 allocation takes free beds, not the resident's own workplace or the commute. "Site Barracks" is the `ConstructionYard` row (`assets/config.toml:4423`) and a clinic doctor holds one; a loader's dorm is 265-444 min from its yard; a retiree's `move_in` "with family" lands in a D6 coffin rack.
- **Lenses.** care, labour.
- **Evidence.** `isle_s43_d30/ripperdoc_25321` Home Site Barracks#26462 D6, clinic D3, travel 19.9-24.0 h, `d34 18:06` 1,412 min; `loader_13401` Dorm Storeys#26520 (D6), 265-444 min; `loader_19054` a D1 bed for a D6 yard, 146-150 min each way; `worker_6720 d5 00:00` Coffin Racks#26535, 721 min.
- **Suspected sites.** `household.rs:217 company_due` and `:303 company_only`, the `company.rs` bed assignment; the `move_in` target in the relative path.
- **OPEN.** New: `OPEN-189`.
- **Owner.** Island fix round (F2). A pass regression.

### 3. The stay-in rule leaves nothing to do: no fun fallback

- **Root cause.** With no spot in reach, the goal never offers the own-Home or own-Block HangOut. The joytoy has 0 `Unwind` goals in 697 lines; elders sit in 90-min `Rest` blocks for 19-24 h with food at home. This goes past I1's wording ("the person stays in"): staying in should still run a low-rate Unwind.
- **Lenses.** street, vice, care (gangs and crime by idle hours).
- **Evidence.**
  - vice: 9 of 70 diaries with no HangOut and no fun hour (V7 0); `s43/joytoy_13649` fun 0.95 to 0.00; group HangOuts away from home 362 to 178; home Rest 1,358 to 2,454 h.
  - street: `isle_s42_d0/elder_25682` home 24.0 h on d0-d3, d5, d6.
  - care: new edges, ripperdoc 14.1 to 8.9 a week; island d90 carer settlements 81 to 10.
  - Part 1: idle +1.5 h/d, home +2 h.
- **Suspected sites.** `leisure.rs` after `unwind_cap`; `leisure::best_spot_kind`; the Unwind goal in `goap/`.
- **OPEN.** New: `OPEN-190`; relates to `OPEN-109`.
- **Owner.** Island fix round (F3, conditional on D1). The rule itself is Dylan's.

### 4. One Hideout per gang: a cross-island commute

- **Root cause.** `plan.rs:606 bed_of` returns the gang's one Hideout for a Sleep plan and does not compare it with Home or Work. 58 of 78 island Hideout legs go to the Sump; the walkers live in D3-D6.
- **Lenses.** gangs (crime and corp see the commute).
- **Evidence.** island GoTo(Hideout) median 103 to 144 min, 21 of 78 at 240+; `isle_s44_d90/purist_14764 d93 00:02` 1,061 min; `gang_leader_9711` (Spire day job, Hideout in D1): sleep 4 h, travel 19.9 h, work 0 on d91 and d93.
- **Suspected site.** `systems/plan.rs:606` `bed_of`.
- **OPEN.** New: `OPEN-191`. Not `OPEN-163`'s phase 1, which drops candidates past a cap *unless none is left*; a gang has one.
- **Owner.** The bed gate: island fix round (F2). The design (a Hideout per district): D2, vertical city pass (`OPEN-187`).

### 5. A person sleeps where they stand, on a street, for 4-13 hours

- **Root cause.** The Work-to-Sleep interrupt sleeps in place after a cut leg or a long shift instead of walking to the bed; a company-housed man picks a street over a bed 3 min away. Most cases follow from bug 1.
- **Lenses.** gangs, labour, street, crime.
- **Evidence.** `isle_s44_d90/gang_leader_14477 d91 18:47` Sleep @ street 709 min, six minutes after leaving the Vat Farm; `isle_s43_d30/gang_leader_8256 d33 17:50` 747 min; `loader_13401 d31 06:32` 765 min; `sweeper_6405 d91 17:41` 240 min; `buyer_9460 d94` on a D1 street after `Housed` in the Spire. Island sweepers 15 Sleep @ street (2k 0); leaders 12 of 101 sleeps to 18 of 119.
- **Suspected site.** The Sleep goal's place pick when a Home exists (the homeless are exempt); not traced.
- **OPEN.** New: `OPEN-192`.
- **Owner.** Island fix round, re-read after F1 and F2; fix only what is left.

### 6. The builder's site day walks the wrong way

- **Root cause.** With a far site the shift still starts with `GoTo(Workplace)` at the Yard, a 16-min load, then the carry out; a Garage, a Ripperdoc and a Capsule Hotel count as "sites".
- **Lenses.** labour.
- **Evidence.** `isle_s43_d30/builder_7236 d31 06:16-18:32`: 473 min walking, 311 min ClerkWork at a Garage in his own D1; `builder_23745` and `_9487 d93-d96`.
- **Suspected sites.** `shift.rs::far_site`, `site_day_dest`; `industry::sites_of`.
- **OPEN.** New: `OPEN-193`.
- **Owner.** Island fix round (F2). A phase 8 regression.

### 7. Non-urgent goals with no reach cap

- **Root cause.** `[distance]` covers the job, bed, hangout and Hotel. Other goals pick any target: the clinic, reporting a crime (one Hall on the island), registering a company, an Intel stakeout, the sanitation beat.
- **Lenses.** labour, vice, crime.
- **Evidence.** `day_labourer_19868 d34 08:44` 577 min to report a crime, a shift lost; `s44/madam_21371` GoTo(Clinic) 208-278 min five mornings, never arrives; `d30/gun_8526 d32 00:01` 897 min to register a company; `worker_friday_17446 d30-d36` a sanitation beat six districts from the Recycler.
- **OPEN.** New: `OPEN-194`.
- **Owner.** Island fix round only as a stretch of F1 (a reach table in config); otherwise the pressure pass.

### 8. 1-minute `Rest` loops, and an Idle goal when work exists

- **Root cause.** The Idle goal fills with 1-min `Rest` plans every 6-30 min when nothing outranks it; an employed pick idles through the shift (`gun_8526 d31` 21.4 h idle with a Manager job); `C/exec_22432` plans no `Work` for seven days with no reason on the page.
- **Lenses.** crime, gangs, care, corp, street.
- **Evidence.** scav `Rest` 77 to 312 (`isle_s44_d90/scav_14509 d91 12:47-17:47`); `super_22721 d6` idle 21.5 h; `C/exec_22432` 113 plans, 0 `Work`; `isle_s42_d0/homeless_11420` d0 16.2 h (`OPEN-135`).
- **Suspected sites.** `goap/` Idle fallback; `life::hq_of` returning `None` (corp).
- **OPEN.** New: `OPEN-195`; relates to `OPEN-135`.
- **Owner.** Pressure pass.

### 9. The remaining presence leaks

- **Evidence.** `isle_s44_d90/vendor_18034 d92 12:14` Sleep @ street 700 min, 20 settles asleep in one day (26 at d90 vs 5); orderlies' `treated` 26 of 26 at 00:00 (`[off]`).
- **OPEN.** `OPEN-170` (annotate).
- **Owner.** Island pass leftover (`presence::stat_home`); pressure pass if not closed in the round.

### 10. The trades that are still not a trade

- **Bouncer, joytoy, scav:** 0, 0, 0 (`OPEN-176`). The bouncer's door exists only at a Club; scav diaries have 0 `Rip` or Abduct.
- **Pimp (`OPEN-120`):** 2 `Collect` and 0 `StreetCut` in 4 pimps; `vice.rs:367 pimp_collect` takes any worker with a pitch in his district, and the worker is buying food in D3 (`d30/pimp_8936`). Picks the worker by district, not travel.
- **Arms dealer:** 1 sale in 28 pick-days, a gang paying itself; `bar_arms_dealer` needs a registered dealer in the bar and a gang `surplus()`.
- **Black doc:** 0 patients (`OPEN-017`).
- **Courier:** 3 of 6 diaries hand nothing; the rest settle ten parcels at one stamp (`courier_6744 d30 10:39`). New: `OPEN-196`.
- **Owner.** Pressure pass.

### 11. The rest, small

- **Fare-evasion arrest loop** (`isle_s44_d90/fixer_14707` arrested 5 times in 6 days), a release a 296-min walk from home, 13 h from arrest to cell: `OPEN-197`.
- **Day-0 post-holders pay from an empty purse and quit unpaid** (4 of 4): `OPEN-198`.
- **Collected "0 to 0 members"** in 8 of 17 lines; `OPEN-118` closed on a different symptom: `OPEN-199`.
- **Shadow tool label:** print `cut at X` for an `ended` leg and recount Part 1's wrong-district rows from `done` legs only: `OPEN-200`.
- **`GuardJail` 538 min**, the 2k warden (0), the super's empty round (13 rounds, 0 calls) and 7-h lobby wait, the d0 foreman commute (11 legs of 4 h+): annotate `OPEN-169` and `OPEN-172`.
- **Gossip label inverted**, `GoTo(Hotel)` `NoSuchPlace` twice, the self-therapy "treated X" label: annotate `OPEN-173`.

## The calibration FINDINGs (not fixes)

- **Hunger.** 6,185 islanders still malnourished at d90 (V7's binary 7,015), 3 of 8 d90 carers at 0.30 or more; the `sick` pick fell 2,188 to 934 because food reached more mouths, not because anyone is treated. `OPEN-167` is untraced and predates the pass.
- **Murders.** Island 16 to 45 against V7's close at 156 on seed 42: the pass is not the cause. 2k 79 to 32: standing decision of 2026-10-17, leave it.
- **Homeless +55 %** (193 to 300; `off` 234, so the pass adds 66): 489 job-loss and 323 buyout evictions a seed are I6 working as designed ("the company town's leverage"). No critic's pick shows an eviction.
- **KnownMetWeek 1.27** (range 2.78-9.47) and new edges down 2-4x: isolation, tied to the stay-in rule; doctors treat nobody present on the island.
- **Idle +1.5 h/d, home +2 h, street 7.4-8.8 to 5.9-6.7 h**: the camera's emptier street. Not tuned.
- **Island Murders 3 x V7's base, arrests -15 %**; the Statistical mirror's kill rate 0.025 to 0.071.
- **Cultist Persuade failed 3 of 3**, scav Extorts paying 0 in 6 of 10, dealer sales on camera 7 to 1.
- **The courier a minute a day, the cab's 8 fares all Statistical regulars at 09:32** (`OPEN-149`).
- **103 body jumps of 8 tiles or fewer at a car's kerb**; the Coarse walk to the El is parked or jumped on 82 % of its body-days (`OPEN-165` "left", for the view milestone).
- **Ticks/s** 248 to 271 on the A/B; print only.

## Decisions for Dylan

### D1. I1's stay-in rule

Cost, from three lenses: home Rest 1,358 to 2,454 h, zero-leisure vice diaries 0 to 9 of 70, fun 0.00 on joytoys and cooks, new social edges down 2-4x, carer settlements at d90 81 to 10, idle +1.5 h/d.

- **A. Keep as it is.** Street life near home is `OPEN-109`'s later feature. The city is quieter and lonelier; cheap, and the camera sees emptier streets.
- **B. Keep the cap, give stay-in a fallback.** Past the cap the person runs a low-rate Unwind at their own Block or Home (flatmates, a street spot next door) and fun does not sink. This is what `distance.rs`'s own comment promises. **Recommended**: the zero-Unwind joytoy is a bug under the rule's own text, and B costs one mechanism.
- **C. Raise the cap.** Longer walks to a spot. Rejected: it is the 4-hour hangout walk I1 removed.

### D2. One Hideout per gang

58 of 78 Hideout legs go to the Sump; a leader with a Spire day job loses it to the Hideout bed.

- **A. Keep one, gate the bed.** A Sleep plan uses the Hideout only when the walk is within the commute cap, else Home. Rounds, court and orders still go to the Hideout. **Recommended now** (F2).
- **B. A Hideout per district the gang holds**, or a second once the median walk passes a cap. Right for the story and ties into territory; belongs with the vertical city pass (`OPEN-187`, a gang's floors in several towers).
- **C. Hideout as a meeting place only.** Members sleep at Home. Cheapest, but it removes the safehouse.

### D3. District labels are not space

The Spire is 150-350 tiles across and is one "district" to every same-district test (`exec_ride` and the map's labels). A rule that treats the label as near is wrong at the Spire.

- **A. Fix every "same district" test to tile distance.** In F1. **Recommended.**
- **B. Place execs' homes next to their Hq** (`life::hq_of`, `exec_home_tier`). Right for the CEO; do it with the next island re-import (`OPEN-182`).
- **C. Split the Spire in the map.** The graphics repo's call, before the view milestone. Not asked for yet.

### D4. Malnutrition has no treatment path (I5 said: decide after distance)

Distance is in; 6,185 are still malnourished and `GOAL Treat` runs once in 36 care diaries.

- **A. No clinic path.** Hunger stays a pressure; the standing "harsh by design" and "no social safety net" decisions apply. **Recommended.**
- **B. A clinic path for staff only** (a doctor treats a sick colleague). Small, and it keeps the carers visible.
- **C. A general malnutrition treatment.** The safety net by another name; the 2026-10-08 decision says no.
- Whatever the choice, trace why `OPEN-167` climbs from d30 to d90 in both binaries. That is a measurement, not a feature.

### D5. When to close the pass

- **A. Fix round F1 and F2 (F3 if D1 is B), then close; the view milestone next.** **Recommended.**
- **B. Close now,** write bugs 1-4 as the view milestone's entry list. The camera starts on a city where sweepers ride 16 h. Not recommended.
- **C. F1-F3 plus pressure-pass items 7-8.** The pressure pass has no spec yet; this delays the view for features.

## Proposed fix-round scope

Each phase is one mechanism, one fresh coder (Opus 5.5), ~300 turns, per the SOP. Light measurement per phase; the full set at the close.

- **F1. Routes (bug 1).** One mechanism: when a trip's route is longer than walking, walk.
  - `streetwork::exec_ride` gates on tile distance (the `leg_replan_tiles` scale), not the district label.
  - `transit::choose` refuses an El route longer than the walk (or with an access walk above a config cap) and never boards a loop to a stop it passes.
  - `motion::alight`: a dropped plan within walking distance of the alight stop gets off.
  - Step 0 (30 min cap): dump the leg log of `sweeper_6405 d90`, `companion_7618 d91`, `orderly_22353 d30`, `loader_13401 d30` and `ceo_21278 d30`, and say which sub-cause each is before changing code.
  - Measure: GoTo legs of 240 min or more that start and end in one district (vice 56 of 165, gangs 25), `exec_ride` and El legs `aborted` (107/149, 64/74), sweeper vehicle hours, the same cases' day summaries.
  - Fold in the Shadow tool's `ended` label (`OPEN-200`), one line in `shadow.rs`.
- **F2. Beds (bugs 2, 4, 6).** One mechanism: a bed is taken only if it is the resident's own workplace's or within the commute cap.
  - Company beds: ownership and reach in the allocation.
  - The retiree `move_in` target limited to a relative's own Home.
  - The Hideout bed gated by the walk (D2 A).
  - The builder's site day starts at the site (small, same file family).
  - Measure: company beds held by non-staff (0), loader and ripperdoc travel hours, GoTo(Hideout) median and 240+ count, the builder's walking minutes, the off identity on the bed switch.
- **F3. Stay-in fallback (bug 3), conditional on D1 = B.** The Unwind goal offered at the own Block or Home when no spot is in reach; the Rest loops (bug 8) are read after it.
  - Measure: zero-leisure diaries (9 of 70 to 0), home Rest hours, `--lives` idle and named per week on both 20k cities.
  - Bug 5 (sleeping on a street) is re-read after F1 and F2.

Not in the round: bug 7 (a stretch for F1 only if the config table is cheap), bugs 8-11, and `OPEN-167`'s trace. They go to the pressure pass or a short trace.
