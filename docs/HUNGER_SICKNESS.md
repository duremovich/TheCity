# Hunger as sickness: cheap synth, slow disease, the bins

2026-10-10. Plan: [docs/plans/hunger-sickness.md](plans/hunger-sickness.md). Memory: `thecity-hunger-as-sickness.md`.

Dylan: "In Cyberpunk 2077, few people actually starve because synthetic food is so cheap. The problem is malnutrition, toxicity, and cancer. So maybe instead of starving to death, the poorest get debuffs and disease. Another lever for food for those who can't afford anything is dumpster diving."

**What changes for a person.** Someone with one coin eats: a synth meal, a third the price of a real one, toxic. Someone with no coins eats too: Mission synth, or scraps from the bin behind the Market (if the regular or the gang lets them). Nobody dies on day 3 of an empty pantry. What the poor get instead is a body that wears out. They are malnourished and tired, earn less, lose shifts, feel worse, have fewer children, and some get a cancer that kills them over two seasons unless they find the coins for a Clinic. Starvation death stays possible, but only after a week of literally nothing.

**Why.** On the flipped config (wages + no_safety_net) the never-hired starve: 145-205 in 120 days (flip readiness, 7c99ffc). That is a broken start, not a brutal city. The recalibration diaries showed that part of it is the Full planner itself. Starving people gamble and pay for beds, never seek a Mission, steal from Homes while they can still buy, and give up after one failed Beg. Calibrating against those gaps spreads them city-wide, so they get fixed here first (§ 6).

## Decisions (Dylan, binding)

| # | Decision |
|---|---|
| H1 | Quality is a per-meal choice on the one `Good::Food`: **synth** (near-free, toxic) or **real** (dear). No second `Good`. |
| H2 | Starvation death is a rare edge case: a long stretch of literally zero food (no synth affordable, no bin, no Mission). |
| H3 | Disease is two accumulating meters, **malnutrition** (hunger, poor diet) and **toxicity** (synth, scraps), with debuffs to energy, work output, mood, health and fertility. On top of them sits a slow **cancer** rolled from high toxicity. It kills over months unless a Clinic treats it, which makes it a money sink and a story hook. |
| H4 | **Dumpster diving**: free food in bins behind Markets, eateries/venues and Homes, contested by the homeless and the gangs. |
| H5 | Fix the Full-planner gaps (§ 6) inside this work. The Mission serves synth. |
| H6 | Both LOD tiers. The calibrate table learns the new behaviours. Everything that changes the default city reads behind a switch. Gates assert broken-system signals, and calibration bands are printed findings. Emergent story first. |

## 1. Synth and real (H1)

The shelf still holds units of `Good::Food`. At the counter, a buyer takes each unit one of two ways:

- **real**: one unit becomes one meal at the Market's price `P` (`World::price_for`, unchanged).
- **synth**: the unit is cut with vat filler into `synth_stretch` meals. Each meal costs `synth_price = max(1, round(P / synth_stretch))`. The shelf drains `1 / synth_stretch` of a unit per meal, through a per-Market accumulator (`Market.synth_frac`).

Per shelf unit the corp's revenue is roughly the same either way, so the Food corps and the World market books keep their calibration. The poor get about 3× the calories per coin, and pay for it in toxicity. Food units are not in the coin identity: the filler is the Market's.

- **Carried and stored**: `Inventory.synth` and `Building.synth_food` are subsets of `food` / `stock_food`, kept like `stolen_food`. Synth does not spoil (`daily_spoilage` skips the subset). An eater takes **real first**.
- **The choice** (`diet::quality(world, id, P)`, one function for both tiers): buy **real** when `coins ≥ real_floor_meals × P`, or when malnutrition ≥ `mal_seek_real` and `coins ≥ 2P`. Otherwise buy synth. `buy_quantity` reads the chosen price. `life::meal_price` (the leisure and Hotel reserve, the Mission's "cannot buy" test, Earn's savings line) becomes `diet::meal_price`: the price this agent would actually pay.
- **Mission meals are synth** (H5). A unit off the Mission's shelf serves `synth_stretch` meals (`Charity.synth_frac`), so the same larder feeds three times the people. The ledger-only `Alms` line is per meal at `meal_cost / synth_stretch`.
- **EatOut** at a NoodleBar or venue counts as a **real** meal (the treat).

Hunger satisfaction is the same for both (`food_satisfy`). The difference is in the meters.

## 2. The meters (H3)

There is a new component, `Health`, on every citizen:

```rust
pub struct Health {
    pub malnutrition: f32,      // 0..=1
    pub toxicity: f32,          // 0..=1
    pub cancer: Option<Cancer>, // { stage: f32 0..=1, since: Tick, diagnosed: bool }
    pub sick_today: bool,       // rolled at midnight
    pub today: MealTally,       // { real, synth, dive, alms: u8, hungry_hours: u8 }; reset at midnight
}
```

**Both tiers keep the tally the same way.** Every eat site calls `health::ate(world, id, MealKind)`. On the Full/Coarse tier these are EatFromInventory, EatAtHome, EatOut, EatAlms and DumpsterDive. On the Statistical tier they are `stat_eat`, `charity::stat_daily` and `stat_dive`. `hungry_hours` counts the hourly marks with `hunger < mal_hungry_edge`: on bodies in the hourly block of `needs::run`, and on Statistical agents in `run_statistical` after the hour's decay. **`health::daily` runs at midnight over every citizen, whatever its LOD.** Meters, cancer and sick days are therefore identical across tiers by construction. Only *what the agent ate* differs, and that is the parity question (§ 5).

The daily update (`s = synth + alms + dive` meals, `n = s + real`):

- `malnutrition += mal_hungry × hungry_hours / 24 + mal_poor_diet × (s / max(n, 1)) − mal_real × min(real, 2)`, clamped to 0..=1.
- `toxicity += tox_synth × (synth + alms) + tox_dive × dive − tox_clear_frac × toxicity`, clamped to 0..=1 (after P5, Dylan: a share is cleared, not a flat amount, so every diet has a resting level).

**The resting level, worked through.** With S synth meals a day the steady level solves `tox_synth × S = tox_clear_frac × t*`, so `t* = (tox_synth / tox_clear_frac) × S`. The sim's day of food is about one meal (`food_satisfy` 0.5 against roughly 0.42-0.5 of hunger lost a day; measured at 0.9-1.0 meals per adult-day). So all-synth means S = 1, and the target t* ≈ 0.8 gives `tox_synth / tox_clear_frac = 0.8`. "One synth meal a day" is one of a person's three meals, S = 1/3, giving t* = 0.27 ≈ 0.3. Real food gives S = 0, so t* → 0. The rate is set by the timescale: `tox_clear_frac` = 0.03 (a 23-day half-life toward a cleaner diet), so `tox_synth` = 0.024 and `tox_dive` = 0.048 (scraps count twice).

**Debuffs** are read through one function, `health::effects(world, id) -> Effects`. Every reader is a function both tiers already call. Write `m' = max(0, m − mal_onset) / (1 − mal_onset)`, `t' = max(0, t − tox_onset) / (1 − tox_onset)`, and `c` = the cancer stage (0 without one).

| debuff | formula | read where (both tiers) |
|---|---|---|
| energy | decay × (1 + `mal_energy`·m' + `tox_energy`·t' + `cancer_energy`·c) | `DecayCtx.energy_mult`, composed with the stims multiplier |
| work output | × (1 − `mal_work`·m' − `cancer_work`·c), floor 0.3 | `accrue_farm_work`, `accrue_fab_work`, `scavenge_p` |
| sick day | a workday at midnight is sick with p = `sick_mal`·m' + c; no shift, no wage | the Work goal (Full) and `stat_work` both read `sick_today` |
| mood | bias − (`mal_mood`·m + `tox_mood`·t + `cancer_mood`·c) | `mood::update_biased` (the litter bias's slot) |
| health | old-age hazard × (1 + `frail_old`·(m + t)); fight term − `frail_fight`·(m' + c) | `demography::old_age`, `law::resolve_fight` |
| fertility | no conception at m ≥ `fertile_mal_max` or with a cancer; p × (1 − t) otherwise | `demography::births` |

## 3. Cancer and the Clinic (H3)

- **Onset** (Dylan, after P5): at midnight, p = `cancer_hazard × toxicity × age_factor`, with `age_factor = 1 + max(0, age − cancer_age_from) / cancer_age_span` (30 and 10 sim years), rolled as a pure hash. The hazard is stationary. Two earlier models were dropped: a threshold at 0.6 that nobody reached, and a dose term that compounded roughly as t⁴.
- **History** (world creation and every arrival, `health::seed_agent`, pure hashes, so the same at both tiers):
  - Each adult's toxicity is the steady level of its likely diet, approached with age by `1 − exp(−(age − 16) / tox_seed_years)`, with a ±15 % hash jitter.
  - The likely synth share is 1 for a Dreg (plus a tenth of a day in bin meals). For the housed it is `1 − 0.9 × min(1, coins / 14 meals)`, scaled ×1 in the Sump, ×0.7 in Mid, and capped at 0.2 in the Spire.
  - A Dreg's malnutrition is the synth cap, approached the same way.
  - A silent early cancer is seeded on `hazard × (cancer_symptom / cancer_growth)` (the prevalence of a cancer still under the symptom), with its stage uniform below the symptom.
- **The paper estimate** (before any run): on the flipped config the seeded distribution gives E = Σ toxicity × age_factor ≈ 1,420-1,460 over the 2,000 adults (seeds 42-44). The mean seeded toxicity by class is Dreg 0.81-0.84, Sump 0.45, Mid 0.22-0.23, Spire 0.025. Expected onsets per sim year (120 days) are `120 × cancer_hazard × E`. For the printed 20-40 (Dylan), 30 gives `cancer_hazard = 30 / (120 × 1,440) ≈ 0.00017`, chosen once. The predicted deaths per sim year roughly equal the onsets, since onset to death takes about 67 days and few patients can pay 240; the first year runs lower because of that lag.
- **Measured** (flipped core_year, seed 42, the P5f table):

  | days | onsets | diagnosed | cured | Disease deaths |
  |---|---|---|---|---|
  | 0-119 | 42 | 40 | 2 | 19 |
  | 120-239 | 22 | 24 | 4 | 29 |
  | 240-359 | 16 | 21 | 4 | 11 |

  Mean toxicity falls through the run as the city's real diets settle below the seeded ones. Sump 0.45 → 0.17, Mid 0.22 → 0.16, Dreg 0.82 → 0.33. The Spire rises 0.025 → 0.17, because its wallets thin once wages flow from revenue.
- **Course**: `stage += cancer_growth` a day. At `stage ≥ cancer_symptom` it becomes **diagnosed**: a `Diagnosed` event, a `MemoryKind::Diagnosed` (salience 0.9, valence −0.8), a diary line and gossip about the person. At `stage ≥ 1` the agent dies of `DeathCause::Disease`.
- **Treatment**: a new `Oncology` step under the existing **Treat** goal, at a Clinic (`nearest_seller(Clinic)`). Its price is `cancer_treat_price`, paid as `Flow::Treatment` to the Clinic's owner (a money sink, and Clinic revenue). Below `cure_stage` the treatment cures: the cancer is removed and toxicity × `treat_tox_mult`. At or above it the patient goes into remission, `stage −= remission`. Treat's utility for a diagnosed patient is U(stage); the price gates it. The Earn goal's savings line is raised to the price while the patient is diagnosed and untreated. Saving for treatment is the hook: it pushes people to work, beg, steal, join a gang or take contracts. Off screen, a diagnosed Statistical patient with the coins treats on `p_stat_treat` of days, the `p_stat_detox` pattern: too rare for `calibrate` to learn.

## 4. The bins (H4)

`Building.bin: u16` holds meals of scraps (cap `bin_cap`). Each midnight, after `daily_spoilage`, the bins fill:

- **Market**: the units spoiled today × `bin_meals_per_unit`.
- **NoodleBar and the eating venues**: `venue_scrap` meals per meal served today (an accumulator).
- **Home**: the pantry's spoiled units × `bin_meals_per_unit` (household waste: the "Homes (litter)" source).
- Then every bin rots: `bin -= floor(bin × bin_rot)`.

A dive is the new plannable action **`DumpsterDive`**: 20 minutes at the bin's door, the new `LocationKey::Bin` bound to the nearest stocked bin within `dive_reach_tiles`. Lookups go through a `World::bins` index (bins with stock), never a scan. It takes one meal of scraps and eats it at once (scraps aren't carried). It satisfies `food_satisfy × dive_satisfy`, tallies a `dive` meal, rolls food poisoning (`dive_sick_p`: energy − 0.2), and drops litter at the door (2 r0). Its plan cost is `5 + 8 × pride − 3 if starving`. It is cheaper than Beg for the proud, dearer than buying for anyone who can. **Contest** (one function, `bins::contest`, which both tiers call before the meal):

1. **Reservation**: one diver per bin at a time (`exec::reservations`). The Statistical tier holds the bin for the hour.
2. **The regular**: a bin keeps `bin_regular: Option<(EntityId, u8)>`, a diver with dives there on `regular_days` of the last 7. A newcomer meets the regular when the regular is within 3 tiles (Full) or is a Statistical diver who dove there today (the off-screen reading). That is an **Intimidate** via `moves::resolve`. The loser goes without and gets a `BinContest` event; the pair's affinity drifts `−0.1`, and the edge turns Rival on a second loss.
3. **The gang**: in a district a gang controls at ≥ `bin_gang_control`, a non-member is shaken down with p = `bin_shake_p × control`. With a member within sight (Full) or any member on the Statistical tier, the diver pays `bin_toll` coins if it has them. Otherwise it is a light Assault through the existing extortion/assault path, at most once a day per diver. Gang members dive free.

**Statistical**: in `stat_eat`, after inventory, pantry and buy, an adult who cannot afford `synth_price` dives on its row's `p_dive` (as `p_scavenge`: phase × lawfulness × hunger). It uses the same bin choice and the same `bins::contest`, before the `p_desperate` theft. The noon Mission (`charity::stat_daily`) is unchanged.

## 5. LOD and the table (H6)

- The meters, cancer, sick days and debuffs run on the same daily and hourly hooks on both tiers (§ 2), so parity is structural.
- The diet choice (`diet::quality`), the bin pick and the contest are shared functions.
- The table learns **`p_dive: Option<[f32; STAT_ROWS]>`**. It is the share of Full agent-hours, for adults who cannot afford a synth meal with a bin in reach, in which a DumpsterDive begins. It sits next to `p_scavenge`, `p_beg` and `p_desperate`, with serde default, so the table stays `version = 2`. With `[sickness]` off, `calibrate` writes no `p_dive`. `p_desperate` is re-learned on the fixed planner: Full thieves who could afford synth no longer exist (§ 6, G3).
- The parity test (`lod.rs::test_full_vs_statistical_within_15pct`) adds three diet rates: synth share of meals, dive meals per agent-day, and mean malnutrition on the last day.

## 6. The Full-planner gaps (H5)

These are base-city bugs, so they get their own switch, `[life] hunger_fixes` (the `violence_fixes` pattern: sub-switches plus `CITYSIM_HFIX_OFF=1..4`). Each one is reproduced first by a person probe that fails on main.

| # | Gap (recalibration diaries) | Likely cause in code | Fix |
|---|---|---|---|
| G1 `hf_food_first` | starving people gamble and pay for beds instead of eating | the reserve (`hotel_reserve_meals × meal_price`) protects coins, not order: Unwind/Sleep(CheckIn) outscore Eat when the Eat plan is far or energy-braked | while `hunger < starving_hunger` and Eat is feasible, Unwind, Shop, GetHigh, Donate and CheckIn are not scored (eat first); the reserve reads `diet::meal_price` |
| G2 `hf_mission` | never seek a Mission | Eat's "can afford/obtain" gate (`goals.rs`) is `food ∨ coins ≥ P ∨ pantry ∨ lawfulness < 0.3`: a lawful broke agent scores Eat 0, so `alms_plan` is never reached | the gate also opens on a stocked Mission within `charity.reach_tiles` (with `hf_mission`, × `hungry_reach_mult` while starving) and, with `[sickness] bins`, on a stocked bin in reach |
| G3 `hf_buy_not_steal` | steal from Homes while able to buy | `StealFood(Home)` cost `7 + 20·law − 4 starving − 3 dark − 4·stealth` beats `GoTo(Market) + BuyFood` for the lawless | `steal_mods` adds `steal_can_buy` (+12) when `coins ≥ diet::meal_price`, for every StealFood source |
| G4 `hf_beg_retry` | a failed Beg is not retried | a Beg with no giver is `Failed(PreconditionLost)` → `goal_failed`, and a second failure cools Earn | a Beg with no giver is a dry hour (`Done`, as Scavenge's): `beg_dry_max` in a row cool Earn for `beg_cool_hours`; the `Rejected` memory only when a known passer refuses |

## 7. Starvation (H2)

With `[sickness]` on, `starvation_grace_ticks` reads `starve_grace_days × 1440` (7 days instead of 3), and malnutrition climbs about `mal_hungry` a day while starving. `Starving` and its memory are unchanged. A death by starvation now means synth, bins and Missions all failed someone for a week, which makes it a broken-floor signal (§ 9).

## 8. Config (`[sickness]`, starting values; every key read only with `enabled`)

```toml
[sickness]
enabled = false            # master (H6); with it off the city is byte-identical
diet = true                # § 1-2: synth/real, the meters, debuffs
bins = true                # § 4
cancer = true              # § 3
synth_stretch = 3          # meals per shelf unit cut with filler (synth meal ~1 coin at P 3-4)
real_floor_meals = 10      # buy real at coins >= this many real meals
mal_seek_real = 0.5        # ... or when this malnourished with 2 real meals' coins
mal_hungry_edge = 0.25     # an hour below this hunger is a hungry hour
mal_hungry = 0.08          # malnutrition per fully hungry day (~12 days to 1.0)
mal_poor_diet = 0.012      # per day at an all-synth diet (~25 days to the onset)
mal_real = 0.02            # off per real meal (max 2 a day)
mal_onset = 0.3
tox_synth = 0.024          # per synth or Mission meal (after P5: t* = 0.8 x S)
tox_dive = 0.048           # per bin meal
tox_clear_frac = 0.03      # the share cleared a day
tox_seed_years = 8.0
tox_seed_jitter = 0.3
tox_onset = 0.4
mal_energy = 0.5
tox_energy = 0.3
cancer_energy = 1.0
mal_work = 0.4
cancer_work = 0.5
sick_mal = 0.3
mal_mood = 0.2
tox_mood = 0.15
cancer_mood = 0.3
frail_old = 2.0
frail_fight = 0.2
fertile_mal_max = 0.5
cancer_hazard = 0.00017    # p a day = this x toxicity x age factor (after P5; the paper estimate in § 3)
cancer_age_from = 30.0
cancer_age_span = 10.0
cancer_growth = 0.015      # stage a day: ~67 days from onset to death (two seasons)
cancer_symptom = 0.25
cancer_treat_price = 240   # ~30-40 days of a wage: the sink
cure_stage = 0.6
remission = 0.4
treat_tox_mult = 0.5
p_stat_treat = 0.2
bin_cap = 40
bin_meals_per_unit = 2
venue_scrap = 0.3
bin_rot = 0.5
dive_reach_tiles = 40
dive_satisfy = 0.6
dive_sick_p = 0.05
regular_days = 3
bin_turf_tiles = 15
bin_shake_p = 0.3
bin_toll = 1
starve_grace_days = 7
```

`[life]` gains `hunger_fixes = true`, `hf_food_first`, `hf_mission`, `hf_buy_not_steal`, `hf_beg_retry` (all true), `hungry_reach_mult = 2.0`, `steal_can_buy = 12.0`, `beg_dry_max = 3` and `beg_cool_hours = 1`. `v1_profile` pins `[sickness] enabled` off. `calibration_city` follows the shipped value, as it does for the wages switches.

## 9. Events, diaries, columns, gates

- **Events** (appended): `Diagnosed [id]`, `Dove [id, bin]` (counted only, not logged per dive in the events TSV), `BinContest [winner, loser, bin]`, `Cured [id, clinic]` (and `Treated` reused for remission). Deaths: `DeathCause::Disease` ("died of vat-cancer"). Memories: `Diagnosed`, `Dove` (salience 0.2, valence −0.2 × pride).
- **Diaries** (shadow): a header line `Health: malnutrition 0.42 | toxicity 0.61 | cancer stage 0.30 (diagnosed day 51) | sick days 4/7`. Each meal is logged with its kind and price ("ate synth (1c)", "dove the bin behind Market #3", "lost the bin to Rafa"). Picks get a `sick` archetype (malnutrition ≥ 0.5 or a cancer), diaries only, no bounds at first. Behaviour metrics: `starve_h/wk` (hours at hunger < `starving_hunger`), `synth%`, `dive/wk`.
- **Columns** (`HealthCols`, appended before `ticks_per_sec`): `meals_real, meals_synth, meals_dive, meals_alms, malnourished, toxic, mean_malnutrition, mean_toxicity, sick_days, cancer_new, cancer_diagnosed, cancer_cured, deaths_disease, bin_meals_in, bin_contests, bin_shakedowns`. `tools/analyze_run.py` gets a "health" section that prints them.
- **Gates (doctrine)**. Asserted: coin identity with treatment and tolls; determinism; save round-trip; mechanism bullets in `core_sanity` with sickness on (a synth meal, a dive, a malnourished agent, a `BinContest`). Diagnosis, cure and Disease death are rare in 120 days, so each gets a unit test in a seeded world. Printed findings: the starvation, Disease-death and malnourished counts, cancer per year, synth share. The judge-by numbers (starvation near zero, sickness visible) are read from the reports and the diaries, not asserted. Whether to tighten the 200 starvation collapse bound is an open question.

## 10. Save

`SAVE_VERSION` 6 → 7. The new pieces are `Health` (component store), `Inventory.synth`, `Building.synth_food`, `bin` and `bin_regular`, `Market.synth_frac`, `Charity.synth_frac`, `DeathCause::Disease`, the new `EventKind`s and `MemoryKind`s, and `StatTable.p_dive`. All of them are serde-default and `skip_serializing_if` default, so a v6 save loads (meters 0, empty bins), and **the switch-off city saves byte for byte as today**.

## 11. Out of scope

A second food good or a World book for synth; children's meters (they eat from the pantry: child health is an open question); contagion and epidemics; toxicity from industry or pollution; a toxicity cleanse or chronic-care service; insurance and Trauma Team; households or gangs pooling coins for a member's treatment (an open question); Street Vendors (Jobs J29); the organic Hit market; cyberware organs.

## Open questions for Dylan

1. **On in the dole city too, or only with the flip?** Recommended: on with the flip (`wages` + `no_safety_net` + `[sickness]` together), one recalibration. In the dole city it would also cut its 10-15 starvation deaths a run.
2. **The synth stretch model.** One shelf unit becomes 3 synth meals at a third of the price each, so revenue per unit is neutral. Is this the right reading of "per-meal choice on the one good"? Side effect: when the poor eat synth, the city's effective food supply grows. The Winter famine thefts will likely fall; whether brutality needs a new source of pressure is a separate call.
3. **Treatment price 240** (about a month or more of wages), the patient's wallet only. Should a spouse, kin or the gang pay (a strong story hook, but more code)?
4. **Sick days.** Lost wages only, or should repeated sick days cost the job (a new dismissal rule)?
5. **Starvation bound.** With sickness on, should `core_sanity`'s starvation collapse bound tighten from 200 to ~20 per 120 days? That treats a broken free-food floor as a broken city.
6. **Children.** Should children's meters (pantry quality) be in v1?
7. **`hunger_fixes` on by default at merge** (the `violence_fixes` precedent)? It changes the default city and needs a `calibrate` before the flip. Recommended: yes.
8. **Bin contests.** Intimidate plus a gang shakedown or light Assault, or may a bin fight escalate to a killing?
9. **Cancer volume.** The starting keys aim at roughly 20-40 Disease deaths a year at 2,000 residents (violent deaths are ~115-160 per 120 days). More, or less?

## Decisions (Dylan, 2026-10-10)

1. `[sickness]` turns on with the flip (one recalibration); off in the dole city until then.
2. The stretch model is confirmed: 1 shelf unit = `synth_stretch` 3 synth meals at a third of the price.
3. Treatment is paid by the patient's wallet first, then a spouse/kin pool, then the gang for its own members.
4. Sick days cost the wage, and a streak costs the job (starting value: 5 sick days in 14 = fired).
5. With sickness on, `core_sanity`'s starvation collapse bound tightens from 200 to 20 per 120 days.
6. Children carry the malnutrition meter in v1 (debuffs only); no toxicity cancer roll for children.
7. `[life] hunger_fixes` is on by default at merge, as `violence_fixes` was.
8. A bin fight is intimidation plus a light assault, and it can escalate through the normal combat rolls, so a death is possible but rare.
9. 20-40 cancer deaths a year at 2,000 is a printed target, not an assert.

## Implemented: deviations (P1-P5, 2026-10-10)

Commits on `worktree-agent-a74ec841dce33aa73`:
- P1 `3822f66`
- P2 `0a3447b`
- the coordinator's calls `f5acf34`
- P3 `1f46fbd`
- P4 `92809f5`
- P5a `4547cf6`
- P4b `514c162`
- P5b `20bc867`
- P5c `5086ba6`
- P5d `88c5339`

Each commit message holds its numbers.

**The model:**
- **Synth malnutrition levels off** at `synth_malnut_cap` 0.45 (coordinator call). Hunger hours and bin scraps push past it; real meals heal. Children follow the same rule.
- **The real-food gate** is `real_meal_reserve` 3 real meals' coins, replacing `real_floor_meals` 10, which kept the working class on synth.
- **The Full-planner gaps (§ 6), as built:**
  - G1 also gates Socialise (the paid Drink), and gates the starving whatever they hold. A meal short, the coins are kept for the meal.
  - G2 also makes Earn wait while a Mission (or a bin) would feed the agent.
  - G4 is pinned by a mechanism-level probe: begging is too rare in the shipped city for a behavioural one.
- **The bins:**
  - The dive is a scripted plan (`[GoTo(Bin), DumpsterDive]`, the Mission's pattern), not a searched action. `LocationKey::Bin` is scripted only. The spec's plan cost (5 + 8 × pride) became a gate: a diver goes under hunger 0.4 − 0.3 × pride.
  - **The gang's turf** is a gang-held Home within `bin_turf_tiles` 15, else a district the gang controls. Districts are almost never a gang's, so `bin_gang_control` never fired; it was replaced. The toll goes to the gang as `Flow::Tribute`.
  - **The supply (stop rule, P5a):** spoilage alone left 0-5 stocked bins city-wide. Now:
    - a Market sale leaves `market_waste` 0.2 scrap meals a unit;
    - a pantry meal leaves `home_scrap` 0.2;
    - a Home carries its spoilage fraction (`spoil_carry`);
    - the off-screen diver reaches from its Home door, a squatter from its squat's door (P5b);
    - the starving reach `[life] hungry_reach_mult` farther (P5b).
- **The cancer's treatment** is a scripted Treat plan (`[GoTo(Clinic), Oncology]`). The pool is the wallet, then the spouse and kin, then the gang (decision 3). `life::broke` reads `health::saving_short`, so a diagnosed patient short of the price keeps Earn alive.
- **Sick days** cost the wage, and the 5th in 14 costs the job (decision 4, `sick_fire_days`).

**The rungs (P4b, Dylan on J25):**
- `[street] shanties` (in flight, on with the flip). While the rough sleepers outnumber the free squat slots, up to 3 vacant Lots a night go up as shanties: derelict Lots of 24 slots, at most 20 standing at once. A builder clears a shanty, which puts its squatters out.
- A Hotel booked full adds a floor of beds, paid by its owner, up to 4 floors.
- **J25 now counts street sleepers only** (Dylan): squats, cots and Hotel beds are rungs. The Dreg definition (`Household.home == None`) is unchanged everywhere else, and core.rs prints the Dregs' split beside J25.

**The gates and the table:**
- With `[sickness]` on, core_sanity's starvation bound is 20 per 120 days (decision 5), with five sickness bullets.
- The recal tree's calibration city is ported: the shipped city at 2,000, the income path and `p_desperate` learned everywhere. There is no `--lod all-full` flag. `calibrate` learns `p_dive` with the bins on (a row under 200 eligible hours takes the all-row mean).

**Findings:**
- **Cancer is near zero at the starting keys.** On the flipped config 0-3 cases start in 120 days; at most 3 people reach toxicity 0.4 in a sickness-only year. The 20-40 Disease deaths a year is a printed target, not tuned.
- **The parity test on the flipped config** passes thefts, violence and the actor shares (the recal port). It fails the diet rates:

  | rate (per 100 agent-days) | Full | Statistical |
  |---|---|---|
  | hunger-days | 20.4 | 4.9 |
  | synth % of meals | 27.8 | 33.7 |
  | bin meals | 3.4 | 9.3 |
  | malnutrition × 100 | 35.8 | 19.3 |

  The off-screen tier eats the hour it is hungry (an old gap, inside the old 0.5 floor) and reaches bins from its Home door, while a body reaches from where it stands. This is the next mechanism to close.
- **The default city, recalibrated:** off-screen thefts fall (seed 42 × 60: 917 vs 3,675 on the stale table) and starvation rises to the all-Full ground truth (18-33 per 120 days).

## Open items (after P5)

- **Marriage parity (default config):** Full 0.86 against Statistical 0.32 marriages per 100 agent-days. This is a density gap from the recalibration's 2,000-resident calibration city (the recal tree saw it too) and predates hunger as sickness. It is filed here, not chased.
- **P5g (held for Dylan's city-design call):** the median commute is about 90 tiles, roughly 3 hours each way (farmers median 116 tiles, about 400 of them past 60). This is the bulk of Full's missed shift time. The options (a commute cap in hiring, relocation workers can afford, quits for a nearer post, transit) are with Dylan.
