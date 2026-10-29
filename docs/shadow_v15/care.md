# Shadow V15 critic: care (ripperdoc, orderly, super, sick, teacher, school_parent)

Window: 100k island, seed 42, d21-27, main bf63dee. Diaries `target/shadow_v15/isle100k_s42_d21/`, cited `file dDD HH:MM`.
Twelve picks (indices differ from V14's, same archetypes, not the same people): ripperdoc_73508, ripperdoc_9880,
orderly_13875, orderly_21547, super_106169, super_97910, sick_50158, sick_65180, teacher_13152, teacher_49124,
school_parent_48165, school_parent_86768. Counters: `target/r6_100k/analyze.txt` against round 5's `target/r5_100k/`.
**No minor is hurt, in vice, a victim or dead in any of the twelve** (every event line with an age under 18 grepped: only
`[family]` lines at School, home or the Camp). A 6-year-old in Work Camp#5132 is fed and untouched (`sick_50158 d21 09:30`).

## 1. Verdict

- Better on the people scale: one teacher's room is full (7-9 pupils, V14 had 1-2), a sick man actually reaches a soup
  bowl, and a super is thrown out of a strip club, reported, arrested and given 10 days: a story that starts and is picked
  up by the law. The clinics are still two-of-four, as in V14.
- Round 6's V6 (empty rooms beside full ones) did not show in this group: no night Clinic patient, no spill, no on-call
  line in any of the twelve. The counters say Treated rose 12.9k -> 16.4k and Installed 3.9k -> 5.8k, so it moved at city
  scale; the two ripperdoc picks are still one near-empty post and one empty post.
- The school number went the wrong way at 100k (school days 15.2 % of child-days against 18.0 % in round 5; unpaid
  47.0k -> 50.2k days), and a fee excuse on a purse that holds the fee is the one thread a school_parent had (bug 3).

## 2. Per archetype (V14 word in brackets)

- **Ripperdoc: thin, one dead (V14 alive / dead).** `ripperdoc_73508` (66 y) treats twice in the week (`d23 10:45 Treated:
  Rafe Fairweather patched Kuro Ashcombe up at Ripperdoc#5628 for 12c (3c of it to the doctor)`, `d23 11:04` Hiro Thorne),
  8.8 h of ClerkWork a day otherwise. His story is the gang: `d21 20:15 GangJoin: joined The Glass Wolves`, `d22 00:07
  Crew: chose the shift as Ripperdoc over The Glass Wolves's court summons ... (33% the gang's way)`, `d24 00:07 Skipped:
  skips the shift as Ripperdoc for The Glass Wolves's court summons`, `d24 21:00 Taz Ramirez held court at Hideout#4569:
  Kofi Pembroke, Rafe Fairweather, Knox Northcott came`. A doctor torn between clinic and gang is the best line in the
  group. `ripperdoc_9880` (31 y, `counts: none`): 8.8 h a day for seven days at Ripperdoc#5835, GoTo(Station) > ClerkWork
  in 45-75 min bites (`d21 09:05-14:05`), net +1 to +5c, no patient, no install. Dead.
- **Orderly: alive in one, thin in one (V14 alive / dead).** `orderly_21547` (68 y): 36 Installed and 4 Treated, 09:00-17:00
  only (`d21 09:00 Installed: Rafe Ravensworth had Legs implant T1 installed at Ripperdoc#5042 by Cerys Dunmore`), plus
  `d23 00:11 Hire: Alaric Blackwood hired as Ripperdoc (picked by Cerys Dunmore, the foreman)`. `orderly_13875`: 8 events,
  mostly chrome (`d23 14:27 / 14:40`, `d24 11:33 / 11:34 Installed ... by Sana Abernathy`) and one Detox (`d22 13:06`).
- **Super: alive (alive).** `super_106169`: 24 Served, Notice growing 4 -> 3 -> 11 -> 12 (`d24 11:07 served notice on 12
  tenants in arrears on the round of 6 Blocks`), repairs by name (`d22 12:57 fixed the wiring in Bram Bellamy's flat`), then
  the break: `d25 19:38 Served: Vale Upton got rough with Zara Kettering at Strip Club#5718; Ulysses Marlow threw Vale Upton
  out, barred from the door`, `d25 20:15 Arrest`, `d25 21:21 Sentence: 10 days`; she is gone from the Block for the rest of
  the window. `super_97910` (19 y): 22 Served, 3 Notice, steady, then `d27 13:44 -10c (Stims)` to The Glass Dogs after
  witnessing Dealing twice (`d27 13:34`, `16:28`).
- **Sick: alive (alive).** `sick_50158` (72 y, cancer 0.53): `d23 00:00 Loan: 232c short of a vat-cancer treatment ... has
  no friend left to ask`, the soup line walked 1 h 20 (`d23 02:43`), the bowl (`d23 05:31 ate a Mission's SYNTH meal`), a
  6-year-old in the Camp he cannot reach (`d26 00:07 Visited: could not make the trip to Nerys Ambrose at Work Camp#5132
  this week: 22 h 22 from home`), then `d27 04:04 Theft: stole 2 food`. `sick_65180` (65 y, cancer 0.34): theft, GangJoin
  (`d21 15:41 joined The Wire Wolves (miserable, mood -0.42)`), 3 days in, the board read six times with `nothing going`,
  second theft, three Assaults on guards, second arrest, and a Marriage inside it (bug 4).
- **Teacher: alive at one, thin at one (V14 thin).** `teacher_13152`: `d21 09:00 Served: taught 7 pupils at School#5992`, 7,
  9, 9, 9, and the door memory `met Juniper Singh at the door (the parent of Kieran Dunmore, a pupil)`. `teacher_49124`: 4,
  2, 1, 2, 2 pupils at School#5432 (`d23 09:00 taught 1 pupil`); he is also shaken down (`d24 00:05 -1c (Coerced)` to The
  Static Rats, `d24 03:23 -5c Stims`).
- **School_parent: flat (flat).** `school_parent_86768`: Zara (8 y) at School#4773 every day, 30 min walk, pantry 28-33
  with "1-2 of it ours (40 households under the roof)". `school_parent_48165`: two kids, nights at the Vat Farm (9.7 h), net
  0 to +5c. One thread: bug 3.

## 3. Round 6's mechanisms seen at 100k

- **Seen V1 (the Block shelf split by household):** `school_parent_48165 d21 09:30 the pantry holds 41, 3 of it ours (30
  households under the roof)`, `school_parent_86768 ... 29, 2 of it ours (40 households)`. Counters: `ch_street` 249 -> 69 a
  day, `child_begged` 3986 -> 1629, `child_thefts` 87 -> 40, `ch_camp` 119 -> 1188 a day (the Camp holds ten times the
  children), `ch_warned_old` 72 a day, `street_takes` 37 -> 30.
- **Seen V3 partly:** an assault reported and the arrest by the reporter (`super_106169 d25 19:38 Report` -> `20:15 Arrest`);
  the kept door throws the aggressor out. No body, cry-out or downed line in the group.
- **Seen V6 at city level only:** `door_refusals` 4461 -> 7113, Treated +27 %, `patient` encounters per staffed shift 0.090 ->
  0.107. In the twelve: zero night Clinic lines, zero spill lines.
- **Seen V2 a little:** `ripperdoc_73508` is a gang member with a court and a skipped shift; `sick_65180` joins a gang; two
  protection and stim payments (`teacher_49124 d24 00:05`, `super_97910 d27 13:44`). No turf, corner or war line.
- **Expected, never seen:** a Clinic opening at night for a patch-up (both orderlies are 09:00-18:00 and home by `orderly_13875
  d21 18:10 Fare`), the spill to a free doctor, the on-call spread; V4's cab (none of the twelve ride); V3's `found the
  body`; V1's warning, fetch or take (no child slipped out).

## 4. Bugs, ranked by what the player sees first

1. **A ripperdoc sits eight hours in an empty clinic, every day.** `ripperdoc_9880` d21-26 8.8 h work, `counts: none`; a
   second doctor with 2 patients in a week (`ripperdoc_73508`). V14 bug 1 (empty clinic): **unchanged** at these two posts.
2. **The installer is the orderly.** `orderly_21547`: all 36 Installed read "by Cerys Dunmore", an orderly, while the Ripperdoc
   post is vacant until `d23 00:11 Hire: ... picked by Cerys Dunmore, the foreman` (an orderly as foreman), and no coin reaches
   her (net +8c a day, wage 5/day). Either the credit names the wrong person or an orderly cuts chrome. Same for
   `orderly_13875` (Sana Abernathy "installed" 6). New.
3. **A School fee excuse on a purse that holds the fee.** `school_parent_48165 d22 09:30 Mateo (7 y) at home (play: the 2c
   fee at School#5332 is beyond the purse)`, but wallet 10c and the d22 00:08 flow took one fee and left 4c; he is back
   in School d23. The excuse reads a stale purse. V14 bug 5: **unchanged**; `school_unpaid` 50,240 of 337,500 child-days is
   the biggest bucket, above `school_far` 30,430.
4. **A wedding in a cell.** `sick_65180 d26 21:40 Marriage: Piers Achebe married Nico Draper`, five hours after `d26 16:39
   JAILED ... until d29 16:39` at Station#5903; no scene or vows, then `d27 04:00 Transferred`. New.
5. **Teacher rolls swing 9 against 1.** `teacher_49124 d23 09:00 taught 1 pupil` against `teacher_13152` 9. V14 bug 3: **better**
   (one of two posts is healthy), but `pupil` per staffed shift 0.50 -> 0.41 and school days 15.2 % say worse overall.
6. **The aggressor tips her victim.** `super_106169 d25 18:07 Enjoy @ street ... 91 min (ended)` though the `19:38` Served line
   has her thrown out; she then logs `memory Enjoyed (+0.30)` and `Tip 1c` to Zara Kettering, the dancer she assaulted. New.
7. **Notices still land nowhere.** `super_106169` names 4 + 3 + 11 + 12 tenants (Dagny Ambrose, Lorcan Duarte repeat on d23 and
   d24) with no eviction by the super; the 11,269 city evictions are all corp-side. V14 bug 5: **unchanged**.
8. **Witness lines are noise.** `sick_50158 d27 04:04 WitnessReacted: 3 told 5 of their people`, `ripperdoc_73508 d24 18:38
   told 2 of his people`. V14 bug 6: **unchanged**.
9. **The soup walk that is cut.** `sick_50158 d25 01:38` sets out on an 80 min walk, `cut at street, 80 min (ended)` a minute
   later, then 4 h of Scavenge and no bowl that night. Reads as a cancelled mission. New, low.

V14 care bugs: 1 empty clinic **unchanged**; 2 struck-off doc idle **not seen**; 3 empty classroom **better**; 4 old sick with
no help **better** (the soup line fed `sick_50158` on d21, d23 and d26 `02:23`); 5 notices **unchanged**; 6 witness noise
**unchanged**.

## 5. Dull spots, ranked (mechanisms)

1. **The empty doctor.** `ripperdoc_9880` is a week of nothing. Mechanism: with no booking, the doctor takes a walk-in at the
   block door or the street's wounded (the night-open exists; nothing hands this clinic a body), or a nearby gang fronts a
   patient to an idle clinic.
2. **The comfortable school_parent.** Two flat weeks. Mechanism: the teacher calling about a truant when the excuse fires (bug
   3's own thread), a child falling sick, a Block pantry running short.
3. **The orderly's chrome line.** Mechanism: pay the doctor of record by the install and credit the right post; an orderly who
   stitches the street's wounded for coin would make the 12c patch-up a character beat.
4. **The super's notice.** Mechanism: the noticed tenant on the stairs, the super at the door with a guard at the eviction.
5. **Sick people cut off from kin.** `sick_50158`'s 6-year-old is 22 h away in the Camp. Mechanism: the warden brings the child
   to the soup line as a visit, or a neighbour carries word; today the elder can neither visit nor be visited.
6. **The teacher at 1 pupil.** Mechanism: pupils pooled to the nearer School by head-count, or the teacher takes the Camp's
   children for a lesson.

## 6. Too much

- `door_scenes` fell 935 -> 28 while `door_refusals` rose 4461 -> 7113 (and `door barred` 83 -> 0): the door turns people
  away almost silently. Flatter, not broken: the refused are counted, not shown.
- `sick_65180` is dense but each beat causes the next; the three guard assaults inside 90 min (`d26 14:00 / 14:11 / 15:31`)
  are the part that reads as a loop.
- `orderly_21547` 36 Installed in a week (V14's doctor had 37 events, same extreme): a chrome factory with the doctor slot
  filled by a midnight Hire. Watch it.
- Nothing monotone-war: no Turf, corner or truce line in any of the twelve.
