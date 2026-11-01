# Shadow V19: the critic brief (2026-11-01): the city milestone's exit read

The city milestone's read (`docs/CITY_MILESTONE.md` § "The exit", and each phase's "as built" note), in V18's window
so the two compare one for one: 100k island (`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42,
days 21-27, two picks per archetype (V18's list). Main at `target/v19/sha.txt`. Diaries:
`D:/TheCity/target/v19/isle100k_s42_d21/<arch>_<index>.md`; V18's in `D:/TheCity/target/shadow_v18/isle100k_s42_d21/`.
The 30-day 100k counters: `target/v19/analyze.txt` (V18's in `target/r9_100k/`).

**What changed since V18** (read the "as built" notes for detail; what a coder already saw reading wrong at 20k is
listed there: say whether it holds at 100k, do not re-report it as new):
- **Floors as tenancies (C6a, C6b):** a tower's shops, Bars, Markets, Hideouts and Lots are tenancies on named floors
  inside a host; a household has a flat on a floor. The host's owner is a **landlord** who takes floor rent; a
  person's floor is taken back after 7 days short; a founder takes a vacant unit near home.
- **Levels (C7):** a body climbs to its floor by the stairs or the lift (`CLIMB` lines: the floor, the lobby wait,
  the queue); lifts are places with a queue; footbridges, the escalator (down 06-10, up 10-24) and station stairs
  are portals.
- **Vertical gangs (C8, C9b):** Hideouts per district on upper floors over a front; a member sleeps at his nearest;
  raids climb to the target's floor (the lift held or cut by a gang, the stairs past 6); a landlord answers a gang
  that will not pay (the Precinct up, muscle through a Fixer, the lift's power cut, a sale); a raid hits the rival's
  nearest Hideout, two members guard the seat each night.
- **The map (C3, C5, C8b, C9a):** the El on rail (Terrace, Hillside, Mid-Levels, Vats; the tram once); the Spire is
  30-60-floor towers; Farms in the Vats, not the Spire. **Travel choices (C9a):** a move-in weighs both commutes; a
  cab with no driver hands off within the hour; leisure caps read the walk a body would make; the El only when the
  fare is worth the time to him.

**The hard rule that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as
a victim, a combatant, in vice, beaten or fined by a guard, in a cell, courted or married, evicted onto the street,
or dead is a top bug. **Names collide at 100k**: before calling a contradiction, check the entity index or age.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, `CLIMB`, `floor`, `lift`, `lobby`, `flat`,
`rent`, `landlord`, `evict`, `founded`, `Hideout`, `raid`, `went up the stairs`, `held`, `cut`, `Precinct`, `muscle`,
`El`, `rode`, `footbridge`, `escalator`, `moved in`, `Eat`, `Hunt`, `grudge`, `Hit`, `contract`, `Fixer`, `heard`,
`Murder`), read the summaries, then two or three full days that matter. Cite as `file dDD HH:MM`. A missing line is
never evidence.

**Write `D:/TheCity/docs/shadow_v19/<group>.md`, at most 120 lines:**
1. Verdict in three lines: against V18, do these lives read more like people in a vertical city?
2. **The exit's checks in your group**, each quoted if found (cite), else "not seen": (a) a life whose home, post and
   evening fall within a walk in or beside one tower; (b) a gang holding floors over a front, and a raid that climbs
   and lands in a member's diary; (c) a Hideout leg past 4 h (should be none); (d) any minor harmed (should be none).
3. **The vertical city at the player level:** do climbs, lifts, floors and flats read like a place (a lift wait at
   the rush, a flat on the 30th floor, a shop on the ground, a neighbour on the same floor)? Anything absurd (a body
   on floor 40 of a 3-storey building, a climb of hours, a lift that never comes, every flat "flat 1")?
4. Per archetype: alive / thin / dead (and V18's word), one excerpt each.
5. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V18 bug in
   your group fixed / better / unchanged / worse.
6. Dull spots and too-much, briefly.

Read-only: write only your report file. Your final reply is the report's verdict, the exit checks found (with
cites) and the top three bugs.

## Groups

| group | archetypes |
| --- | --- |
| street | homeless, sick, elder, vendor, street_worker, sweeper, camp_warden, miserable, camp_parent, neglect_parent, camp_jobless |
| gangs | gang_member, gang_leader, purist, cultist, scav, dealer, runner, raider |
| crime | forger, fixer, gun, buyer, pimp, arms_dealer, black_doc, jailed, marked (no dodger: the shadow found no fare dodger on day 21, V18 had two; say whether the El's new lines or gates explain it, from the event log `target/v19/events.tsv` (grep, never cat)) |
| corp | ceo, exec, guard, reporter, office_worker |
| labour | worker, worker_friday, night_shift, jobless, fabber, loader, builder, foreman, courier, cabbie, day_labourer |
| vice | club_staff, bouncer, dancer, companion, cook, fighter, joytoy, madam, owner_broke |
| care | ripperdoc, orderly, super, sick, teacher, school_parent, bereaved |
| **stories** | **every diary**: hunt the exit's city stories (a raid that climbs and lands; a landlord against a gang; a life in one tower; a founding into a unit) and VISION's five (a friend killed and a friend who goes after the killer; word of a killing acting on someone; a hit as business; a contract an NPC takes in person; a week that follows from its past) across the whole directory; quote the best instance of each with cites and say whether it reads as a story a player would follow; skip sections 3-6 |
