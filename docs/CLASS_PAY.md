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
