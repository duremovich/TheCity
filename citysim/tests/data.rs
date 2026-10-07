//! M14 phase 2: the Data market and the deck sellers (plan 2.8). Data is an
//! abstract integer good of the simulation, moved by seeded dice contests.

use citysim::systems::{assets, law, lod, ownership, tech, virt};
use citysim::virt::{Purpose, RunMode, RunOrder, RunOutcome, RunWhy, Track};
use citysim::{AssetKind, Brain, Building, BuildingKind, Config, Corp, EntityId, GangMember, Kit, Lod, Skills, World};

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name)).expect("seeded corp")
}

fn lab_of(w: &World, corp: EntityId, focus: Track) -> EntityId {
    w.buildings_of_kind(BuildingKind::Lab)
        .iter()
        .copied()
        .find(|&b| w.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(corp) && bd.focus == Some(focus)))
        .expect("a seeded Lab")
}

/// V17/V29: a steal of `steal_units[1]` units lands on the deck; `SellData` at
/// Zetatech's Lab pays `units × data_price × level` (`Flow::Data`, taxed) and
/// moves the units into Zetatech's store.
#[test]
fn test_theft_moves_units_and_sale_pays_tech_corp() {
    let mut cfg = Config::load();
    cfg.hack.quiet_take = 1.0;
    // A tier-2 deck's take, read from `[data] steal_units` (the store holds more).
    let units = cfg.data.steal_units[1];
    assert!(units > 0 && units < 500);
    let mut w = World::new(42, cfg);
    let (arasaka, zeta) = (corp_named(&w, "Arasaka"), corp_named(&w, "Zetatech"));
    let lab = lab_of(&w, arasaka, Track::Deck);
    let n = virt::node_of_building(&w, lab).expect("node");
    virt::profile_mut(&mut w, n).expect("profile").ice = 0;
    virt::bump_epoch(&mut w);
    w.virt.node_mut(n).expect("node").store.units[Track::Deck.index()] = 500;
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(&w, a)
                && !w.has::<GangMember>(a)
                && w.comp::<Kit>(a).is_some_and(|k| k.deck.is_none())
                && !law::is_guard(&w, a)
        })
        .expect("an adult");
    lod::set_lod(&mut w, a, Lod::Coarse);
    assets::grant(&mut w, a, AssetKind::Deck, 2).expect("deck");
    w.comp_mut::<Skills>(a).expect("skills").hacking = 0.5;
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    let now = w.tick;
    w.run_orders.insert(
        a,
        RunOrder {
            patron: None,
            purpose: Purpose::Data { wipe: false },
            target: n,
            chair: bar,
            not_before: now,
            expires: now + 1440,
            why: RunWhy::Freelance,
            mode: RunMode::Quiet,
        },
    );
    virt::start_run(&mut w, a).expect("run");
    while let Some(&(t, _)) = w.run_queue.first() {
        w.tick = w.tick.max(t);
        virt::run(&mut w);
    }
    assert_eq!(w.run_log.back().and_then(|r| r.outcome), Some(RunOutcome::Success));
    assert_eq!(virt::deck_data(&w, a), units);
    assert_eq!(w.virt.node(n).expect("node").store.get(Track::Deck), 500 - units);
    assert_eq!(w.stats.current.virt.data_stolen, units);
    // The sale, inside Zetatech's Lab.
    let zlab = lab_of(&w, zeta, Track::Chrome);
    assert!(tech::is_data_buyer_lab(&w, zlab));
    assert_eq!(tech::data_buyer_lab(&w, a).map(|b| tech::is_data_buyer_lab(&w, b)), Some(true));
    w.leave_building(a);
    w.enter_building(a, zlab);
    w.comp_mut::<Corp>(zeta).expect("corp").treasury = 10_000;
    let price = tech::data_unit_price(&w, zeta) * i64::from(units);
    let (wallet, purse, city, held) =
        (w.purse(Some(a)), w.purse(Some(zeta)), w.purse(None), virt::holding(&w, zeta, Track::Deck));
    let total = ownership::total_coins(&w);
    let flow = w.stats.current.virt.flow_data;
    assert_eq!(tech::sell_deck_data(&mut w, a), units);
    let tax = w.purse(None) - city;
    assert_eq!(w.purse(Some(a)) - wallet + tax, price, "the seller's take plus the tax");
    assert_eq!(purse - w.purse(Some(zeta)), price);
    assert_eq!(virt::holding(&w, zeta, Track::Deck), held + units, "the units went to Zetatech's Lab");
    assert_eq!(virt::deck_data(&w, a), 0);
    assert_eq!(w.stats.current.virt.flow_data - flow, price);
    assert_eq!(w.stats.current.virt.data_sold, units);
    assert_eq!(ownership::total_coins(&w), total);
    println!("sold {units} for {price} (tax {tax})");
}

/// V22/V24: decks at Security Offices by the owner's Deck tier, and at a
/// back-alley Clinic by the street tier.
#[test]
fn test_deck_seller_gate() {
    let w = World::new(42, Config::load());
    let office = |name: &str| {
        ownership::owned_of_kind(&w, Some(corp_named(&w, name)), BuildingKind::SecurityOffice)
            .first()
            .copied()
            .expect("an Office")
    };
    let (militech, arasaka) = (office("Militech"), office("Arasaka"));
    assert!(assets::sells(BuildingKind::SecurityOffice).contains(&AssetKind::Deck));
    assert!(assets::sells(BuildingKind::Clinic).contains(&AssetKind::Deck));
    assert!(assets::can_sell(&w, militech, AssetKind::Deck, 2));
    assert!(!assets::can_sell(&w, militech, AssetKind::Deck, 3), "Militech holds Deck 2");
    assert!(assets::can_sell(&w, arasaka, AssetKind::Deck, 3));
    let alley = w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .find(|&c| w.owner_of(c).is_some_and(|o| !w.has::<Corp>(o)))
        .expect("a back-alley Clinic");
    assert_eq!(tech::street_tier(&w, Track::Deck).0, 3);
    assert!(assets::can_sell(&w, alley, AssetKind::Deck, 3), "the street tier");
}

/// Plan 4.3 (V42): `SetCityIce` installs and lowers ICE on the Treasury's
/// Ledger and the Precinct until both match; the lever is read back.
#[test]
fn test_city_ice_lever_sets_treasury_and_precinct() {
    use citysim::PlayerCommand;
    let mut w = World::new(42, Config::load());
    let hall = w.building_of_kind(BuildingKind::Hall).expect("hall");
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let (treasury, precinct) = (
        virt::ledger_of(&w, hall).expect("the Treasury's node"),
        virt::node_of_building(&w, jail).expect("the Precinct's node"),
    );
    let ice = |w: &World, n| virt::profile(w, n).map(|p| p.ice);
    assert_eq!((ice(&w, treasury), ice(&w, precinct)), (Some(2), Some(2)), "[levers] city_ice seeds 2");
    let coins = w.purse(None);
    for want in [3u8, 0, 1] {
        w.push_command(PlayerCommand::SetCityIce(want));
        w.apply_commands();
        assert_eq!(w.levers.city_ice, want);
        assert_eq!((ice(&w, treasury), ice(&w, precinct)), (Some(want), Some(want)), "tier {want}");
    }
    assert!(w.purse(None) < coins, "the city paid for the installs");
}

/// Plan 4.3 (V42): `SetDataTax` takes an extra share of a Data sale for the
/// Treasury, out of the seller's take; coins are conserved.
#[test]
fn test_data_tax_applies() {
    use citysim::PlayerCommand;
    let sell = |tax: f32| {
        let mut w = World::new(42, Config::load());
        w.push_command(PlayerCommand::SetDataTax(tax));
        w.apply_commands();
        assert!((w.levers.data_tax - tax).abs() < 1e-6);
        let zeta = corp_named(&w, "Zetatech");
        w.comp_mut::<Corp>(zeta).expect("corp").treasury = 100_000;
        let seller = w
            .citizens()
            .into_iter()
            .find(|&a| w.has::<Brain>(a) && citysim::systems::demography::is_adult(&w, a))
            .expect("an adult");
        let total = ownership::total_coins(&w);
        let (wallet, city) = (w.purse(Some(seller)), w.purse(None));
        let n = tech::sell_data(&mut w, seller, Track::Deck, 200, Some(zeta));
        assert_eq!(ownership::total_coins(&w), total, "coins are conserved");
        (n, w.purse(Some(seller)) - wallet, w.purse(None) - city, tech::data_unit_price(&w, zeta) * i64::from(n))
    };
    let (n0, take0, city0, price) = sell(0.0);
    let (n1, take1, city1, _) = sell(0.5);
    assert!(n0 > 0 && n0 == n1, "the same sale: {n0} and {n1}");
    let extra = (price as f32 * 0.5).floor() as i64;
    assert!(extra > 0);
    assert_eq!(city1 - city0, extra, "the Treasury's extra share");
    assert_eq!(take0 - take1, extra, "paid out of the seller's take");
}
