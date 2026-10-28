# Shadow V14 critic: care (ripperdoc, orderly, super, sick, teacher, school_parent)

Window: 100k island, seed 42, d21-27, main f5893b7. Diaries `target/shadow_v14/isle100k_s42_d21/`, cited `file dDD HH:MM`.
Twelve picks: ripperdoc_11838, ripperdoc_76252, orderly_14910, orderly_21739, super_100246, super_124285, sick_49045,
sick_75763, teacher_15873, teacher_48984, school_parent_50346, school_parent_92238. 30-day counters
(`target/r5_100k/analyze.txt`) did not exist; skipped. **No minor is hurt, in vice, a victim or dead in any of the twelve**
(grep of every event line for ages under 18: none; minors appear only in `[family]` lines at School).

## 1. Verdict

- The clinic is the win: V13's dead ripperdoc and midnight-batch orderly are now a working day at two of four posts
  (17 installs and 8 treatments in a week for one doctor, scenes at 09:00-17:00 with the cut to the doctor).
- The other two clinic picks are still empty: one ripperdoc is struck off and moves to a Vat Farm, one orderly has zero
  events in 7 days. The mechanism is real but patchy at 100k.
- Teachers now meet empty rooms (1-2 pupils, "no pupil on the roll"); sick, super and school_parent are V13 again, alive
  but with the same unpicked threads (notices that never land, a friends' ask that finds no one).

## 2. Per archetype

- **Ripperdoc: alive in one, dead in one (V13 dead).** `ripperdoc_76252 d21 10:00 Treated: Yuki Bellamy had the blood
  washed at Ripperdoc#5008 by Fenn Stroud (8c, 2c of it to the doctor)`, then `d21 11:00 / 12:00 / 14:00 / 14:00 / 17:00
  Installed ... Legs implant T1`; 37 events in the week, and the doctor buys her own leg on finance (`d23 11:26 -45c`).
  `ripperdoc_11838`: 1 install in 7 days (`d25 12:04`), then `d26 00:07 Quit: ... quit as Ripperdoc (struck off)` and
  `d26 08:13 taken on as Vat Tech`; before that 8.5 h of ClerkWork a day at #4838 with 2.5-7.5 h idle.
- **Orderly: alive in one, dead in one (V13 thin).** `orderly_21739 d21 11:00 Treated: Nadia Fletcher patched Odessa
  Oakes up at Ripperdoc#4933 for 12c (3c of it to the doctor)`, 9 Treated plus `d24 10:00 Cured: Silas Underhill was
  cured of a vat-cancer ... 246c on the doctor's slate`. `orderly_14910`: `counts: none`, 8.8 h "work" daily at
  Ripperdoc#5622 (`d22-d26 DAY SUMMARY work 8.8h`) and nothing logged, net 0 to +6c.
- **Super: alive (V13 alive).** `super_100246 d21 10:46 Notice: Cutter Singh served notice on 3 tenants in arrears on the
  round of 6 Blocks`, then 29 Served (`d21 13:47 fixed a leak in Raven Ito's flat`, `d25 15:55 fixed a jammed door in
  Roderick Kettering's flat`). `super_124285`: 40 Served, `d21 09:06 SuperRound ... walked Block#107640 door to door, the
  only Nutrix 40 Block near`. Notices 3, 9, 4, 8 a day; still no follow-up.
- **Sick: alive (V13 alive).** `sick_75763` (71 y, cancer 0.27): `d22 00:00 Loan: ... 240c short ... has no friend left to
  ask`, `d26 00:00 Evicted ... by Habitat 18 (arrears)`, `d26 03:57 Crew: tried out for The Salt Widows ... turned away:
  the treasury could not feed another mouth`, `d27 02:30 SquatEvicted ... (taken by The Static Brothers)`. A real slide.
  `sick_49045` (73 y, homeless gang member): `d26 00:07 Crew: stays off The Salt Ghosts's trade today: the nearest spot ...
  is 16 h 53 from home`, `d26 19:53 stood watch ... (unpaid: the gang is broke)`.
- **Teacher: thin (V13 alive).** `teacher_48984 d21 09:00 taught 2 pupils`, then 2, 2, 1; he also steals a truck in the
  middle of the job (`d22 02:40 PLAN Earn on Truck T1`, 107 min GoTo to the Sump, BuildingFull), is reported `d26 09:19`,
  arrested `14:19`, `Sentence ... 6 days`: a real story and the best of the group. `teacher_15873 d22 00:08 Served: No
  pupil on the roll at School#5749 today: no class for Lux Penhallow, Odile Oyelaran (no shift, no wage)`; one class of 1
  pupil in the week (`d24 09:00`).
- **School_parent: alive but flat (V13 alive).** `school_parent_92238 d21 09:30 [family] Zoe (11 y) ... Elspeth (10 y) at
  School#5404 (3 min walk); the pantry holds 29`, both in School every day, +2 to +6c net, 6-7 h leisure.
  `school_parent_50346` (70 y, one 17-y son): `30 min walk`, pantry 28 to 27, 9.7 h at the Vat Farm. No fee excuse, no
  stress: comfortable.

## 3. Round 5's mechanisms seen at 100k

- **Seen U6 (the clinic's day):** patients to the doctor's body (`orderly_21739`, `ripperdoc_76252` Treated at 09:00-17:00,
  not 00:00), the blood wash (`ripperdoc_76252 d21 10:00, d22 16:00, d24 09:00 / 11:00`, 8c with 2c to the doctor), chrome
  booked by the hour (Installed at 11:00, 12:00, 14:00, 17:00), the doctor's slate on a cure.
- **Seen U7:** `Served: ... (no shift, no wage)` for the empty class. No heir or strip lines in this group.
- **Seen U1 partly:** a School within reach of both school_parents (`3 min walk`, `30 min walk`; V13 had `57 min`). No
  child turned away full. But the rolls on the teachers' side are 1-2 (bug 3).
- **Not seen:** U2 (no warning, take or Camp visit in any of the twelve), U4 (no indoor-witness line that mattered).
- **Expected, never seen:** U3 (a bowl, a soup line, a kitchen) for either sick pick with `has no friend left to ask` and
  0c; both steal food instead (`sick_75763 d23 04:04, d25 06:37`; `sick_49045 d26 05:11, d27 09:24`). A super-served
  eviction (`sick_75763 d26 00:00 Evicted` is Habitat 18, not the super).

## 4. Bugs, ranked by what the player sees first

1. **An orderly whose whole shift is nothing.** `orderly_14910` 8.8 h work a day, `counts: none`, no GOAL Work line shows a
   patient at #5622. Likely this clinic has no doctor on duty or gets no booked patients while #4933 does. V13 bug 1
   (ripperdocs see no patients): **better** (2 of 4 clinics work), the other two unchanged.
2. **A struck-off ripperdoc keeps ClerkWorking the post until midnight.** `ripperdoc_11838 d21-25` 8.5 h at #4838 (`d25
   09:21 ClerkWork 68 min`, `10:33 56 min`, `11:33 40 min`) with 1 patient in 5 days, then `d26 00:07 Quit ... (struck
   off)`. Why struck off is never told; the player sees a doctor sit in an empty waiting room and then vanish. V13 bug 2
   (rest at the post five hours before the shift): **fixed** (`d25 06:48 GOAL Work ... in shift 1.00` is the real start).
3. **A teacher with no children at 100k.** `teacher_15873` 8.5 h/day work with 1 pupil once in 7 days and `No pupil on the
   roll`; `teacher_48984` 2, 2, 2, 1. Schools exist now (U1) but the rolls did not follow. V13 bug 7 (always exactly 30):
   **fixed**, to the opposite extreme. Likely the seeded schools opened ahead of enrolment, or the 2c fee still empties them.
4. **Old sick people get no help at 0c.** `sick_75763` (71 y): `has no friend left to ask` x3, evicted d26, turned away by a
   gang d26, squat-evicted d27; nothing offers a bowl or a bed (U3 not seen). `sick_49045` (73 y) is sent on a 16 h 53 walk
   to a trade corner.
5. **Notices never land.** `super_100246` 3+9+4+8 a day; `sick_75763 d22 09:45` a notice on 16 tenants; the eviction is
   Habitat 18's, not the super's. **Unchanged** from V13.
6. **Witness lines are noise.** `ripperdoc_11838 d23 19:12, d27 15:23, d27 19:41` Witness plus "told 3 of his people", no
   consequence. **Unchanged.**

V13 care bugs: 1 **better**; 2 **fixed**; 3 (sick worker foreman of her replacement) **not seen** (the sick picks have no
job); 4 (neglect text contradiction) **not seen**; 5 (fee beyond the purse with a full pantry) **fixed** for the two picks
here (both comfortable); 6 (4 h sweeper commute) **not seen**; 7 **fixed**.

## 5. Dull spots, ranked (mechanisms)

1. **An empty classroom.** A teacher ClerkWorks 8.5 h with 1-2 pupils. Mechanism: a truant officer or a parent at the
   door, a make-up class when the roll is thin, a teacher who tutors the neighbours' children for coin.
2. **The orderly's empty clinic.** Mechanism: with no patient booked, the orderly touts the block for a regular, or the
   clinic takes a wounded rescue from the street (the doctor's body first is already built).
3. **The friends' ask finds nobody.** Mechanism: a priest, a pawn offer, or a gang front that fronts the treatment against
   a debt; a rumour of a back-room cut.
4. **The comfortable school_parent.** `school_parent_92238` seven flat days. Mechanism: the teacher calling about a
   truant, a child falling sick, a fee that rises: a thread to pick up.
5. **The super's notice** with no follow-up: meet the noticed tenant on the stairs, carry out the eviction with the super
   at the door.

## 6. Too much

- Nothing reads broken-harsh. `sick_75763` (71 y) evicted, turned away and squat-evicted inside 24 h is bleak but a story.
- `ripperdoc_76252` at 37 events a week is the opposite extreme (17 Installed, one of them her own on finance): watch that
  it does not read as a chrome factory. Fine for now.
- Witness/WitnessReacted repeats the same "told 3 of his people" line: monotone.
