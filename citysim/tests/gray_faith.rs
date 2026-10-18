//! Jobs & lives phase 6, the gray market and faith (docs/JOBS_AND_LIVES.md
//! § 7, § 8 and § 12's probes): papers bought while wanted pass a guard's
//! sighting more often (pooled over the hour's roll); a ripped body yields
//! its organs once and they rot; the gang types are assigned by share; a
//! cult's leaver takes the leader's Apostasy grudge; the widow attends a
//! Preach, converts at the row's rate and tithes at her next wage. Each
//! test is the shipped seed-42 city and one mechanism, driven directly.

mod common;

use citysim::systems::{faith, gray};
use citysim::{
    Brain, BuildingKind, Config, EntityId, Gang, Good, Job, MemoryKind, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};
use common::{jobless_housed, set_coins, world};

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

/// § 12's probe: a wanted agent with papers passes the guard's sighting on
/// some hours; without, on none (pooled over 20 hours of the roll).
#[test]
fn test_papers_pass_the_sighting_more_often() {
    let mut w = world();
    let a = idle(&w)[0];
    let count = |w: &mut World| {
        let mut n = 0;
        for h in 0..20u64 {
            w.tick = h * TICKS_PER_HOUR;
            if gray::passes_sighting(w, a) {
                n += 1;
            }
        }
        n
    };
    assert_eq!(count(&mut w), 0, "no papers: seen every hour");
    w.jobs2.gray.papers.insert(a, gray::Papers { tier: 2, fake: true, forger: None });
    let with = count(&mut w);
    assert!(with > 0 && with < 20, "papers pass some hours ({with} of 20)");
}

/// A body ripped by a gang yields `organs_per_body` once; the stock rots.
#[test]
fn test_organs_once_and_rot() {
    let mut w = world();
    for _ in 0..30 {
        if !w.gangs().is_empty() {
            break;
        }
        w.run_ticks(TICKS_PER_DAY);
    }
    let g = w.gangs()[0];
    let h = w.hideout_of(g).expect("hideout");
    let body = idle(&w)[0];
    let before = w.stock(h, Good::Organs);
    gray::take_organs(&mut w, Some(g), body);
    assert_eq!(w.stock(h, Good::Organs), before, "a living body yields nothing");
    w.kill_by(body, citysim::DeathCause::Violence, None);
    gray::take_organs(&mut w, Some(g), body);
    let n = w.config.gray.organs_per_body;
    assert_eq!(w.stock(h, Good::Organs), before + n, "the organs");
    gray::take_organs(&mut w, Some(g), body);
    assert_eq!(w.stock(h, Good::Organs), before + n, "once");
    // The World closed: no sale; the midnight pass rots the stock.
    w.econ.world_closed = true;
    gray::daily(&mut w);
    assert!(w.stock(h, Good::Organs) < before + n, "rotted");
}

/// The gang types by share: with one row of share 1, every gang is typed
/// but a creed gang (Shadow V4 bug 13: a cult typed as scavs harvested the
/// chrome it forbids).
#[test]
fn test_gang_types_by_share() {
    let mut c = Config::load();
    c.gray.gang_types.truncate(1);
    c.gray.gang_types[0].share = 1.0;
    let mut w = World::new(42, c);
    for _ in 0..30 {
        if !w.gangs().is_empty() {
            break;
        }
        w.run_ticks(TICKS_PER_DAY);
    }
    gray::daily(&mut w);
    for g in w.gangs() {
        let creed = citysim::systems::creeds::creed_of(&w, g).is_some();
        assert_eq!(gray::gang_type(&w, g).is_some(), !creed, "typed, a creed gang not");
    }
}

/// A cult's member who walks away takes the leader's Apostasy grudge; one
/// cast out does not.
#[test]
fn test_apostasy_grudge() {
    let mut w = world();
    // The rows' churches and chapels stand from the first midnight.
    w.run_ticks(TICKS_PER_DAY + 1);
    let cult = w.gangs().into_iter().find(|&g| faith::cult_creed(&w, g).is_some()).expect("a seeded cult");
    let pool = idle(&w);
    for &m in &pool[..3] {
        citysim::systems::gang::enlist(&mut w, m, cult);
    }
    let leader = w.comp::<Gang>(cult).and_then(|g| g.leader).expect("leader");
    let leaver = pool[..3].iter().copied().find(|&m| m != leader).expect("a member");
    citysim::systems::gang::leave(&mut w, leaver, "walked away");
    let held = |w: &World, t: EntityId| {
        w.comp::<citysim::word::Grudges>(leader)
            .is_some_and(|g| g.list.iter().any(|x| x.target == t && x.cause == citysim::word::GrudgeCause::Apostasy))
    };
    assert!(held(&w, leaver), "the leader's grudge");
    let other = pool[..3].iter().copied().find(|&m| m != leader && m != leaver).expect("another");
    citysim::systems::gang::leave(&mut w, other, "expelled");
    assert!(!held(&w, other), "no grudge on the cast out");
}

/// § 12's widow probe: a grieving resident of a church's district attends
/// its Preach within the week, converts at the row's rate (here 1), and her
/// tithe shows at her next wage.
#[test]
fn test_widow_attends_converts_and_tithes() {
    let mut c = Config::load();
    c.faith.p_attend = 1.0;
    for r in &mut c.faith.creeds {
        r.p_convert = 1.0;
    }
    let mut w = World::new(42, c);
    // The church stands and the district's residents are binned at the first midnight.
    w.run_ticks(TICKS_PER_DAY + 1);
    let (&church, &creed) = w.jobs2.faith.churches.iter().next().expect("a seeded church");
    let d = w.district_of_building(church);
    let widow = w.districts[d.index()]
        .residents
        .clone()
        .into_iter()
        .find(|&a| {
            citysim::systems::demography::is_adult(&w, a)
                && faith::creed_of(&w, a).is_none()
                && w.gang_of(a).is_none()
                && !w.has::<citysim::Sentence>(a)
        })
        .expect("a resident");
    w.remember(widow, MemoryKind::Grief, None, 0.9, -0.9, false);
    faith::daily(&mut w);
    assert_eq!(faith::creed_of(&w, widow), Some(creed), "converted at the Preach");
    // Her next wage: a post with a day owed.
    let market = *w.buildings_of_kind(BuildingKind::Market).first().expect("market");
    if w.has::<Job>(widow) {
        citysim::systems::economy::quit_job(&mut w, widow, "test");
    }
    citysim::systems::demography::hire(&mut w, widow, market, citysim::Role::Clerk);
    if let Some(j) = w.comp_mut::<Job>(widow) {
        j.days_unpaid = 1;
    }
    let purse = |w: &World| w.comp::<citysim::Building>(church).and_then(|b| b.charity.as_ref()).map_or(0, |c| c.purse);
    let before = purse(&w);
    let paid = citysim::systems::economy::collect_wage(&mut w, widow);
    assert!(paid > 0, "paid");
    assert!(purse(&w) > before, "the tithe in the church's purse");
}

/// Off: no papers, no church, no cult, no state; a save round trip on.
#[test]
fn test_off_and_save_round_trip() {
    let mut c = Config::load();
    c.gray.enabled = false;
    c.faith.enabled = false;
    let w = World::new(42, c);
    assert!(w.jobs2.gray.is_default() && w.jobs2.faith.is_default());
    assert!(w.gangs().into_iter().all(|g| faith::cult_creed(&w, g).is_none()));
    assert!(w.buildings_of_kind(BuildingKind::BlackClinic).is_empty());
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY);
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(citysim::save::to_ron(&back), text, "byte-identical round trip");
}

/// Phase 8 (§ 12): a gang's units go to the Clinics' shelves first, then
/// the World's book (capped: no more than the day's cap crosses), before
/// the midnight rot.
#[test]
fn test_organs_sold_then_exported() {
    let mut w = world();
    for _ in 0..30 {
        if !w.gangs().is_empty() {
            break;
        }
        w.run_ticks(TICKS_PER_DAY);
    }
    w.config.gray.p_customs = 0.0;
    let g = w.gangs()[0];
    let h = w.hideout_of(g).expect("hideout");
    // Every Clinic's shelf already full: the units go to the World.
    let full = w.config.gray.clinic_organs;
    for c in w.buildings_of_kind(BuildingKind::Clinic).to_vec() {
        let have = w.stock(c, Good::Organs);
        w.add_stock(c, Good::Organs, full.saturating_sub(have));
    }
    w.add_stock(h, Good::Organs, 500);
    let have = w.stock(h, Good::Organs);
    let cap = citysim::systems::world_market::cap_today(&w, citysim::outside::ExportGood::Organs);
    let row0 = w.stats.current.jobs2.as_ref().map_or(0, |j| j.organs_exported);
    gray::daily(&mut w);
    let sold = w.stats.current.jobs2.as_ref().map_or(0, |j| j.organs_exported) - row0;
    assert!(sold > 0, "exported");
    assert!(sold <= cap, "the book capped: {sold} against a day's cap of {cap}");
    assert!(w.stock(h, Good::Organs) < have - sold, "and the rest rotted after the sale");
}

/// Phase 8 (§ 12): a stabilisation at a Clinic draws a unit off its shelf
/// on `p_organ_needed`; an empty shelf draws nothing.
#[test]
fn test_stabilise_draws_a_clinic_organ() {
    let mut w = world();
    w.config.gray.p_organ_needed = 1.0;
    let c = w.buildings_of_kind(BuildingKind::Clinic)[0];
    let have = w.stock(c, Good::Organs);
    w.take_stock(c, Good::Organs, have);
    gray::on_stabilise(&mut w, c);
    assert_eq!(w.stock(c, Good::Organs), 0, "nothing to draw");
    w.add_stock(c, Good::Organs, 2);
    gray::on_stabilise(&mut w, c);
    assert_eq!(w.stock(c, Good::Organs), 1, "a unit used");
    assert_eq!(w.stats.current.jobs2.as_ref().map_or(0, |j| j.organs_used), 1);
}

/// Phase 8 (§ 12): a licensed Ripperdoc struck off (`p_struck_off` at 1)
/// leaves the post, and the licensed Clinics no longer hire him.
#[test]
fn test_struck_off_leaves_the_post() {
    let mut w = world();
    w.config.gray.p_struck_off = 1.0;
    let doc = w
        .citizens()
        .into_iter()
        .find(|&a| {
            w.comp::<Job>(a).is_some_and(|j| j.role == citysim::Role::Ripperdoc)
                && w.comp::<citysim::Personality>(a).is_some_and(|p| p.lawfulness < 1.0)
        })
        .expect("a Ripperdoc");
    gray::daily(&mut w);
    assert!(w.comp::<Job>(doc).is_none(), "off the post");
    assert!(!gray::hire_ok(&w, doc, citysim::Role::Ripperdoc), "and not hired again");
    assert!(w.stats.current.jobs2.as_ref().is_some_and(|j| j.struck_off >= 1));
}

/// Phase-8 review finding 5: papers exposed at an arrest file a Forgery
/// that counts like any open report: a holder arrested on a Vagrancy alone
/// serves the Forgery's term (on d2eea01 the Forgery was filed after the
/// sentence's crime was picked and resolved with it).
#[test]
fn test_exposed_papers_set_the_sentence() {
    let mut w = world();
    w.config.gray.p_expose = 1.0;
    let a = idle(&w)[0];
    let guard = w.guards()[0];
    w.jobs2.gray.papers.insert(a, gray::Papers { tier: 1, fake: true, forger: None });
    citysim::systems::law::file_report(&mut w, citysim::Crime::Vagrancy, a, None);
    citysim::systems::law::jail_suspect(&mut w, guard, a);
    let s = w.comp::<citysim::Sentence>(a).expect("sentenced");
    assert_eq!(s.crime, citysim::Crime::Forgery, "the exposed papers set the term");
    assert!(!w.jobs2.gray.papers.contains_key(&a), "the papers are void");
}

// ---------------------------------------------------------------------------
// Shadow V4 bug 13 (docs/SHADOW_V4.md): creeds bind.
// ---------------------------------------------------------------------------

/// Bug 13: a Purist member is not sold the chrome that would expel him
/// (a member of a creedless gang in the same state is); a cult preaches
/// under an order other than Expand; a creed gang takes no gray type.
#[test]
fn test_bug13_creeds_bind() {
    use citysim::systems::{assets, creeds, gang, leisure};
    let mut w = world();
    let gangs = w.gangs();
    let (pg, plain) = (gangs[0], gangs[1]);
    w.comp_mut::<Gang>(pg).expect("g").creed = Some(citysim::word::Creed::Purist);
    w.comp_mut::<Gang>(plain).expect("g").creed = None;
    let pool = jobless_housed(&w);
    let (a, b) = (pool[0], pool[1]);
    gang::enlist(&mut w, a, pg);
    gang::enlist(&mut w, b, plain);
    for x in [a, b] {
        set_coins(&mut w, x, 5000);
        w.comp_mut::<citysim::Needs>(x).expect("n").wealth = 1.0;
        w.comp_mut::<citysim::Skills>(x).expect("s").hacking = 0.0;
    }
    assert_eq!(creeds::visible_room(&w, a), Some(w.config.creeds.creed_tolerance));
    assert_eq!(creeds::visible_room(&w, b), None);
    let chrome = |w: &World, x: EntityId| {
        assets::shop_choice(w, x, false).is_some_and(|o| o.category == assets::ShopCategory::Chrome)
    };
    assert!(chrome(&w, b), "the control: a creedless member is sold chrome");
    assert!(!chrome(&w, a), "a Purist is not sold the chrome that expels him");
    // The cult's sermon under LieLow (a Purist's waits for Expand).
    let cult = w.config.faith.creeds.iter().position(|_| true).expect("a cult row") as u8;
    w.comp_mut::<Gang>(plain).expect("g").creed = Some(citysim::word::Creed::Config(cult));
    w.comp_mut::<Gang>(plain).expect("g").order = citysim::Order::LieLow;
    let share = (w.config.leisure.preach_share * 100.0) as u32;
    let preacher = w
        .citizens()
        .into_iter()
        .find(|&x| {
            x.index % 100 < share && !w.has::<citysim::GangMember>(x) && citysim::systems::demography::is_adult(&w, x)
        })
        .expect("a preacher-to-be");
    gang::enlist(&mut w, preacher, plain);
    assert!(leisure::preacher(&w, preacher), "a cult preaches at LieLow");
    // The gray types are dealt at midnight: none for a creed gang.
    w.jobs2.gray.types.clear();
    w.run_ticks(TICKS_PER_DAY);
    for g in [pg, plain] {
        if creeds::creed_of(&w, g).is_some() {
            assert!(gray::gang_type(&w, g).is_none(), "a creed gang is no scav crew");
        }
    }
}
