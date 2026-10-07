# Handoff — 2026-10-07 orchestrator run (close of the third session)

Written at the close of the third orchestrator session (the mandate in `~/.claude/projects/D--TheCity/memory/thecity-next-session-mandate.md`). **M14 Virt is done: phases 1-5 merged, the milestone review applied (`49d2689`), every gate green.** `main` is pushed. The next session starts M15.

## Where the code stands

| Milestone | State | Key commits |
| --- | --- | --- |
| M9 … M13 | done, reviewed, pushed | see `git show 8315b4b:docs/HANDOFF.md` |
| M14 Virt phases 1-2 | merged | 95f0349, c0327c8 |
| M14 phase 3 (gang/corp VirtRaid, Raid prelude, DoorOpen, ICE in Secure/Hunker, cameras, FactionDb, Stat pass) | merged | 5cf0768 + fix round 0e0722f, merge a2d84dd |
| M14 phase 4 (N overlay, Node/Run/Mission panels, Corp Tech block, MissionView, levers, god commands) | merged | 40fe398, merge 36f4715 |
| M14 phase 5 (M14 gate, god scenarios V5, calibration, Research fix, docs) | merged | 6d13984, merge 16b9efe |
| **M14 review fixes** (15 findings + the grudge wipe) | merged | 82e2d91, merge 49d2689 |
| M15 … M18 | specs and plans written | `docs/M15_WORD_AND_BLOOD.md` … `M18_PLAYER.md`; plans `~/.claude/plans/m15-word-and-blood.md` … `m18-player.md` |

Session decisions by Dylan (2026-10-06): **Jail 160 beds** (was 80; ec526b6), **one ticks/s floor at 4,000** for every gate while systems are being built (`TPS_FLOOR`, a179c07; numbers always printed; judge regressions by alternating best-of-5 A/B of two binaries), **stale worktrees cleared**, **check in on subagents every ~10 minutes** (memory `feedback-check-in-on-agents.md`).

Measured at `main`, seed 42, 120 days (box loaded by renders; idle reads higher): ~8.4k ticks/s, bench `tick_2000_agents` median 136 µs, save ~26.8 MB; M14 seed 42: nodes 41, Labs 4, Data made ~4.2k, runs 70 (34 % ok), fried 8, flatlined 2, traced 13, chair arrests 5, Murders 168, assaults/day ~15-19. Jail 160: mean ~129 occupied, no longer pinned.

## First things to do next session

1. **M15 Word and Blood** from `~/.claude/plans/m15-word-and-blood.md` (phases: Knowledge, Stats and moves, Blood, News/UI/levers, Gate and polish). Read the plan's dependencies table against what M14 shipped (`docs/M14_VIRT.md` "Implemented: deviations" and "M14 review fixes"). **The planner key word**: M14 uses 39 of 40 `WorldState::pack` bits; M15 needs a wider key (flagged in the M14 plan).
2. Optional before M15 (M14 god follow-up): add "After M14" notes to `docs/GOD_SCENARIOS_V1..V4.md` from a `--nocapture` god run; V5 is current.
3. Then M16 … M18 per the plans, with the same pattern.

## M14 open items carried forward (all in `docs/M14_VIRT.md` Open and `docs/GOD_SCENARIOS_V5.md` gaps)

- **Printed findings in the M14 gate, not asserts**: IceRaised six-seed mean 7.3 (spec ≥ 10: raises are paid above the fleet reserve and most corps sit under it in the dole city); the ICE-spend Spearman 0.51 pooled (0.76 per building node: it compares portfolios, not responsiveness; ICE-after-loss is the responsiveness assert and passes); DataWiped lands on 1 of 8 seeds (the gate asserts the grudge-ordered wipe run instead; gangs sell all Data at midnight, `gang_data_keep` 0, so targets are empty); a robot turned or blinded is rare (1 in 8 seeds; Blind deferred with V33).
- **Tech upkeep is at the floor** (`upkeep_coins` 0): every coin costs Zetatech about a day of life; TechLost comes from Data lapse. Research now works through a spec deviation (gap vs the street tier, lapse floor 0.4, 3-day build delay, 14-day cashflow gate, `tier_cost[2]` 200).
- **Deferred to M16**: V67 sweeps (no Bridge exists before M16), V33 freelance blind-first Camera order (a prepended JackIn never completes outside the Hack goal: needs exec/world-state work), bridges as dashed links.
- **Hunker never sheds ICE** at upkeep [0,0,1,2] (signed-off deviation; revisit when upkeep has teeth).
- **The city never raises its own ICE** (V5 gap 1); Ledger hits land almost only on the Treasury.
- **Gate device changes this session** (all with data in comments): M13 dealing share ≥ 0.15 by majority (Jail 160 moved the band), episodes-by-law any seed, vehicles and episodes by six-seed mean, hooked by majority; M12 Looted any seed of 42-47; M14 bands by majority of 42-44, means over 42-47, existence over 42-47/42-49.

## The pattern that works (refined this session)

One coder per phase in its own worktree (`isolation: worktree`; Opus for phases with calibration, determinism or planner risk, Sonnet for well-specced ones and UI), a read-only Sonnet spec review of the phase's commit, a fix round by the same coder on "gaps", the orchestrator commits on the branch with deviations and numbers, merges, re-runs the trio and every gate on main, pushes. Per milestone: `/code-review high`, an Opus fix coder on the main tree, god suites re-run, handoff and memory. Keep a running queue file of carry-overs per phase (this session's is folded into "M14 queue" below). Verification is data first.

- **Briefs for M14 (and M16) must open with the board-game framing** ("a second board of abstract nodes; one seeded dice roll per guarded node, as `law::resolve_fight`; no networking or real technique; genre words are identifier names"). The first phase 2 coder was stopped by a safety classifier mid-phase with the lighter framing; a second coder with the heavy framing finished it. Describe effects as timed flags on buildings and moved units, never as techniques.
- **Agents sometimes go idle without their hand-back reaching the orchestrator**; `ListAgents` shows them idle. Message them for the report; a message resumes them. Keep a 10-minute Monitor heartbeat while anything is in flight (Dylan, 2026-10-06), and resend a message that crossed a hand-back. Worktrees made by the Agent tool can start at the session-start HEAD: brief every coder to `git merge --ff-only main` and confirm the SHA first.
- **When a gate flips on a behaviour change, test the mechanism first, then make the check multi-seed; never tune a knob to pass one seed.** This session: "driving guards weaken the law", "dealers divert gang labour" and "decapitation gets no succession" were all noise or test bugs; the robot-seller monopoly, the LOD rank leak and the Data money pump were real.
- **Run the scenario suite serialized**: `cargo test --release -p citysim --test scenario -- --ignored --test-threads=1` (~7 min). The multi-seed gates spawn threads and the harness runs tests in parallel; a plain run competes for cores and its ticks/s readings are meaningless (the floor is 4,000 since 2026-10-06).
- Toolchain: `export PATH="$HOME/.cargo/bin:$PATH"` in bash; trio `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --release --no-fail-fast`; LF files; `git add -A -- . ':!.claude'`; worktrees made by the Agent tool start at main's HEAD (to base one on a branch: `git merge --ff-only <sha>` first); the harness may move a coder's cwd into a worktree mid-task (say "main tree" or not in the brief and check `git worktree list` before committing); coders never commit; tell a finished coder to stop before another touches its files.

## How the gates judge now

Seed-42-alone trajectory checks flipped with every behaviour change (eight-seed means move less than the seed spread), so: **M12 gate** over seeds 42-47 (42 alone for ticks/s; riots by the six-seed mean in **1..=5**; gang control ≥ 14 d and the Sanitation reallocation on ≥ half the seeds; gang landlords pooled windows met ≥ 1/2 with ≥ 4 judged windows, windows past day 120 unjudged; a Split on any seed; mechanism checks on seed 42). **v1**: the starvation-death bullet by majority over seeds 7-9. **M11**: "Squeeze held" by majority over 42-44. **M13**: coin-flip bullets (crash deaths 1..=10, Chopped after a theft, episodes 1..=8, an episode ended by the law, an NPC-founded Clinic/Garage, **vehicles ≥ 150 with every kind**) by majority over 42-44. The god scenarios `god_riot_sump_west` (natural riots suppressed so the control is a control) and `god_decapitate_gang` (asserts succession, a Split, or Retaliate/LieLow within 14 days) were made robust the same way.

## Known residuals (behaviour worth fixing, not gate failures)

- **It is a dole city.** ~250 of 2,000 adults are employed; the median wallet falls from 21 to 2-5 coins by day 60. M13 and M14 prices were cut to fit (motorcycle 60 with upkeep 0 and wear 0, implant T1 60, deck T1 60). Revisit prices when employment and wages are addressed (the goods/economy milestone).
- **The Jail is 160 beds since 2026-10-06** (Dylan): at 80 it was pinned from ~day 28 and full-Jail releases drove the day-45 violence step; at 160 it averages ~129 and assaults+murders fell ~25 %. The bump rule bumps the least severe prisoner. Private prisons remain a planned mechanic (`Building.capacity` is `u8`, so 255 is the ceiling).
- **A Winter food-theft and starvation wave around days 80-105** (~300 off-screen food thefts and ~285 Starving events on the peak day) is in every run since M12; the season multiplier is the likely driver; the economy milestone's business.
- Chrome finance is thin at `implant_down_frac` 0.75; Treat's need term does not fit `psycho` 0.8 (episodes 5-9/day under ChromeEveryone with Therapy flat); vehicle thefts 29-130 across seeds because a recovered vehicle can be stolen again; dealing starts day 30-40; robots wear at 1/day with repairs at the nearest Office; ~70 gang plans a day hit the 200-expansion A* cap from the cheap pre-M13 actions (the dealer and extortion plans are hand-built in `plan.rs`); slowest days ~7.3-8.0k ticks/s.
- God v4 gaps (`docs/GOD_SCENARIOS_V4.md`): FloodStims drew no rival raid; stims-legal did not cut gang income nor push gangs to Harvest/Raid; Chase pins act only on Full-detail trips; `god_split_gang` passes for the wrong reason. V1-V3 "After M13" notes: no v1-v3 gap closed by M13; bankrupt_city now empties the city (pop ~268 by day 100).
- Older: riots go where the grievance points; a dead gang under high coverage never re-forms; incorporated NPC corps die in 30-40 days; night guards sleep through unpaid shifts; hotel occupancy 20-30 %; the allocation's `paid` term is not gated on inhabited districts; Role::Sanitation rides TendGraves (a `SweepWork` action is the right mechanism).

## Where things live

- **Specs**: `docs/SPEC.md`, `docs/M8_FACTIONS.md` … `docs/M18_PLAYER.md` (decisions table at the top; "Implemented: deviations" at the end for M10-M13; M14's is phase 5's). M14, M16 and M18 end with Dylan's addenda.
- **Vision and roadmap**: `docs/VISION.md`, `docs/ROADMAP_POST_M14.md` (M15-M18 sketches plus fourteen addenda). The three 2026-10-06 additions: **world state and cataclysms** (a daily `WorldState` ledger; cataclysms as a TOML trigger + effect table composed from god commands; a chronicle; events built after M18; M14-M18 keep god commands composable and expose a per-faction tech tier), **ambient LOD** (a per-district-per-hour density ledger; promotion to meet it with real errands; `travel_mode`/`company` as agent-level functions shared with Full trips; company as a unit; a short-lived local disturbance field that empties streets near fights and riots and gives civilians a Shelter errand; lands with the player view; M14-M18 keep routines readable per district/hour and vehicle/escort choice out of inline plans), **life-path playtests** (model-driven agents play desired goals with rich kits and immortality-with-ledger or restore-on-death; measures completion, distinct approaches, opportunities, idle days, refused actions; `docs/LIFEPATHS_V1.md` as the gaps list; M15-M18 keep every player mechanic behind a `PlayerCommand` and label approaches).
- **Plans**: `~/.claude/plans/m10-scale.md` … `m18-player.md`. The M14 plan's Toolchain section names an older scratchpad path; use the session's.
- **Tools**: `tools/analyze_run.py run.csv [events.tsv]` (Assets section, corp slot 9, the M13 event kinds; needs an M14 section: `nodes`, `runs`, `runs_ok`, `data_*`, `ice_*`, `tech_*`, the violet events), `tools/compare_runs.py`; `citysim-cli run --days N --seed S --report --events > run.csv 2> events.tsv` (`--virt-off` for identity checks); `calibrate --agents 500 --map assets/map.txt` reproduces `assets/stat_table.toml`; levers via `--lever day=N:key=value` incl. M13's and M14's `wipe_data=`, `set_tech=`, `grant_data=`. App: overlay `K` (hooked adults), `--hooked`; M14's `N` overlay and panels are phase 4.
- **God suites**: `tests/god.rs` (15), `tests/god_corps.rs` (12), `tests/god_districts.rs` (11); `docs/GOD_SCENARIOS_V1..V4.md` with gaps lists and "After M13" notes.
- **Memory**: `~/.claude/projects/D--TheCity/memory/` (project state, the mandate, one file per design addition incl. `thecity-world-state-cataclysms.md`, `thecity-ambient-lod.md`, `thecity-lifepath-playtests.md`, the god-scenario doctrine, the learned-policy verdict).
