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

// ---------------------------------------------------------------------------
// Standing coercions: the weekly coins
// ---------------------------------------------------------------------------

/// Does the protection's gang still hold the building's district, and does
/// the building still have the owner it was levied on?
fn protection_holds(world: &World, c: &Coercion) -> bool {
    let (Some(g), Some(b)) = (c.holder, c.building) else { return false };
    if !world.has::<crate::components::Gang>(g) {
        return false;
    }
    let d = world.district_of_building(b);
    let held = if crate::systems::protection::on(world) {
        crate::systems::protection::ground(world, g).binary_search(&d).is_ok()
    } else {
        crate::systems::gang::held_districts(world, g).iter().any(|&(x, _)| x == d)
    };
    held && payer_of(world, b) == c.payer
}

/// Whose purse a business's protection comes out of: its owner, or for a
/// gang front on the city's deed (L2), the gang it fronts for.
pub fn payer_of(world: &World, b: EntityId) -> Option<EntityId> {
    let bd = world.comp::<Building>(b)?;
    bd.owner.or_else(|| bd.venue.as_ref().and_then(|v| v.front_of).filter(|&g| world.has::<crate::components::Gang>(g)))
}

/// Daily: every standing weekly coercion due pays, or ends: a payer short
/// refuses (`Refused`); a protection whose gang lost the district, or
/// whose building changed hands, a blackmail or tribute whose holder is
/// gone, or a coerced party jailed or dead, is `Broken`.
fn collect(world: &mut World) {
    let now = world.tick;
    let due: Vec<CoercionId> = world
        .coercions
        .values()
        .filter(|c| c.is_active() && c.next_due <= now && matches!(c.demand, Demand::Coins { weekly: true, .. }))
        .map(|c| c.id)
        .collect();
    for id in due {
        // The pre-view pass phase 6 (P13): a protection whose gang's leader
        // has a body is collected on his round, in person; midnight only
        // ends a hold that is gone.
        let in_person = world.coercions.get(&id).is_some_and(|c| {
            c.why == CoercionWhy::Protection && c.holder.is_some_and(|g| crate::systems::leaders::collects(world, g))
        });
        if in_person {
            if broken_week(world, id) {
                close(world, id, CoercionStatus::Broken, "the hold is gone");
            }
            continue;
        }
        pay_week(world, id);
    }
}

/// The pre-view pass phase 6 (P13): one owed protection week collected in
/// person at the leader's round stop (the owner met there): paid, or
/// ended as at midnight. The coins paid (0: refused or broken).
pub fn collect_in_person(world: &mut World, id: CoercionId) -> i64 {
    pay_week(world, id)
}

/// A standing coercion whose hold is gone (see [`pay_week`]).
fn broken_week(world: &World, id: CoercionId) -> bool {
    let Some(c) = world.coercions.get(&id) else { return false };
    let gone = |e: EntityId| !law::living(world, e) || world.has::<Sentence>(e);
    match c.why {
        // The pressure pass phase 3: the hold is the gang's; the leaner
        // jailed or dead does not end it.
        CoercionWhy::Protection if crate::systems::protection::on(world) => !protection_holds(world, c) || gone(c.over),
        CoercionWhy::Protection => !protection_holds(world, c) || gone(c.over) || gone(c.agent),
        CoercionWhy::Parley => {
            c.holder.is_none_or(|h| !world.has::<crate::components::Gang>(h))
                || c.payer.is_none_or(|p| !world.has::<crate::components::Gang>(p))
        }
        _ => c.holder.is_none_or(|h| holder_gone(world, h)) || !law::living(world, c.over),
    }
}

/// A holder gone: a gang or a corp that no longer exists, else a person
/// not living (review finding 1: a faction holder is no person).
fn holder_gone(world: &World, h: EntityId) -> bool {
    if world.has::<crate::components::Gang>(h) || world.has::<crate::components::Corp>(h) {
        return false;
    }
    !law::living(world, h)
}

/// One standing coercion's week: paid, or ended (see [`collect`]). The
/// coins paid.
fn pay_week(world: &mut World, id: CoercionId) -> i64 {
    let now = world.tick;
    let Some(c) = world.coercions.get(&id).cloned() else { return 0 };
    let Demand::Coins { amount, .. } = c.demand else { return 0 };
    if !c.is_active() {
        return 0;
    }
    if broken_week(world, id) {
        close(world, id, CoercionStatus::Broken, "the hold is gone");
        return 0;
    }
    let payer = c.payer.unwrap_or(c.over);
    if world.purse(Some(payer)) < amount || c.refusing {
        // The pressure pass phase 3: a protection week the till cannot meet
        // (or the owner will not) is a miss (punished at `punish_misses`),
        // tried again tomorrow.
        if c.why == CoercionWhy::Protection && crate::systems::protection::on(world) {
            if let Some(x) = world.coercions.get_mut(&id) {
                x.next_due = x.next_due.max(now) + TICKS_PER_DAY;
            }
            crate::systems::protection::miss(world, id);
            return 0;
        }
        close(world, id, CoercionStatus::Refused, "would not pay");
        return 0;
    }
    let paid = ownership::pay(world, Some(payer), c.holder, amount, Flow::Coerced);
    if let Some(b) = c.building {
        // M11 D20: a corp owner books it as a loss (its brain may Secure).
        ownership::note_loss(world, b, paid, Some(c.agent));
    }
    if let Some(x) = world.coercions.get_mut(&id) {
        x.paid = x.paid.saturating_add(1);
        x.next_due = now + 7 * TICKS_PER_DAY;
    }
    if c.why == CoercionWhy::Protection {
        let row = &mut world.stats.current.leverage;
        row.protection_paid += 1;
        row.protection_coins += paid;
        // The pressure pass phase 3: the protection joins the week's take
        // the leader's Collect shares out to the members.
        if crate::systems::protection::on(world) {
            if let Some(g) = c.holder.filter(|&g| world.has::<crate::components::Gang>(g)) {
                crate::systems::leisure::note_tribute(world, g, paid);
            }
            if let Some(x) = world.coercions.get_mut(&id) {
                x.missed = 0;
            }
        }
    }
    paid
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

/// The kinds a gang levies protection on (spec § 4, L2's venues included).
pub const PROTECTED: [BuildingKind; 11] = [
    BuildingKind::Bar,
    BuildingKind::Market,
    BuildingKind::Hotel,
    BuildingKind::Clinic,
    BuildingKind::Fixer,
    BuildingKind::Club,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Lounge,
];

/// The week's revenue a building took (the last seven days and today).
fn week_revenue(b: &Building) -> i64 {
    b.revenue.iter().sum::<i64>() + b.revenue_today
}

/// Who answers for a business: an agent owner itself, a corp's exec, a
/// gang's leader (a rival's front); living and free.
pub(crate) fn answers_for(world: &World, owner: EntityId) -> Option<EntityId> {
    let who = match ownership::owner_kind(world, Some(owner)) {
        OwnerKind::Agent(a) => Some(a),
        OwnerKind::Corp(c) => world.comp::<crate::components::Corp>(c).and_then(|x| x.exec),
        OwnerKind::Gang(g) => world.comp::<crate::components::Gang>(g).and_then(|x| x.leader),
        OwnerKind::City => None,
    }?;
    (law::living(world, who) && !world.has::<Sentence>(who)).then_some(who)
}

/// The business a gang would levy in district `d`: the highest week's
/// revenue (ties the lower id) of the protected kinds owned by an agent, a
/// corp with no Security cover there, or another gang; not derelict or
/// closed, not paying anyone already, not tried by this gang lately, and
/// taking at least the weekly levy.
pub fn protection_target(world: &World, gang: EntityId, d: crate::components::DistrictId) -> Option<EntityId> {
    let paying: std::collections::BTreeSet<EntityId> = world
        .coercions
        .values()
        .filter(|c| c.is_active() && c.why == CoercionWhy::Protection)
        .filter_map(|c| c.building)
        .collect();
    let weekly = world.config.leverage.protection_weekly;
    let mut best: Option<(i64, EntityId)> = None;
    for kind in PROTECTED {
        for &b in world.buildings_of_kind(kind) {
            if world.district_of_building(b) != d || paying.contains(&b) {
                continue;
            }
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if bd.demolished || bd.derelict || world.is_closed(b) {
                continue;
            }
            let Some(owner) = payer_of(world, b).filter(|&o| o != gang) else { continue };
            let ok = match ownership::owner_kind(world, Some(owner)) {
                OwnerKind::Agent(_) | OwnerKind::Gang(_) => true,
                OwnerKind::Corp(_) => bd.secured_by.is_none(),
                OwnerKind::City => false,
            };
            // Never a front of the same gang.
            let own_front = bd.venue.as_ref().is_some_and(|v| v.front_of == Some(gang));
            if !ok || own_front {
                continue;
            }
            let rev = week_revenue(bd);
            // The pressure pass phase 3: a levy sized to the business.
            let short = if crate::systems::protection::on(world) {
                crate::systems::protection::levy(world, rev, weekly).is_none()
            } else {
                rev < weekly
            };
            if short || tried_recently(world, gang, b, tried::PROTECT) {
                continue;
            }
            if best.is_none_or(|(r, x)| rev > r || (rev == r && b < x)) {
                best = Some((rev, b));
            }
        }
    }
    best.map(|(_, b)| b)
}

/// Daily, each gang on its own weekday (`gang index % 7`): under Expand or
/// Contest, its leader leans on one business per held district (see
/// [`protection_target`]) for `protection_weekly` a week. Paid at once and
/// weekly after ([`collect`]).
fn protection_pass(world: &mut World) {
    let day = world.day();
    for (i, g) in world.gangs().into_iter().enumerate() {
        if day % 7 != i as u64 % 7 {
            continue;
        }
        let Some((leader, order)) =
            world.comp::<crate::components::Gang>(g).and_then(|x| x.leader.map(|l| (l, x.order)))
        else {
            continue;
        };
        // The pressure pass phase 3 (OPEN-162): keyed to held ground, not
        // the day's order (only a gang lying low leans on nobody).
        let ground_on = crate::systems::protection::on(world);
        let order_ok = if ground_on {
            order != crate::components::Order::LieLow
        } else {
            matches!(order, crate::components::Order::Expand | crate::components::Order::Contest)
        };
        if !order_ok || !free(world, leader) {
            continue;
        }
        let districts: Vec<_> = if ground_on {
            crate::systems::protection::ground(world, g)
        } else {
            crate::systems::gang::held_districts(world, g).into_iter().map(|(d, _)| d).collect()
        };
        let per = crate::systems::protection::cfg(world).map_or(1, |c| c.lean_per_district.max(1));
        for d in districts {
            for _ in 0..per {
                if !room(world) {
                    return;
                }
                let Some(b) = protection_target(world, g, d) else { break };
                try_protection(world, g, leader, b);
            }
        }
    }
}

/// One protection move: `leader` of `gang` on whoever answers for `b`.
/// True when it was made.
pub fn try_protection(world: &mut World, gang: EntityId, leader: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(owner) = payer_of(world, b) else { return false };
    let Some(over) = answers_for(world, owner) else { return false };
    if over == leader {
        return false;
    }
    mark(world, gang, b, tried::PROTECT);
    let weekly = world.config.leverage.protection_weekly;
    let amount = match world.comp::<Building>(b) {
        Some(bd) if crate::systems::protection::on(world) => {
            crate::systems::protection::levy(world, week_revenue(bd), weekly).unwrap_or(weekly)
        }
        _ => weekly,
    };
    let demand = Demand::Coins { amount, weekly: true };
    let out = roll(world, leader, over, Some(owner), &Leverage::Threat, &demand);
    if !out.success {
        // The pressure pass phase 3 (OPEN-160): a refusal stands as a hold
        // the gang punishes in person.
        if crate::systems::protection::on(world) {
            let id = record(
                world,
                Some(gang),
                leader,
                over,
                Leverage::Threat,
                demand,
                CoercionWhy::Protection,
                Some(owner),
                Some(b),
            );
            crate::systems::protection::refused(world, id);
        }
        return false;
    }
    let id = record(
        world,
        Some(gang),
        leader,
        over,
        Leverage::Threat,
        demand,
        CoercionWhy::Protection,
        Some(owner),
        Some(b),
    );
    world.stats.current.leverage.protection_made += 1;
    if crate::systems::protection::on(world) {
        crate::systems::ownership::jobs2_row(world).pr_leaned += 1;
    }
    let d = world.district_of_building(b);
    crate::systems::gossip::post_deed(world, d, Deed::Extorted, Some(leader), Some(over));
    // The first week now (the pre-view pass phase 6, P13: a body leader
    // collects it on his round, in person).
    if !crate::systems::leaders::collects(world, gang) {
        collect_one(world, id);
    }
    true
}

/// The pressure pass phase 3: an owner who went to rival `gang` for cover
/// pays it `amount` a week from now on (no roll: he asked); the first week
/// now unless its leader collects in person.
pub(crate) fn cover(
    world: &mut World,
    gang: EntityId,
    leader: EntityId,
    over: EntityId,
    payer: EntityId,
    b: EntityId,
    amount: i64,
) {
    let demand = Demand::Coins { amount, weekly: true };
    let id = record(
        world,
        Some(gang),
        leader,
        over,
        Leverage::Threat,
        demand,
        CoercionWhy::Protection,
        Some(payer),
        Some(b),
    );
    world.stats.current.leverage.protection_made += 1;
    if !crate::systems::leaders::collects(world, gang) {
        collect_one(world, id);
    }
}

/// A coercion just made pays its first week now.
fn collect_one(world: &mut World, id: CoercionId) {
    pay_week(world, id);
}

// ---------------------------------------------------------------------------
// Secrets and blackmail
// ---------------------------------------------------------------------------

/// Is `d` a deed a secret can be (spec: `deed_sev ≥ 0.4` or `honour_w < 0`)?
fn shameful(world: &World, d: Deed) -> bool {
    world.config.gossip.deed_sev.get(d) >= 0.4 || world.config.reputation.honour_w.get(d) < 0.0
}

/// The deeds `holder` knows first-hand or at one hop (`memory::deed_of`
/// over its first-hand memories and its heard store) that a secret can be,
/// with a living person as the actor other than the holder.
pub fn secrets_of(world: &World, holder: EntityId) -> SmallVec<[DeedRef; 8]> {
    let mut out: SmallVec<[DeedRef; 8]> = SmallVec::new();
    let Some(m) = world.comp::<Memory>(holder) else { return out };
    for e in m.entries.iter().chain(m.heard.iter()) {
        if e.hops > 1 {
            continue;
        }
        let Some(r) = crate::systems::memory::deed_of(holder, e) else { continue };
        let Some(a) = r.actor else { continue };
        if a == holder || !world.has::<Identity>(a) || !shameful(world, r.deed) {
            continue;
        }
        if !out.iter().any(|x| x.deed == r.deed && x.actor == r.actor && x.object == r.object) {
            out.push(r);
        }
    }
    out
}

/// How many agents hold each of `keys` (any hops): one pass over the
/// memories (the spec's daily tally of the keys referenced, run only on a
/// day a key is asked for).
pub fn holders(world: &World, keys: &[DeedRef]) -> Vec<u32> {
    let mut n = vec![0u32; keys.len()];
    if keys.is_empty() {
        return n;
    }
    // scan-ok: daily, only when a secret is weighed.
    for id in world.with::<Memory>() {
        let Some(m) = world.comp::<Memory>(id) else { continue };
        let mut seen = vec![false; keys.len()];
        for e in m.entries.iter().chain(m.heard.iter()) {
            let Some(r) = crate::systems::memory::deed_of(id, e) else { continue };
            for (i, k) in keys.iter().enumerate() {
                if !seen[i] && k.deed == r.deed && k.actor == r.actor && k.object == r.object {
                    seen[i] = true;
                    n[i] += 1;
                }
            }
        }
    }
    n
}

/// A secret `holder` holds on `over` (a deed `over` did, known to few and
/// not yet public), the most severe; `None` if none.
pub fn secret_on(world: &World, holder: EntityId, over: EntityId) -> Option<Leverage> {
    let cfg = &world.config.leverage;
    let mine: SmallVec<[DeedRef; 8]> =
        secrets_of(world, holder).into_iter().filter(|r| r.actor == Some(over)).collect();
    let counts = holders(world, &mine);
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    mine.iter()
        .zip(counts)
        .filter(|&(r, n)| n <= cfg.secret_known_max && reach(world, r.deed, over, r.object) < cfg.secret_public_reach)
        .max_by(|a, b| {
            let (sa, sb) = (world.config.gossip.deed_sev.get(a.0.deed), world.config.gossip.deed_sev.get(b.0.deed));
            sa.total_cmp(&sb).then(b.0.deed.cmp(&a.0.deed))
        })
        .map(|(r, _)| Leverage::Secret { deed: r.deed, actor: over, object: r.object, day })
}

fn under_blackmail(world: &World, id: EntityId) -> bool {
    world
        .coercions
        .values()
        .any(|c| c.is_active() && c.why == CoercionWhy::Blackmail && (c.over == id || c.agent == id))
}

/// Daily: the adults whose day it is (`index % blackmail_days`), under
/// `blackmail_lawfulness`, free, holding no blackmail: each looks for a
/// secret on someone richer (`standing` above its own by `blackmail_gap`)
/// known to at most `secret_known_max` and not public, and makes the
/// richest of them pay `blackmail_weekly` a week.
fn blackmail_pass(world: &mut World) {
    let cfg = world.config.leverage.clone();
    let day = world.day();
    let period = u64::from(cfg.blackmail_days.max(1));
    let mut cands: Vec<(EntityId, EntityId, DeedRef)> = Vec::new();
    // scan-ok: daily, one adult in `blackmail_days`.
    for a in world.citizens() {
        if !(u64::from(a.index) + day).is_multiple_of(period) {
            continue;
        }
        if world.comp::<Personality>(a).is_none_or(|p| p.lawfulness >= cfg.blackmail_lawfulness) || !free(world, a) {
            continue;
        }
        if under_blackmail(world, a) {
            continue;
        }
        let mine = reputation::rep(world, a).standing;
        let best = secrets_of(world, a)
            .into_iter()
            .filter_map(|r| r.actor.map(|t| (t, r)))
            .filter(|&(t, _)| law::living(world, t) && !world.has::<Sentence>(t) && !under_blackmail(world, t))
            .filter(|&(t, _)| !tried_recently(world, a, t, tried::BLACKMAIL))
            .map(|(t, r)| (reputation::rep(world, t).standing, t, r))
            .filter(|&(s, _, _)| s > mine + cfg.blackmail_gap)
            .max_by(|x, y| x.0.total_cmp(&y.0).then(y.1.cmp(&x.1)));
        if let Some((_, t, r)) = best {
            cands.push((a, t, r));
        }
    }
    if cands.is_empty() {
        return;
    }
    let keys: Vec<DeedRef> = cands.iter().map(|c| c.2).collect();
    let counts = holders(world, &keys);
    let day32 = u32::try_from(day).unwrap_or(u32::MAX);
    for ((a, t, r), n) in cands.into_iter().zip(counts) {
        if !room(world) {
            return;
        }
        if n > cfg.secret_known_max || reach(world, r.deed, t, r.object) >= cfg.secret_public_reach {
            continue;
        }
        if under_blackmail(world, t) || under_blackmail(world, a) {
            continue;
        }
        let lev = Leverage::Secret { deed: r.deed, actor: t, object: r.object, day: day32 };
        try_blackmail(world, a, t, lev);
    }
}

/// One blackmail move by `holder` on `over` with `lev` (a Secret). True
/// when made: the hush money is due weekly from now.
pub fn try_blackmail(world: &mut World, holder: EntityId, over: EntityId, lev: Leverage) -> bool {
    if !on(world) {
        return false;
    }
    mark(world, holder, over, tried::BLACKMAIL);
    let amount = world.config.leverage.blackmail_weekly;
    let demand = Demand::Coins { amount, weekly: true };
    let out = roll(world, holder, over, None, &lev, &demand);
    if !out.success {
        return false;
    }
    let id = record(world, Some(holder), holder, over, lev, demand, CoercionWhy::Blackmail, None, None);
    world.stats.current.leverage.blackmail_made += 1;
    let d = crate::systems::gossip::talk_district(world, holder);
    crate::systems::gossip::post_deed(world, d, Deed::Blackmailed, Some(holder), Some(over));
    collect_one(world, id);
    true
}

/// Daily: a blackmail lapses (no backlash) once any pool entry of its deed
/// reaches `secret_public_reach`: the secret is out.
fn lapse(world: &mut World) {
    let bound = world.config.leverage.secret_public_reach;
    let out: Vec<CoercionId> = world
        .coercions
        .values()
        .filter(|c| c.is_active())
        .filter_map(|c| match c.leverage {
            Leverage::Secret { deed, actor, object, .. } => {
                (reach(world, deed, actor, object) >= bound).then_some(c.id)
            }
            _ => None,
        })
        .collect();
    for id in out {
        close(world, id, CoercionStatus::Lapsed, "public");
    }
}

// ---------------------------------------------------------------------------
// Threats: Silence and the coerced job
// ---------------------------------------------------------------------------

/// Silence (spec § 4 Threat), daily before the Hit market's Silence pass:
/// a free suspect under `silence_lawfulness` with an open report whose
/// witness it knows leans on the witness to withdraw it (one try per pair
/// per `retry_days`). A success resolves every open report the witness
/// filed on the suspect, so the Hit market finds nothing to buy; a failure
/// leaves it to the Beat. Gang members included (a threat is personal).
pub fn silence_pass(world: &mut World) {
    if !on(world) {
        return;
    }
    let max = world.config.leverage.silence_lawfulness;
    for (_, s, w, _) in crate::systems::hitmarket::silenceable(world) {
        if world.comp::<Personality>(s).is_none_or(|p| p.lawfulness >= max) {
            continue;
        }
        if tried_recently(world, s, w, tried::SILENCE) || !law::living(world, w) {
            continue;
        }
        try_silence(world, s, w);
    }
}

/// One Silence move by suspect `s` on witness `w`. True when withdrawn.
pub fn try_silence(world: &mut World, s: EntityId, w: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(tick) = world
        .crime_reports()
        .iter()
        .filter(|r| !r.resolved && r.suspect == s && r.witness == Some(w))
        .map(|r| r.tick)
        .max()
    else {
        return false;
    };
    mark(world, s, w, tried::SILENCE);
    let demand = Demand::Silence { suspect: s, tick };
    if !roll(world, s, w, None, &Leverage::Threat, &demand).success {
        return false;
    }
    let idx: Vec<usize> = world
        .crime_reports()
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.resolved && r.suspect == s && r.witness == Some(w))
        .map(|(i, _)| i)
        .collect();
    for i in idx {
        if let Some(r) = world.reports_mut().get_mut(i) {
            r.resolved = true;
        }
    }
    world.stats.current.leverage.silenced_threat += 1;
    let text = format!("{} leaned on {}: the report was withdrawn", world.name_of(s), world.name_of(w));
    world.push_event(EventKind::Coerced, &[s, w], text);
    let d = crate::systems::gossip::talk_district(world, w);
    crate::systems::gossip::post_deed(world, d, Deed::Extorted, Some(s), Some(w));
    true
}

/// A coerced job (spec § 4 Threat), from the Hit market's faction post
/// when the buyer cannot pay: the faction's head leans on the strongest
/// regular of a Fixer (the buyer's held one, else the nearest) who is
/// weaker than the head, free, not tied to the target, and holds no
/// record; on success the record is posted `Terms::Coerced` (unpaid) and
/// taken by that regular at once. True when posted.
#[allow(clippy::too_many_arguments)]
pub fn coerced_job(
    world: &mut World,
    buyer: EntityId,
    head: EntityId,
    kind: ContractKind,
    target: EntityId,
    broker: Option<EntityId>,
    origin: Origin,
    extra: PostExtra,
) -> bool {
    if !on(world) || !kind.violent() || !room(world) || !free(world, head) {
        return false;
    }
    let fixer = broker.or_else(|| contracts::nearest_fixer(world, head));
    let Some(fixer) = fixer.filter(|&f| contracts::fixer_open(world, f)) else { return false };
    let regulars: Vec<EntityId> =
        world.comp::<crate::contract::Broker>(fixer).map(|k| k.regulars.keys().copied().collect()).unwrap_or_default();
    let my = moves::might(world, head);
    let tied = |r: EntityId| {
        crate::systems::grudges::member_of(world, r, buyer)
            || crate::systems::grudges::factions_of(world, target)
                .iter()
                .any(|&f| crate::systems::grudges::member_of(world, r, f))
            || world.edge(r, target).is_some_and(|e| {
                matches!(
                    e.kind,
                    crate::components::RelKind::Spouse
                        | crate::components::RelKind::Family
                        | crate::components::RelKind::Parent
                        | crate::components::RelKind::Friend
                )
            })
    };
    let pick = regulars
        .into_iter()
        .filter(|&r| r != head && r != target && free(world, r) && !law::is_guard(world, r))
        .filter(|&r| !contracts::holds_taken(world, r) && !tied(r))
        .filter(|&r| !tried_recently(world, head, r, tried::JOB))
        .filter(|&r| moves::might(world, r) < my)
        .map(|r| (law::fighting(world, r), r))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, r)| r);
    let Some(gun) = pick else { return false };
    mark(world, head, gun, tried::JOB);
    // The record's id is the next one: the demand names it.
    let next = world.next_contract.max(1);
    let demand = Demand::Do(next);
    // The value read off the kind (the record does not exist yet).
    let lev = Leverage::Threat;
    let m = SocialMove { actor: head, target: gun, kind: MoveKind::Intimidate, stake: Stake::Info { about: head } };
    let cfg = &world.config.leverage;
    let x = cfg.lev_bias.get(LeverageKind::Threat)
        - cfg.demand_w * world.config.contracts.price_base.get(kind) as f32 / means(world, gun, None).max(1.0);
    let out = moves::resolve_with(world, &m, x);
    world.stats.current.leverage.note_try(LeverageKind::Threat, out.success);
    if !out.success {
        report_failure(world, head, gun, &lev);
        return false;
    }
    let cid = record(world, Some(buyer), head, gun, lev, demand, CoercionWhy::Job, None, None);
    let posting = Posting {
        buyer: Some(buyer),
        agent: Some(head),
        kind,
        target: Target::Agent(target),
        broker: None,
        deadline_days: world.config.contracts.deadline_days.get(kind),
        origin,
        price: None,
    };
    match contracts::post_coerced(world, posting, extra, cid, gun) {
        Some(id) => {
            if let Some(c) = world.coercions.get_mut(&cid) {
                c.demand = Demand::Do(id);
            }
            world.stats.current.leverage.coerced_jobs += 1;
            true
        }
        None => {
            close(world, cid, CoercionStatus::Broken, "the job could not be posted");
            false
        }
    }
}

/// From `contracts::set_status` when a `Terms::Coerced` record closes:
/// done, the coerced complied; otherwise the hold is broken.
pub fn on_job_closed(world: &mut World, cid: CoercionId, status: ContractStatus) {
    match status {
        ContractStatus::Fulfilled => {
            close(world, cid, CoercionStatus::Complied, "done");
            world.stats.current.leverage.coerced_done += 1;
        }
        ContractStatus::Open | ContractStatus::Taken => {}
        _ => close(world, cid, CoercionStatus::Broken, "the job was not done"),
    }
}

// ---------------------------------------------------------------------------
// Poaching by threat, Debt
// ---------------------------------------------------------------------------

/// Poaching by threat (spec § 4), from `competence::try_poach`: an exec
/// under `poach_threat_lawfulness` uses a Secret it holds on the worker,
/// else a Threat, with `Join` at the base wage instead of the premium.
/// `None` when the exec poaches by the wage (lawful, or leverage off).
pub fn poach(world: &mut World, exec: EntityId, target: EntityId, building: EntityId, wage: i64) -> Option<bool> {
    if !on(world)
        || world.comp::<Personality>(exec).is_none_or(|p| p.lawfulness >= world.config.leverage.poach_threat_lawfulness)
    {
        return None;
    }
    let lev = secret_on(world, exec, target).unwrap_or(Leverage::Threat);
    world.stats.current.leverage.poach_threats += 1;
    Some(roll(world, exec, target, None, &lev, &Demand::Join { building, wage }).success)
}

/// Debt (spec § 4), from `social::daily` at a debt's fortnight mark (the
/// M15 `Edge.debt_since` re-stamp: a debt unpaid 14 days): the creditor,
/// free, calls a debt of `debt_call_min` or more from a living debtor
/// (one try per pair per `retry_days`). Success: the debtor pays what it
/// can of it (`Flow::Coerced`) and the edge's debt falls by that.
pub fn call_debts(world: &mut World, aged: &[(EntityId, EntityId, i64)]) {
    if !on(world) {
        return;
    }
    let min = world.config.leverage.debt_call_min;
    for &(creditor, debtor, amount) in aged {
        if amount < min || !free(world, creditor) || !law::living(world, debtor) || world.has::<Sentence>(debtor) {
            continue;
        }
        if tried_recently(world, creditor, debtor, tried::DEBT) {
            continue;
        }
        mark(world, creditor, debtor, tried::DEBT);
        let demand = Demand::Coins { amount, weekly: false };
        if !roll(world, creditor, debtor, None, &Leverage::Debt { amount }, &demand).success {
            continue;
        }
        let paid = ownership::pay(world, Some(debtor), Some(creditor), amount, Flow::Coerced);
        if paid <= 0 {
            continue;
        }
        let (lo, _) = crate::components::edge_key(creditor, debtor);
        let add = i32::try_from(paid).unwrap_or(i32::MAX);
        if world.edge(creditor, debtor).is_some() {
            let e = world.edge_entry(creditor, debtor);
            // The lower id owes the higher id when `debt > 0`.
            if lo == debtor {
                e.debt = (e.debt - add).max(0);
            } else {
                e.debt = (e.debt + add).min(0);
            }
            if e.debt == 0 {
                e.debt_since = None;
            }
        }
        let text = format!("{} called in a debt from {}: {paid} paid", world.name_of(creditor), world.name_of(debtor));
        world.push_event(EventKind::Coerced, &[creditor, debtor], text);
    }
}

// ---------------------------------------------------------------------------
// Parley
// ---------------------------------------------------------------------------

/// A gang's fighting strength: Σ `raid::strength` over its free members.
fn gang_strength(world: &World, g: EntityId) -> f32 {
    world
        .comp::<crate::components::Gang>(g)
        .map(|x| {
            x.members
                .iter()
                .filter(|&&m| law::living(world, m) && !world.has::<Sentence>(m))
                .map(|&m| crate::systems::raid::strength(world, m))
                .sum()
        })
        .unwrap_or(0.0)
}

/// The gang `g` would parley with: a rival gang in an open vendetta with
/// it, else its rival under its Contest, whose strength is above its own
/// by `parley_ratio`.
pub fn parley_rival(world: &World, g: EntityId) -> Option<EntityId> {
    let in_vendetta = world
        .vendettas
        .iter()
        .filter_map(|v| {
            if v.a == g {
                Some(v.b)
            } else if v.b == g {
                Some(v.a)
            } else {
                None
            }
        })
        .filter(|&o| world.has::<crate::components::Gang>(o))
        .min();
    let contest = world
        .comp::<crate::components::Gang>(g)
        .filter(|x| x.order == crate::components::Order::Contest)
        .and_then(|_| world.rival_of(g));
    let r = in_vendetta.or(contest)?;
    let own = gang_strength(world, g);
    (gang_strength(world, r) >= own * (1.0 + world.config.leverage.parley_ratio)).then_some(r)
}

fn paying_tribute(world: &World, g: EntityId, r: EntityId) -> bool {
    world
        .coercions
        .values()
        .any(|c| c.is_active() && c.why == CoercionWhy::Parley && c.payer == Some(g) && c.holder == Some(r))
}

/// Daily, each gang on its own weekday (`(gang index + 3) % 7`): a gang
/// whose leader is free and whose rival ([`parley_rival`]) has a free
/// leader parleys once a week (one try per pair per `retry_days`).
fn parley_pass(world: &mut World) {
    let day = world.day();
    for (i, g) in world.gangs().into_iter().enumerate() {
        if (day + 3) % 7 != i as u64 % 7 {
            continue;
        }
        let Some(leader) = world.comp::<crate::components::Gang>(g).and_then(|x| x.leader) else { continue };
        if !free(world, leader) {
            continue;
        }
        let Some(r) = parley_rival(world, g) else { continue };
        if paying_tribute(world, g, r) || tried_recently(world, g, r, tried::PARLEY) || !room(world) {
            continue;
        }
        try_parley(world, g, r);
    }
}

/// One Parley (spec § 4): `g`'s leader makes a Persuade move with
/// `Stake::Coins(parley_tribute)` on `r`'s leader. Success: `g` pays `r`
/// the tribute weekly (a coercion held by `r`), the vendetta between them
/// closes (its declared hold cleared, the members' grudges across it
/// halved, so it does not reopen at once) and each side's Regard of the
/// other rises 0.2; `Parleyed`. Failure: the feud is declared for
/// `[grudges] declared_days`. True on success.
pub fn try_parley(world: &mut World, g: EntityId, r: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let (Some(l), Some(rl)) = (
        world.comp::<crate::components::Gang>(g).and_then(|x| x.leader),
        world.comp::<crate::components::Gang>(r).and_then(|x| x.leader),
    ) else {
        return false;
    };
    if !free(world, l) || !law::living(world, rl) {
        return false;
    }
    mark(world, g, r, tried::PARLEY);
    let tribute = world.config.leverage.parley_tribute;
    let m = SocialMove { actor: l, target: rl, kind: MoveKind::Persuade, stake: Stake::Coins(tribute) };
    let out = moves::resolve(world, &m);
    if !out.success {
        world.stats.current.leverage.parley_fails += 1;
        let w = world
            .vendettas
            .iter()
            .find(|v| (v.a, v.b) == (g.min(r), g.max(r)))
            .map_or(0.5, |v| v.w[0].max(v.w[1]).max(0.5));
        crate::systems::grudges::declare(world, g, r, w);
        return false;
    }
    let demand = Demand::Coins { amount: tribute, weekly: true };
    let id = record(world, Some(r), l, l, Leverage::Threat, demand, CoercionWhy::Parley, Some(g), None);
    world.stats.current.leverage.parleys += 1;
    // The feud closes.
    world.vendettas.retain(|v| !((v.a == g && v.b == r) || (v.a == r && v.b == g)));
    halve_grudges(world, g, r);
    halve_grudges(world, r, g);
    for (a, b) in [(g, r), (r, g)] {
        let e = world.regard.entry((a, b)).or_default();
        e.value = (e.value + 0.2).min(1.0);
    }
    let text = format!(
        "{} parleyed with {}: peace for {tribute} a week",
        world.owner_label(Some(g)),
        world.owner_label(Some(r))
    );
    world.push_event(EventKind::Parleyed, &[l, rl], text);
    collect_one(world, id);
    true
}

/// Every unsettled grudge a member of `a` holds on `b` or one of its
/// members, × 0.5.
fn halve_grudges(world: &mut World, a: EntityId, b: EntityId) {
    let members: Vec<EntityId> =
        world.comp::<crate::components::Gang>(a).map(|x| x.members.clone()).unwrap_or_default();
    let theirs: Vec<EntityId> = world.comp::<crate::components::Gang>(b).map(|x| x.members.clone()).unwrap_or_default();
    for m in members {
        if let Some(gs) = world.comp_mut::<crate::word::Grudges>(m) {
            for x in gs.list.iter_mut().filter(|x| x.settled.is_none()) {
                if x.target == b || theirs.contains(&x.target) {
                    x.weight *= 0.5;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fraud: the fake job
// ---------------------------------------------------------------------------

/// The fake job (spec § 4 Fraud), daily: at each open Fixer, a regular
/// with deception ≥ 0.6 and lawfulness < 0.3, on a day its keyed draw
/// falls under `fake_p`, sells a job that does not exist to the regular
/// with the fattest wallet (at least `fake_fee`) it has not tried: a
/// Deceive move (`Stake::Coins(fake_fee)`); success takes the fee
/// (`Flow::Coerced`), and the victim learns it after `fake_days`.
fn fake_pass(world: &mut World) {
    let day = world.day();
    let fee = world.config.leverage.fake_fee;
    let p = world.config.leverage.fake_p;
    for f in contracts::open_fixers(world) {
        let regulars: Vec<EntityId> =
            world.comp::<crate::contract::Broker>(f).map(|k| k.regulars.keys().copied().collect()).unwrap_or_default();
        for &r in &regulars {
            let ok = world.comp::<Skills>(r).is_some_and(|s| s.deception >= 0.6)
                && world.comp::<Personality>(r).is_some_and(|x| x.lawfulness < 0.3)
                && free(world, r);
            if !ok || draw(world, Purpose::Fake, day, u64::from(r.index)) >= p {
                continue;
            }
            let victim = regulars
                .iter()
                .copied()
                .filter(|&v| v != r && free(world, v) && !tried_recently(world, r, v, tried::FAKE))
                .filter(|&v| !world.fake_jobs.iter().any(|j| j.victim == v))
                .map(|v| (world.purse(Some(v)), v))
                .filter(|&(c, _)| c >= fee)
                .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
                .map(|(_, v)| v);
            if let Some(v) = victim {
                try_fake_job(world, r, v);
            }
        }
    }
}

/// One fake job: `faker` sells `victim` a job that does not exist. True
/// when the fee changed hands.
pub fn try_fake_job(world: &mut World, faker: EntityId, victim: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    mark(world, faker, victim, tried::FAKE);
    let fee = world.config.leverage.fake_fee;
    let m = SocialMove { actor: faker, target: victim, kind: MoveKind::Deceive, stake: Stake::Coins(fee) };
    if !moves::resolve(world, &m).success {
        return false;
    }
    let paid = ownership::pay(world, Some(victim), Some(faker), fee, Flow::Coerced);
    if paid <= 0 {
        return false;
    }
    let due = world.tick + Tick::from(world.config.leverage.fake_days.max(1)) * TICKS_PER_DAY;
    world.fake_jobs.push(FakeJob { faker, victim, fee: paid, due });
    world.stats.current.leverage.fake_jobs += 1;
    let text = format!("{} sold {} a job for {paid} up front", world.name_of(faker), world.name_of(victim));
    world.push_event(EventKind::Coerced, &[faker, victim], text);
    true
}

/// Daily: a fake job's victim learns it at its due: a `Betrayed` deed
/// (actor the faker) and the victim's `Betrayed` grudge (`renege_grudge`),
/// and a lawful victim (≥ 0.5) files Fraud.
fn fake_due(world: &mut World) {
    let now = world.tick;
    let (due, keep): (Vec<FakeJob>, Vec<FakeJob>) =
        std::mem::take(&mut world.fake_jobs).into_iter().partition(|j| j.due <= now);
    world.fake_jobs = keep;
    for j in due {
        if !law::living(world, j.victim) {
            continue;
        }
        contracts::betrayed(world, j.faker, j.victim);
        let w = world.config.contracts.renege_grudge;
        crate::systems::grudges::add(world, j.victim, j.faker, crate::word::GrudgeCause::Betrayed, w, 0);
        let text = format!(
            "{} learned the job {} sold for {} never existed",
            world.name_of(j.victim),
            world.name_of(j.faker),
            j.fee
        );
        world.push_event(EventKind::Defrauded, &[j.victim, j.faker], text);
        file_fraud(world, j.victim, j.faker);
    }
}

/// Fraud (spec § 4): a defrauded person with lawfulness ≥ 0.5 files
/// `Crime::Fraud` against the fraudster (a person); the rest keep the grudge.
pub fn file_fraud(world: &mut World, victim: EntityId, culprit: EntityId) {
    if !on(world) || !law::living(world, victim) || !world.has::<Identity>(culprit) {
        return;
    }
    if world.comp::<Personality>(victim).is_none_or(|p| p.lawfulness < 0.5) {
        return;
    }
    law::file_report(world, Crime::Fraud, culprit, Some(victim));
    world.stats.current.leverage.frauds_filed += 1;
}

// ---------------------------------------------------------------------------
// Court: the Persuade kind
// ---------------------------------------------------------------------------

/// The person `id` was rejected by at least `court_rejections` times in
/// the last 30 days (its `Rejected` memories), unmarried and living; the
/// most often, ties the lower id.
pub fn court_target(world: &World, id: EntityId) -> Option<EntityId> {
    let m = world.comp::<Memory>(id)?;
    let month = 30 * TICKS_PER_DAY;
    let mut counts: std::collections::BTreeMap<EntityId, u32> = std::collections::BTreeMap::new();
    for e in m.entries.iter() {
        if e.kind == crate::components::MemoryKind::Rejected && world.tick.saturating_sub(e.tick) < month {
            if let Some(s) = e.subject {
                *counts.entry(s).or_default() += 1;
            }
        }
    }
    let need = world.config.leverage.court_rejections;
    counts
        .into_iter()
        .filter(|&(t, n)| n >= need && t != id && law::living(world, t) && world.has::<Identity>(t))
        .filter(|&(t, _)| world.spouse_of(t).is_none() && crate::systems::demography::is_adult(world, t))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(t, _)| t)
}

/// Daily, one adult in seven (`index % 7`): a suitor under
/// `court_sociability`, unmarried and free, rejected twice by the same
/// person ([`court_target`]), with the quote in its wallet and no live
/// record as buyer, buys a `Persuade` on the courted (brokered at the
/// nearest open Fixer, else direct); `Origin::Court`.
fn court_pass(world: &mut World) {
    let day = world.day();
    let soc = world.config.leverage.court_sociability;
    // scan-ok: daily, one adult in seven.
    for a in world.citizens() {
        if !(u64::from(a.index) + day).is_multiple_of(7) {
            continue;
        }
        if world.comp::<Personality>(a).is_none_or(|p| p.sociability >= soc) || !free(world, a) {
            continue;
        }
        if world.spouse_of(a).is_some() {
            continue;
        }
        let Some(t) = court_target(world, a) else { continue };
        if tried_recently(world, a, t, tried::COURT) {
            continue;
        }
        try_court(world, a, t);
    }
}

/// One Court posting: `suitor` buys a `Persuade` on `courted`. True when posted.
pub fn try_court(world: &mut World, suitor: EntityId, courted: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    if world.by_party.get(&suitor).is_some_and(|l| {
        l.iter().any(|id| world.contracts.get(id).is_some_and(|c| c.is_live() && c.buyer == Some(suitor)))
    }) {
        return false;
    }
    mark(world, suitor, courted, tried::COURT);
    let broker = contracts::nearest_fixer(world, suitor).filter(|&f| contracts::fixer_open(world, f));
    let target = Target::Agent(courted);
    let price = contracts::quote(world, ContractKind::Persuade, &target, broker);
    if world.purse(Some(suitor)) < price {
        return false;
    }
    let deadline_days = world.config.contracts.deadline_days.get(ContractKind::Persuade);
    let p = Posting {
        buyer: Some(suitor),
        agent: Some(suitor),
        kind: ContractKind::Persuade,
        target,
        broker,
        deadline_days,
        origin: Origin::Court,
        price: Some(price),
    };
    contracts::post(world, p).is_ok()
}

/// A `Persuade` record at its ledger due (from `contracts::resolve_ledger`):
/// the taker makes a Charm move on the target; success adds
/// `persuade_affinity` (and 0.1 trust) to the target's edge with the buyer
/// and fulfils the record, failure is a failed attempt.
pub fn resolve_persuade(world: &mut World, id: ContractId, taker: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Some(t) = c.target_agent() else { return };
    let Some(buyer) = c.buyer.or(c.agent) else { return };
    if !law::living(world, t) || !law::living(world, buyer) {
        contracts::settle(world, id, contracts::Settle::Cancelled, "a party is gone");
        return;
    }
    let m = SocialMove { actor: taker, target: t, kind: MoveKind::Charm, stake: Stake::Info { about: buyer } };
    if moves::resolve(world, &m).success {
        // M16b phase 4: a board campaign's Persuade pins the member's vote.
        if c.origin == Origin::Board {
            crate::systems::governance::on_persuaded(world, buyer, t);
            contracts::settle(world, id, contracts::Settle::Fulfilled, "");
            return;
        }
        let aff = world.config.leverage.persuade_affinity;
        crate::systems::social::adjust(world, t, buyer, aff, 0.1);
        contracts::settle(world, id, contracts::Settle::Fulfilled, "");
    } else {
        contracts::fail_attempt(world, id, "the courted would not hear it");
    }
}
