# Shadow V10 critique: vice (club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke)

Window: main `33f9c55`, 100k island, seed 42, d21-d27, two picks each (18 diaries; V9 had no fighter, joytoy or madam).
Cites are `file dDD HH:MM` under `target/shadow_v10/isle100k_s42_d21/`. Method: `camera.tsv` rows, grep of event words, full reads of the d23 commutes, bouncer_99482 d26, owner_broke_6315 d21-d25, companion_87767 d21-d22, d27 of four picks.

## 1. Verdict

Better than V9 in two places: the door now exists (one refusal, one ejection, an enemy made) and a failing bar is a chain of named events (loan missed, layoff, walk-out, confrontation). Dancers and companions now have regulars.
It is still mostly people living commutes. Five of eighteen picks spend 6-19 h a day walking, and on d27 every pick drops the whole shift again with no mark in the diary.
Stories start (a bounced drunk, a stiffed bartender who corners the owner) but none is picked up inside the window: Quill never comes back, Anton's grudge ends at 0.47.

## 2. Per archetype

- **club_staff: thin / dead (V9: thin / dead).** club_staff_36462 lives D4 Block#298, works D6 Club#4901: street 10.5 h and home 5.3 h every day d22-d26, work 6.2 h, identical to the minute (still 309 min d22..d26). `club_staff_36462 d23 00:02 GoTo(Home) 432 min, 244 tiles`, `d23 12:31 GoTo(Workplace) 292 min`. club_staff_38135 is thin: street 6-7 h, 4.5 h in a vehicle on d26, work 6 h.
- **bouncer: one alive, one dead (V9: thin, thin).** bouncer_99482 is alive: `d26 20:07 Served: Florian Ironside turned Quill Northcott away at the door of Gambling Den#5256 (drunk)`, then `Brawl: ... swung at Florian Ironside ... threw Quill Northcott out` and `new relationship with Quill Northcott#51264: Enemy affinity -1.00`. He also carries a dozen older Enemy edges. bouncer_59276 is dead: cancer stage 0.35, `Work: sick` on every goal line, work 0.0 h on six of seven days; only `d23 16:56 PLAN Work` at Fight Pit#3160. Nobody covers the post and nothing moves him toward care.
- **dancer: alive / thin (V9: alive, one story).** dancer_56001 and dancer_82810 work 5-6 h and tip, `dancer_56001 d26 21:00 Tip 1c` from three patrons, always at 21:00. dancer_56001 d27 flips `Idle -> Eat` every 30 min (8 times from `d27 09:21`). No story this week; V9's theft arc has no counterpart.
- **companion: one alive, one dead (V9: thin, thin).** companion_76861 earns: `d24 21:00 Served: Slate Bellamy is now a regular of Mirabel Ramirez at Spire Lounge#4865`, 16 Tip lines, purse 38c to 58c. companion_87767 is dead for three days: `d21 04:09 GoTo(Workplace) Block#2361 (D3) -> Spire Lounge#4867 (D5), 866 min, 428 tiles`, then `d21 18:39 GoTo(Home) 864 min`; street 19.8/18.5/19.5 h, work 0.1/0.0/0.0 h. From d24 she lives near work and earns: `d25 21:00 +12c (Tip)`, regular Wisp Underhill. `d26 12:01` gossip says she robbed Bronwen Greaves, with no Robbery line in her own diary.
- **cook: alive, flat (V9: one fine, one casualty).** cook_38504 and cook_82130 work 6.3-6.8 h in their home district, wages land (`d27 00:00 +6c Wage`), counters serve every night (`cook_38504 d26 22:15` three customers in one minute, `d26 21:30` "a regular"). No quit or wage failure this time. Nothing happens either.
- **fighter: thin (V9: absent).** Both go home, rest in 90 min blocks, `Enjoy` a Pit for 4c a visit, work 4.3-6.7 h. The job is ClerkWork at a Pit: no bout, wager, purse or injury in either diary. The only violence is a stranger's Assault seen at `fighter_43233 d27 00:07`.
- **joytoy: dead / thin (V9: absent).** joytoy_103306: `d23 11:17 GoTo(Workplace) Block#261 -> Dollhouse#5421, 384 min, 149 tiles` (D4 to D4), street 13.3 h on d22-d23, work 5.8 h; on d24-d26 work 9.7 h and vehicle 2.6 h. joytoy_64421 is thin: home 15-19 h, work 4-6 h. Both show "regular at: none" and no client or service line.
- **madam: thin (V9: absent).** Brothel posts at 16c/day. madam_41768 works 3-6.5 h, madam_84757 3.5-7 h (`d25 21:11` 169 min in place at the brothel). No girls, bookings, raid or tribute line. A madam who manages nobody.
- **owner_broke: alive / thin (new).** owner_broke_6315 is the best life in the group (section 3). owner_broke_6402: `d23 00:07 LaidOff` x2, `d24 00:07 UnderWater: Brick Kaneda is afloat again at Noodle Bar#5566 (15c in hand against 3c a day in wages)`, then 9 h a day at a Vat Farm while the bar runs staffless.

## 3. The pass's mechanisms seen at 100k

Fired:
- **Door (phase 1/5a refusal for cause):** `bouncer_99482 d26 20:07` refusal, ejection, enemy edge. One in 14 bouncer-nights.
- **Loan, layoff, walk-out, confrontation (phase 6):** `owner_broke_6315 d21 00:07 Loan: missed a payment on the loan from Militech 8 for Bar#4381 (2c of 41c; 159c still owed)`; `d23 00:07 LaidOff: ... let Anton Bartlett go ... (0c in hand against 10c a day in wages), owed 20c`; `d24 19:00 Confronted: Anton Bartlett caught Thea Fairweather ... demanded the 20c ... would not pay; ... (grudge 0.47)`; `d24 23:54 Quit: Tobin Oyelaran walked out ... (unpaid, owed 35c)`. A whole arc in four days.
- **UnderWater recovery:** `owner_broke_6402 d24 00:07`.
- **Regulars:** `companion_76861 d24 21:00`, `companion_87767 d25 21:00`.

Expected, never seen: protection or tribute at any of the two cooks, two bars, brothel, dollhouse, club or den (no smashed till, kicked stall or payment line); crew rest days for anyone; a client or service line for joytoy and madam; a Pit bout for a fighter; a refusal for colours or grudge; a closure or sale for either broke owner (6315 sits at 159c owed and just keeps working).

## 4. Bugs, ranked by what the player sees first

1. **Commute walks of hours (V9 bug 1: unchanged, 5 of 18).** `joytoy_103306 d23 11:17 384 min, 149 tiles` (D4 to D4); `club_staff_36462 d23 00:02 432 min, 244 tiles`; `companion_87767 d21 04:09 866 min, 428 tiles`. Pace is 1.8-2.6 min a tile, against about 0.5 for cook_38504's short hops. Same-district cases fail too, so the fault is walker pace or path cost, not job placement. It clips sleep (club_staff_36462 home 5.3 h).
2. **d27 drops every shift (V9 bug 3: unchanged).** All 18 picks have 0.0-0.6 h of work on d27 against 4-10 h on d26. `fighter_64376 d27` gets the 00:00 wage and then no shift; `cook_38504 d27 19:30 not in shift 0.00->0.30`. Not a rest day (no Crew line, no marker). Probably the last-day slice, but a player scrubbing d27 sees a closed city.
3. **A sick bouncer is invisible (new).** bouncer_59276: sick days 3/14, work 0.0 h six days, no quit, replacement, clinic or loan.
4. **Earn unplannable: ExpansionCap loops (new).** `cook_38504 d27 19:30`, cooled two hours, retried forever; the broke off-shift have no outlet.
5. **Idle/Eat flip (new, minor).** `dancer_56001 d27 09:21..10:21`: `pantry under a shift's meals` pulls Eat and the plan is interrupted 8 times with no meal.
6. **Regular spam (new, minor).** `club_staff_36462 d26 19:45 20:00 20:15 20:45` the same patron "a regular" four times in an hour.
7. **Robbed gossip with no event (new).** `companion_87767 d26 08:53`: gossip about her robbery, nothing in her own diary.

V9 status: commutes unchanged; unpaid cook quit fixed (both cooks paid; the wage failure moved to bar owners, where it is a feature); ReportCrime half-day walk not seen (better); jump lines gone, 0 jumps for all 18 in camera.tsv (fixed); d27 skip unchanged.

## 5. Dull spots, ranked

1. **Fighter, madam, joytoy have no product.** Each runs the generic ClerkWork block. A fighter needs a card: a bout against another fighter at the Pit, spectator wagers, a purse, an injury. A madam needs girls and clients: a booking pays her cut, with a rough client, a raid or crew tribute as the incident. A joytoy needs a client who comes back or turns violent.
2. **The door runs once a fortnight.** Every arrival should resolve against drunk, armed, colours and grudge, and the turned-away person should wait outside. Quill should be seen again; the Enemy edge of d26 has no sequel.
3. **The confrontation ends at 0.47.** `owner_broke_6315 d24 19:00` the stiffed worker leaves with nothing. Give the next step: a fixer or gang takes the debt for a cut, a beating on the owner's walk home, or a court claim, so the loan, the layoff and the unpaid wage become one story, and the owner can close or sell.
4. **Venue staff all go home alone.** After a shift, coworkers should drink at the neighbouring bar so a crew forms, with quarrels and walk-outs.
5. **The sick stay sick.** A bouncer with cancer should visit the clinic, borrow for the ripperdoc, or be replaced at the door by a hire.
6. **Cooks serve strangers.** The customers are anonymous Statistical people. A regular, an inspector to bribe or a gang member would make the kitchen a place.

## 6. Too much

- Nothing reads as noise. The one monotony is the metronome: club_staff_36462 repeats home 5.3, work 6.2, street 10.5 for five days to the decimal, which reads as a broken pace rather than a harsh city.
- The repeated "a regular" marker (bug 6) is spam, not a story.
