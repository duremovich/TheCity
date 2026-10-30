# Shadow V17 critic: care (ripperdoc, orderly, super, sick, teacher, school_parent, bereaved)

Window: 100k island, seed 42, d21-27, main at `target/shadow_v17/sha.txt`. Diaries `target/shadow_v17/isle100k_s42_d21/`,
cited `file dDD HH:MM`. Fourteen picks: ripperdoc_14357, ripperdoc_64026 (same man and post as V16), orderly_13875, orderly_59744,
super_105720, super_98326, sick_103625, sick_49012, teacher_14781, teacher_50799, school_parent_47629, school_parent_86164,
bereaved_6559, bereaved_74870. **No minor is hurt, fined, in vice, a combatant, jailed, courted or dead in any of the fourteen**
(every `[event]` line with an age under 18 grepped: none outside `Bereaved`/`ChildTaken`).

## 1. Verdict

- Better than V16 where the new mechanisms land: a death in the house now has leave, a funeral and a children's line, and the
  promise to tell the law is a decision with an end (told within hours, or "the law is out of reach"). `bereaved_6559` is the best
  single care story yet: a guard's wife dies, the house runs dry, she steals, is jailed, the service takes her children.
- But grief does not stop the job: the same widow, on leave, goes out arresting people (bug 1). The clinics are as empty as V16
  (`ripperdoc_64026`: one patient in seven days at the same post). Supers and teachers are better; the sick still sit idle.
- Care people no longer steal motorcycles (V16 bug 2 not seen). A man with cancer is hired as a guard (bug 4).

## 2. Per archetype (V16 word in brackets)

- **Ripperdoc: thin / dead (V16 alive / dead).** `ripperdoc_14357` (59 y, gang dealer): court summons skip (`d21 00:07 Skipped`),
  arrest `d21 21:28`, five days, then `d26 22:46 Cells: came out to a changed life: the post as Ripperdoc at Ripperdoc#6019 gone; a
  grudge on Mei Upton, who reported her; the Home at Block#3426 still hers; The Unplugged 4 still hers`. Alive, but as a prisoner, not
  a doctor. `ripperdoc_64026` (26 y, Ripperdoc#5835): dead, **unchanged**: 60 `ClerkWork` blocks, 8.8 h a day, net +2..+7c, and the
  week's only `d25 17:49 Treated: Rook Blackwood patched Nova Zhou up ... for 12c (3c of it to the doctor)`.
- **Orderly: alive / thin (V16 alive / dead).** `orderly_59744` (24 y, night): six patch-ups at 12c plus the full promise: `d23 10:09
  WitnessReacted: paid Xia Dunmore's shakedown at Block#449 (3c); he means to tell the law` -> `d23 13:22 Report: went to Station#4891
  and told the law`; again `d25 08:09` -> `11:22`; `d26 16:26 Cells: Xia Dunmore learned who put her inside: Vex Inchbald reported
  her`. `orderly_13875` (43 y, day): four visits, all Therapy/Detox (`d24 10:29` and `11:31`, the same man twice in an hour), no patch-up.
- **Super: alive / alive (V16 alive / dead).** The one-floor loop is gone: `super_105720` (6-floor Block#90) has fixes (`d22 11:31 fixed
  a cracked window in Percival Mbeki's flat`), tenants stopping him (`d21 11:22 Freya Pembroke stopped Knox Tanaka in the hall about the
  rent`), a daily `SuperRound`, a sold sighting (`d22 21:51 BountyPaid ... 15`). `super_98326` (3-floor) serves notice on 22/17/15/13
  tenants a round (`d23 11:40`) and still has 16 `walked the stairs and halls (3 floors)` lines. Both paid a shakedown in their block.
- **Sick: thin / dead-ish (V16 alive / alive).** `sick_49012` (49 y, cancer 0.46, 0c): `d22 00:00 Evicted ... (arrears)`, `d22 01:00
  SoupLine ... 0 h 45 away`, seven `JobSearch` boards "nothing going", `d26 00:11 Hire: hired as Guard`, `d27 00:00 Skipped: off sick
  ... nobody covers the Guard's post`. A spiral with a bite. `sick_103625` (62 y, cancer 0.47): dead: idle 9-13.6 h a day, the Strip
  Club 97-415 min a day, net -6/-12/-21c, three `Loan` lines (`d21`, `d24`, `d27`) each ~210-224c short; nothing else happens.
- **Teacher: alive / thin (V16 alive / dead).** `teacher_14781`: `taught 21, 17, 21, 24, 24 pupils` (V16 29/16/23/23); steady.
  `teacher_50799`: `taught 7, 3, 4, 3, 3 pupils at School#5505`: dull but not zero (V16's `teacher_48984` was 1, then none).
- **School_parent: flat / flat (V16 flat).** `school_parent_47629`: the 6-y at School on 5 of 7 days (`d21 09:30 at School#5153 (38 min
  walk)`), two gang try-outs refused (`d22 06:13`, `d27 09:28`). `school_parent_86164`: counts none; the 12-y is home all week for a
  2c fee (`d24 09:30 ... was beyond the purse at midnight (4c in hand, 3c of it kept for the rent and meals)`). V16 bug 8 **unchanged**.
- **Bereaved: alive / dead (new).** `bereaved_6559`: bugs 1-3 and § 3. `bereaved_74870` (26 y, Cinder Hounds, spouse Nico Quarry): no
  mourning, funeral or leave line in the window; he is a gang soldier (22 `Turf` lines). The only grief is a standing grudge from
  before d21 (`d21 00:00 GRUDGE against Precinct#4485 (ChildTaken, weight 0.81)`). The pick shows nothing of the mechanism.

## 3. Round 8's mechanisms seen at 100k

- **X2 mourning, seen once.** `bereaved_6559 d21 08:00 Bereaved: Rue Oakes stays home from the shift as Guard at Security Office#4593:
  mourning her wife Raven Coldwater (leave, 1 of 3)` (`d22 08:00` 2 of 3, `d23 08:00` 3 of 3); `d21 20:00 Bereaved: The funeral of Raven
  Coldwater at Block#111: Winifred Coldwater, Freya Coldwater there; no friend near to ask`; the children's line `d21 09:30 [family]
  ... Winifred Coldwater (8 y) at home ..., mourning her mother Raven Coldwater, Freya Coldwater (0 y) ... mourning her mother`.
- **X1 the promise, seen well.** Told within hours: `super_98326 d24 16:11 ... he means to tell the law` -> `19:32 Report: went to
  Station#5903 and told the law: Nico Hallow`; `orderly_59744` twice. The out: `super_105720 d22 12:15 means to tell` -> `d23 13:00
  never told the law of Silas Greaves: the law is out of reach (no desk of it within a walk)`; `teacher_14781 d26 11:35` and `15:20` ->
  `d27 12:00` and `16:00 never told ... out of reach`. Both ends of the promise print.
- **X2 the sacked in need search, seen.** `sick_49012`: `d21 08:00 JobSearch: asked Mina Jessop about work: nobody knew of an opening`,
  `d21 13:32 read the board at Street Market#4302: nothing going`, then `d26 00:11 Hire`.
- **X6 the cell's day, seen** in `ripperdoc_14357`: meal `d22-d26 07:00`, yard `d22-d26 15:00`, kin `d22 18:00 Visited: Roxy Kemp ... told
  her the post ... is gone`, release `d26 22:46`; `bereaved_6559 d26 12:00 Visited: Zed Zhou came to the cells to see Rue Oakes; 2 days left`.
- **X5 ages on lines, seen:** `d21 10:40 Arrest: Rue Oakes (26 y) arrested Silas Adeyemi (24 y)`; `d23 14:19 Fare: ... beat Kofi Draper
  (22 y)`. **X4 gangs, in `bereaved_74870`:** `d22 01:00` and `d23 00:00 Turf: Ulysses Osgood was woken`; `d24 00:05 ... called The
  Cinder Hounds off the corners to lie low: 6 of their 14 beaten, dead, jailed or gone since the war with The Static Teeth began on day 23`.
  **X4 Succession, seen** (`ripperdoc_14357 d21 04:55`, `d25 03:16`).
- **Expected, never seen in this group:** `guard on himself`, `let it go`, `searched` after a hit (no target among these lives); a
  `Death:` line in the window (all grief here began before d21); a clinic walk-in line (X6; the clinics stayed silent); `Bout`.

## 4. Bugs, ranked by what the player sees first

1. **Mourning leave does not stop the guard's Arrest goal.** `bereaved_6559 d21 08:00 stays home from the shift ... (leave, 1 of 3)` ->
   `d21 09:19 GOAL Arrest ... in shift 1.00->1.00` -> `10:40 Arrest: arrested Silas Adeyemi`; same on leave day 3 (`d23 09:23 GOAL
   Arrest`, `09:40 Arrest: ... Maren Ravensworth (66 y)`). She is on leave, a day after her wife died, and making arrests. Likely
   cause: the leave gates the Work shift, not the `in shift` input of the law goals. New.
2. **"No friend near to ask" is false.** `bereaved_6559 d21 20:00 ... no friend near to ask`, but `d21 08:54 HANGOUT at Block#111 ...:
   Zelda Fletcher#11143 (known, Friend, affinity +1.00), Ignatius Fletcher#35672 (Friend, +0.57)` are in the same Block. The line reads
   purses, not friends. The children then go unfed (`d26 00:10 ChildTaken ... 2 days unfed and no free parent at home`). New.
3. **The unfed house, one causal line.** `0c wallet` -> `d25 07:59 Theft: stole 2 food` -> `08:55 Sentence 3 days` -> `d26 00:10 ChildTaken`
   (both kids, one 0 y, to Work Camp#5132) -> `GrudgeFormed: swore against the Law (a child taken, 0.60)`. Rule-conform and a real
   story; the bug is only that the house hit zero in three days on a guard's wage while mourning (`pantry holds 10, 0 of it ours`).
4. **A man with stage-0.46 cancer is hired as a Guard and is sick the next day.** `sick_49012 d26 00:11 Hire: hired as Guard (picked
   by Lena Reyes, the foreman)` -> `d27 00:00 Skipped: off sick ... nobody covers the Guard's post`. X3 reads a record for the law's
   posts, not the body. New.
5. **A super changes address three times in two days.** `super_98326 d23 00:05 Housed: moved with Jasper Okafor and the children, into
   Block#5904`; `d24 00:00 Housed: moved into Block#2817, the landlord's Block, as its super (with 2 of the household)`; `d24 00:05
   Housed: moved with Jasper Okafor and the children, into Block#3010`. The super moves into his block and five minutes later the
   household moves elsewhere. Likely: the job-housing move and the household move disagree. New.
6. **The kin say the same news twice.** `ripperdoc_14357 d22 18:00` and `d25 18:00 Roxy Kemp came to the cells ... told her the post as
   Ripperdoc at Ripperdoc#6019 is gone`. Low.
7. **The empty clinic.** V16 bug 7 **unchanged**: `ripperdoc_64026` one patient a week; `orderly_13875` Therapy and Detox only. X6's
   walk-in did not reach Ripperdoc#5835 (D3) or Ripperdoc#4918 (D4).
8. **No School for a 2c fee.** V16 bug 8 **unchanged**: `school_parent_86164 d21-d24 09:30 at home (play: the 2c fee ... beyond the purse
   at midnight)`; a 12-y out of school a week over 2c.
9. **The stairwell smell.** `super_105720` is stopped four times (`d21 15:21`, `16:33`, `d22 12:57`, `d23 12:32`) about the same smell
   with no fix line. Low.
10. **A doctor picks the new doctor as "the foreman".** `ripperdoc_14357 d21 08:24 Hire: Cato Carrow hired as Ripperdoc (picked by Quinn
    Fletcher, the foreman)`; she is jailed 13 h later. Label only. Low.

V16 care bugs: 1 widow works next day **better** (three leave days; but bug 1 above); 2 ripperdoc/teacher steal vehicles **fixed (not
seen)**; 3 held man witnesses the street **fixed** (`ripperdoc_14357 d25 06:49 saw Pixel Mbeki's Assault in the cells and kept to the
far wall`); 4 blackmail from a sleeper **not seen**; 5 soup walk cut **better** (`sick_49012 d22 01:00` 0 h 45, `Soup Kitchen#3139 68m`
that day); 6 one-floor loop **fixed**; 7 empty clinic **unchanged**; 8 school **unchanged**; 9 teacher at zero pupils **better**
(3-24); 10 one first name for two men **not seen**.

## 5. Dull spots, ranked (mechanisms)

1. **The empty clinic.** A clinic with idle hours takes walk-ins from the blocks round it (`ripperdoc_64026` works 2 min from two Blocks
   and a Club); X6's walk-in fires elsewhere but these two see nothing. Demand has to reach them.
2. **The sick elder at a strip club.** `sick_103625`: idle 9-13.6 h a day, net -6..-21c, a cancer bill 210c off. Mechanism: the loan
   line's friends (`Lena Reyes lent 11c`) arrive in person with food, then the second line is the visit.
3. **A mourner's week is three leave lines and one funeral.** After day 3 the grief is gone. Mechanism: a grief memory that sours
   shifts and Unwind picks for a fortnight, and a grudge toward the killer when the death was violence.
4. **The school_parent.** `school_parent_86164`: counts none, a week of `Vat Tech` and a child at home. Mechanism: a neighbour or the
   school asks after the child who never comes; the teacher waives the 2c.
5. **The super's notices.** 22 notices in one line is thin. Mechanism: one named tenant on the stairs a day later pleading (the `stopped
   ... in the hall about the rent` line exists), the notice followed by that tenant's name.

## 6. Too much

- **Yard and meal in the jailed diary.** `ripperdoc_14357`: meal `07:00` and yard `15:00` all five days, and `Ada Whitlock (in for Assault,
  out d26)` is the yard partner on d22, d24, d25, d26. Reads as a schedule. Vary the partner, drop the meal line on a plain day. Mild.
- **War call waking the member:** `bereaved_74870 d22 01:00` and `d23 00:00 was woken` (two nights running). Once is a story, every night
  is a bell. Watch.
- **Witness lines:** `bereaved_6559` 35, `bereaved_74870` 26, `teacher_14781` 11. Noise, not broken.
- **Funerals and mourning:** one funeral, three leave lines in the group. Not too much. Nothing reads as a broken city.
