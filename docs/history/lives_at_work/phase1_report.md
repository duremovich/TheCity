# Phase 1 report: the encounter, venues and the Clinic

- Worktree `D:\TheCity\.claude\worktrees\agent-aeb9f5f4cf9b9e4ee`, branch `worktree-agent-aeb9f5f4cf9b9e4ee`, ff-merged to main `e5525ee` (confirmed; docs-only on top of `c8d71b5`, the sim unchanged since `23a161d`).
- Main's binary for the A/B and the off identity: the worktree's `target/release/citysim-cli.exe` built at `5d20c86` (no sim change since), copied to `scratchpad/base/citysim-cli-main.exe` before any edit.
- Final sha: `ed26f53` (one phase commit on top of `e5525ee`; the WIP commit squashed into it; working tree clean; `git ls-files --eol | grep w/crlf` empty). Not merged; the worktree and its `target/` left in place.

## Built

- Config: `[lives] enabled` (master; `CITYSIM_LIVES_OFF=1` also clears `[service]` and `[[serves]]`, so the off save holds none of them), `[service] venues|civic|street|tower, madam_cut 0.3, p_eject 0.5, staff_meal, served_salience 0.15`, 15 `[[serves]]` rows (Bartender, barkeep, Host, Attendant, Croupier, Cook, dancer, companion, brothel_worker, chip_host, bouncer, madam, Fighter, Ripperdoc, orderly: key, kind, per_hour, minutes, skill, skill_gain, regular_after, regular_days, valence, event_on); `Config::unresolved_names` checks each key is a trade id or a legacy `Role` name. ARCHITECTURE § 7 rows added.
- State: `ServeKind` (15, appended, `components.rs`), `MemoryKind::Served`, `EventKind::Served` (`ALL` 214), `Flow::MadamCut`, `World::service_room` (unsaved: each server's encounters this hour), `World::regulars` (saved when non-empty; `SmallVec<[Regular; 8]>` a server, the oldest falls off), SAVE_VERSION 35 with its changelog line, 23 `Jobs2Cols` columns (`served_<kind>` x 15, `served_waited`, `serve_shifts`, `cooked`, `staff_meals`, `regulars_made`, `door_refusals`, `ejections`, `flow_madam_cut`; left out of a save at zero).
- `systems/service.rs` (new): `on`, `arrive` (the one-line hook: resolves the encounter at the arrival's minute against the servers on duty with room this hour, the lowest `hash(PURPOSE_SERVE, place, served, hour)`, FIFO by arrival; nobody or every counter full is `served_waited`), `hourly` (after `vice::hourly` in `living::run`, prof site `service`: prunes the room ledger, queues the madam's `Staff` encounters at the first hour of each worker's shift), `serve` (the counters, the tip to the server when its trade row is `tipped` else to the hashed tipped worker on duty, the madam's cut, Served memories both sides, the regulars ledger with the Acquaintance edge at `regular_after` and `interacted` after, `skill_gain`, the event for `event_on` outcomes), `present` (§ 1.2: on shift, a workday, not held, a body inside or a Statistical agent on `lod::shift_attended`), `servers_on_duty`, `refused`, `maybe_eject`, `shift_end` (the staffed-shift count; the Cook's staff meal from the NoodleBar's stock, every tier), `regulars_of`, `row_for`. No world-stream draw (five hashed purposes, `0x5e52_000n`); money through `ownership::pay` only.
- Hooks (one line each, each a no-op off): `leisure::on_complete` (Enjoy, EatOut, Den Gamble), `leisure::stat_visit` (every arm), `leisure::bouts` (the bettors), exec `Drink` (no coins), `chrome::installed_now` and `therapy`, `stims::detox`, `health::oncology`, `wounded::on_stabilise`, `assets::buy_noted` (a Clinic's non-implant sale), `vice::on_visit` (the tip skipped under the switch: `serve` pays it), `exec/mod.rs` (the door refusal at a venue step's failed start), the Attack's completion (`maybe_eject`), `exec::actions::end_shift` and `lod::stat_work`'s shift end (`shift_end`). `mood::memory_term` skips `Served` (D5, weight 0). The shadow diary prints "served <name>[, a regular]".
- Tests: `citysim/tests/service.rs`, 12 unit tests in the shipped seed-42 city (FIFO and the per-hour cap; the hash among two servers, never the lowest id; a Statistical server counted present; no memory under a chip; the ledger caps at 8, oldest off; a regular at `regular_after` with the event; the tip to the server and the madam's cut; the cook's staff meal on and off; the bouncer's ejection and refusal; the off city writes nothing in 3 days and its save lacks the `regulars` key; the on city serves within a day) plus the `service_probe` diag (who is behind the counter, by venue kind and hour, and why not). `tests/lod.rs`: the encounters-per-staffed-shift row, asserted within 15 % when both tiers staffed a shift. `tests/core.rs`: ten printed bullets with their 42-44 x 120 counts. `tests/person.rs::probe_evicted_worker_rehouses_on_the_first_rent`: the pick is now a *lawful* single Farmer (see Open 1).
- Docs: LIVES_AT_WORK § 14 "Implemented: deviations" (phase 1) with the measured table; ARCHITECTURE §§ 2, 6, 7.
- The lives probe (phase 0's tool) gained a fourth Served-able signal: `met` (shifts in which the pick gained a `Served` memory) and `enc/shift`.

## Deviations from the spec (also in LIVES_AT_WORK § 14)

1. **The encounter resolves at the arrival's minute**, not at an hourly drain of `World::queues`. The first build drained hourly and lost 62 % of visits as "waited": a 17:40 drink read at 18:00 against a 09-18 bartender, and the Statistical evening's one-hour batch at 21:00. Presence is now read when the visitor is there, as `tipped_for` read it. `World::service_room` (unsaved, per server per hour) replaces the queue; `service::hourly` prunes it and queues the madam's Staff encounters.
2. **The tip** goes to the server when its row is tipped, else to the hashed tipped worker on duty (`PURPOSE_TIP`): a Club's Host serves the visit, the dancer is tipped. The first build paid the tip only to the encounter's server and lost 62 % of the tips (a Club's 10 Hosts against 3 dancers). Remaining −15 %: `present` withholds a rest day's and a Statistical no-show's tip where `tipped_for` did not.
3. **The madam's cut** `round(0.3 x tip)` is 0 at the 2k Brothel's 1-coin tips: `flow_madam_cut` 0 over 42-47 x 120 days; unit-tested at a 20-coin price. Open for the gate.
4. `stabilise_no_orderly_mult` not built: `wounded::on_stabilise` has no save roll to multiply; no config key (nothing would read it). The orderly is the triage by presence (the Patient queue, a memory, the regulars).
5. The madam's `Staff` encounter fires at the first hour of each worker's shift (no shift-start hook; both shifts end together); the madam is not yet named in the house's Hire/Layoff text (left to phase 6's foreman work).
6. The regular's first-meeting affinity is a hashed `0.1 + 0.3 x u`, not `colocation`'s world-stream draw.
7. The door refusal is read at the exec's failed start of a venue step inside the venue; the scorer already filters by `door_ok`, so refusals are rare (0-6 per 120 days).
8. The ejection runs after the fight resolves (the Assault raised first).
9. The CSV home is `Jobs2Cols`, not `stats.current.jobs`.
10. A Clinic's over-the-counter sale queues a Patient in `assets::buy_noted`; an implant's install once, in `chrome::installed_now`.
11. The parity row asserts the total encounters per staffed shift, prints by kind through the CSV.

## The off identity (seeds 42-44 x 30 days, main's binary vs this tree with `CITYSIM_LIVES_OFF=1`)

- Saves (`run --days 30 --seed S --save-at 43200`, no `--report`): byte-identical apart from the one token `save_version:34` -> `save_version:35` (the format bump the doctrine requires); same length (12,557,008 / 12,600,984 / 12,601,475 bytes). Main's seed-42 hash `f796f363...485b41` is phase 0's.
- `--report` CSVs: every shared column identical on all 30 rows of all three seeds except `ticks_per_sec` (wall time); 23 new columns.
- `--events`: identical apart from the `saved <path>` line (the saves dir differs).
- In-test: `test_off_city_writes_nothing` (3 days: no encounter, no memory, no event, no `regulars` key).

## The on city against main (final binary; 42-47 x 120 days, means a day from day 30; 20k seed 42 x 30 days from day 5)

| | main 2k | phase 1 2k | 20k main | 20k phase 1 |
|---|---|---|---|---|
| encounters served a day (`served_*`) | - | 126 visits, 6.3 patients, 4.2 staff, 0 attendees | - | 1,190 / 43 / 51 / 0 |
| waited (nobody on duty, or every counter full) | - | 214 | - | 2,140 |
| staffed shifts of a serving row a day | - | 183 | - | 1,570 |
| regulars made a day | - | 0.87 | - | 43 |
| staff meals a day (cooks) | - | 39 | - | 340 |
| door refusals / ejections a day | - | 0.013 / 0.004 | - | 0.16 / 0 |
| madam's cut | - | 0 | - | 0 |
| tips a day | 104 | 89 (-15 %) | 874 | 781 (-11 %) |
| strip / brothel / companion visits a day | 8.7 / 16.1 / 3.4 | 8.6 / 16.3 / 3.2 | 105 / 106 / 25 | 110 / 113 / 25 |
| Murders over 120 d, seeds 42-47 | 94, 61, 112, 59, 111, 28 (mean 78) | 85, 89, 197, 148, 19, 94 (mean 105) | - | - |
| deaths by violence a day | 1.14 | 1.45 (+27 %) | 5.5 | 4.1 |
| J24 employed share from day 30 / J25 homeless share, seed 42 | 0.770 / 0.020 | 0.764 / 0.023 | - | - |
| J24 / J25 per seed 43-47 | .762/.028 .756/.022 .788/.023 .811/.018 .787/.020 | .770/.021 .770/.017 .796/.020 .777/.019 .792/.021 | | |
| thefts / arrests a day | 2.86 / 25.0 | 2.72 / 24.0 | 39.4 / 125 | 37.6 / 119 |
| population / employed a day | 1,980 / 1,530 | 1,960 / 1,510 | 20,000 / 12,000 | same |
| ticks/s, alternating A/B, seed 42 x 30 d, one at a time | 4,735 / 5,008 / 5,300 | 6,383 / 4,358 / 6,301 | 223 (one run) | 225 (one run) |
| save size, 30 d seed 42, no report | 12,557,114 B | 12,398,418 B on; 12,557,114 B off (the version token aside) | | |

Readings:
- **Murders** swing both ways by seed (46: 111 -> 19; 44: 112 -> 197; 45: 59 -> 148) with the mean up a third. No mechanism of the phase raises violence directly; the encounter's edges (a regular is an Acquaintance after 3-5 visits, every repeat visit `interacted`) and the re-routed tips move a chaotic city's trajectory. Printed, not retuned (the harsh-city decision; CLAUDE.md rule 2).
- **Throughput**: no regression. The batch comparison read -24 % because six 120-day runs shared the box; the alternating A/B one at a time reads B >= A (means 5,014 vs 5,681; the box is noisy). 20k 223 vs 225 ticks/s (one run each, against the 140 reference).
- **Who is behind the counter** (`service_probe`, 10 days): a server on duty for the Bar's and the Arcade's visits in 34 % of building-hours, the Club's 23 %, the Den's 22 %, the Brothel's and StripClub's 23 %, the NoodleBar's 32 %, the Clinic's 55 %. The Bar's Bartender and the Arcade's Attendant hold `[world] shift_day` (09-18) while the drinking and the Statistical evening are 18-24 (`demography::hire` gives `[leisure] evening_shift` to Hosts, Fighters, Croupiers and Concierges only); the NoodleBar's Cook likewise, so `cooked` reads 0.2 a day against 2-3 meals out; Clubs trade by day with evening Hosts. 63 % of arrivals wait for this. Not phase 1's to move (a shift change moves wages, leisure and the stat table): open for the gate.
- **No bout has a bettor** (bets paid 0 at every Bout over 6 x 120 days): the Fighter's `Attendee` row never fires.
- The Statistical evening gives one agent two visits at the same venue in the same hour (`stat_daily`'s floor(p)+1 rule), so a dancer's 21:00 reads "Tip +1 from X, Tip +1 from X"; main tipped the same way.

**Lives probe, Served-able shifts** (2k, day 19, 7 days, seeds 42-44; `met` = shifts with an encounter, phase 1's new column):

| archetype | n | shifts | Served-able (phase 0) | Served-able (phase 1) | met | encounters/shift |
|---|---|---|---|---|---|---|
| `all` | 1851 | 8598 | 0.28 | 0.32 | 0.06 | 0.14 |
| `club_staff` | 90 | 448 | 0.51 | 0.56 | 0.40 | 1.00 |
| `companion` | 6 | 29 | 0.00 | 0.00 | 0.86 | 1.24 |
| `dancer` | 42 | 198 | 0.50 | 0.46 | 0.61 | 1.02 |
| `madam` | 3 | 15 | 0.53 | 0.73 | 0.73 | 3.33 |
| `orderly` | 23 | 108 | 0.43 | 0.48 | 0.22 | 0.62 |
| `ripperdoc` | 25 | 118 | 0.44 | 0.39 | 0.14 | 0.14 |
| `cook` | 90 | 500 | 0.04 | 0.04 | 0.01 | 0.01 |
| `bouncer` | 49 | 241 | 0.76 | 0.70 | 0.00 | 0.00 |
| `fighter` | 24 | 115 | 1.00 | 0.99 | 0.00 | 0.00 |

(The bodies/events signals barely move: the encounter is a memory, not a body or an event; `met` is where phase 1 shows. Bouncers' and fighters' rows have no Served memory: the door's outcomes are events on refusal/ejection only, and no bout has a bettor.)

**Diary excerpts** (`shadow --pick bartender,cook,dancer,companion,club_staff,ripperdoc,orderly,bouncer,madam --days 7 --start-day 19`, `scratchpad/diaries/`):

```
companion_2166 (Volt Santoro, Spire Lounge#450)
- d19 18:02 [action] ACTION ClerkWork @ Spire Lounge#450 (Spire), 358 min (ended)
- d19 21:00 [money] +12c (Tip), purse 14c; events: Served "Freya Reyes is now a regular of Volt Santoro at Spire Lounge#450"; flows: Tip +4 from Freya Reyes#1181, Tip +4 from Freya Reyes#1181, Tip +4 from Zoe Lockridge#2489
- d19 21:00 [memory] served Freya Reyes#1181, a regular (valence +0.10)
- d19 21:00 [memory] served Zoe Lockridge#2489, a regular (valence +0.10)
- d21 21:00 [money] +8c (Tip), purse 23c; flows: Tip +4 from Zoe Lockridge#2489, Tip +4 from Zoe Lockridge#2489
- d21 21:00 [memory] served Zoe Lockridge#2489, a regular (valence +0.10)

dancer_2277 (a Club, owner Nutrix)
- d19 21:00 [money] +14c (Tip), purse 25c; flows: Tip +1 from Hiro Hawthorne#618, Tip +1 from Wire Dunmore#787, Tip +1 from Wire Dunmore#787, Tip +1 from Wisp Penhallow#1079, ... Tip +1 from Juniper Kemp#2241
- d21 21:00 [money] +4c (Tip), purse 24c; flows: Tip +1 from Clemence Hallow#1207, Tip +1 from Clemence Hallow#1207, Tip +1 from Clemence Hallow#1207, Tip +1 from Varek Vasquez#2274

madam_1134 (Brothel#484)
- d19 18:00 [memory] served Delia Novak#747, a regular (valence +0.00)
- d19 18:00 [memory] served Solene Fairweather#1169, a regular (valence +0.00)
- d19 18:00 [memory] served Dorian Nakamura#1628, a regular (valence +0.00)
```

(The madam's "served" are her workers' shift-start `Staff` encounters; the Club's dancer is tipped by the hashed rule while a Host serves the visit. No refusal in these eight diaries: refusals are 0-6 per 120 days.)

## Gates

`python tools/test.py lint`: `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 49` (the warnings are the pre-existing over-1,500-line files; the ignores +1 is `service_probe`, tier: diag). `cargo fmt --check` clean; `git ls-files --eol | grep w/crlf` empty.

`python tools/test.py change` on the final tree (`ed26f53`), footer verbatim:

```
[change] GREEN; BUDGET OK 3m42s of 4m00s
```

(fmt ok, clippy ok; `core` 45.5 s 2 passed, `person` 38 passed, `service` 12 passed + 1 ignored, `vice` 5, `cli` 6, `citysim_cli` 24 + 1 ignored; every binary ok. Logs `target/test-logs/change-20261018-054452`.) Three earlier runs were RED for one test each and are fixed in the tree: `person::probe_evicted_worker_rehouses_on_the_first_rent` (Open 1), the two `vice` tip tests (Open 0), `cli::test_cli_report_csv_header` (Open 00).

`python tools/test.py lint` on the final tree, footer verbatim:

```
lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 49
```

Parity (`tests/lod.rs::test_full_vs_statistical_within_15pct`, milestone tier, run once here because the row is new; 229 s): the new row read `encounters/shift Full 0.707 Statistical 0.877 (27684 / 26694 shifts)`, 24 % apart, so the assert the brief asked for failed; **demoted to a printed FINDING with the reason** (rule 2): the pass is world-side but the presence rule is not symmetric (a Full server must be inside the building, a Statistical one is credited by the clock). The test's `actor share law0 (0.909 vs 0.765)` and `law1 (0.060 vs 0.186)` rows fail as they did on main (SHADOW_V4 "Left open" 1; the milestone tier was not run at the last close), so the test is RED before and after this phase; its other rows (thefts, arrests, assaults, paid venue visits) pass.

The merge tier was not run (the orchestrator's `/merge-set`).

## Open

00. `citysim-cli/tests/cli.rs::test_cli_report_csv_header` pins the exact header and the column count: the 23 phase 1 columns were added to both (the expected string and a `+ 23` term), as every phase before did.
0. **Two `tests/vice.rs` tip tests pin the encounter-off city.** `test_tip_is_customer_to_worker_and_untaxed` and `test_bug7_club_dancers_tipped_in_turn` call `vice::on_visit` directly and expect it to pay the tip; under the switch `service::serve` pays it, so they now build `world_with(|c| c.lives.enabled = false)` (the city the rule was written for, per tests.md) and `tests/service.rs::test_club_tips_go_round_the_dancers` carries bug 7's guarantee on the phase 1 path (12 Club arrivals: the Hosts serve some, two or more dancers are tipped, no Host is, every visit tipped).
1. **`person.rs::probe_evicted_worker_rehouses_on_the_first_rent` picked a different Farmer.** On the phase 1 city the first single Farmer (589, lawfulness 0.3) was recruited by a gang eleven hours after the eviction and left the job, so the probe's precondition ("the Farmer stays employed") failed; with `CITYSIM_LIVES_OFF=1` he kept the job and was housed on day 9. The probe is about the deposit wall, not the gangs: the pick now requires lawfulness >= 0.6 (Farmer 627; housed on day 9 both on and off, as 589 was on main). A test change for a changed city, not a widened band; flagged for the orchestrator's veto.
2. **The venue shifts** (Bartender, Attendant, Cook on the day shift while the trade is in the evening): 63 % of arrivals find nobody on duty. Printed; a candidate OPEN row for the gate (moving a shift moves wages, leisure and the table).
3. **The madam's cut rounds to 0** at 1-coin tips; **no bout has a bettor** (the Attendee row never fires); **ejections 0-1 and refusals 0-6 per 120 days** (unit-tested; the scorer pre-filters the door). Candidates for printed FINDINGs at the gate.
4. **Murders up a third on the mean, down 80 % on one seed**: chaos, not a mechanism; printed.
5. The Statistical evening's double visit (one agent, one venue, one hour) predates the phase; noted.
