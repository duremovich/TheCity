//! The Real economy phase 3c (docs/ECONOMY_V2.md addendum; roadmap addendum
//! 19; plan 3c.6): the child protective service and the work camps. A take
//! is a child's `Household` moved to a Camp, a camp day food units off a
//! shelf and counters, a shift Parts credited to the camp's stock, a release
//! a trait on an adult; every coin moves between integer purses.

use citysim::econ::CampRaised;
use citysim::systems::econ::identity;
use citysim::systems::{assets, camp, demography, founding, grudges};
use citysim::word::{GrudgeCause, Grudges};
use citysim::{
    Building, BuildingKind, Child, Config, EntityId, Good, Household, Identity, Memory, MemoryKind, Position, Skills,
    Wallet, World, TICKS_PER_DAY,
};

/// The dole still on at the base: the camp is idle in a default run, so the
/// tests force an unfed child (school meals off, a Block's pantry and its
/// adults' wallets emptied every evening).
fn config() -> Config {
    let mut cfg = Config::load();
    cfg.demography.school_meals = false;
    cfg
}

/// Phase 2 (plan E13, E40): with wages on a camp's Parts pay `input_per_part`
/// to the World as a Fab's do (the camp is no mint); the identity holds.
#[test]
fn test_camp_parts_pay_inputs_with_wages_on() {
    let mut cfg = config();
    cfg.economy2.wages = true;
    let mut w = World::new(42, cfg);
    assert!(citysim::systems::wages::on(&w));
    let city = city_camp(&w);
    for k in children(&mut w, 6) {
        if let Some(i) = w.comp_mut::<Identity>(k) {
            i.age_days = 12 * citysim::time::DAYS_PER_YEAR as u32;
        }
        if let Some(k2) = w.comp_mut::<Child>(k) {
            k2.hunger_days = 2;
        }
        assert_eq!(camp::take(&mut w, k), Some(city));
    }
    let per = w.config.economy2.input_per_part;
    let (ident, inputs0, world0) = (
        identity(&w),
        w.stats.current.econ.flow_inputs,
        w.outside.faction(citysim::outside::WORLD_ACCOUNT).map_or(0, |f| f.treasury),
    );
    let mut made = 0;
    for _ in 0..12 {
        let before = w.stock(city, Good::Parts);
        camp::daily(&mut w);
        made += w.stock(city, Good::Parts).saturating_sub(before);
        if made >= 2 {
            break;
        }
    }
    assert!(made >= 1, "Parts made in the shifts");
    let expect = (made as f32 * per).floor() as i64;
    let paid = w.stats.current.econ.flow_inputs - inputs0;
    assert!(paid >= expect - 1 && paid <= expect + 1, "inputs {paid} for {made} Parts × {per} (expected ~{expect})");
    assert_eq!(
        w.outside.faction(citysim::outside::WORLD_ACCOUNT).map_or(0, |f| f.treasury) - world0,
        paid,
        "crossed out to the World"
    );
    assert_eq!(identity(&w), ident, "the identity holds");
    assert!(w.comp::<Building>(city).is_some_and(|b| b.input_accum < 1.0), "the fraction carried");
}

fn world() -> World {
    World::new(42, config())
}

fn city_camp(w: &World) -> EntityId {
    camp::standing_camps(w).into_iter().find(|&c| w.owner_of(c).is_none()).expect("the city's camp")
}

fn home_of(w: &World, id: EntityId) -> Option<EntityId> {
    w.comp::<Household>(id).and_then(|h| h.home)
}

fn hunger_days(w: &World, c: EntityId) -> u8 {
    w.comp::<Child>(c).map_or(99, |k| k.hunger_days)
}

/// The seed has no children (`age_min_years` 18): a child born to two adult
/// residents of `h` (`demography::spawn_child`: Parent edges to both).
fn child_in(w: &mut World, h: EntityId) -> Option<EntityId> {
    let adults: Vec<EntityId> =
        w.residents_of(h).iter().copied().filter(|&a| demography::is_adult(w, a) && w.has::<Identity>(a)).collect();
    if adults.len() < 2 {
        return None;
    }
    Some(demography::spawn_child(w, adults[0], adults[1], h))
}

/// `n` children across the Blocks (one per Block), ascending Block id.
fn children(w: &mut World, n: usize) -> Vec<EntityId> {
    let mut out = Vec::new();
    for h in w.buildings_of_kind(BuildingKind::Home).to_vec() {
        if out.len() >= n {
            break;
        }
        if let Some(c) = child_in(w, h) {
            out.push(c);
        }
    }
    assert_eq!(out.len(), n, "{n} children");
    out
}

/// A Block with a child and two living parents among its residents.
/// Two children (a Block's child-food debt is `0.5 × children` a day, so
/// with two an empty pantry is an unfed day every day, as `children` reads it).
fn family_home(w: &mut World) -> (EntityId, EntityId) {
    for h in w.buildings_of_kind(BuildingKind::Home).to_vec() {
        if let Some(c) = child_in(w, h) {
            assert_eq!(camp::parents_of(w, c).len(), 2);
            child_in(w, h).expect("a sibling");
            return (h, c);
        }
    }
    panic!("a family");
}

/// Empty the Block's pantry and its adults' wallets (the broke home).
fn starve_home(w: &mut World, h: EntityId) {
    if let Some(b) = w.comp_mut::<Building>(h) {
        b.stock_food = 0;
    }
    for r in w.residents_of(h).to_vec() {
        if let Some(x) = w.comp_mut::<Wallet>(r) {
            x.coins = 0;
        }
    }
}

/// Run `days` days, starving `home` at the end of each (before midnight's pass).
fn run_days_starving(w: &mut World, days: u64, home: Option<EntityId>) {
    // `World::tick` runs the systems at `tick` and then advances it: a
    // day's midnight pass is the call that runs tick `1440 × n`.
    for _ in 0..days {
        while w.tick_of_day() as u64 != TICKS_PER_DAY - 1 {
            citysim::tick(w);
        }
        if let Some(h) = home {
            starve_home(w, h);
        }
        citysim::tick(w);
        citysim::tick(w);
    }
}

/// E37: the service's own Camp stands at seed on the city's deed with its
/// state and no staff role.
#[test]
fn test_seed_city_camp() {
    let w = world();
    let c = city_camp(&w);
    let b = w.comp::<Building>(c).expect("camp");
    assert_eq!(b.kind, BuildingKind::Camp);
    assert!(b.owner.is_none() && b.camp.is_some());
    assert!(!w.vacancies.contains_key(&c), "no staff role");
    assert!(camp::children_of(&w, c).is_empty());
}

/// E38 (plan 3c.6): a Block with an empty pantry: the child is at the Camp
/// on day 2 (`take_days`), alive on day 5, `ChildTaken` logged with a Life
/// row, each parent holds a `ChildTaken` memory and a grudge on the Law.
#[test]
fn test_unfed_child_taken_at_take_days_before_death() {
    let mut w = world();
    let (h, c) = family_home(&mut w);
    let parents = camp::parents_of(&w, c);
    let camp_b = city_camp(&w);
    run_days_starving(&mut w, 1, Some(h));
    assert_eq!(hunger_days(&w, c), 1);
    assert_eq!(home_of(&w, c), Some(h));
    run_days_starving(&mut w, 1, Some(h));
    assert_eq!(home_of(&w, c), Some(camp_b), "taken at two unfed days");
    assert!(camp::children_of(&w, camp_b).contains(&c));
    assert_eq!(w.comp::<Position>(c).and_then(|p| p.building), Some(camp_b));
    assert!(!w.residents_of(h).contains(&c), "out of the Block's residents");
    // The take is the new day's midnight pass: its row is `stats.current`.
    assert!(w.stats.current.econ.children_taken >= 1);
    let taken = w.events.iter().find(|e| e.kind == citysim::EventKind::ChildTaken).expect("ChildTaken");
    assert_eq!(taken.actors[0], c);
    assert!(w.comp::<citysim::Life>(c).is_some_and(|l| l.events.iter().any(|e| e.kind == citysim::LifeKind::Taken)));
    let law = w.building_of_kind(BuildingKind::Jail).expect("jail");
    for &p in &parents {
        assert!(
            w.comp::<Memory>(p).is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::ChildTaken)),
            "a parent remembers"
        );
        assert!(grudges::holds(&w, p, law, 0.5), "a grudge on the Law");
        assert!(w.comp::<Grudges>(p).is_some_and(|g| g.list.iter().any(|x| x.cause == GrudgeCause::ChildTaken)));
        assert!(w
            .comp::<citysim::Life>(p)
            .is_some_and(|l| l.events.iter().any(|e| e.kind == citysim::LifeKind::ChildTaken)));
    }
    // Alive on day 5, fed at the camp (the hunger debt cleared).
    run_days_starving(&mut w, 3, Some(h));
    assert!(w.has::<Identity>(c) && w.has::<Child>(c), "alive at the camp");
    assert_eq!(hunger_days(&w, c), 0, "the camp fed tonight's debt first");
    // (The identity over days is phase 1's census; the wallets the test
    // empties are not a flow. The custody purchase's booking is
    // `test_camp_feeds_from_owner_purchase_and_city_camp_from_treasury`.)
}

/// E39 (plan 3c.6): the city's Camp restocks from the Treasury
/// (`Flow::CampFood`); a corp's Camp from its own treasury
/// (`Flow::Wholesale`); both feed.
#[test]
fn test_camp_feeds_from_owner_purchase_and_city_camp_from_treasury() {
    let mut w = world();
    let (h, c) = family_home(&mut w);
    let city = city_camp(&w);
    // A corp's camp, nearer than the city's to this Block: on the Lot
    // nearest the Block (the corp pays nothing here: a hand-built camp).
    let corp = w.corps()[0];
    if let Some(cc) = w.comp_mut::<citysim::Corp>(corp) {
        cc.treasury = 5_000;
    }
    let door = w.comp::<Building>(h).map(|b| b.door).expect("door");
    let lot = founding::nearest_lot(&w, door).expect("a Lot");
    let corp_camp = founding::build_on_lot(&mut w, lot, BuildingKind::Camp, Some(corp)).expect("built");
    assert!(w.comp::<Building>(corp_camp).is_some_and(|b| b.camp.is_some()));
    let t0 = w.purse(Some(corp));
    run_days_starving(&mut w, 2, Some(h));
    let at = home_of(&w, c).expect("a camp");
    assert!(at == corp_camp || at == city, "taken to a camp");
    let day_food = w.stats.history.back().map(|r| r.econ.flow_camp_food).unwrap_or(0);
    let _ = day_food;
    run_days_starving(&mut w, 1, Some(h));
    assert_eq!(hunger_days(&w, c), 0, "fed");
    if at == corp_camp {
        assert!(w.purse(Some(corp)) < t0, "the corp paid the Market");
        assert!(w.stats.history.back().is_some_and(|r| r.flow_wholesale > 0));
    } else {
        assert!(w.stats.history.back().is_some_and(|r| r.econ.flow_camp_food > 0), "the Treasury paid");
    }
    // The city camp's own purchase: house a second child there directly.
    let (_, c2) = family_home(&mut w);
    let treasury0 = w.purse(None);
    // Fill the corp camp so the city's takes the child (custody never refuses).
    if let Some(s) = w.comp_mut::<Building>(corp_camp).and_then(|b| b.camp.as_mut()) {
        s.closed_by_law = true;
    }
    if let Some(k) = w.comp_mut::<Child>(c2) {
        k.hunger_days = 2;
    }
    camp::take_daily(&mut w);
    assert_eq!(home_of(&w, c2), Some(city));
    camp::daily(&mut w);
    assert_eq!(hunger_days(&w, c2), 0);
    assert!(w.stats.current.econ.flow_camp_food > 0, "CampFood from the Treasury");
    assert!(w.purse(None) <= treasury0);
}

/// E40 (plan 3c.6): the shift: children past `work_age` make Parts into
/// the camp's stock; the parts market buys them from the camp and the
/// owner (the city) is paid.
#[test]
fn test_camp_shift_makes_parts_and_owner_sells_them() {
    let mut w = world();
    let city = city_camp(&w);
    // Six children of 12 at the camp.
    for k in children(&mut w, 6) {
        if let Some(i) = w.comp_mut::<Identity>(k) {
            i.age_days = 12 * citysim::time::DAYS_PER_YEAR as u32;
        }
        if let Some(k2) = w.comp_mut::<Child>(k) {
            k2.hunger_days = 2;
        }
        assert_eq!(camp::take(&mut w, k), Some(city));
    }
    assert_eq!(camp::children_of(&w, city).len(), 6);
    let skill0: f32 = camp::children_of(&w, city).iter().map(|&k| w.comp::<Skills>(k).map_or(0.0, |s| s.farming)).sum();
    let mut parts = 0;
    for _ in 0..6 {
        camp::daily(&mut w);
        parts = w.stock(city, Good::Parts);
        if parts > 0 {
            break;
        }
    }
    assert!(parts > 0, "Parts in stock after the shifts");
    assert!(w.stats.current.econ.camp_output > 0);
    let skill1: f32 = camp::children_of(&w, city).iter().map(|&k| w.comp::<Skills>(k).map_or(0.0, |s| s.farming)).sum();
    assert!(skill1 > skill0, "the shift teaches farming");
    // A Clinic short of Parts buys at the parts market; the camp is a source
    // before the Recycler: its stock falls and the Treasury is paid.
    let clinic = *w.buildings_of_kind(BuildingKind::Clinic).first().expect("a Clinic");
    let taken = w.stock(clinic, Good::Parts);
    w.take_stock(clinic, Good::Parts, taken);
    // Empty the gangs' and Fabs' Parts so the camp is the first source with stock.
    for g in w.gangs() {
        if let Some(hd) = w.hideout_of(g) {
            let n = w.stock(hd, Good::Parts);
            w.take_stock(hd, Good::Parts, n);
        }
    }
    for f in w.buildings_of_kind(BuildingKind::Fab).to_vec() {
        let n = w.stock(f, Good::Parts);
        w.take_stock(f, Good::Parts, n);
    }
    let treasury0 = w.purse(None);
    let flow0 = w.stats.current.flow_parts;
    assets::parts_market(&mut w);
    assert!(w.stock(city, Good::Parts) < parts, "the camp sold");
    assert!(w.stats.current.flow_parts > flow0);
    let buyer_city = w.owner_of(clinic).is_none();
    if !buyer_city {
        assert!(w.purse(None) > treasury0, "the city, the camp's owner, was paid");
    }
}

/// E42 (plan 3c.6): a corp camp whose owner cannot buy food is a scandal
/// (a Feed story, the standing hit at the rebuild) and the law closes it
/// before the third unfed day, its children moved to the city's camp and
/// fed there; `CampClosed` logged; the closed camp takes nobody.
#[test]
fn test_unfed_camp_is_scandal_then_closed_and_children_moved() {
    let mut w = world();
    let city = city_camp(&w);
    let corp = w.corps()[0];
    let door = w.comp::<Building>(city).map(|b| b.door).expect("door");
    let lot = founding::nearest_lot(&w, door).expect("a Lot");
    let bad = founding::build_on_lot(&mut w, lot, BuildingKind::Camp, Some(corp)).expect("built");
    // Three children housed there directly, fed today (the city's camp
    // closed for the takes so the placement is the corp's).
    let kids = children(&mut w, 3);
    for &k in &kids {
        if let Some(k2) = w.comp_mut::<Child>(k) {
            k2.hunger_days = 2;
        }
        if let Some(s) = w.comp_mut::<Building>(city).and_then(|b| b.camp.as_mut()) {
            s.closed_by_law = true;
        }
        assert_eq!(camp::take(&mut w, k), Some(bad));
        if let Some(s) = w.comp_mut::<Building>(city).and_then(|b| b.camp.as_mut()) {
            s.closed_by_law = false;
        }
        if let Some(k2) = w.comp_mut::<Child>(k) {
            k2.hunger_days = 0;
        }
    }
    assert_eq!(camp::children_of(&w, bad).len(), 3);
    // The owner is broke and the shelf empty.
    if let Some(cc) = w.comp_mut::<citysim::Corp>(corp) {
        cc.treasury = 0;
    }
    if let Some(b) = w.comp_mut::<Building>(bad) {
        b.stock_food = 0;
    }
    let stories0 = w.events.iter().filter(|e| e.kind == citysim::EventKind::Story).count();
    camp::daily(&mut w);
    let s1 = w.comp::<Building>(bad).and_then(|b| b.camp.clone()).expect("state");
    assert_eq!(s1.unfed_today, 3);
    assert_eq!(s1.scandal_days, 1);
    assert!(!s1.closed_by_law);
    assert!(kids.iter().all(|&k| hunger_days(&w, k) == 1));
    assert!(w.events.iter().filter(|e| e.kind == citysim::EventKind::Story).count() > stories0, "a Feed story");
    assert!(camp::scandal_penalty(&w, Some(corp)) < 0.0, "the standing hit");
    assert_eq!(w.stats.current.econ.camp_scandals, 1);
    camp::daily(&mut w);
    let s2 = w.comp::<Building>(bad).and_then(|b| b.camp.clone()).expect("state");
    assert!(s2.closed_by_law, "closed before the third unfed day");
    assert!(s2.children.is_empty(), "the children moved");
    assert!(kids.iter().all(|&k| home_of(&w, k) == Some(city)), "to the city's camp");
    assert!(kids.iter().all(|&k| w.has::<Identity>(k)), "alive");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::CampClosed));
    assert!(w.is_closed(bad));
    // The next camp day: the city's camp feeds them.
    camp::daily(&mut w);
    assert!(kids.iter().all(|&k| hunger_days(&w, k) == 0), "fed at the city's camp");
    // A closed camp takes nobody.
    let (_, c) = family_home(&mut w);
    let _ = &kids;
    if let Some(k2) = w.comp_mut::<Child>(c) {
        k2.hunger_days = 2;
    }
    assert_eq!(camp::take(&mut w, c), Some(city));
}

/// E41 (plan 3c.6): a camp child aged to `ADULT_AGE_DAYS` is released with
/// `CampRaised`, homeless at the camp's door, its parents' affinity at
/// `family_affinity`, its farming above its start; a well-fed stay earns
/// loyalty; `CampRaised` logged with a Life row.
#[test]
fn test_release_at_adulthood_with_camp_raised() {
    let mut w = world();
    let (_, c) = family_home(&mut w);
    let parents = camp::parents_of(&w, c);
    let city = city_camp(&w);
    if let Some(k) = w.comp_mut::<Child>(c) {
        k.hunger_days = 2;
    }
    assert_eq!(camp::take(&mut w, c), Some(city));
    if let Some(i) = w.comp_mut::<Identity>(c) {
        i.age_days = demography::ADULT_AGE_DAYS - 3;
    }
    let farming0 = w.comp::<Skills>(c).map_or(0.0, |s| s.farming);
    let loyalty0 = w.comp::<citysim::Personality>(c).map_or(0.0, |p| p.loyalty);
    run_days_starving(&mut w, 4, None);
    assert!(demography::is_adult(&w, c));
    assert!(!w.has::<Child>(c));
    let trait_ = w.comp::<CampRaised>(c).cloned().expect("CampRaised");
    assert_eq!(trait_.camp, city);
    assert!(trait_.owner.is_none());
    assert!(trait_.fed_share >= w.config.camp.loyal_share, "fed every day: {}", trait_.fed_share);
    assert_eq!(home_of(&w, c), None, "homeless at the door");
    assert!(!camp::children_of(&w, city).contains(&c));
    assert!(w.comp::<Skills>(c).is_some_and(|s| s.farming > farming0), "farming above its start");
    assert!(w.comp::<citysim::Personality>(c).is_some_and(|p| p.loyalty > loyalty0 || loyalty0 >= 1.0));
    for p in parents {
        assert!((w.edge(c, p).map_or(0.0, |e| e.affinity) - w.config.camp.family_affinity).abs() < 1e-6);
    }
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::CampRaised && e.actors[0] == c));
    assert!(w
        .comp::<citysim::Life>(c)
        .is_some_and(|l| l.events.iter().any(|e| e.kind == citysim::LifeKind::CampRaised)));
    assert!(w.stats.history.iter().any(|r| r.econ.camp_released == 1));
    // A badly fed stay leaves a grudge on the owner (the Law for the city).
    let (_, c2) = family_home(&mut w);
    if let Some(k) = w.comp_mut::<Child>(c2) {
        k.hunger_days = 2;
    }
    assert_eq!(camp::take(&mut w, c2), Some(city));
    if let Some(s) = w.comp_mut::<Building>(city).and_then(|b| b.camp.as_mut()) {
        if let Some(e) = s.arrived.iter_mut().find(|(k, _)| *k == c2) {
            e.1 = e.1.saturating_sub(10);
        }
    }
    if let Some(i) = w.comp_mut::<Identity>(c2) {
        i.age_days = demography::ADULT_AGE_DAYS - 1;
    }
    run_days_starving(&mut w, 1, None);
    let t2 = w.comp::<CampRaised>(c2).cloned().expect("CampRaised");
    assert!(t2.fed_share < w.config.camp.grudge_share, "fed 1 of 11 days: {}", t2.fed_share);
    let law = w.building_of_kind(BuildingKind::Jail).expect("jail");
    assert!(grudges::holds(&w, c2, law, 0.5), "a grudge on the Law");
}

/// E38 (plan 3c.6): a `ChildTaken` memory is not a deed: `memory::deed_of`
/// reads `None`, so no killing rumour spreads from a take.
#[test]
fn test_child_taken_memory_spreads_no_killed_deed() {
    let mut w = world();
    let (_, c) = family_home(&mut w);
    let parents = camp::parents_of(&w, c);
    if let Some(k) = w.comp_mut::<Child>(c) {
        k.hunger_days = 2;
    }
    camp::take(&mut w, c).expect("taken");
    let p = parents[0];
    let m = w.comp::<Memory>(p).expect("memory");
    let e = m.entries.iter().find(|e| e.kind == MemoryKind::ChildTaken).expect("the memory");
    assert!(citysim::systems::memory::deed_of(p, e).is_none());
    assert!(e.salience >= 0.9 && e.valence <= -0.9, "Grief-class");
}

/// E37: corps found a camp when every camp is `found_full` full (the
/// richest Tech or Food corp with `3 × found_cost.camp`), paying the cost.
#[test]
fn test_corps_found_a_camp_when_full() {
    let mut w = world();
    let city = city_camp(&w);
    let cap = w.comp::<Building>(city).map_or(0, |b| usize::from(b.capacity));
    for k in children(&mut w, cap) {
        if let Some(k2) = w.comp_mut::<Child>(k) {
            k2.hunger_days = 2;
        }
        camp::take(&mut w, k);
    }
    assert!(camp::children_of(&w, city).len() >= cap);
    let before = camp::standing_camps(&w).len();
    let id0 = identity(&w);
    camp::found_daily(&mut w);
    assert_eq!(camp::standing_camps(&w).len(), before + 1, "a corp built a camp");
    let new = camp::standing_camps(&w).into_iter().find(|&c| w.owner_of(c).is_some()).expect("corp camp");
    assert!(w.owner_of(new).is_some_and(|o| w.has::<citysim::Corp>(o)));
    assert_eq!(identity(&w), id0);
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Founded && e.text.contains("a work camp")));
}

/// Review fix (E38, E39): a corp camp with an empty purse and shelf cannot
/// feed tonight, so the take goes to the city's camp (which buys from the
/// Treasury) even when the corp's is nearer; funded, the nearer corp camp
/// takes the next child and feeds it that night.
#[test]
fn test_take_prefers_a_camp_that_can_feed_tonight() {
    let mut w = world();
    let (h, c) = family_home(&mut w);
    let city = city_camp(&w);
    let corp = w.corps()[0];
    let door = w.comp::<Building>(h).map(|b| b.door).expect("door");
    let lot = founding::nearest_lot(&w, door).expect("a Lot");
    let bad = founding::build_on_lot(&mut w, lot, BuildingKind::Camp, Some(corp)).expect("built");
    if let Some(cc) = w.comp_mut::<citysim::Corp>(corp) {
        cc.treasury = 0;
    }
    assert!(!camp::can_feed_tonight(&w, bad, 1));
    assert!(camp::can_feed_tonight(&w, city, 1));
    if let Some(k) = w.comp_mut::<Child>(c) {
        k.hunger_days = 2;
    }
    assert_eq!(camp::take(&mut w, c), Some(city), "the broke camp is passed over");
    camp::daily(&mut w);
    assert_eq!(hunger_days(&w, c), 0, "fed at the city's camp that night");
    assert!(w.has::<Child>(c));
    // Funded: the nearer corp camp can feed and takes the next child.
    if let Some(cc) = w.comp_mut::<citysim::Corp>(corp) {
        cc.treasury = 5_000;
    }
    assert!(camp::can_feed_tonight(&w, bad, 1));
    let (_, c2) = family_home(&mut w);
    if let Some(k) = w.comp_mut::<Child>(c2) {
        k.hunger_days = 2;
    }
    let placed = camp::take(&mut w, c2).expect("taken");
    let d = |b: EntityId| w.comp::<Building>(b).map(|x| x.door.manhattan(door)).unwrap_or(u32::MAX);
    assert_eq!(placed, if d(bad) <= d(city) { bad } else { city });
    camp::daily(&mut w);
    assert_eq!(hunger_days(&w, c2), 0, "fed that night");
}

/// Review fix: when no camp can feed (every Market empty) the take still
/// happens, nothing is bought (no `CampFood` line), the child the service
/// could not feed dies, the day counts it as `camp_unfed` and the Feeds carry
/// a bulletin naming the service.
#[test]
fn test_no_camp_can_feed_counts_the_death_and_tells_the_feeds() {
    let mut w = world();
    let (h, c) = family_home(&mut w);
    let city = city_camp(&w);
    for m in w.buildings_of_kind(BuildingKind::Market).to_vec() {
        if let Some(b) = w.comp_mut::<Building>(m) {
            b.stock_food = 0;
        }
    }
    if let Some(b) = w.comp_mut::<Building>(city) {
        b.stock_food = 0;
    }
    assert!(!camp::can_feed_tonight(&w, city, 1));
    // Both of the Block's children (a lone child's half ration needs no whole
    // unit on alternate days, the Block's debt shape).
    let kids: Vec<EntityId> = w.residents_of(h).iter().copied().filter(|&k| w.has::<Child>(k)).collect();
    assert_eq!(kids.len(), 2);
    for &k in &kids {
        if let Some(x) = w.comp_mut::<Child>(k) {
            x.hunger_days = 2;
        }
        assert_eq!(camp::take(&mut w, k), Some(city), "custody never refuses");
    }
    let food0 = w.stats.current.econ.flow_camp_food;
    camp::daily(&mut w);
    assert_eq!(w.stats.current.econ.flow_camp_food, food0, "nothing bought from empty shelves");
    assert!(!w.has::<Child>(c) && w.has::<citysim::Corpse>(c), "the third unfed day killed");
    assert!(w.stats.current.econ.camp_unfed >= 1);
    assert!(w
        .events
        .iter()
        .any(|e| e.kind == citysim::EventKind::Story && e.text.contains("could not feed") && e.actors.contains(&c)));
}
