//! The Real economy phase 1 (docs/ECONOMY_V2.md § 3; plan 1.10): the World
//! market's mechanisms (the tranche bid, the appetite walk, the haul's
//! choice, Market imports at the ask, `cross_out`, the identity to the
//! coin, the import refactor's off path, the leak bookings, the off world).

use citysim::components::{hole_id, Hole, HoleKind};
use citysim::econ::WorldBook;
use citysim::outside::{ExportGood, WORLD_ACCOUNT};
use citysim::systems::ownership::{self, Flow, ImportWhy};
use citysim::systems::{bind, econ, gang, world_market as wm};
use citysim::{Building, BuildingKind, Config, Corp, Market, Wallet, World, TICKS_PER_DAY};

fn city() -> World {
    let w = World::new(42, Config::load());
    assert!(econ::market_on(&w) && wm::open(&w), "the market is on and open by the config");
    w
}

fn world_treasury(w: &World) -> i64 {
    w.outside.faction(WORLD_ACCOUNT).map_or(0, |f| f.treasury)
}

/// E8: Σ over q of `bid(q)` equals the closed-form tranche to the coin; a
/// sale past `cap_today` stops at the cap; the floor holds.
#[test]
fn test_tranche_bid_matches_closed_form_and_cap() {
    let mut w = city();
    let cfg = w.config.world_market.clone();
    for g in ExportGood::ALL {
        let cap = wm::cap_today(&w, g);
        assert_eq!(cap, cfg.good(g).cap, "appetite 1.0 at seed: cap_today = cap for {}", g.label());
        for from in [0u32, 1, cap / 3, cap.saturating_sub(5), cap + 10] {
            for n in [1u32, 7, cap / 2, cap, cap * 3] {
                let sum: f64 = (from..from + n).map(|q| wm::bid(&w, g, q)).sum();
                assert_eq!(sum.round() as i64, wm::tranche(&w, g, from, n), "{} from {from} n {n}", g.label());
                assert!((from..from + n).all(|q| wm::bid(&w, g, q) >= f64::from(cfg.good(g).bid_floor)), "floor");
            }
        }
        // The first unit's bid is the reference, the last of the day a slope below it.
        assert!((wm::bid(&w, g, 0) - f64::from(cfg.good(g).bid_ref)).abs() < 1e-6);
        assert!(wm::bid(&w, g, cap.max(1) - 1) < wm::bid(&w, g, 0));
        // A deep tranche is capped at the floor, not below.
        assert!(wm::bid(&w, g, cap * 10) == f64::from(cfg.good(g).bid_floor));
    }
    // A sale past the cap stops at the cap, the next sale today sells nothing.
    let corp = w.corps()[0];
    let before = (w.purse(Some(corp)), world_treasury(&w), econ::identity(&w), w.stats.current.flow_tax);
    let cap = wm::cap_today(&w, ExportGood::Food);
    let (units, paid) = wm::sell(&mut w, None, Some(corp), ExportGood::Food, cap * 2);
    assert_eq!(units, cap, "capped at cap_today");
    assert_eq!(paid, wm::tranche(&w, ExportGood::Food, 0, cap));
    assert_eq!(wm::book(&w, ExportGood::Food).map(|b| b.bought_today), Some(cap));
    assert_eq!(wm::sell(&mut w, None, Some(corp), ExportGood::Food, 10), (0, 0), "the cap is spent");
    let tax = (w.levers.tax_rate * paid as f32).round() as i64;
    assert_eq!(w.purse(Some(corp)), before.0 + paid - tax, "the seller holds the sale net of its tax");
    assert_eq!(world_treasury(&w), before.1 - paid, "the World paid from its account");
    assert_eq!(econ::identity(&w), before.2, "the identity holds across a sale");
    assert_eq!(w.stats.current.living.flow_export, paid);
    assert_eq!(w.stats.current.flow_tax, before.3 + tax);
}

/// E7: pinned stays; 400 hashed days stay in [appetite_min, appetite_max]
/// and revert toward 1; two worlds of one seed walk identically.
#[test]
fn test_appetite_walk_reverts_and_clamps() {
    let cfg = Config::load().world_market;
    let seed = 42u64;
    let mut book = WorldBook::default();
    let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0f32);
    for day in 0..400u64 {
        let z = econ::hash_normal(seed, citysim::econ::PURPOSE_APPETITE, day, ExportGood::Food as u64);
        wm::appetite_step(&cfg, ExportGood::Food, &mut book, (day / 30 % 4) as usize, z);
        assert!((cfg.appetite_min..=cfg.appetite_max).contains(&book.appetite), "day {day}: {}", book.appetite);
        lo = lo.min(book.appetite);
        hi = hi.max(book.appetite);
        sum += book.appetite;
    }
    let mean = sum / 400.0;
    eprintln!("appetite over 400 days: min {lo:.3} max {hi:.3} mean {mean:.3}");
    assert!((0.7..1.3).contains(&mean), "reverts toward 1 (mean {mean})");
    assert!(hi - lo > 0.05, "the walk moves");
    // Pinned: the walk goes on underneath, the appetite stays.
    let mut pinned = WorldBook { appetite_pin: Some(2.0), ..WorldBook::default() };
    for day in 0..50u64 {
        let z = econ::hash_normal(seed, citysim::econ::PURPOSE_APPETITE, day, 0);
        wm::appetite_step(&cfg, ExportGood::Food, &mut pinned, 0, z);
        assert_eq!(pinned.appetite, 2.0);
    }
    assert_ne!(pinned.walk, 1.0, "the walk went on underneath");
    // Two worlds of one seed: identical books after four midnights (the
    // scaled city runs quickly; the hash reads only seed, day and good).
    let run = || {
        let mut w = World::new(7, Config::load().scaled_to(300));
        w.run_ticks(4 * TICKS_PER_DAY + 1);
        ExportGood::ALL.map(|g| wm::book(&w, g).cloned().unwrap_or_default())
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    assert!(a.iter().any(|bk| bk.walk != 1.0), "the walk moved: {a:?}");
    assert!(a.iter().all(|bk| bk.bought.len() == 4), "four days rolled: {a:?}");
}

/// A Farm with stock, its owner's Markets taken away so the haul's target
/// is a rival's (the city's) Market.
fn farm_with_rival_market(w: &mut World) -> (citysim::EntityId, Option<citysim::EntityId>, citysim::EntityId) {
    let farm = w
        .buildings_of_kind(BuildingKind::Farm)
        .iter()
        .copied()
        .find(|&f| w.owner_of(f).is_some_and(|o| w.has::<Corp>(o)))
        .expect("a corp Farm");
    let owner = w.owner_of(farm);
    for m in ownership::owned_of_kind(w, owner, BuildingKind::Market) {
        ownership::transfer_building(w, m, None);
    }
    if let Some(b) = w.comp_mut::<Building>(farm) {
        b.stock_food = 50;
    }
    let door = w.comp::<Building>(farm).map(|b| b.door).expect("door");
    let rival = w.nearest_of_kind(BuildingKind::Market, door).expect("a rival Market");
    assert_ne!(w.owner_of(rival), owner);
    (farm, owner, rival)
}

/// E9: the World's marginal bid above wholesale → the World; below → the
/// rival Market; the owner's own Market first either way.
#[test]
fn test_haul_sells_to_higher_of_world_and_rival() {
    let wholesale = Config::load().corps.wholesale;
    // (a) bid above wholesale: the World.
    let mut w = city();
    let (farm, owner, rival) = farm_with_rival_market(&mut w);
    assert!(wm::marginal_bid(&w, ExportGood::Food) > wholesale as f64);
    let (purse, rival_stock, ident) = (w.purse(owner), w.stock(rival, citysim::Good::Food), econ::identity(&w));
    let moved = citysim::systems::economy::haul(&mut w, farm, None);
    assert_eq!(moved, 50);
    assert_eq!(wm::book(&w, ExportGood::Food).map(|b| b.bought_today), Some(50), "the World took the batch");
    assert_eq!(w.stock(rival, citysim::Good::Food), rival_stock, "the rival Market got none");
    assert!(w.purse(owner) > purse, "the owner was paid");
    assert_eq!(econ::identity(&w), ident);
    // (b) bid below wholesale (appetite pinned at the floor): the rival Market.
    let mut w = city();
    let (farm, owner, rival) = farm_with_rival_market(&mut w);
    w.push_command(citysim::PlayerCommand::SetAppetite { good: ExportGood::Food, mult: Some(0.3) });
    w.run_ticks(1);
    assert!(wm::marginal_bid(&w, ExportGood::Food) < wholesale as f64, "{}", wm::marginal_bid(&w, ExportGood::Food));
    let (purse, rival_stock) = (w.purse(owner), w.stock(rival, citysim::Good::Food));
    citysim::systems::economy::haul(&mut w, farm, None);
    assert_eq!(w.stock(rival, citysim::Good::Food), rival_stock + 50, "the rival Market bought at wholesale");
    assert_eq!(wm::book(&w, ExportGood::Food).map(|b| b.bought_today), Some(0));
    assert_eq!(w.purse(owner), purse + 50 * wholesale - (w.levers.tax_rate * (50 * wholesale) as f32).floor() as i64);
    // (c) the owner's own Market first, whatever the bid.
    let mut w = city();
    let (farm, owner, _) = farm_with_rival_market(&mut w);
    let door = w.comp::<Building>(farm).map(|b| b.door).expect("door");
    let own = w.nearest_of_kind(BuildingKind::Market, door).expect("a Market");
    ownership::transfer_building(&mut w, own, owner);
    let stock = w.stock(own, citysim::Good::Food);
    citysim::systems::economy::haul(&mut w, farm, None);
    assert_eq!(w.stock(own, citysim::Good::Food), stock + 50, "the own Market takes the batch");
    assert_eq!(wm::book(&w, ExportGood::Food).map(|b| b.bought_today), Some(0));
}

/// E11, E12: a Market at stock 0 whose shelf would price at 30 imports
/// `restock_batch` at the ask (customs to the Treasury); a Market that
/// cannot afford to is clamped to `ask + haul_margin`.
#[test]
fn test_market_imports_at_ask_when_above_ceiling() {
    let mut w = city();
    let ask = wm::ask(&w, ExportGood::Food);
    let margin = w.config.world_market.haul_margin;
    let batch = w.config.economy.restock_batch;
    let floor = w.config.economy.restock_floor;
    assert_eq!(ask, 6, "the spec's ask at the placeholders (3.4 × 1.6, rounded up)");
    // At 23:59 (the day's hauls are in): the Reserve empty, two corp
    // Markets empty, one rich, one broke; then the midnight pass.
    w.run_ticks(TICKS_PER_DAY - 1);
    if let Some(wh) = w.building_of_kind(BuildingKind::Warehouse) {
        if let Some(b) = w.comp_mut::<Building>(wh) {
            b.stock_food = 0;
        }
    }
    let markets: Vec<_> = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter(|&m| w.owner_of(m).is_some_and(|o| w.has::<Corp>(o)))
        .collect();
    let (rich, broke) = (markets[0], markets[markets.len() - 1]);
    assert_ne!(w.owner_of(rich), w.owner_of(broke), "two corps");
    for m in [rich, broke] {
        if let Some(b) = w.comp_mut::<Building>(m) {
            b.stock_food = 0;
        }
    }
    let rich_owner = w.owner_of(rich).expect("corp");
    if let Some(c) = w.comp_mut::<Corp>(rich_owner) {
        c.treasury = 50_000;
        c.closing = 50_000;
    }
    let broke_owner = w.owner_of(broke).expect("corp");
    if let Some(c) = w.comp_mut::<Corp>(broke_owner) {
        c.treasury = -10_000;
        c.closing = -10_000;
    }
    let (treasury, world_t, ident) = (w.purse(None), world_treasury(&w), econ::identity(&w));
    let rich_purse = w.purse(Some(rich_owner));
    w.run_ticks(2);
    let want = batch.min(floor);
    let imported = wm::book(&w, ExportGood::Food).map_or(0, |b| b.sold_today);
    // (a last-minute haul may have put a few units in the Reserve)
    assert!(imported >= want - 100, "the rich Market imported its batch ({imported} ≥ {want} - 100)");
    let rich_stock = w.stock(rich, citysim::Good::Food);
    // (the midnight spoilage takes `spoilage_market` of the shelf)
    assert!(rich_stock >= floor - floor / 10, "on the shelf: {rich_stock}");
    let cost = i64::from(imported) * ask;
    let customs = (wm::customs_rate(&w) * cost as f32).round() as i64;
    assert!(w.purse(Some(rich_owner)) <= rich_purse - cost - customs, "paid the ask and customs");
    assert!(
        world_treasury(&w) > world_t - 50_000 || w.stats.current.econ.flow_import_out >= cost,
        "the coins went abroad"
    );
    let row = &w.stats.current.econ;
    assert!(row.flow_customs >= customs, "customs on the day's row ({} ≥ {customs})", row.flow_customs);
    assert!(row.flow_import_out >= cost);
    assert!(w.purse(None) >= treasury - 20_000, "the Treasury took customs, not the import");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Imported), "an Imported event");
    // The broke Market imported nothing and sits at the ceiling, not 30.
    assert_eq!(w.stock(broke, citysim::Good::Food), 0, "no coins, no import");
    let price = w.comp::<Market>(broke).map_or(0, |m| m.price_food);
    assert_eq!(price, ask + margin, "the shelf is capped at ask + margin");
    assert!(w.comp::<Market>(broke).is_some_and(|m| m.price_tenths <= (ask + margin) * 10));
    assert_eq!(citysim::systems::economy::price_ceiling(&w), Some(ask + margin));
    assert_eq!(econ::identity(&w), ident, "the identity holds across imports");
    // With the World closed the ceiling lifts and no Market imports.
    w.push_command(citysim::PlayerCommand::CloseWorld(true));
    w.run_ticks(1);
    assert_eq!(citysim::systems::economy::price_ceiling(&w), None);
}

/// E5, E24: a 20-day run: `coin_identity` is constant to the coin every
/// day and Σ `trade_balance` = Δ `total_coins`; migrants and imports cross.
#[test]
fn test_cross_out_and_identity_with_imports_and_migrants() {
    let mut w = city();
    let base = econ::identity(&w);
    let coins0 = ownership::total_coins(&w);
    for _ in 0..20 {
        w.run_ticks(TICKS_PER_DAY);
        let row = w.stats.history.back().expect("a day");
        assert_eq!(row.econ.coin_identity, base, "day {}: the identity to the coin", row.day);
        assert_eq!(econ::identity(&w), base);
        assert!(row.econ.world_treasury > 0);
    }
    let rows = &w.stats.history;
    let trade: i64 = rows.iter().map(|r| r.econ.trade_balance).sum();
    assert_eq!(trade, ownership::total_coins(&w) - coins0, "Σ trade_balance = Δ total_coins");
    assert_eq!(rows.back().map(|r| r.econ.trade_balance_30), Some(trade), "the 30-day ring over 20 days");
    assert!(rows.iter().any(|r| r.econ.flow_migrant_in > 0), "immigrants crossed in");
    assert!(rows.iter().all(|r| r.econ.flow_import_out >= 0) && rows.iter().any(|r| r.econ.flow_import_out > 0));
    assert!(
        rows.iter().any(|r| r.living.flow_export > 0)
            && rows.iter().all(|r| r.econ.export_paid == r.living.flow_export)
    );
    assert_eq!(w.outside.inbound - w.outside.outbound, trade, "inbound − outbound is the balance");
    assert!(w.outside.minted != w.config.world_market.treasury_ref, "the refill moved minted both ways");
    let last = rows.back().expect("a day");
    assert_eq!(last.econ.outside_outbound, w.outside.outbound);
    assert!(last.econ.appetite_food > 0.0 && last.econ.bid_food > 0.0 && last.econ.ask_food > 0);
}

/// E5: with the market off the six sites charge the City exactly; with it
/// on the coins cross out and customs land in the Treasury.
#[test]
fn test_import_off_is_charge_to_city() {
    let mut off = World::new(42, Config::load().econ_off());
    assert!(!econ::market_on(&off));
    let corp = off.corps()[0];
    let (purse, treasury) = (off.purse(Some(corp)), off.purse(None));
    let moved = ownership::import(&mut off, Some(corp), 100, ImportWhy::Asset);
    assert_eq!(moved, 100);
    assert_eq!(off.purse(Some(corp)), purse - 100);
    assert_eq!(off.purse(None), treasury + 100, "the City's customs, M13 D8");
    assert_eq!(off.stats.current.flow_import, 100);
    assert_eq!(off.stats.current.econ.flow_import_out, 0);
    assert!(off.outside.is_empty());
    let mut on = city();
    let corp = on.corps()[0];
    let (purse, treasury, world_t, ident) =
        (on.purse(Some(corp)), on.purse(None), world_treasury(&on), econ::identity(&on));
    let moved = ownership::import(&mut on, Some(corp), 100, ImportWhy::Asset);
    assert_eq!(moved, 100);
    assert_eq!(on.purse(Some(corp)), purse - 110, "the import and 10 % customs");
    assert_eq!(on.purse(None), treasury + 10);
    assert_eq!(world_treasury(&on), world_t + 100);
    assert_eq!(on.outside.outbound, 100);
    assert_eq!(on.stats.current.flow_import, 100);
    assert_eq!(on.stats.current.econ.flow_import_out, 100);
    assert_eq!(on.stats.current.econ.flow_customs, 10);
    assert_eq!(econ::identity(&on), ident);
    // An agent importer is capped at its purse (a gang's cook, M13).
    let agent = on.citizens().into_iter().find(|&a| on.has::<Wallet>(a)).expect("an agent");
    if let Some(x) = on.comp_mut::<Wallet>(agent) {
        x.coins = 30;
    }
    let ident = econ::identity(&on);
    let moved = ownership::import(&mut on, Some(agent), 100, ImportWhy::Stims);
    assert_eq!(moved, 30);
    assert!(on.purse(Some(agent)) <= 0 && on.purse(Some(agent)) >= -3, "the purse, customs capped by `charge`");
    assert_eq!(econ::identity(&on), ident);
}

/// E25a: a robbery's loot in flight is in `total_coins`; an Unknown binding
/// (an expiry) pays it to the Treasury instead of losing it.
#[test]
fn test_unknown_hole_loot_goes_to_treasury() {
    let mut w = city();
    let victim =
        w.citizens().into_iter().find(|&a| w.comp::<Wallet>(a).is_some_and(|x| x.coins >= 10)).expect("a victim");
    let tile = w.comp::<citysim::Position>(victim).map(|p| p.tile).expect("tile");
    let (zone, district) = (w.map.zone(tile), w.district_of(tile));
    let tick = w.tick;
    let loot = 10;
    if let Some(x) = w.comp_mut::<Wallet>(victim) {
        x.coins -= loot;
    }
    let before = (ownership::total_coins(&w), w.purse(None), econ::identity(&w));
    let hole = Hole {
        id: hole_id(tick, victim, HoleKind::Robbed),
        kind: HoleKind::Robbed,
        victim,
        zone,
        district,
        tick,
        event_id: 0,
        consequential: false,
        spouse: None,
        loot,
        home: None,
        gang: None,
        source: None,
        faction: None,
        riot: None,
    };
    let id = bind::open_hole(&mut w, hole);
    assert_eq!(ownership::total_coins(&w), before.0 + loot, "the loot in flight is counted");
    bind::expire(&mut w, id);
    assert_eq!(w.purse(None), before.1 + loot, "unclaimed property to the Treasury");
    assert_eq!(ownership::total_coins(&w), before.0 + loot);
    assert_eq!(econ::identity(&w), before.2 + loot, "the coins left the wallet before the probe's base");
    assert_eq!(w.probe.loot_lost, loot);
    // With the market off the coin vanishes, as at EC_BASE.
    let mut off = World::new(42, Config::load().econ_off());
    let tick = off.tick;
    let hole = Hole {
        id: hole_id(tick, victim, HoleKind::Robbed),
        kind: HoleKind::Robbed,
        victim,
        zone,
        district,
        tick,
        event_id: 0,
        consequential: false,
        spouse: None,
        loot,
        home: None,
        gang: None,
        source: None,
        faction: None,
        riot: None,
    };
    let treasury = off.purse(None);
    let id = bind::open_hole(&mut off, hole);
    bind::expire(&mut off, id);
    assert_eq!(off.purse(None), treasury, "EC_BASE: lost");
    assert_eq!(off.probe.loot_lost, loot);
}

/// E25b: the fence's resale credit crosses in from the World (`flow_fence`),
/// the gang's income unchanged, the identity whole.
#[test]
fn test_fence_resale_crosses_in() {
    let mut w = city();
    let gang_id = w.gangs()[0];
    let member = w
        .citizens()
        .into_iter()
        .find(|&a| w.gang_of(a).is_none() && citysim::systems::demography::is_adult(&w, a) && w.has::<Wallet>(a))
        .expect("an adult");
    gang::enlist(&mut w, member, gang_id);
    if let Some(i) = w.comp_mut::<citysim::Inventory>(member) {
        i.food = 5;
        i.stolen_food = 5;
    }
    if let Some(g) = w.comp_mut::<citysim::Gang>(gang_id) {
        g.treasury = 100;
    }
    let price = w.mean_price();
    let (ident, inbound, income) = (econ::identity(&w), w.outside.inbound, w.stats.current.gang_income);
    let pay = gang::fence(&mut w, member);
    assert_eq!(pay, (price as f32 * 0.8).floor() as i64 * 5);
    let credit = 5 * price;
    assert_eq!(w.comp::<citysim::Gang>(gang_id).map(|g| g.treasury), Some(100 - pay + credit));
    assert_eq!(w.outside.inbound, inbound + credit, "booked as a crossing");
    assert_eq!(w.stats.current.econ.flow_fence, credit);
    assert_eq!(w.stats.current.gang_income, income + credit, "gang income unchanged in kind");
    assert_eq!(econ::identity(&w), ident);
    assert_eq!(w.probe.fence_credit, credit);
}

/// E1, E2: with every section off nothing is seeded and no account exists.
#[test]
fn test_econ_off_world_has_no_outside_account() {
    let mut w = World::new(42, Config::load().econ_off());
    assert!(!econ::market_on(&w) && !wm::open(&w));
    assert!(w.outside.is_empty(), "no World account at seed");
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.outside.is_empty());
    assert!(w.stats.history.iter().all(|r| r.econ.is_zero() && r.living.flow_export == 0));
    assert!(w.econ.is_default());
    assert!(!citysim::save::to_ron(&w).contains("outside:"));
    assert!(!citysim::save::to_ron(&w).contains("econ:"));
    // The master switch alone is the same off world.
    let mut master = Config::load();
    master.economy2.enabled = false;
    let m = World::new(42, master);
    assert!(!econ::market_on(&m) && m.outside.is_empty());
    // `[living]` off turns the economy off with it (E1).
    assert!(!econ::market_on_cfg(&Config::load().living_off()));
}

/// E4: the World account stands at seed with its books at appetite 1.0, and
/// the identity's base is taken after the seeding (`minted` = `treasury_ref`).
#[test]
fn test_world_seeded_with_books() {
    let w = city();
    let f = w.outside.faction(WORLD_ACCOUNT).expect("the World account at seed");
    assert_eq!((f.treasury, f.treasury_ref), (50_000, 50_000));
    assert_eq!(w.outside.minted, 50_000);
    assert_eq!(f.books.len(), 3);
    for g in ExportGood::ALL {
        let b = &f.books[&g];
        assert_eq!((b.appetite, b.walk, b.ask_mult), (1.0, 1.0, 1.0));
        assert!(b.appetite_pin.is_none() && b.cap_pin.is_none() && b.bought.is_empty());
    }
    assert_eq!(econ::identity(&w), ownership::total_coins(&w), "the outside side is 0 at seed");
    let _ = Flow::Customs;
}

/// E47: the levers pin and release the books; `SetExport` closes buying only.
#[test]
fn test_market_levers() {
    use citysim::PlayerCommand;
    let mut w = city();
    w.push_command(PlayerCommand::SetAppetite { good: ExportGood::Parts, mult: Some(2.5) });
    w.push_command(PlayerCommand::SetExportCap { good: ExportGood::Food, cap: Some(10) });
    w.push_command(PlayerCommand::SetImportAsk { good: ExportGood::Food, mult: 0.5 });
    w.push_command(PlayerCommand::SetCustoms(0.25));
    w.run_ticks(1);
    assert_eq!(wm::appetite(&w, ExportGood::Parts), 2.5);
    assert_eq!(wm::cap_today(&w, ExportGood::Food), 10);
    assert_eq!(wm::ask(&w, ExportGood::Food), 3, "3.4 × 1.6 × 0.5 rounded up");
    assert_eq!(wm::customs_rate(&w), 0.25);
    w.push_command(PlayerCommand::SetAppetite { good: ExportGood::Parts, mult: None });
    w.push_command(PlayerCommand::SetExportCap { good: ExportGood::Food, cap: None });
    w.run_ticks(1);
    assert!(wm::book(&w, ExportGood::Parts).is_some_and(|b| b.appetite_pin.is_none()));
    let cap = w.config.world_market.food.cap as f32;
    assert_eq!(wm::cap_today(&w, ExportGood::Food), (cap * wm::appetite(&w, ExportGood::Food)).round() as u32);
    w.push_command(PlayerCommand::SetExport(false));
    w.run_ticks(1);
    assert!(!wm::open(&w));
    let corp = w.corps()[0];
    assert_eq!(wm::sell(&mut w, None, Some(corp), ExportGood::Food, 5), (0, 0));
    w.push_command(PlayerCommand::SetExport(true));
    w.push_command(PlayerCommand::CloseWorld(true));
    w.run_ticks(1);
    assert!(!wm::open(&w) && w.econ.world_closed);
    w.push_command(PlayerCommand::CloseWorld(false));
    w.run_ticks(1);
    assert!(wm::open(&w));
}

/// E25 (the census's late find): a debt repaid to a creditor killed since
/// lending (no Wallet) vanished at EC_BASE; with the market on the coins
/// stay with the debtor and the identity holds.
#[test]
fn test_repayment_to_a_dead_creditor_leaks_no_coin() {
    let mut w = city();
    let adults: Vec<_> = w
        .citizens()
        .into_iter()
        .filter(|&a| citysim::systems::demography::is_adult(&w, a) && w.has::<Wallet>(a) && w.has::<citysim::Brain>(a))
        .take(2)
        .collect();
    let (debtor, creditor) = (adults[0], adults[1]);
    if let Some(x) = w.comp_mut::<Wallet>(debtor) {
        x.coins = 100;
    }
    // The lower id owes the higher when `debt > 0`.
    let (lo, _) = citysim::components::edge_key(debtor, creditor);
    let e = w.edge_entry(debtor, creditor);
    e.debt = if lo == debtor { 5 } else { -5 };
    w.kill_by(creditor, citysim::DeathCause::Violence, None);
    assert!(!w.has::<Wallet>(creditor) && w.edge(debtor, creditor).is_some(), "a dead creditor with its edge");
    let ident = econ::identity(&w);
    citysim::systems::social::repay_debts(&mut w, debtor);
    assert_eq!(econ::identity(&w), ident, "no coin vanished");
    assert_eq!(w.purse(Some(debtor)), 100, "the debt waits");
}

/// E47: `SetExport` off closes the World's buying only (a short Market still
/// imports at the ask); `CloseWorld` refuses the imports too.
#[test]
fn test_export_off_still_imports_close_world_refuses() {
    use citysim::PlayerCommand;
    fn short_market(w: &mut World) -> (citysim::EntityId, Option<citysim::EntityId>) {
        w.run_ticks(TICKS_PER_DAY - 1);
        if let Some(wh) = w.building_of_kind(BuildingKind::Warehouse) {
            if let Some(b) = w.comp_mut::<Building>(wh) {
                b.stock_food = 0;
            }
        }
        let mk = w
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .find(|&m| w.owner_of(m).is_some_and(|o| w.has::<Corp>(o)))
            .expect("a corp Market");
        if let Some(b) = w.comp_mut::<Building>(mk) {
            b.stock_food = 0;
        }
        let owner = w.owner_of(mk);
        if let Some(c) = owner.and_then(|o| w.comp_mut::<Corp>(o)) {
            c.treasury = 50_000;
            c.closing = 50_000;
        }
        (mk, owner)
    }
    // (a) export=off: the World buys nothing, the Market still imports.
    let mut w = city();
    let (mk, _) = short_market(&mut w);
    w.push_command(PlayerCommand::SetExport(false));
    w.run_ticks(2);
    assert!(!wm::open(&w) && wm::imports_open(&w));
    assert!(wm::book(&w, ExportGood::Food).is_some_and(|b| b.sold_today > 0), "imported at the ask");
    assert!(w.stock(mk, citysim::Good::Food) > 0);
    assert_eq!(wm::book(&w, ExportGood::Food).map(|b| b.bought_today), Some(0), "the World bought nothing");
    assert!(
        w.stats.current.econ.flow_import_out > 0 && w.events.iter().any(|e| e.kind == citysim::EventKind::Imported)
    );
    // (b) close_world=on: no import, the shelf stays short.
    let mut w = city();
    let (mk, _) = short_market(&mut w);
    w.push_command(PlayerCommand::CloseWorld(true));
    w.run_ticks(2);
    assert!(!wm::open(&w) && !wm::imports_open(&w));
    assert_eq!(wm::book(&w, ExportGood::Food).map(|b| b.sold_today), Some(0), "refused");
    assert_eq!(w.stock(mk, citysim::Good::Food), 0, "a Market short of food stays short");
    assert!(!w.events.iter().any(|e| e.kind == citysim::EventKind::Imported));
}
