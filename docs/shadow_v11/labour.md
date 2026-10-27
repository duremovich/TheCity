# Shadow V11: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 20 diaries; file names are `<arch>_<person id>.md`).
100k island, seed 42, d21-27. Method: DAY SUMMARY lines for all 20, `counts:` lines, greps for Hire / fare / parcel / money / GOAL, full stretches read for
cabbie_82150 d23, cabbie_78332 d24, day_labourer_75212 d25 and d27, night_shift_27704 d21-22, worker_friday_37122 d21, foreman_21236 d27, builder_13696 d23,
loader_75794 d27. Cites are `file dDD HH:MM`. A missing line is not evidence.

## 1. Verdict

Against V10 the labour group is a little better, and only where round 2 touched it: cab fares exist (one cabbie took 16 in a week), the law reached a thief the same
morning (V10: four days), the foreman's blackmail ran to a report and a grudge, and the rest day is marked. The shift itself is where V10 left it: a metronome, posts
dealt by vacancy not distance (4-12 h walks for four of 20), builders and day labourers sent to a random "site", day labourers hired and then not working.
One new bad one: a cabbie is told "Paid" every day and no coin ever lands. Stories start (blackmail, theft, a layoff) and again almost nobody picks them up.

Pooled over the 20 (d21-26): work about 8.2 h, travel about 3.4 h a day (V10: 4.9 / 5.0), mostly because the 12-17 h hikes shrank.

## 2. Per archetype

| archetype | V11 | V10 | excerpt |
| --- | --- | --- | --- |
| worker | alive, flat | alive, flat | `worker_58779` Vat Tech, 9.6 h work, 2.3 h travel, net +5c, d22-d26 identical to the line. `worker_40875 d27 11:10` sees a Shakedown, tells one of her people, memory WasRobbed: the only scene. |
| worker_friday | alive (one), thin | mixed | `worker_friday_37122 d21 05:42` Sanitation GoTo(Beat) x7 through the day (166, 82, 13, 20, 7, 29, 8 min): a real round. `d21 02:45` Crew: tried out for The Red Brothers, "turned away: they were full up" (R5 try-out). `_63106` Civic Clerk, 176-tile commute, 5 h travel, net 0-4c. |
| night_shift | thin | alive | `night_shift_27704 d21 08:31` GoTo(Workplace) 702 min, 767 tiles, to a market in the same district, then GoTo(Station) 3-4 min every 20 min from 21:04 to 01:10 (13 times). `_23247` 4-tile commute, leisure 7 h, no scene. V10's fight is not repeated. |
| fabber | thin | thin | `fabber_103028` Parts Fab in D6, 189 min commute, 6.8 h travel and 8.9 h work every day, net +5c; `d27 10:41` buys a Motorcycle T1 on finance (-30c down). `_85823` 11-tile commute, flat. |
| loader | thin, one story | broken | `loader_75794 d27 06:00` Pilfered a Ration, Report; `10:00` "Mirabel Fletcher arrested Smoke Lindqvist"; `12:33` sentenced 3 days: report to sentence in 6.5 h. `_34403` 8-tile commute, 8.5 h, nothing else. |
| builder | thin | thin, better | `builder_13696 d23 09:21` GoTo(TargetHome) 163 min, 128 tiles to Vat Farm#4083, Unload, ClerkWork. `d22 05:48` Stripped (took 4c off a dead man). `builder_86355` 10-tile commute, net +1c, `counts: none`. |
| foreman | alive (one), thin | alive | `foreman_21236 d21 06:00` 190 Hire lines in 7 days (19-32 a morning). `d27 14:20` Blackmail: found Fiora Dunmore, wants 5c, she went for him, he won and went to the law; GrudgeFormed; `16:22` Cells: Fiora learns who put her inside. `foreman_25916 d22 00:07` LaidOff "a shift past full staff", told by the foreman. |
| courier | alive (one), thin | alive / jailed | `courier_48202` 89 PARCEL lines; `d21 00:07` three handed over in one minute at Block#1967. `courier_66222` 19 parcels, net 0c all week (63 min commute, 4-6 h travel). |
| cabbie | alive (one), dead (one) | dead / thin | `cabbie_78332 d24 07:56` "took Static Park as a fare ... a regular", 16 fares d21-26, a FareShort `d21 08:47`. `cabbie_82150` one fare in the week (`d24 09:18`), 9.8 h `Rest` at the Garage (`d23 05:09`-`14:00`), wallet 0c. See bug 2. |
| day_labourer | thin | thin | `day_labourer_75212 d25 06:00` hired (picked by Dex Mercer), scavenges to 08:00, `09:24` GoTo(TargetHome) 263 min, 237 tiles to Parts Fab#4827 in D5, ClerkWork 237 min, +3c. `d27 06:00` hired again, then Wander all day. `_8870` never hired, 0c net all week, `other` 7-9 h a day. |

## 3. Round 2's mechanisms seen at 100k

Fired:
- **R6a cab fares**: `cabbie_78332` about 2 fares a morning, "a regular" by d23 (`d23 07:28`, `d24 07:56`), FareShort `d21 08:47` (the rider skipped the fare).
- **R5 try-out**: `worker_friday_37122 d21 02:45`, the first time a labourer's JoinGang ends in a scene.
- **R4 rest day**: `DAY SUMMARY d27 (rest day: no shift)` on posted workers, `(the city's rest day)` on day labourers. The "work 5.5-6.7 h" on night posts that day is the d27 00:00-06:00 tail of the d26 shift; fine.
- **Law**: `loader_75794 d27 06:00-12:33` with a named officer (V10: reported d21, arrested d25).
- **Layoff with the foreman's name**: `foreman_25916 d22 00:07`.
- **R2 walking**: long walks are 0.8-1.4 tiles/min (`night_shift_27704`, 767 tiles in 702 min; V10 `loader_27669` 800 tiles in 1113 min). Faster, but see bug 4.

Expected, never seen in the 20: a Sacked firing, a skip ("does not go in"; 0 in 140 person-days, V10 had 3), a misery quit, "took a post nearer home", a wage advance or loan, a rescue, slate or pawn, a bet at the pit, a sick-post cover, the muster or stand-down verbs, a court-missed-at-work excuse, a cab carrying a hurt body. Only 4 of 20 have a far post, so the commute fix may simply not be exercised here.

## 4. Bugs, ranked by what the player sees first

1. **Day labourer hired, then not working (V10 bug 5, unchanged).** `day_labourer_75212 d27 06:00` "hired ... picked by Dex Mercer", then Scavenge, Wander x7, no yard visit; hired 2 of 7 days, `_8870` 0 of 7 (net 0c, 7-9 h/day `other`). Where he does go (`d25 09:24`) the site is a Parts Fab 237 tiles off in another district (V10 bug 4, unchanged). Likely cause: Hire stamps the post, the Work goal sits behind Earn/Scavenge (`d25 06:01` to `08:02`), and a hire has no obligation to turn up.
2. **Wage "Paid" with no coin (new).** `cabbie_82150` has `memory Paid` at 16:00 every day d21-26 and zero `[money]` lines in the whole diary, wallet 0c, hunger 0.4-0.8, `RentShort` `d26 00:00` (0 of 1), scavenging 01:00-04:00 and 17:00-20:00. Compare `cabbie_78332 d21 16:00` "+4c (Wage) ... from corp Coldwater Holdings". Likely cause: the Garage owner is broke or the wage skips while the Paid memory is written regardless (Garage#5890 owner not checked).
3. **The cab with no fare is paid for resting (V10 bug 3, better, half-fixed).** `cabbie_82150 d23 05:09-14:00` five `Rest @ Garage#5890 91 min` booked as Work, one fare in a week vs 16 for the other cabbie: the Neon Quarter pool is empty or unreached. `cabbie_78332 d24 08:31` and `11:37` GoTo(Workplace) 146 and 213 tiles back to the Garage for a 2-min Rest: the cab walks to the fare and home, and no ride is seen on the cabbie's side.
4. **Far commutes (V10 bug 1, better, same shape).** `night_shift_27704 d21 08:31` 767 tiles, 11.8 h travel; `worker_friday_37122` 8.7 h travel; `fabber_103028` 6.8 h x 5 days. The 997-min loader is gone; worst day is 11.8 h. `builder_13696 d23 09:21` does 128 tiles in 163 min (0.8 tiles/min), half the others: the laden TargetHome leg may not have R2's speed.
5. **Builders and day labourers go to a random "site" (V10 bug 4, unchanged).** `builder_13696 d23 09:21` Vat Farm, `day_labourer_75212 d25 09:24` Parts Fab, each Unload then ClerkWork on the yard's order. Nothing is built; `GoTo(Station)` is 72 of `builder_13696`'s actions.
6. **The Station metronome (V10 dull spot, unchanged).** `night_shift_27704 d21 21:04` to `d22 00:48` GoTo(Station) every 19-22 min; `worker_58779` d22-26 identical to the minute.
7. **One jump (V10 bug 8, better).** `cabbie_78332 d26 11:36` 4 tiles in 1 min; one in 20 diaries.
8. **No skip, quit, sack or walkout (V10 bug 7, worse: 0 vs 3 skips).** `cabbie_82150` at wallet 0c and RentShort never misses a shift.

V10 status: law invisible to the suspect fixed; 1 better; 3 better; 4 unchanged; 5 unchanged; 7 worse; 8 better; the V10 "Bury eats a shift" and "unlabelled money" not seen. Bug 2 is new.

## 5. Dull spots, ranked (mechanism, not a knob)

1. **The shape-up has losers and no consequence.** `foreman_21236` picks 190, `day_labourer_8870` never; `d22 14:59` he only witnesses Slate Crane steal. Mechanism: the passed-over gather at the yard gate and steal or fight; a hire who does not show is struck from tomorrow's list.
2. **The shift is Rest, ClerkWork, Unload on one tile.** `worker_58779`, `night_shift_23247`, `builder_86355` (`counts: none`). Mechanism: a per-shift incident (short count, wrong crate, inspector, a coworker's debt) that leaves a memory; `foreman_25916 d22 00:07` LaidOff is one nobody on the diary follows.
3. **The cab has no errand for the rider.** `cabbie_78332` takes fares before 10:00 only; the afternoon is a Garage Rest. Mechanism: the fare's own errand (the injured to the Clinic, the wanted away from a Raid) and the driver's tip or refusal as a scene both remember.
4. **The rest day is idle.** `courier_48202 d27` 18.2 h idle, `worker_40875 d27` 16.0 h. Mechanism: church, the Den, kin, the fight pit (the foreman went to the Pit `d27 06:45`).
5. **Parcels arrive in bursts** (`courier_48202 d21 00:07` three in one minute). Mechanism: one errand with a delay that the recipient also sees.

## 6. Too much

Nothing harsh enough to read as noise or broken. Two monotony items: (a) `worker_58779` and `fabber_103028` d22-26 are one day repeated to the minute; (b) `foreman_21236` carries 190 Hire lines in 7 days (19-32 every morning); collapse them into one line with a count and the first names, as R4 did for the super.
