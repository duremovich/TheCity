# Shadow V7 critic: care (ripperdoc, orderly, teacher, super)

36 diaries: isle_s42_d0 8, isle_s43_d30 8, isle_s44_d90 8, gen20k_s43_d30 8, k2_s42_d26 4 (2 ripperdoc, 2 orderly, 2 teacher,
2 super in each 20k window; one of each at 2k). `isle_s43_d30_nopin` holds none of the four. Paths are `<window>/<file>` under
`.../worktrees/agent-a996c55984dfa76d3/target/shadow_v7/`. Every pick was Full and pinned (stat and coarse hours 0.0 in all 36
camera rows of my group), so no absence below is read off a Statistical or Coarse stretch. Counterparts are mostly Statistical;
the presence rule narrates those as `[off]`, and I say so where it matters. V6 comparison is the V6 critique plus
`SHADOW_V6.md`; the V6 diaries are gone, so V6 numbers are quoted, not re-read.

Header facts. **Pick drift**: `isle_s43_d30/teacher_19473` (73 y) is hired Accountant at Ripperdoc#4758 on `d35 00:00` and stops
being a teacher; `isle_s44_d90/ripperdoc_21667` is a jailed thief on d90-d93 (Theft, `d90 17:20` Arrest, 3 days) then
`JoinGang` on d94; `isle_s42_d0/ripperdoc_8954` quits his own clinic on d3. The V6 "Packer" and "Super who quits" drift is gone
(all 6 island ripperdocs hold a Ripperdoc job). **Carer health**: malnutrition at the start of each window, island d0 0.00 on all
8 carers, island d30 0.19-0.26 on 4 of 8, island d90 0.35 / 0.43 / 0.76 on 3 of 8, gen d30 0.21 at worst, 2k 0.05 at worst.

## 1. Per archetype

**Ripperdoc: thin, and the shift is now watchable.** The statue is gone. A doctor's day is 4-5 `GoTo(Station)` + `ClerkWork`
pairs of 25-90 min across 2-5 tiles (`work tiles` 0.9-3.3 a day, longest work action median 71-76 min, V6 534-538). What is missing
is what the shift is *for*: a patient. Present patients in my 6 island doctor-weeks: `isle_s43_d30/ripperdoc_23123` 1, `isle_s44_d90/ripperdoc_10655`
1, the other four 0. Gen has one doctor with 10 (`gen20k_s43_d30/ripperdoc_19525`) and one with 0; the 2k doctor 0. No doctor takes a lunch
or smoke break (`Break` 0 in all 9 doctor diaries and in 8 of 9 orderly diaries (one gen night orderly has 3), while all 9 teacher diaries have it). A new
distortion: hourly "Meeting" memories while the doctor works (bug 4).
> `isle_s43_d30/ripperdoc_23123.md` `d30 09:02` `PLAN Work ... [GoTo(Station) > ClerkWork > GoTo(Station) > ClerkWork ...]`, `d30 09:04` ClerkWork 74 min, `d30 10:18` GoTo(Station) 3 min, `d30 10:21` ClerkWork 53 min, `d30 11:14` GoTo(Station) 4 min, `d30 11:18` ClerkWork 75 min ... to `d30 17:35` ClerkWork 25 min. Day total 8.8 h work; the one patient this week is `d33 17:14` `treated Anselm Okafor#22418 (a patient)`.

**Orderly: day orderly thin, night orderly not alive.** The day orderly has the same station shift as the doctor and is the one
place I can see a coworker: `isle_s43_d30/orderly_20499` and `ripperdoc_11008` share Ripperdoc#4867 and meet from `d30 09:09` (new
Acquaintance +0.13, Friend by `d30 12:39`, +1.00 by `d31 11:27`: 0 to best friend in 26 working hours, too fast but a body-to-body
beat). Patients: `isle_s42_d0/orderly_16345` treats 2 on d0 (`d0 11:24`, `d0 17:49`, no `[off]` line) and 1 on d1; the other day orderlies 0.
The night orderlies (`isle_s44_d90/orderly_20546`, `isle_s43_d30/orderly_23398`, `isle_s42_d0/orderly_23128`, gen 14594 and 19016)
work stations all night (`d92 00:05` ClerkWork 37 min, `00:45` 44 min, `01:32` 64 min ...) and their patients are a midnight batch:
> `isle_s44_d90/orderly_20546.md` `d92 00:00` six `[camera] [off] memory: treated ... (a patient)` lines (Mirabel Wozniak, Tank Oyelaran, Hale Voss, Odile Garland, Dorian Upton, Rosalind Okafor, "and 4 more memories in the same minute"); the same 6 at 00:00 on d93, d94, d95, d96 (45 `treated` in 5 nights). All Statistical, none at the door during the shift.

**Teacher: thin, a better-looking day, still no class.** The day is a real shape now: `isle_s42_d0/teacher_12401` d1 `09:04` through
`17:19` is 4 spots, 37-90 min each, and a mid-shift `Break @ street (D5 The Spire), 27 min` at `d1 13:57` (`GoTo(Street) > Break`, a smoke, then
`GoTo(Workplace)` 1 min). 9 of 9 teacher diaries have the break (4-6 a week each). What is still missing is the school: no pupil
is named in 6 island teacher-weeks, and the only parent lines are
`isle_s44_d90/teacher_10442` Juro Yates at the door on d91, d92, d95 `09:00`, and `gen20k_s43_d30/teacher_22577` Maren Ambrose on d32
`09:00`, each posted `SETTLED APART` (the parent is "at Block#4117" while the teacher is at School#5787; bug 2). Teacher
settlements outside that are bar tips (`isle_s42_d0/teacher_12432` d0 `20:18`, d3 `20:19`, d5 `20:19`).
> `isle_s44_d90/teacher_10442.md` `d91 09:00` `[camera] SETTLED APART: memory: met Juro Yates#30900 at the door (a pupil's parent), a regular (valence +0.10); Juro Yates#30900 at Block#4117 (D3 Neon Quarter)`

**Super: alive as a walk on 6 of 9 supers, not alive on 3, and the round is short.** The round is real and presence-checked
(below), but a super is a 2-hour loop then a 6-hour statue. `isle_s43_d30/super_25171`: round of 6 Habitat 7 Blocks on `d30 12:53`,
158 `called on` memories in 7 days (22 a day, V6 18-25 at d30), 4 notices. `isle_s42_d0/super_22452` walks 4 Blocks 09:04-11:02 each day, calls
on 2-7 tenants (V6 d0 super 13-17 a day) and then does `ClerkWork` for 383 min at its own lobby tile (`d1 11:37`). Three supers
have no round at all: `isle_s44_d90/super_16852` (0 `SuperRound`, 0 `called on`, ClerkWork 495-538 min on 1 tile d91-d96),
`gen20k_s43_d30/super_13452` (0 rounds, 538 min on 24 work tiles that are the commute), `gen20k_s43_d30/super_22815` (0 rounds; a
396-min walk to the Block, then 421 min of ClerkWork `d30 10:59`). V6 had 1 of 6. Supers by window: island d0 2 of 2 with a round,
d30 2 of 2, d90 1 of 2, gen 0 of 2, 2k 1 of 1.
> `isle_s42_d0/super_22452.md` `d1 09:04` `GoTo(TargetHome)` 11 min -> Block#1621, `d1 09:32` called on Mateo Greaves#21428 (a tenant, on the round) `[off]`, `d1 11:02` `SuperRound: Ivo Jessop walked the round of 4 Stackwell 4 Blocks`, `d1 11:37` `ClerkWork @ Block#1620, 383 min (ended)`.

## 2. V6 rows and bugs touching the group

| V6 item | verdict | evidence |
|---|---|---|
| Row 1, the statue shift (OPEN-137), ripperdoc / orderly / teacher | **fixed** | longest work action median 71-76 min on 2-5 tiles in every window (ripperdoc 23123 `d30` blocks 25-89 min); V6 534-538 min on one tile. |
| Row 1, super | **unchanged** | `super_22452` d1 ClerkWork 383 min, `super_25171` 231, `super_14472` 276 (median longest 334 min, 36 of 37 working days over 180 min, per the roll-up); the round is 2 h, the rest of the shift one tile. A ~540-min `ClerkWork` is also the whole shift of the 3 no-round supers. |
| Row 2, the idle freeze (OPEN-138) | **fixed** | no care pick-day with an awake still of 12 h or more; the longest awake still I read is a super's ClerkWork (416-538 min). `goal Work cooled until d1 02:42 (could not plan or plan failed twice)` (`super_22452.md` `d1 00:42`), `goal Earn cooled until d5 09:39` (`ripperdoc_8954.md` `d5 05:39`). |
| Row 3, settlement by presence, the super's stop (OPEN-120) | **fixed** | 0 `SETTLED APART ... called on` lines in 6 super diaries; 25 of 49 `called on` lines in `super_22452` carry `[off] ... Statistical (off screen)`, and the call count fell (d0 super 2-7 a day against V6 13-17). The tenant is still a Statistical body drawn "home" (all `[off]`), so the visible beat is a super at a door and nobody to draw; that is OPEN-149. |
| Row 3, the teacher's door (OPEN-120 shape) | **unchanged** | 5 `met X at the door (a pupil's parent)` lines in 2 diaries, all `SETTLED APART` with the parent in another Block (`teacher_10442` d91 `09:00`, `teacher_22577` d32 `09:00`). |
| Row 3, the clinic's patients (OPEN-119) | **unchanged, relabelled** | night orderlies: 6 `[off] treated` at `00:00` each night (`orderly_20546` d92-d96); day doctors: 0-1 a week. Midnight batch of Statistical patients; no body arrives during the shift. |
| Row 4, long legs (OPEN-139) | **unchanged** | GoTo steps over 120 min on 22 of 56 island d0 care pick-days, 19 of 56 d30, 17 of 56 d90, 22 of 56 gen, 11 of 28 at 2k (island 0.54-0.77 a pick-day, gen 0.71, 2k 0.71). 14 steps of 300-483 min in my group: `isle_s42_d0/ripperdoc_8954` `d3 18:01` GoTo(Spot) 373 min, `isle_s44_d90/teacher_21450` `d90 18:10` GoTo(Home) 482 min, `isle_s42_d0/orderly_16345` `d1 23:59` GoTo(Home) 483 min. Each prints as one action; I cannot see a segment boundary in any of them. The island is no worse than gen or the 2k control on this row, so the pass did not make the walk worse for care. |
| Row 5, housing churn (OPEN-140) | **fixed** | no double `Housed` at one tick, no `NoSuchPlace`, no Soup Kitchen or Capsule line in 36 care diaries. The day-0 retiree no longer lands on the street (bug 6.1 below for the new problem). `teacher_19473` `d35 00:00` "moved nearer work" is not nearer: the commute reads 109 min before (`d33 07:33`) and 119 min after (`d35 07:33`), though the job changed in the same minute (Accountant at Ripperdoc#4758), so I rate it **not observable**. |
| Row 6, dead trades | **not observable** | none of my four. |
| Row 7, the implant loop (OPEN-142) | **not observable** | no `BuyAsset` retry loop in my group. `teacher_12401` plans `Shop > BuyAsset` on d0 `00:02`, d1 `03:13`, d2 `02:28` and each is interrupted by the commute (`d0 03:41`, `d1 05:11`, `d2 04:41`), never failing; `ripperdoc_8954` buys a Deck at `d0 02:08` cleanly. |
| V6 bug 7, the tied super's lobby wait | **unchanged** | `super_22452` d1 `02:50`, `04:23`, `05:56`, `07:29` `Rest` 91 min each (6 h in Block#1620), and `d1 00:14` `GOAL Work ... in shift 1.00` at midnight for a 09:00 shift. |
| V6 bug 6, a super with no round | **worse** | 3 of 9 supers (V6 1 of 6): `super_16852`, `super_13452`, `super_22815` (above). `[presence] round_reach_tiles = 20` (OPEN-154) is my likely cause: a Block with no other Block within 20 tiles of its door has an empty round. |
| V6 bug 5, the notice that repeats three days running | **changed** | the same tenant is served again 3-5 days later, never on 3 consecutive days: `super_25171` Ghost Fairweather `d31 10:01` and `d36 10:01`; 2k `super_2246` Piers Kowalczyk and Viktor Adeyemi `d26 13:51` and `d29 12:07`, Kofi Brandt `d29 11:30` and `d32 11:30`. Still no eviction or payment line in any window. |
| V6 bug 3, midnight batch | see Row 3 | unchanged, now `[off]`. |
| V6 bug 8, pick drift to non-doctors | **fixed** | all 6 island ripperdocs hold the job; the drift now is a doctor who is a thief (`ripperdoc_21667`) and a teacher promoted off the post (`teacher_19473`). |
| V6 bug 9, pre-shift Rest at the workplace | **fixed** | Rest of 14-22 min before the first block (`ripperdoc_23123` `d30 08:46` Rest 14 min; `teacher_12401` `d1 08:38` Rest 22 min), V6 28-54 min. |
| V6 bug 13, one contact a day at the same minute | **changed** | the super's calls now vary (09:32, 10:32, 11:02, `super_22452`); the teacher door is still 09:00 sharp (`teacher_10442` d91, d92, d95). |
| V6 bug 20, the tool's labels | **fixed** (held) | `treated X (a patient)`, `called on X (a tenant, on the round)`, `met X at the door (a pupil's parent)` all present; `[off]` is new and used correctly on every Statistical counterpart I read. |
| V6 `OPEN-135`, no pupils seeded | **unchanged** | no child is named in any teacher diary. |

## 3. The camera: the three worst continuity moments

**3.1 `isle_s42_d0/orderly_16345` `d1 18:55` to `d2 13:38`, the orderly who misses his shift on the way home from a visit.**
At `d1 18:55` `GOAL Visit` (an elder parent) starts `GoTo(Victim)` Block#281 (D5 The Spire) -> Block#1594 (D4 Mid-Levels), 211 min. He pays a
`Fare -1c` at `d1 20:30`, reaches the Block at `d1 22:26`, rests 91 min, and at `d1 23:59` starts `GoTo(Home)` of **483 min**. `d2 00:00 now
at street (D4 Mid-Levels)`, `d2 06:32 now at street (D5 The Spire)`, `d2 08:02 now at Block#302`, then `Sleep` 241 min, `d2 12:05` Eat,
`d2 12:25` a 69-min `GoTo(Workplace)`; first `ClerkWork` is `d2 13:38`. A viewer follows a man who walks the night streets for 8
hours with no stops, goes to bed at 08:02 and starts a 09:00 shift at 13:38 (`d2` work 4.5 h, sleep 3.2 h on d1). The camera columns
read `d1 sleep 3.2h travel 5.6h`, `awake still 90m idle`, `jumps 0`: the tool sees nothing wrong, because the walk is legal and slow.

**3.2 `isle_s42_d0/super_22452` `d1 02:44` to `d1 11:37`, the super who waits nine hours for a 2-hour round and then stands.**
`d1 00:14` `PLAN FAILED: Work failed at EatAtHome: PreconditionLost`, `d1 02:44` `PLAN Work [PackMeal > GoTo(Workplace) > Rest]`, `d1 02:50`,
`04:23`, `05:56`, `07:29` `Rest @ Block#1620, 91 min` each, `d1 09:04` first `GoTo(TargetHome)` 11 min. The round runs to `d1 11:02`
(5 tenants), then `GoTo(Workplace) 35 min` and `d1 11:37 ClerkWork @ Block#1620, 383 min (ended)`: 5 h 23 min on one tile in the
lobby. Repeated on d2 and d3 (`ClerkWork` 383 and 373 min). The tool's own camera line: `d1 idle 9.9 h`, `work 7.7 h`, `awake still 11:38 ... 383m work`.
A viewer sees a super asleep on his feet in a hallway 20 h out of 24 with a 5-tenant visit in the middle.

**3.3 `isle_s44_d90/super_23456` `d90 10:36` to `d91 07:32`, the starving super.** Wallet 0c, malnutrition 0.76 and "OFF SICK TODAY" in
the header; wage 6c a day. `d90 10:36` `GoTo(Spot)` 292 min to a street HangOut, `d90 16:31` `PLAN Earn [Beg]` (`Beg` 30 min and 23 min), `d90 17:26`
`PLAN Eat [GoTo(Bin) > DumpsterDive]`, `GoTo(Bin)` 120 min, `d90 19:26` `GoTo(Home)` 169 min, home at `d90 22:15`. d90: `work 0.0`, `travel 9.7 h`.
`d91 07:32` `meal ate scraps from a BIN | malnutrition 0.77 | toxicity 0.81 | sick days 3/14`, and he is back on shift at
`d91 07:34` (`GOAL Work score 0.957`) for 7.9 h. No clinic is reached on any day, though Ripperdoc#4937 (D1 The Sump, where `orderly_20546` works) is in his district. A viewer sees a starving, sick civil servant walk 9.7 hours to eat from a bin and
go to work; nobody treats him. This is what a followed patient sees of the island's 2,188 sick (section 5).

## 4. The first week (`isle_s42_d0`)

- **The retiree now ends up at family, not on the street.** `super_15263` (63 y) walks a round on d0 (`d0 11:40` SuperRound, 15 settlements),
  is `Retired ... (to family)` and `Housed ... moved in with family at Block#3618` at **`d1 00:00`** (V6: d0 00:00 on the first
  tick), and `d1 00:01` `GoTo(Home) ... Block#3618, 409 min (done)`, in at `d1 06:50`. V6's bug 6.1 (never reaches the Home) is fixed. The cost is a 6.8-hour night walk at 63, and what
  follows: `d4 12:05` Assault, `d5 00:00` RentShort on Block#3618, `d5 06:23` Scavenge. Alive as a life, bleak as it is meant to be.
- **The clinic-owner doctor goes bust in 3 days.** `ripperdoc_8954` (65 y, Wallet 300c, owns Ripperdoc#5344) spends d0 in Bar#4444 in D2
  Civic Core (8.2 h building, 0.0 work), buys a Deck T1 (`d0 02:08`, 53c), pays `Parts -219c` at `d1 00:00` (purse 14c), pays his
  orderly out of his own purse (`d1 17:05` Wage -9c, purse 0c), `d1 18:00` memory Unpaid, `d3 17:05` Wage -19c, `d3 18:00` `Quit: Tristan Abernathy quit as Ripperdoc (unpaid)`,
  `d4 00:00` RentShort. Work days 3 of 7; d5 street 21.1 h. A first-week viewer follows a doctor for 3 days and then a man on the street.
- **Teachers: a school with no children.** `teacher_12401` and `teacher_12432` work stations and a smoke break (d1 `13:57`) from d0; no pupil or parent in 7 days, only bar tips (`d0 20:18`).
  `teacher_12401` plans a `Shop` trip at 00:02/03:13/02:28 each night and turns back to the commute (3 of 3 nights), its `GoTo(Workplace)` of 307 min on d0 (`d0 03:41`), 207 on d1, 221 on d2.
- **Statistical neighbours.** All 8 d0 care picks show `settles apart 0`, `stat` 0-32; the doctor-owner has 11 asleep-apart wage and Finance lines at 05:32 and 00:00 (`ripperdoc_8954` `d1 05:32`, `d6 00:00`), flows not encounters.
- **Day-0 clinic crowd.** `orderly_16345` treats 2 on d0 and 1 on d1 on the island at d0, more than the d30/d90 day doctors (0-1 a week): the Spire clinic has patients on day 0.
- The d0 night orderly `orderly_23128` treats 12 over the week, all at `00:00` (`d3`, `d4`, `d5`, `d6`).

## 5. The island against gen, and against the 2k control

- **Same shape, not the same city.** Station shifts, the smoke break for teachers, `[off]` narration and the labels read the same on the island, on gen and at 2k.
- **Patients.** Doctors treat 2 in 6 island weeks, 10 in 2 gen weeks, 0 at 2k; night orderlies 6 a night on the island at d90 (45 in 5 nights), 4-6 on gen at d30, 2-6 on the island at d30. The island is neither busier nor emptier than gen. 2k has no night orderly in my picks.
- **Supers.** Round on 5 of 6 island supers, 0 of 2 gen, 1 of 1 at 2k. The no-round supers are on the island (d90) and gen: 3 of 9 against V6's 1 of 6 (all 20k). Island d30 supers call 22-30 tenants a day; the 2k super `super_2246` 34 settlements and 7 notices in a week.
- **Commute.** The median `GoTo(Workplace)` is 9-79 min on the island, 10-20 min on gen (orderly, teacher) and 188 min for the 2k orderly (`k2_s42_d26/orderly_1401` `d26 04:37`, 216 min; `d28 04:32`, `d29 04:32`). The 2k control is not free of long legs: 11 of 28 care pick-days have a GoTo over 120 min, as on the island.
- **The island's 2,188 sick (d90) against gen's 151 (d30), and what a carer or patient sees.** The `sick` pick is "malnutrition >= 0.5 or a cancer" (`citysim-cli/src/shadow.rs:261`), so at d90 the island's sick are hunger-sick. In my window: three of eight island d90 carers are malnourished (23456 at 0.76 with 0c, 23169 at 0.43 `d93 05:24` sick days 1, 16852 at 0.35), where gen d30 has none above 0.21 and island d0 has none. The carer's side of it: a doctor or orderly with a paid shift cannot eat (5-16 c a day wage, wallet 8-14c on d90, `orderly_23169` 9c). The patient's side: no sick pick and no care pick in any window ever treats or is treated for malnutrition: the only `treated` memories are the 45 night-batch Statistical ones and the 3 present ones. Every island d90 diary I read of a malnourished person (`super_23456`, `sick_22413`, `sick_9818`) goes to a bin, begs or works; none walks into a Clinic. A follower sees an island whose ill are working and starving, a Clinic that is a counter, and a carer who is one of the ill.
- **Island d0 to d90 is where the carer degrades**: mean carer malnutrition 0.00 (d0), 0.12 (d30), 0.25 (d90) against gen d30 0.06 and 2k 0.02. Which is a food-and-wage story on the island, not a care-trade story; I flag it because the camera will follow these people.

## 6. New bugs, ranked by what a followed character sees first

1. **A carer starves on the wage and no one treats it (island d90; 3 of 8 care picks).** `isle_s44_d90/super_23456` malnutrition 0.76, wallet 0c, off sick on d90, and on shift the next morning; the doctors treat 0-1 present patients a week. Likely anchors: the wage tables (`[[trades]]` super 6c, orderly 5c, teacher 6c a day against island food prices) and `Treat`/patient intake, which only exists as a `ClerkWork` memory stamp (`service.rs` encounter for the clinic row). Not a care-trade fix, a pressure/food fix for OPEN-136; the clinic's part is there is no care action to walk in to.
2. **The teacher's door encounter is still posted apart (5 lines, 2 of 8 20k teacher-weeks at 20k).** `isle_s44_d90/teacher_10442` `d91 09:00`, `d92 09:00`, `d95 09:00` `SETTLED APART` with the parent in another Block. Phase 4 covered the super's stop but not the teacher's `Served`/door posting. Likely anchor: the school's `counter`/`encounter_post` path in `citysim/src/systems/service.rs` (the `teacher` row's pupil-parent serve), not routed through `presence::body_at`.
3. **A super with no round stands on one tile for the whole shift (3 of 9 supers; V6 1 of 6).** `isle_s44_d90/super_16852`, `gen20k_s43_d30/super_13452`, `super_22815`: 0 `SuperRound`, 0 `called on`, 495-538 min `ClerkWork`. Likely anchor: the round pool after `[presence] round_reach_tiles = 20` (OPEN-154), `trades::new_round` / `round_for`: an empty reach list should fall back to the nearest Blocks or raise the reach, not return no round.
4. **A doctor and a teacher "sit at the owner's table" every hour of the shift (3 diaries).** `isle_s43_d30/ripperdoc_23123` has 47 `sat at Kenji Ellery#14282's table (a Meeting), a regular (valence +0.10)` lines in 7 days (hourly 09:00-16:00; `d30 09:00`, `10:00`, `11:00` ...), `gen20k_s43_d30/ripperdoc_19525` 50 (Wisp Stroud), `isle_s43_d30/teacher_19473` 14. The counterpart is the corp's Statistical exec, off screen (`[off]`); the pick is ClerkWorking at the clinic or school, not at a table. Likely anchor: `meeting_hour` in `citysim/src/systems/service.rs:954`: the Statistical exec counts as present and the clinic is the corp's Hq (`hq_of`), so each hour posts an `Attendee` encounter to the staff in the building.
5. **The 8-hour night walk home from a visit (the 211-min and 483-min legs).** `isle_s42_d0/orderly_16345` `d1 23:59` (3.1). Likely anchor: `Visit` goal at `18:55` (the `Visit` goal scoring "not on shift 1.00" with no regard for the 3.5-hour walk) and the unsegmented long leg (`exec/legs.rs`, `leg_replan_tiles`): 14 care GoTo steps of 300-483 min remain, none prints a segment boundary.
6. **The tied super's lobby wait and statue (3.2).** `isle_s42_d0/super_22452` `d1 02:50-09:04` Rest x4 and ClerkWork 383 min. Likely anchor: the `shape = "round"` super plan: after the round it falls back to `ClerkWork` at the home-post tile; the shift's remaining 6 h should be stations or a second round. Also the `Work` goal's `in shift` score is 1.00 at 00:14 for a 09:00-18:00 post (a Home = job post reads as always on shift).
7. **A clinic owner buys parts the purse cannot carry (3 days to bankruptcy).** `isle_s42_d0/ripperdoc_8954` `d1 00:00` `Parts -219c` against a 247c purse, then pays the orderly out of pocket to 0c. Likely anchor: the owner's parts purchase (the Parts flow at the midnight settle) with no floor for the wage week; `Quit ... (unpaid)` is the consequence.
8. **A coworker pair goes from Acquaintance to Friend (+1.00) in 26 working hours.** `isle_s43_d30/orderly_20499` and `ripperdoc_11008`, `d30 09:09` +0.13, `12:39` Friend, `d31 11:27` +1.00, every 2 hours +0.15. Printed, not tuned: a calibration FINDING (doctrine 1), not a bug; I flag it because the pair is the one real body-to-body beat in the group.
9. **Doctors and day orderlies never take the P7 break.** `Break` 0 in 9 doctor diaries and 8 of 9 orderlies, 4-6 a week in every teacher diary. Likely anchor: `[shift.roles] Ripperdoc` is `stations` (config 4274) but the break draw or its `break_walk_max` / hungry branch skips the Clinic's shape; check `shift.rs` for the roles that carry a break.
