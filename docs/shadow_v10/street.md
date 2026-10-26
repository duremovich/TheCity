# Shadow V10, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable)

Window: main `33f9c55`, 100k island, seed 42, d21-27, 16 diaries. Cites are `file dDD HH:MM`, files under
`target/shadow_v10/isle100k_s42_d21/`. Read: camera.tsv, every DAY SUMMARY line, event-word greps for all 16, and full
stretches of `miserable_35413`, `miserable_69530`, `vendor_11728`, `vendor_30714`, `homeless_97788`, `camp_warden_92166`.

## 1. Verdict

Better than V9 in two ways. The El-ride black hole is mostly gone (goto_over_120 totals 18 across 16 diaries; V9 had 9-18 in a
single diary), and the new `miserable` picks give the first street lives with a visible arc: brawl, crew, court summons, arrest,
sentence, talking in the cells.
It is still two cities. People with a gang tie or a rap sheet have stories. The sick, the plain homeless, an idle elder and the
jobless vendor and street worker loop on Idle/Scavenge/Rest with no event for days, and a gang try-out ends in silence.
Jail now has content at the front (Sentence, Release, Cells shift-missed) but the long sentence days are still 24 h of nothing.

## 2. Per archetype (V9 word -> V10 word)

- **Homeless: alive-flat -> thin.** `homeless_97788` (29 y): net 0c all seven days, 102 Scavenge, mood -0.79 to -0.98.
  `d23 11:48` "GOAL JoinGang ... PLAN JoinGang [GoTo(Hideout) > JoinGang]", `d23 12:26` "ACTION JoinGang ... 22 min (ended)",
  `d23 12:48` back to "GOAL Idle ... Wander". Nothing says admitted or turned away. 94 JoinGang mentions in the week, no join.
  `homeless_75984` is the same loop in D1: 120 Scavenge, net -3 to +6c.
- **Sick: thin -> thin.** `sick_70955` has diagnosed cancer 0.39 -> 0.49 over the week, 0 Treat plans, idle 4-7 h daily; Treat
  scores 0.044 and never wins. `sick_61720` (cancer 0.19, silent) works Vat Tech 6-9 h a day, OFF SICK 3 times: fine, no story.
- **Elder: split -> split.** `elder_79067` (68 y) works Vat Tech 9.7 h and gambles at Gambling Den#5378 every evening (80-148 m),
  `d26` +11c, `d27` -10c: a life. `elder_58875` (67 y, 0c) idles at home 14-15 h a day for 7 days, 116 Scavenge lines, zero events.
- **Vendor: one alive, one dead -> same.** `vendor_11728` (66 y) sells 122 times and is in The Static Brothers:
  `d21 19:00` "stood watch at Hideout#4528 ... (unpaid: the gang is broke)", `d23 20:00` "answered ... summons to court",
  `d22 16:38` "Cells: Hana Osgood learned who put her inside: Tank Pembroke reported her for the Fraud". Real story.
  `vendor_30714` (55 y, 199c) is dead: idle 14-20 h at home on 5 of 7 days, 3 Vend lines, then `d24 12:51` "bought a Eyes implant T1
  ... for 60" after a 66-min walk and a 141-min BuyAsset.
- **Street worker: alive/jailed -> thin.** `street_worker_23458` (58 y) has one good night: `d22 01:16`-`05:28` five Vend blocks, then
  `d22 16:55` StealVehicle and `d22 20:39` "Fence @ The Drowned Choir#1766", +10c d23. Then four days of home idle.
  `street_worker_6698` idles 9-16 h a day, 24 h at home d26-27, net 0c; 15 Assault gossip lines, no reaction.
- **Sweeper: thin -> alive.** Both work the Recycler: `sweeper_50144` 37 Sweep actions, `d21 09:17` "Sweep @ street, 46 min", then
  11-21 min GoTo(Beat) hops between stops (`d21 09:03`, `10:05`, `11:14`). +4/+5c a day. Rounds are routes now, not one 320-min block.
- **Camp warden: alive -> alive.** `camp_warden_92166` works 8.8 h five days and now acts: `camp_warden_92166 d22 17:59` "Arrest: Hale
  Abernathy arrested Santiago Blackwood". Day off d27 for both wardens, -20c each.
- **Miserable: new, alive.** `miserable_35413` (40 y, mood -1.00): `d21 17:00` "Brawl: Cass Ellery started a brawl with Ulric
  Abernathy on the street (miserable -1.00 and high; a rival's colours); Ulric Abernathy won", crew days off d21-d24, `d25 08:05`
  "sentenced to 10 days for Assault", `d26 00:07` "Crew: Cass Ellery talked in the cells: named Cass Hallow of The Neon Dogs for
  Extortion". `miserable_69530` (18 y, Vat Tech commute 535 min) is arrested 4 times, sentenced 4, `d24 00:07` "Cells: missed the shift
  as Vat Tech in the cells (absence 2 in 14 days)", `d25 04:50` "Crew: Jocasta Crane is out of The Iron Teeth (lost the colours: 3
  duties missed, beaten out by Sprocket Park)".

## 3. The pass's mechanisms seen at 100k

Fired, quoted:
- **Misery acts (phase 1):** `miserable_35413 d21 17:00` brawl, trigger text in the event.
- **Gang day off, duty, court, welcome back (5b):** `vendor_11728 d21 20:07`, `d23 20:00`, `d23 23:34` "Florian Kemp welcomed back ...
  3 days served, nothing said inside"; `miserable_35413 d22 21:26`.
- **Cells and shift missed:** `miserable_69530 d23 00:07`, `d24 00:07`.
- **Colours lost, beaten out:** `miserable_69530 d25 04:50`. **Talked in the cells:** `miserable_35413 d26 00:07`.
- **Raid order:** `vendor_11728 d26 00:05` "gang standing order is now Raid", `d26 18:58` PLAN Raid [GoTo(MusterPoint) > Muster >
  GoTo(RaidTarget) > Brawl]; it was interrupted by Eat/Earn four times (d26 19:28, 20:28) and never reached a brawl.
- **Reach (phase 4):** sweeper posts are in their own district and walkable; no vendor pitch sent out of district.
- **Tribute pressure (phase 3):** `vendor_11728 d21 01:46` "Block#2068 would not pay Tank Pembroke", repeated "gave ... nothing (nobody had a coin)".

Expected, not seen:
- No protection visit on any vendor (`vendor_11728` is the collector, not the victim); no pimp or cut on either street worker.
- No treatment for the diagnosed cancer (`sick_70955`).
- No Loan, UnderWater, Blackmail or Service line in any of the 16.
- No homeless Scavenge result that differs by place.

## 4. Bugs, ranked by what the player sees first

1. **A gang try-out ends in silence.** `homeless_97788 d23 12:26` "ACTION JoinGang ... 22 min (ended)", then Idle; again `d26 16:48`
   (its walk is "GoTo(Hideout) ... 150 min, 9 tiles"). No accepted or refused event, so 94 JoinGang goals resolve nothing. Likely
   cause: the join action has no outcome event when the gang declines.
2. **Crew duty reads "unpaid: the gang is broke" every night.** `vendor_11728 d21 19:00`, `d22 19:00`, `d24 18:50`. The gang has no
   economy and the vendor still stands watch. Likely cause: crew pay against the treasury.
3. **Raid muster flip-flops.** `vendor_11728 d26 18:58` Raid, `d26 19:28` back to Eat, `d26 19:58` Raid, `d26 20:28` Earn: four
   interrupts in 90 min, no brawl. Needs hysteresis on the muster goal.
4. **ETA wrong for the distance.** `homeless_97788 d26 16:48` 150 min for 9 tiles; `sweeper_64662 d27 15:12` "GoTo(Hideout) 180 min,
   72 tiles". Smaller than V9's El rides (V9 bug 1: better, not gone).
5. **Jail middle is blank.** `miserable_35413` and `miserable_69530` d26-d27 "jail 24.0h", 1440 m at Precinct#4485.
6. **Diagnosed sick never reaches a doctor** (`sick_70955`; V9 bug 6: unchanged).
7. **Witnesses mostly do not report.** `street_worker_6698` 15 Assault lines, `sweeper_50144` 14; the warden arrest `d22 17:59` is the
   exception (V9 bug 7: better).
8. **Job assignment ignores reach.** `miserable_69530` is a Vat Tech with a 624-tile, 535-min commute (D3 to D5).

V9 map: El rides eating 4-14 h better; vendor spot in another district fixed in these picks; sweeper post in another district fixed;
jobless young idle unchanged (`street_worker_6698`, `vendor_30714`); elder idle unchanged (`elder_58875`); cancer no doctor unchanged;
witnesses silent better; same-minute Quit and Hire not seen.

## 5. Dull spots, ranked (mechanism, not knob)

1. **Unrecruited joiners.** A try-out should produce a story: an initiation errand (carry a package, beat a rival, steal a car), a
   test, or a refusal with a reason, so a week of JoinGang becomes a plot.
2. **Jobless 55-y-olds with money sit at home.** `vendor_30714` has 199c and idles 20 h. Give a street trade a reason to open (stock
   to buy, a pitch rented from a gang) so the day starts with a deal.
3. **Diagnosed sick on no wage** (`sick_70955`): price the treatment against 12c; she borrows, steals, or goes home to die.
4. **Elders with nothing to do** (`elder_58875`): a stoop spot and a gossip role, carrying the Evicted rumours.
5. **The homeless Scavenge loop** (120 and 102 Scavenge, net 0c): finds depend on the place, plus a fence path.
6. **Jail middle:** a yard fight, a visit from kin, a gang recruiting inside, a work detail.
7. **Camp warden headcount shortfall** as a trigger (child missing, absconder).
8. **The sweeper meets bodies and caches**; Sweep lines exist but no finds.

## 6. Too much

- **Fare Evasion arrests of one young man** (`miserable_69530`: `d21 02:41`, `d22 19:01`, `d23 14:29`, each a 1-day sentence). A fare
  skip a day reads as a revolving door, not harshness.
- **Gang day-off and stand-watch lines every evening** (`vendor_11728` d21-d25, Crew x15), the same sentence each night.
- **Robbed gossip** (189 mentions in `vendor_11728`) buries the diary; not a gameplay issue.
