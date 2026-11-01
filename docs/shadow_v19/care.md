# Shadow V19 critic: care (ripperdoc, orderly, super, sick, teacher, school_parent, bereaved)

Window: 100k island, seed 42, d21-27, main 41c31b2. Diaries `target/v19/isle100k_s42_d21/`, cited `file dDD HH:MM`; lift/Turf lines from
`target/v19/events.tsv` (col 1 is the minute of the run, so 34565 = d24 00:05). Picks: ripperdoc_100471, _61891; orderly_21852, _25614;
super_105701, _85253; sick_67704, _94437; teacher_25077, _53078; school_parent_39265, _61182; bereaved_14813, _75821 (different people
from V18's, so compare by archetype). **No minor is harmed in any of the fourteen** (every `[event]` line with an age under 18 grepped: none;
no Custody line either).

## 1. Verdict
- More like people in a vertical city than V18: the tower is now a place with a lift that can be held, a flat on the 24th floor, a school
  on the ground floor of the same block, and a gang member who sleeps on floor 24 and rides a lift to extort a unit on floor 4.
- Grief finally has a mechanism and a payoff: a widowed guard takes his three days of leave, comes back, and goes after his wife's killer
  inside the cells he guards (V18's bereaved never acted). A 74-y bereaved man takes a locate contract in person.
- Still thin where V18 was: the lift is two numbers (1 min or 5 min, one caller), the supers walk "(1 floor)" six times a day, and no raid
  that climbs reaches any care diary (raids only as gossip).

## 2. The exit's checks in my group
- **(a) A life in one tower: seen, twice.** `teacher_53078` (Ottilie Greaves, 60 y): flat 4 on floor 24 and School#11136 on floor 0 of the
  same Block#2631. `d22 08:08 GoTo(Workplace) ... School#11136, floor 0 of Block#2631, 1 min, 3 tiles`, `d22 09:00 taught 25 pupils`,
  `d22 18:08 GoTo(Home) ... floor 24, 4 min` -> `18:09 CLIMB up by the lift in Block#2631: floor 0 to 24, 1 min waiting in the lobby, 3 min in
  all`; food from Street Market#6206 34 tiles off. Day summary d22: floor 24 587m, school 568m, street 243m, market 26m. `teacher_25077`
  (Knox Lockridge, 63 y) the same in Block#4768 (flat 1, floor 10, school on floor 0); a shakedown reaches him there (`d22 01:15 woke at
  Block#4768 to find 3c gone ... Cipher Bartlett shook the block down`, `02:02 told the law`). The supers live and work in their block too.
- **(b) A gang holding floors over a front, a raid that climbs: half.** Floors and lifts yes. `ripperdoc_61891` (Benedikt Fairweather, 28 y,
  joined The Glass Teeth `d24 02:01`) sleeps in Hideout#137368, floor 24 of Block#966, 377 min (`d24 19:53`), then `d25 02:12 GoTo(TargetHome)
  ... Block#135930, floor 4 of Block#4561, 47 min, 730 tiles`, `02:57 CLIMB up by the lift in Block#4561`, `03:20 Extortion ... 5 coins`,
  a fight, `FleeToHome`, a back-room patch (`04:55 ... 6c, no questions asked`), then a second Hideout on floor 8 of Block#2191 (`d25 06:56
  CLIMB up by the stairs ... 4 min`). A held lift: events.tsv `21605 The Unplugged 18 took the lift of Block#652: its men ride first,
  everyone else waits or takes the stairs`, and `orderly_21852` (lives Block#652 floor 21) shows it from below: `d21 00:01 CLIMB up by the
  lift ... 5 min waiting in the lobby, 7 min in all`, `d21 06:11 CLIMB down by the stairs ... floor 21 to 0, 6 min` (d21-d23, five up and five
  down), then `d24 06:47 up by the lift, 1 min waiting` after `34565 The Unplugged 18 let go of the lift of Block#652`. Also `d27 00:05 Turf:
  The Grey Lanterns let go of the lift of Block#912`. **No raid that climbs lands in a care diary** (raid hits are gossip only: `orderly_25614
  d21 16:20 Raided by Roxy Bartlett on Precinct#6407`); the Iron Knives raid up to floor 9 is in events.tsv (`30243`), not here.
- **(c) A Hideout leg past 4 h: none.** No `GoTo` over 60 min in any of the fourteen; the longest Hideout legs are 47 min
  (`ripperdoc_61891 d25 02:12`) and 45 min (`d24 02:03`). The long Mid-Levels <-> Sump commute of `ripperdoc_100471` is 33-34 min each way.
- **(d) Any minor harmed: none.** `school_parent_61182` moves her 14, 10 and 5-year-olds out of a gang war (`d21 00:05 Housed ... into
  Block#895`) and all three are fed and in or out of school by plain rules.

VISION's five (care group): **(1) a friend killed, a friend goes after the killer: seen, the best story here.** `bereaved_14813` (Joss Thorne,
37 y, Guard at Precinct#6407): `d21 00:00 GRUDGE against Rook Underhill (KilledKin, weight 0.99)`; `d21 08:00 Bereaved: stays home from the
shift ... mourning his wife Ilse Yardley (leave, 2 of 3)`, `d22 08:00 (3 of 3)`, `d23 08:00 Skipped ... past the employer's leave`; back at work
`d24`; `d25 18:02 GOAL Fight ... enemy near`, `18:10 PLAN Fight on Rook Underhill`, `18:25 Assault: Joss Thorne (37 y) attacked Rook Underhill
(23 y)` (the killer is held in the cells he guards), `21:11 Arrest: Zara Okafor arrested Joss Thorne`, `21:30 Sentence 10 days`, `d26 15:00 joined The
Unplugged 20` in the cells, `18:00 Liesl Garland came to the cells ... and told him the post as Guard is gone`. A player would follow this.
**(2) word of a killing acting: seen, half.** The same man hears and tells it (`d21 09:24 told Yuki Zhou: Killed by Rook Underhill`), and the
rumour thins to "someone unnamed" after two hops (`d22 23:56`); the word does not move anyone but him. **(3) a hit as business: not seen.**
**(4) a contract an NPC takes in person: seen.** `bereaved_75821` (Kuro Marlow, 74 y): `d26 09:11 GOAL Contract (score 0.864)`, `09:33
ContractTaken: came to Fixer#8487 in person and took the locate on Osric Pembroke for 42c`, `10:44 ContractFulfilled: locate ... no cut`, then a
second (`11:27 ... Razor Tanaka for 36c`), then `11:35 goal Contract cooled until d30`. **(5) a week that follows from its past: seen**
(bereaved_14813 above; `ripperdoc_100471` d22 `Succession: Mako Pembroke runs The Pale Hounds` -> `d23 00:07 Hex Wexford reported ... Shakedown` ->
`04:16 Sentence 6 days` -> `Philippa Whitlock runs ... while Mako sits inside`).

## 3. The vertical city at the player level
- **Reads like a place:** the `CLIMB` line is clear and the numbers are sane (21 floors by stairs 6 min, 24 by lift 3 min, 40 by lift 4 min,
  a super's 1-3 floor walk 1-2 min). Flats are real: floors 3, 10, 21, 24, 26, 29, 34, 40, 41 in the lines; schools, clinics, shops on floor 0
  of a host; Hideouts on floors 8 and 24; "27 households under the roof" and a shared pantry (`school_parent_39265 d21 09:30`). Nothing
  absurd by height: no body on a floor the block lacks, no climb of hours.
- **The lift is the thin part.** Of ~1,990 `waiting in the lobby` lines in the whole directory, 1,907 are "1 min waiting (1 called the car)"
  and 24 are "5 min (1 called)"; 51 have 2+ callers and one has 15. So the rush-hour queue is nearly invisible: `sick_94437` rides floor 0 to 40
  23 times, always 1 min wait, 4 min in all; `school_parent_39265 d23 18:09` is 1 min with 3 callers. The only visible drama is the held lift.
- **The 5-min waits that no hold explains.** `ripperdoc_61891 d25 19:54` and `d27 13:19` wait 5 min in Block#966 with 1 caller, but events.tsv
  shows no hold on Block#966 then (Glass Teeth let go `33125` = d23 00:05; Black Hounds Splinter 169 held `34565`-`36005` = d24 00:05 to d25
  00:05, when a Glass Teeth man waited only 1 min, `d24 19:50`). The wait looks reversed or unexplained; a member of the gang that "rides first"
  should not queue 5 min for its own Hideout.
- **Down by the stairs is the held-lift tell**, but the line does not say why (no "the lift was held by The Unplugged 18"). A player sees only
  "5 min waiting" then "stairs"; add the reason to the `CLIMB` line.
- **A lift complaint in a three-storey block:** `super_105701 d22 16:15 Maren Singh stopped Rhys Sallow in the hall of Block#258 about the lift`;
  the diary has no lift ride in Block#258 (floors 1-3, stairs only). Small, but it breaks the place.

## 4. Per archetype (V18's word in brackets)
- **Ripperdoc: alive (alive).** `ripperdoc_100471` (Mako Pembroke, 26 y, flat 6 floor 10, clinic floor 0 of Block#808): 13 implant/cure/patch
  lines in two days (`d21 13:51 Cured: Kieran Mbeki ... vat-cancer (3c to the doctor; 299c on the doctor's slate)`), a Pale Hounds leader
  (`Called ... to the Hideout`, `held court`), then six days in the cells for a Shakedown with no clinic, no climb. `ripperdoc_61891`: gang
  soldier story above; his clinic days are quiet (Therapy/Detox lines).
- **Orderly: alive (alive).** `orderly_21852` (Imogen Blackwood, 22 y): night shift from floor 21; the held-lift week; joins The Grey
  Lanterns `d25 16:18`, deals `16:43`, reported by a bystander, 5 days at `17:08` (a 24-minute road, as V18's). `orderly_25614`: 34-floor
  commute, 13 lift rides each way, only 7 Treated lines; a gang tryout "turned away: full up" (`d26 18:38`).
- **Super: thin (alive).** `super_85253` still has named fixes (`d21 11:14 fixed a leak in Amara Reyes's flat`) and a visiting parent; but from
  `d22` on every line is "walked the stairs and halls of Block#3993 (1 floor)", five or six a day; `super_105701` (3 floors) the same,
  plus one named leak fix in a week. The tenants' complaints are fine (rent, smell, lift); the repetition is the problem.
- **Sick: thin (thin).** `sick_67704` (66 y, flat 5 floor 26, vat cancer 0.40): `d22 00:00 Fire: let go: too sick to work` then `00:11 Hire: hired as
  Vat Tech (picked by Sana Kimura)` at another farm, still sick; begs `d23 00:00` (237c short), then `d23 10:35 Cured ... 298c on the doctor's
  slate`, steals 2 food `12:33`. `sick_94437` (53 y, floor 40): the same three boards daily (`09:50 Street Market#6110, 10:52 #6107, 11:52 #6109:
  nothing going` d21, d23, d24), `d25 12:45` finds a Vat Tech post, `13:28 hired`, `14:15 Cured ... 285c on the doctor's slate`. The
  slate cure is new and good; the daily board loop is V18's loop again.
- **Teacher: alive (alive).** `teacher_25077` pupils climb 5, 6, 7, 8, 9 over the week (`d21-d25 09:00 taught N pupils`); `teacher_53078` taught
  25. Both end near 0c; `teacher_25077 d27 00:00 RentShort: could not pay the rent on Block#4768 to Habitat 20 (0 of 1)`. The teacher's shift
  actions are `ClerkWork` at a "Station" (a label leaking from the clerk job; `teacher_53078 d22 09:02`).
- **School_parent: alive (alive).** `school_parent_61182` moves three children out of a gang war (stairs/lift lines at both ends; her Spire flat on
  floor 41 left behind at `d21 00:13`), kids at School 1 min away; best life in the group. `school_parent_39265` (Builder, flat 2 floor 29, one
  8-y child): school 4 of 5 days, home `d21` over the 2c fee.
- **Bereaved: alive (thin).** The best archetype of the window: mourning leave and revenge (`bereaved_14813`), a contract (`bereaved_75821`), though
  `bereaved_75821`'s funeral still reads `d21 20:00 funeral of Lorcan Moreau at Block#1166: Kuro Marlow there; no friend near to ask`.

## 5. Bugs, ranked by what the player sees first
1. **Idle-loop supers.** `super_85253 d22-d27`, `super_105701 d21-d27` repeat "walked the stairs and halls (1 floor)" 5-6 times a day with no
   outcome. V18 super: alive, now thin. New (a C7 consequence: the stairs round is logged per pass).
2. **A lift wait of 5 min with one caller and no reason**, and a gang man waiting for his own gang's lift. `ripperdoc_61891 d25 19:54`, `d27
   13:19`; see section 3. New. Likely the lift model's cut/held flag is read for the climber's own gang wrongly, or a 5-min "cut" with no event.
3. **The held lift is not named in the CLIMB line.** `orderly_21852 d21 06:11` stairs down 21 floors with no cause. New (narration only).
4. **Fired and rehired in 11 minutes while sick.** `sick_67704 d22 00:00-00:11`. New; plausibly farm to farm, but a "too sick to work" woman is hired
   by the next foreman at once.
5. **A 3-storey walk-up with a lift complaint.** `super_105701 d22 16:15`. New.
6. **Guard sees every cell fight.** `bereaved_14813` has Witness x344 lines in a week, and the precinct adds sentence days in a clump of seven at
   `d25 15:00`. It buries the grief story in the log. New (tool/print noise more than sim).
7. **Cells still full:** 3 of 14 care picks in the cells (V18: 4): `ripperdoc_100471 d23 04:16`, `orderly_21852 d25 17:08`, `bereaved_14813 d25 21:30`;
   yard line reads "alone: 5689 in the cells" (V18 6266). **Better.**
8. **No School for a 2c fee:** `school_parent_39265 d21 09:30` and `school_parent_61182 d21 09:30` (two kids, fee beyond the purse at midnight); the
   child is in School by d22-d25. **Better** (one day lost, not five). `d26-d27` "no School today" is the rest day; fine.

V18 bugs in my group: 1 cells **better** (3/14, 5689); 2 wrong killer **not seen**; 3 Eat after StockGone **not seen** (no starvation; lowest hunger
reads fine in all 14, `teacher_53078 d24 05:18` is a failed body strip, StockGone, not hunger); 4 doctor to cell in 24 min **not seen** (a slower
slide, `ripperdoc_61891`, 3 days to GangJoin); 5 children unfed 2 days **not seen**; 6 2c fee **better**; 7 camera JUMP audit **unchanged**
(`school_parent_61182 d26 02:53`, 21 tiles in 1 min, 12 lines; `orderly_25614` 5); 8 "no friend near to ask" **unchanged** (`bereaved_75821 d21
20:00`); mourning (V17/V18 not seen) **fixed**; near-loop sick **unchanged** (`sick_94437` board loop).

## 6. Dull spots and too much
- **Dull:** the supers (bug 1); `sick_94437`'s three boards; `teacher_25077` idle 10.5h on d27.
- **Too much:** Witness x344 for a guard; 13 lift rides each way in a week for `orderly_25614`; her clinic is quiet (7 Treated vs V18's 21).
