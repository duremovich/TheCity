# God scenarios v5: the Virt plane

The fifth run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M14: the Virt plane (an abstract second board of nodes over the buildings, where a run is one seeded dice
contest per guarded node), Data, ICE and the tech tree. Five scenarios in `citysim/tests/god.rs`, each
against its own unshocked control (`v5_control`):

```
cargo test --release -p citysim --test god -- --ignored --nocapture god_wipe_zetatech_day_20 god_grant_decks_sump god_arasaka_ice_0 god_city_ice_0 god_door_before_raid
```

About a minute for the five (plus the control) on parallel threads. Measured on M14 phase 5 with its
final calibration (`docs/M14_VIRT.md`, "Implemented: deviations").

## How the suite works

- **Seed 42, default config, 105 days**, the shock at the start of day 45 (`god_wipe_zetatech_day_20`:
  day 20), observed against the unshocked control hour by hour. Each prints a table of sums over windows
  (runs, successes, Data stolen, sold and wiped, fried, flatlined, traced, ICE raised and lowered, Ledger
  hits on the Treasury and the coins taken, corp raids won and lost, plus the scenario's own rows), every
  living corp's treasury and tiers every ten days beside the control's, and the Virt story (tier changes,
  wipes, Ledger hits, doors, turned robots, flatlines, ICE moves, bankruptcies, Research orders, corp
  raids). The tests assert only that the god command applied (and, for the door scenario, that a door run
  was ordered); a failure to react is a gap below, not a red test.
- **New commands** (`PlayerCommand`, logged and replayed; a `PlayerAction` event prefixed `God:`):

  | Command | Effect | CLI lever |
  | --- | --- | --- |
  | `WipeCorpData(corp)` | `tech::wipe_store` on every node the corp owns (the wipe rule drops a tier when no backup is left) | `wipe_corp_data=8` |
  | `GrantDecks { district, n, tier }` | a free deck of `tier` to the `n` adults living in the district with the best hacking and no deck | `grant_decks=6:10:3` |
  | `SetCorpIce { corp, tier }` | every node the corp owns to ICE `tier`, the city's own (maker `None`), free | `set_corp_ice=6:0` |

  Phase 4's `SetCityIce`, `GrantDeck`, `RunNow` and `WipeData` are used as they are.

## The scenarios

Sums over the windows; "shock / ctl" against the control over the same days. Seed 42 in the control:
Militech lapses Deck 2 and goes bankrupt (days 70-92 across runs), Zetatech goes bankrupt on day 85, Vatra and
Greenline in days 84-103; Ledger hits land on the Treasury (1-2 a fortnight). With the phase 5 Research change
Nutrix scored Research at 0.45 against its standing Secure at 0.43-0.45 and flipped between the two on every
shock rescore from day 47; the shock rescore now keeps the standing order on an exact tie (phase 5, last change; these tables were
measured just before it).

### `god_wipe_zetatech_day_20`: every Zetatech node wiped on day 20

| days 20-50 / ctl | runs | ok | stolen | sold | Treasury hits / coins | corps in Research |
| --- | --- | --- | --- | --- | --- | --- |
| shock | 25 | 12 | 190 | 90 | 7 / 2,350 | 0 |
| control | 22 | 9 | 90 | 0 | 6 / 1,200 | 1 |

- **Reacted.** The wipe takes 816 units (Lab#442 432 Chrome, Lab#450 384 Industry) and, with no backup
  left, the wipe rule drops **Chrome 3 → 2 and Industry 3 → 2 at once** (`TechLost ... (wipe)` twice on
  day 20). Zetatech holds tier 2 to its bankruptcy (day 88, control 85).
- **Clinic tier-3 sales**: none in either run; nobody installs a tier-2 or tier-3 implant at Zetatech's
  Clinic on seed 42 at all (the dole city), so the spec's "Clinic tier-3 sales drop" cannot be seen. Every
  tier-3 implant Zetatech made now runs at tier 2.
- **No corp takes `Research` in the shocked run** (the control's Nutrix does, 1-3 corp-days): the wipe
  changes nobody's tier gap (Zetatech has no Tech rival, and its Industry 2 still leads the Food corps by a
  tier), and Research's 14-day books gate and score do not move.
- Freelance thefts double after the wipe (190 vs 90 units) as the stripped Labs refill.

### `god_grant_decks_sump`: tier-3 decks for the ten best hackers of Sump Central, day 45

| | runs | ok | stolen | flatlined | Treasury hits | Treasury taken | IceRaised |
| --- | --- | --- | --- | --- | --- | --- | --- |
| days 45-75 | 29 / 8 | 17 / 1 | 0 / 0 | 1 / 1 | 16 / 1 | 15,000 / 200 | 8 / 0 |
| days 75-105 | 42 / 17 | 25 / 8 | 240 / 120 | 0 / 0 | 17 / 2 | 19,400 / 800 | 11 / 1 |

- **The Treasury is hacked, hard**: 33 Ledger hits take 34,400 coins in 60 days (control 1,000). The city
  treasury still reads 101,189 on day 99 against the control's 91,992: the control's day 45-99 runs a
  different economy (Nutrix holds 21,184 there, 5,503 here), so the drain is the readable signal, not the
  closing balance.
- **The city never raises its own ICE** (0 city raises): only the `city_ice` lever moves the Treasury and the
  Precinct (Gaps). The corps make 19 raises (control 1) as robbed nodes harden.
- One flatline (day 48, the Treasury's ICE): a tier-3 deck plus hacking beats ICE 2 at p ≥ 0.81 a contest.
- The runners' winnings become NPC corps (Whitlock, Bartlett, Adeyemi, Eastwick, Oyelaran Holdings), every
  one bankrupt by day 104.

### `god_arasaka_ice_0`: every Arasaka node to ICE 0, day 45

| | runs | ok | hits on Arasaka | Treasury hits / coins | IceRaised |
| --- | --- | --- | --- | --- | --- |
| days 45-52 | 6 / 3 | 3 / 1 | 2 / 0 | 1 / 1, 200 / 200 | 2 / 0 |
| days 45-105 | 73 / 25 | 27 / 9 | 3 / 0 | 10 / 3, 4,050 / 1,000 | 11 / 1 |

- **Reacted, and Arasaka is not bled**: two thefts in the first week (40 Deck units) and one Ledger hit on day
  80 (71 coins), while the robbed Lab hardens under any order (phase 5) and Arasaka buys its ICE back (5
  re-raises). Its treasury ends near the control's (1,431 vs 1,343 on day 99).
- Runs triple after day 75 (59 vs 17) and Treasury hits with them: easy runs drift the runners' hacking up and
  fund their decks, and the better decks go for the biggest Ledger.

### `god_city_ice_0`: the city's ICE to 0, day 45 (the hacker-army test)

| | runs | ok | Treasury hits | Treasury taken | Data stolen |
| --- | --- | --- | --- | --- | --- |
| days 45-75 | 12 / 8 | 12 / 1 | 12 / 1 | 2,950 / 200 | 0 / 0 |
| days 75-105 | 31 / 17 | 28 / 8 | 15 / 2 | 8,350 / 800 | 445 / 120 |

- **Can a gang drain the Treasury? It bleeds it, slowly**: 27 Ledger hits from day 45 take 11,300 coins
  (control 1,000), with no city response (Gaps). Every run against an unguarded Treasury succeeds; the take is
  capped by `ledger_cap`, so the drain is a few hundred coins a day against a treasury near 100,000.

### `god_door_before_raid`: a god `RunNow` Door on every corp raid target from day 45

| days 45-105 | door runs ordered | doors opened | corp raids won | corp raids lost |
| --- | --- | --- | --- | --- |
| shock | 3 (Lab#442, Vat Farm#406 in the Vats, Street Market#413) | 0 | 3 | 0 |
| control | — | — | 2 | 0 |

- **Inconclusive**: the three corp raids after day 45 are ordered late (days 87-104); no door opens and the
  raids win 3 of 3 against the control's 2 of 2, so the door adds nothing measurable on seed 42. A natural door
  then a departing raid exists on seeds 44 and 45 of the M14 gate's first phase 5 run.

## Gaps

1. **The city never defends its own Ledger.** Only the `city_ice` lever moves the Treasury's and the
   Precinct's ICE; robbed 33 times for 34,400 coins (`god_grant_decks_sump`) or 27 times at ICE 0
   (`god_city_ice_0`), the city raises nothing. A city ICE response (the corps' "harden the robbed node",
   paid from the Treasury, or a captain's posture) is the missing mechanism.
2. **Research answers no shock.** Phase 5 made Research reachable (a street-tier gap, a lapse floor, a
   14-day books gate; Nutrix researches Industry and reaches tier 2 on seeds 43 and 45 of the gate), but nothing in
   these scenarios moves it: a wipe that costs Zetatech two tiers changes no rival's gap, and a corp that has
   just lost a tier reads lapse 0 again. (Nutrix's Research ↔ Secure flip at equal scores on every
   shock rescore is fixed: a shock rescore keeps the standing order on an exact tie.)
3. **Tier-3 sales cannot drop where there are none**: no agent on seed 42 installs a tier-2 or tier-3
   implant at Zetatech's Clinic in 105 days, wipe or not (the dole city's wallets), so the spec's spiral
   ends at "its tier-3 implants run at tier 2" with nobody noticing.
4. **Ledger targets are nearly all the Treasury.** At ICE 0 Arasaka's own Ledger is hit once in 60 days
   (`god_arasaka_ice_0`): the Treasury's purse outranks every corp's for every tier-2 deck, so opening a
   corp's door raises the city's losses instead. Target choice wants a diminishing
   return (a Ledger hit recently, an alarmed Trunk) or a risk term beyond the dice.
5. **A god door run does not land** (`god_door_before_raid`): the gangs' runners hold tier-1 decks and
   `RunNow` seats them at home, so against ICE 2 the door contests are lost; on seed 42 the corp raids after day
   45 come late and win anyway, so the scenario cannot show a door's worth. It needs a runner with a tier-2+
   deck or a seed with early corp raids.
6. **A robot turned or a camera blinded is rare in a natural run** (the M14 gate's bullet, now a printed
   finding): a robot is turned only by a Raid prelude on a robot-guarded corp building, seen on 1 of seeds
   42-49, and the freelance blind-first order is deferred with V33, so `Blinded` comes only from god. Phase 5
   makes a god `RunNow` Door on a robot-guarded building a Robot run (as the prelude's) and covers the
   mechanism with `orders_virt::test_god_door_run_on_robot_building_turns_the_robot` (ICE 0, no dice: the
   robot is turned for the runner).
7. **A hack grudge never orders a wipe** (the M14 gate's DataWiped, a printed finding): gang-on-gang runs
   are traced and name the runner (1-5 a seed on 42-49), but the grudge's wipe is a candidate only under
   VirtRaid, so a gang not already in VirtRaid never wipes (0 wipe runs, DataWiped 0 on all eight seeds). For
   the M14 review fix pass. **M14 review (thinly closed):** the premise was wrong. A probe of every grudge on
   42-49 at 16b9efe (eleven, all naming a gang) found the hacked gang already in VirtRaid at ten; the wipe was
   missing because the named gang's Hideout store was empty (0 at nine, 1 and 12 units at two): gangs sell
   every unit at midnight (`gang_data_keep` 0) before `gang_daily` writes the day's orders, and the robbing
   gangs are the have-nots (on seed 46, 63 steals were freelance, the Data going to the runner's deck,
   against 13 for a gang). The fix pass makes a grudge whose wipe is in reach lift VirtRaid
   (`[gangs.order_flat] virt_grudge` 0.2) and send the wipe first
   (`virt_review::test_hack_grudge_lifts_virtraid_and_orders_the_wipe_first`). On 42-49 it brings two wipe
   runs (seeds 44 and 46) and one wipe (seed 44, day 117: 10 units still unsold in the rival's
   Hideout#220), so DataWiped is an asserted existence bullet again, on one seed in eight; Data sold reads a
   six-seed mean of 211 (16b9efe: 343.5; seed 46's 931 fell to 221 after its day-88 grudge kept The Hollow in
   VirtRaid). `gang_data_keep` 20 was tried with it and rejected: 0 wipes on 42-49, Data sold 198.5. Still
   thin: a gang grudge has little to wipe while gangs keep no Data.
8. **ICE spend tracks portfolios, not robberies** (the gate's Spearman, a printed finding, 0.53 pooled over 20
   corp-seed pairs): Nutrix's 40-60 tier-1 nodes take the most spend at a mean ICE of 1.1-1.3, while Arasaka and
   Militech sit at 2.0 on seeded tier-2 Offices paying upkeep only. And **IceRaised** (8.0 a seed, spec ≥ 10)
   follows solvency: corps under their fleet reserve cannot buy.
9. **No hacker army flatlines.** Ten tier-3 decks against ICE ≤ 2 lose almost no contests, so the
   brutality section's "fries your brain" never meets them; nothing in the city escalates to ICE 3 on the
   Treasury (gap 1) to push back.
