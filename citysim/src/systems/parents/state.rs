use super::*;

/// § 4 the State's heat: `0.4 U(murders 7 d ÷ heat_murders) + 0.3 U(riots
/// 30 d) + 0.3 × the hottest gang's heat` (M15).
pub fn city_heat(world: &World) -> f32 {
    let cfg = &world.config.outside.parents;
    let h = &world.stats.history;
    let murders: u32 = h.iter().rev().take(7).map(|r| r.deaths_violence).sum();
    let riots: u32 = h.iter().rev().take(30).map(|r| r.riots).sum();
    let gang_heat =
        world.gangs().into_iter().map(|g| crate::systems::reputation::rep(world, g).heat).fold(0.0f32, f32::max);
    0.4 * sat(murders as f32 / cfg.heat_murders.max(1e-3)) + 0.3 * sat(riots as f32) + 0.3 * gang_heat
}

/// § 4 the State's table: Hold, Fund, Mandate, Withdraw, best first. Fund
/// scores at the cap too (its action caps), so a hot city keeps its guards.
pub fn state_scores(
    cfg: &crate::config::ParentsCfg,
    heat: f32,
    corrupt: f32,
    wealth: f32,
    guards: u16,
    _max: u16,
) -> Vec<(OutsideOrder, f32)> {
    let pride = cfg.factions.iter().find(|r| r.kind == OutsideKind::State).map_or(0.5, |r| r.persona[1]);
    let mut out = vec![(OutsideOrder::Hold, cfg.flat_hold)];
    let mut add = |o: OutsideOrder, cs: &[Consideration], flat: f32| {
        if cs.iter().all(|c| c.output > 0.0) {
            let raw: f32 = cs.iter().map(|c| c.output).product();
            out.push((o, compensate(raw, cs.len()) + flat));
        }
    };
    add(
        OutsideOrder::Fund,
        &[
            Consideration::new("heat", heat, Curve::Logistic { k: 8.0, mid: 0.4 }),
            Consideration::new("1 - corrupt", 1.0 - corrupt, Curve::Linear { m: 0.6, b: 0.4 }),
            Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ],
        cfg.flat_fund,
    );
    add(
        OutsideOrder::Mandate,
        &[
            Consideration::raw("hot", 0.0, can(heat >= cfg.mandate_heat)),
            Consideration::new("heat", heat, Curve::Linear { m: 0.8, b: 0.2 }),
            Consideration::new("pride", pride, Curve::Linear { m: 0.5, b: 0.5 }),
        ],
        cfg.flat_mandate,
    );
    add(
        OutsideOrder::Withdraw,
        &[
            Consideration::raw("guards", 0.0, can(guards > 0)),
            Consideration::new("1 - heat", 1.0 - heat, Curve::Linear { m: 0.7, b: 0.3 }),
            Consideration::new("corrupt", corrupt, Curve::Logistic { k: 8.0, mid: 0.5 }),
        ],
        cfg.flat_withdraw,
    );
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

fn state_id(world: &World) -> Option<OutsideId> {
    world.outside.factions.iter().find(|f| f.kind == OutsideKind::State && f.parent.is_some()).map(|f| f.id)
}

/// A week of the State's guards' wages into the Treasury (`Flow::Grant`).
fn grant_week(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let amount = i64::from(world.outside.state_guards) * cfg.grant_wage * 7;
    if amount <= 0 {
        return;
    }
    let moved = ownership::cross_in(world, id, None, amount, Flow::Grant);
    if moved > 0 {
        if let Some(f) = world.outside.faction_mut(id) {
            f.income_today += moved;
        }
        if let Some(p) = parent_mut(world, id) {
            p.ledger.spent_today += moved;
        }
        world.outside.today.grants += 1;
        world.push_event(EventKind::StateGrant, &[], format!("the State granted {moved} for its guards"));
    }
}

/// The State's guards at most: `state_guards_max`, under the city's
/// `SetStateGuards` cap when one is set.
pub fn state_cap(world: &World) -> u16 {
    let max = world.config.outside.parents.state_guards_max;
    world.outside.state_guards_cap.map_or(max, |c| c.min(max))
}

/// The State's order's action (also a god's, through the tests).
pub fn state_act(world: &mut World, order: OutsideOrder) {
    let cfg = world.config.outside.parents.clone();
    let Some(id) = state_id(world) else { return };
    let now = world.tick;
    match order {
        OutsideOrder::Fund => {
            world.outside.state_guards = (world.outside.state_guards + cfg.state_guards_step).min(state_cap(world));
            grant_week(world, id);
        }
        OutsideOrder::Mandate => {
            world.outside.mandate_until = Some(now + u64::from(cfg.mandate_days) * TICKS_PER_DAY);
            world.outside.today.mandates += 1;
            world.push_event(EventKind::StateMandate, &[], "the State mandated a harder posture".to_string());
        }
        OutsideOrder::Withdraw => {
            world.outside.state_guards = world.outside.state_guards.saturating_sub(cfg.state_guards_step);
            world.outside.today.withdrawals += 1;
            world.push_event(EventKind::StateWithdrew, &[], "the State withdrew some of its guards".to_string());
        }
        _ => {}
    }
}

/// The State's midnight: the heat, the captain's honour, the table with
/// hysteresis and the minimum hold; Fund raises each day it holds,
/// Withdraw lowers, a Mandate runs once; a week's grant each seventh
/// midnight while guards stand. A negative treasury forces Withdraw.
pub(super) fn state_daily(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    // Phase 4: in a hot city (`mandate_heat`), a megacorp parent whose
    // branch is this hot is sanctioned (deviation: the plan's table
    // sanctioned on the branch's heat alone, and M15's corp heat sits near
    // the threshold on most days: Militech was sanctioned on day 3).
    let city_hot = city_heat(world) >= cfg.mandate_heat;
    let hot: Vec<OutsideId> = world
        .outside
        .parents()
        .filter(|f| f.kind == OutsideKind::Megacorp && !f.dead)
        .filter(|f| {
            f.parent.as_ref().is_some_and(|p| {
                p.branches.iter().any(|&b| crate::systems::reputation::rep(world, b).heat >= cfg.state_sanction_heat)
            })
        })
        .map(|f| f.id)
        .filter(|&x| city_hot && !state_sanctioned(world, x))
        .collect();
    for x in hot {
        sanction(world, None, x, cfg.sanction_days, cfg.sanction_cut);
    }
    let heat = city_heat(world);
    let corrupt = world
        .building_of_kind(crate::components::BuildingKind::Jail)
        .map_or(0.0, |j| 1.0 - crate::systems::reputation::rep(world, j).honour);
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    let wealth = sat(f.treasury as f32 / f.treasury_ref.max(1) as f32);
    let current = p.order;
    let mut scores = state_scores(&cfg, heat, corrupt, wealth, world.outside.state_guards, state_cap(world));
    for s in &mut scores {
        if s.0 == current {
            s.1 += cfg.hysteresis;
        }
    }
    if mandate_active(world) {
        scores.retain(|s| s.0 != OutsideOrder::Mandate);
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let held = now.saturating_sub(p.order_since) >= u64::from(cfg.min_hold_days) * TICKS_PER_DAY;
    let still = scores.iter().any(|s| s.0 == current);
    let mut next = match scores.first() {
        Some(&(best, _)) if best != current && (held || !still) => best,
        _ => current,
    };
    if f.treasury < 0 && world.outside.state_guards > 0 {
        next = OutsideOrder::Withdraw;
    }
    let name = f.name.clone();
    if let Some(p) = parent_mut(world, id) {
        p.order_trace = scores;
        if next != current {
            p.order = next;
            p.order_since = now;
        }
    }
    if next != current {
        world.push_event(EventKind::OutsideOrder, &[], format!("{name}: {} -> {}", current.label(), next.label()));
    }
    match next {
        OutsideOrder::Fund if world.outside.state_guards < state_cap(world) => state_act(world, OutsideOrder::Fund),
        OutsideOrder::Mandate if !mandate_active(world) => state_act(world, OutsideOrder::Mandate),
        OutsideOrder::Withdraw if world.outside.state_guards > 0 => state_act(world, OutsideOrder::Withdraw),
        _ => {
            if world.day().is_multiple_of(7) {
                grant_week(world, id);
            }
        }
    }
}

/// The State's Mandate runs.
pub fn mandate_active(world: &World) -> bool {
    crate::systems::outside::on(world) && world.outside.mandate_until.is_some_and(|t| t > world.tick)
}

/// The law's posture scores under a Mandate: Crackdown and Garrison +
/// `mandate_bias`, re-sorted (as `attention::tilt`).
pub fn mandate_tilt(world: &World, scores: &mut [crate::components::PostureScore]) {
    if !mandate_active(world) {
        return;
    }
    let bias = world.config.outside.parents.mandate_bias;
    for s in scores.iter_mut() {
        if matches!(s.posture, crate::components::Posture::Crackdown | crate::components::Posture::Garrison) {
            s.score += bias;
        }
    }
    scores.sort_by(|a, b| b.score.total_cmp(&a.score));
}

/// A parent's branch founded with no building yet, inside its upkeep
/// grace (M11's bankruptcy waits on it; nothing with the parents off).
pub fn in_founding_grace(world: &World, c: &Corp, now: crate::time::Tick) -> bool {
    parents_on(world)
        && c.parent.is_some_and(|p| world.outside.faction(p).is_some_and(|f| f.parent.is_some()))
        && c.upkeep_grace_until.is_some_and(|t| t > now)
}

// ---------------------------------------------------------------------------
// Phase 4: the weapons that reach the ledger, and scrip (docs/M17_OUTSIDE.md
// § 3-4, "Revised 2026-10-12").
//
// The uplink is a node of the M14 plane (`NodeKind::Uplink`): a window on
// the parent's Data and treasury, behind a tier-3 Trunk from its branch's
// Ledger; its ICE is the parent's. A run on it lands abroad: a theft cuts
// the parent's edge, a wipe zeroes its Data in the touched tracks and drops
// a tier there (its branches' effective tiers are capped by the parent's,
// `tech_ceiling`), a Ledger run siphons its treasury. Sanctions cut income
// (and the World's buying from the sanctioned branch); a parent under its
// upkeep for `death_days` dies, its corp branches go bankrupt and its scrip
// is wiped. Scrip is a share of a branch's wages whose coins cross to the
// parent at issue: it is not coins, so `econ::identity` never sees it; it
// becomes coins again only by crossing back from the parent (spent at the
// issuer's buildings, converted at a Fixer, redeemed under `BanScrip`).
// ---------------------------------------------------------------------------

/// The living megacorp parents with a corp branch, and that branch (the
/// uplink's anchor): `virt::wanted`'s and `links`'s source.
pub fn wanted_uplinks(world: &World) -> Vec<(OutsideId, EntityId)> {
    if !parents_on(world) {
        return Vec::new();
    }
    world
        .outside
        .parents()
        .filter(|f| f.kind == OutsideKind::Megacorp && !f.dead)
        .filter_map(|f| corp_branches(world, f.id).first().map(|&b| (f.id, b)))
        .collect()
}

/// A parent's alive uplink node.
pub fn uplink_node(world: &World, id: OutsideId) -> Option<crate::virt::NodeId> {
    world
        .virt
        .nodes
        .iter()
        .position(|n| n.alive && n.kind == crate::virt::NodeKind::Uplink(id))
        .map(|i| crate::virt::NodeId(i as u16))
}

/// An uplink's ICE: `uplink_ice` capped by the parent's Deck tier.
pub fn uplink_ice(world: &World, id: OutsideId) -> u8 {
    let ice = world.config.outside.parents.uplink_ice;
    match parent(world, id).map(|p| p.tech[crate::virt::Track::Deck.index()]) {
        Some(t) if t > 0 => ice.min(t),
        _ => ice,
    }
}

/// The ceiling a branch's parent puts on its tier in `t` (3: none).
pub fn tech_ceiling(world: &World, corp: &Corp, t: crate::virt::Track) -> u8 {
    if !parents_on(world) {
        return 3;
    }
    corp.parent.and_then(|p| parent(world, p)).map(|p| p.tech[t.index()]).filter(|&x| x > 0).unwrap_or(3)
}

/// The uplink node's store mirrors the parent's Data.
pub(super) fn sync_uplink(world: &mut World, id: OutsideId) {
    let Some(data) = parent(world, id).map(|p| p.data.clone()) else { return };
    if let Some(n) = uplink_node(world, id) {
        if let Some(node) = world.virt.node_mut(n) {
            if node.store != data {
                node.store = data;
                crate::systems::virt::bump_epoch(world);
            }
        }
    }
}

/// Take up to `budget` units from the parent's Data, the largest track
/// first (ties the lower track), as `act_begin` takes from a store.
pub fn take_abroad(world: &mut World, id: OutsideId, budget: u32) -> [u32; 3] {
    let mut out = [0u32; 3];
    let Some(p) = parent_mut(world, id) else { return out };
    let mut left = budget;
    let mut order: Vec<(std::cmp::Reverse<u32>, usize)> =
        p.data.units.iter().enumerate().map(|(i, &u)| (std::cmp::Reverse(u), i)).collect();
    order.sort_unstable();
    for (_, i) in order {
        let got = p.data.units[i].min(left);
        p.data.units[i] -= got;
        out[i] += got;
        left -= got;
    }
    sync_uplink(world, id);
    out
}

/// A payload that had nowhere to go goes back abroad.
pub fn return_abroad(world: &mut World, id: OutsideId, payload: [u32; 3]) {
    if let Some(p) = parent_mut(world, id) {
        for (u, x) in p.data.units.iter_mut().zip(payload) {
            *u += x;
        }
    }
    sync_uplink(world, id);
}

/// A run on an uplink lands: a theft cuts the edge by `edge_per_unit` a
/// unit; a wipe zeroes the touched tracks, drops a tier where it is 2 or
/// more and cuts `edge_wipe`; a Ledger run crosses `min(uplink_frac ×
/// treasury, uplink_cap)` to the patron (`Flow::Hack`).
pub fn uplink_landed(
    world: &mut World,
    id: OutsideId,
    purpose: crate::virt::Purpose,
    units: u32,
    patron: Option<EntityId>,
    runner: EntityId,
) {
    let cfg = world.config.outside.parents.clone();
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    let who = if runner == EntityId::NONE { "a runner".to_string() } else { world.name_of(runner) };
    world.outside.today.uplink_runs += 1;
    let text = match purpose {
        crate::virt::Purpose::Data { wipe: false } => {
            if let Some(p) = parent_mut(world, id) {
                p.edge = (p.edge - cfg.edge_per_unit * units as f32).clamp(cfg.edge_min, cfg.edge_max);
            }
            format!("{who} stole {units} Data through {pname}'s uplink")
        }
        crate::virt::Purpose::Data { wipe: true } => {
            let mut dropped = Vec::new();
            if let Some(p) = parent_mut(world, id) {
                for t in crate::virt::Track::ALL {
                    if p.data.units[t.index()] == 0 {
                        continue;
                    }
                    p.data.units[t.index()] = 0;
                    if p.tech[t.index()] >= 2 {
                        p.tech[t.index()] -= 1;
                        dropped.push(t.label());
                    }
                }
                p.edge = (p.edge - cfg.edge_wipe).clamp(cfg.edge_min, cfg.edge_max);
            }
            sync_uplink(world, id);
            crate::systems::virt::bump_epoch(world);
            format!(
                "{who} wiped {pname}'s Data abroad (tiers lost: {})",
                if dropped.is_empty() { "none".into() } else { dropped.join(", ") }
            )
        }
        crate::virt::Purpose::Ledger => {
            let treasury = world.outside.faction(id).map_or(0, |f| f.treasury.max(0));
            let take = ((f64::from(cfg.uplink_frac) * treasury as f64).floor() as i64).min(cfg.uplink_cap);
            let moved = ownership::cross_in(world, id, patron, take, Flow::Hack);
            if let Some(f) = world.outside.faction_mut(id) {
                f.income_today += moved;
            }
            format!("{who} siphoned {moved} from {pname}'s ledger abroad")
        }
        _ => return,
    };
    world.outside.today.uplink_breached += 1;
    let actors = [runner, patron.unwrap_or(EntityId::NONE)];
    world.push_event(EventKind::UplinkBreached, &actors, text);
    // Phase 5: the outside traces the runner; the Blackwall may answer.
    crate::systems::abroad::on_uplink_run(world, id, runner, patron);
}

/// `units` of Data gone from `id` abroad: its edge cut by `edge_per_unit` a unit.
pub fn cut_edge(world: &mut World, id: OutsideId, units: u32) {
    let cfg = world.config.outside.parents.clone();
    if let Some(p) = parent_mut(world, id) {
        p.edge = (p.edge - cfg.edge_per_unit * units as f32).clamp(cfg.edge_min, cfg.edge_max);
    }
}

/// God `ForceUplinkRun`: the run's effect now, no runner (a theft's units
/// at a tier-3 deck's take, to the patron's first Lab when it has one).
pub fn force_uplink_run(
    world: &mut World,
    id: OutsideId,
    purpose: crate::virt::Purpose,
    patron: Option<EntityId>,
) -> Result<String, String> {
    let Some(f) = world.outside.faction(id).filter(|f| f.parent.is_some() && !f.dead) else {
        return Err(format!("ForceUplinkRun: no living parent {id}"));
    };
    let name = f.name.clone();
    let mut units = 0;
    if purpose == (crate::virt::Purpose::Data { wipe: false }) {
        let budget = world.config.data.steal_units.get(2).copied().unwrap_or(0);
        let payload = take_abroad(world, id, budget);
        units = payload.iter().sum();
        let lab = patron
            .map(|x| crate::systems::tech::labs_of(world, x))
            .and_then(|v| v.first().copied())
            .and_then(|l| crate::systems::virt::node_of_building(world, l));
        if let Some(n) = lab {
            if let Some(node) = world.virt.node_mut(n) {
                for (u, x) in node.store.units.iter_mut().zip(payload) {
                    *u += x;
                }
            }
        }
    }
    uplink_landed(world, id, purpose, units, patron, EntityId::NONE);
    Ok(format!("God: a run on {name}'s uplink ({purpose:?})"))
}

/// § 3 sanctions: `by` (a rival parent; `None` the city or the State) cuts
/// `on`'s income by `cut` for `days`.
pub fn sanction(world: &mut World, by: Option<OutsideId>, on: OutsideId, days: u32, cut: f32) {
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.outside.sanctions.push(crate::outside::Sanction { by, on, until, cut: cut.clamp(0.0, 1.0), crew: false });
    world.outside.today.sanctions += 1;
    let name = |w: &World, x: OutsideId| w.outside.faction(x).map_or_else(String::new, |f| f.name.clone());
    let text = match by {
        Some(b) => format!("{} sanctioned {} for {days} days", name(world, b), name(world, on)),
        None => format!("the State sanctioned {} for {days} days", name(world, on)),
    };
    world.push_event(EventKind::Sanctioned, &[], text);
}

/// M17 review: a crew abroad burned `on`'s convoy: its income cut by `cut`
/// for `days`, the crew's sanction (no State's: subsidies and bodies still
/// cross, and a State sanction can still come).
pub fn crew_sanction(world: &mut World, on: OutsideId, days: u32, cut: f32) {
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.outside.sanctions.push(crate::outside::Sanction {
        by: None,
        on,
        until,
        cut: cut.clamp(0.0, 1.0),
        crew: true,
    });
    world.outside.today.sanctions += 1;
    let name = world.outside.faction(on).map_or_else(String::new, |f| f.name.clone());
    world.push_event(
        EventKind::Sanctioned,
        &[],
        format!("a crew burned {name}'s convoy abroad: its income cut for {days} days"),
    );
}

/// The combined cut of the sanctions standing on `id`: `1 − Π(1 − cut)`.
pub fn sanction_cut(world: &World, id: OutsideId) -> f32 {
    let now = world.tick;
    1.0 - world.outside.sanctions.iter().filter(|s| s.on == id && s.until > now).map(|s| 1.0 - s.cut).product::<f32>()
}

/// The State's (or the city's) sanction stands on `id`: no subsidy or body crosses.
pub fn state_sanctioned(world: &World, id: OutsideId) -> bool {
    let now = world.tick;
    world.outside.sanctions.iter().any(|s| s.on == id && s.by.is_none() && !s.crew && s.until > now)
}

/// The cut on a corp's sales to the World (a sanction on its parent).
pub fn world_cut_for(world: &World, owner: Option<EntityId>) -> f32 {
    if !parents_on(world) {
        return 0.0;
    }
    owner.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.parent).map_or(0.0, |p| sanction_cut(world, p))
}

/// God `SanctionParent`.
pub fn god_sanction(world: &mut World, id: OutsideId, days: u32, cut: f32) -> Result<String, String> {
    if parent(world, id).is_none() {
        return Err(format!("SanctionParent: no parent {id}"));
    }
    sanction(world, None, id, days, cut);
    Ok(format!("God: sanctioned parent {id} for {days} days"))
}

/// God `SetParentEdge`.
pub fn set_edge(world: &mut World, id: OutsideId, edge: f32) -> Result<String, String> {
    let cfg = world.config.outside.parents.clone();
    let Some(p) = parent_mut(world, id) else { return Err(format!("SetParentEdge: no parent {id}")) };
    p.edge = edge.clamp(cfg.edge_min, cfg.edge_max);
    Ok(format!("God: parent {id}'s edge set to {:.2}", p.edge))
}

/// God `WipeParentTreasury`: the treasury absorbed abroad (`minted −= it`).
pub fn wipe_treasury(world: &mut World, id: OutsideId) -> Result<String, String> {
    let Some(f) = world.outside.faction_mut(id).filter(|f| f.parent.is_some()) else {
        return Err(format!("WipeParentTreasury: no parent {id}"));
    };
    let t = std::mem::take(&mut f.treasury);
    world.outside.minted -= t;
    Ok(format!("God: parent {id}'s treasury of {t} wiped"))
}
