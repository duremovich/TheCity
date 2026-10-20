//! M16b phase 4 (docs/M16_CONTRACTS.md § 3): tags, scanners and the
//! Purist Patrol.
//!
//! A game abstraction: a tag is a record in `World::tags` (plan deviation:
//! not an M13 asset; its device is bought from the nearest Security Office
//! at `tag_price[tier]`, `Flow::Asset`) writing its carrier's sighting to
//! its owner's database once an hour at `tag_conf[tier]` until the battery
//! (`tag_battery_days`) runs out. A `Tag` record's taker plants it with a
//! contest (the taker's thief tier against `1 + Kit.sight`). Clinics and
//! Security Offices keep a scanner (`clinic_scanner`, `office_scanner`);
//! an adult with heat ≥ `scan_heat` or a hunter on it goes for a scan once
//! in `scan_days` (`scan_price` to the owner): each tag on it is contested
//! scanner tier against tag tier, a win destroys it (`TagFound`), with a
//! `Tagged` memory naming the owner's head at a margin ≥ 1. A Purist gang
//! under Expand buys a tier-1 scanner once and, on `patrol_share` of its
//! evenings, scans the busiest spot in a district it holds (its member
//! nearest the spot; the scanner against each chromed adult's visible
//! tier): a find destroys the adult's tags and the member attacks (an
//! Assault under the normal rules). Rolls on `ContractNs::Leverage`
//! with the spec's purpose bytes (18 the plant, 19 the scan; 23 the
//! Patrol's evening, 24 its contest).
//!
//! `[tags] enabled = false`: no tag, no scan, no
//! Patrol (the phase-3 city).

use rand::Rng;

use crate::components::{BuildingKind, Controller, Gang, Identity, Kit, Lod, MemoryKind, Order, Position};
use crate::contract::{ContractId, Tag};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::systems::{contracts, law, security};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// Is phase 4's tag half on?
pub fn on(world: &World) -> bool {
    world.config.tags.enabled
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Purpose {
    Plant = 18,
    Scan = 19,
    Patrol = 23,
    PatrolScan = 24,
}

fn rng(world: &World, p: Purpose, a: u64, b: u64) -> rand_chacha::ChaCha8Rng {
    crate::systems::leverage::leverage_rng(world, p as u8, a, b)
}

fn step(world: &World) -> f32 {
    world.config.chrome.contest_step
}

/// The tags on `id`.
pub fn tags_on(world: &World, id: EntityId) -> Vec<&Tag> {
    world.tags.iter().filter(|t| t.on == id).collect()
}

/// Plant a tier-`tier` tag on `on` for `owner` (`max_tags` city-wide).
/// The tag's id, `None` at the cap or for a dead carrier.
pub fn plant(world: &mut World, on: EntityId, owner: EntityId, tier: u8) -> Option<u64> {
    if !self::on(world) || !law::living(world, on) || world.tags.len() >= world.config.tags.max_tags {
        return None;
    }
    let tier = tier.clamp(1, 3);
    let id = world.next_tag.max(1);
    world.next_tag = id + 1;
    let until = world.tick + Tick::from(world.config.tags.tag_battery_days) * TICKS_PER_DAY;
    world.tags.push(Tag { id, on, owner, tier, until });
    world.stats.current.gov.tags_planted += 1;
    let text = format!("{} was tagged for {} (tier {tier})", world.name_of(on), world.owner_label(Some(owner)));
    world.push_event(EventKind::Tagged, &[on, owner], text);
    Some(id)
}

/// The nearest Security Office to `from` (its door), ties the lower id.
fn nearest_office(world: &World, from: crate::components::TilePos) -> Option<EntityId> {
    nearest_of(world, from, &[BuildingKind::SecurityOffice])
}

fn nearest_of(world: &World, from: crate::components::TilePos, kinds: &[BuildingKind]) -> Option<EntityId> {
    kinds
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&b| world.comp::<crate::components::Building>(b).is_some_and(|bd| !bd.demolished))
        .filter_map(|b| world.comp::<crate::components::Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

fn here(world: &World, id: EntityId) -> crate::components::TilePos {
    world.comp::<Position>(id).map(|p| p.tile).unwrap_or_default()
}

/// A `Tag` record at its ledger due: the buyer must afford the device
/// (`tag_price[1]`); the taker's thief tier against the target's `1 +
/// Kit.sight` (purpose 18); a win buys the device (`Flow::Asset`, to the
/// nearest Security Office's owner), plants the tag and fulfils the record.
pub fn resolve_plant(world: &mut World, id: ContractId, taker: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Some(t) = c.target_agent() else { return };
    let Some(owner) = c.buyer.or(c.agent) else { return };
    if !law::living(world, t) || !on(world) {
        contracts::settle(world, id, contracts::Settle::Cancelled, "the target is gone");
        return;
    }
    if world.tags.len() >= world.config.tags.max_tags {
        contracts::settle(world, id, contracts::Settle::Cancelled, "no tag to be had");
        return;
    }
    let price = world.config.tags.tag_price[1];
    if world.purse(Some(owner)) < price {
        contracts::fail_attempt(world, id, "no coin for the tag");
        return;
    }
    let att = security::thief_tier(law::stealth(world, taker));
    let def = 1 + world.comp::<Kit>(t).map_or(0, |k| k.sight);
    let won = security::contest(att, def, step(world), &mut rng(world, Purpose::Plant, id, world.tick));
    if !won {
        contracts::fail_attempt(world, id, "the mark felt the hand");
        return;
    }
    let seller = nearest_office(world, here(world, t)).and_then(|b| world.owner_of(b));
    ownership::pay(world, Some(owner), seller, price, Flow::Asset);
    if plant(world, t, owner, 1).is_some() {
        contracts::settle(world, id, contracts::Settle::Fulfilled, "");
    } else {
        contracts::settle(world, id, contracts::Settle::Cancelled, "no tag to be had");
    }
}

/// A building's scanner tier (a Clinic's, a Security Office's), 0 none.
pub fn scanner_tier(world: &World, b: EntityId) -> u8 {
    match world.comp::<crate::components::Building>(b).map(|bd| bd.kind) {
        Some(BuildingKind::Clinic) => world.config.tags.clinic_scanner,
        Some(BuildingKind::SecurityOffice) => world.config.tags.office_scanner,
        _ => 0,
    }
}

/// Contest a scanner of `tier` against every tag on `agent` (purpose 19,
/// keyed by the tag): each win destroys the tag (`TagFound`, `at` the
/// scanner's building or the Patrol's gang) and, at a margin ≥ 1, leaves a
/// `Tagged` memory naming the owner's head. The tags found.
pub fn sweep(world: &mut World, agent: EntityId, tier: u8, at: EntityId) -> u32 {
    let on_it: Vec<Tag> = world.tags.iter().filter(|t| t.on == agent).cloned().collect();
    let mut found = 0;
    for t in on_it {
        let won = security::contest(tier, t.tier, step(world), &mut rng(world, Purpose::Scan, t.id, world.tick));
        if !won {
            continue;
        }
        world.tags.retain(|x| x.id != t.id);
        found += 1;
        world.stats.current.gov.tags_found += 1;
        let text =
            format!("{}'s tag (for {}) was found and burned", world.name_of(agent), world.owner_label(Some(t.owner)));
        world.push_event(EventKind::TagFound, &[agent, at], text);
        if tier > t.tier && world.has::<crate::components::Memory>(agent) {
            let head = contracts::head_of(world, t.owner).or(Some(t.owner)).filter(|&h| world.has::<Identity>(h));
            if let Some(h) = head {
                world.remember(agent, MemoryKind::Tagged, Some(h), 0.6, -0.5, false);
            }
        }
    }
    found
}

/// `agent` pays for a scan at `b` (`scan_price`, `Flow::Asset`, to the
/// owner) and is swept at its tier. The tags found.
pub fn scan_at(world: &mut World, agent: EntityId, b: EntityId) -> u32 {
    let tier = scanner_tier(world, b);
    if tier == 0 {
        return 0;
    }
    let price = world.config.tags.scan_price;
    let owner = world.owner_of(b);
    ownership::pay(world, Some(agent), owner, price, Flow::Asset);
    world.stats.current.gov.scans += 1;
    sweep(world, agent, tier, b)
}

/// Hourly: the tags write their sightings (the dead and the spent drop);
/// at 10:00 the scans, at 21:00 the Patrol.
pub fn run(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if !world.tags.is_empty() {
        write(world);
    }
    let tod = u64::from(crate::time::tick_of_day(world.tick));
    if tod == 10 * TICKS_PER_HOUR {
        scan_pass(world);
    }
    if tod == 21 * TICKS_PER_HOUR {
        patrol_pass(world);
    }
    world.stats.current.gov.tags_live = world.tags.len() as u32;
}

fn write(world: &mut World) {
    let now = world.tick;
    world.tags.retain(|t| t.until > now);
    let live: Vec<Tag> = world.tags.iter().filter(|t| law::living(world, t.on)).cloned().collect();
    world.tags = live.clone();
    for t in live {
        let tile = here(world, t.on);
        let conf = world.config.tags.tag_conf[usize::from(t.tier).min(3)];
        let s = crate::virt::Sighting { who: t.on, tile, tick: now, confidence: conf, relayed: false };
        crate::systems::virt::record_sighting(world, Some(t.owner), s);
    }
}

/// Daily at 10:00, one adult in `scan_days` (`index + day`): a free adult
/// with heat ≥ `scan_heat` or a hunter on it, with the price, scans at the
/// nearest Clinic or Security Office.
fn scan_pass(world: &mut World) {
    let cfg = world.config.tags.clone();
    let days = u64::from(cfg.scan_days.max(1));
    let day = world.day();
    let hunted: std::collections::BTreeSet<EntityId> = world.contract_runs.values().map(|r| r.target).collect();
    // scan-ok: daily, one adult in `scan_days`.
    for a in world.citizens() {
        if !(u64::from(a.index) + day).is_multiple_of(days) || !contracts::free_adult(world, a) {
            continue;
        }
        let heat = crate::systems::reputation::rep(world, a).heat;
        if heat < cfg.scan_heat && !hunted.contains(&a) {
            continue;
        }
        if world.purse(Some(a)) < cfg.scan_price {
            continue;
        }
        let Some(b) = nearest_of(world, here(world, a), &[BuildingKind::Clinic, BuildingKind::SecurityOffice]) else {
            continue;
        };
        scan_at(world, a, b);
    }
}

/// A Purist gang's tier-1 scanner, bought once from its treasury
/// (`scanner_price[1]`, `Flow::Asset`, to the nearest Security Office's
/// owner). True when it has one.
pub fn buy_scanner(world: &mut World, g: EntityId) -> bool {
    if world.gang_scanners.contains(&g) {
        return true;
    }
    let price = world.config.tags.scanner_price[1];
    if world.purse(Some(g)) < price {
        return false;
    }
    let from = world.hideout_of(g).and_then(|h| world.comp::<crate::components::Building>(h).map(|b| b.door));
    let seller = from.and_then(|t| nearest_office(world, t)).and_then(|b| world.owner_of(b));
    ownership::pay(world, Some(g), seller, price, Flow::Asset);
    world.gang_scanners.insert(g);
    world.stats.current.gov.scanners_bought += 1;
    true
}

/// Daily at 21:00: each Purist gang under Expand, on `patrol_share` of its
/// evenings (purpose 23), patrols with its scanner.
fn patrol_pass(world: &mut World) {
    let share = world.config.tags.patrol_share;
    let day = world.day();
    for g in world.gang_list().to_vec() {
        if !crate::systems::creeds::is_purist(world, g) {
            continue;
        }
        if world.comp::<Gang>(g).is_none_or(|x| x.order != Order::Expand) {
            continue;
        }
        if rng(world, Purpose::Patrol, u64::from(g.index), day).random::<f32>() >= share {
            continue;
        }
        if !buy_scanner(world, g) {
            continue;
        }
        patrol(world, g);
    }
}

/// `g`'s Patrol now (plan deviation: resolved at the spot, no walk): the
/// busiest of L2's spots in a district `g` holds (the most free adults on
/// the street within `patrol_radius`, ties the first) is scanned by `g`'s
/// free member nearest it (a body first; `PatrolScan`): the first chromed
/// adult the scanner wins against (its visible tier as the defender's,
/// purpose 24) loses its tags and is attacked at the spot. The adult
/// attacked, if any.
pub fn patrol(world: &mut World, g: EntityId) -> Option<EntityId> {
    let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
    let r = world.config.tags.patrol_radius;
    let spots: Vec<crate::components::TilePos> = world
        .districts
        .iter()
        .enumerate()
        .filter(|(_, d)| d.control == Controller::Gang(g))
        .flat_map(|(i, _)| world.spots.get(i).map(|v| v.iter().map(|s| s.tile).collect::<Vec<_>>()).unwrap_or_default())
        .collect();
    let mut best: Option<(usize, crate::components::TilePos, Vec<EntityId>)> = None;
    for p in spots {
        let around: Vec<EntityId> = world
            .agents_near(p, r, None)
            .into_iter()
            .filter(|&a| !members.contains(&a) && world.has::<Identity>(a))
            .filter(|&a| contracts::free_adult(world, a))
            .filter(|&a| {
                world.comp::<Position>(a).is_some_and(|q| q.building.is_none() && q.tile.manhattan(p) <= r * 2)
            })
            .collect();
        if best.as_ref().is_none_or(|(n, _, _)| around.len() > *n) {
            best = Some((around.len(), p, around));
        }
    }
    let (_, spot, around) = best?;
    let m = members
        .iter()
        .copied()
        .filter(|&m| contracts::free_adult(world, m))
        .map(|m| {
            let body = world.comp::<crate::components::Brain>(m).is_some_and(|b| b.lod != Lod::Statistical);
            let d =
                if body { world.comp::<Position>(m).map_or(u32::MAX, |q| q.tile.manhattan(spot)) } else { u32::MAX };
            (d, m)
        })
        .min()
        .map(|(_, m)| m)?;
    world.stats.current.gov.patrols += 1;
    let text = format!(
        "{} of {} scanned {} at ({}, {})",
        world.name_of(m),
        world.owner_label(Some(g)),
        around.len(),
        spot.x,
        spot.y
    );
    world.push_event(EventKind::PatrolScan, &[m, g], text);
    for a in around {
        let Some(vis) = world.comp::<Kit>(a).filter(|k| k.chrome).map(|k| k.visible.min(3)) else { continue };
        let won = security::contest(
            1,
            vis,
            step(world),
            &mut rng(world, Purpose::PatrolScan, u64::from(a.index), world.tick),
        );
        if !won {
            continue;
        }
        world.stats.current.gov.patrol_finds += 1;
        sweep(world, a, 1, g);
        attack(world, m, a, spot);
        return Some(a);
    }
    None
}

/// The member's attack on a found adult, as the exec's `Attack`: the
/// fight, the Assault (or Murder) event and crime.
fn attack(world: &mut World, m: EntityId, a: EntityId, tile: crate::components::TilePos) {
    let (_, loser, died) = law::resolve_fight(world, m, a);
    let murder = died && loser == a;
    let crime = if murder { crate::components::Crime::Murder } else { crate::components::Crime::Assault };
    let text = format!("{} attacked {} for their chrome", world.name_of(m), world.name_of(a));
    world.push_event(if murder { EventKind::Murder } else { EventKind::Assault }, &[m, a], text);
    if law::living(world, m) {
        law::raise_crime_on(world, m, (!murder).then_some(a), Some(a), crime, tile);
    }
}

/// From the Locate postings (a gang's vendetta, a corp's culprit): a `Tag`
/// on the same head, direct, when the buyer can pay the record and the
/// device and no tag of its is on the head already.
pub fn post_tag(
    world: &mut World,
    buyer: EntityId,
    agent: Option<EntityId>,
    head: EntityId,
    origin: crate::contract::Origin,
) {
    if !on(world) || world.tags.len() >= world.config.tags.max_tags {
        return;
    }
    if world.tags.iter().any(|t| t.on == head && t.owner == buyer) {
        return;
    }
    let target = crate::contract::Target::Agent(head);
    let price = contracts::quote(world, crate::contract::ContractKind::Tag, &target, None);
    if world.purse(Some(buyer)) < price + world.config.tags.tag_price[1] {
        return;
    }
    let _ = contracts::brain_post_with(
        world,
        Some(buyer),
        agent,
        crate::contract::ContractKind::Tag,
        target,
        None,
        origin,
        None,
        crate::contract::PostExtra::default(),
    );
}

/// God `TagAgent`: a tag planted at once (no contest, no price).
pub fn god_tag(world: &mut World, agent: EntityId, owner: EntityId, tier: u8) -> Result<u64, String> {
    if !law::living(world, agent) || !world.has::<Identity>(agent) {
        return Err("not a living agent".to_string());
    }
    plant(world, agent, owner, tier).ok_or_else(|| "at max_tags".to_string())
}
