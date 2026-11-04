use super::*;

/// V10/V11: `JackIn` completes: the agent's `RunOrder` becomes a `Run`,
/// re-routed from the portal with the current ICE. A freelancer whose route
/// is gone or below `min_route_p` loses the step (`PreconditionLost`); an
/// ordered runner jacks in anyway. The run is queued, `runner_of` set, the
/// `JackedIn` event pushed.
pub fn start_run(world: &mut World, runner: EntityId) -> Result<RunId, crate::exec::FailReason> {
    use crate::exec::FailReason;
    if world.runner_of.contains_key(&runner) {
        return Err(FailReason::PreconditionLost);
    }
    let order = world.run_orders.get(&runner).cloned().ok_or(FailReason::PreconditionLost)?;
    let (deck, deck_eff) = world
        .comp::<Kit>(runner)
        .and_then(|k| k.deck.map(|d| (d, k.deck_tier)))
        .filter(|&(_, t)| t > 0)
        .ok_or(FailReason::PreconditionLost)?;
    let from = portal(world, order.chair, runner, order.patron);
    let o = odds_of(world, runner, deck_eff, order.patron, order.mode);
    // V37: an Overwatch run stays on its portal (no route, no contest) and
    // ends when the raid it streams resolves.
    let overwatch = matches!(order.purpose, Purpose::Overwatch(_));
    let route = if overwatch {
        Route { nodes: SmallVec::from_elem(from, 1), p_success: 1.0, p_fry: 0.0, p_trace: 0.0 }
    } else {
        let tree = routes_from(world, from, deck_eff, o.att, order.patron);
        route_to(world, &tree, from, order.target, &o).ok_or(FailReason::PreconditionLost)?
    };
    // M16a (plan C34): a Fixer's order is the freelance scorer's run.
    let free = matches!(order.why, RunWhy::Freelance | RunWhy::Stat | RunWhy::Fixer);
    if free && route.p_success < world.config.virt.min_route_p {
        return Err(FailReason::PreconditionLost);
    }
    // V32: a prelude whose raid is over (resolved, fizzled, called off) is moot.
    if order.why == RunWhy::Prelude && !raid_pending_for(world, order.patron) {
        world.run_orders.remove(&runner);
        return Err(FailReason::PreconditionLost);
    }
    let now = world.tick;
    let id = run_id(now, runner);
    let at_target = route.nodes.len() == 1 && !overwatch;
    let run = Run {
        id,
        runner,
        deck,
        chair: order.chair,
        patron: order.patron,
        purpose: order.purpose,
        target: order.target,
        route: route.nodes.clone(),
        at: 0,
        phase: if overwatch {
            RunPhase::Out
        } else if at_target {
            RunPhase::BreakIn
        } else {
            RunPhase::Hop
        },
        next_at: if overwatch {
            now + TICKS_PER_HOUR
        } else if at_target {
            now
        } else {
            now + hop_ticks(world, deck_eff)
        },
        payload: [0; 3],
        log: SmallVec::new(),
        portal: from,
        att: o.att,
        deck_eff,
        contests: 0,
        outcome: None,
        source: None,
        mode: order.mode,
        why: order.why,
    };
    world.run_queue.insert((run.next_at, id));
    world.runner_of.insert(runner, id);
    world.runs.insert(id, run);
    let what = match order.purpose {
        Purpose::Data { wipe: true } => "a wipe",
        Purpose::Data { wipe: false } => "Data",
        Purpose::Ledger => "a ledger",
        Purpose::Door => "a door",
        Purpose::Robot(_) => "a robot",
        Purpose::Camera(_) => "a camera",
        Purpose::Overwatch(_) => "overwatch",
    };
    let text = format!(
        "{} jacked in at {} ({what} on {})",
        world.name_of(runner),
        world.name_of(order.chair),
        node_label(world, order.target)
    );
    world.push_event(EventKind::JackedIn, &[runner, order.chair], text);
    Ok(id)
}

/// V10: the per-tick pop: every run step due by now, in `(tick, id)` order
/// (a step due at once is handled in the same tick).
pub(super) fn pop_due(world: &mut World) {
    let now = world.tick;
    while let Some(&(t, id)) = world.run_queue.first() {
        if t > now {
            break;
        }
        world.run_queue.remove(&(t, id));
        step(world, id);
    }
}

/// The run sequence (plan 2.4): Hop → BreakIn → Act → Extract → Out; a lost
/// contest ends it ([`lose`]). Copy the run out, mutate, write it back.
pub fn step(world: &mut World, id: RunId) {
    let Some(mut r) = world.runs.get(&id).cloned() else { return };
    let now = world.tick;
    match r.phase {
        RunPhase::Hop => {
            r.at = r.at.saturating_add(1);
            let Some(&n) = r.route.get(usize::from(r.at)) else {
                return end_run(world, r, RunOutcome::Bounced);
            };
            if n == r.target {
                r.phase = RunPhase::BreakIn;
                r.next_at = now;
            } else if contested(world, n, r.patron) {
                if let Some(rolls) = contest(world, &mut r, n) {
                    return lose(world, r, n, rolls);
                }
                r.next_at = now + hop_ticks(world, r.deck_eff) + contest_ticks(world);
            } else {
                r.next_at = now + hop_ticks(world, r.deck_eff);
            }
        }
        RunPhase::BreakIn => {
            let n = r.target;
            if contested(world, n, r.patron) {
                if let Some(rolls) = contest(world, &mut r, n) {
                    return lose(world, r, n, rolls);
                }
            }
            act_begin(world, &mut r);
            r.phase = RunPhase::Act;
            r.next_at = now + act_ticks(world, r.purpose);
        }
        RunPhase::Act => {
            r.phase = RunPhase::Extract;
            r.next_at = now + contest_ticks(world);
        }
        RunPhase::Extract => {
            let n = r.target;
            if contested(world, n, r.patron) {
                if let Some(rolls) = contest(world, &mut r, n) {
                    return lose(world, r, n, rolls);
                }
            }
            r.phase = RunPhase::Out;
            r.next_at = now;
        }
        RunPhase::Out => {
            // V37: an Overwatch run checks hourly whether its raid is over
            // (resolved, called off, or past the march window).
            if let Purpose::Overwatch(g) = r.purpose {
                let live = world
                    .comp::<Gang>(g)
                    .and_then(|x| x.raid_at)
                    .is_some_and(|t| now < t + crate::systems::faction::march_ticks(world));
                if live {
                    r.next_at = now + TICKS_PER_HOUR;
                    world.run_queue.insert((r.next_at, id));
                    world.runs.insert(id, r);
                    return;
                }
            }
            let outcome = apply_out(world, &mut r);
            return end_run(world, r, outcome);
        }
    }
    world.run_queue.insert((r.next_at, id));
    world.runs.insert(id, r);
}

/// The draws of a lost contest, in the stream's fixed order (V9).
#[derive(Copy, Clone, Debug)]
pub struct LossRolls {
    pub gap: f32,
    pub fry: f32,
    pub dodge: f32,
    pub kill: f32,
    pub trace: f32,
}

/// V8/V9: one contest at `n` on the run's own stream `rng.run(id,
/// contests)`: the pass draw, logged; on a loss the fry, dodge, kill and
/// trace draws follow on the same stream. `None` = won.
pub(super) fn contest(world: &World, r: &mut Run, n: NodeId) -> Option<LossRolls> {
    let mut rng = world.rng.run(r.id, r.contests);
    r.contests = r.contests.saturating_add(1);
    let d = f32::from(def(world, n));
    let won = crate::systems::security::contest_f(r.att, d, contest_step(world), &mut rng);
    r.log.push((world.tick, n, won));
    if won {
        return None;
    }
    Some(LossRolls { gap: d - r.att, fry: rng.random(), dodge: rng.random(), kill: rng.random(), trace: rng.random() })
}

/// V14: what a lost contest at `n` does, from its draws: `(fried, dead,
/// traced)`. Fried when the fry draw is under `fry_base[ice] x (1 + fry_gap
/// x max(gap, 0))` and the dodge draw is not under `p_dodge`; dead when
/// fried and the kill draw is under `p_flatline` (ICE 3 at `gap >=
/// flatline_gap`) or `p_fry_kill[ice]`; traced (independently) when the
/// trace draw is under the trace odds.
pub fn loss_verdict(world: &World, n: NodeId, o: &Odds, rolls: &LossRolls) -> (bool, bool, bool) {
    let cfg = &world.config.ice;
    let (fry_p, trace_p) = loss_odds(world, n, rolls.gap, o);
    let ice = ice_eff(world, n);
    let fried = rolls.fry < fry_p && rolls.dodge >= o.dodge;
    let kill_p =
        if ice >= 3 && rolls.gap >= cfg.flatline_gap { cfg.p_flatline } else { IceCfg::at(&cfg.p_fry_kill, ice) };
    (fried, fried && rolls.kill < kill_p, rolls.trace < trace_p)
}

/// V9: a contest's draws on its stream, in their fixed order: the pass
/// draw, then the fry, dodge, kill and trace draws a loss reads.
pub fn contest_draws(world: &World, run: RunId, contest: u8, gap: f32) -> (f32, LossRolls) {
    let mut rng = world.rng.run(run, contest);
    let pass: f32 = rng.random();
    (pass, LossRolls { gap, fry: rng.random(), dodge: rng.random(), kill: rng.random(), trace: rng.random() })
}

/// V66: a run's trace factor and take as `Odds` for a live runner.
pub fn run_odds(world: &World, r: &Run) -> Odds {
    let (_, trace, _) = mode_terms(world, r.mode);
    let stealth = crate::systems::law::stealth(world, r.runner);
    Odds {
        att: r.att,
        patron: r.patron,
        dodge: crate::systems::vehicles::p_dodge(world, r.runner),
        trace_mult: (1.0 - world.config.ice.stealth_w * stealth).max(0.0) * trace,
    }
}

/// V14, in order: the fry (saved by `p_dodge`): `Fried`, sanity and energy
/// hits, the `Fried` memory, deck wear; then the kill (`p_fry_kill[ice]`, or
/// `p_flatline` at ICE 3 with `gap ≥ flatline_gap`). The trace rolls
/// independently. An Extract loss returns the payload (`Captured`). The
/// recorded outcome is the worst of Flatlined > Fried > Traced > Captured >
/// Bounced. A survivor is dazed `daze_ticks` in the chair.
pub fn lose(world: &mut World, mut r: Run, n: NodeId, rolls: LossRolls) {
    let cfg = world.config.ice.clone();
    let o = run_odds(world, &r);
    let (fried, dead, traced) = loss_verdict(world, n, &o, &rolls);
    let owner = owner_of(world, n);
    let runner = r.runner;
    if fried {
        world.stats.current.virt.fried += 1;
        let text = format!("{}'s ICE fried {}", node_label(world, n), world.name_of(runner));
        world.push_event(EventKind::Fried, &[runner, owner.unwrap_or(EntityId::NONE)], text);
        if let Some(b) = world.comp_mut::<crate::components::Body>(runner) {
            b.sanity = (b.sanity - cfg.fry_sanity).max(0.0);
        }
        if let Some(nd) = world.comp_mut::<crate::components::Needs>(runner) {
            nd.energy = (nd.energy - cfg.fry_energy).max(0.0);
        }
        world.remember(runner, crate::components::MemoryKind::Fried, None, 0.9, -0.8, false);
        if let Some(a) = world.comp_mut::<Asset>(r.deck) {
            a.condition = a.condition.saturating_sub(cfg.fry_wear);
        }
        crate::systems::assets::rekit(world, runner);
    }
    let extract = r.phase == RunPhase::Extract;
    if extract {
        return_payload(world, &mut r);
    }
    let outcome = if dead {
        RunOutcome::Flatlined
    } else if fried {
        RunOutcome::Fried
    } else if traced {
        RunOutcome::Traced
    } else if extract {
        RunOutcome::Captured
    } else {
        RunOutcome::Bounced
    };
    if !dead {
        let until = world.tick + Tick::from(cfg.daze_ticks);
        if let Some(b) = world.comp_mut::<Brain>(runner) {
            b.dazed_until = Some(until);
        }
    }
    let snapshot = r.clone();
    end_run(world, r, outcome);
    if dead {
        let text = format!("{} flatlined in {}'s ICE", world.name_of(runner), node_label(world, n));
        world.push_event(EventKind::Flatlined, &[runner, owner.unwrap_or(EntityId::NONE)], text);
        world.kill_by(runner, crate::components::DeathCause::Flatline, None);
    }
    if traced {
        trace(world, &snapshot, n);
    }
}

/// V15 with V65's decay: the owner learns who sat where. Confidence `(1 −
/// trace_decay_per_hop)^hops`; at or above `trace_floor` a `Sighting` (the
/// chair's door) goes into the owner's `FactionDb`, and a corp or the city
/// files `DataTheft` (Data, Ledger) or `Intrusion` and sets `last_seen` to
/// the chair's door (the guards come; `last_trace` for the chair arrest);
/// a gang pushes `Shock::Hacked` naming the patron, else the runner's gang.
/// Below the floor the trace names nobody. A loud run (V66) raises the
/// node's alarm (`alarm_hours`).
pub fn trace(world: &mut World, r: &Run, n: NodeId) {
    let now = world.tick;
    let owner = owner_of(world, n);
    let tag = world.virt.node(n).map_or(OwnerTag::City, |x| x.owner_kind);
    // M14 review: the hops between the chair's portal and the node that
    // traced (its index on the route), not the whole route's length.
    let at = r.route.iter().position(|&x| x == n).unwrap_or(r.route.len().saturating_sub(1));
    let hops = i32::try_from(at).unwrap_or(i32::MAX);
    let confidence = (1.0 - world.config.ice.trace_decay_per_hop).max(0.0).powi(hops);
    let named = confidence >= world.config.ice.trace_floor;
    let door = world.comp::<Building>(r.chair).map(|b| b.door).unwrap_or_default();
    world.stats.current.virt.traced += 1;
    if named {
        record_sighting(
            world,
            owner,
            crate::virt::Sighting { who: r.runner, tile: door, tick: now, confidence, relayed: false },
        );
    }
    if r.mode == RunMode::Loud {
        let until = now + Tick::from(world.config.ice.alarm_hours) * TICKS_PER_HOUR;
        if let Some(x) = world.virt.node_mut(n) {
            x.alarm_until = Some(until);
        }
        bump_epoch(world);
    }
    let alive = crate::systems::law::living(world, r.runner);
    match tag {
        OwnerTag::Gang => {
            let by = if named { r.patron.or_else(|| world.gang_of(r.runner)) } else { None };
            if let Some(g) = owner {
                crate::systems::gang::push_shock(world, g, crate::components::Shock::Hacked { by });
                // Phase 3 (V30): the VirtRaid `hacked` input and its wipe target.
                if let (Some(b), Some(gg)) = (by, world.comp_mut::<Gang>(g)) {
                    gg.hacked_by = Some((b, now));
                }
            }
        }
        OwnerTag::City | OwnerTag::Corp => {
            if named && alive {
                let crime = match r.purpose {
                    Purpose::Data { .. } | Purpose::Ledger => crate::components::Crime::DataTheft,
                    _ => crate::components::Crime::Intrusion,
                };
                crate::systems::law::file_report(world, crime, r.runner, None);
                world.set_last_seen(r.runner, (door, now));
                world.last_trace.insert(r.runner, (r.chair, now));
            }
        }
    }
    let who = if named { world.name_of(r.runner) } else { "an unknown runner".to_string() };
    let text = format!("{}'s ICE traced {who} to {}", world.owner_label(owner), world.name_of(r.chair));
    let actors = [if named { r.runner } else { EntityId::NONE }, owner.unwrap_or(EntityId::NONE), r.chair];
    world.push_event(EventKind::Traced, &actors, text);
}

/// The payload back into the store it came from (Captured, Dumped).
fn return_payload(world: &mut World, r: &mut Run) {
    let payload = std::mem::take(&mut r.payload);
    if payload == [0; 3] {
        return;
    }
    if let Some(node) = r.source.and_then(|s| world.virt.node_mut(s)) {
        for (slot, p) in node.store.units.iter_mut().zip(payload) {
            *slot = slot.saturating_add(p);
        }
    }
}

/// Act (V29): a Data steal takes `steal_units[deck_eff − 1] × take` from
/// the target's store, the largest track first, into the payload. Wipes,
/// Ledgers, doors, robots and cameras take nothing here.
fn act_begin(world: &mut World, r: &mut Run) {
    if r.purpose != (Purpose::Data { wipe: false }) {
        return;
    }
    let (take, _, _) = mode_terms(world, r.mode);
    let mut budget = (steal_units(world, r.deck_eff) as f32 * take).round() as u32;
    // M17 phase 4: the units live on the parent's account (the node is its window).
    if let Some(NodeKind::Uplink(p)) = world.virt.node(r.target).map(|x| x.kind) {
        r.payload = crate::systems::parents::take_abroad(world, p, budget);
        r.source = Some(r.target);
        return;
    }
    let Some(node) = world.virt.node_mut(r.target) else { return };
    let mut order: SmallVec<[(std::cmp::Reverse<u32>, usize); 3]> =
        node.store.units.iter().enumerate().map(|(i, &u)| (std::cmp::Reverse(u), i)).collect();
    order.sort_unstable();
    for (_, i) in order {
        let got = node.store.units[i].min(budget);
        node.store.units[i] -= got;
        r.payload[i] += got;
        budget -= got;
    }
    r.source = Some(r.target);
}

/// The building a node's loss lands on for `note_loss`: its building, or a
/// corp Ledger's front (the owner's building with the highest value).
fn loss_building(world: &World, n: NodeId) -> Option<EntityId> {
    match world.virt.node(n)?.kind {
        NodeKind::Building(b) => Some(b),
        NodeKind::Ledger(c) => world
            .comp::<Corp>(c)?
            .buildings
            .iter()
            .filter_map(|&b| world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (b, bd.kind)))
            .map(|(b, kind)| (std::cmp::Reverse(ownership::value(world, kind)), b))
            .min()
            .map(|(_, b)| b),
        NodeKind::Public(_) | NodeKind::Uplink(_) => None,
    }
}

/// Out (the Purposes table, phase 2 rows): a Data steal's payload to the
/// patron's store (a gang's Hideout node, a corp's Lab of that focus) or the
/// deck (`Asset.data`), `DataStolen`, the owner's `loss_log`; a Ledger run
/// moves V20's take through `ownership::charge` (`Flow::Hack`, untaxed) to
/// the patron's purse or the runner's wallet, `LedgerHacked`, the loss. Each
/// success rings the node's breaches. The Wipe, Door, Robot, Camera and
/// Overwatch arms are phase 3's.
///
/// Returns the run's outcome (M14 review): `Success`, or `Bounced` for a
/// steal whose payload had nowhere to go (it went back to the store). An
/// empty payload (the store was drained before Act) robs nobody: no loss,
/// no `DataStolen` (so no hardening), no breach.
fn apply_out(world: &mut World, r: &mut Run) -> RunOutcome {
    // M17 phase 4: a run on an uplink lands abroad (edge, tiers, the treasury).
    if let Some(NodeKind::Uplink(p)) = world.virt.node(r.target).map(|x| x.kind) {
        let payload = std::mem::take(&mut r.payload);
        let total: u32 = payload.iter().sum();
        if total > 0 && !deliver(world, r, payload) {
            // Nowhere to put it: back abroad.
            crate::systems::parents::return_abroad(world, p, payload);
        }
        crate::systems::parents::uplink_landed(world, p, r.purpose, total, r.patron.or(Some(r.runner)), r.runner);
        return RunOutcome::Success;
    }
    let owner = owner_of(world, r.target);
    let label = node_label(world, r.target);
    let runner_name = world.name_of(r.runner);
    match r.purpose {
        Purpose::Data { wipe: false } => {
            let payload = std::mem::take(&mut r.payload);
            let total: u32 = payload.iter().sum();
            if total > 0 && !deliver(world, r, payload) {
                r.payload = payload;
                return_payload(world, r);
                return RunOutcome::Bounced;
            }
            if total == 0 {
                return RunOutcome::Success;
            }
            world.stats.current.virt.data_stolen += total;
            let value = i64::from(total) * world.config.data.data_price;
            note_virt_loss(world, r.target, value, r.runner);
            let track = Track::ALL.into_iter().max_by_key(|t| (payload[t.index()], std::cmp::Reverse(*t)));
            let for_whom = r.patron.map(|p| format!(" for {}", world.owner_label(Some(p)))).unwrap_or_default();
            let text =
                format!("{runner_name} lifted {total} {} Data from {label}{for_whom}", track.unwrap_or_default());
            // Phase 3: the node's building as a fourth actor (the ICE-after-loss report reads it).
            let building = loss_building(world, r.target).unwrap_or(EntityId::NONE);
            let actors = [r.runner, owner.unwrap_or(EntityId::NONE), r.patron.unwrap_or(EntityId::NONE), building];
            world.push_event(EventKind::DataStolen, &actors, text);
        }
        // V19: a wipe by a run.
        Purpose::Data { wipe: true } => {
            let value =
                world.virt.node(r.target).map_or(0, |x| i64::from(x.store.total())) * world.config.data.data_price;
            crate::systems::tech::wipe_store(world, r.target, Some(r.runner));
            note_virt_loss(world, r.target, value, r.runner);
        }
        // V32: the doors stand open until `raid_at + door_hours` (one per
        // `door_cooldown_days` per building).
        Purpose::Door => {
            let Some(b) = loss_building(world, r.target) else { return RunOutcome::Success };
            // V32: the doors open for a raid still to come.
            if !raid_pending_for(world, r.patron) {
                return RunOutcome::Success;
            }
            let now = world.tick;
            let cool = Tick::from(world.config.ice.door_cooldown_days) * TICKS_PER_DAY;
            if world.comp::<Building>(b).and_then(|x| x.last_door_open).is_some_and(|t| now.saturating_sub(t) < cool) {
                return RunOutcome::Success;
            }
            let gang = r.patron.filter(|&p| world.has::<Gang>(p));
            let raid_at = gang.and_then(|g| world.comp::<Gang>(g)).and_then(|g| g.raid_at).unwrap_or(now);
            let until = raid_at.max(now) + Tick::from(world.config.hack.door_hours) * TICKS_PER_HOUR;
            set_hack(world, r.target, b, crate::virt::HackEffect::DoorOpen, until);
            if let Some(x) = world.comp_mut::<Building>(b) {
                x.last_door_open = Some(now);
            }
            world.stats.current.virt.doors_hacked += 1;
            let loss = world.config.data.door_loss;
            note_virt_loss(world, r.target, loss, r.runner);
            let gname = world.owner_label(gang);
            let text = format!(
                "{runner_name} opened {}'s doors for {gname} until {:02}:{:02}",
                world.name_of(b),
                (until % TICKS_PER_DAY) / TICKS_PER_HOUR,
                until % TICKS_PER_HOUR
            );
            world.push_event(EventKind::DoorHacked, &[r.runner, gang.unwrap_or(EntityId::NONE), b], text);
        }
        // V33: the robot fights for the patron for `hack_hours`.
        Purpose::Robot(a) => {
            let Some(b) = loss_building(world, r.target) else { return RunOutcome::Success };
            let until = world.tick + Tick::from(world.config.robots.hack_hours) * TICKS_PER_HOUR;
            let side = r.patron.unwrap_or(r.runner);
            if let Some(x) = world.comp_mut::<Asset>(a) {
                x.turned = Some((side, until));
            }
            set_hack(world, r.target, b, crate::virt::HackEffect::RobotTurned(a), until);
            world.stats.current.virt.robots_turned += 1;
            let text = format!("{runner_name} turned the robot at {}", world.name_of(b));
            world.push_event(EventKind::RobotTurned, &[r.runner, b], text);
        }
        // V33: the building's cameras and robot sensors skip their contests.
        Purpose::Camera(_) => {
            let Some(b) = loss_building(world, r.target) else { return RunOutcome::Success };
            let until = world.tick + Tick::from(world.config.hack.blind_hours) * TICKS_PER_HOUR;
            set_hack(world, r.target, b, crate::virt::HackEffect::Blind, until);
            world.stats.current.virt.blinded += 1;
            let text = format!("{runner_name} blinded {}'s sensors", world.name_of(b));
            world.push_event(EventKind::Blinded, &[r.runner, b], text);
        }
        // V37: the stream ends with the raid (`end_run` clears `stream_by`).
        Purpose::Overwatch(_) => {
            return RunOutcome::Success;
        }
        Purpose::Ledger => {
            let (take, _, _) = mode_terms(world, r.mode);
            let corp = owner.filter(|&o| world.has::<Corp>(o));
            let purse = world.purse(corp);
            let amount = (ledger_take(world, purse, r.deck_eff) as f32 * take).round() as i64;
            let to = r.patron.or(Some(r.runner));
            let moved = ownership::charge(world, corp, to, amount, Flow::Hack);
            if moved <= 0 {
                return RunOutcome::Success;
            }
            world.stats.current.virt.ledger_hacks += 1;
            note_virt_loss(world, r.target, moved, r.runner);
            let books = audit_ledger(world, corp, r.runner);
            let cname = world.owner_label(corp);
            let tail = match books {
                Audit::Off => String::new(),
                Audit::Short => format!("; {cname}'s books came up {moved}c short, the hand unknown"),
                Audit::Matched(n) => {
                    let before = if n == 1 { "the run before it".to_string() } else { format!("{n} runs before it") };
                    format!("; {cname}'s books came up {moved}c short and its auditors matched it to {before}: {runner_name}")
                }
            };
            let text = format!("{runner_name} took {moved} from {cname}'s ledger{tail}");
            world.push_event(EventKind::LedgerHacked, &[r.runner, corp.unwrap_or(EntityId::NONE)], text);
            if matches!(books, Audit::Matched(_)) {
                named_by_books(world, corp, r);
            }
        }
    }
    let now = world.tick;
    if let Some(node) = world.virt.node_mut(r.target) {
        node.breaches.push_back(now);
        while node.breaches.len() > 8 {
            node.breaches.pop_front();
        }
    }
    RunOutcome::Success
}

/// Round 4 T6: what a corp's books made of a ledger run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Audit {
    /// The audit off (`[hack] ledger_audit_days` 0).
    Off,
    /// The shortfall noticed, the hand unknown.
    Short,
    /// The hand matched to this many of its runs on these books before.
    Matched(usize),
}

/// Round 4 T6 (`fixer_100695`: 600c from a ledger four times a week, heat
/// 0): a ledger's books (a corp's, `None` the city's) catch every run at
/// once (the shortfall), and keep `(tick, runner)` for `[hack]
/// ledger_audit_days` (`World::ledger_books`). A runner whose run
/// is the `ledger_audit_runs`-th on the same books inside the window is
/// matched: the same deck's prints on each (the books, not a roll).
fn audit_ledger(world: &mut World, corp: Option<EntityId>, runner: EntityId) -> Audit {
    let h = &world.config.hack;
    let (days, need) = (h.ledger_audit_days, h.ledger_audit_runs.max(2) as usize);
    if days == 0 {
        return Audit::Off;
    }
    let now = world.tick;
    let horizon = now.saturating_sub(Tick::from(days) * TICKS_PER_DAY);
    let book = world.ledger_books.entry(corp.unwrap_or(EntityId::NONE)).or_default();
    while book.front().is_some_and(|&(t, _)| t < horizon) {
        book.pop_front();
    }
    let before = book.iter().filter(|&&(_, w)| w == runner).count();
    book.push_back((now, runner));
    if before + 1 >= need {
        Audit::Matched(before)
    } else {
        Audit::Short
    }
}

/// Round 4 T6: a ledger run the books matched names the runner as a trace
/// that named him does (`trace`): the corp's Sighting at the chair's door,
/// `DataTheft` filed (wanted: his heat), `last_seen` and `last_trace` for
/// the guards and the chair arrest.
fn named_by_books(world: &mut World, corp: Option<EntityId>, r: &Run) {
    if !crate::systems::law::living(world, r.runner) {
        return;
    }
    let now = world.tick;
    let door = world.comp::<Building>(r.chair).map(|b| b.door).unwrap_or_default();
    record_sighting(
        world,
        corp,
        crate::virt::Sighting { who: r.runner, tile: door, tick: now, confidence: 1.0, relayed: false },
    );
    crate::systems::law::file_report(world, crate::components::Crime::DataTheft, r.runner, None);
    world.set_last_seen(r.runner, (door, now));
    world.last_trace.insert(r.runner, (r.chair, now));
}

/// V27 (phase 3): a Virt loss of `coins` on node `n`: a corp owner books it
/// (`ownership::note_loss` at the node's building: `loss_log` and the
/// `Robbed` shock) and marks the node in `Corp.virt_losses` (14 days); a
/// gang owner's members remember it (`MemoryKind::Hacked`).
fn note_virt_loss(world: &mut World, n: NodeId, coins: i64, runner: EntityId) {
    let owner = owner_of(world, n);
    match world.virt.node(n).map(|x| x.owner_kind) {
        Some(OwnerTag::Corp) => {
            let Some(corp) = owner else { return };
            if let Some(b) = loss_building(world, n) {
                ownership::note_loss(world, b, coins, Some(runner));
            }
            let now = world.tick;
            let horizon = now.saturating_sub(14 * TICKS_PER_DAY);
            if let Some(c) = world.comp_mut::<Corp>(corp) {
                c.virt_losses.push_back((now, n));
                while c.virt_losses.front().is_some_and(|&(t, _)| t < horizon) {
                    c.virt_losses.pop_front();
                }
            }
        }
        Some(OwnerTag::Gang) => {
            let members = owner.and_then(|g| world.comp::<Gang>(g)).map(|g| g.members.clone()).unwrap_or_default();
            for m in members {
                world.remember(m, crate::components::MemoryKind::Hacked, None, 0.5, -0.4, false);
            }
        }
        _ => {}
    }
}

/// V19: a sacked Hideout's store moves to `winner`'s Hideout node.
pub fn move_store(world: &mut World, from: EntityId, winner: EntityId) {
    let (Some(a), Some(b)) =
        (node_of_building(world, from), world.hideout_of(winner).and_then(|h| node_of_building(world, h)))
    else {
        return;
    };
    let cap = world.config.data.store_cap;
    let Some(units) = world.virt.node_mut(a).map(|x| std::mem::take(&mut x.store.units)) else { return };
    if let Some(x) = world.virt.node_mut(b) {
        for (slot, u) in x.store.units.iter_mut().zip(units) {
            *slot = slot.saturating_add(u).min(cap.max(*slot));
        }
    }
}

/// V32: `gang` has a raid scheduled (`raid_at` set) that has not resolved.
fn raid_pending_for(world: &World, gang: Option<EntityId>) -> bool {
    gang.and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.raid_at.is_some())
}

/// V32/V33: a hack on building `b` and its node until `until`.
fn set_hack(world: &mut World, n: NodeId, b: EntityId, effect: crate::virt::HackEffect, until: Tick) {
    if let Some(x) = world.comp_mut::<Building>(b) {
        x.hacked = Some((effect, until));
    }
    if let Some(x) = world.virt.node_mut(n) {
        x.hacked = Some((effect, until));
    }
    bump_epoch(world);
}

/// A steal's payload to where the run's patron keeps Data: a gang's Hideout
/// node, a corp's Lab of the richest track's focus (else its first Lab),
/// a freelancer's deck. False when there is nowhere to put it.
fn deliver(world: &mut World, r: &Run, payload: [u32; 3]) -> bool {
    let into_node = |world: &mut World, n: NodeId| {
        let Some(node) = world.virt.node_mut(n) else { return false };
        for (slot, p) in node.store.units.iter_mut().zip(payload) {
            *slot = slot.saturating_add(p);
        }
        true
    };
    match r.patron {
        Some(g) if world.has::<Gang>(g) => match world.hideout_of(g).and_then(|h| node_of_building(world, h)) {
            Some(n) => into_node(world, n),
            None => false,
        },
        Some(c) if world.has::<Corp>(c) => {
            let best = Track::ALL.into_iter().max_by_key(|t| (payload[t.index()], std::cmp::Reverse(*t)));
            let lab = crate::systems::tech::data_lab_for(world, c, best.unwrap_or_default());
            match lab.and_then(|b| node_of_building(world, b)) {
                Some(n) => into_node(world, n),
                None => false,
            }
        }
        _ => {
            let Some(a) = world.comp_mut::<Asset>(r.deck) else { return false };
            for (slot, p) in a.data.iter_mut().zip(payload) {
                *slot = slot.saturating_add(p);
            }
            true
        }
    }
}

/// V12: the seated body is pulled out (attacked, arrested, killed, gone):
/// `Dumpshock`, sanity − `dump_sanity`, the payload back to its store, the
/// run ends `Dumped`. A no-op for an agent not on a run.
pub fn dump(world: &mut World, runner: EntityId, why: &str) {
    let Some(id) = world.runner_of.get(&runner).copied() else { return };
    let Some(mut r) = world.runs.get(&id).cloned() else {
        world.runner_of.remove(&runner);
        return;
    };
    return_payload(world, &mut r);
    let hit = world.config.virt.dump_sanity;
    if let Some(b) = world.comp_mut::<crate::components::Body>(runner) {
        b.sanity = (b.sanity - hit).max(0.0);
    }
    let text = format!("{} was dumped from the plane ({why})", world.name_of(runner));
    world.push_event(EventKind::Dumpshock, &[runner], text);
    end_run(world, r, RunOutcome::Dumped);
}

/// V37: `gang`'s stream is over if `runner` held it.
pub fn clear_stream(world: &mut World, gang: EntityId, runner: EntityId) {
    if let Some(x) = world.comp_mut::<Gang>(gang) {
        if x.stream_by == Some(runner) {
            x.stream_by = None;
        }
    }
}

/// V10/V35/V29: the run is over: out of `runs`, the queue and `runner_of`,
/// into `run_log` (64); the counters; hacking drift for a survivor (+
/// `hack_drift`, twice on a success); the Hack cooldown (`fried_cooldown_days`
/// after a fry, else `hack_cooldown_days`); a dazed body waits in the chair;
/// a freelancer's deck holding Data gets `GoTo(DataBuyer) → SellData`
/// appended; the `RunOrder` is dropped.
pub fn end_run(world: &mut World, mut r: Run, outcome: RunOutcome) {
    world.runs.remove(&r.id);
    world.run_queue.remove(&(r.next_at, r.id));
    if world.runner_of.get(&r.runner) == Some(&r.id) {
        world.runner_of.remove(&r.runner);
    }
    r.outcome = Some(outcome);
    let now = world.tick;
    // V37 (plan deviation): an Overwatch stream is no run for the counters
    // or the drift (no contest happens on it).
    let watch = matches!(r.purpose, Purpose::Overwatch(_));
    // V37 (M14 review): the stream ends however the run does (the raid's
    // end, a dump, a death).
    if let Purpose::Overwatch(g) = r.purpose {
        clear_stream(world, g, r.runner);
    }
    let v = &mut world.stats.current.virt;
    if !watch {
        v.runs += 1;
        match outcome {
            RunOutcome::Success => v.runs_ok += 1,
            RunOutcome::Bounced => v.runs_bounced += 1,
            RunOutcome::Captured => v.runs_captured += 1,
            RunOutcome::Dumped => v.runs_dumped += 1,
            RunOutcome::Traced | RunOutcome::Fried | RunOutcome::Flatlined => {}
        }
    }
    let base = if watch { 0.0 } else { world.config.decks.hack_drift };
    let drift = base * if outcome == RunOutcome::Success { 2.0 } else { 1.0 };
    if let Some(s) = world.comp_mut::<Skills>(r.runner) {
        s.hacking = (s.hacking.max(0.0) + drift).min(1.0);
    }
    let days = if outcome == RunOutcome::Fried {
        world.config.hack.fried_cooldown_days
    } else {
        world.config.hack.hack_cooldown_days
    };
    let cool = now + Tick::from(days) * TICKS_PER_DAY;
    let free = matches!(r.why, RunWhy::Freelance | RunWhy::Stat | RunWhy::Fixer);
    let data = free && outcome == RunOutcome::Success && deck_data(world, r.runner) > 0;
    let lab = if data { crate::systems::tech::data_buyer_lab(world, r.runner) } else { None };
    if let Some(b) = world.comp_mut::<Brain>(r.runner) {
        b.cooldowns.insert(GoalKind::Hack, cool);
        let seated = matches!(b.exec, crate::exec::ExecState::JackedIn { run, .. } if run == r.id);
        if seated {
            if let Some(until) = b.dazed_until.filter(|&t| t > now) {
                b.exec = crate::exec::ExecState::Wait { until };
            }
        }
        // V29: the sell-append (M13 D23's plan edit).
        if let (Some(lab), Some(plan)) = (lab, b.plan.as_mut().filter(|p| p.goal == GoalKind::Hack)) {
            use crate::goap::{ActionKind, LocationKey};
            let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
            plan.steps.push(step(ActionKind::GoTo(LocationKey::DataBuyer), Some(lab)));
            plan.steps.push(step(ActionKind::SellData, Some(lab)));
        }
    }
    // M16b phase 2: a Steal record's heist settles with its run.
    if r.why == RunWhy::Contract {
        crate::systems::captives::on_heist_end(world, r.runner, outcome == RunOutcome::Success);
    }
    let lent = world.run_orders.remove(&r.runner).is_some_and(|o| o.why == RunWhy::CorpOrder);
    if lent {
        // V31: the fleet deck goes back to the Lab.
        crate::systems::assets::rekit(world, r.runner);
    }
    world.run_log.push_back(r);
    while world.run_log.len() > 64 {
        world.run_log.pop_front();
    }
}
