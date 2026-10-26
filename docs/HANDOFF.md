# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`; the twenty-first
session's page is its last entry. Last rewrite: **2026-10-26, the twenty-first session: the perf pass closed with its
gate met, and the process reset (CLAUDE.md § The doctrine rewritten on Dylan's call).**

## Main

Main holds **the perf pass, closed** (`docs/PERF_PASS.md`, the gate bullet last under § Implemented) and **the process
reset**. The committed 100k map is `assets/maps/isle_m_k2_100k.txt`; the 20k tier map `isle_m_k2` is the P9 re-import;
`[perf] threads` is 8; `SAVE_VERSION` 68. Every in-flight switch is retired.

| gate item (100k seed 42 x 30, quiet box, 8 threads) | target | measured |
|---|---|---|
| ticks/s, days 25-30 | ≥ 30 | **31** (the pass base: 11 at day 4, sliding) |
| longest tick, days 25-30 | ≤ 1 s | 0.79-0.83 s (the 21:00 pass); day 8's 00:11 job search 1.10 s (`OPEN-212`) |
| build | < 2 min | 64 s (the base: 370 s) |
| thread identity 1 vs 8, 2k x 3 seeds and 20k | identical | identical (the last such proof) |

Prints to plan the view with: 1/4/8/16 threads give 40/45/48/48 ticks/s at 100k (the knee is 8); 2.5k/5k/10k Full
agents give 48/41/32, about 1.4 us per Full agent a tick, so a camera ring of thousands is affordable; peak working
set 2.1 GB; 20k runs at 293 ticks/s. The profile's top sites are the goal scorers (`think_with_offer` 50 s of a 100k
game day, the next five under 14 s each): the data layout (`OPEN-206`) is the lever if the view's share needs more.

Behaviour moved by the pass (FINDINGs, nothing retuned): at 2k, 42-47 x 120 against the pass base, Murders 34 -> 31,
employed 0.770 -> 0.754, homeless 0.013 -> 0.015; at 20k on the new map Murders 37.7 -> 56 (seed 42-44 x 30; the map
moves households toward the Sump, phase 10), Friend edges -55 % (the crew cap, phase 7c, per-capita now like the 2k city).

## The process reset (2026-10-26)

Dylan: "We are back to spending all our time on tests and gates and no progress is being made on the game itself
... I would rather move fast and break things than spend days of testing hell. ... Things don't need to be byte
identical or deterministic. We don't need to worry about save compatibility, since we have no players. We need to
add, and iterate, and shadow to make sure it's working relatively well at the player level."

Why it kept happening: every cleanup pruned tests and none touched the rules that generate them. Five of the ten
doctrine rules were test factories (determinism, byte-identical switches, the mirror, SAVE_VERSION, test-first), and
the orchestrator added a standing rule after every incident. What changed, all on main:

- **CLAUDE.md § The doctrine is five rules**; the dropped ones are named there as dropped on purpose. A mechanism is
  proven by a shadow read, not a test. The change tier is the only per-merge gate; merge and milestone tiers run at a
  close as prints. No off switches, no identity runs, no A/B per phase.
- **The test-count ratchet**: `test.py lint` fails when `#[test]` functions exceed `Test functions: N` in
  `docs/TESTING.md` (1,470 now); raising it takes `test-add: <reason>` in the commit.
- **59 tests deleted** (save round trips 48, determinism 4, parity 1, off identity 3, pins 3; `b286ae9`), the
  off-identity stage out of `tools/phase_measure.py`, parity out of the milestone tier. The scan-vs-index oracle
  tests stay (unit tests of an index, a second of CPU). The change tier is ~2 min, set by `core_sanity`.
- The two skills (`/merge-set` is the close's tiers; `/close-milestone` opens with the shadow read), the SOP rule in
  Dylan's global rules, `docs/TESTING.md` and `docs/ARCHITECTURE.md` match.

## In flight

- No agents are running. Locked empty folders under `.claude/worktrees/` (`agent-*`, `island-f3`, `test-cut` if
  it remains) and the `*.log` tier logs there can go after a restart; `target/gate/` holds the gate's runs (~1 GB).
- TheCityGraphics: master `855d0aa`, **no git remote yet**; the maps on main were imported from it.

## Next, in order

1. **Shadow V9 at 100k on the island, the new way**: one shadow run (`citysim-cli shadow`, 100k on
   `isle_m_k2_100k`, 8 threads), one diary per archetype read by critics (Sonnet, in parallel, as V7 did), the issues
   ranked by what the player would see; then fix rounds as mechanisms, one agent per phase, the change tier once,
   shadow again. No identity proofs, no switches. Watch first: the 20k Murders 56 on the new map, the Friend-edge drop,
   the execs' homes (I9 leaves them 133 tiles from the Hq on average), the Spire's full Homes.
2. **The pressure pass** (`OPEN-178` mood drives behaviour, `OPEN-176` the dead trades, `OPEN-209`/`OPEN-205` the
   crew, `OPEN-210` the acquaintance graph, `OPEN-169`, `OPEN-170`, `OPEN-194`), fed by V9's read.
3. The vertical city (`OPEN-187`, `OPEN-191`), the view milestone, M18.

## Open calls for Dylan

- Add a remote and push TheCityGraphics `855d0aa`.
- The Friend-edge drop at 20k (-55 %, the crew cap): a config value if the city feels too lonely in V9.
- `OPEN-212`: the stagger's ~84 unaudited whole-day stamps; audit or accept (the gate read it as harmless at 100k).
- `OPEN-150`: the starting-condition seed's shape. Pressure-pass design: `OPEN-160`, `OPEN-162`, `OPEN-157`,
  `OPEN-176`, `OPEN-178`; the combat model `OPEN-179`. Standing: `OPEN-062`, `OPEN-039`, `OPEN-057`, `OPEN-059`,
  `OPEN-077`, `OPEN-069`, `OPEN-070`.

## Cost (API-rate $ equivalent, `tools/session_cost.py --session 25a0721b`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| phase 7 | Opus | 332 | 216k | 39 |
| phase 8 (+ a Sonnet order audit) | Opus (+ Sonnet) | 286 (+172) | 203k | 28 (+8) |
| phase 9 | Opus | 190 | 173k | 16 |
| phase 7b | Opus | **436** | 237k | 41 |
| phase 7c | Opus | 198 | 129k | 10 |
| phase 7d | Sonnet | 61 | 75k | 2 |
| phase 10 | Opus | 316 | 189k | 17 |
| the switch retirement | Sonnet | 19 | 55k | 1 |
| the test audit and cut | Sonnet | 125 | 162k | 7 |
| orchestrator (tiers, briefs, the gate, the reset, the handoff) | Fable | 307 | 150k | 53 |

Total ≈ $222; with the twentieth session's $137 the pass cost ≈ $360 and added no mechanism to the game. The next
session's cost line is the first measure of the reset.

The standing decisions, the doctrine and the agent rules are in `CLAUDE.md`; the tiers, budgets and caps in `docs/TESTING.md`.
