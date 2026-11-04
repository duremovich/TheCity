use super::*;

/// V29: `[data] steal_units[deck_eff − 1]` (0 past the table).
pub(super) fn steal_units(world: &World, deck_eff: u8) -> u32 {
    world.config.data.steal_units.get(usize::from(deck_eff.saturating_sub(1))).copied().unwrap_or(0)
}

/// V20: `min(ledger_frac × purse, ledger_cap[deck_eff − 1])`.
pub(super) fn ledger_take(world: &World, purse: i64, deck_eff: u8) -> i64 {
    let cap = world.config.data.ledger_cap.get(usize::from(deck_eff.saturating_sub(1))).copied().unwrap_or(0);
    ((world.config.data.ledger_frac * purse.max(0) as f32).floor() as i64).min(cap).max(0)
}

/// V29: what a runner could go after with a deck at `deck_eff`, and its
/// value in coins: Data on alive Lab and Hideout nodes with a store, not of
/// the agent's employer corp, its own gang, the patron or a corp it runs; a Ledger (a corp's
/// or the Treasury) with a tier-2 deck. Values carry the mode's take (V66).
pub fn targets(
    world: &World,
    agent: EntityId,
    patron: Option<EntityId>,
    deck_eff: u8,
    mode: RunMode,
) -> SmallVec<[(NodeId, Purpose, i64); 32]> {
    let mut out = SmallVec::new();
    let employer =
        world.comp::<crate::components::Job>(agent).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e));
    let gang = world.gang_of(agent);
    // Plan addition: nor the corp the agent runs as exec (no robbing oneself).
    let own = |o: EntityId| world.comp::<Corp>(o).is_some_and(|c| c.exec == Some(agent));
    let mine = |o: Option<EntityId>| o.is_some() && (o == employer || o == gang || o == patron || o.is_some_and(own));
    let (take, _, _) = mode_terms(world, mode);
    let price = world.config.data.data_price;
    let units = steal_units(world, deck_eff);
    let ledger_ok = deck_eff >= world.config.virt.trunk_links_tier;
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive && !mine(n.owner)) {
        let id = NodeId(i as u16);
        match n.kind {
            NodeKind::Building(b) => {
                let kind = world.comp::<Building>(b).map(|bd| bd.kind);
                if !matches!(kind, Some(BuildingKind::Lab | BuildingKind::Hideout)) || n.store.total() == 0 {
                    continue;
                }
                let got = (n.store.total().min(units) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((id, Purpose::Data { wipe: false }, got * price));
                }
            }
            // M17 phase 4: an uplink's Data (its routing needs a tier-3 deck),
            // not for its own branches' people or patrons.
            NodeKind::Uplink(p) if n.store.total() > 0 => {
                let own =
                    |o: Option<EntityId>| o.and_then(|x| world.comp::<Corp>(x)).is_some_and(|c| c.parent == Some(p));
                if own(employer) || own(patron) {
                    continue;
                }
                let got = (n.store.total().min(units) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((id, Purpose::Data { wipe: false }, got * price));
                }
            }
            NodeKind::Ledger(c) if ledger_ok => {
                let purse = if world.has::<Corp>(c) { world.purse(Some(c)) } else { world.purse(None) };
                let got = (ledger_take(world, purse, deck_eff) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((id, Purpose::Ledger, got));
                }
            }
            _ => {}
        }
    }
    out
}

/// V29 (one `routes_from` per scorer): the best target by `EV × p_success`
/// (ties the lower `NodeId`) among those at `p_success ≥ min_route_p`.
pub fn best_target(
    world: &World,
    agent: EntityId,
    patron: Option<EntityId>,
) -> Option<(Portal, NodeId, Purpose, i64, Route)> {
    best_target_where(world, agent, patron, false)
}

/// M16a (plan C34): [`best_target`] among the Data targets only (a Fixer
/// brokers the city's Data demand, spec § 2).
pub fn best_data_target(world: &World, agent: EntityId) -> Option<(Portal, NodeId, Purpose, i64, Route)> {
    best_target_where(world, agent, None, true)
}

fn best_target_where(
    world: &World,
    agent: EntityId,
    patron: Option<EntityId>,
    data_only: bool,
) -> Option<(Portal, NodeId, Purpose, i64, Route)> {
    let deck_eff = world.comp::<Kit>(agent).filter(|k| k.deck.is_some()).map(|k| k.deck_tier)?;
    if deck_eff == 0 {
        return None;
    }
    let chair = chair_for(world, agent, patron)?;
    let mode = mode_of(world, agent);
    let o = odds_of(world, agent, deck_eff, patron, mode);
    let mut cands = targets(world, agent, patron, deck_eff, mode);
    if data_only {
        cands.retain(|c| matches!(c.1, Purpose::Data { .. }));
    }
    let (_, n, purpose, ev, _) = rank(world, chair.node, &o, deck_eff, &cands).into_iter().next()?;
    // The same tree `rank` searched (a cache hit): the route for the offer.
    let tree = routes_from(world, chair.node, deck_eff, o.att, patron);
    let route = route_to(world, &tree, chair.node, n, &o)?;
    Some((chair, n, purpose, ev, route))
}

/// The Hack goal's offer (V29): a run, or the sale of the Data a deck holds.
#[derive(Clone, Debug)]
pub struct HackOffer {
    /// `GoTo(DataBuyer) → SellData` (the Lab in `lab`).
    pub sell: bool,
    pub lab: Option<EntityId>,
    pub portal: Option<Portal>,
    pub target: NodeId,
    pub purpose: Purpose,
    pub ev: i64,
    pub route: Option<Route>,
    pub mode: RunMode,
    pub considerations: Vec<crate::utility::Consideration>,
}

/// The Data on the agent's deck (`Kit.deck`'s `Asset.data`), all tracks.
pub fn deck_data(world: &World, agent: EntityId) -> u32 {
    world
        .comp::<Kit>(agent)
        .and_then(|k| k.deck)
        .and_then(|d| world.comp::<Asset>(d))
        .map_or(0, |a| a.data.iter().sum())
}

/// Is the Hack goal cooled for `agent` (a run's or a fry's cooldown, or a
/// failed plan)?
pub fn hack_cooled(world: &World, agent: EntityId) -> bool {
    world.comp::<Brain>(agent).and_then(|b| b.cooldowns.get(&GoalKind::Hack)).is_some_and(|&until| until > world.tick)
}

/// V29: the Hack goal's gate and considerations, for a Full or Coarse adult
/// with a deck, no Sentence: the sale of its deck's Data first (no
/// cooldown: the run already happened); else, off cooldown, at `hacking ≥
/// hack_min`, with a chair and a target at `p_success ≥ min_route_p`, the
/// best run. `None` with the plane off.
pub fn hack_choice(world: &World, id: EntityId) -> Option<HackOffer> {
    hack_offer(world, id, false)
}

/// V11 (phase 3): a gang, corp, prelude, overwatch or Stat order due within
/// the hour (`now ≥ not_before − 60`), not yet run: the agent's one order.
pub fn standing_order(world: &World, id: EntityId) -> Option<&RunOrder> {
    world
        .run_orders
        .get(&id)
        .filter(|o| o.why != RunWhy::Freelance)
        .filter(|o| world.tick + lead_of(o.why) >= o.not_before && o.expires > world.tick)
        .filter(|_| !world.runner_of.contains_key(&id))
}

/// M14 review: the agent holds a gang, corp, prelude, overwatch, Stat or
/// god order that has not expired (due or not). Such an agent does not
/// freelance: a freelance `RunOrder` would replace the pending one.
fn pending_order(world: &World, id: EntityId) -> bool {
    world.run_orders.get(&id).is_some_and(|o| o.why != RunWhy::Freelance && o.expires > world.tick)
}

/// How long before `not_before` an order is taken up: an hour (V11), the
/// Raid prelude four (plan deviation: a runner living across the map walked
/// to the Hideout chair after the muster had left).
fn lead_of(why: RunWhy) -> Tick {
    if why == RunWhy::Prelude {
        4 * TICKS_PER_HOUR
    } else {
        TICKS_PER_HOUR
    }
}

/// [`hack_choice`]; `stat` lets the Statistical pass (V38) score an agent
/// off screen. An agent holding a standing order (V11) scores the single
/// `order_score` instead of the freelance considerations.
pub fn hack_offer(world: &World, id: EntityId, stat: bool) -> Option<HackOffer> {
    use crate::utility::curves::{can, urgency, Curve, GATE};
    use crate::utility::Consideration;
    let kit = world.comp::<Kit>(id)?;
    let deck = kit.deck?;
    let brain = world.comp::<Brain>(id)?;
    if (brain.lod == crate::components::Lod::Statistical && !stat)
        || world.has::<crate::components::Sentence>(id)
        || !crate::systems::demography::is_adult(world, id)
    {
        return None;
    }
    if let Some(o) = standing_order(world, id) {
        let considerations = vec![
            Consideration::new("ordered", can(true), GATE),
            Consideration::new("order", 1.0, Curve::Linear { m: world.config.hack.order_score, b: 0.0 }),
        ];
        let building = o.chair;
        return Some(HackOffer {
            sell: false,
            lab: None,
            portal: Some(Portal { building, node: portal(world, building, id, o.patron) }),
            target: o.target,
            purpose: o.purpose,
            ev: 0,
            route: None,
            mode: o.mode,
            considerations,
        });
    }
    let wealth = world.comp::<crate::components::Needs>(id).map_or(0.5, |n| urgency(n.wealth));
    let held = world.comp::<Asset>(deck).map_or(0, |a| a.data.iter().sum::<u32>());
    // With no buyer able to pay, the Data waits on the deck and the runner may run again.
    let buyer = if held > 0 { crate::systems::tech::data_buyer_lab(world, id) } else { None };
    if let Some(lab) = buyer {
        let value = i64::from(held) * world.config.data.data_price;
        let x = (value as f32 / world.config.hack.hack_ref.max(1) as f32).min(1.0);
        let considerations = vec![
            Consideration::new("can sell", can(true), GATE),
            Consideration::new("U(value)", x, Curve::Linear { m: 0.5, b: 0.5 }),
            Consideration::new("U(wealth)", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ];
        return Some(HackOffer {
            sell: true,
            lab: Some(lab),
            portal: None,
            target: NodeId::default(),
            purpose: Purpose::Data { wipe: false },
            ev: value,
            route: None,
            mode: RunMode::Quiet,
            considerations,
        });
    }
    if hack_cooled(world, id) || world.runner_of.contains_key(&id) || pending_order(world, id) {
        return None;
    }
    let hacking = world.comp::<Skills>(id).map_or(0.0, |s| s.hacking);
    if hacking < world.config.hack.hack_min {
        return None;
    }
    let (portal, target, purpose, ev, route) = best_target(world, id, None)?;
    // L2 shadow fixes item 18: the gap guard: no freelance run where the
    // target's ICE beats the attack by `[ice] flatline_gap` unless the
    // runner is desperate (a T1 deck, courage 0.18, jacked into a hardened
    // Lab and flatlined in 24 min).
    if wealth < world.config.life.hack_gap_wealth {
        let o = odds_of(world, id, kit.deck_tier, None, mode_of(world, id));
        if f32::from(def(world, target)) - o.att >= world.config.ice.flatline_gap {
            return None;
        }
    }
    let p = world.comp::<Personality>(id);
    let (law, greed, courage) = p.map_or((0.5, 0.5, 0.5), |p| (p.lawfulness, p.greed, p.courage));
    let x = (ev as f32 / world.config.hack.hack_ref.max(1) as f32).min(1.0);
    let considerations = vec![
        Consideration::new("can hack", can(true), GATE),
        Consideration::new("U(EV)", x, Curve::Logistic { k: 8.0, mid: 0.3 }),
        Consideration::new("1-lawfulness", 1.0 - law, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("greed", greed, Curve::Linear { m: 0.5, b: 0.5 }),
        // Plan deviation (as M12's Squat): `U(wealth)`, not the spec's
        // `1 − U(wealth)`, so the poor run more, not less.
        Consideration::new("U(wealth)", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new(
            "1-p_fry x (1-courage)",
            1.0 - route.p_fry * (1.0 - courage),
            Curve::Linear { m: 0.6, b: 0.4 },
        ),
    ];
    Some(HackOffer {
        sell: false,
        lab: None,
        portal: Some(portal),
        target,
        purpose,
        ev,
        route: Some(route),
        mode: mode_of(world, id),
        considerations,
    })
}

/// V29: the Hack goal's plan, built directly (`plan.rs` calls it at bind):
/// `[GoTo(DataBuyer)] → SellData` for a deck holding Data; else the
/// freelance `RunOrder` is written and the plan is `[GoTo(Chair)] → JackIn`.
pub fn hack_plan(world: &mut World, id: EntityId) -> Option<crate::goap::Plan> {
    use crate::components::ActionInstance;
    use crate::goap::{ActionKind, LocationKey};
    let offer = hack_choice(world, id)?;
    let tick = world.tick;
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let step = |action: ActionKind, target: Option<EntityId>| ActionInstance { action, target, tile: None };
    if offer.sell {
        let lab = offer.lab?;
        let mut steps = Vec::new();
        if here != Some(lab) {
            steps.push(step(ActionKind::GoTo(LocationKey::DataBuyer), Some(lab)));
        }
        steps.push(step(ActionKind::SellData, Some(lab)));
        return Some(crate::goap::Plan { goal: GoalKind::Hack, target: Some(lab), steps, started_tick: tick });
    }
    let portal = offer.portal?;
    // Phase 3: a standing order keeps its own `RunOrder`; M14 review: so
    // does one not yet due (`standing_order` reads only due orders). Only a
    // stale freelance order, or none, is replaced.
    if world.run_orders.get(&id).is_none_or(|o| o.why == RunWhy::Freelance || o.expires <= tick) {
        world.run_orders.insert(
            id,
            RunOrder {
                patron: None,
                purpose: offer.purpose,
                target: offer.target,
                chair: portal.building,
                not_before: tick,
                expires: tick + TICKS_PER_DAY,
                why: RunWhy::Freelance,
                mode: offer.mode,
            },
        );
    }
    let mut steps = Vec::new();
    if here != Some(portal.building) {
        steps.push(step(ActionKind::GoTo(LocationKey::Chair), Some(portal.building)));
    }
    steps.push(step(ActionKind::JackIn, Some(portal.building)));
    Some(crate::goap::Plan { goal: GoalKind::Hack, target: Some(portal.building), steps, started_tick: tick })
}

/// V5: `JackIn`'s start at a public terminal: `terminal_fee` to the Bar's
/// or Hotel's owner (`Flow::Terminal`, taxed).
pub fn pay_terminal(world: &mut World, runner: EntityId) {
    let Some(chair) = world.run_orders.get(&runner).map(|o| o.chair) else { return };
    if !is_terminal(world, chair) || world.comp::<Position>(runner).and_then(|p| p.building) != Some(chair) {
        return;
    }
    let owner = world.owner_of(chair);
    if owner == Some(runner) {
        return;
    }
    let fee = world.config.virt.terminal_fee;
    let paid = ownership::pay(world, Some(runner), owner, fee, Flow::Terminal);
    ownership::credit(world, chair, paid);
}

/// `hop_ticks[deck_eff − 1]`.
pub(super) fn hop_ticks(world: &World, deck_eff: u8) -> Tick {
    let h = &world.config.virt.hop_ticks;
    Tick::from(h.get(usize::from(deck_eff.saturating_sub(1))).or(h.last()).copied().unwrap_or(8))
}

pub(super) fn contest_ticks(world: &World) -> Tick {
    Tick::from(world.config.virt.contest_ticks)
}

pub(super) fn act_ticks(world: &World, purpose: Purpose) -> Tick {
    let a = &world.config.ice.act_ticks;
    Tick::from(match purpose {
        Purpose::Data { .. } | Purpose::Overwatch(_) => a.data,
        Purpose::Ledger => a.ledger,
        Purpose::Door => a.door,
        Purpose::Robot(_) => a.robot,
        Purpose::Camera(_) => a.camera,
    })
}

/// V10: `(start_tick << 22) | runner.index`.
pub fn run_id(tick: Tick, runner: EntityId) -> RunId {
    debug_assert!(runner.index < (1 << 22), "a runner index fits 22 bits");
    debug_assert!(tick < (1 << 28), "a RunId fits 50 bits");
    (tick << 22) | u64::from(runner.index)
}
