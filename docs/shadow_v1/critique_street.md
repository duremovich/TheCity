# Street critique: homeless_622 (Ronin Hallow), worker_505 (Echo Singh), child_2925

Seed 42, d20-d26. Code cites are file:function, checked unless marked "unverified".

## Ronin Hallow, 56, homeless Dreg: a day in the life (typical d22-d25)
- 00:30 leaves a bar, walks ~2h to Civic Hall, collects 4c dole (the only income, once a day).
- 03:00-07:00 wanders Civic streets in 30-min Wander blocks, or goes to a Mid East bar for a stim he often cannot get.
- 10:30 walks ~105 min to a squat in Sump Central, sleeps ~4h (never a full night).
- 16:00 walks ~107 min to a Mid East market, pays 6-9c for 2-3 food, eats in 11 min.
- 18:00-24:00 Wander again (12 straight blocks on d22), or sits in a bar "Resting" 90 min at a time.
- Week totals: sleep 35h, travel 50h, idle 76h, social 4h, crime 0.4h. Net -4c. Never tries a Hotel, a lease, begging or scavenging.

## Echo Singh, 31, Sanitation worker: a day in the life
- d20 (the one clean day): up 05:37, 109 min walk to a bar, chats with a stranger, leaves 09:18 for work.
- Arrives at the Vats recycler 13:19 after a 4h commute. Works to 18:00 (4.8h of a 9-18 shift).
- Walks 4.4h to Civic Hall to collect a 4c wage, arrives 22:24. Then walks 3.9h home. Sleeps ~5h. Net +2c.
- d21: energy sits at 0.00 for 16 hours, "sleeping" in 1-minute fragments. No work.
- d22 works 5.9h, never collects the wage. d23 does nothing. d24 works 2.9h, then steals food from a neighbour.
- d25 arrested, d26 in jail. Week: worked 13.5h, wage collected once (4c), travel 61h, family and friends 0h.

## Child (baby, 0 y)
24h idle in the home block, all 7 days, no events. A baby's day should still exist: fed and slept on a schedule, carried or kept by a parent, parent's needs and errands bending around it (a worker with a newborn does not do a 10h shift run). Today children cost their parents nothing and produce no signal.

## Findings

1. **Distances make a normal life impossible.** What: Ronin's squat is ~155 min from the Hall, Echo's home is 4-5.8h from her job and 1-2h from the nearest market and bar. Daily errands eat 6-15h of travel. Why: the dole city is meant to be poor, but nobody lives near anything they use. Cause: jobs, markets and squats are assigned without a distance term (`demography::hire_candidate`, distance unverified; `move_ticks_full` is one tile per tick). Severity: shallow, systemic. Fix: weight hiring and squat choice by walking time (cap commute ~60 min), seed homes near their market and bar, and add cheap transit (the vision has rails) that Dregs can pay pennies for.

2. **Work, sleep and the commute thrash (bug).** What: Echo's energy is 0.00 from 04:00 to 18:00 on d21 while her log shows ~30 one-minute Sleep actions. Same pattern d22-d25: Work, Socialise, Work, Eat flip every 30 min (d22 00:37-05:55), and on d25 she reached the Vats at 07:25, then turned and walked 5.5h home. She worked 3 of 6 days and earned 4c. Cause: `actions::finishes_early` ends Sleep whenever `routine::must_leave_for_work` is true. That gate is `ticks_until_shift <= travel_estimate + 60`, so with a 4-6h commute it is true most of the day. Sleep restarts, restores nothing, and Work, Eat and Socialise take turns winning each 30-min think. Severity: bug. Fix: a committed Sleep (minimum 4h) ignores the commute gate; give Work stickiness once a commute starts; cap the leave-early margin.

3. **Wages and dole need a long walk to the Hall (bug/shallow).** What: Echo's wage is collected once, on d20 (a 264-min walk after the shift). The d22 shift was never paid and "wages owed" stays 0.98. Ronin walks 85-154 min every day for 4c, and on d25 the Hall was BuildingFull three times. Why: the main income loop is a pilgrimage and unpaid work is invisible. Cause: `CollectWage` requires standing at `world.wage_desk` (the Hall for a city job; `actions.rs` precondition), and the plan goes GoTo(Hall) right after the shift. Fix: pay at the workplace or a district office, collect dole weekly in bulk, and queue overflow at the Hall instead of failing.

4. **The poor never try to earn anything but the dole.** What: Ronin spends 14h a day in bars and on streets and never begs, scavenges, steals food or sells anything. His hacking is 0.73 and goes unused. Cause: `GoalKind::Earn` is already satisfied once the dole was taken today (`goals.rs` already_satisfied, Earn), and Beg is only planned inside Earn, so after the dole there is no income goal. Severity: missing feature. Fix: keep Earn alive when wealth is below a meal; give it a Beg/Scavenge/odd-job menu with skill and class gates; let hacking 0.7 earn small gig income.

5. **No ladder climbing.** What: 7 days on 6-10c and never a Hotel (3c a night, `night_price`), even on dole day, never a lease. Why: street > hotel > squat > lease is the stated survival loop and shows no motion. Cause: a Hotel is only planned when Sleep fires with coins >= price and a Hotel within `hotel_reach` (48 tiles); the squat sits far from where he is when tired. Fix: a "save for a bed" goal that holds back 3c and heads to a Hotel at dusk, and log why a rung was rejected.

6. **Flee has no plan for the homeless (bug).** What: on d26 Flee scores 0.84 (hostile near) and logs "Flee unplannable"; he keeps Resting in the same bar for 8h. Cause: the only Flee action is `FleeToHome` (`goap/actions.rs`, needs `LocationKey::Home`). Fix: flee to the nearest squat, Precinct or crowd.

7. **Witness spam turns a shrug into a feud (bug).** What: on d26 17:00-18:18 Ronin logs seven "saw Wire Abernathy commit Dealing" events. Affinity goes 0.01 to -0.99, Acquaintance to Enemy, in 32 min, and he buys stims in that same bar. Cause: one Witness event per minute of a multi-minute crime, each calling `social::witnessed_crime_of` (-0.2). Fix: one witness record per crime instance, scaled by severity and the witness's own lawfulness and habits.

8. **Idle behaviour is thin.** What: Wander is a flat 30-min loop, and "Rest" in a bar runs 90 min at a time for 14h on a 2c tab. A corpse (Yuki Fletcher) lay in the bar all afternoon. Bury scored 0.26 but was unplannable and Loot failed with StockGone. Cause: `routine::idle_plan` is Rest-in-bar or Wander only. Fix: a personality-weighted idle menu (busk, watch, scavenge, visit someone), bar owners shoo loiterers, and make Bury and Loot plannable.

9. **Family and friends exist as numbers, not time.** What: Echo has a Spouse and 8+ Friends at affinity 1.00, yet her only chats are strangers (top contact Mateo Sato, 5). Ronin's spouse (affinity 0.68) never appears. Seventeen strangers became Acquaintances in a single minute in a bar. Cause: Chat target choice favours whoever is in the room (unverified). Fix: a weekly "see someone I love" goal (spouse, friend, parent) that pays belonging and intimacy, and slower edge creation from mere co-presence.

10. **The theft chain works but the trigger is dumb.** What: Echo walks home 5.8h to EatAtHome, finds no food (PreconditionLost), steals 2 food next door, is reported, arrested and sentenced to 3 days. Credit: this is good emergence. Concern: she had 1c, food costs 3c, and the plan walked past a market. Fix: check the pantry before committing a long walk home.

11. **No weekly rhythm.** Only day 6 of 7 is a rest day (`routine::is_workday`). No market day, no family day, no leisure block. Fix: weekly routine templates.

12. **Sleep timing is arbitrary.** Both agents sleep in 4h chunks at any hour (Ronin 18:52-22:55, Echo 12:55-18:00). Cause: sleep triggers on energy, with the night phase only a 0.4-0.7 multiplier. Fix: a stronger night gate for the housed, plus a chronotype per person.

## Top 5 changes, ranked
1. Fix the Work/Sleep/commute thrash (#2): committed sleep, sticky Work, capped margin.
2. Cut distances or add cheap transit and distance-aware hiring (#1).
3. Pay wages and dole locally and queue the Hall (#3).
4. Keep Earn alive for the poor with Beg/Scavenge/gigs and a save-for-a-bed goal (#4, #5).
5. Fix the Witness spam and Flee-for-the-homeless bugs (#6, #7).
