# Shadow V19: corp (ceo, exec, guard, reporter, office_worker)

Critic: corp. 100k island, seed 42, d21-27, main 41c31b2, `D:/TheCity/target/v19/isle100k_s42_d21/`. Picks: `ceo_105038`, `ceo_81843`, `exec_40238`, `exec_78500`, `guard_111646`, `guard_16925`,
`reporter_62779`, `reporter_95921`, `office_worker_24498`, `office_worker_42443`. Read by event-word counts (python), every DAY SUMMARY, a min-hunger scan of every `[needs]` line, a CLIMB census, and full stretches:
office_worker_24498 d21-d22, guard_111646 d23 22:00-23:50 and d25-26 goals, guard_16925 d22 15:00-18:00 and d23, ceo_105038 d21 17:00-24:00 and d24 03:00-06:00, ceo_81843 d26, exec_78500 d21, reporter_95921 d21 08:00-10:40.
Cited `file dDD HH:MM`; a missing line is never evidence. The picks differ from V18's, so this is a re-roll, not one for one.

## 1. Verdict

Better than V18 where the tower matters: a clerk's whole week fits inside one block (flat on 27, desk on the ground, a lift between), a guard climbs to a Hideout on floor 24 and tries an arrest, a CEO's rival corp posts a beat that an NPC takes in person.
The class itself is still a desk plus a very long idle: the CEOs and execs idle 7-21 h a day, wandering the street with a flat one lift away, and the nightly stims run to the Sump is their only evening.
The guards patrol ~2 h of a 9 h shift (V18's bug, unchanged); nobody in my ten starved, was a minor harmed, or had a Hideout leg past 1 h.

## 2. The exit's checks (corp group)

- **(a) A life within a walk in or beside one tower: seen, clearly.** `office_worker_24498` Vivienne Draper (27 y): flat 4 on floor 27, post `Corp HQ#9309 floor 0 of Block#1457`, header `Commute ... 0 tiles, travel 0 min`.
  `d22` summary: `work 8.8h, leisure 7.3h, travel 0.2h ... places: Block#1457 floor 27 850m; Corp HQ#9309 floor 0 of Block#1457 552m; street 32m`. The shape: `d22 08:19 CLIMB down by the lift ... floor 27 to 0, 1 min waiting in the lobby, 3 min in all`
  -> ClerkWork all day on the ground -> `18:06 CLIMB up ... floor 0 to 27` -> six `HANGOUT at Block#1457 floor 27 for 61 min ... 3 other(s) there: 3 stranger(s) [0 known]` (18:09-23:24). Also `exec_78500` (flat on 42, `Corp HQ#7410 floor 0 of Block#440`,
  `d21 08:50 GoTo(Seller) ... 5 min, 1 tiles`, of which `CLIMB down floor 42 to 0 ... 4 min in all`), `ceo_105038` (flat floor 30, HQ floor 0 of Block#82), `guard_111646` (flat floor 16 of Block#1085, Security Office#6531 in Block#1084, 8 tiles, `d23 22:26-22:30`).
  The evening half does not hold for the rich: ceo_105038 and exec_78500 leave the tower for the Sump (bugs 1, 3).
- **(b) A gang holding floors over a front; a raid that climbs and lands in a member's diary: partial.** No raid lands in a corp diary (gossip only: `guard_111646 d22 11:14 Raided by gang The Drowned Choir 36 on Precinct#6407`; `office_worker_42443 d21 00:01 ... on corp Nutrix 2`).
  What lands is the law climbing to a Hideout: `guard_111646 d23 22:46 GOAL Arrest ... GoTo(SuspectTile) -> Hideout#137368 floor 24 of Block#966` -> `22:49 CLIMB up by the lift in Block#966: floor 0 to 24, 1 min waiting in the lobby, 3 min in all` -> `22:53 Assault: Kieran Ashcombe (33 y) resisted arrest and fought off the guard Wire Ostrowski (60 y)`
  -> `Pursuit: Wire Ostrowski called for a second ... Vale Ferreira on shift, 10 tiles off, comes` -> `23:03 gave up the chase (already in custody)` -> `23:05 CLIMB down`. A Hideout on a tower floor, a guard on the lift, a second guard: it reads. (The "already in custody" give-up is V18 bug 7, unchanged.)
- **(c) A Hideout leg past 4 h: none.** The one Hideout leg is 6 min, 17 tiles (`guard_111646 d23 22:46`).
- **(d) A minor harmed: none.** No (N<18 y) in any Arrest, Assault, Witness-victim, Hit, Fare, cell, courtship or death line of my ten (scanned by python). Edge, unchanged from V18: Camp Escape sentences name children as the "on" target
  (`guard_16925 d22 18:00 the law added 1 days to Taz Mbeki (58 y)'s sentence for the Camp Escape on Freya Mbeki (15 y)`; `on Roxy Garland (12 y)`; `d24 18:00 ... on Vale Oyelaran (2 y)`): the adult is jailed, the child is only the one taken out; the wording is worth one look.

## 3. The vertical city at the player level

Reads like a place in its bones. Lift times scale with the building: 2 min for floor 16-17, 3 for 27-31, 4 for 40-42 (CLIMB census over all ten); a low block is the stairs, `1 min in all` (`reporter_62779`, 61 climbs;
`reporter_95921 d21 09:08 floor 0 to 1`, `09:39 floor 0 to 2`). No body stands on a floor the building lacks; headers read `flat 4 on floor 30, floors 0-54`, `flat 6 on floor 11, floors 1-28` (flats 1, 4, 6, 7 appear, not all "flat 1"). The escalator and a footbridge
show as `DECK on the escalator at (505, 704), level 1` (`reporter_95921 d21 18:23`, `d22 07:23`; `exec_78500 d24 06:37 DECK on a footbridge`), down in the morning, up at night, as built.
Neighbours exist: `office_worker_24498 d21 02:26-02:55` three people (Cipher Marlow, Echo Northcott, Winifred Pembroke) lean on her asleep on floor 27, 5c each; she reports them (`20:38 ReportCrime`, a 52 min trip to the Hall) and it lands:
`22:32 Cells: Echo Northcott learned who put her inside: Vivienne Draper reported her for the Shakedown`. That is a story the tower made.
What reads thin or odd:
- **The lift never queues.** Every lift CLIMB in my ten says `1 min waiting in the lobby (1 called the car, this call too)` at every hour, the 08:50 and 17:00 rushes included (only 6 of ~240 show "2" or "3 called"). A constant 1 min with the same clause reads as a stub, not a lift. The grammar "1 called the car, this call too" is also odd.
- **Neighbours never become known.** Six straight hours of `3 other(s) there: 3 stranger(s) [0 known]` on floor 27 (`office_worker_24498 d22 18:09-23:24`), the same count every night: people on your floor and no relationship forms.
- **A place label is wrong once:** `office_worker_24498 d22 08:22 now at Corp HQ#9309 floor 0 of Block#1457 (from Corp HQ#9309 floor 27 of Block#1457)`: the flat on 27 is named "Corp HQ floor 27" in the from-clause.
- Plan text can name the wrong block: `office_worker_24498 d21 18:05 PLAN Eat on Block#1318 [GoTo(Home) > EatAtHome]` while home is Block#1457.

## 4. Per archetype (V18's word in brackets)

- **CEO: thin (thin), one real thread.** `ceo_81843` Edric Duarte (54 y): `d26 00:06 Persuade to Beatrix Sato ... SUCCESS` (a poaching) -> `00:09 ContractPosted: Vatra 6 posted a beat on Edric Duarte for 433 at Fixer#7989 (rival over the poaching with Vatra 14)` ->
  `09:26 ContractTaken: Liesl Eastwick came to Fixer#7989 in person and took the beat ... for The Unplugged 2 (LIVE, crew ...)` -> `23:24 GOSSIP ABOUT ME: Alaric Ashcombe told Odile Voss: Hired by corp Vatra 6 on Edric Duarte`, `d27 09:11 told 7 times today`. He hears it and does nothing
  (no Flee, no guard; `d26 23:27` walks 16 min to buy stims). The hit does not land by d27. `ceo_105038` Juniper Pembroke (35 y): `meeting 8.3h` at the tower HQ, then `d21 18:08-23:28` twelve `PLAN Idle [Wander]` legs on the street.
- **Exec: thin (thin).** `exec_78500 d21 00:07 CorpOrder: Habitat 2: Secure in Housing -> Grow in Housing (shock, 0.78 vs 0.73, decided by Rosalind Varley); called from Block#440` (a decision made on floor 42) is the best line; then `17:32` a 54 min, 1047-tile stims run to the Sump and
  `19:01 Sleep @ Capsule Hotel#12329 (D1 The Sump) 253 min` (3c), flat on 42. `exec_40238` Elspeth Underhill (65 y): `idle 11.0-12.8h` d23-d24, Gambling Den on the rest day.
- **Guard: alive (alive).** `guard_111646` above, with a "called for a second" that works. `guard_16925` Volt Bartlett (26 y): `d23 08:00 Skipped: ... sets off for it, then does not go in; walks the streets instead (mood -0.11, absence 1 in 14 days)`;
  `GuardJail` shifts at the Precinct with `d22 15:34-16:09` five Assault witnesses in the cells in 40 min, `Hana Dunmore (67 y) ... Assault on Security robot T1`. A character.
- **Reporter: thin (thin/dead), better.** V18's jobless "Reporter" is gone: both picks work 7.9 h daily. `reporter_95921 d21 09:03-10:32` goes to sources block by block (`interviewed Zed Sallow (a source)`, `Percival Jessop`, `Kofi Yates`) and files
  `d25 00:01 Story: Nutrix 43 Now: Hiro Penhallow assaulted Orrin Duarte, story by Marek Nakamura (from the beat)`. `reporter_62779` files six stories `d22-d27 00:01`, a different victim each night (not a loop). See bug 5.
- **Office worker: alive (alive).** `office_worker_24498` is the tower life above. `office_worker_42443` Tank Novak (37 y): flat floor 40 of Block#824 (Mid-Levels), HQ in the Spire, `4 min` lift each end, `d25 07:13 DECK on the escalator`; hears of an avenging
  (`d23 08:24 Avenged by Lorcan Tanaka on Mags Duarte`, deed from d16) and does nothing.

## 5. The pace

Commutes read right: 0-6 min for the tower lives, 28-41 min across districts for the reporters (`reporter_62779` 851 tiles). The `[camera] JUMP` count fell: 13 (reporter_95921, escalator stretches, `d21 18:24 43 tiles in 1 min (allowance 23)`), 1-3 for the rest, against 13-18 per life in V18 (**better**).

## 6. Bugs (ranked by what the player sees first)

1. **Rich corp people spend their evenings idle on the street, and the CEO is broke.** `ceo_105038` header `Wallet 4c` against `treasury 93439c`; `d21 18:08-23:28` twelve `PLAN Idle [Wander]` (`GOAL Idle (was GetHigh) ... runners-up: none`: Unwind not even listed); `d24 03:19-05:38` Wander in the Neon Quarter at 04:00,
   `05:00 Assault: Bertram Jarrow hit bystander Juniper Pembroke ... Crossfire`, `SawCorpse` x3, `05:08 FleeToHome`. `ceo_81843 d21 00:00 RentShort: could not pay the rent on Block#153 to Habitat (0 of 1)` (1c rent, `Wallet 0c`, `ExecWage +40/day`). Idle hours: `ceo_105038 7-8h`,
   `exec_40238 11-12.8h`, `exec_78500 d25 13.3h`. Likely: ExecWage is all they get and stims eat it, and Corp execs have no home leisure (the clerk gets `HangOut` at home, the CEO does not). Not in V18's list (new on this read).
2. **Guards patrol ~2 h of a 9 h shift.** `guard_111646` (shifts 21-24 and 0-06): `work 1.1-2.5h/day`; `d25 00:28-06:00` Idle/Earn/Court/GetHigh with `GOAL Earn ... not in shift`; `d25 23:44 GOAL Earn (was Patrol)` inside 21-24; `d26 02:16-05:46` `GOAL Idle ... runners-up: none`.
   `guard_16925 d26 15:17 GOAL Sleep (was Patrol)` mid-shift, asleep to 20:39. Patrol is dropped on the first interruption and not scored again (V18 bug 1: **unchanged**; clearest on a split night shift).
3. **The rich leave the tower for a long stims trip that then fails.** `exec_78500 d21 17:32 GoTo(Spot) ... street (D1 The Sump), 54 min, 1047 tiles` -> `18:27 PLAN FAILED: BuyStims: PreconditionLost` -> `18:28 GOAL Sleep ... GoTo(Hotel)` -> `19:01 Sleep @ Capsule Hotel#12329 253 min`.
   Same failure `ceo_105038 d21 17:41`, `ceo_81843 d26 23:44`: the spot is picked and the walk paid, the seller is gone. C9a's travel choices do not weigh the leg for a dose.
4. **A street deal 14 people saw and nobody is taken.** `ceo_105038 d21 18:00 Witness: 14 saw Lena Ashcombe (28 y) deal stims on the corner by Noodle Bar#8988 (17:53-17:57, 6 sales)` after the CEO bought two; no Arrest follows in his diary (V18 bug 2, the guard buying stims, not re-seen: **unchanged in kind**).
5. **Interviews credited to absent sources.** `reporter_95921 d21 09:11 interviewed Zed Sallow (a source)` with `SETTLED APART: ... Zed Sallow at Chapel#12362 floor 13 of Block#3934` while the reporter is on floor 1 of Block#4260; `10:32 Kofi Yates at Lab#12638 floor 0 of Block#4259`.
   The memory is made on reaching the source's block, not the source.
6. **"Killed by ... on #15165".** `guard_16925 d21 21:27 told Brick Ferreira: Killed by Perrin Hawthorne#53614 on #15165` (a nameless victim; V18 bug 6, **unchanged**; the other death rumours in that diary carry names).
7. **A guard gave up the chase of a man already in custody** (`guard_111646 d23 23:03`; V18 bug 7 **unchanged**).
8. **Two reports back to back at the Hall** (`office_worker_24498 d21 21:30 and 21:43 ReportCrime @ Civic Hall`; V18 bug 5 **unchanged**), and the lift stub (section 3).

V18 status in my group: guard patrol (1) unchanged; guard buys stims (2) not re-seen; jobless "Reporter" (3) **fixed**; JUMP false alarms (4) **better**; double report (5) unchanged; nameless "Killed by" (6) unchanged; give-up-in-custody (7) unchanged; Camp Escape child as "on" target unchanged.

## 7. Dull spots and too-much

Dull: the rich evening (bug 1) is 6 h of identical `Wander` lines; the clerk's six `HangOut ... 3 strangers [0 known]` hours; the CEO's and execs' daily `ContractPosted` (ceo_105038 has 25 in six days, same Fixer, guard / locate / tag on named people) of which one in my ten was ever taken.
Too much: `Coerced` leans on a CEO every few nights for "protection of 40 a week" on a venue (`ceo_105038 d21 00:09`, `d22`, `d23`, `d26`, `d27`; six in all); `guard_16925` has 71 Witness lines, mostly cell fights and wage-time sentence reads.
