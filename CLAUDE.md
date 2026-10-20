# TheCity

A Rust city simulator: ~2,000 (or 20,000) residents in a cyberpunk corporate dystopia, built for emergent story. People must act like people; the player view (M18) is isometric and follows one character. The end state is `docs/VISION.md`; the order of work is `docs/ROADMAP_POST_M14.md`.

## Read first

- `docs/HANDOFF.md`: where main stands now, what is in flight, the next steps. Rewritten every session end.
- `docs/README.md`: the map of the docs (one line each) and where the history went.
- `docs/ARCHITECTURE.md`: the codemap. Crates, tick order, LOD tiers, the brain, which config section feeds which system, how to add a mechanism.
- `docs/OPEN.md`: the one ledger of open items (`OPEN-nnn`). Nothing is open anywhere else.
- `docs/TESTING.md`: the tiers, their budgets and caps, and the rules for adding a test.
- The live milestone spec (named in HANDOFF).

## Commands

Tests run through one script; each tier is minutes, so pick the tier, never run the ignored set blind.

```
python tools/test.py change      # fmt, clippy, every non-ignored test   (~4.5 min; budget 4)
python tools/test.py merge       # + the year, the behaviour tier, the god scenarios, serial (budget 8)
python tools/test.py milestone   # + Full-vs-Statistical parity, the 20k profile (budget 20)
python tools/test.py lint        # ignore reasons, caps, OPEN.md ids
python tools/test.py report      # the close-of-milestone table
```

Each prints a line per binary, the FAIL and FINDING lines, and the wall time against the tier's budget; full output is under `target/test-logs/`. `--only <test-binary>` runs one. Numbers come from a run, not a gate: `cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv`, then `python tools/analyze_run.py run.csv events.tsv`. The CLI must run from the repo root (`assets/config.toml`). `core_sanity`'s ticks/s is meaningful only alone on a quiet box; judge a perf regression by alternating A/B of two binaries, never by one gate run.

Install the commit hook once: `git config core.hooksPath tools/hooks` (fmt check only).

## The doctrine

1. **Gates assert broken-system signals**: a panic, nondeterminism, a coin made or lost, a collapse bound, a mechanism that never fires, the LOD parity rule, a tps floor. **Calibration bands are printed `FINDING` lines, never asserts.** No fine-tuning until the sim is complete (Dylan, 2026-10-07).
2. **Never retune an earlier milestone's knob to pass a gate.** If a change moves a number for a known reason, write the reason in the commit and demote the bullet to a FINDING; do not widen a band to fit.
3. **Every number lives in `assets/config.toml`**, documented like its neighbours, read through `Config`. No literals in systems.
4. **Names live in config.** Corp names are placeholders; no logic keys on a name (`Config::unresolved_names` must stay empty).
5. **Determinism**: every random draw is keyed by a hashed purpose unique to its site; a new mechanism never draws from the world stream. Same seed, same save hash, at any day boundary.
6. **Off switches are byte-identical**: while a phase is in flight it has a switch, and the switched-off city matches main on seeds 42-44 x 30 days. Switches retire when the phase lands.
7. **Every Full-tier mechanism states its Statistical mirror** (or why it has none) in the spec and the commit.
8. **Every save-format change bumps `SAVE_VERSION`** (`citysim/src/save.rs`); a save resumes byte for byte.
9. **Test first**: a mechanism's test fails on the base before the change. Rare mechanisms (≤ 2 of 6 seeds) get a unit test in a seeded world, never a sanity bullet.
10. **Rare events are mechanisms, not rates**: when a fix cuts crime or pressure, restore the tone through features (jobs, rent, prices, rival turf), never a knob.

## Standing decisions (do not re-litigate)

- The city is harsh by design; after the V4 sweep halved Murders, leave it and bring pressure back through features (2026-10-17).
- No social safety net; the dole and school meals are policy levers seeded from `[policy]`, off by default (2026-10-08, P10 2026-10-11).
- Outside extraction stays at `remit_share` 0.3 / `scrip_wage_share` 0.3 (2026-10-13).
- Faith growth is uncapped; the equilibrium comes from interactions (predators, raids, disillusion), designed later (2026-10-17).
- Arasaka, Militech and the rest are placeholder names (2026-10-13).
- The view is isometric; a walking player character comes after v1 (2026-10-08).
- Old workers keep their posts until the frailty exit (~74): no pension, work till the body quits (2026-10-19).
- Purists reject implants only; a deck in the hand is a tool (2026-10-19).
- Lives at work moved free time indoors (the own-Home HangOut); social-contact behaviour rows that the milestone's design moved are re-derived, and street life comes back as a feature (OPEN-109), not a knob (2026-10-19).
- **The game city is 20k; the 2k city is a test fixture only** (2026-10-19). 2k keeps the change and merge tiers fast; calibration, shadow reads, acceptance and "does this feel right" are judged at 20k (on the imported island once it holds 20k; Shadow V6 probes it). A phase's report carries 20k seed 42 × 30 counters beside the 2k ones; nobody tunes 2k to feel good (`OPEN-136`).
- Ticks/s on this box are unreliable (Blender renders, parallel builds): print, never tune.

## Agent rules (orchestrated work)

- A worktree's first step is `git merge --ff-only main`; confirm `git log --oneline -1` shows the sha you were briefed on. Worktrees start at the session-start HEAD, not current main.
- Coders commit on their branch and **never merge to main**; the orchestrator merges after running the merge tier on the exact tree handed back.
- Report numbers against the base (the same seeds on main's binary), never alone.
- Cap a test diagnosis at ~30 minutes, then report it as open with what you tried.
- A tool asserts its own preconditions: a `--count` past the candidates or a `--start-day` past the run is an error, not a silent cap.
- When your work merges: delete your worktree's `target/` (each is 5-13 GB) by its absolute path (`rm -rf D:/TheCity/.claude/worktrees/<wt>/target`; never `cd <wt>; rm -rf target`, which wiped main's `target/` at the Lives at work close when the worktree was gone), and expect to be shut down.
- Subagents do not see the orchestrator's memory (only its index); everything they need is in this file and the docs above. This file auto-loads for a worktree subagent (confirmed 2026-10-18); a brief need not say "read CLAUDE.md".
- A script that rewrites files on Windows writes LF (`newline="\n"` in Python); `git ls-files --eol | grep w/crlf` stays empty.
- An agent's final reply can arrive hours late in a batch; have it also write its report to a scratchpad file and name the path in the brief.
- Commits end with the session's attribution lines (the harness supplies them).

**Token rules** (decided 2026-10-18 after the fifteenth session: one coder across phases 0-4 read 640M tokens at a 520k median context; cost is turns × context, 97 % cache reads, so the levers are turns and context, then the model):

- **One fresh agent per phase**, spawned as Opus 5.5 (`model: "opus"`), ~300 turns; reviewers and fix rounds Sonnet 5.5. Never carry a coder into the next phase.
- **Per phase, measure only:** the phase's device rows, the 42-44 × 120 counters against the base binary, and the off identity on seed 42: one command, `python tools/phase_measure.py --base <sha> --off CITYSIM_X_OFF --arch a,b --cols c,d` (~2 min; main's binary built from `git archive` and run from its own root, cached by sha under `target/measure/`). The full set (42-47, the 42-44 off identity, 20k, alternating A/B, the profile, `shadow --lives --population 20000`, diaries) runs once, at the gate.
- **`test.py change` once at the end of the phase**; during it, only the phase's own tests by name. The merge tier is the orchestrator's `/merge-set` on the exact tree; the milestone tier is the close.
- **Logs stay on disk.** `run --report > run.csv` then `analyze_run.py`; `tail`/`grep` a log, never `cat` one. A log read is paid again on every later turn.
- **The orchestrator waits** for the hand-back, reads the report file, merges. No polling, no re-reading the coder's logs, no re-running its sims, no anchor-verification greps for a brief.
- **The session closes with `python tools/session_cost.py`** and a cost line per phase in HANDOFF.

## Closing a milestone

Run `/close-milestone`: HANDOFF Now rewritten, OPEN.md rows added or closed, deviations written into the spec with `OPEN-nnn` tags, `test.py report` run and every cap restored, agents shut down, worktrees removed, memory pointer updated.
