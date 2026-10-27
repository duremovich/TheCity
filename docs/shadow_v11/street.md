# Shadow V11, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent)

Window: main `f0b7e44`, 100k island, seed 42, d21-27, 18 diaries (`target/shadow_v11/isle100k_s42_d21/`). Cites are `file dDD HH:MM`.
Read: every DAY SUMMARY, every "Notable events" block, event-word greps, and full stretches of `vendor_19534` d25, `miserable_22034`
d25-26, `miserable_29154` d21, `homeless_97090` d25, `sick_91688` d26. V11 picks are different people from V10, so compare by archetype.

## 1. Verdict

Better than V10 where it is cheap: a gang try-out now ends in a line, a fare skip climbs a ladder, the sick man begs a loan, a blood grudge
turns into an AskAround/StakeOut hunt. Stories start more often than they get picked up: the loan is refused and nothing follows, the hunt
fails once and cools for days, the try-out refusal is followed by Scavenge. Still two cities: people with a gang tie or a grudge have a
week; the sick, the jobless vendor/street worker and the plain homeless loop Idle/Scavenge, and four of 18 lives are 3-7 days of
"jail 24.0h" (`street_worker_7827`, `sweeper_62086`, `miserable_29154`, `homeless_97090`).

## 2. Per archetype (V10 word -> V11 word)

- **Homeless: thin -> thin, with a spike.** `homeless_75879` (35 y): net 0c all week, 54 Scavenge, then `d27 06:03` "SquatEvicted: Quinn
  Delacroix put out of Block#2230 (taken by The Glass Ghosts)" and `d27 06:09` PLAN Squat on Block#2506: a real beat, picked up in
  minutes. `homeless_97090` (18 y) has the week: `d21 14:16` Theft + "emptied the till" + four WitnessReacted, `d21 22:43` Arrest,
  `d22 01:00` "sentenced to 3 days for Theft", `d25 08:28` "tried out for The Ash Crows ... and was turned away: they were full up".
- **Sick: thin -> thin.** `sick_91688` (60 y): `d26 00:00` "Diagnosed: has a vat-cancer", same minute "begged Joss Ferreira for 218c toward a
  vat-cancer treatment; Joss Ferreira turned them down". Then `d26 00:22` Earn "cooled ... could not plan or plan failed twice", Idle
  15 h/day, no slate, no pawn, no back room. `sick_81571` (72 y, 1c) scavenges 48 times; "Coerced: Nikolai Brandt called in a debt: 4 paid"
  (`d26 00:04`) is the only touch.
- **Elder: split -> alive / flat.** `elder_58802` (61 y, Vat Tech) has a gang life: `d23 09:40` GangJoin Red Ghosts (mood -0.46), `d23 17:47`
  Assault, `d23 19:58` "Gideon Kettering patched Gideon Draper up in the back room of Back Room#1250 for 6c, no questions asked", `d25 00:07`
  missed court summons, `d26 17:07` "Casimir Vasquez attacked Gideon Draper and died", `d26 18:20` Extortion 5 coins. `elder_78947` (60 y)
  works 9.7 h a day and reports nine thefts to "3 of her people": a good citizen, no arc.
- **Vendor: one alive, one dead -> alive-by-grudge, quiet.** `vendor_19534` (27 y, 30 Vend, 121c) carries a KilledKin grudge weight 1.00 from
  `d21 00:00` (against Dante Ramirez) and hunts on d25, and only then (bug 2). `vendor_9220` (67 y, 102c, 45 Vend) sells across D3, hails a
  cab flush (`d22 18:22`), buys Clothing T1 (`d23 05:01`): quiet but a person.
- **Street worker: thin -> thin.** `street_worker_67331` (37 y, 38c): 15 Vend, 7 witness lines, `d25 00:07` "PitchClosed: closed a Street Worker
  pitch (no take)", idle 11-14 h d25-27. `street_worker_7827` (26 y) is the jail/crew story: `d22 11:30` Arrest, `d22 13:12` "8 days for
  Smuggling", `d23 00:07` "went for Smoke Delacroix in the cells (the other colours); won", `d23 17:23` "Bronwen Ellery learned who put her
  inside: Katrin Dunmore reported her for the Shakedown".
- **Sweeper: alive -> thin.** `sweeper_46223` (50 y, Recycler, 42 Sweep, +3-6c/day): "counts: none" for the week. `sweeper_62086` (28 y):
  `d21 01:20` joins The Red Brothers, `d21 01:40` steals a motorcycle, `d21 08:04` reported, `d22 21:38` arrested, "6 days for Grand Theft",
  jail d23-27: a story, but the post is abandoned and the Recycler never notices.
- **Camp warden: alive -> thin.** `camp_warden_56621` (35 y) works 8.6 h daily five days; the week's only lines are an Assault she witnesses
  (`d26 05:56`) and a Treated for the attacker (`d26 10:45` "Thea Jessop patched Wire Novak up at Ripperdoc#5257 for 12c (3c of it to the
  doctor)"). No Arrest (V10's `camp_warden_92166` had one). `camp_warden_62881` (62 y, 273-tile commute): travel 5-8 h/day, Witness x6 only.
- **Miserable: alive -> alive.** `miserable_22034` (72 y): GangJoin `d21 10:36`, Theft x3, Notice `d22 09:35`, Preached `d23 00:08`, Fare
  `d23 22:41` beaten + "banned from the El for 14 days", `d25 13:31` caught again inside the ban and beaten, `d26 00:00` Evicted by Habitat 23,
  `d27 15:00` Brawl lost. A full, grim week. `miserable_29154` (41 y) is the odd one (bug 1).
- **Camp parent: new, thin.** `camp_parent_101931` (21 y) walks 310 min for 436 tiles to Work Camp#5132 (`d23 12:07`), rests 91 min, and `d23
  18:48` "Visited: Lavinia Fairweather visited Dex Lockridge ... (a Camp child)"; that is the whole week. `camp_parent_21316` (35 y, Vat Tech)
  has only Quit/Hire (`d25 00:10` "far commute: took a post at Vat Farm#3872 nearer home") and no camp line.

## 3. Round 2's mechanisms seen at 100k

Fired, quoted:
- **R1 fare ladder:** `miserable_22034 d23 22:41` beat + 14-day El ban ("caught at the gate 3 times in 7 days"), `d25 13:31` re-beat inside the
  ban. Works; "picked himself up off the platform" is a good closing line.
- **R3 back rooms / patch:** `elder_58802 d23 19:58`, `camp_warden_56621 d26 10:45`. **R3 Friend's loan:** `sick_91688 d26 00:00`.
- **R4 witnesses and folds:** `miserable_22034 d22 09:35` one Notice line for 14 tenants; d27 summaries read "the city's rest day" or "rest
  day: no shift"; WitnessReacted is "told N of her people", no grudge flood.
- **R5 try-out outcome, court missed:** `homeless_97090 d25 08:28`, `vendor_19534 d24 15:12` "turned away: the treasury could not feed another
  mouth", `elder_58802 d25 00:07` / `street_worker_7827 d21 00:07` "missed ... court summons (1 in 10 days)".
- **R6a pitch:** `street_worker_67331 d25 00:07` PitchClosed (no take), `street_worker_7827 d23 00:07` PitchClosed (jailed).
- **R6b gun hunt:** `vendor_19534 d25 04:34` PLAN Hunt on Dante Ramirez [GoTo(Intel) > AskAround > GoTo(Intel) > StakeOut > Attack]; AskAround
  `d25 06:28`, StakeOut at Bar#4438 `d25 10:11` (181 min), `d25 13:12` "PLAN FAILED: Hunt failed at StakeOut: PreconditionLost", replanned,
  `d25 17:46` "failed at GoTo(Intel): BuildingFull". The best thing in the group.
- **R2 walking:** `homeless_75879 d27 04:22` 33 min for 27 tiles; faster than V10's 150 min for 9 tiles, but see bug 4.

Expected, not seen: R3 slate/pawn on the sick; R3 rescue of the wounded; R5 muster/Scavenge/stand-down on any street pick; R6a cab for a
street worker; R6b landlord's desk for the evicted `miserable_22034` / `homeless_75879` (no hearing, no desk line); reporter beat story on the
Fare/Evicted beats. No Loan/UnderWater/Blackmail on the vendors.

## 4. Bugs, ranked by what the player sees first

1. **A defender gets ten days for being attacked asleep.** `miserable_29154 d21 01:27-02:48` three "Assault: Cutter Crane fought off X" in 80
   min (Rafe Nakamura, Dex Abernathy, Emeric Yardley) at Block#2541 while Sleep; `d21 02:59` "Emeric Yardley arrested Cutter Crane", `d21 04:26`
   "sentenced to 10 days for Assault", then 7 days of "jail 24.0h". The attackers are never named as attackers and the victim (mood -1.00) is the
   only one jailed. Likely cause: the arrest takes the one participant the cop knows. Reads as broken, not harsh.
2. **A blood grudge hunts once a week.** `vendor_19534`: GOAL Hunt x57, each "goal Hunt cooled until +2h (could not plan or plan failed twice)"
   (`d22 01:34`, `03:34`, `06:04`, `08:04` ...) and back to Idle in the same minute; the first real plan is `d25 04:34`. Likely cause: planning
   fails until an Intel lead exists and the retry is a flat 2 h cooldown. A kin-killing should be the vendor's whole week.
3. **A 72-year-old walks 800 tiles to deal for the gang.** `miserable_22034 d26 12:14` "UNWIND pick: rung Free, Deal at spot (184,816) ...
   GoTo(Spot) 390 min, 801 tiles (ended)"; `d24` is 14.5 h travel, `d25`-`d27` 10 h; the one `ACTION Deal` in the week is `d27 17:23`. Likely
   cause: the Deal spot chooser ignores reach (V10 bug 8's cousin); plans are cut at the district line ("cut at street (D3 Neon Quarter)", `d24 18:14`).
4. **ETA and tiles disagree.** `miserable_22034`: 251 min for 232 tiles (GoTo(Home)), 171 min for 111 tiles (GoTo(Clinic)), but 390 min for 801
   tiles; `homeless_75879 d27 06:09` 21 min for 11 tiles. R2's doubling is not uniform across callers. `camp_warden_62881` still loses 6-8 h/day to
   a 155 min commute (V10 bug 4: better, not gone).
5. **Treat fails at the door.** `miserable_22034 d25 14:02` PLAN Treat on Back Room#1250, walked 171 min, `d25 16:53` "PLAN FAILED ...
   BuildingFull", then Scavenge. A three-hour trip, no line telling the player anyone was turned away.
6. **The refused loan ends the sick man's story.** `sick_91688 d26 00:00` loan refused, then nothing (`d26 00:22` Earn cooled; no second asker,
   pawn or theft). V10 bug 6: better at the front, unchanged after.
7. **Jail middle is still blank** (`street_worker_7827` d24-27, `sweeper_62086` d23-27, `miserable_29154` d22-27: 1440 m at Precinct#4485).
   Cells fired only on `street_worker_7827 d23 00:07` and `d25 00:07`. V10 bug 5: better, not fixed.
8. **Camp warden Arrest never fires** this week; the warden is a worker with a shift, not a warden.
9. **V10 map:** gang try-out silent end -> **fixed** (`homeless_97090 d25 08:28`); broke-gang crew line -> not seen; Raid muster flip-flop ->
   not seen; ETA wrong -> **better, see 4**; jail middle -> **better**; sick no doctor -> **unchanged**; witnesses silent -> **better**; job
   ignores reach -> **unchanged** (`camp_warden_62881` 273 tiles; `camp_parent_21316` quit for it, the fix working).

## 5. Dull spots, ranked (mechanism, not knob)

1. **Idle money-holders.** `vendor_9220` (102c), `vendor_19534` (121c), `sick_91688` (10c) idle 11-15 h at home. A rented pitch, a stock errand
   or a debt collector's knock would start a day.
2. **Cells.** A yard fight, a visitor from kin, a recruiter inside, a work detail: 4 of 18 lives are blank jail days.
3. **Refused help.** The loan refusal, BuildingFull, "gave nothing (nobody was home)" are dead ends. Give each a follow-on: a second ask, a
   pawn, a theft, a vigil, a rumour of who would not help.
4. **Camp parents.** One visit and no child state. A child who is sick, taken or missing makes the Visit a story.
5. **Warden with a shift but no scene.** A missing child, an absconder, a ration dispute, an inspector.
6. **Sweeper finds.** 42 Sweep actions, zero finds, bodies or cache lines in `sweeper_46223`.
7. **Homeless Scavenge** (54 in `homeless_75879`, net 0c): finds by place, a fence path.
8. **Elder bystanders.** `elder_78947`'s nine "told 3 of her people" lines read as a ticker; put her on a stoop.

## 6. Too much

- **Fare Evasion at the same man** (`miserable_22034`, 5 Fare lines, two beatings in 42 h, one inside a ban): the ladder climbs, but a banned
  man should stop riding for a while or the revolving door is back.
- **Hunt flicker** (bug 2): 57 GOAL lines of the same sentence in one diary.
- **WitnessReacted lines** (`elder_78947` 9, `elder_58802` 6): several per day, identical text.
- **Travel:** 10-14 h/day for `miserable_22034`, 5-8 h for `camp_warden_62881` and `elder_58802` (D1 to D6 commute) reads as a commuter
  city, not a harsh one.
