---
paths:
  - "citysim/tests/**"
  - "citysim-cli/tests/**"
  - "citysim-cli/src/shadow.rs"
---

# Writing and changing tests

The tiers, budgets and caps are in `docs/TESTING.md`; `python tools/test.py lint` enforces what is mechanical.

- **Asserts catch broken, FINDINGs report numbers.** Never add a band. If a number matters, print it as a `FINDING` line and read it with `tools/analyze_run.py`.
- **Every `#[ignore]` carries a reason string**: `#[ignore = "tier: merge"]`, `"tier: milestone"`, `"tier: diag"` (prints only, never a gate) or `"gap: OPEN-nnn"` (a probe that exposes a known gap; the row must be open). A `gap:` older than two milestones is fixed or deleted at the next close.
- **A mechanism gets one unit test in a seeded world**, calling the system directly with the preconditions set by hand. It becomes a `core_sanity` bullet only if it fires on ≥ 3 of 6 seeds (42-47 x 120 days), and then it replaces a bullet or names the milestone that justifies a new row (cap 60 asserted).
- **A phase's god scenario is a spec-time device.** It runs at the phase's acceptance; at the phase close it either replaces the scenario of its reaction class or is deleted with its write-up in `docs/history/`. Cap 20 scenarios plus controls, one per class listed in TESTING.md.
- **Name a test file by the system it tests, never by the pass that wrote it** (`factions.rs`, not `shadow_v4.rs` or `m17_review.rs`). Fold a fix's tests into the system's file.
- **Use `citysim/tests/common`** for the world builders (`world`, `dole_world`, `world_with`, `run_days`, `adults`, `free_adults`, `civilians`, `coins`, `set_coins`, `corp_named`; the coin identity is `citysim::systems::econ::identity`) instead of a local copy; add to it when a helper is needed twice.
- **Person probes** read the outcome from the agent's own state (its step, wallet, building), never from a city-wide count. A probe that exposes a gap is `gap: OPEN-nnn`, not a fix in the test change.
- **The behaviour tier** (`shadow.rs` `COLLAPSE` + `CHECKS`) asserts only collapse bounds (dead mechanism) at pooled n >= 10; every other row prints as a `FINDING` with its calibration range. Re-calibrating the printed ranges is never needed to merge (TESTING.md).
- **A test file over 1,500 lines is a lint warning**: split it by system or reaction class the next time you touch it (`god.rs`, `person.rs`, `hitmarket.rs`, `contracts.rs`, `wages.rs` are over today).
- Test code is in the per-change budget: a new test that needs a 120-day city goes in a shared run (a collector over seeds 42-44), not its own.
