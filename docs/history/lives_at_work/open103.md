# OPEN-103 (widened): KnownMetWeek decline on the margins

Verdict: DESIGN, not a bug. No code changed; tree stays at 30444de. No bound touched.

## Bisect (shadow --assert, seeds 42-44 pooled, known/wk; hang/wk in brackets)
Binaries from target/measure caches; the 8a row is this tree.

| sha (phase)        | d19 fighter   | d19 homeless  | d90 homeless  |
|--------------------|---------------|---------------|---------------|
| 23a161d base       | 6.16 (21.7)   | 9.02 (18.8)   | 6.90 (15.4)   |
| 593e8cf p1         | 5.55 (23.9)   | 8.76 (18.6)   | 6.16 (12.5)   |
| 596cd8c p2         | 3.46 (16.2)   | 8.31 (17.1)   | 5.59 (13.0)   |
| c1d2c2a p3         | 3.85 (19.5)   | 7.03 (15.1)   | 5.33 (13.4)   |
| 4da38cb p4a        | 3.00 (17.3)   | 7.32 (14.4)   | 6.86 (15.9)   |
| d019df2 p4b        | 3.40 (17.1)   | 7.00 (16.3)   | 5.40 (11.9)   |
| 087c01e p5         | 3.30 (18.2)   | 5.16 (15.5)   | 4.86 (11.4)   |
| 1810392 p6         | 3.55 (16.9)   | 5.00 (12.8)   | 5.29 (13.2)   |
| fd89cca p7         | 2.85 (19.0)   | 5.65 (14.8)   | 4.56 (11.7)   |
| 30444de 8a (regen) | 2.55 (18.2)   | 4.47 (16.2)   | 3.73 (11.9)   |

Phase 0 is probes only and has no cached binary, so it was skipped. The homeless n is small (17-54 at d19), so single steps of about 1 are noise. Rare-mechanism knockouts below swing the fighter row by about 1.

Fighter: the step is phase 2 (5.55 -> 3.46, HangOuts/wk 23.9 -> 16.2). Homeless: a diffuse slide (p2, p3, p5), then the regen.

## Knockouts on this tree's binary (same three rows)
| variant                                | fighter | hl d19 | hl d90 |
|----------------------------------------|---------|--------|--------|
| as is                                  | 2.55    | 4.47   | 3.73   |
| CITYSIM_LIVES_OFF=1                    | 5.15    | 7.91   | 6.02   |
| CITYSIM_HOUSEHOLD_OFF=1 (phase 2 off)  | 6.10    | 6.73   | 5.77   |
| own Home spot removed (diag patch)     | 4.05    | 9.00   | 4.98   |
| friends' Homes off (visit_hosts 0)     | 2.35    | 5.48   | 4.50   |
| empty leave off (empty_leave_ticks 6e4)| 3.25    | 5.08   | 3.82   |
| reunite off (diag)                     | 2.25    | 7.21   | 4.65   |
| visits off (diag)                      | 3.90    | 4.81   | 3.87   |
| fd89cca stat_table (pre-regen)         | 2.85    | 5.65   | 4.56 (= p7 exactly) |
| pre-regen table + own Home removed     | 4.84    | 8.33   | 4.43   |

## Mechanism
1. **Phase 2's own-Home HangOut spot (household.rs home_spots, leisure.rs best_spot_kind).** The own Home is offered at any distance when company is due. It scores the household term plus the contacts' habit and kin terms, so it outbids street corners.
   - Diaries (seed 42, d19 x 7, 8 fighters): 82 of 141 HangOuts at the own Block, 75 of them with a flatmate present. Phase 1 fighters had 0 of 168 at home.
   - Time moves from corners, where a rotating cast passes, to the same one or two flatmates. KnownMetWeek counts *distinct* known contacts, so it falls although company per HangOut rose (empty 92/168 -> 7/82 at home).
   - The homeless have no Home. Their housed contacts now spend that time indoors, so the corners they use are emptier. Their HangOuts read 57 % empty on this tree.
2. **The 8a regen** learned that quieter on-screen city into the Statistical rates (p_meet -16 %, p_social -28 %). Fewer off-screen meets means fewer edges, so fewer of the faces at a spot are "known". It is the same cause echoed one level down. With the pre-regen table this tree reproduces phase 7's rows exactly.
3. **d90 homeless** is the least attributable. Removing the own Home recovers only part of it, and household-off recovers most. The rest is a slide across phases 1, 3 and 5 that sits inside the noise of a single step.

## Bug candidates checked, none found
- A Home HangOut's company is counted by the metric through residents_inside. Home means a shared flat of up to six by design.
- The empty leave and company-only window do not explain it, since turning empty leave off does not restore the rows.
- Friends' Homes do not explain it. Reunite and visits move within noise.
- No path excludes the homeless from spots. A homeless agent is offered corners and friends' Homes as before.
- The metric counts HangOut company only, so phase 1's Served encounters were never in it. The phase 1 step (6.16 -> 5.55) is within noise.

## Decision for the orchestrator and Dylan
The rows measure "distinct known faces at HangOuts", and phase 2 deliberately moved free time home. Options: re-derive the d19 fighter and d90 homeless floors from the post-milestone calibration (as 7fe1bef did for d19 homeless), or add a feature that gets marginal people back among faces (for example a flatmate who brings the HangOut outdoors, or homeless spots by the housed's routes). I changed no bound and no code.

Footers: no code change, so no test.py change, no off identity rerun. This tree's behaviour tier: 2 FAIL (d19 fighter 2.55 < 2.8, d90 homeless 3.73 < 3.9).
Final sha: 30444de (unchanged). Outputs: D:\TheCity\target\bisect103\*.txt (diag binaries deleted; no worktrees added).
