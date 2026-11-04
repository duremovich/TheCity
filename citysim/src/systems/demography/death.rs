use super::*;

// ---------------------------------------------------------------------------
// Death effects, corpses and burial
// ---------------------------------------------------------------------------

/// Called by `World::kill` before the components go: witnesses, grief,
/// inheritance. M13 D13: with `[assets] loot_window_hours > 0` the coins
/// and goods stay on the body (returned, for the Corpse) until stripped,
/// buried or settled; with 0 (or assets off) the coins are inherited at
/// once, exactly as M12, and the loot is empty.
pub fn on_death(world: &mut World, id: EntityId) -> crate::components::Loot {
    let tick = world.tick;
    let (tile, building) = world.comp::<Position>(id).map_or((TilePos::default(), None), |p| (p.tile, p.building));
    // 4. SawCorpse to Brain agents within 6 tiles or in the same building.
    let sight = world.config.crime.sight;
    // Scale S1: the position index's neighbourhood (and the building), not
    // every citizen; the same citizens, ascending.
    // Round 5 U4 (`OPEN-236`): never through a wall (`law::sees_scene`).
    let out = crate::systems::law::outdoors(world, tile, building);
    let witnesses: Vec<EntityId> = world
        .agents_near(tile, sight, building)
        .into_iter()
        .filter(|&o| o != id && world.has::<Brain>(o) && world.has::<Identity>(o) && !world.has::<Corpse>(o))
        .filter(|&o| {
            world
                .comp::<Position>(o)
                .is_some_and(|p| crate::systems::law::sees_scene(world, p, tile, building, out, sight))
        })
        .collect();
    for w in witnesses {
        world.remember(w, MemoryKind::SawCorpse, Some(id), 0.6, -0.5, false);
    }
    // `social::on_death` writes Grief, frees the widow(er) to remarry and
    // drifts them; the date of widowhood is recorded here.
    if let Some(spouse) = world.spouse_of(id) {
        if let Some(i) = world.comp_mut::<Identity>(spouse) {
            i.spouse_died_tick = Some(tick);
        }
    }
    // Round 8 X2 (`OPEN-247`): the household mourns (`[grief]`).
    crate::systems::grief::on_death(world, id);
    // M11 D45: owned buildings pass to the spouse, a child, else the city.
    crate::systems::ownership::on_owner_gone(world, id);
    // M13 D45: so do parked vehicles and posted robots (carried and
    // installed stay on the body).
    crate::systems::assets::on_owner_gone(world, id);
    // 6. Inheritance: spouse, else children equally (remainder to the first), else Treasury.
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins).max(0);
    let window = world.config.assets.enabled && world.config.assets.loot_window_hours > 0;
    if window {
        let mut loot = crate::components::Loot { coins, ..Default::default() };
        if let Some(inv) = world.comp_mut::<Inventory>(id) {
            loot.food = std::mem::take(&mut inv.food);
            inv.stolen_food = 0;
            loot.stims = std::mem::take(&mut inv.stims);
            loot.parts = std::mem::take(&mut inv.parts);
        }
        if let Some(w) = world.comp_mut::<Wallet>(id) {
            w.coins = 0;
        }
        return loot;
    }
    if coins > 0 {
        inherit_coins(world, id, coins);
        if let Some(w) = world.comp_mut::<Wallet>(id) {
            w.coins = 0;
        }
    }
    crate::components::Loot::default()
}

/// M11 inheritance's heirs: the spouse (a living Spouse edge), else the
/// living children; empty = the Treasury.
pub fn coin_heirs(world: &World, dead: EntityId) -> Vec<EntityId> {
    let spouse = world.kin_candidates(dead).find(|&o| {
        world.edge(dead, o).is_some_and(|e| e.kind == RelKind::Spouse)
            && world.has::<Identity>(o)
            && !world.has::<Corpse>(o)
    });
    match spouse {
        Some(s) => vec![s],
        None => children_of_agent(world, dead),
    }
}

/// Inheritance of `coins` (M11, factored out for M13 D13): the spouse, else
/// the children equally (remainder to the first), else the Treasury. The
/// coins are no purse's when this runs (a wallet about to go, a corpse's loot).
pub fn inherit_coins(world: &mut World, dead: EntityId, coins: i64) {
    if coins <= 0 {
        return;
    }
    let heirs = coin_heirs(world, dead);
    if heirs.is_empty() {
        if let Some(t) = world.treasury_mut() {
            t.coins += coins;
        }
        world.push_event(EventKind::Inheritance, &[dead], format!("{coins} coins to the Treasury"));
    } else {
        let n = heirs.len() as i64;
        let share = coins / n;
        let mut remainder = coins - share * n;
        for h in &heirs {
            let got = share + remainder;
            remainder = 0;
            if let Some(w) = world.comp_mut::<Wallet>(*h) {
                w.coins += got;
            }
        }
        let name = world.name_of(heirs[0]);
        world.push_event(
            EventKind::Inheritance,
            &[dead, heirs[0]],
            format!("{coins} coins to {name}{}", if n > 1 { " and others" } else { "" }),
        );
    }
}

/// Daily: old unburied corpses frighten the neighbourhood, very old ones rot
/// away, buried ones are freed a day after burial.
pub(super) fn corpses(world: &mut World) {
    let tick = world.tick;
    let sight = world.config.crime.sight;
    let mut freed = Vec::new();
    // Scale S1: the living with a Brain and a Position, bucketed by tile once
    // (nobody moves or dies inside this pass), so a rotting body reads its
    // neighbourhood, not the city. Built on the first such body.
    let mut seers: Option<(Vec<(EntityId, TilePos)>, crate::spatial::TileGrid)> = None;
    let mut near_idx: Vec<u32> = Vec::new();
    for c in world.with::<Corpse>() {
        let Some(corpse) = world.comp::<Corpse>(c).cloned() else { continue };
        let age = tick.saturating_sub(corpse.died_tick);
        if corpse.buried {
            if corpse.buried_tick.is_some_and(|b| tick.saturating_sub(b) >= TICKS_PER_DAY) {
                freed.push((c, false));
            }
            continue;
        }
        if age >= 10 * TICKS_PER_DAY {
            freed.push((c, true));
            continue;
        }
        // M12 D17: an unburied body a day old fouls the street around it.
        if age >= TICKS_PER_DAY {
            if let Some((t, b)) = world.comp::<Position>(c).map(|p| (p.tile, p.building)) {
                crate::systems::litter::deposit_near(world, t, b, 24, 0);
            }
        }
        if age > 2 * TICKS_PER_DAY {
            let tile = world.comp::<Position>(c).map_or(TilePos::default(), |p| p.tile);
            let (list, grid) = seers.get_or_insert_with(|| {
                let list: Vec<(EntityId, TilePos)> = world
                    // scan-ok: daily, once a pass (the corpses' seers)
                    .citizens()
                    .into_iter()
                    .filter(|&o| world.has::<Brain>(o))
                    .filter_map(|o| world.comp::<Position>(o).map(|p| (o, p.tile)))
                    .collect();
                let grid =
                    crate::spatial::TileGrid::build(world.map.w(), world.map.h(), sight + 1, list.iter().map(|x| x.1));
                (list, grid)
            });
            near_idx.clear();
            grid.near(tile, sight, &mut near_idx);
            // `list` is ascending by id, so ascending indices are ascending ids.
            near_idx.sort_unstable();
            let near: Vec<EntityId> = near_idx
                .iter()
                .map(|&i| list[i as usize])
                .filter(|&(_, t)| crate::systems::law::chebyshev(t, tile) <= sight)
                .map(|(o, _)| o)
                .collect();
            for o in near {
                if let Some(n) = world.comp_mut::<Needs>(o) {
                    n.safety = (n.safety - 0.1).max(0.0);
                }
                world.remember(o, MemoryKind::SawCorpse, Some(c), 0.4, -0.4, false);
            }
        }
    }
    for (c, rotted) in freed {
        if rotted {
            let name = world.name_of(c);
            world.push_event(EventKind::Rotted, &[c], format!("the body of {name} rotted away"));
        }
        free_corpse(world, c);
    }
}

/// Round 6 V3 (`OPEN-238`): every `[crime.case] find_every_ticks`, each
/// unburied, unfound body of violence is looked for: the first waking body
/// (ascending) that sees it (`law::sees_scene` within `[crime] sight`, never
/// through a wall), not its killer, finds it (`found`). V14 `courier_48760
/// d25 05:25`: a murder and a strip in a Block, the killer at work at 08:10,
/// and the body lay with no line at all (SawCorpse came only at the death's
/// minute and after two days; nothing turned it into a case).
pub fn find_bodies(world: &mut World) {
    let every = world.config.crime.case.find_every_ticks;
    if every == 0 || !world.tick.is_multiple_of(every) {
        return;
    }
    let unfound: Vec<EntityId> = world
        .with::<Corpse>()
        .into_iter()
        .filter(|&c| world.comp::<Corpse>(c).is_some_and(|k| !k.found && !k.buried && k.cause == DeathCause::Violence))
        .collect();
    if unfound.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    for c in unfound {
        let Some((tile, building)) = world.comp::<Position>(c).map(|p| (p.tile, p.building)) else { continue };
        let killer = world.comp::<Corpse>(c).and_then(|k| k.killer);
        let out = crate::systems::law::outdoors(world, tile, building);
        let finder = world.agents_near(tile, sight, building).into_iter().find(|&o| {
            o != c
                && Some(o) != killer
                && world.has::<Identity>(o)
                && !world.has::<Corpse>(o)
                && world.comp::<Brain>(o).is_some_and(|b| {
                    b.lod != Lod::Statistical
                        && !matches!(b.exec, crate::exec::ExecState::Use { kind: crate::goap::ActionKind::Sleep, .. })
                })
                && !crate::systems::wounded::is_down(world, o)
                // Round 8 X1 (`OPEN-246`): the held find only a body in their own cells.
                && (!world.has::<crate::components::Sentence>(o)
                    || (building.is_some() && world.comp::<Position>(o).and_then(|p| p.building) == building))
                && world
                    .comp::<Position>(o)
                    .is_some_and(|p| crate::systems::law::sees_scene(world, p, tile, building, out, sight))
        });
        if let Some(f) = finder {
            found(world, c, f);
        }
    }
}

/// Round 6 V3 (`OPEN-238`): `finder` comes upon the body `c` (the find
/// pass, or the Gravedigger at the burial): SawCorpse, the fright, and the
/// law's case on it. The killer is named (a Murder report, the file's own
/// line) on `[crime.case] lead_p`, or `tie_lead_p` when the two knew each
/// other, x the law's competence; else the case stands with no suspect.
/// Never a minor: a protected killer is never named (CLAUDE.md, the
/// under-18 rule: no arrest force on a child), and the case says no suspect.
pub fn found(world: &mut World, c: EntityId, finder: EntityId) {
    let Some(k) = world.comp::<Corpse>(c).cloned() else { return };
    if k.found || k.cause != DeathCause::Violence {
        return;
    }
    if let Some(x) = world.comp_mut::<Corpse>(c) {
        x.found = true;
    }
    world.remember(finder, MemoryKind::SawCorpse, Some(c), 0.6, -0.5, false);
    if let Some(n) = world.comp_mut::<Needs>(finder) {
        n.safety = (n.safety - 0.2).max(0.0);
    }
    let cfg = world.config.crime.case.clone();
    let place = world.comp::<Position>(c).and_then(|p| p.building).map_or_else(
        || {
            format!(
                "the street ({})",
                world
                    .district_name(world.district_of(world.comp::<Position>(c).map_or(TilePos::default(), |p| p.tile)))
            )
        },
        |b| world.name_of(b),
    );
    let hours = world.tick.saturating_sub(k.died_tick) / crate::time::TICKS_PER_HOUR;
    let (fname, dname) = (world.name_of(finder), world.name_of(c));
    let lay = if hours > 0 { format!(", dead {hours} h") } else { String::new() };
    // Round 8 X1 (`OPEN-246`): the dead went for the man who killed him (or
    // fought the guard at his arrest): no Murder, nobody named. V16
    // `miserable_103693 d26 20:15` named the one who had reported him.
    if k.defended {
        let by = k.killer.map_or_else(|| "the one he went for".to_string(), |x| world.name_of(x));
        let text = format!(
            "{fname} found the body of {dname} at {place}{lay}; the law closed it: {dname} died fighting {by}, who \
             is charged with nothing"
        );
        world.push_event(EventKind::CorpseFound, &[finder, c], text);
        return;
    }
    let killer = k.killer.filter(|&x| crate::systems::law::living(world, x));
    let wanted = killer.is_some_and(|x| {
        world
            .crime_reports()
            .iter()
            .any(|r| !r.resolved && r.suspect == x && r.crime == crate::components::Crime::Murder)
    });
    let nameable = killer.filter(|&x| !crate::systems::ages::is_protected(world, x) && !wanted);
    let named = nameable.filter(|_| {
        let p = if k.tied { cfg.tie_lead_p } else { cfg.lead_p };
        let p = (p * crate::systems::competence::law_mult(world)).clamp(0.0, 1.0);
        world.rng.world().random::<f32>() < p
    });
    // Round 7 W2 (`OPEN-238`): a finder who saw the killing names the
    // killer to the law, or keeps quiet for a reason his line gives (his
    // gang, the killer's gang on his block, his nature: `law::keeps_from_law`)
    // and the case reads from that. V15 `gang_member_6731 d24 02:54`: saw
    // the Murder, found the body a minute later, "no suspect", no word why.
    let saw = killer.is_some_and(|x| saw_murder(world, finder, x));
    let tells = saw && nameable.is_some_and(|x| crate::systems::law::tells_the_law(world, finder, x));
    let quiet =
        if saw && !tells { killer.and_then(|x| crate::systems::law::keeps_from_law(world, finder, x)) } else { None };
    let named = if tells { nameable } else { named };
    if saw {
        let row = crate::systems::ownership::jobs2_row(world);
        row.witness_finds += 1;
        row.witness_named += u32::from(tells);
        row.witness_quiet += u32::from(quiet.is_some());
    }
    let hush = quiet.as_ref().map_or(String::new(), |why| format!(" ({fname} saw it done and said nothing: {why})"));
    let case = match (named, wanted) {
        (Some(_), _) if tells => format!("; the law opened a case and has a name: {fname} saw it done"),
        (Some(_), _) => format!("; the law opened a case and has a name{hush}"),
        (None, true) => format!("; the law's case is open on the killer already wanted{hush}"),
        (None, false) => format!("; the law opened a case: no suspect{hush}"),
    };
    world.push_event(
        EventKind::CorpseFound,
        &[finder, c],
        format!("{fname} found the body of {dname} at {place}{lay}{case}"),
    );
    if let Some(x) = named {
        let text = if tells {
            format!("{fname} told the law {} did it: the Murder of {dname}", world.name_of(x))
        } else {
            let how = if k.tied { "who knew the dead" } else { "from the scene" };
            format!("the law named {} for the Murder of {dname} ({how}; the body found by {fname})", world.name_of(x))
        };
        let witness = tells.then_some(finder);
        crate::systems::law::file_report_as(
            world,
            crate::components::Crime::Murder,
            x,
            witness,
            Some((text, &[c, finder])),
        );
    }
}

/// Round 8 X1 (`OPEN-246`): `dead` struck first and fell to the one he went
/// for, or died resisting arrest: his body's case names nobody (`found`).
pub fn defended(world: &mut World, dead: EntityId) {
    if let Some(k) = world.comp_mut::<Corpse>(dead) {
        k.defended = true;
    }
}

/// Round 7 W2 (`OPEN-238`): `w` holds a first-hand sighting of `killer`
/// committing a Murder.
fn saw_murder(world: &World, w: EntityId, killer: EntityId) -> bool {
    world.comp::<crate::components::Memory>(w).is_some_and(|m| {
        m.entries.iter().any(|e| {
            e.kind == MemoryKind::SawCrime
                && !e.second_hand
                && e.subject == Some(killer)
                && e.crime == Some(crate::components::Crime::Murder)
        })
    })
}

/// The Gravedigger is told of every unburied corpse daily so Bury can fire.
pub(super) fn gravedigger_notices(world: &mut World) {
    let corpses: Vec<EntityId> =
        world.with::<Corpse>().into_iter().filter(|&c| world.comp::<Corpse>(c).is_some_and(|k| !k.buried)).collect();
    if corpses.is_empty() {
        return;
    }
    let diggers: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&id| world.has::<Brain>(id) && world.comp::<Job>(id).is_some_and(|j| j.role == Role::Gravedigger))
        .collect();
    for d in diggers {
        for &c in &corpses {
            world.remember(d, MemoryKind::SawCorpse, Some(c), 0.5, -0.3, false);
        }
    }
}

/// BuryCorpse: the corpse goes to the Cemetery, is marked buried and freed a
/// day later; the digger grieves a little.
pub fn bury(world: &mut World, digger: EntityId, corpse: EntityId) -> bool {
    let Some(cemetery) = world.building_of_kind(BuildingKind::Cemetery) else { return false };
    if !world.comp::<Corpse>(corpse).is_some_and(|c| !c.buried) {
        return false;
    }
    // Round 6 V3 (`OPEN-238`): a body nobody found is found by its digger.
    found(world, corpse, digger);
    let tick = world.tick;
    let slot = world.comp::<Building>(cemetery).map_or(TilePos::default(), |b| b.interior().next().unwrap_or(b.door));
    if let Some(p) = world.comp_mut::<Position>(corpse) {
        p.tile = slot;
        p.building = Some(cemetery);
    }
    if let Some(c) = world.comp_mut::<Corpse>(corpse) {
        c.buried = true;
        c.buried_tick = Some(tick);
    }
    if let Some(b) = world.comp_mut::<Brain>(digger) {
        b.carrying_corpse = None;
    }
    // M13 D13/D14: what is left is inherited; the chrome goes to the Recycler.
    crate::systems::assets::on_buried(world, corpse);
    world.remember(digger, MemoryKind::Grief, Some(corpse), 0.3, -0.2, false);
    world.stats.current.burials += 1;
    let (nd, nc) = (world.name_of(digger), world.name_of(corpse));
    world.push_event(EventKind::Burial, &[digger, corpse], format!("{nd} buried {nc}"));
    true
}

/// Remove a corpse entity entirely (rotted or a day after burial).
pub fn free_corpse(world: &mut World, corpse: EntityId) {
    // Anyone carrying it puts it down (Scale S1: only a Brain carries).
    for id in world.brained() {
        if world.comp::<Brain>(id).is_some_and(|b| b.carrying_corpse == Some(corpse)) {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.carrying_corpse = None;
            }
            world.abort_plan(id);
        }
    }
    world.remove_agent(corpse);
}
