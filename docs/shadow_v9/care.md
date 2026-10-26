# Shadow V9 critic: care (ripperdoc, orderly, super, sick; teacher absent)

Window: main 4108184, 100k island, seed 42, d21-27. Diaries under `target/shadow_v9/isle100k_s42_d21/`, cited `file dDD HH:MM`.
Eight picks: ripperdoc_18160, ripperdoc_82872, orderly_12621, orderly_63998, super_90906, super_98982, sick_82185, sick_85595.

## 1. Verdict

- The clinic trade reads as a desk job with no patients: 1 `treated` line in 8 diaries, 158 `Treated` events city-wide in 30 days at 100k (~5/day).
- The super is a statue: 6 of 6 work days are one 538-minute ClerkWork on one tile, no round, no tenant, ever (`super_90906` d21-d26).
- Care people do not read as living lives in a harsh city: nobody is treated, a diagnosed cancer is ignored, and the only sharp story is a clinic owner who quits her own clinic unpaid.

## 2. Per archetype

- **Ripperdoc: thin.** Day shape holds: 6-8 station blocks of 39-86 min, no statue (`ripperdoc_18160` d24 09:06-17:17). The one patient: `d24 12:21 [memory] treated Hiro Draper#83514 (a patient), a regular`, after a plain ClerkWork block. `ripperdoc_82872` has zero `treated`, zero `patient`, 55 ClerkWork blocks. 18160 then gets arrested for Extortion off a street hangout (`d24 21:22 Arrest: Dex Delacroix arrested Anton Ferreira`; `d25 03:12 JAILED for Some(Extortion) until d31`), a lawfulness-0.80 clinic doctor with no extortion line in his week. Story-wise it is the best thing in the group, and it is unexplained.
- **Orderly: alive for one, dead for the other.** `orderly_12621` (67 y, wage 5, owns the clinic she works at): `d23 18:00 Quit: Maren Fairweather quit as Orderly (unpaid)`, then d24-d27 work 0 h, Scavenge x28, 10-17 h at home. That is a real story (owner-employee, wages owed, quits). `orderly_63998` is a night orderly (shifts 21:00-06:00): `d22 01:26 Break 27 min` then 54-90 min stations, eats, sleeps; no treated line, no named coworker.
- **Super: dead.** `super_90906` d22: leave home 07:00, `GoTo(Workplace) 79 min`, `08:19 Rest 41 min` in the lobby, `09:02 ClerkWork 538 min (ended)`, `18:02 GoTo(Home) 79 min`, hangout at home block with 37 strangers. The same seven-line day repeats d21-d26. `super_98982` is worse: lives D4, post in D6, `GoTo(Workplace) 190 min` every morning from 05:27 (d21-d26), 7 h sleep, 538 min ClerkWork.
- **Sick: thin, one accidental story.** `sick_85595` (72 y, Vat Tech, cancer silent 0.07 -> 0.13) works 538 min statues on a vat farm, `d24 OFF SICK TODAY` then six idle `Rest` blocks at home from 08:44 (`d24 10:17`, `11:49`, `13:21`...). `sick_82185` (42 y, diagnosed cancer 0.38 -> 0.44) loses her job slot and scavenges (`d21 08:30 Scavenge`, `d22 17:04 PLAN FAILED: Earn failed at Scavenge: PreconditionLost`, repeated d23, d24), works vat farms intermittently, `d22 18:03 ... cancer stage 0.41 (diagnosed) | OFF SICK TODAY`. Nobody goes to a clinic.

## 3. Bugs, ranked

1. **The super does no round and no work beyond one tile.** 0 `SuperRound`, 0 `called on`, 1 mention of tenant/rent in each diary. 538 min on one tile for 12 days of 12 (`camera.tsv` work_longest_min 537-538, work_tiles 1). V8 had 6 of 8 supers on rounds; at 100k both picks have none. Cause unknown; the V8 critique's lobby-wait is gone but the post is back to a pure statue (`super_90906` d22 09:02).
2. **Clinic work is ClerkWork, not treatment.** The doctor and orderly both do station blocks all day while patients do not appear. `grep patient` gives 1/0/0/0 over the four clinic diaries. The Treat goal is not on any clinic worker's day (GOAL counts: Work, Unwind, Idle, Eat only).
3. **Diagnosed cancer, no care-seeking.** `sick_82185` is diagnosed from d21 and climbs 0.38 -> 0.44 in six days with `GoTo(Clinic)` never planned; `ledger treatment=301/day` city-wide exists, so the mechanism fires elsewhere.
4. **190-minute commute for a super** (`super_98982` d21 05:27 to 08:37 and back) and 79-minute commute within one district (`super_90906`), against a 36-minute doctor commute. The homes are far from posts, housing does not match the district.
5. **Jailed doctor, clinic empty.** `ripperdoc_18160` is held 4128 min on a 6-day sentence (`d25 03:12`); the clinic has no cover and the diary shows no consequence (no patients lost, no replacement).
6. **Owner-employee quits unpaid** (`orderly_12621` d23 18:00): `Quit` is 3,501 city-wide events in 30 days, but here the quitter owns the building. The clinic has no owner behaviour, just ownership on the sheet.

## 4. Dull spots, ranked (mechanisms)

1. **Patients walk in.** Give clinics a queue: injured, sick, overdosed, gang-hurt and cancer-diagnosed people plan `GoTo(Clinic)`, a Treat action runs on the doctor's station block with the patient present by name. The doctor's day becomes "treated Hiro, then a stims overdose, then a gunshot", and the orderly runs the triage and bed. Today the clinic is a place with an employee in it.
2. **The super's round.** A route over the post's Blocks, with a tenant complaint list (rent short, noise, a leak, a squatter, a corpse). Each stop is a short scene that can resolve to rent paid, an eviction notice, a bribe, or a call to the precinct. Evictions are 2,077 and SquatEvicted 7,487 in 30 days, so the supers have something to do.
3. **Doctor with a second book.** The ripperdoc with greed 0.63 and persuasion 0.40 takes a cash patient off the books: a wanted man, a gang kid, a chrome removal. This is the black-doc seam from PRESSURE_PASS, and gives a lawful ripperdoc (18160, lawfulness 0.80) a reason to be in a bad place that matches the arrest.
4. **Clinic owner under water.** `orderly_12621` owns her clinic and cannot pay herself. A clinic in the red should cut hours, sell kit, take debt, or close; the owner's quit becomes a closure with a reason on the door.
5. **The sick ask for help.** A diagnosed or silent-then-diagnosed pick picks Treat when affordable, begs, pawns an implant (sick_82185 owns a Deck T1), or goes to the black doc when broke. A death arc over weeks follows from the same mechanism.
6. **Night orderly as night witness.** The orderly (21:00-06:00 shift) sees the city's wounded arrive at 02:00. A named arrival with a cause ("knifed on Mid-Levels by The Unplugged") would feed gossip, which this diary does carry (`ripperdoc_18160` d24 19:14 `Assaulted by Sprocket Garland`).

## 5. PRESSURE_PASS evidence

- **Dead trades / misery acts:** super and clinic are the dead trades at the player's level (bugs 1-2). The pass's black-doc line ("treats the wounded who avoid a clinic") has a market: sick_82185 (cancer, 5c wallet) and 18160's arrest (a doctor who goes to jail has no covering colleague).
- **Reach:** the 190-min and 79-min commutes (bug 4) read as a reach problem: no per-purpose cap for "go to work" at all. Supers and doctors should be housed or posted within one district.
- **Protection:** none seen. No care diary shows a protection payer or taker; the arrest of 18160 for Extortion is the only crossover.
- **Ledger signal:** `Treated` 158 in 30 days over 100k people, `treatments` 157, `Quit` 3,501, `SquatEvicted` 7,487 (`analyze100k.txt`).
- **Absent (orchestrator files):** teacher. Not read.
