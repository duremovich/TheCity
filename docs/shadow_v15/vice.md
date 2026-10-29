# Shadow V15 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: 100k island, seed 42, d21-d27, two picks each (18 diaries; the ids differ from V14's). Cites are `file dDD HH:MM` under `target/shadow_v15/isle100k_s42_d21/`.
Method: DAY SUMMARY lines for all 18, every non-Witness [event]/[family]/[law] line, targeted greps for Bout, turned away, Fare, GOSSIP, minors. `target/r6_100k/analyze.txt` read only for door and Bout totals.

## 1. Verdict

Clearly better than V14 where round 6 aimed: the door now judges (the broke are refused by name), the pit prints Bouts with recurring names, and the club floor has room sizes and regulars.
Companion, joytoy, cook and club_staff are still a wage and a commute; about half the picks have no story that starts. The madam feed is V14's (rough client, barred, assault Story) and is still the best loop.
No minor is harmed, a combatant, a client or in vice in this group. Every Assault/Brawl/Bout/Fare line prints ages, all adults (Brawl 71 y vs 41 y; Fare 56/30, 24/30, 60/61); a grep for (0-17 y) on harm lines is empty.

## 2. Per archetype

- **bouncer: alive/thin (V14 thin).** bouncer_61621 `d21 21:15 Served: Maren Carrow turned Elowen Sato away at the door of Strip Club#5655 (broke)`, `d24 00:00 ... 12 let in, 1 turned away (broke 1)`; 5 of 7 nights turn someone away. Only "broke" is ever the reason (bug 3). bouncer_100403 is jailed d21-d26 (resisted arrest, 6 days): a dead week.
- **club_staff: thin (V14 thin).** club_staff_35574 `d26 00:00 Sprocket Stroud worked the floor of Club#4874 tonight: a room of 111, served 45`; rooms of 25-111, regulars named d23, nothing happens to the staffer. club_staff_36880 has the one story: `d23 00:23 Report: Zed Ambrose cried out and Anton Ellery took the report: Juro Upton for Grand Theft`, Arrest d23 12:11, Sentence 6 days 13:20 (the V3 chain works), then four identical jail days.
- **dancer: alive (V14 alive).** dancer_74279 `d21 20:32 Hesper Oyelaran got rough with Echo Duarte ...; Hana Oakes threw Hesper Oyelaran out, barred`, then `d21 22:17 Treated: Vex Moreau patched Echo Duarte up at Ripperdoc#5050 for 12c (3c of it to the doctor)`; three roughings in two nights. dancer_103667 is the empty-room case: `d22 00:00 danced 4 sets (2 to an empty room) ... 6 tips, 6c`, rooms of 1-6 all week.
- **companion: thin (V14 thin).** companion_102505 `d21 21:00 Slate Novak is now a regular of Ottilie Mercer at Spire Lounge#4883` (three regulars in a week, the most this archetype has shown), and `d23 18:00 Brawl: Priya Wexford (71 y) started a brawl with Ottilie Mercer (41 y) ... Priya Wexford won`, grudge formed, no sequel. companion_87767 has no event line in 7 days, travel 6.4 h on d23.
- **cook: thin (V14 thin), one pick a gang story.** cook_23744 (Viktor Okafor) is a full chain: `d21 05:50 Fare: ... beat Viktor Okafor ... left on the platform`, `d21 08:15 GangJoin ... The Salt Dogs (miserable)`, `d22 11:00 Turf: ... at war`, `d22 18:00 Brawl ... a rival's colours`, `d23 00:05 truce ... 12 of their 12 beaten, dead, jailed or gone`, `d24 00:06 ContractPosted: ... a tag on Viktor Okafor for 246`, `d25 05:14 banned from the El for 14 days`, `d26 00:07 missed ... court summons`. cook_60441 has no event line (work 5.8 h, net 0 to +4c).
- **fighter: alive (V14 dead).** fighter_63572 is Alaric Zhou: `d22 23:00 Bout: Casimir Wexford beat Alaric Zhou at Fight Pit#1963 on the undercard`, `d24 23:00 Alaric Zhou beat Casimir Wexford`, `d26 23:00 Casimir Wexford beat Alaric Zhou`: a rivalry of three. fighter_40935 two Bouts (d22, d25), different pairings.
- **joytoy: thin (V14 alive/thin).** joytoy_77256 `d22 20:34-22:29 Ivo Brandt served` six customers in one night at Dollhouse#5063; the same week `d22 16:38 Fare: Jocasta Bartlett (60 y) beat Ivo Brandt (61 y)`, `d24 00:28 VehicleStolen`, and the kids out of School d22 on the 2c fee, back d23. joytoy_105788 has Hire lines and `[family]` lines only.
- **madam: alive (V14 alive).** madam_104391 `d27 00:00 booked 101 clients ... 782c through the house; 4 rough, thrown out`, `d26 18:45 Tristan Oakes roughed up Ines Northcott` and three more by 22:30, Story `d27 00:01 Roxy Crane assaulted Ursula Inchbald`. madam_35331 grows 5 clients d22 to 39 d27, one rough per night, Story `d26 00:01 Sato Feed: Ada Ramirez assaulted Hesper Duarte`.
- **owner_broke: thin (V14 alive).** owner_broke_6522 (Smoke Delacroix) `d22 00:07 Loan: missed a payment ... 196c still owed`, `d22 15:00 Ottilie Northcott called (off screen) ... gave a beating`, then nothing d23-d27. owner_broke_6737 is a Ripperdoc owner (Nerys Singh, 66 y), not broke: 7 installs d24, `d25 20:30 Punished: Wren Zhou smashed the till ... took 80c, shut 1 day`, `d26 10:35 gave up the post of Orderly to take the chair of Singh Holdings` (net -307c). A good week, wrong pick.

## 3. Round 6's mechanisms seen at 100k

Fired: V6 the door (`turned Elowen Sato away ... (broke)`; analyze door_refusals=7113 over 30 days); V6 the pit's own bout book (fighter_63572 three Bouts, fighter_40935 two, all `on the undercard`; analyze Bout 1677); V6 patch-up (dancer_74279 d21 22:17); V5 war and peace (cook_23744 d22 11:00 war, d23 00:05 truce); V3 report/arrest (club_staff_36880 d23); V1 `[family]` lines name why a child is home (`the 2c fee and the El's 2c of fares ... beyond the purse`, joytoy_77256 d22) and the seat returns d23; V4 cab (owner_broke_6737 `d25 19:17 hailed a cab (flush)`).
Expected, never seen: a refusal for drunk, armed or colours (all 8 refusal lines in these diaries read "broke"; drunk may exist off screen); a main-event Bout (all undercard); a Bout injury, purse or bet; a Custody/take line (none of these picks is a hungry household).

## 4. Bugs, ranked by what the player sees first

1. **The cook's commute is back.** cook_23744 `d21 travel 12.9h`, `d23 10.7h`, `d25 13.3h`; `d25 05:45 GoTo(Workplace) ... 263 min, 236 tiles` from The Sump to Noodle Bar#6040 in Neon Quarter, `d25 18:02 GoTo(Seller) ... 222 min` to the Hideout. He is moved nearer work only on `d27 00:12 Housed`. Likely cause: a gang recruit whose job was hired far from home. V14 bug 1 (far commutes): **worse** in this group. `d22 00:07 Crew: stays off The Salt Dogs's trade ... 6 h 28 from home` is V14 bug 4: **unchanged**.
2. **Barred men come back the same night.** dancer_74279 `d21 20:32 Hesper Oyelaran ... barred from the door`, then `d22 00:00 danced a set ... to a room of 4 (Hesper Oyelaran, ...)`, 3.5 h later in the same room. New. Either the bar does not hold or the rough roll bypasses it.
3. **The door has one reason.** 8 of 8 refusals say `broke`. V14 bug 3 (a bouncer who never judges): **better** (it judges now), but narrow. The Fight Pit's bouncer (bouncer_100403) is in jail, so the drunk/colours path is untested here.
4. **Bouts have no stakes for the fighter.** fighter_63572/Alaric Zhou loses 1 of 3 on the undercard and the diary shows no purse, no hurt, no loser's line, no patch-up; his work reads `ClerkWork` at the pit. V14 bug 4 (fighters with no Bout): **fixed** (Bouts print); consequence missing.
5. **Brawl/grudge with no pickup.** companion_102505 `d23 18:00 Brawl ... Priya Wexford (71 y) ... won` and `GrudgeFormed: Ottilie Mercer swore against Priya Wexford`; nothing through d27. V14's GrudgeFormed-goes-nowhere: **unchanged**.
6. **Identical jail days.** bouncer_100403 d22-d26 and club_staff_36880 d24-d27 `sleep 8.0h, jail 16.0h`, Precinct#4485 1440m. V14 bug 6: **unchanged**.
7. **Gossip flood.** 259 `GOSSIP ABOUT ME` lines across the 18 diaries (V14: 60); `madam_35331 d27 04:17 ... told 22 times today (22 heard already)`. V14 bug 8: **worse (~4x)**.
8. Not re-seen in these picks: V14 bug 2 (warning outlasts a jailed parent), bug 5 (sick croupier), bug 1 (18-child Custody line), bug 7 (42c food buy); bug 3 of V13 (36-visit night) is **same shape** (madam 101 clients, a single regular 22).

## 5. Dull spots, ranked

1. **Companion, joytoy, club_staff and cook_60441 are a wage and a commute.** companion_102505 already has three regulars: let one of them be jealous or ask for more (a fight, a gift, a tab), so the regulars list turns into a plot.
2. **Pit bouts without stakes.** A bet book (the bettors get diary lines), a purse, the loser walking to the Ripperdoc (V6's patch-up exists), a main event the crowd is told of. The Casimir/Alaric rivalry is the seed.
3. **owner_broke_6522 goes quiet after one beating.** The 196c debt just sits. The creditor's man should come back (shop sold, the kid out of School, the owner seeks a gang) as it did for V14's owner_broke_6426.
4. **The door has one reason.** Roll arrivals for drunk, armed, colours, a barred face; send the refused on to the next door or back with a friend.
5. **Barred clients return unpunished (bug 2)**: a returner should be thrown out harder, or be the next night's Assault.

## 6. Too much

- Gossip (bug 7): 259 lines and "told 22 times today (22 heard already)" is diary noise; collapse already-known to one line per day.
- The madam feed (madam_104391 d26 four roughings in four hours, 101 clients, 782c) is harsh and a bit monotonous but reads as a house under pressure, not broken.
- Nothing else reads as broken. No nightly war in this group: one war and its truce in the cook's diary.
