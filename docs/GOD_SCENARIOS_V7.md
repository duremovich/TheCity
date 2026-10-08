# God scenarios v7: the living city

The seventh run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
Life pass L2 (`docs/LIFE_L2.md` § 5): wages over the dole, the venues and the Parts Fab, fun and the street,
the Treasury's budget band, the export hook, and the faction layer's violence off screen. Every wage, bet,
beating and killing below is an abstract event between fictional agents of a simulated city: a seeded dice
roll over structs and counters. Five scenarios in `citysim/tests/god.rs`, each against an unshocked
control run of the same seed:

```
cargo test --release -p citysim --test god -- --ignored --nocapture god_close_mid_west_clubs god_double_dole_30 god_kill_fab_staff god_faction_strike_stackwell god_export_10x
```

About a minute for the five and their controls on parallel threads. Measured on L2 phase 5's final tree
(`[budget] works_max` 120 at `works_wage` 7, the per-class faction priors, the regenerated stat table).

## How the suite works

- **Seed 42, default config, 90 days**, the shock at the start of its day, observed against the
  unshocked control day by day. Each prints a table of sums (or daily means, rows marked "(mean)") over
  windows before, during and after the shock, and the story from the first window (god actions,
  foundings and refits, exports, corps' Grow orders). The tests assert only that the god commands
  applied (no `PlayerActionFailed`, the expected `PlayerAction` count); a failure to react is a gap
  below, not a red test.
- **Commands** (`PlayerCommand`, logged and replayed): `CloseLeisure { district, days }`
  (`close_leisure=<d>:<days>`), `SetDolePerDay(n)` (`dole_per_day=<n>`), `KillAgent(id)`,
  `FactionStrike { gang, district, days }` (`faction_strike=<gang i>:<d>:<days>`), `SetExport(bool)` and
  `SetExportPrice { good, price }` (`export=on`, `export_price=<good>:<n>`).

## The scenarios

### `god_close_mid_west_clubs`: every leisure venue in the Club's district closed for 14 days, day 45

Mid West holds no Club on seed 42 (the three seeded Clubs stand in Civic, Vats and Spire: the seeding
rule takes the Mid and Spire tiers and picks the Lot nearest the district's centre), so the scenario closes
**Civic**, the district with the most venues and a Club (7 venues).

| days 45-59 / ctl | Civic visits | city visits | HangOuts | Civic street density (mean) | city fun (mean) | thefts | violent deaths |
| --- | --- | --- | --- | --- | --- | --- | --- |
| shock | 0 | 399 | 6,193 | 0.033 | 0.568 | 1,322 | 7 |
| control | 195 | 461 | 5,933 | 0.043 | 0.578 | 1,373 | 14 |

- **Reacted, by substitution.** Civic's venues took no visit for the 14 days; the city's visits fell 13 %
  and the evenings moved to the street (HangOuts +4 %) and to other districts' venues. Civic's street
  emptied a little (density 0.033 against 0.043): its venues are what drew people there after dark.
- Fun dipped by 0.01; thefts and violence did not rise (1,322 against 1,373; 7 against 14): closing one
  district's leisure is not a crime shock while the rest of the city stays open.

### `god_double_dole_30`: the dole doubled (4 → 8) from day 20 to day 50

| / ctl | employed (mean) | wages | dole | flow_leisure | visits | thefts | Treasury (mean) | public works (mean) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| days 20-50 | 506 / 509 | 71k / 71k | 315k / 178k | 62.8k / 4.3k | 17.6k / 1.0k | 516 / 2,249 | 42k / 85k | 83 / 101 |
| days 50-90 | 500 / 533 | 96k / 103k | 240k / 224k | 15.1k / 6.4k | 4.0k / 1.4k | 2,354 / 4,532 | 33k / 79k | 60 / 120 |

- **Reacted, strongly.** Leisure spend went up fifteenfold (62.8k against 4.3k coins over the 30 days),
  thefts fell to a quarter and violent deaths to 1 against 14: the dole is spendable money and the venues
  took it. Jobs did not empty (506 against 509).
- **The band moved, and kept moving after the shock.** The Treasury halved (42k against 85k) and from
  day ~50 the band laid off public works (60 against 120) for the rest of the run; thefts stayed at half
  the control's (2,354 against 4,532) after the dole went back to 4, the wallets the dole left behind.
  `upkeep_mult` did not move (the corp upkeep cut needs the Treasury above the band).
- **Gap:** the band reads only the Treasury: a god's dole spending is answered by laying off the wage
  earners it hired.

### `god_kill_fab_staff`: every Fabber (18) struck dead on day 20

| / ctl | Fabbers (mean) | Fab Parts made | Parts sold from Fabs | from the Recycler | imports | customs |
| --- | --- | --- | --- | --- | --- | --- |
| days 20-35 | 18 / 18 | 814 / 851 | 22 / 36 | 0 / 0 | 7,992 / 6,984 | 8,277 / 7,269 |
| days 35-90 | 19 / 19 | 112 / 129 | 34 / 74 | 28 / 11 | 14,712 / 18,336 | 16,738 / 19,658 |

- **No lasting reaction**: the Fabs posted their vacancies the same midnight and filled them from the dole
  pool the next day (18 on the daily mean of days 20-35, as the control). Fab stock (the 400 cap) covered
  the gap. Imports and customs rose 14 % over the next two weeks (8.0k against 7.0k), then the two runs
  diverged on their own.
- **Gap:** labour is fungible: a Fabber is an unskilled hire, so killing a staff costs a Fab one day.

### `god_faction_strike_stackwell`: `FactionStrike` Ninefold on Stackwell's district for 14 days, day 45

Stackwell is a Housing corp; its district is the one where it owns the most Homes: **Sump West**.

| / ctl | faction holes in Sump West | of them Ninefold's | fv_assaulted (city) | fv_bound / fv_unknown | fv_bound_wrong |
| --- | --- | --- | --- | --- | --- |
| days 45-59 | 1 / 0 | 0 / 0 | 2 / 0 | 1 / 1, 1 / 0 | 0 / 0 |

- The command applied (Ninefold pinned to Contest on Sump West, the district in `fv_active`) and the binder
  stayed correct (`fv_bound_wrong` 0).
- **Gap: no off-screen hole bound to Ninefold in the 14 days** (the one hole there is The Hollow's Expand).
  The per-class priors put a civilian off screen at the rate on-screen civilians suffer (calibration (b):
  0.27 assaults and 0.03 killings per 1,000 agent-days), and a fresh cell reads its prior: ~200 Statistical
  civilians in Sump West expect under one assault in 14 days. A strike that shows needs a class-weighted
  civilian rate for a god's Contest or bodies in the district (the on-screen ledger fills the cell).

### `god_export_10x`: the export open at ten times the price, day 20 (`export_price` food 30, parts 180, data 400)

| / ctl | flow_export | foundings | employed (mean) | thefts |
| --- | --- | --- | --- | --- |
| days 20-50 | 168,000 / 0 | 0 / 0 | 514 / 509 | 2,294 / 2,249 |
| days 50-90 | 224,000 / 0 | 1 / 2 | 542 / 533 | 4,393 / 4,532 |

| corp treasury, order | day 20 | day 50 (ctl) | day 89 (ctl) |
| --- | --- | --- | --- |
| Militech | 1,895 Hunker | 104,855 Secure (879 Hunker) | 243,636 Secure (gone) |
| Nutrix | 7,030 Secure | 19,439 Secure (17,189) | 68,052 Secure (22,312) |
| Greenline | 4,912 Secure | 14,257 Secure (11,331) | 42,460 Secure (14,394) |
| Zetatech | 12,734 Secure | 20,610 Secure (4,486) | 42,615 Secure (gone) |

- **Reacted in money.** 392k coins crossed in (the World account refills to `treasury_ref` daily, so the
  outside mints them); Militech and Zetatech, bankrupt in the control by day 89, survive on Parts and Data
  sales; Nutrix and Greenline (Food) end three-fold richer.
- **Gap: the corps do not Grow.** No corp holds `Grow` for a day after the export opens; the rich corps sit
  on Secure (Militech with 244k). Grow is scored against niche demand and Lots, not against the treasury,
  so export income does not become buildings, jobs or wages: employment is flat (542 against 533).

## Gaps (the list for M16-M17 and the late calibration)

1. **The band answers a Treasury drain by laying off the public works** (the double dole): the band reads
   only the Treasury, so any shock that spends it (a god's dole, the works' own wage bill in a long run)
   ends in layoffs of the public works.
2. **Labour is fungible** (the dead Fab staff are replaced overnight): unskilled staff make every staff
   kill a one-day shock.
3. **A FactionStrike on a civilian district barely shows off screen**: the off-screen rate is the
   on-screen civilian rate, which is low because gang orders on screen mostly hit members.
4. **Export income does not become growth**: the corps' Grow is gated by niche demand and Lots, not by
   treasury; 170k coins sit idle in two corps.
5. **Closing one district's venues moves the evening elsewhere**: fun dips 0.01 and crime is unmoved; the
   scenario only bites city-wide or under curfew.
6. **No Club in Mid West on seed 42**: the seeding rule (nearest Lot to a Mid or Spire district's centre)
   put the three Clubs in Civic, Vats and Spire.
7. **The roster cap had no exit** (found by the 365-day runs, not a god scenario): four gangs filled to
   `max_members` 60 by ~day 150 and stayed there, every death refilled, 0-18 leaves a month; their
   violence outran births and immigration. L2 phase 5 adds gang desistance (`[living] desist_*`,
   `docs/LIFE_L2.md` "the year-long climb"): the rosters plateau at 70-85 % of the cap.
