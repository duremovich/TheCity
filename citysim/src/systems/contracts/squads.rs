use super::*;

/// The record a body works (for the Inspector and tests).
pub fn run_of(world: &World, id: EntityId) -> Option<&ContractRun> {
    world.contract_runs.get(&id)
}

// ---------------------------------------------------------------------------
// Missions, squads, gang takers and sell-outs (phase 2: C22-C26)
// ---------------------------------------------------------------------------

/// Is `id` a gang (a faction taker)?
pub(super) fn is_gang(world: &World, id: EntityId) -> bool {
    world.has::<crate::components::Gang>(id)
}

/// C22, C25, C26: a record worked as a mission: a gang's job, or a squad
/// (an agent taker with a crew).
pub(super) fn is_mission(world: &World, c: &Contract) -> bool {
    // M16b phase 2: a Rescue always marches on its hold.
    c.taker.is_some_and(|t| is_gang(world, t)) || !c.crew.is_empty() || c.kind == ContractKind::Rescue
}

/// A Taken live record already being worked (a run or a mission).
pub(super) fn running(world: &World, c: &Contract) -> bool {
    world.missions.contains_key(&c.id)
        || c.taker.is_some_and(|t| world.contract_runs.get(&t).is_some_and(|r| r.contract == c.id))
}

/// C42 (phase 4): the Board's render tag: `-` for an open record, QUEUED
/// for a live record waiting for room (C35), else LEDGER, LIVE or PLAYED.
pub fn render_tag(world: &World, c: &Contract) -> &'static str {
    if c.is_open() {
        return "-";
    }
    if c.status == ContractStatus::Taken && c.render == Render::Live && !running(world, c) {
        return "QUEUED";
    }
    c.render.label()
}

/// The people on a record: an agent taker and the crew (`hits_squad`
/// counts fulfilled Hits with two or more, C25).
pub(super) fn squad_size(world: &World, c: &Contract) -> usize {
    usize::from(c.taker.is_some_and(|t| !is_gang(world, t))) + c.crew.len()
}

/// C26: the record a gang holds as its job (`Taken`, the gang the taker).
pub fn job_of(world: &World, gang: EntityId) -> Option<ContractId> {
    world
        .by_party
        .get(&gang)?
        .iter()
        .copied()
        .find(|id| world.contracts.get(id).is_some_and(|c| c.status == ContractStatus::Taken && c.taker == Some(gang)))
}

/// C26 (review fix): the gang's job is marching (its mission open): the
/// window `faction::rescore` holds `Order::Job` in (a job queued past
/// `max_missions` does not freeze the gang's order).
pub fn job_live(world: &World, gang: EntityId) -> bool {
    !world.missions.is_empty() && job_of(world, gang).is_some_and(|id| world.missions.contains_key(&id))
}

/// May `m` march on record `c` as crew: eligible as a taker would be (C12),
/// not in a live riot, not on another mission, and not of a gang mustering
/// its own raid.
pub(super) fn crew_ok(world: &World, c: &Contract, m: EntityId) -> bool {
    eligible(world, c, m)
        && !world.rioter_of.contains_key(&m)
        && !world.mission_of.contains_key(&m)
        && world.gang_of(m).and_then(|g| world.comp::<crate::components::Gang>(g)).is_none_or(|g| g.raid_at.is_none())
}

/// C25: a weak gun's crew. When the solo estimate is below `squad_below`:
/// up to `crew_max − 1` of the taker's gang members who may march (else, a
/// gangless taker or none fit, its Friends who are regulars of the
/// record's Fixer), strongest first (ties the lower id), stopping once the
/// estimate clears `squad_below`. Empty for a strong gun, a Guard or a
/// Locate.
pub fn squad_crew(world: &World, c: &Contract, taker: EntityId) -> SmallVec<[EntityId; 4]> {
    let mut crew: SmallVec<[EntityId; 4]> = SmallVec::new();
    let below = world.config.missions.squad_below;
    let rescue = c.kind == ContractKind::Rescue;
    if (!c.kind.violent() && !rescue) || (!rescue && missions::estimate(world, &[taker], &c.target) >= below) {
        return crew;
    }
    let ok = |m: EntityId| m != taker && crew_ok(world, c, m);
    let mut pool: Vec<EntityId> = world
        .gang_of(taker)
        .and_then(|g| world.comp::<crate::components::Gang>(g))
        .map(|g| g.members.iter().copied().filter(|&m| ok(m)).collect())
        .unwrap_or_default();
    if pool.is_empty() {
        let regulars = c.broker.and_then(|b| world.comp::<Broker>(b));
        pool = world
            .neighbours(taker)
            .filter(|&o| world.edge(taker, o).is_some_and(|e| e.kind == RelKind::Friend))
            .filter(|o| regulars.is_some_and(|k| k.regulars.contains_key(o)))
            .filter(|&o| ok(o))
            .collect();
    }
    crate::systems::raid::by_strength(world, &mut pool);
    let max = usize::from(world.config.fixers.crew_max.saturating_sub(1));
    let mut team: Vec<EntityId> = vec![taker];
    for m in pool {
        if crew.len() >= max {
            break;
        }
        crew.push(m);
        team.push(m);
        if missions::estimate(world, &team, &c.target) >= below {
            break;
        }
    }
    crew
}

/// C26: a gang's crew for a job: its fittest members who may march (C12's
/// eligibility), strongest first, up to `crew_max`, the leader only if
/// fewer than two others.
pub fn job_crew(world: &World, c: &Contract, gang: EntityId) -> SmallVec<[EntityId; 4]> {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return SmallVec::new() };
    let leader = g.leader;
    let mut pool: Vec<EntityId> =
        g.members.iter().copied().filter(|&m| Some(m) != leader && crew_ok(world, c, m)).collect();
    crate::systems::raid::by_strength(world, &mut pool);
    let max = usize::from(world.config.fixers.crew_max.max(1));
    let mut crew: SmallVec<[EntityId; 4]> = pool.into_iter().take(max).collect();
    if crew.len() < 2 && crew.len() < max {
        if let Some(l) = leader.filter(|&l| crew_ok(world, c, l)) {
            crew.push(l);
        }
    }
    crew
}

/// C26: may `gang` take record `c` as a job: a Hit or a Beat, not the
/// buyer, not the target's own gang, a free leader, no job, no raid
/// mustering, no god strike pinned, and a crew to send.
pub fn gang_eligible(world: &World, c: &Contract, gang: EntityId) -> bool {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return false };
    let now = world.tick;
    let t = c.target.id();
    // Review fix: the leader who accepts is held to `eligible`'s
    // relationship exclusions (no job on its Spouse, Family, Parent or
    // Friend), as the crew are.
    let kin = |l: EntityId| {
        l == t
            || Some(l) == c.buyer
            || Some(l) == c.agent
            || world.edge(l, t).is_some_and(|e| {
                matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend)
            })
    };
    // M16b phase 2: a Rescue frees its own: the buyer's gang, a member, kin.
    let rescue = c.kind == ContractKind::Rescue;
    (c.kind.violent() || rescue)
        && (c.buyer != Some(gang) || rescue)
        && (rescue || !crate::systems::grudges::member_of(world, t, gang))
        && g.leader.is_some_and(|l| !world.has::<Sentence>(l) && (rescue || !kin(l)))
        && job_of(world, gang).is_none()
        && g.raid_at.is_none()
        && g.strike.is_none_or(|(_, until)| until <= now)
        && !job_crew(world, c, gang).is_empty()
}

/// C26: a gang's leader takes a job: `greed` → Linear{0.6, 0.4} × `1 −
/// lawfulness` → Linear{0.5, 0.5} × `Regard(gang, buyer's faction).fear` →
/// Linear{0.5, 0.5} ≥ `accept_min` (the spec's sell-out rule, also the
/// matched gang's: one rule for a gang's acceptance).
pub fn leader_accepts(world: &World, gang: EntityId, buyer_faction: EntityId) -> bool {
    let Some(p) = world
        .comp::<crate::components::Gang>(gang)
        .and_then(|g| g.leader)
        .filter(|&l| !world.has::<Sentence>(l))
        .and_then(|l| world.comp::<Personality>(l))
    else {
        return false;
    };
    let fear = crate::systems::reputation::regard(world, gang, buyer_faction).fear;
    let score = Consideration::new("greed", p.greed, Curve::Linear { m: 0.6, b: 0.4 }).output
        * Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }).output
        * Consideration::new("fear of the buyer", fear, Curve::Linear { m: 0.5, b: 0.5 }).output;
    score >= world.config.contracts.accept_min
}

/// C26: `gang` takes Open record `id` as a job: its crew (`job_crew`), the
/// record Taken and live, the mission opened (or queued); the gang's brain
/// pins `Order::Job` once the mission opens (`open_mission`).
pub fn accept_gang(world: &mut World, id: ContractId, gang: EntityId) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let crew = job_crew(world, &c, gang);
    !crew.is_empty() && accept(world, id, gang, &crew)
}

/// C22: the door a mission marches to at `at`: the freshest intel (the
/// taker's or its first crew member's own and gang's sightings, then the
/// buyer's database), else the target's habit at `at`; a building's door
/// is the street tile outside it.
pub fn mission_door(world: &World, id: ContractId, at: Tick) -> (TilePos, Option<EntityId>) {
    let Some(c) = world.contracts.get(&id) else { return (TilePos::default(), None) };
    let Some(t) = c.target_agent() else { return (TilePos::default(), None) };
    // M16b phase 2: a Rescue marches on the captive's hold.
    if c.kind == ContractKind::Rescue {
        if let Some(b) = crate::systems::captives::hold_of(world, t) {
            if let Some(d) = missions::door_of(world, b) {
                return (d, Some(b));
            }
        }
    }
    let lead = c.taker.filter(|&x| !is_gang(world, x)).or_else(|| c.crew.first().copied());
    let intel = lead
        .and_then(|l| crate::systems::hunt::fresh_intel(world, l, t))
        .or_else(|| buyer_intel(world, c, t))
        .unwrap_or_else(|| crate::systems::hunt::habit(world, t, at));
    match intel.building.and_then(|b| missions::door_of(world, b).map(|d| (d, b))) {
        Some((d, b)) => (d, Some(b)),
        None => (intel.tile, None),
    }
}

/// C22: where a mission gathers: outside the taker's gang's held Home
/// nearest the door within `muster_near_tiles`, else outside the record's
/// Fixer, else outside the taker's Home (a gang's Hideout), else where the
/// lead stands.
fn mission_muster(world: &World, c: &Contract, door: TilePos) -> TilePos {
    let near = world.config.gangs.muster_near_tiles;
    // Round 7 W2 (`OPEN-241`): each place in the order, the first within
    // `muster_near_tiles` of the door; none: the one nearest the door (V15
    // `gang_member_11411 d25`: a muster at the Fixer's in D3, then 343 tiles
    // toward a door in the Sump, six hours, "the crew never reached the door").
    let mut places: Vec<TilePos> = Vec::new();
    if let Some(g) = missions::taker_gang(world, c).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        let best = g
            .territory
            .iter()
            .filter_map(|&h| {
                world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(door), h, b))
            })
            .min_by_key(|&(d, h, _)| (d, h));
        if let Some((_, _, b)) = best {
            places.push(world.outside_door(b));
        }
    }
    places.extend(c.broker.and_then(|b| missions::door_of(world, b)));
    let taker = c.taker.unwrap_or(EntityId::NONE);
    let home = if is_gang(world, taker) { world.hideout_of(taker) } else { missions::home_of(world, taker) };
    places.extend(home.and_then(|h| missions::door_of(world, h)));
    let lead = c.crew.first().copied().unwrap_or(taker);
    places.extend(world.comp::<Position>(lead).map(|p| p.tile));
    if let Some(&t) = places.iter().find(|t| t.manhattan(door) <= near) {
        return t;
    }
    places.iter().copied().min_by_key(|t| t.manhattan(door)).unwrap_or(door)
}

/// C22: open record `id`'s mission (a squad's or a gang's): the crew (an
/// agent taker first), the muster, the door, `raid_at` at the next
/// `raid_muster_hour`, `mission_of`, the record live with a fresh strike;
/// the crew promoted to bodies. `false` when `max_missions` live records
/// are going (it stays queued, C17).
pub fn open_mission(world: &mut World, id: ContractId) -> bool {
    if world.missions.contains_key(&id) {
        return true;
    }
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    if c.status != ContractStatus::Taken || c.target_agent().is_none() {
        return false;
    }
    // C35 (phase 4): the run count and the bodies' quota.
    if !has_room(world, &c) {
        return false;
    }
    let now = world.tick;
    let mut crew: SmallVec<[EntityId; 6]> = SmallVec::new();
    crew.extend(c.taker.filter(|&t| !is_gang(world, t)));
    crew.extend(c.crew.iter().copied());
    crew.retain(|a| crate::systems::law::living(world, *a));
    if crew.is_empty() {
        return false;
    }
    let raid_at = crate::systems::faction::next_muster(now, world.config.gangs.raid_muster_hour);
    let (door, door_building) = mission_door(world, id, raid_at);
    let muster = mission_muster(world, &c, door);
    let hint = missions::expected_defenders(world, &c.target).len().saturating_sub(1).min(255) as u8;
    for &a in &crew {
        world.mission_of.insert(a, id);
    }
    world.missions.insert(
        id,
        crate::contract::Mission {
            contract: id,
            crew: crew.clone(),
            muster,
            door,
            door_building,
            raid_at,
            stream_by: None,
            defenders_hint: hint,
        },
    );
    if let Some(x) = world.contracts.get_mut(&id) {
        x.render = Render::Live;
        x.due = None;
        x.strike = None;
        x.holds = 0;
        x.started = x.started.or(Some(now));
    }
    if let Some(d) = c.due {
        world.ledger_due.remove(&(d, id));
    }
    for &a in &crew {
        if world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical) {
            crate::systems::lod::set_lod(world, a, Lod::Coarse);
        }
    }
    // C26: a gang's job marches: its brain pins `Order::Job` at once (at
    // acceptance, a sell-out, or when a queued job finds room).
    if let Some(g) = c.taker.filter(|&t| is_gang(world, t)) {
        crate::systems::faction::rethink(world, g);
    }
    true
}

/// C22: a mission ends (its record left Taken, failed an attempt or was
/// sold out): the crew released, `mission_of` cleaned, their Contract plans
/// dropped.
pub fn end_mission(world: &mut World, id: ContractId) {
    let Some(m) = world.missions.remove(&id) else { return };
    for &a in &m.crew {
        if world.mission_of.get(&a) == Some(&id) {
            world.mission_of.remove(&a);
        }
        if world.comp::<Brain>(a).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
            world.abort_plan(a);
        }
    }
}

/// C22: `who` leaves record `id`'s mission (dead, jailed, cuffed, gone):
/// out of the crew and the record (an agent taker's place goes to the
/// first of the crew), no longer a party; the last one out fails the
/// attempt.
pub fn drop_crew(world: &mut World, id: ContractId, who: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    if let Some(m) = world.missions.get_mut(&id) {
        m.crew.retain(|a| *a != who);
    }
    if world.mission_of.get(&who) == Some(&id) {
        world.mission_of.remove(&who);
    }
    if world.comp::<Brain>(who).and_then(|b| b.plan_goal()) == Some(GoalKind::Contract) {
        world.abort_plan(who);
    }
    let empty = world.missions.get(&id).is_none_or(|m| m.crew.is_empty());
    if empty {
        fail_attempt(world, id, "the crew is gone");
        return;
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.crew.retain(|a| *a != who);
        if x.taker == Some(who) {
            x.taker = if x.crew.is_empty() { None } else { Some(x.crew.remove(0)) };
        }
    }
    unparty(world, &c, id, who);
}

/// `who` (a departing taker or crew member of record `id`) is no longer
/// a party to it, unless it is also the buyer, the placing agent or the
/// target.
pub(super) fn unparty(world: &mut World, c: &Contract, id: ContractId, who: EntityId) {
    if Some(who) == c.buyer || Some(who) == c.agent || who == c.target.id() {
        return;
    }
    if let Some(list) = world.by_party.get_mut(&who) {
        list.retain(|x| *x != id);
        if list.is_empty() {
            world.by_party.remove(&who);
        }
    }
}

/// C26: would `gang` (the door district's controller) take record `id`
/// over: a Taken Hit or Beat whose target is not its member, the gang's
/// `Regard` of the target's faction (its gang, else its corp, else the
/// target) below `sell_regard`, the gang not its taker or buyer and free to
/// take a job, its leader accepting (`leader_accepts`). The buyer's purse
/// is `sell_out`'s check (a short buyer lapses the offer).
pub fn can_sell_out(world: &World, id: ContractId, gang: EntityId) -> bool {
    let Some(c) = world.contracts.get(&id) else { return false };
    let Some(t) = c.target_agent() else { return false };
    if c.status != ContractStatus::Taken || c.taker == Some(gang) || missions::taker_gang(world, c) == Some(gang) {
        return false;
    }
    let tf = world.gang_of(t).or_else(|| world.corp_of_agent(t)).unwrap_or(t);
    gang_eligible(world, c, gang)
        && crate::systems::reputation::regard(world, gang, tf).value < world.config.missions.sell_regard
        && leader_accepts(world, gang, missions::buyer_faction(world, c))
}

/// C26: sell record `id` out to `gang` (the same record, one id): the price
/// raised by `sell_premium` (a brokered buyer tops up the escrow, a direct
/// buyer's purse is checked; short, the offer lapses: `false`), the old
/// taker and crew released, the gang the taker with its crew, `sold_out`,
/// `SoldOut`, the mission opened (or queued) and the gang's brain rethinks.
pub fn sell_out(world: &mut World, id: ContractId, gang: EntityId) -> bool {
    if !can_sell_out(world, id, gang) {
        return false;
    }
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let new_price = ((c.price as f32) * world.config.missions.sell_premium).round().max(c.price as f32) as i64;
    let top = new_price - c.price;
    // The Hit market (P2 fix round): a direct record holding a deposit tops
    // up its escrow as a brokered one does.
    if c.brokered() || c.escrow > 0 {
        // Review fix: a short top-up returns only what it took (the record's
        // own escrow stays for its next taker) and the offer lapses.
        let moved = if top > 0 { ownership::escrow_in(world, c.buyer, id, top) } else { 0 };
        if moved < top {
            if moved > 0 {
                ownership::escrow_out(world, id, refund_to(world, c.buyer), moved, Flow::Escrow);
            }
            return false;
        }
        // A record on credit: the top-up is paid down, the balance unchanged
        // (it was charged a second time at fulfilment).
        if c.deposit > 0 {
            if let Some(x) = world.contracts.get_mut(&id) {
                x.deposit += top;
            }
        }
    } else if world.purse(c.buyer) < new_price {
        return false;
    }
    let crew = job_crew(world, &c, gang);
    let prev = c.taker;
    if let Some(t) = prev {
        end_run(world, t, id);
    }
    for &m in &c.crew {
        end_run(world, m, id);
    }
    end_mission(world, id);
    if let Some(d) = c.due {
        world.ledger_due.remove(&(d, id));
    }
    for p in prev.into_iter().chain(c.crew.iter().copied()) {
        unparty(world, &c, id, p);
    }
    if let Some(x) = world.contracts.get_mut(&id) {
        x.taker = Some(gang);
        x.crew = crew.clone();
        x.price = new_price;
        x.sold_out = true;
        x.holds = 0;
        x.due = None;
        x.render = Render::Live;
    }
    index_parties(world, id);
    for &m in &crew {
        hear_hired(world, id, m);
    }
    world.stats.current.contract.sold_out += 1;
    let t = c.target.id();
    let text = format!(
        "{} took over the job on {} from {}",
        world.owner_label(Some(gang)),
        world.name_of(t),
        world.owner_label(prev)
    );
    world.push_event(EventKind::SoldOut, &[gang, t, prev.unwrap_or(EntityId::NONE)], text.clone());
    log(world, id, text);
    start_live(world, id);
    true
}

/// C24, from `StakeOut`'s contact (exec): a live solo Hit or Beat decides
/// its strike there (the taker against the target's expected defenders,
/// the target's district's controller). `true` strikes (the `Attack`
/// follows; also for anything that is not a contract strike); a Hold sends
/// the run back to asking and cools the goal a day; a SellOut or a Fail
/// ends this attempt. No draw.
pub fn strike_at_contact(world: &mut World, taker: EntityId) -> bool {
    if world.contract_runs.is_empty() {
        return true;
    }
    let Some(r) = world.contract_runs.get(&taker).cloned() else { return true };
    let violent = world
        .contracts
        .get(&r.contract)
        .is_some_and(|c| c.status == ContractStatus::Taken && c.kind.violent() && c.target_agent().is_some());
    if !violent {
        return true;
    }
    let door = world
        .comp::<Position>(r.target)
        .or_else(|| world.comp::<Position>(taker))
        .map_or_else(TilePos::default, |p| p.tile);
    match missions::decide(world, r.contract, &[taker], door) {
        crate::contract::Decision::Strike => true,
        crate::contract::Decision::Hold => {
            reset_chase(world, taker);
            let until = missions::hold_until(world.tick);
            if let Some(b) = world.comp_mut::<Brain>(taker) {
                b.cooldowns.insert(GoalKind::Contract, until);
            }
            false
        }
        _ => false,
    }
}
