# God scenarios v3: districts, the street, riots

The third run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M12: districts and their control, the law per district, litter and the sweepers, the street (Hotels,
derelicts, squats, Vagrancy), riots and splits. Ten scenarios and an unshocked control, in
`citysim/tests/god_districts.rs`:

```
cargo test --release -p citysim --test god_districts -- --ignored --nocapture
```

About 2 minutes for all eleven on one thread. Measured on M12 phase 5 with its final calibration
(`alloc_crime` 1.5 squared, `vagrancy_base` 0.25, `split_loyalty` 0.66, the phase 5 stat table).

## How the suite works

- **The v1/v2 pattern.** Seed 42, default config, baseline days 30-45, the shock at the start of day
  45, observed to day 105 against an unshocked control computed once per process. A scenario passes
  when, within 7 days, a district's controller or stance, the law's posture or a founding gang's order
  differs from the control on some day, or a daily series' mean differs from the control's by more
  than twice its baseline standard deviation. Never a magnitude.
- **What is recorded per day:** per district its controller and share, stance, guards, sweepers,
  unrest (all and Street), litter, crime rate, coverage, fear, happiness, adults, Dregs, rough
  sleepers and whether a riot is live; city-wide population, homeless, emigrants, starvation,
  thefts, arrests, evictions, assaults, raids, riots (counted and events), lootings, crossfire,
  Vagrancy, Hotel nights, squatters, squats taken and broken, derelicts, Dregs, sweepers, Sanitation,
  Stance and control changes, splits, strikes, corp order changes, live gangs, and fights between a
  gang founded after day 0 and any other ("splinter fights").
- **New commands** (`PlayerCommand`, logged and replayed like the rest; god commands are a
  `PlayerAction` event prefixed `God:`):

  | Command | Effect | CLI lever (district by index 0-7) |
  | --- | --- | --- |
  | `SetSanitation(n)` | the city's sweeper headcount | `sanitation=12` |
  | `SetSanitationWeight { district, weight }` | the district's weight in the sweeper deal | `sanitation_weight=4:2.0` |
  | `SetCurfew { district, on }` | Vagrancy x `curfew_mult`, submission + `curfew_fear`, residents' happiness -0.05 | `curfew=5:on` |
  | `SetRiotResponse(Option<RiotResponse>)` | pins Contain, Disperse or Crush; `None` hands it back to the captain | `riot_response=crush` / `auto` |
  | `Riot(district)` (god) | a riot at the next muster hour, unrest, streak, cooldown and caps ignored | `riot=5` |
  | `Litter { district, level }` (god) | every street tile to `round(level x 254)` (rubble stays rubble) | `litter=4:1.0` |
  | `SplitGang(gang)` (god) | the split rule minus the roll and the character tests; refused only on the structure | `split_gang=0` |
  | `Derelict(building)` (god) | a Block, Bar or Hotel goes derelict now | `derelict=<building index>` |
  | `BuyBuilding { buyer, building, price }` (god) | the buyer pays the owner; a derelict is restored to the buyer | `buy_building=city:<index>:0` |

  `SetGuardWeight` and `SetStance` landed in phase 2.

### Reading "reacted"

As in v2, seed 42 is chaotic, and every scenario passes. Any shock reshuffles which day the Sump
stances flip, when The Hollow takes Sump West, when the law garrisons and **whether a gang splits on
its own**: with `split_loyalty` 0.66, a natural split lands in eight of the ten shocked runs between
days 46 and 84 and never in the control. "Live gangs" and "splinter fights" are therefore noise in
every scenario but `god_split_gang`. The verdicts below read the series a shock touches directly.

## The control

Dregs 48 a day at the baseline, 30 by days 75-105; homeless 31-48; squatters 26-36. No riot, no
split, one strike (Nutrix in Sump West, day 75). Sump West has the highest crime rate (5.1-5.5 per
100 a day) and litter (0.21-0.26) and stays Contested to day 69, then The Hollow's; Mid East has the
lowest unrest (0.25-0.28) and the best happiness (0.52-0.54) and is Vatra's. The law spends about
half of days 45-104 in Garrison.

| per-day mean | base 30-45 | 45-75 | 75-105 |
| --- | --- | --- | --- |
| thefts | 50 | 73 | 87 |
| assaults | 11.4 | 16.0 | 18.1 |
| Dregs | 48.3 | 44.3 | 30.5 |
| squatters | 35.6 | 36.0 | 26.0 |
| Sump West guards | 1.9 | 3.1 | 2.5 |
| Sump West litter | 0.18 | 0.23 | 0.26 |

## god_riot_sump_west: a riot in Sump West

**What happened.** Forty gathered at Bar#473 at 20:00 against a corp Block (Block#201, grievance 0)
and won, 15 against one defender: "looted Block#201: 0 coins". Sump West's guards rose to 5 that week
(3.7 in the control), but the captain read Disperse and the stance stayed Patrol. Corp order changes
ran 0.57 a day against the control's 0.43 that week and 0.83 against 0.27 over the month: a slight
corp answer, inside the noise.

**Interesting?** The riot machinery works end to end on demand, but a riot against a Block loots
nothing and costs its corp nothing visible.

## god_riot_in_spire: a riot in the Spire (the spec's case)

**What happened.** Thirty-six gathered at Block#19 against Habitat's Block#0 and won, 17 against no
defender at all: nothing looted. The Spire stayed Patrol (Disperse, no Cordon) and Habitat's all run;
no corp Secured in answer.

**Interesting?** The richest district has no guard at a corp Block's door at 20:00, and the riot
there is as harmless as the Sump's.

## god_litter_mid_east_to_cap: litter Mid East to 1.0

| per-day mean | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- |
| Mid East litter | 1.00 | 0.99 | 0.89 | 0.12 | 0.14 |
| Mid East sweepers | 6.7 | 6.2 | 5.0 | 1.3 | 1.0 |
| Mid East happiness | 0.46 | 0.45 | 0.42 | 0.54 | 0.52 |
| Mid East unrest | 0.24 | 0.25 | 0.24 | 0.27 | 0.28 |
| Mid East adults | 334 | 330 | 314 | 328 | 321 |

**What happened.** 5,356 street tiles went to 254. The sweeper deal answered at the next midnight
and kept half the city's sweepers in Mid East for two months. It made no visible dent: the share of
littered tiles stayed at 1.00 for two weeks and 0.89 sixty days later. Happiness fell 0.08 at once.
Unrest did not move, and residents left no faster than in the control.

**Interesting? Two gaps.** A sweeper clears `clean_per_shift` (120) units a shift, against 254 on
each of thousands of street tiles, so the city cannot dig a district out; and litter reaches mood but
not unrest, residence or prices.

## god_split_gang: split The Hollow by command

| per-day mean | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- |
| live gangs | 3.0 | 3.0 | 3.9 | 2 | 2 |
| splinter fights | 1.7 | 2.4 | 3.5 | 0 | 0 |
| assaults | 11.1 | 15.3 | 27.9 | 16.0 | 18.1 |
| Sump West crime | 5.5 | 5.7 | 8.4 | 5.3 | 5.5 |

**What happened.** The Hollow held Homes in five districts (Sump West 23, Sump Central 14, Sump East
5, Mid West 4, Mid East 2). The command split it at once: Dorian Nakamura took 12 members and 22
Blocks in Sump West as the Rust Saints. On day 79 the Rust Saints' leader fell and **the splinter
split again**: Leopold Upton's Low Choir, 16 members, 10 Blocks. Fights between a splinter and
another gang ran 1.7 a day in the first week and 3.5 a day by the end; assaults rose half again over
the control, Sump West's crime by half.

**Interesting? The best scenario of the suite.** Two gangs, a war, and the hydra the splits were
meant to give. Before phase 5 the command was refused on day 45 ("lieutenant too weak", "too
loyal"); the god command now skips the character tests (a D42 deviation, in `M12_DISTRICTS.md`).

## god_garrison_60_days: Garrison pinned for 60 days

| per-day mean | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- |
| guards in Sump West / Mid East | 0 / 0 | 0 / 0 | 0 / 0 | 3.1 / 1.5 | 2.5 / 1.2 |
| Mid East crime | 3.2 | 2.9 | 9.1 | 3.0 | 4.2 |
| Sump West unrest | 0.44 | 0.44 | 0.49 | 0.40 | 0.39 |
| assaults | 12.1 | 16.8 | 38.2 | 16.0 | 18.1 |
| riots | 0 | 0.03 | 0.03 | 0 | 0 |

**What happened.** Every district went dark the same day (no guard allocated anywhere, coverage at
its floor) and stayed dark. A month in, crime came: Mid East at more than twice the control's rate,
assaults at twice, Sump West's unrest 0.10 above it (still under the riot threshold of 0.54). Two
riots, both in the Civic (one looted a Street Market of 570 food and closed it three days).

**Interesting? Yes, slowly.** The D11 cost made visible: a dark city is fine for a month, then
violence doubles.

## god_evict_sump_rent_10: city rent 10 on Sump Blocks

| per-day mean | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- |
| thefts | 117 | 184 | 207 | 73 | 87 |
| evictions | 0.4 | 0.7 | 3.2 | 0.2 | 0.2 |
| homeless | 45 | 44 | 74 | 44 | 31 |
| Dregs | 45 | 44 | 72 | 44 | 31 |
| Vagrancy | 0.1 | 0.4 | 2.8 | 0.4 | 0.3 |
| assaults | 16.0 | 30.3 | 42.0 | 16.0 | 18.1 |
| riots | 0 | 0.03 | 0.07 | 0 | 0 |
| Sump Central crime | 3.9 | 4.9 | 11.7 | 3.6 | 4.8 |
| population | 1978 | 1954 | 1890 | 1976 | 1952 |

**What happened.** Thefts doubled within the week (tenants stealing to pay), evictions began once
the arrears ran out and ran three a day by the end, the Dregs and the homeless more than doubled,
Vagrancy rose ninefold, crime in all three Sump districts doubled, assaults reached 42 a day (the
gates' bound is 42.7), population fell 62 below the control. Three riots, all in the Civic against
the Precinct (grievance 1-5, its Vagrancy record), two of them won with three convicts freed. No Sump
district rioted: their unrest peaked at 0.50.

**Interesting? The spec's spiral, with the riot in the wrong place.** Eviction wave, Dregs, crime,
Vagrancy: all there. The riot comes where the swept Dregs sleep (the Civic), not where the evictions
were.

## god_kill_sweepers and god_kill_sweepers_no_rehire

**What happened.** Killing the 12 sweepers did nothing: the city re-hired 12 within a day. With the
headcount also set to 0, the dirtiest district's litter climbed from 0.20 on day 45 to 0.42 on day
90 against the control's 0.26-0.28 at the end; Sump West ended at 0.24 against 0.26 and Mid East at
0.15 against 0.14. No unrest, riot or residence change.

**Interesting? A calibration fact.** Decay does most of the cleaning: twelve sweepers are worth about
0.15 on the dirtiest district's share after six weeks and nothing measurable anywhere else.

## god_withdraw_sump_west: guard weight 0 in Sump West (the spec's case)

**What happened.** Sump West went Withdrawn the same day and its guards went elsewhere. The Hollow
took control on day 61, inside the spec's 30 days, held it three days, lost it, and Sump West ended
Contested; the control's Hollow took it on day 70 with the law present and kept it. Crime doubled at
days 75-105 (9.7 against 5.5); unrest moved 0.04.

**Interesting? Some signal, late.** The Sump sits near the coverage floor with the law present, so
taking its beat away shows only after a month, in crime, not control.

## god_derelict_and_buy_back

**What happened.** Nutrix's Bar#473 in Sump West went derelict (its staff let go, Nutrix took
`BuildingLost`), and the city bought a derelict Block back for 0 (its squatters put out). The rest is
butterfly.

**Interesting?** It proves the commands; nothing else.

## Gaps

1. **Riots go where the grievance points, not where the anger is.** An eviction wave in the Sump
   raised Sump unrest to 0.50 and no Sump district rioted; the Civic (a few dozen adults, mostly
   swept rough sleepers) rioted against the Precinct three times. District unrest averages the evicted
   with the housed; a recent-evictee or Dreg weight, or an eviction-count trigger, would put the riot
   in the Sump.
2. **A won riot at the Precinct is a jailbreak** (three convicts freed) without a BreakOut's rules or
   the M9 Garrison answer.
3. **Looting a Block yields nothing and does not shock its corp into an order change**, and no corp
   Secured after a riot in its district (the spec's question).
4. **The sweepers cannot clear a heavily littered district**, and twelve sweepers are worth little
   against decay: `clean_per_shift` is sized for the daily trickle, not a heap.
5. **Litter touches mood only**: a littered district loses happiness but no residents, no unrest and
   no prices.
6. **Disperse never cordons.** Both forced riots met a Disperse captain and the district stance
   stayed Patrol; only Contain and Crush cordon (D34), so the spec's "does the law Cordon" is the
   captain's personality.
7. **A corp Block in the Spire has no defender at 20:00**; the richest district's riot met nobody.
8. **Pressure pushes violence to the gates' bound**: the rent shock (42.0 assaults a day) and a
   60-day Garrison (38.2) approach 42.7. The gates run unshocked, so they do not see it.

## The v1 and v2 suites, re-run

`--test god` 10/10 and `--test god_corps` 12/12 pass. Their documents were measured before M12 and
the baselines have moved:

- **v1 (`GOD_SCENARIOS_V1.md`, measured at 300 residents):** the control's violence is 11-18 a day
  (5-6), thefts 50-87 (1-2), arrests 20-23 (5-7); 48 guards on the books (city and private) against
  35. The stories are the 2,000-resident city's now, not the doc's.
- **v2 (`GOD_SCENARIOS_V2.md`, measured before the M11 calibration):** the control is no longer "zero
  Dregs and zero homeless": 30-48 of each a day (M12's derelicts and the 7-day re-housing wait). The
  price is no longer 3 everywhere: 3.2-4.5 mean, up to 7.6 late in the run. Strikes are rare (one in
  the 75 days). `god_bankrupt_food_leader` no longer churns through 14 bankruptcies: the fire sale clears in
  the first week (2 acquisitions a day at days 45-52) and bankruptcies stay near 0.1 a day.
  `god_rent_shock` raises thefts to 69-94 against 57-73, with no eviction wave.
