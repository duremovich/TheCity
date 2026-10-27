//! P10 (docs/P10_POLICY.md): the safety net as policy levers on the shipped
//! (no-net, market) city, funded from the Treasury. A dole enacted by lever
//! on day 10 is paid from the Treasury to the jobless; a Treasury that cannot
//! pay it for `[policy] lapse_days` days in a row lets it lapse; public works
//! enacted under the market wage law open on the tax band's reading.

use citysim::systems::{demography, economy, policy};
use citysim::{Brain, Config, EventKind, Job, PlayerCommand, Wallet, World, TICKS_PER_DAY};

fn shipped() -> World {
    // Jobs & lives phase 2: P10's levers as written, without the street
    // (its butterfly changed which jobless adult the dole test draws on);
    // phase 3: nor the weapons (the same butterfly: the adult drawn paid
    // rent out of the dole, `pay_rent_from_income`).
    let mut c = Config::load();
    c.jobs2.street = false;
    c.weapons.enabled = false;
    // Phase 4: nor the vice and fashion (the same butterfly); phase 6: nor
    // the gray market and faith (it fails with both on, passes with either).
    c.vice.enabled = false;
    c.fashion.enabled = false;
    c.gray.enabled = false;
    c.faith.enabled = false;
    let w = World::new(42, c);
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
                && w.comp::<Brain>(a).is_some()
                // Jobs & lives phase 7b: no rent to settle (`collect_dole`
                // settles a tenant's rent out of the dole,
                // `pay_rent_from_income`; the first pick under the mirrored
                // off-screen evening was a tenant): no company bed, and the
                // rent from income off for the draw below.
                && !citysim::systems::company::housed(&w, a)
        })
        .expect("a jobless adult");
    // Pressure pass round 2, R1 (a butterfly: no jobless adult left who had
    // not drawn today, none without a tenancy): today's draw forgotten and
    // the rent left out, so the one draw below is the dole's alone.
    if let Some(b) = w.comp_mut::<Brain>(a) {
        b.last_dole_day = None;
    }
    w.config.rent.pay_from_income = false;
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
    // The dry city and the control side by side (the change tier's budget,
    // 2026-10-19); each its own daily Treasury pin, as before.
    let (dry, control, lapse_day) = std::thread::scope(|s| {
        let d = s.spawn(move || {
            let mut lapse_day = None;
            for day in 0..lapse + 3 {
                dry.push_command(PlayerCommand::SetTreasury(-50_000));
                dry.run_ticks(TICKS_PER_DAY);
                if lapse_day.is_none() && lapsed(&dry) > 0 {
                    lapse_day = Some(day);
                }
            }
            (dry, lapse_day)
        });
        let c = s.spawn(move || {
            for _ in 0..lapse + 3 {
                control.push_command(PlayerCommand::SetTreasury(1_000_000));
                control.run_ticks(TICKS_PER_DAY);
            }
            control
        });
        let (dry, lapse_day) = d.join().expect("the dry city");
        (dry, c.join().expect("the control"), lapse_day)
    });
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

/// Public works sit on top of the civic headcount (L2 L10): when the civic
/// budget lowers Sanitation, the city's own sweepers go, never the works
/// hires (they were the newest Sanitation hires, so `reconcile_sanitation`
/// let them go first: works 30 -> 2 inside the band on seed 42).
#[test]
fn test_civic_cut_spares_public_works_hires() {
    // The El off: its maintenance crews are Sanitation hires too (funded by
    // fares, growing with them), and this test reads the city's own.
    let mut cfg = Config::load();
    cfg.transit.enabled = false;
    // Jobs & lives phase 2: the city as written, without the street (a works
    // hire went to jail in the butterfly it makes); phase 4: nor the vice
    // and fashion (a works hire lost his post in theirs).
    cfg.jobs2.street = false;
    // Lives at work phase 6: nor the careers (two works hires lost their
    // posts in its city: its midnight retires and lets go the old).
    cfg.strip_careers();
    cfg.vice.enabled = false;
    cfg.fashion.enabled = false;
    // The island pass phase 1: nor the distance picks (a works hire lost
    // his post in their butterfly).
    cfg.distance = None;
    let mut w = World::new(42, cfg);
    w.run_ticks(2 * TICKS_PER_DAY);
    w.push_command(PlayerCommand::SetSafetyNet(true));
    w.apply_commands();
    let recycler = w.building_of_kind(citysim::BuildingKind::Cemetery).expect("the Recycler");
    let band = w.config.treasury.band;
    let hold = u64::from(w.config.budget.band_hold_days.max(1));
    for _ in 0..3 * hold + 1 {
        w.push_command(PlayerCommand::SetTreasury(band[1] * 4));
        w.run_ticks(TICKS_PER_DAY);
    }
    let works = w.jobs_book.works.clone();
    assert!(!works.is_empty(), "public works hired above the band");
    // The city's own sweepers: Sanitation outside the works roster.
    let civic =
        |w: &World| w.workers(citysim::Role::Sanitation).iter().filter(|s| !w.jobs_book.works.contains(s)).count();
    let sweepers = civic(&w);
    assert!(sweepers > 0, "civic sweepers beside the works");
    // Inside the band (no works step either way), the civic headcount cut to 0.
    w.push_command(PlayerCommand::SetTreasury((band[0] + band[1]) / 2));
    w.push_command(PlayerCommand::SetSanitation(0));
    w.apply_commands();
    w.run_ticks(3 * TICKS_PER_DAY);
    let kept = works
        .iter()
        .filter(|&&a| {
            w.comp::<Job>(a).is_some_and(|j| j.role == citysim::Role::Sanitation && j.employer == Some(recycler))
        })
        .count();
    assert_eq!(kept, works.len(), "every works hire kept his post");
    let after = civic(&w);
    assert!(after < sweepers, "the civic sweepers were let go ({sweepers} -> {after})");
}
