# Shadow V14, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent, neglect_parent, camp_jobless)

Window: 100k island, seed 42, d21-27, 22 diaries (`target/shadow_v14/isle100k_s42_d21/`, main f5893b7). Cites are `file dDD HH:MM`.
Read: every DAY SUMMARY, the event lines of all 22, every `[family]` line of the parent picks, both wardens' Custody lines, d21/d23 of
`vendor_9648`, d21 of `miserable_12664`. `target/r5_100k/analyze.txt` was not there (only run.csv/events.tsv), so no counters. Different people from V13.

## 1. Verdict

Better than V13 where the round aimed: the neglect ladder takes a child with a reason ("unfed 3 days again, inside the warning of d18"), the family line is right
after a take, the starving reach a pot (a dry one is told), and the camp headcount reconciles. Same shape otherwise: harsh lives that start a thread
(a child taken, a slip-out, a rescue) and drop it. **Two toddlers (2 y) are out of care and on the street for days**, the one thing here that reads
broken against the child rule. No under-18 appears as victim, in vice or dead in any of the 22 (the only under-18 lines are `[family]` and Custody).

## 2. Per archetype (V13 word -> V14 word)

- **Homeless: alive/thin -> thin.** `homeless_91465` (45 y): `d21 08:44` Starving, SoupLine 0 h 44 away, `d26 05:43` "the pot at Soup Kitchen#1605 ran dry before
  Dagny Duarte's turn", `d26 14:01` turned away by The Salt Widows "the treasury could not feed another mouth". `homeless_71747` (28 y): `d25 05:12` SquatEvicted,
  `d25 10:00` turned away by The Red Ghosts. V13's rescue arc is not repeated.
- **Sick: alive/thin -> thin.** `sick_49045` (73 y): `d23 00:00` and `d26 00:00` the same Loan line "240c short ... has no friend left to ask", court summons missed twice,
  `d26 00:07` "stays off the trade today: the corner is 16 h 53 from home", `d26 19:53` "stood watch ... (unpaid: the gang is broke)". `sick_75763` (71 y): `d22 09:45` Notice,
  `d26 00:00` Evicted, `d27 02:30` SquatEvicted. A clean slide, no treatment either way.
- **Elder: dead -> alive (one).** `elder_58799` (60 y): DawnPick sent home `d22 06:00`, "Aldo Underhill told Rin Chrome of a Sanitation post" `d23 07:00`, "the post was gone"
  `d23 09:15`, then work 4.2 h `d25` and 5.9 h `d26`, after three days of 7-13 h travel. `elder_78977` (70 y): thin, then `d26 13:08` marries, `14:27` steals, `14:38` arrested.
- **Vendor: alive(money) -> thin/odd.** Both picks print `Job: none`. `vendor_20832`: net -10/-7/+2/-11c, no event all week. `vendor_9648`: travel 5-13 h a day, and bug 2.
- **Street worker: thin -> thin.** `street_worker_81797`: leisure 5.5-8.1 h and idle 3.7-6.6 h daily, travel 0.0 h, zero events. `street_worker_6698` (55 y): Legs implant
  `d21 18:51`, Grand Theft `d25 11:44`, 6 days `d25 17:19`, then two 16 h jail days.
- **Sweeper: thin -> thin.** `sweeper_61654` (183-tile commute): travel 9.1/7.3/10.8/12.0 h `d21`-`d25`, work 2.3-6.8 h; `d26 08:00` "Skipped ... walks the streets instead
  (mood -0.46, misery 0.90)". `sweeper_46469` (118 tiles): travel 3.0-5.2 h for 5.5-6.8 h of work.
- **Camp warden: alive/thin -> alive.** Counts 109, 194, 246, 297, 347, 395; fence catches `camp_warden_56621 d22 10:00` (Adela Yardley, 11 y), `d22 16:00` (Nerys Delacroix, 1 y),
  `d26 14:00` (Volt Reyes, 1 y); visits let in `d23 14:00`, `d25 10:00`, `d26 12:00`, `d26 17:00`; "28" then "54 of the meals from the Reserve".
- **Miserable: alive -> alive.** `miserable_12664` (48 y): `d21 14:12` Abduct, `14:18` "went down, bleeding" (48 y vs 55 y), rescued `14:39`, carried to a ripperdoc `17:25`, then
  `d22 00:11` hired as Ripperdoc and six installs `d22 11:00`-`13:00` (bug 5). `miserable_80388` (43 y): six job boards, theft `d24 15:19`, arrest `21:38`, 3 days, Release `d27 22:46`.
- **Camp parent: thin -> thin.** `camp_parent_17162` (62 y): SoupLine 3 h 19, theft, Fare beating `d22 00:45`, 3 days, released `d25 15:44`, Fare beating again `d25 17:43`, GangJoin
  `d25 21:15` (mood -0.76). `camp_parent_102044` (36 y, children 15/9/2): three thefts in a week and no event about the children (bug 4).
- **Neglect parent: new, alive.** `neglect_parent_85228`: `d21 00:10` ChildTaken (Cass, 10 y), GrudgeFormed against the Law, then SoupLine x5 for the father. `neglect_parent_60512`:
  "the 2c fee at School#5484 is beyond the purse" five mornings while the father holds 17 food (his pantry) and 0c.
- **Camp jobless: new, thin.** `camp_jobless_11156` (18 y, 0-y child in camp): `d22 00:07` "could not make the trip: 17 h 43 from home", `d25 11:39` SquatEvicted. `camp_jobless_61284`: bug 1.

## 3. Round 5's mechanisms seen at 100k

Fired:
- **U2 take on a second warning:** `neglect_parent_85228 d21 00:10`; `miserable_12664`'s 2-y daughter (warning since d13) is in Work Camp#5936 by `d25 09:30`.
  **`[family]` reads the Camp after a take:** `neglect_parent_85228 d21 09:30` "Cass Fairweather (10 y) in Work Camp#5132". V13 bug 1 (stale line): **fixed**.
- **U3:** `neglect_parent_85228 d22 04:45` "waits in the soup line ... the hour's 10 bowls are out, the next at 05:00"; `homeless_91465 d26 05:43` dry pot. 4 waits, 2 dry-pot lines in the group.
- **Warden round** as V13 plus a count that reconciles from d23 (109+64=173 against 194 on d22: +21 unexplained).
- **U6 clinic day:** `miserable_12664 d22 11:00`-`13:00` six installs by the new ripperdoc in the morning, not at 00:00.
Expected, not seen: `MissionServed` (0 in the group); a soup line "opening where the starving are" (0 such lines; every SoupLine is 0 h 44 to 3 h 19 away); a camp visit that succeeds
(0 of 4 parent visits: 17 h 43 and 20 h 05 from home); a "service at the door" arrival line (the take has none, as in V13); a School founded in the Sump (bug 3).

## 4. Bugs, ranked by what the player sees first

1. **Toddlers on the street.** `camp_jobless_61284 d24 09:00` (`camp_warden_56621`) "1 slipped out (Hollis Adeyemi)", a 2 y; `d24 01:12`-`03:18` he is at his father's home, `d27 09:30`
   family line "Hollis Adeyemi (2 y) on the street" while the father holds 0c, hunger 0.66, and wanders. `miserable_12664 d21 09:30`/`d22`/`d23`: Vivienne Tanaka (2 y) "on the street, on the
   service's warning since d13" three mornings, then "at home" `d24 09:30`, camp `d25 09:30`. Across the group 6 family lines read "(2 y) on the street", 7 "(4 y)". No harm line, but a toddler alone on
   a 100k street for three days is the case the rule exists for. Likely cause: a child with no parent at home is placed on the street, and "a warning with no free parent at home takes at once" did not
   fire for 8 days (d13 to d25). V13: new.
2. **Mass till theft with no response.** `vendor_9648 d23 11:58`-`17:49`: ten different people empty "Brick Lockridge's stall" (4, 8, 4, 4, 2, 6, 2, 2, 1, 2 coins), each a `+2c (Food)` sale then
   a `-Nc (other)`; each prints SawCrime, WasRobbed, "new relationship ... Enemy affinity -1.00" with a new stranger; day net -21c. No Report, Pursuit or Arrest in the diary. Reads as a loop with a
   wrong trigger, not a mugging wave. V13: new.
3. **U1 Schools do not reach the Sump.** 40 `[family]` slots read "(play: no School within 1 h)" in `camp_parent_102044` (15 y, 9 y) and `camp_parent_17162` (17 y) all week, while D4 kids get
   "School#5074 (16 min walk)". The seed/founding rule has not put a School in D1 by d27.
4. **A warning that never moves.** `camp_parent_102044 d21`-`d27`: "on the service's warning since d5" (22 days), children 15/9/2, pantry 2-3, father steals 1-2 food in the week; no second warning,
   no take, no event. A 10-y child with a 3-day gap was taken the same night in another pick.
5. **A reported abductor is hired the same night.** `miserable_12664 d21 14:18` reported for Abduction by two witnesses, `d22 00:11` "hired as Ripperdoc (picked by Dex Bartlett, the foreman)" with
   toxicity 0.94 and an open report: no arrest, no consequence. V13 bug 5 (cover hire laid off in 7 min): not seen, **better**.
6. **"(an elder parent)" label.** `elder_78977 d21 15:00` "Kofi Garland visited Wilhelmina Yates at Block#336 (an elder parent)": **unchanged** (V13 bug 8).
7. **Jail middle blank.** `street_worker_6698 d26`-`d27`, 16 h at the Precinct each: **unchanged** (V13 bug 6).
8. **Commute eats the day.** `sweeper_61654` travel 12.0 h `d25`; `vendor_9648` 12.9 h `d27`; `sick_49045 d26 00:07` "16 h 53 from home": **unchanged**.
9. Other V13 bugs: broken-victim memory flip (homeless WasRobbed): not reproduced, the till thefts name the right thief; bin-contest flood: 0 BinContest lines (not in these picks); muster dies silently: no
   raider pick here; camp count: **better**.

## 5. Dull spots, ranked (mechanism, not knob)

1. **Visits fail by distance.** Both camp-jobless fathers live in the Sump, the camp is in D4. A parent who is owed the answer (a pawned thing for the 60c buy-out, a petition the warden writes down, a
   neighbour who carries the word) closes the loop the headcount opens.
2. **The slip-out has a next scene and nobody plays it.** Hollis goes home, then to the street: the father does not take him back, the service does not fetch him, the warden never notes it. A
   "fetched from the street" line, or a father who walks him to the fence, is the missing beat (and fixes bug 1).
3. **The sick man's friends.** The same "has no friend left to ask" line on d23 and d26 (`sick_49045`), d22 and d25 (`sick_75763`): a letter, a loan shark, a gang that pays (V13 had it).
4. **Street workers and vendors with no job.** `street_worker_81797` zero travel, 5-8 h leisure daily, not one event; `vendor_20832` the same. They need a pitch to be told about (the board loop
   the elder used) or a customer who is a person once.
5. **Jail and rest days.** 16 h days with nothing: a yard fight, a visit, a word about a child.
6. **A reported man becoming a ripperdoc** (bug 5): a post that checks a standing report would make the hire a story.

## 6. Too much

- **Witness lines.** 122 Witness and 78 WitnessReacted across the 22; `camp_parent_17162` alone Witness x64 in a jail-and-platform week. The Notable block drowns in it. Same ticker as V13.
- **Cooled goals** (137 lines): a cooled Earn then a Wander, repeated by every jobless pick. Flat, not broken.
- **Commute as the day, rest-day idling** (`sweeper_46469 d27` 17.6 h idle, `sweeper_61654` 12.6 h): unchanged.
- Not too much: starvation, evictions, gang turn-aways, the camp at 395 children, Fare beatings of 62-y-olds (harsh, aged).
