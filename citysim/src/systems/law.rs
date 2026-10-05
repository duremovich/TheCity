//! Law and jail: witnessing, crime reports and warrants, suspect sightings,
//! arrest and escort, sentencing with jail capacity, release, jail upkeep,
//! and the player's Arrest / Release commands.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Crime, CrimeReport, DeathCause, Job, Lod, MemoryKind, Needs, Personality, Position,
    Role, Sentence, Skills, TilePos,
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
    world.citizens().into_iter().any(|g| g != id && is_guard(world, g) && near(world, id, g, r))
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
    }
}

/// Has `suspect` been reported for anything since `tick` (open, or filed and
/// resolved after the sighting)? A witness does not re-file a crime the law
/// already dealt with.
pub fn reported_since(world: &World, suspect: EntityId, tick: Tick) -> bool {
    world.crime_reports.iter().any(|r| r.suspect == suspect && (!r.resolved || r.tick >= tick))
}

/// Is there an open warrant on `suspect`?
pub fn wanted(world: &World, suspect: EntityId) -> bool {
    world.crime_reports.iter().any(|r| !r.resolved && r.suspect == suspect)
}

/// A crime happened: roll every nearby Brain-bearing agent as a witness, give
/// victims their memory, file a report if a guard saw it, and sharpen the
/// actor's stealth if nobody did.
pub fn raise_crime(world: &mut World, actor: EntityId, victim: Option<EntityId>, crime: Crime, tile: TilePos) {
    let cfg = world.config.crime.clone();
    let r = if world.is_dark() { cfg.sight_night_crime } else { cfg.sight_day_crime };
    let stealth = world.comp::<Skills>(actor).map_or(0.0, |s| s.stealth);
    let actor_building = world.comp::<Position>(actor).and_then(|p| p.building);
    let salience = crime_salience(crime);

    let witnesses: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&w| w != actor && world.comp::<Brain>(w).is_some_and(|b| b.lod != Lod::Statistical))
        .filter(|&w| {
            world.comp::<Position>(w).is_some_and(|p| {
                (actor_building.is_some() && p.building == actor_building) || chebyshev(p.tile, tile) <= r
            })
        })
        .collect();

    let mut noticed = 0usize;
    for w in witnesses {
        let guard = is_guard(world, w);
        let p = notice_probability(&cfg, stealth, guard);
        let roll: f32 = world.rng.world().random();
        if roll >= p {
            continue;
        }
        noticed += 1;
        world.remember_crime(w, actor, crime, salience);
        crate::systems::social::witnessed_crime_of(world, w, actor);
        if let Some(n) = world.comp_mut::<Needs>(w) {
            n.safety = (n.safety - 0.2).max(0.0);
        }
        world.push_event(
            EventKind::Witness,
            &[w, actor],
            format!("{} saw {} commit {crime:?}", world.name_of(w), world.name_of(actor)),
        );
        if guard {
            file_report(world, crime, actor, Some(w));
        }
    }
    if let Some(v) = victim {
        let kind = match crime {
            Crime::Assault | Crime::Murder => MemoryKind::Fought,
            Crime::Theft | Crime::Extortion => MemoryKind::WasRobbed,
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
    if noticed == 0 {
        if let Some(s) = world.comp_mut::<Skills>(actor) {
            s.stealth = (s.stealth + 0.01).min(1.0);
        }
    }
}

/// File (or refresh) the one open report per `(suspect, crime)`.
pub fn file_report(world: &mut World, crime: Crime, suspect: EntityId, witness: Option<EntityId>) {
    let tick = world.tick;
    if let Some(r) = world.crime_reports.iter_mut().find(|r| !r.resolved && r.suspect == suspect && r.crime == crime) {
        r.tick = tick;
        if r.witness.is_none() {
            r.witness = witness;
        }
        return;
    }
    world.crime_reports.push(CrimeReport { crime, suspect, witness, tick, resolved: false });
    let who = witness.map_or("the city".to_string(), |w| world.name_of(w));
    world.push_event(EventKind::Report, &[suspect], format!("{who} reported {} for {crime:?}", world.name_of(suspect)));
}

/// The open report on `suspect` whose sighting is freshest, if located.
pub fn located_suspects(world: &World) -> Vec<EntityId> {
    let window = world.config.crime.suspect_seen_ticks;
    let tick = world.tick;
    let mut out: Vec<EntityId> = world
        .crime_reports
        .iter()
        .filter(|r| !r.resolved)
        .map(|r| r.suspect)
        .filter(|&s| world.is_alive(s) && !world.has::<Sentence>(s))
        .filter(|&s| world.comp::<Brain>(s).is_some_and(|b| b.cuffed_by.is_none()))
        .filter(|&s| world.last_seen.get(&s).is_some_and(|&(_, t)| tick.saturating_sub(t) <= window))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Sentence length in ticks for a crime at the current lever.
pub fn sentence_ticks(world: &World, crime: Crime) -> Tick {
    let base = world.config.crime.sentence_days[crime as usize] as f32;
    let days = (base * world.levers.sentence_mult).ceil().max(1.0) as u64;
    days * TICKS_PER_DAY
}

/// The fight model's inputs, with the defaults for agents lacking the component.
pub fn fighting(world: &World, id: EntityId) -> f32 {
    world.comp::<Skills>(id).map_or(0.2, |s| s.fighting)
}

pub fn courage(world: &World, id: EntityId) -> f32 {
    world.comp::<Personality>(id).map_or(0.5, |p| p.courage)
}

/// `p_win = clamp(0.5 + 0.4 (fi_a − fi_b) + 0.1 (C_a − C_b), 0.1, 0.9)`.
/// Returns `(winner, loser, loser_died)`.
pub fn resolve_fight(world: &mut World, a: EntityId, b: EntityId) -> (EntityId, EntityId, bool) {
    let (fi, co) = (fighting, courage);
    let p_win = (0.5 + 0.4 * (fi(world, a) - fi(world, b)) + 0.1 * (co(world, a) - co(world, b))).clamp(0.1, 0.9);
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
    let p_death = world.config.crime.fight_death_p as f32 * (1.0 + fi(world, winner));
    let roll: f32 = world.rng.world().random();
    let died = roll < p_death;
    if died {
        world.kill_by(loser, DeathCause::Violence, Some(winner));
    }
    (winner, loser, died)
}

/// `Arrest` at completion: the guard must be adjacent (or in the same
/// building). A suspect with courage > 0.7 contests it. Returns whether the
/// suspect is now cuffed.
pub fn arrest(world: &mut World, guard: EntityId, suspect: EntityId) -> bool {
    if !living(world, suspect)
        || !living(world, guard)
        || world.has::<Sentence>(suspect)
        || !near(world, guard, suspect, 1)
    {
        return false;
    }
    let courage = world.comp::<Personality>(suspect).map_or(0.0, |p| p.courage);
    if courage > 0.7 {
        let (winner, loser, died) = resolve_fight(world, guard, suspect);
        if died {
            if loser == suspect {
                // The suspect died resisting: nothing left to prosecute.
                resolve_reports(world, suspect);
            } else {
                // The suspect killed the guard: a Murder, witnessed by whoever is near.
                let tile = world.comp::<Position>(suspect).map_or(TilePos::default(), |p| p.tile);
                file_report(world, Crime::Murder, suspect, None);
                raise_crime(world, suspect, None, Crime::Murder, tile);
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
            return false;
        }
    }
    world.abort_plan(suspect);
    if let Some(b) = world.comp_mut::<Brain>(suspect) {
        b.cuffed_by = Some(guard);
        b.current_goal = None;
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
    if !living(world, suspect) {
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
    // The most severe open crime sets the sentence; the rest are covered by it.
    let Some(report) = world
        .crime_reports
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
            Crime::Theft => {
                let fine = world.config.crime.fine_mult * world.market().map_or(1, |m| m.price_food);
                let coins = world.comp::<crate::components::Wallet>(suspect).map_or(0, |w| w.coins);
                if coins >= fine {
                    if let Some(w) = world.comp_mut::<crate::components::Wallet>(suspect) {
                        w.coins -= fine;
                    }
                    if let Some(t) = world.treasury_mut() {
                        t.coins += fine;
                    }
                    world.remember(suspect, MemoryKind::Paid, None, 0.4, -0.4, false);
                    resolve_reports(world, suspect);
                    let name = world.name_of(suspect);
                    world.push_event(EventKind::Sentence, &[suspect], format!("{name} fined {fine} coins (Jail full)"));
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
                        format!("{name} released: Jail full, cannot pay"),
                    );
                    release_at_jail_door(world, suspect, "unpunished");
                }
                return;
            }
            _ => {
                // Free the Theft prisoner with the longest remaining sentence.
                let victim = world
                    .with::<Sentence>()
                    .into_iter()
                    .filter(|&p| world.comp::<Sentence>(p).is_some_and(|s| s.crime == Crime::Theft))
                    .max_by_key(|&p| world.comp::<Sentence>(p).map_or(0, |s| s.until_tick));
                match victim {
                    Some(p) => release(world, p, false),
                    None => {
                        resolve_reports(world, suspect);
                        let name = world.name_of(suspect);
                        world.push_event(EventKind::Unpunished, &[suspect], format!("{name} released: Jail full"));
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
}

fn resolve_reports(world: &mut World, suspect: EntityId) {
    for r in world.crime_reports.iter_mut().filter(|r| r.suspect == suspect) {
        r.resolved = true;
    }
}

/// Put an agent in the Jail with a `Sentence`; a job survives a sentence of
/// three days or less.
pub fn sentence(world: &mut World, who: EntityId, crime: Crime, until: Tick, jail: EntityId) {
    // Whoever was escorting them is done; a cuffed prisoner is a contradiction.
    if let Some(g) = world.comp::<Brain>(who).and_then(|b| b.cuffed_by) {
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
    }
    if let Some(b) = world.comp_mut::<Brain>(who) {
        b.current_goal = None;
        b.lod = crate::components::Lod::Coarse;
    }
    let name = world.name_of(who);
    let days = until.saturating_sub(world.tick).div_ceil(TICKS_PER_DAY);
    world.push_event(EventKind::Sentence, &[who], format!("{name} sentenced to {days} days for {crime:?}"));
    crate::systems::gang::on_member_arrested(world, who);
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
    world.push_event(EventKind::Release, &[who], format!("{name} released from the Jail"));
}

/// Per tick: guards perceive wanted suspects; escorted suspects follow.
/// Daily: warrants expire, prisoners are fed and released.
pub fn run(world: &mut World) {
    sightings(world);
    if world.tick_of_day() == 0 {
        expire_warrants(world);
        jail_upkeep(world);
        reconcile_guards(world);
    }
    releases(world);
}

/// The `guard_count` lever, reconciled daily with at most five changes:
/// hire unemployed, unjailed adults of lawfulness >= 0.4 (highest first);
/// fire the guard with the lowest loyalty.
pub fn reconcile_guards(world: &mut World) {
    let want = usize::from(world.levers.guard_count);
    let guards: Vec<EntityId> = world.citizens().into_iter().filter(|&g| is_guard(world, g)).collect();
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return };
    if guards.len() < want {
        let mut candidates: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = world
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
            world.abort_plan(g);
            world.remove::<Job>(g);
            let name = world.name_of(g);
            world.push_event(EventKind::Fire, &[g], format!("{name} dismissed from the guard"));
        }
    }
}

/// Any guard perceiving (SIGHT, or same building) a wanted suspect records it.
fn sightings(world: &mut World) {
    let suspects: Vec<EntityId> = {
        let mut v: Vec<EntityId> = world
            .crime_reports
            .iter()
            .filter(|r| !r.resolved)
            .map(|r| r.suspect)
            .filter(|&s| world.is_alive(s) && !world.has::<Sentence>(s))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    if suspects.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    let guards: Vec<EntityId> = world.citizens().into_iter().filter(|&g| is_guard(world, g)).collect();
    let tick = world.tick;
    for s in suspects {
        if guards.iter().any(|&g| near(world, g, s, sight)) {
            if let Some(p) = world.comp::<Position>(s).map(|p| p.tile) {
                world.last_seen.insert(s, (p, tick));
            }
        }
    }
}

fn expire_warrants(world: &mut World) {
    let expiry = world.config.crime.warrant_expiry_days * TICKS_PER_DAY;
    let tick = world.tick;
    let mut expired = Vec::new();
    for r in world.crime_reports.iter_mut().filter(|r| !r.resolved && tick.saturating_sub(r.tick) >= expiry) {
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
    world.crime_reports.retain(|r| !r.resolved || tick.saturating_sub(r.tick) < 2 * expiry);
}

/// Prisoners get one meal a day from the Market, paid by the Treasury, and a
/// full night's sleep.
fn jail_upkeep(world: &mut World) {
    let price = world.market().map_or(0, |m| m.price_food);
    let market = world.building_of_kind(BuildingKind::Market);
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
            if let Some(t) = world.treasury_mut() {
                t.coins -= price;
            }
            if let Some(n) = world.comp_mut::<Needs>(who) {
                crate::needs::eat(n, &cfg);
            }
        }
        if let Some(n) = world.comp_mut::<Needs>(who) {
            n.energy = 1.0;
        }
    }
}

fn releases(world: &mut World) {
    let tick = world.tick;
    let due: Vec<EntityId> = world
        .with::<Sentence>()
        .into_iter()
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
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return Err("no Jail".into()) };
    if world.with::<Sentence>().len() >= usize::from(world.config.buildings.jail.capacity) {
        return Err("the Jail is full".into());
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

/// Is a guard's shift today a Jail day (`index % 2 == day % 2`) or a Patrol day?
pub fn jail_day(guard: EntityId, shift_key: i64) -> bool {
    i64::from(guard.index % 2) == shift_key.rem_euclid(2)
}

/// The next patrol route: `[Market, Bar, Hall, Home(rng), Home(rng)]`.
pub fn new_patrol_route(world: &mut World) -> Vec<EntityId> {
    let mut route = Vec::new();
    for kind in [BuildingKind::Market, BuildingKind::Bar, BuildingKind::Hall] {
        if let Some(b) = world.building_of_kind(kind) {
            route.push(b);
        }
    }
    let homes = world.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default();
    if !homes.is_empty() {
        for _ in 0..2 {
            let i = world.rng.world().random_range(0..homes.len());
            route.push(homes[i]);
        }
    }
    route
}

/// The action a guard's shift uses today.
pub fn guard_duty_action(world: &World, guard: EntityId) -> ActionKind {
    let key = world.comp::<Job>(guard).map_or(0, |j| j.next_shift_key(world.tick));
    if jail_day(guard, key) {
        ActionKind::GuardJail
    } else {
        ActionKind::PatrolLeg
    }
}
