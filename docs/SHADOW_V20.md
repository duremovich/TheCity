# Shadow V20: the view milestone's exit read (2026-11-02)

The view milestone's read (`docs/VIEW_MILESTONE.md` § "The exit"): V1-V9 and V8b on main `73c2177`
(`target/v20/sha.txt`), the 100k island, seed 42, days 21-27, one pick per archetype the milestone touched (20
diaries), two picks recorded (`--record office_worker,courier`). Three Sonnet critics (`docs/plans/shadow_v20.md`)
and a follow-cam watch from the fresh day-21 save. Reports in `docs/shadow_v20/`; runs in `target/v20/` (100k and 20k
30-day counters `analyze.txt`, `analyze20k.txt`; the save `isle100k_s42_d21/saves/42-30240.ron`; the recordings
`isle100k_s42_d21/rec/`).

## Verdict

**The walk reads like a city; the work and the gangs still read thin.** 313 lights-crossings in ten commute diaries,
82 % with a wait at the kerb (mean 40 s): `courier_36217 d21 09:39` "CROSS at the lights at (1279, 642) ..., 61 s at
the kerb for the green man"; `guard_16979` waits 61 s, 7 s and none at one junction in two hours. A raider climbs to
the rival's floor in her own body (`raider_20130 d21 00:03`, floor 0 to 8 by the stairs in 4 min) and finds it empty
(20 raiders, 0 defenders, 11c: `OPEN-278`). The El carries bodies (100k ~170 Full, ~700 Coarse, ~4,600 Statistical
rides a day; V19 0 / 3 / 0), but the three rides in the diaries save nothing on the clock (`OPEN-279`). The cells
release early and the released are re-arrested within a day (`gang_leader_105912 d23 00:07`, `d24 04:02`, `d26
00:07`); no bail or work detail in 20 diaries. No minor harmed: no Raid, Murder, Arrest, Release or Assault line in the
30-day 100k events names anyone under 18.

## The exit

| # | line | read |
|---|---|---|
| 1 | followed live at 100k, real time and x60, no stall | **partial**: the 21:00 stall is gone (V19 14-18 s a day); the 00:11 hire is 2.0-2.9 s on days 20-27, 4.2 s on day 28 (`OPEN-265`): no hitch at real time, a 2-4 s hitch a game day at x60. Not watched for a whole game day |
| 2 | everyone drawn as a person, riders in their car, the camera on the body's floor | **met**: `live_ow_d21_1812` (an evening crowd animated, 49 riders drawn), `live_ow_d21_0811` (floor 27, z 228, the cutaway), V9's Hillside train with 32 riders standing in the cars |
| 3 | nobody in the ring Statistical | **met** live: "in camera ... Stat 0" in all three live shots. A recording made without `--record-view` draws the shadow's Statistical stand-ins (`OPEN-270`'s leftover) |
| 4 | a recorded pick's day plays and agrees with its diary | **met**: `courier_36217 d21` 09:38-09:40, the caption at the diary's minute ("walk at (1278, 642)"); V6's Tank Novak agreed at seven minutes. A recording draws no signal heads (`OPEN-260`) |
| 5 | a live key reloaded changes the view within a tick; the pace set by eye | reload **met** (V1, `--reload-at`); the pace **open**: Dylan's, on the fresh save |
| 6 | 100k ticks/s at V18's or better | **met**: 18.9 mean (V19 13.5, V18 13-14) |

## The 100k counters (30 days, seed 42, against V19)

| | V19 | V20 |
|---|---|---|
| tps mean | 13.5 | 18.9 |
| max tick | 14-18 s at 21:00 daily | 0.8-4.2 s at 00:11 |
| El rides a day, Full / Coarse / Stat | 0 / 3 / 0 | ~170 / ~700 / ~4,600 |
| deaths by violence | 525 | 807 |
| thefts / arrests | 1,844 / 17.3k | 3,391 / 26.5k |
| jailed mean / max | 2,755 / 5,886 | 1,616 / 2,912 |
| starvation deaths (20k: 3) | 0 | 2 |
| employed mean | 51.9k | 52.6k |

The cells' early release halved the jailed and put them back on the street: violence and theft rose with it. A
finding, not tuned (doctrine 1, the harsh city); `OPEN-256`. V8's 736 starvation deaths at 20k do not reproduce on
main (`OPEN-277` closed).

## Bugs, by what the player sees first

1. Cabbies drive no fare, 6 of 6 shifts (`cabbie_94847`; V19's, unchanged): `OPEN-217`.
2. Jaywalk hotspots where a daily route has no zebra: `cabbie_94847` one 13-tile gap 25 times a week, `elder_87697`
   nine, `dealer_33044` 34: `OPEN-260`.
3. Raids land on empty Hideouts (66 "nobody held it" of 501 Raid lines): `OPEN-278`.
4. The El ride that saves nothing; a train booked, the next one caught: `OPEN-279`.
5. A guard leaves his 9 h shift about midday and is paid (`guard_16979 d22-27`): `OPEN-280`.
6. Retired at 62 (`camp_parent_44777 d26 00:08`), against the work-till-74 decision: `OPEN-281`.
7. The revolving door of early release: `OPEN-256`.
8. GetHigh walks of ~300 tiles that fail on arrival (26 in four street diaries): `OPEN-253`.
9. A starving line prints no age, so a child cannot be ruled out: `OPEN-282`.

The work still barely reads as work (a Teacher doing ClerkWork, a Guard patrolling 3.5 h of 9), two of six gang
picks spend the week in the Precinct, and the free gang lives hold day jobs with an hour of crime a day: V19's,
unchanged; M18's ground.
