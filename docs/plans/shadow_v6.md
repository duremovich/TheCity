# Shadow V6: the pre-view read (brief, draft 2026-10-19)

V5 read the Lives at work gate tree (`e566e29`) as lives. V6 reads the closed tree (`a25fdc5`) with a second question
on top: **what does a camera following one person see?** It is the assessment before the view milestone and M18, so it
is judged at the size of the game (20k), from the day a player would start (day 0), and for continuity as well as for
lives. No sim behaviour changes in V6; every fix it finds is a later row with a failing test first.

## The three lenses

1. **Lives** (V4/V5's lens, V5's seven critic groups). Does the trade read as the trade? Re-read every V5 bug row
   (`SHADOW_V5.md` § Bugs, `OPEN-100..134`) as fixed, unchanged, worse or not observable.
2. **The camera.** Could each minute of the diary be drawn? Where the body is (tile, building, street, held,
   Statistical), whether it jumps (a position change faster than walking or riding allows), how often it starts and
   drops plans (the run reads 59k `PlanAborted` in 120 days at 2k), the longest stretch of nothing, and every place
   where the diary narrates something the body was not present for (V5 bugs 6 and 7: settlement at midnight, the pimp
   and the sleeping worker).
3. **The first week.** A player starts on day 0. Seed 42 at 2k books 390 marriages, 941 housings and 64 foundings in
   week 1 against ~15, ~150 and ~3 a week later on: the city boots by working through a backlog. What does a person
   live through in days 0-7, and which archetypes do not exist yet?

## Phase 0: the tool (one agent, Opus 5.5, ~150 turns; code in `citysim-cli/src/shadow.rs` only)

- **Labels** (V5 bug 20): `served` carries kind and direction (customer, tenant, co-worker, recipient, client, pimp);
  the courier's parcel line; the Camp headcount is not "a regular"; the school fee is not `Contract`; the Meeting is
  its own time-use row. V5 could not judge the cab, the parcel or the Camp from the text.
- **The camera block** at the top of each diary and as `camera.tsv` per window: per day, hours by place (Home, work,
  other building, street, vehicle, held, Statistical or Coarse), plans started and aborted (by `FailReason`), jumps
  (tile delta over the walk/ride allowance for the elapsed ticks, with the stamp), the longest unchanged stretch, and
  settlements booked while the pick is asleep or elsewhere (a `Collected`/`Served`/flow line whose counterpart is not
  co-located).
- **`--map FILE`**, passed through as `run` does, so a window can run on the committed island
  (`assets/maps/isle_m_k4.txt`).
- Guard: shadow draws no RNG and the sim hook stays `World::shadow_notes`; a 30-day `run` on seed 42 is byte-identical
  to main (CSV and save hash). Tests by name during the phase; `python tools/test.py change` once at the end. Commit on
  the branch; the orchestrator merges.

## Phase 1: the windows (same agent; logs to disk, summaries only)

`A` is V5's archetype list. Count 2 unless noted. Diaries under `target/shadow_v6/<window>/`.

| window | city | seed | days | why |
| --- | --- | --- | --- | --- |
| `k20_s42_d0` | 20k (`--population 20000`) | 42 | 0-7 | the first week; `--list` first, every missing archetype is a finding |
| `k20_s43_d30` | 20k | 43 | 30-37 | the settled city |
| `k20_s44_d90` | 20k | 44 | 90-97 | the mature city (~11 min of warm-up at ~200 tps) |
| `k2_s42_d26` | 2k | 42 | 26-33 | the control, count 1: separates scale artefacts from life artefacts |
| `k20_s43_d30_nopin` | 20k, `--no-pin` | 43 | 30-33 | 12 mixed picks: what an unpinned body on screen does (Statistical stretches, snapshots) |
| `isle_s42` (probe) | island, 20k | 42 | 0-30 run, then `--list` at d30 | does the real map hold 20k? employment, homeless, starvation, tps. Report only, capped at 30 min; a collapse is a finding, not a fix |

Then `shadow --lives --population 20000` against V5's last table (`LIVES_AT_WORK.md` § 13), and `shadow --assert`
(it must stay 133/133; a FAIL is reported, not fixed).

The agent writes its report to `docs/plans/shadow_v6_runs.md` (≤ 100 lines: commands, wall times, the candidate
counts per window, the camera.tsv roll-up per archetype, the island probe's numbers) and names the diary directories.

## Phase 2: the critics (seven agents, Sonnet 5.5, in parallel)

V5's groups: street, gangs, crime, corp, labour, vice, care (`docs/shadow_v5/<group>.md` lists the archetypes). Each
critic gets its group's diaries from every window, its V5 critique, the V5 bug rows touching its group and the
camera.tsv rows of its picks. Output `docs/shadow_v6/<group>.md`, ≤ 250 lines:

- per archetype: alive as the trade / thin / not alive, with one diary excerpt (path and stamp);
- each V5 bug in the group: fixed / unchanged / worse / not observable, with evidence;
- the camera: the three worst continuity moments in the group (stamp, what a viewer would see);
- the first week: what the d0 picks live through that the d30 picks do not;
- 20k against the 2k control: what differs because of size.

Readers cite diaries, never infer from absent lines (V4's method note: a Coarse stretch is not evidence).

## Phase 3: the synthesis (the orchestrator)

`docs/SHADOW_V6.md`, V5's shape (verdict, devices, bugs ranked, V5's bugs as read), plus two sections:

- **Before the camera**: the bugs ranked by what a followed character sees first (visible on screen × lives
  distorted), the cut-off for a pre-M18 fix pass.
- **What the view must handle**: what an on-screen Statistical or Coarse agent is, the in-building state, the
  jump rate, the first-week boot. These become requirements for the view milestone's spec, not sim fixes.

OPEN.md rows added or updated; HANDOFF's Next rewritten.

## Cost

V5's reading cost ≈ $8 for six Sonnet readers plus a $37 fix round. Expected: phase 0+1 ≈ $30-40 (Opus, ~250 turns
with the waits), critics ≈ $10-15, synthesis in the orchestrator. `python tools/session_cost.py` at the end.
