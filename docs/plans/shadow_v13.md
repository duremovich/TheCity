# Shadow V13: the critic brief (2026-10-28)

Pressure pass round 4's read, in V12's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype, plus
round 4's new pick (`jobless`). Main at `target/shadow_v13/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v13/isle100k_s42_d21/<arch>_<index>.md`. V12's diaries are in
`D:/TheCity/target/shadow_v12/isle100k_s42_d21/`. The round's 30-day 100k counters: `target/r4_100k/analyze.txt`
(round 3's in `target/r3_100k/`).

**Context:** `docs/SHADOW_V12.md`, your group's V12 report `docs/shadow_v12/<group>.md`, and `docs/PRESSURE_PASS.md`
§ "Round 4 from Shadow V12" plus the T1-T7 entries in § "Implemented: deviations" (each says what it built and what
its coder already saw reading wrong at 20k; do not re-report those as new, say whether they hold at 100k). The round:
T1 neglect warns first (the service at the door, the grace, a repeat takes; the children's share of the pantry; let
home; the warden's headcount, the fence, visits, break-outs); T2 School by travel time (the nearest School with room
within 1 h, the El fares, a roll of 30 a teacher, the teacher's day follows the roll, the `[family]` line says why);
T3 plans that die silently say why (ledger-kind contracts, a jailed mark waited out or let go, the dropped arrest
chase as a `Pursuit` line handed on, staff skip the door queue, a sat muster keeps the day's duty, the brawl takes
the mark first); T4 the trades' inside (the door's let-in and refusals to a body, the dancer's sets and tips, the
club hand's night, the foreman at the gate and the `DawnPick` line); T5 the jobless search (Friends asked, boards and
doors walked, taken on at the door, worse work, far posts before the Ambient pool, the day labourer's 06:30 shift, an
exec gives up a post); T6 money and labels (who paid a bill and whose till took it, the burned mark wary of a fake
job, the gang's man picks at a seized shop, a CEO's call told once and where, the ledger's books audited); T7 folds
(standing facts on the first day and the change, day folds for chases, refusals, cools and gossip, served
encounters once a day, a refused face that returns is barred) and **the victim's age on every harm line**. **The
hard rule that no one under 18 is ever harmed** holds: every harm line now prints ages, so check them; any minor as
a victim, in vice, or dead is a top bug.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Fare, Brawl, Raid,
Crew, Cured, Patched, slate, pawned, begged, Notice, Story, Bout, booked, took .* as a fare, collected, `GoTo`), read
the summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never
evidence.

**Write `D:/TheCity/docs/shadow_v13/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V12, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V12's word for it), one excerpt each.
3. **Round 4's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V12 bug in
   your group fixed / better / unchanged / worse.
5. **Dull spots**, ranked, each with the mechanism (not a knob) that would make it a story.
6. **Too much**: anything now so frequent or so harsh it reads as noise or a broken city. Dylan accepts harshness;
   flag only what reads as broken or monotonous.

Read-only: write only your report file. Your final reply is the report's verdict and its top three bugs.

## Groups

| group | archetypes |
| --- | --- |
| street | homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent |
| gangs | gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider |
| crime | forger, fixer, gun, buyer, pimp, arms_dealer, black_doc, jailed, dodger |
| corp | ceo, exec, guard, reporter, office_worker |
| labour | worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke |
| care | ripperdoc, orderly, super, sick, teacher, school_parent |
