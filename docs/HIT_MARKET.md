# The Hit market: deaths people can pay for

Companion to `M16_CONTRACTS.md` (the board, the Fixer, squads, the ledger, the accessory rule: all built in M16a), `M15_WORD_AND_BLOOD.md` (grudges, Hunts, vendettas), `TESTING.md` (the tiers) and `VISION.md`. Plan: `~/.claude/plans/hit-market.md`. Where this document and `M16_CONTRACTS.md` disagree on who buys a Hit or a Beat, at what price, and what happens to the money, this one wins. Sits before M16b; M16b's leverage reads it.

Dylan, 2026-10-10: *"Paying for kills should be a mechanism we add."* After M16a no Hit or Beat is fulfilled naturally on seeds 42-47; the board runs on Locates and Guards. The goal: grudges and vendettas that can be paid for (kin revenge, a rival exec, a debtor, a witness), priced against what people actually hold, plus corps and gangs posting Hits for business reasons. Emergent story first: the judge is Hits fulfilled on most of 42-47 and diaries where someone paid for a death.

As everywhere in this project, a "Hit", a "Beat", a "witness" or a "debt" is a record on a struct, a utility score, an integer purse moved by `ownership::pay`, and a seeded dice roll between fictional agents of a simulated city.

**The short version.** The machinery works (god v8: 11 brokered Hits at 2,000 on execs, 5 done in a week). Nothing buys one because of **the price**: `contracts::quote` is a list price (`price_base × (1 + 1.5 risk) × (1 + standing)`, 300-1,300 for an ordinary target) and the people with a reason to kill hold 0-10 % of it. Meanwhile the guns on the Fixer's books would take the same job for **20-60 coins** (their own acceptance score, solved for price). The list price is 10-30× the market. So: **price a job at what the guns ask, let buyers pay with what they and their kin hold plus a balance on credit, and give four new kinds of people a reason to buy** (the suspect with a witness, the creditor, the dirty exec with a rival, the gang protecting its own). An unpaid balance is a debt; a debt is a motive; the chain closes.

Identifiers marked *new* don't exist at HEAD (`d771073`). Numbers are measured unless marked *placeholder*.

---

## 0. Measured

**How.** A scratch probe (an `#[ignore]` test in this worktree, never committed; source in the session scratchpad as `hit_probe.rs`) ran the shipped config on seeds 42-44 for 120 days and, every 20 days, read every grudge holder, wallet, gang and corp purse, open report and Fixer regular. A gun's **ask** is the closed form of `contracts::accept_score` (mode A at urgency 1, brokered, illegal and violent kinds) solved for the price at `accept_min` 0.35: the lowest price at which that regular would take that Hit.

| seeds 42 / 43 / 44, days 20-120 | measured |
| --- | --- |
| adult wallets | p50 2-3, p90 8-11, p99 25-61 (the dole city) |
| holders with an eligible grudge (`hunt_min` 0.75) at a snapshot | 0-6 (d20-60), 4-17 (d80-120); lethal (≥ 0.85) 0-10; "weak" (`might_gap < hire_gap`) 0-14 |
| unsettled grudges ≥ 0.5 by cause, d120 | assault 54-79, killed friend 54-97, killed kin 18-28, robbed 4-15, betrayed 0-5, hired 1-5 |
| the Hit/Beat `quote` on those holders' targets | p50 306-1,203, p90 364-1,301 |
| holder's own wallet ÷ quote | p50 0-4 %, p90 0-10 %; **afford: 0 holders at any snapshot** |
| holder + adult kin wallets (Spouse/Parent/Family) | p50 2-18, p90 5-49; **afford: 0** |
| Fixer regulars / free regulars at lawfulness ≤ 0.3 | 47-83 / 11-46 |
| **asks of those guns on the lethal targets** | p10 19-38, p50 23-69, p90 25-300; the cheapest gun per target p50 19-43 (one seed 159-247 late: targets with a gang behind them); some targets have no finite ask (defenders) |
| gang treasuries | 0-509 (d20), 0-351 (d80-120); a Hit on a rival leader quotes 1,100-1,700 |
| corp treasuries | 0-62k (rich: 8-60k for the big three); an exec Hit quotes 580-1,900; execs below lawfulness 0.3: 2-4 per seed |
| open Murder reports with a living witness | 0-2 at a snapshot; Assault 2-53 |
| financed assets in arrears ≥ 3 days | 10-24 at d20, 0-4 after |

**What it says.** (1) The demand exists but is small and broke: a handful of lethal grudges at a time, held by people with 2-10 coins. (2) The supply exists and is cheap: a dozen to four dozen lawless free guns asking 20-60. (3) `quote` sits an order of magnitude above both. (4) Gangs are as broke as people at the list price; corps are rich and lack a reason. (5) Murder witnesses are rare at any instant; assault witnesses are common. (6) Debt barely exists (begging loans of 1-2 coins; chrome finance arrears early only), so the debtor motive needs a source of real debt: the Hit's own balance supplies it.

---

## 1. Decisions (overturnable)

| # | Question | Decision |
| --- | --- | --- |
| H1 | The price | *new* `contracts::market_price(world, kind, target, cands) -> Option<i64>`: the ask (H2) of the `ask_rank`-th cheapest eligible candidate (`eligible` as matching reads it), at least `price_floor[kind]`; `None` when fewer than `ask_rank` candidates have a finite ask. Candidates: a Fixer's regulars for a brokered post, `direct_candidates` for a direct one. **Every Hit and Beat posted by an agent, a gang or a corp prices at the market**; on `None` it falls back to `quote` (the record goes to squads and gangs as M16a does). Law, god, Locate and Guard keep `quote`. |
| H2 | A gun's ask | *new* `contracts::ask(world, gun, kind, target, brokered, buyer) -> Option<i64>`: the lowest price at which mode A's product reaches `accept_min`, solved in closed form (every term but `U(price ÷ week_wage)` is fixed; invert the Logistic{8, 0.3} and `U(x) = x ÷ (1 + x)`). `None` when the other terms cap the product under `accept_min`. Pure; unit-tested against `accept_score` at `ask` and `ask − 1`. No change to the Contract goal or `accept_score`. |
| H3 | Means | *new* `hitmarket::means(world, buyer, target) -> Means { own, pooled: SmallVec<(EntityId, i64)> }`: the buyer's wallet plus, from each living adult with a Spouse, Parent or Family edge to the buyer (Friends too for a revenge want) **who holds an unsettled grudge ≥ `grudges.grudge_min` on the same target**, `kin_share` of their wallet. A gang's means: `gang_spend` × treasury; a corp's: treasury ÷ `corp_reserve_mult`. |
| H4 | Credit | A buyer whose means cover `deposit_share` of the price posts at the full price: the deposit is escrowed (brokered) or checked (direct), the rest is a **balance** owed at fulfilment. Agents only (factions pay in full). *new* `Contract.deposit: i64` (`#[serde(default)]`; 0 = paid in full, the M16a record). |
| H5 | Pooling the money | At posting each contributor pays its share to the buyer (*new* `Flow::Pool`, untaxed, appended) and joins `known_by` with a first-hand `Hired` (`contracts::hear_hired`): more people paid, more people know. |
| H6 | Paying the balance | At fulfilment `settle` pays the escrowed deposit, then charges the balance from the buyer's purse (`ownership::charge`, `Flow::Payout`), split with the crew as today; the Fixer's cut is taken from what was paid. **Short**: the unpaid part becomes an edge debt buyer → taker (`Edge.debt`, `debt_since` now: the existing `social::repay_debts` takes it out of the buyer's wages when the wallet passes 20) and the taker gains `Grudge { cause: Betrayed, weight: stiff_grudge × unpaid ÷ balance }`; *new* `EventKind::Stiffed` and a `Betrayed` deed as a renege posts. A record never charges a contributor. |
| H7 | A buyer keeps his own Hit quiet | `gossip::exchange_one` never volunteers a `Hired` rumour whose actor is the teller or the teller's faction (god v8 gap 5: a buyer told two guards within the day and was charged the next midnight). Interrogation, the Fixer's talk and other holders still spread it. `person.rs::probe_buyer_keeps_his_own_hit_quiet` is un-ignored. Without this every organic buyer is in Jail by day 2. |
| H8 | The Want | *new* `World::wants: BTreeMap<EntityId, Want>` (saved): `Want { target, kind, motive, since, origin, tries: u8 }`, one per agent buyer, written by the daily passes (§ 2) when a motive stands, dropped when it is posted, the target dies or leaves, the motive lapses or `want_days` pass. A want short of means is kept (the buyer may get paid, kin may join) and re-checked daily. Factions post at once and keep no want. |
| H9 | Posting at Full and Coarse | A buyer at Full or Coarse with a want and a reachable open Fixer (`hire_reach_tiles`) walks there: *new* `GoalKind::Commission` (scripted through `plan::plan_for`'s bypass like the Hunt: `GoTo(Fixer) → Commission`), *new* `ActionKind::Commission` (dur `commission_mins` 30) whose completion calls the posting. Score: want held → GATE; the motive's weight → Linear{0.8, 0.2}; `1 − lawfulness` → Linear{0.5, 0.5}; flat `commission_flat` 0.0. With no Fixer in reach the post is direct, at once (talk to one's own people needs no errand). |
| H10 | Posting at Statistical | The daily pass posts at once (the Hunt hire precedent: "Tier-neutral: a Statistical holder posts at once"). |
| H11 | Volume brakes | At most `max_open` organic Hit/Beat records open (*new* `Contract.motive != Motive::None` counts) and `posts_per_day` new ones a day city-wide, wants ordered by motive weight then id. Brakes, not targets: never tuned to a band. |
| H12 | Who hires instead of hunting | M16a's rule (`might_gap < hire_gap`) **or** courage under `hire_courage`: a coward with a dead brother pays. The Hunt hire path (`try_hire`, `hire_pass`) writes a want instead of posting. |
| H13 | Motive on the record | *new* `Contract.motive: Motive` (`#[serde(default)]`): `None` (M16a, law, god), `Revenge`, `Silence`, `Debt`, `Rival`, `Culprit`, `Protect` (a gang silencing its member's witness). `Origin` is unchanged (who decided); the motive is why. The event text and the log carry it. |
| H14 | A dead witness | A report whose witness is dead and no city guard holds a `SawCrime` of it goes **cold** `cold_days` after the death: `resolved = true`, *new* `EventKind::CaseCold`. "No witness, no case." The law has no use for a report's witness today, so without this silencing buys nothing. |
| H15 | A beaten witness | A fulfilled `Silence` or `Protect` Beat on a report's witness: the witness **recants** with `p = recant_p × (1 − courage)` (`ContractNs::Recant`, *new* purpose), the report resolved, *new* `EventKind::Recanted`. A guard witness never recants and is never a silencing target. |
| H16 | What the suspect knows | A suspect knows a report's witness when they share an edge (any kind) or the witness lives in a district the suspect's gang holds (the street tells). Nobody else is targeted. |
| H17 | The buyer after | At fulfilment the placing agent learns the deed first-hand (`Killed` or `Assaulted`, conf 1) so the grudge settles through M15 as for a Hunt win, and remembers it (*new* `MemoryKind::Commissioned`, valence + `closure_val` for Revenge, 0 otherwise): the diary shows the relief, and the guilt is M16b's. |
| H18 | Corp brains | *new* `hitmarket::corp_pass` daily: a corp whose exec's lawfulness < `contracts.dirty_lobby` (0.3) posts on a **rival** (§ 2) a Hit on its exec (a Beat between 0.3 and `dirty_beat`), brokered at the open Fixer nearest the target, one per `corp_cooldown_days`. Lobby's culprit Hit (`post_lobby`) stays and prices at the market. |
| H19 | Gang brains | `post_gang_hit` drops the "only on an order change" timing: the gang pass runs daily (*new* `hitmarket::gang_pass`). Rival leader: any open vendetta, not only when weaker (the `hire_ratio` test moves to the score: a weaker gang pays more readily). Plus Protect (H15) and avenging a member (the leader holds a `KilledFriend` grudge through `rel_w.leader` or a comrade's death). All priced at the market from `gang_spend` × treasury. |
| H20 | Off by switch | *new* `[hitmarket] enabled = true` with `Config::hitmarket_off()` for A/B runs; `CITYSIM_HITMARKET_OFF=1`; retired with the other in-flight switches once the market is stable (TESTING.md's rule). `v1_profile` and `calibration_city` follow the shipped value (the calibration city is gangless and riot-free: the market rarely fires there, and the stat table is not re-run for it). |
| H21 | Streams | Every new draw on `rng::CONTRACT_KEY` with *new* `ContractNs::{Recant, Want}` appended (M16a C4), never `rng.world()`. The want's daily ordering is by weight then id (no draw). |
| H22 | Save | `SAVE_VERSION` + 1 (7, or max + 1 at merge if the hunger work bumps it first); every field `#[serde(default)]`; no migration code (TESTING.md). |

---

## 2. Demand: who buys a death

Each row is a daily pass over a store, never a citizen scan. "Kind" is Hit when the motive's weight ≥ `hunt.lethal_min` (or the row says Hit), else Beat.

| Motive | Buyer | Trigger (all tiers) | Kind and target | Pays with |
| --- | --- | --- | --- | --- |
| **Revenge** | an adult grudge holder | `hunt::heaviest_eligible` (weight ≥ `hunt_min`) passes H12; not hunting, not cooled; the hire score (`hunt::hire_considerations`) ≥ `contracts.hire_min` | by weight, on the grudge target | means + kin who share the grudge + credit |
| **Silence** | a suspect with an open report | report crime is Murder (Hit) or Assault, Abduction, GrandTheft (Beat); suspect free, lawfulness < `silence_lawfulness`, knows the witness (H16); witness alive, not a guard | Hit or Beat on the witness | means + credit (no kin pool: kin share a grudge, not a crime) |
| **Debt** | a creditor | an edge debt owed to it ≥ `debt_min` and older than `debt_days` (`Edge.debt_since`), or a financed asset of which it is the agent or gang lender in arrears ≥ `assets.repo_days` with nothing repossessable; creditor lawfulness < `debt_lawfulness` | Beat on the debtor; a Hit when the same debtor took a Debt Beat ≥ `debt_hit_days` ago and still owes (an example) | means + credit |
| **Protect** | a gang | an open report against a member (as Silence) with a living non-guard witness the member knows | Beat on the witness (Hit for Murder), direct to the gang's own members first, brokered when none takes | `gang_spend` × treasury |
| **Rival** (gang) | a gang | an open vendetta with a gang, its leader alive and free | Hit on the rival leader | `gang_spend` × treasury |
| **Revenge** (gang) | a gang | its leader holds a `KilledFriend`/`KilledKin` grudge ≥ `hunt_min` on an agent | by weight, on that agent | `gang_spend` × treasury |
| **Rival** (corp) | a dirty-exec corp | a rival corp, the first of: in an open vendetta with it; it took one of our buildings (`CorpShock::BuildingLost` by acquisition), poached our staff (`Poached`) or undercut us (`Undercut`) within `rival_window_days` (*new* `Corp.last_wronged: Option<(EntityId, Tick)>` written where those shocks are pushed with a known culprit) | Hit (Beat in `[dirty_lobby, dirty_beat)`) on the rival's exec | treasury ÷ `corp_reserve_mult` |
| **Culprit** (corp) | a dirty-exec corp | M16a's Lobby row, unchanged but for the price | as M16a | as above |

**Chains this closes** (the stories the market should tell, none scripted): a widow and her brother pool 31 coins and owe 31 more for the man who killed her husband; the gun kills him, she can't pay, he beats her a month later. A Sump dealer pays 25 for a beating of the neighbour who reported him; she recants. A gun stiffed twice by the same family becomes the next family's target. Militech's exec, undercut by Kiroshi for a month, buys Kiroshi's exec for 900; the strike is declined in the Spire on political cost, retaken by a squad, and the Fixer's heat closes the office for a week.

---

## 3. Pricing in detail

```
ask(gun) = ceil(week_wage(gun) × x*),  x* = u* ÷ (1 − u*),
u* = 0.3 + ln(r ÷ (1 − r)) ÷ 8,         r = accept_min ÷ Π(other mode-A terms)
       (None when r ≥ 1: the gun won't at any price)
price  = max(price_floor[kind], ask of the ask_rank-th cheapest eligible candidate)
         (quote() when fewer than ask_rank candidates ask)
```

`week_wage = max(7 × wage, 50)` as M16a set it, so a jobless gun reads a job as a mean worker would. The cheapest guns are the lawless and brave (the `1 − lawfulness` and `courage` terms), not the weakest (risk raises the ask). A high-standing target costs more only through its defenders' risk; `quote`'s standing term does not apply to market prices. `ask_rank` 2 means a first refusal moves the offer to a second gun at the same price (matching's existing loop). A record sits Open until its deadline when nobody takes it, then the want returns after `repost_days` with `tries + 1`; after `max_tries` it lapses.

**Means vs price.** A want posts when `own + pooled ≥ deposit_share × price` (agents) or `budget ≥ price` (factions). A want short of that is kept, counted `hm_unaffordable`, and re-checked daily.

---

## 4. LOD

| | Full / Coarse | Statistical |
| --- | --- | --- |
| Motives and wants | the daily passes (any tier: they read `Grudges`, `Wallet`, edges, reports, purses) | same |
| Posting | the `Commission` errand at the Fixer (H9); direct at once | at once (H10) |
| Pooling, credit, settlement | `ownership` flows (tier-free) | same |
| Taking and executing | M16a's matching, live renders and missions | M16a's ledger draw and pre-bound holes |
| Recant, case cold | daily, tier-free | same |

Nothing here promotes an agent. Live organic records rank in M16a's `contract_quota` like any record (queued by price: cheap organic records queue behind the corps'). `fviolence` is untouched: a contract killing stays the contract's own (M16a's rule), never counted in `order_rates`.

---

## 5. Diaries, bulletins and the report

- **Event texts** carry the motive and the money: `ContractPosted` "Mara Voss posted a Hit on Dex Kane for 62 (31 down, her brother's killer) at Fixer#531"; `Stiffed` "Mara Voss could not pay Jo Ruiz the 31 she owed"; `Recanted`, `CaseCold`; `ContractFulfilled` names the buyer's motive. Diaries read events naming the agent, so the buyer's week shows the walk to the Fixer, the wait, the news and the debt.
- **Shadow**: *new* archetype `buyer` (an agent who placed a Hit or Beat that is open, taken or fulfilled within 7 days of the window's start). No bounds until the behaviour tier's next calibration (as `fixer` and `gun`).
- **News**: a `Hired` rumour is a deed like any other, so Feeds can run it (M15). Not new code; phase 4 checks whether a contract story runs on any seed and lists it as a gap if not.
- **`tools/analyze_run.py`**: a "Hit market" section from `HitCols` and, with the events file, **the story list**: per organic record, the posting line, taker, outcome, days to kill, stiffed or paid, charged (`Accessory`) or not. This list is the judge's first read.

---

## 6. Config (`[hitmarket]`, *new*; every value a placeholder)

```toml
[hitmarket]
enabled = true
ask_rank = 2
price_floor = { hit = 20, beat = 8 }
deposit_share = 0.5
kin_share = 0.5
want_days = 30
repost_days = 7
max_tries = 2
max_open = 16
posts_per_day = 3
hire_courage = 0.4
commission_mins = 30
commission_flat = 0.0
stiff_grudge = 0.8              # above hunt_min 0.75, under lethal_min 0.85: a stiffed gun hunts to beat
closure_val = 0.3
silence_lawfulness = 0.3
recant_p = 0.5
cold_days = 3
debt_min = 15
debt_days = 21
debt_lawfulness = 0.35
debt_hit_days = 30
gang_spend = 0.5
corp_reserve_mult = 5
corp_cooldown_days = 30
dirty_beat = 0.45
rival_window_days = 30
```

`[contracts] hire_min`, `hire_gap`, `dirty_lobby`, `price_base` and the `[hunt]`/`[grudges]` keys are read, never retuned.

## 7. Columns (*new* `HitCols`, after `ContractCols`, before `ticks_per_sec`)

`hm_wants`, `hm_unaffordable`, `hm_posted_revenge`, `hm_posted_silence`, `hm_posted_debt`, `hm_posted_protect`, `hm_posted_rival`, `hm_posted_culprit`, `hm_price_mean`, `hm_deposit_mean`, `hm_pooled` (coins), `hm_hits_done`, `hm_beats_done`, `hm_stiffed`, `hm_recanted`, `hm_cold`, `hm_commissions` (Full errands completed). `citysim-cli/tests/cli.rs`'s header test moves.

## 8. Save impact

New: `World::wants`, `Contract.deposit`, `Contract.motive`, `Corp.last_wronged`, `MemoryKind::Commissioned`, `Flow::Pool`, `GoalKind::Commission`, `ActionKind::Commission`, `EventKind::{Stiffed, Recanted, CaseCold}`, `ContractNs::{Recant, Want}`, `Motive`. All appended or `#[serde(default)]`; `SAVE_VERSION` + 1 (H22); the save tests' mid-contract round trip gains a want and a record with a balance. The derived indices are rebuilt on load as M16a's are.

---

## 9. Acceptance (the TESTING.md tiers)

**Unit tests** (seeded worlds, one per mechanism): `ask` equals the boundary of `accept_score`; `market_price` falls back to `quote` with < `ask_rank` asks; pooling moves only sharers' coins and puts them in `known_by`; credit posts at `deposit_share` and settles the balance; a short buyer is `Stiffed` with an edge debt and a grudge of the unpaid share; `repay_debts` clears it; each motive row posts from its preconditions and not without them (Revenge weak and coward, Silence with and without knowing the witness, Debt from an edge and from a finance plan, Protect, gang Rival, gang Revenge, corp Rival from each wrong); a dead witness's case goes cold unless a guard saw it; a beaten witness recants at `p` and a guard never does; the brakes hold; a buyer never volunteers his own `Hired`.

**Core tier**: `core_sanity`'s coin identity and `escrow_leak` already cover pooling and balances. Measure "an organic Hit fulfilled" on 42-47 × 120 days: **if it fires on 4 or more of 6, it becomes a mechanism bullet** (the TESTING.md rule); else it stays a unit test and the shortfall is a finding for Dylan. The collapse bounds (Assault + Murder ≤ 42.7 a day, population) stay as they are.

**Person probes** (`tests/person.rs`): a widow whose husband was killed by a stronger man, holding 40 coins with a sharing brother, commissions a Hit at the Fixer within 7 days (control: a stronger, braver widow hunts herself); a suspect with a known witness to his assault buys a Beat on her within 7 days (control: an unknown witness is never targeted); a buyer who pays only the deposit is owed by nobody and owes the gun the balance; un-ignore `probe_buyer_keeps_his_own_hit_quiet`.

**Behaviour tier**: unchanged bounds; a failure is read against the diaries first (a few hundred buyers' errands should not move any archetype row). The `buyer` archetype's diaries are read in the phase 4 shadow pass.

**God**: one scenario (v9): `god_kill_ten_fathers_day_10` (ten household heads killed by a named stronger agent each): the world reacted if at least one widow or adult child posts a Hit within 30 days. Control unshocked.

**Printed findings** (numbers, not gates): organic Hits and Beats posted and fulfilled per seed and per motive; price and deposit means; stiffed share; recants and cold cases; accessory charges on organic buyers; contract killings as a share of Murders; Murders vs main (the old M16a "≤ 1.25 ×" is a print); the same on the wages + no-net config.

---

## 10. Interactions

- **Hunger as sickness** (in flight): the market reads `Wallet`, `law::fighting`, `Personality` and the grudge store only. If sickness lowers fighting, the sick hire more (might gap) and ask more (risk). If dumpster diving and cheaper food fatten the poorest wallets a little, means rise a little. A death from disease settles grudges on the dead as any death does. No shared file but `config.rs`/`assets/config.toml` (new sections only) and possibly `plan.rs` (H9's bypass arm): merge after the hunger phase that touches `plan.rs`, or rebase.
- **The flip** (wages + `no_safety_net` on): wallets grow (×10-50 for the employed), so means and the share of wants that clear rise sharply; the gun gate (`wage < 0.7 × median_wage`) shrinks the pool as employment reaches ~90 % and an employed gun's `week_wage` is real, so asks rise. Demand up, supply down: the market should find a higher price on its own. Every phase reports both configs. The Treasury's police budget (Jobs P5a) makes arrests and so interrogations more likely: more buyers charged.
- **M16a**: `post_gang_hit`, `post_lobby` and `try_hire` change (H1, H12, H19); matching, missions, the ledger, the accessory rule and settlement keep their code paths (settlement gains the balance). The behaviour of every Locate and Guard is unchanged.
- **M16b**: its `Debt` leverage and protection should put a threat before H's Debt Beat (a coercion first, the Beat when it fails); its `Crime::Fraud` on renege applies to a stiffing buyer; its Wounded split turns some Hits into wounds. Nothing here names a 16b type.
- **The calibration milestone**: `price_floor`, `deposit_share`, `kin_share`, `stiff_grudge` and the brakes are its knobs, not this pass's.

## 11. Out of scope

Loan sharks (a gang lending to fund a Hit; an Open question), inheritance killings (an heir buying a parent's death for the estate), love rivals, a Hit on a guard or the captain by an agent, buyers outside the city (M17's ledger), the player as buyer or taker (M18: `PostContract`/`TakeContract` already exist), Fixer competition and prices per Fixer (`price_level_b` stays 1.0), the Fixer seeding gap (god v8 gaps 2-3), fraud and fake jobs (M16b), retuning any M8-M16a knob.

---

## Open questions for Dylan

1. **Cheap death.** At market prices a Hit costs ~20-60 coins in today's dole city (about 1-3 weeks of a jobless gun's floor wage; 7-20 days of the dole). Is that the city you want, or should `price_floor.hit` sit higher (e.g. 100) so a killing stays a saving-up decision even at the bottom? (The market finds its own price after the flip.)
2. **No witness, no case** (H14) and **recanting** (H15) give silencing teeth and change the law: some Murders and Assaults now go unpunished. OK, or should a dead witness's case only slow down (a lower arrest chance) rather than close?
3. **Credit** (H4/H6): a buyer can owe half; the unpaid balance becomes a wage-garnished debt and a gun's grudge. Want it, or pay-in-full only (fewer, slower, quieter Hits)?
4. **Loan sharks**: should gangs lend to would-be buyers at interest (a richer debt chain, and gang income), now or with M16b's leverage?
5. **Kin pooling** takes half the wallet of every relative who shares the grudge, and every one of them then knows. Should a relative be able to refuse (a lawfulness or honour roll)?
6. **The exec war** (H18): dirty execs (2-4 per seed) buying rival execs could decapitate the corps (god v8: Militech bankrupt within a month of losing its exec). A cooldown of 30 days is the only brake. Acceptable, or a per-corp cap per run?
7. **The gun's floor wage** (`week_wage = max(7 × wage, 50)` from M16a) sets the whole scale of asks for jobless guns. Keep it, or tie it to the city's median wage so prices follow the economy?

## Decisions (Dylan, 2026-10-10)

1. Death is cheap: the price is set by what the guns would take, with floors of 20 for a Hit and 8 for a Beat; hard targets keep the old quote.
2. No witness, no case: a report goes cold 3 days after its witness dies unless a guard saw the crime, and a beaten witness may recant.
3. Credit and debt: half down, the rest owed out of future wages, and a gun who isn't paid holds a grudge.
4. Corp exec Hits: a 30-day cooldown per corp and at most 2 a year city-wide.
5. (Orchestrator default) Gang loan sharks come with M16b, not here.
6. (Orchestrator default) A relative may refuse to chip in, on a roll against loyalty to the buyer and their own lawfulness.
7. (Orchestrator default) A gun's weekly-wage floor follows the economy: max(50, the city's median weekly wage × 0.5), so the flip rescales Hit prices on its own.

## Implemented: deviations

One line each; the phase commits carry the numbers.

### Phase 1 (price, means, credit, quiet buyers, Revenge)

- **No `World::wants` store (H8).** The want is re-derived daily from the grudge store by the hire pass (`contracts::hire_pass`, its hash-picked day in three kept), so P1 adds no saved state but two `Contract` fields. A closed-unfulfilled organic record cools its buyer's Hunt for `repost_days` (H8's re-try) instead of a `tries` count; the grudge's decay retires the want. Phase 2's errand may add the store if it needs one.
- **`ask` by bisection, not the closed form (H2).** `contracts::ask_against` bisects mode A's product over the price (monotonic), so the ask is the exact acceptance boundary at f32; `mode_a` became `mode_a_at` (a pure refactor) so `ask` and matching read one function.
- **The deposit is exactly `ceil(deposit_share × price)`**, never "all the buyer holds"; own wallet first, kin top it up only to the deposit (minimal transfers). Kin who chip in are refunded if the post is refused.
- **Decision 6's roll**: `chip_p = (0.5 + 0.5 × edge affinity) × (1.5 − lawfulness)`, clamped (loyalty = the edge's affinity to the buyer), on `ContractNs::Want` keyed `(day, holder << 32 | kin)`.
- **Decision 7** applies to every record's acceptance with the market on (`week_wage_floor`), not only organic ones; at today's median wages (~6/day default, ~9-10 flipped) the floor is still 50 in both cities.
- **A stiffed record is `Fulfilled`**, not `Reneged`; `Stiffed` is its event. The edge debt goes to the taker only (not the crew). The gun's grudge merges with the one the first-hand `Betrayed` deed already forms, so it reads ≥ `stiff_grudge`.
- **A bought revenge Beat takes `grudges.beat_settle` off the buyer's grudge** (a Hunt's beating does the same): without it the same buyer bought the same beating again the next day (seed 46 in the first report).
- **H7 is read only with the market on**; it skips a `Hired` rumour whose actor is the teller or a gang/corp the teller belongs to.
- **Statistical and Full buyers both post at once** in phase 1 (H9's errand is phase 2).
- `HitCols` holds 12 columns (no per-motive price columns: `hm_price_sum` ÷ posted).

### Phase 3 (gangs and corps)

- **Protect moves to phase 2.** A gang buying silence for its member needs H14/H15 (cold cases, recanting) to buy anything; it lands with Silence.
- **`gang_pass`, daily, at most one head a gang a day**: its open vendettas' rival gang leaders first (`Rival`, the whole `gang_spend` share of the treasury when the weaker side by `hire_ratio`, half otherwise), else the heaviest Hunt-eligible grudge its leader holds on an outsider (`Revenge`, by weight). M16a's order-change trigger (`post_gang_hit` from `faction::rescore`) is retired with the market on.
- **`corp_pass`, daily**: the rival is the latest wrong (`Corp.last_wronged: (corp, tick, Wrong)`, written at the hostile buyout, the poach and both undercut sites) inside `rival_window_days`, then corps in an open vendetta. One Rival record per corp per `corp_cooldown_days` (read off the board, no state); decision 4's city-wide cap counts Rival Hits **posted** in 365 days (`World::hm_corp_hits`, saved); at the cap a dirty exec buys nothing (no downgrade to a Beat).
- **Factions pay in full at the market price** (no credit); a faction waits `repost_days` before buying the same head again (`bought_lately`, read off the board). Lobby's culprit record is priced at the market with `Motive::Culprit`.
- **Finding for Dylan: execs are cheap.** The strike estimate sees few defenders around an exec, so a rival exec's market price is 20-60 coins (the same as a nobody's); only gang leaders and a few others hit the `quote` fallback (300-1,500). Decision 1 says hard targets keep the old quote; by the risk estimate an exec is not hard.

### Phase 3 follow-up (the orchestrator's two fixes)

- **Lobby waits.** `post_lobby` reads the faction repost wait (`hitmarket::bought_lately`): a live record on the head stands (the bribe is skipped), one closed within `repost_days` is not re-bought (the bribe goes ahead).
- **Execs have protection: `[hitmarket] exec_guards` = 3 is a stand-in for real bodyguards** (roadmap addendum 13, ambient LOD: execs travel with company; retire the key when those land). No exec had a workplace guard or a guarded Home (8 of 9 execs hold no Job; private guards stand only at buildings their corp owns or holds a guard contract on), so `missions::expected_defenders` adds, for a corp's exec, up to `exec_guards` private guards of the corps protecting it (its own when it runs Security Offices, and every Security corp with a guard contract on one of its buildings), strongest first, on or off shift. The price, the ledger strike and matching all read it. A corp with no security keeps a cheap exec (approved). Seed 42, day 30: 5 of 9 execs now fall to the `quote` fallback (1,269-1,394), up from 2; Vatra, Habitat, Stackwell and Kessler stay at 22-40.

### Phase 2 (silence, debt, the walk to the Fixer, Protect)

- **Wants are for the walk only (H8, H9).** `World::wants` (saved) holds a want only for a buyer at Full or Coarse with an open Fixer in reach (`hire_reach_tiles` of home); a Statistical buyer, or one with no Fixer in reach, posts at once (direct when no Fixer). The kin are counted, not asked, when the want is written (`kin_potential`); they roll at the Fixer. The `Commission` step re-prices and re-pays everything there. A want lapses at `want_days`, when its Fixer closes, when the motive goes (the grudge under `hunt_min`, the report closed, the debt off the clock), or (D8) when the holder hunts the target by other means; a buyer demoted to Statistical posts at once (`want_sweep`, daily, before the hire pass). Factions keep no want.
- **The Commission goal has no travel term** (H9 as written: gate, weight Linear{0.8, 0.2}, `1 − lawfulness` Linear{0.5, 0.5}, flat 0). The step is a contract step (`is_contract_step`; `contracts::can_start` checks the Fixer open and the buyer at its door).
- **Every agent buyer waits `repost_days`** after its own organic record on that head closes (done or not), every motive; P1's Revenge used the Hunt cooldown alone. The placing agent's `Origin` is `Hunt` for every agent motive (who decided: the agent).
- **No `World::cold_due` (D10).** `hitmarket::cold_cases` walks the open reports daily: a named witness not living (`Corpse.died_tick`, or gone) `cold_days` after the death, with no city guard holding a `SawCrime` of that suspect and crime (any age; memory decays), resolves the report (`CaseCold`). Any crime with a named witness goes cold, not only the four silenced kinds (decision 2: no witness, no case). The cold check and the recant run from `contracts::daily` and `contracts::set_status`; `law.rs` is untouched.
- **Silence weights**: Murder 1.0 (a Hit), Abduction 0.9, Assault 0.7, Grand Theft 0.6 (Beats). A gang member's reports are his gang's to buy (Protect), never his own; a witness with a live organic record on her is not bought twice (any buyer).
- **Debt has its own clock.** `Edge.debt_since` is re-stamped every fortnight by `social::daily`, so it cannot age a debt: `World::hm_debts` (saved) keys `(creditor, debtor)` with the day the debt was first seen at `debt_min` and the last Debt Beat done on the debtor. The pass scans every edge once a day, unordered (there is no creditor index; D14 asked for creditors' wants first; the A/B below shows no cost). A finance debt is any financed asset still on its owner with `arrears ≥ repo_days` and an agent or gang lender. Weights: Beat 0.6, Hit 0.9. A gang creditor pays in full from `gang_spend` × treasury.
- **Protect runs first in `gang_pass`** (a member's witness, then the feuds, then the leader's revenge; one head a gang a day). D12's move to the held district's Fixer happens at the next midnight (one `match_hour` missed); a gang that cannot escrow the price keeps the record direct.
- **H17**: the placing agent (a gang's record: its leader) gets the deed first-hand as a rumour (`Killed`/`Assaulted`, actor the taker, hops 0, conf 1) and a `Commissioned` memory (salience `max(deed_sal, 0.5)`, valence `closure_val` for Revenge, 0 otherwise). The grudge settles as it did (the death; `beat_settle` for a Revenge Beat). A done Debt Beat starts the Hit's clock.
- **Recant (H15)** rolls on `ContractNs::Recant` keyed `(day, record id << 16 ^ report index)` for each open report the beaten witness filed against the buyer (Silence) or a member of the buying gang (Protect). A guard never recants.
- **`HitCols` is 18 columns**: `hm_posted_other` split into `hm_posted_silence`, `hm_posted_debt`, `hm_posted_protect`; plus `hm_wants`, `hm_recanted`, `hm_cold`, `hm_commissions`.
- **`SAVE_VERSION` stays 8** (renumbered at the merge onto the flip; P2's `World::wants` and `World::hm_debts` are serde defaults in the same unmerged version).
- **core.rs**: M9's "a day in Crackdown" bullet is demoted to a printed finding (TESTING.md's rare-bullet rule): on the flipped main it held [4, 9, 0, 1, 0, 0] days on 42-47 (3 of 6), with P2 [0, 0, 0, 4, 6, 0]; `law_faction.rs::test_posture_patrol_at_rest_and_crackdown_under_pressure` carries it.
- **Rare rows are unit-tested** (`tests/hitmarket.rs`): Debt fired on 0 of 42-47 (edge and finance tests, Beat then Hit), Silence on 1 of 6 (with and without knowing the witness, a lawful suspect, a Murder is a Hit), recants on 1 of 6 (by courage, never a guard). The person probes (`tests/person.rs`): the widow at day 10 (on day 2 too few regulars ask a finite price and a Hit falls to the quote), her brother holding 200 (with 40 each no want formed in 7 days), the killer the stranger with the fewest defenders; the suspect and his known witness at day 2, 22:00 (he posts, then is jailed).
- **A direct record on credit holds its deposit (fix round, overrides D5).** The deposit is escrowed at posting whether the record is brokered or direct, under the same `escrow_leak` identity; at fulfilment the taker gets the deposit plus whatever balance the buyer can pay, and a short buyer is stiffed for the balance only. Before, a direct buyer's deposit was only checked at posting, so the buyer could spend it and be stiffed 33 of 36. A renege on a held deposit pays the taker the deposit and keeps the balance. A sold-out record on credit adds the top-up to its deposit, so the balance is not charged twice (this also fixes the brokered case).
- **Revenge text**: a killed kin or friend whose entity is gone reads "for a killing" (it read `#2521`).
- **Finding, "nobody takes"** (`hit_probe.rs::why_expired`, `trace_record`): most organic records that expire were taken. Over 42-47 (fix round), 32 expired after a taker held them and 14 were never taken. Seed 44's one expired Revenge Hit (#55, direct, 42 coins, 21 down) had 70 viewers: 50 filtered as too lawful (> `gun_lawfulness` 0.3), 4 guards, 4 not free; of the 12 left, two would take it at 42 (asks 42 and 35) and one did at 18:00. The taker is an employed fabber: he worked 08:00-18:00, chased from 20:00 until Sleep won, staked out once on a day off with no contact, and the record expired 10 days later. Neither the price nor a filter is the problem. A direct record's viewers are never put through `gun_gate`, so a day-job taker works his shifts and chases in the evenings.
- **A direct Hit or Beat's viewers pass the gun gate (fix round).** `contracts::gun_gate` (best skill ≥ `gun_skill_min`, jobless or paid under 0.7 × the median wage, a free adult) now filters a direct violent record's viewers in matching and in `market_price`'s direct pool, as networking filters a Fixer's regulars. With the market off, M16a's viewers are unchanged; direct Guards, Locates and city guards on the take (C31) are unchanged. After the gate, on main `8b6841a`, over 42-47 × 120 days: organic Hits fulfilled per seed 1/1/1/0/1/0 (4 of 6); expired after a taker held them 4/6/2/6/2/5 (25), never taken 0/7/1/4/0/3 (15).

### Phase 4 (acceptance, the story list, the reads)

- **No `core_sanity` bullet: "an organic Hit fulfilled" is a printed finding.** `hm_hits_done` (fulfilled or reneged) over 42-47 × 120 days: [1, 1, 1, 0, 1, 0] on `fe60d37` (4 of 6), [1, 1, 1, 0, 0, 0] with this phase's D8 fix (3 of 6), 2 of 6 on the main before the P2 fix round. One a seed at most: it flips with any trajectory change. `core.rs::printed_mechanisms` prints it; the unit tests (`hitmarket.rs`), the widow probe and `god_kill_ten_fathers_day_10` carry the mechanism. **Finding for Dylan: the plan's "Hits fulfilled on most of 42-47" is not met.** The cause is supply and reach, not demand (below).
- **D8 fix: a buyer does not hunt the head it is paying for** (`hitmarket::paying_for`, read in `hunt::considerations` and `hunt::adopt`; a god's Hunt still goes ahead). Three of six buyers' diaries hunted their own target while their paid record stood: the 3-day Hunt cooldown ran out (Kofi Kowalczyk, seed 44; Fenn Vance, seed 45), or (Zoe Kaneda, seed 46) a Hunt adopted through the hire path posted, set the 3-day cooldown, then failed to plan and `World::cool_goal` overwrote it with the planner's 2 hours; she killed the target herself 3 hours after paying. Unit test `test_buyer_does_not_hunt_the_head_it_paid_for`.
- **The `buyer` shadow archetype** (`--pick buyer`): a want's holder or the placing agent of an organic record live or posted in the last 7 days. Diaries only (no `CHECKS` rows). The diary's contracts section prints an organic record's motive and deposit. `hit_probe::hit_market_report` prints each record's buyer and target entity indices (for `shadow --agent`).
- **The story list** (`tools/analyze_run.py`, "the Hit market"): the `HitCols` totals, and with an events file every organic record chained from its posting by (kind, target name): buyer, motive and why, price, deposit, Fixer, takers, attempts, outcome, and the city's answer (stiffed, reneged, a Conspiracy charge, a Hunt on the taker or the buyer within 30 days, a case gone cold, a report taken back). Names, not ids: a Stiffed line naming a crew member of a gang taker is not matched.

**The story list, 42-47 × 120 days (with the D8 fix).** 90 organic records: 10 done and 1 reneged (Hits: Kasimir Ostrowski's and Kofi Kowalczyk's revenge, Ninefold's rival Hit), 43 expired, 28 failed, 3 cancelled, 5 live. 57 brokered, all at Fixer#531 (the one open Fixer on every seed), 33 direct. Wants that could not pay the deposit (`hm_unaffordable`, tries): 250-633 a seed. The city's answer on the 11 done: 6 buyers charged with Conspiracy (0, 2, 5, 5, 10, 13 days after), 3 stiffed balances, 1 renege, 1 Hunt on a taker (Casimir Kimura after Brick Singh, seed 43, twice), no cold case or recant on a bought witness (`hm_recanted` 0 on every seed; Vex Haas's read below has one).

**The reads** (the buyers' diaries, `shadow --agent <id> --start-day <posting day − 1> --days 7`; pinned, so each week diverges from the unpinned run):

1. *Kasimir Ostrowski* (seed 42, sanitation, lawfulness 0.73, courage 0.07; his kin killed by Mateo Sato). Files toward a report, then goes after Mateo himself, attacks and loses (d34). Five days later, after a 9-hour shift, walks 3 h 48 to Fixer#531 with 9 coins; three Friends put in 20; posts a Hit for 58, 29 down, walks home 1 h 37 and sleeps 1.9 h. Reads true but for who he is: a lawful man buys a killing the week his report is open.
2. *Mei Bellamy* (seed 42, a bouncer who assaults freely, lawfulness 0.25). At midnight, on shift, buys a Beat on the witness to one of her assaults: 17 of her 20 coins down, direct (no Fixer in reach). Quill Adeyemi beats the witness that evening; Mei cannot pay the 14 balance, Quill swears a betrayal grudge, hunts her next morning and assaults her on d34. Meanwhile she tells two people the beating she paid for. The best story of the six: the city answered with the gun's grudge within 14 hours.
3. *Kofi Kowalczyk* (seed 44, jobless Unplugged Purist, 60, 0 coins). Out of jail at 23:20, networks at the Fixer as a gun himself, sells a sighting for 15, and with that bounty commissions a Hit for 24 (12 down) on his kin's killer. His own `Hired` deed is then told on the street about 15 times in three days (none by him: the Fixer's owner holds it first-hand, C21, and the street passes it on). Three days on he hunts Jasper himself, the paid record still live (fixed above).
4. *Zoe Kaneda* (seed 46, Rust Saints, courage 0.04, much stronger than the target). Out of jail at 00:28, at a club at 03:04 posts a direct Hit for 28 (14 down); at 06:39 hunts Tamsin herself and kills her at 14:06, the deposit refunded, the body stripped of 15 coins. Two Feeds run "Zoe Kaneda avenged Tamsin Volkov". The cowardice clause (H12) chose the purchase; the Hunt took the killing anyway (fixed above).
5. *Vex Haas* (seed 42, homeless Hollow member). The midnight he is hired as a Vat Tech he leaves the gang, moves into a Block, and buys a Beat (17 of 21 coins down) on Bex Novak, who had been telling the street about his assault. Bex is beaten at 16:11 and takes back her report at once; Vex is stiffed for 4, repaid from wages later. Silence works and reads true.
6. *Fenn Vance* (seed 45, Zetatech mechanic, lawfulness 0.81). Walks to the Fixer before his shift (interrupted by Work), again after it, and buys a Beat on his kin's killer for 24 (12 down); three days later hunts him himself while the Beat is open (fixed above).

**Gaps** (what reads false; for the next pass, not fixed here):

- **One Fixer, and the market stops at its reach.** Every seed runs one open Fixer (#531). Off its `hire_reach_tiles`, a record is direct and its price is the buyer's own circle's asks; with fewer than `ask_rank` guns through the gun gate there, it falls back to the M16a quote (465-1,167 in the god scenario, 8 of 10 widows). Death is cheap only near Fixer#531. I'd price a direct record with too few asks at the nearest Fixer's market price plus a reach premium, and look at why no NPC founds a second Fixer (god v8 gaps 2-3).
- **Lawful buyers.** Kasimir (0.73) and Fenn (0.81) bought violence with an open report or no report at all. The hire score's `1 − lawfulness` term (Linear 0.5, 0.5) costs a lawful man at most half. I'd let the law answer first: a lawful holder whose report on the killer is open or whose killer is wanted or jailed waits for it (the god scenario: 3 of 10 killers jailed within days).
- **The Fixer's owner talks.** A brokered buyer's `Hired` reaches the street within hours through the owner's first-hand copy (C21; ~15 tellings in three days for Kofi), and 6 of the 11 done records' buyers were charged. I'd give a Fixer's owner H7's silence on her own book (the leak stays through interrogation and through kin who pooled).
- **The gun's grudge is instant.** Decision 3 says the balance is owed out of future wages; the stiffed gun swears a grudge at settlement and hunts the next morning (Mei). I'd let the debt run a grace (the Debt row's clock, `debt_days`) before the grudge counts toward a Hunt.
- **The buyer gossips the deed she bought** (Mei told two people of the beating). H17 hands the buyer the deed first-hand; H7 keeps only `Hired` quiet. A buyer should keep the bought deed to herself too.
- **Takers who cannot close** (the phase 2 item): 43 of 90 expired, most after a taker held them; gang crews win the fight and leave the target alive ("won (2 vs 1, 0 dead)": a failed attempt). Not read further here.
- **The Fixer's book is thin early** (the phase 2 item): confirmed in the god scenario, a widow 2 coins short of a 38-coin Hit's deposit on day 10, the killer jailed by the time she could pay.
- **Decision 7's floor** stays 50 at the shipped median wage (6-9): no change read in any diary (every price met was 22-67 for a soft target).

**God v9 (`god_kill_ten_fathers_day_10`, `docs/GOD_SCENARIOS_V9.md`)**: 1 of 10 widows placed a Hit (26 coins, fulfilled the same day), control 0; 2 hunted the killer themselves, 3 killers were jailed by the law, 4 widows held a 0.85 grudge on a free killer for 30 days and could not buy (off the Fixer's reach, a Hit on him priced at the quote, 465-1,029). Passes; thin.

### Review fixes (`/code-review high 8270896..HEAD`)

- **A want stands for its own head.** `agent_buy` with a want already held (no `at`) reports a hire only when the want's target is this head, whatever its motive or kind (as `paying_for` reads); a want on another head buys nothing and returns false, so the caller's Hunt goes ahead on the second head (before, it reported a hire that never happened and the Hunt goal re-scored and failed for up to `want_days`).
- **A bought revenge Beat settles a light grudge.** `grudges::answer_beating` (shared with the Hunt's winning strike): `beat_settle` off the weight, settled at weight 0 when it reaches 0 or under.
- **Lobby's culprit record is under the brake.** Past `hitmarket::room` (`max_open`, `posts_per_day`) `post_lobby` posts nothing and returns false: the bribe goes ahead (the corp still answers the culprit, by the legal path).
- **A Debt creditor is a living free adult** (`contracts::free_adult`, as Silence's suspect): a dead creditor (edges kept, no Wallet) is no longer priced every day into `hm_unaffordable`, and a jailed or cuffed one buys nothing.
- **A finance Debt's text names the arrears**: "over N owed" reads the edge debt plus, for each financed asset of the creditor's on the debtor in arrears ≥ `repo_days`, `per_day × arrears` (at most what remains). It read "over 0 owed".
- **The corp Rival order follows the spec's row: open vendettas first, then the latest wrong** (a feuding corp that also wronged us is tried once, as a feud). Phase 3 had it the other way. The cap rule stays as phase 3 recorded it (decision 4 sets the cap; the decisions table says nothing on a downgrade): at the cap a Hit-grade exec buys nothing, no Beat.
- **Workplace floors only under the room cap.** `founding::floor_seats` and `with_floors` take the config: a Farm's or a Fab's floors multiply its `capacity` only with `[economy2] wages` and `room_cap` (the switch `wages::room_capped` reads); the dole city and a cap-off run keep one floor's room as before the workplace-room change. `add_floor`'s `per_floor = capacity ÷ floors` is right on a fresh world under both rules (off: `with_floors` returns `per_floor` and `.max(capacity)` keeps the room; on: one floor's room added). A world saved under one rule and run under the other (or a v7 save with a multi-floor Farm at one floor's room, run with the cap) keeps its stored capacity and the next floor adds `capacity ÷ floors`; no migration code (TESTING.md).
- **Perf, byte-identical**: `law::located` runs the vagrancy report walk after the cheap checks; `missions::exec_guards` finds no exec without copying the corp list (`World::corp_slice`); `contracts::ask_against` computes the price-independent terms once and folds them after the price term in mode A's order (the same f32 operations, so the boundary is bit-identical; a unit test in `contracts.rs` checks it against the old bisection over 3 kinds × 2 broker modes × 4 defender strengths × every fifth citizen). Seed 42 × 30 days: the `--report` CSV (less `ticks_per_sec`) and the `--events` stream are identical to `9f6214b`'s; `core_throughput` A/B alternating (main, new, main, new): 8,528 / 8,877 / 8,899 / 8,921 ticks/s (noise).

### Gaps pass (the phase 4 gaps, 2026-10-11)

Numbers: seeds 42-47 × 120 days, `citysim-cli run --days 120 --seed S --report --events`, main `7324e60` → this pass, read with `tools/analyze_run.py`'s story list. Each fix has a failing-first unit test in `tests/hitmarket.rs` (all six failed on `7324e60`'s source).

| 42-47 × 120 d | main | gaps pass (15aee9a) | with the fix round |
| --- | --- | --- | --- |
| organic records posted (Hits) | 12 / 24 / 14 / 16 / 11 / 13 = 90 (47) | 11 / 33 / 19 / 16 / 13 / 14 = 106 (56) | 14 / 28 / 16 / 27 / 13 / 19 = 117 (69) |
| organic Hits fulfilled (`hm_hits_done`) | 1 / 1 / 1 / 0 / 0 / 0 = 3 (3 of 6 seeds) | 4 / 5 / 4 / 2 / 1 / 2 = 18 (6 of 6) | 4 / 2 / 5 / 6 / 1 / 1 = 19 (6 of 6) |
| organic Beats fulfilled | 3 / 2 / 1 / 0 / 2 / 0 = 8 | 2 / 2 / 1 / 7 / 0 / 2 = 14 | 1 / 4 / 0 / 6 / 0 / 1 = 12 |
| records done / expired / failed | 10 / 43 / 28 | 30 / 34 / 30 | 31 / 44 / 38 |
| stiffed / buyers charged (Conspiracy) / guns hunting a buyer | 5 / 6 / 0 | 13 / 14 / 2 | 19 / 13 / 1 |
| Murders (events) | 105 / 98 / 78 / 75 / 54 / 92 = 502 | 84 / 78 / 84 / 98 / 83 / 82 = 509 | 87 / 88 / 93 / 116 / 78 / 91 = 553 |
| Hunts started | 85 / 51 / 62 / 38 / 41 / 52 = 329 | 70 / 63 / 83 / 66 / 57 / 50 = 389 | 82 / 55 / 78 / 77 / 59 / 63 = 414 |
| seeded corps alive on day 120 | 7 / 9 / 9 / 8 / 9 / 9 = 51 | 7 / 8 / 8 / 9 / 9 / 9 = 50 | 7 / 9 / 9 / 9 / 9 / 9 = 52 |

- **Reach** (`contracts::reach_offer`, *new* `[hitmarket] reach_premium` 0.5): an agent buyer with no open Fixer within `hire_reach_tiles` whose own people give fewer than `ask_rank` asks sends word to the open Fixer nearest its Home (any distance) at that Fixer's market price × 1.5, brokered there at once (no walk), instead of a direct record at the M16a quote nobody of its own would take. Deviation from the gap's wording ("price it at the nearest Fixer's price"): the record is also *brokered* there, because a direct record with under two asks has nobody to take it at any price (on main, 13 of the 43 expired records were direct quote-priced records never taken). Factions keep their own rules. Brokered organic records 57 → 73; the direct records still at the quote (17, 12 expired untaken) are targets the Fixer's guns would not take either (decision 1: hard targets keep the quote). Test: `test_off_reach_buyer_goes_to_the_nearest_fixer_at_a_premium` (control: two guns among the buyer's friends keep it direct at their price).
- **Why no NPC founds a second Fixer** (read, not fixed): no plain bug. `founding::fixer_ok` needs lawfulness < `fixer_lawfulness` 0.35, persuasion + knowledge ≥ `fixer_skill` 0.8 and a founder through `can_found` (jobless, since the flipped city's dole is 0, no gang, no exec) holding `found_cost.fixer` 400. A probe at days 20-120 on 42-47: 1,860-1,980 adults, 330-960 jobless, 72-290 of them lawless, 2-14 also skilled, 1-13 also gangless, and those hold 2-68 coins (one holder of 485 on seed 44 by day 120); 0 pass `fixer_ok` at any snapshot. The cap (`fixers_per_pop` 800: 2 at 2,000) is not what binds; seed 44 did found a second Fixer early. The founder pool is the jobless lawless, who are broke in the no-net city. A calibration or design call (a lower `found_cost.fixer`, a gang or an employed founder), not this pass's.
- **Lawful buyers wait for the law** (`hitmarket::waits_for_law`, *new* `[hitmarket] lawful_wait` 0.6): a revenge buyer above 0.6 lawfulness whose target is wanted (an open report: its own or anyone's) or held (sentenced or cuffed) buys nothing and its Hunt by the hire path stays cooled; a want on the way waits (the Commission goal does not score, the step at the Fixer keeps the want) until the law lets go or `want_days` lapse. Kasimir (0.73) and Fenn (0.81) of the phase 4 reads would wait; with no case open (Fenn's) he still buys. Tests: `test_lawful_buyer_waits_for_the_law`, `test_lawful_buyers_want_waits_for_the_law`.
- **The Fixer's owner keeps her book quiet** (`gossip::keeps_quiet`): with the market on, a Fixer's owner never volunteers a `Hired` she holds first-hand (hops 0: the copy `post_with` and the Protect re-broker write her). Interrogation, her C21 talk to her regulars at a Drink (kept: an M16a channel with its own test and `[law] fixer_talk` knob), pooled kin, the gun and everyone she did not tell still spread it. Conspiracy charges on organic buyers still came (6 → 14, with 3× the records done): interrogation is the law's road. Test: `test_fixer_owner_keeps_her_book_quiet`.
- **The stiffed gun's grudge has a grace** (decision 3; *new* saved `World::hm_stiffs`, `SAVE_VERSION` 9): the gun swears the grudge at the stiffing as before, but `hunt::heaviest_eligible` skips it while the balance is on its `debt_days` (21) clock. `hitmarket::stiff_sweep` (daily, before the hire pass): paid off out of wages inside the grace (`social::repay_debts`) → the `Betrayed` grudge settles, no Hunt; 21 days unpaid → the clock drops and the gun swears again at the stiffing's weight (it decayed 0.01 a day through the grace), so the grudge counts from then. On seed 43 Vale Fletcher, stiffed 557 on day 65, hunted the buyer on day 92. Tests: `test_stiffed_gun_waits_out_the_grace`, `test_stiffed_gun_paid_in_the_grace_lets_it_go`; the save round trip carries the clock.
- **Buyers keep the bought deed to themselves** (`gossip::keeps_quiet`, `hitmarket::bought_deed`): a placing agent never volunteers H17's first-hand `Killed`/`Assaulted` whose actor took (taker or crew) a fulfilled record it placed on that object; other first-hand deeds it still tells. Test: `test_buyer_keeps_the_bought_deed_to_herself`.
- **Takers who can't close: a broken loop, fixed** (`contracts::ledger_stale_queue`, *new* `[hitmarket] queue_hours` 24). Read with an hourly probe of every organic record's state on 42-47 (main): 12 expired records spent 150-246 h of a 168-240 h life Taken but QUEUED, never started, and many others queued for days. In 3,856 of the 4,113 queued record-hours the `[missions] max_missions` (12) live slots were full (the rest: the bodies' `contract_quota`), held on average by 7.6 Locate runs (14-day public and corp bounties) and 1.9 Guards; the queue drains by price only when a slot frees. Now a taken organic Hit or Beat still queued `queue_hours` after the take is worked on the ledger: a `due` over half the time left (`ContractNs::Due`, key `1 + attempts`), the strike rolled by `resolve_ledger`, which now strikes with a gang's or a squad's crew as one party (a gang's lead is its first free crew member; a solo taker is unchanged). `max_missions` is not retuned. The M16a records (corp, law, god) queue the same way and are left as they were: a finding. What is left of "can't close" is the missions' own dice and design: over 42-47 with this pass the failed attempts read strike called off on political cost (19), target not at the door (22), crew never reached the door (17), the taker died (6), a crew won and left a Hit's target alive (6: the mission's one last strike at `hunt_kill_p`, as designed; a solo taker who wins a Hit goes back to the chase instead). Test: `test_queued_record_goes_to_the_ledger` (a solo gun and a gang's job, every slot held).
- **Why Debt never fires** (read, not fixed): unreachable in today's city, no bug. A daily probe on 42-47: 1,190-1,640 edges carry a debt at day 120, the largest 5-11 coins; no edge debt reached `debt_min` 15 on any day of any seed (the balances stiffed on main were 8-12, repaid from wages; begging loans are 1-2). Financed assets in arrears ≥ `repo_days` with an agent or gang lender: 0-1 at a time, the oldest clock 10 days against `debt_days` 21 (repossessed or paid first). With this pass's bigger balances (a stiffed 557 on seed 43) an edge debt can now cross 15, but Debt still posted 0 records. The unit tests (`test_debt_beat_then_hit`, `test_debt_from_finance_arrears`) carry the mechanism.
- **God v9** (`god_kill_ten_fathers_day_10`): Hits placed by a widow within 30 days 1 → 5 (4 widows; control 0): three off-reach widows now send word to Fixer#531 at 33-42 (the scenario's header still prints the direct quote, 465-1,029, as its estimate); 1 fulfilled, 4 failed attempts' end (missions), 3 killers jailed, 2 dead (one by the Hit, one by a widow's own Hunt). All 22 god scenarios pass.
- **Finding for Dylan: "Hits fulfilled on most of 42-47" is met** on this run (6 of 6, 1-5 a seed), but per the rare-bullet rule it stays a printed finding until it holds across trajectories.

**Fix round (read-only review of `15aee9a`).** Each item has a failing-first test (all four failed on `15aee9a`).

- **The ledger fallback never kills a body off-screen** (`contracts::ledger_ok`, `jumps_queue`). `15aee9a` moved any stale queued record to the ledger, so a Full, Coarse, pinned or on-screen target could die by `lod::stat_hit` ("assailant unknown") with no scene. Now a stale record goes to the ledger only when its target is no body (`render_for`'s test) and none of its taker or crew is at Full, pinned or on screen; a crew member or a solo taker walking off-screen at Coarse may still strike from the ledger, and a lost ledger strike never kills a lead who is a body (the organic records'; M16a's unchanged). A record that may not go to the ledger stays live; a *solo* one, stale, starts past the `max_missions` run count (the bodies' `contract_quota` still holds, `max_open` 16 bounds them). `resolve_ledger` sends an organic record back to the live queue if its target became a body after it went there. Three variants were measured on 42-47 before choosing: the strict `render_for` test for every party (a Coarse crew member keeps a gang's job queued: the LOD pass promotes members) with solo jumps: Hits done 18 → 4, expired 34 → 57, Murders 533; every stale shown record jumping the cap, missions too: Hits done 4, failed attempts 60, Murders 632 (door brawls; off-screen Murders 368 → 486); the shipped rule (target no body, nobody in front of the player): the table's last column. Tests: `test_stale_queued_record_with_a_shown_target_stays_live` (a pinned target: never ledgered, and the stale solo record starts live with every slot held); `test_queued_record_goes_to_the_ledger` now demotes the party and keeps one gang crew member at Coarse.
- **The grace's end restores, never merges** (`hitmarket::stiff_sweep`): the unsettled grudge on the buyer goes back up to the stiffing's weight (`max`, not `grudges::add`'s `1 − (1 − a)(1 − b)`, which raised a standing 0.872 to 0.974); sworn afresh only when it decayed away. Test: `test_grace_end_does_not_overweight_the_grudge`.
- **Only the betrayal waits** (`hunt::heaviest_eligible`): the grace filter skips a grudge on a stiffing buyer only when its cause is `Betrayed`; a gun who already held a killing grudge on the buyer (the stiff merged into it) still hunts. Test: `test_grace_does_not_hide_a_killing_grudge`.
- **A pruned edge is no repayment** (`stiff_sweep`): the balance counts as paid only while the edge stands with nothing owed; an edge pruned with the debt on it keeps the clock (the grudge counts when it runs out). The balance itself is not tracked apart from `Edge.debt`, so a debt lost with a pruned edge is never repaid: the gun's grudge counts after the grace. Test: `test_pruned_debt_edge_is_not_a_repayment`.
- God v9 with the fix round: Hits placed by a widow 5 (control 0), 1 done; all 22 god scenarios pass. Murders 553 against main's 502 (+10 %, 16 of them the extra Hits; the strict variants read 526-533 with 4-5 Hits done): printed, not gated.
