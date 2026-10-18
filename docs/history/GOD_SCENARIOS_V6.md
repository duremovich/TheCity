# God scenarios v6: the word and the blood

The sixth run of the god-scenario doctrine (`docs/VISION.md`, "How we test: god scenarios"), aimed at
M15: deeds as records passed between agents (rumours, district pools, Feeds), reputation rebuilt from what
is known, grudges, vendettas, the Hunt, social skills and moves, and propaganda. Every killing, beating,
hunt and story below is an abstract event between fictional agents of a simulated city: a seeded dice
roll over structs and counters. Five scenarios in `citysim/tests/god.rs`, each against its own unshocked
control (`v6_control`):

```
cargo test --release -p citysim --test god -- --ignored --nocapture god_kill_friend_of_leaders_day_10 god_vendetta_arasaka_hollow god_plant_zetatech_exec god_suited_dregs god_purist_ninefold
```

About a minute for the five (plus the control) on parallel threads. Measured on M15 phase 5 with its
final calibration (`docs/M15_WORD_AND_BLOOD.md`, "Implemented: deviations").

## How the suite works

- **Seed 42, default config, 105 days**, the shock at the start of its day (day 10 for the leaders'
  friends, day 30 for the plant, day 45 otherwise), observed against the unshocked control day by day.
  Each prints a table of sums over windows (violence, Murders, grudges formed, Hunts, Avenged, revenge
  kills, stories, plants, buries, poachings, expulsions, contracts lost on honour), every ten days each
  gang's order (with Retaliate's target), dread, heat and size, the law's posture, Crackdown target and
  Lobby hold, the open vendettas beside the control's, and the word's story (vendettas, Hunts, Avenged,
  plants, buries, paid stories and killings run by a Feed, poachings, expulsions, contracts lost,
  Retaliate and Lobby orders, corp raids, bribes, postures). The tests assert only that the god
  commands applied; a failure to react is a gap below, not a red test.
- **Commands** (`PlayerCommand`, logged and replayed; a `PlayerAction` event prefixed `God:`), all from
  M15 phases 1-3: `KillFriend { of, by }` (`kill_friend=leader0:member1`), `DeclareVendetta { a, b,
  weight }` (`declare_vendetta=corp<slot>:gang0:1.0`; phase 5: the feud is now held open for `[grudges]
  declared_days` 30, since at `vendetta_norm` 10 the two leaders' grudges alone no longer open it),
  `PlantRumour { about, deed, object, district, reach }`, `GrantSkill { agent, skill, value, suit }`
  (`grant_skill=dregs10:persuasion:1.0:suit`), `SetCreed { gang, creed }` (`set_creed=1:purist`).

## The scenarios

Sums over the windows; "shock / ctl" against the control over the same days.

### `god_kill_friend_of_leaders_day_10`: each gang leader's closest Friend killed by the rival's best fighter

| days 10-40 / ctl | violence | Murders | grudges | Hunts | Avenged | stories | plants |
| --- | --- | --- | --- | --- | --- | --- | --- |
| shock | 461 | 11 | 1,040 | 8 | 2 | 183 | 3 |
| control | 448 | 6 | 801 | 3 | 3 | 193 | 13 |

- On day 10 The Hollow is one man, its leader Wire Abernathy, so `member0` is the leader himself: the
  first command has Ninefold's Dorian Dunmore kill Wire's friend, the second has Wire kill Dorian (who was
  Ninefold leader Seraphine Singh's friend). The city is very young (Ninefold 5, The Unplugged 5).
- **Reacted.** Ninefold goes Expand → **Retaliate** the same day (0.74 vs 0.42) and the law's
  **Crackdown turns on The Hollow on day 14** (the control's first Crackdown is on Ninefold, day 16): the
  law reads the avenger's gang. Murders nearly double over the next month (11 vs 6), grudges +30 %,
  Hunts 8 vs 3.
- **No vendetta between the two gangs for 60 days**: the first opened is Ninefold-Rust Saints (day 35);
  The Hollow-Ninefold opens on day 70 (the control's by day 89). Two kin-of-a-friend grudges are far
  from `vendetta_open` at `vendetta_norm` 10, and The Hollow has one member to hold them.
- Neither leader hunts the killer of his friend: the friend's grudge on the leader reads `friend × (0.5
  + 0.5 × affinity) × conf` ≤ 0.6, under `hunt_min` 0.75, and the killers die or the Hunt never wins a
  slot; Wire Abernathy's only Hunt (day 70) is on someone else.
- Plants collapse (3 vs 13 in days 10-40): the shocked run's corps hold fewer known rival misdeeds.

### `god_vendetta_arasaka_hollow`: a 1.0 vendetta between Arasaka and The Hollow on day 45

| days 45-75 / ctl | violence | Murders | grudges | Hunts | Avenged | Buried |
| --- | --- | --- | --- | --- | --- | --- |
| shock | 924 | 13 | 1,366 | 21 | 4 | 9 |
| control | 719 | 15 | 1,066 | 13 | 8 | 13 |

- **Reacted.** The feud opens at once and stays open for the run (60 days: the 30 declared, then the
  members' own grudges hold it). The Hollow takes Retaliate against Arasaka and **raids Arasaka's
  Security Office on day 52 and wins** (5 raiders against 2 defenders, 500 coins). Arasaka goes
  VirtRaid → **Lobby** on day 49 and offers the captain 46 coins for a Crackdown on The Hollow (refused,
  the captain's lawfulness); the law cracks down on The Hollow by day 69 on its own (heat 0.83).
- The Hollow grows from 21 to 46 members by day 59 and opens feuds with Rust Saints and Ninefold; violence
  +29 % over the month.
- Arasaka plants "The Hollow robbed Joss Yardley" on the Civic Wire the day of the declaration (its people
  knew a Hollow misdeed; a vendetta gang is a plant target).
- Arasaka's Lobby hold never takes (every bribe refused), so "does Lobby name The Hollow" is yes in the
  offer and no in the law.

### `god_plant_zetatech_exec`: "Zetatech's exec killed a Sump Central child", reach 1.0, day 30

- The plant: "Juno Ito killed Omar Oyelaran" (a child of a Sump Central Home). Both Feeds run it the same
  night ("Nutrix Now: Juno Ito killed Omar Oyelaran", "Civic Wire: ...").
- **Employees' opinion sags**: Zetatech staff's mean opinion of Zetatech 0.594 vs 0.607 on day 30, 0.531
  vs 0.555 on day 33, 0.545 vs 0.567 on day 36, back to the control by day 38: −0.013 to −0.025 for a week.
- **Zetatech's honour does not move** (0.50 / 0.49 in both runs) and its dread stays 0: a killing has no
  `honour_w` (only betrayal, robbery, stripping, extortion, eviction and poaching cost honour), and the
  deed names the exec, not the corp; the corp's honour takes 0.3 of its exec's honour, which a killing
  does not touch. **No Spin bury**: Spin's `honour drop` term reads 0, so Zetatech never spins.
- No Security contract moves (Zetatech buys none on seed 42), so the honour-weighted renewal has nothing
  to read.
- The run is otherwise identical to the control (violence, Murders and grudges equal to the count).

### `god_suited_dregs`: persuasion 1.0 and a Spire suit for the ten poorest Dreg adults on day 45

| day | employed (ctl) | edges to Corp-class agents (ctl) | mean standing (ctl) |
| --- | --- | --- | --- |
| 45 | 0 (0) | 32 (32) | 0.17 (0.08) |
| 52 | 0 (0) | 43 (43) | 0.17 (0.07) |
| 75 | 1 (0) | 42 (36) | 0.15 (0.04) |
| 104 | 1 (0) | 74 (36) | 0.19 (0.05) |

- **Reacted, slowly.** The suit lifts their standing at once (dress is part of standing's position);
  their edges to Corp-class agents double by day 104 (74 vs 36: first impressions read the suit's taste,
  and the meeting gate lets a Persuade through); one of the ten has a job by day 75.
- **None is poached**: poaching moves employees between corps, and nine of the ten never get a job to be
  poached from. The two poachings after day 45 are the control's (Dorian Nakamura, Nutrix ↔ Greenline).

### `god_purist_ninefold`: Ninefold takes the Purist creed on day 45

| days 45-75 / ctl | violence | grudges | Hunts | Expelled |
| --- | --- | --- | --- | --- |
| shock | 875 | 1,557 | 15 | 9 |
| control | 719 | 1,066 | 13 | 3 |

- **Reacted.** Ninefold casts out its chromed members at once (Nikolai Varley and Nyx Park on day 45,
  two more by day 54): members 23 → 18 by day 52 (control 21). Violence +22 % and grudges +46 % over the
  month; Ninefold and The Unplugged (both Purist) open a vendetta by day 59, absent in the control.
- **Sump chrome falls**: adults housed in a Sump district with visible chrome 10 vs 17 on day 75, 25 vs
  29 on day 104 (of ~925); mean `Kit.visible` 0.011 vs 0.021. The Sump is barely chromed on seed 42 (4
  of 921 on day 44), so the effect is small in absolute terms.
- **Nobody moves out**: the Sump's housed adults stay ~921-930 in both runs; no Home choice reads a
  creed.

## Gaps

1. **A killing does not touch honour.** `honour_w` has no `killed`, and a corp's honour reads its exec's
   at 0.3: a planted killing by the exec moves neither, so Spin's `honour drop` stays 0 and the corp
   never buries the story (`god_plant_zetatech_exec`). Either a killing by an exec costs the employer's
   honour, or Spin reads stories about its exec directly.
2. **Friends do not hunt.** A Friend's grudge on a killing is ≤ 0.6 (`friend × (0.5 + 0.5 affinity) ×
   conf`), under `hunt_min` 0.75: the spec's "the clerk's friend forms a grudge, asks around and kills"
   needs two wrongs merged. Revenge kills stay 1-2 a run (band 3-20). `rel_w.friend` or a lower
   `hunt_min` for a killing is the knob; the phase 3 spiral (826 Hunts at `hunt_min` 0.5) is the risk.
3. **Two killings between gangs open no vendetta.** At `vendetta_norm` 10 a feud needs ~10 grudge
   weights in total; the leaders' friends' killings opened none for 60 days. Good for rarity, slow for a
   god shock between small gangs.
4. **Lobby never lands.** Every corp bribe for a Crackdown was refused on seed 42 (the captain's
   lawfulness), so a corp in a vendetta names the gang but never moves the law.
5. **The poor are never poached.** Poaching only moves the employed; a skilled Dreg needs a job first,
   and nothing hires on persuasion (`god_suited_dregs`).
6. **The chromed do not leave a Purist turf.** Nothing in Home choice or relocation reads a creed or a
   gang's taste (`god_purist_ninefold`).
7. **Security contracts are too few for honour to bite.** 30-50 bought a run, most cancelled by the
   buyer's Hunker; `ContractLost` on honour stays 0 on seeds 42-44 even with honour moving on contracts
   and acquisitions (phase 5).
