# Shadow V13: pressure pass round 4 at 100k (2026-10-28)

Round 4's read, in V12's window so the two compare one for one: main `a70800f` (`target/shadow_v13/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype plus round 4's new pick (`jobless`). Seven Sonnet critics
(`docs/plans/shadow_v13.md`) wrote `docs/shadow_v13/<group>.md`. Runs in `target/shadow_v13/`; the round's 30-day
100k counters in `target/r4_100k/analyze.txt` (round 3's in `target/r3_100k/`); the profile in
`target/r4_perf/prof100k.err`.

## Verdict

**Better than V12 in all seven groups, by less than V12 gained over V11; the gain moved from the edges of a life to
its middle.** What round 4 fixed shows at 100k:
- the Camp is a place: the wardens' nightly headcount, a child caught at the fence and walked back in, slip-outs,
  meals, a visit let in; the service's warning in the `[family]` line before a take (street; T1);
- the jobless search reads as a week: eight Friends asked, two boards, taken on as a Clerk at a door the next day
  (`jobless_6204` d21-22; labour; T5); a purist turned away at four dawn gates, then hired at a door (gangs);
- the door, the stage and the floor print their night (`Served`), the owner's creditor chain runs (vice; T4);
- a held contract replans and cools with a reason, a buyer's 1,912c hit is taken by two crews who fail with a reason
  (crime; T3); arrests complete and name the victim (corp);
- the runner is alive: a ledger hacked, the books audited, the repeat matched, the arrest (gangs; T6); a treatment's
  payer and price agree (street; T6); the CEO's repeated call is 2-3 a week, not 15 (corp; T6);
- the sick picks carry cancer and their arcs read: begging, eviction, a gang, a back-room cut (care; T7's pick);
- gossip is 0-7 lines a diary, was 22-40 (vice; T7).

**The no-harm rule held in every diary**, and for the first time a critic could check it: every harm line prints
ages (T7), and no critic found a minor as a victim, in vice, sick to death or dead in 132 diaries; `harm_blocked` read
0 for the 30-day run. The starvation deaths below are all adults (`kill_by` refuses a protected death and counts it).

What still reads wrong: the trades are labels (forgers, fixers, pimps, ripperdocs), a guard inside the Precinct sees
the street it cannot reach, and walks still eat the day.

## Next pass, ranked by what the player sees

1. **Witnessing ignores walls** (`OPEN-233`'s leftover, three groups). A body inside a building witnesses street
   crime by tile distance: a man in the cells all week logs 31 Fare Evasion sightings, up to 13 an hour
   (`gang_member_6738`); a guard inside the Precinct witnesses 33 Assaults, 4 Murders and 4 Grand Thefts and arrests
   none, the one Arrest goal `Unreachable` (`guard_27360`; corp); jailed picks (crime). A sim fix: who is indoors
   sees indoors. Witness 190k -> 269k at 100k.
2. **The trades are still labels** (`OPEN-217`). Forgers and fixers do ClerkWork (crime); pimps run gang Extort
   errands, no round, no booked client (crime); ripperdocs see 0 and 1 patients a week, 8.8 h of ClerkWork or Rest,
   and the orderly's 34 installs land at 00:00 (care); the 100k door lets in 125 and turns nobody away in 11 nights
   while the madams bar rough clients nightly, and the dancer plays to an empty room six nights in seven (vice); no
   arms_dealer diary for the second round.
3. **Contradictions a player would catch.** A raider "sat out the raid" while his raid marches (`raider_84195 d21
   00:07`; gangs); a taken child still "at home" in the family line hours later (`camp_parent_23527 d25`; street);
   the warning's household is not the child's own (`reporter_57837 d27 00:10`, Rin Duarte "2 days unfed" at
   Block#3363, her home Block#205 with 17-28 in the pantry; corp); the thief gets the "was robbed" memory and the
   grudge (`homeless_71553 d21 16:26`; street); a contract posts a guard on its own taker (`runner_94196`,
   `gun_41438 d23 00:09`); CabFare names one Friend as payer for every rider (`cabbie_73680`; labour); a shakedown's
   victim witnesses his own robbery, and its money line reads `other` (labour); a sick worker is foreman of her own
   cover hire, laid off seven minutes later (`sick_47483 d21`; care, street).
4. **The neglect ladder's edges** (`OPEN-231`). A warning hangs open all week while the parent is in the cells
   (`dancer_99218`; vice); a family can be warned, slip, and be warned again with the children unfed 2 days each time
   (`super_102460`; care). The ladder is Dylan's call (warn first); the jailed parent is a bug.
5. **The search's edges** (T5). A man released with his post gone goes straight to JoinGang, no search
   (`loader_74097 d26 16:13`; labour); boards read 14 times with no hire and no escalation (`jailed_70409`; crime); a
   jobless pimp searches like a labourer (`pimp_84453`; crime).
6. **Walks still eat the day** (`OPEN-221`). The joytoy's 315-min commute (vice); a cabbie's 162-min Spot walks, 9.3 h
   of travel for 0.6 h of work (labour); the foreman's DawnPick fires with him asleep 72 min away (labour). The map
   re-import is the other half.
7. **Noise.** "Spent a day off with the crew" once per attendee, 73 lines in one diary (gangs); 178 BinContest lines,
   24 in half an hour (`miserable_7493`; street).
8. **Unseen consequences.** A CEO strips 585c off a corpse unwitnessed while an exec's 8c is reported the same minute
   (`ceo_20314 d21 08:20`; corp); a jailed exec is still paid his ExecWage (corp); a stakeout of one block for seven
   days, 12 aborts, no result (`gun_41438`; crime); jail days are blank (street).

## What round 4 did to the city (100k, seed 42 x 30, `r4_100k` against `r3_100k`)

Printed, not gated. Employed mean 39.9k -> 40.1k, Materialised 655 -> 597 (T5), thefts 16.7k -> 18.7k, arrests 9.4k
-> 10.3k, jailed mean 1,065 -> 1,141, deaths by violence 441 -> 431, **starvation deaths 11 -> 30** (23 on day 29;
below), homeless max 11.9k -> 13.1k, Starving events 8.3k -> 13.4k. ChildTaken 3,726 -> 677 and ch_camp daily mean
1,782 -> 91 (T1), ch_street 105 -> 266, CampEscape 211 -> 20; school days / child-days 4.0 % -> 6.5 % (T2; 15.1 % of
school-age school days, the 18 Schools full). Witness 190k -> 269k, Served 19.6k -> 25.9k, Story 7.1k -> 4.1k,
GrudgeFormed 11.4k -> 6.1k, Crew 21.0k -> 18.2k; new: JobSearch 83k, DawnPick 11.2k, Pursuit 4.1k, Custody 1.2k.
Ticks/s days 25-29 22.4 -> 20.8 (30 days in 1,748 -> 1,865 s wall); 20k on a quiet box 218 -> 192 (`OPEN-229`).

**The profile** (`--features prof`, 100k days 25-27): `goal::GangWork` scoring is 21.7 % of the profiled time (45.8 s
in three days, 3.66M calls), ahead of `goal::Unwind` (11.3 %), the population study's top goal. The one hotspot that
jumps out (`OPEN-229`).

**The starvation burst** (a Sonnet diagnosis, read-only): day 29's 23 deaths are the 7-day echo
(`[sickness] starve_grace_days`) of one hunger ramp on day 22 (166 Starving at tick 32,700; died 42,798-42,833). Not a
food shortage (the warehouse at its 1M cap all run): the dead are broke adults, every body stripped for 0c, ~17
evicted in the last fortnight, 7 housed with `RentShort 0 of N`, ~7 just out of the cells. 4 of 30 in a warned (T1)
household, 8-9 searching (T5): neither phase is the cause. Round 3's run had the same ramp two days later (Starving
days 18-21: 8/11/24/198 against 51/142/360/529), so fewer deaths fell inside its 30 days; round 4's higher
rent-short, evictions and doubled School fees (T2) start it sooner. 20k seed 42 x 30: 73 Starving, 0 deaths (a 100k
effect). The gap: a broke, evicted or released adult with no meal source that works for a week; the Mission and alms
meals reach 200-350 a day against 1,300+ starving (`OPEN-235`). No minor among them (`kill_by` refuses a protected
death; `harm_blocked` 0).

## Calls for Dylan

- **Schools at 100k**: 18 Schools for ~11k children (fewer than 20k's 20; `schools` counts per Lot copy), every one
  full. Seed by child count, or found Schools on demand?
- **The 2c fee** keeps most children in reach at home at 20k (a teacher's own child among them).
- **The neglect ladder**: a child goes 2 days unfed before the warning, and again before a take; warned, slipping and
  warned again is possible with no take. As built on his call; does it sit right with the under-18 rule?
- **Ticks/s** 20.8 at 100k and 192 at 20k with round 4 in; the GangWork hotspot is a candidate for a small perf phase.
