use super::*;

/// "the blocks around Market#412" for cell `c`.
pub(super) fn around(world: &World, c: u32) -> String {
    match world.turf.cells.get(&c).and_then(|x| x.landmark) {
        Some(b) => format!("the blocks around {}", world.name_of(b)),
        None => {
            // V5 (V2's reads-wrong: "the blocks at (588, 732)"): a cell with
            // no building of its own is named by the nearest one.
            let t = centre(world, c);
            let near = world.building_grid().nearest(t, 0, |b| {
                let x = world.comp::<Building>(b)?;
                (!x.demolished && x.kind != BuildingKind::Lot).then(|| (x.door.manhattan(t), b.index))
            });
            match near {
                Some((_, _, b)) => format!("the blocks near {}", world.name_of(b)),
                None => format!("the blocks at ({}, {})", t.x, t.y),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The corner
// ---------------------------------------------------------------------------

/// `id`'s corner cell: his Home's when his gang holds it, else the nearest
/// cell his gang holds whose corner is within `corner_reach_tiles` of his
/// Home door.
pub fn corner_cell(world: &World, id: EntityId) -> Option<u32> {
    let c = cfg(world)?;
    let g = world.gang_of(id)?;
    let door = home_door(world, id)?;
    let own = cell_of(world, door)?;
    if world.turf.cells.get(&own).is_some_and(|x| x.status == Status::Held(g)) {
        return Some(own);
    }
    world
        .turf
        .held
        .get(&g)?
        .iter()
        .filter_map(|&k| {
            let t = world.turf.cells.get(&k)?.corner?;
            let d = t.manhattan(door);
            (d <= c.corner_reach_tiles).then_some((d, k))
        })
        .min()
        .map(|(_, k)| k)
}

pub(super) fn corner_hours(c: &TurfCfg, tod: u16) -> bool {
    if c.corner_from <= c.corner_to {
        (c.corner_from..c.corner_to).contains(&tod)
    } else {
        tod >= c.corner_from || tod < c.corner_to
    }
}

/// The corner `id` would hold now: a member's body (not Statistical, not
/// under 18, not inside, not on shift) in the corner hours, his gang not
/// lying low, under `corner_ticks_day` tonight, with a corner cell.
pub fn corner_hold(world: &World, id: EntityId) -> Option<TilePos> {
    let c = cfg(world)?;
    let g = world.gang_of(id)?;
    let b = world.comp::<Brain>(id)?;
    if b.lod == Lod::Statistical || b.cuffed_by.is_some() {
        return None;
    }
    if world.comp::<Gang>(g).is_none_or(|x| x.order == Order::LieLow) {
        return None;
    }
    // The under-18 rule: a child never stands a gang's corner, nor is he
    // mustered to one (V5).
    if world.has::<Sentence>(id) || crate::systems::ages::is_protected(world, id) {
        return None;
    }
    let tod = world.tick_of_day();
    if world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod)) {
        return None;
    }
    // V5: a muster outranks the evening's corner (any hour, no cap).
    if let Some(t) = muster_corner(world, id) {
        return Some(t);
    }
    if !corner_hours(c, tod) {
        return None;
    }
    if world.turf.corner_ticks.get(&id).is_some_and(|&t| t >= c.corner_ticks_day) {
        return None;
    }
    let k = corner_cell(world, id)?;
    let x = world.turf.cells.get(&k)?;
    // V5: nobody holds a corner the law cracks down on.
    if x.crackdown_until > world.tick {
        return None;
    }
    x.corner
}

/// GangWork's scripted plan for holding the corner: `GoTo(Spot) -> HangOut`
/// at it, when the GangWork target is the member himself and he deals no
/// corner (the dealer's is `stims::corner_plan`).
pub fn corner_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if crate::systems::stims::corner_target(world, id).is_some() {
        return None;
    }
    let t = corner_hold(world, id)?;
    if crate::systems::gang::gang_work_target(world, id).map(|x| x.0) != Some(id) {
        return None;
    }
    world.unwind.insert(
        id,
        crate::living::UnwindPick {
            rung: crate::living::Rung::Free,
            act: ActionKind::HangOut,
            venue: None,
            spot: Some(t),
            score: 0.0,
            stay_in: false,
        },
    );
    let ticks = world.config.leisure.hangout_ticks.min(u64::from(u16::MAX)) as u16;
    let e = world.turf.corner_ticks.entry(id).or_default();
    *e = e.saturating_add(ticks.max(1));
    crate::systems::ownership::jobs2_row(world).turf_corner_holds += 1;
    let step = |action, target| ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::HangOut, None)];
    Some(Plan { goal: GoalKind::GangWork, target: None, steps, started_tick: world.tick })
}

/// The corners of `gang`'s held cells (the deal spot, the pulpit).
pub fn corners_of(world: &World, gang: EntityId) -> Vec<TilePos> {
    world
        .turf
        .held
        .get(&gang)
        .map(|v| v.iter().filter_map(|k| world.turf.cells.get(k).and_then(|x| x.corner)).collect())
        .unwrap_or_default()
}

/// The cell index of every standing building, by door (the day's pass).
pub(super) fn cell_buildings(world: &World) -> BTreeMap<u32, Vec<EntityId>> {
    let mut out: BTreeMap<u32, Vec<EntityId>> = BTreeMap::new();
    // scan-ok: daily: the turf pass
    for list in world.buildings_by_kind().values() {
        for &b in list {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if bd.demolished || bd.kind == BuildingKind::Lot {
                continue;
            }
            if let Some(k) = cell_of(world, bd.door) {
                out.entry(k).or_default().push(b);
            }
        }
    }
    for v in out.values_mut() {
        v.sort_unstable();
    }
    out
}

/// The cell's corner and its landmark: the outside door of its venue (a Bar,
/// Market, Club or NoodleBar) nearest the mean of its Home doors, else the
/// stoop of its fullest Home.
pub(super) fn corner_in(world: &World, bs: &[EntityId]) -> Option<(TilePos, EntityId)> {
    let homes: Vec<TilePos> = bs
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).filter(|x| x.kind == BuildingKind::Home && !x.derelict))
        .map(|x| x.door)
        .collect();
    let mid = if homes.is_empty() {
        None
    } else {
        let n = homes.len() as u32;
        let (sx, sy) = homes.iter().fold((0u32, 0u32), |a, t| (a.0 + u32::from(t.x), a.1 + u32::from(t.y)));
        Some(TilePos { x: (sx / n) as u16, y: (sy / n) as u16 })
    };
    let venue = bs
        .iter()
        .filter_map(|&b| {
            let x = world.comp::<Building>(b)?;
            let ok = matches!(
                x.kind,
                BuildingKind::Bar | BuildingKind::Market | BuildingKind::Club | BuildingKind::NoodleBar
            ) && !x.derelict;
            ok.then(|| (mid.map_or(0, |m| x.door.manhattan(m)), b, world.outside_door(x)))
        })
        .min();
    if let Some((_, b, t)) = venue {
        return Some((t, b));
    }
    bs.iter()
        .filter_map(|&b| {
            let x = world.comp::<Building>(b).filter(|x| x.kind == BuildingKind::Home && !x.derelict)?;
            Some((std::cmp::Reverse(world.residents_of(b).len()), b, world.outside_door(x)))
        })
        .min()
        .map(|(_, b, t)| (t, b))
}
