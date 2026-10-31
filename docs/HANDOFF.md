# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-31, the twenty-ninth session: the big-picture check, pressure pass round 9 (the last: Y1-Y4, Z1-Z3),
Shadow V18 (the exit read), the pressure pass closed, the city milestone planned.**

## Main

Main holds **the closed pressure pass** (`docs/PRESSURE_PASS.md`, § "Round 9 from Shadow V17" and every phase's
entry under "Implemented: deviations"). Change tier green over the Z set (3m03s on `33b8a17`); tests 1,470 (none
added; 73 pinned to the old pace through `common::r2`, `OPEN-252`). `SAVE_VERSION` 118.

| phase | what it added |
|---|---|
| Y1 hunger in the brain (`fixes::eat_holds`, `[brain] food_tried_*`, `[wounded] care_feed_hunger`) | a started Eat holds, survival first when starving, a failed source tries the next (bin, kitchen, Market, Home, a theft for the less lawful); a laid-up patient is fed |
| Y2 contradictions that are state (`law::on_leave`, `call_backup`, `[justice] backup_*`, `[careers] evicted_days`) | no arrests on leave, the boss inside named, a holed-up leader leads nothing, the second guard, a fare beat is word of the law's force |
| Y3 the narration | a byline, the Camp's count past 64 names, cover shifts, the day_labourer pick |
| **Y4 the walking pace** (`[exec] walk_tiles_per_tick` 21, `step_res`, `ExecCfg::walk_ticks`) | people walk 5 km/h (was 0.24 km/h); every pace reader moved; cars 5x, the El 30 km/h; tile caps that were minutes kept as minutes |
| Z1 the avenger (`hunt.rs`, `[hunt]` Z1 keys, `EventKind::Hunt`) | a killing's grudge hunts (friends too), asks around, waits, hands off when weak, reaches into the cells; every end says how; the killer hears |
| Z2 the taker (`contracts::walk_in_pick`, `ActionKind::TakeJob`, `[fixers]` Z2 keys) | a body takes a contract in person at the Fixer's desk; the mark hears by five routes |
| Z3 the children of the jailed (`camp::jailed_households`) | a child whose adults are all held goes to the Camp the same day (Dylan's call) |

**Shadow V18** (`docs/SHADOW_V18.md`, `docs/shadow_v18/`): the exit's five stories all shown (1 and 4 after Z1, Z2),
nobody starving while Eat loses, frozen days only in the cells, no minor harmed in 237 diaries. 100k seed 42 x 30
(`target/r9_100k/` against `target/r8_100k/`, FINDINGs): starvation 45 -> 0, Starving 11.4k -> 1.1k, raids land
(441), violent deaths 464 -> 756, Murder 244 -> 389, Arrests 13.9k -> 23.1k, jailed mean 1,373 -> 3,133 (max 6,747,
~6 % of the city), ticks/s 18.2 -> 14.2 (on main before Z1-Z3).

## Decided (Dylan, 2026-10-30/31)

- **The pressure pass is over** (roadmap addendum 22): it had an exit and met it. Do not open a round 10 from a ranked
  list; what reads wrong goes to `OPEN.md`.
- **The city milestone is next** (`docs/CITY_MILESTONE.md`, Dylan's answers in it): the Bevy client joins the
  workspace FIRST (CLAUDE.md's standing decision rewritten); the vertical city is pathed levels (floors as tenancies,
  a z, stairs and lifts as portals, lifts a place too, footbridges; no below ground); `k = 2`; a landlord rents floors;
  agents here may write TheCityGraphics on its own branches.
- **The pace is real walking speed now**, to be tuned visually once the follow-cam stands; the cells' load
  (`OPEN-256`) waits for that.
- **Children:** a child whose parents are all jailed goes to the Camp (Z3); a homeless child may be fine if he can
  fend for himself (a later feature, `OPEN-258`); the neglect ladder's size stays (`OPEN-231`).

## In flight

- No agents are running; no worktrees.
- `target/shadow_v18/`, `target/r9_100k/`, `target/z1/` hold the runs.

## Next, in order

1. **The city milestone, wave 1** (`docs/CITY_MILESTONE.md` § Phases): C1 the workspace join (TheCity + TheCityGraphics
   `engine/`) beside C3 the El on rail and the duplicate tram (TheCityGraphics). Then wave 2: C2 the follow-cam on a
   save beside C4 the export for 100k and levels. Then C5 the re-import, C6a/C6b tenancies, C7 levels and lifts, C8
   the vertical gangs, C9 far travel re-read. One Opus coder per phase; WORKSPACE_JOIN.md is stale in the ways the
   planner listed (in the spec's "What exists").
2. At the city milestone's first close: regenerate `assets/stat_table.toml` and read 100k ticks/s on the full Z build
   (`OPEN-255`); the merge and milestone tiers as prints (not run at the pressure pass's close).
3. After it: the view milestone (the camera ring `OPEN-149`, the pace tuned by eye, then `OPEN-256`), M18.

Open from this session: `OPEN-252` (the R2-pinned tests and four behavioural probes), `OPEN-253` (arrival churn,
PlanAborted 277k at 20k), `OPEN-254` (the El and cabs at a walking city), `OPEN-255`, `OPEN-256` (the cells),
`OPEN-257` (the wrong person named), `OPEN-258` (street children), `OPEN-259` (V18's dull and wrong rooms). Carried:
`OPEN-217`, `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`, `OPEN-229`, `OPEN-231`; standing `OPEN-062`, `OPEN-039`,
`OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Process notes from this session

- **A big-picture check paid for itself**: the city milestone's planner, not a shadow critic, found the pace (a
  constant read in every system); eight rounds of diary reads had patched its symptoms. When a class of bug recurs
  across rounds, look for the constant under it.
- **An exit turns a loop into a pass**: V18 graded five named stories instead of ranking seven items, and two
  targeted phases closed it.
- **A stories critic across every diary and the event log** read the exit better than the group critics (who called
  story 1 "found" on an attack with no hunt behind it); keep one for any story-shaped exit.
- **A pace change butterflies a third of the suite**; Y4 pinned 73 tests to the old pace in one helper rather than at
  their killers (the budget): fine once, but re-pin or delete each file as phases touch it (`OPEN-252`).
- Deleting a worktree's `target/` while its agent still has a background job running kills that job; wait for the
  final notification.

## Cost (API-rate $ equivalent, `tools/session_cost.py --last 1`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| Y1 hunger in the brain | Opus | 246 | 165k | 18 |
| Y2 contradictions that are state | Opus | 332 | 209k | 22 |
| Y3 the narration | Sonnet | 68 | 75k | 2 |
| Y4 the walking pace | Opus | 486 | 278k | **60** |
| Z1 the avenger | Opus | 353 | 246k | 32 |
| Z2 the taker | Opus | 261 | 215k | 19 |
| Z3 the children of the jailed | Sonnet | 34 | 64k | 1 |
| the city milestone planner | Opus | 105 | 132k | 6 |
| Shadow V18 critics (8) | Sonnet | 610 | 98-149k | 24 |
| orchestrator (the check, plans, merges, runs, synthesis) | Opus | 282 | 194k | 25 |

Round 9 and the close ≈ $209 for seven phases, the exit read and the city plan (round 8 ≈ $258). Y4 blew the
300-turn brief (486): a pace touches every system and a third of the suite.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
