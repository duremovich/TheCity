# The island pass (draft spec, 2026-10-22)

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

## Scope (to spec with Dylan)

- **Distance (I1, I2):**
  - The pick sites are `plan.rs` `bed_of` (the Hideout bed), the hangout spot, the Hotel pick and the job board/foreman.
  - The segment bug: a leg ends in the wrong district (`exec/legs.rs` `segment_end`, corp bug 1).
  - The Hideout/Home sleep ping-pong.
- **Vehicles (I3):**
  - The moving vehicle and the aboard link, on the existing vehicle entities (`systems/vehicles.rs` trips).
  - El trains in `systems/transit.rs`.
  - The exec ride, the escort and prisoner van replacing the 04:00 transfer jump (`stations.rs:347`), and the arrest hand-off.
- **The dead trades (V7 row 6):**
  - The courier and builder `Unload` at their own post.
  - The cab with no body driver at 20k (`OPEN-149`).
  - The bouncer who never refuses, and the joytoy with no client.
  - The scav's harvest, seen from the victim's side only.
  - No arms dealer.
  - Island foremen who make 0 hires.
- **After those, re-read:**
  - Hunger on the island at day 90 (no clinic treats malnutrition).
  - The remaining one-block shifts: the fabber, the four roles missing from `[shift.roles]`, supers with no round, sweeper stands, Court and Deal.
  - The presence leaks (the teacher's door, the 00:00 orderly batch, the 21:00 venue counters).
- **Phase 0:** retire the pre-view pass's eight switches, unless the close does it.
