use super::*;

// ---------------------------------------------------------------------------
// V5: the war (TURF § 3)
// ---------------------------------------------------------------------------
//
// **Provocation** ([`provoke_at`], the hourly corner in [`war_hourly`], the
// off-screen night in [`war_daily`]): another gang's act on a held cell (a
// body on its corner, his mark sprayed by it, a deal, a shakedown, a beating
// of a holder's member there) raises the cell's heat for the holder, seen by
// a holder's adult body there (a Statistical act is seen off screen). Past
// `answer_heat` the holder **answers** ([`open_fight`]): the leader sends the
// cell's and the neighbouring cells' members to the corner (the muster:
// [`muster_corner`] in `corner_hold`, local, so reach is no gate) and the
// fight comes `answer_window_hours` later ([`resolve`]): the bodies within
// `muster_reach_tiles` of the corner and, off screen, the Statistical members
// living there stand (placed at the corner; promoted to Coarse on screen,
// the raid window's path), each side scaled by its presence about the cell,
// and `raid::fight_out` pairs them (each blow an Assault or a Murder,
// witnessed as any). **Hold or flip**: the side left standing holds the cell
// (`set_status`, the lock `hold_lock_days`); the side that lost it may come
// back within `retake_days` (**the counter-attack**: a daily roll, the
// leader's line). A contested cell is fought over (**the front**, a daily
// roll). A raid won or a Hideout sacked moves the cells about the Hideout
// ([`raid_moves`]). **Peace**: a side whose losses in the war (dead, jailed,
// gone) pass `exhaust_share` of its strength lies low (`faction::rescore`
// pins LieLow, [`lies_low`]); two spent sides' leaders strike a **truce**
// (the line held `truce_days`); a cell fought over past `crackdown_heat` is
// **cracked down on** by the law (no corner held there, the district's beat
// walks its Homes, [`crackdown_beat`]).
//
// The under-18 rule (CLAUDE.md): a protected member provokes nothing, is
// never mustered (`corner_hold` returns before the muster), never fights
// ([`fighters`]; `raid::fight_out` and its crossfire gate again), never
// counts as a loss; a family with children moves out of a contested cell.

/// `[turf.war]` (absent: no war).
pub fn war_cfg(world: &World) -> Option<&WarCfg> {
    cfg(world)?.war.as_ref()
}

pub(super) fn pair(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

pub(super) fn days(n: u16) -> Tick {
    Tick::from(n) * crate::time::TICKS_PER_DAY
}

pub(super) fn hours(n: u16) -> Tick {
    Tick::from(n) * TICKS_PER_HOUR
}

/// `a` and `b` keep a truce.
pub fn in_truce(world: &World, a: EntityId, b: EntityId) -> bool {
    world.turf.truces.get(&pair(a, b)).is_some_and(|&t| t > world.tick)
}

/// A spent gang lies low (`faction::rescore` pins the order).
pub fn lies_low(world: &World, g: EntityId) -> bool {
    world.turf.lie_low.get(&g).is_some_and(|&t| t > world.tick)
}

pub(super) fn cracked(world: &World, k: u32) -> bool {
    world.turf.cells.get(&k).is_some_and(|x| x.crackdown_until > world.tick)
}

pub(super) fn night_of(world: &World) -> u64 {
    world.tick.saturating_sub(12 * TICKS_PER_HOUR) / crate::time::TICKS_PER_DAY
}

/// A keyed draw in [0, 1) (no stream touched).
pub(super) fn draw(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    use rand::Rng;
    let x = purpose.rotate_left(56) ^ a.rotate_left(17) ^ b.rotate_left(37) ^ world.day();
    world.rng.keyed(crate::rng::splitmix64(x) >> 4).random()
}

/// The corner tile of cell `k` (its centre without one).
pub(super) fn corner_tile(world: &World, k: u32) -> TilePos {
    world.turf.cells.get(&k).and_then(|x| x.corner).unwrap_or_else(|| centre(world, k))
}

/// Round 7 W5: [`corner_name`] of the cell holding `t` (`None` off).
pub fn corner_name_at(world: &World, t: TilePos) -> Option<String> {
    cell_of(world, t).map(|k| corner_name(world, k))
}

/// "the corner by Bar#12" (else "the corner of the blocks near ...").
pub(super) fn corner_name(world: &World, k: u32) -> String {
    match world.turf.cells.get(&k).and_then(|x| x.landmark) {
        Some(b) => format!("the corner by {}", world.name_of(b)),
        None => format!("the corner of {}", around(world, k)),
    }
}

pub(super) fn leader_of(world: &World, g: EntityId) -> Option<EntityId> {
    world.comp::<Gang>(g)?.leader.filter(|&l| crate::systems::law::living(world, l))
}

/// "Vic Stone, The Rust Saints' leader," or "The Rust Saints".
pub(super) fn who_leads(world: &World, g: EntityId) -> String {
    match leader_of(world, g) {
        Some(l) => format!("{}, {} leader,", world.name_of(l), poss(&gname(world, g))),
        None => gname(world, g),
    }
}

pub(super) fn order_of(world: &World, g: EntityId) -> Option<Order> {
    world.comp::<Gang>(g).map(|x| x.order)
}

/// An adult member who can stand in a fight (living, free, not down).
pub(super) fn can_fight(world: &World, m: EntityId) -> bool {
    crate::systems::law::living(world, m)
        && world.has::<Brain>(m)
        && !world.has::<Sentence>(m)
        // The under-18 rule: never a combatant.
        && !crate::systems::ages::is_protected(world, m)
        && !world.wounded.contains_key(&m)
        && world.comp::<Brain>(m).is_none_or(|b| b.cuffed_by.is_none())
}

/// `g`'s fit adults (a war side's strength at its start).
pub(super) fn fit(world: &World, g: EntityId) -> u16 {
    world.comp::<Gang>(g).map_or(0, |x| x.members.iter().filter(|&&m| can_fight(world, m)).count() as u16)
}

/// `g`'s members living in cell `k` or within `muster_cells` of it.
pub(super) fn muster_pool(world: &World, w: &WarCfg, k: u32, g: EntityId) -> Vec<EntityId> {
    let Some(x) = world.comp::<Gang>(g) else { return Vec::new() };
    x.members
        .iter()
        .copied()
        .filter(|&m| {
            can_fight(world, m) && home_cell(world, m).is_some_and(|h| cell_dist(world, h, k) <= w.muster_cells)
        })
        .collect()
}

/// The corner `id` is mustered to: a fight his gang stands in, due within
/// `answer_window_hours`, on a cell within `muster_cells` of his Home's.
pub fn muster_corner(world: &World, id: EntityId) -> Option<TilePos> {
    muster_fight(world, id).map(|x| x.0)
}

/// [`muster_corner`] with the tick the fight is due.
fn muster_fight(world: &World, id: EntityId) -> Option<(TilePos, Tick)> {
    if world.turf.open.is_empty() {
        return None;
    }
    let w = war_cfg(world)?;
    let g = world.gang_of(id)?;
    let lead = hours(w.answer_window_hours);
    let home = home_cell(world, id)?;
    world.turf.open.iter().filter(|&&(_, a, b)| a == g || b == g).find_map(|&(k, _, _)| {
        let f = world.turf.cells.get(&k)?.fight?;
        (f.due <= world.tick + lead && cell_dist(world, home, k) <= w.muster_cells)
            .then(|| (corner_tile(world, k), f.due))
    })
}

/// Round 7 W2 (`OPEN-241`): a mustered member standing on the corner he
/// was called to waits there for the fight: the ticks until it is due
/// (`exec::actions::duration`'s HangOut; the empty-spot leave and other
/// goals but danger or starving give way, `muster_holds`). V15: the called
/// reached the corner, hung out an hour alone (or left the empty spot) and
/// walked off before the fight; 391 of 955 called bodies at it at 20k.
pub fn muster_wait(world: &World, id: EntityId) -> Option<Tick> {
    let (t, due) = muster_fight(world, id)?;
    let reach = war_cfg(world)?.muster_reach_tiles;
    // Round 8 X4 (`OPEN-241`): the wait runs to the latest the fight is held
    // for the called on their way (`waits_for_called`, `[crew]
    // march_wait_ticks` past its hour); once it is fought `muster_wait` is
    // None and the empty-spot leave ends the HangOut (20k shadow:
    // `cultist_12260 d25`, roused at 01:00, on the corner 01:37-03:01 for a
    // fight due 03:00, held for the late, then Sleep 1.021 over the corner
    // 1.004 at 03:03 and home before it was fought at 04:00).
    let end = due + crate::systems::crew::cfg(world).map_or(0, |c| c.march_wait_ticks);
    world
        .comp::<Position>(id)
        .filter(|p| p.building.is_none() && p.tile.manhattan(t) <= reach)
        .map(|_| end.saturating_sub(world.tick) + 1)
}

/// Round 7 W2 (`fixes::committed`): a mustered member's corner plan holds
/// while his fight is pending, as the raid's muster does (`crew::holds`).
pub fn muster_holds(world: &World, id: EntityId, plan: &Plan) -> bool {
    plan.goal == GoalKind::GangWork
        && plan.steps.iter().any(|s| s.action == ActionKind::HangOut)
        && world.unwind.get(&id).is_some_and(|u| u.spot.is_some() && u.spot == muster_corner(world, id))
}
