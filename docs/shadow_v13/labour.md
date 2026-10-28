# Shadow V13: labour critic

Group: worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 22 diaries).
100k island, seed 42, d21-27, main a70800f. Picks differ from V12 (new ids), so comparison is by archetype. Method: DAY SUMMARY for all 22, the
notable-events block of each, greps for JobSearch / DawnPick / Served / CabFare / PARCEL / (other) / Sacked / Fire, full stretches of
jobless_6204 d22-24, jobless_6252 d21-22, loader_74097 d21-27, cabbie_73680 d25, builder_86355 d26, foreman_26309 d22, day_labourer_14441 d22-24.
Cites are `file dDD HH:MM`. A missing line is not evidence.

## 1. Verdict

Better than V12, and for the first time the gain is in the middle of a life, not at its edges: a jobless man walks his Friends, the boards and a
door and is taken on as a Clerk, is shaken down four times in a week, skips a shift in misery and is sentenced for theft by d27; a loader's
three arrest-resists end in the cells, a sacking and a gang card in one day. The shift itself is still a metronome (night_shift, worker,
builder, courier d22-26 to within minutes), the far posts are untouched (cabbie 369-tile "Spot" walks, loader 93-min commute), and the
shakedown money line is still `other`. No minor is hurt, in vice or dead in the 22 diaries; every harm line prints ages and all are adults.

## 2. Per archetype (V12 word in brackets)

| archetype | V13 | excerpt |
| --- | --- | --- |
| worker | thin (alive, flat) | `worker_58323` Vat Tech 9.7 h, travel 5.5 h daily, no events at all; `worker_40650` same, net +5/+5/0/0/+1. No `[family]` line this time. |
| worker_friday | alive one, thin (thin) | `worker_friday_37200 d25 06:57` Club before the shift, Sweep beat 09:40-15:55, `18:30` 185-min walk (253 tiles) to The Drowned Choir, `21:35` joins; `d27 14:48` Grand Theft, Motorcycle stolen, reported by Lena Voss. `_62819 d27 11:49` buys a Motorcycle on finance with her last 18c (purse 0), `13:06` joins the same gang, `16:44` begs 1c. |
| night_shift | thin, near dead (thin) | `night_shift_23388` zero events in the window; d22-26 are sleep 6.0 / work 8.4 / leisure 7.4-7.6 / travel 0.6 to the decimal. `_27519` the same with +-1 h. |
| jobless (new) | alive (both) | `jobless_6204 d21 07:00` JobSearch asked 8 Friends: "nobody knew of an opening"; `09:20`, `13:33` boards read "nothing going"; `d22 09:04` "asked at Street Market#4280 about work and was taken on as Clerk"; `d24 08:00` Skipped "does not go in (mood -0.46, misery 0.87)"; `d27 21:29` Arrest for Theft, `22:49` Sentence 3 days. `_6252 d22 08:44` board "a Vat Tech post at Vat Farm#4125", `11:32` taken on. |
| fabber | thin (alive one, thin) | `fabber_97960 d24 21:19` Gambling Den bouncer turns Anselm Kemp away "again: barred till d26", `22:15` Club#5588 turns him away too. `fabber_101305` no events, net +7/-4/+7/-5. |
| loader | alive one, thin (alive one, thin) | `loader_74097 d21 23:09`-`d23 14:53` three resisted arrests then Arrest, `d23 16:13` Sentence 3 days, `d26 00:07` Fire "absent 3 times in 14 days, in the cells", `d26 16:13` "came out to a changed life: the post gone", `19:26` GangJoin "miserable, mood -0.40". `loader_31530` Station/ClerkWork/Unload/Break loop, 12 Witness lines only. |
| builder | alive one, thin (alive one, thin) | `builder_86355 d26 06:51` shaken down 3c by Viktor Lindqvist, `07:05` GOAL Fight, `07:20` Assault "Amara Adeyemi (28 y) attacked Viktor Lindqvist (67 y)", Friend Sprocket Quarry "is now Enemy" for running. `builder_11842` flat, net +2 daily. |
| foreman | alive (thin; V12 was a Hire ticker) | `foreman_26309 d22 06:00` DawnPick "took 13 at dawn off the list, the foreman Elspeth Mbeki not at the gate ... 16 sent home (Bram Stroud, Dagny Lindqvist ...)"; `foreman_25916 d24 06:00` "picked 1 ... Edric Sallow; 21 sent home". |
| courier | thin (alive one, thin) | `courier_50838 d21 13:34` two parcels to Osric Hawthorne at one door in one minute; `courier_66222 d21 11:58` two to Wren Yardley, `d22 12:08` two to Lab#5099 again ("a regular"). Quiet shift otherwise. |
| cabbie | alive (alive) | `cabbie_68453 d26 16:07` FareShort "Varek Ito ... skipped the 28 fare", Enemy edge and Robbed grudge; `d23 00:00` "stood by all shift ... no fare, no pay"; `d26 00:33` marries Corwin Underhill. `cabbie_73680` 1-2 fares a day, 3-4c cut. |
| day_labourer | thin (thin/dead) | `day_labourer_14441 d22 06:00` hired by Yorick Yardley, `06:01` at the yard in 1 min, Rest 28 min (V12 91+83), ClerkWork 16 min, `06:49` 122-min walk to a site; hired d22/23/25/26, `d24` passed over: Scavenge, 11 h leisure, net 0c. `_70003` is an Office Worker (odd pick again). |

## 3. Round 4's mechanisms seen at 100k

Fired:
- **T5 jobless search**: both jobless picks ran the whole chain in the diary (Friends asked, boards read, door asked, taken on at the door), one on d21-22, one on d22. "Post found by board" and "nothing going" both appear. Hire lines name the picking foreman.
- **T4 foreman DawnPick**: 10 lines across two foremen, with the picked names, the count sent home and the "foreman not at the gate" variant (4 of 6 days for `foreman_26309`). The day labourer is picked at 06:00 and at work by 06:34 (yard rest down to 28 min).
- **T4 door**: `fabber_97960 d23 22:22`, `d24 21:19` (barred till d26), `22:15`: the drunk turned away, barred, then turned away at a second house.
- **T3 Pursuit**: `loader_74097 d21 18:03`, `d23 01:01` "gave up the chase (the trail went cold)", handed on to the next guard, then the Arrest `d23 14:53`.
- **Sacked/Fire by absence**: `loader_74097 d26 00:07` the cells count as absence, dismissal, Sacked memory, "came out to a changed life". Law and labour finally link.
- **S5 cab cut / "no fare, no pay"** still present; T6 `FareShort` is new and gives the cabbie a grudge.
- **T1/T2 `[family]`**: one parent in the 22 (`day_labourer_70003`), children 9, 6, 5 at home "play: no School within 1 h" d21-25, "no School today" d26-27. Fired, but see bug 5.
- **T6 asset**: `AssetBought ... on finance` (Motorcycle) on two diaries d27; `VehicleStolen` in a third.
- **T7 folds**: no back-off flood, no repeated standing-fact line (V12's "sat out the raid" x2 gone; one `stays off ... trade` line, `loader_74097 d27 00:07`).

Expected, never seen: "took a post nearer home", a misery quit, an exec giving up a post, a jobless search after a sacking (bug 1), a re-hire of the sacked, a refusal at a shop for a burned mark, any wage advance or slate for a worker, a Report after the builder's Assault.

## 4. Bugs, ranked by what the player sees first

1. **A freshly sacked man does not search (T5 gap).** `loader_74097 d26 16:13` released, post gone, `16:14` GOAL JoinGang 0.480 (lawfulness 0.56->0.32), joined 19:26; zero JobSearch lines d26-27. The born-jobless search; the one who is sacked goes straight to a gang. Likely the search is gated on days-without-post (`jobless_6204` reads "days without a post 0.79"), which a sacked man starts near zero. New.
2. **The cab's fare is paid by the wrong person.** `cabbie_73680` all 8 CabFare lines "from Tess Greaves#76643 at Block#41" (his home block, a Friend) while the rider memory names a different fare each time (Liesl Pembroke, Perrin Draper, Omar Mercer ...). `cabbie_68453` pays "from corp Achebe Holdings" for riders who are people. T6 asked for who paid and whose till; this reads as the home block's occupant or owner paying every fare. New.
3. **Cabbie spends the shift walking to the Spot.** `cabbie_73680 d25 07:00-13:53` GoTo(Spot): 69 min/157 tiles, 107 min/243 tiles, 162 min/369 tiles, 123 min/280 tiles across D4/D5/D6, two fares of 3-4c in 8 h; work 0.6 h, travel 9.3 h. V12 bug 4, worse (V12 max 266 tiles).
4. **Far posts persist (V12 bug 4, unchanged).** `worker_58323` travel 5.5 h daily; `worker_friday_37200` 7.0-8.7 h; `loader_31530 d23 06:02` after a night shift walks 101 min/98 tiles to a market in another district, 79 min home; `foreman_26309 d22 06:59` 72 min/162 tiles to the yard.
5. **A 5-, 6- and 9-year-old alone all day.** `day_labourer_70003 d21-27 09:30` three children at home in play, the mother 8.6 h at work, "no School within 1 h" in D4 Mid-Levels. T2 reaches School at 20k; at 100k a Mid-Levels home has none in the hour. No harm, but nobody is said to mind them.
6. **Shakedown money still `other`, wrong place owner.** `courier_50838 d23 03:28, 03:34` -3c, -2c "(other) ... place owner corp Habitat 5" while asleep at home, two extorters in 6 min; `jobless_6204 d24 01:01, 03:34, d26 00:51, d27 00:57` four times, -5, -2, -5, -4; `builder_86355 d26 06:51`. The victim also gets a "Witness: saw X commit Shakedown" of their own robbery. V12 bug 6, unchanged (T6 labelled the bills, not the shakedown).
7. **Dawn pick at 06:00 for a 09:00 yard with no one at the gate.** `foreman_26309 d22-24, d26` "the foreman not at the gate: took 13 off the list": the foreman is asleep at home 72 min away, leaves 06:54. A list, not a face. When the foreman is there he picks 1 of 19 (`foreman_25916 d23`).
8. **The passed-over have no beat.** The same names are sent home daily (Bram Stroud d22, d23, d25, d26; Dorian Carrow d23, d24; Hesper Kimura d24, d25) and none shows in my jobless diaries. `day_labourer_14441 d24` not picked: Scavenge 03:48-06:21, "goal Earn cooled", HangOut 11 h, net 0c.
9. **A downed 67-year-old, no law.** `builder_86355 d26 07:20` Assault, 4 witnesses, no Report in the next 17 h. (V12 bug 7, unchanged; the loader's resists get a Pursuit, the builder's victim gets nothing.)
10. **Far gang walk.** `worker_friday_37200 d25 18:30` 185 min/253 tiles to join, `22:08` 172 min/180 tiles to a Capsule Hotel. Reads as a marathon, not a gang.
11. **Odd picks / stale header.** `day_labourer_70003` is an Office Worker (Hire d21 00:11); `jobless_6252` header says `Job: none` though hired Broker d21 and Vat Tech d22 (header is the day-28 state, fine, but the pick is a Broker/gang boss on d21).

V12 status in my group: 1 day labourer rest in the yard better (28 min, hired from 1 min away); 2 laid off, no search better (jobless search) but see bug 1; 3 Station metronome unchanged (loader now unloads at named neighbours: better); 4 far posts unchanged/worse; 5 Bury retries fixed; 6 unlabelled money unchanged; 7 downed victim, no law unchanged; 8 no skip/quit/sack better (2 Skipped, 1 Fire in 22 diaries); 9 odd pick unchanged.

## 5. Dull spots, ranked (mechanism, not a knob)

1. **The shift is one tile (night_shift x2, worker x2, builder_11842, courier_50838).** `night_shift_23388` has no events and the same hours five nights. Mechanism: a per-shift incident that leaves a memory (a short count, a wrong crate, an inspector, a late relief) and a shift-end line saying what happened.
2. **The passed-over come to nothing.** Mechanism: a hand sent home three days running drifts to the board (T5 search from the dawn gate), steals, begs or picks a fight at the yard; the foreman remembers who turned up first.
3. **The cabbie has no rider's errand.** One fare a morning, 3-4c. Mechanism: the fare as an errand (the hurt to the Clinic, the wanted away from a raid, the drunk home) that both remember.
4. **Courier parcels arrive in pairs and say nothing.** `courier_66222 d21 11:58` same sender, same recipient twice in a minute. Mechanism: one parcel, one errand, a recipient who is out or a signed-for hand-over the recipient's diary shows.
5. **The sacked and released.** After bug 1: a released man with no post takes the T5 search, a back-room hire or the gang, and says which.
6. **The jailed man's mates.** `loader_74097 d24-d26` three days in the cells and nobody visible. Mechanism: a visit, a note, a Friend covering his rent.
7. **Rest day.** `courier_50838 d27` 15.8 h idle, `courier_66222 d27` 16.6 h, `foreman_26309 d27` 10.9 h. Mechanism: church, the Den, kin, a pit fight.

## 6. Too much

- Nothing harsh enough to read as broken. `jobless_6204` shaken down four times in 6 days for 16c is a lot for a Clerk on 5c a day, but it reads as Sump life and feeds the theft arc.
- Monotony: `night_shift_23388/_27519` and `worker_40650/_58323` d22-26 are one day repeated to the decimal; `loader_31530` is the same 57-min Station/ClerkWork/Unload/Break cycle for 8 h (named Unload sites help).
- `foreman_26309` six near-identical DawnPick lines with the same "and N more" lists; one line with the count and what changed would read better.
- `day_labourer_14441` Hire x4 in 4 days, same 122-137 min walk to a different "site" each day (V11/V12 random site, unchanged).
