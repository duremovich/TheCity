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
    // The flag aside (review finding 4).
    assert!(fitted.scale.fitted);
    fitted.scale.fitted = false;
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

/// Review finding 3: a roster dealt past 256 rows (a ~60,000 city's: k = 30,
/// 270 rows) keeps every row's slot: `corp_template` reads the row each
/// corp copies and the CSV's corp columns don't collide (a `u8` slot wrapped
/// row 256 onto row 0). Dealt by hand on the shipped map (k = 1 there).
#[test]
fn test_roster_past_256_rows_keeps_its_slots() {
    let mut c = Config::load();
    let blocks = c.corps.blocks.iter().sum();
    c.corps.replicate(30, [12, 3, 2, blocks]);
    assert_eq!(c.corps.row_count(), 270);
    let mut w = World::new(42, c);
    let mut seen = std::collections::BTreeSet::new();
    for &id in w.corp_slice() {
        let cc = w.comp::<citysim::Corp>(id).expect("a corp");
        let s = usize::from(cc.slot.expect("a seeded corp has a slot"));
        assert!(seen.insert(s), "slot {s} taken twice");
        assert_eq!(cc.name, w.config.corps.names[s], "slot {s} is its own row");
        assert_eq!(w.corp_template(id), w.config.corps.template_of(s));
    }
    assert_eq!(seen.len(), 270);
    w.run_ticks(citysim::TICKS_PER_DAY);
    let row = w.stats.history.back().expect("a closed day");
    assert_eq!(row.corps.len(), 270);
    let filled = row.corps.iter().filter(|c| c.is_some()).count();
    let alive =
        w.corp_slice().iter().filter(|&&id| w.comp::<citysim::Corp>(id).is_some_and(|c| c.slot.is_some())).count();
    assert_eq!(filled, alive, "one CSV slot per seeded corp");
}

/// Review finding 4: `fit_to_map` is idempotent. `World::new` fits, and a
/// world's own config (already fitted, and saved) handed back to
/// `World::new` (a replay) must not be fitted again: the per-Market keys
/// divided twice, the roster dealt again (2 copies -> 4 on the 4,000 map).
#[test]
fn test_fit_to_map_twice_is_fit_once() {
    let mut c = Config::load().scaled_to(4_000);
    c.world.map = "gen:4000".into();
    let map = MapSource::from_config(&c).load();
    map.apply_to(&mut c);
    c.fit_to_map(&map);
    assert!(c.corps.row_count() > 9, "the 4,000 map deals the roster again");
    let once = format!("{c:?}");
    map.apply_to(&mut c);
    c.fit_to_map(&map);
    assert_eq!(format!("{c:?}"), once, "a fitted config fits to itself");
}

/// Review finding 4: `World::replay(seed, world.config, log, end)` from a
/// generated map's world (its config fitted) runs the same city.
#[test]
fn test_replay_from_a_fitted_config_is_the_same_run() {
    let mut c = Config::load().scaled_to(4_000);
    c.world.map = "gen:4000".into();
    let mut w = World::new(42, c);
    w.run_ticks(3 * citysim::TICKS_PER_HOUR);
    w.push_command(citysim::PlayerCommand::SetSentenceMult(1.5));
    w.run_ticks(citysim::TICKS_PER_HOUR);
    let back = World::replay(42, w.config.clone(), &w.command_log, w.tick);
    assert_eq!(back.corp_slice().len(), w.corp_slice().len(), "the roster dealt once");
    assert!(citysim::save::to_ron(&back) == citysim::save::to_ron(&w), "the replay is the run");
}

/// Seeded buildings of `kind` per owner template: `(template, count)`.
fn seeded_by_template(w: &World, kind: BuildingKind) -> std::collections::BTreeMap<String, usize> {
    let mut m = std::collections::BTreeMap::new();
    for b in w.with::<citysim::Building>() {
        let Some(bd) = w.comp::<citysim::Building>(b).filter(|bd| bd.kind == kind && !bd.demolished) else { continue };
        let t =
            bd.owner.filter(|&o| w.has::<citysim::Corp>(o)).map_or("-".to_string(), |o| w.corp_template(o).to_string());
        *m.entry(t).or_insert(0) += 1;
    }
    m
}

/// Review finding 7: the role-seeding passes deal one copy per copy of the
/// roster (the corps `fit_to_map` dealt for the map), not per
/// `Config::copies` (the population ratio). The shipped map scaled to 6,000
/// keeps one roster (the map's employers are the 2,000 city's): one Lab per
/// row, each owned by its own row's corp (it built 3 copies of every Lab,
/// all owned by the one Zetatech, eating the Lots), one Nutrix Now.
#[test]
fn test_seeding_copies_follow_the_roster_on_a_scaled_shipped_map() {
    let w = World::new(42, Config::load().scaled_to(6_000));
    assert_eq!(w.corp_slice().len(), 9, "the shipped map deals one roster");
    let labs = seeded_by_template(&w, BuildingKind::Lab);
    assert_eq!(labs.get("Zetatech"), Some(&2), "{labs:?}");
    assert_eq!(labs.get("Arasaka"), Some(&1), "{labs:?}");
    assert_eq!(labs.get("Militech"), Some(&1), "{labs:?}");
    let feeds = seeded_by_template(&w, BuildingKind::Feed);
    assert_eq!(feeds.get("Nutrix"), Some(&1), "{feeds:?}");
}

/// Review finding 7, the generated map: the roster dealt twice (4,000), so
/// every copy of a Lab or Feed row's corp holds its own (no copy wraps onto
/// another's owner, none is skipped).
#[test]
fn test_seeding_copies_follow_the_roster_on_a_generated_map() {
    let mut c = Config::load().scaled_to(4_000);
    c.world.map = "gen:4000".into();
    let w = World::new(42, c);
    let k = w.corps_of_template("Zetatech").len();
    assert_eq!(k, 2);
    let mut per_corp = std::collections::BTreeMap::new();
    for b in w.with::<citysim::Building>() {
        let Some(bd) = w.comp::<citysim::Building>(b).filter(|bd| bd.kind == BuildingKind::Lab) else { continue };
        *per_corp.entry(bd.owner.expect("a corp's Lab")).or_insert(0) += 1;
    }
    for (template, each) in [("Zetatech", 2), ("Arasaka", 1), ("Militech", 1)] {
        for corp in w.corps_of_template(template) {
            assert_eq!(per_corp.get(&corp).copied().unwrap_or(0), each, "{template} copy {corp:?}'s Labs");
        }
    }
    for corp in w.corps_of_template("Nutrix") {
        let feeds = w
            .with::<citysim::Building>()
            .into_iter()
            .filter(|&b| {
                w.comp::<citysim::Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Feed && bd.owner == Some(corp))
            })
            .count();
        assert_eq!(feeds, 1, "each Nutrix copy runs its Feed");
    }
}
