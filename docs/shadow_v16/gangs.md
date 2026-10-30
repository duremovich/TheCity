# Shadow V16: the gangs (critic report)

Group: gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider (two picks each, 16 diaries). 100k island, seed 42, d21-d27. Cites are `<arch>_<id> dDD HH:MM` under `target/shadow_v16/isle100k_s42_d21/`. Ids differ from V15, so comparison is by archetype. Counters: `target/r7_100k/analyze.txt` (Raid 216 -> 317 in 30 days vs r6, TerritoryFlipped 55, Turf 4,006).

## 1. Verdict

- **Better than V15: the war now has an ending.** V15 had 0 truces, 0 crackdowns and a war only the leader heard. V16 has a truce in 4 diaries (`gang_member_6769 d22 00:05`, `gang_leader_19672 d24 00:05`, `scav_99001 d23 00:05`, `raider_36163`), a law crackdown (`gang_leader_61823 d26 06:00`), "went quiet" closes, members hearing the leader's lie-low call, and one whole arc (war, crackdown, lie low, arrest at the Hideout, sentence, succession, grudge) in `gang_leader_61823`.
- **Still two thin layers.** Runners are not gang people at all (both picks are a guard and a sanitation worker, no gang line), and the street-level archetypes (scav, dealer, purist, cultist) are a Turf line a day over a work loop. The war is told in the same 00:05 template every night.
- **Under-18 rule holds.** Every `[event]` Assault, Brawl, Murder, corner, Raid and Arrest line across the 16 prints ages; none is under 18 (0 hits). The only minors in these diaries are children at home or in the Camp (`gang_member_13287 d21 09:30`, "14 y in Work Camp#4699"), unharmed.

## 2. Per archetype (V15's word in brackets)

- **gang_member: alive (alive, one).** `_6769` (27 y, Salt Ghosts, 47 men): `d22 00:05` "Bram Ferreira called The Salt Ghosts off the corners to lie low: 10 of their 38 beaten, dead, jailed or gone", same minute the truce; `d23 06:30` "woke on the street to find 4c gone from the purse ... tells nobody: she runs with The Salt Ghosts, and the law is not their way"; `d25 00:05` "The war ... went quiet". A member now hears the war. `_13287`: `d26 02:55` leader calls 22 to the corner, `d26 11:00` "Iron Brothers are on The Neon Dogs' corner ... nobody of The Neon Dogs' there", `d27 00:05` blocks retaken (37 of theirs).
- **gang_leader: alive (alive).** `_61823` (Rue Sato, Ash Wolves, 41 men): four wars in six nights, `d26 06:00` "The law cracked down on the blocks ... the beat walks its Homes for 7 days", `d27 00:05` lie low, `d27 20:03` "Elowen Penhallow arrested Rue Sato for Shakedown", `d27 21:55` sentence 6 days, "Lux Draper runs The Ash Wolves while Rue Sato sits inside", "learned who put her inside: Undine Ferreira", GRUDGE. Best story in the group. `_19672` (Zane Singh, 70 y): `d21 04:50` robbed asleep, `d21 04:51` fights back, beats the robber (24 y) and a second (50 y); `d24 00:05` truce; `d26 09:23` arrested for Assault, `11:47` sentenced 10 days.
- **purist: alive, one (thin).** `_42845` (28 y, Unplugged 9) Preach 31 times in a week (14 GangWork Preach plans), see Dull 2. `_36839` (20 y, Night Stocker, order Job) deals three times with no witness spam, `d24 21:00` arrested at home for Grand Theft, `d27 12:48` "Cato Zhou reported Pixel Draper for Dealing".
- **cultist: thin (alive, one).** `_80865` Recycler tech tending graves, 2 Extort plans, wars `d25 21:00` and `d26 00:05` read from the floor. `_16565` (9-man Drowned Choir 2): `d21 10:26-22:38` twelve hourly `Unwind [GoTo(Home) > HangOut]` plans on the same Block#633 with nothing between. No muster, no creed event.
- **scav: thin (thin).** `_99001` (74 y, Grey Teeth, treasury 9,589c): `d21 04:00` "Grey Teeth pushed The Cinder Hands off the corner, 6 against 3", `d23 00:05` truce "24 blocks and 9", scav plan `[GoTo(Bin) > Scavenge]` nine times, no find line. `_75791` 24 Extort mentions, 6 Raid.
- **dealer: thin (alive).** `_15373` (52 y, Static Teeth, 3 men): 1 Deal in a week, 3 Extort plans, `d24 06:46` "Nadia Yates sat out The Static Teeth's raid: the muster at Hideout#4552 is 5 h 47 from home". V15's dealer arc did not recur.
- **runner: dead (thin).** Both picks are not runners: `runner_45230` is a Precinct guard (`d25 01:50` arrested Vivienne Ambrose for Grand Theft), `runner_87360` is Sanitation with a T1 deck and "goal Hack cooled until d22 00:48". No gang line, no run.
- **raider: alive, one (thin).** `_29990` (67 y, Wire Hands, Retaliate): `d21 03:00` war, `d22 02:00` "attacked 2 at the corner (33 y, 43 y)", `d22 02:00` beaten back 6 against 6, `d25 04:52` "kept off the blocks ... after dark (The Salt Widows and The Hollow at odds there)", `d26 00:05` blocks retaken. `_36163` is a beg-and-scavenge loop with a truce line.

## 3. Round 7's mechanisms seen at 100k

Fired, quoted:
- **W2 members hear their gang's war and a called member holds his corner:** `gang_member_13287 d26 02:55` "called 22 of theirs about the blocks to the corner by Block#720", `d21 00:05` lie-low call in a member's diary; `raider_29990 d25 04:52` keeps off the blocks.
- **W2 a sleeper robbed wakes to the short purse and says why not:** `gang_leader_19672 d21 04:50`, `gang_member_6769 d23 06:30`, `cultist_16565 d21 06:05`, all "tells nobody: ... the law is not their way". Reads as a rule, but a rule that explains itself.
- **W2 raid waits for its marchers / sits out:** `gang_leader_61823 d26 00:07` "sat out The Ash Wolves's raid: the muster ... is 3 h 08 from home"; `dealer_15373 d24 06:46` 5 h 47. V15's bug 1 (a mustered raid walked off for 5.5 h) is gone: the member declines. But no raid lands in these diaries (bug 3).
- **W2 finder names the killer:** `cultist_16565 d24 19:20` "the law named Seraphine Whitlock for the Murder of Hana Yates (who knew the dead; the body found by Nerys Wexford)". V15's repeated "no suspect" did not recur.
- **W3 the guard makes the arrest:** `runner_45230 d26 00:50` "GoTo(SuspectTile) > Arrest > Escort", arrest `d26 02:31`; `gang_leader_61823 d27 20:03` arrested at the Hideout; `gang_leader_19672 d26 09:23`.
- **W5 stand witnesses gathered:** `purist_36839` three Deals, zero per-witness "commit Dealing" lines (V15 bug 3 fixed). Report line `d27 12:48`.
- **Turf you hold / war / truce / crackdown:** all of section 2. Truce and crackdown are new since V15.

Expected, never seen: a Raid marching and landing (a Raid-order gang whose members sat out, no Brawl), Camp Escape, a gang tribute or protection line, a Hideout raid that names the cells that moved, a creed event (a purist refusing a chromed fare), hack runs.

## 4. Bugs, ranked by what the player sees first

1. **A war's lie-low line can name a different enemy than the war line.** `cultist_80865 d25 21:00` war vs "Unplugged 4"; `d26 00:05` "since the war with The Unplugged 5 began on day 24"; `d27 00:05` "war with The Unplugged 4 began on day 25" under a third leader. Likely cause: two concurrent wars and the lie-low line reading one, or the gang name resolving to a neighbour. Reads as a contradiction. New.
2. **Leaders change without a Succession line.** `raider_29990`: Rhys Garland "leads 23" at `d21 03:00`, Jasper Marlow "leads 23" at `d22 02:00`, no Succession event (Rhys still hangs out `d22 00:11`). `scav_99001`: Sabine Vance "leads 61" at `d21 04:00`, Yorick Underhill leader at `d23 00:05`, while Sabine is "sighted at Hideout#4508" `d22 06:38`. Versus the proper `Succession: Lux Draper runs The Ash Wolves` in `gang_leader_61823 d27 21:55`. Likely cause: the war-line leader is derived per line, not from the roster. New.
3. **The raid still never lands.** `gang_leader_61823 d25 21:41` order Raid, `d26 00:07` "sat out ... 3 h 08 from home"; `dealer_15373 d24 06:46` sat out. The 5.5 h walk is gone but the muster point is chosen with no regard to who is near, so members sit out. V15 bug 1: better. Counters say 317 Raid events in 30 days at 100k, so raids land elsewhere.
4. **Runner archetype is not a runner.** `runner_45230` Guard, `runner_87360` Sanitation, no gang. Either the shadow picker's runner filter is wrong or the role is a job label, not a life. V15 had the same shape: unchanged.
5. **Old bodies in corner fights.** `gang_leader_19672` is 70 y (fighting 0.15), attacks three at a corner incl. a 72 y (`d23 23:00`), and is arrested for Assault on a 72 y (`d26 09:23`). `raider_29990` is 67 y. V15 bug 4: unchanged.
6. **Two voices.** `gang_member_13287 d27 19:37` "standing order is now Raid", `d27 22:27` goal GangWork with Raid 0.545 as a runner-up and no Raid plan. V15 bug 5: unchanged.
7. **A guard watches six fare dodgers and does nothing.** `runner_45230 d25 18:21-19:32` "saw X commit Fare Evasion" x6 with no action. One at a time (W3 holds), but passive. Minor.

V15 bugs in this group: mustered raid walked off **better**; witnessed murder reads "no suspect" **fixed** (the finder's line names the law's suspect); five Witness lines per stand **fixed**; old/starving bodies in the corner fight **unchanged**; two voices **unchanged**.

## 5. Dull spots, with the mechanism

1. **Twelve hourly HangOut plans at home.** `cultist_16565 d21 10:26-22:38` (also `scav_99001`). Mechanism: a cult member's free time is a rite (a funeral, a dawn gate, a street Preach a stranger answers), not an Unwind row.
2. **Purist Preach has no listener.** `purist_42845` Preach 31 in a week, no one is turned or turned away. Mechanism: a Preach a passer-by answers (agrees, jeers, reports the chrome one wears).
3. **Scav finds nothing.** `scav_99001` nine Scavenge plans, no find line. Mechanism: sell what the corner leaves, or the bin is a trade the gang taxes.
4. **War lines are one template at 00:05.** "called X off the corners to lie low: N of their M beaten, dead, jailed or gone". Mechanism: name the loser (the one beaten, the one jailed) and give the leader a choice (pay, surrender, call in a favour).
5. **Dealer has no customers.** `dealer_15373` 1 Deal in a week. Mechanism: the stand follows the held corner.

## 6. Too much

- **A fresh war every night on the same leader.** `gang_leader_61823`: d21 Glass Teeth, d24 Salt Crows, d25 Static Hands, d26 Iron Brothers (12 of 38 gone in one day). Dylan accepts harsh; a new enemy each night is the monotonous part, not the violence.
- **Names.** "Cinder Ghosts / Hounds / Hands / Lanterns / Crows" (`gang_leader_19672`), "Unplugged 4 / 5 / 9" (`cultist_80865`): near-identical placeholder names make the war lines hard to read and bug 1 worse.
- **Corner fight witness lines.** `gang_leader_19672 d23 23:00`: 12 witnesses, 3 told, 3 ran, five lines in one minute. Noisy but one event.
- Not broken: truces ("held 21 days"), crackdown and arrest arcs, no stand or marriage spam in these diaries.
