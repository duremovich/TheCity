//! The Real economy phase 3b (docs/ECONOMY_V2.md § 4; plan E26-E30, E34):
//! Missions and the Chapel's kitchen, meals and cots from a purse the
//! donors fill, the `Donate` flow from agents, gangs, corps and the god,
//! the `Gave` deed, and the per-district columns a donor can watch.
//!
//! Everything here is coins moved between integer purses (`Charity.purse`
//! through `ownership::charity_in` / `charity_out`), food units on a
//! building and counters: a "meal" is one unit of `stock_food` taken off a
//! Mission and `needs::eat` on the eater; a "cot" is a Hotel booking at
//! price 0; a "donation" is `purse -= n; Charity.purse += n`.

use crate::components::{
    Brain, Building, BuildingKind, Household, Identity, Job, Lod, MemoryEntry, MemoryKind, Needs, Personality,
    Position, Role, TilePos, Wallet,
};
use crate::econ::{Charity, PURPOSE_DONATE, RING_DAYS};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_HOUR;
use crate::word::Deed;
use crate::world::World;

/// A gift of at least this is logged (`Donated`).
const EVENT_MIN: i64 = 5;

// ---------------------------------------------------------------------------
// Missions
// ---------------------------------------------------------------------------

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.charity.is_some() && !bd.demolished && !bd.derelict)
}

/// A standing Mission (the Volunteers' workplace; `exec::resolve_building`).
pub fn is_mission(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Mission) && standing(world, b)
}

/// Every standing building with a kitchen (Missions, then Hideouts with a
/// `Charity`: the Chapel), ascending by id.
pub fn missions(world: &World) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Mission)
        .iter()
        .chain(world.buildings_of_kind(BuildingKind::Hideout))
        .copied()
        .filter(|&b| standing(world, b))
        .collect();
    out.sort_unstable();
    out
}

/// E26: Σ `Charity.purse` (coins held, in `total_coins`).
pub fn purses(world: &World) -> i64 {
    world
        .buildings_of_kind(BuildingKind::Mission)
        .iter()
        .chain(world.buildings_of_kind(BuildingKind::Hideout))
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.charity.as_ref()))
        .map(|c| c.purse)
        .sum()
}

fn charity(world: &World, b: EntityId) -> Option<&Charity> {
    world.comp::<Building>(b).and_then(|bd| bd.charity.as_ref())
}

fn charity_mut(world: &mut World, b: EntityId) -> Option<&mut Charity> {
    world.comp_mut::<Building>(b).and_then(|bd| bd.charity.as_mut())
}

fn door(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

/// Where an agent's walk starts: its Home's door, else its tile.
fn origin(world: &World, id: EntityId) -> Option<TilePos> {
    world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))
}

/// The district a meal or a `Starving` counts in: the agent's Home's door,
/// else its tile's.
fn district_of_agent(world: &World, id: EntityId) -> usize {
    match world.comp::<Household>(id).and_then(|h| h.home) {
        Some(h) => world.district_of_building(h).index(),
        None => world.comp::<Position>(id).map_or(0, |p| world.district_of(p.tile).index()),
    }
}

/// The meal's cost to the kitchen: `[corps] wholesale + meal_markup`.
pub fn meal_cost(world: &World) -> i64 {
    (world.config.corps.wholesale + world.config.charity.meal_markup).max(1)
}

/// The standing Mission with stock nearest `from` within `reach_tiles`
/// (ties the lower id).
pub fn mission_in_reach(world: &World, from: TilePos) -> Option<EntityId> {
    let reach = world.config.charity.reach_tiles;
    missions(world)
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.stock_food > 0) && !world.is_closed(b))
        .filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, b)| b)
}

/// The standing Mission nearest `from`, any distance.
pub fn nearest_mission(world: &World, from: TilePos) -> Option<EntityId> {
    missions(world).into_iter().filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b))).min().map(|(_, b)| b)
}

// ---------------------------------------------------------------------------
// Seeding and founding (E26, E30)
// ---------------------------------------------------------------------------

/// The Sump district with the most Blocks (ties the lower index).
fn sump_district(world: &World) -> Option<usize> {
    world
        .districts
        .iter()
        .enumerate()
        .filter(|(_, d)| d.zone == crate::components::Zone::Sump)
        .map(|(i, d)| (d.homes.len(), std::cmp::Reverse(i)))
        .max()
        .map(|(_, std::cmp::Reverse(i))| i)
}

/// A vacant Lot in district `d` nearest its centroid, else (`refit`) a
/// derelict Block there, else the same in any district: `(site, on_lot)`.
fn site_for(world: &World, d: usize, refit: bool) -> Option<(EntityId, bool)> {
    let centroid = world.districts.get(d).map(|x| x.centroid).unwrap_or_default();
    let lot_in = |only: Option<usize>| {
        crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| only.is_none_or(|d| world.district_of_building(l).index() == d))
            .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(centroid), l)))
            .min()
            .map(|(_, l)| l)
    };
    let derelict_in = |only: Option<usize>| {
        crate::systems::street::derelicts(world)
            .into_iter()
            .filter(|&x| only.is_none_or(|d| world.district_of_building(x).index() == d))
            .filter_map(|x| {
                let bd = world.comp::<Building>(x).filter(|bd| bd.kind == BuildingKind::Home && !bd.demolished)?;
                Some((bd.door.manhattan(centroid), x))
            })
            .min()
            .map(|(_, x)| x)
    };
    if let Some(l) = lot_in(Some(d)) {
        return Some((l, true));
    }
    if refit {
        if let Some(x) = derelict_in(Some(d)) {
            return Some((x, false));
        }
    }
    if let Some(l) = lot_in(None) {
        return Some((l, true));
    }
    if refit {
        if let Some(x) = derelict_in(None) {
            return Some((x, false));
        }
    }
    None
}

/// A Mission stands on `site` (a Lot or a derelict), deeded to the city.
fn open_mission(world: &mut World, site: EntityId, on_lot: bool, owner: Option<EntityId>) -> Option<EntityId> {
    let made = if on_lot {
        crate::systems::founding::build_on_lot(world, site, BuildingKind::Mission, owner)
    } else {
        crate::systems::founding::refit_with(world, site, BuildingKind::Mission, owner, false)
    };
    let b = made.ok()?;
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.charity = Some(Charity::default());
    }
    Some(b)
}

/// E26 (`World::new`, after the Fixers, before the plane links; no RNG):
/// `[charity] seed` Missions, the first in the Sump district with the most
/// Blocks (its vacant Lot nearest the centroid, else a derelict refitted
/// free, else any district's), on the city's deed; and the Chapel (The
/// Unplugged's Hideout) gains a kitchen. Returns the Missions built.
pub fn seed(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    let n = world.config.charity.seed;
    let Some(d) = sump_district(world) else {
        return built;
    };
    for _ in 0..n {
        let Some((site, on_lot)) = site_for(world, d, true) else { break };
        let Some(b) = open_mission(world, site, on_lot, None) else { break };
        let at = world.district_name(world.district_of_building(b)).to_string();
        let how = if on_lot { "Lot" } else { "refit" };
        let text = format!("the city opened a Soup Kitchen in {at} (seeded; {how} {})", b.index);
        world.push_event(EventKind::Founded, &[b], text);
        built.push(b);
    }
    // The Purist Chapel: a kitchen on the creed's Hideout.
    for g in world.gang_list().to_vec() {
        if crate::systems::creeds::is_purist(world, g) {
            if let Some(h) = world.hideout_of(g) {
                if let Some(bd) = world.comp_mut::<Building>(h) {
                    if bd.charity.is_none() {
                        bd.charity = Some(Charity::default());
                    }
                }
            }
        }
    }
    built
}

/// E26: may `agent` choose a Mission at `Register`: lawful (`≥ 0.6`) and
/// not proud (`< 0.5`).
pub fn founder_ok(world: &World, agent: EntityId) -> bool {
    world.comp::<Personality>(agent).is_some_and(|p| p.lawfulness >= 0.6 && p.pride < 0.5)
}

/// `founding::convert`'s hook: a Mission opens with its purse.
pub fn on_convert(world: &mut World, b: EntityId, kind: BuildingKind) {
    if kind == BuildingKind::Mission {
        if let Some(bd) = world.comp_mut::<Building>(b) {
            if bd.charity.is_none() {
                bd.charity = Some(Charity::default());
            }
        }
        // The Volunteer vacancies `convert` posted wait for a funded larder
        // (`staff_daily`): an unpaid Job holder draws no dole, so a
        // Volunteer at an empty kitchen starved (measured: three of seed
        // 42's four starvation deaths in 120 days were Volunteers).
        world.vacancies.remove(&b);
    }
}

/// The larder's value: the purse plus the shelf at the meal's cost.
fn larder(world: &World, m: EntityId, cost: i64) -> i64 {
    world
        .comp::<Building>(m)
        .map_or(0, |bd| bd.charity.as_ref().map_or(0, |c| c.purse) + i64::from(bd.stock_food) * cost)
}

/// Deviation (measured): a Mission posts its `staff` Volunteer vacancies
/// only while the larder covers a week of their staff meals, and lets them
/// go ("the kitchen is empty") when it cannot cover two days of them, so
/// the unpaid post never starves its holder.
fn staff_daily(world: &mut World, m: EntityId, cost: i64) {
    let want = world.config.buildings.for_kind(BuildingKind::Mission).staff as usize;
    if want == 0 {
        return;
    }
    let value = larder(world, m, cost);
    let staff: Vec<EntityId> = ownership::staff_at(world, m)
        .into_iter()
        .filter(|&s| world.comp::<Job>(s).is_some_and(|j| j.role == Role::Volunteer))
        .collect();
    let per_day = cost * want as i64;
    if value < 2 * per_day {
        world.vacancies.remove(&m);
        let what = world.name_of(m);
        for s in staff {
            crate::systems::economy::dismiss(
                world,
                s,
                Some(m),
                format!("{what} let its Volunteer go: the kitchen is empty"),
            );
        }
        return;
    }
    if value >= 7 * per_day {
        let open = world.vacancies.get(&m).map_or(0, |v| v.len());
        let missing = want.saturating_sub(staff.len() + open);
        if missing > 0 {
            world.vacancies.entry(m).or_default().extend(std::iter::repeat_n(Role::Volunteer, missing));
        }
    }
}

/// E30 (`corp_brain::lobby`, once per Lobby term): when a Mission stands in
/// the corp's district (its largest building's) it receives `lobby_gift`;
/// else, with a treasury of `10 × found_cost.mission`, the corp builds one
/// there on the city's deed (`Founded` "(philanthropy)").
pub fn lobby(world: &mut World, corp: EntityId) {
    let Some(largest) = world.comp::<crate::components::Corp>(corp).and_then(|c| {
        c.buildings
            .iter()
            .filter_map(|&b| world.comp::<Building>(b).map(|bd| (u32::from(bd.rect.w) * u32::from(bd.rect.h), b)))
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, b)| b)
    }) else {
        return;
    };
    let d = world.district_of_building(largest).index();
    let here = missions(world).into_iter().find(|&m| world.district_of_building(m).index() == d);
    let gift = world.config.charity.lobby_gift;
    if let Some(m) = here {
        donate(world, Some(corp), m, gift);
        return;
    }
    let cost = world.config.corps.found_cost.mission;
    if world.purse(Some(corp)) < 10 * cost {
        return;
    }
    let Some((site, on_lot)) = site_for(world, d, true) else { return };
    let Some(b) = open_mission(world, site, on_lot, None) else { return };
    let paid = ownership::pay(world, Some(corp), None, cost, Flow::Found);
    let (who, at) = (world.owner_label(Some(corp)), world.district_name(world.district_of_building(b)).to_string());
    world.push_event(
        EventKind::Founded,
        &[corp, b],
        format!("{who} built a Soup Kitchen in {at} for {paid} (philanthropy)"),
    );
    world.stats.current.foundings += 1;
}

// ---------------------------------------------------------------------------
// The purse (E27, E28)
// ---------------------------------------------------------------------------

/// E28: a gift from `from` (an agent, a gang, a corp, or `None` the
/// Treasury) into `mission`'s purse. Capped as `pay`; `donors` counts it;
/// `Donated` at `EVENT_MIN`; the `Gave` deed at `rep_gift_min` for an
/// agent donor. Returns the coins moved.
pub fn donate(world: &mut World, from: Option<EntityId>, mission: EntityId, amount: i64) -> i64 {
    if !standing(world, mission) {
        return 0;
    }
    let moved = ownership::charity_in(world, from, mission, amount);
    if moved <= 0 {
        return 0;
    }
    if let Some(c) = charity_mut(world, mission) {
        *c.donors.entry(from.unwrap_or(EntityId::NONE)).or_default() += moved;
    }
    refresh_purse_col(world);
    if moved >= EVENT_MIN {
        let who = world.owner_label(from);
        let what = world.name_of(mission);
        let mut actors: Vec<EntityId> = from.into_iter().collect();
        actors.push(mission);
        world.push_event(EventKind::Donated, &actors, format!("{who} gave {moved} to {what}"));
    }
    if let Some(a) = from.filter(|&a| world.has::<Identity>(a)) {
        if moved >= world.config.charity.rep_gift_min {
            gave_deed(world, a, mission);
        }
    }
    moved
}

/// E29: the `Gave` deed, first-hand into the donor (M16a's `Hired`
/// precedent) and into the district pool (its honour weight moves the
/// donor's reputation through the M15 path; there is no standing weight at
/// HEAD, so the spec's `rep_gift` is carried by `honour_w.gave`).
fn gave_deed(world: &mut World, donor: EntityId, mission: EntityId) {
    if !world.has::<crate::components::Memory>(donor) {
        return;
    }
    let sal = world.config.gossip.deed_sal.get(Deed::Gave);
    let e = MemoryEntry {
        subject: Some(donor),
        salience: sal,
        valence: sal,
        deed: Some(Deed::Gave),
        object: Some(mission),
        hops: 0,
        conf: 1.0,
        ..MemoryEntry::blank(MemoryKind::Rumour, world.tick)
    };
    crate::systems::memory::hear_entry(world, donor, e);
    let d = world.district_of_building(mission);
    crate::systems::gossip::post_deed(world, d, Deed::Gave, Some(donor), Some(mission));
}

/// E27: the purse pays `to` (`None` the Treasury) `amount` under `flow`
/// (the kitchen's wholesale). Returns the coins moved.
pub fn spend(world: &mut World, mission: EntityId, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let moved = ownership::charity_out(world, mission, to, amount, flow);
    if moved > 0 {
        refresh_purse_col(world);
    }
    moved
}

/// E28, E47: the god's gift is an outside donor's: the World account is
/// created if need be, minted the amount (the identity's `minted` grows with
/// it) and the coins cross in (`outside.inbound`), so
/// `total_coins + Σ outside − minted` holds to the coin.
pub fn god_donate(world: &mut World, mission: EntityId, amount: i64) -> Result<i64, String> {
    if !standing(world, mission) {
        return Err(format!("{} is no Mission", world.name_of(mission)));
    }
    if amount <= 0 {
        return Err("nothing to give".into());
    }
    use crate::outside::{OutsideFaction, OutsideKind, WORLD_ACCOUNT};
    if world.outside.faction(WORLD_ACCOUNT).is_none() {
        let treasury_ref = world.config.export.treasury_ref;
        world.outside.factions.push(OutsideFaction {
            id: WORLD_ACCOUNT,
            name: "the World".to_string(),
            kind: OutsideKind::World,
            treasury: 0,
            treasury_ref,
            base_income: 0,
            base_upkeep: 0,
            income_today: 0,
            market: 1.0,
            dead: false,
            books: Default::default(),
        });
        world.outside.next_id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
    }
    // Minted into the account and crossed in at once: Σ outside is
    // unchanged, `minted` and the purse both grow by `amount`.
    if let Some(f) = world.outside.faction_mut(WORLD_ACCOUNT) {
        f.income_today -= amount;
    }
    world.outside.minted += amount;
    world.outside.inbound += amount;
    ownership::ledger_only(world, Flow::Donate, amount);
    if let Some(c) = charity_mut(world, mission) {
        c.purse += amount;
        *c.donors.entry(EntityId::NONE).or_default() += amount;
    }
    refresh_purse_col(world);
    let what = world.name_of(mission);
    world.push_event(EventKind::Donated, &[mission], format!("the World gave {amount} to {what}"));
    Ok(amount)
}

/// `mission_purse` is a snapshot: refreshed on every purse move.
fn refresh_purse_col(world: &mut World) {
    let p = purses(world);
    world.stats.current.econ.mission_purse = p;
}

// ---------------------------------------------------------------------------
// Who gives (E28)
// ---------------------------------------------------------------------------

/// A donor's floor: `give_floor_days × (rent share per day + a meal)`.
fn give_floor(world: &World, id: EntityId) -> i64 {
    let rent = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h).map(|b| (h, b.rent_per_day)))
        .map(|(h, r)| {
            let adults = world.residents_of(h).iter().filter(|&&a| world.has::<Brain>(a)).count().max(1) as i64;
            r / adults
        })
        .unwrap_or(0);
    world.config.charity.give_floor_days * (rent + crate::systems::life::meal_price(world, id))
}

/// E28: would `id` give today, and how much: coins above the floor, the
/// keyed give test (`hash_unit(seed, PURPOSE_DONATE, day, id)` against
/// `donate_base × (lawfulness + loyalty) ÷ 2 × (1 + creed)`, `creed` 1
/// for a Purist), `donate_frac` of the surplus (at least 1), once a day.
pub fn wants_to_give(world: &World, id: EntityId) -> Option<i64> {
    if !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let day = world.day();
    let brain = world.comp::<Brain>(id)?;
    if brain.last_gave_day == Some(day) || brain.emigrating {
        return None;
    }
    let coins = world.comp::<Wallet>(id)?.coins;
    let floor = give_floor(world, id);
    if coins <= floor {
        return None;
    }
    let cfg = &world.config.charity;
    let p = world.comp::<Personality>(id)?;
    let creed = world.gang_of(id).is_some_and(|g| crate::systems::creeds::is_purist(world, g));
    let prob = cfg.donate_base * (p.lawfulness + p.loyalty) / 2.0 * if creed { 2.0 } else { 1.0 };
    let u = crate::systems::econ::hash_unit(world.seed(), PURPOSE_DONATE, day, u64::from(id.index));
    if u >= prob {
        return None;
    }
    let amount = ((coins - floor) as f32 * cfg.donate_frac).floor() as i64;
    (amount >= 1).then_some(amount)
}

/// E28: the Socialise branch: `[GoTo(Seller = the nearest Mission)] ->
/// Donate` for a body that would give today.
pub fn donate_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    wants_to_give(world, id)?;
    let from = origin(world, id)?;
    let m = nearest_mission(world, from)?;
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let inside = world.comp::<Position>(id).and_then(|p| p.building) == Some(m);
    let mut steps = Vec::with_capacity(2);
    if !inside {
        steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(m)));
    }
    steps.push(step(ActionKind::Donate, Some(m)));
    Some(Plan { goal: crate::components::GoalKind::Socialise, target: Some(m), steps, started_tick: world.tick })
}

/// E28 (a gang's weekly gift, from `gang::daily_economy`): a gang under
/// Expand with an open front gives `gang_gift` to the Mission in its
/// Hideout's district, else the nearest.
pub fn gang_gift(world: &mut World, gang: EntityId) {
    if !world.day().is_multiple_of(7) {
        return;
    }
    let expand =
        world.comp::<crate::components::Gang>(gang).is_some_and(|g| g.order == crate::components::Order::Expand);
    if !expand || crate::systems::leisure::fronts_of(world, gang).is_empty() {
        return;
    }
    let Some(h) = world.hideout_of(gang) else { return };
    let d = world.district_of_building(h).index();
    let here = missions(world).into_iter().find(|&m| world.district_of_building(m).index() == d);
    let m = match here {
        Some(m) => m,
        None => match door(world, h).and_then(|t| nearest_mission(world, t)) {
            Some(m) => m,
            None => return,
        },
    };
    let gift = world.config.charity.gang_gift;
    donate(world, Some(gang), m, gift);
}

// ---------------------------------------------------------------------------
// Meals (E27)
// ---------------------------------------------------------------------------

/// E27: a hungry body (`hunger < hunger_below`) that cannot buy a meal
/// (coins below the local price) with a stocked Mission within reach:
/// `[GoTo(Seller = mission)] -> EatAlms`. A Volunteer at its own Mission
/// eats there on shift whatever its coins (the staff meal).
pub fn alms_plan(world: &World, id: EntityId) -> Option<Plan> {
    if !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let hunger = world.comp::<Needs>(id)?.hunger;
    if hunger >= world.config.charity.hunger_below {
        return None;
    }
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let staff = volunteer_at(world, id);
    if staff.is_none() && coins >= crate::systems::life::meal_price(world, id) {
        return None;
    }
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let m = match staff {
        Some(m) if world.comp::<Building>(m).is_some_and(|bd| bd.stock_food > 0) => m,
        _ => {
            let from = origin(world, id)?;
            mission_in_reach(world, from)?
        }
    };
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let mut steps = Vec::with_capacity(2);
    if here != Some(m) {
        steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(m)));
    }
    steps.push(step(ActionKind::EatAlms, Some(m)));
    Some(Plan { goal: crate::components::GoalKind::Eat, target: Some(m), steps, started_tick: world.tick })
}

/// The Mission `id` volunteers at, if any.
fn volunteer_at(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Job>(id).filter(|j| j.role == Role::Volunteer).and_then(|j| j.employer).filter(|&m| standing(world, m))
}

/// A Volunteer on shift inside `m` doubles the hour's service.
fn volunteer_on_shift(world: &World, m: EntityId) -> bool {
    let tod = world.tick_of_day();
    ownership::staff_at(world, m).into_iter().any(|s| {
        world.comp::<Job>(s).is_some_and(|j| j.role == Role::Volunteer && j.on_shift(tod))
            && world.comp::<Position>(s).and_then(|p| p.building) == Some(m)
    })
}

/// The hour's cap on meals served at `m`.
fn hour_cap(world: &World, m: EntityId) -> u16 {
    let base = world.config.charity.meals_per_hour;
    if volunteer_on_shift(world, m) {
        base.saturating_mul(2)
    } else {
        base
    }
}

/// E27: the scripted steps' start check (through `actions::can_start`).
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    let Some(m) = target.filter(|&m| standing(world, m)) else { return false };
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    match kind {
        ActionKind::EatAlms => {
            let hungry = world.comp::<Needs>(id).is_some_and(|n| n.hunger < world.config.charity.hunger_below);
            let staff = volunteer_at(world, id) == Some(m);
            let poor = world.comp::<Wallet>(id).map_or(0, |w| w.coins) < crate::systems::life::meal_price(world, id);
            let hour = world.tick / TICKS_PER_HOUR;
            let served = charity(world, m).map_or(0, |c| if c.hour_key == hour { c.meals_this_hour } else { 0 });
            here == Some(m)
                && !world.is_closed(m)
                && world.comp::<Building>(m).is_some_and(|bd| bd.stock_food > 0)
                && (staff || (hungry && poor))
                && served < hour_cap(world, m)
        }
        ActionKind::Donate => {
            let at_door = world.comp::<Position>(id).is_some_and(|p| Some(p.tile) == door(world, m));
            (here == Some(m) || at_door) && world.comp::<Wallet>(id).is_some_and(|w| w.coins >= 1)
        }
        _ => false,
    }
}

/// E27: one meal off `m`'s shelf to `id`: `needs::eat`, the counters, the
/// ledger-only `Alms` line at the meal's cost, the district's `d_meals`.
fn serve(world: &mut World, m: EntityId, id: EntityId) -> bool {
    let hour = world.tick / TICKS_PER_HOUR;
    let Some(bd) = world.comp_mut::<Building>(m) else { return false };
    if bd.stock_food == 0 {
        return false;
    }
    bd.stock_food -= 1;
    if let Some(c) = bd.charity.as_mut() {
        c.meals_today = c.meals_today.saturating_add(1);
        if c.hour_key != hour {
            c.hour_key = hour;
            c.meals_this_hour = 0;
        }
        c.meals_this_hour = c.meals_this_hour.saturating_add(1);
    }
    let cfg = world.config.needs.clone();
    if let Some(n) = world.comp_mut::<Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    world.mark_day(id, crate::components::trace_flags::ATE);
    world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
    let cost = meal_cost(world);
    ownership::ledger_only(world, Flow::Alms, cost);
    let d = district_of_agent(world, id);
    let row = &mut world.stats.current.econ;
    row.mission_meals += 1;
    if let Some(n) = row.d_meals.get_mut(d) {
        *n += 1;
    }
    true
}

/// `EatAlms` at completion.
pub fn on_eat_alms(world: &mut World, id: EntityId, target: Option<EntityId>) -> crate::exec::StepResult {
    use crate::exec::{FailReason, StepResult};
    let Some(m) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
    if serve(world, m, id) {
        StepResult::Done
    } else {
        StepResult::Failed(FailReason::StockGone)
    }
}

/// `Donate` at completion: the gift recomputed now, the day marked.
pub fn on_donate(world: &mut World, id: EntityId, target: Option<EntityId>) -> crate::exec::StepResult {
    use crate::exec::{FailReason, StepResult};
    let Some(m) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let day = world.day();
    let Some(amount) = wants_to_give(world, id) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.last_gave_day = Some(day);
    }
    donate(world, Some(id), m, amount);
    StepResult::Done
}

/// E27 (12:00, from `living::run`'s hourly pass): the Statistical tier's
/// meals and gifts. Per Mission ascending, the Statistical adults whose
/// Home door (else tile) is within `reach_tiles` and who pass the meal test,
/// ascending hunger then id, served from the stock; then every Statistical
/// adult who would give today gives to the nearest Mission.
pub fn stat_daily(world: &mut World) {
    let reach = world.config.charity.reach_tiles;
    let below = world.config.charity.hunger_below;
    let stat: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    for m in missions(world) {
        if world.is_closed(m) {
            continue;
        }
        let Some(md) = door(world, m) else { continue };
        let mut queue: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = stat
            .iter()
            .copied()
            .filter(|&id| {
                crate::systems::demography::is_adult(world, id) && !world.has::<crate::components::Sentence>(id)
            })
            .filter(|&id| origin(world, id).is_some_and(|o| o.manhattan(md) <= reach))
            .filter_map(|id| {
                let h = world.comp::<Needs>(id)?.hunger;
                if h >= below {
                    return None;
                }
                let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
                (coins < crate::systems::life::meal_price(world, id)).then_some((ordered_float::OrderedFloat(h), id))
            })
            .collect();
        queue.sort_unstable();
        for (_, id) in queue {
            if world.comp::<Building>(m).is_none_or(|bd| bd.stock_food == 0) {
                break;
            }
            serve(world, m, id);
        }
    }
    // The givers.
    let day = world.day();
    for id in stat {
        let Some(amount) = wants_to_give(world, id) else { continue };
        let Some(from) = origin(world, id) else { continue };
        let Some(m) = nearest_mission(world, from) else { continue };
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.last_gave_day = Some(day);
        }
        donate(world, Some(id), m, amount);
    }
}

// ---------------------------------------------------------------------------
// Cots (E27, through `street::hotel_for`)
// ---------------------------------------------------------------------------

/// E27: a standing Mission whose purse covers a cot is a Hotel at price 0.
pub fn is_cot_house(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Mission)
        && standing(world, b)
        && !world.is_closed(b)
        && charity(world, b).is_some_and(|c| c.purse >= world.config.charity.cot_price)
}

/// A cot booked tonight: the ledger-only `Alms` line at `cot_price` (the
/// purse pays nobody: the cot is the building's capacity), `cots_today`.
pub fn note_cot(world: &mut World, m: EntityId) {
    let price = world.config.charity.cot_price;
    if let Some(c) = charity_mut(world, m) {
        c.cots_today = c.cots_today.saturating_add(1);
    }
    ownership::ledger_only(world, Flow::Alms, price);
    world.stats.current.econ.mission_cots += 1;
}

// ---------------------------------------------------------------------------
// The day (E27, E34)
// ---------------------------------------------------------------------------

/// Hourly (from `living::run`): the Statistical pass at 12:00.
pub fn hourly(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if world.tick_of_day() == 12 * TICKS_PER_HOUR as u16 {
        stat_daily(world);
    }
}

/// Midnight (from `living::run`): the day's `MissionServed`, the rings,
/// the kitchens' restock from the nearest Market at `wholesale +
/// meal_markup` (`Flow::Wholesale` from the purse), the Dreg column.
pub fn daily(world: &mut World) {
    let cost = meal_cost(world);
    for m in missions(world) {
        let (meals, cots) = charity(world, m).map_or((0, 0), |c| (c.meals_today, c.cots_today));
        if meals > 0 || cots > 0 {
            let what = world.name_of(m);
            world.push_event(EventKind::MissionServed, &[m], format!("{what} served {meals} meals and {cots} cots"));
        }
        if let Some(c) = charity_mut(world, m) {
            c.served.push_back(c.meals_today);
            while c.served.len() > RING_DAYS {
                c.served.pop_front();
            }
            c.meals_today = 0;
            c.cots_today = 0;
            c.clinic_today = 0;
        }
        restock(world, m, cost);
        if world.comp::<Building>(m).is_some_and(|bd| bd.kind == BuildingKind::Mission) {
            staff_daily(world, m, cost);
        }
    }
    refresh_purse_col(world);
}

/// The kitchen buys up to `stock_cap` and the purse from the nearest
/// standing Market with stock.
fn restock(world: &mut World, m: EntityId, cost: i64) {
    let Some(md) = door(world, m) else { return };
    let cap = world.config.buildings.for_kind(BuildingKind::Mission).stock_cap;
    let (stock, purse) = world
        .comp::<Building>(m)
        .map(|bd| (bd.stock_food, bd.charity.as_ref().map_or(0, |c| c.purse)))
        .unwrap_or((0, 0));
    let room = cap.saturating_sub(stock);
    if room == 0 || purse < cost {
        return;
    }
    let Some(market) = world.nearest_of_kind(BuildingKind::Market, md) else { return };
    let have = world.comp::<Building>(market).map_or(0, |b| b.stock_food);
    let afford = u32::try_from(purse / cost).unwrap_or(u32::MAX);
    let units = room.min(have).min(afford);
    if units == 0 {
        return;
    }
    let owner = world.owner_of(market);
    let paid = spend(world, m, owner, i64::from(units) * cost, Flow::Wholesale);
    if paid <= 0 {
        return;
    }
    let bought = u32::try_from(paid / cost).unwrap_or(0).min(units);
    world.take_stock(market, crate::components::Good::Food, bought);
    ownership::credit(world, market, paid);
    if let Some(bd) = world.comp_mut::<Building>(m) {
        bd.stock_food += bought;
    }
}
