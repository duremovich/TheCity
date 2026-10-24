# Shadow V8: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer. Island windows `isle_s42_d0` (18 labour
diaries), `isle_s43_d30` (20), `isle_s44_d90` (20); gen, k2 and nopin were only skimmed. Paths are relative to `D:/TheCity/target/shadow_v8/` (V7:
`shadow_v7/`). Method: scripted `DAY SUMMARY` travel/work hours for all 58 island labour diaries in both trees (378 V7 and 406 V8 pick-days), GoTo leg
scans, Unload and parcel targets, then full days read: builder_7236 d31, foreman_11052 d92, loader_13401 d30-33, loader_19054 d90-96,
day_labourer_19868 d30-36, worker_6720 d5. Only 3 labour people match between V7 and V8, so I compare archetypes. A missing line is never evidence.

## Verdict

The distance pass worked for most of the group. Island labour pick-days average 4.2 h of travel, down from 7.1 h, and pick-days of 8 h+ travel
fell 138 → 69. Builders, loaders and night stockers housed at the Vats now live like workers. Three things still make a followed labourer look broken:
the new **site day walks the builder from his yard back to his own neighbourhood** (about 8 h of walking for 5 h of work); **company housing is sometimes
not near the work** (a D6 dorm 4.4 h from a D6 yard; a D1 bed for a D6 loader); and **Sanitation, Vat Tech and Fab Tech people still commute 6-7 h each way**
from D4/D5. The dead trades are alive in about one diary in three, at one fixed minute a day.

Island, pooled, d0 / d30 / d90: travel h/day V7 6.7 / 7.4 / 7.3 → V8 4.3 / 4.3 / 3.9; work 7.0 / 6.3 / 5.1 → 6.1 / 6.8 / 6.9; no-work days (no jail)
13 / 21 / 36 → 19 / 17 / 16. The d0 window moved least: courier, fabber and foreman at d0 are still 6-8 h of travel.

## 1. Per archetype

**loader (6): fixed for the well housed, broken for the dorm-poor.** `isle_s42_d0/loader_23044` and `loader_9933`: no leg of 4 h+, 18 and 24 Unloads to other
buildings, travel 1.3 h/day (V7 8.3). But `isle_s43_d30/loader_13401` lives in Dorm Storeys#26520, in the same district (D6) as Freight Yard#4791, and walks
443 min (`d30 00:00`, home), 444 min (`d30 13:11`, to work), then 265-267 min (`d32 16:01`, `d33 15:53`). On `d31 06:32` the night loader sleeps 765 min on the street
("Wander > Sleep") rather than walk to the dorm: a company-housed man sleeping rough. Container Bunks#26458, 3 min from the yard, is used for HangOut (`d31 19:19`) only.

**builder (6): new shape, wrong route (bug 1).** Site day gives the 296-min median as claimed. `isle_s43_d30/builder_7236 d31`: walks 153 min D1 → Yard in D6
(`06:16`); `09:22` GoTo(TargetHome) 191 min back to Garage#5745 in D1; Unload `12:33`; ClerkWork 311 min there to `18:00`; GoTo(Home) 129 min (`18:32`). He lives in D1 and the
site is in D1: 473 min walking, 311 min working, at a Garage. `isle_s44_d90/builder_23745` and `builder_9487`: `d93 09:21` 198 min to Ripperdoc#5064 (D1), `d95 09:21` 207 min to Block#1273 (D4),
`d96 09:21` 234 min to Capsule Hotel#4921 (D3), each then 250-305 min of ClerkWork. Where a barracks exists it is lovely: `isle_s42_d0/builder_20151` lives in Site Barracks#26463 with
its yard (home 838 min, work 566 min, travel 0.2 h `d0`), and a friend becomes a spouse in the barracks in one evening (`d1 18:06-20:21`, affinity 0.73 → 1.00; calibration).

**foreman (6): hires now, hollow day.** `isle_s44_d90/foreman_11052 d94 00:00`, `d95`, `d96`: "hired as Builder, picked by ... the foreman"; `isle_s43_d30/foreman_23460 d30-d33 00:00`: one Builder
laid off a day, "revenue, a shift past full staff". Site day: `foreman_11052 d92 11:57` ClerkWork 363 min at Vat Farm#4358 after a 140-min walk from the Yard, then 247 min home (`d92 19:42`).
`isle_s42_d0/foreman_12213` is still the old commute: 342-542 min each way `d0 02:38`, `d1 07:08`, `d2 03:56`, `d3`, `d4`, `d5` (11 legs of 4 h+). `foreman_23460` lives in Site Barracks (travel 2.9 h).

**courier (6): alive in two of six, at one minute a day (bug 6).** `isle_s43_d30/courier_6744`: ten parcels at `d30 10:39`, 11 at `d32 10:39`, 12 at `d33 10:39`, then `d35`, `d36 10:40`, all at Vat Farm#4358 for ten
different Statistical recipients, from Blocks in D3/D4 it never visited. `isle_s42_d0/courier_14724`: 9 at `d3 11:14`, 4 at `d4 11:14`. Zero parcels in `courier_12195` (d30; 61 Unloads, all at its own Yard#4790),
`courier_23849` and `courier_23889` (d90; 60 each; only `d91 09:24` "took a parcel from Garrick Ostrowski#21804 (the courier)"). So the d90 zero is real and wider than d90.

**night_shift (6): fixed, the best archetype.** Travel 1.8-2.7 h, 56-66 Unloads inside the own Street Market (`isle_s42_d0/night_shift_18615`). Nothing wrong that I could find.

**fabber (6): unchanged.** FabWork 537 min a day (`isle_s42_d0/fabber_11456 d1 21:03`, `d2`, `d4`, `d5`), 0 Unloads, 0 breaks. The commute is the old shape: `fabber_11456` lives in D5, works in D6,
309-321 min each way (`d0 16:59`, `d1 06:02`, `d4 15:27`, `d5 06:02`). His move "nearer work" lands `d6 00:00`.

**worker (6): mixed.** Super 417 min on one tile (`isle_s44_d90/worker_23064 d91 11:52`, `d94`, `d96`; V7 373, worse). Bartender `isle_s42_d0/worker_13907` walks 159 min home from Bar#4430 (D2) to D4 at 00:02 every
night `d1-d6`. Driver, Broker and Accountant read like jobs. **worker_6720 (same person): still broken (bug 4).** V7: retired to D4 via 901 min (`d5 01:00`). V8: `d5 00:00` "moved in with family at Block#26535" = Coffin Racks#26535 in
D6, then 120 min to D2, a bar, a 177-min GoTo(Market) inside D2 (`d5 10:32`) and a 721-min walk to the Racks (`d5 13:53`). **worker_13907** is a Cook in V7, a Bartender in V8 (his job differs).

**worker_friday (6): unchanged in kind.** Dancer `isle_s42_d0/worker_friday_25907`: 20 of 20 customer settlements at `21:00`; Club Staff `worker_friday_22891` 14 of 22. The Sanitation pick `isle_s43_d30/worker_friday_17446` has a D6 beat
181-693 min from the D4 Recycler (`d30 07:16`, `d31 07:59` 410 min, `d36 01:51` 693 min): work 2.2-4.5 h, travel 8.7-17.4 h a day (`d32` 17.4), plus JoinGang walks to a D6 hideout (`d30 18:32`, 224 min). Bouncer `isle_s44_d90/worker_18981`: 0 refusals (OPEN-176).

**cabbie (6): thin.** 8 fares, each "took X as a fare" with one Statistical regular at a fixed `09:32` (`isle_s42_d0/cabbie_24718 d0, d1, d3`, Vex Ito#17312); none on screen, no pickup or drop-off leg. The driver pays `-1c Fare` on his own commute
(`d0 21:31`, `d1 18:51`). His work is 6.1-6.9 h (V7 3.8 at d90), so the shift is no longer idle. The 15 body-driven cab rides a seed are not in these diaries.

**day_labourer (4, new): exists, mostly as a Vat Tech.** `isle_s43_d30/day_labourer_19868`: a jobless assaulter, `d31 00:00` hired as Vat Tech by the foreman, same midnight housed in Dorm Storeys#26522 (D6); then he lives like a worker (Farm 3-65 min away). Then `d34 08:44`
GOAL ReportCrime (lawfulness 0.94) sends him 577 min to the Civic Hall in D2, skipping the shift ("travel 47.70->0.15"; Work 0.609), and he sleeps in a D1 Capsule Hotel and walks 655 min back (`d35 00:34`). `isle_s44_d90/day_labourer_39448`: home D5, Farm D6, 428-642 min each way, travel 9.2-19.4 h,
work 0-6.7 h (`d93`: 19.4 h travel, no work, net -2c). `day_labourer_22657` is a Cab Driver at the pick, `39449` an Office Worker (label drift).

## 2. The sore points

| item | status | evidence |
|---|---|---|
| commutes 4 h+ | **fixed in aggregate, alive in a tail** | travel 8 h+ days 138 → 69; the tail is the D4/D5 → D6 commuter: `foreman_12213 d0-d5`, `fabber_11456`, `day_labourer_39448 d93` |
| I6 company housing | **works inside the workplace, fails when not near** | `builder_20151` barracks (0.2 h), `day_labourer_19868 d31` Dorm (26 min); but `loader_13401` Dorm 265-444 min, `loader_19054` Block#3677 in D1 for a D6 yard, 146-150 min each way (`d91 18:39`, `d92 09:14`) |
| evictions on job loss | **not observable** | the one eviction is arrears, `loader_19054 d90 00:00` ("evicted ... by the city"), housed the same midnight; no pick lost a job |
| commute-pressure moves | **fires, slowly** | `courier_14724`, `fabber_11456`, `foreman_12213` all moved `d6 00:00` after six days of 5-8 h walks; `day_labourer_39448` had not in 7 days at 9-19 h |
| builder site day | **bug** | §3 bug 1 |
| foreman | **partly fixed** | hires, layoffs, a 363-min site; d0 commute unchanged |
| courier d90 = 0 parcels | **confirmed, wider** | 3 of 6 courier diaries hand nothing; the others settle ten at one stamp |
| day labourers | **works** | 19868 is hired and housed the same midnight |
| body-driven cabs | **not visible** | 8 fares, all Statistical regulars at 09:32 |

## 3. Bugs, ranked by what a followed worker's viewer sees first

1. **The builder's site day goes to the Yard first, then back to the home district.** `builder_7236 d31 06:16-18:32`; `builder_23745`/`builder_9487 d93-d96`. The site is "the Yard's open site nearest its door by travel" (`citysim/src/systems/shift.rs::far_site`, `site_day_dest`),
   but the shift still starts with GoTo(Workplace) to the Yard, a 16-min load, and the carry out to the site. Fix: with a far site, start the shift at the site (the crate is loaded at the Yard on the same leg), or skip the Yard leg when the site is closer to home. Check what `industry::sites_of` returns:
   a Garage, a Ripperdoc and a Capsule Hotel as "sites" want a reason on screen.
2. **Company beds are assigned without regard to distance from the post.** `loader_13401` (Dorm Storeys#26520, 265-444 min from its yard, both D6), `loader_19054` (D1 bed for a D6 yard). The pass claims 5,221 of 6,148 beds are inside the workplace; the rest sit anywhere. Suspected: the I6 allocation (grep `housing` / company
   housing in `citysim/src/systems/`) picks by free beds, not travel from the employer's door. Fix: rank by `distance::travel(home, door)` with a cap and a fallback to a Block.
3. **A company-housed worker sleeps rough instead of walking to the bed.** `loader_13401 d31 06:32` Sleep 765 min on the street. Fix: Sleep target = the nearest bed within reach (Container Bunks 3 min away), else stay at the post.
4. **The retiree's `move_in` ends in the wrong district.** `worker_6720 d5 00:00`: "with family" = Coffin Racks#26535 (D6); 721-min GoTo(Home). Same bug as V7, a different wrong place. Fix: restrict the target to a relative's own Home, else stay.
5. **Sanitation beat 6 districts from the Recycler.** `worker_friday_17446 d30-d36`. Fix: pick the beat within a reach of the post.
6. **Parcel batch at one stamp, and one courier Yard in effect.** `courier_6744 d30 10:39`; 3 of 6 courier diaries 0. `[island_trades] parcel_cap_ticks` (config comment: the corp's parcels go through the open Yard with the most of its staffed buildings) probably leaves the other Yards' couriers with a crate to themselves. Fix: a courier
   at any Yard delivers its nearest buildings, and stamps each parcel at its arrival.
7. **Dancer and Club Staff settle at 21:00** (V7 bug 6, unchanged): `worker_friday_25907` 20 of 20.
8. **A lawful worker walks 577 min to report a crime and loses a shift** (`day_labourer_19868 d34 08:44`). With one Hall on the island nobody can report on a workday. Fix: a reach cap for a non-urgent goal, or in-shift suppression.

## 4. FINDINGs (calibration, not knobs)

- Travel 4.2 h/day pooled is still high; the fixes land on housed archetypes first. The D5 Spire commuter to the Vats is a durable tail, and phase 4's move takes about six days.
- Barracks courtship: friend 0.73 → spouse 1.00 in one evening (`builder_20151 d1 18:06-20:21`) is fast; it is barracks packing, not a mechanism fault.
- Super 417 min (OPEN-170) and Fab 537 min (OPEN-169) stay one block; the builder joined them at 296.
- The cabbie's fare is V7's gen shape (one Statistical regular at a fixed minute), now on the island.
- One builder laid off a day per Yard (`foreman_23460 d30-d33`) is a plausible rate.
- Statistical recipients on every parcel line are the camera (OPEN-149), not a sim fault.
- Day 0 reads worse than d30/d90 because homes are the pre-pass assignment: courier, fabber and foreman at d0 still have 6.0-7.9 h of travel.
