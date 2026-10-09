# Jobs and room: a city where most adults work

Companion to `ECONOMY_V2.md` (wages from revenue, the World, no dole), `TESTING.md` (the tiers), `LIFE_L2.md`, `M16_CONTRACTS.md` and `VISION.md`. Plan: `~/.claude/plans/jobs-v2.md`. Where this document and `ECONOMY_V2.md` disagree on jobs, staffing, immigration or the Treasury's spending, this one wins.

Dylan, 2026-10-09: *"Build wages. We should have a lot of job types. I think we will end up expanding the map and buildings will have multiple floors. So if we need more room for jobs, make it."* Vagrancy fines stay; the Treasury should pay admin salaries and a police force; emigration stays hard (the 20-coin fare). The overriding goal is emergent story: people who appear to act like people. **A jobless majority is a broken city, not a brutal one.**

The short version of what follows: the wages-on city has a job for one adult in three, so the other two lose their homes, steal to eat, join gangs and die, and the city bleeds 1,983 → 1,158 in a year. The fix isn't more money. The corps already hold 100k and the Treasury 47-109k at a tax rate of 0. What's missing is **places to work** (the Lots run out by day 60), **a wage rule that hires instead of raising pay** in a slack market, **revenue lines** that keep money in the city (local power, freight, construction, packing) instead of leaking it, **a Treasury that spends** on police and clerks, and **immigration that answers open jobs** (today it answers only Street happiness).

Identifiers marked *new* don't exist at HEAD (e720297). All numbers are measured unless marked *placeholder*.

---

## 0. Measured

**How.** A scratch probe crate (session scratchpad, never committed) depends on `citysim` by path, replicates `World::tick` with a timer around every system, and snapshots the city at the end of each day: every adult's Job, class and home district; every building's kind, owner, `full_staff`, `staff_ceiling`, staff and open vacancies; each vacancy from post to close; each job-loss spell (the days from losing a Job to the next one); each immigrant from arrival to day 119. "Wages" means `[economy2] wages = true` and `no_safety_net = true` (the target city); "default" is the shipped config. Seeds 42-44 × 120 days, plus a 365-day seed-42 run of each, all in parallel on the 24-core box (so ticks/s are relative, not absolute). The CLI's `run --report` gives the money flows (seed 42).

### 0.1 Headline (120 days, seeds 42 / 43 / 44)

| | default | wages + no net |
|---|---|---|
| population day 119 | 2,036 / 2,032 / 2,023 | 1,803 / 1,764 / 1,824 |
| adults day 119 | 1,961 / 1,961 / 1,945 | 1,763 / 1,721 / 1,780 |
| **employed day 60** (share of adults) | 523 / 542 / 527 (26-27 %) | 703 / 689 / 702 (36-37 %) |
| homeless (mean over 120 days) | 62 / 94 / 99 | **932 / 957 / 971** |
| evictions | 173 / 216 / 244 | **2,100 / 2,093 / 2,023** |
| violent deaths | 132 / 116 / 131 | 279 / 328 / 268 |
| starvation deaths | 1 / 1 / 3 | 10 / 11 / 7 |
| births | 75 / 71 / 78 | 40 / 43 / 44 |
| immigrants / emigrants | 100 / 97 / 93 · 1 / 6 / 7 | 63 / 67 / 62 · 5 / 4 / 3 |
| open vacancies (daily mean) | 1.4 / 1.3 / 1.6 | 2.2 / 4.8 / 2.1 |
| ticks/s (probe, loaded box) | 5.7k / 5.3k / 5.5k | **3.1k / 3.2k / 3.0k** |

### 0.2 The year (seed 42, 30-day windows)

| days | default pop · employed · homeless · violent deaths · imm · births · emig | wages pop · employed · homeless · violent deaths · imm · births · emig · evictions |
|---|---|---|
| 0-30 | 2,032 · 500 · 60 · 15 · 27 · 20 · 0 | 1,983 · 764 · 494 · 35 · 12 · 13 · 0 · 1,199 |
| 30-60 | 2,042 · 525 · 49 · 32 · 24 · 18 · 0 | 1,936 · 706 · 1,170 · 71 · 17 · 12 · 0 · 345 |
| 90-120 | 2,036 · 548 · 80 · 46 · 25 · 19 · 1 | 1,803 · 728 · 955 · 84 · 18 · 7 · 3 · 319 |
| 150-180 | 1,941 · 559 · 126 · 86 · 19 · 13 · 4 | 1,645 · 733 · 682 · 108 · 15 · 17 · 2 · 339 |
| 210-240 | 1,819 · 554 · 122 · 57 · 22 · 19 · 15 | 1,481 · 732 · 550 · 111 · 16 · 7 · 0 · 305 |
| 270-300 | 1,723 · 486 · 117 · 71 · 18 · 19 · 9 | 1,290 · 760 · 309 · 126 · 15 · 18 · 2 · 229 |
| 330-360 | 1,603 · 525 · 132 · 42 · 20 · 14 · 11 | 1,163 · 804 · 69 · 68 · 17 · 12 · 1 · 128 |
| day 364 | 1,600 (adults 1,392) | **1,158 (adults 1,009, employed 812 = 80 %)** |

Read: **both cities shrink, and from violence.** Wages: 1,168 deaths in the year, 1,083 of them violent, against 193 immigrants and ~150 births. Default: 794 deaths against 266 + ~230. Starvation is ~0 in both. The wages city stops shrinking once it's small enough that its places employ 80 % of its adults: **the population settles where the places are.** To hold ~1,950 adults, the city needs ~1,300-1,550 places that revenue pays for.

### 0.3 Who dies and who leaves (seed 42, the year; status at the previous midnight)

| wages + no net: 1,168 deaths | n | | default: 794 deaths | n |
|---|---|---|---|---|
| violence · jobless · housed · gang | 293 | | violence · jobless · housed · gang | 389 |
| violence · jobless · **homeless** | 251 | | violence · jobless · housed | 95 |
| violence · jobless · homeless · gang | 182 | | violence · employed · housed (Street) | 78 |
| violence · employed · housed (Corp) | 146 | | violence · employed · housed (Corp) | 76 |
| violence · employed · housed (Street) | 142 | | violence · employed · housed · gang | 32 |
| violence · jobless · housed | 40 | | violence · jobless · homeless · gang | 28 |
| overdose, accident, flatline, starvation | 61 | | overdose, accident, flatline, starvation | 50 |

About two thirds of the dead were jobless (gang members, the homeless). Emigrants are rare (16 a year with wages, 80 by default), all jobless and homeless Dregs who could afford the fare. **Immigrants die young:** of 193 who arrived in the wages year, 89 were alive on day 364 (default: 172 of 266).

### 0.4 Jobs by role (day 60 / day 119, seeds 42 / 43 / 44)

| role (label) | default d60 | wages d60 | wages d119 |
|---|---|---|---|
| Farmer (Vat Tech) | 165 / 159 / 158 | **369 / 358 / 345** | 382 / 359 / 346 |
| Cook | 39 / 44 / 43 | 79 / 89 / 77 | 80 / 91 / 80 |
| Guard (city + private) | 44 / 43 / 42 | 49 / 45 / 45 | 42 / 29 / 47 |
| Clerk | 33 / 36 / 33 | 39 / 40 / 38 | 40 / 40 / 38 |
| Fabber | 15 / 23 / 18 | 30 / 23 / 37 | 27 / 20 / 23 |
| Host | 30 / 30 / 30 | 30 / 31 / 31 | 30 / 31 / 31 |
| Researcher | 14 / 12 / 13 | 24 / 21 / 30 | 22 / 23 / 21 |
| Bartender | 15 / 15 / 15 | 20 / 25 / 24 | 25 / 30 / 27 |
| Sanitation (public works) | **130 / 132 / 125** | 12 / 12 / 12 | 12 / 12 / 12 |
| Attendant, Croupier, Fighter, Mechanic, Concierge, Ripperdoc, Reporter, Gravedigger, Volunteer, Fixer | 44-48 | 50-61 | 61-66 |
| **total** | 523 / 542 / 527 | 703 / 689 / 702 | 728 / 685 / 680 |

Wages on, the Farms (overtime against the World's Food order, six Farms grown on World demand) carry the gain; public works (118 sweepers) go with the net. Nineteen roles, but four of them (Farmer, Cook, Sanitation/Guard, Clerk) are 80 % of all jobs. **Most residents can only be a farmhand.**

### 0.5 Places by building kind (seed 42, day 60)

`full` = `corp_brain::full_staff` × standing buildings; `ceiling` = `wages::staff_ceiling` (×3 past full staff while revenue covers the hire); `filled` = Job holders at the kind.

| kind | default: n · full · filled | wages: n · full · ceiling · filled · open |
|---|---|---|
| Farm | 12 · 168 · 165 | **18 · 252 · 756 · 369 · 2** |
| Market | 3 · 36 · 33 | 3 · 36 · 108 · 39 · 0 |
| Bar | 3 · 15 · 15 | 3 · 15 · 45 · 20 · 0 |
| NoodleBar | 17 · 51 · 39 | 17 · 51 · 153 · 79 · 0 |
| Club / Arcade / FightPit / Den / Lounge | 3 / 3 / 2 / 2 / 1 · 61 · 58 | same · 61 · 183 · 61 · 0 |
| Fab | 2 · 24 · 15 | 2 · 24 · 72 · 30 · 0 |
| Lab / Clinic / Garage / Feed / Fixer | 5 / 3 / 3 / 2 / 1 · 38 · 20 | 4 / 3 / 3 / 2 / 1 · 34 · 102 · 39 · 0 |
| Security Office | 2 · 12 · 8 | 2 · 12 · 36 · 13 · 0 |
| Mission | 3 · 6 · 0 | 4 · 8 · 24 · 2 · 0 |
| Precinct (Jail): city guards via `guard_count` 36 | 1 · — · 36 | 1 · — · — · 36 · 0 |
| Recycler (Cemetery): sweepers, diggers | 1 · — · 134 | 1 · — · — · 15 · 1 |
| **Civic Hall, Reserve Depot, Capsule Hotel, Camp, Home** | no role at all | no role at all |
| **Lot (vacant)** | **10** (of 60 at seed) | **3** |
| total | full 411 · filled 523 | full 493 · ceiling 1,479 · filled 703 |

Two things stand out. Five standing kinds employ nobody: the Hall, the Depot, the Hotel, the Camp and 384 Blocks. And **the wages city has used its last Lots by day 60** (2 left by day 119; the default year ends with 1).

### 0.6 Jobless by class and district (day 60)

| seed 42 | default: jobless · employed (share) | wages: jobless · employed (share) |
|---|---|---|
| class Street (housed) / Dreg (homeless) | 1,446 / 22 jobless | **87 / 1,113** jobless |
| Spire | 97 · 204 (0.68) | 15 · 228 (0.94) |
| Mid East | 177 · 176 (0.50) | 241 · 190 (0.44) |
| Mid West | 338 · 26 (0.07) | 422 · 88 (0.17) |
| Sump Central / East / West | 287 · 25 / 259 · 58 / 315 · 19 (0.05-0.18) | 33 · 43 / 58 · 66 / 24 · 64 (homeless counted where they stand) |
| Civic | 5 · 11 | **410 · 13** (the homeless gather here) |

The default city's jobless sit housed in the Sump and Mid West, fed by the dole. With wages on, the jobless are evicted and end up sleeping rough in Civic and Mid West.

### 0.7 The labour market

| | default (42 / 43 / 44) | wages (42 / 43 / 44) | wages year (42) |
|---|---|---|---|
| vacancy life (mean days, by kind) | 1.0-1.4 (Jail 1.2-1.4) | 1.0-1.6; Jail 1.8-2.3, Security Office 1.6-2.7 (max 14) | open at day 364: 18, max age 14 |
| job-loss spells re-employed within 7 / 14 / 30 days | 0.42 / 0.51 / 0.62 · 0.36 / 0.44 / 0.51 · 0.42 / 0.47 / 0.55 | 0.17 / 0.26 / 0.34 · 0.30 / 0.38 / 0.45 · 0.19 / 0.28 / 0.34 | 0.33 / 0.47 / 0.60 (median 13 days) |
| immigrants: arrived · ever hired · employed d119 · homeless d119 | 100 · 47 · 44 · 6 (seed 42) | 63 · 32 · 19 · 23 (seed 42) | 193 · 161 · 64 · 7; 89 alive |
| immigrants a week (`13 × logistic(Street happiness)`) | 5-7 | 2-5 (factor 0.16-0.38) | 3-5 |

A vacancy lasts a day because there are a thousand applicants for every place. The only slow posts are the guards' (`lawfulness ≥ 0.4`). **Immigration doesn't read vacancies or empty homes at all.** In the wages year, 18 posts were open on day 364 while the weekly count sat at 4.

### 0.8 Money (seed 42, CLI, days 91-120 means a day)

| flow | default | wages + no net |
|---|---|---|
| wages (all) | 2,543 | 6,574 |
| mean gross wage · `wage_mult_mean` | — | **11.0 · 2.0** |
| food bought by residents | 6,743 | **3,426** |
| export (World buys) | 1,276 | 2,280 |
| inputs to the World (E13: power, per-unit inputs) | 0 | **715 (a leak)** |
| property rate to the Treasury | 0 | 251 |
| tax (rate) | 1,290 (0.12) | 0 (0.00 from day ~35) |
| Treasury · corps (Σ) · wallets, day 120 | 66.9k · 108.6k · 9.6k | 46.9k · 100.9k · 37.1k |
| thefts a day · vagrancy charges a day | 113 · 6 | **690 · 42** |
| recycler till (scrap economy) | 0 | 6.8k |

The wage bill grew 2.6×, but `wage_rev` doubled: **the same 6.6k a day at the base wage would employ ~1,050, not 715.** The rule raises pay whenever payroll is under its target, even with 1,100 adults out of work. Corps and the Treasury sit on ~150k between them.

### 0.9 Room

| | cells (7 × 7 between roads) | Homes | Lots | other buildings | **free ground** |
|---|---|---|---|---|---|
| Mid | 224 | 140 | 14 | 8 | **62** |
| Sump | 234 | 200 | 16 | 4 | **14** |
| Spire | 90 | 60 | 10 | 0 | **20** |
| Civic | 50 | 0 | 10 | 9 | **31** |
| Vats | 138 | 0 | 10 | 28 | **100** |
| **map (256 × 192)** | **736** | 400 | 60 | 49 | **227 (31 %)** |

A third of the map is empty ground nobody can build on: founding, Grow and seeding only use `BuildingKind::Lot`. Housing is not short (384 Blocks seat 2,304; the wages city has ~950 homeless because of rent, not beds). Buildings have one floor.

### 0.10 Throughput (seed 42, 120 days, seconds in each system)

| system | default | wages | × |
|---|---|---|---|
| think | 9.8 | 21.5 | 2.2 |
| law | 1.3 | **7.5** | 5.8 |
| plan | 3.5 | 5.2 | 1.5 |
| exec | 4.8 | 5.7 | 1.2 |
| lod | 5.4 | 5.8 | 1.1 |
| corp_brain + ownership | 0.3 | 3.0 | 10 (midnight passes) |
| total | 30.2 s (5.7k ticks/s) | 55.1 s (3.1k ticks/s) | 1.8 |

Events per 30 days (days 30-60, seed 42): Theft 1,902 → **25,082**, Starving 1,441 → **22,725**, Witness 3,365 → 18,100, Vagrancy 29 → 1,790, Report 682 → 2,015. Wages-on costs throughput because ~950 homeless people are hungry, stealing and being arrested: the planner re-scores urgent needs, and the law processes the witnesses, reports and Vagrancy sweeps. The halved ticks/s is a symptom of the broken city, not a code hot spot. The one real inefficiency is `ownership::staff_by_building`, which scans every worker per corp per pass at midnight (×10, ~3 s a run).

### 0.11 What the measurements say

1. **Places, not money, bind.** 493 full-staff places (1,479 at the ceiling) for ~1,950 adults; the Lots are gone by day 60; five kinds employ nobody; corps hold 100k and the Treasury 47k.
2. **The wage rule inflates instead of hiring.** `wage_rev` reaches 2.0 in a city with 60 % of adults jobless. At 1.0 the same payroll employs ~45 % more people.
3. **Joblessness kills through rent and violence, not hunger:** evictions → homeless → theft, gangs and violence. Two thirds of the dead are jobless. The year shrinks until ~80 % of adults have a place.
4. **Immigration is blind:** 3-5 a week from Street happiness alone, while open posts age to 14 days. Half of each year's immigrants are dead or gone by its end.
5. **Throughput follows the homeless count.** Fix the jobs and it comes back.

---

## 1. Job types

### 1.1 Today's roles

| role (label) | where | wage | shift | what pays it | the day it gives |
|---|---|---|---|---|---|
| Farmer (Vat Tech) | Farm | 6 | day | Food sold to Markets and the World | the vats, the haul, the season |
| Clerk | Market | 6 | day | food sales | the counter |
| Bartender | Bar | 5 | day | drinks | regulars |
| Guard | Precinct (city) · Security Office (corp) | 8 | day / night (alternate ids) | Treasury · security contracts | patrol legs, arrests |
| Sanitation | Recycler (city) | 5 | day | Treasury | sweeping the beat |
| Gravedigger (Recycler Tech) | Recycler | 5 | day | Treasury | bodies, burials |
| Ripperdoc | Clinic | 8 | day | chrome, therapy, detox | installs |
| Mechanic | Garage | 7 | day | vehicles, repairs | |
| Researcher | Lab | 4 | day | Data sales | runs, ICE |
| Reporter | Feed | 4 | day | ads | stories |
| Host · Concierge · Croupier · Fighter | Club · Lounge · Den · FightPit | 6 · 8 · 6 · 7 | evening | the venue's take | bouts, bets |
| Attendant · Cook | Arcade · NoodleBar | 5 · 5 | day | visits, meals | |
| Fabber (Fab Tech) | Fab | 7 | day | Parts | the floor |
| Fixer | Fixer's office | 6 | day | contract cuts (M16a) | |
| Volunteer | Mission | 0 | day | donations | the kitchen |

Hard limits in the code: **one role per building kind** (`ownership::role_for`); a role is a hard-coded `Role` variant matched in 28 files (wage, workplace, shift, skill, labels); shifts are day, evening (`[leisure] evening_shift`) or the guards' night by id parity.

### 1.2 Proposed trades

Every trade below sits on a flow that already exists or one this pass adds, so its wage comes from revenue (or taxes), never from nowhere. **Story** is what the trade adds to a diary: a shift, coworkers, a commute, a boss. **Phase** refers to the plan.

| trade | where | revenue model (what pays) | shift | story | phase |
|---|---|---|---|---|---|
| **Packer** | Packing Plant (*new*, Vats, 2 floors) | **produce**: turns Farm Food into Rations sold to the World's Food book at `packed_premium` × the bid, or to Markets | day + night | the line, the foreman, the quota, a hand lost to the press | P4 |
| **Loader** | Freight Yard (*new*, Vats edge) | **handling**: a fee per unit on every export and import crossing (paid by the exporter, part of today's leak) | night | the dock, the dock boss, crates that go missing | P4 |
| **Driver** | Freight Yard | **handling**: the Farm → Market haul fee (today free) | day | a route, crashes (M13), hijacks | P4 |
| **Plant Operator** | Utility Plant (*new*, Vats) | **contract**: E13's `power` charges paid to the plant's owner instead of the World; the plant pays `fuel_share` to the World | day + night | the hum, the outage, a sabotage target later | P4 |
| **Builder** | Construction Yard (*new*, Vats/Mid) | **contract**: `found_cost`, refit and floor costs paid to the Yard (today they go to the Treasury, `Flow::Found`); a build takes crew-days | day | site to site; the building you put up | P4 |
| **Day Labourer** | the Labour Corner (a HangOut spot by the Yard; no building) | **one-day posts** from the Yard, the Freight Yard and Farms at harvest | dawn pick | the queue at 6:00, picked or not | P7 |
| **Street Vendor** | a pitch at a HangOut spot (self-employed) | **resale**: buys at the Market's price, sells meals and drinks at a markup | day / evening | own pitch, gang tribute (`Collect`), regulars | P7 |
| **Super** | the landlord's Blocks (1 per `blocks_per_super` 15) | **rent**: the housing niche's `labour_share` (0.4) has no role to pay today | day | knows every tenant; serves the eviction notice; a witness | P3 |
| **Night Stocker** | Market (2nd role) | food sales | night | the empty store at 3:00 | P3 |
| **Night Porter** | Capsule Hotel (none today) | beds by the night | night | the regulars who can't pay | P3 |
| **Bouncer** | Club, Den, FightPit (2nd role) | the venue's take | evening | fights at the door, the gang's man at the door | P3 |
| **Dancer** | Club (2nd role) | the venue's take | evening | the strip club life path (addendum 14) | P3 |
| **Orderly** | Clinic (2nd role) | treatments | day / night | the overdose at 2:00 | P3 |
| **Camp Warden** | Work Camp (none today) | the camp's output | day | the scandal (E37) | P3 |
| **Civic Clerk** | Civic Hall (none today) | **civic**: the Treasury | day | fines, permits, the wage desk; bribable (M16) | P5 |
| **Depot Hand** | Reserve Depot (none today) | **civic** | day | the Reserve's stock | P5 |
| **Jailer** | Precinct (2nd role) | **civic** | night | the held prisoners (L2 L21) | P5 |
| Beat cop (`Guard`) | Precinct | **civic**: `guard_count` set by the budget | day / night | patrol | P5 |
| Sanitation | Recycler | **civic**: `sanitation_count` set by the budget | day | the beat | P5 |
| **Office Worker** | Corp HQ (*new*, Spire, floors) | **overhead**: an `admin_share` of the corp's revenue; raises its competence (M15 W28) | day | the Spire commute, office politics, poaching | P8 |
| Kitchen Hand | Mission | donations (exists as Volunteer, wage 0) | day | | — |

That's 20 new trades on top of the 19 roles. Night work: Loader, Plant Operator, Night Stocker, Night Porter, Jailer and night guards. Evening: Bouncer, Dancer and the venue staff. Gig: Day Labourer. Informal: Street Vendor (plus the scavenger, who already exists). Not this pass: trades that need a need the sim doesn't model yet (a Barber or Tailor needs a status/dress need, per the brutality-and-status memory; childcare needs children with days).

### 1.3 Data-driven trades

Prefer a table to a new enum variant per trade.

- *new* `Role::Trade(TradeId)` (`TradeId(u8)`, an index into `[[trades]]`) next to the 19 bespoke roles. Bespoke behaviour (patrols, bouts, Data, stories, Parts, sweeping, burials) stays on the enum; every new trade goes in the table.
- `[[trades]]` rows (*placeholders*):

```toml
[[trades]]
id = "packer"            # stable key (saves store the key, not the index)
label = "Packer"
workplace = ["PackingPlant"]
staff_per_floor = 8      # places per floor of the workplace
wage = 6
shift = "split"          # day | evening | night | split (half day, half night by id parity)
skill = "industry"       # the skill hiring ranks on and competence reads
model = "produce"        # produce | service | handling | contract | civic | overhead | resale | day
```

- A building's staff becomes a **list of (role, places)**: `ownership::roles_for(kind) -> Vec<(Role, usize)>` (*new*; `role_for` remains its first entry). `jobs::top_up`, `wages::post`/`places`/`past_cap_pass`, the shadow archetypes and `competence::role_skill` iterate over it; `corp_brain::hunker_vacancies` follows in P4, since M16a owns `corp_brain.rs` until its phases 2-3 merge.
- `Role::ALL` becomes `world.roles()` (the 19 bespoke roles and the configured trades) wherever it iterates workers; `World::workers(role)` is keyed by `Role` as now.
- The work itself is generic: `Work` at the workplace door for the shift; producing trades accrue units through one `trades::accrue(world, b)` (the Farm/Fab pattern); service trades gate a venue's sales (no staff on duty, no sale, as the venues do now).
- Saves: `Job.role` serializes `Trade("packer")` by key; `SAVE_VERSION` 5; an unknown key on load is a hard error (no migration code, per TESTING.md).

---

## 2. Room

### 2.1 The cheapest path to ~1,300 places

| source | places (*placeholder*) | paid by | phase |
|---|---|---|---|
| today's kinds at full staff (18 Farms) + city roles | ~590 | revenue, Treasury | — |
| hire before raise (`wage_rev` ~1.0-1.2 instead of 2.0) fills the ceiling it already has | **+250-350** | the same payroll | P1 |
| second roles on existing kinds (Super, Night Stocker, Porter, Bouncer, Dancer, Orderly, Warden) | +60 | rent, sales, takes | P3 |
| civic payroll (police to ~70, sanitation to ~50, Clerks, Depot Hands, Jailers) | +90 | taxes, fines | P5 |
| Packing Plants 2 × 2 floors × 8 per floor × 2 shifts | +64 | World | P4 |
| Freight Yards 2 × 20 · Utility Plants 2 × 12 · Construction Yards 2 × 12 | +88 | handling, power, builds | P4 |
| Grow on the new Lots (Farms, Markets, NoodleBars, Fabs) | +100-150 | revenue | P2 + P4 |
| Day Labourers and Street Vendors | +40-80 (a day) | posts, resale | P7 |
| Corp HQs 3 × 3 floors × 6 | +54 | overhead | P8 |
| **total** | **~1,350-1,550** | | |

The wage bill at 1,300 × ~6.5 is ~8.5k a day. Today's wages-on bill is 6.6k. Add the re-pointed leaks (inputs and power ~0.7k), the Treasury's payroll (~1-1.5k from receipts and a hoard of 47-109k) and the extra food that employed residents buy (food bought fell from 6.7k to 3.4k a day without the dole; wages bring it back), and the money is there.

### 2.2 Lots from the free ground (P2)

`assets/gen_map.py` stamps **Lots on free cells** by zone quota (*placeholders*): Vats +40 (double-wide yards for industry), Mid +30, Civic +12, Spire +10, Sump +8, which is +100 Lots. About 127 free cells stay open ground (HangOut spots, street density, room for later). The loader doesn't change (Lots are plain `B Lot` lines). New Lots change seeding picks (`jobs::seed_lot`), so seeds' numbers move; per TESTING.md that's fine.

### 2.3 Floors (P2)

| | decision |
|---|---|
| state | *new* `Building.floors: u8` (`#[serde(default = "one")]`), drawn from the map line's optional 8th field and then built |
| what floors multiply | a workplace's places (`staff × floors` for bespoke kinds, `staff_per_floor × floors` for trades) and its seats or beds (`capacity × floors`: Homes, Hotels, venues); not its stock cap or its door |
| seeding | 1 everywhere at seed except the new industrial kinds (2) and the Corp HQs (3); Spire Blocks may seed 2-4 later, when housing is short (it isn't now) |
| building up | *new* `AddFloor`: a corp whose building is at its ceiling with revenue covering the next floor's staff, and no Lot (or cheaper than a Lot), adds a floor for `floor_cost_frac × found_cost` paid to a Construction Yard (P4) or the Treasury (before P4); capped at `[buildings.<kind>] floors_max` (Farm 3 "vertical farms", Fab 3, Packing 4, HQ 8, Home 6, venues 2); `FloorAdded` event |
| pathfinding, LOD | **no cost**: floors are logical. Agents inside a building hold no tiles (`exec/reservations.rs` has no work-tile kind) and the door is the one portal |
| drawing | the 2D app writes the floor count beside the kind letter (`F³`); the isometric view (after M18) draws `floors × storey` from the same field; rooms per floor come with the Building-interiors milestone (addendum 10), and stairs/elevators become layer portals then |

### 2.4 The map: grow later, not now

Don't grow the map in this pass. The 227 free cells plus floors give more than twice today's workplace room. A bigger map costs linearly in every flow field (49 KB and one Dijkstra per door field at 256 × 192), the district grid and the ambient street density, and it shifts every seed. Grow it when the isometric player view or M17's outside districts need new ground. When that happens: the loader is generic (`"<w> <h>"` header), `gen_map.py` takes `W, H`, extend east and north, and re-run `calibrate`.

---

## 3. The labour market

| | rule |
|---|---|
| **hire before raise** (P1) | `wages::daily`: `wage_rev` steps up **only on a shortage** (a vacancy unfilled `shortage_days`), not when `P < 0.9 P*`. A corp under its target posts places (`wages::post`, up to the ceiling) and does not raise pay. It still steps down over `1.1 P*`. In a slack market, wages sit near 1.0 and the room becomes jobs. |
| ceiling | `staff_ceiling` = `ceil(places × staff_ceiling_mult 3)` per building, where `places` counts floors and every role of the kind |
| who is hired (P1) | `pick_candidate`'s key becomes `distance − skill_w × skill(role)`, with a `rehire_bonus` for a worker laid off from the same trade in the last 30 days; still no lawfulness test except for guards; the homeless are ranked from where they stand (as now) |
| housing follows work | already there (`life.rs` "moved nearer work", `ownership::rehouse`); P1 checks that a hired homeless worker is housed within a week (probe) |
| the foreman | derived, no state: a building's longest-serving worker. Named in its hires and layoffs ("laid off by Kessler's foreman, Ines Vo") and an archetype for the shadow pass |
| coworkers | edges already form from shared time in a building (`social.rs`, `edge_create_ticks`), so a job brings contacts. The poor's known-contact HangOuts (behaviour tier: half as often with wages on) should recover with jobs, and the behaviour tier checks it |
| layoffs | unchanged (`shed_extra` past the cap first, then the newest at the wage floor) |
| one-day posts (P7) | *new* `Job.until: Option<Tick>`: a day post ends at its shift's end; filled at dawn from adults at the Labour Corner (presence, not distance) |

---

## 4. Immigration that answers the city (P6)

Today: `immigrants_this_week = immigration_per_week 13 × logistic(Street happiness)` = 3-5 a week, and it reads neither jobs nor homes.

**The rule (Harris-Todaro: migrants come while the city's expected wage beats what they'd earn outside):**

```
E      = gross_mean_wage × employed ÷ free adults        (7-day means)
R      = [demography] outside_wage                       (a World lever; M17's outside moves it)
pull   = clamp((E − R) ÷ R, 0, pull_cap)
beds   = clamp(empty beds in standing Blocks ÷ (beds_ref), 0.25, 1)
week   = round(migrants_max × pull ÷ pull_cap × beds × logistic(Street happiness))
offers = vacancies open ≥ offer_days (each one draws a recruited migrant, ≤ offers_max a week)
```

| | decision (*placeholders*) |
|---|---|
| keys | `outside_wage` 3.5, `pull_cap` 1.0, `migrants_max` 20 a week, `beds_ref` 60, `offer_days` 3, `offers_max` 6 a week; `immigration_per_week` retired |
| who comes | working-age adults (18-45), Skills drawn like the seeded residents (not a flat 0.1), a trade skill matching the offer; savings by `vf_arrival` (the tier mean, already built); housed in the emptiest Block **nearest the job** for an offer; `p_spouse` 0.2 brings a spouse (no children) |
| the offer | a recruited migrant arrives holding the vacancy (`Immigration` "arrived for a Packer's job at Nutrix Packing"); the vacancy closes at spawn |
| emigration | unchanged and hard: mood or Dreg misery, plus the 20-coin fare (`can_buy_passage`) |
| the equilibrium | inflow = births + migrants; outflow = deaths + emigrants. A city that kills its workers opens posts and raises `E` (fewer applicants per place), which raises `pull` and the offers: the inflow answers the deaths. A city with a jobless crowd has a low `E` and draws few. The steady state is an employment rate `e* ≈ R ÷ wage`: about 0.6-0.7 at wage 6 and R 3.5-4, **a permanent jobless underclass of 30-40 %, by structure** (brutal and stable). The year's shrink needs ~1-1.6 migrants a day net of births (7-11 a week) at today's violent death rate; the rule allows up to 20 + 6 |
| damping | the means are 7-day, the weekly count is capped and offers are capped, so an emptying city can't overshoot past `POP_MAX` (2,667) inside a month |

---

## 5. The Treasury's outflow: folded in (P5)

**Fold it in now.** The Treasury is a black hole: 47-109k at a tax rate of 0, filling from Vagrancy fines, customs and the property rate (ECONOMY_V2 § 0 warned what a hoarding Treasury does). Dylan wants it to pay admin and police. That's the sink, and it adds ~90 places.

| | rule |
|---|---|
| the budget | `civic_payroll_target = civic_share × trailing 14-day receipts (tax, fines, customs, property) + max(0, Treasury − band_hi) ÷ payout_days` (*placeholders*: `civic_share` 0.8, `payout_days` 60) |
| allocation (priority order) | police (`levers.guard_count`, up to `police_per_1000` 35 × population ÷ 1,000), then Jailers (1 per 25 held), Sanitation (`levers.sanitation_count`, up to 50), Civic Clerks (up to 8), Depot Hands (up to 6) |
| interface | `treasury::daily` writes `levers.guard_count` and `levers.sanitation_count`; `law::reconcile_guards` and `districts::reconcile_sanitation` hire as they do now (**no `law.rs` edit**: M16a phase 3 owns it). A god's `SetGuardCount`/`SetSanitation` pins (`EconState.civic_pinned`; the budget may write past the command's 60 clamp, `u8`) |
| the tax band | reads the balance after payroll: the rate comes back off 0 once the payroll draws the hoard down |
| not a dole | these are posts with duties (patrol, sweep, clerk, guard the held), paid from taxes; ECONOMY_V2's public works were sized by surplus with no service attached and stay gone |
| the story | Vagrancy fines from the poorest pay the cops who fine them. Keep it, and print it (`flow_fines`, `civic_payroll`) |

---

## 6. Throughput

| finding | action |
|---|---|
| wages-on: think ×2.2, law ×5.8, from ~950 homeless who starve, steal and get arrested (Theft ×13, Starving ×16, Vagrancy ×60) | **the jobs are the fix**; no optimisation in this pass. P9 re-measures with the same per-system timer |
| `ownership::staff_by_building` is called per corp and per building in `wages::post`, `places`, `past_cap_pass`, `employees_of` and `hunker_vacancies` (corp_brain + ownership ×10 at midnight) | P1: *new* `World::employer_index` (`BTreeMap<EntityId, Vec<EntityId>>` built once per midnight pass, or kept by `index_job`/`unindex_job`) |
| if, after P1-P6, homeless stay > 300 and seed 42 is under the 4,000 floor | profile `law::sightings` and `law_brain::run` (guards × open suspects) together with M16a phase 3's owner |

---

## 7. Acceptance (TESTING.md tiers)

Asserts catch broken; behaviour judges alive; numbers are reports. No calibration bands.

### Core tier, run with wages + no net on (the shipped config once flipped)

- `core_sanity` 42-44 × 120 days: the coin identity every day; the existing collapse bounds; the ticks/s floor 4,000 on seed 42.
- `core_year` 42 × 365: the collapse bounds over every 30-day window. **This is the gate the pass exists for: population 1,333..=2,667 in every window with wages + no net on.**
- **Two new collapse bounds** (Dylan's words, broken rather than brutal; overturnable rows J24/J25): from day 30, **employed ≥ 50 % of free adults** (adults not jailed) every day, and **homeless ≤ 25 % of adults** every day. Both fail today's wages city (36 %, ~50 %).
- New mechanism-existence bullets (one count ≥ 1 over 42-44; measure on 42-47 first, and if a mechanism fires on ≤ 2 of 6 seeds it gets a unit test instead):
  - a Trade hire (`Hire` with a `Trade` role);
  - a `FloorAdded`;
  - a freight handling fee (`flow_handling > 0`);
  - power bought locally (`flow_power_local > 0`);
  - a build paid to a Construction Yard;
  - a migrant arriving with an offer;
  - a civic-budget guard hire;
  - a day post filled;
  - a Street Vendor sale.
- Unit tests per mechanism, in seeded worlds: the hire-before-raise rule (a slack market never raises `wage_rev`; a shortage does), `roles_for` staffing with two roles and floors, the Harris-Todaro count (pull 0 posts no week; offers capped), the civic allocation order and the pin, each revenue model's conservation (identity over a day), `AddFloor`'s cost and cap, and a save round trip with a `Trade` role.

### Behaviour tier

- `shadow --assert` on the flipped city: re-measure all 146 bounds and tighten where jobs improved them, in the same change, with old and new values in the commit.
- *new* archetypes (5 picks each): Packer, Loader, Super, Civic Clerk, Bouncer, Day Labourer, Street Vendor. Metrics: `worked`, `paid/wkd`, `waged_wd`, `walk_h/d`, `known/wk`.
- **Known contacts met**: the poor archetypes (homeless, gang member, runner) at ≥ 0.5 × the default city's value (today's wages city halves them).
- **The employed share among picked civilians** (a new printed metric, asserted ≥ 0.5 over the civilian archetypes).

### Person probes (`tests/person.rs`)

- **laid off, finds work**: a Farm worker laid off at midnight (with ≥ 3 posts open city-wide) holds a Job within 14 days;
- **a trade keeps its shift**: a Night Porter on shift is at the Hotel at 02:00 and paid by 08:00; a Loader works a night shift and sleeps the next morning (longest sleep block ≥ 5 h);
- **the offer is real**: a migrant spawned with an offer is at that workplace for its first shift within 2 days;
- **the homeless hire is housed**: a homeless adult hired at a Packing Plant sleeps in a Block within 7 days;
- **the Super walks the Blocks**: a Super visits ≥ 3 of his Blocks a workday.

### God scenarios

- *new* `god_kill_workers` (day 45: kill 200 employed adults): migrants with offers arrive within 14 days, and employment is back to ≥ 90 % of the control's by day 60.
- *new* `god_outside_boom` (lever `outside_wage` × 0.5 on day 20): immigration rises against the control.
- Keep `god_bankrupt_city` (the civic payroll stops, the police shrink, and the city still runs).

---

## 8. When to flip `wages` and `no_safety_net`

**Together, once the year holds.** The dole-on, wages-on city is gone (the stop rule; HANDOFF). `no_safety_net` requires `wages`, and a wages-on city with the dole is a measuring device, not the game.

Flip in `assets/config.toml` when all of these pass on the wages + no-net city:

1. **wave 2 has merged** (P1 hire-before-raise and the employer index, P2 Lots and floors, P3 trades on existing kinds, P5 civic payroll, P6 immigration);
2. `core_year` holds the population bound in every window, and `core_sanity` is green on 42-44 with the two new bounds;
3. the behaviour tier passes (re-measured) and the person probes pass;
4. seed 42 is ≥ 4,000 ticks/s in `core_sanity`.

The same change keeps the switches. The next change retires them and their off branches (TESTING.md: switches stay only for work in flight). If wave 2 doesn't hold the year, wave 3 (the industries) goes first and the flip waits; nothing is retuned to pass. M16a phases 4-5 and the god scenarios then run on the flipped city.

---

## 9. Not this pass

| item | why later |
|---|---|
| map growth | §2.4: room exists; the cost lands with the player view or M17 |
| rooms per floor, stairs, elevators, sunlight | the Building-interiors milestone (addendum 10) |
| a status/dress need (Barbers, Tailors) and childcare | needs that don't exist yet |
| Corp HQ competence beyond a flat bonus | M15's competence formula is the hook; P8 adds one term |
| outside contracts as job sources (M17), Daemons on Virt nodes | their milestones |
| cargo theft, plant sabotage, strikes at the new kinds | hooks only (events named); missions in M16b |

---

## Open questions for Dylan

1. **A permanent jobless underclass of ~30-40 %** (the Harris-Todaro equilibrium) is realistic and brutal. Is that the city you want, or should the target be nearer full employment (raise `outside_wage`, more places)?
2. **Two new collapse bounds** (employed ≥ 50 % of free adults; homeless ≤ 25 %). They're asserts of *broken*, not calibration bands, but they're new numbers. OK?
3. **The civic payroll is funded by Vagrancy fines** (the poorest pay for the police who fine them). Keep it as the story, or cap fines' share of the budget?
4. **Corp HQs (Spire office work, P8)**: build them in this pass for the Corp-class commute and poaching story, or defer until the Building-interiors milestone?
5. **Immigrant families**: a spouse only (proposed), or children too (more mouths, more story, more camp intakes)?
