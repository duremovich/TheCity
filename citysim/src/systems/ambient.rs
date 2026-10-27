//! Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier. A map Home's flats
//! beyond its household slots hold households nobody simulates; a flat is a
//! pure function of `(world seed, building, flat index)` and stores nothing.
//! They enter the simulation by materialisation: the labour pool (a vacancy
//! no simulated adult filled), immigration (Ambient flats before the map
//! edge) and, later, a story or the player's circle ([`touch`]). A
//! materialised adult is a Statistical agent like any other; its person is its
//! flat's, its state a same-tier donor's, its savings cross in from the World.
//!
//! No world-stream draw anywhere: the flat's own stream and keyed hashes, so
//! the shipped city (no Ambient flats) is byte-identical and a flat gives the
//! same person whatever was materialised before it.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::ambient::{AmbientDistrict, AmbientHome};
use crate::components::{
    Brain, Building, BuildingKind, Household, Identity, Inventory, Job, Lod, Memory, Mood, Needs, Personality,
    Position, RelKind, Sentence, Sex, Skills, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{DAYS_PER_YEAR, TICKS_PER_DAY};
use crate::world::World;

/// `econ::hash_unit` purposes (distinct from every other caller's).
const H_OCCUPIED: u64 = 0xA3B1_0001;
const H_COUPLE: u64 = 0xA3B1_0002;
const H_STREAM: u64 = 0xA3B1_0003;
const H_DONOR: u64 = 0xA3B1_0004;
const H_PICK: u64 = 0xA3B1_0005;

/// The tier is live: the config allows it and the map has Ambient flats.
pub fn on(world: &World) -> bool {
    world.config.ambient.enabled && !world.ambient.homes.is_empty()
}

/// A flat's key for the keyed hashes (`health::seed_agent_keyed`,
/// `classpay::on_arrival_keyed`): bit 62 set, so never an entity index.
fn flat_key(b: EntityId, k: u32, adult: u32) -> u64 {
    (1u64 << 62) | (u64::from(b.index) << 25) | (u64::from(k) << 1) | u64::from(adult & 1)
}

fn occupied(seed: u64, b: EntityId, k: u32, p: f32) -> bool {
    crate::systems::econ::hash_unit(seed, H_OCCUPIED, u64::from(b.index), u64::from(k)) < p
}

fn couple(seed: u64, b: EntityId, k: u32, p: f32) -> bool {
    crate::systems::econ::hash_unit(seed, H_COUPLE, u64::from(b.index), u64::from(k)) < p
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The flat's own stream (never stored): seeded from `(world seed, building,
/// flat)` alone.
fn flat_rng(seed: u64, b: EntityId, k: u32) -> ChaCha8Rng {
    let x = splitmix64(splitmix64(seed ^ H_STREAM.rotate_left(40)) ^ (u64::from(b.index) << 32 | u64::from(k)));
    ChaCha8Rng::seed_from_u64(x)
}

/// A Home's Ambient flats: its `flats=` beyond its slots (`floors`), else
/// `round(floors x flats_per_slot)`.
fn ambient_flats(world: &World, def: &crate::map::MapBuilding) -> u32 {
    let slots = u32::from(def.floors.max(1));
    match def.flats {
        Some(f) => f.saturating_sub(slots),
        None => (slots as f32 * world.config.ambient.flats_per_slot.max(0.0)).round() as u32,
    }
}

/// `World::new`, last (no RNG): the Ambient Homes from the map, their
/// occupied flats and adults hashed, then the first aggregates and donors.
/// Nothing on a map without Ambient flats.
pub fn seed(world: &mut World) {
    if !world.config.ambient.enabled {
        return;
    }
    // The map's Homes by their rect's corner (a Home the seeding passes
    // built on a Lot has no map Home under it: no Ambient flats).
    let defs: std::collections::BTreeMap<(u16, u16), crate::map::MapBuilding> = world
        .map
        .buildings
        .iter()
        .filter(|d| d.kind == BuildingKind::Home)
        .map(|d| ((d.rect.x, d.rect.y), d.clone()))
        .collect();
    let homes: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Home).to_vec();
    let (seed, occ, cp) = (world.seed(), world.config.ambient.occupancy, world.config.ambient.couple_p);
    let mut out = Vec::new();
    for &b in &homes {
        let Some(def) = world.comp::<Building>(b).and_then(|bd| defs.get(&(bd.rect.x, bd.rect.y))) else { continue };
        let flats = ambient_flats(world, def);
        if flats == 0 {
            continue;
        }
        let (mut occupied_n, mut adults) = (0u32, 0u32);
        for k in 0..flats {
            if occupied(seed, b, k, occ) {
                occupied_n += 1;
                adults += 1 + u32::from(couple(seed, b, k, cp));
            }
        }
        out.push(AmbientHome { building: b, flats, occupied: occupied_n, adults, cursor: 0 });
    }
    world.ambient.homes = out;
    if on(world) {
        daily(world);
    }
}

/// A Home stands with people in it (a derelict or demolished one reads
/// vacant: the people left with the building).
fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// Midnight (`demography::run`, before the job search): the donors by Home
/// tier and the district aggregates. O(adults + Ambient Homes).
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    // The donors: housed, free, staying simulated adults, by Home tier.
    let mut donors: [Vec<EntityId>; 3] = Default::default();
    // scan-ok: daily: the Ambient donors
    for id in world.citizens() {
        if !world.has::<Brain>(id) || world.has::<Sentence>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        if world.comp::<Brain>(id).is_some_and(|b| b.emigrating) {
            continue;
        }
        let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else { continue };
        let Some(bd) = world.comp::<Building>(home).filter(|bd| bd.kind == BuildingKind::Home) else { continue };
        donors[usize::from(bd.tier.min(2))].push(id);
    }
    world.ambient.donors = donors;
    let living: Vec<EntityId> = world
        .ambient
        .materialised
        .iter()
        .copied()
        .filter(|&id| world.is_alive(id) && world.has::<Identity>(id))
        .collect();
    world.ambient.materialised = living;

    // The district aggregates.
    let nd = world.districts.len();
    let mut agg: Vec<AmbientDistrict> = vec![AmbientDistrict::default(); nd];
    for h in &world.ambient.homes {
        if h.occupied == 0 || !standing(world, h.building) {
            continue;
        }
        let d = world.district_of_building(h.building).index();
        if let Some(a) = agg.get_mut(d) {
            a.households += h.occupied;
            a.adults += h.adults;
        }
    }
    let last = world.stats.history.back();
    let meals_per_resident = last.map_or(0.0, |r| {
        let meals = r.health.meals_real + r.health.meals_synth + r.health.meals_dive + r.health.meals_alms;
        meals as f32 / r.population.max(1) as f32
    });
    for (i, a) in agg.iter_mut().enumerate() {
        if a.adults == 0 {
            continue;
        }
        let d = &world.districts[i];
        let adults = a.adults as f32;
        let residents = d.residents.len().max(1) as f32;
        let employed = d.residents.iter().filter(|&&r| world.has::<Job>(r)).count() as f32;
        a.labour = adults * (1.0 - employed / residents).clamp(0.0, 1.0);
        a.food = adults * meals_per_resident;
        let rents: Vec<i64> =
            d.homes.iter().filter_map(|&h| world.comp::<Building>(h)).map(|b| b.rent_per_day).collect();
        let rent = if rents.is_empty() { 0.0 } else { rents.iter().sum::<i64>() as f32 / rents.len() as f32 };
        a.rent = a.households as f32 * rent;
        let crimes: u32 = d.crimes.iter().map(|&c| u32::from(c)).sum();
        a.crime = adults * crimes as f32 / d.crimes.len().max(1) as f32 / residents;
        let street = last.and_then(|r| r.living.street_density.get(i).copied()).unwrap_or(0.0);
        a.street = adults * street / residents;
    }
    world.ambient.districts = agg;
}

/// The density ledger's Ambient input (addendum 13): the district's Ambient
/// street density estimate (adults x the simulated density per resident).
pub fn street_density(world: &World, district: usize) -> f32 {
    world.ambient.districts.get(district).map_or(0.0, |a| a.street)
}

/// The Ambient Home with a household left nearest `door` (by door; ties
/// lower index), standing.
pub fn nearest_home(world: &World, door: TilePos) -> Option<usize> {
    world
        .ambient
        .homes
        .iter()
        .enumerate()
        .filter(|(_, h)| h.occupied > 0 && standing(world, h.building))
        .filter_map(|(i, h)| world.comp::<Building>(h.building).map(|b| (b.door.manhattan(door), i)))
        .min()
        .map(|(_, i)| i)
}

/// An Ambient Home picked by the day's hash `ordinal`, weighted by its adults
/// left (standing Homes only).
pub fn pick_home(world: &World, ordinal: u64) -> Option<usize> {
    let weights: Vec<(usize, u64)> = world
        .ambient
        .homes
        .iter()
        .enumerate()
        .filter(|(_, h)| h.occupied > 0 && standing(world, h.building))
        .map(|(i, h)| (i, u64::from(h.adults)))
        .collect();
    let total: u64 = weights.iter().map(|&(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let u = crate::systems::econ::hash_unit(world.seed(), H_PICK, world.day(), ordinal);
    let mut at = ((u as f64 * total as f64) as u64).min(total - 1);
    for (i, w) in weights {
        if at < w {
            return Some(i);
        }
        at -= w;
    }
    None
}

/// Materialise the next occupied flat of Ambient Home `hi`: its adults,
/// Statistical, living there (the first is the household's head). Empty
/// when the Home has no household left or no longer stands.
pub fn materialise(world: &mut World, hi: usize) -> Vec<EntityId> {
    let Some(home) = world.ambient.homes.get(hi).cloned() else { return Vec::new() };
    let b = home.building;
    if home.occupied == 0 || !standing(world, b) {
        return Vec::new();
    }
    let (seed, occ, cp) = (world.seed(), world.config.ambient.occupancy, world.config.ambient.couple_p);
    let mut k = home.cursor;
    while k < home.flats && !occupied(seed, b, k, occ) {
        k += 1;
    }
    if k >= home.flats {
        // The counts said a household was left: they are hashed from the
        // same flats, so this is unreachable; read the Home as empty.
        let h = &mut world.ambient.homes[hi];
        h.cursor = h.flats;
        h.occupied = 0;
        h.adults = 0;
        return Vec::new();
    }
    let two = couple(seed, b, k, cp);
    {
        let h = &mut world.ambient.homes[hi];
        h.cursor = k + 1;
        h.occupied -= 1;
        h.adults = h.adults.saturating_sub(1 + u32::from(two));
    }
    let mut rng = flat_rng(seed, b, k);
    let last = {
        let names = world.names.last_names();
        if names.is_empty() {
            "Doe".to_string()
        } else {
            names[rng.random_range(0..names.len())].clone()
        }
    };
    let head_sex = if rng.random_bool(0.5) { Sex::Male } else { Sex::Female };
    let mut adults = Vec::new();
    for a in 0..(1 + u32::from(two)) {
        let sex = match (a, head_sex) {
            (0, s) => s,
            (_, Sex::Male) => Sex::Female,
            (_, Sex::Female) => Sex::Male,
        };
        let id = spawn_adult(world, b, k, a, sex, &last, &mut rng);
        adults.push(id);
    }
    // The pressure pass S6 (OPEN-224): a couple brings its children, as
    // the seeded couples have them (keyed on the flat).
    let mut kids: Vec<EntityId> = Vec::new();
    if let [x, y] = adults[..] {
        world.set_spouse(x, y);
        let e = world.edge_entry(x, y);
        e.affinity = 0.6;
        e.trust = 0.6;
        kids = crate::systems::ages::children_for_couple(world, x, y, b, flat_key(b, k, 0));
    }
    for &id in &adults {
        copy_contacts(world, id, b, k, &adults);
        let i = world.ambient.materialised.binary_search(&id).unwrap_or_else(|i| i);
        world.ambient.materialised.insert(i, id);
    }
    world.ambient.total += adults.len() as u64;
    world.ambient.today.materialised += adults.len() as u32;
    let names: Vec<String> = adults.iter().map(|&id| world.name_of(id)).collect();
    let mut actors = adults.clone();
    actors.push(b);
    let with = match kids.len() {
        0 => String::new(),
        1 => " with a child".to_string(),
        n => format!(" with {n} children"),
    };
    world.push_event(
        EventKind::Materialised,
        &actors,
        format!("{} of Block#{} flat {k} entered the simulation{with}", names.join(" and "), b.index),
    );
    adults
}

/// The donor for adult `a` of flat `k`: a simulated adult of the Home's tier
/// (yesterday's list), picked by the flat's hash and the day.
fn donor(world: &World, b: EntityId, k: u32, a: u32, tier: usize) -> Option<EntityId> {
    let list = &world.ambient.donors[tier.min(2)];
    let alive: Vec<EntityId> = list.iter().copied().filter(|&d| world.is_alive(d) && world.has::<Needs>(d)).collect();
    if alive.is_empty() {
        return None;
    }
    let u = crate::systems::econ::hash_unit(world.seed(), H_DONOR, flat_key(b, k, a), world.day());
    Some(alive[((u * alive.len() as f32) as usize).min(alive.len() - 1)])
}

/// One adult of flat `k`: the person from the flat's stream, the state from a
/// donor, the savings from the World.
fn spawn_adult(world: &mut World, b: EntityId, k: u32, a: u32, sex: Sex, last: &str, rng: &mut ChaCha8Rng) -> EntityId {
    let tick = world.tick;
    let key = flat_key(b, k, a);
    let wc = world.config.world.clone();
    // --- the person (the flat's stream, in a fixed order) ---
    let first = {
        let table = world.names.first_names(sex);
        if table.is_empty() {
            "Ambient".to_string()
        } else {
            table[rng.random_range(0..table.len())].clone()
        }
    };
    let years: f32 =
        if wc.age_max_years > wc.age_min_years { rng.random_range(wc.age_min_years..wc.age_max_years) } else { 18.0 };
    let age_days = ((years * DAYS_PER_YEAR as f32) as u32).max(crate::systems::demography::ADULT_AGE_DAYS);
    let personality = Personality::random(rng);
    let mut draw =
        || if wc.skill_max > wc.skill_min { rng.random_range(wc.skill_min..wc.skill_max) } else { wc.skill_min };
    let (stealth, fighting, farming) = (draw(), draw(), draw());
    let u: f32 = rng.random();
    let hacking = world.config.decks.hack_seed_scale * u * u * u;
    let social = crate::systems::moves::draw_social(&world.config.skills, Some(&personality), rng);
    let strength = rng.random_range(0.2f32..0.6);
    let reflex = rng.random_range(0.2f32..0.6);

    // --- the state: a donor of the Home's tier ---
    let (tier, door) = world.comp::<Building>(b).map_or((0, TilePos::default()), |bd| (usize::from(bd.tier), bd.door));
    let donor = donor(world, b, k, a, tier);
    let needs = donor.and_then(|d| world.comp::<Needs>(d).cloned());
    let mood = donor.and_then(|d| world.comp::<Mood>(d)).map_or(0.0, |m| m.value);
    let coins = match donor {
        Some(d) => world.comp::<Wallet>(d).map_or(0, |w| w.coins.max(0)),
        None => {
            let mean = (wc.initial_coins_min + wc.initial_coins_max) as f32 / 2.0;
            (mean * wc.coins_by_tier[tier.min(2)]).round() as i64
        }
    };

    let id = world.spawn();
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
    let mut skills = Skills::basic(stealth, fighting, farming, Skills::unset_hacking());
    skills.hacking = hacking;
    skills.set_social_all(social);
    world.insert(id, skills);
    world.insert(id, Wallet { coins: 0 });
    world.insert(id, Inventory { food: 0, stolen_food: 0, stims: 0, parts: 0, synth: 0 });
    let initial = &wc.needs_initial;
    let mut n = needs.unwrap_or(Needs {
        hunger: initial.hunger,
        energy: initial.energy,
        safety: initial.safety,
        wealth: 0.0,
        belonging: initial.belonging,
        intimacy: initial.intimacy,
        starving_since: None,
        fun: 1.0,
        status: 1.0,
    });
    // A starving donor's clock is the donor's, not this adult's.
    n.starving_since = n.starving_since.map(|_| tick);
    world.insert(id, n);
    world.insert(id, Mood { value: mood, low_since: None, last_computed: tick });
    world.insert(id, Memory::default());
    world.insert(id, Brain { lod: Lod::Statistical, ..Brain::default() });
    crate::systems::assets::give_body(
        world,
        id,
        crate::components::Body {
            strength,
            reflex,
            sanity: 1.0,
            addiction: 0.0,
            last_use: None,
            episode_until: None,
            last_shop: None,
        },
    );
    // A bed for the adult: the flat becomes simulated.
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.capacity = bd.capacity.saturating_add(1);
    }
    world.insert(id, Household::new(Some(b)));
    world.insert(id, Position { tile: door, building: None, entered: tick });
    world.enter_building(id, b);
    crate::systems::classpay::on_arrival_keyed(world, id, Some(b), key);
    if coins > 0 {
        let moved = crate::systems::ownership::cross_in(
            world,
            crate::outside::WORLD_ACCOUNT,
            Some(id),
            coins,
            crate::systems::ownership::Flow::Migrant,
        );
        world.ambient.today.coins_in += moved;
    }
    world.recompute_wealth_for(id);
    crate::systems::health::seed_agent_keyed(world, id, key);
    id
}

/// The donor's contacts (Acquaintance and Friend edges, by affinity, at most
/// `[ambient] edges_max`) copied in kind, affinity and trust onto the
/// residents nearest the adult: its own Home first, then its district's
/// Homes by door distance. The household is skipped.
fn copy_contacts(world: &mut World, id: EntityId, b: EntityId, k: u32, household: &[EntityId]) {
    let a = household.iter().position(|&x| x == id).unwrap_or(0) as u32;
    let tier = world.comp::<Building>(b).map_or(0, |bd| usize::from(bd.tier));
    let Some(d) = donor(world, b, k, a, tier) else { return };
    let mut edges: Vec<(f32, f32, RelKind)> = world
        .neighbours(d)
        .filter_map(|o| world.edge(d, o).map(|e| (e.affinity, e.trust, e.kind)))
        .filter(|&(_, _, kind)| matches!(kind, RelKind::Acquaintance | RelKind::Friend))
        .collect();
    edges.sort_by(|x, y| y.0.total_cmp(&x.0));
    edges.truncate(world.config.ambient.edges_max as usize);
    if edges.is_empty() {
        return;
    }
    let door = world.comp::<Building>(b).map_or(TilePos::default(), |bd| bd.door);
    let district = world.district_of(door).index();
    let mut homes: Vec<(u32, EntityId)> = world
        .districts
        .get(district)
        .map(|dd| dd.homes.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&h| h != b)
        .filter_map(|h| world.comp::<Building>(h).map(|bd| (bd.door.manhattan(door), h)))
        .collect();
    homes.sort_unstable();
    let mut targets: Vec<EntityId> = Vec::new();
    for h in std::iter::once(b).chain(homes.into_iter().map(|(_, h)| h)) {
        for &r in world.residents_of(h) {
            if r != id
                && !household.contains(&r)
                && world.has::<Brain>(r)
                && crate::systems::demography::is_adult(world, r)
                && !targets.contains(&r)
            {
                targets.push(r);
                if targets.len() >= edges.len() {
                    break;
                }
            }
        }
        if targets.len() >= edges.len() {
            break;
        }
    }
    let tick = world.tick;
    for (&(affinity, trust, kind), &t) in edges.iter().zip(targets.iter()) {
        let e = world.edge_entry(id, t);
        e.kind = kind;
        e.affinity = affinity;
        e.trust = trust;
        e.last_interaction = tick;
        world.note_kin(id, t);
    }
}

/// The labour pool's daily cap (`[ambient] hires_per_day` per 2,000
/// residents) not yet spent.
pub fn hires_left(world: &World) -> bool {
    let cap = world.config.per_capita_count(world.config.ambient.hires_per_day as usize);
    (world.ambient.today.hires as usize) < cap
}

/// The labour pool: a household from the Ambient Home nearest the workplace
/// door, its head floored for `role` (as a J21 offer) and hired. `None`
/// without an Ambient household left or past the day's cap.
pub fn hire_from_pool(world: &mut World, employer: EntityId, role: crate::components::Role) -> Option<EntityId> {
    if !on(world) || !hires_left(world) {
        return None;
    }
    let door = world.comp::<Building>(employer).filter(|bd| !bd.demolished).map(|bd| bd.door)?;
    let hi = nearest_home(world, door)?;
    let adults = materialise(world, hi);
    let &head = adults.first()?;
    crate::systems::demography::floor_role_skill(world, head, role);
    crate::systems::demography::hire(world, head, employer, role);
    world.ambient.today.hires += 1;
    Some(head)
}

/// Immigration first (J20 offers): a household from the Ambient Home nearest
/// `work`; `(head, adults)`.
pub fn arrival_near(world: &mut World, work: TilePos) -> Option<(EntityId, u32)> {
    if !on(world) {
        return None;
    }
    let hi = nearest_home(world, work)?;
    let adults = materialise(world, hi);
    let &head = adults.first()?;
    world.ambient.today.arrivals += 1;
    immigrated(world, &adults);
    Some((head, adults.len() as u32))
}

/// Immigration first (a J19 pull-week arrival, the dole city's weekly
/// immigrant): a household from an Ambient Home picked by the day's hash
/// (`ordinal`: the arrival's number this week), weighted by adults.
pub fn arrival_any(world: &mut World, ordinal: u64) -> Option<(EntityId, u32)> {
    if !on(world) {
        return None;
    }
    let hi = pick_home(world, ordinal)?;
    let adults = materialise(world, hi);
    let &head = adults.first()?;
    world.ambient.today.arrivals += 1;
    immigrated(world, &adults);
    Some((head, adults.len() as u32))
}

/// Review finding 6: an Ambient household routed in by immigration is
/// immigration, as an arrival at the map edge (`demography::arrive`): each
/// adult counts in the day's `immigrants` and gets an `Immigration` event in
/// `arrive`'s words (the `Materialised` event stays: the tier's own trace,
/// which the labour pool and a story touch fire too, and those are no
/// immigrants).
fn immigrated(world: &mut World, adults: &[EntityId]) {
    let Some(&head) = adults.first() else { return };
    let home = world.comp::<Household>(head).and_then(|h| h.home);
    let where_ = match home {
        Some(h) => format!("housed in Block#{}", h.index),
        None => "homeless".to_string(),
    };
    let head_name = world.name_of(head);
    for (i, &id) in adults.iter().enumerate() {
        world.stats.current.immigrants += 1;
        let text = if i == 0 {
            format!("{head_name} arrived, {where_}")
        } else {
            format!("{} arrived with {head_name}, {where_}", world.name_of(id))
        };
        world.push_event(EventKind::Immigration, &[id], text);
    }
}

/// The story and player hook (M18's view, a story reaching an unsimulated
/// flat): materialise the next household of `home` if it is an Ambient Home.
/// A stub: nothing calls it yet.
pub fn touch(world: &mut World, home: EntityId) -> Vec<EntityId> {
    if !on(world) {
        return Vec::new();
    }
    match world.ambient.homes.iter().position(|h| h.building == home) {
        Some(hi) => materialise(world, hi),
        None => Vec::new(),
    }
}

/// The day's Ambient CSV group (`systems::stats::snapshot`), and the day's
/// counters reset.
pub fn close_day(world: &mut World) -> Option<crate::stats::AmbientCols> {
    if !on(world) {
        return None;
    }
    let s = &world.ambient;
    let labour: f32 = s.districts.iter().map(|d| d.labour).sum();
    let food: f32 = s.districts.iter().map(|d| d.food).sum();
    let cols = crate::stats::AmbientCols {
        flats: s.flats(),
        households: s.households(),
        adults: s.adults(),
        materialised: s.today.materialised,
        hires: s.today.hires,
        arrivals: s.today.arrivals,
        coins_in: s.today.coins_in,
        living: s.materialised.iter().filter(|&&id| world.is_alive(id)).count() as u32,
        total: s.total,
        labour,
        food,
        food_wanted: s.today.food_wanted,
        food_bought: s.today.food_bought,
        visits_wanted: s.today.visits_wanted,
        visits_made: s.today.visits_made,
        paid_food: s.today.paid_food,
        paid_visits: s.today.paid_visits,
        appetite: crate::systems::outside::ambient_book(world).map_or(0.0, |b| b.appetite),
    };
    // M17 phase 1: the Ambient book's rings take the day.
    crate::systems::outside::close_day(world);
    world.ambient.today = Default::default();
    Some(cols)
}
