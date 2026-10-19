# Phase 6 fix round (branch worktree-agent-a29632d1db3c44bcc)
Final sha: see `git log -1` (one commit on top of bba4a88). Files: citysim-cli/src/shadow.rs, docs/LIVES_AT_WORK.md (sec 14 block).

1. d19 gang_leader PaidOfWorked 0.83 (main 0.98): measurement artefact, not a pay bug.
   - 6 of 7 unpaid shifts: one seed-42 leader (Hiro Achebe) owns Fixer#531 and holds its post. Owner-operator pays staff, no wage to self (phase 5 design).
   - Main has the same case (seed 43, Juniper Park) but they are arrested d19 and jailed all week, so `free` drops the shifts.
   - Both switches off on the phase-6 binary reads 0.98. Leader/lieutenant cuts land. The 7th unpaid shift: leader hired d21, arrested mid-shift d22.
   - Fix: ShiftObs.own_post; shifts at an owned building skipped in paid/waged metrics. No band moved. Row now 0.97, behaviour test green.
2. J24 0.794 -> 0.750 (employed share d30+, 42/43/44): base 0.795/0.802/0.784, cand 0.758/0.776/0.715.
   - Plus retired stock (retire - unretire, over adults, deaths ignored): 0.818/0.827/0.778. Confirmed: seeds 42, 43 above main, 44 within 0.006. Frailty exits (51/34/61) are extra jobless elders.
3. Wallets last day 62.6k -> 81.7k: retired elders' idle coins, not a coin source.
   - coin_identity constant per run (seed 44 differs from main by 14 coins as a run constant; E24 gate green). All 3 seeds above every earlier main (55-70k); top10 share 0.53 -> 0.60-0.71.
   - Savings retirees 13/22/57 (~400 coins each; seed 44's 28k gap = its 57). 266/257/258 retire to a child's Home keeping their wallet.
   - Foremen premium 14.9k coins total, cut flows unmoved, wages -2 %. Design (rich elder, no pension); money supply per head is the thing to watch. Not fixed.
Checks: behaviour test ok (60 s); phase_measure OFF IDENTITY: IDENTICAL; test.py change GREEN 3m51s of 4m00s; fmt/clippy clean.
Logs: scratchpad fix_*.log, candruns/, q*.py.
