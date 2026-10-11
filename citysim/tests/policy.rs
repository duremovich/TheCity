//! P10 (docs/P10_POLICY.md): the safety net as policy levers on the shipped
//! (no-net, market) city, funded from the Treasury. A dole enacted by lever
//! on day 10 is paid from the Treasury to the jobless; a Treasury that cannot
//! pay it for `[policy] lapse_days` days in a row lets it lapse; public works
//! enacted under the market wage law open on the tax band's reading.

use citysim::systems::{demography, economy, policy};
use citysim::{Brain, Config, EventKind, Job, PlayerCommand, Wallet, World, TICKS_PER_DAY};

fn shipped() -> World {
    let w = World::new(42, Config::load());
    assert!(citysim::systems::econ::wages_on(&w) && !policy::net_enacted(&w), "the shipped city: no net");
    w
}

fn treasury(w: &World) -> i64 {
    w.treasury().map_or(0, |t| t.coins)
}

fn lapsed(w: &World) -> usize {
    w.events.iter().filter(|e| e.kind == EventKind::PolicyLapsed).count()
}

/// The dole by lever on day 10 (`--lever day=10:safety_net=on`): nothing
/// was paid before; after it the jobless draw it, every coin out of the
/// Treasury (`collect_dole` moves exactly the dole from the Treasury to the
/// wallet).
#[test]
fn test_dole_enacted_on_day_10_is_paid_from_the_treasury() {
    let mut w = shipped();
    w.run_ticks(10 * TICKS_PER_DAY);
    assert!(w.stats.history.iter().all(|r| r.flow_dole == 0), "no dole before the lever");
    w.push_command(PlayerCommand::SetSafetyNet(true));
    w.apply_commands();
    let rate = w.config.levers.dole_per_day;
    assert!(rate > 0);
    assert_eq!(w.levers.dole_per_day, rate, "the dole at `[levers] dole_per_day`");
    assert!(w.levers.public_works && w.levers.school_meals && w.levers.reserve_release, "the whole net enacted");
    let before = w.stats.history.len();
    w.run_ticks(3 * TICKS_PER_DAY);
    let paid: i64 = w.stats.history.iter().skip(before).map(|r| r.flow_dole).sum();
    assert!(paid > 0, "the jobless drew the dole after the lever ({paid})");
    let drew = w
        .citizens()
        .into_iter()
        .filter(|&a| !w.has::<Job>(a) && w.comp::<Brain>(a).is_some_and(|b| b.last_dole_day.is_some()))
        .count();
    assert!(drew > 0, "jobless adults hold a dole day");
    // One draw, exactly: the Treasury pays, the wallet receives.
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| {
            economy::dole_eligible(&w, a)
                && demography::is_adult(&w, a)
                && w.comp::<Brain>(a).is_some_and(|b| b.last_dole_day != Some(w.day()))
        })
        .expect("a jobless adult who has not drawn today");
    let (t0, c0) = (treasury(&w), w.comp::<Wallet>(a).map_or(0, |x| x.coins));
    assert!(t0 >= 0, "the Treasury holds coins ({t0})");
    assert!(economy::collect_dole(&mut w, a));
    let got = w.comp::<Wallet>(a).map_or(0, |x| x.coins) - c0;
    assert!(got >= i64::from(rate), "at least a day's dole ({got})");
    assert_eq!(treasury(&w), t0 - got, "out of the Treasury");
    assert_eq!(lapsed(&w), 0);
}

/// A Treasury that cannot pay: every dole visit is refused while it is
/// below 0, each such day is dry, and after `lapse_days` dry days in a row
/// the dole and public works lapse (`PolicyLapsed`) and stay off. The
/// control, a Treasury that holds its coins, keeps them.
#[test]
fn test_dole_lapses_when_the_treasury_runs_dry() {
    let lapse = u64::from(Config::load().policy.lapse_days);
    assert!(lapse > 0);
    let mut dry = shipped();
    dry.run_ticks(10 * TICKS_PER_DAY);
    let mut control = dry.clone();
    for w in [&mut dry, &mut control] {
        w.push_command(PlayerCommand::SetSafetyNet(true));
        w.apply_commands();
    }
    let mut lapse_day = None;
    for day in 0..lapse + 3 {
        dry.push_command(PlayerCommand::SetTreasury(-50_000));
        control.push_command(PlayerCommand::SetTreasury(1_000_000));
        dry.run_ticks(TICKS_PER_DAY);
        control.run_ticks(TICKS_PER_DAY);
        if lapse_day.is_none() && lapsed(&dry) > 0 {
            lapse_day = Some(day);
        }
    }
    let day = lapse_day.expect("the net lapsed");
    assert!(day + 1 >= lapse && day < lapse + 1, "after {lapse} dry days (on {day})");
    assert_eq!(lapsed(&dry), 1, "once");
    assert_eq!(dry.levers.dole_per_day, 0, "the dole lapsed");
    assert!(!dry.levers.public_works, "public works lapsed");
    assert!(dry.levers.school_meals, "school meals draw on the Reserve in kind: they do not lapse");
    let tail: i64 = dry.stats.history.iter().rev().take(2).map(|r| r.flow_dole).sum();
    assert_eq!(tail, 0, "no dole once lapsed");
    assert_eq!(lapsed(&control), 0, "a Treasury that pays keeps its net");
    assert_eq!(control.levers.dole_per_day, control.config.levers.dole_per_day);
    assert!(control.stats.history.iter().rev().take(3).any(|r| r.flow_dole > 0), "the control pays");
}

/// Public works under the market wage law (P10): enacted on the shipped city,
/// a Treasury held above the tax band's top opens works posts on the band's
/// hold; repealed, the open posts are withdrawn.
#[test]
fn test_public_works_open_on_the_tax_band() {
    let mut w = shipped();
    w.run_ticks(2 * TICKS_PER_DAY);
    w.push_command(PlayerCommand::SetPublicWorks(true));
    w.apply_commands();
    let top = w.config.treasury.band[1];
    let hold = u64::from(w.config.budget.band_hold_days.max(1));
    for _ in 0..hold + 1 {
        w.push_command(PlayerCommand::SetTreasury(top * 4));
        w.run_ticks(TICKS_PER_DAY);
    }
    assert!(w.events.iter().any(|e| e.kind == EventKind::WorksPosted), "works posted above the band");
    assert!(w.budget.works_posted > 0);
    w.push_command(PlayerCommand::SetPublicWorks(false));
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(w.works_vacancies, 0, "repealed: the open posts withdrawn");
}
