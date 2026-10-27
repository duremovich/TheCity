//! Jobs & lives phase 6, the gray market (docs/JOBS_AND_LIVES.md § 7;
//! Dylan's answers 1 and 3). Under `[gray] enabled` (itself under `[jobs2]
//! enabled`): off, every entry point is a no-op, no BlackClinic stands, no
//! papers or organs exist and no state is written.
//!
//! A board game's counters: tiers, units, seeded rolls and coins.
//!
//! - **Papers.** A wanted agent without papers buys a tier from a forger
//!   on staff at the nearest Fixer (`id_price`, `Flow::Gray` to the forger,
//!   `fixer_cut` to the Fixer's owner) or real ones from a Civic Clerk under
//!   `clerk_lawful` (`id_bribe`, `Flow::Bribe`), whichever is cheaper and
//!   affordable. Papers bought while wanted pass a guard's sighting for the
//!   hour on `id_w × tier` (`law::sightings`). At an arrest fake papers are
//!   exposed on `p_expose`: void, and `Forgery` filed on the holder and the
//!   forger.
//! - **Organs** (`Good::Organs`). A body a gang rips (a Harvest's take or a
//!   corpse, once each) yields `organs_per_body` units into the gang's
//!   BlackClinic (else its Hideout). Stocks lose `organ_rot` a day. At
//!   midnight a gang's units go first to the licensed Clinics' shelves
//!   (`clinic_organs`, `organ_price`, `Flow::Gray`), the rest to the World's
//!   Organs book (capped per capita, printed) past customs (`p_customs`:
//!   seized, `Smuggling` on the gang's leader). A stabilisation at a Clinic
//!   draws a unit on `p_organ_needed`.
//! - **Black clinics.** A gang under Harvest (or of a type that harvests)
//!   with its found cost and `black_clinic_spare` builds one in a district it
//!   holds. Its posts: the struck-off docs (a licensed Ripperdoc is struck
//!   off on `p_struck_off × (1 − lawfulness)` a day) and a broker. It is the
//!   chip's black install (`dollhouse`) for its gang. In a Sweep district or
//!   one where the law cracks down on its gang it is found on `p_bm_raid` a
//!   day: shut `bm_close_days`, `BlackMarket` on its staff.
//! - **Gang types** (data rows, a pure hash of the gang's id by share): a
//!   row's `harvest_flat` lifts the Harvest order, its `kill_p` is its rip's
//!   kill chance, and with `organ_targets` its Harvest target is anyone on
//!   its ground valued by chrome plus organs.
//!
//! Every coin moves between purses or across the city's edge (the World's
//! Organs book, printed). The passes are daily; the sighting term is a map
//! lookup when the map holds anyone.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::components::{Brain, Building, BuildingKind, Crime, Gang, Good, Job, Personality, Role, Sentence, Stance};
use crate::config::GangTypeCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::ExportGood;
use crate::systems::ownership::{self, Flow};
use crate::time::{TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

const PURPOSE_TYPE: u64 = 0x4752_0001;
const PURPOSE_PASS: u64 = 0x4752_0002;
const PURPOSE_EXPOSE: u64 = 0x4752_0003;
const PURPOSE_ORGAN: u64 = 0x4752_0004;
const PURPOSE_CUSTOMS: u64 = 0x4752_0005;
const PURPOSE_RAID: u64 = 0x4752_0006;
const PURPOSE_STRUCK: u64 = 0x4752_0007;

/// The gray market is live: `[jobs2] enabled` and `[gray] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.gray.enabled
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// One holder's papers (bought while wanted).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Papers {
    pub tier: u8,
    pub fake: bool,
    pub forger: Option<EntityId>,
}

/// Phase 6's gray state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrayState {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub papers: BTreeMap<EntityId, Papers>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub struck_off: BTreeSet<EntityId>,
    /// Bodies already ripped for organs.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub ripped: BTreeSet<EntityId>,
    /// Gang -> its type row (255: none), and the gangs typed so far.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<EntityId, u8>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub typed_seen: u32,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl GrayState {
    pub fn is_default(&self) -> bool {
        *self == GrayState::default()
    }
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

// ---------------------------------------------------------------------------
// Gang types
// ---------------------------------------------------------------------------

/// The gang's type row (assigned at the midnight pass, `assign_types`).
pub fn gang_type(world: &World, gang: EntityId) -> Option<&GangTypeCfg> {
    if !on(world) {
        return None;
    }
    let i = *world.jobs2.gray.types.get(&gang)?;
    world.config.gray.gang_types.get(usize::from(i))
}

/// Midnight: each gang not yet typed (ascending id: the order they formed)
/// is the `n`-th; a row takes it when `floor((n + 1) × share)` passes
/// `floor(n × share)` (the rows' shares of the gangs, in turn, with a hashed
/// phase per row so the first gang is not always typed). Untyped gangs
/// record 255.
fn assign_types(world: &mut World) {
    if world.config.gray.gang_types.is_empty() {
        return;
    }
    for g in world.gangs() {
        if world.jobs2.gray.types.contains_key(&g) {
            continue;
        }
        // Shadow V4 bug 13: a creed gang (Purist or cult) takes no gray type
        // (a cult typed as scavs harvested the chrome it forbids).
        if crate::systems::creeds::creed_of(world, g).is_some() {
            world.jobs2.gray.types.insert(g, u8::MAX);
            continue;
        }
        let n = world.jobs2.gray.typed_seen as f32;
        let mut pick = u8::MAX;
        for (i, row) in world.config.gray.gang_types.iter().enumerate() {
            let phase = hash(world, PURPOSE_TYPE, i as u64, 0);
            if ((n + 1.0) * row.share + phase).floor() > (n * row.share + phase).floor() {
                pick = i as u8;
                break;
            }
        }
        world.jobs2.gray.typed_seen += 1;
        world.jobs2.gray.types.insert(g, pick);
    }
    let gone: Vec<EntityId> = world.jobs2.gray.types.keys().copied().filter(|&g| !world.has::<Gang>(g)).collect();
    for g in gone {
        world.jobs2.gray.types.remove(&g);
    }
}

/// The order brain's tilt: a gang type's `harvest_flat` on its Harvest row
/// (re-sorted as `attention::tilt`; nothing off or untyped).
pub fn tilt_orders(world: &World, gang: EntityId, scores: &mut Vec<crate::components::OrderScore>) {
    let Some(row) = gang_type(world, gang) else { return };
    // Jobs & lives phase 8: an organ-hunting row reads the World's organ
    // book: its flat scales with the book's appetite (0.5-2; 1 at rest), so
    // a boom sends the scavs out (the M13 Harvest score itself reads no
    // price).
    let book = if row.organ_targets {
        crate::systems::world_market::appetite(world, ExportGood::Organs).clamp(0.5, 2.0)
    } else {
        1.0
    };
    let flat = row.harvest_flat * book;
    if flat == 0.0 {
        return;
    }
    let mut keyed: Vec<(f32, crate::components::OrderScore)> = scores
        .drain(..)
        .map(|mut s| {
            if s.order == crate::components::Order::Harvest {
                s.score += flat;
            }
            (s.score, s)
        })
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
    scores.extend(keyed.into_iter().map(|(_, s)| s));
}

/// A rip's kill chance for `gang` (its type row's), else `None`.
pub fn kill_p(world: &World, gang: Option<EntityId>) -> Option<f32> {
    gang.and_then(|g| gang_type(world, g)).map(|r| r.kill_p)
}

/// Does `gang`'s Harvest value organs (anyone on its ground)?
pub fn organ_targets(world: &World, gang: EntityId) -> bool {
    gang_type(world, gang).is_some_and(|r| r.organ_targets)
}

/// A body's organs at the World's bid now (`organs_per_body` units).
pub fn organ_value(world: &World) -> i64 {
    let bid = crate::systems::world_market::marginal_bid(world, ExportGood::Organs);
    (bid * f64::from(world.config.gray.organs_per_body)).round() as i64
}

// ---------------------------------------------------------------------------
// Papers
// ---------------------------------------------------------------------------

/// `law::sightings`: a wanted holder of papers bought while wanted passes a
/// guard's sighting for this hour on `id_w × tier`.
pub fn passes_sighting(world: &World, s: EntityId) -> bool {
    let Some(p) = world.jobs2.gray.papers.get(&s) else { return false };
    let hour = world.tick / TICKS_PER_HOUR;
    hash(world, PURPOSE_PASS, key(s), hour) < world.config.gray.id_w * f32::from(p.tier)
}

/// At an arrest (before the sentence): fake papers exposed on `p_expose`:
/// void, `Forgery` filed on the holder and the forger.
pub fn on_arrest(world: &mut World, suspect: EntityId) {
    if !on(world) {
        return;
    }
    let Some(p) = world.jobs2.gray.papers.get(&suspect).cloned() else { return };
    if !p.fake {
        return;
    }
    if hash(world, PURPOSE_EXPOSE, key(suspect), world.day()) >= world.config.gray.p_expose {
        return;
    }
    world.jobs2.gray.papers.remove(&suspect);
    crate::systems::law::file_report(world, Crime::Forgery, suspect, None);
    if let Some(f) = p.forger.filter(|&f| crate::systems::law::living(world, f)) {
        crate::systems::law::file_report(world, Crime::Forgery, f, None);
    }
    ownership::jobs2_row(world).exposures += 1;
    let text = format!("{}'s papers were exposed at the arrest", world.name_of(suspect));
    world.push_event(EventKind::Report, &[suspect], text);
}

/// A forger on staff at a standing Fixer, nearest `from` (ties the lower id).
fn forger_near(world: &World, from: crate::components::TilePos) -> Option<(EntityId, EntityId)> {
    let mut best: Option<(u32, EntityId, EntityId)> = None;
    for &b in world.buildings_of_kind(BuildingKind::Fixer) {
        if !standing(world, b) {
            continue;
        }
        let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { continue };
        let f = world.staff_of(b).iter().copied().find(|&s| {
            world
                .comp::<Job>(s)
                .and_then(|j| crate::systems::trades::row_of(world, j.role))
                .is_some_and(|r| r.gray && r.workplace.contains(&BuildingKind::Fixer))
                && crate::systems::law::living(world, s)
                && !world.has::<Sentence>(s)
        });
        if let Some(f) = f {
            let k = (door.manhattan(from), b, f);
            if best.is_none_or(|x| k < x) {
                best = Some(k);
            }
        }
    }
    best.map(|(_, b, f)| (b, f))
}

/// A Civic Clerk under `clerk_lawful` (the lowest id), free.
fn clerk_on_take(world: &World) -> Option<EntityId> {
    let role = world.config.trade_role("civic_clerk")?;
    let max = world.config.gray.clerk_lawful;
    // Phase-8 review: the role index, not a scan of the city.
    world
        .workers(role)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == role))
        .filter(|&a| !world.has::<Sentence>(a) && world.comp::<Personality>(a).is_some_and(|p| p.lawfulness < max))
        .min()
}

/// Midnight: the wanted without papers buy them (the cheaper of a forger's
/// dearest affordable tier and a Clerk's bribe).
fn papers_daily(world: &mut World) {
    let cfg = world.config.gray.clone();
    let wanted: Vec<EntityId> = world
        .open_suspects()
        .filter(|&s| {
            crate::systems::law::living(world, s)
                && !world.has::<Sentence>(s)
                && world.has::<Brain>(s)
                && !world.jobs2.gray.papers.contains_key(&s)
        })
        .collect();
    if wanted.is_empty() {
        return;
    }
    let clerk = clerk_on_take(world);
    // Lives at work phase 5 (§ 2.4): the forger's papers are a walk-in at
    // the Fixer's open hours (`service::queue_walk_in`), paid at midnight.
    let tower = crate::systems::service::tower_on(world);
    for s in wanted {
        if tower && crate::systems::service::walk_in_pending(world, s) {
            continue;
        }
        let coins = world.purse(Some(s));
        let Some(from) = world.comp::<crate::components::Position>(s).map(|p| p.tile) else { continue };
        let forger = forger_near(world, from);
        let fake = forger
            .and_then(|(b, f)| (1..=2u8).rev().find(|&t| cfg.id_price[usize::from(t - 1)] <= coins).map(|t| (b, f, t)));
        let bribe = clerk.filter(|_| cfg.id_bribe <= coins);
        match (fake, bribe) {
            (Some((b, f, t)), bribe) if bribe.is_none() || cfg.id_price[usize::from(t - 1)] < cfg.id_bribe => {
                // The pre-view pass phase 4 (P3, OPEN-119): the order is paid
                // at the handover (`service::walk_in_sale`), not at midnight.
                if tower && crate::systems::presence::on(world) {
                    crate::systems::service::queue_walk_in(world, s, b, f, t);
                    continue;
                }
                let price = cfg.id_price[usize::from(t - 1)];
                let cut = (price as f32 * cfg.fixer_cut).round() as i64;
                ownership::pay(world, Some(s), Some(f), price - cut, Flow::Gray);
                let owner = world.owner_of(b);
                ownership::pay(world, Some(s), owner, cut, Flow::Gray);
                ownership::jobs2_row(world).fake_sold += 1;
                // Lives at work phase 5 (§ 2.4): the order is paid now; the
                // papers are picked up in the Fixer's open hours.
                if tower {
                    crate::systems::service::queue_walk_in(world, s, b, f, t);
                    continue;
                }
                world.jobs2.gray.papers.insert(s, Papers { tier: t, fake: true, forger: Some(f) });
            }
            (_, Some(c)) => {
                ownership::pay(world, Some(s), Some(c), cfg.id_bribe, Flow::Bribe);
                world.jobs2.gray.papers.insert(s, Papers { tier: 1, fake: false, forger: None });
                ownership::jobs2_row(world).papers_bribed += 1;
                // Lives at work phase 5 (§ 2.2): the clerk on the take named.
                crate::systems::service::bribe_taken(world, c, s, cfg.id_bribe);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Organs
// ---------------------------------------------------------------------------

/// The gang's standing BlackClinic (the lowest id).
pub fn black_clinic_of(world: &World, gang: EntityId) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::BlackClinic)
        .iter()
        .copied()
        .find(|&b| standing(world, b) && world.owner_of(b) == Some(gang))
}

/// Where `gang` keeps its organs: its BlackClinic, else its Hideout.
fn organ_store(world: &World, gang: EntityId) -> Option<EntityId> {
    black_clinic_of(world, gang).or_else(|| world.hideout_of(gang))
}

/// `chrome::rip_for`'s end: a body no longer living, ripped by `gang`
/// for the first time, yields `organs_per_body` units into its store.
pub fn take_organs(world: &mut World, gang: Option<EntityId>, body: EntityId) {
    // S6 (Dylan 2026-10-27): a living child fails `living` too; never one.
    if !on(world)
        || crate::systems::law::living(world, body)
        || (world.is_alive(body)
            && !world.has::<crate::components::Corpse>(body)
            && crate::systems::ages::is_protected(world, body))
    {
        return;
    }
    let Some(g) = gang.filter(|&g| !crate::systems::creeds::is_purist(world, g)) else { return };
    if !world.jobs2.gray.ripped.insert(body) {
        return;
    }
    let Some(store) = organ_store(world, g) else { return };
    let n = world.config.gray.organs_per_body;
    let room = world.goods_cap(store, Good::Organs).saturating_sub(world.stock(store, Good::Organs));
    let added = world.add_stock(store, Good::Organs, n.min(room));
    ownership::jobs2_row(world).organs_made += added;
}

/// Does `agent`'s gang (typed with `organ_targets`) want `body`'s organs:
/// not yet ripped, no longer living?
pub fn wants_organs(world: &World, agent: EntityId, body: EntityId) -> bool {
    on(world)
        && world.gang_of(agent).is_some_and(|g| organ_targets(world, g))
        && !crate::systems::law::living(world, body)
        // S6 (Dylan 2026-10-27): a living child is no body to rip.
        && (world.has::<crate::components::Corpse>(body) || !crate::systems::ages::is_protected(world, body))
        && !world.jobs2.gray.ripped.contains(&body)
}

/// A rip's kill by a typed gang (the scavs' count).
pub fn note_kill(world: &mut World, gang: Option<EntityId>) {
    if gang.and_then(|g| gang_type(world, g)).is_some() {
        ownership::jobs2_row(world).scav_kills += 1;
    }
}

/// `wounded::on_stabilise` at `clinic`: a unit off its shelf on `p_organ_needed`.
pub fn on_stabilise(world: &mut World, clinic: EntityId) {
    if !on(world) || world.stock(clinic, Good::Organs) == 0 {
        return;
    }
    if hash(world, PURPOSE_ORGAN, key(clinic), world.tick) >= world.config.gray.p_organ_needed {
        return;
    }
    world.take_stock(clinic, Good::Organs, 1);
    ownership::jobs2_row(world).organs_used += 1;
}

/// Midnight: every organ stock loses `organ_rot` (rounded up).
fn rot(world: &mut World) {
    let rot = world.config.gray.organ_rot;
    let mut holders: Vec<EntityId> = Vec::new();
    for k in [BuildingKind::BlackClinic, BuildingKind::Hideout, BuildingKind::Clinic] {
        holders.extend(world.buildings_of_kind(k).iter().copied());
    }
    holders.sort_unstable();
    for b in holders {
        let n = world.stock(b, Good::Organs);
        if n == 0 {
            continue;
        }
        let lost = ((n as f32 * rot).ceil() as u32).min(n);
        world.take_stock(b, Good::Organs, lost);
        ownership::jobs2_row(world).organs_rotted += lost;
    }
}

/// Midnight: each gang's units (ascending) to the Clinics' shelves, then the
/// World's book past customs.
fn sell_organs(world: &mut World) {
    let cfg = world.config.gray.clone();
    let mut clinics: Vec<EntityId> =
        world.buildings_of_kind(BuildingKind::Clinic).iter().copied().filter(|&c| standing(world, c)).collect();
    clinics.sort_unstable();
    for g in world.gangs() {
        let Some(store) = organ_store(world, g) else { continue };
        if world.stock(store, Good::Organs) == 0 {
            continue;
        }
        for &c in &clinics {
            let have = world.stock(store, Good::Organs);
            let want = cfg.clinic_organs.saturating_sub(world.stock(c, Good::Organs));
            let buyer = world.owner_of(c);
            let afford = u32::try_from(world.purse(buyer).max(0) / cfg.organ_price.max(1)).unwrap_or(u32::MAX);
            let n = have.min(want).min(afford);
            if n == 0 {
                continue;
            }
            ownership::pay(world, buyer, Some(g), i64::from(n) * cfg.organ_price, Flow::Gray);
            world.take_stock(store, Good::Organs, n);
            world.add_stock(c, Good::Organs, n);
            ownership::jobs2_row(world).organs_to_clinics += n;
        }
        let have = world.stock(store, Good::Organs);
        if have == 0 || !crate::systems::world_market::open(world) {
            continue;
        }
        if hash(world, PURPOSE_CUSTOMS, key(g), world.day()) < cfg.p_customs {
            world.take_stock(store, Good::Organs, have);
            ownership::jobs2_row(world).organs_seized += have;
            if let Some(l) = world.comp::<Gang>(g).and_then(|x| x.leader) {
                crate::systems::law::file_report(world, Crime::Smuggling, l, None);
            }
            continue;
        }
        let (n, paid) = crate::systems::world_market::sell(world, Some(store), Some(g), ExportGood::Organs, have);
        world.take_stock(store, Good::Organs, n);
        let row = ownership::jobs2_row(world);
        row.organs_exported += n;
        row.organs_export_paid += paid;
    }
}

// ---------------------------------------------------------------------------
// Black clinics and struck-off docs
// ---------------------------------------------------------------------------

/// A hire's filter: a licensed Clinic never hires the struck-off; a
/// `struck_off` row hires only them.
pub fn hire_ok(world: &World, id: EntityId, role: Role) -> bool {
    if !on(world) {
        return true;
    }
    let off = world.jobs2.gray.struck_off.contains(&id);
    match role {
        Role::Ripperdoc => !off,
        Role::Trade(t) if world.config.trade(t).is_some_and(|r| r.struck_off) => off,
        _ => true,
    }
}

/// Midnight: a licensed Ripperdoc struck off on `p_struck_off × (1 −
/// lawfulness)`: off the post for good.
fn strike_off(world: &mut World) {
    let p = world.config.gray.p_struck_off;
    let day = world.day();
    // Phase-8 review: the role index, not a scan of the city (ascending, as
    // the citizens were).
    let mut pool: Vec<EntityId> = world.workers(Role::Ripperdoc).to_vec();
    pool.sort_unstable();
    let docs: Vec<EntityId> = pool
        .into_iter()
        .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == Role::Ripperdoc))
        .filter(|&a| {
            let law = world.comp::<Personality>(a).map_or(0.5, |x| x.lawfulness);
            hash(world, PURPOSE_STRUCK, key(a), day) < p * (1.0 - law)
        })
        .collect();
    for a in docs {
        world.jobs2.gray.struck_off.insert(a);
        crate::systems::economy::quit_job(world, a, "struck off");
        ownership::jobs2_row(world).struck_off += 1;
    }
}

/// Midnight: a gang under Harvest (or typed) with no BlackClinic, its found
/// cost and `black_clinic_spare`, builds one on a vacant Lot in a district
/// it holds (the lowest id).
fn build_clinics(world: &mut World) {
    let Some(cost) = crate::systems::founding::found_cost(world, BuildingKind::BlackClinic) else { return };
    let spare = world.config.gray.black_clinic_spare;
    for g in world.gangs() {
        let Some(x) = world.comp::<Gang>(g) else { continue };
        let harvest = x.order == crate::components::Order::Harvest || gang_type(world, g).is_some();
        if !harvest || x.treasury < cost + spare || x.members.is_empty() || black_clinic_of(world, g).is_some() {
            continue;
        }
        if crate::systems::creeds::is_purist(world, g) {
            continue;
        }
        let held: Vec<crate::components::DistrictId> =
            crate::systems::gang::held_districts(world, g).into_iter().map(|(d, _)| d).collect();
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| held.contains(&world.district_of_building(l)))
            .min();
        let Some(lot) = lot else { continue };
        let Ok(b) = crate::systems::founding::build_on_lot(world, lot, BuildingKind::BlackClinic, Some(g)) else {
            continue;
        };
        crate::systems::industry::pay_build(world, Some(g), cost, Some(b), true);
        let text = format!(
            "{} opened a {} ({})",
            world.owner_label(Some(g)),
            BuildingKind::BlackClinic.label(),
            world.name_of(b)
        );
        world.push_event(EventKind::Founded, &[g, b], text);
    }
    doc_clinics(world, cost);
    back_rooms(world, cost);
}

/// The pressure pass phase 2 (OPEN-017; `[customers]`): the back room's
/// reliable founder. F2's struck-off doc with the found cost was one a
/// month at 20k (`p_struck_off`), so the trade stood at 0. While fewer back
/// rooms stand than one per `back_room_adults` adults, a founder opens one,
/// one a night: a struck-off doc not working or owning one (the lowest id),
/// else the least lawful licensed Ripperdoc under `rogue_lawful` (not his
/// own Clinic's owner), who quits his Clinic for it and is struck off. He
/// builds on the vacant Lot nearest his Home when he holds the found cost,
/// else refits the derelict Block nearest it (unpaid: a squat with a
/// table and a lamp), and is hired at his own back room as its doc.
fn back_rooms(world: &mut World, cost: i64) {
    let Some(per) = crate::systems::customers::cfg(world).map(|c| (c.back_room_adults.max(1), c.rogue_lawful)) else {
        return;
    };
    let (per, rogue_lawful) = per;
    let Some(role) = world.config.trade_role("black_doc") else { return };
    let adults: usize = world.districts.iter().map(|d| d.residents.len()).sum();
    let want = (adults / per as usize).max(1);
    let standing_n = world.buildings_of_kind(BuildingKind::BlackClinic).iter().filter(|&&b| standing(world, b)).count();
    if standing_n >= want {
        return;
    }
    let works_one = |w: &World, a: EntityId| {
        w.comp::<Job>(a)
            .and_then(|j| j.employer)
            .is_some_and(|e| w.comp::<Building>(e).is_some_and(|bd| bd.kind == BuildingKind::BlackClinic))
            || w.buildings_of_kind(BuildingKind::BlackClinic)
                .iter()
                .any(|&b| standing(w, b) && w.owner_of(b) == Some(a))
    };
    let free = |w: &World, a: EntityId| w.has::<Brain>(a) && !w.has::<Sentence>(a) && crate::systems::law::living(w, a);
    let struck = world.jobs2.gray.struck_off.iter().copied().find(|&a| free(world, a) && !works_one(world, a));
    let (doc, rogue) = match struck {
        Some(a) => (a, None),
        None => {
            let mut pool: Vec<EntityId> = world.workers(Role::Ripperdoc).to_vec();
            pool.sort_unstable();
            let pick = pool
                .into_iter()
                .filter(|&a| free(world, a) && world.comp::<Job>(a).is_some_and(|j| j.role == Role::Ripperdoc))
                .filter(|&a| {
                    let clinic = world.comp::<Job>(a).and_then(|j| j.employer);
                    clinic.is_none_or(|c| world.owner_of(c) != Some(a))
                })
                .filter_map(|a| {
                    let law = world.comp::<Personality>(a).map_or(1.0, |p| p.lawfulness);
                    (law < rogue_lawful).then_some(((law * 1e6) as i64, a))
                })
                .min();
            let Some((_, a)) = pick else { return };
            (a, world.comp::<Job>(a).and_then(|j| j.employer))
        }
    };
    if rogue.is_some() || struck.is_none() {
        world.jobs2.gray.struck_off.insert(doc);
        crate::systems::economy::quit_job(world, doc, "went to work a back room");
        ownership::jobs2_row(world).struck_off += 1;
    }
    let from = world
        .comp::<crate::components::Household>(doc)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|bd| bd.door)
        .or_else(|| world.comp::<crate::components::Position>(doc).map(|p| p.tile));
    let Some(from) = from else { return };
    let paid_lot = (world.purse(Some(doc)) >= cost)
        .then(|| crate::systems::founding::nearest_lot_for(world, BuildingKind::BlackClinic, from))
        .flatten();
    let b = match paid_lot {
        Some(lot) => match crate::systems::founding::build_on_lot(world, lot, BuildingKind::BlackClinic, Some(doc)) {
            Ok(b) => {
                crate::systems::industry::pay_build(world, Some(doc), cost, Some(b), false);
                b
            }
            Err(_) => return,
        },
        None => {
            // scan-ok: daily, one founder a night: the derelicts nearest his Home
            let derelict = crate::systems::street::derelicts(world)
                .into_iter()
                .filter_map(|d| {
                    let bd = world.comp::<Building>(d)?;
                    (bd.kind == BuildingKind::Home && !bd.demolished).then(|| (bd.door.manhattan(from), d))
                })
                .min()
                .map(|(_, d)| d);
            let Some(d) = derelict else { return };
            match crate::systems::founding::refit_with(world, d, BuildingKind::BlackClinic, Some(doc), false) {
                Ok(b) => b,
                Err(_) => return,
            }
        }
    };
    if world.comp::<Job>(doc).is_none() {
        crate::systems::demography::hire(world, doc, b, role);
    }
    ownership::jobs2_row(world).back_rooms += 1;
    let how = match rogue {
        Some(c) => format!(", leaving {}", world.name_of(c)),
        None => " (struck off)".to_string(),
    };
    let text =
        format!("{} opened a {} ({}){how}", world.name_of(doc), BuildingKind::BlackClinic.label(), world.name_of(b));
    world.push_event(EventKind::Founded, &[doc, b], text);
}

/// Pressure pass F2: the other founder of JOBS_AND_LIVES § 7 ("founded by a
/// struck-off doc or a gang"). Only the gang's door was built, and only a
/// Harvest or typed gang with `found_cost + black_clinic_spare` in its
/// treasury and a vacant Lot on held ground opens it: the typed (scav)
/// gangs are memberless with 50 coins and hold nothing, so no BlackClinic
/// stood at 20k or 100k and no Back-Room Doc was ever hired. A struck-off
/// doc who holds the found cost and has no back room to work in (none
/// standing with his post open, none of his own) opens one on the vacant
/// Lot nearest his Home (ascending id, one a night).
fn doc_clinics(world: &mut World, cost: i64) {
    let docs: Vec<EntityId> = world.jobs2.gray.struck_off.iter().copied().collect();
    if docs.is_empty() {
        return;
    }
    let open_post = world.buildings_of_kind(BuildingKind::BlackClinic).iter().any(|&b| {
        standing(world, b)
            && world.vacancies.get(&b).is_some_and(|v| {
                v.iter()
                    .any(|&r| matches!(r, Role::Trade(t) if world.config.trade(t).is_some_and(|row| row.struck_off)))
            })
    });
    if open_post {
        return;
    }
    for a in docs {
        if !world.has::<crate::components::Brain>(a) || world.has::<crate::components::Sentence>(a) {
            continue;
        }
        let at_clinic = world
            .comp::<Job>(a)
            .and_then(|j| j.employer)
            .is_some_and(|e| world.comp::<Building>(e).is_some_and(|bd| bd.kind == BuildingKind::BlackClinic));
        let owns = world
            .buildings_of_kind(BuildingKind::BlackClinic)
            .iter()
            .any(|&b| standing(world, b) && world.owner_of(b) == Some(a));
        if at_clinic || owns || world.comp::<crate::components::Wallet>(a).is_none_or(|w| w.coins < cost) {
            continue;
        }
        let from = world
            .comp::<crate::components::Household>(a)
            .and_then(|h| h.home)
            .and_then(|h| world.comp::<Building>(h))
            .map(|bd| bd.door)
            .or_else(|| world.comp::<crate::components::Position>(a).map(|p| p.tile));
        let Some(lot) =
            from.and_then(|f| crate::systems::founding::nearest_lot_for(world, BuildingKind::BlackClinic, f))
        else {
            continue;
        };
        let Ok(b) = crate::systems::founding::build_on_lot(world, lot, BuildingKind::BlackClinic, Some(a)) else {
            continue;
        };
        crate::systems::industry::pay_build(world, Some(a), cost, Some(b), true);
        let text =
            format!("{} opened a {} ({})", world.name_of(a), BuildingKind::BlackClinic.label(), world.name_of(b));
        world.push_event(EventKind::Founded, &[a, b], text);
        return;
    }
}

/// Midnight: a BlackClinic in a Sweep district or one where the law cracks
/// down on its gang is found on `p_bm_raid`: shut, `BlackMarket` on its
/// staff (else the gang's leader).
fn raids(world: &mut World) {
    let cfg = world.config.gray.clone();
    let now = world.tick;
    for b in world.buildings_of_kind(BuildingKind::BlackClinic).to_vec() {
        if !standing(world, b) || world.is_closed(b) {
            continue;
        }
        let owner = world.owner_of(b);
        let d = world.district_of_building(b);
        let stance = world.districts.get(d.index()).map(|x| x.stance);
        let hot =
            matches!(stance, Some(Stance::Sweep)) || matches!(stance, Some(Stance::Crackdown(g)) if Some(g) == owner);
        if !hot || hash(world, PURPOSE_RAID, key(b), world.day()) >= cfg.p_bm_raid {
            continue;
        }
        if let Some(bd) = world.comp_mut::<Building>(b) {
            bd.closed_until = Some(now + cfg.bm_close_days.max(1) * TICKS_PER_DAY);
        }
        let mut named: Vec<EntityId> = world.staff_of(b).to_vec();
        if named.is_empty() {
            named.extend(owner.and_then(|g| world.comp::<Gang>(g)).and_then(|x| x.leader));
        }
        for s in named.into_iter().take(2) {
            crate::systems::law::file_report(world, Crime::BlackMarket, s, None);
            ownership::jobs2_row(world).bm_reports += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Midnight (from `living::run`): the papers, the struck-off, the clinics,
/// the raids, the organs' rot and sale; the ripped list pruned.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("gray::daily");
    assign_types(world);
    papers_daily(world);
    strike_off(world);
    build_clinics(world);
    raids(world);
    // Jobs & lives phase 8: the day's units to market, then the rot (rot
    // first took a fresh pair's unit before any buyer saw it: 80 of 82
    // units rotted on the 6-seed mean, 2 exported).
    sell_organs(world);
    rot(world);
    let alive: Vec<EntityId> = world.jobs2.gray.ripped.iter().copied().filter(|&b| world.is_alive(b)).collect();
    world.jobs2.gray.ripped = alive.into_iter().collect();
    let held: Vec<EntityId> =
        world.jobs2.gray.papers.keys().copied().filter(|&h| crate::systems::law::living(world, h)).collect();
    world.jobs2.gray.papers.retain(|h, _| held.contains(h));
}

/// The day's end (`systems::stats`): the gray snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let fake = world.jobs2.gray.papers.values().filter(|p| p.fake).count() as u32;
    let papers = world.jobs2.gray.papers.len() as u32;
    let clinics =
        world.buildings_of_kind(BuildingKind::BlackClinic).iter().filter(|&&b| standing(world, b)).count() as u32;
    let mut stock = 0u32;
    for k in [BuildingKind::BlackClinic, BuildingKind::Hideout, BuildingKind::Clinic] {
        for &b in world.buildings_of_kind(k) {
            stock += world.stock(b, Good::Organs);
        }
    }
    let typed = world.gangs().into_iter().filter(|&g| gang_type(world, g).is_some()).count() as u32;
    let row = ownership::jobs2_row(world);
    row.fake_ids = fake;
    row.papers_held = papers;
    row.black_clinics = clinics;
    row.organ_stock = stock;
    row.scav_gangs = typed;
}
