# Lives at work phase 5: the tower and the brokers (report)

Final sha **7198b00** on branch `worktree-agent-adc6f5a86217b1eb9` (one commit on f5160ee, not merged). SAVE_VERSION 40.
Switch `[service] tower` / `CITYSIM_TOWER_OFF=1` (under `[lives]`). target/ kept.

## Built
- Config: `[service]` tower keys (`meeting_attendees` 6, `decision_margin` 0.05, `expansive`, `defensive`, `beat_stops` 4,
  `byline_promote` 10, `witness_tiles` 3 new); `[[serves]]` rows `exec`, `Reporter`, `Fixer`, `forger`, `civic_clerk`
  (`tower = true`); `Config::strip_tower` (keys, rows, `decided` deed keys); `Deed::Decided` (ALL 23).
- Meeting (`service::meeting_hour`, `World.meeting_tables` saved); decision (`service::decide`/`decided` from
  `corp_brain::rescore`); reporter's beat (`news::beat_plan`, `stat_beats`, `World.beats`, time-gated), byline
  (`Story.byline`, `World.bylines`, a grade step every 10); Fixer: order paid at midnight as main, pickup as a walk-in
  (`World.walk_ins`, `VisitWhy::Papers`), contract encounters (`contract_party`, `fixer_book`), owner's weekly
  collection (`VisitWhy::Collect`); civic clerk named on licences and bribes.
- 14 Jobs2Cols counters (incl. `walk_ins_dropped`). `tests/tower.rs` (6 units), 2 probes in person.rs (CEO meets staff;
  reporter at the scene; each with a tower-off control), 4 printed core bullets, lod.rs papers row.
  `shadow --lives` now counts an exec's office hours as his shift (`ShiftObs.office`, kept out of behaviour metrics).
- Docs: LIVES_AT_WORK § 14 phase 5 + § 13 after-rows, ARCHITECTURE § 6/§ 7, OPEN-102.

## Measure (tools/phase_measure.py, final tree, m_final/measure.md)
Off identity seed 42 x 30: **IDENTICAL** (events, 990 shared CSV columns, save with version masked).

| counter (42-44 x 120 mean) | main | phase 5 |
|---|---|---|
| Murders | 83 (91, 80, 77) | 105 (28, 90, 197) — seed 44 off-screen storm; fresh seeds 45-50 off vs on: 695 vs 521 (variance) |
| J24 / J25 / arrests | 0.768 / 0.019 / 2,514 | 0.795 / 0.017 / 2,578 |
| CorpOrder events | 92 | 110; decisions 30 |
| Meeting hours / attendees | — | 5,146 / 24,874 |
| beat stops / interviews / bylines / promotions | — | 571 / 252 / 927 of 1,135 stories / 2.3 |
| papers fake + bribed | 88 + 22 = 110 | 80 + 31 = 111 |
| pickups served / waited / dropped | — | 56 / 11 / 25 |
| contract encounters / owner collections | — | 322 / 1 |
| licences sold / named; bribes named | 12 / — | 14 / 14; 31 |

| device (shadow --lives) | n | shifts | met | enc/shift |
|---|---|---|---|---|
| ceo/exec main / tower-off same probe / phase 5 | 29/29/27 | 9/138/133 | 0.22/0.29/**0.86** | 0.33/0.30/**30** |
| reporter main / 5 | 12/12 | 69/66 | 0/**0.77** | 0/1.26 |
| forger main / 5 | 3/3 | 9/16 | 0.33/0.75 | 0.33/1.00 |
| fixer main / 5 | 3/3 | 13/17 | 0.38/0.41 | 0.38/0.41 |
| all | 1855/1860 | 8937/8994 | 0.38/0.46 | 2.14/3.45 |

## Footers
- `test.py change`: `[change] GREEN; BUDGET OK 3m43s of 4m00s`
- `test.py lint`: `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 51`
- Behaviour tier (run once on the final code): **FAIL d19 homeless KnownMetWeek 5.16 >= 6.4** (main 7.00, already under
  its 10.81-12.54 calibration; a `meeting_attendees=5` butterfly of this tree reads 8.64 and the whole tier passes).
  Chaos on a row at its floor; NOT widened — the merge tier will show it; orchestrator's call. Reporter rows pass.

## Deviations (full text § 14)
- Exec row key `exec` (no Role::Exec; accepted by unresolved_names); Fixer row is legacy `Fixer` role (owner via encounter_post).
- Meeting table saved (not unsaved) so a resumed save names the table.
- Beat outside round_plan with own state; scene = building (victim's Home / object / actor's Home); **time gate** (leg +
  walk back must fit before ROUND_CUTOFF; the first build walked 11.8 h/day and was paid 7 % of workdays).
- **Papers paid at midnight, pickup at the walk-in** (charging at pickup lost half the sales: buyers at 38-40 coins
  spent below the 40 price by morning). Dropped pickups (jailed/dead) keep the forger's coins.
- Contract encounters off the Fixer's hours (posts at midnight, takes at 18:00) are credited to the Fixer's book.
- Licence clerk named by hash among the Hall's clerks (sale runs at midnight, nobody on shift).
- Decision applies to daily and shock rescores; "keep" outcome gets a Served event without a CorpOrder event.

## Tests touched outside the phase (diverged city)
- `succession::test_bug8_…` pinned with the tower off (as phase 2 pinned the household).
- `probe_buyer_nobody_heard_of_is_never_charged` stops at the buyer's death (abducted + ripped day 23).
- `probe_pimp_purse_grows_on_a_pitch_night` `#[ignore = "gap: OPEN-102"]` (new row: another pimp eats, musters for a
  Raid, unwinds; 0 Collect ticks; passes on main and tower-off).
- cli.rs header +14 columns; captives/news/ownership Story literals gain `byline: None`.

## Left / findings
- A corp's own Hq, once built, stands unstaffed and hq_of ranks it first: that corp's Meetings stop (bug 5's ranking).
- Fixer owner collections are rare (most owners hold a post); forger/fixer device rows are 2-3 agents (noise).
- The gate's set (42-47, A/B ticks/s, save size, diaries, lod papers row at milestone tier) not run here.
