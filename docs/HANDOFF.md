# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite: **2026-10-18, the end of the fourteenth session (the process review).**

## Main

`git log --oneline -30` is the session. Main holds:

- **The process review, done** (`docs/PROCESS_REVIEW.md` § 6 has each task's commit and check): `CLAUDE.md` + `.claude/rules/{tests,config}.md`; `tools/test.py` (`change|merge|milestone|diag|lint|report`) and a short `docs/TESTING.md`; this Now page with the old HANDOFF under `docs/history/SESSIONS.md` and 20 superseded docs under `docs/history/`; `docs/README.md`; `docs/OPEN.md` (99 rows); `docs/ARCHITECTURE.md`; memory 47 → 12 files; the SessionStart briefing hook, the fmt pre-commit (`git config core.hooksPath tools/hooks`), `/close-milestone`, `/merge-set`; the test retirement pass: every `#[ignore]` reasoned, `core_sanity` at 60 asserted bullets, the god set 41 → 20, the five pass-named test files folded by system, `wounded` 32 s → 3 s, `tests/common`.
- **The Shadow V4 sweep** (`2712b4a`, review `6184981`): 21 of 23 bugs fixed; Murders 806 → 381 on 42-44. `docs/SHADOW_V4.md` § The sweep.
- **Jobs & lives** closed (`986ac8b`, SAVE_VERSION 34). Earlier milestones: `docs/README.md`.

**Both tiers green on main at the close** (`test.py change` 4m11s against 4 min; `test.py merge` 4m29s against 8 min: year 111 s, behaviour 62 s, god 61 + 18 + 17 s; the milestone tier was not run this session). Lint: 0 problems, god 20/20, bullets 60/60, 46 ignores (25 merge, 2 milestone, 18 diag, 1 gap).

## test.py report (2026-10-18, `--since 2712b4a`)

| tier | wall | budget | |
|---|---|---|---|
| change | 4m11s | 4m00s | breached by 11 s: `core` 40 s, `wages` 16 s, `weapons` 10 s next (`OPEN-093`) |
| merge | 4m29s | 8m00s | ok (the first run read 9m10s because the runner re-entered cargo per step and rebuilt citysim twice; fixed the same day: the ignored tiers now exec the built binaries) |
| milestone | not run | 20m00s | run at the Lives at work close |

god 20/20 · asserted bullets 60/60 (40 + 3 + 15 + 2) · 93 test files, 55,580 lines · `fn world()` copies outside `tests/common`: 30 (fold them as files are touched) · since `2712b4a`: 47 test files changed, +2,683 −3,502.

## In flight

**Lives at work** (`docs/LIVES_AT_WORK.md`), fifteenth session, 2026-10-18. Phase 0 merged (`c8d71b5`): the law probe answers § 5.1 (the arrest share falls with the walk to the Precinct at both scales, so phase 7 builds Stations) and `shadow --lives` holds the baselines (§ 13). Phase 1 merged (`593e8cf`, SAVE_VERSION 35): the service encounter under `[lives] enabled` / `[service] venues`, both tiers green on the exact tree, eleven deviations in § 14; for the gate: the venue day shifts against an evening trade (63 % of arrivals find nobody on duty), the madam's cut rounding to 0, no bettor at any bout, the parity row `encounters/shift` printed (Full 0.707 vs Statistical 0.877). Phase 2 merged (`596cd8c`, SAVE_VERSION 36): the home spot, the friend's place, the empty-spot leave, reunite, the newborn's carer, the household business, `GoalKind::Visit`, the `Employed`/`LaidOff`/`Born` deeds under `[household]`; one fix round before the merge (the own Home offered only with company in or due: `household::company_due`), after which empty HangOuts read 0.27 → 0.24 and the spouse named 1.87 → 3.55 h a week. **For the gate (phase 8), from phase 2:** whether a Statistical spouse counts as home by the clock (today the home evening moves only for couples with two moving bodies, ~5 % of households; the phase 1 mirror credits a Statistical server by the clock); the parity rows hunger-days, paid venue visits and coins red until the one regen (D14); homeless +5 % / evictions +11 % as a late-window drift (rent, wages, employment flat; hotel nights the first mover); seven test pins to `household.enabled = false` on a changed city (traced, each a pick change). **Phase 3 (the civic counterparts and the round) is being built** by the same solo coder `phase0-coder` in `.claude/worktrees/agent-aeb9f5f4cf9b9e4ee` (branch `worktree-agent-aeb9f5f4cf9b9e4ee`); its brief is the orchestrator's scratchpad `phase3_brief.md`, its report `phase3_report.md`. The orchestrator runs `/merge-set` on the exact tree, merges, then sends the phase 4 brief to the same coder.

## Next, in order

1. **`/code-review high`** on this session's range (`e9bac83^..HEAD`) if it did not run at the close; fix findings on main.
2. **Lives at work** (`docs/LIVES_AT_WORK.md`): phase 0 on main. One solo coder per the SOP; `/merge-set` before each merge; the first subagent of the session confirms `CLAUDE.md` auto-loaded (PROCESS_REVIEW § 6, task 1); `/close-milestone` at the end, which pays back the change tier's 11 s or opens a row for it.
3. **M18**, the player character (`docs/M18_PLAYER.md`).

## Open calls for Dylan

Rows of kind `decision` in `docs/OPEN.md`: the harsh city and a pressure pass (`OPEN-062`), gang wars cooling (`OPEN-039`), faith equilibrium (`OPEN-057`), synth parity (`OPEN-059`), execs are cheap (`OPEN-077`), Ambient rent and the 2k flats (`OPEN-069`, `OPEN-070`). New this session, not decisions but worth a glance: `OPEN-094` (19 `CITYSIM_*_OFF` switches to retire), `OPEN-095` (the V4 after-diaries went with the `v4fix` worktree; regenerable), `OPEN-096..099` (four retired god reactions with no unit test).

## Standing decisions

`CLAUDE.md` § Standing decisions, each dated. The doctrine and the agent rules are there; `docs/TESTING.md` has the tiers, budgets and caps.
