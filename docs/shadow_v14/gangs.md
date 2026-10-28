# Shadow V14: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). 100k island, seed 42, d21-d27, main `f5893b7`. Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v14/isle100k_s42_d21/`. Ids differ from V13, so comparisons are by archetype. `target/r5_100k/analyze.txt` did not exist, so no 30-day counters.

## 1. Verdict

- **Better in the seams, still a day job with a badge.** The cross-life threads are stronger than V13's: `dealer_8674` is a full arc (bail, "changed life", beaten out of the cult, a beat posted, a forged licence, an auditor catch). Gang life itself (raids, musters, dealing) is mostly absent from the 16 lives.
- **No raid in the group.** 0 Muster, 0 Raid result, 0 raid Brawl in 16 diaries (V13 had one accident raid and one 368-min muster). The order reads "Raid" on two headers; nobody acts on it. Extortion at one doorstep (`Block#478`) is the gang's whole street presence.
- **U8 left the choices reading as V13's.** `GOAL GangWork` per archetype (V14 vs V13, same eight archetypes): gang_member 9 vs 7, leader 11 vs 7, purist 2 vs 4, cultist 9 vs 10, scav 9 vs 7, dealer 6 vs 3, runner 0 vs 4, raider 8 vs 3. Same order of magnitude, same plan shape (`GoTo(TargetHome) > Extort->Block#`), slightly more Extort plans (30 vs 25). Nothing reads wrong.
- **Under-18 rule holds.** A grep of every harm line (Assault, Fare, Brawl, Theft, Death, Extort) across the 16 for victims or actors aged 0-17 returned zero hits. Victims are 28-66 y. The Custody at `gang_leader_70166 d24 00:10` takes children (2-15 y) to the service, the sanctioned path.

## 2. Per archetype (V13's word in brackets)

- **gang_member: thin (thin).** `_14193` (62 y, homeless, Glass Teeth): `d21 02:32` evicted from a squat, `d24 21:27` "is starving", `21:30` sells a sighting for 15, `d26 05:18` and `05:35` assaults a 51 y and a 40 y, `23:23` patched for 6c; day-off crew lines down to 32 (V13 73). `_6572` (44 y, Corp, Unplugged 2): fare beating `d21 14:13`, then `d22 14:45` dies of Violence mid-Extort (bug 1).
- **gang_leader: alive, one (alive, one).** `_70166` (24 y, Unplugged): `d21 00:06` three corps post locate+tag on him, `d21 18:00` three takers, `d25 22:09` "Tagged ... (tier 1)" and ContractFulfilled (the tag resolves; V13 dull spot 5 landed), `d26 06:40` "runs The Unplugged while Zelda Santoro sits inside", `d26 21:00` "court failed: nobody came", `d27 00:07` named in the cells for Extortion. `_17694` (57 y, Pale Brothers, 2 members, treasury 0c): a starving homeless "leader", thefts, beatings, `d24 21:01` gives the Harvest order at court. Harsh and true, thin gang.
- **purist: thin (alive, one).** `_14424` (27 y, Cab Driver): drives 1 fare a shift for 3c, `d25 16:00` "stood by all shift and drove no fare"; "stays off the trade ... 21 h 00 from home" for 7 days; assault `d26 03:39` (63 y victim). `_57767` (19 y): `d22 20:22` extorts 3c, `d23 00:05` "left The Unplugged 2 (walked away)". V13's purist arc (dawn gates, hire, stolen bike) is not repeated.
- **cultist: thin (thin).** `_25391` (61 y Clerk) `d22 18:50` hears a sermon, `d23 05:58` "would not pay ... and fought back"; `_48395` (51 y) `d21 01:19` extorts 5c "(now The Drowned Choir 8 territory)", two GrudgeFormed (robbery 0.60). Sermons fire (2 across the pair), no street preach.
- **scav: thin (thin).** `_46032` (Vat Tech, night shift): "picked over the bins ... nothing worth selling" 4x, `d24 00:07` named in the cells, `13:04` resists arrest against a 20 y guard, `14:50` 6 days, succession to Ghost Carrow. `_95578` (49 y): thefts, `d24 16:01` arrested, `d27 22:08` welcomed back "3 days served, nothing said inside".
- **dealer: alive, one (thin).** `_8674` (21 y): `d21 01:32` LedgerHacked 2250 from Militech, `14:56` arrested for Dealing, `d22 00:05` "The Drowned Choir 6 paid 25c bail" and "came out to a changed life: the post as Jailer gone", `00:11` rehired as Jailer, `d24 00:07` quits, "lost the colours: 3 duties missed, beaten out by Ines Jarrow", `00:09` the gang posts a beat for 366 (revenge), `d25 11:44` forged weapons licence at Fixer#5801 (U5), `d26 00:00` incorporates Achebe Holdings, `d27 04:54` auditors match the hack, "Data Theft". Best life in the group. `_36558` (59 y): three corps post on him, six takers, all wait while he serves 6 days (T3 intact).
- **runner: thin (alive).** `_45111` (74 y): 2 JackedIn in a week, no ledger. `_89627` (56 y Vat Tech): `d26 05:36` deck to T2 for 42c, `06:37` jacks in, `07:27` takes 600 from Nutrix's ledger, then silence. V13's chain (hack, audit match, arrest) was richer.
- **raider: dead (thin).** `_23217` (31 y): theft, a bar brawl with the foreman `d23 00:00`, three skipped shifts then `d26 17:00` fired, a 30-min El beating `d24 05:39`, 14-day El ban. Nothing raids. `_43968` (53 y Bartender): the same bins line at 01:33 for 7 days, "nothing worth selling" each time.

## 3. Round 5's mechanisms seen at 100k

Fired:
- **U5 forged licence:** `dealer_8674 d25 11:44` "Delia Chrome handed ... a forged weapons licence at Fixer#5801, 40c paid, 8c held for the owner". The fixer's book names (`gang_leader_70166 d21 00:06`, `scav_46032 d24 00:06`, `dealer_36558 d21 00:06` "on X's book") read as designed.
- **U7 contradictions:** no "robbed" on a thief; `dealer_36558` (jailed `d21 18:42` to `d27`) carries 5 Witness lines, none inside the term; one succession line per jailing.
- **T3 pursuit wait** (carried): `dealer_36558 d21 20:19-d22 12:01` six takers hold and wait.
- **U2 family line:** `scav_46032 d21-d27 09:30` prints ages and "on the service's warning since d14", then "no School today" on d26-27.

Expected, never seen: a U2 take or second warning for `scav_46032`'s children (9 y, 5 y, 1 y; warning "since d14" is still "since d14" on d27); a raid or muster at all; a street Preach; a Deal with a named buyer (`dealer_36558` and `gang_leader_17694` only witness others dealing, `d23 15:41`, `d24 17:44`); a U6 pimp or clinic line (not this group).

## 4. Bugs, ranked

1. **A man dies mid-Extort and then collects.** `gang_member_6572 d22 14:24` "ACTION Extort ... (agent died)", `14:45` "Extortion: ... extorted 3 coins from Block#478" and "Death: died of Violence" in the same minute, then "social move Intimidate ... SUCCESS" after the death. Gossip says "Killed by someone unnamed" and also "Assaulted by Jasper Reyes". A stripped corpse (12c) and implant harvest follow. The extortion should not settle for a dead agent and the killer should be named.
2. **Everyone extorts the same door.** `Block#478` is extorted for 3c by `gang_member_6572 d22 14:45`, `purist_57767 d22 20:22` and `gang_leader_70166 d23 04:09`, three different gangs in 15 hours. Reads as one bank, not a district.
3. **Two voices: unchanged.** `gang_leader_70166` header LEADER while "runs The Unplugged while Zelda Santoro sits inside" (`d26 06:40`) and Zelda preaches `d25 19:50`. `gang_leader_17694` is LEADER of 2 members at 0c.
4. **Old and starving bodies on gang duty.** `gang_member_14193` (62 y, starving `d24 21:27`) still takes a "Raid" order and assaults two adults `d26 05:18`, `05:35`; `runner_45111` is 74 y. Better than V13's 70 y muster, but the order does not look at the body.
5. **Scav bins loop: unchanged.** `raider_43968` and `scav_46032` repeat "picked over the bins ... nothing worth selling" at the same minute (`01:33` x7). V13: 14 lines, none sold.
6. **Neglect warning does not climb.** `scav_46032` "on the service's warning since d14" through d27, three children, no School on d26-27. U2's second-warning rule is not visible (may be a weekend skip).
7. **BountyPaid x3-4 identical lines.** `gang_leader_70166 d21 00:49` (and `d22 19:05`, 4 lines), `gang_member_6572 d22 09:03` x3 "sold a sighting of Corwin Coldwater for 15" in one minute, then one money line summing 45c. Fold them.
8. **A Story headline repeats.** `cultist_48395 d21 00:01`, `d22 00:01`, `d26 00:01` "Wire Duarte assaulted Rue Okafor" from one old assault, "paid by Habitat" then "paid by Militech 3".

V13 bugs in this group: sat-out-then-raids **not seen** (no raids); jailed man witnessing **fixed**; crew day-off line x120 **better** (32 in `gang_member_14193`, 10 in `scav_95578`, 7 elsewhere); mustered raid never lands **not seen** (nothing musters, which is worse as a story); two voices **unchanged**; self-contract "posted a guard on" **not seen**; rich runner idles **not seen**; children out of school **unchanged**; strong grudge never walks **not checked**.

## 5. Dull spots, with the mechanism

1. **No raid anywhere.** The order "Raid" needs a muster the player can read: a muster plan for members who can walk, a turned-back or won result line, a named loser. Today it is only a header.
2. **Doorstep extortion.** The payer remembers: a door that paid this week is off the list, or a block hires a guard or moves after the third visit. Today one door pays three gangs.
3. **Purist life.** `purist_14424` is a cabbie with one fare, 21 h from any spot. A creed event: turns away a chromed fare, a dawn-gate refusal (V13 had four).
4. **Runner second act.** `dealer_8674` shows it: hack, auditors, arrest. Give `runner_89627` a follow-up, not a single 600c hit and silence.
5. **Starving boss.** `gang_leader_17694` runs a gang of 2 on 0c; `dealer_36558 d21 00:07` shows a gang that "cannot pay a 3c duty". A broke gang should merge into a stronger one or be taken over.
6. **Scav find.** A find that sells, a cache, or a body.

## 6. Too much

- **BountyPaid x3-4 per sighting** (bug 7): a wall around every wanted man.
- **Cell days** are lighter than V13: `jail 16.0h` on 10 of 112 person-days (V13 26 of 96). Not broken.
- **Witness lines** (`gang_member_14193` 45, `gang_leader_17694` 45) are mostly folded "N saw ... commit Fare Evasion". Noisy, not broken.
- Nothing reads as a broken city. The harshness is accepted; the dull is the raid that never happens.
