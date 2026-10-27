# Shadow V12: corp (ceo, exec, guard, reporter, office_worker)

Critic: corp. Window: 100k island, seed 42, d21-27, main 6e1c87f, `D:/TheCity/target/shadow_v12/isle100k_s42_d21/`.
Picks (new indices): `ceo_20314` (again), `ceo_88522` (again), `exec_78363`, `exec_84610`, `guard_27347`, `guard_8440`, `reporter_57617`, `reporter_57702`,
`office_worker_21091`, `office_worker_93046`. Read by grep of DAY SUMMARY and the event/grudge/order lines, then full days: ceo_88522 d22+d24, exec_78363 d25-26,
exec_84610 d23, guard_27347 d23, guard_8440 d23-25, reporter_57617 d25, reporter_57702 d22, office_worker_21091 d22. No minor was a victim, in vice or dead in any line I read.
Ages are not in the diaries, so one thing is unverified (bug 4).

## 1. Verdict

Clearly better than V11: the beat on a CEO now lands and is heard of, reporters print Stories, a guard stands jail duty in hour-sized legs, wealthy picks sleep at home, and
two lives chain across the group (a reporter beaten on the El by a guard joins a gang; two office-worker neighbours shaken down on d22 get their revenge on d25).
It still is not a corp life: the desk is a clock that prints the same decision, the arrest chain mostly dies silently (1 of 5 attempts landed), and the corp class spends its
evenings in a 14-stranger Block hangout being shaken down for 2 coins. Stories start; few are picked up by the person they happen to.

## 2. Per archetype

- **CEO: alive on one, thin on the other (V11: alive / thin).** `ceo_88522` Bertram Carrow: `d22 06:08 Assault: Vivienne Ellery attacked Bertram Carrow` + `ContractFulfilled: beat on Bertram Carrow: done by Vivienne Ellery`
  (rumour `d21 04:06 Hired by corp Zetatech 5 on Bertram Carrow`), 16 "GOSSIP ABOUT ME (as the object)" lines that day, safety 1.00 -> 0.32 (`d22 08:00`), `d22 17:02 GOAL Treat`
  then `18:15 PLAN FAILED ... GoTo(Clinic): BuildingFull`, not retried until d24; also `d23 00:09 Coerced: Clemence Santoro leaned on Bertram Carrow (protection of 40 a week)`. A real thread.
  `ceo_20314` Jinx Oakes: still the table-sitter, 7.5 h Meeting at Corp HQ#5262 a day, midnight `ContractPosted: Militech 8 posted a locate on Raven Santoro ... / a tag` on 4 nights, never an outcome.
  New: `d21 09:30 [family] my children: Ignatius Oakes (17 y) at home (play), Alaric Oakes (17 y) at home (play); the pantry holds 24` daily (S6 line, reads fine, never changes in 7 days).
- **Exec: alive on one, thin on the other (V11: thin).** `exec_84610` Edric Greaves (Greaves Holdings): `Repossessed: Edric Greaves took Rin Nightingale's Deck T1` / `bricked Kestrel Blackwood's Legs implant T1`
  at 00:00 on 5 of 7 days, a robbed-neighbour fight (`d23 04:10 GOAL Fight on Silvio Varley`, `d23 05:25 Assault: Edric Greaves attacked Juno Inchbald`), `d23 17:52 Treated: Smoke Nightingale patched Edric Greaves up at Ripperdoc#5545 for 12c`.
  Real corp-dystopia texture. `exec_78363` Rin Duarte (Militech 11, treasury 3c): `d24 19:09 Served: Beatrix Mbeki turned Rin Duarte away at the door of Gambling Den#5289 (drunk)` + GRUDGE, then
  `d26 00:00 Hire: Rin Duarte hired as Engineer (picked by Rook Ferreira, the foreman)` and `Skipped: Quill Wexford is off sick ... Rin Duarte covers the Engineer's shift` (bug 7). A fall from the table that reads like a person.
- **Guard: alive on one, thin on the other (V11: alive / thin).** `guard_8440` Rook Sallow (night shift): jail duty in legs (`d24 00:41 GuardJail 67 min`, `02:27 79`, `03:50 54`), Precinct raids heard mid-shift
  (`d24 01:13 Raid on the Precinct: the Precinct held (3 raiders, 407 defenders); nobody freed; 5 hurt`), a witnessed assault turns into `d24 22:10 PLAN Arrest on Osric Quarry` (fails `22:23 PreconditionLost`), and two fare beatings
  `d24 23:15 Fare: Rook Sallow beat Sabine Ashcombe at the El ... left on the platform 30 min` (and Wendell Vance 23:21) -> `d25 12:23 GrudgeFormed: Cipher Okafor swore against Rook Sallow (a beating, 0.50)`. `guard_27347` Rhoswen Kettering: one real arrest
  (`d23 15:52 Arrest: ... arrested Orrin Brandt for Theft`, 55-min escort), otherwise 52 Fare Evasion witnesses, all from inside the Precinct on jail days, none acted on.
- **Reporter: alive on one, thin-to-alive on the other (V11: thin).** `reporter_57617` Nadia Ashcombe: a Story every midnight (`d22 00:01 Story: Hawthorne Feed: Volt Ironside assaulted Rhoswen Oakes by Nadia Ashcombe (from the beat)`), door-to-door
  `Work on Block#1095 [GoTo(TargetHome)]` hops by day (d25 09:03-11:13, five houses) with `interviewed X (a source)` memories, and `Housed: moved nearer work` (d22 00:12). `reporter_57702` Hex Haas is the best life in the group:
  `d22 12:31 Fare: Cordelia Quarry beat Hex Haas at the El` (V11's guard) -> GRUDGE -> `d22 21:17 GangJoin: Hex Haas joined The Red Brothers (miserable, mood -0.55)` -> `d25 00:01 Promoted: ... Senior Reporter`
  -> `d25 00:07 Crew: Hex Haas missed The Red Brothers's guard` -> `d25 00:08 Preached: took up Church of the Open Circuit` -> `d27 22:57 Assault: Hex Haas attacked Maren Duarte`. A man in four institutions at once.
- **Office worker: thin, improved (V11: thin).** Commute 2.5-4.5 h, 8.6-8.8 h work, 5 h leisure. `office_worker_93046` Ghost Wexford is the foreman who hires a new Office Worker nearly every night (`d21..d27 00:11 Hire: ... picked by Ghost Wexford, the foreman`),
  and the shakedown chain: `d22 06:04..06:34` three shakedowns at Block#658 -> `d25 06:01 Assault: Ghost Wexford attacked Axel Brandt` (revenge, a witness ran, one told 3) -> `d25 22:36 GrudgeFormed: Katrin Stroud swore against Ghost Wexford`. First office-worker story of the series.
  `office_worker_21091` Philippa Bellamy: the same d22 shakedowns, 46 Block hangouts of 30-61 min, one `d27 13:47 AssetBought: Deck T1`.

## 3. Round 3's mechanisms seen at 100k

Fired: **S3** jail duty in blocks (`guard_27347 d24 09:08 36 min, 09:48 42, 10:34 66 ...`; `met 4 in the cells this hour`, `MetInJail`), the fare beating itself (`guard_8440 d24 23:15`);
**S5** the beat story and crime desk (`Story:` lines: 6 in `reporter_57617`, 8 in `reporter_57702`; V11 had 0), "a hit heard of" (`ceo_88522 d22 07:11` on, GOSSIP ABOUT ME x16); **S2** the back-off line
(`ceo_88522 d26 17:02 goal Shop cooled ... backed off, 2 in a row`, 3 in a row d27 09:22, 4 d27 17:22; also exec_84610, office_worker_93046); **S6** the `[family]` line (`ceo_20314`, 7 days);
**S7** the raid result line (`guard_8440` four raids: "held (21 raiders, 422 defenders); nobody freed; 30 hurt ...; The Drowned Choir 9 next: BreakOut"); R4 `WitnessReacted` in 7 of 10 diaries;
V11's R6b hit on a CEO landed this time.
Expected, never seen: "resisted arrest" or a charge naming its victim (the one Arrest line is `arrested Orrin Brandt for Theft`), "the guard's collar", the fare cool-down text (no rider beaten twice), "stays off the trade" / "could not make the trip" /
"sat out the raid" (no gang pick here), an order left at the door, a muster, a Fixer visited in person (posts still 00:06-00:09 with no walk), a cabbie rank, a guard bribe, a court, a wounded guard or rescue (the raids' "5 hurt" never name a pick).

## 4. Bugs (ranked by what the player sees first)

1. **Arrests die silently: 1 of 4 chases completed.** `guard_8440 d23 11:10 PLAN Arrest on Zane Oakes` is four `GoTo(SuspectTile)` legs of 20-35 min at ~1 tile/min, then `13:10 plan interrupted: Arrest -> Unwind`, no event, no "lost him";
   `guard_27347 d23 17:17 Arrest on Clemence Santoro` ends `18:17 interrupted -> Unwind` the same way; `guard_8440 d24 22:23 PLAN FAILED: Arrest ... PreconditionLost` (a witnessed assault walked off in 12 min). Chasing at ~1 tile/min
   (guard_27347's long legs average 1.01 tiles/min) cannot close on a moving target. V11's escorts all completed; now the escort completes (55 min) but the chases mostly do not. The player sees a guard stroll toward nothing and go home.
2. **The CEO's `Served: decided ... would keep Grow in Tech` repeats while he is at home.** `ceo_88522 d24 00:07, 00:29, 00:55, 01:02, 01:17` (five in 70 min, same six at the table, he is `ACTION Rest @ Block#136`), 15 times in the window
   (`d22 22:53, d23 01:05, 08:28, 23:50, d24 22:45, 23:48, d25 00:51, 02:32`), and `d24 00:07` is billed `-1c (Leisure)`. A decision event with no sitting. The hourly `met X at the table (an attendee), a regular` memories
   are also back at 3 per hour plus a "... and 3 more" fold (227 memory lines in `ceo_88522`, 142 in `ceo_20314`); V11 had them gone. **Worse.**
3. **A rich corp pick has nothing to buy.** `ceo_88522 d25 17:02 plan interrupted: Shop unplannable (no feasible action)`, `d26 17:02`, `d27 09:22`, `d27 17:22` (backed off, 4 in a row); `exec_84610 d25 17:02, d26 17:02, d27 09:10`. "can buy 1.00" and "pride 0.95" with no seller: a real desire with no outlet.
4. **UNVERIFIED, top-priority audit: no age guard seen on the fare beating.** `guard_8440 d24 23:15 beat Sabine Ashcombe`, `23:21 beat Wendell Vance`; the diary never prints the victim's age. `citysim/src/systems/fares.rs` `caught()` / `beat()` (lines 116, 225)
   have no `is_adult` check, and `law.rs:348` calls it for any actor. If a school-age rider can be a dodger (`transit.rs:1607 dodge`), S6's hard rule is broken here. I could not tell from the diary; someone should confirm the dodger path excludes under-18.
5. **Corp-class picks as 2-coin shakedown victims and bullies in their own Block.** `office_worker_21091` (4 `WasRobbed`) and `_93046` (4), `exec_84610` as robber (10 `Intimidate` moves, `d23 03:49-05:25`, 3c each at a 209c purse, then `Attack`), `reporter_57702` 5.
   Identical `GRUDGE ... (Robbed, weight 0.36, chain 0)` lines; the same robbers hit both neighbours at `d22 04:50, 06:04, 06:22, 06:34`. The d25 revenge is a good payoff; the ticker before it is noise.
6. **Office workers spend leisure in a 14-stranger Block hangout.** `office_worker_21091` 46, `_93046` 40 `HANGOUT at Block#658 ... 14 other(s), 13 stranger(s) [1 known]` (every Unwind pick); they are Friends at +1.00 in one Block and never meet on purpose.
7. **`exec_78363` is an exec and an Engineer at once and works two shifts.** `d26 00:00 Hire as Engineer` while `ExecWage +10` continues (`d27 CorpOrder ... decided by Rin Duarte`, `at the table: nobody`); `d26 00:01..04:07` a night shift (`Rest` at the Vat Farm 91 min x2),
   then `08:31-18:00` (work 8.3 h, sleep 4.4 h, travel 6.2 h). Header `Job: none`. Covering for a sick worker is good; the midnight start and double pay are not.
8. **Short corp walks are very slow.** `ceo_20314 d22 08:34 GoTo(Seller) ... 54 min, 24 tiles` (a same-district hop to his own HQ), `exec_78363 d26 00:06 55 min, 35 tiles`, `guard_8440 d23 12:15 35 min, 33 tiles`, against `ceo_88522 d22 07:15 78 min, 183 tiles`. V11 bug 4 **unchanged** for short hops.
9. **The beaten CEO's treatment fails once and is dropped.** `ceo_88522 d22 18:15 PLAN FAILED ... GoTo(Clinic): BuildingFull`, next goal is a bar. Reads right for a harsh city; flagging because a full Ripperdoc is the first answer a rich victim gets.
10. **Reporters' stories are a midnight ticker, not their day.** `reporter_57617 d22..d27 00:01 Story:` one a night, unconnected to whom she interviewed; `d25 00:01` both reporters print `Tobin Osgood assaulted Hex Haas`, once `by Nadia Ashcombe`, once `by Hex Haas` (the victim reports his own assault).

V11 status: 1 (guards a 9-h cell block) **fixed** (legs of 28-91 min, `guard_27347 d24 09:08..17:22`). 2 (patrol ends "not in shift") **unchanged**: `guard_27347` work 1.8-1.9 h on d21/23/25/27 against 6+ h of patrol,
and `d23 16:49 GOAL Earn ... not in shift 0.00->0.30` at 16:49 inside `shifts [(540,1080)]`. 3 (beat invisible to the CEO) **better** (gossip, safety hit, Treat goal; still no guard bought). 4 (rich sleep in hotels) **fixed** (every night at Block#136 / #404).
5 (reporters report nothing) **better**. 6 (promoted while already Reporter) not repeated; `Promoted ... Senior Reporter (was Reporter)` is correct. 7 (jail with no wage consequence) not testable (no corp pick jailed).
8 (guards ignore witnessed crime) **better** (a witnessed Assault -> `PLAN Arrest`, d24 22:10), still ~59 Witness lines to 1 arrest. 9 (repeating WitnessReacted) **unchanged, rarer**. 10 (hits a ticker) **unchanged** (4 per CEO).

## 5. Dull spots (ranked, each a mechanism)

1. **A hit that lands should change the CEO's week.** The beat on `ceo_88522` ends the same evening in a bar. Mechanism: the victim's Treat retries at the next clinic, he buys a bodyguard off the same fixer board (it already lists `guard on Garage#5483`), and he learns the doer's name from the gossip he hears (Vivienne Ellery, 16 times).
2. **The desk is a clock.** 7.5-8.7 h Meeting, one decision a night or none. Mechanism: the vote needs a motion from an attendee with a stake (a shareholder who is also the Block's shakedown artist, a rival's pay, a fired tenant); a defeat becomes a grudge; this also removes bug 2's repeated line.
3. **The chase must close or say why not.** Mechanism: a pursuit with an end state: loses sight -> a `lost him` event and the suspect's heat rises, or a second guard is called from the Precinct's 400 defenders.
4. **Rich picks with nowhere to spend.** Mechanism: a high-rung Shop outlet (tailor, art dealer, clinic implants) the corp class can plan to; `exec_78363 d23 20:02 AssetBought Eyes implant T1` shows one exists, but the Shop goal does not find it.
5. **Office-worker neighbours and coworkers.** Mechanism: the Block shakedown (`d22 04:50-06:34`) should hit the Friend standing there, make them step in or flee, and the d25 revenge should name both (it names only Ghost Wexford).
6. **The fare beating has no exit.** Two beatings in 6 min and one sworn grudge, but the riders act on it only sometimes (Hex Haas does). Mechanism: a beaten rider rides the next El with a friend; the guard's grudge list gets a tail.

## 6. Too much / broken-looking

- **The nightly CEO hit ticker** (4 `ContractPosted` pairs at 00:06 per CEO) with no outcome ever shown to the poster.
- **The `Served ... Grow in Tech` repeat** (bug 2) and the hourly "met at the table" memories.
- **Guard witness noise:** `guard_27347` 52 Fare Evasion witness lines, all from inside the Precinct on jail days (d24 31, d26 21), none acted on; it reads as a feed, not a post.
- **FINDING, work h d21-26:** ceo_20314 7.5-7.8 meeting, ceo_88522 8.3-8.6, exec_78363 8.7 (d26 8.3 as an Engineer), exec_84610 8.1-8.4, guard_27347 1.8-1.9 / 8.7-8.8 (patrol / jail), guard_8440 0.4-8.7, reporters 8.1-8.7, office workers 8.6-8.8.
- **FINDING, travel h d21-26:** ceo_20314 1.8, ceo_88522 2.6-4.9, exec_78363 1.4-6.2, exec_84610 0.4-1.6, guard_27347 1.0-6.9, guard_8440 1.5-9.1, reporter_57617 0.9-5.2, reporter_57702 0.9-3.3, office_worker_21091 2.7-2.8, _93046 2.5-4.6.
