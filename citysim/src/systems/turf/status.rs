use super::*;

/// A cell's next status from `old` and the window's presence (best first).
pub(super) fn judge(c: &TurfCfg, old: Status, ranked: &[(EntityId, f32)]) -> Status {
    let p_of = |g: EntityId| ranked.iter().find(|x| x.0 == g).map_or(0.0, |x| x.1);
    let keep = c.keep_share * c.hold_min;
    if let Status::Held(h) = old {
        let ph = p_of(h);
        if ph >= keep {
            // The best challenger.
            let ch = ranked.iter().find(|x| x.0 != h).copied();
            return match ch {
                Some((q, pq)) if pq >= c.hold_min && pq > ph * (1.0 + c.hold_margin) => Status::Held(q),
                Some((q, pq)) if pq >= keep && pq * (1.0 + c.hold_margin) > ph => Status::Contested(h, q),
                _ => Status::Held(h),
            };
        }
    }
    let Some(&(g1, p1)) = ranked.first() else { return Status::Open };
    if p1 < c.hold_min {
        return Status::Open;
    }
    match ranked.get(1) {
        Some(&(g2, p2)) if p2 >= keep && p2 * (1.0 + c.hold_margin) > p1 => Status::Contested(g1, g2),
        _ => Status::Held(g1),
    }
}

/// The holder change, one place (V5 flips cells through it): the status
/// written and, with `tell`, the `Turf` line and the loser's `LostTurf`.
pub fn set_status(world: &mut World, k: u32, old: Status, new: Status, tell: bool, now: Tick) {
    if let Some(x) = world.turf.cells.get_mut(&k) {
        x.status = new;
        x.since = now;
    } else if new != Status::Open {
        world.turf.cells.insert(k, Cell { status: new, since: now, ..Default::default() });
    }
    if !tell {
        return;
    }
    let at = around(world, k);
    let landmark = world.turf.cells.get(&k).and_then(|x| x.landmark);
    // On the corner most nights: the window's median of the nights' counts.
    let faces = |world: &World, g: EntityId| -> u16 {
        let Some(x) = world.turf.cells.get(&k) else { return 0 };
        let mut v: Vec<u16> = x.days.iter().map(|d| d.iter().find(|p| p.gang == g).map_or(0, |p| p.corner)).collect();
        v.sort_unstable();
        v.get(v.len() / 2).copied().unwrap_or(0)
    };
    let about = |world: &World, g: EntityId| -> u16 {
        world.turf.cells.get(&k).map_or(0, |x| {
            x.days.iter().filter_map(|d| d.iter().find(|p| p.gang == g)).map(|p| p.faces).max().unwrap_or(0)
        })
    };
    // The members named: those living there, then the lowest ids (adults only).
    let named = |world: &World, g: EntityId| -> Vec<EntityId> {
        let members = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
        let mut v: Vec<EntityId> = members
            .iter()
            .copied()
            .filter(|&m| home_cell(world, m) == Some(k) && !crate::systems::ages::is_protected(world, m))
            .take(3)
            .collect();
        if v.is_empty() {
            v.extend(
                members
                    .iter()
                    .copied()
                    .filter(|&m| corner_cell(world, m) == Some(k) && !crate::systems::ages::is_protected(world, m))
                    .take(3),
            );
        }
        v
    };
    let row_change = |world: &mut World| crate::systems::ownership::jobs2_row(world).turf_changes += 1;
    match (old, new) {
        (_, Status::Held(y)) if old.holder() != Some(y) => {
            let n = faces(world, y);
            let lost = match old {
                Status::Held(x) => Some(x),
                Status::Contested(a, b) => [a, b].into_iter().find(|&g| g != y),
                Status::Open => None,
            };
            let from = lost.map_or(String::new(), |x| format!(", taken from {}", gname(world, x)));
            let m = about(world, y);
            let text = if n > 0 {
                format!("{} hold {at} now{from}: {n} of their people on the corner most nights", gname(world, y))
            } else if m > 0 {
                format!(
                    "{} hold {at} now{from}: {m} of their people about the blocks, nobody on the corner yet",
                    gname(world, y)
                )
            } else {
                format!("{} hold {at} now{from}: their Hideout's blocks, nobody on the corner yet", gname(world, y))
            };
            let mut actors = vec![y];
            actors.extend(named(world, y));
            if let Some(x) = lost {
                actors.push(x);
                actors.extend(named(world, x));
            }
            actors.extend(landmark);
            world.push_event(EventKind::Turf, &actors, text);
            row_change(world);
            if let Some(x) = lost.filter(|&x| world.has::<Gang>(x)) {
                crate::systems::gang::push_shock(world, x, Shock::LostTurf { by: Some(y) });
                crate::systems::ownership::jobs2_row(world).turf_lost += 1;
            }
        }
        (Status::Held(x), Status::Contested(a, b)) => {
            let y = if a == x { b } else { a };
            let text = format!(
                "{} are on {} corner: {at} are contested ({} of theirs about the blocks, {} of {})",
                gname(world, y),
                poss(&gname(world, x)),
                about(world, y),
                about(world, x),
                poss(&gname(world, x))
            );
            let mut actors = vec![y];
            actors.extend(named(world, y));
            actors.push(x);
            actors.extend(named(world, x));
            actors.extend(landmark);
            world.push_event(EventKind::Turf, &actors, text);
            crate::systems::ownership::jobs2_row(world).turf_contests += 1;
        }
        (Status::Held(x), Status::Open) => {
            let text = format!("{} no longer hold {at}: nobody of theirs on the corner", gname(world, x));
            let mut actors = vec![x];
            actors.extend(named(world, x));
            actors.extend(landmark);
            world.push_event(EventKind::Turf, &actors, text);
            row_change(world);
        }
        _ => {}
    }
}

/// The held lists and the neighbours from the statuses.
pub(super) fn reindex(world: &mut World) {
    let mut held: BTreeMap<EntityId, Vec<u32>> = BTreeMap::new();
    for (&k, x) in &world.turf.cells {
        if let Status::Held(g) = x.status {
            held.entry(g).or_default().push(k);
        }
    }
    // The neighbour: the gang holding the most cells bordering ours (a
    // contest counts its other side), ties by its presence, then the id.
    let mut rival: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    for (&g, cells) in &held {
        let mut border: BTreeMap<EntityId, (u32, f32)> = BTreeMap::new();
        let mine: BTreeSet<u32> = cells.iter().copied().collect();
        let mut near: BTreeSet<u32> = BTreeSet::new();
        for &k in cells {
            for n in neighbours(world, k) {
                if !mine.contains(&n) {
                    near.insert(n);
                }
            }
        }
        for n in near {
            let Some(x) = world.turf.cells.get(&n) else { continue };
            let other = match x.status {
                Status::Held(q) if q != g => Some(q),
                Status::Contested(a, b) if a == g || b == g => Some(if a == g { b } else { a }),
                Status::Contested(a, _) => Some(a),
                _ => None,
            };
            if let Some(q) = other.filter(|&q| q != g && world.has::<Gang>(q)) {
                let e = border.entry(q).or_default();
                e.0 += 1;
            }
        }
        for (q, e) in border.iter_mut() {
            e.1 = held.get(q).map_or(0, |v| v.len()) as f32;
        }
        if let Some((q, _)) =
            border.into_iter().max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(a.1 .1.total_cmp(&b.1 .1)).then(b.0.cmp(&a.0)))
        {
            rival.insert(g, q);
        }
    }
    world.turf.held = held;
    world.turf.rival = rival;
}

/// `World::rival_of`'s turf answer: the neighbour, while both stand.
pub fn rival(world: &World, gang: EntityId) -> Option<EntityId> {
    world.turf.rival.get(&gang).copied().filter(|&r| r != gang && world.has::<Gang>(r))
}

// ---------------------------------------------------------------------------
// Turf that pays
// ---------------------------------------------------------------------------

/// Daily (from `gang::daily_economy`, in place of the per-Home tribute):
/// each Home of a held cell pays its holder, `tribute_held` a Home it holds
/// (claim at `CLAIM_HELD`), `tribute_home` any other, from its adults'
/// purses (`Flow::Coerced`); a held Home outside any held cell pays its
/// claimant `tribute_held` as before. Nobody under 18 pays.
pub fn tribute(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let held_min = crate::systems::gang::CLAIM_HELD;
    let mut dues: Vec<(EntityId, EntityId, i64)> = Vec::new();
    // scan-ok: daily: the tribute pass over the Homes
    for &h in world.buildings_of_kind(BuildingKind::Home) {
        let Some(bd) = world.comp::<Building>(h) else { continue };
        if bd.demolished || bd.derelict {
            continue;
        }
        let claim = bd.claim.filter(|cl| cl.count >= held_min && world.has::<Gang>(cl.gang)).map(|cl| cl.gang);
        let holder = holder_at(world, bd.door).filter(|&g| world.has::<Gang>(g));
        let (to, owed) = match (holder, claim) {
            (Some(g), Some(cg)) if g == cg => (g, c.tribute_held),
            (Some(g), _) => (g, c.tribute_home),
            (None, Some(cg)) => {
                // Contested or open: the claimant keeps his Home's tribute.
                (cg, c.tribute_held)
            }
            (None, None) => continue,
        };
        if owed > 0 {
            dues.push((h, to, owed));
        }
    }
    let mut total = 0i64;
    let mut by_gang: BTreeMap<EntityId, i64> = BTreeMap::new();
    for (h, g, mut owed) in dues {
        let payers: Vec<EntityId> = world
            .residents_of(h)
            .iter()
            .copied()
            .filter(|&r| !crate::systems::ages::is_protected(world, r))
            .filter(|&r| world.gang_of(r) != Some(g))
            .collect();
        let start = owed;
        for r in payers {
            if owed == 0 {
                break;
            }
            let coins = world.comp::<Wallet>(r).map_or(0, |w| w.coins);
            let pay = owed.min(coins.max(0));
            if pay > 0 {
                owed -= crate::systems::ownership::pay(
                    world,
                    Some(r),
                    Some(g),
                    pay,
                    crate::systems::ownership::Flow::Coerced,
                );
            }
        }
        let paid = start - owed;
        if paid > 0 {
            *by_gang.entry(g).or_default() += paid;
            total += paid;
        }
    }
    for (g, paid) in by_gang {
        // L2 L20: the week's tribute, for the leader's Collect.
        crate::systems::leisure::note_tribute(world, g, paid);
    }
    crate::systems::ownership::jobs2_row(world).turf_tribute += total;
}

/// Protection by cell: `gang` leans on business `b` when it holds `b`'s
/// cell or is one side of its contest (off: always).
pub fn leans(world: &World, gang: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return true;
    }
    door_of(world, b).is_some_and(|t| status_at(world, t).involves(gang))
}

/// `b` stands in a cell `gang` contests (a second gang may squeeze it).
pub fn contested_by(world: &World, gang: EntityId, b: EntityId) -> bool {
    door_of(world, b)
        .is_some_and(|t| matches!(status_at(world, t), Status::Contested(..)) && status_at(world, t).involves(gang))
}

/// The districts of `gang`'s held and contested cells, ascending.
pub fn ground(world: &World, gang: EntityId) -> Vec<DistrictId> {
    let mut out: Vec<DistrictId> = world
        .turf
        .cells
        .iter()
        .filter(|(_, x)| x.status.involves(gang))
        .map(|(&k, _)| world.district_of(centre(world, k)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// GangWork's Expand: a door in a cell another gang holds is that gang's
/// (the holder protects his own; V14's `Block#478` paid three gangs).
pub fn open_to(world: &World, gang: EntityId, door: TilePos) -> bool {
    holder_at(world, door).is_none_or(|g| g == gang || !world.has::<Gang>(g))
}

// ---------------------------------------------------------------------------
// The residents read the colours
// ---------------------------------------------------------------------------

/// After dark, `id` (no side of it) passes over tile `t` in a contested cell.
pub fn avoids(world: &World, id: EntityId, t: TilePos) -> bool {
    if !on(world) || !world.is_dark() {
        return false;
    }
    match status_at(world, t) {
        Status::Contested(a, b) => world.gang_of(id).is_none_or(|g| g != a && g != b),
        _ => false,
    }
}

/// A body's evening pick made after dark (`leisure::unwind_plan`,
/// `hangout_plan`): when a spot of its district lies in a contested cell
/// within `avoid_tiles` of where it sets out (passed over by `avoids`), the
/// pass-over is told once a night.
pub fn note_avoided(world: &mut World, id: EntityId, from: TilePos) {
    let Some(c) = cfg(world) else { return };
    let r = c.avoid_tiles;
    if !world.is_dark() || world.comp::<Brain>(id).is_none_or(|b| b.lod != Lod::Full) {
        return;
    }
    let night = world.tick.saturating_sub(12 * TICKS_PER_HOUR) / crate::time::TICKS_PER_DAY;
    if world.turf.avoided.get(&id) == Some(&night) {
        return;
    }
    let d = world.district_of(from);
    let hit = world.spots.get(d.index()).and_then(|v| {
        v.iter()
            .map(|s| s.tile)
            .filter(|&t| t.manhattan(from) <= r && avoids(world, id, t))
            .min_by_key(|t| (t.manhattan(from), t.x, t.y))
    });
    let Some(t) = hit else { return };
    let Some(k) = cell_of(world, t) else { return };
    let Status::Contested(a, b) = status_at(world, t) else { return };
    world.turf.avoided.insert(id, night);
    let text = format!(
        "{} kept off {} after dark ({} and {} at odds there)",
        world.name_of(id),
        around(world, k),
        gname(world, a),
        gname(world, b)
    );
    world.push_event(EventKind::Turf, &[id], text);
    crate::systems::ownership::jobs2_row(world).turf_avoided += 1;
}

/// Daily: up to `family_moves_per_1000` per 1,000 residents, a household
/// with children in a contested cell moves into the nearest Home with room
/// it can pay outside any contested cell within `family_move_tiles` (the
/// adults told, the children with them). The children's protection.
pub(super) fn family_moves(world: &mut World, c: &TurfCfg, index: &BTreeMap<u32, Vec<EntityId>>) {
    let pop = world.citizens().len() as f32;
    let limit = (c.family_moves_per_1000 * pop / 1000.0).round() as usize;
    if limit == 0 {
        return;
    }
    let contested: Vec<u32> =
        world.turf.cells.iter().filter(|(_, x)| matches!(x.status, Status::Contested(..))).map(|(&k, _)| k).collect();
    let mut moved = 0usize;
    for k in contested {
        let homes: Vec<EntityId> = index
            .get(&k)
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|&b| world.comp::<Building>(b).is_some_and(|x| x.kind == BuildingKind::Home))
                    .collect()
            })
            .unwrap_or_default();
        for h in homes {
            if moved >= limit {
                return;
            }
            let residents = world.residents_of(h).to_vec();
            // The household: an adult with children at this Home (not a member
            // of either side: they stay for the fight).
            let Status::Contested(ga, gb) = world.turf.cells.get(&k).map_or(Status::Open, |x| x.status) else {
                continue;
            };
            let head = residents.iter().copied().find(|&a| {
                world.has::<Brain>(a)
                    && !crate::systems::ages::is_protected(world, a)
                    && !world.has::<Sentence>(a)
                    && !crate::systems::life::housed_now(world, a)
                    && world.gang_of(a).is_none_or(|g| g != ga && g != gb)
                    && crate::systems::demography::children_of_agent(world, a).iter().any(|&ch| {
                        world.has::<crate::components::Child>(ch)
                            && world.comp::<Household>(ch).and_then(|x| x.home) == Some(h)
                    })
            });
            let Some(a) = head else { continue };
            let spouse = world
                .spouse_of(a)
                .filter(|&s| world.comp::<Household>(s).and_then(|x| x.home) == Some(h) && world.has::<Brain>(s));
            let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
                .into_iter()
                .filter(|&ch| {
                    world.has::<crate::components::Child>(ch)
                        && world.comp::<Household>(ch).and_then(|x| x.home) == Some(h)
                })
                .collect();
            let need = 1 + usize::from(spouse.is_some()) + kids.len();
            let purse = world.comp::<Wallet>(a).map_or(0, |w| w.coins)
                + spouse.and_then(|s| world.comp::<Wallet>(s)).map_or(0, |w| w.coins);
            let Some(door) = door_of(world, h) else { continue };
            let r = c.family_move_tiles;
            let pick = world.building_grid().nearest(door, 0, |b| {
                if b == h {
                    return None;
                }
                let x = world.comp::<Building>(b)?;
                let d = x.door.manhattan(door);
                let ok = x.kind == BuildingKind::Home
                    && !x.demolished
                    && !x.derelict
                    && d <= r
                    && crate::systems::company::open_to_all(world, b)
                    && world.residents_of(b).len() + need <= usize::from(x.capacity)
                    && !matches!(status_at(world, x.door), Status::Contested(..))
                    && purse >= crate::systems::ownership::rent_for(world, b);
                ok.then_some((d, b.index))
            });
            let Some((_, _, to)) = pick.filter(|&(d, _, _)| d <= r) else { continue };
            let at = around(world, k);
            let (an, bn, place) = (gname(world, ga), gname(world, gb), world.name_of(to));
            let mover = world.name_of(a);
            for m in std::iter::once(a).chain(spouse) {
                world.set_home(m, Some(to));
                if let Some(x) = world.comp_mut::<Household>(m) {
                    x.rent_due = x.rent_due.min(0.0);
                }
                let name = world.name_of(m);
                let text = if m == a {
                    format!("{name} moved the children out of {at} ({an} and {bn} at odds there), into {place}")
                } else {
                    format!("{name} moved with {mover} and the children, into {place}")
                };
                world.push_event(EventKind::Housed, &[m, to], text);
            }
            let tdoor = door_of(world, to).unwrap_or(door);
            let tick = world.tick;
            for ch in kids {
                world.set_home(ch, Some(to));
                world.remove_from_building(ch);
                if let Some(p) = world.comp_mut::<Position>(ch) {
                    p.tile = tdoor;
                    p.building = None;
                    p.floor = 0;
                    p.entered = tick;
                }
                world.enter_building(ch, to);
            }
            moved += 1;
            crate::systems::ownership::jobs2_row(world).turf_family_moves += 1;
        }
    }
}
