use super::*;

/// The kinds worked on the ledger at any tier (`render_for`'s first test):
/// no live plan works them. M16b phase 1: a Persuade is a social move
/// resolved at any tier; phase 2: a Deliver and a Spin likewise, a Steal
/// unless its heist runs (`captives::start_heist`, its own run). Phase 4: a
/// Vote's Intimidate likewise; a Tag too, unless its party is shown (Shadow
/// V4 bug 15: a pinned gun was paid 261 c for a plant made while he rested;
/// shown, he stakes the mark out and plants at contact). M17 phase 3: a
/// Retake is one strike draw at the building. Round 4 T3 (`OPEN-222`): a
/// body taker or target never turns such a record live (`on_promoted`,
/// `resolve_ledger`): V12's pinned `gun_36512` held a Steal on a Lab the
/// Contract plan could not walk to, cooled it "7 in a row" and let it expire.
pub(super) fn ledger_kind(world: &World, c: &Contract) -> bool {
    matches!(
        c.kind,
        ContractKind::Persuade
            | ContractKind::Deliver
            | ContractKind::Spin
            | ContractKind::Steal
            | ContractKind::Vote
            | ContractKind::Retake
    ) || (c.kind == ContractKind::Tag && !party_shown(world, c))
}

/// C17: `Live` when the target or the taker is a body, pinned or on
/// screen, or the record is a Guard taken by an agent; else `Ledger`.
pub fn render_for(world: &World, id: ContractId) -> Render {
    let Some(c) = world.contracts.get(&id) else { return Render::Ledger };
    if ledger_kind(world, c) {
        return Render::Ledger;
    }
    // Phase 2: an Escort is a Guard on a person; an Abduct is never ledger.
    if matches!(c.kind, ContractKind::Guard | ContractKind::Escort | ContractKind::Abduct)
        && c.taker.is_some_and(|t| world.has::<Identity>(t))
    {
        return Render::Live;
    }
    // Phase 2 (C22, C25, C26): a squad or a gang's job is always live.
    if is_mission(world, c) {
        return Render::Live;
    }
    if party_shown(world, c) {
        Render::Live
    } else {
        Render::Ledger
    }
}

/// C17: is any of record `c`'s taker, crew or agent target a body (not
/// Statistical, or pinned) or on screen? (`render_for`'s test; the Hit
/// market's ledger fallback never rolls such a record off-screen.)
pub fn party_shown(world: &World, c: &Contract) -> bool {
    let mut ids: SmallVec<[EntityId; 6]> = SmallVec::new();
    ids.extend(c.taker);
    ids.extend(c.crew.iter().copied());
    if let Target::Agent(t) = c.target {
        ids.push(t);
    }
    ids.into_iter().any(|e| shown(world, e))
}

/// C17's body test for one entity: not Statistical, pinned, or on screen.
pub(super) fn shown(world: &World, e: EntityId) -> bool {
    world.comp::<Brain>(e).is_some_and(|b| b.lod != Lod::Statistical) || seen(world, e)
}

/// At Full, pinned, or on screen: in front of the player.
pub(super) fn seen(world: &World, e: EntityId) -> bool {
    world.comp::<Brain>(e).is_some_and(|b| b.lod == Lod::Full || b.pinned)
        || world.view_rect.zip(world.comp::<Position>(e)).is_some_and(|(r, p)| r.contains(p.tile))
}

/// The Hit market (fix round): may organic record `c` be worked on the
/// ledger: its target is no body (`shown`: a ledger kill is a body's death
/// with no scene), and none of its taker or crew is in front of the player
/// (`seen`). A taker or crew member walking off-screen at Coarse may strike
/// from the ledger (a gang's crew mostly is: the LOD pass promotes members);
/// a lost strike's death never falls on a body (`resolve_ledger`).
pub(super) fn ledger_ok(world: &World, c: &Contract) -> bool {
    !c.target_agent().is_some_and(|t| shown(world, t))
        && !c.taker.into_iter().chain(c.crew.iter().copied()).any(|e| seen(world, e))
}

/// Live records being worked now (each one run or one mission, C17;
/// phase 2: `max_missions` counts both).
pub(super) fn live_count(world: &World) -> usize {
    world.contract_runs.len() + world.missions.len()
}

/// C35 (phase 4): the bodies a record holds while it is worked live: an
/// agent taker, the crew and a living agent target.
pub fn bodies_of(world: &World, c: &Contract) -> SmallVec<[EntityId; 6]> {
    let mut v: SmallVec<[EntityId; 6]> = SmallVec::new();
    let mut add = |e: EntityId| {
        if !v.contains(&e) {
            v.push(e);
        }
    };
    if let Some(t) = c.taker.filter(|&t| world.has::<Identity>(t)) {
        add(t);
    }
    for &m in &c.crew {
        add(m);
    }
    if let Some(t) = c.target_agent().filter(|&t| crate::systems::law::living(world, t)) {
        add(t);
    }
    v
}

/// C35: the records being worked now (a run or a mission), by price
/// descending (ties the lower id).
fn running_by_price(world: &World) -> Vec<ContractId> {
    let mut ids: Vec<(i64, ContractId)> = world
        .contract_runs
        .values()
        .map(|r| r.contract)
        .chain(world.missions.keys().copied())
        .filter_map(|id| world.contracts.get(&id).map(|c| (-c.price, id)))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().map(|(_, id)| id).collect()
}

/// C35 (phase 4): `lod::BudgetSets.contract`: the parties (taker, crew,
/// target) of the records worked live, whole records by price descending
/// (ties the lower id) while the set stays within `[lod] contract_quota`.
/// Admission (`has_room`) keeps every running record inside it.
pub fn contract_set(world: &World) -> std::collections::BTreeSet<EntityId> {
    let mut set = std::collections::BTreeSet::new();
    if world.contract_runs.is_empty() && world.missions.is_empty() {
        return set;
    }
    let quota = world.config.lod.contract_quota;
    for id in running_by_price(world) {
        let Some(c) = world.contracts.get(&id) else { continue };
        let bodies = bodies_of(world, c);
        let fresh = bodies.iter().filter(|b| !set.contains(*b)).count();
        if set.len() + fresh > quota {
            continue;
        }
        set.extend(bodies);
    }
    set
}

/// C35 (phase 4): `lod::set_lod` refuses to demote a member of the
/// contract set (as runners and hunters).
pub fn holds_body(world: &World, id: EntityId) -> bool {
    if world.contract_runs.is_empty() && world.missions.is_empty() {
        return false;
    }
    // Phase 4 review: a running record's taker or crew is never demoted
    // mid-run, even should `contract_set` skip its record.
    world.contract_runs.contains_key(&id) || world.mission_of.contains_key(&id) || contract_set(world).contains(&id)
}

/// C17, C35: may record `c` start being worked live now: fewer than
/// `max_missions` running, and its bodies fit inside `contract_quota`
/// beside the running records' (else it stays queued by price).
pub(super) fn has_room(world: &World, c: &Contract) -> bool {
    if live_count(world) >= world.config.missions.max_missions && !jumps_queue(world, c) {
        return false;
    }
    let set = contract_set(world);
    let fresh = bodies_of(world, c).iter().filter(|b| !set.contains(*b)).count();
    set.len() + fresh <= world.config.lod.contract_quota
}

/// C17: start a live record's run: the taker promoted to Coarse, its
/// chase opened (`Ask` for Hit, Beat and Locate; `Watch` at the client for
/// a Guard). `false` when `max_missions` runs are going (it stays queued).
pub(super) fn start_live(world: &mut World, id: ContractId) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    // Phase 2 (C22): a squad or a gang's job marches as a mission.
    if is_mission(world, &c) {
        return open_mission(world, id);
    }
    let Some(taker) = c.taker.filter(|&t| world.has::<Identity>(t)) else { return false };
    if world.contract_runs.contains_key(&taker) {
        return true;
    }
    // C35 (phase 4): the run count and the bodies' quota.
    if !has_room(world, &c) {
        return false;
    }
    let now = world.tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.render = Render::Live;
        x.due = None;
        x.started = x.started.or(Some(now));
    }
    let (phase, intel) = match (c.kind, c.target) {
        (ContractKind::Guard, Target::Building(b)) => (
            HuntPhase::Watch,
            world.comp::<Building>(b).map(|bd| Intel { building: Some(b), tile: bd.door, source: IntelSource::Habit }),
        ),
        (ContractKind::Guard, Target::Agent(a)) => (HuntPhase::Watch, Some(crate::systems::hunt::habit(world, a, now))),
        _ => (HuntPhase::Ask, None),
    };
    world.contract_runs.insert(
        taker,
        ContractRun {
            contract: id,
            target: c.target.id(),
            since: now,
            phase,
            venue: None,
            intel,
            stakeout_until: None,
            deceived: false,
            liar: None,
            worked: 0,
            tries: 0,
        },
    );
    if world.comp::<Brain>(taker).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, taker, Lod::Coarse);
    }
    crate::systems::hunt::reindex(world);
    true
}

/// C12, C17: `taker` (and `crew`) take record `id`: `Taken`, `known_by`
/// and the `Hired` first-hand copies, the render, a ledger `due` (drawn on
/// `ContractNs::Due`) or the live run. `ContractTaken`.
pub fn accept(world: &mut World, id: ContractId, taker: EntityId, crew: &[EntityId]) -> bool {
    accept_at(world, id, taker, crew, None)
}

/// [`accept`], `walker` the body who came to the Fixer's desk and took it in
/// person (round 9 Z2: the taker, or a crew's leader for its gang): the line
/// says so and names the price and the one behind the desk.
pub(super) fn accept_at(
    world: &mut World,
    id: ContractId,
    taker: EntityId,
    crew: &[EntityId],
    walker: Option<EntityId>,
) -> bool {
    let Some(c) = world.contracts.get(&id) else { return false };
    if c.status != ContractStatus::Open {
        return false;
    }
    let now = world.tick;
    if let Some(c) = world.contracts.get_mut(&id) {
        c.status = ContractStatus::Taken;
        c.taker = Some(taker);
        c.crew = crew.iter().copied().collect();
        c.taken = Some(now);
        if c.kind == ContractKind::Guard {
            c.guard_since = Some(now);
        }
    }
    index_parties(world, id);
    // C31: a city guard on the take keeps its shift: no run, no ledger.
    if crate::systems::law::is_city_guard(world, taker)
        && world.contracts.get(&id).is_some_and(|c| is_corruption(world, c))
    {
        take_on(world, id, taker);
        return true;
    }
    hear_hired(world, id, taker);
    for &m in crew {
        hear_hired(world, id, m);
    }
    let render = render_for(world, id);
    let tag = match render {
        Render::Live => {
            if let Some(c) = world.contracts.get_mut(&id) {
                c.render = Render::Live;
            }
            if start_live(world, id) {
                "LIVE"
            } else {
                "QUEUED"
            }
        }
        _ => {
            let (taken, deadline) = world.contracts.get(&id).map_or((now, now), |c| (now, c.deadline));
            let span = (deadline.saturating_sub(taken) / 2).max(60);
            let u: f64 = world.rng.contract(ContractNs::Due, id, 0).random();
            let due = taken + (u * span as f64).floor() as Tick;
            if let Some(c) = world.contracts.get_mut(&id) {
                c.render = Render::Ledger;
                c.due = Some(due);
            }
            world.ledger_due.insert((due, id));
            "LEDGER"
        }
    };
    // M16b phase 2: a Steal on a Lab with a node is a heist run.
    if world.contracts.get(&id).is_some_and(|c| c.kind == ContractKind::Steal) {
        crate::systems::captives::start_heist(world, id, taker);
    }
    // Phase 1 sends every Hit solo; a weak solo gun is counted (C25).
    if let Some(c) = world.contracts.get(&id) {
        if c.kind == ContractKind::Hit
            && c.crew.is_empty()
            && missions::estimate(world, &[taker], &c.target) < world.config.missions.squad_below
        {
            world.stats.current.contract.hits_solo_weak += 1;
        }
    }
    let (kind, target) =
        world.contracts.get(&id).map(|c| (c.kind, c.target.id())).unwrap_or((ContractKind::Hit, taker));
    let crew_txt = if crew.is_empty() { String::new() } else { format!(", crew {}", crew.len()) };
    // Round 5 U5 (OPEN-217): a brokered take is the fixer's pick from his
    // regulars: he and the crew are named, and his diary holds it.
    // Round 9 Z2: a walk-in's is the one behind the desk.
    let broker = world.contracts.get(&id).and_then(|c| c.broker);
    let hand = broker.and_then(|b| {
        walker
            .and_then(|w| crate::systems::service::desk_server(world, b, w))
            .or_else(|| crate::systems::service::fixer_hand(world, b))
            .map(|h| (h, b))
    });
    let names: Vec<String> = crew.iter().map(|&m| world.name_of(m)).collect();
    let text = match (walker, hand) {
        (Some(w), _) => {
            let price = world.contracts.get(&id).map_or(0, |c| c.price);
            let at = broker.map(|b| world.name_of(b)).unwrap_or_default();
            let gang = if w == taker { String::new() } else { format!(" for {}", world.owner_label(Some(taker))) };
            let with = if names.is_empty() { String::new() } else { format!(", crew {}", names.join(", ")) };
            let from = hand.map(|(h, _)| format!(", from {}", world.name_of(h))).unwrap_or_default();
            format!(
                "{} came to {at} in person and took the {} on {} for {price}c{gang} ({tag}{with}){from}",
                world.name_of(w),
                kind.label(),
                world.name_of(target)
            )
        }
        (None, Some((h, b))) => {
            let with = if names.is_empty() { String::new() } else { format!(" with {}", names.join(", ")) };
            format!(
                "{} took the {} on {} ({tag}{crew_txt}), put on it{with} by {} at {}",
                world.owner_label(Some(taker)),
                kind.label(),
                world.name_of(target),
                world.name_of(h),
                world.name_of(b)
            )
        }
        (None, None) => format!(
            "{} took the {} on {} ({tag}{crew_txt})",
            world.owner_label(Some(taker)),
            kind.label(),
            world.name_of(target)
        ),
    };
    let mut actors: SmallVec<[EntityId; 6]> = SmallVec::from_slice(&[taker, target]);
    for x in walker.into_iter().chain(hand.map(|(h, _)| h)) {
        if !actors.contains(&x) {
            actors.push(x);
        }
    }
    world.push_event(EventKind::ContractTaken, &actors, text.clone());
    log(world, id, text);
    heard_of(world, id, taker);
    // Lives at work phase 5 (§ 2.4): a brokered take is the taker's
    // encounter with the fixer on duty (round 9 Z2: the walker's, in person).
    if let Some(b) = world.contracts.get(&id).and_then(|c| c.broker) {
        crate::systems::service::contract_party(world, walker.unwrap_or(taker), b);
    }
    true
}
