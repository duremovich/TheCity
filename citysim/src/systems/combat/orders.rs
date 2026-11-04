use super::*;

// ---------------------------------------------------------------------------
// C4: the player (docs/COMBAT.md § 5-6)
// ---------------------------------------------------------------------------

pub(super) const PURPOSE_ODDS: u64 = 0x434F_0009;

/// C4: an order the human gives the body it drives (`PlayerCommand::Fight`).
/// In a fight it is applied at the next beat ([`apply_orders`]); a stance, a
/// draw or a holster outside one holds for the next fight. The body's nerve
/// is its own: it still breaks on its morale, as an NPC's does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Order {
    Stance(Stance),
    /// Go for this body: in a fight it steps up to face you next; outside
    /// one, the Attack verb (a live engagement through the NPC's path).
    Attack(EntityId),
    Draw,
    Holster,
    /// The best tile within `cover_tiles` (§ 6).
    Cover,
    Flee,
    Yield,
    /// Drag a downed body clear (the M16b Rescue): in a fight, break off first.
    Help(EntityId),
}

impl Order {
    pub fn label(self, world: &World) -> String {
        match self {
            Order::Stance(s) => format!("stance: {}", s.order_label()),
            Order::Attack(t) => format!("go for {}", world.name_of(t)),
            Order::Draw => "draw".into(),
            Order::Holster => "holster".into(),
            Order::Cover => "take cover".into(),
            Order::Flee => "flee".into(),
            Order::Yield => "yield".into(),
            Order::Help(t) => format!("drag {} clear", world.name_of(t)),
        }
    }
}

/// C4: a break-off the human ordered, tried at the body's turn in a beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    Flee,
    Yield,
}

/// C4: the human's standing choices and the orders waiting for the beat.
#[derive(Clone, Debug, Default)]
pub struct Orders {
    pub stance: Option<Stance>,
    pub holster: bool,
    pub want: Option<Want>,
    pub help: Option<EntityId>,
    pub pending: Vec<Order>,
}

/// The orders of a body the human drives.
pub(super) fn orders_of(world: &World, id: EntityId) -> Option<&Orders> {
    if world.combat.orders.is_empty() {
        return None;
    }
    world.combat.orders.get(&id).filter(|_| crate::systems::player::drives(world, id))
}

pub(super) fn clear_want(world: &mut World, id: EntityId) {
    if let Some(o) = world.combat.orders.get_mut(&id) {
        o.want = None;
    }
}

/// An entry in `e`'s ring log between beats (a joiner, an order).
pub(super) fn push_log(world: &World, e: &mut Engagement, by: EntityId, on: EntityId, blow: Blow) {
    let cap = world.config.combat.log_beats.max(1);
    if e.log.len() >= cap {
        e.log.pop_front();
    }
    let beat = e.duel.as_ref().map_or(0, |d| d.beat);
    e.log.push_back(BeatLog { tick: world.tick, beat, by, on, blow });
}

/// The fighter of `x` in `e`'s duel (a front or behind one).
fn live_fighter(e: &mut Engagement, x: EntityId) -> Option<&mut Fighter> {
    let d = e.duel.as_mut()?;
    d.f.iter_mut().chain(d.aid.iter_mut().flatten()).find(|f| f.id == x)
}

/// A fighter rebuilt for its stance and weapon now (the human's order):
/// the weapon, the skills it reads, the cover; morale moved by the stance.
fn refit(world: &World, f: &mut Fighter, bout: bool) {
    let n = fighter(world, f.id, f.stance, f.side0, f.bonus, bout);
    f.kind = n.kind;
    f.quality = n.quality;
    f.attack = n.attack;
    f.evasion = n.evasion;
    f.gorilla = n.gorilla;
    f.cover = n.cover;
    f.want = n.want;
    if n.morale0.is_finite() && f.morale.is_finite() {
        f.morale += n.morale0 - f.morale0;
        f.morale0 = n.morale0;
    }
}

/// `x` steps out of `e` from behind his side's front (nobody holds him).
pub(super) fn leave(world: &mut World, e: &mut Engagement, x: EntityId, end: End, foe: EntityId) {
    for s in &mut e.sides {
        s.retain(|&y| y != x);
    }
    if let Some(d) = e.duel.as_mut() {
        for a in &mut d.aid {
            a.retain(|h| h.id != x);
        }
    }
    world.combat.engaged.remove(&x);
    clear_want(world, x);
    push_log(world, e, x, foe, Blow::Broke(end));
    let how = if end == End::Fled { "broke away from" } else { "stepped back from" };
    let text = format!("{} {how} the fight with {}", world.name_of(x), world.name_of(foe));
    world.push_event(EventKind::Fight, &[x, foe], text);
    if end == End::Fled {
        world.combat.fleeing.push(x);
    }
}

/// C4: the human's orders for the bodies it drives in `e`, at the next beat.
pub(super) fn apply_orders(world: &mut World, e: &mut Engagement) {
    if world.combat.orders.is_empty() {
        return;
    }
    let ids: Vec<EntityId> =
        e.members().filter(|x| world.combat.orders.get(x).is_some_and(|o| !o.pending.is_empty())).collect();
    let bout = e.mods.why == Purpose::Bout;
    for x in ids {
        let pending = world.combat.orders.get_mut(&x).map(|o| std::mem::take(&mut o.pending)).unwrap_or_default();
        if !crate::systems::player::drives(world, x) {
            continue;
        }
        for o in pending {
            let Some(k) = (0..2).find(|&k| e.sides[k].contains(&x)) else { break };
            let foe = e.duel.as_ref().map(|d| d.f[1 - k].id).or(e.sides[1 - k].first().copied()).unwrap_or(x);
            match o {
                Order::Stance(st) => {
                    world.combat.orders.entry(x).or_default().stance = Some(st);
                    if let Some(f) = live_fighter(e, x) {
                        f.stance = st;
                        refit(world, f, bout);
                    }
                    push_log(world, e, x, foe, Blow::Stance(st));
                }
                Order::Draw | Order::Holster => {
                    let draw = o == Order::Draw;
                    world.combat.orders.entry(x).or_default().holster = !draw;
                    let beat = e.duel.as_ref().map_or(0, |d| d.beat);
                    if let Some(f) = live_fighter(e, x) {
                        refit(world, f, bout);
                        if draw {
                            f.ready_at = beat.saturating_add(kind_row(world, f.kind).map_or(0, |r| r.draw));
                        }
                    }
                    push_log(world, e, x, foe, Blow::Drew(draw));
                }
                Order::Attack(t) => {
                    // the target steps up to face him at the next duel
                    if let Some(i) = e.sides[1 - k].iter().position(|&y| y == t) {
                        let y = e.sides[1 - k].remove(i);
                        e.sides[1 - k].insert(0, y);
                    }
                    if let Some(f) = live_fighter(e, x) {
                        f.attacked = true;
                    }
                }
                Order::Cover => {
                    take_cover(world, x, Some(foe));
                    let c = world.comp::<Position>(x).map_or(0.0, |p| cover_at(world, p.tile, p.building));
                    if let Some(f) = live_fighter(e, x) {
                        f.cover = c;
                    }
                    push_log(world, e, x, foe, Blow::Cover((c * 100.0).round() as u8));
                }
                Order::Flee | Order::Yield | Order::Help(_) => {
                    if let Order::Help(w) = o {
                        world.combat.orders.entry(x).or_default().help = Some(w);
                    }
                    let want = if o == Order::Yield { Want::Yield } else { Want::Flee };
                    let front = match e.duel.as_ref() {
                        Some(d) => d.f[k].id == x,
                        None => e.sides[k].first() == Some(&x),
                    };
                    if front {
                        world.combat.orders.entry(x).or_default().want = Some(want);
                        if let Some(f) = live_fighter(e, x) {
                            f.want = Some(want);
                        }
                    } else {
                        let end = if want == Want::Flee { End::Fled } else { End::Yielded };
                        leave(world, e, x, end, foe);
                    }
                }
            }
        }
    }
}

/// C4 (§ 6): the cover a body has where it stands, abstract until the
/// interiors (`OPEN-285`): inside a building `cover_inside`; on the street
/// a road is open (`cover_open`), a tile beside a doorway `cover_door`, a
/// tile against walls `cover_wall` a walled side; at most `cover_max`. The
/// shooter's side is not read yet (the sight rule, `OPEN-288`).
pub fn cover_at(world: &World, tile: crate::components::TilePos, building: Option<EntityId>) -> f32 {
    use crate::components::TileKind;
    let c = &world.config.combat;
    if building.is_some() {
        return c.cover_inside.min(c.cover_max);
    }
    let map = &world.map;
    if map.tile_at(tile) == TileKind::Road {
        return c.cover_open;
    }
    let (w, h) = (map.w() as i32, map.h() as i32);
    let (mut walls, mut door) = (0u8, false);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let (x, y) = (i32::from(tile.x) + dx, i32::from(tile.y) + dy);
        if x < 0 || y < 0 || x >= w || y >= h {
            continue;
        }
        match map.tile_at(crate::components::TilePos { x: x as u16, y: y as u16 }) {
            TileKind::Wall => walls += 1,
            TileKind::Door => door = true,
            _ => {}
        }
    }
    let by_door = if door { c.cover_door } else { 0.0 };
    (c.cover_open + (c.cover_wall * f32::from(walls)).max(by_door)).min(c.cover_max)
}

/// C4, Take cover: `x` moves to the best street tile within `cover_tiles`
/// (still within `reach_tiles` of `foe`: a body that moves out of reach has
/// fled); inside a building it stays (the interior's cover). Returns the
/// cover now.
fn take_cover(world: &mut World, x: EntityId, foe: Option<EntityId>) -> f32 {
    use crate::components::{TileKind, TilePos};
    let Some(p) = world.comp::<Position>(x).cloned() else { return 0.0 };
    if p.building.is_some() {
        return cover_at(world, p.tile, p.building);
    }
    let c = &world.config.combat;
    let (r, reach) = (i32::from(c.cover_tiles), c.reach_tiles);
    let foe_at = foe.filter(|&f| f != x).and_then(|f| world.comp::<Position>(f).map(|q| q.tile));
    let (w, h) = (world.map.w() as i32, world.map.h() as i32);
    let mut best = (cover_at(world, p.tile, None), 0u32, p.tile);
    for dy in -r..=r {
        for dx in -r..=r {
            let (tx, ty) = (i32::from(p.tile.x) + dx, i32::from(p.tile.y) + dy);
            if tx < 0 || ty < 0 || tx >= w || ty >= h {
                continue;
            }
            let t = TilePos { x: tx as u16, y: ty as u16 };
            let kind = world.map.tile_at(t);
            if !kind.walkable() || kind == TileKind::Door {
                continue;
            }
            if foe_at.is_some_and(|f| crate::systems::law::chebyshev(f, t) > reach) {
                continue;
            }
            let cv = cover_at(world, t, None);
            let d = p.tile.manhattan(t);
            if cv > best.0 + 1e-4 || ((cv - best.0).abs() <= 1e-4 && d < best.1 && cv > 0.0) {
                best = (cv, d, t);
            }
        }
    }
    if best.2 != p.tile {
        if let Some(q) = world.comp_mut::<Position>(x) {
            q.tile = best.2;
        }
    }
    best.0
}

/// C4: the human's order for the body it drives (`PlayerCommand::Fight`).
pub fn order(world: &mut World, id: EntityId, o: Order) {
    use crate::systems::player;
    let name = world.name_of(id);
    let label = o.label(world);
    let refuse = |world: &mut World, why: &str| {
        let text = format!("{name}: {label} refused: {why}");
        world.push_event(EventKind::PlayerActionFailed, &[id], text);
    };
    if !player::drives(world, id) {
        return refuse(world, "not driven");
    }
    // No child is ever a target (CLAUDE.md, 2026-10-27).
    if let Order::Attack(t) = o {
        if !crate::systems::demography::is_adult(world, t) || crate::systems::ages::is_protected(world, t) {
            return refuse(world, "a child is never a target");
        }
    }
    if let Order::Attack(t) | Order::Help(t) = o {
        if t == id {
            return refuse(world, "yourself");
        }
    }
    if engaged(world, id) {
        world.combat.orders.entry(id).or_default().pending.push(o);
        let text = format!("{name}: {label} (at the next beat)");
        world.push_event(EventKind::PlayerAction, &[id], text);
        return;
    }
    match o {
        Order::Stance(s) => {
            world.combat.orders.entry(id).or_default().stance = Some(s);
            let text = format!("{name}: {label} (for the next fight)");
            world.push_event(EventKind::PlayerAction, &[id], text);
        }
        Order::Draw | Order::Holster => {
            world.combat.orders.entry(id).or_default().holster = o == Order::Holster;
            let k = kind_of(world, id);
            let what = kind_row(world, k).filter(|_| k > 0).map_or("bare hands".to_string(), |r| r.id.clone());
            let text = format!("{name}: {label} ({what})");
            world.push_event(EventKind::PlayerAction, &[id], text);
        }
        Order::Attack(t) => player::act(world, id, player::Verb::Attack(t)),
        Order::Help(w) => player::act(world, id, player::Verb::Help(w)),
        Order::Cover => {
            let cv = take_cover(world, id, None);
            let text = format!("{name}: took cover ({:.0}%)", cv * 100.0);
            world.push_event(EventKind::PlayerAction, &[id], text);
        }
        Order::Flee | Order::Yield => refuse(world, "not in a fight"),
    }
}
