# Shadow V15: the critic brief (2026-10-29)

Pressure pass round 6's read, in V14's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype (the same
pick list as V14), plus a pimp run from day 22. Main at `target/shadow_v15/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v15/isle100k_s42_d21/<arch>_<index>.md` (the pimp run:
`target/shadow_v15/isle100k_s42_d22_pimp/`). V14's diaries are in `D:/TheCity/target/shadow_v14/isle100k_s42_d21/`.
The round's 30-day 100k counters: `target/r6_100k/analyze.txt` (round 5's in `target/r5_100k/`).

**Context:** `docs/SHADOW_V14.md`, your group's V14 report `docs/shadow_v14/<group>.md`, `docs/TURF.md` (gangs and
crime groups), and `docs/PRESSURE_PASS.md` § "Round 6 from Shadow V14" plus the V1-V6 entries in § "Implemented:
deviations" (each says what it built and what its coder already saw reading wrong at 20k; do not re-report those as
new, say whether they hold at 100k). The round: V1 the ladder at 100k and children out of care (a Block's shelf is
split into the households' stakes, so a broke family goes unfed and is warned on its own; one warning per household;
a child who slips out of the Camp is fetched back; a street child goes to a free parent with a Home or is taken); V2
turf you hold (turf cells held by presence, the gang's corner and members holding it, holder changes told, the rival
is the neighbour, tribute and protection by cell, dealing on the corner, the pimp named where the members live,
recruits from the turf, residents avoid contested cells after dark, families move their children out); V3 violence
with consequence (a body is found and the law opens a case, the killer named on a lead; a robbed victim cries out to
a guard or tells one on the beat; the downed lie still and sleep, no wage while down; a busted Fixer office closes);
V4 the night out and the trip (gamble before drink at a kept door, the bar honoured; a retiree's move to kin skips a
spouse's long commute; the rider pays the cab driver, a cab under way holds and lands where paid); V5 the turf war
(provocation on a held cell, the leader's answer musters to the corner, the cell held or flipped and taken back, a
Hideout raid moves cells, peace by exhaustion, truce or the law's crackdown); V6 empty rooms beside full ones (a
Clinic opens at night for a patch-up when its orderly is on, patients spill to a free doctor, the back room's doc
spread, the door turns away the broke and the drunk off screen, the pit keeps its bout book and an undercard).
**The hard rule that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as
a victim, a combatant in a corner fight or a war, in vice, or dead is a top bug.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Turf, corner, truce,
crack, Fare, CabFare, Brawl, Raid, Crew, CorpseFound, found the body, cried out, told, Report, arrested, Patched,
SoupLine, warning, took, fetched, off the street, pantry, refused, turned away, Bout, undercard, `GoTo`), read the
summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never evidence.

**Write `D:/TheCity/docs/shadow_v15/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V14, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V14's word for it), one excerpt each.
3. **Round 6's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V14 bug in
   your group fixed / better / unchanged / worse.
5. **Dull spots**, ranked, each with the mechanism (not a knob) that would make it a story.
6. **Too much**: anything now so frequent or so harsh it reads as noise or a broken city (watch the turf war and the
   corner: wars every night, 60-man gangs on three fronts, corner deals everywhere). Dylan accepts harshness; flag only
   what reads as broken or monotonous.

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
