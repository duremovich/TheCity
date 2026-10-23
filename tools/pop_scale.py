#!/usr/bin/env python3
"""The population scaling bench (OPEN-184, docs/POPULATION_STUDY.md).

Runs `citysim-cli run --population N --scaled --map MAP --report --mem` for each
size, interleaved across repetitions (size order 20k, 40k, ... then again), so a
shared box's noise spreads over every point; each run's numbers land in one JSON
line under --out, and the summary prints medians over the repetitions.

  python tools/pop_scale.py --bin target/release/citysim-cli.exe \
      --pop 20000,40000,60000,80000 --days 4 --reps 2 --out <dir> [--save] [--prof]

Per run: ticks/s per day (the steady days are day >= --steady-from), wall per
game day, peak working set (`MEM`), world build (the wall before day 1, i.e.
process wall less the days' own seconds), the save's size at the end tick
(`--save`; the file is deleted after it is measured), the last day's tiers and
headline counters. With --prof (a `--features prof` binary) the PROF lines are
kept in the run's stderr log for `--prof-table`.

  python tools/pop_scale.py --prof-table <dir> --days D --prof-from F   # PROF sites x size, ms/day

A map per size (the committed isle_m_k2 houses only 31,645):
  citysim-cli import-graphics --src D:/TheCityGraphics/assets/city/isle_m/out --out <dir>/isle_N.txt \
      --downsample 2 --residents N --lines assets/maps/isle_m_k2_lines.txt
then --map "<dir>/isle_{n}.txt".

Must run from the repo root (the CLI reads assets/config.toml there). Logs stay
on disk: <out>/<label>_<pop>_r<k>.csv / .err.
"""

import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

TICKS_PER_DAY = 1440
COLS = ["population", "employed", "homeless", "housed", "tier_full", "tier_coarse", "tier_stat",
        "ambient_households", "ambient_adults", "ambient_materialised"]


def run_one(args, exe, label, pop, rep, out):
    stem = out / f"{label}_{pop}_r{rep}"
    saves = out / f"saves_{label}_{pop}_r{rep}"
    cmd = [exe, "run", "--days", str(args.days), "--seed", str(args.seed), "--population", str(pop),
           "--scaled", "--map", args.map.replace("{n}", str(pop)), "--report", "--mem"] + args.extra.split()
    end_tick = args.days * TICKS_PER_DAY
    if args.save:
        cmd += ["--save-at", str(end_tick), "--saves-dir", str(saves)]
    if args.prof_from is not None:
        cmd += ["--prof-from", str(args.prof_from)]
    t0 = time.perf_counter()
    with open(f"{stem}.csv", "w", newline="\n") as fo, open(f"{stem}.err", "w", newline="\n") as fe:
        rc = subprocess.run(cmd, stdout=fo, stderr=fe).returncode
    wall = time.perf_counter() - t0
    rec = {"label": label, "pop": pop, "rep": rep, "rc": rc, "wall_s": round(wall, 1), "cmd": " ".join(cmd)}
    rows = []
    with open(f"{stem}.csv") as f:
        lines = [l.rstrip("\n") for l in f if l.strip()]
    if lines:
        head = lines[0].split(",")
        for l in lines[1:]:
            rows.append(dict(zip(head, l.split(","))))
    tps = [float(r["ticks_per_sec"]) for r in rows if r.get("ticks_per_sec")]
    rec["tps_days"] = [round(t, 1) for t in tps]
    steady = tps[args.steady_from - 1:] or tps
    rec["tps_med"] = round(statistics.median(steady), 1) if steady else None
    days_s = sum(TICKS_PER_DAY / t for t in tps if t > 0)
    rec["wall_per_day_s"] = round(TICKS_PER_DAY / rec["tps_med"], 1) if rec["tps_med"] else None
    rec["build_s"] = round(wall - days_s, 1)
    peak = 0.0
    with open(f"{stem}.err", errors="replace") as f:
        for l in f:
            if l.startswith("MEM"):
                parts = l.split()
                peak = max(peak, float(parts[2]))
            m = re.match(r"PROF world_new ([0-9.]+)s", l)
            if m:
                rec["world_new_s"] = float(m.group(1))
            if "panicked" in l or l.startswith("error"):
                rec.setdefault("errors", []).append(l.strip()[:300])
    rec["peak_mb"] = round(peak)
    if rows:
        last = rows[-1]
        rec["last"] = {c: last.get(c) for c in COLS if c in last}
    if args.save and saves.exists():
        files = list(saves.glob("*"))
        rec["save_mb"] = round(sum(p.stat().st_size for p in files) / 2**20, 1)
        for p in files:
            p.unlink()
        saves.rmdir()
    return rec


def summarise(recs):
    by = {}
    for r in recs:
        by.setdefault((r["label"], r["pop"]), []).append(r)
    print("label pop reps tps_med(runs) wall/day_s build_s peak_MB save_MB full/coarse/stat homeless employed")
    for (label, pop), rs in sorted(by.items(), key=lambda kv: (kv[0][0], kv[0][1])):
        med = lambda k: statistics.median([r[k] for r in rs if r.get(k) is not None]) if any(
            r.get(k) is not None for r in rs) else None
        last = rs[-1].get("last", {})
        tiers = "/".join(str(last.get(c, "?")) for c in ("tier_full", "tier_coarse", "tier_stat"))
        print(f"{label} {pop} {len(rs)} {med('tps_med')} ({','.join(str(r.get('tps_med')) for r in rs)}) "
              f"{med('wall_per_day_s')} {med('build_s')} {med('peak_mb')} {med('save_mb')} {tiers} "
              f"{last.get('homeless')} {last.get('employed')}")


def prof_table(out, days):
    """PROF lines across runs: site -> {pop: [ms per profiled game day, one per rep]}."""
    data = {}
    for err in sorted(Path(out).glob("*.err")):
        m = re.match(r"(.+)_(\d+)_r(\d+)\.err", err.name)
        if not m:
            continue
        sites = {}
        with open(err, errors="replace") as f:
            for l in f:
                # `PROF <secs>s <share>% <calls> <name>` (prof.rs::dump)
                pm = re.match(r"PROF\s+([0-9.]+)s\s+[0-9.]+%\s+(\d+)\s+(\S+)", l)
                if pm:
                    sites[pm.group(3)] = (float(pm.group(1)) * 1000.0 / days, int(pm.group(2)))
        if sites:
            data.setdefault(int(m.group(2)), []).append(sites)
    return data


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", action="append", help="LABEL=EXE (repeat for ABAB); a bare EXE is labelled 'run'")
    ap.add_argument("--pop", default="20000,40000,60000,80000")
    ap.add_argument("--days", type=int, default=4)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--map", default="assets/maps/isle_m_k2.txt",
                    help="the map; '{n}' is replaced by the population (a re-import per size)")
    ap.add_argument("--extra", default="--ambient-flats 50", help="more run flags")
    ap.add_argument("--reps", type=int, default=2)
    ap.add_argument("--steady-from", type=int, default=2, help="first steady day (1-based)")
    ap.add_argument("--prof-from", type=int, default=None)
    ap.add_argument("--save", action="store_true", help="save at the end tick, measure and delete it")
    ap.add_argument("--out", required=False)
    ap.add_argument("--summary", metavar="DIR", help="print the summary of DIR/runs.jsonl and exit")
    ap.add_argument("--prof-table", metavar="DIR", help="print the PROF table of DIR and exit")
    args = ap.parse_args()

    if args.summary:
        with open(Path(args.summary) / "runs.jsonl") as f:
            summarise([json.loads(l) for l in f if l.strip()])
        return
    if args.prof_table:
        days = args.days - (args.prof_from or 0)
        data = prof_table(args.prof_table, days)
        pops = sorted(data)
        names = sorted({n for p in pops for s in data[p] for n in s})
        med = {p: {n: statistics.median([s[n][0] for s in data[p] if n in s] or [0.0]) for n in names} for p in pops}
        top = sorted(names, key=lambda n: -med[pops[-1]].get(n, 0.0))
        print("site ms/day: " + " ".join(str(p) for p in pops))
        for n in top:
            print(n + " " + " ".join(f"{med[p][n]:.0f}" for p in pops))
        return

    if not Path("assets/config.toml").exists():
        sys.exit("run from the repo root (assets/config.toml)")
    if not args.bin or not args.out:
        sys.exit("--bin and --out are required")
    bins = []
    for b in args.bin:
        label, _, exe = b.rpartition("=")
        bins.append((label or "run", exe))
    for _, exe in bins:
        if not Path(exe).exists():
            sys.exit(f"no binary {exe}")
    pops = [int(p) for p in args.pop.split(",")]
    if args.steady_from > args.days:
        sys.exit(f"--steady-from {args.steady_from} is past --days {args.days}")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    recs = []
    for rep in range(1, args.reps + 1):
        for pop in pops:
            for label, exe in bins:
                rec = run_one(args, exe, label, pop, rep, out)
                recs.append(rec)
                with open(out / "runs.jsonl", "a", newline="\n") as f:
                    f.write(json.dumps(rec) + "\n")
                print(f"{label} {pop} r{rep}: rc {rec['rc']} tps {rec['tps_med']} days {rec['tps_days']} "
                      f"build {rec['build_s']}s peak {rec['peak_mb']} MB save {rec.get('save_mb')} MB",
                      flush=True)
                if rec["rc"] != 0:
                    print(f"  FAIL {rec.get('errors', [])[:3]}", flush=True)
    summarise(recs)


if __name__ == "__main__":
    main()
