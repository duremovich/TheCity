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
    r.assert_applied();
    assert!(!ordered.is_empty(), "god_door_before_raid: no corp raid was scheduled after day {SHOCK_DAY}");
}
