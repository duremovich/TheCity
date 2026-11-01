# Shadow V19, stories group (all 118 diaries, 100k seed 42, d21-27, main 41c31b2)

Cites: `file dDD HH:MM` in `target/v19/isle100k_s42_d21/`; bare ticks from `target/v19/events.tsv`. No side-by-side with V18 was done.

## 1. Verdict
- The vertical city now makes stories a player could follow end to end: a landlord posts a beating on a gang leader over
  floor rent and the Precinct finishes the job; a widowed guard goes after his wife's killer inside the Precinct.
- A raid that climbs lands as an event line in a raider's diary, but his body never climbs: he brawls in the street for
  29 min while the event says his crew went up to floor 28.
- Two contradictions break good stories: a jailed woman is listed as welcoming gang members back and sharing a crew day
  off, and a killer is sentenced only for Dealing.

## 2. The exit's checks (stories cut)
(a) A life in one tower: found, plentiful. 11 of 118 picks have home, post and evening in one Block (camp_warden_76587,
cook_25003, cultist_18786, dancer_44300, night_shift_26883, office_worker_24498, owner_broke_21221, raider_24528,
runner_23489, teacher_25077, teacher_53078). Best: `office_worker_24498` d22. Sleeps on floor 27 of Block#1457
(floors 0-55); 08:19 lift down to the Corp HQ on floor 0 ("1 min waiting in the lobby ... 3 min in all"); ClerkWork
09:03-18:00 with a 29 min Break on the street at 14:29; 18:06 "CLIMB up by the lift ... floor 0 to 27". Reads like
someone living over the shop. But the evening is four straight hangouts on floor 27, "3 other(s) there: 3 stranger(s)
[0 known]" (d22 18:09, 19:12, 20:15, 21:18): the same three strangers, never acquaintances. The tower is a place; its
neighbours are not yet people.

(b) A gang holding floors over a front, and a raid that climbs and lands: found, with a gap.
- Floors held: `fabber_46993` d22 00:05 "The Wire Wolves took floor 1 of Block#1129 over Street Market#6154 as a Hideout
  in D4 Mid-Levels (its turf there): 120 key money to Habitat 9"; `foreman_61872` d27 00:05, the Static Widows, same shape.
- The raid: `cultist_112804` d25 22:29 "The Drowned Choir 33's raiders (15) went up the stairs of Block#1000 to the
  Unplugged 4 Hideout on floor 28 (the lift held by The Unplugged 4), 14 min up"; 22:43 "Raid on the Unplugged 4 Hideout:
  The Drowned Choir 33 won (15 raiders, 1 defenders); took 106c ... 2 hurt: Knox Greaves, Taz Abernathy". He is a
  raider by the book: 19:14 GOAL Raid, Muster 156 min, GoTo(RaidTarget); 22:45 he walks home to floor 29 of Block#1144.
  The gap: 22:14 "ACTION Brawl @ street (D4 Mid-Levels), 29 min", no CLIMB, never on floor 28; his PLAN named
  Hideout#136884 but the event raids another Hideout in Block#1000. The climb is gang-level narration. No `ACTION Brawl` in
  any diary carries a floor.
- City-wide the log has 42 "went up the stairs" raids (14 "lift cut", 15 "lift held", 10 "no lift"; the rest not
  classified), so the mechanism fires at volume.

(c) A Hideout leg past 4 h: none. `camera.tsv` `goto_over_120` is 0 over 816 pick-days; the longest climb in any diary
is 7 min in all, the longest lobby wait 5 min.

(d) A minor harmed: none seen. I scanned every harm-type `[event]` line in all 118 diaries for ages under 18: 3 hits, all
`guard_16925` (d22 18:00, d24 18:00), e.g. "the law added 1 days to Taz Mbeki (58 y)'s sentence for the Camp Escape on
Freya Mbeki (15 y)" (also Roxy Garland 12 y, Vale Oyelaran 2 y). The children are the subject of an adult's offence, not
harmed; a "Camp Escape" on a 2-year-old reads odd and deserves one look.

## 3. The exit's city stories and VISION's five

**A landlord against a gang (found, whole chain):** `gang_leader_113134`, Raven Ashcombe of The Red Teeth.
- d24 00:00 "Floor: Habitat 28 hired muscle through Fixer#11905: a beating for Raven Ashcombe of The Red Teeth, who runs
  floor 8 of Block#3202 and will not pay the floor rent (3c owed; 419c on the board)". Same minute: Habitat 5 "sent the
  Precinct up to floor 42 of Block#573: The Red Teeth will not pay the floor rent (6c owed); 2 found up there and reported".
- d24 09:39 Smoke Sato "came to Fixer#11905 in person and took the beat ... (LIVE, crew ...)"; 22:14 attempt 1 failed (the
  crew is gone); d25 09:50 Quill Park takes it with a new crew; d26 00:23 Hilde Haas hits her at Block#65 and loses,
  "escrow back to the buyer"; d26 03:21 she beats off guard Hesper Carrow (19 y), 03:32 arrested at home for Smuggling,
  04:10 8 days, Succession: Cass Mbeki runs the Red Teeth. The rumour reaches strangers: `cabbie_86282` d27 00:01
  "heard a rumour: Hired by corp Habitat 28 on Raven Ashcombe (deed from d24 09:39)".
- Reads as a story a player would follow. Holes: she is a "gang leader" with an Office Worker post (Corp HQ, Block#94)
  who scavenges bins ("nothing worth selling" d22 07:03, d23 20:07, d24 03:12, d25 06:53); she lives on floor 2 of
  Block#65 and never goes near floor 8 or 42, so the tenancy pressure is only text and gossip.

**A life in one tower:** (a) above. **A founding into a unit:** (b) above, one event line, no member walks into the new floor.

**VISION 1, kin killed and the one who goes after the killer (found, strong):** `bereaved_14813`, Joss Thorne (37 y), Guard.
- d21 00:00 GRUDGE against Rook Underhill (KilledKin, 0.99); d21 08:00 and d22 08:00 "stays home from the shift ...
  mourning his wife Ilse Yardley (leave, 2 of 3 / 3 of 3)"; d23 08:00 skips the shift past the leave.
- d25 17:41 GuardJail at the Precinct; 18:02 "GOAL Fight (was Work) ... enemy near"; 18:10 PLAN Fight on Rook Underhill;
  18:25 "attacked Rook Underhill (23 y)"; 21:11 arrested by Zara Okafor; 21:30 10 days.
- d27 15:00 in the yard: "went for Rook Underhill (a killing to answer); Rook Underhill won", +3 days.
- Defects: tick 39780 logs "Joss Thorne avenged a killing on Rook Underhill" and tick 40321 a Story line "avenged Rook
  Underhill" on a fight he lost; 272 witnesses "saw" the yard fight. Second, partial: `pimp_24236` Rue Carrow, d21 00:00
  GRUDGE against Pixel Achebe (KilledFriend, 0.58), but she is in the cells till d28 (d22 04:32) and never acts; someone
  else avenges the friend in the Precinct (tick 34020, Cordelia Oyelaran).

**VISION 2, word of a killing acting on someone (found, thin):** `joytoy_72363`, Benedikt Fletcher (27 y), d22 04:00
"caught in the crossfire at the corner by Noodle Bar#137284 and died" (Yorick Haas); "Bereaved: Mateo Achebe lost his
husband: mourns 5 days (through d27)"; 04:05 body found; 04:24 stripped (14 coins); 07:12 "Killed by Yorick Haas ... told
12 times today"; 12:54 buried. The word travels; no one in these diaries changes a plan on it. Yorick Haas is arrested at
tick 31731 and "sentenced to 5 days for Dealing" (31755), the killing not in it.

**VISION 3, a hit as business (found):** `marked_77516`, Ottilie Ironside (54 y). d22 04:00 "hit ... attempt 1 failed (the
crew never reached the door)"; d22 09:46 "Hilde Carrow came to Fixer#9348 in person and took the hit on Ottilie Ironside
for 1625c for The Glass Teeth (LIVE, crew Zed Delacroix, Nico Tanaka, Static Brandt)"; d24 22:00 "StrikeDeclined: The
Glass Teeth called off the strike ... in D2 Civic Core"; escrow back to the buyer. The target sat in the Precinct cells,
so the crew declined: good economics. Caveat: `buyer_81009` d22 00:00 posts a 339c beating over 3c of floor rent (the
Red Teeth one is 419c over 3c); the price is out of scale with the debt.

**VISION 4, a contract an NPC takes in person (found, clean):** `bereaved_75821`, Kuro Marlow (74 y, no job). d26 09:11
GOAL Contract ("work on a fixer's board"); 09:12 CLIMB down by the stairs; 09:33 "came to Fixer#8487 in person and took
the locate on Osric Pembroke for 42c (LIVE)"; 09:37 back to Vend; 10:35 PLAN Contract on Osric Pembroke (Intel, StakeOut);
10:44 "ContractFulfilled ... no cut"; 11:27 takes a second locate. Also `pimp_24236` d22 09:23 / d23 10:08: takers of the
locate on Rue Carrow, the first dies ("attempt 1 failed (the taker died)", d23 09:47).

**VISION 5, a week that follows from its past (found, strong):** `marked_77516` d26 00:29 "released ... came out to a
changed life: the post as Sanitation at Recycler#6408 gone; a grudge on Kenji Coldwater ...; the Home at Block#1289 still
hers; The Pale Hounds still hers". 03:40 welcomed back to the Hounds; 06:00 starts a brawl with Vex Dunmore ("miserable
-0.33 and drinking"); 08:14 steals a motorcycle; 10:17 hired as Club Staff; 10:24 arrested; 10:56 6 days. Also
`arms_dealer_88945` Quill Oakes: d21 00:01 "Habitat 30 evicted Quill Oakes", homeless in a floor-7 Hideout working the
Fight Pit; d21 08:43 strips a corpse; d22 00:09 takes his gang's abduct order on Sabine Lindqvist; d22 02:45 "went down,
bleeding", 02:56 Dagger Northcott binds the wound, 03:32 carries him to Back Room#2796.
## 4. Bugs seen while hunting stories (most visible first)
1. **Jailed and in two places** (`marked_77516`): arrested d26 10:24, yet 10:28 and 11:59 "welcomed back to The Pale
   Hounds at Hideout#6423 by ... Ottilie Ironside", and 20:13 / 20:18 "Crew: spent a day off with the crew at Hideout#6423:
   ... Ottilie Ironside there", while the 15:00 Cells line has her in the yard "(in for Grand Theft, out d32)".
2. **A raider's body does not climb** (`cultist_112804` d25 22:14-22:43): floor 28 in the event, street in the diary, a
   different Hideout in the plan.
3. **"Avenged" on a loss** (`bereaved_14813` d27 15:00; ticks 39780, 40321).
4. **A killer sentenced for something else** (tick 31755 after `joytoy_72363` d22 04:00).
5. **Gang leader with a day job scavenging bins** (`gang_leader_113134`), "runs floor 8 of Block#3202" yet lives at Block#65.
6. **Hit price versus debt:** 339c and 419c boards for 3c owed (`buyer_81009` d22 00:00; `gang_leader_113134` d24 00:00).
7. **One hit, five bystanders dead** (tick 31920, Osric Vance at Street Market#6214; ticks 34800 and 36480 do 2-3):
   a corner this deadly is too much more than a story. All victims are 18 or over (the youngest is Hiro Volkov, 18, tick 31920).
8. **Teleport after a rescue** (`arms_dealer_88945` d22 03:01): "JUMP (a Full body) 40 tiles in 25 min: Block#2382 (D3
   Neon Quarter) -> Back Room#2796 (D1 The Sump)", the carry across two districts; max_jump in `camera.tsv` reaches 279.
## 5. Vertical-city notes
- 3745 `[climb]` lines; 1931 are "1 called the car, this call too", 44 are 2; the highest are 15 (`loader_82044` d21 00:01,
  floor 0 to 30, 3 min in all), 5 (`purist_59070` d22 22:02), 4 (`school_parent_39265` d26 18:08). The highest floor
  climbed is 48; no lift never comes. No rush-hour queue shows at 08:00 or 18:00: the lift is a 1-3 min tax, not yet a place.
## 6. Dull spots and too-much
- The Precinct raids at d26 22:22-22:43: four crews of 4-11 against 305-319 defenders, up to 17 hurt, each "next:
  BreakOut"; the same order repeating, nobody could win.
- Many diaries are a shift loop with no event; the good ones are those where a contract, a grudge or a sentence arrives.
  Eight diaries carry an in-person contract.
