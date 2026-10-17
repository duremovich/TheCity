# Process review: making the repo work for agents

2026-10-17, written at the end of the thirteenth session (the NPC shadowing session). Dylan asked whether our memories, instructions and docs are laid out well for agents, whether things fall through the cracks, whether agents spend too long searching, and whether hooks would help. This is the handoff for a dedicated session that does the work. **Do it fresh, as its own session**, before the Lives at work build starts. If `v4fix` has merged, do it after the V4 bug sweep.

## The findings (measured 2026-10-17)

1. **The repo has no CLAUDE.md.** Every brief re-explained the same rules by hand:
   - `git merge --ff-only main` first;
   - the gate doctrine (tests assert broken-system signals; calibration bands are printed FINDINGs);
   - every number is a config key;
   - hashed purposes for determinism;
   - byte-identical off switches;
   - never retune an earlier milestone's knob;
   - the commit footer.

   They live in orchestrator briefs, in memory and in `docs/TESTING.md`, so a subagent spawned without a brief knows none of them.
2. **`docs/HANDOFF.md` is a trap.** It is 127 KB and 560 lines, and its top says "Read this first" under a 2026-10-09 header (eight days and five milestones stale). The current state is the last ~30 lines. It has become a session log, not a handoff.
3. **Memory duplicates the repo and costs context every session.**
   - 47 files (132 KB). `MEMORY.md` (loaded every session) is 11 KB of paragraph-long index lines.
   - `thecity-project-state.md` is 24 KB of milestone history that git and HANDOFF hold.
   - About 20 of the 2026-10-05/06 design memories are now in `docs/VISION.md` and `docs/ROADMAP_POST_M14.md`. Spot-checked hits: majordomo 3 in VISION; Blackwall 4/4; Trauma Team 2/3; isometric 3/4; DataKrash 1/1; hit squad 4/2.
   - The memory rule is "what the repo doesn't record".
4. **There is no map of the code.**
   - 117,576 lines of Rust in `citysim/src`: `config.rs` 9,657, `contracts.rs` 4,774, `components.rs` 4,519.
   - `assets/config.toml` is 3,640 lines; the docs are 2.4 MB of specs.
   - This session the spec writer's four Explore subagents died without reporting and it surveyed the code by hand.
   - Seven critics each grepped their way to causes, some wrong: a coder must re-confirm each.
5. **Open items live in five places, and nothing closes them:**
   - each spec's "Implemented: deviations";
   - HANDOFF carry-over lists;
   - SHADOW gap lists;
   - GOD_SCENARIOS gap lists;
   - memory.

   Examples:
   - The anti-chrome cult that buys chrome: SHADOW_V3 #7 → V4 "worse".
   - Trafficking at 0, no black clinics, and the El slow-down on outages: carried through two milestones.
   - The V3 "robbed do not arm" probe is still `#[ignore]`d.
6. **Process slips this session that structure would have caught:**
   - The shadow tool silently left ~60 % of the 2k picks at Coarse (pins past `[lod] max_full`). Fixed in afb4b4c; the general lesson is that **a tool asserts its own preconditions**.
   - A stale coder (`jl1`) sent a review report hours after its work merged as 2cdde71: nothing shuts an agent down at merge.
   - Stale worktrees hold ~40 GB of `target/`.
   - Superseded docs (GOD_SCENARIOS V1-V11, SHADOW V1-V3, the old milestone specs) sit beside the live ones with no index.
   - Python edits on Windows wrote CRLF (git normalises on commit, so the working copy drifts); there is no `.gitattributes`.

## The tasks, in order

Each task lists what to do and how to check it. Commit per task on main with the session footer.

### 1. A repo `CLAUDE.md` (under ~150 lines)

What it holds:

- **What the project is.** Two lines, with links to `docs/VISION.md` and the roadmap.
- **Where to look:**
  - `docs/HANDOFF.md` (Now);
  - `docs/ARCHITECTURE.md`;
  - `docs/OPEN.md`;
  - `docs/TESTING.md`;
  - the live milestone spec.
- **The doctrine:**
  - gates assert broken-system signals (mechanisms, sanity, determinism, LOD parity, tps floors);
  - calibration bands are printed FINDINGs;
  - never retune an earlier milestone's knob to pass a gate;
  - every number lives in `assets/config.toml`, documented like its neighbours;
  - names live in config (`Config::unresolved_names`), and no logic keys on a corp's name;
  - hashed purposes (`PURPOSE_*`, unique), no world-stream draws in new mechanisms;
  - each phase has an off switch, byte-identical on 42-44 × 30;
  - each mechanism states its Statistical mirror;
  - every save-format change bumps `SAVE_VERSION`;
  - test-first: a test that fails on the base.
- **The loops:** the per-change loop and the merge set (copy from TESTING.md; run gates one at a time; `core_sanity`'s tps only alone on the box).
- **Standing decisions** that agents keep re-litigating:
  - the city is harsh by design;
  - no safety net;
  - extraction 0.3/0.3;
  - faith uncapped pending an equilibrium design;
  - corp names are placeholders.
- **Agent rules for orchestrated work:**
  - worktree first-step `git merge --ff-only main` and confirm the sha;
  - coders don't merge to main;
  - report numbers against the base;
  - cap a diagnosis at ~30 min, then report it open;
  - delete a merged worktree's `target/`.
- **The milestone-close checklist:**
  - HANDOFF Now rewritten;
  - `docs/OPEN.md` updated (every carry-over gets a row or is closed);
  - deviations written into the spec;
  - the memory pointer updated;
  - agents for the milestone shut down;
  - worktrees removed.

**Check:** spawn a fresh general-purpose agent with no brief and ask it "what must a new mechanism have before it merges?" It should answer from CLAUDE.md alone.

### 2. Split HANDOFF; index the docs

- `docs/HANDOFF.md` becomes a ~100-line **Now** page: main's sha and what it holds, what is in flight (agents, worktrees, branches), the next steps in order, the open calls for Dylan, and standing decisions with their dates. Rewrite it at every session end; don't append.
- Move every session log to `docs/history/SESSIONS.md`, oldest first, unchanged.
- Move superseded docs to `docs/history/`: GOD_SCENARIOS V1-V11, SHADOW V1-V3 and their dirs, BIG_PICTURE_2026-10-07, EXPERIMENT_LEARNED_STAT_POLICY, and the closed milestone specs if they are only history.
  - Check first that no test, code comment or doc links break: `grep -rn "docs/" citysim* assets docs`.
  - Fix the links.
- Add `docs/README.md`: one line per live doc (what it is, when to read it) and a pointer to `history/`.

**Check:** `wc -l docs/HANDOFF.md` is under 150, and a grep for each moved file's old path finds nothing outside `docs/history/`.

### 3. `docs/ARCHITECTURE.md`

One agent drafts it from the code, read-only. The orchestrator verifies it with five real lookups: e.g. "where does a tip get paid", "where is the hourly LOD assignment", "how does a Statistical agent eat", "where is a gang's daily economy", "which config section feeds `streetwork`".

What it holds:

- the crates;
- the tick order (`citysim::tick` → systems, in order, per tick, hourly and daily);
- the ECS and component layout;
- the LOD tiers (Full, Coarse, Statistical, Ambient), how a tier is assigned, and the parity rule;
- the brain: utility goals → GOAP plan → exec actions, and where each lives;
- the systems directory, one line each;
- config sections → the system that reads them;
- the save format;
- the events and memory pipeline;
- the test tiers and where each lives;
- the CLI tools (`run`, `shadow`, `calibrate`, `import-graphics`).
- **"How to add a mechanism":** a checklist with a worked example from a recent phase.

Keep it under ~400 lines and link to the specs for the why.

### 4. `docs/OPEN.md`, one ledger of open items

Rows: `id | item | origin (doc § / commit) | kind (bug, feature, calibration, decision for Dylan) | owner (milestone or "unowned") | status`.

Seed it from:

- every "Implemented: deviations" carry-over;
- HANDOFF's open lists;
- SHADOW_V4's 23 bugs (mark the ones `v4fix` fixed) and 12 features (the four Dylan picked are owned by Lives at work);
- GOD_SCENARIOS_V12's gaps;
- `#[ignore]`d probes (`grep -rn "#\[ignore\]" citysim*/`; each ignored test that is a known-broken probe gets a row);
- Dylan's open calls (the harsh city, synth parity, faith equilibrium).

Remove each carry-over list from HANDOFF and point at OPEN.md instead.

**Check:** every V3 gap that V4 found unchanged has a row.

### 5. Prune memory to about 15 files

Keep:

- **About Dylan and how he wants the work done:**
  - `feedback-*`;
  - the orchestration lessons;
  - check-in-on-agents;
  - no-fine-tuning;
  - emergent-story-first;
  - brutality-via-pressure;
  - the view is isometric;
  - the player character later.
- **Decisions that live nowhere in the repo.**
- **Pointers:** the next-sessions plan, and the graphics repo path.

Delete each design memory **only after confirming** its content is in VISION.md, the roadmap or a spec (grep its key phrases). Fold anything missing into VISION.md first.

Shrink `thecity-project-state.md` to a pointer at HANDOFF Now and CLAUDE.md. Rewrite `MEMORY.md` with one short line per memory (under ~150 characters each).

**Check:** `MEMORY.md` under ~3 KB; every deleted memory's key facts found in a repo doc.

### 6. Hooks, hygiene, tool asserts

- **SessionStart hook** (project `.claude/settings.json`; use the `update-config` skill). A script prints:
  - HANDOFF's Now section (or its first ~60 lines);
  - `git log --oneline -5`;
  - any worktree whose HEAD is not an ancestor of main, or that is behind main with uncommitted changes.

  Keep the output short.
- **A git pre-commit hook:** `cargo fmt --all -- --check` only. Clippy and tests are too slow for a hook and stay in the loop. Put it in `tools/hooks/pre-commit` with a one-line install note in CLAUDE.md (`git config core.hooksPath tools/hooks`).
- **No "did you update HANDOFF" Stop hook.** It fires every turn and becomes noise; the close checklist in CLAUDE.md carries it.
- **`.gitattributes`:** `* text=auto eol=lf`, then `git add --renormalize .` in its own commit.
- **Worktrees.**
  - Remove every merged worktree and its branch:
    - `agent-a5bb0c78c902534e7`
    - `agent-a864f625a5712bd95` (jl1)
    - `agent-abdc8c1d22393b1ee`
    - `agent-acd591f0773462fb0`
    - `agent-ad4e5963358a7fa4c`
    - `jobs-spec`
    - `m17b`
    - the two scratchpad worktrees
  - Before removing each one, check `git -C <wt> log main..HEAD` is empty and `status` is clean. Ask Dylan about any that isn't.
  - Not `v4fix`'s worktree while it is in flight.
- **Tool preconditions:** look for other silent caps like the shadow overflow (e.g. a `--count` past the candidates, a `--start-day` past the run). Make each one an error or a loud note.

## Out of scope

Sim code, the V4 bug sweep (`v4fix`) and the Lives at work build (`docs/LIVES_AT_WORK.md`, Dylan's answers recorded).
