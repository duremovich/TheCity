# Critique: gang leader #732 and Purist #1541 (seed 42, d20-d27)

Both diaries are mostly jail. Idris is free for about 38 hours, Imani for about 30. Findings cover those free hours plus what the jail stretch shows.

## Idris Eastwick, leader of Ninefold (33 members, treasury 4c)
- d20 00:00 starts to sleep (energy 0.34). 00:12 GangWork (0.86) overrides it. Walks 235 min Mid East to Sump East, alone.
- 04:07 occupies derelict Block#296 (11 min). Ten residents flip to Enemy at -1.00. 04:20 walks 397 min home to sleep. Energy sits at 0.00 from 08:00. Sleeps 10:57-18:12.
- 18:24 walks 59 min to a bar, one drink, then rests 4 h in the bar. Meets acquaintances, talks to nobody.
- d21 00:12 plans Extort, switches to Occupy at 06:12. 08:57 beats Nerys Lindqvist, 10:01 beats Adela Vance, in daylight with witnesses.
- 10:12 walks 177 min to a bar to chat with Dante Tanaka. Chat lasts 1 min and fails. 13:17 arrested, 15:06 sentenced to 10 days.
- d22-d26: 24 h/day in the Precinct. No action, no visitor, no gang contact. The gang's standing order flips 9 times.

## Imani Oyelaran, Unplugged Purist (20 members, treasury 0c)
- d20 to d25 17:13: jailed for Assault (5 days). She is already inside at the start.
- 17:14 released, plans Occupy of Block#361, a 426 min walk. Arrives 00:20 d26, occupies in 11 min.
- 00:33 walks 170 min to her Hideout, the Chapel, and sleeps 03:23-10:36. Eats 10:38.
- 11:08 walks 144 min to a market to chat with Brick Yardley, who is gone. Then a 30 min chat with a stranger, who leaves. Socialise is cooled. 146 min walk back, rests.
- 18:04-22:00 two more half-trips toward a bar, one drink. 22:50 starts a 70 min walk to collect the dole.
- Her gang appears nowhere. Total contacts: two strangers.

## Findings

1. **Both agents spend 12-13 h/day walking.** (bug or tuning)
   - **What:** single legs of 235, 397, 426, 170, 144 and 177 min. Day summaries show travel 11.7, 12.1 and 13.1 h. Imani walks 7 h to squat one block, then 3 h to sleep.
   - **Why it matters:** the day is a commute. There is no daily rhythm, and every long walk makes the target stale.
   - **Likely cause:** walk cost is manhattan distance on a big map (one tick is one minute), and goals pick targets city-wide with no travel-time term (`goap/actions.rs` TargetHome and Hideout resolution).
   - **Fix:** weigh travel time in goal and target choice, so a 2 h walk kills a Socialise. Prefer venues in the agent's own district. Let a Hideout or squat be the sleep and social anchor.

2. **A leader behaves like a member.** (missing feature)
   - **What:** Idris does the Occupy and Extort errands alone. He never musters, recruits, collects, disciplines or meets a member. His top contacts are victims and enemies. VISION says "a gang leader moves with members."
   - **Why it matters:** position does not show. The M8 spec says the brain is "not an embodied leader action", so this is by design, but it is the biggest gap against the brief.
   - **Likely cause:** GangWork in `utility/goals.rs` has no rank term. Rank only feeds `leader_ranking` in `systems/gang.rs`.
   - **Fix:** give rank 2 its own actions: Muster (members gather at the Hideout at a set hour), Collect (dues), Parley with the rival leader, Review (hear a grievance). Make the leader travel with a retinue.

3. **The gang economy is dead.** (shallow)
   - **What:** 33 members, treasury 4c, leader wallet 27c, net +7c in a week. His wealth need is 0.88 and he earns nothing. The planned Extort never ran.
   - **Why it matters:** a greedy leader with no income is the opposite of the fantasy. Squatting pays nothing.
   - **Likely cause:** GangWork follows the standing order (Squat, Expand). `extort` pays, Occupy does not.
   - **Fix:** member dues and a cut of every extort into the treasury, with a leader's cut out. Show treasury flow in the diary.

4. **Jail is a black hole and the diary goes blind there.** (missing feature plus noise)
   - **What:** 129 h and 137 h in the Precinct, shown as Coarse with goal None, so the pin is lost (Idris was "Full (pinned)" on d20). Each inmate logs about 40 MetInJail memories a day, the same people repeating (Casimir Greaves#1492 about ten times).
   - **Why it matters:** nothing happens inside. No fights, recruiting, contraband, messages, bribes or visits. Xia Draper, his spouse, is inside on d25 and nothing links them. Memory slots go to noise.
   - **Likely cause:** `systems/social.rs:537` writes a memory for every cellmate pair on each pass. LOD demotes jailed agents.
   - **Fix:** keep pinned agents Full in jail. Write one MetInJail per cellmate per sentence. Add cell actions: Brawl, Recruit (jail is where gangs get members), SmuggleMessage, AwaitVisit.

5. **The gang's reaction to the arrest is thin and unreadable.** (shallow, partly unverified)
   - **What:** after d21 15:06 the order goes Squat, Expand, BreakOut, Squat, Expand, Squat, BreakOut, LieLow, Squat. BreakOut is issued on d23 03:27 and d24 09:28. Idris is still inside on d26. No Jailbreak event reaches his diary.
   - **Why it matters:** a 33-man gang with its boss inside should visibly storm the Precinct or visibly fail.
   - **Likely cause:** succession works (`gang.rs` `recompute_leader_after`, `boss` set). Each jailing or leader change is a shock, and shock rescoring uses hysteresis 0.0 (`faction.rs` `rescore`), so orders flip on the first shock. BreakOut is gated on `breakout_ready` and `breakout_min_members`. The diary cannot show whether a muster formed.
   - **Fix:** a minimum dwell per order (about 24 h) except Retaliate and BreakOut. Log "muster failed: N of M showed". Let the jailed boss influence the gang: smuggle an order, raise BreakOut's score, name a target.

6. **The squat makes enemies, and the leader brawls himself.** (shallow)
   - **What:** Occupy flips every resident to Enemy -1.00 (13 edges on d20 04:18, 5 on d21 09:44). Within the hour Idris fights an "enemy near" at 0.35 in daylight and is arrested 3 h later for 10 days. Imani got 5.
   - **Why it matters:** a lawfulness 0.16 leader may brawl, but a 33-man leader acting as his own street thug is not believable. Ten days is 8 % of a 120-day run.
   - **Likely cause:** occupy writes -1.00 affinity directly, and the Fight score reads "enemy near".
   - **Fix:** scale the eviction hit by the resident's stake (about -0.6, with a Grudge only for the evicted). Let a leader delegate violence to a member through a Hit order.

7. **Sleep loses to any outranking goal.** (shallow)
   - **What:** energy 0.34 at midnight, 0.00 for 3 h from 08:00, bed only after 11 h awake. He walks 6.6 h to a bed instead of using the squat he just took.
   - **Why it matters:** exhaustion has no cost, and the nearest bed is ignored.
   - **Likely cause:** GangWork scores 0.86 against Sleep 0.52, and Sleep always plans GoTo(Home).
   - **Fix:** below energy 0.15 force Sleep at the nearest owned or squatted place, and add a Tired penalty to fight and work rolls.

8. **The Purist creed is invisible in behaviour.** (missing feature)
   - **What:** Imani has chrome false, pride 0.97, courage 0.94. In 30 free hours there is no chrome scan, no recruiting, no tithe, no gathering. She sleeps in the Chapel and sees none of her 20 members. The only Purist code is passive: recruit filter, expulsion, shakedown order, tithe size, no purchases (`systems/creeds.rs`, `gang.rs:578` and `:668`).
   - **Why it matters:** the creed changes arithmetic, not a day. Anti-chrome should be visible on the street.
   - **Likely cause:** creed is a tag read by gang functions. No goal reads it.
   - **Fix:** Purist goals: Preach (chat a chromed target into a Persuade or Threaten), Patrol (walk the home district, note the chromed, feed Shakedown or Hunt), Gather (members at the Chapel at dusk, which feeds belonging). A chromed sighting should raise a Purist's Fight score.

9. **Social goals fail and belonging stays empty.** (shallow)
   - **What:** Imani's belonging is 0.00 all through d26 and Socialise still scores only 0.07-0.18. She walks 144 min, finds nobody, goes idle. Idris's Chat with Dante lasted 1 min.
   - **Why it matters:** the strongest unmet need stays unmet, and gang mates are the obvious company.
   - **Likely cause:** the partner is picked at plan time and is gone after the walk (finding 1). Socialise ignores gang mates. Her low sociability 0.23 also damps the score.
   - **Fix:** make the gang Hideout a Socialise venue with members present, and re-pick the partner on arrival.

10. **Flat midnight batches.** (shallow)
    - **What:** eleven of Idris's relationships flip Rival to Acquaintance (+0.58) in the same minute at 00:00 d21. His dread rises to 0.89 on d22 while he sits in a cell. Hours of idle Rest in a bar.
    - **Why it matters:** batch changes read as a pass, not a life, and a number rising with no deed behind it is hard to trust.
    - **Likely cause:** the daily reputation and edge pass.
    - **Fix:** log which deed moved the number, and spread edge changes through the day.

## Top 5 changes, ranked
1. Cut travel: weigh distance in goal and target choice, anchor life to district, Hideout and squat (finding 1).
2. Give rank 2 its own goals: Muster, Collect, Parley, retinue travel, delegated violence (findings 2, 6).
3. Make jail a place: keep the pin and Full LOD, add cell actions, collapse MetInJail (finding 4).
4. Give creeds goals: Purist Preach, Patrol and Gather, plus gang-mate Socialise (findings 8, 9).
5. Add income and order dwell: dues into the treasury, a 24 h minimum per order, a readable BreakOut muster log (findings 3, 5).
