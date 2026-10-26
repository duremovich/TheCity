//! The pressure pass phase 2 (docs/PRESSURE_PASS.md § Phase 2; OPEN-176,
//! OPEN-190, OPEN-120, OPEN-017): the dead trades get customers. Under
//! `[customers]`; absent, every entry point is a no-op and the trades serve
//! whom they served. Each item sends a body to a trade that existed but
//! served nobody on screen, through the machinery that trade already had
//! (the service encounter, the Unwind ladder, the Shop goal, the Treat goal,
//! the cab's booked ride, the labour corner):
//!
//! - **The door** ([`door_cause`], read by `leisure::door_ok`): a venue
//!   whose door a fighting trade keeps (the Bouncer's Club, Den, FightPit
//!   and StripClub) turns away, while a bouncer is on duty, a visitor who is
//!   drunk or high (`misery::influence`), started a brawl within
//!   `door_brawl_days`, carries a weapon (`door_armed`), wears a rival gang's colours (the venue's gang: its
//!   owner, else the holder of its district's Homes) or (Club, StripClub)
//!   dresses under `door_dress_min`, on `door_p` (a hash of the visitor, the
//!   venue and the hour). The refusal is `service::refused` (the bouncer
//!   named, the cause in the event); a drunk, high or brawling refused
//!   swings at the bouncer on `door_swing_p × (1 − lawfulness)` ([`scene`]:
//!   a fight, an Assault raised, "threw X out"), and holds a grudge.
//! - **Clients** ([`client_mult`], [`lonely_need`]): a venue with its tipped
//!   worker on duty (a companion's Lounge, a host's Dollhouse, a dancer's
//!   StripClub or Club, a Brothel) is weighed on the Unwind ladder by the
//!   visitor's loneliness (intimacy under `lonely_intimacy`) and misery;
//!   the lonely with `client_coins` unwind for it even with fun full. The
//!   visit is the existing `Enjoy` and its service encounter, so the worker
//!   and the client each remember the other.
//!
//! The Statistical mirror: the door is `door_ok`, which the off-screen
//! visit (`leisure::stat_visit`) already meets; the clients' pull is on the
//! Full tier's ladder only (the off-screen evening is the learned table's,
//! `stat_table.toml`), one sentence in the spec.

use crate::components::{Brain, Building, BuildingKind, Job, Lod, Needs, Personality, Role};
use crate::config::CustomersCfg;
use crate::entity::EntityId;
use crate::time::TICKS_PER_HOUR;
use crate::world::World;

const PURPOSE_DOOR: u64 = 0xc057_0001;
const PURPOSE_SWING: u64 = 0xc057_0002;

pub fn cfg(world: &World) -> Option<&CustomersCfg> {
    world.config.customers.as_ref()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) | (u64::from(id.generation) << 32)
}

// ---------------------------------------------------------------------------
// The door
// ---------------------------------------------------------------------------

/// Does a fighting trade (the Bouncer) keep the door of venues of `kind`?
pub fn kept_kind(world: &World, kind: BuildingKind) -> bool {
    world.config.trades.iter().any(|r| {
        r.skill == crate::config::TradeSkill::Fighting
            && r.model == crate::config::TradeModel::Service
            && r.workplace.contains(&kind)
    })
}

/// The venue's gang: its owner when a gang, else the holder of its district.
fn venue_gang(world: &World, b: EntityId) -> Option<EntityId> {
    let owner = world.owner_of(b);
    if let Some(o) = owner.filter(|&o| world.comp::<crate::components::Gang>(o).is_some()) {
        return Some(o);
    }
    crate::systems::gang::holder_of(world, b)
}

/// The cause a bouncer at `b` would turn `id` away for, or `None`.
pub fn cause(world: &World, id: EntityId, b: EntityId) -> Option<&'static str> {
    let c = cfg(world)?;
    let kind = world.comp::<Building>(b)?.kind;
    if !kept_kind(world, kind) || world.staff_of(b).contains(&id) {
        return None;
    }
    if world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard) {
        return None;
    }
    if let Some(s) = crate::systems::misery::influence(world, id) {
        return Some(s);
    }
    let day = world.day() as u32;
    if crate::systems::misery::last_brawl(world, id).is_some_and(|d| day.saturating_sub(d) < c.door_brawl_days) {
        return Some("a brawler");
    }
    // Carrying (a weapon on the body): the door takes no piece in.
    if c.door_armed && world.comp::<crate::components::Kit>(id).is_some_and(|k| k.weapon > 0) {
        return Some("carrying");
    }
    if let (Some(mine), Some(theirs)) = (world.gang_of(id), venue_gang(world, b)) {
        let worn = crate::systems::reputation::appearance_of(world, id).colours == Some(mine);
        if worn && mine != theirs {
            return Some("a rival's colours");
        }
    }
    if matches!(kind, BuildingKind::Club | BuildingKind::StripClub)
        && crate::systems::reputation::appearance_of(world, id).dress < c.door_dress_min
    {
        return Some("no dress");
    }
    None
}

/// `leisure::door_ok`'s bouncer: the cause `id` is turned away from `b`
/// for now (a bouncer on duty, `door_p` on the hour's hash), else `None`.
pub fn door_cause(world: &World, id: EntityId, b: EntityId) -> Option<&'static str> {
    let c = cfg(world)?;
    let why = cause(world, id, b)?;
    if crate::systems::trades::door_staff(world, b).is_empty() {
        return None;
    }
    let hour = world.tick / TICKS_PER_HOUR;
    (hash(world, PURPOSE_DOOR, id, key(b) ^ hour.rotate_left(17)) < c.door_p).then_some(why)
}

/// `service::refused`'s scene: the turned-away `id` (drunk, high or a
/// brawler) swings at `bouncer` on `door_swing_p × (1 − lawfulness)`: a
/// fight (`law::resolve_fight_mods`), an Assault raised, the event. Either
/// way the refused holds a grudge on the bouncer. Returns the scene's line.
pub fn scene(world: &mut World, id: EntityId, bouncer: EntityId, b: EntityId, why: &str) -> Option<String> {
    let c = cfg(world)?.clone();
    if crate::systems::law::living(world, id) {
        crate::systems::grudges::add(world, id, bouncer, crate::word::GrudgeCause::Assaulted, c.door_grudge, 0);
    }
    if !matches!(why, "drunk" | "high" | "a brawler" | "carrying") {
        return None;
    }
    // A body on either side: the scene is for the camera.
    let body = |w: &World, a: EntityId| w.comp::<Brain>(a).is_some_and(|br| br.lod != Lod::Statistical);
    if !body(world, id) && !body(world, bouncer) {
        return None;
    }
    let lawful = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    let p = c.door_swing_p * (1.0 - lawful).max(0.0);
    if hash(world, PURPOSE_SWING, id, world.tick) >= p {
        return None;
    }
    let tile = world.comp::<crate::components::Position>(id).map_or_else(Default::default, |p| p.tile);
    let (winner, _, died) =
        crate::systems::law::resolve_fight_mods(world, id, bouncer, crate::systems::law::FightMods::default());
    if crate::systems::law::living(world, id) {
        crate::systems::law::raise_crime_on(
            world,
            id,
            Some(bouncer),
            Some(bouncer),
            crate::components::Crime::Assault,
            tile,
        );
    }
    if world.comp::<crate::components::Position>(id).is_some_and(|p| p.building == Some(b)) {
        world.abort_plan(id);
        world.leave_building(id);
    }
    crate::systems::ownership::jobs2_row(world).door_scenes += 1;
    let (who, bo, at) = (world.name_of(id), world.name_of(bouncer), world.name_of(b));
    Some(if winner == bouncer {
        format!(
            "{who} ({why}) swung at {bo} at the door of {at}; {bo} threw {who} out{}",
            if died { ", dead" } else { "" }
        )
    } else {
        format!(
            "{who} ({why}) swung at {bo} at the door of {at} and put {bo} down{}",
            if died { " for good" } else { "" }
        )
    })
}

// ---------------------------------------------------------------------------
// Clients
// ---------------------------------------------------------------------------

/// `id`'s loneliness: 0 at or above `lonely_intimacy`, 1 at intimacy 0.
pub fn lonely(world: &World, id: EntityId) -> f32 {
    let Some(c) = cfg(world) else { return 0.0 };
    let i = world.comp::<Needs>(id).map_or(1.0, |n| n.intimacy);
    ((c.lonely_intimacy - i) / c.lonely_intimacy.max(0.01)).clamp(0.0, 1.0)
}

/// Is a client's worker on duty at `b` (a companion, a host, a dancer, a
/// Brothel's worker: a tipped row's staff; a Dollhouse staffed)?
pub fn worker_on(world: &World, b: EntityId) -> bool {
    match world.comp::<Building>(b).map(|bd| bd.kind) {
        Some(BuildingKind::Dollhouse) => crate::systems::dollhouse::staffed_now(world, b),
        Some(BuildingKind::Lounge | BuildingKind::StripClub | BuildingKind::Club | BuildingKind::Brothel) => {
            crate::systems::vice::tipped_on_duty(world, b).is_some()
        }
        _ => false,
    }
}

/// The ladder's weight on a client's venue: `1 + lonely_pull × lonely +
/// misery_pull × misery` with its worker on duty (1 otherwise, and off).
pub fn client_mult(world: &World, id: EntityId, b: EntityId) -> f32 {
    let Some(c) = cfg(world) else { return 1.0 };
    if !worker_on(world, b) {
        return 1.0;
    }
    1.0 + c.lonely_pull * lonely(world, id) + c.misery_pull * crate::systems::misery::misery(world, id)
}

/// Unwind's need term for a lonely body whose pick is a client's venue:
/// `lonely_unwind × lonely` (0 when not lonely, short of `client_coins`, off).
pub fn lonely_need(world: &World, id: EntityId, venue: Option<EntityId>) -> f32 {
    let Some(c) = cfg(world) else { return 0.0 };
    let Some(b) = venue.filter(|&b| worker_on(world, b)) else { return 0.0 };
    let _ = b;
    if world.comp::<crate::components::Wallet>(id).is_none_or(|w| w.coins < c.client_coins) {
        return 0.0;
    }
    c.lonely_unwind * lonely(world, id)
}

/// `leisure::unwind_satisfied`: a lonely body with `client_coins` is not
/// satisfied by a full fun meter (the ladder may still pick a free rung).
pub fn lonely_wants(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    lonely(world, id) > 0.0 && world.comp::<crate::components::Wallet>(id).is_some_and(|w| w.coins >= c.client_coins)
}

// ---------------------------------------------------------------------------
// The frightened
// ---------------------------------------------------------------------------

/// `weapons::has_reason`'s fear: safety under `fear_safety` (a beating, a
/// Hunt, a district at war: the needs model's reading of it), or a living
/// enemy who holds `id` as his Enemy (a brawl's, a grudge's edge).
pub fn frightened(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    if world.comp::<Needs>(id).is_some_and(|n| n.safety < c.fear_safety) {
        return true;
    }
    // An enemy (a brawl's, a grudge's Enemy edge) who would come for him.
    world.neighbours(id).any(|o| {
        crate::systems::law::living(world, o)
            && world.edge(o, id).is_some_and(|e| e.kind == crate::components::RelKind::Enemy)
    })
}

// ---------------------------------------------------------------------------
// Care: the hurt and the sick walk in (and the back room)
// ---------------------------------------------------------------------------

/// What a care trip buys.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Care {
    /// A lost fight's wound seen to.
    Patch,
    /// The back room's cut-rate remission of a diagnosed cancer.
    Onco,
}

/// A lost fight within `hurt_hours`, not yet seen to (a body's memory).
pub fn hurt(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    let since = world.tick.saturating_sub(u64::from(c.hurt_hours) * TICKS_PER_HOUR);
    let seen = world.comp::<crate::components::Misery>(id).map_or(0, |m| m.patched_at);
    world.comp::<crate::components::Memory>(id).is_some_and(|m| {
        m.entries.iter().any(|e| {
            e.kind == crate::components::MemoryKind::Lost && e.salience >= 0.6 && e.tick >= since && e.tick > seen
        })
    })
}

fn coins(world: &World, id: EntityId) -> i64 {
    world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins)
}

/// A standing, open, staffed back room (a BlackClinic) nearest `id`, within
/// `back_reach_ticks` of travel (a back room is worth a longer walk than a
/// Clinic: there are few, and nobody asks).
fn back_room(world: &World, id: EntityId) -> Option<EntityId> {
    let cap = cfg(world)?.back_reach_ticks;
    let from = crate::systems::life::origin(world, id)?;
    let (d, b) = world
        .buildings_of_kind(BuildingKind::BlackClinic)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .filter(|&b| !world.is_closed(b) && !world.staff_of(b).is_empty())
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()?;
    let door = world.comp::<Building>(b)?.door;
    let _ = d;
    (crate::systems::distance::travel(world, from, door) <= cap).then_some(b)
}

/// One who would rather not walk into a licensed Clinic: wanted, a gang
/// member (the gang-hurt), or short of its price.
fn avoids_clinic(world: &World, id: EntityId, price: i64) -> bool {
    crate::systems::law::wanted(world, id) || world.gang_of(id).is_some() || coins(world, id) < price
}

/// The care trip `id` would make now: `(place, what, price)`. A diagnosed
/// cancer whose pool falls short of the Clinic's treatment goes to the
/// back room when the pool holds `back_onco_price`; a fresh wound to the
/// back room when the patient avoids a Clinic (paying what the pool holds,
/// up to `back_price`), else
/// to the nearest open Clinic with `patch_price`. `None`: nothing to see
/// to, or no coins (the broke beg, scavenge or pawn through Earn).
pub fn care_choice(world: &World, id: EntityId) -> Option<(EntityId, Care, i64)> {
    let c = cfg(world)?;
    if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
        return None;
    }
    if crate::systems::health::saving_short(world, id) && crate::systems::health::pool(world, id) >= c.back_onco_price {
        if let Some(b) = back_room(world, id) {
            return Some((b, Care::Onco, c.back_onco_price));
        }
    }
    if !hurt(world, id) {
        return None;
    }
    // The back room takes the broke too: they pay what they and theirs
    // hold (the patient, the kin, the gang), up to `back_price`.
    if avoids_clinic(world, id, c.patch_price) {
        if let Some(b) = back_room(world, id) {
            return Some((b, Care::Patch, c.back_price));
        }
    }
    if coins(world, id) < c.patch_price {
        return None;
    }
    let from = crate::systems::life::origin(world, id)?;
    let clinic = crate::systems::assets::nearest_seller(world, BuildingKind::Clinic, from, true)?;
    if crate::systems::distance::out_of_reach(world, id, clinic, |r| r.clinic_ticks) {
        return None;
    }
    Some((clinic, Care::Patch, c.patch_price))
}

/// The Treat goal's care plan: `[GoTo(Clinic) b, Patch b]`.
pub fn care_plan(world: &World, id: EntityId) -> Option<crate::goap::Plan> {
    let (b, _, _) = care_choice(world, id)?;
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let steps = vec![
        step(crate::goap::ActionKind::GoTo(crate::goap::LocationKey::Clinic), Some(b)),
        step(crate::goap::ActionKind::Patch, Some(b)),
    ];
    Some(crate::goap::Plan {
        goal: crate::components::GoalKind::Treat,
        target: Some(b),
        steps,
        started_tick: world.tick,
    })
}

/// The doctor on duty at `b` for a patient (a Ripperdoc or an orderly at a
/// Clinic, a Back-Room Doc at a BlackClinic).
fn doctor(world: &World, b: EntityId) -> Option<EntityId> {
    crate::systems::service::servers_on_duty(world, b, crate::components::ServeKind::Patient).into_iter().next()
}

/// `Patch`'s ticks.
pub fn patch_ticks(world: &World) -> crate::time::Tick {
    cfg(world).map_or(30, |c| u64::from(c.patch_min.max(1)))
}

/// `Patch` may start: inside `b`, the care still wanted there, a doctor on
/// duty (else the step fails and the goal comes back).
pub fn can_patch(world: &World, id: EntityId, target: Option<EntityId>) -> bool {
    let Some(b) = target else { return false };
    world.comp::<crate::components::Position>(id).and_then(|p| p.building) == Some(b)
        && !world.is_closed(b)
        && doctor(world, b).is_some()
        && care_choice(world, id).is_some_and(|(p, _, _)| p == b)
}

/// `Patch` done: the price to the place's owner (`Flow::Treatment`), the
/// wound seen to (or the remission), the doctor's encounter (both sides
/// remember it), the event and the counters.
pub fn patch(world: &mut World, id: EntityId, target: Option<EntityId>) -> bool {
    use crate::systems::ownership::{self, Flow};
    let Some(b) = target else { return false };
    let Some((place, what, price)) = care_choice(world, id).filter(|&(p, _, _)| p == b) else { return false };
    let Some(doc) = doctor(world, place) else { return false };
    let back = world.comp::<Building>(place).is_some_and(|bd| bd.kind == BuildingKind::BlackClinic);
    let to = world.owner_of(place);
    let paid = match what {
        Care::Patch if !back => ownership::pay(world, Some(id), to, price, Flow::Treatment),
        Care::Patch | Care::Onco => {
            let mut due = price;
            let mut paid = 0;
            for (payer, holds) in crate::systems::health::payers(world, id) {
                if due <= 0 {
                    break;
                }
                let take = holds.min(due);
                if take > 0 {
                    let p = ownership::pay(world, Some(payer), to, take, Flow::Treatment);
                    due -= p;
                    paid += p;
                }
            }
            paid
        }
    };
    ownership::credit(world, place, paid);
    let (who, dn, at) = (world.name_of(id), world.name_of(doc), world.name_of(place));
    let text = match what {
        Care::Patch => {
            let now = world.tick;
            if !world.has::<crate::components::Misery>(id) {
                world.insert(id, crate::components::Misery::default());
            }
            if let Some(m) = world.comp_mut::<crate::components::Misery>(id) {
                m.patched_at = now;
            }
            ownership::jobs2_row(world).patched += 1;
            if back {
                format!("{dn} patched {who} up in the back room of {at} for {paid}c, no questions asked")
            } else {
                format!("{dn} patched {who} up at {at} for {paid}c")
            }
        }
        Care::Onco => {
            let mult = cfg(world).map_or(0.5, |c| c.back_onco_mult);
            let rem = world.config.sickness.remission * mult;
            if let Some(c) = world.comp_mut::<crate::components::Health>(id).and_then(|h| h.cancer.as_mut()) {
                c.stage = (c.stage - rem).max(0.0);
            }
            world.stats.current.treatments += 1;
            format!("{dn} cut back {who}'s vat-cancer in the back room of {at} for {paid}c (no cure)")
        }
    };
    if back {
        ownership::jobs2_row(world).black_patients += 1;
    }
    world.push_event(crate::events::EventKind::Treated, &[id, doc, place], text);
    crate::systems::service::arrive(world, id, place, crate::components::ServeKind::Patient, paid);
    true
}

// ---------------------------------------------------------------------------
// The cab
// ---------------------------------------------------------------------------

/// `exec::actions::end_shift`: a fare trade's body (the Cab Driver) is paid
/// `min(1, fares / cab_fares_full)` of the day's wage for the shift's fares
/// (the count cleared); `None` for any other worker, a Statistical driver
/// (the mirror keeps the wage) and off.
pub fn fare_scale(world: &mut World, id: EntityId) -> Option<f32> {
    let full = cfg(world)?.cab_fares_full.max(1);
    let j = world.comp::<Job>(id)?;
    let fare = crate::systems::trades::row_of(world, j.role).is_some_and(|r| r.duty == crate::config::TradeDuty::Fare);
    if !fare || world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
        return None;
    }
    let n = world.jobs2.fares_shift.remove(&id).unwrap_or(0);
    Some((f32::from(n) / f32::from(full)).min(1.0))
}

// ---------------------------------------------------------------------------
// The scav's day
// ---------------------------------------------------------------------------

/// A member of a scav gang (a type with `organ_targets`), a body.
pub fn scav(world: &World, id: EntityId) -> bool {
    cfg(world).is_some()
        && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
        && world.gang_of(id).is_some_and(|g| crate::systems::gray::organ_targets(world, g))
}

/// The scav's round's bin (off hours): the nearest Market with scraps in
/// its bin, not held by another diver, within `scav_reach` tiles.
pub fn scav_bin(world: &World, id: EntityId) -> Option<EntityId> {
    let c = cfg(world)?;
    if !scav(world, id) || !crate::systems::leisure::off_hours(world, id) {
        return None;
    }
    let from = world.comp::<crate::components::Position>(id)?.tile;
    let reach = c.scav_reach;
    let pass = |b: EntityId| {
        world.comp::<Building>(b).is_some_and(|bd| {
            !bd.demolished && bd.bin > 0 && bd.bin_diver.is_none_or(|(d, t)| d == id || t <= world.tick)
        })
    };
    let b = world.near_table(BuildingKind::Market).nearest_passing(from, pass)?;
    let t = crate::systems::bins::bin_tile(world, b)?;
    (t.manhattan(from) <= reach).then_some(b)
}

/// GangWork's scripted plan for a scav off hours: `[GoTo(Bin) b, Scavenge b]`,
/// the day's gang task (one round a day).
pub fn scav_plan(world: &World, id: EntityId) -> Option<crate::goap::Plan> {
    let b = scav_bin(world, id)?;
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let steps = vec![
        step(crate::goap::ActionKind::GoTo(crate::goap::LocationKey::Bin), Some(b)),
        step(crate::goap::ActionKind::Scavenge, Some(b)),
    ];
    Some(crate::goap::Plan {
        goal: crate::components::GoalKind::GangWork,
        target: Some(b),
        steps,
        started_tick: world.tick,
    })
}

/// A scav's Scavenge at a bin ended: the find named (`found` coins), the
/// bin's scraps cut by one (picked over), an event the diary reads.
pub fn scav_note(world: &mut World, id: EntityId, target: Option<EntityId>, found: i64) {
    let Some(b) = target.filter(|_| scav(world, id)) else { return };
    if world.comp::<Brain>(id).and_then(|br| br.current_goal) != Some(crate::components::GoalKind::GangWork) {
        return;
    }
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.bin = bd.bin.saturating_sub(1);
    }
    // The round is the day's gang task (one a day, as an extortion is).
    let day = world.day();
    if let Some(br) = world.comp_mut::<Brain>(id) {
        br.gang_task_day = Some(day);
    }
    let (who, at) = (world.name_of(id), world.name_of(b));
    let text = if found > 0 {
        format!("{who} picked over the bins behind {at} on the scav round and sold a find for {found}c")
    } else {
        format!("{who} picked over the bins behind {at} on the scav round: nothing worth selling")
    };
    world.push_event(crate::events::EventKind::Served, &[id, b], text);
}

/// `chrome::loot_target`'s reach: a scav walks `scav_reach` tiles to a body
/// (the gang's organs), anyone else `loot_reach`.
pub fn loot_reach(world: &World, id: EntityId, base: u32) -> u32 {
    match cfg(world) {
        Some(c) if scav(world, id) => base.max(c.scav_reach),
        _ => base,
    }
}
