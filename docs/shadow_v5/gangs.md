# Shadow V5: the gangs (critic report)

Window key: A = k2_s43_d26 (2k, d26-32), B = k2_s44_d90 (2k, d90-96), C = k20_s42_d20 (20k, d20-26). All Full LOD. 38 diaries: 6 each of gang_member, gang_leader, purist, cultist, scav, runner; 2 dealers (C only).
Headline vs V4: picks with 2+ days (48 h) in jail fell from 8/12 and 7/14 at 2k to 3/12 (A) and 4/14 (B); 20k is 4/14 (was 3/14). Leaders: 1 of 7 long-jailed (V4: 7 of 8). The cell is no longer the 2k story.

## Per archetype

**gang_member** (A 935, 1143; B 622, 826; C 11364, 13036). Changed: the civic round reaches them (B 622 is a Super, SuperRound x5 d91-96), but no gang_member has a street counterpart: 0 pimp / StreetCut / Served lines in all six. Alive as a worker who happens to be in a gang, thin as a gang member. C 11364 does almost nothing (counts: HuntAbandoned x1, Witness x2). B 826 spends 62 h in a cell for one Extortion.
> k2_s44_d90/gang_member_622.md d91 12:45: SuperRound: Casimir Park walked the round of 6 Vatra Blocks in Mid West

**gang_leader** (A 1285, 1517; B 1757, 725; C 11598, 24306). Changed since V4: the leader's cut exists and is visible at 2k day 90, and leaders no longer do their own street crime (0 raids, 0 marches, 0 shakedowns by any of the six). Fates: Tess one Assault arrest (A d31 21:35), Dorian 10 h for Fare Evasion, Beatrix a 5-coin fine (B d96), the rest untouched. Leaders have jobs (Tess an Orderly, Beatrix a Bartender). Alive as an owner, thin as a boss: the job is one weekly 11-min Collect plus a +4c daily stipend; no lieutenant is seen doing anything. Liesl (B 1757) holds 2890c, idles 92.7 h and sits at the Chapel 4840 min.
> k2_s44_d90/gang_leader_1757.md d90 08:08: Collected: Liesl Volkov collected for The Unplugged: 64 to 25 members (the week's tribute and take 196; off the top 18 to Liesl Volkov, 9 to 4 lieutenants)

**purist** (A 801, 2230; B 1985, 3675; C 10228, 20687). Changed: they preach now (V4: 0 Preach). 801 preaches 13:04 on d26, d27, d28 and converts two people. Still thin elsewhere: B 1985 is 126 h inside, C 20687 144 h. No chrome anywhere (6/6 Kits chrome false, 0 Installed), but see bug 4. B 3675 is the best: a Cook, 5 Extortions, a Coerced report withdrawal (d94 00:00), 8 Story lines.
> k2_s43_d26/purist_801.md d26 13:35: Preached: Delia Bellamy preached to Lena Garland

**cultist** (A 1359, 2381; B 1214, 1322; C 13277, 22630). V4 bug 13 (0 Preach) is fixed: 4, 6 and 5 Preach actions in C 22630, B 1214, B 1322, and a Persuade conversion (A d27 14:38, p=0.90). Succession works. A 1359 and 2381 still do 138 h and 144 h inside; C 13277 dies. Alive as recruiters; the tithe is still a ledger line.
> k2_s43_d26/cultist_2381.md d26 00:42: Succession: Jinx Zhou runs The Drowned Choir while Hesper Hallow sits inside

**scav** (A 896, 1063; B 2443, 2484; C 8477, 24034). Not alive as scavs, unchanged from V4. 0 Harvest/Strip actions in 5 of 6 (896 has one hit); `following gang order Harvest` appears and nothing follows. B 2484 is a Spire Office Worker who does 144 h for one Extortion; C 8477 preaches, is Coerced and leaves the gang in one week. The only body strip in the set is by a stranger off a runner (k2_s44_d90/runner_1620.md d91 01:23, 19 coins).

**dealer** (C only: 9984, 10097). No dealer pick in A or B, so none at 2k on d26 or d90 (V4 had four at 2k Full). Not alive, no better than V4: no sale, regular or lookout line in either diary. 10097 (72 y, addiction 1.00) runs Deal for 241 min at a time at Club#5347 (d20, d22, d25) with no Sold event and no coins in, then his 8 stims are seized in jail for Fare Evasion. 9984 Deals once for 40 min, then 131.7 h in jail.
> k20_s42_d20/dealer_10097.md d26 10:16: [inv] inventory food 2->2 (stolen 0->0), stims 8->0, parts 0->0

**runner** (A 1808, 2152; B 1620, 2539; C 6305, 21349). Alive at 2k now (A has decks by d26; V4 had none). A 1808: JackedIn x4, Fried, Traced. B 2539: JackedIn x3, DataStolen, DataSold, LicenceSold, Avenged, the best double life in the group. B 1620 and C 6305 are Flatlined and dead within the week; C 21349 one jack-in and 129 h in jail. ICE kills are real, but 2 of 6 runners dead and 2 caged is a hard week.

## Bugs

1. **Leader Collect pays "0 to 0 members" in the young city.** A gang_leader_1285 d27 05:39, gang_leader_1517 d27 08:08 (take 26), C gang_leader_11598 d20 00:22, gang_leader_24306 d20 08:29 (take 6): no "off the top" line, no cut, no lieutenant split. B d90 works (64 to 25 members, 18 to the leader, 9 to 4 lieutenants; gang_leader_725 d90 07:10: 27 to 24 members, 7 and 3). This is V4's "collects 0 to 0" unchanged for A and C. Distorts 4 of 6 leader diaries. Likely the Collect counts payers from a dues list empty until late, while the take comes from another ledger; the flat +4c daily stipend is all that is paid.
2. **No leading beyond the weekly Collect.** 0 of 38 diaries show a raid, a march, a lieutenant leading a raid or a leader delegating. After Collect the Lead goal (score 0.60) does not recur: Kira 59.9 h idle (A), Santiago 49.4 h (C), Liesl 92.7 h (B). V4's lone raids (Hana, Ursula, Bram Coldwater) are gone and nothing replaced them in the diaries. Distorts 5 of 6 leader diaries (Tess's job hides it).
3. **The dealer's corner is empty.** Deal for up to 241 min yields no sale, regular, lookout or Served line in either 20k dealer; summary says "regular at: none"; stock is seized on arrest. Distorts 2 of 2 dealer diaries (C only). Likely Deal finds no buyer and emits no event when it doesn't.
4. **A purist hacks.** A purist_2230 d27 05:07: Mateo Varley (The Unplugged, creed Purist) upgrades a deck to T2 at Security Office#425 for 38, jacks in d27 08:38 and d29 20:52, takes 200 from the city ledger d29 21:47. Chrome is settled; whether the Unplugged creed should cover decks is a design call. Distorts 1 of 6 purists.
5. **Clockwork preaching.** purist_801 Preach at 13:04 on d26, d27 and d28, each 31 min at street (Mid East). Minor; 1 diary.
6. **Scavs never harvest.** See scav entry. Distorts 6 of 6 scav diaries; carried from V4.
7. **Sentence length per take (weak).** B scav_2484 144 h for one Extortion, B purist_1985 126 h, A cultist_2381 144 h, B scav_2443 96 h. The 2k rate (7 of 26) now matches 20k (4 of 14), so parity looks settled; only the 6-day sentence for a tiny take remains.

## Gate items

- Leaders lead: lieutenants leading raids: bug (0 raids, 0 lieutenant actions in 38 diaries; only Succession, cultist_2381 d26 00:42).
- The leader's cut: settled at 2k d90 (gang_leader_1757 d90 08:08, gang_leader_725 d90 07:10), bug in A and C (bug 1).
- The leader delegating: bug (no delegation line in any leader diary; Succession on jailing is the only handoff).
- Leaders' fates over a week: settled (V4 7 of 8 jailed, downed or dead; V5 one arrest, one 10 h Fare Evasion, one fine, three untouched).
- Raid marchers: none seen (0 raid or march lines). Decision for Dylan: were raids meant to be delegated to lieutenants, or just removed for leaders?
- The dealer's corner (regulars, lookout, sales): bug (bug 3); dealer absent at 2k in A and B.
- gang_member met at d90: settled as a non-event. B 622 and 826 show no pimp or StreetCut line; StreetCut flows appear only at 20k to leaders (gang_leader_24306 d25-26 StreetCut +2). Decision for Dylan: should the pimp's round reach gang members at 2k by d90?
- Creeds: settled, cultists preach (22630, 1214, 1322; conversion A d27 14:38).
- Purists still buying chrome: settled, no (0 Installed, 6/6 chrome false); but a purist buys a deck and hacks (bug 4).
