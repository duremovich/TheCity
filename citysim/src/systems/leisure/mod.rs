//! Life pass L2 § 2 (plan phase 2): fun and the street. The `fun` need at
//! every tier, the rung ladder (`choice`) behind the `Unwind` goal and its
//! scripted plans, the venues' door policy, the money of `Enjoy`, `Gamble`
//! and `EatOut`, the HangOut spots and registry, street dice, the nightly
//! bout, the Statistical evening (`stat_daily`), the gang fronts, the
//! leader's Collect and Call, the Purist's Preach and the hand-over of the
//! seeded venues. Every entry point is a no-op unless [`on`].
//!
//! Every roll is a keyed `SimRng::word` stream (plan L31: `Gamble`, `Bout`,
//! `Leisure`) or a `splitmix64` hash; nothing here draws on `rng.world()` or
//! `rng.agent()`.

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, Class, Corp, DistrictId, Gang, GangMember, GoalKind, Household, Job,
    Lod, MemoryKind, Needs, Personality, Position, RelKind, Role, Sentence, TilePos, Wallet, Zone,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::living::{Rung, Spot, SpotKind, UnwindPick};
use crate::rng::splitmix64;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{self, DayPhase, Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, gate_or, urgency, Curve};
use crate::utility::Consideration;
use crate::word::WordNs;
use crate::world::World;

mod bouts;
mod choice;
mod evening;
mod fronts;
mod plan;
mod preach;
mod spots;
mod venues;

pub use bouts::*;
pub use choice::*;
pub use evening::*;
pub use fronts::*;
pub use plan::*;
pub use preach::*;
pub use spots::*;
pub use venues::*;

/// Spots kept per district (plan L15).
const MAX_SPOTS: usize = 24;
/// Contacts a spot is scored for (plan L16).
const MAX_CONTACTS: usize = 6;
/// Tiles of a Sump street from a Home door a fire barrel may stand on.
const BARREL_REACH: u32 = 6;

fn coins(world: &World, id: EntityId) -> i64 {
    world.comp::<Wallet>(id).map_or(0, |w| w.coins)
}

/// Spec § 2: coins after `hotel_reserve_meals` meals (the rung ladder's purse).
/// Deviation (the money-loop risk, measured): a week of the agent's rent
/// share and what it owes are kept back too (a renter in arrears spends
/// nothing), and a homeless agent keeps tonight's bed: on seed 42 the meals
/// alone let the leisure evenings take the rent, homelessness held at 63
/// against 23 and starvation tripled (36 against 12).
pub fn spendable(world: &World, id: EntityId) -> i64 {
    let mut keep = world.config.life.hotel_reserve_meals * crate::systems::life::meal_price(world, id);
    match world.comp::<Household>(id) {
        Some(h) if h.home.is_some() => {
            if h.arrears > 0 {
                return 0;
            }
            let home = h.home.and_then(|b| world.comp::<Building>(b));
            let owner = home.and_then(|b| b.owner);
            if owner != Some(id) {
                let adults = h.home.map_or(1, |b| {
                    world
                        .residents_of(b)
                        .iter()
                        .filter(|&&a| Some(a) != owner && crate::systems::demography::is_adult(world, a))
                        .count()
                        .max(1)
                });
                // `rent_tenths_for` is the ledger's tenths (review fix: `rent_per_day`
                // is whole coins and was divided by 10 again).
                let tenths = h.home.map_or(0, |b| ownership::rent_tenths_for(world, b));
                let week = 7.0 * tenths as f32 / 10.0 / adults as f32;
                keep += (h.rent_due.max(0.0) + week).ceil() as i64;
            }
        }
        _ => {
            // The homeless save for a Sump Block's re-housing deposit first
            // (M18 P5h: the deposit every mover pays, `housing::deposit`).
            keep += if crate::systems::housing::on(world) {
                i64::from(world.config.service.move_in_rent_days).max(1) * world.config.rent.base[0].max(1)
            } else {
                world.config.rent.rehouse_coins_mult * world.config.rent.base[0].max(1)
            };
            if crate::systems::street::enabled(world) {
                keep += world.config.street.night_price;
            }
        }
    }
    // An owner's wallet is its business's purse: a week of its staff's
    // wages stays (an agent Bar owner's evenings unpaid its bartenders).
    keep += 7 * owner_payroll(world, Some(id));
    coins(world, id) - keep
}

/// The daily wage bill of everyone employed at `owner`'s staffed buildings.
pub fn owner_payroll(world: &World, owner: Option<EntityId>) -> i64 {
    const STAFFED: [BuildingKind; 4] =
        [BuildingKind::Bar, BuildingKind::Clinic, BuildingKind::Garage, BuildingKind::Hotel];
    // Review fix (perf): one allocation-free pass over the staffed kinds'
    // lists (`owned_of_kind`'s filter, same order); an owner of nothing,
    // nearly every agent, never reaches the staff scan.
    let owned = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| bd.owner == owner && !bd.demolished);
    let mut bill = 0;
    // Jobs & lives phase 4: every venue kind (none of phase 4's stands off).
    for kind in STAFFED.into_iter().chain(BuildingKind::LEISURE_ALL) {
        for b in world.buildings_of_kind(kind).iter().copied().filter(|&b| owned(b)) {
            bill += ownership::staff_at(world, b)
                .into_iter()
                .filter_map(|s| world.comp::<Job>(s).map(|j| j.wage_per_day))
                .sum::<i64>();
        }
    }
    bill
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

fn here(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Position>(id).and_then(|p| p.building)
}

fn kind_of(world: &World, b: EntityId) -> Option<BuildingKind> {
    world.comp::<Building>(b).map(|bd| bd.kind)
}

// ---------------------------------------------------------------------------
// The need (plan L13)
// ---------------------------------------------------------------------------

fn class_index(c: Class) -> usize {
    match c {
        Class::Corp => 0,
        Class::Street => 1,
        Class::Dreg => 2,
    }
}

/// `fun` lost per hour: `fun_decay_per_tick × 60 × (0.6 + 0.4 ×
/// sociability) × class_mult[class]`.
pub fn fun_per_hour(world: &World, id: EntityId, execs: &std::collections::BTreeSet<EntityId>) -> f32 {
    let soc = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
    let class = crate::systems::classes::class_in(world, id, execs);
    let mult = world.config.leisure.class_mult.get(class_index(class)).copied().unwrap_or(1.0);
    world.config.needs.fun_decay_per_tick * TICKS_PER_HOUR as f32 * (0.6 + 0.4 * soc) * mult
}

/// L2 phase 5 (the day-1 leisure pulse): every resident used to open at
/// `fun` 1.0 and decay in step, so the whole city crossed `fun_satisfied`
/// on the same evening (seed 42: 8,125 coins of leisure on day 1, 1.7k on
/// day 2, 4.8k on day 3, the cohort ringing every other day). At seed each
/// adult's `fun` is spread uniformly over `[fun_satisfied, 1]` by a
/// `splitmix64` hash of the seed and its id (no RNG draw); with leisure
/// off nothing changes.
pub fn seed_fun(world: &mut World) {
    let lo = world.config.needs.fun_satisfied.clamp(0.0, 1.0);
    let seed = world.seed();
    // scan-ok: once, at seed
    for id in world.citizens() {
        let h = splitmix64(seed ^ 0x6675_6e5f_7365_6564 ^ u64::from(id.index));
        let u = (h >> 11) as f32 / (1u64 << 53) as f32;
        if let Some(n) = world.comp_mut::<Needs>(id) {
            n.fun = lo + (1.0 - lo) * u;
        }
    }
}

/// Plan L13: `hours` of decay at `rate` per hour (outside `needs::decay`,
/// which stays byte-identical).
pub fn decay_fun(n: &mut Needs, rate: f32, hours: f32) {
    n.fun = (n.fun - rate * hours).max(0.0);
}

/// Plan L13, bodies: an hour of decay for every Full and Coarse agent (from
/// `needs::run`'s hourly branch).
pub fn bodies_hourly(world: &mut World) {
    let execs = crate::systems::classes::exec_set(world);
    for id in world.bodies() {
        if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
            continue;
        }
        let rate = fun_per_hour(world, id, &execs);
        if let Some(n) = world.comp_mut::<Needs>(id) {
            decay_fun(n, rate, 1.0);
        }
    }
}

/// Plan L13, Statistical: the hour's decay (from `lod::run_statistical`).
pub fn stat_hour(world: &mut World, id: EntityId, execs: &std::collections::BTreeSet<EntityId>) {
    let rate = fun_per_hour(world, id, execs);
    if let Some(n) = world.comp_mut::<Needs>(id) {
        decay_fun(n, rate, 1.0);
    }
}

/// Plan L13: mood's `fun_mood × (fun − 0.5)` bias (0 with leisure off).
pub fn mood_bias(world: &World, id: EntityId) -> f32 {
    world.comp::<Needs>(id).map_or(0.0, |n| world.config.needs.fun_mood * (n.fun - 0.5))
}

fn add_fun(world: &mut World, id: EntityId, gain: f32) {
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.fun = (n.fun + gain).min(1.0);
    }
}

fn add_belonging(world: &mut World, id: EntityId, gain: f32) {
    if gain > 0.0 {
        if let Some(n) = world.comp_mut::<Needs>(id) {
            n.belonging = (n.belonging + gain).min(1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// The daily and hourly passes (plan L36)
// ---------------------------------------------------------------------------

/// Midnight: the spots, the wealth decile, stale picks pruned, the hand-over,
/// the fun columns of the closing day.
pub fn daily(world: &mut World) {
    spots_daily(world);
    let mut purses: Vec<i64> =
        world.with::<Wallet>().iter().filter_map(|&a| world.comp::<Wallet>(a)).map(|w| w.coins).collect();
    purses.sort_unstable();
    world.wealth_p90 = purses.get(purses.len() * 9 / 10).copied().unwrap_or(0).max(1);
    let corner = crate::systems::stims::corner_on(world);
    let keep: Vec<EntityId> = world
        .unwind
        .keys()
        .copied()
        .filter(|&id| {
            world.comp::<Brain>(id).and_then(|b| b.plan_goal()).is_some_and(|g| {
                matches!(g, GoalKind::Unwind | GoalKind::Socialise | GoalKind::GangWork)
                        // Lives at work phase 4b: a corner buyer's pick.
                        || (g == GoalKind::GetHigh && corner)
            })
        })
        .collect();
    world.unwind.retain(|id, _| keep.contains(id));
    // Review fix: an expired Call is cleared.
    let now = world.tick;
    for g in world.gangs() {
        if let Some(x) = world.comp_mut::<Gang>(g) {
            if x.call_until.is_some_and(|t| t <= now) {
                x.call_until = None;
            }
        }
    }
    hand_over(world);
}

/// Hourly: the street density sample; 18:00 the off-screen leaders; the
/// bouts at `bout_hour`; 21:00 the Statistical evening.
pub fn hourly(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if world.spots.is_empty() {
        spots_daily(world);
    }
    crate::prof::timed!("leisure::sample_density", sample_density(world));
    let hour = world.tick_of_day() / TICKS_PER_HOUR as u16;
    if hour == 18 {
        crate::prof::timed!("leisure::stat_leaders", stat_leaders(world));
    }
    if hour == world.config.leisure.bout_hour {
        crate::prof::timed!("leisure::bouts", bouts(world));
    }
    if world.tick_of_day() == 1260 {
        crate::prof::timed!("leisure::stat_daily", stat_daily(world));
    }
}

/// `d{i}_street_density`: HangOut and venue occupants per district, the
/// day's hourly mean.
fn sample_density(world: &mut World) {
    let n = crate::systems::stats::csv_slots(world).1;
    let mut counts = vec![0u32; n];
    for (t, v) in &world.hangouts {
        let d = world.district_of(*t).index();
        if let Some(c) = counts.get_mut(d) {
            *c += v.len() as u32;
        }
    }
    let mut kinds = BuildingKind::LEISURE.to_vec();
    kinds.push(BuildingKind::Bar);
    for k in kinds {
        for &b in world.buildings_of_kind(k) {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            let d = world.district_of(bd.door).index();
            if let Some(c) = counts.get_mut(d) {
                *c += bd.occupants.len() as u32;
            }
        }
    }
    let row = &mut world.stats.current.living.street_density;
    if row.len() < n {
        row.resize(n, 0.0);
    }
    for (i, c) in counts.into_iter().enumerate() {
        row[i] += c as f32 / 24.0;
    }
}

/// The day's fun columns (from `systems::stats` at midnight).
pub fn fun_columns(world: &World) -> (f32, f32) {
    let sat = world.config.needs.fun_satisfied;
    let (mut sum, mut n, mut ok) = (0.0f32, 0u32, 0u32);
    for lod in [Lod::Full, Lod::Coarse, Lod::Statistical] {
        for &id in world.tier(lod) {
            if !crate::systems::demography::is_adult(world, id) {
                continue;
            }
            if let Some(nd) = world.comp::<Needs>(id) {
                sum += nd.fun;
                n += 1;
                ok += u32::from(nd.fun >= sat);
            }
        }
    }
    if n == 0 {
        return (0.0, 0.0);
    }
    (sum / n as f32, ok as f32 / n as f32)
}

/// The day of the week (`day % 7`), for the inspector.
pub fn weekday(tick: Tick) -> u64 {
    time::day(tick) % 7
}

/// The pick a body's plan was built for (the inspector).
pub fn pick_of_agent(world: &World, id: EntityId) -> Option<&UnwindPick> {
    world.unwind.get(&id)
}

/// The contacts a HangOut would look for (the inspector).
pub fn expected_contacts(world: &World, id: EntityId) -> Vec<EntityId> {
    contacts(world, id).into_iter().map(|(c, _, _)| c).collect()
}

/// Is `gang` a front's gang? (the building panel)
pub fn front_gang(world: &World, b: EntityId) -> Option<EntityId> {
    world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).and_then(|v| v.front_of)
}

/// Members of `gang` (helper for tests).
pub fn members(world: &World, gang: EntityId) -> Vec<EntityId> {
    world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default()
}

/// A member of any gang (helper).
pub fn in_gang(world: &World, id: EntityId) -> bool {
    world.has::<GangMember>(id)
}

/// The owner kind of a venue (the building panel).
pub fn owner_kind_of(world: &World, b: EntityId) -> OwnerKind {
    ownership::owner_kind(world, world.owner_of(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old `nearest_venue`: every venue of the kind collected, sorted by
    /// `(door distance, id)`, the first that passes (the base's code).
    fn nearest_venue_scan(
        world: &World,
        id: EntityId,
        kind: BuildingKind,
        from: Origin,
        district: Option<DistrictId>,
    ) -> Option<(u32, EntityId)> {
        let mut near: Vec<(u32, EntityId)> = world
            .buildings_of_kind(kind)
            .iter()
            .filter_map(|&b| world.comp::<Building>(b).map(|bd| (from.dist(bd.door), b)))
            .collect();
        near.sort_unstable();
        let own = world.comp::<crate::components::Job>(id).and_then(|j| j.employer);
        near.into_iter().find(|&(_, b)| {
            Some(b) != own
                && standing(world, b)
                && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
                && district.is_none_or(|d| world.district_of_building(b) == d)
                && open(world, b)
                && door_seen(world, id, b)
                && has_room(world, id, b)
        })
    }

    /// The old `evening_venues`' pick per kind: the lowest `(other district,
    /// door distance, id)` over every venue that passes (the base's code).
    fn evening_scan(world: &World, id: EntityId, kind: BuildingKind) -> Option<EntityId> {
        let (day_from, district) = evening_day(world, id)?;
        let cap = world.config.leisure.venue_walk_cap_ticks;
        let mut near: Vec<(bool, u32, EntityId)> = world
            .buildings_of_kind(kind)
            .iter()
            .filter(|&&b| {
                standing(world, b)
                    && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
                    && open(world, b)
                    && door_seen(world, id, b)
                    && has_room(world, id, b)
                    && (kind != BuildingKind::Den || table_open(world, b))
            })
            .filter_map(|&b| {
                let bd = world.comp::<Building>(b)?;
                let walk = crate::exec::timed_walk(world, id, day_from.near(bd.door), bd.door);
                (walk <= cap).then(|| (world.district_of_building(b) != district, day_from.dist(bd.door), b))
            })
            .collect();
        near.sort_unstable();
        near.first().map(|&(_, _, b)| b)
    }

    /// The index's picks against the scans for every 5th citizen and every
    /// kind of the ladder: `nearest_venue` from where it stands and from its
    /// day, with and without its district, unbounded and cut at the walk
    /// cap's tiles (`None` exactly when the scan's pick lies past them, and
    /// then the pick is past the cap's walk too); and `evening_venues`
    /// against the old sort and its price gates. Returns the queries with a
    /// tie at distance 0 (several doors inside a day's rectangle).
    fn check(w: &World, label: &str) -> usize {
        let cap = w.config.leisure.venue_walk_cap_ticks;
        let mut ties = 0;
        // scan-ok: a unit test.
        for id in w.citizens().into_iter().step_by(5) {
            let Some(at) = origin(w, id) else { continue };
            let max_d = reach_tiles(w, id, Some(cap));
            let mut froms = vec![(Origin::At(at), w.district_of(at))];
            if let Some((o, d)) = evening_day(w, id) {
                froms.push((o, d));
            }
            for kind in LADDER {
                for &(from, district) in &froms {
                    for dist in [None, Some(district)] {
                        let want = nearest_venue_scan(w, id, kind, from, dist);
                        let got = nearest_venue(w, id, kind, from, dist, u32::MAX);
                        assert_eq!(got, want, "{label}: {id} {kind:?} from {from:?} in {dist:?}");
                        let cut = nearest_venue(w, id, kind, from, dist, max_d);
                        assert_eq!(cut, want.filter(|&(d, _)| d <= max_d), "{label}: {id} {kind:?} cut at {max_d}");
                        if let Some((d, b)) = want.filter(|&(d, _)| d > max_d) {
                            let door = w.comp::<Building>(b).expect("a venue").door;
                            assert!(crate::exec::timed_walk(w, id, from.near(door), door) > cap, "{label}: {b} at {d}");
                        }
                    }
                    let zero = w
                        .buildings_of_kind(kind)
                        .iter()
                        .filter(|&&b| w.comp::<Building>(b).is_some_and(|bd| from.dist(bd.door) == 0))
                        .count();
                    ties += usize::from(zero >= 2);
                }
            }
            let got: Vec<(BuildingKind, EntityId)> = evening_venues(w, id);
            let purse = spendable(w, id);
            let high = high_ok(w, id);
            let want: Vec<(BuildingKind, EntityId)> = if purse < 1 {
                Vec::new()
            } else {
                LADDER
                    .iter()
                    .filter_map(|&k| evening_scan(w, id, k).map(|b| (k, b)))
                    .filter(|&(k, b)| {
                        let price = price_of(w, b);
                        !((!high && rung_of(w, b, price) == Rung::High && !lonely_client(w, id, b))
                            || (k != BuildingKind::Bar && 2 * price > purse))
                    })
                    .collect()
            };
            assert_eq!(got, want, "{label}: {id}'s evening");
        }
        ties
    }

    /// The perf pass phase 2: the venue index is the old collect-and-sort on
    /// the shipped city at 20:30 on day 1 (the evening's venues open), then
    /// after a Bar is founded beside a sample agent, the Bar nearest another
    /// is closed (re-kinded a Home), and the next is shut (derelict), each
    /// read at once: without the index's invalidation point
    /// (`buildings_by_kind_mut`) the founded Bar is missed and the closed one
    /// still offered.
    #[test]
    fn test_venue_index_matches_the_scan() {
        let mut w = World::new(42, crate::config::Config::load());
        w.run_ticks(TICKS_PER_DAY + 20 * TICKS_PER_HOUR + 30);
        let ties = check(&w, "day 1 20:30");
        assert!(ties > 0, "no tie at distance 0: the tie-break is not exercised");
        // scan-ok: a unit test.
        let people = w.citizens();
        // A Bar founded on the empty building nearest a sample agent.
        let a = people[people.len() / 3];
        let at = origin(&w, a).expect("an origin");
        let site = [BuildingKind::Lot, BuildingKind::Home]
            .iter()
            .flat_map(|&k| w.buildings_of_kind(k).iter().copied())
            .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && bd.occupants.is_empty()))
            .min_by_key(|&b| (w.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(at)), b))
            .expect("a site");
        let before = nearest_venue(&w, a, BuildingKind::Bar, Origin::At(at), None, u32::MAX);
        crate::systems::founding::convert(&mut w, site, BuildingKind::Bar, None, 1);
        let after = nearest_venue_scan(&w, a, BuildingKind::Bar, Origin::At(at), None);
        assert_ne!(after, before, "the founded Bar is not the agent's nearest: the test exercises nothing");
        check(&w, "a Bar founded");
        // The Bar nearest another agent closed (re-kinded a Home).
        let c = people[2 * people.len() / 3];
        let cat = origin(&w, c).expect("an origin");
        let (_, shut) = nearest_venue_scan(&w, c, BuildingKind::Bar, Origin::At(cat), None).expect("a Bar");
        crate::systems::founding::convert(&mut w, shut, BuildingKind::Home, None, 0);
        assert_ne!(nearest_venue_scan(&w, c, BuildingKind::Bar, Origin::At(cat), None).map(|p| p.1), Some(shut));
        check(&w, "a Bar closed");
        // And the next derelict (a query-time gate, no re-kind).
        let (_, next) = nearest_venue_scan(&w, c, BuildingKind::Bar, Origin::At(cat), None).expect("a Bar");
        if let Some(bd) = w.comp_mut::<Building>(next) {
            bd.derelict = true;
        }
        check(&w, "a Bar derelict");
        // Ten more minutes of the city (the reservations and crowds move).
        w.run_ticks(10);
        check(&w, "ten minutes on");
    }
}
