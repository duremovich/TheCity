# Shadow V6: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer. 68 pinned diaries
(d0 18, d30 20, d90 20, 2k 10; no day_labourer at d0: no candidate) plus 3 unpinned. Paths are relative to
`D:/TheCity/.claude/worktrees/agent-a64b0601941c2715f/target/shadow_v6/`. Shorthand: d0 = k20_s42_d0, d30 = k20_s43_d30,
d90 = k20_s44_d90, k2 = k2_s42_d26. Method: each Camera block in full (script over all 68), then grep of `[camera]`, `[parcel]`,
Hire, served, Wage, Arrest, one full day for loader, night shift, builder, fabber, courier, day labourer. Every pick is Full
(pinned); the nopin windows are read only for the camera. Every event is a game abstraction; the question is the trade and the life.

## One paragraph

The labour picks are lives, not trades. Each has a home, a walk, a meal, an unwind and a shift, and the walking and hanging out
read as a person. The trade itself is mostly one long action: every Trade role works the generic `ClerkWork` (a Builder, Loader,
Foreman, Courier, Night Stocker, Cab Driver; the Fab Tech's `FabWork` is the same shape), a single 530-540 min block at one tile.
That is 264 of 476 pick-days (55 %) in the pinned windows (`camera.tsv`, `still_class` work), 9 hours of one unchanged step. The new
columns also show what V5 could not: the cab is never driven, the courier's parcel round hands over 0 parcels in all 6 rounds
logged, and the settlements that make the counter trades look busy land in one batch at 21:00 for Statistical customers.
OPEN-116 (the Visit goal) is fixed in this group; OPEN-115 (the cab) is unchanged.

## 1. Per archetype

**worker / worker_friday (14): alive at a counter, thin elsewhere.** Counter jobs have counterparts:
d30/worker_8462 (Attendant, Braindance Parlour) 65 `served` lines, d30/worker_friday_16144 (Croupier) 56 with 20 regulars,
d90/worker_friday_6822 (Dancer) 14 and tips; the Vat Tech, Sanitation, Fighter and BD Editor have 0-6.
> `d30/worker_8462 d30 19:51` served Rhoswen Adeyemi#24969 (a customer, at the Attendant's counter) / `21:00` five more customers
> "SETTLED APART", all Statistical. Served by name, but the day's trade is in one stamp (see 3).

**night_shift (7): thin.** A Night Stocker is `Rest` at the Market for an hour, then `ClerkWork` 537 min with a sighting each hour
of the same co-worker. The two d0 picks work the same Street Market#4138 and become Friends (d0/night_shift_15717 d2 01:29 affinity
to Gideon Park#24176 +0.73 to +0.83, +0.93 at 03:29): a real night, but nothing is stocked.
> `d0/night_shift_15717 d2 21:02` ACTION ClerkWork @ Street Market#4138 (Mid 7), 538 min; `d2 22:03-04:44` sightings, no stock.

**fabber (7): thin.** Same as V5: FabWork 535-538 min, a Gambling Den or a Club after, home. 3 of 7 name a person
(d0/fabber_25151, d30/fabber_18313, d90/fabber_18475, 4-8 `served`). No fab output in the text.
> `d0/fabber_24191 d2 21:03` ACTION FabWork @ Parts Fab#4499 (Mid 5), 537 min; `06:22-07:39` Drink, Gamble at Den#4500.

**loader (7): thin, slightly warmer than V5.** The lift is there: `loaded the shift's crates for Vatra N` at 05:00 on 5-6 shifts in
all 7 diaries (a memory, no count, no crate on screen), and one Pilfered (d90/loader_16028 d94 06:00, the Report/arrest chain
nearby). The shift between is 538 min of `ClerkWork`; the pick leaves at 06:00 on a 99-minute walk to a street spot.
> `d0/loader_8699 d2 05:00` loaded the shift's crates for Vatra 6 / `06:00` Wage +6c / `06:02` GoTo(Spot) 99 min /
> `21:02` ClerkWork 538 min.

**builder (7): thin as a trade, alive as a person.** A Builder works the Yard 534 min as `ClerkWork`; the Yard's round (V5's
"round_sites") does not show in these diaries. Crew counterpart is one face (d30/builder_7242: Annika Oyelaran sighted 22 times at the
Yard) or none (d0/builder_20184, k2/builder_1458: 0 sightings). V5 read "crew counterpart on 6 of 6"; V6 does not reproduce it
(6 of 7 have no `served` that is a crew mate: the `served` lines are their own tenancy and customers).
> `d0/builder_6643 d2 05:43` GoTo(Workplace) 139 min / `06:00` Wage -5c to Beatrix Wexford at Capsule Hotel (Sump 3) /
> `09:06` ClerkWork 534 min / `11:47` Hotel +3c from Corwin Ramirez. An absentee hotel owner who also lays brick.

**foreman (7): alive as a hirer, nothing else.** Ages 48-59 (V5: 49-60). Hires at 06:00 stamp: 48 in d0/foreman_6844, 18 in
10662, 37 in d30/14549, 26 in 24199, 8 in d90/7224, 16 in k2/1251; always Day Labourers (one Mechanic, one Clerk, one Cab Driver
elsewhere). New: LaidOff lines "(laid off: revenue, a shift past full staff) (told by <foreman>, the foreman)" at d31-d33 00:00 in
the d30 foreman diaries, so the foreman fires as well. All at 00:00 or 06:00 stamps; the foreman is never seen telling anyone.
> `d0/foreman_6844 d1 06:00` Hire: Agathe Bellamy hired as Day Labourer (picked by Vex Haas, the foreman) / Varek Ferreira; `d3
> 06:00` Agathe Bellamy again (hired d1, d3, d4: re-hired daily).

**courier (7): not alive; one round, no parcel.** 6 of 7 walk `SuperRound`, 3 Vatra Blocks in 4.4 h; the new `[parcel]` line reads
"0 parcel(s) handed over in the window so far, 0 of the yard's still pending" on every one of the 36 rounds logged (d0 14157 and
18941, d30 12311 and 6230, d90 22005, k2 1596; d90/courier_25143 has no round at all: 538 min of ClerkWork). OPEN-120-shaped
settlements ride along. The four couriers over 60 do not retire (71, 66, 63, 57), as decided (OPEN-125).
> `d0/courier_14157 d0 09:02` GoTo(TargetHome) 51 min, `09:55` 99 min, `11:36` 103 min / `13:21` SuperRound ... walked the round of
> 3 Vatra 10 Blocks / `[parcel]` 0 handed over, 0 pending.

**cabbie (7): not alive.** 0 fares driven in 7 pinned diaries (both 20k d0 picks, both d30, both d90, k2). The shift is
`PLAN Work [GoTo(Workplace) > Rest]` repeated every ~91 min at the Garage, then one `ClerkWork` at 22:03. The cab driver rides as a
passenger on his own way in: `-1c (Fare) to the City` mid-commute. d90/cabbie_20343 owns a Motorcycle T1 and logs 0.0 h of vehicle.
> `k2/cabbie_1907 d26 12:47` GoTo(Workplace) 170 min / `13:56` -1c (Fare) / `15:37` Rest 91 min x5 / `22:17` ClerkWork 103 min.

**day_labourer (5): thin, and a different trade.** The pick has no Job (`Job: none`) in 5 of 5; they scavenge, vend, hang out,
or take a Guard contract. d30/day_labourer_24112 is idle or on the street 24 h a day for 7 days (722 and 518 min idle on d30, d31);
k2/day_labourer_2408 takes a Guard contract at Street Market#414 for 480 min on five days (the one that reads as work, with
Witness lines `d28 08:04`); d90/day_labourer_12680 Pitches 9 times on d90 then is Hired as a Clerk at 00:00 on d96 by a foreman and
works 538 min next day. Hired not by the Yard foreman but by a foreman at a Street Market: the label "foreman" is the picker's role.
> `d90/day_labourer_12680 d96 00:00` Hire: Razor Bellamy hired as Clerk (picked by Hiro Whitlock, the foreman) / `07:20` PLAN Work on
> Street Market#4145.

## 2. V5's bugs that touch the group

| V5 | OPEN | status | evidence |
|---|---|---|---|
| 2 cab: a body never drives | OPEN-115 | **unchanged** | 0 fares, 0 pickups in 7 of 7 cabbie diaries; no cab driver in 3 windows has a Ride/Drive action (all 8 only Rest, GoTo, ClerkWork); only Fare out. Nopin cabbie_19012 is Coarse 3 days: no evidence. |
| 3 Visit beats Work, cannot plan | OPEN-116 | **fixed** | `GOAL Visit` 0 lines in 68 diaries (V5: 312 lines in 26 diaries); "cooled" lines are Unwind/Earn (50-75 a window). One residue: `Earn unplannable: Unreachable` 8-13 per window, at 00:05 on d0 for night workers (d0/loader_8699 d0 00:05, 02:08, 05:33: the Earn goal is cooled 2 h and the pick Rests 18 h at home on d0). |
| 4 home evening sees only bodies | OPEN-117 | **unchanged, a little better** | 8 of 47 married picks (17 %) name the spouse at a HangOut (V5: 4 of 53, 7 %): d90/builder_8667 and day_labourer_6817 (20 each), d30/fabber_9293 (9). Cabbies 19012, 24366 5-6. Counted from [Spouse] in the top-10 edges; 39 married picks never name them. |
| 15 old couriers on a physical trade | OPEN-125 | **unchanged, closed as design** | couriers 71, 66, 63, 57 on 8 h a day; worker_20492 is a 72-year-old Office Worker with 40 Meetings; d0/worker_friday_20819 (64, Club Staff) does retire: `d3 00:00` Retired (to family), Housed, married `d6 19:45`. Retirement exists; the courier's age is the decision. |
| 17 homeless hired and housed in a night | OPEN-126 | **not observable** | no hired person is a pick with a Housed line; the only hire pick (day_labourer_12680) already has a Home. |
| 20 `served <name>` has no kind | (V5 label bug) | **fixed** | every `served` carries kind and side: "(a customer, at the Attendant's counter)", "was called on by X (as a tenant; my super)", "sat at X's table (a Meeting)". The parcel line exists now (and reads 0). |
| 11 20k walking (corp row, labour travel) | none | **unchanged** | GoTo median 52-59 min in all four windows; 183/570 (d0), 230/681, 184/613 GoTos >= 80 min at 20k, 110/320 at 2k. d0/loader_8699 d2 06:02 GoTo(Spot) 99 min inside Mid 5. Size is not the cause (see 5). |
| V5 gate: foreman premium (bug 4) | none | **unchanged** | wage +8 vs +8 (d0 10662 vs 20184), +6 vs +6 (d30), +7 vs +7/+6 (k2); only d90/7224 shows +8 vs +5. Different Yards, so not comparable; no failing pair. |
| V5 gate: loader's lift, owner's pilfer | none | **fixed (as V5)** | lift in 7 of 7; Pilfered once (d90/loader_16028 d94 06:00). |

## 3. The camera: the three worst continuity moments

**(a) The arrest blink: builder_18419, d90 `d92 18:19-19:20`, then `d93 04:00`.** A viewer follows a Builder who has stood at the
Construction Yard since 09:xx (`d92` awake still 520 min). At 18:19 he is arrested; at 19:20 the Sentence ("5 days for Dealing") fires
and he is at Station#4432 in the same district, 120 tiles away in one tick (`[camera] JUMP 120 tiles in 538 min (allowance 2): ...
LOD Full -> Coarse, sentenced`); at 04:00 `JUMP 300 tiles ... Station -> Precinct#4180 (Civic); LOD Full, held`. From d93 he is 24 h/day
`held` (5,520 min still). Columns: `jumps (body)` 1 (0) on d92 and d93, `max jump` 120 and 300, `held` 4.7 then 24.0.
Also in k2/night_shift_1249 (`d31 00:27` 32 tiles) and d0/worker_friday_20819 (`d6 22:33`). 35 of 44 jumps in the whole run are arrests
(runs report); this is the labour face of it.

**(b) The frozen worker: 9 hours on one tile, and on screen as Statistical for three days.** Column `still`: "538m work" on 264 of 476
pick-days. d0/loader_8699 d2 `21:02-06:00` one ClerkWork action; d90/loader_16028 d92-d96 "538m work" 5 days running; foreman_6844
"538m work" on d0-d5. The viewer sees the loader stand 9 hours with a "sighting" of one co-worker each hour. Worse, the nopin picks:
`k20_s43_d30_nopin/courier_12311` and `worker_8462` read `stat` 24.0 h on d30, d31, d32 with `still` "4320m stat" (the body is at its
snapshot, no step); `cabbie_19012` is Coarse 24 h x3. That is the view's first-frame problem, not a sim bug; no inference from it.

**(c) Customers who are settled, never met: d30/worker_8462 `d30 21:00`, repeated d31, d32 at 21:00.** At 18:03 the Attendant starts
a 357-min ClerkWork block. One customer is seen and named at 19:51. At 21:00 the day's other 5-8 customers are settled in one stamp
("SETTLED APART ... served Rex Pembroke#5950 (a customer, at the Attendant's counter) ... Statistical (off screen)"), 8 on each of
d30, d31, d32. Columns: `settles` 15, 10, 9, 7, 5, 6 a day, `settles_stat` 8 of 15 on d30, `settles_apart` 0; `awake still` 357 min.
Same shape for d30/worker_friday_16144 (Croupier, 56 served). A viewer sees an empty counter and a purse that fills at 21:00.

(Runner-ups: d90/cabbie_20343 `d90 00:00` "Hire: Seraphine Moreau hired as Cab Driver (picked by Elspeth Kimura, the foreman)" at midnight
with the picker at home; 26 of 68 diaries log a daily "was called on by <super> (as a tenant; my super)" at one stamp, 157 lines, the
super Statistical, 10 of them while the pick sleeps: d0/courier_14157 `d0-d5 09:06` while she walks the street.)

## 4. The first week (d0)

What the d0 picks live through that the d30 picks do not:
- **A fresh city at midnight.** d0 day 0 shows `home` 12.9 h, `work` 6.4 h across the group (d30 day 0: 9.1 and 6.4), 2.4 aborts a pick
  (1.8). Night workers wake at 00:00 at home, `Earn unplannable: Unreachable` at 00:05 and again every ~3 h; d0/loader_8699 Rests
  18.1 h of day 0 (`d0 00:36-09:03` Rest x6), because the shift is 21:00 and the Earn goal cannot plan. The d30 night workers do not
  start that way (`work` 6.4 h).
- **The foreman hires from the first hour** (d0/foreman_6844 `d0 06:00`), and the hired labourers are the same few faces, re-hired
  daily (Agathe Bellamy d1, d3, d4). At d30 the foreman also lays off (`d31-d33 00:00 LaidOff`, 50 lines): the d0 city is still hiring,
  the d30 city is shedding a shift "past full staff".
- **Settlements apart are the d0 story.** 82 of 223 settlements are "apart" and 48 "asleep" at d0, against 6 and 1 at d30. 80 of the 82
  are one pick: d0/builder_6643 owns Capsule Hotel#4865 (Sump 3, two districts from her Yard) and takes +3c from each guest all night
  (`d0 03:00` +35c at once; 39 of them while she sleeps at Block#453). A viewer following her sees coins drop in from nobody.
- **Retirement and marriage on day 3 and 6.** d0/worker_friday_20819 (64) retires `d3 00:00`, is Housed with family, marries `d6 19:45`,
  is arrested `d6 21:32` for Fare Evasion and sentenced to 1 day. A short, real arc; d30 has no retirement.
- **No day labourer at all** (no candidate), no gang yet, 800 homeless: the labour group is the whole of the street economy it can see.
- worker_friday starts on weekday 0, not its Friday (tool note); d0/worker_friday_24239 is on its shift on d0.

## 5. 20k against the 2k control

Only 10 control diaries (one per archetype), so these are directions, not rates.
- **Walking is the same.** GoTo median 52-57 min at 20k, 59 at 2k; share >= 80 min 32 % at 20k, 34 % at 2k. The 20k city is not
  farther for a labourer; k2/cabbie_1907's 170-min walk is a 2k number (Mid East to Vats). V5's "20k walking" bug is a district-size
  property of both, not of 20k.
- **The counterpart is off screen more at 20k.** Statistical share of named settlements: 72 % (d30), 76 % (d90) at 20k against
  56 % (50 of 89) at 2k. The Attendant and Croupier counters (all 20k) are the extreme: 8 Statistical customers a night each.
- **The jail takes a worker faster at 20k** is not shown: arrests are 1 per window at both sizes (builder_18419 d90, night_shift_1249 at 2k).
- **Absentee ownership is a 20k thing.** d0/builder_6643's Capsule Hotel two districts away has no 2k analogue (2k has no labour owner
  settling apart: `settles_apart` 3 of 89 at 2k, 6 of 241 at d30, 15 of 176 at d90).
- **Foreman hires are bigger at 20k** (48, 37, 26, 18 in a week) than at 2k (16): more day labour to pick from, so the hirer is more
  visible; nothing else in the foreman's day differs.
- **The day labourer is a different life at 2k**: a Guard contract (k2/2408) in 20k pick lists is the idle or scavenging type (d30/24112).

## 6. New bugs, ranked

1. **Every Trade shift is one 530-540 min `ClerkWork` action** (61 of 68 diaries have it; 264 of 476 pick-days; no round, no
   crate, no brick, no fare). A followed Builder, Loader, Foreman, Night Stocker or Courier stands at one tile for 9 h. Cause:
   `citysim/src/goap/actions.rs` `Role::Trade(_) => ActionKind::ClerkWork` ("a trade works the generic shift at its workplace (no
   new action)", ~l. 313-316) and the Fab Tech's `FabWork`. The shift ends on the wage tick (06:00, 18:00) and the pick walks away on
   the next minute. For the view: add a visible shift action per trade, or the camera shows a statue.
2. **The courier round hands over nothing and stops at three Blocks** (6 of 7 couriers, 36 rounds, all `[parcel] 0 handed over, 0
   pending`; d90/courier_25143 has no round: 538 min ClerkWork). A viewer sees a 4.4 h walk through 3 Vatra Blocks, called on as a
   "tenant" by a Statistical super at the same stamp, then ClerkWork. Cause: `systems/trades.rs` `round_plan` (l. ~436-455) runs
   `service::super_stop` and `streetwork::courier_stop` at each stop and ends in `SuperRound`; `streetwork::parcel_stops` (l. 925)
   returns the Yard's stops, but `courier_stop` hands none over and the Yard has none pending. Either the parcel producer is off or
   the stops are filled from the Blocks fallback in `new_round`. Worth a unit test before the view.
3. **The cab driver rides, never drives; the Garage shift is Rest** (7 of 7 cabbies). Unchanged OPEN-115; new in V6: the pick pays a
   `Fare` to the City on his own commute (k2/cabbie_1907 `d26 13:56`, d30/cabbie_16859 `d33 16:02`) and a pick with a Motorcycle logs
   0 vehicle hours (d90/cabbie_20343). The cause is the one OPEN-115 names (`streetwork` cab dispatch: the Garage's off-screen cab
   wins); a cab-shaped body would be seen waiting 91-min Rests at the Garage.
4. **Customers are settled in one batch at 21:00 for Statistical counterparts** (the Attendant and Croupier picks, d30 and d90 dancers;
   5-8 lines at a stamp). A followed counter worker has 1 visible customer an evening and 5-8 settled in one minute. Cause: the
   off-screen `lod` mirror posts the venue's night at the hour; the Full pick's `served` memory follows it. Shape of OPEN-120/OPEN-110.
5. **A super you never meet "calls on" you daily at one stamp** (26 of 68 diaries, 157 lines, 10 asleep, super Statistical). d0/courier_14157
   d0-d5 `09:06` while walking in the street. Not a rent event; a service memory that attaches to the pick without presence. Cause:
   `systems/service.rs::super_stop` writes the tenant's memory by the Block link, not by presence (OPEN-120's shape).
6. **Arrest blink and the held block** (3 diaries): 120-300 tile jumps at sentencing and transfer, 24 h/day `held` after. Cause:
   `LOD Full -> Coarse, sentenced` sets the body at the Station/Precinct without a walk (the runs report counts 29 + 6 of 44 jumps). For
   the view: a cut or a "taken away" beat.
7. **Day 0 night workers are cooled for 18 hours** (`Earn unplannable: Unreachable` 13 on d0, 9 d30, 8 d90, 9 k2; d0/loader_8699).
   The Earn goal cannot plan with no shift and the pick Rests at home from 00:36 to 18:00. Only the first-day boot; Earn is also
   unreachable for fabber_25151 at 00:05 on d0.
8. **A cabbie "picks" a Cab Driver at midnight** (2 of 7: d90/cabbie_20343 `d90 00:00`, d30/cabbie_19012 `d36 00:00` Mechanic by
   Dante Bellamy); the Hire event labels the picker "the foreman" for any role and the hirer is asleep. Small; the picker is the pick
   himself, a trade-peer hire in the Garage with no scene.
9. **Absentee owners are paid at the door of a stranger** (d0/builder_6643, 80 apart settlements, 39 asleep; also a `Wage -5c` to a
   Statistical clerk). Not a bug as economy; as footage it is coin ticks with nobody. Same family as OPEN-120.
10. **Crew counterpart is one face** (builders and foremen: 22-24 sightings of a single co-worker in 2 of 6 yards, 0 in 4). V5 read the
    crew as present (Moth Yates x6); the yard is a quiet room now. Cause unknown; the rounds that would bring the crew to the pick
    (bug 1) are the likely lever.

Not seen in this group: Visit-goal churn (fixed), PreconditionLost for couriers (0 in 7), V5's two laid-off-and-rehired stamps.
Camera-column coverage: `jumps (body)` 0 for every labour pick (0 of 476 pick-days), so a followed labourer never teleports on his
own feet; `held`, `stat` and `coarse` hours are the only discontinuities, and only the nopin and jailed picks show them.
