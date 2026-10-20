# The pre-view pass: shifts you can watch, encounters that need bodies

Status: **spec, 2026-10-20, questions answered** (seventeenth-session follow-up, with Dylan). Scope is SHADOW_V6 § Before the
camera, rows 1-7, plus the dead trades. The gate is a Shadow V7 at 20k with the V6 tool. Base: `fc8b6b3`.

## 0. Decisions

| # | decision | by |
|---|---|---|
| P1 | A visible shift is one of **five shapes**, chosen per trade in `[[trades]]` (`duty = ...`) and per built-in role in a `[shift.roles]` table. No per-trade bespoke code. | Dylan 2026-10-20 |
| P2 | A `stations` shift moves between **2-4 distinct tiles inside the workplace footprint**, in 30-90 min blocks, with one break off site. The view animates within a spot; the sim owns which spot. | Dylan 2026-10-20 |
| P3 | **An encounter posts when, and only when, both bodies are present**; otherwise it is narrated as off screen (`[off]` in the shadow tool), never as "served". | SHADOW_V6 § 3 |
| P4 | Shapes are built from existing actions (`GoTo`, `PatrolLeg`, `HaulToMarket`, `BartendWork`, the round's leg loop in `trades::round_plan`) wherever they can be. A new `ActionKind` is allowed only when an existing one would lie about what the body does. | SHADOW_V6 § 1 |
| P5 | Pay, output and the coin ledger do not move: a shape changes where the body is, not what the shift earns. A coin made or lost is a gate failure. | doctrine 1 |
| P6 | The 20k city is the judge; 2k numbers ride along. | CLAUDE.md 2026-10-19 |
| P7 | The `stations` break is **a meal if hungry, else a smoke**: a hungry worker buys and eats at the nearest open food venue; a fed one stands outside the door. The lunch trade moves food numbers for a known reason (FINDING, not a band). | Dylan 2026-10-20 |
| P8 | The sweeper is a **`round` over the district's street tiles, ending at the Recycler** (the post), replacing `TendGraves` at the Recycler; the street credit follows the stops walked, not the midnight ledger. | Dylan 2026-10-20 |
| P9 | Leaders get **the visible half now**: a `round` over the gang's businesses, a court block at the Hideout, orders to a lieutenant given in person (P3). Raid strategy, war cooling (OPEN-039) and inter-gang parleys stay in the pressure pass. | Dylan 2026-10-20 |
| P10 | The 20k legs: **diagnose first, then the El plus the cab**. The El already rides (`systems/transit.rs`, `transit::choose`); the island has 2 lines to the generator's 20. Phase 5 finds why 300-900 min legs walk, adds island El lines (their transit data or `[[transit.lines]]` rows), routes the trips the El misses to `fares`, and segment-replans what still walks. | Dylan 2026-10-20 |

## 1. The five shapes (OPEN-137)

Today `ActionKind::for_role` sends every `Role::Trade(_)` and most built-in staff roles (`Host`, `Cook`, `Ripperdoc`,
`Researcher`, `Reporter`, `Fixer`, `Volunteer`...) to one `ClerkWork` block of 530-540 min on one tile
(`goap/actions.rs:322`). `TradeDuty` (`config.rs:2707`) already has `Post | Round | Fare | Corner | Preach | Sell`.

| shape | `duty` | what a body does | built from | trades |
|---|---|---|---|---|
| **stations** | `post` (gains spots) | picks a spot from the footprint's `station_spots` (seeded per shift), works `station_block` min there, moves; one `break_min` break at mid-shift (P7) | `GoTo(tile)` + the work action in blocks | manager, engineer, accountant, broker, office worker, civic clerk, plant op, packer, teacher, forger, black doc, organ broker, tailor, barber, gunsmith, BD editor/actor, orderly, jailer, camp warden, night porter, dancer, companion, chip host; the built-in staff roles |
| **haul** | `haul` (new) | loads at the workplace's stock tile, carries `haul_load` to a destination (a Market, Warehouse, site or address), unloads, returns; repeats to the cut-off | `GoTo` + `HaulToMarket`'s carry; a vehicle when the trade has one | night stocker, loader, depot hand, builder, day labourer, driver, courier |
| **round** | `round` | the existing door-to-door loop; a stop **serves only who is there** (P3) | `round_plan` | super, vendor (pitch moves), tinker, street worker, sweeper (P8), gang leader (P9) |
| **fares** | `fare` | waits at a rank, takes a rider who asked for a cab, drives/pulls the leg, is paid on arrival | the M13 vehicle drive | cab driver, rickshaw |
| **counter** | `counter` (new) | stands the counter or door tile; serves arrivals **as they arrive**, not in a batch | `BartendWork`'s serve | bouncer, barkeep, madam, brothel worker, cook |

Config (all in `assets/config.toml`, documented like their neighbours): `[shift] station_block_min`,
`station_block_max`, `station_spots_max`, `break_min`, `haul_load`, `haul_cutoff_min`, `counter_settle_window`.
A trade row can override its block length.

**Statistical mirror.** None needed for the movement: a Statistical body is off camera by definition (`OPEN-149`)
and keeps the one-block shift with the same pay. What a shape *produces* (parcels handed over, fares carried, door
refusals, served customers) has a mirror: the shape's expected count per shift, drawn from a purpose-keyed stream,
the same way the Statistical super's stops work today (`trades::round_for`).

## 2. The bugs (rows 2-7)

| row | OPEN | the fix | anchor |
|---|---|---|---|
| idle freeze | 138 | a goal with no plan writes a cooldown line and backs off; the "could not plan" path never leaves a body goal-locked over a tick budget | `goap/`, `plan.rs` |
| settlement by presence | 119, 120, 145 | P3 everywhere: the super stop, the pimp's collect, venue customers per arrival, the day cook's counter (falls out of `counter`), the arms sale at the meeting not midnight, a Locate sighting needs an awake seller | `service.rs::super_stop`, `weapons.rs:699`, venue settle |
| 20k legs | 139 | P10: why the leg walks (`transit::choose`, the island's 2 lines); island El lines; a cab past the El; a GoTo past `leg_replan_tiles` re-plans in segments; escorts keep walking speed | `systems/transit.rs`, `exec/` GoTo |
| housing churn | 140 | one `Housed` writer per midnight; "moved nearer work" only for the employed and only if nearer; `move_in` sends the body home; "Hotel" resolves to a Hotel or fails loudly | `ages.rs:478`, `life.rs:959`, `life.rs:1359` |
| parcels | 141 | falls out of `haul` for the courier: a parcel is a load with an address; a round with zero parcels does not run | `streetwork::courier_stop` |
| implant loop | 142 | `BuyAsset` validates the sale before the 140-min wait; a failed precondition cools the pick | `exec/actions.rs:67`, `chrome.rs::buy_install` |

**The dead trades.** The cab (`OPEN-115`) is the `fares` shape plus the 20k legs creating demand. The bouncer
(`OPEN-121`) is the `counter` shape on the door tile, where `door_refusals` can finally fire. The leaders
(`OPEN-118`/`OPEN-128`, Dylan 2026-10-19: hold court and keep the delegation) get a `round` over the gang's
businesses plus a court block at the Hideout, and the lieutenant's orders are given in person when both are there (P3).

**`OPEN-094`.** Retire the earlier milestones' ten `CITYSIM_*_OFF` switches in phase 0. Audit each one: delete the
dead ones with their off branches, and keep the config paths.

## 3. Phases (one fresh Opus agent each, per the SOP)

| phase | content | device rows (the numbers that prove it) |
|---|---|---|
| 0 | `OPEN-094` switch retirement; the idle freeze (138); the implant loop (142) | switch count 10 → 0, byte-identical on 42 × 30; idle 12 h+ days; implant retry loops |
| 1 | housing churn (140) | double-`Housed` lines; execs who never sleep at home; `NoSuchPlace` hotels |
| 2 | the shape machinery + `stations` (P1, P2), with the built-in roles moved onto it | longest single work action per shift; distinct work tiles per shift; coin identity |
| 3 | `haul` + `counter` (courier 141, cook 145, bouncer 121, barkeep) | parcels handed; `door_refusals`; day-cook served |
| 4 | presence settlement (P3: 119, 120, the arms sale, Locate) + `round` presence (super, sweeper, street worker) | served-while-absent = 0; `[off]` narration share |
| 5 | 20k legs (139) + `fares` (cab 115, rickshaw) | GoTo legs > 120 min at 20k; cab trips driven by a body |
| 6 | leaders (118, 128) | raids/parleys/court in leader diaries |
| gate | Shadow V7 at 20k (seeds 42-44 × 30, the V6 tool, the seven critics), full measurement set, milestone tier | V6 § Before the camera rows 1-7 read as fixed or demoted with a reason |

Each phase has an off switch while it is in flight (doctrine 6), a failing test first (doctrine 9), and the 20k
seed 42 × 30 counters beside the 2k ones. Phases 0 and 1 are independent of 2-6 and can run in parallel worktrees.

## 4. Questions for Dylan

All four answered 2026-10-20 (P7-P10).
