# Shadow V13 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: 100k island, seed 42, d21-d27, two picks each (18 diaries), main a70800f. Cites are `file dDD HH:MM` under `target/shadow_v13/isle100k_s42_d21/`.
Method: DAY SUMMARY lines for all 18, every non-Witness `[event]`/`[law]` line in all 18, `[family]`, GOSSIP and GoTo greps, full reads of bouncer_57196 d23 00:00, dancer_71865 d22, club_staff_38907 d22. The round's counters in `target/r4_100k/analyze.txt` were not read in depth.

## 1. Verdict

Clearly better than V12 on the trades: every door, stage and floor job now prints a nightly `Served` line (V12 had none in 8 of 18 diaries). The line is honest to the point of bleak: clubs are empty (a room of 0), the door lets in 125 and turns away nobody, the dancer plays to an empty room six nights of seven. Owners got a real sequel (Confronted, the Hall takes 2c).
Stories still start and are dropped: Crew refuses, the fired dancer, the child on the service's warning while the mother sits in the cells all end in a repeated line, not a consequence. The ClerkWork / Break metronome under the new lines is unchanged.
No minor is harmed, a victim, a client or in vice anywhere in this group. Every Assault line prints ages (63/47, 63/66, 69/57, 29/63, 29/51), all adults; the only minors seen are a pick's own children in `[family]` (dancer_99218, 11 y and 8 y, play at home).

## 2. Per archetype

- **bouncer: thin (V12 dead).** bouncer_57196 `d23 00:00 Served: Emeric Volkov kept the door of Gambling Den#4886 tonight: 125 let in, nobody turned away` (also 75, 126, 129, 118, 99). The line exists; the door has no judgement yet. bouncer_94173 at a Fight Pit lets in 7, 3, 3, 1, 4 and is a Grey Hands member: `d23 00:07 Crew: Quinn Draper stays off The Grey Hands's trade today: the nearest spot, Bar#4463, is 2 h 40 from home (the same on 4 more days: d24-d27)`; `d25 09:18 BountyPaid ... sold a sighting ... for 15`.
- **club_staff: thin (V12 dead).** club_staff_38907 `d22 00:00 Served: Rue Achebe worked the floor of Club#4836 tonight: a room of 0, nobody to serve` (five of seven nights; `d25 00:00 a room of 1, served 1 (Idris Duarte)`). club_staff_38277 (Gambling Den) has no event at all in 7 days.
- **dancer: thin (V12 dead).** dancer_71865 `d22 00:00 danced 5 sets (4 to an empty room) at Strip Club#6072 tonight to a room of 1: 1 tip, 1c`, then `d23 ... 5 sets (5 to an empty room) ... no tips` through d27. Sets and tips exist; the audience does not. dancer_99218 is jailed d21-d24 (bug 2).
- **companion: thin (V12 alive).** companion_98675 `d26 21:00 Served: Sprocket Greaves is now a regular of Yuki Oakes at Spire Lounge#4850` (one regular in the week, V12 had two per pick); companion_87767 only `d23 14:24 stopped Corwin Moreau in the hall of Block#170 about the lift`. Both still travel 3-5.6 h a day (87767 d26).
- **cook: thin (V12 thin).** cook_62847 has no event line in 7 days. cook_27641: `d24 06:56 Assault: Mina Duarte (69 y) attacked Vex Ferreira (57 y)`, Report, `08:20 Pursuit ... gave up the chase (the trail went cold)`, `11:18 Arrest`, `12:20 Sentence ... 10 days`, then identical jail days.
- **fighter: thin (V12 thin).** fighter_42248 `d21 23:00 Bout: Ulric Thorne beat Emeric Lockridge at Fight Pit#1769 (bets paid 164)`, nothing after for the loser. fighter_64359 has the better arc: `d22 00:00 Skipped ... off sick ... Anton Nightingale covers`, `d23 00:00 Diagnosed: vat-cancer`, `Loan: ... 218c short ... begged their friends: Moth Sallow had nothing to spare; Wendell Kimura had nothing to spare`, `14:06 Cured ... 294c on the doctor's slate`. No Bout line for that pick all week.
- **joytoy: alive/thin (V12 thin).** joytoy_102145 `d22 21:00 Served: Cordelia Kimura served Odessa Delacroix (visit) at Dollhouse#6100`, d23, d25, plus `d24 00:00 Skipped: off sick ... Ada Bellamy covers the Contract Host's shift`. joytoy_75937 three visits and 7-10 h of travel a day.
- **madam: alive (V12 alive).** madam_104391 books 25, 48, 52, 79, 67, 106 and bars rough clients: `d26 22:30 Served: Ignatius Stroud roughed up Marek Abernathy at Brothel#5491; Ursula Inchbald threw him out and barred him from the house`; `d27 00:00 ... 106 clients (Adela Tanner 36 ...), 806c; 3 rough, thrown out`. madam_47125 books 11-28 and is assaulted by the clients she barred: `d26 00:01 Story: Nutrix 8 Now: Nikolai Okafor assaulted Undine Yardley by Pixel Fairweather (from the beat)`, `d27 00:01` Zoe Kemp likewise.
- **owner_broke: alive (V12 alive).** owner_broke_7125: `d23 00:07 LaidOff ... the till could not meet the wages (0c in hand against 10c a day)`, `d23 23:18 Quit ... (unpaid, owed 35c)`, `d24 18:00 Confronted: Rook Thorne caught Wilhelmina Yates ... demanded the 35c ... reported ... to the Hall for the wages`, `d26 10:00 Confronted: Yorick Ellery ... left with nothing (grudge 0.46)`, `d27 00:07 the Hall took 2c ... (33c still owed)`. A full creditor chain. owner_broke_7293 is odd (bug 5).

## 3. Round 4's mechanisms seen at 100k

Fired: T4 door's nightly count (both bouncers), club hand's floor night (club_staff_38907), dancer's sets and tips (dancer_71865); T6 wages-owed Confronted and Hall collection (owner_broke_7125); T3 `Pursuit: gave up the chase (the trail went cold)` and `(already in custody)` (cook_27641, dancer_99218), `Cells: missed the shift ... dismissed` and `came out to a changed life: the post ... gone` (dancer_99218 d22-d24); T5 `JobSearch` (Friends asked, boards read "nothing going") and foreman `Hire ... (picked by ... the foreman)` (owner_broke_7293); T2 `[family]` line says why (`play: no School within 1 h`, dancer_99218); T7 folds: gossip 0-7 lines per diary (V12 22-40).
Expected, never seen: any bouncer refusal or turned-away; a staff skip of the door queue; a barred client returning at a door; T1 service at the door, let home, or a repeat take (bug 2); a dancer regular or club-hand tip; a loan collector (no owner here owes a shark).

## 4. Bugs, ranked by what the player sees first

1. **Far commutes are back for some.** joytoy_75937 lives in D3, works Dollhouse#5420 in D4: `d21 12:07 GoTo(Workplace) ... 315 min, 315 tiles`, `d22 13:49 221 min, 313 tiles`; travel 5.3, 10.4, 7.2, 2.2, 9.3, 10.0 h on d21-d26, sleeping in a Capsule Hotel. owner_broke_7293 travels 12.4 h on d25. V12 bugs 1/2 (owner's 10 h commute, errand walks): **better for owners** (owner_broke_7125 under 2 h), **unchanged for joytoy**.
2. **A neglected child stays on a warning while the parent is in the cells.** dancer_99218's `[family]` line is identical d21-d27 (`on the service's warning since d15`) with her jailed d21 13:03 to d24 13:03; pantry holds 2 then 3, no service at the door, no grace ended, no take. Either the warning ignores a jailed parent or the grace outlasts 12 days. New.
3. **A bouncer turns nobody away in 11 nights.** 125 let in a night, "nobody turned away" every time, while the madams bar rough clients nightly. A door that never judges reads as a counter. V12 bug 3 (no door verbs): **better** (a door line exists), the verb is missing.
4. **Street Assaults with no consequence.** club_staff_38907 `d23 11:12` and `14:12` (Rue Achebe, 63 y, attacks 47 y and 66 y adults): no Report, Arrest or Heat. V12 bug 4: **unchanged**, but ages are now printed (V12's ask met, no minors).
5. **owner_broke_7293 flips between owner and Vat Tech with no reason shown.** `d22 00:07 Acquired: Zetatech 6 bought Ripperdoc#5102 from Vale Tanner for 360 (a fire sale: 1c in hand against 26c a day in wages)`, `d24 00:07 Quit: ... quit as Vat Tech (no need of the wage)`, `d24 02:27 Founded: Noodle Bar#6013 ... for 120`, net -232c that day, then `d25 00:00 Hire ... Vat Tech` and `d27 00:00 Hire ... Vat Tech` with no Quit or LaidOff between. The fire sale is a good beat; the rehire twice is unexplained.
6. **Jail days still identical.** cook_27641 d25-d27 and dancer_99218 d22-d23 `sleep 8.0h, jail 16.0h`. V12 bug 6: **unchanged**.
7. **A 36-visit night.** madam_104391 `d27 00:00 Adela Tanner 36`, 106 clients, count swings 25 to 106. V12 bug 8 (31): **worse**.
8. **Gossip flood** (V12 bug 5): **fixed** in this group (madam_104391 7 lines vs 40, cook_27641 1 vs 29). **Witness spam** (V12 bug 10): **better** (bouncer_57196 7 lines, club_staff_38907 2). V12 bug 7 (sale of a shut bar pays the owner, not creditors): **not seen** (no shutdown in this pair); 7125's Hall takes 2c against 33c owed is the chain working, if tiny.

## 5. Dull spots, ranked

1. **Empty rooms.** A club with a room of 0 five nights in seven, a dancer on stage for nobody (net +1 to +8c). Mechanism: guests arrive by the venue's reputation and the punters' evening choice (the Gambling Den door sees 125, so the crowd exists in the city); an empty club loses its staff or cuts the shift, a busy night pays tips.
2. **The door never refuses.** Give the door a roll against arrivals (drunk, armed, colours, a barred face); the refused waits outside and returns with a friend, the bouncer meets him. The madams already have the barred-client half.
3. **Companion and joytoy regulars are thinner than V12** (one regular, 3-4 visits per pick). Quiet, not broken; the hooks (a returning regular, a jealous partner) are missing.
4. **The bout has no sequel** for the loser; fighter_64359's cancer arc shows how well a consequence reads, and the Bout never reaches that pick.
5. **Gang crew skip** (`stays off the trade ... 2 h 40 from home`, x5 days for bouncer_94173): a recruit with no reachable spot should be given one, or the gang places him; a repeated excuse is dead air.
6. **Jail days** want a guard's meal, a cell fight or a visitor.

## 6. Too much

- Nothing reads as broken harshness. madam_47125 assaulted twice by the clients she barred, both in the Nutrix feed, is the right kind of consequence.
- Monotony: club_staff_38277, cook_62847 and the bouncers' days (bouncer_57196 d24-d26 identical: 5.9 h work, 7.8 h idle, net +5c to +6c) read as a metronome; the new `Served` line is the only thing that changes the day.
- Numbers: 106 clients and 36 visits for one person in a night is arithmetic, not a night.
