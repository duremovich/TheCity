# Shadow V19: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). 100k island, seed 42, d21-d27, main 41c31b2. Cites are `<arch>_<id> dDD HH:MM` under `target/v19/isle100k_s42_d21/`. Ids differ from V18, so comparison is by archetype.

## 1. Verdict

- **Better as a place, same as a week.** A gang life now has floors, lifts and landlords in it: a raid climbs to a floor-28 Hideout, a landlord hires a beating on a leader, a bouncer rides a lift to floor 45 and back all week. The vertical city reads.
- **But the cells still eat the week**: 11 of 16 picks spend time in the Precinct, 6 of them 4+ full days (V18: 8 of 16 for 5+); succession churn is up (1888 Successions in 30 days vs V18's 1377; 77 lines in these 16 diaries, 1-10 a diary). Zero Hunt/Avenge/Revenge goals again.
- Under-18 rule: no Assault, Arrest, Witness, Cells, Murder or raid line in the group prints an age under 18. The only children are the leader's own, at home (`gang_leader_113134` `[family]` lines). No minor harmed.

## 2. The exit's checks

**(a) A life within a walk in or beside one tower: yes, three.**
- `raider_24528` (23 y, Packer, Block#293 The Spire): home flat 1 on floor 14, post Packing Plant floor 0 of the same block (commute 0 tiles). `d25 08:10` HangOut at Block#291 floor 0 (2 min), `08:12` "CLIMB up by the lift in Block#293: floor 0 to 14, 1 min waiting in the lobby, 2 min in all", 61-min HangOuts on floor 14, `d25 18:51` eats at home, `d25 20:03` Rest at the plant, `21:03` ClerkWork. Her evening is two floors of one block plus the next block. The day summary says it: Block#293 floor 14 605m, Packing Plant floor 0 568m, Block#291 127m (`d25`).
- `runner_23489` (44 y, Engineer): "Home Block#311 flat 1 on floor 0, floors 0-1; Job Parts Fab floor 0 of Block#311; commute 0 tiles". `d22` 565m at the Fab, the rest at the chapel and home.
- `gang_member_107667` (63 y, Clerk): flat 4 floor 12 Block#986, market 16 tiles away, travel 0.4-1.6 h a day (`d22`, `d24`).

**(b) A gang holding floors over a front, and a raid that climbs and lands: the raid yes; "over a front" not seen.**
- `cultist_112804 d25 22:29`: "Raid: The Drowned Choir 33's raiders (15) went up the stairs of Block#1000 to the Unplugged 4 Hideout on floor 28 (the lift held by The Unplugged 4), 14 min up", `22:43` "Raid on the Unplugged 4 Hideout: The Drowned Choir 33 won (15 raiders, 1 defenders); took 106c and 0 food from the Hideout; 2 hurt: Knox Greaves, Taz Abernathy". The chain is in her diary: `19:14` GOAL Raid, `19:15` CLIMB down by the lift from floor 29, `19:24` Muster 156 min, `22:00` GoTo(RaidTarget) 14 min, `22:14` Brawl. A cultist (courage 0.09) in a 15-body raid that lands in 3.5 h of her evening is a story.
- Ground raids also land: `purist_59070 d22 23:06` "The Unplugged 13 sacked it (5 raiders, 1 defenders); 500c and 491 food taken; 1 hurt: Delia Greaves"; `scav_44416 d22 23:07` "walked in, nobody held it (7 raiders, 0 defenders)".
- The front: no diary prints what sits on the Hideout's ground floor, no "guard the seat" line, no "sleeps at his nearest" line. Hideouts appear only as "Hideout#6441 (D1 The Sump), floor 8 of Block#3202". So (b)'s "floors over a front" is not visible at the player level.
- **The landlord against a gang: seen, and the best new story in the group.** `gang_leader_113134` (Raven Ashcombe, Red Teeth): `d24 00:00` "Habitat 28 hired muscle through Fixer#11905: a beating for Raven Ashcombe ... who runs floor 8 of Block#3202 and will not pay the floor rent (3c owed; 419c on the board)" and "Habitat 5 sent the Precinct up to floor 42 of Block#573 ... 2 found up there and reported for extortion"; `d24 09:39` "Smoke Sato came to Fixer#11905 in person and took the beat"; `22:14` "attempt 1 failed (the crew is gone)"; `d25 09:50` Quill Park takes it again; `d26 00:23` "Hilde Haas (54 y) attacked Raven Ashcombe (48 y) at Block#65 ... lost (1 vs 2, 0 dead); the escrow back to the buyer"; `03:32` she is arrested for Smuggling at home, 8 days, `04:10` "Cass Mbeki runs The Red Teeth while Raven sits inside". Buyer, price, hitter, attempt, failure, retry, outcome: all there. (Bug 4, 5 below.)

**(c) A Hideout leg past 4 h: none.** Longest GoTo in the 16 diaries is 64 min (`gang_member_82779 d27 00:03`, 612 tiles, cut at the Sump). Hideout legs are 12-43 min (`scav_44416 d21 03:43` 12 min; `gang_leader_113134 d23 20:09` 41 min, 698 tiles). Muster waits are long (156 min), walks are not.

**(d) Any minor harmed: none.** All guard/arrest/assault/witness/murder lines name ages and all are 19+ (guard Hesper Carrow 19 y, `gang_leader_113134 d26 03:21`). Raid "hurt" lists still print no ages (V18 bug 4, unchanged), so a raid's minors cannot be checked from them; the 2 hurt at `cultist_112804 d25 22:43` and 1 at `purist_59070 d22 23:06` are unverifiable.

## 3. The vertical city at the player level

- **Reads like a place:** floor numbers are varied (flats 1-8, floors 0-45), `CLIMB` lines name the floor and the lift/stairs. 48 CLIMB lines in `gang_member_82779` for floor 45 of Block#427 (floors 0-47), each "1 min waiting in the lobby ... 4 min in all"; stairs for short hops (`gang_leader_113134`, Block#65 floors 0-2, 1 min). A flat on the 30th floor (`cultist_18786`, flat 1 floor 30, Night Stocker in the same block floor 0, 1-tile commute) and a shop on the ground read correctly. The tallest buildings check out: floor 45 in "floors 0-47", floor 35 in a chapel of Block#1299.
- **No rush hour, no queue.** Over all CLIMB lines in the group the lobby wait is exactly 1 min (about 270 of 280); "2 called the car" shows only 8 times, "5 called the car" once. A 45th-floor commuter at 08:57 waits the same as at 04:12 (`gang_member_82779 d21`). The lift is a fixed 2-4 min trip, not a place with a queue. Flat.
- **Stairs to floor 8 (4 min)** are plausible; nobody takes stairs past 8 except raiders (28 floors in 14 min is generous, but it is a gang).
- **A climb does not follow a harm:** `gang_leader_113134 d26 00:23` a crew "hit Raven at Block#65" (flat floor 2, 36-floor block, no CLIMB or lift line for the attackers) and she sleeps on ("Sleep ... floor 1, 208 min (ended)"), though every other night she sleeps on floor 2 (`d22 00:04` floor 2). Where is she, and who woke her? The diary shows no wake, no Flee, no guard.
- **Nothing absurd found:** no body on a floor above its building, no climb past 4 min, no "flat 1" everywhere.

## 4. Per archetype (V18's word in brackets)

- **gang_member: alive (alive).** `_107667` (63 y Clerk, Glass Hounds): a working week, floor 12 flat, market 16 tiles away; `d26` Hideout#139062 floor 28 for 32 min; `d27` -50c net. `_82779` (57 y Bouncer, floor 45): the lift life above, `d25` "idle 13.0h".
- **gang_leader: alive (thin).** `_113134`: the landlord story above; she also keeps an office job (8.6 h at Corp HQ), commutes 3 min, and rides 40 min to the Sump to hold court (11 min, `d23 20:50`) and sleep. `_48789`: jailed `d22`-`d25` (4 days), out `d26`, Club.
- **purist: thin (alive).** `_59070` (39 y, Unplugged 13 LEADER, floor 5 flat): ordered the raid `d21 20:08`, led it `d22 20:21`-`23:06`, resisted arrest `d22 09:35`, Shakedown 8 days from `d23 01:41`, five identical jail days. `_71493`: 3.6 h of cell on `d23`, then four identical days.
- **cultist: alive (alive).** `_112804` (57 y): the climbing raid, a Preach on `d24` ("The Drowned Choir#8259" 534m); `_18786` (41 y): jailed `d21`-`d22`, dies `d26 07:00` in a turf fight (see stories); `d27` "no activity". Preach only 2 lines in the group (V18: nightly).
- **scav: thin (thin).** `_44416` (41 y, Feed Manager): a manager who sleeps at a Sump Hideout and raids a market `d22 23:07`; `_68640` (70 y, Recycler Tech): Scavenge "nothing worth selling" every time (32 lines, 31 Scavenge actions in the group, no find). Archetype name does not match the life.
- **dealer: thin (alive).** `_114471` (18 y, no job): a Freight Yard/Street Market worker, `d21 19:06` a 60-min GoTo(StimSource); `_60920`: cells `d21`-`d25`. Only 4 Deal actions in the group.
- **runner: dead (alive).** `_23489` (44 y Engineer, hacking 0.83): 0 Hack actions; jailed `d24`-`d27`; `_78478` (72 y): jailed `d22`-`d27`. Both Runners are mostly cells. No runner raids, hacks or runs.
- **raider: alive (alive).** `_24528` the tower life above; resisted arrest vs guards Ursula Kemp (45 y) and Kasimir Park (38 y) (Dealing), 7.5 h jail `d22`, then free. `_54538`: Iron Lanterns, jailed `d21`-`d27` (6 days) after a Murder-cell yard visit (`d23 15:00` yard with Seraphine Kimura, in for Murder).

## 5. Bugs, ranked by what the player sees first

1. **The cells swallow the week: unchanged / slightly better.** 6 of 16 are inside 4+ full days: `purist_59070`, `dealer_60920` (`d21`-`d25`), `runner_23489`, `runner_78478`, `raider_54538`, `purist_71493`; 11 of 16 see the Precinct. Identical "sleep 8.0h, jail 16.0h" summaries recur. V18 bug 1: unchanged.
2. **Succession churn: worse.** 1888 in 30 days (V18 1377); a spell of 10 in 7 days for `raider_54538`'s Iron Lanterns, 8 for `cultist_112804`, 7 for `gang_member_107667`. Leaders swap every day ("runs while X sits inside" nested, `gang_leader_113134 d27 00:46`: "Percival Northcott runs The Red Teeth while Cass Mbeki sits inside, as Raven Ashcombe has since before her"). V18 bug 2: worse. Header lists LEADER Raven; `d26 04:10` she is not.
3. **Hunt / Avenge / Revenge never fire: unchanged.** 0 in 16 diaries; 53 GRUDGE lines, none acted on. `cultist_18786` dies and her gossip spreads (7 tellings by `d26 08:03`) with no friend's response in this group. V18 bug (no pursuit): unchanged.
4. **A hit with a mismatched price.** 419c escrow for 3c owed (`gang_leader_113134 d24 00:00`). A 419c beating over 3c of floor rent is the landlord mechanism's economics wrong by two orders. The 6c owed on floor 42 gets the Precinct. The sleeping target does not wake or react (see section 3).
5. **A goal that never resolves: Bury.** `scav_68640` (70 y): "GOAL Bury (was GangWork): score 0.814" every night `d21 01:36`, `d22 00:28`, `d23 03:14`, `d24 03:57`, `d25 01:11`, `d26 00:30`, `d27 01:00`, each followed by "Bury unplannable (no feasible action)" and "cooled until ..., backed off, 10 in a row" (`d26 00:30`). A corpse nobody can bury; the player sees a dead goal repeated. New this pass (not in V18).
6. **Raid "hurt" lists carry no ages.** `cultist_112804 d25 22:43`, `purist_59070 d22 23:06`. V18 bug 4: unchanged.
7. **The raid's muster waits 156 min on the street** (`cultist_112804 d25 19:24`, courage 0.09, energy 0.32 then 0.32 at 22:00) before a 14-min walk. V18 bug (159-min muster): unchanged, though no order flip this time.
8. **Lifts have no queue** (section 3): the exit asked for "a lift wait at the rush"; every wait is 1 min.
9. **Camera JUMP noise: unchanged.** 126 JUMP lines in the group. V18's "allowance 2" noise is still printed.
10. **Archetype mismatch:** a "scav" with an 18c/day Manager job (`scav_44416`), a "runner" who never runs (`runner_23489`, `runner_78478`), a "dealer" who works at a market (`dealer_114471`).

V18 bugs in this group: 1 cells **unchanged**; 2 succession **worse**; 3 starving bin retry **not re-checked** (no starving pick here); 4 raid called from sleep dying on an order change **better** (raids landed twice, 22:43/23:06/23:07); 5 mark with no consequence **better** (the Raven beating attempt has an outcome and a refund); 6 neglect line **not seen** (no neglect lines in these 16); 7 quiet feeding **not seen**; 8 gang sizes **unchanged** (`gang_member_82779` header 47 members, `cultist_112804` 48; war line "Rafe Quarry leads 29 of The Drowned Choir 33" `d25 22:43` vs 48; "Osric Chrome leads 28 of the Unplugged 4"); 9 household count 9 to 83 **better** (`gang_leader_113134` reads steady, "34 households under the roof" for six days).

## 6. Dull spots and too much

- **Dull:** Scavenge never finds anything; Court at 38 goals; a 0.83 hacker with zero hacks; `raider_24528`'s ClerkWork broken into 30-70 min bursts each preceded by a 2-min "GoTo(Station) 0 tiles"; 61-min HangOuts repeated on floor 14; a gang leader who rides 40 min to court for 11 min.
- **Too much:** five or six identical jail days (`purist_59070`, `runner_78478`); gossip lines "(already known / dropped)"; JUMP warnings; PreconditionLost 155 times in 16 diaries.
- **Not broken:** the Raid chain (order, goal, climb, muster, brawl, result); the landlord hire/take/fail/retry chain; a turf fight that kills a corner's cultist, with a witness, a corpse found, the body stripped (9 coins) and buried in 100 min (`cultist_18786 d26 07:00`-`08:38`).
