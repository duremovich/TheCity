use super::*;

// ---------------------------------------------------------------------------
// The faith on screen
// ---------------------------------------------------------------------------

/// The place's service day this week (a hash of the place, a weekday) and hour.
fn service_slot(world: &World, c: &ConsequenceCfg, place: EntityId) -> (u64, u8) {
    let u = hash(world, P_SVC_DAY, place, 0);
    let wd = ((u * 7.0) as u64).min(6);
    let span = c.svc_hour_to.saturating_sub(c.svc_hour_from).max(1);
    let v = hash(world, P_SVC_DAY, place, 1);
    (wd, c.svc_hour_from + ((v * f32::from(span)) as u8).min(span - 1))
}

pub(super) fn services_daily(world: &mut World, c: &ConsequenceCfg) {
    world.consequence.services.clear();
    world.consequence.guests.clear();
    if !crate::systems::household::on(world) {
        return;
    }
    let today = world.day();
    // The creeds' gangs (a cult row, or the Purists) at their chapels.
    for g in world.gang_list().to_vec() {
        let Some(gang) = world.comp::<Gang>(g) else { continue };
        let Some(creed) = gang.creed else { continue };
        let place = gang.hideout;
        if place == EntityId::NONE
            || gang.is_sacked(world.tick)
            || world.comp::<Building>(place).is_none_or(|b| b.demolished || b.derelict)
        {
            continue;
        }
        let (wd, hour) = service_slot(world, c, place);
        if today % 7 != wd {
            continue;
        }
        let members: Vec<EntityId> = gang.members.iter().copied().filter(|&m| free(world, m)).collect();
        if members.is_empty() {
            continue;
        }
        let preacher = gang.leader.filter(|&l| free(world, l)).or_else(|| members.first().copied());
        let (label, row) = match creed {
            Creed::Config(i) => (crate::systems::faith::label(world, i), Some(i)),
            Creed::Purist => (gang.name.clone(), None),
        };
        call(world, c, place, preacher, label, Some(g), row, hour, &members);
    }
    // The churches: the followers of their district.
    if crate::systems::faith::on(world) {
        let churches: Vec<(EntityId, u8)> = world.jobs2.faith.churches.iter().map(|(&b, &i)| (b, i)).collect();
        for (b, i) in churches {
            if world.comp::<Building>(b).is_none_or(|x| x.demolished) {
                continue;
            }
            let (wd, hour) = service_slot(world, c, b);
            if today % 7 != wd {
                continue;
            }
            let d = world.district_of_building(b);
            let flock: Vec<EntityId> = world
                .jobs2
                .faith
                .followers
                .iter()
                .filter(|(_, &x)| x == i)
                .map(|(&a, _)| a)
                .filter(|&a| crate::systems::gossip::home_district(world, a) == Some(d) && free(world, a))
                .collect();
            if flock.is_empty() {
                continue;
            }
            // The eldest of the flock not called elsewhere today preaches (a
            // lay preacher; ties the lower id): a district with two churches
            // has two preachers.
            let preacher = flock
                .iter()
                .copied()
                .filter(|a| !world.visits.contains_key(a))
                .max_by_key(|&a| (world.comp::<Identity>(a).map_or(0, |x| x.age_years()), std::cmp::Reverse(a)));
            let label = crate::systems::faith::label(world, i);
            call(world, c, b, preacher, label, None, Some(i), hour, &flock);
        }
    }
}

/// Call the congregation (and the preacher, first) to `place` for today.
#[allow(clippy::too_many_arguments)]
fn call(
    world: &mut World,
    c: &ConsequenceCfg,
    place: EntityId,
    preacher: Option<EntityId>,
    creed: String,
    gang: Option<EntityId>,
    row: Option<u8>,
    hour: u8,
    flock: &[EntityId],
) {
    let visit_hour = hour.saturating_sub(c.svc_lead_hours);
    let day = world.day();
    let mut called = 0u16;
    let mut order: Vec<EntityId> = Vec::with_capacity(flock.len());
    order.extend(preacher);
    order.extend(flock.iter().copied().filter(|&a| Some(a) != preacher));
    let door = world.comp::<Building>(place).map(|b| b.door);
    for a in order {
        if usize::from(called) >= c.svc_cap {
            break;
        }
        if world.visits.contains_key(&a) {
            continue;
        }
        // Past the reach the walk would eat the evening (the preacher goes anyway).
        let far = |world: &World, a: EntityId| {
            let from = crate::systems::distance::set_out(world, a);
            from.zip(door).is_some_and(|(f, d)| crate::systems::distance::travel(world, f, d) > c.svc_reach_ticks)
        };
        if Some(a) != preacher && far(world, a) {
            continue;
        }
        world.visits.insert(a, PendingVisit { kin: place, why: VisitWhy::Service, hour: visit_hour });
        called += 1;
        // A congregant brings an unchurched friend.
        if Some(a) != preacher && hash(world, P_GUEST, a, day) < c.svc_guest_p && usize::from(called) < c.svc_cap {
            let mut near: Vec<(f32, EntityId)> = world
                .neighbours(a)
                .filter_map(|o| {
                    world
                        .edge(a, o)
                        .filter(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse))
                        .map(|e| (e.affinity, o))
                })
                .collect();
            near.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
            let guest = near.into_iter().map(|x| x.1).find(|&o| {
                free(world, o)
                    && !far(world, o)
                    && !world.visits.contains_key(&o)
                    && crate::systems::faith::creed_of(world, o).is_none()
                    && world.gang_of(o).and_then(|g| world.comp::<Gang>(g)).is_none_or(|g| g.creed.is_none())
            });
            if let Some(o) = guest {
                world.visits.insert(o, PendingVisit { kin: place, why: VisitWhy::Service, hour: visit_hour });
                world.consequence.guests.push(o);
                called += 1;
            }
        }
    }
    if called == 0 {
        return;
    }
    world.consequence.services.push((place, Service { preacher, creed, gang, row, hour, called, came: 0 }));
    ownership::jobs2_row(world).svc_held += 1;
}

/// An attendee arrived (a `Visit` to the place, or a Statistical one at its hour).
pub fn attended(world: &mut World, who: EntityId, place: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(s) = world.consequence.services.iter().find(|x| x.0 == place).map(|x| x.1.clone()) else { return };
    let pl = world.name_of(place);
    let hour = s.hour;
    if Some(who) == s.preacher {
        let text = format!(
            "{} preached {} at {pl} at {hour}:00 ({} called){}",
            world.name_of(who),
            s.creed,
            s.called,
            if walked { "" } else { " (off screen)" }
        );
        world.push_event(EventKind::Service, &[who, place], text);
        return;
    }
    if let Some(x) = world.consequence.services.iter_mut().find(|x| x.0 == place) {
        x.1.came += 1;
    }
    ownership::jobs2_row(world).svc_attended += 1;
    let belonging = world.comp::<Needs>(who).map_or(1.0, |n| n.belonging);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        n.belonging = (n.belonging + c.svc_gain).min(1.0);
    }
    let an = world.name_of(who);
    let preacher = s.preacher.filter(|&p| living(world, p));
    if let Some(p) = preacher {
        crate::systems::social::interacted(world, who, p);
    }
    let head = match preacher {
        Some(p) => format!("{an} heard {} preach {} at {pl}", world.name_of(p), s.creed),
        None => format!("{an} sat through the {} service at {pl}", s.creed),
    };
    let head = if walked { head } else { format!("{head} (off screen)") };
    let mut actors = vec![who];
    actors.extend(preacher);
    actors.push(place);
    let guest = world.consequence.guests.contains(&who);
    let day = world.day();
    let m = crate::systems::misery::misery(world, who);
    let t = traits(world, who);
    if guest {
        ownership::jobs2_row(world).svc_guests += 1;
        let p = c.svc_convert_p * (0.5 + m + (1.0 - belonging));
        if hash(world, P_CONVERT, who, day) < p {
            let took = match (s.row, s.gang) {
                (Some(i), _) => {
                    crate::systems::faith::convert(world, who, i);
                    crate::systems::faith::creed_of(world, who) == Some(i)
                }
                (None, Some(g)) => {
                    let open = world.comp::<Gang>(g).is_some_and(|x| x.members.len() < world.config.gangs.max_members)
                        && crate::systems::creeds::tolerated(world, g, who);
                    if open && world.gang_of(who).is_none() {
                        crate::systems::gang::enlist(world, who, g);
                    }
                    world.gang_of(who) == Some(g)
                }
                (None, None) => false,
            };
            if took {
                ownership::jobs2_row(world).svc_converts += 1;
                let text = format!("{head} as a guest, and took it up (misery {m:.2}, belonging {belonging:.2})");
                world.push_event(EventKind::Service, &actors, text);
                return;
            }
        }
        world.push_event(EventKind::Service, &actors, format!("{head} as a guest, and went home unmoved"));
        return;
    }
    // The plate.
    let purse = world.purse(Some(who)).max(0);
    let mut plate = String::new();
    if purse > 0 {
        let amount = ((purse as f32 * c.svc_tithe_share).round() as i64).clamp(1, c.svc_tithe_max.max(1));
        let moved = match s.gang {
            Some(g) => ownership::pay(world, Some(who), Some(g), amount, Flow::Tithe),
            None => ownership::tithe_in(world, who, place, amount),
        };
        if moved > 0 {
            ownership::jobs2_row(world).svc_tithe += moved;
            plate = format!(", put {moved}c on the plate");
        }
    }
    // Walking out.
    let p_out = c.svc_walkout_p * (1.5 - t.loyalty) * (0.5 + m);
    if hash(world, P_WALKOUT, who, day) < p_out {
        walk_out(world, &c, who, &s, preacher, &head, &actors, m, t.loyalty);
        return;
    }
    world.push_event(EventKind::Service, &actors, format!("{head}{plate}"));
}

#[allow(clippy::too_many_arguments)]
fn walk_out(
    world: &mut World,
    c: &ConsequenceCfg,
    who: EntityId,
    s: &Service,
    preacher: Option<EntityId>,
    head: &str,
    actors: &[EntityId],
    misery: f32,
    loyalty: f32,
) {
    let mut hunted = None;
    match s.gang {
        Some(g) if world.gang_of(who) == Some(g) => {
            crate::systems::gang::leave(world, who, "walked out of the service");
            // A cult row's leaver took the leader's apostasy grudge
            // (`faith::on_leave`); a Purist's (or a row without one) takes
            // the preacher's.
            let row_w = s.row.and_then(|i| crate::systems::faith::row(world, i)).map_or(0.0, |r| r.apostasy_w);
            let leader = world.comp::<Gang>(g).and_then(|x| x.leader);
            if row_w > 0.0 {
                hunted = leader;
            } else if let Some(p) = preacher.filter(|&p| p != who) {
                if crate::systems::grudges::add(world, p, who, GrudgeCause::Apostasy, c.svc_heretic_w, 0) {
                    world.jobs2.faith.apostates.insert(who);
                    hunted = Some(p);
                }
            }
        }
        _ => {
            world.jobs2.faith.followers.remove(&who);
            let row_w = s.row.and_then(|i| crate::systems::faith::row(world, i)).map_or(0.0, |r| r.apostasy_w);
            if row_w > 0.0 {
                if let Some(p) = preacher.filter(|&p| p != who) {
                    if crate::systems::grudges::add(world, p, who, GrudgeCause::Apostasy, row_w, 0) {
                        world.jobs2.faith.apostates.insert(who);
                        hunted = Some(p);
                    }
                }
            }
        }
    }
    let tail = match hunted {
        Some(h) => {
            let w = crate::systems::grudges::grudge_on(world, h, who).map_or(0.0, |g| g.weight);
            format!("; {} named {} a heretic (grudge {w:.2})", world.name_of(h), pro(world, who).1)
        }
        None => String::new(),
    };
    let text = format!("{head} and walked out on {} (loyalty {loyalty:.2}, misery {misery:.2}){tail}", s.creed);
    let mut a = actors.to_vec();
    a.extend(hunted.filter(|h| !a.contains(h)));
    world.push_event(EventKind::Service, &a, text);
    ownership::jobs2_row(world).svc_walkouts += 1;
}
