# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-11-03, the thirty-fourth session: the time model designed and built (T1 the pace, T2 the clock).** The
thirty-third session's slice work (combat C1-C5, P5, P5b, P5h) is summarised below as it stood; this session's
additions are step 0 of Next, the Decided list's first entry, and the cost table.

## Main

Main (`b9a69ae` or later, pushed) holds **the player slice through P5h plus the time model** (a 4-hour day at 1x,
`SAVE_VERSION` 135; **the slice save is still v134 and must be remade before play**: `cargo run --release -p
citysim-cli -- run --days 21 --seed 42 --population 20000 --scaled --ambient-flats 50 --map
assets/maps/isle_m_k2.txt --save-at 30240 --saves-dir saves/slice`, then rename `42-30240.ron` to `20k_s42_d21.ron`): the slice is playable end to end and waits on its
exit, **Dylan's 30-minute cold play**. Combat is a live engagement in beats (`docs/COMBAT.md`), the game starts from
a menu (pick a nobody or arrive fresh), dies for good, saves and loads, and a homeless body has to go and find a
place to live. Change tier green on every merge (last: P5h `[change] GREEN; BUDGET OK 2m33s of 4m00s`; view green,
budget breached on cold worktree Bevy builds, see the process notes); tests 1,471 (P5h retired the rehouse pass's
three); `SAVE_VERSION` 134.

**Play it** (repo root): `cargo run --release -p cityview -- --slice`. The pre-run save
`saves/slice/20k_s42_d21.ron` (gitignored) is in place at version 134; after any `SAVE_VERSION` bump the menu prints
the command that remakes it. Player saves go to `saves/player/NAME.ron`. Scripts of clicks: `--play @FILE`
(`view/cityview/scripts/`, e.g. `p5h_fresh.play`).

| phase | what it added |
|---|---|
| combat design | `docs/COMBAT.md` (addendum 26) with Dylan: live engagement in beats, wounds by region that last, melee/shooting/evasion, stance and orders under a slowed clock |
| C1 the beat | `systems/combat.rs` `resolve_now`, the duel chain, `[[weapon_kinds]]` (old tier became quality), `World::last_fight` |
| C2 wounds that last | `Wounds`: six regions, Graze/Wound/Grave, blood, marks (scar, maim, lost limb) |
| C3a live engagement | `Engagement` across ticks, helpers striking (`crowd_reach`), `combat::step` after exec |
| C3b numbers, joiners, chrome | joiners, morale (fatigue, split defence), knockouts, `Verb::Rob`, chrome onto `Implant(kind)` (Kerenzikov, Sandevistan T2+, gorilla arms, mantis blades), robots in the beats, `cover_at` |
| C4 the player | stances (all out, defensive, subdue, hold back), cityview steps the possessed body's fight a beat a second (`Live::stepper`); a human shakes, never breaks |
| C5 feel pass | closed `OPEN-295` to `297` (knockouts last, timid bodies fight back, the fight on screen); `298`-`300` left |
| P5 start and loop | `--slice` egui menu (pick a nobody, arrive fresh), Esc menu, save/load, autopilot as safe idle, auto-pause on an attack, permadeath into the same city, the blackmail as an offer; `citysim-app` retired |
| P5b cold-start polish (Sonnet) | 60x start and a speed row, the story log in plain words, the death screen cleared, the pick list across Sump districts |
| P5h housing as an errand | `systems/housing.rs`, `[housing]`: FindHome walks to a letting desk; the city's 10-day list, corps refuse a record / no job / no address, gangs charge 1.5x, private owners take the deposit; a deposit for all; the Capsule Hotel as the first rung (pay at 08:00 or lose it, lockers robbed); `ownership::rehouse` retired |

**Numbers** (20k, seed 42, 30 days, one run each, FINDINGs, nothing tuned): Murders 90 before combat, 116 after C5
(peaked 161 at C3b); a lone body wins 29% at 1v2, 16% at 1v3+; knockouts 22.8% of fight endings; no Sandevistan use
(the city's chrome is all T1, `OPEN-286`). P5h: homeless at d30 193 -> 146, squatters a day 203 -> 114, 150 refusals
(120 no job, 30 a record), 49 housed by a walked errand, hostels ~58% full.

**Deviations Dylan has heard:** the possessed body stays out of the hire pool, the job search and enlistment even when
driven (P5). P5h made the existing Capsule Hotel the hostel rather than a new building kind.

## Decided (Dylan, this session)

- **The time model** (34th session, `OPEN-304`, `docs/TIME_MODEL.md`): true realtime at 1x was too slow ("I don't
  know of any games that work that way"); the day is compressed 6x and space gives, not time: the walk 3.5 tiles a
  game minute, lives shrink to a few blocks, the El is the long link. The violence drop it brought (~0.5x) is left
  ("we can tweak later"); fix what the shrink breaks (done in T1).

- **The combat model** (`OPEN-179`, `docs/COMBAT.md`, addendum 26): the four recommended forks (above).
- **The ladder** (`OPEN-294`, addendum 27, a note for later): start brutal like Kenshi and get better; megacorps hunt
  you, a new identity costs your name; no cap on the power fantasy (an epic grind to storm the top corp's HQ); the
  murder-hobo route the hardest, a gang or corp of your own the better one.
- **C5 feel pass, then P5.**
- **Autopilot is safe idle**; later, standing orders like a deeper Kenshi squad system with a tunable auto-pause
  (`OPEN-301`).
- **A new nobody: both** (pick one, or arrive as an immigrant).
- **Housing is an errand** (`M18_PLAYER.md` § P5h), the same for NPCs, never done by autopilot; the first rung is a
  capsule bed.

## Next, in order

0. **A compile-time and code-efficiency pass** (Dylan 2026-11-03, the next session's first work): build times
   (the cold Bevy build ~11 min, the sim's release build, incremental rebuilds) and the code's own efficiency. Design
   it with Dylan before spawning; measure before and after.
0b. **The time model is built** (`OPEN-304`, `docs/TIME_MODEL.md`): T1 the pace (`9f4b0d2`: the walk 3.5 tiles a game
   minute, the El, stairs, lifts and crossings x6, the letting desk by trip, `SAVE_VERSION` 135; the slice
   save not yet remade, see Main) and T2 the clock (`ff75885`: 1x a 4-hour day, steps 1/2/5/20/60, auto-speed to 600x through stretches,
   no teleports: the lerp had ignored the play speed, the body moving at the click). Reads wrong, for after the play:
   long commutes for workers placed before the shrink (a 151-min ride, a teacher late at 09:05), 15% of raid marchers
   still miss, a Sump arrest costs a guard ~1.5 h (one Precinct), tower stairs 2 h to floor 40, the drawn body up to a
   tick ahead of the sim (a mid-walk click snaps it back), the big building's 15-tile door hop now visible. Violence
   ~0.5x is a FINDING (Dylan: tweak later). `OPEN-305` (no School out of reach), `306` (the Stat table at the pace:
   the regen cut off-screen murders 40 -> 1, the old table kept).
1. **Dylan plays 30 minutes cold** (the slice's exit): `cargo run --release -p cityview -- --slice`. His notes
   become OPEN rows owned by the next pass; triage them with him before spawning anything.
2. The slice rows that play confirms: `OPEN-298` (fist fights thin), `299` (the guard in Bar#5956 arrests
   everything), `300` (knockouts and the stepper not saved), `302` (the 22-hour sleep), `303` (FindHome at the
   edges), `289`-`293` (beggars' tile, nothing hunts a nobody, Coarse in the cells, work beats, no work for a nobody).
   P5b's leftovers: the click-menu title still reads the building ("Bar#5956"), the click menu stays open on the
   death screen, "4 h/s" wraps in the HUD.
3. After the slice: sight and the shot (addendum 24, `OPEN-288`), then netrunning (addendum 25). Later: the ladder
   (`OPEN-294`), standing orders (`OPEN-301`).

Frozen until the slice reads: `OPEN-278`, `285`, `287`, `265`, traffic (`275`), lighting (`284`), chrome credit
(`286`), calibration. Opened this session: `OPEN-294` to `304`. Closed: `295` to `297`.

## Process notes from this session

- **The view tier's 7-minute budget is unrealistic for a cold worktree**: a fresh worktree's Bevy release build took
  11m05s-11m10s (C5 hit 7m00s exactly). Either raise the budget for a cold target or share a Bevy build cache across
  worktrees; until then a breach on a new worktree is the build, not a regression.
- **Repeated "waiting on its own background work" notices** (Dylan asked): a coder with a long foreground job shows
  as waiting each time it yields; nothing was stuck. Coders that hand back early with a job still running keep their
  worktree locked (unlock after the agent ends).
- **Two branches on the same client files** (P5b polish and P5h both in `play.rs`, `simthread.rs`) merged cleanly;
  the orchestrator ran one `cargo check -p cityview` on the merged tree before pushing, as Dylan was about to play.
- Phases ran 324-437 turns against ~300; the cityview play sessions remain the turn sink.

## Cost (API-rate $ equivalent, `tools/session_cost.py`)

The thirty-fourth session:

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| T1 the pace (sim) | Opus | 416 | 236k | 26 |
| T2 the clock (view) | Opus | 242 | 272k | 22 |
| orchestrator (the design with Dylan, the probe, briefs, merges, close) | Opus | ~90 | 107k | 6 |

The session ≈ $54. T1 ran 416 turns against ~300 (43 log reads, 208 shell calls). No coders or worktrees left.

The thirty-third session (≈ $280):

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| C1 the beat | Opus | 363 | 239k | 26 |
| C2 wounds that last | Opus | 364 | 257k | 26 |
| C3a live engagement | Opus | 437 | 332k | **39** |
| C3b numbers, joiners, chrome | Opus | 364 | 260k | 28 |
| C4 the player | Opus | 324 | 339k | 33 |
| C5 feel pass | Opus | 395 | 363k | 38 |
| P5 start and loop | Opus | 355 | 326k | 33 |
| P5b cold-start polish | Sonnet | 147 | 124k | 6 |
| P5h housing as an errand | Opus | 396 | 266k | 29 |
| orchestrator (combat design, briefs, merges, close) | Opus | 150 | 147k | 22 |

The session ≈ $280: combat C1-C5 $190, the slice's start, polish and housing $68. No coders or worktrees left; the
stale `agent-a2d06b9d9985759f3` folder is deleted.
