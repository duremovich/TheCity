//! God scenarios v2: corps, classes, the economy (docs/VISION.md, "How we
//! test: god scenarios"; written up in docs/GOD_SCENARIOS_V2.md). The v1
//! pattern (tests/god.rs): seed 42, default config, baseline days 30-45, a
//! shock at the start of day 45, observed to day 60 against an unshocked
//! control. Every test is `#[ignore]`: run
//! `cargo test --release -p citysim --test god_corps -- --ignored --nocapture`.
//!
//! Assertions say only that the world *reacted* within 7 days: an order or
//! posture that differs from the control's on some day, or a daily count
//! whose 7-day mean differs from the control's by more than twice the
//! baseline's standard deviation. Never a magnitude: the economy is
//! re-calibrated under these scenarios.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use citysim::systems::ownership;
use citysim::{
    Building, BuildingKind, Config, Corp, CorpOrder, EntityId, EventKind, Gang, Job, Niche, Order, Personality,
    PlayerCommand, Posture, Role, World, TICKS_PER_DAY,
};

macro_rules! out {
    ($o:expr, $($t:tt)*) => {{
        use std::fmt::Write as _;
        let _ = write!($o, $($t)*);
    }};
}
macro_rules! outln {
    ($o:expr) => {{
        $o.push('\n');
    }};
    ($o:expr, $($t:tt)*) => {{
        use std::fmt::Write as _;
        let _ = writeln!($o, $($t)*);
    }};
}

const SEED: u64 = 42;
const SHOCK_DAY: u64 = 45;
const END_DAY: u64 = 60;
const BASE: (u64, u64) = (30, 45);
const REACT_DAYS: u64 = 7;
/// The seeded corps (`[corps]` rows).
const SLOTS: usize = 8;

/// One seeded corp at the end of a day; `None` once dissolved.
#[derive(Clone, Debug, Default)]
struct CorpDay {
    order: CorpOrder,
    niche: Option<Niche>,
    treasury: i64,
    buildings: usize,
    employees: usize,
    exec: Option<EntityId>,
    /// The exec's greed, lawfulness, courage.
    exec_p: Option<(f32, f32, f32)>,
    /// The highest `price_level` over its niches.
    level: f32,
}

#[derive(Clone, Debug, Default)]
struct Day {
    day: u64,
    corps: Vec<Option<CorpDay>>,
    /// Corps with no seeding slot (spinoffs, incorporations) and what they own.
    new_corps: usize,
    new_corp_buildings: usize,
    /// Unrest, loyalty, submission, count per class (Corp, Street, Dreg).
    classes: [(f32, f32, f32, u32); 3],
    /// Per Market (ascending id): price, stock, owner label.
    markets: Vec<(i64, u32, String)>,
    price_mean: f64,
    price_max: i64,
    stock: u32,
    evictions: u32,
    rent_short: u32,
    housed: u32,
    homeless: u32,
    strikes: u32,
    foundings: u32,
    incorporations: u32,
    acquisitions: u32,
    bankruptcies: u32,
    monopolies: u32,
    emigrants: u32,
    starvation: u32,
    population: u32,
    employed: u32,
    gang_orders: Vec<Order>,
    gang_members: Vec<usize>,
    gang_treasury: Vec<i64>,
    gang_territory: Vec<usize>,
    posture: Option<Posture>,
    thefts: u32,
    arrests: u32,
    assaults: u32,
    extortions: u32,
    raids: u32,
    /// Buildings owned by the city, corps, agents, gangs.
    own: [usize; 4],
    /// Gang joins by agents this run has seen evicted.
    evictee_joins: u32,
    corp_order_changes: u32,
    lobbies: u32,
    undercuts: u32,
    city_treasury: i64,
}

type Series = (String, Box<dyn Fn(&Day) -> f64>);

struct Run {
    name: &'static str,
    corp_names: Vec<String>,
    gang_names: Vec<String>,
    days: Vec<Day>,
    story: Vec<(u64, EventKind, String)>,
}

fn slot_of(w: &World, c: EntityId) -> Option<usize> {
    w.comp::<Corp>(c).and_then(|cc| cc.slot).map(usize::from).filter(|&s| s < SLOTS)
}

fn snapshot(w: &World, d: &mut Day, gangs: &[EntityId]) {
    // Employees per corp: one walk over the workers.
    let mut staff: BTreeMap<EntityId, usize> = BTreeMap::new();
    for role in Role::ALL {
        for &a in w.workers(role) {
            if let Some(c) = w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e)) {
                *staff.entry(c).or_default() += 1;
            }
        }
    }
    d.corps = vec![None; SLOTS];
    for c in w.corps() {
        let cc = w.comp::<Corp>(c).expect("listed");
        match slot_of(w, c) {
            Some(s) => {
                d.corps[s] = Some(CorpDay {
                    order: cc.order,
                    niche: cc.order_niche,
                    treasury: cc.treasury,
                    buildings: cc.buildings.len(),
                    employees: staff.get(&c).copied().unwrap_or(0),
                    exec: cc.exec,
                    exec_p: cc.exec.and_then(|e| w.comp::<Personality>(e)).map(|p| (p.greed, p.lawfulness, p.courage)),
                    level: cc.price_level.values().copied().fold(0.0, f32::max),
                })
            }
            None => {
                d.new_corps += 1;
                d.new_corp_buildings += cc.buildings.len();
            }
        }
    }
    for (i, a) in w.classes.iter().enumerate() {
        d.classes[i] = (a.unrest, a.loyalty, a.submission, a.count);
    }
    for &m in w.buildings_of_kind(BuildingKind::Market) {
        let stock = w.comp::<Building>(m).map_or(0, |b| b.stock_food);
        d.markets.push((w.price_at(m), stock, w.owner_label(w.owner_of(m))));
    }
    let n = d.markets.len().max(1) as f64;
    d.price_mean = d.markets.iter().map(|m| m.0 as f64).sum::<f64>() / n;
    d.price_max = d.markets.iter().map(|m| m.0).max().unwrap_or(0);
    d.stock = d.markets.iter().map(|m| m.1).sum();
    for b in w.with::<Building>() {
        let Some(bd) = w.comp::<Building>(b).filter(|bd| !bd.demolished) else { continue };
        let k = match bd.owner {
            None => 0,
            Some(o) if w.has::<Corp>(o) => 1,
            Some(o) if w.has::<Gang>(o) => 3,
            Some(_) => 2,
        };
        d.own[k] += 1;
    }
    for &g in gangs {
        let gg = w.comp::<Gang>(g).expect("gangs are never despawned");
        d.gang_orders.push(gg.order);
        d.gang_members.push(gg.members.len());
        d.gang_treasury.push(gg.treasury);
        d.gang_territory.push(gg.territory.len());
    }
    d.posture = w.law().map(|l| l.posture);
    let row = w.stats.history.back().expect("a finished day");
    d.evictions = row.evictions;
    d.rent_short = row.rent_short;
    d.housed = row.housed;
    d.homeless = row.homeless;
    d.strikes = row.strikes;
    d.foundings = row.foundings;
    d.incorporations = row.incorporations;
    d.acquisitions = row.acquisitions;
    d.bankruptcies = row.bankruptcies;
    d.monopolies = row.monopolies;
    d.emigrants = row.emigrants;
    d.starvation = row.deaths_starvation;
    d.population = row.population;
    d.employed = row.employed;
    d.thefts = row.thefts;
    d.arrests = row.arrests;
    d.city_treasury = row.treasury;
}

/// Seed 42 to `END_DAY`; `setup` fires at the start of `BASE.0`, `shock` at
/// the start of `SHOCK_DAY` and may name an agent to watch.
fn run_from(
    name: &'static str,
    setup: impl FnOnce(&mut World),
    shock: impl FnOnce(&mut World) -> Option<EntityId>,
) -> Run {
    let mut w = World::new(SEED, Config::load());
    let gangs = w.gangs();
    let gang_names = gangs.iter().map(|&g| w.comp::<Gang>(g).map_or(String::new(), |g| g.name.clone())).collect();
    let mut corp_names = vec![String::from("-"); SLOTS];
    for c in w.corps() {
        if let Some(s) = slot_of(&w, c) {
            corp_names[s] = w.comp::<Corp>(c).map_or(String::new(), |cc| cc.name.clone());
        }
    }
    let (mut setup, mut shock) = (Some(setup), Some(shock));
    let mut evicted: BTreeSet<EntityId> = BTreeSet::new();
    let (mut days, mut story) = (Vec::new(), Vec::new());
    let mut seen = 0;
    for day in 0..END_DAY {
        if day == BASE.0 {
            (setup.take().expect("once"))(&mut w);
        }
        if day == SHOCK_DAY {
            let _ = (shock.take().expect("once"))(&mut w);
        }
        w.run_ticks(TICKS_PER_DAY);
        let mut d = Day { day, ..Day::default() };
        for e in w.events.iter().filter(|e| e.tick >= seen) {
            match e.kind {
                EventKind::Assault | EventKind::Murder => d.assaults += 1,
                EventKind::Extortion => d.extortions += 1,
                EventKind::Raid if e.text.contains(" raided ") => d.raids += 1,
                EventKind::Evicted => {
                    evicted.extend(e.actors.first());
                }
                EventKind::GangJoin if e.actors.first().is_some_and(|a| evicted.contains(a)) => d.evictee_joins += 1,
                EventKind::CorpOrder => {
                    d.corp_order_changes += 1;
                    if e.text.contains("-> Lobby") {
                        d.lobbies += 1;
                    }
                    if e.text.contains("-> Undercut") {
                        d.undercuts += 1;
                    }
                }
                _ => {}
            }
            let told = matches!(
                e.kind,
                EventKind::CorpOrder
                    | EventKind::Acquired
                    | EventKind::Bankrupt
                    | EventKind::BrokenUp
                    | EventKind::Strike
                    | EventKind::Founded
                    | EventKind::Incorporated
                    | EventKind::OrderChanged
                    | EventKind::Posture
                    | EventKind::Raid
                    | EventKind::PlayerAction
                    | EventKind::PlayerActionFailed
            );
            if told && day >= SHOCK_DAY - 2 {
                story.push((e.tick, e.kind, e.text.clone()));
            }
        }
        seen = w.tick;
        snapshot(&w, &mut d, &gangs);
        days.push(d);
    }
    Run { name, corp_names, gang_names, days, story }
}

fn run(name: &'static str, shock: impl FnOnce(&mut World) -> Option<EntityId>) -> Run {
    run_from(name, |_| {}, shock)
}

fn mean_sd(xs: &[f64]) -> (f64, f64) {
    let n = xs.len().max(1) as f64;
    let m = xs.iter().sum::<f64>() / n;
    let v = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n;
    (m, v.sqrt())
}

fn order_label(c: &Option<CorpDay>) -> String {
    match c {
        Some(c) => match c.niche {
            Some(n) => format!("{:?}/{}", c.order, &n.label()[..1]),
            None => format!("{:?}", c.order),
        },
        None => "gone".into(),
    }
}

impl Run {
    fn window(&self, from: u64, to: u64) -> impl Iterator<Item = &Day> {
        self.days.iter().filter(move |d| d.day >= from && d.day < to)
    }

    fn series(&self) -> Vec<Series> {
        let f = |name: &str, g: Box<dyn Fn(&Day) -> f64>| -> Series { (name.to_string(), g) };
        let mut s: Vec<Series> = vec![
            f("assaults", Box::new(|d| f64::from(d.assaults))),
            f("thefts", Box::new(|d| f64::from(d.thefts))),
            f("arrests", Box::new(|d| f64::from(d.arrests))),
            f("extortions", Box::new(|d| f64::from(d.extortions))),
            f("raids", Box::new(|d| f64::from(d.raids))),
            f("starvation", Box::new(|d| f64::from(d.starvation))),
            f("emigrants", Box::new(|d| f64::from(d.emigrants))),
            f("evictions", Box::new(|d| f64::from(d.evictions))),
            f("rent short", Box::new(|d| f64::from(d.rent_short))),
            f("housed", Box::new(|d| f64::from(d.housed))),
            f("homeless", Box::new(|d| f64::from(d.homeless))),
            f("employed", Box::new(|d| f64::from(d.employed))),
            f("strikes", Box::new(|d| f64::from(d.strikes))),
            f("foundings", Box::new(|d| f64::from(d.foundings))),
            f("incorporations", Box::new(|d| f64::from(d.incorporations))),
            f("acquisitions", Box::new(|d| f64::from(d.acquisitions))),
            f("bankruptcies", Box::new(|d| f64::from(d.bankruptcies))),
            f("monopolies", Box::new(|d| f64::from(d.monopolies))),
            f("price mean", Box::new(|d| d.price_mean)),
            f("price max", Box::new(|d| d.price_max as f64)),
            f("market stock", Box::new(|d| f64::from(d.stock))),
            f("evictee joins", Box::new(|d| f64::from(d.evictee_joins))),
            f("own city", Box::new(|d| d.own[0] as f64)),
            f("own corps", Box::new(|d| d.own[1] as f64)),
            f("own agents", Box::new(|d| d.own[2] as f64)),
            f("own gangs", Box::new(|d| d.own[3] as f64)),
            f("new corps", Box::new(|d| d.new_corps as f64)),
        ];
        for (i, c) in ["Corp", "Street", "Dreg"].iter().enumerate() {
            s.push((format!("{c} unrest"), Box::new(move |d: &Day| f64::from(d.classes[i].0))));
            s.push((format!("{c} loyalty"), Box::new(move |d: &Day| f64::from(d.classes[i].1))));
            s.push((format!("{c} submission"), Box::new(move |d: &Day| f64::from(d.classes[i].2))));
            s.push((format!("{c} count"), Box::new(move |d: &Day| f64::from(d.classes[i].3))));
        }
        for i in 0..self.gang_names.len() {
            s.push((format!("g{i} members"), Box::new(move |d: &Day| d.gang_members[i] as f64)));
            s.push((format!("g{i} territory"), Box::new(move |d: &Day| d.gang_territory[i] as f64)));
            s.push((format!("g{i} treasury"), Box::new(move |d: &Day| d.gang_treasury[i] as f64)));
        }
        for k in 0..SLOTS {
            let get = move |d: &Day, f: fn(&CorpDay) -> f64| d.corps[k].as_ref().map_or(0.0, f);
            s.push((format!("c{k} treasury"), Box::new(move |d: &Day| get(d, |c| c.treasury as f64))));
            s.push((format!("c{k} buildings"), Box::new(move |d: &Day| get(d, |c| c.buildings as f64))));
            s.push((format!("c{k} employees"), Box::new(move |d: &Day| get(d, |c| c.employees as f64))));
            s.push((format!("c{k} price level"), Box::new(move |d: &Day| get(d, |c| f64::from(c.level)))));
        }
        s
    }

    /// What reacted within `days` of the shock, against the control.
    fn reactions(&self, control: &Run, days: u64) -> Vec<String> {
        let mut out = Vec::new();
        let (from, to) = (SHOCK_DAY, SHOCK_DAY + days);
        let mut corp_seen = [false; SLOTS];
        let (mut gang_seen, mut posture_seen) = (false, false);
        for (d, c) in self.window(from, to).zip(control.window(from, to)) {
            for (k, seen) in corp_seen.iter_mut().enumerate() {
                let (a, b) = (order_label(&d.corps[k]), order_label(&c.corps[k]));
                if a != b && !*seen {
                    *seen = true;
                    out.push(format!("c{k} {} order on day {}: {a} (control {b})", self.corp_names[k], d.day));
                }
            }
            for i in 0..self.gang_names.len() {
                if d.gang_orders[i] != c.gang_orders[i] && !gang_seen {
                    gang_seen = true;
                    out.push(format!(
                        "g{i} order on day {}: {:?} (control {:?})",
                        d.day, d.gang_orders[i], c.gang_orders[i]
                    ));
                }
            }
            if d.posture != c.posture && !posture_seen {
                posture_seen = true;
                out.push(format!("posture on day {}: {:?} (control {:?})", d.day, d.posture, c.posture));
            }
        }
        for (name, f) in self.series() {
            let (_, bsd) = mean_sd(&self.window(BASE.0, BASE.1).map(&f).collect::<Vec<_>>());
            let (pm, _) = mean_sd(&self.window(from, to).map(&f).collect::<Vec<_>>());
            let (cm, _) = mean_sd(&control.window(from, to).map(&f).collect::<Vec<_>>());
            if (pm - cm).abs() > 2.0 * bsd && (pm - cm).abs() > 1e-9 {
                out.push(format!("{name} {pm:.2} (control {cm:.2}, base sd {bsd:.2})"));
            }
        }
        out
    }

    fn corp_history(&self, k: usize) -> String {
        let mut runs: Vec<(String, u64, u64)> = Vec::new();
        for d in self.days.iter().filter(|d| d.day >= BASE.0) {
            let o = order_label(&d.corps[k]);
            match runs.last_mut() {
                Some((p, _, end)) if *p == o => *end = d.day,
                _ => runs.push((o, d.day, d.day)),
            }
        }
        runs.iter().map(|(o, a, b)| format!("{o} {a}-{b}")).collect::<Vec<_>>().join(", ")
    }

    fn gang_history(&self, i: usize) -> String {
        let mut runs: Vec<(Order, u64, u64)> = Vec::new();
        for d in self.days.iter().filter(|d| d.day >= BASE.0) {
            match runs.last_mut() {
                Some((o, _, end)) if *o == d.gang_orders[i] => *end = d.day,
                _ => runs.push((d.gang_orders[i], d.day, d.day)),
            }
        }
        runs.iter().map(|(o, a, b)| format!("{o:?} {a}-{b}")).collect::<Vec<_>>().join(", ")
    }

    fn posture_history(&self) -> String {
        let mut runs: Vec<(Option<Posture>, u64, u64)> = Vec::new();
        for d in self.days.iter().filter(|d| d.day >= BASE.0) {
            match runs.last_mut() {
                Some((p, _, end)) if *p == d.posture => *end = d.day,
                _ => runs.push((d.posture, d.day, d.day)),
            }
        }
        runs.iter()
            .map(|(p, a, b)| format!("{} {a}-{b}", p.map_or("-".to_string(), |p| p.to_string())))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn print(&self, control: Option<&Run>) {
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        let windows = [(BASE.0, BASE.1, "base 30-45"), (45, 52, "45-52"), (52, END_DAY, "52-60")];
        let ctrl = [(45, 52, "ctl 45-52"), (52, END_DAY, "ctl 52-60")];
        out!(o, "{:<18}", "per-day mean");
        for (_, _, n) in windows {
            out!(o, "{n:>13}");
        }
        if control.is_some() {
            for (_, _, n) in ctrl {
                out!(o, "{n:>13}");
            }
        }
        outln!(o);
        let mut rows = self.series();
        rows.push(("corp order chg".into(), Box::new(|d: &Day| f64::from(d.corp_order_changes))));
        rows.push(("lobbies".into(), Box::new(|d: &Day| f64::from(d.lobbies))));
        rows.push(("undercuts".into(), Box::new(|d: &Day| f64::from(d.undercuts))));
        rows.push(("population".into(), Box::new(|d: &Day| f64::from(d.population))));
        rows.push(("city treasury".into(), Box::new(|d: &Day| d.city_treasury as f64)));
        let cell = |r: &Run, f: &dyn Fn(&Day) -> f64, a: u64, b: u64| {
            let (m, sd) = mean_sd(&r.window(a, b).map(f).collect::<Vec<_>>());
            if m.abs() >= 1000.0 {
                format!("{m:.0}±{sd:.0}")
            } else {
                format!("{m:.2}±{sd:.2}")
            }
        };
        for (name, f) in rows {
            // Rows that are zero everywhere say nothing.
            let all_zero = |r: &Run| r.days.iter().filter(|d| d.day >= BASE.0).all(|d| f(d) == 0.0);
            if all_zero(self) && control.is_none_or(all_zero) {
                continue;
            }
            out!(o, "{name:<18}");
            for (a, b, _) in windows {
                out!(o, "{:>13}", cell(self, &f, a, b));
            }
            if let Some(c) = control {
                for (a, b, _) in ctrl {
                    out!(o, "{:>13}", cell(c, &f, a, b));
                }
            }
            outln!(o);
        }
        for (k, n) in self.corp_names.iter().enumerate() {
            outln!(o, "c{k} {n}: {}", self.corp_history(k));
            if let Some(c) = control {
                outln!(o, "   control: {}", c.corp_history(k));
            }
        }
        for (i, n) in self.gang_names.iter().enumerate() {
            outln!(o, "g{i} {n}: {}", self.gang_history(i));
        }
        outln!(o, "law: {}", self.posture_history());
        outln!(o, "every 5 days: day pop price(mean/max) stock evict dreg strike | own city/corp/agent/gang | c0..c7 treasury/buildings/staff");
        for d in self.days.iter().filter(|d| d.day >= BASE.0 && (d.day % 5 == 4 || d.day == SHOCK_DAY)) {
            out!(
                o,
                "  d{:<3} {:>5} {:>4.1}/{:<3} {:>5} {:>3} {:>4} {:>2} | {}/{}/{}/{} |",
                d.day,
                d.population,
                d.price_mean,
                d.price_max,
                d.stock,
                d.evictions,
                d.classes[2].3,
                d.strikes,
                d.own[0],
                d.own[1],
                d.own[2],
                d.own[3]
            );
            for c in &d.corps {
                match c {
                    Some(c) => out!(o, " {}/{}/{}", c.treasury, c.buildings, c.employees),
                    None => out!(o, " -"),
                }
            }
            outln!(o);
        }
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for (_, k, _) in &self.story {
            *kinds.entry(format!("{k:?}")).or_default() += 1;
        }
        outln!(o, "story from day {} ({kinds:?}):", SHOCK_DAY - 2);
        for (t, k, text) in self.story.iter().take(90) {
            outln!(o, "  d{:<3} {k:?}: {text}", t / TICKS_PER_DAY);
        }
        if self.story.len() > 90 {
            outln!(o, "  ... {} more", self.story.len() - 90);
        }
        if let Some(c) = control {
            outln!(o, "reactions within {REACT_DAYS} days, vs control: {:#?}", self.reactions(c, REACT_DAYS));
        }
        eprint!("{o}");
    }

    fn assert_reacted(&self) {
        let c = control();
        self.print(Some(c));
        let r = self.reactions(c, REACT_DAYS);
        assert!(!r.is_empty(), "{}: nothing reacted within {REACT_DAYS} days of the shock", self.name);
    }
}

fn control() -> &'static Run {
    static CONTROL: OnceLock<Run> = OnceLock::new();
    CONTROL.get_or_init(|| run("god_corps_control", |_| None))
}

/// The seeded corps in a niche, by the count of `kind` they own, most first.
fn by_holdings(w: &World, niche: Niche, kind: BuildingKind) -> Vec<EntityId> {
    let mut cs: Vec<(usize, EntityId)> = w
        .corps()
        .into_iter()
        .filter(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&niche) && cc.slot.is_some()))
        .map(|c| (ownership::owned_of_kind(w, Some(c), kind).len(), c))
        .collect();
    cs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    cs.into_iter().map(|(_, c)| c).collect()
}

fn describe(w: &World, c: EntityId) -> String {
    let cc = w.comp::<Corp>(c).expect("a corp");
    format!(
        "{} (slot {:?}, {:?}, treasury {}, {} buildings, exec {})",
        cc.name,
        cc.slot,
        cc.niches,
        cc.treasury,
        cc.buildings.len(),
        cc.exec.map_or("none".into(), |e| w.name_of(e))
    )
}

/// No shock.
#[test]
#[ignore]
fn god_corps_control() {
    control().print(None);
}

/// Kill the largest Food corp's exec. A new exec? A different personality,
/// different orders? Does the corp read the loss as a shock?
#[test]
#[ignore]
fn god_kill_exec() {
    let slot = std::cell::Cell::new(0);
    let r = run("god_kill_exec", |w| {
        let c = by_holdings(w, Niche::Food, BuildingKind::Farm)[0];
        slot.set(slot_of(w, c).expect("seeded"));
        let e = w.comp::<Corp>(c).and_then(|cc| cc.exec);
        eprintln!("killing the exec of {}: personality {:?}", describe(w, c), e.and_then(|e| w.comp::<Personality>(e)));
        w.push_command(PlayerCommand::KillExec(c));
        // Who replaced them is read below from the run's days.
        None
    });
    let k = slot.get();
    let mut o = String::new();
    let mut last = None;
    for d in r.days.iter().filter(|d| d.day >= SHOCK_DAY - 1) {
        let e = d.corps[k].as_ref().and_then(|c| c.exec);
        if e != last {
            let p = d.corps[k].as_ref().and_then(|c| c.exec_p);
            outln!(o, "c{k} exec from day {}: {e:?} (greed, lawfulness, courage) {p:?}", d.day);
            last = e;
        }
    }
    eprint!("{o}");
    r.assert_reacted();
}

/// Every corp's treasury to 0 on the same day.
#[test]
#[ignore]
fn god_wipe_corps() {
    let r = run("god_wipe_corps", |w| {
        w.push_command(PlayerCommand::WipeTreasuries);
        None
    });
    r.assert_reacted();
}
