#!/usr/bin/env python3
"""Token cost of a Claude Code session, per agent, from the transcripts on disk (CLAUDE.md Token rules).

    python tools/session_cost.py                 the newest session of this project
    python tools/session_cost.py --last 3        the newest three
    python tools/session_cost.py --session ecc6  a session by id prefix
    python tools/session_cost.py --dir PATH      another project's transcript directory

Transcripts live under ~/.claude/projects/<cwd with every non-alphanumeric as '-'>/: one
<session>.jsonl for the orchestrator and <session>/subagents/*.jsonl for each agent it spawned.
Every assistant turn carries `usage`; cost is turns x context, so the table prints per agent the
turns, the median and p90 context, the four token counts, what the shell calls did (sim runs,
test-tier runs, cargo test calls, log reads) and an API-rate dollar equivalent as a proxy for
subscription weight. Prices are the first-party rates (cached 2026-09-25, cache write at 1.25x
input; Claude Code's one-hour cache may bill writes at 2x, which moves the proxy, not the ranking).

Stdlib only. Read-only.
"""

import argparse
import collections
import json
import re
import statistics
import sys
from pathlib import Path

# $ per million tokens: input, cache write, cache read, output.
PRICES = {
    "claude-fable-5-1": (10.0, 12.5, 0.25, 50.0),
    "claude-opus-5-5": (4.0, 5.0, 0.20, 20.0),
    "claude-sonnet-5-5": (2.0, 2.5, 0.20, 10.0),
    "claude-haiku-4-5-20251001": (1.0, 1.25, 0.10, 5.0),
}
KEYS = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens")


def project_dir():
    slug = re.sub(r"[^A-Za-z0-9]", "-", str(Path.cwd().resolve()))
    return Path.home() / ".claude" / "projects" / slug


def scan(path):
    """One transcript: token counts, context per turn, bash categories, the first prompt."""
    tok = collections.Counter()
    by_model = collections.defaultdict(collections.Counter)
    ctx, bash = [], collections.Counter()
    label = None
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            try:
                o = json.loads(line)
            except ValueError:
                continue
            m = o.get("message") or {}
            if o.get("type") == "user" and label is None:
                c = m.get("content")
                text = c if isinstance(c, str) else next(
                    (b.get("text", "") for b in c or [] if isinstance(b, dict) and b.get("type") == "text"), "")
                text = re.sub(r"\s+", " ", text).strip()
                if text and not text.startswith("<"):
                    label = text[:70]
            if o.get("type") != "assistant":
                continue
            u = m.get("usage") or {}
            if not u:
                continue
            model = m.get("model", "?")
            for k in KEYS:
                v = u.get(k) or 0
                tok[k] += v
                by_model[model][k] += v
            ctx.append(sum(u.get(k) or 0 for k in KEYS[:3]))
            for c in m.get("content") or []:
                if isinstance(c, dict) and c.get("type") == "tool_use" and c.get("name") in ("Bash", "PowerShell"):
                    cmd = (c.get("input") or {}).get("command", "")
                    bash["calls"] += 1
                    if "citysim-cli" in cmd and ("-- run" in cmd or "shadow" in cmd or "citysim-cli.exe" in cmd):
                        bash["sim"] += 1
                    if "test.py" in cmd:
                        bash["tier"] += 1
                    if "cargo test" in cmd:
                        bash["cargo_test"] += 1
                    if re.search(r"\b(cat|tail|head|grep|sed -n)\b", cmd) and re.search(r"\.log|\.csv|\.tsv|test-logs", cmd):
                        bash["log_reads"] += 1
    return tok, by_model, ctx, bash, label


def dollars(by_model):
    total = 0.0
    for model, c in by_model.items():
        p = PRICES.get(model)
        if p is None:
            continue
        total += sum(c[k] * p[i] for i, k in enumerate(KEYS)) / 1e6
    return total


def row(name, tok, by_model, ctx, bash, label):
    n = len(ctx)
    med = statistics.median(ctx) / 1e3 if n else 0
    p90 = sorted(ctx)[int(n * 0.9)] / 1e3 if n else 0
    models = ",".join(sorted(m.replace("claude-", "").replace("-20251001", "") for m in by_model))
    print(f"  {name:<22} {models:<18} {n:>5} {med:>6.0f}k {p90:>6.0f}k "
          f"{tok['cache_creation_input_tokens']/1e6:>7.1f}M {tok['cache_read_input_tokens']/1e6:>8.1f}M "
          f"{tok['output_tokens']/1e6:>5.2f}M {dollars(by_model):>7.0f} "
          f"{bash['calls']:>5} {bash['sim']:>4} {bash['tier']:>4} {bash['cargo_test']:>5} {bash['log_reads']:>4}")
    if label:
        print(f"      {label}")


def main():
    # A brief can hold any character; the Windows console is cp1252.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--dir", type=Path, default=None, help="the transcript directory (default: this project's)")
    ap.add_argument("--session", default=None, help="a session id prefix")
    ap.add_argument("--last", type=int, default=1, help="the newest N sessions (default 1)")
    a = ap.parse_args()
    d = a.dir or project_dir()
    if not d.is_dir():
        sys.exit(f"no transcript directory: {d}")
    sessions = sorted(d.glob("*.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True)
    if a.session:
        sessions = [p for p in sessions if p.stem.startswith(a.session)]
        if not sessions:
            sys.exit(f"no session starting with {a.session!r} under {d}")
    else:
        sessions = sessions[: a.last]
    if a.last < 1:
        sys.exit("--last must be at least 1")

    for s in sessions:
        print(f"\n== {s.stem}  ({s.stat().st_size / 1e6:.1f} MB)")
        print(f"  {'agent':<22} {'model':<18} {'turns':>5} {'median':>7} {'p90':>7} "
              f"{'cache_w':>8} {'cache_r':>9} {'out':>6} {'$eq':>7} {'bash':>5} {'sim':>4} {'tier':>4} {'ctest':>5} {'logs':>4}")
        tok, by_model, ctx, bash, _ = scan(s)
        total_tok, total_models = collections.Counter(tok), collections.defaultdict(collections.Counter)
        for m, c in by_model.items():
            total_models[m].update(c)
        row("orchestrator", tok, by_model, ctx, bash, None)
        for sub in sorted((s.parent / s.stem / "subagents").glob("*.jsonl")):
            stok, sby, sctx, sbash, label = scan(sub)
            row(sub.stem.replace("agent-", "")[:22], stok, sby, sctx, sbash, label)
            total_tok.update(stok)
            for m, c in sby.items():
                total_models[m].update(c)
        print(f"  {'TOTAL':<22} {'':<18} {'':>5} {'':>7} {'':>7} "
              f"{total_tok['cache_creation_input_tokens']/1e6:>7.1f}M {total_tok['cache_read_input_tokens']/1e6:>8.1f}M "
              f"{total_tok['output_tokens']/1e6:>5.2f}M {dollars(total_models):>7.0f}")


if __name__ == "__main__":
    main()
