//! M17 phase 1 (docs/M17_OUTSIDE.md, "Revised 2026-10-12"): Ambient
//! consumption as outside demand. The Ambient flats' households buy food at
//! simulated Markets and visit venues with World money: export demand at
//! retail, revenue the wage law and Grow read. The shipped city has no
//! Ambient flats, so no demand; demand is linear in Ambient adults up to a
//! per-capita cap; a Market never sells to the book below its keep; a sale
//! is revenue and crosses from the World (the coin identity holds); the
//! switch off sells nothing; a save resumes byte for byte.

use citysim::systems::{econ, outside};
use citysim::{save, Building, BuildingKind, Config, Corp, EntityId, Market, World, TICKS_PER_DAY};

const HOUR: u64 = TICKS_PER_DAY / 24;

/// The shipped city with `per_slot` Ambient flats per household slot.
fn ambient_world(seed: u64, per_slot: f32, tune: impl FnOnce(&mut Config)) -> World {
    let mut c = Config::load();
    c.ambient.flats_per_slot = per_slot;
    tune(&mut c);
    World::new(seed, c)
}

fn due_food(w: &World) -> u32 {
    outside::ambient_book(w).map_or(0, |b| b.food_due.iter().sum())
}

fn due_visits(w: &World) -> u32 {
    outside::ambient_book(w).map_or(0, |b| b.visit_due.iter().sum())
}

#[test]
fn test_shipped_city_has_no_ambient_book() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY + 10 * HOUR);
    assert!(!outside::ambient_on(&w));
    assert!(outside::ambient_book(&w).is_none(), "no Ambient flats, no book");
    let ron = save::to_ron(&w);
    assert!(!ron.contains("AmbientSale") && !ron.contains("food_due"), "nothing of the book in a 2k save");
}

#[test]
fn test_hour_shares_sum_to_the_due() {
    for due in 0..60u32 {
        for n in 1..14u32 {
            let parts: Vec<u32> = (0..n).map(|k| outside::hour_share(due, n, k)).collect();
            assert_eq!(parts.iter().sum::<u32>(), due, "due {due} over {n} hours: {parts:?}");
            let (lo, hi) = (parts.iter().min().copied().unwrap_or(0), parts.iter().max().copied().unwrap_or(0));
            assert!(hi - lo <= 1, "even spread: {parts:?}");
        }
    }
}

#[test]
fn test_apportion_is_exact_and_proportional() {
    assert_eq!(outside::apportion(10, &[1.0, 1.0, 2.0]), vec![3, 2, 5]);
    assert_eq!(outside::apportion(0, &[1.0, 3.0]), vec![0, 0]);
    assert_eq!(outside::apportion(7, &[0.0, 0.0]), vec![0, 0]);
    let w = [0.3, 2.7, 0.0, 5.1, 1.9];
    for total in 0..200 {
        let a = outside::apportion(total, &w);
        assert_eq!(a.iter().sum::<u32>(), total);
        assert_eq!(a[2], 0, "a zero weight gets nothing");
    }
}

#[test]
fn test_caps_scale_per_capita() {
    let c = Config::load();
    let s = Config::load().scaled_to(20_000);
    assert_eq!(s.outside.ambient.food_cap, 10 * c.outside.ambient.food_cap);
    assert_eq!(s.outside.ambient.visit_cap, 10 * c.outside.ambient.visit_cap);
    let small = Config::load().scaled_to(700);
    assert_eq!(small.outside.ambient.food_cap, c.outside.ambient.food_cap, "up only");
}

/// Twice the flats, about twice the demand (under the cap); a small cap binds.
#[test]
fn test_demand_linear_until_the_cap() {
    let open = |c: &mut Config| {
        c.outside.ambient.food_cap = 1_000_000;
        c.outside.ambient.visit_cap = 1_000_000;
        c.outside.ambient.sd = 0.0;
    };
    let mut w1 = ambient_world(42, 1.0, open);
    let mut w2 = ambient_world(42, 2.0, open);
    w1.run_ticks(TICKS_PER_DAY + 9 * HOUR);
    w2.run_ticks(TICKS_PER_DAY + 9 * HOUR);
    let (f1, f2) = (due_food(&w1), due_food(&w2));
    let (v1, v2) = (due_visits(&w1), due_visits(&w2));
    println!("food due {f1} -> {f2}; visits due {v1} -> {v2}");
    assert!(f1 > 0 && v1 > 0, "a city with flats has demand on day 1");
    let r = f64::from(f2) / f64::from(f1);
    assert!((1.7..=2.3).contains(&r), "food demand linear in the flats: x{r:.2}");
    // Jobs & lives phase 7b: the visit cap below the uncapped demand (the
    // off-screen evening's paid visits mirror the Full tier's rates since,
    // and day 0's visits per resident fell: the 2.0-flat city's uncapped
    // visits due are 3, under the old cap of 4; Shadow V4 batch 1, no
    // leisure in a day worker's small hours: 2, under a cap of 1).
    let mut w3 = ambient_world(42, 2.0, |c| {
        c.outside.ambient.food_cap = 10;
        c.outside.ambient.visit_cap = 1;
        c.outside.ambient.sd = 0.0;
    });
    w3.run_ticks(TICKS_PER_DAY + 9 * HOUR);
    assert!(v2 > 1, "the cap is under the uncapped demand ({v2})");
    assert_eq!(due_food(&w3), 10, "the cap binds");
    assert_eq!(due_visits(&w3), 1, "the cap binds");
}

/// A standing Market with a sales history, its owner.
fn market_with_history(w: &World) -> EntityId {
    w.buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find(|&m| w.comp::<Market>(m).is_some_and(|mk| !mk.sales.is_empty() && mk.sales.iter().sum::<u32>() > 0))
        .expect("a Market with sales")
}

#[test]
fn test_sale_never_goes_below_the_keep() {
    let mut w = ambient_world(42, 1.0, |_| {});
    w.run_ticks(2 * TICKS_PER_DAY + 7 * HOUR);
    let m = market_with_history(&w);
    let mk = w.comp::<Market>(m).expect("market");
    let mean = f64::from(mk.sales.iter().sum::<u32>()) / mk.sales.len() as f64;
    let keep = (f64::from(w.config.outside.ambient.keep_days) * mean).ceil() as u32;
    w.comp_mut::<Building>(m).expect("market").stock_food = keep + 3;
    assert_eq!(outside::food_surplus(&w, m), 3);
    assert_eq!(outside::sell_food_at(&mut w, m, 10), 3, "only the surplus");
    assert_eq!(w.comp::<Building>(m).expect("market").stock_food, keep);
    assert_eq!(outside::sell_food_at(&mut w, m, 10), 0, "nothing below the keep");
}

#[test]
fn test_sale_is_revenue_and_crosses_from_the_world() {
    let mut w = ambient_world(42, 1.0, |_| {});
    w.run_ticks(2 * TICKS_PER_DAY + 7 * HOUR);
    let m = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find(|&m| {
            w.owner_of(m).is_some_and(|o| w.has::<Corp>(o))
                && w.comp::<Market>(m).is_some_and(|mk| !mk.sales.is_empty())
        })
        .expect("a corp Market");
    let corp = w.owner_of(m).expect("owner");
    w.comp_mut::<Building>(m).expect("market").stock_food += 500;
    let before = econ::identity(&w);
    let world_before = w.outside.faction(citysim::outside::WORLD_ACCOUNT).expect("the World").treasury;
    let (rev0, sales0, credit0, stock0) = (
        w.comp::<Corp>(corp).expect("corp").rev_today,
        w.comp::<Market>(m).expect("market").sales_today,
        w.comp::<Building>(m).expect("market").revenue_today,
        w.comp::<Building>(m).expect("market").stock_food,
    );
    let paid0 = w.ambient.today.paid_food;
    let sold = outside::sell_food_at(&mut w, m, 5);
    assert_eq!(sold, 5);
    let paid = w.ambient.today.paid_food - paid0;
    assert!(paid >= 5, "at the shelf price: {paid}");
    assert_eq!(econ::identity(&w), before, "coins made or lost");
    assert_eq!(w.outside.faction(citysim::outside::WORLD_ACCOUNT).expect("the World").treasury, world_before - paid);
    assert_eq!(w.comp::<Market>(m).expect("market").sales_today, sales0 + 5, "a sale on the shelf's books");
    assert_eq!(w.comp::<Building>(m).expect("market").stock_food, stock0 - 5, "real goods");
    assert_eq!(w.comp::<Building>(m).expect("market").revenue_today, credit0 + paid);
    assert!(w.comp::<Corp>(corp).expect("corp").rev_today > rev0, "revenue the wage law reads");
    assert_eq!(w.ambient.today.food_bought, 5);
}

/// The switch off: no book, no sale, on the same map.
#[test]
fn test_switch_off_sells_nothing() {
    let mut w = ambient_world(42, 2.0, |c| c.outside.enabled = false);
    w.run_ticks(3 * TICKS_PER_DAY + 1);
    assert!(outside::ambient_book(&w).is_none());
    let bought: u32 = w.stats.history.iter().filter_map(|r| r.ambient.as_ref().map(|a| a.food_bought)).sum();
    assert_eq!(bought, 0);
}

/// On: the book buys and pays, the identity holds every day, and the
/// CSV group carries it.
#[test]
fn test_ambient_sales_happen_and_the_books_balance() {
    let mut w = ambient_world(42, 2.0, |_| {});
    let base = econ::identity(&w);
    for _ in 0..4 {
        w.run_ticks(TICKS_PER_DAY);
        assert_eq!(econ::identity(&w), base, "coins made or lost on day {}", w.day());
    }
    let rows: Vec<_> = w.stats.history.iter().filter_map(|r| r.ambient.clone()).collect();
    for r in &rows {
        println!(
            "food {}/{} visits {}/{} paid {} + {} appetite {:.2}",
            r.food_bought, r.food_wanted, r.visits_made, r.visits_wanted, r.paid_food, r.paid_visits, r.appetite
        );
        assert!(r.food_bought <= r.food_wanted && r.visits_made <= r.visits_wanted);
    }
    assert!(rows.iter().map(|r| r.food_bought).sum::<u32>() > 0, "the book bought food");
    assert!(rows.iter().map(|r| r.paid_food + r.paid_visits).sum::<i64>() > 0, "and paid for it");
    let header = citysim::systems::stats::csv_header(&w);
    assert!(header.contains("ambient_food_bought") && header.contains("flow_ambient"));
    let book = outside::ambient_book(&w).expect("the book");
    assert_eq!(book.units.len(), rows.len(), "a ring entry per closed day");
}

/// `SetAmbientAppetite` pins and releases.
#[test]
fn test_appetite_lever_pins_and_releases() {
    let mut w = ambient_world(42, 1.0, |_| {});
    w.run_ticks(TICKS_PER_DAY + 9 * HOUR);
    w.push_command(citysim::PlayerCommand::SetAmbientAppetite(Some(2.5)));
    w.apply_commands();
    assert_eq!(outside::ambient_book(&w).expect("book").appetite, 2.5);
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(outside::ambient_book(&w).expect("book").appetite, 2.5, "the walk does not move a pin");
    w.push_command(citysim::PlayerCommand::SetAmbientAppetite(None));
    w.apply_commands();
    w.run_ticks(TICKS_PER_DAY);
    assert_ne!(outside::ambient_book(&w).expect("book").appetite, 2.5, "released to the walk");
}
