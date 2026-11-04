# The time model (OPEN-304)

Decided with Dylan 2026-11-03, after the slice's 60x walk read as a teleport. The standing decision (CLAUDE.md, "The
game feels realtime at normal speed") asks for three things that cannot all hold at once: a body on screen walking at
a real pace at 1x, a compressed day at 1x, and the city's real distances in game time (a 1 km walk ~12 game minutes,
the pace every walk-timed number had been built on since round 9 Y4).

**The pick: compress the day 6x and let space give.** At 1x a game day is **4 real hours** (a game minute is 10 real
seconds); a body walks 1.4 m/s on screen; in game time the walk is 6x slower than real, **3.5 tiles a game minute**
(14 m, 0.84 km/h). A person's day keeps its shape in game hours (an 8-hour shift, a 2-hour commute cap, sleep at
night); what shrinks is each life's reach in metres: you live, work and drink within a few blocks, and the El is the
long-distance link. Dylan first took "1x is real time, the day compressed only by auto-speed" (a day of 24 real hours
at 1x), then asked whether that was too slow: almost no game ships it (GTA V and RDR2 48-minute days, Skyrim 72,
Stardew ~15; true realtime is MSFS and Arma). The 6x middle is the answer.

| C (compression) | day at 1x | sim walk, tiles a game min | 1 km walk | reach in 120 ticks | reach in 60 |
|---|---|---|---|---|---|
| 1 (before) | 24 h | 21 | 12 game min | 10 km | 5 km |
| **6 (picked)** | **4 h** | **3.5** | **72 min** | **1.7 km** | **840 m** |
| 30 (GTA) | 48 min | 0.7 | 6 h | 340 m | 170 m: lives break |

**The probe** (2026-11-03, 20k seed 42 x 30, one run each, only `[exec] walk_tiles_per_tick` 21 -> 7 with
`move_ticks_full` 1 -> 2, nothing else converted): the city held. No starvation either way, homeless share 0.7% ->
0.8%, employed share from day 30 61.3% -> 63.0%, hunger and mood flat, ticks/s 149 -> 174. El rides a day 1,744 ->
8,965, cab fares 7x. Fights 0.59x, murders 134 -> 71, arrests 0.59x, hunts started 0.49x, raid marchers who missed
14 -> 92, hotel nights 0.34x. **The violence drop is a FINDING, not retuned** (Dylan: "we can tweak later"); the
pressure comes back through features (`OPEN-109` street life and its kin).

## 1. The sim: one pace, and what it breaks

The tick stays one game minute. The rule for every number:

- **What a person's day is made of keeps its game time**: shift hours, the commute and job caps in ticks
  (`[jobs2] job_cap_ticks`, `home_cap_ticks`, `bed_cap_ticks`), venue caps, sleep, hunger. A 2-hour commute cap
  stays 2 hours; it now reaches 1.7 km, not 10 km. That is the design, not a break.
- **What moves on screen keeps its real speed**, so its game-time pace drops 6x with the walk: every vehicle (the
  `[vehicles]` mults are shares of the walk and follow it), the El (`[transit] ride_ticks_per_tile` is its own
  number: x6), cabs, vans, robots, drones, a pursuit, a climb. A physical duration the camera sees (a dwell at a
  stop, a door's queue) is a real duration, so its ticks grow 6x.
- **A wait that exists to cover someone's travel grows with the travel** or the mechanism stops firing (a raid's
  muster, a hit's expiry, a cab's or van's arrival, a guard's escort): scale it, or bring the parties nearer (muster
  from closer, call the nearer post), whichever reads as people.
- **A reach in tiles that was written as a time** ("20 min at 21 tiles a minute") is converted to keep the time or
  kept as a place on the map; the comment says which.
- **What the shrink breaks for real is fixed as a feature**: a body whose one place (a School, the letting desk at the
  Civic Hall, the Precinct, the clinic) now lies beyond a day's reach rides the El or finds the nearer one; where no
  nearer one exists the gap is an OPEN row.

## 2. The view: the speeds

- **1x is the walk-matched rate** (`[view] walk_m_s` 1.4: rate = walk_m_s / (tile_m x walk_tiles_per_tick /
  move_ticks_full) = 0.1 ticks a second): a game minute 10 real seconds, a day 4 real hours. The game starts at 1x
  (`start_speed_i` 0; P5b's 60x start goes).
- **Manual steps** (`,` / `.`, the HUD's speed row): 1x, 2x, 5x, 20x, 60x. Everything speeds up together,
  Kenshi-style: 5x is GTA's 48-minute day, 60x a game day in 4 real minutes.
- **Auto-speed** (`[view] auto_speed` 600x, a game hour a real second, capped by what the sim thread manages): the
  clock runs fast on its own through stretches with nothing to watch, and comes back when something happens.
- The HUD reads the speed as a multiple and, while auto runs, why ("asleep, 600x"), on one line (P5b's "4 h/s"
  wrap goes).

## 3. Auto-speed: when the clock runs ahead and when it comes back

The possessed body is **in a stretch** when it is doing something long it does not steer: asleep (in its bed, squat,
capsule or cell); on shift inside its workplace (a work beat's line still lands in the story log); riding (the El, a
cab, a van); in a door's queue or a letting desk's list; serving time. A walk is **not** a stretch: the street is the
thing to watch, and the player speeds it up by hand. An indoor activity the body chose (eating, hanging out) is a
stretch only on autopilot with nobody within `autopause_tiles` speaking to it or coming for it.

The clock **comes back** (ramped over `[view] auto_ramp_s` 1.0 real seconds, so the slowdown is seen) when the
stretch ends (it wakes, the shift ends, the ride arrives, the queue lets it in), when the human presses a key or
clicks, or when someone approaches or speaks to the body; it returns to the human's chosen speed, not always 1x. The
auto-pause triggers (an attack, a fight within `autopause_tiles`) **pause** as now, whatever the speed. The auto
toggle sits beside the auto-pause one in the Esc menu (`[view] auto_speed_on` true at the start).

## 4. Drawing a walk at any speed: no teleports

- The view lerps each body along its `ahead` (the way to the next tick's place) between the two newest frames
  (`sim.rs` `along`). The rule: **the way between two drawn frames covers the whole path walked between them,
  however many ticks passed**. When the sim thread ticks more than once a drawn frame, the frame it publishes carries
  the path since the last published one (concatenated `ahead`). Where the way does not reach the newer place, `along`
  falls back to a straight lerp through the block: that fallback is the bug to remove, not a draw mode.
- A body going in or out of a door mid-tick is drawn to the door and through it, then hidden.
- Motion V1 (the films' fix pass): **one clock for the lerp and the pacing** (simthread `ViewClock`): the view's game
  time runs at the speed now, the lerp's `u` is it between the pair's ticks, and the sim runs the next tick when it
  reaches the newest, so a new pair starts where the last ended (a speed change, a pause, a hold, a load: no freeze, no
  forward snap). Each drawn body keeps a track (cityview `motion.rs`): its offset (a lane, a crowd's slot) eases, a
  drawn place never steps off its way past a frame's walk (it closes the gap instead), the facing turns at a rate, the
  walk clip plays at the drawn speed; a click's way is walked from where the body is drawn (`motion::rejoin`).
- At auto-speed the camera's subject is asleep, at work or riding; the street blurs, which is what passing time
  looks like. Nothing is skipped: the sim ticks every minute at every speed.

## 5. The player's body: no tick's lag

At 1x a tick is 10 real seconds, so a click's walk would wait up to 10 s for the next tick. **The possessed body moves
at the click**: the sim thread plans the click's walk at once (between ticks), publishes the body's way, and the view
walks it along that way at `walk_m_s` from the click; at the next tick the sim's place agrees, the paces being the
same. A click's verb (buy, talk, apply) lands at the tick the body arrives. This is the seam WASD rides on after v1
(client-side continuous movement, the sim reading the body's tile each tick).

## 6. What does not change

The tick (a game minute); the hours of a day and every schedule in them; the combat clock (`[view] beat_s`, a beat a
real second while the possessed body fights, slower than 1x's tick); the Statistical tier's logic (its tables regen
from Full as ever); the 2k and 20k tiers.

## The phases

- **T1 the pace (sim)**: § 1. The pace in config (smooth, no 2-tick bursts), every walk-, speed- and travel-timed
  number classified by § 1's rule and converted or kept with a comment, what the shrink breaks fixed or written as an
  OPEN row; `SAVE_VERSION` bumps and the slice save is remade. Proof: one before/after 20k s42 x 30 run (it moves
  numbers, printed as FINDINGs) and the diaries of the commuter, the El rider, the raider, the guard on an escort,
  the homeless body's FindHome and the schoolchild.
  **Landed 2026-11-03**: `[exec]` 7 tiles every 2 ticks (the Full executor steps 4, 3, 4 a tick, no burst); the
  vehicle mults follow; the El's ride, its dwell, the flyers, the stairs, the lifts and the crossings' cycle x6; the
  tile caps Y4 had written as minutes (`shop_price_tiles`, `rough_min_tiles`, `bed_margin_tiles`, `cab_min_tiles`,
  `hail_min_tiles`, `haul_reach`, `round_reach_tiles`, `leg_replan_tiles`, `escort_walk_tiles`) back to their minutes
  (/6); the day's budgets in ticks kept. Judged, not by rule: `transfer_ticks` 5 and `van_call_ticks` 15 kept (a
  platform change and the nearer van), `muster_walk_cap_ticks` 120 -> 180 (the farthest member posted). Fixed as
  features: a letting desk is reached by its trip, the walk or the El (`[housing] desk_reach_ticks`), and a raid's
  muster holds for a member due at it within `[crew] march_wait_ticks`. Deviation: `assets/stat_table.toml` not
  regenerated (`OPEN-306`); the far schoolchildren are `OPEN-305`.
- **T2 the clock (view)**: §§ 2-5 in cityview (`play.rs`, `simthread.rs`, `sim.rs`), the `[view]` keys documented.
  Proof: a play script that walks the nobody across a block at 1x, sleeps through a night at auto and is woken by a
  fight, read in the story log and the smoke PNG at 1x and at 60x.

**T2 as built** (2026-11-03, cityview `simthread.rs`, `sim.rs`, `play.rs`; `player::stretch`, `way_now`, `start_now`):
- **The slice's teleport was the lerp, not the walk.** `sim.rs` ran `u` = real time since the newer frame x the
  walk-matched rate, without the play speed: at 60x it reached u 0.017 when the next frame came (logged with
  `CV_LERP_DEBUG=1`), so a body crawled 1.7% of a tick's 84 m and then jumped the rest once a real second. A walk
  begun inside a tick (the older frame had no way ahead) slid straight through the block as well. Now each published
  frame carries its lerp span (the ticks' own real pacing) and `u` is the time over it.
- **No teleports (§ 4):** the sim thread publishes once the render thread has taken the last frame (or after `[view]
  trail_max_ticks`); the ticks between extend each body's and vehicle's way (`Trail`), stitched onto the older frame.
  A way that misses the newer place is searched on the street within `trail_repair_tiles`; farther, `along` draws a
  jump at the pair's middle (a placement), never a slide. A body going in at a door walks to it, then hides.
- **Auto-speed (§ 3):** `player::stretch` (asleep or about to, at work inside, riding, a queue, serving time, an
  indoor activity on autopilot); geometric ramp over `auto_ramp_s`; a key, a click, someone near naming or targeting
  the body, or the auto-pause latches it off until the stretch changes. The HUD's line: "auto: asleep, 600x".
- **The click (§ 5):** a walk's order is applied between ticks, its way planned (`player::way_now`) and its step begun
  (`player::start_now`); the view walks the way at the clock's pace from the click. The drawn body leads the frames
  by at most a tick and hands back when they reach the way's end or leave it.
- The `[view]` keys come from the `--config` file over a save's copy; the pace (`[exec]`) stays the save's, so the
  walk-matched rate follows T1's pace in a remade save with no view change. Script verbs: `live SECS`, `autospeed
  on|off`, `click ~DX,DY`; the session `view/cityview/scripts/t2_clock.play`.

**Defaults picked by the orchestrator, not yet heard by Dylan:** the manual steps (1, 2, 5, 20, 60), auto at 600x,
walks never auto, chosen indoor activities auto only on autopilot, auto returning to the human's speed, the 1 s ramp.
