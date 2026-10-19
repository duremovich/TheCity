# phase 8a rows re-derived (branch HEAD 9ad01ab, rows at cbf8c67)

Eight runs on 30444de: shipped, stat_violence_mult 0.98/0.99/1.01/1.02/1.03, max_coarse 149/151.

| row | shipped | 0.98 | 0.99 | 1.01 | 1.02 | 1.03 | mc149 | mc151 | min x 0.6 | new bound | new range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| d19 fighter known/wk | 2.55 | 2.95 | 2.60 | 3.05 | 3.05 | 3.00 | 3.00 | 2.70 | 1.53 | Min 1.5 (was 2.8) | 2.55-3.05 |
| d90 homeless known/wk | 3.73 | 3.74 | 4.48 | 4.84 | 4.16 | 2.91 | 4.13 | 5.06 | 1.75 | Min 1.7 (was 3.9) | 2.91-5.06 |
| d19 homeless known/wk (unchanged) | 4.47 | 4.53 | 4.47 | 5.41 | 5.41 | 5.41 | 6.67 | 4.00 | | 3.0 holds, min 4.00 | |

Other rows failing in a butterfly run (not touched; shipped run has none besides the two above):
- stat_violence_mult 1.03: d90 homeless CommuteAbortsDay 0.08 > 0.07.
- max_coarse 149: d90 purist CommuteAbortsDay 0.08 > 0.07.

OPEN: OPEN-103 closed cbf8c67; OPEN-109 feature (street life, pressure pass); OPEN-110 bug (pimp pays his own pitch, unowned). Both named in LIVES_AT_WORK.md § 15.
Checks: behaviour test passes (58 s); test.py lint 0 problems; no CRLF.
Commits: cbf8c67 (rows), 9ad01ab (OPEN + § 15). Logs: target/bf/*.log in the worktree.
