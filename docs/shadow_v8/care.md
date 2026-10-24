# Shadow V8 critic: care (ripperdoc, orderly, teacher, super)

**Verdict.** The shifts, the break and the label work from V7 hold, and the d90 carers are no longer starving on the wage. But the care
group is *thinner* than at V7, not richer: new edges fell by half to three quarters, hangouts by two thirds, island d90 settlements for 8
carers went 81 -> 10, and the two bugs V7 named (the orderlies' 00:00 batch, the super's pre-shift wait and one-tile statue) are
unchanged. One new, loud bug: a company bed for the wrong trade puts a ripperdoc on a 24-hour walk.

36 diaries (8 per 20k window, 4 at 2k; none in `isle_s43_d30_nopin`), all Full and pinned. Paths are `<window>/<file>` under
`D:/TheCity/target/shadow_v8/`; V7 under `shadow_v7/`. Only `gen/teacher_21323` and `k2/teacher_1351` are the same person in both reads, so V8 vs V7 is archetype to archetype, from per-diary roll-ups (my script, read-only). Header malnutrition is the
pick's state at the end of the window. Counts are per 8 diaries unless said.

## 1. Per archetype

**Ripperdoc: the day shape holds; the patient does not come.**
- *Fixed/held.* 5-10 station blocks of 31-88 min, 4-5 tiles, no statue (`isle_s42_d0/ripperdoc_12962` d0 09:04-17:59, ClerkWork 69, 69,
  54, 41, 32, 79, 88, 77 min). Pre-shift Rest 19-41 min.
- *Still broken.* Real patients: the island six doctor-weeks show **0** on the doctor's own shift. The only two island `treated` lines are
  `isle_s44_d90/ripperdoc_17179` d93 10:14 and 11:16, and the patient there is the doctor himself (`treated Rhoswen Kettering#17179`,
  after `d93 09:05 PLAN Treat ... [GoTo(Clinic) > Therapy]`): a self-therapy booked as "treated X (a patient)". V7 had 2 present.
  Gen has one (`gen/ripperdoc_18017` d30 14:25). The hourly Meeting memory is mostly gone (V7 47 and 50 lines -> island 9 in
  `ripperdoc_12962` d4 09:00/15:00, `[off]`), but **2k still has it**: `k2_s42_d26/ripperdoc_1124` 52 lines, hourly from `d28 09:00`.
- *Break.* `ACTION Break` appears for the first time in the two d0 doctors (2 each, `ripperdoc_12962`, `ripperdoc_9394`) and in no doctor
  or orderly after d0: the P7 break still skips the Clinic's shape (V7 bug 9, unchanged but partly lit).
- *Pick drift.* None this time: all 10 ripperdocs hold a Ripperdoc job. The V7 bankrupt clinic owner is gone.

**Orderly: the 00:00 batch is the only clinic work and it did not move.**
- All 26 `treated` lines in the 8 orderlies fall at `00:00` (V7: 55 of 61 at 00:00 in its 5 night orderlies): `isle_s42_d0/orderly_23264` 10, `gen/orderly_14288`
  7, e.g. `isle_s44_d90/orderly_17822` `d92 00:00` `[off] memory: treated Kasimir Ravensworth#11962 (a patient) ... Statistical (off screen)`.
  Volume fell (V7 45 in one orderly-week, V8 at most 10), so a viewer sees fewer, not truer, patients. This is OPEN-170, not fixed.
- Night orderly `isle_s44_d90/orderly_17822` (shift 21:00-06:00) is the healthy case: ClerkWork stations 77-91 min, eats 04:47 and 06:13,
  malnutrition 0.40 (d90) -> 0.34 (d95) *falling*, wallet 11c.
- Social collapse: orderly new edges 10.4 -> 2.5 a week, hangouts 18.6 -> 5.4, `lives` island orderly `met` 0.17 -> 0.03, `enc/shift`
  0.22 -> 0.03. The V7 coworker pair (best friends in 26 h) has no V8 counterpart: no orderly has a named coworker edge.

**Teacher: same days, same empty school.**
- Station day with a smoke break 13:18 (`isle_s44_d90/teacher_7901` d91 `13:18 Break 16 min`); 6 breaks a week in all 9 teacher diaries.
  No pupil is named in any diary (OPEN-135 as V7). Idle 5.5 h/day (V7 3.8).
- The door: **4 lines, all `SETTLED APART`**, in 2 of 8 20k teacher-weeks: `isle_s43_d30/teacher_16791` d32 09:00 and d35 09:00 (parent Dmitri
  Inchbald at Block#3476), `gen/teacher_21323` d30 09:00 and d32 09:00 (Bronwen Nightingale at Block#2541, "Mid 8"). Same shape and rate
  as V7 (5 lines). `teacher_21323` is the same person as V7's: V7 had no door line, V8 has two, so the door is not a fix, it is a draw.
- `isle_s44_d90/teacher_7901` d90 (a rest day): 12 `Rest` blocks of 90 min at home 07:56-22:11 and `HangOut @ Block#288` indoors. See Finding 1.

**Super: the round is longer, the rest is the same.**
- Island and gen: 8 of 9 supers hold a Super job (`isle_s42_d0/super_15703` is a 63-year-old who `Retired ... (to family)` at `d0 00:00` and
  then ClerkWorks 538 min at a Vat Farm: pick drift, the same retiree as V7's `super_15263`). Rounds: 6 of 8. No round: `isle_s44_d90/super_17003`
  (0 `SuperRound`, 536 min of ClerkWork on one tile a day, d91-d96). V7 had 3 of 9; now 1 of 8, so OPEN-154's cap is fine.
- **But two of the six rounds are empty.** `isle_s43_d30/super_15202` walks a 4-Block round every day (d30 09:02-09:34 ...) and `isle_s44_d90/super_24258`
  a 2-Block round (d91 10:23), and neither has a single `called on` or `tenant` line in 7 days. `isle_s42_d0/super_22721` has 170 `called on`
  in 7 days (V7 `super_22452` 49), `isle_s43_d30/super_25046` 32. A super who walks a round and calls on nobody is worse than one with none.
- The statue after it: median longest work action 417 min (V7 373). `super_22721` d2: 6-Block round 09:04-12:33, then `d2 13:10 ClerkWork 290
  min`; `super_24258` d91 `10:52 ClerkWork 428 min (ended)` in the lobby. And the V7 lobby wait is unchanged: `super_22721` d2 `02:17 Rest` 91 min x4
  then `08:29 Rest 31` before a `09:04` round (6.8 h Rest at the post for a 09:00 shift), and `GOAL Work` at `d91 05:58` and `23:42`
  (`super_24258`) with "in shift 1.00" for a 09:00-18:00 post.
- Super idle 3.6 -> 7.8 h/day (the largest care rise): 7-hour pre-shift Rest plus a whole rest day on `Rest` (`super_22721` d6: 21.5 h idle,
  sleep 2.2 h, 1-minute `Rest` re-plans every 6 min from `d6 09:11`).

## 2. The sore points

- **Hunger (OPEN-167, I5).** Better, and the carers are no longer the sick: at d90 the 4 island carers read 0.42 (orderly_17822, eating a
  daily meal and falling), 0.39 (`super_24258`), 0.30 (`super_17003`, eating daily; 0.28 -> 0.23 over d92-d96), 0.10 (`orderly_18240`); V7 had 0.76 at 0c.
  None goes to a bin or starves. Malnourished carers *do* work the shift unfed? No: they eat (`super_17003` `d94 18:16`, `orderly_17822`
  `d92 06:13`). The one who suffers is `super_24258`: `OFF SICK TODAY` on d90 and d95 (sick days 2/14), which he spends gambling
  (`d95 06:15 Gamble`), on stims (`01:38`, `07:57`) and `PLAN FAILED ... ClerkWork: PreconditionLost` (09:19, 09:33), joined a gang at `d90 10:19`.
  **No clinic treats malnutrition**: `GOAL Treat` appears once in 36 care diaries (the self-therapy), in no sick or malnourished pick at
  d90 and in none of the 4 `sick_*` diaries. The sick pick fell 2,188 -> 934 because food reached more mouths, not because anyone is treated.
- **KnownMetWeek 1.27.** Real, not sampling. The d19 assert row is n 17 (8 / 5 / 4 by seed) at 2.00 / 1.20 / 0.25 against V7's 2.56-3.00
  / 1.75 / 4.57, and the same fall shows in the diaries: ripperdoc new edges 14.1 -> 8.9 a week, `lives` island ripperdoc `named/wk` 1.40 -> 0.25,
  `coloc_h/wk` 28.2 -> 6.75 (Statistical spouse share 0.04 -> 0.41). The doctor works a clinic that has no one in it (the spouse and the
  boss are Statistical), travels 2 h to it (`ripperdoc_12962` 124-min `GoTo(Clinic)` in the same district, d0 06:15) and sits at home 9 h. So
  isolation is real: calibration, a FINDING.
- **The teacher's door and the 00:00 batch.** Both unchanged (above). OPEN-170 stays open.
- **The super's round 373 -> 417.** Calibration of a statue, not a bug of its own: a longer round followed by the same one-tile shift.
  The bug is the empty round and the 7-h lobby wait (bugs 3, 4).

## 3. Ranked bugs

1. **A non-builder housed in a Site Barracks, 24-hour walks, no work for six days.** `isle_s43_d30/ripperdoc_25321` (32 y, Corp, wage 16,
   clinic Ripperdoc#5551 in D3 Neon Quarter) has `Home: Site Barracks#26462 (D6 The Vats)`. Work days d30-d35 are 0.0 h; travel 19.9, 24.0,
   19.6, 15.6, 10.0, 24.0 h; sleep 240 min (d30-d33); legs `d30 04:09 GoTo(Clinic) 832 min`, `d30 18:01 GoTo(Home) 720`, `d31 10:15 GoTo(Clinic) 466`,
   `d34 18:06 GoTo(Clinic) 1412 min (done)`; the legs end on a street in D1 or D6 (`(ended)`), never at the door, and the next leg turns
   round. She reaches work once, d36. `d32 08:00` hunger 0.14, mood -0.48. Company housing (I6): "Site Barracks" is the
   `ConstructionYard` row (`assets/config.toml:4423`), so a clinic doctor holds a bed meant for a yard's staff. Suspected site:
   `citysim/src/systems/household.rs:217 company_due` / `:303 company_only` and `company.rs` bed assignment (homeless-staff path), which
   never checks the bed's workplace against the resident's own. Fix idea: a bed goes only to the workplace's own staff, or the bed is offered only
   when the commute from it is within the commute cap; and `Work` cools off when the walk exceeds the shift.
2. **A 5-hour walk, both ways, every day, and a work day at 09:35.** `isle_s43_d30/orderly_22353` (Mid-Levels D4 -> Spire D5, adjacent
   districts): `GoTo(Workplace)` 327, 314, 342, 319, 382, 317, 326 min and `GoTo(Home)` 313, 313, 313, 312 min (`d30 04:08`, `d30 18:02`, ...). Travel
   10.4-14.1 h/day, sleep 4.5-7.0 h, work 3.5-8.2 h; the shift starts 09:00 and the first ClerkWork is `d30 09:42`. This is 12 of the 71 GoTo(Workplace) legs
   at 240 min or more in `isle_s43_d30` (V7: 1 of 72), contra the "distance worked" claim for care, and the leg even shows a vehicle
   (vehicle 4.2 h) so the walk is not on foot alone. Suspected site: `exec/legs.rs` leg planner or the El route for a 5c wage; wage 5c has
   no fare. Fix: report which segments of a 313-min trip are walked and which ride, and cap the commute at hire for a wage that cannot pay the fare.
3. **The super walks an empty round.** `isle_s43_d30/super_15202` 13 rounds, 0 calls; `isle_s44_d90/super_24258` 11 rounds, 0 calls, against 170 / 32
   calls on the other two rounds. The round event fires (`d31 10:07 SuperRound: ... round of 4 Vatra 7 Blocks`) but the tenant call, which V7's
   `service.rs` super stop posts, never lands. Suspected: the tenant is Statistical and the stop skips the call instead of narrating `[off]`
   (V7 did narrate it); or the Blocks are company housing with no tenant. Fix: call on a present tenant or print nothing; never count the round as walked.
4. **The super waits seven hours at the post for a 09:00 shift (V7 bug 6, unchanged).** `isle_s42_d0/super_22721` d2 `02:17-08:29`
   `Rest` 91 min x4; `super_24258` d91 `05:58` and `23:42` `GOAL Work` with "in shift 1.00". Site: the `Work` goal's `in shift` for a Home = post
   job (V7 named it). Fix: shift window only for the post's own hours, a Home post on its own clock.
5. **The 00:00 batch (OPEN-170).** 26 of 26 orderly `treated` lines at 00:00, all Statistical (`[off]`). Site: the clinic row in `service.rs`
   posted by the midnight settle, not by presence. Fix: post treatments when the patient is in the building, or drop the memory for an off-screen patient.
6. **"treated X (a patient)" when the doctor is the patient.** `isle_s44_d90/ripperdoc_17179` d93 10:14 and 11:16 (X = the pick's own name), from
   `GOAL Treat` -> `Therapy`. A label bug in the memory text, harmless to the mechanism. Fix: for self-therapy print `had therapy`.
7. **The Meeting table at 2k (V7 bug 4, not fixed at 2k).** `k2_s42_d26/ripperdoc_1124` 52 hourly lines; island and gen are down to 0-9.
8. **Rest spam on a rest day.** `isle_s42_d0/super_22721` d6 idle 21.5 h, 1-minute `Rest` plans from `d6 09:11`; `isle_s44_d90/teacher_7901` d90 12 x `Rest 90`.
   It is the +1.5 h Idle the gate counts. Calibration unless the rest day has no goal at all: then `Idle` replanning every 6 min is plan churn, a bug.

## 4. FINDINGs (calibration, never a knob)

- Island carer settlements per 8 diaries: d0 144 (V7 110), d30 45 (226), d90 10 (81); only the d90 drop is new and is partly the collapse of
  the rounds (bug 3) and the empty clinic.
- Care idle by role V7 -> V8: ripperdoc 3.7 -> 3.8, orderly 2.9 -> 4.3, teacher 3.8 -> 5.5, super 3.6 -> 7.8 h/day; work 6.5-7.4 -> 6.9-7.3.
- The island commute for care outside the two bugs: `GoTo(Workplace/Clinic/School)` median 48 / 48 / 8 min on d0 / d30 / d90 (V7 44 / 43 / 50),
  gen 25 (37). Home legs 71 / 68 / 29 (V7 89 / 37 / 63).
- Sleep in pieces: `ripperdoc_12962` d0 `00:30 Sleep 54 min`, `01:26 Rest 90`, `04:30 Sleep 62 min` before a 06:15 walk; 5-6 h of broken sleep for a 124-min commute.
- Carer malnutrition at the window end: d0 0.00 on 8, d30 0.00-0.19, d90 0.00-0.42 (V7 0.35 / 0.43 / 0.76). Better, but 3 of 8 d90 carers
  still read 0.30 or more, and nobody is treated for it. This is `OPEN-167`'s clinic path, deferred.
- Doctors per week: 0 present patients on the island, 1 on gen, 0 at 2k. The clinic is a counter and the 00:00 batch is its work.
