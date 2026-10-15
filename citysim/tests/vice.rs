//! Jobs & lives phase 4, vice (docs/JOBS_AND_LIVES.md § 5.1 and § 12's
//! `vice.rs` unit tests): an economic and legal simulation of venues and
//! labour. A tip is customer -> worker and untaxed; a street pitch is swept
//! (and `Solicitation` filed) in a Sweep district only, never with
//! `street_legal` on; a pimp present zeroes the street's assault roll; the
//! seeded venues and levers. Each test is the shipped seed-42 city and one
//! mechanism, driven by calling the system directly.

use citysim::components::{Lod, Needs, Pitch, Stance};
use citysim::systems::{lod, vice};
use citysim::{Brain, BuildingKind, Config, Crime, EntityId, Job, Niche, Role, Wallet, World};

fn on_world() -> World {
    World::new(42, Config::load())
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |x| x.coins)
}

/// Free jobless adults (ascending).
fn idle(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(w, a)
                && w.gang_of(a).is_none()
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
        })
        .collect()
}

fn street_pitch(w: &mut World, owner: EntityId) -> Pitch {
    let trade = w.config.trade_role("street_worker").and_then(|r: Role| r.trade()).expect("the street row");
    let at = w.comp::<citysim::Position>(owner).expect("pos").tile;
    let d = w.district_of(at);
    let p = Pitch {
        spot: at,
        district: d,
        trade,
        owner,
        since: w.tick,
        take_today: 0,
        take_week: 0,
        cut_to: None,
        stock: 0,
        closed_until: None,
        dry_days: 0,
        jobs_today: 0,
    };
    w.jobs2.pitches.insert(owner, p.clone());
    p
}

#[test]
fn test_seeded_venues_niche_and_levers() {
    let w = on_world();
    assert!(!w.buildings_of_kind(BuildingKind::StripClub).is_empty(), "a StripClub is seeded");
    assert!(!w.buildings_of_kind(BuildingKind::Brothel).is_empty(), "a Brothel is seeded");
    let b = w.buildings_of_kind(BuildingKind::Brothel)[0];
    assert!(w.comp::<citysim::Building>(b).is_some_and(|bd| bd.venue.is_some()), "a venue");
    let owner = w.owner_of(b).expect("a corp's");
    assert!(w.comp::<citysim::Corp>(owner).is_some_and(|c| c.niches.contains(&Niche::Vice)));
    assert!(Niche::live(true).contains(&Niche::Vice) && !Niche::live(false).contains(&Niche::Vice));
    // Dylan's defaults.
    assert!(w.levers.vice.brothels_licensed && !w.levers.vice.street_legal);
    assert!(w.levers.vice.dollhouse_contracts && w.levers.vice.captive_chip_trafficking);
    let mut c = Config::load();
    c.vice.enabled = false;
    let off = World::new(42, c);
    assert!(off.buildings_of_kind(BuildingKind::StripClub).is_empty());
    assert!(off.levers.vice.is_default() && off.jobs2.vice.is_default());
}

#[test]
fn test_tip_is_customer_to_worker_and_untaxed() {
    let mut w = on_world();
    let b = w.buildings_of_kind(BuildingKind::Brothel)[0];
    // An evening tick, a tipped worker of the Brothel on duty off screen.
    w.tick = 20 * 60;
    let worker = w
        .staff_of(b)
        .iter()
        .copied()
        .find(|&s| {
            w.comp::<Job>(s).is_some_and(|j| citysim::systems::trades::row_of(&w, j.role).is_some_and(|r| r.tipped))
        })
        .expect("a tipped worker hired at seed");
    lod::set_lod(&mut w, worker, Lod::Statistical);
    assert_eq!(vice::tipped_on_duty(&w, b), Some(worker));
    let visitor = idle(&w)[0];
    w.comp_mut::<Wallet>(visitor).expect("wallet").coins = 100;
    w.comp_mut::<Needs>(visitor).expect("needs").intimacy = 0.1;
    let (c0, wk0) = (coins(&w, visitor), coins(&w, worker));
    let tax0 = w.stats.current.flow_tax;
    let price = 10;
    vice::on_visit(&mut w, visitor, b, price);
    let tip = ((price as f32 * w.config.vice.tip_share).round() as i64).max(1);
    assert_eq!(coins(&w, visitor), c0 - tip, "the customer pays the tip");
    assert_eq!(coins(&w, worker), wk0 + tip, "the worker takes it whole");
    assert_eq!(w.stats.current.flow_tax, tax0, "untaxed");
    assert!(w.comp::<Needs>(visitor).is_some_and(|n| n.intimacy > 0.1), "a Brothel's visit gives intimacy");
    let j2 = w.stats.current.jobs2.clone().unwrap_or_default();
    assert_eq!((j2.brothel_visits, j2.tips), (1, tip));
}

#[test]
fn test_street_sweep_in_a_sweep_district_only() {
    let mut w = on_world();
    w.config.vice.p_street_sweep = 1.0;
    let o = idle(&w)[1];
    let p = street_pitch(&mut w, o);
    let reports =
        |w: &World| w.crime_reports().iter().filter(|r| r.suspect == o && r.crime == Crime::Solicitation).count();
    w.districts[p.district.index()].stance = Stance::Patrol;
    assert!(!vice::sweep_street(&mut w, &p), "no sweep on patrol");
    w.districts[p.district.index()].stance = Stance::Sweep;
    w.levers.vice.street_legal = true;
    assert!(!vice::sweep_street(&mut w, &p), "none with street work legal");
    assert_eq!(reports(&w), 0);
    w.levers.vice.street_legal = false;
    assert!(vice::sweep_street(&mut w, &p), "swept in a Sweep district");
    assert_eq!(reports(&w), 1, "Solicitation filed");
}

#[test]
fn test_pimp_present_zeroes_the_assault_roll() {
    let run = |pimp: bool| -> u32 {
        let mut w = on_world();
        // A day run: the districts' residents are binned at the midnight pass.
        w.run_ticks(citysim::TICKS_PER_DAY + 1);
        w.config.vice.p_assault_street = 10.0;
        w.config.vice.p_street_visit = 0.0;
        let o = idle(&w)[2];
        lod::set_lod(&mut w, o, Lod::Statistical);
        let p = street_pitch(&mut w, o);
        if let Some(x) = w.jobs2.pitches.get_mut(&o) {
            x.take_today = 5;
        }
        if pimp {
            let g = w.gangs()[0];
            let m = idle(&w)[3];
            citysim::systems::gang::enlist(&mut w, m, g);
            w.jobs2.vice.pimps.insert(p.district.index() as u16, m);
        }
        // A night hour (the street worker's shift).
        w.tick = citysim::TICKS_PER_DAY + 2 * 60;
        assert!(citysim::systems::streetwork::working(&w, &p));
        vice::hourly(&mut w);
        w.stats.current.jobs2.clone().unwrap_or_default().street_assaults
    };
    assert_eq!(run(true), 0, "a pimp's protection");
    assert_eq!(run(false), 1, "unprotected, the roll lands");
}
