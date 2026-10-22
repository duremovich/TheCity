# Shadow V7 critique: vice (club_staff, bouncer, dancer, madam, companion, joytoy, fighter, cook)

70 pinned diaries read (s42d0 12, s43 16, s44 16, gen 16, k2 8) plus the 2 unpinned (nopin: dancer_8771, cook_8208), 504 pick-days.
Method: line counts per diary (served, Tip, MadamCut, Bout, Hire/Quit, BuyAsset, GoTo legs, late starts) by script over the `.md` files, the
`camera.tsv` rows for the same picks, and full days read: madam_16575 d32-d33 (s43), bouncer_21748 d2 (s42d0), joytoy_16188 d32 (s43),
joytoy_11880 d94 (s44), cook_10221 d91-d96 (s44), madam_25391 d30-d36 (gen), dancer_9249 d30-d33 (gen), cook_8222 d30-d36 (gen),
cook_16954 d0-d6 (s42d0), club_staff_38173 d95 (s44). Windows are named `s42d0` = isle_s42_d0, `s43` = isle_s43_d30, `s44` = isle_s44_d90,
`gen` = gen20k_s43_d30, `k2` = k2_s42_d26, `nopin` = isle_s43_d30_nopin. A cite is `window/file dD HH:MM`. Nothing below is inferred from a
Coarse or Statistical stretch; the only Coarse stretches in my 70 are the 19-minute sentencing ones (club_staff_38173 d95 15:41-16:00) and they are not used.
Pick-days with the pick jailed over 12 h are left out of the camera means.

Shape of the pass as it reads in this group: every venue trade now works its shift in 2-6 `ClerkWork` blocks, each behind a `GoTo(Station)` of 2-4
min, with a `Break` outside the door at mid-shift for the Club, Lounge, Dollhouse, Strip Club and Pit staff (not for bouncer, madam or cook, who
are `counter` shape and break only while another counter body covers). Median longest work block over the group's non-rest work-days is 85-89 min in every window
(V6: 538 on 50 of 52). The `[off]` narration is present on every settlement naming a Statistical person. What did not move is the part that
needs a customer body.

## 1. Per archetype

**Club staff: thin.** 9 diaries, 5 with any `served X (a customer, at the Attendant's counter)` line (6, 6, 1, 1, 15, 2 lines; 4 with none).
V6 had 5-18 a week in 8 of 8. Still a 21:00 batch, now tagged `[off]` and Statistical on every line (the counter window is for counter-shape
roles only; the Attendant is `stations`). The shift itself is the visible half: club_staff_7782 (gen) has 6 `Break`, 60 `GoTo(Station)`.
> `s43/club_staff_16717 d32 21:00` `[camera] [off] memory: served Priya Ravensworth#7549 (a customer, at the Attendant's counter), a regular ...; Priya Ravensworth#7549 Statistical (off screen)`, then the identical line as `[memory]`; a viewer sees 6 names at one minute and no body at the counter.
One of the 0-served is a retiree: s43/club_staff_7855 `d31 00:00` `Retired: Ines Osgood retired from Club Staff (to family)`, `d32 17:03` GangJoin, `d33 00:00` hired as Mechanic by the foreman.

**Bouncer: not alive.** 9 diaries (s42d0 2, s43 2, s44 2, gen 2, k2 1). 0 door refusals, 0 ejections, 0 `served` in the bouncer's own role,
0 occurrences of the word "door" in any of the 9. The shift is visible (2 blocks behind `GoTo(Station)` on the door tile, e.g. s42d0/bouncer_21748
d2 18:38 and 21:24: `ACTION ClerkWork @ Fight Pit#4807, 161 min` and `156 min`), but nothing happens at it. The one `refus` hit in the 9 is `Rejected of Bram Stroud` (a social
memory, s42d0/bouncer_17712 d1 09:22). The `refusal` column of `trades.md` is 0 in every row of every window, and a grep of all 6 windows' diaries for
"turned away / refused at / Refused / bounced" finds only gang-extortion lines (`Refused: Block#3389 would not pay Kasimir Mbeki`, s43/fixer_20376 d33 11:20) and a proposal. Caveat: a refusal sets the
patron's `Brain.refused`, so it could show in a patron's diary, and I did not read patrons; the bouncer's own diary and the roll-up have none.
> `s42d0/bouncer_21748 d2 18:41` `ACTION ClerkWork @ Fight Pit#4807 (D1 The Sump), 161 min`: no line in the 7 days that is an arrival, a verdict or a patron.
One bouncer is a different story: s43/bouncer_16307 is also a gang leader (`d34 12:56` `Collected: Odile Crane collected for The Unplugged 5: 0 to 0 members`, `d34 20:24` `Court ... 36 min`, `d35 09:14` a Lead round).

**Dancer: alive on 6 of 8 working dancers, thin on the island.** Tips on s42d0/dancer_8196 (2), s43/dancer_8771 (7), s44/dancer_17901 (6), gen/dancer_13142 (10),
k2/dancer_2272 (24); zero for s42d0/dancer_18464 and s43/dancer_12348 (both at a `Club`, 25-30 ClerkWork blocks, no tip, no `served`). The tips are now spread by arrival (k2/dancer_2272 d26
20:19, 20:32, 20:37, 20:38, 21:28, 21:30, one coin each) beside the old 21:00 batch (gen/dancer_13142 d30 21:00: 8 tippers in one line). V6 had 7 of 7 dancers tipped, 4-42 c a week; island
weeks are 2-13 c. Two further dancers quit the trade: s44/dancer_25531 `d93 00:00 Quit ... (no need of the wage)` at 165 c and gen/dancer_9249 `d32 00:00` (same words), and gen/dancer_9249 never worked
d30-d31 (work 0.0 h, 33-43 plans, `Wander` all day, section 6 bug 5).
> `k2/dancer_2272 d26 20:19` `[money] +1c (Tip) ... Tip +1 from Mina Abernathy#1823`, with `[off] ... Statistical (off screen)` on its twin line.

**Madam: alive for the house, not for the person, 2 of 8 lose the trade.** MadamCut in 6 of 8 diaries: s42d0/madam_19324 (21 lines), s43/madam_16575 (34), s43/madam_21552 (8), s44/madam_11518 (36),
s44/madam_24901 (12), k2/madam_1356 (16), gen/madam_23940 (4); s42d0/madam_9835 has 20 `oversaw` lines and no cut. `18:00` opens with the `oversaw X (a co-worker, the house's staff), a regular` set, all `[off]`. The foreman still hires madams off
their houses: gen/madam_25391 `d30 00:00 Hire: Priya Voss hired as Manager (picked by Cipher Lockridge, the foreman)`, `Promoted ... Manager at Lab#5077`, and from `d30 09:11` she works the Lab (stations, 36-80 min
blocks) with no MadamCut in 7 days; s43/madam_21552 `d36 00:00 Hire: Tank Fairweather hired as Manager ... Braindance Parlour#4635`. The madam's commute is the worst in the group (section 3).
> `s43/madam_16575 d32 23:10-23:31` six `[money] +2c/+3c (MadamCut)` lines, each with `[off] MadamCut 2c from; X Statistical (off screen)`, after two `ClerkWork` blocks of 150 and 202 min.

**Companion: alive, thin.** 7 diaries, 5 with tips (s43/companion_25693 6 lines, s44/companion_8699 1, gen 6085 2, gen 11146 1, k2/companion_2166 5), two with none (s43/companion_12794, s44/companion_10738).
`Served: X is now a regular of Y` fires (gen/companion_6085 `d31 21:00` `Cipher Penhallow is now a regular of Wisp Reyes at Spire Lounge#5184`). The repeat-tip line is unchanged (section 2).
> `gen/companion_6085 d31 21:00` `[money] +8c (Tip) ... events: Served "Cipher Penhallow is now a regular of Wisp Reyes at Spire Lounge#5184"; flows: Tip +4 from Cipher Penhallow#17896, Tip +4 from Cipher Penhallow#17896`

**Joytoy: not alive as the trade.** 7 diaries (s43 2, s44 2, gen 2, k2 1), 0 client memories, 0 tips in, 0 `served` in any; the income is the Wage line (49 c a week, s43/joytoy_16188) and the money
table shows only Wage in. The shift is now visible and well-shaped, and still empty: s43/joytoy_16188 d32 18:03 is three `ClerkWork` blocks (37, 62, 28 min) behind `GoTo(Station)`, a `Break` at 20:26 (27 min, outside), then three more
(56, 90, 26 min), and no person appears in the shift. The chip arc is present and costless: gen/joytoy_14646 `d36 00:00 Quit: ... chip contract at Dollhouse#4715 ended (served, 30 days)` then the foreman moves the pick to Night Stocker in the same minute;
gen/joytoy_12542 `d30 00:00 Quit: Santiago Hawthorne's chip contract ... ended (cut short, 1 days)`, then 7 days with work 0.0 and street 14-20 h until a re-hire at `d36 00:00`. Oddity: the pick of gen/joytoy_14646 is named as "the foreman" in the
`d33 00:00` hire line `Hire: Vex Inchbald hired as Contract Host (picked by Garrick Achebe, the foreman)`, a Contract Host acting as the hirer of his own Dollhouse's next Host; no step shows it.

**Fighter: thin.** 9 diaries. `Bout:` at 23:00 every second night, `bets paid 0` on every bout line in all 5 windows (s42d0 d0/d2/d4, s43 d30/d32/d35, s44 d91/d93/d95, k2 d26/d29/d31, gen d30-d36). One fixed pair per Pit (s44 Pit#5204 Ada Novak v Benedikt Greaves
d91, d93, d95 with the result flipping, Pit#5661 Kieran Bartlett v Casimir Fairweather d92/d94/d96). The fighter's shift is stations (6 `Break`, 52-58 `GoTo(Station)` a week) and the bout posts while the fighter is mid-`ClerkWork`, not fighting:
`s43/fighter_8413 d30 22:34` `ClerkWork @ Fight Pit#5202, 86 min`, `23:00 Bout: Aldo Ambrose beat Jocasta Pembroke at Fight Pit#5202 (bets paid 0)`, `memory Bout of Aldo Ambrose#19398 (salience 0.50, valence -0.40)`.

**Cook: alive for evening cooks, partly alive for day cooks.** 9 diaries: 6 evening (shift 18:00-24:00), 3 day (09:00-18:00: s43/cook_8208, s44/cook_10221, k2/cook_2557). The counter window works: gen/cook_8222 `d30 20:45`, `21:30`, `d31 20:45`, `d32 21:18`, `d33 22:15`, `22:30`, `22:45`,
`d35 20:30`, `22:45`, `23:15`, `d36 20:00`, `20:45`, `21:00` = 13 `served ... at the Cook's counter` spread over the evening (V6: all at 21:00). A day cook is served now: s43/cook_8208 `d33 17:15` `served Alaric Mbeki#11456 (a customer, at the Cook's counter)`, k2/cook_2557 `d26 11:37`.
But one served line in 7 days is not a trade; s44/cook_10221 has none (and is hired off, section 6 bug 2). Evening cooks: s42d0/cook_17900 6, s43/cook_15619 6, gen 8222 13, gen 15083 1, s44/cook_20073 0. s42d0/cook_16954 is hired as Researcher at `d0 00:00` (section 4).
> `gen/cook_8222 d33 22:15` `[camera] [off] memory: served Sebastian Ellery#15212 (a customer, at the Cook's counter), a regular ...; Sebastian Ellery#15212 Statistical (off screen)`, 15 and 30 minutes before the next two.

## 2. V6 rows and bugs touching the group

| V6 item | verdict | evidence |
|---|---|---|
| Row 1: shift is one statue action (OPEN-137) | **fixed** for 6 of 8 archetypes; **unchanged** for the cook's day shift and any pick moved to a farm | Median longest work block 85-89 min per window; stations 2-3 tiles, 4-6 `Break`s a week per venue staff. Residue from the roll-up: bouncer 186/234 (median/max), madam 192/235, cook 224/538 (31 blocks over 180). s44/cook_10221 d93 `ClerkWork 322 min` and `212 min`, d96 `FarmWork @ Vat Farm#4881, 538 min` (one tile, awake still 538). Counter-shape roles (bouncer, madam, cook) never break and have one spot, so a madam's block is 150-229 min on one tile. |
| Row 2: silent idle freeze (OPEN-138) | **fixed** in this group | Non-jail awake-still maximum in the group is 538 min (the farm block above) and 453 (s43/bouncer_16307 d35, a 453-min travel). 0 days of 12 h or more outside jail. The cooldown line now exists: `goal Unwind cooled until d2 13:32 (could not plan or plan failed twice)` (s42d0/bouncer_21748 d2 11:32), 57 on the island, 58 gen, 22 k2. |
| Row 3: settlement by presence (OPEN-119/120/145) | **partly fixed** | Every settlement naming a Statistical person carries `[off] ... Statistical (off screen)`, `apart` 0.00-0.02 per pick-day on the island (except gen, 0.24, which is dancer_9249's 211 `Finance` flows, a ledger line). Counter-shape roles now draw spread slots (gen/cook_8222, above) and the day counter opens (cook_8208 d33 17:15). Not fixed: the Attendant, dancer and companion counters still post at 21:00 as one batch (gen/dancer_13142 d30 21:00 8 tippers; s43/club_staff_16717 d32 21:00 6 names), Statistical only. The `served` lines are narrated off screen as designed (P3); the customer is still never a body. |
| Row 4: long legs (OPEN-139) | **worse for non-work legs, unchanged for the commute, fixed for the escort** | GoTo steps over 120 min in the group: island 356 of 2,349 (15 %), gen 101 of 943 (11 %), k2 14 of 468 (3 %). By target on the island: Workplace 118 (max 744), Home 83 (max 901), Spot 82 (max 537), Market 24 (max 341), Hideout 17 (max 830), Hall 3 (max 900). Median GoTo(Workplace) 46 min, p90 319, max 744 (gen 32 / 173 / 901, k2 35 / 76 / 180). s43/madam_16575 d33 is 422 min to a hangout spot and 436 min to work (section 3). No body jump in the group on the island except 1 arrest (s44/club_staff_38173 d95 15:41, 199 tiles); gen 2 (dancer_9249). |
| Row 5: housing churn (OPEN-140) | **partly fixed** | 0 `NoSuchPlace` and 0 double `Housed` in the 70. `GoTo(Hotel)` resolves to a Capsule Hotel every time (V6: a Soup Kitchen 27 times in the 400). But the hotel can be 380 min away: s44/joytoy_11880 d93-d96 00:02 `GoTo(Hotel) @ Dollhouse#5614 (D3 Neon Quarter) -> Capsule Hotel#5529 (D1 The Sump), 380 min`, four nights in a row, with her own Home Block#276 (D5 The Spire) untouched in the header. |
| Row 6: bouncer door (OPEN-121) | **unchanged: not alive** | 0 refusals, 0 ejections in 9 bouncers (section 1). The spec's `door_seen`/`service::refused` path has no output a bouncer's diary can show. |
| Row 6: joytoy's clients | **unchanged: not alive** | 0 client lines in 7 joytoys (V6 0 of 5). Confirmed the runs-report claim. |
| Row 7: implant loop (OPEN-142) | **fixed** | 0 `PLAN FAILED ... BuyAsset` in the 70 diaries (V6: 6 failures in dancer_17341, 2 each in madam_15417 and cook_8872). 7 completed installs, each `ACTION BuyAsset @ Ripperdoc, 141 min (ended)` then `AssetBought` and `Installed` (s42d0/madam_9835 d4 13:33 -> 15:54 `Installed: Dagny Wexford had Legs implant T1 installed at Ripperdoc#5038`). One cost I could not explain: 141 min in a chair is still on the pick's shift (it ends `Shop -> Work` at 15:55). |
| OPEN-143 foreman hires a madam off her house | **unchanged / worse** | 2 of 8 madams on the island and gen (gen/madam_25391 `d30 00:00`, s43/madam_21552 `d36 00:00`). Across all 8 archetypes 6 of 70 diaries lose the trade to a foreman hire: s42d0/cook_16954 `d0 00:00` Researcher; s43/club_staff_7855 `d33` Mechanic; s43/madam_21552; s44/bouncer_12121 `d96 00:00 Hire: Bex Vance hired as Dancer`; s44/cook_10221 `d95 00:00` Vat Tech; gen/joytoy_14646 `d36` Night Stocker; gen/madam_25391. |
| OPEN-122 bouts: one pair, no bets; V5 bug 10 spouse-or-friend | **unchanged** | `bets paid 0` on every line; the same pairs on the same nights (section 1). Relationship check not repeated. |
| Triple tips (V5 bug 16) | **unchanged** | k2/companion_2166 d28 21:00 `Tip +4 from Zoe Lockridge#2489` x3, d31 x3, d32 x3 (3 of 5 tip nights); gen/companion_6085 d31 x2; gen/companion_11146 d32 `Imogen Northcott` x2; s44/companion_8699 d95 `Rhys Kowalczyk` x2. Not on the island's s43 companions. |
| V6 bug 6: late first evening, shift start does not interrupt sleep | **unchanged** | Evening first-block starts after 18:10: island 99 of 240 (41 %, V6 41 %), s42d0 36 of 71 (51 %), gen 28 of 82 (34 %), k2 15 of 47 (32 %). Bouncer s42d0: 10 of 12; madam s44: 8 of 12. |
| V6 weekly rest day (day % 7 == 6) | **unchanged** | 67 of 68 group pick-days on that day have work under 0.5 h (the exception is a jailed pick). |
| V6 bug 7 stims-sale label (`served-pair ... kind and side unknown`) | **worse** | 9 lines in 70 diaries (V6 4 in 54): s43/fighter_15427 d30 13:22, d34 14:05; s44/fighter_7364 d93 12:57; gen/club_staff_7782 d34 17:26; k2/bouncer_2183 d28 13:58, d30 15:48, d32 13:59; k2/fighter_1763 d27 16:47. Camera-tool label gap. |
| V6 bug 8 idle Rest loop | **unchanged** | gen/dancer_9249 has 34 one-minute `ACTION Rest` lines at home (d30 20:34-21:09 and on) inside her own 18:00-24:00 shift; s44/dancer_25531 28. Both are the "employed on paper" pair of dancers above. |
| V6 `nobody there` / OPEN-109 | **not observable as a fix** | `HANGOUT ... nobody there`: island 78 of 477 (16 %), gen 89 of 229 (39 %), k2 22 of 117 (19 %). |
| Statistical bodies on screen (OPEN-149) | **unchanged by design** | nopin: dancer_8771 and cook_8208 are Statistical all 3 days (both `stat` all 72 h, awake still 4,320 min at `street (D4 Mid-Levels)` / `street (D3 Neon Quarter)`). dancer_8771 gets stand-in tips from the same patrons as its pinned twin (`d31 18:30 Tip +2 from Katrin Ferreira#21189`, `d31 22:00 Tip +2 from Maren Okafor#8383`); the cook gets no `served`. |

## 3. The camera: the three worst continuity moments

**1. The madam who walks 14 hours to work 85 minutes (s43/madam_16575 d33 08:10-22:35).** She sleeps at Block#3677 (D1 The Sump) to `07:40`, then `d33 08:10` `GoTo(Spot) @ Block#3677 -> street (D1 The Sump), 422 min (done)`: `now at street (D1 The Sump)` at `08:11`
and then nothing until `15:12`, when a HangOut lasts 3 min. `15:15` `GoTo(Workplace) @ street (D1 The Sump) -> Brothel#5771 (D6 The Vats), 436 min`, `21:45 now at street (D6 The Vats)`, `22:31 now at Brothel#5771`, `22:35 ClerkWork, 85 min`, off at 24:00. A viewer follows
a 29-year-old madam on foot for 14 h through two districts and sees her work 85 min of an 360-min shift; her camera row reads `vehicle 12.6 h`, `awake still 381 min travel` (d33), and d36 repeats it (`vehicle 12.5 h`, 381 min). The d32 before it is the same shape with shorter legs
(`GoTo(Market) 120 min`, `GoTo(Seller) 137 min`, `GoTo(Workplace) 104 min`, work 150 + 202 min). Only the Workplace legs are the commute; the 422-min GoTo(Spot) is a free-time pick that crossed a district.

**2. The dancer who quits and walks 15 hours to a registry (s44/dancer_25531 d93 00:01).** `d93 00:00 Quit: Rosalind Lindqvist quit as Dancer (no need of the wage)`, purse 165 c, `00:01 GOAL Found`, `PLAN Found [GoTo(Hall) > Register]`,
`ACTION GoTo(Hall) @ Block#613 (D4 Mid-Levels) -> street (D2 Civic Core), 900 min (ended)`. Camera d93: `street 16.6 h`, `vehicle 7.3 h`, `home 0.0`, awake still 440 min travel. The next three nights she is at a Capsule Hotel (`GoTo(Hotel)` d94 16:01, d95 23:02, d96 15:52) and `home` is 0.0 for the rest of the window,
though the header still names Block#613. A viewer sees a woman leave her job and her flat in one minute and cross the city for 15 h on one step.

**3. The abducted dancer teleported to a hideout (gen/dancer_9249 d32 12:56-15:31).** `12:56 Abducted: Vex Novak abducted Elowen Ravensworth in Sump 3`, `14:00 now at street (Sump 2)`, `14:30 now at Hideout#4186 (Sump 2)` with
`[camera] JUMP (a Full body) 15 tiles in 30 min (allowance 2): street (Sump 2) -> Hideout#4186; LOD Full; exec Idle, no step`, `15:00 sighting of Vex Novak at Hideout#4186`, `15:31 Harvested: Vex Novak ripped 1 implants from Elowen Ravensworth`. A viewer sees a street, a gap with no abductor or vehicle, then a hideout. It is a good story (the best in the group:
abduction, harvest, a second jump `d33 21:01` of 103 tiles with `exec Idle, Sleep`, then a night in Block#2100 (Mid 8), not her Home Block#480) and the continuity is a cut. Honourable mention: s44/club_staff_38173 d95 15:41 `JUMP 199 tiles in 193 min ... Block#3263 (D3 Neon Quarter) -> Precinct#4460 (D2 Civic Core); LOD Full -> Coarse, sentenced; exec Idle, no step`: the arrest shape is the same as V6's worst moment (a body at home, then a cell).
V6's moment 2 (a 300-tile prisoner transfer) and moment 3 (the night walk to a capsule hotel) are not in this set at that size; the hotel walk is now joytoy_11880's 380 min, listed in section 6.

## 4. The first week (s42d0)

12 pinned diaries, none from a Coarse stretch (none had one).
- **The boot ladder is unchanged.** 12 of 12 diaries begin `d0 00:00 GOAL Idle: score 0.050 ... constant`, `PLAN Idle [Sleep]` 44-45 min, then 3-5 `ACTION Rest 90 min` blocks before 10:00 (bouncer_21748 5, bouncer_17712 4, madam_9835 5, cook_16954 3 ...). V6: 11 of 12.
- **The foreman at midnight is still there.** s42d0/cook_16954 `d0 00:00 Hire: Marek Blackwood hired as Researcher (picked by Viktor Bartlett, the foreman)`, `Promoted ... Researcher at Lab#5727`; the cook never works a Noodle Bar again. It reads as a life, not a bug: day shift 09:02-17:21 in stations (`ClerkWork` 87, 42, 88, 38, then 90, 45, 85 min), a 78-min commute, 9.1 h work a day.
It is the V6 madam_17666 case (a trade person promoted off the trade on day 0), on a cook.
- **Day 0 housing and walks.** The group's d0 `home` is 8.9 h a day on average at s42d0 against 7.5 and 9.7 at s43 and s44 (V6 d0 12.6 h against 8.0 and 10.2). `street` 7.9 h (s43 9.2). GoTo(Workplace) median 37 min, p90 348, max 500 (V6 s42d0: 50, 83, 147). So the d0 island city has longer commutes than the generator's d0 had: s42d0/bouncer_21748 d2 `GoTo(Workplace) 244 min`, `d2 06:52 GoTo(Spot) 257 min`, so a bouncer
walks 4 h to a hangout and 4 h to the Pit and works two blocks (161 and 156 min) at it.
- **Early crime.** s42d0/bouncer_21748 has `d2 00:00 heard a rumour: Arrested by Juno Stroud ... [deed from d1 16:08]`; the arrest is a rumour, the pick is back at work at d2 18:34. No d0 gang for this group.
- **The Ripperdoc loop is gone on d0.** s42d0/madam_9835 `d4 13:33 BuyAsset 141 min` completes with `Installed` (V6's dancer_17341 repeated it three days and never installed); s42d0/club_staff_12232 d1 15:38 the same.
- **Missing:** no companion or joytoy exists at d0 (19 archetypes have no candidate, as V6), so those two archetypes have no first-week evidence. Fighters at d0: 6 `Bout:` lines per pick, Pit#4978 Agathe Park v Bronwen Wexford and Pit#5661 Kira Tanner v Smoke Oakes, d0/d2/d4, `bets paid 0`.

## 5. The island against gen, and against the 2k control

Means per pick-day for the group, excluding jailed days over 12 h (script over `camera.tsv`): s42d0 home 8.9 / work 4.8 / street 7.9 / vehicle 1.7; s43 7.5 / 4.6 / 9.2 / 1.5; s44 9.7 / 4.5 / 6.7 / 1.1; gen 8.5 / 4.4 / 8.7 / 0.7; k2 10.8 / 5.3 / 6.2 / 0.4.
- **Vehicle.** The island rides 1.1-1.7 h a day against gen 0.7 and k2 0.4. The rides have not shortened the legs: island GoTo over 120 min is 1.0-1.3 a pick-day against gen 0.9 and k2 0.25. The 2k city's legs are 5-180 min; the island's reach 744-901. 40 % of the island's long legs are to Workplace or Home, a third are to a hangout Spot (82 of 356).
- **Trade output.** MadamCut: island 21, 42, 48 lines (s42d0, s43, s44), gen 4 (one madam hired away, the other in a quiet house), k2 16. `served` memories per diary: island 14, 20, 2 in s42d0/s43/s44 (16 picks in s44: 2 lines, both single), gen 50 (three diaries hold 44 of them: club_staff_7782 15, dancer_13142 16, cook_8222 13), k2 18 in 8 diaries.
So a venue worker on the island meets fewer posted customers than gen or the 2k city (s44 especially), while the brothel still pays the madam on the island (MadamCut 48 in s44). I do not know if it is demand (the island's `served_tenant` fell 77 %) or the counter rule; both are Statistical-side.
- **Hangouts.** The island's hangouts are emptier of company less often (`nobody there` 16 %) than gen's (39 %), and it has fewer idle `Wander` blocks (288 over 44 picks, 6.5 a pick, against gen 274 over 16, 17 a pick, k2 114 over 8, 14 a pick). That reads as a busier island at the picks' hours; the cause is not visible in these diaries.
- **Control vs the window sizes.** k2 is the only window where every pick works every non-rest day (zero-work days 0 of 46; island s43 11 of 96, s44 5 of 90, gen 10 of 94). The island's zero-work days are the quitters (dancer_25531, dancer_9249), the jailed, and the foreman's hires.
- **Hotel and housing.** The long hotel leg is island-only (joytoy_11880, dancer_25531); gen's `GoTo(Hotel)` legs are 129 min at most, k2's 2 legs are short.
- **Chip, jail, foreman.** Both chip arcs are gen (joytoy_12542, joytoy_14646); the island's joytoys show none in 14 pick-days. Jail: island 2 (club_staff_38173, dancer_17901, both d95), gen 1 (fighter_8133 d30), k2 2 (club_staff_1553, fighter_1763). Foreman hires: island 5 of 44 diaries, gen 2 of 16, k2 0 of 8.

## 6. New bugs, ranked by what a followed character sees first

1. **A free-time leg of 300-900 min, then a work arrival hours after the shift began** (s43/madam_16575 d33: `GoTo(Spot)` 422 min, `GoTo(Workplace)` 436 min, work 85 min; s44/dancer_25531 d93 00:01 `GoTo(Hall)` 900 min; s42d0/bouncer_21748 d2 `GoTo(Spot)` 257 + `GoTo(Workplace)` 244; s43/joytoy_16188 d32 10:18 `GoTo(Spot) 360 min`, cut off by the shift at 16:18 after six hours on one street). Island 82 hangout-spot legs over 120 min of 477 hangouts' worth, max 537. The phase-5 legs work covers the commute; the spot picker (`[unwind] UNWIND pick: ... HangOut at spot (x,y)`) has no distance cap, and the other free-time targets
   (`Hall`, `Hideout`, `Clinic`) are as long. Likely anchor: the `unwind` pick and `GoTo(Spot)` in `goap/` or `systems/` (I did not read the code); the leg-replan threshold for non-commute `GoTo`s.
2. **The foreman takes people off their trade** (6 of 70 vice diaries, section 2): a madam becomes a Lab Manager with no cut after, a bouncer becomes a Dancer, a cook becomes a Vat Tech after quitting `(unpaid)`, a club attendant becomes a Mechanic, a Contract Host is the hirer.
   A followed madam stops being one at `00:00` with no resignation line. OPEN-143 with a bigger count; no archetype guard in the foreman's pick (not read).
3. **Nightly hotel 380 minutes away** (s44/joytoy_11880 d93-d96 00:02, four nights, `vehicle 1.9 h`, `fare -1c`): she sleeps 06:28-13:50, walks 70 min and then 168 min to work, arrives 17:48, starts 18:03. The hotel is chosen by price or availability with no distance term, and the pick's own flat is never used. Likely anchor: `Hotel` resolution in the Sleep plan (`life.rs`/`goap/` housing path of phase 4; not read).
4. **A bouncer is a statue at a door that never opens** (9 of 9, OPEN-121). The shift moves between 1-2 tiles and shows nothing. Not fixable from a diary; the phase-3 `door_seen`/`service::refused` rule needs a patron who reaches a Club door. I saw no patron-side evidence either way.
5. **A trade person with the job on paper who never works: gen/dancer_9249 d30-d31** (work 0.0 h, 33-43 plans a day, `PLAN Idle [Wander]` every 32 min from 08:09, 34 one-minute `Rest`s at home during 18:00-24:00; she runs 110 `Finance` flows a day and quits `no need of the wage` at d32 00:00). The shift-start goal (`GOAL Work`) never wins for a rich pick, and the planner then idles. A viewer sees a woman at home in her work hours. s44/dancer_25531 d93 is the same quit at 165 c.
6. **The shift pays a cook who is paid by a person and then loses the job unpaid** (s44/cook_10221 `d94 18:00 Quit: Xia Ostrowski quit as Cook (unpaid)`, with `Wage 9c from Garrick Lindqvist#12776` posted the same minute and a purse at 0 c all week; and on d91 and d93 `[money] -1c / -12c (Wage) ... Wage -12 to Ivo Lockridge#39314` out of a 0-purse cook, which I cannot explain from the diary). It is a wage that moves both ways in one week. Not read in code.
7. **Arrest and abduction remain cuts** (s44/club_staff_38173 d95 15:41 199 tiles, gen/dancer_9249 d32 14:30 and d33 21:01). 2 of 3 group body jumps are not arrests (an abduction and a sleep walk). The abduction needs a transport beat; the second jump is a `GoTo(Home)` that lands 5 tiles over the allowance (103 against 98), likely the tool's allowance, not the sim.
8. **Customers never reach the counter on the island's s44 window** (2 `served` lines in 110 pick-days against gen's 50 in 112; island s42d0 14 and s43 20). The brothel still pays (MadamCut 48 in s44). Count of posted lines, not an inference from absence; the cause is not in these diaries (OPEN-157 and `served_tenant` -77 % are the nearest numbers the runs report gives).
9. **Triple tips and the stims label** (section 2): unchanged and worse, no new cost to a viewer.
10. **Parity** (outside my reading): the runs report's three parity rows (paid venue visits 12.3 vs 14.2) point at the same thing as 8: fewer paid visits at the venue in Full than Statistical. I only note it; the diaries don't test it.
