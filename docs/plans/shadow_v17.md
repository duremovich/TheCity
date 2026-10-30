# Shadow V17: the critic brief (2026-10-30)

Pressure pass round 8's read, in V16's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype (V16's
list plus `bereaved` and `marked`, X2's new picks). Main at `target/shadow_v17/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v17/isle100k_s42_d21/<arch>_<index>.md`. V16's diaries are in
`D:/TheCity/target/shadow_v16/isle100k_s42_d21/`. The round's 30-day 100k counters: `target/r8_100k/analyze.txt`
(round 7's in `target/r7_100k/`). X6 re-ranked the `gun`, `runner`, `arms_dealer` and `jailed` pickers, so those
picks are different people from V16's by design.

**Context:** `docs/SHADOW_V16.md`, your group's V16 report `docs/shadow_v16/<group>.md`, `docs/TURF.md` (gangs and
crime groups), and `docs/PRESSURE_PASS.md` § "Round 8 from Shadow V16" plus the X1-X6 entries at the end of §
"Implemented: deviations" (each says what it built and what its coder already saw reading wrong at 20k; do not
re-report those as new, say whether they hold at 100k). The round: X1 the promise to tell the law (a robbed sleeper
who means to tell walks to a desk within a day or the diary says why not; the held do not witness the street or get
hit across the cell wall; nobody blackmails asleep; a crime in the cells is a case; a death resisting arrest names no
killer); X2 the target and the bereaved (a death in the household is mourned 2-5 days with leave, a funeral, the
children's line; a hit's target hears and takes a guard, a counter-grudge or lies low; a stalled hit is let go with a
line; the sacked in need search); X3 the wrong person in the role (the law's posts read a record; the employed steal a
vehicle only broke or miserable; no Fighter foreman); X4 gangs (the lie-low line names the right enemy; every leader
change a Succession line; the raid muster where members can reach; the war call wakes a sleeping member); X5 the pit,
the ages and marriages (a loser hurt only on a beating, the purse over the patch-up from the pit's owner; bout,
arrest, witness and cell lines print ages; no kin or minor courts, a marriage needs a courtship); X6 the cell's day,
the walk-in and the picks (the meal, the yard, kin visits, the release line; walk-ins to the least busy open clinic).
**The hard rule that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as
a victim, a combatant in a corner fight or a war, in vice, beaten or fined by a guard, in a cell, courted or married,
or dead is a top bug.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Turf, corner, truce,
war, lie low, woke, Succession, pushed, took over, means to tell, told the law, never told, mourning, funeral, leave,
lying low, guard on himself, let it go, searched, boards, record, yard, meal, came to the cells, came out to, Bout,
purse, points, courting, married, refused, Witness, `GoTo`), read the summaries, then read two or three full days
that matter. Cite as `file dDD HH:MM`. A missing line is never evidence.

**Write `D:/TheCity/docs/shadow_v17/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V16, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V16's word for it), one excerpt each.
3. **Round 8's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V16 bug in
   your group fixed / better / unchanged / worse.
5. **Dull spots**, ranked, each with the mechanism (not a knob) that would make it a story.
6. **Too much**: anything now so frequent or so harsh it reads as noise or a broken city (watch: funerals and
   mourning lines, yard and meal lines in every jailed diary, Succession lines, the war call waking members every
   night, the word-to-the-law walks). Dylan accepts harshness; flag only what reads as broken or monotonous.

Read-only: write only your report file. Your final reply is the report's verdict and its top three bugs.

## Groups

| group | archetypes |
| --- | --- |
| street | homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent, neglect_parent, camp_jobless |
| gangs | gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider |
| crime | forger, fixer, gun, buyer, pimp, arms_dealer, black_doc, jailed, dodger, marked |
| corp | ceo, exec, guard, reporter, office_worker |
| labour | worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke |
| care | ripperdoc, orderly, super, sick, teacher, school_parent, bereaved |
