use super::*;

/// Everything one tick can add to `t`. `now` is the tick that just ran.
pub(super) fn observe(
    world: &World,
    t: &mut Track,
    now: Tick,
    events: &[&Event],
    dbg: &[&Event],
    notes: &[ShadowNote],
) {
    let id = t.id;
    let day = now / TICKS_PER_DAY;
    t.rest_days.entry(day).or_insert_with(|| rest_day_note(world, id, day));
    if now / TICKS_PER_HOUR != t.hourly_hour {
        flush_hourly(t);
        t.hourly_hour = now / TICKS_PER_HOUR;
    }
    // Day rollover: close the finished day.
    if day != t.cur_day {
        flush_day(world, t, now);
        t.cur_day = day;
        // Round 4 T7: the day folds and the day's encounters start afresh.
        t.day_folds.clear();
        t.cool_folds.clear();
        t.served_today.clear();
        // Reputation and class at the day's start.
        if world.has::<Identity>(id) && !world.has::<Corpse>(id) {
            let r = rep(world, id);
            let cur = [r.dread, r.standing, r.honour, r.heat];
            let cls = class_of(world, id).label().to_string();
            if cur.iter().zip(t.rep.iter()).any(|(a, b)| (a - b).abs() >= 0.02) || cls != t.class {
                t.log(
                    now,
                    "rep",
                    format!(
                        "reputation dread {:.2} standing {:.2} honour {:.2} heat {:.2}; class {}",
                        cur[0], cur[1], cur[2], cur[3], cls
                    ),
                );
                t.rep = cur;
                t.class = cls;
            }
        }
    }

    // Events naming the agent (story ring by id cursor).
    let action_now: Option<ActionKind> = world.comp::<Brain>(id).and_then(|b| b.current_step().map(|s| s.action));
    let prev_action = std::mem::replace(&mut t.last_action, action_now);
    // Shadow V6: a courier's parcels handed over (before the round's event).
    cam_parcels(world, t, now);
    let mut event_ctx: Vec<String> = Vec::new();
    // Round 4 T7: the actors of the petty crimes the pick saw this tick.
    let mut petty_now: BTreeSet<EntityId> = BTreeSet::new();
    // Round 3 S7 (OPEN-225): the minute's crowd lines fold into one each.
    let mut folds: Vec<Fold> = Vec::new();
    // The raid's head reads the result line ("Raid on X: ..."); the sim's
    // older summary of the same brawl ("G raided X: Won (...)", "G stormed
    // the Precinct: ...") is the same fact twice and stays in the event log.
    let raid_result: BTreeSet<Tick> = events
        .iter()
        .filter(|e| e.kind == EventKind::Raid && e.actors.contains(&id) && e.text.starts_with("Raid on "))
        .map(|e| e.tick)
        .collect();
    for e in events {
        if !e.actors.contains(&id) {
            continue;
        }
        if e.kind == EventKind::Raid
            && raid_result.contains(&e.tick)
            && (e.text.contains(" stormed the Precinct: ")
                || (e.text.contains(" raided ") && !e.text.starts_with("Raid on ")))
        {
            continue;
        }
        *t.event_kinds.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
        // Round 7 W2 (OPEN-241): a war line's actors include the gang's men who
        // hear it, not people the pick met.
        if e.kind != EventKind::Turf {
            let others: Vec<EntityId> =
                e.actors.iter().copied().filter(|&a| a != id && world.has::<Identity>(a)).collect();
            for o in others {
                t.contact(world, o);
            }
        }
        let text = format!("{:?}: {}", e.kind, e.text);
        // Round 4 T7 (OPEN-233): a petty crime the pick saw (a guard inside
        // the Precinct by the El's gate saw 52 Fare Evasions in two days)
        // folds by the hour; its `SawCrime` memory is the same fact.
        if let Some(key) = petty_seen(e, id) {
            let name = e.text.split_once(" saw ").and_then(|(_, r)| r.split_once(" commit ")).map_or("", |x| x.0);
            if let Some(&a) = e.actors.get(1) {
                petty_now.insert(a);
            }
            t.hourly.push(Quiet { tick: e.tick, cat: "event", key, name: name.to_string(), line: text });
            continue;
        }
        // Round 4 T7: a standing fact told again, or the day's fold.
        let refold = refold_of(e, id);
        if let Some(r) = &refold {
            if fold_into(t, r, e) {
                continue;
            }
        }
        let entry_at = t.entries.len();
        let notable_at = (!matches!(e.kind, EventKind::Witness)).then_some(t.notable.len());
        if let Some(sh) = fold_shape(e) {
            if let Some(f) = folds.iter_mut().find(|f| f.tick == e.tick && f.kind == e.kind && f.shape.same(&sh)) {
                f.names.push(sh.name);
                f.sum += sh.sum;
                let folded = f.text();
                t.entries[f.entry].text.clone_from(&folded);
                if let Some(i) = f.notable {
                    t.notable[i].1.clone_from(&folded);
                }
                event_ctx[f.ctx] = folded;
                continue;
            }
            folds.push(Fold {
                tick: e.tick,
                kind: e.kind,
                names: vec![sh.name.clone()],
                sum: sh.sum,
                shape: sh,
                entry: t.entries.len(),
                notable: (!matches!(e.kind, EventKind::Witness)).then_some(t.notable.len()),
                ctx: event_ctx.len(),
            });
        }
        if !matches!(e.kind, EventKind::Witness) {
            t.notable.push((e.tick, text.clone()));
        }
        event_ctx.push(format!("{:?} \"{}\"", e.kind, e.text));
        t.log(e.tick, "event", text.clone());
        if let Some(r) = refold {
            open_fold(t, r, e, entry_at, notable_at, text);
        }
        // Shadow V6 (V5 bug 20): the courier's round is the parcel round.
        if e.kind == EventKind::SuperRound && e.actors.first() == Some(&id) && is_courier(world, id) {
            let line = format!(
                "the round was the courier's parcel round: {} parcel(s) handed over in the window so far, {} of the yard's still pending",
                t.cam.parcels_handed,
                t.cam.parcels.len()
            );
            t.log(e.tick, "parcel", line);
        }
        if matches!(e.kind, EventKind::Death) && e.actors.first() == Some(&id) && t.died.is_none() {
            t.died = Some(e.tick);
        }
        // `[winner, loser, pit]`: a fighter's bout, and whether she was inside the pit for it.
        if e.kind == EventKind::Bout && e.actors.iter().take(2).any(|&a| a == id) {
            t.bouts += 1;
            let pit = e.actors.get(2).copied();
            if pit.is_some() && world.comp::<Position>(id).and_then(|p| p.building) == pit {
                t.bouts_present += 1;
            }
        }
    }
    for e in dbg {
        if e.actors.first() != Some(&id) {
            continue;
        }
        // "A -> B": interrupted; "A failed at X: reason".
        let label = if e.text.contains(" failed at ") { "PLAN FAILED" } else { "plan interrupted" };
        t.log(e.tick, "plan", format!("{label}: {}", e.text));
        *t.cam.days.entry(e.tick / TICKS_PER_DAY).or_default().aborts.entry(abort_reason(&e.text)).or_insert(0) += 1;
    }
    let mut flow_ctx: Vec<String> = Vec::new();
    // Shadow V4 bug 23: the tick's largest Flow on this agent (the money
    // section buckets by it; the running step read midnight sales as the
    // daily pass and a mid-shift purchase as a wage).
    let mut tick_flow: Option<(i64, String)> = None;
    for n in notes {
        match n {
            ShadowNote::Move { tick, m, out } if m.actor == id || m.target == id => {
                let other = if m.actor == id { m.target } else { m.actor };
                t.contact(world, other);
                if t.spouse == Some(other) {
                    t.spouse_named_hours.insert(*tick / TICKS_PER_HOUR);
                }
                let dir = if m.actor == id { "to" } else { "from" };
                t.log(
                    *tick,
                    "move",
                    format!(
                        "social move {:?} {dir} {} (stake {:?}): {} p={:.2}{}{}",
                        m.kind,
                        nm(world, other),
                        m.stake,
                        if out.success { "SUCCESS" } else { "failed" },
                        out.p,
                        if out.refused { ", refused to meet" } else { "" },
                        if out.backlash { ", BACKLASH" } else { "" }
                    ),
                );
            }
            ShadowNote::Told { tick, from, to, r, heard } if *from == id || *to == id => {
                let other = if *from == id { *to } else { *from };
                t.contact(world, other);
                let verb = if *from == id { "told" } else { "heard from" };
                t.log(
                    *tick,
                    "gossip",
                    format!(
                        "{verb} {}: {:?} by {} on {}{}",
                        nm(world, other),
                        r.deed,
                        r.actor.map_or("someone unnamed".into(), |a| nm(world, a)),
                        r.object.map_or("?".into(), |o| nm(world, o)),
                        if *heard { "" } else { " (already known / dropped)" }
                    ),
                );
            }
            // V2: gossip told ABOUT the agent (as actor or object), heard by others.
            ShadowNote::Told { tick, from, to, r, heard } if r.actor == Some(id) || r.object == Some(id) => {
                t.gossip_about += 1;
                let role = if r.actor == Some(id) { "as the doer" } else { "as the object" };
                let line = format!(
                    "GOSSIP ABOUT ME ({role}): {} told {}: {:?} by {} on {}{}",
                    nm(world, *from),
                    nm(world, *to),
                    r.deed,
                    r.actor.map_or("someone unnamed".into(), |a| nm(world, a)),
                    r.object.map_or("?".into(), |o| nm(world, o)),
                    if *heard { "" } else { " (already known / dropped)" }
                );
                // Round 4 T7 (OPEN-233): the word about me goes round in runs of
                // 20-40 retellings; the day's run of one rumour folds into its
                // first line (the new hearers named, the rest counted).
                let day = *tick / TICKS_PER_DAY;
                let key = format!("gossip:{role}:{:?}:{:?}:{:?}", r.deed, r.actor, r.object);
                let pair = format!("{} to {}", nm(world, *from), nm(world, *to));
                let what = if *heard { "to new ears" } else { "heard already" };
                let refold = match t.day_folds.get_mut(&key) {
                    Some(f) if f.day == day => {
                        f.count += 1;
                        f.note(what);
                        if *heard {
                            f.name(&pair);
                        }
                        let names = if f.names.is_empty() { String::new() } else { format!(": {}", f.names_text()) };
                        let text = format!(
                            "GOSSIP ABOUT ME ({role}): {:?} by {} on {}, told {} times today ({}){names}",
                            r.deed,
                            r.actor.map_or("someone unnamed".into(), |a| nm(world, a)),
                            r.object.map_or("?".into(), |o| nm(world, o)),
                            f.count,
                            f.tally.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")
                        );
                        Some((f.entry, text))
                    }
                    _ => None,
                };
                // Round 7 W6 (OPEN-217; `[noise] gossip_lines_day`): past the
                // day's cap of rumours a role a rumour opens no line of its
                // own: it folds into the day's one "other rumours" line
                // (V15's 100k vice diaries: 259 lines, "told 22 times today
                // (22 heard already)").
                let cap = usize::from(world.config.noise.gossip_lines_day);
                let more_key = format!("gossip:{role}:more");
                let opened = t
                    .day_folds
                    .values()
                    .filter(|f| f.day == day && f.key.starts_with(&format!("gossip:{role}:")) && f.key != more_key)
                    .count();
                // A retelling to ears that knew it opens nothing (the word
                // reaching nobody new is no news; a rumour's line counts them).
                if refold.is_none() && cap > 0 && !*heard {
                    continue;
                }
                let overflow = refold.is_none() && cap > 0 && opened >= cap;
                if overflow {
                    let deed = format!("{:?}", r.deed);
                    let text = |f: &DayFold| {
                        format!(
                            "GOSSIP ABOUT ME ({role}): {} retelling{} today of {} other rumour{} ({})",
                            f.count,
                            if f.count == 1 { "" } else { "s" },
                            f.names.len(),
                            if f.names.len() == 1 { "" } else { "s" },
                            f.tally_text()
                        )
                    };
                    match t.day_folds.get_mut(&more_key) {
                        Some(f) if f.day == day => {
                            f.count += 1;
                            f.note(what);
                            f.note(&deed);
                            f.name(&key);
                            let (entry, line) = (f.entry, text(f));
                            t.refold_line(entry, None, &line);
                        }
                        _ => {
                            let entry = t.entries.len();
                            let mut f = DayFold::new(more_key.clone(), day, entry, None, String::new());
                            f.note(what);
                            f.note(&deed);
                            f.name(&key);
                            let line = text(&f);
                            f.base = line.clone();
                            t.log(*tick, "gossip", line);
                            t.day_folds.insert(more_key, f);
                        }
                    }
                    continue;
                }
                match refold {
                    Some((entry, text)) => t.refold_line(entry, None, &text),
                    None => {
                        let entry = t.entries.len();
                        t.log(*tick, "gossip", line.clone());
                        let mut f = DayFold::new(key.clone(), day, entry, None, line);
                        f.note(what);
                        if *heard {
                            f.name(&pair);
                        }
                        t.day_folds.insert(key, f);
                    }
                }
            }
            ShadowNote::Flow { tick, from, to, coins, flow, refund } if *from == Some(id) || *to == Some(id) => {
                let other = if *from == Some(id) { *to } else { *from };
                let who = other.map_or("the City".to_string(), |o| nm(world, o));
                // A refund is already oriented owner -> agent: the agent's `in`.
                let (out, inn) = if *from == Some(id) { (*coins, 0) } else { (0, *coins) };
                let label = flow_label(world, *flow, *from, *to, *coins);
                let e = t.flow_kinds.entry(label.clone()).or_insert((0, 0, 0));
                e.0 += out;
                e.1 += inn;
                e.2 += 1;
                if tick_flow.as_ref().is_none_or(|(c, _)| *coins > *c) {
                    tick_flow = Some((*coins, label.clone()));
                }
                if let Some(o) = other {
                    let dir = if *from == Some(id) { "to" } else { "from" };
                    cam_settle(world, t, *tick, o, &format!("{label} {coins}c {dir}"));
                }
                use citysim::systems::ownership::Flow as F;
                if *to == Some(id) && !*refund && matches!(flow, F::Wage | F::ExecWage) && *coins > 0 {
                    t.wages.push(*tick);
                }
                if *to == Some(id) && *refund {
                    t.refunds += 1;
                }
                // The city milestone C6b: a floor rent paid or taken is a line of its own.
                if matches!(flow, F::FloorRent) {
                    let dir =
                        if *from == Some(id) { format!("paid -{coins}c to") } else { format!("took +{coins}c from") };
                    t.log(*tick, "rent", format!("FLOOR RENT {dir} {who}"));
                }
                if matches!(flow, citysim::systems::ownership::Flow::Tribute) && *to == Some(id) {
                    t.tribute_in += *coins;
                    t.log(
                        *tick,
                        "tribute",
                        format!("TRIBUTE received +{coins}c from {who} (the daily stipend or the weekly Collect)"),
                    );
                }
                let sign = if inn > 0 { "+" } else { "-" };
                // Round 6 V4 (OPEN-240): a fare paid to a driver says so.
                let who = match other.and_then(|o| world.comp::<Job>(o)) {
                    Some(j) if matches!(flow, F::CabFare) && *from == Some(id) => {
                        format!("{who} ({})", j.role.label())
                    }
                    _ => who,
                };
                flow_ctx.push(format!(
                    "{label} {sign}{}{} {} {who}",
                    coins,
                    if *refund { " (refund)" } else { "" },
                    if sign == "+" { "from" } else { "to" }
                ));
            }
            ShadowNote::StatGang { tick, id: who, act, caught } if *who == id => {
                let text = format!(
                    "STATISTICAL GangWork day hit: {act:?}{} (no body, no walk, no witness)",
                    if *caught { ", reported (a body for the arrest path)" } else { "" }
                );
                t.stat_notes.push((*tick, text.clone()));
                t.log(*tick, "stat", text);
            }
            ShadowNote::Settled { tick, id: who, ticks } if *who == id => {
                t.held_settles += 1;
                t.held_minutes += u64::from(*ticks);
                t.log(
                    *tick,
                    "jail",
                    format!(
                        "cell settlement (the hold, or the promotion out of it): {ticks} min of jailed decay settled"
                    ),
                );
            }
            _ => {}
        }
    }

    let alive = world.has::<Brain>(id) || (world.has::<Child>(id) && !world.has::<Corpse>(id));
    if !alive {
        // Dead (or gone): close the open action once and stop.
        if world.has::<Corpse>(id) && t.died.is_none() {
            t.died = Some(now);
            t.log(now, "event", "died (corpse present)".into());
        }
        close_action(world, t, now, "agent died");
        end_sleep_block(t, now);
        return;
    }

    // Place and time use.
    let here = place(world, id);
    let bld = world.comp::<Position>(id).and_then(|p| p.building);
    if here != t.last_place {
        if !t.last_place.is_empty() {
            t.log(now, "move", format!("now at {here} (from {})", t.last_place));
        }
        t.last_place = here.clone();
    }
    t.last_building = bld;
    // The city milestone C7: a climb (a Full body's at the door, a Coarse
    // leg's), once; a step onto the walkways' decks.
    if let Some((key, line)) = climb_line(world, id) {
        if t.last_climb != Some(key) {
            t.last_climb = Some(key);
            t.log(now, "climb", line);
        }
    }
    // The view milestone V9 (OPEN-255): an El ride, its legs aboard and
    // the alighting.
    for line in el_lines(world, id, &mut t.el_seen, now) {
        t.log(now, "el", line);
    }
    // The view milestone V7: a step onto a marked crossing (and the kerb's
    // wait for the walk phase) or onto a carriageway off one.
    if let Some(v) = world.crossing_watch.as_ref().and_then(|w| w.get(&id)) {
        for &(tick, tile, junction, wait) in v.iter().skip(t.cross_seen) {
            let line = if wait == u32::MAX {
                format!("CROSS jaywalking the carriageway at ({}, {}), no crossing", tile.x, tile.y)
            } else if wait == 0 {
                format!(
                    "CROSS at the lights at ({}, {}) (junction {},{}), on the green man",
                    tile.x, tile.y, junction.x, junction.y
                )
            } else {
                format!(
                    "CROSS at the lights at ({}, {}) (junction {},{}), {} s at the kerb for the green man",
                    tile.x,
                    tile.y,
                    junction.x,
                    junction.y,
                    wait.div_ceil(1000)
                )
            };
            t.log(tick, "cross", line);
        }
        t.cross_seen = v.len();
    }
    let deck = world.comp::<Position>(id).filter(|p| p.building.is_none()).map_or(0, |p| p.floor);
    if deck != t.last_deck {
        if let Some(p) = world.comp::<Position>(id).filter(|_| deck > 0) {
            let kind = world.map.portals.iter().find(|q| q.a == (p.tile, deck) || q.b == (p.tile, deck)).map_or(
                "a deck",
                |q| match q.kind {
                    citysim::map::PortalKind::Escalator => "the escalator",
                    citysim::map::PortalKind::Platform => "a station platform",
                    _ => "a footbridge",
                },
            );
            t.log(now, "deck", format!("DECK on {kind} at ({}, {}), level {deck}", p.tile.x, p.tile.y));
        } else if t.last_deck > 0 {
            let p = world.comp::<Position>(id).map(|p| p.tile).unwrap_or_default();
            t.log(now, "deck", format!("DECK off the walkways at ({}, {}) (from level {})", p.x, p.y, t.last_deck));
        }
        t.last_deck = deck;
    }
    let act = activity(world, id);
    *t.day_use.entry(act).or_insert(0) += 1;
    *t.total_use.entry(act).or_insert(0) += 1;
    // A held prisoner classes as "jail" but is just as much a stand-in.
    if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
        t.stat_minutes += 1;
    }
    *t.use_by_day.entry(day).or_default().entry(act).or_insert(0) += 1;
    *t.day_places.entry(here.clone()).or_insert(0) += 1;
    *t.total_places.entry(here.clone()).or_insert(0) += 1;
    behaviour_minute(world, t, now, act);
    lives_minute(world, t, now, act, events);
    cam_minute(world, t, now, act, &here);

    if let Some(b) = world.comp::<Brain>(id) {
        if !t.pinned_lod_noted && b.lod == Lod::Full {
            t.pinned_lod_noted = true;
            t.log(now, "info", "at Full LOD (pinned)".into());
        }
        // V2: LOD changes. A pinned pick must stay Full, jailed or not (L1's rule under L2's held
        // prisoners); an unpinned run shows its Statistical and held stretches.
        if t.last_lod != Some(b.lod) {
            if let Some(prev) = t.last_lod {
                let jailed = world.has::<Sentence>(id);
                // Sentencing sets Coarse; the hourly assignment re-pins to Full (up to 59 min
                // later). Statistical is the held tier: a pinned prisoner must never reach it.
                let warn = if t.pinned && b.lod == Lod::Statistical {
                    " **PIN BROKEN: a pinned pick became Statistical (held)**"
                } else if t.pinned && b.lod != Lod::Full && jailed {
                    " (sentencing sets Coarse until the next hourly assignment re-pins it)"
                } else if t.pinned && b.lod != Lod::Full {
                    " **PIN NOT HONOURED: Coarse with no sentence**"
                } else {
                    ""
                };
                t.log(
                    now,
                    "lod",
                    format!(
                        "LOD {prev:?} -> {:?}{}{warn}",
                        b.lod,
                        if jailed && b.lod == Lod::Statistical { " (held in the cells)" } else { "" }
                    ),
                );
            }
            t.last_lod = Some(b.lod);
        }
        // V2: the Unwind pick (rung, satisfier, venue or spot).
        let pick = world.unwind.get(&id).map(|p| {
            format!(
                "rung {:?}, {:?}{}{} (score {:.2})",
                p.rung,
                p.act,
                p.venue.map_or(String::new(), |v| format!(" at {}", nm(world, v))),
                p.spot.map_or(String::new(), |s| format!(" at spot ({},{})", s.x, s.y)),
                p.score
            )
        });
        if pick != t.last_pick {
            if let Some(p) = &pick {
                t.log(now, "unwind", format!("UNWIND pick: {p}"));
            }
            t.last_pick = pick;
        }
        // V2: who is at the HangOut spot (the registry empties as the step completes).
        if action_now == Some(ActionKind::HangOut) {
            if let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) {
                if let Some(list) = world.hangouts.get(&tile) {
                    t.hang_others.extend(list.iter().copied().filter(|&o| o != id));
                }
            }
            // Lives at work phase 2 (§ 3.1): a HangOut inside a Home keeps the
            // household present as its company (the registry holds hangers only).
            if let Some(h) = citysim::systems::household::hanging_at_home(world, id) {
                t.hang_others.extend(citysim::systems::household::residents_inside(world, h, id));
            }
        }
        // Goal choice.
        if b.current_goal != t.last_goal {
            if let Some(g) = b.current_goal {
                t.log(
                    now,
                    "goal",
                    format!(
                        "GOAL {:?}{}: {}",
                        g,
                        t.last_goal.map_or(String::new(), |o| format!(" (was {o:?})")),
                        goal_line(world, id, b)
                    ),
                );
            }
            t.last_goal = b.current_goal;
        }
        // Cooldowns: a failed or unplannable goal cools.
        for (&g, &until) in &b.cooldowns {
            if until > now && t.cooldowns.get(&g) != Some(&until) {
                // Round 3 S2 (OPEN-222): the back-off's streak, from the second.
                let streak_n = b.cool_streak.get(&g).filter(|&&(n, end)| n > 1 && end == until).map(|&(n, _)| n);
                let streak = streak_n.map_or_else(String::new, |n| format!("; backed off, {n} in a row"));
                // Round 4 T7 (OPEN-233): a goal's later cools of the day fold
                // into its first ("Bury cooled ... 33 in a row", one every few
                // hours; a jobless "Earn cooled" every two), the streak's
                // range kept (`days` holds the streaks seen).
                let folded = t.cool_folds.get_mut(&g).map(|f| {
                    f.count += 1;
                    f.days.extend(streak_n.map(u64::from));
                    let range = match (f.days.iter().min(), f.days.iter().max()) {
                        (Some(a), Some(z)) if a < z => format!("; backed off, {a} to {z} in a row"),
                        (Some(a), _) => format!("; backed off, {a} in a row"),
                        _ => String::new(),
                    };
                    let text = format!(
                        "goal {g:?} cooled {} times today, the last until {} (could not plan or plan failed twice{range})",
                        f.count,
                        clock(until)
                    );
                    (f.entry, text)
                });
                match folded {
                    Some((entry, text)) => t.refold_line(entry, None, &text),
                    None => {
                        let mut f = DayFold::new(String::new(), day, t.entries.len(), None, String::new());
                        f.days.extend(streak_n.map(u64::from));
                        t.cool_folds.insert(g, f);
                        t.log(
                            now,
                            "plan",
                            format!(
                                "goal {g:?} cooled until {} (could not plan or plan failed twice{streak})",
                                clock(until)
                            ),
                        );
                    }
                }
            }
        }
        t.cooldowns = b.cooldowns.clone();
        // Plans.
        let ps = b.plan.as_ref().map(|p| p.started_tick);
        if ps != t.plan_started {
            if ps.is_some() {
                t.cam.days.entry(day).or_default().plans += 1;
            }
            if let Some(line) = plan_line(world, b) {
                t.log(now, "plan", format!("PLAN {line}"));
            }
        }
        // Actions.
        let key =
            b.plan.as_ref().filter(|p| usize::from(b.plan_step) < p.steps.len()).map(|p| (p.started_tick, b.plan_step));
        let open_key = t.open.as_ref().map(|o| o.key);
        if key != open_key {
            let status =
                if t.open.is_some() && key.is_some_and(|k| Some(k.0) == t.plan_started) { "done" } else { "ended" };
            close_action(world, t, now, status);
            if let (Some(k), Some(step)) = (key, b.current_step()) {
                t.open = Some(OpenAction {
                    key: k,
                    action: step.action,
                    target: step.target,
                    start: now,
                    start_place: here.clone(),
                    start_tile: world.comp::<Position>(id).map(|p| p.tile),
                    goal: b.plan.as_ref().map(|p| p.goal),
                });
            }
        }
        t.plan_started = ps;
        // Sentence.
        let sent = world.comp::<Sentence>(id).map(|s| s.until_tick);
        if sent != t.sentence {
            match (t.sentence, sent) {
                (None, Some(until)) => {
                    let crime = world.comp::<Sentence>(id).map(|s| s.crime);
                    t.log(now, "law", format!("JAILED for {crime:?} until {}", clock(until)));
                }
                (Some(_), None) => t.log(now, "law", "released from jail".into()),
                _ => {}
            }
            t.sentence = sent;
        }
        // Gang orders.
        let gord = world.gang_of(id).and_then(|g| world.comp::<Gang>(g)).map(|g| format!("{:?}", g.order));
        if gord != t.gang_order {
            if let Some(o) = &gord {
                t.log(now, "order", format!("gang standing order is now {o}"));
            }
            t.gang_order = gord;
        }
        let fo = b.following_order.map(|o| format!("{o:?}"));
        if fo != t.following {
            match &fo {
                Some(o) => t.log(now, "order", format!("following gang order {o}")),
                None if t.following.is_some() => t.log(now, "order", "no longer following an order".into()),
                None => {}
            }
            t.following = fo;
        }
        if b.dazed_until.is_some_and(|u| u > now) && b.dazed_until != Some(0) && t.last_goal.is_some() {
            // quiet: dazed is visible in the activity class.
        }
        // Needs every two hours.
        if now.is_multiple_of(2 * TICKS_PER_HOUR) {
            if let (Some(n), Some(m)) = (world.comp::<Needs>(id), world.comp::<Mood>(id)) {
                let body = world
                    .comp::<Body>(id)
                    .map(|b| format!(" sanity {:.2} addiction {:.2}", b.sanity, b.addiction))
                    .unwrap_or_default();
                // Round 3 S7 (OPEN-225): the rest day in the hour lines too.
                let rest = t.rest_days.get(&day).copied().unwrap_or("").trim().trim_matches(['(', ')']);
                // Not while at Work: a night shift's tail runs into the rest day's morning.
                let rest = if rest.is_empty() || b.current_goal == Some(GoalKind::Work) {
                    String::new()
                } else {
                    format!(" | {rest}")
                };
                t.log(
                    now,
                    "needs",
                    format!(
                        "needs hunger {:.2} energy {:.2} safety {:.2} wealth {:.2} belonging {:.2} intimacy {:.2} fun {:.2} mood {:.2}{} | {:?} | {:?} | {}{rest}",
                        n.hunger, n.energy, n.safety, n.wealth, n.belonging, n.intimacy, n.fun, m.value, body, b.lod, b.current_goal, here
                    ),
                );
            }
        }
    }

    // The pressure pass S6 (OPEN-224): a child has no diary, so a parent's
    // reads the children once a day (09:30, the school day begun): where
    // each is today and the Home's pantry.
    if now % TICKS_PER_DAY == 9 * TICKS_PER_HOUR + 30 {
        let kids: Vec<EntityId> = citysim::systems::demography::children_of_agent(world, id)
            .into_iter()
            .filter(|&c| world.has::<Child>(c))
            .collect();
        if !kids.is_empty() {
            use citysim::systems::ages::Place;
            let parts: Vec<String> = kids
                .iter()
                .map(|&c| {
                    let yrs = world.comp::<citysim::Identity>(c).map_or(0, |i| i.age_years());
                    let why = school_why(world, c, now);
                    let at = match citysim::systems::ages::place_of(world, c) {
                        Some(Place::School(s)) => format!("at {}{why}", nm(world, s)),
                        Some(Place::Pitch(_)) => format!("at the pitch (chores{why})"),
                        // Round 5 U2 (`reporter_57837`): a child living
                        // elsewhere than the parent says where.
                        Some(Place::Home) => {
                            let ch = world.comp::<citysim::Household>(c).and_then(|h| h.home);
                            let mine = world.comp::<citysim::Household>(id).and_then(|h| h.home);
                            match ch.filter(|&h| Some(h) != mine) {
                                Some(h) => format!("at home at {} (play{why})", nm(world, h)),
                                None => format!("at home (play{why})"),
                            }
                        }
                        Some(Place::Street) => "on the street".to_string(),
                        Some(Place::Camp) => {
                            let camp = world.comp::<citysim::Household>(c).and_then(|h| h.home);
                            camp.map_or("in a Camp".to_string(), |b| format!("in {}", nm(world, b)))
                        }
                        None => "nowhere known".to_string(),
                    };
                    // Round 4 T1 (OPEN-231): the service's eye on the child
                    // (a warning inside its window) and the let-home count.
                    let camp_cfg = &world.config.camp;
                    let day = (now / TICKS_PER_DAY) as u32;
                    let ward = world.comp::<Child>(c).map_or(String::new(), |k| {
                        let mut v = String::new();
                        if let Some(w) =
                            k.warned_day.filter(|&w| day.saturating_sub(w) <= u32::from(camp_cfg.warn_repeat_days))
                        {
                            v.push_str(&format!(", on the service's warning since d{w}"));
                            // Round 7 W1 (OPEN-243): a warning standing is
                            // not a hang (`camp_parent_106052`'s 4-y): the
                            // child ate, or how long it has not.
                            if k.hunger_days == 0 {
                                v.push_str(" (fed last night)");
                            } else {
                                v.push_str(&format!(" (unfed {} nights)", k.hunger_days));
                            }
                        }
                        if k.home_fed_days > 0 {
                            v.push_str(&format!(
                                ", home after {} of {} nights of a held pantry",
                                k.home_fed_days, camp_cfg.let_home_days
                            ));
                        }
                        v
                    });
                    // Round 8 X2 (OPEN-247): a death in the household.
                    let grief =
                        citysim::systems::grief::family_note(world, c).map_or(String::new(), |g| format!(", {g}"));
                    format!("{} ({yrs} y) {at}{ward}{grief}", nm(world, c))
                })
                .collect();
            // Round 6 V1 (OPEN-231): in a shared Home, the household's share.
            let pantry = world.comp::<citysim::Household>(id).and_then(|h| h.home).map_or(String::new(), |h| {
                let stock = world.comp::<Building>(h).map_or(0, |b| b.stock_food);
                let k = citysim::systems::shelf::key(world, id);
                match citysim::systems::shelf::claim(world, h, k) {
                    Some(c) => format!(
                        "; the pantry holds {stock}, {:.0} of it ours ({} households under the roof)",
                        c.floor(),
                        citysim::systems::shelf::household_count(world, h)
                    ),
                    None => format!("; the pantry holds {stock}"),
                }
            });
            t.log(now, "family", format!("my children: {}{pantry}", parts.join(", ")));
        }
    }

    // Hunger as sickness: each meal by kind (the tally rises), and the
    // meters with the needs line.
    if let Some(hl) = world.comp::<citysim::Health>(id) {
        let now_t = hl.today;
        if let Some(prev) = t.tally {
            let rises = [
                (now_t.real > prev.real, "ate a REAL meal"),
                (now_t.synth > prev.synth, "ate SYNTH"),
                (now_t.dive > prev.dive, "ate scraps from a BIN"),
                (now_t.alms > prev.alms, "ate a Mission's SYNTH meal"),
            ];
            for (up, what) in rises {
                if up {
                    t.log(now, "meal", format!("{what} | {}", health_line(hl)));
                }
            }
            let (real, synth, dive, alms) = (
                now_t.real.saturating_sub(prev.real),
                now_t.synth.saturating_sub(prev.synth),
                now_t.dive.saturating_sub(prev.dive),
                now_t.alms.saturating_sub(prev.alms),
            );
            t.meals += u32::from(real) + u32::from(synth) + u32::from(dive) + u32::from(alms);
            t.meals_synth += u32::from(synth) + u32::from(alms);
            t.meals_dive += u32::from(dive);
        }
        t.tally = Some(now_t);
    }
    // Money.
    let mut money_logged = false;
    if let Some(w) = world.comp::<Wallet>(id) {
        if let Some(prev) = t.wallet {
            let d = w.coins - prev;
            if d != 0 {
                let midnight = now % TICKS_PER_DAY == TICKS_PER_DAY - 1 || now.is_multiple_of(TICKS_PER_DAY);
                // A step that changed this tick paid at its completion (wage, dole) or
                // at its start (a purchase): try the finished step first.
                let kind = match flow_kind(prev_action, midnight) {
                    "other" => flow_kind(action_now, midnight),
                    k => k,
                };
                // L1: a guard's shift clock pays at the shift's end, whatever the step.
                let shift_end = world.comp::<citysim::components::Job>(id).is_some_and(|j| {
                    let tod = (now % TICKS_PER_DAY) as u16;
                    let last = if tod == 0 { 1439 } else { tod - 1 };
                    j.on_shift(last) && !j.on_shift(tod)
                });
                let kind = if kind == "other" && d > 0 && shift_end { "wage" } else { kind };
                // L1: the dole paid in place at 09:00.
                let kind = if kind == "other"
                    && d > 0
                    && now % TICKS_PER_DAY == 540
                    && world.comp::<citysim::components::Job>(id).is_none()
                {
                    "dole"
                } else {
                    kind
                };
                // Shadow V4 bug 23: a Flow on the books names the money.
                let kind: String = tick_flow.as_ref().map_or_else(|| kind.to_string(), |(_, f)| f.clone());
                let kind = kind.as_str();
                let owner = bld.and_then(|b| owner_name(world, b));
                let cp = owner.map_or(String::new(), |o| format!("; place owner {o}"));
                let ev =
                    if event_ctx.is_empty() { String::new() } else { format!("; events: {}", event_ctx.join(" | ")) };
                let fl = if flow_ctx.is_empty() { String::new() } else { format!("; flows: {}", flow_ctx.join(", ")) };
                money_logged = true;
                if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
                    t.stat_notes.push((
                        now,
                        format!("stand-in purse {}{} ({kind}){fl}", if d > 0 { "+" } else { "" }, money(d)),
                    ));
                }
                t.log(
                    now,
                    "money",
                    format!(
                        "{}{} ({kind}), purse {}{cp}{ev}{fl}",
                        if d > 0 { "+" } else { "" },
                        money(d),
                        money(w.coins)
                    ),
                );
                t.day_net += d;
                t.total_net += d;
                let f = t.flows.entry(kind.to_string()).or_insert((0, 0));
                f.0 += d;
                f.1 += 1;
            }
        }
        t.wallet = Some(w.coins);
    }
    if !money_logged && !flow_ctx.is_empty() {
        t.log(now, "money", format!("(net 0c) flows: {}", flow_ctx.join(", ")));
    }
    // V2: off-screen holes against the agent (a Statistical victim's crimes, actor not yet drawn).
    // `Hole.source`/`faction`/`riot` come from the daily off-screen faction pass (L2 phase 4).
    if let Some(hs) = world.holes_by_agent.get(&id) {
        for hid in hs.iter() {
            if !t.holes_seen.insert(*hid) {
                continue;
            }
            if let Some(h) = world.holes.get(hid).filter(|h| h.tick + 2 >= now) {
                t.log(
                    h.tick,
                    "hole",
                    format!(
                        "OFF-SCREEN {:?} against me in {}{}{}{}{}{}",
                        h.kind,
                        world.district_name(h.district),
                        if h.loot > 0 { format!(" (loot {}c)", h.loot) } else { String::new() },
                        h.gang.map_or(String::new(), |g| format!(", my gang {}", nm(world, g))),
                        h.source.map_or(String::new(), |s| format!(", source {s:?}")),
                        h.faction.map_or(String::new(), |f| format!(", by {}", nm(world, f))),
                        h.riot.map_or(String::new(), |r| format!(", riot #{r}"))
                    ),
                );
            }
        }
    }
    // Inventory.
    if let Some(i) = world.comp::<Inventory>(id) {
        let cur = (i.food, i.stolen_food, i.stims, i.parts);
        if let Some(p) = t.inv {
            if p != cur {
                t.log(
                    now,
                    "inv",
                    format!(
                        "inventory food {}->{} (stolen {}->{}), stims {}->{}, parts {}->{}",
                        p.0, cur.0, p.1, cur.1, p.2, cur.2, p.3, cur.3
                    ),
                );
            }
        }
        t.inv = Some(cur);
    }
    let mut mem_msgs: Vec<(String, String)> = Vec::new();
    let mut edge_msgs: Vec<(String, String)> = Vec::new();
    // Round 4 T7: the minute's new encounters by shape (name, regular, off screen, line).
    let mut served: Vec<(String, Vec<Met>)> = Vec::new();
    // Memories.
    if let Some(m) = world.comp::<Memory>(id) {
        let prime = !t.mem_primed;
        t.mem_primed = true;
        for (e, heard) in m.entries.iter().map(|e| (e, false)).chain(m.heard.iter().map(|e| (e, true))) {
            let key = (e.tick, e.kind, e.subject, heard);
            if t.mem_seen.insert(key) && !prime {
                // Lives at work phase 1: an encounter on shift (the fourth Served-able signal).
                if e.kind == MemoryKind::Served && !heard {
                    if let Some(s) = t.shift.as_mut() {
                        s.encounters += 1;
                    }
                }
                if let Some(s) = e.subject {
                    if matches!(
                        e.kind,
                        MemoryKind::Socialised
                            | MemoryKind::Fought
                            | MemoryKind::Won
                            | MemoryKind::Lost
                            | MemoryKind::Bout
                            | MemoryKind::Courted
                    ) {
                        t.contact(world, s);
                    }
                }
                // Round 3 S7 (OPEN-225): a heard sighting refreshed hourly (a
                // colleague at his bench) is logged once a day per place.
                if e.kind == MemoryKind::Sighting {
                    if let Some(s) = e.subject {
                        if t.sighted.get(&s) == Some(&(e.at, day)) {
                            continue;
                        }
                        t.sighted.insert(s, (e.at, day));
                    }
                }
                // Round 4 T7 (OPEN-233): a petty crime's `SawCrime` is its
                // Witness line this tick, told once.
                if e.kind == MemoryKind::SawCrime
                    && !heard
                    && e.crime.is_some_and(citysim::systems::law::is_petty)
                    && e.subject.is_some_and(|s| petty_now.contains(&s))
                {
                    continue;
                }
                let text = mem_text(world, id, e);
                // Shadow V6: an encounter with the counterpart elsewhere.
                if e.kind == MemoryKind::Served && !heard {
                    if let Some(s) = e.subject {
                        // Round 4 T7: the encounter's line says "off screen"
                        // itself; the same pair in the same role again today
                        // (the CEO's hourly table, the madam's staff) is not
                        // news, and a minute's encounters of one shape fold.
                        let off = cam_settle_with(world, t, now, s, &format!("memory: {text}"), false);
                        let who = nm(world, s);
                        let body = text.rsplit_once(" (valence ").map_or(text.as_str(), |x| x.0);
                        let (body, regular) = match body.strip_suffix(", a regular") {
                            Some(b) => (b, true),
                            None => (body, false),
                        };
                        let shape = body.replacen(&who, "{}", 1);
                        if !t.served_today.insert((s, shape.clone())) {
                            continue;
                        }
                        match served.iter_mut().find(|g| g.0 == shape) {
                            Some(g) => g.1.push((who, regular, off, text)),
                            None => served.push((shape, vec![(who, regular, off, text)])),
                        }
                        continue;
                    }
                }
                // Rumours heard already appear as gossip lines when told in person.
                let old = if now.saturating_sub(e.tick) > 60 {
                    format!(" [deed from {}]", clock(e.tick))
                } else {
                    String::new()
                };
                if let (MemoryKind::MetInJail, Some(s)) = (e.kind, e.subject) {
                    let (name, line) = (nm(world, s), format!("{text}{old}"));
                    t.hourly.push(Quiet { tick: now, cat: "memory", key: "cells", name, line });
                } else {
                    mem_msgs.push((format!("{:?}", e.kind), format!("{text}{old}")));
                }
            }
        }
    }
    for (shape, g) in served {
        if let [(_, _, off, text)] = g.as_slice() {
            mem_msgs.push(("Served".into(), if *off { format!("{text} [off screen]") } else { text.clone() }));
            continue;
        }
        let names: Vec<&str> = g.iter().take(FOLD_NAMES).map(|x| x.0.as_str()).collect();
        let more = g.len() - names.len();
        let (regs, offs) = (g.iter().filter(|x| x.1).count(), g.iter().filter(|x| x.2).count());
        let mut tail = if more > 0 { format!(" and {more} more") } else { String::new() };
        if regs > 0 {
            tail.push_str(&format!("; {regs} regular{}", if regs == 1 { "" } else { "s" }));
        }
        if offs > 0 {
            tail.push_str(&format!("; {offs} off screen"));
        }
        let head = shape.replacen("{}", &g.len().to_string(), 1);
        mem_msgs.push(("Served".into(), format!("{head} ({}{tail})", names.join(", "))));
    }
    // Edges.
    let mut seen: BTreeSet<EntityId> = BTreeSet::new();
    let ns: Vec<EntityId> = world.neighbours(id).collect();
    for o in ns {
        let Some(e) = world.edge(id, o) else { continue };
        seen.insert(o);
        match t.edges.get(&o).copied() {
            None => {
                if t.edges_primed {
                    let line =
                        format!("new relationship with {}: {:?} affinity {:+.2}", nm(world, o), e.kind, e.affinity);
                    if e.kind == RelKind::Acquaintance {
                        t.hourly.push(Quiet { tick: now, cat: "edge", key: "new", name: nm(world, o), line });
                    } else {
                        edge_msgs.push((format!("new {:?}", e.kind), line));
                    }
                }
                t.edges.insert(o, (e.affinity, e.kind));
            }
            Some((aff, kind)) => {
                if kind != e.kind {
                    edge_msgs.push((
                        format!("now {:?}", e.kind),
                        format!("{} is now {:?} (was {:?}), affinity {:+.2}", nm(world, o), e.kind, kind, e.affinity),
                    ));
                    t.edges.insert(o, (e.affinity, e.kind));
                } else if (e.affinity - aff).abs() >= 0.1 {
                    let dir = if e.affinity > aff { "warmer" } else { "colder" };
                    let line = format!("affinity to {} {:+.2} -> {:+.2} ({:?})", nm(world, o), aff, e.affinity, e.kind);
                    if e.kind == RelKind::Acquaintance {
                        t.hourly.push(Quiet { tick: now, cat: "edge", key: dir, name: nm(world, o), line });
                    } else {
                        edge_msgs.push((format!("{dir} {:?}", e.kind), line));
                    }
                    t.edges.insert(o, (e.affinity, e.kind));
                }
            }
        }
    }
    t.edges.retain(|k, _| seen.contains(k));
    log_grouped(t, now, "memory", mem_msgs, "memories");
    log_grouped(t, now, "edge", edge_msgs, "relationship changes");
    t.edges_primed = true;
    // Grudges and Hunts.
    if let Some(Some(gs)) = world.grudges.get(id.index as usize) {
        for g in &gs.list {
            if t.grudges.insert((g.target, g.since)) {
                t.log(
                    now,
                    "grudge",
                    format!(
                        "GRUDGE against {} ({:?}, weight {:.2}, chain {})",
                        nm(world, g.target),
                        g.cause,
                        g.weight,
                        g.chain
                    ),
                );
            }
        }
    }
    let hunt = world.hunts.get(&id).map(|h| h.target);
    if hunt != t.hunt {
        match (hunt, world.hunts.get(&id)) {
            (Some(tg), Some(h)) => {
                t.log(now, "grudge", format!("HUNT begins on {} ({:?}, phase {:?})", nm(world, tg), h.why, h.phase))
            }
            _ => t.log(now, "grudge", "hunt over".into()),
        }
        t.hunt = hunt;
    }
}

/// The behaviour tier's per-minute tallies (sleep blocks, energy 0, shifts,
/// the Work plan's commute). Reads only; no RNG.
fn behaviour_minute(world: &World, t: &mut Track, now: Tick, act: &'static str) {
    let id = t.id;
    t.alive_min += 1;
    t.home_tier = world
        .comp::<citysim::components::Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<citysim::Building>(h))
        .map(|b| b.tier);
    if act == "work" {
        t.work_min += 1;
    }
    if act == "sleep" {
        t.sleep_run += 1;
    } else {
        end_sleep_block(t, now);
    }
    let jailed = world.has::<Sentence>(id);
    // Free minutes only: a pinned prisoner is kept at Full in the cells, which the
    // city never does (a sentenced agent is held Statistical, `lod::held_on`).
    if !jailed && world.comp::<Needs>(id).is_some_and(|n| n.energy <= 0.001) {
        t.energy0_min += 1;
    }
    let starving = world.config.life.starving_hunger;
    if !jailed && world.comp::<Needs>(id).is_some_and(|n| n.hunger < starving) {
        t.starve_min += 1;
    }
    let tod = (now % TICKS_PER_DAY) as u16;
    // Lives at work phase 5: an exec's office hours are his shift for `--lives`.
    let (os, oe) = world.config.life.exec_shift;
    let office = (!world.has::<Job>(id) && tod >= os && tod < oe)
        .then(|| citysim::systems::life::exec_corp(world, id))
        .flatten();
    match (world.comp::<Job>(id), office) {
        (Some(j), _) if j.on_shift(tod) => {
            let s = t.shift.get_or_insert_with(|| ShiftObs {
                start: now,
                end: now,
                len: 0,
                work: 0,
                workday: citysim::exec::routine::workday_of(world, id, j, j.shift_key_at(now)),
                whole: j.shifts.iter().any(|&(s, _)| s == tod),
                free: true,
                employer: j.employer,
                own_post: j.employer.is_some_and(|e| world.owner_of(e) == Some(id)),
                samples: 0,
                nonstaff: 0,
                source_events: 0,
                venue_visits: 0,
                last_visits: None,
                encounters: 0,
                office: false,
            });
            s.len += 1;
            s.work += u32::from(act == "work");
            s.free &= !jailed;
            s.end = now;
        }
        (None, Some(corp)) => {
            let day = (now / TICKS_PER_DAY) as i64;
            let s = t.shift.get_or_insert_with(|| ShiftObs {
                start: now,
                end: now,
                len: 0,
                work: 0,
                workday: citysim::exec::routine::is_workday(day),
                whole: tod == os,
                free: true,
                employer: citysim::systems::life::hq_of(world, corp, id),
                own_post: false,
                samples: 0,
                nonstaff: 0,
                source_events: 0,
                venue_visits: 0,
                last_visits: None,
                encounters: 0,
                office: true,
            });
            s.len += 1;
            s.work += u32::from(act == "work");
            s.free &= !jailed;
            s.end = now;
        }
        _ => {
            if let Some(s) = t.shift.take() {
                t.shifts.push(s);
            }
        }
    }
    let Some(b) = world.comp::<Brain>(id) else { return };
    // A run of the Work goal (successive Work plans included: the walk and the
    // wait at the door, then the shift's own plan) that ends before a single
    // work-class minute is a commute turned back.
    if b.current_goal == Some(GoalKind::Work) {
        let reached = t.work_plan.get_or_insert(false);
        *reached |= act == "work";
    } else if let Some(reached) = t.work_plan.take() {
        t.commute_aborts += u32::from(!reached);
    }
}

pub(super) fn end_sleep_block(t: &mut Track, now: Tick) {
    if t.sleep_run > 0 {
        t.sleep_blocks.push((now, t.sleep_run));
        t.sleep_run = 0;
    }
}

/// Lives at work phase 0 (`--lives`): the spouse and the counter, read once
/// an hour (plus the tick's events at the workplace). Reads only; no RNG.
/// Runs after `behaviour_minute`, so `t.shift` is this minute's.
fn lives_minute(world: &World, t: &mut Track, now: Tick, act: &'static str, events: &[&Event]) {
    let id = t.id;
    // The tick's source events at the workplace (every tick: an event is one minute).
    if let Some(s) = t.shift.as_mut() {
        if let Some(wp) = s.employer {
            for e in events.iter().filter(|e| SERVED_SOURCE_KINDS.contains(&e.kind)) {
                let at_work = e.actors.contains(&wp)
                    || e.actors.iter().any(|&a| {
                        a != id && a != EntityId::NONE && world.comp::<Position>(a).and_then(|p| p.building) == Some(wp)
                    });
                if at_work {
                    s.source_events += 1;
                    *t.source_kinds.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
                }
            }
        }
    }
    if !now.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    // The counter: non-staff bodies inside the workplace, and the venue's visit tally.
    if let Some(s) = t.shift.as_mut() {
        if let Some(bd) = s.employer.and_then(|wp| world.comp::<Building>(wp)) {
            let staff = world.staff_of(s.employer.unwrap_or(EntityId::NONE));
            s.samples += 1;
            s.nonstaff += bd.occupants.iter().filter(|&&o| o != id && staff.binary_search(&o).is_err()).count() as u32;
            if let Some(v) = bd.venue.as_ref() {
                // `visits_today` resets at midnight: a fall is a new day's count.
                let cur = v.visits_today;
                if let Some(prev) = s.last_visits {
                    s.venue_visits += u32::from(if cur >= prev { cur - prev } else { cur });
                }
                s.last_visits = Some(cur);
            }
        }
    }
    // The spouse: co-located and both awake.
    let Some(sp) = t.spouse else { return };
    if !world.has::<Brain>(sp) || world.has::<Corpse>(sp) {
        return;
    }
    t.spouse_samples += 1;
    let (Some(a), Some(b)) = (world.comp::<Position>(id), world.comp::<Position>(sp)) else { return };
    let together = match (a.building, b.building) {
        (Some(x), Some(y)) => x == y,
        (None, None) => a.tile.manhattan(b.tile) <= COLOC_TILES,
        _ => false,
    };
    let sp_act = activity(world, sp);
    if together && act != "sleep" && sp_act != "sleep" {
        t.spouse_coloc += 1;
        t.spouse_coloc_stat += u32::from(sp_act == "stat");
    }
}

/// Log up to `FOLD_NAMES` lines, then one line counting the rest by kind (a
/// gang-wide feud adjustment touches dozens of edges in one minute; round 3
/// S7: a raid's 259). Each message is `(kind, line)`; a fold of one line is
/// the line.
fn log_grouped(t: &mut Track, now: Tick, cat: &'static str, msgs: Vec<(String, String)>, noun: &str) {
    let n = msgs.len();
    let shown = if n <= FOLD_NAMES + 1 { n } else { FOLD_NAMES };
    let mut by: BTreeMap<String, u32> = BTreeMap::new();
    for (i, (kind, m)) in msgs.into_iter().enumerate() {
        if i < shown {
            t.log(now, cat, m);
        } else {
            *by.entry(kind).or_insert(0) += 1;
        }
    }
    if n > shown {
        let mut kinds: Vec<(String, u32)> = by.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let parts: Vec<String> = kinds.iter().map(|(k, c)| format!("{c} {k}")).collect();
        t.log(now, cat, format!("... and {} more {noun} in the same minute: {}", n - shown, parts.join(", ")));
    }
}
