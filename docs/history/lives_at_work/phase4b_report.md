# Phase 4b report: the pimp, the dealer, the street trades' regulars

Final sha **3befd05** (branch worktree-agent-a00576221d4bcf094, one squashed commit on 4da38cb). SAVE_VERSION 39.
E, F and G are all built.

## Built
- **E, the pimp** (`vice.rs`): `pimp_present`. A body within `pimp_tiles` protects the pitch. A Statistical pimp is credited while `following_order` holds, which is a deviation. Either one marks `pimp_seen`.
  - His round: `gang_work_target` returns the worker on the pitch due this hour, and `plan_for` builds the scripted `pimp_plan` (GoTo(Spot), then an hour of Collect). The collection is a Staff encounter through the new `service::encounter_post`.
  - In pitch hours his GangWork is scored like a shift, and the day's earlier gang task no longer satisfies it.
  - The cut goes to the pimp (`StreetCut`), and `pass_up` sends `pimp_pass_up` of it on to the gang.
  - `name_pimps` drops a pimp absent for `pimp_absent_days` and prefers a member with no Job.
- **F, the dealer** (`stims.rs`): `deal_site` returns the gang's venue, otherwise its corner (`deal_corner`), otherwise `deal_bar`.
  - `corner_plan` and `corner_buy_plan` are scripted plans. `World.corner_dealers` and `corner_log` are new.
  - Bar and corner sales share one `sell()`, which runs the Buyer encounter (`"dealer"` row) and counts `dealer_regulars`.
  - A buyer goes to the dealer he knows first: at a Bar, on a corner, and in `stat_buy`.
  - A lookout multiplies the witness roll by `witness_lookout_mult` (`law::raise_crime_mult`).
  - A Statistical dealer's day logs his corner (`fviolence`).
- **G**: vendor (`stat_vendor_meal`) and tinker (`tinker_repair`) get encounters. The rickshaw was already done in 4a.

## Fix found by the measure
The first measure showed stims dealt falling from 1,087 to 2. Every 2k gang has no venue, so all of them deal at corners. `exec::start_step` re-checks the planner's symbols (`Deal` needs at(Seller)), and a corner is not a planner location, so every corner Deal failed to start. Fix: a Deal or BuyStims with no target checks only its own start condition.

## Measure (`tools/phase_measure.py`, rerun with `--skip off` after the fix, on the WIP commit 240a3f4; identical code to 3befd05 except tests/docs)
| counter | seed 42 | seed 43 | seed 44 | mean | |
|---|---|---|---|---|---|
| Murders | 30 -> 91 | 88 -> 80 | 126 -> 77 | 81 -> 83 | +2% |
| J24 employed | 0.738 -> 0.768 | 0.785 -> 0.781 | 0.731 -> 0.754 | 0.751 -> 0.768 | +2% |
| J25 homeless | 0.023 -> 0.021 | 0.017 -> 0.016 | 0.023 -> 0.018 | 0.021 -> 0.019 | -12% |
| arrests | 2,708 -> 2,488 | 2,472 -> 2,604 | 2,690 -> 2,451 | 2,623 -> 2,514 | -4% |
| street_visits | 11 -> 6 | 0 -> 4 | 50 -> 38 | 20 -> 16 | -21% |
| street_assaults | 0 -> 0 | 0 -> 0 | 4 -> 1 | 1.333 -> 0.333 | -75% |
| street_take | 55 -> 30 | 0 -> 20 | 250 -> 190 | 102 -> 80 | -21% |
| pimp_cut | 12 -> 7 | 0 -> 5 | 5 -> 20 | 5.667 -> 11 | +88% |
| stims_dealt | 1,206 -> 1,077 | 979 -> 1,402 | 1,076 -> 1,353 | 1,087 -> 1,277 | +18% |
| dealing_reports | 183 -> 74 | 146 -> 115 | 177 -> 92 | 169 -> 94 | -44% |
| vendor_meals | 728 -> 995 | 681 -> 866 | 1,384 -> 1,456 | 931 -> 1,106 | +19% |
| tinker_repairs | 0 -> 0 | 0 -> 0 | 0 -> 0 | 0 -> 0 |  |
| pimp_collections | 0 -> 19 | 0 -> 0 | 0 -> 33 | 0 -> 17 | new |
| pimp_pass_up | 0 -> 4 | 0 -> 3 | 0 -> 11 | 0 -> 6 | new |
| corner_deals | 0 -> 912 | 0 -> 1,389 | 0 -> 1,353 | 0 -> 1,218 | new |
| dealer_regulars | 0 -> 519 | 0 -> 761 | 0 -> 830 | 0 -> 703 | new |
| lookout_halved | 0 -> 176 | 0 -> 350 | 0 -> 259 | 0 -> 262 | new |

| arch | n | idle h/d | shifts | Served-able | met | enc/shift |
|---|---|---|---|---|---|---|
| gang_member | 72 -> 78 | 3.310 -> 3.580 | 219 -> 256 | 0.590 -> 0.560 | 0.330 -> 0.320 | 1.240 -> 1.120 |
| vendor | 16 -> 17 | 2.730 -> 2.910 | 4 -> 4 | 1 -> 0.500 | 0.250 -> 0 | 0.500 -> 0 |
| cabbie | 18 -> 22 | 3.520 -> 4.450 | 91 -> 114 | 0.110 -> 0.040 | 0.140 -> 0.120 | 0.140 -> 0.120 |
| homeless | 34 -> 23 | 5.580 -> 4.740 | 30 -> 48 | 0.570 -> 0.350 | 0.130 -> 0.120 | 0.170 -> 0.120 |
| all | 1,852 -> 1,855 | 4.080 -> 4.150 | 8,639 -> 8,937 | 0.300 -> 0.290 | 0.380 -> 0.380 | 2.260 -> 2.140 |

**Device: gang_member `met` did NOT go up (0.33 -> 0.32).** The day-19 window holds only a few dealers (5-23 doses per seed in days 19-26) and no pimp. The first pimps appear on days 32-36. 4b shows up in the counters instead. A day-90 diary (dealer #2495, seed 42) shows a Club dealer serving five buyers, two of them "a regular".

Street assaults now follow pimp presence: 1.3 -> 0.3 on the mean, but the base mean is 4 assaults on one seed, so the sample is tiny.

**OFF IDENTITY: DIFFER as the script reports it, and IDENTICAL by the rule.** `CITYSIM_LIVES_STREET_OFF` is also 4a's switch, so the script's off row compares candidate-off against base-on and differs by everything 4a does. I compared base-off against 4b-off with `scratchpad/offcmp.py` on seeds 42, 43 and 44 x 30 days:
- report CSV: IDENTICAL
- save (version masked): IDENTICAL
- events: IDENTICAL except the "saved <path>" line, whose directory differs per run.

The orchestrator should decide whether that is acceptable or whether 4b gets its own switch.

## Footers
- `test.py change`: `[change] GREEN; BUDGET OK 3m51s of 4m00s`
- `test.py lint`: `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 50`
- behaviour (`cargo test --release -p citysim-cli -- --ignored behaviour`): ok, 1 passed, 64 s.
- Budget: 4a ended at 4m26. One change cut real time: wages' emigrate test now runs its two 40-day cities in parallel (16.4 s -> 9.6 s). The new pimp probe adds about 3 s to person. The three slowest binaries are core 42.9 s (three 120-day seeds, already parallel), person 15.2 s and weapons 10.3 s. Box noise is large, since the measure ran beside the tier.

## Tests (each is new API or semantics, so it fails on base)
- `street.rs` lives_street: `test_street_assault_reads_the_pimps_presence_not_his_freedom`. I also extended `test_off_city_writes_nothing`. `street_night` now clones a shared OnceLock base.
- `stims.rs` lives_dealer: `test_dealers_third_buyer_is_a_regular_and_stat_buy_picks_the_known_dealer` and `test_lookout_halves_the_dealing_witness_term`.
- `person.rs`: `probe_pimp_purse_grows_on_a_pitch_night`. On day 12 it gives the gang a Home and opens a pitch, because seed 42 has neither that early.
- Printed bullets in `core.rs`: pimp's collection [19, 0, 33], dealer's regular [519, 761, 830], corner deal [912, 1389, 1353].
- `lod.rs` prints stims dealt per 100 adult-days by tier. That is milestone tier and was not run.

## Changed existing tests (please review)
- `vice.rs::test_pimp_present_zeroes_the_assault_roll` now places the pimp's body at the pitch, which is the semantics 4b changes.
- `person.rs::probe_parent_with_the_fee...`: the control mother was hired as a Sweeper on the 4b city and never reached the Camp, so the probe failed. Each eve the test now removes her Job, restoring the probe's stated premise ("the mother jobless"). The bought-out half was unaffected.

## Deviations (in § 14)
- The Statistical pimp is credited on his order, not on `shift_attended`.
- The pimp's round is scored like a shift, and new pimps prefer jobless members.
- A corner dealer's GangWork target is himself, and his plan is scripted.
- Corner steps skip the planner's symbols.
- No Full vendor sale exists, so the vendor's regulars come only from Statistical sales.
- The tinker encounter is wired but never fires at 2k.
- 4a's SAVE 38 note named `corner_dealers` before it existed; corrected.

## Left
- The device row at day 19 cannot show dealers or pimps. A later window (day 90) is needed for the gate.
- 2k street pitches are rare (0-2 at a time), so pimp numbers are small.
