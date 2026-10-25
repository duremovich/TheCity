#!/usr/bin/env python3
"""The per-phase measurement set as one command (CLAUDE.md Token rules).

Usage:
  python tools/phase_measure.py --base SHA --off CITYSIM_STREET_OFF \\
      [--arch cabbie,courier,loader] [--cols cab_rides_driven,parcels_delivered] \\
      [--events Pilfered,CabFare] [--seeds 42,43,44] [--days 120] \\
      [--off-seeds 42] [--off-days 30] [--skip off,counters,lives] [--jobs 6] [--out DIR]

It prints one table, and writes it to <out>/measure.md, for the three things a phase reports:

1. **Off identity**: the candidate with every `--off` variable set to 1 against the base binary,
   `--off-seeds` x `--off-days` (default 42 x 30): events byte-identical, the report CSV's shared
   columns identical (ticks_per_sec aside), the end-of-run save identical once `save_version:N` is
   masked. Any difference prints `OFF IDENTITY: DIFFER` and the script exits 2.
2. **Counters**: base and candidate (switches on), `--seeds` x `--days` (default 42-44 x 120):
   Murders, J24 (employed share from day 30), J25 (homeless share), arrests, last-day wallets,
   ticks/s (print only: the runs share the box), every `--cols` column summed over the run and every
   `--events` kind counted, per seed and the mean, base -> candidate.
3. **Device rows**: `shadow --lives` (2k, 42-44 pooled) on both binaries, the `--arch` rows and the
   `all` row: n, idle h/d, short rest/d (an idle Rest cut within 5 min; a binary without the column prints -), shifts, Served-able, met, enc/shift.

The base is built from `git archive <sha>` into `<main repo>/target/measure/base-<sha>/` with its own
target dir and is **run from that source root** (its `assets/config.toml` lacks the phase's keys; running
main's binary from the worktree root reads the wrong config). The base build and every base run are
cached there by sha and parameters, so a second call pays only for the candidate. Delete that directory
to rebuild. The candidate is `cargo build --release -p citysim-cli` in the current directory, copied to
<out>/ before any run; run from the repo root.

Logs (CSV, events, shadow output, cargo output) stay under <out>; read measure.md, not the logs.
Stdlib only.
"""
import argparse
import concurrent.futures as cf
import csv
import hashlib
import io
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tarfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import analyze_run  # noqa: E402  (load_csv, load_events, analyze)

EXE = ".exe" if os.name == "nt" else ""
TPD = 1440
LIVES_FIELDS = [("n", r"n (\d+)"), ("idle h/d", r"idle_h/d ([\d.]+)"), ("short rest/d", r"short_rest/d ([\d.]+)"),
                ("shifts", r"shifts (\d+)"),
                ("Served-able", r"servedable ([\d.]+)"), ("met", r"met ([\d.]+)"), ("enc/shift", r"enc/shift ([\d.]+)")]


def die(msg):
    sys.exit(f"phase_measure: {msg}")


def git(*args, cwd=None):
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if r.returncode != 0:
        die(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout.strip()


def log(msg):
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def build(src: Path, target: Path, logfile: Path) -> Path:
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    with open(logfile, "w") as f:
        r = subprocess.run(["cargo", "build", "--release", "-p", "citysim-cli"], cwd=src, env=env,
                           stdout=f, stderr=subprocess.STDOUT)
    if r.returncode != 0:
        die(f"cargo build failed in {src}; tail {logfile}")
    exe = target / "release" / f"citysim-cli{EXE}"
    if not exe.exists():
        die(f"no binary at {exe}")
    return exe


def base_tree(sha: str, root: Path) -> tuple[Path, Path]:
    """Main's source at `sha` (git archive) and its binary, cached by sha.

    Every call re-syncs the source to the archive (a file whose bytes differ is rewritten, so its mtime moves
    and cargo rebuilds it: an agent that edited the cached source and reverted it leaves no stale binary),
    runs `cargo build` (a no-op when up to date) and stamps the binary's hash: when the hash changes, every
    cached base run under `root` is dropped (2026-10-19: a cache built from an edited source read DIFFER
    against a fresh build of the same sha)."""
    src, target = root / "src", root / "target"
    tar = subprocess.run(["git", "archive", "--format=tar", sha], capture_output=True)
    if tar.returncode != 0:
        die(f"git archive {sha}: {tar.stderr.decode(errors='replace')}")
    rewritten = 0
    with tarfile.open(fileobj=io.BytesIO(tar.stdout)) as t:
        for m in t.getmembers():
            dest = src / m.name
            if m.isdir():
                dest.mkdir(parents=True, exist_ok=True)
                continue
            if not m.isfile():
                continue
            data = t.extractfile(m).read()
            if dest.exists() and dest.read_bytes() == data:
                continue
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
            rewritten += 1
    if rewritten:
        log(f"base {sha[:8]}: {rewritten} source file(s) written from the archive")
    exe = target / "release" / f"citysim-cli{EXE}"
    if not exe.exists():
        log(f"building base {sha[:8]} (first time for this sha; minutes)")
    exe = build(src, target, root / "cargo.log")
    digest = hashlib.sha1(exe.read_bytes()).hexdigest()
    stamp = root / "exe.sha1"
    if not stamp.exists() or stamp.read_text().strip() != digest:
        stale = [d for d in root.iterdir() if d.name.startswith(("off_", "offoff_", "run_")) or d.name.startswith("lives")]
        if stale and stamp.exists():
            log(f"base {sha[:8]}: binary changed, dropping {len(stale)} cached run(s)")
        for d in stale:
            shutil.rmtree(d) if d.is_dir() else d.unlink()
        stamp.write_text(digest)
    return src, exe


def run_city(exe: Path, cwd: Path, seed: int, days: int, outdir: Path, env_extra=None, save=False):
    """`run --report --events` to outdir/run.csv, events.tsv; returns outdir. Skips if already done."""
    done = outdir / "done"
    if done.exists():
        return outdir
    outdir.mkdir(parents=True, exist_ok=True)
    args = [str(exe), "run", "--days", str(days), "--seed", str(seed), "--report", "--events"]
    if save:
        args += ["--save-at", str(days * TPD), "--saves-dir", str(outdir / "saves")]
    env = dict(os.environ, **(env_extra or {}))
    with open(outdir / "run.csv", "w") as so, open(outdir / "events.tsv", "w") as se:
        r = subprocess.run(args, cwd=cwd, env=env, stdout=so, stderr=se)
    if r.returncode != 0:
        die(f"run failed (seed {seed}, {exe}); tail {outdir / 'events.tsv'}")
    done.write_text("ok")
    return outdir


def run_lives(exe: Path, cwd: Path, out: Path):
    if out.exists() and out.stat().st_size > 0:
        return out
    tmp = out.with_suffix(".tmp")
    with open(tmp, "w") as so, open(out.with_suffix(".err"), "w") as se:
        r = subprocess.run([str(exe), "shadow", "--lives"], cwd=cwd, stdout=so, stderr=se)
    if r.returncode != 0:
        die(f"shadow --lives failed ({exe}); tail {out.with_suffix('.err')}")
    tmp.replace(out)
    return out


# ---------------------------------------------------------------------------------------------- off identity

def csv_cols(path: Path):
    with open(path, newline="") as f:
        rows = [r for r in csv.reader(f) if r]
    return rows[0], rows[1:]


def off_compare(base: Path, cand: Path, save_masks=()):
    """Returns (identical, lines). Each `save_masks` regex is blanked in both saves (a known format change);
    one that matches neither save is an error."""
    lines, ok = [], True
    eb, ec = (base / "events.tsv").read_bytes(), (cand / "events.tsv").read_bytes()
    # The `saved <path>` line names the run's own directory; drop it before comparing. It can land glued to
    # the end of the last event's line (no newline between them), so it is cut wherever it starts.
    strip = lambda b: re.sub(rb"saved [^\n]*\.ron\r?\n?", b"", b)
    same_ev = strip(eb) == strip(ec)
    ok &= same_ev
    lines.append(f"events {'identical' if same_ev else 'DIFFER'} ({eb.count(b'\n')} lines base, {ec.count(b'\n')} cand)")
    hb, rb = csv_cols(base / "run.csv")
    hc, rc = csv_cols(cand / "run.csv")
    shared = [h for h in hb if h in hc and h != "ticks_per_sec"]
    ib, ic = {h: i for i, h in enumerate(hb)}, {h: i for i, h in enumerate(hc)}
    diff_cells, diff_cols = 0, set()
    if len(rb) != len(rc):
        ok = False
        lines.append(f"report rows DIFFER: {len(rb)} base, {len(rc)} cand")
    for a, b in zip(rb, rc):
        for h in shared:
            if a[ib[h]] != b[ic[h]]:
                diff_cells += 1
                diff_cols.add(h)
    ok &= diff_cells == 0
    new = [h for h in hc if h not in hb]
    gone = [h for h in hb if h not in hc]
    lines.append(f"report {len(shared)} shared columns x {min(len(rb), len(rc))} rows: "
                 + ("identical" if diff_cells == 0 else f"DIFFER in {diff_cells} cells, columns {sorted(diff_cols)[:12]}")
                 + f"; {len(new)} new columns" + (f", {len(gone)} gone: {gone[:8]}" if gone else ""))
    sb = sorted((base / "saves").glob("*.ron"))
    sc = sorted((cand / "saves").glob("*.ron"))
    if len(sb) != 1 or len(sc) != 1:
        ok = False
        lines.append(f"save MISSING (base {len(sb)}, cand {len(sc)} files)")
    else:
        vb, vc = sb[0].read_bytes(), sc[0].read_bytes()
        pat = re.compile(rb"save_version:\s*(\d+)")
        mb, mc = pat.search(vb), pat.search(vc)
        # The saved stats history holds each day's wall-clock ticks_per_sec: noise, masked with the version.
        tps = re.compile(rb"ticks_per_sec:\s*[-\d.eE+]+")
        mask = lambda v: tps.sub(b"ticks_per_sec:_", pat.sub(b"save_version:_", v, count=1))
        masked = []
        for m in save_masks:
            rx = re.compile(m.encode())
            nb, nc = len(rx.findall(vb)), len(rx.findall(vc))
            if nb + nc == 0:
                die(f"--save-mask {m!r} matches neither save")
            vb, vc = rx.sub(b"", vb), rx.sub(b"", vc)
            masked.append(f"{m!r} x{nb} base, x{nc} cand")
        same = mask(vb) == mask(vc)
        ok &= same
        ver = f"save_version {mb.group(1).decode() if mb else '?'} -> {mc.group(1).decode() if mc else '?'}"
        lines.append(f"save {len(vb):,} B base, {len(vc):,} B cand: {'identical' if same else 'DIFFER'} "
                     f"({ver} and ticks_per_sec masked" + (f"; masked {', '.join(masked)}" if masked else "") + ")")
    return ok, lines


# ---------------------------------------------------------------------------------------------- counters

def summarise(rundir: Path, cols, kinds):
    rows = analyze_run.load_csv(rundir / "run.csv")
    events = analyze_run.load_events(rundir / "events.tsv")
    o = analyze_run.analyze(rows, events)
    j = o.get("jobs2", {})
    ev = {}
    for _, k, _ in events:
        ev[k] = ev.get(k, 0) + 1
    tot = lambda c: sum(analyze_run.col(rows, c)) if rows and c in rows[0] else None
    last = lambda c: rows[-1].get(c) if rows and c in rows[0] else None
    s = {"Murders": ev.get("Murder", 0),
         "J24 employed": j.get("employed_share_from30"),
         "J25 homeless": j.get("homeless_share_from30"),
         "arrests": tot("arrests"),
         "wallets (last day)": last("wallets"),
         "ticks/s (print only)": statistics.mean(analyze_run.col(rows, "ticks_per_sec")) if rows and "ticks_per_sec" in rows[0] else None}
    for c in cols:
        s[c] = tot(c)
    for k in kinds:
        s[f"ev {k}"] = ev.get(k, 0)
    return s, (set(rows[0]) if rows else set())


def fmt(v):
    if v is None:
        return "-"
    if isinstance(v, float) and not v.is_integer():
        return f"{v:.3f}" if abs(v) < 10 else f"{v:,.0f}"
    return f"{int(v):,}"


def rel(a, b):
    if a is None or b is None:
        return ""
    if a == 0:
        return "new" if b else ""
    return f"{(b - a) / abs(a):+.0%}"


# ---------------------------------------------------------------------------------------------- lives

def parse_lives(path: Path):
    out = {}
    for line in path.read_text(errors="replace").splitlines():
        m = re.match(r"PROBE lives (\S+) (\S+): (.*)", line)
        if not m:
            continue
        row = {}
        for name, rx in LIVES_FIELDS:
            f = re.search(rx, m.group(3))
            row[name] = float(f.group(1)) if f else None
        out[m.group(2)] = row
    return out


# ---------------------------------------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="main", help="base commit (default main); resolved to a sha")
    ap.add_argument("--base-off", action="store_true", help="run the base with the --off variables set too: for a fix to a switch that already exists on the base (off vs off); default: the base runs as shipped (the phase that adds the switch)")
    ap.add_argument("--off-base", help="base for the off identity when it differs from --base (a switch spanning sub-phases, e.g. phase 4b against main before 4a); default --base")
    ap.add_argument("--off", action="append", default=[], metavar="VAR",
                    help="env var set to 1 for the off identity (repeatable); required unless --skip off")
    ap.add_argument("--identity", action="store_true",
                    help="the off identity with no variable set: a retirement or refactor that must not move the city (no --off)")
    ap.add_argument("--save-mask", action="append", default=[], metavar="REGEX",
                    help="blank this regex in both saves before the identity compare (a known save-format change; repeatable)")
    ap.add_argument("--arch", default="", help="comma list of shadow --lives archetypes to show")
    ap.add_argument("--cols", default="", help="comma list of report columns summed over the run")
    ap.add_argument("--events", default="", help="comma list of event kinds counted")
    ap.add_argument("--seeds", default="42,43,44")
    ap.add_argument("--days", type=int, default=120)
    ap.add_argument("--off-seeds", default="42")
    ap.add_argument("--off-days", type=int, default=30)
    ap.add_argument("--skip", default="", help="comma list of off,counters,lives")
    ap.add_argument("--jobs", type=int, default=6, help="parallel sim processes")
    ap.add_argument("--out", help="output dir (default target/measure/cand-<HEAD short sha>)")
    ap.add_argument("--no-build", action="store_true", help="use target/release/citysim-cli as is")
    a = ap.parse_args()

    split = lambda s: [x.strip() for x in s.split(",") if x.strip()]
    skip = set(split(a.skip))
    if skip - {"off", "counters", "lives"}:
        die(f"--skip takes off,counters,lives; got {sorted(skip)}")
    if "off" not in skip and not a.off and not a.identity:
        die("--off VAR is required for the off identity (or --identity, or --skip off)")
    if a.identity and a.off:
        die("--identity runs with no switch; drop --off")
    for v in a.off:
        if not re.fullmatch(r"CITYSIM_[A-Z0-9_]+_OFF", v):
            die(f"--off {v}: expected a CITYSIM_*_OFF variable")
    seeds, off_seeds = [int(s) for s in split(a.seeds)], [int(s) for s in split(a.off_seeds)]
    if a.days < 31:
        die("--days must exceed 30 (J24/J25 read from day 30)")
    arch, cols, kinds = split(a.arch), split(a.cols), split(a.events)

    repo = Path.cwd()
    if not (repo / "assets" / "config.toml").exists():
        die("run from the repo root (assets/config.toml)")
    sha = git("rev-parse", "--verify", f"{a.base}^{{commit}}")
    head = git("rev-parse", "--short", "HEAD")
    dirty = bool(git("status", "--porcelain", "--untracked-files=no"))
    common = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir")).parent
    broot = common / "target" / "measure" / f"base-{sha[:12]}"
    out = Path(a.out) if a.out else repo / "target" / "measure" / f"cand-{head}{'-dirty' if dirty else ''}"
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    t0 = time.time()

    # Builds: the base (cached) and the candidate, the candidate's binary copied so a rebuild mid-run can't swap it.
    bsrc, bexe = base_tree(sha, broot)
    osha = git("rev-parse", "--verify", f"{a.off_base}^{{commit}}") if a.off_base else sha
    oroot = common / "target" / "measure" / f"base-{osha[:12]}"
    osrc, oexe = base_tree(osha, oroot) if osha != sha else (bsrc, bexe)
    if a.no_build:
        cexe_built = repo / "target" / "release" / f"citysim-cli{EXE}"
        if not cexe_built.exists():
            die(f"--no-build but no {cexe_built}")
    else:
        log("building candidate")
        cexe_built = build(repo, Path(os.environ.get("CARGO_TARGET_DIR", repo / "target")), out / "cargo.log")
    cexe = out / f"citysim-cli-cand{EXE}"
    shutil.copy2(cexe_built, cexe)

    # Every sim in one pool: base runs land in the sha cache, candidate runs under out/.
    off_env = {v: "1" for v in a.off}
    jobs = {}
    with cf.ThreadPoolExecutor(max_workers=max(1, a.jobs)) as pool:
        if "lives" not in skip:
            jobs["lives base"] = pool.submit(run_lives, bexe, bsrc, broot / "lives.txt")
            jobs["lives cand"] = pool.submit(run_lives, cexe, repo, out / "lives.txt")
        if "off" not in skip:
            for s in off_seeds:
                jobs[f"off base {s}"] = pool.submit(run_city, oexe, osrc, s, a.off_days, oroot / (f"offoff_{s}x{a.off_days}" if a.base_off else f"off_{s}x{a.off_days}"), off_env if a.base_off else None, True)
                jobs[f"off cand {s}"] = pool.submit(run_city, cexe, repo, s, a.off_days, out / f"off_{s}x{a.off_days}", off_env, True)
        if "counters" not in skip:
            for s in seeds:
                jobs[f"run base {s}"] = pool.submit(run_city, bexe, bsrc, s, a.days, broot / f"run_{s}x{a.days}")
                jobs[f"run cand {s}"] = pool.submit(run_city, cexe, repo, s, a.days, out / f"run_{s}x{a.days}")
        log(f"{len(jobs)} jobs queued, {a.jobs} at a time (base runs cached under {broot})")
        names = {fut: name for name, fut in jobs.items()}
        for fut in cf.as_completed(names):
            fut.result()
            log(f"done: {names[fut]}")

    md = [f"# Phase measurement: {head}{' (dirty)' if dirty else ''} against base {sha[:8]}", ""]
    status = 0

    if "off" not in skip:
        md += [f"## Off identity ({', '.join(a.off) + '=1' if a.off else 'no switch'} against {osha[:8]}{' with the same switches off' if a.base_off else ''}; seeds {a.off_seeds} x {a.off_days} d)", ""]
        all_ok = True
        for s in off_seeds:
            ok, lines = off_compare(oroot / (f"offoff_{s}x{a.off_days}" if a.base_off else f"off_{s}x{a.off_days}"), out / f"off_{s}x{a.off_days}", a.save_mask)
            all_ok &= ok
            md += [f"- seed {s}: **{'IDENTICAL' if ok else 'DIFFER'}**"] + [f"  - {l}" for l in lines]
        md += ["", f"OFF IDENTITY: {'IDENTICAL' if all_ok else 'DIFFER'}", ""]
        if not all_ok:
            status = 2

    if "counters" not in skip:
        per = {}
        cand_cols = set()
        for s in seeds:
            per[("base", s)], _ = summarise(broot / f"run_{s}x{a.days}", cols, kinds)
            per[("cand", s)], cc = summarise(out / f"run_{s}x{a.days}", cols, kinds)
            cand_cols |= cc
        missing = [c for c in cols if c not in cand_cols]
        if missing:
            die(f"--cols not in the candidate's report: {missing}")
        keys = list(per[("base", seeds[0])])
        md += [f"## Counters (seeds {a.seeds} x {a.days} d, switches on; base -> candidate)", "",
               "| counter | " + " | ".join(f"seed {s}" for s in seeds) + " | mean | |",
               "|---|" + "---|" * (len(seeds) + 2)]
        for k in keys:
            cells, bs, cs = [], [], []
            for s in seeds:
                b, c = per[("base", s)][k], per[("cand", s)][k]
                cells.append(f"{fmt(b)} -> {fmt(c)}")
                if b is not None:
                    bs.append(b)
                if c is not None:
                    cs.append(c)
            mb = statistics.mean(bs) if len(bs) == len(seeds) else None
            mc = statistics.mean(cs) if len(cs) == len(seeds) else None
            md.append(f"| {k} | " + " | ".join(cells) + f" | {fmt(mb)} -> {fmt(mc)} | {rel(mb, mc)} |")
        md.append("")

    if "lives" not in skip:
        lb, lc = parse_lives(broot / "lives.txt"), parse_lives(out / "lives.txt")
        if not lc:
            die(f"no PROBE lives lines in {out / 'lives.txt'}")
        unknown = [x for x in arch if x not in lb and x not in lc]
        if unknown:
            die(f"--arch not in either probe: {unknown}; known: {sorted(set(lb) | set(lc))}")
        md += ["## Device rows (shadow --lives, 2k, 42-44 pooled, day 19, 7 d; base -> candidate)", "",
               "| arch | " + " | ".join(n for n, _ in LIVES_FIELDS) + " |", "|---|" + "---|" * len(LIVES_FIELDS)]
        for x in arch + ["all"]:
            b, c = lb.get(x, {}), lc.get(x, {})
            md.append(f"| {x} | " + " | ".join(f"{fmt(b.get(n))} -> {fmt(c.get(n))}" for n, _ in LIVES_FIELDS) + " |")
        md.append("")

    md.append(f"Wall {time.time() - t0:.0f} s. Logs under {out} and {broot}.")
    text = "\n".join(md) + "\n"
    (out / "measure.md").write_text(text, encoding="utf-8", newline="\n")
    (out / "measure.json").write_text(json.dumps({"base": sha, "head": head, "dirty": dirty, "status": status}),
                                      encoding="utf-8", newline="\n")
    sys.stdout.write(text)
    return status


if __name__ == "__main__":
    sys.exit(main())
