# docs

One line per live document: what it is and when to read it. Everything superseded is in `history/`.

## Read first

- `HANDOFF.md`: where main stands, what is in flight, the next steps, Dylan's open calls. Rewritten every session end.
- `../CLAUDE.md`: the doctrine, the standing decisions, the agent rules, the commands.
- `ARCHITECTURE.md`: the codemap (crates, tick order, LOD tiers, the brain, config → system, how to add a mechanism).
- `OPEN.md`: the one ledger of open items, `OPEN-nnn`; nothing is open anywhere else.
- `TESTING.md`: the tiers, budgets and caps; how to add a test; the god reaction classes.
- `PROCESS_REVIEW.md`: why the repo is laid out this way (the 2026-10-17/18 review, the research, the test process).

## The vision and the order of work

- `VISION.md`: the end state (revenge chains, fixers, contracts, the player's life paths). The why behind every spec.
- `ROADMAP_POST_M14.md`: the milestones after M14 and the addenda (Dylan's directives, dated; addendum 23 is the player slice).
- `BIG_PICTURE_2026-11-02.md`: the step-back review before M18 (why work and gangs read thin, the shortest path to playable, scope, code health) and Dylan's seven decisions: the player slice next, at 20k.
- `LIVES_AT_WORK.md`: work has a counterpart, evenings and households, ages and leaders lead, 20k scale (closed 2026-10-19). Dylan's answers recorded.
- `VIEW_MILESTONE.md`: the view milestone (closed 2026-11-02): config hot-reload, perf, people drawn as people, riders, the orbit camera and cutaway, the ring, the replay, crossings, the cells, the El; gate `SHADOW_V20.md` and `shadow_v20/` (brief `plans/shadow_v20.md`).
- `M18_PLAYER.md`: **the next milestone**, the player slice (rewritten 2026-11-02): a nobody in the Sump for 14 days at 20k in cityview; P1 the seam, P2 verbs, W work beats, P3 the board and talk, P4 consequences, P5 start and loop (exit: Dylan plays 30 minutes cold), P6 founding optional. The old 494-line spec is `git show 2c2e0fd:docs/M18_PLAYER.md`.
- `COMBAT.md`: the combat model (designed 2026-11-02, addendum 26, `OPEN-179`): the live engagement in beats, wounds by body region that last, melee/shooting/evasion, weapon kinds and chrome, stance and orders under a slowed clock; C1-C4 inside the slice, ranged with sight, the blast with netrunning.
- `MOVEMENT_AUDIT.md`: the movement audit by film (2026-11-04): doors, climbs, the El, crossings, crowds, a commute, a fight, indoors and the player's clicks filmed at 1x-20x; the defects ranked, `OPEN-307` to `318`, sim vs view.
- `TIME_MODEL.md`: the time model (decided 2026-11-03, `OPEN-304`): the day compressed 6x (4 real hours at 1x), the walk 1.4 m/s on screen and 3.5 tiles a game minute, lives shrink in space not time; auto-speed over stretches, manual steps to 60x, no teleports, the player's body moving at the click; phases T1 (the pace) and T2 (the clock).
- `NETRUNNING.md`: netrunning in the street (designed 2026-11-02, addendum 25): quickhacks by sight, hand deck or neural chrome, RAM and trace, deep runs on the overlay, the counter-runner and the strike team, the crew's chair guard, the blast; after the slice and sight and the shot.

## Milestone specs (closed; the why and the "Implemented: deviations" of each)

- `SPEC.md`: M0-M7, the base city (needs, brain, LOD, law, social, demography).
- `M8_FACTIONS.md`, `M9_LAW.md`, `M10_SCALE.md`, `M11_OWNERSHIP.md`, `M12_DISTRICTS.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M15_WORD_AND_BLOOD.md`, `M16_CONTRACTS.md` (16a and 16b), `M17_OUTSIDE.md`.
- `LIFE_L2.md`: the living city pass after M15 (jobs, fun, leisure businesses, the faction layer at the Statistical tier).
- `ECONOMY_V2.md`: the Real economy (wages from revenue, the World market, Missions, the camps).
- `P10_POLICY.md`: the switches retired; the safety net as policy levers. `CLASS_PAY.md`: graded posts.
- `HUNGER_SICKNESS.md` (plan: `plans/hunger-sickness.md`): synth food, malnutrition, bins, clinics.
- `HIT_MARKET.md`: organic paid kills, the Fixer's book, the buyer's silence.
- `TRANSIT.md`: the El. `JOBS_V2.md` then `JOBS_AND_LIVES.md`: the trades, the street, weapons, ages, the gray market, faith.
- `SCALE_20K.md`: the 20,000 city and the Ambient tier. `IMPORT_GRAPHICS.md`: the island map from D:\TheCityGraphics.
- `ISLAND_PASS.md`: the island pass (closed 2026-10-24): distance in every pick, company housing, vehicles and El trains with riders aboard; gate `SHADOW_V8.md` and `shadow_v8/`.
- `PERF_PASS.md`: the perf pass toward the 100k city, closed 2026-10-26 with the gate met (decisions R1-R6, byte-identical index work, parallel scoring, the P9 re-import, a quiet-box gate).
- `PRESSURE_PASS.md`: the live pass (opened and built 2026-10-26; the next round from Shadow V10): misery acts, the dead trades get customers, protection bites, reach and the social graph; fed by Shadow V9.
- `TURF.md`: gang turf (Dylan 2026-10-28, after San Andreas): cells held by presence, the corner, turf that pays, the war (provoked, answered, held or lost) and its peace; built in pressure pass round 6 (V2, V5).
- `POPULATION_STUDY.md`: how far the island city scales (OPEN-184, 2026-10-23): the 20k-150k curve, what binds (the import's cuts), the hot systems with exponents, the target (60k now, 100k after the index work), the optimisation roadmap.

## The judges

- `PREVIEW_PASS.md`: the live milestone, the pre-view pass (five shift shapes, encounters need bodies, SHADOW_V6 rows 1-7 and the dead trades; P1-P10; gate Shadow V7 at 20k).
- `SHADOW_V17.md` and `shadow_v17/`: the latest shadow pass (pressure pass round 8 at 100k, read against V16 in the same window, with round 9's input; critic brief `plans/shadow_v17.md`). `SHADOW_V16.md` and `shadow_v16/`: the one before (round 7, against V15); `SHADOW_V15.md` and `shadow_v15/`: the one before (round 6, against V14); `SHADOW_V14.md` and `shadow_v14/`: the one before (round 5, against V13); `SHADOW_V13.md` and `shadow_v13/` (round 4, against V12); `SHADOW_V12.md` and `shadow_v12/`: the one before (round 3, against V11); `SHADOW_V11.md` and `shadow_v11/` (round 2, against V10); `SHADOW_V10.md` and `shadow_v10/` (the pressure pass, against V9).
- `SHADOW_V6.md` and `shadow_v6/`: the latest shadow pass (the pre-view read at 20k: lives, the camera, the first week, the island probe; 298 diaries; `OPEN-137..150`). `SHADOW_V5.md` and `shadow_v5/`: the one before (Lives at work gate, 250 diaries; 21 bugs). `SHADOW_V4.md` and `shadow_v4/`: the one before (23 bugs, 12 features; § The sweep has the fixes). The pattern for the next one.
- `GOD_SCENARIOS_V12.md`: the live god-scenario write-up and its gaps.

## history/

Superseded, kept unchanged: `SESSIONS.md` (every session log, 2026-10-05 → 10-17, the old HANDOFF), `TESTING_2026-10-09.md` (the long testing page with the per-seed history), `GOD_SCENARIOS_V1`-`V11`, `SHADOW_V1`-`V3` with `shadow_v1/`-`v3/`, `BIG_PICTURE_2026-10-07.md`, `EXPERIMENT_LEARNED_STAT_POLICY.md`. Code comments and specs still cite them by these paths.
