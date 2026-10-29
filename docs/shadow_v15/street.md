# Shadow V15, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent, neglect_parent, camp_jobless)

Window: 100k island, seed 42, d21-27, 22 diaries (`target/shadow_v15/isle100k_s42_d21/`, main bf63dee). Cites are `file dDD HH:MM`.
Read: every DAY SUMMARY, every `[event]`/`[family]` line of all 22, both wardens' d22, `camp_parent_51134` d22-d23 in full. Different people from V14.
`target/r6_100k/analyze.txt` read for city counters (Witness 207k, Turf 4.3k, ChildTaken 4.1k in 30 days). Group counts below are greps over these 22 files.

## 1. Verdict

Better than V14 on the thing that was broken: no child is on the street (0 of ~130 `[family]` lines; V14 had 13), no under-18 appears in any
harm line, and the Witness ticker is lighter (about 150 matching lines across 22 files, max 17 in one; V14 had 200). Where a story starts it now
gets picked up (a fraud victim's grudge, a report, the fraudster's jail, a fight, an arrest; a father frees his girl from the camp and is jailed for it,
and his other child is then taken). But most of the 22 are still a clean slide with no scene: sweepers, street workers, the baby's father, the 4-y on a nine-day warning.

## 2. Per archetype (V14 word -> V15 word)

- **Homeless: thin -> thin.** `homeless_67711` (28 y): starving, soup line `d21 10:50` "the hour's 10 bowls are out, the next at 11:00", thefts, `d24 01:00` Crossfire at a corner,
  `d24 05:46` "tried out for The Hollow ... turned away: they were full up". `homeless_106186` (24 y): SoupLine 3 h 13 away `d22 00:16`, then days of Wander and idle 5-9 h, net 0c.
- **Sick: thin -> alive/thin.** `sick_65180` (65 y): `d21 12:24` theft, `15:41` GangJoin, `17:14` arrest, 3 days, `d25` begs five friends (all "nothing to spare"), six empty job boards,
  `d26 14:00`-`15:31` resists arrest three times against 60/55-y guards, `21:40` marries in a cell. `sick_50158` (72 y): the same "has no friend left to ask" Loan line `d23 00:00` and `d26 00:00`, a 6-y daughter in camp.
- **Elder: alive -> thin.** `elder_58807` (63 y): every event is a car thief's (VehicleStolen, Fare, hired as Engineer `d25 00:11`), none his own. `elder_79020` (60 y): three food thefts, then `d26 07:39` and `08:06` two beatings in 27 min, no treatment, no report.
- **Vendor: thin/odd -> alive (one).** Header still prints `Job: none`, but pitches run. `vendor_9611` (68 y): `d22 00:09` "learned the job Dagny Delacroix sold for 40 never existed" -> `d23 00:42` reports her
  -> `d24 16:26` attacks her, "went down, bleeding", rescued `16:46`, arrested `17:07`, 10 days, `d25 00:07` "closed a Street Vendor pitch (jailed)". A whole arc. `vendor_20227`: `d21 00:09` buys that same job scam for 40c,
  then "kept off the blocks around Club#5539 after dark" for 5 nights, moves in with family `d25 00:08`, buys Eyes T1 `d26 11:42`.
- **Street worker: thin -> dead.** `street_worker_60440` (57 y): one event all week (`d25 00:07` "pitch closed (no take)"), idle 5-8 h, "other" 7-10 h daily, net 0c. `street_worker_56190`: Grand Theft `d21`, six days, jail 16 h x5, one line.
- **Sweeper: thin -> dead.** `sweeper_46728`, `sweeper_61630` (same Recycler#4486, same Block#390): zero `[event]` lines in 7 days, work 5.4-7.0 h, travel 4-7 h, idle 4-6.5 h, net -4..+5c, `d27` idle 15.4 h and 12.6 h.
- **Camp warden: alive -> thin.** `camp_warden_8656` is posted at Work Camp#4775 (15 children, "nobody in or out ... every child ate last night", the same line six days). One scene:
  `camp_warden_90940 d22 13:55` "Anton Ambrose tried to break Taz Ambrose out ... beaten back by Mirabel Kemp, Silas Penhallow, Varek Ashcombe". Every child taken goes to Camp#5132, which neither warden guards.
- **Miserable: alive -> alive.** `miserable_37834` (58 y) and `miserable_61964` (48 y) cross: `d23 10:49` "Nikolai Volkov (58 y) attacked Anton Tanaka (48 y)", both patched at Back Room#3139
  (`d23 14:01`, `d22 14:26`); Nikolai `d26 00:00` "holds The Ash Widows' corner by Block#3526 (5 more of theirs on it)", `d27 00:33` SquatEvicted "taken by The Pale Lanterns".
- **Camp parent: thin -> alive (one).** `camp_parent_51134` (75 y, children 16/14/11): bug 1; the best arc in the group. `camp_parent_106052` (31 y, child 4 y): one Theft `d27 12:11` in the week.
- **Neglect parent: alive -> alive.** `neglect_parent_66285 d21 00:10` ChildTaken "unfed 2 days again, inside the warning of d17", then soup lines (0 h 26, dry pot `d24 06:19`, then 3 h 59 away), evicted `d26 00:00`.
  `neglect_parent_32056 d22 13:36` hired as Guard, yet `d23 00:10` the 12-y is taken, `d27 00:07` "could not make the trip ... 5 h 36 from home".
- **Camp jobless: thin -> thin.** `camp_jobless_79404` (37 y, baby 0 y in camp, home in the same district D4): no visit all week; `d27 20:34` GOAL Visit "a visit due", `21:24` "plan interrupted: Visit -> Unwind".
  `camp_jobless_64259`: `d21 00:07` "could not make the trip ... 20 h 04 from home".

## 3. Round 6's mechanisms seen at 100k

Fired in this group:
- **V1 children out of care:** zero "on the street" family lines (V14: 13); `[family]` reads the Camp after every take; three takes each with a reason (`neglect_parent_66285 d21 00:10`, `camp_parent_51134 d26 00:10`, `neglect_parent_32056 d23 00:10`);
  the pantry line splits the roof: "the pantry holds 10, 0 of it ours (62 households under the roof)" (`camp_parent_106052`). "No School within 1 h" is 2 lines in the group (V14: 40).
- **V2 turf:** "kept off the blocks around Club#5539 after dark (The Grey Hands and The Black Wolves at odds there)" x10 in `vendor_20227`, the rival changing `d26`; "holds The Ash Widows' corner" `miserable_37834 d26 00:00`;
  `miserable_61964 d21 03:00` "The Pale Lanterns beat The Salt Crows back on the corner by Block#3358, 6 against 3, and hold it now"; SquatEvicted "taken by The Pale Lanterns".
- **V6:** Back Room patch-ups x8 (`miserable_61964 d22 14:26` "4c (Imogen Yates put up 4c)"; `miserable_37834 d23 14:01` "for 0c, no questions asked"); gang door "turned away: they were full up" x2.
- **V4:** CabFare to a named driver: -45c to Leopold Carrow (Cab Driver) `vendor_9611 d23 09:52`, -14c `d24 12:32`.
- **Child rule:** every Assault/Wounded/Crossfire/Fare line carries ages; none under 18.

Expected, never seen: V3 body found / lead / "cried out to a guard" (0; the robbed here are tills, and a till theft is reported but never a cry); a V5 war line, truce or crackdown (0; one corner flip);
tribute received (22 lines, all "0c"); "fetched from the street" (0, but nothing to fetch); a warden's note of a slip-out.

## 4. Bugs, ranked by what the player sees first

1. **Jail for freeing your own child, with the witness flipped.** `camp_parent_51134 d22 19:18` CampBreakout (Wilhelmina, 14 y, to Block#519); the witness line reads "Sprocket Abernathy saw Cipher Mercer commit Abduction",
   then `d22 20:36` the mirror "Cipher Mercer saw Sprocket Abernathy commit Abduction" and he flees: actor and witness swapped (the V13/V14 victim-flip family, **worse**: the culprit reads as the witness).
   `d23 02:34` Arrest for Abduction, `03:42` 20 days for a 75-y father. With him jailed the 16-y goes unfed and `d26 00:10` is taken. Coherent chain, but the law calls a parent freeing his child from a buy-out camp "Abduction"
   with no hearing line, and the 14-y is on a fresh warning at Block#54 while he sits in a cell.
2. **A bleeding call nobody answers.** `camp_warden_8656 d22 13:55`/`14:25`/`14:55` "Taz Greaves (61 y), bleeding, called Dagger Northcott (34 y) for help" x3: no Rescued, no Treated. And Taz Greaves (wounded) vs Taz Ambrose
   (the breakout target) in the same minute: two names for one scene. New.
3. **A warning that never moves, and a visit that never happens.** `camp_parent_106052`: Edric (4 y) "on the service's warning since d18" on all seven mornings (nine days by d27), pantry 6-12 "0 of it ours", no second warning, no take.
   V14 bug 4: **unchanged**. `camp_jobless_79404 d27 20:34` plans a Visit to his baby one district away and drops it for Unwind.
4. **Visits by distance.** `sick_50158 d26 00:07` 22 h 22 from home; `camp_jobless_64259 d21 00:07` 20 h 04; `neglect_parent_66285 d27 00:07` 19 h 35; `neglect_parent_32056 d27 00:07` 5 h 36. Four of five camp visits fail on distance: **unchanged**.
5. **Camp#4775 frozen.** "counted 15 children ... nobody in or out" six days running while every take lands in Camp#5132. The count V14 praised (109 -> 395) is not in these wardens' camp; the mechanism is fine, the picks are in the wrong camp.
6. **"3 days' grace, and unfed again before d53 the service takes them"** (`camp_parent_51134 d23 00:10`): d53 reads like a typo for the retake window; the take lands `d26 00:10`. Wording only.
7. **"(an elder parent)" label** on Visited lines (`sick_50158 d21 12:00`, `elder_79020 d21 12:00`, `vendor_20227 d21 14:00`, `camp_parent_51134 d21 15:00`): **unchanged**.
8. **Jail middle blank:** `street_worker_56190 d22`-`d26` jail 16.0 h x5; `camp_parent_51134 d24`-`d27` x4: **unchanged**.
9. V14 status: toddlers on the street **fixed**; mass till theft **fixed** (3 emptied tills per vendor in a week, each reported); no School in the Sump **better**; commute eats the day **better** (sweepers 4-7 h of travel, not 12) but still over half a shift;
   reported man hired as ripperdoc: not seen.

## 5. Dull spots, ranked (mechanism, not knob)

1. **Sweepers and street workers have no scene.** Zero events, net +-5c. The Recycler could post a found body or scrapped goods (sweepers are who finds a V3 body, and that is the lead), a street worker a "pitch watched by the corner holder" where V2 tribute pays something real.
2. **A bleeding call needs an answer.** A warden who is called should go, or send for the Back Room doc; as is, the Wounded line is a ticker.
3. **The father who cannot reach the camp.** A failed visit should leave something: a neighbour who carries the word, a petition the warden logs, a pawned thing toward the 60c buy-out. `camp_parent_51134` shows the buy-out works for the bold; give the timid a smaller version.
4. **The fraud thread.** Two vendors paid 40c for a job that was never there, one of them avenges it and loses. One more beat (the buyers learn of each other, a refund, a crew cut off the scammer) would turn two lives into a gang story.
5. **Elders beaten twice in 27 min with nothing after** (`elder_79020 d26 07:39`, `08:06`): no Report, no clinic, no neighbour. A bystander line or a patch-up with a bill.
6. **`street_worker_60440`, `homeless_106186`**: a week of idle and "other". They need a pitch to be told about or a soup-line regular.

## 6. Too much

- Nothing broken-loud. The gang `Crew` day-off lines (`miserable_37834 d22 20:05`-`21:25`, `d26 21:12`-`23:36`) print the same five names as a separate event per member, a roll call: noisy, not wrong.
- "kept off the blocks after dark" every night for one vendor: right behaviour, one line per night, never changes wording; the war itself (V5) never shows in this group.
- Not too much: starvation, evictions, dry pots, a 20-day sentence for a 75-y, resisting arrest at 65. Harsh and aged.
