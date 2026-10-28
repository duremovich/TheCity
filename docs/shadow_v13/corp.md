# Shadow V13: corp (ceo, exec, guard, reporter, office_worker)

Critic: corp. Window: 100k island, seed 42, d21-27, `D:/TheCity/target/shadow_v13/isle100k_s42_d21/`. Picks (all new indices): `ceo_20314` (again), `ceo_91004`,
`exec_74262`, `exec_81172`, `guard_27360`, `guard_8358`, `reporter_57837`, `reporter_59309`, `office_worker_13025`, `office_worker_66033`. Read by event-word
counts and DAY SUMMARY, then full stretches: ceo_91004 d23 00:00-05:00 and d24 00:00, exec_74262 d24 04:12-06:09 and d27, guard_8358 d22 07:18 / d23 06:48 / d25 00:51,
guard_27360 d22 07:08 and 21:00, reporter_57837 d27 00:10, office_worker_66033 d22 17:55-20:43. Ages now print on every harm line; the ages in the harm lines I read
are all adults (Assault 51-63 y, Fare 21 y beat 20 y). No minor was a victim, in vice or dead in any line I read. A missing line is not evidence.

## 1. Verdict

Better than V12 on the desk and the street, still not a corp life. A CEO who is hit now answers it (a crew of three loses, he buys a weapons licence at the next
midnight); the `Served ... keep Grow` repeat and the hourly table memories are gone; arrests complete and name the victim. A jailed exec is the first corp life with a
fall, but the consequences stop at the cell door (still paid, nothing at the corp).
Still flat: the Fixer posts never come back with an outcome, reporters print one midnight story with an ambiguous byline, and office workers have almost no events
(one marries in ten minutes, one buys a motorcycle).

## 2. Per archetype

- **CEO: alive on one, thin on the other (V12: alive / thin).** `ceo_91004` Dagger Novak (63 y): `d21 18:00 ContractTaken: The Drowned Choir 8 took the hit on Dagger Novak (LIVE, crew 4)`
  -> `d22 03:00 ContractFailed: ... found no Dagger Novak at Block#386` (he was at Club#5182 after `d22 01:47 hailed a cab`) -> `d23 00:42 Assault: Rex Hawthorne (62 y) / Omar Upton (62 y) / Knox Adeyemi (51 y) attacked Dagger Novak (63 y) at Block#386`
  + `ContractFailed: ... lost (3 vs 3, 0 dead)` -> `00:44 GOAL Flee` -> `d24 00:07 LicenceSold: Dagger Novak bought a weapons licence for 60`. Three -1.00 enemy edges, GOSSIP x52. A real week.
  `ceo_20314` Jinx Oakes: the table-sitter (7.5-7.8 h Meeting), one `CorpOrder: Grow -> Secure in Housing (shock, 0.85 vs 0.82)` at `d23 00:07`, and `d21 08:20 Stripped ... Roderick Kemp (585 coins)` (bug 3).
- **Exec: alive on one, dead on the other (V12: alive / thin).** `exec_74262` Emeric Nightingale (35 y): `d24 04:12 GOAL Loot` -> `04:28 Stripped ... Delia Nightingale (8 coins)` + `Witness`/`Report`/`GrudgeFormed (a stripped body, 0.60)`
  -> `05:07 Arrest: Ulric Jessop arrested Emeric Nightingale for Theft` -> `06:09 Sentence: ... 3 days for Theft` -> d25-26 16 h jail a day, `MetInJail` x6 -> `d27 06:09 Release` -> 198 min walk home -> Strip Club. A fall that reads like a person.
  `exec_81172` Gideon Tanner (71 y): 9.5 h Meeting nightly, five `ContractPosted` lines, nothing else. Dead.
- **Guard: alive on one, thin on the other (V12: alive / thin).** `guard_8358` Tristan Greaves (21 y): `d22 07:23 Witness: saw Quill Ostrowski commit Assault`, `07:38 PLAN Arrest on Varek Brandt` -> `07:48 Arrest: ... for Assault on Nadia Fletcher`;
  `d23 06:58 PLAN FAILED: Arrest ... PreconditionLost` -> re-plan `GoTo(SuspectTile)` 83 min across two districts -> `08:33 Arrest ... for Assault on Joss Delacroix` (the chase closes); `d25 00:51 Fare: Tristan Greaves (21 y) beat Dahlia Underhill (20 y)`;
  `00:58 Pursuit: ... gave up the chase of Sebastian Singh (already in custody)`; `d27 03:12 Arrest ... for Vagrancy`. Three jail-duty legs of 45-82 min every night.
  `guard_27360` Tomas Inchbald (19 y): Witness x52 (33 Assault, 4 Murder, 4 Grand Theft), no Arrest; `d22 21:00 plan interrupted: Arrest unplannable: Unreachable`, then Patrol. Thin.
- **Reporter: alive on one, thin on the other (V12: alive / alive).** `reporter_57837` Freya Duarte (73 y): a `Story:` every 00:01, `d23 01:00 Attributed: the D1 The Sump beating of Garrick Marlow on day 5 is laid to Freya Duarte`, 25 `GoTo(TargetHome)` legs with 23 `interviewed ... (a source)`,
  and the group's only T1 line (`d27 00:10`, bug 2). `reporter_59309` Mina Moreau (22 y): six Stories, five `WitnessReacted ... saw X's Assault and ran` / `told 3 of her people what Nico Whitlock did`; a 19-min commute, 8.5 h desk, 6 h idle. Thin with a good witness thread.
- **Office worker: thin, one spark (V12: thin).** `office_worker_66033` Verity Hawthorne (63 y): `d22 18:02 GOAL Court ... PLAN Court on Osric Chrome [Propose]` -> `18:12 Marriage: Verity Hawthorne married Osric Chrome` (Friend +0.64 to Spouse in 10 min at her desk), the spouse in her Block hangout every night after.
  `office_worker_13025` Odile Bellamy (19 y): 83 min commute, 28 Block hangouts of 14-17 strangers, one `d27 10:11 AssetBought: Motorcycle T1 ... on finance`. WasRobbed 0 in both (V12: 4 each).

## 3. Round 4's mechanisms seen at 100k

Fired: **T6** `CorpOrder ... decided by Dagger Novak` and `Served: ... decided Vatra 9 would Grow in Housing; called from Block#386; the table last sat: Lavinia Voss, ...` (told once, with where: `ceo_91004 d27 06:01`, `ceo_20314 d23 00:07, 01:59`);
**T3** the dropped chase as a `Pursuit` line (`exec_74262 d24 05:22`, `guard_8358 d25 00:58`), the charge naming its victim, a dead plan that says so (`guard_27360 d22 21:00 ... goal Arrest cooled until d22 23:00`, `guard_8358 d23 06:58`);
**T1** the service at the door (`reporter_57837 d27 00:10 Custody`, 3 days' grace; the `[family]` line then reads `on the service's warning since d27`); **T2** the `[family]` line says why (`no School within 1 h` in 2 diaries every day, `no School today` on rest days);
**T7** folds (table memories 3-35 per diary against V12's 142-227; `met 4 in the cells this hour`; `saw 2 commit Fare Evasion this hour (... and 11 more)` as one line) and **ages on every harm line**.
Expected, never seen: **T4** door let-in/refusal or dancer tip (`exec_74262 d27 11:07` and `12:40` are two 91-min `Enjoy` visits at Strip Club#5407 with only `-6c (Leisure)` and `memory Enjoyed`), `DawnPick`; **T5** (no jobless pick here);
a **T6** burned-mark / seized-shop / audit line; an exec giving up a post; a T3 jailed-mark-waited-out or staff-skip-queue line; a muster. The corp group touches few of these by design.

## 4. Bugs (ranked by what the player sees first)

1. **A guard sees 33 assaults and 4 murders and arrests none.** `guard_27360` Witness lines `d21 23:00, d22 00:37, 05:29, 07:23, d24 01:13 (Casimir Pembroke commit Murder)`, zero Arrest. Its one Arrest goal, `d22 21:00`, is `Unreachable` and cooled 2 h. `guard_8358` converts 3 of ~44, so the chain works, but only while the guard is out of the Precinct (27360 spends 8.8 h in it). V12 bug 1 (arrests die silently): **better**; the witness-to-arrest rate is the new one.
2. **The Custody line contradicts the `[family]` line.** `reporter_57837 d27 00:10 Custody: ... the door of Block#3363: Rin Duarte (9 y) went 2 days unfed`, while `[family]` on d21-d26 09:30 has the child `at home` (her home is Block#205), pantry 17-28 all week, and she eats at home herself (`d27 05:04 EatAtHome`). Either the child lives elsewhere or "unfed" ignores a pantry of 17. Also: a 9-year-old two days without food sits close to the hard rule (no child hurt by starvation); the mechanism is the warning-first design, so Dylan should confirm that reading.
3. **A 585-coin corpse for a CEO.** `ceo_20314 d21 08:20 Stripped: Jinx Oakes stripped the body of Roderick Kemp (585 coins)`, purse 596 -> 1193, no Witness, no Report, no grudge, three minutes from his door; `exec_74262 d24 04:28` strips 8 coins and is reported the same minute. The windfall with no consequence is the bug, not the looting (`1-lawfulness 0.33->0.60`).
4. **A jailed exec keeps his wage and nothing happens at the corp.** `exec_74262 d25 00:00 +25c ExecWage`, `d26 00:00 +24c` while `jail 16.0h`; header `Corp: Vatra 26 (EXEC)` unchanged on release. V12 bug 7 (jail with no wage consequence): untestable then, **unchanged now**.
5. **Short corp walks are still very slow.** `ceo_20314 d21 08:34 GoTo(Seller) ... 55 min, 8 tiles` to his own HQ. V12 bug 8: **unchanged**.
6. **Reporter headlines read as the reporter's crime.** `Story: Nutrix 2 Now: Juno Chrome robbed by Freya Duarte (from the beat)`, `Lena Sato extorted by Freya Duarte`: the byline reads as the robber. Still one per night at 00:01 (V12 bug 10): **unchanged**; the V12 double byline did not recur.
7. **Plan and action disagree on the Home.** `guard_27360 d22 07:08 PLAN Eat on Block#2346 (D3 Neon Quarter) [GoTo(Home) > EatAtHome]` then `ACTION GoTo(Home) ... -> Block#1639 (D1 The Sump), 91 min, 122 tiles`. The Sump-to-Civic-Core commute costs him 5.0-7.0 travel h a day (d21-24, d26).
8. **Sleep alternates 10 h / 3 h in the day summary.** `office_worker_66033` sleep 11.3, 2.7, 9.9, 2.8, 9.8, 3.3 on d21-26, the short nights cut into `Rest` 90 min and `Sleep` 46-69 min blocks. Probably accounting rather than a person; the summary reads wrong.
9. **A CEO is "hired" by a different corp each night.** `ceo_91004 d21 00:01 Story: Civic Wire: Greenline 41 hired Dagger Novak (paid by Vatra)`, `d23 00:01 ... Nutrix 3 hired Dagger Novak (paid by Vatra 9)`, `d26 00:01` two more; GOSSIP "Hired by corp Greenline 41 on Dagger Novak" told 46 times on d21. He has `Job: none` and never moves; `d24 00:06 Poached by corp Nutrix 23 on corp Vatra 9` hints the label is the corp's poach.

V12 status: 1 arrests die silently **better**; 2 `Served ... keep Grow` repeat **fixed** (2-3 per CEO against 15); 3 rich pick with nothing to buy **better** (3 `Shop unplannable` in `ceo_91004` on d27, 0 in the other three); 4 age unverified on the fare beating **fixed** (prints 20 y);
5 corp picks as 2-coin shakedown victims **fixed in these picks**; 6 14-stranger Block hangout **unchanged** (`office_worker_13025` 28 hangouts of 14-17); 7 exec as Engineer not seen; 8 slow short walks **unchanged**; 9 beaten CEO's Treat fails **better** (he buys a licence); 10 reporters' midnight ticker **unchanged**.

## 5. Dull spots (ranked, each a mechanism)

1. **The Fixer posts never come back.** Every CEO and exec posts `locate` + `tag` at 00:06 (`ceo_20314` d24-d27, `exec_81172` d23, d26, d27); no outcome reaches the poster. Mechanism: a contract's resolution (found, tagged, failed, the mark gone) lands on the poster as a line, and the next post names the last.
2. **A failed hit should buy a guard, not only a licence.** `ceo_91004 d24 00:07` takes the licence after the 3-vs-3, but no bodyguard, no change of Home. Mechanism: the beaten victim posts a `guard on <place>` ledger line (the board listed one in V12).
3. **Jail should reach the corp.** Mechanism: ExecWage withheld while jailed, the table sits without him, his corp marks him a risk on release.
4. **The Precinct guard sees and cannot act.** Mechanism: a witnessed crime in the Precinct's district posts a `Pursuit` to the idle guards inside, so the witness stays on shift and another takes the arrest.
5. **Office workers have no coworkers.** Both picks spend 8.7 h in ClerkWork with no Chat or gossip at work. Mechanism: the `Break` leg (`d22 14:35`) becomes a hangout with the colleagues on shift, so a marriage like `office_worker_66033`'s starts at the next desk.
6. **A wedding with no wedding.** `office_worker_66033 d22 18:02-18:12` Friend to Spouse in a 10-minute Propose; no refusal path seen, no move-in, no gossip. Mechanism: a proposal can be refused (grudge, a Story) and an accepted one moves the pair to one Home.
7. **Corp-class kids have no School.** `ceo_20314` (17 y x2) and `reporter_57837`'s 9-y child read `no School within 1 h` every day d21-26 in D4/D5; T2's reach does not extend to the corp districts at 100k. Mechanism: Schools seeded by reach of every class's Homes, or a corp tutor as the class's School.

## 6. Too much / broken-looking

- **The nightly Fixer ticker** (8 `ContractPosted` for `ceo_20314`, 5 for `exec_81172`), the only thing a corp pick does after 00:06, never resolved.
- **Guard Witness feed, folded but lopsided:** `guard_27360` 52 Witness lines (42 of them d25-27), `guard_8358` 44, against 3 arrests. Reads as a feed, not a post.
- **FINDING, work h d21-26:** ceo_20314 7.5-7.8 meeting, ceo_91004 8.2-9.6, exec_74262 7.7-8.7 (then jail), exec_81172 9.2-9.6, guard_27360 1.1-8.8, guard_8358 2.9-8.9, reporter_57837 7.9-8.6, reporter_59309 8.2-8.6, office workers 8.6-8.8.
- **FINDING, travel h d21-26:** ceo_20314 1.8-1.9, ceo_91004 2.4-3.4, exec_74262 0.3-2.2, exec_81172 2.3-2.5, guard_27360 5.0-7.0, guard_8358 1.1-6.0, reporter_57837 4.5-5.9, reporter_59309 0.8-2.8, office_worker_13025 2.3-3.4, office_worker_66033 2.5-3.5.
- **FINDING, T1 at 100k:** one Custody line in 10 diaries, from the only pick with a young child in the household; `ceo_20314`'s two 17-y children read `play` all week with no service line. Too few to read the mechanism's rate.
