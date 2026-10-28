# Shadow V14: the critic brief (2026-10-28)

Pressure pass round 5's read, in V13's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype, plus
round 5's new picks (`neglect_parent`, `camp_jobless`). Main at `target/shadow_v14/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v14/isle100k_s42_d21/<arch>_<index>.md`. V13's diaries are in
`D:/TheCity/target/shadow_v13/isle100k_s42_d21/`. The round's 30-day 100k counters: `target/r5_100k/analyze.txt`
(round 4's in `target/r4_100k/`).

**Context:** `docs/SHADOW_V13.md`, your group's V13 report `docs/shadow_v13/<group>.md`, and `docs/PRESSURE_PASS.md`
§ "Round 5 from Shadow V13" plus the U1-U8 entries in § "Implemented: deviations" (each says what it built and what
its coder already saw reading wrong at 20k; do not re-report those as new, say whether they hold at 100k). The round:
U1 Schools for the children there are (seeded one per 60 school-age children where the most lack a seat, founded on
demand when a district turns children away full; the 2c fee stays); U2 the neglect ladder tightens (a second warning
inside 30 days takes, a warning with no free parent at home takes at once, an elder's move brings his young children,
the `[family]` line reads the Camp after a take, a Camp visit due outranks a search day); U3 the starving broke reach
a meal (a starving adult walks to a kitchen with a bowl left by trip, a pot that runs dry, the line and the turned
away, kitchens share a purse, a soup line opens where the starving are); U4 who is indoors sees indoors (a witness
sees his own building, or the street within sight when outside); U5 the forger and the fixer (the forger's walk-in
at his own desk, the forged weapons licence, the fixer's book names him: posts, takes with the crew, the close's
cut); U6 the pimp and the ripperdoc (the pimp's round told, a client sent to a dry pitch, the named pimp off the
hire; the clinic's day: patients to the doctor's body, chrome booked by the hour, the blood wash, no ten-hour wait
at the post); U7 contradictions (the thief not "robbed", victims and accomplices no witnesses, the raider on his
raid, no guard on oneself, a spent Locate settles, the cab owner's cut, the foreman on hand, an unseen heir learns
of a strip, a jailed exec unpaid); U8 the GangWork perf phase (the choices should read as V13's). **The hard rule
that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as a victim, in
vice, or dead is a top bug.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Fare, Brawl, Raid,
Crew, Cured, Patched, SoupLine, MissionServed, warning, took, School, licence, forged, posted, collected, booked,
wash, Witness, `GoTo`), read the summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`.
A missing line is never evidence.

**Write `D:/TheCity/docs/shadow_v14/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V13, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V13's word for it), one excerpt each.
3. **Round 5's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V13 bug in
   your group fixed / better / unchanged / worse.
5. **Dull spots**, ranked, each with the mechanism (not a knob) that would make it a story.
6. **Too much**: anything now so frequent or so harsh it reads as noise or a broken city. Dylan accepts harshness;
   flag only what reads as broken or monotonous.

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
