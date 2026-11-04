use super::*;

// ---------------------------------------------------------------------------
// M18 P3: talk
// ---------------------------------------------------------------------------

/// `id` and `o` near enough to talk: one building, or the street within
/// `[talk] street_tiles` (`goap::actions::street_beside`).
pub fn beside(world: &World, id: EntityId, o: EntityId) -> bool {
    let (a, b) = (here(world, id), here(world, o));
    (a.is_some() && a == b) || crate::goap::actions::street_beside(world, id, o)
}

/// `id` asked `o` something within `[talk] again_hours`: `o` remembers a
/// move of his (Persuaded, Threatened), or `id` was turned down by `o`.
fn asked_lately(world: &World, id: EntityId, o: EntityId) -> bool {
    let span = world.config.talk.again_hours * crate::time::TICKS_PER_HOUR;
    let recent = |who: EntityId, by: EntityId, kinds: &[MemoryKind]| {
        world.comp::<crate::components::Memory>(who).is_some_and(|m| {
            m.entries
                .iter()
                .any(|e| e.subject == Some(by) && kinds.contains(&e.kind) && world.tick.saturating_sub(e.tick) < span)
        })
    };
    recent(o, id, &[MemoryKind::Persuaded, MemoryKind::Threatened]) || recent(id, o, &[MemoryKind::Rejected])
}

/// A talk move's circumstance: the body can take an order, `o` is an adult
/// beside him, not asked lately.
pub(super) fn talk_ready(world: &World, id: EntityId, o: EntityId) -> Result<(), String> {
    if let Some(why) = refusal(world, id) {
        return Err(why);
    }
    if !valid_person(world, id, o) {
        return Err("nobody to talk to".into());
    }
    if !beside(world, id, o) {
        return Err("not beside them (walk over first)".into());
    }
    if asked_lately(world, id, o) {
        return Err("you asked them lately".into());
    }
    Ok(())
}

/// A talk verb's move and its line (`Err`: no move to make: nothing to lend,
/// nothing on them, no post at their work).
pub(super) fn talk_move(world: &World, id: EntityId, verb: Verb) -> Result<(crate::word::SocialMove, String), String> {
    use crate::word::{MoveKind, SocialMove, Stake};
    let t = &world.config.talk;
    let coins = |e: EntityId| world.comp::<Wallet>(e).map_or(0, |w| w.coins);
    let mv = |target, kind, stake| SocialMove { actor: id, target, kind, stake };
    match verb {
        Verb::Befriend(o) => Ok((mv(o, MoveKind::Charm, Stake::Coins(0)), format!("Befriend {}", world.name_of(o)))),
        Verb::Loan(o) => {
            let n = t.loan_coins.min(coins(o) - t.loan_keep);
            if n <= 0 {
                return Err("they have nothing to spare".into());
            }
            Ok((mv(o, MoveKind::Persuade, Stake::Coins(n)), format!("Ask {} for a loan of {n}c", world.name_of(o))))
        }
        Verb::Threaten(o) => {
            let n = t.threat_coins.min(coins(o));
            if n <= 0 {
                return Err("nothing on them".into());
            }
            Ok((mv(o, MoveKind::Intimidate, Stake::Coins(n)), format!("Lean on {} for {n}c", world.name_of(o))))
        }
        Verb::AskWorkOf(o) => {
            let Some(e) = world.comp::<Job>(o).and_then(|j| j.employer) else {
                return Err("they have no post".into());
            };
            let Some(role) = crate::systems::search::open_post(world, id, e) else {
                return Err(format!("no post you could take at {}", world.name_of(e)));
            };
            let wage = world.config.wage(role);
            Ok((
                mv(o, MoveKind::Charm, Stake::Job { building: e, wage }),
                format!("Ask {} to put in a word at {} ({})", world.name_of(o), world.name_of(e), role.label()),
            ))
        }
        _ => Err("not a talk move".into()),
    }
}

/// A Flirt's acceptance as exec rolls it (`exec::actions` Flirt).
pub(super) fn court_p(world: &World, id: EntityId, o: EntityId) -> f32 {
    let aff = world.edge(id, o).map_or(0.0, |e| e.affinity);
    let soc = world.comp::<crate::components::Personality>(o).map_or(0.5, |p| p.sociability);
    (0.3 + aff + 0.2 * soc).clamp(0.0, 1.0)
}

/// A refused move: the pair cools and `id` remembers it.
fn turned_down(world: &mut World, id: EntityId, o: EntityId) {
    let d = world.config.talk.refused_affinity;
    crate::systems::social::adjust(world, id, o, d, 0.0);
    world.remember(id, MemoryKind::Rejected, Some(o), 0.4, -0.3, false);
}

/// The talk move made now, one roll (`moves::resolve`), its consequences;
/// the line for the log (`Err`: refused before the roll).
pub(super) fn talk(world: &mut World, id: EntityId, verb: Verb) -> Result<String, String> {
    let (m, what) = talk_move(world, id, verb)?;
    let o = m.target;
    talk_ready(world, id, o)?;
    let out = crate::systems::moves::resolve(world, &m);
    let (me, them) = (world.name_of(id), world.name_of(o));
    let pct = format!("{:.0}%", 100.0 * out.p);
    if out.refused {
        turned_down(world, id, o);
        return Ok(format!("{what}: {them} would not hear {me} out ({pct})"));
    }
    if !out.success {
        turned_down(world, id, o);
        let back = if out.backlash { format!("; {them} squares up to {me}") } else { String::new() };
        return Ok(format!("{what}: no ({pct}){back}"));
    }
    let t = world.config.talk.clone();
    let line = match (verb, m.stake) {
        (Verb::Befriend(_), _) => {
            crate::systems::social::adjust(world, id, o, t.befriend_affinity, t.befriend_trust);
            world.remember(id, MemoryKind::Socialised, Some(o), 0.3, 0.3, false);
            let aff = world.edge(id, o).map_or(0.0, |e| e.affinity);
            format!("{them} warms to {me} (affinity {aff:+.2})")
        }
        (Verb::Loan(_), crate::word::Stake::Coins(n)) => {
            let n = n.min(world.comp::<Wallet>(o).map_or(0, |w| w.coins));
            if let Some(w) = world.comp_mut::<Wallet>(o) {
                w.coins -= n;
            }
            if let Some(w) = world.comp_mut::<Wallet>(id) {
                w.coins += n;
            }
            let (lo, _) = crate::components::edge_key(id, o);
            let tick = world.tick;
            let e = world.edge_entry(id, o);
            e.debt += if lo == id { n as i32 } else { -(n as i32) };
            e.debt_since.get_or_insert(tick);
            e.last_interaction = tick;
            world.remember(o, MemoryKind::Paid, Some(id), 0.5, 0.0, false);
            world.remember(id, MemoryKind::Paid, Some(o), 0.5, 0.3, false);
            format!("{them} lent {me} {n}c (a debt)")
        }
        (Verb::Threaten(_), crate::word::Stake::Coins(n)) => {
            let n = n.min(world.comp::<Wallet>(o).map_or(0, |w| w.coins));
            if let Some(w) = world.comp_mut::<Wallet>(o) {
                w.coins -= n;
            }
            if let Some(w) = world.comp_mut::<Wallet>(id) {
                w.coins += n;
            }
            crate::systems::social::robbed_by(world, o, id);
            world.remember(o, MemoryKind::Threatened, Some(id), 0.6, -0.6, false);
            let d = crate::systems::gossip::talk_district(world, o);
            crate::systems::gossip::post_deed(world, d, crate::word::Deed::Extorted, Some(id), Some(o));
            format!("{them} handed {me} {n}c, afraid")
        }
        (Verb::AskWorkOf(_), crate::word::Stake::Job { building: e, .. }) => {
            crate::systems::search::tell_lead(world, id, e, o);
            crate::systems::social::adjust(world, id, o, 0.0, 0.02);
            // the ask at the door, walked to now (P2's AskWork row)
            let at = world.name_of(e);
            match plan_of(world, id, Verb::AskWork(e)) {
                Ok((plan, label)) => {
                    install_directed(world, id, plan, label);
                    format!("{them} will put in a word for {me} at {at}: off to ask")
                }
                Err(why) => format!("{them} will put in a word for {me} at {at} ({why})"),
            }
        }
        _ => what.clone(),
    };
    Ok(format!("{what} ({pct}): {line}"))
}

// ---------------------------------------------------------------------------
// M18 P3: the board and the city's offers
// ---------------------------------------------------------------------------

/// A record as one line: the kind, the mark, the price, where, the deadline.
pub(super) fn job_line(world: &World, c: &crate::contract::Contract) -> String {
    let from = match (c.broker, c.buyer) {
        (Some(f), _) => format!("at {}", world.name_of(f)),
        (None, Some(b)) => format!("from {}", world.owner_label(Some(b))),
        (None, None) => "a public bounty".to_string(),
    };
    let per = if c.kind == crate::contract::ContractKind::Locate {
        let left = if c.brokered() { format!(", {}c left", c.escrow) } else { String::new() };
        format!(", {}c a sighting{left}", world.config.bounty.per_sighting)
    } else {
        String::new()
    };
    let away = crate::systems::contracts::mark_note(world, c).map_or(String::new(), |w| format!(" ({w})"));
    format!(
        "{} on {}: {}c{per}, {from}, by d{} {}{away}",
        c.kind.label(),
        world.name_of(c.target.id()),
        c.price,
        c.deadline / crate::time::TICKS_PER_DAY,
        crate::time::clock(c.deadline)
    )
}

/// The record `id` holds taken (taker or crew), if any.
pub fn held_job(world: &World, id: EntityId) -> Option<u64> {
    if let Some(r) = world.contract_runs.get(&id) {
        return Some(r.contract);
    }
    world.by_party.get(&id)?.iter().copied().find(|c| {
        world.contracts.get(c).is_some_and(|x| {
            x.status == crate::contract::ContractStatus::Taken && (x.taker == Some(id) || x.crew.contains(&id))
        })
    })
}

/// The board: the record you hold, the open records you see
/// (`contracts::visible`: a Fixer's regulars, the buyer's people, the
/// public bounties; the ones you cannot take greyed with why), and the
/// open Fixers nearest you where you are no regular (get known there).
pub fn menu_board(world: &World, id: EntityId) -> Menu {
    use crate::contract::ContractStatus;
    let mut verbs: Vec<Verb> = Vec::new();
    if held_job(world, id).is_some() {
        verbs.push(Verb::WorkJob);
    }
    let mut open: Vec<(bool, i64, u64)> = crate::systems::contracts::visible(world, id)
        .into_iter()
        .filter_map(|c| world.contracts.get(&c).filter(|x| x.status == ContractStatus::Open).map(|x| (c, x.price)))
        .map(|(c, price)| (crate::systems::contracts::player_take_refusal(world, id, c).is_some(), -price, c))
        .collect();
    open.sort();
    verbs
        .extend(open.into_iter().take(usize::from(world.config.fixers.board_size).max(8)).map(|(.., c)| Verb::Take(c)));
    if let Some(at) = world.comp::<Position>(id).map(|p| p.tile) {
        let mut fixers: Vec<(u32, EntityId)> = world
            .buildings_of_kind(BuildingKind::Fixer)
            .iter()
            .copied()
            .filter(|&f| crate::systems::contracts::fixer_open(world, f))
            .filter(|&f| world.comp::<crate::contract::Broker>(f).is_some_and(|k| !k.regulars.contains_key(&id)))
            .filter_map(|f| world.comp::<Building>(f).map(|b| (b.door.manhattan(at), f)))
            .collect();
        fixers.sort();
        verbs.extend(fixers.into_iter().take(4).map(|(_, f)| Verb::Network(f)));
    }
    Menu { title: "the board".into(), rows: rows_with(world, id, verbs, |_| true) }
}

/// The city's offers standing for `id` (two rows each: take, turn down).
pub fn menu_offers(world: &World, id: EntityId) -> Menu {
    let offers = world
        .comp::<Brain>(id)
        .and_then(|b| b.player.as_ref())
        .map(|p| p.offers.iter().filter(|o| o.until > world.tick).map(|o| o.made).collect::<Vec<_>>())
        .unwrap_or_default();
    let verbs = offers.into_iter().flat_map(|m| {
        let demand = offer_of(world, id, m).is_some_and(|o| matches!(o.what, OfferKind::Blackmail { .. }));
        [Some(Verb::Answer(m, true)), Some(Verb::Answer(m, false)), demand.then_some(Verb::Resist(m))]
            .into_iter()
            .flatten()
    });
    Menu { title: "offers".into(), rows: rows_with(world, id, verbs, |_| false) }
}

pub(super) fn offer_of(world: &World, id: EntityId, made: u64) -> Option<CityOffer> {
    let p = world.comp::<Brain>(id)?.player.as_ref()?;
    p.offers.iter().find(|o| o.made == made && o.until > world.tick).cloned()
}

/// The city offers `id` (a body the human drives) what it would write an
/// NPC (`care::covers`, `underwater`'s draw); it stands until `until`.
pub fn offer(world: &mut World, id: EntityId, what: OfferKind, until: crate::time::Tick, line: String) {
    let tick = world.tick;
    let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) else { return };
    p.offers.retain(|o| o.until > tick);
    // two offers in one tick keep apart (the key is `made`)
    let made = p.offers.iter().map(|o| o.made + 1).max().unwrap_or(tick).max(tick);
    p.offers.push(CityOffer { made, until, what, line });
}

/// M18 P4 (exec, each tick for a body with an offer out): the offers past
/// their tick lapse, each with a line (P3 let them go silently).
pub fn lapse(world: &mut World, id: EntityId) {
    let tick = world.tick;
    let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) else { return };
    let gone: Vec<CityOffer> = p.offers.iter().filter(|o| o.until <= tick).cloned().collect();
    p.offers.retain(|o| o.until > tick);
    let name = world.name_of(id);
    for o in gone {
        world.push_event(EventKind::PlayerAction, &[id], format!("{name}: the offer lapsed: {}", o.line));
        // M18 P5: a demand let lapse is refused (he goes to the law)
        if let OfferKind::Blackmail { witness, demand } = o.what {
            let _ = crate::systems::consequence::blackmail_answer(world, witness, id, demand, 1);
        }
    }
}

/// M18 P4 (exec, at a plan's end or abort): a body the human drives with no
/// plan left after its shift or job had their turn is doing nothing: the
/// HUD's "doing" goes (it read the failed row for hours, P3).
pub fn plan_ended(world: &mut World, id: EntityId) {
    if let Some(b) = world.comp_mut::<Brain>(id).filter(|b| b.plan.is_none()) {
        if let Some(p) = b.player.as_mut() {
            p.doing = None;
        }
    }
}

/// The human answers an offer: taken (the NPC's write runs), or turned down.
pub(super) fn answer(world: &mut World, id: EntityId, made: u64, take: bool) {
    let Some(o) = offer_of(world, id, made) else { return refuse(world, id, "Answer", "the offer lapsed") };
    if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
        p.offers.retain(|x| x.made != made);
    }
    let name = world.name_of(id);
    if !take {
        world.push_event(EventKind::PlayerAction, &[id], format!("{name} turned down: {}", o.line));
        // M18 P5: a demand refused: he goes to the law
        if let OfferKind::Blackmail { witness, demand } = o.what {
            let _ = crate::systems::consequence::blackmail_answer(world, witness, id, demand, 1);
        }
        return;
    }
    let done = match o.what {
        OfferKind::Cover { employer, sick, until, .. } => {
            crate::systems::care::take_cover(world, id, employer, sick, until)
        }
        OfferKind::Gang { gang, front } => crate::systems::underwater::take_front(world, id, gang, front),
        // M18 P5: a witness's demand paid
        OfferKind::Blackmail { witness, demand } => {
            crate::systems::consequence::blackmail_answer(world, witness, id, demand, 0)
        }
    };
    match done {
        Ok(()) => {
            world.push_event(EventKind::PlayerAction, &[id], format!("{name} took it: {}", o.line));
        }
        Err(why) => refuse(world, id, "Take the offer", &why),
    }
}

/// M18 P3 (`exec`'s plan end, a body the human drives, and the board's
/// rows): the record the human set the body to work goes on: the Contract
/// goal's next plan (`contracts::plan`: the walk to the mark's haunts, the
/// asking, the stakeout, the strike), until the record closes.
pub fn job_next(world: &mut World, id: EntityId) {
    use crate::contract::ContractStatus;
    let Some(b) = world.comp::<Brain>(id) else { return };
    if !b.possessed() || b.plan.is_some() {
        return;
    }
    let Some(c) = b.player.as_ref().and_then(|p| p.contract) else { return };
    let name = world.name_of(id);
    let rec = world.contracts.get(&c).cloned();
    let mine = rec
        .as_ref()
        .is_some_and(|x| x.status == ContractStatus::Taken && (x.taker == Some(id) || x.crew.contains(&id)));
    let end = |world: &mut World, text: String| {
        if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
            p.contract = None;
            p.doing = None;
        }
        if let Some(b) = world.comp_mut::<Brain>(id) {
            if b.current_goal == Some(GoalKind::Contract) {
                b.current_goal = None;
            }
        }
        world.push_event(EventKind::PlayerAction, &[id], text);
    };
    if !mine {
        let how = rec.map_or("gone".to_string(), |x| match x.status {
            ContractStatus::Open => "not taken".to_string(),
            s => s.label().to_string(),
        });
        return end(world, format!("{name}: the job is over ({how})"));
    }
    if let Some(why) = think::held(world, id) {
        return end(world, format!("{name}: the job waits ({why})"));
    }
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Contract);
        b.goal_since = tick;
    }
    match crate::systems::contracts::plan(world, id) {
        Some(plan) => {
            let line = steps_line(&plan.steps);
            plan::install(world, id, plan);
            world.push_event(EventKind::PlayerAction, &[id], format!("{name}: the job goes on [{line}]"));
        }
        None => end(world, format!("{name}: no way to work the job now (queued, or the mark out of reach)")),
    }
}

/// A directed plan from a row: the wheel taken, the plan installed and its
/// shared things held (`plan::reserve_for`), the row's label on the HUD.
pub(super) fn install_directed(world: &mut World, id: EntityId, plan: Plan, label: String) {
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(plan.goal);
        b.goal_since = tick;
        if let Some(p) = b.player.as_mut() {
            p.doing = Some(label.clone());
        }
    }
    let line = steps_line(&plan.steps);
    let held = Plan {
        steps: plan.steps.iter().filter(|s| s.action != ActionKind::CheckIn).cloned().collect(),
        ..plan.clone()
    };
    plan::install(world, id, plan);
    plan::reserve_for(world, id, &held);
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {label} [{line}]"));
}
