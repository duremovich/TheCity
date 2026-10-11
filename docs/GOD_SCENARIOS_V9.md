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

## `god_kill_ten_fathers_day_10` (the Hit market)

2026-10-11, the Hit market phase 4 ([docs/HIT_MARKET.md](HIT_MARKET.md) § 9). Every killing and every Hit here is a record on a struct between fictional agents, matched by a score and resolved by a seeded roll.

**Setup.** Seed 42 on the shipped config, run to day 10. Ten household heads are picked: married men with a living wife, the most children first. Each gets a killer: a stranger to the house, neither a guard nor a gang member, at least 0.2 stronger (fighting) than the widow and every adult child, and the one with the fewest defenders about him. Each father is killed by the natural path (`kill_by`, the crime raised at the tile, a named witness when nobody noticed, as `KillFriend` does). A control clone of the day-10 city runs unshocked, watching the same houses. 30 days.

**Asserted.**
- At least one widow or adult child places a Hit within 30 days.

**Measured (2026-10-11, with P4's D8 fix).**

| | Hits placed by a house | organic Revenge posted | organic Hits done | Commission errands | violent deaths |
|---|---|---|---|---|---|
| shock | 1 | 5 | 2 | 2 | 48 |
| control | 0 | 1 | 1 | 0 | 45 |

The houses:

| outcome | houses | what the widow could buy on day 10 |
|---|---|---|
| bought a Hit on him (26 coins, 13 down, fulfilled the same day) | 1 | 26 at Fixer#531 |
| hunted him herself | 2 | 499-563, direct |
| the law jailed the killer first | 3 | 38 at Fixer#531 (17 coins: 2 short of the deposit), 497 and 1,029 direct |
| held a 0.85 grudge on a free killer for 30 days, bought nothing | 4 | 465-1,025, direct |

**Reading.** The world reacted, thinly. Every widow formed a lethal grudge from the named deed. Two of the ten lived in reach of the city's one Fixer, where a Hit on a loner cost 26-38 coins. One of those two bought one and the killer died that evening. The other was 2 coins short of the deposit, and the law jailed the killer before she could pay. The eight off the Fixer's reach could only post direct. Their own circle held fewer than `ask_rank` guns through the gun gate, so the price fell back to M16a's quote (465-1,029), against wallets of 0-47. Two hunted the killer themselves, braver than the clause expected. Four waited out the month with the grudge and the killer free. The shock run posted four more organic Revenge records than the control besides the widow's (their buyers are not printed: the scenario watches the houses only).

On the first draft (killers merely stronger than the widow) five of ten widows hunted the killer themselves and no one bought: the hire gap (`[contracts] hire_gap` -0.1) sends a near-equal widow after him on her own.

**Gaps.**
- The market stops at the one Fixer's reach (HIT_MARKET.md phase 4, gap 1): off it, a widow faces the old quote. Cheap death is a Mid East privilege.
- No father on day 10 has an adult child (the founding population's children are children), so "a widow or adult child" is a widow on this seed.
- Nothing of the law's answer reaches the widow's choice: three of the killers were jailed, and their widows still held a buy-or-hunt grudge (HIT_MARKET.md phase 4, the lawful-buyer gap).
