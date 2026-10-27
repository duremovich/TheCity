//! Pressure pass round 2, R3 (docs/PRESSURE_PASS.md § R3, OPEN-216): the
//! wounded and the sick.
//!
//! - **The fee reaches the doctor**: the doctor on the table takes
//!   `doctor_cut` of a Clinic's fee (a patch, a treatment) into his wallet;
//!   the rest goes to the owner's till as before ([`pay_fee`]).
//! - **The short have choices**: a diagnosed patient whose pool falls short
//!   of the Clinic's treatment pawns an owned deck, weapon or vehicle to the
//!   doctor (`pawn_mult` of its value) and, with a wage, goes on the
//!   doctor's slate for the rest (`slate_down` down, `slate_rate` marked up,
//!   an edge debt to the doctor: repaid from the wallet as any debt, and a
//!   lawless doctor's Debt Beat past `[hitmarket] debt_min`); with no wage
//!   it begs a Friend for the money at midnight ([`daily`]), or the back
//!   room's cut rate (phase 2). The hurt short of a patch go on the slate
//!   the same way where no back room is within reach.
//! - **The sick worker's post**: at midnight a sick body's post is covered by
//!   a jobless adult of the employer's district (a day hire on the sick
//!   worker's shifts) when the owner can pay it, else the absence is written.
//! - **The downed** (`wounded.rs` reads [`stranger_term`] and the call): a
//!   stranger at hand or a guard on the beat carries a downed body, the
//!   downed calls one tied body who can reach it in time, and the nearest
//!   staffed doctor (a Clinic or a back room) takes it.
//!
//! The off-screen city: the ask is world-side (any tier); the slate and the
//! pawn are a body's (the Treat goal's); a Statistical patient treats on
//! `p_stat_treat` as before. `[care]` absent: none of it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{Asset, AssetLoc, Brain, Building, BuildingKind, Health, Job, Lod, RelKind, Wallet};
use crate::config::CareCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

const PURPOSE_ASK: u64 = 0xca7e_0001;

pub fn cfg(world: &World) -> Option<&CareCfg> {
    world.config.care.as_ref()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

/// A day cover of a sick worker's post.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cover {
    pub cover: EntityId,
    pub employer: EntityId,
    pub sick: EntityId,
    pub until: Tick,
}

/// R3's saved state: the last day each patient asked for treatment money,
/// the day covers standing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub asked: BTreeMap<EntityId, u64>,
    pub covers: Vec<Cover>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.asked.is_empty() && self.covers.is_empty()
    }
}

fn coins(world: &World, id: EntityId) -> i64 {
    world.comp::<Wallet>(id).map_or(0, |w| w.coins.max(0))
}

// ---------------------------------------------------------------------------
// The fee
// ---------------------------------------------------------------------------

/// `payer` pays `amount` for care at `place`: `doctor_cut` of it to the
/// doctor on the table (`doc`, when he is not the owner), the rest to the
/// owner (`Flow::Treatment`, the place credited). Returns `(paid, to the doctor)`.
pub fn pay_fee(world: &mut World, payer: EntityId, place: EntityId, doc: Option<EntityId>, amount: i64) -> (i64, i64) {
    if amount <= 0 {
        return (0, 0);
    }
    let owner = world.owner_of(place);
    let share = cfg(world).map_or(0.0, |c| c.doctor_cut.clamp(0.0, 1.0));
    let doc = doc.filter(|&d| Some(d) != owner && d != payer && crate::systems::law::living(world, d));
    let cut = match doc {
        Some(_) => ((amount as f32) * share).round() as i64,
        None => 0,
    };
    let to_doc = match doc {
        Some(d) if cut > 0 => ownership::pay(world, Some(payer), Some(d), cut, Flow::Treatment),
        _ => 0,
    };
    let to_owner = ownership::pay(world, Some(payer), owner, amount - to_doc, Flow::Treatment);
    ownership::credit(world, place, to_owner);
    if to_doc > 0 {
        ownership::jobs2_row(world).care_doc_cut += to_doc;
    }
    (to_doc + to_owner, to_doc)
}

// ---------------------------------------------------------------------------
// The short: the pawn, the slate
// ---------------------------------------------------------------------------

/// The best pawnable asset `id` owns (a deck, a weapon or a vehicle, not in
/// use, not financed or stolen, worth something) and what it covers.
pub fn pawnable(world: &World, id: EntityId) -> Option<(EntityId, i64)> {
    let c = cfg(world)?;
    crate::systems::assets::assets_of(world, Some(id))
        .iter()
        .copied()
        .filter_map(|a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter(|(_, x)| {
            x.kind.is_vehicle()
                || matches!(x.kind, crate::components::AssetKind::Deck | crate::components::AssetKind::Weapon)
        })
        .filter(|(_, x)| !x.stolen && x.finance.is_none() && !matches!(x.loc, AssetLoc::InUse(_)) && x.value > 0)
        .map(|(a, x)| (((x.value as f32) * c.pawn_mult) as i64, a))
        .filter(|&(v, _)| v > 0)
        .max_by(|p, q| p.0.cmp(&q.0).then(q.1.cmp(&p.1)))
        .map(|(v, a)| (a, v))
}

/// What `id` can put up at a Clinic: the pool (wallet, kin, gang) and a pawn.
pub fn means(world: &World, id: EntityId) -> i64 {
    crate::systems::health::pool(world, id) + pawnable(world, id).map_or(0, |p| p.1)
}

/// Coins `id` owes `doc` (an edge debt; 0 none).
pub fn owed(world: &World, id: EntityId, doc: EntityId) -> i64 {
    world.edge(id, doc).map_or(0, |e| {
        let (lo, _) = crate::components::edge_key(id, doc);
        i64::from(if lo == id { e.debt } else { -e.debt }).max(0)
    })
}

/// A wage to answer for a slate: a standing post (not a day hire).
fn has_wage(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.employer.is_some() && j.until.is_none() && j.wage_per_day > 0)
}

/// `id` may go on the slate at `clinic` for `short` coins: `[care]` on, a
/// wage, the down in its means, and owing the clinic's staff under
/// `slate_cap` with the new debt.
pub fn slate_ok(world: &World, id: EntityId, clinic: EntityId, price: i64) -> bool {
    let Some(c) = cfg(world) else { return false };
    let m = means(world, id);
    if m >= price {
        return true;
    }
    if !has_wage(world, id) || m < c.slate_down.min(price) {
        return false;
    }
    let short = (((price - m) as f32) * (1.0 + c.slate_rate)).round() as i64;
    let owing: i64 = world.staff_of(clinic).iter().map(|&d| owed(world, id, d)).sum();
    owing + short <= c.slate_cap
}

/// How a fee was settled.
#[derive(Clone, Debug, Default)]
pub struct Settled {
    pub cash: i64,
    pub to_doc: i64,
    pub pawned: Option<(EntityId, i64)>,
    pub slate: i64,
}

impl Settled {
    /// The diary's tail: " (3c to the doctor; pawned a Deck; 120c on his slate)".
    pub fn note(&self, world: &World) -> String {
        let mut bits: Vec<String> = Vec::new();
        if self.to_doc > 0 {
            bits.push(format!("{}c of it to the doctor", self.to_doc));
        }
        if let Some((a, v)) = self.pawned {
            let what = world.comp::<Asset>(a).map_or("an item", |x| x.kind.label());
            bits.push(format!("pawned a {what} for {v}c"));
        }
        if self.slate > 0 {
            bits.push(format!("{}c on the doctor's slate", self.slate));
        }
        if bits.is_empty() {
            String::new()
        } else {
            format!(" ({})", bits.join("; "))
        }
    }
}

/// `id` settles `price` for care at a Clinic `place` with `doc` on the
/// table: the pool in order (the patient, the kin, the gang; the doctor's
/// cut), then a pawn handed to the doctor, then the rest on the doctor's
/// slate, marked up (an edge debt: the patient owes the doctor).
pub fn settle(world: &mut World, id: EntityId, place: EntityId, doc: Option<EntityId>, price: i64) -> Settled {
    let mut s = Settled::default();
    let mut due = price;
    for (payer, holds) in crate::systems::health::payers(world, id) {
        if due <= 0 {
            break;
        }
        let take = holds.min(due);
        if take > 0 {
            let (paid, cut) = pay_fee(world, payer, place, doc, take);
            due -= paid;
            s.cash += paid;
            s.to_doc += cut;
            if payer != id && world.has::<crate::components::Identity>(payer) {
                world.remember(payer, crate::components::MemoryKind::Paid, Some(id), 0.4, -0.1, false);
            }
        }
    }
    let Some(c) = cfg(world).cloned() else { return s };
    let Some(d) = doc.filter(|&d| d != id && crate::systems::law::living(world, d)) else { return s };
    if due > 0 {
        if let Some((a, v)) = pawnable(world, id) {
            crate::systems::assets::reset_title(world, a);
            if world.comp::<Asset>(a).is_some_and(|x| x.loc == AssetLoc::Carried(id)) {
                crate::systems::assets::set_loc(world, a, AssetLoc::Carried(d));
            }
            crate::systems::assets::set_owner(world, a, Some(d));
            due -= v.min(due);
            s.pawned = Some((a, v));
            ownership::jobs2_row(world).care_pawns += 1;
            world.remember(d, crate::components::MemoryKind::Paid, Some(id), 0.4, 0.1, false);
        }
    }
    // The slate is the Clinic's doctor's book (an orderly on the table
    // patches; the Ripperdoc on staff carries the debt).
    let d = slate_holder(world, place, id).unwrap_or(d);
    if due > 0 {
        let debt = (((due as f32) * (1.0 + c.slate_rate)).round() as i64).min(i64::from(i32::MAX / 2));
        let (lo, _) = crate::components::edge_key(id, d);
        let tick = world.tick;
        let e = world.edge_entry(id, d);
        e.debt += if lo == id { debt as i32 } else { -(debt as i32) };
        e.debt_since.get_or_insert(tick);
        e.last_interaction = tick;
        s.slate = debt;
        let row = ownership::jobs2_row(world);
        row.care_slates += 1;
        row.care_slate_coins += debt;
        world.remember(id, crate::components::MemoryKind::Paid, Some(d), 0.6, -0.3, false);
    }
    s
}

/// The slate's holder at `place`: a living Ripperdoc on its staff (on duty
/// first, then the lowest id), not the patient.
fn slate_holder(world: &World, place: EntityId, id: EntityId) -> Option<EntityId> {
    let docs: Vec<EntityId> = world
        .staff_of(place)
        .iter()
        .copied()
        .filter(|&o| o != id && crate::systems::law::living(world, o))
        .filter(|&o| world.comp::<Job>(o).is_some_and(|j| j.role == crate::components::Role::Ripperdoc))
        .collect();
    let tod = world.tick_of_day();
    docs.iter()
        .copied()
        .find(|&o| world.comp::<Job>(o).is_some_and(|j| j.on_shift(tod)))
        .or_else(|| docs.into_iter().min())
}

// ---------------------------------------------------------------------------
// The downed
// ---------------------------------------------------------------------------

/// A stranger `a` at hand for the downed `w` (within `help_radius`, not its
/// attacker, holding no grudge on it): the Rescue tie's term, `stranger_term
/// × (sociability + lawfulness) / 2`.
pub fn stranger_term(world: &World, a: EntityId, w: EntityId, by: Option<EntityId>) -> Option<f32> {
    let c = cfg(world)?;
    if by == Some(a) || !world.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical) {
        return None;
    }
    if !crate::systems::demography::is_adult(world, a) {
        return None;
    }
    if world.edge(a, w).is_some_and(|e| e.affinity < 0.0 || e.kind == RelKind::Enemy) {
        return None;
    }
    let (pa, pw) = (world.comp::<crate::components::Position>(a)?, world.comp::<crate::components::Position>(w)?);
    if crate::systems::law::chebyshev(pa.tile, pw.tile) > c.help_radius {
        return None;
    }
    let p = world.comp::<crate::components::Personality>(a)?;
    Some(c.stranger_term * (p.sociability + p.lawfulness) / 2.0)
}

/// The street hears a body go down: the `help_wake` nearest off-screen
/// adults within `help_radius` of `tile` get a body (Coarse); while it
/// bleeds they keep one ([`bystander`], `lod::class_with`).
pub fn wake_bystanders(world: &mut World, w: EntityId, tile: crate::components::TilePos) {
    let Some(c) = cfg(world).cloned() else { return };
    let mut near: Vec<(u32, EntityId)> = world
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .filter(|&a| a != w && crate::systems::demography::is_adult(world, a) && crate::systems::law::living(world, a))
        .filter_map(|a| world.comp::<crate::components::Position>(a).map(|p| (p, a)))
        .filter(|(p, _)| p.building.is_none() && crate::systems::law::chebyshev(p.tile, tile) <= c.help_radius)
        .map(|(p, a)| (p.tile.manhattan(tile), a))
        .collect();
    near.sort_unstable();
    for (_, a) in near.into_iter().take(c.help_wake as usize) {
        crate::systems::lod::set_lod(world, a, Lod::Coarse);
        ownership::jobs2_row(world).care_woken += 1;
    }
}

/// `id` stands within `help_radius` of a body bleeding (the hourly LOD
/// pass keeps it a body).
pub fn bystander(world: &World, id: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    let Some(p) = world.comp::<crate::components::Position>(id) else { return false };
    world
        .wounded
        .iter()
        .any(|(&w, r)| w != id && !r.stable && crate::systems::law::chebyshev(r.tile, p.tile) <= c.help_radius)
}

/// A guard on the beat for any downed body: a city guard on shift.
pub fn beat_term(world: &World, a: EntityId) -> Option<f32> {
    let c = cfg(world)?;
    (crate::systems::law::is_city_guard(world, a) && crate::systems::law::on_duty(world, a)).then_some(c.beat_term)
}

/// The nearest staffed doctor's door to `tile` for a downed body: a Clinic
/// with a Ripperdoc on staff, or (with `[care]`) an open back room with its
/// doc. `(place, door distance)`.
pub fn doctor_near(world: &World, tile: crate::components::TilePos) -> Option<EntityId> {
    let kinds: &[BuildingKind] =
        if cfg(world).is_some() { &[BuildingKind::Clinic, BuildingKind::BlackClinic] } else { &[BuildingKind::Clinic] };
    let mut best: Option<(u32, EntityId)> = None;
    for &k in kinds {
        for &b in world.buildings_of_kind(k) {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if bd.demolished || bd.derelict || world.is_closed(b) || !staffed(world, b) {
                continue;
            }
            let d = bd.door.manhattan(tile);
            if best.is_none_or(|x| (d, b) < x) {
                best = Some((d, b));
            }
        }
    }
    best.map(|x| x.1)
}

/// A doctor on `place`'s staff (on call): a Ripperdoc at a Clinic, any
/// living staff at a back room.
pub fn staffed(world: &World, place: EntityId) -> bool {
    let back = world.comp::<Building>(place).is_some_and(|b| b.kind == BuildingKind::BlackClinic);
    world.staff_of(place).iter().any(|&o| {
        crate::systems::law::living(world, o)
            && (back || world.comp::<Job>(o).is_some_and(|j| j.role == crate::components::Role::Ripperdoc))
    })
}

/// The downed `w` calls for help (every `call_every_min` while unclaimed):
/// the best tied body (kin, a Friend, a gang-mate, a colleague; the highest
/// affinity) who can reach it and the doctor before it bleeds out (a tile a
/// tick), not already on a Rescue. Returns the callee.
pub fn call_for_help(world: &mut World, w: EntityId) -> Option<EntityId> {
    let every = Tick::from(cfg(world)?.call_every_min.max(1));
    let rec = world.wounded.get(&w)?.clone();
    if rec.stable || rec.rescuer.is_some() || world.tick.saturating_sub(rec.since) % every != 0 {
        return None;
    }
    let at = world.comp::<crate::components::Position>(w).map_or(rec.tile, |p| p.tile);
    let doc = rec.clinic?;
    let to_doc = world.comp::<Building>(doc).map_or(u32::MAX, |b| b.door.manhattan(at));
    let left = rec.bleed_until.saturating_sub(world.tick);
    let gang = world.gang_of(w);
    let employer = world.comp::<Job>(w).and_then(|j| j.employer);
    let mut best: Option<(f32, EntityId)> = None;
    for o in world.neighbours(w) {
        if Some(o) == rec.by || crate::systems::wounded::is_down(world, o) || !crate::systems::law::living(world, o) {
            continue;
        }
        let Some(e) = world.edge(w, o) else { continue };
        let tied = matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend)
            || (gang.is_some() && world.gang_of(o) == gang)
            || (employer.is_some() && world.comp::<Job>(o).and_then(|j| j.employer) == employer);
        if !tied || e.affinity < 0.0 {
            continue;
        }
        if world.comp::<Brain>(o).is_none_or(|b| {
            b.lod == Lod::Statistical
                || b.cuffed_by.is_some()
                || b.plan_goal() == Some(crate::components::GoalKind::Rescue)
        }) || world.has::<crate::components::Sentence>(o)
        {
            continue;
        }
        let Some(p) = world.comp::<crate::components::Position>(o) else { continue };
        let reach = Tick::from(p.tile.manhattan(at)) + Tick::from(to_doc);
        if reach > left {
            continue;
        }
        if best.is_none_or(|(a, b)| e.affinity > a || (e.affinity == a && o < b)) {
            best = Some((e.affinity, o));
        }
    }
    let (_, o) = best?;
    if let Some(x) = world.wounded.get_mut(&w) {
        x.called = Some(o);
    }
    let (wn, on) = (world.name_of(w), world.name_of(o));
    world.push_event(EventKind::Wounded, &[w, o], format!("{wn}, bleeding, called {on} for help"));
    ownership::jobs2_row(world).care_calls += 1;
    Some(o)
}

// ---------------------------------------------------------------------------
// Midnight: begging for it, the sick worker's post
// ---------------------------------------------------------------------------

/// Midnight, after the sick roll (`health::daily`): the diagnosed short of
/// the treatment ask a Friend for the money; the sick bodies' posts covered
/// or the absence written.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    asks(world, &c);
    covers(world, &c);
}

/// A diagnosed patient whose pool falls short of the Clinic's treatment
/// asks (once in `ask_days`) the warmest Friend at or above `ask_affinity`
/// with coins above `ask_keep` for the shortfall; he lends on `affinity ×
/// (1 − greed / 2)` what he holds above `ask_keep`, up to the shortfall (an
/// edge debt), or turns him down. Kin already pay through the pool.
fn asks(world: &mut World, c: &CareCfg) {
    let price = crate::systems::health::treat_price(world);
    let day = world.day();
    let short: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&id| crate::systems::health::saving_short(world, id) && !crate::systems::wounded::is_down(world, id))
        .collect();
    world.care.asked.retain(|_, d| day.saturating_sub(*d) < 30);
    for id in short {
        if world.care.asked.get(&id).is_some_and(|&d| day.saturating_sub(d) < u64::from(c.ask_days.max(1))) {
            continue;
        }
        if world.has::<crate::components::Sentence>(id) {
            continue;
        }
        let need = price - crate::systems::health::pool(world, id);
        if need <= 0 {
            continue;
        }
        let friend = world
            .neighbours(id)
            .filter(|&o| crate::systems::law::living(world, o) && coins(world, o) > c.ask_keep)
            .filter_map(|o| {
                world
                    .edge(id, o)
                    .filter(|e| e.kind == RelKind::Friend && e.affinity >= c.ask_affinity)
                    .map(|e| (e.affinity, o))
            })
            .max_by(|p, q| p.0.total_cmp(&q.0).then(q.1.cmp(&p.1)));
        let Some((aff, f)) = friend else { continue };
        world.care.asked.insert(id, day);
        let greed = world.comp::<crate::components::Personality>(f).map_or(0.5, |p| p.greed);
        let yes = hash(world, PURPOSE_ASK, id, day) < aff * (1.0 - greed / 2.0);
        let (who, fname) = (world.name_of(id), world.name_of(f));
        if !yes {
            world.remember(id, crate::components::MemoryKind::Rejected, Some(f), 0.5, -0.4, false);
            world.push_event(
                EventKind::Loan,
                &[id, f],
                format!("{who} begged {fname} for {need}c toward a vat-cancer treatment; {fname} turned them down"),
            );
            ownership::jobs2_row(world).care_asks_refused += 1;
            continue;
        }
        let lend = (coins(world, f) - c.ask_keep).min(need);
        if lend <= 0 {
            continue;
        }
        if let Some(w) = world.comp_mut::<Wallet>(f) {
            w.coins -= lend;
        }
        if let Some(w) = world.comp_mut::<Wallet>(id) {
            w.coins += lend;
        }
        let (lo, _) = crate::components::edge_key(id, f);
        let tick = world.tick;
        let e = world.edge_entry(id, f);
        e.debt += if lo == id { lend as i32 } else { -(lend as i32) };
        e.debt_since.get_or_insert(tick);
        e.last_interaction = tick;
        world.remember(f, crate::components::MemoryKind::Paid, Some(id), 0.5, 0.1, false);
        world.remember(id, crate::components::MemoryKind::Paid, Some(f), 0.6, 0.3, false);
        world.push_event(
            EventKind::Loan,
            &[id, f],
            format!("{who} begged {fname} for {need}c toward a vat-cancer treatment; {fname} lent {lend}c"),
        );
        let row = ownership::jobs2_row(world);
        row.care_asks_lent += 1;
        row.care_ask_coins += lend;
    }
}

/// The sick bodies' posts: each sick-today body (a standing post at
/// another's business) is covered by the nearest jobless lawful adult living
/// in the employer's district, hired for the day on the sick worker's
/// shifts, when the owner's purse holds `cover_purse_days` of the role's
/// wage; else the absence is written. The covers ended at their `until`
/// ([`hourly`]).
fn covers(world: &mut World, c: &CareCfg) {
    let sick: Vec<EntityId> =
        world.citizens().into_iter().filter(|&id| world.comp::<Health>(id).is_some_and(|h| h.sick_today)).collect();
    let now = world.tick;
    for id in sick {
        let Some(job) = world.comp::<Job>(id).cloned() else { continue };
        let Some(emp) = job.employer else { continue };
        if job.until.is_some() || world.owner_of(emp) == Some(id) || world.has::<crate::components::Sentence>(id) {
            continue;
        }
        if world.comp::<Building>(emp).is_none_or(|b| b.demolished || b.derelict) {
            continue;
        }
        let days = world.comp::<Health>(id).map_or(1, |h| h.sick_days.count_ones());
        let owner = world.owner_of(emp);
        let wage = job.wage_per_day.max(1);
        let pick =
            if world.purse(owner) >= wage * c.cover_purse_days { cover_pick(world, c, emp, job.role) } else { None };
        let (who, at, role) = (world.name_of(id), world.name_of(emp), job.role.label());
        let Some(cv) = pick else {
            world.push_event(
                EventKind::Skipped,
                &[id, emp],
                format!("{who} is off sick from {at} ({days} sick days in 14); nobody covers the {role}'s post"),
            );
            ownership::jobs2_row(world).care_uncovered += 1;
            continue;
        };
        crate::systems::demography::hire(world, cv, emp, job.role);
        let until = now + TICKS_PER_DAY;
        if let Some(j) = world.comp_mut::<Job>(cv) {
            j.until = Some(until);
            j.shifts = job.shifts.clone();
        }
        world.care.covers.push(Cover { cover: cv, employer: emp, sick: id, until });
        let cn = world.name_of(cv);
        world.push_event(
            EventKind::Skipped,
            &[id, cv, emp],
            format!("{who} is off sick from {at} ({days} sick days in 14); {cn} covers the {role}'s shift for the day"),
        );
        world.remember(cv, crate::components::MemoryKind::Served, Some(id), 0.3, 0.1, false);
        ownership::jobs2_row(world).care_covered += 1;
    }
}

/// The cover for a post at `emp`: a jobless, free, lawful adult body living
/// in its district (no gang, not down), admitted to the role's band, the
/// nearest to the door.
fn cover_pick(world: &World, c: &CareCfg, emp: EntityId, role: crate::components::Role) -> Option<EntityId> {
    let door = world.comp::<Building>(emp)?.door;
    let d = world.district_of_building(emp);
    let band = crate::systems::careers::hire_band(world, role);
    world
        .districts
        .get(d.index())?
        .residents
        .iter()
        .copied()
        .filter(|&a| !world.has::<Job>(a) && crate::systems::demography::is_adult(world, a))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| b.cuffed_by.is_none()))
        .filter(|&a| crate::systems::law::living(world, a) && !world.has::<crate::components::Sentence>(a))
        .filter(|&a| world.gang_of(a).is_none() && !crate::systems::wounded::is_down(world, a))
        .filter(|&a| world.comp::<crate::components::Personality>(a).is_some_and(|p| p.lawfulness >= c.cover_lawful))
        .filter(|&a| crate::systems::careers::admits(world, a, band))
        .filter(|&a| !world.has::<crate::components::Child>(a))
        .filter_map(|a| world.comp::<crate::components::Position>(a).map(|p| (p.tile.manhattan(door), a)))
        .min()
        .map(|x| x.1)
}

/// Hourly (from `wounded::run`): the covers past their `until` end (the Job
/// gone, no layoff: the day was the post).
pub fn hourly(world: &mut World) {
    if cfg(world).is_none() || world.care.covers.is_empty() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let now = world.tick;
    let due: Vec<Cover> = world.care.covers.iter().filter(|c| c.until <= now).cloned().collect();
    world.care.covers.retain(|c| c.until > now);
    for c in due {
        let still =
            world.comp::<Job>(c.cover).is_some_and(|j| j.employer == Some(c.employer) && j.until == Some(c.until));
        if still {
            world.abort_plan(c.cover);
            world.remove::<Job>(c.cover);
        }
    }
}

// ---------------------------------------------------------------------------
// The founder
// ---------------------------------------------------------------------------

/// Coins a diagnosed founder keeps out of a founding: the Clinic's
/// treatment while the cancer stands (0 with `[care]` absent or none).
pub fn founder_reserve(world: &World, id: EntityId) -> i64 {
    if cfg(world).is_none() {
        return 0;
    }
    let diagnosed = world.comp::<Health>(id).and_then(|h| h.cancer).is_some_and(|c| c.diagnosed);
    if diagnosed {
        crate::systems::health::treat_price(world)
    } else {
        0
    }
}
