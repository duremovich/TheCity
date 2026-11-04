use super::*;

// ---------------------------------------------------------------------------
// Phase 2: routing (plan V5-V7), the run as an event chain (V9-V15, V20),
// the freelance Hack goal (V29), decks (V30, V36)
// ---------------------------------------------------------------------------
//
// A run is a short chain of seeded dice contests between a fictional runner
// (deck tier plus skill) and a node (its ICE tier), resolved exactly as
// `law::resolve_fight` resolves a brawl: one draw against one probability.

/// M16 addendum hook: an owner's alertness multiplies every detection roll
/// a run meets (the trace). 1.0 until M16 defines it.
pub fn alertness_mult(world: &World, owner: Option<EntityId>) -> f32 {
    // M16b phase 4: the faction hook (the city's nodes read the law's).
    crate::systems::faction::alertness_mult(world, owner)
}

/// V8: `[chrome] contest_step` (one value for locks, sensors and ICE).
pub(super) fn contest_step(world: &World) -> f32 {
    world.config.chrome.contest_step
}

/// V7: a runner for `patron` contests `n` when the node is guarded (`def >
/// 0`) and not the patron's own (a freelancer: every owner).
pub fn contested(world: &World, n: NodeId, patron: Option<EntityId>) -> bool {
    def(world, n) > 0 && (patron.is_none() || owner_of(world, n) != patron)
}

/// What a runner brings to every contest of a route (V6, V8, V14, V66).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Odds {
    /// `deck_eff + hack_w × hacking` (+ the mode's `*_att`).
    pub att: f32,
    pub patron: Option<EntityId>,
    /// M13's `p_dodge` (the fry's save).
    pub dodge: f32,
    /// `(1 − stealth_w × stealth) × the mode's trace factor`.
    pub trace_mult: f32,
}

/// V66: a freelancer runs loud when `(1 − lawfulness) × courage ≥ loud_min`.
pub fn mode_of(world: &World, agent: EntityId) -> RunMode {
    let p = world.comp::<Personality>(agent);
    let bold = p.map_or(0.0, |p| (1.0 - p.lawfulness) * p.courage);
    if bold >= world.config.hack.loud_min {
        RunMode::Loud
    } else {
        RunMode::Quiet
    }
}

/// V66: `(take, trace factor, att shift)` of a mode.
pub(super) fn mode_terms(world: &World, mode: RunMode) -> (f32, f32, f32) {
    let h = &world.config.hack;
    match mode {
        RunMode::Quiet => (h.quiet_take, h.quiet_trace, h.quiet_att),
        RunMode::Loud => (h.loud_take, h.loud_trace, h.loud_att),
    }
}

/// The runner's odds with a deck working at `deck_eff`.
pub fn odds_of(world: &World, runner: EntityId, deck_eff: u8, patron: Option<EntityId>, mode: RunMode) -> Odds {
    let hacking = world.comp::<Skills>(runner).map_or(0.0, |s| s.hacking.max(0.0));
    let (_, trace, shift) = mode_terms(world, mode);
    let stealth = crate::systems::law::stealth(world, runner);
    Odds {
        att: f32::from(deck_eff) + world.config.ice.hack_w * hacking + shift,
        patron,
        dodge: crate::systems::vehicles::p_dodge(world, runner),
        trace_mult: (1.0 - world.config.ice.stealth_w * stealth).max(0.0) * trace,
    }
}

/// V14: a lost contest's raw fry chance and trace chance at node `n`, the
/// gap being `def − att`: `fry_base[ice] × (1 + fry_gap × max(gap, 0))` and
/// `trace_p[ice] × trace_mult × alertness`, each capped at 1.
pub(super) fn loss_odds(world: &World, n: NodeId, gap: f32, o: &Odds) -> (f32, f32) {
    let cfg = &world.config.ice;
    let ice = ice_eff(world, n);
    let fry = (IceCfg::at(&cfg.fry_base, ice) * (1.0 + cfg.fry_gap * gap.max(0.0))).clamp(0.0, 1.0);
    let alert = alertness_mult(world, owner_of(world, n));
    let trace = (IceCfg::at(&cfg.trace_p, ice) * o.trace_mult * alert).clamp(0.0, 1.0);
    (fry, trace)
}

/// V6: one Dijkstra from `from` over alive nodes, links above `deck_eff`
/// skipped; entering `n` costs `hop_eps` plus `−ln p_pass(att, def(n))`
/// when `n` is contested for `patron`. Ties `(cost, NodeId)`. Cached per
/// `RouteKey` under the cache's stamp (`att` quantized to 1/20, the search
/// runs on the quantized value so a tree is a pure function of its key):
/// the plane's epoch, valid until the next hour or the next alarm or hack
/// expiry ([`cache_until`]). Takes `&World`: the cache is interior (a think
/// scores through it).
pub fn routes_from(world: &World, from: NodeId, deck_eff: u8, att: f32, patron: Option<EntityId>) -> Arc<RouteTree> {
    let att_q = (att * 20.0).round().clamp(0.0, f32::from(u16::MAX)) as u16;
    let key = RouteKey { from, deck_eff, att_q, patron: patron.unwrap_or(EntityId::NONE) };
    let (now, epoch) = (world.tick, world.virt.epoch);
    world.virt.cache.get_or_build(
        now,
        epoch,
        || cache_until(world),
        key,
        || search(world, from, deck_eff, f32::from(att_q) / 20.0, patron),
    )
}

/// The route cache's validity bound from now (M14 review): the next hour
/// boundary, or the earliest alarm or hack expiry still ahead if sooner
/// (`def` drops the alarm's +1 and `ice_eff` a DoorOpen's 0 at that tick
/// with no write, so no epoch bump marks it). O(nodes), only when the cache
/// is re-stamped.
fn cache_until(world: &World) -> Tick {
    let now = world.tick;
    let mut until = (now / TICKS_PER_HOUR + 1) * TICKS_PER_HOUR;
    let mut take = |t: Tick| {
        if t > now && t < until {
            until = t;
        }
    };
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        if let Some(t) = n.alarm_until {
            take(t);
        }
        if let Some((_, t)) = n.hacked {
            take(t);
        }
        let hacked =
            profile_building(world, NodeId(i as u16)).and_then(|b| world.comp::<Building>(b)).and_then(|b| b.hacked);
        if let Some((_, t)) = hacked {
            take(t);
        }
    }
    until
}

pub(super) fn search(world: &World, from: NodeId, deck_eff: u8, att: f32, patron: Option<EntityId>) -> RouteTree {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let p = &world.virt;
    let len = p.nodes.len();
    let mut tree = RouteTree { dist: vec![f32::INFINITY; len], pred: vec![None; len], hops: vec![0; len] };
    if p.nodes.get(from.index()).is_none_or(|n| !n.alive) {
        return tree;
    }
    let (eps, step) = (world.config.virt.hop_eps, contest_step(world));
    let enter: Vec<f32> = (0..len)
        .map(|i| {
            let m = NodeId(i as u16);
            let toll = if p.nodes[i].alive && contested(world, m, patron) {
                -crate::systems::security::contest_p(att, f32::from(def(world, m)), step).ln()
            } else {
                0.0
            };
            eps + toll
        })
        .collect();
    let mut done = vec![false; len];
    let mut heap = BinaryHeap::new();
    tree.dist[from.index()] = 0.0;
    heap.push(Reverse((ordered_float::OrderedFloat(0.0f32), from)));
    while let Some(Reverse((ordered_float::OrderedFloat(d), n))) = heap.pop() {
        if done[n.index()] {
            continue;
        }
        done[n.index()] = true;
        for &(m, l) in &p.adj[n.index()] {
            if p.links.get(usize::from(l)).is_none_or(|k| k.tier > deck_eff) || done[m.index()] {
                continue;
            }
            let nd = d + enter[m.index()];
            if nd < tree.dist[m.index()] {
                tree.dist[m.index()] = nd;
                tree.pred[m.index()] = Some(n);
                tree.hops[m.index()] = tree.hops[n.index()].saturating_add(1);
                heap.push(Reverse((ordered_float::OrderedFloat(nd), m)));
            }
        }
    }
    tree
}

/// V6: the route to `to` read off `tree` (walking `pred`), with its odds:
/// `p_success` = Π over contested intermediates `p` × `p(to)²` (the target
/// twice, when contested); `p_fry` and `p_trace` = `1 − Π(1 − q)` over the
/// same contests, `q = (1 − p) × fry × (1 − dodge)` and `(1 − p) × trace`.
/// `None` when unreachable or longer than 8 nodes.
pub fn route_to(world: &World, tree: &RouteTree, from: NodeId, to: NodeId, o: &Odds) -> Option<Route> {
    if !tree.dist.get(to.index()).is_some_and(|d| d.is_finite()) {
        return None;
    }
    let mut nodes: SmallVec<[NodeId; 8]> = SmallVec::new();
    let mut cur = to;
    loop {
        nodes.push(cur);
        if nodes.len() > 8 {
            return None;
        }
        if cur == from {
            break;
        }
        cur = tree.pred.get(cur.index()).copied().flatten()?;
    }
    nodes.reverse();
    let step = contest_step(world);
    let (mut ok, mut no_fry, mut no_trace) = (1.0f32, 1.0f32, 1.0f32);
    for &n in nodes.iter().skip(1) {
        if !contested(world, n, o.patron) {
            continue;
        }
        let d = f32::from(def(world, n));
        let p = crate::systems::security::contest_p(o.att, d, step);
        let (fry, trace) = loss_odds(world, n, d - o.att, o);
        for _ in 0..if n == to { 2 } else { 1 } {
            ok *= p;
            no_fry *= 1.0 - (1.0 - p) * fry * (1.0 - o.dodge);
            no_trace *= 1.0 - (1.0 - p) * trace;
        }
    }
    Some(Route { nodes, p_success: ok, p_fry: 1.0 - no_fry, p_trace: 1.0 - no_trace })
}

/// V5: the node a run starts from: the chair building's own node when it
/// has one and its owner is the patron or the runner's employer corp, else
/// the Public node of the chair's district.
pub fn portal(world: &World, chair: EntityId, runner: EntityId, patron: Option<EntityId>) -> NodeId {
    let employer =
        world.comp::<crate::components::Job>(runner).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e));
    if let Some(n) = node_of_building(world, chair) {
        let owner = owner_of(world, n);
        if owner.is_some() && (owner == patron || owner == employer) {
            return n;
        }
    }
    public_of(world, world.district_of_building(chair))
}

/// V5: the agent's chair: the nearest (Manhattan from its tile, ties the
/// lower id) of its Home, its gang's Hideout, its workplace when that is a
/// Lab, and every open Bar and Hotel (a public terminal).
pub fn chair_for(world: &World, agent: EntityId, patron: Option<EntityId>) -> Option<Portal> {
    let tile = world.comp::<Position>(agent)?.tile;
    let usable = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict);
    let mut cands: SmallVec<[EntityId; 8]> = SmallVec::new();
    cands.extend(world.comp::<crate::components::Household>(agent).and_then(|h| h.home));
    cands.extend(world.gang_of(agent).and_then(|g| world.hideout_of(g)));
    cands.extend(
        world
            .comp::<crate::components::Job>(agent)
            .and_then(|j| j.employer)
            .filter(|&e| world.comp::<Building>(e).is_some_and(|b| b.kind == BuildingKind::Lab)),
    );
    let terminals =
        world.buildings_of_kind(BuildingKind::Bar).iter().chain(world.buildings_of_kind(BuildingKind::Hotel));
    let chair = cands
        .into_iter()
        .chain(terminals.copied().filter(|&b| !world.is_closed(b)))
        .filter(|&b| usable(b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(tile), b)))
        .min()
        .map(|(_, b)| b)?;
    Some(Portal { building: chair, node: portal(world, chair, agent, patron) })
}

/// A public terminal: a Bar or a Hotel.
pub(super) fn is_terminal(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| matches!(bd.kind, BuildingKind::Bar | BuildingKind::Hotel))
}
