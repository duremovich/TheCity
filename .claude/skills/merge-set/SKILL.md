---
name: merge-set
description: Run TheCity's heavier tiers (merge, then milestone) on main as prints at a close or overnight, and report the footers. Not a per-merge gate since 2026-10-26: a phase merges on the coder's change-tier footer.
---

# /merge-set

Since the process reset (2026-10-26, CLAUDE.md § The doctrine) a phase merges on the coder's `[change] GREEN` footer. This skill runs the heavier tiers **once, on main, at a close or overnight**, as prints that tell the next pass what moved.

1. Confirm main: `git log --oneline -1` and `git status --porcelain` (clean).
2. `python tools/test.py merge` then `python tools/test.py milestone`, one after the other, with nothing else building on the box, in the background (each is minutes; the merge tier's budget is 8, the milestone's 20).
3. Paste both footers (`[merge] GREEN|RED; BUDGET ...`, `[milestone] ...`) and every `FAIL` line verbatim. The logs are under `target/test-logs/<tier>-<stamp>/`.
4. A RED step is a broken-system signal (doctrine 1): read the step's log, name the test and the assertion, and either fix it on main or open an `OPEN-nnn` row owned by the next pass. Never widen a bound to pass; a number that moved for a wanted reason is demoted to a FINDING with the reason in the commit.
5. A budget breach is not a block; report it, and the next pass pays it back.

Reply with main's sha, the two footers, and any FAIL lines.
