# The island pass (spec, 2026-10-22)

Shadow V7 (`docs/SHADOW_V7.md`) read the pre-view pass as done on the generated city and failed on the island,
the city the camera will show. This pass makes the island read before the view milestone. Its gate is Shadow V8 on
the island (`isle_m_k2`, 20k, seeds 42-44), with V7's tool, windows and seven critics. The pre-view pass closes
first, as "done on gen", with the island failures written as OPEN rows.

## Decisions

| # | decision | by |
|---|---|---|
| I1 | **Distance counts in every destination pick.** The job, the bed (the Hideout bed included), the hangout spot and the Hotel are each scored by travel time against what they offer. Each purpose has a hard cap in config, past which a candidate is dropped unless none is left. | Dylan 2026-10-22 |
| I2 | **A far job is a pressure that resolves over weeks.** A commute past a threshold pushes the worker to move nearer work, which is the existing "moved nearer work" path, made to work. If no housing is free, they look for a nearer job. | Dylan 2026-10-22 |
| I3 | **A vehicle in motion is an entity with its own position, and riders are aboard it.** This covers the car, the cab, the van (escorts, prisoner transfers), the exec's flyer and the El as trains running the line. The vehicle advances along its route each tick at its speed, and a rider's position is the vehicle's. No body is parked on a boarding tile, and none jumps on landing. The view draws the vehicle with its occupants. Later combat works on this model: chases, a vehicle destroyed with everyone aboard killed, a carjacking that pulls the driver out. That is design for later, not built here. | Dylan 2026-10-22 |
| I4 | **The dead trades split into bugs and features.** The bugs land in this pass: the courier and builder `Unload` at their own post, island foremen with 0 hires and no day labourer, the arms dealer no one is appointed to (`weapons.rs` `bar_arms_dealer` exists), and the cab with no body driver at 20k. The bouncer who never refuses, the joytoy with no client and a scav harvest seen from the scav's side are features for the pressure pass. | Dylan 2026-10-22 |
| I5 | **Island hunger is re-read at V8 before it gets a fix.** The likely cause is commutes eating mealtimes; a clinic path for malnutrition waits until the distance phases show what is left. | Dylan 2026-10-22 |

## Phases

One mechanism per phase, one fresh coder each, sized to ~300 turns (the pre-view pass's phases 2-5 ran 395-507).
Distance comes before vehicles: with 4-hour commutes every ride is absurd, so vehicle numbers read on broken trips mean nothing.

| # | phase | OPEN |
|---|---|---|
| 0 | Retire the pre-view pass's eight switches and `assets/stat_table_base.toml`; bring the change tier back under its 4m budget | 174, 175 |
| 1 | The distance machinery: a travel-time cost term in the pick score and a hard cap per purpose in config (dropped past the cap unless none is left); applied to the job board/foreman and `plan.rs` `bed_of` (the Hideout bed included) | 163 (part) |
| 2 | Distance in the hangout spot and the Hotel pick; the Hideout/Home sleep ping-pong; "Hotel" resolving to a Soup Kitchen or a Church | 163, 171 |
| 3 | I2: a commute past a threshold pushes the "moved nearer work" path, then a nearer job when no housing is free; the segmented leg that ends in the wrong district (`exec/legs.rs` `segment_end`) | 164 |
| 4 | I3 core: a vehicle in motion is an entity advancing along its route, riders aboard it (no parked body, no landing jump); the car and the cab (`systems/vehicles.rs`) | 165 (part) |
| 5 | I3: the El as trains running the line (`systems/transit.rs`) | 165 |
| 6 | I3: the van: the escort and the prisoner transfer replacing the 04:00 jump (`stations.rs:347`), and the arrest hand-off | 166 |
| 7 | I4's bugs: the courier/builder `Unload`, island foremen hires and the day labourer, the arms dealer appointment, the cab's body driver at 20k | 168 (part) |
| gate | Shadow V8 on the island (`isle_m_k2`, 20k, seeds 42-44) against `target/shadow_v7/`, V7's tool, windows and critics | — |

**Re-read at the gate, not fixed here unless V8 says so:** island hunger at d90 (`OPEN-167`, I5); the remaining
one-block shifts (`OPEN-169`); the presence leaks (`OPEN-170`).

**To the pressure pass (I4):** the bouncer's refusals, the joytoy's clients, the scav's harvest from the scav's side.
