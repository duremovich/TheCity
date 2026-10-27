# Shadow V11: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). Main `f0b7e44`, 100k island, seed 42, d21-d27. Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v11/isle100k_s42_d21/`. Counts are greps over the 16 diaries. d27 is a rest day for job holders.

## 1. Verdict

- **Better than V10: one raid finally lands and has a tail.** `purist_56353` musters 142 min, marches to the Precinct, brawls, flees home, is gossiped about all night, arrested at 06:09 and sentenced to 10 days. That is a gang story from start to consequence. Fare Evasion jail loops are gone, GangLeave fell from 4 to 1, Scavenge no longer dies on its first tick.
- **Still a day job with a badge for 12 of 16.** Gang life in the diary is a Crew line at midnight, a missed muster, a day off with the crew. Collect, Court, finds and Deal are 1-minute no-ops or pay nothing visible.
- **The walk is still the day for anyone whose Hideout is not their district.** Leaders and dealers cross the island (500-650 tiles) for a 1-min Collect/Court/PickUp and sleep on the street at the door.

## 2. Per archetype (V10 word in brackets)

- **gang_member: thin (thin).** `_6668` is a Bartender 8.9 h a day in a 61-member gang (order Job) with zero Crew lines in 7 days; the only gang-adjacent line is `d27 00:08` "took up Church of the Open Circuit". `_13625` (Purist, 59 members): `d23 21:18` drunk, turned away from Gambling Den#4981, "swung at Casimir Bellamy ... and put Casimir Bellamy down", four witnesses; `d25 00:05` GangLeave "walked away"; `d25 18:20` Assault on her. A real arc, but the gang never shows.
- **gang_leader: thin (alive, one).** `_96308` (Grey Ghosts, 16): `d24 19:08` Raid ordered, muster called, `GoTo(MusterPoint)` 330 min, then Sleep; the next muster is a 630 min walk (`d25 19:38`); `d26 00:07` and `d27 00:07` "missed the muster". He is the leader and misses his own muster. `_18246` (Cinder Hands, treasury 0c) is a Vat Tech; his gang is one `d27 20:18-22:19` day-off-at-Hideout#4542 burst and 5 Scavenge rounds ("nothing worth selling", `d27 15:18`).
- **purist: alive, one (thin).** `_56353`: `d21 18:02` Raid, `d21 19:38` Muster 142 min at Chapel#4914, `d21 23:39` Brawl 27 min, `d22 00:06` 10 Assaults at the Precinct, `d22 00:09` ContractPosted "Lux Park posted a guard on Lux Park for 41", `d22 06:09` arrested, `d22 07:38` "10 days for Assault". `_13450` (BD Actor, Unplugged 4): shakedown of Block#469 twice (`d22 11:32`, `d23 01:16`), reported by a witness, arrested `d23 04:07`, 6 days, jail to d27.
- **cultist: thin (thin).** `_31015` has 5 Preach plans and 0 Preach actions (every `GoTo(Spot)` is cut: `d22 01:21` 64 min, `d27 12:50` 395 min); only grudges, a marriage, a death in view. `_89446` (jobless, Choir 8): Raid plan at `d21 00:00` cut by Sleep in 12 min; nothing else.
- **scav: thin (thin).** `_58013` (Sump, jailed d21-d24, 3 days for Theft) scavenges 12 rounds of 60 min and logs no Served line; `_56266` is a Manager (18c) and gang watch-keeper who hacks Vatra's ledger three times for 600c (`d22 11:24`, `d24 14:56`) and picks bins for 3c once (`d23 01:53`). The scav trade is a hobby of a hacker.
- **dealer: thin (thin).** `_22253` (Sanitation): 5 Deal plans, 2 Deals, no buyer, no coin, no product; `d21 18:06` the plan is cut for Loot, `d21 18:43` Strip is a 1-min PreconditionLost; `d27 12:23` GoTo(Hideout) 274 min, 539 tiles, PickUp `d27 16:57`, Deal `d27 23:19`. `_74815` is the best snitch story: `d26 00:37` released starving, `d26 08:06` "welcomed back ... he leads now", `d27 00:07` "Rin Crane talked in the cells: named Maren Zhou", `d27 01:23` arrested, 6 days, `d27 03:27` Succession "Hex Sato runs The Grey Ghosts".
- **runner: thin (alive/thin).** `_78458` jacks in, lifts 10 Chrome Data (`d21 07:56`), sells it for 20 (`d21 18:31`), joins The Hollow (`d23 02:27`); then nothing. `_43612` is a Sanitation worker with 2 JackIn and no data event.
- **raider: dead (thin).** `_39544` musters 120 min at Chapel#4914 and is then marked "missed" (bug 3); `_58062` "ignores BreakOut order and works alone" three times, shakes down Block#469 (refused `d23 04:14`), is arrested `d23 05:57`, 6 days.

## 3. Round 2's mechanisms seen at 100k

Fired, quoted:
- **R5 muster held, target bounded, raid lands:** `purist_56353 d21 18:02-23:39` above (muster walk 96 min, Muster 142 min, 99 min march, Brawl). Sleep did not win until after the raid (`d22 00:08`).
- **R5 try-out turned away / posted watch:** `d25 17:08` "tried out for The Salt Rats at Hideout#4559 and was turned away: the treasury could not feed another mouth", `d25 08:28` "they were full up" (The Ash Crows); `scav_56266 d21 19:38` "stood watch ... (paid 3c)", but `d22 20:11` still "(unpaid: the gang is broke)".
- **R5 court missed at work excused:** `scav_56266 d24 00:07` "missed ... court summons ... (at work: excused)".
- **R5 Scavenge starts:** zero Scavenge `PreconditionLost` in 16 diaries (V10: 52).
- **R1 fares are a ladder:** 5 `Fare:` events in the group, no Fare Evasion jail (V10: 8 arrests in two lives). `gang_leader_96308 d21 14:43` "beat ... for Fare Evasion: no fare, no fine, left on the platform 30 min".
- **R2 walk:** 1 min/tile on short legs (`gang_member_13625 d21 09:07` 82 min, 59 tiles); see bug 2 for the long ones.
- **R4 witnesses/fold, R6b gun trade/hunts:** not visible in this group (the witness fold did not reach crowds, bug 4).

Expected, never seen:
- A Raid result line (no "Raid on ... won/lost" anywhere; the raid shows only as assaults and an arrest).
- A Collected line (`gang_leader_96308 d21 08:42` Collect is a 1-min PreconditionLost).
- A Preach that reaches an audience on the street (`raider_39544 d27 14:31` is the only Preach action, 31 min). Service at the Hideout reached `_39544` once, `d25 20:00`.
- The broke-gang stand-down line ("stood its watch down"), a Protected, gear or loan event.

## 4. Bugs, ranked by what the player sees first

1. **A Collect and a Court are 1-minute no-ops after an 8-11 h walk.** `gang_leader_96308 d21 00:00` GoTo(Seller) 522 min, 491 tiles to Hideout#4544 (D1); `d21 08:42` Collect "1 min (ended)", `08:43` "PLAN FAILED: Lead failed at Collect: PreconditionLost". `d26 15:52` GoTo 212 min, `d26 19:24` Court 1 min, `19:25` PreconditionLost. He then sleeps on the street at the door (`d21 09:14` 241 min, `d26 19:56` 793 min). V10 bugs 6 and 8: unchanged.
2. **The walk is still the day for a far Hideout, Home or errand.** `gang_leader_96308` travel 17.8 h (d21), 14.3 h (d25), 14.2 h (d26): Home D4, Hideout D1, post D6; `d21 17:02` GoTo(Home) 653 min, 621 tiles. Mid-Levels workers with a 56-min commute still travel 5-12 h (`gang_member_13625` d21 11.7 h; `d21 00:00` GoTo(Beat) 394 min, 486 tiles; `d22 01:39` GoTo(Spot) 296 min). V10 bug 3: better for the median, unchanged for leaders and any Preach, Beat or Spot errand.
3. **A mustered raider is marked "missed".** `raider_39544 d21 20:00` Muster 120 min at Chapel#4914 (done), march `d21 22:00` "cut at street ... 154 min (ended)", then `d22 00:07` "missed The Unplugged's muster at Hideout#4914 (2 in 10 days)". `purist_56353`, same muster, is not marked. The attendance flag probably does not survive the march being cut. The muster is `Chapel#4914` but the Crew line says `Hideout#4914` (V10 bug 12, cosmetic).
4. **The raid is a wall of noise and has no result line.** `purist_56353 d22 00:06`: 260 `Witness: ... saw Lux Park commit Assault` lines, 10 Assault events, "... and 256 more relationship changes in the same minute"; no "Raid on X: won/lost". R4's fold did not reach witnesses.
5. **Fare Evasion witness loop.** `gang_leader_18246` sees the same Matthias Moreau commit Fare Evasion at `18:43` on d22, d23, d24, d25, d26. No Fare event follows. A nightly line, not a story.
6. **A leader walks 330-630 min to his own muster and misses it.** `gang_leader_96308 d24 19:08` (330 min, cut), `d25 19:38` (630 min, cut), "missed" d26 and d27. The muster point is in the Hideout 500-600 tiles away.
7. **Pavement sleep 4-13 h at the Hideout door** (`gang_leader_96308 d26 19:56` 793 min on a D1 street). V10 bug 5: unchanged.
8. **Preach spots are walks that never end in a sermon.** `cultist_31015`: 5 plans, 0 actions (`d22 01:21`, `d23 00:46`, `d24 20:03`, `d26 00:07`, `d27 12:50`, each `GoTo(Spot)` cut). `gang_member_13625` 3 plans, 0 actions.
9. **Self-contract:** `purist_56353 d22 00:09` "Lux Park posted a guard on Lux Park for 41" and `d22 18:00` "Juniper Bartlett is on Lux Park's payroll". Could be a guard against reprisal (fine) or a mis-named buyer; her wallet was 22c and the guard cost 41.
10. **ExpansionCap** kills: 2 (`gang_member_13625`, `cultist_89446`), V10 8.

V10 bugs in this group: 1 Scavenge no-op: **better** (starts, no PreconditionLost; still ~1 find in ~45 rounds). 2 Raid cut by Sleep: **better** (one lands; two cut by Sleep, `raider_39544 d22 00:34`, `cultist_89446 d21 00:12`). 3 walk: **better/unchanged** (bug 2). 4 Fare jail loop: **fixed**. 5 pavement sleep: **unchanged**. 6 Court statue: **unchanged**. 7 Zero ReportCrime: **better** (Report events in `purist_13450 d23 01:16`, `purist_56353 d22 00:06`, `raider_58062 d22 05:41`, `dealer_74815 d27 00:07`). 8 No Collected: **unchanged**. 9 Church hopping: **fixed** (one conversion in 16 lives). 10 ExpansionCap: **better**. 11 Friend edges +1.00: **unchanged** (`gang_leader_96308` has five at +0.96 to +1.00, not traced). 12 Hideout/Chapel id: **unchanged**.

## 5. Dull spots (mechanism, not a knob)

1. **The raid result.** A raid logs a result (winner, stolen stock, wounded) to the victim venue and both gangs; the loot lands in the treasury and the rival hears of it. `purist_56353`'s night would then be a headline.
2. **A Hideout near its members.** Leaders and dealers cross the island for a 1-min Collect. The gang moves its Hideout to the district where its leader and most members live and work, or a lieutenant who lives near collects.
3. **A scav find that sells.** A bin has stock that regrows, a find is an item (food, parts, a chip) the gang buys at tithe price; "nothing worth selling" becomes a regrow wait, not a dead end.
4. **Deals leave a trace.** A Deal moves product for coin to a named buyer who then drinks at a Club, and a cellmate snitch (`dealer_74815`) can name the buyer. The snitch story is already the best thing in the group; give it a source.
5. **The runner's contract.** `runner_78458` steals data, sells it, joins a gang, then stops. A Network yields a payer and a deadline.
6. **A creed with a pulpit.** Preach goes to the Hideout's chapel on the gang's night with the plate and the crowd; the street Spot is the fallback. Service already works (`raider_39544 d25 20:00`), Preach should feed it.
7. **A snitch's pay-off.** `dealer_74815` is named by a cellmate within 17 h of release and the leadership swaps (Succession). The new leader should call a court on the snitch; nothing follows today.

## 6. Too much

- **Witness lines on a brawl:** 260 lines in one minute (`purist_56353 d22 00:06`). Noise, not harsh. Fold to one "N saw X's Assault" line and keep the Report.
- **Nightly Fare Evasion witness at 18:43** (bug 5), five days running in `gang_leader_18246`.
- Gang life is quiet at 100k: 16 lives, one mustered raid landing, no deal paying, no raid result. Closer to "thin" than "too much"; nothing reads as a broken city.
