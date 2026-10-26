# Shadow V10: labour critic

Group: worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer (two each, 20 diaries).
100k island, seed 42, d21-27, main `33f9c55`. Method: `camera.tsv` rolled up per person, DAY SUMMARY lines for all 20, event counts for all 20,
full stretches read for day_labourer_14274 d22/d27, day_labourer_81276 d22, builder_13929 d21-22, builder_92107 d23, cabbie_96284 d21-26,
loader_27669 d21-27, courier_67672 d23, night_shift_25982 d22, worker_friday_39479 d25. Cites are `file dDD HH:MM`. A missing line is not evidence.

## 1. Verdict

Against V9 the labour group is better where the pass touched it: the day labourer has a dawn hire by a named foreman, the builder works at his own yard and sleeps near it,
and the miserable cabbie skips a shift with a reason. The trades inside the shift are still a metronome, and the long commute (4-17 h of walking for six people) is still the biggest thing a player sees.
Harsh-city stories now start (a drunk builder floors a bouncer, a theft reported on d21 and a sentence on d25, a cabbie who starves and sleeps in the Sump), but few are picked up by anyone.

Pooled over the 20: work about 4.9 h, moving about 5 h, home about 10.3 h a day (V9: 5.7 / 4.4 / 12.5).

## 2. Per archetype

| archetype | V10 | V9 | excerpt |
| --- | --- | --- | --- |
| night_shift | alive | alive | `night_shift_25982 d22 18:47` GOAL Fight (enemy near, odds 0.89), Assault on Yorick Kowalczyk, three witnesses, two tell their people, one runs, a new Enemy edge, gossip about him for 5 h. 0.5 h moving a day. |
| foreman | alive | alive, thin | `foreman_21236` picks 18 Day Labourers at every 06:00 (201 "picked by" lines); `foreman_33807` commutes 77 min to the Spire yard. The picked are never seen working. |
| worker (Vat Tech, Bartender) | alive, flat | alive, flat | `worker_60545` 9.6 h work, 4.8 h travel, net +5c, identical d22-24. `worker_42267` is a 72-year-old bartender, 8.2 h work, one witness line all week. |
| courier | alive (one), jailed (one) | thin | `courier_48760 d21 11:48` six "PARCEL handed over at Bar#4464 ... fee 1c" in one minute: parcels exist now. `courier_67672 d23 15:49` arrested at his post, 5 days for Data Theft (`d23 20:11`), wage still paid at the arrest. |
| builder | thin, better | broken | `builder_13929 d21 09:21` GoTo(TargetHome) 176 min to Vat Farm#3940, ClerkWork there; 2.1 h travel, 8.8 h work, no 6-8 h hike. Still no building. `builder_92107 d23 19:54` Brawl at Gambling Den#5886: turned away drunk, swings, floors the bouncer, Enemy edge. |
| day_labourer | thin, alive as a person | dead | `day_labourer_81276 d22 06:00` "hired as Day Labourer (picked by Silas Ostrowski, the foreman)", 8.9 h work, +3c; works 3 of 7 days. `d27 11:26` Theft with five witnesses and a Report, +15c. |
| fabber | thin | thin | `fabber_88330` 8.1 h on a 10-min commute, Served x10, a HuntStarted. `fabber_37081` Spire/Vats commute 298 min each way: `d23` 13.5 h travel, 4.0 h sleep, net -5c. |
| loader | broken | thin | `loader_27669` commute 997 min, 17.9 h travel d21, 0 h work d21-23; Theft reported `d21 06:00`, arrested `d25 09:32`, 3 days. See bug 1. |
| cabbie | dead (one), thin (one) | dead | `cabbie_96284` homeless, 18, mood -1.00: `d26 05:00` "wakes for it and stays in bed". `cabbie_69395` 7.8 h at the Garage, only Fare Evasion witnessed (4 times). Zero rides. |
| worker_friday | mixed | mixed | `worker_friday_39479/_65719` both Depot Hands at one Depot in D6, homes in D1: 168 min each way, 6.8 h travel, 8.5 h work. One skip `d25 08:00` "sets off for it, then does not go in; walks the streets instead". |

## 3. The pass's mechanisms seen at 100k

Fired:
- **Shape-up / day labour (F2)**: `day_labourer_81276 d22 06:00`, `day_labourer_14274 d22 06:00`; 18-25 hires a morning from one foreman.
- **Skips (P1)**: `cabbie_96284 d22 05:00` "does not go in (mood -0.73, misery 1.00; absence 1 in 14 days)" and `d26 05:00`; `worker_friday_39479 d25 08:00`. Three skips in 140 person-days: restrained.
- **Parcels (F2)**: `courier_48760`, 47 handed over in the window, each with sender, recipient, fee.
- **Bouncer brawl (P1)**: `builder_92107 d23 19:54`.
- **Jail**: `loader_27669 d25 14:26` "sentenced to 3 days for Theft", 24 h a day in the Precinct d26-27; `courier_67672 d24`.

Expected, never seen in these 20: a misery quit, a Sacked firing, an unpaid walkout, a far-commute move ("took a post nearer home"), a laid-off builder met (0 LaidOff lines), a wage advance or loan (0 Loan), a skipper whose day became a story (the skips end in a walk or in bed).

## 4. Bugs, ranked by what the player sees first

1. **The far commute is still the day (V9 loader/fabber commutes: unchanged, plainer at 100k).** Six of 20 have a post 280-1000 tiles from home: `loader_27669` 997 min, `fabber_37081` 298, both `worker_friday` 168. `loader_27669 d21 06:05` GoTo(Workplace) 1113 min, 800 tiles, arriving long after the shift; `d22 20:53` home again 774 min. 13-20 h a day walking, 0-2.5 h at work, net -1c. F3's "far commute: took a post nearer home" never fired for any of them in a week. Likely cause: the hire deals the post by vacancy, not distance, and the quit clock does not run while the wage keeps landing.
2. **The shift closes under the walker (V9 bug 6, unchanged).** `loader_27669 d23 20:37` GoTo(Workplace) 572 min "ended" (cut); same at `d24 21:28` (521 min cut). The Work goal re-plans every few hours for a post unreachable inside the shift.
3. **Cabbies still drive nobody (V9 bug 2, unchanged).** `cabbie_69395`: 7.8 h of `Rest` at Garage#5812 booked as work, net 0c; four Fare Evasion witnesses (a fare exists as a crime, never as a ride). `cabbie_96284 d25 13:48` after the Garage he takes a 296-min GoTo(Squat) to the Sump to sleep: a homeless cab driver in D3.
4. **Builders and day labourers go to a random "site" (V9 bug 1, better, not fixed).** The leg is now 2-3 h not 5-6 h and stays in the district (`builder_13929 d21 09:21` 176 min; `day_labourer_14274 d22 09:23` 204 min to Noodle Bar#5381), but the site is a Vat Farm, a Noodle Bar or a Capsule Hotel. `day_labourer_81276 d22 12:52` ClerkWork 308 min at Capsule Hotel#5548 with target "Construction Yard#5328". Nothing is built.
5. **Hired but not working.** `day_labourer_14274 d27 06:00` "hired as Day Labourer (picked by Ottilie Singh, the foreman)", then GOAL Unwind, Rest x5 at home to 15:00, net -3c. The Hire event promises a day that does not happen (a rest day, or the post never reached him). He is hired 2 of 7 days, `_81276` 3 of 7, and nobody turned away is ever seen.
6. **A skip has no consequence yet.** `cabbie_96284` is homeless, starving (`d21 00:00`, `d25 23:25`), on a 6c wage, skips twice at misery 1.00, and nothing follows in the week: no boss line, no `Sacked`, though 2 absences in 14 days. The third would fire him; not reached.
7. **No quit, fire or walkout in 20 people over a week** on a harsh island. May be luck or the idle and stain gates; 20 diaries cannot say.
8. **Small jumps (V9 bug 8: better).** `worker_friday_39479` three "JUMP (a Full body) 4 tiles in 1 min" (`d21 19:32`, `d26 18:31`, `d27 17:31`), GoTo(Home). The fabber's daily jumps are gone.

Other V9 bugs: owner quits his own bar fixed per F3 (no owner in this group, not seen); Bury eats a shift not seen (no corpse leg in 20 diaries, unproven); unlabelled money not checked.

## 5. Dull spots, ranked (mechanism, not a knob)

1. **The cab has no fares.** Mechanism: a body that needs to cross districts (injured, wanted, late, drunk) hails the nearest cab and the ride is an event both remember. `cabbie_69395` already witnesses fare evasion four times: the evader should have been a fare.
2. **The shift is Unload + ClerkWork on one tile.** `worker_60545`, `night_shift_21867`, `courier_67672`: the same 9.6 h, net +5c, d21-24 to the minute. Mechanism: an incident per shift (short count, wrong crate, inspector, a coworker's debt) that leaves a memory. The parcel line shows the pattern works: `courier_48760` is the only one with "for whom".
3. **The long commute meets nobody.** Six people walk 3-17 h a day with no scene. Mechanism: a regular fellow rider, a fare dodger, a mugger, a stall; or a bed at the post (company bed, a Capsule Hotel by the yard) so the walker moves.
4. **The shape-up has no losers.** 18 picked, the turned-away invisible. Mechanism: the turned-away gather at the gate, the pick is by acquaintance, the passed-over fights or steals. `day_labourer_81276 d27 11:26` steals in front of five witnesses three days after a paid day: the story is already there, unconnected.
5. **The rest day is ten hours of Idle Rest.** `builder_13929 d27` 13.1 h idle, `cabbie_69395 d27` 12.6 h, `courier_48760 d27` 13.7 h. Mechanism: a weekly ritual with people in it (church, the Den, kin).
6. **Law is invisible to the suspect.** `loader_27669 d21 06:00` reported for Theft, arrested `d25 09:32`: four days in which he walks to work meanwhile and the officer is never seen. Mechanism: the officer turns up at the door or the post.

## 6. Too much

- **Walking.** 4-17 h a day of GoTo for six people reads as a sim glitch, not as grind. It is the one thing that makes these lives monotonous.
- **Identical days.** `worker_60545` and `builder_13929` repeat to the minute for a week (`builder_13929` d22 and d23 summaries are the same line). Machine-like, not harsh.
- Not too much: skips (3), brawls (1), arrests (2), fights (1) across 140 person-days. The labour group is nowhere near noisy.
