# P10: the switches retired, the safety net as policy

Companion to `ECONOMY_V2.md` (wages from revenue, no dole), `JOBS_V2.md`, `HUNGER_SICKNESS.md`, `TESTING.md` and the memories `thecity-utopia-dystopia-game` and `thecity-no-safety-net`. Dylan (2026-10-10): the brutal no-net city is the **starting state, not a law**; a player strong enough to win a seat should be able to enact a safety net if the city can fund it, and the corps should push back. So P10 retires the flip's config switches and keeps every safety-net mechanism behind **saved policy levers** the city government enacts and funds from the Treasury.

## Decisions (overturnable)

| # | Question | Decision |
| --- | --- | --- |
| 1 | Where the policy lives | **`World::levers`** (`levers::Levers`), the saved lever state the dole (`dole_per_day`) and public works (`public_works`) already lived in, plus three new fields: `levy`, `school_meals`, `reserve_release`. No second `CityPolicy` struct: one policy, one home. Seeded at world creation from a new `[policy]` section (`PolicyCfg`), read only from the levers after that. |
| 2 | The default | `[policy]`'s defaults and the serde defaults are the shipped city: the market wage law, no dole, no public works, no school meals, no Reserve release. |
| 3 | `wages`: the only wage path? | **No: the fixed-wage path stays, as the `levy` lever.** It is not a civic wage subsidy, but it is the funding side of the old safety net: the M11 wage schedule comes with the corps' upkeep levy to the Treasury (Farm 245, Market 600 a day), which is the pump that paid the dole (ECONOMY_V2 § 0: the Food corps hand ~74 % of revenue to the Treasury, the Treasury hands it back as the dole). With wages from revenue and the dole on, the Treasury ran dry by day 12 (phase 2's stop rule). A government that wants a dole that holds needs the levy, so it is a policy, not a dead path. It is also the city ~60 unit-test files pin (`Config::dole_city`); retiring it would re-pin hundreds of tests on another economy. |
| 4 | What the levy carries | Everything the retired `[economy2] wages` switch gated, inverted: under the levy the config wage schedule (no `wage_rev`), the upkeep levy (no inputs or property rate), L2's budget band (public works sized by the surplus, the levy cut), the Treasury's scavenge coin; under the market law wages from revenue, the hiring rules, seed staff and working capital (at creation), the room cap, class pay, the property rate, the tax band and civic budget, the Recycler's till. |
| 5 | What `no_safety_net` gated | Split in two. **The net itself** became four levers: the dole (`dole_per_day`), public works (`public_works`), school meals (`school_meals`), the Reserve release (`reserve_release`). **The no-net city's institutions** (the tax band and civic budget, the Recycler's till, the opening `tax_initial`, the household rent share, the housing pre-roll, hunger fix 13 `rehouse_employed`, the Statistical tier's two no-net rules in `lod.rs`, the tax/till CSV columns) follow the **market wage law** (`econ::wages_on`), not the absence of a dole: enacting a dole in the shipped city keeps the tax band that has to fund it. Both presets are unchanged by the regrouping (the shipped city is market + no net, the dole city levy + net); only the never-shipped "wages on, net on" measuring city moves. |
| 6 | "Rent grace" | No switch gated one. The city's eviction lever `NoCityEvictions` already is a policy lever; nothing to retire. |
| 7 | Who pays | The dole and public works in **coins from the Treasury** (`collect_dole` debits it; public works are city wages). School meals and the Reserve release are **in kind, from the Reserve's stock** (the Treasury bought it); they pay short when it is empty. |
| 8 | A Treasury that cannot pay | Per payment, the rules that stood: a dole visit is refused while the Treasury is below 0; city wages pay short. **Lapse:** a day on which a dole visit was refused for want of coins, or that ends with the Treasury below 0, is a *dry day*; after `[policy] lapse_days` (7) dry days in a row the dole and public works **lapse** (`dole_per_day` 0, `public_works` off, one `PolicyLapsed` event) and stay off until enacted again. School meals and the release do not lapse (no coins). The levy never lapses (it is revenue). |
| 9 | Public works under the market law | L2's budget band does not run there (the tax band does), so `budget::market_works` opens and closes works posts on the **tax band's** reading: above its top on the hold's multiples it posts `works_step` up to `works_max`, below its bottom it withdraws and lays off `works_step`, newest first. L2's rule without the levy cut. |
| 10 | A mid-run levy switch | Allowed (`SetLevy`); the code paths read the lever at run time (several unit tests already flipped the switch on a running world). Creation-time effects (seed staff, working capital, the pre-roll, the opening tax rate) stay as created. Uncalibrated: a god scenario, not a tuned path. |
| 11 | `[sickness] enabled`, `[street] shanties` | **Retired, always on.** No unit test needed them off (8 tests moved, each fixed by its world state, see below). `[sickness] diet`, `bins`, `cancer` (part switches no test turns off) are left as they are; the shanties and the Hotel floors now ride the street rung (`[street] enabled`, which `v1_profile` cuts). |
| 12 | `Config::dole_city()` | A policy preset: `[policy]` = `PolicyCfg::dole_city()` (the levy and the whole net) and the flat housing catch-up, as before. Hunger as sickness and the shanties now run in it (decision 11), so it is **not** byte-identical to the old dole city; it is byte-identical to the old dole city with `[sickness]` and the shanties on (proved below). `citysim-cli run --dole-city` starts it. `v1_profile` starts under the dole city's policy less its school meals (as before). |
| 13 | The save | `SAVE_VERSION` 10; serde defaults (the shipped city). A version-9 save's retired config keys are ignored; its dole and public works levers load as saved, its levy and school meals load off: an old shipped save resumes byte-identically (proved below), an old dole-city save resumes under the market law (not supported, as ECONOMY_V2 § 8 already said of flips on a running city). |

## The switches and what became of them

| switch (main `ecaf8e5`) | what it gated | now |
| --- | --- | --- |
| `[economy2] wages` | wages from revenue and everything phase 2 / the jobs rounds / class pay read behind it | the **`levy`** lever, inverted (`[policy] levy`, `SetLevy`, `levy=on|off`); `econ::wages_on` reads `!levers.levy` |
| `[economy2] no_safety_net` | the dole 0 and school meals off at seed (`econ::apply`), public works off, no Reserve release; the tax band, the till, `tax_initial`, the household rent share, the pre-roll, hfix 13, two LOD rules, CSV columns | the net: **`dole_per_day`** (`[policy] dole` × `[levers] dole_per_day`), **`public_works`**, **`school_meals`**, **`reserve_release`**; the institutions: the market wage law (decision 5). `econ::no_net`, `no_net_cfg` and `apply` are gone |
| `[sickness] enabled` | hunger as sickness (synth meals, the meters, bins, cancer) | retired, always on (`diet::on` reads `[sickness] diet`) |
| `[street] shanties` | shanties on vacant Lots, Hotel floors | retired, always on with the street rung |
| `[demography] school_meals` | school meals (forced off by `no_safety_net`) | moved: `[policy] school_meals` seeds the `school_meals` lever |
| `Config::classpay_on`, `Config::trade_live` | class pay with wages on | `classpay::on(world)`, `classpay::trade_live(world, row)` (they read the lever) |

The switch list that remains (TESTING.md): `[life] violence_fixes` with its `vf_*` items and `CITYSIM_VFIX_OFF`, `[hitmarket] enabled` (A/B), and the part switches named above.

## The levers

| lever | `--lever day=D:` | `PlayerCommand` | effect | paid by | when the Treasury cannot pay |
| --- | --- | --- | --- | --- | --- |
| the levy | `levy=on|off` | `SetLevy(bool)` | the M11 wage schedule, the corps' upkeep levy, L2's band; off: the market law | (revenue) | never lapses |
| the dole | `dole_per_day=<n>` | `SetDolePerDay(u8)` | the jobless draw `n` a day | the Treasury | a visit refused below 0; lapses after `lapse_days` dry days |
| public works | `public_works=on|off` | `SetPublicWorks(bool)` | Sanitation posts above the band's top | the Treasury (city wages) | wages pay short; laid off below the band; lapses with the dole |
| school meals | `school_meals=on|off` | `SetSchoolMeals(bool)` | an unfed child eats from the Reserve | the Reserve, in kind | short when the Reserve is empty; never lapses |
| the Reserve release | `reserve_release=on|off` | `SetReserveRelease(bool)` | free food to a stranded Market | the Reserve, in kind | short when the Reserve is empty; never lapses |
| the whole net | `safety_net=on|off` | `SetSafetyNet(bool)` | the four above on (the dole at `[levers] dole_per_day`) or off | as above | as above |

M18's player calls the same commands from a council seat (a god issues them today).

## Proof

- **The shipped city is byte-identical.** `citysim-cli run --days 30 --seed 42 --report --events`, main `ecaf8e5` vs this change: the CSV less `ticks_per_sec` identical (31 lines), the events identical (26,136 lines).
- **The dole-city preset reproduces the old dole city's policy byte for byte.** `run --dole-city` (this change) vs main run on an assets copy with `wages`/`no_safety_net` off and the housing catch-up 0, `[sickness]` and the shanties on: CSV and events identical (24,697 event lines, 30 days, seed 42). Against main's actual `dole_city()` (sickness and shanties off) it diverges from day 1 (population, food, Treasury, thefts...): that is decision 11, by design.
- **An old save resumes.** A version-9 save of the shipped city written by main at tick 1440, run 3 days by main and by this change: CSV and events identical (3,251 event lines).
- **The levers** (`tests/policy.rs`, each failing on a mutant: the lever a no-op, the lapse pass a no-op, `market_works` a no-op): a dole enacted on day 10 of the shipped seed-42 city is paid from the Treasury to the jobless (no dole before; one draw moves exactly its coins from the Treasury to the wallet); a Treasury held below 0 lets the dole and public works lapse once, after 7 dry days, school meals untouched, no dole after; the control (a Treasury that pays) keeps its net; public works enacted on the shipped city open on the tax band and close when repealed.

## Tests moved by the retirement

`economy2.wages = true` over the dole city became `policy.levy = false` (the market law with the net); `wages = true; no_safety_net = true` over the dole city became `policy = PolicyCfg::default()`; the same pair over `Config::load()` was dropped (already the shipped policy); a running world's `config.economy2.wages = false` became `levers.levy = true`. The "with the net" twins of `safety.rs` and `treasury.rs` (band off, no till, no pin) are the dole city (under the levy: decision 5). Retired with their switches: `sickness.rs::test_sickness_off_leaves_no_trace` (the off switch's identity test) and the "off, nothing rises" half of the shanty test, replaced by a world-state control (a city whose rough sleepers fit the free squat slots raises none).

Moved by hunger as sickness now running in the dole city (none needed it off):

| test | why | fix |
| --- | --- | --- |
| `assets::test_capacity_with_strength_and_pack` | a broke buyer's meal is synth, a third of the price | the expected quantity reads the meal's price (`diet::price_at`) |
| `charity::test_mission_meal_costs_exactly_its_purse` | the kitchen serves synth, `synth_stretch` meals a unit | Alms is the unit's cost over the stretch |
| `charity::test_hourly_cap_doubles_with_a_volunteer` | after a day's ticks two eaters held a coin (a dive or the dole) | the eaters are set broke again before the second hour |
| `demography::test_starvation_kills_after_3_days_at_zero` | starvation takes `starve_grace_days` (7) with the diet | renamed `..._after_the_grace_at_zero`, reads the sickness grace |
| `goap::test_reservation_prevents_overcommit` | a one-meal buyer buys synth meals, and the reservation counts meals as shelf units (below) | the buyers hold `real_meal_reserve` real meals' coins and buy real: the units are units |
| `goap::test_beg_does_not_promise_a_meal_at_price_three` | a meal at shelf 3 is a synth meal at 1 | the shelf at 3 × stretch: a meal at 3 |
| `sickness::test_real_bought_above_the_floor` | its "diet off" half used the master switch | it uses the part key `[sickness] diet = false` |
| `sickness::test_shanties_rise_with_rough_sleepers_and_are_capped` | its "off" half used the switch | a world-state control (above) |

Moved by the regrouping (decision 5; the "wages on, net on" test city now has the market law's institutions): `wages.rs`' and `floors.rs`' phase-2 cities pin `[life] housing_preroll = false` (the pre-roll read no-net and now runs under the market law; five wages tests and two floors tests read the seeded staffing or a Lot near a Farm), and `wages::test_seed_capital_moves_hoard_pro_rata_and_conserves` pins `[treasury] tax_initial` negative and `till_initial` 0 against the levy twin (the market law now opens at the 0.05 rate and seeds the till; the till keeps its own test).

## Found, not fixed

- **A synth buyer's reservation counts meals as shelf units** (`plan::reserve_for` reserves `buy_quantity` units; with the diet on that is synth *meals*, a third of a unit each, while `WorldState::observe`'s `market_free` compares it with `stock_food` in units). A buyer of three synth meals reserves three units of a shelf they take one from: a near-empty Market reads "reserved" to the next buyer three times too early. Live in the shipped city since hunger as sickness; fixing it moves the shipped CSV, so it is left for its own change (`goap::test_reservation_prevents_overcommit` now pins the real-buyer case).
- `[needs] starvation_grace_ticks` is read only with `[sickness] diet` off now.
- **Under the market law the civic budget lays off public-works hires first.** `--lever day=10:safety_net=on` on seed 42 (30 days): works rose 10 → 30 by day 19 on the tax band, then fell to 2 by day 28 with the Treasury still inside the band (19.8k): the civic budget lowers `sanitation_count` and `districts::reconcile_sanitation` lets the newest sweepers go, who are the works hires. L2's rule, now reachable in a city with a civic budget; read before a player relies on public works.
- The tax band reads only the balance: with the dole enacted the same run cut the rate 0.05 → 0.02 while paying ~700-850 a day of dole (Treasury 28.7k on day 10 → 19.8k on day 28, above the band's top, 25k, through day ~16). It will raise the rate once the balance crosses the bottom; no lapse in 30 days.
