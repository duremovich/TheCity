# Phase 8d report (Lives at work gate 8d)

Branch HEAD bfac404 (phase commit 14c15ff + OPEN close bfac404), on base 698603c. SAVE_VERSION 45.
Off identity: CITYSIM_LIVES_OFF=1 vs 698603c, seed 42 x 30: IDENTICAL (save_version masked).

## Built
1. OPEN-123 madam's cut: tips at her own Brothel tally on `Job.shift_tips`; `service::shift_end` (both
   tiers: end_shift / lod::stat_work) takes madam_cut of the total once (`madam_settles`).
   flow_madam_cut 0/0/0 -> 376/335/178 (42-44 x 120). Device madam: Served-able 0.87 -> 0.67, met 0.67 -> 0.75.
2. OPEN-126 deposit first: `[service] move_in_rent_days = 7`; rehouse's worker branch needs 7 x rent and
   pays it to the owner (Flow::Rent, prepaid rent_due < 0); jobless keep rehouse_coins_mult; relocate
   keeps the prepaid credit (it wiped it the same midnight). J25 0.010 -> 0.013, homeless-days +20 %.
   Hired homeless housed within a night 30 % -> 24 % (mean days hire->housed 6.5 -> 5.9, events heuristic).
   Climb: s42/homeless_2367 hired d90 00:00, rough two nights, d92 Rent -7 to Stackwell, housed.
   Murders 91 -> 115 (66/101/178). First build charged every mover: 144; variants isolated it to the
   deposit (all keys off = base exactly 77/125/72). FINDING, not retuned.
3. OPEN-129 exec ride: `[service] exec_ride`; `streetwork::exec_ride` in exec::start_step GoTo. exec_rides
   21/25/12; 0 in corp vehicles. 20k travel_h/wk: ceo 25.30 -> 25.05, exec 20.43 -> 29.63 (picks changed;
   likely Hqs in the Spire district, not read). Statistical execs never walk (no mirror needed).
4. OPEN-130: not a double count. Lounge excess = one founded Lounge on Statistical seed 2002 (604 of 711
   status night outs; status_daily takes only owned Lounges); purses match; rest ~10 % excess every kind.
   law0/law1 -> FINDING (OPEN-038). New rows OPEN-132 (parity residual), OPEN-133 (widow probe).

## Parity (final tree)
paid venue visits Full 12.600 / Stat 14.594 (tol 1.89) RED; Lounge 0.193 / 0.349; law0 0.897/0.741,
law1 0.087/0.216 FINDINGs; law2 0.017/0.043 in.

## Footers (final tree)
[change] RED (tests); BUDGET OK 2m51s of 4m00s   (person.rs::probe_widow_commissions_a_hit_at_the_fixer)
[merge] GREEN; BUDGET OK 4m26s of 8m00s
[milestone] RED (parity); BUDGET OK 13m29s of 20m00s
lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 54

## Notes
- Widow probe: passes with move_in_rent_days 0; she spends to 22-27 c day 11, jailed d13.75. Not loosened.
- An earlier merge run read d90 homeless CommuteAbortsDay 0.14 > 0.07 (green on the final tree).
- I once killed every rustc/cargo process on the box while stopping my own probe build.
