# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`; the seventeenth
session's close page is its last entry. Last rewrite: **2026-10-22, the eighteenth session: Track B, the pre-view
fix pass (phases 0-6) and its gate, Shadow V7.**

## Main

Main holds **the pre-view fix pass, closed as done on the generated city** (`docs/PREVIEW_PASS.md` § 5,
`SAVE_VERSION` 51). It also holds **Track B**: the island holds 20k (`assets/maps/isle_m_k2.txt`, `4fb10b9`).
The gate's tree, 45aef8c, ran change, merge and milestone green.

| merge | phase | OPEN closed |
|---|---|---|
| `4fb10b9` | Track B: the island at k = 2 for 20,000 | — (`OPEN-136` stays: move `core_20k`/`god_20k` and cut the 2k fixture from the island) |
| `f14b460` | 0: the switches retired, the idle freeze, the implant loop | 094, 138, 142 |
| `b0245eb` | 1: housing churn | 140 (part) |
| `d186412` | 2: the shift-shape machinery and `stations` | 137 (with 3) |
| `cf9556f` | 3: `haul` and `counter` | 121, 141, 145 |
| `24392c0` | 4: settlement by presence | 119, 120, 154 |
| `129f17f` | 5: long legs, the island El, the cab | 115, 139, 140, 156 |
| `0e09dee` | 6: leaders walk, hold court, give orders in person (P13-P16) | 118, 128 |
| `45aef8c` | the gate: Shadow V7 plus the parity fix (the Statistical table regenerated) | — |

## Shadow V7 in five lines (`docs/SHADOW_V7.md`, critiques in `docs/shadow_v7/`)

1. On gen and at 2k the pass worked:
   the shift reads as the trade for most roles (longest work action 530 → 89 min median); the idle freeze, housing
   churn and the implant loop are fixed; 1 % settled apart, from 8 %.
2. **The island picks destinations without distance** (`OPEN-163`). A quarter of commute legs run 4 h or more,
   a third of labour days are mostly travel, and by d90 2,188 people are hunger-sick (`OPEN-167`).
3. **Timed rides park the body, then jump it** (`OPEN-165`); so do arrests and transfers (`OPEN-166`).
   A segment can end in the wrong district (`OPEN-164`).
4. Row 6's dead trades are still dead on the island: courier, bouncer, joytoy, scav, arms dealer, foremen
   (`OPEN-168`); the cab is `OPEN-149`.
5. All eight pass switches off reproduces `4fb10b9` exactly. `shadow --assert` passes 132 of 132, and
   ticks/s on the island are 260 → 255.

## In flight

- No agents are running, and the only worktree is main.
- The V7 diaries are kept in main's `target/shadow_v7/` (314 MB, untracked) for V8 to compare against.
- Three empty dirs are left under `.claude/worktrees/`: `agent-a48242db…`, `agent-a75bba52…` and
  `agent-ab0f1e7a…`. A process still holds them, so `rmdir` them after a restart.

TheCityGraphics (a separate repo, read-only from here) has its own handoff and questions for Dylan. The authored El
lines need track there (`OPEN-158`).

## Next, in order

1. **The island pass** (`docs/ISLAND_PASS.md`, a draft). Dylan's decisions so far:
   I1, distance counts in every pick (a cost term plus a hard cap per purpose); I2, a far job resolves by moving
   nearer, then by a nearer job; I3, a vehicle in motion is an entity with its own position, and riders are aboard it. The El runs as trains,
     with the van and the flyer. Chases, explosions and carjackings come later, built on this model.
   Still to spec with Dylan: the dead trades (`OPEN-168`) and the phase split. Phase 0 retires the pre-view
   switches and `stat_table_base.toml` (`OPEN-174`) and pays back the change budget (`OPEN-175`). The gate is Shadow V8 on the island against
   `target/shadow_v7/`.
2. Then `OPEN-136`'s remainder: the 2k fixture cut from the island, with `core_20k`/`god_20k` on it.
3. Then the view milestone: the Bevy client, a follow-cam, `OPEN-149`'s ring, and the requirements in `SHADOW_V6`
   § What the view must handle.
4. Then M18, rewritten against the 3D client.

## Open calls for Dylan

- The island pass: the dead trades and the phase split (`OPEN-168`, `ISLAND_PASS` § Scope).
- `OPEN-150`: the starting-condition seed's shape.
- `OPEN-160` (owed protection piles up), `OPEN-162` (no protection at 20k), `OPEN-157` (Fixer parties at midnight):
  pressure-pass design.
- Standing: `OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`.

## The report (`test.py report --since 4fb10b9`, 2026-10-22)

Every cap met: god 20 of 20, asserted bullets 60 of 60, 58 ignores each with a reason, 105 test files (new: housing,
leaders, legs, presence, shift).

The change tier is **over budget**: 4m53s against 4m on main at the close, 3m50s-4m33s through the pass. The island
pass's phase 0 pays it back (`OPEN-175`). The merge tier ran 4m54s of 8m, and the milestone tier 13m53s of 20m on the gate tree.

## Cost (eighteenth session, API-rate $ equivalent, `tools/session_cost.py --session 201a627d`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| Track B | Opus | 209 | 139k | 9 |
| phase 0 (+ the arms fix round, Sonnet) | Opus | 309 (+36) | 191k | 17 |
| phase 1 | Opus | 316 | 211k | 20 |
| phase 2 | Opus | **507** | 295k | 50 |
| phase 3 | Opus | 446 | 242k | 33 |
| phase 4 (+ the camp fix round, Sonnet) | Opus | 420 (+18) | 283k | 32 |
| phase 5 (+ fix rounds: Opus 159, Sonnet 112) | Opus | 395 | 283k | 46 |
| phase 6 (+ the staff-pay fix round, Sonnet) | Opus | 327 (+85) | 241k | 33 |
| V7 runs | Opus | 256 | 190k | 25 |
| V7 critics, seven in parallel | Sonnet | 86-147 each | 137-162k | 36 |
| the parity fix | Sonnet | 123 | 101k | 4 |
| orchestrator | Opus | 399 | 130k | 36 |

Total ≈ $342. Phases 2-5 blew the 300-turn brief. Their contexts sat at 240-295k, and phase 2 ran 507 turns.
The next pass should split its phases smaller, at about one mechanism each.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers, budgets and caps in `docs/TESTING.md`.
