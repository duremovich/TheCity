#!/usr/bin/env python3
"""Summarise a TheCity headless run.

Usage: python tools/analyze_run.py run.csv events.tsv [--days N] [--json] [--jail-cap K]

run.csv    per-day report from `citysim-cli run --report` (stdout). Normally has a header
           line (stats::CSV_HEADER); columns are looked up by name, so new columns are fine.
           If there is no header, the documented 25-column order is assumed.
events.tsv per-event log from `--events` (stderr): `tick<TAB>Kind<TAB>text`. Lines that do
           not start with an integer tick (e.g. "saved ...") are ignored. 1 day = 1440 ticks.
--days N   only look at the first N days.
--jail-cap K  jail capacity for the "stuck at cap" flag (default: the run's max jailed).
"""
import argparse
import csv
import json
import re
import statistics as st
import sys
from collections import Counter, defaultdict

TICKS_PER_DAY = 1440
DEFAULT_COLS = ("day,season,population,employed,homeless,jailed,gang_members,food_market,"
                "food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,"
                "deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,"
                "mean_mood,goal_changes_per_agent,ticks_per_sec").split(",")
STORY = ["OrderChanged", "Raid", "Jailbreak", "Posture", "Bribe", "TerritoryFlipped", "GangJoin",
         "Marriage", "Birth", "Death", "Murder", "Assault", "Evicted", "Founded", "Incorporated",
         "Bankrupt", "Acquired", "Strike", "CorpOrder", "Attributed", "Robbed", "Assaulted"]
TRANSITIONS = ["OrderChanged", "Posture", "CorpOrder"]


def num(v):
    try:
        return float(v)
    except ValueError:
        return v


def load_csv(path):
    with open(path, newline="") as f:
        rows = [r for r in csv.reader(f) if r]
    if rows[0][0].strip().isdigit():
        header = DEFAULT_COLS[:len(rows[0])]
    else:
        header, rows = rows[0], rows[1:]
    return [{h: num(v) for h, v in zip(header, r)} for r in rows]


def load_events(path):
    ev = []
    with open(path, errors="replace") as f:
        for line in f:
            p = line.rstrip("\n").split("\t", 2)
            if len(p) >= 2 and p[0].isdigit():
                ev.append((int(p[0]), p[1], p[2] if len(p) > 2 else ""))
    return ev


def col(rows, name):
    return [r[name] for r in rows if isinstance(r.get(name), float)]


def mean(xs):
    return st.mean(xs) if xs else float("nan")


def stat(xs):
    return {"min": min(xs), "mean": mean(xs), "max": max(xs)} if xs else {}


def longest_run(xs, target):
    best = cur = 0
    for x in xs:
        cur = cur + 1 if x == target else 0
        best = max(best, cur)
    return best


def analyze(rows, events, jail_cap=None):
    flags = []
    n = len(rows)
    days = [int(r["day"]) for r in rows]
    last_day = max(days) if days else 0
    total = lambda k: sum(col(rows, k))
    out = {"days": n}

    pop = col(rows, "population")
    if pop:
        out["population"] = {
            "start": pop[0], "end": pop[-1], "min": min(pop), "max": max(pop),
            "births": total("births"), "deaths_starvation": total("deaths_starvation"),
            "deaths_old_age": total("deaths_old_age"), "deaths_violence": total("deaths_violence"),
            "burials": total("burials"), "homeless_max": max(col(rows, "homeless") or [0]),
            "employed_mean": mean(col(rows, "employed"))}
        for i in range(1, len(pop)):
            if pop[i - 1] and (pop[i - 1] - pop[i]) / pop[i - 1] > 0.05:
                flags.append(f"day {days[i]}: population {pop[i-1]:.0f} -> {pop[i]:.0f} (drop > 5%)")

    market, treas = col(rows, "food_market"), col(rows, "treasury")
    eco = {"price": stat(col(rows, "price")), "market_zero_days": sum(1 for m in market if m == 0)}
    if treas:
        eco["treasury"] = {"start": treas[0], "end": treas[-1], "min": min(treas)}
    for k in ("food_warehouse", "food_pantry"):
        c = col(rows, k)
        if c:
            eco[k] = {"first": c[0], "last": c[-1]}
    if rows and "season" in rows[0]:
        eco["winter_days"] = sum(1 for r in rows if r["season"] == "Winter")
    out["economy"] = eco

    for r in rows:
        d = int(r["day"])
        if isinstance(r.get("price"), float) and not 2 <= r["price"] <= 8:
            flags.append(f"day {d}: price {r['price']:g} outside 2..8")
        if isinstance(r.get("mean_hunger"), float) and r["mean_hunger"] < 0.4:
            flags.append(f"day {d}: mean hunger {r['mean_hunger']:.3f} < 0.4")
        if isinstance(r.get("ticks_per_sec"), float) and r["ticks_per_sec"] < 8000:
            flags.append(f"day {d}: tps {r['ticks_per_sec']:.0f} < 8000")

    kinds = Counter(k for _, k, _ in events)
    jailed = col(rows, "jailed")
    out["law"] = {
        "thefts": total("thefts"), "arrests": total("arrests"), "jailed_mean": mean(jailed),
        "jailed_max": max(jailed or [0]),
        "assaults_murders_per_day": (kinds["Assault"] + kinds["Murder"]) / max(n, 1),
        "starvation_deaths_per_week": total("deaths_starvation") / max(n / 7, 1)}
    cap = jail_cap or (max(jailed) if jailed else 0)
    if cap > 0 and longest_run(jailed, cap) > 10:
        flags.append(f"jailed at cap ({cap:g}) for {longest_run(jailed, cap)} consecutive days")

    mood = {}
    for k in ("mean_hunger", "mean_mood"):
        pairs = [(r[k], int(r["day"])) for r in rows if isinstance(r.get(k), float)]
        if pairs:
            worst = min(pairs)
            mood[k] = {"mean": mean([p[0] for p in pairs]),
                       "last30_mean": mean([p[0] for p in pairs[-30:]]),
                       "worst": worst[0], "worst_day": worst[1]}
    out["mood"] = mood
    tps = col(rows, "ticks_per_sec")
    if tps:
        out["throughput"] = {"tps_mean": mean(tps), "tps_min": min(tps)}

    out["event_counts"] = dict(kinds.most_common())
    weekly = defaultdict(Counter)
    span = {}
    trans = defaultdict(Counter)
    for tick, kind, text in events:
        d = tick // TICKS_PER_DAY
        weekly[kind][d // 7] += 1
        span[kind] = (span.get(kind, (d,))[0], d)
        if kind in TRANSITIONS:
            m = re.search(r"(\w+)\s*->\s*(\w+)", text)
            if m:
                trans[kind][f"{m.group(1)} -> {m.group(2)}"] += 1
    nweeks = last_day // 7 + 1
    out["weekly"] = {k: [weekly[k][w] for w in range(nweeks)] for k in STORY if k in kinds}
    out["transitions"] = {k: dict(v.most_common()) for k, v in trans.items()}
    for k, (_, last) in span.items():
        if last_day - last >= 30:
            flags.append(f"no {k} events since day {last} ({kinds[k]} before)")
    out["flags"] = flags
    return out


def fmt(o):
    f = lambda d: ", ".join(f"{k}={v:.4g}" if isinstance(v, float) else f"{k}={v}" for k, v in d.items())
    e = o["economy"]
    L = [f"== run: {o['days']} days =="]
    if "population" in o:
        L += ["-- population --", f(o["population"])]
    L += ["-- economy --", "price: " + f(e["price"]), f"market empty days: {e['market_zero_days']}"]
    L += [f"{k}: " + f(e[k]) for k in ("treasury", "food_warehouse", "food_pantry") if k in e]
    if "winter_days" in e:
        L.append(f"winter days: {e['winter_days']}")
    L += ["-- law --", f(o["law"]), "-- mood / hunger --"]
    L += [f"{k}: " + f(v) for k, v in o["mood"].items()]
    if "throughput" in o:
        L += ["-- throughput --", f(o["throughput"])]
    L.append("-- events (count) --")
    L += [f"{k:<18}{v}" for k, v in o["event_counts"].items()]
    L.append("-- weekly histograms (7-day buckets) --")
    L += [f"{k:<18}{' '.join(map(str, v))}" for k, v in o["weekly"].items()]
    for k, v in o["transitions"].items():
        L.append(f"-- {k} transitions --")
        L += [f"  {c:>5}  {t}" for t, c in v.items()]
    L.append("-- flags --")
    L += [f"  {x}" for x in o["flags"]] or ["  none"]
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv")
    ap.add_argument("events")
    ap.add_argument("--days", type=int)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--jail-cap", type=float)
    a = ap.parse_args()
    rows, events = load_csv(a.csv), load_events(a.events)
    if a.days:
        rows = rows[:a.days]
        events = [e for e in events if e[0] // TICKS_PER_DAY < a.days]
    o = analyze(rows, events, a.jail_cap)
    print(fmt(o))
    if a.json:
        print(json.dumps(o, indent=1))


if __name__ == "__main__":
    sys.exit(main())
