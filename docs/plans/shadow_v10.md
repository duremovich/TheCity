# Shadow V10: the critic brief (2026-10-26)

The pressure pass's read. Same window as V9 so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype, plus the
pass's new picks (`miserable`, `owner_broke`, `jailed`, `raider`). Main `33f9c55` (`target/shadow_v10/sha.txt`).
Diaries: `D:/TheCity/target/shadow_v10/isle100k_s42_d21/<arch>_<index>.md`, roll-up `camera.tsv` there. V9's
diaries are in `D:/TheCity/target/shadow_v9/isle100k_s42_d21/`. The 30-day 100k counter run of the pass (one build
earlier, without phase 5b) is `target/pp100k/analyze.txt`; V9's is `target/shadow_v9/analyze100k.txt`.

**Context:** `docs/SHADOW_V9.md`, your group's V9 report `docs/shadow_v9/<group>.md`, and
`docs/PRESSURE_PASS.md` (every phase's "Implemented: deviations" entry says what it built and what its coder
already saw reading wrong; do not re-report those as new, say whether they hold at 100k).

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Skipped, Brawl,
Punished, Blackmail, Service, Cells, Crew, UnderWater, Loan, Confronted, Refused, `GoTo`), read the summaries, then
read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never evidence.

**Write `D:/TheCity/docs/shadow_v10/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V9, do these lives now read as people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V9's word for it), one excerpt each.
3. **The pass's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and
   never saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V9 bug
   in your group fixed / better / unchanged / worse.
5. **Dull spots**, ranked, each with the mechanism (not a knob) that would make it a story.
6. **Too much**: anything now so frequent or so harsh it reads as noise or as a broken city (a firing every day, a
   brawl every hour, everyone jailed). Dylan accepts harshness; flag only what reads as broken or monotonous.

Read-only: write only your report file. Your final reply is the report's verdict and its top three bugs.

## Groups

| group | archetypes |
| --- | --- |
| street | homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable |
| gangs | gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider |
| crime | forger, fixer, gun, buyer, pimp, arms_dealer, black_doc, jailed |
| corp | ceo, exec, guard, reporter, office_worker |
| labour | worker, worker_friday, night_shift, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke |
| care | ripperdoc, orderly, super, sick, teacher |
