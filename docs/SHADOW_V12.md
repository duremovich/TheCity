# Shadow V12: pressure pass round 3 at 100k (2026-10-27)

Round 3's read, in V11's window so the two compare one for one: main `6e1c87f` (`target/shadow_v12/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype plus round 3's new pick (`school_parent`). Seven Sonnet
critics (`docs/plans/shadow_v12.md`) wrote `docs/shadow_v12/<group>.md`. Runs in `target/shadow_v12/`; the round's
30-day 100k counters in `target/r3_100k/analyze.txt` (round 2's in `target/r2_100k/`).

## Verdict

**Better than V11 in all seven groups; the biggest jumps are care and corp, the smallest labour and vice.** What
round 3 fixed shows at 100k:
- care is alive: supers serve notice on their round (`super_101989 d21 12:53`, 13 tenants), orderlies treat with the
  doctor's cut (`orderly_21701 d25 09:00`), teachers teach a counted class (`teacher_43475 d21 09:00`, 13 pupils);
  Treated 3.0k -> 12.3k in 30 days (care);
- a raid has a headline ("Raid on Zetatech 6's Garage#5483: The Black Dogs won (7 raiders, 0 defenders); the till
  smashed, 500c", `gang_leader_19331 d24 01:02`), and the 500-minute leader walks are gone (gangs);
- reporters print (6 and 8 Stories against 0), a hit on a CEO lands and is heard of (`ceo_88522 d22 06:08`), jail
  duty runs in 28-91 min legs (corp);
- jail feeds its inmates and the 1,171-edge flood is gone; a contract runs muster to arrest to sentence in a day
  (`gun_37892`); back-room docs have patients (crime);
- the cab pays by the fare, V11's coinless `Paid` is gone; the charge names its victim (labour, vice);
- lines now say why: "stays off the trade today: ... 5 h 17 from home", "could not make the trip" (street).

**The no-harm rule held in every diary**: no critic found a minor hurt, in vice, sick to death or a victim in 110
diaries; `harm_blocked` read 0 for the whole 30-day run (no kill ever reached a minor). Two code paths could still
land *non-lethal* harm on a minor (below, item 1); fixed before the close.

What still reads wrong: the body still spends the day walking (4-10 h of travel in every group), shifts are a
metronome with no trade inside them (bouncers, dancers, club staff, foreman), and the same standing-fact line prints
every morning.

## Next pass, ranked by what the player sees

1. **The no-harm rule had two non-lethal holes** (fixed at the close, `OPEN-230`). `law::resolve_fight_mods` had no
   age check, and `missions::defenders` / the raid's defenders took a target's Friends at the door unfiltered; the
   fare ladder's `caught()` / `beat()` had none either (a 16-17-year-old by age is not a `Child`). `kill_by` only
   guards deaths. Unseen in the diaries (`gun_37892 d26 00:09` "attacked 3 at Block#3343" and `guard_8440 d24 23:15`
   fare beatings print no ages; the critics asked for the age on victim lines).
2. **The camp is a number, not a place** (`OPEN-231`). ChildTaken 32 -> 3,726 in 30 days (~124 a day; a daily mean
   of 1,782 children inside, of ~11k): the service working as Dylan asked, at a size worth his look. Neither camp
   parent can reach it ("could not make the trip ... 4 h 07 / 4 h 55 from home", `camp_parent_* d21 00:07`), the
   wardens are clerks with one headcount line a week. Visits, a warden's day among the children, and the parent's
   buy-out or break-out read from the parent's side.
3. **Walks still eat the day** (`OPEN-221`, kept open). S1 cut the goal-side legs; what's left is home and post: a
   broke owner commuting ~10 h to his own bar (`owner_broke_10159 d23`), sweepers 4-8 h a day, a loader's 11.9 h of
   travel for a 6.5 h shift (`loader_33807 d26`), leisure and errands across the island (`madam_104391 d24`). The map
   re-import is half; "move nearer work" fired for the fighters only, and leisure picks ignore distance.
4. **Children not at School** (`OPEN-232`). Most school-age kids are "at home (play)" on school mornings
   (`gang_member_6764`, 6 y, d21-d27; `forger_90429`, `buyer_56165`): school days / child-days 4.0 %, 18 Schools for
   ~11k children, only a few hundred in reach. A teacher works d26 with no class (`teacher_43475`). A School per
   district or reach by travel time: Dylan's call.
5. **Plans still die silently** (`OPEN-222`, kept open). A held contract is never planned and expires
   (`gun_36512`, "backed off, 7 in a row"); arrest chases end in `Unwind` with no event, 1 of 4 completed
   (`guard_8440 d23 11:10`); a Guard's Work plan is a 0-tile GoTo timeout 25 times in 6 days (`runner_90375`); a
   mustered raider still marked "missed" (`raider_70326 d22 00:07`, V11's, unchanged); a contract brawl hits three
   people who are not the target.
6. **Trades without the trade** (`OPEN-217`, kept open). Bouncers, dancers and club staff are Wage + ClerkWork for 28
   job-nights (S5's door fired at 20k, not in these four at 100k); the foreman is 174 Hire lines; a laid-off night
   worker sits idle four days with no job search (`night_shift_23119 d21`); a hired day labourer rests and walks
   (`day_labourer_100168 d23`); the pimp and buyer picks never do their trade (V11's, unchanged).
7. **Money and labels that disagree**. A treatment billed 80c moves 1c to a different payee
   (`miserable_64146 d22 05:10`); a con names two sellers (`vendor_11367 d24 00:09`); the foreman named is the man who
   just lost the post (`ripperdoc_61226 d27 00:11`, V11's); a CEO's decision `Served` five times in 70 min at home
   (`ceo_88522 d24 00:07`); an exec hired as an Engineer while an exec (`exec_78363 d26`).
8. **Noise** (`OPEN-225` closed; new `OPEN-233`). The 00:07 standing-fact line ("stays off the trade", "sat out the
   raid", "chose the shift") daily, 4-6 times a life; "Bury cooled ... 33 in a row"; gossip runs of 20-40 lines; a
   guard's 52 Fare Evasion witness lines from inside the Precinct; the hourly "met at the table" memories back.
   Log on the first day and on the change.

The sick picks carry no illness (`no cancer | sick days 0/14`), so S4's Friends' ladder went unwatched here: a pick
filter for V13. The rest day still reads blank (19-21 h idle) though S7 marks it.

## What round 3 did to the city (100k, seed 42 x 30, `r3_100k` against `r2_100k`)

Printed, not gated. The city starts at 111.2k (S6 seeds ~11k school-age children on the 100k adults). Employed mean
39.8k -> 39.9k, thefts 17.0k -> 16.7k, arrests 9.3k -> 9.4k, jailed mean 1,028 -> 1,065, deaths by violence 393 ->
441, starvation deaths 10 -> 11 (no minor; `harm_blocked` 0), homeless max 11.3k -> 11.9k (S1's stranded natives,
`OPEN-228`'s follow-up). Witness 377k -> 190k (S7), Treated 3.0k -> 12.3k and Cured 38 -> 129 (S4), Served 10.8k ->
19.6k, Story 3.4k -> 7.1k (S5), Crew 12.7k -> 21.0k, GrudgeFormed 4.2k -> 11.4k, TerritoryFlipped 80 -> 220,
ChildTaken 32 -> 3,726, CampEscape 3 -> 211. Ticks/s days 25-29 24.6 -> 22.4 (30 days in 1,748 s wall); at 20k on a
quiet box 277 (round 2) -> 237 (wave 1) -> 218 (all of round 3), `OPEN-229`.
