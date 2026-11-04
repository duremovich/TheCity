use super::*;

// ---------------------------------------------------------------------------
// Emigration and immigration
// ---------------------------------------------------------------------------

/// Daily: mood below the threshold for `emigrate_days` marks the agent as
/// leaving; the executor walks them to the edge and `emigrate` despawns them.
/// M11 § 7: a Corp-class agent never emigrates.
pub(super) fn emigration(world: &mut World) {
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
pub(super) fn immigration(world: &mut World) {
    if crate::systems::wages::on(world) {
        for (b, role) in offers_due(world) {
            spawn_immigrant_for(world, b, role);
        }
        for i in 0..pull_week(world) {
            // Scale S5: an Ambient household before the map edge.
            let (_, n) = match crate::systems::ambient::arrival_any(world, u64::from(i)) {
                Some(x) => x,
                None => spawn_migrant(world, None),
            };
            world.stats.current.jobs.migrants_pull += n;
        }
        return;
    }
    let n = crate::systems::classes::immigrants_this_week(world);
    for i in 0..n {
        // Scale S5: an Ambient household before the map edge.
        if crate::systems::ambient::arrival_any(world, u64::from(i)).is_none() {
            spawn_immigrant(world);
        }
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

/// M18 P5 ("arrive fresh"): one adult arrival at `tile` (else a random edge
/// road), homeless, with an arrival's coins: the start menu's newcomer.
pub fn arrive_at(world: &mut World, tile: Option<TilePos>) -> EntityId {
    let kind = if crate::systems::wages::on(world) { ArrivalKind::Migrant } else { ArrivalKind::Legacy };
    arrive(world, Arrival { home: None, kind, sex: None, surname: None, tile }, None)
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
    world.insert(id, Inventory { food: 0, stolen_food: 0, stims: 0, parts: 0, synth: 0 });
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
    world.insert(id, Brain { lod: Lod::Coarse, ..Brain::default() });
    world.insert(id, Position { tile, building: None, entered: tick, floor: 0 });
    // M13 D4: a keyed Body (the world stream is untouched).
    let body = crate::systems::assets::new_body(world, id);
    crate::systems::assets::give_body(world, id, body);
    let home = a.home;
    world.insert(id, Household::new(home));
    arrival_savings(world, id, home);
    crate::systems::classpay::on_arrival(world, id, home);
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
    // Hunger as sickness (after P5): an arrival carries a history too.
    crate::systems::health::seed_agent(world, id);
    id
}

// ---------------------------------------------------------------------------
// Jobs and room P6: immigration that answers the city (docs/JOBS_V2.md § 4,
// plan J19-J21). Every arrival is an abstract event: an entity spawned at
// the map edge whose coins cross in from the World account. Read only with
// the market wage law.
// ---------------------------------------------------------------------------

/// J19: today's labour market into `EconState.migrant_days` (midnight,
/// after the job search; wages on): the mean gross wage over Job holders,
/// the employed free adults and the free adults (adults not jailed).
pub(super) fn note_labour_market(world: &mut World) {
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
        // The island pass phase 3a (I6): a company bed is its staff's only.
        .filter(|&&h| crate::systems::company::open_to_all(world, h))
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
    // Scale S5: the Ambient household nearest the workplace before the map edge.
    let (id, n) = match crate::systems::ambient::arrival_near(world, door) {
        Some(x) => x,
        None => spawn_migrant(world, Some(door)),
    };
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
pub(crate) fn floor_role_skill(world: &mut World, id: EntityId, role: Role) {
    let floor = world.config.demography.offer_skill.clamp(0.0, 1.0);
    let label = crate::systems::competence::role_skill(world, id, role).map(|(_, _, l)| l);
    if let Some(s) = world.comp_mut::<Skills>(id) {
        match label {
            Some("farming") => s.farming = s.farming.max(floor),
            Some("fighting") => s.melee = s.melee.max(floor),
            Some("persuasion") => s.persuasion = s.persuasion.max(floor),
            Some("knowledge") => s.knowledge = s.knowledge.max(floor),
            // Jobs and room P3: a trade whose row ranks on hacking.
            Some("hacking") => s.hacking = s.hacking.max(floor),
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
    let mut r = migrant_rng(world, 3, u64::from(world.stats.current.immigrants));
    let years: f32 = r.random_range(1.0..17.5);
    let age_days = ((years * DAYS_PER_YEAR as f32) as u32).min(ADULT_AGE_DAYS - 1);
    let id = spawn_child_aged(world, mother, father, home, age_days, &mut r);
    world.stats.current.immigrants += 1;
    let (name, parent) = (world.name_of(id), world.name_of(mother));
    world.push_event(EventKind::Immigration, &[id, mother], format!("{name} arrived with {parent}, a child"));
    id
}

/// A child of `age_days` (under 18) of `mother` and `father` in `home`, as
/// a born child is (no occupant's place taken; Parent and sibling edges as
/// `spawn_child` writes; an empty wallet; no Birth, no event): the sex, the
/// first name and the blended personality from `r` (the world stream is
/// untouched). The migrant's child and the pressure pass S6's seeded
/// children (`ages::seed_children`, OPEN-224).
pub fn spawn_child_aged(
    world: &mut World,
    mother: EntityId,
    father: EntityId,
    home: EntityId,
    age_days: u32,
    r: &mut rand_chacha::ChaCha8Rng,
) -> EntityId {
    let tick = world.tick;
    let id = world.spawn();
    let sex = if r.random_bool(0.5) { Sex::Male } else { Sex::Female };
    let age_days = age_days.min(ADULT_AGE_DAYS - 1);
    let personality = {
        let m = world.comp::<Personality>(mother).cloned();
        let f = world.comp::<Personality>(father).cloned();
        Personality::inherit(m.as_ref(), f.as_ref(), r)
    };
    let last = world
        .comp::<Identity>(father)
        .or_else(|| world.comp::<Identity>(mother))
        .map(|i| i.name.rsplit(' ').next().unwrap_or("Doe").to_string())
        .unwrap_or_else(|| "Doe".to_string());
    let first = {
        let table = world.names.first_names(sex);
        if table.is_empty() {
            "Nameless".to_string()
        } else {
            table[r.random_range(0..table.len())].clone()
        }
    };
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
    world.insert(id, Child { hunger_days: 0, ..Child::default() });
    crate::systems::classpay::on_birth(world, id, mother, father, home);
    let body = crate::systems::assets::child_body(world, id, mother, father);
    crate::systems::assets::give_body(world, id, body);
    let door = world.comp::<Building>(home).map_or(TilePos::default(), |b| b.door);
    world.insert(id, Position { tile: door, building: Some(home), entered: tick, floor: 0 });
    crate::systems::floors::born(world, id, mother);
    let siblings: Vec<EntityId> = children_of_agent(world, mother).into_iter().filter(|&o| o != id).collect();
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
    id
}

/// M17 phase 3: one adult from abroad (an Outsider, a syndicate's
/// brother): `arrive`'s migrant (the World's 15 coins and the arrival
/// savings cross in), alone, at `tile`, housed at `home`. Not counted as an
/// immigrant (the parents' own columns count it).
pub(crate) fn spawn_outsider(world: &mut World, home: Option<EntityId>, tile: Option<TilePos>) -> EntityId {
    let id = arrive(world, Arrival { home, kind: ArrivalKind::Migrant, sex: None, surname: None, tile }, None);
    world.stats.current.immigrants = world.stats.current.immigrants.saturating_sub(1);
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
        // The island pass phase 3a (I6): a company bed is its staff's only.
        .filter(|&&h| crate::systems::company::open_to_all(world, h))
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
        .buildings_by_kind()
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
