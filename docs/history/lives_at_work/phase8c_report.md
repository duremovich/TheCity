# Phase 8c report: the read and the close

The base was e566e29, confirmed after `git merge --ff-only main`. The branch has one commit, **97e7cef**. Nothing is merged to main, and target/ is kept.

## Done
- **Shadow V5.** V4's recipe gave 250 diaries: k2_s43_d26 80, k2_s44_d90 78 and k20_s42_d20 92. Seven Sonnet critics read them in V4's groups. The write-up is docs/SHADOW_V5.md, and the per-archetype critiques with excerpts are in docs/shadow_v5/*.md. `shadow --assert` gave 133 PASS and 0 FAIL. I re-calibrated no bound, because none fails. The moved rows and their reasons are listed in SHADOW_V5 § The devices.
- **OPEN-113 is fixed.** `World.service_room` is now saved (Room is Serialize/Deserialize, skipped when empty) and SAVE_VERSION is 44. The new test `save.rs::test_mid_hour_save_keeps_the_service_rooms` failed on base (no rooms after the load) and passes now. Off identity against e566e29 with `CITYSIM_LIVES_OFF` on seed 42 × 30 is IDENTICAL. With every switch on, the 42-44 × 120 counters are identical to main.
- **Change tier.** On a quiet, warm box it reads 3m36s against the 4m00s budget. The 4m20s on main was box noise; my first run here read 7m32s while clippy rebuilt alongside the critics. I made `weapons` cheaper: 10.0 s → 7.1 s (`test_rich_failing_parent_lives` now uses death_days 7 over 20 days, and also asserts the failing streak passed death_days). I left the other slow binaries alone. `core` (41 s) already runs its seeds on threads, `person` (14 s) is seeded probes whose picks would move, and `wages` (9 s) already shares a OnceLock base.
- **OPEN-105.** This came from a scratch diag that I did not commit. On seed 42 (43) × 120, retirees hold 5.0 % of all coins on day 10 and 1.6 % (1.3 %) on day 120. On day 120 there is 1 retiree (seed 43: 0) in the richest tenth. Retirees spend their coins down and unretire. The rising top-tenth share (0.44 → 0.56) does not come from retirees.
- **Docs.**
  - LIVES_AT_WORK § 13: the closing device table (phase 0 vs final, 2k and 20k).
  - § 14: "Open at the close", with `OPEN-???` placeholders.
  - § 15: "The read and the close", one line per gate item, plus the tiers.
  - § 5.1: the gate row.
  - ARCHITECTURE § 7: the switch list.
  - TESTING: the wall times.
  - README: a SHADOW_V5 line.
  - OPEN-113 is marked closed.

## Shadow V5 bugs (21; the top five)
1. Bar day drinkers get no bartender since OPEN-111 moved bar staff to the evening. 73 of 100 Bar visits in the vice diaries happen before 18:00. This is the likely cause of the parity RED below.
2. A body never drives the cab. There are 0 fares in 6 cabbie diaries, and on seed 42 × 120 a body drove 1 of 92 rides.
3. The Visit goal beats Work (1.08 vs 0.99) but cannot plan. It shows in 26 of 250 diaries as 312 "cooled (could not plan)" lines, and each one interrupts the shift.
4. The home evening counts only bodies: a Statistical spouse is never home. 73 % of the married picks in the lives probe never name their spouse.
5. Leaders collect but do not lead. 38 gang diaries show 0 raids or lieutenant actions; the delegation happens only off screen.

The other 16 are in SHADOW_V5. Among them: settlements happen at midnight before the encounter that narrates them, the pimp collects from a sleeping worker, an exec with no Hq meets at a shop (OPEN-104's family), the street worker never gets a client, bouts are one fixed pair of kin, 20k walks are long, a fare-evasion arrest loop, and the shadow tool's labels hide the cab, the parcel and the Camp.

## Gate items that need a decision from Dylan
- **The madam's cut** is 0 in 120 days. Should it have a 1 c floor, or are named staff with no cut the design?
- **The 2k School** has 90 pupil-days in 120 days across two Schools. Should schooling matter at 2k (ease the distance or fee), or is the teacher a one-family counter?
- **`god_cut_the_precinct`.** Jailed guards are replaced at midnight by reconcile_guards up to the funded `guard_count`, so the shock is not a cut. Should the god cut `levers.guard_count` instead, or keep the jailing and assert only Station bookings and transfers? This test is RED, see Footers.
- **The frailty exit** fires near 74, so couriers aged 64-73 still work 57-62 h a week.
- **The foreman** hires homeless people and houses them in one night.
- **Whether the Purist creed covers decks.**
- From the critics: should raids be delegated to lieutenants, and should a CEO whose Hq is in Mid live in the Spire?

Settled: a tied super serving notice on her spouse (not seen), re-taken children (too rare to read), the 20k pimp's night, vendor regulars, OPEN-103's diary read (the homeless have a thin but real web), OPEN-104 for corps that have an Hq, OPEN-105 (close the row), and guards at 2k. OPEN-110 is settled for coins; keep the row for the "served" label.

## OPEN placeholders (`OPEN-???`, for the orchestrator to number)
- The Bar's day drinkers, together with the parity row `paid venue visits`.
- The cab never driven by a body.
- The Visit goal that cannot plan.
- The Statistical spouse in the home evening.
- Leaders that do not lead.
- Settlement at midnight before the encounter.
- The pimp's collection built from the link rather than presence.
- `door_refusals` 0 (the bouncer has no door).
- Bouts.

OPEN-104 and OPEN-110 stay open. Close OPEN-105 and OPEN-113.

## Switches for OPEN-094 (9)
LIVES, HOUSEHOLD, CIVIC, LIVES_STREET, TOWER, CAREERS, LIEUTENANTS, STATIONS and SPIRE_HOMES, each as `CITYSIM_*_OFF`.

## Footers (final tree)
- `[change] GREEN; BUDGET OK 3m36s of 4m00s`
- `[merge] GREEN; BUDGET OK 4m26s of 8m00s` (core_year 116 s, behaviour 63 s, god 50 s, god_corps 18 s, god_districts 18 s)
- `[milestone] RED (parity, god_20k); BUDGET OK 13m46s of 20m00s`. Both RED rows are main's own readings, because the on city is byte-identical to main.
  - Parity is RED on `paid venue visits` (Full 12.33 vs Statistical 15.09, tolerance 1.85; it was in at 8a) and on `law1` (0.131 vs 0.211, as on main). `law0` now passes.
  - `god_cut_the_precinct` is RED: 1.17 vs 1.25.
  - Neither test was loosened.
- `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 54`
- `test.py report`: change green 3m36s, merge green 4m26s, milestone RED 13m46s; every cap met.
