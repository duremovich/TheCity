# Shadow V10 critic: care (ripperdoc, orderly, super, sick, teacher)

Window: main 33f9c55, 100k island, seed 42, d21-27. Diaries `target/shadow_v10/isle100k_s42_d21/`, cited `file dDD HH:MM`.
Ten picks: ripperdoc_7779, ripperdoc_62427, orderly_12853, orderly_20724, super_101327, super_87493, sick_61720, sick_70955, teacher_52153, teacher_53862.
Day 27 is a rest day for every worker here (work 0.0 h in `camera.tsv`), so "d21-26" below is the work week.

## 1. Verdict

- Against V9 the super has gone from a statue to the best story in the group, and the clinic now has patients, but only one of four clinic workers shows one.
- The sick and the teachers are unchanged: a diagnosed cancer climbs 0.39 to 0.49 in a week with no clinic visit, and teachers clerk-work all day with no pupils.
- Lives read as people in a harsh city where the new trades fire (round, patient, notice) and as clerks where they did not; a story starts for the super and one ripperdoc, nobody picks the sick up.

## 2. Per archetype

- **Super: alive (V9 dead).** `super_101327` walks a 6-block round every day (`d21 13:37 SuperRound: Annika Thorne walked the round of 6 Habitat 14 Blocks`, again d22-d26) and serves notice (108 `served notice` lines in the week). The round turns into a feud: `d25 09:17 Notice: served notice on Brick Lockridge`, `d26 08:15 WasRobbed of Brick Lockridge` + `Enemy affinity -1.00` + GRUDGE, `d27 00:17 GOAL Fight ... PLAN Fight on Brick Lockridge`, then three witnesses run, three new enemies and a lost fight (`d27 00:32`). `super_87493` runs the same round (5 blocks, 82 notices) with no follow-on story: notices, then only the rumour of someone's eviction (`d23 00:01 Evicted by corp Greenline 32 on Clemence Kaneda`).
- **Ripperdoc: alive for one, thin for one (V9 thin).** `ripperdoc_7779` (61 y, lawfulness 0.32) treats a patient on d21, d22, d23 (`d21 11:10 Treated: Hesper Voss patched Cerys Kemp up at Ripperdoc#5027 for 12c`), then steals a motorcycle after work (`d21 18:09 PLAN Earn on Motorcycle T1 [GoTo(Vehicle) > StealVehicle > GoTo(Hideout) > Fence]`), kills a bystander in a crash (`d21 20:40 Crash ... Vex Park ... killed`) and fences it to The Drowned Choir for 12c (`d21 20:56`). A doctor-fence story is a real one. `ripperdoc_62427` (34 y, wage 16, lawfulness 0.65): zero patients in 6 work days, no `treated`, no `Treated:`; the day is `GoTo(Clinic) > Rest` then ClerkWork blocks of 79-90 min (`d21 07:57`, `d22 07:57` ...).
- **Orderly: thin (V9 alive/dead).** `orderly_12853` gets two `treated ... (a patient)` memories (`d24 13:20`, `d25 16:14`) and no `Treated:` event, no named case. `orderly_20724` (night shifts 21:00-06:00, wallet 0c) has no patient at all; it scavenges between shifts (`d23 17:40 PLAN Earn [Wander > Scavenge]`) and holds the only tenant memory (`d22 09:27 was called on by Odile Ramirez (as a tenant; my super)`). Nobody here owns a clinic, so V9's quit-unpaid story did not recur.
- **Sick: thin (V9 thin).** `sick_70955` (57 y, no job, diagnosed 0.39) scavenges 3c an hour at night (`d23 02:50 +3c Scavenge`) and the cancer climbs 0.40, 0.42, 0.43, 0.45, 0.46, 0.48, 0.49 across the d21-27 meal lines, no clinic. `sick_61720` (52 y, Vat Tech, 4-7 h of travel a day Mid-Levels to Spire) gets `d24 00:00 Diagnosed: Silas Underhill has a vat-cancer`, then `OFF SICK TODAY` on d24 and d27 and the same shift pattern; no `GoTo(Clinic)`.
- **Teacher: dead (V9 absent).** Both picks do ClerkWork all day at School#6099 (`teacher_52153 d21 09:05 ClerkWork 77 min`, then 65, 89, 31, 36, 52, 47, 77). Wage is `+8c from the City`. The only school line is a pupil's parent met at the door at 09:00 on d24 and d25 (`teacher_53862 d24 09:00`), the same line twice. No lesson, no pupil, no truant.

## 3. The pass's mechanisms seen at 100k

- **Seen: the super's round with tenant calls.** 6 of 6 work days, both supers, about 10 `called on ... (a tenant, on the round)` memories per stop. Counter run: SuperRound 323, Notice 9,469 in 30 days.
- **Seen: clinic patients.** `Treated` 158 (V9) -> 393 in 30 days (`pp100k/analyze.txt`). In the diaries only 1 of 4 clinic workers got `Treated:` events (6 in 7779's week); orderly_12853 got 2 `treated` memories.
- **Seen: reach.** The super's post is the home Block (0-tile commute); V9's 190-min and 79-min commutes are gone. Clinic commutes are 14-36 min.
- **Seen: crime as a doctor's second income.** `ripperdoc_7779` steals and fences; the Earn-by-crime seam firing is the best line in the group.
- **Expected, never seen:** a diagnosed person seeking Treat or a black doc (0 of 2 sick), a patient with a stated cause, a clinic owner under water, protection payers in a care life, a jailed doctor (no `JAILED` in any care diary), any Skipped/Loan/UnderWater line (all zero).

## 4. Bugs, ranked

1. **Diagnosed cancer, no care-seeking** (V9 bug 3: unchanged). `sick_70955` d21-27, `sick_61720` after `d24 00:00 Diagnosed`. `Diagnosed` 490 vs `Treated` 393 city-wide in 30 days says the mechanism fires elsewhere; here wallets are 12c and 14c against a 12c fee, so affordability is the likely gate and nothing offers an alternative.
2. **Clinic is still ClerkWork for most doctors** (V9 bug 2: better). `ripperdoc_62427` sees 0 patients in 6 days while `ripperdoc_7779` (another building) sees one a day; the day shape is the same ClerkWork blocks either way. The patient never appears in the doctor's block, only as an event at 11:10-14:42.
3. **Treatment fee does not reach the doctor.** `Treated ... for 12c` at `ripperdoc_7779 d21 11:10`, but the purse goes 24c (07:33) to 29c (14:51) with only the 5c wage. It reads as a doctor who treats for free; maybe the fee goes to the clinic owner.
4. **521-minute walk home** (`ripperdoc_7779 d21 20:58 GoTo(Home) The Drowned Choir -> Block#272, 521 min, 247 tiles`; the way out was 132 min, 249 tiles). The fence is two districts away, so a thief walks 11 hours for 12c and starts the next shift late (`d22 09:42` against a 09:00 shift). A reach problem, new with the stealing story.
5. **Super serves notice with no stated cause or outcome.** 108 and 82 notices in 6 days (14-18 a day for one person), each line identical, valence 0.00 on all call memories, same hour daily (13:37, 13:42, 13:47, 13:47, 13:47, 14:14). The diary never ties a notice to rent short, noise or a squatter; the only consequence seen is an eviction rumour.
6. **Possibly inverted gossip.** `super_101327 d27 10:04 told Mirabel Stroud: Assaulted by Brick Lockridge on Annika Thorne` after `d27 00:32 Assault: Annika Thorne attacked Brick Lockridge`. The deed names the loser as the attacker. One sample, low confidence.
7. **"Shop unplannable (no feasible action)" twice on a rest day** (`ripperdoc_62427 d27 09:05, 12:31`) with 105c and `can buy 1.00`; the day then goes to 90-min Rest blocks.
8. **Slow short walks** (V9 bug 4: better). `teacher_52153 d21 08:07 GoTo(Workplace) 36 min, 6 tiles` against the 12-min header estimate; `sick_61720` spends 5-7 h travelling on shift days from D4 to a D5 vat farm.
9. **V9 bug 1 (super statue): fixed.** V9 bug 5 (jailed doctor) and bug 6 (owner quits unpaid): not exercised, nobody here is jailed or an owner.

## 5. Dull spots, ranked (mechanisms)

1. **The sick do not ask for help.** A diagnosed person with a wallet under the fee should pick: borrow from a friend (`sick_70955` has Adela Ramirez at +0.58), sell the deck or a part, or go to a black doc (cheap, risky). The cancer climb becomes a decision, and a death arc over weeks follows.
2. **A patient in the doctor's block.** The Treat action runs on the doctor's station block with the patient named and a cause (cut, stims, gunshot). Today `treated Cerys Kemp#48163 (a patient)` carries nothing; the elder-parent visit at `ripperdoc_7779 d21 13:00` shows the clinic can hold a person.
3. **Teaching.** A class is a scheduled block: pupils from nearby Homes attend, a truant is a missed visit, a pupil's parent comes to the door (that line exists, twice). The teacher gets a stand-out pupil or a parent complaint; the wage is `+8c from the City`, so a missed pay or a strike has a hook.
4. **The round resolves.** A notice names its cause (rent short, noise, squatter) and the tenant answers: pays, begs, bribes, threatens. 101327's Brick feud shows the outcome can be a story; the 87493 round had none in a week. Evictions run 9,938 in 30 days, so the loop is half built.
5. **Night orderly sees arrivals.** `orderly_20724` works 21:00-06:00 and sees no wounded. A named gunshot or overdose arrival at 02:00 would feed the gossip this diary already carries.
6. **Doctor's second book.** The stealing ripperdoc proves the seam; give the lawful, well-paid doctor (62427, wage 16) a cash patient off the books so a quiet doctor has an option other than Rest.

## 6. Too much

- **Notices and "called on" memories** read as noise: 14-18 notices a day, about 40 identical zero-valence call memories per round, 6 of 6 days, one time of day. It is monotonous, not harsh. Fold repeated calls into one line ("called on 18 tenants") and keep a full line only for a notice with a cause.
- Nothing reads as broken: no firing every day, no brawl every hour, no jailed doctors. One crash death from a stolen motorbike in the week is the only body.
