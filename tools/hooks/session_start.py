#!/usr/bin/env python3
"""SessionStart hook: a short briefing (under ~25 lines) printed into the
session's context. On `compact` only the Now header. Stdlib; runs from the
repo root (Claude Code's cwd); never fails the session."""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent


def git(*args):
    try:
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True,
                              encoding="utf-8", errors="replace", timeout=5).stdout.strip()
    except Exception:
        return ""


def now_block(brief):
    path = ROOT / "docs" / "HANDOFF.md"
    if not path.exists():
        return ["docs/HANDOFF.md is missing: write the Now page."]
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    out = []
    section = None
    for line in lines:
        if line.startswith("## "):
            section = line[3:].strip().lower()
            continue
        if section is None and line.strip() and not line.startswith("# "):
            out.append(line.strip()[:160])        # the header: last rewrite date
            continue
        if brief:
            continue
        if section and section.startswith(("in flight", "next")) and line.strip():
            out.append(line.rstrip()[:160])
    return out[: (2 if brief else 14)]


def stale_worktrees():
    out = []
    for block in git("worktree", "list", "--porcelain").split("\n\n"):
        path = next((l[9:] for l in block.splitlines() if l.startswith("worktree ")), None)
        if not path or Path(path).resolve() == ROOT.resolve():
            continue
        head = git("-C", path, "rev-parse", "--short", "HEAD")
        ahead = git("-C", path, "rev-list", "--count", "main..HEAD")
        dirty = git("-C", path, "status", "--porcelain")
        if ahead not in ("", "0") or dirty:
            out.append(f"  {path}  {head}  ahead {ahead or '?'}  {'dirty' if dirty else 'clean'}")
    return out


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8")  # Windows consoles default to cp1252
    except Exception:
        pass
    brief = "--brief" in sys.argv
    try:
        payload = json.load(sys.stdin) if not sys.stdin.isatty() else {}
    except Exception:
        payload = {}
    brief = brief or payload.get("source") == "compact"
    lines = ["TheCity briefing (tools/hooks/session_start.py; CLAUDE.md has the rules, docs/HANDOFF.md the Now page)"]
    lines += now_block(brief)
    if not brief:
        log = git("log", "--oneline", "-5")
        if log:
            lines.append("git log -5:")
            lines += ["  " + l for l in log.splitlines()]
        wt = stale_worktrees()
        if wt:
            lines.append("worktrees ahead of main or dirty:")
            lines += wt
        status = git("status", "--porcelain")
        if status:
            lines.append(f"main tree: {len(status.splitlines())} uncommitted file(s)")
    print("\n".join(lines[:28]))


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # a hook must never break a session
        print(f"session_start hook: {e}")
    sys.exit(0)
