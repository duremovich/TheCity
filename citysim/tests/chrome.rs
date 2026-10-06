//! M13 phase 3: chrome at the Clinic (docs/M13_ASSETS.md § 3, plan 3.9):
//! the fight, sight and stealth terms, sanity and episodes, Install,
//! Therapy, Strip, Rip, the Harvest order, the Abducted hole and the
//! Statistical multipliers.

use rand::Rng;

use citysim::systems::faction::{self, OrderInputs};
use citysim::systems::{assets, bind, chrome, demography, gang, law, lod};
use citysim::world::StatRow;
use citysim::{
    Asset, AssetKind, AssetLoc, Body, Brain, BuildingKind, Config, Corp, DeathCause, EntityId, EventKind, Gang, Job,
    Kit, Lod, Memory, MemoryKind, Order, Personality, Position, Role, ShopPick, Skills, Slot, TilePos, Wallet, World,
};

/// Living adults with a Brain and a Wallet, ascending.
fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && w.has::<Wallet>(a) && demography::is_adult(w, a))
        .collect()
}

fn small() -> World {
    World::new(42, Config::load().scaled_to(300))
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |x| x.coins)
}

fn set_coins(w: &mut World, a: EntityId, c: i64) {
    w.comp_mut::<Wallet>(a).expect("wallet").coins = c;
}

fn asset(w: &World, a: EntityId) -> &Asset {
    w.comp::<Asset>(a).expect("asset")
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

/// Stand an agent on a street tile, out of any building, with a body.
fn place(w: &mut World, id: EntityId, tile: TilePos) {
    if w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
        lod::set_lod(w, id, Lod::Full);
    }
    w.remove_from_building(id);
    let p = w.comp_mut::<Position>(id).expect("position");
    p.tile = tile;
    p.building = None;
}

/// Agents not in a gang, without a job: free to be placed and enlisted.
fn free(w: &World) -> Vec<EntityId> {
    adults(w).into_iter().filter(|&a| w.gang_of(a).is_none() && !w.has::<Job>(a)).collect()
}

/// A seeded Clinic of an agent owner (price level 1.0), with `who` hired as
/// its Ripperdoc and the clock at 10:00 (on shift).
fn open_clinic(w: &mut World, who: EntityId) -> EntityId {
    let c = w
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .find(|&c| w.owner_of(c).is_some_and(|o| w.has::<Wallet>(o)))
        .expect("an agent's Clinic");
    demography::hire(w, who, c, Role::Ripperdoc);
    w.tick = 600;
    c
}

/// Plan 3.9, written against the M12 fight code before any Kit term: two
/// unchromed fighters roll exactly as in M12, assets on or off (D31:
/// branches, not zero terms).
#[test]
fn test_unchromed_fight_identical_to_m12() {
    let mut on = Config::load().scaled_to(300);
    on.assets.enabled = true;
    // The loot window (D13) defers a death's inheritance event; it is not a
    // fight term, so it is held at M12's 0 here.
    on.assets.loot_window_hours = 0;
    let mut off = on.clone();
    off.assets.enabled = false;
    let mut a = World::new(42, on);
    let mut b = World::new(42, off);
    let (ev_a, ev_b) = (a.events.len(), b.events.len());
    let mut log_a = Vec::new();
    let mut log_b = Vec::new();
    for i in 0..50 {
        for (w, log) in [(&mut a, &mut log_a), (&mut b, &mut log_b)] {
            let people = adults(w);
            let (x, y) = (people[(2 * i) % people.len()], people[(2 * i + 1) % people.len()]);
            assert!(w.comp::<citysim::Kit>(x).is_none_or(|k| k.is_bare()));
            let r = law::resolve_fight(w, x, y);
            log.push(r);
        }
    }
    assert_eq!(log_a, log_b, "identical (winner, loser, died)");
    let tail = |w: &World, from: usize| -> Vec<String> {
        w.events.iter().skip(from).map(|e| format!("{:?} {}", e.kind, e.text)).collect()
    };
    assert_eq!(tail(&a, ev_a), tail(&b, ev_b), "the same event log");
    assert_eq!(a.rng.world().random::<u64>(), b.rng.world().random::<u64>(), "the world stream in step");
}

#[test]
fn test_arms_raise_p_win_and_skin_lowers_death() {
    // Win share: Arms T3 against an equal bare fighter, no deaths.
    let mut cfg = Config::load();
    cfg.crime.fight_death_p = 0.0;
    let mut w = World::new(42, cfg);
    let people = free(&w);
    let (a, b) = (people[0], people[1]);
    assets::grant(&mut w, a, AssetKind::Implant(Slot::Arms), 3).expect("arms");
    let mut wins = 0;
    for _ in 0..2000 {
        for x in [a, b] {
            w.comp_mut::<Skills>(x).expect("skills").fighting = 0.3;
            w.comp_mut::<Personality>(x).expect("p").courage = 0.5;
        }
        let (winner, _, died) = law::resolve_fight(&mut w, a, b);
        assert!(!died);
        wins += usize::from(winner == a);
    }
    let share = wins as f32 / 2000.0;
    assert!((share - 0.62).abs() <= 0.03, "Arms T3 win share {share}");

    // Death share: the loser's death chance is 1 for a bare body, so a Skin
    // T2 loser's share is its (1 − armour) = 0.7.
    let mut w = World::new(43, Config::load());
    let pool = free(&w);
    let (attackers, victims) = pool.split_at(pool.len() / 3);
    for &v in victims {
        assets::grant(&mut w, v, AssetKind::Implant(Slot::Skin), 2).expect("skin");
    }
    let p = w.config.crime.fight_death_p as f32;
    let (mut losses, mut deaths, mut bare_losses, mut bare_deaths) = (0u32, 0u32, 0u32, 0u32);
    let (mut ai, mut vi) = (0usize, 0usize);
    while losses < 700 && vi < victims.len() && ai < attackers.len() {
        let (x, v) = (attackers[ai], victims[vi]);
        if !law::living(&w, x) {
            ai += 1;
            continue;
        }
        if !law::living(&w, v) {
            vi += 1;
            continue;
        }
        for y in [x, v] {
            w.comp_mut::<Skills>(y).expect("skills").fighting = 0.0;
        }
        // kill_mult so that a bare loser always dies: p × (1 + 0) × m = 1.
        let mods = law::FightMods { kill_mult: 1.0 / p, a_bonus: 0.0 };
        let (_, loser, died) = law::resolve_fight_mods(&mut w, x, v, mods);
        if loser == v {
            losses += 1;
            deaths += u32::from(died);
        } else {
            bare_losses += 1;
            bare_deaths += u32::from(died);
        }
    }
    assert!(losses >= 500, "enough fights: {losses}");
    let share = deaths as f32 / losses as f32;
    assert!((share - 0.7).abs() <= 0.05, "Skin T2 death share {share} (bare {bare_deaths}/{bare_losses})");
    assert_eq!(bare_deaths, bare_losses, "a bare loser always dies at this multiplier");
}

#[test]
fn test_load_past_psycho_starts_episode_ending_at_arrest() {
    let mut cfg = Config::load().scaled_to(300);
    cfg.chrome.episode_base = 5.0;
    let mut w = World::new(42, cfg);
    let people = free(&w);
    let who = people[0];
    for slot in [Slot::Arms, Slot::Nerves, Slot::Skin] {
        assets::grant(&mut w, who, AssetKind::Implant(slot), 3).expect("chrome");
    }
    assert!((w.comp::<Kit>(who).expect("kit").load - 0.75).abs() < 1e-6);
    w.comp_mut::<Body>(who).expect("body").sanity = 0.05;
    assert_eq!(w.tick_of_day(), 0, "midnight");
    assets::run(&mut w);
    assert!(chrome::in_episode(&w, who), "an episode at the midnight roll");
    assert_eq!(count(&w, EventKind::Episode), 1);
    assert_eq!(w.stats.current.episodes, 1);
    assert!(law::wanted(&w, who), "the law hunts it");
    // Guards come for it until one ends it (cuffed, or killed resisting).
    let tile = TilePos { x: 100, y: 100 };
    place(&mut w, who, tile);
    for g in people.iter().copied().skip(1).take(30) {
        if !chrome::in_episode(&w, who) || !law::living(&w, who) {
            break;
        }
        let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
        demography::hire(&mut w, g, jail, Role::Guard);
        place(&mut w, g, TilePos { x: 101, y: 100 });
        w.comp_mut::<Skills>(g).expect("skills").fighting = 1.0;
        law::arrest(&mut w, g, who);
    }
    assert!(!chrome::in_episode(&w, who), "the law ended it");
    assert_eq!(w.stats.current.episodes_by_law, 1);
    // Review fix: with assets off, an M9 arrest leaves a suspect's LOD rank
    // class where it was (only an abductee ranks with the gangs, D46).
    let mut off = Config::load().scaled_to(300);
    off.assets.enabled = false;
    let mut m9 = World::new(42, off);
    let civilians = free(&m9);
    let (suspect, guard) = (civilians[0], civilians[1]);
    let before = lod::rank_class(&m9, suspect);
    m9.comp_mut::<Brain>(suspect).expect("brain").cuffed_by = Some(guard);
    assert_eq!(lod::rank_class(&m9, suspect), before, "a cuffed suspect ranks as in M12");

    // Carry-over (phase 2 review): an episode survivor below 0.5 loyalty
    // leaves its gang, and its kept gang bike goes home.
    let member = people[40];
    let g = w.gangs()[0];
    let hideout = w.hideout_of(g).expect("hideout");
    gang::enlist(&mut w, member, g);
    w.comp_mut::<Personality>(member).expect("p").loyalty = 0.3;
    let bike = assets::spawn_asset(&mut w, AssetKind::Motorcycle, 1, Some(g), AssetLoc::Parked(hideout), 300);
    assets::set_keeper(&mut w, bike, Some(member));
    chrome::start_episode(&mut w, member);
    assert!(chrome::in_episode(&w, member));
    let now = w.tick;
    w.comp_mut::<Body>(member).expect("body").episode_until = Some(now);
    chrome::episodes_hourly(&mut w);
    assert!(!chrome::in_episode(&w, member) && w.gang_of(member).is_none(), "left the gang");
    let x = asset(&w, bike);
    assert_eq!((x.loc, x.keeper), (AssetLoc::Parked(hideout), None), "the gang's bike went home");
    assert!(w.comp::<Memory>(member).is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Episode)));
    w.check_indices().expect("indices in step");
}

#[test]
fn test_therapy_raises_sanity() {
    let mut w = small();
    let people = free(&w);
    let (doc, patient) = (people[0], people[1]);
    let c = open_clinic(&mut w, doc);
    w.comp_mut::<Body>(patient).expect("body").sanity = 0.4;
    set_coins(&mut w, patient, 100);
    let owner = w.owner_of(c);
    let (purse0, tax0, flow0) = (w.purse(owner), w.stats.current.flow_tax, w.stats.current.flow_treatment);
    assert!(chrome::therapy(&mut w, patient, c));
    let price = chrome::therapy_price(&w, c);
    assert_eq!(price, 60);
    assert!((w.comp::<Body>(patient).expect("body").sanity - 0.55).abs() < 1e-6);
    assert_eq!(coins(&w, patient), 40);
    assert_eq!(w.stats.current.flow_treatment - flow0, price, "Flow::Treatment");
    let tax = w.stats.current.flow_tax - tax0;
    assert!(tax > 0, "taxed");
    assert_eq!(w.purse(owner) - purse0, price - tax, "to the Clinic's owner");
    assert_eq!(count(&w, EventKind::Treated), 1);
}

#[test]
fn test_install_replaces_slot_with_buyback() {
    let mut w = small();
    // D18, phase 3 rows: the Spire's (the Tech corp's) and two back-alley docs.
    let clinics = w.buildings_of_kind(BuildingKind::Clinic).to_vec();
    let mut names: Vec<String> =
        clinics.iter().map(|&c| w.district_name(w.district_of_building(c)).to_string()).collect();
    names.sort();
    assert_eq!(names, ["Mid East", "Spire", "Sump West"]);
    let zetatech = w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == "Zetatech"));
    let spire = clinics.iter().copied().find(|&c| w.district_name(w.district_of_building(c)) == "Spire");
    assert_eq!(spire.and_then(|c| w.owner_of(c)), zetatech);
    let agents = clinics.iter().filter(|&&c| w.owner_of(c).is_some_and(|o| w.has::<Wallet>(o))).count();
    assert_eq!(agents, 2, "two back-alley docs");
    let people = free(&w);
    let (doc, buyer) = (people[0], people[1]);
    let c = open_clinic(&mut w, doc);
    let owner = w.owner_of(c);
    set_coins(&mut w, buyer, 2000);
    let t1 =
        chrome::buy_install(&mut w, buyer, c, &ShopPick { kind: AssetKind::Implant(Slot::Arms), tier: 1, used: None })
            .expect("T1");
    assert_eq!(asset(&w, t1).loc, AssetLoc::Installed(buyer));
    assert_eq!(coins(&w, buyer), 2000 - 150);
    let value = asset(&w, t1).value;
    let t2 =
        chrome::buy_install(&mut w, buyer, c, &ShopPick { kind: AssetKind::Implant(Slot::Arms), tier: 2, used: None })
            .expect("T2");
    let back = (0.4 * value as f32).round() as i64;
    assert_eq!(coins(&w, buyer), 2000 - 150 - 500 + back, "the T1 bought back at 0.4 x value");
    let x = asset(&w, t1);
    assert_eq!((x.loc, x.owner), (AssetLoc::Stock(c), owner), "the T1 in the Clinic's stock");
    assert_eq!(asset(&w, t2).loc, AssetLoc::Installed(buyer));
    assert_eq!(chrome::installed(&w, buyer), vec![t2]);
    assert_eq!(w.stats.current.chrome_installs, 2);
    assert_eq!(count(&w, EventKind::Installed), 2);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_strip_takes_loot_and_settles() {
    let mut w = small();
    let people = free(&w);
    let (dead, stripper) = (people[0], people[1]);
    let tile = TilePos { x: 120, y: 100 };
    place(&mut w, dead, tile);
    place(&mut w, stripper, tile);
    set_coins(&mut w, dead, 50);
    let pack = assets::grant(&mut w, dead, AssetKind::Pack, 1).expect("pack");
    w.kill_by(dead, DeathCause::Violence, None);
    assert_eq!(w.comp::<citysim::Corpse>(dead).expect("corpse").loot.coins, 50, "the loot window holds it");
    set_coins(&mut w, stripper, 0);
    let crimes0 = w.district(w.district_of(tile)).crimes_today;
    let total = citysim::systems::ownership::total_coins(&w);
    assert_eq!(chrome::loot_target(&w, stripper), Some(dead));
    assert!(chrome::strip(&mut w, stripper, dead));
    assert_eq!(coins(&w, stripper), 50);
    assert_eq!(asset(&w, pack).loc, AssetLoc::Carried(stripper));
    assert_eq!(w.district(w.district_of(tile)).crimes_today, crimes0 + 1, "a Theft raised");
    let k = w.comp::<citysim::Corpse>(dead).expect("corpse");
    assert!(k.settled && k.stripped);
    assert!(!w.loot_corpses.contains(&dead));
    assert_eq!(citysim::systems::ownership::total_coins(&w), total, "conserved");
    assert_eq!(count(&w, EventKind::Stripped), 1);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_rip_takes_every_implant_to_hideout() {
    let mut w = small();
    let people = free(&w);
    let (dead, ripper) = (people[0], people[1]);
    let g = w.gangs()[0];
    let hideout = w.hideout_of(g).expect("hideout");
    gang::enlist(&mut w, ripper, g);
    let tile = TilePos { x: 120, y: 100 };
    place(&mut w, dead, tile);
    place(&mut w, ripper, tile);
    let arms = assets::grant(&mut w, dead, AssetKind::Implant(Slot::Arms), 1).expect("arms");
    let eyes = assets::grant(&mut w, dead, AssetKind::Implant(Slot::Eyes), 1).expect("eyes");
    w.kill_by(dead, DeathCause::Violence, None);
    assert_eq!(chrome::rip(&mut w, ripper, dead), 2);
    for a in [arms, eyes] {
        let x = asset(&w, a);
        assert_eq!((x.loc, x.owner, x.stolen), (AssetLoc::Stock(hideout), Some(g), true));
    }
    assert_eq!(w.stats.current.harvests, 1);
    assert_eq!(count(&w, EventKind::Harvested), 1);
    w.check_indices().expect("indices in step");
}

#[test]
fn test_bound_abducted_hole_hands_implants_to_binders_gang() {
    for unknown in [false, true] {
        let mut cfg = Config::load().scaled_to(300);
        cfg.chrome.abduct_base = 1.0e6;
        cfg.bind.p_unknown = if unknown { 1.0 } else { 0.0 };
        let mut w = World::new(42, cfg);
        let people = free(&w);
        let (victim, crew) = (people[0], people[1]);
        // One gang member in the city: the only Abducted candidate.
        for m in w.with::<citysim::GangMember>() {
            gang::leave(&mut w, m, "test");
        }
        let g = w.gangs()[0];
        let hideout = w.hideout_of(g).expect("hideout");
        gang::enlist(&mut w, crew, g);
        // A day-0 city has no traces yet: the crew's is built live.
        w.insert(crew, citysim::Trace::default());
        w.comp_mut::<Gang>(g).expect("gang").order = Order::Harvest;
        let implants: Vec<EntityId> = [Slot::Arms, Slot::Eyes, Slot::Skin]
            .into_iter()
            .map(|s| assets::grant(&mut w, victim, AssetKind::Implant(s), 1).expect("chrome"))
            .collect();
        assert!(w.comp::<Kit>(victim).expect("kit").visible >= 3);
        lod::set_lod(&mut w, victim, Lod::Statistical);
        set_coins(&mut w, victim, 30);
        let crew_coins = coins(&w, crew);
        chrome::abduction_daily(&mut w);
        assert!(w.has::<citysim::Corpse>(victim), "taken");
        let hole = *w.holes.keys().find(|&&h| w.holes[&h].kind == citysim::HoleKind::Abducted).expect("a hole");
        for &a in &implants {
            assert!(matches!(asset(&w, a).loc, AssetLoc::Limbo(h) if h == hole));
        }
        let cands = bind::candidates(&w, &w.holes[&hole]);
        assert_eq!(cands.iter().map(|&(c, _)| c).collect::<Vec<_>>(), vec![crew], "only gang members");
        let bound = bind::bind(&mut w, hole).expect("bound");
        if unknown {
            assert_eq!(bound, citysim::Bound::Unknown);
            assert!(implants.iter().all(|&a| !w.has::<Asset>(a)), "destroyed");
            assert_eq!(coins(&w, crew), crew_coins);
        } else {
            assert_eq!(bound, citysim::Bound::Actor(crew));
            for &a in &implants {
                let x = asset(&w, a);
                assert_eq!((x.loc, x.owner, x.stolen), (AssetLoc::Stock(hideout), Some(g), true));
            }
            assert_eq!(coins(&w, crew), crew_coins + 30, "the coins as a robbery's");
        }
        w.check_indices().expect("indices in step");
    }
}

#[test]
fn test_eyes_widen_witness_radius() {
    let mut cfg = Config::load().scaled_to(300);
    cfg.crime.witness_base = 1.0;
    let mut w = World::new(42, cfg);
    let people = free(&w);
    let (actor, witness) = (people[0], people[1]);
    let r = if w.is_dark() { w.config.crime.sight_night_crime } else { w.config.crime.sight_day_crime };
    let tile = TilePos { x: 100, y: 100 };
    place(&mut w, actor, tile);
    place(&mut w, witness, TilePos { x: 100 + r as u8 + 2, y: 100 });
    let saw = |w: &World| {
        w.comp::<Memory>(witness)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(actor)))
    };
    law::raise_crime(&mut w, actor, None, citysim::Crime::Theft, tile);
    assert!(!saw(&w), "out of sight without Eyes");
    assets::grant(&mut w, witness, AssetKind::Implant(Slot::Eyes), 1).expect("eyes");
    law::raise_crime(&mut w, actor, None, citysim::Crime::Theft, tile);
    assert!(saw(&w), "Eyes T1 see two tiles further");
}

#[test]
fn test_harvest_scored_only_with_target_and_clinic() {
    let base = OrderInputs {
        frontier: 0,
        frontier_total: 30,
        rival_territory: 0,
        own: 5,
        rival: 5,
        heat: 0.0,
        prize: 0,
        grudge: false,
        greed: 0.9,
        courage: 0.5,
        pride: 0.5,
        raid_ready: true,
        rival_exists: true,
        sacked: false,
        jailed: 0,
        boss_jailed: false,
        breakout_ready: true,
        garrison: false,
        loyalty: 0.5,
        hoard: 0.0,
        hoard_corp: None,
        hoard_tilt: 0.1,
        target_cover: 0.0,
        jail_cover: 0.0,
        derelicts: 0,
        districts_held: 0,
        open_districts: 0,
        corp_prize: None,
        corp_cover: 0.0,
        corp_guards: 0,
        corp_raids: false,
        clinic_exists: true,
        harvest_target: Some((EntityId { index: 7, generation: 0 }, 900)),
        harvest_cover: 0.0,
        lawfulness: 0.2,
        treasury_x: 0.5,
    };
    let cfg = Config::load().gangs;
    let has = |i: &OrderInputs| faction::score_orders(i, &cfg).iter().any(|s| s.order == Order::Harvest);
    assert!(has(&base), "a target and a Clinic: scored");
    assert!(!has(&OrderInputs { harvest_target: None, ..base.clone() }), "no target");
    assert!(!has(&OrderInputs { clinic_exists: false, ..base.clone() }), "no Clinic");
    assert!(!has(&OrderInputs { own: 2, ..base.clone() }), "no crew");
    let mut tilted = cfg.clone();
    tilted.order_flat.harvest = 5.0;
    assert_eq!(faction::score_orders(&base, &tilted)[0].order, Order::Harvest);
}

#[test]
fn test_stat_multipliers_only_for_kitted() {
    let mut w = small();
    let people = free(&w);
    let (bare, kitted) = (people[0], people[1]);
    let row = StatRow { p_killed: 0.0123, p_assaulted: 0.0456, p_robbed: 0.0789, p_steal: 0.031, ..StatRow::default() };
    let m = w.config.lod.stat_violence_mult;
    let (k, a, r, s) = lod::stat_probs(&w, bare, &row);
    assert_eq!(
        (k.to_bits(), a.to_bits(), r.to_bits(), s.to_bits()),
        (
            (row.p_killed * m).to_bits(),
            (row.p_assaulted * m).to_bits(),
            (row.p_robbed * m).to_bits(),
            row.p_steal.to_bits()
        ),
        "a bare agent rolls the M12 numbers bit for bit"
    );
    assets::grant(&mut w, kitted, AssetKind::Implant(Slot::Nerves), 2).expect("nerves");
    assets::grant(&mut w, kitted, AssetKind::Implant(Slot::Skin), 2).expect("skin");
    let (k, a, r, _) = lod::stat_probs(&w, kitted, &row);
    let harm = 0.82 * 0.7;
    assert!((k - row.p_killed * m * harm).abs() < 1e-6, "p_killed x 0.82 x 0.7: {k}");
    assert!((a - row.p_assaulted * m * harm).abs() < 1e-6);
    let flash = w.comp::<Kit>(kitted).expect("kit").flash;
    assert!((r - row.p_robbed * m * (1.0 + 0.5 * flash)).abs() < 1e-6);
}
