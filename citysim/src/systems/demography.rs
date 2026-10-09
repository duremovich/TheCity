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
use crate::time::{Tick, DAYS_PER_YEAR, TICKS_PER_DAY};
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
        // The Real economy E38: the child protective service, after the
        // Blocks fed their children (a child is judged on today's meal).
        crate::systems::camp::take_daily(world);
    }
    corpses(world);
    gravedigger_notices(world);
    job_search(world);
    // Jobs and room J19: the day's labour market for the migrants' means.
    note_labour_market(world);
    // L1: a long commute moves nearer work (a few households a night).
    crate::systems::life::relocate(world);
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
            fun: 1.0,
        },
    );
    world.insert(id, Mood::default());
    world.insert(id, Memory::default());
    world.insert(id, Inventory { food: 0, stolen_food: 0, stims: 0, parts: 0 });
    world.insert(id, Brain { lod: Lod::Coarse, ..Brain::default() });
    // The Real economy E41: a camp's child is released at its door.
    if crate::systems::camp::on_mature(world, id) {
        return;
    }
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
    world.insert(id, Skills::basic(0.0, 0.0, 0.0, 0.0));
    // M14 V35: half the parents' mean, half the child's keyed draw.
    let hacking = crate::systems::tech::child_hacking(world, id, mother, father);
    crate::systems::tech::give_hacking(world, id, hacking);
    // M15 W25: the parents' social skills blended with the child's own draw.
    let social = crate::systems::moves::child_social(world, id, mother, father);
    crate::systems::moves::give_social(world, id, social);
    world.insert(id, Wallet { coins: 0 });
    world.insert(id, Household::new(Some(home)));
    world.insert(id, Child { hunger_days: 0 });
    // M13 D4: the parents' mean from the child's keyed stream.
    let body = crate::systems::assets::child_body(world, id, mother, father);
    crate::systems::assets::give_body(world, id, body);
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
/// inheritance. M13 D13: with `[assets] loot_window_hours > 0` the coins
/// and goods stay on the body (returned, for the Corpse) until stripped,
/// buried or settled; with 0 (or assets off) the coins are inherited at
/// once, exactly as M12, and the loot is empty.
pub fn on_death(world: &mut World, id: EntityId) -> crate::components::Loot {
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
    let spouse = world.neighbours(dead).find(|&o| {
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
        // M12 D17: an unburied body a day old fouls the street around it.
        if age >= TICKS_PER_DAY {
            if let Some((t, b)) = world.comp::<Position>(c).map(|p| (p.tile, p.building)) {
                crate::systems::litter::deposit_near(world, t, b, 24, 0);
            }
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

/// Jobs and room J4: a layoff earns the rehire bonus for this many days.
pub const REHIRE_DAYS: u64 = 30;

/// J4: note a layoff (`economy::dismiss_as`, every employer-side dismissal)
/// for the rehire bonus. Written only with wages on.
pub fn note_laid_off(world: &mut World, id: EntityId, role: Role) {
    if crate::systems::wages::on(world) {
        let now = world.tick;
        world.laid_off.insert(id, (role, now));
    }
}

/// J4: was `id` laid off from `role` in the last [`REHIRE_DAYS`]?
pub fn laid_off_from(world: &World, id: EntityId, role: Role) -> bool {
    world.laid_off.get(&id).is_some_and(|&(r, t)| r == role && t + REHIRE_DAYS * TICKS_PER_DAY > world.tick)
}

/// Daily, for each vacancy in BTreeMap order: the nearest unemployed adult
/// (home door to workplace door, ties by id) is hired.
fn job_search(world: &mut World) {
    // J4: layoffs older than the bonus's window are forgotten.
    if !world.laid_off.is_empty() {
        let now = world.tick;
        world.laid_off.retain(|_, &mut (_, t)| t + REHIRE_DAYS * TICKS_PER_DAY > now);
    }
    let vacancies: Vec<(EntityId, Vec<Role>)> = world.vacancies.iter().map(|(&e, r)| (e, r.clone())).collect();
    for (employer, roles) in vacancies {
        let Some(workplace_door) = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door) else {
            world.vacancies.remove(&employer);
            continue;
        };
        for role in roles {
            let Some(id) = pick_candidate(world, employer, workplace_door, role) else { continue };
            hire(world, id, employer, role);
            // L2 L10: a public-works vacancy's hire joins the works roster.
            crate::systems::budget::note_hire(world, id, employer, role);
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

/// The adult `job_search` would hire for `role` at a workplace door: the
/// nearest unemployed, free adult (home door, else tile; ties by id), a
/// guard lawful (≥ 0.4); a Lab the best hacker, a Feed the most
/// knowledgeable (ties lower id). Jobs and room J4 (wages on): the default
/// key is `1024 + distance − round(skill_w × 100 × skill) − rehire_bonus`
/// (the role's `competence::role_skill`; the bonus for an adult laid off
/// from the same role in the last 30 days).
fn pick_candidate(world: &World, employer: EntityId, workplace_door: TilePos, role: Role) -> Option<EntityId> {
    // L2 shadow fixes item 13: a corp's exec is not in the labour pool
    // (Zetatech's exec was hired as a Militech Fab Tech and laid off).
    let execs = crate::systems::classes::exec_set(world);
    let (skill_w, rehire) = if crate::systems::wages::on(world) {
        (world.config.economy2.skill_w.max(0.0), world.config.economy2.rehire_bonus)
    } else {
        (0.0, 0)
    };
    world
        .citizens()
        .into_iter()
        .filter(|&id| world.comp::<Brain>(id).is_some_and(|b| !b.emigrating))
        // L1: not the worker who just quit this employer (rehired at midnight, daily).
        .filter(|&id| !crate::systems::life::quit_blocks(world, id, employer))
        .filter(|&id| !world.has::<Job>(id) && !world.has::<Sentence>(id))
        .filter(|&id| is_adult(world, id))
        .filter(|id| !execs.contains(id))
        .filter(|&id| role != Role::Guard || world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.4))
        .map(|id| {
            // M14 V16: a Lab hires the best hacker (ties lower id),
            // so Labs collect the city's runners.
            if role == Role::Researcher {
                let h = world.comp::<crate::components::Skills>(id).map_or(0.0, |s| s.hacking);
                return (u32::MAX - (h.clamp(0.0, 1.0) * 1_000_000.0) as u32, id);
            }
            // L2 L2: a FightPit hires the best fighter (ties lower id).
            if role == Role::Fighter {
                let f = world.comp::<crate::components::Skills>(id).map_or(0.0, |s| s.fighting);
                return (u32::MAX - (f.clamp(0.0, 1.0) * 1_000_000.0) as u32, id);
            }
            // M16a (plan C8): a Fixer's office hires the best talker
            // (persuasion + knowledge, ties lower id), the Reporter's shape.
            if role == Role::Fixer {
                let k = world.comp::<crate::components::Skills>(id).map_or(0.0, |s| s.persuasion + s.knowledge);
                return (u32::MAX - (k.clamp(0.0, 2.0) * 500_000.0) as u32, id);
            }
            // M15 W36: a Feed hires the most knowledgeable (ties lower id).
            if role == Role::Reporter {
                let k = world.comp::<crate::components::Skills>(id).map_or(0.0, |s| s.knowledge);
                return (u32::MAX - (k.clamp(0.0, 1.0) * 1_000_000.0) as u32, id);
            }
            let from = world
                .comp::<Household>(id)
                .and_then(|h| h.home)
                .and_then(|h| world.comp::<Building>(h))
                .map(|b| b.door)
                .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
                .unwrap_or_default();
            // The Real economy E41: a `CampRaised` applicant comes first for
            // a Farm or Fab vacancy (the distance key halved past the map).
            let first = crate::systems::camp::applicant_first(world, id, role);
            if first {
                return (from.manhattan(workplace_door) / 2, id);
            }
            // J4: skill and the rehire bonus come off the distance, capped
            // at 512 so the key stays above the `CampRaised` band (a halved
            // distance, at most 224 on the 256 x 192 map).
            let skill = if skill_w > 0.0 {
                crate::systems::competence::role_skill(world, id, role).map_or(0.0, |(v, _, _)| v.clamp(0.0, 1.0))
            } else {
                0.0
            };
            let off = (skill_w * 100.0 * skill).round() as u32
                + if rehire > 0 && laid_off_from(world, id, role) { rehire } else { 0 };
            ((1024 + from.manhattan(workplace_door)).saturating_sub(off.min(512)), id)
        })
        .min()
        .map(|(_, id)| id)
}

/// M15 W29: the adult the vacancy at `employer` would hire for `role` now
/// (poaching compares a rival's worker with this one).
pub fn hire_candidate(world: &World, employer: EntityId, role: Role) -> Option<EntityId> {
    let door = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door)?;
    pick_candidate(world, employer, door, role)
}

pub fn hire(world: &mut World, id: EntityId, employer: EntityId, role: Role) {
    let wc = &world.config.world;
    let mut shifts =
        if role == Role::Guard && id.index.is_multiple_of(2) { wc.shift_night.clone() } else { wc.shift_day.clone() };
    // L2 (plan key `[leisure] evening_shift`): the night venues' staff work
    // the evening, so a bout at `bout_hour` has its Fighters on shift.
    if matches!(role, Role::Host | Role::Fighter | Role::Croupier | Role::Concierge) {
        shifts = world.config.leisure.evening_shift.clone();
    }
    let wage_per_day = world.config.wage(role);
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
            premium: 1.0,
            // L2 fix round: unpaid until the first wage (L2 on only).
            paid_once: false,
            duty_fixed: None,
        },
    );
    world.abort_plan(id);
    // J4: a rehire spends the bonus.
    world.laid_off.remove(&id);
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
        if !can_buy_passage(world, id) {
            continue;
        }
        start_emigrating(world, id, "");
    }
}

/// Plan E35 (`[economy2] emigrate_cost`, the jobs round: read only with
/// wages on): leaving costs passage, so an agent holding fewer coins cannot
/// start emigrating (the wallet still crosses out whole at the edge). 0 or
/// wages off: everyone may leave.
pub fn can_buy_passage(world: &World, id: EntityId) -> bool {
    let cost = world.config.economy2.emigrate_cost;
    if cost <= 0 || !crate::systems::wages::on(world) {
        return true;
    }
    world.comp::<Wallet>(id).map_or(0, |w| w.coins) >= cost
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
    // Real economy (plan E24): the wallet crosses out to the World (with
    // the market off it is destroyed with the agent, as before).
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins.max(0));
    world.probe.emigrant_coins += coins;
    if coins > 0 {
        crate::systems::ownership::cross_out(
            world,
            Some(id),
            crate::outside::WORLD_ACCOUNT,
            coins,
            crate::systems::ownership::Flow::Migrant,
            false,
        );
    }
    // L2 L15 (review fix): off the HangOut registry (empty with leisure off).
    crate::systems::leisure::leave_spot(world, id);
    world.remove_agent(id);
}

/// Weekly: `immigration_per_week` newcomers at the map edge; M11 § 7 scales
/// the lever by Street happiness (`classes::immigrants_this_week`). Jobs
/// and room P6 (wages on): the offers first ([`offers_due`], J20), then the
/// Harris-Todaro week ([`pull_week`], J19); `immigration_per_week` stays the
/// wages-off rule.
fn immigration(world: &mut World) {
    if crate::systems::wages::on(world) {
        for (b, role) in offers_due(world) {
            spawn_immigrant_for(world, b, role);
        }
        for _ in 0..pull_week(world) {
            let (_, n) = spawn_migrant(world, None);
            world.stats.current.jobs.migrants_pull += n;
        }
        return;
    }
    let n = crate::systems::classes::immigrants_this_week(world);
    for _ in 0..n {
        spawn_immigrant(world);
    }
}

/// One immigrant: adult of random age, uniform Personality, Skills 0.1,
/// 15 coins, Coarse, at a random edge road; housed in the emptiest Home
/// under 6 residents, else homeless. With wages on, a J21 migrant alone
/// (18-45, the seeding skill draw, the emptiest Block with room).
pub fn spawn_immigrant(world: &mut World) -> EntityId {
    if crate::systems::wages::on(world) {
        let home = emptiest_home_near(world, None, 1);
        return arrive(world, Arrival { home, kind: ArrivalKind::Migrant, sex: None, surname: None, tile: None }, None);
    }
    let home = emptiest_home(world);
    arrive(world, Arrival { home, kind: ArrivalKind::Legacy, sex: None, surname: None, tile: None }, None)
}

/// Who an arrival is: M11's immigrant (any adult age, Skills 0.1) or a
/// Jobs and room J21 migrant (`[demography] migrant_age`, the seeding
/// draw of the three v1 skills on a keyed stream).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArrivalKind {
    Legacy,
    Migrant,
}

/// One arrival's givens: the Home (already chosen), the sex and surname
/// (a spouse's), the tile (an offer's nearest edge road; else a random one).
struct Arrival {
    home: Option<EntityId>,
    kind: ArrivalKind,
    sex: Option<Sex>,
    surname: Option<String>,
    tile: Option<TilePos>,
}

/// J21: the keyed stream of a migrant's own draws (skills, the household's
/// size, a child's age), never stored: the world stream keeps M11's draws.
/// Keys: `salt` (1 skills, 2 household, 3 child), `key` (an entity index,
/// or the day's arrival ordinal) and the tick.
pub const MIGRANT_KEY: u64 = 0x1A16 << 44;

fn migrant_rng(world: &World, salt: u64, key: u64) -> rand_chacha::ChaCha8Rng {
    world.rng.keyed(MIGRANT_KEY ^ (salt << 40) ^ ((key & 0xF_FFFF) << 20) ^ world.tick)
}

/// Spawn one arrival at the map edge. The world-stream draws are M11's
/// (sex unless given, the name, the age, the Personality, the edge tile
/// unless given), in M11's order, so the wages-off city is unchanged.
/// `with`: the partner a spouse arrives with (the event's text).
fn arrive(world: &mut World, a: Arrival, with: Option<EntityId>) -> EntityId {
    let tick = world.tick;
    let sex = match a.sex {
        Some(s) => s,
        None => {
            if world.rng.world().random_bool(0.5) {
                Sex::Male
            } else {
                Sex::Female
            }
        }
    };
    let name = match &a.surname {
        Some(last) => format!("{} {last}", world.random_first_name(sex)),
        None => world.random_name(sex),
    };
    let (age_days, personality, tile) = {
        let edge_roads = world.edge_roads.clone();
        let (lo, hi) = match a.kind {
            ArrivalKind::Legacy => (world.config.world.age_min_years, world.config.world.age_max_years),
            ArrivalKind::Migrant => {
                let [lo, hi] = world.config.demography.migrant_age;
                let lo = lo.max(18.0);
                (lo, hi.max(lo + 0.01))
            }
        };
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
        let tile = match a.tile {
            Some(t) => t,
            None => edge_roads.choose(rng).copied().unwrap_or_default(),
        };
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
    let skills = match a.kind {
        ArrivalKind::Legacy => Skills::basic(0.1, 0.1, 0.1, 0.0),
        ArrivalKind::Migrant => {
            // J21: the seeded residents' draw (`World::spawn_population`).
            let (lo, hi) = (world.config.world.skill_min, world.config.world.skill_max);
            let mut r = migrant_rng(world, 1, u64::from(id.index));
            let mut draw = || if hi > lo { r.random_range(lo..hi) } else { lo };
            let (stealth, fighting, farming) = (draw(), draw(), draw());
            Skills::basic(stealth, fighting, farming, 0.0)
        }
    };
    world.insert(id, skills);
    // M14 V35: an immigrant's keyed draw.
    let hacking = crate::systems::tech::draw_hacking(world, id);
    crate::systems::tech::give_hacking(world, id, hacking);
    // Real economy (plan E24): the 15 coins come from the World with the
    // market on (the wallet is inserted empty and the coins cross in).
    world.insert(id, Wallet { coins: 0 });
    world.probe.immigrant_coins += 15;
    crate::systems::ownership::cross_in(
        world,
        crate::outside::WORLD_ACCOUNT,
        Some(id),
        15,
        crate::systems::ownership::Flow::Migrant,
    );
    // M15 W25: an immigrant's own draw (after the Personality it tilts on).
    let social = crate::systems::moves::seed_skills(world, id);
    crate::systems::moves::give_social(world, id, social);
    world.insert(id, Inventory { food: 0, stolen_food: 0, stims: 0, parts: 0 });
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
            fun: 1.0,
        },
    );
    world.insert(id, Mood::default());
    world.insert(id, Memory::default());
    world.insert(id, Brain { lod: Lod::Coarse, ..Brain::default() });
    world.insert(id, Position { tile, building: None, entered: tick });
    // M13 D4: a keyed Body (the world stream is untouched).
    let body = crate::systems::assets::new_body(world, id);
    crate::systems::assets::give_body(world, id, body);
    let home = a.home;
    world.insert(id, Household::new(home));
    arrival_savings(world, id, home);
    world.stats.current.immigrants += 1;
    let name = world.name_of(id);
    let where_ = match home {
        Some(h) => format!("housed in Block#{}", h.index),
        None => "homeless".to_string(),
    };
    let text = match with {
        Some(p) => format!("{name} arrived with {}, {where_}", world.name_of(p)),
        None => format!("{name} arrived, {where_}"),
    };
    world.push_event(EventKind::Immigration, &[id], text);
    if home.is_none() {
        world.push_event(EventKind::Homeless, &[id], format!("{name} has no home"));
    }
    id
}

// ---------------------------------------------------------------------------
// Jobs and room P6: immigration that answers the city (docs/JOBS_V2.md § 4,
// plan J19-J21). Every arrival is an abstract event: an entity spawned at
// the map edge whose coins cross in from the World account. Read only with
// `[economy2] wages` on.
// ---------------------------------------------------------------------------

/// J19: today's labour market into `EconState.migrant_days` (midnight,
/// after the job search; wages on): the mean gross wage over Job holders,
/// the employed free adults and the free adults (adults not jailed).
fn note_labour_market(world: &mut World) {
    if !crate::systems::wages::on(world) {
        return;
    }
    let gross_mean = crate::systems::wages::gross_mean(world);
    let (mut employed, mut free) = (0u32, 0u32);
    // scan-ok: daily: the Harris-Todaro means
    for id in world.citizens() {
        if !world.has::<Brain>(id) || !is_adult(world, id) || world.has::<Sentence>(id) {
            continue;
        }
        free += 1;
        if world.has::<Job>(id) {
            employed += 1;
        }
    }
    let ring = &mut world.econ.migrant_days;
    if ring.len() >= crate::econ::MIGRANT_DAYS {
        ring.pop_front();
    }
    ring.push_back(crate::econ::MigrantDay { gross_mean, employed, free_adults: free });
}

/// J19: the outside wage `R` (the `SetOutsideWage` pin, else `[demography]
/// outside_wage`), at least 0.1.
pub fn outside_wage(world: &World) -> f32 {
    world.econ.outside_wage_pin.unwrap_or(world.config.demography.outside_wage).max(0.1)
}

/// J19: the expected wage `E = gross_mean × employed ÷ free adults` over the
/// 7-day means (0 before the first midnight's note or with no free adult).
pub fn expected_wage(world: &World) -> f32 {
    let ring = &world.econ.migrant_days;
    if ring.is_empty() {
        return 0.0;
    }
    let n = ring.len() as f32;
    let gross = ring.iter().map(|d| d.gross_mean).sum::<f32>() / n;
    let employed = ring.iter().map(|d| d.employed as f32).sum::<f32>() / n;
    let free = ring.iter().map(|d| d.free_adults as f32).sum::<f32>() / n;
    if free <= 0.0 {
        return 0.0;
    }
    gross * employed / free
}

/// J19: `pull = clamp((E − R) ÷ R, 0, pull_cap)`.
pub fn pull(world: &World) -> f32 {
    let r = outside_wage(world);
    let cap = world.config.demography.pull_cap.max(0.0);
    ((expected_wage(world) - r) / r).clamp(0.0, cap)
}

/// J19: empty beds in standing Blocks (capacity, floors included, minus
/// residents).
pub fn empty_beds(world: &World) -> usize {
    world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| {
            world
                .comp::<Building>(h)
                .filter(|b| !b.demolished && !b.derelict)
                .map(|b| usize::from(b.capacity).saturating_sub(world.residents_of(h).len()))
        })
        .sum()
}

/// J19: the Harris-Todaro week: `round(migrants_max × pull ÷ pull_cap ×
/// beds × immigration_factor)`, `beds = clamp(empty beds ÷ beds_ref, 0.25, 1)`.
pub fn pull_week(world: &World) -> u32 {
    let c = &world.config.demography;
    if c.pull_cap <= 0.0 || c.migrants_max == 0 {
        return 0;
    }
    let p = pull(world) / c.pull_cap;
    if p <= 0.0 {
        return 0;
    }
    let beds = (empty_beds(world) as f32 / c.beds_ref.max(1.0)).clamp(0.25, 1.0);
    let factor = crate::systems::classes::immigration_factor(world);
    (c.migrants_max as f32 * p * beds * factor).round().max(0.0) as u32
}

/// J20: the vacancies an offer answers this week: every open place (a
/// role's slot in `world.vacancies`) at a standing building whose `(building,
/// role)` has stood `offer_days` (`EconState.vacancy_since`, stamped for
/// every vacancy by `wages::daily`), oldest first (ties by building, role),
/// at most `offers_max`.
pub fn offers_due(world: &World) -> Vec<(EntityId, Role)> {
    let c = &world.config.demography;
    let (now, days) = (world.tick, c.offer_days);
    let mut due: Vec<(Tick, EntityId, Role)> = Vec::new();
    for (&b, roles) in &world.vacancies {
        if !world.comp::<Building>(b).is_some_and(|bd| !bd.demolished) {
            continue;
        }
        for &role in roles {
            let Some(&since) = world.econ.vacancy_since.get(&(b, role)) else { continue };
            if since + days * TICKS_PER_DAY <= now {
                due.push((since, b, role));
            }
        }
    }
    due.sort();
    due.into_iter().take(c.offers_max as usize).map(|(_, b, r)| (b, r)).collect()
}

/// J20: a recruited migrant for the open `role` at `b`: hired at spawn
/// (`hire`; the vacancy closes), housed in the Block with room nearest the
/// workplace door ([`emptiest_home_near`]), crossing in at the edge road
/// nearest the workplace; the role's skill floored at `offer_skill` (and a
/// guard lawful, as `pick_candidate` requires). `None` when the place is no
/// longer open or the workplace no longer stands.
pub fn spawn_immigrant_for(world: &mut World, b: EntityId, role: Role) -> Option<EntityId> {
    let door = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.door)?;
    if !world.vacancies.get(&b).is_some_and(|v| v.contains(&role)) {
        return None;
    }
    let (id, n) = spawn_migrant(world, Some(door));
    floor_role_skill(world, id, role);
    hire(world, id, b, role);
    crate::systems::budget::note_hire(world, id, b, role);
    if let Some(v) = world.vacancies.get_mut(&b) {
        if let Some(i) = v.iter().position(|&r| r == role) {
            v.remove(i);
        }
        let still = v.contains(&role);
        if v.is_empty() {
            world.vacancies.remove(&b);
        }
        if !still {
            world.econ.vacancy_since.remove(&(b, role));
        }
    }
    world.stats.current.jobs.migrants_offer += n;
    let (name, place) = (world.name_of(id), world.name_of(b));
    world.push_event(EventKind::Immigration, &[id, b], format!("{name} arrived for a {} job at {place}", role.label()));
    Some(id)
}

/// J21: an offer's role skill at least `offer_skill` (the skill
/// `competence::role_skill` names; a Lab's hacking and a Fixer's knowledge
/// too, their other slots); a guard's lawfulness at least 0.4
/// (`pick_candidate`'s rule).
fn floor_role_skill(world: &mut World, id: EntityId, role: Role) {
    let floor = world.config.demography.offer_skill.clamp(0.0, 1.0);
    let label = crate::systems::competence::role_skill(world, id, role).map(|(_, _, l)| l);
    if let Some(s) = world.comp_mut::<Skills>(id) {
        match label {
            Some("farming") => s.farming = s.farming.max(floor),
            Some("fighting") => s.fighting = s.fighting.max(floor),
            Some("persuasion") => s.persuasion = s.persuasion.max(floor),
            Some("knowledge") => s.knowledge = s.knowledge.max(floor),
            _ => {}
        }
        if role == Role::Researcher {
            s.hacking = s.hacking.max(floor);
        }
        if role == Role::Fixer {
            s.knowledge = s.knowledge.max(floor);
        }
    }
    if role == Role::Guard {
        if let Some(p) = world.comp_mut::<Personality>(id) {
            p.lawfulness = p.lawfulness.max(0.4);
        }
    }
}

/// J19-J21: one migrant household: the migrant (18-45, the seeding skill
/// draw); with `p_spouse` a spouse of the other sex (the same surname and
/// Block, a Spouse edge at 0.6 as seeded couples); with `[demography]
/// migrant_children` > 0 (Dylan's open question; 0 by default) up to that
/// many children. Housed together: nearest `work` (an offer) else the
/// emptiest Block, with room for the household (with no room for the
/// children, the couple comes alone), else homeless; an offer
/// crosses in at the edge road nearest `work`. Returns the migrant and the
/// arrivals counted.
pub fn spawn_migrant(world: &mut World, work: Option<TilePos>) -> (EntityId, u32) {
    // The household's size first, keyed on the day's arrival ordinal and
    // the tick (unique per arrival; the world stream is untouched).
    let c = world.config.demography.clone();
    let mut r = migrant_rng(world, 2, u64::from(world.stats.current.immigrants));
    let spouse = c.p_spouse > 0.0 && r.random_bool(c.p_spouse.min(1.0));
    let mut kids = if spouse && c.migrant_children > 0 { r.random_range(0..=c.migrant_children) } else { 0 };
    let adults = 1 + usize::from(spouse);
    let mut home = emptiest_home_near(world, work, adults + usize::from(kids));
    if home.is_none() && kids > 0 {
        // No Block holds the family: the couple comes without the children.
        kids = 0;
        home = emptiest_home_near(world, work, adults);
    }
    let tile = work.and_then(|w| nearest_edge_road(world, w));
    let id = arrive(world, Arrival { home, kind: ArrivalKind::Migrant, sex: None, surname: None, tile }, None);
    let mut n = 1;
    if spouse {
        let sex = match world.comp::<Identity>(id).map(|i| i.sex) {
            Some(Sex::Male) => Sex::Female,
            _ => Sex::Male,
        };
        let surname = world.comp::<Identity>(id).and_then(|i| i.name.rsplit(' ').next().map(str::to_string));
        let at = world.comp::<Position>(id).map(|p| p.tile);
        let s =
            arrive(world, Arrival { home, kind: ArrivalKind::Migrant, sex: Some(sex), surname, tile: at }, Some(id));
        world.set_spouse(id, s);
        let e = world.edge_entry(id, s);
        e.affinity = 0.6;
        e.trust = 0.6;
        n += 1;
        if let Some(h) = home {
            let (mother, father) = if sex == Sex::Female { (s, id) } else { (id, s) };
            for _ in 0..kids {
                spawn_migrant_child(world, mother, father, h);
                n += 1;
            }
        }
    }
    (id, n)
}

/// Dylan's open question (immigrant families; `[demography]
/// migrant_children`, 0 by default): a child arriving with migrant parents,
/// under 18 (a keyed age), in their Block as a born child is (no
/// occupant's place taken), with Parent and sibling edges as `spawn_child`
/// writes; no coins cross (an empty wallet), no Birth.
fn spawn_migrant_child(world: &mut World, mother: EntityId, father: EntityId, home: EntityId) -> EntityId {
    let tick = world.tick;
    let id = world.spawn();
    let mut r = migrant_rng(world, 3, u64::from(id.index));
    let sex = if r.random_bool(0.5) { Sex::Male } else { Sex::Female };
    let years: f32 = r.random_range(1.0..17.5);
    let age_days = ((years * DAYS_PER_YEAR as f32) as u32).min(ADULT_AGE_DAYS - 1);
    let personality = {
        let m = world.comp::<Personality>(mother).cloned();
        let f = world.comp::<Personality>(father).cloned();
        Personality::inherit(m.as_ref(), f.as_ref(), &mut r)
    };
    let last = world
        .comp::<Identity>(father)
        .or_else(|| world.comp::<Identity>(mother))
        .map(|i| i.name.rsplit(' ').next().unwrap_or("Doe").to_string())
        .unwrap_or_else(|| "Doe".to_string());
    let first = world.random_first_name(sex);
    world.insert(
        id,
        Identity {
            name: format!("{first} {last}"),
            age_days,
            sex,
            born_tick: tick as i64 - i64::from(age_days) * TICKS_PER_DAY as i64,
            spouse_died_tick: None,
        },
    );
    world.insert(id, personality);
    world.insert(id, Skills::basic(0.0, 0.0, 0.0, 0.0));
    let hacking = crate::systems::tech::child_hacking(world, id, mother, father);
    crate::systems::tech::give_hacking(world, id, hacking);
    let social = crate::systems::moves::child_social(world, id, mother, father);
    crate::systems::moves::give_social(world, id, social);
    world.insert(id, Wallet { coins: 0 });
    world.insert(id, Household::new(Some(home)));
    world.insert(id, Child { hunger_days: 0 });
    let body = crate::systems::assets::child_body(world, id, mother, father);
    crate::systems::assets::give_body(world, id, body);
    let door = world.comp::<Building>(home).map_or(TilePos::default(), |b| b.door);
    world.insert(id, Position { tile: door, building: Some(home), entered: tick });
    let siblings: Vec<EntityId> = children_of_agent(world, mother).into_iter().filter(|&o| o != id).collect();
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
    world.stats.current.immigrants += 1;
    let (name, parent) = (world.name_of(id), world.name_of(mother));
    world.push_event(EventKind::Immigration, &[id, mother], format!("{name} arrived with {parent}, a child"));
    id
}

/// J20: the Block with room for `need` (residents + need ≤ capacity,
/// floors included) nearest `near` (door to door; ties fewer residents,
/// then id), or with no `near` the emptiest (fewest residents, then id).
/// Standing, not derelict.
pub fn emptiest_home_near(world: &World, near: Option<TilePos>, need: usize) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter_map(|&h| {
            let b = world.comp::<Building>(h).filter(|b| !b.demolished && !b.derelict)?;
            let n = world.residents_of(h).len();
            (n + need <= usize::from(b.capacity)).then(|| (near.map_or(0, |t| t.manhattan(b.door)), n, h))
        })
        .min()
        .map(|(_, _, h)| h)
}

/// Violence fix 5 (`[life] vf_arrival`): an immigrant arrives with the
/// savings of a seeded adult of its Home's tier, the mean of
/// `[world] initial_coins_min..=max` times `coins_by_tier` (the homeless:
/// times 1, as the seeded homeless keep their draw), not a flat 15. The 15
/// put a newcomer in the emptiest Home (often a Spire block at rent 4) three
/// days from broke: on seed 43 nearly every year-3 immigrant was starving
/// within two weeks of arriving. The top-up crosses in from the World
/// (`Flow::Migrant`) with the market on, as the 15 does; no draw.
fn arrival_savings(world: &mut World, id: EntityId, home: Option<EntityId>) {
    if !crate::systems::fixes::arrival(world) {
        return;
    }
    let wc = &world.config.world;
    let mean = (wc.initial_coins_min + wc.initial_coins_max) as f32 / 2.0;
    let mult = home
        .and_then(|h| world.comp::<crate::components::Building>(h))
        .map_or(1.0, |b| wc.coins_by_tier[usize::from(b.tier.min(2))]);
    let extra = ((mean * mult).round() as i64 - 15).max(0);
    if extra == 0 {
        return;
    }
    world.probe.immigrant_coins += extra;
    crate::systems::ownership::cross_in(
        world,
        crate::outside::WORLD_ACCOUNT,
        Some(id),
        extra,
        crate::systems::ownership::Flow::Migrant,
    );
}

/// The Home with the fewest residents among those under 6 (ties by id).
pub fn emptiest_home(world: &World) -> Option<EntityId> {
    let mut counts: std::collections::BTreeMap<EntityId, usize> = world
        .buildings_by_kind
        .get(&BuildingKind::Home)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| !b.demolished && !b.derelict))
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
