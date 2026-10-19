# Phase 8a report: the gate's numbers

Branch on fd89cca (ff-only from main confirmed). Milestone start 23a161d. Logs: `<worktree>/target/p8/`.
Code-identical cached bases (git diff on citysim, citysim-cli, assets, Cargo.lock empty):
c1d2c2a = dc91503 (pre-4a), d019df2 (pre-5), 087c01e = 2c3c902 (pre-6), 1810392 = d98bcd3 (pre-7).

## The milestone, 42-47 x 120 (23a161d -> fd89cca, switches on, main's table)
Murders 94/61/112/59/111/28 -> 133/136/140/121/79/49, mean 78 -> 110 (+42%); J24 0.779 -> 0.768;
J25 0.022 -> 0.014; arrests 2,620 -> 2,641; wallets 63,465 -> 79,731; ticks/s 3,979 -> 3,739 (print).

## The regen (calibrate v2, seeds 1000-1002 x 30 d, 2k, pathed): KEPT
Table diff, key by key, split in two by a second `calibrate` run on 23a161d's binary from its own
source root (`stat_table_23a.toml`): main's table -> 23a161d regen = the drift before the milestone
(+ calibration noise); 23a161d regen -> fd89cca regen = the milestone. Means over rows; full diff in
`target/p8/tdiff*.txt`.

| key | main table | 23a161d regen | fd89cca regen (kept) |
|---|---|---|---|
| p_chat_home (D14) | 0.767 | 0.782 | 0.740 (-5 %) |
| p_chat (D14) | 0.00215 | 0.00241 | 0.00184 (-24 %) |
| p_social (D14) | 0.00139 | 0.00142 | 0.00100 (-29 %) |
| p_theft_caught (D14) | 0.661 | 0.623 | 0.601 (-3.5 %) |
| p_meet | 0.0502 | 0.0480 | 0.0403 (-16 %) |
| p_flirt | 0.00473 | 0.00463 | 0.00918 (+98 %) |
| p_steal | 4.32e-5 | 5.09e-5 | 3.62e-5 (-29 %) |
| p_beg / p_beg_short | 0.0656 / 0.0268 | 0.0678 / 0.0264 | 0.0150 / 0.0053 (-78 / -80 %) |
| p_scavenge / _short | 0.223 / 0.085 | 0.225 / 0.087 | 0.251 / 0.097 (+12 %) |
| p_desperate | 0.00265 | 0.00221 | 0.00327 (+48 %) |
| p_dodge_caught | 0.116 | 0.099 | 0.117 (+18 %) |
| p_pitch | 0.635 | 0.697 | 0.622 (-11 %) |
| pantry_per_resident | 0.233 | 0.242 | 0.277 (+14 %) |
| p_meal (80) | 0.0661 | 0.0646 | 0.0716 (+11 %) |
| p_venue Club / Den | 0.0365 / 0.0566 | 0.0337 / 0.0534 | 0.0252 / 0.0365 (-25 / -32 %) |
| p_venue StripClub / FightPit / NoodleBar | .00465/.00866/.00417 | .00508/.00783/.00599 | .00653/.00909/.00517 |
| p_assaulted / p_robbed / p_killed | 4.3e-5/1.06e-5/2.9e-6 | 3.3e-5/1.19e-5/2.6e-6 | 3.1e-5/1.41e-5/2.8e-6 |
| p_eat / p_sleep / p_work | .0947/.262/.138 | .0935/.267/.139 | .0907/.259/.137 |
| p_attend (147 roles) | | | 134 moved; >10 %: super +28..40 % (the round's stops), Reporter -21 % (the beat) |

Findings (keys the expected four do not explain): **p_beg -78 %** (the household's kin take-in, reunite
and the super's tied housing cut the homeless: J25 0.022 -> 0.014 in Full; fewer beggars per agent-hour);
**p_flirt +98 %** (no mechanism names flirting; the home evening puts spouses and housemates in one room,
the likeliest cause, not proven); p_steal -29 % and p_meet -16 % (the evening at home, the paid venue
visits); Club/Den -25/-32 % (phase 1's door refusals and ejections). p_theft_caught moves little because
the 2k calibration city has no Station (`station_min_tiles`): the stations' rate is 20k-only and the
table is learned at 2k (a finding for the 20k parity, not fixable by this regen).

## After set, 42-47 x 120 (fd89cca main table -> regen table)
| | 42 | 43 | 44 | 45 | 46 | 47 | mean |
|---|---|---|---|---|---|---|---|
| Murders | 133->46 | 136->144 | 140->151 | 121->90 | 79->61 | 49->80 | 110->95 (-13 %) |
Thefts 320 -> 284, arrests 2,641 -> 2,418 (-8 %), J24 0.768 -> 0.778, J25 0.014 -> 0.011, wallets
79,731 -> 104,122 (+31 %: the Statistical tier steals and begs less, p_steal -29 %, p_beg -78 %), ticks/s
3,977 -> 3,995 (print).
**Murders check:** the noise is the seed spread: per seed 49-140 on the before tree (28-112 on
23a161d), and the 42-44 vs 45-47 means of the one before set are 136 vs 83. The regen's mean moves
-15 (110 -> 95), inside that spread; per seed 46-151 stays inside it (seed 42's 133 -> 46 is the storm,
seed 47 moves the other way 49 -> 80). **Kept** (Q8).

## Parity (`lod.rs::test_full_vs_statistical_within_15pct`, regen tree; main's table in brackets)
thefts 0.080/0.083 [0.117]; hunger-days 1.238/1.323 (0.50) [1.745 RED] now in; arrests 0.652/0.307
[0.370]; assaults 0.101/0.123; violent deaths 0.005/0.011; marriages 1.085/0.845; bin meals
0.244/0.341 [0.503]; **malnutrition x100 12.483/9.693 (1.87) RED** [11.580 in]; commute rides
33.89/34.19; ride share 35.24/36.13; **paid venue visits 12.82/15.42 (1.92) RED** [16.84];
**paid venue coins 56.34/69.44 (8.45) RED** [73.20]; **law0 0.906/0.710 RED**, **law1 0.071/0.228 RED**
(red on main, stay printed); law2 0.023/0.062 in. FINDINGs: synth % 32.7/38.4 [40.1], encounters/shift
6.37/6.99, door rent share 0.256/0.260, papers 0.228/0.161, retirements 2.04/1.91, Station bookings 0
(2k), cab rides 0, stims 0, lieutenant raids 0 (none in the parity city).
- **paid venue visits/coins: demoted to FINDINGs** (lod.rs, OPEN-106). Diagnosed: `calibrate` learns
  `p_venue` from every paid Full visit including `classpay::status_daily`'s night out (world-side,
  both tiers), and the Statistical evening pays that rate on top of it: double count off screen
  (Lounge 0.47 vs Full 0.20, table 0.195). Bar 1.52 vs 0.82 (table 1.03) is a second path, not found.
- **malnutrition x100: new RED, left asserted** (OPEN-107). The regen fixed hunger-days (a midnight
  snapshot) by feeding the Statistical tier more (p_meal +11 %, p_scavenge +12 %, pantry +14 %);
  malnutrition integrates hungry hours over the day (`health::daily`), and drops below Full.

## Off identity, 42-44 x 30 (main's table in place; a sub-switch with all later switches, vs its pre-phase sha)
master vs 23a161d; HOUSEHOLD+later vs 593e8cf; CIVIC+later vs 596cd8c; LIVES_STREET+later vs c1d2c2a(=dc91503);
TOWER+later vs d019df2; CAREERS+LIEUTENANTS+p7 vs 087c01e(=2c3c902); STATIONS+SPIRE_HOMES vs 1810392(=d98bcd3):
**21/21 IDENTICAL** (events, CSV shared columns, save with version masked). A sub-switch alone with later
phases on has no base to match (not run). Logs `target/p8/off_*/measure.md`.

## A/B (alternating, one process, 2k seed 42 x 30, 3 rounds)
23a161d 7,713/7,737/7,666 ticks/s (csv mean) vs final 7,336/7,400/7,356: -4.4 % (wall -2.8 %). Save 30 d
seed 42: 12,557,278 -> 13,006,413 B (+3.6 %).

## 20k close (seed 42 x 30; 23a161d -> final; [final tree with main's table])
Murders 198 -> 104 [189]; arrests 3,350 -> 3,366 [3,880]; thefts 1,008 -> 786 [940]; deaths by violence
251 -> 162 [244] (off screen 188 -> 91 [183]); Station bookings 1,733 [2,011], transfers 440, stations_full
198, precinct_full 0; execs 107, 102 in the Spire. Seed 43: Murders 53 -> 51 [54], thefts 1,046 -> 774.
Read: the seed-42 drop is the regen not lighting the Sump 2 storm; [main's table] reproduces phase 7 exactly.
law_probe_20k bands, arrested (reported): 0.76 (0.65), 0.77 (0.57), 0.79 (0.57), 0.80 (0.55); all 0.77 (0.60),
6,533 acts, no UNPUNISHED (phase 7: 0.75/0.70/0.74/0.73). core_20k: in the milestone footer.

## Footers (final tree c590aaf)
- `test.py change`: **RED (tests)**, BUDGET OK 3m28s of 4m00s. fmt ok, clippy ok, every binary ok but
  `person` 40 passed / 7 failed. All 7 pass on fd89cca's table (47/47, rerun with the table swapped):
  the regen diverges the seeded worlds' Statistical ambient and the probes' picks change (OPEN-108,
  spec § 15 "What the regen costs" lists them). Not fixed: 8b's (tests), or revert da9e3f3 alone.
- `test.py milestone`: RED (parity: malnutrition, law0, law1), BUDGET OK 7m51s of 20m. core_20k ok
  (Murders 90, J24 0.663, stations 8, bookings 1,663, transfers 429, Precinct full 0, Spire 103/109).
- `test.py lint`: 0 problems, 6 warnings (file lengths); god 20/20, bullets 60/60, ignores 52.

## Commits
da9e3f3 the regen alone (revertible); c590aaf lod.rs demotion, OPEN-106/107/108, spec § 15.
Two commits, not one squashed: phase8_brief's rule for a kept regen (its own commit) overrides.
target/ kept. Logs under the worktree's target/p8/.
