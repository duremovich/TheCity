# Big-picture review, 2026-10-07

Dylan asked for a step back after M14 closed, M15 phases 1-4, the shadow pass, Life L1/L1b and the Fable code review: where does the project stand against `VISION.md` and `ROADMAP_POST_M14.md`, and is the path (M15 phase 5, Life pass L2, M16-M18, a late calibration milestone) the right one? This is that review. Everything below is measured on main at 1eb11a3 (the Fable review fixes), seeds 42 and 43, 120 days, `citysim-cli run --report --events`, plus seed 42 with `--word-off`, `--virt-off` and `--life-off` for attribution. Raw CSVs and event logs are regenerable with those commands.

**Verdict in one paragraph.** The order of the roadmap is right and the process (specs with overturnable decisions, data-first gates, god suites, the shadow pass, a review per milestone) is working: five days of work stand at ~49k lines of sim, every gate green, and the vision's five stories are each either built or specified. But three things underneath the milestones are wrong, and all three get worse with every feature that lands on top of them: **throughput slides 3x over a run and already touches the floor on day 120**; **the economy is the dole**, which makes M16's prices meaningless and has been quietly bending every calibration since M11; and **off-screen residents can no longer be murdered**, which breaks the LOD promise the whole architecture rests on. None of these is a milestone's bug. They are the base city, and I recommend one milestone on the base city before M16, in place of (and wider than) Life pass L2.

## 1. Where the project stands

| Vision story | State | Evidence (seed 42 / 43, 120 days) |
| --- | --- | --- |
| Friends: your friend is killed, you go hunting | built (M15 p3), thin | Hunts 50 / 58; Avenged 5 / 12; revenge killings 2 / 5 (spec band 3-20); longest chain 2 / 1 (spec ≥ 2) |
| Word gets around | built (M15 p1), saturated | second-hand share 0.59 / 0.57 (band 0.30-0.70); median `known_by` of a killer at 7 days 17-436 (band 25-150, most samples above it); pool reach 178-192 of a 192 cap |
| Assassinations, hired guns, fixers | specified (M16) | `Contract`, `Fixer`, `Hit`: 0 lines of code; hooks exist (`Governance` enum Dictator-only, `alertness_mult` stub at 1.0, `Corp.parent`, `Sighting` relay) |
| Quests are real contracts | specified (M16) | same |
| Rich lives on inspection | built (M10), one hole | `Life`, `Trace`, the binder, the Story tab all stand; but 93-99 % of violent deaths are on-screen (§ 4) |
| Break, take over, exist | specified (M18) | `PlayerControlled`, `Circle`: 0 lines; `Register`, the gang bootstrap, the levers and god commands exist |

The city, main, 120 days: population 1,957 / 1,960 on day 120, starvation deaths 10 / 16, violent deaths 153 / 155, thefts 15.2k / 15.1k, riots 4 / 4, arrests 2.8k / 2.6k, mean jailed 115 / 105 of 160 (pinned at 160 from ~day 70 on both seeds), employed 267-310 of ~1,950 adults, ticks/s mean 7.8k with a day-120 reading of 4.1k / 4.8k.

Code: `citysim` 49k lines over 43 systems, 25k lines of tests (12 scenario gates, 3 god suites with 38 scenarios), `config.toml` 1,410 lines in 51 sections, the app 6.5k lines, the CLI 5k. 159 commits and 116k insertions since 2026-10-03.

## 2. Throughput slides 3x over a run, and the gate reports the mean

Ticks/s on seed 42 by window, four configurations of the same binary:

| run | days 0-9 | days 50-59 | days 110-119 | Coarse on day 119 | gang members | violent deaths |
| --- | --- | --- | --- | --- | --- | --- |
| main | 13.0k | 7.2k | **4.5k** | 308 | 177 | 153 |
| `--word-off` (no M15) | 14.0k | 10.5k | 5.2k | 303 | 168 | 130 |
| `--virt-off` (no M14) | 13.2k | 8.5k | 5.2k | 282 | 164 | 149 |
| `--life-off` (no L1) | 13.7k | 7.8k | 5.0k | 304 | 169 | 143 |

The slide is in every configuration, so it is the base city's, not M14's or M15's: each of those costs 10-15 % on top at day 120. Three drivers are visible in the CSV:

- **The Coarse tier runs at twice its budget, and the overrun is the Jail.** `[lod] max_coarse = 150`; the tier sits at 240-312 from day 30 on. *Correction (2026-10-08, from the L2 spec pass):* the first draft of this review blamed the forced classes (gang members "never Statistical", guards, runners, hunters); in fact `lod::assign` sets sentenced and emigrating agents Coarse outside the ranked slots, so the tier is `max_coarse` plus the jailed (150 + 158 = 308 on day 119, 150 + 140 on day 30), and gang members over the slots already fall to Statistical by rank. The conclusion stands: the budget is nominal, a prisoner costs a body for nothing, and M16 adds Fixers, crews, captives, the wounded and squads to the classes that want bodies, M18 a circle of 24. The fix (held Statistical prisoners, a quota per forced class) is in `docs/LIFE_L2.md` § 4.
- **Violence rises 10x across the run** (Assault events per week 21 → 270; grudges formed 2,742; vendettas open 28 on day 120). Fights, raids, hunts and arrests are the expensive events, and the city does not reach a steady state in 120 days. Nobody has run 365 days.
- **Plan churn**: 177k `PlanAborted` in 120 days (1,475 a day), 46k of them `Earn` failing at `Scavenge: StockGone` (*correction 2026-10-08:* the 85 % miss of the `scavenge_p` 0.15 roll reported as an abort, not a race), 20k `Sleep` losing its bed at the step (`ReservationKind::Bed` exists and nothing writes it). Each abort is a re-plan.

The gates print one number: the mean over the run (7.8k here; the handoff says "6.9-7.1k on the box"). The floor is 4,000 and seed 42 reads 4,099 on day 120 with nothing of M16-M18 built. "Optimisation is a later pass" is right for constant factors (profiling, allocation, the A* cap); it is wrong for the budget rule, which is a design decision: what does a forced class cost, how many may there be, and does the Statistical tier need to carry gang members (an orders-aware table) so that the gang roster is not a Coarse-tier pin? That decision gets harder after M16, not easier.

**Recommend**: the gate's throughput number becomes the last-10-day mean, not the run mean (the doctrine keeps the 4,000 floor; it just stops hiding the slide); a profile of days 100-120 (`probe_system_timing`); a forced-class budget rule and held prisoners; one 365-day run per seed (three sim winters: a year is 120 days) to see what is unbounded. All before M16.

## 3. The economy is the dole

Money per day on day 120, seed 42 (seed 43 in parentheses):

| flow | coins/day |
| --- | --- |
| dole paid | 5,720 (5,704) |
| wages paid | 1,428 (1,306) |
| rent paid | 1,320 (1,267) |
| food bought | 4,816 (5,964) |
| tax | 690 (977) |
| contracts | 120 (80) |

The dole is four times the wage bill. 291 of ~1,950 adults have a job (the ten roles are Farmer, Guard, Clerk, Bartender, Gravedigger, Sanitation, Ripperdoc, Mechanic, Researcher, Reporter; sixteen building kinds, of which Home, Lot, Jail, Cemetery and Hideout employ nobody). Over 120 days the population's wallets go from 57.5k to 8.4k (10.0k) coins, the Treasury from 41k to 94k (88k), the wallet Gini from 0.31 to 0.71 (0.78) and the top decile's share from 0.28 to 0.65 (0.73). Of nine corps, three are dissolved on seed 42 and the survivors hold `Secure` or `Hunker`; two hold 16k each and four under 1k. Money has pooled in the Treasury and two corps, the brains have nothing to do but defend, and 2,000 people live on 4 coins each.

The vision asks for "a deeply unequal, tiered economy" and the city has one, but by accident, and the accident has been shaping decisions since M11:

- M13 cut every price to fit (motorcycle 60 with no upkeep, implant and deck T1 at 60); chrome finance is "thin" because nobody can borrow.
- Nobody installs a tier-2 or tier-3 implant in 105 days (god v5 gap 3), so the tech tree's top tiers have no customers and a wipe that drops a tier changes nothing anyone sees.
- The Data market dies with Zetatech (V17); corps go bankrupt and are not replaced because founding needs a purse.
- L1b had to restore pressure with food at 4 and scavenging at 0.15 because the dole in place made everyone comfortable: the only lever left was the price of the one good.
- **M16 cannot be measured on this economy.** Its acceptance reads "a Fixer's weekly income within 0.5-2x a Bar owner's", the `Contract` goal scores "price over wage", a Hit is "a clerk's friend takes her savings to the Fixer", protection is "weekly, on owner-run businesses". There are no savings, the wage is 5 coins a day, and there are ~20 owner-run businesses. Every M16 band would be set against a dole city and would have to be reset later.

The roadmap knows this. A goods milestone is named in addendum 2 ("likely its own milestone between M14 and M15 or folded into M13"), addendum 9 ("the goods milestone after M14: Power and Water"), addendum 15 (leisure businesses "add jobs to the dole city, spread money downwards") and the handoff ("revisit prices when employment and wages are addressed; the economy milestone's business"). It has no row in the roadmap table and no slot in the order. That is the biggest gap between the roadmap and the vision.

**Recommend**: give it the slot before M16. Not the whole goods vision (Power, Water, scrip, production chains are later), but enough that wages are the economy and the dole is the floor under it: addendum 15 in full (Club, Arcade, food stall, fight pit, gambling den, lounge; the `fun` need; street hang-outs), each a business with staff, which is where the jobs come from; a wage/dole rebalance so Σ wages > Σ dole in aggregate; one more production chain so corps have a `Grow` worth taking; and the M13 prices revisited. The brutality-through-pressure doctrine holds: rent, prices, jobs and gang pressure are what hurt, and a city with wages has more of all four to lean on than a city with a dole.

## 4. Off-screen residents cannot be murdered any more

Violent deaths, 120 days: 153 on seed 42 of which 11 off-screen; 155 on seed 43 of which 2. Roughly 350 bodies (Full plus Coarse) account for 93-99 % of the killings and ~1,600 Statistical adults for the rest; per resident that is a 50-100x difference between being watched and not. The handoff records it ("off-screen killings 3-6; the Statistical table learns its violence from the Full tier, and L1 removed its old sources"), and the M10 gate prints it as a FINDING with a band of ≥ 20.

It is not a calibration band. The table's rows are phase x lawfulness x hunger, so what it can learn is need-driven violence, and L1 removed exactly that (Hall-queue fights, witness-spam feuds, broken days). What the Full tier does now is faction and story violence: gang orders, raids, Hunts, vendettas, episodes, crossfire, the Jail's bump releases. The table has no column for any of it, so the Statistical tier, 80 % of the city, now lives a life of theft, starvation and arrest in which nobody is ever killed. Two vision promises fail on that: "rich lives on inspection" (an off-screen resident's biography cannot contain a murder; of the 333 holes the binder attributed on seed 42, 178 are robberies, 144 beatings and 11 killings), and the parity rule itself (the Full-vs-Statistical parity test compares a forced-Full city with a forced-Statistical one under the calibration config, which turns M15 off, so it measures the table's fidelity to need-driven violence and never sees the faction violence that is now most of it). It also feeds § 2: if off-screen residents must be promoted to be killed, every story needs a body.

**Recommend**: the Statistical tier gets its violence from the faction layer, not the hourly table: a gang's order, a vendetta, a Hunt and a riot roll holes (`Killed`, `Assaulted`, `Robbed`) against Statistical residents of the districts they touch, at rates read from what the Full tier does under the same order, bound by the existing binder. M15 W23 already does the hunter's side ("Statistical hunters"); this is the victim's side. The parity test then compares a city with the faction layer on, not off. Same milestone as § 2 and § 3.

## 5. M15 works, at saturation

The machinery is all there and every phase's identity check held (`--word-off` reproduces M14). The numbers on main:

| quantity | seed 42 / 43 | spec band | reading |
| --- | --- | --- | --- |
| grudges formed | 2,742 / — | 60-400 | 7x over: every `Lost` and `WasRobbed` forms one |
| `known_by` of a killer at 7 days | 17-436 (median of samples ~105) / 24-685 | 25-150 | mostly over the band |
| second-hand share | 0.59 / 0.57 | 0.30-0.70 | in band |
| distorted share | 0.013 / 0.013 | 0.03-0.15 | under |
| Hunts / Avenged / revenge kills | 50 / 5 / 2 and 58 / 12 / 5 | 10-60 / ≥ 3 / 3-20 | kills under on one seed; chain ≥ 2 on one seed |
| vendettas opened / open on day 120 | 54 / 28 and — / 25 | ≥ 1 | 14 factions, so about a third of all pairs are in blood at once |
| gang `dread`, `heat` on day 120 | 0.50-0.98, 0.83-1.00 (all four gangs) | 1-5 % of adults ≥ 0.5 | saturated: the brains read no difference between gangs |
| corp `honour` on day 120 | 0.50 x 6, 0.41, 0.00 x 2 | spread 0.15-0.6 | flat: 0 or 1 `ContractLost` on honour per run |
| stories / Planted / Buried | 829 / 115 / 29 and 789 / 75 / 10 | 3-12 a day | in band, but the Feeds run marriages and foundings, and three corps paid to plant the same story about the same man on the same day |
| skill rarity, Poached, Expelled | 0.058 / 0.047, 9 / 2, 29 / 20 | 3-8 %, ≥ 3, ≥ 1 | in band except Poached on 43 |

Under the gate doctrine none of these is a failure, and phase 5 is the right place to look at them. But three are not bands, they are signals with no dynamic range, which is a mechanism problem: an axis at 1.0 for every gang tells the law's pressure term and the rival's fear term nothing; a vendetta between a third of all faction pairs is the ambient state, not an event; honour that never moves cannot lose a contract. **Recommend** that phase 5's calibration loop (plan 5.3) is scoped to dynamic range (the axes spread, vendettas rare, grudges formed on killings and robberies rather than every beating) and not to band-hitting, which is consistent with "assert that the system works": a reader with signal is the mechanism.

The M14 band Dylan has not signed off on (six-seed mean Data sold ≥ 200 → a printed finding with "a sale on every seed" asserted): keep it as a finding. Seed 46's 608 was one gang sale, and the band dies with Zetatech, which is § 3's problem, not M14's.

## 6. M16-M18: right order, stale premises, one milestone too big

The dependency argument in the roadmap holds (reputation prices contracts; the outside reuses the board; the player adds no mechanics). Two notes:

- **The specs are five days old and read M13-M15 "as specified"**, and each of those deviated (the M15 plan alone has 49 decision rows). Examples visible already: M16 renames the roadmap's `Den` to `Fixer` and reads `Corp.contracts` as "guard contracts"; `Governance` exists but is `Dictator` only; `alertness_mult` is a stub returning 1.0; `Abduct` and `Abduction` were built in M13 with their own shape; `Sighting` is M14's struct carrying M15's relay. The M15 plan's device ("verify against HEAD after M1N phase K, adapt identifiers, never behaviour") worked and should be the first step of each plan.
- **M16 is two milestones.** Fourteen contract kinds, missions at three LODs with the raid machinery refactored, the wounded and bleed-out, Rescue and temperament, Trauma Team, captives and private prisons, blackmail and secrets, boards with votes, bounties, tags and scanners, fraud in three forms. The vision's two stories ("assassinations are popular", "quests are real") need the contract entity, the Fixer, Hit/Beat/Guard/Locate, the accessory rule and the board. **Recommend** M16a (those) and M16b (leverage, hostages, the wounded, boards, tags), with M17 free to follow M16a if the outside is wanted sooner.

M18 as specified ships a third-person debug app with a player agent and a template dialogue writer. The roadmap should say what is after it, because the vision's player experience is five or six milestones further: ambient LOD and density (addendum 13), interiors and verticality (10), destruction and wreckage (6), goods, Power and Water (9), the generated dialogue layer, daemons (11), cataclysms (12), first-person. None of that is wrong; it is unlisted, and the roadmap table ends at M18.

## 7. Smaller things worth knowing

- **The Jail is pinned at 160 again** by day 70 on both seeds (mean 105-115, max 160). 3,115 arrests and 2,986 sentences in 120 days saturate any capacity; the bump rule decides who serves. Private prisons (M16) and a sentence economy are the designed answers.
- **The Winter wave is still there**: 14,486 `Starving` events, mean hunger and mood both worst on day 111.
- **`Scavenge: StockGone`** aborts 46k plans; the shadow pass's "no fallback for the poor" became a fallback everyone races for.
- **Feeds report marriages and Ripperdoc openings** because the story score has little else of reach; the news needs the deeds § 5 is drowning in, sorted.
- **Config sprawl**: 51 sections and 1,410 lines of `config.toml`, 3.8k lines of `config.rs`, each milestone's knobs commented with the reason, nobody has swept them. Fine for now; the late calibration milestone's first job is a knob census (thirteen printed findings across the gates today).
- **`components.rs` 3.3k, `virt.rs` 3.1k, `assets.rs` 2.4k, `world.rs` 2.6k, `levers.rs` 2k lines.** Big files, not bad ones; the review passes keep finding real bugs (ten this week), which argues for keeping the per-milestone review and not for a refactor.

## 8. The recommended path

1. **M15 phase 5 and review**, as planned, with 5.3 scoped to dynamic range (§ 5). Small.
2. **A base-city milestone before M16** (call it M15.5 or Life L2 widened; it needs a spec like any milestone), with four parts: the economy of jobs (§ 3: addendum 15 in full, wages over dole, a second production chain, M13 prices revisited); Statistical violence from the faction layer (§ 4); throughput (§ 2: last-10-day gate number, a profile of days 100-120, a forced-class budget rule, the plan churn); a 365-day run on two seeds as a sanity gate. Re-shadow afterwards, as already decided. This replaces Life pass L2 rather than adding to it: L2's role features (leader, creed, household goals, Clinic customers, a runner archetype) ride on the jobs and businesses this milestone builds.
3. **M16a**, then **M16b**, each planned against HEAD (§ 6).
4. **M17, M18** as specified.
5. **The late calibration milestone**: a knob census, the printed findings judged together, prices and bands reset on the wage economy.
6. **Roadmap table extended past M18** with the addenda that have no milestone, so the distance to the vision's player experience is visible.

The only item on this list that changes direction is 2, and it changes it by one milestone. Everything the vision describes is reachable on the current architecture; what the review found is that the floor the next four milestones stand on has three cracks, and they are cheaper to fix now than under M16's weight.

## Appendix: how the numbers were made

```
cargo build --release -p citysim-cli
./target/release/citysim-cli.exe run --days 120 --seed 42 --report --events > run42.csv 2> events42.tsv
./target/release/citysim-cli.exe run --days 120 --seed 43 --report --events > run43.csv 2> events43.tsv
./target/release/citysim-cli.exe run --days 120 --seed 42 --report --word-off > run42_wordoff.csv
./target/release/citysim-cli.exe run --days 120 --seed 42 --report --virt-off > run42_virtoff.csv
./target/release/citysim-cli.exe run --days 120 --seed 42 --report --life-off > run42_lifeoff.csv
python tools/analyze_run.py run42.csv events42.tsv
```

Run from the repo root (the CLI reads `assets/config.toml`). Throughput was read with the box otherwise idle; the four seed-42 runs were sequential. Column sums are over the 120 daily rows; "day 120" is the last row.
