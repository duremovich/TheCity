# Shadow V11: the critic brief (2026-10-27)

Pressure pass round 2's read, in V10's window so the two compare one for one: 100k island
(`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype, plus
round 2's new picks (`dodger`, `camp_parent`). Main at `target/shadow_v11/sha.txt`.
Diaries: `D:/TheCity/target/shadow_v11/isle100k_s42_d21/<arch>_<index>.md`. V10's diaries are in
`D:/TheCity/target/shadow_v10/isle100k_s42_d21/`.

**Context:** `docs/SHADOW_V10.md`, your group's V10 report `docs/shadow_v10/<group>.md`, and `docs/PRESSURE_PASS.md`
§ "Round 2 from Shadow V10" plus the R1-R6b entries in § "Implemented: deviations" (each says what it built and what
its coder already saw reading wrong at 20k; do not re-report those as new, say whether they hold at 100k). The round:
R1 fares are a ladder (fine, beating, El ban), never a cell; R2 walking twice as fast; R3 the wounded and the sick
(rescue, the slate, the pawn, the Friend's loan, back rooms, the sick post covered); R4 the friendship flip (only
witnesses turn on a hold blow) and the diary's folds (the super's one notice line, the rest day marked); R5 the gang
verbs (the muster holds, the try-out, Scavenge, the broke gang's stand-down, court missed at work excused); R6a the
cab, the pimp's round, bets at the pit, the madam's bookings, the Dollhouse; R6b the gun hunts, the Hq as the office,
the landlord's desk, the reporter's beat story, the gangs' arms trade.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, event words such as Fare, Brawl, Raid,
Crew, Cured, Patched, slate, pawned, begged, Notice, Story, Bout, booked, took .* as a fare, collected, `GoTo`), read
the summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never
evidence.

**Write `D:/TheCity/docs/shadow_v11/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V10, do these lives read more like people in a harsh city, with stories that
   start and get picked up?
2. Per archetype: alive / thin / dead (and V10's word for it), one excerpt each.
3. **Round 2's mechanisms seen at 100k**: which fired in your group's diaries, quoted; which you expected and never
   saw.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V10 bug in
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
| care | ripperdoc, orderly, super, sick, teacher, child |
