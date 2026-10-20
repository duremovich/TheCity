# Shadow V6 critique: vice (club_staff, bouncer, dancer, madam, companion, joytoy, fighter, cook)

54 diaries (52 pinned, 2 unpinned) read by camera block, the camera.tsv rows, and grep/sed on served, settled, Tip, MadamCut, Bout, JUMP, plan-abort
and Hire lines. Full days read: madam_24700 d31 and d34-35, madam_17666 d0-2, bouncer_19587 d3-4, club_staff_7615 d32,
companion_6085 d30, companion_11406 d34-36, joytoy_14648 d31, joytoy_1056 d31, fighter_8373 d30, fighter_1763 d26-27,
cook_8497 d32, dancer_9076 d32, dancer_17341 d0-1, club_staff_11674 d0.

Windows: `s42d0` = k20_s42_d0 (12 diaries, no companion or joytoy candidate at d0), `s43` = k20_s43_d30 (16), `s44` =
k20_s44_d90 (16), `k2` = k2_s42_d26 (8), `nopin` = k20_s43_d30_nopin (dancer_9076, cook_8497). Cites are `window/file dD HH:MM`.
Every pinned pick is Full all week; only the arrest drops it to Coarse for 22-33 min, and that stretch is never used as
evidence. 50 of 52 picks work `[(1080, 1440)]` (18:00-24:00); the exceptions are the two day-shift cooks
(cook_8497 s43, cook_8872 s44: `[(540, 1080)]`). The weekly rest day is day % 7 == 6 for every pick (w 0.1 h, 90 min idle).

## 1. Per archetype

**Club staff: alive as the trade, thin in variety.** 5-18 `served X (a customer, at the Host's counter)` memories a week,
2-4 regulars, new Acquaintance edges the minute they are served. Every one is stamped 21:00 and every customer is Statistical
(`settles apart` 0 in all 8 diaries); the staffer is one 357 min ClerkWork block with no bouncing, no refusal. 22 of 42
club shifts start after 18:10 (walks of 51-114 min), see section 2 bug 6.
> `s43/club_staff_7615 d32 21:00` `[memory] served Vivienne Carrow#6291 (a customer, at the Host's counter), a regular`

**Bouncer: not alive.** 7 diaries, 0 door refusals, 0 ejections, 0 `served` memory in the bouncer's own role. Every
`served` line is the bouncer being served (`was served by ... the Bartender` at 11:17-16:22) or a street stim sale labelled
`served-pair ... kind and side unknown`. A bouncer at a Fight Pit does the same ClerkWork block as a clerk. The one story in
the group's d0 week (bouncer_19587) is a theft, not the door. OPEN-121 is open and unchanged.
> `k2/bouncer_2183 d26 11:17` `[memory] was served by Raven Moreau#2497 (as a customer; ... the Bartender)`, with no line in 7 days that is the bouncer serving

**Dancer: alive.** Tips in all 7 dancers' diaries (4-42 coins a week, 3-30 distinct tippers), each tipper a `served ... at the
dancer's counter` memory, 1-7 flagged regular. Strip Club, one ClerkWork block, tips posted at 21:00 in one batch
(dancer_9076 d32: 7 tippers in one minute). The patrons never appear as bodies. s44 dancers are thin (3 tip nights, 4-6c).
> `s43/dancer_9076 d32 21:00` `[money] +14c (Tip) ... Tip +2 from Odile Varley#5921, Tip +2 from Hesper Nightingale#7299, ...` then `served ... a regular`

**Madam: alive for the house, thin for the person.** OPEN-123 is fixed: MadamCut 1-2c per worker per night, 4-6 workers a
night, posted 23:01-23:55 as `SETTLED APART: MadamCut 1c from X Statistical` (s44 madam_24444: 29 lines in 6 nights;
k2 madam_1356: 27; s43 madam_25391: 6, then none). 18:00 opens with 3-8 `oversaw X (a co-worker, the house's staff), a regular`
memories. But the madam is not kept at the house: 2 of the 6 20k madams (s42d0/madam_17666, s43/madam_25391) are hired off to a Vat Farm by the
foreman (section 6, bug 1), and madam_24700 (s43) has 34 `oversaw` lines and one MadamCut in five nights.
> `s44/madam_24444 d91 23:02` `[money] +1c (MadamCut) ... flows: MadamCut +1 from Tess Northcott#33902`

**Companion: alive.** Tips 12-60c a week, `Served: X is now a regular of Y` events in 3 of 5 diaries (s43 companion_6085 d30
21:00, d36 21:00; s43 companion_11406 d32 21:00), regular memories, a Friend edge forming in the same shift (companion_6085
Silvio Tanner Acquaintance +0.15 -> Friend +0.45 d30 19:16-22:16). Flawed by the repeat-tip line (section 6, bug 3).
> `s43/companion_6085 d30 21:00` `[event] Served: Zara Blackwood is now a regular of Wisp Reyes at Spire Lounge#5184`

**Joytoy: not alive as the trade.** 5 diaries, 0 client memories, 0 tips in; income is the Wage line (5-13c). The only
people named are the super calling at the same minute every morning (`was called on by ... (as a tenant; my super)`
d26-d32 09:40 in k2/joytoy_1056, d91-96 09:01 in s44/joytoy_22922) and one bartender who served the joytoy. The chip story is
the single life event: k2/joytoy_1056 d31 00:00 `Quit: ... chip contract at Dollhouse#521 ended (episode, 22 days)`, then
`Episode: went berserk in Vats`, five Assaults, one Murder (d31 04:28), Arrest d32 00:36, 10 days. It reads as a life, but
the trade before it is invisible. s44 joytoy_22922 d91-92: two `Hire ... Contract Host (picked by Wren Fairweather, the
foreman)` lines naming other people hired into the pick's own Dollhouse; the pick never works with them on screen.

**Fighter: thin.** Bout line at 23:00 every second night, one fixed pair per Pit (section 2). Between, a ClerkWork block at
the Pit and a long walk home. 36-78 edges, 1-3 Friends at +1.00; 7-25 HangOuts a week of which 3-18 are `nobody there`
(s43 fighter_8373: 18 of 25). One 2k homeless week (k2/fighter_1763 d27-31: home 0.0 h, d27 slept 786 min on street
(Sump East)) is the most human fighter story in the set, and it is the rest day, not the trade.
> `s43/fighter_8373 d30 23:00` `[event] Bout: Hiro Okafor beat Delia Moreau at Fight Pit#4844 (bets paid 0)`

**Cook: alive as work for the evening cooks, not alive for the two day-shift cooks.** The 5 evening cooks: 4-12 `served X
(a customer, at the Cook's counter)` at 21:00, 0-2 regulars. No cooking action in 8 diaries (only `PackMeal` for the cook's
own lunch). The two day-shift cooks (cook_8497 s43, cook_8872 s44) work 09:02-18:00 in one 538 min block and serve
**nobody**: 0 `served` memories in 7 and 6 days. Counters settle at 21:00, which is after their shift.
> `s43/cook_14716 d35 21:00` `served Leopold Hallow#24616 (a customer, at the Cook's counter), a regular`; against `s43/cook_8497 d32 09:02` `ACTION ClerkWork @ Noodle Bar#5340 (Mid 6), 538 min` and no served line d30-d36

## 2. V5's bugs that touch this group

(SHADOW_V5.md numbering; the vice critique's own numbers in brackets.)

| V5 bug | verdict | evidence |
|---|---|---|
| 1 [vice 1] Bar staffed in the evening, drinkers by day (OPEN-111, OPEN-114) | **fixed** (regular formation partly) | 20 Bar drinks in my diaries, 19 by day (s42d0 4, s43 6 of 7, s44 4, k2 5); 26 `was served by ... the Bartender` memories, at 11:17, 11:28, 11:45, 12:55, 14:10, 15:32. Regular flags: 2 of 26 (`s44/bouncer_34066 d95 12:21` Kofi Penhallow). OPEN-111 stays partly open: the day Bar regulars are rare. |
| 2 [vice 2] / V5 bug 10 Bouts: one fixed pair per Pit, no bets (OPEN-122) | **unchanged** | 3 bouts a week per pair, every second night (d0/2/4, d30/32/35, d91/93/95, d26/29/31): s42d0 Pit#4838 Lavinia v Rhoswen, Pit#5690 Sprocket v Ronin; s43 Pit#4844 Hiro v Delia; s44 Pit#4886 Hex v Freya, Pit#5669 Ada v Kieran; k2 Pit#578 Orrin v Zelda. 21 of 21 `bets paid 0` in the 7 fighters' diaries. |
| 3 [vice 3] / V5 bug 10 the opponent is spouse or best friend | **unchanged** | s43 fighter_8373 and 15404 *are* the pair, `[Friend] affinity +1.00` at the end of d36 (`d36 07:31` HangOut together); s44 fighter_7084 `Ada Ambrose#21737 [Friend] +1.00`; s44 fighter_14233 `Hex Singh#22646 [Friend] +1.00`. Winner memory +0.30, loser -0.40; edge never moves. |
| 4 [vice 4] / V5 bug 16 one patron tips three times a night | **unchanged, and now seen at 2k** | 3 of 3 tip nights s43/companion_11406 (`d30 21:00` `Tip +4 from Gideon Bellamy#8496, Tip +4 from Gideon Bellamy#8496`), 5 of 6 s43/companion_6085 (`d30 21:00` Zara Blackwood x3, 12c), 2 of 3 k2/companion_2034 (`d26 21:00` Brenna Osgood x2). s44 companions and all dancers: 0. No OPEN row. |
| 5 [vice 5] every venue trade is one 18:00-24:00 ClerkWork block | **unchanged** (2 of 52 picks differ) | 297 ClerkWork blocks in 52 diaries, 189 of 340-360 min (the rest are shortened by a late start); the two day-shift cooks get a 538 min block. The madam's d32+ day at Vat Farm (s43/madam_25391, 538 min) is a different job, not a different block of the trade. |
| 6 [vice 6] a late first evening, shift start does not interrupt sleep | **unchanged, worse as a pattern** | s43/club_staff_7615 `d32 15:54` Sleep 220 min, `d32 19:36` GOAL Work, `20:27` ClerkWork 213 min. Of 282 evening ClerkWork starts in my 52 pinned diaries, 116 (41 %) begin after 18:10 (club staff 22 of 42, bouncer 13 of 40, madam 15 of 30); club_staff_11674 s42d0 is late 6 of 6 days (18:39-20:22). Walks of 51-114 min precede them. |
| V5 madam 18 / OPEN-123 no cut | **fixed** | MadamCut lines in 5 of 7 madam diaries; see section 1. |
| V5 bartender/server regular (gate item) | **fixed for Club/Lounge/Strip Club/Brothel**, same as V5 | |
| V4 bug 7 / OPEN-008 dancers never tipped | **fixed** (held) | 7 of 7 dancers tipped. |
| V5 joytoy no client / no chip cost (OPEN-037) | **unchanged** | no client memory in 5 diaries; the chip contract ends with no cost line (k2/joytoy_1056 d31 00:00). |
| OPEN-121 the door refusal never fires (bouncer has no door) | **unchanged** | 0 in 7 bouncer diaries. |
| OPEN-109 street life | **not observable as a fix** | HangOuts `nobody there`: s43/fighter_8373 18 of 25, cook_8497 14 of 22, bouncer_15446 14 of 24. Not a bug in the set, it is the OPEN-109 shape. |
| OPEN-115 a body never drives the cab | **not observable** | no cabbie in my group. |

## 3. The camera: the three worst continuity moments

**1. The sleeper in his bed becomes a prisoner in another district (s42d0/bouncer_19587 d4 03:37).** Bouncer Flux Thorne
sleeps at Block#1451 (Sump 3) from 01:25. `d4 02:36 Arrest: Hex Bellamy arrested Flux Thorne` happens with no officer in
sight; `03:37 Sentence ... 6 days`, then `[camera] JUMP 96 tiles in 132 min (allowance 2): Block#1451 (Sump 3) ->
Precinct#4180 (Civic); LOD Full -> Coarse, sentenced`, Coarse -> Full at 04:00. A viewer following him sees a man asleep,
then an empty room, then a man in a cell 96 tiles away. Columns: `jumps` 1, `max jump` 96, `held` 20.4 h, `awake still`
4103 min `jail`, `home` 2.2 h. Same shape in s42d0/madam_17666 d2 11:38 (JUMP 20 tiles, awake still 6502 min), s43/companion_11406
d35 10:27 (JUMP 68 tiles, Station#4432) and k2/joytoy_1056 d32 01:37 (JUMP 36). 5 of the 5 body jumps in my group are arrests.

**2. The prisoner transferred 300 tiles in one tick (s43/companion_11406 d36 04:00).** `JUMP 300 tiles in 1053 min (allowance
2): Station#4432 (Mid 2) -> Precinct#4180 (Civic); LOD Full, held`. The viewer watches a companion stand in a Mid 2
cell for 17.5 hours (`awake still` 1053 min, `held` 13.6 h then 24.0 h) and then reappear in the Civic Precinct with no
transport (`vehicle` 0.0). Camera columns: `held` 24.0, `jumps` 1, `max jump` 300. The V6 runs report credits these to the
arrest/transfer; for a following camera it is a cut, not a continuity.

**3. The night walk to a capsule hotel (s43/madam_24700 d31 00:02 - d31 17:37).** Off at 24:00, GoTo(Hotel) 155 min across
Vats -> Sump East ("now at street (Sump East)" 00:51, Hotel check-in 02:37), sleep 02:43-08:54, then HangOut on the street
(Sump East) 09:09-11:46 with `nobody there`, a 84 min GoTo(Home) to Block#3064 that ends at 13:59 only after an Eat
interruption, Idle at home, `15:43 GoTo(Workplace) 114 min`, arrival 17:37, ClerkWork 18:03. She owns a home and rents
a 3c bed anyway. Camera columns, d31-33: `street` 8.0 / 6.1 / 8.3 h, `home` 1.8 / 1.3 / 0.0 h, `vehicle` 1.6-2.4 h, `building`
6.3-8.2 h (the hotel). A viewer sees a 57-year-old madam spend most of three days walking between a brothel and a bunk.

Honourable mentions: s42d0/dancer_17341 d0-d2, 140 min inside Ripperdoc#4437 twice a day for three days (section 6,
bug 2); s43/club_staff_7615 d32 woke at 19:36, walked 51 min, started at 20:27, `served` memory at 21:00 as if the shift had
started on time.

## 4. The first week (s42d0)

What the d0 picks live through that the d30 picks do not (all 12 diaries, none from a Coarse stretch):

- **The boot morning.** 9 of 12 begin with the same ladder: `d0 00:00 PLAN Idle [Sleep]` 44 min, `00:46 Rest 90`, `02:18
  Rest 90`, `03:50 Sleep ~62`, `04:54 Rest 90`, `06:26 Rest 90`, `07:58 Rest`; the other 3 are shifted by a minute or two.
  11 of 12 have 3-5 `Rest 90 min` blocks before 10:00. Identical to the minute across club_staff_11674, 11971, bouncer_17058,
  19587, dancer_25716 and others. The d30 picks arrive mid-life at their shift end (`d30 00:00` Wage, `Sleep`).
- **d0 home hours.** `home` 13.0-17.9 h on d0 against 6-10 h at s43/s44 (workday means: s42d0 12.6 h, s43 8.0, s44 10.2,
  k2 9.6); `street` is 4.6 h at s42d0 against 7.7 (s43) and 6.3 (s44).
- **Shorter walks.** GoTo(Workplace) median 50 min, p90 83, max 147 at s42d0, against median 82 / p90 179 / max 230 at s43
  and 51 / 153 / 285 at s44. The city is freshly built; people sit near work. (Single window each, not a trend.)
- **The foreman at midnight.** s42d0/madam_17666 d0 00:00 `Hire: Joss Jessop hired as Manager (picked by Dahlia Nakamura,
  the foreman)`, `Promoted ... Manager at Vat Farm#4077`; she then buys a deck on finance at 02:35 (107c), hacks 600c from a
  ledger at 05:41, drinks at Bar#4166, and d1 00:00 `Retired ... (on savings)`. The madam has left the trade by the end of
  day 0. Nothing in the d30 window is this violent a first day.
- **Early crime.** bouncer_19587 buys a Motorcycle T1 on finance and 13 min later `StealVehicle` fails with 9 witnesses
  (`d3 12:59` -> `13:12-13:28`), arrest d4, six days.
- **Ripperdoc loops** appear in the d0 window (dancer_17341) and not in the d30 window.
- Missing: no companion or joytoy exists at d0 (no candidate in the run), so those two archetypes have no first-week
  evidence at all. Day 0 has no gang, 800 homeless, and a 5,214-elder city.

## 5. 20k against the 2k control

- **Counterparts.** Statistical at both sizes: `settles apart` is 0.0-0.1 a pick-day in every window. 20k is 1.9-2.0 Statistical
  settlements a pick-day, k2 1.1: more customers per venue night at 20k (dancer_25716 s42d0: 38c from 30 distinct tippers;
  k2/dancer_2156: 12c from 11).
- **Time use.** Workday `street` is 4.6 / 7.7 / 6.3 h at 20k against 6.8 at k2; `home` 12.6 / 8.0 / 10.2 against 9.6. Not a
  size effect: the 2k walks are as long (k2 GoTo(Home) p90 115 min, max 452; k2/fighter_1763 d26 `GoTo(Bar)` 188 + 119 + 160 min
  for one 19-min drink, `vehicle` 9.4 h). The 20k walking bug (V5 bug 11) is present at both sizes.
- **The madam's cut.** k2 27 lines in 6 nights, s44 22 and 29, s43 6 and 1. The 2k Brothel has one madam and one worker pool;
  at 20k each madam's cut depends on her house.
- **Hire/Promote.** 2 of 6 20k madams (s42d0/17666, s43/25391) are promoted off the house to a Vat Farm Manager; k2 madam 0 of 1.
  The foreman has a larger pool at 20k to draw on.
- **Chip, berserk, jail.** The only chip episode (k2/joytoy_1056) is at 2k; 20k joytoys (s43 x2, s44 x2) show none in
  14 pick-days. 20k has 4 of the 5 body jumps, k2 has 1.
- **Bout pair.** Same shape at both sizes: one pair, 3 bouts a week.
- **The day shift.** Both day-shift cooks are at 20k; no 2k cook is. The day shift appears once the venue has two cooks.

## 6. New bugs, ranked (visible to a followed character, diaries affected)

1. **A madam is hired off her house by the foreman, and the house loses its madam** (2 of 6 20k madams; s42d0/madam_17666
   d0 00:00, s43/madam_25391 d32 00:00). `Hire: ... hired as Manager (picked by <foreman>, the foreman)`, `Promoted ... Manager
   at Vat Farm#nnnn`; the pick's header `Job` still reads Madam, but from d32 she works `ClerkWork @ Vat Farm#4036, 538 min`
   (09:02-18:00) and no MadamCut posts after d31 (6 lines d30-d31, 0 after). A followed madam stops being a madam without a
   resignation. Likely cause: the foreman's pick draws any Employed adult of suitable skill with no archetype guard
   (`citysim/src/systems/foreman*` or `jobs2` hire path; I did not read it). Related OPEN-126 (foreman hires the homeless).
2. **BuyAsset for an implant fails after the 140 min install, repeatedly** (3 diaries: s42d0/dancer_17341 d0, d1, d2
   x 2 attempts each = 6 failures and 14 h in the Ripperdoc; s44/madam_15417 d95 02:57 and 03:18; s44/cook_8872 d95 13:39 and
   16:00). Each attempt: `ACTION BuyAsset @ Ripperdoc#4437, 141 min (ended)` then `PLAN FAILED: Some(Shop) failed at
   BuyAsset: PreconditionLost`, retried at once for another 140 min. The dancer repeats it three days running and never
   installs. Camera: `still` 140m `other` at Ripperdoc#4437, `aborts` PreconditionLost 2 a day, `building` 4.7-6.3 h.
   Likely cause: `citysim/src/exec/actions.rs:67` sets the duration to 140 for an implant before the sale is validated; the
   buy runs at the end (`exec/actions.rs:1094`, `systems/chrome.rs::buy_install` Err -> PreconditionLost at :1108). Check
   the price/stock before the wait, or fail at step 1.
3. **Day-shift venue workers serve nobody** (2 of 2 day-shift cooks: s43/cook_8497 d30-36, s44/cook_8872 d91-96; 0 `served`
   memories). Counters settle once, at 21:00; their shift ends 18:00. Likely `systems/service.rs` (the hourly counter
   settle for `evening_staff` roles, lines ~117-145) never opens a day counter for a cook. OPEN-114 moved the shift but not
   the counter. A day cook's week is `ClerkWork 538 min` and a street hangout.
4. **The same patron tips repeatedly in one hour-line** (3 of 6 companion diaries). See V5 bug 4 above; no OPEN row. The
   settle loop runs once per customer slot with the same hashed patron for the hour (`vice.rs::on_visit`-style hourly pick).
5. **Bouncer has no door** (7 of 7; OPEN-121). Same diary-level cost as V5: nothing a viewer follows.
6. **Counter transactions are invisible: 21:00 mass settle, Statistical customers** (every venue worker, 40+ diaries).
   Tips and `served` memories for 4-13 patrons post at exactly 21:00 while the worker is mid-ClerkWork and no patron is a
   body (`settles stat` 100 % of the group's settles). A viewer sees an empty counter and a coin counter. Not a sim bug; a
   requirement for the view (on-screen Statistical customers).
7. **A street stim sale is labelled `served-pair ... (kind and side unknown: my row Cook, theirs none)`** (4 lines in
   3 diaries: s44/cook_8872 d90 16:15, k2/cook_612 d26 14:49, k2/bouncer_2183 d26 14:08 and d27 16:45; s42d0/madam_17666
   d3 00:00 with `my row none`). These follow `BuyStims` from a gang dealer (`Stims -10 to gang Ninefold`). The camera
   tool's kind/side label has no dealer row. Camera-tool label gap, not a sim bug.
8. **Idle `Rest` 1-minute loop** (1 diary: s42d0/madam_17666 d0 08:25-17:56, 16 `PLAN Idle [Rest]` / `ACTION Rest, 1 min`
   lines at a bar). The pick stands in a Bar for 3.5 h after one drink, with the planner emitting a new Idle plan every 30
   min. Looks like a stuck Idle with no goal winning.
9. **A weekly rest day for the whole venue** (52 of 52 pinned diaries, day % 7 == 6): every pick's `work` is 0.1 h, `awake still` 90
   min. The camera sees every club, pit and brothel with no staff on the same day. Not a bug if venues close, but the venue
   rows show no replacement staff (cook_8497 d34: `work 0.0`, `street 17.9 h`).

## Notes on method

- Counts of memories exclude Coarse stretches (3 picks have 22-33 min Coarse windows after sentencing; none was used).
- `nopin` dancer_9076 and cook_8497 are Statistical for the whole 3 days (`stat` 24.0 h, `awake still` 4320 min at `street
  (Mid 6)`). Evidence of what the unpinned pick shows: both stand at the same street tile for 72 hours; the dancer's settle
  count (2, 3, 8) tracks the pinned run (3, 3, 13), the cook's is 0. Nothing is inferred about the absent bodies.
- One life that stands out: s43/madam_24700 (d34-35). The 57-year-old madam joins The Static Knives at 10:46 on her rest
  day (`GangJoin`), is Intimidated for 5c at home at 08:27 on d35, fights the robber (`Attack ... 15 min (agent died)`) and
  dies at 08:55, stripped of 14 coins by 09:09, buried d36 07:28. It reads as a life; it is the best story in the group
  and it is not the trade.
