# Lives at work, phase 8a: the gate's numbers

Phase 8 (`phase8_brief.md`, the gate) is split in three (2026-10-19, the token rules: one agent per ~300 turns): **8a** the numbers, **8b** the tests, **8c** the read and the close. Each is a fresh agent; each cites `phase8_brief.md`'s sections instead of repeating them.

Rules carried from phases 4-7: grep every anchor on your tree; never ignore, delete or loosen an existing test or behaviour row to get green (fix a test's setup, or report a row with main's reading and the cause); a headline number that moves gets a one-line cause with evidence or a fresh-seed variance check (the off-screen fviolence storms swing Murders ±40 % by seed: phases 4a, 5, 6, 7); long runs to files under target/, read summaries; `test.py change` once at the end. Report (≤ 100 lines) at the path the orchestrator names, written as you go. One squashed commit on your branch; never merge to main.

## Do

1. **The one regen** exactly as `phase8_brief.md` § The one regen: the 42-47 × 120 before set, `calibrate`, the table diff key by key, the after set, the Murders check against the seed spread, keep or revert with the reason (Q8). Use `python tools/phase_measure.py --base <briefed sha> --no-build --seeds 42,43,44,45,46,47 --skip off,lives --cols <the milestone's counters> --events Murder` for a before/after pair if it helps (it reports base -> candidate per seed). Then the parity test once on the final tree (`tests/lod.rs::test_full_vs_statistical_within_15pct`, milestone tier), every row pasted; a RED row that is a known mirror gap (the phases' § 14 blocks name them: hunger-days, paid venue visits, coins until the regen) is demoted with its reason (doctrine 2), a new RED row diagnosed (30 min cap) and reported.
2. **The deferred full sets** (`phase8_brief.md`'s paragraph): off identity on 42-44 × 30 for the master `CITYSIM_LIVES_OFF` and every sub-switch the milestone added (`grep -o 'CITYSIM_[A-Z_]*_OFF' citysim/src/config.rs`; a sub-switch spanning sub-phases uses `--off-base` with the pre-phase sha from `git log --merges`); alternating A/B ticks/s main-before-the-milestone (`23a161d`) vs your tree, seed 42 × 30, three rounds each, one process at a time; save size 30 d seed 42 both sides.
3. **The 20k close**: `core_20k` and the 20k seed 42 × 30 run on vs `23a161d` (Murders, arrests, deaths by violence, Station bookings, execs by tier); the law probe's 20k bands.

## Hand back

The regen decision with the Murders table (per seed, before/after, the spread), the table diff, the parity rows, the off-identity table (switch × seed), the A/B and save size, the 20k rows, the footers (`test.py change`, `test.py milestone`, `lint`). § 15 "Gate" in `docs/LIVES_AT_WORK.md` started with these (8b and 8c add to it).
