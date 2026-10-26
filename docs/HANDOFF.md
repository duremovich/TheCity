# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-10-26, the twenty-second session: Shadow V9 at 100k, the pressure pass built (eight mechanism phases, three
fix phases), Shadow V10 read against V9.** The first session under the reset doctrine.

## Main

Main holds **the pressure pass** (`docs/PRESSURE_PASS.md`, every phase's "Implemented: deviations" entry) on the
perf pass. The change tier is green at the merge of phase 5b (`33f9c55`); the test count is still 1,470 (no test
added by the pass; probes whose premise the meaner city moved were re-pinned with the reason in place).
`SAVE_VERSION` 71. The 100k map is `assets/maps/isle_m_k2_100k.txt`; 100k runs at 31 ticks/s with the pass in.

| phase | what it added (each its own module and config section) |
|---|---|
| 1 misery acts (`[misery]`) | skipped shifts and firing, misery quits, petty crime, brawls with grudges, recruitment and conversion of the miserable |
| 4 reach (`[distance.reach]`, `[social]`) | a reach cap per purpose; weak acquaintances pruned in 7 days, 40 a person |
| F1 distance | the city's own hires near the post, beats and patrols near the shift, pitches near home, FleeToHome on the El, registering at the nearest Hall or Station, hunts offered in reach |
| F2 trades and verbs | Lot copies fit the map (the six trades missing at 100k exist), Preach near home, 0-coin shakedowns refused, the witness cap, guards, Bury |
| F3 quits and the jobless | the held cut settles, owners never quit their venue, unpaid walkouts owe a debt, the fired sit out and are refused, the skip is a moment |
| 6 a business under water (`[underwater]`) | cut hours, lay off, borrow (shark, gang, corp; collectors), sell (corp, gang front), close and reopen; the stiffed worker confronts the owner |
| 2 customers (`[customers]`) | the door refuses for cause, lonely clients, the pimp's round, the frightened buy guns, back rooms, the scav's round, hailed cabs, the dawn shape-up, wounds patched |
| 3 protection (`[protection]`) | the levy on held ground, refusals punished (till, owner, fire), the owner's choice, tribute, bail, the Fixer in person |
| 5a consequence (`[consequence]`) | witnesses report, flee, gossip, blackmail; hits reposted or done in person; jail as a place; the faith's weekly services and heretics |
| 5b the gang's side (`[crew]`) | raid results, duty against the day job, court quorums, release welcomed or beaten out, the gear sink capped, rest days with the crew |

**Shadow V9** (`docs/SHADOW_V9.md`, `docs/shadow_v9/`): the 100k city read as commuters with job titles. **Shadow
V10** (`docs/SHADOW_V10.md`, `docs/shadow_v10/`, same window): better in every group; lives have consequences
(chains nobody scripted: a broke back-room doc's slide to eviction and her creditors' confrontation, an exec
reported, fleeing, arrested, defrauded, buying a guard, a super's round turning into a feud). Still thin: the
trades' own verbs on a followed body, and the fare-evasion loop now fills the gaps.

FINDINGs (printed, 100k seed 42 x 30 against V9's run, `OPEN-220`): employed 51.6k -> 39.3k, thefts 3.0k -> 22.5k,
Murders 205 -> 264, starvation deaths 0 -> 34, jailed mean 470 -> 890. The employment drop is mostly F2's per-Lot
venue mix at 100k (the map is short of Lots and has no Sump Lot: `OPEN-204`'s re-import).

## In flight

- No agents are running. Locked empty worktree folders under `.claude/worktrees/` can go after a restart (their
  `target/` dirs are deleted); `target/shadow_v9/`, `target/shadow_v10/`, `target/pp100k/` hold the runs.

## Next, in order

1. **The next pressure pass round from V10** (`OPEN-213`-`OPEN-219`), one agent per phase as this session ran:
   the fare-evasion loop (fines, not cells), gang verbs that stall (raids lose to Sleep, JoinGang walks, Scavenge's
   failing step), walking pace inside a district, the wounded and the sick (rescue, care on credit), the trades'
   verbs on a followed body, the friendship flip bug, diary noise. Then Shadow V11 in V10's window.
2. **The 100k map re-import** (`OPEN-204`, `OPEN-182`): Lots enough for the venue mix and Sump Lots; it is the
   lever on `OPEN-220`'s employment and the execs' commutes.
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

## Process notes from this session

- Parallel coders on one main work, but each merge of branches built apart turned tests red on the combination
  (four fix rounds; one real bug: phase 6's fronts re-staffed a gang that could not reform). Running the change
  tier once on main after each hand-resolved merge caught every one. Union merges of `config.rs` defaults dropped
  closing braces twice; the build catches it.
- Critics on Sonnet with a one-page brief cost ~$1-2 each; the 100k shadow window takes ~40 min.

## Open calls for Dylan

- `OPEN-220`: is the meaner 100k city the city you want (a quarter of adults miserable, 34 starved in a month), or
  should the next round restore jobs first (the map) before adding more pressure?
- Gangs lose members fast under 5b's duty rule (91 colours lost in 30 days at 20k, mostly missed court summons).
- Carried: `OPEN-212`, `OPEN-150`, `OPEN-179`; standing `OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`,
  `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session f370b8f3`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| phase 1 misery | Opus | 254 | 213k | 23 |
| phase 4 reach | Opus | 174 | 143k | 8 |
| F1 distance | Opus | **556** | 321k | **58** |
| F2 trades and verbs | Opus | 511 | 291k | 47 |
| F3 quits | Opus | 228 | 176k | 11 |
| phase 6 under water | Opus | 240 | 233k | 26 |
| phase 2 customers | Opus | 458 | 321k | 41 |
| phase 3 protection | Opus | 281 | 225k | 21 |
| phase 5a consequence | Opus | 309 | 294k | 25 |
| phase 5b the gang | Opus | 387 | 284k | 37 |
| four fix rounds | Sonnet | 224 | 57-91k | 8 |
| Shadow V9 and V10 critics (14) | Sonnet | 616 | 81-122k | 21 |
| orchestrator (runs, briefs, merges, synthesis) | Opus | 379 | 212k | 27 |

Total ≈ $350 for ten phases of mechanism and two shadow reads (the perf pass's two sessions: ≈ $360, no
mechanism). F1 and F2 ran past the ~300-turn brief size; both were diagnosis-heavy.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers in `docs/TESTING.md`.
