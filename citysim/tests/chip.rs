//! Jobs & lives phase 5, the chip contract and braindance (docs/JOBS_AND_LIVES.md
//! § 5.2, § 5.3 and § 12's `vice.rs` unit tests and the debtor's person
//! probe), in numbers only: a contract's hire, its implant's load, the
//! memory and mood skip under it, its end and the `Chipped` trait; a captive
//! chipped at a gang's holding files `Trafficking`; a recorded deed becomes a
//! black unit. Each test is the shipped seed-42 city and one mechanism,
//! driven by calling the system directly.

use citysim::components::{Chipped, Kit, Memory, MemoryKind};
use citysim::systems::{braindance, captives, dollhouse};
use citysim::{AssetKind, AssetLoc, Brain, BuildingKind, Config, Crime, EntityId, Job, World, TICKS_PER_HOUR};

fn on_world() -> World {
    World::new(42, Config::load())
}

/// Free jobless adults outside any gang (ascending).
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

/// `debtor` owes `creditor` `amount` coins on their edge.
fn owe(w: &mut World, debtor: EntityId, creditor: EntityId, amount: i32) {
    let (lo, _) = citysim::components::edge_key(debtor, creditor);
    let e = w.edge_entry(debtor, creditor);
    e.debt = if lo == debtor { amount } else { -amount };
}

fn set_coins(w: &mut World, a: EntityId, coins: i64) {
    if let Some(x) = w.comp_mut::<citysim::Wallet>(a) {
        x.coins = coins;
    }
}

/// The world run until a gang has `n` members (at most 30 days).
fn with_gang(w: &mut World, n: usize) -> EntityId {
    for _ in 0..30 {
        let g = w.gangs().into_iter().find(|&g| w.comp::<citysim::Gang>(g).is_some_and(|x| x.members.len() >= n));
        if let Some(g) = g {
            return g;
        }
        w.run_ticks(citysim::TICKS_PER_DAY);
    }
    panic!("no gang of {n} in 30 days");
}

fn dollhouse(w: &World) -> EntityId {
    *w.buildings_of_kind(BuildingKind::Dollhouse).first().expect("a Dollhouse is seeded")
}

/// A debtor with fewer coins than its debt, its creditor, the contract begun.
fn contracted(w: &mut World) -> EntityId {
    let pool = idle(w);
    let (a, b) = (pool[0], pool[1]);
    owe(w, a, b, 500);
    set_coins(w, a, 10);
    dollhouse::daily(w);
    assert!(w.jobs2.chip.contracts.contains_key(&a), "the debtor took the contract");
    a
}

#[test]
fn test_seeded_and_the_chip_row_never_posted() {
    let w = on_world();
    assert!(!w.buildings_of_kind(BuildingKind::Dollhouse).is_empty(), "a Dollhouse is seeded");
    assert!(!w.buildings_of_kind(BuildingKind::BdStudio).is_empty(), "a BdStudio is seeded");
    let role = dollhouse::chip_role(&w).expect("the chip row is live");
    assert!(!dollhouse::hire_ok(&w, role), "the hiring pass never fills a chip post");
    let d = dollhouse(&w);
    assert!(w.vacancies.get(&d).is_none_or(|v| !v.contains(&role)), "no vacancy is posted for it");
}

/// § 12's person probe: a debtor offered the contract takes it when its
/// coins are below its debt; the same agent with the debt paid refuses.
#[test]
fn test_debtor_takes_the_offer_only_while_short() {
    let mut w = on_world();
    let pool = idle(&w);
    let (a, b) = (pool[0], pool[1]);
    assert!(!dollhouse::takes_offer(&w, a), "no debt, no contract");
    owe(&mut w, a, b, 500);
    set_coins(&mut w, a, 10);
    assert!(dollhouse::takes_offer(&w, a), "coins below the debt: taken");
    owe(&mut w, a, b, 0);
    assert!(!dollhouse::takes_offer(&w, a), "the debt paid: refused");
    owe(&mut w, a, b, 500);
    set_coins(&mut w, a, 600);
    assert!(!dollhouse::takes_offer(&w, a), "coins above the debt: refused");
}

#[test]
fn test_contract_installs_the_chip_and_its_load() {
    let mut w = on_world();
    let a = contracted(&mut w);
    let c = w.jobs2.chip.contracts.get(&a).cloned().expect("contract");
    assert_eq!(w.comp::<Job>(a).and_then(|j| j.employer), Some(c.at), "hired at the Dollhouse");
    assert_eq!(w.comp::<Job>(a).map(|j| j.premium), Some(w.config.vice.chip_mult), "the contract's wage");
    let x = w.comp::<citysim::Asset>(c.chip).expect("the implant");
    assert_eq!(x.kind, AssetKind::Implant(citysim::Slot::Behaviour));
    assert_eq!(x.loc, AssetLoc::Installed(a));
    let load = w.comp::<Kit>(a).map_or(0.0, |k| k.load);
    assert!((load - w.config.vice.chip_load).abs() < 1e-6, "Kit.load carries the chip ({load})");
}

/// D9: a contracted worker on shift writes no memory; off shift it does.
#[test]
fn test_no_memory_under_the_chip_on_shift() {
    let mut w = on_world();
    let a = contracted(&mut w);
    let n = |w: &World| w.comp::<Memory>(a).map_or(0, |m| m.entries.len());
    // Walk the clock to an hour on shift, then to one off it.
    let mut on_shift = false;
    for _ in 0..24 {
        if dollhouse::under_chip(&w, a) {
            on_shift = true;
            break;
        }
        w.tick += TICKS_PER_HOUR;
    }
    assert!(on_shift, "the contract's shift comes round in a day");
    let before = n(&w);
    w.remember(a, MemoryKind::Enjoyed, None, 0.5, 0.5, false);
    assert_eq!(n(&w), before, "nothing written under the chip");
    for _ in 0..24 {
        if !dollhouse::under_chip(&w, a) {
            break;
        }
        w.tick += TICKS_PER_HOUR;
    }
    w.remember(a, MemoryKind::Enjoyed, None, 0.5, 0.5, false);
    assert_eq!(n(&w), before + 1, "written off shift");
}

/// The contract's end: the implant out, the post vacated, the `Chipped`
/// trait and its sanity cut.
#[test]
fn test_end_uninstalls_and_leaves_chipped() {
    let mut w = on_world();
    let a = contracted(&mut w);
    let chip = w.jobs2.chip.contracts.get(&a).map(|c| c.chip).expect("contract");
    let sanity = w.comp::<citysim::Body>(a).map_or(1.0, |b| b.sanity);
    let now = w.tick;
    if let Some(c) = w.jobs2.chip.contracts.get_mut(&a) {
        c.until = now;
    }
    // The debt paid (the contract's savings): no new contract follows.
    let pool = idle(&w);
    for o in pool {
        if w.edge(a, o).is_some_and(|e| e.debt != 0) {
            owe(&mut w, a, o, 0);
        }
    }
    dollhouse::daily(&mut w);
    assert!(!w.jobs2.chip.contracts.contains_key(&a), "the contract ended");
    assert!(w.comp::<citysim::Asset>(chip).is_none(), "the implant is out");
    assert!(w.comp::<Job>(a).is_none(), "the post is vacated");
    assert!(w.has::<Chipped>(a), "the trait stays");
    assert_eq!(w.comp::<Kit>(a).map_or(0.0, |k| k.load), 0.0, "no load left");
    let after = w.comp::<citysim::Body>(a).map_or(1.0, |b| b.sanity);
    assert!(after < sanity, "the sanity cut ({sanity} -> {after})");
}

/// A captive held at a gang's Dollhouse is chipped and `Trafficking` filed
/// on the member who handed it over (the lever on, as shipped).
#[test]
fn test_captive_chipped_is_trafficking() {
    let mut w = on_world();
    let g = with_gang(&mut w, 1);
    let agent = w.comp::<citysim::Gang>(g).map(|x| x.members[0]).expect("member");
    let d = dollhouse(&w);
    let victim = idle(&w)[0];
    assert!(captives::take(&mut w, victim, Some(g), agent, d, None, None), "held at the Dollhouse");
    let filed =
        |w: &World| w.crime_reports().iter().filter(|r| r.crime == Crime::Trafficking && r.suspect == agent).count();
    assert_eq!(filed(&w), 0);
    dollhouse::daily(&mut w);
    let c = w.jobs2.chip.contracts.get(&victim).cloned().expect("the captive is chipped");
    assert!(c.captive);
    assert!(dollhouse::under_chip(&w, victim), "under the chip for the whole hold");
    assert_eq!(filed(&w), 1, "Trafficking filed on the member");
    // Phase-8 review finding 3: past `chip_days` a held captive keeps the
    // same chip (on d2eea01 the contract ended and the same midnight
    // re-chipped it: a new contract, a second Trafficking report).
    w.tick = c.until + 1;
    dollhouse::daily(&mut w);
    let again = w.jobs2.chip.contracts.get(&victim).cloned().expect("still chipped while held");
    assert_eq!(again.since, c.since, "the same contract");
    assert_eq!(filed(&w), 1, "no second Trafficking report");
    // The hold over: the chip comes out, the trait stays.
    captives::release(&mut w, victim, captives::Freed::Expired);
    dollhouse::daily(&mut w);
    assert!(!w.jobs2.chip.contracts.contains_key(&victim));
    assert!(w.has::<Chipped>(victim));
}

/// A gang winner in sight of its gang's Recorder carrier is a recorded deed,
/// made into a black unit at midnight.
#[test]
fn test_recorded_deed_makes_a_black_unit() {
    let mut w = on_world();
    let g = with_gang(&mut w, 2);
    let members = w.comp::<citysim::Gang>(g).map(|x| x.members.clone()).expect("members");
    let (winner, carrier) = (members[0], members[1]);
    let a = citysim::systems::assets::spawn_asset(
        &mut w,
        AssetKind::Recorder,
        1,
        Some(carrier),
        AssetLoc::Carried(carrier),
        1,
    );
    w.jobs2.bd.recorders.insert(a);
    let at = w.comp::<citysim::Position>(carrier).map(|p| p.tile).expect("pos");
    if let Some(p) = w.comp_mut::<citysim::Position>(winner) {
        p.tile = at;
    }
    braindance::after_fight(&mut w, winner);
    assert_eq!(w.jobs2.bd.recorded.get(&g).copied(), Some(1), "a recorded deed");
    braindance::daily(&mut w);
    assert_eq!(w.jobs2.bd.black.get(&g).copied(), Some(1), "a black unit");
}

/// Off: no Dollhouse, no BdStudio, no state; a save round trip with a contract.
#[test]
fn test_off_and_save_round_trip() {
    let mut c = Config::load();
    c.vice.chip = false;
    c.vice.bd = false;
    let w = World::new(42, c);
    assert!(w.buildings_of_kind(BuildingKind::Dollhouse).is_empty());
    assert!(w.buildings_of_kind(BuildingKind::BdStudio).is_empty());
    assert!(w.jobs2.chip.is_default() && w.jobs2.bd.is_default());
    let mut w = on_world();
    contracted(&mut w);
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(citysim::save::to_ron(&back), text, "byte-identical round trip");
}

/// Phase 4's carry-over: with `patrol_sweep` on, a street pitch in a district
/// the law patrols with guards out is swept on `p_patrol_sweep` (and
/// `Solicitation` filed); never with no guards there, or the switch off.
#[test]
fn test_patrol_sweeps_a_street_pitch() {
    use citysim::components::{Pitch, Stance};
    let mut w = on_world();
    w.config.vice.p_patrol_sweep = 1.0;
    let o = idle(&w)[1];
    let trade = w.config.trade_role("street_worker").and_then(|r| r.trade()).expect("the street row");
    let at = w.comp::<citysim::Position>(o).expect("pos").tile;
    let p = Pitch {
        spot: at,
        district: w.district_of(at),
        trade,
        owner: o,
        since: w.tick,
        take_today: 0,
        take_week: 0,
        cut_to: None,
        stock: 0,
        closed_until: None,
        dry_days: 0,
        jobs_today: 0,
        cut_paid: 0,
    };
    let d = p.district.index();
    w.districts[d].stance = Stance::Patrol;
    w.districts[d].guards = 0;
    assert!(!citysim::systems::vice::sweep_street(&mut w, &p), "no guards out, no sweep");
    w.districts[d].guards = 3;
    w.config.vice.patrol_sweep = false;
    assert!(!citysim::systems::vice::sweep_street(&mut w, &p), "the switch off");
    w.config.vice.patrol_sweep = true;
    assert!(citysim::systems::vice::sweep_street(&mut w, &p), "swept on patrol");
    let filed = w.crime_reports().iter().filter(|r| r.suspect == o && r.crime == Crime::Solicitation).count();
    assert_eq!(filed, 1, "Solicitation filed");
}

/// Phase-8 review finding 6: a drinker rolls for a black unit once an
/// agent-day, however many drinks (on d2eea01 every Full drink read the
/// same day's draw and bought again).
#[test]
fn test_black_unit_once_an_agent_day() {
    let mut w = on_world();
    w.config.vice.p_black_buy = 1.0;
    let g = with_gang(&mut w, 1);
    let dealer = w.comp::<citysim::Gang>(g).map(|x| x.members[0]).expect("member");
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    w.enter_building(dealer, bar);
    w.dealers.insert(bar, vec![dealer]);
    w.jobs2.bd.black.insert(g, 5);
    let a = idle(&w)[0];
    set_coins(&mut w, a, 500);
    let sold = |w: &World| w.stats.current.jobs2.as_ref().map_or(0, |j| j.black_sold);
    braindance::bar_visit(&mut w, a, bar, true);
    assert_eq!(sold(&w), 1, "the first drink's roll buys");
    braindance::bar_visit(&mut w, a, bar, true);
    braindance::bar_visit(&mut w, a, bar, true);
    assert_eq!(sold(&w), 1, "one roll a day");
}
