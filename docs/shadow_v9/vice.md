# Shadow V9 critique: vice (club_staff, bouncer, dancer, companion, cook)

Window: main `4108184`, 100k island, seed 42, d21-d27, two picks each (10 diaries). Cites are `file dDD HH:MM` under `target/shadow_v9/isle100k_s42_d21/`. Method: `camera.tsv` rows, DAY SUMMARY lines, grep of GOAL/GoTo/event words, then full-day reads of club_staff_101547 d22, cook_95294 d24-d27, bouncer_65259 d21 and d27, dancer_57317 d25-d27. fighter, joytoy and madam are absent at 100k.

## 1. Verdict

Seven of ten picks live a flat but sane life: home, a 6 h shift, an hour of HangOut, bed. Two others spend 10-20 h a day walking a commute, and a third is an unpaid cook who quits.
The V8 pathing defect (a walk of 500-700 min between a home and a job in the same district) is still the first thing a player would see. The venue trades still have no door, no client and no customer.
This is people living commutes more than lives. The one real story is a dancer who steals a motorcycle and gets six days (dancer_57317 d27).

## 2. Per archetype

- **club_staff: one thin, one dead.** club_staff_105931 (home 13-15 h, work 6.0, street 3, every day the same) is flat but fine. club_staff_101547 is dead: home Block#287 and job Club#5304 are both D4 Mid-Levels, yet
  > `club_staff_101547 d22 15:19 GoTo(Workplace) Block#287 -> Club#5304, 526 min` and `d22 00:02 GoTo(Home) 691 min`.
  She arrives at midnight for an 18:00 shift. Camera d22/d24/d26: work 0.1/0.3/0.1 h, street 16.2/16.3/16.0 h, Time-use travel 20.2 h. Wage still lands on the shifts she half works.
- **bouncer: thin (a statue).** Both keep the 18:00-24:00 post (work 6-7 h) and otherwise sit at home with 4-5 h a day of HangOut at Block#447 (bouncer_65259 d22 DAY SUMMARY: leisure 4.1 h, idle 5.6 h; 85 HangOut lines). Zero refus, door or eject lines in both diaries, at a Club (#5137), so the Pits-and-Dens gap is not the only cause.
- **dancer: alive, one story.** Both work 4-6 h and tip (dancer_57317 `d25 21:00` and `d26 21:00`, +2c each, at 21:00 as always). dancer_61686 Enjoys Club#5139 for 91 min at 06:19 on d23, d25 and d27: same minute, same club, same length. dancer_57317 is the best life in the group: purse 4c on `d26 21:00`, `d27 00:27 VehicleStolen: Elowen Ito stole Rhoswen Singh's Motorcycle T1`, witnessed and reported, `d27 10:49 Arrest`, `d27 13:21 sentenced to 6 days for Grand Theft`. Greed 0.78 plus no money turning to crime is the mechanism working.
- **companion: thin.** companion_106540 (home 12-18 h, work 3-6 h) shows no Tip line, only `was served by` as a customer (`d23 10:08`, `d26 11:50`). companion_106531 lives in D4 Block#607 and works at D5 Spire Lounge#5267: street 9-12 h a day, work 0.9-5.1 h, 2 HangOuts in the week. Her work block is the same ClerkWork as an attendant's. There is no client, booking or companion act in either diary.
- **cook: one fine, one casualty.** cook_18290 (Noodle Bar#1465, same district D1 as work, work 6-7 h, 102 HangOut lines, home 15-18 h) is sane and flat. cook_95294 is the casualty (bugs 1 and 2).

## 3. Bugs, ranked by what the player sees first

1. **Commutes of hours (still).** club_staff_101547 (above) is a pure same-district case, so the fault is the walker's path or a missing link, not job placement. cook_95294 is cross-district (home D3 Block#2612, job D1 Noodle Bar#2385): `d24 00:02 GoTo(Home) 271 min`, camera d24/d25/d27 street 13.1/19.8/17.2 h, work 0.1/0.0/0.0 h. companion_106531 street 9-12 h a day for the D4 to D5 hop. Same family as V8 bug 1; cause not found here.
2. **A cook is unpaid and quits with no cause on screen.** `cook_95294 d27 00:00 Quit: Wren Tanner quit as Cook (unpaid)`. Wage 5c came from owner Freya Nakamura at `d22 00:00`, then net 0c on d25 and d26 while she still worked 5.9 h on d26. Wealth 0.00, mood -0.32 by d26, then `d27 00:01 GOAL Contract` toward a fixer 133 min away. Check whether the owner ran dry or the wage flow broke.
3. **d27 drops the whole group's shift.** Group mean work was 5.2 h on d26 and 0.8 h on d27, all ten picks at 0.0-0.1 h on an 18:00-24:00 post. Examples: `cook_18290 d27 19:20 not in shift 0.00`; `bouncer_65259 d27 16:12 Sleep 270 min`, then `20:44 GOAL ReportCrime GoTo(Hall) 196 min` with no Work goal in between. I did not find the cause. If it is a designed rest day, nothing in the diary marks it.
4. **ReportCrime travels for hours.** `bouncer_65259 d27 20:44 GoTo(Hall) 196 min (run ended)`; `cook_95294 d24 12:31 ReportCrime (travel 15.80)`. A witnessed fare evasion earns a half-day walk to the hall at score 0.05-0.2.
5. **Jump lines.** cook_95294 `d23 13:47` and `d24 13:47`, 3 tiles in 1 min at `GoTo(Workplace)`: the kerb allowance again, not the sim.

## 4. Dull spots, ranked (mechanisms, not knobs)

1. **The door.** Both bouncers have 6 h of nothing a night. The door should see each arrival and refuse on drunk, armed, a rival's colours or a grudge. A refusal leaves a grudge in the turned-away person, who waits outside, and the bouncer becomes a target for a gang member on a later night.
2. **Dancers and companions have no clients.** Their diaries differ from an attendant's by a label. Give them regulars: a customer returns to the same worker, a patron edge grows, and it can end in a gift, a debt, a stalker, a rival's visit or leaving together. Tips are 1c in two lines at 21:00; a regular paying 10c would have kept dancer_57317 off a stolen bike.
3. **Money pressure on the venue trades.** dancer_57317 d25-d27 is the template: low purse plus greed plus an unattended vehicle gives theft. Add a fence or handler at the club so loot goes somewhere, and make the six-day sentence cost the post (the club refills it), so she comes back to a different job.
4. **Free time is a clone.** dancer_61686 repeats the same Enjoy to the minute, and bouncer_65259 HangOuts at home 85 times. After a shift, staff of a venue should drink together at a neighbouring venue with the same coworkers. A rotating crew forms, with quarrels, romances and walk-outs.
5. **A cook with no recourse.** cook_95294 can only quit when the bar stops paying. Unpaid staff should confront the owner, take stock, or strike with the others, and the owner should carry a ledger, so the quit becomes an event with a name on it.
6. **Cooks serve nobody.** Counter lines are 4 (cook_18290) and 2 (cook_95294) in a week, and the shift is ClerkWork like everyone else's. A real order queue (a regular, a gang member, an inspector to bribe) would make the kitchen a place.

## 5. Already in `PRESSURE_PASS.md`

- **Bouncer refuses at Pits and Dens too, and for cause (line 44):** seen in two bouncers, both zero refusals at a Club. The door has no mechanism anywhere yet, not only at Pits and Dens.
- **Shop owner under protection, shadow includes cook (line 60):** cook_95294's unpaid quit is the nearest evidence of owner-side pressure. No protection payment appeared in either cook diary.
- **Misery acts:** dancer_57317's theft d27 is a misery act already firing. cook_95294 (mood -0.32, wealth 0.00) picked Contract, not a misery act, at `d27 00:01`.
- **Not in the pass, worth adding:** the commute walks (bug 1), the d27 whole-group shift skip (bug 3) and clients for dancers and companions (dull 2).
