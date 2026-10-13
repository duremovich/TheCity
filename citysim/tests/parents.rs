//! M17 phase 2 (docs/M17_OUTSIDE.md, "Revised 2026-10-12"): the ledger and
//! the parents. One account per megacorp brand, per syndicate and the State,
//! seeded from `[outside.parents] factions` (minted into the identity); the
//! daily tick (income abroad, upkeep, remittances, health, rep, attitude,
//! failure flags); Hold and Subsidise; `Embargo` and `SetTariff`; the
//! pre-M17 save migration. Every crossing keeps the coin identity.

use citysim::outside::{OutsideKind, OutsideOrder};
use citysim::systems::{econ, outside};
use citysim::{save, Config, Corp, EntityId, EventKind, Gang, PlayerCommand, World, TICKS_PER_DAY};

fn account(w: &World, name: &str) -> citysim::outside::OutsideFaction {
    w.outside.factions.iter().find(|f| f.name == name).cloned().unwrap_or_else(|| panic!("no account {name}"))
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == name)).expect("corp")
}

#[test]
fn test_parents_seeded_per_brand_with_the_identity() {
    let mut w = World::new(42, Config::load());
    assert!(outside::parents_on(&w));
    let names: Vec<String> = w.outside.parents().map(|f| f.name.clone()).collect();
    assert_eq!(
        names,
        ["Arasaka Global", "Militech International", "The State", "Hollow Brothers", "Ninefold Lodge"]
            .map(String::from)
    );
    let a = account(&w, "Arasaka Global");
    assert_eq!(a.kind, OutsideKind::Megacorp);
    let arasaka = corp_named(&w, "Arasaka");
    assert_eq!(a.parent.as_ref().expect("parent").branches, vec![arasaka]);
    assert_eq!(w.comp::<Corp>(arasaka).and_then(|c| c.parent), Some(a.id), "Corp.parent re-pointed at the brand");
    assert_eq!(w.comp::<Corp>(arasaka).map(|c| c.outside_treasury), Some(0), "the old hook zeroed");
    let hollow = account(&w, "Hollow Brothers");
    assert_eq!(hollow.parent.as_ref().expect("parent").branches.len(), 1, "a creed's gang has no syndicate");
    let gang = hollow.parent.as_ref().expect("parent").branches[0];
    assert_eq!(w.comp::<Gang>(gang).map(|g| g.name.clone()), Some("The Hollow".to_string()));
    let state = account(&w, "The State");
    assert_eq!(state.parent.as_ref().expect("parent").branches.len(), 1, "the Law's Jail");
    let base = econ::identity(&w);
    for _ in 0..3 {
        w.run_ticks(TICKS_PER_DAY);
        assert_eq!(econ::identity(&w), base, "coins made or lost on day {}", w.day());
    }
    // The ledgers ticked (the midnights of days 1 and 2; tick 0 has not traded).
    let a = account(&w, "Arasaka Global");
    assert_eq!(a.parent.as_ref().expect("parent").ledger.income.len(), 2);
}

/// Scale: a brand dealt twice is one account with both copies as branches,
/// its treasury and income per branch.
#[test]
fn test_brand_account_collects_every_copy() {
    let mut c = Config::load().scaled_to(4000);
    c.world.map = "gen:4000".into();
    let w = World::new(42, c);
    let copies: Vec<EntityId> =
        w.corps().into_iter().filter(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name.starts_with("Arasaka"))).collect();
    let a = account(&w, "Arasaka Global");
    let p = a.parent.as_ref().expect("parent");
    println!("Arasaka copies {}, branches {}", copies.len(), p.branches.len());
    assert!(copies.len() >= 2, "the 4k roster deals Arasaka twice");
    assert_eq!(p.branches, copies);
    let row = &w.config.outside.parents.factions[0];
    assert_eq!(a.base_income, row.income * copies.len() as i64);
    assert!(copies.iter().all(|&c| w.comp::<Corp>(c).and_then(|x| x.parent) == Some(a.id)));
}

#[test]
fn test_income_applies_every_factor() {
    // base 3000, market 1.1, edge 1.0, rep 0.5 (x1.0), exposure 0.2, loss 0.5
    // (x0.9), no cut; then the embargo's 0.08 cut (phase 4: one cut term).
    assert_eq!(outside::parent_income(3000, 1.1, 1.0, 0.5, 0.2, 0.5, 0.0), 2970);
    assert_eq!(outside::parent_income(3000, 1.1, 1.0, 0.5, 0.2, 0.5, 0.08), 2732);
    assert_eq!(outside::parent_income(3000, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0), 3600);
}

#[test]
fn test_branch_loss_rises_on_a_sale_and_a_remit_gap() {
    // Phase 3: the gap is the share of branches a week in the red.
    assert_eq!(outside::branch_loss(1000, 1000, 0.0), 0.0, "whole and trading");
    let sale = outside::branch_loss(500, 1000, 0.0);
    assert!(sale > 0.25, "half the buildings gone: {sale}");
    let gap = outside::branch_loss(1000, 1000, 1.0);
    assert!(gap > 0.3, "every branch a week in the red: {gap}");
    assert_eq!(outside::branch_loss(0, 0, 0.0), 0.0, "nothing to lose yet");
}

/// A profitable branch with a healthy treasury sends `remit_share` of its
/// day home; the tariff's share goes to the Treasury; a capital move.
#[test]
fn test_remittance_from_a_profitable_branch() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(2 * TICKS_PER_DAY + 30);
    let arasaka = corp_named(&w, "Arasaka");
    let parent = account(&w, "Arasaka Global").id;
    w.outside.tariff = 0.1;
    {
        let c = w.comp_mut::<Corp>(arasaka).expect("corp");
        c.cashflow.push_back(1000);
        c.closing = c.treasury_ref;
        c.treasury += 5000;
    }
    let base = econ::identity(&w);
    let (t0, cf0) = w.comp::<Corp>(arasaka).map(|c| (c.treasury, c.cashflow_today)).expect("corp");
    let p0 = account(&w, "Arasaka Global").treasury;
    let city0 = w.treasury().map_or(0, |t| t.coins);
    outside::remit(&mut w, parent);
    let (t1, cf1) = w.comp::<Corp>(arasaka).map(|c| (c.treasury, c.cashflow_today)).expect("corp");
    assert_eq!(t0 - t1, 300, "remit_share 0.3 of 1000");
    assert_eq!(cf1, cf0, "a capital move, not a trading loss");
    assert_eq!(account(&w, "Arasaka Global").treasury - p0, 270, "net of the tariff");
    assert_eq!(w.treasury().map_or(0, |t| t.coins) - city0, 30, "the tariff to the Treasury");
    assert_eq!(econ::identity(&w), base, "coins made or lost");
    // A branch below the floor keeps its coins.
    {
        let c = w.comp_mut::<Corp>(arasaka).expect("corp");
        c.closing = 0;
    }
    let t2 = w.comp::<Corp>(arasaka).map(|c| c.treasury).expect("corp");
    outside::remit(&mut w, parent);
    assert_eq!(w.comp::<Corp>(arasaka).map(|c| c.treasury), Some(t2));
}

/// A weak branch is subsidised: capital into its treasury, never revenue.
#[test]
fn test_subsidy_is_capital_not_revenue() {
    let mut w = World::new(42, Config::load());
    // Past the orders' minimum hold.
    w.run_ticks(4 * TICKS_PER_DAY + 30);
    let militech = corp_named(&w, "Militech");
    let parent = account(&w, "Militech International").id;
    let base = econ::identity(&w);
    let mut paid_any = false;
    for _ in 0..6 {
        let cut = {
            let c = w.comp_mut::<Corp>(militech).expect("corp");
            let cut = c.treasury - c.treasury_ref / 10;
            c.treasury -= cut;
            c.closing = c.treasury;
            cut
        };
        let (t0, cf0, rev0) = w.comp::<Corp>(militech).map(|c| (c.treasury, c.cashflow_today, c.rev_today)).expect("c");
        outside::decide(&mut w, parent);
        let (t1, cf1, rev1) = w.comp::<Corp>(militech).map(|c| (c.treasury, c.cashflow_today, c.rev_today)).expect("c");
        assert_eq!(cf1, cf0, "capital");
        assert_eq!(rev1, rev0, "never revenue (wage_rev)");
        if t1 > t0 {
            paid_any = true;
            assert_eq!(account(&w, "Militech International").parent.expect("p").order, OutsideOrder::Subsidise);
        }
        // Put the cut back, so only the subsidy moved coins.
        let c = w.comp_mut::<Corp>(militech).expect("corp");
        c.treasury += cut;
    }
    assert!(paid_any, "a weak branch was subsidised");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Subsidised));
    assert_eq!(econ::identity(&w), base, "the subsidy crossed in from the parent");
}

/// `Embargo` stops the remittance out and the subsidy in.
#[test]
fn test_embargo_stops_every_crossing() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(2 * TICKS_PER_DAY + 30);
    let militech = corp_named(&w, "Militech");
    let parent = account(&w, "Militech International").id;
    w.push_command(PlayerCommand::Embargo { id: parent, on: true });
    w.apply_commands();
    assert!(account(&w, "Militech International").parent.expect("p").embargoed);
    {
        let c = w.comp_mut::<Corp>(militech).expect("corp");
        c.cashflow.push_back(1000);
        c.closing = c.treasury_ref;
    }
    let t0 = w.comp::<Corp>(militech).map(|c| c.treasury).expect("corp");
    outside::remit(&mut w, parent);
    assert_eq!(w.comp::<Corp>(militech).map(|c| c.treasury), Some(t0), "no remittance out");
    {
        let c = w.comp_mut::<Corp>(militech).expect("corp");
        c.treasury = c.treasury_ref / 10;
        c.closing = c.treasury;
    }
    let t1 = w.comp::<Corp>(militech).map(|c| c.treasury).expect("corp");
    for _ in 0..4 {
        outside::decide(&mut w, parent);
    }
    assert_eq!(w.comp::<Corp>(militech).map(|c| c.treasury), Some(t1), "no subsidy in");
    w.push_command(PlayerCommand::SetTariff(0.25));
    w.apply_commands();
    assert_eq!(w.outside.tariff, 0.25);
}

/// A parent whose income stays under its upkeep for a week is flagged.
#[test]
fn test_failing_flag_after_a_week_under_upkeep() {
    let mut c = Config::load();
    c.outside.parents.factions[3].income = 0;
    let mut w = World::new(42, c);
    w.run_ticks(6 * TICKS_PER_DAY + 30);
    assert!(account(&w, "Hollow Brothers").parent.expect("p").failing_since.is_none(), "not before 7 days of books");
    w.run_ticks(2 * TICKS_PER_DAY);
    assert!(account(&w, "Hollow Brothers").parent.expect("p").failing_since.is_some());
    assert!(account(&w, "Ninefold Lodge").parent.expect("p").failing_since.is_none());
    assert!(w.events.iter().any(|e| e.kind == EventKind::ParentFailing && e.text.contains("Hollow Brothers")));
}

/// The switch off: no parents, the M11 hook untouched.
#[test]
fn test_switch_off_seeds_no_parents() {
    let mut c = Config::load();
    c.outside.enabled = false;
    let w = World::new(42, c);
    assert_eq!(w.outside.parents().count(), 0);
    let arasaka = corp_named(&w, "Arasaka");
    assert_eq!(w.comp::<Corp>(arasaka).and_then(|c| c.parent), Some(citysim::OUTSIDE_PARENT_BASE + 6));
}

/// A save from before M17 phase 2 loads with the accounts built; the
/// identity carries over (the seeding is minted).
#[test]
fn test_pre_m17_save_builds_the_accounts() {
    let mut c = Config::load();
    c.outside.enabled = false;
    let mut w = World::new(42, c);
    w.run_ticks(TICKS_PER_DAY + 30);
    w.config.outside.enabled = true;
    w.save_version = 21;
    let before = econ::identity(&w);
    let back = save::from_ron(&save::to_ron(&w)).expect("loads");
    assert_eq!(back.outside.parents().count(), 5);
    assert_eq!(econ::identity(&back), before);
    let arasaka = corp_named(&back, "Arasaka");
    assert_eq!(back.comp::<Corp>(arasaka).and_then(|c| c.parent), Some(account(&back, "Arasaka Global").id));
}

/// A save mid-run resumes byte for byte with the ledgers.
#[test]
fn test_save_round_trip_with_the_parents() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(3 * TICKS_PER_DAY + 400);
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("loads");
    assert_eq!(save::to_ron(&back), text);
    w.run_ticks(2 * TICKS_PER_DAY);
    back.run_ticks(2 * TICKS_PER_DAY);
    assert_eq!(save::to_ron(&back), save::to_ron(&w));
}

/// A syndicate's gang pays `tribute_share` of its treasury above
/// `tribute_floor` on the week's first midnight, and nothing on other days.
#[test]
fn test_gang_tribute_is_weekly() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(6 * TICKS_PER_DAY + 30);
    let hollow = account(&w, "Hollow Brothers");
    let gang = hollow.parent.as_ref().expect("parent").branches[0];
    w.comp_mut::<Gang>(gang).expect("gang").treasury = 1100;
    outside::remit(&mut w, hollow.id);
    assert_eq!(w.comp::<Gang>(gang).map(|g| g.treasury), Some(1100), "day 6: not the week's midnight");
    w.run_ticks(TICKS_PER_DAY);
    w.comp_mut::<Gang>(gang).expect("gang").treasury = 1100;
    let (base, p0) = (econ::identity(&w), account(&w, "Hollow Brothers").treasury);
    outside::remit(&mut w, hollow.id);
    assert_eq!(w.comp::<Gang>(gang).map(|g| g.treasury), Some(1000), "10 % of 1,000 above the floor");
    assert_eq!(account(&w, "Hollow Brothers").treasury - p0, 100);
    assert_eq!(econ::identity(&w), base, "a crossing, no coin made or lost");
}
