# Shadow V12: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 20 diaries).
100k island, seed 42, d21-27, main 6e1c87f. The picks differ from V11 (new person ids), so the comparison is by archetype. Method: DAY SUMMARY for all 20,
greps for Hire / Fare / PARCEL / Served / Crew / Visited / plan back-offs, full stretches of cabbie_73790 d22-25, day_labourer_100168 d23-24, night_shift_23119 d21-22,
loader_33807 d21 and d26, builder_86934 d24-25, fabber_105004 d27, foreman_21236 events. Cites are `file dDD HH:MM`. A missing line is not evidence.

## 1. Verdict

Slightly better than V11, and the gain is in the scenes at the edges, not the shift: the cab now pays by the fare and says so, a drunk builder is turned away from three
dens and swings at a bouncer, a fabber's assault runs to arrest and a 10-day sentence in 12 hours, a laid-off hand is called back as sick cover. The shift itself is still a
metronome (Rest / ClerkWork / GoTo(Station) every 20 min), the foreman is a Hire ticker (174 lines, no scene), and the passed-over and the laid-off have no consequence.
No minor is hurt, in vice or dead in the 20 diaries; the only children seen are `[family]` lines (a 13-year-old at School, a 6-year-old at play, a 12 and a 10 in a Work Camp).

## 2. Per archetype (V11 word in brackets)

| archetype | V12 | excerpt |
| --- | --- | --- |
| worker | alive, flat (flat) | `worker_41048` Vat Tech 9.7 h work, net +5/+4/-4/+2/-8/+5c; colour is `[family] my children: Odessa Greaves#108062 (13 y) at School#5903` d21/23/24 and "at home (play)" d22/25/26 (S6 line works, School is not daily). `worker_58937` 8.9 h, travel 0.9 h, no events. |
| worker_friday | thin (alive one, thin) | `worker_friday_62108` Sanitation, 6.2-8.3 h travel on every shift (work 4.9-6.9 h). `worker_friday_37005 d23 04:29` Crew: tried out for The Unplugged 3 "turned away: they were full up"; `d26 08:41`, `d27 09:57` Burial x2. |
| night_shift | thin (thin) | `night_shift_27364` identical d22-26 to the minute (work 8.4, leisure 7.1, net +6). `night_shift_23119` dead: bug 2. |
| fabber | alive (one), thin (thin) | `fabber_105004 d27 01:21-13:22` robbed (-4c), `01:38` Assault on Odile Fletcher, `06:44` patched in a back room for 6c, `11:56` Arrest, `13:22` Sentence 10 days: a whole arc in a day, then 8.6 h jail. `fabber_128928` Fab Tech in D6, travel 5.5-5.7 h daily. |
| loader | alive (one), thin (thin, one story) | `loader_33807 d21 07:15` Assault, 3 witnesses who tell 6 people, `07:27` walks 180 min to a back-room Clinic, `13:50` GangJoin The Iron Lanterns ("miserable, mood -0.36"); `d26 00:07`, `d27 00:07` "sat out The Iron Lanterns's raid: the muster at Hideout#4529 is 5 h 40 from home". `loader_73631` 8.5 h, 0.8 h travel, net 0. |
| builder | alive (one), thin (thin) | `builder_86934 d25 20:00` Brawl: drunk, turned away at Gambling Den#4954 20:00, #5048 20:33, #5079 21:52 (three bouncers), swings at Dagny Bellamy and puts her down, 4 witnesses. `builder_13929` flat. |
| foreman | thin (alive one, thin) | `foreman_21236` 174 Hire lines, 19-32 each 06:00, no other events; `foreman_25916` Fare Evasion witnessed x2 (`d23 06:52`, `d27 08:45`), `d27 15:35` JackedIn. V11's blackmail/grudge is not repeated. |
| courier | alive (one), thin (alive one, thin) | `courier_45748` 94 PARCEL lines, `d21 11:25` four handed to Wisp Northcott in one minute (fee 1c each). `courier_64717` 72 PARCEL; `d25 00:49-05:33` purse 13c to 0c in "other" lines (bug 7), wage +8c `18:00`. |
| cabbie | alive (both) (alive one, dead one) | `cabbie_68516 d23 09:27` "+4c (CabFare) ... took Roxy Thorne#95067 as a fare", `d23 16:00` "drove 1 fare ... 4c his cut"; `d21/d22/d24 16:00` "stood by all shift ... and drove no fare: no fare, no pay". `cabbie_73790` 1-2 fares a morning, 5-10 h travel on 2-3 h work. |
| day_labourer | thin / dead (thin) | `day_labourer_100168 d23 06:00` hired by Lavinia Oakes, `06:01` to the yard in 3 min, Rest 91+83 min, ClerkWork 16 min, 131-min walk to a block, ClerkWork 371 min: net 0c. Hired twice in 7 days. `_9762` is a Bouncer, not a day labourer (bug 9). |

## 3. Round 3's mechanisms seen at 100k

Fired:
- **S5 cab cut**: `cabbie_68516 d23 16:00` "4c his cut", `d25 16:00` 11c, `d26 16:00` 5c, and the honest zero "no fare, no pay". V11's unpaid "Paid" is gone: money lines now match the Served line.
- **S5 bouncer's bar**: `builder_86934 d24 20:00-d25 21:52` six Served lines, a Brawl the second night. Reads like a person.
- **S1 distance**: `loader_33807 d26/d27 00:07` "sat out ... raid: the muster ... is 5 h 40 from home"; `day_labourer_100168 d24 00:07` "could not make the trip to Casimir Kaneda at Work Camp#5132 this week: 9 h 16 from home" (children 12 and 10 in the camp; his pantry 0 from d23).
- **S2 back-off**: `worker_friday_37005` `goal Bury cooled ... backed off, 3 in a row` up to 12 in a row (`d21 00:00` to `d25 18:02`): fires (see bug 5).
- **Sick cover**: `night_shift_23119 d25 00:00` Skipped: Smoke Garland off sick, "Zelda Stroud covers the Vat Tech's shift for the day", plus Hire as Vat Tech.
- **S3 law**: `fabber_105004 d27 11:56` "arrested Rook Kimura for Assault on Odile Fletcher" (victim named), `13:22` Sentence 10 days, `13:52` "met 4 in the cells this hour".
- **S6**: `[family]` lines on worker, courier, cabbie, day_labourer: children at School, at play, in a Work Camp. No harm.
- **S7**: rest day marked on posted workers, counts lines present.

Expected, never seen in the 20: "stays off the trade", "took a post nearer home", Sacked, a skip "does not go in", a misery quit, a hired hand at the yard, a rough client in a cab, a walk for a lost post turning back, "order left at the door", Blackmail, a wage advance, slate/pawn for a worker, a hit heard of.

## 4. Bugs, ranked by what the player sees first

1. **Day labourer hired, then a day of Rest in the yard (V11 bug 1, better, same shape).** `day_labourer_100168 d23 06:04` Rest 91 min + `07:37` Rest 83 min at the yard before 16 min of ClerkWork, then a 131-min walk to a "site" Block#5849 (122 tiles), 371 min ClerkWork: net 0c. Hired 2 of 7 days; the other days Scavenge / HangOut / Wander (`d24 02:32-14:19`). The hire now turns up (V11: never), but the day is waiting and walking. V11 bug 5 (random site) unchanged.
2. **Laid off, no income, no search (new).** `night_shift_23119 d21 00:07` LaidOff "a shift past full staff" (told by Adela Thorne, the foreman), then d21-d24 idle 13.8-15.2 h/day, Rest 90-min blocks in Block#104, net 0c, no job-seek goal. Re-hired only by sick cover `d25 00:00`. Nobody follows it.
3. **The Station metronome and TargetHome ping-pong (V11 dull spot, worse).** `loader_33807 d21 21:04-23:48` and `d26 00:00-03:34`: GoTo(Station) 15 tiles, GoTo(TargetHome) to a neighbouring yard, GoTo(Workplace) back, every 20-30 min, ~12 hops in 3 hours; travel 11.9 h on d26 for a 6.5 h shift. Reads as a loop, not work.
4. **Far posts persist (V11 bug 4, unchanged).** `worker_friday_62108` 6.2-8.3 h travel/day; `fabber_128928` 5.5-7.3 h; `cabbie_73790` 5-10 h (fares 188-266 tiles for 3-4c: `d22 08:39` 83 min 188 tiles, `d24 09:24` 117 min 266 tiles). S1 did not move these posts.
5. **Unreachable Bury goal retries all week (new).** `worker_friday_37005 d21 00:00-d25 18:02` backed off 3 to 12 in a row at 12 h each: the 12th is the same as the 3rd; nothing escalates.
6. **Unlabelled money (new, minor).** `courier_64717 d25 00:49, 02:14, 05:33`: -5, -5, -3c booked as "other" with a Witness event beside it; purse 13c to 0c in 5 h. Either the courier is the shakedown target and the label is wrong, or a witness is charged.
7. **A downed bouncer, no law (new, minor).** `builder_86934 d25 20:00` Brawl put Dagny Bellamy down, 4 witnesses, no Report, no Arrest in the next 28 h; `d26 06:19` he drinks at her door again. The fabber's assault was arrested in 10 h; the difference is the victim or the hour.
8. **No skip, quit, sack or walkout (V11 bug 8, unchanged).** 0 "does not go in", 1 sick cover in 140 person-days.
9. **Odd pick.** `day_labourer_9762` is a Bouncer at Strip Club#5933. Wastes a diary, not a sim bug.

V11 status: 1 better; 2 (Paid without coin) fixed; 3 (cab rests for pay) fixed ("no fare, no pay"); 4 far commutes unchanged; 5 random site unchanged; 6 Station metronome unchanged, worse in the loader; 7 jump not seen; 8 unchanged.

## 5. Dull spots, ranked (mechanism, not a knob)

1. **The foreman's shape-up is a log, not a scene.** 174 Hire lines, no passed-over face. Mechanism: the ones not picked drift to the yard gate; a repeatedly passed-over hand steals, begs or picks a fight, and the foreman remembers who turned up first.
2. **The jobless have no hunt** (`night_shift_23119`). Mechanism: laid-off people walk to the yards and the market boards; a "Hired by corp ..." rumour (the gossip lines exist) turns into a GoTo.
3. **The shift is one tile** (`worker_41048`, `night_shift_27364`, `loader_73631`, `builder_13929`). Mechanism: a per-shift incident that leaves a memory (short count, wrong crate, an inspector).
4. **The cab has no errand for the rider.** One fare in the morning, then nothing. Mechanism: the fare's own errand (the injured to the Clinic, the wanted away from a raid), and the tip or refusal as a scene both remember.
5. **The rest day is idle** (`courier_64717 d27` 16.1 h idle, `foreman_21236 d27` 13.8 h). Mechanism: church, the Den, kin, the pit.
6. **Parcels arrive in bursts** (`courier_45748 d21 11:25`). Mechanism: one errand with a delay that the recipient also sees.
7. **A father cannot reach his children** (`day_labourer_100168`): the "could not make the trip" line repeats weekly with no next step. Mechanism: he saves for the trip, asks a neighbour, or takes a job near the camp.

## 6. Too much

Nothing harsh enough to read as broken. Monotony: (a) `night_shift_27364` and `worker_41048` d22-26 are one day repeated to the minute; (b) `foreman_21236` 174 Hire lines (collapse into one line with a count and the first names); (c) `loader_33807` ~12 Station/TargetHome hops in 3 hours; (d) the same standing-fact line on consecutive days (`loader_33807 d26 00:07`, `d27 00:07` "sat out ... raid").
