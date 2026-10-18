# God scenarios v11: leverage, captives, the wounded, boards

The eleventh run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M16b (`docs/M16_CONTRACTS.md` § 4-6, § 11): hostages and the `Release` demand, the wounded and the Trauma
Team, boards and votes, and the Parley. (The spec names this file V8; V8-V10 were taken by M16a, hunger
as sickness and the El.) A coercion is a record on a struct, a hold a cuff flag on an agent, a wound a
timer, a vote a tally over a list: every one is game state between fictional agents of a simulated city,
moved by seeded dice rolls. Four scenarios in `citysim/tests/god.rs` (the v11 section, on the v8
harness), one per reaction class, each against the unshocked v8 control of the same seed:

```
cargo test --release -p citysim --test god -- --ignored --nocapture god_hostage_captain_spouse_release god_subscribe_and_chrome_everyone_3 god_call_vote_habitat_after_coerce god_parley_strongest_gangs_in_vendetta
```

About 35 s for the four and the control on parallel threads. Measured on M16b phase 5's tree (main at
fc0e649: phases 1-4 merged).

## How the suite works

- **Seed 42, default config, 60 days**, the shock at the start of its day, against `v8_control`. Each
  prints the v8 board's table plus the M16b rows (coercions active, hostages taken, freed and held,
  Rescues posted, downed, bled out, stabilised, Trauma calls, saves and subscribers, board votes, votes
  swung, chairs fallen, Parleys, tribute, factions distracted, the law's alertness), the story naming
  the watched agents, and the order runs of the factions concerned.
- **Each asserts that the god commands applied and that the world reacted** (a named reaction per
  scenario below); how it reacted is printed and judged here, as gaps, not as red tests.
- **Commands** (`PlayerCommand`, logged and replayed; CLI grammar in brackets): `Hostage { captive,
  holder, at, demand }` (`hostage=<agent>:<holder|none>:<building>[:coins<n>|release<agent>]`),
  `Subscribe { agent, corp }` (`subscribe=<agent>:<corp slot>`), `ChromeEveryone { tier }`, `Coerce {
  agent, over, leverage, demand }` (`coerce=<agent>:<over>:<threat|debt<n>>:<coins<n>|weekly<n>|vote<slot>>`),
  `CallVote(corp)` (`call_vote=<slot>`), `DeclareVendetta { a, b, weight }`.

## The scenarios

### `god_hostage_captain_spouse_release`: the strongest gang holds the captain's spouse for a convict, day 20

The Unplugged (22 members) took Lavinia Voss, captain Florian Bellamy's spouse, into Hideout#292 and
demanded Roxy Whitlock let out. The law's competence read 0.35 (under the 0.4 that lets a captain bend),
but the captain's lawfulness read 1.00. Asserted reaction: the hold was taken and something answered it
(a freed or killed captive, a Rescue, a release, a broken coercion).

| days / ctl | 20-30 | 30-60 |
| --- | --- | --- |
| hostages taken / freed | 1 / 0, 1 / 0 | 0 / 1, 0 / 1 |
| Rescues posted | 0 / 0 | 0 / 0 |
| violent deaths | 19 / 8 | 45 / 45 |
| coercions active (mean) | 2 / 1 | 2 / 2 |
| law posture | Patrol to day 32, Crackdown 33-34, 37-57, Garrison 58-59 | control: Patrol to day 45, Crackdown 46-59 |

- **Reacted, but not as the law.** The captain refused every daily roll (lawfulness 1.0). The convict
  walked out on day 23 when her sentence ran out, not because of the demand. On day 22 the captain was
  himself arrested and sentenced for an earlier street beating laid to him by a witness. While he sat in
  the Jail the coercion over him rolled nothing. On day 27 the hold ran out (`hostage_days`) and The
  Unplugged let Lavinia walk.
- The law's posture moved to Crackdown 13 days earlier than the control. Violent deaths in the first
  ten days ran 19 against 8.

**Gaps:**
1. **The law never answers a hostage taken over its own captain.** There was no Rescue: `seek_help` posts
   one only from the captain's own purse. There was no Crush against the holding gang and no Treasury-paid
   Rescue. The spec's "does the law comply, Crush, or post a Rescue" has only one live branch, and only
   for a bent captain.
2. **The hostage starved in the hold** (`Starving` on day 24). The Unplugged's treasury held a few coins,
   and the hold feeds from the holder's purse. Nothing releases or relocates a captive the holder can't
   feed. The spec says "no captive starves"; she lived, but only because the hold ended on day 27.
3. **A jailed coerced party freezes the coercion.** `roll_hostage` skips an `over` in the Jail, so the
   hold just runs out the clock. The demand could pass to the next captain.
4. Not M16b: the captain was convicted for a beating that a witness laid to him, from a street fight on
   day 19. Guards' own fights are attributed as any agent's. Worth a look in the law.

### `god_subscribe_and_chrome_everyone_3`: every Corp-class adult subscribes to Trauma, then everyone is chromed to tier 3, day 5

746 Corp-class adults were offered Arasaka's crew at 28 a week; 248 could pay, 498 were refused for
unaffordable. Then `ChromeEveryone 3` was applied. Asserted reaction: subscriptions stood above the
control's and the chromed city's downed and episodes moved.

| days / ctl | 5-20 | 20-60 |
| --- | --- | --- |
| Trauma subscribers (mean) | 132 / 9 | 9 / 10 |
| violent deaths | 182 / 15 | 574 / 53 |
| downed / bled out / stabilised | 88 / 5, 39 / 3, 0 / 0 | 288 / 22, 141 / 15, 4 / 2 |
| Trauma calls / saves | 3 / 0, 0 / 0 | 2 / 0, 0 / 0 |
| contracts expired | 249 / 2 | 35 / 30 |
| episodes (5-60) | 1,179 / 3 | |

- **Reacted, violently.** Tier-3 chrome everywhere made 1,179 cyberpsychosis episodes against 3. Violent
  deaths ran at about ten times the control, and 376 people went down against 27.
- **The Trauma crews did not keep up. There were no saves at all:** 5 calls, 0 saves. 257
  subscriptions stood on day 10, but the second week's fee lapsed almost all of them. 249 standing
  records expired unpaid, and the subscriber count was back at 9 by day 20.

**Gaps:**
1. **The Corp class can't carry a weekly fee.** Two thirds of Corp-class adults held under 28 coins on
   day 5, and the rest lapsed a week later. A subscription is an exec's luxury at HEAD. Corp-paid
   subscriptions for staff (an employer benefit) are the obvious hook.
2. **A Trauma call is one guard carrying, and it can't scale to a riot of psychos.** With 88 down in 15
   days there were 3 calls, and none arrived in time inside `bleed_hours`. The spec's `trauma_crew`-sized
   response (phase 3 deviation 5) is the missing half.
3. **Stabilising saved 4 of 376.** Clinics need a Ripperdoc on staff, and a chromed city's Ripperdocs
   are themselves in episodes.

### `god_call_vote_habitat_after_coerce`: two Habitat board members coerced to vote for a third, then a vote called, day 10

Kofi Voss, the board member of the highest standing after the chair, threatened Mags Mercer and Axel
Oakes (`Demand::Vote`), and god called the vote. Asserted reaction: a vote was held, and the pinned
members swung or the chair fell.

| days / ctl | 10 | 11-60 |
| --- | --- | --- |
| board votes | 1 / 0 | 2 / 2 |
| votes swung | 2 / 0 | 0 / 0 |
| chairs fallen | 1 / 0 | 0 / 1 |
| violent deaths | 0 / 0 | 54 / 65 |

- **Reacted, exactly as written.** Both pinned members swung. The tally was Kofi Voss 3, Dagny Kaneda 2,
  so the chair fell and Kofi became Habitat's exec on the spot (`Promoted`, his post left).
- **The brain's order barely moved.** Habitat ran Hunker/Secure in alternation in both runs, shifted by a
  day or two (Hunker 5-13, Secure 14-16, Hunker 17-30, Secure 31-44 against the control's 5-14, 15-18,
  19-32, 33-38). Habitat's orders are driven by cash and losses, and a board Personality of greed 0.69,
  lawfulness 0.58, courage 0.44 doesn't tip them. Habitat's wage multiplier climbed from 1.09 to 1.51 in
  13 days (`WageMoved`: payroll against revenue).
- **The deposed chair fell all the way down.** Dagny Kaneda left the board (no longer the chair, not an
  employee). She took an Orderly post on day 14, quit it unpaid on day 21, was evicted by Habitat (her own
  former corp) on day 23, and was jailed overnight for vagrancy twice by day 25.

**Gaps:**
1. **No golden parachute.** An exec deposed by a vote keeps no seat, no savings beyond its wallet, and no
   standing housing. Brutal and in tone, but a deposed chair could keep a board seat as a member, and
   could be a challenger next time. At HEAD the departed rule drops it.
2. **Habitat's mean Personality barely differs from the exec's in the orders it drives.** The board shows
   in Nutrix, not Habitat (below).
3. **A coerced vote is not a crime.** Nobody files on Kofi's threats (the leverage roll's backlash report
   did not fire here), and the press never hears of a bought chair. A `Story` for a swung vote would give
   Spin and the honour economy something to chew on.

### `god_parley_strongest_gangs_in_vendetta`: a vendetta declared between the two strongest gangs, day 20

The Unplugged (22) and The Hollow (20) were put in blood by god (weight 1.0). Asserted reaction: a gang's
orders moved off the control's, or a Parley.

| days / ctl | 20-35 | 35-60 |
| --- | --- | --- |
| contracts posted | 68 / 45 | 192 / 139 |
| Hits posted / done (squad) | 3 / 2, 0 / 0 | 11 / 3, 3 / 0 (3 / 0) |
| violent deaths | 20 / 13 | 42 / 40 |
| Parleys | 0 / 0 | 0 / 0 |
| factions distracted (mean) | 6 / 4 | 8 / 5 |

- **Reacted through the board, not the street.** The feud bought Hits: 14 against 5 posted, and 3 done by
  squads against none. Both gangs' orders shifted: The Hollow tried
  Retaliate on days 41-42, The Unplugged on day 47. Attention held both gangs and their neighbours
  distracted (8 against 5 factions).
- **No Parley.** A Parley needs the rival to be stronger by `parley_ratio`. Two near-equal gangs never
  qualify, so the strongest pair is exactly the pair that cannot make peace.

**Gaps:**
1. **Equals never parley.** Peace between evenly matched gangs needs a war-weariness term: losses on both
   sides, or the vendetta's age.
2. **A declared vendetta is slow to bite.** Retaliate waited 21 and 27 days, because its gate wants a
   fresh grudge and a raid-ready crew. The hired Hits are where the violence went.

## Why the boards move employment (the phase-4 finding)

The phase-4 A/B moved employment −5 % and homelessness +25 % with boards on. Read off Habitat's and
Nutrix's order mix (share of days in each order, seeds 42-47 × 120) and their events:

| | Habitat | Nutrix | Nutrix Founded | Habitat Evicted |
| --- | --- | --- | --- | --- |
| main 5c3d2c4 (no boards) | Secure 71 %, Hunker 23 %, Grow 4 % | Grow 40 %, Secure 38 %, Research 20 % | 121 | 980 |
| phase 4, governance off | Secure 78 %, Hunker 20 % | Grow 45 %, Secure 31 %, Research 19 % | 117 | 1,037 |
| phase 4, boards on (two builds) | Secure 78 %, Hunker 22 % | Secure 57-61 %, Grow 13-18 %, Research 18 % | 62-63 | 1,075-1,124 |

- **It isn't rents.** Habitat's mix is the same with and without a board: no Squeeze, so no higher rent.
- **It's Nutrix growing less.** A board's mean greed (about 0.6-0.7) sits under the greedy exec's (0.9 or
  more; execs are picked greediest), and Grow reads greed. So Nutrix spends a third of the days in Grow
  that it did, and founds half as many Food outlets (about 62 against 117). Fewer outlets mean fewer jobs.
  The unemployed then fall behind on rent, and Habitat evicts 5-8 % more, through the same Secure/Hunker
  mix as before.
