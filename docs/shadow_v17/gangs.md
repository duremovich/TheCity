# Shadow V17: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). 100k island, seed 42, d21-d27, main 389bd1a. Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v17/isle100k_s42_d21/`. Ids differ from V16 (picker re-ranked), so comparison is by archetype.

## 1. Verdict

- **Gang politics got a spine; gang people did not get a life.** Every leader change now prints a Succession line (31 in 16 diaries, V16 had almost none), cells are a real place (meal 07:00, yard 15:00, release "came out to the life he left"), and the sacked go looking for work. But 5 of 16 diaries are mostly a jail week (dealer x2, cultist_75764, runner_93438, scav_57400), and the gang mechanisms (raid, muster, war call, protection) fire as one-liners the member cannot act on.
- **Under-18 rule holds**: no Assault, Arrest, Fare, Cells or Witness line in these 16 prints an age under 18 (0 hits). Caveat: Turf lines name the dead with no age ("Bertram Nightingale dead", `gang_member_10413 d27 11:00`; "Imogen Ashcombe dead", `raider_91429 d26 04:00`), so I cannot check those from the diary.
- Against V16: level. Better on Succession, the cell's day and the sacked's search; worse on churn (leaders jailed every 2 days) and thinner on the war arc (one truce, `gang_leader_17198 d23 00:05`). The runner finally has one real life (`runner_93438`).

## 2. Per archetype (V16's word in brackets)

- **gang_member: thin (alive).** `_10413` (42 y, Glass Dogs): `d21 02:29` "Cordelia Vance pushed Ursula Draper aside and runs The Glass Dogs: the old hands follow her"; `d23 00:07` "chose the shift as Vat Tech over The Glass Dogs's court summons (19% the gang's way)"; `d27 11:00` hit at a corner while her gang loses it. A worker choosing the wage over the gang is good. `_6454` (50 y mechanic, Wire Hands): a 34c bodyguard contract fails `d26 00:09` "the client was hit"; `d25 23:59` Succession; otherwise a work loop.
- **gang_leader: alive (alive), one dead.** `_17198` (75 y, Ninefold): best arc: `d23 00:09` Unplugged 2 demand protection of 40 a week on his Gambling Den, he refuses; `d27 13:00` "Lena Nakamura smashed the till ... Yorick Northcott owes the week", GRUDGE, "Ninefold will answer". Then he walks 13 h (bug 2). `_63568` (70 y, Unplugged 9, 9 men, 0c): d24, d25, d26 DAY SUMMARY are identical to the minute (972m home, 468m street, idle 6.5h); a shakedown he pays and "tells nobody" `d27 13:14`, then he attacks the 43 y man `13:43` and loses. Dead loop.
- **purist: alive (alive).** `_32297` (43 y, Unplugged 3): `d21 21:45` "Block#127 would not pay Pixel Oakes, and fought back"; `d27 02:30` "Dagny Lockridge woke on the street to find 1c gone ... she means to tell the law", Enemy -1.00 edge; Extort again `d27 08:50` "gave nothing (nobody had a coin)". `_9284` (62 y Dollhouse host): the gang is a footnote, 18 Served (visit) lines, bug 3.
- **cultist: thin (thin).** `_35651` (32 y, Drowned Choir 9): the best jail arc: `d21 09:00` blackmailed 5c, `d21 23:48` arrested for Dealing, `d22 01:12` 5 days, `d27 01:12` "came out to the life she left: the Home at Block#692 still hers; The Drowned Choir 9 still hers", `d27 07:53` "welcomed back ... 5 days served, nothing said inside", re-arrested `d27 14:18` for Grand Theft 13 h later. No creed event in either diary. `_75764` (69 y): 12 Cells lines, a jailed week, three of its gang's leaders jailed in 4 days.
- **scav: thin (thin).** `_29091` (35 y Vat Tech, homeless, Grey Teeth): evicted `d21 00:01` (Story x3), court summons missed `d26 00:07`; no Scavenge find line. `_57400` (33 y Dreg): Arrest, Sentence, Cells x10, a prisoner from d22.
- **dealer: dead (thin).** `_58604` arrested `d21 00:56` for Assault on a neighbour at home, 10 days; `_105046` in the cells d21-d25, released into "the post as Broker at Back Room#2151 gone" (`d25 16:14`), reads job boards seven times. 0 deals in either diary; both picks are inmates.
- **runner: thin (dead).** `_93438` (68 y, Back Room owner): GuardTaken `d21 18:00`, Defrauded `d24 00:09`, bail paid by the gang `d25 00:05`, LedgerHacked 200c from Vatra `d25 02:09` (a runner at last), Dealing arrest `d25 13:17`. `_31923` (19 y Engineer): took a steal contract `d21 18:00`, "ICE fried" `d21 19:10`, turned away by the Red Teeth `d25 03:08` ("treasury could not feed another mouth"), Data Theft arrest `d27 14:48`. Both hackers, neither in a runner crew.
- **raider: thin (alive, one).** `_90567` (38 y Dreg, Grey Hands): food theft `d21 16:46` "miserable, mood -0.21", jailed, welcomed back `d22 21:11`, beaten on the El platform `d27 16:07` ("left on the platform 30 min", picked herself up `16:37`). `_91429` (24 y): holds a corner, sees a rival spraying `d24 01:00`, hit in a crossfire `d26 00:00`, 20 job-board reads. Order Retaliate, no Raid in the week.

## 3. Round 8's mechanisms seen at 100k

Fired:
- **X4 Succession on every change**: 31 lines, e.g. `purist_32297 d23 15:01`, `cultist_75764 d24 03:43`, `dealer_58604 d23 01:27`. Seen, but see bug 1.
- **X4 raid muster / sit out**: `purist_9284 d27 14:06` "sat out The Unplugged 8's raid: the muster at Hideout#2602 is 6 h 51 from home"; `runner_93438 d21 00:07` and `purist_9284 d21 00:07` "stays off the trade: the nearest spot ... is 5 h 14 / 7 h 05 from home".
- **X6 the cell's day**: meal 07:00 and yard 15:00 daily in every jailed diary (`cultist_35651 d22 07:00`, `runner_93438 d22 15:00` "walked the yard ... alone: 2695 in the cells, none he would walk with"); release line with the post, Home and gang kept or lost (`dealer_105046 d25 16:14`, `runner_93438 d25 00:05`).
- **X2 the sacked search**: `dealer_105046 d26 08:00` asked 9 people, "nobody knew of an opening", then boards.
- **X1 the promise to tell the law**: `purist_32297 d27 02:30` "she means to tell the law" and `gang_leader_63568 d27 13:14` "tells nobody". The first never gets its follow-up in the diary (told or never told); the week ends.
- **X4 lie-low line**: `raider_91429 d21 00:05` names the war it ends (Cinder Lanterns) and the truce is with the same gang.

Expected, never seen: a war call waking a sleeping member (call lines are in the leader's diary only), a Raid that lands (0 Raid events in 16 diaries), a `came to the cells` kin visit in any jailed gang diary, X5 pit/bout/courtship in this group, mourning/funeral/leave, a hit's target taking a guard (`runner_93438 d21 18:00` buys one but for another reason), a creed event.

## 4. Bugs, ranked by what the player sees first

1. **The man who comes out of the cells is not the man who went in.** `purist_32297 d23 15:01` "Annika Achebe runs ... while Alaric Fairweather sits inside"; `d24 11:43` "Smoke Wozniak is out of the cells and runs ... again (Annika Achebe stood in for him)". `purist_9284 d21 04:24` Juniper Sallow inside, `d24 00:05` Fenn Sato returns. `dealer_105046 d22 17:02` Zed Bellamy inside, `d26 05:57` Garrick Kowalczyk returns. Three of nine gangs: a silent leader change between the two lines. Likely cause: another leader took over and went in without a Succession line, or the "returns" line reads the current leader, not the jailed one. V16 bug 2 (leader change without a line): **better**, not gone.
2. **A 75 y leader walks 14-20 h a day.** `gang_leader_17198 d27 04:10-10:54`: Lead plan `GoTo(Seller) ... 275 min, 246 tiles` to his own den in his own district, then `GoTo ... 786 min, 735 tiles (run ended)` toward the Sump Hideout. DAY SUMMARY travel 14.8h d21, 19.6h d27, 74.5h for the week, work 1.1h. A Collect loop between D4 and D1 for a man with no cab fare. New.
3. **The gang's trade and muster points are unreachable for its members.** The Unplugged 8: `purist_9284` stays off the trade six days running (`d22 00:07` "the same on 5 more days: d23-d27", 7 h from home), `runner_93438` the same, and the raid muster is 6 h 51 away (`d27 14:06`). X4's "muster where members can reach" does not hold here; the member just declines politely. V16 bug 3 (raid never lands): **unchanged**.
4. **Gang sizes do not reconcile.** `gang_leader_17198`: war lines say Ninefold has 58 then 56 men (`d21 10:00`, `d22 22:00`), header says 24, and the lie-low `d23 00:05` says "3 of their 11 beaten, dead, jailed or gone" with a truce with The Hollow (a war not otherwise in the diary). `gang_member_10413`: Glass Dogs 16 (`d21 00:05`), header 30, "leads 46" (`d27 11:00`). `purist_9284` Unplugged 8: header 34, "leads 24" (`d21 03:00`), "leads 34" (`d24 11:00`). Counts that jump 2-3x in a week read as a broken roster. V16 bug 1 (lie-low names a different enemy): **better** (the right gang is named, the count is off).
5. **A held man seems to act on the street.** `dealer_58604 d23 01:27` "Hilde Wexford learned who put her inside: Cipher Sallow reported her for the Shakedown"; Cipher was sentenced `d21 02:30` and is in the cells all week. Possibly a report filed before arrest but dated d23. Uncertain.
6. **Old, starved bodies start fights.** `gang_leader_63568` (71 y, malnutrition 0.41, toxicity 0.68) attacks a 43 y man `d27 13:43` and loses; `gang_leader_17198` is 75 y. V16 bug 5: **unchanged**.
7. **Runner picks are hackers outside any crew.** `_93438` and `_31923` hack on contract and are jailed, neither is run by a gang. V16 bug 4: **better**.
8. V16 bug 6 (two voices, order vs goal): not seen in these 16, **not re-checked**.

## 5. Dull spots, with the mechanism

1. **The jailed week.** Dealer, cultist, runner, scav picks are inmates, 16 h a day "jail" (`dealer_105046` DAY SUMMARY d21-d24). Mechanism: the gang sends bail or a message, kin come to the cell (X6 built it; `came to the cells` never prints), a cellmate asks a favour.
2. **Gang members are told, never asked.** The leader calls 16 to the corner (`scav_29091 d24 05:40`); the scav is hauling at a vat farm and nothing happens. Mechanism: the call arrives as a summons the member answers, ignores (loses rank) or pays off, with a line either way.
3. **War as a template.** Every lie-low and truce reads "called X off the corners ... N of their M beaten". Mechanism: name the leader's loss (the jailed lieutenant) and let the rival ask a price.
4. **Purist and cultist creed is invisible.** No purist refuses chrome, no cultist keeps a rite, in 4 diaries. Mechanism: the creed vetoes a job or fare (a chromed customer turned away, a gate rite instead of a shift).
5. **Scav finds nothing, dealer deals nothing.** No find or Deal line. Mechanism: the bin is a corner the gang taxes; the stand follows the held corner.
6. **Raider cannot raid.** Order Retaliate, 20 board reads (`raider_91429`), zero raids. Mechanism: raid marchers picked by who is near the muster, not by gang size.

## 6. Too much

- **Leaders in and out of the cells every 2 days.** `purist_32297`: 4 Successions in 3 days (`d23 15:01`, `d24 11:43`, `d25 23:39`, `d26 00:05`); `cultist_75764`: three leaders jailed in 4 days (`d24 03:43`, `d26 02:46`, `d27 03:09`); `cultist_35651` three. A boss arrested every 40 hours reads as a revolving door. The Succession line is good; the rate is the noise.
- **Yard and meal lines**: two lines a day for 5-7 days (`cultist_35651`, `runner_93438`, `dealer_58604`: 16 Cells lines). Short, but every jailed diary now looks the same; "2695 in the cells" (`runner_93438 d22 15:00`) is a lot for one precinct.
- **JobSearch**: `raider_91429` 20 board reads, `dealer_105046` 7, all "nothing going" at the same three markets. A search that always fails reads as a loop; X3's record gate should say once "turned away on the record".
- Not broken: cell fights, 1c shakedowns, truces, Fare beatings of adults, the till-smash vendetta.
