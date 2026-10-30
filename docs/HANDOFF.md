# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-30, the twenty-seventh session: pressure pass round 7 from Shadow V15 (six phases W1-W6 in two waves, one
fix round), Shadow V16 read against V15, the 100k counters.**

## Main

Main holds **pressure pass round 7** (`docs/PRESSURE_PASS.md` § "Round 7 from Shadow V15", every phase's entry under
"Implemented: deviations"). The change tier is green at `8d0d499` (3m05s, the whole round merged); tests still 1,470
(none added; probes whose premises the phases moved re-pinned in place with "Round 7" reasons, one by the fix round
`664e773`). `SAVE_VERSION` 107.

| phase | what it added |
|---|---|
| W1 the household buys for its children (`shelf::kids_short`, `own_held`, `feeds_household`, `[camp]` W1 keys) | while the household's own units are short of its children's food any free adult of it shops, hungry or fed, both tiers; an adult who can buy keeps the children's share back; a warning scores the run first; the `[family]` line says fed or unfed |
| W2 the war and the case in the diaries (`turf::hearers`, `raid::wait_for_crew`, `consequence::woke`, `law::keeps_from_law`, `[crew] march_wait_ticks`, `[crime.case] fear_courage_max`) | the raid waits for its tail, members hear the war, a called member holds his corner; the sleeper wakes to the short purse and tells or says why not; the finder who saw the murder names the killer or keeps quiet for a reason |
| W3 the guard makes the arrest (`plan::arrest_chain`, `law::serve_at_door`, `[justice]`, `[fares]`, `[care]` W3 keys) | the arrest planned straight to the suspect, the cuffs at the step's start, the unseen warrant served at the door at night, a chase past the shift; the fare one dodger at a time, the broke old and frail booked; a call for help reaches a guard, the doctor's staff or a passer-by |
| W4 the right person in the role (`careers::can_foreman`, `service::install_server`, `[[trades]] no_foreman`, `[care] install_rows`, `install_cut`) | no hire picked by a hand's trade; a chrome install cut and paid by a ripperdoc or black doc on duty |
| W5 contradictions (`Crime::CampEscape`, `vice::set_done`/`on_set`, `consequence::paid_awake`, `stims::Stand`, `Child.school_purse`) | a parent's break-out is Camp Escape; a set counts only the set; sleepers do not witness; the awake payer's line; a stand's witnesses in one line; the fee reads the midnight purse; nobody held marries; a lead says what came of it |
| W6 the dull rooms (`[customers] door_trouble_heat`, `door_stat_barred_p`, `[leisure] bout_purse*`, `bout_hurt*`, `[care] book_slack_ticks`, `[noise] gossip_lines_day`) | the door's gang is the cell's holder, trouble and barred off screen; the bout's purse and the loser's hurt; a named pimp out of the pooled hire; bookings spread to the least-booked clinic; gossip retellings folded |

**Shadow V16** (`docs/SHADOW_V16.md`, `docs/shadow_v16/`, V15's window): better than V15 in every group where round 7
aimed (wars end and members hear them, guards arrest, crimes get their case, the pit has stakes, the gossip flood
gone, the orderly's installs and the fee line fixed); the desk and half the street are still dull; W1 fires but is
nearly invisible in the diaries. **The under-18 rule held** in 227 diaries; `harm_blocked` 0 over 30 days.

FINDINGs (100k seed 42 x 30, `target/r7_100k/` against `target/r6_100k/`): ChildTaken 4,103 -> 3,703, warnings past
14 days 72 -> 18, starvation 18 -> 13; Murder 284 -> 256, Arrests 12.1k -> 11.2k (W5's sleepers off the witness
roll), Raids 216 -> 317; members' war lines 16.3k; sleepers woke to it 455, reported 76; door refusals 7,113 -> 8,927;
bout purses 1,723; street visits 466 -> 784; Marriage 23,267 (16.7k in days 0-6, r6 the same); ticks/s 17.

## Decided

- **The neglect ladder's size at 100k stays as built** (Dylan 2026-10-30: "Lets not worry about the amount of kids
  taken right now"): ~3,700 children taken in 30 days, nearly all from households that cannot pay one night's food
  (`OPEN-231`). Round 8 does not touch the ladder.

## In flight

- No agents are running; no worktrees.
- `target/shadow_v16/`, `target/r7_100k/` hold the runs.

## Next, in order

1. **Pressure pass round 8 from V16** (`docs/PRESSURE_PASS.md` § "Round 8 from Shadow V16", planned 2026-10-30): the promise to
   tell the law and the held and asleep who still act (`OPEN-246`); the target and the bereaved who do not react, the
   wrong hire and the doctor who steals (`OPEN-247`); gangs (`OPEN-241`: the lie-low's enemy, a silent succession, no
   raid lands); the pit and the missing ages on lines (`OPEN-217`); marriages (`OPEN-248`). Same loop.
2. The 100k map re-import (`OPEN-204`, `OPEN-182`; home-to-post walking, `OPEN-221`; the far posts and V16's 4-6 h
   commutes, the guard's patrol start, the cab's Spot legs, "moved nearer work" farther, `OPEN-240`).
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

Carried, still open: `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`, `OPEN-229`; standing `OPEN-062`, `OPEN-039`,
`OPEN-057`, `OPEN-059`, `OPEN-077`, `OPEN-069`, `OPEN-070`. Closed this round: `OPEN-244` (W3), `OPEN-245` (W4).
Opened: `OPEN-246` to `OPEN-248`.

## Process notes from this round

- **Subagents cannot write to the orchestrator's scratchpad** (the harness refuses: "Subagents should return
  findings as text"); the report is the final reply. Briefs no longer name a report file.
- Two red combinations, both seeded premises: the widow probe after W3 + W4 (cleared by W2's own re-pin when W2
  merged next, no fix round), the chip probe after wave 1 (a Sonnet fix round, $1). Merging the next green phase
  before spawning a fix round saved one.
- Starting wave 2 per dependency held again: W6 (no W1 files) started when W2-W4 merged, W5 (the Camp, the fee) after
  W1.
- Each merge's change tier ran on main, 4-5.5 min on a shared box (over the 4-min budget only under load).
- The shadow and the 30-day counters ran at once (41 and 44 min); seven critics read in ~15 min.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session 515f8b34`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| W1 the household buys (one 100k x 30) | Opus | 232 | 195k | 33 |
| W2 the war and the case | Opus | 457 | 287k | **64** |
| W3 the guard's arrest | Opus | 341 | 220k | 45 |
| W4 the right person in the role | Opus | 195 | 144k | 12 |
| W5 contradictions | Opus | 286 | 207k | 22 |
| W6 the dull rooms | Opus | 300 | 225k | 26 |
| fix round (the chip probe) | Sonnet | 22 | 54k | 1 |
| Shadow V16 critics (7) | Sonnet | 400 | 81-133k | 12 |
| orchestrator (plan, briefs, merges, runs, synthesis) | Opus | ~250 | 196k | ~18 |

Round 7 ≈ $232 for six phases and a shadow read (round 6: ≈ $316, round 5: ≈ $369).

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
