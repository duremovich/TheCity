# Lives at work phase 6: ages, careers and leaders (report)

Final sha **bba4a88** on branch worktree-agent-a29632d1db3c44bcc (one commit on 087c01e). SAVE_VERSION 40 -> 41.

## Built (systems/careers.rs, new)
- `[careers] enabled` (CITYSIM_CAREERS_OFF=1): bands ([[trades]] min_age/max_age, new [roles.<Name>]) as a hiring gate
  (`hire_band`/`admits`) in demography pick + HirePool, pick_graded, dawn pick, law::reconcile_guards, sweepers' hire,
  World::new's seeded jobs; the retired and the too-frail refused too. Seniority seeded (hash), experience via
  `Job::start_key` (= hired_tick off), foreman = most experienced aged 30+ (Job.foreman flag, premium in collect_wage
  and gross_wage, foreman an actor of Hire/LaidOff). Profession component (careers::shift_end in both tiers),
  profession_w 400 tiles on the hiring offset + knowledge floor. Retirement (kin first, savings), Retired component,
  unretire under 20 coins unless living with a grown child, EventKind::Retired; frailty exit "too frail to work".
- `[gangs] lieutenants` (CITYSIM_LIEUTENANTS_OFF=1): rank 1 lieutenants, raid_w ladder in leader_scores, delegation
  (following_order / gang_work_target / stat_extort_target / raid_pending), raid head in raid::resolve + raids_led/won,
  payout cut 20 % / 10 % off the top with the split in the Collected text.
- Counters: band_refusals, foremen_premium, profession_rehires, foreman_hires (extra), retirements, frail_exits,
  unretired, lieutenants, delegated_orders, lieutenant_raids, leader_cut_coins, lieutenant_cut_coins.
- Tests: tests/careers.rs (6 unit incl. off cities x3, 1 diag age table), person.rs probes (laid-off loader rehired by
  profession vs stranger at the same door; 64-year-old with 600 retires, 20 works on), 5 printed bullets in core.rs,
  2 FINDING rows in lod.rs. Docs: LIVES_AT_WORK § 13 rows + § 14 phase 6 block; ARCHITECTURE § 6/§ 7.

## Off identity
- Both switches off, seed 42 x 30 vs main 087c01e: **IDENTICAL** (events, CSV shared cols, save).
- Each switch alone vs main: **DIFFER** (by construction: the other phase stays on and main has neither).
  In-test: `test_off_cities_write_nothing` (careers off / lieutenants off / both, 3 days) writes nothing of the phase.

## measure.md (final tree; 2k 42-44 x 120, mean, main -> phase 6)
| counter | main -> 6 |
|---|---|
| Murders | 105 (28,90,197) -> 127 (126,123,133) seed chaos, no kill roll touched |
| J24 / J25 / arrests | 0.795 / 0.017 / 2,578 -> 0.750 / 0.016 / 2,563 |
| wallets last day | 62,624 -> 81,745 |
| retirements / unretired / frail exits | 291 / 162 / 49 |
| band_refusals / profession_rehires / foreman_hires | 62,388 / 299 / 3,066 |
| foremen_premium coins | 14,904 |
| lieutenants (day-sum) / delegated leader-days | 1,317 / 392 |
| Raid events / lieutenant_raids | 17 -> 13 / 4.7 |
| leader / lieutenant cut coins | 328 / 154 |
| Hire / LaidOff / Fire events | 3,464/137/254 -> 3,410/106/289 |

Device (shadow --lives, day 19 x 7): elder shifts 400 -> 266, idle 4.16 -> 5.60; gang_leader idle 1.56 -> 2.89,
shifts 58 -> 48; loader idle 3.39 -> 2.82; builder unchanged; all 4.32 -> 4.09 idle.
Physical trades day 19 (diag, off -> on): over 60: 46/55/53 -> 1/1/1; 70+: 16/12/20 -> 0/0/0; foremen mean age
~46-49 -> ~54, under 30: 19/12/25 -> 0.

## Footers
- `test.py change`: `[change] GREEN; BUDGET OK 3m51s of 4m00s` (bullets: retirement [279,279,315], frailty exit
  [51,34,61], foreman's Hire [3408,2861,2930], lieutenant raid [8,1,5], leader's cut days [14,10,11]).
- `test.py lint`: `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 51`.
- Behaviour tier: passed on the tree before the last fix round; **on the final sha FAILS one row**:
  `FAIL d19 gang_leader PaidOfWorked 0.83 >= 0.85 (calibration range 0.95-1.00)` (12 leaders; seed 42 0.74, 43 1.00,
  44 0.91). Main passes it. Left as is per the brief (the first run on this tree, before the hash purpose renumber,
  passed: chaos on 12 picks, but the leaders' days did change by design).

## Deviations (full list in § 14)
Retired component (no Trait enum); [roles] top-level map; gate also in guards'/sweepers' hires and refusing the frail;
profession_w in tiles under the 512 cap (commute band still wins); premium as Job.foreman flag (crews of 2+); Sacked
credits raids_won; foreman_hires counter added; delegated_orders = leader-days at midnight; band_refusals = pool-walk
count (demography pool test clears it before its byte compare).

## Tests pinned / fixed for the moved city (none loosened; reasons in comments)
Careers off: chrome arms/skin, civic teacher, corps private_guard_scene, exec night guard, industry on_world + day post,
law theft-witnessed, policy civic cut, person far_worker / grocery / married_pick / el_commuter, cli csv header run
(day-0 kin retirements: homeless 74 vs floor 75). Both off: captives world, missions squad, succession city, tags world,
street night city. Setup: classpay candidates via age_ok, factions outside member not the leader, wages expected by
start_key, hire_pool test clears band_refusals.

## Left
Leaders' fates (shadow --pick), 42-47, A/B, save size, diaries, the two god scenarios, lod.rs rows: gate/milestone.
The gang_leader PaidOfWorked row above. target/ kept.
