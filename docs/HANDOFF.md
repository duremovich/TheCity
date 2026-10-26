# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`; the eighteenth
session's close page is its last entry. Last rewrite: **2026-10-26, the twenty-first session: perf pass phases 6-9
merged (the twentieth session merged 0-5, ran the checkpoint and wrote R7, and ended without a rewrite).**

## Main

Main holds **the perf pass, phases 0-9 of 10** (`docs/PERF_PASS.md`, deviations per phase under § Implemented;
`SAVE_VERSION` 67, unchanged by the pass). Phases 2-9 are byte-identical to their bases on 2k 42-44 x 30 and 20k
`isle_m_k2` 42 x 30 (save, report, events); phases 8-9 also across 1, 2 and 8 threads. Every merge ran the change
and merge tiers green on the exact tree; the last, 915574f: change 1m14s of 4m, merge 4m55s of 8m.

| merge | phase | OPEN |
|---|---|---|
| `aecc8a6` | 0: the island switches retired, the change tier six binaries at a time | 201 closed, 202 |
| `acacea1` | 1: the Rest loop (`[rest]`, `CITYSIM_REST_OFF`, retires at the close) | 195 closed, 207-208 opened |
| `2ced00a` / `218ca03` / `4f021de` / `e5048d7` | 2-5: the venue index, the world build, the price-keyed caches, the claim index and guard grid | 184 |
| `11ff4d4` / `7ad2ec7` | the checkpoint (found the 30-day slope) and R7 (the slope and midnight before parallel scoring) | — |
| `7359e73` | 6: the slope: the kin index, the recruit's roster walk, `edges`/`edges_new`/`coworker_pairs` columns | 203 closed, 209-211 opened |
| `560ad51` | 7: midnight per pass: faith draws first, the district grid memo, rehouse's rent once, `run --prof-midnight` | 184 |
| `25a8673` | 8: parallel think: `think::score`/`apply` with serial barriers and re-scoring, `[perf] threads` (1), `run --threads` | 184 |
| `915574f` | 9: the Statistical hour stays serial (its rolls read what earlier agents wrote); the co-work bond in two exact steps | 184 |

## The pass so far, in five lines (100k seed 42, `isle_100000`, 8 threads where the branch has them)

1. Days 2-4: 11 -> 37 (checkpoint) -> 51 ticks/s. Days 25-30: 18 -> **25.5** against the gate's 30. Build 370 -> 47 s (gate 2 min, met).
2. Midnight, the longest tick every day: 7.5-8.1 s (checkpoint, days 25-30) -> 3.9 (phase 6) -> **2.6 s** (phase 7) against the gate's 1 s. What is left is forty passes under 0.3 s each.
3. The slope is growth by design: the acquaintance graph (0.6M -> 8.7M edges by day 30, still +8k/day at day 60 at 20k, `OPEN-210`) and the co-work bond's crew square (10 s of a 76 s game day, `OPEN-209`). Phase 6 indexed the kin walks; phase 7 made faith draw before it reads.
4. Parallel scoring is bounded: think is a third of the tick and its plain split was not order-free (an abort's releases are read by later scorers), so phase 8 re-scores after barriers; the Statistical hour cannot split at all without a behaviour change (phase 9). Memo locks cost a quarter of a think's CPU at 8 threads: a lock-free memo read is the next byte-identical lever.
5. 20k seed 42 days 25-30: 246 -> 284 ticks/s (8 threads); the 20k and 100k end cities are unchanged by the pass (homeless 2,903, employed 56,184 at 100k).

## In flight

- No agents are running; the only worktree is main. Empty locked folders under `.claude/worktrees/` (`agent-*`,
  `island-f3`) and the `p*_change.log` / `p*_merge.log` tier logs there can go after a restart.
- TheCityGraphics: master `855d0aa`, **merged locally, not pushed**; phase 10 re-imports from it.

## Next, in order

1. **Dylan's four calls** (below), then the phases they open: **7b** the midnight stagger across the night's first
   ticks, switched, off-identical (measured ~0.35 s longest tick against 2.6); the **crew cap** `[lod] coworker_crew`
   32 as a switched behaviour phase with its stat table regenerated and the parity test (`OPEN-205`/`209`); the
   **gang dwell** fix as a switched Sonnet round (`OPEN-211`).
2. **Phase 10**, the P9 re-import (`OPEN-182`, `OPEN-204`): after the graphics push; the 20k tier map and the 100k
   map at `k = 2` and `k = 1` side by side (R5); the 20k FINDINGs re-baselined for a written reason.
3. **The gate** (`PERF_PASS` § The gate) on a quiet box against `6d6f7bc`, with `[perf] threads` set (8 is the knee
   at 20k; 100k not measured past 8). If 30 ticks/s misses: lock-free memo reads (byte-identical, a small phase),
   then `OPEN-206` as its own pass; the target is not lowered. The Rest switch retires at the close.
4. Then as before: Shadow V9 at 100k on the re-imported island (`OPEN-192`, `OPEN-167`, `OPEN-203` closed), the
   pressure pass (`OPEN-178`, `OPEN-176`, `OPEN-169`, `OPEN-170`, `OPEN-194`, `OPEN-205`, `OPEN-209`, `OPEN-210`), the
   vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

## Open calls for Dylan

- **Phase 7b, the midnight stagger** (recommended yes): byte-identical work is exhausted at 2.6 s.
- **The crew cap into this pass** (`OPEN-209`, recommended yes): the largest single term at 100k; R4 deferred it on a
  7 % estimate that 100k has now measured at ~13 %.
- **The gang dwell rescore** (`OPEN-211`): a bug, not byte-identical (`order_trace`, `rep_flips`); a switched Sonnet round or the pressure pass.
- **The acquaintance graph** (`OPEN-210`): not levelled at day 60; an earlier prune is a knob retune (doctrine 2), a
  per-person cap a mechanism. Recommended: the pressure pass.
- Push TheCityGraphics `855d0aa` before phase 10.
- `OPEN-150`: the starting-condition seed's shape. Pressure-pass design: `OPEN-160`, `OPEN-162`, `OPEN-157`,
  `OPEN-176`, `OPEN-178`; the combat model `OPEN-179`. Standing: `OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`,
  `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Cost (API-rate $ equivalent, `tools/session_cost.py`)

Twentieth session (`--session 61af1740`, Opus orchestrator): phases 0-5 at 173 / 251 / 152 / 179 / 243 / 247 turns,
$8 / 15 / 8 / 15 / 16 / 20; phase 6 **330** turns, $32; the orchestrator 199 turns, $23. Total ≈ $137.

Twenty-first session (`--session 25a0721b`, Fable orchestrator):

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| phase 7 | Opus | **332** | 216k | 39 |
| phase 8 (+ a Sonnet read-only order audit it spawned) | Opus (+ Sonnet) | 286 (+172) | 203k | 28 (+8) |
| phase 9 | Opus | 190 | 173k | 16 |
| orchestrator (merge tiers, three briefs, the handoff) | Fable | 91 | 123k | 20 |

Total ≈ $111 for three phases. Phases 8 and 9 each stalled once on a background sim run and needed a nudge; the
briefs now say foreground polling, and the next brief should say it in the first line, not the proof section.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers, budgets and caps in `docs/TESTING.md`.
