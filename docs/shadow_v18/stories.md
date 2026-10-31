# Shadow V18: the stories critic (2026-10-31)

Scope: all 118 diaries in `target/shadow_v18/isle100k_s42_d21/` (d21-27) plus the 100k 30-day log
`target/r9_100k/events.tsv`. Cites: diaries `file dDD HH:MM`; events `tick Kind` (tick/1440 = day). The event log is
a separate run of the seed, so a person in one is not in the other unless said. Every chain was followed by name
plus a second anchor (building, kin edge, entity index), because names collide at 100k.

## Verdict

1. Story 1 (friend killed, friend hunts the killer): **mechanism only in the diaries, shown in events.** The full
   chain fires and reads well; no diary caught a hunter hunting, they stop at the grudge.
2. Story 2 (word spreads, someone acts): **shown.** Story 3 (a hit as business): **shown.** Story 5 (a life that
   makes sense): **shown.** Story 4 (open contract an NPC takes): **mechanism only.**
3. V18 has stories where V17 had log lines, but a player following one person sees the middle of each story, not
   its ends. The best chain (Moth Wexford) is in no diary; the hunter the diaries do carry (Dmitri) is 71 and
   his Hunt "cooled" the same minute.

## 1. A friend killed, and a friend goes after the killer: MECHANISM ONLY in diaries; SHOWN in events

Best instance (events; no diary carries these people):
- `38862 Murder` "Liesl Draper (65 y) hit bystander Joss Inchbald (53 y) at Arasaka's Security Office#4589 and died";
  `Bereaved` "Moth Wexford lost his husband Joss Inchbald".
- `38865 Report` "the law named Liesl Draper for the Murder ... (from the scene)"; the CorpseFound row notes a witness
  "saw it done and said nothing: she runs with The Glass Teeth Splinter 125, and the law is not their way".
- `38881 GrudgeFormed` fifteen people at once: Moth, Viktor Upton, Roxy Singh "(killed kin, 1.00)"; Nadia Bellamy,
  Kira Kowalczyk, Rex Yates and ten more "(killed a friend, 0.55-0.60)".
- `38919 HuntStarted` "Moth Wexford went looking for Liesl Draper" (38 ticks after the death).
- `38992 Murder` / `Avenged` "Moth Wexford (66 y) attacked Liesl Draper (65 y)" / "avenged Joss Inchbald on Liesl Draper
  (killed)"; `38995 Report` "the law named Moth Wexford for the Murder of Liesl Draper (who knew the dead)".
Killing, grieving husband, hunt, revenge killing, the law coming for the avenger, in 130 ticks. It reads as a story.

Second instance, a hunt that drifts: `1144 Murder` Zed Hawthorne (71 y); `1157 GrudgeFormed` husband Yorick Coldwater vs
the body-stripper Cutter Singh; `1441 Attributed` laid to Ottilie Kemp, Yorick and Nikolai Kemp "(killed kin, 1.00)";
`1443 HuntStarted`; Ottilie is thrown out of four clubs, jailed `5102`; Yorick, after leave, attacks Cutter Singh instead
(`5623 Assault`). It picks the closer target; nothing says why.

Diaries, what a player sees: `bereaved_30486 d21 00:01` "GrudgeFormed: Dmitri Wexford swore against Flux Tanaka (killed
kin, 1.00)" off "heard a rumour: Killed by Flux Tanaka#72315 on Verity Kemp#37317 (hops 1, conf 1.00)"; then `d21 00:09`
"goal Hunt cooled until d24 00:09 (could not plan or plan failed twice)". No Hunt line d24-27. `bereaved_36837 d21
00:00` (Parent) and `purist_12613 d21 00:00` (Friend, 0.60) hold the same grudge against Idris Hawthorne#14336; neither
hunts. A grudge with no follow-through is not story 1 by the brief's own test.

City-wide (30 days): 389 Murder; 2,361 killed-kin/friend grudges; **150 HuntStarted (125 distinct pairs, 71 from a
killed grudge, ~3 %)**; 63 Avenged (37 by a killed-grudge holder; 6 tagged "(killed)"). Fires, but rarely.

## 2. Word of a killing getting around, and someone acting on it: SHOWN

Diaries: `bereaved_30486 d21 00:01` the rumour arrives with its age ("deed from d20 22:56", hops 1, conf 1.00) and the
grudge forms the same minute; `d21 01:32` "told Sabine Reyes#39839: Killed by Flux Tanaka#72315 on Verity Kemp#37317";
`d21 02:04` the fact decays ("Killed by someone unnamed ... already known / dropped"); `d26 06:45` he is still telling it.
Same pattern at `bereaved_36837 d21 07:26 / 10:35` and `purist_12613 d24 20:04 / d25 20:58`.

A revenge killing travelling across diaries: "Avenged by Odessa Mbeki#103969 on Bertram Novak#26344" (deed d26 02:06)
reaches `builder_87052 d27 00:01` (hops 1, conf 0.60), is retold at `d27 08:08, 11:23, 17:35, 23:23` (once as "Avenged by
gang The Drowned Choir 8"), and reaches `raider_101991 d27 17:35` at hops 2, conf 0.58. `raider_11425` lists Bertram as a Friend.

Acting on it (events, the Odessa Mbeki arc): `36096 Cells` "came out to a changed life: the post as Sanitation at
Recycler#4486 gone; a grudge on Bertram Novak, who reported her" -> `36636 HuntStarted` -> `36756 Assault` / `Avenged ...
themself on Bertram Novak` -> `36854 Cells` "learned who put her inside: Bertram Novak reported her" -> `37441 Story`
"Kimura Feed: Odessa Mbeki avenged Bertram Novak". Also `38881`: fifteen grudges within 19 ticks of Joss Inchbald's death.

A hit heard of: `2520 Contract` "Lavinia Sato got word on the street: Dahlia Fairweather took the hit on Lavinia Sato"
then "put a guard on herself for 42c, Nutrix 6 paying". A target hears and buys a bodyguard. Not in a diary.

## 3. A hit as ordinary business (buyer, price, hitter, outcome): SHOWN

`fixer_26141.md` (Philippa Whitlock, Fixer#5897) plus the loser's diary `cultist_11723.md`:
- `cultist_11723 d23 00:07` "Assault: Adela Achebe (45 y) caught up with Wisp Sato (32 y), who fought back and won";
  `edge ... Wisp Sato#9095: Enemy affinity -0.90`. Adela is a Drowned Choir 2 member (`**Gang**` line).
- `fixer_26141 d23 06:05` "ContractPosted: Odile Fletcher posted a beat on Wisp Sato for 364 at Fixer#5897 (182 down,
  revenge), on Philippa Whitlock's book". Odile is the Fixer's place owner and (`cultist_11723 d22 21:54`) leader of
  The Drowned Choir 2: five hours after her member lost a fight, the leader buys the beating.
- `fixer_26141 d25 18:00` "ContractTaken: The Ash Widows took the beat on Wisp Sato (LIVE, crew 4), put on it with
  Ignatius Achebe, Zoe Stroud, Raven Voss, Rook Ironside".
- `fixer_26141 d25 22:28` "ContractFulfilled: beat on Wisp Sato: done by The Ash Widows (... crew hit Wisp Sato at
  Block#692: won (3 vs 3, 0 dead)); ... the cut 41c to Odile Fletcher" (she owns the Fixer, so she pays herself).
Buyer, price, down payment, motive, hitter, place, odds, outcome, cut. The best single read in V18. Failure path:
`marked_30280 d21 18:00` "The Drowned Choir 9 took the beat on Ulysses Kemp (LIVE, crew 4) ... by Varek Nakamura at
Fixer#5410", `d22 03:00` "failed (... crew found no Ulysses Kemp at Block#201)". `fixer_26141 d27 22:31` "beat on Hana
Tanaka: failed (the strike was lost) ... the escrow back to the buyer".

A hit that is beaten to it: `marked_42962` (Cipher Thorne, 38 y, Corp EXEC, courage 0.14): `d22 16:07` "ContractFailed: hit
... attempt 1 failed (the taker was taken off the street)", `d22 22:30` "Abducted: Cipher Ito abducted Cipher Thorne", `d23
00:34` "Harvested: ripped 2 implants" and "ContractFailed: hit on Cipher Thorne: cancelled (the target is dead)". The
mechanism is fine; the man is not: he has a hit on him, goes to work (`d22 16:00`), buys stims (`17:02`), and never hides,
hires or runs, where Lavinia Sato (story 2) did.

## 4. An open contract an NPC takes that a player could have taken: MECHANISM ONLY

The log: 1,890 ContractPosted, 1,499 ContractTaken (846 `LEDGER`, 589 `LIVE`, 64 `LIVE, crew`); 592 Fulfilled, 784 Failed.
Takes by kind: tag 650, steal 316, spin 224, locate 171, beat 57, hit 39, guard 26, abduct 9. Takers are named people and
crews ("Cordelia Mbeki took the beat on Hana Tanaka (LIVE), put on it by Philippa Whitlock at Fixer#5897", `fixer_26141
d23 18:00`; "Wisp Sato took the tag on Nerys Duarte (LIVE)", `34200`). Posts carry a price and a book ("Habitat 2 posted a
locate on Ulric Oakes for 32 at Fixer#5120, on Matthias Hallow's book"), so a player at the desk could be offered the same job.
Why not "shown": no diary has the taker's decision (no GOAL, no walk to the fixer, no read of the board). The take is an
event row at 18:00 or 00:01, the contract tick. 57 % are LEDGER corp abstractions: `fixer_99560` shows "Nutrix 4 took
the spin on Osric Oyelaran (LEDGER)" re-taken d25, d26, d27, with 20+ corps posting the same spin at 96 each.

## 5. A life that makes sense on inspection: SHOWN

`raider_101991` (Rosalind Moreau, 56 y, Dreg, Static Lanterns member):
- `d24 00:00` Loan "230c short of a vat-cancer treatment ... has no friend left to ask"; `d24 10:51` starving, soup line;
  `d25 16:45` "the pot at Church of the Open Circuit#5320 ran dry before Rosalind Moreau's turn", so a chapel 8 min away;
  JobSearch `d24 14:31, d25 11:43, d25 14:21` "nothing going".
- `d26 00:00` "moved into Block#343", `00:11` "hired as Sanitation" (work 6.5 h), `13:59` "stole 2 food from the Street
  Market#4236 (miserable, mood -0.47)" and "emptied the till of Cerys Chrome's stall (10 coins)", `14:55` "bought a remission
  of a vat-cancer at Ripperdoc#5235 (3c of it to the doctor; 299c on the doctor's slate)".
- `d27 00:07` "missed The Static Lanterns's muster", `06:06` "attacked Hollis Sato (59 y)", `07:00` reported, `20:12` answers
  the gang's summons to court, `22:17` arrested, `22:37` "learned who put her inside", sentenced 10 days.
Poverty, a cure bought on credit, a stolen till, a half-kept gang, a fight, a report, a cell: the reasons are on the page. Weak: Hollis Sato has no prior edge and the cause of the fight is not logged. Contrast `neglect_parent_76519`: two places every day, "net 0c" for seven days, a dull diary but not a contradiction. 
## Other checks and what to carry forward

- Minors: 0 of 389 Murder rows are under 18; no child in the Moth Wexford chain. The 6,277 "(a child taken)" grudges are
  adults grieving the Camp, not harm.
- Gaps: (a) hunt rate ~3 % of killed grudges, and Hunt cools after one failed plan, so for the old and poor a grudge never
  costs the killer; (b) contracts have no taker decision line; (c) a marked man does nothing about the contract on him,
  while a target who hears (Lavinia Sato) acts; (d) Witness rows (312,801) bury the story rows: one rampage by Liesl Draper
  (`38887`) made 60+ rows of one Assault; fold them.
- Collisions: Idris Hawthorne#14336 vs #6626 sit in `purist_12613`; "Zane Whitlock" is several people in the event log, so no Zane/Taz chain used.
