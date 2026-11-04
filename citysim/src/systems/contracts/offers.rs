use super::*;

// --- Fixer-fed run orders (C34) ---

/// C34, in `match_day` after a Fixer's book: at most `run_offers_per_day`
/// run orders to its regulars (ascending id) with hacking ≥ `gun_skill_min`,
/// a deck in the Kit (plan deviation: `hack_offer` needs `Kit.deck`), no
/// `RunOrder`, and a freelance offer from M14's scorer
/// (`virt::hack_offer(.., true)`, not a sale) and the scorer's best Data
/// target (`virt::best_data_target`; deviation: the plan takes the
/// scorer's purpose, the spec says `Purpose::Data`, and a Ledger run has no
/// `SellData` for the cut): `RunOrder { patron: None, .., why:
/// RunWhy::Fixer }` through `virt::give_order`;
/// `World::fixer_runs[runner] = (fixer, now)`.
pub fn run_offers(world: &mut World, fixer: EntityId) {
    let n = usize::from(world.config.fixers.run_offers_per_day);
    if n == 0 {
        return;
    }
    let Some(k) = world.comp::<Broker>(fixer) else { return };
    let regulars: Vec<EntityId> = k.regulars.keys().copied().collect();
    let min = world.config.fixers.gun_skill_min;
    let now = crate::systems::midnight::stamp(world);
    let mut given = 0;
    for r in regulars {
        if given >= n {
            break;
        }
        if world.run_orders.contains_key(&r) || world.fixer_runs.contains_key(&r) {
            continue;
        }
        if !world.comp::<crate::components::Kit>(r).is_some_and(|k| k.deck.is_some()) {
            continue;
        }
        if world.comp::<Skills>(r).is_none_or(|s| s.hacking < min) {
            continue;
        }
        // M14's scorer gates the runner (cooldown, a run on, the skill, the
        // gap guard); spec § 2: the run is a Data run (the Fixer brokers the
        // city's Data demand), the scorer's best Data target.
        let Some(offer) = crate::systems::virt::hack_offer(world, r, true).filter(|o| !o.sell) else { continue };
        let Some((portal, target, purpose, _, _)) = crate::systems::virt::best_data_target(world, r) else { continue };
        let order = crate::virt::RunOrder {
            patron: None,
            purpose,
            target,
            chair: portal.building,
            not_before: now,
            expires: now + TICKS_PER_DAY,
            why: crate::virt::RunWhy::Fixer,
            mode: offer.mode,
        };
        crate::systems::virt::give_order(world, r, order);
        world.fixer_runs.insert(r, (fixer, now));
        let text = format!("{} handed {} a run order", world.name_of(fixer), world.name_of(r));
        log(world, 0, text);
        given += 1;
    }
}

/// C34, from the `SellData` completion: a runner who sold within 3 days
/// of a Fixer's order pays the owner `fixer_cut` of `sale` (`FixerCut`,
/// taxed; the Fixer credited), and stays a regular from now.
pub fn on_sell_data(world: &mut World, runner: EntityId, sale: i64) {
    if world.fixer_runs.is_empty() {
        return;
    }
    let Some(&(f, offered)) = world.fixer_runs.get(&runner) else { return };
    let now = world.tick;
    if now > offered + 3 * TICKS_PER_DAY {
        world.fixer_runs.remove(&runner);
        return;
    }
    if sale <= 0 {
        return;
    }
    world.fixer_runs.remove(&runner);
    let owner = world.owner_of(f);
    let cut = ((sale as f32) * c_cut(world, f)).round() as i64;
    let paid = ownership::pay(world, Some(runner), owner, cut, Flow::FixerCut);
    note_cut(world, f, paid);
    if let Some(k) = world.comp_mut::<Broker>(f) {
        k.regulars.insert(runner, now);
    }
    world.stats.current.contract.fixer_runs += 1;
    let text = format!("{} paid {} a cut of {paid} on a run", world.name_of(runner), world.name_of(f));
    log(world, 0, text);
}

/// C34, at midnight: Fixer orders older than 3 days lapse.
fn prune_fixer_runs(world: &mut World) {
    let now = world.tick;
    world.fixer_runs.retain(|_, &mut (_, t)| now <= t + 3 * TICKS_PER_DAY);
}

/// Phase 3's midnight steps (C30, C31, C33, C34), from `daily`.
pub(super) fn law_and_street_daily(world: &mut World) {
    // Round 6 V3: `heat_daily` runs at the top of `daily` (before the posts).
    corruption_daily(world);
    post_vendetta_locates(world);
    prune_fixer_runs(world);
}

// ---------------------------------------------------------------------------
// The day's snapshot (C38)
// ---------------------------------------------------------------------------

/// Review fix (probe): coins still held in escrow by records that are
/// neither Open nor Taken (M16b's Standing counts as live then); `settle`
/// empties every closing record, so this must read 0. `escrow_leak` cannot
/// see it (`escrow_held` moves with every record's escrow).
pub fn escrow_stuck(world: &World) -> i64 {
    world.contracts.values().filter(|c| !c.is_live()).map(|c| c.escrow).sum()
}

/// C38's snapshot columns, from `stats::snapshot`.
pub fn snapshot(world: &mut World) {
    let open = world.contracts.values().filter(|c| c.is_live()).count() as u32;
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    let regulars: u32 = fixers.iter().filter_map(|&f| world.comp::<Broker>(f)).map(|k| k.regulars.len() as u32).sum();
    let mut parties: std::collections::BTreeSet<EntityId> = world.contract_runs.keys().copied().collect();
    parties.extend(world.contract_runs.values().filter_map(|r| world.has::<Identity>(r.target).then_some(r.target)));
    // Phase 2 (2.5): the missions' crews and targets.
    for (id, m) in &world.missions {
        parties.extend(m.crew.iter().copied());
        parties.extend(world.contracts.get(id).and_then(|c| c.target_agent()));
    }
    let queued = world
        .contracts
        .values()
        .filter(|c| c.status == ContractStatus::Taken && c.render == Render::Live)
        .filter(|c| !running(world, c))
        .count() as u32;
    let escrow: i64 = world.contracts.values().map(|c| c.escrow).sum();
    let held = world.escrow_held;
    let stuck = escrow_stuck(world);
    let mut heat = [0.0f32; crate::contract::FIXER_SLOTS];
    let mut income = [0i64; crate::contract::FIXER_SLOTS];
    for (i, &f) in fixers.iter().take(crate::contract::FIXER_SLOTS).enumerate() {
        if let Some(k) = world.comp::<Broker>(f) {
            heat[i] = k.heat;
            income[i] = k.income_today;
        }
    }
    let on_take = guards_on_take(world) as u32;
    let row = &mut world.stats.current.contract;
    row.contracts_open = open;
    row.regulars = regulars;
    row.live_parties = parties.len() as u32;
    row.live_queued = queued;
    row.escrow_held = held;
    row.escrow_leak = escrow - held;
    row.escrow_stuck = stuck;
    row.guards_on_take = on_take;
    row.f_heat = heat;
    row.f_income = income;
}

// ---------------------------------------------------------------------------
// God commands (C39)
// ---------------------------------------------------------------------------

/// God `PostContract`: a record by `buyer` (`None` the city), the price
/// escrowed (brokered with the open Fixer nearest the target) or checked
/// (direct).
pub fn god_post(
    world: &mut World,
    buyer: Option<EntityId>,
    kind: ContractKind,
    target: Target,
    price: i64,
    brokered: bool,
    deadline_days: u16,
) -> Result<ContractId, String> {
    let broker = if brokered {
        let from = match target {
            Target::Agent(a) => world.comp::<Position>(a).map(|p| p.tile),
            Target::Building(b) => world.comp::<Building>(b).map(|bd| bd.door),
            Target::Carry { what, .. } => world.comp::<Position>(what.id()).map(|p| p.tile),
            Target::Story { actor, .. } => world.comp::<Position>(actor).map(|p| p.tile),
        }
        .ok_or("no such target")?;
        Some(nearest_fixer_to(world, from).ok_or("no open Fixer")?)
    } else {
        None
    };
    let agent = buyer.filter(|&b| world.has::<Identity>(b));
    let posting = Posting {
        buyer,
        agent,
        kind,
        target,
        broker,
        deadline_days: if deadline_days == 0 { world.config.contracts.deadline_days.get(kind) } else { deadline_days },
        origin: Origin::God,
        price: (price > 0).then_some(price),
    };
    post(world, posting).map_err(|r| format!("refused: {r:?}"))
}

/// God `TakeContract` (and M18's player call): `taker` takes the open
/// record now, past the matching (a living agent; gang and corp takers are
/// phase 2).
pub fn god_take(world: &mut World, id: ContractId, taker: EntityId) -> Result<(), String> {
    let c = world.contracts.get(&id).ok_or("no such contract")?;
    if !c.is_open() {
        return Err("the contract is not open".into());
    }
    // Phase 2 (C26): a gang takes it as a job.
    if world.has::<crate::components::Gang>(taker) {
        let c = c.clone();
        if !gang_eligible(world, &c, taker) {
            return Err("the gang cannot take it".into());
        }
        return if accept_gang(world, id, taker) { Ok(()) } else { Err("not accepted".into()) };
    }
    if !free_adult(world, taker) || c.target.id() == taker {
        return Err("the taker cannot take it".into());
    }
    if holds_taken(world, taker) {
        return Err("the taker holds a contract".into());
    }
    if accept(world, id, taker, &[]) {
        Ok(())
    } else {
        Err("not accepted".into())
    }
}
