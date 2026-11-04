use super::*;

// ---------------------------------------------------------------------------
// Phase 3: bodies (docs/M17_OUTSIDE.md § 2, § 4; "Revised 2026-10-12").
//
// Agents from abroad are spawned through demography's migrant arrival (the
// World's coins and the arrival savings cross in as any migrant's), carry a
// side-table record (`Outside.outsiders`, not a component) and leave by
// demography's emigration path at the end of their tour. A Retake is a
// contract record on the M16 board, struck as one draw over the missions'
// strength estimate (a ledger record, as Tag and Vote). Every coin a parent
// moves crosses through `ownership::{cross_in, cross_out}`; a cost paid
// abroad is booked out of `minted`. No stream draw but `arrive`'s (M11's
// immigrant draws, in order) and the contract's keyed outcome.
// ---------------------------------------------------------------------------

/// `econ::hash_unit` purpose of an Outsider's lawfulness.
const PURPOSE_OUTSIDER: u64 = 0x0E17_0003;
/// `econ::hash_unit` purpose of the brothers' roll on a gutted gang (phase 4b).
pub(super) const PURPOSE_BROTHERS: u64 = 0x0E17_0005;

/// Phase 3: the share of a parent's corp branches a whole week in the red
/// (Σ of the last 7 days' cashflow below 0). Sporadic remittances and a
/// bad day or two read no loss.
pub fn remit_gap(world: &World, id: OutsideId) -> f32 {
    let Some(p) = parent(world, id) else { return 0.0 };
    let corps: Vec<&Corp> = p.branches.iter().filter_map(|&b| world.comp::<Corp>(b)).collect();
    if corps.is_empty() {
        return 0.0;
    }
    let red =
        corps.iter().filter(|c| c.cashflow.len() >= 7 && c.cashflow.iter().rev().take(7).sum::<i64>() < 0).count();
    red as f32 / corps.len() as f32
}

/// A parent's living Outsiders (not its brothers).
pub(super) fn living_outsiders(world: &World, id: OutsideId) -> usize {
    world.outside.outsiders.iter().filter(|(&a, o)| o.from == id && !o.brother && world.is_alive(a)).count()
}

/// A parent's corp branches, ascending.
pub(super) fn corp_branches(world: &World, id: OutsideId) -> Vec<EntityId> {
    parent(world, id).map_or_else(Vec::new, |p| p.branches.iter().copied().filter(|&b| world.has::<Corp>(b)).collect())
}

/// Pay a cost abroad: out of the parent's treasury and out of `minted`
/// (the coins leave existence with the outside economy).
pub(super) fn pay_abroad(world: &mut World, id: OutsideId, cost: i64) -> bool {
    let Some(f) = world.outside.faction_mut(id) else { return false };
    if cost <= 0 {
        return true;
    }
    if f.treasury < cost {
        return false;
    }
    f.treasury -= cost;
    world.outside.minted -= cost;
    // M17 review: every spend is booked (attitude and Abandon read it).
    if let Some(p) = parent_mut(world, id) {
        p.ledger.spent_today += cost;
    }
    true
}

/// Coins from a parent into a branch: `Flow::ParentFund`, capital (a corp's
/// day of trading does not count it). Returns the coins moved.
pub(super) fn fund(world: &mut World, id: OutsideId, branch: EntityId, amount: i64) -> i64 {
    let moved = ownership::cross_in(world, id, Some(branch), amount, Flow::ParentFund);
    if moved > 0 {
        if let Some(f) = world.outside.faction_mut(id) {
            f.income_today += moved;
        }
        if let Some(c) = world.comp_mut::<Corp>(branch) {
            c.cashflow_today -= moved;
        }
        // M17 review: every spend is booked (attitude and Abandon read it).
        if let Some(p) = parent_mut(world, id) {
            p.ledger.spent_today += moved;
        }
    }
    moved
}

/// M17 review: coins `fund` sent for a post that failed go back abroad (as
/// much as the branch still holds), the booking undone.
pub(super) fn unfund(world: &mut World, id: OutsideId, branch: EntityId, amount: i64) {
    let back = amount.min(world.purse(Some(branch)).max(0));
    if back <= 0 {
        return;
    }
    let moved = ownership::cross_out(world, Some(branch), id, back, Flow::ParentFund, true);
    if moved > 0 {
        if let Some(f) = world.outside.faction_mut(id) {
            f.income_today -= moved;
        }
        if let Some(c) = world.comp_mut::<Corp>(branch) {
            c.cashflow_today += moved;
        }
        if let Some(p) = parent_mut(world, id) {
            p.ledger.spent_today -= moved;
        }
    }
}

/// One agent from abroad at `door` (the edge road nearest it, a bed in the
/// Block with room nearest it), fighting at `outsider_skill`, its
/// lawfulness the parent's ± 0.2 (a keyed hash).
pub(super) fn spawn_from_abroad(world: &mut World, id: OutsideId, door: crate::components::TilePos) -> EntityId {
    let home = crate::systems::demography::emptiest_home_near(world, Some(door), 1);
    let tile = crate::systems::demography::nearest_edge_road(world, door);
    let a = crate::systems::demography::spawn_outsider(world, home, tile);
    let cfg = world.config.outside.parents.clone();
    let law = parent(world, id).map_or(0.5, |p| p.persona.lawfulness);
    let u = crate::systems::econ::hash_unit(world.seed(), PURPOSE_OUTSIDER, u64::from(a.index), u64::from(id));
    if let Some(p) = world.comp_mut::<Personality>(a) {
        p.lawfulness = (law + (u - 0.5) * 0.4).clamp(0.0, 1.0);
    }
    if let Some(s) = world.comp_mut::<Skills>(a) {
        s.melee = s.melee.max(cfg.outsider_skill);
    }
    a
}

/// § 2 Reinforce: an episode of `bodies_per_call` bodies starts (a cohort,
/// the cooldown's clock). False inside the cooldown or with no corp branch.
pub fn start_reinforce(world: &mut World, id: OutsideId) -> bool {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return false };
    let cooled = p
        .last_reinforce
        .is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.reinforce_cooldown_days) * TICKS_PER_DAY);
    if !cooled || corp_branches(world, id).is_empty() {
        return false;
    }
    let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    if let Some(p) = parent_mut(world, id) {
        p.reinforce_left = cfg.bodies_per_call;
        p.last_reinforce = Some(now);
        p.cohort_next += 1;
    }
    world.outside.today.reinforcements += 1;
    world.push_event(EventKind::Reinforced, &[], format!("{name} sends its people to the city"));
    true
}

/// The branch building a body goes to: the corp branches' staffed kinds,
/// fewest staff for its places first (ties the lower id).
fn post_for(world: &World, id: OutsideId) -> Option<(EntityId, crate::components::Role)> {
    let mut best: Option<(u64, EntityId, crate::components::Role)> = None;
    for c in corp_branches(world, id) {
        let Some(corp) = world.comp::<Corp>(c) else { continue };
        let buildings = corp.buildings.clone();
        let staff = ownership::staff_by_building(world, &buildings);
        for b in buildings {
            let Some(kind) = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| bd.kind)
            else {
                continue;
            };
            let Some(role) = ownership::role_for(kind) else { continue };
            let places = crate::systems::jobs::places_of(world, b, role).max(1) as u64;
            let have = staff.get(&b).map_or(0, |v| v.len()) as u64;
            let ratio = have * 1000 / places;
            if best.is_none_or(|(r, x, _)| ratio < r || (ratio == r && b < x)) {
                best = Some((ratio, b, role));
            }
        }
    }
    best.map(|(_, b, r)| (b, r))
}

/// Send up to `n` bodies now (`outsiders_cap` holds): each a hire at a
/// branch post with the cohort's Acquaintance edges; `body_cost` abroad and
/// `body_fund` into the branch each. Returns the bodies sent.
fn send_bodies(world: &mut World, id: OutsideId, n: u32) -> u32 {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let cohort = parent(world, id).map_or(0, |p| p.cohort_next);
    let mut sent: Vec<EntityId> = Vec::new();
    for _ in 0..n {
        if living_outsiders(world, id) >= cfg.outsiders_cap as usize {
            break;
        }
        let Some((b, role)) = post_for(world, id) else { break };
        let Some(branch) = world.owner_of(b) else { break };
        let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { break };
        if world.outside.faction(id).is_none_or(|f| f.treasury < cfg.body_cost + cfg.body_fund) {
            break;
        }
        pay_abroad(world, id, cfg.body_cost);
        let a = spawn_from_abroad(world, id, door);
        crate::systems::demography::floor_role_skill(world, a, role);
        crate::systems::demography::hire(world, a, b, role);
        if let Some(v) = world.vacancies.get_mut(&b) {
            if let Some(i) = v.iter().position(|&r| r == role) {
                v.remove(i);
            }
            if v.is_empty() {
                world.vacancies.remove(&b);
            }
        }
        fund(world, id, branch, cfg.body_fund);
        let tour_until = now + u64::from(cfg.tour_days) * TICKS_PER_DAY;
        world.outside.outsiders.insert(a, Outsider { from: id, arrived: now, tour_until, cohort, brother: false });
        for &o in &sent {
            for (x, y) in [(a, o), (o, a)] {
                let e = world.edge_entry(x, y);
                e.affinity = e.affinity.max(0.4);
                e.last_interaction = now;
            }
        }
        sent.push(a);
    }
    let k = sent.len() as u32;
    if k > 0 {
        world.outside.today.arrived += k;
        let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
        world.push_event(EventKind::OutsidersArrived, &sent, format!("{k} of {name}'s people arrived in the city"));
    }
    k
}

/// A Reinforce episode's day: up to `bodies_per_day` of what is left.
pub fn reinforce_step(world: &mut World, id: OutsideId) -> u32 {
    let cfg = world.config.outside.parents.clone();
    let left = parent(world, id).map_or(0, |p| p.reinforce_left);
    let n = send_bodies(world, id, left.min(cfg.bodies_per_day));
    if let Some(p) = parent_mut(world, id) {
        // A body that could not come (the cap, no post, no coins) ends the episode.
        p.reinforce_left = if n < left.min(cfg.bodies_per_day) { 0 } else { p.reinforce_left - n };
    }
    n
}

/// God `ForceReinforce`: `bodies` now, the cooldown and the daily caps
/// ignored, `outsiders_cap` not.
pub fn force_reinforce(world: &mut World, id: OutsideId, bodies: u32) -> Result<String, String> {
    let now = world.tick;
    let name = world.outside.faction(id).map(|f| f.name.clone()).ok_or(format!("ForceReinforce: no parent {id}"))?;
    let Some(p) = parent_mut(world, id) else { return Err(format!("ForceReinforce: {name} is no parent")) };
    p.cohort_next += 1;
    p.last_reinforce = Some(now);
    world.outside.today.reinforcements += 1;
    let n = send_bodies(world, id, bodies);
    Ok(format!("{name} sent {n} of {bodies} people at once"))
}

/// God `SetOutsideOrder`: the order pinned for `days` (a Reinforce armed).
pub fn set_order(world: &mut World, id: OutsideId, order: OutsideOrder, days: u32) -> Result<String, String> {
    let now = world.tick;
    let name = world.outside.faction(id).map(|f| f.name.clone()).ok_or(format!("SetOutsideOrder: no parent {id}"))?;
    let Some(p) = parent_mut(world, id) else { return Err(format!("SetOutsideOrder: {name} is no parent")) };
    p.order = order;
    p.order_since = now;
    p.pinned_until = Some(now + u64::from(days) * TICKS_PER_DAY);
    if order == OutsideOrder::Reinforce {
        p.last_reinforce = None;
        start_reinforce(world, id);
    }
    Ok(format!("{name}'s order pinned to {} for {days} days", order.label()))
}

/// The daily tour pass: the dead and the gone drop off the table; an
/// Outsider past `tour_until` with a Spouse or a child settles (stays,
/// loses the record), else emigrates.
pub fn tours(world: &mut World) {
    let now = world.tick;
    let all: Vec<(EntityId, Outsider)> = world.outside.outsiders.iter().map(|(&a, o)| (a, o.clone())).collect();
    for (a, o) in all {
        if !world.is_alive(a) || !world.has::<crate::components::Identity>(a) {
            world.outside.outsiders.remove(&a);
            continue;
        }
        if o.brother || now < o.tour_until {
            continue;
        }
        world.outside.outsiders.remove(&a);
        let kin = world.spouses.contains_key(&a) || !crate::systems::demography::children_of_agent(world, a).is_empty();
        let name = world.name_of(a);
        if kin {
            world.outside.today.settled += 1;
            world.push_event(EventKind::Settled, &[a], format!("{name}'s tour ended; they stay for their family"));
        } else {
            crate::systems::demography::start_emigrating(world, a, "tour over");
            world.outside.today.left += 1;
            world.push_event(EventKind::OutsiderLeft, &[a], format!("{name}'s tour ended; they leave the city"));
        }
    }
}

/// The holder a lost building's buy-back or Retake deals with: its owner
/// when not a branch of `id` (`Some(None)`: the city, a squat or a
/// foreclosure), `None` when the building is back, gone or derelict.
pub(super) fn retake_holder(world: &World, id: OutsideId, b: EntityId) -> Option<Option<EntityId>> {
    let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished)?;
    let owner = bd.owner;
    if owner.is_some_and(|o| world.comp::<Corp>(o).is_some_and(|c| c.parent == Some(id))) {
        return None;
    }
    Some(owner)
}

/// § 2: the branches' buildings against the last midnight's: a building
/// gone from them (and standing) is lost; a lost one back with a branch, or
/// past `retake_window_days`, drops off the list.
pub fn note_losses(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let mut current: Vec<EntityId> = corp_branches(world, id)
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|x| x.buildings.clone()))
        .flatten()
        .collect();
    current.sort_unstable();
    let Some(p) = parent(world, id) else { return };
    let mut lost = p.lost.clone();
    for &b in &p.held {
        if current.binary_search(&b).is_err()
            && !lost.iter().any(|l| l.building == b)
            && world.comp::<Building>(b).is_some_and(|bd| !bd.demolished)
        {
            lost.push(LostBuilding { building: b, at: now, refusals: 0, contract: None, hit_posted: false });
        }
    }
    let window = u64::from(cfg.retake_window_days) * TICKS_PER_DAY;
    lost.retain(|l| now.saturating_sub(l.at) < window && retake_holder(world, id, l.building).is_some());
    if let Some(p) = parent_mut(world, id) {
        p.held = current;
        p.lost = lost;
    }
}

/// The corp seller's rule (M11, a simple form): a corp sells a building
/// back when it is short of cash or greedy; an agent when greedy or poorer
/// than the building; a gang never; the city always.
fn accepts_offer(world: &World, holder: Option<EntityId>, value: i64) -> bool {
    match ownership::owner_kind(world, holder) {
        ownership::OwnerKind::City => true,
        ownership::OwnerKind::Gang(_) => false,
        ownership::OwnerKind::Corp(c) => world.comp::<Corp>(c).is_some_and(|x| {
            x.closing < x.treasury_ref / 2
                || x.exec.and_then(|e| world.comp::<Personality>(e)).is_some_and(|p| p.greed >= 0.5)
        }),
        ownership::OwnerKind::Agent(a) => {
            world.comp::<Personality>(a).is_some_and(|p| p.greed >= 0.5) || world.purse(Some(a)) < value
        }
    }
}

/// § 2 Retake's daily action on the newest lost building: a buy-back at
/// `retake_premium × value` (funded by the parent) while its holder has
/// refused fewer than `refusals_to_force` offers, then a `Retake` record
/// (first to the branch's own Outsiders), a Hit on a raider gang's leader
/// when the parent's lawfulness is low, and a Guard on each remaining
/// branch building. Nothing under the Embargo.
pub fn retake_step(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let Some(p) = parent(world, id) else { return };
    if p.embargoed {
        return;
    }
    let lawfulness = p.persona.lawfulness;
    let Some(i) = (0..p.lost.len()).rev().find(|&i| retake_holder(world, id, p.lost[i].building).is_some()) else {
        return;
    };
    let entry = p.lost[i].clone();
    let b = entry.building;
    let Some(holder) = retake_holder(world, id, b) else { return };
    let Some(branch) = corp_branches(world, id).into_iter().next() else { return };
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let value = ownership::value(world, kind).max(1);
    if let Some(c) = entry.contract {
        // A record stands (or ran): nothing new until it closes.
        if world.contracts.get(&c).is_some_and(|x| {
            matches!(x.status, crate::contract::ContractStatus::Open | crate::contract::ContractStatus::Taken)
        }) {
            return;
        }
        if let Some(p) = parent_mut(world, id) {
            p.lost[i].contract = None;
        }
        return;
    }
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    let (bname, hname) = (world.name_of(b), world.owner_label(holder));
    if entry.refusals < cfg.refusals_to_force {
        let price = (f64::from(cfg.retake_premium) * value as f64).round() as i64;
        world.outside.today.buybacks += 1;
        if accepts_offer(world, holder, value) {
            let got = fund(world, id, branch, price);
            if got >= price && crate::systems::corps::acquire(world, branch, b, price, "bought back") {
                world.push_event(
                    EventKind::BuyBackOffered,
                    &[branch, b],
                    format!("{pname} bought {bname} back from {hname} for {price}"),
                );
                return;
            }
            // M17 review: the sale fell through: the coins go back abroad.
            unfund(world, id, branch, got);
        }
        world.push_event(
            EventKind::BuyBackOffered,
            &[branch, b],
            format!("{hname} refused {pname}'s offer of {price} for {bname}"),
        );
        if let Some(p) = parent_mut(world, id) {
            p.lost[i].refusals += 1;
        }
        return;
    }
    // Force: the Retake on the board, the parent's coins behind it.
    let target = crate::contract::Target::Building(b);
    let quote = crate::systems::contracts::quote(world, crate::contract::ContractKind::Retake, &target, None);
    let sent = fund(world, id, branch, quote);
    let door = world.comp::<Building>(b).map(|bd| bd.door).unwrap_or_default();
    let broker = crate::systems::contracts::nearest_fixer_to(world, door);
    let exec = world.comp::<Corp>(branch).and_then(|c| c.exec);
    let r = crate::systems::contracts::brain_post_with(
        world,
        Some(branch),
        exec,
        crate::contract::ContractKind::Retake,
        target,
        broker,
        crate::contract::Origin::Parent,
        Some(quote),
        crate::contract::PostExtra::default(),
    );
    let Ok(cid) = r else {
        unfund(world, id, branch, sent);
        return;
    };
    if let Some(p) = parent_mut(world, id) {
        p.lost[i].contract = Some(cid);
    }
    // First to the branch's own: its strongest free Outsider, the others as crew.
    let mut own: Vec<EntityId> = world
        .outside
        .outsiders
        .iter()
        .filter(|(&a, o)| o.from == id && !o.brother && crate::systems::contracts::free_adult(world, a))
        .map(|(&a, _)| a)
        .collect();
    crate::systems::raid::by_strength(world, &mut own);
    if let Some((&lead, rest)) = own.split_first() {
        let max = usize::from(world.config.fixers.crew_max.saturating_sub(1));
        let crew: Vec<EntityId> = rest.iter().copied().take(max).collect();
        crate::systems::contracts::accept(world, cid, lead, &crew);
    }
    // A raider gang's leader, when the parent is lawless enough.
    if let ownership::OwnerKind::Gang(g) = ownership::owner_kind(world, holder) {
        if lawfulness < cfg.hit_lawfulness && !entry.hit_posted {
            if let Some(l) = world.comp::<Gang>(g).and_then(|x| x.leader) {
                let t = crate::contract::Target::Agent(l);
                let q = crate::systems::contracts::quote(world, crate::contract::ContractKind::Hit, &t, broker);
                let sent = fund(world, id, branch, q);
                let posted = crate::systems::contracts::brain_post_with(
                    world,
                    Some(branch),
                    exec,
                    crate::contract::ContractKind::Hit,
                    t,
                    broker,
                    crate::contract::Origin::Parent,
                    Some(q),
                    crate::contract::PostExtra::default(),
                );
                if posted.is_err() {
                    unfund(world, id, branch, sent);
                }
                if let Some(p) = parent_mut(world, id) {
                    p.lost[i].hit_posted = true;
                }
            }
        }
    }
    // A Guard on each remaining branch building.
    let remaining: Vec<EntityId> = world.comp::<Corp>(branch).map(|c| c.buildings.clone()).unwrap_or_default();
    for rb in remaining {
        let t = crate::contract::Target::Building(rb);
        let q = crate::systems::contracts::quote(world, crate::contract::ContractKind::Guard, &t, None);
        fund(world, id, branch, q);
        crate::systems::contracts::post_secure(world, branch, rb);
    }
}

/// A won Retake (or a god's): the branch buys the building from its holder
/// at its value (funded by the parent: a forced sale), the record settles.
pub fn retake_win(world: &mut World, cid: crate::contract::ContractId) -> bool {
    let Some(c) = world.contracts.get(&cid).cloned() else { return false };
    let crate::contract::Target::Building(b) = c.target else { return false };
    let Some(branch) = c.buyer.filter(|&x| world.has::<Corp>(x)) else { return false };
    let Some(id) = world.comp::<Corp>(branch).and_then(|x| x.parent) else { return false };
    let Some(kind) = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.kind) else { return false };
    let value = ownership::value(world, kind).max(1);
    let holder = world.owner_of(b);
    let short = (value - world.purse(Some(branch))).max(0);
    if short > 0 {
        fund(world, id, branch, short);
    }
    if !crate::systems::corps::acquire(world, branch, b, value, "retaken") {
        return false;
    }
    let text = format!(
        "{} took {} back from {}",
        world.owner_label(Some(branch)),
        world.name_of(b),
        world.owner_label(holder)
    );
    if c.status == crate::contract::ContractStatus::Taken {
        crate::systems::contracts::settle(world, cid, crate::systems::contracts::Settle::Fulfilled, &text);
    } else if c.status == crate::contract::ContractStatus::Open {
        crate::systems::contracts::settle(world, cid, crate::systems::contracts::Settle::Cancelled, &text);
    }
    world.push_event(EventKind::BranchRetook, &[branch, b], text);
    true
}

/// The Retake's strike (`contracts::resolve_ledger`): the crew's summed
/// strength against the building's posted guards and, when a gang holds
/// it, its strongest members (up to `crew_max`); one keyed draw at the
/// missions' `p_win`. A win forces the sale; a loss is a failed attempt.
pub fn resolve_retake(world: &mut World, id: crate::contract::ContractId, lead: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let crate::contract::Target::Building(b) = c.target else { return };
    let Some(pid) = c.buyer.and_then(|x| world.comp::<Corp>(x)).and_then(|x| x.parent) else {
        crate::systems::contracts::fail_attempt(world, id, "the branch is gone");
        return;
    };
    if retake_holder(world, pid, b).is_none() {
        crate::systems::contracts::settle(world, id, crate::systems::contracts::Settle::Cancelled, "already back");
        return;
    }
    let mut party: Vec<EntityId> = vec![lead];
    party.extend(c.crew.iter().copied().filter(|&m| m != lead && crate::systems::contracts::free_adult(world, m)));
    let s_crew: f32 = party.iter().map(|&a| crate::systems::raid::strength(world, a)).sum();
    let mut defenders = crate::systems::raid::posted_guards(world, b);
    if let ownership::OwnerKind::Gang(g) = ownership::owner_kind(world, world.owner_of(b)) {
        let mut m: Vec<EntityId> = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
        crate::systems::raid::by_strength(world, &mut m);
        defenders.extend(m.into_iter().take(usize::from(world.config.fixers.crew_max)));
    }
    let s_def: f32 = defenders.iter().map(|&a| crate::systems::raid::strength(world, a)).sum::<f32>()
        + crate::systems::missions::MEDIAN_STRENGTH;
    let p = crate::systems::missions::p_win(world, s_crew, s_def);
    use rand::Rng;
    let win = world.rng.contract(crate::contract::ContractNs::Outcome, id, u64::from(c.attempts)).random::<f32>() < p;
    if win {
        if !retake_win(world, id) {
            crate::systems::contracts::fail_attempt(world, id, "the sale did not go through");
        }
    } else {
        crate::systems::contracts::fail_attempt(world, id, "the strike on the building was lost");
    }
}
