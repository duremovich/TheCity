# Shadow V10: corp (ceo, exec, guard, reporter, office_worker)

Critic: corp. Window: main `33f9c55`, 100k island, seed 42, days 21-27, `D:/TheCity/target/shadow_v10/isle100k_s42_d21/`.
Picks: `ceo_20314`, `ceo_91004` (V9's ceo_91004 again), `exec_74531`, `exec_81688`, `guard_26314`, `guard_8327`, `reporter_51264`, `reporter_57837`,
`office_worker_14234`, `office_worker_61227` (office_worker had no V9 candidates; new). Read by grep of summaries and events, then d21-22 (guards), d22-23
(exec_81688), d25 (reporter_57837), d26 (guard_8327, ceo_20314). Day 27 is a rest day for the group (work 0 h is not a bug).

## 1. Verdict

Better than V9 and still not a corp life. The commute bug is mostly cured for CEOs, execs and reporters (ceo_91004 now arrives and eats; exec_74531 travels 0.3 h a day),
guard arrests now land, and for the first time a corp pick has a chain of consequences (exec_81688: reported, flees, fights a guard, is arrested, fined, defrauded, buys a guard)
and a guard dies of a wound in the open. What is left is that the jobs are still clocks: CEOs and execs sit 7-14 h at a table or a street-market stall and decide one thing a night,
reporters clerk, office workers spend 9-10 h a day walking, and a guard's arrest costs him 15 hours of walking.

## 2. Per archetype

- **CEO: alive on one, thin on the other (V9: alive / dead).** `ceo_20314` Jinx Oakes (Militech 8, treasury 126k): 7-9 h `Meeting` at his own HQ in D5, a spouse and seven friends, a
  nightly Fixer contract, and a real decision on the page: `d25 00:07 CorpOrder: Militech 8: Grow in Housing -> Secure in Housing (shock, 0.85 vs 0.82, decided by Jinx Oakes)`, and
  back to Grow `d27 00:07`, with the six at the table named. `ceo_91004` Dagger Novak (Vatra 9, 108k treasury): now reaches his desk and eats, but the workplace is still
  `Street Market#4185` (`d22 03:44 PLAN Work on Street Market#4185 [GoTo(Seller) > Meeting->...]`), he sleeps in a capsule hotel, wanders at night, and works 03:44 to 17:14.
- **Exec: thin (V9: thin).** `exec_74531` Elowen Upton (Vatra 11): 8.2 h meeting at a street market, 9 h idle at home, 0.3 h travel, a posted hit each midnight (`d23 00:06 ContractPosted: Vatra 11 posted a locate on Amara Oakes for 39 ... tag 290`); nothing touches her. `exec_81688` Fenn Kettering (Greenline 12): alive for three days, see section 3.
- **Guard: alive on one, thin on the other (V9: thin).** `guard_8327` Juniper Varley (Precinct D2, home D1, married `d21 05:06`) arrests `d21 14:07 Slate Wozniak`, loses a fight and dies: `d26 13:24 Wounded: Juniper Varley went down, bleeding, at Hex Brandt's hands`, `d26 16:24 Death ... BledOut`, `d26 16:35 Stripped: Rin Novak stripped the body (8 coins)`. `guard_26314` Dmitri Carrow (Arasaka 3, D4) makes three arrests (`d22 00:21`, `d24 01:00`, `d25 22:09`), each followed by a 12-15 h escort walk (bug 2).
- **Reporter: thin (V9: thin).** Both hold the shift (8.6-9.4 h) as a `ClerkWork` clerk, 34 and 37 clerk actions, 0 interviews, 0 stories. One skip fired: `reporter_51264 d23 08:00 Skipped: Quill Northcott skips the shift as Reporter: sets off for it, then does not go in; walks the streets instead (mood -0.11, misery 0.28; absence 1 in 14 days)`. `reporter_57837` is laid off and re-hired in four minutes (bug 5).
- **Office worker: thin (new).** Both work 7-8.6 h every day (46 and 49 ClerkWork) and the diary has no event beyond nightly rumours: `office_worker_14234 d21 04:09 GoTo(Workplace) -> Corp HQ#5226 (D5), 383 min, 176 tiles`, home D4, so 9-10 h of travel for 7 h of desk. A person with a pulse and no plot.

## 3. The pass's mechanisms seen at 100k

Fired in the group:
- **Skipped shift (phase 1):** `reporter_51264 d23 08:00` (above). Reads well. No consequence follows (no docked wage, no foreman line; he goes home and hangs out).
- **Layoff (phase 1):** `reporter_57837 d25 00:07 LaidOff ... (laid off: revenue, a shift past full staff) (told by Rosalind Vasquez, the foreman)`.
- **Hit result memory (V9 dull spot 1, partly answered):** `ceo_20314 d26 00:06` posts a locate and a tag on Freya Garland and the same minute logs `heard a rumour: Hired by corp Militech 8 on Freya Garland (hops 0, conf 1.00)`; same for Marek Volkov `d27 00:06`. The poster now learns the outcome, as a hire. No victim-side grudge seen.
- **Guard on the payroll (phases 3/5b):** `exec_81688 d25 18:00 GuardTaken: Hana Kowalczyk is on Fenn Kettering's payroll`, after `d23 00:09 ContractPosted: Fenn Kettering posted a guard on Fenn Kettering for 52`.
- **Fixer fraud (phase 3):** `exec_81688 d23 00:09 Defrauded: Fenn Kettering learned the job Cordelia Wozniak sold for 40 never existed`, then `GrudgeFormed ... swore against Cordelia Wozniak (a betrayal, 0.60)`. A good beat.
- **Report, flight, arrest, fine:** `exec_81688 d22 17:43 Report: Quinn Sallow reported Fenn Kettering for Fare Evasion`; `d22 18:28 PLAN Sleep [FleeToHome > Sleep]`; `d23 11:47 Assault: fought off Fiora Yardley`; `d23 11:58 Arrest`; `d23 14:42 Sentence: fined 5 coins`. The whole chain happens. A 5c fine on an exec reads as nothing.
- **Weapons licence:** `ceo_20314 d27 00:07 LicenceSold: Jinx Oakes bought a weapons licence for 60 from Priya Bartlett`.
- **Wound, bleed-out, stripping:** `guard_8327 d26`. Fired.

Expected, never seen: a Shakedown against a CEO (V9 had one), a gang raid touching these picks (`Raided by gang The Wire Hounds on corp Militech 8` arrives only as gossip to exec_74531 `d24 18:26`), a Fixer visited in person (every post is at midnight with no walk), a bribe or fine paid to a guard, a court quorum, a reporter writing a Story (the nine `Story` lines in ceo_91004 are the Civic Wire writing about the CEO's own hire).

## 4. Bugs (ranked by what the player sees first)

1. **A wounded guard bleeds for three hours with no goal.** `guard_8327 d26 13:24 Wounded` to `16:24 Death`: the needs lines read `Full | None | street (D1 The Sump)` every hour, he "witnesses" three Fare Evasions while bleeding, sees his attacker Hex Brandt again at `15:43`, and nobody (spouse, medic, bystander) acts. Likely cause: no downed-state plan (call for help, crawl home, find a ripperdoc) and no spouse trigger. Death itself is fine; the dead air is the bug. New.
2. **An arrest costs the guard 12-15 h of walking.** `guard_26314 d22 00:21 ACTION Escort @ Precinct#4485 (D2), 935 min (ended)`; he goes D4, D5, D4, D2 (`d22 03:05`, `07:33`, `14:35`, `15:56`), then sleeps in a capsule hotel in D2 and again d23-d25 (`d23 12:34 PLAN Sleep [GoTo(Hotel) > CheckIn > Sleep]`), seldom reaching his own bed. Travel 13-19 h/day, patrol "work" 0.0-0.4 h. V9 bug 6 (escort as one long action, loop route) **unchanged**, but with arrests landing it now hits every night.
3. **CEO and exec Work still targets a street-market stall.** `ceo_91004 d22 03:44`, `exec_74531`, `exec_81688`: `PLAN Work on Street Market#NNNN [GoTo(Seller) > Meeting->...]`, Job `none` on all four. V9 bug 2 **unchanged**. `ceo_91004` runs a nocturnal day as a result: hotel at 17:14, seven `PLAN Idle [Wander]` in a row `d22 00:07-03:19`, then 03:44 to the stall.
4. **Walking is far slower than the tile count says.** `ceo_91004 d21 07:20 GoTo(Seller) ... 311 min, 1543 tiles (done)` for a same-district stall (later legs 182 tiles, 160 min). `office_worker_14234 d21 04:09` walks 383 min for 176 tiles against a header that promises `182 tiles, travel 172 min`. V9 bug 1 is **better** (no starving CEO, exec_74531 no travel, reporters commute 15-45 min) and **worse** for office workers and `ceo_91004` d21.
5. **Layoff, then rehire to a worse job in four minutes.** `reporter_57837 d25 00:07 LaidOff`, `d25 00:11 Hire: hired as Office Worker (picked by Kieran Bellamy, the foreman)`. The new desk is Corp HQ#5226 in D5 (`d25 04:34 GoTo(Workplace) 325 min, 267 tiles`), so she travels 11 h a day on d25-26. No unemployment, no worry, no search; wired but it reads as a shuffle.
6. **Guards still ignore what they witness.** `guard_26314` 27 Witness events, `guard_8327` 24, almost all Fare Evasion; `ReportCrime` appears only as an unused runner-up (`guard_8327 d25 18:17`). V9 bug 4 **unchanged**. Arrest on a moving target is **better**: `guard_26314 d22 00:10 PreconditionLost` then `00:21 Arrest`; `guard_8327` 4 failures, 1 arrest; V9 `guard_105381` was 0 of 16.
7. **A rich exec evades the fare.** `exec_81688 d22 17:43` (wallet 179c, treasury 5.4k) is reported for Fare Evasion, flees home for 392 min, and is later fined 5c.
8. **Hourly meeting memories still spam.** `ceo_20314`: 401 `met X at the table (an attendee) (valence +0.10)` lines in a week, one per attendee per hour. V9 bug 8 **unchanged**.
9. **`posted a guard on Fenn Kettering`** reads as a man hiring a guard against himself (`d23 00:09`; V9 had the same for Rex Inchbald). Text only.

V9 bug 3 (arrest never lands): **fixed on stationary and mostly fixed on moving targets**. V9 bug 7 (district-border ping-pong): not seen. V9 bug 5 (guard food trip): not seen, guards ate near the beat.

## 5. Dull spots (ranked; each a mechanism, not a knob)

1. **Reporters do not report.** 71 `ClerkWork` actions across two reporters, 0 Story, 0 Interview; their Witness events (`reporter_57837 d22 14:17 saw Yorick Kowalczyk commit Shakedown`, `d27 08:29` told 3 of her people) are gossip, not news. Mechanism: a Witness a reporter logs becomes a Feed story seed; the shift walks to the scene and publishes a Story naming the reporter, which moves the subject's heat or the corp's standing.
2. **CEO and exec tables yield one decision a night.** Eight hours at a table, a `CorpOrder` flip every other night (`ceo_20314 d25`, `d27`) and a midnight hit. Mechanism: an attendee with a grudge, a debt or a rival's pay pushes a motion; a rejected one leaves a grudge, an accepted one reaches someone the CEO knows (the Housing flip landing on an evicted tenant).
3. **Hits are fire and forget.** Both CEOs and `exec_74531` post a locate and a tag every midnight; the result is a rumour that the target was hired. Mechanism: the target can refuse, which turns the tag into a hunt and puts a refusal in the poster's circle.
4. **Law has no price for the rich.** `exec_81688` is arrested and fined 5c, then buys a guard. Mechanism: fines scaled to wealth, or a bribe scene with the arresting guard (a bribe to an 8c/day guard is also his best income).
5. **The skip has no read-back.** `reporter_51264 d23 08:00` costs nothing and nobody notices. Mechanism: the foreman remarks on the next shift, wage docked, then the layoff track (bug 5).
6. **The arrest as a scene.** The nearest guard takes the suspect to the nearest cell and the arresting guard returns to the beat. Halves bug 2.
7. **Office workers have no workplace life.** 7-8 h at a desk, no coworker or foreman event. Mechanism: shift-end hangouts with colleagues in the building's own bar and a gossip line from the HQ.

## 6. Too much / broken-looking

- **Nightly hit postings:** `ceo_20314` 9 ContractPosted in a week, `exec_74531` 11, `exec_81688` 5. Reads as a ticker, not an event.
- **A guard's shift is all travel:** `guard_26314` 13-19 h travel on six days and 0.2 h of work; the police are a long walk. Reads broken (bug 2).
- **`ceo_91004` nocturnal schedule** (hotel at 17:14, wandering through the night) reads as a bug, not a CEO.
- FINDING, work hours d21-26: `ceo_20314` 7.2-8.9, `ceo_91004` 8.6-11.7, `exec_74531` 8.2-8.4, `exec_81688` 2.2-10.3, `guard_26314` 0.0-0.4, `guard_8327` 0.0-1.2 (died d26), `reporter_51264` 0.0-9.4 (skip d23), `reporter_57837` 4.4-9.1, `office_worker_14234` 7.0-7.8, `office_worker_61227` 8.6.
- FINDING, travel hours d21-26: `guard_26314` 13-19, `office_worker_14234` 9-10, `office_worker_61227` 7-8, `ceo_91004` 5-9 plus 3-6 vehicle, `exec_74531` 0.3.
