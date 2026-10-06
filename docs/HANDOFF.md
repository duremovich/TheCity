# Handoff — 2026-10-06 orchestrator run

Written at the close of the second orchestrator session (the mandate in `~/.claude/projects/D--TheCity/memory/thecity-next-session-mandate.md`). Everything below is committed and pushed; the tree is clean. The next session picks up at **M14 phase 1** (it may already be in flight: check `git log` and `git worktree list` first).

## Where the code stands

| Milestone | State | Key commits |
| --- | --- | --- |
| M9 The law, M10 Scale, M11 Ownership | done, reviewed, pushed | see the 2026-10-05 handoff in git history (`git show 2a73480:docs/HANDOFF.md`) |
| M12 Districts | done, **reviewed**, pushed | phases 8f94860 … 0ac8069, fix pass 4b8fe41, review fixes **f81bb23** (16 of 17 findings) |
| M13 Assets | **done, reviewed, pushed** | 2038070 asset model; 34172ce + 233e559 vehicles; 539d496 + 4446f91 chrome; 8b95259 stims and the robot; 3bac96a levers, 0b4af75 gate/god v4/calibration/docs/throughput, e7c49f2 UI; merge 4107b06; review fixes **6c04ece** (13 of 13) |
| M14 Virt … M18 Player | specs and plans written, nothing implemented | specs `docs/M14_VIRT.md` … `M18_PLAYER.md`; plans `~/.claude/plans/m14-virt.md` … `m18-player.md` |
| Design additions | recorded, not planned | `docs/VISION.md` sections and `docs/ROADMAP_POST_M14.md` addenda 1-12 (12 = world state and cataclysms, 2026-10-06) |

Measured at HEAD, seed 42, 2,000 residents, 120 days: ~10.5k ticks/s mean (daily min ~8.0k; gate runs 10.1-10.3k), save 22.8 MB, 169 vehicles (153 motorcycles), 318 chromed agents (17.8 % of adults, 51 % of gang members), episodes 8, hooked 68 (3.6 %), dealing 28 % of gang income, crash deaths 4, repossessions 20, vehicle thefts 58 (7 chopped), truck haul share 0.73, assaults+murders 23.7/day (cap 42.7), Murders 177, child starvation 0. All gates pass: `scenario` (7 gates + 2 probes, **serialized**), `lod` 3/3 (parity and kitted parity), `scale`, `determinism`, `save`, `god` 15, `god_corps` 12, `god_districts` 11.

## The first things to do next session

1. **M14 phase 1** from `~/.claude/plans/m14-virt.md` (lines 272-538; shared sections 1-271; risks and toolchain 678-end; the two addenda at the very end: firewalls/bridges/hop chains from `docs/M14_VIRT.md`'s addendum, and the `alertness_mult` hook). Its gate is byte identity with `--virt-off`. Then phases 2-5, then M15 … M18 in order.
2. **The pattern that works** (unchanged, refined): one coder per phase (Opus for phases with calibration, determinism or planner risk; Sonnet for well-specced ones and UI), each in its own worktree (`isolation: worktree`), a read-only Sonnet spec review of the phase's commit, a fix round by the same coder when the verdict is "gaps", the orchestrator commits on the branch with the deviations and numbers, merges into main, re-runs the trio and every gate on main, pushes. Per milestone: `/code-review high <prev review fixes>..HEAD --max-findings 20`, an Opus fix coder on the main tree, god suites re-run, handoff and memory updated.
3. **Run the scenario suite serialized**: `cargo test --release -p citysim --test scenario -- --ignored --test-threads=1` (~4.5 min). The M12 gate runs seeds 43-47 in threads and the harness runs the gates in parallel, so a plain run measures ~7.7k ticks/s and fails the 8k floor for no reason.

## How the gates judge now (changed this session)

Seed-42-alone trajectory checks flipped with every behaviour change (eight-seed means moved less than the seed spread), so:

- **M12 gate** (`test_m12_districts_seed_42`): seeds 42-47, 42 alone first for ticks/s, 43-47 in threads; riots by the six-seed mean in 1..=4; gang control ≥ 14 d and the Sanitation reallocation on ≥ half the seeds; gang landlords met by pooled windows ≥ 1/2 with ≥ 4 judged windows (a window that runs past day 120 is unjudged); a Split on any seed. Mechanism checks (ticks/s, litter and Dregs bands, Crackdown share, raids) stay on seed 42. The data behind it (dealer diversion refuted; Garrison is M12's own regime) is in the test comment.
- **v1 acceptance**: the starvation-death bullet by majority over seeds 7-9.
- **M11 gate**: "Squeeze held" by majority over seeds 42-44.
- **M13 gate** (`test_m13_assets_seed_42`): coin-flip bullets (crash deaths 1..=10, a stolen vehicle later chopped, episodes 1..=8, an episode ended by the law, an NPC-founded Clinic or Garage) by majority over seeds 42-44; the rest on seed 42.

## Known residuals (behaviour worth fixing, not gate failures)

- **It is a dole city.** ~250 of 2,000 adults are employed; the median wallet falls from 21 to 2-5 coins by day 60. Most M13 calibration cut prices to fit (motorcycle 300 → 60 with upkeep 0 and wear 0, i.e. a permanent free asset; implant T1 150 → 60; car 500, truck 900, flyer 2000). Revisit prices when employment and wages are addressed; the goods/economy milestone is the place.
- **The Jail is pinned at its 80-bed cap from about day 28 in every run since at least M12 phase 5** (diagnosed 2026-10-06, bisect over f81bb23 … 6c04ece: no M13 commit moved it; the old "~41" figure was gang members in the Jail, not occupancy). Uncapped, the law would sentence ~35 a day and the Jail would pass 200 beds. Beds: Assault ~46 (half gang), Shakedown ~16 (all gang), Murder ~10, Dealing ~5. Once full, 25-45 arrests a day end in "Precinct full" releases, and the day-45 violence step is those releases: 58 % of attackers in days 46-60 had been freed that way (83 % of attackers and ~98 % of victims are gang members). Jail capacity is the only knob that moves violence (160 beds: −28 %, 255: −22 %); shorter sentences or parole just cycle the same members faster. **Left as a design decision for Dylan** (a chronically full jail is a dystopian dynamic, and private prisons are a planned mechanic); `Building.capacity` is `u8`. The law garrisons 50-90 days a run because `jailed_gang` reads ~40-60 gang members in those beds; a dealer-share cap was tried and gave nothing.
- **A food-theft and starvation wave around days 80-105** (~300 off-screen food thefts and ~285 Starving events on the peak day, mean hunger 0.66 → 0.54) is in every run since M12 and unrelated to the Jail; the Winter season multiplier is the likely driver. Deserves its own look at the economy milestone.
- Chrome finance is thin at `implant_down_frac = 0.75` (bricks 3-9 a run); 0.5 breaks the repossession and chrome-share bands. Treat's need term (`edgy − sanity`) does not fit `psycho` 0.8: bodies at risk of episodes barely seek Therapy (god ChromeEveryone: episodes 5-9/day, Therapy flat).
- God v4 gaps (`docs/GOD_SCENARIOS_V4.md`): FloodStims drew no rival raid for the stock; stims-legal did not cut gang income nor push gangs to Harvest/Raid; Chase pins act only on Full-detail trips; `god_split_gang` passes for the wrong reason (the split command is refused "no squat or Lot in Sump East"; gangs split on their own once `max_gangs` is lifted).
- Vehicle thefts 29-63 on seeds 42-44, up to ~130 elsewhere: a recovered vehicle can be stolen again. Dealing starts only around day 30-40 (short of buyers, not stock). Robots are repaired at the nearest Security Office (phase 5) but still wear at 1/day.
- Planner: ~70 gang plans a day hit the 200-expansion A* cap because of the cheap pre-M13 actions (a GoTo to every key, Wander, Rest, Beg); the dealer and extortion plans are hand-built in `plan.rs` to dodge it. Next planner costs if speed is needed: Arrest (~7 ms/day), GetHigh. Slowest days ~7.3-8.0k ticks/s.
- Older residuals still open: riots go where the grievance points, not the unrest; a dead gang under high coverage never re-forms; incorporated NPC corps go bankrupt in 30-40 days; night guards sleep through unpaid shifts; hotel occupancy ~20-30 %; the allocation's `paid` term is not gated on inhabited districts; Role::Sanitation rides the TendGraves/Cemetery special cases (a `SweepWork` action keyed to the beat district is the right mechanism; M12 review finding, skipped).
- Two abandoned worktrees from the 2026-10-05 session remain under `.claude/worktrees/` (agent-a8fb53c…, agent-ac09976…: dirty M10/M11 phase 3 experiments whose work shipped by other commits). Dylan's call to delete.

## Where things live

- **Specs**: `docs/SPEC.md` (v1, with an Assets section), `docs/M8_FACTIONS.md` … `docs/M18_PLAYER.md`. Each from M10 on has a decisions table at the top and, for the implemented ones, an "Implemented: deviations" section at the end (M13's lists every D1-D52 row, every knob added, and the review fix pass). M14, M16 and M18 end with Dylan's addenda.
- **Vision and roadmap**: `docs/VISION.md`, `docs/ROADMAP_POST_M14.md` (M15-M18 sketches plus twelve addenda).
- **Plans**: `~/.claude/plans/m10-scale.md` … `m18-player.md`. The M14 plan says to table the addendum's decisions as V64+ when implementing.
- **Tools**: `tools/analyze_run.py run.csv [events.tsv]` (now with an Assets section and corp slot 9) and `tools/compare_runs.py a.csv b.csv`; `citysim-cli run --days N --seed S --report --events > run.csv 2> events.tsv`; `calibrate --agents 500 --map assets/map.txt` (reproduces `assets/stat_table.toml` byte for byte at HEAD); levers via `--lever day=N:key=value` including the M13 ones (`stims_legal`, `asset_tax=car:0.5`, `impound`, god `grant_asset`, `wreck`, `chrome_everyone`, `flood_stims`, `brick`, `chase`). App: overlay `K` heats districts by hooked adults, `--hooked` starts with it on, the asset panel opens from any asset link.
- **God suites**: `tests/god.rs` (15), `tests/god_corps.rs` (12), `tests/god_districts.rs` (11); `docs/GOD_SCENARIOS_V1..V4.md` with gaps lists as the backlog.
- **Memory**: `~/.claude/projects/D--TheCity/memory/` (project state, the mandate, one file per design addition incl. `thecity-world-state-cataclysms.md`, the god-scenario doctrine, the learned-policy verdict).

## Working notes for the next orchestrator

- Toolchain: prefix cargo with `export PATH="$HOME/.cargo/bin:$PATH"` in bash; the trio is `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --release --no-fail-fast`; the long gates one at a time (scenario with `--test-threads=1`); keep files LF; `git add -A -- . ':!.claude'` is the safe staging form.
- The harness may move a coder (and the orchestrator's cwd) into a worktree mid-task; a coder told "main tree" will keep editing main, one told nothing will follow the harness. Say which in the brief, and check `git worktree list` before committing. Worktrees created by the Agent tool start at main's HEAD: to base one on a branch commit, tell the coder to `git merge --ff-only <sha>` first.
- Coders must not commit; the orchestrator commits so the message carries the deviations and numbers. A coder resumes when messaged after its report; tell it to stop explicitly before handing its files to another agent (one M12 fix coder kept editing after its report and its follow-up landed in a merge commit unnoticed).
- When a gate flips on a behaviour change, test the mechanism hypothesis first (the phase 2 "driving guards weaken the law" and phase 5 "dealers divert gang labour" scares both turned out to be trajectory noise, but the robot-seller monopoly and the LOD rank leak were real), then make the check multi-seed; never tune a knob to pass one seed.
- Briefs for M14 and M16 must say up front that runs, ICE, hits and leverage are dice contests between fictional agents in a simulation (a planning attempt was once stopped by a safety classifier on the vocabulary alone).
- Verification is data first: the CSV, the event log, the gates, parity, the throughput floor (8k; margin target 8.8k in the M13 gate), then screenshots. "Make improvements as you see fit" covers tuning and residuals; the roadmap order is not to be changed.
