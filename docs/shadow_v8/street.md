# Shadow V8, the street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden)

Reader: street critic, read-only. Base `$V8` = `D:/TheCity/target/shadow_v8/`, `$V7` = `D:/TheCity/target/shadow_v7/`. Cites are
`window/file dDD HH:MM`. A missing line is never evidence; Coarse/Statistical stretches are quoted only for lines that exist.
Read: my 7 archetypes in all five pinned windows (V8: 9 + 16 + 16 + 16 + 9 picks; V7 same archetypes), every camera.tsv row of
the group rolled up V7 against V8 (`home_h/work_h/street_h/vehicle_h`, awake-still 300+, jumps, `settles_asleep`), GoTo legs of
300+ min, Sweep actions and places, Hotel/Cot targets, and full days: `isle_s44_d90/sweeper_6405` d90-d92, `isle_s43_d30/sweeper_10991`
d30, `isle_s42_d0/elder_25682` d1 and d4, `isle_s44_d90/vendor_18034` d92, `isle_s43_d30/homeless_21559` d32-d35,
`isle_s43_d30/street_worker_21709` d32. Same-person pairs: `elder_25682`, `elder_22703`, `street_worker_21709`, `sweeper_6405`.

## Verdict

The pass fixed the street group's frozen bodies, its hotel naming and its camp warden, and it made the sweeper round and the pitch
work. **The island's El rides are now the worst thing a camera sees**: a sweeper rides the loop 10-16 h a day without reaching a
post one district away, and the pass's own I1 rule ("no spot in reach, stay in") has put the elders behind their doors for 24 h.
Island sweepers: `vehicle` 4.5 h a day (V7) → 5.2/4.3 (V8), awake-still 300+ min on 25 pick-days → 5, but the new vehicle days are
the 10-16 h rides below.

## 1. Per archetype

**Homeless: alive, and the hotel is fixed.** V7 had `GoTo(Hotel)` land in a Soup Kitchen 19 times and a Church 14 times.
V8 has the same stops under their own plan, `GoTo(Cot)` (26 Soup Kitchen, 8 Church) and `GoTo(Hotel)` (28, all Capsule Hotels): 0 misnamed
(OPEN-171 fixed). `isle_s43_d30/homeless_21559` d34 23:49 `GoTo(Cot)` to Church of the Open Circuit#5556, CheckIn 00:47, a 376-min Sleep; d32
07:20 CheckIn at Capsule Hotel#5529, -3c, Sleep 366 min. The Scavenge/Beg/squat life from V7 persists (41 Scavenge in
`isle_s44_d90/homeless_12546`). Still thin: day 0 (see Still broken 4).
**Sick: still not sick as a trade.** 1 of 9 picks touches care (`isle_s44_d90/sick_21914`: 19 Ripperdoc lines, 2 treat; the others 0);
no diagnosed pick (`sick_9548` cancer 0.56-0.66, `isle_s44_d90/vendor_18034` cancer 0.99) seeks a doctor. One real gain: a sick-day
mechanic shows (`sick_9548` "OFF SICK TODAY", d33-d35 home 18.6/14.6/13.1 h idle then back at the Freight Yard d36 8.4 h work). Malnourished
72-year-old `isle_s44_d90/sick_13573` (malnutrition 0.50 on d90) eats 0.2-0.4 h a day and sits at home 9 h idle a day (hunger down, present).
**Elder: the new bug.** `isle_s42_d0/elder_25682` (the same person as V7, who travelled 13-21 h a day) is now at home 24.0 h on
d0, d1, d2, d3, d5, d6: `PLAN Idle [Rest]` in 90-min blocks, `idle 19.1h` (d1 summary), no UNWIND pick on 6 of 7 days (2 lines in the
week); a Market trip only d4, 375 min to D5 (d4 02:02) then 20.8 h on the street. Elder home ≥22 h: 7 of 14 pick-days
at isle d0 (V7 0). `isle_s43_d30/elder_13288` and `isle_s44_d90/elder_11065` do go out (UNWIND 15 and 17). Visits: 3 each in 5 of 9 (V7 6 of 9, 1 each).
**Vendor: alive on the island, loss-making, and still selling asleep.** `isle_s43_d30/vendor_15862`: 47 Vend, 110 `sold to`, 71 `GoTo(Spot)`;
`isle_s44_d90/vendor_17273` 25 Vend. V7's island vendors sold 29 / 9 / 11 / 20; V8's 110 / 6 / 28 / 30. Asleep: 26 `settles_asleep` at d90
island (V7 5). `isle_s44_d90/vendor_18034` d92 12:14 `Sleep @ street` 700 min at midday; 12:16-12:37 four sales, each "SETTLED APART ... I am asleep at
street (D5 The Spire)" (22 such lines, 20 asleep in one day). OPEN-170 unchanged, and louder.
**Street worker: fixed in 2 of 6.** `isle_s43_d30/street_worker_21709` (V7: Scavenge 28, the pitch closed "no take") now pitches:
d32 00:26-04:38 four `Vend` hours, d32 01:00 "+5c ... served Casimir Delacroix#10282", d33 05:00, d36 04:00 (net +37c on d34). Elsewhere
`street_worker_24937` is a ClerkWork hire (24 lines, pitch closed) and `_16619` Scavenges 29 times; PitchClosed in 5 of 6. Work hours 3.7/3.0 (V7 0.0).
**Sweeper: the round exists; the ride kills it.** Island stands of 300+ min fell 11 → 5; Sweep actions at the island 40 → 46, and a real
round in D4: `isle_s44_d90/sweeper_6405` d94 09:04-15:32 six 46-min `Sweep @ street (D4)` stops, 17:06 `Sweep @ Recycler#4461` 54 min (V7: 4 sweeps).
`sweeper_6228` d94 the same shape. 2k fine (53 sweeps, 26 street + 27 yard).
**Camp warden: alive, and the 2k regression is not fixed.** 94-102 ClerkWork lines a week in 9 of 9, 12 Breaks each, headcount
20-57 on 6 of 9 (`isle_s44_d90/camp_warden_18971` 57). `k2_s42_d26/camp_warden_833` 0 headcount lines (V7 `_2544` 0, V6 12);
`isle_s42_d0` wardens 0 (no children at d0). 0 "at the Camp" events in all 9.

## 2. V7 findings, re-read

| V7 finding | V8 verdict | evidence |
|---|---|---|
| Same-district GoTo of half a day, body stands still (bug 1) | **fixed as "stands still", worse as a ride** | island sweeper legs 300+ min: 10+9 → 7+9 of 49; max 962. No tile freeze; the body now rides, see Still broken 1 |
| Sweeper's stand a district from the post (bug 2) | **improved, not gone** | `isle_s43_d30/sweeper_8335` d32 09:12 `Sweep @ street (D1 The Sump)` 528 min (post in D4); d31 185, d33 206 and 263. `sweeper_6405` d96 09:58 482 min at street D4 |
| Arrest jump (bug 3) | **fixed** | 0 `Transferred` lines in my group; arrests are fines at the kerb (`isle_s43_d30/sweeper_19080` d32 05:00 "fined 5 coins"); body jumps 24, every one ≤6 tiles at a kerb (`isle_s44_d90/sweeper_8203` d93 06:45, 6 tiles) |
| Vendor sells asleep (bug 4) | **unchanged, worse** | above, 26 vs 5 |
| Day-0 aimless idle (bug 5) | **unchanged** | `isle_s42_d0/homeless_11420` d0 idle 16.2 h, d2 17.7 h; `sick_10511` d0 16.3 h |
| Hired homeless walks 8 h (bug 6) | **fixed in this sample** | no hire leg of 300+ min in the homeless picks |
| 2k warden counts nobody (bug 8) | **unchanged** | `k2_s42_d26/camp_warden_833` 0 |
| Hotel lands in a Soup Kitchen (bug 9) | **fixed** | 0 of 54 Hotel legs miswired; `Cot` is its own plan |
| Street worker pitch closed | **fixed for 1 of 6, same shape for the rest** | above |

## 3. Bugs, ranked (what a camera sees first)

1. **An El rider tours the island for half a day and never reaches a post in his own district; he then sleeps on the street.**
   `isle_s44_d90/sweeper_6405` d90 23:08 `PLAN Work ... GoTo(Beat) ... 1111 min` from Block#90 (D4) to Recycler#4461 (D4); d91 04:56
   D6, 08:22 D1, 10:01 D3, 11:41 D4, 12:51 D2, 12:59 D4, 13:15 D2, 13:17 D4, 13:36 D1, 15:07 D4, 17:39 `Rest @ street`, 17:41 Sleep on the
   street 240 min (camera d91: `vehicle 10.1 h`, `work 0.0`). Then d91 21:43 `GoTo(Home)` 962 min: d92 00:13 D1, 01:49 D4, 02:08 D2 ... 06:56 D6.
   Same on `isle_s43_d30/sweeper_10991` d30 07:41 `GoTo(Beat)` 630 min: D5, D4, D6, D1, D3, D2, then Hotel, then `GoTo(Home)` 667 min (d30
   20:01); `sweeper_7882` d30 07:09 673 min, `sweeper_8203` d91 07:12 661 min, `sweeper_8335` d35 720 min. Every one targets the same
   Recycler#4461. Island sweepers `Sleep @ street` 15 times (2k 0). Vehicle ≥6 h on 30 island pick-days of the group (V7 30; same count, new cause).
   Likely site: `transit.rs::route` (picks the structural ride, the 1111-min estimate is the bug's signature) plus the "plan dropped between
   two stops keeps him aboard" rule in `motion::alight` (`held`): the loop is ridden past the alight stop, replanned, ridden again.
   Fix: a leg whose `Route::total` exceeds the walk to the same district's target by a margin walks instead; check `alight` when the plan is
   dropped and the stop is within walking distance.
2. **The "stay in" rule empties elders and sick** (`isle_s42_d0/elder_25682` above; `elder` home ≥22 h on 7 of 14 pick-days at d0 vs 0 in V7;
   gen `elder_16993` still out; `sick` 1 of 14). Plausible for a 70-year-old, but this is every day with no UNWIND pick line and 90-min `Rest` blocks, i.e.
   the hangout resolver returns nothing and the fallback is Idle Rest for 19 h. No person sits motionless 19 h with food in the fridge; a hangout
   in reach is missing near the elder's home, not past a cap. Site: `leisure.rs` spot pick after the I1 cap (`unwind_cap`). Fix: when no spot in reach, fall back
   to the home's own HangOut (the Lives-at-work own-Home HangOut) or a block neighbour, not to Idle. **The sore point is real**.
3. **Vendors sleep at the pitch and trade over their body** (`isle_s44_d90/vendor_18034` d92 12:14-23:54; 20 settles asleep in one day).
   Site: `presence::stat_home` / the pick's sleep is not consulted in the Statistical-counterpart settle (OPEN-170). Midday `Sleep` itself
   (energy 0.88, phase 0.25) is a different thing: this pick has cancer 0.99 and sleeps on the pavement; calibration.
4. **Day-0 and day-1 aimless Idle**, 16-18 h (`isle_s42_d0/homeless_11420`, `sick_10511`): the backlog week the camera sees first. Site: `goap/` Idle
   plan fallback (OPEN-135).
5. **Camp warden counts nobody at 2k** (`k2_s42_d26/camp_warden_833` 0; V6 12). Not diagnosed; see `presence.rs`.
6. **Stray same-district legs of 100-180 min** (`isle_s43_d30/street_worker_21709` d32 04:38 `GoTo(Home)` 181 min, D1 to D1 via a D4 El stop; d32 13:54
   `GoTo(Market)` 113 min). Same cause as bug 1 at lower amplitude.
7. **`GoTo(Hotel)` NoSuchPlace** (`isle_s43_d30/sweeper_7882` d32 08:05) 2 lines; name the shelter or fail loudly.

## 4. Sore points

- **Home +2 h a day**: in the group, home ≥22 h on 20 island pick-days (V7 6); elder home 7.1/6.4 → 18.0/15.7 at isle d0/d90; sweeper home
  4.9/4.7 → 5.3/7.7. Street time: homeless 16.9/15.5 → 13.4/16.6 (steady), elder 13.6 → 4.1. The elders are the mass; the street is
  emptier because the elders are in, not because the trades are.
- **Homelessness +55%**: my homeless picks do not show an eviction, and the cot, hotel and squat each work (`homeless_21559`:
  a gang member sleeping in a Church cot d34). Rumour lines only (`homeless_12546` d93 00:00 "Evicted by corp Nutrix 5 ..." about others).
  Not confirmed in my sample; see the counters.
- **Hunger**: `sick_13573` malnutrition 0.50 and 7 of 9 sick picks have a malnutrition ≥0.1 line; the eat lines are real (`ate a REAL meal`) at the 2k and
  island vendors, SYNTH for `sick_9548`. Down, present.
- **Coarse walk to the El**: no street pick shows a Statistical hour (`stat_h` 0 on every row of my group); not observable here.

## 5. Calibration FINDINGs (not bugs)

- FINDING: island vendors sell 6-110 times a week (V7 9-29), net negative still; `vendor_15862` buys 12 Wholesale.
- FINDING: street workers' clients are all Statistical `[off]`; 21709 served 5, 16496 3.
- FINDING: 7 of 9 sick picks have no care step; the care lens (`care` critic) owns this.
- FINDING: the camp warden sleeps/idles 11-13.6 h at home on island days (work 7.7-8.4 h); a normal clerk shift.
- FINDING: the `Cot` target is a Soup Kitchen in 26 of 34 legs, a Church otherwise; a viewer sees a person sleep in a kitchen (the intended OPEN-171 behaviour).
- FINDING: body jumps are 24 in the group, all ≤6 tiles at a kerb or platform.
