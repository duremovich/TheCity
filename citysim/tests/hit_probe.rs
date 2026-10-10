//! The Hit market's measuring device (docs/HIT_MARKET.md § 0; `#[ignore]`,
//! prints only, asserts nothing):
//!
//! - `hit_probe`: seeds 42-44 × 120 days, every 20 days, who could pay for a
//!   death (grudge holders, wallets, kin pools, quotes, the guns' asks,
//!   gang and corp purses, witnesses, finance arrears);
//! - `hit_market_report`: seeds 42-47 × 120 days, the organic records
//!   (posted by motive and kind, fulfilled, stiffed, charged), violent
//!   deaths, and the story lines of the fulfilled ones.
//!
//! Run alone: `cargo test --release -p citysim --test hit_probe -- --ignored --nocapture`
//! (with `CITYSIM_ASSETS=<a copy of assets with wages + no_safety_net on>`
//! for the flipped city).
use std::collections::BTreeMap;

use citysim::contract::{ContractKind, ContractStatus, Motive, Target};
use citysim::systems::{contracts, demography, hunt, law};
use citysim::word::{GrudgeCause, Grudges};
use citysim::{Config, Corp, Crime, EntityId, EventKind, Gang, Wallet, World, TICKS_PER_DAY, TICKS_PER_HOUR};

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
                    }
                    let mut out = format!(
                        "seed {seed}: violent deaths {violent}; organic [posted, done, failed, expired, cancelled, live] {:?}; mean price {} deposit {}; brokered {brokered}/{n}; stiffed {stiffed}; buyers charged {charged}; tries short {unaff}; kin refused {refused}; pooled {pooled}\n",
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
