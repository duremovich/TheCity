# Shadow V18 critic: care (ripperdoc, orderly, super, sick, teacher, school_parent, bereaved)

Window: 100k island, seed 42, d21-27, main 621882a. Diaries `target/shadow_v18/isle100k_s42_d21/`, cited `file dDD HH:MM`.
Picks: ripperdoc_13518, _48320; orderly_14415, _18501; super_43772, _80256; sick_42893, _71555; teacher_14367, _47553; school_parent_34478, _68009; bereaved_30486, _36837 (different people from V17's, so compare by archetype). **No minor is a victim, combatant, in vice, fined, jailed, courted or dead in any of the fourteen** (every `[event]` line with an age under 18 grepped: only the Custody line in bug 5).

## 1. Verdict

- Better than V17, mainly the clinics and the supers: a clinic now has 16-21 patients a week (V17's had one), supers name the tenant
  on every notice and fix, a night-shift mother's kids are warned, lent 3c and fed. It reads like a city.
- The walking pace makes the law efficient: 4 of 14 care picks sit in the cells (V17: 2 of 14), a teacher and a doctor lose their
  posts, and the cells have doubled. Dealing is a 20-minute road to a cell.
- Grief is still the thin spot: neither bereaved pick shows the mourning mechanism, and the one who swore against a killer never acts.

## 2. The exit's five stories (care group)

1. **A friend killed, a friend after the killer: partial.** `bereaved_30486 d21 00:01 GrudgeFormed: Dmitri Wexford swore against Flux
   Tanaka (killed kin, 1.00)`, then `d21 00:09 goal Hunt cooled until d24 00:09 (could not plan or plan failed twice)`. A 71-y with
   fighting 0.13 and 0c scavenges all week (steals 2 food `d22 04:38`). Believable; nobody goes after the killer. Not a story yet.
2. **Word of a killing getting around: seen, nobody acts.** `bereaved_30486 d21 01:32 told Sabine Reyes: Killed by Flux Tanaka on Verity
   Kemp`, and again `d26 06:45 told Tomas Ferreira`; `sick_42893 d23 21:07 GOSSIP: Wren Kettering told Ulysses Bellamy: Killed by someone
   unnamed on Wire Vance`, `d24 21:05` on to Undine Santoro. Two hops; no one reacts.
3. **A hit as business: not seen.**
4. **An open contract an NPC takes: not seen.**
5. **A life that makes sense on inspection: seen, `teacher_47553` (Quinn Upton, 50 y).** Gang soldier first, teacher second: `d21 07:07
   HANGOUT 176 min` on the corner instead of the 09:00 shift; `d21 10:00 Wounded: Cipher Brandt went down at Quinn Upton's hands`;
   `11:03 patched up ... no questions asked`; at School#5105 only from `11:18`, no `taught` line. `d22 01:53 Report: Juno Wozniak reported
   Quinn Upton for Dealing` (16 witnesses) -> `04:37 Arrest at home` -> `05:01 Sentence 5 days` -> `d27 05:01 Release ... the post as Teacher
   at School#5105 gone; ... The Glass Dogs Splinter 119 still hers` -> `08:49 welcomed back ... made lieutenant`. Follows from who she is.

## 3. The exit's three checks

- **Starving while Eat loses to a non-survival goal: none.** Lowest fed (2-hourly samples): `school_parent_68009` 0.11 (`d22 04:00`),
  `bereaved_30486` 0.32 (`d27 22:00`). Nearest miss, Mei Haas: `d21 11:42 GOAL Unwind (was Earn): score 0.399; runners-up: Eat 0.393 ...
  fed 0.46->0.68` (Unwind beats Eat by 0.006); `12:46 EatAtHome` -> `13:01 PLAN FAILED: StockGone`, 0c, and Eat is not a runner-up at `13:16`.
  Sleep, a night shift on an empty stomach, then `d22 04:29 GOAL Eat (was Work): score 0.889`, bought 3 food, ate twice. Not starving, but
  15 h with no kitchen, bin or friend step (bug 3).
- **Frozen identical-day loop: a near-loop, no exact repeat.** `sick_71555` (Annika Bellamy, 31 y, cancer, a 3-y child, net 0c every day,
  no `[money]` line in the whole diary): d23, d24, d26, d27 each run JobSearch `07:23` Street Market#4328, `08:23` Freight Yard#5948,
  `09:2x` Street Market#4324 ("nothing going"), with `Vend 16 min` / `HangOut 38 min` alternating hourly (52 Vend lines, never a sale), a
  DawnPick "sent home" `06:00` (d22, 23, 24, 26) and an 80c cut-back at `03:01-03:08` (d23, d24, d26, d27).
- **Any minor harmed: no** (bug 5 is the closest).

## 4. Per archetype (V17 word in brackets)

- **Ripperdoc: alive (thin/dead).** `ripperdoc_13518` (72 y): 6 patients by d23 (`d21 09:00 Treated ... blood washed (8c, 2c to the
  doctor)`, three implants at 6c), then `d24 04:39 Stripped: Tobin Jessop stripped the body of Verity Ito (0 coins)` -> Theft report ->
  `10:15 Arrest` -> 3 days -> `d27 10:37 ... the post as Ripperdoc still his`. `ripperdoc_48320` (47 y): his stolen bike reported and the
  thief inside in 42 minutes (`d21 11:44` -> `14:42`), then his own cell (bug 4).
- **Orderly: alive (alive/thin).** `orderly_14415` (Imani Ambrose, 43 y): 21 Treated lines (blood washed, 12c patch-ups, Therapy/Detox,
  `d26 17:18 Cured: Rhys Ambrose ... vat-cancer (60c)`); her bike hits a man (`d21 20:12 Crash`), is stolen (`d23 21:48`), she reports the
  thief (`d24 09:27`), buys another (`d27 09:42`). `orderly_18501` (18 y): 16 Treated, one Cured (`d21 09:00`).
- **Super: alive (alive/alive).** `super_80256 d21 06:22 Report: Jinx Bellamy went to Station#5619 and told the law: Yuki Mercer, for
  Shakedown` -> `12:38` she is inside; five fixes by name (`d24 11:58 fixed a jammed door in Beatrix Nightingale's flat`), seven stops in
  the hall (`d25 11:59` water). Notices are one named tenant (`d21 11:02 ... (Nadia Coldwater)`), no "22 tenants" wall. `super_43772`: a
  `SuperRound` 10:32 daily. No super moves house.
- **Sick: thin (thin/dead-ish).** `sick_42893` (Wire Vance, 74 y, Ash Rats): 213c lent `d21 00:00`, two 80c cut-backs (`07:23`, `08:06`),
  seven JobSearch boards, then dies in a fight (bug 2). `sick_71555`: the near-loop. No kin visit, no meal brought, no clinic hand.
- **Teacher: alive (alive/thin).** `teacher_14367` (Rue Underhill, 59 y): `taught 5, 7, 9, 11, 10 pupils` (V17: 21/17/21/24, fewer now),
  `d22 18:02 GOAL Visit` -> `19:41 came to the cells ... to see Mina Bellamy and told her the post ... is gone`, `d25 19:48 Visited: Mina
  Bellamy at Block#250 (hurt)`. A stim run `d22 00:48`, then 12 Wander blocks (01:48-07:40), then she teaches 7. `teacher_47553`: story 5.
- **School_parent: alive (flat).** `school_parent_34478`: two kids at School 5 of 7 days (`d25 09:30 at School#5195 (8 min walk)`);
  `d25 20:51 tried out for The Iron Brothers ... turned away: the treasury could not feed another mouth`; `d27 00:05 Housed: moved the
  children out of the blocks around Block#174 (The Neon Hands and The Hollow at odds there)`. Best of the group. `school_parent_68009`:
  the kids home all week over a 2c fee (bug 6).
- **Bereaved: thin (alive/dead).** `bereaved_30486`: stories 1-2. `bereaved_36837` (Fiora Kaneda, 73 y): no bereavement line at all;
  `d21 14:38 Report ... Dealing` (41 witnesses, 8 sales in 3 min) -> `15:21 Sentence 5 days` -> `d26 15:21 Release` -> `16:05 Deal` -> `17:46
  Arrest` -> `18:16 Sentence 5 days`: a revolving door in three hours. The mechanism is not seen (again).

## 5. The pace

Human scale: orderly commute 6 min, foot legs 18-20 tiles/min everywhere (`teacher_14367 d22 07:40 535 tiles 27 min`), Mei Haas's night
commute 953 tiles 53 min (`d21 19:29`), bikes 46-61 tiles/min (`orderly_14415 d21 18:32` 237 tiles in 4 min). Nothing crosses the island
in minutes. Pursuits end at human distances (`teacher_47553 d22 02:30 Pursuit: Quill Haas gave up the chase (out of reach) ... 46 tiles
off`). No absurd speed. The camera still audits at the old pace (bug 7).

## 6. Bugs, ranked by what the player sees first

1. **A third of the care picks are in the cells, and the cells have doubled.** `teacher_47553 d22 05:01`, `ripperdoc_13518 d24 10:37`,
   `ripperdoc_48320 d23 02:40`, `bereaved_36837 d21 15:21` (V17 2 of 14). JAILED lines over all diaries 23 -> 49; yard lines read
   "alone: 6266 in the cells" (`teacher_47553 d23 15:00`; V17 2.3-3.0k): about 6 % of the city locked up. Two posts lost (`teacher_47553
   d27 05:01`, `ripperdoc_48320 d24 14:00`). Likely the walking pace landing every Dealing report. New (a Y4 consequence).
2. **The law credits the wrong killer.** `sick_42893 d23 03:01 Assault: Hesper Varley (46 y) attacked Wire Vance (74 y)`; `03:08 Death:
   Wire Vance died of Violence` and in the same minute `Assault: Wire Vance (74 y) attacked Elowen Fletcher (52 y) and died`; `03:10
   CorpseFound ... Wire Vance died fighting Elowen Fletcher, who is charged with nothing`; `d23 12:34 Burial: Hesper Varley buried Wire
   Vance`. The man who struck is not named, the woman named did not, and the killer buries the body. The gossip then reads "unnamed".
   Likely the last-hit record holds the victim's own attack. New.
3. **Eat is not retried after StockGone for a lawful, broke parent.** `school_parent_68009 d21 13:01 PLAN FAILED: StockGone` -> no
   Eat candidate for 15 h (fed 0.43 -> 0.11 by `d22 04:00`) and a Vat shift on top. Y1's chain ends in theft for the less lawful; for
   a lawful 0c parent there is no kitchen, bin or friend step. Better than V17 (the kids eat after `d23 00:10`).
4. **A doctor goes stim -> gang -> dealing -> arrest in 24 minutes.** `ripperdoc_48320 d23 01:11 GOAL JoinGang (score 0.448)` -> `01:44
   GangJoin` -> `01:48 Deal` -> `01:59 Report` -> `02:08 Arrest` -> `02:40 Sentence 5 days`; the post is gone `d24 14:00`. A switch, not
   a slide. Compare `ripperdoc_13518`'s slow story.
5. **Children unfed two days before the service acts.** `school_parent_68009 d23 00:10 Custody: ... Rhoswen Thorne (8 y), Dante Thorne
   (6 y) went 2 days unfed ... 3 days' grace`; a friend lent 3c ("covered"), "fed last night" every day after. Doctrine says neglect has
   teeth through the service taking the child; two days with no meal is the closest a minor comes to harm here. For Dylan to judge.
6. **No School for a 2c fee** (V17 bug 8): **unchanged**. `school_parent_68009 d21-d25 09:30 at home (play: the 2c fee at School#5727 ...
   beyond the purse at midnight (0c in hand))`: five weekdays lost, on the service's list.
7. **The camera's JUMP audit is at the old pace.** `teacher_47553 d22 04:49-04:59`: 12 `JUMP (a Full body) 21 tiles in 1 min (allowance 2)`
   lines for a walk at exactly the new pace. Tool noise, not the sim.
8. **"No friend near to ask"** (V17 bug 2): **unchanged**. `bereaved_30486 d21 20:00 funeral ... no friend near to ask`; at `19:02` Sabine
   Reyes (Acquaintance +0.39) and five others were in the same Block#3262 hangout.

V17 care bugs: 1 mourning leave vs Arrest **not seen**; 2 no friend **unchanged**; 3 the unfed house **better**; 4 cancer man hired as a
Guard **not seen**; 5 a super changing address **not seen**; 6 repeated news **unchanged** (`ripperdoc_48320 d24 14:00` and `d27 12:00`
word for word); 7 empty clinic **fixed**; 8 School 2c **unchanged**; 9 stairwell smell **fixed**; 10 "foreman" label **not seen**.

## 7. OPEN-252's two questions

- **A parent with the fee buys the child out of the Camp: not exercised.** No care pick has a child in a Camp. Across every diary each
  `Visited ... Camp child` line ends `the 60c buy-out beyond the 0c/2c/4c/5c/7c/8c in hand`; nobody holds 60c, and one break-out is
  beaten back (`d23 12:57 Custody: Vale Lockridge tried to break Greta Crane out of Work Camp#4698 ... 5c in hand, and 13 of the gang free
  to come`). The window cannot say whether a parent with 60c would buy; this one has none.
- **A laid-up patient is fed: yes, silently.** `gang_member_92195` (Wisp Oyelaran, 24 y): `d22 02:30 Wounded`, `02:38 Rescued: Verity
  Northcott bound ... wound`, laid up in Back Room#1008 until `d25 03:12 Recovered`. Fed climbs 0.58 -> 0.98 at 02:00 on d23, d24 and d25
  (`needs hunger` 00:00 vs 02:00) with no `[meal]`, `Eat` or "brought a meal" line. The mechanism works; the narration is missing.

## 8. Dull spots and too much

- **Dull:** the idle block (`teacher_14367 d22 01:48-07:40`, 12 Wander blocks); the `sick_71555` Vend loop that never sells. **Too much:** Dealing as the easiest catch (3 of 14 jailed, 16-41 witnesses per deal); 21 identical "blood washed 8c" lines for one orderly.
