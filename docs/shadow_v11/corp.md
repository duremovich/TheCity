# Shadow V11: corp (ceo, exec, guard, reporter, office_worker)

Critic: corp. Window: 100k island, seed 42, days 21-27, `D:/TheCity/target/shadow_v11/isle100k_s42_d21/`.
Picks: `ceo_20314` (V10's again), `ceo_88522`, `exec_76109`, `exec_81568`, `guard_27183`, `guard_8360`, `reporter_61680`, `reporter_61951`,
`office_worker_18717`, `office_worker_88012`. Read by grep of DAY SUMMARY, event lines and the Camera tables, then d21-22 / d24 of guard_27183,
d25-26 of reporter_61951, d24-26 of ceo_88522. d27 is the city's rest day (guards and some still work it).

## 1. Verdict

Better than V10: the corp commute and workplace bugs are mostly gone (CEOs and execs now work at their own HQ / plant, office workers travel 2-3 h, not 9-10),
a guard's arrest is a 1-hour escort instead of 15 hours, and two picks have a thread that starts (a rival's beat on a CEO, a reporter jailed three days for
stripping a corpse of nothing). It is still not a corp life: the desks are clocks, the stories that start (the beat, the jail) are invisible to the person
they happen to, and guards sit nine hours in a cell block while the street goes by.

## 2. Per archetype

- **CEO: alive on one, thin on the other (V10: alive / thin).** `ceo_88522` Bertram Carrow (Zetatech 6) works a Lab, goes to clubs, sleeps in capsule hotels, and a rival
  posts a beat on him: `d24 00:09 ContractPosted: Arasaka 15 posted a beat on Bertram Carrow for 482 at Fixer#5806 (rival over the poaching with Zetatech 6)`, taken
  `d24 18:00`, failed `d25 04:00 (the crew never reached the door)`, taken again `d25 18:00`, failed `d26 04:00`. He never hears of it (bug 3). `ceo_20314` Jinx Oakes
  (Militech 8) is the V10 table-sitter: 7.5 h Meeting at Corp HQ#5262, nightly locate+tag, and a lovely small beat, `d23 00:00 Loan: Hana Ambrose begged Jinx Oakes for 203c toward a vat-cancer treatment; Jinx Oakes lent 203c`, again `d26 00:00` (193c).
- **Exec: thin (V10: thin).** `exec_76109` Zara Haas (67, Greenline 17): 8.8 h Meeting at Packing Plant#5471, steals posted at midnight, and one real hit,
  `d24 00:09 Defrauded: Zara Haas learned the job Cordelia Wozniak sold for 40 never existed` then `GrudgeFormed ... (a betrayal, 0.60)`. `exec_81568` Roderick Kemp: 7.2-9.7 h
  Meeting at Packing Plant#4759, but 5-10 h/day travel, and his only events are `WitnessReacted: told 3 of his people what Kira Sato did (the Theft)` (d23 11:00, d25 16:23, d26 12:48), three near-identical lines.
- **Guard: alive on one, thin on the other (V10: alive / thin).** `guard_27183` Cordelia Quarry (home D3, Precinct D2) arrests three times, each a plan chain
  `PLAN Arrest ... [GoTo(SuspectTile) > Arrest > Escort]`: `d21 11:32 Arrest ... Odessa Jessop`, escort 72 min; `d23 09:45 Mags Osgood`, escort 63 min; `d27 13:04 Liesl Delacroix`, escort 156 min.
  `guard_8360` Zane Wexford (night shift): 24 Witness events, no arrest, one `d23 08:50 GrudgeFormed: swore against Quinn Eastwick (a robbery, 0.60)` after a rumour; his shift is `ACTION GuardJail ... 538 min` ×2 and 360 min.
- **Reporter: thin, but not dead (V10: thin).** `reporter_61680` Fenn Ironside: 8.8 h at Feed#5599, 43 ClerkWork, one odd `d21 09:02 PLAN Work on Block#2891 [GoTo(TargetHome)]` (a 118-min,
  113-tile trip to a house, then back 118 min), `d25 00:01 Promoted: promoted to Reporter at Santoro Feed` (he already held `Job: Reporter`). `reporter_61951` Quinn Novak interviews a source every
  day (`d21 12:02`, `d22 12:03`, `d23 11:28`, `d24 12:03 interviewed ... (a source)`) but no Story line ever; then she strips a corpse (`d25 03:57 Stripped ... (0 coins)`), is reported by a witness, arrested `d25 05:14`,
  `d25 06:18 Sentence: sentenced to 3 days for Theft`, and `d26 00:07 Cells: Quinn Novak missed the shift as Reporter in the cells (absence 1 in 14 days)`. First real corp-side consequence chain since V10's exec_81688.
- **Office worker: thin, improving (V10: thin).** Both commute 2-3 h (V10 9-10 h) and work 8.8 h at Corp HQ#5266, then 3-5 h of leisure. Events: `office_worker_18717 d22 04:44 Assault: Verity Upton attacked Idris Sato`
  (a lawfulness-0.81 woman, no lead-up in the lines read), three `WitnessReacted` ("saw ... Abduction and ran"), both `d27 AssetBought: Motorcycle T1 ... for 40 on finance` at Garage#5455 on the same day. Two coworkers at one desk, nothing between them.

## 3. Round 2's mechanisms seen at 100k

Fired in the group:
- **R2 walking:** guard escort 935 min -> 63-72 min; office workers' commute 383 min -> about 80 min each way; `reporter_61680 GoTo(Workplace) 118 min, 113 tiles` (about 1 tile/min, still slow but half of V10's ratio).
- **Hq as the office (R6b):** every CEO/exec Work plan targets a real building: `ceo_20314 d21 08:34 PLAN Work on Corp HQ#5262 [GoTo(Seller) > Meeting->Corp HQ#5262]`, `exec_76109 ... Packing Plant#5471`. The street-market-stall workplace is gone.
- **Gangs' hits (R5/R6b):** the beat on `ceo_88522` (above), two attempts, both failed on "never reached the door".
- **R4 folds:** meeting memories are gone (ceo_20314 has 0 `at the table` lines; V10 had 401). One `Served: decided ... would keep Grow in Tech; at the table: ...` line per sitting instead.
- **R4 friendship / witnesses:** `WitnessReacted` lines (told 3 of his people / saw ... and ran) appear in four of ten diaries and read well.
- **R1 fares (ladder):** `guard_27183 d21 12:58 -1c (Fare), purse 15c; flows: Fare -1 to the City`, a guard pays a fare. 120 of her 132 Witness lines are Fare Evasion and none produces a report or arrest.
- **R6a cab:** every corp pick hails a cab (`Served: ... hailed a cab (flush) from D4 Mid-Levels to Capsule Hotel#5234`); it also shows the wealthy sleeping in hotels instead of home (below).
- **R3 sick/loan:** `ceo_20314 d23 00:00 Loan` (203c to a vat-cancer patient). Beautiful, and a CEO-side story.
- **Reporter's beat story (R6b):** interviews fire daily (`reporter_61951`), no Story line in either reporter.

Expected, never seen: a reporter Story, a guard bribe or fine paid to a guard, a wounded guard / rescue (R3: nobody in the group was wounded), a gang Raid on a corp pick, a court quorum, a Fixer visited in person (every post still at 00:06-00:09 with no walk), a gang muster touching a corp pick.

## 4. Bugs (ranked by what the player sees first)

1. **Guards on jail duty spend their whole shift in a cell block.** `guard_8360 d23 21:02 ACTION GuardJail @ Precinct#4485, 538 min (ended)` (again `d26 21:02`, `d21 00:00 360 min`); `guard_27183 d24 09:04` and `d26 09:02` the same, 536 and 538 min. Nine hours with `max still 536m work` and no event, while the same guards on patrol (d21-23, d27) see 120 Fare Evasions in 130 witness lines. A single action that ends only when the shift does. Likely cause: GuardJail is one 9-h action with no interrupt for a witnessed crime. V10 bug 2 (escort walk) **fixed**; this is the new shape of the same dead police.
2. **`guard_27183` patrol ends at 13:43 with "not in shift".** `d22 13:43 plan interrupted: Earn unplannable: ExpansionCap` / `GOAL Earn ... because ... not in shift 0.00->0.30`, then 4 h of `PLAN Idle [Wander]` (14:03-17:05) and sleep at 17:37, though the header says shifts `[(540, 1080)]`. Same `d23 15:33`. The Camera table credits 1.8 h of work on d22-23 against 4.5-6.5 h on patrol; patrol is counted as travel and shift end is not 18:00. Either the shift math or the work accounting is wrong. New.
3. **A beat on a CEO is invisible to him.** `ceo_88522 d24 18:00 ContractTaken: The Static Widows took the beat on Bertram Carrow (LIVE, crew 4)` is logged from the CEO's file, and he goes to a club (`d24 17:52 UNWIND pick ... Club#5255`), a hotel and 12 h sleep, no safety drop, no guard bought, no `heard`. "The crew never reached the door" twice (`d25 04:00`, `d26 04:00`) while he sleeps at a capsule hotel in the same district: a hit cannot find a man whose location is public. New (R5/R6b mechanism holds but is toothless here).
4. **Wealthy corp picks sleep in capsule hotels, not home.** `ceo_88522 d25 01:57 ACTION Sleep @ Capsule Hotel#5234` three nights (and d23 `Sleep failed at CheckIn: PreconditionLost [bed Hotel: away_hotel(plan),no_free_bed]` at `06:06`); his home is Block#135 in D5 and he works in D4. `exec_76109` and `exec_81568` the same (Capsule Hotel#5473 / #4811, 370-440 min a night). Reads like a commuting man with a standing hotel habit; V10's ceo_91004 problem moved from "no desk" to "no bed". Worse than V10 for execs.
5. **Reporters report on nothing.** Both interview daily (`reporter_61951 d21 12:02`, `d24 12:03`) or walk to a house (`reporter_61680 d21 09:02 GoTo(TargetHome) 118 min`), but 0 Story lines; the tool that was supposed to give the reporter a beat story (R6b) produced an interview memory and a 4-hour round trip. V10 bug (reporters do not report) **unchanged**.
6. **`reporter_61680` "promoted to Reporter" while already Reporter** (`d25 00:01 Promoted`, header `Job: Reporter at Feed#5599`, new employer `Santoro Feed`). Text or rank bug. New.
7. **A fine-less, pay-less jail for a corp reporter.** `reporter_61951 d25 06:18 Sentence: 3 days`, 17.7 h jail d25, 24 h d26, d27; no wage consequence read in the diary beyond `Cells: missed the shift ... (absence 1 in 14 days)`, and a stripped body of 0 coins is a Theft arrest. Reads right for a harsh city, but the Cells line says "absence 1 in 14 days" with no foreman, no layoff track. V10 skipped-shift read-back **unchanged**.
8. **Guards still ignore what they witness.** `guard_27183` 132 Witness (120 Fare Evasion), 3 arrests, all from warrants (`GOAL Arrest ... because warrant located`), none from a witness; `guard_8360` 24 Witness, 0 arrests. V10 bug 6 **unchanged**. Arrests landing: **fixed** (3/3 completed escorts).
9. **Hourly gossip / witness reaction repeats.** `exec_81568` three "told 3 of his people what X did (the Theft)" lines (d23, d25, d26): same text, three different thieves, all Theft. Minor.
10. **Hits still a nightly ticker.** `ceo_20314` 4 ContractPosted lines in 7 days (V10 9), `exec_76109` 3 (V10 11): **better**. A midnight-posting pattern remains (00:06, 00:09).
11. **Posted hits: "posted a guard on X" self-hire text:** not seen this window (V10 bug 9 not reproduced).

V10 bug status: 1 (wounded guard bleeds, no goal) not testable, no wounded pick. 2 (arrest = 15 h walk) **fixed**. 3 (CEO workplace = stall) **fixed**. 4 (slow walking) **better** (about 2x). 5 (layoff, 4-minute rehire) not seen. 6 **unchanged**. 7 (rich exec evades fare) not seen. 8 (meeting spam) **fixed**. 9 not seen.

## 5. Dull spots (ranked; each a mechanism, not a knob)

1. **Desks are clocks.** 7.5-9.7 h of Meeting a day with one `Served: decided ... would keep Grow in Tech` line a night (`ceo_88522` the same decision three nights running). Mechanism: an attendee with a grudge, debt, or rival's pay pushes a motion and the vote is shown; a rejected one makes a grudge, an accepted one reaches someone the CEO knows (the Grow flip landing on a fired tenant).
2. **Threats to the powerful are invisible.** The beat on `ceo_88522` leaves no mark. Mechanism: a contract on you reaches you as a rumour (the fixer's tip), you buy a guard or move hotels, and the crew's failure ("never reached the door") becomes a heard event.
3. **Reporters collect, do not print.** Mechanism: an interview + a witnessed crime becomes a Feed Story naming the reporter, moving the subject's heat or the corp's standing; a reporter who prints about their own corp gets the skip/laid-off track.
4. **Guard jail duty as a block.** Mechanism: jail duty is a post with a rota; the guard stands the watch, gets called out when a cell fills, and the long action splits at each hour so a witness can interrupt it. A prisoner scene (the reporter in the cells meets the guard).
5. **Office workers have no workplace life.** Mechanism: lunch hangout in the HQ canteen, a coworker friendship that forms and a rival promotion (the two co-workers at Corp HQ#5266 live at the same Block#650, so they could be tied, as neighbours).
6. **Rich picks sleep in hotels.** Mechanism: a CEO at a hotel for a week should have a reason (spouse fight, eviction rumour, a threat); or give home beds priority when a bed exists.
7. **Law has no price for the rich** (no rich pick arrested this window): unchanged from V10; keep the bribe scene as the next pass.

## 6. Too much / broken-looking

- **Guard shifts as 9-hour blocks (bug 1)** and **patrol ending mid-afternoon (bug 2)**: read as broken police, not harsh ones.
- **Execs and CEOs on 10 h of travel:** `exec_81568` d21-24 travel 7.8-10.0 h with 7-9.6 h of Meeting; `exec_76109 d21` 11.1 h travel; commute and hotel hops. Better than V10 for ceo_91004 but `exec_81568`'s 7.6 h/day travel is still a day lost.
- **FINDING, work hours d21-26:** `ceo_20314` 7.0-7.8 meeting, `ceo_88522` 0-8.5 (d23 off), `exec_76109` 5.0-9.4, `exec_81568` 7.2-9.7, `guard_27183` 1.2-9.0 (1.2-1.9 on patrol days), `guard_8360` 1.5-6.7, `reporter_61680` 8.7-8.8, `reporter_61951` 8.7 (jailed d25), office workers 8.7-8.8.
- **FINDING, travel hours d21-26:** `ceo_20314` 1.8-3.8, `ceo_88522` 2.8-6.7, `exec_76109` 1.2-11.1, `exec_81568` 5.2-10.0, `guard_27183` 2.6-8.9, `guard_8360` 3.3-8.0, reporters 2.3-4.7, office workers 1.8-2.8.
