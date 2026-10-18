# Critique: gang members 2114 (Dante Tanaka) and 2198 (Axel Yardley), d20-d26, seed 42

Both are Ninefold rank-0 members. The "dealer" pick is a real dealer, but only one day of his week is free.

## A day in the life

**Dante Tanaka (52, Street, loyalty 0.87, greed 0.71)**
- d20: jailed 00:00-10:25 (Extortion). Released, picks GangWork, walks 6 h to Sump East. Extorts 16:08, takes 0 coins, walks 5 h home. Sleeps 21:30.
- d21: wakes 03:56 after 6 h. Order is Squat. Walks 6 h to a derelict, occupies it 10:10 (11 min), punches Adela Vance, walks 3 h to a bar, chats 2x30 min, walks 5.5 h home. Asleep 20:05.
- d22: up 01:31, walks toward another derelict, abandons it to eat, 2 h to a bar, drinks 46 min, buys stims, 3 h to a second derelict, 4 h to Civic Hall for 4c dole, 5 h home. Sleeps 22:56.
- d24: 21.4 h travel and nothing else. Walks toward a target, switches to Sleep, walks home, wakes for a muster, walks 4 h toward the muster point, U-turns, walks home.
- d25-26: bar-hops, then an "Idle" 7 h walk to the Hideout, only to sleep. Orders flip to LieLow and back to Squat.
- Week: 114 h travel, 34 h sleep, 4 h social, 1 h crime, 0 h work. One extortion (0c), two punches, two squats. Purse 23c to 12c.

**Axel Yardley (38, dealer, sociability 0.77, lawfulness 0.03)**
- d20-d21: in jail from tick 0, released 23:55 d21, walks home 4 h.
- d22: sleeps 04:16-11:31. Walks 3 h to the Hideout, picks up 10 stims, walks 4 h to the deal Bar. Deals 18:36-22:38, sells all 10 for 10c total. About 58 witness lines. A stranger reports him. Flees 22:38.
- d23: sleeps 111 min, is arrested at home 06:52, walks himself 5 h to the Precinct, gets 5 days. Jailed through the end of the run.

## Findings

1. **Travel is the whole day (bug, scale).** Dante walks 114 of 168 h, and 19-21 h on d22 and d24. Single GoTos are 4-6 h (d20 10:26; d26 11:14, 270 min). Matters because nobody spends 70% of life walking, and nothing else gets time. Cause: target choice ignores distance cost. `systems/gang.rs: gang_work_target` takes the unclaimed Home nearest the Hideout, `squat_targets` takes any derelict in the district, and `utility/goals.rs` has no travel term. Fix: add a travel-time penalty to target choice and to GangWork/Socialise scores, and check tile speed against district spacing.

2. **GangWork is a flat ~0.85 that overrides sleep (shallow).** d21 03:56, d22 01:31, d24 06:14 all read "GangWork (was Sleep): 0.856" after 3-6 h of sleep, with runners-up at 0.05-0.4. Cause: `utility/goals.rs` GangWork considerations are U(wealth), greed, "not in shift" (always 1.0 when unemployed), target and order. There is no energy or hunger term. Fix: multiply by an energy/hunger factor and stop it preempting Sleep at night.

3. **Sleep is fragmented and wrong (bug).** Sleep totals 2.5, 7.8, 2.5, 6.1, 2.6, 8.3, 4.3 h. "Sleep" plans are 4-5 h walks to Home (d20 16:31 to 21:30; d22 17:38 to 22:56), so he walks while energy falls to 0.16. Fix: Sleep should pick the nearest bed he can use (Hideout, held squat, own Block), not Home.

4. **Extortion is rare and pays nothing (shallow).** One Extort in 7 days, 21 min, 0c. `gang.rs: extort` returns 0 when `intimidate` fails (intimidation 0.17, dread 0.04), and the log still says "extorted 0 coins" with a Shakedown witness. Income is dole (4c) against 9c meals. A greed-0.71 thug who lives on handouts is not believable. Fix: standing daily tribute from claimed Homes, with a cut to the collector, instead of one 21 min trip.

5. **The gang never touches the gang (missing).** Dante visits the Hideout only to sleep, twice. No Hideout socialising, briefing, stash or drinking with members. Top contacts are strangers (Kasimir Bartlett, Ursula Zhou) and enemies, not one Ninefold member. Fix: make the Hideout a Socialise venue for loyal members and weight Chat targets toward gang-mates.

6. **Order churn makes orders meaningless (bug).** The order flips Squat, Expand, Squat, BreakOut, LieLow within about 36 h (d22 17:51, d23 03:27, 13:20, 23:07, d24 06:32, 09:28, d25 01:42). A 4-6 h walk can never finish before the next flip. The d24 14:14 muster costs 4 h of walking and a U-turn at 18:14. No raid happens. Fix: minimum order duration of 1-2 days, and let members already en route finish.

7. **Events do not change behaviour (shallow).** On d22-d24 Dante gets two beating grudges against him, heat 0.65, dread 0.70, and a refused bribe notice for the gang. He does not lie low, seek backup or retaliate. Gossip repeats: Ursula tells him the same Assault story three times in 70 min (d25 10:09, 10:41, 11:13, "already known / dropped"). Fix: dedupe gossip per pair per day, and feed grudges and heat into Flee, LieLow or ask-for-backup goals.

8. **Violence is opportunistic only (shallow).** Both fights (d21 10:23, d23 19:44) are "enemy near" Fight at about 0.35. There is no turf guarding, no shop intimidation, no escort for the dealer. Fix: a Patrol/Guard goal on held territory scoring on enemy sightings inside own claim.

9. **Jail is a free friend-making exploit (bug).** In 10 h of d20 jail Dante logs about 70 edge lines. Every Friend gains +0.2-0.3 affinity every 3 h until +1.00 (Philippa Zhou, Hiro Achebe, Kofi Novak, Percival Moreau, Rhoswen Garland). About 15 strangers become Acquaintances via MetInJail. Cause: the Coarse-LOD jail social tick (`systems/social.rs:537`) is a flat gain with no cost. Fix: cap daily affinity gain in a cell and make jail a gang-credibility event, with a grudge against the arresting guard.

10. **Edge kind and affinity disagree (bug).** Marek Bellamy, Delia Novak and Jocasta Crane show "Enemy" at affinity +0.60 (both headers). Adela Vance becomes "Enemy (was Friend), affinity +0.30" at d20 19:29. Fix: derive kind from one function of affinity and flags.

11. **Dealing is a poor, public job (shallow).** One 241 min Deal action in a Bar sells 10 stims for 10c (1-2c each, against `deal_price: 5` and a 9c meal). Each sale logs 6-13 witnesses until a stranger reports him. Fix: raise margins, limit witnesses to people who can identify the dealer, and make him relocate after a Report.

12. **Axel is arrested at home and walks himself to jail (bug).** Arrest at 06:52 d23, then a 5 h unescorted walk to Civic, sentenced 11:47. He is a free man for most of that walk. Fix: escort, or the arrest places him at the nearest station.

13. **Axel's week is mostly an artefact (tool).** He starts jailed (to d21 23:55) with a grudge from tick 0. Fix: the shadow tool should skip jailed agents when picking, or start later.

14. **Flee ends where it started (shallow).** After Flee at 22:38 he stops after 60 min, eats, then Socialise sends him back to the same bar (22:50) where the hostile was. Fix: Flee leaves an avoid memory on the venue.

15. **Missing entirely:** the spouse Moth Wexford (never visited), any odd job (Job none, Earn is just dole), recruiting, using the stims he buys (GetHigh 0.087), partying, fencing loot, spending the 4c treasury. Fix: a spouse/family visit goal, a Hideout party/stash goal.

## Top 5 changes, ranked

1. Travel cost in goal and target choice (finding 1). Most of the week is walking.
2. Energy/hunger term in GangWork, and Sleep at the nearest usable bed (2, 3).
3. The Hideout as home base for loyal members: socialise, sleep, stash, brief (5, 15).
4. Order hysteresis plus a tribute flow that actually pays (4, 6).
5. Reaction goals for heat, grudges and reports, and better dealer margins and discretion (7, 11, 14).
