# tools

Stdlib-only Python 3 helpers for headless runs.

```
cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv
python tools/analyze_run.py run.csv events.tsv [--days N] [--json] [--jail-cap K]
python tools/compare_runs.py a.csv b.csv
```

- `analyze_run.py`: population, economy, law, mood, throughput, event counts, weekly histograms
  for story events, order/posture transitions, and a list of anomaly flags. CSV columns are read by
  header name, so new columns do not break it. M11 adds an ownership section (evictions, rent,
  registered foundings and corp builds, incorporations, hostile acquisitions, bankruptcies, strikes,
  monopoly days, unrest per class, the ledger per day), each corp slot's treasury and days per order,
  CorpOrder transitions per corp, and flags for a corp in the red 3+ days running, a monopoly,
  Street unrest outside 0.2-0.7 and no Dregs for 30+ days running.
- `compare_runs.py`: per-column means of two reports with the relative difference, for A/B
  comparisons across seeds or commits.
- `train_stat_policy.py` (numpy; the learned-policy experiment, `docs/EXPERIMENT_LEARNED_STAT_POLICY.md`):
  fits the 24-row table, a coarse extended table and 2-layer MLPs on `calibrate --rows-csv` agent-hours,
  prints losses, calibration and held-out regimes, and exports `stat_mlp.toml`.
