# Shadow V16 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: 100k island, seed 42, d21-d27, two picks each (18 diaries; ids differ from V15's). Cites are `file dDD HH:MM` under `target/shadow_v16/isle100k_s42_d21/`.
Method: DAY SUMMARY for all 18, each diary's Notable-events block, targeted greps (Bout, purse, barred, turned, rough, GOSSIP, minors, GoTo). `target/r7_100k/analyze.txt` vs `r6_100k` for door/Bout/purse totals only.

## 1. Verdict

Better than V15 where W6 aimed: the pit now has stakes (purse, hurt loser, patch-up), the door has a second reason, owner_broke's creditor comes back, and the gossip flood is gone (5 `GOSSIP ABOUT ME` lines in 18 diaries vs 259).
The wage-and-commute archetypes (cook, joytoy, club_staff, companion_87767) are still a clock with no story. The best stories run through law and money: madam_35331 jailed, fired, turned away by a gang; owner_broke_6471 debt, beating, grudge, withdrawn report.
No minor is harmed in this group (a grep for (0-17 y) on every diary is empty; fighters 56 y and 44 y, madam 36, owner_broke 73). The new Bout line prints no ages (bug 5).

## 2. Per archetype

- **bouncer: alive/thin (V15 alive/thin).** bouncer_92590 `d26 21:00 Served: Jinx Whitlock turned Mako Ironside away at the door of Fight Pit#2582 (trouble)`, `d27 00:00 ... 6 let in, 1 turned away (trouble 1)`: the first non-"broke" reason in these picks. bouncer_58694 is a story from the wrong side of the door: shaken down in his own block (`d22 09:48`, `d24 07:10 woke ... 5c gone`), `d25 03:00 Report: ... Freya Kettering, for Shakedown`, `d25 10:34 Cells: Freya Kettering learned who put her inside` (the W2 sleeper chain works). His pit door nights are "nobody turned away" 4 of 4.
- **club_staff: thin (V15 thin).** club_staff_37318 `d24 00:00 worked the floor of Club#5657 tonight: a room of 62, served 25 (Dahlia Okafor ...)`; rooms swing 1 / 41 / 62 / 2 / 39 / 63; nothing happens to the staffer. club_staff_38885 (a Braindance Parlour Attendant, 52 y) has `counts: none` for seven days, idle 7-10 h a day.
- **dancer: alive / dead split (V15 alive).** dancer_104874 `d24 22:45 Solene Quarry got rough with Aldo Inchbald at Club#5754; Raven Singh threw Solene Quarry out, barred from the door`, `d25 21:30` again (Gwendolyn Oakes); rooms of 55-105, tips 26-53c a night. dancer_76642 is the empty room: `d23 00:00 danced 5 sets (5 to an empty room) ... to a room of 0: no tips`, one tip all week from Razor Wexford. Same as V15's dancer_103667.
- **companion: thin (V15 thin).** companion_102505 steals three motorcycles in two days between shifts (`d24 13:45`, `d24 14:43`, `d25 12:01 VehicleStolen`), a Story `d25 00:01 Nutrix Now: Ottilie Mercer robbed Zara Upton`, two regulars (`d25 21:00`, `d26 21:00`). companion_87767: one line in seven days (`d26 21:00 Florian Thorne is now a regular`), travel 3-7 h a day.
- **cook: thin (V15 thin).** cook_25080 (71 y) has one line: a shakedown paid `d24 13:08 ... he means to tell the law`, no Report follows through d27. cook_63885 follows through: `d22 07:06/07:14` two shakedowns, `d22 12:00/13:00 Report ... for Shakedown`, two Cells lines `d22 15:30/16:50`. No V15-style gang chain this time.
- **fighter: alive (V15 alive).** fighter_40359 (Roxy Haas, 56 y) is the W6 showcase: `d22 23:00 Bout: Mateo Reyes beat Roxy Haas at Fight Pit#2869 on the undercard; purse 5c; Roxy Haas hurt, to have it seen to`, `d23 00:02 GOAL Treat (was Work)`, `d25 12:46 Treated: Ada Dunmore patched Roxy Haas up in the back room ... 6c ... no questions asked`; three bouts, three losses, plus `d22 10:21 GangJoin The Ash Brothers`, a summons excused by shift, `d27 23:00 Turf: ... spraying The Ash Brothers' mark`. fighter_61295 (Jocasta Sallow, 44 y) wins `d23 23:00` and `d25 23:00`, `+5c (GambleWin)`, and nothing else happens.
- **joytoy: thin (V15 thin).** joytoy_106038 six `served ... (visit)` lines in a week, four of them Perrin Mbeki; joytoy_66497 three. A regular with no consequence, no jealousy, no Fare, no kids line this time.
- **madam: alive (V15 alive).** madam_104391 `d27 00:00 booked 95 clients ... 726c through the house; 3 rough, thrown out`, six `roughed up ... threw him out and barred him` lines d24-d26, different victims. madam_35331 is the strongest arc in the group: `d21 14:45 Arrest ... for Theft at home in Block#2815` (2 food), `d24 00:07 Fire: dismissed as Madam (absent 3 times in 14 days, in the cells)`, `d24 16:40 came out to a changed life: the post as Madam ... gone`, `d25 13:31 tried out for The Grey Hounds ... turned away: the treasury could not feed another mouth`.
- **owner_broke: alive (V15 thin).** owner_broke_6471 (Nyx Greaves, 73 y): `d21 19:00 Wire Reyes ... demanded the 28c in wages owed ... left with nothing`, `d22 12:00 Anton Ashcombe swung ... won`, GrudgeFormed, `d23 00:07 UnderWater: cut the hours`, `d23 13:58 Quit: Kieran Wexford walked out ... owed 35c`, `d24 03:12 Cells: Anton Ashcombe learned who put him inside`, `d26 23:31 Wire Reyes walked up again ... Nyx Greaves won`, `d27 00:09 Coerced: ... the report was withdrawn`. A week with a beginning, middle and callback. owner_broke_6737 (Nerys Singh) `d23 00:07 Acquired: Vatra 2 bought Noodle Bar#5640 ... (a fire sale: 0c in hand against 15c a day in wages)`, then a shakedown report; short but good.

## 3. Round 7's mechanisms seen at 100k

Fired:
- W6 bout purse and hurt: fighter_40359 three Bouts each `purse 5c; ... hurt`; fighter_61295 two, `+5c`; analyze bout purses 0 -> 1,723 (10,720c), losers hurt 1,724 of 1,724.
- W6 patch-up after the pit: fighter_40359 `d25 12:46`, `d27 13:14` back-room Treated; `GOAL Treat ... hurt or stage 0.00->0.40`.
- W6 door "trouble": bouncer_92590 `d26 21:00`. analyze off-screen door broke 6,688, trouble 432, colours 299, barred 29; door_refusals 7,113 -> 8,927.
- W6 owner_broke callback (V15's missing sequel): owner_broke_6471 d21-d27, the creditor's man returns twice.
- W6 gossip fold: 5 GOSSIP ABOUT ME lines in the group, each one line with counts (`companion_102505 d25 05:05 told 2 times today (2 to new ears)`).
- W2 sleeper robbed wakes and tells: bouncer_58694 `d24 07:10`, `d25 03:00`, `d25 10:34`. Shakedown payer tells: cook_63885 within hours.
- W4 chrome cut by a doctor on duty: dancer_104874 `d21 12:31 Installed: ... Legs implant T1 ... by Amara Ironside (6c to the doctor)`.
- W3 arrest at home and a chase that gives up: madam_35331 `d21 14:45`, `Pursuit: Dahlia Kaneda gave up the chase ... (already in custody)`.

Expected, never seen: a Fight Pit colours or armed refusal in a diary (only off-screen counts); a named pimp's round (32 pimp-days of ~39 venues, none in these picks); a bet or main-event Bout (all undercard); a barred man returning (so V15's bug 2 is untestable); W1 `[family]` lines (no hungry household in the picks).

## 4. Bugs, ranked by what the player sees first

1. **The dancer commutes 2.5-3 h each way, and "moved nearer work" goes the wrong way.** dancer_104874 `Commute: 327 tiles, travel 143 min`; `d21 14:24 GoTo(Workplace) ... 184 min, 261 tiles`; travel 8.0 / 9.6 / 8.3 h on d23-d25. `d25 00:12 Housed: moved nearer work, into Block#2389` is in D1 The Sump, the club is in D6 The Vats (`d26 00:02 GoTo(Home) Club#5754 -> Block#2389, 82 min`); no work on d26. V15 bug 1 (far commutes): **unchanged**. Likely cause: the move cuts distance only, not district.
2. **A resident just moved sleeps at the old Home.** dancer_104874 `d25 02:25 Sleep @ Block#299`, `02:26 PLAN FAILED: Some(Sleep) failed at Sleep: PreconditionLost [bed Home: ]`, then a 13c cab to the new Block at `02:27`. Housed was at 00:12, so the plan walked to the old address. New.
3. **"he means to tell the law", then nothing.** cook_25080 `d24 13:08` and dancer_104874 `d22 10:36` print no Report or "kept quiet" through d27. W2's promise was a tell or a reason. **Partly better** (bouncer_58694, cook_63885 report).
4. **Every Bout hurts the loser.** analyze losers hurt 1,724 of 1,724. fighter_40359 (56 y) is hurt on d22, d24, d26 with a 6c patch against a 5c purse, so a fighter who loses is a net negative forever. The winner's purse is booked as `GambleWin` from the City (fighter_61295 `d23 23:00`), not a purse from the house.
5. **Bout lines carry no ages.** The brief wants every harm line to print ages; the Bout line prints names and purse only. All four fighters here are adults, but the hard rule is unreviewable on this line. New.
6. **A Fighter picks fighters.** fighter_40359 `d23 00:00 Hire: Flux Haas hired as Fighter (picked by Roxy Haas, the foreman)` and `00:10 Tank Fairweather ... (picked by Roxy Haas, the foreman)`: Roxy's job header is Fighter. W4 said no hire picked by a hand's trade. Her work also still reads `ClerkWork` at the pit (V15 bug 4's tail). New.
7. **Unexplained 6c treatment.** bouncer_92590 `d26 23:00 Treatment 6c to Tobin Yardley (off screen)`, purse 0c, no Bout or fight line in the diary. Low confidence on cause.
8. **Identical jail days.** madam_35331 d22 and d23 `sleep 8.0h, jail 16.0h`, Precinct#4485 1440m. V15 bug 6: **unchanged**, but the sentence now costs the post (the Fire line), which is a story.
V15 bugs: barred men back the same night **not reproduced**; one door reason **better**; fighter stakes **fixed**; brawl/grudge no pickup **better** (owner_broke_6471's grudge gets a second fight); gossip flood **fixed**.

## 5. Dull spots, ranked

1. **Cook, joytoy, club_staff_38885, companion_87767**: zero or one event line a week. Mechanism: a regular who turns up twice a week gets a want (a tab, a gift, jealousy of another regular) so "is now a regular" starts a thread; a cook or attendant is asked by a gang to look away or by the owner to short the till.
2. **An empty room never closes.** dancer_76642 dances 5 sets to an empty room five nights running for 1c. Mechanism: a venue empty N nights cuts hours or fires the dancer (the owner's `UnderWater` line already exists), so the dancer gets a Quit or a move.
3. **The same loser loses every night.** Roxy Haas 0 for 3. Mechanism: her gang or a creditor reads the loss (a Crew line), a bet book pays the crowd, a rematch is posted when a pair meets twice, a main event once a week.
4. **A winner has nothing to spend the win on.** fighter_61295 wins twice then `Food`. Mechanism: a promoter or gang recruits a streak winner (the GangJoin line exists only for the loser).
5. **The madam's roughed-up workers have no consequence for the house.** Mechanism: a worker walks out after two, or the madam hires a guard past two a night.

## 6. Too much

- Nothing reads as broken. The madam's 3-6 roughings a day is harsh but varied (different clients, different victims).
- Pit losers hurt 1,724 of 1,724 reads as monotone, not harsh (bug 4).
- Off-screen door refusals are still 6,688 of 7,449 "broke" (90%); the new reasons are a long tail, not a shift in the door.
- Barred 29 a month and no returner seen: the bar may now hold, or nobody comes back to be counted. One more window would tell.
