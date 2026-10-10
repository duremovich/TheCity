# God scenarios v9: hunger as sickness

2026-10-10. One scenario for [docs/HUNGER_SICKNESS.md](HUNGER_SICKNESS.md), in `citysim/tests/god.rs`. It asserts that the world reacted to the shock, not a calibration band.

## `god_famine_synth`

**Setup.** Seed 42 on the shipped config, with `[sickness] enabled` and `[street] shanties` on (the dole city, the default stat table). From day 45, the Warehouse and every Market shelf are emptied each morning for 15 days. A control city runs the same days without the shock.

**Asserted.**
- Starvation on days 45-60 stays within the control's + 5.
- Bin meals rise above the control's.

**Measured (2026-10-10, P5).**

| | starvation | bin meals | Mission meals | synth meals |
|---|---|---|---|---|
| famine | 0 | 262 | 24 | 17,049 |
| control | 0 | 22 | 3 | 12,214 |

**Reading.** A single emptying is no famine: the World's imports refill a Market overnight, so the first draft (one emptying on day 45) moved nothing. Under a sustained famine the free-food floor answered:
- the bins fed 12 times as many meals as in the control;
- the Missions fed 8 times as many;
- buyers switched to synth, cutting each scarce shelf unit into three meals.

Nobody starved.

**Gaps.**
- The scenario runs the dole city's stat table (no `p_dive`), so every eligible off-screen hour dives. The flipped table's learned rate is the city the flip ships.
- A famine does not move the bin contests (no regular met a newcomer in 15 days), so the scarcity tells no fight story yet.
