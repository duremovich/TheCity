use super::*;

/// Lives at work phase 3 (§ 2.2, camp warden; civic only, midnight): an
/// escape on `p_camp_escape` (`release(Escaped)`, then the first living
/// parent's Home with room, else the street as `release` leaves it).
/// Hashed on (child, day): no world-stream draw. The pressure pass S6
/// (Dylan 2026-10-27, no child is harmed): the warden's beating is gone.
pub(super) fn warden_rolls(world: &mut World) {
    let cfg = world.config.service.clone();
    let day = world.day();
    let kids: Vec<(EntityId, EntityId)> = standing_camps(world)
        .into_iter()
        .flat_map(|c| children_of(world, c).iter().map(move |&k| (k, c)).collect::<Vec<_>>())
        .collect();
    for (k, camp) in kids {
        if !world.is_alive(k) || world.has::<crate::components::Corpse>(k) {
            continue;
        }
        if hash(world, PURPOSE_ESCAPE, key(k), day) < cfg.p_camp_escape {
            escape(world, k, camp, "");
        }
    }
}

/// A Camp child out and away: `release(Escaped)`, then the first living
/// parent's Home, else the street; the event (`how` after the Camp's name).
pub(super) fn escape(world: &mut World, k: EntityId, camp: EntityId, how: &str) {
    release(world, k, ReleaseWhy::Escaped);
    // Round 6 V1 (OPEN-242): the service fetches it back the next midnight.
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    if let Some(c) = world.comp_mut::<Child>(k) {
        c.slipped = Some((camp, day));
    }
    let home = parents_of(world, k).into_iter().find_map(|p| send_home(world, k, p));
    let (name, what) = (world.name_of(k), world.name_of(camp));
    let to = home.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
    let mut actors = vec![k, camp];
    actors.extend(home);
    world.push_event(EventKind::CampEscape, &actors, format!("{name} slipped out of {what}{how} to {to}"));
    ownership::jobs2_row(world).camp_escapes += 1;
}

/// Round 4 T1 (OPEN-231, the warden's day; civic only, hourly from
/// `service::civic_hour`): a Camp child's try at the fence on `p_abscond`
/// at its hashed hour inside `abscond_hours`; a warden on duty there
/// catches it on `p_abscond_caught` and walks it back in (no blow, no
/// fight: the under-18 rule; the child stays), else it gets away
/// (`escape`). Hashed on (child, day): no world-stream draw.
pub fn abscond_hour(world: &mut World) {
    let cfg = world.config.camp.clone();
    let [from, to] = cfg.abscond_hours;
    if cfg.p_abscond <= 0.0 || to <= from {
        return;
    }
    let day = world.day();
    let hour = (u64::from(world.tick_of_day()) / crate::time::TICKS_PER_HOUR) as u8;
    if !(from..to).contains(&hour) {
        return;
    }
    let span = f32::from(to - from);
    let mut tries: Vec<(EntityId, EntityId)> = Vec::new();
    for camp in standing_camps(world) {
        for &k in children_of(world, camp) {
            let h = from + ((hash(world, PURPOSE_ABSCOND_HOUR, key(k), day) * span) as u8).min(to - from - 1);
            if h == hour && hash(world, PURPOSE_ABSCOND, key(k), day) < cfg.p_abscond {
                tries.push((k, camp));
            }
        }
    }
    for (k, camp) in tries {
        if !world.is_alive(k) || world.has::<crate::components::Corpse>(k) {
            continue;
        }
        let wardens = crate::systems::service::servers_on_duty(world, camp, crate::components::ServeKind::Inmate);
        let w = wardens.first().copied();
        if let Some(w) = w.filter(|_| hash(world, PURPOSE_CAUGHT, key(k), day) < cfg.p_abscond_caught) {
            let (wn, cn, what) = (world.name_of(w), world.name_of(k), world.name_of(camp));
            let age = world.comp::<Identity>(k).map_or(0, |i| i.age_years());
            world.push_event(
                EventKind::Custody,
                &[w, k, camp],
                format!("{wn} caught {cn} ({age} y) at the fence of {what} and walked them back in"),
            );
            note_move(world, camp, k, CampMove::Caught);
            ownership::jobs2_row(world).camp_caught += 1;
        } else {
            let how = match w {
                Some(w) => format!(" past {} in the day", world.name_of(w)),
                None => " in the day, no warden on the gate,".to_string(),
            };
            escape(world, k, camp, &how);
        }
    }
}

/// Round 4 T1 (OPEN-231, the warden's day): the headcount at the first
/// hour of `wardens`' shift, one `Custody` line: the children counted, the
/// comings and goings since the last count (by name, three a kind), last
/// night's meals (the unfed, the Reserve's share). The ledger cleared.
pub fn headcount(world: &mut World, camp: EntityId, wardens: &[EntityId]) {
    if wardens.is_empty() {
        return;
    }
    let n = children_of(world, camp).len();
    let (moves, over, unfed, reserve) = match state_mut(world, camp) {
        Some(s) => (
            std::mem::take(&mut s.since_count),
            std::mem::take(&mut s.since_over),
            s.unfed_days.back().copied().unwrap_or(0),
            s.reserve_today,
        ),
        None => return,
    };
    let kinds = [
        (CampMove::Taken, "brought in"),
        (CampMove::Escaped, "slipped out"),
        (CampMove::Caught, "caught at the fence"),
        (CampMove::LetHome, "let home"),
        (CampMove::BoughtOut, "bought out"),
        (CampMove::BrokenOut, "broken out"),
        (CampMove::CameOfAge, "came of age and left"),
        (CampMove::FetchedBack, "fetched back"),
    ];
    let mut bits: Vec<String> = Vec::new();
    for (m, label) in kinds {
        let who: Vec<EntityId> = moves.iter().filter(|(_, x)| *x == m).map(|(c, _)| *c).collect();
        let extra = over.iter().find(|(k, _)| *k == m).map_or(0, |t| t.1 as usize);
        if who.is_empty() && extra == 0 {
            continue;
        }
        let total = who.len() + extra;
        let mut names: Vec<String> = who.iter().take(3).map(|&c| world.name_of(c)).collect();
        if total > 3 {
            names.push(format!("{} more", total - 3));
        }
        bits.push(format!("{total} {label} ({})", names.join(", ")));
    }
    let since = if bits.is_empty() {
        "; nobody in or out since the last count".to_string()
    } else {
        format!("; since the last count {}", bits.join(", "))
    };
    let meals = match (unfed, reserve) {
        (0, 0) => "; every child ate last night".to_string(),
        (0, r) => format!("; every child ate last night, {r} of the meals from the Reserve"),
        (u, _) => format!("; {u} went without last night"),
    };
    let mut names: Vec<String> = wardens.iter().take(2).map(|&x| world.name_of(x)).collect();
    if wardens.len() > 2 {
        names.push(format!("{} other wardens", wardens.len() - 2));
    }
    let text = format!("{} counted {n} children at {}{since}{meals}", names.join(" and "), world.name_of(camp));
    let mut actors = wardens.to_vec();
    actors.push(camp);
    world.push_event(EventKind::Custody, &actors, text);
}

/// Lives at work phase 3: a released child goes home to `parent`'s Home
/// when it stands: its Household and its body there. `None` with no such
/// Home (the street, as `release` leaves it). Deviation from the brief's
/// "with room": a child is its parents' dependant and never counted against
/// the flat's capacity (`demography::spawn_child` houses a newborn the same
/// way; most pairs live in a flat of two, so a room check sent every
/// returning child to the street).
pub fn send_home(world: &mut World, child: EntityId, parent: EntityId) -> Option<EntityId> {
    let h = world.comp::<Household>(parent).and_then(|x| x.home)?;
    let bd = world.comp::<Building>(h)?;
    if bd.demolished || bd.derelict {
        return None;
    }
    let d = bd.door;
    let tick = world.tick;
    world.set_home(child, Some(h));
    world.remove_from_building(child);
    if let Some(p) = world.comp_mut::<Position>(child) {
        p.tile = d;
        p.building = Some(h);
        p.entered = tick;
    }
    crate::systems::floors::settle(world, child);
    // Round 5 U2: at home from tonight (the midnight pick sets tomorrow's).
    if let Some(c) = world.comp_mut::<Child>(child) {
        if c.routine == crate::components::Routine::Camp {
            c.routine = crate::components::Routine::Play;
        }
    }
    Some(h)
}

pub(super) fn reopen(world: &mut World, camp: EntityId, now: u64) {
    let closed = state(world, camp).is_some_and(|s| s.closed_by_law);
    let until = world.comp::<Building>(camp).and_then(|b| b.closed_until);
    if closed && until.is_none_or(|t| t <= now) {
        if let Some(s) = state_mut(world, camp) {
            s.closed_by_law = false;
            s.scandal_days = 0;
        }
    }
}

/// E39: the owner buys from the nearest Market at `wholesale` up to
/// `stock_cap` and its purse; the city's Camp from the Treasury
/// (`Flow::CampFood`, the Jail meal's shape).
pub(super) fn restock(world: &mut World, camp: EntityId) {
    let n = children_of(world, camp).len() as u32;
    if n == 0 {
        return;
    }
    let Some(cd) = door(world, camp) else { return };
    let cap = world.config.buildings.for_kind(BuildingKind::Camp).stock_cap;
    let stock = world.comp::<Building>(camp).map_or(0, |b| b.stock_food);
    // A week of meals is enough; the cap bounds it.
    let want = (n.div_ceil(2) * 7).min(cap).saturating_sub(stock);
    if want == 0 {
        return;
    }
    // Review fix: the nearest Market with stock (an empty shelf next door
    // is skipped; nothing in the city: nothing bought, no `CampFood` line).
    let Some(market) = market_with_stock(world, cd, 1) else { return };
    let price = world.config.corps.wholesale.max(1);
    let have = world.comp::<Building>(market).map_or(0, |b| b.stock_food);
    let owner = world.owner_of(camp);
    let afford = match ownership::owner_kind(world, owner) {
        ownership::OwnerKind::City => u32::MAX,
        _ => u32::try_from(world.purse(owner).max(0) / price).unwrap_or(u32::MAX),
    };
    let units = want.min(have).min(afford);
    if units == 0 {
        return;
    }
    let seller = world.owner_of(market);
    let amount = i64::from(units) * price;
    let paid = if owner.is_none() {
        let paid = ownership::charge(world, None, seller, amount, Flow::CampFood);
        if seller.is_none() {
            // The city buying from its own Market: no coin moves, the line shows.
            ownership::ledger_only(world, Flow::CampFood, amount);
            amount
        } else {
            paid
        }
    } else {
        ownership::pay(world, owner, seller, amount, Flow::Wholesale)
    };
    if paid <= 0 {
        return;
    }
    let bought = u32::try_from(paid / price).unwrap_or(0).min(units);
    world.take_stock(market, crate::components::Good::Food, bought);
    ownership::credit(world, market, paid);
    if let Some(b) = world.comp_mut::<Building>(camp) {
        b.stock_food += bought;
    }
}
