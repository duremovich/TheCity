# Phase 8c fix round: report

Branch `agent-a65fe6e79ced8314e`, base 97e7cef (on main e566e29). **Final sha 7a68128.** Nothing is merged to main, and target/ is kept. All three fixes are in. The parity test stays RED, for a cause other than the bartenders (OPEN-130).

## Fix 1: venue staff cover the day and the evening (b433beb, OPEN-114 closed)
- **Change.** `service::venue_shift(world, employer, role)` puts each new `[service] evening_staff` post on the thinner of `[world] shift_day` and `[leisure] evening_shift` among that role's staff at the venue. A tie goes to the evening, so a venue of one keeps the drinking's peak and the 19:00 regular.
- **Why this split.** No venue keeps hours, so a split by the venue's open hours has nothing to read. A hash per post can leave a Bar with no day staff. Filling the thinner shift guarantees cover with two staff and adds no number.
- **Where it applies.** Seeding and `demography::hire` alike. `wages::site_shifts` counts the role on both shifts. The Statistical mirror is unchanged, since it reads the same `Job::shifts`.
- **Test.** `service.rs::test_venue_staff_cover_the_day_and_the_evening` failed on the base and passes now.
- **The regular probe passes,** with the base's own patron and server: Rafe Kettering at Bar#415, served by Leopold Adeyemi, ledger 3, an Acquaintance.
  - On the moved city the trip was what failed. The patron rode 170 ticks to the Bar. His own Unwind, past `venue_walk_cap_ticks`, could not plan, cooled, and dropped the probe's plan each time the cooldown lapsed. The control arm's second Bar was a 5 h walk.
  - The probe now puts him at the Bar's door at 19:00. Its assertions are unchanged.
- **Off identity** (LIVES_OFF, seed 42 × 30, off vs off against e566e29): IDENTICAL, both after this fix and on the final tree.

## Parity (`lod.rs::test_full_vs_statistical_within_15pct`)
| tree | paid venue visits, Full / Stat (tol) | Lounge, F / S | Bar, F / S | law0 | law1 |
|---|---|---|---|---|---|
| after fix 1 (run once) | 12.32 / 15.46 (1.85) RED | 0.19 / 1.16 | 0.82 / 1.34 | 0.845 / 0.738 in | 0.129 / 0.203 RED |
| final (milestone tier) | 12.77 / 14.75 (1.92) RED by 0.06 | 0.19 / 0.55 | 1.02 / 1.33 | 0.893 / 0.730 RED | 0.079 / 0.215 RED |

**Diagnosis (30-minute cap): the bartenders were not the cause, and nothing was demoted.**
- The Statistical Lounge is the largest and least stable gap. Its learned `p_venue` is 0.21, 8a read 0.19 / 0.20, and no Statistical venue code changed after 8a. So the excess comes from outside the learned evening.
- **The lead:** `classpay::status_daily`'s night out pays a Lounge through `pay_venue` → `note_paid` in both tiers, so a richer Statistical purse makes more Lounge visits. This is not verified. It is recorded as OPEN-130.
- `law0` moves around its line across trees: red at 8a, in at 8c, red now. `law1` is red, as on main.

## Fix 2: `god_cut_the_precinct` (f70f6ff)
- **Change.** The god now halves the funded force with `SetGuardCount` (the lever path, which pins the civic budget). It cuts 400 → 200 on day 20 and restores it on day 30. The run stays 40 days, two 20k cities.
- **Day 20, not day 10.** The brief said day 10. I kept day 20, as § 10 and the test name it.
- **Asserted:**
  - The cut lands in both cities: the city's guards (`law_brain::guards`) fall to the lever plus the State's guards (`outside.state_guards`). The jailing version fails this.
  - The Stations book and transfer during the cut.
- **Demoted to a FINDING** (doctrine 1, the brief's "print the rest"): "arrests fall less than in the no-station city". With the real cut, arrests still rise in both cities, so that row compares two noisy ratios.
- **Reading** (346 s, run once, again in the milestone tier):

| | with Stations | without |
|---|---|---|
| city guards, day 19 → max in days 25-29 | 435 → 319 | 479 → 318 |
| funded, lever plus State | 320 | 320 |
| arrests kept under the cut | 1.095 | 1.102 |
| bookings during the cut | 651 | 0 |
| transfers during the cut | 185 | 0 |

The arrest count does not track the force at 20k. That is a finding for Dylan.

## Fix 3: the Visit (43ab7cd, OPEN-116 closed)
- **Cause.** The score read the kin as alive (`kin_there`). The plan also needed the kin inside a building (`Position.building`). So the Visit won and could not plan.
- **Change:**
  - `household::visit_place` is now the one rule for both the score and the plan.
  - A "not on shift" gate (`shift_reachable`, the Work goal's own reading) holds the Visit until the shift ends. The visit stays due to midnight.
  - A visit with nowhere to walk is dropped at its plan.
  - A Visit that cools (`cool_goal`, or `goal_failed` twice) is dropped for the day.
- **Test.** `household.rs::test_a_visit_waits_out_the_shift_and_an_unplannable_one_is_dropped` failed on the base at the shift gate.
- **Diary,** `shadow --seed 43 --start-day 26 --days 7 --agent 2279` (the V5 courier), base vs fix:

| | base | fix |
|---|---|---|
| Visit wins | 17 | 0 |
| Visit cooldowns ("could not plan") | 18 | 0 |
| shifts interrupted by a Visit | 6 | 0 |

- **Left open (written in OPEN-116 and § 14):** a Full visitor whose kin is in no building makes no visit.

## Probes restaged (ae175cb, 6dd4577; no assertion changed)
Fix 1 changes seeding, so seed 42 diverges from tick 0. Each probe was restaged to its own written premise:
- **`person::probe_sixty_four_with_savings_retires`.** The pick married on the first evening and quit "no need of the wage". The pick is now single for the week.
- **`person::probe_pimp_purse_grows_on_a_pitch_night`.** The subject took the chair of a gang whose leader was new. The gang now needs a living veteran leader.
  - **Found on the way:** a gang's `leader` can name someone who belongs to no gang (seed 42, day 12, index 588). No OPEN row yet; this is for you to number.
- **`person::probe_parent_with_the_fee_…`.** The control's broke mother was capped by `min(10)` and stayed at 0. She walked 250 tiles to the Camp and fell asleep on the way. She now holds exactly 10 coins at each eve.
- **`succession::test_bug8_…`.** "Two free members besides the leader" is now part of the gang filter.
- **`tags::test_patrol_scans_and_attacks_the_chromed`.** The adult now stands at the spot the patrol picks: the district's most crowded, by the patrol's own count.

## OPEN ids (3e1676f, 7a68128)
- **Bugs:**
  - OPEN-114 Bar day drinkers, closed b433beb.
  - OPEN-115 the cab never driven by a body.
  - OPEN-116 the Visit, closed 43ab7cd.
  - OPEN-117 the off-screen spouse in the home evening.
  - OPEN-118 leaders do not lead.
  - OPEN-119 settlement at midnight before the encounter.
  - OPEN-120 the pimp's collection by link, not presence.
  - OPEN-121 `door_refusals` 0.
  - OPEN-122 bouts.
  - OPEN-130 the parity row.
- **Decisions:**
  - OPEN-123 the madam's cut.
  - OPEN-124 the 2k School.
  - OPEN-125 the frailty exit near 74.
  - OPEN-126 the foreman housing the homeless in a night.
  - OPEN-127 the Purist creed and decks.
  - OPEN-128 leaders delegating raids.
  - OPEN-129 a CEO with a Mid Hq living in the Spire.
- **OPEN-105** is closed with 97e7cef (settled).
- **Docs.** No `OPEN-???` is left in docs. LIVES_AT_WORK § 10, § 14 (a new "8c fix round" block) and § 15 are updated, and so is SHADOW_V5.

## Footers
- `[change] GREEN; BUDGET OK 3m33s of 4m00s`
- `[merge] GREEN; BUDGET OK 4m28s of 8m00s` (it includes the behaviour tier: green)
- `[milestone] RED (parity); BUDGET OK 13m38s of 20m00s` (core_20k ok, god_20k ok; the parity rows are above)
- `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 54`

The tiers ran on 6dd4577. The two commits after it, 07ec36d and 7a68128, change docs only.

## Addendum: Dylan's answers (f431c2b, the new final sha)
- OPEN-123 (the madam's cut, settled at shift end at 30 %): a bug, owner Lives at work 8d, open.
- OPEN-126 (deposit first): a feature, owner Lives at work 8d, open.
- OPEN-129 (the Spire CEO rides in): a feature, owner Lives at work 8d, open.
- OPEN-128 (hold court): a feature, owner pressure pass, open.
- OPEN-124 (the 2k School is fine), OPEN-125 (keep the frailty exit), OPEN-127 (flesh only): decisions, each `closed 7a68128` with the answer in the text. A row cannot cite the commit that writes it, so they cite the tree they settle.
- No code changed. Lint: 0 problems.
