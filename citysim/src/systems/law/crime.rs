use super::*;

/// A crime happened: roll every nearby Brain-bearing agent as a witness, give
/// victims their memory, file a report if a guard saw it, and sharpen the
/// actor's stealth if nobody did.
pub fn raise_crime(world: &mut World, actor: EntityId, victim: Option<EntityId>, crime: Crime, tile: TilePos) {
    raise_crime_on(world, actor, victim, victim, crime, tile);
}

/// M15 W5: `raise_crime` with the deed's object apart from the living
/// victim who gets the memory (a Murder's victim is dead: `victim None,
/// object Some(dead)`). The witnesses' `SawCrime` carries the object, and
/// a deed someone knows (a noticing witness, or the living victim) goes into
/// the crime district's pool named; a noticed Murder names the killing
/// `kill_by` posted unnamed (W10). Nothing here draws or reads a word
/// stream: the posts are state no decision reads in phase 1.
pub fn raise_crime_on(
    world: &mut World,
    actor: EntityId,
    victim: Option<EntityId>,
    object: Option<EntityId>,
    crime: Crime,
    tile: TilePos,
) {
    raise_crime_except(world, actor, victim, object, crime, tile, None);
}

/// `raise_crime_on` with one party who is no witness (L2 shadow fixes item
/// 16: a buyer is not a witness of the sale it made).
pub fn raise_crime_except(
    world: &mut World,
    actor: EntityId,
    victim: Option<EntityId>,
    object: Option<EntityId>,
    crime: Crime,
    tile: TilePos,
    except: Option<EntityId>,
) {
    raise_crime_mult(world, actor, victim, object, crime, tile, except, 1.0);
}

/// `raise_crime_except` with each witness's notice probability multiplied
/// by `witness_mult` (Lives at work phase 4b, § 2.3: a dealer's lookout,
/// `witness_lookout_mult`; 1.0 everywhere else). The rolls are drawn as
/// before, one a witness.
#[allow(clippy::too_many_arguments)]
pub fn raise_crime_mult(
    world: &mut World,
    actor: EntityId,
    victim: Option<EntityId>,
    object: Option<EntityId>,
    crime: Crime,
    tile: TilePos,
    except: Option<EntityId>,
    witness_mult: f32,
) {
    raise_crime_core(world, actor, victim, object, crime, tile, except, &[], witness_mult);
}

/// Round 5 U7 (`OPEN-237`): `raise_crime_on` with the crime's other parties
/// (a shakedown's payers, who have their own `WasRobbed`) kept out of the
/// witnesses.
pub fn raise_crime_parties(
    world: &mut World,
    actor: EntityId,
    victim: Option<EntityId>,
    object: Option<EntityId>,
    crime: Crime,
    tile: TilePos,
    parties: &[EntityId],
) {
    raise_crime_core(world, actor, victim, object, crime, tile, None, parties, 1.0);
}

/// Round 5 U7 (`OPEN-237`): an accomplice of `actor`: a member of the same
/// gang on the same goal (two partners at one shakedown, a raid's crew).
/// An accomplice is in the deed, not a witness of it.
pub fn accomplice(world: &World, w: EntityId, actor: EntityId) -> bool {
    let Some(g) = world.gang_of(actor) else { return false };
    if world.gang_of(w) != Some(g) {
        return false;
    }
    let goal = |id: EntityId| world.comp::<Brain>(id).and_then(|b| b.current_goal);
    goal(actor).is_some_and(|k| goal(w) == Some(k))
}

#[allow(clippy::too_many_arguments)]
fn raise_crime_core(
    world: &mut World,
    actor: EntityId,
    victim: Option<EntityId>,
    object: Option<EntityId>,
    crime: Crime,
    tile: TilePos,
    except: Option<EntityId>,
    parties: &[EntityId],
    witness_mult: f32,
) {
    let held_in = cells_of(world, actor);
    // L2 phase 4 (plan L25): the ledger's on-screen victims.
    if let Some(v) = victim {
        let kind = match crime {
            Crime::Assault => Some(crate::components::HoleKind::Assaulted),
            Crime::Theft => Some(crate::components::HoleKind::Robbed),
            Crime::Abduction => Some(crate::components::HoleKind::Abducted),
            _ => None,
        };
        if let Some(k) = kind {
            crate::systems::fviolence::note_victim(world, actor, v, k);
        }
    }
    let cfg = world.config.crime.clone();
    let r = if world.is_dark() { cfg.sight_night_crime } else { cfg.sight_day_crime };
    let stealth = stealth(world, actor);
    let actor_building = world.comp::<Position>(actor).and_then(|p| p.building);
    let salience = crime_salience(crime);
    // M12 D7: the district's crime counter.
    crate::systems::districts::note_crime(world, tile);
    // M12 D17: the crime's litter (a Murder's is its death's, `kill_by`).
    match crime {
        Crime::Theft | Crime::GrandTheft => crate::systems::litter::deposit_near(world, tile, actor_building, 6, 0),
        Crime::Extortion | Crime::Assault => crate::systems::litter::deposit_near(world, tile, actor_building, 12, 1),
        // A crash's litter is the crash's (`vehicles::crash`).
        // M14 V18: a run leaves nothing on the street.
        Crime::Murder
        | Crime::Vagrancy
        | Crime::FareEvasion
        | Crime::Manslaughter
        | Crime::Dealing
        | Crime::Intrusion
        | Crime::DataTheft
        // M16a (plan C29): never raised on the street.
        | Crime::Conspiracy
        // M16b: filed by their victims, never raised on the street.
        | Crime::Fraud
        | Crime::Blackmail
        // Jobs & lives phase 3: a sale, a sighting, a crate leave nothing.
        | Crime::ArmsDealing
        | Crime::UnlicensedArms
        | Crime::Smuggling
        // Jobs & lives phase 4: filed at a sweep.
        | Crime::Solicitation
        // Jobs & lives phase 5: filed at the holding, never raised on the street.
        | Crime::Trafficking
        // Jobs & lives phase 6: filed at an arrest or a raid.
        | Crime::Forgery
        | Crime::BlackMarket
        // Round 7 W5: a parent's break-out leaves the fight's litter only.
        | Crime::CampEscape => {}
        Crime::Abduction => crate::systems::litter::deposit_near(world, tile, actor_building, 12, 1),
    }

    // M13 D41: a powered robot at the building contests a theft or a
    // shakedown before anyone else sees it; a thief it detains is booked
    // already, so nobody else's report follows; one it kills is charged with
    // nothing (the dead cannot be): no witness, memory or report follows.
    let sensed = actor_building
        .map_or(crate::systems::robots::Sensed::Unseen, |b| crate::systems::robots::sense(world, actor, crime, b));
    if sensed == crate::systems::robots::Sensed::Died {
        return;
    }
    let detained = sensed == crate::systems::robots::Sensed::Detained;
    // M14 V28: a camera at the building contests what the robot did not book.
    if let (false, Some(b)) = (detained, actor_building) {
        crate::systems::virt::camera_sense(world, actor, crime, b);
    }
    // Round 5 U4 (`OPEN-236`): who is indoors sees indoors. A scene behind
    // walls is seen only from its building (the position index's building
    // list, not every body); a street scene by the bodies outside within
    // sight (the cells do not see the El gate).
    let scene_out = outdoors(world, tile, actor_building);
    let candidates: Vec<EntityId> = if detained {
        Vec::new()
    } else if scene_out {
        world.bodies()
    } else {
        world
            .agents_near(tile, 0, actor_building)
            .into_iter()
            .filter(|&w| world.comp::<Brain>(w).is_some_and(|b| b.lod != crate::components::Lod::Statistical))
            .collect()
    };
    let mut witnesses: Vec<EntityId> = candidates
        .into_iter()
        // Round 5 U7 (`OPEN-237`): the actor, the victim, the deed's object,
        // its named parties and the actor's accomplices are in the deed, never
        // its witnesses (a shakedown's payer saw his own robbery, `courier_50838
        // d23 03:28`; two partners in one Home each saw the other's, U4's shadow).
        .filter(|&w| w != actor && Some(w) != except && Some(w) != victim && Some(w) != object && !parties.contains(&w))
        .filter(|&w| !accomplice(world, w, actor))
        // M14 V12: a body jacked in sees nothing.
        .filter(|w| world.runner_of.is_empty() || !world.runner_of.contains_key(w))
        // Round 7 W5 (`OPEN-237`): nor does a sleeper (`ceo_16592 d21 01:03`,
        // 20k: "saw Hale Tanaka commit Shakedown" asleep in the block he shook
        // down, W2's sleeper who finds the purse short on waking).
        .filter(|&w| !crate::systems::presence::asleep(world, w))
        // Round 8 X1 (`OPEN-246`): nor a man held in the cells, but for a
        // scene in his own cells (V16 `ripperdoc_10662`: 14 Witness lines of
        // the El gate after his sentence; at 100k the cells' overflow stands
        // on the Precinct's door tile, which reads as the street).
        .filter(|&w| {
            !world.has::<Sentence>(w)
                || (actor_building.is_some() && world.comp::<Position>(w).and_then(|p| p.building) == actor_building)
        })
        .filter(|&w| {
            // M13 D32: a witness with Eyes sees `Kit.sight` tiles further.
            let reach = r + world.comp::<Kit>(w).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(w).is_some_and(|p| sees_scene(world, p, tile, actor_building, scene_out, reach))
        })
        .collect();
    // Pressure pass F2: the law's eye is rolled first (the cap below never
    // drops it), then the rest in id order.
    witnesses.sort_by_key(|&w| (!(on_duty(world, w) || (is_guard(world, w) && !is_petty(crime))), w));

    let mut noticed = 0usize;
    let mut recorded = 0u32;
    // Pressure pass round 9, Y2 (`OPEN-250`): who took it in (a crime in
    // the cells: the case's line is theirs too).
    let mut seen_by: Vec<EntityId> = Vec::new();
    let cap = cfg.witness_cap;
    // M15 W28: a guard notices × the law's competence multiplier (1 when off).
    let law_mult = crate::systems::competence::law_mult(world);
    // M15 W24: a Hunt's strike on its target: the street keeps quiet.
    let silence = street_silence_on(world, actor, object, crime);
    for w in witnesses {
        // L1: one record per crime and witness: a multi-minute crime (a
        // Deal's every sale) was seen afresh each minute, each sighting
        // another -0.2 of affinity, so a stranger was an Enemy in 32 minutes.
        if crate::systems::life::saw_recently(world, w, actor, crime) {
            noticed += 1;
            continue;
        }
        // Pressure pass F2: the law's eye: a guard on shift, or one off shift
        // for anything past petty (a murder is reported off duty, a fare
        // dodge seen from a commuting guard's seat is not).
        let duty = on_duty(world, w) || (is_guard(world, w) && !is_petty(crime));
        // Round 3 S7 (OPEN-225): a petty habit seen again is not news. Two
        // commutes crossing at one gate at one minute daily wrote the same
        // Fare Evasion line five days running and sank a stranger to Enemy
        // -1.00 (`gang_leader_18246` 18:43, 100k). A civilian holding a
        // sighting of the same petty crime by the same actor from the last
        // `[noise] petty_rewitness_days` takes no new record (no line, no
        // memory, no affinity hit); the law's eye always acts.
        let habit = Tick::from(world.config.noise.petty_rewitness_days) * TICKS_PER_DAY;
        if !duty && is_petty(crime) && habit > 0 && crate::systems::life::saw_within(world, w, actor, crime, habit) {
            noticed += 1;
            continue;
        }
        // Past `witness_cap` noting witnesses the crowd saw nothing it will
        // remember (the law's eye always counts).
        if cap > 0 && recorded >= cap && !duty {
            continue;
        }
        let guard = is_guard(world, w);
        let mut p = notice_probability(&cfg, stealth, guard);
        if guard && law_mult != 1.0 {
            p = (p * law_mult).clamp(0.0, 1.0);
        }
        if witness_mult != 1.0 {
            p *= witness_mult;
        }
        let roll: f32 = world.rng.world().random();
        if roll >= p {
            continue;
        }
        noticed += 1;
        if silence && !guard && silent_witness(world, w, actor, object) {
            // Known, not reported: a heard entry at hops 0 (no SawCrime,
            // so no ReportCrime), and the actor is not resented for it.
            if let Some(deed) = crate::systems::gossip::deed_of_crime(crime) {
                let e = crate::components::MemoryEntry {
                    subject: Some(actor),
                    salience,
                    valence: -salience,
                    deed: Some(deed),
                    object,
                    ..crate::components::MemoryEntry::blank(MemoryKind::Rumour, world.tick)
                };
                crate::systems::memory::hear_entry(world, w, e);
            }
            world.stats.current.word.silenced += 1;
            continue;
        }
        recorded += 1;
        // Round 8 X1 (`OPEN-246`): the held outside the scene's cells and the
        // asleep see nothing (0 is right).
        let held_out = world.has::<Sentence>(w)
            && (actor_building.is_none() || world.comp::<Position>(w).and_then(|p| p.building) != actor_building);
        if held_out || crate::systems::presence::asleep(world, w) {
            crate::systems::ownership::jobs2_row(world).witness_unfree += 1;
        }
        world.remember_crime(w, actor, crime, salience, object);
        if held_in.is_some() {
            seen_by.push(w);
        }
        crate::systems::social::witnessed_crime_of(world, w, actor);
        if let Some(n) = world.comp_mut::<Needs>(w) {
            n.safety = (n.safety - 0.2).max(0.0);
        }
        // Round 7 W5 (`OPEN-237`): a sale's witness at a stand in progress
        // is the stand's one line at its end (`stims::stand_seen`).
        if !(crime == Crime::Dealing && crate::systems::stims::stand_seen(world, actor, w)) {
            // Round 9 Y2 (`OPEN-250`): a blow names whom it fell on (V17
            // `pimp_9933 d23 07:00`: a Murder in the cells with no victim).
            let on = object
                .filter(|_| matches!(crime, Crime::Murder | Crime::Assault))
                .filter(|&o| world.has::<crate::components::Identity>(o))
                .map_or_else(String::new, |o| format!(" on {}", world.name_of(o)));
            world.push_event(
                EventKind::Witness,
                &[w, actor],
                format!("{} saw {} commit {}{on}", world.name_of(w), world.name_of(actor), crime.label()),
            );
        }
        // M16a (plan C31): a guard on the take keeps the memory, files nothing.
        // Pressure pass F2: nor does one off shift for a petty offence (a
        // commuting guard filed 113 fare dodges from his train seat and the
        // watch walked to each); there he is a witness like anyone.
        if duty && crime == Crime::FareEvasion && crate::systems::fares::on(world) {
            // Pressure pass round 2, R1: the fare's ladder on the spot (a
            // fine, a beating, a ban), never a warrant or a cell.
            if !crate::systems::contracts::on_take_of(world, w, actor) {
                crate::systems::fares::caught(world, Some(w), actor, tile);
            }
        } else if duty && !crate::systems::contracts::on_take_of(world, w, actor) {
            file_report(world, crime, actor, Some(w));
        } else if !guard {
            // The pressure pass phase 5a: the civilian reacts (report, flee,
            // gossip, blackmail); the culprit may see who saw him.
            crate::systems::consequence::witnessed(world, w, actor, crime, object);
        }
    }
    // Round 5 U7 (`OPEN-237`): the victim and the named parties are no
    // witnesses, but they know who did it: a first-hand `SawCrime` (what
    // `ReportCrime` reads; the witness roll gave them one before), with no
    // Witness line and no witness's reaction. A guard on shift who is the
    // victim files at once, as his witness roll did.
    if !detained {
        let knowers: Vec<EntityId> =
            victim.into_iter().chain(parties.iter().copied()).filter(|&k| k != actor).collect();
        for k in knowers {
            if crate::systems::robots::is_robot(world, k) || !living(world, k) || !world.has::<Brain>(k) {
                continue;
            }
            // Round 7 W2 (`OPEN-238`): a party with no object of its own
            // (a shakedown's payer) was robbed himself: his sighting is a
            // victim's, which the tell reads (`tell_the_beat`).
            let obj = if victim == Some(k) { object } else { object.or(Some(k)) };
            world.remember_crime(k, actor, crime, salience, obj);
            noticed += 1;
            if on_duty(world, k) && !is_petty(crime) && !crate::systems::contracts::on_take_of(world, k, actor) {
                file_report(world, crime, actor, Some(k));
            } else {
                // Round 6 V3 (`OPEN-238`): the victim cries out on the street.
                hue_and_cry(world, k, actor, crime, tile);
            }
        }
    }
    // M13 D41: a robot victim has no memory, edges or needs.
    let living_victim = victim.is_some_and(|v| !crate::systems::robots::is_robot(world, v) && living(world, v));
    if let Some(v) = victim.filter(|&v| !crate::systems::robots::is_robot(world, v)) {
        let kind = match crime {
            Crime::Assault | Crime::Murder | Crime::Manslaughter | Crime::Abduction | Crime::Trafficking => {
                MemoryKind::Fought
            }
            Crime::Theft
            | Crime::Extortion
            | Crime::Vagrancy
            | Crime::GrandTheft
            | Crime::Dealing
            | Crime::Intrusion
            | Crime::DataTheft
            | Crime::Conspiracy
            | Crime::Fraud
            | Crime::Blackmail
            | Crime::FareEvasion
            | Crime::ArmsDealing
            | Crime::UnlicensedArms
            | Crime::Smuggling
            | Crime::Solicitation
            | Crime::Forgery
            | Crime::BlackMarket
            | Crime::CampEscape => MemoryKind::WasRobbed,
        };
        world.remember(v, kind, Some(actor), 0.6, -0.6, false);
        crate::systems::social::robbed_by(world, v, actor);
        if let Some(n) = world.comp_mut::<Needs>(v) {
            n.safety = (n.safety - 0.4).max(0.0);
        }
        if let Some(p) = world.comp_mut::<Personality>(v) {
            p.drift(Drift::Robbed);
        }
    }
    if noticed == 0 && !detained {
        if let Some(s) = world.comp_mut::<Skills>(actor) {
            s.stealth = (s.stealth + 0.01).min(1.0);
        }
    }
    // M15 W5/W10: what someone knows goes into the crime district's pool.
    if let Some(deed) = crate::systems::gossip::deed_of_crime(crime) {
        if noticed > 0 || living_victim {
            if crime == Crime::Murder && noticed > 0 {
                match object {
                    Some(o) => crate::systems::gossip::name_actor(world, o, actor),
                    None => crate::systems::gossip::watch_killer(world, actor),
                }
            }
            let d = world.district_of(tile);
            crate::systems::gossip::post_deed(world, d, deed, Some(actor), object);
        }
    }
    // Round 8 X1 (`OPEN-246`): a crime in the cells is the law's case (held
    // when he did it: a robot's detention above books him on its own).
    if let Some(j) = held_in {
        cell_case(world, actor, object.or(victim), crime, j, &seen_by);
    }
}

/// Round 8 X1 (`OPEN-246`): the cells `id` is held in (a `Sentence`, inside
/// a building), `None` for the free.
pub fn cells_of(world: &World, id: EntityId) -> Option<EntityId> {
    if !world.has::<Sentence>(id) {
        return None;
    }
    world.comp::<Position>(id).and_then(|p| p.building)
}

/// Round 8 X1 (`OPEN-246`): a crime in the cells (`raise_crime_core`, the
/// held actor in his cells) is the law's case at once: the cells are
/// watched, the man is already inside. His reports are settled, his
/// sentence runs on (`[crime.case] cell_term_share` of the crime's
/// `sentence_ticks`, a Murder's whole term), a Murder's body
/// is the law's (found: no second case) and the line names the place, the
/// parties and the days. V16 `pimp_14518 d26 06:18`, `black_doc_79637 d27
/// 07:16`: an Assault and a Murder in the Precinct with no place, no
/// parties and no case. The under-18 rule: a child is never held (no
/// arrest force on a child), and no term is ever added to one.
fn cell_case(
    world: &mut World,
    actor: EntityId,
    other: Option<EntityId>,
    crime: Crime,
    jail: EntityId,
    seen_by: &[EntityId],
) {
    if is_petty(crime) || crate::systems::ages::is_protected(world, actor) {
        return;
    }
    // A scuffle in the cells is the warden's discipline (`[crime.case]
    // cell_term_share` of the term); a Murder there is a Murder's term.
    let full = sentence_ticks(world, crime);
    let add = if crime == Crime::Murder {
        full
    } else {
        ((full as f32 * world.config.crime.case.cell_term_share).ceil() as Tick).max(TICKS_PER_DAY)
    };
    resolve_reports(world, actor);
    let Some(until) = world.comp_mut::<Sentence>(actor).map(|s| {
        s.until_tick += add;
        s.until_tick
    }) else {
        return;
    };
    if crime == Crime::Murder {
        if let Some(k) = other.and_then(|o| world.comp_mut::<crate::components::Corpse>(o)) {
            k.found = true;
        }
    }
    let on = other.map_or_else(String::new, |o| {
        let of = if crime == Crime::Murder { "of" } else { "on" };
        format!(" {of} {}", world.name_of(o))
    });
    let days = add.div_ceil(TICKS_PER_DAY);
    let text = format!(
        "the law added {days} days to {}'s sentence for the {}{on} in the cells of {} (held now to day {})",
        world.name_of(actor),
        crime.label(),
        world.name_of(jail),
        until / TICKS_PER_DAY
    );
    let mut actors = vec![actor];
    actors.extend(other);
    actors.push(jail);
    // Round 9 Y2 (`OPEN-250`): the cellmates who saw it read the case (V17
    // `pimp_9933 d23 07:00`: a Murder seen, then no victim, Death or case).
    for &w in seen_by {
        if !actors.contains(&w) {
            actors.push(w);
        }
    }
    world.push_event(EventKind::Sentence, &actors, text);
    crate::systems::ownership::jobs2_row(world).cell_cases += 1;
}
