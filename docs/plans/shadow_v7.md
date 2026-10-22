# Shadow V7: the pre-view pass gate (brief, 2026-10-22)

The gate of the pre-view fix pass (`docs/PREVIEW_PASS.md` § 3). It reads main after phase 6 (`62589b3`).
The base is the pre-pass main, `4fb10b9` (Lives at work closed, plus Shadow V6's tool and Track B's island).
The question: does each row of `SHADOW_V6.md` § Before the camera (rows 1-7) now read as fixed, or as demoted
with a reason? It uses V6's tool, windows, lenses and seven critic groups, so V6 and V7 compare like for like.
The one change from V6: the 20k windows run on the island (`assets/maps/isle_m_k2.txt`, Track B), because the game
city is the island now that it holds 20k. The V6 generated-city window comes back as a control. No sim behaviour
changes during V7; a gate failure becomes a fix round or an OPEN row.

## Phase 1: the windows and the measurement set (one Opus agent)

The 20k island flags are those of `docs/IMPORT_GRAPHICS.md` and Track B:
`--population 20000 --scaled --map assets/maps/isle_m_k2.txt --ambient-flats 50`.

| window | city | seed | days | why |
| --- | --- | --- | --- | --- |
| `isle_s42_d0` | island 20k | 42 | 0-7 | the first week |
| `isle_s43_d30` | island 20k | 43 | 30-37 | the settled city |
| `isle_s44_d90` | island 20k | 44 | 90-97 | the mature city |
| `gen20k_s43_d30` | `gen:20000` (V6's map) | 43 | 30-37 | like for like with V6's `k20_s43_d30` |
| `k2_s42_d26` | 2k, count 1 | 42 | 26-33 | the control |
| `isle_s43_d30_nopin` | island 20k, `--no-pin` | 43 | 30-33 | V6's mixed picks |

The archetype list is V6's (V5's 48), plus `gang_leader` and `sweeper` at count 4 for phases 6 and 4.

Then:
- `shadow --lives --population 20000` on the island and on gen, against V6's table.
- `shadow --assert`.
- The camera roll-up per archetype against V6's `camera.tsv`. It covers the longest work action per shift, distinct work
  tiles per shift, jumps, the longest still stretch, settlements apart/asleep/stat, `PlanAborted` per day, and GoTo
  legs over 120 min.
- **The measurement set:**
  - the 2k counters on seeds 42-47 × 120 against `4fb10b9`;
  - the island 20k counters on seeds 42-44 × 30 against `4fb10b9`;
  - the composed off identity: every pass switch at once (`CITYSIM_IDLE_FIX_OFF`, `IMPLANT_FIX`, `HOUSING_FIX`,
    `STATIONS`, `HAUL_COUNTER`, `PRESENCE`, `LEGS`, `LEADERS`) against `4fb10b9` on 42-44 × 30;
  - alternating A/B ticks/s at 20k (print only);
  - `python tools/test.py milestone`.
- The pass's FINDINGs, each read on the island at 20k:
  - murders by seed;
  - wages and employment;
  - parcels;
  - served_tenant and `stat_home_p`;
  - contract encounters (OPEN-157);
  - Eat→Earn aborts;
  - protection and gang income (OPEN-160 and OPEN-162);
  - orders by word (OPEN-161);
  - body cab drivers;
  - the sweeper's ride vs walk.

Report: `docs/plans/shadow_v7_runs.md`, ≤ 120 lines, in V6's runs-report shape.

## Phase 2: the critics (seven Sonnet agents in parallel)

The groups are V6's: street, gangs, crime, corp, labour, vice, care. Each critic gets:
- its group's diaries;
- its V6 critique, `docs/shadow_v6/<group>.md`;
- the V6 Before-the-camera rows touching its group;
- its camera rows.

Output: `docs/shadow_v7/<group>.md`, ≤ 250 lines:
- per archetype, alive / thin / not alive, with an excerpt;
- each V6 row in the group, fixed / unchanged / worse / not observable, with evidence;
- the three worst camera moments;
- island against gen.

Readers cite diaries and never infer from absent lines.

## Phase 3: the synthesis (the orchestrator)

`docs/SHADOW_V7.md`: the verdict, rows 1-7 as read, the new bugs ranked, and the FINDINGs. Then the close
(`/close-milestone`) if the rows read as fixed or demoted.
