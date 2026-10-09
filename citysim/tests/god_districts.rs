//! God scenarios v3: districts, the street, riots (docs/VISION.md, "How we
//! test: god scenarios"; written up in docs/GOD_SCENARIOS_V3.md). The v1/v2
//! pattern (tests/god.rs, tests/god_corps.rs): seed 42, default config,
//! baseline days 30-45, a shock at the start of day 45, observed to day 60
//! against an unshocked control. Every test is `#[ignore]`: run
//! `cargo test --release -p citysim --test god_districts -- --ignored --nocapture`.
//!
//! Assertions say only that the world *reacted* within 7 days: a district's
//! controller or stance, the law's posture or a gang's order that differs
//! from the control's on some day, or a daily series whose 7-day mean
//! differs from the control's by more than twice the baseline's standard
//! deviation. Never a magnitude.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use citysim::systems::{districts, law_brain};
use citysim::{
    Config, DistrictId, EntityId, EventKind, Gang, Order, PlayerCommand, Posture, Role, World, TICKS_PER_DAY,
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
/// District indices on the v2 map (`[districts] names`).
const MID_EAST: u8 = 4;
const SUMP_WEST: u8 = 5;

/// One district at the end of a day.
#[derive(Clone, Debug, Default)]
struct DistrictDay {
    control: String,
    share: f32,
    stance: String,
    guards: u8,
    sweepers: u8,
    unrest: f32,
    street_unrest: f32,
    litter: f32,
    crime: f32,
    coverage: f32,
    fear: f32,
    happiness: f32,
    adults: u32,
    dregs: u32,
    rough: u16,
    riot_live: bool,
}

#[derive(Clone, Debug, Default)]
struct Day {
    day: u64,
    districts: Vec<DistrictDay>,
    posture: Option<Posture>,
    gang_orders: Vec<Order>,
    gang_members: Vec<usize>,
    gang_territory: Vec<usize>,
    gangs_live: usize,
    population: u32,
    homeless: u32,
    emigrants: u32,
    starvation: u32,
    thefts: u32,
    arrests: u32,
    evictions: u32,
    assaults: u32,
    raids: u32,
    riots: u32,
    riot_events: u32,
    looted: u32,
    crossfire: u32,
    vagrancy: u32,
    hotel_nights: u32,
    squatters: u32,
    squatted: u32,
    squat_evicted: u32,
    derelicts: u32,
    dregs: u32,
    sanitation: u32,
    stances: u32,
    control_changes: u32,
    splits: u32,
    strikes: u32,
    corp_orders: u32,
    /// Cross-gang Assaults/Murders between gangs founded after day 0 and any other.
    splinter_fights: u32,
    sweepers: usize,
}

type Series = (String, Box<dyn Fn(&Day) -> f64>);

struct Run {
    name: &'static str,
    district_names: Vec<String>,
    gang_names: Vec<String>,
    days: Vec<Day>,
    story: Vec<(u64, EventKind, String)>,
}

fn snapshot(w: &World, d: &mut Day, gangs: &[EntityId]) {
    for x in &w.districts {
        d.districts.push(DistrictDay {
            control: districts::controller_label(w, x.control),
            share: x.control_share,
            stance: law_brain::stance_label(w, x.stance),
            guards: x.guards,
            sweepers: x.sweepers,
            unrest: x.unrest,
            street_unrest: x.street_unrest,
            litter: x.litter,
            crime: x.crime_rate,
            coverage: x.coverage,
            fear: x.fear,
            happiness: x.happiness,
            adults: x.adults,
            dregs: x.classes[2],
            rough: x.rough,
            riot_live: w.riots.iter().any(|r| r.district == x.id),
        });
    }
    d.posture = w.law().map(|l| l.posture);
    for &g in gangs {
        let gg = w.comp::<Gang>(g).expect("gangs are never despawned");
        d.gang_orders.push(gg.order);
        d.gang_members.push(gg.members.len());
        d.gang_territory.push(gg.territory.len());
    }
    d.gangs_live = w.gangs().iter().filter(|&&g| w.comp::<Gang>(g).is_some_and(|x| !x.members.is_empty())).count();
    d.sweepers = w.workers(Role::Sanitation).len();
    let row = w.stats.history.back().expect("a finished day");
    d.population = row.population;
    d.homeless = row.homeless;
    d.emigrants = row.emigrants;
    d.starvation = row.deaths_starvation;
    d.thefts = row.thefts;
    d.arrests = row.arrests;
    d.evictions = row.evictions;
    d.riots = row.riots;
    d.crossfire = row.crossfire;
    d.vagrancy = row.vagrancy;
    d.hotel_nights = row.hotel_nights;
    d.squatters = row.squatters;
    d.derelicts = row.derelicts;
    d.dregs = row.class_dreg;
    d.strikes = row.strikes;
}

/// Seed 42 to `END_DAY`; `shock` fires at the start of `SHOCK_DAY`.
fn run(name: &'static str, shock: impl FnOnce(&mut World)) -> Run {
    run_from(name, |_| {}, shock)
}

/// `run`, with `setup` fired at the start of the baseline window (`BASE.0`),
/// so the baseline and the shock's control share it (M13 phase 5).
fn run_from(name: &'static str, setup: impl FnOnce(&mut World), shock: impl FnOnce(&mut World)) -> Run {
    let mut setup = Some(setup);
    let mut w = World::new(SEED, Config::load());
    let gangs = w.gangs();
    let founders: Vec<EntityId> = gangs.clone();
    let gang_names = gangs.iter().map(|&g| w.comp::<Gang>(g).map_or(String::new(), |g| g.name.clone())).collect();
    let district_names = w.districts.iter().map(|d| d.name.clone()).collect();
    let mut shock = Some(shock);
    let (mut days, mut story) = (Vec::new(), Vec::new());
    let mut next_id = 0u64;
    for day in 0..END_DAY {
        if day == BASE.0 {
            (setup.take().expect("once"))(&mut w);
        }
        if day == SHOCK_DAY {
            (shock.take().expect("once"))(&mut w);
        }
        w.run_ticks(TICKS_PER_DAY);
        let mut d = Day { day, ..Day::default() };
        for e in w.events.iter().filter(|e| e.id >= next_id) {
            match e.kind {
                EventKind::Assault | EventKind::Murder => {
                    d.assaults += 1;
                    let gs: Vec<_> = e.actors.iter().take(2).map(|&a| w.gang_of(a)).collect();
                    if let [Some(a), Some(b)] = gs[..] {
                        if a != b && (!founders.contains(&a) || !founders.contains(&b)) {
                            d.splinter_fights += 1;
                        }
                    }
                }
                EventKind::Raid if e.text.contains(" raiders vs ") => d.raids += 1,
                EventKind::Riot => d.riot_events += 1,
                EventKind::Looted => d.looted += 1,
                EventKind::Squatted => d.squatted += 1,
                EventKind::SquatEvicted => d.squat_evicted += 1,
                EventKind::Sanitation => d.sanitation += 1,
                EventKind::Stance => d.stances += 1,
                EventKind::DistrictControl => d.control_changes += 1,
                EventKind::Split => d.splits += 1,
                EventKind::CorpOrder => d.corp_orders += 1,
                _ => {}
            }
            let told = matches!(
                e.kind,
                EventKind::Riot
                    | EventKind::Looted
                    | EventKind::Crossfire
                    | EventKind::DistrictControl
                    | EventKind::Stance
                    | EventKind::Split
                    | EventKind::Posture
                    | EventKind::Strike
                    | EventKind::Raid
                    | EventKind::Derelict
                    | EventKind::Emigration
                    | EventKind::PlayerAction
                    | EventKind::PlayerActionFailed
            );
            if told && day + 2 >= SHOCK_DAY {
                story.push((e.tick, e.kind, e.text.clone()));
            }
        }
        next_id = w.events.back().map_or(next_id, |e| e.id + 1);
        snapshot(&w, &mut d, &gangs);
        days.push(d);
    }
    Run { name, district_names, gang_names, days, story }
}

fn control() -> &'static Run {
    static C: OnceLock<Run> = OnceLock::new();
    C.get_or_init(|| run("control", |_| {}))
}

fn mean_sd(xs: &[f64]) -> (f64, f64) {
    let n = xs.len().max(1) as f64;
    let m = xs.iter().sum::<f64>() / n;
    let v = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n;
    (m, v.sqrt())
}

impl Run {
    fn window(&self, from: u64, to: u64) -> impl Iterator<Item = &Day> {
        self.days.iter().filter(move |d| d.day >= from && d.day < to)
    }

    fn city_series() -> Vec<Series> {
        let f = |name: &str, g: Box<dyn Fn(&Day) -> f64>| -> Series { (name.to_string(), g) };
        vec![
            f("population", Box::new(|d| f64::from(d.population))),
            f("homeless", Box::new(|d| f64::from(d.homeless))),
            f("emigrants", Box::new(|d| f64::from(d.emigrants))),
            f("starvation", Box::new(|d| f64::from(d.starvation))),
            f("thefts", Box::new(|d| f64::from(d.thefts))),
            f("arrests", Box::new(|d| f64::from(d.arrests))),
            f("evictions", Box::new(|d| f64::from(d.evictions))),
            f("assaults", Box::new(|d| f64::from(d.assaults))),
            f("raids", Box::new(|d| f64::from(d.raids))),
            f("riots", Box::new(|d| f64::from(d.riots))),
            f("riot events", Box::new(|d| f64::from(d.riot_events))),
            f("looted", Box::new(|d| f64::from(d.looted))),
            f("crossfire", Box::new(|d| f64::from(d.crossfire))),
            f("vagrancy", Box::new(|d| f64::from(d.vagrancy))),
            f("hotel nights", Box::new(|d| f64::from(d.hotel_nights))),
            f("squatters", Box::new(|d| f64::from(d.squatters))),
            f("squatted", Box::new(|d| f64::from(d.squatted))),
            f("squat evicted", Box::new(|d| f64::from(d.squat_evicted))),
            f("derelicts", Box::new(|d| f64::from(d.derelicts))),
            f("dregs", Box::new(|d| f64::from(d.dregs))),
            f("sanitation chg", Box::new(|d| f64::from(d.sanitation))),
            f("stance chg", Box::new(|d| f64::from(d.stances))),
            f("control chg", Box::new(|d| f64::from(d.control_changes))),
            f("splits", Box::new(|d| f64::from(d.splits))),
            f("strikes", Box::new(|d| f64::from(d.strikes))),
            f("corp orders", Box::new(|d| f64::from(d.corp_orders))),
            f("live gangs", Box::new(|d| d.gangs_live as f64)),
            f("splinter fights", Box::new(|d| f64::from(d.splinter_fights))),
            f("sweepers", Box::new(|d| d.sweepers as f64)),
        ]
    }

    fn district_series(i: usize) -> Vec<Series> {
        let f = |name: &str, g: fn(&DistrictDay) -> f64| -> Series {
            (format!("d{i} {name}"), Box::new(move |d: &Day| d.districts.get(i).map_or(0.0, g)))
        };
        vec![
            f("guards", |x| f64::from(x.guards)),
            f("sweepers", |x| f64::from(x.sweepers)),
            f("unrest", |x| f64::from(x.unrest)),
            f("street unrest", |x| f64::from(x.street_unrest)),
            f("litter", |x| f64::from(x.litter)),
            f("crime", |x| f64::from(x.crime)),
            f("coverage", |x| f64::from(x.coverage)),
            f("fear", |x| f64::from(x.fear)),
            f("happiness", |x| f64::from(x.happiness)),
            f("adults", |x| f64::from(x.adults)),
            f("dregs", |x| f64::from(x.dregs)),
            f("rough", |x| f64::from(x.rough)),
            f("ctl share", |x| f64::from(x.share)),
            f("riot live", |x| f64::from(u8::from(x.riot_live))),
        ]
    }

    fn series(&self) -> Vec<Series> {
        let mut s = Self::city_series();
        for i in 0..self.district_names.len() {
            s.extend(Self::district_series(i));
        }
        s
    }

    /// What reacted within `days` of the shock, against the control.
    fn reactions(&self, control: &Run, days: u64) -> Vec<String> {
        let mut out = Vec::new();
        let (from, to) = (SHOCK_DAY, SHOCK_DAY + days);
        let n = self.district_names.len();
        let (mut ctl_seen, mut stance_seen) = (vec![false; n], vec![false; n]);
        let (mut gang_seen, mut posture_seen) = (false, false);
        for (d, c) in self.window(from, to).zip(control.window(from, to)) {
            for i in 0..n {
                let (a, b) = (&d.districts[i], &c.districts[i]);
                if a.control != b.control && !ctl_seen[i] {
                    ctl_seen[i] = true;
                    out.push(format!(
                        "{} control on day {}: {} (control {})",
                        self.district_names[i], d.day, a.control, b.control
                    ));
                }
                if a.stance != b.stance && !stance_seen[i] {
                    stance_seen[i] = true;
                    out.push(format!(
                        "{} stance on day {}: {} (control {})",
                        self.district_names[i], d.day, a.stance, b.stance
                    ));
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

    fn label_history(&self, get: impl Fn(&Day) -> String) -> String {
        let mut runs: Vec<(String, u64, u64)> = Vec::new();
        for d in self.days.iter().filter(|d| d.day >= BASE.0) {
            let v = get(d);
            match runs.last_mut() {
                Some((p, _, end)) if *p == v => *end = d.day,
                _ => runs.push((v, d.day, d.day)),
            }
        }
        runs.iter().map(|(v, a, b)| format!("{v} {a}-{b}")).collect::<Vec<_>>().join(", ")
    }

    /// The table (city series plus the `focus` districts' series), the
    /// focus districts' controller and stance histories, the law and the
    /// gangs, every 5 days of the focus districts, the story and the reactions.
    fn print(&self, control: Option<&Run>, focus: &[u8]) {
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        let windows = [(BASE.0, BASE.1, "base 30-45"), (45, 52, "45-52"), (52, END_DAY, "52-60")];
        let ctrl = [(45, 52, "ctl 45-52"), (52, END_DAY, "ctl 52-60")];
        out!(o, "{:<22}", "per-day mean");
        for (_, _, n) in windows {
            out!(o, "{n:>13}");
        }
        if control.is_some() {
            for (_, _, n) in ctrl {
                out!(o, "{n:>13}");
            }
        }
        outln!(o);
        let mut rows = Self::city_series();
        for &i in focus {
            rows.extend(Self::district_series(usize::from(i)));
        }
        let cell = |r: &Run, f: &dyn Fn(&Day) -> f64, a: u64, b: u64| {
            let (m, sd) = mean_sd(&r.window(a, b).map(f).collect::<Vec<_>>());
            if m.abs() >= 1000.0 {
                format!("{m:.0}±{sd:.0}")
            } else {
                format!("{m:.2}±{sd:.2}")
            }
        };
        for (name, f) in rows {
            let all_zero = |r: &Run| r.days.iter().filter(|d| d.day >= BASE.0).all(|d| f(d) == 0.0);
            if all_zero(self) && control.is_none_or(all_zero) {
                continue;
            }
            out!(o, "{name:<22}");
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
        for &i in focus {
            let i = usize::from(i);
            let name = &self.district_names[i];
            outln!(o, "{name} control: {}", self.label_history(|d| d.districts[i].control.clone()));
            outln!(o, "{name} stance: {}", self.label_history(|d| d.districts[i].stance.clone()));
            if let Some(c) = control {
                outln!(o, "   control run: {}", c.label_history(|d| d.districts[i].control.clone()));
                outln!(o, "   control stance: {}", c.label_history(|d| d.districts[i].stance.clone()));
            }
        }
        outln!(o, "law: {}", self.label_history(|d| d.posture.map_or("-".to_string(), |p| p.to_string())));
        for (i, n) in self.gang_names.iter().enumerate() {
            outln!(o, "g{i} {n}: {}", self.label_history(|d| format!("{:?}", d.gang_orders[i])));
        }
        outln!(o, "every 5 days, per focus district: guards stance unrest litter crime adults dregs rough | city riots squatters dregs live-gangs");
        for d in self.days.iter().filter(|d| d.day >= BASE.0 && (d.day % 5 == 4 || d.day == SHOCK_DAY)) {
            out!(o, "  d{:<3}", d.day);
            for &i in focus {
                let x = &d.districts[usize::from(i)];
                out!(
                    o,
                    " | {:>2} {:<12} {:.2} {:.2} {:.2} {:>4} {:>3} {:>2}",
                    x.guards,
                    x.stance,
                    x.unrest,
                    x.litter,
                    x.crime,
                    x.adults,
                    x.dregs,
                    x.rough
                );
            }
            outln!(o, " | {} {} {} {}", d.riots, d.squatters, d.dregs, d.gangs_live);
        }
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for (_, k, _) in &self.story {
            *kinds.entry(format!("{k:?}")).or_default() += 1;
        }
        outln!(o, "story from day {} ({kinds:?}):", SHOCK_DAY - 2);
        for (t, k, text) in self.story.iter().take(80) {
            outln!(o, "  d{:<3} {k:?}: {text}", t / TICKS_PER_DAY);
        }
        if self.story.len() > 80 {
            outln!(o, "  ... {} more", self.story.len() - 80);
        }
        if let Some(c) = control {
            outln!(o, "reactions within {REACT_DAYS} days, vs control: {:#?}", self.reactions(c, REACT_DAYS));
        }
        eprint!("{o}");
    }

    fn assert_reacted(&self, focus: &[u8]) {
        self.assert_reacted_against(control(), focus);
    }

    fn assert_reacted_against(&self, c: &Run, focus: &[u8]) {
        self.print(Some(c), focus);
        let r = self.reactions(c, REACT_DAYS);
        assert!(!r.is_empty(), "{}: nothing reacted within {REACT_DAYS} days of the shock", self.name);
    }
}

/// No shock.
#[test]
#[ignore]
fn god_districts_control() {
    control().print(None, &[SUMP_WEST, MID_EAST]);
}

/// Force a riot in Sump West. Does the law respond (Cordon, Disperse,
/// Crush)? Do the corps shock? Does control change?
#[test]
#[ignore]
fn god_riot_sump_west() {
    // Seed 42's Sump West riots on its own on the shock's midnight (the
    // control's natural riot is the god riot, one tick apart), so the shared
    // setup switches natural riots off: the control then has none and the
    // forced riot is the only one.
    let r = run_from(
        "god_riot_sump_west",
        |w| w.config.riots.riot_days = u16::MAX,
        |w| w.push_command(PlayerCommand::Riot(DistrictId(SUMP_WEST))),
    );
    r.assert_reacted(&[SUMP_WEST]);
}

/// City rent 10 on Sump Blocks: evict the Sump's city tenants. Dregs,
/// squats, riots, emigration?
#[test]
#[ignore]
fn god_evict_sump_rent_10() {
    let r = run("god_evict_sump_rent_10", |w| {
        let [_, mid, spire] = w.levers.city_rent;
        w.push_command(PlayerCommand::SetCityRent([10, mid, spire]));
    });
    r.assert_reacted(&[SUMP_WEST, 6, 7]);
}
