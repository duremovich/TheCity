//! L2 phase 4 (plan 4.7, spec § 3): faction violence off screen. The
//! ledger's victim side counts on-screen victims of sourced violence per
//! `(source, district, victim class)`, the daily pass rolls the Statistical
//! tier against those rates, and the binder lays a faction's holes only to
//! its members (or nobody). Everything here is counters, seeded dice rolls
//! and hole records between fictional agents of a simulated city.

mod common;

use citysim::ledger::{ActiveSource, VictimClass, ViolenceSource};
use citysim::systems::{bind, demography, fixes, fviolence, law, lod};
use citysim::{
    ActionInstance, ActionKind, Bound, Brain, Config, DeathCause, DistrictId, EntityId, Gang, GoalKind, Hole, HoleKind,
    Household, Lod, Order, Personality, Plan, PlayerCommand, Position, World, TICKS_PER_HOUR,
};
use common::civilians;

fn world(seed: u64) -> World {
    World::new(seed, Config::load().dole_city().scaled_to(300))
}

/// The first gang, eight civilians enlisted in it, and one of them.
fn gang_and_member(w: &mut World) -> (EntityId, EntityId) {
    let g = w.gangs()[0];
    let recruits: Vec<EntityId> = civilians(w).into_iter().rev().take(8).collect();
    for &m in &recruits {
        citysim::systems::gang::enlist(w, m, g);
    }
    let m = w.comp::<Gang>(g).and_then(|x| x.members.first().copied()).expect("a member");
    (g, m)
}

fn killed_total(w: &World) -> u32 {
    w.order_rates.cells.values().map(|c| u32::from(c.victims_today[0])).sum()
}

fn give_gang_work(w: &mut World, m: EntityId) {
    let now = w.tick;
    if let Some(p) = w.comp_mut::<Personality>(m) {
        p.loyalty = 1.0;
    }
    let b = w.comp_mut::<Brain>(m).expect("brain");
    b.plan = Some(Plan {
        goal: GoalKind::GangWork,
        target: None,
        steps: vec![ActionInstance { action: ActionKind::Extort, target: None, tile: None }],
        started_tick: now,
    });
    b.plan_step = 0;
}

#[test]
fn test_ledger_counts_contest_killing_in_victim_district_and_class() {
    let mut w = world(42);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let (g, m) = gang_and_member(&mut w);
    w.comp_mut::<Gang>(g).expect("gang").order = Order::Contest;
    give_gang_work(&mut w, m);
    let civ = civilians(&w);
    let (v, v2, v3) = (civ[0], civ[1], civ[2]);
    for a in [m, v, v3] {
        lod::set_lod(&mut w, a, Lod::Coarse);
    }
    assert_eq!(fviolence::source_of(&w, m, v), Some((ViolenceSource::Order(Order::Contest), Some(g), None)));
    let d = w.district_of(w.comp::<Position>(v).expect("pos").tile);
    w.kill_by(v, DeathCause::Violence, Some(m));
    let cell = w.order_rates.cells.get(&(ViolenceSource::Order(Order::Contest), d, VictimClass::Civilian));
    assert_eq!(cell.map(|c| c.victims_today[0]), Some(1), "one Killed, the victim's district, Civilian");
    assert_eq!(killed_total(&w), 1);

    // An off-screen hole adds nothing.
    lod::set_lod(&mut w, v2, Lod::Statistical);
    assert!(lod::stat_hit(&mut w, v2, HoleKind::Killed, None));
    assert_eq!(killed_total(&w), 1, "an off-screen killing never feeds the ledger");

    // A Hunt's killing adds nothing, even under a GangWork plan.
    w.push_command(PlayerCommand::Hunt { hunter: m, target: v3 });
    w.run_ticks(1);
    assert!(w.hunts.contains_key(&m), "the god Hunt is under way");
    give_gang_work(&mut w, m);
    lod::set_lod(&mut w, v3, Lod::Coarse);
    assert_eq!(fviolence::source_of(&w, m, v3), None, "Hunts are excluded");
    w.kill_by(v3, DeathCause::Violence, Some(m));
    assert_eq!(killed_total(&w), 1, "a Hunt killing adds nothing");
}

#[test]
fn test_rate_matches_worked_numbers() {
    // Ledger maths, Case B: 25 civilian bodies × 24 h × 14 days = 8,400
    // body-hours (350 body-days), victims K 1, A 9, R 6, the spec's order
    // prior at prior_weight 48.
    let mut w = world(43);
    w.config.fviolence.prior_weight = 48.0;
    w.config.fviolence.fv_mult = 1.0;
    w.config.fviolence.rate_days = 14;
    w.config.fviolence.prior.order = [0.0003, 0.002, 0.003, 0.0];
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    let key = (ViolenceSource::Order(Order::Contest), DistrictId(2), VictimClass::Civilian);
    w.order_rates.cells.clear();
    let cell = w.order_rates.cell_mut(key);
    let per_day = [[1u16, 0, 0, 0], [0, 3, 2, 0], [0, 3, 2, 0], [0, 3, 2, 0]];
    for day in 0..14 {
        cell.victims_today = per_day.get(day).copied().unwrap_or([0; 4]);
        cell.exposure_today = 600;
        cell.roll(14);
    }
    let r = fviolence::cell_rates(&w, key.0, key.1, key.2);
    let want = [(1.0 + 0.0144) / 398.0, (9.0 + 0.096) / 398.0, (6.0 + 0.144) / 398.0, 0.0];
    for k in 0..4 {
        assert!((f64::from(r[k]) - want[k]).abs() < 1e-6, "rate[{k}] {} vs {}", r[k], want[k]);
    }
    // An empty cell reads the prior (Case A).
    let a = fviolence::cell_rates(&w, ViolenceSource::Order(Order::Expand), DistrictId(3), VictimClass::Civilian);
    assert!((a[0] - 0.0003).abs() < 1e-7 && (a[1] - 0.002).abs() < 1e-7, "{a:?}");
}

/// A district with Statistical free civilians living in it (their Homes').
fn stat_district(w: &World) -> DistrictId {
    let mut count = std::collections::BTreeMap::<DistrictId, usize>::new();
    for &a in w.tier(Lod::Statistical) {
        if w.gang_of(a).is_none() && demography::is_adult(w, a) {
            if let Some(h) = w.comp::<Household>(a).and_then(|h| h.home) {
                *count.entry(w.district_of_building(h)).or_default() += 1;
            }
        }
    }
    count.into_iter().max_by_key(|&(d, n)| (n, std::cmp::Reverse(d))).map(|(d, _)| d).expect("a district")
}

#[test]
fn test_daily_pass_opens_killed_hole_in_contested_district() {
    let mut w = world(44);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let (g, _) = gang_and_member(&mut w);
    let d = stat_district(&w);
    // The god FactionStrike: Contest pinned, the district touched.
    w.push_command(PlayerCommand::FactionStrike { gang: g, district: d, days: 3 });
    w.run_ticks(1);
    assert_eq!(w.comp::<Gang>(g).map(|x| x.order), Some(Order::Contest));
    assert!(w.fv_active.iter().any(|s| s.district == d && s.faction == Some(g)));
    // Rates forced high: every Statistical adult there is killed, but the cap.
    w.config.fviolence.prior.order = [1.0, 0.0, 0.0, 0.0];
    w.config.fviolence.prior_weight = 1e6;
    w.config.fviolence.fv_mult = 1.0;
    // L2 phase 5: the per-class multipliers off (these priors are the test's own).
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    // The victim side alone (the hands' cap has its own tests below).
    w.config.fviolence.hands = None;
    fviolence::daily(&mut w);
    let holes: Vec<&Hole> = w
        .holes
        .values()
        .filter(|h| h.kind == HoleKind::Killed && h.source == Some(ViolenceSource::Order(Order::Contest)))
        .collect();
    assert!(!holes.is_empty(), "a Killed hole with the Contest source");
    assert!(holes.iter().all(|h| h.faction == Some(g) && h.consequential), "the gang is the acting faction");
    assert!(holes.iter().any(|h| h.district == d), "filed in the struck district");
    let cap = w.config.fviolence.day_cap.killed;
    assert_eq!(w.stats.current.living.fv_killed, cap, "the day cap bites");
    assert!(w.stats.current.living.fv_capped > 0, "fv_capped counts what the cap stopped");
    // The pin holds over the daily rescore.
    citysim::systems::faction::rescore(&mut w, g, 0.1);
    assert_eq!(w.comp::<Gang>(g).map(|x| x.order), Some(Order::Contest), "the strike pins Contest");
}

#[test]
fn test_faction_hole_binds_member_or_unknown_never_civilian() {
    let mut w = world(45);
    w.run_ticks(citysim::TICKS_PER_DAY + TICKS_PER_HOUR + 1);
    let (g, _) = gang_and_member(&mut w);
    let civ = civilians(&w);
    let tick = w.tick;
    let (mut bound, mut unknown) = (0, 0);
    for k in 0..500usize {
        let victim = civ[k % civ.len()];
        let kind = [HoleKind::Assaulted, HoleKind::Robbed][(k / civ.len()) % 2];
        let tile = w.comp::<Position>(victim).expect("pos").tile;
        let hole = Hole {
            id: citysim::hole_id(tick + (k / (2 * civ.len())) as u64, victim, kind),
            kind,
            victim,
            zone: w.map.zone(tile),
            district: w.district_of(tile),
            tick,
            event_id: 0,
            consequential: true,
            spouse: w.spouse_of(victim),
            loot: 0,
            home: w.comp::<Household>(victim).and_then(|h| h.home),
            gang: None,
            source: Some(ViolenceSource::Order(Order::Contest)),
            faction: Some(g),
            riot: None,
            prebound: None,
        };
        let id = bind::open_hole(&mut w, hole);
        match bind::bind(&mut w, id).expect("open") {
            Bound::Actor(a) => {
                assert_eq!(w.gang_of(a), Some(g), "hole {k}: bound to a non-member");
                bound += 1;
            }
            Bound::Unknown => unknown += 1,
        }
    }
    eprintln!("500 faction holes: {bound} bound to members, {unknown} Unknown");
    assert!(bound > 0, "some bound to members");
    let l = &w.stats.current.living;
    assert_eq!(l.fv_bound_wrong, 0);
    assert_eq!(l.fv_bound + l.fv_unknown, 500);
}

#[test]
fn test_riot_hole_binds_a_rioter() {
    use citysim::{Riot, RiotResponse, RiotTarget};
    let mut w = world(46);
    w.run_ticks(citysim::TICKS_PER_DAY + TICKS_PER_HOUR + 1);
    let civ = civilians(&w);
    let rioters: Vec<EntityId> = civ[10..40].to_vec();
    let jail = w.buildings_of_kind(citysim::BuildingKind::Jail)[0];
    let d = w.district_of_building(jail);
    let riot = Riot {
        id: 77,
        district: d,
        target: jail,
        kind: RiotTarget::Precinct,
        muster: jail,
        muster_at: w.tick,
        rioters: rioters.clone(),
        response: RiotResponse::Contain,
        started: w.tick,
        departed: None,
    };
    w.riots.push(riot);
    for &r in &rioters {
        w.rioter_of.insert(r, 77);
    }
    let src = ActiveSource { source: ViolenceSource::Riot, district: d, faction: None, riot: Some(77), episode: None };
    let mut bound = 0;
    for &v in &civ[..10] {
        lod::set_lod(&mut w, v, Lod::Statistical);
        assert!(lod::stat_hit(&mut w, v, HoleKind::Assaulted, Some(&src)));
        let id = *w.holes_by_agent.get(&v).and_then(|l| l.last()).expect("a hole");
        assert_eq!(w.holes.get(&id).map(|h| (h.source, h.riot)), Some((Some(ViolenceSource::Riot), Some(77))));
        if let Some(Bound::Actor(a)) = bind::bind(&mut w, id) {
            assert!(rioters.contains(&a), "bound to a non-rioter");
            bound += 1;
        }
    }
    assert!(bound > 0, "some riot hole bound to a rioter");
    assert_eq!(w.stats.current.living.fv_bound_wrong, 0);
}

#[test]
fn test_harvest_cell_abducts() {
    use citysim::{AssetKind, Kit, Slot};
    // The Harvest cell of `fviolence::daily` (M13's own abduction roll is gone).
    let build = || -> (World, EntityId, EntityId) {
        let cfg = Config::load().dole_city().scaled_to(300);
        let mut w = World::new(47, cfg);
        w.run_ticks(TICKS_PER_HOUR + 1);
        let (g, _) = gang_and_member(&mut w);
        w.comp_mut::<Gang>(g).expect("gang").order = Order::Harvest;
        // A Statistical civilian whose Home the gang holds.
        let v = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| {
                w.gang_of(a).is_none()
                    && demography::is_adult(&w, a)
                    && w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
            })
            .expect("a Statistical resident");
        let home = w.comp::<Household>(v).and_then(|h| h.home).expect("home");
        w.comp_mut::<Gang>(g).expect("gang").territory.push(home);
        citysim::systems::assets::grant(&mut w, v, AssetKind::Implant(Slot::Arms), 2).expect("granted");
        w.comp_mut::<Kit>(v).expect("kit").visible = 9;
        w.config.chrome.abduct_base = 1e9;
        (w, g, v)
    };
    let (mut on, g, v) = build();
    assert!(on.holes.values().all(|h| h.kind != HoleKind::Abducted));
    on.config.fviolence.prior = Default::default();
    on.config.fviolence.prior.order = [0.0; 4];
    on.config.fviolence.prior.harvest_abducted = 1.0;
    on.config.fviolence.prior_weight = 1e6;
    on.config.fviolence.fv_mult = 1.0;
    // L2 phase 5: the per-class multipliers off (these priors are the test's own).
    on.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    on.config.fviolence.day_cap.abducted = 1000;
    // The victim side alone (the hands' cap has its own tests below).
    on.config.fviolence.hands = None;
    fviolence::rebuild_active(&mut on);
    fviolence::daily(&mut on);
    assert!(!law::living(&on, v), "the Harvest cell abducts the visible chrome");
    let hole = on.holes.values().find(|h| h.victim == v).expect("the abduction's hole");
    assert_eq!(
        (hole.kind, hole.source, hole.faction),
        (HoleKind::Abducted, Some(ViolenceSource::Order(Order::Harvest)), Some(g))
    );
    assert!(on.stats.current.living.fv_abducted >= 1);
}

/// Review fix: a riot live at noon and over by midnight still has its
/// rates applied by the midnight pass (`FvTally::day_sources`), and its
/// holes bind among its rioters after it ended (`FvTally::riot_rosters`).
#[test]
fn test_riot_gone_by_midnight_still_rolls_and_binds_a_rioter() {
    use citysim::{Riot, RiotResponse, RiotTarget};
    let mut w = world(48);
    w.config.bind.p_unknown = 0.0;
    w.run_ticks(citysim::TICKS_PER_DAY + 12 * TICKS_PER_HOUR);
    let d = stat_district(&w);
    let civ = civilians(&w);
    let rioters: Vec<EntityId> = civ
        .iter()
        .copied()
        .filter(|&a| w.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical))
        .take(20)
        .collect();
    assert!(rioters.len() >= 5, "bodies to riot");
    let jail = w.buildings_of_kind(citysim::BuildingKind::Jail)[0];
    w.riots.push(Riot {
        id: 91,
        district: d,
        target: jail,
        kind: RiotTarget::Precinct,
        muster: jail,
        muster_at: w.tick,
        rioters: rioters.clone(),
        response: RiotResponse::Contain,
        started: w.tick,
        departed: None,
    });
    for &r in &rioters {
        w.rioter_of.insert(r, 91);
    }
    // Noon's hourly tally sees it live; then it ends.
    fviolence::tally_exposure(&mut w);
    w.riots.clear();
    for r in &rioters {
        w.rioter_of.remove(r);
    }
    assert!(fviolence::active_now(&w).iter().all(|s| s.source != ViolenceSource::Riot), "gone by midnight");
    w.config.fviolence.prior = Default::default();
    w.config.fviolence.prior.order = [0.0; 4];
    w.config.fviolence.prior.vendetta = [0.0; 4];
    w.config.fviolence.prior.episode = [0.0; 4];
    w.config.fviolence.prior.riot = [1.0, 0.0, 0.0, 0.0];
    w.config.fviolence.prior_weight = 1e6;
    w.config.fviolence.fv_mult = 1.0;
    // L2 phase 5: the per-class multipliers off (these priors are the test's own).
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    // The victim side alone: these rioters are bodies (with the hands a
    // riot of bodies strikes only on screen, its own test below).
    w.config.fviolence.hands = None;
    fviolence::daily(&mut w);
    let holes: Vec<u64> = w
        .holes
        .values()
        .filter(|h| h.source == Some(ViolenceSource::Riot) && h.riot == Some(91))
        .map(|h| h.id)
        .collect();
    assert!(!holes.is_empty(), "the day's riot rolled at midnight");
    assert!(w.fv_tally.day_sources.is_empty(), "the day's list is cleared");
    let mut bound = 0;
    for id in holes {
        if let Some(Bound::Actor(a)) = bind::bind(&mut w, id) {
            assert!(rioters.contains(&a), "bound to a non-rioter");
            bound += 1;
        }
    }
    assert!(bound > 0, "a riot hole bound to a rioter after the riot ended");
    assert_eq!(w.stats.current.living.fv_bound_wrong, 0);
}

/// Review fix: an agent the midnight pass hit is not skipped as "already hit
/// today" at the next midnight (its hole carries the previous midnight's tick).
#[test]
fn test_yesterdays_pass_hole_does_not_shield_today() {
    let mut w = world(49);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let (g, _) = gang_and_member(&mut w);
    let d = stat_district(&w);
    w.push_command(PlayerCommand::FactionStrike { gang: g, district: d, days: 5 });
    w.run_ticks(1);
    w.config.fviolence.prior.order = [0.0, 0.0, 1.0, 0.0];
    w.config.fviolence.prior_weight = 1e6;
    w.config.fviolence.fv_mult = 1.0;
    // L2 phase 5: the per-class multipliers off (these priors are the test's own).
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    w.config.fviolence.day_cap.robbed = 1;
    // The victim side alone (the hands' cap has its own tests below).
    w.config.fviolence.hands = None;
    w.tick = (w.tick / citysim::TICKS_PER_DAY + 1) * citysim::TICKS_PER_DAY;
    fviolence::daily(&mut w);
    let first: Vec<EntityId> = w.holes.values().filter(|h| h.source.is_some()).map(|h| h.victim).collect();
    assert_eq!(first.len(), 1, "one robbery under the cap");
    w.tick += citysim::TICKS_PER_DAY;
    fviolence::daily(&mut w);
    let robbed: Vec<EntityId> =
        w.holes.values().filter(|h| h.source.is_some() && h.victim == first[0]).map(|h| h.victim).collect();
    assert_eq!(robbed.len(), 2, "the first victim, lowest id, is robbed again the next midnight");
}

// ---------------------------------------------------------------------------
// The V3 violence fixes (2026-10-09, `[life] violence_fixes`): the ledger's
// Street class, and short sources rolled for their live hours.
// ---------------------------------------------------------------------------

/// `world(seed)` with the fixes asserted on (the shipped config).
fn fixes_world(seed: u64) -> World {
    let w = world(seed);
    assert!(fixes::violence_on(&w), "[life] violence_fixes on in the shipped config");
    w
}

fn housed(w: &World, a: EntityId) -> bool {
    w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
}

fn unhome(w: &mut World, a: EntityId) {
    w.comp_mut::<Household>(a).expect("household").home = None;
}

#[test]
fn test_street_class_is_the_homeless_civilian_only_with_the_fix() {
    let mut w = fixes_world(42);
    let civ: Vec<EntityId> = civilians(&w).into_iter().filter(|&a| housed(&w, a)).collect();
    let (a, b) = (civ[0], civ[1]);
    assert_eq!(fviolence::victim_class(&w, a), VictimClass::Civilian, "housed: Civilian");
    unhome(&mut w, b);
    assert_eq!(fviolence::victim_class(&w, b), VictimClass::Street, "no Home: Street");
    // A homeless gang member stays a Member.
    let g = w.gangs()[0];
    citysim::systems::gang::enlist(&mut w, b, g);
    assert_eq!(fviolence::victim_class(&w, b), VictimClass::Member);
    // Fix off: the homeless are Civilians again (the 644eb10 ledger).
    let c = civ[2];
    unhome(&mut w, c);
    w.config.life.violence_fixes = false;
    assert_eq!(fviolence::victim_class(&w, c), VictimClass::Civilian);
    w.config.life.violence_fixes = true;
    w.config.life.vf_street_class = false;
    assert_eq!(fviolence::victim_class(&w, c), VictimClass::Civilian, "the sub-switch alone");
}

#[test]
fn test_evictee_killing_is_learned_as_street_not_civilian() {
    let mut w = fixes_world(42);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let g = w.gangs()[0];
    let recruits: Vec<EntityId> = civilians(&w).into_iter().rev().take(8).collect();
    for &m in &recruits {
        citysim::systems::gang::enlist(&mut w, m, g);
    }
    let m = w.comp::<Gang>(g).and_then(|x| x.members.first().copied()).expect("a member");
    w.comp_mut::<Gang>(g).expect("gang").order = Order::Contest;
    if let Some(p) = w.comp_mut::<Personality>(m) {
        p.loyalty = 1.0;
    }
    let now = w.tick;
    let br = w.comp_mut::<Brain>(m).expect("brain");
    br.plan = Some(Plan {
        goal: GoalKind::GangWork,
        target: None,
        steps: vec![ActionInstance { action: ActionKind::Extort, target: None, tile: None }],
        started_tick: now,
    });
    br.plan_step = 0;
    let v = civilians(&w).into_iter().find(|&a| housed(&w, a)).expect("a civilian");
    unhome(&mut w, v);
    for a in [m, v] {
        lod::set_lod(&mut w, a, Lod::Coarse);
    }
    let d = w.district_of(w.comp::<Position>(v).expect("pos").tile);
    w.kill_by(v, DeathCause::Violence, Some(m));
    let key = |c| (ViolenceSource::Order(Order::Contest), d, c);
    let street = w.order_rates.cells.get(&key(VictimClass::Street)).map(|c| c.victims_today[0]);
    let civ = w.order_rates.cells.get(&key(VictimClass::Civilian)).map_or(0, |c| c.victims_today[0]);
    assert_eq!(street, Some(1), "the evictee's death is the Street cell's");
    assert_eq!(civ, 0, "and never the housed civilians'");
    // The parity tallies still count it as a civilian victim.
    assert_eq!(w.fv_tally.body_civ_victims, 1);
}

#[test]
fn test_street_prior_is_the_civilian_prior_times_the_mult() {
    let mut w = fixes_world(43);
    w.order_rates.cells.clear();
    w.config.fviolence.fv_mult = 1.0;
    w.config.life.vf_street_prior_mult = 3.0;
    let s = ViolenceSource::Order(Order::Contest);
    let civ = fviolence::cell_rates(&w, s, DistrictId(2), VictimClass::Civilian);
    let street = fviolence::cell_rates(&w, s, DistrictId(2), VictimClass::Street);
    for k in 0..4 {
        assert!((street[k] - 3.0 * civ[k]).abs() < 1e-7, "kind {k}: {} vs 3 x {}", street[k], civ[k]);
    }
    assert!(street[0] > 0.0);
}

#[test]
fn test_street_rates_are_not_applied_to_the_housed() {
    let run = |fix: bool| {
        let mut w = fixes_world(44);
        w.run_ticks(TICKS_PER_HOUR + 1);
        let g = w.gangs()[0];
        let d = stat_district(&w);
        w.push_command(PlayerCommand::FactionStrike { gang: g, district: d, days: 3 });
        w.run_ticks(1);
        w.config.life.violence_fixes = fix;
        // No prior: only what the ledger learned acts.
        w.config.fviolence.prior.order = [0.0; 4];
        w.config.fviolence.fv_mult = 1.0;
        w.config.fviolence.day_cap.killed = 10_000;
        // The victim side alone (the hands' cap has its own tests below).
        w.config.fviolence.hands = None;
        // The ledger learned a deadly street: the evictees' cell (Street with
        // the fix, Civilian without) full of killings.
        let class = if fix { VictimClass::Street } else { VictimClass::Civilian };
        let cell = w.order_rates.cell_mut((ViolenceSource::Order(Order::Contest), d, class));
        cell.victims_today = [500, 0, 0, 0];
        cell.exposure_today = 24 * 500;
        cell.roll(14);
        fviolence::daily(&mut w);
        // The hole keeps the victim's Home at the killing (the dead are unlinked).
        let killed: Vec<bool> = w
            .holes
            .values()
            .filter(|h| h.kind == HoleKind::Killed && h.source.is_some())
            .map(|h| h.home.is_some())
            .collect();
        (killed.len(), killed.iter().filter(|&&h| h).count())
    };
    let (off_all, off_housed) = run(false);
    assert!(off_housed > 10, "without the fix the evictees' rate kills the housed ({off_housed} of {off_all})");
    let (_, on_housed) = run(true);
    assert_eq!(on_housed, 0, "with the fix no housed civilian reads the Street cell");
}

#[test]
fn test_live_share_is_hours_over_24() {
    let s = ActiveSource {
        source: ViolenceSource::Riot,
        district: DistrictId(1),
        faction: None,
        riot: Some(7),
        episode: None,
    };
    let other = ActiveSource { riot: Some(8), ..s };
    assert!((fviolence::live_share(&[(s, 6)], &s) - 0.25).abs() < 1e-7);
    assert!((fviolence::live_share(&[(s, 6)], &other) - 1.0 / 24.0).abs() < 1e-7, "untallied: one hour");
    assert!((fviolence::live_share(&[(s, 30)], &s) - 1.0).abs() < 1e-7, "at most a day");
}

/// A riot live at `hours` hourly tallies in the biggest Statistical
/// district, a Killed riot prior of 0.9 and no cap: the Statistical
/// adults the midnight pass kills.
fn riot_kills(fix: bool, hours: u32) -> (u32, u32) {
    use citysim::{Riot, RiotResponse, RiotTarget};
    let mut w = fixes_world(48);
    w.config.life.violence_fixes = fix;
    w.run_ticks(citysim::TICKS_PER_DAY + 6 * TICKS_PER_HOUR);
    let d = stat_district(&w);
    let rioters: Vec<EntityId> = civilians(&w)
        .into_iter()
        .filter(|&a| w.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical))
        .take(10)
        .collect();
    let jail = w.buildings_of_kind(citysim::BuildingKind::Jail)[0];
    w.riots.push(Riot {
        id: 91,
        district: d,
        target: jail,
        kind: RiotTarget::Precinct,
        muster: jail,
        muster_at: w.tick,
        rioters: rioters.clone(),
        response: RiotResponse::Contain,
        started: w.tick,
        departed: None,
    });
    for &r in &rioters {
        w.rioter_of.insert(r, 91);
    }
    for _ in 0..hours {
        fviolence::tally_exposure(&mut w);
    }
    let tallied = w.fv_tally.source_hours.iter().find(|(s, _)| s.riot == Some(91)).map_or(0, |&(_, h)| h);
    w.riots.clear();
    for r in &rioters {
        w.rioter_of.remove(r);
    }
    w.config.fviolence.prior = Default::default();
    w.config.fviolence.prior.order = [0.0; 4];
    w.config.fviolence.prior.vendetta = [0.0; 4];
    w.config.fviolence.prior.episode = [0.0; 4];
    w.config.fviolence.prior.riot = [0.9, 0.0, 0.0, 0.0];
    w.config.fviolence.prior_weight = 1e6;
    w.config.fviolence.fv_mult = 1.0;
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    w.config.fviolence.day_cap.killed = 10_000;
    // The victim side alone: these rioters are bodies.
    w.config.fviolence.hands = None;
    fviolence::daily(&mut w);
    assert!(w.fv_tally.source_hours.is_empty(), "the day's hours are cleared");
    (w.stats.current.living.fv_killed, tallied)
}

#[test]
fn test_short_riot_rolls_its_hours_not_a_day() {
    let (off, off_hours) = riot_kills(false, 6);
    assert_eq!(off_hours, 0, "the fix off tallies no hours");
    let (on6, on_hours) = riot_kills(true, 6);
    assert_eq!(on_hours, 6, "six hourly tallies");
    let (on24, _) = riot_kills(true, 24);
    assert!(off > 15, "a full day's 0.9 kills many ({off})");
    // A 24-hour riot is the old full day (same rolls).
    assert_eq!(on24, off, "a day-long riot rolls as before");
    // A 6-hour riot: a quarter of the rate (0.225 vs 0.9): well under half.
    assert!(on6 * 2 < off && on6 > 0, "six hours {on6} vs a day {off}");
}

// ---------------------------------------------------------------------------
// The island pass phase 10 (OPEN-185, `[fviolence.hands]`): a source strikes
// off screen only with its off-screen hands, each hit pre-bound to one.
// ---------------------------------------------------------------------------

/// `world(44)` an hour in, the first gang enlisting eight civilians, every
/// member loyal (following the order), struck on the biggest Statistical
/// district under a pinned Contest with a deadly order prior, no day cap;
/// the first `stat` members Statistical and off screen since before
/// yesterday, the rest Coarse bodies. The gang, its Statistical members,
/// its bodies.
fn hands_world(stat: usize) -> (World, EntityId, Vec<EntityId>, Vec<EntityId>) {
    let mut w = world(44);
    assert!(fviolence::hands_on(&w), "[fviolence.hands] on in the shipped config");
    w.run_ticks(TICKS_PER_HOUR + 1);
    let (g, _) = gang_and_member(&mut w);
    let d = stat_district(&w);
    w.push_command(PlayerCommand::FactionStrike { gang: g, district: d, days: 3 });
    w.run_ticks(1);
    let members = w.comp::<Gang>(g).map(|x| x.members.clone()).expect("gang");
    assert!(members.len() > stat, "members {}", members.len());
    let (mut off, mut on) = (Vec::new(), Vec::new());
    for (i, &m) in members.iter().enumerate() {
        if let Some(p) = w.comp_mut::<Personality>(m) {
            p.loyalty = 1.0;
        }
        if i < stat {
            lod::set_lod(&mut w, m, Lod::Statistical);
            w.comp_mut::<Brain>(m).expect("brain").body_day = None;
            off.push(m);
        } else {
            lod::set_lod(&mut w, m, Lod::Coarse);
            on.push(m);
        }
    }
    off.sort_unstable();
    w.config.fviolence.prior.order = [1.0, 0.0, 0.0, 0.0];
    w.config.fviolence.prior_weight = 1e6;
    w.config.fviolence.fv_mult = 1.0;
    w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
    w.config.fviolence.day_cap.killed = 10_000;
    (w, g, off, on)
}

fn killed_by(w: &World, g: EntityId) -> Vec<&Hole> {
    w.holes.values().filter(|h| h.kind == HoleKind::Killed && h.faction == Some(g)).collect()
}

#[test]
fn test_source_with_no_statistical_members_makes_no_offscreen_hits() {
    let (mut w, g, _, _) = hands_world(0);
    fviolence::daily(&mut w);
    let n = killed_by(&w, g).len();
    assert_eq!(n, 0, "a gang all on screen kills nobody off screen ({n})");
    // The same city with the switch off: the victim side alone kills.
    let (mut off, g, _, _) = hands_world(0);
    off.config.fviolence.hands = None;
    fviolence::daily(&mut off);
    assert!(killed_by(&off, g).len() > 10, "the old mirror: {}", killed_by(&off, g).len());
}

#[test]
fn test_offscreen_hits_per_day_at_most_the_attacker_cap() {
    let (mut w, g, stat, _) = hands_world(4);
    // Rate 0.5 per member-day exactly (no evidence, the prior's weight
    // unbounded): 4 free hands x 0.5 = a budget of 2 kills.
    w.config.fviolence.hands = Some(citysim::config::FvHandsCfg { prior: [0.5, 0.0, 0.0, 0.0], prior_days: 1e9 });
    assert_eq!(fviolence::hand_rate(&w, (ViolenceSource::Order(Order::Contest), g))[0], 0.5);
    let all = fviolence::active_now(&w);
    let pools = fviolence::build_pools(&w, &all);
    let p = pools.get(&fviolence::PoolKey::Faction(ViolenceSource::Order(Order::Contest), g)).expect("the pool");
    assert_eq!(p.hands, stat, "the free Statistical members are the hands");
    assert_eq!(p.budget[0], 2, "4 x 0.5");
    fviolence::daily(&mut w);
    assert_eq!(killed_by(&w, g).len(), 2, "the attacker cap, not the victims' roll, sets the kills");
}

#[test]
fn test_offscreen_kill_is_bound_to_a_statistical_member_never_an_onscreen_one() {
    let (mut w, g, stat, bodies) = hands_world(3);
    w.config.fviolence.hands = Some(citysim::config::FvHandsCfg { prior: [1.0, 0.0, 0.0, 0.0], prior_days: 1e9 });
    w.config.bind.p_unknown = 0.0;
    fviolence::daily(&mut w);
    let holes: Vec<(u64, Option<EntityId>)> = killed_by(&w, g).iter().map(|h| (h.id, h.prebound)).collect();
    assert_eq!(holes.len(), 3, "one kill per hand");
    let mut hands: Vec<EntityId> = holes.iter().map(|&(_, h)| h.expect("pre-bound")).collect();
    hands.sort_unstable();
    assert_eq!(hands, stat, "each hand strikes once");
    for (id, hand) in holes {
        match bind::bind(&mut w, id).expect("open") {
            Bound::Actor(a) => {
                assert_eq!(Some(a), hand, "bound to its hand");
                assert!(!bodies.contains(&a), "never a member on screen");
            }
            Bound::Unknown => panic!("a living, free hand binds"),
        }
    }
    assert_eq!(w.stats.current.living.fv_bound_wrong, 0);
}

#[test]
fn test_riot_of_bodies_strikes_nobody_off_screen() {
    use citysim::{Riot, RiotResponse, RiotTarget};
    let build = |stat: usize| {
        let mut w = world(48);
        w.run_ticks(citysim::TICKS_PER_DAY + 12 * TICKS_PER_HOUR);
        let d = stat_district(&w);
        let rioters: Vec<EntityId> = civilians(&w)
            .into_iter()
            .filter(|&a| w.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical))
            .take(10)
            .collect();
        for &r in rioters.iter().take(stat) {
            lod::set_lod(&mut w, r, Lod::Statistical);
            w.comp_mut::<Brain>(r).expect("brain").body_day = None;
        }
        let jail = w.buildings_of_kind(citysim::BuildingKind::Jail)[0];
        w.riots.push(Riot {
            id: 93,
            district: d,
            target: jail,
            kind: RiotTarget::Precinct,
            muster: jail,
            muster_at: w.tick,
            rioters: rioters.clone(),
            response: RiotResponse::Contain,
            started: w.tick,
            departed: None,
        });
        for &r in &rioters {
            w.rioter_of.insert(r, 93);
        }
        w.config.fviolence.prior = Default::default();
        w.config.fviolence.prior.order = [0.0; 4];
        w.config.fviolence.prior.vendetta = [0.0; 4];
        w.config.fviolence.prior.episode = [0.0; 4];
        w.config.fviolence.prior.riot = [1.0, 0.0, 0.0, 0.0];
        w.config.fviolence.prior_weight = 1e6;
        w.config.fviolence.fv_mult = 1.0;
        w.config.fviolence.class_mult = citysim::config::FvClassCfg::default();
        w.config.fviolence.day_cap.killed = 10_000;
        w.config.life.violence_fixes = false;
        fviolence::daily(&mut w);
        let riot: Vec<Option<EntityId>> =
            w.holes.values().filter(|h| h.source == Some(ViolenceSource::Riot)).map(|h| h.prebound).collect();
        (riot, rioters)
    };
    let (riot, _) = build(0);
    assert!(riot.is_empty(), "a riot of bodies strikes only on screen ({})", riot.len());
    let (riot, rioters) = build(2);
    assert_eq!(riot.len(), 2, "two Statistical rioters, a hit each");
    assert!(riot.iter().all(|h| h.is_some_and(|a| rioters[..2].contains(&a))), "pre-bound to them");
}

#[test]
fn test_vendetta_norm_scales_with_the_city() {
    let c = Config::load();
    assert!(c.fviolence.hands.is_some());
    let n = c.world.population;
    let norm = c.grudges.vendetta_norm;
    assert_eq!(c.clone().scaled_to(n).grudges.vendetta_norm, norm, "the 2,000 city keeps its norm");
    let big = c.clone().scaled_to(n * 10).grudges.vendetta_norm;
    assert!((big - 10.0 * norm).abs() < 1e-3, "ten times the city, ten times the norm: {big} vs {norm}");
    assert_eq!(c.clone().scaled_to(n / 10).grudges.vendetta_norm, norm, "up only, as the other caps");
    let mut off = c.clone();
    off.fviolence.hands = None;
    assert_eq!(off.scaled_to(n * 10).grudges.vendetta_norm, norm, "absolute with the switch off");
}
