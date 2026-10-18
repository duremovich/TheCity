# Phase 4a Murders fall: diagnosis

Verdict: variance in the faction-violence (fviolence) midnight pass, not a 4a path. No code changed. Final sha 09d4924 (docs only) on worktree-agent-a8967e7ffa1d29e9a.

Path
- The "assailant unknown" kills are not the hourly p_killed roll. On seed 44 base, 190 of 208 land at tick-of-day 0: fviolence's midnight pass calling stat_hit.
- That pass's rate per (source, district, class) is learned from body victims over body-hour exposure, so a small sample sets it.
- Seed 44: Order(Harvest) in district 7 (Sump East) read 3-5 kills over ~900 body-hours -> 0.035-0.057/day for every Statistical civilian there. Live 65 days on main-equivalent, 27 on 4a. Sump East = 165 of 208 base kills, 61 of 94 on 4a.
- fv_killed (3 seeds): 21/84/190 -> 9/50/87. Body kill_rate roughly equal (389/300/440 -> 280/340/440); only the stat side moved.

Evidence it is chaos, not one mechanism
- CITYSIM_LIVES_STREET_OFF=1 reproduces base (seed 44: 208; 4a: 94).
- Seed 44 single-key config bisect (off-screen kills): cab_shifts=[] 128; deal_corner off 94; p_short_report 0 104; client_tiles 0 181; pimp_tiles 0 94; p_corner 0 47. No consistent direction.
- client_tiles=0 on 42/43/45/46: 16/66/115/22 (base 38/102/124/52): no restore either.
- Fresh seeds 48-53, 120 d, OFF vs ON off-screen kills: 30/19/127/63/35/142 (416) vs 13/109/181/149/26/79 (557). ON higher in 4 of 6.

Changed
- Temporary debug print in fviolence.rs and config edits: reverted before commit.
- Added a paragraph to docs/LIVES_AT_WORK.md section 14, phase 4a block, with the above.
- Off identity not re-run (no code change). test.py change not run (docs only).
- Scratch outputs under worktree target/ab/ (delete with the worktree target/).
