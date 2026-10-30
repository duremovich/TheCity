# Shadow V16: pressure pass round 7 at 100k (2026-10-30)

Round 7's read, in V15's window so the two compare one for one: main `128d5b7` (`target/shadow_v16/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype (V15's list, 227 diaries). Seven Sonnet critics
(`docs/plans/shadow_v16.md`) wrote `docs/shadow_v16/<group>.md`. Runs in `target/shadow_v16/`; the round's 30-day
100k counters in `target/r7_100k/analyze.txt` (round 6's in `target/r6_100k/`).

## Verdict

**Better than V15 in every group where round 7 aimed, and the desk and the street floor are still dull.** Wars end
(a truce, a crackdown, "went quiet") and members hear the call to lie low; guards arrest (4 and 2 a week against 1),
the fare is one dodger at a time and the frail are booked; crimes get their case (both forgers arrested within hours
of the report, a buyer charged with conspiracy); the pit has stakes and the door a second reason; the GOSSIP flood is
gone (259 -> 5 lines in the vice diaries); the orderly no longer installs chrome; the School fee reads the midnight
purse. Care has the round's best stories (`sick` 74-y cancer patient who joins a gang, sits on a witnessed assault,
blackmails the assaulter and is leaned on until the report is withdrawn). **The under-18 rule held**: no minor in any
harm, vice, corner-fight, war or fare line in 227 diaries; `harm_blocked` 0 over the 30-day run; the one hit target
whose age no line printed (`night_shift_23475 d27 18:00`, Fenn Coldwater#22660) is an adult (`contracts::target_ok`
refuses a protected mark and needs a Brain; the 12-y Fenn Coldwater is another resident). W1's household shopping
fires but is nearly invisible in the diaries (one goal line, `day_labourer_77349 d22 12:09`; the `[family]` fed/unfed
tag in none), and at 100k the ladder still takes ~3,700 children in 30 days, almost all of broke households (Dylan's
call, below).

## The 30-day counters (100k seed 42 x 30, round 6 -> round 7; FINDINGs, nothing tuned)

| | r6 | r7 |
|---|---|---|
| ChildTaken / ch_camp daily mean / ch_street | 4,103 / 1,188 / 69 | 3,703 / 1,053 / 76 |
| warnings standing past 14 days (daily mean) | 72 | **18** |
| starvation deaths / Starving events | 18 / 7,416 | 13 / 7,272 |
| school days / child-days | 15.2 % | 15.2 % |
| deaths by violence / Murder / CorpseFound | 482 / 284 / 481 | 436 / 256 / 436 |
| Reports / Arrests / jailed mean | 16.3k / 12.1k / 1,332 | 15.5k / 11.2k / 1,201 |
| Assault / Extortion / Raid / TerritoryFlipped | 5,753 / 1,116 / 216 / 82 | 5,560 / 1,025 / **317** / 55 |
| war: called / came; members' war lines; raid marchers at the door | - | 3,297 / 1,586; 16.3k; 702 of 786 |
| sleepers robbed / woke to it / reported / kept quiet | - | 509 / 455 / 76 / 325 |
| witnesses who found the body: named the killer / kept quiet | - | 6 / 23 of 40 |
| door refusals; off screen broke / trouble / colours / barred | 7,113; - | 8,927; 6,688 / 432 / 299 / 29 |
| bouts / purses / losers hurt | 1,677 / - / - | 1,724 / 1,723 (10.7k c) / 1,724 |
| pimps daily mean / street visits / named pimp-days with a round | 1.1 / 466 / - | 1.2 / **784** / 26 of 32 |
| Clinic-days with a Ripperdoc / of them no patient; care_doc_cut | - | 1,861 / 853; 38.3k c |
| Marriage | 23,502 | 23,267 (16.7k in days 0-6) |
| harm_blocked (a minor reached the kill backstop) | 0 | 0 |
| ticks/s mean (shared with the shadow run: a print) | 16 | 17 |

Arrests fell at 100k while W3 raised them at 20k (1,121 -> 1,325): W5 took sleepers off the witness roll (20k
Witness lines 12,100 -> 9,957, arrests 1,315 -> 1,200), a known cause, not tuned.

## What reads right (quoted in the group reports)

- gangs: `gang_leader_61823` war, crackdown, lie low, arrested at the Hideout, sentenced, a successor and a grudge;
  members hear the lie-low call; wars end in truces and "went quiet".
- corp: `reporter_53592` steals a motorcycle, is reported, two guards give up the chase, arrested on the beat, 6 days;
  a frail 67-y booked not beaten; a CEO mother's "children's food short" goal feeds her son after the buy-out.
- crime: both forgers sell and are arrested within hours of the report; a buyer charged with conspiracy, 23 days; a
  gang leader loses his fixer office to the loan while in the cells.
- labour: `night_shift_28130 d23 19:25-d25 02:04` woke short, told a stranger on the street, the thief "learned who
  put him inside"; builders no longer sent 200 tiles a day.
- vice: the pit's purse, the hurt loser and the patch-up, a three-bout rivalry; the door's "trouble";
  `owner_broke_6471`'s creditor comes back; `madam_35331` jailed, fired, turned away by a gang.
- care: the blackmail arcs (`sick`, `orderly_20962`), a widow's grudge sworn within 3 h of the killing; W4's doctor
  cuts and is paid.
- street: a 20-y gang member's war and death; W3's arrest at the door; W5's Camp Escape.

## Round 8's input, ranked

1. **"Means to tell the law" is a broken promise, and the held and the asleep still act** (`OPEN-246`, new): 455
   sleepers woke to the short purse at 100k, 76 reported; in the diaries 5 of 7 who "mean to tell" never do
   (`foreman_21236 d23, d27`, `courier_45596 d24-25`, `gun_44357` three times then attacks instead, `cook_25080`,
   `dancer_104874`). A man in a cell witnesses 14 crimes after his sentence (`ripperdoc_10662`), a victim beats the
   thief while he is held in the Precinct (`street_worker_35520 d22 18:16`), cell fights and a Murder with no place,
   parties or case (`pimp_14518 d26 06:18`, `black_doc_79637 d27 07:16`), an orderly blackmails while asleep
   (`orderly_20962 d26 12:50`).
2. **Far travel at 100k** (`OPEN-240`, `OPEN-221`; the map re-import's): execs walk 4-6 h each way (`exec_78540`,
   704-tile legs; worse than V15); a guard walks 4-5 h to the start of a patrol (`guard_26834 d22 21:14`; the route
   is built per shift, not from where he stands, W3's left-open); a cabbie sacked as a "no-show" mid-walk on 94-125
   min Spot legs for one fare a shift (`cabbie_80901 d22 00:07`); a move-in with family 481 tiles from work, 9-10 h a
   day for six days (`worker_friday_35814`); "moved nearer work" moves a dancer farther (`dancer_104874 d25 00:12`),
   then he sleeps at the old Home (`PLAN FAILED: Sleep PreconditionLost`).
3. **The target and the bereaved do not react** (`OPEN-247`, new): a 1,453c hit taken on a sleeping CEO who idles on
   (`ceo_20314 d27 18:00`, V15's, unchanged in kind); a widow's hunt cools in 16 min and she is at work the next
   morning, no funeral, no missed shift, her children no line (`school_parent_87579 d25 21:00`); a fired cabbie and a
   day labourer never search all week; "Quit (no need of the wage)" then four job searches the same day
   (`gun_44357 d27`, 255c).
4. **The wrong person in the role, II** (`OPEN-245`'s follow-up): a man three days out of a theft sentence hired as a
   Guard (`camp_jobless_66381 d27 11:30`); a 61-y ripperdoc and a teacher steal motorcycles to earn (`ripperdoc_10662
   d24 04:34`, `teacher_14781 d25`: the Earn crime plan winning at 0.05); a Fighter the foreman of Fighter hires (the
   `no_foreman` list); a murder after a pit brawl names the bystander who reported it (`miserable_103693 d26 20:15`).
5. **Gangs** (`OPEN-241`): the lie-low line names a different enemy than the war (`cultist_80865 d26 00:05`); leaders
   change with no Succession line (`raider_29990`, `scav_99001`); no raid lands in 16 diaries (the muster point
   ignores who is near; the walk-off is gone, members now sit out "3 h 08 from home"); corner turnout 1,586 of 3,297.
6. **The pit and the lines** (`OPEN-217`): every bout hurts its loser (1,724 of 1,724) and the patch-up (6-12c) costs
   more than the purse (5c); the purse is `GambleWin` from the City, not the house; bout, arrest, cell-fight and
   witness lines print no ages (the minor rule cannot be read on them).
7. **Marriages** (`OPEN-248`, new): 23k in 30 days at 100k, 16.7k in the first week (r6 the same): a seeding burst
   and the off-screen flirt rate, reads as noise.
8. **Dull**: the clinics (853 of 1,861 doctor-days with no patient; walk-in patch-ups still to the nearest); the desk
   (execs, office workers), cook, joytoy, club staff; half the street group a clean slide (homeless, sweepers, street
   workers, the neglect and camp parents); both `gun` picks corp techs with no weapon, the `runner` picks not gang
   members (the archetype pickers); no arms_dealer diary for the fifth round; jail days empty.

**Dylan's call, carried:** the neglect ladder at 100k takes ~3,700 children in 30 days, 3,274 of W1's 3,430
second-warning takes from households that cannot pay one night's food (`OPEN-231`; W1's entry). The fix for families
with coins is in; what is left is the ladder's size on the broke.
