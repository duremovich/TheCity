# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-28, the twenty-fifth session: pressure pass round 4 from Shadow V12 (seven mechanism phases T1-T7 in two
waves, two fix rounds), Shadow V13 read against V12, the 100k profile.**

## Main

Main holds **pressure pass round 4** (`docs/PRESSURE_PASS.md` § "Round 4 from Shadow V12", every phase's entry under
"Implemented: deviations"). The change tier is green at `a70800f` (quiet box 2m16s at wave 1's close); tests still
1,470 (none added; probes the combined city broke were re-pinned with "Round 4" reasons in place, seven of them with
`[search]` off where the search is not the probe's subject). `SAVE_VERSION` 90.

| phase | what it added |
|---|---|
| T1 neglect warns first (`camp.rs`, `[camp]` T1 keys) | the service at the door, a grace, a repeat inside the window takes; the children's share of the pantry on a warning (`camp::adults_share`); let home; placement by reach; the warden's headcount, the fence, visits, break-outs; `EventKind::Custody` |
| T2 School by travel time (`ages::pick_school`) | the nearest School with room within `school_trip_ticks` (walk or El, fares paid), a roll of 30 a teacher, no class at an empty School, the `[family]` line says why; Schools seeded by reach of the Homes (`school_by_reach`) |
| T3 plans say why (`OPEN-222`) | ledger-kind contracts stay on the ledger, a jailed mark waited out or let go, the dropped chase a `Pursuit` line handed to a guard on shift, staff skip the door queue, a sat muster keeps the day's duty, the brawl takes the mark first; raid crossfire drops the protected |
| T4 the trades' inside (`OPEN-217`) | the door's let-in, refusal and throw-out to a body first, the floor's visits to a dancer or Host body, the dancer's sets and tips, the night's line, the foreman at the gate and one `DawnPick` line |
| T5 the jobless search (`systems/search.rs`, `[search]`) | Friends asked, boards and doors walked, taken on at the door, worse work, far posts before the Ambient pool, the day labourer's 06:30 shift; an exec gives up a held post (`OPEN-234` closed) |
| T6 money and labels | who paid a bill and whose till took it, a burned mark wary of a fake job, the gang's man picks at a seized shop, a CEO's call told once and where, no exec hired into a post, the ledger's books audited (`[hack] ledger_audit_*`) |
| T7 noise (`OPEN-233`) | standing facts on the first day and the change, day folds (chases, refusals, cools, gossip), served encounters once a day; **the age on every harm line**; the `sick` pick ill; a refused face that returns is barred (`[verbs] door_return_*`) |

**Shadow V13** (`docs/SHADOW_V13.md`, `docs/shadow_v13/`, V12's window): better than V12 in all seven groups, by
less than V12 gained; the gain moved into the middle of a life (the Camp's inside, the jobless week, the door's
night, the runner's audited ledger). The under-18 rule held and was checkable for the first time: every harm line
prints ages, no minor harmed in 132 diaries, `harm_blocked` 0. Left: witnessing through walls (`OPEN-236`), trades
as labels (`OPEN-217`), contradictions (`OPEN-237`), the ladder's edges (`OPEN-231`), broke adults starving at 100k
(`OPEN-235`), walks (`OPEN-221`).

FINDINGs (100k seed 42 x 30, `target/r4_100k/` against `target/r3_100k/`): employed 39.9k -> 40.1k, Materialised 655
-> 597, deaths by violence 441 -> 431, **starvation 11 -> 30** (adults, 23 on day 29, `OPEN-235`), homeless max 11.9k
-> 13.1k, jailed mean 1,065 -> 1,141, ChildTaken 3,726 -> 677, ch_camp 1,782 -> 91, ch_street 105 -> 266, school
days / child-days 4.0 % -> 6.5 %, Witness 190k -> 269k, Story 7.1k -> 4.1k. Ticks/s days 25-29 22.4 -> 20.8; 20k
quiet box 218 -> 192. The profile: `goal::GangWork` 21.7 % of the 100k profiled time (`OPEN-229`).

## In flight

- No agents are running; no worktrees. `target/shadow_v13/`, `target/r4_100k/`, `target/r4_perf/` hold the runs.

## Calls Dylan made (2026-10-28, on Shadow V13)

- Schools: seeded by child count **and** founded on demand (`OPEN-232`); the 2c fee stays.
- The neglect ladder tightens: a second warning inside 30 days takes; a warning with no free parent at home takes at
  once (`OPEN-231`).
- The starving broke get a Mission meal that reaches them: charity that runs short, not a dole (`OPEN-235`).
- A small perf phase for `goal::GangWork` (`OPEN-229`).
- Round 5 also takes witnessing indoors (`OPEN-236`), the label trades (`OPEN-217`) and the contradictions
  (`OPEN-237`); the search's other edges and the crew / BinContest noise wait for the next noise pass.

## Next, in order

1. **Pressure pass round 5 from V13** (`docs/PRESSURE_PASS.md` § "Round 5 from Shadow V13", planned on the calls
   above): wave 1 U1 Schools by child count and on demand, U3 the starving reach a meal, U4 who is indoors sees
   indoors, U5 the forger and the fixer; wave 2 U2 the ladder tightens, U6 the pimp and the ripperdoc, U7
   contradictions, U8 the GangWork hotspot. One Opus coder per phase in a worktree, as rounds 2-4 ran. Then Shadow
   V14 in V13's window.
2. The 100k map re-import (`OPEN-204`, `OPEN-182`; home-to-post walking, `OPEN-221`): the Lot shortage needs the
   footprint redesign or `OPEN-187` multi-tenancy, a joint job with TheCityGraphics.
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

Carried, still open: `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`; standing `OPEN-062`, `OPEN-039`, `OPEN-057`,
`OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Process notes from this session

- Two waves (four, then three) on disjoint areas, as round 3. The combination went red three times (six premise
  re-pins, $1-3 of Sonnet or orchestrator time each); every one was a moved premise, no code bug.
- Every coder's change tier breached the 4-min budget with three wave-mates building (4m30-4m49, `core_sanity` 200+
  s); main alone ran 2m16s. A breach under load is fine to report, not to chase (the wave-2 brief said so).
- The harness now refuses a subagent's report file in the scratchpad: the hand-back message is the report.
- A `cd` into a worktree from the orchestrator moved the session's cwd there and broke `SendMessage`'s resume of that
  agent ("worktree could not be verified") until the cwd went back to the repo root. Never `cd` into a worktree;
  use `git -C`.
- Three coders stopped on their own background jobs despite the brief's first line; a one-line nudge resumed each.
- T5 switched its search off in five probes, the fix round in two more. Defensible (not their subject), but it hid one
  real regression (a jobless parent's search crowding out the Camp visit), now on the next pass's list.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session 32fcc1d8`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| T1 neglect and the Camp | Opus | 247 | 212k | 30 |
| T2 School by travel | Opus | 272 | 177k | 29 |
| T3 plans that die silently | Opus | 293 | 218k | 27 |
| T4 the door and the yard (two 100k shadows) | Opus | 353 | 243k | **49** |
| T5 the jobless search | Opus | 421 | 258k | 33 |
| T6 money and labels | Opus | 248 | 165k | 22 |
| T7 noise | Opus | 312 | 240k | 26 |
| two fix rounds + the starvation diagnosis | Sonnet | 152 | 65-70k | 4 |
| Shadow V13 critics (7) | Sonnet | 295 | 87-101k | 9 |
| orchestrator (briefs, merges, runs, synthesis) | Opus | ~213 | 167k | 14 |

Total ≈ $243 for seven mechanism phases and a shadow read (round 3: ≈ $275). T5 ran past the ~300-turn brief (four
items under one brief); T4's two 100k shadows were the dearest single cost.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
