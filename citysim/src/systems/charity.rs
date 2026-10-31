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
    mission_within(world, from, world.config.charity.reach_tiles)
}

/// The nearest standing, stocked, open Mission within `reach` tiles of `from`.
pub fn mission_within(world: &World, from: TilePos, reach: u32) -> Option<EntityId> {
    mission_within_but(world, from, reach, |_| false)
}

/// [`mission_within`], passing over the kitchens `skip` names (round 9 Y1:
/// those a starving body found without a bowl lately).
fn mission_within_but(world: &World, from: TilePos, reach: u32, skip: impl Fn(EntityId) -> bool) -> Option<EntityId> {
    missions(world)
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.stock_food > 0) && !world.is_closed(b) && !skip(b))
        .filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, b)| b)
}

/// The standing Mission nearest `from`, any distance.
pub fn nearest_mission(world: &World, from: TilePos) -> Option<EntityId> {
    missions(world).into_iter().filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b))).min().map(|(_, b)| b)
}

/// Round 5 U3: `m` has a bowl left (a shelf unit, or the open unit's).
fn has_meal(world: &World, m: EntityId) -> bool {
    world
        .comp::<Building>(m)
        .is_some_and(|bd| bd.stock_food > 0 || bd.charity.as_ref().is_some_and(|c| c.synth_frac > 0))
}

/// Round 5 U3: a kitchen as the diary names it, its label over its kind's
/// ("Church of the Open Circuit#2968", "Chapel#2602"): `name_of` read the
/// kind, so one kitchen had two names in one diary.
fn kitchen_name(world: &World, m: EntityId) -> String {
    match world.comp::<Building>(m) {
        Some(bd) => format!("{}#{}", bd.label.as_deref().unwrap_or_else(|| bd.kind.label()), m.index),
        None => world.name_of(m),
    }
}

/// Round 5 U3: below `[life] starving_hunger`.
fn is_starving(world: &World, id: EntityId) -> bool {
    world.comp::<Needs>(id).is_some_and(|n| n.hunger < world.config.life.starving_hunger)
}

/// Round 5 U3 (OPEN-235): the open kitchens `id` reaches from `from`
/// within `[charity] starving_reach_ticks` (`distance::trip`: the walk, or
/// the El for a body that would board it), nearest first; `fed` keeps only
/// those with a bowl left.
fn kitchens_by_trip(world: &World, id: EntityId, from: TilePos, fed: bool) -> Vec<(crate::time::Tick, EntityId)> {
    let cap = world.config.charity.starving_reach_ticks;
    let rides = std::cell::OnceCell::new();
    let mut v: Vec<(crate::time::Tick, EntityId)> = missions(world)
        .into_iter()
        .filter(|&m| {
            !world.is_closed(m) && (!fed || (has_meal(world, m) && !crate::systems::fixes::food_tried(world, id, m)))
        })
        .filter_map(|m| {
            let d = door(world, m)?;
            let rider = || *rides.get_or_init(|| crate::systems::distance::rider(world, id));
            let t = crate::systems::distance::trip_by(world, from, d, rider, Some(world.tick));
            (t <= cap).then_some((t, m))
        })
        .collect();
    v.sort_unstable();
    v
}

// ---------------------------------------------------------------------------
// Seeding and founding (E26, E30)
// ---------------------------------------------------------------------------

/// The Sump districts by Blocks, most first (ties the lower index).
fn sump_districts(world: &World) -> Vec<usize> {
    let mut v: Vec<(std::cmp::Reverse<usize>, usize)> = world
        .districts
        .iter()
        .enumerate()
        .filter(|(_, d)| d.zone == crate::components::Zone::Sump)
        .map(|(i, d)| (std::cmp::Reverse(d.homes.len()), i))
        .collect();
    v.sort_unstable();
    v.into_iter().map(|(_, i)| i).collect()
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

/// Jobs & lives phase 6 (§ 8): a Mission in district `d` (its vacant Lot
/// nearest the centroid, else a derelict refitted, else any district's),
/// deeded to `owner` (a church: the city's).
pub(crate) fn open_mission_in(world: &mut World, d: usize, owner: Option<EntityId>) -> Option<EntityId> {
    let (site, on_lot) = site_for(world, d, true)?;
    open_mission(world, site, on_lot, owner)
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
    let n = world.config.charity.seed as usize;
    let sump = sump_districts(world);
    if sump.is_empty() {
        return built;
    }
    // Scale S4: Mission `j` in the `j`-th Sump district by Blocks (wrapped).
    for j in 0..n {
        let d = sump[j % sump.len()];
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
    let paid = crate::systems::industry::pay_build(world, Some(corp), cost, Some(b), true);
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
            ambient: None,
            parent: None,
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
    let m = near_target(world, id)?;
    Some(alms_steps(world, id, m))
}

/// Round 5 U3 (OPEN-235): the starving broke's long walk, after the near
/// kitchen and the bin (`plan`'s Eat): the nearest kitchen with a bowl left
/// within `starving_reach_ticks`, and a `SoupLine` line saying so.
pub fn far_alms_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let (t, m) = far_target(world, id)?;
    let plan = alms_steps(world, id, m);
    if t > 0 {
        let (who, what) = (world.name_of(id), kitchen_name(world, m));
        let how = crate::systems::distance::hours(t);
        world.push_event(
            EventKind::SoupLine,
            &[id, m],
            format!("{who} set out for the soup line at {what}, {how} away (starving, no bin nearer)"),
        );
    }
    Some(plan)
}

fn alms_steps(world: &World, id: EntityId, m: EntityId) -> Plan {
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let mut steps = Vec::with_capacity(2);
    if here != Some(m) {
        steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(m)));
    }
    steps.push(step(ActionKind::EatAlms, Some(m)));
    Plan { goal: crate::components::GoalKind::Eat, target: Some(m), steps, started_tick: world.tick }
}

/// The Mission that would serve `id` a meal now (Eat's can-obtain gate):
/// the near one ([`near_target`]), else the starving's far one
/// ([`far_target`]).
pub fn alms_target(world: &World, id: EntityId) -> Option<EntityId> {
    near_target(world, id).or_else(|| far_target(world, id).map(|(_, m)| m))
}

/// The meal test both reaches share: a hungry adult who cannot buy a meal
/// (or a Volunteer); `Some(staff Mission)`.
fn alms_eligible(world: &World, id: EntityId) -> Option<Option<EntityId>> {
    if !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let hunger = world.comp::<Needs>(id)?.hunger;
    if hunger >= world.config.charity.hunger_below {
        return None;
    }
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let staff = volunteer_at(world, id);
    // Hunger fix G2 (parity, after P5): with the fix on, "cannot buy" is
    // no stocked Market within its means (`life::can_buy_meal`).
    let can_buy = if crate::systems::fixes::mission_gate(world) {
        crate::systems::life::can_buy_meal(world, id)
    } else {
        coins >= crate::systems::life::meal_price(world, id)
    };
    if staff.is_none() && can_buy {
        return None;
    }
    Some(staff)
}

/// The near Mission: its own for a Volunteer, else the nearest stocked one
/// within `reach_tiles` of its Home door (else tile). Round 5 U3: the
/// starving's reach is [`far_target`]'s (G2's `hungry_reach_mult` x the
/// tiles retired here; the bin keeps it).
pub fn near_target(world: &World, id: EntityId) -> Option<EntityId> {
    match alms_eligible(world, id)? {
        Some(m) if world.comp::<Building>(m).is_some_and(|bd| bd.stock_food > 0) => Some(m),
        // Round 9 Y1 (`OPEN-249`): a kitchen that failed him lately is passed over.
        _ => mission_within_but(world, origin(world, id)?, world.config.charity.reach_tiles, |m| {
            crate::systems::fixes::food_tried(world, id, m)
        }),
    }
}

/// Round 5 U3 (OPEN-235): a starving adult's kitchen past the near reach:
/// the nearest with a bowl left within `starving_reach_ticks` of where he
/// sets out (`life::origin`), and the trip. Hunger fix G2's gate.
pub fn far_target(world: &World, id: EntityId) -> Option<(crate::time::Tick, EntityId)> {
    if !crate::systems::fixes::mission_gate(world) || !is_starving(world, id) {
        return None;
    }
    alms_eligible(world, id)?;
    let from = crate::systems::life::origin(world, id)?;
    kitchens_by_trip(world, id, from, true).into_iter().next()
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
                && world
                    .comp::<Building>(m)
                    .is_some_and(|bd| bd.stock_food > 0 || bd.charity.as_ref().is_some_and(|c| c.synth_frac > 0))
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

/// Round 5 U3 (OPEN-235): the line at the kitchen. A body inside `m` the
/// kitchen would feed (a bowl left, open) when the hour's bowls are served
/// waits for the next hour (`Some(its first tick)`), counted and said.
pub fn queue_until(world: &mut World, id: EntityId, target: Option<EntityId>) -> Option<crate::time::Tick> {
    let m = target.filter(|&m| standing(world, m))?;
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    if here != Some(m) || world.is_closed(m) || !has_meal(world, m) || !line_eligible(world, id, m) {
        return None;
    }
    let hour = world.tick / TICKS_PER_HOUR;
    let served = charity(world, m).map_or(0, |c| if c.hour_key == hour { c.meals_this_hour } else { 0 });
    let cap = hour_cap(world, m);
    if served < cap {
        return None;
    }
    let until = (hour + 1) * TICKS_PER_HOUR;
    ownership::jobs2_row(world).soup_queued += 1;
    let (who, what) = (world.name_of(id), kitchen_name(world, m));
    let next = crate::time::clock(until);
    world.push_event(
        EventKind::SoupLine,
        &[id, m],
        format!("{who} waits in the soup line at {what}: the hour's {cap} bowls are out, the next at {next}"),
    );
    Some(until)
}

/// Round 5 U3 (OPEN-235): the soup line gets into the hall whatever its
/// cots (`[buildings.mission] capacity` is the night's beds): a full
/// kitchen turned its line away at the door (`BuildingFull`), and the
/// starving walked 1-3 h to it and back again (100k shadow, d26-d27).
pub fn capacity_exempt(world: &World, agent: EntityId, b: EntityId) -> bool {
    standing(world, b) && line_eligible(world, agent, b)
}

/// The hungry poor (or the staff) a kitchen's line serves.
fn line_eligible(world: &World, id: EntityId, m: EntityId) -> bool {
    let hungry = world.comp::<Needs>(id).is_some_and(|n| n.hunger < world.config.charity.hunger_below);
    let poor = world.comp::<Wallet>(id).map_or(0, |w| w.coins) < crate::systems::life::meal_price(world, id);
    volunteer_at(world, id) == Some(m) || (hungry && poor)
}

/// Round 5 U3: a body in the line at `m` when the pot is dry is turned
/// away (`true`): counted, the day's tally, a `SoupLine` line.
pub fn turned_away(world: &mut World, id: EntityId, target: Option<EntityId>) -> bool {
    let Some(m) = target.filter(|&m| standing(world, m)) else { return false };
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    if here != Some(m) || has_meal(world, m) || !line_eligible(world, id, m) {
        return false;
    }
    ownership::jobs2_row(world).soup_short += 1;
    if let Some(c) = charity_mut(world, m) {
        c.short_today = c.short_today.saturating_add(1);
    }
    let (who, what) = (world.name_of(id), kitchen_name(world, m));
    world.push_event(EventKind::SoupLine, &[id, m], format!("the pot at {what} ran dry before {who}'s turn"));
    true
}

/// E27: one meal off `m`'s shelf to `id`: `needs::eat`, the counters, the
/// ledger-only `Alms` line at the meal's cost, the district's `d_meals`.
fn serve(world: &mut World, m: EntityId, id: EntityId) -> bool {
    let hour = world.tick / TICKS_PER_HOUR;
    // Hunger as sickness § 1: the Mission serves synth, `synth_stretch`
    // meals a shelf unit (the open unit's first).
    let stretch = if crate::systems::diet::on(world) { crate::systems::diet::stretch(world) } else { 1 };
    let Some(bd) = world.comp_mut::<Building>(m) else { return false };
    let frac = bd.charity.as_ref().map_or(0, |c| c.synth_frac);
    if frac == 0 {
        if bd.stock_food == 0 {
            return false;
        }
        bd.stock_food -= 1;
    }
    if let Some(c) = bd.charity.as_mut() {
        c.synth_frac = (frac + 1) % stretch;
    }
    if let Some(c) = bd.charity.as_mut() {
        c.meals_today = c.meals_today.saturating_add(1);
        if c.hour_key != hour {
            c.hour_key = hour;
            c.meals_this_hour = 0;
        }
        c.meals_this_hour = c.meals_this_hour.saturating_add(1);
    }
    if is_starving(world, id) {
        ownership::jobs2_row(world).soup_starving_fed += 1;
    }
    let cfg = world.config.needs.clone();
    if let Some(n) = world.comp_mut::<Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    crate::systems::health::ate(world, id, crate::components::MealKind::Alms);
    world.mark_day(id, crate::components::trace_flags::ATE);
    world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
    let cost =
        if stretch > 1 { (meal_cost(world) + i64::from(stretch) / 2) / i64::from(stretch) } else { meal_cost(world) };
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
        turned_away(world, id, Some(m));
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

/// E27 (`[charity] soup_hour`, from `living::run`'s hourly pass): the
/// Statistical tier's meals and gifts. Round 5 U3 (OPEN-235): one line for
/// the city, the Statistical hungry poor (adults out of the cells, hunger
/// below `hunger_below`, a meal's coins short), hungriest first then id;
/// each eats at the nearest open kitchen with a bowl left within reach of
/// its Home door (else tile): `reach_tiles`, a starving body's
/// `starving_reach_ticks` by trip. With a kitchen in reach and every pot dry
/// it is turned away (`soup_short`, the kitchen's day); a starving body with
/// none in reach is `soup_far`. Then every Statistical adult who would give
/// today gives to the nearest Mission.
pub fn stat_daily(world: &mut World) {
    let reach = world.config.charity.reach_tiles;
    let below = world.config.charity.hunger_below;
    let starving_h = world.config.life.starving_hunger;
    let stat: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    let kitchens: Vec<(EntityId, TilePos)> = missions(world)
        .into_iter()
        .filter(|&m| !world.is_closed(m))
        .filter_map(|m| door(world, m).map(|d| (m, d)))
        .collect();
    let mut queue: Vec<(ordered_float::OrderedFloat<f32>, EntityId)> = if kitchens.is_empty() {
        Vec::new()
    } else {
        stat.iter()
            .copied()
            .filter(|&id| {
                crate::systems::demography::is_adult(world, id) && !world.has::<crate::components::Sentence>(id)
            })
            .filter_map(|id| {
                let h = world.comp::<Needs>(id)?.hunger;
                if h >= below {
                    return None;
                }
                let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
                (coins < crate::systems::life::meal_price(world, id)).then_some((ordered_float::OrderedFloat(h), id))
            })
            .collect()
    };
    queue.sort_unstable();
    let mut far: Vec<TilePos> = Vec::new();
    for (h, id) in queue {
        let Some(from) = origin(world, id) else { continue };
        let starving = h.0 < starving_h;
        // The kitchens in reach, nearest first (the starving's by trip).
        let in_reach: Vec<EntityId> = if starving {
            kitchens_by_trip(world, id, from, false).into_iter().map(|(_, m)| m).collect()
        } else {
            let mut v: Vec<(u32, EntityId)> =
                kitchens.iter().map(|&(m, d)| (d.manhattan(from), m)).filter(|&(d, _)| d <= reach).collect();
            v.sort_unstable();
            v.into_iter().map(|(_, m)| m).collect()
        };
        let Some(&nearest) = in_reach.first() else {
            if starving {
                ownership::jobs2_row(world).soup_far += 1;
                far.push(from);
            }
            continue;
        };
        match in_reach.iter().copied().find(|&m| has_meal(world, m)) {
            Some(m) => {
                serve(world, m, id);
            }
            None => {
                ownership::jobs2_row(world).soup_short += 1;
                if let Some(c) = charity_mut(world, nearest) {
                    c.short_today = c.short_today.saturating_add(1);
                }
            }
        }
    }
    found_far(world, &far);
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

/// Hourly (from `living::run`): the Statistical pass at `[charity]
/// soup_hour`.
pub fn hourly(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if world.tick_of_day() == world.config.charity.soup_hour.saturating_mul(TICKS_PER_HOUR as u16) {
        stat_daily(world);
    }
}

/// Midnight (from `living::run`): the day's `MissionServed`, the rings,
/// the kitchens' restock from the nearest Market at `wholesale +
/// meal_markup` (`Flow::Wholesale` from the purse), the Dreg column.
pub fn daily(world: &mut World) {
    let cost = meal_cost(world);
    share_purses(world, cost);
    for m in missions(world) {
        let (meals, cots, short) =
            charity(world, m).map_or((0, 0, 0), |c| (c.meals_today, c.cots_today, c.short_today));
        if meals > 0 || cots > 0 || short > 0 {
            let what = kitchen_name(world, m);
            // Round 5 U3: the line the pot did not reach.
            let dry = if short > 0 { format!("; the pot ran dry for {short} more in the line") } else { String::new() };
            world.push_event(
                EventKind::MissionServed,
                &[m],
                format!("{what} served {meals} meals and {cots} cots{dry}"),
            );
        }
        if let Some(c) = charity_mut(world, m) {
            c.served.push_back(c.meals_today);
            while c.served.len() > RING_DAYS {
                c.served.pop_front();
            }
            c.meals_today = 0;
            c.cots_today = 0;
            c.clinic_today = 0;
            c.short_today = 0;
        }
        restock(world, m, cost);
        if world.comp::<Building>(m).is_some_and(|bd| bd.kind == BuildingKind::Mission) {
            staff_daily(world, m, cost);
        }
    }
    refresh_purse_col(world);
}

/// Round 5 U3 (OPEN-235), after the soup hour's line: a soup line where
/// the starving are out of every kitchen's reach. `far` holds where each
/// such off-screen starving body set out from, hungriest first. The first
/// of them with `found_far_min` or more of the others within a walk of
/// `starving_reach_ticks` anchors a new kitchen: the richest kitchen
/// holding past its `share_keep_days` pays `[corps] found_cost.mission` to
/// the Treasury (`Flow::Found`), a Mission opens on the vacant Lot (else
/// the derelict Block, refitted) nearest him on the city's deed, and it is
/// sent a pot; the bodies it covers drop out and the next anchor is sought,
/// up to `found_far_per_day`. None when no purse can.
fn found_far(world: &mut World, far: &[TilePos]) {
    let cfg = &world.config.charity;
    let (min, per_day) = (cfg.found_far_min as usize, cfg.found_far_per_day);
    if min == 0 || per_day == 0 || far.len() < min {
        return;
    }
    let walk = u32::try_from(world.config.exec.walk_tiles(cfg.starving_reach_ticks)).unwrap_or(u32::MAX);
    let cost = meal_cost(world);
    let pot = i64::from(pot_units(world)) * cost;
    let keep = world.config.charity.share_keep_days * pot;
    let build = world.config.corps.found_cost.mission;
    let mut left: Vec<TilePos> = far.to_vec();
    let mut tried: Vec<TilePos> = Vec::new();
    let mut opened = 0;
    while opened < per_day {
        // The anchor: the hungriest whose cluster holds `min` (the first 64 read).
        let anchor = left
            .iter()
            .copied()
            .filter(|t| !tried.contains(t))
            .take(64)
            .map(|t| (t, left.iter().filter(|o| o.manhattan(t) <= walk).count()))
            .find(|&(_, n)| n >= min);
        let Some((at, n)) = anchor else { return };
        tried.push(at);
        let donor = missions(world)
            .into_iter()
            .filter_map(|m| charity(world, m).map(|c| (c.purse, std::cmp::Reverse(m))))
            .filter(|&(p, _)| p >= keep + build + pot)
            .max()
            .map(|(_, m)| m.0);
        let Some(donor) = donor else { return };
        // The site within half the walk of him (a site farther off covers
        // too few of them).
        let Some((site, on_lot)) = site_near(world, at, walk / 2) else { continue };
        let Some(b) = open_mission(world, site, on_lot, None) else { continue };
        let paid = spend(world, donor, None, build, Flow::Found);
        let seed = pot.min(charity(world, donor).map_or(0, |c| c.purse));
        if let Some(c) = charity_mut(world, donor) {
            c.purse -= seed;
        }
        if let Some(c) = charity_mut(world, b) {
            c.purse += seed;
        }
        ownership::ledger_only(world, Flow::Donate, seed);
        refresh_purse_col(world);
        let bd = door(world, b).unwrap_or(at);
        left.retain(|t| t.manhattan(bd) > walk);
        let (who, at_d) = (kitchen_name(world, donor), world.district_name(world.district_of_building(b)).to_string());
        let what = kitchen_name(world, b);
        world.push_event(
            EventKind::Founded,
            &[donor, b],
            format!(
                "{who} paid {paid} to open {what} in {at_d} and sent it {seed}: {n} starving out of every kitchen's reach there"
            ),
        );
        world.stats.current.foundings += 1;
        opened += 1;
    }
}

/// Round 5 U3: the vacant Lot nearest `at` within `within` tiles, else the
/// derelict Block nearest it within them: `(site, on_lot)`.
fn site_near(world: &World, at: TilePos, within: u32) -> Option<(EntityId, bool)> {
    let lot = crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(at), l)))
        .filter(|&(d, _)| d <= within)
        .min()
        .map(|(_, l)| (l, true));
    lot.or_else(|| {
        crate::systems::street::derelicts(world)
            .into_iter()
            .filter_map(|x| {
                let bd = world.comp::<Building>(x).filter(|bd| bd.kind == BuildingKind::Home && !bd.demolished)?;
                Some((bd.door.manhattan(at), x))
            })
            .filter(|&(d, _)| d <= within)
            .min()
            .map(|(_, x)| (x, false))
    })
}

/// Round 5 U3: the shelf units of one day's pot (`servings_per_day` at
/// the diet's stretch, under `stock_cap`).
fn pot_units(world: &World) -> u32 {
    let stretch = if crate::systems::diet::on(world) { crate::systems::diet::stretch(world).max(1) } else { 1 };
    let pot = world.config.charity.servings_per_day.div_ceil(stretch);
    world.config.buildings.for_kind(BuildingKind::Mission).stock_cap.min(pot)
}

/// Round 5 U3 (OPEN-235), midnight before the restock: the kitchens share
/// a purse. A kitchen whose line was turned away today (`short_today`) and
/// whose purse is short of a pot is sent the difference by the kitchens
/// holding more than `share_keep_days` of their own pots, the richest
/// first; the most turned away are sent to first (ties the lower id). A
/// purse-to-purse move (`total_coins` unchanged; the ledger's `Donate`
/// line) and a `Donated` line. At 100k every tithe of a creed lands in its
/// first church (`faith::tithe`): one kitchen sat on the city's charity and
/// cooked one pot a day of it while the others ran dry.
fn share_purses(world: &mut World, cost: i64) {
    let pot = i64::from(pot_units(world)) * cost;
    let keep = world.config.charity.share_keep_days * pot;
    if pot <= 0 {
        return;
    }
    let all = missions(world);
    let purse_of = |world: &World, m: EntityId| charity(world, m).map_or(0, |c| c.purse);
    let mut needy: Vec<(std::cmp::Reverse<u32>, EntityId)> = all
        .iter()
        .copied()
        .filter_map(|m| charity(world, m).map(|c| (c.short_today, c.purse, m)))
        .filter(|&(short, purse, _)| short > 0 && purse < pot)
        .map(|(short, _, m)| (std::cmp::Reverse(short), m))
        .collect();
    if needy.is_empty() {
        return;
    }
    needy.sort_unstable();
    let mut donors: Vec<(std::cmp::Reverse<i64>, EntityId)> = all
        .iter()
        .copied()
        .map(|m| (purse_of(world, m), m))
        .filter(|&(p, _)| p > keep + pot)
        .map(|(p, m)| (std::cmp::Reverse(p), m))
        .collect();
    donors.sort_unstable();
    for (std::cmp::Reverse(short), to) in needy {
        let mut want = pot - purse_of(world, to);
        for &(_, from) in &donors {
            if want <= 0 {
                break;
            }
            let spare = purse_of(world, from) - keep;
            let n = want.min(spare);
            if n <= 0 {
                continue;
            }
            if let Some(c) = charity_mut(world, from) {
                c.purse -= n;
            }
            if let Some(c) = charity_mut(world, to) {
                c.purse += n;
            }
            ownership::ledger_only(world, Flow::Donate, n);
            want -= n;
            let (a, b) = (kitchen_name(world, from), kitchen_name(world, to));
            world.push_event(
                EventKind::Donated,
                &[from, to],
                format!("{a} sent {n} to {b}: its pot ran dry for {short} in the line"),
            );
        }
    }
    refresh_purse_col(world);
}

/// The kitchen buys its pot (Round 5 U3: the units that make `[charity]
/// servings_per_day` bowls at the diet's stretch, under `stock_cap`) as the
/// purse allows from the nearest standing Market with stock.
fn restock(world: &mut World, m: EntityId, cost: i64) {
    let Some(md) = door(world, m) else { return };
    let cap = pot_units(world);
    let (stock, purse) = world
        .comp::<Building>(m)
        .map(|bd| (bd.stock_food, bd.charity.as_ref().map_or(0, |c| c.purse)))
        .unwrap_or((0, 0));
    let mut room = cap.saturating_sub(stock);
    if room == 0 || purse < cost {
        return;
    }
    // Round 5 U3 (OPEN-235): the nearest Markets with stock, nearest first,
    // until the pot is bought (the base took the nearest Market whatever
    // its shelf: at 100k the kitchens sat on 126k coins beside empty ones).
    let mut markets: Vec<(u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .filter_map(|&b| {
            world
                .comp::<Building>(b)
                .filter(|bd| !bd.demolished && !bd.derelict && bd.stock_food > 0)
                .map(|bd| (bd.door.manhattan(md), b))
        })
        .collect();
    markets.sort_unstable();
    for (_, market) in markets {
        let purse = charity(world, m).map_or(0, |c| c.purse);
        let have = world.comp::<Building>(market).map_or(0, |b| b.stock_food);
        let afford = u32::try_from(purse / cost).unwrap_or(u32::MAX);
        let units = room.min(have).min(afford);
        if units == 0 {
            if afford == 0 {
                return;
            }
            continue;
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
        room = room.saturating_sub(bought);
        if room == 0 {
            return;
        }
    }
}
