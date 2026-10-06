//! M14 phase 1: Labs, Data, the tech tree, the tier gates and effective
//! tiers (plan 1.10).

use citysim::systems::{assets, demography, lod, tech, virt};
use citysim::virt::{NodeId, Track};
use citysim::{
    AssetKind, Building, BuildingKind, Config, Corp, CorpOrder, CorpShock, EntityId, EventKind, Job, Kit, Lod, Role,
    ShopPick, Skills, Slot, Wallet, World, TICKS_PER_DAY,
};

fn world() -> World {
    World::new(42, Config::load())
}

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

fn node(w: &World, b: EntityId) -> NodeId {
    virt::node_of_building(w, b).expect("node")
}

/// Every store of `corp` in `track` emptied.
fn drain(w: &mut World, corp: EntityId, track: Track) {
    for n in w.virt.nodes.iter_mut().filter(|n| n.owner == Some(corp)) {
        n.store.units[track.index()] = 0;
    }
}

/// Jobless adults, ascending, not execs.
fn jobless(w: &World, n: usize) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| !w.has::<Job>(a) && demography::is_adult(w, a))
        .filter(|&a| !citysim::systems::founding::is_exec(w, a))
        .take(n)
        .collect()
}

/// V16: four Researchers at hacking 0.5 who worked yesterday add `4 ×
/// round(3 × 1.0)` = 12 at midnight, whatever their LOD tier.
#[test]
fn test_lab_produces_from_shift_ledger_at_every_tier() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    let lab = lab_of(&w, z, Track::Chrome);
    let n = node(&w, lab);
    w.vacancies.remove(&lab);
    let staff = jobless(&w, 4);
    for &a in &staff {
        demography::hire(&mut w, a, lab, Role::Researcher);
        w.comp_mut::<Skills>(a).expect("skills").hacking = 0.5;
    }
    w.virt.node_mut(n).expect("node").store.units = [0; 3];
    for (round, lod_tier) in [(0u64, None), (1, Some(Lod::Statistical))] {
        w.tick = (2 + round) * TICKS_PER_DAY;
        for &a in &staff {
            if let Some(l) = lod_tier {
                lod::set_lod(&mut w, a, l);
            }
            let key = w.comp::<Job>(a).expect("job").shift_key_at(w.tick - 1);
            w.comp_mut::<Job>(a).expect("job").last_shift_day = Some(key);
        }
        let before = w.virt.node(n).expect("node").store.get(Track::Chrome);
        tech::produce(&mut w);
        let after = w.virt.node(n).expect("node").store.get(Track::Chrome);
        assert_eq!(after - before, 12, "round {round} ({lod_tier:?})");
    }
    assert_eq!(w.comp::<citysim::Brain>(staff[0]).expect("brain").lod, Lod::Statistical);
}

/// V21: Chrome 3 with nothing held lapses daily; on the 7th midnight it
/// drops to Chrome 2 (`TechLost`, `CorpShock::TechLost`).
#[test]
fn test_upkeep_lapse_drops_tier_after_decay_days() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    drain(&mut w, z, Track::Chrome);
    for day in 1..=7u8 {
        tech::research_upkeep(&mut w, z);
        let c = w.comp::<Corp>(z).expect("corp");
        if day < 7 {
            assert_eq!(c.tech.tier_of(Track::Chrome), 3, "day {day}");
            assert_eq!(c.tech.lapse[Track::Chrome.index()], day);
        }
    }
    let c = w.comp::<Corp>(z).expect("corp");
    assert_eq!(c.tech.tier_of(Track::Chrome), 2);
    assert_eq!(c.tech.tier_of(Track::Industry), 3, "Industry held (its Lab holds 300)");
    assert!(c.shocks.contains(&CorpShock::TechLost));
    assert!(w.events.iter().any(|e| e.kind == EventKind::TechLost && e.actors.first() == Some(&z)));
}

/// V21: research never spends the backup reserve (`upkeep_data[tier] × backup_days`).
#[test]
fn test_research_keeps_backup_reserve() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    let lab = node(&w, lab_of(&w, z, Track::Chrome));
    drain(&mut w, z, Track::Chrome);
    {
        let c = w.comp_mut::<Corp>(z).expect("corp");
        c.order = CorpOrder::Research;
        c.tech.focus = Track::Chrome;
    }
    // The plan's Chrome 3 case is vacuous (tier 3 has no next tier, so
    // research spends nothing); Chrome 2 (upkeep 3, reserve 3 x 3 = 9) is
    // the live case: of 30 held, 30 - 9 = 21 move and the reserve stays.
    tech::set_tier(&mut w, z, Track::Chrome, 2);
    w.comp_mut::<Corp>(z).expect("corp").tech.progress = [0; 3];
    w.virt.node_mut(lab).expect("node").store.units[Track::Chrome.index()] = 30;
    tech::research(&mut w, z);
    assert_eq!(virt::holding(&w, z, Track::Chrome), 9);
    assert_eq!(w.comp::<Corp>(z).expect("corp").tech.progress[Track::Chrome.index()], 21);
}

/// V21: progress 1,190 + 40 reaches tier 3's 1,200 with 3,000 in the
/// treasury: Chrome 3 and -2,500 through `Flow::Research`.
#[test]
fn test_research_gains_tier() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    let lab = node(&w, lab_of(&w, z, Track::Chrome));
    tech::set_tier(&mut w, z, Track::Chrome, 2);
    drain(&mut w, z, Track::Chrome);
    w.virt.node_mut(lab).expect("node").store.units[Track::Chrome.index()] = 100;
    {
        let c = w.comp_mut::<Corp>(z).expect("corp");
        c.order = CorpOrder::Research;
        c.tech.focus = Track::Chrome;
        c.tech.progress[Track::Chrome.index()] = 1190;
        c.treasury = 3000;
    }
    let flow = w.stats.current.virt.flow_research;
    tech::research(&mut w, z);
    let c = w.comp::<Corp>(z).expect("corp");
    assert_eq!(c.tech.tier_of(Track::Chrome), 3);
    assert_eq!(c.treasury, 500);
    assert_eq!(w.stats.current.virt.flow_research - flow, 2500);
    assert!(w.events.iter().any(|e| e.kind == EventKind::TechGained));
}

/// V19: a wipe with a backup held elsewhere (30 >= 8 × 3) costs no tier;
/// without one (10) the track drops at once.
#[test]
fn test_wipe_without_backup_drops_tier_and_with_backup_does_not() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    let (chrome, other) = (node(&w, lab_of(&w, z, Track::Chrome)), node(&w, lab_of(&w, z, Track::Industry)));
    drain(&mut w, z, Track::Chrome);
    w.virt.node_mut(chrome).expect("node").store.units[Track::Chrome.index()] = 100;
    w.virt.node_mut(other).expect("node").store.units[Track::Chrome.index()] = 30;
    assert_eq!(tech::wipe_store(&mut w, chrome, None), 100);
    assert_eq!(w.comp::<Corp>(z).expect("corp").tech.tier_of(Track::Chrome), 3, "the backup holds");
    assert_eq!(w.stats.current.virt.data_wiped, 100);
    w.virt.node_mut(chrome).expect("node").store.units[Track::Chrome.index()] = 100;
    w.virt.node_mut(other).expect("node").store.units[Track::Chrome.index()] = 10;
    tech::wipe_store(&mut w, chrome, None);
    assert_eq!(w.comp::<Corp>(z).expect("corp").tech.tier_of(Track::Chrome), 2, "no backup: a tier lost");
    assert!(w.events.iter().any(|e| e.kind == EventKind::DataWiped));
}

/// V22: a corp seller sells at its own tier, a back-alley seller at the
/// street tier (maker: the corp holding it); no tier, no sale.
#[test]
fn test_seller_gate_and_street_tier() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    let arms = AssetKind::Implant(Slot::Arms);
    let clinics = w.buildings_of_kind(BuildingKind::Clinic).to_vec();
    let corp_clinic = *clinics.iter().find(|&&c| w.owner_of(c) == Some(z)).expect("Zetatech's Clinic");
    let alley = *clinics.iter().find(|&&c| w.corp_of_building(c).is_none()).expect("a back-alley Clinic");
    assert!(assets::can_sell(&w, corp_clinic, arms, 3), "Chrome 3 sells tier 3");
    tech::set_tier(&mut w, z, Track::Chrome, 2);
    assert!(!assets::can_sell(&w, corp_clinic, arms, 3), "Chrome 2: no tier-3 implants");
    assert!(assets::can_sell(&w, corp_clinic, arms, 2));
    assert_eq!(tech::street_tier(&w, Track::Chrome), (2, Some(z)));
    assert!(assets::can_sell(&w, alley, arms, 2), "the street tier is 2");
    assert!(!assets::can_sell(&w, alley, arms, 3));
    let buyer = w
        .citizens()
        .into_iter()
        .find(|&a| {
            demography::is_adult(&w, a)
                && assets::assets_at(&w, a)
                    .iter()
                    .all(|&x| w.comp::<citysim::Asset>(x).is_none_or(|y| !y.kind.is_implant()))
        })
        .expect("an unchromed adult");
    w.comp_mut::<Wallet>(buyer).expect("wallet").coins = 100_000;
    let a = assets::buy(&mut w, buyer, alley, &ShopPick { kind: arms, tier: 2, used: None }).expect("bought");
    assert_eq!(w.comp::<citysim::Asset>(a).expect("asset").maker, Some(z), "made by the street tier's corp");
    tech::set_tier(&mut w, z, Track::Chrome, 1);
    assert!(w.corps().iter().all(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.tech.tier_of(Track::Chrome) < 2)));
    assert!(
        !assets::can_sell(&w, alley, arms, 2) && !assets::can_sell(&w, corp_clinic, arms, 2),
        "nobody sells tier 2"
    );
    assert!(assets::buy(
        &mut w,
        buyer,
        corp_clinic,
        &ShopPick { kind: AssetKind::Implant(Slot::Legs), tier: 2, used: None }
    )
    .is_err());
}

/// V23: Arms T3 from Zetatech fight at 0.3; Zetatech at Chrome 2, at 0.2;
/// the sanity load stays the tier-3 one.
#[test]
fn test_maker_tier_loss_lowers_kit() {
    let mut w = world();
    let z = corp_named(&w, "Zetatech");
    let clinic = *w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .find(|&&c| w.owner_of(c) == Some(z))
        .expect("Zetatech's Clinic");
    let agent = w
        .citizens()
        .into_iter()
        .find(|&a| {
            demography::is_adult(&w, a)
                && assets::assets_at(&w, a)
                    .iter()
                    .all(|&x| w.comp::<citysim::Asset>(x).is_none_or(|y| !y.kind.is_implant()))
        })
        .expect("an unchromed adult");
    w.comp_mut::<Wallet>(agent).expect("wallet").coins = 100_000;
    let pick = ShopPick { kind: AssetKind::Implant(Slot::Arms), tier: 3, used: None };
    let a = assets::buy(&mut w, agent, clinic, &pick).expect("bought");
    assert_eq!(w.comp::<citysim::Asset>(a).expect("asset").maker, Some(z));
    let k3 = w.comp::<Kit>(agent).expect("kit").clone();
    assert!((k3.fighting - 0.3).abs() < 1e-6, "{}", k3.fighting);
    tech::set_tier(&mut w, z, Track::Chrome, 2);
    let k2 = w.comp::<Kit>(agent).expect("kit").clone();
    assert!((k2.fighting - 0.2).abs() < 1e-6, "{}", k2.fighting);
    assert_eq!(k2.load, k3.load, "the load keeps the nominal tier");
    assert_eq!(assets::eff_tier(&w, a), 2);
    w.check_indices().expect("every Kit current after the rekit");
}

/// V23 calibration guard: after ten days with the caps on, every Kit equals
/// the Kit with every effective tier forced to the nominal one.
#[test]
fn test_caps_on_change_no_kit_at_seed() {
    let mut w = world();
    w.run_ticks(10 * TICKS_PER_DAY);
    let mut uncapped = w.clone();
    uncapped.config.virt.enabled = false;
    let mut n = 0;
    for id in w.citizens() {
        if w.comp::<Kit>(id).is_none() {
            continue;
        }
        assert_eq!(assets::compute_kit(&w, id), assets::compute_kit(&uncapped, id), "agent {id}");
        n += 1;
    }
    assert!(n > 1000);
    let made = assets::all_assets(&w)
        .iter()
        .filter(|&&a| w.comp::<citysim::Asset>(a).is_some_and(|x| x.maker.is_some()))
        .count();
    assert!(made > 0, "the guard compared Kits with at least one maker-capped asset");
    println!("{n} Kits compared; {made} assets with a maker");
}
