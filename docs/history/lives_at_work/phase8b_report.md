# Phase 8b report: the gate's tests

Branch on 464358a (ff-only from main confirmed). Logs: `<worktree>/target/p8b/`.

## 1. Swap lists (for the orchestrator's veto; applied in the commit)

Census: new `core.rs::core_census` (tier: diag), 42-47 x 120 on 464358a. Every Lives candidate 6/6
except door refusal 1/6, madam's cut 1/6, cab fare by a named driver 4/6, street client near 5/6,
lieutenant-led raid 5/6. `lint`: asserted bullets 60/60.

**Bullets in (12, all 6/6):** regular made, Served patient (P1) · HangOut at home with a spouse,
Visit to a hurt kin (P2) · School day taught, super's door payment (P3) · parcel to a named
recipient (P4a) · dealer's regular (P4b) · Meeting with attendees, byline story (P5) · retirement,
leader's cut (P6). Phase 7's two are `core_20k` asserts already (outside the 60).
**Bullets out (12 -> printed; unit test; sibling still asserted):**
School day (School day taught replaces it; civic.rs taught-day test) · a scan (tags.rs) · a Trauma
subscription (wounded.rs) · a debt called in (leverage.rs::test_debt_called_settles_the_edge) ·
a Guard fulfilled (contracts.rs guard + fulfilled tests) · a Fixer in business (a state;
test_npc_registers_a_fixer) · conversion (gray_faith.rs) · tip (day count; service.rs tip test) ·
handling fee (day count; industry.rs) · Jailer on shift (day count; civic.rs jailer meal) · a day
with scrip paid (day count; weapons.rs scrip split) · a malnourished agent (day count; sickness.rs).
Full table with counts: LIVES_AT_WORK.md § 15 "The gate's tests".

**Gods in (5):** god_kill_all_teachers, god_child_camp_sweep, god_decapitate_lieutenants,
god_retire_everyone_over_55 (god.rs, merge tier); god_cut_the_precinct (new god_20k.rs, tier:
milestone; a `god_20k` step added to test.py's milestone tier; 40 days, two 20k cities, 392 s).
**Gods out (5), each reaction carried by a unit test (none of the OPEN-096..099 kind):**
god_decapitate_close_succession (succession.rs::test_close_succession_is_a_civil_war) ·
god_hostage_captain_spouse_release (captives.rs::test_ransom_paid_frees_the_captive,
governance.rs::test_release_demand_and_the_captain) · god_call_vote_habitat_after_coerce
(governance.rs::test_pinned_member_swings_the_vote, test_vote_coercion_breaks_with_the_board) ·
god_power_cut (industry.rs::test_outage_darkens_districts_and_comes_back) · god_flood_guns
(arms.rs::test_landed_crate_fills_the_armoury_and_the_muster_draws_it). god 20/20.
TESTING.md class table and GOD_SCENARIOS_V12 "Retired 2026-10-19" updated.

## 2. God readings (seed 42)
- teachers: 4 killed d20; taught d20 0, d21-29 7 (ctl 7), d30-59 7 (33); taught again d21
- camp sweep: 0 children on the street at d10 (fallback: poorest households'); 2 taken; parents'
  answer d11-59 11 (ctl 4). Thin at 2k (few children).
- lieutenants: 11 killed d30; lieutenants again d31 (the ranking re-runs at the death)
- retire 55+: 322 workers at 79 employers; 323 retired d10; 182 Hires there d11-20; foremen moved
  60/79; elders taken in d10-19 69 (ctl 41)
- precinct (20k): Stations city 208 Precinct guards jailed (230 at Stations), arrests 10-19 1,167 ->
  20-29 1,528 (x1.31), bookings 779, transfers 179; no-station city 398 jailed, 1,189 -> 1,427 (x1.20).
  **Finding: arrests rise in both** - a 10-day sentence costs the job (law::sentence keeps a job
  only to 3 days), `reconcile_guards` hires replacements at midnight, the force never thins. The
  spec's "arrests fall less" holds only as "rise more". Not a test fix; for 8c's read.

## 3. The probe
`person.rs::probe_regular_at_one_bar_meets_its_bartender`: **gap: OPEN-111 (new row in OPEN.md;
renumber if it clashes)**. Three drinks at the Bar nearest his Home (from 10:00: the Bar's bartenders
hold shift_day 09-18, the evening drink is unserved) go to three different bartenders of five
(the server is a fresh per-hour hash, D6; the ledger is the server's): no edge. Behaviour fix, not 8b's.

## 4. analyze_run.py "Lives at work" section (42 x 120, this tree) - pasted in § 15
encounters/staffed shift visit 0.43, patient 0.023, tenant 3.35, crate 1.44, attendee 0.81; 1,697 a
day, 10.1 % waited; regulars 2.2/day; home share of HangOuts 39.5 %, friend's 9.6 %, spouse 21.9 % of
home ones; spouse evenings 104/wk per 1,000 adults; empty spots 114/day (33.9 %); visits hurt kin 632,
elder 3,921, Camp 21; buy-outs 0, break-outs 3; retirements 249, frailty 23, unretired 119;
lieutenants 11.4, 6 lieutenant-led raids, leader's cut 392; execs 15/15 at tier; 19.6 arrests/day.

## 5. The change tier's budget (paid back by making tests cheaper; nothing ignored or deleted)
Before (464358a, cold clippy 45 s): 4m56s; orchestrator's 4m25s. Changes and binary before -> after:
- citysim Cargo.toml `[lib] doctest = false` (no doc examples): doctests 2.9 s -> 0
- person: `world()` built once (OnceLock) and cloned; super probe's two cities, the buy-out
  probe's control and the sweeper's two cities on threads; the starving probe's 60 runs over six
  threads; the widow's city to the pick and `quiet_buyer` built once for their two probes each: 16.0 -> 13.8
- abroad: promotion's four cities and the draw's three runs on threads: 7.9 -> 2.8
- succession: `city()` and `hollow_city()` built once; the brothers test's three cities on threads: 7.8 -> 4.5
- policy: dry city and control on threads: 6.2 -> 4.3
- citysim lib: lazy-relocation test's two maps on threads: 5.2 -> 3.8
- careers, stations: the three off cities on threads: 2.3 -> 0.8, 2.2 -> 0.7
- bind: the Full/Statistical cities on threads: 3.5 -> 3.1
Tried and reverted: a weapons test shortened to the death clock (narrowed a window: reverted);
captives/governance/parents threads (no gain); **leverage/captives/governance/tags `gang_world`
as a clone instead of a save round trip: `leverage.rs::test_save_mid_coercion_round_trips` FAILED**
(a clone of the in-memory city, saved mid-coercion and loaded, diverges from the clone in a day:
some unsaved state steers the sim, so a save does not resume byte for byte from a live world;
masked today because the base is itself a loaded world). Reverted; **a finding for an OPEN row
(OPEN-???, doctrine 8)**, not diagnosed past that.

## 6. Footers (final tree, commit 2a045b7)
- `test.py change`: `[change] GREEN; BUDGET OK 3m29s of 4m00s` (fmt 2.4 s, clippy 0.7 s warm; core 40.4,
  person 13.7, weapons 9.7, wages 9.5)
- `test.py merge`: `[merge] GREEN; BUDGET OK 4m16s of 8m00s` (core_year 115.4, behaviour 58.0, god 49.3 at
  16 tests, god_corps 16.8, god_districts 16.6)
- `god_20k` (milestone, run alone): ok, 391.99 s
- `test.py lint`: 0 problems, 6 warnings (file lengths); god 20/20, asserted bullets 60/60, ignores 54

## 7. For the orchestrator
- OPEN-111 taken for the probe's gap (renumber if 8c/close clashes). OPEN-??? needed: the save of a
  live world does not resume byte for byte (§ 5), and the precinct cut never thins the force (§ 2).
- One squashed commit 2a045b7 on the branch; target/ kept.
