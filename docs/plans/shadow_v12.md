# Shadow V12: the critic brief (2026-10-27)

Pressure pass round 3's read, in V11's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype, plus
round 3's new pick (`school_parent`). Main at `target/shadow_v12/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v12/isle100k_s42_d21/<arch>_<index>.md`. V11's diaries are in
`D:/TheCity/target/shadow_v11/isle100k_s42_d21/`.

**Context:** `docs/SHADOW_V11.md`, your group's V11 report `docs/shadow_v11/<group>.md`, and `docs/PRESSURE_PASS.md`
§ "Round 3 from Shadow V11" plus the S1-S7 entries in § "Implemented: deviations" (each says what it built and what
its coder already saw reading wrong at 20k; do not re-report those as new, say whether they hold at 100k). The round:
S1 distance in the choice (the Deal spot, muster, hire, site, Visit and Camp answer, JoinGang by the body's own trip;
"stays off the trade", "could not make the trip", "sat out the raid"); S2 plans that fail at the end (a walk for a
lost post turns back, the order left at the door, the back-off "N in a row", a muster sat is kept); S3 justice (the
charge names its victim, "resisted arrest", jail meals and sleep, the guard's collar, jail duty in blocks, the fare
beating's cool-down); S4 care (off-screen patients seen at the doctor on duty, the Friends' ladder, the super's legs);
S5 trades (the cabbie's cut and rank, the hired hand at the yard, the bouncer's rough client and bar, the beat story
at 100k, a hit heard of); S6 school-age children (seeded, School from 6, the `[family]` line) and **the hard rule that
no one under 18 is ever harmed** (flag any line where a minor is a victim, in vice, or dies: that is a top bug); S7
the diary's folds (the raid result line, crowd and hourly counts, the rest day in the hour lines).

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Fare, Brawl, Raid,
Crew, Cured, Patched, slate, pawned, begged, Notice, Story, Bout, booked, took .* as a fare, collected, `GoTo`), read
the summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never
evidence.

**Write `D:/TheCity/docs/shadow_v12/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V11, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V11's word for it), one excerpt each.
3. **Round 3's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V11 bug in
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
| labour | worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke |
| care | ripperdoc, orderly, super, sick, teacher, school_parent |
