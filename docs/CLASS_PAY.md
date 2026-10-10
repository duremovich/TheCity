# Class pay: tiered posts, tiered wages

Companion to `ECONOMY_V2.md` (wages from revenue), `JOBS_V2.md` (trades, the labour market) and `HUNGER_SICKNESS.md` (the flipped city). Read only with `[economy2] wages` on, like everything in the Real economy: the default (dole) city is byte-identical.

## The problem (measured)

Flipped config (`wages` + `no_safety_net` + `[sickness]` + shanties), seeds 42-44, 120 days, adults by Home tier, pooled (`tests/classpay.rs::class_table`, before this pass):

| day | class | adults | employed | mean coins | mean wage | synth % (last 30 days) |
|---|---|---|---|---|---|---|
| 0 | Spire | 1,043 | 98 % | 62.1 | 6.15 | 0 |
| 30 | Spire | 1,046 | 99 % | 10.6 | 6.36 | 51 |
| 119 | Spire | 982 | 97 % | 14.0 | 7.72 | 52 |
| 119 | Mid | 1,747 | 90 % | 28.0 | 7.54 | 37 |
| 119 | Sump | 1,882 | 81 % | 14.7 | 7.59 | 40 |
| 119 | Dreg | 1,065 | 41 % | 3.6 | 6.58 | 54 |

Every post pays the role's flat wage times the corp's `wage_rev`, so a Spire resident is a Vat Tech on ~6-7 coins like anyone else; the day-0 wallet (`coins_by_tier` 3x) is spent in a month and the Spire eats as much synth as the Dregs. Nothing in the flipped economy pays by class.

## Dylan's call (2026-10-10)

Tiered jobs and pay. Spire residents hold professional, management and executive posts with skill-tiered wages; such posts are scarce; class persists through income, not a day-0 wallet. More job variety in general.

## The design

**Grades.** Every Job carries a grade: 0 labour, 1 skilled, 2 professional, 3 management (`Job.grade`, saved, skipped when 0). A post pays `role wage x grade_mult[grade]` (`[classpay] grade_mult`, placeholders `[1.0, 1.3, 2.0, 3.0]`), times the corp's `wage_mult x wage_rev` and the poaching premium as before. The grade is fixed in `Job.wage_per_day` at hire or promotion, so the Full and Statistical tiers pay through the one path (`economy::collect_wage_scaled`): parity holds by construction.

**The pool split, not a top-up.** Grades apply only at corp-owned workplaces, and a corp pays every wage from its purse, whose payroll the revenue rule holds at `labour_share x R` (ECONOMY_V2 § 5). A graded post raises the payroll `P`, so the corp posts fewer places and its `wage_rev` steps down: the same revenue pool split unevenly, never a coin from elsewhere. The marginal wage the margin rule prices a new place at (`wages::post`, `seed_staff`, the floor pass) includes the post's grade. City, agent and gang employers are untouched (their wages are the Treasury's budget or a fixed wage).

**Where the grades come from.**
- *Bespoke roles*: a base grade by role (`[classpay] role_grade`, e.g. Ripperdoc and Researcher professional, Mechanic and Reporter skilled); the rest labour.
- *New trades* (`[[trades]]` rows with `classpay = true`, live only with class pay on; `grade` and `corp_only` keys): **Manager** (grade 3: every staffed corp workplace, one a floor), **Engineer** (2: Farm, Fab, Lab, Garage), **Accountant** (2: Market, Hotel, Clinic, Security Office), **Broker** (2: Market, Lab). Their work is the generic trade shift at the workplace; their pay is overhead out of the owner's revenue.
- *Skilled grade within labour roles*: `skilled_share` of a corp building's labour staff in a role (floor) hold grade 1 ("Senior Vat Tech"), filled by promotion.

**Who gets them.**
- *Background.* Each adult carries a `Schooling { origin, education }` (*new* component, only with class pay on): `origin` the class it was seeded or born into (0 Dreg, 1 Sump, 2 Mid, 3 Spire by Home tier), `education` the origin's mean (`[classpay] education`) plus a pure-hash spread (no RNG stream). A child born in the city takes its parents' mean education (plus a smaller spread) and its household's tier as origin; an immigrant its arrival Home's tier.
- *Merit* = `merit_edu x education + merit_skill x skill(role) + merit_tenure x tenure` (tenure: days at the same corp over `tenure_full_days`, capped at 1), minus `commute_w x tiles` from Home to the workplace.
- *Graded hiring (grade >= 2).* Graded vacancies are filled first each day (`job_search` ordering), by merit, from the jobless and from anyone holding a lower-graded post (a career move; a move within the same corp is a promotion and counts tenure). The mover's old post is vacated and posted as usual. No commute band: professionals commute.
- *Promotion to skilled* (daily, `classpay::daily`): an open skilled slot goes to the role's best labour holder at that building by merit after `promote_days` of tenure.
- *Execs.* A corp's replacement exec is picked (L1's `pick_exec`) among its management, then professional, post holders before anyone; the new exec leaves the post (vacated and re-posted).
- *Class persists.* Layoffs (the margin rule's newest hire, Hunker) take a graded (>= 2) post last. A graded post holder or an exec never relocates to a lower-tier Home (the P5g relocation and swap): a professional commutes rather than give up a Spire flat.

**Seeding by class** (world creation, after `seed_staff`): `trades::seed` posts the graded trades' places, `seed_staff` prices them at their grade, and two rounds of the graded-first `job_search` fill them from the city by merit, which pulls the Spire's (and some Mid's) adults out of the labour posts the nearest-first seeding gave them; the skilled slots are filled by merit with tenure waived.

**Scarcity.** Professional and management posts are a fixed number of places per workplace (no past-cap multiplier: `ceiling_at` returns the places for a grade >= 2 row), so the corps' building count bounds them: ~150-250 a city against ~330 Spire adults.

## Config

```toml
[classpay]
enabled = true                 # read only with [economy2] wages
grade_mult = [1.0, 1.3, 2.0, 3.0]
role_grade = { Ripperdoc = 2, Researcher = 2, Mechanic = 1, Reporter = 1 }
skilled_share = 0.25
promote_days = 30
merit_edu = 0.5
merit_skill = 0.3
merit_tenure = 0.2
tenure_full_days = 180
commute_w = 0.0005
education = [0.10, 0.25, 0.50, 0.80]
education_spread = 0.20
child_spread = 0.10
```

## Gates

Mechanisms are unit tests (`tests/classpay.rs`): the default config is byte-identical with the class-pay keys present (wages off); a graded post's wage at hire; Full and Statistical pay a graded post the same through the one path; graded vacancies go to merit (a Spire-educated candidate over a nearer Sump one) and a long-tenured internal candidate can win; promotion to skilled after tenure, not before; layoffs spare graded posts; the exec is picked from management; Schooling is deterministic and ordered by origin. The class table, Gini, employed, corps, population and Treasury are printed findings (`class_table`), never bands.

## Out of scope

Corp HQs (JOBS_V2 P8), education as a learnable stat (schools, training), wage bargaining, grade-specific behaviour beyond the shift (the manager's office politics), civic grades (sergeants, the Treasury's budget).

## Implemented (P1 `f49e081`, P2 `9957302`; 2026-10-10)

As specified, plus: with class pay on, a role the Statistical table has not learned (the four new trades; Ripperdoc and Mechanic were never in it) works the table's mean `p_attend` instead of a whole paid shift until `calibrate` learns it (P2). Not recalibrated (a single recalibration follows on the merged branch).

**Identity.** The default city: the 60- and 365-day seed-42 CSVs equal `ff60ea6`'s; `test_default_city_identical_with_classpay_keys`. The flipped city with `[classpay] enabled = false` equals `ff60ea6`'s flipped city (30 days, seed 42). `tests/wages.rs` pins class pay off (it pins the revenue rule at flat wages).

**Findings** (flipped config: wages + no_safety_net + sickness + shanties, the P5f flipped table; seeds 42-44 pooled; "off" is this branch with class pay off, identical to `ff60ea6`). By Home tier:

| day | class | mean coins off → on | median | mean wage | synth % |
|---|---|---|---|---|---|
| 119 | Spire | 14.0 → 33.7 | 6 → 11 | 7.72 → 9.49 | 52.3 → 35.5 |
| 119 | Mid | 28.0 → 44.6 | 10 → 9 | 7.54 → 7.47 | 36.7 → 36.5 |
| 119 | Sump | 14.7 → 24.0 | 9 → 7 | 7.59 → 6.77 | 39.9 → 43.5 |
| 119 | Dreg | 3.6 → 3.3 | 1 → 1 | 6.58 → 6.26 | 54.2 → 54.8 |
| 364 | Spire | 59.0 → 66.9 | 16 → 15 | 11.81 → 10.83 | 40.3 → 31.9 |
| 364 | Mid | 100.6 → 93.4 | 17 → 12 | 11.23 → 8.35 | 30.3 → 32.7 |
| 364 | Sump | 56.8 → 45.7 | 16 → 11 | 11.05 → 7.59 | 31.3 → 37.1 |
| 364 | Dreg | 154.8 (76 adults) → 5.6 (406) | 8 → 3 | 8.57 → 6.46 | 53.5 → 54.7 |

City: wallet Gini day 119 0.716 → 0.820, day 364 0.823 → 0.850; employed day 119 1,422/1,656/1,415 → 1,398/1,407/1,339, day 364 1,590/1,498/1,633 → 1,596/1,547/1,662; population day 364 1,773/1,819/1,892 → 1,858/1,844/1,809; corps (alive, founded included) day 364 9/13/11 → 19/19/12; Treasury day 364 60-69k → 62-127k (the tax rate parks at 0 either way).

By origin (`Schooling`), day 364: Spire-born mean 165.7 / median 18, wage 12.16, 305 of the ~440 graded (>= 2) posts; Mid-born 59.2 / 11, 105; Sump-born 35.6 / 10, 4; Dreg-born 31.0 / 8, 0. The skilled grade is the Sump's ladder (182 Sump-born skilled at day 364).

Read: class now persists through income (the Spire's wage edge over the Sump is 1.43x at a year, 1.07x off) and the Spire keeps the best diet, but the pool split costs the labour class: with graded posts in the payroll, the corps' `wage_rev` stays ~1.1 where it climbs to ~1.8 off (the payroll meets `P*` sooner and a bigger jobless pool fills vacancies before a shortage raise), so every class's labour wage is lower over a year and the Dregs are five times as many at day 364. Mobility into graded posts is near zero for the Sump in a year (tenure counts in full at 180 days).

Gates: flipped `core_sanity` 42-44 and `core_year` (seed 42) green; the default trio green (800 passed); god 21 green. The default `core_year` fails one window (days 270-300 starvation 51 > 50) on `ff60ea6` itself (its own CLI reproduces 356 / 51): pre-existing from P5g, not this branch.

## P3 (Dylan's calls, 2026-10-10)

1. **The premium from capital and profit** (P3a `77b16d6`). A graded wage still leaves the corp's purse in full, but only its labour-rate part counts as payroll for the revenue rule; the premium above it (`gross − gross ÷ grade_mult`) goes to a `Corp.prem` window. `P* = labour_share × R + payout`, where the premium is charged to profit (cashflow + premium) first and only then to the capital payout, so labour's share of revenue is the no-classpay city's. A graded place is posted and hired only while `profit + payout − premium` covers its premium; a corp in the red on its premium withdraws its open graded vacancies (no layoffs). An unfilled graded post is no labour shortage. The coin identity holds (unit test; `core_sanity` daily).
2. **The thin ladder** (P3b `f1e7e95`): a day in a skilled or graded post adds `edu_gain_per_day` 0.0003 education (+0.11 a year), capped at `edu_cap` 0.75; deterministic. A Sump-born skilled worker out-ranks a Spire-born outsider for its corp's Manager post after about three sim years, not after a month (unit test).
3. **No Corp HQs now**: they come with the bigger map (JOBS_V2 P8); management works at the corp's workplaces until then.
4. **The status sink** (P3c `3799b72`): a midnight ledger at both tiers. Above `status_floor` 60 coins an adult spends `status_frac` 0.05 of the excess a day: half on a night at the nearest Lounge (else a Spire Club, the door policy and the venue's fun), a quarter on fashion at the nearest corp Market, a quarter on cosmetic chrome at the nearest corp Clinic. Every coin lands in an owner's purse and its building's revenue, which pays the staff through the wage rule. Real food every meal was already the diet's rule above `real_meal_reserve`.
5. **Civic grades**: later (open items).

**Findings after P3** (flipped config, 42-44 pooled; off = class pay off, identical to `ff60ea6`):

| day | class | mean coins (off → P3) | median | mean wage | synth % |
|---|---|---|---|---|---|
| 119 | Spire | 14.0 → 40.9 | 6 → 12 | 7.72 → 10.91 | 52.3 → 32.1 |
| 119 | Mid | 28.0 → 30.2 | 10 → 11 | 7.54 → 8.65 | 36.7 → 32.3 |
| 119 | Sump | 14.7 → 19.6 | 9 → 9 | 7.59 → 7.68 | 39.9 → 40.9 |
| 119 | Dreg | 3.6 → 5.6 | 1 → 1 | 6.58 → 6.98 | 54.2 → 54.3 |
| 364 | Spire | 59.0 → 73.3 | 16 → 13 | 11.81 → 9.67 | 40.3 → 30.1 |
| 364 | Mid | 100.6 → 50.8 | 17 → 12 | 11.23 → 8.21 | 30.3 → 29.8 |
| 364 | Sump | 56.8 → 43.3 | 16 → 9 | 11.05 → 7.29 | 31.3 → 35.5 |
| 364 | Dreg | 154.8 → 14.9 | 8 → 3 | 8.57 → 5.90 | 53.5 → 53.4 |

Dreg adults on day 364 (pooled): off 76, P2 406, P3 257. Wallet Gini day 364: off 0.823, P2 0.850, P3 0.791 (day 119: 0.716 / 0.820 / 0.744). Employed day 364: 1,590/1,498/1,633 → 1,581/1,683/1,639. Population 1,773/1,819/1,892 → 1,786/1,882/1,823. Treasury day 364: 60-69k → 136-158k (tax parked at 0; explained below). Status spent ~395-450k a seed-year. By origin at day 364: Spire-born median 16, wage 11.64, 278 graded posts; Mid-born 11, 161; Sump-born 9, 7 (P2: 4); Dreg-born 9, 0.

## Why the Treasury doubles (seed 42, 365 days; `treasury_census`)

Every Treasury inflow and outflow through `ownership::transfer`, `cross_in`/`cross_out` and `refund_tax` is booked by flow in `World::probe` (diagnostic, never read by the sim; a payee that is no longer a corp, gang or agent would book as "(dead owner)": none did). The residual the census does not explain (the direct Treasury paths: the civic budget's own spends) is the same in both cities (−563/−588 a day in days 0-30, −74/−61 in days 240-365). The difference in days 240-365 (P3 +722 a day against off +93):

| flow | off | P3 | what |
|---|---|---|---|
| in `AssetUpkeep` | 631 | 1,056 | every non-city asset's daily upkeep to the Treasury (M13 D8): the class-pay city owns more assets (day 364 motorcycles 707 vs 555, cars 18 vs 10, robots 160 vs 80, chromed agents 610 vs 434) |
| in `Sale` | 4 | 236 | all of it dissolved corps' estates (`corps::dissolve`: a bankrupt corp's balance after its sell-off, past the heir cap, goes to the Treasury; M11). The class-pay city founds and incorporates more corps (19/19/12 alive vs 9/13/11) and some fail |
| in `Found` | 76 | 141 | founding fees (more foundings) |
| in `Export` (from outside) | 292 | 37 | the off city's foreclosed, city-owned producers selling to the World |
| out `Hack` | 174 | 0 | a Ledger run on the city in the off run's last window |
| out `Import`, `GambleWin` | 191 | 29 | |

Over the year the asset upkeep alone is ~+100k against off, the estates ~+32k, less the off city's larger export income. The status sink is not in it: its Leisure payments go to owned venues (`Leisure` into the Treasury is city-owned venues' ordinary visits, 93 vs 45 a day), and no status coin lands on a dead or missing owner. Legitimate flows, no bug: richer graded agents and cash-rich corps buy assets and pay their upkeep to the city, and more corps are founded and dissolved. With the tax rate parked at 0 nothing spends it back (the Treasury's outflow is an open item from JOBS_V2).

## Open items

- **A corp school or scholarships** (a faster, rarer ladder than night school; a corp buying talent).
- **Corp HQs** with the bigger map: management's home in the Spire.
- **Civic grades** (sergeants, Treasury clerks) once the Treasury's budget can carry them.
- **Labour wages stay under the no-classpay city's at a year** (Sump 7.3 vs 11.1; Dreg adults 257 vs 76). Read-only diagnosis (`wage_rule_census`, seed 42; summed over the corps, a day, days 240-365, off vs P3): revenue R 10,073 vs 11,162 (larger with class pay, not smaller); labour payroll P 6,443 vs 5,880; target P* 13,441 vs 7,003. Labour's revenue share (`labour_share × R`, ~5.5-6k) is the same in both; the gap is the **capital payout**. In the off city the corps' capital above `capital_hi_days` adds ~7.9k a day to P*, which keeps P under 0.9 P* (raises), keeps hiring until the jobless pool runs dry (140 jobless, 49 vacancies open), and then shortage raises follow (55 "a vacancy stood unfilled" steps against 7). Under P3 the graded premium is charged to that payout (profit before premium is near zero, so `labour_payout` hands labour almost nothing of it): P sits over 1.1 P* and `wage_rev` steps down (86 downs against 4). So it is neither a smaller revenue base nor graded hires draining labour; it is that Dylan's "premium from capital" takes exactly the excess capital that, without class pay, was the engine of the labour raises. If labour should keep it, the premium would need its own source (profit only, or a share of the payout).
- **The Treasury hoard grows** with class pay (to 136-158k at tax 0).
- **The stat table** must learn `p_attend` for Manager, Engineer, Accountant, Broker, Ripperdoc, Mechanic (the single recalibration on the merged branch).
