# Shadow V11 critic: care (ripperdoc, orderly, super, sick, teacher; child has no diary)

Window: main f0b7e44 (shadow sha), 100k island, seed 42, d21-27. Diaries `target/shadow_v11/isle100k_s42_d21/`, cited `file dDD HH:MM`.
Ten picks: ripperdoc_64758, ripperdoc_9836, orderly_18123, orderly_23601, super_104162, super_96896, sick_81571, sick_91688, teacher_43475, teacher_51715.
Day 27 is a rest day for every worker here (the diary now marks it: `DAY SUMMARY d27 (rest day: no shift)`).

## 1. Verdict

- Against V10 this group reads worse: the super, V10's best story, is a statue again (no round, no notice, in both picks), and no clinic worker saw a patient (V10: one of four).
- What is new is thin but real: the sick make a first Friend's-loan ask (refused), a broke orderly's week ends in a brawl and ten days in jail, an evicted 71-year-old doctor is housed and walks 150 minutes to work.
- Stories start (a refused loan, a jailing) and nobody picks them up: the refusal ends the sick man's care-seeking, and the jailed orderly's post is simply empty.

## 2. Per archetype

- **Super: dead (V10 alive).** Both supers do one `ClerkWork` of 506-538 min at their Block and nothing else (`super_104162 d21 09:02 PLAN Work [ClerkWork]`, 538 min; `super_96896 d22 09:02 ClerkWork` 538 min). Zero `Notice`, zero "called on", zero `SuperRound` in either. The fold works for another super (`miserable_22034 d22 09:35 Notice: Fiora Fairweather served notice on 14 tenants in arrears on the round of 6 Blocks`), so the round exists; these two never take it. 96896 (0-tile post) then HangOuts "nobody there" five times a night (`d22 18:26`-`22:38`); 104162 walks 66-72 min to a Fight Pit and 120-136 min home (`d24 22:16 GoTo(Home) 136 min, 94 tiles`).
- **Ripperdoc: thin, both (V10 alive/thin).** No `Treated:`, no patient; after the morning walk the day is `Rest` then six to eight `ClerkWork` blocks of 31-79 min (`ripperdoc_64758 d21 09:05`-`15:44`, `ripperdoc_9836 d21 09:03`-`11:51`). 9836 is the story: `d21 00:01 Story: Civic Wire: Habitat 9 evicted Brick Reyes`, a church cot (`PLAN Sleep [GoTo(Cot) > CheckIn > Sleep]`), `d25 00:00 Housed ... Block#624 (Habitat 6)` in D4, then a D3 clinic commute of 153 and 162 min (see bug 4). 64758 buys a motorcycle on finance (`d27 10:24 AssetBought ... Motorcycle T1 ... on finance`) and has Witness lines only.
- **Orderly: alive for one, thin for one (V10 thin).** `orderly_23601` is a full arc: `d21 00:00 RentShort ... (0 of 1)`, `d24 18:35 Drink` (2c), `d24 19:22 Brawl: Bex Inchbald (drunk) swung at Joss Vasquez at the door of Gambling Den#6009`, `d25 01:37 Arrest`, `d25 02:52 Sentence ... 10 days for Assault`, jail 24 h a day d26-27. `orderly_18123` has three `treated ... (a patient)` memories (`d21 11:55`, `d22 12:27`, `d25 10:33`), no `Treated:` event, and witnesses thefts and assaults (`d25 01:00 WitnessReacted ... ran`).
- **Sick: thin (V10 thin), one new step.** `sick_91688` (60 y, 10c): `d26 00:00 Diagnosed: ... vat-cancer`, same minute `Loan: Rhys Tanaka begged Joss Ferreira for 218c toward a vat-cancer treatment; Joss Ferreira turned them down`. Then `d26 00:22 Earn cooled`, Idle, Rest; no second ask, no clinic; the stage goes 0.25 to 0.27 by d27. `sick_81571` (72 y, 1c, diagnosed before the window) scavenges and idles; the stage climbs 0.46 to 0.57 (`d22 06:42` 0.49, `d27 09:49` 0.57), a Spouse at +0.87, no ask line at all.
- **Teacher: dead (V10 dead).** Both do `ClerkWork` blocks of 31-90 min at the School all day. The one school line is the same person at the door, five mornings: `teacher_43475 d21 09:00 met Dorian Adeyemi#113955 at the door (a pupil's parent)`, again d22, d23, d24, d25. R6b's finding holds: no pupil, no lesson, no truant.
- **Child:** no diary (cannot be pinned). The teachers confirm there is nothing to teach.

## 3. Round 2's mechanisms seen at 100k

- **Seen: R3's Friend's loan** (`sick_91688 d26 00:00`), refused. One ask, to one name.
- **Seen: R4's rest-day mark** (all ten diaries, `d27 (rest day: no shift)`; the jailed orderly shows `(the city's rest day)`).
- **Seen: R4's swapped-gossip fix** in the sense that no inverted `Assaulted by` line appears in these diaries.
- **Seen: R1 fares in the witnesses** (`ripperdoc_9836 d23 12:06 Witness ... Fare Evasion`, `SawCrime ... valence -0.10`) and `d25 20:03 -1c (Fare)` to the City. No fine or El ban in this group.
- **Seen: jail** (`orderly_23601 d25 02:52`, `d25 03:22 MetInJail`).
- **Expected, never seen:** R3's slate, pawn, back room, `Cured`, `Patched`, a rescue (all zero in ten diaries; `Treated:` zero); R4's super `Notice` line (zero for both supers); patients at either clinic; a wounded arrival for either orderly; the sick post covered when the orderly was jailed (`orderly_23601` d26-27, no cover line); a teacher's class.

## 4. Bugs, ranked

1. **The supers do no round** (V10: alive; now **worse**). `super_104162 d21 09:02`, `super_96896 d22 09:02`: a 500-min ClerkWork in place of the round, while a super elsewhere serves notice (`miserable_22034 d22 09:35`). Likely the round is chosen only when the Blocks have arrears and the fallback is ClerkWork. The player sees a super who sits 9 h in the lobby.
2. **No patients reach the clinic** (V10 bugs 2 and 3: **worse**; V10 had `Treated:` for one of four, now none of four). Both ripperdocs and both orderlies, six work days: no `Treated:` and no `GoTo(Clinic)` past the commute. Whether R3's walk-ins failed or the sample missed them, the diary shows a clinic with nobody in it.
3. **The sick stop after one refusal** (V10 bug 1: **unchanged, one new step**). `sick_91688 d26 00:00` asks `Joss Ferreira` while the Spouse (+0.94, trust 1.00) and six Friends at +1.00 head the relationship list; 218c is asked against a 10c purse; then nothing. `sick_81571` never asks.
4. **A home 150 minutes from the post** (V10 bug 4, 521-min walk: **better pace, new cause**). `ripperdoc_9836 d25 00:00 Housed ... Block#624 (Habitat 6)` is in D4, the clinic in D3: `d25 05:56 GoTo(Clinic) 153 min, 138 tiles`, `d26 05:26 162 min, 148 tiles`, `d25 02:20 GoTo(Home) 216 min, 199 tiles (cut)`. That is about 0.9-1.1 tiles per minute: R2's "twice as fast" is not visible on a long haul (9 tiles take 6 min at `teacher_43475 d21 08:05`). A 71-year-old doctor housed far from his post is a housing-pick problem.
5. **A pupil's parent with no pupil** (V10 teacher dead: **unchanged**). `teacher_43475 d21 09:00` through `d25 09:00`, the same name each morning. The seeded city has no school-age children (R6b), so this is a memory line with no child behind it.
6. **An orderly named as the hiring foreman** (`orderly_18123 d26 00:11 Hire: Bertram Northcott hired as Manager (picked by Clemence Santoro, the foreman)` and the Accountant line). The role label or the picker is wrong for an orderly of Ambrose Holdings.
7. **One eviction, two Stories** (`ripperdoc_9836 d21 00:01 ... (paid by Vatra 10)` and `d24 00:01 ... (paid by Vatra 40) by Dagny Nightingale`; the gossip names `Habitat 9` and `Habitat 18` for the same fact at `d26 22:18`/`23:23`, one for a different person). It reads as the same eviction reported twice, or a second eviction after a Housed that came later.
8. **Seven Friends drop 0.94 to 0.83 at the diagnosis minute** (`sick_91688 d26 00:04`; `orderly_18123 d26 00:04` shows the same drop for a different person). No cause line. It looks like a midnight drift, but it lands on the sick man's worst night and reads as the diagnosis souring his friends.
9. **Drunk after a 46-min, 2c drink** (`orderly_23601 d24 18:35`-`19:22`). A short fuse, not a bug, but the jail sentence of ten days and the lost clinic post follow from it.

V10 bugs in this group: 1 (no care-seeking) **unchanged**; 2 (clinic is ClerkWork) **unchanged**; 3 (fee does not reach the doctor) **not exercised**; 4 (521-min walk) **better**; 5 (notices without a cause) **worse** (none at all; the fold itself is fine elsewhere); 6 (swapped gossip) **fixed**; 7 (Shop unplannable on a rest day) **not seen**; 8 (slow short walks) **better**.

## 5. Dull spots, ranked (mechanisms)

1. **The clinic has no door.** A patient (the wounded, a sick person on the slate, a cash walk-in) arrives and the doctor's block becomes a `Treat` with a name and a cause; a day with no arrival is a back-room cash job, not eight ClerkWork blocks.
2. **The super's day without arrears.** When the round has nothing to serve, give the post other duties: rent short, a leak, a squatter, a complaint at the lobby. Today a quiet round is a lobby clerk.
3. **The refused loan opens a ladder.** The refusal moves to the next name (the Spouse, the Friends at +1.00), then the slate, the pawn (deck, parts) or a black doc; the diary shows only the first rung.
4. **Teaching.** A class is a scheduled block with pupils from nearby Homes; a truant is a missed visit; a parent at the door is a complaint. This needs school-age children in the seeded city first (R6b), or the school is an office.
5. **Housing that weighs the post.** An old doctor moved off a cot should be housed near the clinic, or the post should move with the home.
6. **A jailed orderly's post.** `orderly_23601` holds a job it cannot work on d26-27; a vacancy, a short-staffed doctor or a colleague covering would pick up the story (R3's "sick post covered" is in the brief and is not in the diary).

## 6. Too much

- Nothing reads as a broken city. The monotone is the flat ClerkWork days (about 8.8 h x 6) for supers, doctors, orderlies and teachers, plus the 61-min empty HangOuts. Harshness is fine and should stay: a ten-day jailing for a drunk shove, a refused 218c, a cot at 71 all read right.
