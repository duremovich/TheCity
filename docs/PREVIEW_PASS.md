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
| P11 | The island's extra El lines: **import TheCityGraphics' own transit first** (the Tai Ning Road tram, any rail in its export) through the importer, **then fill** with `[[transit.lines]]` rows the phase 5 agent proposes between district cores; every authored line is listed in the phase report for Dylan's veto. | Dylan 2026-10-21 |
| P12 | Long escorts: **walk at walking speed up to a distance, past it the precinct van** (the transfer van as a vehicle trip with the guard and the suspect aboard). No escort covers ground faster than its mode allows. | Dylan 2026-10-21 |
| P13 | The leader's `round` **collects protection in person**: its stops replace the off-screen collection, and a stop with the owner out collects nothing that day (P3). The coin flows that move are printed FINDINGs at the gate. | Dylan 2026-10-21 |
| P14 | **Court at the Hideout** is a gathering: the lieutenants take orders there, and members passing through hand over their take in person. Petitioners (debtors, locals asking favours) wait for the pressure pass. | Dylan 2026-10-21 |
| P15 | A due order whose lieutenant is not at court **waits for the next court, up to a cap** (config, about a day). Past the cap the leader walks to the lieutenant and gives the order there. Raids may slip; they must not vanish. | Dylan 2026-10-21 |
| P16 | At a round stop, **whoever is on shift at the business pays the week from the till; the owner answers only when nobody is on shift.** Statistical staff on shift count as present (being on shift is the presence). | Dylan 2026-10-21 |

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

**`stations` as built (phase 2, `systems/shift.rs`, `[shift]`).** The shape is `shift::shape_of`: a `duty = "post"`
trade is worked as `[shift] trade_post` (`stations`), a built-in role by its `[shift.roles]` key (all twelve
`stations`; absent is `post`, the one block). `shift::plan` is the Work goal's bypass after the commute, the round,
the beat and the fare: a shift is three shape plans, the first half's blocks to mid-shift, the break, the second
half's blocks, each block a `GoTo(Station)` (a new `LocationKey`: the step's `tile` inside its `target`, a walk across
the room of at most `station_move_max` minutes, the body kept inside) and the role's own work action. The spots
(2-`station_spots_max` walkable interior tiles; under 2, the one block) and each block's length are keyed draws
(`SHIFT_KEY`, the body, the shift key, the ordinal). Between the shape plans the shift holds against non-emergency
goals as its one block did (`fixes::committed`). The break is a new `ActionKind::Break` outside the door (P4: `Rest`
would lie; it also ends at once off the premises), or, hungry, the packed meal outside the door, else the nearest
open Market (`BuyFood`, eaten) or NoodleBar (`EatOut`) within `break_walk_max` (at 2k the Markets are 3 and the
nearest was 104 min away, so the NoodleBar counts as a food venue). Three deviations from P7, each for a measured
reason: the break starts up to `break_stagger` minutes either side of mid-shift, drawn per body per shift (a night
crew all stepped out at 01:30 together); it waits a block while a customer is mid-procedure in the workplace (an
implant on the table); and a Clinic's last doctor on shift inside keeps it through the break (a Clinic sells only
with its doctor in: taken alone, 2k `AssetBought` fell 14 % and Ripperdocs and Orderlies employed fell). A block before the last closes nothing; the last
runs to the shift's end and closes it at the post (the wage, `last_shift_day`), a producing trade's output accrued
over the span from the first block, a pro-rata cut read over the same span. A shape plan's steps check their own start
(`actions::can_start`): the planner reads a worker inside his own Market as at `Workplace`. **Statistical mirror:**
none: a Statistical body is off camera (OPEN-149) and has no plan; it keeps the one block and the same pay.

**`haul` and `counter` as built (phase 3, `systems/shift.rs`, `[shift.haul]`, `[shift.counter]`; OPEN-121, 141,
145).** Deviation from P1's `duty = "haul"`: a trade row names its shape in a new `shape` key and keeps its `duty`
(the courier's and builder's `round` and the day labourer's `corner` carry their hiring and their Statistical
path; the off switch clears `shape`, so the off save is main's byte for byte). `haul`: night stocker, loader,
driver, depot hand, builder (its round of open sites becomes loads carried to them: truer than a door-touch,
and nothing builds from the round), courier, day labourer. A haul shift is one shape plan per trip: to the post
if out, `GoTo(Station)` to the stock tile (the interior tile nearest the door), the role's work action for
`haul_handle_min` (the load; a block, no wage), the carry (`GoTo(TargetHome)` to the destination, or
`GoTo(Station)` to a drop tile inside the post when it has none), and a new `ActionKind::Unload` there (P4: a work
block would lie, the body is not at its post) for `haul_handle_min`; the next call walks back. Destinations
(`haul_dests`): a Construction Yard's open sites, a Freight Yard's, Farm's or Warehouse's nearest staffed
workplaces (not Homes) within `haul_reach` tiles, `haul_dests_max` of them (deviation: Markets and Warehouses
only gave the 2k Yard none, its nearest Market is 56 tiles off); a Market's night stocker hauls inside. No trip
starts that would end (its estimate a quarter over the doors' distance) within `haul_cutoff_min` of the shift's
end: the rest is one block at the stock tile to the end, which closes and pays the shift as before (P5). The P7
break applies at mid-shift. The `drive` trades claim the post's fleet truck when one is parked there
(`vehicles::claim_haul`); otherwise the GoTo takes whatever vehicle it finds. **The courier (OPEN-141).** Root
cause: the Yard's Statistical couriers walk their rounds world-side at the shift's first hour (`lod::stat_work`),
and the rank rotation of `new_round` wraps over the Yard's few distinct stops, so they handed over every parcel
at 09:00 before a body's first leg arrived (36 of 36 Full rounds, 0 handed). Now each address goes to one courier
(`courier_addresses`, a hash of the address over the Yard's couriers); a parcel is a load carried to it and
handed over at the `Unload` only to the addressee or a staff member present (P3, `courier_stop`), else it stays
(`parcels_missed`), is tried once more, and goes back at the shift's end; a courier with none left works the
Yard's floor (a round with no parcel does not run). A parcel left from an earlier day goes back at midnight (it
lay pending for good). **`counter`**: bouncer (the door tile: the interior tile nearest the door), barkeep, madam,
brothel worker (the tile farthest from the door), and the built-in Cook and Bartender (`[shift.counter] roles`).
It is the stations plan with one spot and each half one block; the break only while another counter body
covers the post. **Served as they arrive (OPEN-145):** a body's visit was already settled per arrival
(`service::arrive`); the 21:00 batch was the Statistical evening, served by whoever was on at 21:00, so a day cook
served no off-screen customer. A Statistical visit to a venue with a counter body (a Full or Coarse counter
staffer) is now drawn (purpose-keyed: the agent, the venue, the day, the ordinal) into a `counter_settle_window`
slot of the next 24 hours inside the bodies' shifts (`World.counter_due`, saved) and paid and served then; a
venue with no counter body keeps the 21:00 evening (the off-screen economy as before). **The door (OPEN-121):**
the patron's pick read the bouncer's verdict (`door_ok`'s Club heat rule) from across the city, so nobody ever
reached a door that would refuse him. With `[shift.counter]` on the pick reads `door_seen` (the Club's door open
unless it turned him away within `[exec] refused_ticks`), and the verdict is made at the door (`door_ok` at the
venue step's start: `service::refused`, which now sets `Brain.refused`). **Statistical mirror:** the movement none
(off camera, no plan; one block, same pay). Parcels: the Statistical courier hands over his own addresses at his
shift's first hour on the same presence rule. Door refusals: the off-screen visit meets the same door at
`stat_visit` (picked by `door_seen`, refused by `door_ok`). Served customers: the drawn slot above, by
construction the same `arrive`. Haul loads have no output to mirror (P5).

## 2. The bugs (rows 2-7)

| row | OPEN | the fix | anchor |
|---|---|---|---|
| idle freeze | 138 | a goal with no plan writes a cooldown line and backs off; the "could not plan" path never leaves a body goal-locked over a tick budget | `goap/`, `plan.rs` |
| settlement by presence | 119, 120, 145 | P3 everywhere: the super stop, the pimp's collect, venue customers per arrival, the day cook's counter (falls out of `counter`), the arms sale at the meeting not midnight, a Locate sighting needs an awake seller | `service.rs::super_stop`, `weapons.rs:699`, venue settle |
| 20k legs | 139 | P10: why the leg walks (`transit::choose`, the island's 2 lines); island El lines; a cab past the El; a GoTo past `leg_replan_tiles` re-plans in segments; escorts keep walking speed | `systems/transit.rs`, `exec/` GoTo |
| housing churn | 140 | one `Housed` writer per midnight; "moved nearer work" only for the employed and only if nearer; `move_in` sends the body home; "Hotel" resolves to a Hotel or fails loudly | `ages.rs:478`, `life.rs:959`, `life.rs:1359` |
| parcels | 141 | falls out of `haul` for the courier: a parcel is a load with an address; a round with zero parcels does not run | `streetwork::courier_stop` |
| implant loop | 142 | `BuyAsset` validates the sale before the 140-min wait; a failed precondition cools the pick | `exec/actions.rs:67`, `chrome.rs::buy_install` |

**Housing churn as built (phase 1, `[life] housing_fix`).** The first `Housed` line of a midnight wins: the kin
take-in, the retirement to kin, the Spire (`house_exec`) and the "nearer work" relocation skip a body (or a spouse
who would move along) already housed at that tick (`World.housed_now`, not saved); the kin take-in skips corp execs,
and the relocation skips an elder under a grown child's roof. The relocation moves a worker only when his own commute
shrinks by the margin, and a spouse who comes along "moved with" him. A body moved into a Home it is not standing in
is `World.moving_in` until it walks in: `idle_near` sends it home and `away_hotel` gives it no Hotel. A housed
sleeper's `GoTo(Hotel)` resolves to `life::away_hotel` (the planner's Hotel), never a Mission cot (the homeless's
bed, by design E27). **Statistical mirror:** none needed: the midnight passes are tier-agnostic, and `moving_in` and
the Hotel resolution steer Full and Coarse plans only (a Statistical body is off camera and has no plan).

**Settlement by presence as built (phase 4, `systems/presence.rs`, `[presence]`; OPEN-119, OPEN-120, OPEN-154,
P8).** One rule (P3): a Full or Coarse party is present at a place when its body stands in it awake
(`presence::body_at`, not in a `Sleep`); a Statistical party is drawn present by a purpose-keyed draw consistent
with its schedule (`presence::stat_home`: not on its attended shift, not 00:00-06:00, then `stat_home_p`). An
encounter whose counterpart is absent posts nothing in either diary and is counted `presence_missed`; the shadow
tool's camera narrates an encounter with an off-screen party, the pick awake, as `[off]` (a body elsewhere or the
pick asleep stays `SETTLED APART`, the bug). Per item, with its Statistical mirror:
- **The super's stop** (`service::super_stop`): serves only the tenants met at home (a body in the Block awake, a
  Statistical one on the draw); the rest wait for the next round. Mirror: the Statistical super's first-hour stops
  (`lod::stat_work`) meet tenants on the same rule, so a Statistical pair meets on the draw.
- **The round's reach (OPEN-154)**: a round's stops (the super's, the vendor's, the tinker's; `trades::new_round`)
  lie within `round_reach_tiles` of the post's door, so a near-home round can no longer span the district. Mirror:
  `round_for` is the same list, the Statistical super is credited it.
- **The pimp's collect** (`vice::pimp_collect`): needs the worker at his pitch (`streetwork::working`: a body
  within `pitch_reach` of the spot, a Statistical one on the hour's `p_pitch` draw); else no collection, the next
  hour's pitch is tried. **StreetCut at the meeting**: with a body on either side the pimp's cut is taken at each
  collection, `pimp_cut` of the take since the last (`Pitch.cut_paid`), worker to pimp, `pimp_pass_up` of it to the
  gang (`cut_at_meet`); the day's close takes none (the take nobody collected stays with the worker: a timing and
  a total shift, P5's identity holds). Mirror: a Statistical-only pair keeps the cut at the day's close
  (`vice::street_cut_at_close`). **The street worker's client** (`vice::street_payer`): the drawn payer is an
  off-screen resident only; a body pays only in person (`client_near`).
- **The sweeper (P8)**: a body sweeper's shift is a `round` (`jobs::sweep_plan`, before the commute in the Work
  bypass): `sweep_stops` street stops of his beat, dirtiest first (ties by a keyed hash; the seeded set without
  litter), spaced past two `sweep_radius`, within `round_reach_tiles` of the first, walked nearest-first; at each a
  `Sweep` of `sweep_stop_min` cleans the tiles within `sweep_radius` (`sweep_per_hour` x the minutes:
  `sweep_stops`); then the walk to the Recycler (the post) and a `Sweep` there to the shift's end, which closes and
  pays it. The street credit follows the stops; the midnight ledger skips the body sweeper (a shift swept in the
  yard credits nothing). Deviations, each measured: (1) the beat is used only when the round and the walks to it
  and back fit the shift's time left from where he stands (`round_ticks`: the Manhattan tiles x
  `sweep_walk_factor` 1.5 plus the stops); else the stops are the Recycler's own district's within
  `round_reach_tiles` of its door, trimmed to fit (2k seed 42: a beat 185 tiles from the Recycler, a 370-minute
  walk back, closed no shift and paid no wage); (2) a shift begun past a shift's walk from the Recycler is the old
  stand on the beat, closed and paid there (`sweep_stands`; OPEN-139's legs); (3) the round's steps check their own
  start (the planner reads the Recycler as no `Beat`). The relocation's work tile is the Recycler's door
  (`life::work_tile`), not the day's beat. Mirror: the Statistical sweeper keeps the midnight ledger credit
  (`districts::sanitation`); it has no plan.
- **The arms sale** (`weapons::book_meeting`, `arms_meetings`): a Statistical buyer's weapon from a Bar whose dealer
  is a body is booked at midnight (`World.arms_due`, saved) and sold at the first hour the dealer's body is in the
  Bar (`buy` with the dealer present: the coins, the hand-over, `ArmsDealing` raised at his tile, the witnesses the
  bodies there), lapsing after `arms_wait_hours` (`arms_missed`). Mirror: a Statistical dealer sells at midnight as
  before (`arms_midnight`); a Gunsmith's licensed sale is not an encounter with a person and stays.
- **The Locate sighting** (`contracts::on_sighting`): an observer asleep sells none (`sight_asleep`). Mirror: none
  needed (a body's sighting only; the off-screen tracker's report is the contract's own resolution).
- **The papers and the Fixer owner's cut** (`gray::papers_daily`, `service::walk_in_sale`, `collected`): a forger's
  papers ordered at midnight are paid at the handover in the Fixer's open hours (`papers_handover`; a buyer short
  then is dropped), the forger's share to him, the owner's `fixer_cut` to the member of staff serving, who holds it
  (`World.fixer_owed`, saved) until the owner collects in person at the Fixer (`collected`) and hands it over then.
  A body buyer is handed his papers only inside the Fixer, and a brokered post or take (`service::contract_party`)
  meets a server on duty with a body party inside; the Fixer's book takes the credit off hours only between two
  Statistical parties (the 2k camera read a forger asleep at 00:00 "serving" clients across the city). Mirror: a Statistical buyer or party is served at the
  hour as before; the clerk's bribe (no Fixer) stays at midnight. Left open: a runner's `FixerCut` on a data sale (`contracts::on_sell_data`) still pays the owner at the
  sale (`OPEN-155`).

**The 20k legs as built (phase 5, `exec/legs.rs`, `[legs]`; OPEN-139, OPEN-115, OPEN-140's remainder, OPEN-156; P10-P12).**
The diagnosis (20k seed 42 x 30 on `isle_m_k2`, `run --legs`, `exec::leglog`): 62,364 Full or Coarse GoTo legs of
120 min or more (18,786 of 300 or more); 74 % walked, 26 % rode the El (its access walks long), a few hundred drove or
were driven. Of the long walks the El refused 49 % as `Slow` (the ride and its access walks save too little for the
fare) and 48 % as `NoRoute` (the board and alight shortlists on lines that never meet: the Terrace Line and the tram
share no tile, and the access walk to the nearest stop was a median 100 tiles); the fare 3 %. Two lines on a 1,500 x
938 island, both along the harbour, and nothing up the hill, where the Mid-Levels and the Spire make 70 % of the long
legs' ends. The fix by cause, each with its Statistical mirror:
- **Segments** (P10): a Full or Coarse walking `GoTo` past `leg_replan_tiles` walks to a segment end first, filed in
  `World::segments` against the plan step. A Full body's end is the tile `leg_replan_tiles` steps along its own route
  (the door's flow field, the big map's A*, a street target's A*), so it walks the streets the one leg walked (fix
  round: the straight line's Road snap took a guard's patrol leg off the street its beat passed, the thief it would
  have sighted was never seen, `law::test_theft_witnessed…` failed). A Coarse body's end (no route: it is timed) is the
  Road tile, else open hill or field, within `segment_snap` of the point that far along the straight line, nearer the
  target by half a segment. There the step re-starts from where the body
  stands (`exec::start_step` again: the cab, the El and the next segment re-checked). A Coarse body is seen a segment at
  a time instead of frozen at its door for the whole leg; a cab driver's leg drives in segments at the cab's speed
  (`fare_segment`). Mirror: none (a Statistical body has no movement; `OPEN-149`).
- **The island's lines** (P11): the importer reads the export's funicular (`elevated.json`, its two ends the stops;
  the escalator stays unmapped: a walkway at a walk's pace on a flat ground) and the authored lines of `--lines
  assets/maps/isle_m_k2_lines.txt` (the Mid-Levels Line up from the tram over the Terrace Line to the Spire, the
  Hillside Line along the Mid-Levels joining the Funicular, the Mid-Levels Line and the Vats Line, and the Vats Line from
  the tram's east end to the Farms), all written `L+` (in flight: dropped with the switch, so the off island is the
  base's two lines). The two harbour lines now meet through the hill lines. Mirror: the El is one function for every
  tier (`transit::commute_ticks`, `stat_commute`).
- **The exec's ride home** (OPEN-140): `streetwork::exec_ride` drives the exec home as it drives him in (the corp's car
  or the Garage's off-screen cab on the corp's purse), once a day (`exec_home_rolled`), from outside the Home's
  district. **The bed by ride**: the away bed (`life::away_hotel`) reads the trip home as `life::home_reach`, the walk
  or the ride where shorter (the El's `commute_ticks`, the exec's car: `ride_home_ticks`), so a guard whose patrol ends
  150 tiles out on an El line sleeps at home. Mirror: the rule itself (a Statistical exec's office trip is the same
  timed GoTo; the midnight passes do not read beds).
- **Escorts** (P12): an escort walks at walking speed up to `escort_walk_tiles` (a Full guard on foot with the suspect
  at his side, a Coarse one timed at the walk); past it the van: `van_call_ticks` for it to come, then the drive with
  both aboard at the Car's speed. The old cap (`[life] escort_van_ticks`, 60) let a 200-tile escort cover the ground in
  72 min. Mirror: none (an arrest is a body's; a Statistical guard's arrests are the law's off-screen roll).
- **The `fares` shape** (OPEN-115): a commuter's booked cab with a driver's body free within `fare_wait_ticks` (the
  old `cab_wait_ticks` 30 is ~34 tiles at the cab's speed) is a body ride: the rider waits at his door
  (`Jobs2State.passengers`, the exec skips him), the driver drives to him (the pick-up's `Fare` encounter there, both
  bodies present, P3), carries him at his side (`streetwork::carry`) to the destination, and the fare is paid on
  arrival (`pay_ride`, rider to the Garage's owner as before). A driver who cannot finish (his shift over, jailed, gone
  off screen, midnight) leaves the rider to the Garage's off-screen cab, timed from where he stands and paid then. A
  commute of `cab_min_tiles` or more that the El does not shorten takes the cab without the `p_cab` roll when the purse
  holds the fare and the reserve (`fares_el_missed`), both tiers (`cab_roll` is the one source). Mirror: the Statistical
  rider pays at his shift's first hour and the ride is booked to a driver's body as before (`stat_cab`); with no body
  free, the Garage's off-screen cab serves both tiers (`fares_unmanned`, `fares_no_body`).
- **The sweeper** (OPEN-156): the walk back to the Recycler in `jobs::round_ticks` reads the ride (`sweep_by_ride`).
- **FINDING (fix round, 2k 42-44 x 120 against 97a71dc).** With the straight-line segments the 2k city lost 1.5 pt of
  employment (J24 0.779 -> 0.764, all three seeds) and 4 % of arrests; with the route-following Full segments J24 is
  0.779 -> 0.779 and arrests 2,595 -> 2,626. Murders read 105 -> 134 (seed 42 68 -> 132): no single feature off brings
  them back (each one off, the rest on: 110-156; segments off 110), and the counter's own spread across the three base
  seeds is 68-161, so it is printed here for the gate's 42-47 set, not tuned.

**The dead trades.** The cab (`OPEN-115`) is the `fares` shape plus the 20k legs creating demand. The bouncer
(`OPEN-121`) is the `counter` shape on the door tile, where `door_refusals` can finally fire. The leaders
(`OPEN-118`/`OPEN-128`, Dylan 2026-10-19: hold court and keep the delegation) get a `round` over the gang's
businesses plus a court block at the Hideout, and the lieutenant's orders are given in person when both are there (P3).

**The leaders as built (phase 6, `systems/leaders.rs`, `[leaders]`; OPEN-118, OPEN-128; P9, P13-P16).** For a
leader with a body (Full or Coarse, free); off with `CITYSIM_LEADERS_OFF=1`. **The round** (P13): from `round_hour`
(not begun from `round_last_hour`) the Lead plan walks up to `round_stops_max` of the gang's businesses, nearest-first:
the protected businesses owing a week (the gang's standing `Protection` coercions due) first, then the gang's fronts.
At each `Collect` (`round_stop`) every owed week there is collected in person (`leverage::collect_in_person`, the same
`pay_week`: the coins and the refusal as at midnight) from **whoever is on shift there** (P16: a body inside and awake,
or a Statistical worker, present without a draw; the coin is `pay_week`'s, the owner's purse, so the ledger holds) and
only when nobody is on shift from whoever answers for it (`c.over`: the owner, a corp's exec, a rival's leader) when he
is inside and awake, or, off screen, on a purpose-keyed draw (`owner_stat_p`); nobody there, nothing that day and the
week stays owed (`lead_owner_out`). Before P16 the owner alone answered: 127 of 160 visits found him out (OPEN-160). The midnight `collect` no longer pays a body leader's gang (it
still ends a hold that is gone), and a new protection's first week waits for the round. **Court** (P14): the leader
sets out by his trip's estimate (at most `court_walk_max`) to be at the Hideout by `court_hour` and holds court there
(a new `ActionKind::Court`, P4: a Collect would lie) to `court_min`. A member at the Hideout during court holding take
(`loot_today`) hands it over in person (`split_loot`, `TakeHanded`); petitioners wait for the pressure pass.
**Orders in person** (P15): with a body leader and a free lieutenant, the faction brain's choice is held
(`Gang.pending_order`, `leaders::hold`; the members keep the old order) and put in force when given
(`faction::apply_order`: the muster timed from then, `OrderChanged` noting how): at court, to a lieutenant there (a
body inside, an off-screen one drawn at `court_stat_p`; while an order waits a body lieutenant is sent for to court,
`attend_due`); past `order_wait_ticks` the leader walks to the nearest lieutenant (`GoTo(Victim)`, an off-screen one at
his Home on the presence draw) and gives it on arriving beside him (`order_reach`, awake); past `order_word_ticks`
more it goes by word (`orders_word`). Deviations, each measured: the round window ends at 18:00, not mid-afternoon
(most leaders hold a day job: 2 rounds in 30 days with 10-16); court sets out by the trip (a leader 115 min out
reached it at 21:08, shut); the word cap is a day past the wait, not half (720 put it at night with the lieutenants
asleep: 7 of 13 orders by word on 2k seed 42 x 30). A strike pin or a contract's `Job` drops a waiting order.
**Statistical mirror:** a Statistical leader keeps the off-screen path unchanged (the protection at midnight, the
order in force when chosen): he is off camera (OPEN-149) with no body to walk or hold court, and a draw would add a
stream for a scene nobody sees. A gang whose leader turns Statistical, is jailed, or loses its last lieutenant has its
waiting order put in force at once (`leaders::run`).
- **FINDING (2k 42-44 x 120 against 4ce9a88).** Protection weeks paid 36 -> 11 a seed (coins 1,440 -> 440): the
  owner was out at 42 of 55 stops (P13 as decided: no stop collects from an absent owner; the owed weeks stand,
  OPEN-160); gang income -17 %. Raids 17 -> 17 (they do not fall); `OrderChanged` 79 -> 61 (an order that waited was
  rechosen before the giving), `Called` 43 -> 17 (the court is the Call). Orders given a seed: 26 at court, 18 walked,
  10 by word (OPEN-161). Courts 193, takes handed at court 12. In the lives window leaders' and members' worked shifts
  45 -> 33 and 271 -> 218 (the round, court and the summons take their hours; J24 0.776 -> 0.778). Murders 114 -> 118.
  At 20k seed 42 x 30 (switch off -> on): no protection is made in either run, so no round has a stop; courts 42,
  orders 9 at court, 11 walked, 3 by word; raids 7 -> 8; Murders 345 -> 233 (seed 43: 112 -> 172, a butterfly).

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
| 6 | leaders (118, 128; P9, P13-P15) | rounds, court and orders given in person in leader diaries; raids per 30 days against the base (they must not fall to 0) |
| gate | Shadow V7 at 20k (seeds 42-44 × 30, the V6 tool, the seven critics), full measurement set, milestone tier | V6 § Before the camera rows 1-7 read as fixed or demoted with a reason |

Each phase has an off switch while it is in flight (doctrine 6), a failing test first (doctrine 9), and the 20k
seed 42 × 30 counters beside the 2k ones. Phases 0 and 1 are independent of 2-6 and can run in parallel worktrees.

## 4. Questions for Dylan

All answered: P7-P10 on 2026-10-20, P11-P15 on 2026-10-21.
