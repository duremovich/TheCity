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
  optional: `analyze_run.py run.csv` skips the event-based parts.
- `compare_runs.py`: per-column means of two reports with the relative difference, for A/B
  comparisons across seeds or commits.
- `train_stat_policy.py` (numpy; the learned-policy experiment, `docs/EXPERIMENT_LEARNED_STAT_POLICY.md`):
  fits the 24-row table, a coarse extended table and 2-layer MLPs on `calibrate --rows-csv` agent-hours,
  prints losses, calibration and held-out regimes, and exports `stat_mlp.toml`.
