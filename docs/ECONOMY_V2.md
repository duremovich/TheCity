# The Real economy: wages from revenue, the World as the market, no dole

Companion to `SPEC.md`, `M11_OWNERSHIP.md`, `M13_ASSETS.md`, `M14_VIRT.md`, `M17_OUTSIDE.md`, `LIFE_L2.md`, `SHADOW_V2.md`, `GOD_SCENARIOS_V7.md`, `BIG_PICTURE_2026-10-07.md`, `ROADMAP_POST_M14.md` (addenda 17 and 18) and `VISION.md`. L2 gave the city jobs, venues, a Fab and an export hook, and left its money where M11 put it: **the population buys food, the Food corps hand ~74 % of that revenue to the Treasury as "upkeep", and the Treasury hands it back as the dole.** That loop is the economy; wages are a third of it. This milestone replaces it: **wages are paid out of revenue, revenue comes from residents who have wages and from an outside World that buys what the city makes, the Treasury lives on taxes and spends on guards, sweepers and the Reserve, and the only help for the jobless is a charity on donations.** A Vats farmhand earns 9 a day because Nutrix sells a third of its harvest to the World at 3.4 a unit; when the World's appetite for food doubles in a drought abroad, Nutrix's farms post six jobs, its export pays for a new Farm on day 41, the Civic Market's price climbs from 4 to 6 because the shelf now competes with the outside buyer, and a Sump household with no earner eats at the Purist Chapel's kitchen, which can feed forty a day on what its congregation and one rich donor put in the box. Where this document and the earlier ones disagree, this one wins for the Real economy.

Addendum 18 names the milestone: *built from M17's ledger slice (the World account with per-good demand, prices and caps as levers) plus the removal of the dole and public works, charities on donations, and the jobs-from-revenue rules; measured by the L2 gate's economy findings turned into asserts.* It slots before M16a (Dylan to confirm), because M16a's prices ("price over wage", "a Fixer's weekly income within 0.5-2x a Bar owner's") need wages and savings that mean something.

Scale: 2,000 residents on the 256 x 192 map; the milestone lands on main after the L2 shadow fixes (62e8779). Identifiers marked *new* do not exist at HEAD; every other identifier named here does. **Every number below is a placeholder for calibration unless it is labelled measured.** Measured numbers are main at 62e8779, seed 42, 120 days, `citysim-cli run --report --events` (the "base" run) and the same with `--lever day=0:dole_per_day=0` (the "dole-off" run); one sim year is 120 days.

Decisions taken by the drafting agent (overturnable, listed so they are cheap to overturn):

| Question | Decision |
| --- | --- |
| What "magic money" is | **Any unconditional transfer from a hoard to a wallet**: the base dole, the unpaid-worker and absentee dole, public works sized by the Treasury's surplus, `reserve_release` (free food), the Treasury's scavenge coin with nothing sold behind it. Money creation is not the problem (the city mints almost nothing today, § 0); the dole is a redistribution pump disguised as a city service. All of it goes. |
| Where coins enter | **The World's purchases** (`Flow::Export`) of Food, Parts, Data and, from phase 2, chrome and decks as Parts-equivalents; **immigrants' pockets** (15 coins today, minted at spawn; kept, booked as an inbound crossing); later M17's remittances inbound, subsidies and the State's grant. Nothing else. |
| Where coins leave | **The World's sales** (*new* `Flow::Import` re-pointed to the World account, M17's `import_to_outside` switch on): production inputs (feedstock, power, spares), food bought abroad when the city's price passes the World's ask, Parts a seller lacks (`part_credit`); **emigrants' wallets** (destroyed today; booked as an outbound crossing). |
| Upkeep | **Re-based.** `[corps] upkeep` (Farm 245, Market 600 a day, M11 D4: "sized from the ledger" to carry the Treasury) is not a running cost, it is a 70 % levy that funds the dole. It splits into **inputs paid to the World per unit produced** (`input_per_food`, `input_per_part`, `input_per_data`, *new*) and a small **property rate** to the Treasury per building (*new* `[treasury] property_rate`). |
| The Treasury | **Taxes and customs only**: wage tax, owner-revenue tax, property rate, customs on imports (`customs_rate`, *new*), fines. It spends on guards, sanitation, gravediggers, jail food, the Reserve's purchases and school meals. Its band (`[budget] band`) moves **the tax rate**, not public works or upkeep: above the band the rate falls a step, below it rises, inside `[tax_min, tax_max]`. A god's `SetTaxRate` pins it. |
| The opening hoard | Treasury seeded at a working balance (`treasury_initial` 35,000 → 12,000); the 23,000 difference seeded into corp treasuries as **working capital**, pro rata to their day-0 payroll, so the first week's wages exist before the first week's revenue. Total coins at seed unchanged. |
| The World account | M17's `OutsideFaction` of kind `World` (L2 built it at `WORLD_ACCOUNT` 1069), extended with a **book per good**: a bid curve (what it pays for the q-th unit today), an ask (what it sells for), a daily cap, an appetite random walk, and a 30-day ring of quantities. It is the only outside account this milestone builds. |
| The demand curve | **Linear inverse demand per day, paid per tranche**: unit i of the day's sales fetches `bid_ref × market × appetite × (1 − slope × i ÷ cap)`, floored at `bid_floor`. Selling more pays less per unit, so the World is a market, not an ATM; the cap stops a corp from draining it. |
| The supply side | **The World sells at an ask**: `ask = bid_ref × market × (1 + spread)`, uncapped, paid by the buyer to the World (`Flow::Import`, a leak, customs on top). A Market whose shelf price passes `ask + haul_margin` imports its restock instead of starving. That ceiling, not a Reserve release, is the famine bound, and its lever (`ask_mult`) is the famine lever. |
| Who sells to the World | **The owner, by price**: a Farm's haul goes to its own Market, then to the higher of a rival Market's wholesale and the World's marginal bid, then the Reserve. A corp exports while the World's bid beats the city's wholesale, so food leaves the city in a good year abroad and the city's price rises to keep it: the real tension, wanted. |
| Wages | **A payroll from revenue**: each corp's daily `wage_mult` is set so its payroll tracks `labour_share × trailing revenue` (7-day mean), bounded by `[wage_floor_mult, wage_cap_mult]` and moving at most `wage_step` a day; unfilled vacancies push it up (a shortage), a queue of applicants does not push it down below the floor. The per-role config wage stays as the role's relative weight. |
| Hiring | **From margin and demand**: a corp posts a vacancy when payroll ÷ (labour share × revenue) < `hire_below` for 3 days and its niche demand (now including unfilled export orders) > `hire_demand`; it stops replacing quitters above `fire_above`, and lays off the newest above it for 7 days. Agents and gangs as employers keep M11's rules. |
| Grow | **Export demand counts** (god v7 gap 4): a Food corp's `demand` input reads `max(market sell-through, World fill ratio)`; a Tech corp's reads Parts and Data orders too; the Grow gate "lots & cash" adds "or a refit target" so a rich corp without Lots refits a derelict (L2 `founding::refit`). |
| Prices | `price_for_stock` stays (the shelf curve); it gains a **floor** at the owner's unit cost (`wholesale + input_per_food`) and the **import ceiling** (`ask + haul_margin`). Corp `price_level` stays. |
| The jobless | **No income but** work, crime, kin (the Home pantry, M11 `StoreFood`, and friends' loans, `social::repay_debts`), scavenging (the Recycler buys scrap from its Parts sales, not from the Treasury), gang stipends, and charity. Starvation is the pressure; the gate bounds it by the levers' settings, not by a floor. |
| School meals | **Kept** (`demography.school_meals`, children eat from the Reserve when the pantry is empty), paid by the Treasury at wholesale: a child cannot earn, and a starving child is a broken sim, not a brutal one. The only exception to addendum 17; overturnable. |
| Charity | **A building kind `Mission`** (*new*; label "Soup Kitchen", and the Purist Chapel gains the same kitchen as a function of its Hideout) whose purse is fed only by `Flow::Donate` (*new*) from agents, corps, gangs and the player; it spends per coin on a meal, a cot or a clinic hour, inside a reach in its district. |
| Gangs as employers | Unchanged (stipends, tribute, fronts), and now a real alternative: with no dole, a gang's stipend is the jobless's best offer. Desistance's "employed" factor reads a paid Job, as today. |
| M17 slice | The World account with books, the conservation identity, `import_to_outside` on, `Outside.{inbound, outbound, minted}`, immigrant and emigrant crossings. **Waits**: parents, remittances, reinforcement, scrip, sanctions, syndicates, the State, uplinks (§ 6). |
| The off switch | `[economy2] enabled` (*new*) and `--econ-off` reproduce the L2-closing city byte for byte (the `--l2-off` precedent); each phase has its own section switch. |

## Goals and acceptance

The city should read as a place where a wage is what a business can pay, a business is what its customers and the World will buy, and a jobless Sump resident lives on crime, family, a gang or a soup kitchen. The target spiral: **the World's food appetite rises → Nutrix's farms export above the city's wholesale and post vacancies → farmhands hired off the street draw a wage set from Nutrix's revenue → they buy food and a night at the Arcade → the Civic Market's price rises as the shelf competes with the export → Greenline undercuts and grows a Farm → the city's balance of trade turns positive and wallets hold more by day 60 than day 10 → the appetite falls in Autumn, Nutrix's revenue ÷ payroll crosses the band and it lays off the newest six → they go to the Purist Chapel's kitchen, two join Ninefold, one steals and is jailed.** On seeds 42-44 (majority) and 42-47 (means and existence), 2,000 residents, 120 days, gate `test_econ_real_economy_seed_42` (*new*):

**Asserted (mechanism and sanity):**

- no magic money: `flow_dole` is 0 every day; no public-works hire exists; `reserve_release` never fires; the Treasury pays wages only to its own roles (guard, sanitation, gravedigger, its Feed and Hall staff);
- the conservation identity holds every day to the coin: `total_coins + Σ outside treasuries − outside.minted` constant, with immigrant and emigrant crossings booked (`outside.inbound`, `outside.outbound`);
- **wages are the main income**: Σ wages ≥ 60 % of the population's inflows (wages, stipends, tribute, theft and robbery takes, charity, gambling wins, scavenging, loans) over days 31-120 on every seed;
- **the Treasury runs on taxes**: its inflows are ≥ 95 % tax, customs, property rate and fines; it is inside `[0, 3 × band_hi]` every day; the tax rate moves at least once and stays inside `[tax_min, tax_max]`;
- **the World trades**: `flow_export` > 0 and `flow_import_out` (*new*) > 0 every week; ≥ 1 corp vacancy posted on the export rule; ≥ 1 Grow or refit whose demand input came from the World's fill ratio (existence over 42-47);
- **starvation bounded by the levers**: starvation deaths ≤ the scaled v1 bound with the World at default appetite; the god twin with appetite x0.3 and ask x2 starves more (the lever bites) and its population on day 120 is ≥ 1,200 (the city does not empty);
- **no collapse into a monopoly**: ≥ 2 Food sellers alive on day 120 (majority), ≥ 5 of 9 seeded corps or their successors alive, no corp with a 0.8 share of two niches;
- charity: ≥ 1 `Donated` from an agent, a corp and a gang each over 42-47; every Mission meal is paid from its purse (exactly); the god "big donor" scenario lowers its district's starving and Dregs against the twin (§ 4);
- the year: 365 days on seeds 42 and 43 hold the L2 year bounds (population ≥ 1,600 on day 365, starvation in year 3 ≤ 1.5 x year 1 + 5) and the Treasury and the World's cumulative net crossing stay inside their bounds;
- the v1, M8-M15 and L2 gates still pass (each bullet that moves is re-read under the doctrine: tested against a variant before moving, recorded under "Implemented: deviations").

**Printed findings (calibration bands, not asserted):** employed share of adults on day 60 45-60 %; mean gross wage on day 60 8-10 (food at 4); Σ wages ÷ population spending 0.8-1.1; balance of trade over 120 days within ±5 % of `total_coins`; outside coins in as a share of corp revenue 10-30 %; wallet Gini on day 120 0.55-0.75 and top-decile share 0.4-0.6 (unequal by structure, not by accident); population wallets on day 120 ≥ 25k; Food price 3-6 outside Winter, ≤ the import ceiling always; Dregs share of adults 5-15 %; Mission meals 30-150 a day city-wide; M13's tier-1 good at 10-20 days of a median wage (the price print, as L2 phase 5).

## 0. The circuit today (measured)

Coins per day on seed 42, means over the window. `flow_public_works` is a subset of `flow_wages`; the city/private split of wages is derived from the Treasury's balance (its Δ is +217 a day over days 91-120), so it is an estimate.

| flow (coins/day) | days 1-10 | 31-60 | 91-120 | from → to |
| --- | --- | --- | --- | --- |
| food bought | 7,205 | 7,217 | 6,970 | wallets → Market owners (taxed 12 %) |
| **dole** | 5,655 | 5,641 | **5,399** | Treasury → wallets |
| **upkeep** | 5,143 | 5,158 | **5,144** | owners → Treasury (Farms 245, Markets 600 each) |
| wages (all) | 2,135 | 2,496 | 2,651 | employers → workers |
| of which public works | 84 | 714 | 833 | Treasury → sweepers |
| of which other city roles (est.) | ~500 | ~500 | ~500 | Treasury → guards, sweepers, diggers |
| tax | 1,764 | 1,319 | 1,228 | everyone → Treasury |
| wholesale (inter-owner) | 2,188 | 1,097 | 1,716 | Market owners → Farm owners |
| overflow (Reserve buys surplus) | 1,747 | 877 | 726 | Treasury → Farm owners |
| restock (Reserve sells) | 417 | 220 | 989 | Market owners → Treasury |
| import (`Flow::Import`) | 1,734 | 309 | 174 | sellers → Treasury (customs, not a leak) |
| rent | 698 | 704 | 727 | tenants → landlords |
| assets, asset upkeep, parts | 3,129 | 843 | 879 | buyers → sellers, owners → Treasury |
| leisure, gamble (net) | 2,216 | 142 | 228 | wallets → venue owners |
| gang income | 30 | 231 | 783 | victims, fronts → gangs |
| export | 0 | 0 | 0 | the World is closed (`[export] enabled = false`) |

Who holds the stock (coins): day 1: Treasury 46.4k, wallets 52.7k, corps 36.6k (≈ 135.7k); day 120: **Treasury 78.3k (58 %)**, corps 42.0k (31 %; Nutrix 14.3k and Arasaka 15.9k hold two thirds of it, Militech and Zetatech bankrupt), **wallets 14.5k (11 %)**, gangs and loot ≈ 2.5k by difference. Wallet Gini 0.35 → 0.84, top decile 0.30 → 0.77. Employed 388 → 565 (19 → 28 % of adults), 119 of them public works. Starvation deaths 0, `Starving` events 9,668, thefts 10.8k, violent deaths 104. Every surviving corp holds `Secure` or `Hunker` on day 120.

**The pump.** The city is nearly closed: immigrants mint 15 coins each (100 arrived, 1.5k), emigrants destroy their wallets (none left), imports and customs land in the Treasury, export is off. So the dole is not printed money; it is this loop, at ~5.2k a day:

```
            food 7.0k                         upkeep 5.1k
  wallets ──────────────▶ Food corps (Markets) ──────────────▶ Treasury (78k hoard)
     ▲                         │ wages ~1.3k                     │  │ dole 5.4k
     │                         ▼                                 │  │ city wages ~1.3k
     └──────────── workers ◀───┘                                 │  │ overflow 0.7k → Farms
     └──────────────────────────────────────────────────────────┘  ▼
```

M11 D4 built it on purpose ("the Treasury's return flow is building upkeep ... sized from the ledger, since food revenue now goes to the Market owners"). A Food corp's revenue is three quarters a tax; its wages are a fifth of what the Treasury pays out for nothing.

**The dole-off run** (`dole_per_day=0` from day 0, everything else unchanged) shows what the pump hides:

| day | population | employed | homeless | wallets | Treasury | food bought/day | thefts/day | mean mood |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 2,000 | 388 | 80 | 47.9k | 51.3k | 328 | 4 | 0.21 |
| 9 | 2,009 | 427 | 70 | 7.1k | 109.7k | 2,419 | 1,204 | −0.28 |
| 21 | 1,827 | 475 | 955 | 2.5k | 125.7k | 1,612 | 1,183 | −0.43 |
| 30 | 1,080 | 501 | 570 | 4.0k | 128.5k | 2,032 | 320 | −0.35 |
| 60 | 685 | 541 | 177 | 3.7k | 131.7k | ~2,000 | 38 | — |
| 120 | 669 | 539 | 102 | 2.7k | **130.1k** | 1,945 | 99 | — |

Totals: **1,307 emigrated**, 1,714 evicted, 43 starved, 27.7k `Starving` events, thefts 32.0k (3x), arrests 2,951. **Seven of nine seeded corps bankrupt by day 63** (Greenline day 12, Vatra 13, Militech 25, Kessler 26, Nutrix 28, Zetatech 38, Arasaka 63; Habitat and Stackwell alone survive) and 166 buildings foreclosed to the City, which ends as the employer of most of the 539 who stayed. The Treasury absorbs **95 % of all coins** (130k of ~136k) because upkeep and tax keep flowing in and nothing flows out; the band's upkeep cut reached 0.65 on day 60 and 0.30 by day 90, a month after the corps it was meant to save were dead. Read: switching the dole off without rebuilding the circuit is not brutality, it is a black hole. Starvation was not the killer; emigration was (the miserable leave), and with no inflow the city shrinks to the size its Treasury's payroll can feed.

## 1. The circuit without magic money

### The boundary

| enters the city | today | target (placeholder, coins/day at day 60) |
| --- | --- | --- |
| the World buys Food | 0 (export off) | 600-1,000 (the Reserve's overflow today is ~350 units a day at the cap) |
| the World buys Parts, chrome, decks | 0 | 300-700 (Fabs, Recycler scrap, Clinics' and Garages' surplus) |
| the World buys Data | 0 | 200-500 (Labs; M14's market) |
| immigrants' pockets | ~28 (minted at spawn) | ~28 (booked inbound) |
| **total in** | ~28 | **1,100-2,200** |

| leaves the city | today | target |
| --- | --- | --- |
| production inputs to the World | 0 (upkeep goes to the Treasury) | 700-1,400 (`input_per_food` 0.5, `input_per_part` 4, `input_per_data` 8, Market and venue power) |
| Parts a seller lacks (`part_credit`, M13 D8) | 174 (to the Treasury) | 150-400 (to the World) |
| food imports above the ceiling | 0 | 0 normally; Winter 0-1,500 |
| emigrants' wallets | 0 (destroyed, unbooked) | small (booked outbound) |
| **total out** | — | **≈ in over a year** (balance of trade within ±5 % of the stock) |

### Inside

```
                 export 1.1-2.2k                       inputs 0.7-1.4k, Parts, Winter food
   the World ───────────────────▶ producers ─────────────────────────────────▶ the World
                                  (Farms, Fabs, Labs; Markets, venues, landlords)
                                    │  ▲                       │ taxes, property rate, customs ~1.2-1.6k
                       wages 6-8k   │  │ food 7-8k, rent,      ▼
                                    ▼  │ leisure, assets    Treasury (band) ── guards, sweepers, diggers,
                                  wallets ─────────────────────────────────── jail food, Reserve, school meals
                                    │  ▲ kin (pantry), loans, stipends, theft     (~1.2-1.6k back as wages
                                    ▼  │                                           and purchases)
                       donations ─▶ Missions ─▶ meals, cots, clinic hours (in kind, not coins)
```

The arithmetic at day 60 (placeholders): the population spends ~9k a day (food 7.5k at price 4 and ~0.95 meals a resident, rent 0.8k, leisure and assets 0.7k); producers take that plus 1.1-2.2k of export, pay `labour_share` (0.55-0.70) of it as wages (~6.3k), inputs to the World (~1k), taxes (~1.2k), and keep ~1.5k as retained earnings (Grow, Secure, research). The Treasury returns its ~1.3k as city wages and purchases. Σ wages ≈ 7.6k ≈ 85 % of spending: households close the gap with kin, savings and theft. At a mean gross wage of 8-10 that is **~800-950 jobs (40-50 % of adults)**, against 565 today (119 of them public works) and ~650-700 in L2's arithmetic.

Money stock: ~136k coins, unchanged at seed (the hoard moved, not minted). Turnover ~9k a day. A positive balance of trade grows the stock slowly; the levers (§ 3) are how the operator grows or drains it on purpose.

## 2. Removing the dole and public works

### What breaks

Measured above: ~1,400 jobless adults lose their only income on day 0 and ~5.4k a day leaves the wallets' inflow; Food corps lose three quarters of their customers' money and, under today's upkeep, go bankrupt in two to four weeks; the Treasury becomes the sink. Also gone: the unpaid-worker dole (`Job.paid_once`, `UNPAID_DOLE_DAYS`, addendum 17; still in `economy::dole_eligible` at HEAD), the absentee dole (`fixes::absentee`), `jobs_book.works` and the band's works posting, `reserve_release` (free food to a high-priced Market), and the Treasury's scavenge coin (`Flow::Scavenge` from the city).

### What replaces it

| income for a resident without a dole | mechanism | where it lands |
| --- | --- | --- |
| a job with a producer that exports | Farms, Fabs, Labs hire to fill World orders (§ 5 hiring); their staff's wages are their revenue's labour share | phase 2 |
| a job with a business that has customers | Markets, Bars, NoodleBars, Clubs, Clinics, Garages, landlords: their customers now hold wages, not a dole; the same hiring rule | phase 2 |
| kin | the Home pantry (`StoreFood`) is the household's purse in kind; an earner's meal buys feed the jobless spouse and the children; `social` loans between friends | exists |
| crime | theft, robbery, extortion, dealing: redistribution, no new coins; the law's arrests and the Jail are the brake | exists |
| a gang | stipends and the leader's `Collect` (L2): the jobless's best offer | exists |
| scavenging | the Recycler (city-owned) pays `scavenge_coins` only out of its own Parts sales (`World::scrap` → Recycler Parts → buyers and the World); a Recycler with an empty purse pays in scrap credit (nothing) | phase 3 |
| charity | Missions on donations (§ 4): meals, cots, clinic hours, in kind | phase 3 |
| the Treasury's own roles | guards, sweepers (`sanitation_count`), gravediggers, the Civic Wire, the Hall clerks | exists |

### The transition

The dole goes **last**, after the circuit that replaces it is running, and the World is open **from day 0**, so the first wages exist before the first hunger:

1. Phase 1 opens the World (bids, asks, imports to the World, the identity) with the dole still on: measure that export revenue reaches producers and that inputs leak.
2. Phase 2 re-bases upkeep into inputs and the property rate, moves the opening hoard into working capital, and puts wages and hiring on revenue, still with the dole on: the wage ÷ dole ratio should pass 1.0 before anything is removed (L2 could not get there on private jobs: 0.41-0.52).
3. Phase 3 removes the dole, public works, the unpaid and absentee dole, `reserve_release` and the Treasury's scavenge coin, adds Missions and the Treasury's tax band, and turns the gate's economy findings into asserts.

**Day 0 of a fresh world** with the milestone on: corps hold their working capital and pay the first evening's wages from it; the World's orders stand from the first midnight; opening wallets (10-40 coins x `coins_by_tier`) buy three to eight days of food; the seeded jobless reach a vacancy by day 3-10 (L1's `job_search`) or do not. A first-week starvation wave is the risk the gate watches (the L2 gate's first-week starving count printed per day); the brakes are the opening wallets and working capital, not a grace dole.

## 3. The levers of the global economy

### The World's book per good

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum ExportGood { Food, Parts, Data }   // exists (L2); chrome and decks sell as Parts x `[assets] parts_per`

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldBook {                      // new: OutsideFaction.books_by_good (World only), #[serde(default)]
    pub appetite: f32,                      // random walk around 1.0, clamped [appetite_min, appetite_max]
    pub appetite_pin: Option<f32>,          // a lever pins it (None: the walk)
    pub bought_today: u32, pub sold_today: u32,   // units the World bought from / sold to the city
    pub bought: VecDeque<u32>, pub sold: VecDeque<u32>, // 30-day rings, newest last
    pub paid: VecDeque<i64>, pub charged: VecDeque<i64>,
}
```

Each midnight, after `corps::daily` and before `demography::run` (M17's slot):

1. **Appetite.** `appetite = 1 + (appetite − 1) × (1 − revert) + N(0, sd) + season[good][season]`, from `SimRng::outside(WORLD_ACCOUNT, day, good)` (M17 § 1's random walk, per good), clamped `[0.3, 3.0]`, unless pinned.
2. **The bid.** The q-th unit sold today fetches `bid(q) = max(bid_floor, bid_ref × market × appetite × (1 − slope × q ÷ cap))`; the daily cap is `cap × appetite`. A seller is paid per tranche, in id order of the selling buildings (deterministic).
3. **The ask.** `ask = bid_ref × market × (1 + spread) × ask_mult`; uncapped. Customs on an import: `customs_rate × ask` to the Treasury on top.
4. **Refill.** The World's treasury refills to `treasury_ref` (`minted += refill`, L2's rule), so it never runs dry; what it minted is the city's net trade, visible as `outside_minted`.

Who sells: **the owner chooses by price**. A Farm's haul (`economy::haul`) goes to its own Market first (today), then compares the World's marginal bid with the rival Market's `wholesale` and sells to the higher, then the Reserve at wholesale. Fabs sell Parts above `parts_floor` to the higher of a buyer's offer (`part_credit`) and the bid; Labs sell Data above `data_floor` to the World or M14's buyers. A Clinic or Garage may sell surplus chrome or a deck at `parts_per` Parts-equivalents (phase 2). Who buys: a Market whose shelf price would exceed `ask + haul_margin` with its restock unmet imports up to `restock_batch` at the ask; a Clinic or Garage lacking Parts pays `part_credit` to the World instead of the Treasury (`import_to_outside` on).

```toml
[world_market]                       # new; placeholders
enabled = true
treasury_ref = 50000
revert = 0.05
sd = 0.03
appetite_min = 0.3
appetite_max = 3.0
food  = { bid_ref = 3.4, bid_floor = 1.5, cap = 450, slope = 0.5, spread = 0.6, season = [0.0, -0.15, 0.0, 0.25] }
parts = { bid_ref = 16,  bid_floor = 8,   cap = 40,  slope = 0.4, spread = 0.5, season = [0, 0, 0, 0] }
data  = { bid_ref = 35,  bid_floor = 15,  cap = 12,  slope = 0.5, spread = 0.8, season = [0, 0, 0, 0] }
haul_margin = 1                      # coins a unit over the ask before a Market imports
customs_rate = 0.1                   # of an import's price, to the Treasury
ask_mult = 1.0                       # the famine lever (lower: cheaper imports)
```

At these placeholders the World pays at most ~1,000 a day for Food (450 units, mean bid ~2.6), ~550 for Parts and ~310 for Data: ~1,900 a day at appetite 1.0, in the § 1 target band. Food's ask is 5.4: the shelf cannot pass ~6.4 while the World sells, so Winter's 2 → 28 cliff (M11's history) becomes a climb to 6 and a leak of coins abroad.

### The balance of trade

New CSV columns (§ 8): `flow_export` (exists), `flow_import_out` (inputs, Parts and food paid to the World), `flow_customs`, `trade_balance` (export + immigrant pockets − imports − emigrant wallets, daily), `trade_balance_30` (rolling), `appetite_food`, `appetite_parts`, `appetite_data`, `bid_food` (today's first-unit bid), `ask_food`. The app's city panel shows a one-line readout: "Trade +312 today, +4.1k this month (3 % of coins); World food appetite 1.24".

### The levers

| lever (`PlayerCommand`, CLI `--lever`) | effect | exists? |
| --- | --- | --- |
| `SetExport(bool)` (`export=on|off`) | the World buys / does not | yes (L2) |
| `SetExportPrice { good, price }` | pins `bid_ref` | yes (L2), reread as `bid_ref` |
| `SetAppetite { good, mult }` (`appetite=food:1.5`) | pins `appetite` (None releases it to the walk) | *new* |
| `SetExportCap { good, cap }` | the daily cap | *new* |
| `SetImportAsk { good, mult }` (`ask=food:0.8`) | `ask_mult` per good: the famine lever and the import tariff's inverse | *new* |
| `SetCustoms(rate)` | `customs_rate` | *new* |
| `SetTaxRate`, pin or `auto` | pins the tax rate, or hands it back to the band | exists, extended |

### An honest infusion

"Artificial" money is the World paying more than the city's price: an appetite of 2.0 doubles the cap and raises the bid; coins cross in through producers' sales, never into wallets directly. Its visible consequences, in order, are the gate's god scenario `god_world_boom` (*new*, § 9): producers' revenue rises → `wage_mult` rises toward `wage_cap_mult` and vacancies post (hiring) → the demand input passes Grow's curve and a corp builds or refits (the v7 gap closed) → employed and wallets rise → the city's food price rises because export competes with the shelf (`flow_food` per unit up) → the Treasury's tax take rises and the band cuts the rate. A drain is the opposite: appetite 0.4 → layoffs, a falling `wage_mult`, Missions' queues, theft and emigration. A god who wants the city fed in a famine lowers `ask_mult` (cheap imports, coins leak) or raises the Food appetite's floor; one who wants it broke closes the World.

## 4. Charities and churches

```rust
pub enum BuildingKind { /* … L2 */ Mission }          // new; label "Soup Kitchen"; letter M (check the M16 plan's claims first)
pub enum Role { /* … L2 */ Volunteer }                // new; unpaid (wage 0), fed a staff meal from the kitchen
pub enum Flow { /* … */ Donate, Alms }                // new; Donate untaxed (agent/corp/gang/player → Mission), Alms is a meal's ledger-only cost
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Charity {                                  // Building.charity: Option<Charity>, #[serde(default)]
    pub purse: i64,                                   // the Mission's own coins (in total_coins)
    pub donors: BTreeMap<EntityId, i64>,              // 30-day sums, for the thanks and the god readout
    pub meals_today: u16, pub cots_today: u16, pub clinic_today: u16,
    pub served: VecDeque<u16>,                        // last 30 days
}
```

**What it is.** A Mission is a building with a kitchen, cots and (Tier 1+) a clinic bench. Seeded: one in a Sump district (city deed, then an heir's, as L2's seeded venues), and the Purist Chapel's Hideout gains the `Charity` component (The Unplugged run it; a meal there is a pool deed `Gave` (*new* `Deed`) that raises the creed's standing). Foundable by `Register` (`found_cost` 150) by an agent with `lawfulness ≥ 0.6` and pride below 0.5, or by a corp under Lobby (philanthropy as reputation: `standing` + `rep_gift` per 100 donated).

**What a coin buys** (placeholders): a **meal** costs the Mission `wholesale + 1` (it buys food at the nearest Market's wholesale; a meal lifts hunger as `EatOut`); a **cot** 1 coin a night (a bed for a homeless resident, the Hotel's shape at a quarter of the price); a **clinic hour** 6 coins (an `Injured` or `Withdrawal` state treated as a Clinic's Therapy at tier 0). The kitchen serves while the purse covers it; a Volunteer on shift doubles the meals served an hour. Who is served: the hungry poor (`hunger < 0.4`, coins < one meal) within `reach_tiles` (40) of the door, queued by hunger; the Statistical tier through a daily pass (`charity::stat_daily`, *new*, the L2 `leisure::stat_daily` shape), drawing meals from the purse at the same price.

**Who gives.** `ActionKind::Donate` (*new*), a single-step plan at a Mission's door on the **Socialise** goal: an agent with coins above two weeks of rent and food gives `donate_frac` (0.05) of the surplus with `p = donate_base × (lawfulness + loyalty) ÷ 2 × (1 + creed)`; a gang under a front's Expand gives to its own district's Mission for dread's opposite (a small `standing`); a corp under Lobby gives `lobby_gift`; the player gives anything (`PlayerCommand::Donate { mission, amount }`, *new*). Donations are slim by design: the placeholders give ~20-60 coins a day city-wide, 5-15 meals.

**The neighbourhood effect a donor sees.** Per-district columns (§ 8): `dX_meals` (Mission meals served), `dX_starving` (`Starving` events), `dX_dregs` (Dreg-class adults) and the existing `dX_unrest`. The god scenario `god_big_donor` (*new*) gives 5,000 coins to the Sump Mission on day 20; the assert is that the district's starving events per day fall and its Dregs and unrest are lower over days 21-50 than the twin's (mechanism), with the size printed. That is the lever addendum 17 promises the player.

```toml
[charity]                             # new; placeholders
enabled = true
seed = 1                              # Missions at seed, Sump first
reach_tiles = 40
meal_markup = 1                       # coins over wholesale
cot_price = 1
clinic_price = 6
donate_base = 0.02
donate_frac = 0.05
lobby_gift = 40
rep_gift = 0.02                       # standing per 100 coins given
```

## 5. Prices and wages that clear

**Wages from revenue.** Daily at midnight, for each corp (`corps::daily`, *new* step `wages_from_revenue`): `R` = its 7-day mean revenue (every taxed inflow and `Flow::Export`, net of tax), `P` = its 7-day mean payroll (wages, exec wage). The target is `P* = labour_share[primary niche] × R`. `wage_mult += wage_step` when `P < 0.9 P*` or a vacancy stood unfilled `shortage_days`; `−= wage_step` when `P > 1.1 P*`; clamp to `[wage_floor_mult, wage_cap_mult]`. M11 D22's Squeeze cut (x0.9) and M15 W29's poaching premium multiply on top, as today. A role's wage is `role weight × wage_mult` (the config wages become weights: Farmer 6, Guard 8, Fabber 7, …). Agent and gang employers keep their fixed wages; the city's roles are paid from the Treasury at the config wage.

**Hiring and layoffs.** A corp posts one vacancy per building a day when `P < hire_below × P*` for 3 days and its niche demand (`corp_brain` inputs, now with the World's fill ratio) is at least `hire_demand`, up to the building's staff cap; above `fire_above × P*` for 7 days it stops replacing quitters and lays off the newest (`economy::dismiss`, a `Fire` event "laid off: revenue"). Farms gain an overtime rule: the staff cap rises by `export_staff` per 100 units of unfilled World order a day.

**Prices.** The shelf curve stays (`price_for_stock`, `price_level`); the price is clamped below by the owner's unit cost (`wholesale + input_per_food`, so a Market never sells at a loss to its own inputs) and above by the import ceiling (`ask + haul_margin`) whenever the World sells. Venue prices (L2 `Venue.price`) follow `wage_mult` of the owner's region at half strength (*new* `venue_wage_pass` 0.5): a city whose wages rise pays more at the Club.

**What stops a death spiral.** Less income → less food bought → less revenue → lower `wage_mult` and layoffs → less income. Three brakes, none of them a hoard: (1) **the World**: export demand is exogenous, so producers' revenue has a floor that the city's poverty does not touch (the bid floor and the cap); (2) **the wage floor**: `wage_floor_mult` holds a job's wage near a meal and a half while the corp can pay it; a corp that cannot goes under and its buildings go to a buyer or the City (M11 D29), and the City runs a foreclosed Farm on its own payroll until it sells; (3) **the import ceiling**: food is never dearer than the World's ask plus a coin, so a famine costs coins abroad, not the shelf. The famine lever is the World's ask and its Food appetite, not a Reserve release.

**Monopoly.** M11's monopoly markup cap (2.0) stays; the import ceiling makes it moot for Food while the World sells. A Food seller that falls under `monopoly_share` has the World as a buyer at the bid, which keeps a small Farm alive against a squeezing rival.

**The bounds** (the gate, § Goals): wages ≥ 60 % of the population's inflows; Treasury inflows ≥ 95 % tax-shaped; starvation within the scaled v1 bound at default levers and the lever twins ordered; ≥ 2 Food sellers alive; a 365-day run that holds.

```toml
[economy2]                            # new; placeholders
enabled = true
labour_share = { food = 0.6, housing = 0.4, security = 0.7, tech = 0.55 }
wage_step = 0.03
wage_floor_mult = 0.8
wage_cap_mult = 2.5
shortage_days = 5
hire_below = 0.85
hire_demand = 0.5
fire_above = 1.25
export_staff = 2                      # Farm staff per 100 units of unfilled order
input_per_food = 0.5                  # coins to the World per unit produced
input_per_part = 4
input_per_data = 8
power = { market = 20, bar = 3, club = 8, arcade = 4, lounge = 15, clinic = 6, garage = 6, lab = 10, fab = 12 }  # a day, to the World
venue_wage_pass = 0.5
[treasury]
property_rate = { farm = 6, market = 10, bar = 1, home = [0, 0, 1], hotel = 1, clinic = 2, garage = 2, lab = 3, fab = 3, venue = 1 }
treasury_initial = 12000              # [world] treasury_initial 35000 → this; the rest is corp working capital
tax_min = 0.05
tax_max = 0.20
tax_step = 0.01
band = [8000, 25000]                  # [budget] band reread
```

## 6. What it takes from M17, and what waits

**Taken** (M17 § 1, built as specified, with L2's names): `OutsideKind::World` and its account at 1069 (exists); the World's `market` walk (M17 step 1; L2 held it at 1.0); `Outside.{minted, inbound, outbound}` and **the conservation identity**, now with outbound flows (imports, emigrant wallets) and inbound immigrant pockets; `[outside] import_to_outside = true` (M13's `Flow::Import` re-pointed abroad); `cross_in` and a *new* `cross_out` (the World's treasury gains, `outbound` grows, `minted −=` its refill excess at the daily refill). The World's per-good book (§ 3) is this milestone's addition to M17's account; M17 reads it as "the World account's demand".

**Waits for M17** (unchanged in its spec): the megacorp parents, syndicates and the State as accounts; `OutsideOrder` and the considerations; remittances (`Flow::Remit`) and `exposure`/`branch_loss`; reinforcement; scrip; sanctions (a sanction will cut the World's appetite for a sanctioned branch's goods, the natural join); uplinks and the Blackwall; Abroad contracts. M17 then needs only to append fields to `OutsideFaction` (`#[serde(default)]`) and to let a parent's subsidy cross in through `cross_in`, which keeps the identity.

**Waits for later**: Power and Water as goods (the `power` input above is coins to the World, a stand-in for a generator; the goods milestone makes it a stock); per-good food variety; corp scrip; the late calibration milestone's price reset (this milestone moves the M13 prices only if the price print says so, as L2 § 1 planned).

## 7. Events, CSV, UI

Events (amber): `Exported` (exists; now per good with the bid), `Imported` (*new*: "Civic Market imported 300 food at 5.4 from the World"), `WageMoved` (*new*, a corp's `wage_mult` crossing a 0.1 step), `LaidOff` (the `Fire` text "laid off: revenue"), `Donated`, `MissionServed` (daily summary), `TaxMoved`, `AppetiteShift` (a walk crossing ±0.25 from 1.0: "the World wants food: appetite 1.31").

CSV (*new*, `EconCols` before `ticks_per_sec`): `flow_import_out`, `flow_customs`, `flow_inputs`, `flow_property`, `flow_donate`, `trade_balance`, `trade_balance_30`, `appetite_food`, `appetite_parts`, `appetite_data`, `bid_food`, `ask_food`, `food_imported`, `food_exported`, `wage_mult_mean` (payroll-weighted), `wage_gross_mean`, `vacancies_open`, `laid_off`, `pop_inflow_wages`, `pop_inflow_other` (stipends, theft, charity, wins, scavenging, loans), `tax_rate`, `mission_meals`, `mission_cots`, `mission_purse`, per district `dX_meals`, `dX_starving`, `dX_dregs`. Removed with the milestone on (left at 0, columns kept): `flow_dole`, `flow_public_works`, `works_jobs`, `upkeep_mult`, `wage_dole_ratio` (replaced by `pop_inflow_wages ÷ (wages + other)`).

The app: the city panel's trade line (§ 3); a Mission's inspector (purse, served today, top donors); a corp's inspector shows `wage_mult`, revenue ÷ payroll and its export share.

## 8. Save compatibility

`SAVE_VERSION` 3 (*new*). `WorldBook`, `Building.charity`, `Corp` working-capital seed and the tax band's state default on load; a version-2 save loads with the milestone's switches set as the save's config says (no migration of a dole city into a real one mid-run: a loaded v2 save keeps `[economy2] enabled = false` unless the operator flips it, and the flip on a running city is a god scenario, not a supported path).

## 9. Testing, phases

Unit tests: the tranche bid (sum over q matches the closed form; cap respected); the ask ceiling clamps the shelf price; `cross_out` and the identity with imports and emigrants; `wages_from_revenue` converges on a fixed revenue in ≤ 20 days and never leaves its clamps; the hiring rule posts and lays off at the thresholds; a Mission meal moves exactly its cost; the tax band steps with hysteresis; the off switch reproduces the L2 city (`--econ-off`, 15-day report byte-identical).

God scenarios (`GOD_SCENARIOS_V8.md`, *new*): `god_world_boom` (all appetites 2.0 from day 20: vacancies, a Grow or refit, wages and wallets up, food price up; vs the twin), `god_world_bust` (appetites 0.3 from day 20: layoffs, Mission queues, thefts and emigration up; no Treasury hoard grows), `god_famine_ask` (Winter with `ask=food:2`: starvation rises; with `ask=food:0.6`: the shelf holds and `flow_import_out` rises), `god_big_donor` (§ 4), `god_close_world` (export off at day 30: the city contracts to its domestic circuit and does not empty: population on day 120 ≥ 1,200), `god_kill_food_corp` (the Food monopoly risk: the World's bid keeps a second seller alive).

| Phase | What lands | Commit gate | Numbers per phase (seed 42, 120 days; placeholders) |
| --- | --- | --- | --- |
| 1. The World market | § 3: `WorldBook`, appetite walk, tranche bids, the ask and Market imports, `import_to_outside`, `cross_out`, immigrant/emigrant crossings, owners choosing by price, the identity, levers, trade CSV; **the dole still on** | identity unit test; `--econ-off` identity; trio | `flow_export` 1.1-2.2k a day; `flow_import_out` 0.7-1.4k; trade balance within ±5 % of coins; Food never above the ceiling; Winter starvation ≤ L2's |
| 2. Wages from revenue | § 1, § 5: upkeep → inputs + power + property rate; the hoard → working capital; `wages_from_revenue`, hiring and layoffs, Farm overtime, Grow reads the World (+ refit), venue prices follow wages; chrome/decks as Parts-equivalents; **the dole still on** | identity; trio; L2 gate | wage ÷ dole ≥ 1.0 by day 60 (L2: 0.41-0.52); employed ≥ 750 on day 60; no seeded corp bankrupt by day 60 that L2 kept; ≥ 1 Grow from export |
| 3. No safety net | § 2, § 4: the dole, the unpaid and absentee dole, public works, `reserve_release`, the Treasury's scavenge coin removed; the Recycler pays scrap from its sales; the tax band; Missions, `Donate`, the Chapel's kitchen, the district columns | identity; trio; L2 gate with its economy bullets re-read | starvation within the scaled v1 bound; population ≥ 1,800 on day 120; wages ≥ 60 % of inflows; Treasury in band from day 30; Mission meals 30-150 a day |
| 4. Gate, year, god suite | the gate, `GOD_SCENARIOS_V8`, 365 days on 42 and 43, findings on 42-47, the M13 price print, docs ("Implemented: deviations"), `/code-review high` and a fix commit; a short re-shadow (worker, cook, homeless, farmhand, Mission volunteer) appended to `SHADOW_V2.md` | every gate on main | the § Goals findings; the year bounds |

Phases are sequential (each reads the last's money). Phase 1 is the M17 slice and is safe alone; phase 2 is the risky one (every corp's books change) and must be judged with the dole still on, so the dole is not blamed for what the wage rule does; phase 3 removes the safety net only when phase 2's wage ÷ dole passed 1.0 on the majority of 42-44. If it did not, phase 3 waits and the orchestrator asks Dylan.

## 10. Risks

- **The transition wave.** The first week without a dole, with ~1,400 jobless and opening wallets of 10-40 coins. The dole-off run lost 900 residents to emigration by day 30; even with wages from revenue the jobless have nothing on day 0. Brakes: working capital, the World from day 0, kin through the pantry, Missions seeded; the gate prints days 1-14 per day. If emigration still empties the city, the honest knob is `emigrate_mood`/`emigrate_days` (leaving costs money and contacts: a resident with no coins cannot buy passage, *new* `emigrate_cost` 20 coins), not a dole.
- **The Winter wave.** Winter's yield x0.3 and the Food appetite's Winter season +0.25 compound: farms sell abroad when the city is hungriest. The ask ceiling holds the shelf price; the coins leak abroad and wallets thin. Watch days 90-119 for starvation and thefts; the lever is `ask_mult`, the season term or the Food cap.
- **Corps that only Secure.** God v7: the rich corps sat on Secure with 244k. Grow's demand input reading the World, the refit path, and retained earnings above `hoard_heat` raising `flow` are the fixes; if Secure still wins, the considerations' flats (`order_flat`) are phase 2's calibration, not a new order.
- **Thin wallets kill M13 and M14 markets.** Chrome, vehicles, stims and Data sales need savers. With wages at 8-10 and food at 4 a saver exists only in a two-earner home; M13's tier-1 good at 60 is 6-8 days of a median wage, inside L2's rule. If installs fall below L2's band, the Parts-equivalent export keeps Clinics alive and the price print decides, not a subsidy.
- **The wage rule rings.** Revenue → wages → spending → revenue is a feedback loop; a 7-day mean, a 0.03 step and the floor and cap are the dampers. If it oscillates, widen the window; do not add a controller.
- **The Treasury band rings.** A tax rate that moves every three days moves every price. The hysteresis is L2's (`band_hold_days`), the step 0.01.
- **The city as owner of last resort.** The dole-off run ended with the City owning 166 foreclosed buildings. Foreclosures must sell (M11 D29's buyer search, daily, at falling prices) or the City becomes the economy by default; the gate prints City-owned buildings on day 120.
- **Gangs grow.** No dole makes the stipend the best offer; L2's desistance and roster cap bound the rosters, and the year run watches Murders against L2's bound.

## 11. What L2's economy findings become

| L2 finding (printed) | becomes |
| --- | --- |
| Σ wages ÷ Σ dole on day 60 ≥ 1.0 | gone (no dole); replaced by the asserted "wages ≥ 60 % of the population's inflows" |
| employed share 30-40 % | printed, band 45-60 % |
| Treasury inside its band from day 30 | asserted (the tax band, `[0, 3 × band_hi]`) |
| wallet Gini 0.50-0.70, wallets ≥ 20k | printed, Gini 0.55-0.75, wallets ≥ 25k; the gate's sanity bound 0.95 stays |
| the export hook off, `flow_export` 0 | asserted on, > 0 every week |
| `total_coins` drift printed | asserted to the coin (immigrants and emigrants booked) |
| the M13 price print | printed again on the new wages; moved only by Dylan's call |
| public works at `works_wage` 7, `works_max` 120 | removed |
| `reserve_release` (seed 43's year-3 famine) | removed; the ask ceiling is the fix (the stranded shelf imports) |
| `UNPAID_DOLE_DAYS`, `paid_once`'s dole, the absentee dole | removed (addendum 17) |

## 12. Out of scope

M17's parents, syndicates, the State, remittances, reinforcement, scrip, sanctions, uplinks; Power and Water as stocks (the `power` input is a coin stand-in); food variety and the dystopian producers (vertical farms, the Soylent factory); staff actions with customers (the cook serving `EatOut`, the attendant), markets by zone and distance-aware hiring (SHADOW_V2 cause 1, the leisure/roles pass after M18); a `Pray` goal and religious services beyond the kitchen; loans from corps or a bank (chrome finance stays M13's); M16's contracts, protection and the Fixer.

## Builds on

| Milestone | Decision | What the Real economy reads or changes |
| --- | --- | --- |
| M1/SPEC | the dole, `dole_per_day` 4, the Hall | removed with the milestone on; the lever stays for `--econ-off` |
| M11 | D2 `pay`/`charge`; D3 conservation; **D4 upkeep as the Treasury's return flow**; D5 wholesale's three legs; D13 wages at the workplace; D22 Squeeze wages; D23 Grow; D29 bankruptcy and foreclosure; D41 `ParentId`, `OUTSIDE_PARENT_BASE` | D4 reversed (upkeep → inputs to the World + property rate); D5's overflow leg competes with the World's bid; D22 multiplies `wage_mult`; D29's foreclosures must sell |
| M12 | D15 fines; D20 Hotels; D23 Sanitation | fines stay a Treasury inflow; a Mission cot is a quarter-price Hotel night; sweepers stay city roles, without public works |
| M13 | D8 `Flow::Import`, `part_credit`, `import_frac`; D14 the parts market; D16-D17 Clinics, Garages, Tech demand | imports re-pointed to the World (a leak); chrome and decks export as Parts-equivalents; Tech demand reads Parts orders |
| M14 | V16-V17 Labs and the Data market | Data exports to the World above `data_floor` |
| M15 | W29 poaching premium; W33 honour; W37 ads | the premium multiplies `wage_mult`; a donation is a told deed (`Gave`) that moves standing |
| L2 | L10 the band; L11 the export hook, `cross_in`, `WORLD_ACCOUNT`; L9 scrap Parts; L20 fronts and stipends; desistance; `reserve_release`; `UNPAID_DOLE_DAYS` | the band moves the tax rate, not works or upkeep; L11 extended into `WorldBook` and `cross_out`; scrap paid from the Recycler's sales; stipends compete with wages; `reserve_release` and the unpaid dole removed |
| L2 shadow fixes | item 11 the absentee dole; item 21 `Flow::Scavenge` | the absentee dole removed; scavenging paid by the Recycler, not the Treasury |
| M17 (spec) | § 1 the ledger, the World account, `minted`/`inbound`/`outbound`, the identity, `import_to_outside` | built for the World only; M17 appends the rest |
| Addenda 17, 18 | no safety net; charities on donations; no magic money; the outside's supply and demand as levers | §§ 2-4 |

## Addendum (2026-10-08, Dylan, after the draft): sequencing, no school meals, the child work camp

- **Built next**, after M16a phase 1 merges and before M16a's phases 2-5 (roadmap addendum 19).
- **Decision 16 overturned: no school meals.** The Treasury makes no transfer to any wallet or pantry.
- **In their place, in phase 3: a child protective service and work camps.** A daily pass takes a child whose hunger has stayed below `[camp] take_hunger` for `take_days` (`ChildTaken`: a Life row, a Grief-class memory and a grudge on the service for the parents, a Feed story) to a `BuildingKind::Camp` owned by a corp or the city (one seeded, foundable by corps), which feeds and houses the children from the revenue of a `CampWork` shift (a low-yield good the owner sells: Parts or Food processing; no magic coins) and releases them at adulthood with a `CampRaised` trait (skills from the work, low family affinity). A camp that cannot feed its children is a Feed scandal and the law may close it; freeing children is M16b's Rescue (a hook only). Acceptance: no child starvation death with camps on (asserted); ≥ 1 child taken on some seed, the camp's output sold, a release at adulthood in the year run (existence); `children_taken`, `camp_children`, `camp_output` columns.

## Addendum (2026-10-09): what landed, and the stop rule fired

- **Phase 1 (the World market) merged** 5a1a09c, on by default: money enters (the World buys ~1.1-1.2k a day of Food, Parts and Data at sloped tranche bids), leaves (imports ~650 a day, inputs from phase 2 when on), the identity holds to the coin; the Winter export squeeze (§ 10) was real at the placeholder Food cap 450 (88 starved on days 118-119, seed 42) and is 350 now. Findings: the Reserve is no longer the Winter buffer; Food corps hold 83-113k on day 120 while the Treasury takes only customs.
- **Phases 3b/3c merged** 689151e, on by default: Missions on `Donate` (corps on Lobby, gangs, agents, the god lever), Volunteers, cots and meals, the Chapel's kitchen, per-district meal/starving/Dregs columns; the child protective service (two unfed days), corp and city Camps with a daily shift ledger selling Parts, `CampRaised` at release, scandal bulletins and closure, a take goes to a camp that can feed tonight. The Donate lever works (5,000 coins to the Sump West Mission: meals 134 → 1,771, the fed districts' starving counts down). In the dole city meals and donations are far under § 8's bands (few hungry-and-broke within reach): a calibration finding.
- **Phase 2 (wages from revenue) merged off by default** 08dd798 (`[economy2] wages = false`): **the stop rule failed on every seed**, structurally. The dole was funded by the corp upkeep levy that E13 replaces with a property rate; the Treasury is dry by day 12, the dole stops by day 18, the city is § 0's dole-off city (population 683-819 on day 120; employed 474-496 on day 60 against 750). Decision 18's "dole still on" step cannot exist. Dylan decides (the options and the levers are in `docs/HANDOFF.md`'s head and the phase 2 commit). Phases 3a and 4 wait.
