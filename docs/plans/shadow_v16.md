# Shadow V16: the critic brief (2026-10-30)

Pressure pass round 7's read, in V15's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype (V15's
list). Main at `target/shadow_v16/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v16/isle100k_s42_d21/<arch>_<index>.md`. V15's diaries are in
`D:/TheCity/target/shadow_v15/isle100k_s42_d21/`. The round's 30-day 100k counters: `target/r7_100k/analyze.txt`
(round 6's in `target/r6_100k/`).

**Context:** `docs/SHADOW_V15.md`, your group's V15 report `docs/shadow_v15/<group>.md`, `docs/TURF.md` (gangs and
crime groups), and `docs/PRESSURE_PASS.md` § "Round 7 from Shadow V15" plus the W1-W6 entries in § "Implemented:
deviations" (each says what it built and what its coder already saw reading wrong at 20k; do not re-report those as
new, say whether they hold at 100k). The round: W1 the household buys for its children (while the household's own
food is short of the children's, any free adult of it shops, hungry or fed; an adult who can buy keeps the children's
share back; the `[family]` line says fed or unfed); W2 the war and the case in the diaries (the raid waits for its
marchers, members hear their gang's war, a called member holds his corner; a sleeper robbed wakes to the short purse
and tells the law or says why not; a finder who saw the murder names the killer or says why he kept quiet); W3 the
guard makes the arrest (the arrest planned straight to the suspect, the cuffs at the step's start, an unseen warrant
served at the door at night, a chase past the shift; the fare one dodger at a time, the broke old and frail booked not
beaten; a call for help reaches a guard, the doctor's staff or a passer-by); W4 the right person in the role (no hire
picked by a hand's trade; a chrome install cut and paid by a ripperdoc or black doc on duty); W5 contradictions (a
parent breaking his own child out is Camp Escape; a dancer's set counts only the set; sleepers do not witness; the
awake payer's line; a dealer's stand gathers its witnesses into one line; the School fee reads the midnight purse;
nobody held marries; a told job lead says what came of it); W6 the dull rooms (see its entry).
**The hard rule that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as
a victim, a combatant in a corner fight or a war, in vice, beaten or fined by a guard, or dead is a top bug.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Turf, corner, truce,
crack, war, lie low, woke, tells nobody, means to tell, kept quiet, Fare, fined, booked, arrested, at home, warrant,
called for help, heard, Rescued, Camp Escape, warning, fed last night, unfed, took, children's food, installed, hired,
picked by, Bout, purse, refused, turned away, Witness, `GoTo`), read the summaries, then read two or three full days
that matter. Cite as `file dDD HH:MM`. A missing line is never evidence.

**Write `D:/TheCity/docs/shadow_v16/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V15, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V15's word for it), one excerpt each.
3. **Round 7's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V15 bug in
   your group fixed / better / unchanged / worse.
5. **Dull spots**, ranked, each with the mechanism (not a knob) that would make it a story.
6. **Too much**: anything now so frequent or so harsh it reads as noise or a broken city (watch: war lines in every
   member's diary, door arrests every night, marriages (W5 saw 5,033 a month at 20k), stand Witness lines). Dylan
   accepts harshness; flag only what reads as broken or monotonous.

Read-only: write only your report file. Your final reply is the report's verdict and its top three bugs.

## Groups

| group | archetypes |
| --- | --- |
| street | homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent, neglect_parent, camp_jobless |
| gangs | gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider |
| crime | forger, fixer, gun, buyer, pimp, arms_dealer, black_doc, jailed, dodger |
| corp | ceo, exec, guard, reporter, office_worker |
| labour | worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke |
| care | ripperdoc, orderly, super, sick, teacher, school_parent |
