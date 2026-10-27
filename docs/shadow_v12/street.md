# Shadow V12, street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent)

Window: 100k island, seed 42, d21-27, 18 diaries (`target/shadow_v12/isle100k_s42_d21/`). Cites are `file dDD HH:MM`. Read: every DAY SUMMARY,
every Notable-events block, full d21-d24 of `camp_parent_18178` and `camp_parent_24900`, d24 of `camp_warden_84386`, d22/d24-26 of
`homeless_72389`, d22 of `homeless_92272`, the treatment stretch of `miserable_64146`. Different people from V11, so compare by archetype.

## 1. Verdict

Harsh, not broken, for the lives I read: starvation, jail, cancer, a gang day-off and a fare beating all read as a hard city, and no
under-18 appears as a victim, in vice, sick or dead in any of the 18 (grep for any "(N y)" below 18 outside `[family]` lines: none).
The camp reads as a distance gulf rather than a scene: both camp parents have a child inside and cannot reach it ("4 h 07 from home"),
the wardens are clerks, and nobody in the group ever sees the camp from inside. Against V11: more lines that name a cause (try-out,
treated, "stays off the trade"), but still stories that start and are not picked up, and the jail middle is still blank.

## 2. Per archetype (V11 word -> V12 word)

- **Homeless: thin -> alive/thin.** `homeless_92272` (54 y): `d21 00:25` Starving, `d21 20:56` GangJoin The Grey Hands, `d22 14:54`
  "smashed the till at Fight Pit#2157 for The Grey Hands: took 80c (kept 20c)", `d22 18:23` "sentenced to 6 days", then 5 days of
  "jail 16.0h" (blank). `homeless_72389` (60 y, 0c all week, travel 8-11 h/day dumpster-diving, hunger 1.00 `d24 06:40`..`d26 10:41`,
  repeated "PLAN FAILED: Eat failed at DumpsterDive: PreconditionLost") is hired as Guard `d25 00:11`, "skips the shift ... mood -0.98,
  misery 1.00" `d26 08:00`, then arrests two thieves and beats two fare evaders `d27 09:24`/`12:54`. A starving woman policing the
  poor is a good beat; it does not follow from anything (no line says she ate).
- **Sick: thin -> thin, and the pick is wrong.** `sick_101646` (45 y, toxicity 0.95, malnutrition 0.62) and `sick_89736` (29 y) both read
  "no cancer", 0c, Starving x2 and nothing else; idle 5-10 h/day `sick_89736 d24`. These are hungry men, not sick ones. `sick_101646`
  `d27 00:11` Hire Guard, `d27 21:45` Arrest: the only touch. The real vat-cancer life is `miserable_64146` (below).
- **Elder: alive/flat -> flat.** `elder_79035` (61 y, Vat Tech): `d23 23:32`/`d24 23:48` turned away at Gambling Den doors "(drunk)",
  `d24 23:48` Brawl (swung at the bouncer, thrown out), `d25 00:08` "Retired ... (to family)" and "moved in with family at Block#250",
  then idle 17.6 h `d26`, 20.6 h `d27`. A real arc that stops at the door of the new home. `elder_58828` (72 y, 0c): `d22 00:00` RentShort,
  `d26 03:43` Theft "(miserable, mood -0.67)": harsh and quiet.
- **Vendor: alive-by-grudge -> alive (one), quiet (one).** `vendor_11367` (24 y): `d24 00:09` "learned the job Omar Oakes sold for 40 never
  existed", `d24 06:23` Cells "Omar Oakes learned who put him inside: Bex Zhou reported him for the Fraud": a con, a grudge, a report in
  six hours (see bug 3). `vendor_21993` (65 y, 100c): five "hailed a cab (flush)", `d25 12:34` bought and installed a Legs implant (60c);
  quiet but a person.
- **Street worker: thin -> alive/thin.** `street_worker_40556` (54 y, no job, 0c): `d21 06:45` Theft, `d21 08:39` Arrest, "3 days", `d24 11:19`
  beaten at the El "left on the platform 30 min", `d24 15:55` patched for 0c in a back room, `d24 20:44` GangJoin The Neon Wolves, `d26 10:44`
  struck at The Salt Widows's hold (6 saw it), `d27 00:07` "stays off The Neon Wolves's trade today: the nearest spot ... is 5 h 17 from
  home". The best street life of the group. `street_worker_6698` (55 y, 64c): one implant, one PitchClosed, idle 6-8 h/day from d24: dead-ish.
- **Sweeper: thin -> thin.** `sweeper_45834` (19 y) and `sweeper_60400` (53 y) work 4.7-7.0 h of a 9 h shift and travel 4-8 h/day
  (`sweeper_60400 d24` travel 8.2 h); the only lines are a cab "late for a shift" `d24 16:18` and an implant/motorcycle purchase. No finds.
- **Camp warden: thin -> thin.** `camp_warden_84386` (26 y) does 84 GoTo and 46 ClerkWork, 8.8 h at the camp daily, then gambles and drinks
  `d24 19:27`-`21:27`; only events: Preached, Served (turned away at a door). `camp_warden_101976` (64 y): one Visited line all week; "counted
  Dmitri Chrome (the Camp's headcount)" `d21 09:00` is the sole camp-specific line. The warden is a clerk with a headcount.
- **Miserable: alive -> alive (one), dead (one).** `miserable_64146` (61 y): vat-cancer, "240c short ... has no friend left to ask" `d21 00:00`,
  GangJoin The Hollow `d21 13:00`, Treated x3 "(no cure)", Extortion, Refused x4, `d25 15:37` SquatEvicted, `d25 15:50` Stripped a body. A full
  grim week. `miserable_105640` (50 y): `d21 08:51` "died of Starvation", succession in The Wire Ghosts, stripped `d21 09:11`, buried `d22 15:31`:
  harsh and handled, the diary is 7 lines.
- **Camp parent: thin -> thin.** `camp_parent_18178` (64 y, Clerk): child Nerys (11 y) in Work Camp#5132, GRUDGE against Precinct#4485
  (ChildTaken 0.48) from `d21 00:00`, `d21 00:07` "could not make the trip ... 4 h 07 from home", `d22 19:46` Earn: steals a motorcycle,
  `d23 01:37` arrested, "6 days for Grand Theft", jail d23-27, `d27 19:00` "Brick Thorne visited Moth Blackwood at Precinct#4485 (an elder
  parent)". `camp_parent_24900` (72 y, no job): Rafe (4 y) and Dahlia (0 y) in the camp, ChildTaken 0.71, `d21 00:07` "could not make the trip ...
  4 h 55", then 6 scavenge hours a day with a pantry of 0-2. Neither parent ever reaches the camp.

## 3. Round 3's mechanisms seen at 100k

Fired, quoted:
- **S1 distance:** `camp_parent_18178 d21 00:07` and `camp_parent_24900 d21 00:07` "could not make the trip"; `street_worker_40556 d27 00:07` "stays off
  the trade today: ... 5 h 17 from home". The wording is good.
- **S3 justice:** charge names the victim: `camp_parent_18178 d23 03:00` "sentenced to 6 days for Grand Theft from Delia Moreau", `homeless_92272 d22 18:23`
  "Shakedown from Ada Adeyemi". Fare beating cool-down: `street_worker_40556 d24 11:19`/`11:49` "picked himself up off the platform".
- **S4 care:** back-room patch `street_worker_40556 d24 15:55`, `miserable_64146 d22 05:10` (but see bug 1).
- **S5 trades:** hired guard acting: `homeless_72389 d27 09:24`, `sick_101646 d27 21:45`; bouncer's turn-away `elder_79035 d23 23:32`.
- **S6 children:** `[family] my children: ... in Work Camp#5132; the pantry holds N` every day at 09:30 in both camp parents. Camp escapes and
  the service's take are not visible in any of the 18 diaries (the ChildTaken grudge is the only trace).
- **S7 folds:** rest-day label in the summaries; "met 4 in the cells this hour" (`homeless_92272 d22 18:53`).

Expected, not seen: S1 Visit/Camp answer *succeeding* (zero visits to a camp in 18 lives); S2 "N in a row" back-off only once
(`camp_parent_24900 d22 03:05` "2 in a row"); S3 jail meals/sleep/guard's collar/"resisted arrest" never in any jail stretch (`camp_parent_18178`
d24-27, `homeless_92272` d23-27: 1440 m at the Precinct, no line); S4 Friends' ladder on a sick life (the `miserable_64146` line says "no friend left
to ask" three times); S5 hired hand at the yard, cabbie's cut on the cab hailers.

## 4. Bugs, ranked by what the player sees first

1. **A treatment is billed 80c and paid 1c by a man with 0c.** `miserable_64146 d22 05:10` "Anselm Whitlock cut back Quill Vance's vat-cancer ...
   for 80c (no cure)" vs the wallet 0c and `d24 03:04` `[money] -1c (Treatment), purse 0c ... flows: Treatment -1 to Cipher Santoro#80852` (not the
   doctor named in the Treated line), `d25 09:34` same "for 80c". Three "80c" treatments, 1c of money moved; the Loan line says he is 240c short.
   The event text and the ledger disagree (and the payee is a different person). Likely the text prints the list price, the pay path charges
   what the purse holds. Also three 144-minute walks (128 tiles) for a "no cure" patch; the point of the loop is unclear.
2. **Camp parents cannot ever reach the camp, and the camp has no inside.** Both parents' Visit answer is "could not make the trip" and nothing
   follows; the wardens (`camp_warden_84386`, `camp_warden_101976`) are clerks (84 GoTo(Station)/46 ClerkWork, zero child, absconder, ration or
   count line beyond one "counted ... headcount"). With ~124 children taken a day this is a gulf the player never sees from either side.
   V11 bug 8 (warden Arrest never fires): unchanged.
3. **A con names two different sellers.** `vendor_11367 d24 00:09`: Defrauded "the job Omar Oakes sold for 40", Coerced "Kenji Blackwood sold Bex Zhou a
   job" (the money flow and the Deceive move are Kenji's), the grudge goes against Omar Oakes, and `d24 06:23` Cells "Omar Oakes learned who put him inside:
   Bex Zhou reported him". Bex's grudge and report land on the wrong man. Likely the Defrauded/GrudgeFormed pair uses a stale `seller` while the Coerced pair
   uses the actual deceiver.
4. **Jail middle blank (V11 bug 7): unchanged, and S3's jail meals/sleep never show.** `camp_parent_18178` d24-27 (4 x 1440 min), `homeless_92272`
   d23-27, `street_worker_40556` d22-23: "SETTLED APART ... counted by the jailer" and nothing else; "jail 16.0h" in the summary. 3 of 18 lives lose 3-5
   days.
5. **The sick pick is not sick.** `sick_101646`, `sick_89736` have "no cancer" and 0 sick days; the pick keys on malnutrition/toxicity. V11 bug 6
   (refused loan ends the story): not seen this window.
6. **Commute eats the post (V11 bug 4): unchanged.** `sweeper_60400 d24` travel 8.2 h, work 5.1 h; `sweeper_45834 d21` travel 5.8 h, work 4.8 h;
   `elder_79035 d24` travel 7.2 h, work 9.6 h; `homeless_72389` travel 11.3 h `d23` (dumpster dives, 1 h each, mostly PreconditionLost).
7. **Idle money-holders (V11 dull 1): unchanged.** `street_worker_6698` (64c) idle 6-8 h, `vendor_11367` 67c idle 3-6 h.
8. **V11 map:** try-out refusal/fare ladder/hunt flicker not in this window; WitnessReacted flood -> **better** (one line per event); Treat BuildingFull
   walk -> not seen; ETA vs tiles: `miserable_64146 d22 02:05` 144 min for 128 tiles, `homeless_92272 d22 11:22` 121 min for 113 tiles (consistent,
   **better**).

## 5. Dull spots, ranked (mechanism, not knob)

1. **The camp from inside.** A warden's round that finds a child sick, an absconder, a ration short, or a parent at the gate; a count that does not
   match. Without it the 124 takes a day are a ledger number.
2. **A reachable visit.** A camp day that a parent can afford (a pass, a lift, the Camp answering a petition), or a letter/rumour of the child in the
   parent's diary; today the answer is always "too far".
3. **Cells.** A yard fight, a meal line, a visitor, a recruiter (3 of 18 lives).
4. **Sick/hungry without a second ask.** A starving man should beg, steal, or be taken in by a Soup Kitchen line; `sick_89736` idles 9.6 h `d24` on 0c.
5. **Retired elder at the family door.** `elder_79035 d26-27` idle 17-20 h after the move: a child to mind, a rent to share, a quarrel.
6. **Sweeper finds / sweeper's route.** 5 h/day of travel for a post, no find, no body, no cache.

## 6. Too much

- **Travel as the day.** 4-8 h/day for the sweepers, the elder, the wardens (`camp_warden_84386 d26` travel 4.9 h to a Gambling Den and back) reads as a
  commuter city, not a harsh one. Dumpster dives that fail on PreconditionLost 5 times a day (`homeless_72389 d24-26`) are monotonous.
- **Rest-day idling.** `sweeper_45834 d27` idle 19.1 h, `sweeper_60400 d27` 21.0 h, `elder_79035 d27` 20.6 h: the rest day is a blank, not a day off.
- **Gossip ticker.** `camp_warden_101976` has ~10 "Raided by gang ..." gossip lines in two days, same text, mostly "already known / dropped".
- Not too much: starvation deaths and the camp's size at 100k read as harshness, and I found nothing in the 18 that touches a minor.
