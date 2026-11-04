use super::*;

/// C12's board order: Fixer `f`'s Open records by `price × (1 + age_days ÷
/// deadline_days)` descending (ties the lower id), a mark held past the
/// deadline left off (round 5 U5, OPEN-217: the fixer puts nobody on a job
/// whose mark is in the cells, abroad or leaving till after it; 20k: three
/// Locates taken at 18:00 on marks in the cells till d36, each let go
/// within the hour). The daily match offers its top `offers_per_day`; a
/// walk-in reads its top `board_size` (round 9 Z2).
pub(super) fn ranked_book(world: &World, f: EntityId) -> Vec<ContractId> {
    let now = world.tick;
    let Some(k) = world.comp::<Broker>(f) else { return Vec::new() };
    let mut book: Vec<(f64, ContractId)> = k
        .book
        .iter()
        .filter_map(|id| world.contracts.get(id))
        .filter(|c| c.is_open())
        .filter(|c| !mark_gone_past_deadline(world, c))
        .map(|c| {
            let age = now.saturating_sub(c.posted) as f64 / TICKS_PER_DAY as f64;
            let days = (c.deadline.saturating_sub(c.posted) as f64 / TICKS_PER_DAY as f64).max(1.0);
            (c.price as f64 * (1.0 + age / days), c.id)
        })
        .collect();
    book.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    book.into_iter().map(|(_, id)| id).collect()
}

// ---------------------------------------------------------------------------
// Round 9 Z2: the walk-in (the take as a body's act)
// ---------------------------------------------------------------------------

/// Round 9 Z2: a would-be taker's pick off a Fixer's board.
#[derive(Clone, Debug)]
pub struct WalkIn {
    pub fixer: EntityId,
    pub id: ContractId,
    /// `Some(gang)`: a crew's take, the walker its leader.
    pub gang: Option<EntityId>,
    /// Mode A's raw product for the walker (the take's weight).
    pub score: f32,
    /// Mode A's considerations (the Contract goal's walk-in reads them).
    pub cs: Vec<Consideration>,
}

/// Round 9 Z2: the record `who` (a body) would walk to a Fixer and take now,
/// the best by mode A's product (ties the first found: Fixers ascending,
/// each board in its rank): he reads the board of each open Fixer whose
/// desk is manned (`service::desk_server`) and that knows him: a gun
/// through the gun gate who is its regular (the fixer's word reaches his
/// regulars; a first visit's `Network` reads it at the door), or the leader
/// of a gang networked there (a crew's take is its leader's walk). An
/// individual's take is `eligible` and `hunt_in_reach` with mode A at least
/// `accept_min` (risk against his skill and the target's guard, the buyer's
/// honour); a crew's is `gang_eligible` with `leader_accepts` (the match's
/// rule), weighed by mode A against the crew's odds. A bribe for the law is
/// no walk-in's (the guard's shift takes it at the match). `at` limits the
/// read to one Fixer (the desk he stands at). Never a minor's
/// (`ages::is_protected`, also in `eligible` and the crew's gates).
pub fn walk_in_pick(world: &World, who: EntityId, at: Option<EntityId>) -> Option<WalkIn> {
    if !crate::systems::presence::is_body(world, who)
        || !free_adult(world, who)
        || crate::systems::ages::is_protected(world, who)
        || world.contract_runs.contains_key(&who)
        || world.mission_of.contains_key(&who)
    {
        return None;
    }
    let fixers = world.buildings_of_kind(BuildingKind::Fixer);
    if fixers.is_empty() {
        return None;
    }
    let gang =
        world.gang_of(who).filter(|&g| world.comp::<crate::components::Gang>(g).is_some_and(|x| x.leader == Some(who)));
    let gun = gun_gate(world, who);
    if gang.is_none() && !gun {
        return None;
    }
    if holds_taken(world, who) {
        return None;
    }
    let cfg = &world.config;
    let keep = Tick::from(cfg.fixers.regular_days) * TICKS_PER_DAY;
    let now = world.tick;
    let min = cfg.contracts.accept_min;
    let mut best: Option<WalkIn> = None;
    let take = |w: WalkIn, best: &mut Option<WalkIn>| {
        if best.as_ref().is_none_or(|b| w.score > b.score) {
            *best = Some(w);
        }
    };
    for &f in fixers {
        if at.is_some_and(|a| a != f) || !fixer_open(world, f) {
            continue;
        }
        let Some(k) = world.comp::<Broker>(f) else { continue };
        let regular = gun && k.regulars.get(&who).is_some_and(|&t| t + keep > now);
        let crew_of = gang.filter(|g| k.gangs.contains(g));
        if !regular && crew_of.is_none() {
            continue;
        }
        if crate::systems::service::desk_server(world, f, who).is_none() {
            continue;
        }
        for id in ranked_book(world, f).into_iter().take(usize::from(cfg.fixers.board_size)) {
            let Some(c) = world.contracts.get(&id) else { continue };
            if is_corruption(world, c) || held_for_other(world, c, who) {
                continue;
            }
            // The under-18 rule: a minor is no mark (`target_ok` at the post; again at the take).
            if !matches!(c.kind, ContractKind::Guard | ContractKind::Escort)
                && c.target_agent().is_some_and(|t| crate::systems::ages::is_protected(world, t))
            {
                continue;
            }
            if regular && eligible(world, c, who) && hunt_in_reach(world, c, who) {
                let s_def = missions::defender_strength(world, &c.target);
                let cs = mode_a(world, who, c, risk_against(world, c.kind, who, s_def), 1.0);
                let score: f32 = cs.iter().map(|x| x.output).product();
                if score >= min {
                    take(WalkIn { fixer: f, id, gang: None, score, cs }, &mut best);
                }
            }
            if let Some(g) = crew_of {
                if gang_eligible(world, c, g) && leader_accepts(world, g, missions::buyer_faction(world, c)) {
                    let crew = job_crew(world, c, g);
                    let risk = 1.0 - missions::estimate(world, &crew, &c.target);
                    let cs = mode_a(world, who, c, risk, 1.0);
                    let score: f32 = cs.iter().map(|x| x.output).product();
                    take(WalkIn { fixer: f, id, gang: Some(g), score, cs }, &mut best);
                }
            }
        }
    }
    best
}

/// Round 9 Z2: record `c` is held for another walk-in on his way (claimed
/// within `[fixers] walk_in_hold_mins`).
pub(super) fn held_for_other(world: &World, c: &Contract, who: EntityId) -> bool {
    let hold = Tick::from(world.config.fixers.walk_in_hold_mins);
    c.claimed.is_some_and(|(w, at)| w != who && world.tick < at + hold)
}

/// Round 9 Z2: `who` at Fixer `f`'s desk takes the best record off its board
/// in person ([`walk_in_pick`] at `f`): his own (a weak gun brings a squad,
/// C25) or his gang's job. `said`: a walk-in who finds the job gone says
/// so (a first visit's `Network` reads the board quietly). `true` taken.
pub(super) fn take_at_desk(world: &mut World, who: EntityId, f: EntityId, said: bool) -> bool {
    let Some(w) = walk_in_pick(world, who, Some(f)) else {
        if said {
            let text = format!("{} came to {} for work: the job was gone", world.name_of(who), world.name_of(f));
            let mut actors: SmallVec<[EntityId; 2]> = SmallVec::from_slice(&[who]);
            if let Some(h) = crate::systems::service::desk_server(world, f, who) {
                actors.push(h);
            }
            world.push_event(EventKind::Contract, &actors, text);
        }
        return false;
    };
    let Some(c) = world.contracts.get(&w.id).cloned() else { return false };
    match w.gang {
        Some(g) => {
            let crew = job_crew(world, &c, g);
            !crew.is_empty() && accept_at(world, w.id, g, &crew, Some(who))
        }
        None => {
            let crew = squad_crew(world, &c, who);
            accept_at(world, w.id, who, &crew, Some(who))
        }
    }
}

/// Round 9 Z2: `TakeJob` completes at Fixer `fixer`'s desk: his stamp as a
/// regular renewed, the job off its board taken in person ([`take_at_desk`]).
pub fn on_take_job(world: &mut World, id: EntityId, fixer: Option<EntityId>) -> StepResult {
    let Some(f) = fixer.filter(|&f| fixer_open(world, f)) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    stamp_regular(world, id, f);
    // M18 P3: the human picked the record off the board: that one is taken.
    if crate::systems::player::drives(world, id) {
        match claimed_at(world, id, f) {
            Some(c) => {
                player_take(world, id, c);
            }
            None => {
                let text = format!("{} came to {} for work: the job was gone", world.name_of(id), world.name_of(f));
                world.push_event(EventKind::Contract, &[id], text);
            }
        }
        return StepResult::Done;
    }
    take_at_desk(world, id, f, true);
    StepResult::Done
}

/// C10: `id` a regular of Fixer `f` from now (a gang member's gang networked
/// there too).
pub(super) fn stamp_regular(world: &mut World, id: EntityId, f: EntityId) {
    let now = world.tick;
    let gang = world.gang_of(id);
    if let Some(k) = world.comp_mut::<Broker>(f) {
        k.regulars.insert(id, now);
        if let Some(g) = gang {
            k.gangs.insert(g);
        }
    }
}

/// C11: a direct record's own people: the buyer's edges but Enemies, its
/// gang's members, its corp's staff (unsorted; may repeat).
pub(super) fn direct_pool(world: &World, buyer: EntityId) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    if world.has::<Identity>(buyer) {
        out.extend(world.neighbours(buyer).filter(|&o| world.edge(buyer, o).is_some_and(|e| e.kind != RelKind::Enemy)));
    }
    let gang = if world.has::<crate::components::Gang>(buyer) { Some(buyer) } else { world.gang_of(buyer) };
    if let Some(g) = gang.and_then(|g| world.comp::<crate::components::Gang>(g)) {
        out.extend(g.members.iter().copied());
    }
    if world.has::<crate::components::Corp>(buyer) {
        out.extend(crate::systems::ownership::employees_of(world, buyer));
    }
    out
}

/// C11: a direct record's viewers who could take it: the buyer's edges but
/// Enemies, its gang's members, its corp's staff (a public record: every
/// adult).
pub(super) fn direct_candidates(world: &World, id: ContractId) -> Vec<EntityId> {
    let Some(c) = world.contracts.get(&id) else { return Vec::new() };
    let Some(buyer) = c.buyer else {
        // scan-ok: daily, a public record (god only in phase 1).
        return world.citizens();
    };
    let mut out = direct_pool(world, buyer);
    // C31: the on-shift city guards of the buyer's district.
    if is_corruption(world, c) {
        out.extend(crate::systems::law_brain::guards(world).into_iter().filter(|&g| guard_viewer(world, c, g)));
    }
    out.sort();
    out.dedup();
    out.retain(|&a| sees(world, a, id));
    if direct_gated(world, c.kind) {
        out.retain(|&a| gun_gate(world, a));
    }
    out
}

/// The Hit market (P2 fix round): a direct Hit or Beat goes to the buyer's
/// people who pass the gun gate (C13), as a brokered one goes to the
/// Fixer's regulars, who passed it to network: a man with a day job paid
/// above 0.7 × the median wage takes no direct work either. With the
/// market off: M16a's viewers.
pub(super) fn direct_gated(world: &World, kind: ContractKind) -> bool {
    world.config.hitmarket.enabled && kind.violent()
}
