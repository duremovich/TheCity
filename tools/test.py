#!/usr/bin/env python3
"""The test tiers, their budgets, the caps and the close report (docs/TESTING.md).

    python tools/test.py change      fmt, clippy, every non-ignored test
    python tools/test.py merge       the year, the behaviour tier, the god scenarios (serial)
    python tools/test.py milestone   the 20k profile (core_20k, god_20k)
    python tools/test.py diag        list the diag-tier tests; --only NAME runs one
    python tools/test.py lint        ignore reasons, caps, OPEN.md ids
    python tools/test.py report      the close-of-milestone table [--since SHA]

Each tier runs its cargo commands one at a time, except the change tier's
tests step, which runs the built test binaries itself, `--jobs` (default
`CHANGE_JOBS`) at a time, its long poles (`CHANGE_BESIDE`, `core_sanity`
first) as jobs of their own. It writes every byte to
target/test-logs/<tier>-<stamp>/<n>-<name>.log (the tests step's under
<n>-tests/) and prints only: one line per
test binary, every FAIL / panicked / FINDING line, and the wall time against
the tier's budget. A budget breach is loud, never a failure (the box is noisy):
the phase that breached pays it back before it closes.

Stdlib only. Run from the repo root.
"""

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / "target" / "test-logs"
OPEN_MD = ROOT / "docs" / "OPEN.md"

# Budgets in seconds (wall, after compile, the tier alone on this box). Change
# only with a line in the commit message.
BUDGETS = {"change": 240, "merge": 480, "milestone": 1200}

# Caps checked by `lint` (docs/TESTING.md § caps).
CAP_GOD_SCENARIOS = 20      # excluding the *_control tests
CAP_ASSERTED_BULLETS = 60   # asserted exactly by core.rs::asserted_bullets_within_cap; the lint count is a report
WARN_FILE_LINES = 1500
PASS_NAMED = re.compile(r"(_review|_fixes|shadow_v\d|_probe)\.rs$")

CARGO = ["cargo"]
TIERS = {
    "change": [
        ("fmt", CARGO + ["fmt", "--all", "--", "--check"]),
        ("clippy", CARGO + ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
        ("tests", CARGO + ["test", "--workspace", "--release", "--no-fail-fast", "--", "--nocapture"]),
    ],
    # The ignored tiers run the test binaries the `--no-run` build left in
    # target/release/deps directly: `cargo test -p <one package>` resolves
    # features for that package alone and rebuilt citysim in release (2m22s)
    # before every step on 2026-10-18. ("step", "<binary stem>", [test args]).
    "merge": [
        ("core_year", "core", ["--ignored", "core_year", "--nocapture"]),
        ("behaviour", "citysim_cli", ["--ignored", "behaviour", "--nocapture"]),
        ("god", "god", ["--ignored", "--nocapture"]),
        ("god_corps", "god_corps", ["--ignored", "--nocapture"]),
        ("god_districts", "god_districts", ["--ignored", "--nocapture"]),
    ],
    "milestone": [
        ("core_20k", "core", ["--ignored", "core_20k", "--nocapture"]),
        ("god_20k", "god_20k", ["--ignored", "--nocapture"]),
    ],
}

# The change tier's long poles (island pass phase 0, OPEN-175): `core_sanity`
# (three seeds x 120 days on three threads, ~45-55 s) and the single test that
# sets each slow binary's wall (a seeded world run for weeks, 5-15 s, while the
# binary's other tests end in a few). Each is a job of its own in the tests
# step's pool, and its binary's job skips it; every test still runs in this
# tier. ("<binary stem>", "<exact test name>"); the name must be unique across
# the workspace (it is the `--skip` filter too).
CHANGE_BESIDE = [
    ("core", "core_sanity"),
    ("person", "probe_laid_off_farm_worker_finds_work_within_14_days"),
    ("person", "probe_buyer_nobody_heard_of_is_never_charged"),
    ("wages", "test_emigrate_cost_keeps_the_identity"),
    ("bodies", "test_refound_only_after_refound_days"),
    ("ambient", "test_materialised_look_like_born_residents"),
    ("weapons", "test_parent_dies_at_death_days_and_takes_its_branches"),
]

# The change tier's tests step runs its jobs (every test binary, the long
# poles above) this many at a time, longest first by the last change run
# (perf pass phase 0, OPEN-201). `cargo test` ran the ~107 binaries one after
# another: each launch of a 16 MB release test binary costs 0.3-0.5 s on this
# box (30-52 s a run, measured 2026-10-24), and most binaries end on one test
# while the rest of the box idles. Every test still runs; a binary runs as
# cargo runs it (its package's directory, its own process), and the tests
# write only to pid-keyed temp paths, so two binaries never share a file.
# Measured on a quiet box (24 threads), the tests step: serial 223 s
# (a08d383), 3 jobs 133 s, 4 jobs 110 s, 6 jobs 87 s; at 6 `core_sanity`
# (68 s alone, 82 s in the pool) is the wall, so more jobs only slow it.
CHANGE_JOBS = 6


def built_binary(stem):
    """The newest test executable for a binary stem under target/release/deps."""
    deps = ROOT / "target" / "release" / "deps"
    cands = [p for p in deps.glob(f"{stem}-*") if p.suffix in (".exe", "") and p.is_file() and not p.name.endswith((".d", ".pdb"))]
    cands = [p for p in cands if re.fullmatch(rf"{stem}-[0-9a-f]{{8,}}(\.exe)?", p.name)]
    if not cands:
        return None
    return max(cands, key=lambda p: p.stat().st_mtime)


def step_command(step):
    """A tier step is either (name, argv) or (name, binary stem, test args)."""
    if len(step) == 2:
        return step[0], list(step[1])
    name, stem, args = step
    exe = built_binary(stem)
    if exe is None:
        return name, None
    return name, [str(exe), *args]

RUNNING = re.compile(r"Running (?:tests[\\/]|unittests )?(\S+?)(?: \(|\s)")
BINARY = re.compile(r"deps[\\/]([A-Za-z0-9_]+?)-[0-9a-f]{8,}(?:\.exe)?\)")
RESULT = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored")
NOISY = re.compile(r"^(test \S+ \.\.\. ok|running \d+ tests?|\s*Compiling|\s*Finished|\s*Blocking)")
LOUD = re.compile(r"FAILED|panicked at|FINDING|^error(\[|:)|BUDGET|assertion")
IGNORE_ATTR = re.compile(r'^\s*#\[ignore(?:\s*=\s*"([^"]*)")?\]')
IGNORE_REASON = re.compile(r"^(tier: (merge|milestone|diag)|gap: OPEN-\d{3})(\b|$)")
TEST_ATTR = re.compile(r"^\s*#\[test\]")
FN_NAME = re.compile(r"^\s*(?:pub\s+)?fn\s+([A-Za-z0-9_]+)")
OPEN_ROW = re.compile(r"^\|\s*(OPEN-\d{3})\s*\|(.*)\|\s*$")
OPEN_STATUSES = {"open", "owned", "closed"}


def fmt_secs(s):
    s = int(round(s))
    return f"{s // 60}m{s % 60:02d}s" if s >= 60 else f"{s}s"


def run_step(name, cmd, log_path, only=None, cwd=ROOT, allow_empty=False):
    """Run one cargo command, log every line, print the few that matter."""
    start = time.time()
    binary, bin_start = (None if cmd[0] == "cargo" else name), start
    rows = []
    with open(log_path, "w", encoding="utf-8") as log:
        log.write(" ".join(cmd) + "\n\n")
        proc = subprocess.Popen(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, encoding="utf-8", errors="replace", bufsize=1)
        for line in proc.stdout:
            log.write(line)
            line = line.rstrip("\n")
            m = BINARY.search(line)
            if m and "Running" in line:
                binary, bin_start = m.group(1), time.time()
                continue
            if line.startswith("   Doc-tests "):
                binary, bin_start = "doctests " + line.split()[-1], time.time()
                continue
            r = RESULT.search(line)
            if r:
                status, passed, failed, ignored = r.groups()
                secs = time.time() - bin_start
                label = binary or name
                print(f"  {secs:6.1f}s  {label:<24} {status:<6} {passed:>4} passed {failed:>3} failed {ignored:>3} ignored")
                rows.append({"binary": label, "status": status, "passed": int(passed), "failed": int(failed),
                             "ignored": int(ignored), "secs": round(secs, 1)})
                continue
            if LOUD.search(line) and not NOISY.match(line):
                print("    " + line[:200])
        proc.wait()
    secs = time.time() - start
    ok = proc.returncode == 0
    if name not in ("fmt", "clippy") and not allow_empty and sum(r["passed"] + r["failed"] for r in rows) == 0:
        ok = False
        print(f"    {name}: ran no test (a renamed filter, or a stale binary under --no-build): RED")
    if name in ("fmt", "clippy"):
        print(f"  {secs:6.1f}s  {name:<24} {'ok' if ok else 'FAILED'}")
        if not ok:
            print(f"    see {log_path.relative_to(ROOT)}")
    return {"step": name, "ok": ok, "secs": round(secs, 1), "binaries": rows, "log": str(log_path.relative_to(ROOT))}


def test_executables():
    """Every test binary of the workspace's release build: (label, exe, its
    package's directory). `cargo test --no-run` is a no-op when built; it
    builds what is stale, as the `cargo test` step did."""
    r = subprocess.run(CARGO + ["test", "--workspace", "--release", "--no-run", "--message-format=json"], cwd=ROOT,
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        return None, r.stderr
    found = []
    for line in r.stdout.splitlines():
        if not line.startswith("{"):
            continue
        m = json.loads(line)
        if m.get("reason") != "compiler-artifact" or not m.get("executable") or not m.get("profile", {}).get("test"):
            continue
        exe = Path(m["executable"])
        label = re.sub(r"-[0-9a-f]{8,}$", "", exe.stem)
        found.append((label, exe, Path(m["manifest_path"]).parent))
    return found, r.stderr


def last_secs():
    """The last change run's seconds per binary label and per long-pole step."""
    s = last_summary("change")
    secs = {}
    for st in (s or {}).get("steps", []):
        secs[st["step"]] = st.get("secs", 0.0)
        for b in st.get("binaries", []):
            secs.setdefault(b["binary"], b["secs"])
    return secs


def run_pool(out, beside, jobs):
    """The change tier's tests step: every test binary and every long pole as
    a job, `jobs` at a time, longest first. Returns (tests result, pole results)."""
    start = time.time()
    exes, err = test_executables()
    if exes is None:
        print("  the test build FAILED")
        for l in err.splitlines():
            if l.startswith("error"):
                print("    " + l[:200])
        return {"step": "tests", "ok": False, "secs": 0.0, "binaries": [], "log": ""}, []
    logs = out / "3-tests"
    logs.mkdir(exist_ok=True)
    by_label = {label: (exe, cwd) for label, exe, cwd in exes}
    skips = [a for (_, t) in beside for a in ("--skip", t)]
    known = last_secs()
    work = []  # (expected secs, kind, name, cmd, cwd)
    for stem, test in beside:
        if stem not in by_label:
            print(f"- {test}: no test binary `{stem}` in the build")
            continue
        exe, cwd = by_label[stem]
        work.append((known.get(f"{stem}::{test}", 60.0), "pole", f"{stem}::{test}",
                     [str(exe), test, "--exact", "--nocapture"], cwd))
    for label, exe, cwd in exes:
        work.append((known.get(label, 30.0), "bin", label, [str(exe), "--nocapture", *skips], cwd))
    work.sort(key=lambda w: -w[0])
    print(f"- tests: {sum(1 for w in work if w[1] == 'bin')} binaries and {sum(1 for w in work if w[1] == 'pole')} "
          f"long pole(s), {jobs} at a time; logs in {logs.relative_to(ROOT)}")
    lock = threading.Lock()
    results = {}

    def worker():
        while True:
            with lock:
                if not work:
                    return
                _, kind, name, cmd, cwd = work.pop(0)
            log = logs / (name.replace("::", "-") + ".log")
            results[name] = (kind, run_step(name, cmd, log, cwd=cwd, allow_empty=(kind == "bin")))

    threads = [threading.Thread(target=worker) for _ in range(max(1, jobs))]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    bins = [r for k, r in results.values() if k == "bin"]
    poles = [r for k, r in results.values() if k == "pole"]
    tests = {"step": "tests", "ok": all(r["ok"] for r in bins) and bool(bins), "secs": round(time.time() - start, 1),
             "binaries": [row for r in bins for row in r["binaries"]], "log": str(logs.relative_to(ROOT))}
    print(f"  {tests['secs']:6.1f}s  {'tests (wall)':<24} {'ok' if tests['ok'] else 'FAILED'}")
    return tests, poles


def cmd_tier(tier, args):
    steps = TIERS[tier]
    if args.only:
        names = [s[0] for s in steps]
        if tier == "change":
            steps = [s for s in steps if s[0] == "tests"]
        elif args.only in names:
            steps = [s for s in steps if s[0] == args.only]
        else:
            print(f"--only {args.only!r} names no {tier} step; the steps are: {', '.join(names)}")
            return 2
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = LOGS / f"{tier}-{stamp}"
    out.mkdir(parents=True, exist_ok=True)
    if not args.no_build:
        print(f"[{tier}] building (release, all targets)...")
        b = subprocess.run(CARGO + ["test", "--workspace", "--release", "--no-run"], cwd=ROOT,
                           capture_output=True, text=True, encoding="utf-8", errors="replace")
        (out / "0-build.log").write_text(b.stdout + b.stderr, encoding="utf-8")
        if b.returncode != 0:
            print("  build FAILED; see", (out / "0-build.log").relative_to(ROOT))
            for l in (b.stdout + b.stderr).splitlines():
                if l.startswith("error"):
                    print("    " + l[:200])
            return 1
    pool = tier == "change" and not args.only
    print(f"[{tier}] {len(steps)} step(s), serial" + (f", the tests {args.jobs} at a time" if pool else "")
          + f"; logs in {out.relative_to(ROOT)}")
    start = time.time()
    results = []
    beside_results = []
    for i, step in enumerate(steps, 1):
        if pool and step[0] == "tests":
            tests, beside_results = run_pool(out, CHANGE_BESIDE, args.jobs)
            results.append(tests)
            continue
        name, cmd = step_command(step)
        if cmd is None:
            print(f"- {name}: no built test binary for `{step[1]}` under target/release/deps (run without --no-build)")
            results.append({"step": name, "ok": False, "secs": 0.0, "binaries": [], "log": ""})
            continue
        if name == "tests" and args.only:
            cmd = CARGO + ["test", "--workspace", "--release", "--no-fail-fast", "--test", args.only, "--", "--nocapture"]
        print(f"- {name}")
        results.append(run_step(name, cmd, out / f"{i}-{name}.log"))
    results += beside_results
    wall = time.time() - start
    budget = BUDGETS[tier]
    failed = [r for r in results if not r["ok"]]
    verdict = "GREEN" if not failed else "RED (" + ", ".join(r["step"] for r in failed) + ")"
    budget_line = (f"BUDGET OK {fmt_secs(wall)} of {fmt_secs(budget)}" if wall <= budget
                   else f"BUDGET BREACHED {fmt_secs(wall)} of {fmt_secs(budget)}: the phase that breached pays it back before it closes")
    print(f"[{tier}] {verdict}; {budget_line}")
    (out / "summary.json").write_text(json.dumps({"tier": tier, "stamp": stamp, "wall": round(wall, 1), "budget": budget,
                                                  "green": not failed, "steps": results}, indent=1), encoding="utf-8")
    return 0 if not failed else 1


# --- the source scans -------------------------------------------------------

def test_files():
    for d in (ROOT / "citysim" / "tests", ROOT / "citysim-cli" / "tests"):
        yield from sorted(d.glob("*.rs"))
    yield ROOT / "citysim-cli" / "src" / "shadow.rs"


def ignore_scan_files():
    """Every Rust file that may hold a test: the tests dirs and the crates' src."""
    seen = set()
    for f in test_files():
        seen.add(f)
        yield f
    for crate in ("citysim", "citysim-cli", "citysim-app"):
        for f in sorted((ROOT / crate / "src").rglob("*.rs")):
            if f not in seen:
                yield f


def scan_ignores():
    """Every #[ignore] attribute: (file, line, fn name, reason or None)."""
    found = []
    for f in ignore_scan_files():
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(lines):
            m = IGNORE_ATTR.match(line)
            if not m:
                continue
            name = None
            for j in range(i + 1, min(i + 6, len(lines))):
                n = FN_NAME.match(lines[j])
                if n:
                    name = n.group(1)
                    break
            found.append((f, i + 1, name, m.group(1)))
    return found


def scan_god():
    """God scenario test fns, excluding the *_control ones."""
    names = []
    for f in (ROOT / "citysim" / "tests").glob("god*.rs"):
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(lines):
            if TEST_ATTR.match(line):
                for j in range(i + 1, min(i + 6, len(lines))):
                    n = FN_NAME.match(lines[j])
                    if n:
                        if n.group(1).startswith("god_") and not n.group(1).endswith("_control"):
                            names.append(n.group(1))
                        break
    return names


def scan_bullets():
    """Asserted mechanism entries in core.rs: tuples in mechanisms() and
    transit_mechanisms(), string literals in jobs2_mechanisms()."""
    src = (ROOT / "citysim" / "tests" / "core.rs").read_text(encoding="utf-8", errors="replace")
    counts = {}
    for fn in ("mechanisms", "sickness_mechanisms", "jobs2_mechanisms", "transit_mechanisms"):
        m = re.search(rf"\nfn {fn}\(\) -> Vec<Mechanism> \{{(.*?)\n\}}", src, re.S)
        body = m.group(1) if m else ""
        tuples = len(re.findall(r'\(\s*"[^"]+",\s*[km]\(', body))
        literals = len(re.findall(r'^\s*"[^"]+",\s*$', body, re.M))
        counts[fn] = tuples if tuples else literals
    return counts


def scan_open():
    """OPEN.md rows: id -> status. None when the ledger does not exist yet."""
    if not OPEN_MD.exists():
        return None
    rows = {}
    dupes = []
    for line in OPEN_MD.read_text(encoding="utf-8", errors="replace").splitlines():
        m = OPEN_ROW.match(line)
        if not m:
            continue
        oid, rest = m.group(1), [c.strip() for c in m.group(2).split("|")]
        status = rest[-1].split()[0].lower() if rest and rest[-1] else ""
        if oid in rows:
            dupes.append(oid)
        rows[oid] = status
    return rows, dupes


def helper_copies(name="world"):
    n = 0
    for f in test_files():
        if "common" in f.parts:
            continue
        n += len(re.findall(rf"^fn {name}\(", f.read_text(encoding="utf-8", errors="replace"), re.M))
    return n


TEST_ATTR = re.compile(r"^\s*#\[test\]\s*$", re.M)
BASELINE_RE = re.compile(r"Test functions: (\d+)")


def scan_test_count():
    """Every `#[test]` under citysim/ and citysim-cli/ (src and tests): the ratchet's count (doctrine 2, 2026-10-26)."""
    n = 0
    for crate in ("citysim", "citysim-cli"):
        for f in (ROOT / crate).rglob("*.rs"):
            if "target" in f.parts:
                continue
            n += len(TEST_ATTR.findall(f.read_text(encoding="utf-8", errors="replace")))
    return n


def test_baseline():
    """The count recorded in docs/TESTING.md (`Test functions: N`); None when the line is missing."""
    doc = ROOT / "docs" / "TESTING.md"
    if not doc.exists():
        return None
    m = BASELINE_RE.search(doc.read_text(encoding="utf-8", errors="replace"))
    return int(m.group(1)) if m else None


def cmd_lint(args):
    problems, warnings = [], []
    open_rows = scan_open()
    for f, ln, name, reason in scan_ignores():
        where = f"{f.relative_to(ROOT)}:{ln} {name or '?'}"
        if reason is None:
            problems.append(f"ignore without a reason: {where}")
        elif not IGNORE_REASON.match(reason):
            problems.append(f"ignore reason not 'tier: merge|milestone|diag' or 'gap: OPEN-nnn': {where} ({reason!r})")
        elif reason.startswith("gap:"):
            oid = reason.split()[1]
            if open_rows is None:
                warnings.append(f"no docs/OPEN.md yet to check {oid}: {where}")
            elif open_rows[0].get(oid) not in ("open", "owned"):
                problems.append(f"gap {oid} is not an open row in docs/OPEN.md: {where}")
    god = scan_god()
    if len(god) > CAP_GOD_SCENARIOS:
        problems.append(f"god scenarios {len(god)} > cap {CAP_GOD_SCENARIOS} (one per reaction class, docs/TESTING.md)")
    bullets = scan_bullets()
    total = sum(bullets.values())
    if total > CAP_ASSERTED_BULLETS:
        problems.append(f"asserted core_sanity bullets {total} > cap {CAP_ASSERTED_BULLETS} {bullets}")
    tests, baseline = scan_test_count(), test_baseline()
    if baseline is None:
        warnings.append("no `Test functions: N` line in docs/TESTING.md: the test-count ratchet has no baseline")
    elif tests > baseline:
        problems.append(f"test functions {tests} > baseline {baseline} (the ratchet, doctrine 2): delete one, or raise "
                        f"`Test functions:` in docs/TESTING.md in a commit whose message says `test-add: <reason>`")
    elif tests < baseline:
        warnings.append(f"test functions {tests} < baseline {baseline}: lower `Test functions:` in docs/TESTING.md")
    for f in test_files():
        n = len(f.read_text(encoding="utf-8", errors="replace").splitlines())
        if n > WARN_FILE_LINES:
            warnings.append(f"{f.relative_to(ROOT)} is {n} lines (> {WARN_FILE_LINES}): split by system")
        if PASS_NAMED.search(f.name):
            text = f.read_text(encoding="utf-8", errors="replace")
            live = len(re.findall(r"^\s*#\[test\]\s*$", text, re.M)) - len(re.findall(r'#\[ignore = "tier: diag"\]', text))
            if live > 0:
                problems.append(f"{f.relative_to(ROOT)} is named after a pass, not a system, and holds {live} gate test(s): fold them")
    if open_rows is not None:
        rows, dupes = open_rows
        for d in dupes:
            problems.append(f"duplicate id in docs/OPEN.md: {d}")
        for oid, st in rows.items():
            if st not in OPEN_STATUSES:
                problems.append(f"bad status {st!r} on {oid} in docs/OPEN.md (open|owned|closed <sha>)")
    for w in warnings:
        print("WARN", w)
    for p in problems:
        print("FAIL", p)
    print(f"lint: {len(problems)} problem(s), {len(warnings)} warning(s); god {len(god)}/{CAP_GOD_SCENARIOS}, "
          f"asserted bullets {total}/{CAP_ASSERTED_BULLETS}, tests {tests}/{baseline}, ignores {len(scan_ignores())}")
    return 1 if problems else 0


def last_summary(tier):
    runs = sorted(LOGS.glob(f"{tier}-*/summary.json")) if LOGS.exists() else []
    if not runs:
        return None
    return json.loads(runs[-1].read_text(encoding="utf-8"))


def cmd_report(args):
    print("## test.py report", dt.date.today().isoformat())
    print()
    print("| tier | last run | wall | budget | |")
    print("|---|---|---|---|---|")
    for tier in TIERS:
        s = last_summary(tier)
        if s is None:
            print(f"| {tier} | never | | {fmt_secs(BUDGETS[tier])} | run it |")
            continue
        flag = "ok" if s["wall"] <= s["budget"] else "**BREACHED**"
        print(f"| {tier} | {s['stamp']} {'green' if s['green'] else 'RED'} | {fmt_secs(s['wall'])} | {fmt_secs(s['budget'])} | {flag} |")
    god = scan_god()
    bullets = scan_bullets()
    ig = scan_ignores()
    by_tier = {}
    for _, _, _, reason in ig:
        key = (reason or "no reason").split(" ")[0].rstrip(":") if reason else "no reason"
        if reason and reason.startswith("gap:"):
            key = "gap"
        elif reason and reason.startswith("tier:"):
            key = reason.split()[1]
        by_tier[key] = by_tier.get(key, 0) + 1
    files = list(test_files())
    lines = sum(len(f.read_text(encoding="utf-8", errors="replace").splitlines()) for f in files)
    print()
    print(f"- god scenarios: {len(god)} of cap {CAP_GOD_SCENARIOS}")
    print(f"- asserted core_sanity bullets: {sum(bullets.values())} of cap {CAP_ASSERTED_BULLETS} {bullets}")
    print(f"- ignored tests: {len(ig)} {dict(sorted(by_tier.items()))}")
    print(f"- test files: {len(files)}, {lines} lines")
    print(f"- `fn world()` copies outside tests/common: {helper_copies('world')}; `fn adults()`: {helper_copies('adults')}")
    if args.since:
        d = subprocess.run(["git", "diff", "--shortstat", args.since, "--", "citysim/tests", "citysim-cli/tests"],
                           cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.strip()
        added = subprocess.run(["git", "diff", "--name-status", "--diff-filter=A", args.since, "--", "citysim/tests", "citysim-cli/tests"],
                               cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.split()
        new_files = [a for a in added if a.endswith(".rs")]
        print(f"- since {args.since}: {d or 'no test changes'}; new test files: {', '.join(Path(p).name for p in new_files) or 'none'}")
    over = []
    if len(god) > CAP_GOD_SCENARIOS:
        over.append("god")
    if sum(bullets.values()) > CAP_ASSERTED_BULLETS:
        over.append("bullets")
    print()
    print("Every cap over → a row in docs/OPEN.md owned by the next milestone before the close is accepted."
          if over else "Every cap met.")
    return 0


def cmd_diag(args):
    diag = [(f, ln, name) for f, ln, name, reason in scan_ignores() if reason and reason.startswith("tier: diag")]
    if args.only:
        name = args.only
        hits = [x for x in diag if x[2] == name]
        if not hits:
            print(f"no diag test named {name}; `test.py diag` lists them")
            return 1
        f = hits[0][0]
        pkg = next((c for c in ("citysim-cli", "citysim-app", "citysim") if c in f.parts), "citysim")
        target = ["--test", f.stem] if f.parent.name == "tests" else (["--lib"] if pkg == "citysim" else [])
        cmd = CARGO + ["test", "--release", "-p", pkg] + target + ["--", "--ignored", name, "--nocapture"]
        print(" ".join(cmd))
        return subprocess.call(cmd, cwd=ROOT)
    for f, ln, name in diag:
        print(f"{name:<48} {f.relative_to(ROOT)}:{ln}")
    print(f"{len(diag)} diag-tier test(s); run one with --only NAME")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("tier", choices=list(TIERS) + ["diag", "lint", "report"])
    ap.add_argument("--only", help="one test binary (change) or step (merge/milestone) or diag test name")
    ap.add_argument("--no-build", action="store_true", help="skip the --no-run build step")
    ap.add_argument("--jobs", type=int, default=CHANGE_JOBS, help="change: test binaries run at a time")
    ap.add_argument("--since", help="report: a base sha for the test-growth lines")
    args = ap.parse_args()
    os.chdir(ROOT)
    if args.tier == "lint":
        return cmd_lint(args)
    if args.tier == "report":
        return cmd_report(args)
    if args.tier == "diag":
        return cmd_diag(args)
    return cmd_tier(args.tier, args)


if __name__ == "__main__":
    sys.exit(main())
