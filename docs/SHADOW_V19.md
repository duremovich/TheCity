# Shadow V19: the city milestone's exit read (2026-11-01)

The city milestone's read (`docs/CITY_MILESTONE.md` § "The exit"): C1-C9a on main `41c31b2`
(`target/v19/sha.txt`) with `assets/stat_table.toml` regenerated at the real pace on the 2k map (`f630f5f`; **reverted
after the read**, `6875a4a`: at 2k nobody rides, so the table zeroed every Statistical ride, `OPEN-255`), the 100k
island, seed 42, days 21-27, two picks per archetype (118 diaries; no `dodger` candidate on day 21, an error line).
Eight Sonnet critics (`docs/plans/shadow_v19.md`, V18's groups and the stories critic) and a follow-cam watch.
Reports in `docs/shadow_v19/`; runs in `target/v19/` (30-day counters `analyze.txt`, `run.csv`; the save
`isle100k_s42_d21/saves/42-30240.ron`).

## Verdict

**The vertical city reads.** Every diary has a flat on a real floor (floor 2 to floor 48), climbs are narrated and
none is absurd, and a home, a post and an evening in one block is common (11 of 118 picks): `office_worker_24498 d22`
sleeps on floor 27 of Block#1457, rides the lift to her desk on floor 0 at 08:19 and back at 18:06, then hangs out
on floor 27 until 23:24. Gangs hold floors over fronts and lifts (`fabber_46993 d22 00:05` The Wire Wolves take floor
1 of Block#1129 over a Street Market; `camp_jobless_37295 d27 00:05` "its men ride first"), raids climb (42 city-wide;
`cultist_112804 d25 22:29` 15 raiders up the stairs of Block#1000 to a floor-28 Hideout, won at 22:43), and the
landlord answers a gang (`gang_leader_113134 d24-26`: Habitat hires a beating over 3c of floor rent, two crews take
it in person, both fail, she is jailed). **Far travel is gone as a bug class**: no leg past 120 min in 816
pick-days, the longest Hideout leg 64 min. **Housing worked too well for the tone**: homeless mean 8,459 -> 2,433,
evictions 7,242 -> 192, employed 40.8k -> 51.9k, and with it Murders 389 -> 272, violent deaths 756 -> 525 (doctrine
5: the tone comes back through features, not a knob). **What is thin:** the lift never queues (all but 4 of ~800
climbs wait exactly 1 min), neighbours never become known, the cells still eat gang and crime weeks, and the El is
dead (an artifact of the 2k-calibrated table the run carried, since reverted). **No minor harmed** (`harm_blocked` 0 every day; every harm line 18+).

## The exit

| | verdict | cite |
|---|---|---|
| 1. home, post and evening in or beside one tower; 100k legs over 120 min well below C5's | **met** | `office_worker_24498 d22`, `cook_25003 d26 00:05` (floor 9 over his Noodle Bar), `night_shift_26883` (floor 48 over a market), `elder_67354` the Super of her own block; `goto_over_120` 0 across 816 pick-days (C9a's run 33 -> 0) |
| 2. a gang holding floors over a front; a raid that climbs and lands in a member's diary | **met, with a gap** | floors: `fabber_46993 d22 00:05`, `owner_broke_21221 d21 00:07` (a Ripperdoc bought as a front after a short floor rent); the climb: `cultist_112804 d25 22:29`, `fabber_48104 d25 22:23` (lift cut, 5 min up, "nobody held it"). The gap: the raid's climb is the event's line; the member's own body brawls on the street (`OPEN-264`) |
| 3. no Hideout leg past 4 h | **met** | longest `GoTo(Hideout)` 64 min (`gang_member_82779 d27 00:03`) |
| 4. the follow-cam on three V19 subjects | **partial** | a climb and a lift ride on camera, matching the diary; the footbridge, escalator and El ride not verified (the replay drifts from the diary within hours, `OPEN-270`); riders drawn on a white slab (`OPEN-271`); no tram rider in the diaries |
| 5. no minor harmed, `harm_blocked` printed | **met** | `harm_blocked` 0 on days 0-29; the only minors on a harm-shaped line are children named as the object of an adult's "Camp Escape" sentence (`guard_16925 d22 18:00`): wording, not harm |
| 6. ticks/s at 100k days 25-30 ~25; change tier < 4 min without Bevy; view tier green and timed | **missed on ticks/s** | 100k days 24-29 **11 ticks/s** (V18's run 13-14); a single **14.5 s tick at 21:00 every day** (V18's run 2.3 s, round 8 1.5 s): the city milestone's growth landed in one nightly system (`OPEN-265`). Change tier 2m58s (C9a's branch), no Bevy; view tier 6m44s green (C9a's branch) |

The VISION stories (the stories critic, `docs/shadow_v19/stories.md`): a friend killed and a friend who goes after
the killer **shown** in a diary for the first time (`bereaved_14813`: Joss Thorne mourns his wife, a Precinct guard,
then attacks her killer at work `d25 18:25` and again in the cells `d27`); a hit as business **shown**
(`marked_77516`: 1,625c, taken in person `d22`, called off `d24` while the mark sits in the cells); an NPC taking a
contract in person **shown** (`bereaved_75821 d26 09:33`, fulfilled `10:44`; `joytoy_50675 d25 10:37`, the hit
lands in his floor-14 flat `22:23`); a week that follows from its past **shown**; word of a killing acting on someone
**thin** (told twelve times, moves only the widower).

## The 30-day counters (100k seed 42 x 30, V18's run `target/r9_100k/` -> `target/v19/`; FINDINGs)

| | V18 | V19 | | V18 | V19 |
|---|---|---|---|---|---|
| employed (mean) | 40,819 | 51,940 | Murder | 389 | 272 |
| homeless (mean) | 8,459 | 2,433 | violent deaths | 756 | 525 |
| evictions | 7,242 | 192 | Arrests | 23,002 | 17,261 |
| jailed (mean / last) | 3,133 / 6,747 | 2,755 / 5,872 | Raid | 441 | 552 |
| gang members (last) | 6,893 | 9,081 | Succession | 1,377 | 1,888 |
| Starving | 1,138 | 115 | starvation deaths | 0 | 0 |
| ticks/s (mean) | 14.2 | 13.5 (11 from day 24) | max tick | 3.5 s | 15.5 s (21:00) |
| El rides (bodied / Statistical) | ~68 / ~5,400 a day | 66 / **0** in 30 days | lift wait ticks a day | | ~43k |

The run carried the 2k-calibrated stat table (reverted after): the El row is its artifact, and the off-screen rows
it feeds (Statistical commutes, its violence and arrest rolls) may be too; the next 100k run on the reverted table
re-reads Murder, violence and the El before any of it is called a feature gap.

## The watch

A Sonnet watcher on the day-21 save (`cityview --save ... --follow N --shots`), six subjects, 26 PNGs (report and
shots in the session scratchpad `C:/Users/Dylan/AppData/Local/Temp/claude/D--TheCity/7b1ba3b7-2491-4424-88e2-2165f8dd5830/scratchpad/v19_cam_report.md`, `v19_cam/`). **Exit 4 is partial.** The picture matches the
diary line by line in the small hours and the morning: a lift ride to floor 45 (`gm_d21_0101`-`0118`, Taz Nakamura),
a 15-floor climb (`climb_d21_0855`-`0859`, Bex Nakamura), an elevated deck walked at a sensible railing height with no
clipping (`el2_d21_0820`). **Then the replay drifts from the diary**: the camera re-runs the world from the save with
its own ring and pins, and with determinism dropped as a gate (2026-10-26) the two worlds part within hours (Axel
Dunmore at home at 17:03 where the diary has him on the El; Flux Underhill asleep at 19:03 where the diary has the
escalator; Undine Reyes eating at 07:51 where the diary has the footbridge). So the footbridge, the escalator and a
routed El ride were **not verified on camera**, and no diary rides the tram. What the picture gets wrong: lift and El
riders stand on a bare white slab (no shaft, car, train or station); the HUD reads "@ street" through a climb and the
camera falls (79 -> 52) as the body rises; a fast mover pulls hundreds of Coarse and Statistical bodies into the ring
(611 -> 1,263 -> 2,107 in eight minutes); a shot takes ~1.5 min. `OPEN-270` (the replay), `OPEN-271` (the drawing).

## What reads wrong (to OPEN)

- **The lift is a constant** (every critic): ~800 climbs, all but 4 wait 1 min, rush hour and midnight alike; a held
  lift shows a 5-min wait once and a 1-min wait under a hold once (`ripperdoc_61891 d24 19:50`). Lift queues bind
  only on a gang hold (`OPEN-266`).
- **The El is dead in this run because of the table** (the 2k regen zeroed `p_commute_ride`, reverted `6875a4a`; read it again on the next 100k run): rides_stat 0, fares 0, 66 bodied rides in
  30 days, `fare_dodges` 71 > rides 66 (a counter bug), so no `dodger` pick exists; refused `slow` 1.76M (`OPEN-254`).
- **The raid's climb is the event's, not the member's** (`cultist_112804`, `fabber_48104`) and the founding into a
  floor is an event line no member walks into (`OPEN-264`).
- **The cells eat the week** (gangs 6 of 16, crime 7 of 18 picks in identical `jail 16.0h` days) and Succession
  churns (1,888; ten in one gang in a week): `OPEN-256`, carried.
- **Rich corp people are broke and idle** (`ceo_105038`: a 4c wallet on a 93k treasury, twelve Idle legs a night),
  guards patrol ~2 h of a 9 h shift, failed stims runs after long trips (`exec_78500 d21 17:32`) (`OPEN-267`).
- **Story contradictions**: a jailed leader welcomes members back (`marked_77516 d26 10:28`), "avenged" after a lost
  fight, a killer sentenced for Dealing, "both parents in the cells" with one dead (`camp_parent_49192 d26 00:10`),
  an eviction Story line three days after the death (`arms_dealer_88945 d26 00:01`), "lies low at home" holds nobody
  (`joytoy_50675 d25 10:37`): `OPEN-257` and `OPEN-268`.
- **Labour loops**: a 74-y-old hired, chipped and fired as too frail every midnight (`jobless_14715 d21-23 00:07`),
  sick fired and rehired in 11 min (`sick_67704 d22 00:11`), cabbies drive no fare (288 driven, 2,364 unmanned),
  (`OPEN-268`); company housing with no flat (`homeless_68297 d22`, `OPEN-269`).
- **Distance**: an hour's HangOut still walks 1,073 tiles (`worker_40474 d26 06:04`), a fighter commutes 76 min each
  way Mid-Levels to the Sump with the El refused (`OPEN-221`, carried).
- **Dull**: neighbours stay strangers (`3 stranger(s) [0 known]` for six hours), supers walk the same stairs five
  times a day, a Bury goal unplannable every night (`scav_68640`), raid hurt lists print no ages.

## The close's tiers

- merge (`target/v19/merge.txt`): `[merge] RED (god); BUDGET OK 7m33s of 8m00s`: `god_line_cut` (Y4 left the 2k El riderless), `god_decapitate_gang` (X4's jail churn: no gang keeps a leader 14 days), `god_child_camp_sweep` (a stale matcher, and a real gap: a parent answered only for the lower-id child). Fixed in `100c7cd` (each green in one run), merged `d41f927`; the change tier on main after: `[change] GREEN; BUDGET OK 3m37s of 4m00s`
- milestone (`target/v19/milestone.txt`): `[milestone] GREEN; BUDGET OK 11m36s of 20m00s`
