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
- `toxicity += tox_synth × (synth + alms) + tox_dive × dive − tox_clear`, clamped to 0..=1.

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

- **Onset**: at midnight, with `toxicity ≥ cancer_tox_min`, p = `cancer_p × (t − cancer_tox_min) / (1 − cancer_tox_min)`, rolled on the agent's stream.
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
tox_synth = 0.005          # per synth or Mission meal
tox_dive = 0.012           # per bin meal
tox_clear = 0.003          # a day
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
cancer_tox_min = 0.6
cancer_p = 0.003           # a day at toxicity 1.0
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
bin_gang_control = 0.5
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
