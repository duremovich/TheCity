# Gate 8a probe fix (OPEN-108): seven person probes restaged

Fix commit 69c46f3, OPEN-108 closed in 30444de. Worktree agent-a0f1add04306eb48f. Only citysim/tests/person.rs and docs/OPEN.md changed.
No assertion was touched. No real behaviour change found: in all seven the premise was left to chance by the setup.
All 47 person tests pass on the new table AND on fd89cca's old table (swapped in, tested, swapped back, never committed).

Per probe (cause -> staging):
1. hungry_worker_eats_from_the_wage: pick was tired (a farmer), went to Sleep at 19:00 with hunger 0.15 and 6 coins -> energy set 0.9 at 15:00.
2. gun_never_takes_a_hit_on_kin: control gun took the Hit, was arrested, then bled out on day 3 (run_to panics on a dead subject) -> control is read the moment the gun holds the record (loop breaks on `took`); the kin run still runs the full window under run_to.
3. one_grocery_run_to_nearest_detour_market: a posted Visit (kin hurt/arrested, household.rs) won an hour into the run, plan restarted, two starts -> `w.visits.remove(&a)` each tick, next to the existing energy/hunger/fun top-up.
4. stocks_the_pantry_off_shift: fun 0.10, the pick hung out (Unwind/HangOut) all evening -> fun 0.95 and the Visit cleared at staging.
5. suspect_buys_a_beat_on_a_known_witness: the law arrested the suspect asleep at home 90 min after the report (a patrol sighting), before the midnight silence pass could send him to the Fixer -> `remove_last_seen` each tick (wanted, never sighted); loop replaces run_to for the main run only.
6. pimp_purse_grows_on_a_pitch_night: the pimp was the pitch's own client at midnight (street_intimacy raised to 1.0 makes any body near the pitch a client): paid 5 in, took 3 of a doubled take, passed 2 up, net 0. Found by instrumenting purse_add (instrumentation reverted) -> pimp intimacy held past `street_intimacy` each tick in the existing `calm` closure. The earlier hire/move of the pimp at midnight was a red herring (reproduced the failure with it, not the cause).
7. worker_sleeps_the_night_before_a_shift: two of twenty farmers spent the evening at a far Socialise/Unwind, walked ~4 h home, slept past 09:00 -> in each clone fun/belonging/intimacy topped to 0.9 at hourly marks 18:00-22:00 (hunger and energy untouched; mean energy 0.74, was 0.79).

Footer (python tools/test.py change, once, on 69c46f3):
[change] GREEN; BUDGET BREACHED 4m25s of 4m00s: the phase that breached pays it back before it closes
Lint: 0 problems, 6 warnings (person.rs now 3117 lines, > 1500 cap warning, pre-existing).

Final sha: 30444de
