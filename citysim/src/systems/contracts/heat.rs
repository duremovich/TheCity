use super::*;

// --- Fixer heat, the bust, the bribe (C30) ---

/// C30: a Fixer's heat moves by `by` (clamped 0..1; floored at 0.5 while the
/// licence lever is off).
pub fn heat_add(world: &mut World, fixer: EntityId, by: f32) {
    let floor = if world.levers.fixer_licence { 0.0 } else { 0.5 };
    if let Some(k) = world.comp_mut::<Broker>(fixer) {
        k.heat = (k.heat + by).clamp(floor, 1.0);
    }
}

/// C30, from `faction::offer_bribe`'s `Payer::Owner` arm on a taken bribe:
/// every Fixer `owner` owns cools by `bribe_cool`.
pub fn heat_bribed(world: &mut World, owner: EntityId) {
    let cool = world.config.law.bribe_cool;
    let mine: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Fixer)
        .iter()
        .copied()
        .filter(|&f| world.owner_of(f) == Some(owner))
        .collect();
    for f in mine {
        heat_add(world, f, -cool);
    }
}

/// C30: the law closes a Fixer's office: a Conspiracy report on the owner,
/// the owner last seen at the door, `closed_until = now + close_days`, every
/// Open record in the book cancelled with its refund (Taken ones run on),
/// the heat back to the floor. `FixerBusted`.
pub fn heat_bust(world: &mut World, fixer: EntityId) {
    let now = world.tick;
    let days = world.config.law.close_days;
    let owner = world.owner_of(fixer).filter(|&o| world.has::<Identity>(o));
    let door = world.comp::<Building>(fixer).map(|b| b.door).unwrap_or_default();
    if let Some(o) = owner.filter(|&o| crate::systems::law::living(world, o)) {
        crate::systems::law::file_report(world, crate::components::Crime::Conspiracy, o, None);
        world.set_last_seen(o, (door, now));
    }
    let open: Vec<ContractId> = world
        .comp::<Broker>(fixer)
        .map(|k| k.book.iter().copied().filter(|id| world.contracts.get(id).is_some_and(|c| c.is_open())).collect())
        .unwrap_or_default();
    // Review fix: the bust spends the heat (down to the licence floor), so
    // the office is not busted again the day it reopens.
    let floor = if world.levers.fixer_licence { 0.0 } else { 0.5 };
    if let Some(k) = world.comp_mut::<Broker>(fixer) {
        k.closed_until = Some(now + Tick::from(days) * TICKS_PER_DAY);
        k.heat = k.heat.min(floor);
    }
    for id in open {
        settle(world, id, Settle::Cancelled, "the Fixer was closed by the law");
    }
    let d = world.district_name(world.district_of_building(fixer)).to_string();
    // Round 6 V3 (`OPEN-238`): the staff go home (`routine::workday_of`
    // reads the shut office), so the line names them.
    let staff = world.staff_of(fixer).len();
    let sent = if staff > 0 { format!("; its staff of {staff} sent home") } else { String::new() };
    let text = format!("the {d} Fixer office was closed by the law for {days} days{sent}");
    world.push_event(EventKind::FixerBusted, &[fixer, owner.unwrap_or(EntityId::NONE)], text.clone());
    log(world, 0, text);
}

/// C30, at midnight: every Fixer's heat decays `heat_decay` (floored at 0.5
/// while `levers.fixer_licence` is off); an open office at `warrant_heat`
/// is busted; an owner at `corrupt_heat` or more scores a bribe (`heat` →
/// Linear{0.8, 0.2} × `1 − lawfulness` → Linear{0.5, 0.5}) and at
/// `bribe_min` offers it (`Payer::Owner`). Plan deviation: no offer while
/// the law is hardened (`Law.hardened_until`; a refused captain would
/// refuse again, and each refusal is a `BribeRefused` shock).
pub fn heat_daily(world: &mut World) {
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    let decay = world.config.law.heat_decay;
    for &f in &fixers {
        heat_add(world, f, -decay);
    }
    let warrant = world.config.law.warrant_heat;
    for &f in &fixers {
        if fixer_open(world, f) && world.comp::<Broker>(f).is_some_and(|k| k.heat >= warrant) {
            heat_bust(world, f);
        }
    }
    let now = world.tick;
    let hardened = world.law().and_then(|l| l.hardened_until).is_some_and(|t| t > now);
    if hardened {
        return;
    }
    let (corrupt, min) = (world.config.law.corrupt_heat, world.config.fixers.bribe_min);
    let mut offered: Vec<EntityId> = Vec::new();
    for &f in &fixers {
        let Some(owner) = world.owner_of(f).filter(|&o| world.has::<Identity>(o)) else { continue };
        if offered.contains(&owner) || !crate::systems::law::living(world, owner) || world.has::<Sentence>(owner) {
            continue;
        }
        let heat = world.comp::<Broker>(f).map_or(0.0, |k| k.heat);
        if heat < corrupt {
            continue;
        }
        let law = world.comp::<Personality>(owner).map_or(1.0, |p| p.lawfulness);
        let score = Curve::Linear { m: 0.8, b: 0.2 }.eval(heat) * Curve::Linear { m: 0.5, b: 0.5 }.eval(1.0 - law);
        if score < min {
            continue;
        }
        offered.push(owner);
        crate::systems::faction::offer_bribe(
            world,
            crate::systems::faction::Payer::Owner(owner),
            crate::systems::faction::BribeAsk::LookAway,
            score,
        );
        if world.law().and_then(|l| l.hardened_until).is_some_and(|t| t > now) {
            break;
        }
    }
}

// --- Guard corruption (C31) ---

/// C31: a direct `Guard` its buyer bought on itself (or its placing agent,
/// or the placing agent's Fixer office): the record a city guard may take.
/// Deviation: `corruption_daily` posts on agents only (the buyer or its
/// placing agent); the Fixer-office branch is never produced in 16a.
pub fn is_corruption(world: &World, c: &Contract) -> bool {
    if c.kind != ContractKind::Guard || c.broker.is_some() {
        return false;
    }
    let t = c.target.id();
    Some(t) == c.buyer
        || Some(t) == c.agent
        || (world.comp::<Building>(t).is_some_and(|b| b.kind == BuildingKind::Fixer)
            && world.owner_of(t).is_some_and(|o| Some(o) == c.agent))
}

/// The district a corruption record's buyer is in: the placing agent's (or
/// the buyer's) Home district, else where it stands.
fn buyer_district(world: &World, c: &Contract) -> Option<crate::components::DistrictId> {
    let who = c.agent.or(c.buyer)?;
    crate::systems::gossip::home_district(world, who)
        .or_else(|| world.comp::<Position>(who).map(|p| world.district_of(p.tile)))
}

/// C11, C31: a city guard on shift whose beat (else where it stands) is the
/// buyer's district sees a corruption record.
pub(super) fn guard_viewer(world: &World, c: &Contract, viewer: EntityId) -> bool {
    if !crate::systems::law::is_city_guard(world, viewer) || !is_corruption(world, c) {
        return false;
    }
    let tod = time::tick_of_day(world.tick);
    if !world.comp::<Job>(viewer).is_some_and(|j| j.on_shift(tod)) {
        return false;
    }
    let beat = world
        .law()
        .and_then(|l| l.beats.get(&viewer).copied())
        .or_else(|| world.comp::<Position>(viewer).map(|p| world.district_of(p.tile)));
    beat.is_some() && beat == buyer_district(world, c)
}

/// C31: a city guard may take a corruption record: lawfulness under
/// `corrupt_lawfulness`, and not escorting the buyer (a live arrest).
pub(super) fn corrupt_ok(world: &World, c: &Contract, guard: EntityId) -> bool {
    if !crate::systems::law::is_city_guard(world, guard) || !is_corruption(world, c) {
        return false;
    }
    let law = world.comp::<Personality>(guard).map_or(1.0, |p| p.lawfulness);
    let escorting = world.comp::<Brain>(guard).and_then(|b| b.escorting);
    law < world.config.law.corrupt_lawfulness
        && escorting.is_none_or(|e| Some(e) != c.buyer && Some(e) != c.agent && !buyer_member(world, c, e))
}

/// Is `id` a member of a gang buyer?
fn buyer_member(world: &World, c: &Contract, id: EntityId) -> bool {
    c.buyer.is_some_and(|b| world.has::<crate::components::Gang>(b) && world.gang_of(id) == Some(b))
}

/// C31: is `guard` on the take for `suspect` (the suspect, or a member of a
/// gang buyer)? OPEN-112: whether the taker is a city guard and the record
/// corruption are read now, not at the last reindex (a guard who lost the
/// job between two reindexes stayed on the take in a live world and not in
/// a loaded one).
pub fn on_take_of(world: &World, guard: EntityId, suspect: EntityId) -> bool {
    if world.on_take.is_empty() {
        return false;
    }
    take_buyers(world, guard)
        .any(|b| b == suspect || (world.has::<crate::components::Gang>(b) && world.gang_of(suspect) == Some(b)))
}

/// C31: the buyers (and placing agents) `guard` is on the take for: its
/// Taken corruption records while it is a city guard.
pub fn take_buyers(world: &World, guard: EntityId) -> impl Iterator<Item = EntityId> + '_ {
    let on = crate::systems::law::is_city_guard(world, guard);
    world
        .on_take
        .get(&guard)
        .into_iter()
        .flatten()
        .filter(move |_| on)
        .filter_map(|id| world.contracts.get(id))
        .filter(move |c| c.status == ContractStatus::Taken && c.taker == Some(guard) && is_corruption(world, c))
        .flat_map(|c| [c.buyer, c.agent].into_iter().flatten())
}

/// C31: the city guards on the take now (the daily row).
pub fn guards_on_take(world: &World) -> usize {
    world.on_take.keys().filter(|&&g| take_buyers(world, g).next().is_some()).count()
}

/// C31: `World::on_take`, the Taken Guard records without a broker by
/// taker (after a load, and whenever a Guard record changes hands). The
/// filters that read the world (a city guard, a corruption record) apply
/// at the read ([`on_take_of`]), so the index is a function of the saved
/// records alone (OPEN-112).
pub fn reindex_take(world: &mut World) {
    let mut take: std::collections::BTreeMap<EntityId, SmallVec<[ContractId; 2]>> = std::collections::BTreeMap::new();
    for c in world.contracts.values() {
        if c.status != ContractStatus::Taken || c.kind != ContractKind::Guard || c.broker.is_some() {
            continue;
        }
        let Some(g) = c.taker else { continue };
        take.entry(g).or_default().push(c.id);
    }
    world.on_take = take;
}

/// C31: a city guard takes a corruption record: no plan (it keeps its
/// shift), no ledger draw; on the take until the record settles. The take
/// is known as `Hired` (actor the buyer, object the guard) to the placing
/// agent, and the guard is in `known_by`; a Fixer buyer's heat falls by
/// `bribe_cool` once.
/// `GuardTaken` (god log).
pub(super) fn take_on(world: &mut World, id: ContractId, guard: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let now = world.tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.render = Render::Ledger;
        x.due = None;
        // The guard stands the post by keeping its shift.
        x.started = x.started.or(Some(now));
    }
    reindex_take(world);
    // The take's knowledge: the placing agent holds it as a rumour (its
    // telling can expose it); the guard joins `known_by` without one (a
    // rumour with itself as the object would leave it a grudge on its payer).
    if let Some(x) = world.contracts.get_mut(&id) {
        if !x.known_by.contains(&guard) {
            x.known_by.push(guard);
        }
    }
    if let (Some(actor), Some(a)) = (c.buyer.or(c.agent), c.agent) {
        hear_named(world, id, a, actor, guard, 0);
    }
    if let Some(a) = c.agent {
        heat_bribed(world, a);
    }
    let text = format!("{} is on {}'s payroll", world.name_of(guard), world.owner_label(c.buyer.or(c.agent)));
    world.push_event(EventKind::GuardTaken, &[guard, c.buyer.or(c.agent).unwrap_or(EntityId::NONE)], text.clone());
    log(world, id, text);
}

/// C31, from `law::jail_suspect`: a guard on the take that arrested its
/// buyer fails the record (refunded).
pub fn on_arrest(world: &mut World, guard: EntityId, suspect: EntityId) {
    if !on_take_of(world, guard, suspect) {
        return;
    }
    let ids: Vec<ContractId> = world
        .contracts
        .values()
        .filter(|c| c.status == ContractStatus::Taken && c.taker == Some(guard) && is_corruption(world, c))
        .filter(|c| c.buyer == Some(suspect) || c.agent == Some(suspect) || buyer_member(world, c, suspect))
        .map(|c| c.id)
        .collect();
    for id in ids {
        settle(world, id, Settle::Failed, "the guard made the arrest");
    }
}

/// C31, at midnight: at most one open corruption record per buyer, posted
/// by a Fixer owner at `corrupt_heat` or more, a gang (its leader placing
/// it) under a Crackdown, or a suspect with an open report who can pay.
/// Plan deviations: a suspect posts only below `corrupt_lawfulness` (the
/// spec's "may post" needs a decision; the guard's own bar is reused); the
/// record's origin is `Origin::Law` (16a has no corruption origin: "a bribe
/// to a guard is a contract on the law").
pub fn corruption_daily(world: &mut World) {
    let corrupt = world.config.law.corrupt_heat;
    let mut posts: Vec<(Option<EntityId>, Option<EntityId>)> = Vec::new();
    for f in world.buildings_of_kind(BuildingKind::Fixer).to_vec() {
        let Some(o) = world.owner_of(f).filter(|&o| world.has::<Identity>(o)) else { continue };
        if world.comp::<Broker>(f).is_some_and(|k| k.heat >= corrupt) {
            posts.push((Some(o), Some(o)));
        }
    }
    for g in world.gangs() {
        if !crate::systems::law::cracking_down_on(world, g) {
            continue;
        }
        let leader = world.comp::<crate::components::Gang>(g).and_then(|x| x.leader);
        if leader.is_some() {
            posts.push((Some(g), leader));
        }
    }
    let lawless = world.config.law.corrupt_lawfulness;
    let suspects: Vec<EntityId> = world.open_suspects().collect();
    for s in suspects {
        if world.comp::<Personality>(s).is_some_and(|p| p.lawfulness < lawless) {
            posts.push((Some(s), Some(s)));
        }
    }
    for (buyer, agent) in posts {
        let Some(a) = agent else { continue };
        if !crate::systems::law::living(world, a) || world.has::<Sentence>(a) {
            continue;
        }
        let held = world.by_party.get(&a).is_some_and(|l| {
            l.iter().any(|id| world.contracts.get(id).is_some_and(|c| c.is_live() && is_corruption(world, c)))
        });
        if held {
            continue;
        }
        let target = Target::Agent(a);
        if world.purse(buyer) < quote(world, ContractKind::Guard, &target, None) {
            continue;
        }
        let days = world.config.contracts.deadline_days.get(ContractKind::Guard);
        let r = post(
            world,
            Posting {
                buyer,
                agent: Some(a),
                kind: ContractKind::Guard,
                target,
                broker: None,
                deadline_days: days,
                origin: Origin::Law,
                price: None,
            },
        );
        note_refused(world, &r);
    }
}

// --- The Security-corp Guard (C32) ---

/// C32: an Open Guard on a building converts into an M11 guard contract
/// with the cheapest Security corp that has room (not the client's own
/// owner) when the record pays at least a day of its price: the escrow back
/// to the buyer less the Fixer's cut (the placement fee, `FixerCut`), the
/// record `Fulfilled` at once; the corp bills the client's owner daily under
/// M11 from then on.
pub fn corp_take(world: &mut World, id: ContractId) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let Target::Building(b) = c.target else { return false };
    if c.kind != ContractKind::Guard || !c.is_open() {
        return false;
    }
    if world.comp::<Building>(b).is_none_or(|bd| bd.secured_by.is_some() || bd.demolished) {
        return false;
    }
    let owner = world.owner_of(b);
    let Some(corp) = crate::systems::corps::cheapest_seller(world, owner) else { return false };
    if c.price < crate::systems::corps::contract_price(world, corp) {
        return false;
    }
    if !crate::systems::corps::buy_contract(world, b, corp) {
        return false;
    }
    let now = world.tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.taker = Some(corp);
        x.taken = Some(now);
    }
    index_parties(world, id);
    if let Some(f) = c.broker {
        let cut = ((c.escrow as f32) * c_cut(world, f)).round() as i64;
        let to = world.owner_of(f);
        let paid = ownership::escrow_out(world, id, to, cut, Flow::FixerCut);
        note_cut(world, f, paid);
    }
    refund(world, id);
    let text = format!("{} took the guard on {} (an M11 contract)", world.owner_label(Some(corp)), world.name_of(b));
    world.push_event(EventKind::ContractTaken, &[corp, b], text.clone());
    log(world, id, text);
    set_status(world, id, ContractStatus::Fulfilled, "");
    true
}
