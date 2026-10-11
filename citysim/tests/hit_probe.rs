//! The Hit market's measuring device (docs/HIT_MARKET.md § 0; `#[ignore]`,
//! prints only, asserts nothing):
//!
//! - `hit_probe`: seeds 42-44 × 120 days, every 20 days, who could pay for a
//!   death (grudge holders, wallets, kin pools, quotes, the guns' asks,
//!   gang and corp purses, witnesses, finance arrears);
//! - `hit_market_report`: seeds 42-47 × 120 days, the organic records
//!   (posted by motive and kind, fulfilled, stiffed, charged), violent
//!   deaths, and the story lines of the fulfilled ones;
//! - `exec_protection`: seed 42 at day 30, each corp exec's guards, the
//!   strike estimate's defenders, the market price and the quote.
//!
//! Run alone: `cargo test --release -p citysim --test hit_probe -- --ignored --nocapture`
//! (with `CITYSIM_ASSETS=<a copy of assets with wages + no_safety_net on>`
//! for the flipped city).
use std::collections::BTreeMap;

use citysim::contract::{ContractKind, ContractStatus, Motive, Target};
use citysim::systems::{contracts, demography, hunt, law};
use citysim::word::{GrudgeCause, Grudges};
use citysim::{Config, Corp, Crime, EntityId, EventKind, Gang, Wallet, World, TICKS_PER_DAY, TICKS_PER_HOUR};

/// Seeds 42-43, the market on and off: each seeded corp's fate (the day it
/// went, the Bankrupt/BrokenUp lines naming it), its execs' deaths, what it
/// paid for organic records, and its treasury every 15 days.
#[test]
#[ignore]
fn corp_fates() {
    let runs: Vec<(u64, bool)> = vec![(42, true), (42, false), (43, true), (43, false)];
    let out: Vec<String> = std::thread::scope(|s| {
        let hs: Vec<_> = runs
            .iter()
            .map(|&(seed, on)| {
                s.spawn(move || {
                    let mut cfg = Config::load();
                    cfg.hitmarket.enabled = on;
                    let mut w = World::new(seed, cfg);
                    let seeded: Vec<EntityId> = w.corps();
                    let names: BTreeMap<EntityId, String> = seeded
                        .iter()
                        .map(|&c| (c, w.comp::<Corp>(c).map_or_else(String::new, |x| x.name.clone())))
                        .collect();
                    let mut lines: BTreeMap<EntityId, Vec<String>> = BTreeMap::new();
                    let mut execs: BTreeMap<EntityId, EntityId> = BTreeMap::new();
                    let mut cursor = 0u64;
                    let mut gone: BTreeMap<EntityId, u64> = BTreeMap::new();
                    let mut kinds: BTreeMap<(EntityId, u64, String), u32> = BTreeMap::new();
                    let mut paid: BTreeMap<EntityId, i64> = BTreeMap::new();
                    let mut counted: std::collections::BTreeSet<u64> = Default::default();
                    for day in 0..120u64 {
                        for x in w.contracts.values() {
                            if let Some(b) = x.buyer.filter(|b| seeded.contains(b)) {
                                if counted.insert(x.id) {
                                    *paid.entry(b).or_insert(0) += x.price;
                                }
                            }
                        }
                        for c in &seeded {
                            if let Some(e) = w.comp::<Corp>(*c).and_then(|x| x.exec) {
                                execs.insert(e, *c);
                            }
                        }
                        for _ in 0..24 {
                            w.run_ticks(TICKS_PER_HOUR);
                            for e in w.events.iter().rev().take_while(|e| e.id >= cursor) {
                                for (&ex, &c) in &execs {
                                    if e.kind == EventKind::Death && e.actors.first() == Some(&ex) {
                                        lines.entry(c).or_default().push(format!("d{day} EXEC DEATH {}", e.text));
                                    }
                                }
                                for &c in &seeded {
                                    let named = e.actors.contains(&c) || e.text.contains(&w.owner_label(Some(c)));
                                    let k = format!("{:?}", e.kind);
                                    if named
                                        && (k.contains("Bankrupt")
                                            || k.contains("CorpOrder")
                                            || k.contains("TalentLost")
                                            || k.contains("BrokenUp")
                                            || k.contains("Acquired")
                                            || (e.kind == EventKind::ContractPosted
                                                && (e.text.contains("(rival") || e.text.contains("(culprit"))))
                                    {
                                        lines.entry(c).or_default().push(format!("d{day} {k} {}", e.text));
                                    } else if named {
                                        *kinds.entry((c, day / 15, k)).or_insert(0) += 1;
                                    }
                                }
                            }
                            cursor = w.events.back().map_or(cursor, |e| e.id + 1);
                        }
                        let alive: std::collections::BTreeSet<EntityId> = w.corps().into_iter().collect();
                        for &c in &seeded {
                            if !alive.contains(&c) && !gone.contains_key(&c) {
                                gone.insert(c, day);
                            }
                            if day % 15 == 14 && alive.contains(&c) {
                                lines
                                    .entry(c)
                                    .or_default()
                                    .push(format!("d{day} treasury {}", w.comp::<Corp>(c).map_or(0, |x| x.treasury)));
                            }
                        }
                    }
                    let mut s = format!("=== seed {seed} market {}\n", if on { "ON" } else { "OFF" });
                    for &c in &seeded {
                        let name = names[&c].clone();
                        s += &format!(
                            "-- {name} {c:?}: gone day {:?}; contract prices posted (all kinds) {}\n",
                            gone.get(&c),
                            paid.get(&c).copied().unwrap_or(0)
                        );
                        for win in 0..8u64 {
                            let row: Vec<String> = kinds
                                .iter()
                                .filter(|((cc, ww, _), _)| *cc == c && *ww == win)
                                .map(|((_, _, k), n)| format!("{k} {n}"))
                                .collect();
                            if !row.is_empty() {
                                s += &format!("   days {}-{}: {}\n", win * 15, win * 15 + 14, row.join(", "));
                            }
                        }
                        for l in lines.get(&c).into_iter().flatten() {
                            if !l.contains("Acquired") {
                                s += &format!("   {l}\n");
                            }
                        }
                    }
                    s
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for s in out {
        print!("{s}");
    }
}

#[test]
#[ignore]
fn exec_protection() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(30 * TICKS_PER_DAY);
    let f = contracts::open_fixers(&w).first().copied();
    for c in w.corps() {
        let Some(e) = w.comp::<Corp>(c).and_then(|x| x.exec) else { continue };
        let job = w.comp::<citysim::Job>(e).and_then(|j| j.employer);
        let home = w.comp::<citysim::Household>(e).and_then(|h| h.home);
        let guards = |b: Option<EntityId>| b.map_or(0, |b| citysim::systems::raid::private_guards_of(&w, b).len());
        let t = Target::Agent(e);
        let defs = citysim::systems::missions::expected_defenders(&w, &t).len();
        let mp = contracts::market_price(&w, ContractKind::Hit, &t, Some(c), None, f);
        let q = contracts::quote(&w, ContractKind::Hit, &t, f);
        println!(
            "{}: exec {} job {:?} ({} guards) home {:?} ({} guards) defenders {defs} market {mp:?} quote {q}",
            w.owner_label(Some(c)),
            w.name_of(e),
            job.map(|b| w.name_of(b)),
            guards(job),
            home.map(|b| w.name_of(b)),
            guards(home),
        );
    }
}

/// One organic record's last seen state.
#[derive(Clone, Debug)]
struct Seen {
    kind: ContractKind,
    motive: Motive,
    status: ContractStatus,
    price: i64,
    deposit: i64,
    charged: bool,
    brokered: bool,
    /// A corp bought it on a corp exec (decided when first seen).
    exec: bool,
    /// Ever taken.
    taken: bool,
}

#[test]
#[ignore]
fn hit_market_report() {
    let seeds = [42u64, 43, 44, 45, 46, 47];
    let lines: Vec<String> = std::thread::scope(|s| {
        let hs: Vec<_> = seeds
            .iter()
            .map(|&seed| {
                s.spawn(move || {
                    let mut w = World::new(seed, Config::load());
                    let seeded: Vec<EntityId> = w.corps();
                    let mut seen: BTreeMap<u64, Seen> = BTreeMap::new();
                    let mut stories: Vec<String> = vec![];
                    let (mut cursor, mut violent, mut stiffed, mut unaff, mut refused, mut pooled) = (0u64, 0u32, 0u32, 0u32, 0u32, 0i64);
                    for _day in 0..120 {
                        for _ in 0..24 {
                            w.run_ticks(TICKS_PER_HOUR);
                            for e in w.events.iter().rev().take_while(|e| e.id >= cursor) {
                                let t = &e.text;
                                let organic = t.contains("(revenge")
                                    || t.contains(" down, ")
                                    || t.contains("(rival")
                                    || t.contains("(culprit")
                                    || t.contains("(protect");
                                match e.kind {
                                    EventKind::ContractPosted if organic => {
                                        stories.push(format!("  d{} POST {t}", e.tick / TICKS_PER_DAY))
                                    }
                                    EventKind::Stiffed => stories.push(format!("  d{} STIFF {t}", e.tick / TICKS_PER_DAY)),
                                    EventKind::Accessory => {
                                        stories.push(format!("  d{} ACCESSORY {t}", e.tick / TICKS_PER_DAY))
                                    }
                                    _ => {}
                                }
                            }
                            cursor = w.events.back().map_or(cursor, |e| e.id + 1);
                        }
                        for c in w.contracts.values().filter(|c| !c.motive.is_none()) {
                            let first = !seen.contains_key(&c.id);
                            let prev = seen.get(&c.id).map(|s| s.status);
                            if first {
                                stories.push(format!(
                                    "  d{} #{} is the {} on {} ({})",
                                    c.posted / TICKS_PER_DAY,
                                    c.id,
                                    c.kind.label(),
                                    w.name_of(c.target.id()),
                                    c.motive.label()
                                ));
                            }
                            if (first || prev != Some(c.status)) && c.status == ContractStatus::Fulfilled {
                                stories.push(format!(
                                    "  d{} DONE {} on {} for {} by {} ({} {})",
                                    w.tick / TICKS_PER_DAY,
                                    c.kind.label(),
                                    w.name_of(c.target.id()),
                                    w.owner_label(c.buyer),
                                    w.owner_label(c.taker),
                                    c.motive.label(),
                                    c.render.label()
                                ));
                            }
                            seen.insert(
                                c.id,
                                Seen {
                                    kind: c.kind,
                                    motive: c.motive,
                                    status: c.status,
                                    price: c.price,
                                    deposit: c.deposit,
                                    charged: c.charged,
                                    brokered: c.brokered(),
                                    taken: c.taker.is_some() || seen.get(&c.id).is_some_and(|s| s.taken),
                                    exec: seen.get(&c.id).map_or_else(
                                        || {
                                            c.buyer.is_some_and(|b| w.has::<Corp>(b))
                                                && w.corps().iter().any(|&k| {
                                                    w.comp::<Corp>(k).and_then(|x| x.exec) == Some(c.target.id())
                                                })
                                        },
                                        |s| s.exec,
                                    ),
                                },
                            );
                        }
                        let since = w.tick.saturating_sub(TICKS_PER_DAY);
                        for (t, id, text) in w.contract_log.iter().filter(|(t, _, _)| *t >= since) {
                            if seen.contains_key(id) && !text.contains(" posted a ") {
                                stories.push(format!("  d{} #{id} {text}", t / TICKS_PER_DAY));
                            }
                        }
                        if let Some(r) = w.stats.history.back() {
                            violent += r.deaths_violence;
                            stiffed += r.hit.hm_stiffed;
                            unaff += r.hit.hm_unaffordable;
                            refused += r.hit.hm_refused;
                            pooled += r.hit.hm_pooled;
                        }
                    }
                    let mut by: BTreeMap<(String, &str), [u32; 6]> = BTreeMap::new();
                    let (mut exec_prices, mut exec_done): (Vec<i64>, u32) = (Vec::new(), 0);
                    let (mut price, mut dep, mut n, mut charged, mut brokered) = (0i64, 0i64, 0i64, 0u32, 0u32);
                    for s in seen.values() {
                        let e = by.entry((s.motive.label().to_string(), s.kind.label())).or_default();
                        e[0] += 1;
                        e[match s.status {
                            ContractStatus::Fulfilled | ContractStatus::Reneged => 1,
                            ContractStatus::Failed => 2,
                            ContractStatus::Expired => 3,
                            ContractStatus::Cancelled => 4,
                            _ => 5,
                        }] += 1;
                        price += s.price;
                        dep += if s.deposit > 0 { s.deposit } else { s.price };
                        n += 1;
                        charged += u32::from(s.charged);
                        brokered += u32::from(s.brokered);
                        if s.exec && s.kind == ContractKind::Hit {
                            exec_prices.push(s.price);
                            exec_done += u32::from(matches!(s.status, ContractStatus::Fulfilled | ContractStatus::Reneged));
                        }
                    }
                    let alive: std::collections::BTreeSet<EntityId> = w.corps().into_iter().collect();
                    let corps_alive = seeded.iter().filter(|c| alive.contains(c)).count();
                    let mut out = format!(
                        "seed {seed}: violent deaths {violent}; seeded corps alive {corps_alive}/{}; corp exec Hits posted {}, prices {exec_prices:?}, fulfilled {exec_done}; organic [posted, done, failed, expired, cancelled, live] {:?}; mean price {} deposit {}; brokered {brokered}/{n}; stiffed {stiffed}; buyers charged {charged}; tries short {unaff}; kin refused {refused}; pooled {pooled}\n",
                        seeded.len(),
                        w.hm_corp_hits.len(),
                        by,
                        if n > 0 { price / n } else { 0 },
                        if n > 0 { dep / n } else { 0 },
                    );
                    for l in stories {
                        out += &l;
                        out.push('\n');
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for l in lines {
        print!("{l}");
    }
}

fn coins(w: &World, id: EntityId) -> i64 {
    w.comp::<Wallet>(id).map_or(0, |x| x.coins)
}
fn pct(v: &[i64], p: f64) -> i64 {
    if v.is_empty() {
        return -1;
    }
    let mut v = v.to_vec();
    v.sort();
    v[((v.len() - 1) as f64 * p) as usize]
}

#[test]
#[ignore]
fn hit_probe() {
    let seeds = [42u64, 43, 44];
    std::thread::scope(|s| {
        for &seed in &seeds {
            s.spawn(move || {
                let mut w = World::new(seed, Config::load());
                for day in 1..=120u64 {
                    w.run_ticks(TICKS_PER_DAY);
                    if day % 20 != 0 {
                        continue;
                    }
                    let mut out = String::new();
                    let adults: Vec<EntityId> = w
                        .citizens()
                        .into_iter()
                        .filter(|&a| demography::is_adult(&w, a) && law::living(&w, a))
                        .collect();
                    let wal: Vec<i64> = adults.iter().map(|&a| coins(&w, a)).collect();
                    out += &format!(
                        "seed {seed} d{day}: adults {} wallet p50 {} p75 {} p90 {} p99 {}\n",
                        adults.len(),
                        pct(&wal, 0.5),
                        pct(&wal, 0.75),
                        pct(&wal, 0.9),
                        pct(&wal, 0.99)
                    );
                    let (mut n_hunt, mut n_leth, mut n_kin, mut n_weak, mut n_afford, mut n_afford_kin) =
                        (0, 0, 0, 0, 0, 0);
                    let mut quotes: Vec<i64> = vec![];
                    let mut ratio: Vec<i64> = vec![];
                    let mut kinpool: Vec<i64> = vec![];
                    let mut causes = std::collections::BTreeMap::<&str, u32>::new();
                    for h in w.with::<Grudges>() {
                        if !hunt::hunter_ok(&w, h) {
                            continue;
                        }
                        let g = w.comp::<Grudges>(h).unwrap().clone();
                        for x in g.list.iter().filter(|x| x.settled.is_none() && x.weight >= 0.5) {
                            let c = match x.cause {
                                GrudgeCause::KilledKin(_) => "kin",
                                GrudgeCause::KilledFriend(_) => "friend",
                                GrudgeCause::Assaulted => "assault",
                                GrudgeCause::Robbed => "robbed",
                                GrudgeCause::Inherited(_) => "inherited",
                                GrudgeCause::Betrayed => "betrayed",
                                GrudgeCause::Hired => "hired",
                                _ => "other",
                            };
                            *causes.entry(c).or_default() += 1;
                        }
                        let Some((t, wt, _)) = hunt::heaviest_eligible(&w, h) else { continue };
                        n_hunt += 1;
                        if wt >= w.config.hunt.lethal_min {
                            n_leth += 1;
                        }
                        if hunt::might_gap(&w, h, t) < w.config.contracts.hire_gap {
                            n_weak += 1;
                        }
                        let kind = if wt >= w.config.hunt.lethal_min { ContractKind::Hit } else { ContractKind::Beat };
                        let q = contracts::quote(&w, kind, &Target::Agent(t), None);
                        quotes.push(q);
                        let own = coins(&w, h);
                        let mut pool = own;
                        for o in w.neighbours(h) {
                            if let Some(e) = w.edge(h, o) {
                                if matches!(
                                    e.kind,
                                    citysim::RelKind::Spouse | citysim::RelKind::Parent | citysim::RelKind::Family
                                ) && demography::is_adult(&w, o)
                                    && law::living(&w, o)
                                {
                                    pool += coins(&w, o);
                                }
                            }
                        }
                        kinpool.push(pool);
                        ratio.push((own * 100) / q.max(1));
                        if own >= q {
                            n_afford += 1;
                        }
                        if pool >= q {
                            n_afford_kin += 1;
                        }
                        if matches!(
                            g.list.iter().find(|x| x.target == t).map(|x| x.cause),
                            Some(GrudgeCause::KilledKin(_))
                        ) {
                            n_kin += 1;
                        }
                    }
                    out += &format!(
                        "  grudges>=0.5 by cause {:?}\n  eligible {n_hunt} lethal {n_leth} kinkill {n_kin} weak {n_weak} afford_own {n_afford} afford_kinpool {n_afford_kin}; quote p10 {} p50 {} p90 {}; own%quote p50 {} p90 {}; kinpool p50 {} p90 {}\n",
                        causes,
                        pct(&quotes, 0.1),
                        pct(&quotes, 0.5),
                        pct(&quotes, 0.9),
                        pct(&ratio, 0.5),
                        pct(&ratio, 0.9),
                        pct(&kinpool, 0.5),
                        pct(&kinpool, 0.9),
                    );
                    let mut gt: Vec<String> = vec![];
                    for g in w.gangs() {
                        let gg = w.comp::<Gang>(g).unwrap();
                        let lq = gg.leader.map_or(0, |l| contracts::quote(&w, ContractKind::Hit, &Target::Agent(l), None));
                        gt.push(format!("{}m/{}c/q{}", gg.members.len(), w.purse(Some(g)), lq));
                    }
                    out += &format!("  gangs [members/treasury/leader-hit-quote] {}\n", gt.join(" "));
                    let mut ct: Vec<String> = vec![];
                    for c in w.corps() {
                        let cc = w.comp::<Corp>(c).unwrap();
                        let eq = cc.exec.map_or(0, |e| contracts::quote(&w, ContractKind::Hit, &Target::Agent(e), None));
                        let lawf =
                            cc.exec.and_then(|e| w.comp::<citysim::Personality>(e)).map_or(-1.0, |p| p.lawfulness);
                        ct.push(format!("{}c/q{}/L{:.2}/{:?}", w.purse(Some(c)), eq, lawf, cc.order));
                    }
                    out += &format!("  corps [treasury/exec-hit-quote/exec-law/order] {}\n", ct.join(" "));
                    let (mut wm, mut wa, mut wm_rich) = (0, 0, 0);
                    for r in w.crime_reports().iter().filter(|r| !r.resolved) {
                        let Some(wit) = r.witness else { continue };
                        if !law::living(&w, wit) || !law::living(&w, r.suspect) {
                            continue;
                        }
                        match r.crime {
                            Crime::Murder => {
                                wm += 1;
                                let q = contracts::quote(&w, ContractKind::Hit, &Target::Agent(wit), None);
                                if coins(&w, r.suspect) >= q / 4 {
                                    wm_rich += 1;
                                }
                            }
                            Crime::Assault => wa += 1,
                            _ => {}
                        }
                    }
                    out += &format!(
                        "  open reports w/ living witness: murder {wm} (suspect holds q/4: {wm_rich}) assault {wa}\n"
                    );
                    let mut arrears = 0;
                    for a in w.with::<citysim::Asset>() {
                        if let Some(f) = w.comp::<citysim::Asset>(a).and_then(|x| x.finance.clone()) {
                            if f.arrears >= 3 {
                                arrears += 1;
                            }
                        }
                    }
                    out += &format!("  financed assets in arrears>=3: {arrears}\n");
                    // gun asks: closed form of the Contract goal's acceptance at accept_min
                    let mut regs: Vec<EntityId> = vec![];
                    for f in contracts::open_fixers(&w) {
                        if let Some(k) = w.comp::<citysim::contract::Broker>(f) {
                            regs.extend(k.regulars.keys().copied());
                        }
                    }
                    let lawmax = 0.3f32;
                    let guns: Vec<EntityId> = regs
                        .iter()
                        .copied()
                        .filter(|&g| w.comp::<citysim::Personality>(g).is_some_and(|p| p.lawfulness <= lawmax))
                        .filter(|&g| contracts::free_adult(&w, g))
                        .collect();
                    let mut asks: Vec<i64> = vec![];
                    let mut cheapest: Vec<i64> = vec![];
                    let mut targets: Vec<EntityId> = vec![];
                    for h in w.with::<Grudges>() {
                        if let Some((t, wt, _)) = hunt::heaviest_eligible(&w, h) {
                            if wt >= w.config.hunt.lethal_min {
                                targets.push(t);
                            }
                        }
                    }
                    for &t in &targets {
                        let tg = Target::Agent(t);
                        let mut best = i64::MAX;
                        for &g in &guns {
                            let Some(ask) = contracts::ask(&w, g, ContractKind::Hit, &tg, true, None) else {
                                continue;
                            };
                            asks.push(ask);
                            best = best.min(ask);
                        }
                        if best < i64::MAX {
                            cheapest.push(best);
                        }
                    }
                    out += &format!(
                        "  fixer regulars {} guns(law<=0.3,free) {}; lethal targets {}; asks p10 {} p50 {} p90 {}; cheapest per target p50 {} max {}\n",
                        regs.len(),
                        guns.len(),
                        targets.len(),
                        pct(&asks, 0.1),
                        pct(&asks, 0.5),
                        pct(&asks, 0.9),
                        pct(&cheapest, 0.5),
                        pct(&cheapest, 1.0)
                    );
                    print!("{out}");
                }
            });
        }
    });
}

/// One seed's run for `hm_ab_report`.
#[derive(Default)]
struct AbRun {
    murders: u32,
    crackdown_days: u32,
    violent: u32,
    corps_alive: usize,
    corps_seeded: usize,
    /// (motive, kind) -> [posted, done, failed, expired, cancelled, live].
    by: BTreeMap<(String, String), [u32; 6]>,
    prices_hit: Vec<i64>,
    prices_beat: Vec<i64>,
    deposits: Vec<i64>,
    charged: u32,
    charge_delays: Vec<u64>,
    /// Organic records expired: (after a taker held them, never taken).
    expired: (u32, u32),
    cols: BTreeMap<&'static str, i64>,
    floors: Vec<(u64, i64, i64, i64)>,
    stories: Vec<String>,
}

fn ab_run(seed: u64, on: bool) -> AbRun {
    let mut cfg = Config::load();
    cfg.hitmarket.enabled = on;
    let mut w = World::new(seed, cfg);
    let seeded: Vec<EntityId> = w.corps();
    let mut r = AbRun { corps_seeded: seeded.len(), ..Default::default() };
    let mut seen: BTreeMap<u64, Seen> = BTreeMap::new();
    let mut closed_day: BTreeMap<u64, u64> = BTreeMap::new();
    let mut cursor = 0u64;
    for day in 0..120u64 {
        for _ in 0..24 {
            w.run_ticks(TICKS_PER_HOUR);
            for e in w.events.iter().rev().take_while(|e| e.id >= cursor) {
                let d = e.tick / TICKS_PER_DAY;
                match e.kind {
                    EventKind::Murder => r.murders += 1,
                    EventKind::Stiffed | EventKind::Accessory | EventKind::Recanted | EventKind::CaseCold => {
                        r.stories.push(format!("  d{d} {:?} {}", e.kind, e.text))
                    }
                    EventKind::ContractPosted if e.text.contains(" down, ") || e.text.ends_with(')') => {
                        r.stories.push(format!("  d{d} POST {}", e.text))
                    }
                    _ => {}
                }
            }
            cursor = w.events.back().map_or(cursor, |e| e.id + 1);
        }
        for c in w.contracts.values().filter(|c| !c.motive.is_none()) {
            let prev = seen.get(&c.id).cloned();
            if prev.is_none() {
                match c.kind {
                    ContractKind::Hit => r.prices_hit.push(c.price),
                    _ => r.prices_beat.push(c.price),
                }
                r.deposits.push(if c.deposit > 0 { c.deposit } else { c.price });
            }
            if !c.status.is_live() && !closed_day.contains_key(&c.id) {
                closed_day.insert(c.id, c.closed.unwrap_or(w.tick) / TICKS_PER_DAY);
                if c.status == ContractStatus::Fulfilled {
                    r.stories.push(format!(
                        "  d{day} DONE {} on {} for {} by {} ({})",
                        c.kind.label(),
                        w.name_of(c.target.id()),
                        w.owner_label(c.buyer),
                        w.owner_label(c.taker),
                        c.motive.label()
                    ));
                }
            }
            if c.charged && !prev.as_ref().is_some_and(|p| p.charged) {
                let cd = closed_day.get(&c.id).copied().unwrap_or(day);
                r.charge_delays.push(day.saturating_sub(cd));
            }
            seen.insert(
                c.id,
                Seen {
                    kind: c.kind,
                    motive: c.motive,
                    status: c.status,
                    price: c.price,
                    deposit: c.deposit,
                    charged: c.charged,
                    brokered: c.brokered(),
                    taken: c.taker.is_some() || seen.get(&c.id).is_some_and(|s| s.taken),
                    exec: false,
                },
            );
        }
        if w.law().is_some_and(|l| l.posture == citysim::Posture::Crackdown) {
            r.crackdown_days += 1;
        }
        if let Some(row) = w.stats.history.back() {
            r.violent += row.deaths_violence;
            let h = &row.hit;
            for (k, v) in [
                ("unaffordable", i64::from(h.hm_unaffordable)),
                ("refused", i64::from(h.hm_refused)),
                ("pooled", h.hm_pooled),
                ("stiffed", i64::from(h.hm_stiffed)),
                ("wants", i64::from(h.hm_wants)),
                ("commissions", i64::from(h.hm_commissions)),
                ("recanted", i64::from(h.hm_recanted)),
                ("cold", i64::from(h.hm_cold)),
            ] {
                *r.cols.entry(k).or_default() += v;
            }
        }
        if matches!(day, 29 | 59 | 119) {
            let floor = 50i64.max((0.5 * 7.0 * w.median_wage as f32).round() as i64);
            // The gross wage the flip pays (`wages::gross_wage`), median.
            let mut gross: Vec<i64> = w
                .with::<citysim::Job>()
                .iter()
                .filter_map(|&a| w.comp::<citysim::Job>(a).map(|j| citysim::systems::wages::gross_wage(&w, j)))
                .collect();
            gross.sort_unstable();
            let g = gross.get(gross.len() / 2).copied().unwrap_or(0);
            r.floors.push((day + 1, w.median_wage, g, floor));
        }
    }
    for s in seen.values() {
        let e = r.by.entry((s.motive.label().to_string(), s.kind.label().to_string())).or_default();
        e[0] += 1;
        e[match s.status {
            ContractStatus::Fulfilled | ContractStatus::Reneged => 1,
            ContractStatus::Failed => 2,
            ContractStatus::Expired => 3,
            ContractStatus::Cancelled => 4,
            _ => 5,
        }] += 1;
        r.charged += u32::from(s.charged);
        if s.status == ContractStatus::Expired {
            if s.taken {
                r.expired.0 += 1;
            } else {
                r.expired.1 += 1;
            }
        }
    }
    let alive: std::collections::BTreeSet<EntityId> = w.corps().into_iter().collect();
    r.corps_alive = seeded.iter().filter(|c| alive.contains(c)).count();
    r
}

/// The Hit market on and off (≈ main) on seeds 42-47 × 120 days, the
/// shipped config: Murders, violent deaths, corps alive at d120, the
/// organic records per motive and kind ([posted, done, failed, expired,
/// cancelled, live]), prices, the gun's week-wage floor (decision 7) at
/// days 30/60/120, charges and their delay, and the `HitCols` sums.
/// `HM_STORIES=<file>` writes the market-on story lines there.
#[test]
#[ignore]
fn hm_ab_report() {
    let seeds = [42u64, 43, 44, 45, 46, 47];
    let runs: Vec<(u64, bool)> = seeds.iter().flat_map(|&s| [(s, true), (s, false)]).collect();
    let out: Vec<(u64, bool, AbRun)> = std::thread::scope(|s| {
        let hs: Vec<_> = runs.iter().map(|&(seed, on)| s.spawn(move || (seed, on, ab_run(seed, on)))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut stories = String::new();
    for (seed, on, r) in &out {
        let tag = if *on { "on " } else { "off" };
        let p = |v: &Vec<i64>| {
            let mut v = v.clone();
            v.sort_unstable();
            if v.is_empty() {
                "-".to_string()
            } else {
                format!("n{} min {} p50 {} max {}", v.len(), v[0], v[v.len() / 2], v[v.len() - 1])
            }
        };
        println!(
            "seed {seed} {tag}: murders {} violent {} corps {}/{}; crackdown days {}; floor (day, median wage, median gross, floor) {:?}",
            r.murders, r.violent, r.corps_alive, r.corps_seeded, r.crackdown_days, r.floors
        );
        if *on {
            println!("    organic {:?}; expired (taken, never taken) {:?}", r.by, r.expired);
            println!(
                "    hit prices [{}] beat prices [{}] deposits [{}]; charged {} delays {:?}; cols {:?}",
                p(&r.prices_hit),
                p(&r.prices_beat),
                p(&r.deposits),
                r.charged,
                r.charge_delays,
                r.cols
            );
            stories.push_str(&format!("seed {seed}\n"));
            for l in &r.stories {
                stories.push_str(l);
                stories.push('\n');
            }
        }
    }
    if let Ok(path) = std::env::var("HM_STORIES") {
        std::fs::write(path, stories).expect("write stories");
    }
}

/// "Nobody takes" (P2 fix round): seed 44, every organic Revenge Hit, read
/// at 17:59 each day it is Open (the minute before `match_day`): its place
/// in its Fixer's book (`offers_per_day`), and per candidate (a brokered
/// record's regulars, a direct one's viewers as `direct_pool` reads them:
/// the buyer's edges but Enemies) why matching would not take it: the
/// `eligible` filter that fails, else the candidate's ask against the
/// price and its `accept_score`. Prints the log of each record that ends
/// Expired.
#[test]
#[ignore]
fn why_expired() {
    use citysim::systems::demography;
    use citysim::RelKind;
    let seed: u64 = std::env::var("HM_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(44);
    let mut w = World::new(seed, Config::load());
    let mut logs: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    let mut last: BTreeMap<u64, (ContractStatus, Option<u32>)> = BTreeMap::new();
    let match_tick = u64::from(w.config.fixers.match_hour) * TICKS_PER_HOUR;
    let per_day = usize::from(w.config.fixers.offers_per_day);
    let hit_law = w.config.fixers.gun_lawfulness.get(ContractKind::Hit);
    for _day in 0..120 {
        w.run_ticks(match_tick - 1);
        let ids: Vec<u64> = w
            .contracts
            .values()
            .filter(|c| c.motive == Motive::Revenge && c.kind == ContractKind::Hit && c.is_open())
            .map(|c| c.id)
            .collect();
        for id in ids {
            let c = w.contracts[&id].clone();
            let t = c.target.id();
            let mut out: Vec<String> = Vec::new();
            let (cands, book) = match c.broker {
                Some(f) => {
                    let k = w.comp::<citysim::contract::Broker>(f).unwrap();
                    let now = w.tick;
                    let mut book: Vec<(f64, u64, i64)> = k
                        .book
                        .iter()
                        .filter_map(|x| w.contracts.get(x))
                        .filter(|x| x.is_open())
                        .map(|x| {
                            let age = now.saturating_sub(x.posted) as f64 / TICKS_PER_DAY as f64;
                            let days = (x.deadline.saturating_sub(x.posted) as f64 / TICKS_PER_DAY as f64).max(1.0);
                            (x.price as f64 * (1.0 + age / days), x.id, x.price)
                        })
                        .collect();
                    book.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                    let rank = book.iter().position(|x| x.1 == id).unwrap_or(99);
                    let prices: Vec<i64> = book.iter().map(|x| x.2).collect();
                    (
                        k.regulars.keys().copied().collect::<Vec<_>>(),
                        format!("book rank {rank} of {} (offers {per_day}) prices {prices:?}", book.len()),
                    )
                }
                None => {
                    let b = c.buyer.unwrap();
                    let v: Vec<EntityId> =
                        w.neighbours(b).filter(|&o| w.edge(b, o).is_some_and(|e| e.kind != RelKind::Enemy)).collect();
                    (v, "direct".to_string())
                }
            };
            out.push(format!(
                "d{} #{id} {} hit on {} for {} ({} down) by {}: {book}, {} candidates",
                w.tick / TICKS_PER_DAY,
                if c.broker.is_some() { "brokered" } else { "direct" },
                t.index,
                c.price,
                c.deposit,
                c.buyer.map_or(0, |b| b.index),
                cands.len()
            ));
            let mut why: BTreeMap<&str, u32> = BTreeMap::new();
            for a in cands {
                let reason = if a == t || Some(a) == c.buyer {
                    "party"
                } else if !contracts::free_adult(&w, a) {
                    if demography::is_adult(&w, a) {
                        "jailed/cuffed/leaving/dead"
                    } else {
                        "child"
                    }
                } else if law::is_guard(&w, a) {
                    "guard"
                } else if w.edge(a, t).is_some_and(|e| {
                    matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend)
                }) {
                    "close to the target"
                } else if w.comp::<citysim::Personality>(a).is_some_and(|p| p.lawfulness > hit_law) {
                    "lawful (> gun_lawfulness)"
                } else if contracts::holds_taken(&w, a) {
                    "holds a taken record"
                } else {
                    let ask = contracts::ask(&w, a, c.kind, &c.target, c.broker.is_some(), c.buyer);
                    let score = contracts::accept_score(&w, a, id);
                    out.push(format!(
                        "    cand {}: ask {ask:?} vs price {} -> score {score:.3} (min {}) {}",
                        a.index,
                        c.price,
                        w.config.contracts.accept_min,
                        if score >= w.config.contracts.accept_min { "WOULD TAKE" } else { "declines" }
                    ));
                    continue;
                };
                *why.entry(reason).or_default() += 1;
            }
            out.push(format!("    filtered: {why:?}"));
            logs.entry(id).or_default().extend(out);
        }
        let since = w.tick;
        w.run_ticks(TICKS_PER_DAY - (match_tick - 1));
        if std::env::var("HM_LIVE").is_ok() && logs.values().any(|_| true) {
            let mut live: Vec<(String, &str, i64, bool)> = w
                .contracts
                .values()
                .filter(|c| c.status == ContractStatus::Taken && c.render == citysim::contract::Render::Live)
                .map(|c| {
                    (
                        format!("#{}", c.id),
                        c.kind.label(),
                        c.price,
                        c.started.is_some() || w.contract_runs.values().any(|r| r.contract == c.id),
                    )
                })
                .collect();
            live.sort_by_key(|x| -x.2);
            let row = w.stats.history.back().map(|r| (r.contract.live_parties, r.contract.live_queued));
            println!("day {} live (parties, queued) {row:?}: {live:?}", w.tick / TICKS_PER_DAY);
        }
        let lines: Vec<(u64, String)> = w
            .contract_log
            .iter()
            .filter(|(tk, cid, _)| *tk >= since && logs.contains_key(cid))
            .map(|(tk, cid, text)| {
                (*cid, format!("    log d{}h{}: {text}", tk / TICKS_PER_DAY, (tk % TICKS_PER_DAY) / TICKS_PER_HOUR))
            })
            .collect();
        for (cid, l) in lines {
            logs.entry(cid).or_default().push(l);
        }
        for id in logs.keys() {
            if let Some(c) = w.contracts.get(id) {
                last.insert(*id, (c.status, c.taker.map(|x| x.index)));
            }
        }
    }
    let mut shown = 0;
    for (id, log) in &logs {
        let status = last.get(id).copied();
        println!("record #{id} ends {status:?}");
        if status.is_some_and(|s| s.0 == ContractStatus::Expired) {
            for l in log {
                println!("{l}");
            }
            shown += 1;
        }
    }
    println!("{shown} expired Revenge Hits of {} logged", logs.len());
}

/// One taken record's chase, every 2 hours (`HM_SEED`, `HM_RECORD`,
/// `HM_FROM` day): the taker's goal and step, its run's phase and intel,
/// both tiers and the distance to the target.
#[test]
#[ignore]
fn trace_record() {
    let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(d);
    let (seed, rid, from) = (env("HM_SEED", 44), env("HM_RECORD", 55), env("HM_FROM", 29));
    let mut w = World::new(seed, Config::load());
    w.run_ticks(from * TICKS_PER_DAY);
    for _ in 0..(12 * 12) {
        w.run_ticks(2 * TICKS_PER_HOUR);
        let Some(c) = w.contracts.get(&rid).cloned() else { continue };
        let Some(g) = c.taker else { continue };
        let t = c.target.id();
        let b = w.comp::<citysim::Brain>(g);
        let run = w.contract_runs.get(&g);
        let tile = |x: EntityId| w.comp::<citysim::Position>(x).map(|p| (p.tile, p.building.map(|b| b.index)));
        println!(
            "d{}h{} {:?} goal {:?} step {:?} run {:?} intel {:?} lods {:?}/{:?} taker {:?} target {:?}",
            w.tick / TICKS_PER_DAY,
            (w.tick % TICKS_PER_DAY) / TICKS_PER_HOUR,
            c.status,
            b.and_then(|b| b.current_goal),
            b.and_then(|b| b.current_step().map(|s| s.action)),
            run.map(|r| r.phase),
            run.and_then(|r| r.intel.map(|i| (i.tile, i.building.map(|x| x.index)))),
            b.map(|b| b.lod),
            w.comp::<citysim::Brain>(t).map(|b| b.lod),
            tile(g),
            tile(t)
        );
        if !c.status.is_live() {
            break;
        }
    }
}
