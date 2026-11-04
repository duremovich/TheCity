use super::*;

pub fn courage(world: &World, id: EntityId) -> f32 {
    match world.comp::<Personality>(id) {
        Some(p) => p.courage,
        // M13 D41: a robot never breaks.
        None if crate::systems::robots::is_robot(world, id) => 1.0,
        None => 0.5,
    }
}

/// M13 D31: what a fight's caller adds: the death chance × `kill_mult` (a
/// Crush, a berserker), and `a_bonus` on the attacker's fighting (an
/// abduction crew's members within 2 tiles). M15 W22: `kill_p`, when the
/// attacker wins, replaces `fight_death_p × (1 + fi_winner)` (a lethal
/// Hunt's strike); `kill_mult` and armour still apply.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FightMods {
    pub kill_mult: f32,
    pub a_bonus: f32,
    pub kill_p: Option<f32>,
    /// Shadow V4 review 4: `a` is a guard taking `b` in (`arrest`): a `b`
    /// who loses remembers `WasArrested`, not `Lost` (no Assaulted deed, so
    /// no grudge or Enemy edge on the guard from `grudges::on_learn`).
    pub arrest: bool,
    /// The combat model C1: why the fight was started (the stances).
    pub why: crate::systems::combat::Purpose,
}

impl Default for FightMods {
    fn default() -> Self {
        FightMods {
            kill_mult: 1.0,
            a_bonus: 0.0,
            kill_p: None,
            arrest: false,
            why: crate::systems::combat::Purpose::Brawl,
        }
    }
}

/// `p_win = clamp(0.5 + 0.4 (fi_a − fi_b) + 0.1 (C_a − C_b), 0.1, 0.9)`.
/// Returns `(winner, loser, loser_died)`.
pub fn resolve_fight(world: &mut World, a: EntityId, b: EntityId) -> (EntityId, EntityId, bool) {
    resolve_fight_mods(world, a, b, FightMods::default())
}

/// M12 D32: [`resolve_fight`] with the death chance × `kill_mult` (a Crush).
pub fn resolve_fight_with(world: &mut World, a: EntityId, b: EntityId, kill_mult: f32) -> (EntityId, EntityId, bool) {
    resolve_fight_mods(world, a, b, FightMods { kill_mult, ..FightMods::default() })
}

/// M13 D31: the fight with its mods and the Kit terms. Only when either
/// side's Kit is not bare does `+ 0.2 (K.reflex_a − K.reflex_b)` join
/// `p_win`, and only when the loser has armour is the death chance ×
/// `(1 − armour)`: branches, not zero terms, so two unchromed fighters roll
/// exactly as in M12 (`tests/chrome.rs` asserts it).
///
/// M13 D41, C3b: a robot on either side (no Brain) fights in the beats at
/// `robot_fighting[tier − 1]` for all three skills, never breaks, takes no
/// wounds (`[combat] robot_wreck_hits` wreck it: `assets::wreck`, Parts at
/// the door); the agent's memories name no subject; a robot that kills
/// passes its owner as the killer (no crime is raised for it).
pub fn resolve_fight_mods(world: &mut World, a: EntityId, b: EntityId, mods: FightMods) -> (EntityId, EntityId, bool) {
    // Round 3 V12 fix (Dylan 2026-10-27, no child is harmed): the non-lethal
    // backstop is the engagement's (`combat::build`): no blow lands when
    // either side is under 18 (`(a, b, false)`, `harm_blocked` counted).
    // C3a: a downed party finished, the beats (C3b: a robot's too), then
    // `turf::beaten` and the chair's dump: `combat::instant`, `after_duel`.
    let c = crate::systems::combat::clash_now(world, a, b, mods);
    (c.winner, c.loser, c.died)
}

/// `Arrest` at completion: the guard must be adjacent (or in the same
/// building). A suspect with courage > 0.7 contests it. Returns whether the
/// suspect is now cuffed.
pub fn arrest(world: &mut World, guard: EntityId, suspect: EntityId) -> bool {
    // Pressure pass round 9, Y2: the under-18 rule (CLAUDE.md): a child is
    // never a target of the cuffs, the fight for them or a second guard (no
    // warrant names one; this is the backstop).
    if crate::systems::ages::is_protected(world, suspect) {
        crate::systems::ownership::jobs2_row(world).harm_blocked += 1;
        return false;
    }
    if guard == suspect
        || !living(world, suspect)
        || !living(world, guard)
        || world.has::<Sentence>(suspect)
        || !near(world, guard, suspect, 1)
        // S3: two guards after one suspect (a collar and a claim): the
        // second to reach a cuffed suspect has nothing to do.
        || world.comp::<Brain>(suspect).and_then(|b| b.cuffed_by).is_some_and(|c| c != guard && c != suspect)
        // C3a: a suspect already in a fight (the cuffs fought, a brawl) is
        // taken when it ends.
        || crate::systems::combat::engaged(world, suspect)
    {
        return false;
    }
    let courage = world.comp::<Personality>(suspect).map_or(0.0, |p| p.courage);
    // M13 D33: a berserker always fights the cuffs, and the guard fights to
    // kill (`psycho_kill`, × `crush_kill_mult` under a Crush).
    let berserk = crate::systems::chrome::in_episode(world, suspect);
    // M14 V12: a runner dazed by a lost contest cannot contest the cuffs.
    let dazed = world.comp::<Brain>(suspect).and_then(|b| b.dazed_until).is_some_and(|t| t > world.tick);
    if (courage > 0.7 || berserk) && !dazed {
        let mods = if berserk {
            FightMods {
                kill_mult: crate::systems::chrome::arrest_kill_mult(world, suspect),
                arrest: true,
                ..FightMods::default()
            }
        } else if crate::systems::weapons::tier(world, suspect) > 0 {
            // Jobs & lives phase 3 (section 4): an armed suspect is fought harder.
            FightMods { kill_mult: world.config.weapons.armed_arrest_mult, arrest: true, ..FightMods::default() }
        } else {
            FightMods { arrest: true, ..FightMods::default() }
        };
        // Shadow V4 bug 10: a suspect who loses the scuffle for the cuffs is
        // arrested, not assaulted (`FightMods::arrest`: a Lost memory
        // gossiped "Assaulted by <guard>" and fed dread and grudges).
        // C3a: the scuffle is an engagement (live when both are Full: the
        // guard's Arrest step waits it out); its end is `arrest_end`.
        let tail = crate::systems::combat::Tail::Arrest { berserk };
        let sides = [vec![guard], vec![suspect]];
        return match crate::systems::combat::engage(world, sides, mods, tail, true, usize::MAX) {
            crate::systems::combat::Begun::Live => false,
            crate::systems::combat::Begun::Over(cuffed) => cuffed,
        };
    }
    cuff(world, guard, suspect, None, berserk)
}

/// C3a, the arrest's per-duel tail: Pressure pass round 9, Y2
/// (`OPEN-251`): the arresting guard fought off (alive, the suspect too),
/// the guards at hand take their turn at him (`[justice] backup_reach`,
/// `backup_max`): they join the guard's side in order. V17 `joytoy_123639
/// d27 15:32-23:49`: five guards fought off one at a time, none with a
/// second. A death on the guards' side ends it (none follow).
pub(crate) fn arrest_duel(
    world: &mut World,
    e: &mut crate::systems::combat::Engagement,
    c: &crate::systems::combat::Clash,
) {
    let (Some(guard), Some(suspect)) = (e.first[0], e.first[1]) else { return };
    if c.loser == suspect {
        return;
    }
    if c.died {
        for x in std::mem::take(&mut e.sides[0]) {
            world.combat.engaged.remove(&x);
        }
        return;
    }
    if c.a != guard {
        return;
    }
    let id = world.combat.engaged.get(&suspect).copied();
    for p in backup_at_hand(world, guard, suspect) {
        if e.live && crate::systems::combat::engaged(world, p) {
            continue;
        }
        e.sides[0].push(p);
        if let (true, Some(id)) = (e.live, id) {
            world.combat.engaged.insert(p, id);
        }
    }
}

/// C3a, the arrest's end (the scuffle's tail): the suspect dead resisting,
/// the guard killed (a Murder), the guards fought off (backup called), the
/// suspect fled on foot (no cuffs: the backup's chase), else the cuffs.
/// Returns whether the arresting guard cuffed him.
pub(crate) fn arrest_end(world: &mut World, e: &crate::systems::combat::Engagement, berserk: bool) -> bool {
    let (Some(guard), Some(suspect)) = (e.first[0], e.first[1]) else { return false };
    let c = e.result();
    let (winner, loser, died) = (c.winner, c.loser, c.died);
    // The guards who fought him, in turn; the one in the fight that ended it.
    let mut fought: Vec<EntityId> = e.clashes.iter().filter(|x| x.fought).map(|x| x.a).collect();
    fought.dedup();
    if fought.is_empty() {
        fought.push(guard);
    }
    if fought.len() > 1 {
        crate::systems::ownership::jobs2_row(world).backup_fights += (fought.len() - 1) as u32;
    }
    let fighter = *fought.last().unwrap_or(&guard);
    if !c.fought {
        // No blow struck (a party gone): nobody is cuffed here.
        return false;
    }
    if died {
        if loser == suspect {
            // The suspect died resisting: nothing left to prosecute.
            // Round 8 X1 (`OPEN-246`): the line says so, and the body's
            // case names nobody (V16 `miserable_103693 d26 20:15`: the
            // law named for the Murder the guard who had reported him).
            let charge = wanted_for(world, suspect).map_or_else(
                || "an open warrant".to_string(),
                |c| format!("wanted for {}", charge_label(world, suspect, c)),
            );
            let text = format!(
                "{} died resisting arrest by the guard {} ({charge})",
                world.name_of(suspect),
                world.name_of(fighter)
            );
            world.push_event(EventKind::Assault, &[suspect, fighter], text);
            crate::systems::demography::defended(world, suspect);
            resolve_reports(world, suspect);
            if berserk {
                crate::systems::chrome::ended_by_law(world, suspect, "killed by the law");
            }
        } else {
            // The suspect killed the guard: a Murder, witnessed by whoever is near.
            let tile = world.comp::<Position>(suspect).map_or(TilePos::default(), |p| p.tile);
            file_report(world, Crime::Murder, suspect, None);
            raise_crime_on(world, suspect, None, Some(fighter), Crime::Murder, tile);
        }
        return false;
    }
    let charge = |world: &World| {
        wanted_for(world, suspect).map_or_else(
            || "an open warrant".to_string(),
            |c| format!("wanted for {}", charge_label(world, suspect, c)),
        )
    };
    if winner == suspect {
        if let Some(n) = world.comp_mut::<Needs>(suspect) {
            n.safety = (n.safety - 0.3).max(0.0);
        }
        // Pressure pass round 3, S3 (OPEN-223): the line says it was an
        // arrest resisted and for what ("fought off X" read as the
        // sleeper attacked three times and booked for it).
        let charge = charge(world);
        let who = match fought.as_slice() {
            [g] => format!("the guard {}", world.name_of(*g)),
            gs => format!("the guards {}", gs.iter().map(|&g| world.name_of(g)).collect::<Vec<_>>().join(" and ")),
        };
        world.push_event(
            EventKind::Assault,
            &fought.iter().copied().chain(std::iter::once(suspect)).rev().collect::<Vec<_>>(),
            format!("{} resisted arrest and fought off {who} ({charge})", world.name_of(suspect)),
        );
        crate::systems::law_brain::push_shock(world, LawShock::GuardBeaten);
        // Round 9 Y2 (`OPEN-251`): a guard fought off calls for a second
        // (the nearest free guard on shift who can chase him: the
        // chase's hand-on, his collar), so the next try is two.
        call_backup(world, guard, suspect);
        return false;
    }
    if c.end == crate::systems::combat::End::Fled {
        // C3a: he broke and ran (C1 cuffed him all the same): no cuffs; the
        // second guard's chase takes him on as his collar.
        let charge = charge(world);
        let text =
            format!("{} broke from the guard {} and ran ({charge})", world.name_of(suspect), world.name_of(fighter));
        world.push_event(EventKind::Assault, &[suspect, fighter], text);
        crate::systems::ownership::jobs2_row(world).arrest_escapes += 1;
        call_backup(world, guard, suspect);
        return false;
    }
    let helper = (fought.len() > 1).then(|| {
        crate::systems::ownership::jobs2_row(world).backup_cuffed += 1;
        fighter
    });
    if !living(world, suspect) || world.has::<Sentence>(suspect) {
        return false;
    }
    cuff(world, guard, suspect, helper, berserk)
}

/// The cuffs go on: `guard` (or, laid out, `helper`, the second who held
/// him) walks him in. Returns whether the arresting guard cuffed him.
fn cuff(world: &mut World, guard: EntityId, suspect: EntityId, helper: Option<EntityId>, berserk: bool) -> bool {
    // Round 9 Y2: the arresting guard laid out, the second who held him
    // walks him in (the first's step fails; the second replans to escort).
    let cuffer = match helper {
        Some(h) if !living(world, guard) || crate::systems::wounded::is_down(world, guard) => h,
        _ => guard,
    };
    // M14 V15: cuffed in the chair its newest trace named (the gate's
    // "arrest at the chair"; `jail_suspect` counts the rest).
    let at_chair = world.last_trace.get(&suspect).map(|&(chair, _)| chair).is_some_and(|chair| {
        world.comp::<Position>(suspect).and_then(|p| p.building) == Some(chair)
            && world
                .crime_reports()
                .iter()
                .any(|r| !r.resolved && r.suspect == suspect && matches!(r.crime, Crime::Intrusion | Crime::DataTheft))
    });
    if at_chair {
        world.stats.current.virt.hack_arrests_chair += 1;
    }
    world.abort_plan(suspect);
    if let Some(b) = world.comp_mut::<Brain>(suspect) {
        b.cuffed_by = Some(cuffer);
        b.current_goal = None;
    }
    // M13 D33: the cuffs end an episode.
    if berserk {
        crate::systems::chrome::ended_by_law(world, suspect, "arrested");
    }
    if cuffer != guard {
        world.abort_plan(cuffer);
    }
    if let Some(b) = world.comp_mut::<Brain>(cuffer) {
        b.escorting = Some(suspect);
    }
    // Lives at work phase 7 (§ 5.2): the nearest Station full: sent on.
    crate::systems::stations::on_arrest(world, cuffer);
    // S3: the arrest names its charge; the witness's own arrest is his collar.
    if world.crime_reports().iter().any(|r| !r.resolved && r.suspect == suspect && r.witness == Some(guard)) {
        crate::systems::ownership::jobs2_row(world).law_collars += 1;
    }
    let charge =
        wanted_for(world, suspect).map_or_else(String::new, |c| format!(" for {}", charge_label(world, suspect, c)));
    // Round 7 W3: the arrest at home (the warrant taken to his door).
    let home = world.comp::<crate::components::Household>(suspect).and_then(|h| h.home);
    let at_home = home.is_some() && world.comp::<Position>(suspect).and_then(|p| p.building) == home;
    let place = match home.filter(|_| at_home) {
        Some(h) => {
            crate::systems::ownership::jobs2_row(world).law_door_arrests += 1;
            format!(" at home in {}", world.name_of(h))
        }
        None => String::new(),
    };
    if on_leave(world, guard) {
        crate::systems::ownership::jobs2_row(world).leave_arrests += 1;
    }
    let (held, mut actors) = match helper {
        Some(h) if h == cuffer => (format!(", {} laid out", world.name_of(guard)), vec![cuffer, suspect, guard]),
        Some(h) => (
            format!(", {} holding {}", world.name_of(h), crate::systems::consequence::pro(world, suspect).1),
            vec![guard, suspect, h],
        ),
        None => (String::new(), vec![guard, suspect]),
    };
    actors.dedup();
    world.push_event(
        EventKind::Arrest,
        &actors,
        format!("{} arrested {}{charge}{place}{held}", world.name_of(cuffer), world.name_of(suspect)),
    );
    cuffer == guard
}

/// Pressure pass round 9, Y2 (`OPEN-251`): the on-duty city guards at
/// hand for `guard`'s arrest of `suspect` (`[justice] backup_reach` of the
/// suspect, nearest first, up to `backup_max` − 1): on their feet, free of
/// a prisoner, not on the suspect's take, never a minor (CLAUDE.md, the
/// under-18 rule; the suspect is never one: `arrest`'s backstop).
fn backup_at_hand(world: &World, guard: EntityId, suspect: EntityId) -> Vec<EntityId> {
    let Some(c) = world.config.justice.as_ref() else { return Vec::new() };
    let (reach, max) = (c.backup_reach, c.backup_max.saturating_sub(1));
    if reach == 0 || max == 0 {
        return Vec::new();
    }
    let Some(sp) = world.comp::<Position>(suspect).map(|p| (p.tile, p.building)) else { return Vec::new() };
    let mut near: Vec<(u32, EntityId)> = world
        .guards()
        .iter()
        .copied()
        .filter(|&g| g != guard && g != suspect && living(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| on_duty(world, g) && is_city_guard(world, g) && !crate::systems::ages::is_protected(world, g))
        .filter(|&g| {
            world.comp::<Brain>(g).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical && b.cuffed_by.is_none() && b.escorting.is_none()
            }) && !crate::systems::wounded::is_down(world, g)
        })
        .filter(|&g| !crate::systems::contracts::on_take_of(world, g, suspect))
        .filter_map(|g| {
            let p = world.comp::<Position>(g)?;
            let d = chebyshev(p.tile, sp.0);
            // In the room with him, or on the street within reach (at his
            // door: the one at the door comes in).
            (d <= reach && (p.building == sp.1 || outdoors(world, p.tile, p.building))).then_some((d, g))
        })
        .collect();
    near.sort();
    near.into_iter().take(max).map(|(_, g)| g).collect()
}

/// Pressure pass round 9, Y2 (`OPEN-251`): `guard`, fought off by
/// `suspect`, calls for a second: the nearest free guard on shift who can
/// chase him (`handoff_guard`) takes him as his collar for `[justice]
/// collar_hours` (`handoffs`); the sighting is fresh (the guard just saw
/// him). Never on a minor (the under-18 rule: `arrest`'s backstop).
fn call_backup(world: &mut World, guard: EntityId, suspect: EntityId) {
    if crate::systems::ages::is_protected(world, suspect) || !living(world, suspect) {
        return;
    }
    let Some(at) = world.comp::<Position>(suspect).map(|p| p.tile) else { return };
    let now = world.tick;
    world.set_last_seen(suspect, (at, now));
    let Some(r) = handoff_guard(world, guard, suspect, at) else { return };
    let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
    let since = now.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    world.handoffs.retain(|_, &mut (_, t)| t >= since);
    world.handoffs.insert(r, (suspect, now));
    let far = world.comp::<Position>(r).map_or(0, |p| p.tile.manhattan(at));
    let text = format!(
        "{} called for a second on {}: {} on shift, {far} tiles off, comes",
        world.name_of(guard),
        world.name_of(suspect),
        world.name_of(r)
    );
    world.push_event(EventKind::Pursuit, &[guard, suspect, r], text);
    crate::systems::ownership::jobs2_row(world).backup_called += 1;
}

/// The escorting guard moved: the cuffed suspect comes along.
pub fn follow_guard(world: &mut World, guard: EntityId) {
    let Some(suspect) = world.comp::<Brain>(guard).and_then(|b| b.escorting) else { return };
    // A prisoner stays in the Jail whoever still thinks they are escorting them.
    if !living(world, suspect) || world.has::<Sentence>(suspect) {
        if let Some(b) = world.comp_mut::<Brain>(guard) {
            b.escorting = None;
        }
        return;
    }
    let Some(gp) = world.comp::<Position>(guard).cloned() else { return };
    let was_inside = world.comp::<Position>(suspect).and_then(|p| p.building);
    if was_inside != gp.building {
        world.remove_from_building(suspect);
        if let Some(b) = gp.building {
            // appear inside with the guard (capacity is the Jail's problem at arrival)
            if let Some(bd) = world.comp_mut::<Building>(b) {
                if let Err(i) = bd.occupants.binary_search(&suspect) {
                    bd.occupants.insert(i, suspect);
                }
            }
        }
    }
    if let Some(p) = world.comp_mut::<Position>(suspect) {
        p.tile = gp.tile;
        p.building = gp.building;
        p.floor = gp.floor;
    }
}

/// Escort arrived at the Jail: sentence the suspect (or fine / release when
/// the Jail is full), resolve the report, free the guard. The island pass
/// phase 7: a Station whose cells filled while the escort drove books him
/// to the Precinct, and he rides the van on from its door (no write of the
/// body to the Precinct).
pub fn jail_suspect(world: &mut World, guard: EntityId, suspect: EntityId) {
    let station = world
        .comp::<Position>(guard)
        .and_then(|p| p.building)
        .filter(|&b| crate::systems::motion::vans_on(world) && crate::systems::stations::is_station(world, b));
    jail_suspect_at(world, guard, suspect);
    if let Some(s) = station {
        crate::systems::stations::send_on(world, s, suspect);
    }
}

fn jail_suspect_at(world: &mut World, guard: EntityId, suspect: EntityId) {
    if let Some(b) = world.comp_mut::<Brain>(guard) {
        b.escorting = None;
    }
    if !living(world, suspect) {
        return;
    }
    if let Some(b) = world.comp_mut::<Brain>(suspect) {
        b.cuffed_by = None;
    }
    // M13 D38: the arrest confiscates the suspect's Stims (destroyed).
    crate::systems::stims::confiscate(world, suspect);
    // Jobs & lives phase 3 (§ 4): and every weapon carried (the Precinct's).
    crate::systems::weapons::confiscate(world, suspect);
    // M14 V12/V18: a runner is dumped; the deck's Data is confiscated.
    crate::systems::virt::dump(world, suspect, "arrested");
    crate::systems::virt::confiscate_data(world, suspect);
    // M14 V15: an arrest on a run's report.
    let hacked = world
        .crime_reports()
        .iter()
        .any(|r| !r.resolved && r.suspect == suspect && matches!(r.crime, Crime::Intrusion | Crime::DataTheft));
    if hacked {
        world.stats.current.virt.hack_arrests += 1;
    }
    // M13 D33: jailing ends an episode still running.
    crate::systems::chrome::ended_by_law(world, suspect, "jailed");
    // Sentenced while in cuffs (a second escort, a player or god jailing):
    // the sentence they are serving stands.
    if world.has::<Sentence>(suspect) {
        return;
    }
    // Jobs & lives phase 6 (§ 7): fake papers exposed (nothing off). Before
    // the sentence's crime is picked (phase-8 review): the Forgery is an
    // open report like any other and sets the term when it is the worst.
    crate::systems::gray::on_arrest(world, suspect);
    // The most severe open crime sets the sentence; the rest are covered by it.
    let Some(report) = world
        .crime_reports()
        .iter()
        .filter(|r| !r.resolved && r.suspect == suspect)
        .max_by_key(|r| (r.crime, r.tick))
        .cloned()
    else {
        release_at_jail_door(world, suspect, "no open warrant");
        return;
    };
    let crime = report.crime;
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    // Lives at work phase 7 (§ 5.2): an escort that ended in a Station books
    // there (its cells: a fine or a night served there, anything longer to
    // the Precinct by the van). The Precinct's room still decides a full
    // house: a Station's held go there by morning.
    let station = world
        .comp::<Position>(guard)
        .and_then(|p| p.building)
        .filter(|&b| crate::systems::stations::on(world) && crate::systems::stations::is_station(world, b))
        // Review fix 5: the cells filled while the escort walked: the Precinct's.
        .filter(|&b| crate::systems::stations::has_cell(world, b));
    let jail = station.unwrap_or(precinct);
    let capacity = usize::from(world.config.buildings.jail.capacity);
    // Review fix 5: the Station's held bound for the Precinct count.
    let jailed = crate::systems::stations::precinct_bound(world);

    // Shadow V4 bug 4: a petty offence (a night in the cells) is fined
    // whenever the suspect can pay, not only when the Precinct is full (fare
    // evasion was the city's top sentence, served from a desk or a bar).
    let full = jailed >= capacity;
    if full && crate::systems::stations::on(world) {
        crate::systems::ownership::jobs2_row(world).precinct_full += 1;
    }
    let petty = is_petty(crime);
    let can_pay = world.comp::<crate::components::Wallet>(suspect).is_some_and(|w| w.coins >= fine_for(world, crime));
    if full || (petty && can_pay) {
        match crime {
            c if c == Crime::Theft || is_petty(c) => {
                let fine = fine_for(world, crime);
                let coins = world.comp::<crate::components::Wallet>(suspect).map_or(0, |w| w.coins);
                // M12 review: a Vagrancy report is settled as `street::vagrancy_hit`'s
                // fine is: where the vagrant was swept, on the ledger, in the log.
                let swept = (crime == Crime::Vagrancy).then(|| vagrancy_place(world, suspect));
                if coins >= fine {
                    if crime == Crime::Vagrancy || crime == Crime::FareEvasion {
                        crate::systems::ownership::pay(
                            world,
                            Some(suspect),
                            None,
                            fine,
                            crate::systems::ownership::Flow::Fine,
                        );
                    } else {
                        if let Some(w) = world.comp_mut::<crate::components::Wallet>(suspect) {
                            w.coins -= fine;
                        }
                        if let Some(t) = world.treasury_mut() {
                            t.coins += fine;
                        }
                    }
                    world.remember(suspect, MemoryKind::Paid, None, 0.4, -0.4, false);
                    resolve_reports(world, suspect);
                    let name = world.name_of(suspect);
                    world.push_event(
                        EventKind::Sentence,
                        &[suspect],
                        format!("{name} fined {fine} coins{}", if full { " (Precinct full)" } else { "" }),
                    );
                    if let Some(place) = swept {
                        crate::systems::street::note_vagrancy(world, place);
                        let text = format!("{name} fined {fine} for sleeping rough in {}", world.district_name(place));
                        world.push_event(EventKind::Vagrancy, &[suspect], text);
                    }
                    world.stats.current.arrests += 1;
                    if station.is_some() {
                        let row = crate::systems::ownership::jobs2_row(world);
                        row.station_bookings += 1;
                        row.station_fines += 1;
                    }
                    // Shadow V4 review 5: the fine spares the cell, not the
                    // arrest (the take, the interrogation, the rumour, the
                    // sweep's counter, the law's log).
                    booked(world, guard, suspect, crime, jail);
                    release_at_jail_door(world, suspect, "fined");
                } else {
                    if let Some(p) = world.comp_mut::<Personality>(suspect) {
                        p.lawfulness = (p.lawfulness - 0.05).max(0.0);
                    }
                    resolve_reports(world, suspect);
                    let name = world.name_of(suspect);
                    world.push_event(
                        EventKind::Unpunished,
                        &[suspect],
                        format!("{name} released: Precinct full, cannot pay"),
                    );
                    release_at_jail_door(world, suspect, "unpunished");
                }
                return;
            }
            _ => {
                // Free the least prisoner whose crime is strictly less severe
                // than this arrest's (a vagrant first, M12 D15: a night in the
                // cells never keeps a real criminal out); ties go to the one
                // released soonest, then the lower id.
                let victim = world
                    .with::<Sentence>()
                    .into_iter()
                    .filter_map(|p| world.comp::<Sentence>(p).map(|s| (p, s.crime, s.until_tick)))
                    .filter(|&(_, c, _)| c < crime)
                    .min_by_key(|&(p, c, until)| (c, until, p))
                    .map(|(p, ..)| p);
                match victim {
                    Some(p) => release(world, p, false),
                    None => {
                        resolve_reports(world, suspect);
                        let name = world.name_of(suspect);
                        world.push_event(EventKind::Unpunished, &[suspect], format!("{name} released: Precinct full"));
                        release_at_jail_door(world, suspect, "unpunished");
                        return;
                    }
                }
            }
        }
    }

    // V8 (`OPEN-256`): a short term bought out from the purse (the
    // convict's, his kin's): booked, no cell.
    if crate::systems::cells::fine_out(world, suspect, crime, sentence_ticks(world, crime)) {
        world.remember(suspect, MemoryKind::Paid, None, 0.4, -0.4, false);
        resolve_reports(world, suspect);
        world.stats.current.arrests += 1;
        booked(world, guard, suspect, crime, jail);
        release_at_jail_door(world, suspect, "fined");
        return;
    }
    let until = world.tick + sentence_ticks(world, crime);
    sentence(world, suspect, crime, until, jail);
    resolve_reports(world, suspect);
    world.stats.current.arrests += 1;
    if station.is_some() {
        // A sentence that ends before the next van is served in the cells.
        let van = crate::systems::stations::next_van(world);
        let row = crate::systems::ownership::jobs2_row(world);
        row.station_bookings += 1;
        if until <= van + TICKS_PER_DAY {
            row.station_nights += 1;
        }
    }
    booked(world, guard, suspect, crime, jail);
}

/// An arrest's record, jailed or fined: the sweep's counter, the take and
/// the interrogation, the rumour and the law's log.
pub(super) fn booked(world: &mut World, guard: EntityId, suspect: EntityId, crime: Crime, jail: EntityId) {
    // Jobs & lives phase 4: the sweeps' arrests.
    if crime == Crime::Solicitation {
        crate::systems::ownership::jobs2_row(world).solicit_arrests += 1;
    }
    // M16a (plan C28, C31): the take ends with an arrest by the guard on
    // it; the arresting guard interrogates a recent taker.
    if !world.contracts.is_empty() {
        crate::systems::contracts::on_arrest(world, guard, suspect);
        interrogate(world, guard, suspect);
    }
    // M15: an arrest is public: talked about where the suspect lives (the
    // Precinct's district for the homeless), and on the law's own record.
    let d = crate::systems::gossip::home_district(world, suspect).unwrap_or_else(|| world.district_of_building(jail));
    crate::systems::gossip::post_deed(world, d, crate::word::Deed::Arrested, Some(suspect), None);
    let t = world.tick;
    world.arrest_log.push_back((t, suspect));
}
