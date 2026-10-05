# tools

Stdlib-only Python 3 helpers for headless runs.

```
cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv
python tools/analyze_run.py run.csv events.tsv [--days N] [--json] [--jail-cap K]
python tools/compare_runs.py a.csv b.csv
```

- `analyze_run.py`: population, economy, law, mood, throughput, event counts, weekly histograms
  for story events, order/posture transitions, and a list of anomaly flags. CSV columns are read by
  header name, so new columns do not break it.
- `compare_runs.py`: per-column means of two reports with the relative difference, for A/B
  comparisons across seeds or commits.
