# Shadow V17: pressure pass round 8 at 100k (2026-10-30)

Round 8's read, in V16's window so the two compare one for one: main `389bd1a` (`target/shadow_v17/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype (V16's list plus X2's `bereaved` and `marked`; 117
diaries: the `day_labourer` pick found nobody, X6's picker's "nobody does its thing"). Seven Sonnet critics
(`docs/plans/shadow_v17.md`) wrote `docs/shadow_v17/<group>.md`. Runs in `target/shadow_v17/`; the round's 30-day
100k counters in `target/r8_100k/analyze.txt` (round 7's in `target/r7_100k/`).

## Verdict

**Better than V16 where round 8 aimed, and the city is hungrier.** The law keeps its word (promises end in a desk or
a reason line in every group), the cells have a day (meal, yard, kin, the release line), a death in the house is
mourned (leave, a funeral, the children's line: care's `bereaved_6559` is the round's best story, a guard's wife dead,
then theft, the cells and the children to the Camp), the hit's target hears and lies low or takes a guard (both
`marked` diaries), the pit's loser is hurt on a beating only, marriages are flat by week, Succession lines print, the
pickers find doers. **But starvation tripled** (13 -> 45 deaths, Starving 7,272 -> 11,423 at 100k): street, labour
and crime all show the same cause, a starving person's Eat dropped within the half hour for Earn, a Pitch to a job
board or Idle (0.05), fed only by the midnight soup line. Corp and the desk did not move (round 8 did not aim there).
**The under-18 rule held**: no minor in any harm, cell, vice, courtship or marriage line in 117 diaries;
`harm_blocked` 0 over 30 days.

## The 30-day counters (100k seed 42 x 30, round 7 -> round 8; FINDINGs, nothing tuned)

| | r7 | r8 |
|---|---|---|
| starvation deaths / Starving events / SoupLine | 13 / 7,272 / 7,131 | **45 / 11,423** / 9,226 |
| ChildTaken / ch_camp daily mean / warnings past 14 days | 3,703 / 1,053 / 18 | 4,229 / 1,336 / 27 |
| school days / child-days; births | 15.2 %; 274 | 12.3 %; 158 |
| Marriage (by week) | 23,267 (16.7k in d0-6) | 8,494 (1,885 / 2,451 / 2,086 / 1,672 / 400) |
| Reports / Arrests / jailed mean / Murder | 15.5k / 11.2k / 1,201 / 256 | 19.3k / 13.9k / 1,373 / 244 |
| promises to tell: made / kept / dropped with a line | - | 282 / 197 / 82 |
| witnesses held or asleep / hits across the cell wall / asleep blackmail | - | 0 / **1** / 0 |
| cell cases; jail-days with a line | -; - | 61; 37.4k of 37.4k |
| Bereaved (mourners) / Succession lines | - / 153 | 801 / 406 |
| war: called / came; raid marchers at the door | 3,297 / 1,586; 702 of 786 | 1,904 / 1,141; 655 of 717 |
| bouts / losers hurt / purse coins | 1,724 / 1,724 / 10.7k | 1,733 / **509** / 27.6k |
| Clinic-days with a Ripperdoc / of them no patient | 1,861 / 853 | 1,850 / **560** |
| vehicle thefts (VehicleStolen) | 2,354 | 729 (X3's employed gate) |
| JobSearch / PlanAborted / Cells lines | 84k / 887k / 6.3k | **223k / 1.16M** / 84.5k |
| arms lots sold by a dealer (`sold_dealer`) | 8 | 0 |
| harm_blocked | 0 | 0 |
| ticks/s mean (shared box: a print) | 17 | 18 |

Births fell with the marriages (X5's courtship; a known cause, not tuned). JobSearch tripled with X2's needy who keep
searching; all 45 who starved had board reads, but the diaries put the cause in Eat's replan, not the boards.

## What reads right (quoted in the group reports)

- care: `bereaved_6559` (the funeral, leave, theft, the cells, the children taken); promises end in a report within
  hours or "the law is out of reach"; no care worker steals a motorcycle; the super's one-floor loop gone.
- crime: both `marked` hear, lie low, take a guard (a guard beat a hit crew 3-2, `marked_66443`); the cells' meal,
  yard, kin visit, a yard fight that adds 3 days, a held witness at the far wall; the pickers find real doers.
- vice: all five "means to tell" resolved within hours; `dancer_76642` jail, sacked, release, the boards; the pit's
  loser hurt on 509 of 1,733 bouts, the purse 14c.
- labour: both jobless hired within 2-4 days; a beaten builder reports within 17 h; a gang foreman mustered off his
  shift to a raid that wins.
- gangs: every leader change a Succession line (31 in 16 diaries); the release line and the cell's day fire.
- corp: exec commutes 0.2-2.5 h (V16 8-12 h); arrest lines print ages; `office_worker_18717` shaken down twice,
  "tells nobody: afraid", then fights on her rest day when the odds turn, and wins.
- street: the cell's meal, yard, visit and release in every jailed diary; the jail's middle no longer blank.

## Round 9's input, ranked

1. **Hunger loses to everything** (`OPEN-249`, new): a starving person's Eat is dropped within the half hour for
   Earn, a Pitch (X2's "a door to ask about work") or Idle at 0.05 (`neglect_parent_35940 d23 13:30-14:00`,
   `jobless_6204 d21 11:45`, `d22`, `sick_49012 d21`, `homeless_86438 d21`); the bin walk cut at its first step and
   replanned away; Eat cooling after a failed dumpster dive; the home meal at one fixed minute; a wounded target
   starving 72 h in a clinic while kin visit four times with no food (`marked_22051`). 100k: starvation 13 -> 45,
   Starving 7.3k -> 11.4k, ChildTaken 3.7k -> 4.2k. Find why Eat loses at hunger 1.0 and fix it there.
2. **Round 8's contradictions** (`OPEN-250`, new): a guard on mourning leave makes arrests (`bereaved_6559 d21
   09:19`, `d23`); the funeral's "no friend near to ask" with a Friend in the Block; the "out of the cells and runs it
   again" line names a different man (`purist_32297`, `purist_9284`, `dealer_105046`) and a successor takes over from
   the cells (`pimp_9933 d24 03:43`); "lies low at home" then an 875-min walk to collect (`fixer_23959 d23 09:19`);
   "never told: out of reach" then a report the same day (`night_shift_24245`); a cell murder with no victim, Death or
   case line (crime); a hit's Assault line names the victim twice; one hit across the cell wall at 100k; a man with
   cancer or fresh off an eviction hired as Guard (`sick_49012 d26`, `neglect_parent_80351 d27`).
3. **The marriage move-in and far travel** (`OPEN-240`, via X5): a companion married into a Home 8 h from her post
   (`companion_87767 d23`); a 75-y leader walking 14-20 h a day to his own den (`gang_leader_17198 d27`), a 64-y fixer
   13-23 h; cabbies' Spot legs (`cabbie_104468`, 45 legs for a few fares); guards' 9-12 h patrol walks
   (`guard_8429`). The move-in reads the post now; the rest waits for the map re-import.
4. **The law's force** (`OPEN-251`, new): a joytoy with no kit fights off a guard five times in one evening and no
   second guard comes (`joytoy_123639 d27 15:32-23:49`); a sanctioned fare beat gossiped as "Assaulted by" and a
   grudge sworn on the guard (`guard_8429 d22-26`); bylines read as the reporter's crime (`reporter_59309 d23 00:01`,
   V16's).
5. **Gangs** (`OPEN-241`): no raid lands in the gang diaries (members 5-7 h from the corner and the muster;
   `purist_9284 d27 14:06` sits out a muster 6 h 51 away: X4's left-open, the members' homes); the leader's chair
   churns (four Successions in 3 days, `gun_89965`); member counts that jump 2-3x in a week; a frozen identical-day
   loop (`gang_leader_63568`).
6. **The Camp's count** (street): Camp#5132 grows 2,644 -> 3,656 children in five days while its Custody line says
   "64 brought in" a day (`camp_warden_56621 d21-26`); a Camp of 3,656 with two wardens reads absurd.
7. **Dull**: arms dealers sell nothing (`sold_dealer` 8 -> 0); a rich madam never works (Work 0.031 at 169c, quits
   "no need"; `madam_104391`); the corp-district Clinic (one patient a week, `ripperdoc_64026`); the desk, the cook,
   club staff, the shift floor (8 of 20 labour diaries nothing but Witness lines); the forger's 1c week; cover shifts
   printed as Hires; the `day_labourer` picker empty at 100k.

**Dylan's calls, carried:** the neglect ladder's size at 100k (`OPEN-231`) is left as built; ChildTaken rose with
the hunger (item 1), so the ladder's input moved, not the ladder.
