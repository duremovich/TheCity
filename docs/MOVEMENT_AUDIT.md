# The movement audit by film (2026-11-04)

Dylan: "I think movement of npcs and the player through the world needs a lot of work that we are missing with still
frames." This is a reader's pass, no code changed: the city filmed in the situations stills miss (film P1's tool,
`docs/TESTING.md` "Filming motion"), every clip run through `tools/film.py --crop --strip 8`, its sheet, strip and
`paths.png` read, its `trace.csv` read per frame for the subject. Each defect is an OPEN row (`OPEN-307` to `OPEN-318`),
marked **sim** (pathing, doors, transport, placement, the clock) or **view** (drawing, interpolation, poses).

**Not re-reported** (a parallel view coder has them): the freeze after a hold, the first-tick 5-8 m jumps, the walk clip
rate fixed at 1, the camera losing the subject at speed, facing snaps, standing stacks, back-snaps at tick changes.
Where a clip shows one in a new form it says so.

**How the clips were made.** The save is `saves/slice/20k_s42_d21.ron` (d21 00:00). For the morning and the evening two
saves were written headless from it (`citysim-cli run --load .. --days 1 --save-at 30690` and `31290`: d21 07:30 and
17:30, in `D:/TheCity/target/films/saves/`), and subjects were picked from the saves' own columns (each body's
`Brain.exec`: a `Climb` with `lift` or without, a `Ride` at a stop, a `Goto` whose path crosses a map `X` crossing, the
densest street tiles). NPC clips are `cityview --save S --follow N --play-speed X --zoom 40 --res 1280x720 --out
D:/TheCity/target/films --film NAME:SECS` (10 fps); player clips are `--slice --play @view/cityview/scripts/audit_*.play`.
Every clip is `D:/TheCity/target/films/<NAME>/` (`film.mp4`, `sheet_1.png`, `strip.png`, `paths.png`, `findings.txt`,
`trace.csv`). Frames are `f`, bodies `#`, ticks absolute (30240 = d21 00:00).

**Tool caveats met on the way** (film.rs, not fixed: the brief keeps code to the scripts): two `film` lines back to back
leave the second with a stalled clock (`audit_1_door_player_in_5x` first run: 79 of 79 frames stalled; a `wait 1`
between films avoids it); a script line at a later tick runs the sim flat out to it while the click way is walked on the
view's clock, so the drawn player falls behind (see `OPEN-315`); `click ~DX,DY` refuses anything off the window, and at
`zoom 40` that is 5 tiles (the far-tile click never landed, even at `zoom 220 pitch 85`); a fight films with
`speed_x` 0, so film.py's clock line reads "no frame pair at a running speed".

## 1. Doors

**Clips.** `audit_1_door_player_5x` (the fresh nobody to Hotel#7254's door at 465,792, after `live 17` at 20x),
`audit_8_sleep_player_1x` (the same body going in, 1x), `audit_1_door_npc_1x` / `_5x` (#13238 at d21 00:00, Sleep,
whose leg ended on the street: no door in 20 s), `audit_1_door_npc2_5x` (#21359 at 00:00 leaving a building),
`audit_6_commute_20x` (#25081 going in at the Cemetery, f179-184), `audit_8_hangout_npc_20x` (#22811 leaving its home from
floor 1, f58-78), `audit_2_stairs_5x` (#12478 out of its tower, f139). Script `view/cityview/scripts/audit_1_door_player.play`.

**Reads right.** The player's click way walks the street at 0.70 m a frame at 5x (1.4 m/s x 5), steady; the walk to
the door follows the sidewalk.

**Reads wrong.**
- **Going in, the body walks through the facade into the block, then pops.** `audit_8_sleep_player_1x` f1-100: the
  player #33428, at the door tile 465,792 at tick 30281, is drawn walking on, 0.12 m a frame, through the bakery's
  counter and the wall next door for 10 s, toward its sim tile 465,788 (the hostel's interior slot, four tiles in),
  then at the tick f101 jumps 10.3 m to the room (the sheet's f109 is a bunk room). The cutaway is off for all of it
  (the body is "on the street" to the view).
- **The interior slot is the building's north-west corner.** `audit_6_commute_20x` f179-184: #25081 reaches the
  Cemetery's door 745,596 and is drawn walking 7.2 m a frame (2.6x the walk at 20x) north through the building to
  745,587, climbing z 79 -> 92 onto its roof, then jumps 26.3 m back to the spot inside (f184). `audit_2_stairs_5x`
  f139: #12478 ends its stairs at the drawn core (2251.5, 924.0), then pops 73.5 m north to its sim tile 562,687 (the
  slot), and slides 5.0 m a frame (7x the walk) 55 m back south through the tower to the street door 560,710 in one tick;
  the camera's subject jumps 1524 px; `paths.png` shows the spike. Same in `audit_8_hangout_npc_20x` f73-77 (#22811,
  754,590 -> 757,596, 7.2 m a frame, a 7.2 m back-snap) and `audit_1_door_npc2_5x` f58 (#21359, a 23.5 m jump, an 18.5 m
  back-snap, z 79 -> 92 -> 79).
- **The player stands at the door in the walk clip waiting for the sim.** `audit_1_door_player_5x` f101-160: the click
  way has ended at the door, the sim is two ticks behind (459 -> 465,793), and the body treads in place for 6 s.

**Causes.** Sim: `World::enter_building` (`citysim/src/exec/places.rs:249`) puts the body on "the first free interior
tile", and `Building::interior` (`components/building.rs:293`) runs row-major from the rect's north-west corner, so in a
big building the slot can be 20+ tiles from the door; `leave_building` (`places.rs:277`) puts it back outside the door in
the same tick, and a climb that ends `out` walks on from there at once (`exec/step.rs:193`). The 15-tile door hop of
HANDOFF 0b is this. View: the going-in branch (`view/cityview/src/sim.rs:977`, `entering`) draws the body along its way
to the newer tile, which is the slot inside, not the door; an indoor body is drawn at a fixed spot in the middle 60 % of
the lot (`indoor_place`, `sim.rs:635`), so the first indoor frame jumps from the slot tile to that spot; going out the
older frame's drawn spot (the core, or the lot's middle) is not the older tile, and the lerp starts from the tile.
`OPEN-307` (sim), `OPEN-308` (view).

## 2. Stairs and the lift

**Clips.** `audit_2_lift_5x` (#8692, lift 0 -> 40 in Building#952, d21 00:00, f1-200), `audit_2_lift_down_5x` (#13238, 15
-> 0 at 07:30, waiting at floor 15), `audit_2_stairs_5x` and `_1x` (#12478, stairs 5 -> 0 in Building#1945 at 07:30).

**Reads right.** The lift car rises smoothly, 3 storeys a tick (z 132 -> 216 m over 10 ticks; a tick 2 s at 5x), the
floors slide past it in the cut, the camera keeps it framed (98 px the worst jump). The stairs walk circles the core a
turn a storey, facing the way round, z falling 3 m a storey (f1-121).

**Reads wrong.**
- The lift is a constant 3 storeys a tick both ways (0.9 m/s at 1x for a 40-storey ride of 130 s), and the wait is 6
  ticks to the minute at every hour (`OPEN-266`: confirmed in motion, `depart` 30230 `board` 30236; #13238 `depart`
  30689 `board` 30695).
- Off the lift the body hops 3.1 m from the car to its spot on the floor (`audit_2_lift_5x` f198).
- The stairs' exit is §1's 73.5 m pop and 50 m/s slide (it is a door: the core is drawn at the lot's core, the sim's
  slot is the rect's corner).
- The stairs are drawn on the spot: 1.6 ticks a storey, a circle 0.5 m across at the core with no stair under it.

**Causes.** Sim: `levels::plan` times the lift as a constant (`OPEN-266`). View: the lift car's exit and the stairs'
exit are §1's indoor-place jump (`sim.rs:962`, the subject's climb drawn at `pl.core`; the stairs `sim.rs:1061`). Grouped into `OPEN-308`.

## 3. The El

**Clips.** `audit_3_el_wait_5x` (#10265 at the stop 823,619 at 07:30, `Ride` departing 30698), `audit_3_el_ride_20x`
(the same body from 30696: boarding, the Hillside Line ride west, the stop at 627,616, alighting at 569,616 and the walk
on). Also `audit_6_commute_20x` (#25081 walking off from the same stop).

**Reads right.** The ride itself: the rider is drawn seated along the car's wall, the car roofless for the subject, moving
17.6 m a frame at 20x along the deck, dwelling at the stop f61-75, the camera on it. No teleport while aboard.

**Reads wrong.**
- **The wait is on the road under the El, on a marked crossing.** The stop tile 823,619 is the zebra of the crossing
  821-824,617-618; #10265 and a dozen others stand on it (sheet f19-164), z 66.7, 33 m under the deck, for 8 ticks.
- **Boarding is a 33 m leap.** f179 (5x) / f15 (20x): z 66.7 -> 99.4 and 23.4-39.3 m in one frame, into the car; the
  camera's subject jumps 3811 px. Then the car's first frames run back east and round (yaw 94 -> 180, a 26.8 m back-snap,
  steps 15-27 m) before it heads west: the train is laid on the nearest rail deck to its tile, which at this stop is a
  curve.
- **Alighting drops 18 m.** `ride_20x` f105: z 86.9 -> 69.1 and 24.9 m sideways, from the car to the street.
- Riders stand in the idle pose in the car, facing across it (no sit).

**Causes.** Sim: the El's stops are street tiles at ground level (`Ride` waits on its stop tile, floor 0), so the
platform is not a place a body stands on; the view has the deck's z (`deck_z`) but a rider waiting is a street body.
View: boarding and alighting switch between the street draw and the car's seat (`sim.rs:1083`, the seats `sim.rs:1240`) with
no walk between. `OPEN-309` (sim and view).

## 4. A signalled crossing

**Clips.** `audit_4_crossing_c_5x` (#12385 at 17:30, crossing at 556,613), `audit_4_crossing_5x` (#15864 at 07:30: its
crossing at 627,576 is past the clip's end), `audit_4_crossing_b_5x` (#27183).

**Reads right.** The V7 wait is real: #12385 walks 568 -> 560,613 and stands at the kerb six ticks (31292-31298, 60 s at
1x, inside the 75 s red), then crosses at the crossing tile 556 and on (f178-200). The pole and its heads stand at the
kerb.

**Reads wrong.**
- The wait is drawn as the walk clip treading in place for 12 s (f61-177; film.py's `slide_walk_in_place` frames 59-177).
- It stands two tiles back from the painted zebra and crosses beside the stripes, not on them (sheet f182-200): the
  sim's crossing tile and the painted zebra do not line up at this junction.
- The walk's pace pulses: 0.6 then 0.8 m a frame at 5x, tick on tick (`audit_4_crossing_5x` f19-199; 3 then 4 tiles a
  tick), a 25 % surge every 10 s at 1x.

**Causes.** View: `walking` is the sim's act, not the drawn motion (`sim.rs:1006`: act 0 is Walk while the executor holds
the body at the kerb). Import or view: the X lines are imported at downsample 2 (`assets/maps/isle_m_k2.txt` 11937-,
e.g. `X 1279,177 1282,177`) and TheCityGraphics' zebra mesh is drawn at full resolution; which is off was not
established (capped). Sim: T1's executor steps 4, 3, 4 tiles a tick (`docs/TIME_MODEL.md` "Landed"), and the lerp draws
each tick's tiles over the tick. `OPEN-310` (view), `OPEN-316` (view/import), `OPEN-317` (sim).

## 5. A crowd

**Clips.** `audit_5_crowd_stack_5x` (#18308 at 17:30 by tile 303,759: 61 bodies on it), `audit_5_crowd_rough_1x` (#9056 at
646,712: 32 bodies, `Use` Sleep and Idle), `audit_5_crowd_patrol_5x` / `_1x` (#15697 by 532,815: 43 Coarse patrolmen
`GotoTimed` on one tile).

**Reads right.** The V4 spread lays a standing crowd on the walkable ground round its tile, not on the road.

**Reads wrong.**
- **A crowd evaporates in one tick.** `crowd_stack_5x`: at f15 61 bodies stand on 303,759, every one of them tier
  `Stat` in the camera's view (`OPEN-149`'s rule broken at the film's start); by f19 56 of them are indoors and by f40
  none is traced (`pop` 62). They walk into the nearest building in one tick, jumps to 10.9 m a frame (#26743, 303,759 ->
  303,770).
- **A patrol moves as one clump at 27x the walk.** `crowd_patrol_5x` f28-30: about twenty patrolmen drawn as one tight
  knot sliding 19.2 m a frame (#21634, its tile 577,801 unchanged), gone by f42.
- **Rough sleepers stand in a queue.** `crowd_rough_1x`: the 32 on 646,712 stand along the wall in two rows for the whole
  clip, interpenetrating, a few behind the railing; the sleepers among them stand (`OPEN-285`'s sleeper, on the street).
- A body promoted into the ring slides from its stale place: #22305 (`crowd_rough_1x` f116-120) is drawn inside its
  building moving 4.1 m a frame at 1x (29x the walk) on its first frames.

**Causes.** Sim: a Statistical body's `Position` is a stale snapshot (`lod::snap_to_phase_door`,
`citysim/src/systems/lod/assign.rs:397`; `OPEN-149`), and dozens share one tile; the first Full tick sends them all into
the door at once. A Coarse `GotoTimed` leg holds its tile until arrival while the view walks a whole leg's `ahead`
inside one pair (`sim.rs:1091`, `along`). `OPEN-312` (sim), `OPEN-318` (view and sim).

## 6. An NPC commute

**Clips.** `audit_6_commute_20x` / `_5x` (#25081 at 07:30, from the El stop 823,619 west to the Cemetery at 745,587, 102
tiles), `audit_1_door_npc_5x` (#13238, a Sleep walk at 00:00).

**Reads right.** A long walk on the sidewalks at an even 3.2 m a frame at 20x (f29-163), turning at corners, the camera
behind (the known lag: the subject held at ~(420, 170) px).

**Reads wrong.**
- **A walker stops for 20 s at every segment end.** `audit_1_door_npc_5x` f59-98: #13238 reaches 717,613 (its exec
  target: `Street`, a segment's end) and stands two ticks, then treads in place a third, then walks on east. The player
  stopped the same way, two ticks at 404,793 (`audit_1_door_player_5x` second run f60-79), while its click way drew it
  walking on.
- It slows to a crawl before the door: 0.8 m a frame for its last eight tiles (748 -> 745,596, f164-178) against 2.4-3.2
  before.
- The going-in is §1's walk through the building.

**Causes.** Sim: a walk past `[legs] leg_replan_tiles` (70) goes in segments (`exec/board.rs:187`, `segment`); each end
re-plans the rest at the body's next think, which here took two ticks. The slowdown before the door was not traced
(a door queue or crowd block, `exec/goto.rs` `try_enter`; capped). `OPEN-311` (sim), `OPEN-317`.

## 7. A fight

**Clips.** `audit_7_fight_1x` (`provoke` on the fresh nobody: #9964 goes for #33428), `audit_7_flee_1x` (`order flee`),
`audit_7_lethal_1x` (`order stance allout`, `provoke lethal`), `audit_7_after_5x`. Script
`view/cityview/scripts/audit_7_fight.play`.

**Reads right.** Each beat lands a second apart with its words over the bodies ("HIT right arm (staggers)", "dodge",
"swing"), and the lunge and the stagger lean the right ways (lean to 0.27 rad, strip f26-33).

**Reads wrong.**
- **The street freezes while you fight.** The sim tick stays 30240 for the 15 s of `fight_1x` and 30242 for the 20 s of
  `lethal_1x`; every bystander stands frozen mid-crowd (16 bodies round the fighters, the strip), none reacts or steps
  back.
- **The fight hardly moves.** A lunge is 0.1 m (the nudge), the fighters never close or circle (#9964 stays 1.7 m off);
  between beats #9964's facing flips 64 -> 33 deg and back (f12, f29, f39 ..), and the player's -116 -> 228.
- **Flee does not run.** `flee_1x`: the player stands at 1337.8, 575.9 for all 100 frames after `order flee`.
- **Lethal shows nothing new.** `lethal_1x`: 20 s of the same beats, no body down, no lying pose.
- The player stands inside a bystander (#20383 within 0.3 m of #33428 for 150 frames): the arrival street's stack.

**Causes.** View clock: while the possessed body is engaged the sim thread steps `beats_per_tick` beats a `beat_s` apart
and holds the walkers' rate at 0 (`view/cityview/src/simthread.rs:1383-1385`, `walking_rate`), so the tick (and every
walker's lerp) waits for the fight. View: the lunge is a pose on the existing rig (`sim.rs:1330`, `lunge_m`), timed
on the wall clock (film P1's caveat), and a non-lunging fighter keeps its last sim facing. Sim: Flee and the bystanders
were not traced (capped). `OPEN-313` (view clock, sim), `OPEN-314` (view, sim).

## 8. Indoors

**Clips.** `audit_8_sleeper_npc_1x` (#7815 asleep in Building#1017, floor 3, 00:00), `audit_8_hangout_npc_20x` (#22811 in
the same building, leaving), `audit_8_jail_guard_1x` (#7954, GuardJail in the Jail#5986 with 46 others),
`audit_8_sleep_player_1x` (the player into the hostel; "Sleep at" was no row, so no sleep).

**Reads right.** Indoors the subject's storey is cut open and its flatmates are drawn on it; the climb out to the street
from floor 1 is the stairs' circle (f58-68).

**Reads wrong.** The sleeper stands idle at its fixed spot for 12 s (`OPEN-285`: confirmed in motion, no new form); nobody
indoors moves while a Use runs (every indoor body holds one spot for its whole activity); going in and out is §1.

## 9. The player clicking around

**Clips.** `audit_9_click_1x`, `audit_9_reclick_1x`, `audit_9_click_5x`, `audit_9_reclick_5x` (script
`view/cityview/scripts/audit_9_click.play`, `zoom 90`), and the hostel's menu walk in §1.

**Reads right.** The body moves at the click (T2 § 5): 0.14 m a frame at 1x from the first frame; a re-click mid-walk
turns it round at once and it walks the new way (`reclick_1x` f1-120); at 5x the walk ends on the clicked tile and the
body stops (`reclick_5x` f41-57).

**Reads wrong.**
- **The click walk glides in the idle pose.** `click_1x` f1-80 and `reclick_1x` f1-53: the player moves 1.4 m/s with the
  idle clip (act 2) until the sim's first tick makes it a Walk, 5 s at 1x (film.py `slide_idle_moving`).
- **The drawn player does not follow the sim when they part.** First run of `audit_1_door_player_5x` (a script line at a
  later tick ran the sim flat out): the drawn body stayed 43-52 tiles (170-210 m) behind its sim tile for the whole 24 s
  (drawn x 1390.7 at tile 399; 1558 at tile 432), and when the sim held the body two ticks at 404,793 the drawn body kept
  walking. At the way's end it treads at the door for the sim (§1).
- The far-tile click could not be filmed (off the window at every zoom tried).

**Causes.** View: `walking` comes from the sim's act (`sim.rs:1006`), not from the click way that moves the body
(`sim.rs:1095-1112`); the click way is walked at the view's own clock and handed back only at its end or when the sim's
tile leaves it (`way_over`), so a sim that is ahead, behind or waiting is not followed. `OPEN-310`, `OPEN-315` (view).

## 10. Along the way

- The camera's first frame of every follow film is somewhere else (`sheet_1` f1 of most clips: "subject OFF SCREEN at px
  (nan, nan)", or another lot): the follow-cam is placed one frame late. The known camera row; noted, not re-filed.
- The El wait clip and the arrival street hold 16 bodies within 0.3 m for whole clips (known stacks).

## The ranked list

Ranked by how much each breaks "people acting like people" for a player watching:

1. `OPEN-307` + `OPEN-308` **Doors** (sim + view): every going-in and coming-out is a walk through walls and a 10-73 m pop;
   the camera jumps with it. A player sees it at every door, the commonest thing a body does.
2. `OPEN-313` **The street freezes during the player's fight** (view clock, sim): the whole visible city stops for every
   fight; nobody reacts.
3. `OPEN-309` **The El** (sim + view): waiting on the road under the deck, a 33 m leap aboard, an 18 m drop off.
4. `OPEN-312` **Crowds evaporate on promotion** (sim): dozens of bodies on one tile, then all gone in a tick.
5. `OPEN-315` **The player's drawn body and the sim part** (view): it walks through the sim's waits and can trail it by
   200 m.
6. `OPEN-311` **Segment-end stops** (sim): every long walk halts 20 s at each 70 tiles.
7. `OPEN-310` **The clip follows the act, not the motion** (view): treading at kerbs and seams, gliding on a click.
8. `OPEN-314` **The fight barely moves; Flee does not run** (view + sim).
9. `OPEN-318` **Clumps and queues** (view + sim): a patrol as one knot at 27x the walk, sleepers queued standing.
10. `OPEN-317` **The pace pulses and crawls before doors** (sim).
11. `OPEN-316` **The crossing off the zebra** (view/import).

Sim-side: 307, 309 (the stop's place), 311, 312, 313 (the bystanders), 314 (Flee), 317, 318 (the Coarse leg). View-side:
308, 309 (boarding and alighting), 310, 313 (the clock), 314 (the poses), 315, 316, 318 (the draw of a leg).
