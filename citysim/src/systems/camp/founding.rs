use super::*;

/// E37 (midnight): when every standing Camp is at least `found_full` full,
/// or a taken child had no place in the last 7 days, the richest Tech or
/// Food corp with `3 × found_cost.camp` builds one (a Lot near its
/// buildings, else a derelict); not a corp order.
pub fn found_daily(world: &mut World) {
    let camps = standing_camps(world);
    let full = world.config.camp.found_full;
    let all_full = !camps.is_empty()
        && camps.iter().all(|&c| {
            let cap = capacity(world, c).max(1) as f32;
            children_of(world, c).len() as f32 >= full * cap
        });
    let day = world.day();
    let overflow = camps
        .iter()
        .any(|&c| state(world, c).and_then(|s| s.last_overflow_day).is_some_and(|d| day.saturating_sub(d) <= 7));
    if !all_full && !overflow {
        return;
    }
    let cost = world.config.corps.found_cost.camp;
    let pick = world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (cc.treasury, c, cc)))
        .filter(|(t, _, cc)| *t >= 3 * cost && (cc.niches.contains(&Niche::Tech) || cc.niches.contains(&Niche::Food)))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, c, _)| c);
    let Some(corp) = pick else { return };
    let from = world
        .comp::<Corp>(corp)
        .and_then(|c| c.buildings.first().copied())
        .and_then(|b| door(world, b))
        .unwrap_or_else(|| vats_centroid(world));
    let Some((site, on_lot)) = site_near(world, from) else { return };
    let Some(b) = open_camp(world, site, on_lot, Some(corp)) else { return };
    let paid = crate::systems::industry::pay_build(world, Some(corp), cost, Some(b), true);
    let (who, at) = (world.owner_label(Some(corp)), world.district_name(world.district_of_building(b)).to_string());
    world.push_event(
        EventKind::Founded,
        &[corp, b],
        format!("{who} built a Work Camp in {at} for {paid} (a work camp)"),
    );
    world.stats.current.foundings += 1;
}

// ---------------------------------------------------------------------------
// The service (E38)
// ---------------------------------------------------------------------------

/// The living parents of `child` (Parent edges to someone born earlier), ascending.
pub fn parents_of(world: &World, child: EntityId) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = world
        .neighbours(child)
        .filter(|&o| world.has::<Identity>(o) && crate::systems::law::living(world, o))
        .filter(|&o| crate::systems::demography::is_child_of(world, child, o))
        .collect();
    out.sort_unstable();
    out
}

/// The standing Market nearest `from` with at least `units` on its shelf
/// (ties the lower id); the city's kitchens buy from any Market with stock.
pub(super) fn market_with_stock(world: &World, from: TilePos, units: u32) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter_map(|m| {
            let bd = world.comp::<Building>(m).filter(|bd| !bd.demolished && !bd.derelict)?;
            (bd.stock_food >= units.max(1)).then(|| (bd.door.manhattan(from), m))
        })
        .min()
        .map(|(_, m)| m)
}

/// Review fix (E38, E39): can `camp` feed its children plus `extra`
/// tonight: the shelf covers their rations, or the owner can buy the
/// shortfall (the city's purse is unbounded) from a Market with stock.
pub fn can_feed_tonight(world: &World, camp: EntityId, extra: usize) -> bool {
    let n = (children_of(world, camp).len() + extra) as u32;
    let need = n.div_ceil(2);
    let stock = world.comp::<Building>(camp).map_or(0, |b| b.stock_food);
    if stock >= need {
        return true;
    }
    let short = need - stock;
    let price = world.config.corps.wholesale.max(1);
    let owner = world.owner_of(camp);
    let afford = match ownership::owner_kind(world, owner) {
        ownership::OwnerKind::City => true,
        _ => world.purse(owner).max(0) >= i64::from(short) * price,
    };
    afford && door(world, camp).is_some_and(|d| market_with_stock(world, d, short).is_some())
}

/// The camp a take goes to: among the open camps with room, one that can
/// feed tonight first (review fix: a taken child arrives at `take_days`
/// unfed; S6: hungry, never killed), then
/// corp camps before the city's, then the nearest (ties the lower id);
/// else the city camp over capacity (the first camp when none is the
/// city's); `(camp, over)`.
pub(super) fn placement(world: &World, from: TilePos, parent: Option<EntityId>) -> Option<(EntityId, bool)> {
    let camps = standing_camps(world);
    let with_room = camps
        .iter()
        .copied()
        .filter(|&c| open(world, c) && children_of(world, c).len() < capacity(world, c))
        .filter_map(|c| {
            door(world, c).map(|d| {
                (
                    !can_feed_tonight(world, c, 1),
                    !in_reach(world, parent, from, d),
                    is_city(world, c),
                    d.manhattan(from),
                    c,
                )
            })
        })
        .min()
        .map(|(_, _, _, _, c)| c);
    if let Some(c) = with_room {
        return Some((c, false));
    }
    let city = camps.iter().copied().filter(|&c| open(world, c)).find(|&c| is_city(world, c));
    city.or_else(|| camps.iter().copied().find(|&c| open(world, c))).map(|c| (c, true))
}

/// Round 4 T1 (OPEN-231): a Camp door within `[distance.choice]
/// visit_ticks` of the child's Home by the parent's trip (S1's
/// `distance::trip_ahead`, the week's visit as `household::camp_too_far`
/// reads it). True with no parent or the choice off.
fn in_reach(world: &World, parent: Option<EntityId>, from: TilePos, to: TilePos) -> bool {
    let (Some(p), Some(c)) = (parent, crate::systems::distance::choice(world)) else { return true };
    crate::systems::distance::trip_ahead(world, p, from, to) <= c.visit_ticks
}

/// E38: a child moves into `camp`: its Household, the camp's lists, its
/// position inside.
pub(super) fn house(world: &mut World, child: EntityId, camp: EntityId) {
    let day = world.day();
    world.set_home(child, Some(camp));
    // Round 5 U2 (`camp_parent_23527 d25 09:30`): the Camp's routine from the
    // take (the day's was picked at midnight before it).
    if let Some(c) = world.comp_mut::<Child>(child) {
        c.routine = crate::components::Routine::Camp;
        c.school = None;
        c.school_miss = None;
    }
    world.remove_from_building(child);
    let tick = world.tick;
    let d = door(world, camp).unwrap_or_default();
    if let Some(p) = world.comp_mut::<Position>(child) {
        p.tile = d;
        p.building = Some(camp);
        p.entered = tick;
    }
    if let Some(s) = state_mut(world, camp) {
        if let Err(i) = s.children.binary_search(&child) {
            s.children.insert(i, child);
        }
        if !s.arrived.iter().any(|(c, _)| *c == child) {
            s.arrived.push((child, day));
        }
        if !s.fed_by_child.iter().any(|(c, _)| *c == child) {
            s.fed_by_child.push((child, 0));
        }
    }
}

/// Out of the camp's lists (the Household is the caller's).
pub(super) fn unhouse(world: &mut World, child: EntityId, camp: EntityId) -> (u32, u32) {
    let day = world.day();
    let mut days = 0;
    let mut fed = 0;
    if let Some(s) = state_mut(world, camp) {
        s.children.retain(|&c| c != child);
        if let Some(i) = s.arrived.iter().position(|(c, _)| *c == child) {
            days = day.saturating_sub(s.arrived[i].1) as u32;
            s.arrived.remove(i);
        }
        if let Some(i) = s.fed_by_child.iter().position(|(c, _)| *c == child) {
            fed = s.fed_by_child[i].1;
            s.fed_by_child.remove(i);
        }
    }
    (days, fed)
}

/// E38 (`demography::run`, after `children`): every `Child` in a Block
/// with `hunger_days ≥ take_days` is on the ladder, ascending id (round 4
/// T1, OPEN-231: the first neglect a warning at the door, a repeat inside
/// `warn_repeat_days` past `warn_grace_days` the take); before it the
/// Camp children whose parent's pantry has held are let home.
pub fn take_daily(world: &mut World) {
    let cfg = world.config.camp.clone();
    let take_days = cfg.take_days;
    for c in standing_camps(world) {
        prune(world, c);
    }
    let_home_daily(world);
    follow_parents(world);
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    let window = u32::from(cfg.warn_repeat_days);
    // Round 6 V1 (OPEN-242): children out of care are found and returned.
    fetch_daily(world, day);
    street_daily(world, day, window);
    jailed_households(world);
    // Round 5 U2 (OPEN-231, Dylan 2026-10-28): a warning with no free parent
    // at home takes at once: a child on an open warning whose every parent
    // is jailed, held, downed or gone (fed or not tonight).
    if window > 0 {
        let mut open: Vec<(EntityId, EntityId)> = world
            .with::<Child>()
            .into_iter()
            .filter(|&c| {
                world.comp::<Child>(c).is_some_and(|k| {
                    k.hunger_days < take_days && k.warned_day.is_some_and(|w| day.saturating_sub(w) <= window)
                })
            })
            .filter_map(|c| {
                let h = world.comp::<Household>(c).and_then(|h| h.home)?;
                world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Home).then_some((c, h))
            })
            .collect();
        open.sort_unstable();
        for (c, h) in open {
            if free_parents_at(world, c, h).is_empty() {
                let why = no_parent_why(world, c, Some(h));
                if take_why(world, c, &format!("on the service's warning and {why}")).is_some() {
                    ownership::jobs2_row(world).camp_take_no_parent += 1;
                }
            }
        }
    }
    let mut due: Vec<EntityId> = world
        .with::<Child>()
        .into_iter()
        .filter(|&c| world.comp::<Child>(c).is_some_and(|k| k.hunger_days >= take_days))
        .filter(|&c| {
            world
                .comp::<Household>(c)
                .and_then(|h| h.home)
                .is_none_or(|h| world.comp::<Building>(h).is_some_and(|b| b.kind != BuildingKind::Camp))
        })
        .collect();
    due.sort_unstable();
    // Round 6 V1 (OPEN-231, `owner_broke_6426 d21 00:10`: eighteen children
    // of ten families in one line): a warning is per household at the door.
    let mut warns: std::collections::BTreeMap<(EntityId, EntityId), Vec<EntityId>> = std::collections::BTreeMap::new();
    for c in due {
        let home = world.comp::<Household>(c).and_then(|h| h.home);
        let warned = world.comp::<Child>(c).and_then(|k| k.warned_day);
        match (home, warned) {
            // No door to come to (a street child), or the ladder off: the take.
            (None, _) => {
                take(world, c);
            }
            _ if window == 0 => {
                take(world, c);
            }
            // Round 5 U2: a neglect with no free parent at home takes at
            // once (no door to warn: nobody free behind it).
            (Some(h), _) if free_parents_at(world, c, h).is_empty() => {
                let why = no_parent_why(world, c, Some(h));
                let unfed = world.comp::<Child>(c).map_or(0, |k| k.hunger_days);
                if take_why(world, c, &format!("{unfed} days unfed and {why}")).is_some() {
                    ownership::jobs2_row(world).camp_take_no_parent += 1;
                }
            }
            // A second neglect inside `warn_repeat_days` of the warning takes
            // (past the grace).
            (Some(_), Some(w)) if day.saturating_sub(w) <= window => {
                if day.saturating_sub(w) >= u32::from(cfg.warn_grace_days) {
                    let unfed = world.comp::<Child>(c).map_or(0, |k| k.hunger_days);
                    if take_why(world, c, &format!("unfed {unfed} days again, inside the warning of d{w}")).is_some() {
                        ownership::jobs2_row(world).camp_take_repeat += 1;
                    }
                }
            }
            (Some(h), _) => warns.entry((h, crate::systems::shelf::key(world, c))).or_default().push(c),
        }
    }
    for ((h, _), kids) in warns {
        warn(world, h, &kids, day);
    }
}
