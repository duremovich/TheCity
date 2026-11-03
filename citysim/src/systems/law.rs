//! Law and jail: witnessing, crime reports and warrants, suspect sightings,
//! arrest and escort, sentencing with jail capacity, release, jail upkeep,
//! and the player's Arrest / Release commands.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Crime, CrimeReport, GoalKind, Job, Kit, LawShock, MemoryKind, Needs, Personality,
    Position, Posture, Role, Sentence, Skills, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::ActionKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// Chebyshev distance between two tiles.
/// Lives at work phase 7 (§ 5.2): the `Jail` key's building and a Station
/// guard's post (`systems::stations`).
pub use crate::systems::stations::{jail_for, station_post};

pub fn chebyshev(a: TilePos, b: TilePos) -> u32 {
    let dx = (i32::from(a.x) - i32::from(b.x)).unsigned_abs();
    let dy = (i32::from(a.y) - i32::from(b.y)).unsigned_abs();
    dx.max(dy)
}

/// Are two agents within `r` Chebyshev tiles, or in the same building?
pub fn near(world: &World, a: EntityId, b: EntityId, r: u32) -> bool {
    let (Some(pa), Some(pb)) = (world.comp::<Position>(a), world.comp::<Position>(b)) else { return false };
    (pa.building.is_some() && pa.building == pb.building) || chebyshev(pa.tile, pb.tile) <= r
}

/// Round 8 X1 (`OPEN-246`): a wall of the cells stands between `a` and `b`:
/// one of them is held (a `Sentence`) and they are not in the same building
/// (a cellmate is within reach; the victim at the Precinct's door, the
/// street, a passer-by are not).
pub fn held_apart(world: &World, a: EntityId, b: EntityId) -> bool {
    if !world.has::<Sentence>(a) && !world.has::<Sentence>(b) {
        return false;
    }
    let here = |x: EntityId| world.comp::<Position>(x).and_then(|p| p.building);
    here(a).is_none() || here(a) != here(b)
}

/// Round 5 U4 (`OPEN-236`): is the spot `(tile, building)` in the open?
/// Outside any building, on a building's door tile (the threshold sees both
/// ways), or inside a wall-less kind (`[crime] open_air`).
pub fn outdoors(world: &World, tile: TilePos, building: Option<EntityId>) -> bool {
    let Some(b) = building else { return true };
    world.comp::<Building>(b).is_none_or(|bd| bd.door == tile || world.config.crime.open_air.contains(&bd.kind))
}

/// Round 5 U4 (`OPEN-236`): who is indoors sees indoors. A body at `p` sees
/// a scene at `(tile, building)` when both are in the same building, or both
/// are [`outdoors`] within `reach` Chebyshev tiles; never through a wall (the
/// cells do not see the El gate, the street does not see into a flat).
/// `scene_out` is `outdoors(tile, building)`, read once by the caller.
pub fn sees_scene(
    world: &World,
    p: &Position,
    tile: TilePos,
    building: Option<EntityId>,
    scene_out: bool,
    reach: u32,
) -> bool {
    if building.is_some() && p.building == building {
        return true;
    }
    scene_out && chebyshev(p.tile, tile) <= reach && outdoors(world, p.tile, p.building)
}

/// A live agent: the entity exists and still has a Brain (corpses keep
/// Position and Identity, so `is_alive` alone is not enough).
pub fn living(world: &World, id: EntityId) -> bool {
    world.is_alive(id) && world.has::<Brain>(id)
}

pub fn is_guard(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
}

/// Pressure pass round 9, Y2 (`OPEN-250`): `id` is on leave from the post
/// now: mourning or lying low (`grief::holed_up`), or today's shift struck
/// (the employer's leave, an absence, a strike: `classes::walk_out`). V17
/// `bereaved_6559 d21 09:19`: a guard a day widowed, on leave, made arrests.
pub fn on_leave(world: &World, id: EntityId) -> bool {
    crate::systems::grief::holed_up(world, id)
        || world.comp::<Job>(id).is_some_and(|j| j.struck_shift.is_some_and(|k| k == j.shift_key_at(world.tick)))
}

/// Pressure pass F2: a guard on shift now (the law at work, whose sighting
/// of a crime is a report).
pub fn on_duty(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard && j.on_shift(world.tick_of_day()))
        && !on_leave(world, id)
}

/// Guards (any LOD) within `r` tiles of `id`, excluding `id`.
pub fn guard_within(world: &World, id: EntityId, r: u32) -> bool {
    world.guards().iter().any(|&g| g != id && near(world, id, g, r))
}

/// The witness notice probability: `0.6 − 0.5 × stealth + 0.3 × guard`.
pub fn notice_probability(cfg: &crate::config::CrimeCfg, actor_stealth: f32, witness_is_guard: bool) -> f32 {
    let p = cfg.witness_base - cfg.witness_stealth_factor * actor_stealth
        + if witness_is_guard { cfg.witness_guard_bonus } else { 0.0 };
    p.clamp(0.0, 1.0)
}

pub fn crime_salience(crime: Crime) -> f32 {
    match crime {
        Crime::Theft => 0.5,
        Crime::Extortion => 0.6,
        Crime::Assault => 0.8,
        Crime::Murder => 1.0,
        Crime::Vagrancy => 0.2,
        // Transit T10: petty.
        Crime::FareEvasion => 0.1,
        // M13 D47.
        Crime::GrandTheft => 0.6,
        Crime::Manslaughter => 0.8,
        Crime::Abduction => 0.9,
        // M13 D38.
        Crime::Dealing => 0.5,
        // M14 V18.
        Crime::Intrusion => 0.4,
        Crime::DataTheft => 0.6,
        // M16a (plan C29).
        Crime::Conspiracy => 0.8,
        // M16b § 4.
        Crime::Fraud => 0.4,
        Crime::Blackmail => 0.5,
        // Jobs & lives phase 3 (§ 4).
        Crime::ArmsDealing => 0.6,
        Crime::UnlicensedArms => 0.3,
        Crime::Smuggling => 0.6,
        // Jobs & lives phase 4: petty.
        Crime::Solicitation => 0.1,
        // Jobs & lives phase 5.
        Crime::Trafficking => 0.9,
        // Jobs & lives phase 6.
        Crime::Forgery => 0.4,
        Crime::BlackMarket => 0.5,
        // Round 7 W5: a parent's break-out of his own child.
        Crime::CampEscape => 0.5,
    }
}

/// M13 D32: an agent's stealth, its skill plus its Kit's (Eyes); read as
/// M12 when the Kit adds 0.
pub fn stealth(world: &World, id: EntityId) -> f32 {
    let skill = world.comp::<Skills>(id).map_or(0.0, |s| s.stealth);
    let kit = world.comp::<Kit>(id).map_or(0.0, |k| k.stealth);
    if kit != 0.0 {
        skill + kit
    } else {
        skill
    }
}

/// Has `suspect` been reported for anything since `tick` (open, or filed and
/// resolved after the sighting)? A witness does not re-file a crime the law
/// already dealt with.
pub fn reported_since(world: &World, suspect: EntityId, tick: Tick) -> bool {
    world.reports_on(suspect).is_some_and(|(open, last)| open > 0 || last >= tick)
}

/// Is there an open warrant on `suspect`?
pub fn wanted(world: &World, suspect: EntityId) -> bool {
    world.reports_on(suspect).is_some_and(|(open, _)| open > 0)
}

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

/// Round 6 V3 (`OPEN-238`): would `w` take what it saw of `suspect` to the
/// law (the ReportCrime goal's filters, for a word on the street): a waking,
/// free adult on its feet and wanted for nothing, `[crime.case]
/// tell_lawful_min` lawful, no gang
/// member (unless betraying), not keeping it to sell, and never on a minor
/// (CLAUDE.md, the under-18 rule: no warrant, so no arrest force, on a child).
pub fn tells_the_law(world: &World, w: EntityId, suspect: EntityId) -> bool {
    let min = world.config.crime.case.tell_lawful_min;
    living(world, w)
        && living(world, suspect)
        && w != suspect
        && !wanted(world, w)
        && crate::systems::demography::is_adult(world, w)
        && !crate::systems::ages::is_protected(world, suspect)
        && !world.has::<Sentence>(w)
        && !crate::systems::wounded::is_down(world, w)
        && world.comp::<crate::components::Personality>(w).is_some_and(|p| p.lawfulness >= min)
        && (world.gang_of(w).is_none() || world.comp::<Brain>(w).is_some_and(|b| b.betraying))
        && world
            .comp::<Brain>(w)
            .is_some_and(|b| !matches!(b.exec, crate::exec::ExecState::Use { kind: ActionKind::Sleep, .. }))
        && !crate::systems::consequence::keeps_quiet(world, w, suspect)
        && fears(world, w, suspect).is_none()
}

/// Round 7 W2 (`OPEN-238`): `suspect`'s gang holds the block `w` lives on
/// and `w` is under `[crime.case] fear_courage_max` brave: the gang, feared.
fn fears(world: &World, w: EntityId, suspect: EntityId) -> Option<EntityId> {
    let g = world.gang_of(suspect)?;
    if courage(world, w) >= world.config.crime.case.fear_courage_max || world.gang_of(w) == Some(g) {
        return None;
    }
    (crate::systems::turf::home_holder(world, w) == Some(g)).then_some(g)
}

/// Round 7 W2 (`OPEN-238`): why `w` keeps what he knows of `suspect` from
/// the law (`tells_the_law` false), for his line: his gang, the suspect's
/// gang on his block, the sale, his own warrant, his nature; `None` when he
/// would tell.
pub fn keeps_from_law(world: &World, w: EntityId, suspect: EntityId) -> Option<String> {
    if tells_the_law(world, w, suspect) {
        return None;
    }
    let (he, him, his) = crate::systems::consequence::pro(world, w);
    // The under-18 rule: a child is never named a culprit, not even here.
    if crate::systems::ages::is_protected(world, suspect) {
        return Some(format!("{he} says nothing"));
    }
    let gname = |g: EntityId| world.comp::<crate::components::Gang>(g).map_or_else(String::new, |x| x.name.clone());
    let betraying = world.comp::<Brain>(w).is_some_and(|b| b.betraying);
    if let Some(g) = world.gang_of(w).filter(|_| !betraying) {
        return Some(if world.gang_of(suspect) == Some(g) {
            format!("{} runs with {}, {his} own crew", world.name_of(suspect), gname(g))
        } else {
            format!("{he} runs with {}, and the law is not their way", gname(g))
        });
    }
    if let Some(g) = fears(world, w, suspect) {
        return Some(format!("afraid: {} hold the block {he} lives on", gname(g)));
    }
    if crate::systems::consequence::keeps_quiet(world, w, suspect) {
        return Some(format!("it is worth something to {him}"));
    }
    if wanted(world, w) {
        return Some(format!("{he} is wanted {him}self"));
    }
    let min = world.config.crime.case.tell_lawful_min;
    if world.comp::<crate::components::Personality>(w).is_some_and(|p| p.lawfulness < min) {
        return Some(format!("{he} is not one to go to the law"));
    }
    Some(format!("{he} says nothing"))
}

/// Round 6 V3 (`OPEN-238`): the nearest on-duty city guard outdoors within
/// `[crime.case] hue_reach` of `tile` (Chebyshev; ties the lower id), not on
/// `suspect`'s take.
fn guard_in_earshot(world: &World, tile: TilePos, suspect: EntityId) -> Option<EntityId> {
    let reach = world.config.crime.case.hue_reach;
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| g != suspect && on_duty(world, g) && is_city_guard(world, g) && !world.has::<Sentence>(g))
        .filter_map(|g| {
            let p = world.comp::<Position>(g)?;
            let d = chebyshev(p.tile, tile);
            (d <= reach && outdoors(world, p.tile, p.building)).then_some((d, g))
        })
        .filter(|&(_, g)| !crate::systems::contracts::on_take_of(world, g, suspect))
        .min()
        .map(|(_, g)| g)
}

/// Round 6 V3 (`OPEN-238`): a victim on the street shouts, and an on-duty
/// guard in earshot takes the report on the spot (the witness the victim,
/// the suspect seen there). V14 `vendor_9648 d23`: ten thieves on one till,
/// a SawCrime each, and no report (the Hall out of `report_ticks` reach).
pub fn hue_and_cry(world: &mut World, victim: EntityId, actor: EntityId, crime: Crime, tile: TilePos) {
    if is_petty(crime) || !tells_the_law(world, victim, actor) {
        return;
    }
    let Some((vt, vb)) = world.comp::<Position>(victim).map(|p| (p.tile, p.building)) else { return };
    if !outdoors(world, vt, vb) {
        return;
    }
    let Some(g) = guard_in_earshot(world, vt, actor) else { return };
    let (vn, gn, an) = (world.name_of(victim), world.name_of(g), world.name_of(actor));
    let text = format!("{vn} cried out and {gn} took the report: {an} for {}", crime.label());
    if file_report_as(world, crime, actor, Some(victim), Some((text, &[victim, g]))) {
        world.set_last_seen(actor, (tile, world.tick));
    }
}

/// Round 6 V3 (`OPEN-238`), hourly: an on-duty city guard on the street
/// meets the people around him (`[crime.case] hue_reach`); one holding a
/// first-hand sighting from the last `tell_days` (as its victim, or of a
/// Murder) of a suspect not reported since tells him, and he takes the
/// report. The street's way to the law where the one Hall is out of reach.
fn tell_the_beat(world: &mut World) {
    let cfg = world.config.crime.case.clone();
    let since = world.tick.saturating_sub(cfg.tell_days * TICKS_PER_DAY);
    let guards: Vec<EntityId> = world
        .guards()
        .iter()
        .copied()
        .filter(|&g| on_duty(world, g) && is_city_guard(world, g) && !world.has::<Sentence>(g))
        .collect();
    // One tell a person a pass (V3 shadow: one victim told five guards five
    // suspects in one minute).
    let mut tellers: BTreeSet<EntityId> = BTreeSet::new();
    for g in guards {
        let Some((gt, gb)) = world.comp::<Position>(g).map(|p| (p.tile, p.building)) else { continue };
        if !outdoors(world, gt, gb) {
            continue;
        }
        let near: Vec<EntityId> = world.agents_near(gt, cfg.hue_reach, None);
        for w in near {
            if w == g
                || tellers.contains(&w)
                || world.comp::<Brain>(w).is_none_or(|b| b.lod == crate::components::Lod::Statistical)
            {
                continue;
            }
            let Some((wt, wb)) = world.comp::<Position>(w).map(|p| (p.tile, p.building)) else { continue };
            if !outdoors(world, wt, wb) || chebyshev(wt, gt) > cfg.hue_reach {
                continue;
            }
            let Some(mem) = world.comp::<crate::components::Memory>(w) else { continue };
            let told = mem
                .entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.tick >= since)
                .filter_map(|e| Some((e.subject?, e.crime?, e.object, e.tick, e.salience)))
                .filter(|&(_, c, o, _, _)| !is_petty(c) && (o == Some(w) || c == Crime::Murder))
                .filter(|&(s, _, _, t, _)| !reported_since(world, s, t))
                .max_by(|a, b| a.4.total_cmp(&b.4))
                .map(|(s, c, _, _, _)| (s, c));
            let Some((s, crime)) = told else { continue };
            if !tells_the_law(world, w, s) || crate::systems::contracts::on_take_of(world, g, s) {
                continue;
            }
            let (wn, gn, sn) = (world.name_of(w), world.name_of(g), world.name_of(s));
            let text = format!("{wn} told {gn} on the street: {sn}, for {}", crime.label());
            tellers.insert(w);
            file_report_as(world, crime, s, Some(w), Some((text, &[w, g])));
        }
    }
}

/// Round 8 X1 (`OPEN-246`): the law's desks open now: the Halls by day
/// (Morning, Work, Evening: ReportCrime's hours), the Precinct and the
/// Stations around the clock (a desk sergeant at each).
fn desks_open(world: &World) -> Vec<EntityId> {
    use crate::time::DayPhase;
    let mut v: Vec<EntityId> = Vec::new();
    if matches!(world.phase(), DayPhase::Morning | DayPhase::Work | DayPhase::Evening) {
        v.extend(world.buildings_of_kind(BuildingKind::Hall).iter().copied());
    }
    v.extend(world.buildings_of_kind(BuildingKind::Jail).iter().copied());
    v.extend(world.buildings_of_kind(BuildingKind::Station).iter().copied());
    v.retain(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished));
    v
}

/// Round 8 X1: the nearest open desk by the trip from where `id` sets out
/// (`distance::trip_to`; ties the lower id), with that trip; `None` past
/// `[crime.case] word_reach_ticks`.
pub fn law_desk(world: &World, id: EntityId) -> Option<(EntityId, Tick)> {
    let (t, b) = desks_open(world)
        .into_iter()
        .filter_map(|b| crate::systems::distance::trip_to(world, id, b).map(|t| (t, b)))
        .min()?;
    (t <= world.config.crime.case.word_reach_ticks).then_some((b, t))
}

/// Round 8 X1: the promise `w` carries: the first standing word owed
/// (`consequence::owed_word`) on a suspect he would still name now.
pub fn word_owed(world: &World, w: EntityId) -> Option<EntityId> {
    let now = world.tick;
    world
        .consequence
        .owed_word
        .iter()
        .find(|&&(v, s, until)| v == w && until > now && !world.has::<Sentence>(s) && tells_the_law(world, w, s))
        .map(|&(_, s, _)| s)
}

/// Round 8 X1: `w` gave his word to tell the law of `s` and it stands
/// (the Fight goal leaves him to the law).
pub fn owes_word_on(world: &World, w: EntityId, s: EntityId) -> bool {
    let now = world.tick;
    world.consequence.owed_word.iter().any(|&(v, x, until)| v == w && x == s && until > now)
}

/// Round 8 X1: the promise's plan: the nearest open desk, the report there
/// (`ReportCrime` on the suspect: `exec::actions::report_crime` files it).
pub fn word_plan(world: &World, w: EntityId) -> Option<crate::goap::Plan> {
    use crate::components::{ActionInstance, GoalKind};
    let s = word_owed(world, w)?;
    let (desk, _) = law_desk(world, w)?;
    let steps = vec![
        ActionInstance { action: ActionKind::GoTo(crate::goap::LocationKey::Hall), target: Some(desk), tile: None },
        ActionInstance { action: ActionKind::ReportCrime, target: Some(s), tile: None },
    ];
    Some(crate::goap::Plan { goal: GoalKind::ReportCrime, target: Some(s), steps, started_tick: world.tick })
}

/// Round 8 X1: `w` gives the law his word on `s` at `desk` (the ReportCrime
/// step at the desk, or `off`: a Statistical promiser's hour at it): the
/// report in his name (his freshest first-hand sighting of `s`, else the
/// shakedown). A refreshed warrant files no new report: the word is given
/// all the same (`file_report_as` reads `consequence::word_given` first).
pub fn told_at_desk(world: &mut World, w: EntityId, s: EntityId, desk: Option<EntityId>, off: bool) {
    let crime = world
        .comp::<crate::components::Memory>(w)
        .and_then(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.subject == Some(s))
                .max_by_key(|e| e.tick)
                .and_then(|e| e.crime)
        })
        .unwrap_or(Crime::Extortion);
    let at = desk.map_or_else(|| "the law".to_string(), |d| world.name_of(d));
    let off = if off { " (off screen)" } else { "" };
    let text =
        format!("{} went to {at} and told the law: {}, for {}{off}", world.name_of(w), world.name_of(s), crime.label());
    let mut actors = vec![w];
    actors.extend(desk);
    file_report_as(world, crime, s, Some(w), Some((text, &actors)));
    if let Some(p) = world.comp_mut::<Personality>(w) {
        p.drift(Drift::ReportedCrime);
    }
}

/// Round 8 X1: a Statistical promiser is up and off his shift this hour
/// (`presence::stat_home`'s schedule: not the night 00:00-06:00, not on a
/// shift he works).
fn stat_free(world: &World, w: EntityId) -> bool {
    if u64::from(world.tick_of_day()) < 6 * crate::time::TICKS_PER_HOUR {
        return false;
    }
    world.comp::<Job>(w).is_none_or(|j| {
        !(j.employer.is_some()
            && j.on_shift(world.tick_of_day())
            && crate::exec::routine::workday_of(world, w, j, j.shift_key_at(world.tick)))
    })
}

/// Pressure pass round 9, Y2 (`OPEN-250`): the tick `w`'s freshest
/// first-hand sighting of `s` goes stale to the beat (`tell_the_beat`'s
/// `[crime.case] tell_days` window); `None` with no such sighting.
fn beat_until(world: &World, w: EntityId, s: EntityId) -> Option<Tick> {
    let days = world.config.crime.case.tell_days;
    if days == 0 {
        return None;
    }
    let seen = world
        .comp::<crate::components::Memory>(w)?
        .entries
        .iter()
        .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.subject == Some(s))
        .map(|e| e.tick)
        .max()?;
    Some(seen + days * TICKS_PER_DAY)
}

/// Round 8 X1: `w` drops the word owed on `s`, the reason on his line.
pub fn drop_word(world: &mut World, w: EntityId, s: EntityId, why: &str) {
    let before = world.consequence.owed_word.len();
    world.consequence.owed_word.retain(|&(v, x, _)| !(v == w && x == s));
    if world.consequence.owed_word.len() == before {
        return;
    }
    crate::systems::ownership::jobs2_row(world).word_dropped += 1;
    let text = format!("{} never told the law of {}: {why}", world.name_of(w), world.name_of(s));
    world.push_event(EventKind::WitnessReacted, &[w, s], text);
}

/// Round 8 X1 (`OPEN-246`), hourly: each standing promise is kept (the
/// ReportCrime goal, the tell, the hue), waits (asleep, down) or is dropped
/// with the reason: the man taken in, the suspect held or dead, the teller
/// taken in himself, his gang, fear, his own warrant, the hours gone by with
/// no desk in reach. V16: 5 of 7 "means to tell" in the diaries never did.
fn keep_word(world: &mut World) {
    if world.consequence.owed_word.is_empty() {
        return;
    }
    let now = world.tick;
    let words = world.consequence.owed_word.clone();
    for (v, s, until) in words {
        if !living(world, v) {
            world.consequence.owed_word.retain(|&(x, y, _)| !(x == v && y == s));
            continue;
        }
        let (he, _, _) = crate::systems::consequence::pro(world, v);
        let why = if !living(world, s) {
            Some(format!("{} is dead", world.name_of(s)))
        } else if world.has::<Sentence>(s) {
            Some(format!("{} is in the cells already", world.name_of(s)))
        } else if world.has::<Sentence>(v) {
            Some(format!("{he} was taken in first"))
        } else if crate::systems::presence::asleep(world, v) || crate::systems::wounded::is_down(world, v) {
            // He tells when he is up (or the word runs out below).
            (until <= now).then(|| {
                format!(
                    "{he} lay {} and the day went by",
                    if crate::systems::presence::asleep(world, v) { "asleep" } else { "hurt" }
                )
            })
        } else if !tells_the_law(world, v, s) {
            keeps_from_law(world, v, s)
        } else if world.comp::<Brain>(v).is_some_and(|b| b.lod == crate::components::Lod::Statistical)
            && stat_free(world, v)
            && law_desk(world, v).is_some()
        {
            // Off screen, no brain to carry the goal: his first free waking
            // hour with a desk open in reach is the walk to it.
            let desk = law_desk(world, v).map(|(d, _)| d);
            told_at_desk(world, v, s, desk, true);
            None
        } else if until <= now {
            match law_desk(world, v) {
                // Pressure pass round 9, Y2 (`OPEN-250`): no desk within a
                // walk, the word waits on the beat (`tell_the_beat`, the
                // street's way to the law) for as long as the sighting is
                // fresh to it (`[crime.case] tell_days`); dropped then. V17
                // `night_shift_24245 d22`: "never told: the law is out of
                // reach" at 07:00, then a guard on the street told at 23:00.
                None => match beat_until(world, v, s).filter(|&t| t > now) {
                    Some(t) => {
                        if let Some(x) = world.consequence.owed_word.iter_mut().find(|x| x.0 == v && x.1 == s) {
                            x.2 = t;
                        }
                        None
                    }
                    None => Some(
                        "the law is out of reach (no desk of it within a walk, and no guard on the beat came by)"
                            .to_string(),
                    ),
                },
                Some((d, _)) => Some(format!("the day went by and {he} never got to {}", world.name_of(d))),
            }
        } else {
            None
        };
        if let Some(why) = why {
            drop_word(world, v, s, &why);
        }
    }
}

/// M15 W24: is this an Attack's crime by a hunter on its Hunt's target,
/// with `[hunt] street_silence` on?
pub fn street_silence_on(world: &World, actor: EntityId, object: Option<EntityId>, crime: Crime) -> bool {
    world.config.hunt.street_silence
        && matches!(crime, Crime::Murder | Crime::Assault)
        && object.is_some()
        && world.hunts.get(&actor).is_some_and(|s| Some(s.target) == object)
}

/// M15 W24: a witness the street keeps quiet: one holding a grudge on the
/// victim, or a Friend of the actor.
pub fn silent_witness(world: &World, w: EntityId, actor: EntityId, victim: Option<EntityId>) -> bool {
    victim.is_some_and(|v| crate::systems::grudges::holds(world, w, v, 0.0))
        || world.edge(w, actor).is_some_and(|e| e.kind == crate::components::RelKind::Friend)
}

/// File (or refresh) the one open report per `(suspect, crime)`.
pub fn file_report(world: &mut World, crime: Crime, suspect: EntityId, witness: Option<EntityId>) {
    file_report_as(world, crime, suspect, witness, None);
}

/// `file_report` with the Report line's text and extra actors given (Round 6
/// V3: the law's case on a found body, a guard taking a victim's word);
/// `None` is the plain "X reported Y for Z". True when a new report filed.
pub fn file_report_as(
    world: &mut World,
    crime: Crime,
    suspect: EntityId,
    witness: Option<EntityId>,
    line: Option<(String, &[EntityId])>,
) -> bool {
    // Pressure pass round 2, R1: a fare is settled at the gate
    // (`systems::fares`), nobody's warrant.
    if crime == Crime::FareEvasion && crate::systems::fares::on(world) {
        return false;
    }
    let tick = world.tick;
    // Round 7 W2 (`OPEN-238`): a robbed sleeper's word reached the law.
    if let Some(w) = witness {
        crate::systems::consequence::word_given(world, crime, suspect, w);
    }
    if world.refresh_open_report(crime, suspect, witness) {
        return false;
    }
    world.push_report(CrimeReport { crime, suspect, witness, tick, resolved: false });
    // M13 D38: a new Dealing report (a refreshed one is the same warrant).
    if crime == Crime::Dealing {
        world.stats.current.dealing_reports += 1;
    }
    match line {
        Some((text, actors)) => {
            let mut all: Vec<EntityId> = vec![suspect];
            all.extend(actors.iter().copied().filter(|&a| a != suspect));
            world.push_event(EventKind::Report, &all, text);
        }
        None => {
            let who = witness.map_or("the city".to_string(), |w| world.name_of(w));
            world.push_event(
                EventKind::Report,
                &[suspect],
                format!("{who} reported {} for {}", world.name_of(suspect), crime.label()),
            );
        }
    }
    // Fix pass (phase 2 review): sleeping rough is not gang business; a
    // Vagrancy report against a member must not feed the wanted-gang log,
    // the pressure or the district's top gang.
    if crime != Crime::Vagrancy && crime != Crime::FareEvasion {
        crate::systems::law_brain::log_report(world, suspect);
    }
    true
}

/// Is `a` at work: on shift inside its employer, or on a Work plan (the
/// commute to or from a shift included)? A person at work is not loitering.
pub fn at_work(world: &World, a: EntityId) -> bool {
    let Some(job) = world.comp::<Job>(a) else { return false };
    if job.role == Role::Guard {
        return false;
    }
    let inside = job.employer.is_some() && job.employer == world.comp::<Position>(a).and_then(|p| p.building);
    (inside && job.on_shift(world.tick_of_day()))
        || world
            .comp::<Brain>(a)
            .is_some_and(|b| b.plan.as_ref().is_some_and(|p| p.goal == crate::components::GoalKind::Work))
}

/// Is Vagrancy the only thing `s` is wanted for, while `s` is at work? Such a
/// suspect is not chased: a night-shift worker with no bed is not loitering.
fn vagrant_at_work(world: &World, s: EntityId) -> bool {
    if !at_work(world, s) {
        return false;
    }
    // Scale S1: from the report index (this walked every report).
    world.wanted_only_for_vagrancy(s)
}

/// A wanted suspect the law can chase now: free, uncuffed and seen by a
/// guard within `suspect_seen_ticks`. Callers pair it with an open report.
fn located(world: &World, s: EntityId) -> bool {
    let window = world.config.crime.suspect_seen_ticks;
    let tick = world.tick;
    // L1: a corpse keeps its Position and Identity (`is_alive`), and guards
    // walked hours to arrest the dead. The cheap checks first: the vagrancy
    // test walks the reports (a pure conjunction, reordered).
    (living(world, s))
        && !world.has::<Sentence>(s)
        && world.comp::<Brain>(s).is_some_and(|b| b.cuffed_by.is_none())
        && world.last_seen().get(&s).is_some_and(|&(_, t)| tick.saturating_sub(t) <= window)
        && !vagrant_at_work(world, s)
}

/// Who a guard can chase: `Some((tile, guard))` keeps the located suspects
/// last seen within `[law] pursuit_radius` tiles of `tile` (M10), and never
/// the guard themselves (one who chased their own warrant cuffed themselves,
/// with no escort to end it, and starved); `None` is the whole city.
#[derive(Clone, Copy, Debug)]
pub struct Pursuer {
    pub tile: TilePos,
    pub guard: EntityId,
}

/// M11 D18: a private guard chases only nearby warrants.
pub fn pursuit_radius_for(world: &World, guard: EntityId) -> u32 {
    if is_private_guard(world, guard) {
        world.config.corps.private_pursuit_radius
    } else {
        world.config.law.pursuit_radius
    }
}

/// M11 D18: a guard job at a Security Office (the one test behind
/// `is_private_guard`, `is_city_guard` and `routine::workplace_key_for`).
pub fn job_is_private_guard(world: &World, job: &Job) -> bool {
    job.role == Role::Guard
        && job.employer.and_then(|e| world.comp::<Building>(e)).is_some_and(|b| b.kind == BuildingKind::SecurityOffice)
}

/// M11 D18: a guard employed by a Security Office.
pub fn is_private_guard(world: &World, guard: EntityId) -> bool {
    world.comp::<Job>(guard).is_some_and(|j| job_is_private_guard(world, j))
}

/// A guard on the city payroll: a guard who is not a Security Office's
/// (employed at the Precinct, or by nobody). The complement of
/// `is_private_guard` among guards.
pub fn is_city_guard(world: &World, guard: EntityId) -> bool {
    world.comp::<Job>(guard).is_some_and(|j| j.role == Role::Guard && !job_is_private_guard(world, j))
}

/// Shadow V4 bug 11: a private guard polices his corp's beat: a suspect
/// last seen within `[corps] private_beat_tiles` of his office or one of its
/// clients (the corp guards chased warrants city-wide). A city guard's beat
/// is the city. Applied in `chaseable`, so every pursuer path (the Arrest
/// goal, its plan's bind, `any_located_suspect`) reads it.
pub fn on_beat(world: &World, guard: EntityId, s: EntityId) -> bool {
    if !is_private_guard(world, guard) {
        return true;
    }
    let Some(&(seen, _)) = world.last_seen().get(&s) else { return false };
    on_beat_at(world, guard, seen)
}

/// `on_beat` for a private guard and a sighting tile.
fn on_beat_at(world: &World, guard: EntityId, seen: TilePos) -> bool {
    let max = world.config.corps.private_beat_tiles;
    let Some(office) = world.comp::<Job>(guard).and_then(|j| j.employer) else { return false };
    let near = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| bd.door.manhattan(seen) <= max);
    if near(office) {
        return true;
    }
    let Some(corp) = world.corp_of_building(office) else { return false };
    world.comp::<crate::components::Corp>(corp).is_some_and(|c| c.contracts.iter().any(|&(b, _)| near(b)))
}

/// `claimed`: the suspects other guards already chase (L1), built once per
/// scoring call by `claim_set`, on its first read, with the pursuer's
/// `pursuit_radius_for`. `seen`: `s`'s `World::last_seen` entry.
fn chaseable(world: &World, s: EntityId, seen: (TilePos, Tick), by: Option<Pursuer>, claimed: &Claims<'_>) -> bool {
    let near = |p: Pursuer| {
        s != p.guard
            && seen.0.manhattan(p.tile) <= claimed.radius
            // L1: a stale sighting is chased only from near by, and a
            // suspect another guard is after is theirs.
            && crate::systems::life::sighting_fresh_at(world, seen, p.tile)
            && !claimed.contains(s)
            // Round 7 W3: a warrant handed to another guard (his collar) is his.
            && !handed_to_other(world, p.guard, s)
            // M16a (plan C31): never the buyer of a guard on the take.
            && !crate::systems::contracts::on_take_of(world, p.guard, s)
            // Shadow V4 bug 11: a corp guard's warrants on its beat only.
            && (!claimed.private || on_beat_at(world, p.guard, seen.0))
    };
    // The cheap reach test first (the same conjunction, reordered).
    by.is_none_or(near) && located(world, s)
}

/// Round 7 W3 (`OPEN-244`): `s` is a guard other than `guard`'s collar by
/// hand-on (`handoffs`, inside `[justice] collar_hours`): a warrant taken to
/// the door was served by whichever guard stood nearer.
fn handed_to_other(world: &World, guard: EntityId, s: EntityId) -> bool {
    if world.handoffs.is_empty() {
        return false;
    }
    let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
    let since = world.tick.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    world.handoffs.iter().any(|(&g, &(x, t))| g != guard && x == s && t >= since && living(world, g))
}

/// The L1 claim set for a pursuer (empty for the whole city). The perf
/// pass phase 5: read from the world's claim index
/// (`World::claimed_by_other`), kept current by marks, not built per
/// scoring call by walking every guard (`life::claimed_suspects`, which a
/// unit test's `spatial::scans()` still reads).
struct Claims<'a> {
    world: &'a World,
    guard: Option<EntityId>,
    /// The pursuer's `pursuit_radius_for` (0 for the whole city: unread).
    radius: u32,
    /// The pursuer is a private (corp) guard: `on_beat` applies.
    private: bool,
    /// The scan's set, built on the first read (`spatial::scans()` only).
    set: std::cell::OnceCell<BTreeSet<EntityId>>,
}

impl Claims<'_> {
    fn contains(&self, s: EntityId) -> bool {
        match self.guard {
            Some(g) if crate::spatial::scans() => {
                self.set.get_or_init(|| crate::systems::life::claimed_suspects(self.world, g)).contains(&s)
            }
            Some(g) => self.world.claimed_by_other(g, s),
            None => false,
        }
    }
}

fn claim_set(world: &World, by: Option<Pursuer>) -> Claims<'_> {
    let radius = by.map_or(0, |p| pursuit_radius_for(world, p.guard));
    let private = by.is_some_and(|p| is_private_guard(world, p.guard));
    Claims { world, guard: by.map(|p| p.guard), radius, private, set: std::cell::OnceCell::new() }
}

/// Scale S1: the open-warrant suspects `located` can pass for `by`,
/// ascending, with their `last_seen` entries: those last seen inside
/// `[crime] suspect_seen_ticks` (`World::located_candidates`, memoised per
/// tick) and, for a pursuer, inside its reach's square (the memo's grid),
/// not every open warrant in the city (a guard's every think walked them all).
fn located_candidates(world: &World, by: Option<Pursuer>, claimed: &Claims<'_>) -> Vec<(EntityId, (TilePos, Tick))> {
    let located = world.located_candidates(world.config.crime.suspect_seen_ticks);
    match by {
        Some(p) => located.near(p.tile, claimed.radius),
        None => located.list.clone(),
    }
}

/// Open-warrant suspects that are located (and in reach of `by`), ascending.
pub fn located_suspects(world: &World, by: Option<Pursuer>) -> Vec<EntityId> {
    let claimed = claim_set(world, by);
    located_candidates(world, by, &claimed)
        .into_iter()
        .filter(|&(s, t)| chaseable(world, s, t, by, &claimed))
        .map(|(s, _)| s)
        .collect()
}

/// Pressure pass round 3, S3 (`OPEN-223`, `[justice] collar_hours`): the
/// suspect `guard` saw commit a crime past petty himself (the open report
/// he filed in the last `collar_hours`), when it is located and chaseable
/// by him: his collar, the Arrest goal's target on shift or off (a guard
/// at 100k witnessed 132 crimes in a week and arrested none: off shift the
/// Arrest goal is shut, and on shift the nearest warrant won). The newest.
pub fn collar_of(world: &World, guard: EntityId) -> Option<EntityId> {
    let hours = world.config.justice.as_ref()?.collar_hours;
    if hours == 0 || !is_guard(world, guard) {
        return None;
    }
    let since = world.tick.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    let mut mine: Vec<(Tick, EntityId)> = world
        .crime_reports()
        .iter()
        .filter(|r| !r.resolved && r.witness == Some(guard) && r.tick >= since && !is_petty(r.crime))
        .map(|r| (r.tick, r.suspect))
        .collect();
    // Round 4 T3 (OPEN-222): a chase handed to him (`chase_dropped`) is his
    // collar for as long.
    if let Some(&(s, t)) = world.handoffs.get(&guard) {
        if t >= since {
            mine.push((t, s));
        }
    }
    if mine.is_empty() {
        return None;
    }
    let tile = world.comp::<Position>(guard)?.tile;
    let radius = pursuit_radius_for(world, guard);
    let private = is_private_guard(world, guard);
    // `chaseable` but for the claim: the witness's collar is his before any
    // guard who heard the report (another's chase from across the district
    // claimed it first and lost the thief, while the guard who saw it walked
    // on to his desk).
    mine.into_iter()
        .filter(|&(_, s)| {
            s != guard
                && wanted(world, s)
                && located(world, s)
                && world.last_seen().get(&s).is_some_and(|&seen| {
                    seen.0.manhattan(tile) <= radius
                        && crate::systems::life::sighting_fresh_at(world, seen, tile)
                        && (!private || on_beat_at(world, guard, seen.0))
                })
                && !crate::systems::contracts::on_take_of(world, guard, s)
        })
        .max()
        .map(|(_, s)| s)
}

/// Round 4 T3 (`OPEN-222`): guard `guard`'s Arrest plan ends without the
/// arrest (a re-think's `winner`, or a failed step with `None`): one
/// `Pursuit` line says why (the warrant closed, the trail cold, another
/// guard on him, out of his reach, the shift over, another want first; a
/// failed step with the chase still on says nothing: he re-plans it) and,
/// while the suspect is still wanted and located, hands the case to the
/// nearest free guard on shift who can chase him (`handoffs`: his collar
/// for `[justice] collar_hours`). V12's `guard_8440 d23 11:10` chased Zane
/// Oakes four legs, then `Arrest -> Unwind` at 13:10 with no line (his
/// collar lapsed off shift); 20k seed 42 x 30 had 347 such ends. A chase
/// that made its arrest (escorting) or whose suspect is dead says nothing.
pub fn chase_dropped(world: &mut World, guard: EntityId, winner: Option<GoalKind>) {
    let Some(b) = world.comp::<Brain>(guard) else { return };
    if b.escorting.is_some() {
        return;
    }
    let Some(s) = b.plan.as_ref().filter(|p| p.goal == GoalKind::Arrest).and_then(|p| p.target) else { return };
    if s == guard || !living(world, s) {
        return;
    }
    let now = world.tick;
    let held = world.has::<Sentence>(s) || world.comp::<Brain>(s).is_some_and(|x| x.cuffed_by.is_some());
    let seen = world.last_seen().get(&s).copied();
    let here = world.comp::<Position>(guard).map(|p| p.tile);
    let on = world.comp::<Job>(guard).is_some_and(|j| {
        j.on_shift(world.tick_of_day()) && crate::exec::routine::workday_of(world, guard, j, j.shift_key_at(now))
    });
    let open = wanted(world, s) && !held;
    let out_of_reach = seen.zip(here).is_some_and(|((t, _), h)| t.manhattan(h) > pursuit_radius_for(world, guard));
    // A failed step (the suspect gone from the tile he was seen on, or he
    // fought free) with the chase still on: the guard re-plans it, silent.
    if winner.is_none() && open && located(world, s) && !world.claimed_by_other(guard, s) && !out_of_reach {
        return;
    }
    let why = if held {
        "already in custody".to_string()
    } else if !wanted(world, s) {
        "the warrant is closed".to_string()
    } else if !located(world, s) {
        match seen {
            Some((_, t)) => {
                format!("the trail went cold: last seen {} h ago", now.saturating_sub(t) / crate::time::TICKS_PER_HOUR)
            }
            None => "the trail went cold".to_string(),
        }
    } else if world.claimed_by_other(guard, s) {
        "another guard has the chase".to_string()
    } else if out_of_reach {
        "out of reach".to_string()
    } else if !on {
        "the shift is over".to_string()
    } else {
        winner.map_or_else(|| "got away".to_string(), |w| format!("{w:?} came first"))
    };
    let free = open && located(world, s) && !world.claimed_by_other(guard, s);
    let receiver =
        if free { seen.and_then(|(at, _)| handoff_guard(world, guard, s, at).map(|r| (r, at))) } else { None };
    let (gname, sname) = (world.name_of(guard), world.name_of(s));
    let text = match receiver {
        Some((r, at)) => {
            let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
            let since = now.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
            world.handoffs.retain(|_, &mut (_, t)| t >= since);
            world.handoffs.insert(r, (s, now));
            let far = world.comp::<Position>(r).map_or(0, |p| p.tile.manhattan(at));
            format!(
                "{gname} gave up the chase of {sname} ({why}): handed to {} on shift, {far} tiles off",
                world.name_of(r)
            )
        }
        None if free => {
            format!("{gname} gave up the chase of {sname} ({why}): no guard on shift near; the warrant stays open")
        }
        None => format!("{gname} gave up the chase of {sname} ({why})"),
    };
    let mut actors = vec![guard, s];
    actors.extend(receiver.map(|(r, _)| r));
    world.push_event(EventKind::Pursuit, &actors, text);
}

/// Pressure pass round 7, W3 (`OPEN-244`; `[justice] door_from_hour`..
/// `door_to_hour`): on the hour at night, each open warrant whose suspect no
/// guard has seen within `[crime] suspect_seen_ticks` (the chase never starts
/// without a sighting: 296 warrants stood at day 30 of 20k x 30, a guard made
/// 0.65 arrests a week) and who is inside his own Home goes to the nearest
/// free city guard on shift within his pursuit radius (`handoff_guard`'s
/// tests, one warrant a guard): the address is the sighting
/// (`World::set_last_seen` at the door, he is in) and the warrant his collar
/// (`handoffs`), so the arrest is made at home. Never a minor
/// (`ages::is_protected`), nor a warrant for sleeping rough alone.
fn serve_at_door(world: &mut World) {
    let Some((from, to)) = world.config.justice.as_ref().map(|c| (c.door_from_hour, c.door_to_hour)) else { return };
    if from == to {
        return;
    }
    let hour = (world.tick_of_day() / crate::time::TICKS_PER_HOUR as u16) as u8;
    let open = if from < to { hour >= from && hour < to } else { hour >= from || hour < to };
    if !open {
        return;
    }
    let now = world.tick;
    let window = world.config.crime.suspect_seen_ticks;
    let hours = world.config.justice.as_ref().map_or(0, |c| c.collar_hours);
    let since = now.saturating_sub(hours * crate::time::TICKS_PER_HOUR);
    let tod = world.tick_of_day();
    // The free city guards on shift, with bodies, holding no warrant now.
    let mut free: Vec<(EntityId, TilePos)> = world
        .guards()
        .iter()
        .copied()
        .filter(|&g| is_city_guard(world, g) && living(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| !world.handoffs.get(&g).is_some_and(|&(_, t)| t >= since))
        .filter(|&g| {
            world.comp::<Brain>(g).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical
                    && b.cuffed_by.is_none()
                    && b.escorting.is_none()
                    && b.plan_goal() != Some(GoalKind::Arrest)
            })
        })
        .filter(|&g| {
            world
                .comp::<Job>(g)
                .is_some_and(|j| j.on_shift(tod) && crate::exec::routine::workday_of(world, g, j, j.shift_key_at(now)))
                && !on_leave(world, g)
        })
        .filter_map(|g| world.comp::<Position>(g).map(|p| (g, p.tile)))
        .collect();
    if free.is_empty() {
        return;
    }
    let suspects: Vec<EntityId> = world.open_suspects().collect();
    for s in suspects {
        if free.is_empty() {
            break;
        }
        if !living(world, s)
            || world.has::<Sentence>(s)
            || crate::systems::ages::is_protected(world, s)
            || world.comp::<Brain>(s).is_none_or(|b| b.cuffed_by.is_some())
            || world.last_seen().get(&s).is_some_and(|&(_, t)| now.saturating_sub(t) <= window)
            || world.wanted_only_for_vagrancy(s)
            || world.claimed_by_other(EntityId::NONE, s)
        {
            continue;
        }
        let Some(home) = world.comp::<crate::components::Household>(s).and_then(|h| h.home) else { continue };
        if world.comp::<Position>(s).and_then(|p| p.building) != Some(home) {
            continue;
        }
        let Some(door) = world.comp::<Building>(home).map(|b| b.door) else { continue };
        let Some(i) = free
            .iter()
            .enumerate()
            .filter(|&(_, &(g, t))| {
                g != s
                    && t.manhattan(door) <= pursuit_radius_for(world, g)
                    && !crate::systems::contracts::on_take_of(world, g, s)
            })
            .min_by_key(|&(_, &(g, t))| (t.manhattan(door), g))
            .map(|(i, _)| i)
        else {
            continue;
        };
        let (g, gt) = free.swap_remove(i);
        if world.comp::<Brain>(s).is_some_and(|b| b.lod == crate::components::Lod::Statistical) {
            crate::systems::lod::set_lod(world, s, crate::components::Lod::Coarse);
        }
        world.set_last_seen(s, (door, now));
        world.handoffs.insert(g, (s, now));
        crate::systems::ownership::jobs2_row(world).law_door_taken += 1;
        let charge = wanted_for(world, s).map_or_else(String::new, |c| format!(" for {}", charge_label(world, s, c)));
        let text = format!(
            "{} took the warrant on {}{charge} to the door of {}, {} tiles off: home tonight, never seen on the street",
            world.name_of(g),
            world.name_of(s),
            world.name_of(home),
            gt.manhattan(door)
        );
        world.push_event(EventKind::Pursuit, &[g, s], text);
    }
}

/// `chase_dropped`'s hand-on: the nearest guard to `at` (the suspect's last
/// sighting; ties the lower id), not `from`, of the same force (a private
/// guard's only on his beat), at work on shift, free (no chase, no escort,
/// not a Statistical one, not on the suspect's take), within his pursuit
/// radius of `at` with the sighting fresh to him.
fn handoff_guard(world: &World, from: EntityId, s: EntityId, at: TilePos) -> Option<EntityId> {
    let private = is_private_guard(world, from);
    let tod = world.tick_of_day();
    let seen = world.last_seen().get(&s).copied()?;
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| g != from && g != s && is_private_guard(world, g) == private)
        .filter(|&g| living(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| {
            world.comp::<Brain>(g).is_some_and(|b| {
                b.lod != crate::components::Lod::Statistical
                    && b.cuffed_by.is_none()
                    && b.escorting.is_none()
                    && b.plan_goal() != Some(GoalKind::Arrest)
            })
        })
        .filter(|&g| {
            world.comp::<Job>(g).is_some_and(|j| {
                j.on_shift(tod) && crate::exec::routine::workday_of(world, g, j, j.shift_key_at(world.tick))
            }) && !on_leave(world, g)
        })
        .filter_map(|g| world.comp::<Position>(g).map(|p| (g, p.tile)))
        .filter(|&(g, t)| {
            t.manhattan(at) <= pursuit_radius_for(world, g)
                && crate::systems::life::sighting_fresh_at(world, seen, t)
                && (!private || on_beat_at(world, g, at))
                && !crate::systems::contracts::on_take_of(world, g, s)
        })
        .min_by_key(|&(g, t)| (t.manhattan(at), g))
        .map(|(g, _)| g)
}

/// Is `located_suspects(world, by)` non-empty?
pub fn any_located_suspect(world: &World, by: Option<Pursuer>) -> bool {
    let claimed = claim_set(world, by);
    located_candidates(world, by, &claimed).into_iter().any(|(s, t)| chaseable(world, s, t, by, &claimed))
}

/// The pre-S1 `located_suspects` (every open warrant), for the tests.
#[cfg(test)]
fn located_suspects_scan(world: &World, by: Option<Pursuer>) -> Vec<EntityId> {
    let claimed = claim_set(world, by);
    world
        .open_suspects()
        .filter(|&s| world.last_seen().get(&s).is_some_and(|&seen| chaseable(world, s, seen, by, &claimed)))
        .collect()
}

/// Is `s` a wanted, located suspect (anywhere)?
pub fn is_located_suspect(world: &World, s: EntityId) -> bool {
    wanted(world, s) && located(world, s)
}

/// The fine settling a fineable report: Vagrancy's, the dodger's (Transit
/// T10), else `fine_mult` meals.
fn fine_for(world: &World, crime: Crime) -> i64 {
    match crime {
        Crime::Vagrancy => world.config.law.vagrancy_fine,
        Crime::FareEvasion => world.config.transit.dodge_fine,
        _ => world.config.crime.fine_mult * world.mean_price(),
    }
}

/// A petty offence: a night in the cells (M12 D15: Vagrancy; Transit T10:
/// fare evasion; Jobs & lives phase 3, § 4: an unlicensed weapon; phase 4:
/// Solicitation), fined whenever the suspect can pay (Shadow V4 bug 4).
pub fn is_petty(crime: Crime) -> bool {
    matches!(crime, Crime::Vagrancy | Crime::FareEvasion | Crime::UnlicensedArms | Crime::Solicitation)
}

/// The worst open warrant on `suspect` (the one its sentence would be for).
pub fn wanted_for(world: &World, suspect: EntityId) -> Option<Crime> {
    world.crime_reports().iter().filter(|r| !r.resolved && r.suspect == suspect).map(|r| r.crime).max()
}

/// Pressure pass round 3, S3 (OPEN-223): the charge as a line reads it, the
/// crime and whom it was done to when the report's witness saw that
/// ("Assault on Thea Kemp"): an arrest of a sleeper read as the sleeper
/// attacked, with the warrant nowhere in his diary. The victim comes from
/// the newest report of `crime` on `suspect`, its witness's `SawCrime`
/// memory; none (an off-screen report, a memory gone): the crime alone.
pub fn charge_label(world: &World, suspect: EntityId, crime: Crime) -> String {
    let prep = match crime {
        Crime::Assault | Crime::Abduction => "on",
        Crime::Murder | Crime::Manslaughter => "of",
        Crime::Theft | Crime::GrandTheft | Crime::Extortion | Crime::Blackmail | Crime::Fraud => "from",
        _ => return crime.label().to_string(),
    };
    let victim = world
        .crime_reports()
        .iter()
        .filter(|r| r.suspect == suspect && r.crime == crime)
        .max_by_key(|r| (!r.resolved, r.tick))
        .and_then(|r| r.witness)
        .and_then(|w| world.comp::<crate::components::Memory>(w))
        .and_then(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(suspect) && e.crime == Some(crime))
                .max_by_key(|e| e.tick)
                .and_then(|e| e.object)
        })
        .filter(|&v| v != suspect && world.has::<crate::components::Identity>(v));
    match victim {
        Some(v) => format!("{} {prep} {}", crime.label(), world.name_of(v)),
        None => crime.label().to_string(),
    }
}

/// Sentence length in ticks for a crime at the current lever.
pub fn sentence_ticks(world: &World, crime: Crime) -> Tick {
    // M12 D15: a night in the cells, whatever the sentence lever.
    if is_petty(crime) {
        return world.config.law.vagrancy_sentence_ticks.max(1);
    }
    // M16a (plan C29): Conspiracy is `accessory_mult` (or the lever's) of Murder's.
    if crime == Crime::Conspiracy {
        let mult = world.levers.accessory_mult.unwrap_or(world.config.law.accessory_mult);
        let murder = sentence_ticks(world, Crime::Murder);
        return ((murder as f32 * mult).ceil() as Tick).max(TICKS_PER_DAY);
    }
    // M13 D47: the crimes appended after Vagrancy read `sentence_days_ext`.
    let ext = &world.config.crime.sentence_days_ext;
    let base = match crime {
        Crime::GrandTheft => ext.grand_theft,
        Crime::Manslaughter => ext.manslaughter,
        Crime::Abduction => ext.abduction,
        // Jobs & lives phase 5: Abduction's term.
        Crime::Trafficking => ext.abduction,
        // Jobs & lives phase 6: Fraud's and Dealing's terms.
        Crime::Forgery => ext.fraud,
        Crime::BlackMarket => ext.dealing,
        Crime::Dealing => ext.dealing,
        // M14 V18.
        // V42 `SetHackSentence` replaces the base.
        Crime::Intrusion => world.levers.hack_sentence_days[0].map_or(ext.intrusion, u32::from),
        Crime::DataTheft => world.levers.hack_sentence_days[1].map_or(ext.data_theft, u32::from),
        // M16b § 4.
        Crime::Fraud => ext.fraud,
        Crime::Blackmail => ext.blackmail,
        // Jobs & lives phase 3 (§ 4).
        Crime::ArmsDealing => ext.arms_dealing,
        Crime::Smuggling => ext.smuggling,
        // Pressure pass round 7, W5 (`OPEN-237`).
        Crime::CampEscape => ext.camp_escape,
        _ => world.config.crime.sentence_days[crime as usize],
    } as f32;
    let days = (base * world.levers.sentence_mult).ceil().max(1.0) as u64;
    days * TICKS_PER_DAY
}

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
fn booked(world: &mut World, guard: EntityId, suspect: EntityId, crime: Crime, jail: EntityId) {
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

/// M16a (plan C28): the arresting guard interrogates a taker (or crew) of
/// a Hit or Beat fulfilled within `interrogate_days`, once per record per
/// arrest: an Intimidate move (`moves::resolve_with`, the move's own
/// `WordNs::Move` draw) with the bonus `0.3 × Law.competence +
/// interrogate_bias`, the stake the buyer's name. A win puts `Hired` (actor
/// the buyer, object the target, hops 1, conf 1) in the guard and, for a
/// brokered record, a second naming the Fixer's owner; the Fixer warms by
/// `heat_per_named`. `contract_cleared` counts a Hit's taker jailed within
/// `interrogate_days` of the killing (once per record).
fn interrogate(world: &mut World, guard: EntityId, suspect: EntityId) {
    use crate::contract::{ContractKind, ContractStatus};
    let Some(ids) = world.by_party.get(&suspect).cloned() else { return };
    let now = world.tick;
    let window = Tick::from(world.config.law.interrogate_days) * TICKS_PER_DAY;
    for id in ids {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        let took = c.taker == Some(suspect) || c.crew.contains(&suspect);
        let done = matches!(c.status, ContractStatus::Fulfilled | ContractStatus::Reneged);
        if !took || !done || !c.kind.violent() {
            continue;
        }
        let Some(closed) = c.closed else { continue };
        if now.saturating_sub(closed) > window || c.interrogated.contains(&now) {
            continue;
        }
        // Review fix: once per record (the first arrest inside the window
        // marks `interrogated` below).
        if c.kind == ContractKind::Hit && c.interrogated.is_empty() {
            world.stats.current.contract.contract_cleared += 1;
        }
        if let Some(x) = world.contracts.get_mut(&id) {
            x.interrogated.push(now);
        }
        world.stats.current.contract.interrogations += 1;
        let Some(buyer) = c.buyer.or(c.agent) else { continue };
        let competence = world.law().map_or(0.0, |l| l.competence);
        let bonus = 0.3 * competence + world.config.law.interrogate_bias;
        let m = crate::word::SocialMove {
            actor: guard,
            target: suspect,
            kind: crate::word::MoveKind::Intimidate,
            stake: crate::word::Stake::Info { about: buyer },
        };
        let out = crate::systems::moves::resolve_with(world, &m, bonus);
        if !out.success || out.refused {
            continue;
        }
        world.stats.current.contract.interrogations_won += 1;
        let target = c.target.id();
        crate::systems::contracts::hear_named(world, id, guard, buyer, target, 1);
        if let Some(f) = c.broker {
            if let Some(owner) = world.owner_of(f).filter(|&o| world.has::<crate::components::Identity>(o)) {
                crate::systems::contracts::hear_named(world, id, guard, owner, target, 1);
            }
            let by = world.config.law.heat_per_named;
            crate::systems::contracts::heat_add(world, f, by);
        }
    }
}

/// M16a (plan C29), at midnight: for each Hit or Beat fulfilled within
/// `accessory_days` whose placing agent is living, free and not yet
/// charged, a city guard or a witness of the strike (a `SawCrime` of the
/// taker or crew within an hour of the killing) holding `Hired` about it at
/// `conf ≥ accessory_conf` (`contracts::holds_hired`: the memory, never
/// `known_by`) files `Crime::Conspiracy` against the placing agent (the
/// lowest holder id). The law's own records (buyer `None`, `Origin::Law`:
/// the death squad) are never charged, and the record's own side (its
/// taker, crew and the broker's owner) never files. `Accessory` with the
/// days since the killing.
pub fn accessory_check(world: &mut World) {
    use crate::contract::{ContractKind, ContractStatus};
    if world.contracts.is_empty() {
        return;
    }
    let now = world.tick;
    let window = Tick::from(world.config.law.accessory_days) * TICKS_PER_DAY;
    let cands: Vec<crate::contract::Contract> = world
        .contracts
        .values()
        .filter(|c| c.kind.violent() && matches!(c.status, ContractStatus::Fulfilled | ContractStatus::Reneged))
        // Review fix: the law does not prosecute its own records (the death
        // squad: buyer `None`, `Origin::Law`).
        .filter(|c| c.buyer.is_some() && c.origin != crate::contract::Origin::Law)
        .filter(|c| !c.charged && c.closed.is_some_and(|t| now.saturating_sub(t) <= window))
        .filter(|c| {
            c.agent.is_some_and(|a| {
                living(world, a)
                    && !world.has::<Sentence>(a)
                    && world.comp::<Brain>(a).is_some_and(|b| b.cuffed_by.is_none())
            })
        })
        .cloned()
        .collect();
    if cands.is_empty() {
        return;
    }
    let guards: BTreeSet<EntityId> = crate::systems::law_brain::guards(world).into_iter().collect();
    let broker_owner: BTreeMap<crate::contract::ContractId, EntityId> =
        cands.iter().filter_map(|c| c.broker.and_then(|f| world.owner_of(f)).map(|o| (c.id, o))).collect();
    let mut found: Vec<(crate::contract::ContractId, EntityId)> = Vec::new();
    // scan-ok: daily, the memory holders, only while a fulfilled record waits.
    for holder in world.with::<crate::components::Memory>() {
        if found.len() == cands.len() {
            break;
        }
        let Some(m) = world.comp::<crate::components::Memory>(holder) else { continue };
        let guard = guards.contains(&holder);
        for c in &cands {
            if found.iter().any(|&(id, _)| id == c.id) || Some(holder) == c.agent {
                continue;
            }
            // Review fix: the record's own side (its taker, a squad's crew,
            // the broker's owner) never files against its buyer.
            if Some(holder) == c.taker || c.crew.contains(&holder) || broker_owner.get(&c.id) == Some(&holder) {
                continue;
            }
            let witness = !guard && {
                let closed = c.closed.unwrap_or(0);
                m.entries.iter().any(|e| {
                    e.kind == MemoryKind::SawCrime
                        && e.subject.is_some_and(|s| Some(s) == c.taker || c.crew.contains(&s))
                        && e.tick.abs_diff(closed) <= 60
                })
            };
            if (guard || witness) && crate::systems::contracts::holds_hired(world, holder, c) {
                found.push((c.id, holder));
            }
        }
    }
    found.sort_unstable();
    for (id, holder) in found {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        let Some(agent) = c.agent else { continue };
        file_report(world, Crime::Conspiracy, agent, Some(holder));
        if let Some(x) = world.contracts.get_mut(&id) {
            x.charged = true;
        }
        world.stats.current.contract.accessory += 1;
        let days = now.saturating_sub(c.closed.unwrap_or(now)) / TICKS_PER_DAY;
        let what = if c.kind == ContractKind::Hit { "killing" } else { "beating" };
        let target = c.target.id();
        let text = format!(
            "{} charged with Conspiracy in the {what} of {} ({days} days after)",
            world.name_of(agent),
            world.name_of(target)
        );
        world.push_event(EventKind::Accessory, &[agent, target, holder], text);
    }
}

fn resolve_reports(world: &mut World, suspect: EntityId) {
    world.resolve_reports_of(suspect);
}

/// Put an agent in the Jail with a `Sentence`; a job survives a sentence of
/// three days or less.
pub fn sentence(world: &mut World, who: EntityId, crime: Crime, until: Tick, jail: EntityId) {
    // M12 D15: a Vagrancy sentence is counted where the vagrant was swept
    // (`World::vagrancy_places`, else where they stand), before the Jail.
    // Any other crime leaves the entry for the Vagrancy report it belongs to.
    let place = (crime == Crime::Vagrancy).then(|| vagrancy_place(world, who));
    // The pressure pass phase 5a: the run in the cells begins (the post held
    // going in, who reported him).
    crate::systems::consequence::on_sentenced(world, who, crime);
    // Whoever was escorting them is done; a cuffed prisoner is a contradiction.
    // Every escort, not only `cuffed_by`: when two guards had cuffed the same
    // suspect, the first to reach the Jail cleared `cuffed_by`, the second kept
    // escorting, and `follow_guard` walked the new prisoner out of the Jail
    // (M10 phase 5b: prisoners with a Sentence outside the Jail).
    let escorts: Vec<EntityId> = world
        .guards()
        .iter()
        .copied()
        .chain(world.comp::<Brain>(who).and_then(|b| b.cuffed_by))
        .filter(|&g| world.comp::<Brain>(g).is_some_and(|b| b.escorting == Some(who)))
        .collect();
    for g in escorts {
        if let Some(gb) = world.comp_mut::<Brain>(g) {
            gb.escorting = None;
        }
    }
    if let Some(b) = world.comp_mut::<Brain>(who) {
        b.cuffed_by = None;
    }
    world.abort_plan(who);
    world.leave_building(who);
    world.enter_building(who, jail);
    world.insert(who, Sentence { until_tick: until, crime });
    // Round 8 X3 (`OPEN-247`): a sentence of `record_min_days` is a record.
    if until.saturating_sub(world.tick) >= u64::from(world.config.careers.record_min_days) * TICKS_PER_DAY {
        crate::systems::misery::note_record(world, who);
    }
    world.remember(who, MemoryKind::WasArrested, None, 0.7, -0.5, false);
    if let Some(p) = world.comp_mut::<Personality>(who) {
        p.drift(Drift::Arrested);
    }
    if until.saturating_sub(world.tick) > 3 * TICKS_PER_DAY {
        world.vacate_job(who);
        // M12 D27: a long sentence loses the squat (and tonight's bed).
        world.remove::<crate::components::Squatter>(who);
        world.hotel_beds.remove(&who);
    }
    if let Some(b) = world.comp_mut::<Brain>(who) {
        b.current_goal = None;
        b.lod = crate::components::Lod::Coarse;
    }
    world.retier(who);
    let name = world.name_of(who);
    let days = until.saturating_sub(world.tick).div_ceil(TICKS_PER_DAY);
    let charge = charge_label(world, who, crime);
    world.push_event(EventKind::Sentence, &[who], format!("{name} sentenced to {days} days for {charge}"));
    if let Some(place) = place {
        crate::systems::street::note_vagrancy(world, place);
        let text = format!("{name} jailed for the night for vagrancy in {}", world.district_name(place));
        world.push_event(EventKind::Vagrancy, &[who], text);
    }
    crate::systems::gang::on_member_arrested(world, who);
}

/// Where a Vagrancy report against `who` is counted: the district it was
/// swept in (`World::vagrancy_places`, consumed), else where it stands.
fn vagrancy_place(world: &mut World, who: EntityId) -> crate::components::DistrictId {
    world.vagrancy_places.remove(&who).unwrap_or_else(|| {
        let tile = world.comp::<Position>(who).map(|p| p.tile).unwrap_or_default();
        world.district_of(tile)
    })
}

fn release_at_jail_door(world: &mut World, who: EntityId, _why: &str) {
    // The escort ended inside the Jail (or at its door): step out onto the street.
    world.leave_building(who);
}

/// Release a prisoner: `full` means the sentence was served to the end.
pub fn release(world: &mut World, who: EntityId, full: bool) {
    // L2 (L21): a held prisoner gets its body back inside, then walks out.
    if crate::systems::lod::is_held(world, who) {
        crate::systems::lod::set_lod(world, who, crate::components::Lod::Coarse);
    }
    // Lives at work phase 7: a night served in a Station's cells ends there.
    let from = if world
        .comp::<Position>(who)
        .and_then(|p| p.building)
        .is_some_and(|b| crate::systems::stations::is_station(world, b))
    {
        BuildingKind::Station.label()
    } else {
        "Precinct"
    };
    let Some(_) = world.remove::<Sentence>(who) else { return };
    world.leave_building(who);
    world.remember(who, MemoryKind::WasArrested, None, 0.7, -0.5, false);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        n.belonging = 0.2;
        n.safety = 0.5;
    }
    if full {
        if let Some(p) = world.comp_mut::<Personality>(who) {
            p.drift(Drift::ServedFullSentence);
        }
    }
    let name = world.name_of(who);
    world.push_event(EventKind::Release, &[who], format!("{name} released from the {from}"));
    // The pressure pass phase 5a: the life he comes out to.
    crate::systems::consequence::on_released(world, who);
    crate::systems::gang::on_member_released(world, who);
}

/// M9: a convict is broken out. The `Sentence` goes, they stand at the Jail
/// door hunted (`safety` 0.3) and glad of it (`Escaped`), and the warrant
/// **reopens** for the sentence's crime, so a re-arrest sentences them afresh.
pub fn escape(world: &mut World, who: EntityId) {
    // L2 (L21): a held prisoner broken out is promoted to Coarse first.
    if crate::systems::lod::is_held(world, who) {
        crate::systems::lod::set_lod(world, who, crate::components::Lod::Coarse);
    }
    let Some(s) = world.remove::<Sentence>(who) else { return };
    world.leave_building(who);
    world.remember(who, MemoryKind::Escaped, None, 0.7, 0.4, false);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        n.safety = 0.3;
    }
    file_report(world, s.crime, who, None);
    crate::systems::gang::on_member_released(world, who);
}

/// M9: a gang stormed the Jail. `won`: convicts were freed, which is the
/// shock that garrisons the Jail; `beaten` guards lost their fight.
pub fn on_breakout(world: &mut World, _gang: EntityId, won: bool, beaten: usize) {
    for _ in 0..beaten {
        crate::systems::law_brain::push_shock(world, LawShock::GuardBeaten);
    }
    if won {
        let now = world.tick;
        if let Some(l) = world.law_mut() {
            l.last_breakout_tick = Some(now);
        }
        crate::systems::law_brain::push_shock(world, LawShock::Jailbreak);
    }
}

/// M9: is every guard holding the Jail (`Posture::Garrison`)?
pub fn garrisoned(world: &World) -> bool {
    world.law().is_some_and(|l| l.posture == Posture::Garrison)
}

/// M9: does this guard hold the Jail on the shift `shift_key`? Patrol: the
/// split in two by roster rank; Crackdown: one in three; Garrison: everyone.
/// A Crackdown with nobody to crack down on is a Patrol. The rank is the
/// guard's place among the guards by entity index, so a small roster never
/// leaves the Jail empty: when no rank matches the shift, the first guard holds it.
pub fn jail_duty(world: &World, guard: EntityId, shift_key: i64) -> bool {
    // L2 shadow fixes item 10: the duty is fixed for a shift as its key
    // comes due (it flipped from Patrol to the desk at 13:13 and the guard
    // walked 100 min back).
    if let Some((k, v)) = world.comp::<Job>(guard).and_then(|j| j.duty_fixed) {
        if k == shift_key {
            return v;
        }
    }
    jail_duty_now(world, guard, shift_key)
}

/// `jail_duty` as the roster and posture read now.
fn jail_duty_now(world: &World, guard: EntityId, shift_key: i64) -> bool {
    // M11 D18: only the city's guards hold the Jail, Garrison included. The
    // roster is read off the role index without a copy: this runs per guard
    // per tick (`credit_guard_shifts`) and in every guard's think.
    use crate::systems::law_brain::is_city_guard;
    if !is_city_guard(world, guard) {
        return false;
    }
    let mut modulus: i64 = match world.law().map_or((Posture::Patrol, None), |l| (l.posture, l.target)) {
        (Posture::Garrison, _) => return true,
        (Posture::Crackdown, Some(_)) => 3,
        _ => 2,
    };
    // Lives at work phase 7 (§ 5.2): with Stations standing, each post's
    // roster is its own (a Station's desk is `station_desk_share` of its
    // guards a shift; the Precinct's guards rank among themselves).
    let post = crate::systems::stations::station_post(world, guard);
    let stations = !world.buildings_of_kind(BuildingKind::Station).is_empty();
    if post.is_some() {
        let share = world.config.law.station_desk_share.max(f32::EPSILON);
        modulus = ((1.0 / share).round() as i64).max(1);
    }
    let same_post = |g: EntityId| {
        is_city_guard(world, g) && (!stations || crate::systems::stations::station_post(world, g) == post)
    };
    let all = world.guards();
    let rank = all.iter().take_while(|&&g| g.index < guard.index).filter(|&&g| same_post(g)).count() as i64;
    let slot = shift_key.rem_euclid(modulus);
    if rank % modulus == slot {
        return true;
    }
    rank == 0 && (all.iter().filter(|&&g| same_post(g)).count() as i64) <= slot
}

/// Per tick: guards perceive wanted suspects; escorted suspects follow.
/// Daily: warrants expire, prisoners are fed and released.
pub fn run(world: &mut World) {
    crate::prof::timed!("law::tally_watch", tally_watch(world));
    crate::prof::timed!("law::credit_guard_shifts", credit_guard_shifts(world));
    crate::prof::timed!("law::sightings", sightings(world));
    // Round 6 V3 (`OPEN-238`): bodies found, the case opened; victims who
    // tell the beat.
    crate::prof::timed!("law::find_bodies", crate::systems::demography::find_bodies(world));
    // Round 7 W2 (`OPEN-238`): a sleeper robbed in his sleep wakes to it.
    if world.tick.is_multiple_of(world.config.crime.case.find_every_ticks.max(1)) {
        crate::systems::consequence::woke(world);
    }
    if world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        crate::prof::timed!("law::tell_the_beat", tell_the_beat(world));
        // Round 8 X1 (`OPEN-246`): the promises to tell the law, kept or dropped.
        crate::prof::timed!("law::keep_word", keep_word(world));
        // Round 7 W3 (`OPEN-244`): the warrant the street never saw, at the door.
        crate::prof::timed!("law::serve_at_door", serve_at_door(world));
    }
    // The island pass phase 7: the transfer vans at the Precinct.
    crate::prof::timed!("law::stations::van_tick", crate::systems::stations::van_tick(world));
    // L2 (L21): the held prisoners' needs decay hour by hour, as a body's
    // do through the day (a day's decay at once left them fed and rested
    // all day, and their mood lifted their districts' unrest off the riot
    // line: riots on 42-47 read 0.17 a seed against 2.67 with the budget off).
    if world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        crate::prof::timed!("law::held_hourly", held_hourly(world));
        // Round 8 X6 (`OPEN-217`): the cells' meal at its hour.
        crate::prof::timed!("law::jail_meals", jail_meals(world));
        // Lives at work phase 7 (§ 5.2): the van, at `transfer_hour`.
        crate::prof::timed!("law::stations::van", crate::systems::stations::van(world));
    }
    if crate::systems::midnight::due(world, crate::systems::midnight::Mid::Law) {
        crate::prof::timed!("law::expire_warrants", expire_warrants(world));
        // Round 7 W3: the warrants standing (a print).
        let open = world.open_suspects().count() as u32;
        crate::systems::ownership::jobs2_row(world).law_open_warrants = open;
        // L2 (L21): the held prisoners' day (meetings, the pitch), before their meal.
        crate::prof::timed!("law::held_daily", held_daily(world));
        crate::prof::timed!("law::jail_upkeep", jail_upkeep(world));
        crate::prof::timed!("law::reconcile_guards", reconcile_guards(world));
        // M12 D23: the city's sweepers, toward `levers.sanitation_count`.
        crate::prof::timed!(
            "law::districts::reconcile_sanitation",
            crate::systems::districts::reconcile_sanitation(world)
        );
        // M16a (plan C29): after `word::run`'s pass.
        crate::prof::timed!("law::accessory_check", accessory_check(world));
    }
    // The captain's daily rescoring, after the guard roster is reconciled.
    crate::prof::timed!("law::law_brain::run", crate::systems::law_brain::run(world));
    crate::prof::timed!("law::releases", releases(world));
}

/// M10 D33, M12 D3: one tick of watch per on-shift, unjailed guard, in the
/// district it stands in (`bind::district_coverage` reads yesterday's
/// totals; `bind::zone_law_coverage` sums them by zone). `World::zone_watch`
/// is kept for old saves and no longer written.
fn tally_watch(world: &mut World) {
    let tod = world.tick_of_day();
    for i in 0..world.guards().len() {
        let g = world.guards()[i];
        if world.has::<Sentence>(g) || !world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)) {
            continue;
        }
        let Some(tile) = world.comp::<Position>(g).map(|p| p.tile) else { continue };
        let d = world.district_of(tile).index().min(crate::components::MAX_DISTRICTS - 1);
        world.district_watch.today[d] += 1;
    }
    if world.tick.is_multiple_of(crate::time::TICKS_PER_HOUR) {
        tally_home_watch(world, tod);
    }
}

/// M11 D34: on the hour, one guard-hour to every standing Home whose door is
/// within `[classes] fear_radius` (Chebyshev) of an on-shift, unjailed guard.
/// O(guards x Homes) once an hour.
pub fn tally_home_watch(world: &mut World, tod: u16) {
    let r = i32::try_from(world.config.classes.fear_radius).unwrap_or(i32::MAX);
    let tiles: Vec<crate::components::TilePos> = world
        .guards()
        .iter()
        .filter(|&&g| !world.has::<Sentence>(g) && world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)))
        .filter_map(|&g| world.comp::<Position>(g).map(|p| p.tile))
        .collect();
    if tiles.is_empty() {
        return;
    }
    let doors: Vec<(EntityId, crate::components::TilePos)> = world
        .buildings_of_kind(crate::components::BuildingKind::Home)
        .iter()
        .filter_map(|&h| world.comp::<crate::components::Building>(h).filter(|b| !b.demolished).map(|b| (h, b.door)))
        .collect();
    for t in tiles {
        for &(h, d) in &doors {
            let dx = (i32::from(d.x) - i32::from(t.x)).abs();
            let dy = (i32::from(d.y) - i32::from(t.y)).abs();
            if dx.max(dy) <= r {
                let e = world.home_watch.today.entry(h).or_insert(0);
                *e = e.saturating_add(1);
            }
        }
    }
}

/// M10 5c: a guard's day of wages is owed here, and only here, by the shift
/// clock: each on-shift tick on law duty (Patrol, Arrest, or Work on a Jail
/// day) counts, and when the shift ends the day is owed if it was completed
/// (five patrol legs, a held Jail) or on duty for `[law] shift_duty_share` of
/// it, whatever plan was running at the end. Before, a Patrol day was owed
/// only by a PatrolLeg finishing at or after the end (whose precondition needs
/// the shift still running) or by an arrest or delivery: on seed 42 at 2,000,
/// 412 of 584 workday shifts in 20 days went unpaid, most of them on duty
/// for most of the shift, and the watch starved on full pay. A segment end
/// that a 0-start segment continues (night guards) is not an end, so each
/// shift is judged once.
fn credit_guard_shifts(world: &mut World) {
    use crate::components::GoalKind;
    let tod = world.tick_of_day();
    let last = if tod == 0 { 1439 } else { tod - 1 };
    let ended_tick = world.tick.saturating_sub(1);
    let share = world.config.law.shift_duty_share;
    // A copy: `maybe_quit` can take a guard off the roster mid-loop.
    let guards = world.guards().to_vec();
    for g in guards {
        // L2 shadow fixes item 10: fix the coming shift's duty once.
        let key = world.comp::<Job>(g).map(|j| j.next_shift_key(world.tick));
        let stale = world.comp::<Job>(g).is_some_and(|j| j.duty_fixed.map(|d| d.0) != key);
        if let (true, Some(key)) = (stale, key) {
            let v = jail_duty_now(world, g, key);
            if let Some(j) = world.comp_mut::<Job>(g) {
                j.duty_fixed = Some((key, v));
            }
        }
        let Some(job) = world.comp::<Job>(g) else { continue };
        let (on_now, ended) = (job.on_shift(tod), job.on_shift(last) && !job.on_shift(tod));
        if on_now {
            let key = job.shift_key_at(world.tick);
            let on_duty = !world.has::<Sentence>(g)
                && job.struck_shift != Some(key)
                && match world.comp::<Brain>(g).and_then(|b| b.current_goal) {
                    Some(GoalKind::Patrol | GoalKind::Arrest) => true,
                    Some(GoalKind::Work) => jail_duty(world, g, key),
                    _ => false,
                };
            if on_duty {
                if let Some(j) = world.comp_mut::<Job>(g) {
                    j.duty_ticks = j.duty_ticks.saturating_add(1);
                }
            }
            continue;
        }
        if !ended {
            continue;
        }
        let key = job.shift_key_at(ended_tick);
        let length: u32 = job.shifts.iter().map(|&(s, e)| u32::from(e.saturating_sub(s))).sum();
        let worked = job.last_shift_day == Some(key) || f32::from(job.duty_ticks) >= share * length as f32;
        // A struck shift (M11 D35) is marked worked but owes nothing.
        let struck = job.struck_shift == Some(key);
        let owed =
            worked && !struck && crate::exec::routine::workday_of(world, g, job, key) && !world.has::<Sentence>(g);
        if let Some(j) = world.comp_mut::<Job>(g) {
            j.duty_ticks = 0;
            if owed {
                j.last_shift_day = Some(key);
                j.days_unpaid = j.days_unpaid.saturating_add(1);
            }
        }
        if let Some(b) = world.comp_mut::<Brain>(g) {
            b.patrol_legs = 0;
            b.patrol_route.clear();
        }
        if owed {
            // L1: the shift's wage at its end, not on a walk to the Hall.
            crate::systems::economy::collect_wage(world, g);
        }
    }
}

/// The `guard_count` lever, reconciled daily with at most five changes (Scale
/// S4: five per 2,000 residents, `Config::per_capita_count`):
/// hire unemployed, unjailed adults of lawfulness >= 0.4 (highest first);
/// fire the guard with the lowest loyalty.
pub fn reconcile_guards(world: &mut World) {
    // M17 phase 3: the State's funded guards on top (0 with the outside off).
    let want = usize::from(world.levers.guard_count) + usize::from(world.outside.state_guards);
    // M11 D18: the city's payroll only; private guards are the corps'.
    let guards: Vec<EntityId> = crate::systems::law_brain::guards(world);
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return };
    // Lives at work phase 7 (§ 5.2): with Stations, each hire's post is the
    // nearest his Home of the Stations under their share and the Precinct.
    let mut posts = if guards.len() < want { crate::systems::stations::Posts::new(world, want) } else { None };
    if guards.len() < want {
        let candidates: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = world
            // scan-ok: daily: reconcile_guards
            .citizens()
            .into_iter()
            .filter(|&id| world.has::<Brain>(id) && !world.has::<Job>(id) && !world.has::<Sentence>(id))
            .filter(|&id| crate::systems::demography::is_adult(world, id))
            // Pressure pass F3: the idle days and the reputation (`misery::pool_out`).
            .filter(|&id| !crate::systems::misery::pool_out(world, id))
            // Lives at work phase 6 (§ 4.1, § 4.4): the Guard's band, the retired.
            .filter(|&id| crate::systems::careers::age_ok(world, id, Role::Guard))
            // Round 8 X3 (`OPEN-247`): the law's post reads a record.
            .filter(|&id| crate::systems::careers::record_ok(world, id, Role::Guard))
            .filter_map(|id| world.comp::<Personality>(id).map(|p| (ordered_float::OrderedFloat(p.lawfulness), id)))
            .filter(|&(l, _)| l.0 >= 0.4)
            .collect();
        // The pressure pass F1: those within `[distance] job_cap_ticks` of a
        // city post (a Station or the Precinct) first, the most lawful first
        // among them, then the rest (the base took the most lawful of the
        // island: Sump guards posted on Civic Core). Off: the base order.
        let doors: Vec<crate::components::TilePos> = posts
            .as_ref()
            .map(|p| p.posts.iter().map(|&(_, d, _)| d).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .chain(world.comp::<crate::components::Building>(jail).map(|b| b.door))
            .collect();
        let far = |id: EntityId| match (
            crate::systems::distance::job_cap(world),
            crate::systems::distance::set_out(world, id),
        ) {
            (Some(cap), Some(from)) => doors.iter().all(|&d| crate::systems::distance::travel(world, from, d) > cap),
            _ => false,
        };
        let mut candidates: Vec<(bool, ordered_float::OrderedFloat<f32>, EntityId)> =
            candidates.into_iter().map(|(l, id)| (far(id), l, id)).collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        for (_, _, id) in candidates.into_iter().take((want - guards.len()).min(world.config.per_capita_count(5))) {
            let post = posts.as_mut().and_then(|p| p.pick(world, id)).unwrap_or(jail);
            crate::systems::demography::hire(world, id, post, Role::Guard);
            // A dead guard's vacancy is this hire, not a second one (at the
            // post, else at any other city post).
            let at = std::iter::once(post)
                .chain(std::iter::once(jail))
                .chain(crate::systems::stations::stations(world))
                .find(|b| world.vacancies.get(b).is_some_and(|v| v.contains(&Role::Guard)));
            if let Some(at) = at {
                if let Some(v) = world.vacancies.get_mut(&at) {
                    if let Some(i) = v.iter().position(|&r| r == Role::Guard) {
                        v.remove(i);
                    }
                    if v.is_empty() {
                        world.vacancies.remove(&at);
                    }
                }
            }
        }
    } else if guards.len() > want {
        let mut by_loyalty: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = guards
            .iter()
            .filter_map(|&g| world.comp::<Personality>(g).map(|p| (ordered_float::OrderedFloat(p.loyalty), g)))
            .collect();
        by_loyalty.sort();
        for (_, g) in by_loyalty.into_iter().take((guards.len() - want).min(world.config.per_capita_count(5))) {
            let text = format!("{} dismissed from the guard", world.name_of(g));
            crate::systems::economy::dismiss(world, g, None, text);
        }
    }
    crate::systems::law_brain::recompute_captain(world);
}

/// Any guard perceiving (SIGHT, or same building) a wanted suspect records it.
///
/// Scale S1: the guards are bucketed once a tick (a [`TileGrid`] by tile, a
/// map by building), so each suspect reads only the guards in its square
/// and its building, not every guard in the city: the same pairs, the same
/// `seen`, the same seers in ascending order (`sightings_scan` is the old
/// loop, kept as the test's reference).
fn sightings(world: &mut World) {
    sightings_with(world, SIGHTINGS_GRID_PAIRS);
}

/// [`sightings`], with the grid past `grid_pairs` guard-suspect pairs.
fn sightings_with(world: &mut World, grid_pairs: usize) {
    let open = world.open_suspects().count();
    if open == 0 {
        return;
    }
    let sight = world.config.crime.sight;
    // M13 D32: a guard with Eyes sees `Kit.sight` tiles further.
    let guards = guard_sights(world);
    let max_extra = guards.iter().map(|g| g.extra).max().unwrap_or(0);
    // A small city (a few thousand guard-suspect pairs) reads every guard
    // for each suspect: the grid's upkeep would cost more than it saves.
    let grid = (open * guards.len() > grid_pairs).then(|| {
        let grid = crate::spatial::TileGrid::build(
            world.map.w(),
            world.map.h(),
            sight + max_extra + 1,
            guards.iter().map(|g| g.tile),
        );
        let mut in_building: BTreeMap<EntityId, Vec<u32>> = BTreeMap::new();
        for (i, g) in guards.iter().enumerate() {
            if let Some(b) = g.building {
                in_building.entry(b).or_default().push(i as u32);
            }
        }
        (grid, in_building)
    });
    // With the grid, only a suspect with a guard in its square's cells or its
    // building can be seen (positions do not move in this pass): the rest
    // are passed over before their stores are read.
    let suspects: Vec<EntityId> = world
        .open_suspects()
        .filter(|&s| {
            grid.as_ref().is_none_or(|(grid, in_building)| {
                world.comp::<Position>(s).is_some_and(|p| {
                    grid.any_near(p.tile, sight + max_extra) || p.building.is_some_and(|b| in_building.contains_key(&b))
                })
            })
        })
        .filter(|&s| (living(world, s)) && !world.has::<Sentence>(s))
        .collect();
    let tick = world.tick;
    // M15 W12 (plan deviation: hourly, not every tick): the guards who see a
    // suspect hold a Sighting of them.
    let note = tick.is_multiple_of(crate::time::TICKS_PER_HOUR);
    let mut near: Vec<u32> = Vec::new();
    for s in suspects {
        let Some(ps) = world.comp::<Position>(s) else { continue };
        let (tile, building) = (ps.tile, ps.building);
        // Round 5 U4 (`OPEN-236`): the walls hold (`sees_scene`).
        let scene_out = outdoors(world, tile, building);
        let sees = |g: &GuardSight| {
            (g.building.is_some() && g.building == building)
                || (scene_out && g.outside && chebyshev(g.tile, tile) <= sight + g.extra)
        };
        // The guards who see `s`, ascending (`guards` is ascending by id, so
        // ascending indices are): from the grid's cells and the building, or
        // without the grid, only whether any does until a note needs them.
        near.clear();
        let seen = match &grid {
            Some((grid, in_building)) => {
                grid.near(tile, sight + max_extra, &mut near);
                if let Some(list) = building.and_then(|b| in_building.get(&b)) {
                    near.extend_from_slice(list);
                }
                near.sort_unstable();
                near.dedup();
                near.retain(|&i| sees(&guards[i as usize]));
                !near.is_empty()
            }
            None => guards.iter().any(sees),
        };
        if !seen {
            continue;
        }
        // Jobs & lives phase 6 (§ 7): papers bought while wanted pass the
        // hour's sighting on `id_w × tier` (nothing off: the map is empty).
        if !world.jobs2.gray.papers.is_empty() && crate::systems::gray::passes_sighting(world, s) {
            continue;
        }
        world.set_last_seen(s, (tile, tick));
        if note {
            if grid.is_none() {
                near.extend((0..guards.len() as u32).filter(|&i| sees(&guards[i as usize])));
            }
            let seers: Vec<EntityId> = near.iter().map(|&i| guards[i as usize].id).collect();
            for g in seers {
                crate::systems::gossip::maybe_sight(world, g, s, building, tile);
            }
        }
    }
}

/// Scale S1: past this many guard-suspect pairs a tick, `sightings` buckets
/// the guards by tile (the 2,000 city reads ~2,300 a tick over 30 days on
/// seed 42, the 20,000 probe city hundreds of thousands).
const SIGHTINGS_GRID_PAIRS: usize = 16_384;

/// A guard as `sightings` reads it: where, inside what, whether in the open
/// (Round 5 U4, `law::outdoors`), and its Eyes' reach.
#[derive(Clone, Copy, Debug)]
struct GuardSight {
    id: EntityId,
    tile: TilePos,
    building: Option<EntityId>,
    outside: bool,
    extra: u32,
}

/// Every guard with a Position, ascending by id.
fn guard_sights(world: &World) -> Vec<GuardSight> {
    world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| GuardSight {
                id: g,
                tile: p.tile,
                building: p.building,
                outside: outdoors(world, p.tile, p.building),
                extra,
            })
        })
        .collect()
}

/// The pre-S1 `sightings` (every suspect against every guard), kept for
/// `test_sightings_grid_matches_the_scan`.
#[cfg(test)]
fn sightings_scan(world: &mut World) {
    let suspects: Vec<EntityId> =
        world.open_suspects().filter(|&s| (living(world, s)) && !world.has::<Sentence>(s)).collect();
    if suspects.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    let guards: Vec<(Position, u32)> = world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| (p.clone(), extra))
        })
        .collect();
    let tick = world.tick;
    let note = tick.is_multiple_of(crate::time::TICKS_PER_HOUR);
    let ids: Vec<EntityId> = if note { world.guards().to_vec() } else { Vec::new() };
    for s in suspects {
        let Some(ps) = world.comp::<Position>(s) else { continue };
        let (tile, building) = (ps.tile, ps.building);
        let out = outdoors(world, tile, building);
        let seen = guards.iter().any(|(p, extra)| sees_scene(world, p, tile, building, out, sight + extra));
        if seen {
            world.set_last_seen(s, (tile, tick));
            if note {
                let seers: Vec<EntityId> = ids
                    .iter()
                    .copied()
                    .filter(|&g| {
                        let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
                        world
                            .comp::<Position>(g)
                            .is_some_and(|p| sees_scene(world, p, tile, building, out, sight + extra))
                    })
                    .collect();
                for g in seers {
                    crate::systems::gossip::maybe_sight(world, g, s, building, tile);
                }
            }
        }
    }
}

fn expire_warrants(world: &mut World) {
    let expiry = world.config.crime.warrant_expiry_days * TICKS_PER_DAY;
    let tick = world.tick;
    let mut expired = Vec::new();
    for r in world.reports_mut().iter_mut().filter(|r| !r.resolved && tick.saturating_sub(r.tick) >= expiry) {
        r.resolved = true;
        expired.push(r.suspect);
    }
    for s in expired {
        if let Some(p) = world.comp_mut::<Personality>(s) {
            p.drift(Drift::CrimeUnpunished);
        }
        let name = world.name_of(s);
        world.push_event(EventKind::Unpunished, &[s], format!("warrant on {name} expired"));
    }
    world.reports_mut().retain(|r| !r.resolved || tick.saturating_sub(r.tick) < 2 * expiry);
}

/// Prisoners get one meal a day from the Market nearest the Jail, paid by the
/// Treasury at the city's mean price to the Market's owner (M11 D46; the
/// coins used to vanish), and a full night's sleep. Pressure pass round 3,
/// S3 (`[justice]`): the meal from the first of the nearest
/// `kitchen_markets` Markets with food (a bare kitchen is a `Cells` line for
/// a body on screen), and the night's sleep is the off-screen hold's alone
/// (a body sleeps at lights-out, `in_lights_out`).
fn jail_upkeep(world: &mut World) {
    let lights = world.config.justice.is_some();
    // Round 8 X6 (`OPEN-217`): the meal at `[justice] meal_hour` (`jail_meals`);
    // 0 (or no `[justice]`) at midnight, as before.
    if world.config.justice.as_ref().is_none_or(|c| c.meal_hour == 0) {
        serve_meals(world, false);
    }
    for who in world.with::<Sentence>() {
        if lights && !crate::systems::lod::is_held(world, who) {
            continue;
        }
        if let Some(n) = world.comp_mut::<Needs>(who) {
            n.energy = 1.0;
        }
    }
}

/// Round 8 X6 (`OPEN-217`), hourly: the cells' meal at `[justice] meal_hour`
/// (not 0), every prisoner's line its own ("ate the cells' meal"; V16's jail
/// days had no line at all).
fn jail_meals(world: &mut World) {
    let Some(h) = world.config.justice.as_ref().map(|c| c.meal_hour).filter(|&h| h > 0 && h < 24) else { return };
    if world.tick_of_day() / crate::time::TICKS_PER_HOUR as u16 != u16::from(h) {
        return;
    }
    serve_meals(world, true);
}

/// The cells' meal for every prisoner (`jail_meal`); `lined`: each fed
/// prisoner's line too (the kitchen it came from).
fn serve_meals(world: &mut World, lined: bool) {
    let price = world.mean_price();
    let kitchens = jail_kitchens(world);
    let cfg = world.config.needs.clone();
    let lights = world.config.justice.is_some();
    // Lives at work phase 3 (§ 2.2, jailer): the meal and the count are the
    // jailer's: a fed prisoner is an `Inmate` encounter (civic only).
    let jail = world.building_of_kind(BuildingKind::Jail).filter(|_| crate::systems::service::civic_on(world));
    for who in world.with::<Sentence>() {
        if let Some(market) = jail_meal(world, who, &kitchens, price, &cfg) {
            // Lives at work phase 7: a Station's held are fed, but the
            // Precinct's jailer serves only the Precinct's.
            let cell = world.comp::<Position>(who).and_then(|p| p.building);
            let in_station = cell.is_some_and(|b| crate::systems::stations::is_station(world, b));
            if let Some(j) = jail.filter(|_| !in_station) {
                crate::systems::service::arrive(world, who, j, crate::components::ServeKind::Inmate, 0);
            }
            if lined {
                let at = cell.map_or_else(|| "the cells".to_string(), |b| format!("the cells of {}", world.name_of(b)));
                let text = format!("{} ate the meal in {at} (food from {})", world.name_of(who), world.name_of(market));
                let mut actors = vec![who];
                actors.extend(cell);
                world.push_event(EventKind::Cells, &actors, text);
                crate::systems::ownership::jobs2_row(world).cell_meals += 1;
            }
        } else if lights && (lined || !crate::systems::lod::is_held(world, who)) {
            let text = format!("{} went without: the cells' kitchen had no food", world.name_of(who));
            world.push_event(EventKind::Cells, &[who], text);
        }
    }
}

/// Pressure pass round 3, S3: is it lights-out in the cells (`[justice]`
/// `lights_out_hour`..`wake_hour`)? A sentenced body sleeps then.
pub fn in_lights_out(world: &World) -> bool {
    let Some(c) = world.config.justice.as_ref() else { return false };
    let h = (u64::from(world.tick_of_day()) / crate::time::TICKS_PER_HOUR) as u8;
    if c.lights_out_hour <= c.wake_hour {
        h >= c.lights_out_hour && h < c.wake_hour
    } else {
        h >= c.lights_out_hour || h < c.wake_hour
    }
}

/// S3: is `id` a prisoner asleep in the cells now (sentenced, inside, at
/// lights-out)? Read by `needs::run` (the bed's restore) and the shadow tool.
pub fn asleep_in_cell(world: &World, id: EntityId) -> bool {
    in_lights_out(world)
        && world.has::<Sentence>(id)
        && world.comp::<Position>(id).is_some_and(|p| p.building.is_some())
}

/// The cells' kitchens: the Market nearest the Jail's door; with
/// `[justice]`, its nearest `kitchen_markets`, nearest first.
fn jail_kitchens(world: &World) -> Vec<EntityId> {
    let Some(door) = world.building_of_kind(BuildingKind::Jail).and_then(|j| world.comp::<Building>(j).map(|b| b.door))
    else {
        return Vec::new();
    };
    let Some(n) = world.config.justice.as_ref().map(|c| c.kitchen_markets.max(1)) else {
        return world.nearest_of_kind(BuildingKind::Market, door).into_iter().collect();
    };
    let mut all: Vec<(u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .filter_map(|&b| {
            world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| (bd.door.manhattan(door), b))
        })
        .collect();
    all.sort_unstable();
    all.into_iter().take(n).map(|(_, b)| b).collect()
}

/// One Jail meal for a prisoner from the first of `kitchens` with food on
/// the shelf, the Treasury paying (`Flow::JailFood`): the Market it came
/// from. `None` when every shelf is empty.
fn jail_meal(
    world: &mut World,
    who: EntityId,
    kitchens: &[EntityId],
    price: i64,
    cfg: &crate::config::NeedsCfg,
) -> Option<EntityId> {
    let market = kitchens.iter().copied().find(|&m| world.comp::<Building>(m).is_some_and(|b| b.stock_food > 0))?;
    if let Some(b) = world.comp_mut::<Building>(market) {
        b.stock_food -= 1;
    }
    if crate::systems::lod::is_held(world, who) {
        world.stats.current.budget.held_fed += 1;
    }
    let owner = world.owner_of(market);
    let paid = crate::systems::ownership::charge(world, None, owner, price, crate::systems::ownership::Flow::JailFood);
    crate::systems::ownership::credit(world, market, paid);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        crate::needs::eat(n, cfg);
    }
    world.mark_day(who, crate::components::trace_flags::ATE);
    Some(market)
}

/// L2 (L21): the held prisoners' day, at midnight before `jail_upkeep`
/// feeds them: the decay not yet settled and a starvation check. By intent
/// both also run every hour (`held_hourly`, as the Statistical tier's hourly
/// loop: `needs::starvation` kills on `starving_since` plus the grace and
/// remembers once a day), so this pass settles at most the midnight tick.
/// Then, for each held prisoner who arrived in the last day,
/// `jail_meet_max` first meetings among the held (and as many gang members
/// with `jail_pitch`), drawn on `WordNs::Held` keyed `(day, id.index)`
/// (`stat_meet`'s first meeting: a bare edge, `MetInJail` both ways); and
/// the cells' pitch: each held member drifts with every other held
/// prisoner by a day of the Jail's co-location drift.
pub fn held_daily(world: &mut World) {
    use crate::systems::{lod, social};
    if !lod::held_on(world) {
        return;
    }
    let held: Vec<EntityId> = world.sentenced().iter().copied().filter(|&p| lod::is_held(world, p)).collect();
    // Entries for prisoners no longer held (dead, released) go.
    world.held_since.retain(|p, _| held.binary_search(p).is_ok());
    if held.is_empty() {
        return;
    }
    for &id in &held {
        settle_held(world, id);
        crate::needs::starvation(world, id);
    }
    let held: Vec<EntityId> = held.into_iter().filter(|&p| living(world, p)).collect();
    let now = world.tick;
    let today = world.day();
    let cap = world.config.life.jail_meet_max;
    let pitch = world.config.life.jail_pitch;
    let member = |w: &World, x: EntityId| pitch && w.has::<crate::components::GangMember>(x);
    let arrivals: Vec<EntityId> = held
        .iter()
        .copied()
        .filter(|&p| world.comp::<Position>(p).is_some_and(|pos| pos.entered + TICKS_PER_DAY >= now))
        // One arrival, one round of meetings: a prisoner who met the cells
        // as a body (`social::colocation`'s arrival) meets nobody again.
        .filter(|&p| {
            let since = world.comp::<Position>(p).map_or(now, |pos| pos.entered);
            !world
                .comp::<crate::components::Memory>(p)
                .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::MetInJail && e.tick >= since))
        })
        .collect();
    for a in arrivals {
        let mut rng = world.rng.word(crate::word::WordNs::Held, today, u64::from(a.index));
        let mut strangers: Vec<(bool, u64, EntityId)> = held
            .iter()
            .copied()
            .filter(|&b| b != a && world.edge(a, b).is_none())
            .map(|b| (member(world, b), rng.random::<u64>(), b))
            .collect();
        strangers.sort_unstable();
        let (mut plain, mut gang) = (0usize, 0usize);
        for (m, _, b) in strangers {
            let n = if m { &mut gang } else { &mut plain };
            if *n >= cap {
                continue;
            }
            *n += 1;
            let sa = world.comp::<Personality>(a).map_or(0.5, |p| p.sociability);
            let sb = world.comp::<Personality>(b).map_or(0.5, |p| p.sociability);
            let (u1, u2): (f32, f32) = (rng.random(), rng.random());
            social::first_meeting(world, a, b, social::first_affinity(sa, sb, u1, u2));
            world.remember(a, MemoryKind::MetInJail, Some(b), 0.5, 0.0, false);
            world.remember(b, MemoryKind::MetInJail, Some(a), 0.5, 0.0, false);
        }
    }
    if pitch {
        let hours = 24.0;
        let members: Vec<EntityId> = held.iter().copied().filter(|&m| member(world, m)).collect();
        for &m in &members {
            for &p in &held {
                // Each pair once: a member with a non-member, or the lower id of two members.
                if p == m || (member(world, p) && p < m) {
                    continue;
                }
                // Pressure pass round 3, S3 (OPEN-223): the met only (see
                // `social::colocation`'s jail drift): a member's day drifted
                // him with the whole hold, an edge with each.
                if world.edge(m, p).is_none() {
                    continue;
                }
                let step = world.config.social.affinity_per_hour * social::similarity_mult(world, m, p);
                social::adjust(world, m, p, hours * step, hours * 0.02);
            }
        }
    }
}

/// L2 (L21): hourly, every held prisoner's jailed decay since its last
/// settlement and the starvation check, hourly by intent (the Statistical
/// tier's hourly step; ~150 prisoners an hour).
fn held_hourly(world: &mut World) {
    if !crate::systems::lod::held_on(world) {
        return;
    }
    let held: Vec<EntityId> =
        world.sentenced().iter().copied().filter(|&p| crate::systems::lod::is_held(world, p)).collect();
    let mut kitchen: Option<(Vec<EntityId>, i64, crate::config::NeedsCfg)> = None;
    for id in held {
        settle_held(world, id);
        // L2 phase 5: a held prisoner who is starving (an arrest of the
        // starving: thieves of food arrive at hunger 0) gets the cell's
        // meal now, not at midnight: on 42-47 one or two a run died within
        // the hour of arrival (the gate's "no held prisoner starves").
        if world.comp::<Needs>(id).is_some_and(|n| n.starving_since.is_some()) {
            let (kitchens, price, cfg) = kitchen
                .get_or_insert_with(|| (jail_kitchens(world), world.mean_price(), world.config.needs.clone()))
                .clone();
            jail_meal(world, id, &kitchens, price, &cfg);
        }
        crate::needs::starvation(world, id);
    }
    // Pressure pass round 3, S3: a prisoner on screen (a pinned body in the
    // cells) starving gets the same meal now (the hold's rule was the held's
    // alone: `jailed_75054` starved from d26 at Full).
    if world.config.justice.is_some() {
        let bodies: Vec<EntityId> = world
            .sentenced()
            .iter()
            .copied()
            .filter(|&p| !crate::systems::lod::is_held(world, p))
            .filter(|&p| world.comp::<Needs>(p).is_some_and(|n| n.starving_since.is_some()))
            .collect();
        for id in bodies {
            let (kitchens, price, cfg) = kitchen
                .get_or_insert_with(|| (jail_kitchens(world), world.mean_price(), world.config.needs.clone()))
                .clone();
            jail_meal(world, id, &kitchens, price, &cfg);
        }
    }
}

/// L2 (L21): a held prisoner's jailed decay for the ticks since it was last
/// settled (`World::held_since`: its demotion into the hold, or the last
/// midnight pass), at most a day; the body's per-tick decay covered the
/// rest of the day. Restarts the clock.
pub fn settle_held(world: &mut World, id: EntityId) {
    let now = world.tick;
    let since = world.held_since.get(&id).copied().unwrap_or(now.saturating_sub(TICKS_PER_DAY));
    let ticks = now.saturating_sub(since).min(TICKS_PER_DAY) as u32;
    world.held_since.insert(id, now);
    if ticks == 0 {
        return;
    }
    if let Some(log) = world.shadow_notes.as_mut() {
        log.push(crate::word::ShadowNote::Settled { tick: now, id, ticks });
    }
    let cfg = world.config.needs.clone();
    let season_energy_mult = world.config.economy.energy_decay_mult[world.season().index()];
    let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
    let under_18 = !crate::systems::demography::is_adult(world, id);
    if let Some(n) = world.comp_mut::<Needs>(id) {
        let ctx = crate::needs::DecayCtx {
            jailed: true,
            season_energy_mult,
            sociability,
            under_18,
            ..crate::needs::DecayCtx::plain()
        };
        crate::needs::decay(n, &cfg, &ctx, ticks);
    }
    // L2 L13 (phase 2): the hold's `fun` decay too (a no-op with leisure off).
    let execs = crate::systems::classes::exec_set(world);
    let rate = crate::systems::leisure::fun_per_hour(world, id, &execs);
    if let Some(n) = world.comp_mut::<Needs>(id) {
        crate::systems::leisure::decay_fun(n, rate, ticks as f32 / crate::time::TICKS_PER_HOUR as f32);
    }
}

fn releases(world: &mut World) {
    let tick = world.tick;
    let due: Vec<EntityId> = world
        .sentenced()
        .iter()
        .copied()
        .filter(|&p| world.comp::<Sentence>(p).is_some_and(|s| tick >= s.until_tick))
        .collect();
    for p in due {
        release(world, p, true);
    }
}

/// Player `Arrest`: no witness roll, straight to the Jail for `3 × sentence_mult` days.
pub fn player_arrest(world: &mut World, who: EntityId) -> Result<(), String> {
    if !world.is_alive(who) || !world.has::<Brain>(who) {
        return Err("no such agent".into());
    }
    if world.has::<Sentence>(who) {
        return Err("already jailed".into());
    }
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return Err("no Precinct".into()) };
    if crate::systems::stations::precinct_bound(world) >= usize::from(world.config.buildings.jail.capacity) {
        return Err("the Precinct is full".into());
    }
    let until = world.tick + sentence_ticks(world, Crime::Theft);
    sentence(world, who, Crime::Theft, until, jail);
    resolve_reports(world, who);
    world.stats.current.arrests += 1;
    Ok(())
}

/// Player `Release`.
pub fn player_release(world: &mut World, who: EntityId) -> Result<(), String> {
    if !world.has::<Sentence>(who) {
        return Err("not jailed".into());
    }
    release(world, who, false);
    Ok(())
}

/// The next patrol route: `[Market, Bar, Hall, Home(rng), Home(rng)]` on a
/// beat. M10: with several Markets the beat is one drawn at random, the Bar
/// is the one nearest it and the Homes come from the `[law]
/// patrol_beat_homes` nearest it, so a loop fits in a shift on the 256 x 192
/// map (with every Home in play a single leg could take half the shift). On
/// the v1 map (one Market, 60 Homes) the loop is the v1 loop. Under
/// a Crackdown: `[Market, Home(t), Home(t), Hideout(t), Home(t)]`, the Homes
/// drawn from the target gang's territory (or, with fewer than three held,
/// from the six inhabited Homes nearest its Hideout).
///
/// M12 D11: a city guard with a district beat (`Law.beats`, dealt daily by
/// `law_brain::allocate`) walks its district instead ([`district_route`]);
/// a guard without one (districts off, Garrison, a fresh load) walks the
/// M11 route above exactly.
pub fn new_patrol_route(world: &mut World, guard: EntityId) -> Vec<EntityId> {
    if let Some(route) = private_patrol_route(world, guard) {
        return route;
    }
    let beat = world
        .law()
        .filter(|_| world.config.law.district_beats)
        .and_then(|l| l.beats.get(&guard).copied())
        .filter(|d| d.index() < world.districts.len());
    if let Some(d) = beat {
        let from = crate::exec::walk_origin(world, guard);
        return district_route(world, d, from);
    }
    let target = world.law().filter(|l| l.posture == Posture::Crackdown).and_then(|l| l.target);
    if let Some(gang) = target.filter(|&g| world.has::<crate::components::Gang>(g)) {
        return crackdown_route(world, gang, None);
    }
    let mut route = Vec::new();
    let markets: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .collect();
    // No draw with a single Market, so the v1 city keeps its v1 sequence.
    let beat = match markets.len() {
        0 => None,
        1 => Some(markets[0]),
        n => Some(markets[world.rng.world().random_range(0..n)]),
    };
    let beat_door = beat.and_then(|m| world.comp::<Building>(m)).map(|b| b.door);
    route.extend(beat);
    route.extend(beat_door.and_then(|d| world.nearest_of_kind(BuildingKind::Bar, d)));
    route.extend(world.building_of_kind(BuildingKind::Hall));
    let mut homes = world.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default();
    let beat_homes = world.config.law.patrol_beat_homes;
    if let (Some(door), true) = (beat_door, homes.len() > beat_homes) {
        let mut near: Vec<(u32, EntityId)> = homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(door), h)))
            .collect();
        near.sort();
        homes = near.into_iter().take(beat_homes).map(|(_, h)| h).collect();
    }
    if !homes.is_empty() {
        for _ in 0..2 {
            let i = world.rng.world().random_range(0..homes.len());
            route.push(homes[i]);
        }
    }
    route
}

/// M12 D11: a beat in district `d`. Under the district's `Crackdown(g)` the
/// crackdown loop on g's turf in d; else a Market in d drawn as the city's
/// beat is (no draw with one), the Bar nearest it, the Hall only when d
/// holds it, and two Homes drawn from the `patrol_beat_homes` Homes of d
/// nearest the Market (none in d: the nearest anywhere). Plan deviation: a
/// district with Homes and no Market (the Spire, the three Sumps) walks
/// four Homes drawn from the `patrol_beat_homes` nearest its centroid, so
/// the beat stays in the district; with the plan's Market nearest the
/// centroid, Sump guards spent their loop at a Mid Market and the Sump read
/// the coverage floor (0.5) with four guards allocated.
///
/// The pressure pass F1 (`[distance]` on, `from` the guard's place): the
/// Market is drawn among the district's within `[distance] job_cap_ticks`
/// of `from` (none: the nearest), and the Hall joins only within that
/// travel of the beat. A district label spans up to 1,000 tiles on the
/// island: a random Market of it sent guards on 4-13 h patrol legs.
/// The pressure pass R2: a district with no Market walks the
/// `patrol_beat_homes` of its Homes nearest `from`, not its centroid.
pub fn district_route(
    world: &mut World,
    d: crate::components::DistrictId,
    from: Option<crate::components::TilePos>,
) -> Vec<EntityId> {
    use crate::components::Stance;
    // M12 D34: under a Cordon every guard on the beat stands at the riot's target.
    if world.district(d).stance == Stance::Cordon {
        if let Some(t) = crate::systems::riot::cordon_target(world, d) {
            return vec![t];
        }
    }
    if let Stance::Crackdown(g) = world.district(d).stance {
        if world.has::<crate::components::Gang>(g) {
            return crackdown_route(world, g, Some(d));
        }
    }
    // Round 6 V5 (TURF § 3): the law's crackdown on a turf cell of the
    // district: the beat walks the cell's Homes.
    if let Some(route) = crate::systems::turf::crackdown_beat(world, d) {
        return route;
    }
    let standing = |w: &World, b: EntityId| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished);
    let of_kind = |w: &World, k: BuildingKind| -> Vec<EntityId> {
        w.district(d)
            .buildings
            .iter()
            .copied()
            .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| bd.kind == k && !bd.demolished))
            .collect()
    };
    let markets = of_kind(world, BuildingKind::Market);
    // R2: the patrol's own reach (`[distance.reach] patrol_ticks`), else
    // the job cap.
    let patrol_cap = crate::systems::distance::reach(world).and_then(|r| r.patrol_ticks);
    let near_cap = crate::systems::distance::job_cap(world).map(|c| patrol_cap.unwrap_or(c)).zip(from);
    // The draw stays one over the district's Markets (the world stream
    // unchanged), mapped onto those within reach.
    let mut pool = markets.clone();
    // The pressure pass R2 (OPEN-215): with `[distance]` on, a district
    // whose Markets all lie past the cap of the guard walks its Homes
    // nearest him (below), one Market or several: the Spire's one Market
    // sent Mid-Levels guards on 340-550-tile legs.
    let homes_d: Vec<EntityId> = world.district(d).homes.iter().copied().filter(|&h| standing(world, h)).collect();
    let far_markets = near_cap.is_some_and(|(cap, f)| {
        !homes_d.is_empty()
            && markets.iter().all(|&m| {
                world.comp::<Building>(m).is_none_or(|bd| crate::systems::distance::travel(world, f, bd.door) > cap)
            })
    });
    if let Some((cap, f)) = near_cap.filter(|_| markets.len() > 1) {
        let door = |w: &World, b: EntityId| w.comp::<Building>(b).map_or(f, |bd| bd.door);
        let within: Vec<EntityId> = markets
            .iter()
            .copied()
            .filter(|&m| crate::systems::distance::travel(world, f, door(world, m)) <= cap)
            .collect();
        pool = if within.is_empty() {
            markets.iter().copied().min_by_key(|&m| (door(world, m).manhattan(f), m)).into_iter().collect()
        } else {
            within
        };
    }
    let hall_here = !of_kind(world, BuildingKind::Hall).is_empty();
    let centroid = world.district(d).centroid;
    let beat = match markets.len() {
        _ if far_markets => None,
        0 if !homes_d.is_empty() => None,
        0 => world.nearest_of_kind(BuildingKind::Market, centroid),
        1 => Some(markets[0]),
        n => {
            let i = world.rng.world().random_range(0..n);
            Some(pool[i % pool.len().max(1)])
        }
    };
    let draws = if (markets.is_empty() || far_markets) && !homes_d.is_empty() { 4 } else { 2 };
    let beat_door = beat.and_then(|m| world.comp::<Building>(m)).map(|b| b.door);
    let mut route = Vec::new();
    route.extend(beat);
    if beat.is_some() {
        let bar = beat_door.and_then(|door| world.nearest_of_kind(BuildingKind::Bar, door).map(|b| (door, b)));
        // R2: the Bar only within the patrol's reach of the Market.
        route.extend(
            bar.filter(|&(door, b)| {
                patrol_cap.is_none_or(|cap| {
                    world
                        .comp::<Building>(b)
                        .is_some_and(|bd| crate::systems::distance::travel(world, door, bd.door) <= cap)
                })
            })
            .map(|(_, b)| b),
        );
    }
    if hall_here {
        let hall = world.building_of_kind(BuildingKind::Hall);
        // R2: a Homes-only beat measures the Hall from the guard.
        let anchor = beat_door.or(near_cap.map(|(_, f)| f));
        let near = |h: EntityId| match (near_cap, anchor, world.comp::<Building>(h).map(|b| b.door)) {
            (Some((cap, _)), Some(b), Some(hd)) => crate::systems::distance::travel(world, b, hd) <= cap,
            _ => true,
        };
        route.extend(hall.filter(|&h| near(h)));
    }
    let mut homes = if homes_d.is_empty() {
        world.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default()
    } else {
        homes_d
    };
    let beat_homes = world.config.law.patrol_beat_homes;
    // The pressure pass R2 (OPEN-215): a district with no Market walks the
    // Homes nearest the guard (`[distance]` on), not its centroid's: the
    // Spire is one label 150-350 tiles across.
    let from = beat_door.or(near_cap.map(|(_, f)| f)).unwrap_or(centroid);
    if homes.len() > beat_homes {
        let mut near: Vec<(u32, EntityId)> = homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(from), h)))
            .collect();
        near.sort();
        // R2: within the patrol's reach of the loop's anchor (walked), the
        // nearest always (the Spire's 60 nearest towers spanned 200 tiles).
        let tiles = patrol_cap.map(|c| world.config.exec.walk_tiles(c));
        let first = near.first().map(|&(t, _)| t).unwrap_or(0);
        homes = near
            .into_iter()
            .take(beat_homes)
            .filter(|&(t, _)| tiles.is_none_or(|r| u64::from(t) <= r) || t == first)
            .map(|(_, h)| h)
            .collect();
    }
    if !homes.is_empty() {
        for _ in 0..draws {
            let i = world.rng.world().random_range(0..homes.len());
            route.push(homes[i]);
        }
    }
    route
}

/// M12 D9: is the law cracking down on `gang` anywhere: the posture's
/// Crackdown, or (with district beats) any district's `Crackdown(gang)`?
pub fn cracking_down_on(world: &World, gang: EntityId) -> bool {
    use crate::components::Stance;
    world.law().is_some_and(|l| l.posture == Posture::Crackdown && l.target == Some(gang))
        || (world.config.law.district_beats && world.districts.iter().any(|d| d.stance == Stance::Crackdown(gang)))
}

/// M11 D18: a private guard walks up to five of its corp's contracted
/// clients, nearest its office first (Shadow V4 bug 11: five drawn
/// city-wide when it had more), or with no clients its corp's own
/// buildings nearest its Security Office.
/// `None` for a city guard, and for a guard whose Office no corp owns (none
/// should exist: `corps::move_building` lays them off when it changes hands).
fn private_patrol_route(world: &mut World, guard: EntityId) -> Option<Vec<EntityId>> {
    if !is_private_guard(world, guard) {
        return None;
    }
    let office = world.comp::<Job>(guard).and_then(|j| j.employer)?;
    let from = world.comp::<Building>(office)?.door;
    let corp = world.corp_of_building(office)?;
    let usable = |w: &World, b: EntityId| w.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.door);
    let clients: Vec<EntityId> = world
        .comp::<crate::components::Corp>(corp)
        .map(|c| c.contracts.iter().map(|&(b, _)| b).collect())
        .unwrap_or_default();
    let clients: Vec<EntityId> = clients.into_iter().filter(|&b| usable(world, b).is_some()).collect();
    // Shadow V4 bug 11: the five clients nearest the office, not five drawn
    // city-wide (a 382-min first leg, 88 h of travel a week at 20k).
    let pool: Vec<EntityId> = if clients.is_empty() {
        world.comp::<crate::components::Corp>(corp).map(|c| c.buildings.clone()).unwrap_or_default()
    } else {
        clients
    };
    let mut near: Vec<(u32, EntityId)> =
        pool.into_iter().filter_map(|b| usable(world, b).map(|d| (d.manhattan(from), b))).collect();
    near.sort();
    Some(near.into_iter().take(5).map(|(_, b)| b).collect())
}

/// The turf a Crackdown patrols: the gang's territory, or the six inhabited
/// Homes nearest its Hideout while it holds fewer than three.
pub fn crackdown_turf(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return Vec::new() };
    if g.territory.len() >= 3 {
        return g.territory.clone();
    }
    let Some(door) = world.comp::<Building>(g.hideout).map(|b| b.door) else { return Vec::new() };
    let mut homes: Vec<(u32, EntityId)> = world
        .buildings_by_kind()
        .get(&BuildingKind::Home)
        .map(|v| v.as_slice())
        .unwrap_or(&[])
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && !b.occupants.is_empty())
        .map(|(h, b)| (b.door.manhattan(door), h))
        .collect();
    homes.sort();
    homes.into_iter().take(6).map(|(_, h)| h).collect()
}

/// M12 D11: the turf a district Crackdown patrols: g's held Homes in `d`,
/// or, with fewer than three, the six inhabited Homes of `d` nearest g's
/// Hideout door.
pub fn crackdown_turf_in(world: &World, gang: EntityId, d: crate::components::DistrictId) -> Vec<EntityId> {
    let Some(g) = world.comp::<crate::components::Gang>(gang) else { return Vec::new() };
    let held: Vec<EntityId> = g.territory.iter().copied().filter(|&h| world.district_of_building(h) == d).collect();
    if held.len() >= 3 {
        return held;
    }
    let door = world.comp::<Building>(g.hideout).map_or(world.district(d).centroid, |b| b.door);
    let mut homes: Vec<(u32, EntityId)> = world
        .district(d)
        .homes
        .iter()
        .copied()
        .filter_map(|h| world.comp::<Building>(h).map(|b| (h, b)))
        .filter(|(_, b)| !b.demolished && !b.occupants.is_empty())
        .map(|(h, b)| (b.door.manhattan(door), h))
        .collect();
    homes.sort();
    homes.into_iter().take(6).map(|(_, h)| h).collect()
}

/// The crackdown loop: `[Market, Home(t), Home(t), Hideout, Home(t)]`. With
/// `district` (M12 D11) the turf is the district's ([`crackdown_turf_in`]),
/// the Market the one nearest the turf's first Home, and the Hideout stop is
/// kept only when it lies in the district; `None` is the M9 loop exactly.
pub fn crackdown_route(
    world: &mut World,
    gang: EntityId,
    district: Option<crate::components::DistrictId>,
) -> Vec<EntityId> {
    let turf = match district {
        Some(d) => crackdown_turf_in(world, gang, d),
        None => crackdown_turf(world, gang),
    };
    let hideout = match district {
        Some(d) => world.hideout_of(gang).filter(|&h| world.district_of_building(h) == d),
        None => world.hideout_of(gang),
    };
    let mut route = Vec::new();
    // The Market nearest the target's Hideout (the only one on the v1 map);
    // in a district, the one nearest the turf's first Home.
    let near = match district {
        Some(_) => turf.first().and_then(|&h| world.comp::<Building>(h)).map(|b| b.door),
        None => hideout.and_then(|h| world.comp::<Building>(h)).map(|b| b.door),
    };
    if let Some(m) = near.and_then(|d| world.nearest_of_kind(BuildingKind::Market, d)) {
        route.push(m);
    }
    let pick = |world: &mut World| {
        if !turf.is_empty() {
            let i = world.rng.world().random_range(0..turf.len());
            Some(turf[i])
        } else {
            None
        }
    };
    route.extend(pick(world));
    route.extend(pick(world));
    route.extend(hideout);
    route.extend(pick(world));
    route
}

/// The action a guard's shift uses today.
pub fn guard_duty_action(world: &World, guard: EntityId) -> ActionKind {
    let key = world.comp::<Job>(guard).map_or(0, |j| j.next_shift_key(world.tick));
    if jail_duty(world, guard, key) {
        ActionKind::GuardJail
    } else {
        ActionKind::PatrolLeg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scale S1: the bucketed `sightings` is the old every-guard scan: the
    /// shipped city on day 2 with a warrant on every tenth adult (so every
    /// guard sees someone), over an hour of ticks (the hourly Sighting
    /// note included), the whole world byte for byte after each tick.
    #[test]
    fn test_sightings_grid_matches_the_scan() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(2 * TICKS_PER_DAY + 7 * crate::time::TICKS_PER_HOUR - 3);
        // scan-ok: a unit test, once.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| living(&w, a)).collect();
        let tick = w.tick;
        for &s in people.iter().step_by(10) {
            w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
        }
        let mut seen_any = 0usize;
        for _ in 0..6 {
            let mut b = w.clone();
            sightings_scan(&mut b);
            let want = crate::save::to_ron(&b);
            // The grid always, and never (the small city's walk).
            for grid_pairs in [0, usize::MAX] {
                let mut a = w.clone();
                sightings_with(&mut a, grid_pairs);
                assert_eq!(a.last_seen(), b.last_seen(), "tick {}, grid past {grid_pairs}", w.tick);
                assert_eq!(crate::save::to_ron(&a), want, "tick {}, grid past {grid_pairs}", w.tick);
            }
            seen_any += b.last_seen().values().filter(|&&(_, t)| t == w.tick).count();
            // On to the next hour boundary or tick (both paths taken).
            w.run_ticks(1);
        }
        assert!(seen_any > 0, "no suspect was seen: the test exercises nothing");
    }

    /// Scale S1: the warrants a guard can chase, read from the by-tick
    /// sighting index (and the lazily built claim set), are the old scan of
    /// every open warrant's: every guard, and the whole city, at ticks over
    /// two days of a city with a warrant on every tenth adult.
    #[test]
    fn test_located_suspects_index_matches_the_scan() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 9 * crate::time::TICKS_PER_HOUR);
        // scan-ok: a unit test, once.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| living(&w, a)).collect();
        let tick = w.tick;
        for &s in people.iter().step_by(10) {
            w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
        }
        let mut found = 0usize;
        for _ in 0..24 {
            w.run_ticks(97);
            assert_eq!(located_suspects(&w, None), located_suspects_scan(&w, None), "the city, tick {}", w.tick);
            for &g in w.guards() {
                let Some(tile) = w.comp::<Position>(g).map(|p| p.tile) else { continue };
                let by = Some(Pursuer { tile, guard: g });
                let got = located_suspects(&w, by);
                assert_eq!(got, located_suspects_scan(&w, by), "guard {g}, tick {}", w.tick);
                assert_eq!(any_located_suspect(&w, by), !got.is_empty());
                found += got.len();
            }
        }
        assert!(found > 0, "no guard had a warrant in reach: the test exercises nothing");
    }

    /// The perf pass phase 5: the claim index (`World::claimed_by_other`)
    /// is the scan it replaced (`life::claimed_suspects`) for every guard
    /// and every claimed suspect, read at once after each of its
    /// invalidation points in one tick: an Arrest plan installed, an escort
    /// begun, both dropped, a citizen holding an Arrest plan hired as a
    /// guard, a claiming guard fired and another killed; then through a day
    /// of the city with a warrant on every tenth adult. Without the Brain
    /// mark (`comp_mut`) or the Job marks (`index_job` / `unindex_job`), the
    /// read after the write is stale.
    #[test]
    fn test_claim_index_matches_the_scan() {
        use crate::components::{DeathCause, GoalKind, Job, Role};
        use crate::goap::Plan;
        use crate::systems::life::claimed_suspects;
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 9 * crate::time::TICKS_PER_HOUR);
        // scan-ok: a unit test.
        let people: Vec<EntityId> = w.citizens().into_iter().filter(|&a| living(&w, a)).collect();
        let check = |w: &World, at: &str| -> usize {
            let guards = w.guards().to_vec();
            let mut all: BTreeSet<EntityId> = BTreeSet::new();
            for &g in &guards {
                all.extend(claimed_suspects(w, g));
            }
            let claimed = all.len();
            all.extend(people.iter().step_by(97).copied());
            for &g in &guards {
                let scan = claimed_suspects(w, g);
                for &s in &all {
                    assert_eq!(w.claimed_by_other(g, s), scan.contains(&s), "{at}: guard {g}, suspect {s}");
                }
            }
            claimed
        };
        let arrest = |w: &mut World, g: EntityId, s: EntityId| {
            let tick = w.tick;
            if let Some(b) = w.comp_mut::<Brain>(g) {
                b.plan = Some(Plan { goal: GoalKind::Arrest, target: Some(s), steps: Vec::new(), started_tick: tick });
            }
        };
        check(&w, "day 1 09:00");
        let guards = w.guards().to_vec();
        let civ: Vec<EntityId> = people.iter().copied().filter(|a| !guards.contains(a)).collect();
        let (g1, g2, g3) = (guards[0], guards[1], guards[2]);
        let (s1, s2, s3) = (civ[10], civ[20], civ[30]);
        // An Arrest plan installed: g1 is after s1.
        arrest(&mut w, g1, s1);
        check(&w, "an Arrest plan installed");
        assert!(w.claimed_by_other(g2, s1) && !w.claimed_by_other(g1, s1), "the plan claimed nobody");
        // An escort begun: g2 takes s2.
        if let Some(b) = w.comp_mut::<Brain>(g2) {
            b.escorting = Some(s2);
        }
        check(&w, "an escort begun");
        assert!(w.claimed_by_other(g1, s2), "the escort claimed nobody");
        // Both dropped.
        if let Some(b) = w.comp_mut::<Brain>(g1) {
            b.plan = None;
        }
        if let Some(b) = w.comp_mut::<Brain>(g2) {
            b.escorting = None;
        }
        check(&w, "both dropped");
        assert!(!w.claimed_by_other(g3, s1) && !w.claimed_by_other(g3, s2), "a dropped claim stayed");
        // A citizen after s3 (no guard: nobody's claim) hired as a guard.
        let recruit = civ[40];
        arrest(&mut w, recruit, s3);
        check(&w, "a citizen's Arrest plan");
        assert!(!w.claimed_by_other(g1, s3), "a citizen's plan claimed");
        let job = w.comp::<Job>(g1).cloned().expect("a guard's Job");
        assert_eq!(job.role, Role::Guard);
        w.insert(recruit, job);
        check(&w, "a guard hired");
        assert!(w.claimed_by_other(g1, s3), "the hire's claim unseen");
        // The recruit fired, g2 (after s1) killed.
        w.remove::<Job>(recruit);
        check(&w, "a guard fired");
        assert!(!w.claimed_by_other(g1, s3), "the fired guard's claim stayed");
        arrest(&mut w, g2, s1);
        check(&w, "an Arrest plan before the kill");
        w.kill(g2, DeathCause::Violence);
        check(&w, "a guard killed");
        // A day of the city, a warrant on every tenth adult, read every 97 ticks.
        let tick = w.tick;
        for &s in people.iter().step_by(10) {
            w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
        }
        let mut claims = 0;
        for _ in 0..15 {
            w.run_ticks(97);
            claims += check(&w, &format!("tick {}", w.tick));
        }
        assert!(claims > 0, "no guard claimed a suspect in the city: the test exercises nothing");
    }

    /// The perf pass phase 5: the law's indexes (the claim index, the
    /// needs' guard grid) are the scans they replaced (`spatial::scans()`),
    /// byte for byte: two days of the shipped city with a warrant on every
    /// tenth adult, run both ways, save the same (`band_refusals` aside: the
    /// switch also runs phase 3's labour-pool scan, which never counted it).
    #[test]
    fn test_law_indexes_match_the_scans() {
        let run = |scans: bool| {
            crate::spatial::SCANS.with(|s| s.set(scans));
            let mut w = World::new(42, crate::config::Config::load());
            w.run_ticks(9 * crate::time::TICKS_PER_HOUR);
            let tick = w.tick;
            // scan-ok: a unit test, once.
            for s in w.citizens().into_iter().step_by(10) {
                w.push_report(CrimeReport { crime: Crime::Theft, suspect: s, witness: None, tick, resolved: false });
            }
            w.run_ticks(2 * TICKS_PER_DAY);
            crate::spatial::SCANS.with(|s| s.set(false));
            // Phase 3's scan of the labour pool counts no `band_refusals`
            // (a saved counter of the indexed walk): masked, all else equal.
            let ron = crate::save::to_ron(&w);
            let mut out = String::with_capacity(ron.len());
            let mut rest = ron.as_str();
            while let Some(i) = rest.find("band_refusals:") {
                out.push_str(&rest[..i]);
                rest = rest[i + "band_refusals:".len()..].trim_start_matches(|c: char| c.is_ascii_digit());
                rest = rest.strip_prefix(',').unwrap_or(rest);
            }
            out.push_str(rest);
            (out, w.stats.history.iter().map(|r| r.arrests).sum::<u32>())
        };
        let (a, arrests) = run(false);
        let (b, _) = run(true);
        assert!(arrests > 0, "no arrest: the law's claims exercised nothing");
        assert!(a == b, "the indexes and the scans part");
    }
}
