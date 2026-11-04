//! M16b § 4 (docs/M16_CONTRACTS.md; phase 1): leverage and coercion.
//!
//! A game abstraction between fictional agents of a simulated city: a
//! "coercion" is a record in `World::coercions` (who holds what over whom,
//! and the demand), made by one seeded Intimidate (or, for a Parley, a
//! Persuade) roll through M15's resolver, `moves::resolve_with`, with the
//! leverage and the demand's cost added to the actor's skill term. Every
//! payment is coins moved between purses by `ownership::pay`
//! (`Flow::Coerced`), inside the coin identity; nothing is minted.
//!
//! Phase 1 (without hostages): the shakedown routed through the move (the
//! same roll), protection, threats (a coerced job, Silence, poaching by
//! threat), Debt, Parley, blackmail with secret tallies and lapsing, fraud
//! (the renege's filing, the fake job) and the Court suitor's `Persuade`.
//! `[leverage] enabled = false` is M16a's city.
//!
//! Rolls: the move's own `WordNs::Move` draw; every other draw is keyed on
//! `ContractNs::Leverage` with a purpose byte (`Purpose`), never the world
//! stream. Nothing here runs per agent per tick: the passes are daily (a
//! gang's weekly pass on its own day), the standing coercions are few.

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{Building, BuildingKind, Crime, Identity, Job, Memory, Personality, Sentence, Skills};
use crate::contract::{
    Coercion, CoercionId, CoercionStatus, CoercionWhy, ContractId, ContractKind, ContractNs, ContractStatus, Demand,
    FakeJob, Leverage, LeverageKind, Origin, PostExtra, Posting, Target,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::systems::{contracts, law, moves, reputation};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::word::{Deed, DeedRef, MoveKind, MoveOutcome, SocialMove, Stake};
use crate::world::World;

mod coercion;
mod fraud;
mod protection;
mod secrets;

pub use coercion::*;
pub use fraud::*;
pub use protection::*;
pub use secrets::*;

/// Is the switch in flight on?
pub fn on(world: &World) -> bool {
    world.config.leverage.enabled
}

/// The purpose byte of a `ContractNs::Leverage` draw (`a`'s top byte).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Purpose {
    /// A failed move's report, key `(tick, over << 32 | agent)`.
    Report = 1,
    /// A fake-job candidate's day, key `(day, faker)`.
    Fake = 2,
}

/// The one keyed stream of M16b's rolls: `ContractNs::Leverage` with the
/// purpose byte in `a`'s top byte (the low 56 bits `a`'s), `b` the second
/// key. Every module's purposes are listed in `M16_CONTRACTS.md`.
pub(crate) fn leverage_rng(world: &World, purpose: u8, a: u64, b: u64) -> rand_chacha::ChaCha8Rng {
    let key = (u64::from(purpose) << 56) | (a & ((1 << 56) - 1));
    world.rng.contract(ContractNs::Leverage, key, b)
}

/// One uniform draw off [`leverage_rng`].
pub(crate) fn leverage_draw(world: &World, purpose: u8, a: u64, b: u64) -> f32 {
    leverage_rng(world, purpose, a, b).random()
}

fn draw(world: &World, p: Purpose, a: u64, b: u64) -> f32 {
    leverage_draw(world, p as u8, a, b)
}

/// The tried-pair purposes (`World::lev_tried`'s third key).
pub mod tried {
    pub const PROTECT: u8 = 1;
    pub const BLACKMAIL: u8 = 2;
    pub const SILENCE: u8 = 3;
    pub const PARLEY: u8 = 4;
    pub const JOB: u8 = 5;
    pub const FAKE: u8 = 6;
    pub const DEBT: u8 = 7;
    pub const COURT: u8 = 8;
    pub const HOSTAGE: u8 = 9;
    pub const FABRICATE: u8 = 10;
    /// M16b phase 4: a board campaign's try on a swing member.
    pub const VOTE: u8 = 11;
}

fn retry(world: &World) -> Tick {
    Tick::from(world.config.leverage.retry_days) * TICKS_PER_DAY
}

/// Was `(a, b, k)` tried within `retry_days`?
pub fn tried_recently(world: &World, a: EntityId, b: EntityId, k: u8) -> bool {
    world.lev_tried.get(&(a, b, k)).is_some_and(|&t| world.tick.saturating_sub(t) < retry(world))
}

pub(crate) fn mark(world: &mut World, a: EntityId, b: EntityId, k: u8) {
    let now = world.tick;
    world.lev_tried.insert((a, b, k), now);
}

// ---------------------------------------------------------------------------
// The move
// ---------------------------------------------------------------------------

/// A demand's worth in coins (`demand_cost`'s numerator): a weekly demand
/// is read as a month of it; a coerced job at its kind's list price; a
/// poach at a week of the post's wage; withdrawn testimony at nothing.
fn demand_value(world: &World, d: &Demand) -> f32 {
    match *d {
        Demand::Coins { amount, weekly } => (if weekly { 4 * amount } else { amount }) as f32,
        Demand::Do(id) => world.contracts.get(&id).map_or(0, |c| world.config.contracts.price_base.get(c.kind)) as f32,
        Demand::Join { wage, .. } => (7 * wage) as f32,
        Demand::Silence { .. } | Demand::Release(_) | Demand::Vote { .. } => 0.0,
        // M16b phase 2: a building is worth its value.
        Demand::Sell(b) => world.comp::<Building>(b).map_or(0, |bd| ownership::value(world, bd.kind)) as f32,
    }
}

/// The purse and a week's wage of whoever pays (`payer`, else `over`).
fn means(world: &World, over: EntityId, payer: Option<EntityId>) -> f32 {
    let who = payer.unwrap_or(over);
    let wage = world.comp::<Job>(over).map_or(0, |j| j.wage_per_day);
    (world.purse(Some(who)).max(0) + 7 * wage) as f32
}

/// How far the deed of a secret has gone: the highest reach of a pool
/// entry naming it, over every district (0 when nobody talks of it).
pub fn reach(world: &World, deed: Deed, actor: EntityId, object: Option<EntityId>) -> f32 {
    world
        .rumours
        .iter()
        .flat_map(|p| p.entries.iter())
        .filter(|e| e.deed == deed && e.actor == Some(actor) && e.object == object)
        .map(|e| e.reach)
        .fold(0.0f32, f32::max)
}

/// Spec § 4: the leverage and demand terms added to the actor's skill:
/// `lev_bias[leverage] − demand_w × value ÷ max(purse + 7 × wage, 1)`,
/// plus a Secret's `secret_w × deed_sev × (1 − reach)`. A one-off coin
/// demand up to twice `extort_amount` costs nothing (the shakedown: with
/// `lev_bias.threat` 0 it is M15's roll, the same `p`).
pub fn extra(world: &World, over: EntityId, payer: Option<EntityId>, lev: &Leverage, demand: &Demand) -> f32 {
    let cfg = &world.config.leverage;
    let mut x = cfg.lev_bias.get(lev.kind());
    let free = matches!(*demand, Demand::Coins { amount, weekly: false }
        if amount <= 2 * world.config.social.extort_amount);
    if !free {
        let v = demand_value(world, demand);
        if v > 0.0 {
            x -= cfg.demand_w * v / means(world, over, payer).max(1.0);
        }
    }
    if let Leverage::Secret { deed, actor, object, .. } = *lev {
        let sev = world.config.gossip.deed_sev.get(deed);
        x += cfg.secret_w * sev * (1.0 - reach(world, deed, actor, object));
    }
    // M16b phase 2: a hostage weighs by what the captive is to the coerced
    // (M15's `rel_w`; a stranger's life at the Friend floor).
    if let Leverage::Hostage { captive } = *lev {
        let w = crate::systems::grudges::relation(world, over, Some(captive)).map_or(0.2, |(w, _)| w);
        x += cfg.hostage_w * w;
    }
    x
}

fn stake_of(agent: EntityId, demand: &Demand) -> Stake {
    match *demand {
        Demand::Coins { amount, .. } => Stake::Coins(amount),
        Demand::Join { building, wage } => Stake::Job { building, wage },
        Demand::Do(_) | Demand::Silence { .. } | Demand::Sell(_) | Demand::Release(_) | Demand::Vote { .. } => {
            Stake::Info { about: agent }
        }
    }
}

/// The shakedown (M8's Home extortion, M15 W27's Intimidate): routed
/// through the leverage terms, which are 0 for its demand, so it is the
/// same roll and the same `p` (gang income holds). Off: M15's call.
pub fn shakedown(world: &mut World, actor: EntityId, target: EntityId, amount: i64) -> MoveOutcome {
    let m = SocialMove { actor, target, kind: MoveKind::Intimidate, stake: Stake::Coins(amount) };
    if !on(world) {
        return moves::resolve(world, &m);
    }
    let x = extra(world, target, None, &Leverage::Threat, &Demand::Coins { amount, weekly: false });
    moves::resolve_with(world, &m, x)
}

/// `leverage::coerce`'s roll (spec § 4): one Intimidate by `agent` on
/// `over` with the leverage and demand terms, counted by leverage kind; a
/// failure may be reported by the coerced (`backlash_report ×
/// lawfulness`: Blackmail on a secret, else a Shakedown), on top of M15's
/// backlash. The caller makes the demand happen on success.
pub fn roll(
    world: &mut World,
    agent: EntityId,
    over: EntityId,
    payer: Option<EntityId>,
    lev: &Leverage,
    demand: &Demand,
) -> MoveOutcome {
    let m = SocialMove { actor: agent, target: over, kind: MoveKind::Intimidate, stake: stake_of(agent, demand) };
    let x = extra(world, over, payer, lev, demand);
    let out = moves::resolve_with(world, &m, x);
    world.stats.current.leverage.note_try(lev.kind(), out.success);
    if lev.kind() == LeverageKind::Hostage {
        let c = &mut world.stats.current.captive;
        c.lev_hostage_tries += 1;
        c.lev_hostage_ok += u32::from(out.success);
    }
    if !out.success {
        report_failure(world, agent, over, lev);
    }
    out
}

/// M16b phase 2: a coercion recorded without a roll and without a first
/// payment (a hostage's: the demand is rolled daily by `captives`).
#[allow(clippy::too_many_arguments)]
pub fn open(
    world: &mut World,
    holder: Option<EntityId>,
    agent: EntityId,
    over: EntityId,
    lev: Leverage,
    demand: Demand,
    why: CoercionWhy,
    payer: Option<EntityId>,
) -> CoercionId {
    record(world, holder, agent, over, lev, demand, why, payer, None)
}

/// M16b phase 2: close a coercion from outside (a hostage freed or killed).
pub fn finish(world: &mut World, id: CoercionId, status: CoercionStatus, why: &str) {
    close(world, id, status, why);
}

/// A failed move's report: the coerced files with `backlash_report ×
/// lawfulness` (a keyed draw).
fn report_failure(world: &mut World, agent: EntityId, over: EntityId, lev: &Leverage) {
    if !law::living(world, over) || world.has::<Sentence>(over) || law::is_guard(world, agent) {
        return;
    }
    let law = world.comp::<Personality>(over).map_or(0.5, |p| p.lawfulness);
    let p = world.config.leverage.backlash_report * law;
    let key = (u64::from(over.index) << 32) | u64::from(agent.index);
    if draw(world, Purpose::Report, world.tick, key) >= p {
        return;
    }
    let crime = if matches!(lev, Leverage::Secret { .. }) { Crime::Blackmail } else { Crime::Extortion };
    law::file_report(world, crime, agent, Some(over));
}

/// M16b phase 4 (the Inspector): a demand in words.
pub fn demand_text(world: &World, d: &Demand) -> String {
    match *d {
        Demand::Coins { amount, weekly: true } => format!("{amount} a week"),
        Demand::Coins { amount, weekly: false } => format!("{amount} coins"),
        Demand::Do(cid) => {
            let k = world.contracts.get(&cid).map_or("job", |c| c.kind.label());
            format!("a {k} for nothing")
        }
        Demand::Join { wage, .. } => format!("a post at {wage}/day"),
        Demand::Silence { .. } => "the report withdrawn".to_string(),
        Demand::Sell(b) => format!("{} sold", world.name_of(b)),
        Demand::Release(c) => format!("{} released", world.name_of(c)),
        Demand::Vote { corp, for_ } => {
            format!("a vote for {} on {}'s board", world.name_of(for_), world.owner_label(Some(corp)))
        }
    }
}

/// Standing coercions at once (the cap's count).
pub fn active_count(world: &World) -> usize {
    world.coercions.values().filter(|c| c.is_active()).count()
}

fn room(world: &World) -> bool {
    active_count(world) < world.config.leverage.max_coercions
}

/// Record a coercion made (`Active`), its `Coerced` event.
#[allow(clippy::too_many_arguments)]
fn record(
    world: &mut World,
    holder: Option<EntityId>,
    agent: EntityId,
    over: EntityId,
    lev: Leverage,
    demand: Demand,
    why: CoercionWhy,
    payer: Option<EntityId>,
    building: Option<EntityId>,
) -> CoercionId {
    let now = world.tick;
    let id = world.next_coercion.max(1);
    world.next_coercion = id + 1;
    let weekly = matches!(demand, Demand::Coins { weekly: true, .. });
    let c = Coercion {
        id,
        holder,
        agent,
        over,
        leverage: lev,
        demand,
        since: now,
        until: Tick::MAX,
        next_due: if weekly { now } else { Tick::MAX },
        paid: 0,
        status: CoercionStatus::Active,
        why,
        payer,
        building,
        closed: None,
        missed: 0,
        punished: 0,
        refusing: false,
    };
    world.coercions.insert(id, c);
    let what = match demand {
        Demand::Coins { weekly: true, .. } => format!("{} of {}", why.label(), demand_text(world, &demand)),
        d => demand_text(world, &d),
    };
    let on_what = building.map(|b| format!(" on {}", world.name_of(b))).unwrap_or_default();
    let text =
        format!("{} leaned on {} ({}): {what}{on_what}", world.name_of(agent), world.name_of(over), lev.kind().label());
    let actors = [agent, over, holder.unwrap_or(EntityId::NONE)];
    world.push_event(EventKind::Coerced, &actors, text);
    id
}

/// A coercion made without the roll (the tests; M16b phase 4's god
/// `Coerce` is this call): recorded `Active`, a weekly demand's first week
/// paid now.
#[allow(clippy::too_many_arguments)]
pub fn force(
    world: &mut World,
    holder: Option<EntityId>,
    agent: EntityId,
    over: EntityId,
    lev: Leverage,
    demand: Demand,
    why: CoercionWhy,
    payer: Option<EntityId>,
    building: Option<EntityId>,
) -> CoercionId {
    let id = record(world, holder, agent, over, lev, demand, why, payer, building);
    match demand {
        Demand::Coins { weekly: true, .. } => {
            pay_week(world, id);
        }
        // Review finding 3: a forced one-off sum is paid now (or refused).
        Demand::Coins { amount, weekly: false } => {
            let from = payer.unwrap_or(over);
            if world.purse(Some(from)) < amount {
                close(world, id, CoercionStatus::Refused, "would not pay");
            } else {
                ownership::pay(world, Some(from), holder, amount, Flow::Coerced);
                close(world, id, CoercionStatus::Complied, "paid");
            }
        }
        // A vote closes at the vote, a hostage's hold and a job with
        // their own; any other forced demand stands `retry_days`.
        Demand::Vote { .. } | Demand::Do(_) => {}
        _ if why == CoercionWhy::Hostage => {}
        _ => {
            let until = world.tick + retry(world);
            if let Some(c) = world.coercions.get_mut(&id) {
                c.until = until;
            }
        }
    }
    id
}

/// Review finding 3, daily before `prune`: a coercion past its `until`
/// stood its term (`Complied`); a `Vote` coercion whose corp lost its board
/// (or is gone), or whose member is dead, is `Broken`.
fn validity(world: &mut World) {
    let now = world.tick;
    let mut done: Vec<(CoercionId, CoercionStatus, &'static str)> = Vec::new();
    for c in world.coercions.values().filter(|c| c.is_active()) {
        if c.until <= now {
            done.push((c.id, CoercionStatus::Complied, "its term ran"));
        } else if let Demand::Vote { corp, .. } = c.demand {
            if crate::systems::governance::board_of(world, corp).is_none() || !law::living(world, c.over) {
                done.push((c.id, CoercionStatus::Broken, "the board is gone"));
            }
        }
    }
    for (id, st, why) in done {
        close(world, id, st, why);
    }
}

/// Close a coercion (the one writer of a closing status).
fn close(world: &mut World, id: CoercionId, status: CoercionStatus, why: &str) {
    let now = world.tick;
    let Some(c) = world.coercions.get_mut(&id) else { return };
    if !c.is_active() {
        return;
    }
    c.status = status;
    c.closed = Some(now);
    let c = c.clone();
    match status {
        CoercionStatus::Refused | CoercionStatus::Broken => {
            world.stats.current.leverage.coercions_broken += 1;
            let text = format!(
                "{}'s {} over {} ended: {} ({why})",
                world.name_of(c.agent),
                c.why.label(),
                world.name_of(c.over),
                status.label()
            );
            world.push_event(EventKind::CoercionBroken, &[c.over, c.agent], text);
            // The pair is not tried again for `retry_days`, keyed as the
            // pass that tries it checks it (review finding 5).
            let (by, target, k) = retry_pair(&c);
            mark(world, by, target, k);
        }
        CoercionStatus::Lapsed => {
            world.stats.current.leverage.secrets_lapsed += 1;
            let text =
                format!("{}'s hold over {} lapsed: the secret is out", world.name_of(c.agent), world.name_of(c.over));
            world.push_event(EventKind::Lapsed, &[c.agent, c.over], text);
        }
        CoercionStatus::Complied | CoercionStatus::Active => {}
    }
}

/// The `(a, b, purpose)` a closed coercion cools, as its trying pass reads
/// it: protection `(gang, building)`, a blackmail or a vote `(holder,
/// over)`, a Parley `(payer gang, holder gang)`, a coerced job `(head,
/// gun)`.
pub fn retry_pair(c: &Coercion) -> (EntityId, EntityId, u8) {
    let by = c.holder.unwrap_or(c.agent);
    match c.why {
        CoercionWhy::Protection => (by, c.building.unwrap_or(c.over), tried::PROTECT),
        CoercionWhy::Blackmail => (by, c.over, tried::BLACKMAIL),
        CoercionWhy::Parley => (c.payer.unwrap_or(c.agent), by, tried::PARLEY),
        CoercionWhy::Job | CoercionWhy::Other => (c.agent, c.over, tried::JOB),
        CoercionWhy::Hostage => (by, c.over, tried::HOSTAGE),
        CoercionWhy::Vote => (by, c.over, tried::VOTE),
    }
}

/// A free living person (no Sentence, not cuffed).
fn free(world: &World, id: EntityId) -> bool {
    contracts::free_adult(world, id)
}

// ---------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------

/// The leverage slot (after `contracts`): the midnight passes and the
/// day-end snapshot. Off: nothing.
pub fn run(world: &mut World) {
    if !on(world) {
        return;
    }
    let tod = u64::from(crate::time::tick_of_day(world.tick));
    if crate::systems::midnight::due(world, crate::systems::midnight::Mid::Tail) {
        daily(world);
    }
    if tod == TICKS_PER_DAY - 1 {
        snapshot(world);
    }
}

fn daily(world: &mut World) {
    collect(world);
    lapse(world);
    fake_due(world);
    protection_pass(world);
    blackmail_pass(world);
    parley_pass(world);
    court_pass(world);
    fake_pass(world);
    validity(world);
    prune(world);
}

fn snapshot(world: &mut World) {
    let mut by = [0u32; 4];
    for c in world.coercions.values().filter(|c| c.is_active()) {
        let i = match c.why {
            CoercionWhy::Protection => 1,
            CoercionWhy::Blackmail => 2,
            CoercionWhy::Parley => 3,
            CoercionWhy::Job | CoercionWhy::Other | CoercionWhy::Hostage | CoercionWhy::Vote => 0,
        };
        by[i] += 1;
    }
    let row = &mut world.stats.current.leverage;
    row.coercions_active = by.iter().sum();
    row.protection_active = by[1];
    row.blackmail_active = by[2];
    row.tribute_active = by[3];
}

/// Closed coercions drop `retry_days` after closing; stale tries drop.
fn prune(world: &mut World) {
    let now = world.tick;
    let keep = retry(world);
    world.coercions.retain(|_, c| c.closed.is_none_or(|t| now.saturating_sub(t) < keep));
    world.lev_tried.retain(|_, &mut t| now.saturating_sub(t) < keep);
}
