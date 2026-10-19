# Shadow V5 critique: vice (club_staff, bouncer, dancer, madam, companion, joytoy, fighter, cook)

45 diaries (2 per archetype per window; s43 has one madam because the 2k city has one Brothel), read by grep on header,
summary and event/flow/memory lines. All at Full LOD (pinned). Every header shows the same shift `[(1080, 1440)]`
(18:00-24:00), Bartender and Cook included. Cites are `window/file dD HH:MM`.

## Per archetype

**Club staff** (Concierge, Croupier, Attendant, Club Staff). Changed: the server now meets people. 8-23 `served X`
memories a week (s43 club_staff_1681: 23, s44 2157: 19), 3-15 of them flagged regular, with Acquaintance edges made at
21:00. Idle fell from 53-72 h (V4) to 14-50 h. Still one 356 min ClerkWork block; no refusal, no ejection. Verdict:
**alive-ish, thin** (from thin).
> `k2_s44_d90/club_staff_646 d92 21:00` `[memory] served Jinx Varley#906, a regular (valence +0.10)`

**Bouncer.** Unchanged. 32 ClerkWork blocks over 6 diaries, no door refusal, no ejection, no fight event. The only
`served` lines are the bouncer being served at a street market (s44 bouncer_1303 d91 09:52). 43-47 h of travel at s43,
one 95 h jail stay (s44 1303). Verdict: **not alive as a bouncer**.

**Dancer.** Changed, and the best change in the group (V4 bug 7: 0 tips in six dancers). Dancers are tipped: 12-23 Tip
flows from 10-20 distinct tippers at 2k, 4 flows (6-12c) at k20. Tippers become `served ... a regular` memories and edges
(dancer_1829: Mags Coldwater +0.21 -> +0.31 d93; Percival Ambrose Acquaintance -> Friend d92 22:05). k20 dancers are thin
(6c, 12c in the week, tips only at 19-21 h). Verdict: **alive** (from thin).
> `k2_s44_d90/dancer_1829 d91 21:00` `[money] +3c (Tip) ... Tip +1 from Bex Oyelaran#931, Tip +1 from Static Ferreira#1466,
> Tip +1 from Nadia Ravensworth#2017`, then `served Static Ferreira#1466, a regular`

**Madam.** Changed: the house is met. Each shift opens with 3-6 `served <name>, a regular` memories at 18:00 (the Staff
encounter: s44 madam_602 d91 Ursula Osgood, Brenna Stroud, Jocasta Santoro; 14 of 15 regular-flagged). But no cut flow in
any of 4 diaries (Wage only, 12-16c from corp Kessler), no tip at her house, no Hire/Layoff by her, valence +0.00 and no
edge from the staff lines. Idle 27-51 h. Verdict: **thin** (from not alive): workers are named, management is not.
> `k2_s44_d90/madam_602 d91 18:00` `[memory] served Ursula Osgood#885, a regular (valence +0.00)`

**Companion.** Changed: idle 40-66 h (V4 74-94); k20 Lounges now pay (72c and 32c, V4 0c). A `Served: X is now a regular
of Y` event fires in 5 of 6 diaries, with a memory and an Acquaintance edge in the same minute. No follow-through: Greta
Tanaka is never met again after d31 21:00.
> `k2_s43_d26/companion_1381 d31 21:00` `[event] Served: Greta Tanaka is now a regular of Tess Kemp at Spire Lounge#450`
> `... [edge] new relationship with Greta Tanaka#2351: Acquaintance affinity +0.12`
Verdict: **alive, thin on follow-through** (from thin).

**Joytoy.** Mostly unchanged. 29-35 h at Dollhouse, 3-17 `served` memories from 1-3 distinct people, no tips (untipped
trade), no regular flag. Foreman `Hire` lines appear (s44 joytoy_625 d91, joytoy_795 d92 x2; k20 22006 d23-24). s44
joytoy_625's chip contract ended "cut short, 22 days" (d92) and he then spent 132 h jailed. No chip-cost episode.
Verdict: **thin**.

**Fighter.** Thin but real web: 36-66 edges, a Spouse, 2-3 Friends at +1.00, the bout partner as top contact. HangOut
distinct known contacts: 0 (s43 1595), 3 (953), 0 (s44 1485), 6 (1676), 3 (k20 11888), 1 (8372). Bouts at 23:00 every
second night, ONE fixed pair per Pit, 18 of 18 `bets paid 0` (bugs 2, 3).
> `k2_s44_d90/fighter_1485 d91 23:00` `[event] Bout: Cerys Thorne beat Tess Crane at Fight Pit#544 (bets paid 0)`
> (his header: `Tess Crane#2509 [Spouse] affinity +1.00`)
Verdict: **thin** (the bout is a nightly ritual with a friend or spouse).

**Cook.** Mostly V4: 30 ClerkWork blocks, no cooked meal or staff meal in any diary. Changed: foreman lines (s43
cook_2281 Quit d29 "unpaid", Hired d30), 2-9 `served` memories, no regular flag. 24-61 h idle. k20 cook_19650 (82 h
jailed) is again the crime story. Verdict: **alive as work, thin as a life**.

## Bugs

1. **The Bar is staffed in the evening; the drinkers come by day.** 100 Bar actions in these 45 diaries: 73 before 18:00,
   27 after. s44 cook_1978 drinks at Bar#460 d92 13:17, d93 13:15, d94 12:13 and the file has 0 `served` memories, no edge
   to anyone there; s44 dancer_1829 Bar#460 d90 07:19, d92 07:26, d96 14:02; k20 madam_19915 Bar#4166 d20-24 x5
   (01:45-09:34); s43 fighter_1595 Bar#415 d26/30/32 at 10:08-11:38. Bartenders work 18:00-00:00 (s43 sick_2157
   BartendWork Bar#416 d26 19:41, 259 min; d28/d29 18:27, 333 min). OPEN-111 moved the staff to the evening; the Unwind
   pick (`UNWIND pick: rung Cheap, Drink at Bar#460`) has no hour, so the "nobody on duty" problem moved to the other
   side of the clock. Distorts 10 of 45 diaries (every Bar patron) and the whole Bar-regular story.
2. **Bouts: one fixed pair per Pit, every other night, no wagers.** s43 Pit#544 Hesper v Cerys d26/29/31, Pit#578 Kieran v
   Sabine d28/30/32; s44 Pit#544 Cerys v Tess Crane d91/93/95, Pit#578 Viktor v Imogen d91/93/95; k20 Pit#5686 Mateo v
   Nico, Pit#5690 Sprocket v Orrin, d21/23/25. 18 of 18 `bets paid 0`. V4 bug 17 is thinned to alternate nights, not fixed.
   Distorts 6 of 6 fighters.
3. **The opponent is the spouse or best friend.** s44 fighter_1485 fights his Spouse Tess Crane three nights; every
   other fighter fights a `[Friend] +1.00`. The Bout memory (valence -0.40) does not touch the edge (still +1.00 in the
   header), so no rivalry grows. Likely cause: opponent picked by fitness among the Pit's staff, no edge test. Distorts 6 of 6.
4. **One patron tips 3 times a night (k20 companion_19139).** d21-24 at 21:00, `Tip +4 from Nikolai Varley#6070` x3 in one
   line, 12c a night, so the 72c week is one man. It looks like the same patron served three times in one tick (the same
   hashed purpose per hour). Distorts 1 of 4 companions.
5. **Every venue trade is the same 18:00-24:00 ClerkWork block.** Bouncer, cook, dancer, madam, joytoy, fighter,
   companion: `(1080, 1440)`, one 356 min block, a walk home at 00:00 (e.g. s44 madam_602 d91 18:03). The bouncer has no
   door, the cook no pass, the fighter nothing before 23:00. Unchanged since V4 gap 2. Distorts 45 of 45.
6. **A late first evening.** s43 companion_1381 d31 18:00 asleep, GoTo 19:12, ClerkWork 19:29, 271 of 360 min: the shift
   start does not interrupt Sleep. Weak evidence (one diary read in full); not counted elsewhere.

## Gate items

- **Venue evening shift and the stable server: bug (bug 1) for the Bar; settled for Club, Lounge and Strip Club.**
  No bartender is among my eight archetypes. In the four Bartender diaries I found (other groups), only sick_2157 (s43)
  has Bar#416 evening shifts; they show BartendWork 259-333 min d26/28/29 evenings, but no `served` memory and no Regular
  event for a bartender in that week (the player is a sick pick, not a trade life). The three others
  (gang_leader_725 s44, purist_10228 and worker_friday_16743 k20) were not on shift. One week does not prove a stable
  server. The Club trades do show regulars: club_staff_11628 (k20) 15 of 16 `served` memories are regular-flagged
  across 5 distinct patrons.
- **A regular's week from the patron side: bug (bug 1).** cook_1978 drinks at the same Bar 3 days at 13:15, no edge and no
  `served` memory. Companion regulars appear but never repeat in a week except Nikolai Varley (bug 4).
- **Venue day shifts, visits with no one serving: bug (bug 1).** 73 of 100 Bar visits are by day against an evening-only
  staff; no `served` line at any of them. Club/Gambling Den visits split near 50/50 day/eve.
- **Madam's cut: decision for Dylan.** No cut anywhere (4 of 4 madams, Wage only). `madam_cut` 0.3 of a 1c tip rounds to 0
  and the 2k Brothel's tips are 1c: do you want a minimum 1c cut (so a figure is visible at 2k), or is "named staff, no
  money" the design for now?
- **Fighters: settled as a thin but real web** (36-66 edges, HangOut distinct known contacts 0-6, median 3). **Bouts: bug
  (bugs 2, 3)**: a fixed pair per Pit, every other night, `bets paid 0` 18 of 18, opponent a spouse or best friend.
- **Tips (V4 bug 7): settled for dancers** (12-23 flows, 10-20 distinct tippers at 2k, V4 had 0). Spread across workers:
  dancers and companions only, by design (§2.1); **bug (bug 4)** at k20, where one patron is the whole companion income.
