---
name: close-milestone
description: Close a TheCity milestone or session: rewrite HANDOFF Now, reconcile docs/OPEN.md, write the spec's deviations, run test.py report and restore every cap, shut down agents, remove worktrees, update the memory pointer. Use when Dylan says a milestone is done, at a session end, or when asked to "close out".
disable-model-invocation: true
---

# /close-milestone

The checklist that keeps the repo honest between sessions. Do every step; paste the evidence (command output, shas) into your reply. Nothing here is optional.

1. **Main is green on the exact tree.** `python tools/test.py merge` on main (the change tier too if anything merged since its last run). Paste the footers. A RED tier blocks the close.
2. **`python tools/test.py report`** (`--since <the milestone's base sha>`). Paste the table. Every cap over (god scenarios, asserted bullets, pass-named files, ignores without a reason) is fixed now or becomes a `docs/OPEN.md` row owned by the next milestone; a budget breach the same. Lint must be green: `python tools/test.py lint`.
3. **`docs/OPEN.md`**: every carry-over the milestone produced gets a row (`OPEN-nnn`, next free id); every row the milestone fixed becomes `closed <sha>`; the spec's "Implemented: deviations" tags each open item with its `OPEN-nnn`. No open list anywhere else (HANDOFF, SHADOW, GOD docs point at OPEN.md).
4. **The spec**: "Implemented: deviations" written (what differs from the spec and why, with the numbers against the base); the god write-up updated or the phase scenarios deleted to `docs/history/` per `docs/TESTING.md` § caps.
5. **`docs/HANDOFF.md`** rewritten as a Now page (≤ 100 lines; never append): main's sha and what it holds, in flight, next steps in order, Dylan's open calls as OPEN pointers, the report's table. Old content that is history goes to `docs/history/SESSIONS.md` (append there, dated).
6. **Agents and worktrees**: shut down every agent of the milestone (`ListAgents`, then a shutdown request to each idle one); `git worktree list`: remove every merged worktree (check `git -C <wt> log main..HEAD` is empty and status clean) and its branch; `git worktree prune`.
7. **Memory**: the `thecity-next-sessions-plan` memory says what comes next, with today's date; `thecity-project-state` stays a pointer. Nothing the repo now records goes into memory.
8. **Commit** the close with the session footer, then `git push` if Dylan's standing instruction for the milestone says so.

Reply with: the two footers, the report table, the OPEN rows added/closed (ids), the agents shut down, the worktrees removed, and the commit sha.
