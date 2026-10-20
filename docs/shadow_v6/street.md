# Shadow V6, the street critic (homeless, sick, elder, vendor, street_worker, sweeper, camp_warden)

Reader: the "street" critic. Read-only except this file. Every event is a seeded game abstraction; I judge whether it reads as the trade
and as a life, and as footage for a camera that follows one body.

Diaries read, base `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\` (paths below are relative to it):
- `k20_s42_d0` (8 picks: 2 homeless, 2 sick, 2 elder, 2 camp_warden; **no sweeper, vendor or street_worker exists on day 0**: `k20_s42_d0_list.log`)
- `k20_s43_d30` (14), `k20_s44_d90` (13; one street_worker, the list had 1), `k2_s42_d26` (6; no street_worker), `k20_s43_d30_nopin` (4 of mine).
- Method: the `## Camera (Shadow V6)` block of every pick, `camera.tsv` rolled up per pick, then `[camera]`, Meeting/served, plan-abort and
  Report/Arrest greps, and at least one full day per archetype (sweeper_25075 d30, vendor_13771 d33, camp_warden_8463 d31, elder_22423 d0,
  elder_982 d28, homeless_7968 d0/d3, homeless_23500 d32, sick_2055 d28, street_worker_18088 d30-31, sick_18467 d92). Stretches where the
  pick was Statistical or Coarse (the `nopin` window) are quoted only for lines that exist.
- Camera "work" is zero for every outdoor job (a Sweep or a Vend stands on the street, not in the employer's building); I read the
  `still` and `awake still` columns instead (`still_class work`).

## 1. Per archetype

**Homeless: thin (alive in 2 of 7).** The squat is the new street life and it reads: on day 0 the picks `Squat` an empty Block and `Occupy` it
(`k20_s42_d0/homeless_7968` d0 03:12 "PLAN Squat on Block#3833 ... Occupy", 04:17), fined 3c for sleeping rough (d0 03:00), eat out of the
inventory, and the poorest sleep in a Capsule Hotel or a Soup Kitchen bed (`k20_s43_d30/homeless_23500` d32 00:10 GoTo(Hotel) 113 min, CheckIn
02:03, -3c "Hotel", Sleep 376 min, then Scavenge x3, Beg x4, +2c). That one is a person: Capsule bed, scavenge, a HangOut with a known
Acquaintance (13:09), begging at dusk. The others are hired by the foreman within a day and leave the street (22885 hired d1, 18332 and
21024 already employed, 10718 hired d92 and housed d93, 2k 2376 hired d26 and housed d27). 7968 is the opposite: a squatter who sits in the Block
24 h a day d3-d5 (see section 3, moment 3).
> `d32 02:09` [money] -3c (Hotel), purse 0c; place owner Nikolai Oyelaran#11546 (`k20_s43_d30/homeless_23500.md`)

**Sick: not alive as sick.** Seven picks, 0 lines about care, a clinic, a Ripperdoc, a treatment or an orderly in any of them. Cancer 0.36 / 0.57 / 0.75
(diagnosed) changes nothing: `k20_s44_d90/sick_18467` (75 y, stage 0.75) leads a Hideout (`Lead`, Collect, a 475 min `GoTo(Seller)` on
`d90 03:13`) and sleeps in it; `k2_s42_d26/sick_2055` (stage 0.40) does 57 Scavenge actions in a week. The two that read as a person are
poor, not ill: 6514 (hired as Vat Tech d30, 9.2 h work a day, malnutrition 0.51) and 2055's Church loop. V5 said the same.
> `d28 06:02` [meal] ate a REAL meal | malnutrition 0.17 | toxicity 0.53 | cancer stage 0.40 (diagnosed) (`k2_s42_d26/sick_2055.md`)

**Elder: thin.** Visits exist and the label reads: 5 of 7 picks are "visited ... (an elder parent)" (`k20_s42_d0/elder_25402` d0 11:00 at
Freight Yard#4542, `k20_s42_d0/elder_22423` d1 15:54, `k20_s44_d90/elder_17903` d91 18:00, `k2_s42_d26/elder_982` d27 12:40, `k20_s43_d30/elder_16993` d35 15:00 "at where they were"), but each is one
event line with no body at the door (the visitor is off screen, no `[camera]` line). 22423 is the new one: retired at d0 00:00 "(to family)"
and "moved in with family" (the retirement mechanic works, section 4). The jobless elders are scavenge-and-HangOut (26-34 Scavenge a week) with
0-6 served lines; 5 of 7 have a 12 h+ idle day (section 6, bug 1). 17903 (61, Vat Tech, 134c) is a worker who happens to be old.
> `d1 15:54` Visited: Tess Duarte visited Undine Ironside at Block#2073 (an elder parent) (`k20_s42_d0/elder_22423.md`)

**Vendor: alive as the trade in 2 of 5, thin on screen.** 13771 (68 F, Sump Central) is the trade: 52 `Vend` actions in 7 days, pitch GoTo(Spot) 09:01,
a Vend hour every ~63 min, coins mid-action and a named sale ("sold to Knox Chrome#10642 (a buyer, the vendor), a regular", +2c), Scavenge in the
dead hours, a rent shortfall at d33 00:00, a HangOut at dusk, a real meal at home. The V5 label gap is fixed: the memory now says `(a buyer, the
vendor)`. But 100 % of the named buyers are Statistical and "off screen" (13771 settles 3.0/day, 2.9 stat; 17332 2.4, 2.4; 22782 4.3, 4.3): the
followed vendor stands an hour with no customer in sight. 22721 (74 F) commutes 137-176 min each way to a Sump 4 pitch from Mid 8 for 38 Vend
hours and 3 sales, and the pitch closes "no take" on d96 00:00. 1799 (2k) is hired at a School and shakedowns herself into jail (section 6).
> `d33 12:22` [memory] sold to Knox Chrome#10642 (a buyer, the vendor), a regular (valence +0.10) (`k20_s43_d30/vendor_13771.md`)

**Street worker: thin, one life.** Better than V5's "not alive": `k20_s43_d30/street_worker_18088` (20 M, Mid 8) has a pitch (Vend at street 22:33,
23:36, 00:39 ... 05:54 ), a client ("served Hex Mbeki#8482 (a client, the street_worker)", +5c Leisure, `StreetCut -2 to Rex Kowalczyk#20714` at
`d31 00:00`), a pimp who collects (`d31 00:10`, `d31 23:56`, `d32 00:59`, `d32 02:02`) and a rumour of her own arrest. That is the trade, with the
V5 seams (section 2). The other two are not: 20755 spends 41 h in her flat (section 3), the pitch is closed "no take" at d34 00:00, and she is
paid 40c "Coerced: sold Rhys Reyes a job" instead. 20158 (24 M, d90) does 29 Scavenge and 2 Vend; her Solicitation Report and PitchSwept land at
`d93 00:00`, arrest 09:39, a second arrest d96 22:09 (jump to the Station).
> `d31 00:00` [money] +3c (Leisure), purse 12c; flows: Leisure +5 from Hex Mbeki, StreetCut -2 to Rex Kowalczyk#20714 (`k20_s43_d30/street_worker_18088.md`)

**Sweeper: not alive as the trade.** Five of six picks hold a `Sanitation` post (wage 7/day, shift 09:00-18:00) and the Sweep is ONE action of
472-540 min at one tile (`k20_s43_d30/sweeper_25075` d30 09:06 "ACTION Sweep @ street (Sump 8), 534 min"; `k2_s42_d26/sweeper_671` the same,
529-534 min, 6 of 7 days; `k20_s44_d90/sweeper_6665` d92-d96, 535 min). Camera `awake still` is 410-540 min `work` at one street tile on 22 of ~28
sweeper-days. The sweeper meets nobody on the beat; the only counterpart is a daily 09:27 memory "was called on by Priya Kaneda (as a tenant; my
super)" with the super Statistical and "off screen" (25075 d30-d36 at the same minute). What lives: the 183-230 min GoTo(Beat) at 04:00 and
the Soup Kitchen bed after. V5's "a job with nothing inside it" is unchanged, and the camera now shows what that looks like.
> `d30 09:06` [action] ACTION Sweep @ street (Sump 8), 534 min (ended) (`k20_s43_d30/sweeper_25075.md`)

**Camp warden: thin, label fixed.** 7 picks, 12 ClerkWork actions of 537 min inside Work Camp#5479 (a clerk at a desk, so the camera reads it as a
person in a building, not a freeze). The 09:00 count now reads right: "counted Rin Jessop#32260 (the Camp's headcount) (valence -0.10)" with
"... and 6 more memories in the same minute" (`k20_s43_d30/camp_warden_8463` d31 09:00; 46 headcount lines for that pick, 12-18 for the
others), no longer "a regular" (V5 bug 3's mislabel). Still no visit, buy-out, break-out or re-taken child in any of the seven; the only Camp
events in my diaries are two assaults by an unnamed attacker on Juniper Varley "at the Camp" (`k20_s44_d90` d93 12:00, d94 17:00) which the warden
neither sees nor answers. On day 0 the warden has no children to count at all (no headcount lines in the two d0 diaries; OPEN-135).
> `d31 09:00` [memory] counted Rin Jessop#32260 (the Camp's headcount) (valence -0.10) (`k20_s43_d30/camp_warden_8463.md`)

## 2. V5's bugs that touch the street

| V5 bug | verdict | evidence |
|---|---|---|
| 1 fare-evasion arrest loop (OPEN-004 closed for the sentence mix; no cooldown row) | **fixed in size, unchanged in kind** | V5: 2 of 38 badly, 4 touched. V6: 1 of 38 badly. `k20_s44_d90/vendor_22782`: 4 Reports for Fare Evasion `d91 10:17`, `d91 12:38`, `d91 14:33`, `d94 10:27`, 4 fines of 5c ("Wren Brandt fined 5 coins" d91 12:36, 14:31, d92 08:17, d95 11:47) = 20c on a 2-8c purse, a different reporter every time. Two more picks fined once (`homeless`, `street_worker_20158` d93 Solicitation is a different offence). The 6-day sentences in my diaries are Shakedown, not fares (OPEN-005 is the shape). |
| 2 / V5 #9 street worker's client never appears (no OPEN row found) | **fixed in 1 of 3** | 18088: client Hex Mbeki, StreetCut flow, pimp collections (above). 20755 and 20158: no client, 0 coins from the pitch. The client is settled at `d31 00:00` "at street (Sump 8)" while the street worker's Vend hour at 22:33 d30 was at street (Mid 8): apart by a district (camera `settles_apart` 1-2 on d30-31). |
| 3 Camp children invisible, "regular" mislabel (OPEN-026 family; label part of V5 bug 20) | **label fixed, mechanism unchanged** | `(the Camp's headcount)` in 5 of 7 wardens d30+; 0 visits, buy-outs, break-outs. d0 has no children (OPEN-135). |
| 4 homeless hired and housed in one night (OPEN-126, closed 14c15ff: deposit first) | **partly fixed** | 10718: Hire `d92 00:00`, Housed `d93 00:00` (Rent -7c, one wage day later). 2k 2376: Hire d26, Housed d27 -7c after +5 wage. 21024 is Housed d91 00:00 and "moved nearer work" at d92 00:00 on the hire. 18332 Housed twice in one midnight (`d30 00:00` into Block#1929 then "nearer work, into Block#2159"). The deposit makes it a day or two, not a night; a 3-8c person still has a flat after one shift. |
| 5 let go as too sick, rehired same stamp | **not observable** | no "Fire ... too sick" line in my 7 sick diaries. |
| 12 fare loop (=1) / 11 20k walking (no OPEN row) | **unchanged** | 20k GoTo of 113-230 min inside one district: `sweeper_25075` d30 04:09 GoTo(Beat) 230 min; `vendor_22721` d93 GoTo(Spot) 137 min, GoTo(Home) 139 min; and at 2k `sweeper_671` d28 04:37 GoTo(Beat) 183 min, `sick_2055` d28 09:15 GoTo(Spot) 163 min. Not just a 20k bug. |
| 17 elders / Retired-to-family (V5 gate item) | **improved** | the Retired event and the family move-in both fire (22423 d0 00:00); 5 of 7 visited vs 1 of 6. |
| 20 shadow labels (Camp "regular", `served` without side) | **fixed** | `sold to X (a buyer, the vendor)`, `was collected by X (my pimp)`, `was called on by X (as a tenant; my super)`, `was counted by X (as an inmate; the jailer)`, `counted X (the Camp's headcount)` all read. |
| OPEN-103/109 street life (known contacts, free time indoors) | **unchanged** | homeless HangOut lines are mostly "nobody there" (`homeless_7968` d0 09:37; `vendor_13771` d33 18:01 "nobody there"). The few that name a face are 23500 d32 13:09 (Osric Garland, +0.20). |
| OPEN-021 plan-queue starvation (closed) / OPEN-135 no seeded children | open | see section 6 bug 1 and section 4. |

## 3. The camera: three worst continuity moments

**1. The sweeper that is arrested at home and appears at a Station 304 tiles away.** `k20_s44_d90/sweeper_7873` (64 F). `d91 23:16` Arrest at
Block#580 (Mid 2); `d92 00:17` "JUMP 304 tiles in 251 min (allowance 2): Block#580 (Mid 2) -> Station#4412 (Mid 1); LOD Full -> Coarse, sentenced";
`d92 04:00` "JUMP 556 tiles in 223 min: Station#4412 -> Precinct#4180 (Civic); LOD Full, held". A viewer follows a grandmother who is in her
flat, then is a Coarse body at another district's Station an hour later, then a held body 556 tiles away at the Precinct four hours after that,
with no walk, no cell door, no escort. Columns: `jumps (body)` 2 (0), `max jump` 556, `held` 23.7 h on d92 then 24 h x4, `still` 6960 m jail.
Same shape at `k2_s42_d26/vendor_1799` d27 14:36 sentence (no jump row but the body appears at Precinct#418; `awake still` 7764 m jail) and
`k20_s44_d90/street_worker_20158` d96 23:10 "JUMP 40 tiles in 165 min ... Full -> Coarse, sentenced". All 3 are gang Shakedown or Solicitation
arrests; all 3 hand the body to Coarse at the arrest, not at the cell.

**2. The sweeper who stands on one tile for nine hours.** `k20_s43_d30/sweeper_25075` d30: GoTo(Beat) from Block#2694 (Mid 9) at 04:09 for 230
min (moves logged at 04:11 Mid 9, 05:41 Sump 9, 07:50 Sump 8), then five `Rest` plans of 2 min each (07:59, 08:03, 08:25, 08:33, 08:55) at the
same tile while the shift has not started, then `d30 09:06 Sweep @ street (Sump 8), 534 min`. Camera d30: `still 534m work`, `awake still 534m
work, street (Sump 8)`, `street 12.8 h`; the other sweeper-days read the same (`sweeper_6665` d92-d96 535 min, `sweeper_671` d26/d28-d32 529-534
min). A viewer sees a woman walk four hours through four districts, stop, and not move again until 18:00, when she walks to a Soup Kitchen
"Hotel" (`GoTo(Hotel) -> Soup Kitchen#5693`, CheckIn 19:00, Sleep 395 min). She is also woken for work at 04:04 for a 09:00 shift (energy 0.65).

**3. The street worker who is frozen at home for 41 hours.** `k20_s43_d30/street_worker_20755` d31 01:20 Sleep 389 min, `d31 07:51` "GOAL Earn
(was Sleep) score 0.452" and then nothing: no PLAN, no cooled-until line, no action, to midnight; d32 the same ("GOAL Earn" at 06:33, "GOAL Eat"
00:21 and a 10-min EatFromInventory, then silence). Camera d31: `home 22.7`, `plans 0`, `awake still 992m idle at Block#777`; d32: `home 24.0`,
`plans 2`, `awake still 1052m idle`; d33 512 m idle on the street; d35 19.0 h idle. A viewer sees a woman who does not leave her flat for two
days and does nothing in it. Same freeze: `k20_s42_d0/homeless_7968` d3-d6 (`awake still` 607, 632, 722, 812 m idle in Block#3833),
`k2_s42_d26/elder_982` d28-d32 (1080, 782, 749, 422, 475 m idle at Block#158).
Honourable mention (the Coarse hand-off, nopin): `k20_s43_d30_nopin/homeless_18332` d30 01:15 "PLAN FAILED: Some(Sleep) failed at CheckIn:
PreconditionLost [bed Hotel]" in a Soup Kitchen, then `Sleep @ street (Sump 7), 241 min`, `GoTo(Home) 193 min (ended)` at 05:49 that leaves
the position unchanged at 06:00 and 08:00 (`street (Sump 7)`): a Coarse body is told it walked home and has not.

## 4. The first week (k20_s42_d0)

What the day-0 picks live through that the day-30 picks do not:
- **There is no street trade to follow.** `k20_s42_d0_list.log`: sweeper 0, vendor 0, street_worker 0 candidates (V6 runs report: 19 archetypes
  empty on day 0). The player's first week has homeless (800 of 20,000), elders (5,214), 86 sick and 2 wardens, and nothing else of my group.
  By d30 there are 556 sweepers, 132 vendors and 10 street workers (list logs), so the trades are a build-up of the first weeks, not a
  day-0 state.
- **A midnight boot.** At d0 00:00 the city shifts people at once and the diaries open on it: `elder_22423` is "Retired ... (to family)" and
  "Housed: moved in with family at Block#2073" in the same stamp as "at Full LOD (pinned)", but her body is still in Mid 5 and WANDERS for 9 h 30 min
  (20 Wander plans of 30 min, `d0 00:13`-`09:17`, after "Earn unplannable"), sleeping nowhere (camera `home 0.0`, `vehicle 7.3 h` crossing to Mid
  West on d0, `d2 home 24.0`). The bodies of day 0 start unrooted. Day-0 homeless do the same: `homeless_7968` Wander x6 from 00:00 to 02:40
  before the Vagrancy fine at 03:00.
- **The generated homeless (800) are squatters and then the foreman's hires.** d0 PLAN Squat / Occupy for 22885 (Block#1458, 00:01-00:13) and
  7968 (Block#3833, 03:12-04:28), hired d1 for 22885 (Vat Tech) and housed on a later midnight: the 800 shrink to 258 by d30
  (`k20_s43_d30_list.log`). Day 0 is a mass of people who sleep rough and are fined (Vagrancy 3c).
- **No children.** Camp wardens at d0 have no headcount lines (11881, 16750: 0), while d30 wardens count 9 names at 09:00 (8463 "... and 6 more").
  OPEN-135: every child is born in the run.
- **The ends of the week.** d6 camp wardens sit 450+ min idle in their Blocks (camera `awake still` idle at Block#2747 and Block#2818, Mid 8).
- Settlements on day 0: 142 of 1,396 (10 %) names are of a pick asleep (V6 runs report), none of mine.

## 5. 20k against the 2k control

- **Same trade, longer legs.** My picks' day summaries average travel 4.4 h (d0) / 5.8 h (d30) / 5.7 h (d90) at 20k against 3.9 h at 2k, idle 4.7 /
  3.3 h against 4.8 h, work 3.1-4.5 h against 4.2 h (computed from the `DAY SUMMARY` lines, jail days left out). Two-and-a-half-hour walks
  inside one district exist at 2k too (sweeper_671 183 min, sick_2055 163 min); at 20k the pitch trips are the outliers (vendor_22721 137-176 min
  each way for 3 sales in 7 days).
- **The counterpart is Statistical at 20k, rarer at 2k.** 20k vendors: 100 % of named buyers off screen. The one 2k vendor (1799) is hired and
  jailed before it sells. 2k has no street worker (list: 0), 11 homeless (8 free), 12 sick, 489 elders: the 2k pick set is much thinner, so a
  Shadow at 2k cannot show a street life at all; the 20k picks do.
- **The Camp has children at 2k d26 (12 headcount lines for 2544) and at 20k d30 (18-46)**, none at d0.
- **The idle freeze is size-independent** (elder_982 at 2k: five idle days; elder_22423, homeless_7968, street_worker_20755 at 20k): it is a
  life artefact, not a scale artefact. The sweeper's 530 min stand is also identical at both sizes (671 at 2k; 25075, 6665 at 20k).
- **Sick count at 20k explodes with the days:** 86 (d0), 157 (d30), 1,202 (d90) candidates (list logs): by d90 sickness is a mass state and
  the d90 picks are malnourished 52-year-olds hired as Attendants or Vat Techs (12754, malnutrition 0.51), not the d0 cancer cases.

## 6. New bugs, ranked

1. **Goal chosen, no plan, no cooldown: a silent 12-19 h idle freeze** (14 of 33 pinned diaries in my group show at least one idle day of 12 h+,
   about 29 such days; the worst runs of 40 h: `k20_s43_d30/street_worker_20755` d31-d33, `k20_s42_d0/homeless_7968` d3-d6, `k2_s42_d26/elder_982`
   d28-d32, `k20_s42_d0/elder_22423` d5-d6 and sick_9888 d0). What the followed character does: nothing, indoors or on a street tile, for a day or
   more; the diary shows `GOAL Earn (was Sleep)` and silence (the usual "goal Earn cooled until ... (could not plan or plan failed twice)" line
   is absent in the frozen stretches, e.g. 20755 d31 07:51 to d32 00:21). Camera `plans` 0-2, `awake still` 600-1050 min idle. Likely cause: the
   Earn goal's planner returning an empty plan without writing the cooldown (the "could not plan" path in `citysim/src/goap/` / `plan.rs`), or Earn
   with wealth already high and no feasible Earn action for a non-worker; cf. OPEN-021, closed, which was a different starvation. I did not run the
   code; this is where to look first.
2. **The stationary Sweep** (22 of ~28 sweeper-days, 5 of 6 sweeper picks). A body on a street tile for 530 min, `exec/actions.rs:929`
   (`ActionKind::Sweep` is one action whose cleaning is booked at the end by `systems::jobs::sweep_done`). A camera sees an immobile woman. Fix
   shape for the view: a patrol of legs (as `PatrolLeg` has) rather than one stand.
3. **Arrest hand-off: Full -> Coarse at the arrest, then a 300-550 tile jump** (3 of 38 diaries: sweeper_7873, vendor_1799, street_worker_20158;
   gang Shakedown inside 24-36 h of `JoinGang`). The body is Coarse "sentenced", appearing at a Station and then the Precinct with no travel.
   `citysim/src/systems/` justice; the camera `held`, `jumps` and `max jump` columns. Also: JoinGang at `d90 12:58` or `d26 22:00` followed by a
   Shakedown report within 20 h for a 64-year-old sweeper and a 32-year-old vendor (`d27 09:45 Report`, `d27 12:42 Arrest`, `d27 14:36 Sentence 6 days`):
   OPEN-005 (6 days for a 0-coin shakedown) is the cause and it still reads here as a 6-day sentence for the first act of a new member.
4. **The pimp and the street worker settle at midnight and at a distance** (street_worker_18088: `d34 05:36` "was collected by Idris Abernathy
   (my pimp); Idris Abernathy at street (Mid 8); I am asleep at Block#2977"; `d31 00:00` client "at street (Sump 8)" while the pitch is Mid 8).
   1 of 3 street workers shows it; OPEN-120 and OPEN-119 unchanged.
5. **"Hotel" resolves to a Soup Kitchen** (11 of my 38 diaries: `GoTo(Hotel) -> Soup Kitchen#5693`, `CheckIn @ Soup Kitchen`, Sleep 395 min
   for 25075, 7811 x2, 22782 x3; failed `CheckIn ... !hotel_available` for the Coarse homeless in nopin). Maybe by design (the shelter bed), but the plan
   names a Hotel and the viewer sees a Soup Kitchen interior with a bed. Needs a name decision (shelter).
6. **Walk-to-shift starts 5 h early** (sweepers 25075 04:04, 7811 03:10, 671 04:32, vendors 22721 09:10 for 137 min). The commute is longer than
   the free time before the shift, so the sweeper is up at 03:00-04:00 and sleeps 2.7-7 h (`sweeper_7811` d32: sleep 2.7 h, travel 14.9 h).
7. **Header snapshot disagrees with the first day** (`k20_s43_d30/homeless_18332`: "Home: none" but Housed d30 00:00 twice and a Vat Tech job;
   `k2_s42_d26/vendor_1799`: "Job: none" and a ClerkWork day at School#510 on d26). A tool labelling gap, not a sim bug; a viewer's ID card would
   be wrong on day 1.
8. **Camp assaults happen with a warden on the payroll and no response** (2 of 7, d93 and d94 `Assault ... at the Camp`). One line each.
   The camp's trade is the count, no visits (V5 bug 3 stays).

## Verdict for the pre-M18 cut

Street trades that a followed character would show well: the vendor (13771), the street worker with a pimp (18088), the beggar-and-capsule homeless
(23500), the squat on day 0. That a followed street character would show badly, in order: (1) the idle freeze (bug 1), (2) the standing
sweeper (bug 2), (3) the arrest teleport (bug 3). Everything else in this group is thin, not broken.
