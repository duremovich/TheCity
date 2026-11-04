use super::*;

// ---------------------------------------------------------------------------
// Hunt hiring (C20)
// ---------------------------------------------------------------------------

/// C20: a Hunt holder too weak to win (`might_gap < hire_gap`) who can pay
/// posts a Hit (grudge `weight ≥ lethal_min`) or a Beat on its grudge
/// target instead of hunting: brokered with the nearest open Fixer within
/// `hire_reach_tiles` of its Home door, else direct; the Hunt cools for
/// `hunt_cooldown_days`. Tier-neutral: a Statistical holder posts at once.
pub fn try_hire(world: &mut World, holder: EntityId, target: EntityId, weight: f32) -> bool {
    // The Hit market (H1-H5, H12): priced at the market, paid with the
    // holder's means and kin, half down.
    if world.config.hitmarket.enabled {
        return crate::systems::hitmarket::try_revenge(world, holder, target, weight);
    }
    if crate::systems::hunt::might_gap(world, holder, target) >= world.config.contracts.hire_gap {
        return false;
    }
    let dup = world.by_party.get(&holder).is_some_and(|l| {
        l.iter().any(|id| {
            world.contracts.get(id).is_some_and(|c| {
                c.is_live() && c.origin == Origin::Hunt && c.buyer == Some(holder) && c.target == Target::Agent(target)
            })
        })
    });
    if dup {
        return false;
    }
    let kind = if weight >= world.config.hunt.lethal_min { ContractKind::Hit } else { ContractKind::Beat };
    let tgt = Target::Agent(target);
    let broker = nearest_fixer(world, holder);
    if world.purse(Some(holder)) < quote(world, kind, &tgt, broker) {
        return false;
    }
    let days = world.config.contracts.deadline_days.get(kind);
    let posting = Posting {
        buyer: Some(holder),
        agent: Some(holder),
        kind,
        target: tgt,
        broker,
        deadline_days: days,
        origin: Origin::Hunt,
        price: None,
    };
    let r = post(world, posting);
    note_refused(world, &r);
    if r.is_err() {
        return false;
    }
    let until = world.tick + Tick::from(world.config.hunt.hunt_cooldown_days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(holder) {
        b.cooldowns.insert(GoalKind::Hunt, until);
    }
    true
}

/// C20, daily: the grudge holders the Hunt score never reaches (every
/// tier, not hunting, not cooled), each on one hash-picked day in three,
/// whose heaviest eligible grudge passes the gap test and whose Hunt
/// considerations without the might term score `hire_min` or more, hire.
pub fn hire_pass(world: &mut World) {
    let day = world.day();
    let seed = world.seed();
    let now = world.tick;
    let min = world.config.contracts.hire_min;
    let mut queue: Vec<(EntityId, EntityId, f32)> = Vec::new();
    // scan-ok: daily, the grudge store.
    for id in world.with::<crate::word::Grudges>() {
        if world.hunts.contains_key(&id) {
            continue;
        }
        if !splitmix64(seed ^ u64::from(id.index) ^ day ^ HIRE_SALT).is_multiple_of(3) {
            continue;
        }
        if world.comp::<Brain>(id).and_then(|b| b.cooldowns.get(&GoalKind::Hunt)).is_some_and(|&t| t > now) {
            continue;
        }
        if !crate::systems::hunt::hunter_ok(world, id) {
            continue;
        }
        let Some((t, w, _)) = crate::systems::hunt::heaviest_eligible(world, id) else { continue };
        if !crate::systems::hitmarket::hires(world, id, t) {
            continue;
        }
        let Some((cs, flat)) = crate::systems::hunt::hire_considerations(world, id, t, w) else { continue };
        let Some(s) = crate::utility::score_goal(GoalKind::Hunt, cs, None, 0.0, flat) else { continue };
        if s.score >= min {
            queue.push((id, t, w));
        }
    }
    // The Hit market (H11): the heaviest wants first under the day's brake.
    if world.config.hitmarket.enabled {
        queue.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    }
    for (id, t, w) in queue {
        try_hire(world, id, t, w);
    }
}

// ---------------------------------------------------------------------------
// The Contract goal and its plans (C13, C15)
// ---------------------------------------------------------------------------

/// C13's gun gate: a free adult, jobless or paid below 0.7 × the median
/// wage, whose best of fighting, stealth, hacking, persuasion and
/// intimidation reaches `gun_skill_min`.
pub fn gun_gate(world: &World, id: EntityId) -> bool {
    // Cheapest first: this runs in every body's think (perf).
    let Some(s) = world.comp::<Skills>(id) else { return false };
    let best = crate::systems::combat::skill_power(world, s)
        .max(s.stealth)
        .max(s.hacking)
        .max(s.persuasion)
        .max(s.intimidation);
    if best < world.config.fixers.gun_skill_min {
        return false;
    }
    let wage = world.comp::<Job>(id).map(|j| j.wage_per_day);
    if wage.is_some_and(|w| w as f32 >= 0.7 * world.median_wage as f32) {
        return false;
    }
    free_adult(world, id)
}

/// C13: the Contract goal. Mode A (work a live record: the taker holds a
/// run) and mode B (network at a Fixer within reach: not a regular, or
/// lapsing within 2 days, and through the gun gate).
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    // Phase 2 (C22): a mission's crew member works its record as a taker does.
    let mission = if world.missions.is_empty() { None } else { world.mission_of.get(&id).copied() };
    // The crew goes about its day until the gather window (the Raid goal's
    // `raid_gather_hours`) opens before the muster.
    let window = Tick::from(world.config.gangs.raid_gather_hours) * TICKS_PER_HOUR;
    if mission.and_then(|m| world.missions.get(&m)).is_some_and(|m| world.tick + window < m.raid_at) {
        return None;
    }
    if let Some(cid) = world.contract_runs.get(&id).map(|r| r.contract).or(mission) {
        let c = world.contracts.get(&cid)?;
        if c.status != ContractStatus::Taken || !free_adult(world, id) {
            return None;
        }
        let total = c.deadline.saturating_sub(c.taken.unwrap_or(c.posted)).max(1) as f32;
        let left = c.deadline.saturating_sub(world.tick) as f32;
        let risk = match mission.and_then(|m| world.missions.get(&m)) {
            Some(m) => 1.0 - missions::estimate(world, &m.crew, &c.target),
            None => risk_for(world, c, id),
        };
        // Phase 2 (deviation): a mission's crew reads its muster, not the
        // deadline: inside the gather window the muster is called (urgency
        // 1, the Raid goal's "muster called").
        let urgency_v = if mission.is_some() { 1.0 } else { (1.0 - left / total).clamp(0.0, 1.0) };
        let mut cs = vec![Consideration::new("holds a taken contract", can(true), GATE)];
        cs.extend(mode_a(world, id, c, risk, urgency_v));
        return Some((cs, world.config.contracts.contract_flat));
    }
    // Round 9 Z2: work on a board he knows, weighed against his day: the
    // record's mode A and the walk to the desk.
    if let Some(w) = walk_in_pick(world, id, None) {
        let tiles = crate::systems::life::tiles_to(world, id, w.fixer).unwrap_or(0);
        let mut cs = vec![Consideration::new("work on a fixer's board", can(true), GATE)];
        cs.extend(w.cs);
        cs.push(crate::systems::life::travel(world, tiles));
        return Some((cs, world.config.fixers.walk_in_flat));
    }
    if world.buildings_of_kind(BuildingKind::Fixer).is_empty() || !gun_gate(world, id) {
        return None;
    }
    if is_regular(world, id, 2 * TICKS_PER_DAY) {
        return None;
    }
    nearest_fixer(world, id)?;
    let p = world.comp::<Personality>(id)?;
    let n = world.comp::<Needs>(id)?;
    let cs = vec![
        Consideration::new("can network", can(true), GATE),
        Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    Some((cs, world.config.fixers.network_flat))
}

/// C13, C15: the Contract goal's scripted plan (`plan::plan_for`'s bypass;
/// `None` cools the goal). A run: Hit and Beat `GoTo(Intel) → [AskAround
/// → GoTo(Intel)] → StakeOut → Attack` (a fresh sighting, the taker's or
/// its gang's or the buyer's database, skips the asking), a Locate the
/// same without the Attack, a Guard `GoTo(client) → Guard`. No run:
/// `GoTo(Fixer) → Network`.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    // Phase 2 (C22): a crew member marches with its mission.
    if !world.missions.is_empty() && world.mission_of.contains_key(&id) {
        return missions::plan(world, id);
    }
    let step = |action, target| ActionInstance { action, target, tile: None };
    let now = world.tick;
    let Some(r) = world.contract_runs.get(&id).cloned() else {
        // Round 9 Z2: the walk to the Fixer whose board has his job.
        if let Some(w) = walk_in_pick(world, id, None) {
            let f = w.fixer;
            // The fixer holds it for him while he comes.
            if let Some(c) = world.contracts.get_mut(&w.id) {
                c.claimed = Some((id, now));
            }
            let steps = vec![step(ActionKind::GoTo(LocationKey::Seller), Some(f)), step(ActionKind::TakeJob, Some(f))];
            return Some(Plan { goal: GoalKind::Contract, target: Some(f), steps, started_tick: now });
        }
        let f = nearest_fixer(world, id)?;
        let steps = vec![step(ActionKind::GoTo(LocationKey::Seller), Some(f)), step(ActionKind::Network, Some(f))];
        return Some(Plan { goal: GoalKind::Contract, target: Some(f), steps, started_tick: now });
    };
    let c = world.contracts.get(&r.contract)?.clone();
    if c.status != ContractStatus::Taken {
        return None;
    }
    if matches!(c.kind, ContractKind::Guard | ContractKind::Escort) {
        let steps = match c.target {
            Target::Building(b) => {
                vec![step(ActionKind::GoTo(LocationKey::Seller), Some(b)), step(ActionKind::Guard, Some(b))]
            }
            Target::Agent(a) => {
                let i = crate::systems::hunt::habit(world, a, now);
                if let Some(mut x) = crate::systems::hunt::chase(world, id) {
                    x.phase = HuntPhase::Watch;
                    x.intel = Some(i);
                    crate::systems::hunt::set_chase(world, id, x);
                }
                vec![step(ActionKind::GoTo(LocationKey::Intel), None), step(ActionKind::Guard, Some(a))]
            }
            _ => return None,
        };
        crate::systems::hunt::reindex(world);
        return Some(Plan { goal: GoalKind::Contract, target: Some(c.target.id()), steps, started_tick: now });
    }
    // Round 4 T3 (OPEN-222): a run no walk can work (a ledger kind turned
    // live before the gate, a building for a mark) goes back to the ledger,
    // said; a mark out of reach is waited out or let go, said.
    if ledger_kind(world, &c) || c.target_agent().is_none() {
        back_to_ledger(world, id, r.contract);
        return None;
    }
    let t = r.target;
    if !crate::systems::hunt::target_ok(world, t) {
        mark_out_of_reach(world, id, &c, t);
        return None;
    }
    let mut next = crate::systems::hunt::chase(world, id)?;
    if next.phase == HuntPhase::Ask {
        let fresh = crate::systems::hunt::fresh_intel(world, id, t).or_else(|| buyer_intel(world, &c, t));
        if let Some(i) = fresh {
            next.phase = HuntPhase::Watch;
            next.intel = Some(i);
        } else {
            match crate::systems::hunt::last_known_district(world, id, t)
                .and_then(|d| crate::systems::hunt::busiest_bar(world, d))
            {
                Some(v) => next.venue = Some(v),
                None => {
                    next.phase = HuntPhase::Watch;
                    next.intel = Some(crate::systems::hunt::habit(world, t, now));
                }
            }
        }
    }
    if next.phase == HuntPhase::Watch && next.intel.is_none() {
        next.intel = Some(crate::systems::hunt::habit(world, t, now));
    }
    // The pressure pass phase 4 (OPEN-194): the first leg within reach.
    if !crate::systems::hunt::within_reach(world, id, next.phase, &mut next.venue, next.intel) {
        // R6b (OPEN-217): the stakeout falls back to a door of the
        // target's the taker can reach (the take gate's: its Home, then
        // its post) before the goal cools: the habit's pick (a modal
        // district's busiest Bar, the post by day) stood past the reach and
        // V10's guns held a taken hit to its deadline, never walking.
        next.intel = Some(reachable_door(world, id, t)?);
        next.phase = HuntPhase::Watch;
    }
    let phase = next.phase;
    crate::systems::hunt::set_chase(world, id, next);
    crate::systems::hunt::reindex(world);
    // Round 8 X2 (`OPEN-247`): one more setting-out (`stalled_runs` says it).
    if let Some(x) = world.contract_runs.get_mut(&id) {
        x.tries = x.tries.saturating_add(1);
    }
    let tt = Some(t);
    let mut steps = vec![step(ActionKind::GoTo(LocationKey::Intel), None)];
    if phase == HuntPhase::Ask {
        steps.push(step(ActionKind::AskAround, tt));
        steps.push(step(ActionKind::GoTo(LocationKey::Intel), None));
    }
    steps.push(step(ActionKind::StakeOut, tt));
    if c.kind.violent() {
        steps.push(step(ActionKind::Attack, tt));
    }
    // M16b phase 2: an abduction drags the target to its hold.
    if c.kind == ContractKind::Abduct {
        let at = c.hold.map(|h| h.at)?;
        steps.push(step(ActionKind::Abduct, tt));
        steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(at)));
        steps.push(step(ActionKind::HandOver, Some(at)));
    }
    Some(Plan { goal: GoalKind::Contract, target: tt, steps, started_tick: now })
}

/// Round 4 T3 (`OPEN-222`): why mark `t` cannot be walked to now, and when
/// that ends if it does (a sentence's release); `None` for a mark in reach
/// of the chase, or one in a guard's hands (the cells come next: the plain
/// back-off waits for them).
/// Round 5 U5: `c`'s agent mark is away ([`mark_away`]) with no return
/// before `c`'s deadline.
pub(super) fn mark_gone_past_deadline(world: &World, c: &Contract) -> bool {
    c.target_agent().and_then(|t| mark_away(world, t)).is_some_and(|(_, until)| until.is_none_or(|u| u >= c.deadline))
}

pub(super) fn mark_away(world: &World, t: EntityId) -> Option<(String, Option<Tick>)> {
    let name = world.name_of(t);
    if !crate::systems::law::living(world, t) {
        return Some((format!("{name} is dead"), None));
    }
    if let Some(s) = world.comp::<Sentence>(t) {
        let until = s.until_tick;
        let when = format!("d{} {}", until / TICKS_PER_DAY, time::clock(until));
        return Some((format!("{name} is in the cells till {when}"), Some(until)));
    }
    let b = world.comp::<Brain>(t)?;
    if b.cuffed_by.is_some() {
        return None;
    }
    if b.emigrating {
        return Some((format!("{name} is leaving the city"), None));
    }
    if crate::systems::abroad::is_away(world, t) {
        return Some((format!("{name} is away from the city"), None));
    }
    None
}

/// Round 4 T3 (`OPEN-222`): the taker of a run whose mark is out of reach
/// (`mark_away`): a mark out of the cells before the deadline is waited
/// out (the Contract goal cooled to the release, one `Pursuit` line);
/// otherwise the taker lets it go (`fail_attempt` with the reason). V12 and
/// 20k seed 42: held Locates and Tags on jailed marks cooled every 2-12 h
/// to the deadline (955 cools in 30 days at 20k), then expired unsaid.
fn mark_out_of_reach(world: &mut World, taker: EntityId, c: &Contract, t: EntityId) {
    let Some((why, until)) = mark_away(world, t) else { return };
    let (kind, tname, who) = (c.kind.label(), world.name_of(t), world.name_of(taker));
    match until {
        Some(u) if u < c.deadline => {
            if let Some(b) = world.comp_mut::<Brain>(taker) {
                b.cooldowns.insert(GoalKind::Contract, u);
            }
            let text = format!("{who} holds the {kind} on {tname} and waits: {why}");
            world.push_event(EventKind::Pursuit, &[taker, t], text.clone());
            log(world, c.id, text);
        }
        _ => fail_attempt(world, c.id, &format!("{who} let it go: {why}")),
    }
}

/// Round 4 T3 (`OPEN-222`): `taker`'s live run on record `id` that no walk
/// works goes back to the ledger: the run ended, a `due` drawn over half the
/// time left (`ContractNs::Due`, key `1 + attempts`, as
/// `ledger_stale_queue`), one `Pursuit` line.
fn back_to_ledger(world: &mut World, taker: EntityId, id: ContractId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    end_run(world, taker, id);
    let now = world.tick;
    let span = (c.deadline.saturating_sub(now) / 2).max(60);
    let u: f64 = world.rng.contract(ContractNs::Due, id, 1 + u64::from(c.attempts)).random();
    let due = now + (u * span as f64).floor() as Tick;
    if let Some(x) = world.contracts.get_mut(&id) {
        x.render = Render::Ledger;
        x.due = Some(due);
    }
    world.ledger_due.insert((due, id));
    let tname = world.name_of(c.target.id());
    let text = format!(
        "{} works the {} on {tname} off the street: no walk plans it (due d{} {})",
        world.name_of(taker),
        c.kind.label(),
        due / TICKS_PER_DAY,
        time::clock(due)
    );
    world.push_event(EventKind::Pursuit, &[taker, c.target.id()], text.clone());
    log(world, id, text);
}

/// R6b (OPEN-217): the first of the target's Home and workplace doors
/// within `[distance.reach] stakeout_ticks` of the taker's origin (the
/// doors `hunt_in_reach` admitted the take on), as a habit intel; `None`
/// without `[verbs]` or with neither in reach.
fn reachable_door(world: &World, id: EntityId, t: EntityId) -> Option<Intel> {
    world.config.verbs.as_ref()?;
    let cap = crate::systems::distance::reach(world).map(|r| r.stakeout_ticks)?;
    let from = crate::systems::life::origin(world, id)?;
    let home = world.comp::<crate::components::Household>(t).and_then(|h| h.home);
    let post = world.comp::<Job>(t).and_then(|j| j.employer);
    [home, post].into_iter().flatten().find_map(|b| {
        let door = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.door)?;
        (crate::systems::distance::travel(world, from, door) <= cap).then_some(Intel {
            building: Some(b),
            tile: door,
            source: IntelSource::Habit,
        })
    })
}

/// A sighting of `t` in the buyer's database younger than
/// `fresh_sighting_hours` (C15: the buyer's intel skips the asking).
pub(super) fn buyer_intel(world: &World, c: &Contract, t: EntityId) -> Option<Intel> {
    let owner = c.buyer.unwrap_or(EntityId::NONE);
    let horizon = world.tick.saturating_sub(Tick::from(world.config.hunt.fresh_sighting_hours) * TICKS_PER_HOUR);
    world
        .db
        .get(&owner)?
        .sightings
        .iter()
        .filter(|s| s.who == t && s.tick >= horizon)
        .max_by_key(|s| s.tick)
        .map(|s| Intel { building: None, tile: s.tile, source: IntelSource::Sighting })
}

/// Inside `b`, or within a tile of its outside door.
fn at_building(world: &World, id: EntityId, b: EntityId) -> bool {
    world.comp::<Position>(id).is_some_and(|p| {
        p.building == Some(b)
            || world
                .comp::<Building>(b)
                .is_some_and(|bd| crate::systems::law::chebyshev(p.tile, world.outside_door(bd)) <= 1)
    })
}

/// The scripted steps' start check (`exec::start_step`'s scripted branch
/// for `GoalKind::Contract`, and `actions::can_start` for the two
/// contract steps): `Network` at an open Fixer, `Guard` at its client
/// while the record is taken; the chase steps through `hunt::can_start`.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    match kind {
        // The Hit market (H9): the Commission step likewise.
        ActionKind::Network | ActionKind::Commission => {
            target.is_some_and(|f| fixer_open(world, f) && at_building(world, id, f))
        }
        // Round 9 Z2: the walk-in at a manned desk.
        ActionKind::TakeJob => target.is_some_and(|f| {
            fixer_open(world, f)
                && at_building(world, id, f)
                && crate::systems::service::desk_server(world, f, id).is_some()
        }),
        ActionKind::Guard => {
            let Some(r) = world.contract_runs.get(&id) else { return false };
            let taken = world.contracts.get(&r.contract).is_some_and(|c| c.status == ContractStatus::Taken);
            taken
                && target.is_some_and(|t| {
                    if world.has::<Building>(t) {
                        at_building(world, id, t)
                    } else {
                        crate::systems::law::living(world, t)
                            && (crate::systems::law::near(world, id, t, 3)
                                || r.intel.is_some_and(|i| match (i.building, world.comp::<Position>(id)) {
                                    (Some(b), _) => at_building(world, id, b),
                                    (None, Some(p)) => crate::systems::law::chebyshev(p.tile, i.tile) <= 1,
                                    _ => false,
                                }))
                    }
                })
        }
        // Phase 2 (C22): the mission's muster and brawl.
        ActionKind::Muster | ActionKind::Brawl => missions::can_start(world, id, kind),
        // M16b phase 2: an Abduct's grab (within an Attack's reach) and hand-over.
        ActionKind::Abduct => target.is_some_and(|v| {
            crate::systems::law::living(world, v)
                && crate::systems::law::near(world, id, v, 4)
                && !crate::systems::law::held_apart(world, id, v)
                && world.comp::<Brain>(v).is_some_and(|b| b.cuffed_by.is_none())
        }),
        ActionKind::HandOver => {
            target.is_some_and(|b| at_building(world, id, b)) && crate::systems::chrome::dragging(world, id).is_some()
        }
        // The walk to the hold: in tow or not, the step starts.
        ActionKind::GoTo(LocationKey::Seller)
            if world
                .comp::<Brain>(id)
                .and_then(|b| b.plan.as_ref())
                .is_some_and(|p| p.steps.iter().any(|s| s.action == ActionKind::HandOver)) =>
        {
            true
        }
        _ => crate::systems::hunt::can_start(world, id, kind, target),
    }
}

/// C10: `Network` completes: a regular of the Fixer from now (a gang
/// member's gang networked there too).
pub fn on_network(world: &mut World, id: EntityId, fixer: Option<EntityId>) -> StepResult {
    let Some(f) = fixer.filter(|&f| fixer_open(world, f)) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    stamp_regular(world, id, f);
    // Round 9 Z2: a new regular reads the board at the door and takes what
    // he would (quiet when there is nothing for him). M18 P3: the human
    // reads it on his board and picks.
    if !crate::systems::player::drives(world, id) {
        take_at_desk(world, id, f, false);
    }
    StepResult::Done
}

/// C32: a Guard post starts: the taker stands over its client.
pub fn on_guard_start(world: &mut World, id: EntityId, client: Option<EntityId>) {
    if let Some(c) = client {
        let list = world.contract_guards.entry(c).or_default();
        if !list.contains(&id) {
            list.push(id);
        }
    }
}

/// C32: a Guard post ends (done or abandoned).
pub fn on_guard_end(world: &mut World, id: EntityId, client: Option<EntityId>) {
    if let Some(c) = client {
        if let Some(list) = world.contract_guards.get_mut(&c) {
            list.retain(|g| *g != id);
            if list.is_empty() {
                world.contract_guards.remove(&c);
            }
        }
    }
}

/// C15: a Guard post completes: off post, the goal cooled until the next
/// day's post (once a day, taking to deadline).
pub fn on_guard_done(world: &mut World, id: EntityId, client: Option<EntityId>) -> StepResult {
    on_guard_end(world, id, client);
    let rest = (24u64.saturating_sub(u64::from(world.config.fixers.guard_hours))).max(1) * TICKS_PER_HOUR;
    let until = world.tick + rest;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.cooldowns.insert(GoalKind::Contract, until);
    }
    StepResult::Done
}
