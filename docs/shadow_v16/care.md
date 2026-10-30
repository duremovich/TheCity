# Shadow V16 critic: care (ripperdoc, orderly, super, sick, teacher, school_parent)

Window: 100k island, seed 42, d21-27, main at `target/shadow_v16/sha.txt`. Diaries `target/shadow_v16/isle100k_s42_d21/`,
cited `file dDD HH:MM`. Twelve picks, new people against V15 (same archetypes): ripperdoc_10662, ripperdoc_64026,
orderly_16116, orderly_20962, super_106262, super_98765, sick_34254, sick_47721, teacher_14781, teacher_48984,
school_parent_48160, school_parent_87579. Counters: `target/r7_100k/analyze.txt` against `target/r6_100k/`.
**No minor is hurt, in vice, a victim, fined or dead in any of the twelve.** Every event line with an age under 18 was
grepped: only `[family]` lines (School, home) carry a child.

## 1. Verdict

- Better on the people scale than V15, and the stories are the best this group has produced: a 74-y cancer man joins a gang,
  watches an assault and later tries to blackmail the assaulter (he is beaten, wins, goes to the law, is leaned on, the report
  is withdrawn); an orderly sits on a witnessed assault for two days and then shakes the assaulter down; a widow's spouse is
  killed and she swears a grudge within three hours. These start, are picked up and end in a line.
- The clinics are still the weak spot: two of the four care posts are a week of nothing (a day-shift doctor, a night orderly),
  and the city counter says 853 of 1,861 Clinic-days with a Ripperdoc had no patient (46 %).
- Care people are also thieves now: a 61-y ripperdoc and a 50-y teacher each steal motorcycles at night and are jailed
  (bug 2). Grief does not reach the widow's day (bug 1).

## 2. Per archetype (V15 word in brackets)

- **Ripperdoc: alive in one, dead in one (V15 thin / dead).** `ripperdoc_10662` (61 y) treats 3, installs 1, wage 5c a day, then
  steals a Motorcycle at night on d22 23:46, d23 00:02 and d24 04:34, is reported by the owner (`d24 04:49 Report: Gwendolyn
  Novak cried out and Tobin Draper took the report`), arrested at his own desk (`d24 10:44 Arrest: Bex Mbeki arrested Ivo
  Coldwater for Grand Theft`), sentenced (`12:02 6 days`) and attacked in the Precinct by the victim (`18:19 Assault:
  Gwendolyn Novak (48 y) attacked Ivo Coldwater (61 y)`). `ripperdoc_64026` (26 y, `counts: none`): 9.9 h a day of `ClerkWork`
  at Ripperdoc#5835 for six days, net +0 to +5c, no patient. The same post V15 called dead (`ripperdoc_9880`).
- **Orderly: alive in one, dead in one (V15 alive / thin).** `orderly_20962` (67 y, night shift 21:00-06:00 at Ripperdoc#4872):
  `d24 04:27 Treated: Dante Hawthorne patched Mirabel Hallow up at Ripperdoc#4872 for 12c (3c of it to the doctor)`, then the
  blackmail below. `orderly_16116` (63 y): 21:00-06:00 at Ripperdoc#5890 d21-27, 16 `ClerkWork` blocks a night
  (`d23 21:04-05:19`), counts none, zero patients.
- **Super: alive and dull (V15 alive).** `super_106262` (23 y), the best super line yet: `d21 10:48 fixed the wiring in Thea
  Ashcombe's flat`, `d21 12:37 Lena Volkov stopped Pixel Eastwick in the hall of Block#144 about the noise next door`, then
  `d23 09:03 Notice: ... served notice on 1 tenant in arrears (Lena Volkov)`. `super_98765` (41 y) is dead: 41 Served lines in
  a week, every one `walked the stairs and halls of Block#4700 (1 floor)`, no repair, no stopped-by, no tenant named.
- **Sick: alive, both (V15 alive).** `sick_34254` (74 y, cancer 0.31-0.36, homeless): the blackmail thread below, the loan line
  (`d23 00:00 Loan: 240c short of a vat-cancer treatment ... has no friend left to ask`) and five gang days off at Hideout#4563.
  `sick_47721` (25 y): `d21 09:04 Treated: Hollis Oyelaran cut back Perrin Sato's vat-cancer in the back room of Back Room#3079
  for 80c ... (no cure)`, a fare beating (`d22 09:10`), theft, `d25 00:00 Evicted`, a gang try-out refused (`d25 17:16 turned away:
  the treasury could not feed another mouth`), arrest (`d25 18:31`), 3 days. A full downward spiral in one week.
- **Teacher: alive in one, dead in one (V15 alive / thin).** `teacher_14781`: rooms of 29, 16, 23, 23 pupils (`d21 09:00 taught 29
  pupils at School#5500`; V15's best room was 9). `teacher_48984`: `d21 09:00 taught 1 pupil at School#5487`, then no `taught`
  line d22-25 though he works 8.7-8.9 h a day. Dead.
- **School_parent: flat and one tragedy (V15 flat).** `school_parent_48160`: counts none; his 11-y daughter is at home every day
  (`d21-27 09:30 at home (play: no School within 1 h)`, Dorm Storeys in The Vats). `school_parent_87579`: bug 1.

## 3. Round 7's mechanisms seen at 100k

- **W2 the case in the diaries, seen well.** `orderly_20962 d24 17:00 WitnessReacted: saw Solene Duarte's Assault and kept it
  to himself: it is worth something` -> `d26 12:50 Blackmail: ... wants 5c to keep it quiet: Solene Duarte paid 5c`. `sick_34254
  d25 00:13` the same kept silence -> `d26 21:00 Blackmail: Hiro Mbeki had nothing like it and went for him; Hiro Achebe won
  and went to the law` -> `d27 00:09 Coerced: Hiro Mbeki leaned on Hiro Achebe: the report was withdrawn`. The witness
  reaction is now a decision with a reason.
- **W2 a killer named, seen.** `school_parent_87579 d25 21:00 Death: Mako Jessop died of Violence` -> `d26 00:01 GrudgeFormed:
  swore against Slate Yardley (killed kin, 1.00)` -> `d26 04:23 HuntStarted`. It cools within 16 min (bug 1).
- **W4 the right person in the role, seen.** `ripperdoc_10662 d23 12:00 Installed: Volt Ironside had Legs implant T1 installed at
  Ripperdoc#5575 by Ivo Coldwater (6c to the doctor)`, +6c (Treatment) in his purse. No orderly line says Installed. V15 bug 2 fixed.
- **W5 the School fee reads the midnight purse, seen.** `school_parent_87579 d22 09:30 ... the 2c fee ... was beyond the purse
  at midnight (4c in hand, 3c of it kept for the rent and meals)`: it names the purse and the reserve. V15 bug 3 fixed.
- **W6 the door, seen once.** `teacher_14781 d26 19:06 Served: Emeric Dunmore turned Xia Duarte away at the door of Fight Pit#1963
  (high) (turned away 2 times today: high x2)`. One night patch-up at a night clinic (`orderly_20962 d24 04:27`).
- **W3 the fare one dodger at a time.** `sick_47721 d22 09:10 Fare: Juniper Stroud (30 y) beat Perrin Sato (25 y) ... left on
  the platform 30 min`, then `09:40 picked herself up`. One line per dodger.
- **W3 the guard and the arrest, partly.** `ripperdoc_10662 d24 05:55 Pursuit: 2 chases given up (... another guard has the
  chase, out of reach)`, arrest 5 h later at the desk; `teacher_14781 d26 18:25 Pursuit: ... no guard on shift near; the
  warrant stays open` -> `21:06 Arrest`. The warrant waits and is served within hours.
- **Expected, never seen in this group:** W1's `fed last night` / `unfed` line (it fires in camp_parent, neglect_parent, ceo,
  forger; none of these twelve has a hungry child); W6's booked clinic (no `booked` line); the dealer stand; Camp Escape.

## 4. Bugs, ranked by what the player sees first

1. **The widow works the next day.** `school_parent_87579 d25 21:00 Death: Mako Jessop` (her Spouse, affinity +0.51). She
   checks into a Capsule Hotel at 22:27 while her 10-y and 6-y children are at Block#245, the grudge forms at 00:01, `d26 04:23
   HuntStarted` -> `plan: goal Hunt cooled until d26 06:23 (could not plan or plan failed twice)` -> `Wander`, and `d26 06:09 GOAL
   Work` she works 6.1 h as usual. The `[family]` line `d26 09:30` says only `at home (play, no School today)`: no word of
   the death, a funeral, a missed shift or a custody question. New.
2. **A ripperdoc and a teacher steal motorcycles to earn.** `ripperdoc_10662 d24 04:34 PLAN Earn on Motorcycle T1 [StealVehicle
   -> GoTo(Hideout) > Fence]` at wealth 0.09 with a wage job; `teacher_14781 d25 05:36` and `d26 14:38 VehicleStolen`, Arrest
   `d26 21:06`. A 61-y clinic doctor stealing a bike three nights running reads as the goal engine, not a man. Likely cause: the
   Earn goal's crime plan wins at score 0.05 against Idle on a 5c wage. New.
3. **A held man witnesses the street.** `ripperdoc_10662`, sentenced at `d24 12:02`, still logs `d24 19:06 saw 3 commit Fare
   Evasion`, `d25 07:03 saw 5`, `d25 18:25 saw 6` (14 Witness lines after the sentence). A jailed man sees the cell. V15's wedding
   in a cell was not seen; this is its cousin. New.
4. **Blackmail from a sleeper.** `orderly_20962 d26 12:50 Blackmail: ... paid 5c` while the camera line reads `I am asleep at
   Block#261` (`Sleep` 359 min from `11:45`). W5 took sleepers off the witness roll, not off the extortion roll. New.
5. **The soup walk is cut again.** `sick_34254 d21 08:34 SoupLine: set out ... 2 h 58 away (starving, no bin nearer)` ->
   `GoTo ... cut at street (D3 Neon Quarter), 230 min, 172 tiles (ended)` -> `Scavenge`, `13:11 Starving`. A 74-y cancer man walks nearly
   four hours, never reaches the bowl, and first eats at `21:15`. V15 bug 9: **unchanged**.
6. **A one-floor Block is a loop.** `super_98765`: 41 identical `walked the stairs and halls (1 floor)`, no repairs, no calls.
   V15 bug 7 (notices land nowhere): **unchanged**; `super_106262 d23 09:03 Notice: ... Lena Volkov` and no eviction or
   follow-up for her by d27. City `Notice` 3,582 -> 3,671, `Evicted` all corp-side (11.3k).
7. **The empty clinic.** V15 bug 1: **unchanged**. `ripperdoc_64026` 9.9 h a day, no patient; `orderly_16116` 9 h a night, none.
   City: 853 of 1,861 Clinic-days with a Ripperdoc, no patient.
8. **No School within 1 h.** `school_parent_48160`: the 11-y is at home all week. City: `school_unpaid` 50.2k -> 51.3k and
   `school_far` 30.4k -> 31.0k child-days (both worse), `school_days` 51.3k -> 51.5k (flat). V15 bug 3 changed (stale purse ->
   fixed), but the number it hid did not move.
9. **Teacher at zero pupils.** `teacher_48984` `taught 1 pupil` once, nothing d22-25. V15 bug 3/5 (swing 9 against 1): **worse
   at the low end** (1, then none), better at the high end (29).
10. **One first name for two men.** `sick_34254` is Hiro Achebe and blackmails Hiro Mbeki; `Hiro Achebe won` reads as a typo.
    Name pool, low.

V15 care bugs: 1 empty clinic **unchanged**; 2 installer is the orderly **fixed**; 3 stale-purse excuse **fixed**; 4 wedding in
a cell **not seen**; 5 teacher rolls **worse at the low end**; 6 aggressor tips her victim **not seen**; 7 notices
**unchanged**; 8 witness noise **unchanged**; 9 soup walk cut **unchanged**.

## 5. Dull spots, ranked (mechanisms)

1. **The empty clinic.** Mechanism: a clinic with no booking takes walk-ins from the block's sick and the street's wounded
   (`sick_47721` walks to a Back Room for 80c, `d21 09:04`; Ripperdoc#5835 never sees one).
2. **A widow's week.** Mechanism: a death in the household changes the day: a funeral, a missed shift, a neighbour taking the
   children, the service looking in. The `HuntStarted` grudge exists; its cooldown turns it into a wander.
3. **The super of a one-floor block.** Mechanism: a small block's super does repairs for coin from the tenants' purses, or her
   round skips to the nearest block with a notice.
4. **The notice with no consequence.** Mechanism: the noticed tenant on the stairs a day later, the super at the door with a
   guard; `sick_47721 d25 00:00 Evicted ... by Habitat 22` is the corp's, never the super's.
5. **The comfortable school_parent.** `school_parent_48160` is a week of `home`, `Engineer` and a child at home. Mechanism: a
   neighbour or the service asks after a child who is never at School.
6. **The night orderly.** Mechanism: the night clinic takes the wounded from the El and the doors; the patch-up of
   `orderly_20962 d24 04:27` exists once in seven nights.

## 6. Too much

- **Witness lines.** `ripperdoc_10662` 23, `sick_47721` 44 (`d25 06:02 4 saw Perrin Sato commit Theft`), `teacher_14781` 12.
  City `Witness` 207k -> 177k (better), `WitnessReacted` 16.9k -> 18.0k. `orderly_20962 d22 17:53`, `17:54`, `d23 06:09`, `07:17`:
  four `told 3 of his people what X did (the Grand Theft)` in 14 hours. Noise, not broken.
- **Fare Evasion on one platform.** `ripperdoc_10662 d25 18:25 saw 6 commit Fare Evasion this hour`; city `fares short` 170 -> 118,
  so this is local. Watch.
- **Marriage** 23.5k -> 23.3k a month at 100k: unchanged, none in this group's diaries.
- **Arrests** 12.1k -> 11.2k a month (398 -> 367 a day). Three of twelve care people were arrested in the week
  (`ripperdoc_10662`, `teacher_14781`, `sick_47721`); each a theft they set out to commit (bug 2).
- Nothing reads as a broken city. The harsh bits (cancer without a cure, eviction, a fare beating, a gang's court) are each
  causal inside one diary.
