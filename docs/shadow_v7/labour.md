# Shadow V7: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer. Windows: `isle_s42_d0`
(18 labour diaries), `isle_s43_d30` (18), `isle_s44_d90` (18), `gen20k_s43_d30` (20), `k2_s42_d26` (10), `isle_s43_d30_nopin` (3). Paths are
relative to `D:/TheCity/.claude/worktrees/agent-a996c55984dfa76d3/target/shadow_v7/`. Method: `camera.tsv` for all 84 pinned diaries
(scripted, labour rows only), `[action]`, `[plan]`, `Hire`, `served`, `[parcel]` and `GoTo` leg counts over every labour diary, and one full day read
per archetype (builder d31, loader d31, courier d32 at the island and at gen, night stocker d32, fabber d32, cabbie d30 at gen and d32 at the island,
Club Staff d2, Packer d92, Croupier d94-95, retiree d5). V6 numbers are from `docs/shadow_v6/labour.md`; V6's diaries are gone, so V6 is not re-counted.
Absent lines are not evidence: where a number is a count of lines, it is a count over Full, pinned diaries.

## One paragraph

The shift shapes work, and they are the best news in the group. A followed Loader, Builder, Courier or Night Stocker no longer stands for nine
hours: the diary reads GoTo(Station), a 16-minute load, a carry, a 16-minute Unload, the break outside the door at mid-shift, and the same again
(`isle_s43_d30/loader_20890.md d31 00:27-04:13`). Longest work action, median over labour work-days, is 83-120 min (V6: 530-540 for 55 % of
pick-days). The Loader is alive as the trade: he hauls to other buildings (Vat Farm, Security Office, Noodle Bar, Utility Plant, Packing Plant, Gambling
Den, BD Studio). The rest read as a visible shape with nothing in it: builders, foremen and night stockers unload at their own post, the island courier
hands over no parcel, and the cab driver still waits at the Garage. What a viewer sees first on the island is not the shape but the legs: 24-25 % of
island commute legs are 4 to 15 hours, the body stands on its boarding tile for the ride, and a third of island labour pick-days are travel. The 2k and
the generated city do not have this.

## 1. Per archetype

Verdict: alive as the trade / thin / not alive. One excerpt each.

**loader (9 diaries): alive as the trade.** 15-29 Unloads a diary on the island, all to another building (0 of 9 to the own Yard); lift memory
`loaded the shift's crates` 1-6 times; Pilfered 3 and 4 in two diaries (`gen20k_s43_d30/loader_10046`, `isle_s43_d30/loader_20890`).
> `isle_s43_d30/loader_20890.md d31 00:27` PLAN Work on Freight Yard#4625 [GoTo(Workplace) > GoTo(Station) > ClerkWork > GoTo(TargetHome) > Unload->Vat
> Farm#4298] / `01:47` Unload @ Vat Farm#4298 / `02:05` the next load, to Vat Farm#4300 / `03:41` Break / `04:13` ClerkWork 107 min to the shift end.
Thin point: the carry is a walk of 14-34 min with no crate, count or vehicle in the text, and the shift ends at 04:13 in one 107-min tail block.

**builder (6): thin.** The loop exists (`GoTo(Station) > ClerkWork 16 > GoTo(Station) > Unload 16`, one trip every ~40 min, 8 a shift), but the Unload is
at the own Construction Yard in 109/109, 54/54 (d30 island), 124/124 (gen); only the d90 island has 11 of 62 to a Vat Farm.
> `isle_s43_d30/builder_6834.md d31 14:51` PLAN Work [GoTo(Station) > ClerkWork > GoTo(Station) > Unload->Construction Yard#5418] / `15:13` Unload @
> Construction Yard#5418 16 min / `15:31` the same plan / `16:35` Unload / `16:56` ClerkWork 64 min / `18:00` Wage +5.
No site, brick or crew: four co-workers are "sighted" every half hour (`15:17`, `15:47`, `16:47`) and never met. The same pick walks 382 min to the Yard
(`d31 08:25`), arrives 14:47 for a 09:00 shift and works 3.2 h.

**foreman (6): thin; not alive on the island.** A Builder with the loop above. At gen the foreman is the V6 hirer (`gen20k_s43_d30/foreman_14549.md d30
06:00`: 30 Hires in a week, 15 different Day Labourers; foreman_24199 13 Hires and 15 LaidOff; k2 foreman_1251 13). On the island **0 Hires in all 6
foreman diaries** (d0, d30, d90), and 11 LaidOff lines. See 6.3.

**courier (7): alive at gen, not alive on the island and at 2k.** Gen hands over parcels (10 in the two diaries); island and 2k hand over 0.
> gen: `gen20k_s43_d30/courier_6230.md d32 09:23` GoTo(TargetHome) 76 min to Block#4556 / `10:39` Unload / `10:55` PARCEL handed over at Block#4556 ... for
> Percival Volkov#15274 (fee 1c) x2, "Statistical (off screen)" / `10:57` back to the Yard for eight loops that Unload at the own Yard.
> island: `isle_s43_d30/courier_7866.md d32 11:31-16:49` eight ClerkWork 16 / Unload 16 loops, every Unload at Freight Yard#4625; no [parcel] line in 7 diaries.
The gen delivery is one trip a day, to the same Statistical recipient at 10:53-10:57 on d30, d31, d32, d33, d36. Island: all 265 Unloads are at the own Yard.

**night_shift (6): thin, the shape is visible.** 51-65 Unloads a diary, all inside the own Street Market (the spec: a Market's night stocker hauls
inside), a 40-minute metronome from 00:06 to 05:03 with the break at 02:28.
> `isle_s43_d30/night_shift_8751.md d32 00:06-05:03` GoTo(Station) / ClerkWork 16 / GoTo(Station) / Unload 16, eight times, Break 27 min at `02:28`.
Nothing is stocked on screen (no count, no shelf) and no co-worker is met. V6's friendship at the shared Market is not in these diaries.

**fabber (7): not alive.** Unchanged since V5: one `FabWork` block of 537-538 min, 0 stations, 0 breaks, 0 Unloads in 7 of 7 diaries.
> `isle_s43_d30/fabber_9375.md d32 09:03` FabWork @ Parts Fab#4620 537 min (ended) / `18:00` Wage +4. One `served` line in the file; no sighting.
The runs report already says this; the cause is that the Fab Tech is neither a `[shift.roles]` key nor a haul trade.

**worker (9): thin; four jobs, two shapes.** Stations apply to the Packer, Cook and Companion; the Vat Tech and Engineer stay one block.
> station: `isle_s44_d90/worker_23791.md d92 16:14` Packer: ClerkWork 72 min, GoTo(Station), ClerkWork 30 min, then `18:00` Wage +6 and `18:01` GoTo(Home) 416 min.
> one block: `isle_s43_d30/worker_9116.md d31 09:02` Engineer at Vat Farm#4854: ClerkWork 538 min, wage +14.
Cook (counter): 1-3 customers in a week, drawn slots (`isle_s42_d0/worker_13907.md d5 12:00`, `12:15`, "served ... at the Cook's counter",
Statistical). Vat Tech `isle_s43_d30/worker_22418` works 0.0 h on 6 of 7 days: he is a burglar with 177c and a steal contract (`d31 15:26`
ContractFulfilled, +259c), so the archetype label is the tool's. Gen workers (Vat Tech x2, 538 min, 0 stations) are one block.

**worker_friday (9): thin.** Club Staff, Croupier and Dancer have a stations shift (`isle_s42_d0/worker_friday_22218.md d2 18:02` three spots, blocks of
33, 75 and 70 min, `21:13` Break); the Bouncer stands the door in 156-208 min blocks (`isle_s43_d30/worker_friday_17188.md d30 18:03`, 199 min). The
Sanitation pick still stands (`isle_s43_d30/worker_friday_15453.md d30 09:05`: Sweep @ street 535 min, again d32). Customers: see 2 (row 4).

**cabbie (7): not alive.** 0 fares driven on the island (0 of 42 pick-days); gen 1 regular, twice; 2k once. The shift is the V6 shape: five Rests of 91 min at
the Garage, then a ClerkWork stand-by of 108-118 min on one tile (camera: work tiles 1 on 39 of 39 days).
> `gen20k_s43_d30/cabbie_15342.md d30 06:42` "took Mirabel Mercer#24845 as a fare" / GoTo(Spot) 58 min to Mid West / `07:42` GoTo(Workplace) 85 min back /
> `09:07` Rest 91 x3 / `14:12` ClerkWork 108. The same fare repeats on d31 06:53; no fare on d32-d36.
The island "cabbie" `isle_s43_d30/cabbie_22145` is an Engineer by d30 (`d30 00:00` Hire "Wire Singh hired as Engineer", then ClerkWork 537 min at Lab#5374);
the pick tool did not change the label. 0 island cab drivers are in the diaries as cab drivers.

**day_labourer (3): not alive.** No island candidate in any window (V6: none at d0). Gen and 2k picks are jobless scavengers (Scavenge 16, 34; Beg, StealVehicle)
or a Vat Tech (`gen20k_s43_d30/day_labourer_24420`: FarmWork 7, HaulToMarket 5). The 43 Day Labourer hires at gen are not picks.

## 2. V6 rows and bugs touching the group

| V6 item | status | evidence |
|---|---|---|
| bug 1 / row 1: the shift is one statue action (OPEN-137) | **fixed for 6 of 10**; unchanged for fabber, Vat Tech, Engineer, Sanitation | median longest work action 83-120 min; fabber 537 (`isle_s43_d30/fabber_9375.md d32 09:03`), Engineer 538, Sanitation Sweep 535; cabbie 118 on one tile |
| bug 2 / row 6: the courier round hands over nothing (OPEN-141) | **fixed at gen, unchanged on the island and at 2k** | gen 10 parcels (`courier_6230 d32 10:55`); island 0 in 7 diaries, 265 Unloads all at the own Yard; 2k 0 (61 own) |
| bug 3 / V5 2: the cab never drives (OPEN-115) | **unchanged** | island 0 fares; gen 2 and 2k 1 `took X as a fare`; the cabbie pays `-1c Fare` on his own commute (`k2_s42_d26/cabbie_2152.md d27 12:32`, `d31 07:18`) |
| bug 4: customers settled in a 21:00 batch | **fixed for the Cook, unchanged for Club Staff, Croupier, Dancer, Companion** | Cook's customers at 12:00, 12:15, 14:00; Club Staff 8 of 8 at 21:00 (`isle_s42_d0/worker_friday_22218`), Croupier 3 of 3 (`isle_s44_d90/worker_friday_17722.md d92 21:00` six lines), Dancer 4 of 4, Companion 3 of 5 |
| bug 5: a super calls on you daily at one stamp | **fixed** | `was called on by` 2 lines in 2 of 84 diaries (V6 157 in 26 of 68) |
| bug 6: the arrest blink and the held block | **unchanged** | `isle_s44_d90/foreman_11142.md d91 07:11` JUMP 402 tiles, `d92 04:00` 286 tiles, held 24 h d92-d96; `isle_s43_d30/foreman_20104` held 15.4 h on d36 |
| bug 7: day-0 night workers cooled | **unchanged, smaller** | `isle_s42_d0/night_shift_18848.md d0 00:03`, `03:34` Earn unplannable: Unreachable; labour 10 on island d0 (V6 13); the pick Rests 90 min blocks, home 15.8-17.7 h on d0 |
| bug 8: a cabbie "picks" a Cab Driver at midnight | **unchanged** | `gen20k_s43_d30/cabbie_15342.md d34 00:00` Hire "picked by Quill Fairweather, the foreman"; `isle_s42_d0/cabbie_23043.md d3 00:00` hired as a Vat Tech |
| bug 9: absentee owners paid at a stranger's door | **fixed** | labour settles apart 12, asleep 15 of 473 (6 %); 14 of the 27 from two picks (worker_22418, a gen worker) |
| bug 10: crew counterpart is one face | **unchanged** | the haul days "sight" 3-4 yard hands every half hour, none met; builder/foreman have 0 `served` from a crew mate |
| row 2 / OPEN-116 Visit | **fixed** | 5 `GOAL Visit` lines in gen and 2k labour diaries, all after the shift ends (`gen20k_s43_d30/builder_8172.md d35 18:02` "not on shift 1.00"); none beats a shift |
| row 4 / OPEN-117 spouse at the HangOut | **unchanged** | 10 of 61 married picks name the spouse at a HangOut (V6 8 of 47) |
| row 15 / OPEN-125 old couriers | **unchanged, closed as design** | courier ages 66, 72, 64, 63, 66 still work 8 h |
| row 17 / OPEN-126 homeless hired and housed | **not observable** | no hired person is a pick |
| row 11 / bug 4 (corp): 20k walking | **worse on the island, better at gen and 2k** | non-Station GoTo share >= 80 min: island 32 / 40 / 34 %, gen 25 %, 2k 21 % (V6: 30-34 % at 20k, 34 % at 2k); GoTo(Workplace) >= 240 min: island 13 / 25 / 24 %, gen 0.5 %, 2k 0 % |
| V5 gate: foreman premium | **unchanged** | foreman wage 8 vs builders 6 at d0, 6 vs 6-8 at d30 and d90; different Yards |
| V5 gate: loader's lift, owner's pilfer | **fixed (as V6)** | lift lines in 9 of 9; Pilfered in 2 |
| V5 label bug 20 | **fixed** | `served ... (a customer, at the Cook's counter)`, `handed a parcel to X (the recipient)`, `took X as a fare (the cab_driver)` |
| OPEN-157 contract encounters | **not observable** | no labour pick holds a contract except worker_22418 |

## 3. The camera: the three worst continuity moments

**(a) The body stands for the ride: `isle_s44_d90/worker_friday_17722` (Croupier), `d94 17:46` to `d95 05:28`.** At 15:57 the plan is GoTo(Home) 826 min.
The body moves street D1 -> D4 at 17:06, back to street D1 at 17:46, pays a Fare at 17:48 and stays on that tile until 05:28 the next day (camera: awake still 700
min, class `travel`, vehicle 9.9 h, work 0.1 h). A viewer sees a woman who stood on one Sump pavement for 11.7 hours and then appears at her Block. It is the
worst continuity gap in the group but not a one-off: 126 of 378 island labour pick-days (33 %) have a travel-class awake still; 89 of them 180 min or more, 22 of
300 or more, max 700 (gen 5 days, 2k 1; none of them 180 min). It repeats the exec ride the runs report found (row 4).
**(b) The builder who arrives for the last third of his shift: `isle_s43_d30/builder_6834`, `d31 08:25` to `14:47`.** GoTo(Workplace) is 382 min for a 09:00
shift. He is at street D3 Neon Quarter from 08:43 to 13:52 (309 min, one tile, `-1c Fare` at 08:45), then street D6, then the Yard at 14:47. He works
14:51-18:00 (3.2 h, paid +5c) and leaves on a 376-min walk home (`18:24`). The viewer sees the pawn teleport across a district while the clock runs and a
worker with 9.8 h of vehicle and 4.2 h of work. The same shape: `isle_s44_d90/worker_23791.md d92` (Packer, 414 min out, 1.8 h at work, 416 min home).
**(c) The arrest cut: `isle_s44_d90/foreman_11142`, `d90 23:45` to `d92 04:00`.** Arrested at 23:45; at 07:11 the sentence moves him from Block#2350 (D1) to
Station#5825 (D3) in one tick (`[camera] JUMP 402 tiles in 680 min (allowance 2) ... LOD Full -> Coarse, sentenced`), at 04:00 d92 to Precinct#4460 (D2), 286
tiles; then 5 days of `held` 24 h, 0 plans. It is V6 (a) again, unchanged (the pass did not touch sentencing).
Runner-up: the day-0 retiree, `isle_s42_d0/worker_6720 d5 00:14-16:02`: a Night Stocker retires (to family, Block#121, D4) and his GoTo(Home) ends after 46 min on a
street in D7 The Stack, then a second GoTo(Home) of 901 min to a D4 street, where he sleeps on the pavement for 792 min (travel 15.8 h). Second runner-up, by design
(OPEN-149): `isle_s43_d30_nopin` worker_9116, courier_14305, cabbie_22145 are Statistical for all 72 h on one snapshot tile (3 of 3).

## 4. The first week (`isle_s42_d0`)

- **Day 0 is no longer cold for labour.** Mean day 0: home 9.6 h, work 6.7 h, aborts 1.6 a pick; days 1-5 work 7.1-8.1 h (`camera.tsv`). V6's 18-hour night-worker
  cool is gone: both night workers are at their shift (`night_shift_18848 d0`: 3.5 h work; Earn unreachable at 00:03 and 03:34 only).
- **The first-week shape is the same as the later windows.** Station and haul plans appear from d0: 218-262 `GoTo(Station)` lines in a week for foreman, courier, night stocker,
  builder; 103-125 Unloads. d0 loaders are the lightest (47 and 18 Unloads).
- **The long legs are there from d0.** 29 of 216 GoTo(Workplace) legs at d0 are 4 h or more; day 0 shows `veh` 12.7 h (loader Winifred Stroud, `d0` travel-still 367
  min), 9.2 h (fabber Axel Eastwick) and 8.4 h (foreman Dahlia Draper).
- **Retirement and housing.** `worker_6720` retires `d5 00:00`; `fabber_23160` moves in with family `d2 00:00`, marries, then moves "nearer work" `d3 00:00`
  (`Block#444`, a real move). Housing churn is clean (no double Housed).
- **No hirer story.** The two d0 foremen hire nobody (0 Hires), and no day labourer exists: V6's "the foreman hires from the first hour, re-hired daily" is gone.
  The cabbie `cabbie_23043` is hired as a Vat Tech `d3 00:00` by "the foreman" (the label bug).
- **The weekly rest day is on screen.** Mean work 1.4 h on d6 (the whole group is off), street 10.6 h.

## 5. The island against gen, and against the 2k control

| | island (d0 / d30 / d90) | gen d30 | 2k d26 |
|---|---|---|---|
| GoTo(Workplace) legs >= 240 min | 29/216, 42/166, 40/166 | 1/196 | 0/107 |
| all GoTo legs >= 240 min | 74, 112, 98 | 5 | 0 |
| longest GoTo | 901, 710, 901 | 399 | 239 |
| travel-class awake still, pick-days | 37, 45, 44 of 126 each | 5 of 140 | 1 of 70 |
| zero-work days (no jail) | 12, 24, 36 of 126/125/117 | 10 of 126 | 5 of 59 |
| street + vehicle >= 12 h days | 35, 54, 65 | 19 | 5 |
| mean work h | 6.7, 5.4, 4.4 | 6.8 | 5.9 |
| Hires by the two foremen | 0, 0, 0 | 43 | 13 (one foreman) |
| Unloads at the own post (builder, courier) | 100 % | 94-100 % | 71-100 % |
| parcels handed over | 0 | 10 | 0 |
| fares driven | 0 | 2 | 1 |

- **The island is a commute city.** Homes cluster in D4 Mid-Levels and D1 The Sump, the workplaces in D6 The Vats, and the same people walk, ride, and stand 6 h each way. The shape work
  is the same at the island and at gen (stations and haul plans in the same share of diaries; breaks 168 across the d0 window's 58 diaries). The difference is wholly the legs.
- **Gen is the better trade.** Parcels, foreman hires, a cab fare and the Statistical recipient exist at gen only. The 2k reads like gen without the yard variety: the same loops, 0 parcels, 1 fare.
- **Counterparts**: Statistical share of named settlements in labour diaries is 55 % (262 of 473; V6 72-76 % at 20k, 56 % at 2k). Better at all sizes.
- **Day labourer and foreman are linked**: no island Day Labourer candidate and no island foreman Hire.

## 6. New bugs, ranked by what a followed character sees first

1. **Island commutes of 4-15 hours** (126 of 378 island pick-days are travel; zero work on 36 of 117 d90 pick-days; the Packer works 1.8 h of a 13.8 h day, paid in full). The worst legs:
   GoTo(Workplace) 382 and 390 min (`isle_s43_d30/builder_6834 d31 08:25`, `courier_7866 d32 04:57`), GoTo(Home) 826 min (`isle_s44_d90/worker_friday_17722 d94 15:57`),
   GoTo(Market) 278 min for a 3c meal (`d94 06:22`), 778 min to a job 6 districts away (`d95 11:24`). Likely anchors: the phase 5 leg/El work (`systems/transit.rs`, `transit::choose`;
   the island has 2 lines to gen's 20, P10/P11), and the seller/market choice (`goap` picks a Market 278 min away; nearest by tile, not by time). The phase 5 report says the El was added;
   in these diaries the legs are as long as V6's 300-901 min.
2. **The body stands on the boarding tile for a timed ride**: 89 island pick-days of 180 min or more, max 700 (see 3a). Same mechanism as `exec_ride` (runs report row 4): a timed GoTo with a Fare
   leaves the body still and lands it at the end. Likely anchor: the `GoTo` vehicle/transit timed step (`exec/actions.rs` GoTo, `vehicles::`/`transit::` ride), not an interpolated or
   segment-replanned walk (P10's "segment-replan what still walks" is not visible).
3. **Island foremen hire nobody** (0 Hires in 6 diaries; gen 13-30 a week; 8 and 3 LaidOff at d90 and d30). Cause not read: no island Day Labourer candidate in any window, so it may be the jobless pool
   or the hiring radius. Look at the foreman hire path in `systems/trades.rs` (the `Hire ... picked by X, the foreman` event) on the island.
4. **Fab Tech, Vat Tech, Engineer and Sanitation are still one block.** `[shift.roles]` in `assets/config.toml` (l. 4267) lists 12 roles and has none of these; the Fab Tech is `FabWork`, the Sanitation
   worker `Sweep` 535 min on one street tile (`isle_s43_d30/worker_friday_15453 d30 09:05`). PREVIEW_PASS § 1 lists "engineer" under `stations`; the Engineer diaries do not show it
   (`worker_9116 d31 09:02` 538 min, `cabbie_22145 d32 09:03` 537 min at a Lab). `systems/shift.rs::shape_of`.
5. **The haul goes nowhere for builders, foremen, couriers and night stockers**: every Unload is at the own post (island 265 of 265 courier, 100 % builder). `haul_dests` (`systems/shift.rs`,
   `[shift.haul]`) returns the post itself; the island's `haul_reach` and `haul_dests_max` do not find a site or address. The courier on the island needs one parcel to make it a delivery.
6. **Platform staff still settle at 21:00**: Club Staff, Croupier, Dancer and Companion (`isle_s42_d0/worker_friday_22218`, 8 customers at 21:00; `isle_s44_d90/worker_friday_17722 d92 21:00`, six). Only
   the counter roles (Cook, Bartender, bouncer, barkeep, madam, brothel) use `counter_settle_window`; `[shift.roles]` marks these `stations`. `World.counter_due`, `lod.rs` stat visit.
7. **The retiree's move_in still ends in the wrong place** (`isle_s42_d0/worker_6720 d5 00:14`): GoTo(Home) 46 min to a D7 street, then 901 min to D4. SHADOW_V6 row 5 names `move_in`; not fixed.
8. **The cab driver waits, then walks back**: a fare leg of 36 min is followed by an 85-min deadhead walk (`gen20k_s43_d30/cabbie_15342 d30 07:42`); `Fare` paid on his own commute; "picked by ... the foreman" at midnight. OPEN-115.
9. **Archetype drift (tool)**: the cabbie `isle_s43_d30/cabbie_22145` is hired as an Engineer at d30 00:00 and `isle_s42_d0/cabbie_23043` as a Vat Tech at d3 00:00, so a "cabbie" diary can be a lab worker;
   a "worker" (`worker_22418`) is a contract thief. `citysim-cli/src/shadow.rs` pick: re-check the role at the first pinned day, or log the role change in the header.
