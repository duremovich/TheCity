use super::*;

// ---------------------------------------------------------------------------
// Price, posting, visibility
// ---------------------------------------------------------------------------

/// The risk a record carries: `1 − estimate(&[MEDIAN], target)` for the
/// violent kinds, 0.1 for Guard and Locate (C7).
pub fn risk_of(world: &World, kind: ContractKind, target: &Target) -> f32 {
    if kind.fights() {
        1.0 - missions::estimate(world, &[missions::MEDIAN], target)
    } else {
        0.1
    }
}

/// The standing the price reads: the target's, a building's owner's (0.3
/// for the city).
fn standing_of(world: &World, target: &Target) -> f32 {
    match *target {
        Target::Agent(t) => crate::systems::reputation::rep(world, t).standing,
        Target::Building(b) => match world.owner_of(b) {
            Some(o) => crate::systems::reputation::rep(world, o).standing,
            None => 0.3,
        },
        // M16b phase 2: a carry is a porter's job; a story's price reads its actor.
        Target::Carry { .. } => 0.0,
        Target::Story { actor, .. } => crate::systems::reputation::rep(world, actor).standing,
    }
}

/// C7: `price_base[kind] × (1 + risk_w × risk) × (1 + standing_w ×
/// standing) × price_level_b` (1.0 for every broker), rounded.
pub fn quote(world: &World, kind: ContractKind, target: &Target, broker: Option<EntityId>) -> i64 {
    let _ = broker;
    let c = &world.config.contracts;
    let base = c.price_base.get(kind) as f32;
    let risk = risk_of(world, kind, target);
    let standing = standing_of(world, target);
    let level = 1.0;
    (base * (1.0 + c.risk_w * risk) * (1.0 + c.standing_w * standing) * level).round().max(1.0) as i64
}

/// Round 5 U5 (the under-18 rule, CLAUDE.md): nobody under 18 is a record's
/// mark (`ages::is_protected`): no Hit, Beat, Abduct, Locate, Persuade or
/// any other job on a child; only a Guard or an Escort (protection) may
/// name one.
pub(super) fn target_ok(world: &World, kind: ContractKind, target: &Target) -> bool {
    match *target {
        // M17 review: an agent away on a job abroad is no target.
        Target::Agent(t) => {
            world.has::<Identity>(t)
                && crate::systems::law::living(world, t)
                && world.has::<Brain>(t)
                && !crate::systems::abroad::is_away(world, t)
                && (matches!(kind, ContractKind::Guard | ContractKind::Escort)
                    || !crate::systems::ages::is_protected(world, t))
        }
        Target::Building(b) => world.comp::<Building>(b).is_some_and(|bd| !bd.demolished),
        // M16b phase 2: a corpse still unburied; a story's actor alive.
        Target::Carry { what, .. } => match what {
            crate::contract::Cargo::Corpse(c) => world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried),
            crate::contract::Cargo::Captive(c) => crate::systems::law::living(world, c),
        },
        Target::Story { actor, .. } => crate::systems::law::living(world, actor),
    }
}

/// A buyer that can still hold coins: the city, a corp, a gang or a living agent.
pub(super) fn buyer_ok(world: &World, buyer: Option<EntityId>) -> bool {
    match buyer {
        None => true,
        Some(b) => {
            world.has::<crate::components::Corp>(b)
                || world.has::<crate::components::Gang>(b)
                || world.has::<crate::components::Wallet>(b)
        }
    }
}

/// The renege draw (C6): direct records placed by an agent only.
fn renege_draw(world: &World, id: ContractId, agent: Option<EntityId>) -> bool {
    let Some(a) = agent else { return false };
    let law = world.comp::<Personality>(a).map_or(0.5, |p| p.lawfulness);
    let honour = crate::systems::reputation::rep(world, a).honour;
    let p = world.config.contracts.renege_base * (1.0 - law) * (1.0 - honour);
    let mut rng = world.rng.contract(ContractNs::Renege, id, 0);
    rng.random::<f32>() < p
}

/// C21: a first-hand `Hired` rumour (actor the buyer, object the target)
/// in `holder`, who joins `known_by`.
pub(crate) fn hear_hired(world: &mut World, id: ContractId, holder: EntityId) {
    let Some(c) = world.contracts.get(&id) else { return };
    // M16b: a Persuade is no hire on its target.
    if !c.kind.illegal() && c.kind != ContractKind::Locate {
        return;
    }
    let (actor, object) = (c.buyer.or(c.agent), c.target.id());
    if let Some(c) = world.contracts.get_mut(&id) {
        if !c.known_by.contains(&holder) {
            c.known_by.push(holder);
        }
    }
    if !world.has::<Memory>(holder) {
        return;
    }
    let sal = world.config.gossip.deed_sal.get(Deed::Hired);
    let e = MemoryEntry {
        subject: actor,
        salience: sal,
        valence: -world.config.gossip.deed_sev.get(Deed::Hired) * sal,
        deed: Some(Deed::Hired),
        object: Some(object),
        hops: 0,
        conf: 1.0,
        ..MemoryEntry::blank(MemoryKind::Rumour, world.tick)
    };
    crate::systems::memory::hear_entry(world, holder, e);
}

/// The one entry point (C1, spec § 1): brains, agents, god commands and
/// (M18) the player post through it. Checks, the price (quoted unless a
/// god's override), escrow for a brokered record (a direct one checks the
/// purse only), the renege draw, the `Hired` first-hand writes, the
/// Fixer's book, the log and `ContractPosted`.
pub fn post(world: &mut World, p: Posting) -> Result<ContractId, Refusal> {
    post_with(world, p, PostExtra::default())
}

/// A fresh Open record (no escrow yet), the one constructor: `post` and the
/// Hit market's price probe build records here.
#[allow(clippy::too_many_arguments)]
pub(super) fn record_of(
    id: ContractId,
    buyer: Option<EntityId>,
    agent: Option<EntityId>,
    kind: ContractKind,
    target: Target,
    broker: Option<EntityId>,
    origin: Origin,
    now: Tick,
    price: i64,
) -> Contract {
    Contract {
        id,
        buyer,
        agent,
        kind,
        target,
        terms: Terms::Pay { price },
        escrow: 0,
        broker,
        posted: now,
        deadline: now,
        origin,
        status: ContractStatus::Open,
        taker: None,
        crew: SmallVec::new(),
        attempts: 0,
        risk: 0.0,
        political: 0.0,
        renege: false,
        due: None,
        render: Render::Ledger,
        known_by: SmallVec::new(),
        price,
        taken: None,
        closed: None,
        paid_sightings: 0,
        last_paid: None,
        strike: None,
        holds: 0,
        sold_out: false,
        charged: false,
        interrogated: SmallVec::new(),
        guard_since: None,
        started: None,
        deposit: 0,
        motive: Motive::None,
        hold: None,
        cut: 0,
        heard: false,
        claimed: None,
    }
}

/// [`post`] with the Hit market's extras (docs/HIT_MARKET.md H4, H13): a
/// deposit put down now (escrowed on the record, brokered or direct) with
/// the balance owed at fulfilment, the motive and a few words
/// for the event text.
pub fn post_with(world: &mut World, p: Posting, extra: PostExtra) -> Result<ContractId, Refusal> {
    let live = world.contracts.values().filter(|c| c.is_live()).count();
    if live >= world.config.contracts.max_open {
        return Err(Refusal::MaxOpen);
    }
    // A Guard may be bought on oneself (C31); any other kind may not.
    let on_self = Some(p.target.id()) == p.buyer && !matches!(p.kind, ContractKind::Guard | ContractKind::Escort);
    // Review fix: only a Guard may name a building (a Hit, Beat or Locate
    // on one has no plan and no ledger row: it would sit Taken to its deadline).
    // M16b phase 2: and a Steal; M17 phase 3: and a Retake.
    let building = matches!(p.target, Target::Building(_))
        && !matches!(p.kind, ContractKind::Guard | ContractKind::Steal | ContractKind::Retake);
    if !target_ok(world, p.kind, &p.target) || on_self || building || !buyer_ok(world, p.buyer) {
        return Err(Refusal::NoTarget);
    }
    if world.contracts.values().any(|c| c.is_live() && c.buyer == p.buyer && c.kind == p.kind && c.target == p.target) {
        return Err(Refusal::Duplicate);
    }
    if let Some(b) = p.broker {
        if !fixer_open(world, b) {
            return Err(Refusal::BrokerClosed);
        }
    }
    let price = p.price.unwrap_or_else(|| quote(world, p.kind, &p.target, p.broker)).max(1);
    // H4: a deposit under the price is put down now; the rest is owed.
    let deposit = extra.deposit.filter(|&d| d > 0 && d < price);
    let down = deposit.unwrap_or(price);
    if world.purse(p.buyer) < down {
        return Err(Refusal::CannotPay);
    }
    let now = world.tick;
    let id = world.next_contract.max(1);
    world.next_contract = id + 1;
    let renege = p.broker.is_none() && renege_draw(world, id, p.agent);
    let deadline = now + Tick::from(p.deadline_days.max(1)) * TICKS_PER_DAY;
    let risk = risk_of(world, p.kind, &p.target);
    let mut rec = record_of(id, p.buyer, p.agent, p.kind, p.target, p.broker, p.origin, now, price);
    rec.deadline = deadline;
    rec.risk = risk;
    rec.renege = renege;
    rec.deposit = deposit.unwrap_or(0);
    rec.motive = extra.motive;
    world.contracts.insert(id, rec);
    // H4 (P2 fix round): a deposit is held on the record, brokered or
    // direct, so a direct buyer cannot spend it before fulfilment.
    if p.broker.is_some() || deposit.is_some() {
        let moved = ownership::escrow_in(world, p.buyer, id, down);
        debug_assert_eq!(moved, down, "the purse was checked");
    }
    index_parties(world, id);
    if let Some(b) = p.broker {
        if let Some(k) = world.comp_mut::<Broker>(b) {
            k.book.push(id);
        }
    }
    if p.kind == ContractKind::Locate {
        if let Target::Agent(t) = p.target {
            world.locate_targets.entry(t).or_default().push(id);
        }
    }
    // C21: first-hand into the placing agent and, brokered, the Fixer's owner.
    if let Some(a) = p.agent {
        hear_hired(world, id, a);
    }
    if let Some(owner) = p.broker.and_then(|b| world.owner_of(b)) {
        if world.has::<Identity>(owner) {
            hear_hired(world, id, owner);
        }
    }
    let row = &mut world.stats.current.contract;
    row.contracts_posted += 1;
    note_kind_posted(world, p.kind);
    let who = world.owner_label(p.buyer);
    let on = world.name_of(p.target.id());
    let at = p.broker.map(|b| format!(" at {}", world.name_of(b))).unwrap_or_default();
    // Round 5 U7 (`OPEN-237`): a guard or an escort is posted for its ward,
    // a bodyguard for oneself as such (V13: "Cato Park posted a guard on Cato
    // Park", a hit's grammar on his own name, `gun_41438 d23 00:09`).
    let mut text = if matches!(p.kind, ContractKind::Guard | ContractKind::Escort) {
        let on = if Some(p.target.id()) == p.buyer {
            match world.comp::<Identity>(p.target.id()).map(|i| i.sex) {
                Some(crate::components::Sex::Female) => "herself".to_string(),
                _ => "himself".to_string(),
            }
        } else {
            on
        };
        format!("{who} posted a {} for {on} for {price}{at}", p.kind.label())
    } else {
        format!("{who} posted a {} on {on} for {price}{at}", p.kind.label())
    };
    if !extra.motive.is_none() {
        let down = deposit.map(|d| format!("{d} down, ")).unwrap_or_default();
        let why = extra.why.as_deref().map(|w| format!(" {w}")).unwrap_or_default();
        text.push_str(&format!(" ({down}{}{why})", extra.motive.label()));
        let h = &mut world.stats.current.hit;
        h.note_posted(extra.motive);
        h.hm_price_sum += price;
        h.hm_deposit_sum += deposit.unwrap_or(price);
    }
    // Round 5 U5 (OPEN-217): a brokered post is on its fixer's book (his
    // diary holds it; V13's fixers never saw a post).
    let hand = p.broker.and_then(|b| crate::systems::service::fixer_hand(world, b));
    if let Some(h) = hand {
        text.push_str(&format!(", on {}'s book", world.name_of(h)));
    }
    let mut actors: SmallVec<[EntityId; 4]> =
        SmallVec::from_slice(&[p.agent.unwrap_or(EntityId::NONE), p.target.id(), p.broker.unwrap_or(EntityId::NONE)]);
    actors.extend(hand.filter(|h| !actors.contains(h)));
    world.push_event(EventKind::ContractPosted, &actors, text.clone());
    log(world, id, text);
    // Lives at work phase 5 (§ 2.4): a brokered post is the placing
    // agent's encounter with the fixer on duty.
    if let (Some(a), Some(b)) = (p.agent, p.broker) {
        crate::systems::service::contract_party(world, a, b);
    }
    Ok(id)
}

/// A record posted, by kind: M16a's four in `ContractCols`, M16b's in
/// `LeverageCols`.
pub(super) fn note_kind_posted(world: &mut World, kind: ContractKind) {
    match kind {
        ContractKind::Persuade => world.stats.current.leverage.k_persuade_posted += 1,
        ContractKind::Trauma => world.stats.current.wounded.trauma_subs_new += 1,
        ContractKind::Tag => world.stats.current.gov.k_tag_posted += 1,
        ContractKind::Vote => world.stats.current.gov.k_vote_posted += 1,
        ContractKind::Retake => world.outside.today.retakes_posted += 1,
        k => match crate::stats::CaptiveCols::slot(k) {
            Some(i) => world.stats.current.captive.k_posted[i] += 1,
            None => world.stats.current.contract.k_posted[k.index()] += 1,
        },
    }
}

/// M16b § 4 (Threat): post a coerced job: `Terms::Coerced(coercion)`, no
/// price, no escrow, direct, taken at once by `taker` (the coerced
/// regular) as any acceptance (`accept`). `None` when the checks of
/// `post_with` refuse it (the record's id is then unused).
pub fn post_coerced(
    world: &mut World,
    p: Posting,
    extra: PostExtra,
    coercion: crate::contract::CoercionId,
    taker: EntityId,
) -> Option<ContractId> {
    post_unpaid(world, p, extra, Terms::Coerced(coercion), taker, "under a threat")
}

/// M16b phase 2: a gang's order to one of its own: an unpaid direct record
/// (`price` 0) its leader hands a member, taken at once (a hostage taken
/// by the holder's members, spec § 4). `None` as [`post_coerced`].
pub fn post_order(world: &mut World, p: Posting, extra: PostExtra, taker: EntityId) -> Option<ContractId> {
    post_unpaid(world, p, extra, Terms::Pay { price: 0 }, taker, "as an order")
}

/// The unpaid records' one constructor (a coerced job, a gang's order).
fn post_unpaid(
    world: &mut World,
    p: Posting,
    extra: PostExtra,
    terms: Terms,
    taker: EntityId,
    how: &str,
) -> Option<ContractId> {
    let live = world.contracts.values().filter(|c| c.is_live()).count();
    if live >= world.config.contracts.max_open || !target_ok(world, p.kind, &p.target) || !buyer_ok(world, p.buyer) {
        return None;
    }
    if world.contracts.values().any(|c| c.is_live() && c.buyer == p.buyer && c.kind == p.kind && c.target == p.target) {
        return None;
    }
    let now = world.tick;
    let id = world.next_contract.max(1);
    world.next_contract = id + 1;
    let mut rec = record_of(id, p.buyer, p.agent, p.kind, p.target, None, p.origin, now, 0);
    rec.terms = terms;
    rec.deadline = now + Tick::from(p.deadline_days.max(1)) * TICKS_PER_DAY;
    rec.risk = risk_of(world, p.kind, &p.target);
    rec.motive = extra.motive;
    world.contracts.insert(id, rec);
    index_parties(world, id);
    if let Some(a) = p.agent {
        hear_hired(world, id, a);
    }
    world.stats.current.contract.contracts_posted += 1;
    note_kind_posted(world, p.kind);
    let text = format!(
        "{} put a {} on {} on {} {how}",
        world.owner_label(p.buyer),
        p.kind.label(),
        world.name_of(p.target.id()),
        world.name_of(taker)
    );
    let actors = [p.agent.unwrap_or(EntityId::NONE), p.target.id(), taker];
    world.push_event(EventKind::ContractPosted, &actors, text.clone());
    log(world, id, text);
    if !accept(world, id, taker, &[]) {
        settle(world, id, Settle::Cancelled, "the coerced could not take it");
        return None;
    }
    Some(id)
}

// --- The brains' postings (C33) ---

/// A brain's posting at the quoted price and the kind's deadline; never a
/// violent record or a Locate on the buyer's own member.
fn brain_post(
    world: &mut World,
    buyer: Option<EntityId>,
    agent: Option<EntityId>,
    kind: ContractKind,
    target: Target,
    broker: Option<EntityId>,
    origin: Origin,
) -> Result<ContractId, Refusal> {
    brain_post_with(world, buyer, agent, kind, target, broker, origin, None, PostExtra::default())
}

/// [`brain_post`] at a given price (`None`: quoted) with the Hit market's
/// extras (a faction pays in full: no deposit).
#[allow(clippy::too_many_arguments)]
pub(crate) fn brain_post_with(
    world: &mut World,
    buyer: Option<EntityId>,
    agent: Option<EntityId>,
    kind: ContractKind,
    target: Target,
    broker: Option<EntityId>,
    origin: Origin,
    price: Option<i64>,
    extra: PostExtra,
) -> Result<ContractId, Refusal> {
    if let (Some(b), Target::Agent(t)) = (buyer, target) {
        if !matches!(kind, ContractKind::Guard | ContractKind::Escort)
            && crate::systems::grudges::member_of(world, t, b)
        {
            return Err(Refusal::NoTarget);
        }
    }
    let deadline_days = world.config.contracts.deadline_days.get(kind);
    let r = post_with(world, Posting { buyer, agent, kind, target, broker, deadline_days, origin, price }, extra);
    note_refused(world, &r);
    r
}

/// Review fix: a brain's posting refused at `max_open` is counted
/// (`contracts_refused_full`).
pub(super) fn note_refused(world: &mut World, r: &Result<ContractId, Refusal>) {
    if matches!(r, Err(Refusal::MaxOpen)) {
        world.stats.current.contract.contracts_refused_full += 1;
    }
}

/// An open Fixer in a district the gang holds (lowest id).
pub(crate) fn fixer_in_held(world: &World, gang: EntityId) -> Option<EntityId> {
    let held: Vec<usize> = crate::systems::gang::held_districts(world, gang).iter().map(|&(d, _)| d.index()).collect();
    open_fixers(world).into_iter().find(|&f| held.contains(&world.district_of_building(f).index()))
}

/// A faction's head: a gang's leader, a corp's exec (living, free).
pub(crate) fn head_of(world: &World, f: EntityId) -> Option<EntityId> {
    let h = world
        .comp::<crate::components::Gang>(f)
        .and_then(|g| g.leader)
        .or_else(|| world.comp::<crate::components::Corp>(f).and_then(|c| c.exec))?;
    (crate::systems::law::living(world, h) && !world.has::<Sentence>(h)).then_some(h)
}

/// C33, from `faction::rescore` when it chooses Raid or Retaliate: a gang
/// weaker than its rival gang (`own ÷ rival` members below `hire_ratio`)
/// whose treasury holds the quote posts a Hit on the rival's leader,
/// brokered with an open Fixer in a held district, else direct. The raid
/// still goes. Plan deviation: gang rivals only (a corp rival has no member
/// count to weigh against).
pub fn post_gang_hit(world: &mut World, gang: EntityId, order: crate::components::Order) {
    let Some(rival) = crate::systems::raid::expedition_rival(world, gang) else { return };
    let (Some(own), Some(theirs)) = (
        world.comp::<crate::components::Gang>(gang).map(|g| g.members.len()),
        world.comp::<crate::components::Gang>(rival).map(|g| g.members.len()),
    ) else {
        return;
    };
    if theirs == 0 || (own as f32 / theirs as f32) >= world.config.contracts.hire_ratio {
        return;
    }
    let Some(leader) = head_of(world, rival) else { return };
    let target = Target::Agent(leader);
    let broker = fixer_in_held(world, gang);
    if world.purse(Some(gang)) < quote(world, ContractKind::Hit, &target, broker) {
        return;
    }
    let agent = world.comp::<crate::components::Gang>(gang).and_then(|g| g.leader);
    let _ = brain_post(world, Some(gang), agent, ContractKind::Hit, target, broker, Origin::GangOrder(order));
}

/// C33, at midnight: a gang in an open vendetta with no sighting of the
/// other side's head (a gang's leader, a corp's exec) younger than
/// `fresh_sighting_hours` in its database posts a Locate on it (direct).
pub fn post_vendetta_locates(world: &mut World) {
    if world.vendettas.is_empty() {
        return;
    }
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    let pairs: Vec<(EntityId, EntityId)> = world.vendettas.iter().flat_map(|v| [(v.a, v.b), (v.b, v.a)]).collect();
    for (g, f) in pairs {
        if !world.has::<crate::components::Gang>(g) {
            continue;
        }
        let Some(head) = head_of(world, f) else { continue };
        let fresh = world.db.get(&g).is_some_and(|db| db.sightings.iter().any(|s| s.who == head && s.tick >= horizon));
        if fresh {
            continue;
        }
        let target = Target::Agent(head);
        if world.purse(Some(g)) < quote(world, ContractKind::Locate, &target, None) {
            continue;
        }
        let (agent, order) = world
            .comp::<crate::components::Gang>(g)
            .map_or((None, crate::components::Order::Retaliate), |x| (x.leader, x.order));
        let _ = brain_post(world, Some(g), agent, ContractKind::Locate, target, None, Origin::GangOrder(order));
        // M16b phase 4: a tag on the head too, when the gang can pay the
        // record and the device.
        crate::systems::tags::post_tag(world, g, agent, head, Origin::GangOrder(order));
    }
}

/// C33, from `corp_brain::secure`: a loss building it cannot cover with a
/// guard contract gets a `Guard` record, brokered with the open Fixer
/// nearest its door (else direct). True when posted (or already open).
pub fn post_secure(world: &mut World, corp: EntityId, building: EntityId) -> bool {
    let Some(door) = world.comp::<Building>(building).map(|b| b.door) else { return false };
    let broker = nearest_fixer_to(world, door);
    let exec = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec);
    let target = Target::Building(building);
    let origin = Origin::CorpOrder(crate::components::CorpOrder::Secure);
    matches!(
        brain_post(world, Some(corp), exec, ContractKind::Guard, target, broker, origin),
        Ok(_) | Err(Refusal::Duplicate)
    )
}

/// C33, from `corp_brain::secure`: a named culprit gang's leader gets a
/// Locate, brokered with the open Fixer nearest `near` (else direct).
pub fn post_culprit_locate(world: &mut World, corp: EntityId, culprit: EntityId, near: TilePos) {
    let Some(head) = head_of(world, culprit) else { return };
    let broker = nearest_fixer_to(world, near);
    let exec = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec);
    let origin = Origin::CorpOrder(crate::components::CorpOrder::Secure);
    let _ = brain_post(world, Some(corp), exec, ContractKind::Locate, Target::Agent(head), broker, origin);
    // M16b phase 4: and a tag on the culprit's head when the corp can pay.
    crate::systems::tags::post_tag(world, corp, exec, head, origin);
}

/// C33, from `corp_brain::lobby`: an exec below `dirty_lobby` posts a Beat
/// (a Hit below `dirty_lobby ÷ 2`) on the culprit gang's leader instead of
/// the bribe, brokered with the open Fixer nearest the leader. True when
/// the record stands (posted now or already open): the bribe is skipped.
pub fn post_lobby(world: &mut World, corp: EntityId, gang: EntityId) -> bool {
    let Some(exec) = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec) else { return false };
    let law = world.comp::<Personality>(exec).map_or(1.0, |p| p.lawfulness);
    let dirty = world.config.contracts.dirty_lobby;
    if law >= dirty {
        return false;
    }
    let kind = if law < dirty / 2.0 { ContractKind::Hit } else { ContractKind::Beat };
    let Some(head) = head_of(world, gang) else { return false };
    let Some(at) = world.comp::<Position>(head).map(|p| p.tile) else { return false };
    let Some(broker) = nearest_fixer_to(world, at) else { return false };
    // The Hit market (P3 follow-up): the faction repost wait. A record on
    // this head still live stands (the bribe is skipped); one closed within
    // `repost_days` is not bought again (the bribe goes ahead).
    if world.config.hitmarket.enabled && crate::systems::hitmarket::bought_lately(world, corp, head) {
        return world
            .contracts
            .values()
            .any(|c| c.is_live() && c.buyer == Some(corp) && c.target == Target::Agent(head));
    }
    // The Hit market (H11, review fix): the culprit record is organic, so
    // the day's brake holds it (`max_open`, `posts_per_day`); past the brake
    // nothing is posted and the bribe goes ahead.
    if world.config.hitmarket.enabled && !crate::systems::hitmarket::room(world) {
        return false;
    }
    let origin = Origin::CorpOrder(crate::components::CorpOrder::Lobby);
    let target = Target::Agent(head);
    // The Hit market (H18): at the market price, the motive on the record.
    let (price, extra) = if world.config.hitmarket.enabled {
        let p = organic_price(world, kind, &target, Some(corp), Some(exec), Some(broker));
        (Some(p), PostExtra { deposit: None, motive: Motive::Culprit, why: None })
    } else {
        (None, PostExtra::default())
    };
    matches!(
        brain_post_with(world, Some(corp), Some(exec), kind, target, Some(broker), origin, price, extra),
        Ok(_) | Err(Refusal::Duplicate)
    )
}

/// C33, from `law_brain::run`'s daily pass: the law's public Locates
/// (buyer `None`, placed by the captain, paid from the Treasury) on a
/// wanted suspect unseen (`World::last_seen`, else its oldest open report)
/// for `law_bounty_days` (1 under `levers.public_bounties`), or an escaped
/// convict (an `Escaped` memory within two days); and, under a captain
/// below `death_squad_lawfulness`, a brokered Hit on the hottest gang's
/// leader (`faction::heat`, ties the lower id), escrowed from the Treasury.
/// Plan deviations: one new public Locate a day (the most severe open
/// crime first, then the longest unseen; every wanted thief unseen for
/// three days put ~11 a day on the board by day 100 and filled
/// `max_open`). Treasury cover (`cover = cap_sightings × per_sighting`,
/// what one public bounty can still charge): a new public Locate only
/// while the purse is at least `(open public Locates + 1) × cover`, a
/// death-squad Hit only while it is at least its price plus `open public
/// Locates × cover` (the Hit's escrow leaves the purse when posted, and so
/// has every open Law Hit's). Not checked: the city's other spending
/// between postings.
pub fn post_law(world: &mut World) {
    let Some(captain) = world.law().and_then(|l| l.captain).filter(|&c| crate::systems::law::living(world, c)) else {
        return;
    };
    let now = world.tick;
    let days = if world.levers.public_bounties { 1 } else { world.config.contracts.law_bounty_days };
    let unseen = Tick::from(days) * TICKS_PER_DAY;
    // Per suspect: the most severe open crime and the oldest open report.
    let mut open: std::collections::BTreeMap<EntityId, (crate::components::Crime, Tick)> =
        std::collections::BTreeMap::new();
    for r in world.crime_reports().iter().filter(|r| !r.resolved) {
        let e = open.entry(r.suspect).or_insert((r.crime, r.tick));
        *e = (e.0.max(r.crime), e.1.min(r.tick));
    }
    let mut due: Vec<(std::cmp::Reverse<crate::components::Crime>, Tick, EntityId)> = Vec::new();
    for (s, (crime, reported)) in open {
        if !crate::systems::law::living(world, s) || world.has::<Sentence>(s) || !world.has::<Brain>(s) {
            continue;
        }
        let seen = world.last_seen().get(&s).map_or(reported, |&(_, t)| t.max(reported));
        let escaped = world.comp::<Memory>(s).is_some_and(|m| {
            m.entries.iter().any(|e| e.kind == MemoryKind::Escaped && now.saturating_sub(e.tick) < 2 * TICKS_PER_DAY)
        });
        if escaped || now.saturating_sub(seen) >= unseen {
            due.push((std::cmp::Reverse(crime), seen, s));
        }
    }
    // One new public bounty a day: the most severe crime, then the longest
    // unseen, then the lower id.
    due.sort_unstable();
    let cover = i64::from(world.config.bounty.cap_sightings) * world.config.bounty.per_sighting;
    let public =
        world.contracts.values().filter(|c| c.is_live() && c.buyer.is_none() && c.kind == ContractKind::Locate).count()
            as i64;
    if world.purse(None) >= (public + 1) * cover {
        for (_, _, s) in due {
            let posted =
                brain_post(world, None, Some(captain), ContractKind::Locate, Target::Agent(s), None, Origin::Law);
            if posted.is_ok() {
                break;
            }
        }
    }
    let law = world.comp::<Personality>(captain).map_or(1.0, |p| p.lawfulness);
    if law >= world.config.contracts.death_squad_lawfulness {
        return;
    }
    let hottest = world
        .gangs()
        .into_iter()
        .filter_map(|g| head_of(world, g).map(|h| (crate::systems::faction::heat(world, g), g, h)))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let Some((heat, _, head)) = hottest else { return };
    if heat <= 0.0 {
        return;
    }
    let Some(at) = world.comp::<Position>(head).map(|p| p.tile) else { return };
    let Some(broker) = nearest_fixer_to(world, at) else { return };
    // Review fix: the Hit's escrow leaves the Treasury at once; it is posted
    // only while the Treasury still covers the open public bounties after
    // it (open Law Hits' escrow has already left the purse).
    let target = Target::Agent(head);
    let price = quote(world, ContractKind::Hit, &target, Some(broker));
    let public =
        world.contracts.values().filter(|c| c.is_live() && c.buyer.is_none() && c.kind == ContractKind::Locate).count()
            as i64;
    if world.purse(None) < price + public * cover {
        return;
    }
    let _ = brain_post(world, None, Some(captain), ContractKind::Hit, target, Some(broker), Origin::Law);
}
