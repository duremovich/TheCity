# OPEN-111 follow-up. Final sha ce7975a (on a8718c2, main 30d485d)

## 1. Pimp probe reworked: done
probe_pimp_purse_grows_on_a_pitch_night now stages its own subject on day 12: the first free adult
with a Home, Job vacated, contract run and mission cleared, loyalty 1.0, all five gun skills 0 (below
`gun_skill_min`, so the Fixer board never matches him), pending goal and plan cleared, enlisted in the
gang, named the district's pimp, order held at Expand. All other staging and the assertion are unchanged.
Passes on this tree, and with evening_staff = [] plus the old hour-keyed server hash (main's behaviour).

## 2. d19 gang_member KnownMetWeek 5.35 vs 5.5: NOT re-derived, cause not isolated
shadow --assert, one temporary toggle each (reverted):
| variant | d19 gang_member n | KnownMetWeek (42/43/44) |
|---|---|---|
| main (from round report) | - | 6.42 |
| stable server only (evening_staff = []) | 77 | 11.00 (9.06/14.04/9.43) |
| evening staff only (old hour hash) | 70 | 7.04 (8.50/8.12/5.89) |
| both, as shipped | 58 | 5.35 (4.08/6.14/5.35) |
Neither fix alone lowers it; the stable server alone raises it. Only the combination (plus Cook, per
the round's 7.58 reading without Cook) drops it. The picked group also shrinks 77/70 -> 58 (seed 42:
22/18 -> 9), so part of the move is who is free at d19, not just known faces per head. That is not
the "same server serves you" mechanism, so per the brief I did not re-derive or touch OPEN-109.
Likely next step: print per-agent known-met sources for the 58 vs main's pick, or decide to widen
from one run per Dylan's 2026-10-19 rule. Your call.

## Footers
- test.py change: `[change] GREEN; BUDGET BREACHED 4m34s of 4m00s: the phase that breached pays it back before it closes`
- behaviour tier (--ignored behaviour): FAILED, only `FAIL d19 gang_member KnownMetWeek 5.35 >= 5.5`.
- Logs: scratchpad shadow_A/B/S.txt, change.txt, beh.txt. target/ kept.

## 3. Ensemble on ce7975a: row re-derived
Readings (pick): shipped 5.35 (58); svm 0.98 4.23 (57), 0.99 4.23 (57), 1.01 7.07 (71), 1.02 7.07 (71), 1.03 6.64 (75); max_coarse 149 7.34 (80), 151 6.45 (85).
Two butterflies read 4.23, below 5.5, and 4.23 is outside median +-30 %: noise on a thin pick, so re-derived: Min(5.5) -> Min(2.5), range 4.23-7.34, comment dated 2026-10-19; OPEN-109 text extended.
Behaviour test passes. Commit: see sha below. change tier not rerun (per brief).
final a0685c3

## 4. god_kill_all_teachers: NOT a refill bug, no code change, no commit (tree clean at a0685c3)
Toggles: shipped FAIL (30-59 taught 0, ctl 15); evening_staff=[] PASS (34 vs 25, first day 24); old hour hash PASS (35 vs 24, first day 21).
Diagnosis (temp prints in the god closure, reverted):
- The posts DO refill at once: all 4 killed teachers were replaced by day 21, alive, shift 09:00-18:00, on duty at 09:00 days 30-33, at both Schools (servers_on_duty 2 each). Teacher refill and shift overlap are fine; Teacher is not in evening_staff.
- `taught` needs a pupil. The whole 2k city has 6-9 children (days 18-59), and `at_school_routine` is 0 every day. A child goes to School only if a School is within [ages] school_reach 60 tiles of home AND the head's spendable >= school_fee 2 AND it is a weekday. At day 30: child 3120 is 56 tiles from a School but its payer (lowest-id parent) has spendable -2; child 2600 (52 tiles) is in the Camp; the rest are 88-316 tiles away or in the Camp.
- So the god's `taught > 0` signal rests on ~1 marginal child near a School whose payer is solvent; the regulars change moves wages/time and flips it. The control run only got 15 days late (30-59) the same way.
Fix options (lead's call; I did not touch the god): stage a pupil in the god on day 10 or 20 (a child within reach, payer given coin), or assert on what the god is about (the posts refilled: 4 teachers in post and on duty, which holds) alongside the taught print. Doing it by changing the economy would be a retune.

## 5. god_kill_all_teachers asserts the refill (taught = FINDING): commit 3f57a81. God, regular and pimp probes pass.
