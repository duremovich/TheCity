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

Tests run through one script. **The change tier is the only per-merge gate**; the merge and milestone tiers run at a milestone close (or overnight), as prints.

```
python tools/test.py change      # fmt, clippy, every non-ignored test   (~1.5 min, six binaries at a time; budget 4)
python tools/test.py merge       # + the year, the behaviour tier, the god scenarios, serial (budget 8): at a close
python tools/test.py milestone   # + the 20k city (budget 20): at a close
python tools/test.py lint        # ignore reasons, caps, the test-count ratchet, OPEN.md ids
python tools/test.py report      # the close-of-milestone table
```

Each prints a line per binary, the FAIL and FINDING lines, and the wall time against the tier's budget; full output is under `target/test-logs/`. `--only <test-binary>` runs one. Numbers come from a run, not a gate: `cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv`, then `python tools/analyze_run.py run.csv events.tsv`. The CLI must run from the repo root (`assets/config.toml`). Ticks/s on this box are a print, never a tune (Blender renders, parallel builds).

Install the commit hook once: `git config core.hooksPath tools/hooks` (fmt check only).

## The doctrine

Rewritten 2026-10-26 (Dylan: "I would rather move fast and break things than spend days of testing hell"; the ten-rule version is in git history at `357ad11`). The deliverable is the game: **add, iterate, shadow.**

1. **Gates assert broken-system signals only**: a panic, a coin made or lost, a collapse bound, a mechanism that never fires, a tps floor. **Calibration bands are printed `FINDING` lines, never asserts.** No fine-tuning until the sim is complete (Dylan, 2026-10-07). Never retune an earlier milestone's knob to pass a gate: a number that moved for a known reason gets the reason in the commit and a FINDING, not a wider band.
2. **A mechanism is proven by a shadow, not a test.** A phase lands when it compiles, the change tier is green and the diaries of the archetypes it touches read right at the player level (`citysim-cli shadow`). What reads wrong goes into the next pass. A unit test is written only where a broken-system signal needs one (the mechanism would otherwise fail silently), the brief names it, and **the test count is a lint ratchet** (`docs/TESTING.md`): a commit that raises it says `test-add: <reason>`.
3. **Every number lives in `assets/config.toml`**, documented like its neighbours, read through `Config`. No literals in systems.
4. **Names live in config.** Corp names are placeholders; no logic keys on a name (`Config::unresolved_names` must stay empty).
5. **Rare events are mechanisms, not rates**: when a fix cuts crime or pressure, restore the tone through features (jobs, rent, prices, rival turf), never a knob.

**Dropped 2026-10-26, on purpose (do not reintroduce):** determinism as a gate (keyed draws stay a habit so a seed can be replayed; no save-hash identity, no thread-count identity, no byte-identical off switches, no scan-vs-index identity proofs); save compatibility (a save must load; an old save need not, `SAVE_VERSION` bumps and nothing tests resume); the Statistical mirror as a test (one sentence in the spec says what the off-screen city does); test-first.

## Standing decisions (do not re-litigate)

- **Move fast and break things** (Dylan 2026-10-26): the perf pass spent two sessions on gates and added no mechanism. No determinism, identity or save-compatibility gates; the change tier is the only per-merge gate; a phase is proven by a shadow; the process was re-examined before Shadow V9 and this file is the result.
- The city is harsh by design; after the V4 sweep halved Murders, leave it and bring pressure back through features (2026-10-17).
- No social safety net; the dole and school meals are policy levers seeded from `[policy]`, off by default (2026-10-08, P10 2026-10-11).
- Outside extraction stays at `remit_share` 0.3 / `scrip_wage_share` 0.3 (2026-10-13).
- Faith growth is uncapped; the equilibrium comes from interactions (predators, raids, disillusion), designed later (2026-10-17).
- Arasaka, Militech and the rest are placeholder names (2026-10-13).
- The view is isometric; a walking player character comes after v1 (2026-10-08).
- Old workers keep their posts until the frailty exit (~74): no pension, work till the body quits (2026-10-19).
- Purists reject implants only; a deck in the hand is a tool (2026-10-19).
- Lives at work moved free time indoors (the own-Home HangOut); social-contact behaviour rows that the milestone's design moved are re-derived, and street life comes back as a feature (OPEN-109), not a knob (2026-10-19).
- **No child is ever harmed** (Dylan 2026-10-27): under 18 nobody is killed, hurt, beaten or worked in vice, by anyone or anything (violence, hits, raids, the Statistical rolls, arrest force, starvation, sickness). A child is never a valid target; neglect still has teeth through the service taking the child, and the work camps stay but beat nobody and feed everyone. At 18 all bets are off. A hard rule, not a config switch (round 3 S6).
- **The game city is 100k; the test tiers stay at 2k and 20k for good** (2026-10-19; 100k Dylan 2026-10-23, `docs/POPULATION_STUDY.md`, `OPEN-184`; tiers Dylan 2026-10-24, `docs/PERF_PASS.md` R3). 100k is read at closes and shadow passes, never by a test tier. The committed 100k map is `assets/maps/isle_m_k2_100k.txt` (perf pass phase 10); calibration and "does this feel right" are judged at 100k on it, at 20k on `isle_m_k2` when 100k is too slow to iterate. Nobody tunes 2k to feel good (`OPEN-136`).
- **M18 waits for the 3D view and a solid sim** (2026-10-20): the order is the pre-view fix pass (Shadow V6), the island holding 20k, the view milestone, then M18 rewritten against the 3D client. Getting the full city to feel natural and alive comes first.
- **Nothing in camera range is Statistical** (2026-10-20): Statistical LOD exists only off camera; everyone the camera (later the player) can see is Full (`OPEN-149`).
- **The player never starts on day 0** (2026-10-20): the world is pre-run or, preferred, built from a starting-condition seed that prepopulates it, for control over the city you get (`OPEN-150`).
- **TheCityGraphics stays a separate repo until the view milestone** (2026-10-20); then its Bevy client joins this workspace outside `default-members` with its own test tier, and the Python/Blender pipeline and the assets stay there, read as a generated-assets path (no LFS, ~1.1 GB of PNGs, Bevy's build would break the change tier).

## Agent rules (orchestrated work)

- A worktree's first step is `git merge --ff-only main`; confirm `git log --oneline -1` shows the sha you were briefed on. Worktrees start at the session-start HEAD, not current main.
- Coders commit on their branch and **never merge to main**. The coder runs `test.py change` once at the end of the phase and pastes its footer; **the orchestrator merges on that footer** and does not re-run tiers per phase.
- **A phase adds no test unless its brief names one** (doctrine 2). A phase's proof is its shadow read: the brief says which archetypes' diaries show the mechanism, the coder reads them and quotes the lines that show it working, and the lines that show it wrong go in the report as the next pass's input.
- Numbers: a phase that claims to move a number gives it before and after on the same seed, from one run each; everything else is one run, printed. Sweeps, profiles, benches and 100k runs happen at a close, once, from a script left running.
- Cap a diagnosis at ~30 minutes, then report it as open with what you tried.
- A tool asserts its own preconditions: a `--count` past the candidates or a `--start-day` past the run is an error, not a silent cap.
- When your work merges: delete your worktree's `target/` (each is 5-13 GB) by its absolute path (`rm -rf D:/TheCity/.claude/worktrees/<wt>/target`; never `cd <wt>; rm -rf target`, which wiped main's `target/` at the Lives at work close when the worktree was gone), and expect to be shut down.
- Subagents do not see the orchestrator's memory (only its index); everything they need is in this file and the docs above. This file auto-loads for a worktree subagent; a brief need not say "read CLAUDE.md".
- A script that rewrites files on Windows writes LF (`newline="\n"` in Python); `git ls-files --eol | grep w/crlf` stays empty.
- An agent's final reply can arrive hours late in a batch; have it also write its report to a scratchpad file and name the path in the brief. Long jobs run in the foreground (a background job finishing does not wake an agent).
- Commits end with the session's attribution lines (the harness supplies them).

**Token rules** (2026-10-18, retuned 2026-10-26: cost is turns × context, 97 % cache reads; the levers are turns and context, then the model):

- **One fresh agent per phase**, spawned as Opus 5.5 (`model: "opus"`), ~300 turns; reviewers and fix rounds Sonnet 5.5. Never carry a coder into the next phase. Adjacent small phases share one brief.
- **The brief spends the coder's turns on the mechanism**, not on measurement: the shadow read and one `test.py change` at the end. No identity runs, no A/B, no counters against a base unless the phase claims to move them.
- **Logs stay on disk.** `run --report > run.csv` then `analyze_run.py`; `tail`/`grep` a log, never `cat` one.
- **The orchestrator waits** for the hand-back, reads the report file, merges. No polling, no re-reading the coder's logs, no re-running its sims, no tier re-runs.
- **The session closes with `python tools/session_cost.py`** and a cost line per phase in HANDOFF.

## Closing a milestone

Run `/close-milestone`: the shadow read (one diary per archetype the milestone touched; what reads wrong goes to OPEN or the next pass), HANDOFF Now rewritten, OPEN.md rows added or closed, deviations written into the spec with `OPEN-nnn` tags, the merge and milestone tiers run once as prints, `test.py report` run and every cap restored, agents shut down, worktrees removed, memory pointer updated.
