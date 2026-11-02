# Shadow V20: the critic brief (2026-11-02): the view milestone's exit read

The view milestone's read (`docs/VIEW_MILESTONE.md` § "The exit", and each phase's "as built" note), in V19's window
so the two compare one for one: 100k island (`--population 100000 --map assets/maps/isle_m_k2_100k.txt`), seed 42,
days 21-27, one pick per archetype the milestone touched. Main at `target/v20/sha.txt`. Diaries:
`D:/TheCity/target/v20/isle100k_s42_d21/<arch>_<index>.md`; V19's in `D:/TheCity/target/v19/isle100k_s42_d21/`.
The 30-day 100k counters: `target/v20/analyze.txt` (V19's in `target/v19/analyze.txt`); the 20k 30-day counters
`target/v20/analyze20k.txt`.

**What changed since V19** (read the "as built" notes for detail; what a coder already saw reading wrong at 20k is
listed there: say whether it holds at 100k, do not re-report it as new):
- **Crossings (V7):** walkers cross carriageways at TheCityGraphics' zebras and wait at the kerb for the green man
  (`CROSS at the lights ... s at the kerb` lines); a jaywalk is near-forbidden; the D2 footbridges carry their share.
- **The El (V9):** lines stop at stations only; the walk-or-ride choice weighs a minute on foot or on the platform as
  two aboard; the off-screen ride odds recalibrated on the 20k island. `[el]` lines: the ride planned, boarded, off.
- **Raids (V8, V8b):** each raider climbs to the rival's floor in his own body (`CLIMB` in his diary, his own pace);
  the muster holds for a crew of three or breaks up naming who came and where the rest were.
- **The cells (V8):** short terms bought out from the purse or kin's bail, a crowded Precinct's early releases, a
  work detail and cards in the yard, a BreakOut dropped with its reason.
- **Perf (V2, V2b):** nothing a diary shows, unless an evening plan or a Market pick reads oddly.
- **The ring (V5):** the follow-cam's surroundings are Full; not visible in a shadow.

**The hard rule that no one under 18 is ever harmed** holds: every harm line prints ages, so check them; any minor as
a victim, a combatant, in vice, beaten or fined, in a cell, courted or married, evicted onto the street, starved, or
dead is a top bug. **Names collide at 100k**: before calling a contradiction, check the entity index or age.

**Diaries are large.** Never read one whole. Grep (`DAY SUMMARY`, `GOAL`, `CROSS`, `jaywalk`, `[el]`, `EL `,
`footbridge`, `CLIMB`, `Muster`, `raid`, `broke up`, `went up the stairs`, `Precinct`, `bail`, `let out early`,
`fine`, `work detail`, `cards`, `BreakOut`, `Eat`, `starv`, `hungry`, `Murder`), read the summaries, then two or three
full days that matter. Cite as `file dDD HH:MM`. A missing line is never evidence.

**Write `D:/TheCity/docs/shadow_v20/<group>.md`, at most 100 lines:**
1. Verdict in three lines: against V19, do these lives read more like people in the city?
2. **The exit's checks in your group**, each quoted if found (cite), else "not seen": (a) a street crossed at the
   lights with a wait; (b) an El ride a person would take (door to door against the walk); (c) a raid climbed in a
   member's own body; (d) a cell stay that varies day to day or ends by bail or early release; (e) any minor harmed
   (should be none); (f) anyone starving, and who (OPEN-277: 20k showed hundreds of starvation deaths a month).
3. Per archetype: alive / thin / dead (and V19's word), one excerpt each.
4. **Bugs**, ranked by what the player sees first: what, where (cite), likely cause if obvious. Mark each V19 bug in
   your group fixed / better / unchanged / worse (V19's reports: `docs/shadow_v19/`).
5. Dull spots and too-much, briefly.

Read-only: write only your report file. Your final reply is the report's verdict, the exit checks found (with
cites) and the top three bugs.

## Groups

| group | archetypes |
| --- | --- |
| commute | worker, office_worker, courier, cabbie, night_shift, school_parent, day_labourer, teacher, exec, guard |
| street | elder, homeless, camp_parent, neglect_parent (and the 20k starvation in `target/v20/analyze20k.txt` / `events20k.tsv`: who starves, grep the events, never cat) |
| gangs | gang_member, gang_leader, raider, cultist, jailed, dealer |
