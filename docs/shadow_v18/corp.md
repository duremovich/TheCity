# Shadow V18: corp (ceo, exec, guard, reporter, office_worker)

Critic: corp. 100k island, seed 42, d21-27, `D:/TheCity/target/shadow_v18/isle100k_s42_d21/`, main 621882a. Picks: `ceo_67712`, `ceo_88522`, `exec_74262`, `exec_81192`, `guard_29797`, `guard_9368`,
`reporter_54070`, `reporter_97964`, `office_worker_13438`, `office_worker_94167`. Read by event-word counts (python), DAY SUMMARYs, a min-hunger scan of every `[needs]` line, and full stretches:
guard_9368 d23 00:00-01:20 and d26 21:07-23:01, guard_29797 d26 01:41-05:58 and 12:17-18:00, exec_74262 d25 22:42-d26 01:16, ceo_67712 d21 00:49-07:17, office_worker_13438 d21 06:54-08:58,
reporter_97964 d22. Cited `file dDD HH:MM`; a missing line is never evidence. The picks differ from V17's, so this is a re-roll, not a one-for-one.

## 1. Verdict

Better than V17 where the pace matters: guards patrol instead of walking all shift, commutes are 4-14 min, a robbed CEO gets to the desk in 30 minutes, and the law is the one living part of the group again.
The corp class itself is still a desk, a Lab and a Feed; its only stories come from the street coming to it (a shakedown, a corpse to loot, a murder witnessed). The exit's five stories: one partial, four not seen here.
Nobody in my ten starved, nobody was harmed who is under 18, and no day repeats exactly.

## 2. The exit's five stories (corp group)

1. **Friend killed, friend goes after the killer: not seen.** No picked life lost a friend; the only Murder lines are witnessed (`guard_9368 d23 01:18`, `d23 17:56`; `exec_74262 d26 00:38`).
2. **Word of a killing getting around, someone acting on it: partial.** `exec_74262 d26 00:38 Witness: Emeric Nightingale (35 y) saw Pixel Draper (58 y) commit Murder on Dante Volkov` ->
   `WitnessReacted: ... told 3 of his people what Pixel Draper did (the Murder)` (word does travel) -> `00:42 GOAL Loot ... body within reach` -> `00:52 Stripped: ... the body of Dante Volkov (4 coins)` ->
   `Witness: 2 saw Emeric Nightingale commit Theft (Sprocket Bellamy, Pixel Draper)` -> `Report` -> `00:54 Arrest` -> `01:16 Sentence: 3 days`, a jail week (d26-27 `jail 16.0h`). The one who acts is on the corpse,
   not the killer: the killer Pixel Draper reports the looter. Dark and funny, but the telling goes nowhere (no one goes to the law about the Murder, and he never tells it again). Gossip of killings is also
   everywhere in the diaries (`exec_74262 d21 07:12`, `ceo_88522 d25 07:49`), always "(already known / dropped)" or told on, never acted on.
3. **A hit as ordinary business: not seen.** `ContractPosted` is corp-side only: `exec_81192 d21 00:06 Greenline 30 posted a locate on Elspeth Draper for 37 at Fixer#5410 ... posted a tag on Elspeth Draper for 277`,
   `ceo_67712` x5, `ceo_88522` x5. No buyer-meets-hitter scene and no outcome reaches a corp diary.
4. **An open contract an NPC takes: not seen** in my group (same lines; no `took the job`).
5. **A life that makes sense on inspection: seen, twice.** `ceo_67712`: asleep at Block#648 while Lux Tanner and Yuki Duarte shake the block down (`d21 00:49 Coerced`, 12 Coerced lines) -> `06:00 woke ... 10c gone ...
   he means to tell the law` -> `06:02 hailed a cab (flush)` -> `06:34 Precinct` -> `06:45 Report: ... Lux Tanner, for Shakedown` -> `06:55 ReportCrime @ Civic Hall`. Cause, a rich man who can afford the cab, the walk, the desk: it follows.
   `guard_9368` Taz Reyes (59 y, wallet 2c, SYNTH meals): `d23 00:08 GOAL Eat ... [GoTo(Market) > StealFood(Market) > EatFromInventory]`, `00:34 Theft: stole 2 food`, `00:39 Arrest` by a witness,
   `00:58 JAILED until d26`, `d24 00:07 Cells: missed the shift as Guard in the cells (absence 1 in 14 days)`, `d26 21:40 Pursuit: ... called for a second`. A poor guard whose pay does not cover the pantry, jailed in his own precinct.

## 3. The exit's three checks

- **Starving while Eat loses to a non-survival goal: none.** Lowest hunger across every `[needs]` line of my ten is 0.47 (`guard_9368 d23 06:00`), 0.49-0.58 for the rest (hunger here is satisfaction, 1.0 full); no pick under 0.25;
  no `Starving` line. Eat wins at U(hunger) ~0.2-0.3 each time (`guard_9368 d23 00:08`, `reporter_97964 d22 06:44`). The stolen-food fallback fired for a guard (`guard_9368 d23 00:13 StealFood`): Y1's chain reads right.
- **Frozen identical-day loop: none identical.** Near-loop: `reporter_97964` d21-d27 summaries run the same shape (other 7.8-10.8h, leisure 3-4.7h, net 0c) (see bug 3). `reporter_54070` d22-d26 are the same shape too (work 8.0h,
  one Story at 00:01 each night), but the nightly Story names a different victim, so not a loop.
- **Minor harmed: none.** No (N<18 y) in any Arrest, Assault, Fare, Witness-victim, Hit or death line of my ten. Edge: `Camp Escape` sentences name children as the "on" target
  (`guard_29797 d23 12:00 Tristan Greaves (40 y) ... Camp Escape on Sabine Reyes (6 y)`, `d27 15:00 ... on Tobin Penhallow (0 y)`; 11 in guard_29797, 9 in guard_9368): the adult gets +1 day, the child is only the one taken out, nothing in the line
  says the child is hurt or held. Read as "taken from the Camp", but a child as the object of a crime line is worth one look.

## 4. Per archetype (V17 in brackets)

- **CEO: thin (thin).** Two quiet lives with a coherent night: `ceo_67712` above (sleeps in the block that is shaken down, reports, goes back to `meeting 8.7h`); `ceo_88522` Bertram Carrow (40 y): `meeting 8.5h` at `Lab#5288`
  daily, `social 1.0h`, `d25 07:49` gossip about Zed Haas's killing. Contract postings still go nowhere.
- **Exec: thin (thin), one good week.** `exec_74262` is the group's best week (Murder witnessed, corpse looted, jailed). `exec_81192` Sprocket Kowalczyk (72 y): `meeting 8.2h`, Fight Pit and Gambling Den on the rest day;
  nine `ContractPosted`, none heard back.
- **Guard: alive (alive).** `guard_9368` d26 21:38 `Arrest on Leopold Fletcher ... Assault: Leopold Fletcher (35 y) resisted arrest and fought off the guard Taz Reyes (59 y) (wanted for Camp Escape)` -> `21:40 Pursuit: Taz Reyes called for a second ...
  Percival Kimura on shift, 1 tiles off, comes` -> `21:50 gave up the chase (already in custody)`. `guard_29797` Philippa Santoro (60 y) has a plain arrest week (`d26 01:54` Dealing, `d26 14:13` Theft) and one odd night (bug 2).
- **Reporter: thin (thin).** `reporter_54070`: `d22 00:01 Story: Reyes Holdings Feed: Jinx Ambrose robbed, story by Wendell Moreau (from the beat)` x6, work 7.9-8.2h daily. `reporter_97964` is dead (bug 3).
- **Office worker: alive (alive/thin).** `office_worker_13438` Dmitri Inchbald (54 y): `d21 06:54 GOAL JoinGang` -> 59 min, 1044 tiles to Hideout#4561 -> `07:53 JoinGang` -> back to the Spire for `08:58` work; `d21 23:52 GangJoin`; summons to court
  (`d23 20:34`, `d25 20:34 Crew: answered The Pale Crows's summons`), and the Succession lines (`d24 13:26`, `d26 10:24`, `d27 00:05`, `09:24`) reach a desk clerk. A 54-year-old clerk with a gang on the side is a life. `office_worker_94167` Tamsin Northcott (26 y): 8.7-8.9h at HQ, gossips with the exec (`exec_74262 d21 07:12`), `d27 leisure 15.2h` at home.

## 5. The pace

Reads like people. Guard patrol legs `6 min 99 tiles`, `9 min 169 tiles`, `12 min 214 tiles`, `17 min 298 tiles` (`guard_9368 d22 22:32-23:34`, `d26 21:07`) with 21-minute PatrolLegs between; V17's `207 min, 101 tiles` is gone.
Commutes 4-14 min (headers), CEO to the Precinct `32 min, 433 tiles` by cab. A gang trip across the island is an hour (`office_worker_13438 d21 06:54`, `59 min, 1044 tiles`; back `60 min, 1077 tiles`): human, if a lot of day.
**Not absurd, but wrong:** the cab at `433 tiles / 32 min` is 13.5 tiles/min, slower than walking (21); a cab ride is the same as OPEN-254 and unchanged.
Nothing teleports; the `[camera] JUMP` lines (see bug 4) are escort steps of 17-21 tiles a minute, i.e. the walking pace itself.

## 6. Bugs (ranked by what the player sees first)

1. **A guard on shift wanders for hours with Patrol not even a runner-up.** `guard_29797 d26 12:17-18:00` (shift 09:00-18:00): `GOAL Idle (was Eat) ... runners-up: Earn 0.001` then ten `PLAN Idle [Wander]` and a `16:07 goal Arrest cooled 2 times today`;
   `d26 work 2.1h idle 12.0h`. Earlier that day `09:07 GOAL Patrol score 0.988`. Also `d26 01:54 GOAL Arrest ... in shift 1.00` at 01:54 on a 09-18 shift, so "in shift" is true for her at any hour. Likely: the Patrol goal is dropped
   after an Eat interruption until the next shift flag, and the "in shift" input is not the shift window. (V17 guard patrol-travel bug: **fixed**.)
2. **A guard buys stims on the corner, then arrests the dealer.** `guard_29797 d26 01:54 Witness: 36 saw Hana Quarry (49 y) deal stims on the corner by Block#2785 (01:37-01:41, 6 sales): Philippa Santoro (60 y), ...` ->
   same minute `Arrest: Philippa Santoro (60 y) arrested Hana Quarry (49 y) for Dealing`, after `01:07 GOAL GetHigh` and `01:44 stims 2->1`. Then `02:25 GOAL Flee ... hostile near` at the Precinct door and `FleeToHome`. A real, strange scene; the guard's buyer status
   is not visible to the law.
3. **A "Reporter" who is a jobless street vendor.** `reporter_97964` header `Job: Reporter at Feed#5133 (wage 5/day)`, but all week `work` is absent, `net 0c`, `GOAL Pitch ... days without a post 0.93`, `JobSearch ... nothing going` (`d21 07:00`, `d22 07:21`, ...).
   Header and behaviour disagree: either the post is empty on paper or the job-seeking goal is scoring a different (stale) employer. Eat/Unwind/Pitch repeat every hour (`d22 04:14-09:37`).
4. **`[camera] JUMP` false alarms at walking pace.** `exec_74262 d26 01:06-01:15` nine lines `JUMP 21 tiles in 1 min (allowance 2) ... exec Idle, no step` while escorted to jail; 18 in exec_74262, 13 in ceo_67712 (`allowance 23-27` at the commute: `JUMP 29 tiles in 1 min`).
   The camera allowance is still sized to the old crawl for escorted/inert bodies and at the shifted pace; noise in the diary, and a real jump would hide in it. New, from Y4.
5. **A second report for the same shakedown at a second desk.** `ceo_67712 d21 06:45 Report ... told the law` at the Precinct, then `06:47 GoTo(Hall) 8 min` and `06:55 ReportCrime @ Civic Hall`. Likely one report (to the law) and one to the Hall; reads as double work, and no outcome follows (Lux Tanner is not heard of again).
6. **"Killed by ... on #11587".** `exec_74262 d21 07:12 told Tamsin Northcott: Killed by Mateo Fairweather#54810 on #11587` and `exec_81192 d22 18:01 ... on #52536`: a victim with no name; the same rumour alternates with "Killed by gang The Ash Teeth on Nadia Dunmore"
   for the same death (`d21 07:44`). Two versions of one fact in one conversation.
7. **A guard gave up chasing a man already in custody.** `guard_9368 d26 21:50 gave up the chase of Leopold Fletcher (already in custody)` after the second guard made the arrest (V17 bug 5 **unchanged**; but the second-guard line itself
   works: `d26 21:40 called for a second ... Percival Kimura on shift, 1 tiles off, comes`; no "holding him" line seen, so the second guard's arrival and the take-down are inferred, not shown).

V17 status in my group: guard patrol travel **fixed**; Fare beat gossiped as "Assaulted by" the guard **not seen** (no `Fare:` lines in the guards at all, either fixed or no beats this week); byline grammar **fixed**
(`Story: ... Jinx Ambrose robbed, story by Wendell Moreau`); reporter hires as "the foreman" **not seen** (no Hire lines); chasing a man in custody **unchanged**; raid lines without ages **not seen** (no Raid in my guards'
pick window); SETTLED APART spam **better** (1 in exec_81192, 6 in ceo_67712); loot walk ending nowhere **better** (`exec_74262` strip completes; `guard_29797 d26 05:21 PLAN FAILED ... StockGone` ends in a line).

## 7. Dull spots and too-much

- Dull: corp contracts that nobody answers (`exec_81192` x9, `ceo_*` x5); an exec's week is `meeting 8.2-8.7h` and a Capsule Hotel; reporters write one Story a night about people they never meet.
- Dull: two of my four desk jobs (CEO, exec) never leave one district all week; the street comes to them only by shakedown.
- Too much: guards' `Witness` lines are 88-93 per diary (cell fights while jailed, `guard_9368 d23-d25`); the 11 and 9 `Camp Escape on <child>` sentence lines are the same line.
- Too much: 12 `Coerced` lines against one sleeping CEO, 4c and 1c, one per payer.
