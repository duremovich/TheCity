# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite: **2026-10-18, the fourteenth session (the process review), in progress.**

## Main

`git log --oneline -8`. Main holds:

- **The Shadow V4 sweep** (`2712b4a`, 2026-10-17; review round `6184981`): 21 of 23 bugs fixed, the stat table regenerated, three god scenarios re-staged. The city is about half as murderous (Murders 806 → 381 on 42-44). Outcome, numbers and open items: `docs/SHADOW_V4.md` § The sweep.
- **Jobs & lives** closed (`986ac8b`, SAVE_VERSION 34): vice, street trades, weapons and items, the Hq, ages, the gray market, faith, fashion. `docs/JOBS_AND_LIVES.md`.
- Before that, in order: M17 Outside, Scale 20k, the Hit market, transit (the El on by default), P10 policy levers, hunger as sickness, the Real economy, M16a/b, Life L2, M8-M15. `docs/README.md` maps the specs.
- **The process review** (this session, `docs/PROCESS_REVIEW.md`): CLAUDE.md, `tools/test.py`, the short TESTING.md, HANDOFF split, the docs indexed (done); OPEN.md, ARCHITECTURE.md, memory prune, hooks and skills, the test retirement pass (in progress, see § In flight).

Every tier was green on `2712b4a`; `python tools/test.py change` measured 264 s summed on 2026-10-18 (budget 3 min, over).

## In flight

- This session: process review tasks 0-8a are on main (`docs/PROCESS_REVIEW.md` § 6). Two coders in worktrees finish task 8: `fold-coder` (the five pass-named test files folded into system files, `wounded.rs`/`wages.rs` sharing a base world) and `god-coder` (the god set 41 → 20). The orchestrator merges each after `test.py merge` on its tree, then runs `/code-review high` on the session.

## Next, in order

1. **Finish the process review**: OPEN.md seeded and linted; ARCHITECTURE.md verified by five lookups; memory pruned to ~15 files; the SessionStart and pre-commit hooks, `/close-milestone`; the retirement pass (god 41 → ≤ 20, the seven pass-named test files folded, ignore reasons, `tests/common`); `/code-review high` on the session.
2. **Lives at work** (`docs/LIVES_AT_WORK.md`; Dylan's answers to questions 1-4 and 6 recorded; 5, 7, 8 stand at the spec's recommendations): phase 0 on main. One solo coder per the SOP; `test.py merge` on the exact tree before each merge; close with `/close-milestone`.
3. **M18**, the player character (`docs/M18_PLAYER.md`): isometric, one character or squad; shadow runs over many NPC kinds; the life-path playtests.

## Open calls for Dylan

Tracked in `docs/OPEN.md` (kind `decision`). The live ones at this rewrite:

- **The harsh city**: Murders halved after V4; bring the pressure back through features (jobs, rent, prices, rival-turf shakedowns, organ and arms feeds) or leave it. Standing: leave it for now (2026-10-17).
- **Gang wars cool into nothing**: the V4 fixes removed the false enmity; gang-vs-gang vendetta days on seed 42 went 92 → 0, Parleys 37 → 9. The fix is a feature (rival-turf shakedowns, raids that land): Lives at work or before M18.
- **Faith uncapped**: followers reach 34 % by day 120, church purses pile up; equilibrium through interactions (predators, raids on rich churches, disillusion). No hard cap (2026-10-17).
- **The synth parity row** is a printed FINDING (M10's pantry sharing); reversible with carried leftovers plus a dive-rate mirror.
- **Bug-fed FINDINGs after batch 3** (Purist expulsions, organs exported, gang feud days): each is a feature to restore, not a knob.

## Standing decisions

In `CLAUDE.md` § Standing decisions, each with its date. The doctrine and the agent rules are there too; `docs/TESTING.md` has the tiers, budgets and caps.
