# Critique: the guard and the sweeper (seed 42, d19-d25, L2 build a737870)

Diaries: `guard_1723.md` (Mina Abernathy) and `sweeper_824.md` (Dex Duarte), both pinned at Full LOD for all seven days. Day 19 is the Friday. Day 20 is each worker's rest day (`is_workday`: shift key 6 is off).

## Mina Abernathy, city guard (37, wage 8/day): a day in the life

She has two kinds of day, alternating (`law::jail_duty`, modulus 2).

**Desk day (d19, d21, d24).**
- She wakes between 03:00 and 06:00 and walks 70 to 137 minutes from her Spire flat (Block#76) to the Precinct in Civic.
- She rests there for an hour, then sits in `GuardJail` from 09:03 to 18:00 as one 9-hour action.
- The wage (+7 c, one coin of rent out) arrives at 18:00 at her desk. There is no Hall walk any more.
- She then walks to a Civic market (67 min), buys three meals for 12 c, and walks home (75 to 105 min).

**Patrol day (d20, d22, d23, d25).**
- She wakes around 01:00 to 05:00 and walks about two hours to a street tile in Spire to hang out with other guards. She wanders at home until 09:13.
- At 09:13 the Patrol goal starts from her front door. She walks 3.7 to 3.9 hours to Street Market#414 in Mid East.
- She then does four 21-minute "legs" (market, Bar#417, two Blocks) joined by 35 to 180 minute walks, and walks home at 18:00 or later.
- On d23 she meets and arrests Hex Quarry at 09:07 and escorts him in.

**Evenings.**
- She hangs out an hour or two at a street spot (Civic or Spire) with Nyx Osgood, Vale Achebe, Katrin Haas and others. She swaps gossip and wins or loses 1 c at dice.
- On d23 she pays 3 c to Zetatech for a braindance.
- She sleeps 4 to 7 hours.

**Week totals.**

| | hours |
| --- | --- |
| work | 33.7 |
| travel | 68.2 (9.7 h/d) |
| sleep | 38.6 |
| leisure | 12.9 |
| idle | 13 |

She earned 37 c in wages and ended the week +11 c. She made 1 arrest in 1 Arrest plan. Over the week she witnessed no crime on patrol.

## Dex Duarte, public-works Sanitation worker (19, wage 5/day): a day in the life

- He lives in Spire (Block#50) with a Spouse and nine Friends at affinity 1.00. His beat is a different district each day: Spire on d19, Civic from d21 to d25.
- A work day starts when he wakes at 03:30 to 06:30 and walks 130 to 265 minutes toward the beat. The Recycler he is employed by is in Vats, and he never goes there.
- He arrives between 09:00 and 12:06. He sweeps one street in one block of 4.5 to 9 hours, then collects 4 to 5 c at 18:00 where he stands.
- He walks 97 to 121 minutes to a market, buys one 4 c meal, and walks two hours to a street spot for a one-hour hang-out with acquaintances. Then he walks home or sleeps on the pavement.
- Whenever the shift is not on, or he is broke, he scavenges in 60-minute blocks for about 1 c per two hours. He did this for about 12 hours on his rest day.
- Week: sleep 39.6 h, work 40.2 h, travel 41.2 h, leisure 7.7 h. Wages +22 c, scavenging +6 c, food -24 c with 8 c refunded, rent -4 c, upkeep -7 c. Net +1 c.
- He is "starving" on d22 and d23 and his hunger reads 0.00 for all of d24 and d25.
- He owns Noodle Bar#445 and never goes near it.

## Findings: the guard

**G1. The patrol is a walk with four short stops.**
- Evidence:
  - d20 09:13 goes to Street Market#414 in 233 min, with the first leg at 13:06. d22 09:13 takes 221 min, and d25 09:13 takes 233 min.
  - The legs are always the same: Market#414, Bar#417, a Block, a Block, all in Mid East. All four patrol days use the same circuit.
  - Patrol-day "work" is 1.1 to 1.4 h and travel is 10.2 to 14.0 h. This is where the 9.8 h/d walk lives.
  - 0 witness events across four patrol days. A leg at Bar#417 (d20 14:05, d23 12:57, d25 14:05) logs nothing about who was there.
- Cause:
  - `must_leave_for_work` and `commute_plan` only run on jail duty (`exec/routine.rs:118`), so a patrol day has no walk to the Precinct. Patrol starts wherever she stands.
  - The route is chosen by `patrol_route` and ignores distance from the guard.
  - The leg itself has no encounter roll.
- Severity: distorts the day (most of the remaining ten hours of walking).
- Fix:
  - A patrol shift begins at the Precinct: a briefing Rest there is the commute, as on desk days.
  - The route is the Precinct's district plus a neighbour, ordered near-first, and rotated so the same four buildings are not the beat every day.
  - A leg should be a loop of the street tiles with a witness roll, and log "walked the market, saw N people, nothing" or the crime.

**G2. She patrols on her day off, unpaid.**
- Evidence: d20 is the rest day (the sweeper does not work), yet she patrols from 09:13 to 18:13 and no wage arrives at 18:00 on d20. The wage flows are d19, d21, d22, d23, d24, d25: six, not seven.
- Cause: `utility/goals.rs` Patrol `on_duty` does not read `is_workday`. `law::credit_guard_shifts` does (`law.rs:1190`), so the shift is walked and never owed.
- Severity: bug.
- Fix: gate Patrol and Arrest on `is_workday(key)`, or give guards a rota with a different rest day. Today the guard works 7 days and the sweeper 6.

**G3. The duty flips mid-shift and wastes the walk.**
- Evidence: d22 patrols from 09:13, does one leg at 12:54, then at 13:13 the goal becomes Work (GuardJail) and she walks 100 minutes back to the Precinct for a 187-minute desk stint.
- Cause: `jail_duty` is recomputed per call from posture, roster rank and slot (`law.rs:1044`). Something in that mix changed at 13:13 (posture or roster).
- Severity: distorts (a 100-minute reversal; the shift plays as two jobs).
- Fix: fix the duty of a shift at shift start, and let posture changes apply from the next shift.

**G4. The commute and every other walk are two-hour trips inside a district.**
- Evidence:
  - Spire to Precinct runs 71 min (d24 06:32), 75 (d19 17:43), 89 to 105 (d22, d23, d25 evenings) and 137 (d21 05:43). The Precinct to Street Market#413 inside Civic is 67 min (d21 18:03, d24 18:04).
  - Her HangOut spot (107,7) is in her own district, yet GoTo(Spot) from her block takes 115 min (d20 01:46, d25 07:04), 105 (d24 21:13) and 82 (d21 03:21).
  - Two of the three pinned guards live in the same Spire Block#76 (guard 2005 too), 75 minutes from the Precinct.
- Cause: housing is still assigned without regard to the workplace (V1 findings 3 and 7 were not fixed). Hang-out spot choice carries no distance cost (V1 cause 1). A Block in Spire is far from its own street spot.
- Severity: distorts.
- Fix:
  - Guard housing near the Precinct (the Garrison or a Civic Block).
  - A distance term in the Unwind pick (prefer a spot under about 15 minutes, else a venue).
  - A per-block hang-out spot at the doorstep.

**G5. The commute flickers: she turns around on the road.**
- Evidence:
  - d19 00:25 to 07:45: Work, then Idle with a 30-minute Wander at 01:13, 02:43 and 07:13, each time resuming. Two hours go to wandering on a 7-hour walk.
  - d21 04:43 Work, 05:13 Unwind (walks to a spot), 05:43 Work again.
  - d24 06:32 walks 71 min toward the Precinct, at 07:43 turns to go home for a Rest (30 min), then at 08:13 walks to the Precinct again.
- Cause:
  - `must_leave_for_work` compares time left to a straight-line `manhattan * move_ticks * 3/2 + 60` and is re-evaluated on every leg boundary, with no commitment once the trip has begun. Where it is optimistic she is late, and where it is pessimistic she stops.
  - On d19 she starts the day in a Sump West capsule hotel, 7 hours from the Precinct, which is why she leaves at midnight.
- Severity: distorts.
- Fix: once Work has started a commute, hold the plan until arrival (hysteresis). Use path length, not manhattan, for the estimate.

**G6. The one arrest does not read as an arrest.**
- Evidence:
  - d23 09:07 Arrest, 09:17 "Mina Abernathy arrested Hex Quarry", then Escort at the Precinct for 61 min with no walk.
  - Nothing after it: no memory of the arrest, no booking or sentence line, no standing or heat change, no pay. The next lines are "Patrol" at 10:20.
  - The suspect is not described. We learn only from other diaries that Hex stole on d19 (Hana Santoro witnessed, 09:23) and assaulted someone on d23 06:17 (Katrin Vance witnessed).
  - She had sat beside him at the Civic hang-out on d22 19:07 and 20:10 (Acquaintance, +0.02), and arrested him three hours after the assault only because he was in the street on her route.
- Cause: `law::arrest` and Escort are teleport-short actions. No memory is written to the guard for the arrest, and the sentence is visible only from the suspect's diary.
- Severity: distorts (it is the guard's headline event).
- Fix:
  - Write an `Arrested` memory with the charge, a booking line (suspect, charge, sentence length) in the guard's diary, and a small standing gain and wage bonus.
  - Escort should walk from the arrest tile to the Precinct.

**G7. The volume of law is tiny, and witnessed crime goes unacted.**
- Evidence:
  - 4 arrests in 4 plans across three guards in seven days (STATS), against thefts and assaults that three Full-LOD guards witnessed.
  - On d22 08:04 Nyx Osgood and Hana Santoro both saw Sabine Duarte commit a Theft. No arrest followed by d25.
  - Mina meets Sabine as a harmless Acquaintance (d20 03:41) and hears "Robbed by Sabine Duarte" as a rumour at 19:39 on d22. "no warrant 1.00" holds on every patrol day.
- Cause: the only way to an Arrest is a located warrant in reach (`Patrol`'s "no warrant" and `Arrest`'s gate), and a witnessed crime by a guard does not itself raise one. V1 finding 12 (witness-first intervention) is not built.
- Severity: distorts the role (a guard who sees theft and does nothing).
- Fix: an in-shift guard who witnesses a crime within a few tiles opens a warrant and Detains at once. A warrant raised by a guard's own witnessing gets priority over a report.

**G8. The guard befriends prisoners on desk duty.**
- Evidence:
  - d19 08:42 three prisoners become Acquaintances through MetInJail memories. d19 12:12 Hale Kettering goes +0.08 to +0.28, d19 16:12 +0.48, d21 12:00 +0.68, d21 16:00 +0.88, d24 12:48 +1.00.
  - d24 09:18 she meets the Ninefold leader Ysolde Moreau, and by 16:48 she is a Friend (+0.47). Kenji Ito goes the same way.
  - Silvio Carrow and Rhys Stroud rise +0.20 every four hours to +1.00 in the same stretch.
- Cause: the Jail's coarse co-presence still hands out affinity every few hours (V1 cause 5, jail LOD, `MetInJail`), now from the guard's side.
- Severity: distorts (a jailer reads as everyone's best friend, and the jailed gang leader is a Friend by teatime).
- Fix: guard-prisoner contact is neutral to hostile and capped (affinity delta 0 for the guard on `GuardJail`), with the jail still counting as a contact for gossip.

**G9. Her nights: she wakes at 01:00 and walks two hours to stand in a street.**
- Evidence:
  - She goes to bed about 19:00 to 21:00, which is when she gets home. She wakes at 01:46 (d20), 03:21 (d21), 05:13 (d23) and 07:04 (d25), and each time walks to the street spot (G4) to hang out with other guards (d20 03:41, with Nyx Osgood).
  - d24 03:32 to 05:43 is ten Sleep actions of 1 to 6 minutes each, logged as "PLAN Idle [Sleep]", while energy is 0.97.
- Cause: sleep ends when energy reaches about 0.9, so an early bed means a pre-dawn waking. The Unwind pick fires "off hours 1.00" at any dark hour and ignores that a venue is closed. The fragments are an Idle plan that picks Sleep with a full tank.
- Severity: distorts (a nocturnal sleep pattern and the pavement as her social life) and cosmetic (the fragments).
- Fix: after a pre-dawn waking, stay in and Rest or Eat at home until the departure time. Keep street hang-outs for evening hours, and send Unwind to a venue first. Idle should never plan Sleep above a high energy value.

**G10. Court goes nowhere.**
- Evidence:
  - d21 19:34 and 19:43 Court (Flirt) fails PreconditionLost twice, then cools for two hours. d23 05:43 Court on Nyx Osgood walks 30 minutes toward the market, then is dropped for Unwind at 06:13.
  - Intimacy reads 0.00 all week. The Friend at +1.00 affinity (Leopold Yardley) is never seen. She actually hangs out beside Nyx on d20, d22 and d25.
- Cause: the Court plan goes to a Market, not to where the candidate is standing, and the target moves.
- Severity: cosmetic (the goal exists and is visible, which is progress over V1's invisible marriage).
- Fix: Court takes the "go where the target was last seen" route, and a hang-out where the candidate is counts as the Flirt step.

**G11. No corruption, no bribe, anywhere.**
- Evidence: no "bribe" line in any of the three guard diaries. Mina has lawfulness 0.59, greed 0.21, wealth 0.07 to 0.11 all week and sat beside Sabine Duarte (a thief) and Hex Quarry (a wanted man) with no offer made.
- Cause: bribes are faction-level (V1 finding 9); there is no guard-level take-a-bribe decision.
- Severity: missing feature (not a bug at this personality, but nobody ever offers).
- Fix: an offender who meets a guard and has coins can offer a payoff. The guard's roll uses lawfulness, greed and wealth. It earns coins and a quiet reputation risk.

## Findings: the sweeper

**S1. The commute is 2 to 4.5 hours each way and the beat moves.**
- Evidence:
  - d21 06:26 walks 245 min and arrives 10:31, 91 minutes after shift start. d23 08:42 walks 204 min and arrives 12:06. d24 07:07 walks 239 min and arrives 11:06. d19 04:04 starts a 130-minute walk, stops, and he never reaches his beat.
  - The beat is Spire on d19 (his own district) and Civic from d21. The employer, Recycler#419, is in Vats and he never goes there.
  - On the Civic days he sweeps 4.6 to 8.9 h, not 9.
- Cause: housing is unrelated to the beat. `beat_of` is the midnight allocation (`sweep_beats`), so the beat changes between days. `travel_estimate` is straight-line, so he leaves late and arrives late.
- Severity: blocks the role (he cannot work a full shift on most days and pays for it in hours on the road).
- Fix: allocate sweepers to the district they live in (the beat is the home district, rebalanced rarely). If the allocation must move people, move their housing or give a fixed beat per sweeper.

**S2. He works full time and starves.**
- Evidence:
  - d21 00:00 RentShort (0 of 1 c). d22 07:44 and d23 22:07 "Starving". Hunger snapshots read 0.00 for all of d24 and d25 and 0.00 to 0.12 on d22.
  - Money: wage 22 c (+4 or +5 a day), scavenging 6 c, food 24 c less 8 c refunded, rent 4 c, upkeep 7 c, net +1 c in a week of 40 working hours.
  - d24 he buys no food (purse 1 c). He eats once a day, only after 18:00, after a 97 to 121 minute walk to a market.
  - He owns Noodle Bar#445 and is never in it.
- Cause: his job shows `wage 5/day`, which the config comment itself says is below the 4 c dole after tax (`assets/config.toml` near `works_wage`). `works_wage = 7` applies to new hires only. The daily upkeep and rent fall on the same coins. Nothing feeds a sweeper on shift.
- Severity: blocks the role (the job does not keep a person alive).
- Fix: raise `wage_sanitation` to the works wage (7), take the upkeep tick off public-works pay, and give the Recycler or the beat a canteen so a meal is bought near the street he sweeps. His own bar should feed him if he owns it.

**S3. A half shift pays nothing, and the half shift is hunger-driven.**
- Evidence:
  - d24 15:44 after 274 minutes of Sweep, Work is replaced by Earn ("not in shift 0.00 to 0.30"). The wage on d24 is absent (the five wage flows are d19, d21, d22, d23, d25). He leaves at 15:44 and scavenges until 20:44.
  - d19 13:14 he sleeps 240 minutes on the street mid-shift (energy 0.28), and the wage is paid only because he happens to be sweeping again at 17:16.
- Cause: pay is credited to a worker who is on the job at 18:00 (or who completed the shift), not for hours worked. Earn outbids Work when hunger and wealth are both at 1.00.
- Severity: distorts.
- Fix: pay pro rata for hours swept, and let a starving worker eat on shift (S2) so Earn does not need to win.

**S4. Rough sleeping on the beat and 4-hour sleeps.**
- Evidence:
  - Sleep blocks are exactly 240 or 241 minutes six times (d19 00:01, d19 13:14, d21 02:23, d21 23:46, d23 04:39, d23 23:27). Total sleep is 3.5 to 4.8 h on five of seven days.
  - He sleeps on the pavement of Civic on d21 (22:10 and 23:46), d22 (03:48 ends), d24 (16:46, 19:14 and 20:46 for 404 minutes).
- Cause: the commute (S1) makes home unreachable, so Sleep plans in place. The 241-minute cap is not explained by the diary (it recurs every time he is hungry, so I suspect starvation limits a sleep action). I did not find it in code.
- Severity: distorts.
- Fix: S1 and S2. Check whether hunger caps `Sleep`.

**S5. The "waiting for the shift" loop spams.**
- Evidence: d22 07:23 to 08:59 and d25 06:49 to 08:44: "PLAN Work [GoTo(Beat) > Rest]" followed by "Rest, 2 min", repeated eight or nine times at a 6 to 24 minute rhythm.
- Cause: the commute plan's Rest step ends after two minutes and is replanned until the shift starts.
- Severity: cosmetic.
- Fix: one Rest until shift start.

**S6. We cannot see the beat get cleaner.**
- Evidence: a day's work is one `Sweep` action of 240 to 532 minutes with no line about what it did. The diary has no litter figure, and no event says "swept N units".
- Cause: `jobs::sweep_done` cleans `sweep_per_hour` (6) units of the dirtiest tiles when the action ends, and nothing is logged. The Sweep that was cut short by sleeping on d19 is not shown as credited.
- Severity: cosmetic for the reader, but it means I cannot judge the point of the job.
- Fix: log "swept N units on the beat, litter a to b" at the end of each Sweep, and put the beat's before-and-after litter in the diary's day summary.

**S7. The Friday night is six hours of walking and one hour of company.**
- Evidence:
  - d19 (Friday): wage 4 c at 18:00, then 121 minutes to Street Market#413, one meal at 20:04, 139 minutes back to the Spire street spot, a hang-out at 22:46 (three people, two strangers, +1 c at dice), then 129 minutes home.
  - His Spouse (Silas Park) and nine Friends at affinity 1.00 are never seen. One sighting of the Spouse at home, d23 05:09.
- Cause: V1 cause 1 again: the food pick walks to a market far from home and the Unwind pick goes to a fixed spot. There is no household or friends goal.
- Severity: distorts.
- Fix: eat at the market in the home district or on the beat, a Visit goal toward affinity 1.00 contacts, and a hang-out at the doorstep.

**S8. Scavenge income is booked as a wage.**
- Evidence: every scavenged coin is "Sanitation +1 from the City" (d19 08:16, d20 10:19 and 12:23, d21 21:06).
- Cause: Scavenge credit is booked under the Sanitation ledger flow, the D23 sweeper credit.
- Severity: cosmetic.
- Fix: label it Scavenge. The city does not pay a sweeper 1 c an hour to rummage.

## What reads right

Against V1:
- **The guard walks 9.8 h/d, not 13 to 17.** Energy-zero hours are 0, against 54 before L1 and 6 after. Wages arrive at the desk at 18:00 with no walk to the Hall (V1 finding 7, the Hall trip, is fixed).
- **She sleeps in real blocks** of 5 to 7 hours (6.7 h on d22 and 7.3 h on d23).
- **Her one arrest is a true hit.** The suspect was wanted, in reach, and it took 10 minutes. There were no `PreconditionLost` failures and no corpse targets, against 3 arrests in 42 plans.
- **Her leisure exists.** HangOuts with named colleagues (5 known at once on d22 19:07), gossip told and heard, a 3 c braindance on d23, and a fun need that sends her out.
- **The desk shift is a real place**: 9 hours at the Precinct, with prisoners arriving and being met.

Against L1b:
- **The guard's walk figure is similar (9.0 then, 9.8 now)** but the idle share fell (7.3 to 1.9 h/d) and work rose (2.6 to 4.5 h/d).
- **The sweeper has a shift, a wage and a physical action** (`Sweep` on a street tile), where L1b had a TendGraves ledger. His wage lands where he stands, and a sweeper can sweep a street of his beat for eight or nine hours.
- **Hang-outs are alive.** On d22 he is in a crowd of 9 to 11 on a Mid East street and gossip passes.
- **The poor have a fallback.** Scavenge keeps a broke worker earning (V1 cause 4), RentShort and Starving events read plainly, and a refund appears when a plan breaks (d22 19:44).

## Verdicts

**The guard: the role reads about half right.** The numbers moved most of the way from V1. A reader would still ask where the guarding is. On patrol days she walks to a district where nothing happens, patrols four short legs, and goes home. On desk days she is a gaoler who befriends the inmates. The one arrest is the thinnest part of the story: no description, no booking, no consequence. The biggest levers are G1 (patrol starts at the Precinct and sees something), G7 (witnessed crime opens a warrant) and G6 (an arrest leaves a trace). G2 (the unpaid day off) is a plain bug.

**The sweeper: the role does not hold yet.** The job exists, but the day is built from walking, sleeping on a pavement and starving. He works a full week and ends +1 c. A reader cannot tell whether the street got any cleaner. Fix the commute (S1) and the wage (S2) before anything else, then log what the Sweep did (S6). Until then a sweeper is better off on the dole.

Top changes, in order:
1. Housing and beat: guards near the Precinct, sweepers on their home-district beat (G4, S1).
2. The sweeper's wage, upkeep and food on shift (S2, S3).
3. Patrol from the Precinct, with a leg that can see something (G1, G7).
4. Gate Patrol on the workday and fix a shift's duty at shift start (G2, G3).
5. Commit to a commute once started (G5), and make arrests and sweeps leave a trace in the diary (G6, S6).
6. Cap affinity from jail contact (G8), and fix the night routine and its distant spot (G9).
