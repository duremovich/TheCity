# Shadow V12 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: 100k island, seed 42, d21-d27, two picks each (18 diaries). Cites are `file dDD HH:MM` under `target/shadow_v12/isle100k_s42_d21/`.
Method: DAY SUMMARY lines and camera tables for all 18, all non-Witness `[event]`/`[law]` lines, ACTION histograms, `[rep]`/`[family]` greps, full reads of cook_72225 d22-d27, owner_broke_10159 d23 and d27, club_staff_36728 d25, and the madam d24 and d26 stretches.

## 1. Verdict

Mixed against V11, slightly better. The money-and-law chains still work and got a new victim-named charge (cook_72225: assault, report, arrest, 10 days), the madam books and bars rough clients, and fighters now "move nearer work". The distance fix reaches the commute but not the errand: owner_broke_10159 walks 5 h each way from the Sump to his own bar, 10 h a day, and a madam spends two hours on foot to enjoy a strip club in the next district.
Bouncer, dancer, club_staff and cook are still a metronome of Wage plus ClerkWork (every vice job's ACTION is ClerkWork), with no door, tip or sequel line in 8 of 18 diaries; stories start and are dropped (the barred client, the lost bout, the unreported bouncer assaults).
No minor is a victim, worker, client or fighter anywhere in this group: the `[family]` lines show only the picks' own 11-14 y children at School or home (play).

## 2. Per archetype

- **club_staff: dead (V11 thin).** Both picks work 5.6-5.8 h and sit idle 7-12 h; the shift is `ClerkWork` x3 plus a `Break`. club_staff_36728 `d25 18:02 ... ClerkWork->Club#5807` repeats all week; commute is 3 tiles so V11's 7.7 h travel is gone. club_staff_35238 witnesses seven Dealing lines (`d21 13:48`, `d22 01:53` ...) and does nothing.
- **bouncer: dead (V11 dead).** bouncer_95519 at Strip Club#5671: no event in the week bar `d27 14:26 Witness ... Dealing`. bouncer_59276 at Fight Pit#3160: two street Assaults off shift (`d23 14:41 Juro Ito attacked Volt Reyes`, `d26 13:41 ... Nadia Achebe`), no Report, no arrest, no door. S5's "bouncer's rough client and bar" never appeared.
- **dancer: dead (V11 thin).** dancer_79123 and dancer_105303: no event at all in 7 days. Work 5.8 h, net +1 to +10c a day, nothing marks the stage.
- **companion: alive (V11 alive/thin).** companion_73001 `d24 21:00 Served: Elowen Whitlock is now a regular of Clemence Ravensworth at Spire Lounge#4884`, `d26 21:00` Nikolai Osgood; companion_98675 `d21 21:00` Aldo Kemp, `d24 21:00` Fenn Upton. A regular every two or three nights; thin but real. The 109- and 88-tile commutes still cost 3-5 h some days (companion_98675 d24 5.5 h).
- **cook: thin (V11 alive/flat).** cook_34100 is a Wage-ClerkWork week with no event. cook_72225 is the one story: `d22 12:25 social move Intimidate from Clemence Santoro: failed, BACKLASH`, `12:48 Assault`, `15:24 Treated: Dahlia Yates patched Cordelia Wexford up in the back room of Back Room#1456 for 5c, no questions asked`, `22:20 Arrest`, `d23 00:12 Sentence ... 10 days for Assault on Clemence Santoro`, `d24 00:01 Story: Achebe Holdings Feed ...`.
- **fighter: thin (V11 alive).** One Bout line each and both lose: fighter_17230 `d26 23:00 Bout: Aldo Nightingale beat Hex Dunmore at Fight Pit#2594 (bets paid 14)`, fighter_42248 `d24 23:00 Bout: Idris Greaves beat Emeric Lockridge (bets paid 0)`. No injury, purse or rematch. fighter_17230 `d23 00:12 Housed: moved nearer work, into Block#1343` is S1 working (travel 6.7 and 9.1 h on d21/d22, 3.1 h from d23).
- **joytoy: thin (V11 alive/thin).** joytoy_128942 `d25 21:00 Served: Mina Ellery served Nyx Bartlett (visit) at Dollhouse#5907`, `d26 21:00` Lorcan Sato; joytoy_71782 one visit (`d21 21:00`). Three Served lines in 14 pick-days against V11's nightly run; joytoy_71782 travels 3-6 h a day on a 184-tile commute.
- **madam: alive (V11 alive).** madam_104391 nightly bookings 33, 27, 70, 79, 51, 108 and eight barred rough clients: `d24 21:15 Served: Quill Novak roughed up Adela Tanner at Brothel#5491; Ursula Inchbald threw him out and barred him from the house`. madam_47125 books 19-35 a night; `d24 10:00 hailed a cab (in a hurry) from D3 Neon Quarter to Ripperdoc#5628` (S5 cabs reach her).
- **owner_broke: alive (V11 alive).** owner_broke_10159 closes an arc: `d21 00:07 Loan: borrowed 140c from Mateo Eastwick (a loan shark) to keep Noodle Bar#5198 open`, `d26 00:07 UnderWater ... shut for good`, `d27 00:07 Acquired: Rhoswen Ito bought Noodle Bar#5198`, `d27 00:08 Starving`, `d27 10:47 Marriage`. owner_broke_8150: `d22 14:43 Quit: Tamsin Delacroix walked out ... (unpaid, owed 21c)`, `d24 00:07 UnderWater ... hangs on`, then a flat week with no Hall chain (as V11's 15514).

## 3. Round 3's mechanisms seen at 100k

Fired in this group:
- **S3 charge names its victim; report, arrest, sentence, feed story:** cook_72225 (above). Jail days are a flat `sleep 8.0h, jail 16.0h` d23-d27; cellmates become Friends hour by hour (`d23 00:42 met 4 in the cells`, Corwin Underhill to +1.00 by 20:12).
- **S1 housing nearer work:** fighter_17230 d23, fighter_42248 `d21 00:12 Housed ... Block#1324`.
- **S5 cab hail:** madam_47125 `d24 10:00`. Back-room patch: cook_72225 `d22 15:24`.
- **S6 school-age kids seeded and attending:** companion_73001 `d23 09:30 [family] my children: Ivo Chrome#109230 (12 y) at School#5117 ...` (home d21/d26/d27, school d23-d25); joytoy_128942 `d22 00:08 SchoolFee -2`, children at School#5758.
- **S7 rest day:** `DAY SUMMARY d27 (rest day: no shift)` and the `| rest day: no shift` suffix on every worker's needs line; `the city's rest day` for the unemployed and jailed. Work 0.0 h on d27 is design, V11's bug 1 is closed.
- Round 2 still fires: madam bookings and the barred client, Dollhouse visits, Pit bout with bets, owner collapse.

Expected, never seen: any bouncer door line (S5); any club hand's reaction to Dealing; "resisted arrest", the guard's collar, jail meals or jail duty in blocks (S3; cook_72225 has no meal line in 5 jail days, which is not evidence of absence); "stays off the trade", "could not make the trip", "sat out the raid", "N in a row", an order left at the door (S1/S2: nothing in this group's lives needs them); the cabbie's cut; a hit heard of; crowd and hourly counts for a club or pit.

## 4. Bugs, ranked by what the player sees first

1. **A broke owner commutes 10 h a day to his own bar (new, S1 gap).** owner_broke_10159 lives in Block#3074 (D1 The Sump); Noodle Bar#5198 is in D4, 273 tiles. `d23 03:05 GoTo(Home) ... 230 min, 208 tiles` after buying food in D4, sleep in D1, `d23 13:03 GoTo(Workplace) ... 308 min, 273 tiles`. Travel 6.5, 10.1, 10.6, 10.4, 10.4 h on d21-d25 with 5.5-5.7 h of work. The "Housed: nearer work" rule that fired for the fighters did not fire for the owner (maybe it needs rent he lacks); let an owner sleep at his business.
2. **Leisure and errand trips ignore distance (S1 covers choices, not these).** madam_104391 `d24 09:33 GoTo(Seller) -> Strip Club#5548 (D3), 57 min` to Enjoy 91 min, `12:03` 57 min back, while she runs a brothel; the same day `d24 00:02 GoTo(Seller) -> Security Office#4644, 108 min, 87 tiles` for a Deck T1. joytoy_71782 `d22 00:47 GoTo(Home) 213 min, 192 tiles` then `d22 15:22 GoTo(Workplace) 116 min, 176 tiles`. companion_73001 `d25 06:18 GoTo(Seller) -> Boutique#4998, 68 min` and 68 back. Pace is fine; what they walk to is not.
3. **Bouncers and club hands never meet their trade (V11 bug 4: unchanged).** Zero door, refusal or Brawl lines for two bouncers and two club staff in 28 job-nights; the only job action is `ClerkWork`. The first thing a player at a club door would see.
4. **Street Assaults with no consequence (new).** bouncer_59276 `d23 14:41` and `d26 13:41` (witness Sana Thorne "saw ... Assault and ran", new Enemy edge), madam_104391 `d26 00:36 Assault: Ursula Inchbald attacked Juniper Jarrow` after a midnight `GOAL Fight ... odds 1.00`. No Report, Arrest or Heat follows, while cook_72225's two witnesses reported at once. Under-18 check: victims have adult-range ids and nothing marks them as children, but the diary does not print the victim's age; add `(NN y)` to the Assault line so this rule can be read.
5. **Gossip flood (V11 bug 7: unchanged).** madam_104391 40 `GOSSIP ABOUT ME` lines, cook_72225 29 (the same `Assaulted by Cordelia Wexford#72225 on Clemence Santoro#50705` told about 20 times on d24 07:07-09:58), companion_98675 32, madam_47125 22. One line with a count would do.
6. **Jail days are identical (S3 half-seen).** cook_72225 d23-d27: `sleep 8.0h, jail 16.0h, net 0c` five times; dread decays 0.92 to 0.87 and nothing else. No meal, guard, or cell event. V11's Starving-in-cells is not repeated (better), but no meal line proves the fix either.
7. **Sale of a shut bar pays the owner, not the creditors (new).** owner_broke_10159 `d26 00:07` owes two cooks 20c each and a loan shark 224c; `d27 00:07 +90c (Sale)` stays with the owner, who buys 12c of food at `00:23`. Nobody is paid and no collector arrives.
8. **A 31-client night for one worker (new).** madam_104391 `d27 00:00 Served: booked 108 clients ... (Adela Tanner 31, Vivienne Chrome 16, Fenn Bartlett 14 and 4 more), 811c`; d25 79 clients (Adela 15, Fenn 15, Ines 14). One person 31 times in a six-hour shift is bookkeeping, not a night; the count swings 27 to 108 with no cause shown.
9. **Far trips for 100-tile commuters (V11 bug 3: better).** companion_98675 travel 3.3-5.5 h d22-d26, fighter_42248 3.2-3.9 h daily on 100 tiles. Gone is the 7.7 h metronome; still a quarter of a day.
10. **Witness spam (V11 bug 9: better).** Only club_staff_35238 shows a run (ten Witness lines in a week, mostly Dealing); no four-in-a-minute clusters.

V11 bug status: d27 shifts dropped - **fixed** (explicit rest day); far posts 7.7 h - **better**; stim run by a salaried attendant - not seen; bouncers have no door - **unchanged**; dread 0.99 with no deed - not seen at that level (companion_98675 starts at dread 0.53, decays to 0.30, no cause line in window - **unchanged in kind**); fired and rehired in four minutes - not seen (owner_broke_10159's three LaidOff are one shutdown); no food in the cells - **better**, unproven; gossip flood - **unchanged**; Fare Evasion witness spam - **better**.

## 5. Dull spots, ranked

1. **The door and the stage have no verbs.** Bouncer, club_staff, dancer, cook and fighter all act as `ClerkWork`. Give the bouncer a nightly roll against arrivals (drunk, armed, colours, grudge): the turned-away waits, returns with a friend or a knife. Dancers and club hands reuse the companion regulars mechanism: a tip, a regular who shows again.
2. **The bout has no consequence.** Both fighters lose and clerk on. The loser gets a wound to heal (the back room exists), a purse split, a rematch the Pit posts; bets paid 0 and 14 are the only trace.
3. **The barred client is never seen again.** Eight names, none repeats. A barred client returns outside the door a night later with a friend; the bouncer, once he has a verb, meets him.
4. **Witnessed Dealing leads nowhere for people who work beside it.** club_staff_35238 sees seven in a week. A club hand who sees Dealing reports it to the Hall, tells the gang, or is warned by a dealer.
5. **The quiet owner gets no sequel.** owner_broke_8150 hangs on four days at 0c with no Hall chain, shark or layoff, while the other owner runs a full arc. A missed loan payment has a collector.
6. **Jail days.** A guard's meal, a shift of jail duty, a cell fight or a visitor would break five identical days.
7. **The loan shark after a shutdown** (bug 7): a collector arrives the night the bar sells.

## 6. Too much

- Nothing reads as broken harshness. Eight barred clients in five nights is plenty but plausible for a brothel; the owner's loan, layoff, shutdown, hunger and 90c sale is the right tone.
- Monotony: the club_staff, bouncer and dancer weeks are the same 24 hours seven times (V11's complaint about club_staff_34383, now spread to the whole shift group).
- Noise: the 20-line gossip run and the 40-line madam list bury real events.
- Numbers: 31 clients in one night for one worker reads as arithmetic, not a night.
