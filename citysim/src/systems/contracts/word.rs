use super::*;

/// Round 3 S5 (OPEN-217): a Hit, Beat or Abduct taken on an agent is heard
/// of: a corp's exec through his security (`missions::exec_guards`, who
/// know the street) at once; anyone else on `[verbs] hit_heard_p` (a hash
/// of the record), the street's tip. The target's safety falls by
/// `hit_heard_safety` (a `Threatened` memory of the taker), the event names
/// him and his guards (their diaries hold it). The client is not named.
/// A beat on a CEO was in his file and invisible to him (`ceo_88522 d24`:
/// a club, a hotel, twelve hours' sleep). Nothing with `[verbs]` absent.
/// Round 9 Z2: V18's `marked_30280` (a beat taken twice, by two crews) and
/// both guns' marks never heard: the tip was one hash of the record and the
/// target, so a record taken again rolled the same miss every time, and
/// nothing else carried word. Now a target hears once per record (`heard`),
/// by the first of: his security; word from someone he knows among the
/// taker, the crew and the one behind the desk (an edge that is no Rival or
/// Enemy); the street's tip drawn per take (the record, the taker and the
/// attempt); and a crew at his door that found him out ([`heard_at_door`]).
pub(super) fn heard_of(world: &mut World, id: ContractId, taker: EntityId) {
    const PURPOSE_HEARD: u64 = 0xc0_7e_a7_d1;
    let Some(v) = world.config.verbs.clone() else { return };
    let Some((kind, target, attempts, crew, broker)) = world.contracts.get(&id).and_then(|c| match c.target {
        Target::Agent(t)
            if !c.heard && matches!(c.kind, ContractKind::Hit | ContractKind::Beat | ContractKind::Abduct) =>
        {
            Some((c.kind, t, c.attempts, c.crew.clone(), c.broker))
        }
        _ => None,
    }) else {
        return;
    };
    if !crate::systems::law::living(world, target) {
        return;
    }
    let guards = missions::exec_guards(world, target);
    let (who, by) = (world.name_of(target), world.owner_label(Some(taker)));
    let text = if guards.is_empty() {
        let hand = broker.and_then(|b| crate::systems::service::fixer_hand(world, b));
        let knows = |p: EntityId| {
            p != target && world.edge(target, p).is_some_and(|e| !matches!(e.kind, RelKind::Rival | RelKind::Enemy))
        };
        let tie = std::iter::once(taker).chain(crew.iter().copied()).chain(hand).find(|&p| knows(p));
        let key = (u64::from(taker.index) << 32 | u64::from(target.index)) ^ (u64::from(attempts) << 56);
        let tipped = crate::systems::econ::hash_unit(world.seed(), PURPOSE_HEARD, id, key) < v.hit_heard_p;
        match tie {
            Some(p) => format!("{who} got word from {}: {by} took the {} on {who}", world.name_of(p), kind.label()),
            None if tipped => format!("{who} got word on the street: {by} took the {} on {who}", kind.label()),
            None => return,
        }
    } else {
        let names: Vec<String> = guards.iter().take(3).map(|&g| world.name_of(g)).collect();
        format!(
            "{}, {who}'s security, heard {by} took the {} on {who} and warned the boss",
            names.join(", "),
            kind.label()
        )
    };
    let mut actors = vec![target];
    actors.extend(guards.iter().take(3).copied());
    hear(world, id, target, taker, &actors, text);
}

/// Round 9 Z2: target `target` has heard of record `id` (`taker` its
/// taker): his safety falls by `hit_heard_safety`, a `Threatened` memory of
/// the taker, the line (`actors` hold it), and he acts on it ([`react`]).
fn hear(world: &mut World, id: ContractId, target: EntityId, taker: EntityId, actors: &[EntityId], text: String) {
    let Some(v) = world.config.verbs.clone() else { return };
    if let Some(c) = world.contracts.get_mut(&id) {
        c.heard = true;
    }
    if let Some(n) = world.comp_mut::<crate::components::Needs>(target) {
        n.safety = (n.safety - v.hit_heard_safety).max(0.0);
    }
    world.remember(target, crate::components::MemoryKind::Threatened, Some(taker), 0.6, -0.5, false);
    ownership::jobs2_row(world).hit_heard += 1;
    world.push_event(EventKind::Contract, actors, text);
    react(world, id, target, taker);
}

/// Round 9 Z2: `hunter`, working a taken record on his mark, asked after him
/// of `r` at `venue` (`hunt::ask_around`): one close to the mark (a Friend,
/// kin or spouse) tells him. Once per record, as [`heard_of`].
pub fn asked_after(world: &mut World, hunter: EntityId, r: EntityId, venue: Option<EntityId>) {
    if world.config.verbs.is_none() {
        return;
    }
    let Some(id) = world.contract_runs.get(&hunter).map(|x| x.contract) else { return };
    let Some((kind, target, taker)) = world.contracts.get(&id).and_then(|c| match (c.target, c.taker) {
        (Target::Agent(t), Some(k))
            if !c.heard
                && c.status == ContractStatus::Taken
                && matches!(c.kind, ContractKind::Hit | ContractKind::Beat | ContractKind::Abduct) =>
        {
            Some((c.kind, t, k))
        }
        _ => None,
    }) else {
        return;
    };
    let close = world
        .edge(r, target)
        .is_some_and(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse | RelKind::Parent));
    if !close || r == target || !crate::systems::law::living(world, target) {
        return;
    }
    let who = world.name_of(target);
    let at = venue.map(|v| format!(" at {}", world.name_of(v))).unwrap_or_default();
    let him = if crate::systems::grief::his(world, target) == "his" { "him" } else { "her" };
    let text = format!(
        "{who} got word from {}: {} was asking after {him}{at} (the {} on {who})",
        world.name_of(r),
        world.name_of(hunter),
        kind.label()
    );
    hear(world, id, target, taker, &[target, r], text);
}

/// Round 9 Z2: a crew that came to `place` for record `id`'s mark and found
/// him out (`missions`' "found no X"): the mark hears of it from whoever was
/// at his door (V18 `marked_30280`: two crews at Block#201 in two nights,
/// no word). Once per record, as [`heard_of`].
pub fn heard_at_door(world: &mut World, id: ContractId, place: &str) {
    if world.config.verbs.is_none() {
        return;
    }
    let Some((kind, target, taker)) = world.contracts.get(&id).and_then(|c| match (c.target, c.taker) {
        (Target::Agent(t), Some(k))
            if !c.heard && matches!(c.kind, ContractKind::Hit | ContractKind::Beat | ContractKind::Abduct) =>
        {
            Some((c.kind, t, k))
        }
        _ => None,
    }) else {
        return;
    };
    if !crate::systems::law::living(world, target) || crate::systems::ages::is_protected(world, target) {
        return;
    }
    let who = world.name_of(target);
    let him = crate::systems::grief::his(world, target);
    let him = if him == "his" { "him" } else { "her" };
    let text = format!(
        "{who} heard a crew came to {place} for {him}: {} holds the {} on {who}",
        world.owner_label(Some(taker)),
        kind.label()
    );
    hear(world, id, target, taker, &[target], text);
}

/// A live Guard record on agent `t` (his own bodyguard, standing or asked for).
fn guarded(world: &World, t: EntityId) -> bool {
    world.by_party.get(&t).is_some_and(|l| {
        l.iter().any(|id| {
            world.contracts.get(id).is_some_and(|c| {
                c.is_live()
                    && matches!(c.kind, ContractKind::Guard | ContractKind::Escort)
                    && c.target == Target::Agent(t)
            })
        })
    })
}

/// Round 8 X2 (`OPEN-247`): the word of a Hit or Beat taken on `target`
/// acted on, each with a line (V16 `ceo_20314`: a 1,453c hit taken by a
/// crew of four, his security warned him, he slept 295 min and idled). One
/// at `[verbs] hit_guard_wealth` whose purse holds the quote, or a corp's
/// exec whose corp's treasury does, hires a guard on himself (a Guard record
/// at his nearest Fixer, the existing bodyguard); one brave and lawless enough (`hit_counter_courage`,
/// `hit_counter_lawful`) swears on the taker's lead (a `Hired` grudge the
/// Hunt and the hire read: a counter-hit when he is weak and can pay; never
/// on an under-18, `ages::is_protected`); one who does neither lies low at
/// home `hit_low_days` (`grief::lie_low`: his shifts struck and counted,
/// his free hours indoors). The target is an adult (`target_ok`).
fn react(world: &mut World, id: ContractId, target: EntityId, taker: EntityId) {
    let Some(v) = world.config.verbs.clone() else { return };
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let who = world.name_of(target);
    let his = crate::systems::grief::his(world, target);
    let him = if his == "his" { "himself" } else { "herself" };
    // A guard already on him keeps close (no second post, no lying low).
    let mut acted = guarded(world, target);
    // A corp's exec has his corp pay (his purse is his pocket; the corp's
    // treasury keeps him: 20k `ceo_16592`, 1c in hand, lay low instead).
    let corp = world
        .corp_slice()
        .iter()
        .copied()
        .find(|&k| world.comp::<crate::components::Corp>(k).and_then(|x| x.exec) == Some(target));
    let wealth = world.comp::<Needs>(target).map_or(0.0, |n| n.wealth);
    if (corp.is_some() || wealth >= v.hit_guard_wealth) && !guarded(world, target) {
        let tgt = Target::Agent(target);
        let fixer = nearest_fixer(world, target);
        let price = quote(world, ContractKind::Guard, &tgt, fixer);
        let payer = corp.unwrap_or(target);
        if world.purse(Some(payer)) >= price {
            let posting = Posting {
                buyer: Some(payer),
                agent: Some(target),
                kind: ContractKind::Guard,
                target: tgt,
                broker: fixer,
                deadline_days: world.config.contracts.deadline_days.get(ContractKind::Guard),
                origin: Origin::Hunt,
                price: None,
            };
            if post(world, posting).is_ok() {
                let at = fixer.map(|f| format!(" at {}", world.name_of(f))).unwrap_or_default();
                let by = corp.map(|k| format!(", {} paying", world.owner_label(Some(k)))).unwrap_or_default();
                let text = format!(
                    "{who} put a guard on {him} for {price}c{at}{by}: word of the {} on {his} head",
                    c.kind.label()
                );
                world.push_event(EventKind::Contract, &[target], text);
                ownership::jobs2_row(world).hit_guarded += 1;
                acted = true;
            }
        }
    }
    let lead = if is_gang(world, taker) { c.crew.first().copied() } else { Some(taker) };
    let bold = world
        .comp::<Personality>(target)
        .is_some_and(|p| p.courage >= v.hit_counter_courage && p.lawfulness <= v.hit_counter_lawful);
    if let Some(l) = lead.filter(|&l| {
        bold && crate::systems::law::living(world, l)
            && crate::systems::demography::is_adult(world, l)
            && !crate::systems::ages::is_protected(world, l)
    }) {
        crate::systems::grudges::add(world, target, l, crate::word::GrudgeCause::Hired, v.hit_counter_w, 0);
        let text = format!(
            "{who} swore to get {} first: {} took the {} on {who}",
            world.name_of(l),
            world.owner_label(Some(taker)),
            c.kind.label()
        );
        world.push_event(EventKind::Contract, &[target, l], text);
        ownership::jobs2_row(world).hit_countered += 1;
        acted = true;
    }
    if !acted && v.hit_low_days > 0 {
        let until = (world.tick / TICKS_PER_DAY + 1 + u64::from(v.hit_low_days)) * TICKS_PER_DAY;
        let why = format!("word of the {} on {his} head", c.kind.label());
        let text = format!("{who} lies low at home for {} days: {why}", v.hit_low_days);
        world.push_event(EventKind::Contract, &[target], text);
        crate::systems::grief::lie_low(world, target, until, why);
        ownership::jobs2_row(world).hit_low += 1;
    }
}

/// C12: one record's offers: its candidates by `fit` descending (ties the
/// lower id), offered in turn until one accepts (`accept_score ≥
/// accept_min`). A refusal costs nothing; one acceptance per candidate per
/// day (`taken_today`).
pub(super) fn offer(
    world: &mut World,
    id: ContractId,
    cands: Vec<EntityId>,
    gangs: Vec<EntityId>,
    taken_today: &mut std::collections::BTreeSet<EntityId>,
) -> bool {
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let s_def = missions::defender_strength(world, &c.target);
    let mut scored: Vec<(f32, EntityId)> = cands
        .into_iter()
        .filter(|a| !taken_today.contains(a))
        .filter(|&a| eligible(world, &c, a))
        // The pressure pass F1: a hunt's taker within reach of its target.
        .filter(|&a| hunt_in_reach(world, &c, a))
        .map(|a| (fit(world, &c, a, s_def), a))
        .collect();
    // Phase 2 (C12, C26): a gang is a candidate for a Hit or a Beat, scored
    // by its best member.
    for g in gangs {
        if taken_today.contains(&g) || !gang_eligible(world, &c, g) {
            continue;
        }
        let best = job_crew(world, &c, g).iter().map(|&m| fit(world, &c, m, s_def)).fold(0.0f32, f32::max);
        scored.push((best, g));
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let min = world.config.contracts.accept_min;
    for (_, a) in scored {
        if world.has::<crate::components::Gang>(a) {
            if leader_accepts(world, a, missions::buyer_faction(world, &c)) {
                taken_today.insert(a);
                return accept_gang(world, id, a);
            }
            continue;
        }
        let risk = risk_against(world, c.kind, a, s_def);
        let score: f32 = mode_a(world, a, &c, risk, 1.0).iter().map(|x| x.output).product();
        if score >= min {
            // Phase 2 (C25): a weak gun brings a squad.
            let crew = squad_crew(world, &c, a);
            taken_today.insert(a);
            taken_today.extend(crew.iter().copied());
            return accept(world, id, a, &crew);
        }
    }
    false
}

/// C12, daily at `match_hour`: (1) each open Fixer (by id) ranks its book's
/// Open records by `price × (1 + age_days ÷ deadline_days)` (ties the
/// lower id) and works the top `offers_per_day` against its regulars; (2)
/// every Open direct record (by id) against its viewers; (3) public
/// Locates are never matched (a stream, C27). Gangs and Security corps as
/// candidates are phases 2 and 3. Round 9 Z2: a Fixer's book is offered to
/// its Statistical regulars and to the gangs whose leader is no body (the
/// mirror); a body takes off the board in person (`walk_in_pick`).
pub fn match_day(world: &mut World) {
    let mut taken_today = std::collections::BTreeSet::new();
    for f in open_fixers(world) {
        let Some(k) = world.comp::<Broker>(f) else { continue };
        // Round 9 Z2: a body takes a brokered record in person (the walk-in,
        // `walk_in_pick`); the match offers to the Statistical regulars and
        // to the gangs whose leader is no body (the mirror).
        let regulars: Vec<EntityId> =
            k.regulars.keys().copied().filter(|&r| !crate::systems::presence::is_body(world, r)).collect();
        let gangs: Vec<EntityId> = k
            .gangs
            .iter()
            .copied()
            .filter(|&g| {
                !world
                    .comp::<crate::components::Gang>(g)
                    .and_then(|x| x.leader)
                    .is_some_and(|l| crate::systems::presence::is_body(world, l))
            })
            .collect();
        let book = ranked_book(world, f);
        for &id in book.iter().take(usize::from(world.config.fixers.offers_per_day)) {
            // Round 9 Z2: held for a walk-in on his way.
            if world.contracts.get(&id).is_some_and(|c| held_for_other(world, c, EntityId::NONE)) {
                continue;
            }
            // C32: a Security corp converts a Guard on a building first.
            if corp_take(world, id) {
                continue;
            }
            offer(world, id, regulars.clone(), gangs.clone(), &mut taken_today);
        }
        // C34: the Fixer's run orders, after its book.
        run_offers(world, f);
    }
    let direct: Vec<ContractId> = world
        .contracts
        .values()
        .filter(|c| c.is_open() && c.broker.is_none())
        .filter(|c| !(c.buyer.is_none() && c.kind == ContractKind::Locate))
        .map(|c| c.id)
        .collect();
    for id in direct {
        if corp_take(world, id) {
            continue;
        }
        let cands = direct_candidates(world, id);
        // Phase 2 (C26): a direct record's gang candidate is the buyer's gang.
        let gangs: Vec<EntityId> = world
            .contracts
            .get(&id)
            .and_then(|c| c.buyer)
            .and_then(|b| if world.has::<crate::components::Gang>(b) { Some(b) } else { world.gang_of(b) })
            .into_iter()
            .collect();
        offer(world, id, cands, gangs, &mut taken_today);
    }
}
