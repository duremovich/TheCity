# God scenarios v2: corps, classes, the economy

The second run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M11: ownership, rent, the eight corps and their brain, founding, classes and strikes. Ten scenarios, a
greedy variant of the takeover, and an unshocked control, in `citysim/tests/god_corps.rs`:

```
cargo test --release -p citysim --test god_corps -- --ignored --nocapture
```

About 35 s for all twelve. Measured at commit 23ae1ee (M11 phases 1-4), **before the economy
calibration pass**. Every number here is to be re-measured after it; the assertions only say that
something reacted, so the suite survives calibration, but the stories below may not.

## How the suite works

- **The v1 pattern.** Seed 42, default config, baseline days 30-45, shock at the start of day 45,
  observed to day 105, compared against an unshocked control computed once per process. A scenario
  passes when, within 7 days, a corp order, gang order or law posture differs from the control on some
  day, or a daily series' mean differs from the control's by more than twice its baseline standard
  deviation.
- **What is recorded per day:** each seeded corp's order (and niche), treasury, buildings, employees,
  price level and exec; corps without a seeding slot (incorporations, spinoffs); unrest, loyalty,
  submission and head-count per class; price and stock per Street Market; evictions, rent shortfalls,
  re-housings, strikes, foundings, incorporations, acquisitions, bankruptcies, monopolies; gang orders,
  members, territory, treasury; the law's posture; thefts, arrests, assaults, extortions, raids; who
  owns the buildings (city, corps, agents, gangs); gang joins by agents evicted earlier in the run;
  Lobby and Undercut orders taken.
- **New god commands** (`PlayerCommand`, logged and replayed like the rest, each a `PlayerAction`
  event prefixed `God:`):

  | Command | Effect | CLI lever (corp by seeding slot, 0-based) |
  | --- | --- | --- |
  | `FundCorp { corp, amount }` | coins into the treasury from nowhere | `fund_corp=2:200000` |
  | `BankruptCorp(corp)` | treasury to -1, `negative_since` back-dated past `bankrupt_days`, then `corps::bankrupt` at once (a day's takings cannot save it before midnight) | `bankrupt_corp=0` |
  | `SeizeBuildings { owner, to }` | every building of an owner (`None` = the city) to another (agent, gang, corp, city) via `corps::move_building`; the losing corp takes one `BuildingLost`, as when a rival's Acquire takes one | `seize_corp=3:gang0` / `:city` / `:<slot>` |
  | `KillExec(corp)` | the exec dies (Violence, no killer) | `kill_exec=0` |
  | `KillStaff(corp)` | every non-exec employee dies | `kill_staff=0` |
  | `SetCorpOrder { corp, order, niche, days }` | pins the order; the rescores keep the trace but do not switch until it lapses (new `Corp.pinned_until`, serde-defaulted); entering or leaving Squeeze applies D22 as the brain does | `corp_order=3:Squeeze:30`, `corp_order=1:Squeeze:Housing:30` |
  | `StrikeNow(corp)` | the corp's workers walk out of their next shift, as a Street strike (D35), whatever the unrest; cooldown untouched | `strike=0` |
  | `WipeTreasuries` | every corp's treasury to 0 | `wipe_corps=1` |

- **Cross-check.** `god_bankrupt_food_leader` and `god_rent_shock` were re-run through the CLI
  (`--lever day=45:bankrupt_corp=0`; `--lever day=45:city_rent=4/4/4` plus five `corp_order` levers)
  and `tools/analyze_run.py`. Days 45-51 matched the tests to the digit: bankrupt 88.14 thefts/day,
  2.43 acquisitions/day, 141.71 Corp-class adults; rent shock 169.29 thefts/day, 4.43 rent
  shortfalls/day, 0 evictions. `analyze_run` on the rent shock: 19,610 thefts in 105 days (8,856 in
  the bankrupt run), price max 12, `homeless_max=0`.

### Reading "reacted"

Seed 42 is chaotic: any shock that touches agents perturbs the gangs' order sequence within a day or
two, and gang member counts have zero baseline variance, so a 0.3-member difference passes. **Every
scenario passes, and in two (`god_kill_exec`, `god_takeover_by_wealth`) the only "reactions" are that
butterfly noise.** The verdicts below separate a reaction from noise by hand. A sharper check (ignore
gang series whose difference is below one member; require a corp or class series) is a follow-up.

## The control

Price is **3 at every Market on every day** of the run; nothing in M11 moves it (calibration). The
corps settle into near-permanent orders: Nutrix Secure (Food), Greenline Secure, Habitat Hunker,
Vatra flapping Secure/Hunker every few days, the Security corps decaying from Squeeze to Hunker as
their guards quit. Nutrix buys a Farm from Vatra twice (days 54, 81); Habitat buys one Block from
Militech. Evictions ~0.1/day, every evictee re-housed the same night, **zero Dregs and zero homeless
on every day**. No strikes (Street unrest 0.34 against a threshold it never reaches). Kessler loses
exactly 50 coins every 5 days in every run: 20 Blocks, no income at all.

## god_bankrupt_food_leader: bankrupt Nutrix (6 Farms, 1 Market, 4 Bars)

| per-day mean | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- | --- |
| acquisitions | 0.07 | 2.43 | 1.27 | 4.30 | 0.03 | 0.10 |
| bankruptcies | 0.07 | 0.29 | 0.23 | 0.23 | 0 | 0 |
| incorporations | 0 | 0.14 | 0.20 | 0.27 | 0 | 0 |
| Corp-class adults | 197 | 142 | 122 | 68 | 197 | 191 |
| owned by agents | 1 | 6.9 | 6.6 | 16.2 | 1 | 3.2 |
| starvation | 0.73 | 0.86 | 0.73 | 0.67 | 0.67 | 0.73 |
| price mean | 3 | 3 | 3 | 3.03 | 3 | 3 |

**What happened.** The fire sale went to the highest purse above its reserve: Greenline bought 3
Farms, the city took 3 Farms and the Market at half price, four agents bought the four Bars. Nutrix's
98 workers lost their corp jobs at once (Corp class 197 to 142). Greenline had bought beyond its
means and went bankrupt **five days later** (`bankrupt_days = 3`); the dead corp's ex-exec, Freya
Singh, bought its 5 Farms with her wallet, incorporated Singh Holdings (treasury 0), went bankrupt in
four days, bought them back, re-incorporated, went bankrupt again. Lena Voss (Greenline's ex-exec,
who inherited its estate) bought four, incorporated Voss Holdings, which Undercut and then
**Acquired a Farm from Vatra**. By day 104 Vatra and Militech are gone too, the city owns 231
buildings (157 in the control) and 14 Bankrupt events have fired. No famine, no price move, no law
or gang reaction beyond noise; Vatra took Lobby once (day 61).

**Interesting? Split and churn, not panic.** The right shape (rivals, ex-execs and the city carve up
the corpse) wrapped around a wrong loop: incorporations born with treasury 0 die within days and the
same founder buys the same buildings back. Calibration artefact in part (`bankrupt_days = 3`), design
gap in part (see Gaps).

## god_fund_corp_to_monopoly: give Greenline (2 Farms, 1 Market) 200,000

**What happened.** Greenline took Grow the same day and **built Bars** (four in 21 days, `growing in
Food`), never a Farm or a Market, then flapped Undercut/Secure, at one point five switches in a
single day (`0.24 vs 0.24`). It took Acquire once (day 57) and bought nothing. Its treasury ended at
200,596: it spent 7,000 of 200,000 in 60 days. One monopoly-day (day 59) and no monopoly after;
nobody Undercut it because nobody felt it (the price never moved). Strikes 0, gangs untouched.
Employment +9 (its Bars).

**Interesting? Nothing.** Money is not a weapon for a corp: Grow builds the cheapest niche building,
Acquire is limited to the weakest rival's cheapest building with a cooldown, and nothing scales with
the war chest. A 200,000-coin treasury should be a raid prize (`hoard_heat = 5000`) and was not raided.

## god_kill_exec: kill Nutrix's exec (Freya Singh)

**What happened.** A replacement exec was in place before the day ended (the greediest employee:
greed 0.96 against 0.99, lawfulness 0.30 against 0.69, courage 0.60 against 0.69). Nutrix's order
history is identical to the control's to day 103. No `CorpShock`: `EmployeeKilled` fires only for an
agent with a `Job` at a corp building, and execs have none. The 7-day "reactions" are gang-order and
member noise.

**Interesting? Nothing, as expected.** Leadership change is not a shock yet. A much less lawful exec
changed nothing because the corp brain reads greed and little else.

## god_kill_staff: kill Nutrix's 98 employees

**What happened.** Population -99. **Every post was re-filled by the end of the same day** (98
employees at the day-45 snapshot): job search re-hired the whole roster from the unemployed. Market
stock and price did not move. The Corp class dipped (186 vs 200) and Corp submission rose (0.58 vs
0.46). Nutrix bled faster afterwards (4,153 mean treasury over 75-105 against 14,128) and went
bankrupt on day 101 alongside Vatra; whether that is the shock or the trajectory is unclear.

**Interesting? Recover, too fast.** A massacre of a corp's workforce costs it nothing for a week. 98
`EmployeeKilled` shocks reached the brain and forced a rethink, and the rethink kept Secure: the brain
has no input that a dead workforce changes (Market sales and cash are untouched once the posts
refill). No fear, no hiring premium, no Hunker.

## god_seize_to_gang: every Block of Habitat (121) to The Hollow

| per-day mean | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- | --- |
| Hollow members | 16 | 52.6 | 57.9 | 59.8 | 19.4 | 47.8 |
| Hollow territory | 33 | 54 | 95 | 162 | 53 | 119 |
| Hollow treasury | 2.7 | 139 | 154 | 161 | 16 | 14 |
| Ninefold territory | 31 | 37 | 20 | 11 | 57 | 43 |
| assaults | 11.7 | 25.1 | 25.3 | 17.2 | 15.3 | 25.9 |
| extortions | 10.8 | 29.3 | 25.1 | 15.9 | 14.5 | 30.7 |

**What happened.** Rent now flows to the gang (`OwnerKind::Gang` charges `base[tier]`), which paid
stipends and **tripled its membership in a week**, to the 60-member cap. Assaults and extortion
doubled in the first week, then The Hollow took Ninefold's turf (Ninefold 57 to 20 territory). Habitat,
left with nothing, **was dissolved the same day** (`buildings.is_empty()`); its 6,495 coins went to
its exec Winifred Okafor, who founded two Bars, incorporated, went bankrupt in 11 days, founded a Block
and died. The law's posture did not change beyond noise; arrests flat.

**Interesting? Violence and expansion.** The strongest reaction in the suite: property is power for a
gang. But the dissolution is wrong (a corp robbed of its Blocks should fight, Lobby, or sue, not
vanish), and the law never notices a gang becoming the city's second-largest landlord.

## god_rent_shock: city rent 4/4/4, every Housing corp pinned to Squeeze (Housing) for 30 days

The spec's failure spiral, link by link (45-75 mean vs control; MOVED = more than 2 baseline sd):

| link | shocked | control | |
| --- | --- | --- | --- |
| 1 evictions | 0.13 | 0.13 | - |
| rent shortfalls | 5.40 | 1.93 | MOVED |
| 2 Dregs | 0 | 0 | - |
| 3 gang joins by evictees | 0.07 | 0.03 | (noise) |
| gang members, both gangs | 55.4 | 37.4 | MOVED |
| 4 raids | 0.07 | 0.07 | - |
| 4 thefts | 228 | 93 | MOVED |
| 5 price mean | 3.06 | 3.00 | MOVED (3.82 over 75-105, max 12) |
| 6 Street unrest | 0.33 | 0.34 | - |
| 6 strikes | 0 | 0 | - |
| 7 Lobby orders | 0.03 | 0 | one |
| population | 1,942 | 1,957 | MOVED |

**What happened.** The pins held (Squeeze lifted every Housing price level to 1.14 and cut eviction
days). Rent shortfalls tripled, yet **evictions did not move and no one became a Dreg**: shortfalls
build arrears slowly, and an evictee is re-housed the same night. The spiral routed around eviction:
residents **stole to pay rent** (thefts 2.5x in the month, 3.4x in the month after), The Hollow
grew to 57 members and 166 territory, food stock fell and the price climbed late (mean 3.8 over
75-105). Unrest did not move (rent is not an input), so no strike, and Lobby fired once. Vatra, pinned
in Housing while its Food side bled, went bankrupt on day 74; Greenline on day 93.

**Interesting? Violence, by the back door.** The chain closes rent -> theft -> recruitment -> price,
but links 1-2 (evictions, Dregs) and 6-7 (unrest, strike, Lobby) do not fire. The eviction ->
recruitment link the spec drew does not exist at this calibration.

## god_wipe_corps: every corp treasury to 0

| per-day mean | base | 45-52 | 45-75 | 75-105 | ctl 75-105 |
| --- | --- | --- | --- | --- | --- |
| owned by city / corps / agents | 162/321/1 | 258/221/5 | 352/110/22 | 372/68/45 | 158/323/3 |
| acquisitions | 0.07 | 42.7 | 20.1 | 8.9 | 0.10 |
| bankruptcies | 0.07 | 0.86 | 0.67 | 0.67 | 0 |
| incorporations | 0 | 0.57 | 0.57 | 0.67 | 0 |
| Corp-class adults | 197 | 130 | 60 | 34 | 191 |
| city treasury | 43,584 | 35,099 | 22,509 | 20,972 | 44,476 |
| thefts | 76 | 97 | 98 | 141 | 101 |

**What happened.** Every corp took Hunker the same day. Six of eight were bankrupt on day 48 (three
days in the red), Greenline on day 100; Kessler alone survived, frozen at exactly 0 for 60 days (its
only cost is the exec's wage, paid only from a positive purse). With every corp purse at or below its reserve, the fire sales
went to **wallets and the city at half price**: the city ended with 365 of ~500 buildings and paid
20,000 coins of Treasury to absorb the estates' debts. A few rich agents (Freya Singh bought 7 Blocks
on day 48) incorporated, went bankrupt, bought back, re-incorporated: 37 incorporations and 40
bankruptcies in 60 days. Employment held (238: the city runs what it takes), starvation flat, thefts
up 40% late.

**Interesting? Collapse into nationalisation.** A bankruptcy wave hands the city to the city, not to
one buyer. Nobody raided the dying corps; no corp tried to borrow, sell assets early or merge.

## god_nationalise_food: the city buys all 12 Farms and 3 Markets (15,000, Treasury topped up first)

**What happened.** Greenline, owning nothing else, **was dissolved the same day** with 8,605 coins
paid to its exec. Nutrix (left with 4 Bars) took Undercut in Food with no Market to undercut at, then
Acquire, buying a Bar; its treasury rose to 27,900 (no payroll). Vatra took Grow in Housing and built a
Block. Food workers became city employees (Nutrix 97 -> 10 employees, Corp class 200 -> 28, Street
1,757 -> 1,928). Market stock fell by a third (4,736 -> 3,068) and stayed there; price unchanged;
starvation flat. **No Hunker, no Lobby** from the food corps: they had nothing left to protect, so
nothing to lobby for.

**Interesting? Nothing from the corps.** The class map flipped overnight with no unrest consequence.
The lower stock under city ownership is worth a look (hauling or staffing differences between owners).

## god_no_evictions_forever: `no_city_evictions` plus rent cap 1

**What happened.** Landlords bled: Habitat 4,814 -> 2,545 (mean over 75-105 vs control), Stackwell and
Militech down 10-20%. Rent shortfalls and evictions barely changed because both are already near zero.
No landlord reacted by order (Squeeze needs a price it is now capped from; nobody Lobbied against the
cap). Nobody is homeless in either run, so "does anyone still get housed" has no subject.

**Interesting? Nothing.** The cap is a slow transfer from corps to tenants that no one notices.

## god_takeover_by_wealth: 1,000,000 coins to one agent, pinned to Full detail

Two variants: the **richest** eligible adult (Cato Delacroix, greed 0.24) and the **greediest**
(Vale Mbeki, greed 0.99). Both were Statistical-tier; in the first, unpinned run Cato never thought,
so never founded. Pinned:

- **Richest:** founded nothing in 60 days. Spent ~400 coins.
- **Greediest:** registered one Bar on day 45, nothing for 48 days, a second building on day 93,
  incorporated Mbeki Holdings (treasury 0) which went bankrupt within 8 days; she bought both buildings
  back. Wallet on day 104: 998,193. **Peak holding: 2 buildings.**

No corp or gang reacted to her (the reactions are noise).

**Verdict: no, you cannot buy the city.** The Found goal scores `U(wealth)`, so a million coins
*suppresses* founding; there is a 10-day founding cooldown; an agent has no action to buy an existing
building from its owner; incorporation strands the founder's wealth outside the corp. The cap is
structural, as force was in v1.

## Gaps

### Commands that could not express a scenario

- **Buy a building as an agent.** No `BuyBuilding { buyer, building, price }`; the only agent route is
  Found (one Lot at a time) or a fire sale. "Can a player buy the city" cannot be asked properly.
- **Move coins between an agent and their corp.** No `Invest`/`Dividend`; the takeover's million never
  reaches Mbeki Holdings.
- **Force a goal.** "Grant the Found goal" has no command; pinning only stops LOD demotion.
- **Raise a corp's prices** directly (`SetPriceLevel`), to test Undercut and strikes without a pin.
- **Seize one building**, not an owner's whole estate; **transfer a corp's exec** (board coup).
- **Make the market move.** Price is 3 at every Market on every day of every run; no lever moves it
  except starvation of stock, which no scenario achieved.

### Reactions that did not happen but should

- **A corp losing its exec never panics.** Same-day replacement, no shock, identical orders, even when
  the successor's lawfulness halves. Needs a `LeaderChanged` shock and a brain that reads more of the
  Personality than greed (the v1 gang gap, repeated).
- **A massacre of a corp's staff costs it nothing.** Re-hired the same day; the `EmployeeKilled`
  shocks force a rethink that changes nothing, because no brain input reads staff loss; no fear among
  the new hires, no wage premium.
- **Nobody Undercuts a rich rival, and money buys nothing.** 200,000 coins built four Bars. Grow should
  scale with cash; Acquire should target the leader's best asset, not only the weakest rival's
  cheapest; a hoard should draw a raid.
- **Corps vanish instead of fighting.** A corp stripped of its buildings (seizure, nationalisation) is
  dissolved the same midnight, treasury and all. It should be able to Lobby, sue, buy back, or hire
  muscle.
- **Incorporation churn.** A corp born of incorporation has treasury 0, so it goes bankrupt within
  `bankrupt_days` and the founder buys its buildings back: up to 40 bankruptcy/incorporation pairs in
  60 days. The founder should capitalise the corp, or incorporation should need a treasury.
- **A bankruptcy wave hands the city to the city.** With purses at reserve, estates go to the Treasury
  at half price (it paid 20,000 coins to absorb the debts). Nobody bids low, nobody merges, nobody
  raids the dying.
- **Evicted families never riot, because there are none.** Every evictee is re-housed the same night;
  zero homeless and zero Dregs in every scenario, including rent 4 under a 30-day Squeeze. The spec's
  link eviction -> Dreg -> gang join never fires; desperation leaks out as theft instead.
- **Rent never moves unrest.** Shortfalls tripled with Street unrest flat, so no strike, and Lobby
  fired once. Unrest should read rent burden (rent ÷ income) and arrears.
- **The law never notices a gang landlord.** The Hollow owning 121 Blocks and tripling to its member
  cap changed no posture beyond noise.
- **Order flapping on ties.** Shock rescores have no hysteresis; Greenline switched Undercut/Secure
  five times in one day at `0.24 vs 0.24`, Vatra flaps Secure/Hunker every few days in the control.
- **Zombie corp.** Kessler sits at exactly 0 for 60 days after the wipe. It earns no rent in any
  run, its Blocks cost no upkeep, and the exec's wage is `pay`, not `charge`, so it never goes
  negative and can never go bankrupt. A corp with no income should die.

### Probable calibration artefacts (re-check after the economy pass)

- Flat price 3 everywhere (no scenario can test price spikes, Undercut wars or monopoly markups).
- `bankrupt_days = 3` (fire-sale buyers and incorporations die within a week).
- No homelessness at rent 4 (re-housing threshold `coins >= 3 × rent` is always met).
- Kessler's empty Blocks and the steady bleed of every Housing and Security corp in the control.
- Nutrix and Vatra both going bankrupt around day 100 in several runs (the control's corp treasuries
  trend down too).
- Market stock a third lower under city ownership.

### Scenarios to try after M12

- **Buy the law** (v1 carried over): a corp funds the Precinct's payroll while the city is broke.
- **A board coup:** with `Governance::Board`, bribe a majority and replace the exec; does the corp's
  order change?
- **Squat the estate:** bankrupt a Housing corp with no buyer; do Dregs and gangs squat its Blocks?
- **Gang landlord vs the law:** seize Blocks to a gang and pin Crackdown; do tenants side with the gang?
- **Corp funds a gang** against a rival corp (`FundGang` from a corp purse); does the law trace it?
- **Strike while a gang offers stipends** (`StrikeNow` plus `FundGang`): do strikers defect?
- **Hyperinflation:** `ReleaseReserve` zero and Farms nationalised then demolished; does price ever move?
- **Every v2 scenario over 10 seeds**, reporting distributions (seed 42 is chaotic enough that kill_exec
  "reacts" through gang noise alone).

## After calibration (M11 phases 5a and 5b)

Re-run on the calibrated economy (dole 4, tax 0.12, rent [1, 2, 4], prices in tenths, the estate rule, owners eligible to found from their own payroll, `found_flat` 0.1): `--test god_corps` passes 12/12. What changed against the gaps above: evictees now wait three days on the street, so Dregs exist (rare: mean 1.6 on seed 42, none before day 57); a millionaire still founds two buildings and incorporates ("Quarry Holdings", 999k in the treasury, Undercut), since money still buys nothing beyond `found_cost`; incorporated corps start with the founder's savings and 14 days free of upkeep, yet the ordinary two-Bar Holdings still go bankrupt 30-40 days later; bankrupting Nutrix now shakes the other Food corps' orders within a week and the law swings to Garrison. The zombie corp, the gang landlord the law ignores and order flapping on ties are not addressed.

## After M13 (2026-10-06)

`--test god_corps` 12/12 pass. Seed 42; per-day means over days 45-75 against the control (ctl), later window named. The previous note is "After calibration" above. The control is the same world as the v1 control (assaults 25.9, thefts 50.6, extortions 23.6, price 3.1 rising to 4.2 late).

- **bankrupt_food_leader.** Nutrix gone from day 45; the city forecloses 9 buildings and one agent buys a Bar for 200. Acquisitions 1.43 vs 0.14 a day in week one, assaults 31.3 vs 25.3, homeless 47 vs 31 late, price 3.7 vs 4.2 late. Same as the calibration note.
- **fund_corp_to_monopoly.** Greenline's 200,000 buys 2 to 8.6 buildings (4.6 in days 45-75) and employees 14 to 70, but it goes Secure, not Grow, and keeps 188,000 unspent. Same: money still buys little. Price 3.9 vs 4.2.
- **kill_exec.** Nutrix leaves Secure for Acquire on day 46 (the control does it on day 87), one worker strikes on day 45; corp order changes 0.77 vs 0.67. Same: a faint shock, no panic.
- **kill_staff.** Re-hired at once; assaults 18.7 vs 25.9, extortions 13.3 vs 23.6, Nutrix treasury 2,532 vs 5,250, corp order changes 0.33 vs 0.67 and 0.00 late (it sits in Hunker from day 48). Weaker orders than before, a bit more cost.
- **seize_to_gang.** 121 Blocks to The Hollow; extortions 6.3 vs 13.3 in week one, 42 vs 51.5 late. Habitat is left with 0 coins. Same: the law does not react to a gang landlord; g1 territory 93 vs 47 is gang noise.
- **rent_shock.** thefts 74 vs 50.6 (152 vs 117 late), rent shortfalls 16.5 vs 11.2, homeless 44.6 vs 38.1, Habitat treasury 5,493 vs 1,667; Street unrest 0.40 vs 0.37 and evictions 0.67 vs 0.47. Same: theft rises, no eviction wave, unrest barely moves.
- **wipe_corps.** Bankruptcies 0.29 vs 0 and acquisitions 0.86 vs 0.14 in week one; new corps 1.0 vs 2.0. Kessler holds 14-32 coins rather than exactly 0, so the zombie gap is not reproduced as stated. Same otherwise.
- **nationalise_food.** Price pinned at 3.00 all run (control 4.2 late), but thefts are 51.6 vs 117 late and extortions 71.8 vs 51.5. Same price story; the theft drop is new.
- **no_evictions_forever.** Acquisitions 4.13 vs 0.10 a day, Habitat goes from 120 Blocks to 48 to 0, corps 324 to 188. Price 4.8 vs 4.2 late. Stronger: the landlord sells out, not merely loses rent.
- **takeover_by_wealth (and _greedy).** New corps 2.0 a day late vs 0.17 (the millionaire incorporates), price 5.6 (6.2 greedy) vs 4.2 late. Same as the calibration note, with a higher price.
- Vehicles, chrome, stims and robots: no visible effect. The only trace is an agent founding a Ripperdoc for 200 in the fund_corp run; no vehicle or robot row exists.

### Gaps (after M13)

- **Closed by M13:** none of the listed gaps. No corp scenario reacts to assets.
- **Still open:** exec loss without a panic, staff loss costing nothing, nobody Undercutting a rich rival, corps dissolving instead of fighting, incorporation churn, unrest ignoring rent, the law ignoring a gang landlord, no `BuyBuilding`/`Invest` commands, and no board coup. kill_exec's gang noise is back (g1 territory 105 vs 47 late).
- **New:** a nationalised Food sector halves late theft (51.6 vs 117). Worth a look: it may be the price pin, or an artefact of stock.
