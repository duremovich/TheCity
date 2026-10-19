//! M17 phase 4b (Dylan, 2026-10-13): a gang's succession. A decapitated
//! gang promotes from within; a close succession (the top two of
//! `leader_ranking` within `war_margin`) is a civil war: the members take
//! sides, the claimants hunt each other, a claimant killed or jailed loses,
//! and after `war_days` the sides' strength settles it on a keyed roll. The
//! loser dies, flees the city, or leads a splinter (D36). A syndicate's
//! brothers come only to a gang gutted by a sack (a rare roll) or by god.

mod common;

use citysim::components::{Brain, Building, Claim, GangMember, Personality, Skills};
use citysim::systems::{gang, succession};
use citysim::word::{GrudgeCause, Grudges};
use citysim::{Config, DeathCause, EntityId, EventKind, Gang, PlayerCommand, RelKind, World, TICKS_PER_DAY};

/// The seed-42 city on day 4.
fn city() -> World {
    // Built once per test process and cloned (the change tier's budget, 2026-10-19).
    static CITY: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    CITY.get_or_init(build_city).clone()
}

fn build_city() -> World {
    // Jobs & lives phase 1: the M17 city as written (the industries move
    // which gang is at war on day 4).
    let mut c = Config::load();
    c.jobs2.enabled = false;
    // Lives at work phase 6: nor the careers and lieutenants (their ranks and
    // seeded jobs move the Hollow's succession after a decapitation).
    c.strip_careers();
    c.gangs.strip_lieutenants();
    let mut w = World::new(42, c);
    w.config.crime.fight_death_p = 0.0;
    w.run_ticks(4 * TICKS_PER_DAY + 30);
    w
}

/// The gang with the most free members.
fn biggest(w: &World) -> EntityId {
    w.gangs().into_iter().max_by_key(|&g| (gang::fit_headcount(w, g), std::cmp::Reverse(g))).expect("a gang")
}

/// Every member joined now at loyalty 0.1, so the ranking reads the
/// loyalties set here: the old leader 1.0, the heir 0.9, the challenger
/// `0.9 - gap`. Returns `(old, heir, challenger)`.
fn stage(w: &mut World, g: EntityId, gap: f32) -> (EntityId, EntityId, EntityId) {
    let now = w.tick;
    // Grow the gang past `war_min_members` with free adults outside any gang.
    let want = w.config.outside.succession.war_min_members + 6;
    let have = w.comp::<Gang>(g).expect("gang").members.len();
    let recruits: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&id| {
            w.has::<Brain>(id)
                && w.gang_of(id).is_none()
                && citysim::systems::demography::is_adult(w, id)
                && !w.has::<citysim::components::Sentence>(id)
        })
        .take(want.saturating_sub(have))
        .collect();
    for r in recruits {
        gang::enlist(w, r, g);
    }
    let members: Vec<EntityId> =
        w.comp::<Gang>(g).expect("gang").members.iter().copied().filter(|&m| w.has::<Brain>(m)).collect();
    assert!(members.len() > w.config.outside.succession.war_min_members, "a big enough gang");
    for &m in &members {
        w.comp_mut::<GangMember>(m).expect("gm").joined_tick = now;
        w.comp_mut::<Personality>(m).expect("p").loyalty = 0.1;
    }
    let (old, heir, ch) = (members[0], members[1], members[2]);
    w.comp_mut::<Personality>(old).expect("p").loyalty = 1.0;
    w.comp_mut::<Personality>(heir).expect("p").loyalty = 0.9;
    w.comp_mut::<Personality>(ch).expect("p").loyalty = 0.9 - gap;
    gang::recompute_leader(w, g);
    assert_eq!(w.comp::<Gang>(g).expect("gang").leader, Some(old));
    (old, heir, ch)
}

/// Kill the leader and let `gang::run` take the decapitation.
fn decapitate(w: &mut World, g: EntityId, old: EntityId) {
    w.kill_by(old, DeathCause::Violence, None);
    assert_eq!(w.comp::<Gang>(g).expect("gang").split_check, Some(old));
    w.run_ticks(1);
}

fn grudge(w: &World, holder: EntityId, target: EntityId) -> f32 {
    w.comp::<Grudges>(holder)
        .and_then(|g| g.list.iter().find(|x| x.target == target && x.settled.is_none()).map(|x| x.weight))
        .unwrap_or(0.0)
}

fn count(w: &World, k: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == k).count()
}

/// A war staged and started: `(world, gang, heir, challenger)`.
fn at_war() -> (World, EntityId, EntityId, EntityId) {
    let mut w = city();
    let g = biggest(&w);
    let (old, heir, ch) = stage(&mut w, g, 0.001);
    decapitate(&mut w, g, old);
    assert!(w.comp::<Gang>(g).expect("gang").war.is_some(), "a war");
    (w, g, heir, ch)
}

/// Make the heir's side strong and the challenger's weak.
fn heir_strong(w: &mut World, g: EntityId) {
    let war = w.comp::<Gang>(g).expect("gang").war.clone().expect("war");
    for &m in &war.heir_side {
        w.comp_mut::<Skills>(m).expect("s").fighting = 1.0;
    }
    for &m in &war.challenger_side {
        w.comp_mut::<Skills>(m).expect("s").fighting = 0.0;
        w.comp_mut::<Personality>(m).expect("p").courage = 0.0;
    }
}

#[test]
fn test_clear_succession_promotes_from_within() {
    let mut w = city();
    let g = biggest(&w);
    let (old, heir, _) = stage(&mut w, g, 0.3);
    decapitate(&mut w, g, old);
    let x = w.comp::<Gang>(g).expect("gang");
    assert_eq!(x.leader, Some(heir), "the ranking's top takes over");
    assert!(x.war.is_none(), "no war on a clear succession");
    assert_eq!(count(&w, EventKind::CivilWar), 0);
    let ev = w.events.iter().find(|e| e.kind == EventKind::Succession).expect("a Succession event");
    assert_eq!(ev.actors.first(), Some(&heir));
    assert!(ev.actors.contains(&g) && ev.actors.contains(&old));
}

#[test]
fn test_close_succession_is_a_civil_war() {
    let (mut w, g, heir, ch) = at_war();
    let cfg = w.config.outside.succession.clone();
    let war = w.comp::<Gang>(g).expect("gang").war.clone().expect("war");
    assert_eq!((war.heir, war.challenger), (heir, ch));
    assert_eq!(war.heir_side.first(), Some(&heir));
    assert_eq!(war.challenger_side.first(), Some(&ch));
    assert!(!war.challenger_side.contains(&heir) && !war.heir_side.contains(&ch));
    let free = gang::fit_headcount(&w, g);
    assert_eq!(war.heir_side.len() + war.challenger_side.len(), free, "every free member took a side");
    assert_eq!(war.until, war.since + u64::from(cfg.war_days) * TICKS_PER_DAY);
    assert_eq!(count(&w, EventKind::CivilWar), 1);
    assert_eq!(count(&w, EventKind::TookSides), free - 2, "a TookSides per member, for the diaries");
    assert_eq!(count(&w, EventKind::Succession), 0, "a war, not a promotion");
    // The feud: each claimant holds a lethal grudge on the other, and they are Enemies.
    assert!(grudge(&w, heir, ch) >= w.config.hunt.lethal_min);
    assert!(grudge(&w, ch, heir) >= w.config.hunt.lethal_min);
    assert!(w
        .comp::<Grudges>(ch)
        .is_some_and(|x| x.list.iter().any(|y| y.target == heir && y.cause == GrudgeCause::Succession)));
    assert!(w.edge(heir, ch).is_some_and(|e| e.kind == RelKind::Enemy));
    // The heir holds the chair while the war runs, whatever the ranking says.
    w.comp_mut::<Personality>(ch).expect("p").loyalty = 1.0;
    gang::recompute_leader(&mut w, g);
    assert_eq!(w.comp::<Gang>(g).expect("gang").leader, Some(heir));
}

#[test]
fn test_sides_follow_loyalty_edges_and_regard() {
    let (mut w, g, heir, ch) = at_war();
    let war = w.comp::<Gang>(g).expect("gang").war.clone().expect("war");
    let others: Vec<EntityId> =
        war.heir_side.iter().chain(war.challenger_side.iter()).copied().filter(|&m| m != heir && m != ch).collect();
    let (friend, loyal) = (others[0], others[1]);
    for m in [friend, loyal] {
        for c in [heir, ch] {
            let e = w.edge_entry(m, c);
            e.affinity = 0.0;
            e.kind = RelKind::Acquaintance;
        }
    }
    // A Friend of the challenger backs the challenger; a loyal member with
    // no tie backs the heir (the order of succession).
    let e = w.edge_entry(friend, ch);
    e.kind = RelKind::Friend;
    e.affinity = 0.9;
    w.comp_mut::<Personality>(friend).expect("p").loyalty = 0.1;
    w.comp_mut::<Personality>(loyal).expect("p").loyalty = 1.0;
    let (hs, cs) = succession::sides(&w, g, heir, ch);
    assert!(cs.contains(&friend) && !hs.contains(&friend));
    assert!(hs.contains(&loyal) && !cs.contains(&loyal));
    assert_eq!(hs.first(), Some(&heir));
    assert_eq!(cs.first(), Some(&ch));
}

#[test]
fn test_a_claimant_killed_loses_the_war() {
    // The challenger falls: the heir is crowned.
    let (mut w, g, heir, ch) = at_war();
    w.kill_by(ch, DeathCause::Violence, Some(heir));
    let x = w.comp::<Gang>(g).expect("gang");
    assert!(x.war.is_none(), "the war ended");
    assert_eq!((x.leader, x.crowned), (Some(heir), Some(heir)));
    assert_eq!(x.split_check, None, "no new succession");
    let ev = w.events.iter().find(|e| e.kind == EventKind::CivilWarEnded).expect("CivilWarEnded");
    assert_eq!(ev.actors.first(), Some(&heir));
    assert!(ev.text.contains("dead"), "{}", ev.text);
    w.run_ticks(2);
    assert_eq!(count(&w, EventKind::Succession) + count(&w, EventKind::CivilWar), 1, "the one war, nothing new");
    // The heir falls: the challenger takes the chair.
    let (mut w, g, heir, ch) = at_war();
    w.kill_by(heir, DeathCause::Violence, Some(ch));
    let x = w.comp::<Gang>(g).expect("gang");
    assert!(x.war.is_none());
    assert_eq!(x.leader, Some(ch), "the challenger takes the chair");
    assert_eq!(x.split_check, None);
}

#[test]
fn test_a_jailed_claimant_loses_the_war() {
    let (mut w, g, heir, ch) = at_war();
    let jail = w.building_of_kind(citysim::BuildingKind::Jail).expect("a Jail");
    let until = w.tick + 10 * TICKS_PER_DAY;
    citysim::systems::law::sentence(&mut w, ch, citysim::Crime::Assault, until, jail);
    let x = w.comp::<Gang>(g).expect("gang");
    assert!(x.war.is_none());
    assert_eq!(x.leader, Some(heir));
    assert!(w.events.iter().any(|e| e.kind == EventKind::CivilWarEnded && e.text.contains("cell")));
    // The jailed heir lost the chair; the gang does not hold it as its boss.
    let (mut w, g, heir, ch) = at_war();
    let jail = w.building_of_kind(citysim::BuildingKind::Jail).expect("a Jail");
    let until = w.tick + 10 * TICKS_PER_DAY;
    citysim::systems::law::sentence(&mut w, heir, citysim::Crime::Assault, until, jail);
    let x = w.comp::<Gang>(g).expect("gang");
    assert!(x.war.is_none());
    assert_eq!(x.leader, Some(ch), "the challenger took the chair");
    assert_ne!(x.boss, Some(heir), "the loser is nobody's boss");
}

/// Past `war_days` the stronger side wins (a keyed roll on its share of
/// the strength); a loser without a splinter to lead flees the city.
#[test]
fn test_war_settles_by_strength_and_the_loser_flees() {
    let (mut w, g, heir, ch) = at_war();
    heir_strong(&mut w, g);
    // No held Homes: no splinter can stand.
    w.comp_mut::<Gang>(g).expect("gang").territory.clear();
    let mut twin = w.clone();
    w.comp_mut::<Gang>(g).expect("gang").war.as_mut().expect("war").until = w.tick;
    succession::daily(&mut w, g);
    let x = w.comp::<Gang>(g).expect("gang");
    assert!(x.war.is_none());
    assert_eq!(x.leader, Some(heir), "the strong side's claimant wins");
    assert_ne!(w.gang_of(ch), Some(g), "the loser is out");
    assert!(w.comp::<Brain>(ch).is_some_and(|b| b.emigrating), "and flees the city");
    assert!(w.events.iter().any(|e| e.kind == EventKind::CivilWarEnded && e.text.contains("fled")));
    // Before `until` nothing settles; the roll is keyed (a twin settles the same).
    succession::daily(&mut twin, g);
    assert!(twin.comp::<Gang>(g).expect("gang").war.is_some(), "not before war_days");
    twin.comp_mut::<Gang>(g).expect("gang").war.as_mut().expect("war").until = twin.tick;
    succession::daily(&mut twin, g);
    assert_eq!(twin.comp::<Gang>(g).expect("gang").leader, Some(heir));
}

/// A loser with a following and a second held district leads a splinter.
#[test]
fn test_loser_with_a_following_leads_a_splinter() {
    let (mut w, g, heir, ch) = at_war();
    w.config.gangs.max_gangs = 99;
    heir_strong(&mut w, g);
    // The challenger's side: at least two followers.
    let war = w.comp::<Gang>(g).expect("gang").war.clone().expect("war");
    let movers: Vec<EntityId> = war.heir_side.iter().copied().filter(|&m| m != heir).take(2).collect();
    {
        let x = w.comp_mut::<Gang>(g).expect("gang").war.as_mut().expect("war");
        x.heir_side.retain(|m| !movers.contains(m));
        x.challenger_side.extend(&movers);
    }
    for &m in &movers {
        w.comp_mut::<Skills>(m).expect("s").fighting = 0.0;
    }
    // Homes held in the heir's district and in a district with a vacant Lot.
    let heir_home = w.comp::<citysim::Household>(heir).and_then(|h| h.home).expect("the heir's home");
    let hd = w.district_of_building(heir_home);
    let lot = citysim::systems::founding::vacant_lots(&w)
        .into_iter()
        .find(|&l| {
            let d = w.district_of_building(l);
            d != hd && w.districts[d.index()].homes.len() >= 3
        })
        .expect("a Lot in another district");
    let d = w.district_of_building(lot);
    let mut homes: Vec<EntityId> = w.districts[hd.index()].homes.iter().copied().take(2).collect();
    homes.extend(w.districts[d.index()].homes.iter().copied().take(3));
    for &h in &homes {
        w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    }
    homes.sort_unstable();
    homes.dedup();
    w.comp_mut::<Gang>(g).expect("gang").territory = homes;
    w.comp_mut::<Gang>(g).expect("gang").war.as_mut().expect("war").until = w.tick;
    succession::daily(&mut w, g);
    let x = w.comp::<Gang>(g).expect("gang");
    assert!(x.war.is_none());
    assert_eq!(x.leader, Some(heir));
    let splinter = w.gang_of(ch).expect("the loser leads a gang");
    assert_ne!(splinter, g);
    let sg = w.comp::<Gang>(splinter).expect("splinter");
    assert_eq!(sg.leader, Some(ch));
    assert_eq!(sg.split_from, Some(g));
    assert!(movers.iter().all(|&m| w.gang_of(m) == Some(splinter)), "the following went with it");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Split));
    assert!(w.events.iter().any(|e| e.kind == EventKind::CivilWarEnded && e.text.contains("splinter")));
}

/// With `[outside.succession]` off a decapitation is M12's: no event, no war.
#[test]
fn test_succession_off_is_the_old_promotion() {
    let mut w = city();
    w.config.outside.succession.enabled = false;
    let g = biggest(&w);
    let (old, heir, _) = stage(&mut w, g, 0.001);
    decapitate(&mut w, g, old);
    let x = w.comp::<Gang>(g).expect("gang");
    assert_eq!(x.leader, Some(heir));
    assert!(x.war.is_none());
    assert_eq!(count(&w, EventKind::Succession) + count(&w, EventKind::CivilWar), 0);
}

// ---------------------------------------------------------------------------
// The brothers: a rare trigger only.
// ---------------------------------------------------------------------------

fn hollow(w: &World) -> EntityId {
    let f = w.outside.factions.iter().find(|f| f.name == "Hollow Brothers").expect("Hollow Brothers");
    f.parent.as_ref().expect("p").branches[0]
}

/// `city()` run on until The Hollow has a leader and `brothers_gutted_below`
/// members (Shadow V4 batch 1: on the moved city its roster was empty on
/// day 4; the brothers' tests need a gang to lose).
fn hollow_city() -> World {
    // Built once per test process and cloned (the change tier's budget, 2026-10-19).
    static HOLLOW: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    HOLLOW.get_or_init(build_hollow_city).clone()
}

fn build_hollow_city() -> World {
    let mut w = city();
    let g = hollow(&w);
    let want = w.config.outside.parents.brothers_gutted_below;
    for _ in 0..30 {
        if w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= want) {
            break;
        }
        w.run_ticks(TICKS_PER_DAY);
    }
    w
}

/// BrothersArrived since `t` (`hollow_city`'s run-on may have had some).
fn brothers_since(w: &World, t: u64) -> usize {
    w.events.iter().filter(|e| e.kind == EventKind::BrothersArrived && e.tick >= t).count()
}

fn cool_hollow(w: &mut World) {
    for f in w.outside.factions.iter_mut().filter(|f| f.name == "Hollow Brothers") {
        f.parent.as_mut().expect("p").last_brothers = None;
    }
}

#[test]
fn test_a_decapitation_sends_no_brothers() {
    let mut w = hollow_city();
    let g = hollow(&w);
    cool_hollow(&mut w);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
    let t0 = w.tick;
    w.kill_by(leader, DeathCause::Violence, None);
    let delay = u64::from(w.config.outside.parents.brother_delay_days);
    w.run_ticks((delay + 2) * TICKS_PER_DAY);
    assert_eq!(brothers_since(&w, t0), 0, "the gang settles its own succession");
}

/// A sack that leaves a syndicate's gang under `brothers_gutted_below`
/// free members sends brothers on the roll; the roll off, or a gang with
/// its people, gets none.
#[test]
fn test_brothers_after_a_gutting_sack() {
    let sacked = |w: &mut World, g: EntityId| {
        let until = w.tick + w.config.gangs.sacked_days * TICKS_PER_DAY;
        w.comp_mut::<Gang>(g).expect("gang").sacked_until = Some(until);
    };
    let mut w = hollow_city();
    let delay = u64::from(w.config.outside.parents.brother_delay_days);
    w.config.outside.parents.brothers_p = 1.0;
    let g = hollow(&w);
    cool_hollow(&mut w);
    let members = w.comp::<Gang>(g).expect("gang").members.clone();
    let keep = w.config.outside.parents.brothers_gutted_below - 2;
    for &m in members.iter().skip(keep) {
        gang::leave(&mut w, m, "test");
    }
    assert!(gang::fit_headcount(&w, g) < w.config.outside.parents.brothers_gutted_below);
    let mut big = w.clone();
    let mut off = w.clone();
    let t0 = w.tick;
    // Gutted, the roll certain; the roll off; not gutted. The three cities
    // side by side (the change tier's budget, 2026-10-19).
    off.config.outside.parents.brothers_p = 0.0;
    big.config.outside.parents.brothers_gutted_below = 1;
    let [gutted, rolled_off, whole] = std::thread::scope(|s| {
        [w, off, big]
            .map(|mut x| {
                s.spawn(move || {
                    sacked(&mut x, g);
                    x.run_ticks((delay + 2) * TICKS_PER_DAY);
                    brothers_since(&x, t0)
                })
            })
            .map(|h| h.join().expect("a sacked city"))
    });
    assert!(gutted >= 1, "brothers for the gutted gang");
    assert_eq!(rolled_off, 0, "the roll said no");
    assert_eq!(whole, 0, "a gang with its people needs no brothers");
}

#[test]
fn test_god_send_brothers() {
    let mut w = city();
    let g = hollow(&w);
    w.push_command(PlayerCommand::SendBrothers(g));
    w.run_ticks(TICKS_PER_DAY + 2);
    assert!(count(&w, EventKind::BrothersArrived) >= 1, "brothers by the next midnight");
    // A gang without a syndicate: refused.
    if let Some(o) = w.gangs().into_iter().find(|&x| !citysim::systems::parents::has_syndicate(&w, x)) {
        assert!(citysim::systems::parents::send_brothers(&mut w, o).is_err());
    }
}

/// A war in flight saves and loads, and runs on byte for byte.
#[test]
fn test_save_round_trip_with_a_war() {
    let (mut w, g, _, _) = at_war();
    let mut back = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("loads");
    assert_eq!(back.comp::<Gang>(g).expect("gang").war, w.comp::<Gang>(g).expect("gang").war);
    w.run_ticks(TICKS_PER_DAY);
    back.run_ticks(TICKS_PER_DAY);
    assert_eq!(citysim::save::to_ron(&w), citysim::save::to_ron(&back));
}

/// A jailed leader leaves an acting leader, never a war (the boss walks
/// back in on release), however close the runners-up stand.
#[test]
fn test_a_jailed_leader_leaves_an_acting_leader() {
    let mut w = city();
    let g = biggest(&w);
    let (old, heir, _) = stage(&mut w, g, 0.001);
    let jail = w.building_of_kind(citysim::BuildingKind::Jail).expect("a Jail");
    let until = w.tick + 10 * TICKS_PER_DAY;
    citysim::systems::law::sentence(&mut w, old, citysim::Crime::Assault, until, jail);
    w.run_ticks(1);
    let x = w.comp::<Gang>(g).expect("gang");
    assert_eq!(x.leader, Some(heir));
    assert!(x.war.is_none(), "no war over a jailed leader's chair");
    let ev = w.events.iter().find(|e| e.kind == EventKind::Succession).expect("a Succession event");
    assert!(ev.text.contains("sits inside"), "{}", ev.text);
}

/// Review 2 (Shadow V4 bug 8's tenure ranking): the runner-up is read from
/// the heir's own tenure class. A newcomer of higher loyalty ranks below a
/// veteran heir; read as the runner-up it stood "ahead by -0.2" within
/// `war_margin` and started a civil war against the untenured.
#[test]
fn test_newcomer_is_no_runner_up_to_a_veteran_heir() {
    let mut w = city();
    let g = biggest(&w);
    let (old, heir, ch) = stage(&mut w, g, 0.3);
    // Tenure: the old leader and the heir joined on day 0, the rest now.
    w.config.gangs.leader_min_days = 3;
    for m in [old, heir] {
        w.comp_mut::<GangMember>(m).expect("gm").joined_tick = 0;
    }
    w.comp_mut::<Personality>(heir).expect("p").loyalty = 0.5;
    w.comp_mut::<Personality>(ch).expect("p").loyalty = 1.0;
    gang::recompute_leader(&mut w, g);
    assert_eq!(w.comp::<Gang>(g).expect("gang").leader, Some(old));
    let ranking = gang::leader_ranking(&w, g);
    assert_eq!(&ranking[..2], &[old, heir], "the veterans first");
    decapitate(&mut w, g, old);
    let x = w.comp::<Gang>(g).expect("gang");
    assert_eq!(x.leader, Some(heir), "the veteran takes over");
    assert!(x.war.is_none(), "no war against an untenured newcomer");
    let ev = w.events.iter().rev().find(|e| e.kind == EventKind::Succession).expect("a Succession event");
    assert!(!ev.text.contains("by -"), "no negative lead: {}", ev.text);
}

// ---------------------------------------------------------------------------
// Shadow V4 bug 8 (docs/SHADOW_V4.md): the lead by tenure, no succession for
// a night; M17 review 4: the crown through a night in the cells.
// ---------------------------------------------------------------------------

/// Bug 8: a gang's lead goes to a member of `leader_min_days` before a
/// newer, more loyal one; a night's term (a fare) hands nobody the gang.
#[test]
fn test_bug8_leader_by_tenure_and_no_succession_for_a_night() {
    // Lives at work phase 2: on the household's day-12 city no gang of four
    // has a leader and two free members; this pins the city the pick was
    // written for (phase 5's tower moves it again: the beat's time gate
    // left no gang of four with a leader on day 12; pinned off likewise).
    let mut w = common::world_with(|c| {
        c.household.enabled = false;
        c.strip_tower();
    });
    // The seed's gangs start empty and recruit through the first days.
    w.run_ticks(12 * TICKS_PER_DAY);
    let g = w
        .gangs()
        .into_iter()
        .find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= 4))
        .expect("a gang with a leader");
    // Two free members besides the leader (the ranking's own pool; Review:
    // on the moved city members[0] was the leader).
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader);
    let members: Vec<EntityId> = w
        .comp::<Gang>(g)
        .expect("g")
        .members
        .iter()
        .copied()
        .filter(|&m| Some(m) != leader && !w.has::<citysim::Sentence>(m))
        .collect();
    let (vet, new) = (members[0], members[1]);
    w.tick += 40 * TICKS_PER_DAY;
    let now = w.tick;
    w.comp_mut::<GangMember>(vet).expect("gm").joined_tick = now - 30 * TICKS_PER_DAY;
    w.comp_mut::<GangMember>(new).expect("gm").joined_tick = now - 2 * TICKS_PER_DAY;
    w.comp_mut::<Personality>(vet).expect("p").loyalty = 0.1;
    w.comp_mut::<Personality>(new).expect("p").loyalty = 1.0;
    let scores = gang::leader_scores(&w, g);
    let rank = |a: EntityId| scores.iter().position(|x| x.0 == a).expect("ranked");
    assert!(rank(vet) < rank(new), "a month in outranks two days of loyalty");
    // The leader jailed for a fare's night: no Succession.
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("leader");
    w.insert(leader, citysim::Sentence { until_tick: now + 600, crime: citysim::Crime::FareEvasion });
    let before = w.events.iter().filter(|e| e.kind == EventKind::Succession).count();
    assert!(!succession::decide(&mut w, g, leader));
    let after = w.events.iter().filter(|e| e.kind == EventKind::Succession).count();
    assert_eq!(before, after, "a night in the cells is no interregnum");
}

/// M17 review 4: the crowned keeps the crown through a night in the cells.
#[test]
fn test_the_crown_outlasts_a_night_in_the_cells() {
    let mut w = common::world_at(4 * TICKS_PER_DAY + 30);
    let g = w.gangs().into_iter().max_by_key(|&g| gang::fit_headcount(&w, g)).expect("a gang");
    for a in common::free_crew(&mut w, 4, 0.5) {
        gang::enlist(&mut w, a, g);
    }
    let ranking = gang::leader_ranking(&w, g);
    let (king, rival) = (ranking[1], ranking[0]);
    w.comp_mut::<Gang>(g).expect("gang").crowned = Some(king);
    gang::recompute_leader(&mut w, g);
    assert_eq!(w.comp::<Gang>(g).and_then(|x| x.leader), Some(king), "the crown over the ranking");
    let jail = w.building_of_kind(citysim::BuildingKind::Jail).expect("a Jail");
    let until = w.tick + 600;
    citysim::systems::law::sentence(&mut w, king, citysim::Crime::Vagrancy, until, jail);
    assert_eq!(w.comp::<Gang>(g).and_then(|x| x.crowned), Some(king), "still crowned inside");
    w.remove::<citysim::Sentence>(king);
    gang::on_member_released(&mut w, king);
    assert_eq!(w.comp::<Gang>(g).and_then(|x| x.leader), Some(king), "back in the chair, not {rival:?}");
}
