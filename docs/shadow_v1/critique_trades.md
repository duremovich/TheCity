# Critique: ripperdoc_762 and runner_1242 (seed 42, d20-d26 diary)

Headline: neither agent shows their trade. The ripperdoc is a clerk on a short wage who treats nobody. The "runner" is a Guard chasing stale warrants, and the deck is never touched.

## Yorick Wozniak, ripperdoc (Street, 41, wage 8/day at Clinic in Sump West, home in Mid West)
A day (d21): 00:12 leaves the bar, 124 min walk to the Clinic. 02:16 collects 7c, then five 90-min Rests at the Clinic until 09:06. 09:06-18:00 ClerkWork as one 534-min action with zero customers. 18:08 walks 36 min home, eats 16 min, Rests 90 min four times until midnight. Net +15c. d20 was 14 h idle in a bar after one chat. d22-d26 repeat: a 9 h shift, "Quit (unpaid)" at 18:00, "Hire" at 00:00. Wallet 13-42c all week. No patient, part, stim or gang customer ever appears.

## Yorick Pembroke, runner (Street, 36, hacking 0.70, greed 0.93, deck T1, Guard at Precinct, shifts 21:00-06:00)
A day (d21): 03:08 Arrest goal on Hesper Haas, 3.5 h walk, plan fails PreconditionLost 06:43. 06:44 next suspect, fails 08:11. 10:12 and 12:12 new Arrest plans, fail 13:34 and 14:58. 17:12 Odessa Stroud, fails 18:12, retried at once. Sleep 3.6 h, travel 19.8 h, work 0.3 h, net -1c. Wallet 0-17c all week. Hacking and greed never matter.

## Findings

1. **Ripperdoc never practises.** What: ClerkWork is the only job action; 5 shifts, 0 customers, no install, therapy, detox or sell-back, parts 0. Why: the provider side of the M13 chrome economy is invisible. Cause: goap/actions.rs `ActionKind::allowed` and the role map (line ~236) turn Role::Ripperdoc into ClerkWork; the Clinic actions (D34/D39) exist only as customer actions under Treat/Shop goals. Severity: missing feature. Fix: a Clinic counter action that waits for a customer and then performs the install or treatment as the provider, paying a cut. Wage becomes a floor, fees the income.

2. **Wage-chase loop eats the day.** What: d20 05:17 "Work" scores 0.947 while off shift (shift is 09:00-18:00), walks 36 min, takes 4c, walks home. d21 00:12 walks 124 min from the bar, arrives 02:16, Rests five times until the shift opens. Why: 3-6 h a day of travel for a few coins. Cause: the Work score uses "wages owed" and is not gated by `on_shift` (the flag at goap/actions.rs:779 gates only the ClerkWork action); `Job::wage_collectable` (components.rs:1319) needs only days_unpaid >= 1. Severity: bug. Fix: gate the wages-owed term of Work by shift or shift-end, and do not travel more than a short hop for under 10c.

3. **Quit and rehire churn.** What: "Quit (unpaid)" at 18:00 d23-d26, "Hire" at 00:00 next day, four cycles. Why: no consequence, no new employer, noise in the event log. The owner is Ilse Kemp, a +1.00 friend. Cause: systems/economy.rs `maybe_quit` fires on three Unpaid memories in 7 days; the vacancy rehires the same person. Severity: bug. Fix: after a quit, a 5-7 day cooldown before that employer can rehire that agent; the quitter looks for other work meanwhile.

4. **The profession pays below subsistence and has no revenue.** What: wage 8c is paid as 4-7c; food costs 9-12c; wealth need stays 0.15. Cause: wages come from a person-owner purse, no fee income exists (follows 1). Severity: missing feature. Fix: price list per install from M13 chrome values, revenue split between owner and ripperdoc, wage paid out of revenue.

5. **12-14 h of Rest.** What: 50 Rest actions of 90 min. "Earn unplannable (no feasible action)" repeats every 2 h on d20 and the goal is cooled each time. Why: the diary reads like a statue. Cause: Idle constant 0.05 wins when Earn finds no plan; a Street-class 41-year-old has no Earn candidate. Severity: shallow. Fix: give Earn a fallback of odd jobs, busking or the profession action at their own Clinic, even off shift.

6. **GetHigh never completes.** What: scored 0.085 five times with "1-lawfulness" climbing 0.22 to 0.61 as unpaid shifts erode lawfulness, then interrupted by Work or by Idle before BuyStims. Why: the stiffed professional sliding to stims is a good story arc, and it never lands. Cause: plan abandons on any higher goal; long GoTo (162 min to Bar#417) is not resumed. Severity: shallow. Fix: allow a nearby stim source for the plan; resume interrupted travel.

7. **No relationships from the trade.** What: 44 edges, one friend (the owner), no client edges. Why: a ripperdoc's network is patients, gangs, fixers, suppliers. Severity: missing feature. Fix: every treatment creates a Client memory and edge; gang-reserved implants (D34) tie the ripperdoc to the gang boss.

8. **Runner is not a runner.** What: zero Virt goals out of 29; the only deck mention is the kit line. Why: hacking 0.70 and greed 0.93 are unused. Cause: the run goal needs a held RunOrder (goap/actions.rs near line 479) and the Guard job plus Arrest at 0.53-0.88 outranks everything. Severity: missing feature. Fix: runner archetype with no precinct job and a fixer contact that hands out RunOrders; deck owners with hacking above 0.5 and greed above 0.7 get a standing run goal.

9. **Arrest chases stale tiles.** What: 29 Arrest goals, about 25 fail PreconditionLost after 1-3.5 h walks (Vale Achebe, Kieran Oyelaran, Thea Carrow twice, Lux Fletcher twice, Odessa Stroud twice). Travel is 13-20 h a day, work 0.2-1.9 h. Cause: the plan binds a SuspectTile at plan time and fails when the suspect has moved; the same warrant is replanned seconds later. Severity: bug. Fix: re-locate on arrival, a per-warrant cooldown after failure, dispatch only the nearest cop to a fresh sighting.

10. **Runner sleep starved and broke, no reaction.** What: sleep 0.8 h on d26, 3.6 h on d21; Earn 0.788 beats Sleep 0.586; wallet near 0 with wealth need 0.97-1.00. Cause: no fatigue penalty on chase goals; Earn for a greedy low-lawfulness agent has no high-yield action (Fence/Extort need a gang). Severity: shallow. Fix: sleep urgency override below energy 0.2; give Earn deck-sale (SellData exists but is never planned) and fencing.

11. **Violence leaves no trace.** What: the ripperdoc diary opens with two unsolved beatings (d6, d9) and shows no fear, no self-treatment, no grudge. The runner holds four grudges (weights 0.23-0.39) and plans no revenge all week. Severity: shallow. Fix: wounded ripperdocs treat themselves at their Clinic; grudges above 0.3 feed a Hunt goal for non-cops.

## Top 5 changes
1. Give Clinic staff a real job: customers, fees, revenue-funded wage (1, 4, 7).
2. Remove the off-shift wage-chase and the quit/rehire cycle (2, 3).
3. Make the runner an archetype: fixer-fed RunOrders, no precinct day job, active sell and upgrade loop (8, 10).
4. Fix Arrest on stale suspect info: re-locate, cooldown, nearest cop (9).
5. Give Earn a feasible fallback for broke Street agents so 12-14 h of Rest and the GetHigh dead end disappear (5, 6).
