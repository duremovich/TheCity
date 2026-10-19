# tools

Stdlib-only Python 3 helpers for headless runs.

```
cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv
python tools/analyze_run.py run.csv [events.tsv] [--days N] [--json] [--jail-cap K]
python tools/compare_runs.py a.csv b.csv
```

- `analyze_run.py`: population, economy, law, mood, throughput, event counts, weekly histograms
  for story events, order/posture transitions, and a list of anomaly flags. CSV columns are read by
  header name, so new columns do not break it. M11 adds an ownership section (evictions, rent,
  registered foundings and corp builds, incorporations, hostile acquisitions, bankruptcies, strikes,
  monopoly days, unrest per class, the ledger per day), each corp slot's treasury and days per order,
  CorpOrder transitions per corp, and flags for a corp in the red 3+ days running, a monopoly,
  Street unrest outside 0.2-0.7 and no Dregs for 30+ days running. M12 adds a districts section
  (skipped on CSVs without the `d1..d8_*` columns): per-district mean coverage, unrest, litter,
  crime and guards and days per controller (Contested/City/Gang/Corp), with names from
  `assets/config.toml` `[districts] names`; totals for riots, crossfire, vagrancy and hotel nights;
  Dregs as a share of adults (`dregs` over the three `class_*` counts) and the days in the 1-5 %
  band; and from the events riots started/won/lost/fizzled/dispersed, strikes and splits per
  district and corp-building raids won/lost. Flags: a district above unrest 0.8 for 30+ days
  running with no riot there (per-district riot events, or the city `riots` column without an
  events file), a district's litter above 0.5 (with the days), `dregs == 0` for 30+ days running,
  and corp raids never lost. M14 adds a Virt section (skipped when the plane did not run): the
  plane's last-day snapshots (nodes, Labs, decks, cameras, mean corp ICE, Data held), run totals of
  runs and outcomes, Data made/stolen/wiped/sold, Ledger hacks, doors, turned robots, traces, fries,
  flatlines, arrests, ICE raised/lowered/spend and tiers gained/lost, the success share, traced share
  of lost runs and stolen / made, each corp slot's tiers first and last day, the M14 flows and, with
  events, the violet event counts and the tier, wipe, door, robot and flatline lines; flags for the
  spec section 12 bands. L2 adds an "L2" section (skipped when the jobs economy did not run): wages /
  dole on day 60 and per 30 days, the employed share, the Treasury against the budget band with the
  works roster and `upkeep_mult`, the leisure, gamble, tribute, export and public-works flows, fun,
  venues and visits by kind, HangOut contacts, fronts, the Parts chain, the faction-violence totals
  and the off-screen share of killings, kill rates by tier, the tiers with the held prisoners and the
  aborts by cause; flags for the spec's printed bands and `fv_bound_wrong`. The events file is
  optional: `analyze_run.py run.csv` skips the event-based parts. Jobs & lives (docs/JOBS_AND_LIVES.md § 12) adds
  a "Jobs & lives" section (skipped without the `[jobs2]` columns): the employed share from day 30 (J24) and the
  homeless share (J25), each new flow's daily mean (handling, its leak to the World, power, the plants' fuel,
  builds, the Rations book's pay-out), the Rations share of all export pay and the run's World inbound, units
  (food packed, Rations made and sold, pilfered, builds, outages), jobs by `TradeModel` and by new kind on day 60
  and the last day, the civic three, the Hqs and, with events, the outages, builds, pilfers and foreman lines.
  Phase 2 adds the street's lines: run totals (Pitches opened and closed, the gang's cut, sweeps, vendor meals and
  take, day posts offered and filled, the corner's queue, parcels, cab and rickshaw trips and fares, tinker repairs,
  plants seized), the Pitches open by kind, the take a Pitch-day, the cut's share of the take and the fill rate.
  Lives at work (docs/LIVES_AT_WORK.md § 10) adds a "Lives at work" section (skipped without the `served_visit`
  column): encounters by kind per staffed shift, the waited share, regulars made, the home and friend's shares of
  HangOuts, spouse evenings a week per 1,000 adults, empty spots left, visits by purpose, the Camp (buy-outs,
  break-outs), the civic, street and tower counters, retirements and frailty exits, the leaders' lieutenants and
  cut, the Stations and the execs' homes, arrests a day and ticks/s. Printed bands, never asserted; the bands with
  no column are `shadow --lives` (ages by trade, the foreman, leaders' jail-days, guards' travel) and
  `law_probe_20k` (arrests per witnessed crime by escort distance).
- `compare_runs.py`: per-column means of two reports with the relative difference, for A/B
  comparisons across seeds or commits.
- `train_stat_policy.py` (numpy; the learned-policy experiment, `docs/history/EXPERIMENT_LEARNED_STAT_POLICY.md`):
  fits the 24-row table, a coarse extended table and 2-layer MLPs on `calibrate --rows-csv` agent-hours,
  prints losses, calibration and held-out regimes, and exports `stat_mlp.toml`.
- `phase_measure.py`: the per-phase measurement set in one command (CLAUDE.md Token rules): the off identity (`--off CITYSIM_X_OFF`, seed 42 x 30: events, the report's shared columns and the save with its version and `ticks_per_sec` masked), the 42-44 x 120 counters base -> candidate (Murders, J24, J25, arrests, wallets, ticks/s printed only, every `--cols` summed and `--events` counted) and the `shadow --lives` rows for `--arch`. Main's binary is built from `git archive <base>` into `<main repo>/target/measure/base-<sha>/` and run from that root; it and every base run are cached by sha, so a rerun pays for the candidate only. Writes `measure.md` under `target/measure/cand-<sha>/`; exits 2 on an off-identity DIFFER. `--off-base SHA` checks the off identity against another base (a switch spanning sub-phases); `--base-off` runs the base with the same switches off (a fix to a switch that already exists on the base). Every call re-syncs the cached base source to `git archive`, runs `cargo build` and drops cached runs when the binary's hash changes. A wrong `--arch` or `--cols` is an error naming the known ones.
- `session_cost.py`: the token cost of a Claude Code session per agent from the transcripts under `~/.claude/projects/` (turns, median and p90 context, cache writes and reads, output, an API-rate dollar equivalent, and what the shell calls did: sim runs, test-tier runs, `cargo test` calls, log reads). `--last N`, `--session PREFIX`, `--dir PATH`. Run at every session close (CLAUDE.md Token rules).
