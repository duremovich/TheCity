//! God scenarios (docs/VISION.md, "How we test: god scenarios"). Shock the
//! world with a god-mode actor at day 45 and read how the factions react.
//! Every test is `#[ignore]` (a 2,000-resident city for 60 days each; the
//! 13 god scenarios kept of 53 across the three files, plus M16a's four, docs/TESTING.md): run
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
const END_DAY: u64 = 60;
const BASE: (u64, u64) = (30, 45);
const REACT_DAYS: u64 = 7;

/// One finished day.
#[derive(Clone, Debug, Default)]
struct Day {
    day: u64,
    orders: Vec<Order>,
    /// Each gang's leader at the day's end (the decapitation's succession).
    leaders: Vec<Option<EntityId>>,
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
    /// M13 (docs/GOD_SCENARIOS_V4.md): printed by `print_m13`, never in
    /// `series`, so the v1 scenarios' reaction checks are unchanged.
    m13: M13Day,
}

/// The M13 readings of one day.
#[derive(Clone, Debug, Default)]
struct M13Day {
    episodes: u32,
    episodes_by_law: u32,
    therapy: u32,
    detox: u32,
    installs: u32,
    hooked: u32,
    dealt: u32,
    legal_sales: u32,
    gang_income: i64,
    gang_dealing: i64,
    harvest_orders: u32,
    raid_orders: u32,
    /// Gang members with a working fighting implant (`Kit.fighting > 0`).
    fighters: u32,
    bricked: u32,
    crashes: u32,
    crash_deaths: u32,
    manslaughter_reports: u32,
    murder_reports: u32,
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
    run_from(name, |_| {}, shock)
}

/// `run`, with `setup` fired at the start of the baseline window (`BASE.0`),
/// so the baseline and the shock's control share it.
fn run_from(name: &'static str, setup: impl FnOnce(&mut World), shock: impl FnOnce(&mut World)) -> Run {
    run_daily(name, setup, shock, |_, _| {})
}

/// `run_from`, with `daily` called at the start of every day (M13: a chase
/// pinned for a week).
fn run_daily(
    name: &'static str,
    setup: impl FnOnce(&mut World),
    shock: impl FnOnce(&mut World),
    mut daily: impl FnMut(&mut World, u64),
) -> Run {
    let mut w = World::new(SEED, Config::load());
    let mut setup = Some(setup);
    let gangs = w.gangs();
    let gang_names = gangs.iter().map(|&g| w.comp::<Gang>(g).map_or(String::new(), |g| g.name.clone())).collect();
    let mut shock = Some(shock);
    let mut days = Vec::new();
    let mut story = Vec::new();
    let mut seen = 0;
    for day in 0..END_DAY {
        if day == BASE.0 {
            (setup.take().expect("once"))(&mut w);
        }
        if day == SHOCK_DAY {
            (shock.take().expect("once"))(&mut w);
        }
        daily(&mut w, day);
        w.run_ticks(TICKS_PER_DAY);
        let mut d = Day { day, ..Day::default() };
        d.joins = vec![0; gangs.len()];
        for e in w.events.iter().filter(|e| e.tick >= seen) {
            match e.kind {
                EventKind::OrderChanged => d.order_changes += 1,
                EventKind::Posture => d.posture_changes += 1,
                EventKind::Assault | EventKind::Murder => d.violence += 1,
                EventKind::Raid if e.text.contains(" raided ") => d.raids += 1,
                EventKind::Raid if e.text.contains("stormed the Precinct") => d.storms += 1,
                EventKind::Raid => d.fizzles += 1,
                EventKind::Jailbreak => d.jailbreaks += 1,
                EventKind::Bribe => d.bribes += 1,
                EventKind::GangJoin => {
                    if let Some(i) = e.actors.get(1).and_then(|g| gangs.iter().position(|x| x == g)) {
                        d.joins[i] += 1;
                    }
                }
                EventKind::Episode if e.text.contains(" went berserk ") => d.m13.episodes += 1,
                EventKind::Treated if e.text.contains("Therapy") => d.m13.therapy += 1,
                EventKind::Treated => d.m13.detox += 1,
                EventKind::Report if e.text.ends_with(" for Manslaughter") => d.m13.manslaughter_reports += 1,
                EventKind::Report if e.text.ends_with(" for Murder") => d.m13.murder_reports += 1,
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
                    | EventKind::Split
                    | EventKind::Riot
            ) || (e.kind == EventKind::GangLeave && e.text.contains("lost its claims"));
            if told && day >= SHOCK_DAY - 2 {
                story.push((e.tick, e.kind, e.text.clone()));
            }
        }
        seen = w.tick;
        for &g in &gangs {
            let gg = w.comp::<Gang>(g).expect("gangs are never despawned");
            d.orders.push(gg.order);
            d.leaders.push(gg.leader);
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
        d.m13.episodes_by_law = row.episodes_by_law;
        d.m13.installs = row.chrome_installs;
        d.m13.hooked = row.hooked;
        d.m13.dealt = row.stims_dealt;
        d.m13.gang_income = row.gang_income;
        d.m13.gang_dealing = row.gang_income_dealing;
        d.m13.crashes = row.crashes;
        d.m13.crash_deaths = row.crash_deaths;
        d.m13.legal_sales = w
            .buildings_of_kind(citysim::BuildingKind::Market)
            .iter()
            .filter_map(|&m| w.comp::<citysim::Market>(m))
            .map(|m| m.stim_sales_today)
            .sum();
        for g in w.gangs() {
            let Some(gg) = w.comp::<Gang>(g) else { continue };
            d.m13.harvest_orders += u32::from(gg.order == Order::Harvest);
            d.m13.raid_orders += u32::from(gg.order.is_raid());
            d.m13.fighters +=
                gg.members.iter().filter(|&&m| w.comp::<citysim::Kit>(m).is_some_and(|k| k.fighting > 0.0)).count()
                    as u32;
        }
        d.m13.bricked = citysim::systems::assets::all_assets(&w)
            .into_iter()
            .filter(|&a| {
                w.comp::<citysim::Asset>(a)
                    .is_some_and(|x| x.bricked && matches!(x.loc, citysim::AssetLoc::Installed(_)))
            })
            .count() as u32;
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
        let windows = [(BASE.0, BASE.1, "base 30-45"), (45, 52, "45-52"), (52, END_DAY, "52-60")];
        let ctrl_windows = [(45, 52, "ctl 45-52"), (52, END_DAY, "ctl 52-60")];
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
        self.assert_reacted_against(control());
    }

    fn assert_reacted_against(&self, c: &Run) {
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

/// The control for a posture pin: the law pinned to Patrol from the baseline
/// window on, so pinning another posture at the shock is a change the
/// captain did not choose. At 2,000 residents the unpinned captain sat in
/// Crackdown or Garrison on most days while `crackdown_reports` was an
/// absolute count (per capita since the M11 review), so an unpinned control
/// was often already in Crackdown.
fn patrol_control() -> &'static Run {
    static CONTROL: OnceLock<Run> = OnceLock::new();
    CONTROL.get_or_init(|| run_from("god_patrol_control", pin(Posture::Patrol), |_| {}))
}

fn pin(p: Posture) -> impl FnOnce(&mut World) {
    move |w: &mut World| w.push_command(PlayerCommand::SetLawPosture(Some(p)))
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
    // M14 phase 2: the deck shop shifts seed 42's gang 0 into a Squat held
    // across day 45; a leader kill does not knock a held order, and the 7-day
    // generic window misses the succession. A decapitation's direct
    // consequences are gang-internal, so only they are asserted (14 days):
    // a succession (the leader differs from the control's on the same day,
    // after changing from the pre-shock one: the control's leaders churn
    // too, so "no change in the control" is the wrong test), a Split beyond
    // the control's, or a Retaliate or LieLow order the control never had.
    // The order difference and the generic reactions are printed only.
    let c = control();
    r.print(Some(c));
    let (from, to) = (SHOCK_DAY, SHOCK_DAY + 14);
    let before = |run: &Run| run.days.iter().find(|d| d.day == SHOCK_DAY - 1).and_then(|d| d.leaders[0]);
    let old = before(&r);
    let succession = r
        .window(from, to)
        .zip(c.window(from, to))
        .find(|(d, x)| d.leaders[0].is_some() && d.leaders[0] != old && d.leaders[0] != x.leaders[0]);
    let splits = |run: &Run| {
        run.story.iter().filter(|(t, k, _)| *k == EventKind::Split && (from..to).contains(&(t / TICKS_PER_DAY))).count()
    };
    let shaken = |run: &Run| run.window(from, to).any(|d| matches!(d.orders[0], Order::Retaliate | Order::LieLow));
    let mut fired = Vec::new();
    if let Some((d, x)) = succession {
        fired.push(format!(
            "succession on day {}: leader {:?} (before {:?}, control {:?})",
            d.day, d.leaders[0], old, x.leaders[0]
        ));
    }
    if splits(&r) > splits(c) {
        fired.push(format!("Split events {} (control {})", splits(&r), splits(c)));
    }
    if shaken(&r) && !shaken(c) {
        fired.push("a Retaliate or LieLow order".to_string());
    }
    let mut seen = Vec::new();
    if let Some(d) = r.window(from, to).zip(c.window(from, to)).find(|(d, x)| d.orders[0] != x.orders[0]) {
        seen.push(format!("g0 order on day {}: {:?} (control {:?})", d.0.day, d.0.orders[0], d.1.orders[0]));
    }
    seen.push(format!("generic: {:?}", r.reactions(c, 14)));
    eprintln!("decapitation reactions within 14 days: {fired:?}; also (not asserted): {seen:?}");
    assert!(!fired.is_empty(), "god_decapitate_gang: nothing reacted within 14 days of the shock");
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

/// Pin Crackdown. Does the target gang bribe, LieLow, break out, collapse?
/// Does the other gang profit?
#[test]
#[ignore]
fn god_crackdown_forever() {
    // A pin is only a shock when the captain would not have chosen it: an
    // unpinned control was already in Crackdown on most days (M10 phase 5c).
    // So the law holds Patrol from the baseline window on, in this run and in
    // its control, and the shock turns that Patrol into a Crackdown.
    let r = run_from("god_crackdown_forever", pin(Posture::Patrol), pin(Posture::Crackdown));
    r.assert_reacted_against(patrol_control());
    eprintln!("reactions to day {END_DAY}, vs control: {:?}", r.reactions(patrol_control(), END_DAY - SHOCK_DAY));
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

// ---------------------------------------------------------------------------
// M13 god scenarios (docs/GOD_SCENARIOS_V4.md, plan 5.4): findings printed
// against the control, never asserted; a failure to react goes into the
// docs' Gaps list.
// ---------------------------------------------------------------------------

impl Run {
    /// The M13 rows (window means, this run against the control).
    fn print_m13(&self, control: &Run) {
        let mut o = String::new();
        outln!(o, "---- M13 readings: {} ----", self.name);
        let windows = [(BASE.0, BASE.1, "base 30-45"), (45, 52, "45-52"), (52, END_DAY, "52-60")];
        out!(o, "{:<20}", "per-day mean");
        for (_, _, n) in windows {
            out!(o, "{n:>12}");
        }
        for (_, _, n) in &windows[1..] {
            out!(o, "{:>12}", format!("ctl {n}"));
        }
        outln!(o);
        let rows: Vec<Series> = vec![
            ("episodes".into(), Box::new(|d: &Day| f64::from(d.m13.episodes))),
            ("episodes by law".into(), Box::new(|d: &Day| f64::from(d.m13.episodes_by_law))),
            ("Therapy".into(), Box::new(|d: &Day| f64::from(d.m13.therapy))),
            ("Detox".into(), Box::new(|d: &Day| f64::from(d.m13.detox))),
            ("installs".into(), Box::new(|d: &Day| f64::from(d.m13.installs))),
            ("hooked".into(), Box::new(|d: &Day| f64::from(d.m13.hooked))),
            ("dealt".into(), Box::new(|d: &Day| f64::from(d.m13.dealt))),
            ("legal sales".into(), Box::new(|d: &Day| f64::from(d.m13.legal_sales))),
            ("gang income".into(), Box::new(|d: &Day| d.m13.gang_income as f64)),
            ("gang dealing".into(), Box::new(|d: &Day| d.m13.gang_dealing as f64)),
            ("Harvest orders".into(), Box::new(|d: &Day| f64::from(d.m13.harvest_orders))),
            ("raid orders".into(), Box::new(|d: &Day| f64::from(d.m13.raid_orders))),
            ("raids".into(), Box::new(|d: &Day| f64::from(d.raids))),
            ("chromed fighters".into(), Box::new(|d: &Day| f64::from(d.m13.fighters))),
            ("bricked implants".into(), Box::new(|d: &Day| f64::from(d.m13.bricked))),
            ("crashes".into(), Box::new(|d: &Day| f64::from(d.m13.crashes))),
            ("crash deaths".into(), Box::new(|d: &Day| f64::from(d.m13.crash_deaths))),
            ("Manslaughter rep.".into(), Box::new(|d: &Day| f64::from(d.m13.manslaughter_reports))),
            ("Murder reports".into(), Box::new(|d: &Day| f64::from(d.m13.murder_reports))),
            ("violence".into(), Box::new(|d: &Day| f64::from(d.violence))),
            ("posture changes".into(), Box::new(|d: &Day| f64::from(d.posture_changes))),
        ];
        let cell = |r: &Run, f: &dyn Fn(&Day) -> f64, a: u64, b: u64| {
            let (m, _) = mean_sd(&r.window(a, b).map(f).collect::<Vec<_>>());
            format!("{m:.2}")
        };
        for (name, f) in &rows {
            out!(o, "{name:<20}");
            for (a, b, _) in windows {
                out!(o, "{:>12}", cell(self, f, a, b));
            }
            for (a, b, _) in &windows[1..] {
                out!(o, "{:>12}", cell(control, f, *a, *b));
            }
            outln!(o);
        }
        outln!(o, "law: {}", self.posture_history());
        eprint!("{o}");
    }

    /// The standard table, the M13 rows and the v1 reaction list, without
    /// an assertion (plan 5.4: findings, not asserts).
    fn report_m13(&self, control: &Run) {
        self.print(Some(control));
        self.print_m13(control);
    }
}

/// Every adult gets Arms and Nerves T2. Does the episode rate rise, does
/// the captain's posture move, do the Clinics sell Therapy?
#[test]
#[ignore]
fn god_chrome_everyone_2() {
    let r = run("god_chrome_everyone_2", |w| w.push_command(PlayerCommand::ChromeEveryone { tier: 2 }));
    r.report_m13(control());
}

// ---------------------------------------------------------------------------
// M14 god scenarios (docs/GOD_SCENARIOS_V5.md, plan 5.2): the Virt plane.
// Seed 42, 60 days, each against its own unshocked control (`v5_control`).
// Findings are printed; a world that fails to react goes into V5's gaps
// list, not an assert. Each scenario asserts only that its god command
// applied (a `PlayerAction`, not a `PlayerActionFailed`), and the door
// scenario that its door runs were ordered.
// ---------------------------------------------------------------------------

/// One finished day of an M14 god run.
#[derive(Clone, Debug, Default)]
struct V5Day {
    day: u64,
    runs: u32,
    runs_ok: u32,
    stolen: u32,
    sold: u32,
    wiped: u32,
    fried: u32,
    flatlined: u32,
    traced: u32,
    ice_raised: u32,
    ice_lowered: u32,
    /// Ledger hacks on the Treasury, and the coins they took.
    treasury_hits: u32,
    treasury_taken: i64,
    city_treasury: i64,
    /// Thefts (Data or Ledger) on a named corp's nodes, keyed by corp name.
    hits_on: std::collections::BTreeMap<String, u32>,
    /// Living corps: treasury and `[chrome, deck, industry]` tiers, by name.
    corps: std::collections::BTreeMap<String, (i64, [u8; 3])>,
    /// Corps under `Research` at the day's end.
    researching: Vec<String>,
    /// Implant installs at tier 2 and tier 3 at a Clinic the named corp owns, by name.
    implants_t23: std::collections::BTreeMap<String, u32>,
    /// Corp-building raids won and lost (`raid::corp_raid`'s event).
    corp_raids_won: u32,
    corp_raids_lost: u32,
}

struct V5Run {
    name: &'static str,
    days: Vec<V5Day>,
    /// `(tick, kind, text)` of the Virt, tech and raid story.
    story: Vec<(u64, EventKind, String)>,
    /// God commands that failed.
    failed: Vec<String>,
}

const V5_END: u64 = 60;

/// A named per-day reading of an M14 god run.
type V5Series = (String, Box<dyn Fn(&V5Day) -> f64>);

/// The corp named `name`, if alive.
fn corp_named(w: &World, name: &str) -> Option<EntityId> {
    w.corps().into_iter().find(|&c| w.comp::<citysim::Corp>(c).is_some_and(|cc| cc.name == name))
}

/// Seed 42 to `V5_END`, `hourly` called before every game hour (day, hour).
fn run_v5(name: &'static str, mut hourly: impl FnMut(&mut World, u64, u64)) -> V5Run {
    use citysim::TICKS_PER_HOUR;
    let mut w = World::new(SEED, Config::load());
    let mut days = Vec::new();
    let mut story = Vec::new();
    let mut failed = Vec::new();
    let mut next_id = 0u64;
    for day in 0..V5_END {
        let mut d = V5Day { day, ..V5Day::default() };
        for hour in 0..24 {
            hourly(&mut w, day, hour);
            w.run_ticks(TICKS_PER_HOUR);
            let fresh: Vec<&citysim::Event> = w.events.iter().rev().take_while(|e| e.id >= next_id).collect();
            for e in fresh.into_iter().rev() {
                let corp_of =
                    |x: Option<&EntityId>| x.and_then(|&c| w.comp::<citysim::Corp>(c)).map(|cc| cc.name.clone());
                match e.kind {
                    EventKind::LedgerHacked => {
                        if e.actors.get(1).is_some_and(|&c| c == EntityId::NONE) {
                            d.treasury_hits += 1;
                            d.treasury_taken += e
                                .text
                                .split(" took ")
                                .nth(1)
                                .and_then(|t| t.split(' ').next())
                                .and_then(|n| n.parse::<i64>().ok())
                                .unwrap_or(0);
                        } else if let Some(n) = corp_of(e.actors.get(1)) {
                            *d.hits_on.entry(n).or_default() += 1;
                        }
                    }
                    EventKind::DataStolen => {
                        // `[runner, owner, …]`: the owner's name when a corp.
                        if let Some(n) = corp_of(e.actors.get(1)) {
                            *d.hits_on.entry(n).or_default() += 1;
                        }
                    }
                    EventKind::Installed if e.text.contains(" T2 ") || e.text.contains(" T3 ") => {
                        // `[agent, clinic, implant]` (`chrome::install`).
                        let clinic = (e.actors.len() == 3).then(|| e.actors[1]);
                        if let Some(n) = clinic.and_then(|b| w.owner_of(b)).and_then(|o| corp_of(Some(&o))) {
                            *d.implants_t23.entry(n).or_default() += 1;
                        }
                    }
                    EventKind::Raid if e.text.contains(" raided ") && e.text.contains("'s ") => {
                        if e.text.contains(": Won") || e.text.contains(": Sacked") {
                            d.corp_raids_won += 1;
                        } else {
                            d.corp_raids_lost += 1;
                        }
                    }
                    EventKind::PlayerActionFailed => failed.push(format!("d{day} {}", e.text)),
                    _ => {}
                }
                let told = matches!(
                    e.kind,
                    EventKind::TechLost
                        | EventKind::TechGained
                        | EventKind::DataWiped
                        | EventKind::LedgerHacked
                        | EventKind::DoorHacked
                        | EventKind::RobotTurned
                        | EventKind::Flatlined
                        | EventKind::IceRaised
                        | EventKind::IceLowered
                        | EventKind::Bankrupt
                        | EventKind::PlayerAction
                        | EventKind::PlayerActionFailed
                ) || (e.kind == EventKind::CorpOrder && e.text.contains("Research"))
                    || (e.kind == EventKind::Raid && (e.text.contains("'s ") || e.text.contains("open doors")))
                    || (e.kind == EventKind::JackedIn && e.text.contains("(a door on "));
                if told {
                    story.push((e.tick, e.kind, e.text.clone()));
                }
            }
            next_id = w.events.back().map_or(next_id, |e| e.id + 1);
        }
        let row = w.stats.history.back().expect("a finished day").clone();
        let v = &row.virt;
        d.runs = v.runs;
        d.runs_ok = v.runs_ok;
        d.stolen = v.data_stolen;
        d.sold = v.data_sold;
        d.wiped = v.data_wiped;
        d.fried = v.fried;
        d.flatlined = v.flatlined;
        d.traced = v.traced;
        d.ice_raised = v.ice_raised;
        d.ice_lowered = v.ice_lowered;
        d.city_treasury = row.treasury;
        for c in w.corps() {
            let Some(cc) = w.comp::<citysim::Corp>(c) else { continue };
            d.corps.insert(cc.name.clone(), (w.purse(Some(c)), cc.tech.tier));
            if cc.order == citysim::CorpOrder::Research {
                d.researching.push(cc.name.clone());
            }
        }
        days.push(d);
    }
    V5Run { name, days, story, failed }
}

/// The unshocked M14 control, computed once per test process.
fn v5_control() -> &'static V5Run {
    static CONTROL: OnceLock<V5Run> = OnceLock::new();
    CONTROL.get_or_init(|| run_v5("v5_control", |_, _, _| {}))
}

impl V5Run {
    fn window(&self, from: u64, to: u64) -> impl Iterator<Item = &V5Day> {
        self.days.iter().filter(move |d| d.day >= from && d.day < to)
    }

    fn sum(&self, from: u64, to: u64, f: impl Fn(&V5Day) -> f64) -> f64 {
        self.window(from, to).map(f).sum()
    }

    /// The day a corp's row is last seen (its bankruptcy or dissolution), `None` while alive at the end.
    fn gone(&self, corp: &str) -> Option<u64> {
        let last = self.days.iter().rev().find(|d| d.corps.contains_key(corp))?;
        (last.day + 1 < V5_END).then_some(last.day + 1)
    }

    fn tiers(&self, corp: &str, day: u64) -> Option<[u8; 3]> {
        self.days.get(day as usize).and_then(|d| d.corps.get(corp)).map(|&(_, t)| t)
    }

    /// The side-by-side table against the control over `windows`, then the story from `from`.
    fn print_v5(&self, control: &V5Run, windows: &[(u64, u64)], from: u64, extra: &[V5Series]) {
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        out!(o, "{:<22}", "sum over days");
        for (a, b) in windows {
            out!(o, "{:>16}", format!("{a}-{b} / ctl"));
        }
        outln!(o);
        let rows: Vec<V5Series> = vec![
            ("runs".into(), Box::new(|d: &V5Day| f64::from(d.runs))),
            ("runs ok".into(), Box::new(|d: &V5Day| f64::from(d.runs_ok))),
            ("Data stolen".into(), Box::new(|d: &V5Day| f64::from(d.stolen))),
            ("Data sold".into(), Box::new(|d: &V5Day| f64::from(d.sold))),
            ("Data wiped".into(), Box::new(|d: &V5Day| f64::from(d.wiped))),
            ("fried".into(), Box::new(|d: &V5Day| f64::from(d.fried))),
            ("flatlined".into(), Box::new(|d: &V5Day| f64::from(d.flatlined))),
            ("traced".into(), Box::new(|d: &V5Day| f64::from(d.traced))),
            ("IceRaised".into(), Box::new(|d: &V5Day| f64::from(d.ice_raised))),
            ("IceLowered".into(), Box::new(|d: &V5Day| f64::from(d.ice_lowered))),
            ("Treasury hits".into(), Box::new(|d: &V5Day| f64::from(d.treasury_hits))),
            ("Treasury taken".into(), Box::new(|d: &V5Day| d.treasury_taken as f64)),
            ("corp raids won".into(), Box::new(|d: &V5Day| f64::from(d.corp_raids_won))),
            ("corp raids lost".into(), Box::new(|d: &V5Day| f64::from(d.corp_raids_lost))),
        ];
        for (name, f) in rows.iter().chain(extra) {
            out!(o, "{name:<22}");
            for &(a, b) in windows {
                out!(o, "{:>16}", format!("{:.0} / {:.0}", self.sum(a, b, f), control.sum(a, b, f)));
            }
            outln!(o);
        }
        for d in self.days.iter().filter(|d| d.day % 10 == 9) {
            let c = control.days.get(d.day as usize);
            let fmt = |x: &std::collections::BTreeMap<String, (i64, [u8; 3])>| {
                x.iter()
                    .map(|(n, (t, tier))| format!("{n} {t} [{}{}{}]", tier[0], tier[1], tier[2]))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            outln!(o, "  d{:<3} city {:>7} | {}", d.day, d.city_treasury, fmt(&d.corps));
            if let Some(c) = c {
                outln!(o, "   ctl city {:>7} | {}", c.city_treasury, fmt(&c.corps));
            }
        }
        outln!(o, "story from day {from}:");
        let told: Vec<_> = self.story.iter().filter(|(t, _, _)| t / TICKS_PER_DAY >= from).collect();
        for (t, k, text) in told.iter().take(80) {
            outln!(o, "  d{:<3} {k:?}: {text}", t / TICKS_PER_DAY);
        }
        if told.len() > 80 {
            outln!(o, "  ... {} more", told.len() - 80);
        }
        if !self.failed.is_empty() {
            outln!(o, "failed god commands: {:?}", self.failed);
        }
        eprint!("{o}");
    }

    fn assert_applied(&self) {
        assert!(self.failed.is_empty(), "{}: a god command failed: {:?}", self.name, self.failed);
        assert!(
            self.story.iter().any(|(_, k, t)| *k == EventKind::PlayerAction && t.starts_with("God: ")
                || *k == EventKind::PlayerAction && t.starts_with("City ICE")),
            "{}: no god command applied",
            self.name
        );
    }
}

/// Zetatech's Data wiped on day 20 (every node it owns). Does it lose
/// Chrome 3 and Industry 3, do its Clinics' tier-2/3 installs stop, does a
/// rival take `Research`?
#[test]
#[ignore]
fn god_wipe_zetatech_day_20() {
    const DAY: u64 = 20;
    let r = run_v5("god_wipe_zetatech_day_20", |w, day, hour| {
        if day == DAY && hour == 0 {
            match corp_named(w, "Zetatech") {
                Some(z) => w.push_command(PlayerCommand::WipeCorpData(z)),
                None => eprintln!("no Zetatech on day {DAY}"),
            }
        }
    });
    let c = v5_control();
    let implants = |name: &'static str| -> V5Series {
        (
            format!("{name} T2/T3 installs"),
            Box::new(move |d: &V5Day| f64::from(d.implants_t23.get(name).copied().unwrap_or(0))),
        )
    };
    let research = |d: &V5Day| d.researching.len() as f64;
    r.print_v5(
        c,
        &[(0, DAY), (DAY, DAY + 7), (DAY, DAY + 30), (DAY + 30, V5_END)],
        DAY,
        &[implants("Zetatech"), ("corps in Research".into(), Box::new(research))],
    );
    let t = |run: &V5Run, d: u64| run.tiers("Zetatech", d);
    eprintln!(
        "Zetatech tiers [chrome deck industry]: day {} {:?} (control {:?}), day {} {:?} (control {:?}); gone on day {:?} (control {:?})",
        DAY + 1,
        t(&r, DAY + 1),
        t(c, DAY + 1),
        DAY + 30,
        t(&r, DAY + 30),
        t(c, DAY + 30),
        r.gone("Zetatech"),
        c.gone("Zetatech"),
    );
    let rivals: std::collections::BTreeSet<&String> =
        r.window(DAY, V5_END).flat_map(|d| d.researching.iter()).collect();
    eprintln!("corps that took Research after the wipe: {rivals:?}");
    r.assert_applied();
}

// ---------------------------------------------------------------------------
// M15 god scenarios (docs/GOD_SCENARIOS_V6.md, plan 5.2): the word and the
// blood. Seed 42, 60 days, each against its own unshocked control
// (`v6_control`). Findings are printed; a world that fails to react goes into
// V6's gaps list, not an assert. Each scenario asserts only that its god
// commands applied (a `PlayerAction`, not a `PlayerActionFailed`).
// ---------------------------------------------------------------------------

/// One finished day of an M15 god run.
#[derive(Clone, Debug, Default)]
struct V6Day {
    day: u64,
    /// The day's M15 counters (the CSV row's word columns).
    word: citysim::stats::WordCols,
    violence: u32,
    murders: u32,
    /// Per gang `(name, order, Retaliate's target, dread, heat, members)`.
    gangs: Vec<(String, Order, Option<String>, f32, f32, usize)>,
    posture: Option<Posture>,
    /// The Crackdown's gang and a Lobby hold `(corp, gang)`.
    crackdown: Option<String>,
    lobby: Option<(String, String)>,
    /// Living corps: `(honour, dread, employees' mean opinion of it, contracts sold)` by name.
    corps: std::collections::BTreeMap<String, (f32, f32, f32, usize)>,
    vendettas: Vec<String>,
    /// Adults whose Home is in a Sump district: how many, how many with visible chrome, mean `Kit.visible`.
    sump: (u32, u32, f32),
    /// The tracked agents (a scenario's subjects): `(employed, Corp-class edges, standing)`, by agent.
    tracked: std::collections::BTreeMap<EntityId, (bool, u32, f32)>,
}

struct V6Run {
    name: &'static str,
    days: Vec<V6Day>,
    /// `(tick, kind, text)` of the word and blood story.
    story: Vec<(u64, EventKind, String)>,
    failed: Vec<String>,
}

const V6_END: u64 = 60;

/// A named per-day reading of an M15 god run.
type V6Series = (&'static str, Box<dyn Fn(&V6Day) -> f64>);

/// Seed 42 to `V6_END`; `daily` fires at the start of every day and returns
/// the agents to track from then on (an empty list keeps the old ones).
fn run_v6(name: &'static str, mut daily: impl FnMut(&mut World, u64) -> Vec<EntityId>) -> V6Run {
    use citysim::systems::reputation;
    let mut w = World::new(SEED, Config::load());
    let mut days = Vec::new();
    let mut story = Vec::new();
    let mut failed = Vec::new();
    let mut tracked: Vec<EntityId> = Vec::new();
    let mut next_id = 0u64;
    for day in 0..V6_END {
        let fresh = daily(&mut w, day);
        if !fresh.is_empty() {
            tracked = fresh;
        }
        w.run_ticks(TICKS_PER_DAY);
        let mut d = V6Day { day, ..V6Day::default() };
        let events: Vec<&citysim::Event> = w.events.iter().rev().take_while(|e| e.id >= next_id).collect();
        for e in events.into_iter().rev() {
            match e.kind {
                EventKind::Assault => d.violence += 1,
                EventKind::Murder => {
                    d.violence += 1;
                    d.murders += 1;
                }
                EventKind::PlayerActionFailed => failed.push(format!("d{day} {}", e.text)),
                _ => {}
            }
            let told = matches!(
                e.kind,
                EventKind::Vendetta
                    | EventKind::VendettaEnded
                    | EventKind::HuntStarted
                    | EventKind::HuntAbandoned
                    | EventKind::Avenged
                    | EventKind::Planted
                    | EventKind::Buried
                    | EventKind::Poached
                    | EventKind::Expelled
                    | EventKind::ContractLost
                    | EventKind::PlayerAction
                    | EventKind::PlayerActionFailed
                    | EventKind::Posture
                    | EventKind::Bribe
            ) || (e.kind == EventKind::OrderChanged && e.text.contains("Retaliate"))
                || (e.kind == EventKind::CorpOrder && e.text.contains("Lobby"))
                || (e.kind == EventKind::Raid && e.text.contains("'s "))
                || (e.kind == EventKind::Story && (e.text.contains("(paid by") || e.text.contains(" killed ")));
            if told {
                story.push((e.tick, e.kind, e.text.clone()));
            }
        }
        next_id = w.events.back().map_or(next_id, |e| e.id + 1);
        let row = w.stats.history.back().expect("a finished day").clone();
        d.word = row.word.clone();
        for g in w.gangs() {
            let Some(gg) = w.comp::<Gang>(g) else { continue };
            let r = reputation::rep(&w, g);
            let on = gg.retaliate_on.map(|t| citysim::systems::grudges::label(&w, t));
            d.gangs.push((gg.name.clone(), gg.order, on, r.dread, r.heat, gg.members.len()));
        }
        if let Some(l) = w.law() {
            d.posture = Some(l.posture);
            d.crackdown = l.target.map(|g| citysim::systems::grudges::label(&w, g));
            d.lobby = l.lobby.map(|h| (w.owner_label(Some(h.corp)), citysim::systems::grudges::label(&w, h.gang)));
        }
        for c in w.corps() {
            let Some(cc) = w.comp::<citysim::Corp>(c) else { continue };
            let r = reputation::rep(&w, c);
            let staff = reputation::members_of(&w, c);
            let op = if staff.is_empty() {
                0.0
            } else {
                staff.iter().map(|&m| reputation::opinion(&w, m, c)).sum::<f32>() / staff.len() as f32
            };
            d.corps.insert(cc.name.clone(), (r.honour, r.dread, op, cc.contracts.len()));
        }
        d.vendettas = w
            .vendettas
            .iter()
            .map(|v| {
                format!("{}-{}", citysim::systems::grudges::label(&w, v.a), citysim::systems::grudges::label(&w, v.b))
            })
            .collect();
        let (mut n, mut chromed, mut vis) = (0u32, 0u32, 0u32);
        for a in w.citizens() {
            if !citysim::systems::demography::is_adult(&w, a) || !citysim::systems::law::living(&w, a) {
                continue;
            }
            let Some(home) = w.comp::<citysim::Household>(a).and_then(|h| h.home) else { continue };
            if !w.district_name(w.district_of_building(home)).contains("Sump") {
                continue;
            }
            let v = w.comp::<citysim::Kit>(a).map_or(0, |k| k.visible);
            n += 1;
            chromed += u32::from(v > 0);
            vis += u32::from(v);
        }
        d.sump = (n, chromed, vis as f32 / n.max(1) as f32);
        let execs = citysim::systems::classes::exec_set(&w);
        for &a in &tracked {
            let employed = w.has::<citysim::Job>(a) && citysim::systems::law::living(&w, a);
            let corp_edges = w
                .neighbours(a)
                .filter(|&o| citysim::systems::classes::class_in(&w, o, &execs) == citysim::components::Class::Corp)
                .count() as u32;
            d.tracked.insert(a, (employed, corp_edges, reputation::rep(&w, a).standing));
        }
        days.push(d);
    }
    V6Run { name, days, story, failed }
}

/// The unshocked M15 control, computed once per test process.
fn v6_control() -> &'static V6Run {
    static CONTROL: OnceLock<V6Run> = OnceLock::new();
    CONTROL.get_or_init(|| run_v6("v6_control", |_, _| Vec::new()))
}

impl V6Run {
    fn window(&self, from: u64, to: u64) -> impl Iterator<Item = &V6Day> {
        self.days.iter().filter(move |d| d.day >= from && d.day < to)
    }

    fn sum(&self, from: u64, to: u64, f: &dyn Fn(&V6Day) -> f64) -> f64 {
        self.window(from, to).map(f).sum()
    }

    /// The word table against the control over `windows`, the gangs and the law every ten days, then the
    /// story from `from`.
    fn print_v6(&self, control: &V6Run, windows: &[(u64, u64)], from: u64) {
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        out!(o, "{:<22}", "sum over days");
        for (a, b) in windows {
            out!(o, "{:>16}", format!("{a}-{b} / ctl"));
        }
        outln!(o);
        let rows: Vec<V6Series> = vec![
            ("violence", Box::new(|d: &V6Day| f64::from(d.violence))),
            ("Murders", Box::new(|d: &V6Day| f64::from(d.murders))),
            ("grudges formed", Box::new(|d: &V6Day| f64::from(d.word.grudges))),
            ("Hunts", Box::new(|d: &V6Day| f64::from(d.word.hunts))),
            ("Avenged", Box::new(|d: &V6Day| f64::from(d.word.avenged))),
            ("revenge kills", Box::new(|d: &V6Day| f64::from(d.word.revenge_kills))),
            ("stories", Box::new(|d: &V6Day| f64::from(d.word.stories))),
            ("Planted", Box::new(|d: &V6Day| f64::from(d.word.planted))),
            ("Buried", Box::new(|d: &V6Day| f64::from(d.word.buried))),
            ("Poached", Box::new(|d: &V6Day| f64::from(d.word.poached))),
            ("Expelled", Box::new(|d: &V6Day| f64::from(d.word.expelled))),
            ("contracts lost (hon)", Box::new(|d: &V6Day| f64::from(d.word.contracts_lost_honour))),
        ];
        for (name, f) in &rows {
            out!(o, "{name:<22}");
            for &(a, b) in windows {
                out!(o, "{:>16}", format!("{:.0} / {:.0}", self.sum(a, b, f), control.sum(a, b, f)));
            }
            outln!(o);
        }
        for d in self.days.iter().filter(|d| d.day >= from.saturating_sub(1) && d.day % 10 == 9) {
            let c = control.days.get(d.day as usize);
            let gangs = |x: &V6Day| {
                x.gangs
                    .iter()
                    .map(|g| {
                        let on =
                            g.2.as_ref()
                                .filter(|_| g.1 == Order::Retaliate)
                                .map_or(String::new(), |t| format!(" on {t}"));
                        format!("{} {:?}{on} d{:.2} h{:.2} n{}", g.0, g.1, g.3, g.4, g.5)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let law = |x: &V6Day| format!("{:?} crackdown {:?} lobby {:?}", x.posture, x.crackdown, x.lobby);
            outln!(o, "  d{:<3} {} | {} | vendettas {:?}", d.day, gangs(d), law(d), d.vendettas);
            if let Some(c) = c {
                outln!(o, "   ctl {} | {} | vendettas {:?}", gangs(c), law(c), c.vendettas);
            }
        }
        outln!(o, "story from day {from}:");
        let told: Vec<_> = self.story.iter().filter(|(t, _, _)| t / TICKS_PER_DAY >= from).collect();
        for (t, k, text) in told.iter().take(80) {
            outln!(o, "  d{:<3} {k:?}: {text}", t / TICKS_PER_DAY);
        }
        if told.len() > 80 {
            outln!(o, "  ... {} more", told.len() - 80);
        }
        if !self.failed.is_empty() {
            outln!(o, "failed god commands: {:?}", self.failed);
        }
        eprint!("{o}");
    }

    fn assert_applied(&self, n: usize) {
        assert!(self.failed.is_empty(), "{}: a god command failed: {:?}", self.name, self.failed);
        let applied =
            self.story.iter().filter(|(_, k, t)| *k == EventKind::PlayerAction && t.starts_with("God: ")).count();
        assert!(applied >= n, "{}: {applied} god commands applied, {n} expected", self.name);
    }
}

/// The CLI's `member<g>`: the gang's living member with the best fighting (ties the lower id).
fn best_fighter(w: &World, g: EntityId) -> Option<EntityId> {
    w.comp::<Gang>(g)?
        .members
        .iter()
        .copied()
        .filter(|&m| citysim::systems::law::living(w, m))
        .map(|m| (citysim::systems::law::fighting(w, m), m))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, m)| m)
}

/// `kill_friend=leader0:member1` and `leader1:member0` on day 10: each gang leader's closest Friend is
/// killed by the rival's best fighter. Do they hunt, does a vendetta open between the gangs, does the
/// law's Crackdown move to the avenger's gang?
#[test]
#[ignore]
fn god_kill_friend_of_leaders_day_10() {
    const DAY: u64 = 10;
    let mut names: Vec<String> = Vec::new();
    let mut leaders: Vec<EntityId> = Vec::new();
    // `(leader, killer)` names, for the Hunt texts ("X went looking for Y").
    let mut who: Vec<(String, String)> = Vec::new();
    let r = run_v6("god_kill_friend_of_leaders_day_10", |w, day| {
        if day != DAY {
            return Vec::new();
        }
        let gs = w.gangs();
        for (a, b) in [(0usize, 1usize), (1, 0)] {
            let (Some(&ga), Some(&gb)) = (gs.get(a), gs.get(b)) else { continue };
            // L2 phase 5: the leader if it has a living Friend, else the gang's highest-ranked member with
            // one (gang turnover changed who is friends with whom by day 10); none: the command is skipped.
            let has_friend = |w: &World, x: EntityId| {
                w.neighbours(x).any(|o| {
                    citysim::systems::law::living(w, o)
                        && w.edge(x, o).is_some_and(|e| e.kind == citysim::components::RelKind::Friend)
                })
            };
            let leader = w.comp::<Gang>(ga).and_then(|g| g.leader);
            let leader = leader
                .filter(|&l| has_friend(w, l))
                .or_else(|| citysim::systems::gang::leader_ranking(w, ga).into_iter().find(|&m| has_friend(w, m)));
            match (leader, best_fighter(w, gb)) {
                (Some(of), Some(by)) => {
                    names.push(format!(
                        "{} (leader of {}) loses a Friend to {} of {}",
                        w.name_of(of),
                        w.comp::<Gang>(ga).map_or(String::new(), |g| g.name.clone()),
                        w.name_of(by),
                        w.comp::<Gang>(gb).map_or(String::new(), |g| g.name.clone())
                    ));
                    who.push((w.name_of(of), w.name_of(by)));
                    leaders.push(of);
                    w.push_command(PlayerCommand::KillFriend { of, by });
                }
                _ => names.push(format!("gang {a} has no leader or gang {b} no member on day {DAY}")),
            }
        }
        leaders.clone()
    });
    let c = v6_control();
    r.print_v6(c, &[(0, DAY), (DAY, DAY + 7), (DAY, DAY + 30), (DAY + 30, V6_END)], DAY);
    eprintln!("god_kill_friend_of_leaders_day_10: {names:?}");
    let hunts: Vec<String> = r
        .story
        .iter()
        .filter(|(t, k, x)| {
            *k == EventKind::HuntStarted
                && t / TICKS_PER_DAY >= DAY
                && who.iter().any(|(l, by)| x.starts_with(l.as_str()) || x.ends_with(&format!(" for {by}")))
        })
        .map(|(t, _, x)| format!("d{} {x}", t / TICKS_PER_DAY))
        .collect();
    eprintln!("Hunts by the two leaders or on the two killers: {hunts:?}");
    let opened: Vec<String> = r
        .story
        .iter()
        .filter(|(t, k, _)| *k == EventKind::Vendetta && t / TICKS_PER_DAY >= DAY)
        .map(|(t, _, x)| format!("d{} {x}", t / TICKS_PER_DAY))
        .collect();
    let opened_c = c.story.iter().filter(|(t, k, _)| *k == EventKind::Vendetta && t / TICKS_PER_DAY >= DAY).count();
    eprintln!("vendettas opened from day {DAY}: {opened:?} (control {opened_c})");
    let crack = |run: &V6Run| -> Vec<(u64, Option<String>)> {
        let mut v: Vec<(u64, Option<String>)> = Vec::new();
        for d in run.window(DAY, V6_END) {
            if v.last().is_none_or(|x| x.1 != d.crackdown) {
                v.push((d.day, d.crackdown.clone()));
            }
        }
        v
    };
    eprintln!("Crackdown target from day {DAY}: {:?} (control {:?})", crack(&r), crack(c));
    r.assert_applied(names.iter().filter(|n| n.contains(" loses a Friend ")).count());
}

// ---------------------------------------------------------------------------
// God scenarios v7: the living city (Life pass L2 phase 5, docs/LIFE_L2.md
// § 5, docs/GOD_SCENARIOS_V7.md). Every wage, bet, killing and arrest is a
// seeded dice roll over structs; the tests assert only that the commands
// applied and print what the city did.
// ---------------------------------------------------------------------------

const V7_END: u64 = 60;

/// One finished day of an L2 god run.
#[derive(Clone)]
struct V7Day {
    row: citysim::DayRow,
    /// Living corps: name -> (treasury, order).
    corps: std::collections::BTreeMap<String, (i64, citysim::CorpOrder)>,
    /// Faction holes opened today in the watched district: (faction label or "-", source).
    holes: Vec<(String, String)>,
    /// The watched district's adults (by Home): mean fun, and visits to its venues today.
    fun: f32,
    venue_visits: u32,
    /// Fabbers employed, Parts in Fab stock.
    fabbers: u32,
    fab_stock: u32,
}

struct V7Run {
    name: &'static str,
    days: Vec<V7Day>,
    /// `(day, kind, text)` of the L2 story (God actions, Founded/Refit, Exported, WorksPosted, CorpOrder).
    story: Vec<(u64, EventKind, String)>,
    failed: Vec<String>,
}

/// Seed 42 to `V7_END`; `daily` fires at the start of every day and may
/// set the watched district.
fn run_v7(name: &'static str, mut daily: impl FnMut(&mut World, u64, &mut Option<citysim::DistrictId>)) -> V7Run {
    use citysim::{Building, Household, Needs};
    let mut w = World::new(SEED, Config::load());
    let mut days = Vec::new();
    let mut story = Vec::new();
    let mut failed = Vec::new();
    let mut next_id = 0u64;
    let mut watch: Option<citysim::DistrictId> = None;
    let mut seen_holes: std::collections::BTreeSet<citysim::HoleId> = Default::default();
    for day in 0..V7_END {
        daily(&mut w, day, &mut watch);
        w.run_ticks(TICKS_PER_DAY);
        let mut d = V7Day {
            row: w.stats.history.back().expect("a finished day").clone(),
            corps: Default::default(),
            holes: Vec::new(),
            fun: 0.0,
            venue_visits: 0,
            fabbers: 0,
            fab_stock: 0,
        };
        let events: Vec<&citysim::Event> = w.events.iter().rev().take_while(|e| e.id >= next_id).collect();
        for e in events.into_iter().rev() {
            match e.kind {
                EventKind::PlayerActionFailed => failed.push(format!("d{day} {}", e.text)),
                EventKind::PlayerAction
                | EventKind::Founded
                | EventKind::Refit
                | EventKind::Exported
                | EventKind::WorksPosted => story.push((day, e.kind, e.text.clone())),
                EventKind::CorpOrder if e.text.contains("Grow") => story.push((day, e.kind, e.text.clone())),
                _ => {}
            }
        }
        next_id = w.events.back().map_or(next_id, |e| e.id + 1);
        for c in w.corps() {
            if let Some(cc) = w.comp::<citysim::Corp>(c) {
                d.corps.insert(cc.name.clone(), (cc.treasury, cc.order));
            }
        }
        let fresh: Vec<citysim::HoleId> = w.holes.keys().copied().filter(|h| !seen_holes.contains(h)).collect();
        for h in fresh {
            seen_holes.insert(h);
            let Some(x) = w.holes.get(&h) else { continue };
            if let Some(src) = x.source.filter(|_| Some(x.district) == watch) {
                let f = x.faction.map_or("-".to_string(), |f| citysim::systems::grudges::label(&w, f));
                d.holes.push((f, format!("{src:?} {:?}", x.kind)));
            }
        }
        if let Some(dd) = watch {
            let (mut n, mut fun) = (0u32, 0.0f32);
            for a in w.citizens() {
                let home = w.comp::<Household>(a).and_then(|h| h.home);
                if home.is_some_and(|h| w.district_of_building(h) == dd)
                    && citysim::systems::demography::is_adult(&w, a)
                {
                    n += 1;
                    fun += w.comp::<Needs>(a).map_or(0.0, |x| x.fun);
                }
            }
            d.fun = fun / n.max(1) as f32;
            for k in citysim::BuildingKind::LEISURE {
                for &b in w.buildings_of_kind(k) {
                    if w.district_of_building(b) == dd {
                        d.venue_visits += w
                            .comp::<Building>(b)
                            .and_then(|x| x.venue.as_ref())
                            .map_or(0, |v| u32::from(v.visits_today));
                    }
                }
            }
        }
        d.fabbers = w
            .with::<citysim::Job>()
            .iter()
            .filter(|&&a| w.comp::<citysim::Job>(a).is_some_and(|j| j.role == citysim::Role::Fabber))
            .count() as u32;
        d.fab_stock =
            w.buildings_of_kind(citysim::BuildingKind::Fab).iter().map(|&f| w.stock(f, citysim::Good::Parts)).sum();
        days.push(d);
    }
    V7Run { name, days, story, failed }
}

/// An unshocked L2 control run, recomputed by each scenario that calls it
/// (it watches the district the scenario names).
fn v7_control(watch_name: &'static str) -> V7Run {
    run_v7("v7_control", |w, day, watch| {
        if day == 0 {
            *watch = district_named(w, watch_name);
        }
    })
}

fn district_named(w: &World, name: &str) -> Option<citysim::DistrictId> {
    (0..w.districts.len()).map(|i| citysim::DistrictId(i as u8)).find(|&d| w.district_name(d) == name)
}

/// A named per-day reading of an L2 god run.
type V7Series = (&'static str, Box<dyn Fn(&V7Day) -> f64>);

impl V7Run {
    fn sum(&self, from: u64, to: u64, f: &dyn Fn(&V7Day) -> f64) -> f64 {
        self.days.iter().skip(from as usize).take((to - from) as usize).map(f).sum()
    }

    fn print_v7(&self, control: &V7Run, windows: &[(u64, u64)], rows: Vec<V7Series>) {
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        out!(o, "{:<26}", "sum over days");
        for (a, b) in windows {
            out!(o, "{:>20}", format!("{a}-{b} / ctl"));
        }
        outln!(o);
        for (name, f) in &rows {
            out!(o, "{name:<26}");
            for &(a, b) in windows {
                // A "(mean)" row is the window's daily mean, the rest sums.
                let n = if name.contains("(mean)") { (b - a) as f64 } else { 1.0 };
                out!(o, "{:>20}", format!("{:.0} / {:.0}", self.sum(a, b, f) / n, control.sum(a, b, f) / n));
            }
            outln!(o);
        }
        let from = windows.first().map_or(0, |w| w.0);
        outln!(o, "story from day {from}:");
        for (d, k, t) in self.story.iter().filter(|(d, k, _)| *k != EventKind::WorksPosted && *d >= from).take(40) {
            outln!(o, "  d{d:<3} {k:?}: {t}");
        }
        if !self.failed.is_empty() {
            outln!(o, "failed god commands: {:?}", self.failed);
        }
        eprint!("{o}");
    }

    fn assert_applied(&self, n: usize) {
        assert!(self.failed.is_empty(), "{}: a god command failed: {:?}", self.name, self.failed);
        let applied = self.story.iter().filter(|(_, k, _)| *k == EventKind::PlayerAction).count();
        assert!(applied >= n, "{}: {applied} god commands applied, {n} expected", self.name);
    }
}

/// L2 god 5: the export open at ten times the price on day 20
/// (`export=on`, `export_price=food:30`, `parts:180`, `data:400`): do the
/// Food and Tech corps Grow?
#[test]
#[ignore]
fn god_export_10x() {
    use citysim::outside::ExportGood;
    let r = run_v7("god_export_10x", |w, day, _| {
        if day == 20 {
            w.push_command(PlayerCommand::SetExport(true));
            for (good, price) in [(ExportGood::Food, 30), (ExportGood::Parts, 180), (ExportGood::Data, 400)] {
                w.push_command(PlayerCommand::SetExportPrice { good, price });
            }
        }
    });
    let c = v7_control("Mid West");
    let rows: Vec<V7Series> = vec![
        ("flow_export", Box::new(|d: &V7Day| d.row.living.flow_export as f64)),
        ("outside inbound (end)", Box::new(|d: &V7Day| d.row.living.outside_inbound as f64 / 1000.0)),
        ("foundings", Box::new(|d: &V7Day| f64::from(d.row.foundings))),
        ("employed (mean)", Box::new(|d: &V7Day| f64::from(d.row.employed))),
        ("food price (mean)", Box::new(|d: &V7Day| d.row.price as f64)),
        ("starvation", Box::new(|d: &V7Day| f64::from(d.row.deaths_starvation))),
        ("thefts", Box::new(|d: &V7Day| f64::from(d.row.thefts))),
    ];
    r.print_v7(&c, &[(10, 20), (20, 50), (50, V7_END)], rows);
    let mut o = String::new();
    outln!(o, "corp treasury / order on days 20, 50, {} (control)", V7_END - 1);
    for name in r.days[19].corps.keys() {
        let at = |run: &V7Run, d: usize| {
            run.days.get(d).and_then(|x| x.corps.get(name)).map_or("gone".to_string(), |v| format!("{} {:?}", v.0, v.1))
        };
        outln!(
            o,
            "  {name:<12} {} ({}) | {} ({}) | {} ({})",
            at(&r, 19),
            at(&c, 19),
            at(&r, 49),
            at(&c, 49),
            at(&r, (V7_END - 2) as usize),
            at(&c, (V7_END - 2) as usize)
        );
    }
    let grow = |run: &V7Run| -> std::collections::BTreeMap<String, usize> {
        run.days.iter().skip(20).fold(Default::default(), |mut m, d| {
            for (n, v) in &d.corps {
                if v.1 == citysim::CorpOrder::Grow {
                    *m.entry(n.clone()).or_default() += 1;
                }
            }
            m
        })
    };
    outln!(o, "days on Grow from day 20: {:?} (control {:?})", grow(&r), grow(&c));
    eprint!("{o}");
    r.assert_applied(4);
}

// ---------------------------------------------------------------------------
// God scenarios v8: the contract board (M16a phase 5, docs/M16_CONTRACTS.md,
// docs/GOD_SCENARIOS_V8.md). A contract is a record with a price on a struct,
// matched by a score and resolved by one seeded roll (or a ledger draw)
// between fictional agents. The tests assert only that the god commands
// applied; what the city did is printed, and a city that did not react is a
// gap in the write-up, not a failure.
// ---------------------------------------------------------------------------

const V8_END: u64 = 60;

/// One finished day of a v8 run.
#[derive(Clone)]
struct V8Day {
    row: citysim::DayRow,
    /// Every gang's order, by name.
    gangs: std::collections::BTreeMap<String, Order>,
    /// Living corps' orders, by name.
    corps: std::collections::BTreeMap<String, citysim::CorpOrder>,
    /// Open Fixers, and each Fixer's (heat, book size, closed).
    fixers_open: usize,
    fixers: Vec<(f32, usize, bool)>,
    /// The watched agents still living, and jailed.
    alive: usize,
    jailed: usize,
}

struct V8Run {
    name: &'static str,
    days: Vec<V8Day>,
    /// `(day, kind, text)`: god actions, every event naming a watched entity
    /// (`BountyPaid` counted, not listed), and the board's rare kinds.
    story: Vec<(u64, EventKind, String)>,
    /// `BountyPaid` naming a watched agent, per day.
    bounties: Vec<u32>,
    failed: Vec<String>,
    watched: Vec<EntityId>,
}

/// Seed 42 to `V8_END`; `daily` fires at the start of every day and may
/// add to the watched set.
fn run_v8(name: &'static str, mut daily: impl FnMut(&mut World, u64, &mut Vec<EntityId>)) -> V8Run {
    use citysim::systems::contracts;
    let mut w = World::new(SEED, Config::load());
    let mut days = Vec::new();
    let mut story = Vec::new();
    let mut bounties = Vec::new();
    let mut failed = Vec::new();
    let mut watched: Vec<EntityId> = Vec::new();
    let mut next_id = 0u64;
    for day in 0..V8_END {
        daily(&mut w, day, &mut watched);
        w.run_ticks(TICKS_PER_DAY);
        let mut paid = 0u32;
        let events: Vec<&citysim::Event> = w.events.iter().rev().take_while(|e| e.id >= next_id).collect();
        for e in events.into_iter().rev() {
            let names = e.actors.iter().any(|a| watched.contains(a));
            match e.kind {
                EventKind::PlayerActionFailed => failed.push(format!("d{day} {}", e.text)),
                EventKind::BountyPaid if names => paid += 1,
                EventKind::BountyPaid => {}
                EventKind::PlayerAction
                | EventKind::SoldOut
                | EventKind::StrikeDeclined
                | EventKind::Accessory
                | EventKind::FixerBusted => story.push((day, e.kind, e.text.clone())),
                EventKind::CorpOrder if e.text.contains("Lobby") => story.push((day, e.kind, e.text.clone())),
                EventKind::Founded if e.text.contains("Fixer") => story.push((day, e.kind, e.text.clone())),
                // The noisy kinds stay out of the story.
                EventKind::Witness | EventKind::AssetBought | EventKind::Installed => {}
                _ if names => story.push((day, e.kind, e.text.clone())),
                _ => {}
            }
        }
        next_id = w.events.back().map_or(next_id, |e| e.id + 1);
        bounties.push(paid);
        let mut d = V8Day {
            row: w.stats.history.back().expect("a finished day").clone(),
            gangs: Default::default(),
            corps: Default::default(),
            fixers_open: contracts::open_fixers(&w).len(),
            fixers: Vec::new(),
            alive: watched.iter().filter(|&&a| citysim::systems::law::living(&w, a)).count(),
            jailed: watched.iter().filter(|&&a| w.has::<citysim::Sentence>(a)).count(),
        };
        for g in w.gangs() {
            if let Some(gg) = w.comp::<Gang>(g) {
                d.gangs.insert(gg.name.clone(), gg.order);
            }
        }
        for c in w.corps() {
            if let Some(cc) = w.comp::<citysim::Corp>(c) {
                d.corps.insert(cc.name.clone(), cc.order);
            }
        }
        for &f in w.buildings_of_kind(citysim::BuildingKind::Fixer) {
            if let Some(k) = w.comp::<citysim::contract::Broker>(f) {
                d.fixers.push((k.heat, k.book.len(), !contracts::fixer_open(&w, f)));
            }
        }
        days.push(d);
    }
    V8Run { name, days, story, bounties, failed, watched }
}

/// The unshocked v8 run, computed once per test process.
fn v8_control() -> &'static V8Run {
    static CONTROL: OnceLock<V8Run> = OnceLock::new();
    CONTROL.get_or_init(|| run_v8("v8_control", |_, _, _| {}))
}

fn gang_named(w: &World, name: &str) -> Option<EntityId> {
    w.gangs().into_iter().find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.name == name))
}

/// A named per-day reading of a v8 run.
type V8Series = (&'static str, Box<dyn Fn(&V8Day) -> f64>);

impl V8Run {
    fn sum(&self, from: u64, to: u64, f: &dyn Fn(&V8Day) -> f64) -> f64 {
        self.days.iter().skip(from as usize).take((to - from) as usize).map(f).sum()
    }

    /// The board's columns over `windows` against the control, the extra
    /// `rows`, and the story from the first window.
    fn print_v8(&self, windows: &[(u64, u64)], extra: Vec<V8Series>) {
        let c = v8_control();
        let mut rows: Vec<V8Series> = vec![
            ("contracts posted", Box::new(|d: &V8Day| f64::from(d.row.contract.contracts_posted))),
            ("Hits posted", Box::new(|d: &V8Day| f64::from(d.row.contract.k_posted[0]))),
            ("Locates posted", Box::new(|d: &V8Day| f64::from(d.row.contract.k_posted[3]))),
            ("contracts fulfilled", Box::new(|d: &V8Day| f64::from(d.row.contract.contracts_fulfilled))),
            ("Hits done", Box::new(|d: &V8Day| f64::from(d.row.contract.hits_done))),
            ("Hits by a squad", Box::new(|d: &V8Day| f64::from(d.row.contract.hits_squad))),
            ("strikes declined (pol)", Box::new(|d: &V8Day| f64::from(d.row.contract.strikes_declined_pol))),
            ("sold out", Box::new(|d: &V8Day| f64::from(d.row.contract.sold_out))),
            ("contracts expired", Box::new(|d: &V8Day| f64::from(d.row.contract.contracts_expired))),
            ("contracts failed", Box::new(|d: &V8Day| f64::from(d.row.contract.contracts_failed))),
            ("Accessory", Box::new(|d: &V8Day| f64::from(d.row.contract.accessory))),
            ("bounties paid", Box::new(|d: &V8Day| f64::from(d.row.contract.bounties_paid))),
            ("violent deaths", Box::new(|d: &V8Day| f64::from(d.row.deaths_violence))),
            ("open Fixers (mean)", Box::new(|d: &V8Day| d.fixers_open as f64)),
            ("Fixer heat x100 (mean)", Box::new(|d: &V8Day| f64::from(d.row.contract.f_heat[0]) * 100.0)),
            ("FixerCut income", Box::new(|d: &V8Day| d.row.contract.flow_fixer_cut as f64)),
            ("guards on the take (mean)", Box::new(|d: &V8Day| f64::from(d.row.contract.guards_on_take))),
        ];
        rows.extend(extra);
        let mut o = String::new();
        outln!(o, "\n================ {} ================", self.name);
        out!(o, "{:<28}", "sum over days");
        for (a, b) in windows {
            out!(o, "{:>20}", format!("{a}-{b} / ctl"));
        }
        outln!(o);
        for (name, f) in &rows {
            out!(o, "{name:<28}");
            for &(a, b) in windows {
                let n = if name.contains("(mean)") { (b - a) as f64 } else { 1.0 };
                out!(o, "{:>20}", format!("{:.0} / {:.0}", self.sum(a, b, f) / n, c.sum(a, b, f) / n));
            }
            outln!(o);
        }
        let last = self.days.last().expect("a day");
        outln!(o, "watched {}: living {} jailed {} on day {}", self.watched.len(), last.alive, last.jailed, V8_END - 1);
        let from = windows.first().map_or(0, |w| w.0);
        let told: Vec<&(u64, EventKind, String)> = self.story.iter().filter(|(d, _, _)| *d >= from).collect();
        outln!(o, "story from day {from} ({} lines):", told.len());
        for (d, k, t) in told.iter().take(120) {
            outln!(o, "  d{d:<3} {k:?}: {t}");
        }
        if told.len() > 120 {
            outln!(o, "  ... {} more", told.len() - 120);
        }
        if !self.failed.is_empty() {
            outln!(o, "failed god commands: {:?}", self.failed);
        }
        eprint!("{o}");
    }

    /// The day-by-day order of `who` (a gang or a corp) from `from`, as runs.
    fn orders(&self, from: u64, f: impl Fn(&V8Day) -> String) -> String {
        let mut runs: Vec<(String, u64, u64)> = Vec::new();
        for (i, d) in self.days.iter().enumerate().skip(from as usize) {
            let v = f(d);
            match runs.last_mut() {
                Some((x, _, end)) if *x == v => *end = i as u64,
                _ => runs.push((v, i as u64, i as u64)),
            }
        }
        runs.iter().map(|(v, a, b)| format!("{v} {a}-{b}")).collect::<Vec<_>>().join(", ")
    }

    fn applied(&self) -> usize {
        self.story.iter().filter(|(_, k, _)| *k == EventKind::PlayerAction).count()
    }
}

/// M16a god 1: on day 10 the city posts a brokered Hit at 2,000 coins on
/// every corp's exec. Who takes them, does a squad strike in the Spire or
/// sell out, does a corp's Lobby turn on anyone?
#[test]
#[ignore]
fn god_hit_each_exec_day_10() {
    let r = run_v8("god_hit_each_exec_day_10", |w, day, watched| {
        if day == 10 {
            let execs: Vec<EntityId> =
                w.corps().into_iter().filter_map(|c| w.comp::<citysim::Corp>(c).and_then(|k| k.exec)).collect();
            eprintln!("day 10: {} execs; the Treasury holds {}", execs.len(), w.purse(None));
            for e in execs {
                watched.push(e);
                w.push_command(PlayerCommand::PostContract {
                    buyer: None,
                    kind: citysim::contract::ContractKind::Hit,
                    target: citysim::contract::Target::Agent(e),
                    price: 2000,
                    brokered: true,
                    deadline_days: 0,
                });
            }
        }
    });
    let extra: Vec<V8Series> = vec![
        ("execs living (mean)", Box::new(|d: &V8Day| d.alive as f64)),
        ("execs jailed (mean)", Box::new(|d: &V8Day| d.jailed as f64)),
    ];
    r.print_v8(&[(0, 10), (10, 25), (25, V8_END)], extra);
    let c = v8_control();
    let lobby = |run: &V8Run| -> usize {
        run.days.iter().skip(10).map(|d| d.corps.values().filter(|&&o| o == citysim::CorpOrder::Lobby).count()).sum()
    };
    eprintln!("corp-days on Lobby from day 10: {} (control {})", lobby(&r), lobby(c));
    assert!(r.applied() >= 1, "no Hit posted: {:?}", r.failed);
}

/// M16a god 2: on day 45 (The Hollow has a leader from ~day 40) Arasaka
/// posts a brokered Hit on The Hollow's leader. Where does he live and who
/// holds that district; does Ninefold, if it holds it, sell him out?
#[test]
#[ignore]
fn god_hit_hollow_leader_from_arasaka() {
    let r = run_v8("god_hit_hollow_leader_from_arasaka", |w, day, watched| {
        if day == 45 {
            let arasaka = corp_named(w, "Arasaka").expect("Arasaka");
            let hollow = gang_named(w, "The Hollow").expect("The Hollow");
            let leader = w.comp::<Gang>(hollow).and_then(|g| g.leader).expect("The Hollow has a leader on day 45");
            let home = w.comp::<citysim::Household>(leader).and_then(|h| h.home);
            let d = home.map(|h| w.district_of_building(h));
            eprintln!(
                "day 45: the leader {} lives in {:?} ({:?}); Arasaka holds {}",
                leader.index,
                d.map(|d| w.district_name(d).to_string()),
                d.map(|d| w.districts[d.index()].control),
                w.purse(Some(arasaka))
            );
            watched.push(leader);
            w.push_command(PlayerCommand::PostContract {
                buyer: Some(arasaka),
                kind: citysim::contract::ContractKind::Hit,
                target: citysim::contract::Target::Agent(leader),
                price: 2000,
                brokered: true,
                deadline_days: 0,
            });
        }
    });
    r.print_v8(&[(30, 45), (45, V8_END)], vec![("the leader living (mean)", Box::new(|d: &V8Day| d.alive as f64))]);
    let c = v8_control();
    for g in ["The Hollow", "Ninefold"] {
        let at = |d: &V8Day| d.gangs.get(g).map_or("-".to_string(), |o| format!("{o:?}"));
        eprintln!("{g}: {} (control {})", r.orders(40, at), c.orders(40, at));
    }
    assert!(r.applied() >= 1 && r.failed.is_empty(), "the Hit was not posted: {:?}", r.failed);
}

/// M16a god 3: Fixers unlicensed on day 20 (`fixer_licence=off`: every
/// office's heat floored at 0.5, no new office registers). Do the Fixers
/// bribe, close, or stop opening?
#[test]
#[ignore]
fn god_fixer_licence_off() {
    let r = run_v8("god_fixer_licence_off", |w, day, watched| {
        if day == 20 {
            for &f in w.buildings_of_kind(citysim::BuildingKind::Fixer) {
                watched.extend(w.owner_of(f));
            }
            w.push_command(PlayerCommand::SetFixerLicence(false));
        }
    });
    let extra: Vec<V8Series> = vec![
        ("closed Fixers (mean)", Box::new(|d: &V8Day| d.fixers.iter().filter(|x| x.2).count() as f64)),
        ("book size (mean)", Box::new(|d: &V8Day| d.fixers.iter().map(|x| x.1 as f64).sum::<f64>())),
    ];
    r.print_v8(&[(10, 20), (20, 40), (40, V8_END)], extra);
    let heat = |run: &V8Run, d: usize| run.days.get(d).map(|x| x.fixers.iter().map(|f| f.0).collect::<Vec<_>>());
    eprintln!(
        "Fixer heat on days 19, 21, 40, 59: {:?} {:?} {:?} {:?}",
        heat(&r, 19),
        heat(&r, 21),
        heat(&r, 40),
        heat(&r, 59)
    );
    eprintln!(
        "control: {:?} {:?} {:?} {:?}",
        heat(v8_control(), 19),
        heat(v8_control(), 21),
        heat(v8_control(), 40),
        heat(v8_control(), 59)
    );
    assert!(r.applied() >= 1 && r.failed.is_empty(), "the licence lever did not apply: {:?}", r.failed);
}

/// M16a god 4: on day 45 the city posts a public Locate on Ninefold's
/// leader. Does the bounty find him (sightings paid, an arrest), does his
/// gang Retaliate?
#[test]
#[ignore]
fn god_law_locate_on_gang_leader() {
    let r = run_v8("god_law_locate_on_gang_leader", |w, day, watched| {
        if day == 45 {
            let nine = gang_named(w, "Ninefold").expect("Ninefold");
            let leader = w.comp::<Gang>(nine).and_then(|g| g.leader).expect("Ninefold has a leader on day 45");
            watched.push(leader);
            w.push_command(PlayerCommand::PostContract {
                buyer: None,
                kind: citysim::contract::ContractKind::Locate,
                target: citysim::contract::Target::Agent(leader),
                price: 0,
                brokered: false,
                deadline_days: 0,
            });
        }
    });
    r.print_v8(&[(30, 45), (45, V8_END)], vec![("the leader jailed (mean)", Box::new(|d: &V8Day| d.jailed as f64))]);
    eprintln!("sightings of the leader paid, per day from 45: {:?}", &r.bounties[45..]);
    let c = v8_control();
    let at = |d: &V8Day| d.gangs.get("Ninefold").map_or("-".to_string(), |o| format!("{o:?}"));
    eprintln!("Ninefold: {} (control {})", r.orders(40, at), c.orders(40, at));
    assert!(r.applied() >= 1 && r.failed.is_empty(), "the Locate was not posted: {:?}", r.failed);
}
