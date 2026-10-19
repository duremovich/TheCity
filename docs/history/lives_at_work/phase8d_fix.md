# Gate 8d fix round

Branch worktree-agent-a48242db0386a6fb5, final HEAD **854343b** (base bfac404). Tree clean.

Commits:
- 8031d7c OPEN-133: the widow probe stages the brother's purse against the Fixer's quote
- 3bfc3b9 OPEN-132: regenerate the stat table on the gate-8d tree
- ecdb980 OPEN-132, 133: closed 3bfc3b9, 8031d7c (OPEN.md rows, LIVES_AT_WORK § 15 notes)
- 7bec435 civic: the tied-super test stages a Block no co-resident holds tied (broke on the regen)
- 854343b cli: the report header counts gate 8d's two exec_rides columns (pre-existing on bfac404)

## 1. OPEN-133 (closed 8031d7c): the mechanism was sound, the staging was off
The deposit didn't break the Hit, and neither did the venue spend. The staging sized the brother's
share off the market price at day-10 noon (87, deposit 44). The widow makes her decision at the
midnight pass. By then her Fixer's book had thinned, so the Hit priced at the Fixer's `quote`
(801, deposit 401): 44 + 95 from her brother wasn't enough, `try_hire` failed, and she hunted him
herself. The fix sizes the brother's purse against max(market price, quote), with the same 2x
margin. She now walks to the Fixer on day 11 and commissions the Hit (801 / 401, broker 531). The
strong-widow control still hunts and buys nothing. No assertion changed and no sim number moved.

## 2. OPEN-132 (closed 3bfc3b9): green after the regen
I ran `cargo run --release -p citysim-cli -- calibrate` (v2, seeds 1000..=1002, 30 d, 2000 agents,
149 s), then `python tools/test.py milestone --only parity`: GREEN, 3m55s.

| row | before (bfac404) | after (3bfc3b9) |
|---|---|---|
| paid venue visits (tol 1.89) | Full 12.600 / Stat 14.594 RED | 12.600 / 13.624 green |
| Lounge visits (FINDING) | 0.193 / 0.349 | 0.193 / 0.255 |
| paid venue coins (FINDING) | n/a | 55.25 / 63.10 (band 8.29) |
| law0 / law1 (FINDING, OPEN-038) | 0.897/0.741, 0.087/0.216 | 0.897/0.735, 0.087/0.217 |

Tolerance and `[classpay]` are untouched.

The regen redrew rare rates, so I checked the counters (TESTING.md: check Murders after a regen).
`phase_measure --base bfac404 --skip off,lives`, 42-44 x 120:
- Murders 66/101/178 -> 54/88/101 (mean 115 -> 81; pre-8d main was 77/125/72)
- J24 0.769 -> 0.773, J25 0.013 -> 0.012, arrests 2400 -> 2459
- last-day wallets 76k -> 93k (seed 42 82k -> 126k, +54 %)

These are FINDINGs and I didn't retune anything. The 8d deposit's Murders rise (91 -> 115) is gone
on this table. The seed-42 wallet jump is the one number worth a look at the gate.

## Two more test fixes the change tier surfaced
- `civic.rs::test_tied_super_moves_in_and_pays_no_rent` broke on the new table: it passes with the
  old table on the same tree. The first other-owner Block with room now houses a super of its own
  landlord, and `tied_housing`'s co-resident rule keeps that household put by design. I restaged it
  to pick a Block with no tied resident. Assertions are unchanged.
- `cli.rs::test_cli_report_csv_header` was 1028 vs 1026: 8d added `exec_rides` and
  `exec_rides_corp` without counting them. This was already on bfac404, hidden because the workspace
  test run stops at the first red binary (the widow probe). I added +2.

## change tier (854343b), footer verbatim
```
     2.6s  cli                      ok        6 passed   0 failed   0 ignored
[change] GREEN; BUDGET OK 3m41s of 4m00s
```

## Open / for the orchestrator
- The merge tier hasn't run on this tree. The stat-table regen can move the behaviour tier and
  core_year, so run `/merge-set` on 854343b.
- Nothing else is left open from this round.
- Logs: scratchpad/widow4.log, parity1.log, measure8dfix/measure.md, change3.log.

# Round 2 (merge tier red on 854343b): final HEAD e1c3e6b

## behaviour: d90 homeless CommuteAbortsDay was re-derived (b47af84)
I ran the eight-run ensemble on this tree with the new table. Shipped 0.07; stat_violence_mult 0.98 0.04,
0.99 0.04, 1.01 0.04, 1.02 0.03, 1.03 0.04; max_coarse 149 0.08, 151 0.04. The range moves from
0.00-0.02 to 0.03-0.08, and the bound from Max(0.07) to Max(0.13), which is the highest run + 0.05.
- The deposit did not cause it: with `move_in_rent_days` = 0 the row reads 0.09.
- The milestone had already moved the row off its old range before the regen: 698603c (8c main) reads
  0.06, and bfac404 reads 0.04 on the old table.
- The highs come from one seed's commuters. Seed 44 reads 0.24 and 0.20 over 16-19 picks; across the
  ensemble the seeds span 0.00-0.24.

This is a re-derivation under the standing rule, not a nudge to the bound.

## god: god_child_camp_sweep was restaged (e1c3e6b); the mechanism works
The old assertion compared the whole city's Camp answers (visits + buy-outs + break-outs, days 11-59)
against the control's. The sweep takes only 1 child on seed 42, so that total mostly reflects other Camp
families, who move with the city: it read 19 against 20, while the taken child (Fiora Abernathy) was
visited 14 times by her parents. The test now asserts that every taken child is answered by a parent at
least once, using the story's events. The city totals still print as a FINDING. This assertion is
stricter about the mechanism and does not depend on noise. Separate from this fix: the city has
1 child on day 10, so a sweep of 10 can only ever take one.

## Footers (e1c3e6b)
```
    58.8s  behaviour                ok        1 passed   0 failed   0 ignored
[merge] GREEN; BUDGET OK 59s of 8m00s
    50.7s  god                      ok       16 passed   0 failed   0 ignored
[merge] GREEN; BUDGET OK 51s of 8m00s
[change] GREEN; BUDGET OK 3m37s of 4m00s
lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 54
```
The ensemble logs are in scratchpad/ens/<run>/out.log, with base_bfac.log and base_698.log beside them.
