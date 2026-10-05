# God scenarios v1

The first run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"): shock
the seed-42 city (2,000 residents, default config) with a god-mode actor and read how the factions
react. Nine scenarios plus an unshocked control, in `citysim/tests/god.rs`:

```
cargo test --release -p citysim --test god -- --ignored --nocapture
```

About 90 s for all ten at four threads. Each test prints the tables below, the order and posture
history of the whole run, a row every 5 days and the story events (orders, postures, raids,
jailbreaks, bribes, sacks) from day 43.

## How the suite works

- **The shock is at day 45, not day 30.** The brief asked for day 30. On seed 42 the gangs are still
  forming then: The Hollow had 4-5 members at day 30, all of them in the Jail, so it had no leader to
  decapitate and `JailGang` jailed 0 of them. Both gangs reach 15-16 members by day 40-45. Baseline is
  days 30-45 and the run is observed to day 105, so the post-shock windows are 45-75 and 75-105.
- **Reactions are measured against the control, not the baseline.** The first version compared the
  shocked week with the pre-shock baseline, and the unshocked world passed that check easily: territory
  is still growing at day 45 and orders change about twice a day. Seed 42 is deterministic, so every
  difference between a shocked run and the control comes from the shock. A scenario passes when, within
  7 days, an order or the law's posture differs from the control on some day, or a daily count's mean
  differs from the control's by more than twice the baseline's standard deviation. The control is
  computed once per test process (`OnceLock`).
- **God commands** (new `PlayerCommand` variants, logged and replayed like the rest, each a
  `PlayerAction` event prefixed `God:`): `KillAgent`, `JailAgent { who, days }`, `FreeAgent`,
  `FundGang`, `SeizeGangTreasury`, `KillGang`, `JailGang`, `FireAllGuards`, `SetTreasury`. CLI
  levers name a gang by its index and resolve to ids when they fire: `kill_leader=0`,
  `jail_gang=0:60`, `kill_gang=0`, `fund_gang=1:10000`, `seize_gang=0`, `fire_guards=1`,
  `treasury=-50000`.
- **Cross-check.** The bankrupt and takeover scenarios were also run through the CLI levers
  (`--lever day=45:...`) and `tools/analyze_run.py`. Every number matched the tests' own: bankrupt
  days 45-51 were 272.3 thefts/day and 13.0 starvation deaths/day in both; takeover days 45-51
  were 1.6 thefts/day and 0.3 starvation deaths/day, with 73.7 gang members (58.7 + 15.0).

### A bug the suite found (fixed)

`law::jail_suspect` (an escort arriving at the Jail) never checked whether the suspect already had a
`Sentence`, so it overwrote one. The first god jailing hit a member who was in cuffs at that moment: a
60-day sentence was replaced by "6 days for Extortion" and she walked out on day 51. The same thing
happens in ordinary play: `law::arrest` does not check `cuffed_by`, and the event log shows two guards
arresting the same suspect one tick apart ("Hilde Whitlock arrested Zane Thorne" / "Marek Abernathy
arrested Zane Thorne"). The second escort then sentenced him again. Fixed with a three-line guard in
`jail_suspect`. Separately, god sentences are filed as **Assault**, not Theft: a full Jail makes room
by freeing the Theft prisoner with the longest sentence, and a 60-day god sentence for Theft was always
that prisoner.

The fix changed the trajectory before day 45 (fewer double sentences), so every number below comes
from the post-fix run.

### Reading the tables

Per-day means (± standard deviation) over each window. g0 is The Hollow (south-west), g1 is Ninefold
(south-east). "ctl" columns are the unshocked control. Territory counts Homes held (claim count 3),
out of 400.

## The control

| metric | base 30-45 | 45-75 | 75-105 |
|---|---|---|---|
| violence (Assault+Murder) | 4.9 | 5.7 | 5.8 |
| thefts | 1.0 | 2.4 | 1.8 |
| arrests | 6.1 | 7.3 | 5.5 |
| g0 members / territory | 13.3 / 3.9 | 22.1 / 18.1 | 21.1 / 22.6 |
| g1 members / territory | 15.6 / 10.7 | 20.6 / 21.3 | 40.6 / 34.3 |
| guards | 35.3 | 35.5 | 35.3 |
| starvation | 1.1 | 1.0 | 1.0 |

The baseline itself has things worth knowing. Orders flap about 1.7 times a day (Expand ↔ LieLow on
every arrest). The law cycles Patrol → Garrison → Patrol about every 5 days. Each gang's membership
jumps on a windfall: g0 to 24 after 36 coins on day 49, g1 to 42 after 110 coins on day 69. Raids are
mostly sacks of an empty Hideout, and jail storms are mostly one raider.

## god_decapitate_gang: kill The Hollow's leader

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 4.0 | 6.5 | 5.0 | 5.7 | 5.8 |
| g0 members | 13.3 | 15.0 | 21.4 | 22.1 | 22.1 | 21.1 |
| g0 territory | 3.9 | 9.4 | 17.1 | 30.1 | 18.1 | 22.6 |
| g0 treasury | 1.1 | 1.1 | 4.2 | 1.8 | 3.9 | 1.9 |

Florian Northcott is struck dead on day 45. `recompute_leader` installs a successor the same tick, and
the `LeaderChanged` and `MemberKilled` shocks make it rethink at once: Contest, then Expand. On day 46
the new leader orders a BreakOut (the control lies low); it fizzles into LieLow. Nobody leaves. The
gang misses the day-49 windfall the control got, so it stays at 15 members for ten days before its own
windfall (29 coins, day 54) lifts it to 25. By day 105 it holds more territory than the control's
Hollow (30 against 23). There is **no Retaliate against Ninefold**: the death has no killer, and
`MemberKilled { by_rival: false }` carries no grudge.

**Verdict: recover.** The decapitated gang behaves like a gang with a different personality at the
top, nothing more. It does not panic, does not split, and does not suspect anyone.

## god_jail_whole_gang: jail all 15 Hollow members for 60 days

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 2.1 | 2.8 | 1.3 | 5.7 | 5.8 |
| arrests | 6.1 | 3.9 | 4.9 | 4.5 | 7.3 | 5.5 |
| g0 territory | 3.9 | 7.0 | 6.6 | 2.6 | 18.1 | 22.6 |
| g1 territory | 10.7 | 17.0 | 23.1 | 40.2 | 21.3 | 34.3 |
| jailed | 41.5 | 45.3 | 43.9 | 41.0 | 52.7 | 40.7 |

Eleven members were already inside and had their sentences extended; four were taken in. The Hollow
is frozen: with no free leader, `gather_inputs` returns `None` and the order stays LieLow from day 41 to
day 70. Then a fresh recruit, Quill Mercer, joins on day 70. As the only free member he becomes acting
leader, and on day 71 he takes a gang of 15 jailed veterans from LieLow to Expand. Until then nobody
joins, because the gang is at the promise threshold with an empty treasury. Its territory decays from 7 Homes to 2 as
Ninefold contests the border, and Ninefold ends with 45 Homes against the control's 38. The Hollow
storms nothing, since none can: BreakOut needs `breakout_min_members` free. The law **stays in Garrison**
(the control drops to Patrol on day 45), because a quarter of its cells now hold gang members. It
holds Garrison to day 57, then cracks down on Ninefold. Ninefold attempts four breakouts (days 53, 70, 87,
104), each by one or two raiders against 3-9 guards, and loses all four. City violence halves.

**Verdict: bunker (the law) and nothing (the jailed gang).** The rival profits slowly, through Contest
and not through any war. Nobody outside the Jail cares that 15 people are inside.

## god_kill_gang: kill all 15 Hollow members

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 2.3 | 6.5 | 5.5 | 5.7 | 5.8 |
| g0 members | 13.3 | 4.4 | 17.2 | 17.7 | 22.1 | 21.1 |
| g0 territory | 3.9 | 7.3 | 16.3 | 30.0 | 18.1 | 22.6 |
| g1 members | 15.6 | 20.3 | 20.2 | 34.9 | 20.6 | 40.6 |
| g1 territory | 10.7 | 17.4 | 22.8 | 32.1 | 21.3 | 34.3 |

The Hollow is a hydra. By the end of day 45 it has 1 member again: an ex-convict joins through the
JoinGang bootstrap, which sends anyone with a `WasArrested` memory to the nearest empty gang. The dead
gang keeps its 7 Homes of territory, and their tribute fills its treasury (16, then 30 coins). On day 52
a re-formed Hollow of two breaks three convicts out of the Jail (1 raider against 4 guards, won). By day
54 it has 22 members, level with the control's Hollow (23). Ninefold notices only through a raid
on day 46, when it sacks the empty Hideout for 24 coins. It does not take the city: its territory is
within a few Homes of the control's for the whole run. Crime falls for a week (violence 2.3 against
4.1), then returns to normal.

**Verdict: recover, fast.** Killing a whole gang costs it about nine days. The Hideout, the territory
and the tribute outlive the members, and the Jail is a recruiting pool.

## god_fund_gang: give Ninefold 10,000 coins

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 18.3 | 12.1 | 3.5 | 5.7 | 5.8 |
| arrests | 6.1 | 11.9 | 7.4 | 2.2 | 7.3 | 5.5 |
| g1 members | 15.6 | 56.9 | 59.2 | 60.0 | 20.6 | 40.6 |
| g1 territory | 10.7 | 28.1 | 48.6 | 64.0 | 21.3 | 34.3 |
| g1 treasury | 2.5 | 9447 | 7608 | 3016 | 8.2 | 1.9 |
| g0 territory | 3.9 | 6.6 | 3.4 | 4.6 | 18.1 | 22.6 |
| bribes | 0 | 0 | 0.03 | 0 | 0 | 0 |

Money turns into members almost at once. Ninefold has 44 members by the end of day 45 and hits the
`max_members` cap of 60 by day 49. It goes Contest and takes The Hollow's turf: The Hollow falls from 7
Homes to 0 by day 54. Violence is 4x the control in the first week, mostly extortion fights and arrests
of the new recruits. The law garrisons (days 46-68) when the Jail fills (73 inside). It only turns on
Ninefold on day 74, and then Ninefold, sitting on 5,400 coins, **offers the captain 46 coins and is
refused (score 0.37)**. That is its only bribe in 60 days. The stipend (60 × 4 a day) drains the
treasury to 769 by day 105, after which the cap no longer holds. The Hollow keeps its 15 members and
never defects, never lies low for good, and never raids the richest target in the city: its ratio
against a gang of 60 shuts the Raid gate.

**Verdict: violence, then a plateau.** Funding a gang buys 4x the members and 3x the territory, and it
starves the rival of turf. It does not buy the law, and it does not buy the whole city: by day 105 the
funded gang holds 70 Homes, 17.5% of the city.

## god_fire_all_guards: dismiss all 36 guards

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| guards | 35.3 | 19.7 | 31.2 | 34.5 | 35.5 | 35.3 |
| violence | 4.9 | 8.3 | 5.2 | 4.5 | 5.7 | 5.8 |
| thefts | 1.0 | 2.7 | 2.2 | 3.2 | 2.4 | 1.8 |
| g0 territory | 3.9 | 10.3 | 20.5 | 31.8 | 18.1 | 22.6 |
| g1 territory | 10.7 | 16.7 | 21.6 | 30.7 | 21.3 | 34.3 |

`reconcile_guards` hires back at its cap of 5 a day: 5 guards at the end of day 45, 25 by day 49 and the
full 36 by day 54. The law, with a new captain, goes Garrison on day 50. Violence is about double for a
week, then back to normal. Nobody exploits the gap: no gang storms the Jail while it is held by 5
rookies (the first storm is day 53, against 9 guards, and is lost). Ninefold sacks The Hollow on day 46, as
it does in the control. Starvation barely moves. The Hollow ends 9 Homes ahead of the control, the
only lasting trace.

**Verdict: recover.** The law is self-healing at 5 hires a day, and the gangs do not read the law's
strength at all. Nothing in `OrderInputs` counts guards, so a week of a nearly empty police force is
not an opportunity anyone sees.

## god_garrison_forever: pin the law to Garrison from day 45

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 6.3 | 6.2 | 2.5 | 5.7 | 5.8 |
| thefts | 1.0 | 2.1 | 2.1 | 2.6 | 2.4 | 1.8 |
| arrests | 6.1 | 8.1 | 9.0 | 2.5 | 7.3 | 5.5 |
| raids | 0.0 | 0.3 | 0.1 | 0.1 | 0.1 | 0.0 |
| g0 territory | 3.9 | 8.1 | 12.2 | 21.1 | 18.1 | 22.6 |
| g1 territory | 10.7 | 15.7 | 26.2 | 37.7 | 21.3 | 34.3 |
| jailed | 41.5 | 52.4 | 65.9 | 17.1 | 52.7 | 40.7 |

With the whole guard holding the Jail, Ninefold follows the day-46 sack the control also has with a
second one on day 48 (29 coins), and recruits on the loot (16 to 23). Its territory runs about 4 Homes
ahead of the control's. The Hollow does slightly worse. Street crime is **no higher**: arrests in days
45-75 are higher than in the control (9.0 against 7.3), presumably because guards still see
suspects on their way to and from the Jail. In days 75-105 the city is quieter than the control (violence 2.5 against
5.8, 17 jailed against 41). Every jail storm fails (5 guards at the door).

**Verdict: little.** Garrison is supposed to be the posture that leaves the streets open, but posture
barely changes how many arrests the law makes. Only the gang-on-gang raids respond.

## god_crackdown_forever: pin the law to Crackdown from day 45

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 5.7 | 5.2 | 2.7 | 5.7 | 5.8 |
| jailbreaks | 0 | 0.14 | 0.07 | 0.03 | 0 | 0 |
| g0 territory | 3.9 | 9.0 | 18.1 | 31.4 | 18.1 | 22.6 |
| g1 members | 15.6 | 16.0 | 16.0 | 16.0 | 20.6 | 40.6 |
| g1 territory | 10.7 | 15.9 | 20.7 | 25.0 | 21.3 | 34.3 |
| starvation | 1.1 | 0.9 | 1.5 | 2.8 | 1.0 | 1.0 |
| bribes | 0 | 0 | 0 | 0 | 0 | 0 |

The pin turns on The Hollow first, and the target flips between gangs every few days (at least six
"the crackdown turns on" events). With a third of the guard on the Jail, **jailbreaks succeed**: The
Hollow frees 3 on day 46, and Ninefold frees 3 on each of days 53, 69 and 86. The last two met 0
guards. The law cannot garrison in answer, because it is pinned. Ninefold is the gang that pays: it
never gets the windfall that took the control's Ninefold to 40 members, and it stays at 16. Its
territory ends 9 Homes short. The Hollow ends 9 Homes ahead. **Nobody bribes**: the price is 46 coins
and both treasuries run at 0-5.

Fragility note: before the `jail_suspect` fix, the captain was already in Crackdown on day 45 in the
control. The same pin then changed nothing for 19 days and failed the 7-day check. A pin only shocks
the world when the captain would not have chosen it.

**Verdict: split pressure, jailbreaks.** A permanent crackdown that keeps changing target is weaker than
the brain's own cycle. It costs the law its Jail, and it hurts whichever gang it happens to sit on
during that gang's windfall.

## god_bankrupt_city: set the Treasury to -50,000

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| thefts | 1.0 | 272 | 1200 | 1719 | 2.4 | 1.8 |
| violence | 4.9 | 18.3 | 23.6 | 18.9 | 5.7 | 5.8 |
| arrests | 6.1 | 17.4 | 20.5 | 17.3 | 7.3 | 5.5 |
| starvation | 1.1 | 13.0 | 5.3 | 1.3 | 1.0 | 1.0 |
| guards | 35.3 | 31.6 | 23.6 | 8.1 | 35.5 | 35.3 |
| g0 territory | 3.9 | 9.6 | 28.3 | 82.0 | 18.1 | 22.6 |
| g1 territory | 10.7 | 14.0 | 32.2 | 92.7 | 21.3 | 34.3 |
| population | 2058 | 2032 | 1959 | 1913 | 2083 | 2112 |
| city treasury | 26324 | -29395 | -12076 | -3117 | 26987 | 26679 |

This is the biggest reaction in the suite, and the god actor did not touch a single faction. The dole
stops (`treasury < 0`) and wages go unpaid. Theft climbs from 16 to 694 a day in a week and settles
at about 1,700 a day, nearly one theft per resident per day (87,617 in the run). Starvation spikes
first: 58 deaths on day 49 alone and 226 over the run, against the control's ~100. Guards quit unpaid,
`reconcile_guards` re-hires 5 a day, and they quit again (1,582 Hire and 1,496 Quit events), so the
force decays to 5-8. The Jail sits at its cap of 80. The law flips Patrol ↔ Crackdown 20 times. **The
gangs take the city by default**: with the guard gone and every Home's residents broke, extortion
claims stick. By day 105 the two gangs hold 95 + 124 = 219 of 400 Homes (55%), against 61 in the
control, with **no more members than before** (15-24 each). Taxes still flow, so the Treasury climbs
back to -1,500 by day 79 and slides again. Throughput drops from about 5,000 to about 1,500 ticks/s
(`analyze_run.py` flags 89 days under 8,000), with 121,000 `PlanAborted` events from theft churn.

**Verdict: collapse.** The cheapest way to hand the city to the gangs is to stop paying the guards. The
gangs never decide anything here; the vacuum does the work. Nobody gets recruited off the misery
either: desperate recruits need a gang that can pay a stipend.

## god_city_takeover_by_force: fund The Hollow 100,000, kill Ninefold's leader, fire the guards

| metric | base | 45-52 | 45-75 | 75-105 | ctl 45-75 | ctl 75-105 |
|---|---|---|---|---|---|---|
| violence | 4.9 | 32.7 | 15.6 | 4.7 | 5.7 | 5.8 |
| g0 members | 13.3 | 58.7 | 59.7 | 60.0 | 22.1 | 21.1 |
| g0 territory | 3.9 | 26.3 | 51.5 | 72.7 | 18.1 | 22.6 |
| g0 treasury | 1.1 | 99368 | 97516 | 93324 | 3.9 | 1.9 |
| g1 members | 15.6 | 15.0 | 15.0 | 15.0 | 20.6 | 40.6 |
| g1 territory | 10.7 | 13.6 | 4.5 | 0.0 | 21.3 | 34.3 |
| guards | 35.3 | 19.4 | 31.8 | 34.8 | 35.5 | 35.3 |

Territory share of the 400 Homes (g0 / g1):

| day | 44 | 49 | 54 | 59 | 64 | 74 | 84 | 94 | 104 |
|---|---|---|---|---|---|---|---|---|---|
| The Hollow | 1.8% | 8.0% | 12.0% | 14.8% | 15.2% | 17.0% | 17.8% | 18.5% | 19.0% |
| Ninefold | 3.5% | 3.5% | 2.0% | 0.2% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |

The Hollow has 51 members at the end of day 45 and hits the cap of 60 on day 49 (50 GangJoin events
that week). Violence runs at 8x the control for a week. Ninefold, beheaded, takes a new leader,
loses a raid (2 raiders against 5 defenders) and is pushed off every Home by day 64: 20
`TerritoryFlipped` events in days 49-55. It never dies, though. It keeps exactly 15 members and lives
on as a gang with no turf and no money. The guard is back to full in 9 days, as in `fire_all_guards`.
From day 60 The Hollow, with 93,000 coins and 60 members, simply Expands at about 0.4 Homes a day and
stops at **19% of the city**. It never raids (the rival has nothing to take), and it never bribes,
though the law cracks down on it during days 78-94 and 101-103.

An earlier run (before the `jail_suspect` fix) ended differently. Ninefold, at 15 members, raided The
Hollow's Hideout on day 99, found 1 defender, sacked it and took **90,774 coins**, then recruited to the
cap of 60 in a day. A war chest kept at the Hideout is a target, and a 60-member gang leaves 1 person
home.

**Verdict: no, you cannot take the city by force.** You can take the other gang's turf in a week, but
the ceiling is structural: `max_members` caps the army at 60, extortion runs at one Home per member per
few days, and there is nothing to spend 93,000 coins on. Bankrupting the city (55% in 60 days) beats
funding a gang (19%).

## Gaps

### Things that could not be expressed (no command)

- **Name a Crackdown's target.** `SetLawPosture(Crackdown)` cracks down on whichever gang is
  most-reported that day, and the target flips every few days. There is no way to say "go after The
  Hollow".
- **Assassinate with attribution.** `KillAgent` has no killer, so a god kill can never start a feud.
  `KillAgent { by: Option<EntityId> }` (kill as a member of the rival gang) would test retaliation.
- **Found, merge or split a gang.** Gangs are one per Hideout from the config. There is no `FoundGang`,
  `MergeGangs` or `PromoteToLeader`.
- **Move money or people between gangs.** No `Defect(agent, gang)` and no bribe-a-member.
- **Pay the guards from outside.** The bankrupt city can only be fixed with `SetTreasury`. There is no
  corp or patron that can fund the law (an M11 hook).
- **Raise the member cap or seize a Hideout.** `max_members` is config only, and Hideouts cannot be
  demolished (`DemolishHome` is Homes only).

### Reactions that did not happen but should

- **A decapitated gang never splits or panics.** The successor takes over in the same tick with no
  succession fight, no defection of the dead leader's loyalists, and no LieLow-for-mourning. A
  `LeaderChanged` shock should at least open a window for a split or a rival's opportunism.
- **No attribution, so no wrongful revenge.** A killer-less death of a leader never casts suspicion on
  the rival. Gangs should guess a culprit (the rival, if there is a grudge) and sometimes guess wrong.
- **A dead gang re-forms at once, and its territory and tribute outlive it.** Killing all 15 cost The
  Hollow nine days. Empty gangs should lose their claims (the 30-day disband is far too slow), and the
  bootstrap should need more than an arrest record.
- **The rival never takes a dead or jailed gang's turf by war.** Ninefold took only what Contest
  happened to flip. A gang with a vanished rival should Expand into the vacuum on purpose.
- **Nobody reads the law's strength.** No `OrderInputs` term counts guards or the Jail's garrison, so
  firing the whole guard or pinning Garrison opens no window that any gang uses.
- **Money buys members and nothing else.** A 10,000- or 100,000-coin gang never bribes (the score has
  no "can afford it easily" term, and the one offer it made at 5,400 coins was 46 coins and refused).
  It never buys guns or Hideouts and never pays the rival's members to defect. The rich gang's
  treasury is a liability: it drains on stipends, and in one run it was looted in a single raid.
- **The law never hires when broke, and never reacts to being broke.** Unpaid guards quit and are
  re-hired at 5 a day forever (about 1,500 Hire/Quit pairs). The law has no "unpaid" posture, no
  volunteer guard, and no shift to private security.
- **Desperation never feeds the gangs.** 87,000 thefts and 226 starvation deaths did not grow either
  gang by one member, because desperate recruits need a gang that can pay a stipend. A collapsing city
  should be the gangs' best recruiting ground.
- **Raids and storms are one-man affairs.** Most jail storms and raids muster 1-2 raiders ("1 raiders vs
  4 guards"), even for a 60-member gang. Muster attendance does not scale with headcount.
- **The Jail is empty at muster hour.** Under the pinned Crackdown, Ninefold's storms on days 69 and 86
  met "0 guards". In a pre-fix run a storm met 0 guards under a pinned *Garrison*, the posture that
  is supposed to put the whole guard on the Jail. Jail duty is per shift, and the 22:00 muster can
  land on an unmanned hour.
- **Pinned Crackdown flips its target every few days** (at least six "the crackdown turns on" events in 60
  days), so it never collapses either gang.
- **Order flapping.** About 1.7 order changes a day at baseline (Expand ↔ LieLow on every arrest),
  even with `hysteresis = 0.10`. Shock rethinks bypass hysteresis entirely.
- **The Jail is a weak sink.** The law freed Theft prisoners to make room until god sentences were
  filed as Assault. A full Jail should be a law shock (build, release, or harden), not a silent
  rotation.
- **Performance under mass crime.** The bankrupt city runs at about 1,500 ticks/s, a third of normal.

### New scenarios worth trying after M11 (corps)

- **Buy the law:** a corp funds the Jail's payroll while the city is bankrupt. Does the law become the
  corp's?
- **Hostile takeover by money:** a corp buys every Home in a gang's territory. Does extortion of a
  corp's tenants start a corp-gang war?
- **Strike a corp's workers** while a gang offers stipends. Do the strikers join the gang?
- **A board decapitation:** kill a corp's CEO. Does the board split into factions, as a decapitated
  gang should and does not?
- **Corp funds a gang** as deniable muscle against a rival corp. Does the law trace the money?
- **Private prisons:** the city sells the Jail to a corp. Does capacity, or the sentence length per
  coin, change crime?
- **Repeat every v1 scenario across 10 seeds** and report the distribution, not one story. Seed 42
  showed how much the day-45 world state matters (the Crackdown pin was a no-op in one trajectory).
