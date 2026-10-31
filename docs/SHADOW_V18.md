# Shadow V18: the pressure pass's exit read (2026-10-31)

Round 9's read (Y1 hunger in the brain, Y2 contradictions that are state, Y3 the narration, Y4 the walking pace) and
**the pressure pass's exit** (`docs/PRESSURE_PASS.md` § "Round 9 from Shadow V17", "The exit"), in V17's window:
main `621882a` (`target/shadow_v18/sha.txt`), the 100k island, seed 42, days 21-27, two picks per archetype (237
diaries; the `pimp` pick empty, an error line). Eight Sonnet critics (`docs/plans/shadow_v18.md`): V17's seven
groups and a **stories** critic over every diary and the 30-day event log. Reports in `docs/shadow_v18/`; runs in
`target/shadow_v18/`, the 30-day 100k counters in `target/r9_100k/analyze.txt`.

## Verdict

**Better than V17 everywhere the round aimed, and the city is harsher.** Nobody starves (100k starvation deaths 45
-> 0, Starving 11.4k -> 1.1k); every hunger sample at 0.10 or below in street ends in a meal, a dry pot sends the
body to the next source. **People walk like people**: commutes 1-14 min, guards' patrol legs 6-21 min (V17 207 min),
leaders no longer walk 14-20 h, musters are reachable, **a raid lands in a member's diary** (`runner_41936 d23
22:37`, 19 raiders on 126 defenders, held). The second guard comes (`guard_9368 d26 21:40`); V17's byline, the
madam who never works, the five failed arrests on a joytoy are fixed. Clinics see 16-21 patients a week (V17 one).
**But the law now reaches everyone in minutes**: Arrests 13.9k -> 23.1k, jailed mean 1,373 -> 3,133 (max 6,747, ~6 %
of the city); half the gang picks sit 5+ days in the cells, both gang leaders six of seven days; cell days are the
only identical days. Violent deaths 464 -> 756, Murders 244 -> 389. **No minor harmed** in 237 diaries; 0 of 389
Murders under 18.

## The exit

| | verdict | best cite |
|---|---|---|
| 1. a friend killed, a friend goes after the killer | **mechanism only** | events `38862`-`38995`: Liesl Draper kills Joss Inchbald, her husband Moth Wexford's grudge, HuntStarted, "avenged Joss Inchbald on Liesl Draper (killed)", the law names Moth; in no diary. The diaries stop at the grudge: `bereaved_30486 d21 00:09` "goal Hunt cooled until d24 00:09 (could not plan or plan failed twice)"; `purist_12613`, `joytoy_47077`, `foreman_21236`, `elder_58897` swear and never hunt. 30 days: 2,361 killed-kin/friend grudges, 150 HuntStarted, 63 Avenged |
| 2. word of a killing gets around, someone acts | **shown** | "Avenged by Odessa Mbeki on Bertram Novak" retold through `builder_87052 d27` to `raider_101991 d27 17:35` at two hops; Odessa's arc in events `36096`-`37441`; a target hears and hires a guard (`2520`) |
| 3. a hit as ordinary business | **shown** | `fixer_26141 d23 06:05` Odile Fletcher posts a beat on Wisp Sato for 364 (182 down, "revenge") after her gang's Adela lost to him (`cultist_11723 d23 00:07`); The Ash Widows take it `d25 18:00`; "done ... won (3 vs 3) ... the cut 41c" `d25 22:28` |
| 4. an open contract an NPC takes | **mechanism only** | 1,499 taken in 30 days (592 done, 784 failed), but a take is a line at the 18:00 or 00:01 contract tick (`fixer_26141 d23 18:00` "Cordelia Mbeki took the beat"): no taker reads a board, decides or walks to the fixer; 57 % of takes are corp ledger |
| 5. a life that makes sense on inspection | **shown** | `raider_101991 d24-27` (a cancer loan, the dry pot, a hire, a till stolen, a remission on the doctor's slate, an assault, 10 days); `teacher_47553`; `forger_90429`; `neglect_parent_37754`; `cook_22421` |
| nobody starves while Eat loses to a non-survival goal | **holds** | near misses above the starving line: `foreman_21729 d21 08:09` Work 0.936 over Eat at hunger 0.27 -> 0.10; `raider_101991 d21` retries one bin; `school_parent_68009 d21 13:01` StockGone, no retry |
| no frozen identical-day loop | **holds outside the cells** | cell days identical in nine gang and four crime picks; `jobless_6208 d23 = d26`, `day_labourer_89859 d22 = d26` to the minute (the gate, nothing earned) |
| no minor harmed | **holds** | the edges, for Dylan: a 2-y-old on the street five days after an eviction (`sweeper_14150 d23-27`: the service reacts only to unfed children); an 11-y-old "2 days unfed" while both parents are jailed, one with 235c (`forger_90429 d27 00:10`); the ladder's 2-day grace (`OPEN-231`, decided) |

**Stories 1 and 4 get one targeted phase each** (round 9's Z1, Z2); then the pass closes.

## The 30-day counters (100k seed 42 x 30, round 8 -> round 9; FINDINGs, nothing tuned)

| | r8 | r9 |
|---|---|---|
| starvation deaths / Starving | 45 / 11,423 | **0 / 1,138** |
| ChildTaken | 4,229 | 3,180 |
| violent deaths / Murder / assaults+murders a day | 464 / 244 / 182 | **756 / 389 / 338** |
| Arrests / jailed mean / max | 13.9k / 1,373 / 3,513 | **23.1k / 3,133 / 6,747** |
| thefts | 25.5k | 15.3k |
| Raid (by week) | 335 (0/27/51/166/91) | 441 (16/100/170/135/20) |
| Marriage / births | 8,494 / 158 | 8,345 / 164 |
| JobSearch / PlanAborted | 223k / 1.16M | 295k / 1.64M |
| VehicleStolen | 729 | 1,093 |
| homeless max / employed mean | 16.8k / 40.4k | 12.3k / 40.8k |
| ticks/s mean (min) | 18.2 (15) | **14.2 (11)** |

## Beyond the exit (to OPEN, no round of their own)

- **The cells swallow the week** (gangs, care, crime): the law at walking speed lands every report; 6 % of the city
  held, posts lost, identical cell days, BreakOut orders that never act. A feature question (bail, fines paid from the
  purse, overcrowding, the cell's day varied), not a knob.
- **Hits fail at the door and the target never hears** (`marked_30280 d21-22`, both guns): no "got word" at 100k.
- **The wrong person named**: a Cells line names an old reporter (`night_shift_11299 d26 13:12`); the law credits the
  wrong killer (`sick_42893 d23 03:01`); a Harvested victim gossiped "unnamed".
- **OPEN-252's four, seen at 100k**: workers asleep at the shift's start (`night_shift_10386 d21`, `cabbie_102659`,
  `foreman_21729 d26 05:36`); married and out all night (`courier_19618 d23`, `day_labourer_89859`); cabbies earn
  nothing; the Camp buy-out never exercised (nobody holds 60c).
- **Vice**: companions work 6 h a night and serve nobody; a barred client booked nightly (`madam_102688 d22-26`);
  a loan for the bar spent on the owner (`owner_broke_6722 d22`); founding stalls at a full Station.
- **Corp**: a guard idles a whole shift (`guard_29797 d26`) and arrests his own dealer; the reporter with no work.
- **The shadow tool**: the JUMP audit flags escorts (arrest transport 73-85 tiles in a minute: a jump or a real
  teleport); the `pimp` pick empty at 100k.

## The close (2026-10-31)

Z1, Z2 and Z3 merged (`33b8a17`, change tier green over the set). Stories 1 and 4 read in the coders' own 100k runs in
V17's window (not a second critic pass; the exit rule closes the pass after the targeted phases):

- **Story 1, shown**: `target/z1/sh100k/bereaved_32861.md` (Cerys Tanner, her father Orrin Quarry killed by Mako
  Singh): `d21 20:00` mourning leave; `d23 00:02` "put word inside Precinct#4485: Mina Pembroke, held there with Mako
  Singh, will see to it"; `d25 18:17` "went looking for Mako Singh" (out of the cells), a stake-out at Hideout#4532, an
  Attack; `d25 19:15` "went for Mako Singh over a killing and lost; she nurses it, the grudge heavier". 20k: killing
  grudges with a hunt line 5 -> 63 %, Avenged 24 -> 38.
- **Story 4, shown**: `fixer_66801` (100k) `d23 01:19` a beat posted on Dante Draper for 349 (revenge for a
  betrayal); `11:45` "Aldo Penhallow came to Fixer#5202 in person and took the beat ... for The Red Dogs"; `d24
  22:22` done by The Cinder Teeth. 20k: takes in person 0 -> 250, take hours 2 -> 11.

**The pressure pass is closed.** What V18 and the Z phases left reading wrong is on `OPEN.md` (`OPEN-252` to
`OPEN-259`, `OPEN-217`) with no round of its own.
