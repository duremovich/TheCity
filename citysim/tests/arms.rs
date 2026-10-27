//! Jobs & lives phase 3, weapons (docs/JOBS_AND_LIVES.md § 4 and § 12's
//! `weapons.rs` unit tests, here as `arms.rs`: `tests/weapons.rs` is M17's
//! weapons that reach the ledger; Dylan's answer 1, the owned-item store): the
//! fight's tier term (unarmed rolls as before; armed moves `p_win` by
//! `weapon_w` a tier), a seized crate in the Precinct, the licence at a
//! sighting, the concealment contest by tier, the petty thefts and the
//! fence, the muster's issue, and a save round trip. Each test is the
//! shipped seed-42 city and one mechanism, driven by calling the system
//! directly (they fire rarely, or only after a set-up a run reaches by chance).

use rand::Rng;

use citysim::systems::{demography, law, weapons};
use citysim::{
    Asset, AssetLoc, Brain, BuildingKind, Config, Crime, EntityId, EventKind, Gang, Household, Job, Kit, Personality,
    Position, ProvKind, Provenance, Role, Skills, TilePos, Wallet, World,
};

fn on_world() -> World {
    let mut cfg = Config::load();
    // OPEN-111: the venues' evening staff move seed 42's gangs (no gang of
    // three with a Hideout by day 12); pinned to the day shift.
    cfg.service.evening_staff.clear();
    // R6a: the trades' verbs (the cab's dispatch, the pit's bettors) moved the gang the crate tests are cut from.
    cfg.trade_verbs = None;
    World::new(42, cfg)
}

/// Free adults with a Brain and a Wallet, no job, no gang (ascending).
fn civilians(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            w.has::<Brain>(a)
                && w.has::<Wallet>(a)
                && demography::is_adult(w, a)
                && w.gang_of(a).is_none()
                && !w.has::<Job>(a)
        })
        .collect()
}

fn arm(w: &mut World, who: EntityId, tier: u8) -> EntityId {
    let p = Provenance { how: ProvKind::Granted, by: None, at: w.tick };
    weapons::spawn_weapon(w, tier, Some(who), AssetLoc::Carried(who), p, true).expect("a weapon")
}

fn kit_weapon(w: &World, a: EntityId) -> u8 {
    w.comp::<Kit>(a).map_or(0, |k| k.weapon)
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

fn tile_of(w: &World, a: EntityId) -> TilePos {
    w.comp::<Position>(a).expect("position").tile
}

/// Stand `who` on `at` (no building).
fn stand(w: &mut World, who: EntityId, at: TilePos) {
    w.remove_from_building(who);
    let p = w.comp_mut::<Position>(who).expect("position");
    p.tile = at;
    p.building = None;
}

#[test]
fn test_unarmed_fight_rolls_as_before() {
    // The same world twice, `[weapons]` on and off: fifty fights between
    // unarmed civilians give the same outcomes and leave the world stream
    // in step (the tier term is a branch, `tests/chrome.rs`'s pattern).
    let mut cfg = Config::load();
    cfg.assets.loot_window_hours = 0;
    let mut a = World::new(42, cfg);
    let mut b = a.clone();
    b.config.weapons.enabled = false;
    let people = civilians(&a);
    let (mut log_a, mut log_b) = (Vec::new(), Vec::new());
    for i in 0..50 {
        for (w, log) in [(&mut a, &mut log_a), (&mut b, &mut log_b)] {
            let (x, y) = (people[(2 * i) % people.len()], people[(2 * i + 1) % people.len()]);
            if !law::living(w, x) || !law::living(w, y) {
                log.push(None);
                continue;
            }
            assert_eq!(weapons::fight_tiers(w, x, y), None, "an unarmed pair");
            log.push(Some(law::resolve_fight(w, x, y)));
        }
    }
    assert_eq!(log_a, log_b, "identical (winner, loser, died)");
    assert_eq!(a.rng.world().random::<u64>(), b.rng.world().random::<u64>(), "the world stream in step");
}

#[test]
fn test_armed_fight_moves_p_win_by_weapon_w() {
    let mut w = on_world();
    let people = civilians(&w);
    let (a, b) = (people[0], people[1]);
    for x in [a, b] {
        w.comp_mut::<Skills>(x).expect("skills").fighting = 0.3;
        w.comp_mut::<Personality>(x).expect("p").courage = 0.5;
    }
    let base = law::p_win_of(&w, a, b, 0.0, None);
    assert!((base - 0.5).abs() < 1e-6, "an even pair: {base}");
    arm(&mut w, a, 2);
    assert_eq!(kit_weapon(&w, a), 2, "the Kit reads the carried tier");
    let arms = weapons::fight_tiers(&w, a, b);
    assert_eq!(arms, Some((2, 0)));
    let armed = law::p_win_of(&w, a, b, 0.0, arms);
    let want = 2.0 * w.config.weapons.weapon_w;
    assert!((armed - base - want).abs() < 1e-6, "p_win {base} -> {armed}, want +{want}");
    // The other way round: the defender's tier counts against the attacker.
    let back = law::p_win_of(&w, b, a, 0.0, weapons::fight_tiers(&w, b, a));
    assert!((base - back - want).abs() < 1e-6, "p_win {base} -> {back}");
    // The loser's death chance × (1 + kill_w × the winner's tier).
    let k = weapons::kill_mult(&w.config.weapons, 2);
    assert!((k - (1.0 + 2.0 * w.config.weapons.kill_w)).abs() < 1e-6);
    // Off, the same Kit reads unarmed.
    w.config.weapons.enabled = false;
    assert_eq!(weapons::fight_tiers(&w, a, b), None);
}

#[test]
fn test_winner_takes_the_losers_weapon() {
    let mut w = on_world();
    w.config.crime.fight_death_p = 0.0;
    w.config.wounded.wound_share = 0.0;
    let people = civilians(&w);
    let (a, b) = (people[2], people[3]);
    let g = arm(&mut w, b, 1);
    // Fight until `a` wins one (the armed side wins more often).
    let mut won = false;
    for _ in 0..200 {
        let (winner, ..) = law::resolve_fight(&mut w, a, b);
        if winner == a {
            won = true;
            break;
        }
    }
    assert!(won, "the unarmed side wins some fights");
    let x = w.comp::<Asset>(g).expect("the weapon");
    assert_eq!(x.loc, AssetLoc::Carried(a), "taken by the winner");
    assert_eq!(x.owner, Some(a));
    assert!(x.stolen);
    assert_eq!(x.prov.map(|p| p.how), Some(ProvKind::Stolen));
    assert_eq!(kit_weapon(&w, b), 0);
    assert_eq!(kit_weapon(&w, a), 1);
}

/// The seed city run until a gang has members.
fn city_with_a_gang() -> (World, EntityId) {
    let mut w = on_world();
    // Day by day until a gang qualifies (when one forms is city history).
    let qualifies =
        |w: &World, g: EntityId| w.comp::<Gang>(g).is_some_and(|x| x.members.len() >= 3) && w.hideout_of(g).is_some();
    let mut day = 0;
    while day < 60 && !w.gangs().into_iter().any(|g| qualifies(&w, g)) {
        for _ in 0..citysim::TICKS_PER_DAY {
            w.tick();
        }
        day += 1;
    }
    let gang = w
        .gangs()
        .into_iter()
        .filter(|&g| qualifies(&w, g))
        .max_by_key(|&g| w.comp::<Gang>(g).map_or(0, |x| x.members.len()))
        .expect("a gang with members within 60 days");
    // Pin the precondition: the gang's armoury is under target (whether its
    // own history already filled it is incidental), so it ships a crate.
    let h = w.hideout_of(gang).expect("hideout");
    let target = (w.comp::<Gang>(gang).map_or(0, |g| g.members.len()) as f32 * w.config.weapons.armoury_per_member)
        .ceil() as u32;
    let have = weapons::stock_at(&w, h).len() as u32;
    w.config.weapons.deal_float = w.config.weapons.deal_float.max((have + 1).saturating_sub(target).max(6));
    (w, gang)
}

#[test]
fn test_seized_crate_lands_in_the_precinct() {
    let (mut w, gang) = city_with_a_gang();
    w.config.weapons.p_seize = 1.0;
    w.config.weapons.bribe_mult = 1.0;
    w.comp_mut::<Gang>(gang).expect("gang").treasury = 1000;
    w.jobs2.arms.last_crate.clear();
    let pre = w.building_of_kind(BuildingKind::Jail).expect("a Precinct");
    let before = weapons::stock_at(&w, pre).len();
    let leader = w.comp::<Gang>(gang).and_then(|g| g.leader.or_else(|| g.members.first().copied())).expect("a leader");
    weapons::crates(&mut w);
    let after = weapons::stock_at(&w, pre).len();
    assert!(after > before, "the crate is in the Precinct's stock ({before} -> {after})");
    assert!(count(&w, EventKind::CrateSeized) >= 1);
    assert!(
        w.crime_reports().iter().any(|r| !r.resolved && r.suspect == leader && r.crime == Crime::Smuggling),
        "Smuggling filed on the leader"
    );
    let h = w.hideout_of(gang).expect("hideout");
    assert!(weapons::stock_at(&w, h).iter().all(|&a| w.comp::<Asset>(a).is_some_and(|x| x.owner == Some(gang))));
}

#[test]
fn test_landed_crate_fills_the_armoury_and_the_muster_draws_it() {
    let (mut w, gang) = city_with_a_gang();
    w.config.weapons.p_seize = 0.0;
    w.comp_mut::<Gang>(gang).expect("gang").treasury = 1000;
    w.jobs2.arms.last_crate.clear();
    let h = w.hideout_of(gang).expect("hideout");
    let before = weapons::stock_at(&w, h).len();
    weapons::crates(&mut w);
    let stock = weapons::stock_at(&w, h);
    assert!(stock.len() > before, "the crate landed ({before} -> {})", stock.len());
    assert!(count(&w, EventKind::CrateLanded) >= 1);
    // A member marching out draws one, and it goes back at midnight.
    let m = w.comp::<Gang>(gang).expect("gang").members[0];
    let had = kit_weapon(&w, m);
    weapons::issue(&mut w, m);
    if had == 0 {
        assert!(kit_weapon(&w, m) > 0, "issued at the muster");
        assert_eq!(weapons::stock_at(&w, h).len(), stock.len() - 1);
    }
    weapons::daily(&mut w);
    if !w.has::<citysim::Sentence>(m) {
        assert_eq!(kit_weapon(&w, m), had, "returned to the armoury");
    }
}

#[test]
fn test_licence_passes_the_sighting() {
    let mut w = on_world();
    let people = civilians(&w);
    let c = people[4];
    arm(&mut w, c, 1);
    let g = w.guards()[0];
    let at = tile_of(&w, g);
    w.remove_from_building(g);
    if let Some(p) = w.comp_mut::<Position>(g) {
        p.building = None;
    }
    stand(&mut w, c, at);
    let reports =
        |w: &World| w.crime_reports().iter().filter(|r| r.suspect == c && r.crime == Crime::UnlicensedArms).count();
    // Licensed: a day of hourly sightings files nothing.
    w.jobs2.arms.licences.insert(c);
    for h in 0..24 {
        w.tick = h * citysim::TICKS_PER_HOUR;
        w.jobs2.arms.ua_today = 0;
        weapons::sight_carriers(&mut w);
    }
    assert_eq!(reports(&w), 0, "a licence holder passes");
    // Unlicensed: the guard beside him notices within the day.
    w.jobs2.arms.licences.remove(&c);
    for h in 24..48 {
        w.tick = h * citysim::TICKS_PER_HOUR;
        w.jobs2.arms.ua_today = 0;
        weapons::sight_carriers(&mut w);
    }
    assert!(reports(&w) >= 1, "an unlicensed carrier is reported");
}

#[test]
fn test_concealment_contest_at_each_tier() {
    let step = Config::load().weapons.conceal_step;
    // A bigger weapon is easier to notice, at every stealth.
    for stealth in [0.0f32, 0.5, 1.0] {
        let p: Vec<f32> = (1..=3).map(|t| weapons::notice_p(stealth, 0, t, 1, step)).collect();
        assert!(p[0] < p[1] && p[1] < p[2], "stealth {stealth}: {p:?}");
    }
    // Stealth and a pack hide it; a guard's Eyes find it.
    assert!(weapons::notice_p(1.0, 0, 2, 1, step) < weapons::notice_p(0.0, 0, 2, 1, step));
    assert!(weapons::notice_p(0.0, 2, 2, 1, step) < weapons::notice_p(0.0, 0, 2, 1, step));
    assert!(weapons::notice_p(0.0, 0, 2, 3, step) > weapons::notice_p(0.0, 0, 2, 1, step));
    for t in 1..=3 {
        let p = weapons::notice_p(0.5, 0, t, 1, step);
        assert!((0.05..=0.95).contains(&p), "tier {t}: {p}");
    }
}

#[test]
fn test_pickpocket_lifts_a_carried_item() {
    let mut w = on_world();
    w.config.weapons.p_pickpocket = 1.0;
    w.config.weapons.conceal_step = 1.0;
    let people = civilians(&w);
    let (thief, mark) = (people[5], people[6]);
    w.comp_mut::<Skills>(thief).expect("skills").stealth = 1.0;
    let g = arm(&mut w, mark, 1);
    let at = tile_of(&w, mark);
    stand(&mut w, mark, at);
    stand(&mut w, thief, at);
    let mut lifted = false;
    for t in 0..50 {
        w.tick = 1000 + t;
        weapons::market_petty(&mut w, thief);
        if w.comp::<Asset>(g).is_some_and(|x| x.loc == AssetLoc::Carried(thief)) {
            lifted = true;
            break;
        }
    }
    assert!(lifted, "the thief lifts the item");
    let x = w.comp::<Asset>(g).expect("weapon");
    assert!(x.stolen && x.owner == Some(thief));
    assert_eq!(w.stats.current.jobs2.as_ref().map_or(0, |j| j.pickpockets), 1);
    assert!(count(&w, EventKind::Theft) >= 1);
}

#[test]
fn test_market_thief_empties_a_stall_in_the_district() {
    let mut w = on_world();
    w.config.weapons.p_pickpocket = 1.0;
    let people = civilians(&w);
    let (thief, vendor) = (people[7], people[8]);
    let at = tile_of(&w, thief);
    stand(&mut w, thief, at);
    let d = w.district_of(at);
    let trade = w.config.trade_role("vendor").and_then(|r| r.trade()).expect("the vendor row");
    let pitch = citysim::Pitch {
        spot: at,
        district: d,
        trade,
        owner: vendor,
        since: w.tick,
        take_today: 25,
        take_week: 25,
        cut_to: None,
        stock: 0,
        closed_until: None,
        dry_days: 0,
        jobs_today: 0,
        cut_paid: 0,
    };
    w.jobs2.pitches.insert(vendor, pitch);
    w.comp_mut::<Wallet>(vendor).expect("wallet").coins = 50;
    let before = w.comp::<Wallet>(thief).map_or(0, |x| x.coins);
    weapons::market_petty(&mut w, thief);
    let max = w.config.weapons.stall_take_max;
    assert_eq!(w.comp::<Wallet>(vendor).map_or(0, |x| x.coins), 50 - max, "the till emptied up to the cap");
    assert_eq!(w.comp::<Wallet>(thief).map_or(0, |x| x.coins), before + max);
    assert_eq!(w.jobs2.pitches.get(&vendor).map(|p| p.take_today), Some(25 - max));
    assert_eq!(w.stats.current.jobs2.as_ref().map_or(0, |j| j.stall_thefts), 1);
}

#[test]
fn test_home_theft_takes_a_kept_item_and_the_fence_buys_it() {
    let (mut w, gang) = city_with_a_gang();
    let owner = civilians(&w)
        .into_iter()
        .find(|&a| w.comp::<Household>(a).and_then(|h| h.home).is_some())
        .expect("a housed civilian");
    let home = w.comp::<Household>(owner).and_then(|h| h.home).expect("home");
    let p = Provenance { how: ProvKind::Granted, by: None, at: w.tick };
    let g = weapons::spawn_weapon(&mut w, 2, Some(owner), AssetLoc::Parked(home), p, true).expect("weapon");
    let thief = w.comp::<Gang>(gang).expect("gang").members[0];
    weapons::home_petty(&mut w, thief, home);
    let x = w.comp::<Asset>(g).expect("weapon");
    assert_eq!(x.loc, AssetLoc::Carried(thief), "taken from the Home");
    assert!(x.stolen);
    // The gang fence pays fence_frac of its value into the thief's wallet.
    w.comp_mut::<Gang>(gang).expect("gang").treasury = 500;
    let coins = w.comp::<Wallet>(thief).map_or(0, |x| x.coins);
    let value = w.comp::<Asset>(g).map_or(0, |x| x.value);
    assert_eq!(weapons::fence_items(&mut w, thief, gang), 1);
    let paid = w.comp::<Wallet>(thief).map_or(0, |x| x.coins) - coins;
    assert_eq!(paid, ((value as f32 * w.config.weapons.fence_frac).round() as i64).max(1));
    let h = w.hideout_of(gang).expect("hideout");
    assert_eq!(w.comp::<Asset>(g).map(|x| (x.owner, x.loc)), Some((Some(gang), AssetLoc::Stock(h))));
}

#[test]
fn test_weapons_off_spawn_nothing() {
    let mut c = Config::load();
    c.weapons.enabled = false;
    let w = World::new(42, c);
    assert!(w.buildings_of_kind(BuildingKind::Gunsmith).is_empty());
    assert!(w.jobs2.arms.is_default());
    assert!(!w.config.trades.iter().any(|r| r.weapons && citysim::systems::classpay::trade_live(&w, r)));
    let on = on_world();
    assert!(!on.buildings_of_kind(BuildingKind::Gunsmith).is_empty(), "a Gunsmith is seeded");
    let staffed = on.buildings_of_kind(BuildingKind::Gunsmith).iter().any(|&b| !on.staff_of(b).is_empty());
    assert!(staffed, "its gunsmiths are hired");
    let role: Role = on.config.trade_role("gunsmith").expect("the gunsmith row");
    assert!(on.workers(role).iter().all(|&s| on.comp::<Job>(s).is_some_and(|j| j.role == role)));
}

/// Phase-8 review finding 1: an unlicensed weapon is petty. With a Theft
/// open too, the Theft sets the sentence (on d2eea01 the weapon outranked
/// it and the suspect served Vagrancy's night).
#[test]
fn test_unlicensed_arms_does_not_shorten_a_worse_sentence() {
    let mut w = on_world();
    let pool = civilians(&w);
    let (guard, a) = (w.guards()[0], pool[0]);
    assert!(citysim::Crime::UnlicensedArms < citysim::Crime::Theft, "petty");
    law::file_report(&mut w, citysim::Crime::Theft, a, None);
    law::file_report(&mut w, citysim::Crime::UnlicensedArms, a, None);
    let now = w.tick;
    law::jail_suspect(&mut w, guard, a);
    let s = w.comp::<citysim::Sentence>(a).expect("sentenced");
    assert_eq!(s.crime, citysim::Crime::Theft, "the worse crime sets the term");
    assert_eq!(s.until_tick - now, law::sentence_ticks(&w, citysim::Crime::Theft));
}
