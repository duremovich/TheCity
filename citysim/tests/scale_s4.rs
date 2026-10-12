//! Scale S4 (docs/SCALE_20K.md § S4): seeding by role (`districts::Seat`,
//! `seat_district`), the corp roster dealt per ~2,000 residents
//! (`CorpsCfg::replicate` through `Config::fit_to_map`, `[corps] template`,
//! `World::corps_of_template`), `Config::fit_to_map` on the shipped map and
//! `Config::scaled_to`'s up-only keys. The 2,000 city must resolve exactly
//! as before S4 (the old district names, the nine corps).

use citysim::map::MapSource;
use citysim::systems::districts::{seat_district, Seat};
use citysim::{BuildingKind, Config, World, Zone};

fn shipped() -> World {
    World::new(42, Config::load())
}

fn name(w: &World, seat: Seat, copy: usize, copies: usize) -> String {
    let d = seat_district(w, seat, copy, copies).expect("a district");
    w.districts[d.index()].name.clone()
}

/// One copy on the shipped map is the old named district of every row the
/// seeding passes use (Sump 0.0 / 0.5 / 0.9, Mid 0.0 / 0.75, the one Spire,
/// Civic and Vats).
#[test]
fn test_seat_resolves_the_shipped_names() {
    let w = shipped();
    let cases = [
        (Zone::Sump, 0.0, "Sump West"),
        (Zone::Sump, 0.5, "Sump Central"),
        (Zone::Sump, 0.9, "Sump East"),
        (Zone::Mid, 0.0, "Mid West"),
        (Zone::Mid, 0.75, "Mid East"),
        (Zone::Spire, 0.0, "Spire"),
        (Zone::Civic, 0.0, "Civic"),
        (Zone::Vats, 0.0, "Vats"),
    ];
    for (zone, pos, want) in cases {
        assert_eq!(name(&w, Seat::new(zone, pos), 0, 1), want, "{zone:?} at {pos}");
    }
}

/// Copy `j` of `n` takes the fraction `pos + j / n` (wrapped) of the zone's
/// districts west to east.
#[test]
fn test_seat_copy_j_of_n_steps_by_one_over_n() {
    let w = shipped();
    let sump0 = Seat::new(Zone::Sump, 0.0);
    let got: Vec<String> = (0..3).map(|j| name(&w, sump0, j, 3)).collect();
    assert_eq!(got, ["Sump West", "Sump Central", "Sump East"]);
    // 0.5 + 1/2 wraps to 0.0.
    let sump5 = Seat::new(Zone::Sump, 0.5);
    let got: Vec<String> = (0..2).map(|j| name(&w, sump5, j, 2)).collect();
    assert_eq!(got, ["Sump Central", "Sump West"]);
    // 0.75 of two districts is the second; 0.75 + 1/2 = 0.25 the first.
    let mid = Seat::new(Zone::Mid, 0.75);
    let got: Vec<String> = (0..2).map(|j| name(&w, mid, j, 2)).collect();
    assert_eq!(got, ["Mid East", "Mid West"]);
    // A copy index past `n` wraps (`copy % copies`).
    assert_eq!(name(&w, sump0, 4, 3), "Sump Central");
}

/// More copies than the zone has districts widen the list to the Mid's (the
/// generator folds the outer boroughs' Spire, Civic and Vats into Mid); the
/// Mid itself never widens.
#[test]
fn test_seat_widens_to_the_mid_districts() {
    let w = shipped();
    // Spire + Mid by centroid (west to east): Mid West, Mid East, Spire.
    let spire = Seat::new(Zone::Spire, 0.0);
    let got: Vec<String> = (0..2).map(|j| name(&w, spire, j, 2)).collect();
    assert_eq!(got, ["Mid West", "Mid East"]);
    let got: Vec<String> = (0..3).map(|j| name(&w, spire, j, 3)).collect();
    assert_eq!(got, ["Mid West", "Mid East", "Spire"]);
    // Three copies of a Mid row stay in the two Mid districts.
    let mid = Seat::new(Zone::Mid, 0.0);
    let got: Vec<String> = (0..3).map(|j| name(&w, mid, j, 3)).collect();
    assert_eq!(got, ["Mid West", "Mid West", "Mid East"]);
}

/// The shipped map at 2,000: every ratio is 1, so `fit_to_map` leaves the
/// whole config alone (the nine corps, no templates).
#[test]
fn test_fit_to_map_changes_nothing_on_the_shipped_map() {
    let base = Config::load();
    let mut c = base.clone();
    let map = MapSource::from_config(&c).load();
    map.apply_to(&mut c);
    let mut fitted = c.clone();
    fitted.fit_to_map(&map);
    assert_eq!(format!("{fitted:?}"), format!("{c:?}"));
    assert_eq!(fitted.corps.row_count(), 9);
    assert!(fitted.corps.template.is_empty());
    assert_eq!(fitted.corps.template_of(8), "Zetatech");
}

/// The generated 20,000 map: the roster dealt `k = round(map employers /
/// 17)` = 10 times ("Nutrix", ..., "Nutrix 2", ...), each row's template the
/// shipped row it copies, every Farm, Market and Security Office owned (the
/// 2,000 city owns all of its 12, 3 and 2) in the shipped proportions, the
/// treasuries split ten ways and the Blocks as at 2,000 per copy.
#[test]
fn test_corp_roster_dealt_per_2000_residents_on_the_20k_map() {
    let shipped = Config::load();
    let n = shipped.corps.row_count();
    assert_eq!(n, 9);
    let mut c = Config::load().scaled_to(20_000);
    c.world.map = "gen:20000".into();
    let map = MapSource::from_config(&c).load();
    map.apply_to(&mut c);
    c.fit_to_map(&map);
    let count = |k: BuildingKind| map.buildings.iter().filter(|b| b.kind == k).count() as u32;
    let (farms, markets, offices) =
        (count(BuildingKind::Farm), count(BuildingKind::Market), count(BuildingKind::SecurityOffice));
    assert_eq!((farms, markets, offices), (120, 30, 20), "the generated 20k map's employers");
    let k = ((farms + markets + offices) as f64 / 17.0).round() as usize;
    assert_eq!(k, 10);
    let corps = &c.corps;
    assert_eq!(corps.row_count(), n * k);
    for j in 0..k {
        for i in 0..n {
            let row = j * n + i;
            let base = &shipped.corps.names[i];
            let want = if j == 0 { base.clone() } else { format!("{base} {}", j + 1) };
            assert_eq!(corps.names[row], want);
            assert_eq!(corps.template_of(row), base.as_str());
            assert_eq!(corps.niches[row], shipped.corps.niches[i]);
            // `scaled_to` multiplied the treasuries by 10, the deal splits
            // them 10 ways: each copy starts as the shipped row.
            assert_eq!(corps.treasury_initial[row], shipped.corps.treasury_initial[i]);
            assert_eq!(corps.blocks[row], shipped.corps.blocks[i], "{want}'s Blocks");
        }
    }
    // Every employer owned, at the 2,000 city's share per template.
    let sum = |v: &Vec<u32>| v.iter().sum::<u32>();
    assert_eq!((sum(&corps.farms), sum(&corps.markets), sum(&corps.offices)), (farms, markets, offices));
    for i in 0..n {
        let per_template = |v: &Vec<u32>| (0..k).map(|j| v[j * n + i]).sum::<u32>();
        assert_eq!(per_template(&corps.farms), shipped.corps.farms[i] * 10, "{}'s Farms", shipped.corps.names[i]);
        assert_eq!(per_template(&corps.markets), shipped.corps.markets[i] * 10);
        assert_eq!(per_template(&corps.offices), shipped.corps.offices[i] * 10);
        // Each copy holds the shipped row's holdings (the counts divide evenly).
        for j in 0..k {
            assert_eq!(corps.farms[j * n + i], shipped.corps.farms[i]);
            assert_eq!(corps.markets[j * n + i], shipped.corps.markets[i]);
            assert_eq!(corps.offices[j * n + i], shipped.corps.offices[i]);
        }
    }
}

/// `replicate`'s largest remainder: totals that do not divide evenly go to
/// the rows with the largest remainders, ties to the lower row.
#[test]
fn test_corp_columns_redealt_by_largest_remainder() {
    let mut c = Config::load().corps;
    // Farms [6, 4, 2, 0 ...] twice, 25 to deal over weights summing to 24.
    c.replicate(2, [25, 7, 5, c.blocks.iter().sum()]);
    let n = 9;
    assert_eq!(c.farms.iter().sum::<u32>(), 25);
    // 6/24 x 25 = 6.25, 4/24 x 25 = 4.17, 2/24 x 25 = 2.08: the one left over
    // goes to the first Nutrix (the largest remainder, the lower row).
    assert_eq!(&c.farms[..3], &[7, 4, 2]);
    assert_eq!(&c.farms[n..n + 3], &[6, 4, 2]);
    // Markets [1, 1, 1] twice, 7 over 6: 1.17 each, the first row takes it.
    assert_eq!(&c.markets[..3], &[2, 1, 1]);
    assert_eq!(&c.markets[n..n + 3], &[1, 1, 1]);
    // Offices [Arasaka 1, Militech 1] twice, 5 over 4.
    assert_eq!((c.offices[6], c.offices[7], c.offices[n + 6], c.offices[n + 7]), (2, 1, 1, 1));
    // Rows with no holding of a kind get none.
    assert_eq!(c.farms[3..n].iter().sum::<u32>(), 0);
}

/// `World::corps_of_template`: every copy of a shipped row, by row; a copy's
/// `corp_template` is the row it copies. A 2,000 city whose roster is dealt
/// twice by hand (the map's employers then read k = 1: no second deal).
#[test]
fn test_corps_of_template_finds_every_copy() {
    let w = shipped();
    let z = w.corps_of_template("Zetatech");
    assert_eq!(z.len(), 1);
    assert_eq!(w.corp_template(z[0]), "Zetatech");

    let mut c = Config::load();
    let blocks = c.corps.blocks.iter().sum();
    c.corps.replicate(2, [12, 3, 2, blocks]);
    let w = World::new(42, c);
    assert_eq!(w.corp_slice().len(), 18);
    for template in ["Nutrix", "Zetatech", "Arasaka"] {
        let copies = w.corps_of_template(template);
        let names: Vec<String> =
            copies.iter().map(|&c| w.comp::<citysim::Corp>(c).map(|cc| cc.name.clone()).unwrap_or_default()).collect();
        assert_eq!(names, [template.to_string(), format!("{template} 2")]);
        for &c in &copies {
            assert_eq!(w.corp_template(c), template);
        }
    }
    assert!(w.corps_of_template("Nutrix 2").is_empty(), "a copy is no template");
}

/// `scaled_to`: the S4 keys scale up only (a city under 2,000 keeps the
/// 2,000 city's value), the older keys both ways; 2,000 changes nothing.
#[test]
fn test_scaled_to_s4_keys_scale_up_only() {
    let base = Config::load();
    assert_eq!(base.world.population, 2000);
    assert_eq!(format!("{:?}", base.clone().scaled_to(2000)), format!("{base:?}"));

    let small = Config::load().scaled_to(1000);
    assert_eq!((small.lod.max_full, small.lod.max_coarse, small.lod.private_quota), (50, 150, 12));
    assert_eq!(small.brain.plan_budget_per_tick, base.brain.plan_budget_per_tick);
    assert_eq!(small.hunt.max_hunts, base.hunt.max_hunts);
    assert_eq!(small.demography.migrants_max, base.demography.migrants_max);
    assert_eq!(small.street.seed_hotels, base.street.seed_hotels);
    assert_eq!(small.world_market.treasury_ref, base.world_market.treasury_ref);
    assert_eq!(small.per_capita_count(5), 5);
    assert_eq!(small.copies(), 1);
    // The older keys go down with the city.
    assert_eq!(small.world.market_initial, (f64::from(base.world.market_initial) / 2.0).round() as u32);
    assert_eq!(small.gangs.max_members, (base.gangs.max_members as f64 / 2.0).round() as usize);

    let big = Config::load().scaled_to(20_000);
    assert_eq!((big.lod.max_full, big.lod.max_coarse, big.lod.private_quota), (500, 1500, 120));
    assert_eq!(big.brain.plan_budget_per_tick, base.brain.plan_budget_per_tick * 10);
    assert_eq!(big.hunt.max_hunts, base.hunt.max_hunts * 10);
    assert_eq!(big.demography.migrants_max, base.demography.migrants_max * 10);
    assert_eq!(big.per_capita_count(5), 50);
    assert_eq!(big.copies(), 10);
    assert_eq!(big.lever_ceiling(60), 600);
}
