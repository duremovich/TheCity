# Shadow V14 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: 100k island, seed 42, d21-d27, two picks each (18 diaries). Cites are `file dDD HH:MM` under `target/shadow_v14/isle100k_s42_d21/`.
Method: DAY SUMMARY lines for most picks, every non-Witness `[event]`/`[law]`/`[family]` line (first of each kind) for all 18, GoTo and money greps on companion_98675. `target/r5_100k/analyze.txt` does not exist yet; not read.

## 1. Verdict

Slightly better than V13: dancers now play to real rooms with tips and a named regular (dancer_79451), owners keep their creditor chain, and the far commutes of V13 are gone from this group.
The door and the ring are still counters. Both bouncers say "nobody turned away" every night they work (7 of 7 lines read); neither fighter has a single Bout line in 7 days. Cell days are still identical.
No minor is harmed, a victim, a client or in vice in this group. Every Assault/Wounded line prints ages (19/26, 59/23, 49/51, 26/31), all adults; dancer_79451 is 18.

## 2. Per archetype

- **bouncer: thin (V13 thin).** bouncer_96862 `d25 00:00 Served: Nico Yardley kept the door of Fight Pit#2344 tonight: 59 let in, nobody turned away` (29, 19, 28, 59, 18, 30 across six nights). bouncer_60915 at Strip Club#6028: `d26 00:00 ... 2 let in, nobody turned away`, one line in 7 days.
- **club_staff: thin (V13 thin).** club_staff_36159 (65 y croupier) `d23 00:00 Skipped: Elowen Vasquez is off sick from Gambling Den#4830 (1 sick days in 14); nobody covers the Croupier's post`, again d24, then nothing. club_staff_38055 has no event line in 7 days (only gossip).
- **dancer: alive (V13 thin).** dancer_79451 `d24 00:00 Served: Pixel Adeyemi danced 5 sets at Strip Club#5736 tonight to a room of 13: 13 tips, 13c; regulars Ivo Reyes`, then `d25 00:00 ... room of 21: 21 tips, 21c; regulars Santiago Penhallow`. dancer_55114 is the empty-room case: `d22 00:00 danced 6 sets (5 to an empty room) at Club#5111 tonight to a room of 1: no tips`, ending in a theft arrest (d25 11:03) and a 6-day sentence (d25 12:46).
- **companion: thin (V13 thin).** companion_98675 has no event line in 7 days (V13: one regular); days read work 4.5-5.8 h at Spire Lounge#4850 plus 1-5 h travel. companion_73001 has only Preached, Housed and `d25 11:07 GrudgeFormed: swore against Vivienne Draper (a robbery, 0.56)`, which goes nowhere.
- **cook: thin (V13 thin).** cook_66721 (71 y): one `Visited ... (an elder parent)` in 7 days, work 8.9 h every day. cook_26799 is a clean chain: `d25 05:20 VehicleStolen ... Flyer T1`, Report 09:13, Arrest 09:54, `Pursuit: ... gave up the chase (already in custody)` 09:59, Sentence 6 days 10:32.
- **fighter: dead (V13 thin).** No Bout line for either pick. fighter_42709 steals, is wounded by a 19 y (`d22 00:44 Wounded: Gwendolyn Inchbald (19 y) went down, bleeding, at Ilse Fairweather (26 y)'s hands`), resists arrest (02:05), jailed d22 02:57 for 6 days, then five identical jail days. fighter_65089 `d21 10:01 Theft: stole 2 food`, `d22 13:04 Assault (23 y on 41 y)`.
- **joytoy: alive/thin (V13 alive/thin).** joytoy_53526 `d22 21:00 Served: Varek Lindqvist served Joss Brandt (visit) at Dollhouse#5000` x3 that night, 2-3 more on d24, d25, d26. joytoy_102126 has no Served line (work 5.8 h a day, net 0c d21-d24).
- **madam: alive (V13 alive).** madam_104391 `d27 00:00 booked 89 clients ... 677c through the house; 1 rough, thrown out`, `d26 18:45 Tristan Oakes roughed up Ines Northcott; Ursula Inchbald threw him out and barred him`, sequel `d27 00:01 Story: Nutrix 3 Now: Tristan Oakes assaulted Ursula Inchbald`. madam_35331 (small house, books 2-9) has the same chain: `d22 23:15 roughed up ... barred`, `d25 00:01 Joss Kimura assaulted Hesper Duarte`.
- **owner_broke: alive (V13 alive).** owner_broke_6522: `d22 00:07 LaidOff ... (5c in hand against 9c a day)`, `d22 16:00 Confronted: Wren Yates ... demanded the 9c ... paid 5c, all there was`, `d23 00:07 Loan: missed a payment ... 171c still owed`, `d25 14:00 ... would not pay; left with nothing (grudge 0.46)`, `d26 10:00 GrudgeFormed ... (a beating, 0.50)`, `d27 13:04 Treated: patched up in the back room for 6c ... no questions asked`. owner_broke_6426 is best: `d26 00:07 Repossessed: Florian Abernathy took Bar#4426 ... for the 448c still owed`, after a Shakedown arrest d25 22:24 and a Crew court at the Hideout (d24 21:00, d25 21:00).

## 3. Round 5's mechanisms seen at 100k

Fired: U7 `Pursuit: ... (already in custody)` (cook_26799 d25 09:59, fighter_42709 d22 02:05); U6 back-room Treated with a stranger chipping in (owner_broke_6522 d27 13:04); U1/U2 `[family]` lines say why a child is home (`the 2c fee and the El's 2c of fares ... beyond the purse`, companion_73001 d24) and a seat appears (joytoy_102126 d21 fee beyond purse, d22 the same three children `at School#5560 (20 min walk)`); U2 `Custody: the child protective service came to the door` (owner_broke_6426 d21 00:10).
Expected, never seen: a bouncer refusal; any Bout; a Served line for joytoy_102126 or companion_98675; a take after a second warning (owner_broke_6426's children stay on the warning d21-d27). U3, U4, U5 are not in this group's picks.

## 4. Bugs, ranked by what the player sees first

1. **The Custody line lists 18 children from ten-odd families at one door.** owner_broke_6426 `d21 00:10 Custody: ... came to the door of Block#683: Priya Garland (10 y), Ulric Garland (4 y), ... Osric Ostrowski (17 y), ... Ada Bellamy (14 y) ... Idris Lockridge (5 y) went 2 days unfed; Rhys Garland and Jocasta Penhallow and Nico Sallow and ...`. One event for a whole block, every child "unfed", every adult named; the pick's own `[family]` shows only his three. Either the message merges the block's warnings or the take counted a block, not a household. (Also: it is a warning, not a take; none of them is harmed.) New.
2. **The warning goes nowhere.** owner_broke_6426 `on the service's warning since d21` on every [family] line d21-d27, pantry 17-19, and unchanged after the parent is jailed d26 00:44 for 6 days. V13 bug 2 (warning outlasts a jailed parent): **unchanged**; the jailed-parent case did not recur here, but this one shows a second warning inside the window never firing.
3. **A bouncer who never judges.** 7 of 7 nightly lines "nobody turned away" while madams bar rough clients nightly. V13 bug 3: **unchanged**.
4. **Fighters with no Bout.** Neither fighter has a Bout; the pit's bouncer sees 18-59 a night and the ring never prints. V13 had one Bout with no sequel for the loser: **worse**.
5. **Sick croupier, nobody covers, no consequence.** club_staff_36159 d23/d24 `nobody covers the Croupier's post`, then silence. Minor.
6. **Identical jail days.** fighter_42709 d23-d27 `sleep 8.0h, jail 16.0h`, Precinct#4485 1440m each. V13 bug 6: **unchanged**.
7. **A 42c food buy.** companion_98675 `d27 06:54 -42c (Food) ... Nutrix 10`, purse 61 to 19, net -49c on a rest day. If it is a household stock, the line should say so. Cosmetic, new.
8. **Gossip flood.** dancer_55114 `d27 08:29 GOSSIP ABOUT ME ... told 182 times today (92 to new ears, 90 heard already)`; 60 such lines across these 18 diaries. V13 bug 5 (fixed in this group): **worse**.
9. V13 bug 1 (far commutes): **fixed** here (largest travel day 7.8 h, bouncer_60915 d24, an errand day, no 5-10 h commutes). V13 bug 7 (36-visit night): **better** (largest single regular 22, madam_104391 d27; 89 clients a night is still arithmetic). V13 bug 4 (Assaults with no consequence): not re-checked.

## 5. Dull spots, ranked

1. **The door and the ring.** A door that rolls arrivals (drunk, armed, colours, a barred face), sends the refused away to come back with a friend; a Bout line (win, loss, injury, bets paid) for the pit's fighters. Both venues see a crowd and nothing happens to it.
2. **Companion and joytoy have nothing to say.** companion_98675 and joytoy_102126 are a wage and a commute. A returning regular, a jealous partner, a client who wants more; the madam half already exists.
3. **The empty club still bleeds into crime.** dancer_55114 plays five of six sets to an empty room three nights and then steals (d25). That is a story if it shows: an empty house cuts her pay or her set and the diary says so before the arrest.
4. **Crew skip repeats.** owner_broke_6426 `d24 00:07 Crew: stays off The Salt Widows's trade today: the nearest spot, Bar#4463, is 6 h 27 from home`; a recruit with no near spot should be placed. The court at the Hideout is the good half.
5. **Elder cook (71 y)** works 8.9 h every day for +6c to -6c; by doctrine he keeps his post, but a bad day (a burn, a drunk, a visit that hurts) is missing.

## 6. Too much

- Gossip: 60 GOSSIP ABOUT ME lines in 18 diaries (one "told 182 times today") reads as diary noise.
- Nothing reads as broken harshness. The madam assault feed and the owner's repossession are the right size.
