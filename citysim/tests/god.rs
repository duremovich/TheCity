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
    // M13 phase 5: as `god_crackdown_forever`. The unpinned captain now
    // garrisons most of the year (the Jail is half gang members), and the
    // control sat in Garrison on days 43-56, so pinning it at day 45 was no
    // shock. The law holds Patrol from the baseline on, here and in the
    // control, and the shock turns that Patrol into Garrison.
    let r = run_from("god_garrison_forever", pin(Posture::Patrol), pin(Posture::Garrison));
    r.assert_reacted_against(patrol_control());
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
    eprintln!("reactions within 60 days, vs control: {:?}", r.reactions(patrol_control(), END_DAY - SHOCK_DAY));
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
        let windows = [(BASE.0, BASE.1, "base 30-45"), (45, 52, "45-52"), (45, 75, "45-75"), (75, 105, "75-105")];
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

/// 500 doses into every Hideout dealing in Sump East (district 7). Does
/// addiction spread, does a rival gang raid for the stock?
#[test]
#[ignore]
fn god_flood_stims_sump_east() {
    let r = run("god_flood_stims_sump_east", |w| {
        w.push_command(PlayerCommand::FloodStims { district: citysim::DistrictId(7), n: 500 });
    });
    r.report_m13(control());
}

/// Stims legal from day 30 (the setup at the baseline's start). Does gang
/// income fall, does a gang turn to Harvest or Raid?
#[test]
#[ignore]
fn god_stims_legal_day_30() {
    let r = run_from("god_stims_legal_day_30", |w| w.push_command(PlayerCommand::SetStimsLegal(true)), |_| {});
    r.report_m13(control());
}

/// The Spire Clinic's owner bricks every implant it financed. Does a gang
/// lose its fighters?
#[test]
#[ignore]
fn god_brick_spire_clinic_owner() {
    let r = run("god_brick_spire_clinic_owner", |w| {
        let spire = w
            .buildings_of_kind(citysim::BuildingKind::Clinic)
            .iter()
            .copied()
            .find(|&c| w.district_of_building(c).index() == 0);
        match spire.and_then(|c| w.owner_of(c)) {
            Some(o) => w.push_command(PlayerCommand::Brick(o)),
            None => eprintln!("no Spire Clinic with an owner"),
        }
    });
    r.report_m13(control());
}

/// A chase pinned on every driver of the Civic (Home or work there) each
/// day for a week. A crash death, a Manslaughter or Murder report?
#[test]
#[ignore]
fn god_chase_civic_core() {
    let r = run_daily(
        "god_chase_civic_core",
        |_| {},
        |_| {},
        |w, day| {
            if !(SHOCK_DAY..SHOCK_DAY + REACT_DAYS).contains(&day) {
                return;
            }
            let civic = |w: &World, b: Option<EntityId>| b.is_some_and(|b| w.district_of_building(b).index() == 1);
            let drivers: Vec<EntityId> = w
                .citizens()
                .into_iter()
                .filter(|&a| w.comp::<citysim::Kit>(a).is_some_and(|k| k.vehicle.is_some()))
                .filter(|&a| {
                    civic(w, w.comp::<citysim::Household>(a).and_then(|h| h.home))
                        || civic(w, w.comp::<citysim::Job>(a).and_then(|j| j.employer))
                })
                .collect();
            if day == SHOCK_DAY {
                eprintln!("god_chase_civic_core: {} Civic drivers pinned a day", drivers.len());
            }
            for a in drivers {
                w.push_command(PlayerCommand::Chase(a));
            }
        },
    );
    r.report_m13(control());
}

// ---------------------------------------------------------------------------
// M14 god scenarios (docs/GOD_SCENARIOS_V5.md, plan 5.2): the Virt plane.
// Seed 42, 105 days, each against its own unshocked control (`v5_control`).
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

const V5_END: u64 = 105;

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

/// Ten Sump Central adults (district 6) with the best hacking get tier-3
/// decks on day 45. Is the Treasury hacked, does the city raise ICE, how
/// many flatline?
#[test]
#[ignore]
fn god_grant_decks_sump() {
    let r = run_v5("god_grant_decks_sump", |w, day, hour| {
        if day == SHOCK_DAY && hour == 0 {
            w.push_command(PlayerCommand::GrantDecks { district: citysim::DistrictId(6), n: 10, tier: 3 });
        }
    });
    let c = v5_control();
    r.print_v5(c, &[(30, SHOCK_DAY), (SHOCK_DAY, SHOCK_DAY + 7), (SHOCK_DAY, 75), (75, V5_END)], SHOCK_DAY, &[]);
    let city_raises = r
        .story
        .iter()
        .filter(|(t, k, x)| *k == EventKind::IceRaised && t / TICKS_PER_DAY >= SHOCK_DAY && x.starts_with("the city "))
        .count();
    eprintln!("city ICE raises after the grant: {city_raises}");
    r.assert_applied();
}

/// Every Arasaka node to ICE 0 on day 45. How fast is it bled, and does it
/// buy its ICE back?
#[test]
#[ignore]
fn god_arasaka_ice_0() {
    let r = run_v5("god_arasaka_ice_0", |w, day, hour| {
        if day == SHOCK_DAY && hour == 0 {
            match corp_named(w, "Arasaka") {
                Some(a) => w.push_command(PlayerCommand::SetCorpIce { corp: a, tier: 0 }),
                None => eprintln!("no Arasaka on day {SHOCK_DAY}"),
            }
        }
    });
    let c = v5_control();
    let hits: V5Series = (
        "hits on Arasaka".to_string(),
        Box::new(|d: &V5Day| f64::from(d.hits_on.get("Arasaka").copied().unwrap_or(0))),
    );
    let purse: V5Series = (
        "Arasaka treasury (sum)".to_string(),
        Box::new(|d: &V5Day| d.corps.get("Arasaka").map_or(0.0, |x| x.0 as f64)),
    );
    r.print_v5(
        c,
        &[(30, SHOCK_DAY), (SHOCK_DAY, SHOCK_DAY + 7), (SHOCK_DAY, 75), (75, V5_END)],
        SHOCK_DAY,
        &[hits, purse],
    );
    let first = r
        .story
        .iter()
        .find(|(t, k, x)| {
            t / TICKS_PER_DAY >= SHOCK_DAY
                && matches!(k, EventKind::LedgerHacked | EventKind::DataWiped)
                && x.contains("Arasaka")
        })
        .map(|(t, _, x)| format!("d{} {x}", t / TICKS_PER_DAY));
    let rebuys = r
        .story
        .iter()
        .filter(|(t, k, x)| *k == EventKind::IceRaised && t / TICKS_PER_DAY >= SHOCK_DAY && x.starts_with("Arasaka "))
        .count();
    eprintln!(
        "first Ledger hit or wipe on Arasaka after the shock: {first:?}; Arasaka ICE re-raises: {rebuys}; Arasaka gone on day {:?} (control {:?})",
        r.gone("Arasaka"),
        c.gone("Arasaka")
    );
    r.assert_applied();
}

/// The city's ICE to 0 on day 45 (the hacker-army test from VISION.md):
/// can a gang drain the Treasury?
#[test]
#[ignore]
fn god_city_ice_0() {
    let r = run_v5("god_city_ice_0", |w, day, hour| {
        if day == SHOCK_DAY && hour == 0 {
            w.push_command(PlayerCommand::SetCityIce(0));
        }
    });
    let c = v5_control();
    r.print_v5(c, &[(30, SHOCK_DAY), (SHOCK_DAY, SHOCK_DAY + 7), (SHOCK_DAY, 75), (75, V5_END)], SHOCK_DAY, &[]);
    let by_gang = r
        .story
        .iter()
        .filter(|(t, k, x)| {
            *k == EventKind::LedgerHacked && t / TICKS_PER_DAY >= SHOCK_DAY && x.ends_with("the city's ledger")
        })
        .count();
    eprintln!(
        "Treasury hits from day {SHOCK_DAY}: {by_gang}; city treasury on day {}: {} (control {})",
        V5_END - 1,
        r.days.last().map_or(0, |d| d.city_treasury),
        c.days.last().map_or(0, |d| d.city_treasury)
    );
    r.assert_applied();
}

/// From day 45, every gang corp raid gets a god `RunNow Door` on its target
/// the hour its muster is scheduled (its best member is given a tier-3 deck
/// first when the gang has no runner). Does the raid win more often than in
/// the paired control? Vat Farms (the Vats, district 2) are named.
#[test]
#[ignore]
fn god_door_before_raid() {
    let mut ordered: Vec<String> = Vec::new();
    let mut seen: std::collections::BTreeSet<(EntityId, u64)> = std::collections::BTreeSet::new();
    let r = run_v5("god_door_before_raid", |w, day, _| {
        if day < SHOCK_DAY {
            return;
        }
        for g in w.gangs() {
            let Some((target, at)) = w.comp::<Gang>(g).and_then(|gg| {
                (gg.order == Order::Raid).then_some(())?;
                Some((gg.raid_target?, gg.raid_at?))
            }) else {
                continue;
            };
            if at <= w.tick
                || !seen.insert((target, at))
                || w.owner_of(target).is_none_or(|o| !w.has::<citysim::Corp>(o))
            {
                continue;
            }
            let members = w.comp::<Gang>(g).map(|gg| gg.members.clone()).unwrap_or_default();
            let hacking = |w: &World, m: EntityId| w.comp::<citysim::Skills>(m).map_or(0.0, |s| s.hacking);
            let with_deck = members
                .iter()
                .copied()
                .filter(|&m| w.comp::<citysim::Kit>(m).is_some_and(|k| k.deck.is_some()))
                .max_by(|&a, &b| hacking(w, a).total_cmp(&hacking(w, b)).then(b.cmp(&a)));
            let runner = match with_deck {
                Some(m) => m,
                None => {
                    let Some(m) = members
                        .iter()
                        .copied()
                        .filter(|&m| !w.has::<citysim::Sentence>(m))
                        .max_by(|&a, &b| hacking(w, a).total_cmp(&hacking(w, b)).then(b.cmp(&a)))
                    else {
                        continue;
                    };
                    w.push_command(PlayerCommand::GrantDeck { agent: m, tier: 3 });
                    m
                }
            };
            w.push_command(PlayerCommand::RunNow { agent: runner, target, purpose: citysim::virt::Purpose::Door });
            let vats = w.district_of_building(target).index() == 2;
            ordered.push(format!(
                "d{day} {} on {}{}",
                w.name_of(runner),
                w.name_of(target),
                if vats { " (Vats)" } else { "" }
            ));
        }
    });
    let c = v5_control();
    r.print_v5(c, &[(30, SHOCK_DAY), (SHOCK_DAY, 75), (75, V5_END), (SHOCK_DAY, V5_END)], SHOCK_DAY, &[]);
    let share = |run: &V5Run| {
        let won = run.sum(SHOCK_DAY, V5_END, |d| f64::from(d.corp_raids_won));
        let lost = run.sum(SHOCK_DAY, V5_END, |d| f64::from(d.corp_raids_lost));
        (won, lost, won / (won + lost).max(1.0))
    };
    let (rw, rl, rs) = share(&r);
    let (cw, cl, cs) = share(c);
    eprintln!("door runs ordered: {ordered:?}");
    eprintln!("corp raids from day {SHOCK_DAY}: won {rw} lost {rl} ({rs:.2}) vs control won {cw} lost {cl} ({cs:.2})");
    // FINDING (calibration, not asserted; L1b gate doctrine): the scenario needs a gang to schedule a
    // corp raid after day 45 on seed 42 (ab79188 one, on day 98; Raid orders run 0-7 a run). Without
    // one nothing is applied and nothing is tested; with one, the god command must apply.
    if ordered.is_empty() {
        eprintln!("FINDING god_door_before_raid: no corp raid was scheduled after day {SHOCK_DAY}; nothing to test");
        return;
    }
    r.assert_applied();
}

// ---------------------------------------------------------------------------
// M15 god scenarios (docs/GOD_SCENARIOS_V6.md, plan 5.2): the word and the
// blood. Seed 42, 105 days, each against its own unshocked control
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

const V6_END: u64 = 105;

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

    /// Days a gang held Retaliate against `target` (by label) in the window.
    fn retaliate_days(&self, gang: &str, target: &str, from: u64, to: u64) -> usize {
        self.window(from, to)
            .filter(|d| {
                d.gangs.iter().any(|g| g.0 == gang && g.1 == Order::Retaliate && g.2.as_deref() == Some(target))
            })
            .count()
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

/// Gang `name`'s entity, if alive.
fn gang_named(w: &World, name: &str) -> Option<EntityId> {
    w.gangs().into_iter().find(|&g| w.comp::<Gang>(g).is_some_and(|gg| gg.name == name))
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

/// `declare_vendetta=corp<Arasaka>:gang<The Hollow>:1.0` on day 45. Does Retaliate raid an Arasaka
/// building, does Arasaka's Lobby name The Hollow?
#[test]
#[ignore]
fn god_vendetta_arasaka_hollow() {
    let mut what = String::new();
    let r = run_v6("god_vendetta_arasaka_hollow", |w, day| {
        if day == SHOCK_DAY {
            match (corp_named(w, "Arasaka"), gang_named(w, "The Hollow")) {
                (Some(a), Some(b)) => w.push_command(PlayerCommand::DeclareVendetta { a, b, weight: 1.0 }),
                (a, b) => what = format!("Arasaka {a:?}, The Hollow {b:?} on day {SHOCK_DAY}"),
            }
        }
        Vec::new()
    });
    let c = v6_control();
    r.print_v6(c, &[(30, SHOCK_DAY), (SHOCK_DAY, SHOCK_DAY + 7), (SHOCK_DAY, 75), (75, V6_END)], SHOCK_DAY);
    if !what.is_empty() {
        eprintln!("god_vendetta_arasaka_hollow: {what}");
    }
    let ret = r.retaliate_days("The Hollow", "Arasaka", SHOCK_DAY, V6_END);
    let ret_c = c.retaliate_days("The Hollow", "Arasaka", SHOCK_DAY, V6_END);
    let raids: Vec<String> = r
        .story
        .iter()
        .filter(|(t, k, x)| {
            *k == EventKind::Raid
                && t / TICKS_PER_DAY >= SHOCK_DAY
                && x.starts_with("The Hollow")
                && x.contains("Arasaka")
        })
        .map(|(t, _, x)| format!("d{} {x}", t / TICKS_PER_DAY))
        .collect();
    let lobby = |run: &V6Run| {
        run.window(SHOCK_DAY, V6_END)
            .filter(|d| d.lobby.as_ref().is_some_and(|(c, g)| c == "Arasaka" && g == "The Hollow"))
            .count()
    };
    let open = r
        .window(SHOCK_DAY, V6_END)
        .filter(|d| d.vendettas.iter().any(|v| v.contains("Arasaka") && v.contains("The Hollow")))
        .count();
    eprintln!(
        "The Hollow on Retaliate against Arasaka {ret} days (control {ret_c}); Hollow raids on Arasaka buildings {raids:?}; \
         Arasaka's Lobby naming The Hollow {} days (control {}); the vendetta open {open} days",
        lobby(&r),
        lobby(c)
    );
    r.assert_applied(1);
}

/// `plant_rumour=<Zetatech exec>:killed:<a Sump child>:6:1.0` on day 30 (before Zetatech's usual
/// bankruptcy). Does Zetatech's honour fall, its Security contracts move, its employees' loyalty sag, its
/// Spin bury the story?
#[test]
#[ignore]
fn god_plant_zetatech_exec() {
    const DAY: u64 = 30;
    let mut what = String::new();
    let r = run_v6("god_plant_zetatech_exec", |w, day| {
        if day != DAY {
            return Vec::new();
        }
        let exec = corp_named(w, "Zetatech").and_then(|z| w.comp::<citysim::Corp>(z).and_then(|c| c.exec));
        // A child of a Sump Home (Sump Central first, then any Sump district), else any child.
        let sump = |w: &World, a: EntityId| -> Option<usize> {
            let h = w.comp::<citysim::Household>(a).and_then(|h| h.home)?;
            let d = w.district_of_building(h);
            if d.index() == 6 {
                Some(0)
            } else if w.district_name(d).contains("Sump") {
                Some(1)
            } else {
                None
            }
        };
        let child = w
            .citizens()
            .into_iter()
            .filter(|&a| !citysim::systems::demography::is_adult(w, a))
            .min_by_key(|&a| (sump(w, a).unwrap_or(2), a));
        match (exec, child) {
            (Some(about), Some(o)) => {
                let home = w
                    .comp::<citysim::Household>(o)
                    .and_then(|h| h.home)
                    .map(|h| w.district_name(w.district_of_building(h)).to_string());
                what = format!("{} killed {} (home {home:?})", w.name_of(about), w.name_of(o));
                w.push_command(PlayerCommand::PlantRumour {
                    about,
                    deed: citysim::word::Deed::Killed,
                    object: Some(o),
                    district: citysim::DistrictId(6),
                    reach: 1.0,
                });
            }
            (e, ch) => what = format!("no Zetatech exec ({e:?}) or child ({ch:?}) on day {DAY}"),
        }
        Vec::new()
    });
    let c = v6_control();
    r.print_v6(c, &[(0, DAY), (DAY, DAY + 7), (DAY, DAY + 30), (DAY + 30, V6_END)], DAY);
    eprintln!("god_plant_zetatech_exec: {what}");
    let mut o = String::new();
    outln!(o, "Zetatech day: honour / dread / employees' opinion / contracts sold (control)");
    for d in r.window(DAY - 2, DAY + 21) {
        let x = d.corps.get("Zetatech");
        let y = c.days.get(d.day as usize).and_then(|cd| cd.corps.get("Zetatech"));
        let f = |v: Option<&(f32, f32, f32, usize)>| {
            v.map_or("gone".to_string(), |v| format!("{:.2} / {:.2} / {:.3} / {}", v.0, v.1, v.2, v.3))
        };
        outln!(o, "  d{:<3} {} ({})", d.day, f(x), f(y));
    }
    eprint!("{o}");
    let zeta = |run: &V6Run, k: EventKind| {
        run.story
            .iter()
            .filter(|(t, kk, x)| *kk == k && t / TICKS_PER_DAY >= DAY && x.starts_with("Zetatech"))
            .map(|(t, _, x)| format!("d{} {x}", t / TICKS_PER_DAY))
            .collect::<Vec<_>>()
    };
    let exec_name = what.split(" killed ").next().unwrap_or_default().to_string();
    let stories: Vec<String> = r
        .story
        .iter()
        .filter(|(t, k, x)| *k == EventKind::Story && t / TICKS_PER_DAY >= DAY && x.contains(&exec_name))
        .map(|(t, _, x)| format!("d{} {x}", t / TICKS_PER_DAY))
        .collect();
    eprintln!("Feed stories naming {exec_name}: {stories:?}");
    eprintln!("Zetatech buries {:?} (control {:?})", zeta(&r, EventKind::Buried), zeta(c, EventKind::Buried));
    eprintln!("Zetatech plants {:?} (control {:?})", zeta(&r, EventKind::Planted), zeta(c, EventKind::Planted));
    let lost: Vec<&String> = r
        .story
        .iter()
        .filter(|(_, k, x)| *k == EventKind::ContractLost && x.contains("Zetatech"))
        .map(|(_, _, x)| x)
        .collect();
    eprintln!("ContractLost naming Zetatech: {lost:?}");
    r.assert_applied(1);
}

/// `grant_skill=dregs10:persuasion:1.0:suit` on day 45: the ten poorest Dreg adults get persuasion 1.0 and
/// a Spire suit. Do they get meetings (edges to Corp-class agents), jobs, poached?
#[test]
#[ignore]
fn god_suited_dregs() {
    let mut chosen: Vec<EntityId> = Vec::new();
    let r = run_v6("god_suited_dregs", |w, day| {
        if day != SHOCK_DAY {
            return Vec::new();
        }
        let mut dregs: Vec<(i64, EntityId)> = w
            .citizens()
            .into_iter()
            .filter(|&a| citysim::systems::law::living(w, a) && citysim::systems::demography::is_adult(w, a))
            .filter(|&a| citysim::systems::classes::class_of(w, a) == citysim::components::Class::Dreg)
            .map(|a| (w.comp::<citysim::components::Wallet>(a).map_or(0, |x| x.coins), a))
            .collect();
        dregs.sort();
        chosen = dregs.into_iter().take(10).map(|(_, a)| a).collect();
        for &agent in &chosen {
            w.push_command(PlayerCommand::GrantSkill {
                agent,
                skill: citysim::word::SocialSkill::Persuasion,
                value: 1.0,
                suit: true,
            });
        }
        chosen.clone()
    });
    // The control tracks nobody; its readings of the same ten come from a run tracking them from day 45.
    let c = run_v6("god_suited_dregs_control", |_, day| if day == SHOCK_DAY { chosen.clone() } else { Vec::new() });
    r.print_v6(&c, &[(30, SHOCK_DAY), (SHOCK_DAY, SHOCK_DAY + 7), (SHOCK_DAY, 75), (75, V6_END)], SHOCK_DAY);
    let mut o = String::new();
    outln!(o, "the ten: employed / edges to Corp-class agents / standing, shocked (control)");
    for day in [SHOCK_DAY, SHOCK_DAY + 7, SHOCK_DAY + 30, V6_END - 1] {
        let at = |run: &V6Run| run.days.get(day as usize).map(|d| d.tracked.clone()).unwrap_or_default();
        let (a, b) = (at(&r), at(&c));
        let sum = |m: &std::collections::BTreeMap<EntityId, (bool, u32, f32)>| {
            let emp = m.values().filter(|v| v.0).count();
            let edges: u32 = m.values().map(|v| v.1).sum();
            let st = m.values().map(|v| v.2).sum::<f32>() / m.len().max(1) as f32;
            format!("{emp} / {edges} / {st:.2}")
        };
        outln!(o, "  d{day:<3} {} ({})", sum(&a), sum(&b));
    }
    eprint!("{o}");
    let names: Vec<String> = chosen.iter().map(|&a| format!("{a:?}")).collect();
    let poached: Vec<&String> = r
        .story
        .iter()
        .filter(|(t, k, _)| *k == EventKind::Poached && t / TICKS_PER_DAY >= SHOCK_DAY)
        .map(|(_, _, x)| x)
        .collect();
    eprintln!("god_suited_dregs: the ten {names:?}; Poached from day {SHOCK_DAY}: {poached:?}");
    r.assert_applied(10);
}

/// `set_creed=<Ninefold>:purist` on day 45. Does the Sump's chrome fall, do the chromed move out, how
/// many are expelled?
#[test]
#[ignore]
fn god_purist_ninefold() {
    let mut what = String::new();
    let r = run_v6("god_purist_ninefold", |w, day| {
        if day == SHOCK_DAY {
            match gang_named(w, "Ninefold") {
                Some(g) => {
                    w.push_command(PlayerCommand::SetCreed { gang: g, creed: Some(citysim::word::Creed::Purist) })
                }
                None => what = format!("no Ninefold on day {SHOCK_DAY}"),
            }
        }
        Vec::new()
    });
    let c = v6_control();
    r.print_v6(c, &[(30, SHOCK_DAY), (SHOCK_DAY, SHOCK_DAY + 7), (SHOCK_DAY, 75), (75, V6_END)], SHOCK_DAY);
    if !what.is_empty() {
        eprintln!("god_purist_ninefold: {what}");
    }
    let mut o = String::new();
    outln!(o, "Sump adults: housed / chromed / mean Kit.visible, shocked (control); Ninefold members");
    for day in [SHOCK_DAY - 1, SHOCK_DAY, SHOCK_DAY + 7, SHOCK_DAY + 30, V6_END - 1] {
        let f = |run: &V6Run| {
            run.days.get(day as usize).map_or(String::new(), |d| {
                let nf = d.gangs.iter().find(|g| g.0 == "Ninefold").map_or(0, |g| g.5);
                format!("{} / {} / {:.3}; Ninefold {nf}", d.sump.0, d.sump.1, d.sump.2)
            })
        };
        outln!(o, "  d{day:<3} {} ({})", f(&r), f(c));
    }
    eprint!("{o}");
    r.assert_applied(1);
}

// ---------------------------------------------------------------------------
// God scenarios v7: the living city (Life pass L2 phase 5, docs/LIFE_L2.md
// § 5, docs/GOD_SCENARIOS_V7.md). Every wage, bet, killing and arrest is a
// seeded dice roll over structs; the tests assert only that the commands
// applied and print what the city did.
// ---------------------------------------------------------------------------

const V7_END: u64 = 90;

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

/// Mid West if a Club stands there, else the district with the most leisure
/// venues holding a Club (ties the lower id).
fn club_district(w: &World) -> Option<citysim::DistrictId> {
    let clubs: Vec<citysim::DistrictId> =
        w.buildings_of_kind(citysim::BuildingKind::Club).iter().map(|&b| w.district_of_building(b)).collect();
    if let Some(mw) = district_named(w, "Mid West").filter(|d| clubs.contains(d)) {
        return Some(mw);
    }
    let venues = |d: citysim::DistrictId| {
        citysim::BuildingKind::LEISURE
            .iter()
            .flat_map(|&k| w.buildings_of_kind(k).iter())
            .filter(|&&b| w.district_of_building(b) == d)
            .count()
    };
    clubs.into_iter().max_by_key(|&d| (venues(d), std::cmp::Reverse(d)))
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

fn v7_rows_city() -> Vec<V7Series> {
    vec![
        ("thefts", Box::new(|d: &V7Day| f64::from(d.row.thefts))),
        ("violent deaths", Box::new(|d: &V7Day| f64::from(d.row.deaths_violence))),
        ("starvation", Box::new(|d: &V7Day| f64::from(d.row.deaths_starvation))),
        ("employed (mean)", Box::new(|d: &V7Day| f64::from(d.row.employed))),
        ("wages", Box::new(|d: &V7Day| d.row.flow_wages as f64)),
        ("dole", Box::new(|d: &V7Day| d.row.flow_dole as f64)),
        ("flow_leisure", Box::new(|d: &V7Day| d.row.living.flow_leisure as f64)),
        ("visits (all kinds)", Box::new(|d: &V7Day| f64::from(d.row.living.visits.iter().sum::<u32>()))),
    ]
}

/// L2 god 1: every leisure venue in Mid West closed for 14 days on day 45
/// (`close_leisure=<Mid West>:14`): does fun fall there, do theft and
/// assaults rise, does the street fill?
#[test]
#[ignore]
fn god_close_mid_west_clubs() {
    const DAY: u64 = 45;
    let r = run_v7("god_close_mid_west_clubs", |w, day, watch| {
        if day == 0 {
            *watch = club_district(w);
        }
        if day == DAY {
            if let Some(d) = *watch {
                w.push_command(PlayerCommand::CloseLeisure { district: d, days: 14 });
            }
        }
    });
    let w0 = World::new(SEED, Config::load());
    let dd = club_district(&w0).expect("a district with a Club");
    let i = dd.index();
    eprintln!(
        "god_close_mid_west_clubs: Mid West holds no Club on seed 42 (Clubs stand in Civic, Vats and Spire); the shock closes {} (its leisure venues, the Club among them)",
        w0.district_name(dd)
    );
    let c = run_v7("v7_control_clubs", |w, day, watch| {
        if day == 0 {
            *watch = club_district(w);
        }
    });
    let mut rows = v7_rows_city();
    rows.push(("district venue visits", Box::new(|d: &V7Day| f64::from(d.venue_visits))));
    // Civic has no Homes: the city's fun, the district's street.
    rows.push(("city fun x1000 (mean)", Box::new(|d: &V7Day| f64::from(d.row.living.fun_mean) * 1000.0)));
    rows.push((
        "street density x100 (mean)",
        Box::new(move |d: &V7Day| f64::from(d.row.living.street_density.get(i).copied().unwrap_or(0.0)) * 100.0),
    ));
    rows.push(("HangOuts", Box::new(|d: &V7Day| f64::from(d.row.living.hangouts))));
    r.print_v7(&c, &[(30, DAY), (DAY, DAY + 14), (DAY + 14, V7_END)], rows);
    r.assert_applied(1);
}

/// L2 god 2: the dole doubled (4 -> 8) from day 20 to day 50: do jobs
/// empty, do venues gain, does the band move upkeep?
#[test]
#[ignore]
fn god_double_dole_30() {
    let r = run_v7("god_double_dole_30", |w, day, _| {
        if day == 20 {
            let n = w.levers.dole_per_day.saturating_mul(2);
            w.push_command(PlayerCommand::SetDolePerDay(n));
        }
        if day == 50 {
            w.push_command(PlayerCommand::SetDolePerDay(4));
        }
    });
    let c = v7_control("Mid West");
    let mut rows = v7_rows_city();
    rows.push(("Treasury (mean)", Box::new(|d: &V7Day| d.row.treasury as f64)));
    rows.push(("public works (mean)", Box::new(|d: &V7Day| f64::from(d.row.living.works_jobs))));
    rows.push(("upkeep_mult x100 (mean)", Box::new(|d: &V7Day| f64::from(d.row.living.upkeep_mult) * 100.0)));
    rows.push(("fun satisfied x100 (mean)", Box::new(|d: &V7Day| f64::from(d.row.living.fun_satisfied_share) * 100.0)));
    r.print_v7(&c, &[(10, 20), (20, 50), (50, V7_END)], rows);
    let quits = |run: &V7Run| run.days.iter().skip(20).take(30).map(|d| f64::from(d.row.employed)).sum::<f64>() / 30.0;
    eprintln!("god_double_dole_30: employed mean days 20-50 {:.0} (control {:.0})", quits(&r), quits(&c));
    r.assert_applied(2);
}

/// L2 god 3: every Fabber killed on day 20: do imports and the Treasury's
/// customs rise, do the Fabs rehire?
#[test]
#[ignore]
fn god_kill_fab_staff() {
    let mut n = 0usize;
    let r = run_v7("god_kill_fab_staff", |w, day, _| {
        if day == 20 {
            let fabbers: Vec<citysim::EntityId> = w
                .with::<citysim::Job>()
                .into_iter()
                .filter(|&a| w.comp::<citysim::Job>(a).is_some_and(|j| j.role == citysim::Role::Fabber))
                .collect();
            n = fabbers.len();
            for a in fabbers {
                w.push_command(PlayerCommand::KillAgent(a));
            }
        }
    });
    let c = v7_control("Mid West");
    let rows: Vec<V7Series> = vec![
        ("Fabbers employed (mean)", Box::new(|d: &V7Day| f64::from(d.fabbers))),
        ("Fab Parts made", Box::new(|d: &V7Day| f64::from(d.row.living.fab_parts))),
        ("Parts sold from Fabs", Box::new(|d: &V7Day| f64::from(d.row.living.parts_sold_fab))),
        ("Parts sold from Recycler", Box::new(|d: &V7Day| f64::from(d.row.living.parts_sold_recycler))),
        ("Fab stock (mean)", Box::new(|d: &V7Day| f64::from(d.fab_stock))),
        ("parts_imported", Box::new(|d: &V7Day| d.row.living.parts_imported as f64)),
        ("flow_import (customs)", Box::new(|d: &V7Day| d.row.flow_import as f64)),
        ("Treasury (mean)", Box::new(|d: &V7Day| d.row.treasury as f64)),
    ];
    r.print_v7(&c, &[(10, 20), (20, 35), (35, V7_END)], rows);
    eprintln!("god_kill_fab_staff: {n} Fabbers struck dead on day 20");
    // No Fabber on day 20: nothing was issued, so nothing to assert applied.
    if n > 0 {
        r.assert_applied(n);
    }
}

/// L2 god 4: `faction_strike=<Ninefold>:<Stackwell's district>:14` on day
/// 45: do off-screen holes appear there, bound to Ninefold or Unknown?
#[test]
#[ignore]
fn god_faction_strike_stackwell() {
    const DAY: u64 = 45;
    let mut what = String::new();
    let mut picked: Option<citysim::DistrictId> = None;
    let r = run_v7("god_faction_strike_stackwell", |w, day, watch| {
        if day != DAY {
            return;
        }
        // Stackwell (a Housing corp): the district where it owns the most Homes (ties the lower id).
        let d = corp_named(w, "Stackwell").and_then(|c| {
            let mut n: std::collections::BTreeMap<citysim::DistrictId, usize> = Default::default();
            for &h in w.buildings_of_kind(citysim::BuildingKind::Home) {
                if w.owner_of(h) == Some(c) {
                    *n.entry(w.district_of_building(h)).or_default() += 1;
                }
            }
            n.into_iter().max_by_key(|x| (x.1, std::cmp::Reverse(x.0))).map(|x| x.0)
        });
        let d = d.or_else(|| {
            // Else a Sump district Ninefold does not hold.
            let held: Vec<citysim::DistrictId> = gang_named(w, "Ninefold")
                .map(|g| citysim::systems::gang::held_districts(w, g).into_iter().map(|x| x.0).collect())
                .unwrap_or_default();
            (0..w.districts.len())
                .map(|i| citysim::DistrictId(i as u8))
                .find(|&x| w.district_name(x).contains("Sump") && !held.contains(&x))
        });
        match (gang_named(w, "Ninefold"), d) {
            (Some(g), Some(d)) => {
                what = format!("Ninefold strikes {}", w.district_name(d));
                *watch = Some(d);
                picked = Some(d);
                w.push_command(PlayerCommand::FactionStrike { gang: g, district: d, days: 14 });
            }
            (g, d) => what = format!("Ninefold {g:?}, district {d:?} on day {DAY}"),
        }
    });
    let pd = picked;
    let c = run_v7("v7_control_strike", |_, day, watch| {
        if day == DAY {
            *watch = pd;
        }
    });
    let rows: Vec<V7Series> = vec![
        ("faction holes in the district", Box::new(|d: &V7Day| d.holes.len() as f64)),
        ("  of them Ninefold's", Box::new(|d: &V7Day| d.holes.iter().filter(|h| h.0 == "Ninefold").count() as f64)),
        ("fv_killed (city)", Box::new(|d: &V7Day| f64::from(d.row.living.fv_killed))),
        ("fv_assaulted (city)", Box::new(|d: &V7Day| f64::from(d.row.living.fv_assaulted))),
        ("fv_bound (city)", Box::new(|d: &V7Day| f64::from(d.row.living.fv_bound))),
        ("fv_unknown (city)", Box::new(|d: &V7Day| f64::from(d.row.living.fv_unknown))),
        ("fv_bound_wrong", Box::new(|d: &V7Day| f64::from(d.row.living.fv_bound_wrong))),
        ("violent deaths", Box::new(|d: &V7Day| f64::from(d.row.deaths_violence))),
    ];
    r.print_v7(&c, &[(30, DAY), (DAY, DAY + 14), (DAY + 14, V7_END)], rows);
    let by: std::collections::BTreeMap<String, usize> =
        r.days.iter().skip(DAY as usize).take(15).flat_map(|d| d.holes.iter()).fold(Default::default(), |mut m, h| {
            *m.entry(format!("{} {}", h.0, h.1)).or_default() += 1;
            m
        });
    eprintln!(
        "god_faction_strike_stackwell: {what}; faction holes there days {DAY}-{} by faction and source: {by:?}",
        DAY + 14
    );
    r.assert_applied(1);
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
    outln!(o, "corp treasury / order on days 20, 50, 89 (control)");
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
            at(&r, 88),
            at(&c, 88)
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
