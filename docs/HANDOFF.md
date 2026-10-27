# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-27, the twenty-third session: pressure pass round 2 from Shadow V10 (six mechanism phases R1-R6b, three fix
rounds), Shadow V11 read against V10.**

## Main

Main holds **pressure pass round 2** (`docs/PRESSURE_PASS.md` § "Round 2 from Shadow V10", every phase's entry under
"Implemented: deviations") on the pressure pass. The change tier is green at the merge of fix round 3 (`fcdee37`);
tests still 1,470 (none added; probes the moved city broke were re-pinned with the reason in place). `SAVE_VERSION`
76.

| phase | what it added |
|---|---|
| R1 fares (`[fares]`, `systems/fares.rs`) | a fare dodger is fined, else beaten, a repeat dodger banned from the El; never a warrant or a cell |
| R2 pace | walking was v1's toy pace (2 min a tile; `move_ticks_full` 2 -> 1); the escort van by road; guard beats by the guard's Home |
| R3 care (`[care]`, `systems/care.rs`) | the doctor's cut, the pawn, the slate, the Friend's loan, the downed call and the woken street, back rooms reached, the sick post covered |
| R4 noise (`[noise]`) | the friendship flip (only holders who see a blow turn Enemy), the loser's gossip, the super's one notice line, the rest day marked, the Camp answer |
| R5 gangs (`[crew]` keys) | the muster holds, full Hideouts admit their own, the try-out and its refusal, Scavenge, the broke gang's stand-down; court missed at work excused (colours lost 100 -> 33 at 20k) |
| R6a trades I (`[trade_verbs]`) | the cab pool and rides home, the pimp's round, bets at the pit, the madam's bookings, the Dollhouse |
| R6b trades II (`[verbs]`) | the gun hunts (contract quotas per capita), the Hq as the office, the landlord's desk (`OPEN-207` closed), the reporter's beat story, the gangs' arms trade |

**Shadow V11** (`docs/SHADOW_V11.md`, `docs/shadow_v11/`, V10's window): better than V10 in five groups of seven;
care reads worse at 100k (no clinic worker sees a patient, the supers do no round). Most of what's left is a body
that never gets there: distance-blind goals on a map short of Lots, and plans that fail at the end of the walk.

FINDINGs (100k seed 42 x 30, `target/r2_100k/` against V10's `target/pp100k/`): employed 39.3k -> 39.8k, thefts
22.5k -> 17.0k, arrests 16.2k -> 9.3k, Murders 264 -> 242, starvation deaths 34 -> 10, jailed mean 890 -> 1,028.
Ticks/s days 25-30 23.7 -> 24.8 (`OPEN-226`).

**A 20k run needs `--scaled --ambient-flats 50`**: `citysim-cli run --days 30 --seed 42 --population 20000 --scaled
--map assets/maps/isle_m_k2.txt --ambient-flats 50 --report --events > run.csv 2> events.tsv`. Without `--scaled`
the city keeps the 2k config's jobs and starves (it cost a false-alarm bisect this session). `shadow --population`
scales by itself; a full shadow pass needs `--pick` with the archetype list (`target/shadow_v11/picks.txt`).

## In flight

- No agents are running; no worktrees. Locked empty folders under `.claude/worktrees/` can go after a restart.
  `target/shadow_v11/`, `target/r2_100k/`, `target/r2main/` (an unscaled 20k run: ignore) hold the runs.

## Next, in order

1. **Dylan's calls below**, then either:
   - **the 100k map re-import** (`OPEN-204`, `OPEN-182`): V11's top item (`OPEN-221`) is half the map, and it is the
     lever on employment; or
   - **round 3 from V11**: distance in goal choice (`OPEN-221`'s other half), plans that fail at the end
     (`OPEN-222`), justice (`OPEN-223`), care at 100k (`OPEN-216`), the trades left (`OPEN-217`), noise (`OPEN-225`).
     One agent per phase in waves, as this session ran. Then Shadow V12 in V11's window.
2. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

## Open calls for Dylan

- **Map first or round 3 first?** Distance is now the top complaint at 100k; the re-import is a joint job with
  TheCityGraphics.
- `OPEN-224`: the seeded city has no school-age children (Schools empty). Seed them at world generation and gate
  School by age (my take: yes, as its own phase; it changes household make-up citywide).
- `OPEN-226`: 100k reads ~24-25 ticks/s on this run's method against the perf gate's 31; round 2 didn't move it.
  Re-measure at the gate's conditions, then a perf phase or accept.
- R2: the walk is still ~20x slower than a person on 4 m tiles; going faster needs sub-tick steps and an El and
  vehicle rescale.
- Carried: `OPEN-220`, `OPEN-212`, `OPEN-150`, `OPEN-179`; standing `OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`,
  `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Process notes from this session

- Waves of four on disjoint areas worked; every hand-resolved merge was followed by the change tier on main, and the
  combinations went red three times (all premise re-pins by Sonnet, ~$1-2 each, no code bug). One probe
  (`probe_parent_with_the_fee_buys_the_child_out...`) flipped on four merges until fix round 3 found its real cause
  (the employed spouse paid the buy-out) and isolated it.
- Union merges of `config.rs` dropped a Default impl's closing braces again (R3); fixing a conflicted file after
  `git add` left the merge commit broken while the working tree built. `git status` before committing a merge.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session befa23da`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| R1 fares | Opus | 292 | 204k | 17 |
| R2 pace | Opus | 358 | 209k | 35 |
| R3 care | Opus | 273 | 220k | 26 |
| R4 flip and noise | Opus | 301 | 178k | 18 |
| R5 gangs | Opus | 331 | 234k | 25 |
| R6a trades I | Opus | 373 | 231k | **48** |
| R6b trades II | Opus | 332 | 226k | 21 |
| three fix rounds | Sonnet | 170 | 57-77k | 4 |
| the false-alarm bisect | Opus | 58 | 61k | 1 |
| Shadow V11 critics (7) | Sonnet | 279 | 78-109k | 9 |
| orchestrator (briefs, merges, runs, synthesis) | Opus | ~280 | 167k | ~20 |

Total ≈ $225 for six mechanism phases and a shadow read (the last session: ≈ $350 for ten phases and two reads).
R6a ran past the ~300-turn brief (four trades in one brief; three change-tier runs).

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
