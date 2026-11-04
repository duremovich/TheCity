use super::*;

// ---------------------------------------------------------------------------
// Settlement and the status machine
// ---------------------------------------------------------------------------

/// The one status writer (C16): the closing tick, the indices (the
/// ledger, the Locate targets, the Fixer's book, the live run and its
/// plan), the counters, the log and the event.
pub fn set_status(world: &mut World, id: ContractId, status: ContractStatus, why: &str) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let now = world.tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.status = status;
        if !status.is_live() {
            x.closed = Some(now);
        }
    }
    if c.status == ContractStatus::Taken && status != ContractStatus::Taken {
        if let Some(d) = c.due {
            world.ledger_due.remove(&(d, id));
        }
        if let Some(t) = c.taker {
            end_run(world, t, id);
        }
        for &m in &c.crew {
            end_run(world, m, id);
        }
        // Phase 2 (C22): the mission ends with the record's Taken state.
        end_mission(world, id);
    }
    if !status.is_live() {
        if let Target::Agent(t) = c.target {
            if let Some(l) = world.locate_targets.get_mut(&t) {
                l.retain(|x| *x != id);
                if l.is_empty() {
                    world.locate_targets.remove(&t);
                }
            }
        }
        if let Some(b) = c.broker {
            if let Some(k) = world.comp_mut::<Broker>(b) {
                k.book.retain(|&x| x != id);
            }
        }
    }
    let kind = c.kind;
    // Phase 2 (C25): a squad of two or more.
    let squad = squad_size(world, &c) >= 2;
    let row = &mut world.stats.current.contract;
    match status {
        ContractStatus::Fulfilled => {
            row.contracts_fulfilled += 1;
            if kind == ContractKind::Persuade {
                world.stats.current.leverage.k_persuade_done += 1;
            } else if let Some(i) = crate::stats::CaptiveCols::slot(kind) {
                world.stats.current.captive.k_done[i] += 1;
            } else if kind == ContractKind::Trauma {
                // M16b phase 3: a subscription is never fulfilled.
            } else if kind == ContractKind::Tag {
                world.stats.current.gov.k_tag_done += 1;
            } else if kind == ContractKind::Vote {
                world.stats.current.gov.k_vote_done += 1;
            } else if kind == ContractKind::Retake {
                world.outside.today.retakes_won += 1;
            } else {
                world.stats.current.contract.k_done[kind.index()] += 1;
            }
            let row = &mut world.stats.current.contract;
            if kind == ContractKind::Hit {
                row.hits_done += 1;
                row.contract_murders += 1;
                if squad {
                    row.hits_squad += 1;
                }
            }
        }
        ContractStatus::Reneged => {
            row.reneged += 1;
            if kind == ContractKind::Hit {
                row.hits_done += 1;
                row.contract_murders += 1;
            }
        }
        ContractStatus::Failed => row.contracts_failed += 1,
        ContractStatus::Expired => row.contracts_expired += 1,
        ContractStatus::Cancelled => row.contracts_cancelled += 1,
        ContractStatus::Open | ContractStatus::Taken | ContractStatus::Standing => {}
    }
    // The Hit market: organic outcomes, and a buyer whose record closed
    // unfulfilled waits `repost_days` before buying again (H8).
    if !c.motive.is_none() {
        match status {
            ContractStatus::Fulfilled | ContractStatus::Reneged => {
                let h = &mut world.stats.current.hit;
                match kind {
                    ContractKind::Hit => h.hm_hits_done += 1,
                    _ => h.hm_beats_done += 1,
                }
                // A bought beating answers the grudge as a Hunt's beating
                // does (`beat_settle`; a Hit's death settles it outright),
                // so the buyer does not buy the same beating twice.
                if kind == ContractKind::Beat && c.motive == Motive::Revenge {
                    if let Some(a) = c.agent {
                        crate::systems::grudges::answer_beating(world, a, c.target.id());
                    }
                }
                // Phase 2 (H15, H17, the Debt clock).
                crate::systems::hitmarket::on_fulfilled(world, &c);
            }
            ContractStatus::Failed | ContractStatus::Expired | ContractStatus::Cancelled => {
                let until = now + Tick::from(world.config.hitmarket.repost_days) * TICKS_PER_DAY;
                let own = c.agent.filter(|&a| c.buyer == Some(a));
                if let Some(b) = own.and_then(|a| world.comp_mut::<Brain>(a)) {
                    let t = b.cooldowns.entry(GoalKind::Hunt).or_insert(until);
                    *t = (*t).max(until);
                }
            }
            ContractStatus::Open | ContractStatus::Taken | ContractStatus::Standing => {}
        }
    }
    // M16b § 4: a coerced job's coercion follows its record.
    if let Terms::Coerced(cid) = c.terms {
        if !status.is_live() {
            crate::systems::leverage::on_job_closed(world, cid, status);
        }
    }
    // C30: a fulfilled brokered Hit warms its Fixer.
    if status == ContractStatus::Fulfilled && kind == ContractKind::Hit {
        if let Some(b) = c.broker {
            let by = world.config.law.heat_per_hit;
            heat_add(world, b, by);
        }
    }
    // C31: a corruption record leaving Taken takes its guard off the take.
    if kind == ContractKind::Guard && c.status == ContractStatus::Taken {
        reindex_take(world);
    }
    let target = c.target.id();
    let tname = world.name_of(target);
    let taker = c.taker.unwrap_or(EntityId::NONE);
    let (ev, text) = match status {
        ContractStatus::Fulfilled if why.is_empty() => (
            EventKind::ContractFulfilled,
            format!("{} on {tname}: done by {}", kind.label(), world.owner_label(c.taker)),
        ),
        ContractStatus::Fulfilled => (
            EventKind::ContractFulfilled,
            format!("{} on {tname}: done by {} ({why})", kind.label(), world.owner_label(c.taker)),
        ),
        ContractStatus::Failed => (EventKind::ContractFailed, format!("{} on {tname}: failed ({why})", kind.label())),
        // Round 8 X2 (`OPEN-247`): a taken record's lapse says whose it was.
        ContractStatus::Expired if c.taker.is_some() && c.kind.violent() => (
            EventKind::ContractExpired,
            format!(
                "{} on {tname}: expired ({} never struck before the deadline)",
                kind.label(),
                world.owner_label(c.taker)
            ),
        ),
        ContractStatus::Expired => (EventKind::ContractExpired, format!("{} on {tname}: expired", kind.label())),
        ContractStatus::Cancelled => {
            (EventKind::ContractFailed, format!("{} on {tname}: cancelled ({why})", kind.label()))
        }
        ContractStatus::Reneged => (
            EventKind::Reneged,
            format!("{} stiffed {} on the {}", world.owner_label(c.buyer), world.owner_label(c.taker), kind.label()),
        ),
        ContractStatus::Open | ContractStatus::Taken | ContractStatus::Standing => return,
    };
    let mut actors: SmallVec<[EntityId; 6]> = if status == ContractStatus::Reneged {
        SmallVec::from_slice(&[c.buyer.unwrap_or(EntityId::NONE), taker])
    } else {
        SmallVec::from_slice(&[taker, target])
    };
    // Round 4 T3 (OPEN-222): the crew hears how its job ended (V12's
    // `gun_37892` brawled for a corp's Beat and no line closed it).
    for &m in &c.crew {
        if !actors.contains(&m) {
            actors.push(m);
        }
    }
    // Round 5 U5 (OPEN-217): a brokered record's outcome comes back to its
    // fixer (V13: corp posts "never come back with an outcome"), with the
    // cut paid to the owner, or the escrow returned.
    let mut text = text;
    if let Some(b) = c.broker {
        if let Some(h) = crate::systems::service::fixer_hand(world, b) {
            let cut = world.contracts.get(&id).map_or(0, |x| x.cut);
            let owner = world.owner_of(b).map(|o| world.owner_label(Some(o))).unwrap_or_else(|| "nobody".into());
            let money = match status {
                ContractStatus::Fulfilled if cut > 0 => format!("the cut {cut}c to {owner}"),
                ContractStatus::Fulfilled => "no cut".to_string(),
                ContractStatus::Reneged => "the buyer's coins gone".to_string(),
                _ => "the escrow back to the buyer".to_string(),
            };
            text.push_str(&format!("; back to {} at {}: {money}", world.name_of(h), world.name_of(b)));
            if !actors.contains(&h) {
                actors.push(h);
            }
            ownership::jobs2_row(world).fixer_outcomes += 1;
        }
    }
    world.push_event(ev, &actors, text.clone());
    log(world, id, text);
    // Round 8 X2 (`OPEN-247`): a man lying low over it hears it is off.
    if c.kind.violent() && crate::systems::grief::lying_low(world, target) && crate::systems::law::living(world, target)
    {
        crate::systems::grief::end_low(world, target);
        let how = match status {
            ContractStatus::Fulfilled => "was carried out",
            ContractStatus::Expired => "has lapsed",
            _ => "is off",
        };
        let text = format!(
            "{tname} heard the {} on {} head {how}: no more lying low",
            kind.label(),
            crate::systems::grief::his(world, target)
        );
        world.push_event(EventKind::Contract, &[target], text);
    }
    // The pressure pass phase 5a: an unfilled bought Hit or Beat: the buyer
    // reposts it higher, goes himself, or lets it go.
    if !c.motive.is_none() && matches!(status, ContractStatus::Expired | ContractStatus::Failed) {
        let mut closed = c.clone();
        closed.status = status;
        crate::systems::consequence::hit_unfilled(world, &closed, status);
    }
}

/// Drop `who`'s run on record `id` (and its Contract plan).
pub(super) fn end_run(world: &mut World, who: EntityId, id: ContractId) {
    if world.contract_runs.get(&who).is_some_and(|r| r.contract == id) {
        world.contract_runs.remove(&who);
        crate::systems::hunt::reindex(world);
        if world.comp::<Brain>(who).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
            world.abort_plan(who);
        }
        // M18 P3: a body the human sent on it hears the job is over.
        if world.comp::<Brain>(who).is_some_and(|b| b.player.as_ref().is_some_and(|p| p.contract == Some(id))) {
            crate::systems::player::job_next(world, who);
        }
        for list in world.contract_guards.values_mut() {
            list.retain(|g| *g != who);
        }
        world.contract_guards.retain(|_, l| !l.is_empty());
    }
}

/// The purse a refund goes to: the buyer while it can hold coins, else
/// the Treasury.
pub(super) fn refund_to(world: &World, buyer: Option<EntityId>) -> Option<EntityId> {
    buyer.filter(|&b| buyer_ok(world, Some(b)))
}

/// C16: every coin left in escrow back to the buyer (`Flow::Escrow`).
pub fn refund(world: &mut World, id: ContractId) -> i64 {
    let Some(c) = world.contracts.get(&id) else { return 0 };
    let (to, held) = (refund_to(world, c.buyer), c.escrow);
    ownership::escrow_out(world, id, to, held, Flow::Escrow)
}

/// The payees of a fulfilment: the taker first, then the crew.
fn payees(c: &Contract) -> SmallVec<[EntityId; 6]> {
    let mut v: SmallVec<[EntityId; 6]> = SmallVec::new();
    v.extend(c.taker);
    v.extend(c.crew.iter().copied());
    v
}

/// `total` split equally over `n`, the remainder coin to the first.
fn shares(total: i64, n: usize) -> Vec<i64> {
    let n = n.max(1) as i64;
    let each = total / n;
    let mut v = vec![each; n as usize];
    v[0] += total - each * n;
    v
}

/// C16: the one money path out of a live record.
pub fn settle(world: &mut World, id: ContractId, how: Settle, why: &str) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if !c.is_live() {
        return;
    }
    match how {
        Settle::Fulfilled => fulfil(world, id, &c, why),
        Settle::Failed => {
            refund(world, id);
            set_status(world, id, ContractStatus::Failed, why);
        }
        Settle::Expired => {
            refund(world, id);
            set_status(world, id, ContractStatus::Expired, why);
        }
        Settle::Cancelled => {
            refund(world, id);
            set_status(world, id, ContractStatus::Cancelled, why);
        }
    }
    debug_assert!(
        world.contracts.get(&id).is_none_or(|x| x.is_live() || x.escrow == 0),
        "a closed record holds no escrow"
    );
}

fn fulfil(world: &mut World, id: ContractId, c: &Contract, why: &str) {
    let price = c.price;
    if c.kind == ContractKind::Locate {
        // The sightings were paid as they came (C27): the Fixer's cut of
        // the price from what is left, the rest back to the buyer.
        if let Some(b) = c.broker {
            let cut = ((price as f32) * c_cut(world, b)).round() as i64;
            let owner = world.owner_of(b);
            let paid = ownership::escrow_out(world, id, owner, cut, Flow::FixerCut);
            note_cut(world, b, paid);
            if let Some(x) = world.contracts.get_mut(&id) {
                x.cut = paid;
            }
        }
        refund(world, id);
        set_status(world, id, ContractStatus::Fulfilled, "");
        return;
    }
    let pay_to = payees(c);
    if let Some(b) = c.broker {
        // The Hit market (H6): a record posted on credit charges the
        // balance now, into escrow, so the cut and the split read one sum.
        let balance = c.balance();
        let got = if balance > 0 { ownership::escrow_in(world, c.buyer, id, balance) } else { 0 };
        let held = world.contracts.get(&id).map_or(0, |x| x.escrow);
        let cut = ((held as f32) * c_cut(world, b)).round() as i64;
        let owner = world.owner_of(b);
        let to_takers = held - cut;
        for (who, share) in pay_to.iter().zip(shares(to_takers, pay_to.len())) {
            ownership::escrow_out(world, id, Some(*who), share, Flow::Payout);
        }
        let paid = ownership::escrow_out(world, id, owner, cut, Flow::FixerCut);
        note_cut(world, b, paid);
        if let Some(x) = world.contracts.get_mut(&id) {
            x.cut = paid;
        }
        refund(world, id);
        set_status(world, id, ContractStatus::Fulfilled, why);
        if got < balance {
            stiff(world, c, balance - got, balance);
        }
        return;
    }
    // The Hit market (H4, P2 fix round): a direct record on credit holds its
    // deposit: the taker gets the deposit and the balance from the purse,
    // and a short buyer is stiffed for the balance only. A renege keeps
    // the balance (the deposit is the taker's).
    let held = world.contracts.get(&id).map_or(0, |x| x.escrow);
    if held > 0 {
        let balance = c.balance();
        let got = if balance > 0 && !c.renege { ownership::escrow_in(world, c.buyer, id, balance) } else { 0 };
        let held = world.contracts.get(&id).map_or(0, |x| x.escrow);
        for (who, share) in pay_to.iter().zip(shares(held, pay_to.len())) {
            ownership::escrow_out(world, id, Some(*who), share, Flow::Payout);
        }
        refund(world, id);
        if c.renege {
            renege(world, id, c);
        } else {
            set_status(world, id, ContractStatus::Fulfilled, why);
            if got < balance {
                stiff(world, c, balance - got, balance);
            }
        }
        return;
    }
    // C6: a direct record pays from the buyer's purse, unless it reneges or
    // runs short (the taker cannot tell malice from poverty).
    if c.renege {
        renege(world, id, c);
        return;
    }
    let mut moved = 0;
    for (who, share) in pay_to.iter().zip(shares(price, pay_to.len())) {
        moved += if c.buyer.is_none() {
            ownership::charge(world, None, Some(*who), share, Flow::Payout)
        } else {
            ownership::pay(world, c.buyer, Some(*who), share, Flow::Payout)
        };
    }
    if moved < price && c.deposit > 0 {
        // The Hit market (P5): a credit buyer short at fulfilment is broke,
        // not false: the job stands done and the rest is owed.
        set_status(world, id, ContractStatus::Fulfilled, why);
        stiff(world, c, price - moved, price);
    } else if moved < price {
        renege(world, id, c);
    } else {
        set_status(world, id, ContractStatus::Fulfilled, why);
    }
}

/// The Hit market (H6): the buyer of a fulfilled record could not pay
/// `unpaid` of `owed`: the unpaid part is an edge debt buyer → taker (the
/// existing `social::repay_debts` takes it out of later wages), the taker
/// holds a `Betrayed` grudge of `stiff_grudge × unpaid ÷ owed` on the
/// buyer, a `Betrayed` deed goes round, and `Stiffed` is logged.
fn stiff(world: &mut World, c: &Contract, unpaid: i64, owed: i64) {
    world.stats.current.hit.hm_stiffed += 1;
    let Some(taker) = c.taker else { return };
    let Some(buyer) = c.agent.or(c.buyer) else { return };
    let person = |e: EntityId| world.has::<Identity>(e) && crate::systems::law::living(world, e);
    if !person(taker) || !person(buyer) || taker == buyer {
        return;
    }
    let tick = world.tick;
    let (lo, _) = crate::components::edge_key(buyer, taker);
    let add = i32::try_from(unpaid).unwrap_or(i32::MAX);
    let e = world.edge_entry(buyer, taker);
    // The lower id owes the higher id when `debt > 0`.
    e.debt = e.debt.saturating_add(if lo == buyer { add } else { -add });
    e.debt_since.get_or_insert(tick);
    betrayed(world, buyer, taker);
    let w = world.config.hitmarket.stiff_grudge * unpaid as f32 / owed.max(1) as f32;
    crate::systems::grudges::add(world, taker, buyer, crate::word::GrudgeCause::Betrayed, w, 0);
    // Gaps pass (decision 3): the balance is owed out of future wages; the
    // grudge counts toward a Hunt only once `debt_days` run out unpaid.
    world.hm_stiffs.entry((taker, buyer)).and_modify(|x| x.1 = x.1.max(w)).or_insert((tick, w));
    let text = format!(
        "{} could not pay {} the {unpaid} owed for the {}",
        world.name_of(buyer),
        world.name_of(taker),
        c.kind.label()
    );
    world.push_event(EventKind::Stiffed, &[buyer, taker], text.clone());
    log(world, c.id, text);
    // M16b § 4 Fraud (docs/HIT_MARKET.md's follow-up): it applies to a
    // stiffing buyer as to a reneging one.
    crate::systems::leverage::file_fraud(world, taker, buyer);
}

/// C6 and H6: a `Betrayed` deed (actor `buyer`, object `taker`) in the
/// taker's talk district and first hand in the taker.
pub(crate) fn betrayed(world: &mut World, buyer: EntityId, taker: EntityId) {
    let d = crate::systems::gossip::home_district(world, taker)
        .or_else(|| world.comp::<Position>(taker).map(|p| world.district_of(p.tile)));
    if let Some(d) = d {
        crate::systems::gossip::post_deed(world, d, Deed::Betrayed, Some(buyer), Some(taker));
    }
    if crate::systems::law::living(world, taker) && world.has::<Memory>(taker) {
        let sal = world.config.gossip.deed_sal.get(Deed::Betrayed);
        let e = MemoryEntry {
            subject: Some(buyer),
            salience: sal,
            valence: -world.config.gossip.deed_sev.get(Deed::Betrayed) * sal,
            deed: Some(Deed::Betrayed),
            object: Some(taker),
            ..MemoryEntry::blank(MemoryKind::Rumour, world.tick)
        };
        crate::systems::memory::hear_entry(world, taker, e);
    }
}

pub(super) fn c_cut(world: &World, fixer: EntityId) -> f32 {
    world.comp::<Broker>(fixer).map_or(world.config.fixers.fixer_cut, |k| k.cut)
}

/// The Fixer's revenue: the building's credit and today's income.
pub(super) fn note_cut(world: &mut World, fixer: EntityId, paid: i64) {
    if paid <= 0 {
        return;
    }
    ownership::credit(world, fixer, paid);
    if let Some(k) = world.comp_mut::<Broker>(fixer) {
        k.income_today += paid;
    }
}

/// C6: the buyer did not pay in full: `Reneged`, a `Betrayed` deed (actor
/// the buyer, object the taker) in the taker's talk district and first
/// hand in the taker, and the taker's `Betrayed` grudge on the buyer.
pub(super) fn renege(world: &mut World, id: ContractId, c: &Contract) {
    set_status(world, id, ContractStatus::Reneged, "");
    let Some(taker) = c.taker else { return };
    let Some(buyer) = c.buyer.or(c.agent) else { return };
    betrayed(world, buyer, taker);
    let w = world.config.contracts.renege_grudge;
    if crate::systems::law::living(world, taker) {
        crate::systems::grudges::add(world, taker, buyer, crate::word::GrudgeCause::Betrayed, w, 0);
    }
    // M16b § 4 Fraud: a lawful taker files against the placing agent.
    if let Some(a) = c.agent.or(c.buyer) {
        crate::systems::leverage::file_fraud(world, taker, a);
    }
}

/// C16: a failed attempt (taker dead, jailed or beaten; a strike `Fail`):
/// back to `Open` with `attempts + 1`, taker and crew cleared, until
/// `max_attempts`, then `Failed` with the refund. `ContractFailed`.
pub fn fail_attempt(world: &mut World, id: ContractId, why: &str) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if c.status != ContractStatus::Taken {
        return;
    }
    let attempts = c.attempts.saturating_add(1);
    if let Some(x) = world.contracts.get_mut(&id) {
        x.attempts = attempts;
    }
    // M16b: a coerced job is the coerced's alone: one failed attempt ends it
    // (back on the board it would be a free job for anyone).
    // Phase 2: a gang's unpaid order likewise.
    let unpaid = matches!(c.terms, Terms::Coerced(_) | Terms::Pay { price: 0 });
    if attempts >= world.config.contracts.max_attempts || unpaid {
        settle(world, id, Settle::Failed, why);
        return;
    }
    if let Some(d) = c.due {
        world.ledger_due.remove(&(d, id));
    }
    if let Some(t) = c.taker {
        end_run(world, t, id);
    }
    for &m in &c.crew {
        end_run(world, m, id);
    }
    // Phase 2 (C22): the mission ends; the record goes back on the board.
    end_mission(world, id);
    // Review fix: the departing taker and crew are no longer parties.
    for p in c.taker.into_iter().chain(c.crew.iter().copied()) {
        unparty(world, &c, id, p);
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.status = ContractStatus::Open;
        x.taker = None;
        x.crew.clear();
        x.taken = None;
        x.due = None;
        x.render = Render::Ledger;
        x.guard_since = None;
        x.started = None;
        x.holds = 0;
    }
    if c.kind == ContractKind::Guard {
        reindex_take(world);
    }
    let text = format!("{} on {}: attempt {attempts} failed ({why})", c.kind.label(), world.name_of(c.target.id()));
    // Round 4 T3 (OPEN-222): the crew hears it too.
    let mut actors: SmallVec<[EntityId; 6]> = SmallVec::from_slice(&[c.taker.unwrap_or(EntityId::NONE), c.target.id()]);
    for &m in &c.crew {
        if !actors.contains(&m) {
            actors.push(m);
        }
    }
    world.push_event(EventKind::ContractFailed, &actors, text.clone());
    log(world, id, text);
}

/// C16, from `World::kill_by` (and `remove_agent`), before the dead is
/// unlinked: a target killed by its taker fulfils a Hit or Beat; by other
/// hands (or not by violence) cancels it (a Locate with a paid sighting is
/// fulfilled); a Guard's client dead fails it; a dead taker fails the
/// attempt; a dead buyer cancels (the refund lands in its wallet, its
/// estate's).
pub fn on_death(world: &mut World, dead: EntityId, killer: Option<EntityId>) {
    if world.contracts.is_empty() {
        return;
    }
    let Some(ids) = world.by_party.get(&dead).cloned() else { return };
    for id in ids {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        if !c.is_live() {
            continue;
        }
        let by_taker = killer.is_some_and(|k| c.taker == Some(k) || c.crew.contains(&k));
        if c.target.id() == dead {
            match c.kind {
                ContractKind::Hit | ContractKind::Beat if c.status == ContractStatus::Taken && by_taker => {
                    settle(world, id, Settle::Fulfilled, "");
                }
                ContractKind::Guard | ContractKind::Escort if c.status == ContractStatus::Taken => {
                    settle(world, id, Settle::Failed, "the client died");
                }
                ContractKind::Locate if c.paid_sightings > 0 => settle(world, id, Settle::Fulfilled, ""),
                _ => settle(world, id, Settle::Cancelled, "the target is dead"),
            }
        } else if c.status == ContractStatus::Taken && (c.taker == Some(dead) || c.crew.contains(&dead)) {
            // Phase 2 (C22): a mission loses one of its crew (the last one
            // out fails the attempt); a solo taker's death fails it.
            if world.missions.contains_key(&id) {
                drop_crew(world, id, dead);
            } else {
                fail_attempt(world, id, "the taker died");
            }
        } else if c.buyer == Some(dead) {
            settle(world, id, Settle::Cancelled, "the buyer is gone");
        }
    }
}

/// C19: does `actor` work a live record on `victim` (its run's target)?
pub fn live_job_on(world: &World, actor: EntityId, victim: EntityId) -> bool {
    // Phase 2 (C19, C23): a mission's crew working it (its target, or
    // whoever stands at its door in the brawl) does the record's own violence.
    if let Some(m) = world.mission_of.get(&actor).filter(|_| !world.missions.is_empty()) {
        let on_target = world.contracts.get(m).and_then(|c| c.target_agent()) == Some(victim);
        if on_target || world.comp::<Brain>(actor).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
            return true;
        }
    }
    !world.contract_runs.is_empty() && world.contract_runs.get(&actor).is_some_and(|r| r.target == victim)
}

/// C15: a contract Hit's strike kills at `hunt_kill_p`; a Beat and every
/// other strike at the base rate.
pub fn strike_kill_p(world: &World, taker: EntityId, victim: EntityId) -> Option<f32> {
    let r = world.contract_runs.get(&taker).filter(|r| r.target == victim)?;
    let c = world.contracts.get(&r.contract)?;
    (c.kind == ContractKind::Hit).then_some(world.config.hunt.hunt_kill_p)
}

/// C15, from `hunt::on_strike` (the `Attack` completion): a Beat won is
/// fulfilled, a strike lost fails the attempt; a Hit won whose target
/// lives sends the run back to asking (a death settled at `on_death`).
pub fn on_strike(world: &mut World, taker: EntityId, victim: EntityId, winner: EntityId, died: bool) {
    let Some(r) = world.contract_runs.get(&taker).filter(|r| r.target == victim).cloned() else { return };
    let Some(c) = world.contracts.get(&r.contract).cloned() else { return };
    if c.status != ContractStatus::Taken {
        return;
    }
    if winner != taker {
        if !died {
            fail_attempt(world, c.id, "the taker was beaten");
        }
        return;
    }
    match c.kind {
        ContractKind::Beat => settle(world, c.id, Settle::Fulfilled, ""),
        ContractKind::Hit if !died => reset_chase(world, taker),
        _ => {}
    }
}

/// C32: an on-post Guard taker within 2 tiles of `victim` that an attack
/// on it meets first (the lowest id), not the attacker.
pub fn guard_for(world: &World, attacker: EntityId, victim: EntityId) -> Option<EntityId> {
    if world.contract_guards.is_empty() {
        return None;
    }
    world
        .contract_guards
        .get(&victim)?
        .iter()
        .copied()
        .filter(|&g| g != attacker && crate::systems::law::living(world, g))
        .find(|&g| crate::systems::law::near(world, g, victim, 2))
}

// ---------------------------------------------------------------------------
// The ledger (C18)
// ---------------------------------------------------------------------------

/// C18: a ledger record at `due`: the strike's `p_win` (the solo taker
/// against the target's expected defenders), one draw on
/// `ContractNs::Outcome` (key `id, attempts`). A Hit won settles and opens
/// a pre-bound `Killed` hole through `lod::stat_hit` (the binder names the
/// taker); a Beat won an `Assaulted` hole; a Locate pays `cap_sightings ÷
/// 2` sightings at the target's habit tile. A loss fails the attempt and,
/// below `ledger_death_p`, may cost the taker its life on
/// `ContractNs::TakerDeath` (a hole pre-bound to the target). Phase 2
/// adds the strike decision's Hold and SellOut.
pub fn resolve_ledger(world: &mut World, id: ContractId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if c.status != ContractStatus::Taken || c.render != Render::Ledger {
        return;
    }
    let Some(taker) = c.taker else { return };
    // Fix round: an organic record whose target became a body, or whose
    // party came in front of the player, after it went to the ledger goes
    // back to the live queue rather than roll a death off-screen
    // (`ledger_ok`).
    if !c.motive.is_none() && !ledger_ok(world, &c) && !ledger_kind(world, &c) {
        if let Some(x) = world.contracts.get_mut(&id) {
            x.render = Render::Live;
            x.due = None;
        }
        return;
    }
    // The Hit market's gaps pass: a gang's or a squad's job that waited out
    // `queue_hours` for a live slot is worked here: the crew strikes as one
    // party, its first free member (the strongest, `job_crew`'s order) for
    // a gang's job. A solo taker's party is the taker alone (M16a's ledger).
    let lead = if is_gang(world, taker) { c.crew.iter().copied().find(|&m| free_adult(world, m)) } else { Some(taker) };
    let Some(lead) = lead else {
        fail_attempt(world, id, "the crew is gone");
        return;
    };
    if world.contract_runs.contains_key(&lead) {
        // Review fix: the taker is busy on a live run: look again in an
        // hour (the record stays in the ledger's index).
        let due = world.tick + TICKS_PER_HOUR;
        if let Some(old) = world.contracts.get_mut(&id).and_then(|x| x.due.replace(due)) {
            world.ledger_due.remove(&(old, id));
        }
        world.ledger_due.insert((due, id));
        return;
    }
    if !free_adult(world, lead) {
        fail_attempt(world, id, "the taker was not free");
        return;
    }
    // M17 review: a target away abroad is struck when it is home: look
    // again in a day (the record stays in the ledger's index).
    if let Target::Agent(t) = c.target {
        if crate::systems::abroad::is_away(world, t) {
            let due = world.tick + TICKS_PER_DAY;
            if let Some(old) = world.contracts.get_mut(&id).and_then(|x| x.due.replace(due)) {
                world.ledger_due.remove(&(old, id));
            }
            world.ledger_due.insert((due, id));
            return;
        }
    }
    // M16b phase 1: a Persuade is the taker's one move.
    if c.kind == ContractKind::Persuade {
        crate::systems::leverage::resolve_persuade(world, id, lead);
        return;
    }
    // M16b phase 4: a Tag's plant and a Vote's Intimidate.
    if c.kind == ContractKind::Tag {
        crate::systems::tags::resolve_plant(world, id, lead);
        return;
    }
    if c.kind == ContractKind::Vote {
        crate::systems::governance::resolve_vote(world, id, lead);
        return;
    }
    // M17 phase 3: a Retake's strike at the building.
    if c.kind == ContractKind::Retake {
        crate::systems::parents::resolve_retake(world, id, lead);
        return;
    }
    // M16b phase 2: a Steal (no heist run going) and a Deliver.
    if c.kind == ContractKind::Steal {
        if world.steal_runs.get(&lead) == Some(&id) {
            return;
        }
        crate::systems::captives::resolve_steal(world, id, lead);
        return;
    }
    if c.kind == ContractKind::Deliver {
        crate::systems::captives::resolve_deliver(world, id, lead);
        return;
    }
    if c.kind == ContractKind::Spin {
        return;
    }
    let mut party: SmallVec<[EntityId; 6]> = SmallVec::new();
    party.push(lead);
    party.extend(c.crew.iter().copied().filter(|&m| m != lead && free_adult(world, m)));
    let Target::Agent(t) = c.target else {
        // A ledger Guard cannot occur (a Guard is always live).
        return;
    };
    if !crate::systems::law::living(world, t) {
        settle(world, id, Settle::Cancelled, "the target is dead");
        return;
    }
    let now = world.tick;
    let habit = crate::systems::hunt::habit(world, t, now);
    let district = habit.building.map_or_else(|| world.district_of(habit.tile), |b| world.district_of_building(b));
    if c.kind == ContractKind::Locate {
        let n = (world.config.bounty.cap_sightings / 2).max(1);
        for _ in 0..n {
            if !pay_sighting(world, id, taker, t, habit.tile, true) {
                break;
            }
        }
        // Review fix: the tracker's report settles the record (the cap
        // already did, in `pay_sighting`): Fulfilled with a sighting paid;
        // nothing paid, a direct buyer short is reneged (C6) and any other
        // payer's record goes back on the board.
        let Some(x) = world.contracts.get(&id).filter(|x| x.status == ContractStatus::Taken).cloned() else {
            return;
        };
        if x.paid_sightings > 0 {
            settle(world, id, Settle::Fulfilled, "");
        } else if x.buyer.is_some() && !x.brokered() {
            renege(world, id, &x);
        } else {
            fail_attempt(world, id, "nothing paid for the sighting");
        }
        return;
    }
    // Phase 2 (C18, C24): the strike decision against the habit's district
    // first: a Hold looks again in a day, a SellOut hands the record to the
    // district's gang (live), a Fail is a failed attempt.
    let door = habit.building.and_then(|b| missions::door_of(world, b)).unwrap_or(habit.tile);
    match missions::decide(world, id, &party, door) {
        crate::contract::Decision::Strike => {}
        crate::contract::Decision::Hold => {
            let due = missions::hold_until(now);
            if let Some(old) = world.contracts.get_mut(&id).and_then(|x| x.due.replace(due)) {
                world.ledger_due.remove(&(old, id));
            }
            world.ledger_due.insert((due, id));
            return;
        }
        _ => return,
    }
    let p_win = world
        .contracts
        .get(&id)
        .and_then(|x| x.strike)
        .map_or_else(|| missions::estimate(world, &party, &c.target), |s| s.p_win);
    let attempts = u64::from(c.attempts);
    let win = world.rng.contract(ContractNs::Outcome, id, attempts).random::<f32>() < p_win;
    let src = |faction: EntityId| crate::ledger::ActiveSource {
        source: crate::ledger::ViolenceSource::Contract(id),
        district,
        faction: Some(faction),
        riot: None,
        episode: None,
    };
    if win {
        match c.kind {
            ContractKind::Hit => {
                // Settled first, so the death's `on_death` finds it closed.
                settle(world, id, Settle::Fulfilled, "");
                crate::systems::lod::stat_hit(world, t, crate::components::HoleKind::Killed, Some(&src(lead)));
            }
            _ => {
                crate::systems::lod::stat_hit(world, t, crate::components::HoleKind::Assaulted, Some(&src(lead)));
                settle(world, id, Settle::Fulfilled, "");
            }
        }
        return;
    }
    let lethal = p_win < world.config.missions.ledger_death_p
        && world.rng.contract(ContractNs::TakerDeath, id, attempts).random::<f32>() < 1.0 - p_win;
    fail_attempt(world, id, "the strike was lost");
    // Fix round: an organic record's lost strike is never a body's death
    // off-screen (a Coarse lead loses and lives; M16a's records unchanged).
    let body = !c.motive.is_none() && shown(world, lead);
    if lethal && crate::systems::law::living(world, lead) && !body {
        crate::systems::lod::stat_hit(world, lead, crate::components::HoleKind::Killed, Some(&src(t)));
    }
}
