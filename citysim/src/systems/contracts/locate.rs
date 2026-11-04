use super::*;

// ---------------------------------------------------------------------------
// Locate (C27)
// ---------------------------------------------------------------------------

/// C27: `gossip::wants_sighting`'s clause: `who` is the target of a live
/// Locate that `observer` sees.
pub fn locate_wants(world: &World, observer: EntityId, who: EntityId) -> bool {
    if world.locate_targets.is_empty() {
        return false;
    }
    world.locate_targets.get(&who).is_some_and(|l| l.iter().any(|&id| sees(world, observer, id)))
}

/// One paid sighting of `who` at `tile` by `writer` under record `id`
/// (`force` skips the gap: a ledger tracker's half-cap): the sighting into
/// the buyer's database (the law's under `EntityId::NONE`), `per_sighting`
/// from escrow (brokered), the buyer's purse (direct, capped) or the
/// Treasury (public, which also sets `World::last_seen`). `BountyPaid`;
/// the cap fulfils the record.
pub(super) fn pay_sighting(
    world: &mut World,
    id: ContractId,
    writer: EntityId,
    who: EntityId,
    tile: TilePos,
    force: bool,
) -> bool {
    let paid = pay_sighting_quiet(world, id, writer, who, tile, force);
    if paid > 0 {
        tell_sighting(world, writer, who, paid, 1);
    }
    if paid > 0 && world.contracts.get(&id).is_some_and(|x| x.paid_sightings >= world.config.bounty.cap_sightings) {
        settle(world, id, Settle::Fulfilled, "");
    }
    paid > 0
}

/// Round 6 V2 (V14: `BountyPaid` printed three or four times a sighting):
/// one line a sighting, whatever the buyers it was sold to.
fn tell_sighting(world: &mut World, writer: EntityId, who: EntityId, paid: i64, buyers: usize) {
    let to = if buyers > 1 { format!(" ({buyers} buyers)") } else { String::new() };
    let text = format!("{} sold a sighting of {} for {paid}{to}", world.name_of(writer), world.name_of(who));
    world.push_event(EventKind::BountyPaid, &[writer, who], text);
}

/// [`pay_sighting`] untold and unsettled: the coins paid (0: none).
fn pay_sighting_quiet(
    world: &mut World,
    id: ContractId,
    writer: EntityId,
    who: EntityId,
    tile: TilePos,
    force: bool,
) -> i64 {
    let Some(c) = world.contracts.get(&id).cloned() else { return 0 };
    if !c.is_live() || c.paid_sightings >= world.config.bounty.cap_sightings {
        return 0;
    }
    let now = world.tick;
    let gap = Tick::from(world.config.bounty.min_gap_hours) * TICKS_PER_HOUR;
    if !force && c.last_paid.is_some_and(|t| now < t + gap) {
        return 0;
    }
    if Some(writer) == c.buyer || Some(writer) == c.agent || writer == who {
        return 0;
    }
    // Shadow V4 bug 15: a taken Locate is its taker's job: only the taker's
    // and the crew's sightings pay (others' filled the cap and refunded the
    // price while the taker staked out three days for nothing).
    if c.status == ContractStatus::Taken && c.taker.is_some() && c.taker != Some(writer) && !c.crew.contains(&writer) {
        return 0;
    }
    let s = crate::virt::Sighting { who, tile, tick: now, confidence: 1.0, relayed: true };
    let owner = c.buyer.unwrap_or(EntityId::NONE);
    crate::systems::gossip::relay_sighting(world, owner, s);
    if c.buyer.is_none() {
        world.set_last_seen(who, (tile, now));
    }
    let per = world.config.bounty.per_sighting;
    let paid = if c.brokered() {
        ownership::escrow_out(world, id, Some(writer), per, Flow::Payout)
    } else if c.buyer.is_none() {
        ownership::charge(world, None, Some(writer), per, Flow::Payout)
    } else {
        ownership::pay(world, c.buyer, Some(writer), per, Flow::Payout)
    };
    // Nothing left to pay with: the sighting went in, unpaid.
    if paid <= 0 {
        return 0;
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.paid_sightings = x.paid_sightings.saturating_add(1);
        x.last_paid = Some(now);
    }
    world.stats.current.contract.bounties_paid += 1;
    paid
}

/// C27, from `gossip::maybe_sight` after a body's sighting is written: each
/// live Locate on `who` that `observer` sees pays it (at most one per
/// `min_gap_hours`, `cap_sightings` in all).
pub fn on_sighting(world: &mut World, observer: EntityId, who: EntityId, tile: TilePos) {
    if world.locate_targets.is_empty() {
        return;
    }
    let Some(ids) = world.locate_targets.get(&who).cloned() else { return };
    // The pre-view pass phase 4 (P3, OPEN-119): a sighting is sold by an
    // observer awake (a gang member asleep beside the target sold one).
    if crate::systems::presence::on(world) && crate::systems::presence::asleep(world, observer) {
        crate::systems::ownership::jobs2_row(world).sight_asleep += 1;
        return;
    }
    // Round 6 V2: the buyers paid for one sighting, told on one line.
    let mut paid = 0i64;
    let mut buyers = 0usize;
    let mut full: Vec<ContractId> = Vec::new();
    for id in ids {
        if sees(world, observer, id) {
            let p = pay_sighting_quiet(world, id, observer, who, tile, false);
            if p > 0 {
                paid += p;
                buyers += 1;
                if world.contracts.get(&id).is_some_and(|x| x.paid_sightings >= world.config.bounty.cap_sightings) {
                    full.push(id);
                }
            }
        }
    }
    if paid > 0 {
        tell_sighting(world, observer, who, paid, buyers);
    }
    for id in full {
        settle(world, id, Settle::Fulfilled, "");
    }
}

/// C27, a Locate tracker's `StakeOut` contact: a paid sighting, and the
/// run goes back to asking (the tracker keeps tracking until the cap or
/// the deadline). No-op for any other run.
/// M18 P4 (the stakeout loop, P3's finding 3): a taken Locate's taker in the
/// gap between paid sightings (`[bounty] min_gap_hours`) is waiting: the
/// stakeout's contact does not end it (an unpaid contact reset the chase and
/// the next plan staked out again, every 4 min for hours; NPC trackers made
/// 49 two-minute stake-outs). The stake-out runs to its timeout or to a
/// contact after the gap, which is paid.
pub fn locate_waits(world: &World, taker: EntityId) -> bool {
    let Some(r) = world.contract_runs.get(&taker) else { return false };
    let Some(c) = world.contracts.get(&r.contract) else { return false };
    let gap = Tick::from(world.config.bounty.min_gap_hours) * TICKS_PER_HOUR;
    c.kind == ContractKind::Locate
        && c.status == ContractStatus::Taken
        && c.last_paid.is_some_and(|t| world.tick < t + gap)
}

pub fn on_contact(world: &mut World, taker: EntityId) {
    let Some(r) = world.contract_runs.get(&taker).cloned() else { return };
    let Some(c) = world.contracts.get(&r.contract) else { return };
    // Shadow V4 bug 15: a live Tag's plant is made at contact.
    if c.kind == ContractKind::Tag && c.status == ContractStatus::Taken {
        crate::systems::tags::resolve_plant(world, r.contract, taker);
        return;
    }
    if c.kind != ContractKind::Locate || c.status != ContractStatus::Taken {
        return;
    }
    let tile = world.comp::<Position>(r.target).map_or_else(Default::default, |p| p.tile);
    if !pay_sighting(world, r.contract, taker, r.target, tile, false) {
        spent_locate(world, r.contract);
    }
    reset_chase(world, taker);
}

/// Round 5 U7 (`OPEN-222`): a taken Locate whose payer has nothing left for
/// a sighting is settled as the ledger tracker's report settles it
/// (fulfilled on the sightings it bought; a direct buyer short with none
/// bought reneges; else back on the board). A brokered Locate escrows its
/// price (30-40c) while `cap_sightings` x `per_sighting` is 150c: after two
/// or three paid sightings every contact was refused, the cap never came,
/// and the taker staked out a week (`gun_41438`: 12 aborts, 5-11 h a day, no
/// result; 20k `gun_23612`: 49 two-minute stake-outs). A refusal for the
/// gap between sightings is not this (the payer still holds coins).
pub(super) fn spent_locate(world: &mut World, id: ContractId) {
    let Some(x) = world.contracts.get(&id).filter(|x| x.status == ContractStatus::Taken).cloned() else { return };
    let spent = if x.brokered() { x.escrow <= 0 } else { x.buyer.is_some() && world.purse(x.buyer) <= 0 };
    if !spent || x.paid_sightings >= world.config.bounty.cap_sightings {
        return;
    }
    if x.paid_sightings > 0 {
        settle(world, id, Settle::Fulfilled, "");
    } else if x.buyer.is_some() && !x.brokered() {
        renege(world, id, &x);
    } else {
        fail_attempt(world, id, "nothing left to pay a sighting with");
    }
}

/// C15: `taker`'s chase back to asking (intel, venue and stakeout cleared).
pub(super) fn reset_chase(world: &mut World, taker: EntityId) {
    if let Some(mut x) = crate::systems::hunt::chase(world, taker) {
        x.phase = HuntPhase::Ask;
        x.intel = None;
        x.venue = None;
        x.stakeout_until = None;
        crate::systems::hunt::set_chase(world, taker, x);
        crate::systems::hunt::reindex(world);
    }
}
