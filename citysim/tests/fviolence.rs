//! L2 phase 4 (plan 4.7, spec § 3): faction violence off screen. The
//! ledger's victim side counts on-screen victims of sourced violence per
//! `(source, district, victim class)`, the daily pass rolls the Statistical
//! tier against those rates, and the binder lays a faction's holes only to
//! its members (or nobody). Everything here is counters, seeded dice rolls
//! and hole records between fictional agents of a simulated city.

use citysim::ledger::{ActiveSource, VictimClass, ViolenceSource};
use citysim::systems::{bind, demography, fviolence, law, lod};
use citysim::{
    ActionInstance, ActionKind, Bound, Brain, Config, DeathCause, DistrictId, EntityId, Gang, GoalKind, Hole, HoleKind,
    Household, Lod, Order, Personality, Plan, PlayerCommand, Position, World, TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().scaled_to(300))
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&a| law::living(w, a) && demography::is_adult(w, a)).collect()
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

/// A free civilian adult (no gang, no guard's job), not `but`.
fn civilians(w: &World) -> Vec<EntityId> {
    adults(w)
        .into_iter()
        .filter(|&a| w.gang_of(a).is_none() && !law::is_guard(w, a) && !w.has::<citysim::Sentence>(a))
        .collect()
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
        let cfg = Config::load().scaled_to(300);
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
