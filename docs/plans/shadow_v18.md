# Shadow V18: the critic brief (2026-10-31): the pressure pass's exit read

Pressure pass round 9's read and **the pass's exit** (`docs/PRESSURE_PASS.md` § "Round 9 from Shadow V17", "The
exit"), in V17's window so the two compare one for one: 100k island (`--population 100000 --map
assets/maps/isle_m_k2_100k.txt`), seed 42, days 21-27, two picks per archetype (V17's list). Main at
`target/shadow_v18/sha.txt`. Diaries: `D:/TheCity/target/shadow_v18/isle100k_s42_d21/<arch>_<index>.md`; V17's in
`D:/TheCity/target/shadow_v17/isle100k_s42_d21/`. The round's 30-day 100k counters: `target/r9_100k/analyze.txt`
(round 8's in `target/r8_100k/`).

**Context:** `docs/SHADOW_V17.md`, your group's V17 report `docs/shadow_v17/<group>.md`, and the Y1-Y4 entries at the
end of `docs/PRESSURE_PASS.md` § "Implemented: deviations" (each says what it built and what its coder already saw
reading wrong at 20k; do not re-report those as new, say whether they hold at 100k). The round: Y1 hunger in the
brain (a started Eat holds, survival goes first when starving, a failed source tries the next: bin, kitchen, Market,
Home, then a theft for the less lawful; a laid-up patient is fed, kin bring a meal); Y2 contradictions that are state
(no arrests on mourning leave, the boss inside named, a holed-up leader leads nothing, a word with no desk waits on
the beat, a cell case's witnesses, no hit across the cell wall, the Guard post reads the body and an eviction, the
funeral asks the mourners' friends, **a second guard** comes to a resisting suspect, a fare beat is word of the law's
force); Y3 the narration (a byline, the Camp's custody count, cover shifts); **Y4 the walking pace: people now walk
5 km/h (21 tiles a minute; before, 0.24 km/h)**, so commutes are minutes, patrols and musters short, raids land, and
the El and cabs lost riders. Expect a different-feeling city; judge it on whether it reads like people.

**The hard rule that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as
a victim, a combatant, in vice, beaten or fined by a guard, in a cell, courted or married, or dead is a top bug.

**Names collide at 100k** (five Rosalind Duartes, three Cutter Ambroses in V17): before calling a contradiction,
check the line's entity index or age; a different person with the same name is not a bug.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, `GoTo`, event words: Eat, Starving, ran
dry, emptied, brought a meal, stole food, Turf, corner, raid, departs, won, Succession, called for a second, holding,
means to tell, told, mourning, funeral, Hunt, grudge, revenge, Hit, contract, Fixer, heard, word, Witness, Murder),
read the summaries, then read two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is never
evidence.

**Write `D:/TheCity/docs/shadow_v18/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V17, do these lives read more like people in a harsh city?
2. **The exit's five stories in your group**, each quoted if you find it (cite), else "not seen": (1) a friend killed
   and a friend who goes after the killer; (2) word of a killing getting around and someone acting on it; (3) a hit as
   ordinary business (buyer, price, hitter, outcome); (4) an open contract an NPC takes that a player could have
   taken; (5) a life that makes sense on inspection (a week that follows from its past).
3. **The exit's three checks**: anyone starving while Eat loses to a goal that is not survival or danger (quote it);
   any frozen identical-day loop (two or more identical days); any minor harmed.
4. Per archetype: alive / thin / dead (and V17's word), one excerpt each.
5. **The pace**: walks, commutes, patrols, musters at human scale? Anything now absurdly fast (a body across the
   island in minutes on foot, a jump)?
6. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V17 bug in
   your group fixed / better / unchanged / worse.
7. Dull spots and too-much, briefly.

Read-only: write only your report file. Your final reply is the report's verdict, the stories found (with cites) and
the top three bugs.

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
| **stories** | **every diary**: hunt only the five stories across the whole directory (grep Hunt, grudge, revenge, avenge, Hit, contract, Fixer, bounty, heard, word, killed, Murder, took the job), quote the best instance of each with cites, and say for each whether it reads as a story a player would follow; skip sections 4-7 |
