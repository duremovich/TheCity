use super::*;

/// The jobless free adults' first day without a post (`idle_since`).
pub(super) fn idle_days(world: &mut World) {
    let today = world.day();
    // scan-ok: daily: the jobless' idle days
    let adults: Vec<EntityId> = world.citizens().into_iter().filter(|&a| world.has::<Brain>(a)).collect();
    let mut keep = std::collections::BTreeSet::new();
    for a in adults {
        // Shadow V4 bug 5: an exec draws a salary and is not idle (a
        // 68-year-old exec opened a solicitation pitch).
        if !free_adult(world, a) || world.has::<Job>(a) || crate::systems::founding::is_exec(world, a) {
            continue;
        }
        keep.insert(a);
        world.jobs2.idle_since.entry(a).or_insert(today);
    }
    world.jobs2.idle_since.retain(|a, _| keep.contains(a));
}

/// Each Pitch's day closes: the holding gang's cut of the take, a sweep in
/// a `Sweep` district (capped a day), the 7-day take and the dry days.
pub(super) fn close_the_day(world: &mut World) {
    use crate::components::{Controller, Stance};
    let cfg = world.config.jobs2.clone();
    let (now, day) = (world.tick, world.day());
    let mut swept = 0u16;
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        let d = world.districts.get(p.district.index());
        let gang = d.and_then(|d| match d.control {
            Controller::Gang(g) => Some(g),
            _ => None,
        });
        let sweep_stance = d.is_some_and(|d| d.stance == Stance::Sweep);
        let mut cut_to = p.cut_to;
        // Jobs & lives phase 4 (§ 5.1): a street pitch under a pimp pays his
        // gang `pimp_cut` instead.
        let pimp = crate::systems::vice::street_cut(world, &p);
        let (gang, frac) = match pimp {
            Some(c) => (Some(c.gang), c.frac),
            None => (gang, cfg.cut_frac),
        };
        // The pre-view pass phase 4 (OPEN-119): a pimp's cut with a body on
        // either side was taken at his collections in person, not here.
        let at_close = pimp.is_none_or(|c| crate::systems::vice::street_cut_at_close(world, &p, &c));
        if let Some(g) = gang.filter(|_| p.take_today > 0 && at_close) {
            let cut = (p.take_today as f32 * frac).round() as i64;
            // Lives at work phase 4b (§ 2.3): the cut lands in the pimp's
            // purse; he passes `pimp_pass_up` of it to the gang.
            let to = pimp.and_then(|c| c.pimp).unwrap_or(g);
            let paid = ownership::pay(world, Some(o), Some(to), cut, Flow::StreetCut);
            ownership::jobs2_row(world).street_cut += paid;
            if pimp.is_some() {
                ownership::jobs2_row(world).pimp_cut += paid;
            }
            if to != g {
                crate::systems::vice::pass_up(world, to, g, paid);
            }
            cut_to = Some(g);
        }
        // Jobs & lives phase 4: a street pitch's sweep is the vice's
        // (`Solicitation`, never with `street_legal` on).
        let street = crate::systems::vice::on(world) && crate::systems::vice::is_street_pitch(world, &p);
        let sweep = if street {
            swept < cfg.sweeps_per_day && crate::systems::vice::sweep_street(world, &p)
        } else {
            sweep_stance
                && swept < cfg.sweeps_per_day
                && hash(world, PURPOSE_SWEEP, u64::from(o.index), day) < cfg.p_pitch_sweep
        };
        let mut closed_until = p.closed_until.filter(|&t| t > now);
        if sweep {
            swept += 1;
            let fine = ownership::pay(world, Some(o), None, cfg.pitch_fine, Flow::Fine);
            closed_until = Some(now + Tick::from(cfg.sweep_close_days.max(1)) * TICKS_PER_DAY);
            ownership::jobs2_row(world).swept += 1;
            let (who, at) = (world.name_of(o), world.district_name(p.district).to_string());
            world.push_event(
                EventKind::PitchSwept,
                &[o],
                format!("{who}'s pitch in {at} was swept (fined {fine}, closed a day)"),
            );
        }
        if let Some(x) = world.jobs2.pitches.get_mut(&o) {
            x.cut_to = cut_to;
            x.closed_until = closed_until;
            x.take_week = x.take_week - x.take_week / 7 + x.take_today;
            x.dry_days = if x.take_today > 0 { 0 } else { x.dry_days.saturating_add(1) };
            x.take_today = 0;
            x.cut_paid = 0;
            x.jobs_today = 0;
        }
    }
}

/// A Pitch closes when its owner is gone, hired, held or dry for
/// `pitch_fail_days` (`PitchClosed`; a gone owner's silently).
pub(super) fn closings(world: &mut World) {
    let fail = world.config.jobs2.pitch_fail_days;
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let why = if !crate::systems::law::living(world, o) {
            world.jobs2.pitches.remove(&o);
            continue;
        } else if world.has::<Job>(o) {
            "hired"
        } else if world.has::<Sentence>(o) {
            "jailed"
        } else if world.jobs2.pitches.get(&o).is_some_and(|p| p.dry_days >= fail) {
            "no take"
        } else {
            continue;
        };
        let Some(p) = world.jobs2.pitches.remove(&o) else { continue };
        ownership::jobs2_row(world).pitch_closed += 1;
        let what = world.config.trade(p.trade).map_or("pitch", |r| r.label.as_str()).to_string();
        let who = world.name_of(o);
        world.push_event(EventKind::PitchClosed, &[o], format!("{who} closed a {what} pitch ({why})"));
    }
}

/// The midnight openings: per district under its cap, the idle jobless
/// with the stake, in id order on `p_pitch_open` (a pure hash of the day
/// and the agent); the trade by skill (knowledge a tinker, fighting a
/// rickshaw, else a vendor); the spot a HangOut spot of the district no
/// Pitch holds, else its centroid.
pub(super) fn openings(world: &mut World) {
    let cfg = world.config.jobs2.clone();
    let (today, now) = (world.day(), world.tick);
    let n = world.districts.len();
    let mut count = vec![0u16; n];
    for p in world.jobs2.pitches.values() {
        if let Some(c) = count.get_mut(p.district.index()) {
            *c += 1;
        }
    }
    let cands: Vec<EntityId> = world
        .jobs2
        .idle_since
        .iter()
        .filter(|&(_, &since)| today.saturating_sub(since) >= u64::from(cfg.pitch_after_days))
        .map(|(&a, _)| a)
        .collect();
    for a in cands {
        if world.jobs2.pitches.contains_key(&a) || !free_adult(world, a) || world.has::<Job>(a) {
            continue;
        }
        if coins(world, a) < cfg.pitch_stake || hash(world, PURPOSE_OPEN, u64::from(a.index), today) >= cfg.p_pitch_open
        {
            continue;
        }
        let Some(d) = district_of_agent(world, a) else { continue };
        // The pressure pass F1: the spot is chosen below from the home's
        // reach, not the district the opener stood in at midnight.
        if !crate::systems::distance::on(world) && count.get(d.index()).is_none_or(|&c| c >= cfg.pitches_per_district) {
            continue;
        }
        let skills = world.comp::<Skills>(a).cloned();
        // Shadow V4 bug 6: an elder (past `[ages] frail_from_years`) vends;
        // the rickshaw and the street are not his rows (3 of 4 street
        // workers were 64-68).
        let elder = crate::systems::ages::on(world)
            && world.comp::<crate::components::Identity>(a).map_or(0, |i| i.age_years())
                >= world.config.ages.frail_from_years;
        let kind = match skills {
            Some(s) if s.knowledge >= cfg.tinker_skill => PitchKind::Tinker,
            Some(s) if s.melee >= cfg.rickshaw_skill && !elder => PitchKind::Rickshaw,
            _ => PitchKind::Vendor,
        };
        // Jobs & lives phase 4 (§ 5.1): with `[vice]` on, a lawless opener
        // works a street pitch on `p_street` (a pure hash of the day).
        let street = !elder
            && crate::systems::vice::on(world)
            && world.comp::<Personality>(a).is_some_and(|p| p.lawfulness < world.config.vice.street_lawful)
            && hash(world, PURPOSE_STREET, u64::from(a.index), today) < world.config.vice.p_street;
        let kind = if street && pitch_row(world, PitchKind::Street).is_some() { PitchKind::Street } else { kind };
        let Some(trade) = pitch_row(world, kind).or_else(|| pitch_row(world, PitchKind::Vendor)) else { continue };
        let used: Vec<TilePos> = world.jobs2.pitches.values().map(|p| p.spot).collect();
        // Shadow V4 bug 6: the free spot nearest the opener's home (the
        // district's first free one had vendors walking 47-168 min a way).
        let from = crate::systems::contracts::home_door(world, a)
            .or_else(|| world.comp::<crate::components::Position>(a).map(|p| p.tile));
        // The pressure pass F1 (`[distance]` on): the free spot nearest the
        // home door in any district under its cap, and none past `[distance]
        // job_cap_ticks` (a vendor whose midnight found him in the Sump
        // pitched 670 tiles from his Mid-Levels home, 9 h a way).
        // Pressure pass round 6, V2 (TURF § 2): a Street pitch opens in a held
        // cell where the holder's members live (its corner, else a spot of
        // the cell), the nearest the opener's Home within the job cap, so the
        // pimp named among them walks to it; none: as before.
        let turf_spot = (kind == PitchKind::Street)
            .then(|| turf_pitch(world, from, &used, &count, cfg.pitches_per_district))
            .flatten();
        if turf_spot.is_some() {
            ownership::jobs2_row(world).turf_pitches += 1;
        }
        let (spot, d) = if let Some((t, d)) = turf_spot {
            (Some(t), d)
        } else if let (Some(cap), Some(f)) = (crate::systems::distance::job_cap(world), from) {
            let near = world
                .spots
                .iter()
                .enumerate()
                .filter(|&(i, _)| count.get(i).is_some_and(|&c| c < cfg.pitches_per_district))
                .flat_map(|(i, s)| s.iter().map(move |x| (x.tile, i)))
                .filter(|(t, _)| !used.contains(t))
                .min_by_key(|&(t, i)| (f.manhattan(t), t.x, t.y, i));
            match near.filter(|&(t, _)| crate::systems::distance::travel(world, f, t) <= cap) {
                Some((t, i)) => (Some(t), crate::components::DistrictId(i as u8)),
                None => continue,
            }
        } else {
            let spot = world
                .spots
                .get(d.index())
                .and_then(|s| {
                    s.iter()
                        .map(|x| x.tile)
                        .filter(|t| !used.contains(t))
                        .min_by_key(|t| (from.map_or(0, |f| f.manhattan(*t)), t.x, t.y))
                })
                .or_else(|| world.districts.get(d.index()).map(|x| x.centroid));
            (spot, d)
        };
        let Some(spot) = spot else { continue };
        let p = Pitch {
            spot,
            district: d,
            trade,
            owner: a,
            since: now,
            take_today: 0,
            take_week: 0,
            cut_to: None,
            stock: 0,
            closed_until: None,
            dry_days: 0,
            jobs_today: 0,
            cut_paid: 0,
        };
        world.jobs2.pitches.insert(a, p);
        count[d.index()] += 1;
        ownership::jobs2_row(world).pitch_opened += 1;
        let what = world.config.trade(trade).map_or("pitch", |r| r.label.as_str()).to_string();
        let (who, at) = (world.name_of(a), world.district_name(d).to_string());
        world.push_event(EventKind::PitchOpened, &[a], format!("{who} opened a {what} pitch in {at}"));
    }
}

/// Round 6 V2 (TURF § 2): the Street pitch's tile in the turf: the corner or
/// a spot of a held cell where the holder's members live, free, in a
/// district under its cap, the nearest `from` within `[distance]
/// job_cap_ticks` (no cap without `[distance]`); `None` off or none.
fn turf_pitch(
    world: &World,
    from: Option<TilePos>,
    used: &[TilePos],
    count: &[u16],
    per_district: u16,
) -> Option<(TilePos, crate::components::DistrictId)> {
    if !crate::systems::turf::on(world) {
        return None;
    }
    let f = from?;
    let mut cands: Vec<TilePos> = Vec::new();
    for (&g, cells) in &world.turf.held {
        for &k in cells {
            let Some(cell) = world.turf.cells.get(&k) else { continue };
            if cell.residents(g) == 0 {
                continue;
            }
            cands.extend(cell.corner);
            let d = world.district_of(crate::systems::turf::centre(world, k));
            if let Some(v) = world.spots.get(d.index()) {
                cands.extend(
                    v.iter()
                        .filter(|s| !matches!(s.kind, crate::living::SpotKind::Hideout(_)))
                        .map(|s| s.tile)
                        .filter(|&t| crate::systems::turf::cell_of(world, t) == Some(k)),
                );
            }
        }
    }
    let cap = crate::systems::distance::job_cap(world);
    cands
        .into_iter()
        .filter(|t| !used.contains(t))
        .filter(|&t| count.get(world.district_of(t).index()).is_some_and(|&c| c < per_district))
        .map(|t| (f.manhattan(t), t.x, t.y, t))
        .filter(|&(_, _, _, t)| cap.is_none_or(|c| crate::systems::distance::travel(world, f, t) <= c))
        .min()
        .map(|(_, _, _, t)| (t, world.district_of(t)))
}
