# The time model (OPEN-304)

Decided with Dylan 2026-11-03, after the slice's 60x walk read as a teleport. The standing decision (CLAUDE.md, "The
game feels realtime at normal speed") asks for three things that cannot all hold: a body on screen walking at a real
pace at 1x, a day/night cycle compressed at 1x, and the city's real distances in game time (a 1 km walk ~12 game
minutes, the pace every commute, travel cap and walk-timed config number has been built on since round 9 Y4).

| option | what gives | cost |
|---|---|---|
| A, GTA-style (a 48-min day) | distances: the sim's walk drops to 0.7 tiles a game minute | a 1 km walk is 6 game hours, the island 2.5 game days; every walk-timed number rebuilt, commutes stop fitting a day (R2's crawl again) |
| B, fast walkers | the walk on screen: 1x runs the clock a few times real | bodies sprint (5.6 m/s at 4x): the teleport, slower |
| **C, realtime 1x and auto-speed (picked)** | the day at 1x: a game minute is a real minute | at 1x the sky barely moves; the day compresses through where the time goes, not through the 1x clock |

**C:** the sim does not change. One tick stays one game minute, `[exec] walk_tiles_per_tick` stays 21 (84 m a minute,
1.4 m/s), every walk-timed number stands. The time model is the view's clock: what speed it runs at, when it changes
speed on its own, and how a walk is drawn at each speed.

## 1. The speeds

- **1x is real time**: a game minute a real minute, `[view] walk_m_s` 1.4 walk-matched (the rate it already has).
  The game starts at 1x (`start_speed_i` 0; P5b's 60x start goes).
- **Manual steps** (`,` / `.`, the HUD's speed row): 1x, 2x, 5x, 20x, 60x (`[view] play_speeds`). Everything speeds up
  together, Kenshi-style; 60x is a game hour a real minute and a walker at 84 m/s, the fastest step a human picks to
  watch the street.
- **Auto-speed** (new, `[view] auto_speed` 3600x, an hour a real second, capped by what the sim thread manages): the
  clock runs fast on its own through stretches with nothing to watch, and comes back when something happens.
- The HUD reads the speed as a multiple and, while auto runs, why ("asleep, 3600x"), on one line (P5b's "4 h/s"
  wrap goes).

## 2. Auto-speed: when the clock runs ahead and when it comes back

The possessed body is **in a stretch** when it is doing something long it does not steer:

- asleep (Sleep, in its bed, squat, capsule or cell);
- on shift inside its workplace (a work beat's line still lands in the story log; it does not stop the clock);
- riding (the El, a cab, a van to the cells);
- in a door's queue or a letting desk's list;
- in the cells serving time.

A walk is **not** a stretch: the street is the thing to watch. A long walk at 1x is a real walk (1 km is 12 real
minutes); the player speeds it up by hand. An indoor activity the body chose (eating, hanging out at home or a bar)
is a stretch only on autopilot with nobody within `autopause_tiles` speaking to it or coming for it.

The clock **comes back to 1x** (ramped over `[view] auto_ramp_s` 1.0 real seconds, so the slowdown is seen) when the
stretch ends (it wakes, the shift ends, the ride arrives, the queue lets it in), when the human presses a key or
clicks, or when someone approaches or speaks to the body. The auto-pause triggers (an attack, a fight within
`autopause_tiles`) **pause** as now, whatever the speed. A manual speed above 1x set by the human is kept after a
stretch (auto returns to the human's speed, not to 1x); the auto toggle sits next to the auto-pause one in the Esc
menu (`[view] auto_speed_on` true at the start).

A played day is then roughly its street time: sleep and a shift go by in seconds each.

## 3. Drawing a walk at any speed: no teleports

- The view already lerps each body along its `ahead` (the way to the next tick's place) between the two newest frames
  (`sim.rs` `along`). The rule: **the way between two drawn frames covers the whole path walked between them,
  however many ticks passed**. At 20x and up the sim thread can tick more than once a drawn frame; the frame it
  publishes must then carry the path since the last published one (concatenated `ahead`), not one tick's. Where it
  does not reach the newer place, `along` falls back to a straight lerp, which cuts through a block: that fallback is
  the bug to remove, not a draw mode.
- A body going in or out of a door mid-tick is drawn to the door and through it, then hidden (not hidden at the tick's
  start).
- At auto-speed the camera's subject is asleep, at work or riding; the street around it blurs at 3600x, which is
  what passing time looks like. Nothing is skipped: the sim ticks every minute at every speed.
- OPEN-304's "a walk shorter than one tick" is read first at 1x, where a tick is a real minute: if a body still
  jumps at 1x, it is a door or a frame-pair gap, not the speed.

## 4. The player's body at 1x: no minute's lag

At 1x a tick is a real minute, so a click's walk would wait up to a minute for the next tick. **The possessed body
moves at the click**: the sim thread plans the click's walk at once (one path search, between ticks), publishes the
body's way, and the view walks it along that way at `walk_m_s` from the click; at the next tick the sim's place
agrees, since the paces are the same (84 m a game minute is 1.4 m/s). A click's verb (buy, talk, apply) lands at the
tick the body arrives. This is the seam WASD rides on after v1 (CLAUDE.md, a walking player character): client-side
continuous movement at 1x, the sim reading the body's tile each tick.

## 5. What does not change

The sim's pace and every number built on it; the combat clock (`[view] beat_s`, a beat a real second while the
possessed body fights, already slower than 1x's minute); the Statistical tier and the off-camera city; saves (the
speed and the auto toggle are the view's, not the save's).

## The phase

One view phase (cityview's `play.rs`, `simthread.rs`, `sim.rs`; no sim crate change beyond what § 4's plan between
ticks needs): §§ 1-4, the `[view]` keys documented in `config.toml`, the old 60x start and its comment gone. Proof: a
play script (`view/cityview/scripts/`) that walks the nobody across a block at 1x, sleeps through a night at auto
and is woken by a fight, read in the story log and the smoke PNG at 1x and at 60x; no new test.

**Defaults picked by the orchestrator, not yet heard by Dylan:** the manual steps (1, 2, 5, 20, 60), auto at 3600x,
walks never auto, chosen indoor activities auto only on autopilot, auto returning to the human's speed, the 1 s ramp.
