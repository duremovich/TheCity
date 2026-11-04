use super::*;

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
        .map(|r| (crate::systems::combat::power(world, r), r))
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
pub(super) fn parley_pass(world: &mut World) {
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
