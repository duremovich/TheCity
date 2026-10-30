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
    /// Round 3 S4: `(patient, friend, day)` a Friend turned the patient down
    /// (not asked again inside `ask_refused_days`).
    pub refused: Vec<(EntityId, EntityId, u64)>,
    /// Round 5 U6 (OPEN-217): the off-screen chrome booked at midnight for
    /// an hour of the clinic's day (`book`, kept by `keep_bookings`).
    pub booked: Vec<Booking>,
    /// Round 7 W6 (OPEN-217): a Clinic -> its patients since midnight (the
    /// day's idle count, `clinic_idle_days`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub seen: BTreeMap<EntityId, u32>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.asked.is_empty()
            && self.covers.is_empty()
            && self.refused.is_empty()
            && self.booked.is_empty()
            && self.seen.is_empty()
    }
}

/// Round 5 U6: an off-screen patient's visit, booked at `clinic` for the
/// hour `at` (a tick of today's clinic day).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Booking {
    pub patient: EntityId,
    pub clinic: EntityId,
    pub visit: Visit,
    pub at: Tick,
}

/// Round 5 U6: what a booking is for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Visit {
    /// `stat_shop`'s chrome, the pick bought and installed at the visit.
    Install(crate::components::ShopPick),
    /// The blood washed (`[care] wash_*`).
    Wash,
}

const PURPOSE_BOOK: u64 = 0xca7e_0002;
const PURPOSE_WASH: u64 = 0xca7e_0003;

/// Round 5 U6 (OPEN-217; `[care] book_hours`): `stat_shop`'s chrome is
/// booked, not installed at 00:00: the patient comes to `clinic` at an hour
/// of `book_hours` (a hash of the patient and the day), when the doctor is
/// at the counter. `false` with booking off (the base rule installs in
/// place). V13: 34 installs a week on one night orderly, 5-6 every 00:00.
pub fn book(world: &mut World, patient: EntityId, clinic: EntityId, pick: &crate::components::ShopPick) -> bool {
    book_visit(world, patient, clinic, Visit::Install(pick.clone()))
}

fn book_visit(world: &mut World, patient: EntityId, clinic: EntityId, visit: Visit) -> bool {
    let Some([from, to]) = cfg(world).map(|c| c.book_hours) else { return false };
    if to < from || to >= 24 {
        return false;
    }
    if world.care.booked.iter().any(|b| b.patient == patient) {
        return true;
    }
    // Round 7 W6 (OPEN-217): a visit the clinic's own stock does not tie
    // (a wash, a new implant) goes where the book is lightest nearby.
    let free = match &visit {
        Visit::Wash => true,
        Visit::Install(pick) => pick.used.is_none(),
    };
    let clinic = if free { book_clinic(world, patient, clinic) } else { clinic };
    let span = u64::from(to - from) + 1;
    let day = world.day();
    let h = u64::from(from) + ((hash(world, PURPOSE_BOOK, patient, day) * span as f32) as u64).min(span - 1);
    let at = day * TICKS_PER_DAY + h * TICKS_PER_HOUR;
    world.care.booked.push(Booking { patient, clinic, visit, at });
    ownership::jobs2_row(world).clinic_booked += 1;
    true
}

/// Round 7 W6 (OPEN-217; `[care] book_slack_ticks`): the Clinic a booked
/// visit goes to: among the standing Clinics with a Ripperdoc on staff
/// within `book_slack_ticks` of travel more than the nearest such from the
/// patient's Home, the fewest bookings today per Ripperdoc (the nearest on
/// ties); `proposed` (the nearest seller) without a Home or the key. Every
/// pick was the nearest: a Clinic with nearer ones all round saw nobody
/// (V15 `ripperdoc_9880` a week; 20k `ripperdoc_11516`, Ripperdoc#5892, no
/// wash in 30 days), and V6's spill on a doctor's hour (`[[serves]]
/// per_hour` 4) never binds at the city's few patients an hour.
fn book_clinic(world: &World, patient: EntityId, proposed: EntityId) -> EntityId {
    let Some((slack, take)) = cfg(world).map(|c| (c.book_slack_ticks, c.book_cands)).filter(|&(s, _)| s > 0) else {
        return proposed;
    };
    let Some(from) = crate::systems::contracts::home_door(world, patient) else { return proposed };
    let docs = |b: EntityId| {
        world
            .staff_of(b)
            .iter()
            .filter(|&&o| {
                crate::systems::law::living(world, o)
                    && !world.has::<crate::components::Sentence>(o)
                    && world.comp::<Job>(o).is_some_and(|j| j.role == crate::components::Role::Ripperdoc)
            })
            .count()
    };
    // The nearest few by the door's tiles, then by travel (a pick a booking).
    let mut by_tiles: Vec<(u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .filter(|&b| !world.is_closed(b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .collect();
    by_tiles.sort_unstable();
    let cands: Vec<(u64, usize, EntityId)> = by_tiles
        .into_iter()
        .filter_map(|(_, b)| {
            let n = docs(b);
            let door = world.comp::<Building>(b)?.door;
            (n > 0).then(|| (crate::systems::distance::travel(world, from, door), n, b))
        })
        .take(take.max(1))
        .collect();
    let Some(near) = cands.iter().map(|c| c.0).min() else { return proposed };
    let load = |b: EntityId| world.care.booked.iter().filter(|x| x.clinic == b).count();
    cands
        .into_iter()
        .filter(|c| c.0 <= near + slack)
        // fewest booked per doctor: load / n compared as load x n' < load' x n
        .min_by(|x, y| (load(x.2) * y.1).cmp(&(load(y.2) * x.1)).then(x.0.cmp(&y.0)).then(x.2.cmp(&y.2)))
        .map_or(proposed, |c| c.2)
}

/// Round 7 W6 (OPEN-217): a patient arrived at `place` (`service::arrive`).
pub fn note_seen(world: &mut World, place: EntityId) {
    if cfg(world).is_none() || !world.comp::<Building>(place).is_some_and(|b| b.kind == BuildingKind::Clinic) {
        return;
    }
    *world.care.seen.entry(place).or_default() += 1;
}

/// Round 7 W6: midnight's tally of the day's Clinics: standing and open
/// with a Ripperdoc on staff (`clinic_doc_days`), and of those, the ones
/// that saw no patient (`clinic_idle_days`).
fn tally_clinics(world: &mut World) {
    let seen = std::mem::take(&mut world.care.seen);
    let (mut doc, mut idle) = (0u32, 0u32);
    for &b in world.buildings_of_kind(BuildingKind::Clinic) {
        if !world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict) || world.is_closed(b) {
            continue;
        }
        if !staffed(world, b) {
            continue;
        }
        doc += 1;
        if seen.get(&b).copied().unwrap_or(0) == 0 {
            idle += 1;
        }
    }
    let row = ownership::jobs2_row(world);
    row.clinic_doc_days += doc;
    row.clinic_idle_days += idle;
}

/// Round 5 U6: the hour's bookings kept (from `hourly`): one due this hour
/// at a Clinic open now (its doctor at the counter) is installed by the
/// doctor the counter gives him (`chrome::stat_install`; a body first,
/// `[care] clinic_body_first`); one at a shut Clinic goes to the nearest
/// Clinic open now, else waits for the next hour of `book_hours`, and past
/// the last is let go (counted booked, not kept), as is a patient held,
/// dead or become a body.
fn keep_bookings(world: &mut World) {
    if world.care.booked.is_empty() {
        return;
    }
    let Some([_, to]) = cfg(world).map(|c| c.book_hours) else { return };
    let now = world.tick;
    let end = now - now % TICKS_PER_DAY + (u64::from(to) + 1) * TICKS_PER_HOUR;
    let due: Vec<Booking> = world.care.booked.iter().filter(|b| b.at < now + TICKS_PER_HOUR).cloned().collect();
    if due.is_empty() {
        return;
    }
    world.care.booked.retain(|b| b.at >= now + TICKS_PER_HOUR);
    for b in due {
        let gone = !crate::systems::law::living(world, b.patient)
            || world.has::<crate::components::Sentence>(b.patient)
            || world.comp::<Brain>(b.patient).is_none_or(|x| x.lod != Lod::Statistical);
        if gone {
            continue;
        }
        // A shut Clinic at the hour: the nearest one open now (a used
        // implant of the booked Clinic's stock waits for it), else the next
        // hour (the first 20k run: 1,679 booked, 811 kept, installs 1,275 ->
        // 1,006 against the midnight's in-place install).
        let used = matches!(&b.visit, Visit::Install(pick) if pick.used.is_some());
        let clinic = if crate::systems::assets::seller_open(world, b.clinic) {
            Some(b.clinic)
        } else if used {
            None
        } else {
            crate::systems::contracts::home_door(world, b.patient)
                .and_then(|from| crate::systems::assets::nearest_seller(world, BuildingKind::Clinic, from, true))
        };
        let Some(clinic) = clinic else {
            if now + TICKS_PER_HOUR < end {
                world.care.booked.push(Booking { at: now + TICKS_PER_HOUR, ..b });
            }
            continue;
        };
        let kept = match &b.visit {
            Visit::Install(pick) => crate::systems::chrome::stat_install(world, b.patient, clinic, pick),
            Visit::Wash => wash(world, b.patient, clinic),
        };
        if kept {
            ownership::jobs2_row(world).clinic_kept += 1;
        }
    }
}

/// Round 5 U6 (OPEN-217; `[care] wash_*`): the clinic's everyday trade.
/// At midnight an off-screen adult carrying `wash_min_tox` or more of
/// toxicity (the synth diet's, the bins') and holding `wash_price` above
/// `wash_keep` books a wash on `p_wash` (a hash of him and the day) at the
/// Clinic nearest his Home, for an hour of `book_hours`. V13: a clinic's
/// trade was the chrome alone, 25 Clinics at 20k a median of 5 patients a
/// week; the island's toxicity is everyone's (8,259 of 20k over 0.4).
/// Adults only: a child is never a patient of the pass (nothing here is a
/// harm; the fee is an adult's).
fn book_washes(world: &mut World, c: &CareCfg) {
    if c.p_wash <= 0.0 || c.wash_price <= 0 {
        return;
    }
    let day = world.day();
    // scan-ok: daily: the off-screen toxic who can pay
    let due: Vec<EntityId> = world
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .filter(|&id| hash(world, PURPOSE_WASH, id, day) < c.p_wash)
        .filter(|&id| world.comp::<Health>(id).is_some_and(|h| h.toxicity >= c.wash_min_tox))
        .filter(|&id| coins(world, id) >= c.wash_price + c.wash_keep)
        .filter(|&id| {
            crate::systems::demography::is_adult(world, id)
                && !crate::systems::ages::is_protected(world, id)
                && !world.has::<crate::components::Sentence>(id)
        })
        .collect();
    for id in due {
        let Some(from) = crate::systems::contracts::home_door(world, id) else { continue };
        let Some(clinic) = crate::systems::assets::nearest_seller(world, BuildingKind::Clinic, from, false) else {
            continue;
        };
        book_visit(world, id, clinic, Visit::Wash);
    }
}

/// Round 5 U6: the wash at the counter: `wash_price` paid (`pay_fee`, the
/// doctor's `doctor_cut`), `wash_frac` of the toxicity taken out, the line
/// naming the doctor the counter gave him (a body first), the encounter.
fn wash(world: &mut World, patient: EntityId, clinic: EntityId) -> bool {
    let Some(c) = cfg(world).cloned() else { return false };
    if coins(world, patient) < c.wash_price {
        return false;
    }
    let doc = crate::systems::service::server_for(world, clinic, crate::components::ServeKind::Patient, patient);
    let (paid, to_doc) = pay_fee(world, patient, clinic, doc, c.wash_price);
    if paid <= 0 {
        return false;
    }
    if let Some(h) = world.comp_mut::<Health>(patient) {
        h.toxicity = (h.toxicity * (1.0 - c.wash_frac.clamp(0.0, 1.0))).clamp(0.0, 1.0);
    }
    let (who, at) = (world.name_of(patient), world.name_of(clinic));
    let by = doc.map_or_else(String::new, |d| format!(" by {}", world.name_of(d)));
    let cut = if to_doc > 0 { format!(", {to_doc}c of it to the doctor") } else { String::new() };
    let text = format!("{who} had the blood washed at {at}{by} ({paid}c{cut})");
    let actors: Vec<EntityId> = [Some(patient), Some(clinic), doc].into_iter().flatten().collect();
    world.push_event(EventKind::Treated, &actors, text);
    crate::systems::service::arrive(world, patient, clinic, crate::components::ServeKind::Patient, paid);
    ownership::jobs2_row(world).clinic_washes += 1;
    true
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

/// Round 7 W4 (OPEN-245; `[care] install_cut`): the doctor who cut an
/// implant worth `value` at `place` takes `install_cut` of it from the
/// owner's till (`Flow::Treatment`; nothing when he owns the place or the
/// till is dry). V15: 36 installs a week and no coin to the one named.
pub fn pay_install(world: &mut World, place: EntityId, doc: EntityId, value: i64) -> i64 {
    let Some(share) = cfg(world).map(|c| c.install_cut.clamp(0.0, 1.0)) else { return 0 };
    let cut = ((value.max(0) as f32) * share).round() as i64;
    let owner = world.owner_of(place);
    if cut <= 0 || owner.is_none() || owner == Some(doc) || !crate::systems::law::living(world, doc) {
        return 0;
    }
    let paid = ownership::pay(world, owner, Some(doc), cut, Flow::Treatment);
    ownership::credit(world, place, -paid);
    if paid > 0 {
        ownership::jobs2_row(world).care_doc_cut += paid;
    }
    paid
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
    // Round 7 W3: the one it called, wherever he is.
    world.wounded.iter().any(|(&w, r)| {
        w != id
            && !r.stable
            && (r.called == Some(id) || crate::systems::law::chebyshev(r.tile, p.tile) <= c.help_radius)
    })
}

/// A guard on the beat for any downed body: a city guard on shift.
pub fn beat_term(world: &World, a: EntityId) -> Option<f32> {
    let c = cfg(world)?;
    (crate::systems::law::is_city_guard(world, a) && crate::systems::law::on_duty(world, a)).then_some(c.beat_term)
}

/// Round 6 V6 (OPEN-217; `[care] orderly_opens`): `clinic` is open for a
/// patch-up: its Ripperdoc at the counter (`assets::seller_open`), or any of
/// its patient staff on duty (the night orderly). A sale, an install and a
/// treatment still wait for the Ripperdoc (`seller_open`).
pub fn open_for_patch(world: &World, clinic: EntityId) -> bool {
    if crate::systems::assets::seller_open(world, clinic) {
        return true;
    }
    cfg(world).is_some_and(|c| c.orderly_opens)
        && world.comp::<Building>(clinic).is_some_and(|bd| !bd.demolished && !bd.derelict)
        && !world.is_closed(clinic)
        && !crate::systems::service::servers_on_duty(world, clinic, crate::components::ServeKind::Patient).is_empty()
}

/// Round 6 V6 (`[care] clinic_spill`): a doctor at `place` would see
/// `patient` this hour (one on duty or on call with room left).
pub fn room_at(world: &World, place: EntityId, patient: EntityId) -> bool {
    crate::systems::customers::doctor_for(world, place, patient)
        .is_some_and(|d| crate::systems::service::has_room(world, d))
}

/// Round 6 V6 (OPEN-217): the Clinic a wound walks to from `from`: the
/// nearest open for a patch-up ([`open_for_patch`]) whose doctor has room
/// this hour ([`room_at`], `[care] clinic_spill`), else the nearest open.
/// The nearest open alone took every patient, queue or none, and every
/// Clinic was shut from 21:00 to 09:00 with its orderly on.
pub fn patch_clinic(world: &World, from: crate::components::TilePos, patient: EntityId) -> Option<EntityId> {
    let table = world.near_table(BuildingKind::Clinic);
    if cfg(world).is_some_and(|c| c.clinic_spill) {
        let roomy = table.nearest_passing(from, |b| open_for_patch(world, b) && room_at(world, b, patient));
        if roomy.is_some() {
            return roomy;
        }
    }
    table.nearest_passing(from, |b| open_for_patch(world, b))
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
///
/// Pressure pass round 7, W3 (`OPEN-244`): a callee who has not come by the
/// next call is passed over (`Wounded.tried`; a bleeding 61-y called the same
/// colleague three times while he worked on). With no tied body able, the
/// call reaches the nearest stranger who can make it in time: a city guard on
/// shift or the doctor's own staff on duty within `call_radius`, or an adult
/// passer-by within `help_radius`. When nobody can, the line says why, once
/// a turn of the calls (the first call, or the one after a callee failed).
pub fn call_for_help(world: &mut World, w: EntityId) -> Option<EntityId> {
    let c = cfg(world)?.clone();
    let every = Tick::from(c.call_every_min.max(1));
    let rec = world.wounded.get(&w)?.clone();
    if rec.stable || rec.rescuer.is_some() || world.tick.saturating_sub(rec.since) % every != 0 {
        return None;
    }
    // Round 7 W3: on someone's back (a carrier whose claim lapsed) he calls nobody.
    if world.comp::<Brain>(w).and_then(|b| b.cuffed_by).is_some_and(|c| c != w) {
        return None;
    }
    let first = world.tick.saturating_sub(rec.since) < every;
    let wpos = world.comp::<crate::components::Position>(w).cloned();
    let at = wpos.as_ref().map_or(rec.tile, |p| p.tile);
    let Some(doc) = rec.clinic else {
        if first {
            let wn = world.name_of(w);
            world.push_event(
                EventKind::Wounded,
                &[w],
                format!("{wn}, bleeding, called for help: no doctor open in reach"),
            );
        }
        return None;
    };
    let to_doc = world.comp::<Building>(doc).map_or(u32::MAX, |b| b.door.manhattan(at));
    let left = rec.bleed_until.saturating_sub(world.tick);
    // Round 7 W3: the last callee did not come (no claim): passed over now.
    let mut tried = rec.tried.clone();
    let failed = rec.called;
    if let Some(f) = failed {
        if !tried.contains(&f) {
            tried.push(f);
        }
    }
    let gang = world.gang_of(w);
    let employer = world.comp::<Job>(w).and_then(|j| j.employer);
    // Free to answer: alive, on its feet, not held, not already on a Rescue,
    // not the attacker, not passed over. An off-screen callee is given a body
    // below (at 20k most of the street within reach was Statistical: the
    // first call found nobody).
    let free = |world: &World, o: EntityId| {
        o != w
            && Some(o) != rec.by
            && !tried.contains(&o)
            && !crate::systems::wounded::is_down(world, o)
            && crate::systems::law::living(world, o)
            && !world.has::<crate::components::Sentence>(o)
            && world
                .comp::<Brain>(o)
                .is_some_and(|b| b.cuffed_by.is_none() && b.plan_goal() != Some(crate::components::GoalKind::Rescue))
    };
    let reach_of = |world: &World, o: EntityId| {
        world
            .comp::<crate::components::Position>(o)
            .map(|p| (p.tile.manhattan(at), Tick::from(p.tile.manhattan(at)) + Tick::from(to_doc)))
    };
    let mut best: Option<(f32, EntityId)> = None;
    for o in world.neighbours(w) {
        if !free(world, o) {
            continue;
        }
        let Some(e) = world.edge(w, o) else { continue };
        let tied = matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend)
            || (gang.is_some() && world.gang_of(o) == gang)
            || (employer.is_some() && world.comp::<Job>(o).and_then(|j| j.employer) == employer);
        if !tied || e.affinity < 0.0 {
            continue;
        }
        let Some((_, reach)) = reach_of(world, o) else { continue };
        if reach > left {
            continue;
        }
        if best.is_none_or(|(a, b)| e.affinity > a || (e.affinity == a && o < b)) {
            best = Some((e.affinity, o));
        }
    }
    // Round 7 W3: no tied body able: the nearest stranger who can come.
    let mut stranger: Option<(&'static str, u32)> = None;
    if best.is_none() && c.call_radius > 0 {
        let tod = world.tick_of_day();
        let r = c.call_radius.max(c.help_radius);
        let building = wpos.as_ref().and_then(|p| p.building);
        let near = world.agents_near(at, r, building);
        let mut pick: Option<(u32, EntityId, &'static str)> = None;
        for o in near {
            if !free(world, o) || !crate::systems::demography::is_adult(world, o) {
                continue;
            }
            if world.edge(w, o).is_some_and(|e| e.affinity < 0.0 || e.kind == RelKind::Enemy) {
                continue;
            }
            let Some(p) = world.comp::<crate::components::Position>(o) else { continue };
            let cheb = crate::systems::law::chebyshev(p.tile, at);
            let what = if crate::systems::law::is_city_guard(world, o) && crate::systems::law::on_duty(world, o) {
                (cheb <= c.call_radius).then_some("a guard on the beat")
            } else if world.comp::<Job>(o).is_some_and(|j| j.employer == Some(doc) && j.on_shift(tod)) {
                (cheb <= c.call_radius).then_some("the doctor's own staff")
            } else if cheb <= c.help_radius || (building.is_some() && p.building == building) {
                Some("a passer-by")
            } else {
                None
            };
            let Some(what) = what else { continue };
            let Some((d, reach)) = reach_of(world, o) else { continue };
            if reach > left {
                continue;
            }
            if pick.is_none_or(|(bd, bo, _)| (d, o) < (bd, bo)) {
                pick = Some((d, o, what));
            }
        }
        if let Some((d, o, what)) = pick {
            best = Some((0.0, o));
            stranger = Some((what, d));
        }
    }
    let wn = world.name_of(w);
    let gone = failed.map(|f| format!(" ({} never came)", world.name_of(f))).unwrap_or_default();
    let Some((_, o)) = best else {
        if let Some(x) = world.wounded.get_mut(&w) {
            x.called = None;
            x.tried = tried;
        }
        // Once a turn: the first call, or the one after a callee failed.
        if first || failed.is_some() {
            world.push_event(
                EventKind::Wounded,
                &[w],
                format!("{wn}, bleeding, called for help{gone}: nobody in reach could get them to a doctor in the {left} min left"),
            );
        }
        return None;
    };
    if let Some(x) = world.wounded.get_mut(&w) {
        x.called = Some(o);
        x.tried = tried;
    }
    // The callee keeps a body while the call stands (`bystander`).
    if world.comp::<Brain>(o).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, o, Lod::Coarse);
    }
    let on = world.name_of(o);
    let text = match stranger {
        Some((what, d)) => format!("{wn}, bleeding, called for help{gone}: {on}, {what} {d} tiles off, heard"),
        None => format!("{wn}, bleeding, called {on} for help{gone}"),
    };
    world.push_event(EventKind::Wounded, &[w, o], text);
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
    tally_clinics(world);
    asks(world, &c);
    covers(world, &c);
    book_washes(world, &c);
}

/// A diagnosed patient whose pool falls short of the Clinic's treatment
/// (his own and his kin's coins, the pool, already in) asks his own for the
/// rest once in `ask_days`: round 3 S4, a ladder of up to `ask_tries`
/// Friends at or above `ask_affinity`, warmest first, until the shortfall is
/// covered. One with coins above `ask_keep` lends on `affinity × (1 −
/// greed / 2)` what he holds above it (an edge debt) or turns him down (not
/// asked again for `ask_refused_days`); one with nothing to spare says so.
/// The night is one line naming each. The street (begging, Earn) comes after.
fn asks(world: &mut World, c: &CareCfg) {
    let price = crate::systems::health::treat_price(world);
    let day = world.day();
    let short: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&id| crate::systems::health::saving_short(world, id) && !crate::systems::wounded::is_down(world, id))
        .collect();
    world.care.asked.retain(|_, d| day.saturating_sub(*d) < 30);
    let keep = u64::from(c.ask_refused_days);
    world.care.refused.retain(|&(_, _, d)| day.saturating_sub(d) < keep);
    for id in short {
        if world.care.asked.get(&id).is_some_and(|&d| day.saturating_sub(d) < u64::from(c.ask_days.max(1))) {
            continue;
        }
        if world.has::<crate::components::Sentence>(id) {
            continue;
        }
        let in_pool = crate::systems::health::pool(world, id);
        let mut need = price - in_pool;
        if need <= 0 {
            continue;
        }
        let refused = |w: &World, o: EntityId| w.care.refused.iter().any(|&(p, f, _)| p == id && f == o);
        let mut friends: Vec<(f32, EntityId)> = world
            .neighbours(id)
            .filter(|&o| crate::systems::law::living(world, o) && !refused(world, o))
            .filter_map(|o| {
                world
                    .edge(id, o)
                    .filter(|e| e.kind == RelKind::Friend && e.affinity >= c.ask_affinity)
                    .map(|e| (e.affinity, o))
            })
            .collect();
        friends.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.1.cmp(&q.1)));
        friends.truncate(c.ask_tries.max(1) as usize);
        world.care.asked.insert(id, day);
        let asked_for = need;
        // The pool named: his own coins, then each of his kin's (the spouse
        // first) and his gang's, as `health::payers` takes them.
        let pool: Vec<String> = crate::systems::health::payers(world, id)
            .into_iter()
            .map(|(p, c)| if p == id { format!("{c}c of their own") } else { format!("{} {c}c", world.name_of(p)) })
            .collect();
        if friends.is_empty() {
            // Nobody of his own left to ask: the line says so (the street,
            // Earn's begging and scavenging, is what is left).
            let who = world.name_of(id);
            let text = format!(
                "{who}, {asked_for}c short of a vat-cancer treatment after the {in_pool}c they and theirs could put in ({}), has no friend left to ask",
                pool.join(", ")
            );
            world.push_event(EventKind::Loan, &[id], text);
            continue;
        }
        let mut bits: Vec<String> = Vec::new();
        let mut actors = vec![id];
        for (k, (aff, f)) in friends.into_iter().enumerate() {
            if need <= 0 {
                break;
            }
            actors.push(f);
            let fname = world.name_of(f);
            let spare = coins(world, f) - c.ask_keep;
            if spare <= 0 {
                bits.push(format!("{fname} had nothing to spare"));
                continue;
            }
            let greed = world.comp::<crate::components::Personality>(f).map_or(0.5, |p| p.greed);
            let roll = hash(world, PURPOSE_ASK, id, day.wrapping_mul(8).wrapping_add(k as u64));
            if roll >= aff * (1.0 - greed / 2.0) {
                world.remember(id, crate::components::MemoryKind::Rejected, Some(f), 0.5, -0.4, false);
                world.care.refused.push((id, f, day));
                bits.push(format!("{fname} turned them down"));
                ownership::jobs2_row(world).care_asks_refused += 1;
                continue;
            }
            let lend = spare.min(need);
            if let Some(w) = world.comp_mut::<Wallet>(f) {
                w.coins -= lend;
            }
            if let Some(w) = world.comp_mut::<Wallet>(id) {
                w.coins += lend;
            }
            need -= lend;
            let (lo, _) = crate::components::edge_key(id, f);
            let tick = world.tick;
            let e = world.edge_entry(id, f);
            e.debt += if lo == id { lend as i32 } else { -(lend as i32) };
            e.debt_since.get_or_insert(tick);
            e.last_interaction = tick;
            world.remember(f, crate::components::MemoryKind::Paid, Some(id), 0.5, 0.1, false);
            world.remember(id, crate::components::MemoryKind::Paid, Some(f), 0.6, 0.3, false);
            bits.push(format!("{fname} lent {lend}c"));
            let row = ownership::jobs2_row(world);
            row.care_asks_lent += 1;
            row.care_ask_coins += lend;
        }
        let who = world.name_of(id);
        let tail = if need <= 0 { ": covered".to_string() } else { format!(": still {need}c short") };
        let text = format!(
            "{who}, {asked_for}c short of a vat-cancer treatment after the {in_pool}c they and theirs could put in ({}), begged their friends: {}{tail}",
            pool.join(", "),
            bits.join("; ")
        );
        world.push_event(EventKind::Loan, &actors, text);
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
        // Round 5 U1: a `Served` memory is the service section's (the off
        // city remembers none, `service`'s `test_off_city_writes_nothing`;
        // U1's seed moved a cover onto day 1 there).
        if crate::systems::service::on(world) {
            world.remember(cv, crate::components::MemoryKind::Served, Some(id), 0.3, 0.1, false);
        }
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
        // Round 4 T6: a corp's exec holds no Job and is not jobless
        // (`exec_78363 d26 00:00`: an exec hired as a sick man's Engineer).
        .filter(|&a| !crate::systems::founding::is_exec(world, a))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| b.cuffed_by.is_none()))
        .filter(|&a| crate::systems::law::living(world, a) && !world.has::<crate::components::Sentence>(a))
        .filter(|&a| world.gang_of(a).is_none() && !crate::systems::wounded::is_down(world, a))
        .filter(|&a| world.comp::<crate::components::Personality>(a).is_some_and(|p| p.lawfulness >= c.cover_lawful))
        .filter(|&a| crate::systems::careers::admits(world, a, band))
        // Round 8 X3 (`OPEN-247`): a post that reads a record.
        .filter(|&a| crate::systems::careers::record_ok(world, a, role))
        .filter(|&a| !world.has::<crate::components::Child>(a))
        .filter_map(|a| world.comp::<crate::components::Position>(a).map(|p| (p.tile.manhattan(door), a)))
        .min()
        .map(|x| x.1)
}

/// Hourly (from `wounded::run`): the covers past their `until` end (the Job
/// gone, no layoff: the day was the post).
pub fn hourly(world: &mut World) {
    if cfg(world).is_none() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    // Round 3 S4 (OPEN-216): the off-screen patients seen where a body goes.
    crate::systems::customers::stat_care(world);
    keep_bookings(world);
    if world.care.covers.is_empty() {
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
