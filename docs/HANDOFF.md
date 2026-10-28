# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-28, the twenty-sixth session: pressure pass round 5 from Shadow V13 (eight phases U1-U8 in two waves, two
fix rounds), Shadow V14 read against V13, the 100k counters.**

## Main

Main holds **pressure pass round 5** (`docs/PRESSURE_PASS.md` § "Round 5 from Shadow V13", every phase's entry under
"Implemented: deviations"). The change tier is green at `f5893b7` (2m42s); tests still 1,470 (none added; three
probes whose picks the merges moved were re-pinned with "Round 5" reasons in place). `SAVE_VERSION` 96.

| phase | what it added |
|---|---|
| U1 Schools by child count (`ages::seed_by_children`, `found_on_demand`, `[ages]` U1 keys) | one School per 60 school-age children, sited where the most lack a seat; a district turned away full 5 school days running gets one founded (fires only on a demo root); the fee stays |
| U2 the ladder tightens (`camp::free_parent`, `follow_parents`, `[camp] warn_repeat_days` 30) | a second warning inside 30 days takes; no free parent at home takes at once; an elder's move brings his young children; the `[family]` line after a take; a Camp visit due outranks a search day |
| U3 the soup line (`charity.rs`, `[charity]` U3 keys, `EventKind::SoupLine`) | the starving reach a kitchen with a bowl left by trip; a pot of 150 that runs dry, the line and the turned away; the off-screen line at the soup hour; kitchens share a purse; a soup line opens where 20+ starving stand out of reach |
| U4 who is indoors sees indoors (`law::outdoors`, `sees_scene`, `[crime] open_air`) | a witness sees his own building, or the street within sight from outside; guard sightings, SawCorpse and the off-screen pool the same |
| U5 the forger and the fixer (`gray.rs`, `service::walk_in_sale`, `fixer_hand`, `[gray]` licence keys) | the forger's own walk-ins at his desk; the forged weapons licence; the fixer's book names him (posts, takes with the crew, the cut); contracts refuse a protected mark, taker or crew |
| U6 the pimp and the ripperdoc (`vice.rs`, `care.rs`, `service::pick_server`, `[care]`, `[trade_verbs]`, `[verbs] wait_at_work_ticks`) | the round told, a client sent to a dry pitch, the named pimp off the hire and within `pimp_trip_ticks` of a pitch; patients to the doctor's body, chrome booked by the hour, the blood wash, the pre-shift wait capped |
| U7 contradictions (`law::raise_crime_parties`, `contracts::spent_locate`, `Flow::FareCut`, `industry::foreman_on_hand`) | the thief not robbed; victims, parties and accomplices no witnesses; the raider on his raid; no self-guard; a spent Locate settles; the cab owner's cut; the foreman on hand; an unseen heir learns of a strip; a jailed exec unpaid |
| U8 GangWork (`World::chromed_homes`, `World::guard_grid`) | the Purist pick on an hourly grid, the guard grid once a tick: 100k `goal::GangWork` 30.1 % -> 6.9 % of the profile; `goal::Unwind` now the top goal |

**Shadow V14** (`docs/SHADOW_V14.md`, `docs/shadow_v14/`, V13's window): better than V13 where round 5 aimed (the
forger forges and is caught, the fixer's book, a clinic's day, the take's reason, the soup line, the guard's night),
flat elsewhere. The under-18 rule held in every diary (ages on every harm line; contract lines print no target age).

FINDINGs (100k seed 42 x 30, `target/r5_100k/` against `target/r4_100k/`): starvation 30 -> 7, Starving 13.4k ->
6.4k, school days / child-days 6.5 -> 18.0 % (114 Schools), Witness 269k -> 162k, ChildTaken 677 -> 624, ch_camp 91
-> 119, violence deaths 431 -> 441, jailed mean 1,141 -> 1,052, homeless max 13.1k -> 13.3k, **pimps 1.63 -> 0.2 a
day** (U6's `pimp_trip_ticks` 120), licences sold 120 -> 158; ticks/s 25 -> 24 (shared with the shadow run).

## In flight

- No agents are running. One worktree folder is left locked by its finished agent
  (`.claude/worktrees/agent-a62cc7605f7d41757`, U7's, its `target/` deleted): `git worktree unlock` then remove.
- `target/shadow_v14/`, `target/r5_100k/` hold the runs.

## Calls for Dylan (open)

- **The empty street** (`OPEN-217`): U6's bound names a pimp only within 2 h of a pitch, and at 100k almost nobody
  is. Raise the bound, or a mechanism (the pimp takes the El to the pitch; a pitch where the members live)?
- **The soup lines** (`OPEN-235`, closed): the charity founds up to 3 soup lines a day (150c each) where the starving
  are and shares the kitchens' purses. The pots still run dry for thousands a day; confirm that is the charity you
  meant.
- **The fee keeps the new Schools near empty** (teachers with 1-2 pupils): your call stands (the fee stays); say if
  the empty rooms should become a feature (a School closes, a teacher laid off).
- **Order:** round 6 from V14, or the 100k map re-import first.

## Next, in order

1. **Pressure pass round 6 from V14** (`docs/SHADOW_V14.md` § "Round 6's input"): the ladder hanging at 100k and
   children out of care (`OPEN-231`, `OPEN-242`), the street (`OPEN-217`), violence with no law after it
   (`OPEN-238`), empty rooms beside full ones (`OPEN-232`, `OPEN-217`), the night out at the door (`OPEN-239`),
   homes and fares far from work (`OPEN-240`), gang life without a raid (`OPEN-241`). Plan it on the calls above,
   one Opus coder per phase in a worktree as rounds 2-5 ran, then Shadow V15 in V14's window.
2. The 100k map re-import (`OPEN-204`, `OPEN-182`; home-to-post walking, `OPEN-221`).
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

Carried, still open: `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`, `OPEN-229` (Unwind next); standing `OPEN-062`,
`OPEN-039`, `OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Process notes from this session

- Wave 2 started the moment its dependencies (U1, U4) merged, not after all of wave 1: the waves overlapped.
- Four phases bumped `SAVE_VERSION` to the same number; renumbered at each merge (91-96). A brief can say "main is at
  N; the orchestrator renumbers".
- Two red combinations, three probe re-pins, each a pick the merges moved (no code bug), $1-2 of Sonnet each.
- One coder stalled on its own background job; one nudge resumed it.
- The shadow picks at the start day's 00:00, before the midnight pass names pimps: the pimp needed its own run from
  day 22.
- A cleanup that looped `rm -rf` over worktree paths by variable was refused by the permission check; remove
  worktrees one explicit path at a time.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session 13cbd2db`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| U1 Schools | Opus | 312 | 162k | 30 |
| U2 the ladder | Opus | 282 | 158k | 21 |
| U3 the soup line (two 100k x 30 runs, a 100k shadow) | Opus | 429 | 217k | **81** |
| U4 witnessing indoors | Opus | 186 | 110k | 15 |
| U5 the forger and the fixer (a 100k shadow) | Opus | 395 | 204k | 53 |
| U6 the pimp and the ripperdoc | Opus | 510 | 259k | **80** |
| U7 contradictions | Opus | 404 | 189k | 39 |
| U8 GangWork (a 100k save and two profiles) | Opus | 209 | 132k | 19 |
| two fix rounds | Sonnet | 63 | 53-68k | 3 |
| Shadow V14 critics (7) | Sonnet | 282 | 68-108k | 9 |
| an unlabelled Opus agent | Opus | 110 | 106k | 4 |
| orchestrator (briefs, merges, runs, synthesis) | Opus | 214 | 165k | 14 |

Total ≈ $369 for eight phases and a shadow read (round 4: ≈ $243). U3 and U6 ran far past the ~300-turn brief, with
cache writes of 11-13M each (100k runs and shadows inside the phase); cap 100k work in a phase at one run.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
