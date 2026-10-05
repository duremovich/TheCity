#!/usr/bin/env python3
"""Summarise a TheCity headless run.

Usage: python tools/analyze_run.py run.csv [events.tsv] [--days N] [--json] [--jail-cap K]

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

M12: a "districts" section (per-district means of coverage, unrest, litter, crime and guards
from the d1..d8 columns, days per controller, riot/crossfire/vagrancy/hotel-night totals, Dregs
as a share of adults) and, with an events file, riots, strikes and splits by district and corp
raids won/lost. Flags: a district above unrest 0.8 for 30+ days running with no riot there,
district litter above 0.5, no Dregs (`dregs`) for 30+ days running, corp raids never lost.
The events file is optional; without it the event-based parts are skipped.
"""
import argparse
import csv
import json
import os
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
DISTRICTS = ["Spire", "Civic", "Vats", "Mid West", "Mid East", "Sump West", "Sump Central", "Sump East"]
CONTROLLERS = {0: "Contested", 1: "City", 2: "Gang", 3: "Corp"}


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
    districts(rows, events, out, flags)
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


def district_names():
    """`[districts] names` from assets/config.toml next to this script, else the default."""
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "config.toml")
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        return list(DISTRICTS)
    sec = re.search(r"^\[districts\][ \t]*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    m = sec and re.search(r"^names\s*=\s*\[(.*?)\]", sec.group(1), re.M | re.S)
    names = re.findall(r'"([^"]*)"', m.group(1)) if m else []
    return names or list(DISTRICTS)


def day_ranges(ds):
    """[3, 4, 5, 9] -> "3-5, 9"."""
    ds = sorted(set(ds))
    if not ds:
        return ""
    have = set(ds)
    spans = runs(((d, d in have) for d in range(ds[0], ds[-1] + 1)), bool)
    return ", ".join(f"{a}-{b}" if a != b else f"{a}" for a, b, _ in spans)


def districts(rows, events, out, flags):
    """M12: the district story of a run, from the D45 columns and (if given) the events."""
    if not rows or "d1_control" not in rows[0]:
        return
    names = district_names()
    days = [int(r["day"]) for r in rows]
    total = lambda k: sum(col(rows, k))

    # Events: riots (start "{D} is rising: ...", finish "{D} rioted at T: won|lost (...)" or
    # "{D}'s riot against T fizzled|dispersed: ..."), strikes ("... in {D} walk out"), splits
    # ("... Blocks in {D}...: the X"), corp raids ("G raided Owner's B: Won|Lost (... food taken)").
    riot_days, riots_ev, strikes, splits = defaultdict(set), defaultdict(Counter), Counter(), []
    raids = Counter()
    if events:
        alt = "|".join(re.escape(n) for n in sorted(names, key=len, reverse=True))
        riot_re = re.compile(rf"({alt})(?:( is rising):|'s riot against .*? (fizzled|dispersed):| rioted at .*?: (\w+) \()")
        for tick, kind, text in events:
            d = tick // TICKS_PER_DAY
            if kind == "Riot":
                m = riot_re.match(text)
                if m:
                    riot_days[m.group(1)].add(d)
                    riots_ev[m.group(1)]["started" if m.group(2) else (m.group(3) or m.group(4))] += 1
            elif kind == "Strike":
                m = re.search(rf" in ({alt}) walk out", text)
                strikes[m.group(1) if m else "?"] += 1
            elif kind == "Split":
                m = re.search(rf" Blocks in ({alt})", text)
                splits.append((d, m.group(1) if m else "?", text))
            elif kind == "Raid":
                m = re.match(r".+? raided (.+?)'s .+?: (Won|Lost) \(.* food taken\)$", text)
                if m and m.group(1) != "the city":
                    raids[m.group(2)] += 1
    city_riot_days = {d for d, r in zip(days, rows) if isinstance(r.get("riots"), float) and r["riots"] > 0}

    per = {}
    for i in range(8):
        p = f"d{i + 1}_"
        if p + "control" not in rows[0]:
            continue
        name = names[i] if i < len(names) else f"d{i + 1}"
        ctrl = Counter(CONTROLLERS.get(int(r[p + "control"]), str(r[p + "control"])) for r in rows
                       if isinstance(r.get(p + "control"), float))
        v = {k: mean(col(rows, p + k)) for k in ("coverage", "unrest", "litter", "crime", "guards")}
        v["control_days"] = {c: ctrl[c] for c in CONTROLLERS.values() if ctrl[c]}
        if events:
            v["riots"] = dict(riots_ev[name])
            v["strikes"] = strikes[name]
            v["splits"] = sum(1 for _, dn, _ in splits if dn == name)
        per[name] = v
        rdays = riot_days[name] if events else city_riot_days
        hot = ((d, isinstance(r.get(p + "unrest"), float) and r[p + "unrest"] > 0.8 and d not in rdays)
               for d, r in zip(days, rows))
        for a, b, length in runs(hot, bool):
            if length >= 30:
                flags.append(f"{name} unrest > 0.8 days {a}-{b} ({length} days) without a riot there")
        dirty = [d for d, r in zip(days, rows) if isinstance(r.get(p + "litter"), float) and r[p + "litter"] > 0.5]
        if dirty:
            flags.append(f"{name} litter > 0.5 on {len(dirty)} days: {day_ranges(dirty)}")

    o = {k: total(k) for k in ("riots", "crossfire", "vagrancy", "hotel_nights") if k in rows[0]}
    for k in ("dregs", "squatters", "derelicts", "gangs"):
        if k in rows[0]:
            o[k + "_mean"] = mean(col(rows, k))
    adults = ("class_corp", "class_street", "class_dreg")
    if "dregs" in rows[0] and all(k in rows[0] for k in adults):
        share = []
        for r in rows:
            a = sum(r[k] for k in adults if isinstance(r.get(k), float))
            if a > 0 and isinstance(r.get("dregs"), float):
                share.append(r["dregs"] / a)
        o["dreg_share_mean"] = mean(share)
        o["dreg_share_1_5pct_days"] = sum(1 for x in share if 0.01 <= x <= 0.05)
    if events:
        o["corp_raids_won"], o["corp_raids_lost"] = raids["Won"], raids["Lost"]
        o["splits"] = len(splits)
        o["strike_events"] = sum(strikes.values())
    out["districts"] = {"city": o, "per_district": per, "splits": [f"day {d}: {t}" for d, _, t in splits]}

    if "dregs" in rows[0]:
        for a, b, length in runs(zip(days, (r.get("dregs") == 0 for r in rows)), bool):
            if length >= 30:
                flags.append(f"no Dregs (dregs == 0) days {a}-{b} ({length} days)")
    if events and raids["Won"] + raids["Lost"] > 0 and raids["Lost"] == 0:
        flags.append(f"corp raids never lost ({raids['Won']} won)")


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
    if "districts" in o:
        dd = o["districts"]
        L.append("-- districts (M12) --")
        L.append(f(dd["city"]))
        L.append(f"  {'district':<14}{'cover':>6}{'unrest':>7}{'litter':>7}{'crime':>7}{'guards':>7}  control days")
        for name, v in dd["per_district"].items():
            ctrl = " ".join(f"{c}:{n}" for c, n in v["control_days"].items())
            L.append(f"  {name:<14}{v['coverage']:>6.2f}{v['unrest']:>7.2f}{v['litter']:>7.3f}"
                     f"{v['crime']:>7.2f}{v['guards']:>7.1f}  {ctrl}")
        if any("riots" in v for v in dd["per_district"].values()):
            L.append("  by district: riots started/won/lost/fizzled/dispersed, strikes, splits")
            for name, v in dd["per_district"].items():
                r = v["riots"]
                if r or v["strikes"] or v["splits"]:
                    rs = "/".join(str(r.get(k, 0)) for k in ("started", "won", "lost", "fizzled", "dispersed"))
                    L.append(f"    {name:<14}riots {rs}, strikes {v['strikes']}, splits {v['splits']}")
        L += [f"  {x}" for x in dd["splits"]]
    L.append("-- flags --")
    L += [f"  {x}" for x in o["flags"]] or ["  none"]
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv")
    ap.add_argument("events", nargs="?")
    ap.add_argument("--days", type=int)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--jail-cap", type=float)
    a = ap.parse_args()
    rows, events = load_csv(a.csv), (load_events(a.events) if a.events else [])
    if a.days:
        rows = rows[:a.days]
        events = [e for e in events if e[0] // TICKS_PER_DAY < a.days]
    o = analyze(rows, events, a.jail_cap)
    print(fmt(o))
    if a.json:
        print(json.dumps(o, indent=1))


if __name__ == "__main__":
    sys.exit(main())
