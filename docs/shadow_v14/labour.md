# Shadow V14: labour critic

Group: worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 22 diaries).
100k island, seed 42, d21-27. Picks differ from V13 except `jobless_6204`. `target/r5_100k/analyze.txt` does not exist yet, so no 30-day counters.
Method: DAY SUMMARY for all 22, `[event]` blocks of each, greps (JobSearch, DawnPick, Hire, CabFare, family, Housed, Brawl, Murder, Report), full stretches of
night_shift_27693 d22-25, courier_48760 d24-25, jobless_6263 d22-24, cabbie_76417 d24. Cites are `file dDD HH:MM`. A missing line is not evidence.
Minors: every Brawl / Assault / Murder / Shakedown line in the 22 diaries prints adult ages (a grep for `(0-17 y)` on harm lines is empty). Nothing under 18 hurt, in vice or dead.

## 1. Verdict

Slightly better than V13 on the job-market arc, visibly worse on one thing a player sees at once: a worker rehoused 475 tiles from her post spends the day walking.
The chain passed over at the gate -> asks friends -> board -> hired -> moves nearer work -> steals a bike -> arrested now runs end to end inside one diary
(`jobless_6263`, `day_labourer_9155`), which V13 had only in pieces. The shift floor is still a metronome (night_shift, worker, builder, loader, courier) and the cab is one fare a night.

## 2. Per archetype (V13 word in brackets)

| archetype | V14 | excerpt |
| --- | --- | --- |
| worker | thin (thin) | `worker_40514` Vat Tech, net +5/+5/+4/-15/+4, only line is a `[family]` one: baby "0 y at home (play, under 6 for School)" d21-27. `worker_57983` one GrudgeFormed `d21 00:02`, otherwise 9.7 h shifts to the decimal. |
| worker_friday | alive one (alive one) | `worker_friday_36020 d24 20:15` Brawl "Juro Reyes (57 y) (drunk) swung at Mags Hallow (47 y) at the door of Gambling Den#5060 and put Mags Hallow down"; `d25 22:42` same man, same door, same bouncer ("turned away again: barred till d27"); `d26 07:17` a stranger swears a grudge for the beating. `_60467` is a Recycler Tech with Crew lines only (bug 6). |
| night_shift | thin, one broken (thin, near dead) | `night_shift_23451` zero events, d22-26 work 8.3 h exactly. `night_shift_27693` see bug 1: d23-26 travel 14.0/9.3/14.0/16.9 h, work 5.5/5.6/5.5/2.4 h. |
| jobless | alive (alive) | `jobless_6263 d22 14:20` "asked at Ripperdoc#5750 about work and was taken on as Orderly", `d23 00:12` "moved nearer work, into Block#2282", `02:16` steals Silas Marlow's Motorcycle, `d24 00:26` Arrest, `02:16` Sentence 6 days. `jobless_6204` (V13's alive one) is now the flat one: d24-27 only Scavenge/HangOut, no JobSearch after `d23 12:30` "the post was gone". |
| fabber | thin (thin) | `fabber_105004 d26 19:29` bouncer "turned Rook Kimura away again (drunk): barred till d28", `20:58` Club#5914 turns him away too (V13's door arc again). `fabber_84414` pays a 7c CabFare each leg on a 7c wage (bug 3). |
| loader | thin both (alive one, thin) | `loader_31530` 82 Unload lines, no events; `loader_73631` one line, `d21 00:00` "robbery of Ilse Novak on day 19 will never be solved". The V13 jailed/sacked loader has no heir. |
| builder | alive one, thin (alive one, thin) | `builder_13320 d22 00:07` LaidOff (revenue), `12:28` board "a Vat Tech post at Vat Farm#3676", `14:58` taken on, `d23 00:12` moved nearer work; the three school-age children then ride "59 min by the El, the fares paid" d23. `builder_87051` no events, net +1/+2/+2/+2/+2. |
| foreman | alive (alive) | `foreman_21236 d21 06:00` "picked 17 at the gate ... 605 sent home"; `foreman_25916 d23-26 06:00` "opened the gate ... and took nobody: nobody at the gate he would take" after three LaidOff (`d21 00:07`, `d22`, `d23`). |
| courier | alive one, thin (thin) | `courier_48760 d25 05:09` Intimidate fails ("BACKLASH"), Enemy edge, `05:10` GOAL Fight, `05:25` "Murder: Fenn Fletcher (32 y) attacked Anselm Chrome (44 y)", `05:37` strips the body (2 coins), `08:10` back at work. `courier_65373` nothing. |
| cabbie | thin (alive) | `cabbie_76417 d22-27 00:00` "stood by all shift ... no fare, no pay" x6; `d23 00:09` "Coerced: blackmail of 30 a week" and CoercionBroken in the same minute. `cabbie_70655` "drove 1 fare, 3c his cut" nearly every night. |
| day_labourer | alive (thin) | `day_labourer_9155 d22 06:00` DawnPick "stood at the gate ... and was sent home", `07:00` asked 8 friends, `07:46` Bar "the post was gone", `13:15` "taken on as Fab Tech", `d25 11:05` steals a motorcycle, `11:29` Report, `12:35` Arrest, `13:44` Sentence 6 days. V13's "passed-over have no beat" is closed here. `_101414` hired Bartender for a gang front `d22 00:11`, +5c a night from "gang The Drowned Choir 8". |

## 3. Round 5's mechanisms seen at 100k

Fired in this group:
- **U1/U2 `[family]` line**: 3 parents (builder_13320, worker_40514, worker_friday_60467), 7 lines each. Seat and fee reasons both print: `builder_13320 d21 09:30` "Rook (12 y) at home (play: the 2c fee at School#5195 is beyond the purse)", `d22` two of three at School, `d23` all three "59 min by the El, the fares paid" after the move. First time the fee and the move read as one story.
- **"Moved nearer work"**: `jobless_6263 d23 00:12`, `builder_13320 d23 00:12`, both straight after a hire.
- **Foreman DawnPick**: 13 lines for `foreman_21236`, three variants (picked N at the gate; "took N off the list, the foreman not at the gate" d25 only; "took nobody"). "Not at the gate" fell from 4 of 6 days (V13) to 1 of 6. Better.
- **JobSearch chain** in 4 of 6 unemployed diaries: friends, board, door, "the post was gone", "turned away", "taken on as X". "Post was gone" 5 times (V13: 0).
- **U7 contradictions**: no victim "Witness: saw X commit Shakedown" of his own robbery in the group (`night_shift_27693 d21 06:08` is a bystander, correct); `UnderWater` names a clinic (`jobless_6263 d24 00:07`: "wages 21c -> 13c a day ... no hiring").

Expected, never seen: any SoupLine / kitchen / starving walk (U3; nobody here starved); MissionServed; a school founded for a home with "no School within 1 h" (bug 5); a Sacked or Fire (V13 had one, 0 here); a Skipped shift (V13 two, 0 here); a CabFare on the cabbie's own side; a `booked` / `wash` line in `jobless_6263`'s own clinic diary.

## 4. Bugs, ranked by what the player sees first

1. **Rehoused 475 tiles from the post.** `night_shift_27693 d23 00:08` "Housed: Thea Sallow moved in with family at Block#2096 (D1 The Sump)"; her post is Street Market#4212 in D4 Mid-Levels. `d23 06:02` "GoTo(Home) ... 503 min, 475 tiles", `18:28` "GoTo(Workplace) ... 509 min, 470 tiles". d23-26 travel 14.0/9.3/14.0/16.9 h, work 5.5/5.6/5.5/2.4 h on a 5c wage; `d24 06:02` she switches to Capsule Hotel#5029 (188 min each way). Likely cause: the family move-in branch has no distance check against the post (the "moved nearer work" branch does). New; worst commute in the group.
2. **Murder with no law after.** `courier_48760 d25 05:25` kills a 44-y stranger, strips the body `05:37`, at work `08:10`; zero Report / Arrest / Pursuit / SawCorpse-by-others lines d25-27. `day_labourer_9155` is reported 24 min after a bike theft. A body found and a case opened is missing. V13 bug 9 (downed victim, no law) unchanged, now with a death.
3. **CabFare to a stranger or a corp, 7c each way, a whole day's wage.** `fabber_84414` (night-shift Fab Tech, 87-tile commute) d21-27: 8 lines, `-7c (CabFare) ... to Florian Duarte#15682 (Statistical)`, `to Sable Marlow#104713`, `to corp Zetatech 6`, `to corp Voss Holdings`; net -12/-7/0/-5/0/-1/-5 on a 7c wage. A different payee each time, none a cabbie. V13 bug 2 unchanged, and now a commuter loses money by going to work. The cabbies' own diaries show "drove 1 fare ... 3c his cut" and no CabFare.
4. **Cab spends the shift walking.** `cabbie_70655 d21 17:21` "GoTo(Spot) ... 95 min, 216 tiles" D1 to D6, `d23 17:47` 96 min/219 tiles; work 0.2-1.8 h in a 5-h shift, one fare. V13's 369-tile worst case is milder, not fixed. `cabbie_76417` stays in D3 but drives no fare six nights, resting 86-90 min at the Spot.
5. **A 6-year-old alone all day, again.** `worker_friday_60467 d21-25 09:30` "Verity Northcott (6 y) at home (play: no School within 1 h)", mother at work 8.9 h; "no School today" d26-27. V13 bug 5 unchanged at 100k; the U1 school did not reach this home. Nobody is said to mind the child.
6. **Gang call from a recycler.** `worker_friday_60467 d21 00:07` "sat out The Black Ghosts's raid: the muster at Hideout#4500 is 6 h 56 from home"; `d23 00:07` "stays off ... trade: the nearest spot is 12 h 53 from home (same d24)"; `d25 00:07` 10 h 45. A holder 7-13 h from any spot is a member by name only. V13 bug 10 unchanged.
7. **Duplicate payout.** `worker_friday_60467 d21 09:03` two identical `BountyPaid: sold a sighting of Hex Underhill for 15` in one minute, one `+30c (Payout)`. A sighting sold twice. New.
8. **Midnight hires stamped by a sleeping foreman.** `foreman_21236 d21 00:11`, `d23 00:11`, `d24 00:11` "Hire: X hired as Builder (picked by Silas Ostrowski, the foreman)": a 00:11 batch stamp, not a scene. Minor.
9. **A hired man tries out for his own employer.** `day_labourer_101414 d22 00:11` hired Bartender for The Drowned Choir 8, paid +5c at 00:00 d23-27; `d27 08:19` PLAN JoinGang on the same crew, `11:25` "tried out ... and was turned away: they were full up". Header says `Job: none`.
10. **A clinic hire picks hires.** `jobless_6263 d22 14:20` hired Orderly, `d23 00:11` "Odessa Oyelaran hired as Orderly (picked by Lavinia Osgood, the foreman)": a worker hired 10 h earlier is already the foreman, then steals a bike at 02:16.
11. **Coercion with no scene.** `cabbie_76417 d23 00:09` "leaned on Orrin Ashcombe (secret): blackmail of 30 a week", then `CoercionBroken: refused (would not pay)` the same minute. A 6c-wage cabbie asking 30c a week.

V13 labour bugs: 1 sacked man no search: unknown (0 Sacked); 2 CabFare payee: unchanged; 3 cab walk: better; 4 far posts: worse (bug 1); 5 child alone: unchanged; 6 shakedown money `other`: not seen (1 Shakedown line, no money line); 7 dawn pick not at gate: better; 8 passed-over no beat: fixed (`day_labourer_9155`); 9 downed victim no law: worse; 10 far gang walk: unchanged; 11 odd pick: unchanged (foremen are Builder-titled, a day labourer is a Bartender).

## 5. Dull spots, ranked (mechanism, not a knob)

1. **The shift is still one tile.** `night_shift_23451`, `loader_31530`, `builder_87051`, `worker_57983`, `courier_65373`: no event in a week, hours to the decimal. Mechanism: a per-shift incident with a memory (a short count, wrong crate, an inspector, a mate who did not show) and a shift-end line saying what.
2. **The jobless man who stops looking.** `jobless_6204 d24-27`: Scavenge 3 h, HangOut 4 h, zero JobSearch, and a `Loan` line repeated verbatim d21, d24, d27 ("Rafe Haas, 240c short of a vat-cancer treatment ... nothing to spare"). Mechanism: after the board and door fail twice, the search turns to the dawn gate, a friend's yard or a gang front; the Loan line changes when a friend has something.
3. **The killer goes to work.** `courier_48760 d25`. Mechanism: a found body, a case, a witness-less investigation; the killer carries it (a Haunted memory, a skipped shift, a drink at the Den).
4. **Six nights of "no fare, no pay".** `cabbie_76417`. Mechanism: the fare as an errand (the hurt to the Clinic, the wanted away from a raid) so a night has one real fare.
5. **A drunk beats the bouncer twice with no answer.** `worker_friday_36020 d24 20:15`, `d25 22:42`: no Report, no counter-hit; a stranger swears the grudge `d26 07:17`. Mechanism: the den's owner or the bouncer's mates answer the second beating.
6. **Rest day.** `courier_65373 d27` idle 15.6 h, `worker_friday_60467 d27` 15.8 h, `foreman_25916 d27` 9.4 h. Mechanism: church, the Den, kin, a pit fight.
7. **The clinic that cuts the day after it hires.** `jobless_6263`: Orderly d22, UnderWater d24 00:07, bike theft d23 02:16. A good arc; what is missing is the new job's first pay packet before the theft.

## 6. Too much

- **600 sent home at one yard.** `foreman_21236 d21 06:00` "605 sent home (Sprocket Ravensworth, Kestrel Garland, Vex Okafor and 602 more)", d22 623, d23 572, d24 603 for a yard that takes 10-19. A ticker, not a crowd; one line with the count would read better (V13 asked for the same).
- Nothing else reads broken. The courier murder and the two bike thefts in three days are harsh, not noise.
