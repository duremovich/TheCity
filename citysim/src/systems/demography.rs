//! Demography: ageing, children, births, old age, death effects, corpses and
//! burial, inheritance, job search, emigration, immigration. Daily at
//! `tick_of_day == 0`; `on_death` per death; `bury` per BuryCorpse.

use rand::seq::IndexedRandom;
use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Child, Corpse, DeathCause, Household, Identity, Inventory, Job, Lod, Memory,
    MemoryKind, Mood, Needs, Personality, Position, RelKind, Role, Sentence, Sex, Skills, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{DAYS_PER_YEAR, TICKS_PER_DAY};
use crate::world::World;

/// 18 years.
pub const ADULT_AGE_DAYS: u32 = 18 * DAYS_PER_YEAR as u32;
/// 45 years: the end of child-bearing.
pub const FERTILE_UNTIL_DAYS: u32 = 45 * DAYS_PER_YEAR as u32;
pub const CHILD_FOOD_PER_DAY: f32 = 0.5;
pub const BIRTH_SPACING_DAYS: u64 = 120;

pub fn is_adult(world: &World, id: EntityId) -> bool {
    world.comp::<Identity>(id).is_some_and(|i| i.age_days >= ADULT_AGE_DAYS)
}

pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    if world.tick > 0 {
        ageing(world);
        children(world);
        births(world);
        old_age(world);
    }
    corpses(world);
    gravedigger_notices(world);
    job_search(world);
    emigration(world);
    if world.tick > 0 && world.day().is_multiple_of(7) {
        immigration(world);
    }
}

// ---------------------------------------------------------------------------
// Ageing and old age
// ---------------------------------------------------------------------------

fn ageing(world: &mut World) {
    let mut matured = Vec::new();
    for id in world.citizens() {
        let Some(ident) = world.comp_mut::<Identity>(id) else { continue };
        ident.age_days = ident.age_days.saturating_add(1);
        if ident.age_days == ADULT_AGE_DAYS && world.has::<Child>(id) {
            matured.push(id);
        }
    }
    for id in matured {
        mature(world, id);
    }
}

/// A child comes of age: Brain, Needs and job eligibility, at Coarse LOD.
pub fn mature(world: &mut World, id: EntityId) {
    world.remove::<Child>(id);
    let initial = world.config.world.needs_initial.clone();
    world.insert(
        id,
        Needs {
            hunger: initial.hunger,
            energy: initial.energy,
            safety: initial.safety,
            wealth: 0.0,
            belonging: initial.belonging,
            intimacy: initial.intimacy,
            starving_since: None,
        },
    );
    world.insert(id, Mood::default());
    world.insert(id, Memory::default());
    world.insert(id, Inventory { food: 0, stolen_food: 0 });
    world.insert(id, Brain { lod: Lod::Coarse, ..Brain::default() });
    // At the door, not pushed in over the cap: a resident may always enter
    // their own Home (see `capacity_exempt`), so they walk in on their own.
    if let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) {
        world.stand_at_door(id, home);
    }
}

/// Daily chance of dying of old age from `old_age_days` on.
pub fn old_age_roll(rng: &mut rand_chacha::ChaCha8Rng, age_days: u32, cfg: &crate::config::DemographyCfg) -> bool {
    age_days >= cfg.old_age_days && rng.random_bool(cfg.old_age_p_per_day)
}

fn old_age(world: &mut World) {
    let cfg = world.config.demography.clone();
    let mut dying = Vec::new();
    for id in world.citizens() {
        let Some(age) = world.comp::<Identity>(id).map(|i| i.age_days) else { continue };
        if old_age_roll(world.rng.world(), age, &cfg) {
            dying.push(id);
        }
    }
    for id in dying {
        world.kill(id, DeathCause::OldAge);
    }
}

// ---------------------------------------------------------------------------
// Children and births
// ---------------------------------------------------------------------------

/// Children of a Home: residents under 18.
fn children_of(world: &World, home: EntityId) -> Vec<EntityId> {
    world.residents_of(home).iter().copied().filter(|&c| world.has::<Child>(c)).collect()
}

/// Each Home feeds its children from the pantry; three days unfed kills.
fn children(world: &mut World) {
    let homes = world.buildings_by_kind.get(&BuildingKind::Home).cloned().unwrap_or_default();
    let mut starved = Vec::new();
    let school = world.config.demography.school_meals;
    let reserve = world.building_of_kind(BuildingKind::Warehouse);
    for home in homes {
        let kids = children_of(world, home);
        if kids.is_empty() {
            continue;
        }
        let fed = {
            let Some(b) = world.comp_mut::<Building>(home) else { continue };
            b.child_food_debt += CHILD_FOOD_PER_DAY * kids.len() as f32;
            let mut fed = true;
            while b.child_food_debt >= 1.0 {
                if b.stock_food == 0 {
                    fed = false;
                    break;
                }
                b.stock_food -= 1;
                b.child_food_debt -= 1.0;
            }
            fed
        };
        // M11 phase 5: school meals. A child whose pantry is empty eats from
        // the Reserve (children ate only from the pantry and the dole is per
        // adult: ~70 child starvation deaths in 120 days on seeds 42-44).
        let fed = fed || (school && school_meal(world, home, reserve));
        for k in kids {
            let Some(c) = world.comp_mut::<Child>(k) else { continue };
            if fed {
                c.hunger_days = 0;
            } else {
                c.hunger_days = c.hunger_days.saturating_add(1);
                if c.hunger_days >= 3 {
                    starved.push(k);
                }
            }
        }
    }
    for k in starved {
        world.kill(k, DeathCause::Starvation);
    }
}

/// Pay a Home's whole child-food debt from the Reserve; false when it cannot.
fn school_meal(world: &mut World, home: EntityId, reserve: Option<EntityId>) -> bool {
    let Some(r) = reserve else { return false };
    let want = world.comp::<Building>(home).map_or(0, |b| b.child_food_debt.floor() as u32);
    let have = world.comp::<Building>(r).map_or(0, |b| b.stock_food);
    if want == 0 || have < want {
        return false;
    }
    if let Some(b) = world.comp_mut::<Building>(r) {
        b.stock_food -= want;
    }
    if let Some(b) = world.comp_mut::<Building>(home) {
        b.child_food_debt -= want as f32;
    }
    true
}

/// Spouse pairs aged 18–45 in the same Home with intimacy >= 0.6 roll for a
/// birth; at most one per couple per 120 days.
fn births(world: &mut World) {
    let p_base = world.config.demography.birth_p_per_day;
    let tick = world.tick;
    let pairs: Vec<(EntityId, EntityId)> = world.spouses.iter().filter(|(a, b)| a < b).map(|(&a, &b)| (a, b)).collect();
    let mut born = Vec::new();
    for (a, b) in pairs {
        let fertile = |id: EntityId| {
            world.comp::<Identity>(id).is_some_and(|i| (ADULT_AGE_DAYS..=FERTILE_UNTIL_DAYS).contains(&i.age_days))
                && world.comp::<Needs>(id).is_some_and(|n| n.intimacy >= 0.6)
        };
        if !fertile(a) || !fertile(b) {
            continue;
        }
        let home_a = world.comp::<Household>(a).and_then(|h| h.home);
        let home_b = world.comp::<Household>(b).and_then(|h| h.home);
        let Some(home) = home_a.filter(|_| home_a == home_b) else { continue };
        let spaced = world.edge(a, b).is_some_and(|e| {
            e.last_birth_tick.is_none_or(|t| tick.saturating_sub(t) >= BIRTH_SPACING_DAYS * TICKS_PER_DAY)
        });
        if !spaced {
            continue;
        }
        let happy = world.comp::<Mood>(a).is_some_and(|m| m.value > 0.5)
            && world.comp::<Mood>(b).is_some_and(|m| m.value > 0.5);
        let p = p_base * if happy { 1.5 } else { 1.0 };
        if world.rng.world().random_bool(p.min(1.0)) {
            born.push((a, b, home));
        }
    }
    for (a, b, home) in born {
        let (mother, father) = match world.comp::<Identity>(a).map(|i| i.sex) {
            Some(Sex::Female) => (a, b),
            _ => (b, a),
        };
        spawn_child(world, mother, father, home);
        if let Some(e) = world.edges.get_mut(&crate::components::edge_key(a, b)) {
            e.last_birth_tick = Some(tick);
        }
    }
}

/// A newborn: Identity at age 0, blended Personality, Skills 0, a Wallet, the
/// parents' Home, Parent edges (0.6 / 0.8) and Family edges to siblings (0.3).
pub fn spawn_child(world: &mut World, mother: EntityId, father: EntityId, home: EntityId) -> EntityId {
    let tick = world.tick;
    let sex = if world.rng.world().random_bool(0.5) { Sex::Male } else { Sex::Female };
    let last = world
        .comp::<Identity>(father)
        .or_else(|| world.comp::<Identity>(mother))
        .map(|i| i.name.rsplit(' ').next().unwrap_or("Doe").to_string())
        .unwrap_or_else(|| "Doe".to_string());
    let first = world.random_first_name(sex);
    let personality = {
        let m = world.comp::<Personality>(mother).cloned();
        let f = world.comp::<Personality>(father).cloned();
        Personality::inherit(m.as_ref(), f.as_ref(), world.rng.world())
    };
    let id = world.spawn();
    world.insert(
        id,
        Identity { name: format!("{first} {last}"), age_days: 0, sex, born_tick: tick as i64, spouse_died_tick: None },
    );
    world.insert(id, personality);
    world.insert(id, Skills { stealth: 0.0, fighting: 0.0, farming: 0.0 });
    world.insert(id, Wallet { coins: 0 });
    world.insert(id, Household::new(Some(home)));
    world.insert(id, Child { hunger_days: 0 });
    // In the Home, but not an occupant: a birth may exceed the capacity of 6.
    let door = world.comp::<Building>(home).map_or(TilePos::default(), |b| b.door);
    world.insert(id, Position { tile: door, building: Some(home), entered: tick });
    // Siblings: the parents' other children.
    let mut siblings: Vec<EntityId> = Vec::new();
    for parent in [mother, father] {
        for o in world.neighbours(parent) {
            if o != id
                && world.edge(parent, o).is_some_and(|e| e.kind == RelKind::Parent)
                && is_child_of(world, o, parent)
                && !siblings.contains(&o)
            {
                siblings.push(o);
            }
        }
    }
    for parent in [mother, father] {
        let e = world.edge_entry(parent, id);
        e.kind = RelKind::Parent;
        e.affinity = 0.6;
        e.trust = 0.8;
        e.last_interaction = tick;
    }
    for s in siblings {
        let e = world.edge_entry(s, id);
        e.kind = RelKind::Family;
        e.affinity = 0.3;
        e.last_interaction = tick;
    }
    world.stats.current.births += 1;
    let (nm, nf) = (world.name_of(mother), world.name_of(father));
    let name = world.name_of(id);
    world.push_event(EventKind::Birth, &[id, mother, father], format!("{name} born to {nm} and {nf}"));
    id
}

/// On a Parent edge, the younger party is the child.
pub fn is_child_of(world: &World, child: EntityId, parent: EntityId) -> bool {
    world.edge(child, parent).is_some_and(|e| e.kind == RelKind::Parent)
        && match (world.comp::<Identity>(child), world.comp::<Identity>(parent)) {
            (Some(c), Some(p)) => c.born_tick > p.born_tick,
            _ => false,
        }
}

/// Living children of `id` (Parent edges to someone born later).
pub fn children_of_agent(world: &World, id: EntityId) -> Vec<EntityId> {
    world
        .neighbours(id)
        .filter(|&o| world.has::<Identity>(o) && !world.has::<Corpse>(o) && is_child_of(world, o, id))
        .collect()
}

// ---------------------------------------------------------------------------
// Death effects, corpses and burial
// ---------------------------------------------------------------------------

/// Called by `World::kill` before the components go: witnesses, grief,
/// inheritance.
pub fn on_death(world: &mut World, id: EntityId) {
    let tick = world.tick;
    let (tile, building) = world.comp::<Position>(id).map_or((TilePos::default(), None), |p| (p.tile, p.building));
    // 4. SawCorpse to Brain agents within 6 tiles or in the same building.
    let sight = world.config.crime.sight;
    let witnesses: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&o| o != id && world.has::<Brain>(o))
        .filter(|&o| {
            world.comp::<Position>(o).is_some_and(|p| {
                (building.is_some() && p.building == building) || crate::systems::law::chebyshev(p.tile, tile) <= sight
            })
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
    // M11 D45: owned buildings pass to the spouse, a child, else the city.
    crate::systems::ownership::on_owner_gone(world, id);
    // 6. Inheritance: spouse, else children equally (remainder to the first), else Treasury.
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins).max(0);
    if coins > 0 {
        let spouse = world.neighbours(id).find(|&o| {
            world.edge(id, o).is_some_and(|e| e.kind == RelKind::Spouse)
                && world.has::<Identity>(o)
                && !world.has::<Corpse>(o)
        });
        let heirs: Vec<EntityId> = match spouse {
            Some(s) => vec![s],
            None => children_of_agent(world, id),
        };
        if heirs.is_empty() {
            if let Some(t) = world.treasury_mut() {
                t.coins += coins;
            }
            world.push_event(EventKind::Inheritance, &[id], format!("{coins} coins to the Treasury"));
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
                &[id, heirs[0]],
                format!("{coins} coins to {name}{}", if n > 1 { " and others" } else { "" }),
            );
        }
        if let Some(w) = world.comp_mut::<Wallet>(id) {
            w.coins = 0;
        }
    }
}

/// Daily: old unburied corpses frighten the neighbourhood, very old ones rot
/// away, buried ones are freed a day after burial.
fn corpses(world: &mut World) {
    let tick = world.tick;
    let sight = world.config.crime.sight;
    let mut freed = Vec::new();
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
        if age > 2 * TICKS_PER_DAY {
            let tile = world.comp::<Position>(c).map_or(TilePos::default(), |p| p.tile);
            let near: Vec<EntityId> = world
                .citizens()
                .into_iter()
                .filter(|&o| world.has::<Brain>(o))
                .filter(|&o| {
                    world.comp::<Position>(o).is_some_and(|p| crate::systems::law::chebyshev(p.tile, tile) <= sight)
                })
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

/// The Gravedigger is told of every unburied corpse daily so Bury can fire.
fn gravedigger_notices(world: &mut World) {
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
    world.remember(digger, MemoryKind::Grief, Some(corpse), 0.3, -0.2, false);
    world.stats.current.burials += 1;
    let (nd, nc) = (world.name_of(digger), world.name_of(corpse));
    world.push_event(EventKind::Burial, &[digger, corpse], format!("{nd} buried {nc}"));
    true
}

/// Remove a corpse entity entirely (rotted or a day after burial).
pub fn free_corpse(world: &mut World, corpse: EntityId) {
    // Anyone carrying it puts it down.
    for id in world.citizens() {
        if world.comp::<Brain>(id).is_some_and(|b| b.carrying_corpse == Some(corpse)) {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.carrying_corpse = None;
            }
            world.abort_plan(id);
        }
    }
    world.remove_agent(corpse);
}

// ---------------------------------------------------------------------------
// Job search
// ---------------------------------------------------------------------------

/// Daily, for each vacancy in BTreeMap order: the nearest unemployed adult
/// (home door to workplace door, ties by id) is hired.
fn job_search(world: &mut World) {
    let vacancies: Vec<(EntityId, Vec<Role>)> = world.vacancies.iter().map(|(&e, r)| (e, r.clone())).collect();
    for (employer, roles) in vacancies {
        let Some(workplace_door) = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door) else {
            world.vacancies.remove(&employer);
            continue;
        };
        for role in roles {
            let candidate = world
                .citizens()
                .into_iter()
                .filter(|&id| world.comp::<Brain>(id).is_some_and(|b| !b.emigrating))
                .filter(|&id| !world.has::<Job>(id) && !world.has::<Sentence>(id))
                .filter(|&id| is_adult(world, id))
                .filter(|&id| role != Role::Guard || world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.4))
                .map(|id| {
                    let from = world
                        .comp::<Household>(id)
                        .and_then(|h| h.home)
                        .and_then(|h| world.comp::<Building>(h))
                        .map(|b| b.door)
                        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
                        .unwrap_or_default();
                    (from.manhattan(workplace_door), id)
                })
                .min();
            let Some((_, id)) = candidate else { continue };
            hire(world, id, employer, role);
            if let Some(v) = world.vacancies.get_mut(&employer) {
                if let Some(i) = v.iter().position(|&r| r == role) {
                    v.remove(i);
                }
                if v.is_empty() {
                    world.vacancies.remove(&employer);
                }
            }
        }
    }
}

pub fn hire(world: &mut World, id: EntityId, employer: EntityId, role: Role) {
    let wc = &world.config.world;
    let shifts =
        if role == Role::Guard && id.index.is_multiple_of(2) { wc.shift_night.clone() } else { wc.shift_day.clone() };
    let wage_per_day = world.config.economy.wage(role);
    world.insert(
        id,
        Job {
            employer: Some(employer),
            role,
            wage_per_day,
            shifts,
            days_unpaid: 0,
            tax_accum: 0.0,
            last_shift_day: None,
            last_wage_attempt_day: None,
            duty_ticks: 0,
            hired_tick: world.tick,
            struck_shift: None,
        },
    );
    world.abort_plan(id);
    let name = world.name_of(id);
    world.push_event(EventKind::Hire, &[id, employer], format!("{name} hired as {}", role.label()));
}

// ---------------------------------------------------------------------------
// Emigration and immigration
// ---------------------------------------------------------------------------

/// Daily: mood below the threshold for `emigrate_days` marks the agent as
/// leaving; the executor walks them to the edge and `emigrate` despawns them.
/// M11 § 7: a Corp-class agent never emigrates.
fn emigration(world: &mut World) {
    let cfg = world.config.demography.clone();
    let tick = world.tick;
    let leaving: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&id| world.has::<Brain>(id) && !world.has::<Sentence>(id))
        .filter(|&id| world.comp::<Brain>(id).is_some_and(|b| !b.emigrating && b.cuffed_by.is_none()))
        .filter(|&id| {
            world.comp::<Mood>(id).is_some_and(|m| {
                m.value < cfg.emigrate_mood
                    && m.low_since
                        .is_some_and(|s| tick.saturating_sub(s) >= u64::from(cfg.emigrate_days) * TICKS_PER_DAY)
            })
        })
        .collect();
    let leaving: Vec<EntityId> = if leaving.is_empty() {
        leaving
    } else {
        leaving
            .into_iter()
            .filter(|&id| crate::systems::classes::class_of(world, id) != crate::components::Class::Corp)
            .collect()
    };
    for id in leaving {
        start_emigrating(world, id, "");
    }
}

/// Mark an agent as leaving: the plan is dropped and the executor walks them
/// to the map edge (`emigrate` despawns them there). `why` is appended to
/// the event text when not empty.
pub fn start_emigrating(world: &mut World, id: EntityId, why: &str) {
    world.abort_plan(id);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.emigrating = true;
        b.current_goal = None;
    }
    let name = world.name_of(id);
    let text = if why.is_empty() {
        format!("{name} is leaving the city")
    } else {
        format!("{name} is leaving the city ({why})")
    };
    world.push_event(EventKind::Emigration, &[id], text);
}

/// The nearest map-edge Road tile.
pub fn nearest_edge_road(world: &World, from: TilePos) -> Option<TilePos> {
    world.edge_roads.iter().copied().min_by_key(|t| (t.manhattan(from), t.y, t.x))
}

/// The emigrant has reached the edge: logged, counted, removed.
pub fn emigrate(world: &mut World, id: EntityId) {
    let name = world.name_of(id);
    world.stats.current.emigrants += 1;
    world.push_event(EventKind::Emigration, &[id], format!("{name} emigrated"));
    world.remove_agent(id);
}

/// Weekly: `immigration_per_week` newcomers at the map edge; M11 § 7 scales
/// the lever by Street happiness (`classes::immigrants_this_week`).
fn immigration(world: &mut World) {
    let n = crate::systems::classes::immigrants_this_week(world);
    for _ in 0..n {
        spawn_immigrant(world);
    }
}

/// One immigrant: adult of random age, uniform Personality, Skills 0.1,
/// 15 coins, Coarse, at a random edge road; housed in the emptiest Home
/// under 6 residents, else homeless.
pub fn spawn_immigrant(world: &mut World) -> EntityId {
    let tick = world.tick;
    let sex = if world.rng.world().random_bool(0.5) { Sex::Male } else { Sex::Female };
    let name = world.random_name(sex);
    let (age_days, personality, tile) = {
        let edge_roads = world.edge_roads.clone();
        let (lo, hi) = (world.config.world.age_min_years, world.config.world.age_max_years);
        let rng = world.rng.world();
        let years: f32 = rng.random_range(lo..hi);
        let age_days = ((years * DAYS_PER_YEAR as f32) as u32).max(ADULT_AGE_DAYS);
        let personality = Personality {
            lawfulness: rng.random(),
            greed: rng.random(),
            pride: rng.random(),
            sociability: rng.random(),
            courage: rng.random(),
            loyalty: rng.random(),
        };
        let tile = edge_roads.choose(rng).copied().unwrap_or_default();
        (age_days, personality, tile)
    };
    let initial = world.config.world.needs_initial.clone();
    let id = world.spawn();
    world.insert(
        id,
        Identity {
            name,
            age_days,
            sex,
            born_tick: tick as i64 - i64::from(age_days) * TICKS_PER_DAY as i64,
            spouse_died_tick: None,
        },
    );
    world.insert(id, personality);
    world.insert(id, Skills { stealth: 0.1, fighting: 0.1, farming: 0.1 });
    world.insert(id, Wallet { coins: 15 });
    world.insert(id, Inventory { food: 0, stolen_food: 0 });
    world.insert(
        id,
        Needs {
            hunger: initial.hunger,
            energy: initial.energy,
            safety: initial.safety,
            wealth: 0.0,
            belonging: initial.belonging,
            intimacy: initial.intimacy,
            starving_since: None,
        },
    );
    world.insert(id, Mood::default());
    world.insert(id, Memory::default());
    world.insert(id, Brain { lod: Lod::Coarse, ..Brain::default() });
    world.insert(id, Position { tile, building: None, entered: tick });
    let home = emptiest_home(world);
    world.insert(id, Household::new(home));
    world.stats.current.immigrants += 1;
    let name = world.name_of(id);
    let where_ = match home {
        Some(h) => format!("housed in Block#{}", h.index),
        None => "homeless".to_string(),
    };
    world.push_event(EventKind::Immigration, &[id], format!("{name} arrived, {where_}"));
    if home.is_none() {
        world.push_event(EventKind::Homeless, &[id], format!("{name} has no home"));
    }
    id
}

/// The Home with the fewest residents among those under 6 (ties by id).
pub fn emptiest_home(world: &World) -> Option<EntityId> {
    let mut counts: std::collections::BTreeMap<EntityId, usize> = world
        .buildings_by_kind
        .get(&BuildingKind::Home)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| !b.demolished))
        .map(|h| (h, 0))
        .collect();
    for c in world.citizens() {
        if let Some(h) = world.comp::<Household>(c).and_then(|h| h.home) {
            if let Some(n) = counts.get_mut(&h) {
                *n += 1;
            }
        }
    }
    counts.into_iter().filter(|&(_, n)| n < 6).min_by_key(|&(h, n)| (n, h)).map(|(h, _)| h)
}
