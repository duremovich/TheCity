//! Law and jail: witnessing, crime reports and warrants, suspect sightings,
//! arrest and escort, sentencing with jail capacity, release, jail upkeep,
//! and the player's Arrest / Release commands.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Crime, CrimeReport, DeathCause, Job, Kit, LawShock, MemoryKind, Needs, Personality,
    Position, Posture, Role, Sentence, Skills, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::ActionKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// Chebyshev distance between two tiles.
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

/// A live agent: the entity exists and still has a Brain (corpses keep
/// Position and Identity, so `is_alive` alone is not enough).
pub fn living(world: &World, id: EntityId) -> bool {
    world.is_alive(id) && world.has::<Brain>(id)
}

pub fn is_guard(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
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
        // M13 D47.
        Crime::GrandTheft => 0.6,
        Crime::Manslaughter => 0.8,
        Crime::Abduction => 0.9,
        // M13 D38.
        Crime::Dealing => 0.5,
        // M14 V18.
        Crime::Intrusion => 0.4,
        Crime::DataTheft => 0.6,
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
        | Crime::Manslaughter
        | Crime::Dealing
        | Crime::Intrusion
        | Crime::DataTheft => {}
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
    let witnesses: Vec<EntityId> = world
        .bodies()
        .into_iter()
        .filter(|_| !detained)
        .filter(|&w| w != actor)
        // M14 V12: a body jacked in sees nothing.
        .filter(|w| world.runner_of.is_empty() || !world.runner_of.contains_key(w))
        .filter(|&w| {
            // M13 D32: a witness with Eyes sees `Kit.sight` tiles further.
            let reach = r + world.comp::<Kit>(w).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(w).is_some_and(|p| {
                (actor_building.is_some() && p.building == actor_building) || chebyshev(p.tile, tile) <= reach
            })
        })
        .collect();

    let mut noticed = 0usize;
    // M15 W28: a guard notices × the law's competence multiplier (1 when off).
    let law_mult = crate::systems::competence::law_mult(world);
    // M15 W24: a Hunt's strike on its target: the street keeps quiet.
    let silence = street_silence_on(world, actor, object, crime);
    let dedupe = world.config.life.enabled;
    for w in witnesses {
        // L1: one record per crime and witness: a multi-minute crime (a
        // Deal's every sale) was seen afresh each minute, each sighting
        // another -0.2 of affinity, so a stranger was an Enemy in 32 minutes.
        if dedupe && crate::systems::life::saw_recently(world, w, actor, crime) {
            noticed += 1;
            continue;
        }
        let guard = is_guard(world, w);
        let mut p = notice_probability(&cfg, stealth, guard);
        if guard && law_mult != 1.0 {
            p = (p * law_mult).clamp(0.0, 1.0);
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
        world.remember_crime(w, actor, crime, salience, object);
        crate::systems::social::witnessed_crime_of(world, w, actor);
        if let Some(n) = world.comp_mut::<Needs>(w) {
            n.safety = (n.safety - 0.2).max(0.0);
        }
        world.push_event(
            EventKind::Witness,
            &[w, actor],
            format!("{} saw {} commit {}", world.name_of(w), world.name_of(actor), crime.label()),
        );
        if guard {
            file_report(world, crime, actor, Some(w));
        }
    }
    // M13 D41: a robot victim has no memory, edges or needs.
    let living_victim = victim.is_some_and(|v| !crate::systems::robots::is_robot(world, v) && living(world, v));
    if let Some(v) = victim.filter(|&v| !crate::systems::robots::is_robot(world, v)) {
        let kind = match crime {
            Crime::Assault | Crime::Murder | Crime::Manslaughter | Crime::Abduction => MemoryKind::Fought,
            Crime::Theft
            | Crime::Extortion
            | Crime::Vagrancy
            | Crime::GrandTheft
            | Crime::Dealing
            | Crime::Intrusion
            | Crime::DataTheft => MemoryKind::WasRobbed,
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
    if world.config.gossip.enabled {
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
    }
}

/// M15 W24: is this an Attack's crime by a hunter on its Hunt's target,
/// with `[hunt] street_silence` on?
pub fn street_silence_on(world: &World, actor: EntityId, object: Option<EntityId>, crime: Crime) -> bool {
    world.config.gossip.enabled
        && world.config.hunt.street_silence
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
    let tick = world.tick;
    if world.refresh_open_report(crime, suspect, witness) {
        return;
    }
    world.push_report(CrimeReport { crime, suspect, witness, tick, resolved: false });
    // M13 D38: a new Dealing report (a refreshed one is the same warrant).
    if crime == Crime::Dealing {
        world.stats.current.dealing_reports += 1;
    }
    let who = witness.map_or("the city".to_string(), |w| world.name_of(w));
    world.push_event(
        EventKind::Report,
        &[suspect],
        format!("{who} reported {} for {}", world.name_of(suspect), crime.label()),
    );
    // Fix pass (phase 2 review): sleeping rough is not gang business; a
    // Vagrancy report against a member must not feed the wanted-gang log,
    // the pressure or the district's top gang.
    if crime != Crime::Vagrancy {
        crate::systems::law_brain::log_report(world, suspect);
    }
}

/// A wanted suspect the law can chase now: free, uncuffed and seen by a
/// guard within `suspect_seen_ticks`. Callers pair it with an open report.
fn located(world: &World, s: EntityId) -> bool {
    let window = world.config.crime.suspect_seen_ticks;
    let tick = world.tick;
    // L1: a corpse keeps its Position and Identity (`is_alive`), and guards
    // walked hours to arrest the dead.
    (if world.config.life.enabled { living(world, s) } else { world.is_alive(s) })
        && !world.has::<Sentence>(s)
        && world.comp::<Brain>(s).is_some_and(|b| b.cuffed_by.is_none())
        && world.last_seen.get(&s).is_some_and(|&(_, t)| tick.saturating_sub(t) <= window)
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

fn chaseable(world: &World, s: EntityId, by: Option<Pursuer>) -> bool {
    let near = |p: Pursuer| {
        let radius = pursuit_radius_for(world, p.guard);
        s != p.guard
            && world.last_seen.get(&s).is_some_and(|&(t, _)| t.manhattan(p.tile) <= radius)
            // L1: a stale sighting is chased only from near by, and a
            // suspect another guard is after is theirs.
            && (!world.config.life.enabled
                || (crate::systems::life::sighting_fresh(world, s, p.tile)
                    && !crate::systems::life::arrest_claimed(world, p.guard, s)))
    };
    // The cheap reach test first (the same conjunction, reordered).
    by.is_none_or(near) && located(world, s)
}

/// Open-warrant suspects that are located (and in reach of `by`), ascending.
pub fn located_suspects(world: &World, by: Option<Pursuer>) -> Vec<EntityId> {
    // `open_suspects` is ascending and deduplicated.
    world.open_suspects().filter(|&s| chaseable(world, s, by)).collect()
}

/// Is `located_suspects(world, by)` non-empty?
pub fn any_located_suspect(world: &World, by: Option<Pursuer>) -> bool {
    world.open_suspects().any(|s| chaseable(world, s, by))
}

/// Is `s` a wanted, located suspect (anywhere)?
pub fn is_located_suspect(world: &World, s: EntityId) -> bool {
    wanted(world, s) && located(world, s)
}

/// Sentence length in ticks for a crime at the current lever.
pub fn sentence_ticks(world: &World, crime: Crime) -> Tick {
    // M12 D15: a night in the cells, whatever the sentence lever.
    if crime == Crime::Vagrancy {
        return world.config.law.vagrancy_sentence_ticks.max(1);
    }
    // M13 D47: the crimes appended after Vagrancy read `sentence_days_ext`.
    let ext = &world.config.crime.sentence_days_ext;
    let base = match crime {
        Crime::GrandTheft => ext.grand_theft,
        Crime::Manslaughter => ext.manslaughter,
        Crime::Abduction => ext.abduction,
        Crime::Dealing => ext.dealing,
        // M14 V18.
        // V42 `SetHackSentence` replaces the base.
        Crime::Intrusion => world.levers.hack_sentence_days[0].map_or(ext.intrusion, u32::from),
        Crime::DataTheft => world.levers.hack_sentence_days[1].map_or(ext.data_theft, u32::from),
        _ => world.config.crime.sentence_days[crime as usize],
    } as f32;
    let days = (base * world.levers.sentence_mult).ceil().max(1.0) as u64;
    days * TICKS_PER_DAY
}

/// The fight model's inputs, with the defaults for agents lacking the
/// component. M13 D31: `Skills.fighting + Kit.fighting` (Arms), the Kit term
/// added only when there is one, so a bare fighter reads as M12.
pub fn fighting(world: &World, id: EntityId) -> f32 {
    // M13 D41: a robot (no Skills) fights at `robot_fighting[tier − 1]`.
    let base = match world.comp::<Skills>(id) {
        Some(s) => s.fighting,
        None => crate::systems::robots::fighting(world, id).unwrap_or(0.2),
    };
    let f = match world.comp::<Kit>(id).map(|k| k.fighting) {
        Some(k) if k != 0.0 => base + k,
        _ => base,
    };
    // M14 V12: a body jacked in fights at `jacked_fight_mult` (a branch).
    if !world.runner_of.is_empty() && world.runner_of.contains_key(&id) {
        return f * world.config.virt.jacked_fight_mult;
    }
    f
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
}

impl Default for FightMods {
    fn default() -> Self {
        FightMods { kill_mult: 1.0, a_bonus: 0.0, kill_p: None }
    }
}

/// `p_win = clamp(0.5 + 0.4 (fi_a − fi_b) + 0.1 (C_a − C_b), 0.1, 0.9)`.
/// Returns `(winner, loser, loser_died)`.
pub fn resolve_fight(world: &mut World, a: EntityId, b: EntityId) -> (EntityId, EntityId, bool) {
    resolve_fight_mods(world, a, b, FightMods::default())
}

/// M12 D32: [`resolve_fight`] with the death chance × `kill_mult` (a Crush).
pub fn resolve_fight_with(world: &mut World, a: EntityId, b: EntityId, kill_mult: f32) -> (EntityId, EntityId, bool) {
    resolve_fight_mods(world, a, b, FightMods { kill_mult, a_bonus: 0.0, kill_p: None })
}

/// M13 D31: the fight with its mods and the Kit terms. Only when either
/// side's Kit is not bare does `+ 0.2 (K.reflex_a − K.reflex_b)` join
/// `p_win`, and only when the loser has armour is the death chance ×
/// `(1 − armour)`: branches, not zero terms, so two unchromed fighters roll
/// exactly as in M12 (`tests/chrome.rs` asserts it).
///
/// M13 D41: a robot on either side (no Brain) fights at
/// `robot_fighting[tier − 1]` with courage 1.0 (`fighting`, `courage`); the
/// memories, drift, mood, safety, skill gain and `social::fought` are
/// skipped for it (the agent's memories name no subject); a robot loser is
/// wrecked (`assets::wreck`, Parts at the door), never killed; a robot that
/// kills passes its owner as the killer (no crime is raised for it).
pub fn resolve_fight_mods(world: &mut World, a: EntityId, b: EntityId, mods: FightMods) -> (EntityId, EntityId, bool) {
    let out = fight_inner(world, a, b, mods);
    // M14 V12: a body jacked in and attacked is dumped from its run.
    if !world.runner_of.is_empty() {
        for x in [a, b] {
            crate::systems::virt::dump(world, x, "attacked in the chair");
        }
    }
    out
}

fn fight_inner(world: &mut World, a: EntityId, b: EntityId, mods: FightMods) -> (EntityId, EntityId, bool) {
    if crate::systems::robots::is_robot(world, a) || crate::systems::robots::is_robot(world, b) {
        return resolve_robot_fight(world, a, b, mods);
    }
    let (fi, co) = (fighting, courage);
    let (kit_a, kit_b) = (world.comp::<Kit>(a).cloned(), world.comp::<Kit>(b).cloned());
    let kitted = kit_a.as_ref().is_some_and(|k| !k.is_bare()) || kit_b.as_ref().is_some_and(|k| !k.is_bare());
    let fi_a = if mods.a_bonus != 0.0 { fi(world, a) + mods.a_bonus } else { fi(world, a) };
    let p_win = if kitted {
        let reflex = |k: &Option<Kit>| k.as_ref().map_or(0.0, |k| k.reflex);
        (0.5 + 0.4 * (fi_a - fi(world, b))
            + 0.1 * (co(world, a) - co(world, b))
            + 0.2 * (reflex(&kit_a) - reflex(&kit_b)))
        .clamp(0.1, 0.9)
    } else {
        (0.5 + 0.4 * (fi_a - fi(world, b)) + 0.1 * (co(world, a) - co(world, b))).clamp(0.1, 0.9)
    };
    let roll: f32 = world.rng.world().random();
    let (winner, loser) = if roll < p_win { (a, b) } else { (b, a) };
    world.remember(a, MemoryKind::Fought, Some(b), 0.7, -0.5, false);
    world.remember(b, MemoryKind::Fought, Some(a), 0.7, -0.5, false);
    world.remember(winner, MemoryKind::Won, Some(loser), 0.6, 0.4, false);
    world.remember(loser, MemoryKind::Lost, Some(winner), 0.6, -0.6, false);
    if let Some(p) = world.comp_mut::<Personality>(winner) {
        p.drift(Drift::WonFight);
    }
    if let Some(p) = world.comp_mut::<Personality>(loser) {
        p.drift(Drift::LostFight);
    }
    if let Some(n) = world.comp_mut::<Needs>(loser) {
        n.safety = (n.safety - 0.4).max(0.0);
    }
    if let Some(m) = world.comp_mut::<crate::components::Mood>(loser) {
        m.value = (m.value - 0.3).max(-1.0);
    }
    if let Some(s) = world.comp_mut::<Skills>(winner) {
        s.fighting = (s.fighting + 0.01).min(1.0);
    }
    crate::systems::social::fought(world, winner, loser);
    let base = match mods.kill_p {
        Some(p) if winner == a => p,
        _ => world.config.crime.fight_death_p as f32 * (1.0 + fi(world, winner)),
    };
    let mut p_death = base * mods.kill_mult;
    let loser_kit = if loser == a { &kit_a } else { &kit_b };
    let armour = loser_kit.as_ref().map_or(0.0, |k| k.armour);
    if armour > 0.0 {
        p_death *= 1.0 - armour;
    }
    let roll: f32 = world.rng.world().random();
    let died = roll < p_death;
    if died {
        world.kill_by(loser, DeathCause::Violence, Some(winner));
    }
    (winner, loser, died)
}

/// M13 D41: [`resolve_fight_mods`] with a robot on one side or both. The
/// same `p_win` (the Kit terms read a robot's absent Kit as bare) and the
/// same two world-stream rolls, the death roll only for an agent loser.
fn resolve_robot_fight(world: &mut World, a: EntityId, b: EntityId, mods: FightMods) -> (EntityId, EntityId, bool) {
    let (fi, co) = (fighting, courage);
    let (kit_a, kit_b) = (world.comp::<Kit>(a).cloned(), world.comp::<Kit>(b).cloned());
    let kitted = kit_a.as_ref().is_some_and(|k| !k.is_bare()) || kit_b.as_ref().is_some_and(|k| !k.is_bare());
    let fi_a = fi(world, a) + mods.a_bonus;
    let reflex = |k: &Option<Kit>| k.as_ref().map_or(0.0, |k| k.reflex);
    let mut p_win = 0.5 + 0.4 * (fi_a - fi(world, b)) + 0.1 * (co(world, a) - co(world, b));
    if kitted {
        p_win += 0.2 * (reflex(&kit_a) - reflex(&kit_b));
    }
    let p_win = p_win.clamp(0.1, 0.9);
    let roll: f32 = world.rng.world().random();
    let (winner, loser) = if roll < p_win { (a, b) } else { (b, a) };
    let agent = |w: &World, x: EntityId| w.has::<Brain>(x);
    if agent(world, loser) {
        world.remember(loser, MemoryKind::Fought, None, 0.7, -0.5, false);
        world.remember(loser, MemoryKind::Lost, None, 0.6, -0.6, false);
        if let Some(p) = world.comp_mut::<Personality>(loser) {
            p.drift(Drift::LostFight);
        }
        if let Some(n) = world.comp_mut::<Needs>(loser) {
            n.safety = (n.safety - 0.4).max(0.0);
        }
        if let Some(m) = world.comp_mut::<crate::components::Mood>(loser) {
            m.value = (m.value - 0.3).max(-1.0);
        }
    }
    if agent(world, winner) {
        world.remember(winner, MemoryKind::Fought, None, 0.7, -0.5, false);
        world.remember(winner, MemoryKind::Won, None, 0.6, 0.4, false);
        if let Some(p) = world.comp_mut::<Personality>(winner) {
            p.drift(Drift::WonFight);
        }
        if let Some(s) = world.comp_mut::<Skills>(winner) {
            s.fighting = (s.fighting + 0.01).min(1.0);
        }
    }
    if crate::systems::robots::is_robot(world, loser) {
        crate::systems::assets::wreck(world, loser, "beaten in a fight");
        return (winner, loser, false);
    }
    let mut p_death = world.config.crime.fight_death_p as f32 * (1.0 + fi(world, winner)) * mods.kill_mult;
    let loser_kit = if loser == a { &kit_a } else { &kit_b };
    let armour = loser_kit.as_ref().map_or(0.0, |k| k.armour);
    if armour > 0.0 {
        p_death *= 1.0 - armour;
    }
    let roll: f32 = world.rng.world().random();
    let died = roll < p_death;
    if died {
        let killer = world.comp::<crate::components::Asset>(winner).and_then(|x| x.owner);
        world.kill_by(loser, DeathCause::Violence, killer);
    }
    (winner, loser, died)
}

/// `Arrest` at completion: the guard must be adjacent (or in the same
/// building). A suspect with courage > 0.7 contests it. Returns whether the
/// suspect is now cuffed.
pub fn arrest(world: &mut World, guard: EntityId, suspect: EntityId) -> bool {
    if guard == suspect
        || !living(world, suspect)
        || !living(world, guard)
        || world.has::<Sentence>(suspect)
        || !near(world, guard, suspect, 1)
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
                a_bonus: 0.0,
                kill_p: None,
            }
        } else {
            FightMods::default()
        };
        let (winner, loser, died) = resolve_fight_mods(world, guard, suspect, mods);
        if died {
            if loser == suspect {
                // The suspect died resisting: nothing left to prosecute.
                resolve_reports(world, suspect);
                if berserk {
                    crate::systems::chrome::ended_by_law(world, suspect, "killed by the law");
                }
            } else {
                // The suspect killed the guard: a Murder, witnessed by whoever is near.
                let tile = world.comp::<Position>(suspect).map_or(TilePos::default(), |p| p.tile);
                file_report(world, Crime::Murder, suspect, None);
                raise_crime_on(world, suspect, None, Some(guard), Crime::Murder, tile);
            }
            return false;
        }
        if winner != guard {
            if let Some(n) = world.comp_mut::<Needs>(suspect) {
                n.safety = (n.safety - 0.3).max(0.0);
            }
            world.push_event(
                EventKind::Assault,
                &[suspect, guard],
                format!("{} fought off {}", world.name_of(suspect), world.name_of(guard)),
            );
            crate::systems::law_brain::push_shock(world, LawShock::GuardBeaten);
            return false;
        }
    }
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
        b.cuffed_by = Some(guard);
        b.current_goal = None;
    }
    // M13 D33: the cuffs end an episode.
    if berserk {
        crate::systems::chrome::ended_by_law(world, suspect, "arrested");
    }
    if let Some(b) = world.comp_mut::<Brain>(guard) {
        b.escorting = Some(suspect);
    }
    world.push_event(
        EventKind::Arrest,
        &[guard, suspect],
        format!("{} arrested {}", world.name_of(guard), world.name_of(suspect)),
    );
    true
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
    }
}

/// Escort arrived at the Jail: sentence the suspect (or fine / release when
/// the Jail is full), resolve the report, free the guard.
pub fn jail_suspect(world: &mut World, guard: EntityId, suspect: EntityId) {
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
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return };
    let capacity = usize::from(world.config.buildings.jail.capacity);
    let jailed = world.with::<Sentence>().len();

    if jailed >= capacity {
        match crime {
            Crime::Theft | Crime::Vagrancy => {
                let fine = if crime == Crime::Vagrancy {
                    world.config.law.vagrancy_fine
                } else {
                    world.config.crime.fine_mult * world.mean_price()
                };
                let coins = world.comp::<crate::components::Wallet>(suspect).map_or(0, |w| w.coins);
                // M12 review: a Vagrancy report is settled as `street::vagrancy_hit`'s
                // fine is: where the vagrant was swept, on the ledger, in the log.
                let swept = (crime == Crime::Vagrancy).then(|| vagrancy_place(world, suspect));
                if coins >= fine {
                    if crime == Crime::Vagrancy {
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
                        format!("{name} fined {fine} coins (Precinct full)"),
                    );
                    if let Some(place) = swept {
                        crate::systems::street::note_vagrancy(world, place);
                        let text = format!("{name} fined {fine} for sleeping rough in {}", world.district_name(place));
                        world.push_event(EventKind::Vagrancy, &[suspect], text);
                    }
                    world.stats.current.arrests += 1;
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

    let until = world.tick + sentence_ticks(world, crime);
    sentence(world, suspect, crime, until, jail);
    resolve_reports(world, suspect);
    world.stats.current.arrests += 1;
    // M15: an arrest is public: talked about where the suspect lives (the
    // Precinct's district for the homeless), and on the law's own record.
    if world.config.gossip.enabled {
        let d =
            crate::systems::gossip::home_district(world, suspect).unwrap_or_else(|| world.district_of_building(jail));
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::Arrested, Some(suspect), None);
        let t = world.tick;
        world.arrest_log.push_back((t, suspect));
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
    world.push_event(EventKind::Sentence, &[who], format!("{name} sentenced to {days} days for {}", crime.label()));
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
    world.push_event(EventKind::Release, &[who], format!("{name} released from the Precinct"));
    crate::systems::gang::on_member_released(world, who);
}

/// M9: a convict is broken out. The `Sentence` goes, they stand at the Jail
/// door hunted (`safety` 0.3) and glad of it (`Escaped`), and the warrant
/// **reopens** for the sentence's crime, so a re-arrest sentences them afresh.
pub fn escape(world: &mut World, who: EntityId) {
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
    // M11 D18: only the city's guards hold the Jail, Garrison included. The
    // roster is read off the role index without a copy: this runs per guard
    // per tick (`credit_guard_shifts`) and in every guard's think.
    use crate::systems::law_brain::is_city_guard;
    if !is_city_guard(world, guard) {
        return false;
    }
    let modulus: i64 = match world.law().map_or((Posture::Patrol, None), |l| (l.posture, l.target)) {
        (Posture::Garrison, _) => return true,
        (Posture::Crackdown, Some(_)) => 3,
        _ => 2,
    };
    let all = world.guards();
    let rank = all.iter().take_while(|&&g| g.index < guard.index).filter(|&&g| is_city_guard(world, g)).count() as i64;
    let slot = shift_key.rem_euclid(modulus);
    if rank % modulus == slot {
        return true;
    }
    rank == 0 && (all.iter().filter(|&&g| is_city_guard(world, g)).count() as i64) <= slot
}

/// Per tick: guards perceive wanted suspects; escorted suspects follow.
/// Daily: warrants expire, prisoners are fed and released.
pub fn run(world: &mut World) {
    tally_watch(world);
    credit_guard_shifts(world);
    sightings(world);
    if world.tick_of_day() == 0 {
        expire_warrants(world);
        jail_upkeep(world);
        reconcile_guards(world);
        // M12 D23: the city's sweepers, toward `levers.sanitation_count`.
        crate::systems::districts::reconcile_sanitation(world);
    }
    // The captain's daily rescoring, after the guard roster is reconciled.
    crate::systems::law_brain::run(world);
    releases(world);
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
        let owed = worked && !struck && crate::exec::routine::is_workday(key) && !world.has::<Sentence>(g);
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
            if world.config.life.enabled {
                crate::systems::economy::collect_wage(world, g);
            } else {
                crate::systems::economy::maybe_quit(world, g);
            }
        }
    }
}

/// The `guard_count` lever, reconciled daily with at most five changes:
/// hire unemployed, unjailed adults of lawfulness >= 0.4 (highest first);
/// fire the guard with the lowest loyalty.
pub fn reconcile_guards(world: &mut World) {
    let want = usize::from(world.levers.guard_count);
    // M11 D18: the city's payroll only; private guards are the corps'.
    let guards: Vec<EntityId> = crate::systems::law_brain::guards(world);
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return };
    if guards.len() < want {
        let mut candidates: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = world
            // scan-ok: daily: reconcile_guards
            .citizens()
            .into_iter()
            .filter(|&id| world.has::<Brain>(id) && !world.has::<Job>(id) && !world.has::<Sentence>(id))
            .filter(|&id| crate::systems::demography::is_adult(world, id))
            .filter_map(|id| world.comp::<Personality>(id).map(|p| (ordered_float::OrderedFloat(p.lawfulness), id)))
            .filter(|&(l, _)| l.0 >= 0.4)
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, id) in candidates.into_iter().take((want - guards.len()).min(5)) {
            crate::systems::demography::hire(world, id, jail, Role::Guard);
            // A dead guard's vacancy is this hire, not a second one.
            if let Some(v) = world.vacancies.get_mut(&jail) {
                if let Some(i) = v.iter().position(|&r| r == Role::Guard) {
                    v.remove(i);
                }
                if v.is_empty() {
                    world.vacancies.remove(&jail);
                }
            }
        }
    } else if guards.len() > want {
        let mut by_loyalty: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = guards
            .iter()
            .filter_map(|&g| world.comp::<Personality>(g).map(|p| (ordered_float::OrderedFloat(p.loyalty), g)))
            .collect();
        by_loyalty.sort();
        for (_, g) in by_loyalty.into_iter().take((guards.len() - want).min(5)) {
            let text = format!("{} dismissed from the guard", world.name_of(g));
            crate::systems::economy::dismiss(world, g, None, text);
        }
    }
    crate::systems::law_brain::recompute_captain(world);
}

/// Any guard perceiving (SIGHT, or same building) a wanted suspect records it.
fn sightings(world: &mut World) {
    let life = world.config.life.enabled;
    let suspects: Vec<EntityId> = world
        .open_suspects()
        .filter(|&s| (if life { living(world, s) } else { world.is_alive(s) }) && !world.has::<Sentence>(s))
        .collect();
    if suspects.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    // `near` per (guard, suspect) pair, with the guards' positions read once.
    // M13 D32: a guard with Eyes sees `Kit.sight` tiles further.
    let guards: Vec<(TilePos, Option<EntityId>, u32)> = world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| (p.tile, p.building, extra))
        })
        .collect();
    let tick = world.tick;
    // M15 W12 (plan deviation: hourly, not every tick): the guards who see a
    // suspect hold a Sighting of them.
    let note = world.config.gossip.enabled && tick.is_multiple_of(crate::time::TICKS_PER_HOUR);
    let ids: Vec<EntityId> = if note { world.guards().to_vec() } else { Vec::new() };
    for s in suspects {
        let Some(ps) = world.comp::<Position>(s) else { continue };
        let (tile, building) = (ps.tile, ps.building);
        let seen =
            guards.iter().any(|&(t, b, extra)| (b.is_some() && b == building) || chebyshev(t, tile) <= sight + extra);
        if seen {
            world.last_seen.insert(s, (tile, tick));
            if note {
                let seers: Vec<EntityId> = ids
                    .iter()
                    .copied()
                    .filter(|&g| {
                        let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
                        world.comp::<Position>(g).is_some_and(|p| {
                            (p.building.is_some() && p.building == building) || chebyshev(p.tile, tile) <= sight + extra
                        })
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
/// coins used to vanish), and a full night's sleep.
fn jail_upkeep(world: &mut World) {
    let price = world.mean_price();
    let market = world
        .building_of_kind(BuildingKind::Jail)
        .and_then(|j| world.comp::<Building>(j).map(|b| b.door))
        .and_then(|door| world.nearest_of_kind(BuildingKind::Market, door));
    let cfg = world.config.needs.clone();
    for who in world.with::<Sentence>() {
        let fed = market.and_then(|m| world.comp_mut::<Building>(m)).is_some_and(|b| {
            if b.stock_food > 0 {
                b.stock_food -= 1;
                true
            } else {
                false
            }
        });
        if fed {
            if let Some(m) = market {
                let owner = world.owner_of(m);
                let paid = crate::systems::ownership::charge(
                    world,
                    None,
                    owner,
                    price,
                    crate::systems::ownership::Flow::JailFood,
                );
                crate::systems::ownership::credit(world, m, paid);
            }
            if let Some(n) = world.comp_mut::<Needs>(who) {
                crate::needs::eat(n, &cfg);
            }
            world.mark_day(who, crate::components::trace_flags::ATE);
        }
        if let Some(n) = world.comp_mut::<Needs>(who) {
            n.energy = 1.0;
        }
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
    if world.with::<Sentence>().len() >= usize::from(world.config.buildings.jail.capacity) {
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
        return district_route(world, d);
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
    let mut homes = world.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default();
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
pub fn district_route(world: &mut World, d: crate::components::DistrictId) -> Vec<EntityId> {
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
    let hall_here = !of_kind(world, BuildingKind::Hall).is_empty();
    let centroid = world.district(d).centroid;
    let homes_d: Vec<EntityId> = world.district(d).homes.iter().copied().filter(|&h| standing(world, h)).collect();
    let beat = match markets.len() {
        0 if !homes_d.is_empty() => None,
        0 => world.nearest_of_kind(BuildingKind::Market, centroid),
        1 => Some(markets[0]),
        n => Some(markets[world.rng.world().random_range(0..n)]),
    };
    let draws = if markets.is_empty() && !homes_d.is_empty() { 4 } else { 2 };
    let beat_door = beat.and_then(|m| world.comp::<Building>(m)).map(|b| b.door);
    let mut route = Vec::new();
    route.extend(beat);
    if beat.is_some() {
        route.extend(beat_door.and_then(|door| world.nearest_of_kind(BuildingKind::Bar, door)));
    }
    if hall_here {
        route.extend(world.building_of_kind(BuildingKind::Hall));
    }
    let mut homes = if homes_d.is_empty() {
        world.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default()
    } else {
        homes_d
    };
    let beat_homes = world.config.law.patrol_beat_homes;
    let from = beat_door.unwrap_or(centroid);
    if homes.len() > beat_homes {
        let mut near: Vec<(u32, EntityId)> = homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (b.door.manhattan(from), h)))
            .collect();
        near.sort();
        homes = near.into_iter().take(beat_homes).map(|(_, h)| h).collect();
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
/// clients (all of them, nearest its office first, when there are five or
/// fewer; else five drawn with the world stream as the city's beat is), or
/// with no clients its corp's own buildings nearest its Security Office.
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
    if clients.len() > 5 {
        let picks = (0..5).map(|_| clients[world.rng.world().random_range(0..clients.len())]).collect();
        return Some(picks);
    }
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
        .buildings_by_kind
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
