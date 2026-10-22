# The island pass (spec, 2026-10-22)

Shadow V7 (`docs/SHADOW_V7.md`) read the pre-view pass as done on the generated city and failed on the island,
the city the camera will show. This pass makes the island read before the view milestone. Its gate is Shadow V8 on
the island (`isle_m_k2`, 20k, seeds 42-44), with V7's tool, windows and seven critics. The pre-view pass closes
first, as "done on gen", with the island failures written as OPEN rows.

## Decisions

| # | decision | by |
|---|---|---|
| I1 | **Distance counts in every destination pick.** The job, the bed (the Hideout bed included), the hangout spot and the Hotel are each scored by travel time against what they offer. Each purpose has a hard cap in config, past which a candidate is dropped unless none is left. **The exception is the hangout spot**: past its cap there is no spot, and the person stays in, because an hour of free time is not worth a 4-hour walk (phase 2; street life near home is `OPEN-109`'s feature, Dylan 2026-10-22). | Dylan 2026-10-22 |
| I2 | **A far job is a pressure that resolves over weeks.** A commute past a threshold pushes the worker to move nearer work, which is the existing "moved nearer work" path, made to work. If no housing is free, they look for a nearer job. | Dylan 2026-10-22 |
| I3 | **A vehicle in motion is an entity with its own position, and riders are aboard it.** This covers the car, the cab, the van (escorts, prisoner transfers), the exec's flyer and the El as trains running the line. The vehicle advances along its route each tick at its speed, and a rider's position is the vehicle's. No body is parked on a boarding tile, and none jumps on landing. The view draws the vehicle with its occupants. Later combat works on this model: chases, a vehicle destroyed with everyone aboard killed, a carjacking that pulls the driver out. That is design for later, not built here. | Dylan 2026-10-22 |
| I4 | **The dead trades split into bugs and features.** The bugs land in this pass: the courier and builder `Unload` at their own post, island foremen with 0 hires and no day labourer, the arms dealer no one is appointed to (`weapons.rs` `bar_arms_dealer` exists), and the cab with no body driver at 20k. The bouncer who never refuses, the joytoy with no client and a scav harvest seen from the scav's side are features for the pressure pass. | Dylan 2026-10-22 |
| I5 | **Island hunger is re-read at V8 before it gets a fix.** The likely cause is commutes eating mealtimes; a clinic path for malnutrition waits until the distance phases show what is left. | Dylan 2026-10-22 |
| I6 | **Company housing.** The island's 4-hour tail is the layout, not the pick: its 120 Farms hold 3,473 Farmer posts with 2,525 adults living within 60 tiles, and a perfect rematch of the same workers to the same posts floors at about 30 % of commutes at 4 h or more (phase 1). Farms and industry put up company housing (bunkhouses, company blocks) for their workers, the rent docked from the wage; it gives I2 the beds near work that it lacks, and works on any map. **In the workplace, then bought out** (the city is dense; free lots are rare): the seeded beds live inside the workplace building (dorm floors on a vertical farm, coffin racks in a plant's basement), with no new footprint; a corp whose staff still commute far buys a nearby apartment block and converts it, its tenants given notice and pushed out over the grace period. Building on a free lot is only the fallback when nothing near is for sale. **A worker who loses or quits the job is evicted after a grace period** (config): the company town's leverage. | Dylan 2026-10-22 |

## Phases

One mechanism per phase, one fresh coder each, sized to ~300 turns (the pre-view pass's phases 2-5 ran 395-507).
Distance comes before vehicles: with 4-hour commutes every ride is absurd, so vehicle numbers read on broken trips mean nothing.

| # | phase | OPEN |
|---|---|---|
| 0 | Retire the pre-view pass's eight switches and `assets/stat_table_base.toml`; bring the change tier back under its 4m budget | 174, 175 |
| 1 | The distance machinery: a travel-time cost term in the pick score and a hard cap per purpose in config (dropped past the cap unless none is left); applied to the job board/foreman and `plan.rs` `bed_of` (the Hideout bed included) | 163 (part) |
| 2 | Distance in the hangout spot and the Hotel pick; the Hideout/Home sleep ping-pong; "Hotel" resolving to a Soup Kitchen or a Church | 163, 171 |
| 3a | I6, seeded (merged, 81fac09): company beds beside Farms and industry at world build, filled at seed; the rent docked from the wage; eviction after `grace_days` on losing the job. 52 of 56 blocks fell back to open-ground plots | 163 |
| 3b | I6, in-building and bought out: the seeded beds move inside the workplace (storeys, basements; the plots retire); a corp whose staff commute far buys a nearby apartment block and converts it, the tenants pushed out over the grace period; a free lot only as the fallback | 163 |
| 3c | A housing term in mood (`OPEN-177`): tier, crowding (the coffin rack worst) and a company-owned bed, a `mood_bias` like `litter_bias` | 177 |
| 4 | I2: a commute past a threshold pushes the "moved nearer work" path, then a nearer job when no housing is free; the segmented leg that ends in the wrong district (`exec/legs.rs` `segment_end`) | 164 |
| 5 | I3 core: a vehicle in motion is an entity advancing along its route, riders aboard it (no parked body, no landing jump); the car and the cab (`systems/vehicles.rs`) | 165 (part) |
| 6 | I3: the El as trains running the line (`systems/transit.rs`) | 165 |
| 7 | I3: the van: the escort and the prisoner transfer replacing the 04:00 jump (`stations.rs:347`), and the arrest hand-off | 166 |
| 8 | I4's bugs: the courier/builder `Unload`, island foremen hires and the day labourer, the arms dealer appointment, the cab's body driver at 20k | 168 (part) |
| gate | Shadow V8 on the island (`isle_m_k2`, 20k, seeds 42-44) against `target/shadow_v7/`, V7's tool, windows and critics | — |

**Re-read at the gate, not fixed here unless V8 says so:** island hunger at d90 (`OPEN-167`, I5); the remaining
one-block shifts (`OPEN-169`); the presence leaks (`OPEN-170`).

**To the pressure pass (I4):** the bouncer's refusals, the joytoy's clients, the scav's harvest from the scav's side.
