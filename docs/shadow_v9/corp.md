# Shadow V9: corp (ceo, exec, guard, reporter)

Critic: corp. Window: main `4108184`, 100k island, seed 42, days 21-27, `D:/TheCity/target/shadow_v9/isle100k_s42_d21/`. office_worker has no candidates at 100k.
Read: `ceo_35958` (d21-27 by grep, d23 and d26-27 in slices), `ceo_91004` (summaries, d21-27 GoTo/GOAL lines), `exec_80297` (d21, d23), `exec_89966`
(d22), `guard_8312` (d21, d23, d27), `guard_105381` (d22, counts), `reporter_63440` (d22, d24), `reporter_85999` (d24, d27); per-day numbers from `camera.tsv`.
Day 27 is a rest day for everyone in the group (work 0.0 h), so "work 0" on d27 is not a bug.

## 1. Verdict

The corp group reads as people who spend their days getting to their desks and not as people living lives. The V8 commute bug is unfixed and worse at 100k: a short same-district
errand costs 2-7 h each way, and the leg is cut or arrives with nothing to do. The jobs underneath are thin: no reporter ever reports, no guard ever completes an arrest on a
suspect that stands still, an exec's meeting is a clock that produces no decision. The few real stories (a corp posting a hit every night, a CEO reported for a Shakedown, a guard losing a fight to a wanted man) are in the diary but nothing follows them.

## 2. Per archetype

- **CEO: alive on one, dead on the other.** `ceo_35958` Rex Inchbald, Nutrix 14: meets 9-14 h a day at Lab#4998 (d23 `Meeting` 802 min, 02:50 to 16:12), 4 contract-posting events, a Therapy visit, a fight.
  `ceo_91004` Dagger Novak, Vatra 9: work 0.0 h all week, vehicle 12-19 h a day, `d21 06:40 GoTo(Seller) -> cut at street (D4 Mid-Levels), 634 min (ended) [target Street Market#4185]`, repeated d22-d26 (401-526 min). Dead.
- **Exec: thin.** `exec_80297` (Arasaka 39, ExecWage +10c) works 5.4-7.7 h but lives 265 min from her Lab (`d23 05:17 GoTo(Seller) 260 min`, `d23 17:17 GoTo(Home) 266 min`); `exec_89966` (Zetatech 37, treasury 1,333c, Hunker)
  travels 13.1 h a day for 2-5 h of Meeting (`d22 08:37 GoTo(Seller) -> Lab#5669 (D3), 394 min (done)`, `d22 17:02 GoTo(Home) 324 min (ended)`), vehicle none.
- **Guard: thin.** `guard_8312` (Arasaka 47, 8c/day): work 0.2-1.2 h, the shift is a walk between waypoints (`d23 22:17 GoTo(PatrolWaypoint) 111 min`), but one real beat: `d21 22:25 Arrest: Sebastian Delacroix arrested Ines Oakes`
  and `d27 02:00 Assault: Ulric Upton fought off Sebastian Delacroix` (three tries, Rival then Enemy, `d27 02:31 GOAL Flee`). `guard_105381` (Precinct D2, home D1): 0 arrests, see bug 3.
- **Reporter: thin.** Both hold the shift (work 7.7-9.2 h most days) and both spend it as a `ClerkWork` clerk at a Feed (`reporter_63440`: 42 ClerkWork actions, 0 Interview or Story in the week). Closest thing to reporting is
  `d24 09:15 PLAN Work on Block#510 [GoTo(TargetHome)] ... 180 min (ended)`: a 3 h walk to a stranger's flat and straight back with no scene at the door.

## 3. Bugs (ranked by what the player sees first)

1. **The commute is still geometry, not choice (V8 bug 1, unfixed).** Home and workplace share a district label and sit hundreds of tiles apart; GoTo legs of 260-790 min are normal.
   - `ceo_91004 d22 10:33` leaves for a market meeting that is cut at 17:14 by hunger; he never arrives, any day of the week (`d21 06:40`, `d22 10:33`, `d23 08:28`, `d24 09:32`, `d25 09:16`). Travel 15.9-19.5 h on d21-25.
   - `ceo_35958 d27 09:18`: `Visit` the Fixer in D3 from D4, `790 min (ended)`; the moves run D4, D6, D4, D6, D1, D3, D4, D3 (`d27 11:27` to `d27 20:11`), then `PLAN FAILED ... BuildingFull` twice. A tour of five districts to reach the neighbouring one: the El loop route, not the shortest.
   - `exec_80297 d21 00:00 Housed ... Block#312 (D4)` while her Lab and Hq are in D6: she was placed 265 min from work, so every workday is 9 h of travel.
   - `ceo_91004 d25 17:14 GoTo(Market) 329 min (done)`, `d25 22:17 Starving` with 500c and a 58k treasury: the nearest food is a five-hour trip. The first starving CEO in the game is a placement and reach bug.
   - Likely causes: the `exec_ride` same-district gate and `transit::choose` accepting loop routes (V8 corp.md section 1); nothing keeps a placed home near the Hq or Lab. `PRESSURE_PASS` phase 4 (reach) is the right fix for the food and Fixer trips.
2. **A CEO's "Work" goal targets a street-market seller.** `ceo_91004 d21 06:40 PLAN Work on Street Market#4185 [GoTo(Seller) > Meeting->Street Market#4185]`. The exec's workplace for this CEO is a stall in another district, not an Hq or Lab. His Job line reads `none`. Seen on every day d21-26.
3. **Guard Arrest never lands on a moving suspect.** `guard_105381`: 16 Arrest plans in the week, 13 `PLAN FAILED ... Arrest: PreconditionLost` (`d22 12:15`, `12:59`, `15:40`, `17:48`), 0 `Arrest` events. Each Arrest action lasts 1 min (`d22 12:14 ACTION Arrest ... 1 min`) after a 23-56 min approach; the target has moved, the plan retries on the same suspect (`Zoe Bartlett d22 15:21` and `15:41`). Contrast `guard_8312 d21 22:14`, where the arrest on a stationary target at home succeeds.
4. **Guard witnesses crime and does nothing.** `guard_105381` logs 119 `Witness` events, 113 of them Fare Evasion (`d22 12:36`, `d22 16:14`), with no ReportCrime, Warn or Chase. The "warrant located" Arrest goal fires only for warrant suspects; a guard standing next to a crime is invisible to the law.
5. **Guard food trip leaves the beat.** `guard_8312 d23 06:02 GOAL Eat -> GoTo(Market) Street Market#4266 (D4)` from D1 (47 min out, 169 min back, `07:13 ... cut at street (D1)`). Wage 8c/day, meal 12c; net 0c most days, malnutrition 0.27, toxicity 0.50.
6. **`GuardJail`/Escort as a single long action.** `guard_8312 d21 22:25 ACTION Escort @ Precinct#4485 (D2), 357 min (ended)` (V8 bug 4/5, unchanged).
7. **Ping-pong at a district border.** `ceo_35958 d23 01:16 to 02:45`: D4, D5, D4, D5, D4 inside one 99-min GoTo (`d26 03:26 to 03:36` the same).
8. **Hourly meeting memories are spam.** `ceo_35958 d23 09:00 to 16:00`: the same 6-9 attendees "met at the table (an attendee), valence +0.10" every hour, one memory each per hour; the Meeting ends with no decision line except the nightly `Served` event.
9. **Reporter's Job/Feed has no story hook.** `reporter_63440 d25 00:07 Served: Lena Voss decided Nutrix 2 would Acquire in Food` is the whole Meeting output; the reporter in the diary (a Feed employee at the table) does not write it up.

## 4. Dull spots (ranked; each a mechanism, not a knob)

1. **Corp hit campaign with no consequence.** `ceo_35958` posts a `locate` (33c, at Fixer#5545) plus a `tag` (about 245c) on a named person almost every night: Viktor Blackwood d21, Axel Yates d22, Santiago Tanner d24, Raven Sallow d25, Mateo Quarry d27. Each tag shows `Taken, Live`. The CEO is never told the outcome, no victim, rival or witness shows it, and the Fixer is never visited. Mechanism: when a taken tag resolves, give the poster a Result memory and event, and give the victim's circle a grudge against the named corp and its CEO; let the CEO attend the Fixer in person when the contract is large.
2. **A CEO is reported for a Shakedown and nothing escalates.** `ceo_35958 d26 00:06 Report: Philippa Oakes reported Rex Inchbald for Shakedown`; the same minute a failed `Intimidate` on her (`stake Job wage 5`), then `d26 00:09 ContractPosted: Rex Inchbald posted a guard on Rex Inchbald for 42`. This is the best story in the group and it ends there. Mechanism: the report drives a Hunt or an eviction/firing of the worker, and a real guard-for-hire turns up at his flat.
3. **A reporter who wants to join a gang.** `reporter_63440` is poor (wage 5, purse 0c, mood -0.23 by `d25 20:00`) and low-lawfulness; `JoinGang` is a runner-up every day and the top goal at `d24 18:02`, `d25 19:44`, `d21 21:06`. The walk to the Hideout (`d24 18:03 GoTo(Hideout) 137 min`, cut) is always ended by `Sleep`. 7 JoinGang plans, 0 joined. Mechanism: carry the intention overnight so the walk resumes at the next free hour, or a gang scout/runner meets him at a bar; a reporter inside a gang is a story-maker.
4. **Reporters should report.** Give the Feed a beat: Witness events (`reporter_85999` logs 9) become story seeds; the reporter interviews the subject at a `TargetHome` visit (`d24 09:03` today ends at an empty door), and publishes a `Story` event that names the reporter (V8: names only the arrester). The stories change standing and heat of the people in them.
5. **Corp meetings as plots.** 9-14 h of table time (`ceo_35958 d23`) should yield a decision per meeting (hire, buy, hit, merge, evict) from attendees' ambitions, a rival attendee who pushes back, and a result that reaches the exec's life (a windfall, a demotion, a rival's grudge).
6. **Exec wealth and place.** `ceo_35958` earns ExecWage +40c against a 67,255c treasury; `ceo_91004` sits on 58,506c with 500c in hand and starved. Mechanism: CEOs draw a dividend or carry company expenses; the corp pays for a driver, a company flat near the Hq, or a meal allowance. Execs of poor corps (`exec_80297` +10c, `exec_89966` Hunker) show the harsh version; the top should look different.
7. **Guards should police the street they walk.** The patrol (`guard_8312`) crosses its beat in 100-minute legs and sees only what the Witness event shows. Mechanism: a patrol stop that checks bystanders (fare evasion, loitering, unlicensed chrome), with a fine, a bribe or a beating (a bribe to a poor 8c/day guard is a story and income). The Arrest goal should also pick the suspect the guard witnessed.
8. **Guard-vs-wanted fights.** `guard_8312 d27 02:00 to 05:34` is the only real story: three tries on Ulric Upton, a loss each, Rival then Enemy, then back to Patrol. Mechanism: call for backup (another guard on shift near the tile), or a lasting scar/fear so the guard stops pursuing; a revenge arc for Ulric.
9. **Rich commute should look different.** `ceo_91004` has 12-19 vehicle hours a day and still never arrives, so a vehicle is not a fast one here; the Flyer in his kit (`vehicle Flyer T1`) should make the rich commute short and a status mark.

## 5. Already in `PRESSURE_PASS.md`, seen here

- **Phase 4 (reach table):** the evidence is strong: 329-min food trip (`ceo_91004 d25 17:14`), 790-min Fixer visit (`ceo_35958 d27 09:18`), 169-min return from a market (`guard_8312 d23 07:13`), 160-200 min "doorstep" walks in a single district (`reporter_85999 d24 09:03`). A reach cap per purpose would cut all of them; commute reach (home to workplace) needs its own row.
- **Phase 1 item 5 (recruitment of the miserable):** `reporter_63440` has the JoinGang goal alive every evening but the walk never completes (bug in section 4.3). Phase 1 should make the arrival, not just the goal, happen.
- **Phase 1 item 4 (quitting), 1 (absenteeism):** `guard_8312` and `exec_89966` have 9-13 h of travel, 4c-a-day nets and low pay; they are the quitters the phase would produce. No skipped shift appears for any corp pick; CEO/exec have no wage mechanic to skip.
- **Phase 3 (the Fixer's parties come in person):** `ceo_35958` posts at `Fixer#5545` each midnight (`d21 00:06`) without ever walking there. His only Fixer trip, d27, fails at the door (`BuildingFull`).
- **Phase 2 (dead trades):** not corp.

## 6. FINDINGs (printed)

- Work hours by pick, d21-26: `ceo_35958` 9.4-14.5 h, `ceo_91004` 0.0, `exec_80297` 5.4-13.2, `exec_89966` 1.9-7.5, `guard_8312` 0.2-1.1, `guard_105381` 0.0-3.8, `reporter_63440` 2.4-9.0, `reporter_85999` 2.8-9.2.
- Travel hours d21-26: `ceo_91004` 15.1-18.6 vehicle plus street; `exec_89966` 13.1 street; guards 10-16.
- Corp treasuries: Vatra 9 58,506c, Nutrix 14 67,255c, Zetatech 37 1,333c, Arasaka 39 2,518c. The rich corp's CEO personal wallet is 40-500c.
