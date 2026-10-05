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

M11: an "ownership" section (evictions, rent, foundings, incorporations, acquisitions,
bankruptcies, strikes, monopolies, class unrest, each corp slot's treasury and order days),
CorpOrder transitions per corp, and flags for a corp in the red 3+ days running, any
monopoly, Street unrest outside 0.2-0.7 and no Dregs for 30+ days running.
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
         "Bankrupt", "Acquired", "Strike", "CorpOrder", "Contract", "Housed", "Attributed", "Robbed",
         "Assaulted"]
TRANSITIONS = ["OrderChanged", "Posture", "CorpOrder"]


def num(v):
    try:
        return float(v)
    except ValueError:
        return v


def load_csv(path):
    with open(path, newline="") as f:
        rows = [r for r in csv.reader(f) if r]
    if not rows:
        return []
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
        slow = [t for t in tps if t < 8000]
        if slow:
            flags.append(f"tps < 8000 on {len(slow)} of {len(tps)} days (min {min(slow):.0f})")

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
    # Only recurring story kinds can go stale; one-offs like Founded never repeat.
    one_offs = {"Founded", "Incorporated"}
    for k, (_, last) in span.items():
        if k in STORY and k not in one_offs and last_day - last >= 30:
            flags.append(f"no {k} events since day {last} ({kinds[k]} before)")
    ownership(rows, events, out, flags)
    out["flags"] = flags
    return out


def runs(days, pred):
    """Maximal runs of consecutive rows where pred holds: [(first_day, last_day, length)]."""
    out, start, prev = [], None, None
    for d, ok in days:
        if ok and start is None:
            start = d
        if not ok and start is not None:
            out.append((start, prev, prev - start + 1))
            start = None
        prev = d
    if start is not None:
        out.append((start, prev, prev - start + 1))
    return out


def ownership(rows, events, out, flags):
    """M11: the money and class story of a run, from the D38 columns and the events."""
    if not rows or "evictions" not in rows[0]:
        return
    total = lambda k: sum(col(rows, k))
    kinds = Counter(k for _, k, _ in events)
    o = {k: total(k) for k in ("evictions", "rent_paid", "rent_short", "housed", "foundings",
                               "incorporations", "acquisitions", "bankruptcies", "strikes")
         if k in rows[0]}
    o["registered"] = sum(1 for _, k, t in events if k == "Founded" and " registered " in t)
    o["corp_builds"] = sum(1 for _, k, t in events if k == "Founded" and " built " in t)
    o["hostile"] = sum(1 for _, k, t in events if k == "Acquired" and "(hostile)" in t)
    o["contracts_events"] = kinds["Contract"]
    mono = col(rows, "monopolies")
    o["monopoly_days"] = sum(1 for m in mono if m > 0)
    for c in ("unrest_corp", "unrest_street", "unrest_dreg", "class_dreg", "happiness_street"):
        if c in rows[0]:
            o[c] = stat(col(rows, c))
    ledger = [k for k in rows[0] if k.startswith("flow_")]
    o["ledger_per_day"] = {k[5:]: total(k) / max(len(rows), 1) for k in ledger}
    out["ownership"] = o

    days = [int(r["day"]) for r in rows]
    corps = {}
    for i in range(1, 9):
        t, od = f"corp{i}_treasury", f"corp{i}_order"
        if t not in rows[0]:
            continue
        orders = Counter(r[od] for r in rows if r.get(od) not in ("-", None, ""))
        tre = [r[t] for r in rows if isinstance(r.get(t), float)]
        alive = [r for r in rows if r.get(od) not in ("-", None, "")]
        corps[f"corp{i}"] = {"start": tre[0] if tre else 0, "end": tre[-1] if tre else 0,
                             "min": min(tre) if tre else 0, "alive_days": len(alive),
                             "order_days": dict(orders.most_common())}
        for a, b, length in runs(zip(days, (r.get(od) not in ("-", None, "") and isinstance(r.get(t), float)
                                              and r[t] < 0 for r in rows)), bool):
            if length >= 3:
                flags.append(f"corp{i} in the red days {a}-{b} ({length} days)")
    out["corps"] = corps

    for d, m in zip(days, (r.get("monopolies") for r in rows)):
        if isinstance(m, float) and m > 0:
            flags.append(f"day {d}: monopoly ({m:g} niche(s)); first only")
            break
    if "unrest_street" in rows[0]:
        bad = [(d, r["unrest_street"]) for d, r in zip(days, rows)
               if isinstance(r.get("unrest_street"), float) and not 0.2 <= r["unrest_street"] <= 0.7]
        if bad:
            flags.append(f"Street unrest outside 0.2-0.7 on {len(bad)} days (first day {bad[0][0]}: "
                         f"{bad[0][1]:.2f})")
    if "class_dreg" in rows[0]:
        for a, b, length in runs(zip(days, (r.get("class_dreg") == 0 for r in rows)), bool):
            if length >= 30:
                flags.append(f"no Dregs days {a}-{b} ({length} days)")

    # CorpOrder transitions per corp: "Name: Old [in N] -> New in N (why, ...)".
    per = defaultdict(Counter)
    for _, kind, text in events:
        if kind != "CorpOrder":
            continue
        m = re.match(r"(.+?): (\w+(?: in \w+)?) -> (\w+(?: in \w+)?)", text)
        if m:
            per[m.group(1)][f"{m.group(2)} -> {m.group(3)}"] += 1
    out["corp_transitions"] = {c: dict(v.most_common()) for c, v in sorted(per.items())}


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
    if "ownership" in o:
        w = o["ownership"]
        L.append("-- ownership (M11) --")
        L.append(f({k: v for k, v in w.items() if not isinstance(v, dict)}))
        L += [f"{k}: " + f(v) for k, v in w.items() if isinstance(v, dict) and v]
        L.append("-- corp slots (treasury start/end/min, alive days, days per order) --")
        for k, v in o.get("corps", {}).items():
            orders = " ".join(f"{a}:{b}" for a, b in v["order_days"].items())
            L.append(f"  {k}: {v['start']:.0f} -> {v['end']:.0f} (min {v['min']:.0f}), "
                     f"{v['alive_days']} days, {orders}")
        L.append("-- CorpOrder transitions per corp --")
        for c, v in o.get("corp_transitions", {}).items():
            L.append(f"  {c}: " + ", ".join(f"{t} x{n}" for t, n in v.items()))
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
