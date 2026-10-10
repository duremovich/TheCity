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
district litter above 0.5, no Dregs (`class_dreg`) for 30+ days running, corp raids never lost.
The events file is optional; without it the event-based parts are skipped.

M14: a "Virt" section (the plane's snapshots `nodes, labs, decks, cameras, ice_mean_corp, data_held`
on the last day; run totals of `runs, runs_ok` and the outcome columns, `data_made, data_stolen,
data_wiped, data_sold`, `ledger_hacks, doors_hacked, robots_turned, blinded`, `traced, fried, flatlined,
hack_arrests(_chair)`, `ice_raised, ice_lowered, ice_spend`, `tech_gained, tech_lost, research_spent`;
the success share, traced share of lost runs and stolen / made; each corp slot's tiers on the first and
last day; the M14 flows) and, with an events file, the violet event counts and the TechLost / TechGained
/ DataWiped / DoorHacked / RobotTurned / Flatlined lines. Flags: the spec § 12 bands (runs 40-400,
success 30-70 %, fried 3-30, flatlines 1-10, traced 20-50 %, stolen / made 5-30 %, decks 30-150 on the
last day, mean corp ICE 1.0-2.2).

L2 (the living city): an "L2" section from the `LivingCols` and `BudgetCols` columns: wages / dole on day 60
and by 30-day window, the employed share, the Treasury against the budget band (days inside it, the works roster,
`upkeep_mult`), the leisure flows (`flow_leisure`, `flow_gamble`, `flow_gamble_win`, `flow_tribute`, `flow_street_dice`), fun (mean,
satisfied share), venues and visits by kind, HangOuts and their known-contact mean, fronts, Collects, Preaches,
the Parts chain (`fab_parts`, `scrap_parts`, `parts_imported`), faction violence off screen (`fv_*` totals, the
off-screen share of violent deaths, kill rates by tier), the tiers with the held prisoners, and aborts by cause.
Flags: the spec's printed bands (wages / dole on day 60 >= 1.0, employed share 30-40 %, Gini on the last day
0.50-0.70, wallets >= 20k, fun satisfied 40-70 %, the off-screen share of killings 0.3-0.7) and `fv_bound_wrong`
above 0.

M15 (the word and the blood): a "word" section from the `WordCols` columns, replacing the scenario
gate's `print_m15` (2026-10-09): run totals of the counters (`rumours_heard`, `distorted`, `grudges` formed,
`grudges_inherited`, `hunts`, `avenged`, `revenge_kills`, `stories`, `planted`, `buried`, `poached`,
`talent_lost`, `extort_tries` / `extort_success`, `rep_flips`, `contradicted`, `silenced`, `hunts_failed`,
`hunts_abandoned`, `guard_body`, `expelled`, `contracts_lost_honour`), the extortion success share, the
snapshots on the last day and their run max (`hunts_active`, `vendettas_open`, `chain_max`,
`rumour_hops_max`, `second_hand_share`, `known_by_killers_median`, `pool_reach`, `skill_rare_share`,
`law_competence`), each gang slot's dread and heat (first, last, max) and each corp slot's honour,
standing and competence on the last day, and, with an events file, the crimson event counts. No flags:
numbers are reports, not gates (docs/TESTING.md).

M16a (the contract board): a "contracts" section from the `ContractCols` columns, replacing the retired
scenario gate's devices (2026-10-09): posted and fulfilled by kind, closed by status and the fulfilled
share of closed, Hits done (by a squad, solo-weak), strikes declined on political cost, sell-outs,
`Reneged`, contract killings against all Murders (with an events file) and their clearance, `Accessory`
and interrogations, bounties paid per Locate posted, guards on the take, Fixer-fed runs, the Fixer slots'
heat and `FixerCut` income per week, the LOD's live parties and queue (max), escrow held, and the probes
that must read 0 (`escrow_leak`, `escrow_stuck`, `contract_hole_wrong`); with an
events file, Hits taken by render (`LIVE`, `LEDGER`, `QUEUED`) and the amber event counts. The spec's
bands (`docs/M16_CONTRACTS.md`, the M16a printed findings) are printed beside their numbers. One flag: a
probe above 0 (a broken record, not a calibration miss).
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
         "Assaulted",
         "AssetBought", "Repossessed", "Wrecked", "VehicleStolen", "Chopped", "Crash", "Installed",
         "Episode", "Treated", "Abducted", "Harvested", "Stripped", "Overdose"]
ASSET_EVENTS = STORY[-13:]
ASSET_SNAPSHOT = ["vehicles_moto", "vehicles_car", "vehicles_truck", "vehicles_flyer", "robots",
                  "chrome_agents", "mean_sanity", "hooked"]
ASSET_TOTALS = ["truck_hauls", "walk_hauls", "crashes", "crash_deaths", "vehicle_thefts", "chops",
                "repos", "impounds", "chrome_installs", "episodes", "episodes_by_law", "stims_dealt",
                "overdoses", "treatments", "detoxes", "abductions", "harvests", "stripped"]
ASSET_FLOWS = ["flow_asset", "flow_asset_upkeep", "flow_finance", "flow_import", "flow_stims",
               "flow_parts", "flow_treatment"]
ASSET_COLS = (ASSET_SNAPSHOT + ASSET_TOTALS + ASSET_FLOWS + ["commute_tpt_walk", "commute_tpt_drive"])
VIRT_EVENTS = ["JackedIn", "DataStolen", "DataWiped", "DataSold", "LedgerHacked", "DoorHacked", "RobotTurned",
               "Blinded", "Traced", "Fried", "Flatlined", "Dumpshock", "IceRaised", "IceLowered", "TechGained",
               "TechLost"]
VIRT_STORY = ["TechLost", "TechGained", "DataWiped", "DoorHacked", "RobotTurned", "Flatlined"]
VIRT_SNAPSHOT = ["nodes", "labs", "decks", "cameras", "ice_mean_corp", "data_held"]
VIRT_TOTALS = ["runs", "runs_ok", "runs_bounced", "runs_captured", "runs_dumped", "data_made", "data_stolen",
               "data_wiped", "data_sold", "ledger_hacks", "doors_hacked", "robots_turned", "blinded", "traced",
               "fried", "flatlined", "hack_arrests", "hack_arrests_chair", "sightings", "ice_raised",
               "ice_lowered", "ice_spend", "tech_gained", "tech_lost", "research_spent"]
VIRT_FLOWS = ["flow_data", "flow_hack", "flow_ice_upkeep", "flow_research", "flow_terminal"]
LEISURE_KINDS = ["club", "arcade", "noodle_bar", "fight_pit", "den", "lounge"]
L2_FLOWS = ["flow_leisure", "flow_gamble", "flow_gamble_win", "flow_tribute", "flow_export", "flow_public_works",
            "flow_street_dice"]
L2_TOTALS = ["fab_parts", "scrap_parts", "parts_imported", "hangouts", "collected", "preached", "fv_killed",
             "fv_assaulted", "fv_robbed", "fv_abducted", "fv_bound", "fv_unknown", "fv_capped", "fv_bound_wrong",
             "stat_extorts", "stat_claims", "stat_deals", "aborts", "aborts_scavenge", "aborts_sleep",
             "aborts_checkin", "aborts_seat", "rough_sleeps", "scavenge_dry"]
ASSAULT_PER_DAY_FLAG = 42.7  # the scenario gate's assault bound (M8 gate, HANDOFF)
WORD_TOTALS = ["rumours_heard", "distorted", "grudges", "grudges_inherited", "hunts", "avenged", "revenge_kills",
               "stories", "planted", "buried", "poached", "talent_lost", "extort_tries", "extort_success", "rep_flips",
               "contradicted", "silenced", "hunts_failed", "hunts_abandoned", "guard_body", "expelled",
               "contracts_lost_honour"]
WORD_SNAPSHOT = ["hunts_active", "vendettas_open", "chain_max", "rumour_hops_max", "second_hand_share",
                 "known_by_killers_median", "pool_reach", "skill_rare_share", "law_competence"]
WORD_EVENTS = ["GrudgeFormed", "HuntStarted", "HuntAbandoned", "Avenged", "Vendetta", "VendettaEnded", "Deceived",
               "Story", "Planted", "Buried", "Poached", "TalentLost", "Expelled", "Refused", "ContractLost"]
CONTRACT_KINDS = ["hit", "beat", "guard", "locate"]
CONTRACT_TOTALS = ["contracts_posted", "contracts_fulfilled", "contracts_failed", "contracts_expired",
                   "contracts_cancelled", "reneged", "hits_done", "hits_squad", "hits_solo_weak",
                   "strikes_declined_pol", "sold_out", "contract_murders", "contract_cleared", "contract_holes",
                   "accessory", "interrogations", "interrogations_won", "bounties_paid", "fixer_runs",
                   "contracts_refused_full", "flow_escrow", "flow_payout", "flow_fixer_cut"]
CONTRACT_PROBES = ["escrow_leak", "escrow_stuck", "contract_hole_wrong"]
CONTRACT_EVENTS = ["ContractPosted", "ContractTaken", "ContractFulfilled", "ContractFailed", "ContractExpired",
                   "Reneged", "SoldOut", "StrikeDeclined", "BountyPaid", "Accessory", "FixerBusted", "GuardTaken"]
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
    assets(rows, events, out, flags)
    virt(rows, events, out, flags)
    living(rows, events, out, flags)
    word(rows, events, out)
    contracts(rows, events, out, flags)
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
    for i in range(1, 10):
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
    # The Dreg adults are `class_dreg` (M12 review dropped the duplicate `dregs` column).
    if "class_dreg" in rows[0]:
        o["dregs_mean"] = mean(col(rows, "class_dreg"))
    for k in ("squatters", "derelicts", "gangs"):
        if k in rows[0]:
            o[k + "_mean"] = mean(col(rows, k))
    adults = ("class_corp", "class_street", "class_dreg")
    if all(k in rows[0] for k in adults):
        share = []
        for r in rows:
            a = sum(r[k] for k in adults if isinstance(r.get(k), float))
            if a > 0 and isinstance(r.get("class_dreg"), float):
                share.append(r["class_dreg"] / a)
        o["dreg_share_mean"] = mean(share)
        o["dreg_share_1_5pct_days"] = sum(1 for x in share if 0.01 <= x <= 0.05)
    if events:
        o["corp_raids_won"], o["corp_raids_lost"] = raids["Won"], raids["Lost"]
        o["splits"] = len(splits)
        o["strike_events"] = sum(strikes.values())
    out["districts"] = {"city": o, "per_district": per, "splits": [f"day {d}: {t}" for d, _, t in splits]}

    if "class_dreg" in rows[0]:
        for a, b, length in runs(zip(days, (r.get("class_dreg") == 0 for r in rows)), bool):
            if length >= 30:
                flags.append(f"no Dregs (class_dreg == 0) days {a}-{b} ({length} days)")
    if events and raids["Won"] + raids["Lost"] > 0 and raids["Lost"] == 0:
        flags.append(f"corp raids never lost ({raids['Won']} won)")


def assets(rows, events, out, flags):
    """M13: vehicles, hauling, chrome, sanity, stims, parts; only when the columns exist and any is non-zero."""
    if not rows:
        return
    have = [c for c in ASSET_COLS if c in rows[0] and c != "mean_sanity"]
    if not any(any(v != 0 for v in col(rows, c)) for c in have):
        return
    total = lambda k: sum(col(rows, k))
    a = {"snapshot_last_day": {k: col(rows, k)[-1] for k in ASSET_SNAPSHOT if col(rows, k)}}
    a["totals"] = {k: total(k) for k in ASSET_TOTALS if k in rows[0]}
    late = [r for r in rows if int(r["day"]) > 30]
    t, w = sum(col(late, "truck_hauls")), sum(col(late, "walk_hauls"))
    if "truck_hauls" in rows[0] and "walk_hauls" in rows[0]:
        a["truck_share_after_day30"] = t / (t + w) if t + w else None
    if "commute_tpt_walk" in rows[0] and "commute_tpt_drive" in rows[0]:
        cw, cd = mean(col(late, "commute_tpt_walk")), mean(col(late, "commute_tpt_drive"))
        a["commute_after_day30"] = {"walk": cw, "drive": cd,
                                    "drive_over_walk": cd / cw if cw and cw == cw else None}
    a["flows_total"] = {k[5:]: total(k) for k in ASSET_FLOWS if k in rows[0]}
    if events:
        ek = Counter(k for _, k, _ in events)
        a["events"] = {k: ek[k] for k in ASSET_EVENTS if ek[k]}
    out["assets"] = a
    cd = a["totals"].get("crash_deaths", 0)
    if cd > 10:
        flags.append(f"crash_deaths {cd:g} > 10 over the run")
    apd = out["law"].get("assaults_murders_per_day", 0)
    if events and apd > ASSAULT_PER_DAY_FLAG:
        flags.append(f"assaults+murders {apd:.2f}/day > {ASSAULT_PER_DAY_FLAG:g}")


def virt(rows, events, out, flags):
    """M14: the Virt plane, Data and the tech tree; only when the columns exist and the plane ran."""
    if not rows or "nodes" not in rows[0] or not any(v for v in col(rows, "nodes")):
        return
    total = lambda k: sum(col(rows, k))
    v = {"snapshot_last_day": {k: col(rows, k)[-1] for k in VIRT_SNAPSHOT if col(rows, k)}}
    v["totals"] = {k: total(k) for k in VIRT_TOTALS if k in rows[0]}
    t = v["totals"]
    runs, ok = t.get("runs", 0), t.get("runs_ok", 0)
    v["success_share"] = ok / runs if runs else None
    v["traced_share_of_lost"] = t.get("traced", 0) / (runs - ok) if runs > ok else None
    v["stolen_over_made"] = t.get("data_stolen", 0) / t["data_made"] if t.get("data_made") else None
    tiers = {}
    for i in range(1, 10):
        ks = [f"corp{i}_tier_chrome", f"corp{i}_tier_deck", f"corp{i}_tier_industry"]
        if all(k in rows[0] for k in ks):
            first = "".join(f"{rows[0][k]:.0f}" for k in ks)
            last = "".join(f"{rows[-1][k]:.0f}" for k in ks)
            if first != "000" or last != "000":
                tiers[f"corp{i}"] = f"{first} -> {last}"
    v["tiers_first_last"] = tiers
    v["flows_total"] = {k[5:]: total(k) for k in VIRT_FLOWS if k in rows[0]}
    if events:
        ek = Counter(k for _, k, _ in events)
        v["events"] = {k: ek[k] for k in VIRT_EVENTS if ek[k]}
        v["story"] = [f"day {tk // TICKS_PER_DAY}: {k}: {x}" for tk, k, x in events if k in VIRT_STORY][:40]
    out["virt"] = v
    bands = [("runs", runs, 40, 400), ("fried", t.get("fried", 0), 3, 30), ("flatlined", t.get("flatlined", 0), 1, 10),
             ("decks on the last day", v["snapshot_last_day"].get("decks", 0), 30, 150),
             ("mean corp ICE on the last day", v["snapshot_last_day"].get("ice_mean_corp", 0), 1.0, 2.2)]
    for share, lo, hi in (("success_share", 0.3, 0.7), ("traced_share_of_lost", 0.2, 0.5), ("stolen_over_made", 0.05, 0.3)):
        if v[share] is not None:
            bands.append((share, v[share], lo, hi))
    for name, x, lo, hi in bands:
        if not lo <= x <= hi:
            flags.append(f"Virt: {name} {x:.3g} outside {lo:g}-{hi:g} (spec section 12)")


def living(rows, events, out, flags):
    """L2: the living city's columns; only when they exist and the jobs economy ran."""
    if not rows or "employed_share" not in rows[0] or not any(v for v in col(rows, "employed_share")):
        return
    total = lambda k: sum(col(rows, k))
    at = lambda d, k: rows[min(d, len(rows)) - 1][k]
    L = {}
    wd = lambda r: r["flow_wages"] / max(1.0, r["flow_dole"])
    L["wage_dole_day60"] = wd(rows[min(60, len(rows)) - 1])
    L["wage_dole_by_30d"] = [round(sum(r["flow_wages"] for r in rows[i:i + 30]) /
                                   max(1.0, sum(r["flow_dole"] for r in rows[i:i + 30])), 2)
                             for i in range(0, len(rows), 30)]
    L["employed_share_day60"] = at(60, "employed_share")
    band = (30000, 60000)
    late = rows[29:]
    L["treasury_in_band_after_day30"] = f"{sum(1 for r in late if band[0] <= r['treasury'] <= band[1])}/{len(late)}"
    L["treasury_min_max_after_day30"] = (min(col(late, "treasury"), default=0), max(col(late, "treasury"), default=0))
    L["works_jobs_last"] = at(len(rows), "works_jobs")
    L["upkeep_mult_min"] = min(col(rows, "upkeep_mult"), default=1.0)
    L["flows_total"] = {k[5:]: total(k) for k in L2_FLOWS if k in rows[0]}
    L["flow_leisure_per_day_after_day30"] = mean(col(late, "flow_leisure"))
    L["fun_mean_last"] = at(len(rows), "fun_mean")
    L["fun_satisfied_mean_after_day30"] = mean(col(late, "fun_satisfied_share"))
    L["venues_last"] = {k: at(len(rows), f"venues_{k}") for k in LEISURE_KINDS if f"venues_{k}" in rows[0]}
    L["visits_total"] = {k: total(f"visits_{k}") for k in LEISURE_KINDS if f"visits_{k}" in rows[0]}
    L["hangout_contacts_mean"] = mean([r["hangout_contacts_mean"] for r in rows if r.get("hangouts")])
    L["fronts_max"] = max(col(rows, "fronts"), default=0)
    L["totals"] = {k: total(k) for k in L2_TOTALS if k in rows[0]}
    vd, vo = total("deaths_violence"), total("deaths_violence_offscreen")
    L["offscreen_share_of_killings"] = vo / vd if vd else None
    L["kill_rates_mean"] = {k: mean(col(rows, k)) for k in ("kill_rate_body", "kill_rate_stat", "kill_rate_body_civ",
                                                           "kill_rate_stat_civ") if k in rows[0]}
    L["tiers_last"] = {k: at(len(rows), k) for k in ("tier_full", "tier_coarse", "tier_stat", "tier_held",
                                                      "gang_bodies", "gang_stat") if k in rows[0]}
    out["living"] = L
    bands = [("wages / dole on day 60", L["wage_dole_day60"], 1.0, 2.0),
             ("employed share on day 60", L["employed_share_day60"], 0.30, 0.40),
             ("wallet Gini on the last day", at(len(rows), "wallet_gini"), 0.50, 0.70),
             ("wallets on the last day", at(len(rows), "wallets"), 20000, float("inf")),
             ("fun satisfied after day 30", L["fun_satisfied_mean_after_day30"], 0.40, 0.70)]
    if L["offscreen_share_of_killings"] is not None:
        bands.append(("off-screen share of killings", L["offscreen_share_of_killings"], 0.3, 0.7))
    for name, x, lo, hi in bands:
        if not lo <= x <= hi:
            flags.append(f"L2: {name} {x:.3g} outside {lo:g}-{hi:g} (spec Goals, printed findings)")
    if L["totals"].get("fv_bound_wrong", 0) > 0:
        flags.append(f"L2: fv_bound_wrong {L['totals']['fv_bound_wrong']:g} > 0 (a faction hole bound outside its faction)")


def word(rows, events, out):
    """M15: the word and the blood's columns; only when they exist."""
    if not rows or "rumours_heard" not in rows[0]:
        return
    last = rows[-1]
    W = {"totals": {k: sum(col(rows, k)) for k in WORD_TOTALS if k in rows[0]}}
    tries = W["totals"].get("extort_tries", 0)
    W["extort_success_share"] = W["totals"].get("extort_success", 0) / tries if tries else None
    W["snapshot_last_day"] = {k: last[k] for k in WORD_SNAPSHOT if k in last}
    W["snapshot_max"] = {k: max(col(rows, k), default=0) for k in WORD_SNAPSHOT if k in rows[0]}
    gangs = {}
    for g in range(1, 10):
        dk, hk = f"g{g}_dread", f"g{g}_heat"
        if dk not in rows[0] or not any(col(rows, dk)) and not any(col(rows, hk)):
            continue
        gangs[f"g{g}"] = {"dread": (rows[0][dk], last[dk], max(col(rows, dk), default=0)),
                          "heat": (rows[0][hk], last[hk], max(col(rows, hk), default=0))}
    W["gangs_first_last_max"] = gangs
    corps = {}
    for c in range(1, 10):
        if f"c{c}_honour" not in last:
            continue
        v = (last[f"c{c}_honour"], last[f"c{c}_standing"], last[f"c{c}_competence"])
        if any(v):
            corps[f"c{c}"] = v
    W["corps_last_honour_standing_competence"] = corps
    if events:
        counts = Counter(k for _, k, _ in events)
        W["events"] = {k: counts[k] for k in WORD_EVENTS if counts[k]}
    out["word"] = W


def contracts(rows, events, out, flags):
    """M16a: the contract board's columns; only when they exist."""
    if not rows or "contracts_posted" not in rows[0]:
        return
    total = lambda k: sum(col(rows, k))
    weeks = max(1.0, len(rows) / 7.0)
    C = {"totals": {k: total(k) for k in CONTRACT_TOTALS if k in rows[0]}}
    t = C["totals"]
    C["by_kind_posted_done"] = {k: (total(f"k_{k}_posted"), total(f"k_{k}_done")) for k in CONTRACT_KINDS
                                if f"k_{k}_posted" in rows[0]}
    closed = sum(t.get(k, 0) for k in ("contracts_fulfilled", "contracts_failed", "contracts_expired",
                                       "contracts_cancelled", "reneged"))
    ratio = lambda a, b: a / b if b else None
    C["fulfilled_share_of_closed"] = ratio(t.get("contracts_fulfilled", 0), closed)
    C["squad_share_of_hits"] = ratio(t.get("hits_squad", 0), t.get("hits_done", 0))
    C["clearance"] = ratio(t.get("contract_cleared", 0), t.get("contract_murders", 0))
    C["bounties_per_locate"] = ratio(t.get("bounties_paid", 0), C["by_kind_posted_done"].get("locate", (0, 0))[0])
    C["guards_on_take_per_week"] = mean(col(rows, "guards_on_take"))
    C["open_last"] = rows[-1].get("contracts_open")
    C["regulars_last"] = rows[-1].get("regulars")
    C["lod_max"] = {k: max(col(rows, k), default=0) for k in ("live_parties", "live_queued") if k in rows[0]}
    C["escrow_held_last_max"] = (rows[-1].get("escrow_held"), max(col(rows, "escrow_held"), default=0))
    C["probes_max"] = {k: max((abs(x) for x in col(rows, k)), default=0) for k in CONTRACT_PROBES if k in rows[0]}
    fixers = {}
    for i in range(4):
        hk, ik = f"f{i}_heat", f"f{i}_income"
        if hk not in rows[0] or not any(col(rows, hk)) and not any(col(rows, ik)):
            continue
        fixers[f"f{i}"] = {"heat_last_max": (rows[-1][hk], max(col(rows, hk), default=0)),
                           "income_per_week": total(ik) / weeks}
    C["fixers"] = fixers
    if events:
        counts = Counter(k for _, k, _ in events)
        C["events"] = {k: counts[k] for k in CONTRACT_EVENTS if counts[k]}
        renders = Counter()
        for _, k, text in events:
            if k == "ContractTaken" and " the hit on " in text:
                m = re.search(r"\((LIVE|LEDGER|QUEUED)", text)
                renders[m.group(1) if m else "?"] += 1
        C["hits_taken_by_render"] = dict(renders)
        murders = counts["Murder"]
        C["murders"] = murders
        C["contract_share_of_murders"] = ratio(t.get("contract_murders", 0), murders)
    out["contracts"] = C
    for k, v in C["probes_max"].items():
        if v > 0:
            flags.append(f"M16a: {k} reached {v:g} (must stay 0: a record broke)")


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
    if "assets" in o:
        w = o["assets"]
        L.append("-- Assets (M13) --")
        L.append("last day: " + f(w["snapshot_last_day"]))
        L.append("run totals: " + f(w["totals"]))
        ts = w.get("truck_share_after_day30")
        L.append("truck share of hauls after day 30: " + ("n/a" if ts is None else f"{ts:.1%}"))
        if "commute_after_day30" in w:
            c = w["commute_after_day30"]
            r = c["drive_over_walk"]
            L.append(f"commute ticks/trip after day 30: walk {c['walk']:.4g}, drive {c['drive']:.4g}, "
                     f"drive/walk {'n/a' if r is None else f'{r:.3f}'}")
        L.append("asset flows (run totals): " + f(w["flows_total"]))
        if w.get("events"):
            L.append("asset events: " + f(w["events"]))
    if "virt" in o:
        w = o["virt"]
        pct = lambda x: "n/a" if x is None else f"{x:.1%}"
        L.append("-- Virt (M14) --")
        L.append("last day: " + f(w["snapshot_last_day"]))
        L.append("run totals: " + f(w["totals"]))
        L.append(f"success {pct(w['success_share'])}, traced of lost {pct(w['traced_share_of_lost'])}, "
                 f"stolen / made {pct(w['stolen_over_made'])}")
        if w["tiers_first_last"]:
            L.append("tiers [chrome deck industry] first -> last: " + f(w["tiers_first_last"]))
        L.append("Virt flows (run totals): " + f(w["flows_total"]))
        if w.get("events"):
            L.append("violet events: " + f(w["events"]))
        L += [f"  {x}" for x in w.get("story", [])]
    if "living" in o:
        w = o["living"]
        L.append("-- L2 (the living city) --")
        L.append(f"wages / dole day 60 {w['wage_dole_day60']:.2f}, by 30 days {w['wage_dole_by_30d']}; employed share "
                 f"day 60 {w['employed_share_day60']:.3f}")
        L.append(f"Treasury in band [30k, 60k] after day 30 {w['treasury_in_band_after_day30']}, min/max "
                 f"{w['treasury_min_max_after_day30']}, works jobs {w['works_jobs_last']:g}, upkeep_mult min "
                 f"{w['upkeep_mult_min']:.2f}")
        L.append("flows (run totals): " + f(w["flows_total"]) +
                 f"; flow_leisure/day after day 30 {w['flow_leisure_per_day_after_day30']:.0f}")
        L.append(f"fun mean (last) {w['fun_mean_last']:.3f}, satisfied after day 30 {w['fun_satisfied_mean_after_day30']:.2f}; "
                 f"HangOut known contacts {w['hangout_contacts_mean']:.3f}; fronts max {w['fronts_max']:g}")
        L.append("venues (last): " + f(w["venues_last"]) + "; visits: " + f(w["visits_total"]))
        L.append("run totals: " + f(w["totals"]))
        share = w["offscreen_share_of_killings"]
        L.append(f"off-screen share of killings {'n/a' if share is None else f'{share:.2f}'}; kill rates per 1,000: " +
                 f(w["kill_rates_mean"]))
        L.append("tiers (last): " + f(w["tiers_last"]))
    if "word" in o:
        w = o["word"]
        L.append("-- the word and the blood (M15) --")
        L.append("run totals: " + f(w["totals"]))
        share = w["extort_success_share"]
        L.append(f"extortion success {'n/a' if share is None else f'{share:.1%}'}")
        L.append("last day: " + f(w["snapshot_last_day"]))
        L.append("run max: " + f(w["snapshot_max"]))
        for g, v in w["gangs_first_last_max"].items():
            L.append(f"  {g}: dread first/last/max {v['dread'][0]:.2f}/{v['dread'][1]:.2f}/{v['dread'][2]:.2f}, "
                     f"heat {v['heat'][0]:.2f}/{v['heat'][1]:.2f}/{v['heat'][2]:.2f}")
        if w["corps_last_honour_standing_competence"]:
            L.append("corps (last day, honour/standing/competence): " +
                     ", ".join(f"{c} {h:.2f}/{s:.2f}/{k:.2f}" for c, (h, s, k) in
                               w["corps_last_honour_standing_competence"].items()))
        if w.get("events"):
            L.append("crimson events: " + f(w["events"]))
    if "contracts" in o:
        w = o["contracts"]
        pct = lambda x: "n/a" if x is None else f"{x:.1%}"
        L.append("-- the contract board (M16a) --")
        L.append("posted/done by kind: " + ", ".join(f"{k} {a:g}/{b:g}" for k, (a, b) in w["by_kind_posted_done"].items())
                 + f"; open on the last day {w['open_last']}, regulars {w['regulars_last']}")
        L.append("run totals: " + f(w["totals"]))
        L.append(f"posted {w['totals'].get('contracts_posted', 0):g} (band 30-150 per 120 days); fulfilled of closed "
                 f"{pct(w['fulfilled_share_of_closed'])} (band 35-70 %); Hits done {w['totals'].get('hits_done', 0):g} "
                 f"(band 4-20); clearance {pct(w['clearance'])} (band 20-60 %); squads of Hits "
                 f"{pct(w['squad_share_of_hits'])}")
        if "murders" in w:
            L.append(f"contract killings / Murders {pct(w['contract_share_of_murders'])} of {w['murders']} "
                     "(band 5-25 %)")
        bpl = w["bounties_per_locate"]
        L.append(f"bounties per Locate posted {'n/a' if bpl is None else f'{bpl:.2f}'}; guards on the take "
                 f"(mean) {w['guards_on_take_per_week']:.2f}; LOD max " + f(w["lod_max"]))
        L.append(f"escrow held last/max {w['escrow_held_last_max']}; probes (max, must be 0): " + f(w["probes_max"]))
        for k, v in w["fixers"].items():
            L.append(f"  {k}: heat last/max {v['heat_last_max'][0]:.2f}/{v['heat_last_max'][1]:.2f}, FixerCut "
                     f"{v['income_per_week']:.1f}/week")
        if w.get("hits_taken_by_render"):
            L.append("Hits taken by render: " + f(w["hits_taken_by_render"]))
        if w.get("events"):
            L.append("amber events: " + f(w["events"]))
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
