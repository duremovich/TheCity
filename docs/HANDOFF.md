# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`; the eighteenth
session's close page is its last entry. Last rewrite: **2026-10-24, the nineteenth session: the island pass
(phases 0-10), its gate Shadow V8, the fix round F1-F3, and the close.**

## Main

Main holds **the island pass, closed** (`docs/ISLAND_PASS.md`, deviations at its end; `SAVE_VERSION` 67). Distance
counts in every pick, company housing sits in the workplace, vehicles and El trains move with riders aboard, and the
off-screen violence amplifier is gone. The close tree, a08d383, ran change, merge and milestone green.

| merge | phase | OPEN closed |
|---|---|---|
| `fa74b64` | 0: the pre-view switches retired, the change tier under budget | 174, 175 |
| `de348c8` / `82f626a` | 1-2: distance in the job, bed, hangout and Hotel picks (I1) | 171 |
| `7fd7a7e` / `60dd9ba` / `61e854b` | 3a-3c: company housing, seeded then in-building with buyouts (I6); housing in mood | 177 |
| `f9dfd8a` | 4: commute pressure (I2), legs spared at the plan timeout | 164 |
| `b68297e` / `0a693b2` / `d1b64ee` | 5-7: vehicles in motion, El trains, the van (I3) | 165, 166 |
| `b39229e` | 8: the dead trades' bugs (I4) | 168 |
| `298807d` | 9: a Coarse timed walk's body on its path | 181 |
| `d42a234` | 10: the Statistical violence mirror limited by its attackers | 185 |
| `a9271e1` | the gate: Shadow V8 (`docs/SHADOW_V8.md`, critiques in `docs/shadow_v8/`) | — |
| `449ea95` / `afcd34b` / `7351eb8` | F3 stay-in fallback (I7), F1 routes on tiles (I9), F2 company beds to staff, Hideout gate (I8) | 190, 188, 189, 193 |
| `a08d383` | the shadow tool prints `cut at X` for an interrupted leg | 200 |

`OPEN-163` (the island picks without distance) closed with the pass.

## The pass in five lines

1. Island labour legs of 4 h or more: 20.3 % → 4.4 % (V8); structural commutes 29.6 % → 8.7 % (company housing).
2. Murders' runaway was the Statistical mirror amplifying itself (phase 10). Island hunger's scare was two measures
   disagreeing (`OPEN-167`, I10: no clinic path).
3. F3: island zero-leisure diaries 5 → 0 of 36, Murders 45.0 → 40.7. F1: exec rides 2.2x, exec El legs −22 %.
   F2: non-staff in company beds 14/10/10 → 8/2/0.
4. What is left of the long trips is distance, not routing: exec homes ignore the Hq (`OPEN-182`) and far
   non-urgent goals (`OPEN-194`).
5. The behaviour tier asserts only collapse bounds; calibration prints as FINDING (doctrine 1, this pass).

## In flight

- No agents are running, and the only worktree is main.
- Locked empty folders under `.claude/worktrees/` (`agent-*`, `island-f3`): a process still holds them; delete
  them after a restart. The `*.log` files there are old tier logs and can go too.
- TheCityGraphics: master `855d0aa`, **merged locally, not pushed** (P9 closed: the D4 regen of 3,196 lots, 6 of 10
  review findings fixed, deviations 4-5 in its DECISIONS). No camera path crosses D4 yet, so the 45-floor towers
  are untested as occluders. Its HANDOFF lists its next steps (El lines on rail `OPEN-158`, building character).

## Next, in order

Set with Dylan 2026-10-24 (`docs/ROADMAP_POST_M14.md` addendum 21): emergent story before city structure.

1. **The perf pass**, spec written: `docs/PERF_PASS.md` (decisions R1-R6, phases 0-8, a checkpoint, a quiet-box
   gate). Phase 0 housekeeping (`OPEN-201`, `OPEN-202`), phase 1 the Rest loop (`OPEN-195`), phases 2-5 the
   study's items 1-5 (byte-identical), phase 6 the 30-day slope and 7 midnight (R7, after the checkpoint), 8-9
   parallel scoring (thread-count identity), phase 10 the P9
   re-import at `k = 2` and `k = 1` (`OPEN-182`, reported, not fixed: `OPEN-204`). Gate: 100k ≥ 30 ticks/s, no tick
   over 1 s, a build under 2 min.
2. **Shadow V9** at 100k on the re-imported island: street sleeping (`OPEN-192`), the hunger climb (`OPEN-167`),
   a new-edges counter first (`OPEN-203`).
3. **The pressure pass** (`OPEN-178` mood drives behaviour, `OPEN-176` the dead trades, `OPEN-169`, `OPEN-170`,
   `OPEN-194`, the co-worker crew `OPEN-205`). Its spec states, for every mechanism, what the camera sees.
4. **The vertical city pass** (`OPEN-187`): many tenancies per building, a Hideout per district (`OPEN-191`).
5. The view milestone (Bevy client, follow-cam, `OPEN-149`'s ring), then M18 against the 3D client.

## Open calls for Dylan

- Push TheCityGraphics `855d0aa` (merged locally) before the perf pass's phase 10 re-imports from it.
- `OPEN-150`: the starting-condition seed's shape.
- Pressure-pass design: `OPEN-160`, `OPEN-162`, `OPEN-157`, `OPEN-176`, `OPEN-178`; the combat model `OPEN-179`.
- Standing: `OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`.

## The report (`test.py report --since 595ff23`, 2026-10-24)

Every cap met: god 20 of 20, asserted bullets 60 of 60, 59 ignores each with a reason, 105 test files; lint green.
On a08d383 with the box quiet: change **4m36s of 4m (breached, `OPEN-201`)**, merge 5m48s of 8m, milestone
15m56s of 20m.

## Cost (nineteenth session, API-rate $ equivalent, `tools/session_cost.py --session 844f4590`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| phase 0 / 1 (+ Sonnet fix 110) / 2 | Opus | 210 / 308 / 316 | 153-218k | 15 / 19 (+5) / 21 |
| phase 3a / 3b / 3c | Opus | **355** / 183 / 229 | 142-247k | 25 / 10 / 9 |
| phase 4 / 5 / 6 (+ Sonnet fix 28) | Opus | 310 / 286 / 272 | 224-261k | 18 / 24 / 17 (+1) |
| phase 7 (+ Sonnet fix 41) / 8 / 9 / 10 | Opus | 220 / **369** / 216 / 246 | 164-224k | 17 (+2) / 30 / 16 / 22 |
| F1 / F2 / F3 | Opus | 281 / 276 / 207 | 168-210k | 28 / 34 / 32 |
| Shadow V8 runs / seven critics + synthesis | Opus / Sonnet | 302 / 35-118 each | 204k / 94-128k | 35 / 24 |
| population study, murders and hunger probes, tier restructure, stat regen fix | Opus, Sonnet | 52-201 | 69-122k | 22 / 5 / 2 / 2 / 4 |
| graphics: bridge / look / T-D (+ review) | Opus | 229 / 118 / **430** (+113) | 88-187k | 14 / 5 / 114 (+9) |
| orchestrator | Opus | 692 | 162k | 77 |

Total ≈ $658 for 13 phases, 3 fix phases, a gate and three graphics stages. Every island phase stayed near the
300-turn brief (3a and 8 ran 355-369), against 395-507 last pass. The cost sinks: graphics T-D ($114, 20.5M cache
writes over a 4-hour regen) and the F rounds' ~8 h stalls on background jobs that never woke them: briefs now say
run long jobs in the foreground.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers, budgets and caps in `docs/TESTING.md`.
