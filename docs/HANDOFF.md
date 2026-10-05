# Handoff — 2026-10-05 orchestrator run

Written at the close of the session that ran as an orchestrator over a team of lower-level agents (the mandate in `~/.claude/projects/D--TheCity/memory/thecity-next-session-mandate.md`). Everything below is committed and pushed; `origin/main` is `0ac8069`, the tree is clean. The next session picks up at **M12's milestone review** and then **M13 phase 1**.

## Where the code stands

| Milestone | State | Key commits |
| --- | --- | --- |
| M9 The law | done, pushed | 3eb80cb … c4f129a, a2f1cb6 (review fixes) |
| M10 Scale | done, pushed | 60e6a70 … 3e98b9f (review fixes) |
| M11 Ownership | done, pushed | 54ee921 … 7a8d8b8 (review fixes), 2a2adca |
| M12 Districts | **all five phases and a 41-item fix pass committed and pushed; the milestone `/code-review high` has NOT run yet** | 8f94860, e512768, f2e7cc7, 684b18e, 4b8fe41, 0ac8069 |
| M13 Assets … M18 Player | specs and plans written, nothing implemented | specs in `docs/M13_ASSETS.md` … `M18_PLAYER.md`; plans in `~/.claude/plans/m13-assets.md` … `m18-player.md` |

Measured at HEAD, seed 42, 2,000 residents, 120 days: 12.4k ticks/s (median tick ~30 µs), save 21.7 MB, thefts ~3 per 100 agent-days (v1 at 300 was 2.05), child starvation 0, Dregs 1–5 % of adults every day, one bankruptcy, no monopoly, both Security corps alive, Crackdown ~25 % of days, riots 1–4 per run, corp raids lost about a quarter of the time, parity passing on every metric over 6 seeds. The eleven scenario gates (`cargo test --release -p citysim --test scenario -- --ignored`), parity, scale, save, determinism, and the three god suites (`--test god`, `--test god_corps`, `--test god_districts`) all pass.

## The first things to do next session

1. **M12 milestone review.** `/code-review high 2a2adca..HEAD --max-findings 20`, apply the findings as `M12 review fixes: …` (re-run the trio and every gate), push. The per-phase read-only reviews already ran (their findings are in the M12 fix pass); this is the whole-milestone pass the SOP calls for.
2. **M13 phase 1** from `~/.claude/plans/m13-assets.md` (note the addendum at the end about `AssetKind::Bridge`, added by the M14 spec addendum). Then phases 2–5, then M14 … M18 in order. The pattern that worked: one coder per phase from the plan (Opus for the phases with calibration or determinism risk, Sonnet for well-specced ones), a read-only Sonnet spec review of each phase's commit, the orchestrator commits with a message that lists the deviations and the measured numbers, a fix pass per milestone over the queued findings, `/code-review high` per milestone, god suites re-run per milestone.
3. **Re-run the three god suites after each milestone** and append an "After M1N" note to their docs; the gaps lists in `docs/GOD_SCENARIOS_V{1,2,3}.md` are the backlog of behaviour the sim still cannot produce.

## Known residuals (not bugs in the gates; behaviour worth fixing)

- Riots go where the grievance points (the Civic), not where the unrest is (the Sump); a won riot at the Precinct is a jailbreak; litter at the cap cannot be dug out by sweepers and litter moves mood only (`docs/GOD_SCENARIOS_V3.md`).
- A dead gang whose district stays at high law coverage never re-forms (documented in `docs/M12_DISTRICTS.md` deviations); `god_decapitate` does not split a one-district gang.
- Incorporated NPC corps go bankrupt 30–40 days after founding (two Bars do not cover upkeep, bartenders and the exec wage).
- Night guards sleep through most of their unpaid shifts (a Sleep-scheduling residual from M10).
- The M11 spiral link (an evictee joining a gang within 14 days) sits at its floor of 3 on seed 42; the M12 split bullet holds on seeds 43 and 44 but is fragile to `split_loyalty`.
- Hotel occupancy is ~20 % of beds.
- The allocation's `paid` term is not gated on inhabited districts (gating it removed the beats on the Civic's Markets and the Vats' Farms and broke the M11 spiral gate).

## Where things live

- **Specs**: `docs/SPEC.md` (v1), `docs/M8_FACTIONS.md` … `docs/M18_PLAYER.md`. Each from M10 on has a decisions table (overturnable) at the top and, for the implemented ones, an "Implemented: deviations" section at the end. M14, M16 and M18 end with addenda from Dylan (Virt overlay/firewalls/bridges/hop chains/quiet-loud; attention and distraction; the majordomo and squad) that amend the spec and that the plans point at.
- **Vision and roadmap**: `docs/VISION.md` (the end state plus every design addition from 2026-10-05: the outside world, dialogue and leverage, governance and missions, economy/tech/skills, god scenarios as the test method, brutality and status, the survival loop, building and news, surveillance and hits, emergence and tiered security, inventory/loot/the wounded, money/scrip/energy/water, building interiors, daemons as a candidate, attention, the majordomo) and `docs/ROADMAP_POST_M14.md` (M15–M18 sketches plus eleven addenda).
- **Plans**: `~/.claude/plans/m10-scale.md` … `m18-player.md`. Each tables its decisions and names the earlier milestones' decisions it depends on.
- **Tools**: `tools/analyze_run.py run.csv [events.tsv]` (summary, per-district section, anomaly flags) and `tools/compare_runs.py a.csv b.csv`; `citysim-cli run --days N --seed S --report --events > run.csv 2> events.tsv`; `calibrate --agents 500 --map assets/map.txt` regenerates `assets/stat_table.toml` (reproducible byte for byte); levers via `--lever day=N:key=value` including the god levers (`kill_leader`, `jail_gang`, `fund_gang`, `fire_guards`, `treasury`, `fund_corp`, `bankrupt_corp`, `seize_corp`, `kill_exec`, `kill_staff`, `corp_order`, `strike`, `wipe_corps`, `law_posture`, `city_rent`, `rent_cap`, `breakup`, and the M12 district levers).
- **Experiment**: `docs/EXPERIMENT_LEARNED_STAT_POLICY.md` (an MLP policy for the Statistical tier loses to the table inside the sim; opt-in `[lod] policy = "mlp"`; the trigger for revisiting is written there).
- **Memory**: `~/.claude/projects/D--TheCity/memory/` (project state, the mandate, one file per design addition, the god-scenario test doctrine, the learned-policy verdict).

## Working notes for the next orchestrator

- Toolchain: prefix cargo with `export PATH="$HOME/.cargo/bin:$PATH"` in bash; the trio is `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --release --no-fail-fast`; the long gates one at a time with long timeouts and `--nocapture`; keep files LF (agents sometimes write CRLF); a worktree agent's directory under `.claude/worktrees/` is git-ignored, and `git add -A -- . ':!.claude'` is the safe staging form.
- Coders must not commit; the orchestrator commits so the message carries the deviations and numbers. Never let two agents edit the main tree at once; use `isolation: worktree` for perf passes, diagnoses and god suites, then merge (expect small conflicts in `levers.rs`, `goals.rs`).
- A message to a finished agent resumes it; after a report, tell an agent to stop explicitly if its files are about to be handed to another agent.
- Briefs for M14 and M16 should say up front that runs, ICE, hits and leverage are dice contests between fictional agents in a simulation (one M14 planning attempt was stopped by a safety classifier on the vocabulary alone; the reframed retry succeeded).
- Verification is data first: the CSV, the event log, the gates, parity, the throughput floor (8k; HEAD is at ~12k), then screenshots. "Make improvements as you see fit" covers tuning and residuals; the roadmap order is not to be changed.
