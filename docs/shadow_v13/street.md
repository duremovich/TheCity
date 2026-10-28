# Shadow V13, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent)

Window: 100k island, seed 42, d21-27, 18 diaries (`target/shadow_v13/isle100k_s42_d21/`, main a70800f). Cites are `file dDD HH:MM`.
Read: every DAY SUMMARY and Notable-events block; d21 afternoon of `homeless_71553`, d24-27 of `camp_parent_23527`, the family/take
lines of both camp parents, the treat loop of `sick_33745`, d21/d24 of `sick_47483`, the raid plan of `miserable_88388`. Different people
from V12, so I compare by archetype.

## 1. Verdict

The camp finally has an inside: both wardens print a nightly headcount (50, 100, 127, 179, 259, 342 children), a fence catch, a slip-out,
"every child ate last night", the Reserve's meals and a paid-for visit; the service warns first (`[family] ... on the service's warning since
d17`) and the take is a dated event. The rest of the group is V12's life: starve, steal, a gang day-off, long blank stretches (elders, street
workers, jail). Harsh, not broken; no under-18 appears as victim, in vice, sick or dead in any of the 18 (grep for `(N y)` below 18 outside
`[family]` and the warden's fence-catch lines: none). Stories start more often than they get picked up.

## 2. Per archetype (V12 word -> V13 word)

- **Homeless: alive/thin -> alive/thin.** `homeless_71553` (54 y, 0c): `d21 14:31` GangJoin The Static Brothers, `15:33` steals 2 food at Block#1934,
  `16:06` extort "nobody was home", `16:48` attacks Garrick Carrow (46 y), crew day-offs `d24 21:11`, bins "nothing worth selling" x3.
  `homeless_91199` (47 y): `d24 03:06` "went down, bleeding, at Slate Nakamura (38 y)'s hands", put out of her squat the same minute, `05:09`/`05:56`
  Tank Kettering binds and carries her to a back room, `d27 05:56` recovered, `d27 21:57` turned away by The Pale Lanterns "the treasury could not feed
  another mouth". That rescue arc is the best new beat of the group.
- **Sick: thin (wrong pick) -> alive (one), thin (one); the pick is right this time.** `sick_33745` (66 y, vat-cancer 0.34): married `d21 13:20`, `d22 09:47`
  Notice, `d23 00:00` Loan "240c short ... begged their friends: nothing to spare", `d26 00:00` evicted, `02:04` joins The Glass Teeth, `04:06` Treated
  "for 80c (The Glass Teeth put up 80c), to Hana Fletcher's till (no cure)". A full week. `sick_47483` (68 y, Vat Tech): `d24 00:00` "let go: too sick to
  work", `d24 08:32` "asked at Vat Farm#3704 about work and was taken on" (T5), 9.6 h at work `d25`/`d26`, "off sick" `d27 00:00`.
- **Elder: flat -> dead.** `elder_58803` (71 y, 3c, no job): "Witness x3, WitnessReacted x1", day = 56 Scavenge, 23 HangOut, 21 Wander, idle 4-8 h.
  `elder_78984` (64 y, Vat Tech, +4c/day): "counts: none", 9.6 h of work a day. No retirement, no family door, no quarrel.
- **Vendor: alive/quiet -> alive (money), quiet (story).** `vendor_19509` (69 y): 112 `[money]` lines, +2c a Food sale (`d21 10:03`..`12:27`), 49 Vend.
  `vendor_9542` (36 y): cabs home x3, `d25 00:11` hired Bartender, `d26 00:07` "closed a Street Vendor pitch (hired)". A ledger; customers are Statistical names.
- **Street worker: alive/thin -> thin.** `street_worker_6698` (55 y): `d26 00:07` Report for Solicitation, pitch swept "fined 5, closed a day"; idle 6-12 h/day.
  `street_worker_67331` (37 y, 39c): 52 HangOut, 40 Rest, 12 Vend, one `[money]` line all week, `d25 00:07` PitchClosed "(no take)", idle 11-13 h `d25`-`d27`.
- **Sweeper: thin -> thin.** `sweeper_45914` (33 y): travel 5.8-8.0 h on a 181-tile commute, work 5.5-7.1 h, `d25 08:00` "skips the shift ... walks the
  streets instead (mood -0.24, misery 0.27)" then leisure 10.1 h. `sweeper_60786` (38 y): 82-tile commute, `d27` travel 9.6 h on a rest day, `d27 10:29`
  motorcycle on finance, `16:55` GangJoin The Drowned Choir. No finds on the route.
- **Camp warden: thin -> alive/thin.** Both do the 09:00 count (`camp_warden_56621 d22 15:00` "caught Garrick Crane (5 y) at the fence ... walked them back in";
  `d23 14:00` Visited: Zara Ito let in, "the 60c buy-out beyond the 0c in hand"). 8.6 h at the camp, 4-6 h leisure. A clerk with a ledger that now has a pulse.
- **Miserable: alive/dead -> alive (one), noisy (one).** `miserable_88388` (40 y, mal 0.60, tox 0.94): thefts `d21 05:08`/`09:50`, reported, joins The Black
  Knives, three strikes `d24 23:27`-`d26 00:44`, Assault on Wilhelmina Draper (65 y) `d26 01:01`, patched for 0c `d26 13:24`. `miserable_7493` (43 y): 178 BinContest
  lines, 9 PLAN FAILED, 50 dives (bug 3).
- **Camp parent: thin -> alive/thin.** `camp_parent_23527` (37 y): warning since d17, `d24 22:21` arrested, `d25 00:10` three children taken (GrudgeFormed x3 against the
  Law), `d25 01:30` 3 days. `camp_parent_32892` (43 y): warning since d18, `d22 00:10` Ada and Delia taken, `d25 00:07` "could not make the trip ... 6 h 45 from
  home", `d26 00:00` evicted, `d26 00:08` takes up the Church of the Open Circuit.

## 3. Round 4's mechanisms seen at 100k

Fired, quoted:
- **T1 warns first:** `[family] ... on the service's warning since d17` (`camp_parent_23527` d21-d25), `d18` (`camp_parent_32892`). Pantry share: `camp_parent_32892 d21 09:30`
  "the pantry holds 0"; `sick_47483 d21 09:30` "pantry holds 30".
- **T1 the warden's round:** headcount, "1 slipped out (Philippa Carrow)", fence catches (`camp_warden_56621 d22 15:00`, `d24 12:00`), "every child ate last night, 9 of the
  meals from the Reserve" (`d26 09:00`), a visit let in (`d23 14:00`). The break-out line fires; no child is ever recorded reaching the street or a parent.
- **T2 School by travel time:** `sick_47483 d21 09:30` "Pixel Moreau (10 y) at School#5103 (41 min walk)", daily `[money] -2c (SchoolFee) ... to the City`;
  "(play: no School within 1 h)" for the 15/10/6-year-olds in the Sump (`camp_parent_23527`), "(play, under 6 for School)" (`camp_parent_32892`).
- **T4 trades:** `vendor_9542 d25 00:11` Hire Bartender "(picked by Dante Nightingale, the foreman)", `d26 00:07` PitchClosed "(hired)".
- **T5 jobless search:** `sick_47483 d24 07:00` "asked Zara Oakes, Annika Ironside, Thea Upton and 5 more about work: nobody knew of an opening", `08:32` "asked at
  Vat Farm#3704 about work and was taken on as Vat Tech". Works.
- **T6 who paid, whose till:** `sick_33745 d26 04:06` "(The Glass Teeth put up 80c), to Hana Fletcher's till (no cure)", `d26 23:15` "6c ... to Hana Fletcher's till". V12 bug 1
  (80c text vs 1c ledger) is better: price and payer agree. A gang paying a member's patch is a good beat.
- **T7 age on harm:** `homeless_71553 d21 16:48` "(54 y) attacked Garrick Carrow (46 y)", `miserable_88388 d26 01:01` "(40 y) ... (65 y)", `sick_33745 d26 21:10`
  "(66 y) ... Lux Duarte (73 y)", `camp_warden_56621 d25 06:41` "(35 y) ... Bram Crane (65 y)". Day folds for chases, refusals, cools, gossip hold.

Expected, not seen: a break-out reaching a parent or the street; a camp parent's visit succeeding (both "could not make the trip"); any `Pursuit` line; the Friends' ladder
moving past "nothing to spare" (same three people, every Loan line, 7 days); a "service at the door" line (only the standing "warning since" appears; the take at 00:10 has no
arrival line); T3 jailed-mark waits (not in this group's picks).

## 4. Bugs, ranked by what the player sees first

1. **The family line is stale after a take.** `camp_parent_23527 d25 09:30` still reads "Hana Hallow (15 y) at home ... Dante Hallow (10 y) at home ... Mina Hallow (6 y) at
   home" nine hours after `d25 00:10` ChildTaken x3; `d26 09:30` is right. The line tells the player three taken children are home.
2. **A thief's victim is printed as the thief's robber.** `homeless_71553 d21 15:12` she steals from Block#1934; `16:26` "memory WasRobbed of Garrick Carrow", Garrick "is now Enemy",
   GRUDGE against Garrick (Robbed, 0.36); gossip all week "Robbed by Garrick Carrow on Undine Jessop" (`d21 18:58` ... `d22 13:41`); she then attacks him `16:48`. Garrick is one of
   the three witnesses of her theft (`d21 15:33`). Culprit and victim are flipped in the memory/grudge/gossip row. Not reported in V12.
3. **Bin-contest flood.** `miserable_7493`: BinContest x178 ("Wire Lockridge saw X off the bin behind #4294"), 24 inside `d24 07:14`-`07:45`, one person a line. T7 folded chases,
   refusals, cools and gossip, not this; the Notable block cuts at 25 "... and 157 more".
4. **A sat muster dies silently.** `miserable_88388 d22 14:08` PLAN Raid [GoTo(MusterPoint) > Muster > GoTo(RaidTarget) > Brawl]; Muster 399 min ends 22:00, the raid walk is
   "cut at street" after 128 min, `d23 00:08` PLAN Sleep. No Brawl and no "stood down" line: nothing says why a 14-hour plan stopped (T3 reached the dropped arrest chase, not this).
5. **A cover hire laid off in 7 minutes.** `sick_47483 d21 00:00` Hire "Cutter Reyes (picked by Priya Chrome, the foreman)", `00:07` LaidOff "revenue, a shift past full staff":
   the foreman's cover hire and the corp's layoff rule fight each other.
6. **Jail middle still blank (V12 bug 4: unchanged).** `camp_parent_23527 d25-d27`: 1440 min at Precinct#4485, "SETTLED APART ... counted by the jailer" and nothing; "jail 16.0h" x3.
7. **Camp count does not reconcile.** `camp_warden_56621 d25 09:00` 179 -> 259 (+80) with "64 brought in", `d26 09:00` 259 -> 342 (+83) with "64 brought in"; d21-d24 reconcile.
   16 and 19 children are unaccounted for.
8. **"(an elder parent)" label.** `camp_warden_73258 d21 18:00` "Percival Marlow visited Silas Penhallow at Work Camp#5132 (an elder parent)" (V12 had the same label at a Precinct): reads as nonsense.
9. **V12 bugs:** 1 (80c vs 1c) better; 2 (camp has no inside) better (headcount, fence, Reserve); warden Arrest never fires: unchanged (none in either warden); 3 (con names two sellers)
   not seen; 4 jail blank unchanged; 5 sick pick fixed; 6 commute eats the post unchanged (`sweeper_45914` travel 8.0 h `d21`, 7.4 h `d26`); 7 idle money-holders unchanged
   (`street_worker_67331` 39c, idle 11-13 h).

## 5. Dull spots, ranked (mechanism, not knob)

1. **The camp parent's answer is still "too far".** Both parents: "could not make the trip" (`camp_parent_32892 d25 00:07`, 6 h 45) while the warden lets in a stranger. A parent who
   pawns a thing for the buy-out, or a petition the warden answers, closes the loop the headcount opened.
2. **The break-out has no next scene.** "1 slipped out" is never followed: found, on the street, at a gang door, dead of cold, or home.
3. **Cells.** A 3-day sentence is three identical days. A yard fight, a meal line, a warden's word about the child.
4. **Elders.** Two lives of 64-71 with zero events: a rent to share, a grandchild to mind, the retirement door.
5. **A pitch with no take.** `street_worker_67331` closes and sits 13 h idle; the pitch should move or be sold on, and a customer should be a person once.
6. **Sweeper's route.** 5-8 h/day of walking for 5-7 h of work: a body, a cache, a bribe at the Recycler.
7. **The sick man's "nothing to spare".** Same three friends, same text, a week; try a sibling, a letter, a shark.

## 6. Too much

- **Bin contests and witnessed-theft re-tellers.** `miserable_7493` BinContest x178; `street_worker_67331` WitnessReacted x14 for 14 thefts, each "told 1 of her people what X did (the Theft)": a ticker.
- **Commute as the day (V12): unchanged.** `sweeper_45914` 7.3 h travel `d24`; `vendor_9542` 6.3-6.7 h travel `d21`-`d24` with no post; `camp_parent_32892` and `miserable_7493` dive 36-50 times with 9 PLAN FAILED each.
- **Rest-day idling (V12): unchanged.** `vendor_9542 d27` idle 16.6 h, `street_worker_67331 d27` idle 12.8 h, `camp_warden_73258 d27` idle 12.4 h.
- Not too much: starvation, evictions, a gang paying for the patch, the 342-child camp at 100k read as harshness.
