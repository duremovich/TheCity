# God scenarios v12: jobs and lives

The twelfth run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at the Jobs & lives milestone (`docs/JOBS_AND_LIVES.md` § 12): the industries and the corner, the El's power, weapons, vice and the chip, the organ book and the scavs, faith, the School, and an aged city. A post is a record on a building, a weapon an item on the asset store, a chip contract a timed flag on a Job, an organ a unit of stock, a creed a row of data: every one is game state between fictional agents of a simulated city, moved by seeded dice rolls. Ten scenarios in `citysim/tests/god.rs` (the v12 section, on the v8 harness), one per reaction class, each against the unshocked v8 control of the same seed:

```
cargo test --release -p citysim --test god -- --ignored --nocapture --test-threads=1 god_kill_workers god_power_cut god_flood_guns god_vice_legal god_chip_everyone_in_debt god_organ_boom god_cult_in_sump god_school_free god_kill_the_young god_outside_boom
```

About 150 s for the ten in sequence (35 s on parallel threads). Measured on the phase-8 tree (main at 4a9df24 plus the phase-8 work: the scavs' fix, the organ market order).

## How the suite works

- **Seed 42, default config, 60 days**, the shock at the start of its day, against `v8_control`. Each prints the v8 board's table plus the v12 rows (employment, the corner and day posts, pitches, dark districts and outages, armed fights and deaths, street pitches, sweeps, Solicitation arrests, the pimps' cut, chip hires, chipped living, chip episodes, Trafficking, organs made and exported, scav kills, abductions, black clinics, cult members, apostates and the hunted, tithes, school days and fees, children taken, elders employed and taken in, Rations).
- **Each asserts that the god commands applied and that the world reacted** (a named reaction per scenario below); how it reacted is printed and judged here, as gaps, not as red tests.
- **Commands** (`PlayerCommand`): `KillAgent`, `JailAgent { who, days }`, `FloodGuns { gang, n, tier }`, `SetStreetLegal`, `SetBrothelsLicensed`, `FundCorp`, `FundGang`, `SetAppetite { good, mult }`, `SetCreed { gang, creed }`, `SetOutsideWage`. Three levers have no command and are set on the world directly (gap 1 below): `public_school`, the chip trade's places, and a creed row's tithe.

## The scenarios

Windows are sums over days (means where a row says so); each cell reads `scenario / control`.

### `god_kill_workers`: 200 employed residents killed, day 45

| days / ctl | 30-45 | 45-50 | 50-60 |
| --- | --- | --- | --- |
| employed (mean) | 1312 / 1312 | 1295 / 1307 | 1302 / 1302 |
| population (mean) | 1896 / 1896 | 1665 / 1867 | 1650 / 1852 |
| violent deaths | 55 / 55 | 214 / 14 | 27 / 27 |
| immigrants | 7 / 7 | 4 / 3 | 5 / 3 |

Asserted reaction: employment at day 59 at least 90 % of the control's. **Reacted fully and at once:** employed read 1,295 on day 46 and 1,318 on day 59 (101 % of the control). The midnight job search refilled the dead workers' posts from the jobless the next night; 200 fewer people and the same employment.

**Gaps:**
1. The corner empties instead of filling: the queue's mean fell from 8 to 2 (the jobless it held were hired at midnight), so the dawn pick is not the route back to work it was specced to be after a shock.

### `god_power_cut`: every UtilityPlant's staff jailed 10 days, day 30

14 staff of the city's one plant. Asserted reaction: a district dark, or the plant staffed again the next day. **No district went dark** (0 dark district-days, 0 outages); at day 31's start 13 free staff held the plant's posts: the midnight search hired replacements the same night.

**Gaps:**
2. **The outage never lands.** A plant goes dark only when it has no staff through a whole day; the job search refills it overnight. A strike, a sabotage or a jailing does not cut the power unless the hiring pool is empty. The El slow-down on outages (a phase-1 carry-over) stays unexercised.

### `god_flood_guns`: 200 tier-3 weapons to the weakest gang, day 20

The crate went to Ninefold. Asserted reaction: more muster issues than the control.

| days / ctl | 10-20 | 20-40 | 40-60 |
| --- | --- | --- | --- |
| armed fights | 64 / 64 | 116 / 126 | 128 / 97 |
| armed deaths | 2 / 2 | 3 / 4 | 5 / 0 |
| violent deaths | 23 / 23 | 116 / 102 | 103 / 60 |

**Reacted, slowly:** armoury issues 44 against 15; armed fights over days 20-60 244 against 223, but fewer than the control in 20-40 (the crate sits in the armoury until a muster draws it). Violent deaths 219 against 162. The law moved to Crackdown on day 25 (the control on day 23) and spent the second month alternating Crackdown and Garrison.

**Gaps:**
3. A weapon in the armoury is not a weapon in a hand: issue happens only at a muster, so a flooded gang fights its street fights bare for weeks.

### `god_vice_legal`: street work legal and brothels licensed, day 10

The spec's day 20 comes after seed 42's only sweeps (days 10-20), so the levers move to day 10. Asserted reaction: fewer sweeps than the control. **Reacted:** 0 swept and 0 Solicitation arrests over days 11-60, against 5 and 4.

**Gaps:**
4. **Nothing else moves.** Street pitches stay at about 1 open (mean) and the pimps' cut stays 0. The street trade is too thin on seed 42 for the utopia lever to show (phase 4's carry-over: no pitch lands in a Sweep district).

### `god_chip_everyone_in_debt`: every jobless debtor offered the chip, day 20

47 jobless debtors in the pool; the chip trade's places opened past the pool and each Dollhouse's holder funded for the fees (no command). Asserted reaction: more chipped than the control.

| days / ctl | 10-20 | 20-40 | 40-60 |
| --- | --- | --- | --- |
| chip hired | 3 / 3 | 187 / 7 | 100 / 2 |
| chipped living (mean) | 1 / 1 | 32 / 2 | 129 / 4 |
| chip episodes | 0 / 0 | 4 / 1 | 3 / 1 |
| Trafficking | 0 / 0 | 0 / 0 | 0 / 0 |
| employed (mean) | 1225 / 1225 | 1385 / 1297 | 1405 / 1306 |

**Reacted strongly:** 287 hired against 9; 129 chipped alive on average in the last 20 days; 7 episodes against 2. The chip became the city's largest employer of the desperate: employment up 100 a day. More of them died too (violent deaths 210 against 162 over days 20-60).

**Gaps:**
5. No Rescue: nobody posts a Rescue for a chipped relative (no kin term reads `Chipped`).
6. Trafficking stays 0 (no gang holds a Dollhouse front; phase 5's carry-over).

### `god_organ_boom`: the World's appetite for organs x 5, day 20

Asserted reaction: more days on Harvest or more abductions and scav kills than the control. **Reacted, weakly:** abductions plus scav kills 106 against 99; days with a gang on Harvest 39 against 39 (both scav gangs already hold Harvest most days since the phase-8 fix); organs exported 22 against 25.

**Gaps:**
7. **The organ book is supply-bound, not demand-bound.** Two units a body, rotting a quarter a day; the Clinics' shelves take the first units; a boom raises the price, not the volume. The scavs' order reads the appetite (phase 8: the Harvest tilt scales with it, 0.5-2) but they were already on Harvest.
8. No black clinic opens (0 on every run): a gang needs a held district's vacant Lot and 200 coins over the found cost.

### `god_cult_in_sump`: the largest creedless Sump gang turns cult, day 10

The Hollow took the cult row with its tithe raised to 1.0 and a leader of persuasion 1.0. Asserted reaction: a cult stands on day 59.

| days / ctl | 0-10 | 10-30 | 30-60 |
| --- | --- | --- | --- |
| cult members (mean) | 4 / 4 | 59 / 28 | 78 / 37 |
| apostates | 1 / 1 | 10 / 5 | 14 / 4 |
| apostates hunted | 3 / 3 | 3 / 2 | 23 / 3 |
| tithes | 47 / 47 | 1154 / 436 | 2628 / 974 |
| violent deaths | 6 / 6 | 84 / 89 | 123 / 96 |

**Reacted strongly:** 80 members on day 59 (the control's cult 37); apostates 24 against 9 and the hunted 26 against 5: the tithe drives members out and the cult hunts them. Tithes ran 2.7x the control's.

**Gaps:**
9. The Purists do not react to a second creed in the Sump: no order, no Patrol scan, no vendetta (the Purist tilt reads chrome, not creeds).
10. A 1.0 tithe leaves the members broke by design, but nothing houses them at the chapel (the spec's "housed at the chapel").

### `god_school_free`: the `public_school` lever, day 10

Set on the levers (no command). Asserted reaction: more school days or lower fees than the control. **Reacted:** fees 0 against 26, school days 20 against 13 over days 10-60.

**Gaps:**
11. **Almost no child is in reach of a School.** Two Schools stand per copy, in Mid and up; a child counts only within `school_reach` (60 tiles) of its Home. Free school reaches the same handful; the Sump's children play or go to the camp.

### `god_kill_the_young`: every adult under 35 killed, day 30

588 killed. Asserted reaction: the city stands and the survivors take up the posts (the employed share of the population rises).

| days / ctl | 20-30 | 30-40 | 40-60 |
| --- | --- | --- | --- |
| population (mean) | 1945 / 1945 | 1335 / 1903 | 1335 / 1863 |
| employed (mean) | 1283 / 1283 | 1265 / 1311 | 1258 / 1306 |
| elders employed (mean) | 326 / 326 | 449 / 334 | 439 / 333 |
| immigrants | 5 / 5 | 16 / 4 | 48 / 9 |
| elders taken in | 6 / 6 | 2 / 10 | 0 / 10 |
| pitches (mean) | 8 / 8 | 1 / 6 | 0 / 4 |

**Reacted:** the city lost 30 % of its people and kept 96 % of its employment: elders employed rose from 326 to 449, immigration rose 5x, the corner (10 a day in the control) and the pitches emptied as their people took posts, the scavs' takings stopped for ten days. Output (Rations made) held at the control's.

**Gaps:**
12. Kin take-in falls to 0: the adult children are dead, and nobody else (a friend, the Mission) takes a frail elder in.

### `god_outside_boom`: the outside wage at half its level, day 1

Asserted reaction: more immigrants than the control. **Reacted:** 74 against 21 over 60 days; employment +35 a day in the second month.

## Gaps, ranked

1. **Missing god commands** (3 scenarios): `SetPublicSchool`, a chip offer to a named pool, a creed row's tithe. Direct world edits stand in.
2. **Jobs refill overnight** (kill workers, power cut): the midnight search makes every vacancy a one-night hole, so an outage, a strike or a massacre does not reach the corner, the El or the lights.
3. **The street trade is too thin to show its levers** (vice legal): about one street pitch open, no pimp's cut.
4. **The organ book is supply-bound and rots**: a boom moves the price only.
5. **No black clinic, no Trafficking, no Rescue of the chipped** on any run.
6. **Schools out of reach** of the children who most need one.
7. **The Purists ignore a rival creed**; a 1.0-tithe cult's members are not housed.
8. **Weapons sit in the armoury** until a muster.
9. **Elders without children** have no one to take them in.

## Retired 2026-10-18 (the process review)

Dylan's decision: one scenario per reaction class, at most 20 plus the three controls (`docs/TESTING.md`, the god class table). The set went 41 → 20; the 21 below were deleted from `citysim/tests/god.rs` (the other two files lost nothing). The write-ups above and in `docs/history/GOD_SCENARIOS_V*.md` stay as the record. For each: its class, the reaction it asserted (the command-applied asserts left out), and the unit test that carries that reaction; "none" means an `OPEN.md` row, not a new test in this pass. Of the ten v12 scenarios two stand, `god_power_cut` and `god_flood_guns`; the command at the top now runs those two.

| scenario | class | reaction asserted | carrying unit test |
|---|---|---|---|
| `god_bankrupt_city` | the city's money | generic: a gang order, the posture or a daily series moved within 7 days of `SetTreasury(-50_000)` | `treasury.rs::test_civic_spending_stops_under_band_lo`, `treasury.rs::test_civic_headcounts_drop_at_the_pace` (the Treasury's own responses; the generic reaction had no one mechanism) |
| `god_crackdown_forever` | the law pinned | generic reaction against the Patrol-pinned control within 7 days of a Crackdown pin (to day 60 printed) | `law_faction.rs::test_player_pins_and_releases_the_posture` (the pin), `factions.rs::test_order_lie_low_under_heat` (the gang's LieLow), `law_faction.rs::test_a_greedy_captain_takes_the_bribe_and_the_crackdown_lifts` (the bribe) |
| `god_subscribe_and_chrome_everyone_3` | chrome and violence (the Trauma crews) | Trauma subscribers on day 10 above the control's; downed or episodes over days 5-60 differ from the control's | `wounded.rs::test_trauma_subscription_weekly_and_lapses`, `wounded.rs::test_trauma_call_and_save` |
| `god_parley_strongest_gangs_in_vendetta` | grudges, Hunts, vendettas (the Parley) | a gang's orders off the control's on a day 20-60, or a Parley | `leverage.rs::test_parley_makes_peace_for_tribute`, `leverage.rs::test_parley_refused_declares_the_feud` |
| `god_ambient_boom` | the outside world's demand (the Ambient book) | Ambient coins paid from day 10 above the control's (the pin applied) | `outside.rs::test_appetite_lever_pins_and_releases` (the pin), `outside.rs::test_sale_is_revenue_and_crosses_from_the_world`, `outside.rs::test_ambient_sales_happen_and_the_books_balance` (the spend reaching the businesses) |
| `god_outside_boom` | the outside world's demand (the outside wage) | immigrants over days 1-60 above the control's | `immigration.rs::test_expected_wage_and_pull` (`SetOutsideWage` and the pull), `immigration.rs::test_who_comes` |
| `god_kill_militech` | the corps' money (the outside parents) | Militech International dead by day 120 after three Data wipes, two thefts and an embargo; the control's alive | `weapons.rs::test_theft_cuts_edge_and_wipe_drops_the_tier_and_caps_the_branch`, `weapons.rs::test_parent_dies_at_death_days_and_takes_its_branches`, `weapons.rs::test_rich_failing_parent_lives`, `parents.rs::test_embargo_stops_every_crossing` |
| `god_raze_arasaka` | the corps' money (the outside bodies) | a Reinforced, BuyBackOffered, BranchRetook or BranchFounded within 30 days of every Arasaka building going to Ninefold | `bodies.rs::test_reinforce_caps_jobs_and_cohort`, `bodies.rs::test_buyback_then_retake_forces_the_sale`, `bodies.rs::test_found_branch_after_the_branch_died` |
| `god_decapitate_twice` | gang succession | a Succession, CivilWar or CivilWarEnded naming The Hollow within two days of each of two leader kills (days 20 and 35) | `succession.rs::test_clear_succession_promotes_from_within`, `succession.rs::test_close_succession_is_a_civil_war` (and `god_decapitate_gang`, `god_decapitate_close_succession` stay) |
| `god_hit_each_exec_day_10` | the hit market | command-applied only (at least one brokered Hit posted); the corps' Lobby days printed | `contracts.rs::test_brokered_post_escrows_and_identity_holds`, `contracts.rs::test_matching_never_offers_hit_to_target_kin` |
| `god_hit_hollow_leader_from_arasaka` | the hit market | command-applied only (the corp's Hit posted, brokered or direct) | `contracts.rs::test_brokered_post_escrows_and_identity_holds`, `contracts.rs::test_direct_renege_posts_betrayed_and_grudge` |
| `god_fixer_licence_off` | the board's levers | command-applied only (the licence lever applied); the offices' heat printed | `contracts.rs::test_fixer_licence_off_floors_heat_and_blocks_register` |
| `god_law_locate_on_gang_leader` | the board's levers (a public bounty) | command-applied only (the Locate posted); sightings paid printed | `contracts.rs::test_public_locate_paid_by_treasury_sets_last_seen`, `contracts.rs::test_locate_pays_at_most_once_per_gap_and_caps` |
| `god_abroad` | the State (a job abroad) | a crew left; an AbroadResolved within `abroad_days` + 2; a CastArrived for the crew within `promote_delay_days`, with the edge when the crew member lived | `abroad.rs::test_crew_is_away_then_comes_back`, `abroad.rs::test_won_depot_pays_and_keeps_the_identity`, `abroad.rs::test_promotion_past_the_thresholds_arrives_with_the_edge` |
| `god_kill_workers` | Jobs & lives: labour | employment on day 59 at or above 90 % of the control's after 200 employed killed on day 45 | none (the overnight refill after a mass death is gap 2 above; `jobs.rs` tests the band's posting, not the refill) |
| `god_vice_legal` | Jobs & lives: vice | sweeps over days 11-60 below the control's (which swept someone) | `vice.rs::test_street_sweep_in_a_sweep_district_only` (no sweep with `street_legal` on), `streetwork.rs::test_street_cut_and_sweep` |
| `god_chip_everyone_in_debt` | Jobs & lives: the chip | chip hires over days 20-60 above the control's | `chip.rs::test_debtor_takes_the_offer_only_while_short`, `chip.rs::test_contract_installs_the_chip_and_its_load` |
| `god_organ_boom` | Jobs & lives: the organ book | days with a gang on Harvest, or abductions plus scav kills, above the control's after the appetite x 5 | none for the appetite-to-Harvest link (gap 4 above); the supply side is `scavs.rs::test_scav_prey_ranked_past_cover`, `gray_faith.rs::test_organs_sold_then_exported` |
| `god_cult_in_sump` | Jobs & lives: faith | cult members above zero on day 59 after a Sump gang turns cult | `gray_faith.rs::test_widow_attends_converts_and_tithes`, `gray_faith.rs::test_apostasy_grudge` |
| `god_school_free` | Jobs & lives: the School (a policy lever) | school days up or fees down against the control after `public_school` (the lever held) | none (`public_school` has no command and no unit test; gaps 1 and 6 above) |
| `god_kill_the_young` | Jobs & lives: ages | population and employment above zero on day 59, and the employed share of the population higher than on day 29, after every adult under 35 killed on day 30 | none for the asserted reaction; `ages.rs::test_fired_elder_moves_in_with_his_daughter` covers the elders taken in (printed, not asserted) |

Also removed with them: `patrol_control` and `pin` (the Patrol-pinned control, used only by `god_crackdown_forever`), `gang_named`, the `AmbientDay` / `run_ambient` harness, and the v8 harness's `bounties` and `fixers` readings (read only by the retired board scenarios). The `OnceLock` controls `v5_control`, `v6_control` and `v8_control` keep callers and stay.
