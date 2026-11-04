use super::*;

/// Hunger fix item 10 (P5h), daily before `relocate`: housing demand at
/// each employer (its workers whose `commute_cost` is past
/// `commute_move_ticks`). Demand of at least `housing_demand_min` with
/// fewer free beds than that within range of the employer's door counts a
/// midnight; any other midnight resets it. After `housing_demand_days`,
/// highest demand first (ties lower id), up to `housing_builds_per_1000` a
/// day per 1,000 residents: a Home on the nearest vacant Lot in range (a
/// plain Lot before a shanty's, whose squatters `founding::convert` puts
/// out), else a floor on the nearest Home in range below `[floors]
/// floors_max`. The employer's owner corp pays `found_cost` / `floor_cost`
/// (Flow::Found, as Grow and AddFloor) when its purse holds it; else the
/// Treasury buys it from the World when it holds the cost above its band
/// floor (`housing_payer`); else nothing goes up today. No RNG; bounded by
/// the Lots and the floor cap.
///
/// The catch-up (`housing_catchup_days` > 0; the flipped city's Farms
/// outgrew two builds a day: 458 far workers on day 120, seed 42): a ready
/// employer's unmet demand is its far workers less the free beds in range;
/// it gets `ceil(unmet / beds a build adds / housing_catchup_days)` builds
/// today (at least one), the city `ceil(sum of unmet / beds / catch-up
/// days)`, floored at `housing_builds_per_1000` and capped at
/// `housing_builds_max_per_1000` per 1,000 residents; and a build keeps the
/// employer's demand count (the next midnight builds again while the beds
/// fill and the far workers remain; free beds in range reset it). Payment
/// is unchanged: no unpaid builds.
pub fn grow_housing(world: &mut World) {
    if !crate::systems::fixes::grow_housing(world) {
        if !world.housing_demand.is_empty() {
            world.housing_demand.clear();
        }
        return;
    }
    let cfg = world.config.life.clone();
    let cap = cfg.commute_move_ticks;
    let demand = housing_far(world);
    let homes = housing_homes(world);
    let grid = homes_grid(world, &homes);
    // (demand, employer, unmet far workers)
    let mut ready: Vec<(std::cmp::Reverse<u32>, EntityId, u32)> = Vec::new();
    let mut next: std::collections::BTreeMap<EntityId, u16> = std::collections::BTreeMap::new();
    for (&employer, &n) in &demand {
        if n < cfg.housing_demand_min {
            continue;
        }
        let Some(work) = world.comp::<Building>(employer).map(|b| b.door) else { continue };
        let free = free_beds_near(world, &homes, &grid, work, cap);
        if free >= cfg.housing_demand_min as usize {
            continue;
        }
        let days = world.housing_demand.get(&employer).copied().unwrap_or(0).saturating_add(1);
        next.insert(employer, days);
        if days >= cfg.housing_demand_days {
            let unmet = n.saturating_sub(u32::try_from(free).unwrap_or(u32::MAX));
            ready.push((std::cmp::Reverse(n), employer, unmet));
        }
    }
    world.housing_demand = next;
    ready.sort_unstable();
    let pop = world.citizens().len() as f32;
    let base = (cfg.housing_builds_per_1000 * pop / 1000.0).round().max(1.0) as usize;
    let catchup = cfg.housing_catchup_days > 0.0;
    let beds = f32::from(world.config.buildings.for_kind(BuildingKind::Home).capacity.max(1));
    // Builds the unmet demand asks for today (the catch-up).
    let want = |unmet: u32| -> usize { (unmet as f32 / beds / cfg.housing_catchup_days).ceil().max(0.0) as usize };
    let limit = if catchup {
        let max = (cfg.housing_builds_max_per_1000 * pop / 1000.0).round().max(base as f32) as usize;
        want(ready.iter().map(|&(_, _, u)| u).sum()).clamp(base, max)
    } else {
        base
    };
    let mut built = 0;
    'employers: for (_, employer, unmet) in ready {
        let mine = if catchup { want(unmet).max(1) } else { 1 };
        for _ in 0..mine {
            if built >= limit {
                break 'employers;
            }
            if !build_housing_near(world, employer) {
                break;
            }
            built += 1;
            if !catchup {
                world.housing_demand.remove(&employer);
            }
        }
    }
}

/// P5h: the housing demand per employer, `grow_housing`'s measure: its
/// workers (not sentenced, not emigrating, not sweepers, housed) whose
/// `commute_cost` from their Home's door to its door is past
/// `commute_move_ticks`.
fn housing_far(world: &World) -> std::collections::BTreeMap<EntityId, u32> {
    let cap = world.config.life.commute_move_ticks;
    let mut demand: std::collections::BTreeMap<EntityId, u32> = std::collections::BTreeMap::new();
    // scan-ok: daily: the housing demand pass
    for a in world.citizens() {
        let Some(employer) = world.comp::<Job>(a).and_then(|j| j.employer) else { continue };
        if world.has::<Sentence>(a) || world.comp::<Brain>(a).is_none_or(|b| b.emigrating) {
            continue;
        }
        // A sweeper's day is its beat, not the employer's door: no demand there.
        if crate::systems::jobs::is_sweeper(world, a) {
            continue;
        }
        let Some(work) = world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door) else { continue };
        let Some(hd) = home_of(world, a).and_then(|h| world.comp::<Building>(h)).map(|b| b.door) else { continue };
        if commute_cost(world, hd, work) > cap {
            *demand.entry(employer).or_default() += 1;
        }
    }
    demand
}

/// P5h: every standing Home's `(door, beds, residents)`.
fn housing_homes(world: &World) -> Vec<(TilePos, usize, usize)> {
    world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        // The island pass phase 3a (I6): a company bed is its staff's only.
        .filter(|&&h| crate::systems::company::open_to_all(world, h))
        .filter_map(|&h| {
            let b = world.comp::<Building>(h).filter(|b| !b.demolished && !b.derelict)?;
            Some((b.door, usize::from(b.capacity), world.residents_of(h).len()))
        })
        .collect()
}

/// P5h: the free beds in `homes` within `cap` (`commute_cost`) of `work`.
/// The perf pass phase 3: `grid` holds `homes`' doors ([`homes_grid`]); the
/// Homes in it over `transit::reach_discs` are a superset of those within
/// `cap`, cut by the measure (a sum: the order is free).
fn free_beds_near(
    world: &World,
    homes: &[(TilePos, usize, usize)],
    grid: &crate::spatial::TileGrid,
    work: TilePos,
    cap: u64,
) -> usize {
    let near: Vec<u32> = if crate::spatial::scans() {
        (0..homes.len() as u32).collect()
    } else {
        grid.near_discs(&crate::systems::transit::reach_discs(world, work, cap))
    };
    near.into_iter()
        .map(|i| homes[i as usize])
        .filter(|&(door, _, _)| commute_cost(world, door, work) <= cap)
        .map(|(_, c, r)| c.saturating_sub(r))
        .sum()
}

/// The perf pass phase 3: [`free_beds_near`]'s grid of `homes`' doors.
fn homes_grid(world: &World, homes: &[(TilePos, usize, usize)]) -> crate::spatial::TileGrid {
    crate::spatial::TileGrid::build(
        world.map.w(),
        world.map.h(),
        crate::spatial::BUILDING_CELL,
        homes.iter().map(|&(door, _, _)| door),
    )
}

/// The pre-roll (the seed city has stood for years: it is not a city that
/// hired 800 people this morning into Homes laid out for 500), at world
/// creation after the staffing: the seeded staff housed near work
/// (`relocate_up_to`, uncapped; hunger fix item 9), then, with `[life]
/// housing_preroll`, items 8-10, wages and no safety net on (the flipped
/// city whose staffing outgrew the map's Homes; the dole city's creation,
/// and the wages-on dole city the unit tests pin to the shipped map, are as
/// before), rounds of: each employer whose far workers (`grow_housing`'s
/// measure) outnumber the free beds within `commute_move_ticks` of its door
/// gets one build (`build_housing_near`'s rule: a plain vacant Lot in range,
/// then a shanty's, then a floor on a Home below `[floors] floors_max`;
/// most unmet first, ties lower id), then the uncapped relocation fills
/// them; until no employer builds (no unmet demand, or no Lot or floor left
/// in range). A pre-roll building is a seed building: it stands unpaid (no
/// coin moves, so `econ::identity` and the escrow are untouched; the
/// opening hoard is already the corps' capital) and silent (no event),
/// owned as a paid build would be by the employer's owner corp (company
/// housing) else the city, with a seed Home's pantry. Then the day's job
/// search fills the places the new Blocks posted (their trades), the
/// relocation runs once more, and every adult housed away from the Block it
/// was dealt into wakes up in its own (the seed city lives where it is
/// housed: a Statistical body left in the old Block never slept at home).
/// No RNG. Returns the builds (Lots, floors).
pub fn preroll_housing(world: &mut World) -> (u32, u32) {
    relocate_up_to(world, usize::MAX);
    let on = world.config.life.housing_preroll
        && crate::systems::fixes::relocate_fix(world)
        && crate::systems::fixes::grow_housing(world)
        && crate::systems::fixes::hire_near(world)
        && crate::systems::wages::on(world);
    let mut built = (0u32, 0u32);
    if !on {
        return built;
    }
    let cap = world.config.life.commute_move_ticks;
    loop {
        let demand = crate::prof::timed!("preroll::housing_far", housing_far(world));
        let homes = housing_homes(world);
        let grid = homes_grid(world, &homes);
        let mut unmet: Vec<(std::cmp::Reverse<usize>, EntityId)> = crate::prof::timed!(
            "preroll::unmet",
            demand
                .iter()
                .filter_map(|(&employer, &n)| {
                    let work = world.comp::<Building>(employer)?.door;
                    let free = free_beds_near(world, &homes, &grid, work, cap);
                    (n as usize > free).then(|| (std::cmp::Reverse(n as usize - free), employer))
                })
                .collect()
        );
        unmet.sort_unstable();
        let mut round = 0;
        for (_, employer) in unmet {
            match crate::prof::timed!("preroll::build_near", build_near(world, employer, true)) {
                Some(true) => built.0 += 1,
                Some(false) => built.1 += 1,
                None => continue,
            }
            round += 1;
        }
        if round == 0 {
            break;
        }
        crate::prof::timed!("preroll::relocate", relocate_up_to(world, usize::MAX));
    }
    if built != (0, 0) {
        crate::systems::demography::job_search(world);
        relocate_up_to(world, usize::MAX);
    }
    // Bodies to their Homes: the creation's movers are still inside the
    // Block they were dealt into.
    let tick = world.tick;
    // scan-ok: once, at world creation
    for a in world.citizens() {
        let Some(to) = home_of(world, a) else { continue };
        let Some(at) = world.comp::<Position>(a).and_then(|p| p.building) else { continue };
        if at == to || world.comp::<Building>(at).is_none_or(|b| b.kind != BuildingKind::Home) {
            continue;
        }
        let Some(door) = world.comp::<Building>(to).map(|b| b.door) else { continue };
        world.remove_from_building(a);
        if let Some(p) = world.comp_mut::<Position>(a) {
            p.tile = door;
            p.building = None;
            p.floor = 0;
            p.entered = tick;
        }
        world.enter_building(a, to);
    }
    built
}

/// P5h: one Home (or a floor) within `commute_move_ticks` of `employer`'s
/// door, as `grow_housing` says. Returns whether anything went up.
pub fn build_housing_near(world: &mut World, employer: EntityId) -> bool {
    build_near(world, employer, false).is_some()
}

/// `build_housing_near`, paid (`housing_payer`) or, at the pre-roll,
/// unpaid (`seeded`: a seed building, owned by the employer's owner corp,
/// else the city; a Lot's Home opens with `[world] home_pantry_initial`).
/// `Some(true)` a Home on a Lot, `Some(false)` a floor, `None` nothing.
fn build_near(world: &mut World, employer: EntityId, seeded: bool) -> Option<bool> {
    let cap = world.config.life.commute_move_ticks;
    let (work, owner) = world.comp::<Building>(employer).map(|b| (b.door, b.owner))?;
    let corp = owner.filter(|&o| world.has::<Corp>(o));
    let what = world.name_of(employer);
    // A vacant Lot in range: plain Lots first, then shanties; nearest; ties lower id.
    // The perf pass phase 3: the Lots and Homes read are those of the kind's
    // grid over `transit::reach_discs` (a superset of those in range, cut
    // by the measure; a minimum: the order is free).
    let discs = crate::systems::transit::reach_discs(world, work, cap);
    let near = |kind: BuildingKind| -> Vec<EntityId> {
        if crate::spatial::scans() {
            world.buildings_of_kind(kind).to_vec()
        } else {
            world.kind_grid(kind).near_discs(&discs)
        }
    };
    let lot = near(BuildingKind::Lot)
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lot && !bd.demolished))
        .filter_map(|l| {
            let d = commute_cost(world, world.comp::<Building>(l)?.door, work);
            (d <= cap).then(|| (crate::systems::street::is_shanty(world, l), d, l))
        })
        .min()
        .map(|(_, _, l)| l);
    if let Some(lot) = lot {
        let cost = crate::systems::founding::found_cost(world, BuildingKind::Home).unwrap_or(0);
        let by = if seeded { corp } else { housing_payer(world, corp, cost)? };
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Home, by).is_err() {
            return None;
        }
        if seeded {
            // A seed Block: its pantry, no event (it stood before day 0).
            let pantry = world.config.world.home_pantry_initial;
            if let Some(b) = world.comp_mut::<Building>(lot) {
                b.stock_food = pantry;
            }
            return Some(true);
        }
        let paid = pay_housing(world, by, cost);
        let (who, home) = (world.owner_label(by), world.name_of(lot));
        let mut actors: Vec<EntityId> = by.into_iter().collect();
        actors.push(lot);
        world.push_event(
            EventKind::Founded,
            &actors,
            format!("{who} built {home} on a Lot for {paid}: housing for {what}'s far workers"),
        );
        return Some(true);
    }
    // Else a floor on the nearest Home in range below its cap.
    let max = world.config.floors.max_for(BuildingKind::Home);
    let home = near(BuildingKind::Home)
        .into_iter()
        .filter_map(|h| {
            // The island pass phase 3a (I6): a company block is its staff's
            // only; another employer's far workers get no storey on it.
            if !crate::systems::company::open_to_all(world, h) {
                return None;
            }
            let b = world.comp::<Building>(h).filter(|b| !b.demolished && !b.derelict && b.floors.max(1) < max)?;
            let d = commute_cost(world, b.door, work);
            (d <= cap).then_some((d, h))
        })
        .min()
        .map(|(_, h)| h)?;
    if seeded {
        // A seed storey: `add_floor`'s seats, no coin and no event.
        let (floors, capacity) = world.comp::<Building>(home).map(|b| (b.floors.max(1), b.capacity))?;
        let per_floor = capacity / u16::from(floors);
        let new_capacity =
            crate::systems::founding::with_floors(world, BuildingKind::Home, per_floor, floors + 1).max(capacity);
        let b = world.comp_mut::<Building>(home)?;
        b.capacity = new_capacity;
        b.floors = floors + 1;
        b.floor_days = 0;
        return Some(false);
    }
    let cost = crate::systems::founding::floor_cost(world, BuildingKind::Home).unwrap_or(0);
    let by = housing_payer(world, corp, cost)?;
    // A corp's floor is paid inside `add_floor` (Flow::Found); the city's
    // after it stands.
    crate::systems::founding::add_floor(world, home, by).ok()?;
    if by.is_none() {
        pay_housing(world, None, cost);
    }
    Some(false)
}

/// P5h (the no-safety-net doctrine: no unpaid builds): who pays `cost` for
/// housing toward `corp`'s workplace. The owner corp when its purse holds
/// it (`Some(Some(corp))`); else the Treasury (`Some(None)`) when it holds
/// the cost and keeps `[treasury] band[0]` after it (with the Treasury band
/// on; else just the cost), the civic budget's floor; else nobody (`None`:
/// nothing is built today).
pub(crate) fn housing_payer(world: &World, corp: Option<EntityId>, cost: i64) -> Option<Option<EntityId>> {
    if let Some(c) = corp.filter(|&c| world.purse(Some(c)) >= cost) {
        return Some(Some(c));
    }
    let floor = if crate::systems::treasury::on(world) { world.config.treasury.band[0] } else { 0 };
    (world.purse(None) - cost >= floor).then_some(None)
}

/// P5h: the payment for a Home or floor: a corp pays the Treasury
/// `Flow::Found` (Grow's and AddFloor's path); the Treasury buys the build
/// from the World (`ownership::import`, `ImportWhy::Build`), so the coins
/// leave the city as any import's. Returns the coins paid.
pub(crate) fn pay_housing(world: &mut World, by: Option<EntityId>, cost: i64) -> i64 {
    if cost <= 0 {
        return 0;
    }
    match by {
        // Jobs & lives phase 1 (§ 2): a Construction Yard's build when one
        // stands (no site: the Block opens at once), else M11's path.
        Some(c) => crate::systems::industry::pay_build(world, Some(c), cost, None, false),
        // The Treasury hires the city's own Yard before it imports the build.
        None if crate::systems::industry::yard_open(world) => {
            crate::systems::industry::pay_build(world, None, cost, None, false)
        }
        None => crate::systems::ownership::import(world, None, cost, crate::systems::ownership::ImportWhy::Build),
    }
}
