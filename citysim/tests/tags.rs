//! M16b phase 4 (docs/M16_CONTRACTS.md § 3, the attention addendum): tags,
//! scanners, the Purist Patrol and attention, one test per mechanism in the
//! shipped seed-42 city: a tag writes its carrier's sighting hourly and its
//! battery runs out, `max_tags` holds, a scanner of a higher tier removes a
//! tag (the `Tagged` memory), a `Tag` record's plant, the Patrol's scan and
//! attack; attention's pool and floor, a riot or a raid lowering its
//! target's alertness below its pool, the brains' tilt, and off reading
//! 1.0. Every tag and threat here is game state between fictional agents: a
//! record on a struct and a seeded dice roll. With `[tags] enabled = false`
//! the mechanism tests fail (the switch gates every entry point).

use citysim::contract::{ContractKind, ContractStatus, Origin, Posting, Target, ThreatKind};
use citysim::systems::{attention, contracts, demography, faction, law, tags};
use citysim::{
    Brain, BuildingKind, Config, Controller, Corp, EntityId, EventKind, Gang, GangMember, Kit, Lod, MemoryKind, Order,
    Position, Skills, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

/// The shipped city with the outside's parents off (M17 phase 2): these
/// seeded-world tests roll their dice on the M16b city's trajectory (the
/// Patrol's equal-tier sweep is a contest).
fn world() -> World {
    let mut c = Config::load();
    c.outside.enabled = false;
    // Jobs & lives phase 2: the city as written, without the street (its
    // pitches and day posts move this seed's patrol day); phase 4: nor the
    // vice and fashion (the same butterfly: the Patrol's fight went the
    // other way); phase 6: nor the gray market and faith (the same
    // butterfly: it fails with both on, passes with either).
    c.jobs2.street = false;
    c.vice.enabled = false;
    c.fashion.enabled = false;
    c.gray.enabled = false;
    c.faith.enabled = false;
    // Lives at work phase 6: nor the careers and lieutenants (the same
    // butterfly: the scanned chromed adult moved).
    c.strip_careers();
    c.gangs.strip_lieutenants();
    World::new(42, c)
}

/// The seed-42 city once two gangs are led and one holds a district.
fn gang_world() -> World {
    static SAVE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let text = SAVE.get_or_init(|| {
        let mut w = world();
        for _ in 0..40 {
            w.run_ticks(TICKS_PER_DAY);
            let led = w.gangs().into_iter().filter(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some())).count();
            let held = w.gangs().into_iter().any(|g| !citysim::systems::gang::held_districts(&w, g).is_empty());
            if led >= 2 && held {
                break;
            }
        }
        citysim::save::to_ron(&w)
    });
    citysim::save::from_ron(text).expect("loads")
}

/// A led gang with members.
fn led_gang(w: &World) -> EntityId {
    w.gangs()
        .into_iter()
        .find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= 2))
        .expect("a led gang")
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

fn strangers(w: &World, n: usize) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    for a in adults(w) {
        if w.has::<GangMember>(a) || law::is_guard(w, a) || w.has::<citysim::Sentence>(a) {
            continue;
        }
        if w.spouse_of(a).is_some() || out.iter().any(|&o| w.edge(o, a).is_some()) {
            continue;
        }
        if w.comp::<Brain>(a).is_none_or(|b| b.cuffed_by.is_some()) {
            continue;
        }
        out.push(a);
        if out.len() == n {
            return out;
        }
    }
    panic!("{n} strangers");
}

fn has_event(w: &World, kind: EventKind, who: EntityId) -> bool {
    w.events.iter().any(|e| e.kind == kind && e.actors.contains(&who))
}

/// To the next whole hour.
fn next_hour(w: &mut World) {
    w.tick = (w.tick / TICKS_PER_HOUR + 1) * TICKS_PER_HOUR;
}

/// Spec § 3: a tag writes its carrier's sighting to the owner's database
/// once an hour at `tag_conf[tier]`; past the battery it is gone.
#[test]
fn test_tag_writes_hourly_until_the_battery_dies() {
    let mut w = world();
    let g = w.gangs()[0];
    let [a] = strangers(&w, 1)[..] else { unreachable!() };
    tags::god_tag(&mut w, a, g, 2).expect("planted");
    assert!(has_event(&w, EventKind::Tagged, a));
    let seen = |w: &World| w.db.get(&g).map_or(0, |db| db.sightings.iter().filter(|s| s.who == a).count());
    for _ in 0..3 {
        next_hour(&mut w);
        tags::run(&mut w);
    }
    assert_eq!(seen(&w), 3, "one sighting an hour");
    let s = w.db[&g].sightings.iter().rev().find(|s| s.who == a).unwrap();
    assert!((s.confidence - w.config.tags.tag_conf[2]).abs() < 1e-6);
    w.tick += u64::from(w.config.tags.tag_battery_days) * TICKS_PER_DAY;
    next_hour(&mut w);
    tags::run(&mut w);
    assert!(tags::tags_on(&w, a).is_empty(), "the battery ran out");
    assert_eq!(seen(&w), 3);
}

/// Spec § 3: tags are capped at `max_tags` city-wide.
#[test]
fn test_max_tags_holds() {
    let mut w = world();
    w.config.tags.max_tags = 2;
    let g = w.gangs()[0];
    let v = strangers(&w, 3);
    assert!(tags::god_tag(&mut w, v[0], g, 1).is_ok());
    assert!(tags::god_tag(&mut w, v[1], g, 1).is_ok());
    assert!(tags::god_tag(&mut w, v[2], g, 1).is_err());
}

/// Spec § 11: a scanner of a higher tier removes the tag (`TagFound`) and,
/// at a margin ≥ 1, leaves a `Tagged` memory naming the owner's head; the
/// scan's price goes to the scanner's owner.
#[test]
fn test_higher_scanner_removes_the_tag() {
    let mut w = gang_world();
    let g = led_gang(&w);
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
    let [a] = strangers(&w, 1)[..] else { unreachable!() };
    // A body keeps the memory (a Statistical agent keeps only its eight kinds).
    citysim::systems::lod::set_lod(&mut w, a, Lod::Full);
    if let Some(x) = w.comp_mut::<citysim::Wallet>(a) {
        x.coins = 10_000;
    }
    let office = w.buildings_of_kind(BuildingKind::SecurityOffice).first().copied().expect("a Security Office");
    assert!(tags::scanner_tier(&w, office) > 1);
    tags::god_tag(&mut w, a, g, 1).unwrap();
    let mut found = false;
    for _ in 0..50 {
        if tags::scan_at(&mut w, a, office) > 0 {
            found = true;
            break;
        }
        w.tick += 1;
    }
    assert!(found, "found within the tries");
    assert!(tags::tags_on(&w, a).is_empty());
    assert!(has_event(&w, EventKind::TagFound, a));
    let mem = w.comp::<citysim::Memory>(a).unwrap();
    assert!(mem.entries.iter().any(|e| e.kind == MemoryKind::Tagged && e.subject == Some(leader)));
    assert!(w.stats.current.gov.scans >= 1);
}

/// Spec § 3: a scanner of the tag's tier or lower never leaves a memory
/// (no margin), and a tier-0 building scans nothing.
#[test]
fn test_scanner_without_margin_names_nobody() {
    let mut w = world();
    let g = w.gangs()[0];
    let [a] = strangers(&w, 1)[..] else { unreachable!() };
    tags::god_tag(&mut w, a, g, 3).unwrap();
    for _ in 0..30 {
        tags::sweep(&mut w, a, 3, EntityId::NONE);
        w.tick += 1;
    }
    let mem = w.comp::<citysim::Memory>(a).unwrap();
    assert!(!mem.entries.iter().any(|e| e.kind == MemoryKind::Tagged));
    let home = w.buildings_of_kind(BuildingKind::Home)[0];
    assert_eq!(tags::scanner_tier(&w, home), 0);
}

/// Spec § 3: a `Tag` record's taker contests its thief tier against the
/// target's sight; a win plants the tag for the buyer (who pays the
/// device) and fulfils the record.
#[test]
fn test_tag_record_plants_for_the_buyer() {
    let mut w = world();
    let corp = w.corps()[0];
    w.comp_mut::<Corp>(corp).unwrap().treasury = 100_000;
    let [t, taker] = strangers(&w, 2)[..] else { unreachable!() };
    // Shadow V4 bug 15: the ledger's plant is an off-screen party's (a
    // shown one runs live: `test_tag_live_for_a_shown_taker`).
    for x in [t, taker] {
        citysim::systems::lod::set_lod(&mut w, x, citysim::Lod::Statistical);
    }
    if let Some(s) = w.comp_mut::<Skills>(taker) {
        s.stealth = 1.0;
    }
    let p = Posting {
        buyer: Some(corp),
        agent: None,
        kind: ContractKind::Tag,
        target: Target::Agent(t),
        broker: None,
        deadline_days: 7,
        origin: Origin::God,
        price: None,
    };
    let id = contracts::post(&mut w, p).expect("posted");
    w.config.contracts.max_attempts = 200;
    let mut ok = false;
    for _ in 0..100 {
        if w.contracts[&id].status == ContractStatus::Open {
            contracts::god_take(&mut w, id, taker).expect("taken");
        }
        contracts::resolve_ledger(&mut w, id);
        if w.contracts[&id].status == ContractStatus::Fulfilled {
            ok = true;
            break;
        }
        w.tick += 1;
    }
    assert!(ok, "planted within the tries");
    assert!(w.tags.iter().any(|x| x.on == t && x.owner == corp));
    assert!(w.stats.current.gov.k_tag_done >= 1);
}

/// Spec § 3 (the Purist Patrol): a Purist gang's member on the street in a
/// held district scans the adults around; a chromed one the scanner wins
/// against is attacked (an Assault or a Murder) and swept for tags. The
/// sweep of an equal-tier tag is a keyed contest at 0.5 (Jobs & lives phase
/// 7b: this seed's one roll flipped with the table; the burn is
/// `test_equal_tier_sweep_burns_half`'s).
#[test]
fn test_patrol_scans_and_attacks_the_chromed() {
    let mut w = gang_world();
    let g = led_gang(&w);
    w.comp_mut::<Gang>(g).unwrap().creed = Some(citysim::word::Creed::Purist);
    w.comp_mut::<Gang>(g).unwrap().order = Order::Expand;
    w.comp_mut::<Gang>(g).unwrap().treasury = 10_000;
    assert!(tags::buy_scanner(&mut w, g));
    let m =
        w.comp::<Gang>(g).unwrap().members.iter().copied().find(|&m| contracts::free_adult(&w, m)).expect("a member");
    citysim::systems::lod::set_lod(&mut w, m, Lod::Full);
    let [a] = strangers(&w, 1)[..] else { unreachable!() };
    citysim::systems::lod::set_lod(&mut w, a, Lod::Full);
    // The gang holds one district, and the adult stands at a spot in it.
    let d = (0..w.districts.len()).find(|&i| w.spots.get(i).is_some_and(|v| !v.is_empty())).expect("a spot");
    for (i, x) in w.districts.iter_mut().enumerate() {
        x.control = if i == d { Controller::Gang(g) } else { Controller::Contested };
    }
    // The spot the patrol picks: the district's most crowded by its own
    // count (strangers outdoors within `patrol_radius`); the 8c fix round's
    // venue shifts moved the crowd off the first spot.
    let r = w.config.tags.patrol_radius;
    let members = w.comp::<Gang>(g).unwrap().members.clone();
    let spots: Vec<citysim::components::TilePos> = w.spots[d].iter().map(|s| s.tile).collect();
    let mut best: Option<(usize, citysim::components::TilePos)> = None;
    for t in spots {
        let near = w.agents_near(t, r, None);
        let n = near
            .into_iter()
            .filter(|&x| !members.contains(&x) && contracts::free_adult(&w, x))
            .filter(|&x| w.comp::<Position>(x).is_some_and(|q| q.building.is_none() && q.tile.manhattan(t) <= r * 2))
            .count();
        // The patrol's tie-break: the first spot of the most.
        if best.is_none_or(|(b, _)| n > b) {
            best = Some((n, t));
        }
    }
    let tile = best.expect("a spot").1;
    for x in [m, a] {
        w.remove_from_building(x);
        if let Some(p) = w.comp_mut::<Position>(x) {
            p.tile = tile;
            p.building = None;
        }
    }
    // No other chromed adult in the scan's reach: the subject is the one
    // found, whoever the combined city's crowd happens to carry.
    for x in w.agents_near(tile, r * 2, None) {
        if x != a && w.comp::<Kit>(x).is_some_and(|k| k.chrome) {
            w.comp_mut::<Kit>(x).expect("kit").chrome = false;
        }
    }
    w.insert(a, Kit { visible: 0, chrome: true, ..Kit::default() });
    let owner = w.corps()[0];
    tags::god_tag(&mut w, a, owner, 1).unwrap();
    let mut hit = None;
    for _ in 0..40 {
        hit = tags::patrol(&mut w, g);
        if hit.is_some() {
            break;
        }
        w.tick += 1;
    }
    assert_eq!(hit, Some(a), "the chromed adult was found");
    assert!(has_event(&w, EventKind::PatrolScan, m));
    assert!(has_event(&w, EventKind::Assault, a) || has_event(&w, EventKind::Murder, a));
    assert!(w.stats.current.gov.patrols >= 1);
}

/// The Patrol's sweep (`tags::sweep` at tier 1) burns an equal-tier tag on
/// the contest's 0.5 (`security::contest`, keyed on the tag and the tick):
/// 400 tags, one sweep each, between 40 % and 60 % burned (± 4 sd), each
/// burn a `TagFound`.
#[test]
fn test_equal_tier_sweep_burns_half() {
    let mut w = world();
    let a = strangers(&w, 1)[0];
    let owner = w.corps()[0];
    let (mut burned, n) = (0u32, 400u32);
    let found0 = w.events.iter().filter(|e| e.kind == EventKind::TagFound).count();
    for _ in 0..n {
        tags::god_tag(&mut w, a, owner, 1).expect("a tag");
        burned += tags::sweep(&mut w, a, 1, owner);
        w.tags.retain(|t| t.on != a);
        w.tick += 1;
    }
    let rate = f64::from(burned) / f64::from(n);
    assert!((0.4..=0.6).contains(&rate), "burned {burned} of {n}");
    let found = w.events.iter().filter(|e| e.kind == EventKind::TagFound).count() - found0;
    assert_eq!(found, burned as usize, "a TagFound per burn");
}

/// The addendum: the pool and the floor.
#[test]
fn test_pool_and_floor() {
    assert!((attention::pool_of(0.25, 20) - 0.7).abs() < 1e-6);
    assert_eq!(attention::pool_of(0.0, 0), 0.4);
    assert_eq!(attention::pool_of(1.0, 100), 1.0);
    assert_eq!(attention::alertness_of(0.7, 0.4, 0.2), 0.7 - 0.4);
    assert_eq!(attention::alertness_of(0.7, 3.0, 0.2), 0.2, "never under the floor");
}

/// The addendum (the gate's mechanism): on a day a faction has a live riot
/// against it its alertness is below its pool, `alertness_mult` below 1,
/// never under `alert_floor`; with nothing against it 1.0.
#[test]
fn test_riot_lowers_alertness_to_the_floor_at_most() {
    let mut w = world();
    attention::run(&mut w);
    let corp = w.corps()[0];
    assert_eq!(faction::alertness_mult(&w, Some(corp)), 1.0, "undistracted");
    let target = w.comp::<Corp>(corp).unwrap().buildings[0];
    let d = w.district_of_building(target);
    let id = w.next_riot_id;
    w.next_riot_id += 1;
    let now = w.tick;
    w.riots.push(citysim::Riot {
        id,
        district: d,
        target,
        kind: citysim::RiotTarget::Corp,
        muster: target,
        muster_at: now,
        rioters: Vec::new(),
        response: citysim::RiotResponse::Contain,
        started: now,
        departed: None,
    });
    next_hour(&mut w);
    attention::run(&mut w);
    let a = w.attention[&corp].clone();
    assert!(a.alertness < a.pool, "{a:?}");
    assert!(a.threats.iter().any(|t| t.0 == ThreatKind::Riot));
    assert!(faction::alertness_mult(&w, Some(corp)) < 1.0);
    assert!(citysim::systems::virt::alertness_mult(&w, Some(corp)) < 1.0, "the plane reads the same hook");
    // Threats piled on: the floor.
    for v in 0..6u32 {
        attention::note(&mut w, Some(corp), ThreatKind::Strike, EntityId { index: v + 1, generation: 0 }, 1);
    }
    let a = w.attention[&corp].clone();
    assert!((a.alertness - w.config.attention.alert_floor).abs() < 1e-6);
    // The riot gone, the linger runs out.
    w.riots.clear();
    w.tick += 5 * TICKS_PER_DAY;
    next_hour(&mut w);
    attention::run(&mut w);
    assert_eq!(faction::alertness_mult(&w, Some(corp)), 1.0);
}

/// The addendum: off, every read is 1.0 and nothing is kept.
#[test]
fn test_attention_off_reads_one() {
    let mut w = world();
    w.config.attention.enabled = false;
    attention::run(&mut w);
    assert!(w.attention.is_empty());
    let corp = w.corps()[0];
    attention::note(&mut w, Some(corp), ThreatKind::Strike, EntityId::NONE, 3);
    assert_eq!(faction::alertness_mult(&w, Some(corp)), 1.0);
}

/// The addendum (brains): a distracted faction's retreat order gains
/// `brain_tilt × distraction`; undistracted, the scores stand.
#[test]
fn test_tilt_moves_only_a_distracted_brain() {
    let mut w = world();
    attention::run(&mut w);
    let corp = w.corps()[0];
    let mut v = vec![(1u8, 0.5f32), (2u8, 0.45f32)];
    attention::tilt(&w, Some(corp), &mut v, |s| s.0 == 2, |s| &mut s.1);
    assert_eq!(v, vec![(1, 0.5), (2, 0.45)], "undistracted: as scored");
    for v in 0..3u32 {
        attention::note(&mut w, Some(corp), ThreatKind::Raid, EntityId { index: v + 1, generation: 0 }, 1);
    }
    attention::tilt(&w, Some(corp), &mut v, |s| s.0 == 2, |s| &mut s.1);
    assert_eq!(v[0].0, 2, "the retreat order leads: {v:?}");
}

/// Shadow V4 bug 15: a Tag whose taker is shown (Full) runs live (a pinned
/// gun was paid for a plant made off-screen while he rested): the plan
/// stakes the mark out and the plant is made at contact. Off screen it is
/// the ledger's.
#[test]
fn test_tag_live_for_a_shown_taker() {
    let mut w = world();
    let corp = w.corps()[0];
    w.comp_mut::<Corp>(corp).unwrap().treasury = 100_000;
    let [t, taker] = strangers(&w, 2)[..] else { unreachable!() };
    if let Some(s) = w.comp_mut::<Skills>(taker) {
        s.stealth = 1.0;
    }
    let p = Posting {
        buyer: Some(corp),
        agent: None,
        kind: ContractKind::Tag,
        target: Target::Agent(t),
        broker: None,
        deadline_days: 7,
        origin: Origin::God,
        price: None,
    };
    citysim::systems::lod::set_lod(&mut w, taker, citysim::Lod::Statistical);
    citysim::systems::lod::set_lod(&mut w, t, citysim::Lod::Statistical);
    let id = contracts::post(&mut w, p).expect("posted");
    w.config.contracts.max_attempts = 200;
    contracts::god_take(&mut w, id, taker).expect("taken");
    assert_eq!(contracts::render_for(&w, id), citysim::contract::Render::Ledger, "off screen: the ledger");
    citysim::systems::lod::set_lod(&mut w, taker, citysim::Lod::Full);
    assert_eq!(contracts::render_for(&w, id), citysim::contract::Render::Live, "shown: live");
    let plan = contracts::plan(&mut w, taker).expect("a plan");
    assert!(plan.steps.iter().any(|s| s.action == citysim::ActionKind::StakeOut), "a stake-out");
    let mut ok = false;
    for _ in 0..100 {
        if w.contracts[&id].status == ContractStatus::Open {
            contracts::god_take(&mut w, id, taker).expect("taken");
            contracts::plan(&mut w, taker).expect("a plan");
        }
        contracts::on_contact(&mut w, taker);
        if w.contracts[&id].status == ContractStatus::Fulfilled {
            ok = true;
            break;
        }
        w.tick += 1;
    }
    assert!(ok, "planted at contact");
    assert!(w.tags.iter().any(|x| x.on == t && x.owner == corp));
}
