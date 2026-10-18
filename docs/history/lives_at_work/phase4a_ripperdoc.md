# 4a ripperdoc PaidOfWorked / WagedWorkdays: no mechanism found, world-chaos variance. No commit.

Worktree D:\TheCity\.claude\worktrees\agent-a8967e7ffa1d29e9a, HEAD 09d4924 unchanged, tree clean (config.toml restored after each toggle).

## What the rows are
d19 ripperdoc, seeds 42-44 pooled (shadow --assert), PaidOfWorked / WagedWorkdays, n = sampled ripperdocs:
- 4a on:                    0.73 / 0.69  (n18; per seed 0.65/0.61, 0.77/0.72, 0.81/0.81)
- CITYSIM_LIVES_STREET_OFF: 0.89 / 0.85  (n21)
- 4a on, cab_shifts = []:   0.86 / 0.83  (n19)
The unpaid shifts are two small clinics (Ripperdoc#515 owned by Rin Wozniak, #546 by Ilse Kemp): the owner's purse is short, so
collect_wage pays 0 and writes the Unpaid memory. Zetatech's clinic (#453, 11k treasury) pays every shift in every variant.

## Why this is not a mechanism
1. The worlds diverge from day 0. First event difference on seed 42 is tick 588: a Full commuter's cab arrives a minute earlier, so a
   Fare Evasion witness line moves 589 -> 588. Everything after is chaos. By d10 Rin Wozniak's life differs completely
   (on: purse 0, pays wages from nothing, Noodle Bar d16; ns: founds a Ripperdoc d10, 231c by d19).
2. cab_shifts is not the cause either; it looked like one on seeds 42-44 only. Quits "(unpaid)" by Ripperdoc+Orderly, 26 days:
   - seeds 42-44: on 28, off 9, ns 12 (looks like a signal)
   - seeds 45-50: on 36, off 46, ns 47 (on is the lowest; no signal)
   Pooled over all 9 seeds: on 64, off 55, ns 59. No effect of the street switch or of cab_shifts on clinic payroll.
3. Clinic revenue is unchanged: AssetBought at Ripperdoc#: on 426/425/412, off 414/430/406, ns 431/426/407 (seeds 42/43/44).
4. Nothing in the 4a diff touches clinic income or the payer: full_cab is commute-only (target == job.employer), row_of_agent only adds a
   pitch-trade fallback for Job-less agents, lift_hour/parcels_back are Yard/courier only.

So the pooled d19 ripperdoc sample (18 agents, effectively 3 owners) is the variance, the same story as the Murders row in the 4a
block (seeds 48-53 go the other way). Same kind of fragile cell as the gang_leader rows already noted in shadow.rs CHECKS comments.

## Decision for the lead (I did not make it)
- Doctrine forbids lowering the floor or retuning to pass. Options: (a) merge with the two rows recorded as a FINDING-class variance with
  this evidence (the gates should assert broken-system signals; a pooled n=18 / 3-owner rate is a calibration band by doctrine 1);
  (b) demote the two ripperdoc rows to printed FINDINGs in CHECKS in a separate, explicit commit; (c) re-pin by changing nothing
  and re-running on a different BEHAVIOUR_SEEDS set (not done; changes the tier).
- I did not run the 4a street/person tests, the off-identity script or test.py change: no code changed, so they are unchanged from the
  coder's footers.

Artifacts under target/: bh_on, bh_off, bh_ns (diaries), ev_{on,off,ns}_{42..50}.tsv (event streams), run_{on,off}.csv.
