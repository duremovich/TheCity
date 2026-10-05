//! God scenarios (docs/VISION.md, "How we test: god scenarios"). Shock the
//! world with a god-mode actor at day 45 and read how the factions react.
//! Every test is `#[ignore]` (a 2,000-resident city for 90 days each): run
//! `cargo test --release -p citysim --test god -- --ignored --nocapture`.
//!
//! Assertions say only that the world *reacted* within 7 days of the shock:
//! an order or posture changed, or a daily count's 7-day mean moved by more
//! than twice the baseline's (days 30-45) standard deviation. What it did is
//! printed, and written up in docs/GOD_SCENARIOS_V1.md.

use std::sync::OnceLock;

use citysim::{Config, EntityId, EventKind, Gang, Order, PlayerCommand, Posture, World, TICKS_PER_DAY};

/// `write!` / `writeln!` into a `String`, ignoring the (infallible) result.
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
// The VISION brief asked for a shock at day 30, but on seed 42 the gangs
// are only forming then (The Hollow has 4-5 members at day 30, all in the
// Jail, so no leader); both reach ~16 members by day 40-45. The shock waits
// for an established underworld; `god_control` is the unshocked run.
const SHOCK_DAY: u64 = 45;
const END_DAY: u64 = 105;
const BASE: (u64, u64) = (30, 45);
const REACT_DAYS: u64 = 7;

/// One finished day.
#[derive(Clone, Debug, Default)]
struct Day {
    day: u64,
    orders: Vec<Order>,
    members: Vec<usize>,
    treasury: Vec<i64>,
    territory: Vec<usize>,
    joins: Vec<u32>,
    order_changes: u32,
    posture: Option<Posture>,
    posture_changes: u32,
    violence: u32,
    thefts: u32,
    arrests: u32,
    /// Raids on the rival Hideout (any outcome), storms of the Jail, fizzles.
    raids: u32,
    storms: u32,
    fizzles: u32,
    jailbreaks: u32,
    bribes: u32,
    population: u32,
    starvation: u32,
    guards: usize,
    jailed: u32,
    city_treasury: i64,
    homes: usize,
}

/// A named per-day metric.
type Series = (String, Box<dyn Fn(&Day) -> f64>);

struct Run {
    name: &'static str,
    gang_names: Vec<String>,
    days: Vec<Day>,
    /// `(tick, kind, text)` of the story events from day 28 on.
    story: Vec<(u64, EventKind, String)>,
}

/// Run seed 42 to `END_DAY`; `shock` fires at the start of `SHOCK_DAY`.
fn run(name: &'static str, shock: impl FnOnce(&mut World)) -> Run {
    let mut w = World::new(SEED, Config::load());
    let gangs = w.gangs();
    let gang_names = gangs.iter().map(|&g| w.comp::<Gang>(g).map_or(String::new(), |g| g.name.clone())).collect();
    let mut shock = Some(shock);
    let mut days = Vec::new();
    let mut story = Vec::new();
    let mut seen = 0;
    for day in 0..END_DAY {
        if day == SHOCK_DAY {
            (shock.take().expect("once"))(&mut w);
        }
        w.run_ticks(TICKS_PER_DAY);
        let mut d = Day { day, ..Day::default() };
        d.joins = vec![0; gangs.len()];
        for e in w.events.iter().filter(|e| e.tick >= seen) {
            match e.kind {
                EventKind::OrderChanged => d.order_changes += 1,
                EventKind::Posture => d.posture_changes += 1,
                EventKind::Assault | EventKind::Murder => d.violence += 1,
                EventKind::Raid if e.text.contains(" raided ") => d.raids += 1,
                EventKind::Raid if e.text.contains("stormed the Jail") => d.storms += 1,
                EventKind::Raid => d.fizzles += 1,
                EventKind::Jailbreak => d.jailbreaks += 1,
                EventKind::Bribe => d.bribes += 1,
                EventKind::GangJoin => {
                    if let Some(i) = e.actors.get(1).and_then(|g| gangs.iter().position(|x| x == g)) {
                        d.joins[i] += 1;
                    }
                }
                _ => {}
            }
            let told = matches!(
                e.kind,
                EventKind::OrderChanged
                    | EventKind::Posture
                    | EventKind::Raid
                    | EventKind::Jailbreak
                    | EventKind::Bribe
                    | EventKind::Sacked
                    | EventKind::PlayerAction
                    | EventKind::PlayerActionFailed
            );
            if told && day >= SHOCK_DAY - 2 {
                story.push((e.tick, e.kind, e.text.clone()));
            }
        }
        seen = w.tick;
        for &g in &gangs {
            let gg = w.comp::<Gang>(g).expect("gangs are never despawned");
            d.orders.push(gg.order);
            d.members.push(gg.members.len());
            d.treasury.push(gg.treasury);
            d.territory.push(gg.territory.len());
        }
        d.posture = w.law().map(|l| l.posture);
        let row = w.stats.history.back().expect("a finished day").clone();
        d.thefts = row.thefts;
        d.arrests = row.arrests;
        d.population = row.population;
        d.starvation = row.deaths_starvation;
        d.jailed = row.jailed;
        d.city_treasury = row.treasury;
        d.guards = w.guards().len();
        d.homes = w.buildings_of_kind(citysim::BuildingKind::Home).len();
        days.push(d);
    }
    Run { name, gang_names, days, story }
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

    /// Every series the reaction check looks at, by name.
    fn series(&self) -> Vec<Series> {
        let mut s: Vec<Series> = vec![
            ("violence".into(), Box::new(|d: &Day| f64::from(d.violence))),
            ("thefts".into(), Box::new(|d: &Day| f64::from(d.thefts))),
            ("arrests".into(), Box::new(|d: &Day| f64::from(d.arrests))),
            ("raids".into(), Box::new(|d: &Day| f64::from(d.raids))),
            ("jail storms".into(), Box::new(|d: &Day| f64::from(d.storms))),
            ("raid fizzles".into(), Box::new(|d: &Day| f64::from(d.fizzles))),
            ("jailbreaks".into(), Box::new(|d: &Day| f64::from(d.jailbreaks))),
            ("bribes".into(), Box::new(|d: &Day| f64::from(d.bribes))),
            ("starvation".into(), Box::new(|d: &Day| f64::from(d.starvation))),
            ("guards".into(), Box::new(|d: &Day| d.guards as f64)),
        ];
        for i in 0..self.gang_names.len() {
            s.push((format!("g{i} members"), Box::new(move |d: &Day| d.members[i] as f64)));
            s.push((format!("g{i} territory"), Box::new(move |d: &Day| d.territory[i] as f64)));
            s.push((format!("g{i} treasury"), Box::new(move |d: &Day| d.treasury[i] as f64)));
        }
        s
    }

    /// What reacted within `days` of the shock, against the unshocked
    /// control (seed 42 is deterministic, so every difference from the
    /// control is the shock's doing): an order or posture that differs on
    /// some day, or a daily count whose window mean differs from the
    /// control's by more than twice the baseline's standard deviation.
    fn reactions(&self, control: &Run, days: u64) -> Vec<String> {
        let mut out = Vec::new();
        let (from, to) = (SHOCK_DAY, SHOCK_DAY + days);
        let (mut order_seen, mut posture_seen) = (false, false);
        for (d, c) in self.window(from, to).zip(control.window(from, to)) {
            for i in 0..self.gang_names.len() {
                if d.orders[i] != c.orders[i] && !order_seen {
                    order_seen = true;
                    out.push(format!("g{i} order on day {}: {:?} (control {:?})", d.day, d.orders[i], c.orders[i]));
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
                out.push(format!("{name} {pm:.1} (control {cm:.1}, base sd {bsd:.1})"));
            }
        }
        out
    }

    /// `Expand d0-14, LieLow d15-17, ...` for gang `i`.
    fn order_history(&self, i: usize) -> String {
        let mut runs: Vec<(Order, u64, u64)> = Vec::new();
        for d in &self.days {
            match runs.last_mut() {
                Some((o, _, end)) if *o == d.orders[i] => *end = d.day,
                _ => runs.push((d.orders[i], d.day, d.day)),
            }
        }
        runs.iter().map(|(o, a, b)| format!("{o:?} {a}-{b}")).collect::<Vec<_>>().join(", ")
    }

    fn posture_history(&self) -> String {
        let mut runs: Vec<(Option<Posture>, u64, u64)> = Vec::new();
        for d in &self.days {
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
        // One write, so parallel tests do not interleave their tables.
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        let windows = [(BASE.0, BASE.1, "base 30-45"), (45, 52, "45-52"), (45, 75, "45-75"), (75, 105, "75-105")];
        let ctrl_windows = [(45, 52, "ctl 45-52"), (45, 75, "ctl 45-75"), (75, 105, "ctl 75-105")];
        out!(o, "{:<16}", "per-day mean");
        for (_, _, n) in windows {
            out!(o, "{n:>13}");
        }
        if control.is_some() {
            for (_, _, n) in ctrl_windows {
                out!(o, "{n:>13}");
            }
        }
        outln!(o);
        let mut rows: Vec<Series> = self.series();
        rows.push(("order changes".into(), Box::new(|d: &Day| f64::from(d.order_changes))));
        rows.push(("posture changes".into(), Box::new(|d: &Day| f64::from(d.posture_changes))));
        rows.push(("population".into(), Box::new(|d: &Day| f64::from(d.population))));
        rows.push(("jailed".into(), Box::new(|d: &Day| f64::from(d.jailed))));
        rows.push(("city treasury".into(), Box::new(|d: &Day| d.city_treasury as f64)));
        for i in 0..self.gang_names.len() {
            rows.push((format!("g{i} joins"), Box::new(move |d: &Day| f64::from(d.joins[i]))));
        }
        let cell = |r: &Run, f: &dyn Fn(&Day) -> f64, a: u64, b: u64| {
            let (m, sd) = mean_sd(&r.window(a, b).map(f).collect::<Vec<_>>());
            if m.abs() >= 1000.0 {
                format!("{m:.0}±{sd:.0}")
            } else {
                format!("{m:.1}±{sd:.1}")
            }
        };
        for (name, f) in rows {
            out!(o, "{name:<16}");
            for (a, b, _) in windows {
                out!(o, "{:>13}", cell(self, &f, a, b));
            }
            if let Some(c) = control {
                for (a, b, _) in ctrl_windows {
                    out!(o, "{:>13}", cell(c, &f, a, b));
                }
            }
            outln!(o);
        }
        for (i, n) in self.gang_names.iter().enumerate() {
            outln!(o, "g{i} {n}: {}", self.order_history(i));
        }
        outln!(o, "law: {}", self.posture_history());
        outln!(o, "every 5 days: day pop viol theft arr raid | g0 mem/terr/tres | g1 mem/terr/tres | guards treasury");
        for d in self.days.iter().filter(|d| d.day >= BASE.0 && (d.day % 5 == 4 || d.day == SHOCK_DAY)) {
            outln!(
                o,
                "  d{:<3} {:>5} {:>3} {:>3} {:>3} {:>2} | {:>3}/{:>3}/{:>6} | {:>3}/{:>3}/{:>6} | {:>3} {:>7}",
                d.day,
                d.population,
                d.violence,
                d.thefts,
                d.arrests,
                d.raids,
                d.members[0],
                d.territory[0],
                d.treasury[0],
                d.members[1],
                d.territory[1],
                d.treasury[1],
                d.guards,
                d.city_treasury,
            );
        }
        outln!(o, "story from day {}:", SHOCK_DAY - 2);
        for (t, k, text) in self.story.iter().take(60) {
            outln!(o, "  d{:<3} {k:?}: {text}", t / TICKS_PER_DAY);
        }
        if self.story.len() > 60 {
            outln!(o, "  ... {} more", self.story.len() - 60);
        }
        if let Some(c) = control {
            outln!(o, "reactions within {REACT_DAYS} days, vs control: {:?}", self.reactions(c, REACT_DAYS));
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

/// The unshocked run every scenario is compared against, computed once per
/// test process.
fn control() -> &'static Run {
    static CONTROL: OnceLock<Run> = OnceLock::new();
    CONTROL.get_or_init(|| run("god_control", |_| {}))
}

/// No shock: the baseline world, for comparison with every scenario.
#[test]
#[ignore]
fn god_control() {
    control().print(None);
}

fn gang(w: &World, i: usize) -> EntityId {
    w.gangs()[i]
}

fn leader(w: &World, i: usize) -> Option<EntityId> {
    w.comp::<Gang>(gang(w, i)).and_then(|g| g.leader)
}

/// Kill gang 0's leader. Does the gang recompute a leader, Retaliate against
/// the rival (wrongly), LieLow, lose members?
#[test]
#[ignore]
fn god_decapitate_gang() {
    let r = run("god_decapitate_gang", |w| {
        let g = w.comp::<Gang>(gang(w, 0)).expect("gang 0");
        let l = g.leader.or(g.boss).expect("gang 0 has a leader or a jailed boss at the shock");
        eprintln!("killing {} (leader of gang 0)", w.name_of(l));
        w.push_command(PlayerCommand::KillAgent(l));
    });
    r.assert_reacted();
}

/// Jail every member of gang 0 for 60 days. Does the rival expand into its
/// turf? Does the law go Garrison? Any breakout (none can: nobody is outside)?
#[test]
#[ignore]
fn god_jail_whole_gang() {
    let r = run("god_jail_whole_gang", |w| {
        let g = gang(w, 0);
        w.push_command(PlayerCommand::JailGang { gang: g, days: 60 });
    });
    r.assert_reacted();
}

/// Kill every member of gang 0. Does the rival take the city? Does crime fall
/// or rise? Does the empty gang re-form (the JoinGang bootstrap)?
#[test]
#[ignore]
fn god_kill_gang() {
    let r = run("god_kill_gang", |w| {
        let g = gang(w, 0);
        w.push_command(PlayerCommand::KillGang(g));
    });
    r.assert_reacted();
}

/// Give gang 1 a treasury of 10,000. Recruitment surge? Raids? Bribes? Does
/// the law notice?
#[test]
#[ignore]
fn god_fund_gang() {
    let r = run("god_fund_gang", |w| {
        let g = gang(w, 1);
        w.push_command(PlayerCommand::FundGang { gang: g, amount: 10_000 });
    });
    r.assert_reacted();
}

/// Fire every guard. Crime and violence curves; does the law re-hire? Do the
/// gangs go Expand? Starvation?
#[test]
#[ignore]
fn god_fire_all_guards() {
    let r = run("god_fire_all_guards", |w| w.push_command(PlayerCommand::FireAllGuards));
    r.assert_reacted();
}

/// Pin the law to Garrison for the rest of the run (60 days). Do the gangs
/// expand unchecked, raid more, extort more?
#[test]
#[ignore]
fn god_garrison_forever() {
    let r = run("god_garrison_forever", |w| w.push_command(PlayerCommand::SetLawPosture(Some(Posture::Garrison))));
    r.assert_reacted();
}

/// Pin Crackdown. Does the target gang bribe, LieLow, break out, collapse?
/// Does the other gang profit?
#[test]
#[ignore]
fn god_crackdown_forever() {
    let r = run("god_crackdown_forever", |w| w.push_command(PlayerCommand::SetLawPosture(Some(Posture::Crackdown))));
    // A pin is only a shock when the captain would not have chosen it: if
    // the control is already in Crackdown at the shock, nothing differs
    // until it leaves. The 60-day line printed below shows the long view.
    r.assert_reacted();
    eprintln!("reactions within 60 days, vs control: {:?}", r.reactions(control(), END_DAY - SHOCK_DAY));
}

/// Set the Treasury to -50,000: the dole stops and the guards go unpaid.
#[test]
#[ignore]
fn god_bankrupt_city() {
    let r = run("god_bankrupt_city", |w| w.push_command(PlayerCommand::SetTreasury(-50_000)));
    r.assert_reacted();
}

/// Can a player take the city by force with unlimited resources? Fund gang 0
/// with 100,000, kill gang 1's leader, fire the guards; report gang 0's share
/// of the Homes per day (not asserted).
#[test]
#[ignore]
fn god_city_takeover_by_force() {
    let r = run("god_city_takeover_by_force", |w| {
        let (g0, l1) = (gang(w, 0), leader(w, 1));
        w.push_command(PlayerCommand::FundGang { gang: g0, amount: 100_000 });
        if let Some(l) = l1 {
            w.push_command(PlayerCommand::KillAgent(l));
        }
        w.push_command(PlayerCommand::FireAllGuards);
    });
    let mut o = String::from("territory share of the Homes (g0 / g1), every 5 days:\n");
    for d in r.days.iter().filter(|d| d.day % 5 == 4) {
        let share: Vec<String> =
            d.territory.iter().map(|&t| format!("{:.1}%", 100.0 * t as f64 / d.homes.max(1) as f64)).collect();
        outln!(o, "  d{:<3} {} (of {} Homes)", d.day, share.join(" / "), d.homes);
    }
    eprint!("{o}");
    r.assert_reacted();
}
