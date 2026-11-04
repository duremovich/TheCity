use super::*;

/// Round 9 Z3 (OPEN-258, Dylan 2026-10-31: "kids who have both parents
/// jailed should go to camp"): midnight, a child under 18 living at a Home
/// whose every adult is in the cells (a `Sentence`) is taken the same day,
/// by the neglect ladder's take, with no unfed grace. The adults are the
/// child's living parents, else the adults of the household the Home names
/// (the guardians). Nobody to hold (no adult at all) is not this rule.
pub(super) fn jailed_households(world: &mut World) {
    let mut kids: Vec<(EntityId, EntityId)> = world
        .with::<Child>()
        .into_iter()
        .filter_map(|c| {
            let h = world.comp::<Household>(c).and_then(|h| h.home)?;
            world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Home).then_some((c, h))
        })
        .collect();
    kids.sort_unstable();
    for (c, h) in kids {
        let mut adults: Vec<EntityId> =
            parents_of(world, c).into_iter().filter(|&p| crate::systems::demography::is_adult(world, p)).collect();
        if adults.is_empty() {
            adults = world
                .residents_of(h)
                .iter()
                .copied()
                .filter(|&r| {
                    r != c
                        && world.has::<Identity>(r)
                        && crate::systems::law::living(world, r)
                        && crate::systems::demography::is_adult(world, r)
                })
                .collect();
        }
        if adults.is_empty() || !adults.iter().all(|&p| world.has::<crate::components::Sentence>(p)) {
            continue;
        }
        take_why(world, c, "both parents in the cells");
    }
}

/// Round 6 V1 (OPEN-242, `camp_jobless_61284`: a 2-year-old slipped out on
/// d24, at his father's Block, then on the street to d27): midnight, the
/// child who slipped out of a Camp before today is fetched back by the
/// service (a warden of the Camp named with it): walked back in, no force
/// (the under-18 rule: a protection, nothing calls `law`). Into the same
/// Camp when it stands open, else the take's placement.
pub(super) fn fetch_daily(world: &mut World, day: u32) {
    let mut out: Vec<(EntityId, EntityId, u32)> = world
        .with::<Child>()
        .into_iter()
        .filter_map(|c| world.comp::<Child>(c).and_then(|k| k.slipped).map(|(camp, d)| (c, camp, d)))
        .filter(|&(_, _, d)| d < day)
        .collect();
    out.sort_unstable();
    for (c, camp, d) in out {
        if let Some(k) = world.comp_mut::<Child>(c) {
            k.slipped = None;
        }
        if !world.is_alive(c) || world.has::<crate::components::Corpse>(c) {
            continue;
        }
        let home = world.comp::<Household>(c).and_then(|h| h.home);
        if home.is_some_and(|h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp)) {
            continue;
        }
        let from = home.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
        let what = world.name_of(camp);
        let back = standing_camps(world).contains(&camp) && open(world, camp);
        if !back {
            if take_why(world, c, &format!("fetched after slipping out of {what} on d{d}")).is_some() {
                ownership::jobs2_row(world).camp_fetched += 1;
            }
            continue;
        }
        house(world, c, camp);
        note_move(world, camp, c, CampMove::FetchedBack);
        let warden = world.staff_of(camp).iter().copied().find(|&w| {
            world.is_alive(w)
                && world.comp::<crate::components::Job>(w).is_some_and(|j| j.employer == Some(camp))
                && free_parent(world, w)
        });
        let yrs = world.comp::<Identity>(c).map_or(0, |i| i.age_years());
        let who = match warden {
            Some(w) => format!("{} of {what} and the service", world.name_of(w)),
            None => "the service".to_string(),
        };
        let text =
            format!("{who} fetched {} ({yrs} y) back to {what} from {from}: slipped out on d{d}", world.name_of(c));
        let mut actors = vec![c, camp];
        actors.extend(warden);
        actors.extend(parents_of(world, c));
        actors.extend(home);
        world.push_event(EventKind::Custody, &actors, text);
        ownership::jobs2_row(world).camp_fetched += 1;
    }
}

/// Round 6 V1 (OPEN-242, `miserable_12664 d21-23`: a 2-year-old on the
/// street three mornings, on a warning, her parents held): midnight, a
/// child on the street (no Home, not in a Camp) goes to live with a free
/// parent who has a standing Home; with no free parent at all, or on an
/// open warning (the street is no door to come back to), the service takes
/// it. A homeless family with a free parent and no warning keeps the
/// `[ages] street_take_days` rule.
pub(super) fn street_daily(world: &mut World, day: u32, window: u32) {
    let mut kids: Vec<EntityId> = world
        .with::<Child>()
        .into_iter()
        .filter(|&c| world.is_alive(c) && !world.has::<crate::components::Corpse>(c))
        .filter(|&c| world.comp::<Household>(c).is_some_and(|h| h.home.is_none()))
        .collect();
    kids.sort_unstable();
    for c in kids {
        let free: Vec<EntityId> = parents_of(world, c).into_iter().filter(|&p| free_parent(world, p)).collect();
        let to = free.iter().copied().find(|&p| {
            world.comp::<Household>(p).and_then(|x| x.home).is_some_and(|ph| {
                world.comp::<Building>(ph).is_some_and(|b| b.kind == BuildingKind::Home && !b.demolished && !b.derelict)
            })
        });
        if let Some(p) = to {
            let Some(h) = send_home(world, c, p) else { continue };
            if let Some(k) = world.comp_mut::<Child>(c) {
                k.hunger_days = 0;
            }
            let yrs = world.comp::<Identity>(c).map_or(0, |i| i.age_years());
            let text = format!(
                "{} ({yrs} y) went to live with {} at {}: off the street",
                world.name_of(c),
                world.name_of(p),
                world.name_of(h)
            );
            world.push_event(EventKind::Housed, &[p, c, h], text);
            ownership::jobs2_row(world).camp_off_street += 1;
            continue;
        }
        let warned_on = world.comp::<Child>(c).and_then(|k| k.warned_day).filter(|&w| day.saturating_sub(w) <= window);
        let why = if free.is_empty() {
            format!("on the street and {}", no_parent_why(world, c, None))
        } else if let Some(w) = warned_on.filter(|_| window > 0) {
            let names: Vec<String> = free.iter().map(|&p| world.name_of(p)).collect();
            format!("on the service's warning of d{w} and on the street ({} with no Home)", names.join(", "))
        } else {
            continue;
        };
        if take_why(world, c, &why).is_some() {
            ownership::jobs2_row(world).camp_take_street += 1;
        }
    }
}

/// Round 5 U2 (OPEN-231): `p` is a free parent: alive, adult, not
/// sentenced, not in cuffs (held, or carried), not downed or laid up
/// (`World::wounded`), not leaving the city.
pub fn free_parent(world: &World, p: EntityId) -> bool {
    crate::systems::law::living(world, p)
        && crate::systems::demography::is_adult(world, p)
        && !world.has::<crate::components::Sentence>(p)
        && !world.wounded.contains_key(&p)
        && world.comp::<crate::components::Brain>(p).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// Round 5 U2: the free parents of `child` living at `home`, ascending.
pub fn free_parents_at(world: &World, child: EntityId, home: EntityId) -> Vec<EntityId> {
    parents_of(world, child)
        .into_iter()
        .filter(|&p| free_parent(world, p) && world.comp::<Household>(p).and_then(|h| h.home) == Some(home))
        .collect()
}

/// Round 5 U2: why no parent of `child` is free at `home` (the take's
/// line): each parent and where he is, or "no parent living".
pub(super) fn no_parent_why(world: &World, child: EntityId, home: Option<EntityId>) -> String {
    let ps = parents_of(world, child);
    if ps.is_empty() {
        return "no parent living".to_string();
    }
    let bits: Vec<String> = ps
        .iter()
        .map(|&p| {
            let n = world.name_of(p);
            if world.has::<crate::components::Sentence>(p) {
                format!("{n} in the cells")
            } else if world.wounded.contains_key(&p) {
                format!("{n} laid up")
            } else if world.comp::<crate::components::Brain>(p).is_some_and(|b| b.cuffed_by.is_some()) {
                format!("{n} held")
            } else if !crate::systems::demography::is_adult(world, p) {
                format!("{n} a child")
            } else if home.is_some() && world.comp::<Household>(p).and_then(|h| h.home) != home {
                format!("{n} not living there")
            } else {
                format!("{n} leaving the city")
            }
        })
        .collect();
    format!("no free parent at home ({})", bits.join(", "))
}

/// Round 5 U2 (OPEN-231, `reporter_57837`: a child unfed at a Block his
/// mother had left, her pantry full at hers): midnight, before the ladder,
/// a child living (not in a Camp, not on the street) where no free parent
/// of his lives goes to live with his first free parent who has a standing
/// Home (`send_home`: a dependant, never counted against the flat). A
/// marriage, a swap or a move that took a parent and left the child is
/// mended here; a parent in the cells keeps the Home, so the child goes to
/// the other one if he is free elsewhere.
pub(super) fn follow_parents(world: &mut World) {
    let mut moves: Vec<(EntityId, EntityId, EntityId)> = Vec::new();
    for c in world.with::<Child>() {
        let Some(h) = world.comp::<Household>(c).and_then(|x| x.home) else { continue };
        if !world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Home) {
            continue;
        }
        if !world.is_alive(c) || world.has::<crate::components::Corpse>(c) || !free_parents_at(world, c, h).is_empty() {
            continue;
        }
        let to = parents_of(world, c).into_iter().find(|&p| {
            free_parent(world, p)
                && world.comp::<Household>(p).and_then(|x| x.home).filter(|&ph| ph != h).is_some_and(|ph| {
                    world
                        .comp::<Building>(ph)
                        .is_some_and(|b| b.kind == BuildingKind::Home && !b.demolished && !b.derelict)
                })
        });
        if let Some(p) = to {
            moves.push((c, p, h));
        }
    }
    moves.sort_unstable();
    for (c, p, from) in moves {
        let why = no_parent_why(world, c, Some(from));
        let Some(to) = send_home(world, c, p) else { continue };
        // Round 6 V1 (`camp_jobless_25426 d27 00:10`: the father warned the
        // night his son came, over days unfed at the mother's door): the
        // unfed count starts again at the new door.
        if let Some(k) = world.comp_mut::<Child>(c) {
            k.hunger_days = 0;
        }
        let yrs = world.comp::<Identity>(c).map_or(0, |i| i.age_years());
        let text = format!(
            "{} ({yrs} y) went to live with {} at {}: {why} at {}",
            world.name_of(c),
            world.name_of(p),
            world.name_of(to),
            world.name_of(from)
        );
        world.push_event(EventKind::Housed, &[p, c, to], text);
        ownership::jobs2_row(world).camp_followed += 1;
    }
}
