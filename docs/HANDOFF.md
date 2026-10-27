# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-27, the twenty-fourth session: pressure pass round 3 from Shadow V11 (seven mechanism phases S1-S7, two
fix rounds, the under-18 rule), Shadow V12 read against V11.**

## Main

Main holds **pressure pass round 3** (`docs/PRESSURE_PASS.md` § "Round 3 from Shadow V11", every phase's entry under
"Implemented: deviations"). The change tier is green at the V12 fix round (`1aa78df`, merged `b70fa7d`); tests still 1,470 (none added; probes the combined
city broke were re-pinned with "Round 3" reasons in place). `SAVE_VERSION` 83.

| phase | what it added |
|---|---|
| S1 distance (`[distance.choice]`, `distance::trip`) | the Deal spot, muster, hire, site, Visit, Camp answer and JoinGang chosen by the body's own trip; "stays off the trade", "could not make the trip", "sat out the raid" |
| S2 plans | a walk for a lost post turns back, the order left at the door, the goal back-off (`[brain] cool_backoff`), a muster sat is kept |
| S3 justice | the charge names its victim ("resisted arrest"), jail meals and sleep, the guard's collar, jail duty in blocks, the fare beating's cool-down |
| S4 care at 100k | off-screen patients seen at the doctor on duty (`customers::stat_care`), the Friends' ladder of asks, the super's round in legs |
| S5 trades | the cabbie's cut and rank, the hired hand at the yard, the bouncer's rough client and bar, the beat story and crime desk at 100k, a hit heard of |
| S6 children (`ages::seed_children`) | school-age children seeded, School from 6, the jobless parent's grocery run, `ages::is_protected` at every victim pick, the `kill_by` backstop, the camp beating gone |
| S7 noise | the raid result line, crowd and hourly folds, the petty re-witness skip, the same-crime memory merge, titles by grade, the rest day in the hour lines |
| V12 fix (`OPEN-230`) | the non-lethal backstop: no fight and no fare beating lands on a protected person (`law::resolve_fight_mods` refuses a protected party; defenders, raiders and the crossfire drop them; a protected dodger is turned off the platform; the bin shakedown skips them) |

**The under-18 rule** (CLAUDE.md, Dylan 2026-10-27) is a hard rule, not a config switch: `ages::is_protected` at the
pick, `World::kill_by` and the fight resolver as backstops, `harm_blocked` counting a pick site that missed (0 over 30
days at 100k).

**Shadow V12** (`docs/SHADOW_V12.md`, `docs/shadow_v12/`, V11's window): better than V11 in all seven groups; care
and corp jump most (supers' rounds, orderlies' patients, teachers' classes, reporters' Stories, a raid's headline). No
minor harmed in 110 diaries. Left: walks that eat the day, shifts with no trade inside them, the camp as a number,
children not at School, the 00:07 standing-fact line.

FINDINGs (100k seed 42 x 30, `target/r3_100k/` against `target/r2_100k/`): the city starts 111.2k (+11k children);
employed 39.8k -> 39.9k, deaths by violence 393 -> 441, starvation 10 -> 11, jailed mean 1,028 -> 1,065, homeless max
11.3k -> 11.9k, Treated 3.0k -> 12.3k, Story 3.4k -> 7.1k, Witness 377k -> 190k, ChildTaken 32 -> 3,726. Ticks/s
days 25-29 24.6 -> 22.4; 20k quiet box 277 -> 218 (`OPEN-229`).

## In flight

- No agents are running; no worktrees. `target/shadow_v12/`, `target/r3_100k/`, `target/r3_perf/` hold the runs.

## Next, in order

1. **Pressure pass round 4 from V12** (`docs/PRESSURE_PASS.md` § "Round 4 from Shadow V12"): wave 1 T1 neglect warns
   first and the Camp as a place (`OPEN-231`), T3 plans that still die silently (`OPEN-222`), T4 the trades' inside
   (`OPEN-217`), T6 money and labels that disagree; wave 2 T2 School by travel time (`OPEN-232`), T5 the jobless look
   for work, T7 noise (`OPEN-233`). One Opus coder per phase in a worktree, as rounds 2 and 3 ran. Then Shadow V13 in
   V12's window, and one 100k profile at that close (`OPEN-229`).
2. The 100k map re-import (`OPEN-204`, `OPEN-182`; home-to-post walking, `OPEN-221`): the Lot shortage needs the
   footprint redesign or `OPEN-187` multi-tenancy, a joint job with TheCityGraphics.
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

## Calls Dylan made at the close (2026-10-27)

- Round 4 before the map.
- Neglect warns first; a repeat takes the child (`OPEN-231`).
- School reach by travel time, not a School per district (`OPEN-232`).
- Ticks/s 22.4 at 100k and 218 at 20k accepted again; one profile at the next close (`OPEN-229`).
- Carried, still open: `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`; standing `OPEN-062`, `OPEN-039`, `OPEN-057`,
  `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Process notes from this session

- Two waves (four, then three) on disjoint areas; the combinations went red twice, both premise re-pins by Sonnet
  ($1-2 each). One re-pin excluded a farmer by index (`OPEN-227`): fix rounds key on the premise, never an index.
- My first guess at `OPEN-228` (the children leaking from Ambient) was wrong; a $1 Sonnet diagnosis found S1's hire
  cap. Diagnose before guessing in a brief.
- The critics' brief named the under-18 rule as a top bug, which is how the two non-lethal holes surfaced: a shadow
  cannot check what a line doesn't print (victims' ages), so the backstop at the resolver is what holds the rule.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session 4535a6ef`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| S1 distance | Opus | 392 | 244k | **40** |
| S2 plans | Opus | 222 | 178k | 12 |
| S3 justice | Opus | 387 | 248k | 27 |
| S4 care at 100k | Opus | 372 | 248k | **45** |
| S5 trades | Opus | 453 | 263k | **45** |
| S6 children (+ its read-only harm audit, $14) | Opus | 346 + 290 | 214k | 46 |
| S7 noise | Opus | 323 | 217k | 28 |
| three fix rounds + the OPEN-228 diagnosis | Sonnet | 165 | 62-69k | 5 |
| Shadow V12 critics (7) | Sonnet | 329 | 77-121k | 11 |
| orchestrator (briefs, merges, runs, synthesis) | Opus | ~270 | 114k | 14 |

Total ≈ $275 for seven mechanism phases and a shadow read (the last session: ≈ $225 for six). S1, S4 and S5
ran past the ~300-turn brief; S5 was four trades in one brief, as R6a was: split trades two to a brief next time.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
