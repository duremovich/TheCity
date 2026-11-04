# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-11-04, the thirty-sixth session: films and the movement pass** (Dylan away ten hours; the cold play skipped by
his call). The slice and combat state of the thirty-third to thirty-fifth sessions is summarised under Main.

## Main

Main (`81628da` or later) holds **the player slice through P5h, the time model, and today's movement pass**:
`SAVE_VERSION` 136, the slice save remade and in place (`saves/slice/20k_s42_d21.ron`, v136; the v134 and v135
copies beside it as `.bak`). Change tier green on every merge (last: R4
`[change] GREEN; BUDGET OK 2m39s of 4m00s`); view tier green (last: R4 `[view] GREEN; BUDGET OK 1m32s of 7m00s`);
tests 1,471 (none added today; five restaged by S3, three by S1).

**Watch first:** `D:/TheCity/target/films/reel/movement_before_after.mp4` (gitignored, ~2 min: nine before/after
pairs side by side: the 60x walk, an NPC at 5x, doors, out of a tower, the El, a crowd, the fight, going to bed, a
re-click). Every clip is `target/films/<name>/film.mp4` with its sheets, `strip.png`, `paths.png`, `findings.txt`.
Rebuild the reel: `python target/films/reel/make_reel.py before=after ...` (the script's PAIRS list).

**Play it** (repo root): `cargo run --release -p cityview -- --slice`. Player saves go to `saves/player/NAME.ron`.
Scripts of clicks: `--play @FILE` (`view/cityview/scripts/`).

### This session (all merged and pushed)

| phase | agent | what it did |
|---|---|---|
| film P1 | Opus | `film NAME SECS [FPS]` play verb and `--save F --follow N --film NAME:SECS:FPS`; a fixed-step film clock (frames at even game-real intervals); `trace.csv` (drawn place, sim tile, lerp pair, clip, tier, screen point) and `meta.json`; `tools/film.py` (mp4, 4x3 sheets, `--strip N`, `paths.png`, FINDING lines: teleport, back-snap, frozen, slide, stack, floor pop, facing snap, clock stall, pops, camera jump); `[view] film_trace_m`; TESTING § Filming motion |
| movement audit | Opus | `docs/MOVEMENT_AUDIT.md` by film (doors, stairs/lift, El, crossings, crowds, commute, fight, indoors, clicks), `OPEN-307`..`318` |
| motion V1 (view) | Opus | one view clock for lerp and pacing (no freeze after a hold), `motion.rs` tracks (eased offsets, a jump guard, turn rate, walk/idle and clip rate from drawn speed), crowd slots, the click way rejoined from the drawn body, the camera kept on the subject at every speed |
| sim S1 | Opus | the door slot nearest the door and a walked exit (`307`), segment ends flow on (`311`), litter a share of a step (the door crawl, `317`), `exec::stride`, El waiting places on the deck and level boarding (`309` sim), stops off the zebras; `SAVE_VERSION` 136 |
| sim S2 | Opus | the first frame after a load drawn before the ring ran (`312`): `World::view_now`, a Stat street body drawn in its schedule's building, promotion into its own Home/work; Flee runs 2x; street spots capped (`[life] spot_cap` 3, `289`); bystanders stir at a fight (flee or watch) |
| motion V2 (view) | Opus | one clock for the fight and the street (`313`: no freeze; above 1x the city slows to the fight), doors drawn as one walk (`308`), fighters staged, circling, facing, strike/stagger poses, **procedural poses** (sleepers lie; rest, drink, eat, beg sit; downed lie), walker lanes, the click walk clamped to the sim (`315`), stride drawn, walk on/off the El car, `goto` verb, the ring at once in `take()` |
| sim S3 | Opus | S2's view gates removed (spread, run, stir, enter-on-promotion always on, so the shadowed city is the played one; five tests restaged, not loosened), walkway hops timed (`319`), patrol first pick and muster, spill on arrival (`318` sim), Sleep capped at 12 h and a day wake (`302`), a climb's exit through the core, `ViewAct::Timed.depart` |
| sim S4 | Sonnet | the climb's entry walked slot -> core (`320`); the first-tick load jump was already gone with `view_now` (probe 7 -> 0) |
| motion V3 (view) | Sonnet | the player faces only a staged foe, a run look (lean, bent arms) from ground speed, a long way inside one pair drawn as its chord (`crossing_c` teleports 129 -> 26), standing bodies take their crowd slot at once |
| fight R4 | Opus | the fight regression: provoke had fought a passing bus rider (S2's `312` fix drew the stale crowd at the slice arrival indoors, so nobody was on foot near); riders never fight or join, provoke walks the nearest Full adult on foot up (`combat::come_for`, `[view] provoke_*`), a lunge stops short (`lunge_sep_m`); the hop drawn along its span (`exec::hop_span`, `319`'s view half); facing, the run look, strike and stagger read right on its crops (`314`) |

**Numbers (films, FINDING counts, one clip each, before -> after):** the 60x walk: camera loses the subject 230 px ->
3-4 px, teleports 47 -> 0; NPC at 5x after a load: frozen 19 of 59 frames -> 0, teleports 13 -> 0; a crowd at 17:30:
Stat/Coarse bodies in view 96/3 -> 0/0, teleports 375 -> 0, worst tile 61 -> 3; out of a tower: the 73.5 m pop ->
a 9 m/frame slide (then S3/S4 timed the core walk); the El: boarding leap 33 m -> ~6 m, camera 3811 -> 77 px; the fight:
the street frozen 15-20 s -> walks on; R4: a foe drawn 0 -> 13 beats over 3 min, the lunges 0.11 -> 0.54 m apart; the
lift hop: a 17 m one-frame drop -> over ten ticks (`floor_pop` 78 -> 59 at 5x, 133 -> 116 at 20x).

## Decided (Dylan)

- **Films are the proof of motion** (36th session, Dylan: "grabbing either videos or several frame snippets so you can
  see the game moving in time"): a phase that changes how bodies move on screen proves it with films before and after
  (`docs/TESTING.md` § Filming motion); agents read `paths.png`, strips, sheets and the trace's FINDINGs (they see
  images, not video); the mp4 is for Dylan.
- **Procedural poses** until real clips exist (36th session): lying, sitting, the guard, strike, stagger and the run
  lean pose the Mixamo rig in code (`people.rs pose_bones`); only `walk` and `idle` are clips.
- **The build and the layout** (35th session): the release profile is incremental; no ECS framework, SoA or crate
  split; files split at domain seams under a 1,500-line warning, never a gate.
- **The game is capped at 30 fps** (35th session): `[view] fps_cap`.
- **The time model** (34th session, `OPEN-304`, `docs/TIME_MODEL.md`): the day is compressed 6x and space gives, not
  time; violence ~0.5x left ("we can tweak later").
- **The combat model** (`OPEN-179`, `docs/COMBAT.md`, addendum 26); **the ladder** (`OPEN-294`, addendum 27, later);
  **autopilot is safe idle**, standing orders later (`OPEN-301`); **a new nobody: both**; **housing is an errand**.

## For Dylan to decide (from this session)

1. **Fast speeds' feet**: the walk clip's rate is clamped at 6 (`[view] walk_clip_rate_max`): at 20x/60x the stride
   slides; at 20 (matching the ground) the legs jump between unrelated poses at 30 fps
   (`target/films/v2_rate6_20x` vs `v2_rate20_20x`). Kenshi-like is the latter; V2 kept 6.
2. **The fight's pace at 1x**: one clock for the fight and the street means a beat lands every ~1.7 s at 1x (it was
   1 s, with the street frozen). 1 s beats would need the street at 1.67x during a fight.
3. **Real clips**: a run, a sit on a chair, a swing, a fall, a climb would replace the procedural poses (your Mixamo
   login); chairs/seats do not exist, so bodies sit on the floor.

## Next, in order

1. **Dylan watches the reel**, then plays 30 minutes cold (the slice's exit, skipped this session):
   `cargo run --release -p cityview -- --slice`. His notes become OPEN rows owned by the next pass.
2. The movement rows still open: `OPEN-321` (a walk the view hands a body is re-thought like any plan),
   `322` (the lift hop drawn as a ramp, no car), `323` (the fight's staging snaps; fists pass into the struck), `324`
   (the step off the El jumps at 20x), `309` (the walk onto the car is drawn; check and close), `316` (the zebra:
   likely `Sim::metres`' per-body jitter), `318` (patrol knots in view), `285` (bodies sit on the floor
   and against partitions).
3. The slice rows: `OPEN-298` (fist fights thin), `299` (the guard in Bar#5956), `300` (knockouts not saved), `303`
   (FindHome at the edges), `290`-`293`; P5b's leftovers (the click-menu title reads the building, the click menu on
   the death screen, "4 h/s" wraps in the HUD).
4. After the slice: sight and the shot (addendum 24, `OPEN-288`), then netrunning (addendum 25).

Frozen until the slice reads: `OPEN-278`, `285`, `287`, `265`, traffic (`275`), lighting (`284`), chrome credit
(`286`), calibration. Opened this session: `OPEN-307` to `324`; closed: `289`, `302`, `307`, `308`, `310`-`315`,
`317`, `319`, `320`.

## Process notes from this session

- **Films found what diaries and stills never did**: the freeze after every hold, the first-frame Stat crowd, the
  door hop, the El leap. The fixed-step film clock (P1) makes a film true-pace however slow the capture.
- **Parallel sim and view coders worked**: S1/S2 (sim, disjoint files) beside V1/V2 (view); messages carried the
  interfaces across (`exec::stride`, `view_now`, `Timed.depart`). Each merge needed one `cargo check -p cityview` on
  the merged tree (all green).
- **Subagents cannot write outside their worktree** (main's `target/agent_reports/` and the scratchpad refused
  most of them); films under `D:/TheCity/target/films/` did write. Briefs should ask for the report in the reply only.
- `cityview --slice` reads `saves/` relative to the worktree: pass `--save D:/TheCity/saves/slice/20k_s42_d21.ron`.
- A `timeout` that kills cargo mid-link corrupts the rlib (LNK1120): `cargo clean --release -p citysim`.
- A view-gated mechanism (S2) to dodge butterfly tests was the wrong trade: the shadowed city stopped being the
  played one; S3 restaged the tests instead.

## Cost (API-rate $ equivalent, `tools/session_cost.py`)

The thirty-sixth session (one transcript, eleven agents):

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| film P1 | Opus | 202 | 217k | 16 |
| movement audit | Opus | 260 | 193k | 17 |
| motion V1 (view) | Opus | 260 | 248k | 22 |
| sim S1 | Opus | 392 | 248k | 29 |
| sim S2 | Opus | 483 | 265k | 33 |
| motion V2 (view) | Opus | 477 | 344k | 44 |
| sim S3 | Opus | 469 | 223k | 29 |
| sim S4 | Sonnet | 163 | 110k | 5 |
| motion V3 (view) | Sonnet | 217 | 152k | 8 |
| fight R4 | Opus | 417 | 211k | 25 |
| orchestrator (plan, briefs, merges, reel, close) | Opus | ~300 | 231k / 87k | 19 |

The session ≈ $248. V2 (477 turns at a 344k median) and S2/S3 (~470 each) ran well past the ~300-turn brief; the
two Sonnet fix rounds cost $13 together. No coders or worktrees left.
