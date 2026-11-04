use super::*;

// ---------------------------------------------------------------------------
// Ageing and old age
// ---------------------------------------------------------------------------

pub(super) fn ageing(world: &mut World) {
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
    let childhood = world.remove::<Child>(id);
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
            status: 1.0,
        },
    );
    world.insert(id, Mood::default());
    world.insert(id, Memory::default());
    world.insert(id, Inventory { food: 0, stolen_food: 0, stims: 0, parts: 0, synth: 0 });
    world.insert(id, Brain { lod: Lod::Coarse, ..Brain::default() });
    // The Real economy E41: a camp's child is released at its door.
    // Jobs & lives phase 7 (§ 6.1): the childhood's mark (nothing off).
    if let Some(c) = childhood {
        crate::systems::ages::on_mature(world, id, &c);
    }
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

pub(super) fn old_age(world: &mut World) {
    let cfg = world.config.demography.clone();
    let sick_on = crate::systems::health::on(world);
    let mut dying = Vec::new();
    for id in world.citizens() {
        let Some(age) = world.comp::<Identity>(id).map(|i| i.age_days) else { continue };
        // Hunger as sickness § 2: a worn body's hazard (the same one draw).
        let dies = if sick_on && age >= cfg.old_age_days {
            let p = (cfg.old_age_p_per_day * f64::from(crate::systems::health::effects(world, id).old_mult)).min(1.0);
            world.rng.world().random_bool(p)
        } else {
            old_age_roll(world.rng.world(), age, &cfg)
        };
        if dies {
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

/// Each Home feeds its children from the pantry. The pressure pass S6
/// (Dylan 2026-10-27, no child is harmed): an unfed day counts toward the
/// service's take (`camp::take_daily`, `[camp] take_days`), never a death.
pub(super) fn children(world: &mut World) {
    let homes = world.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default();
    let school = world.levers.school_meals;
    let reserve = world.building_of_kind(BuildingKind::Warehouse);
    for home in homes {
        let kids = children_of(world, home);
        if kids.is_empty() {
            continue;
        }
        // Round 6 V1 (OPEN-231): a parent's pocket first (`shelf::pocket_to_shelf`).
        let pocket = crate::systems::shelf::pocket_to_shelf(world, home, &kids);
        if pocket > 0 {
            crate::systems::ownership::jobs2_row(world).camp_pocket += pocket;
        }
        // Round 6 V1 (OPEN-231): in a shared Home each household's children
        // eat its claim on the shelf (`shelf::meal_groups`); a household
        // whose claim is short of their meal goes unfed tonight, whatever
        // the neighbours' groceries on the shelf. The rest eat together.
        let (eat, hungry): (Vec<EntityId>, Vec<EntityId>) = match crate::systems::shelf::meal_groups(world, home, &kids)
        {
            None => (kids.clone(), Vec::new()),
            Some(groups) => {
                let (mut eat, mut hungry, mut ate) = (Vec::new(), Vec::new(), Vec::new());
                for (k, ks, claim) in groups {
                    let need = CHILD_FOOD_PER_DAY * ks.len() as f32;
                    if claim + 1e-3 >= need {
                        ate.push((k, need));
                        eat.extend(ks);
                    } else {
                        hungry.extend(ks);
                    }
                }
                for (k, need) in ate {
                    crate::systems::shelf::eaten(world, home, k, need);
                }
                (eat, hungry)
            }
        };
        for &k in &hungry {
            crate::systems::health::child_day(world, k, false, None);
            if let Some(c) = world.comp_mut::<Child>(k) {
                c.hunger_days = c.hunger_days.saturating_add(1);
            }
        }
        let kids = eat;
        if kids.is_empty() {
            continue;
        }
        let (fed, synth) = {
            let Some(b) = world.comp_mut::<Building>(home) else { continue };
            b.child_food_debt += CHILD_FOOD_PER_DAY * kids.len() as f32;
            let mut fed = true;
            // None: no unit eaten today (the debt below one).
            let mut synth: Option<bool> = None;
            while b.child_food_debt >= 1.0 {
                if b.stock_food == 0 {
                    fed = false;
                    break;
                }
                // Hunger as sickness § 2: real first (0 synth with the diet off).
                let s = crate::systems::health::eat_pantry_unit(b) == crate::components::MealKind::Synth;
                synth = Some(synth.unwrap_or(false) | s);
                b.child_food_debt -= 1.0;
            }
            (fed, synth)
        };
        // M11 phase 5: school meals. A child whose pantry is empty eats from
        // the Reserve (children ate only from the pantry and the dole is per
        // adult: ~70 child starvation deaths in 120 days on seeds 42-44).
        let fed = fed || (school && school_meal(world, home, reserve));
        for &k in &kids {
            crate::systems::health::child_day(world, k, fed, synth);
        }
        for k in kids {
            let Some(c) = world.comp_mut::<Child>(k) else { continue };
            if fed {
                c.hunger_days = 0;
            } else {
                c.hunger_days = c.hunger_days.saturating_add(1);
            }
        }
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
pub(super) fn births(world: &mut World) {
    let p_base = world.config.demography.birth_p_per_day;
    let tick = world.tick;
    let pairs: Vec<(EntityId, EntityId)> = world.spouses.iter().filter(|(a, b)| a < b).map(|(&a, &b)| (a, b)).collect();
    let mut born = Vec::new();
    for (a, b) in pairs {
        let fertile = |id: EntityId| {
            world.comp::<Identity>(id).is_some_and(|i| (ADULT_AGE_DAYS..=FERTILE_UNTIL_DAYS).contains(&i.age_days))
                && world.comp::<Needs>(id).is_some_and(|n| n.intimacy >= 0.6)
                // Hunger as sickness § 2 (true with the diet off).
                && crate::systems::health::effects(world, id).fertile
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
        let mut p = p_base * if happy { 1.5 } else { 1.0 };
        if crate::systems::health::on(world) {
            let (ea, eb) = (crate::systems::health::effects(world, a), crate::systems::health::effects(world, b));
            p *= f64::from(ea.conceive_mult * eb.conceive_mult);
        }
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
    world.insert(id, Child { hunger_days: 0, ..Child::default() });
    crate::systems::classpay::on_birth(world, id, mother, father, home);
    // M13 D4: the parents' mean from the child's keyed stream.
    let body = crate::systems::assets::child_body(world, id, mother, father);
    crate::systems::assets::give_body(world, id, body);
    // In the Home, but not an occupant: a birth may exceed the capacity of 6.
    let door = world.comp::<Building>(home).map_or(TilePos::default(), |b| b.door);
    world.insert(id, Position { tile: door, building: Some(home), entered: tick, floor: 0 });
    crate::systems::floors::born(world, id, mother);
    // Siblings: the parents' other children.
    let mut siblings: Vec<EntityId> = Vec::new();
    for parent in [mother, father] {
        for o in world.kin_candidates(parent) {
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
        world.note_kin(parent, id);
    }
    for s in siblings {
        let e = world.edge_entry(s, id);
        e.kind = RelKind::Family;
        e.affinity = 0.3;
        e.last_interaction = tick;
        world.note_kin(s, id);
    }
    world.stats.current.births += 1;
    let (nm, nf) = (world.name_of(mother), world.name_of(father));
    let name = world.name_of(id);
    world.push_event(EventKind::Birth, &[id, mother, father], format!("{name} born to {nm} and {nf}"));
    // Lives at work phase 2 (§ 3.4, § 3.6): the parents hold the newborn; the Born deed.
    crate::systems::household::on_birth(world, id, mother, father);
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
    // The perf pass phase 6: a child is on a Parent edge, so the kin
    // candidates (`World::kin_candidates`), not every neighbour.
    world
        .kin_candidates(id)
        .filter(|&o| world.has::<Identity>(o) && !world.has::<Corpse>(o) && is_child_of(world, o, id))
        .collect()
}
