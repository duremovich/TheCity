use super::*;

pub(super) fn cells_daily(world: &mut World, c: &ConsequenceCfg) {
    let today = world.day();
    // Prisoners gone (dead, broken out) drop their run.
    let runs: Vec<EntityId> = world.consequence.inside.iter().map(|x| x.0).collect();
    for p in runs {
        if !world.has::<Sentence>(p) || !living(world, p) {
            world.consequence.inside.retain(|x| x.0 != p);
        }
    }
    // Round 8 X6 (`OPEN-217`): yesterday's prisoner-days in the cells, and
    // those a line named (`note_line`), before today's lines start.
    let lined = std::mem::take(&mut world.consequence.lined);
    let (mut days, mut with_line) = (0u32, 0u32);
    for (p, x) in &world.consequence.inside {
        if x.since < today && world.has::<Sentence>(*p) {
            days += 1;
            with_line += u32::from(lined.contains(p));
        }
    }
    let row = ownership::jobs2_row(world);
    row.cell_days += days;
    row.cell_days_lined += with_line;
    let yard = c.cell_yard_hour < 24;
    for (jail, prisoners) in cells_now(world) {
        let mut events = 0u16;
        let mut fought = std::collections::BTreeSet::new();
        for &p in &prisoners {
            if !living(world, p) || !world.has::<Sentence>(p) {
                continue;
            }
            jail_absence(world, p);
            if yard || events >= c.cell_events_max || prisoners.len() < 2 {
                continue;
            }
            if cell_day(world, c, jail, p, &prisoners, today, &mut fought) {
                events += 1;
            }
        }
    }
}

/// The prisoners in the cells now, by the building they are held in.
fn cells_now(world: &World) -> std::collections::BTreeMap<EntityId, Vec<EntityId>> {
    let mut cells: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for &p in world.sentenced() {
        if !person(world, p) {
            continue;
        }
        if let Some(b) = world.comp::<Position>(p).and_then(|x| x.building) {
            cells.entry(b).or_default().push(p);
        }
    }
    cells
}

/// Round 8 X6 (`OPEN-217`): `visitor` came to `prisoner` in the cells
/// (`household::credit`, a body's walk or the Statistical hour): the
/// prisoner's belonging up `gain` too, and the news he brings from outside
/// (the post gone, the Home gone), else how long is left.
pub fn cell_visit(world: &mut World, visitor: EntityId, prisoner: EntityId, gain: f32) {
    if let Some(n) = world.comp_mut::<Needs>(prisoner) {
        n.belonging = (n.belonging + gain).min(1.0);
    }
    let cell = world.comp::<Position>(prisoner).and_then(|p| p.building);
    let at = cell.map_or_else(|| "the cells".to_string(), |b| format!("the cells of {}", world.name_of(b)));
    let him = pro(world, prisoner).1;
    let x = world.consequence.inside.iter().find(|(p, _)| *p == prisoner).map(|(_, x)| x.clone());
    let mut news: Vec<String> = Vec::new();
    if let Some((role, employer)) = x.as_ref().and_then(|x| x.job.clone()) {
        if !world.comp::<Job>(prisoner).is_some_and(|j| j.employer == employer) {
            let place = employer.map(|e| format!(" at {}", world.name_of(e))).unwrap_or_default();
            news.push(format!("the post as {role}{place} is gone"));
        }
    }
    if let Some(h) = x.as_ref().and_then(|x| x.home) {
        if world.comp::<crate::components::Household>(prisoner).and_then(|hh| hh.home) != Some(h) {
            news.push(format!("the Home at {} is gone", world.name_of(h)));
        }
    }
    let told = if news.is_empty() {
        let left = world
            .comp::<Sentence>(prisoner)
            .map_or(0, |s| s.until_tick.saturating_sub(world.tick).div_ceil(crate::time::TICKS_PER_DAY));
        format!("; {left} day{} left for {him}", if left == 1 { "" } else { "s" })
    } else {
        format!(" and told {him} {}", news.join(" and "))
    };
    let text = format!("{} came to {at} to see {}{told}", world.name_of(visitor), world.name_of(prisoner));
    let mut actors = vec![visitor, prisoner];
    actors.extend(cell);
    world.push_event(EventKind::Visited, &actors, text);
    ownership::jobs2_row(world).cell_visits += 1;
}

/// Round 8 X6 (`OPEN-217`): a line named `actors` (`World::push_event`): a
/// prisoner among them has a line today.
pub fn note_line(world: &mut World, actors: &[EntityId]) {
    for &a in actors {
        if world.has::<Sentence>(a) {
            world.consequence.lined.insert(a);
        }
    }
}

/// Round 8 X6 (`OPEN-217`), hourly (from `living::run`): the yard at
/// `[consequence] cell_yard_hour`. Each prisoner's day among his cellmates
/// (`cell_day`: at most one thing, `cell_events_max` a jail), else a walk
/// with the cellmate he likes best or his own company, a line either way.
/// Tier-blind (the held walk it as the bodies do). Under 18: a child is
/// never held, and `cell_day`'s pick skips the protected.
pub fn hourly(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    if c.cell_yard_hour >= 24 || !world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        return;
    }
    if world.tick_of_day() / crate::time::TICKS_PER_HOUR as u16 != u16::from(c.cell_yard_hour) {
        return;
    }
    let today = world.day();
    for (jail, prisoners) in cells_now(world) {
        let mut events = 0u16;
        let mut fought = std::collections::BTreeSet::new();
        for &p in &prisoners {
            // `fought`: the day's taken (a fight's two, a walk's pair): one
            // thing a prisoner a day, one line a walk.
            if !living(world, p) || !world.has::<Sentence>(p) || fought.contains(&p) {
                continue;
            }
            if events < c.cell_events_max
                && prisoners.len() >= 2
                && cell_day(world, &c, jail, p, &prisoners, today, &mut fought)
            {
                events += 1;
                continue;
            }
            if fought.contains(&p) || !living(world, p) {
                continue;
            }
            // V8 (`OPEN-256`): cards with a cellmate, else the walk.
            let yard = yard_of(world, jail);
            if crate::systems::cells::cards(world, &yard, jail, p, &prisoners, &mut fought) {
                continue;
            }
            yard_walk(world, &c, jail, p, &prisoners, &mut fought);
        }
    }
}

/// X6: the Precinct's yard; a Station's night cells have none.
fn yard_of(world: &World, jail: EntityId) -> String {
    if crate::systems::stations::is_station(world, jail) {
        format!("the cells of {}", world.name_of(jail))
    } else {
        format!("the yard of {}", world.name_of(jail))
    }
}

/// X6: `p`'s hour in the yard with nothing on: a walk with the cellmate he
/// likes best not yet walking (an edge at or above 0; their affinity up
/// `cell_yard_affinity`; the pair `taken`, one line for the two) and what
/// that one is in for, else his own company.
fn yard_walk(
    world: &mut World,
    c: &ConsequenceCfg,
    jail: EntityId,
    p: EntityId,
    mates: &[EntityId],
    taken: &mut std::collections::BTreeSet<EntityId>,
) {
    let best = mates
        .iter()
        .copied()
        .filter(|&m| m != p && !taken.contains(&m) && living(world, m) && world.has::<Sentence>(m))
        .filter_map(|m| world.edge(p, m).filter(|e| e.affinity >= 0.0).map(|e| (e.affinity, m)))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let pn = world.name_of(p);
    let text = match best {
        Some((_, m)) => {
            crate::systems::social::adjust(world, p, m, c.cell_yard_affinity, 0.0);
            let what = world
                .comp::<Sentence>(m)
                .map(|s| format!(" (in for {}, out d{})", s.crime.label(), s.until_tick / crate::time::TICKS_PER_DAY))
                .unwrap_or_default();
            format!("{pn} walked {} with {}{what}", yard_of(world, jail), world.name_of(m))
        }
        None => {
            let n = mates.len().saturating_sub(1);
            let he = pro(world, p).0;
            format!("{pn} walked {} alone: {n} in the cells, none {he} would walk with", yard_of(world, jail))
        }
    };
    let mut actors = vec![p, jail];
    actors.extend(best.map(|b| b.1));
    taken.extend(actors.iter().copied().filter(|&a| a != jail));
    world.push_event(EventKind::Cells, &actors, text);
    ownership::jobs2_row(world).cell_yard_walks += 1;
}

/// A jailed worker's workday: an absence on phase 1's count.
fn jail_absence(world: &mut World, p: EntityId) {
    let Some(job) = world.comp::<Job>(p).cloned() else { return };
    let key = job.next_shift_key(world.tick);
    if !crate::exec::routine::workday_of(world, p, &job, key) {
        return;
    }
    let Some((count, window, fired)) = crate::systems::misery::jail_absence(world, p) else { return };
    let mut actors = vec![p];
    actors.extend(job.employer);
    let text = format!(
        "{} missed the shift as {} in the cells (absence {count} in {window} days{})",
        world.name_of(p),
        job.role.label(),
        if fired { ": dismissed" } else { "" }
    );
    world.push_event(EventKind::Cells, &actors, text);
    if let Some(x) = world.consequence.inside_mut(p) {
        x.missed = x.missed.saturating_add(1);
    }
    let row = ownership::jobs2_row(world);
    row.jail_absences += 1;
    if fired {
        row.jail_fired += 1;
    }
}

/// One prisoner's day among his cellmates: at most one thing happens.
#[allow(clippy::too_many_arguments)]
fn cell_day(
    world: &mut World,
    c: &ConsequenceCfg,
    jail: EntityId,
    p: EntityId,
    mates: &[EntityId],
    day: u64,
    fought: &mut std::collections::BTreeSet<EntityId>,
) -> bool {
    let t = traits(world, p);
    let others: Vec<EntityId> = mates.iter().copied().filter(|&m| m != p && living(world, m)).collect();
    if others.is_empty() {
        return false;
    }
    let pn = world.name_of(p);
    let roll = |world: &World, salt: u64| hash(world, P_CELL, p, day ^ (salt << 48));
    // A fight with a cellmate he dislikes.
    let dislike = |world: &World, m: EntityId| -> Option<&'static str> {
        if crate::systems::grudges::holds(world, p, m, 0.0) {
            return Some("an old grudge");
        }
        if let (Some(a), Some(b)) = (world.gang_of(p), world.gang_of(m)) {
            if a != b {
                return Some("the other colours");
            }
        }
        world.edge(p, m).filter(|e| e.affinity < c.cell_fight_affinity).map(|_| "bad blood")
    };
    // Pressure pass round 9, Z1: a grudge over a killing (or one handed
    // inside) on a cellmate: he goes for him on `[hunt] inside_fight_p`.
    let blood_mark = if fought.contains(&p) {
        None
    } else {
        let free: Vec<EntityId> = others.iter().copied().filter(|m| !fought.contains(m)).collect();
        crate::systems::hunt::inside_mark(world, p, &free, roll(world, 6))
    };
    if !fought.contains(&p) && (blood_mark.is_some() || roll(world, 1) < c.cell_fight_p * (0.5 + t.pride)) {
        // Round 8 X1: the under-18 rule at the pick (a child is never held;
        // `resolve_fight_mods` is the backstop). Round 8 X6 (`OPEN-217`): a
        // cellmate in a fight today is in no second one, and the one he
        // goes for is drawn among those he dislikes, not the first of the
        // list (V16's midnight: eleven haters on one cellmate in a night).
        let disliked: Vec<(EntityId, &'static str)> = others
            .iter()
            .copied()
            .filter(|m| !fought.contains(m))
            .filter(|&m| !crate::systems::ages::is_protected(world, m) && !crate::systems::ages::is_protected(world, p))
            .filter_map(|m| dislike(world, m).map(|w| (m, w)))
            .collect();
        let pick = blood_mark.map(|m| (m, "a killing to answer")).or_else(|| {
            (!disliked.is_empty()).then(|| {
                let i = ((roll(world, 5) * disliked.len() as f32) as usize).min(disliked.len() - 1);
                disliked[i]
            })
        });
        if let Some((m, why)) = pick {
            fought.insert(p);
            fought.insert(m);
            let mods = crate::systems::law::FightMods { kill_mult: c.cell_kill_mult, ..Default::default() };
            let place = if c.cell_yard_hour < 24 {
                yard_of(world, jail)
            } else {
                format!("the cells of {}", world.name_of(jail))
            };
            // C3a: live when both are Full; the tail is `cell_fight_end`.
            let tail = CellFight { jail, why: why.to_string(), place };
            crate::systems::combat::fight(world, p, m, mods, crate::systems::combat::Tail::Cells(Box::new(tail)));
            for x in [p, m] {
                if let Some(r) = world.consequence.inside_mut(x) {
                    r.fights = r.fights.saturating_add(1);
                }
            }
            ownership::jobs2_row(world).cell_fights += 1;
            return true;
        }
    }
    // A member cellmate's offer.
    if world.gang_of(p).is_none() && !crate::systems::founding::is_exec(world, p) {
        let m = crate::systems::misery::misery(world, p);
        if roll(world, 2) < c.cell_recruit_p * (1.0 - t.lawful) * (0.5 + m) {
            let max = world.config.gangs.max_members;
            let now = world.tick;
            let offer = others.iter().copied().find_map(|o| {
                let g = world.comp::<GangMember>(o)?.gang;
                let gang = world.comp::<Gang>(g)?;
                let open =
                    gang.members.len() < max && !gang.is_sacked(now) && crate::systems::creeds::tolerated(world, g, p);
                open.then_some((o, g))
            });
            if let Some((o, g)) = offer {
                crate::systems::gang::enlist(world, p, g);
                let text = format!("{} brought {pn} into {} in the cells", world.name_of(o), gang_name(world, g));
                world.push_event(EventKind::Cells, &[p, o, g], text);
                if let Some(r) = world.consequence.inside_mut(p) {
                    r.joined = Some(g);
                }
                ownership::jobs2_row(world).cell_recruits += 1;
                return true;
            }
        }
    }
    // A Fixer's name.
    if roll(world, 3) < c.cell_contact_p && !regular_anywhere(world, p) {
        if let Some((o, f)) = others.iter().copied().find_map(|o| fixer_known(world, o).map(|f| (o, f))) {
            let now = world.tick;
            if let Some(k) = world.comp_mut::<Broker>(f) {
                k.regulars.insert(p, now);
            }
            let text = format!("{} gave {pn} a name in the cells: ask at {}", world.name_of(o), world.name_of(f));
            world.push_event(EventKind::Cells, &[p, o, f], text);
            if let Some(r) = world.consequence.inside_mut(p) {
                r.fixer = Some(f);
            }
            ownership::jobs2_row(world).cell_contacts += 1;
            return true;
        }
    }
    // A loan from a cellmate.
    if world.purse(Some(p)) < c.cell_loan_broke && roll(world, 4) < c.cell_loan_p {
        let lender = others.iter().copied().find(|&o| {
            world.purse(Some(o)) >= 2 * c.cell_loan_coins && world.edge(p, o).is_none_or(|e| e.affinity >= 0.0)
        });
        if let Some(o) = lender {
            let lent = ownership::pay(world, Some(o), Some(p), c.cell_loan_coins, Flow::Finance);
            if lent > 0 {
                crate::systems::underwater::owe(world, p, o, lent);
                let text = format!("{} lent {pn} {lent}c in the cells", world.name_of(o));
                world.push_event(EventKind::Cells, &[p, o], text);
                if let Some(r) = world.consequence.inside_mut(p) {
                    r.owed.push((o, lent));
                }
                ownership::jobs2_row(world).cell_loans += 1;
                return true;
            }
        }
    }
    false
}
