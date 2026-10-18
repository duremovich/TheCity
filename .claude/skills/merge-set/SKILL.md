---
name: merge-set
description: Run TheCity's merge tier (the year, the behaviour tier, the god scenarios) on the exact tree about to be merged, and report the footer. Use before any merge to main, when Dylan asks "is it green", or when a coder hands back a phase.
---

# /merge-set

Before anything merges to main, the merge tier runs on **the exact tree being merged** (a coder's final worktree, or main after the merge, never a fix-round tree from an hour ago).

1. Confirm the tree: `git -C <tree> log --oneline -1` and `git -C <tree> status --porcelain` (clean).
2. `python tools/test.py change` then `python tools/test.py merge` in that tree, one after the other, with nothing else building on the box (the year and the god suites spawn their own threads; `core_sanity`'s ticks/s is noise under load).
3. Paste both footers (`[change] GREEN|RED; BUDGET ...`, `[merge] ...`) and every `FAIL` line verbatim. The logs are under `target/test-logs/<tier>-<stamp>/`.
4. A RED step: read the step's log, name the test and the assertion, and either fix it in the tree (then rerun that step with `--only <step>`) or report it as blocked. Never widen a bound to pass; a number that moved for a wanted reason is demoted to a FINDING with the reason in the commit (CLAUDE.md § doctrine).
5. A budget breach is not a block; report it, and the phase pays it back before it closes.

Reply with the tree's sha, the two footers, and any FAIL lines.
