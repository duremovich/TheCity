# Shadow V15: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). 100k island, seed 42, d21-d27, main `bf63dee`. Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v15/isle100k_s42_d21/`. Ids differ from V14, so comparison is by archetype. Counters: `target/r6_100k/analyze.txt`.

## 1. Verdict

- **Gangs now have a street presence, and it is mostly a spray-can.** V14 had no gang life beyond a doorstep; V15 has a held corner, holder lines, a mark on the wall and one real war. In the 16 lives: 0 truce, 0 crackdown, 1 corner fight, 2 war lines, 1 muster, 1 raid brawl, 1 Raid result. Alive in the seams, thin in the telling.
- **The war reads, but only the leader hears it.** `gang_leader_68028` (14 men) is at war on two fronts against a 57-man crew and a 5-man one and calls the gang off the corners after 3 are beaten (`d24 00:05`); then nothing. Members in other diaries never hear of a war.
- **Under-18 rule holds.** A grep of every Assault/Brawl/Theft/Extort/Deal/Death/corner line across the 16 for any age 0-17 returned zero hits. Victims 40-69 y. No child in a corner fight or war line.

## 2. Per archetype (V14's word in brackets)

- **gang_member: alive, one (thin).** `_6731` (40 y, homeless, Ninefold, 14 men): `d21 19:00` "holds Ninefold's corner by Street Market#4271 (4 more of theirs on it)"; `d22 00:00` assaults a 69 y at the corner and, same minute, "The Hollow beat Ninefold back on the corner ... 6 against 5, and hold it now"; `d23 00:05` "the blocks are contested (22 of theirs, 17 of Ninefold's)"; `d24 02:55` finds a body. `_11761`: 20 goals, nothing printed.
- **gang_leader: alive, one (alive, one).** `_68028` (Ghost Haas, Neon Lanterns, 14): `d23 00:05` "called 8 of theirs about the blocks to the corner by Block#2785 ... The Grey Dogs claim the blocks too", `d23 02:00` "at war ... Cordelia Upton leads 57 of The Salt Widows", `d23 03:00` second war, `d24 00:05` lies low. Court nightly at `Hideout#4562` with named absences. `_19636`: 52 goals, little printed.
- **purist: thin (thin).** `_48604`: `d25 16:57` Preach 31 min, `d25 14:50` "turned away" at the Precinct. `_39862`: raid-order header, no raid.
- **cultist: alive, one (thin).** `_62792` (Drowned Choir 2, 21 men, order Raid): `d21 18:32` "muster called" interrupts Unwind, PLAN Raid on Precinct#4485 [GoTo(MusterPoint) > Muster > GoTo(RaidTarget) > Brawl], `d21 21:51` Muster done. Then bug 1. `_17944` thin.
- **scav: thin (thin).** `_79520` 9 corner and 9 Turf lines, 2 Deal; `_98910` raid header only. No find lines this week.
- **dealer: alive (alive, one).** `_101293` (Juniper Ostrowski, 61 y, Unplugged): `d22 18:31` Deal 42 min, then four dealings `19:01-19:05` each with an 8-witness line, `d22 19:02` "Brenna Stroud reported", `20:21` resists arrest and fights the 41 y guard, `d26 20:00` brawl with a 69 y at a Club over "a rival's colours". A real arc, a sloppy dealer. `_66549`: 12 goals, 1 Deal.
- **runner: thin (thin).** `_62946`, `_83343`: 0 Deal, 0 Muster, 0 raid; `_83343` has 5 Turf lines. No JackedIn chain this week.
- **raider: thin (dead).** `_82950` (Aldo Ito, Neon Dogs): 17 Deal lines, daily "holds The Neon Dogs' corner by Block#720 (12 more of theirs on it)" at `d24 18:00`, then 20, 3, 6. A raider that only deals. `_27570`: same shape (14 Turf lines, 1 Extort).

## 3. Round 6's mechanisms seen at 100k

Fired:
- **V2 turf you hold:** `gang_member_6731 d21 19:00`, `raider_82950 d24 18:00` "holds ... corner ... (N more of theirs on it)". Spray witness: `gang_member_6731 d21 18:00`, `d27 01:00`, `gang_leader_68028 d23 00:00`.
- **V2 dealing on the corner:** `dealer_101293 d22 19:00` "holds The Unplugged's corner by Club#5154" in the same minutes as the Deals; `raider_82950` Deal at spot (266,712) four days running.
- **V5 war:** `gang_leader_68028 d23 02:00/03:00` war lines, `d24 00:05` lie low on exhaustion (3 of 14); `gang_member_6731 d22 00:00` corner fight and flip, "6 against 5". Counters: 6311 provocations, 415 answers, 207 corner fights, 137 flips, 10 retaken in 30 days.
- **V3 body found:** `gang_member_6731 d24 02:55` "found the body of Dahlia Bartlett at Block#583; the law opened a case: no suspect".
- **V3 report:** `dealer_101293 d22 19:02` "Brenna Stroud reported Juniper Ostrowski for Dealing".
- **V6 patch-up:** `gang_member_6731 d22 14:29` "Treated ... in the back room of Back Room#2722 for 6c (1c of their own, Ninefold put up 5c)", the gang treasury pays.

Expected, never seen: a truce; a law crackdown on a cell; cells moving after a Hideout raid (one Raid result, `scav_98910 d27 01:26`, "Iron Widows held (2 raiders, 11 defenders)", no cell consequence printed); a tribute or protection line by cell; recruits from the turf; a street Preach in the cult diaries; a killer named on a lead (bug 2).

## 4. Bugs, ranked

1. **The mustered raid is walked off.** `cultist_62792 d21 18:32` "GoTo(MusterPoint) 199 min, 293 tiles", `21:51` Muster done, `22:00` "GoTo(RaidTarget) ... cut at street, 128 min, 220 tiles", `d22 00:09` Sleep (energy 0.03) and the raid is dropped. About 5.5 h of travel and no Brawl, no result line. V14's "muster never lands" is unchanged. Likely cause: muster point and target are not chosen near the member, and energy is not checked before the plan.
2. **A witnessed murder reads "no suspect".** `gang_member_6731 d24 02:54` saw Ulysses Draper commit Murder "and ran"; `02:55` the same man finds the body, "the law opened a case: no suspect". A gang member keeping quiet is a fine story, but the line should say he told nobody.
3. **Five Witness and five money lines for one stand.** `dealer_101293 d22 19:01-19:05`. V14's fold for BountyPaid never reached Dealing.
4. **Old and starving bodies in the corner fight.** `gang_member_6731` is homeless, malnutrition 0.17, toxicity 0.55, wallet 1c, and is one of the "6 against 5" that flips a cell. `dealer_101293` (61 y) starts a brawl with a 69 y. V14 bug 4 unchanged.
5. **Two voices.** Header "order Raid" with no Raid plan for days (`gang_leader_19636`, `purist_39862`). V14 bug 3 not rechecked in full; unchanged.

V14 bugs in this group: man dies mid-Extort **not seen** (no Extort+Death overlap); everyone extorts the same door **better** (14 Extort lines across the 16, no repeat door seen, checked by count only); two voices **unchanged**; old/starving bodies **unchanged**; scav bins loop **better** (no "nothing worth selling" line in either scav); neglect warning **not this group**; BountyPaid x3-4 **not seen** (0 here); Story headline repeat **not seen**.

## 5. Dull spots, with the mechanism

1. **A member hears nothing of the war.** Only the leader prints war lines. A member should be called to the corner, told "we lost the corner, 6 against 5", see the mark painted over, and a Hideout raid should name the cells that moved.
2. **No truce, no crackdown in a week of war.** 137 flips in 30 days, 5 Patrol/Crackdown transitions, none in 16 diaries. A 57-man crew against 14 should end with someone paying: a leader jailed, a tribute demanded, a surrender.
3. **Runners and scavs have no second act.** Runners have no corner or Deal lines; a runner should carry the corner's goods or a message between two crews, a scav should sell what the corner leaves.
4. **A raider that deals.** `raider_82950` 17 Deal, 0 Brawl, 0 Raid. Muster should pull a raider to the nearest contested cell before the Deal plan takes the day.
5. **Purist.** `purist_48604` preaches and is turned away from a Precinct; no creed event (a chromed fare refused, a dawn gate).

## 6. Too much

- **Spray lines**: 3 of 7 Turf lines in `gang_member_6731` are "saw X spraying" at 18:00-19:00 and 00:00 sharp. Reads as an hourly tick, not an event.
- **Witness lines per stand** (bug 3): noisy.
- Not broken: 70 gangs hold corners daily, 3.7 cells a gang, 207 corner fights in 30 days at 100k. No war every night in these diaries, no 60-man gang on three fronts (one leader, two fronts, 14 men).
