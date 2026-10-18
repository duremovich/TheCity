# God scenarios v4: assets

The fourth run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M13: vehicles, chrome, stims and the security robot. Five scenarios in `citysim/tests/god.rs`, beside
the v1 scenarios and against the same unshocked control:

```
cargo test --release -p citysim --test god -- --ignored --nocapture god_chrome_everyone_2 god_flood_stims_sump_east god_stims_legal_day_30 god_brick_spire_clinic_owner god_chase_civic_core
```

About 35 seconds for the five (plus the control) on parallel threads. Measured on M13 phase 5 with its
final calibration (`docs/M13_ASSETS.md`, "Phase 5 calibration").

## How the suite works

- **The v1 pattern**, seed 42, default config, baseline days 30-45, the shock at the start of day 45,
  observed to day 105 against the unshocked control. Unlike v1-v3 these five **assert nothing**
  (plan 5.4): each prints the v1 table and reaction list and an M13 block (window means of episodes,
  episodes ended by the law, Therapy, Detox, installs, hooked adults, doses dealt, legal sales, gang
  income and its dealing share, Harvest and raid orders, raids, gang members with working fighting
  chrome, bricked implants, crashes, crash deaths, Manslaughter and Murder reports, violence, posture
  changes). A failure to react is a gap below, not a red test.
- **New commands** (`PlayerCommand`, logged and replayed; god commands are a `PlayerAction` event
  prefixed `God:`):

  | Command | Effect | CLI lever |
  | --- | --- | --- |
  | `SetAssetTax { kind, rate }` | upkeep × `1 + rate` for one asset class | `asset_tax=car:0.5` |
  | `SetImpound(bool)` | the city impounds vehicles with unpaid upkeep, or not | `impound=off` |
  | `SetStimsLegal(bool)` (phase 4) | Markets restock and sell Stims | `stims_legal=on` |
  | `GrantAsset { agent, kind, tier }` (god) | a free asset, no import | `grant_asset=17:arms:2` |
  | `Wreck(asset)` (god) | wreck it now (an implant fails) | `wreck=2100` |
  | `ChromeEveryone { tier }` (god) | every adult gets Arms and Nerves of that tier, filled slots kept | `chrome_everyone=2` |
  | `FloodStims { district, n }` (god) | n doses into the Hideout stock of every gang dealing in, holding or hiding in the district | `flood_stims=7:500` |
  | `Brick(lender)` (god) | every implant that lender financed is bricked and its loan called | `brick=8` / `brick=agent:42` |
  | `Chase(agent)` (god) | the agent's next trip is a chase for the crash roll | `chase=99` |

## The scenarios

Window means per day; "ctl" is the control over the same days.

### `god_chrome_everyone_2`: every adult gets Arms T2 and Nerves T2

1,932 adults chromed (3,722 implants). **Reacted hard.** Episodes 5.0 a day in days 45-52 and 8.5-8.9
a day after (control 0-0.1); 0.4-0.8 a day ended by the law; violence 37-49 a day (control 15-37);
Murder reports 1.0-1.4 (control 0.1-0.7). The captain's posture moved within six days (Crackdown on
day 50, control Garrison). Gangs: chromed fighters 100 → 69 as berserkers die and are jailed. Dealing
collapsed (0.6-6 a day vs 32 by days 75-105 in the control): the chromed buy no stims and the gangs'
income fell to 80-117 a day. **Therapy did not rise** (0-0.07 a day against the control's 0.14-0.27):
the Treat goal's need term reads `edgy − sanity` (edgy 0.5), so a body at sanity 0.6-0.8, already
rolling episodes under `psycho` 0.8, scores almost nothing for Therapy, and a Clinic's staff of two
serve few.

### `god_flood_stims_sump_east`: 500 doses into every Hideout dealing in Sump East

Two Hideouts flooded (1,000 doses). **Addiction spread:** hooked adults 10.9 → 21.7 → 29.7 (control
6.1 → 3.9 → 17.2), doses dealt 25-28 a day from the start (control 4-5 until day 75), gang dealing
income 108-118 a day (control 18-20). The Sump East gang grew (57 members against 25) and its
treasury went from nothing to ~9,000; the posture went to Crackdown on day 50 (control Garrison).
**No rival raided for the stock**: raid orders stayed at the control's 0-0.2 a day. The stock is in a
Hideout, which the raid machinery already targets, but no order reads a rival's stock (D38's raid
win only moves it).

### `god_stims_legal_day_30`: Stims legal from day 30

Legal sales 27 a day at first, 16-22 through day 75, then 5 (the Markets' purses limit the restock).
**Gang income did not fall**: dealing income 113 a day in days 45-52 against the control's 20, and
level with the control later (154 vs 145). Legal stims made addicts (hooked 34 → 46 → 65 against 4-17)
and the addicts buy from whichever source is nearer, dealers included. **No turn to Harvest or Raid**
(Harvest orders 0, raid orders as the control). The prohibition lever works backwards here: legalising
grows the market the dealers share.

### `god_brick_spire_clinic_owner`: Zetatech bricks what it financed

Re-run with the shipped `[assets] implant_down_frac` 0.75 (the other four scenarios were measured with
implants cash only, 1.0; the control differs between the two). "Zetatech bricked 2 financed implants in
2 bodies": at three-quarters down few implants are financed, and by day 45 only two of them at the Spire
Clinic. Bricked implants 2 -> 3 (the third the ordinary arrears path); chromed gang fighters 12.3
against the control's 12.4 in days 45-52. **No gang lost its fighters**: the Spire Clinic's lender
reaches almost nobody, and gang chrome is bought cash by the gang (`gang_arms`) or by members at the
nearer back-alley docs. The only reaction within 7 days was noise (g1 members 22.6 vs 22.9).

### `god_chase_civic_core`: a chase pinned on every Civic driver for a week

Ten drivers whose Home or work is in the Civic pinned each day 45-51. **Identical to the control**: no
crash, no death, no report. Seven days of ten pins is ~70 chase trips at most, and most of those
drivers are Statistical (their trips are not rolled) or stay home; at `crash_per_tile` 0.0001 × the
chase multiplier 6 a Full road trip of ~30 tiles crashes ~2 % of the time.

## Gaps

- **The law garrisons most of the year because the Jail is half gang members.** The M9 Garrison
  threat is the gang share of the 80 beds; with ~40 members inside (Assault, Extortion and, since M13,
  Dealing sentences) seed 42 runs spend 57-85 days in Garrison with assets off and much the same with
  them on. Every shock's "posture moved" above is Crackdown against the control's Garrison. Two v1/v3
  posture scenarios (`god_garrison_forever`, `god_garrison_60_days`) found the control already in
  Garrison at the shock and now compare against a Patrol-pinned control, as `god_crackdown_forever`
  did since M10.
- **Dealer diversion is not why gangs hold fewer districts** (tested for the M12 gate in phase 5):
  dealing off left gang control unchanged, and a cap on the dealers' share of the roster (0.25) was no
  better; it was removed. Do not retry it.
- **Therapy does not answer an episode wave.** Treat's need term reads `edgy − sanity` (edgy 0.5),
  and since phase 5 bodies roll episodes from `psycho` 0.8, so the berserk-prone score near zero for
  Therapy; a Clinic has two staff. Phase 5 moved `psycho` and left the Treat curve: a follow-up.
- **No raid for a rival's stims.** A flooded Hideout is a target only by the existing raid reasons; no
  order reads the rival's stock.
- **Legal stims grow the dealers' market** instead of shrinking it: legal sales cap at the Markets'
  purses and make addicts who buy from dealers too. No gang turned to Harvest or Raid.
- **`Brick` barely bites**: at `implant_down_frac` 0.75 a lender holds a handful of implant loans
  (2 at Zetatech by day 45), and gang chrome is bought cash. A Brick aimed at the back-alley docs, or a
  vehicle-tow command (vehicles are financed at half down), would be the levers with an effect.
- **A chase is invisible off screen**: pins only bite on Full and Coarse trips; the Statistical tier
  has no crash roll at all. A chase through a district needs the drivers promoted first.
- **`SplitGang` is refused for want of a site** on seed 42 at day 45 ("no squat or Lot in Sump East");
  the v3 scenario lifts `max_gangs` (the gangs had split on their own by day 28) and now passes on the
  reactions the lifted cap allows, not on the command.
- Carried from earlier suites: riots go where the grievance points, not where the unrest is; litter
  moves mood only; a dead gang at high coverage never re-forms (`GOD_SCENARIOS_V3.md`).
