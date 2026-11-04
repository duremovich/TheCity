use super::*;

/// The gang with the most territory (held Homes) in each district with a
/// street pitch (ties the lower id) names its pimp: the sitting one while he
/// is free and a member, else its loyal member of lowest id who deals no
/// Stims; none for a district no gang has a foot in.
pub(super) fn name_pimps(world: &mut World) {
    let street = crate::systems::service::street_on(world);
    let today = world.day();
    let absent_days = u64::from(world.config.service.pimp_absent_days.max(1));
    let mut seen: BTreeMap<u16, u64> = BTreeMap::new();
    let mut want: BTreeMap<u16, EntityId> = BTreeMap::new();
    let mut quits: Vec<(EntityId, EntityId, DistrictId)> = Vec::new();
    // Round 5 U6: a sitting pimp's street passed on, and why (told below).
    let mut lost: Vec<(EntityId, Option<EntityId>, DistrictId, String)> = Vec::new();
    let mut held: BTreeMap<DistrictId, (usize, std::cmp::Reverse<EntityId>)> = BTreeMap::new();
    // Pressure pass round 6, V2 (TURF § 2): with `[turf]`, a district's street
    // is the gang's holding the cells its street pitches stand in (most
    // pitches, then most cells held in the district, then the lower id).
    let turf = crate::systems::turf::on(world);
    if turf {
        let mut by: BTreeMap<DistrictId, BTreeMap<EntityId, (usize, usize)>> = BTreeMap::new();
        for p in world.jobs2.pitches.values().filter(|p| is_street_pitch(world, p)) {
            if let Some(g) = crate::systems::turf::holder_at(world, p.spot) {
                by.entry(p.district).or_default().entry(g).or_default().0 += 1;
            }
        }
        for (&g, cells) in &world.turf.held {
            for &k in cells {
                let d = world.district_of(crate::systems::turf::centre(world, k));
                by.entry(d).or_default().entry(g).or_default().1 += 1;
            }
        }
        for (d, m) in by {
            if let Some((g, (a, b))) = m.into_iter().max_by(|x, y| x.1.cmp(&y.1).then(y.0.cmp(&x.0))) {
                held.insert(d, (a + b, std::cmp::Reverse(g)));
            }
        }
    } else {
        for g in world.gangs() {
            for (d, n) in crate::systems::gang::held_districts(world, g) {
                let e = held.entry(d).or_insert((0, std::cmp::Reverse(g)));
                if (n, std::cmp::Reverse(g)) > *e {
                    *e = (n, std::cmp::Reverse(g));
                }
            }
        }
    }
    let mut districts: Vec<DistrictId> = Vec::new();
    for p in world.jobs2.pitches.values() {
        if is_street_pitch(world, p) && !districts.contains(&p.district) {
            districts.push(p.district);
        }
    }
    districts.sort_unstable();
    for d in districts {
        let Some(&(n, std::cmp::Reverse(g))) = held.get(&d) else { continue };
        if n == 0 {
            continue;
        }
        let k = d.index() as u16;
        let sitting_any = world.jobs2.vice.pimps.get(&k).copied();
        // Lives at work phase 4b (§ 2.3): absent from every pitch of his
        // `pimp_absent_days`, the sitting pimp loses the district.
        let absent = street
            && sitting_any.is_some()
            && world.jobs2.vice.pimp_seen.get(&k).is_some_and(|&s| today.saturating_sub(s) >= absent_days);
        let sitting = sitting_any.filter(|&p| {
            !absent && world.gang_of(p) == Some(g) && crate::systems::law::living(world, p) && !world.has::<Sentence>(p)
        });
        let fit = |m: EntityId| {
            crate::systems::law::living(world, m)
                && !world.has::<Sentence>(m)
                // Round 5 U6: no child runs the street (`ages::is_protected`).
                && !crate::systems::ages::is_protected(world, m)
                && crate::systems::gang::following_order(world, m).is_some()
                && !crate::systems::stims::is_dealer(world, m)
                && !(absent && sitting_any == Some(m))
        };
        // Lives at work phase 4b (§ 2.3): the round is a night's work, so
        // a member with no Job first (a day job's sleeper is no pimp).
        // R6a (OPEN-217): the jobless member living nearest the district's
        // street pitches (the lowest id walked 453 tiles, six hours, to his
        // pitch and came after its hours); the lowest id without `[trade_verbs]`.
        let spots: Vec<TilePos> = world
            .jobs2
            .pitches
            .values()
            .filter(|p| p.district == d && is_street_pitch(world, p))
            .map(|p| p.spot)
            .collect();
        let near = |m: EntityId| {
            crate::systems::contracts::home_door(world, m)
                .map_or(u32::MAX, |h| spots.iter().map(|s| s.manhattan(h)).min().unwrap_or(u32::MAX))
        };
        // Round 5 U6 (OPEN-217): nobody whose trip from his Home door to the
        // district's nearest street pitch is past `[trade_verbs]
        // pimp_trip_ticks` is named (the round's own bound, `round_pitch`:
        // the 20k Sump's pimp lived 771 tiles, a 347-minute trip, from its
        // one pitch, so no round ever and the post a label); a district with
        // no member that near has no pimp. Without `[distance.choice]`,
        // `pimp_round_tiles` of it (tiles).
        let reach = world.config.trade_verbs.as_ref().map(|c| c.pimp_round_tiles);
        let cap = pimp_trip_cap(world);
        // Round 6 V2 (TURF § 2): with `[turf]` the pimp is named among the
        // members living on the gang's blocks: his Home cell held by the gang
        // within `[turf] pimp_cells` of a cell with one of the district's
        // street pitches (replaces U6's `pimp_trip_ticks` bound).
        let pitch_cells: Vec<u32> = spots.iter().filter_map(|&s| crate::systems::turf::cell_of(world, s)).collect();
        let near_cells = crate::systems::turf::cfg(world).map_or(1, |c| u32::from(c.pimp_cells));
        let within = |m: EntityId| {
            if turf {
                let Some(hc) = crate::systems::turf::home_cell(world, m) else { return false };
                // Round 6 V5 (V2's reads-wrong: a pimp lost his street the
                // night his cell turned contested): blocks his gang is
                // fighting for are still his; a cell lost is not.
                return world.turf.cells.get(&hc).is_some_and(|x| x.status.involves(g))
                    && pitch_cells.iter().any(|&pc| crate::systems::turf::cell_dist(world, hc, pc) <= near_cells);
            }
            if reach.is_none() {
                return true;
            }
            let Some(h) = crate::systems::contracts::home_door(world, m) else { return false };
            let Some(&spot) = spots.iter().min_by_key(|s| (s.manhattan(h), s.x, s.y)) else { return false };
            match cap {
                Some(c) => crate::systems::distance::trip_ahead(world, m, h, spot) <= c,
                None => reach.is_none_or(|r| spot.manhattan(h) <= r),
            }
        };
        let free = if street {
            world.comp::<Gang>(g).and_then(|x| {
                let jobless = x.members.iter().copied().filter(|&m| fit(m) && !world.has::<Job>(m) && within(m));
                if world.config.trade_verbs.is_some() {
                    jobless.min_by_key(|&m| (near(m), m))
                } else {
                    jobless.into_iter().next()
                }
            })
        } else {
            None
        };
        // R6a (OPEN-217): a sitting pimp who has taken a Job gives the post to
        // a jobless member (a Recycler Tech pimp slept through every pitch
        // night; a Statistical pimp is credited present, so absence never
        // dropped him).
        // Likewise one living past `pimp_round_tiles` farther from the
        // pitches than the nearest jobless member (`pimp_18110` walked 3 h
        // 20 min to his one pitch and met nobody).
        let sitting = sitting.filter(|&p| within(p)).filter(|&p| match (reach, free) {
            (None, _) | (_, None) => true,
            // Round 6 V2: on the turf, the sitting pimp keeps the street while
            // he lives on it and holds no day post.
            (Some(_), Some(_)) if turf => !world.has::<Job>(p),
            (Some(r), Some(f)) => !world.has::<Job>(p) && near(p) <= near(f).saturating_add(r),
        });
        // Round 5 U6 (OPEN-217): with no jobless member fit, the fit member
        // living nearest the pitches (the lowest id without `[trade_verbs]`),
        // who gives up his day post for the round (`pimp_quits`).
        let pick = sitting.or(free).or_else(|| {
            let members = &world.comp::<Gang>(g)?.members;
            if world.config.trade_verbs.is_some() {
                members.iter().copied().filter(|&m| fit(m) && within(m)).min_by_key(|&m| (near(m), m))
            } else {
                members.iter().copied().find(|&m| fit(m))
            }
        });
        if let Some(old) = sitting_any.filter(|&o| pick != Some(o) && crate::systems::law::living(world, o)) {
            let why = if world.has::<Sentence>(old) {
                "in the cells".to_string()
            } else if world.gang_of(old) != Some(g) {
                let gang = world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
                format!("{gang} holds the district now")
            } else if absent {
                format!("away from every pitch {absent_days} days")
            } else if !within(old) && turf {
                let contested = crate::systems::turf::home_cell(world, old)
                    .and_then(|k| world.turf.cells.get(&k))
                    .is_some_and(|x| matches!(x.status, crate::systems::turf::Status::Contested(..)));
                if contested { "the blocks the pimp lives on contested" } else { "living off the gang's blocks" }
                    .to_string()
            } else if !within(old) {
                format!("living {} tiles from the pitches", near(old))
            } else if world.has::<Job>(old) {
                "holding a day post".to_string()
            } else if crate::systems::gang::following_order(world, old).is_none() {
                "no longer following the gang's order".to_string()
            } else {
                "a member living nearer the pitches".to_string()
            };
            lost.push((old, pick, d, why));
        }
        if let Some(p) = pick {
            if street && world.has::<Job>(p) && world.config.trade_verbs.as_ref().is_some_and(|c| c.pimp_quits) {
                quits.push((p, g, d));
            }
            want.insert(k, p);
            // A newly named pimp's days of grace start today.
            if street {
                let last = world.jobs2.vice.pimp_seen.get(&k).copied().filter(|_| sitting == Some(p));
                seen.insert(k, last.unwrap_or(today));
            }
        }
    }
    // Round 5 U6: a district dropped (no street pitch left, no gang holding
    // Homes in it) takes its pimp's street with it.
    for (&k, &old) in &world.jobs2.vice.pimps {
        if want.contains_key(&k) || !crate::systems::law::living(world, old) || lost.iter().any(|l| l.0 == old) {
            continue;
        }
        let Some(d) = world.districts.get(usize::from(k)).map(|x| x.id) else { continue };
        let pitched = world.jobs2.pitches.values().any(|p| p.district == d && is_street_pitch(world, p));
        let why = if pitched { "no gang holds the district" } else { "no street pitch left" };
        lost.push((old, None, d, why.to_string()));
    }
    world.jobs2.vice.pimps = want;
    world.jobs2.vice.pimp_seen = seen;
    if world.config.trade_verbs.is_some() {
        for (old, new, d, why) in lost {
            let at = world.district_name(d).to_string();
            let text = match new {
                Some(n) => format!("{} lost the street of {at} to {} ({why})", world.name_of(old), world.name_of(n)),
                None => format!("{} lost the street of {at} ({why})", world.name_of(old)),
            };
            let mut actors = vec![old];
            actors.extend(new);
            world.push_event(EventKind::Crew, &actors, text);
        }
    }
    // Round 5 U6 (OPEN-217): a named pimp holding a day post gives it up:
    // the round is a night's work (V13's 100k `pimp_19928`, an Engineer, and
    // the 20k `pimp_21194`, a Sanitation hand, slept through every pitch
    // night; his GangWork was a daytime Extort).
    for (p, g, d) in quits {
        let gang = world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
        let reason = format!("to run {gang}'s street in {}", world.district_name(d));
        crate::systems::economy::quit_job(world, p, &reason);
    }
}

/// D7: an unlicensed Brothel closes a day in a Sweep or a Crackdown
/// district (a licensed one is shut only as a front of the gang a
/// Crackdown targets, `leisure::fronts_daily`).
pub(super) fn close_brothels(world: &mut World) {
    if world.levers.vice.brothels_licensed {
        return;
    }
    let now = world.tick;
    for b in world.buildings_of_kind(BuildingKind::Brothel).to_vec() {
        let d = world.district_of_building(b);
        let stance = world.districts.get(d.index()).map(|x| x.stance);
        let shut = matches!(stance, Some(Stance::Crackdown(_) | Stance::Sweep));
        if shut && !world.is_closed(b) {
            if let Some(bd) = world.comp_mut::<Building>(b) {
                bd.closed_until = Some(now + TICKS_PER_DAY);
            }
            ownership::jobs2_row(world).brothel_closed += 1;
        }
    }
}

/// `World::new`, after the industries: each copy of `[vice] corp_rows`
/// takes the Vice niche and opens `strip_clubs` StripClubs and `brothels`
/// Brothels on the Lots left (the venue tier rule). No RNG, no coins.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.vice.clone();
    let copies = world.lot_copies();
    for j in 0..copies {
        for &r in &cfg.corp_rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            if let Some(cc) = world.comp_mut::<Corp>(c) {
                cc.niches.insert(Niche::Vice);
            }
            for (kind, n) in [(BuildingKind::StripClub, cfg.strip_clubs), (BuildingKind::Brothel, cfg.brothels)] {
                for _ in 0..n {
                    seed_venue(world, kind, Some(c));
                }
            }
        }
    }
}

/// A seeded venue of `kind` for `owner` on a vacant Lot (`jobs::seed_lot`).
pub(crate) fn seed_venue(world: &mut World, kind: BuildingKind, owner: Option<EntityId>) -> Option<EntityId> {
    let lot = crate::systems::jobs::seed_lot(world, kind)?;
    seed_venue_on(world, lot, kind, owner)
}

/// [`seed_venue`] on a Lot already chosen (round 4 T2: a School by reach).
pub(crate) fn seed_venue_on(
    world: &mut World,
    lot: EntityId,
    kind: BuildingKind,
    owner: Option<EntityId>,
) -> Option<EntityId> {
    crate::systems::founding::build_on_lot(world, lot, kind, owner).ok()?;
    let at = world.districts.get(world.district_of_building(lot).index()).map_or("?", |x| x.name.as_str());
    let who = world.owner_label(owner);
    let text = format!("{who} opened a {} in {at} (seeded; Lot {})", kind.label(), lot.index);
    let mut actors: Vec<EntityId> = owner.into_iter().collect();
    actors.push(lot);
    world.push_event(EventKind::Founded, &actors, text);
    Some(lot)
}

/// The day's end (`systems::stats`): the vice group's snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let venues = [BuildingKind::StripClub, BuildingKind::Brothel]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .count() as u32;
    let street = world.jobs2.pitches.values().filter(|p| is_street_pitch(world, p)).count() as u32;
    let pimps = world.jobs2.vice.pimps.len() as u32;
    let row = ownership::jobs2_row(world);
    row.vice_venues = venues;
    row.street_pitches = street;
    row.pimps = pimps;
}
