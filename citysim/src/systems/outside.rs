//! The World account (`outside::WORLD_ACCOUNT`): created on first use and
//! booked by the World market (`world_market`). L2's flat-price export hook
//! that used to live here retired with the off switches (2026-10-09): the
//! market is the one World path. M17 phase 1 adds the Ambient book (below).

use crate::components::{Building, BuildingKind, Corp, Good, Market};
use crate::entity::EntityId;
use crate::outside::{AmbientBook, OutsideFaction, OutsideKind, WORLD_ACCOUNT};
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_HOUR;
use crate::world::World;

/// The World account, created on first use (Real economy plan E4: the
/// market seeds it with `[world_market] treasury_ref`).
pub(crate) fn ensure_world_account(world: &mut World, treasury_ref: i64) {
    if world.outside.faction(WORLD_ACCOUNT).is_some() {
        return;
    }
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
    });
    world.outside.next_id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
}

// ---------------------------------------------------------------------------
// M17 phase 1 (docs/M17_OUTSIDE.md, "Revised 2026-10-12"): the Ambient book.
//
// The Ambient flats' households are the World's (their purses are outside
// the simulated population): when they buy at a simulated Market or venue,
// World money buys a city good, export demand at retail. The sale is the
// owner's revenue (`Corp.rev_today`, so `wage_rev` and the vacancies follow)
// and a Market's sale (`Market.sales`, so sell-through and Grow follow); the
// coins cross in from the World account (`ownership::cross_in`), so the coin
// identity holds. Goods are real: a Market sells only its stock above
// `keep_days` of its own sales. Nothing runs per agent: one pass an hour,
// O(districts + Markets + venues). No stream draw: the appetite walk is a
// pure hash. Nothing at all on a map without Ambient flats.
// ---------------------------------------------------------------------------

/// `econ::hash_normal` purpose of the Ambient appetite walk.
const PURPOSE_AMBIENT_APPETITE: u64 = 0x0E17_0001;

/// `[outside] enabled` (`CITYSIM_OUTSIDE_OFF=1` turns it off).
pub fn on(world: &World) -> bool {
    world.config.outside.enabled
}

/// The Ambient book runs: the outside on and the map has Ambient flats.
pub fn ambient_on(world: &World) -> bool {
    on(world) && crate::systems::ambient::on(world)
}

/// The World's Ambient book, once made.
pub fn ambient_book(world: &World) -> Option<&AmbientBook> {
    world.outside.faction(WORLD_ACCOUNT).and_then(|f| f.ambient.as_ref())
}

fn book_mut(world: &mut World) -> Option<&mut AmbientBook> {
    world.outside.faction_mut(WORLD_ACCOUNT).and_then(|f| f.ambient.as_mut())
}

/// The book, made on first use (the World account too, should a god have
/// removed it: the market seeds it at `World::new`).
fn ensure_book(world: &mut World) {
    if ambient_book(world).is_some() {
        return;
    }
    let treasury_ref = world.config.world_market.treasury_ref;
    ensure_world_account(world, treasury_ref);
    if let Some(f) = world.outside.faction_mut(WORLD_ACCOUNT) {
        f.ambient = Some(AmbientBook::default());
    }
}

/// Hour `k`'s part of `due` spread over `n` hours: the parts differ by at
/// most one and sum to `due`.
pub fn hour_share(due: u32, n: u32, k: u32) -> u32 {
    if n == 0 || k >= n {
        return 0;
    }
    let (due, n, k) = (u64::from(due), u64::from(n), u64::from(k));
    (due * (k + 1) / n - due * k / n) as u32
}

/// `total` split over `weights` by largest remainder (ties to the lower
/// index): exact, proportional, a zero weight gets nothing.
pub fn apportion(total: u32, weights: &[f64]) -> Vec<u32> {
    let sum: f64 = weights.iter().map(|w| w.max(0.0)).sum();
    if total == 0 || sum <= 0.0 {
        return vec![0; weights.len()];
    }
    let exact: Vec<f64> = weights.iter().map(|w| w.max(0.0) * f64::from(total) / sum).collect();
    let mut out: Vec<u32> = exact.iter().map(|e| e.floor() as u32).collect();
    let mut left = total.saturating_sub(out.iter().sum::<u32>());
    let mut order: Vec<usize> = (0..weights.len()).filter(|&i| weights[i] > 0.0).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())).then(a.cmp(&b)));
    for &i in order.iter().cycle() {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    out
}

/// A standing building (not demolished, not derelict).
fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// The units a Market may sell to the book now: its stock above
/// `keep_days` × its mean daily sales over the 7-day ring (none before the
/// ring holds a day: a fresh Market keeps everything).
pub fn food_surplus(world: &World, m: EntityId) -> u32 {
    let (Some(bd), Some(mk)) = (world.comp::<Building>(m), world.comp::<Market>(m)) else { return 0 };
    if bd.demolished || bd.derelict || mk.sales.is_empty() {
        return 0;
    }
    let mean = f64::from(mk.sales.iter().sum::<u32>()) / mk.sales.len() as f64;
    let keep = (f64::from(world.config.outside.ambient.keep_days.max(0.0)) * mean).ceil() as u32;
    bd.stock_food.saturating_sub(keep)
}

/// Coins for `units` at `tenths` a unit (rounded, at least a coin a unit).
fn cost_of(units: u32, tenths: i64) -> i64 {
    ((i64::from(units) * tenths + 5) / 10).max(i64::from(units))
}

/// The World pays `cost` to `owner` for a sale at building `b`: the coins
/// cross in (`Flow::AmbientSale`), the owner pays the sale's tax explicitly
/// (as `Export`), a corp books it as revenue (wages on), the building is
/// credited. Returns the coins moved.
fn pay_owner(world: &mut World, b: EntityId, cost: i64) -> i64 {
    let owner = world.owner_of(b);
    let moved = ownership::cross_in(world, WORLD_ACCOUNT, owner, cost, Flow::AmbientSale);
    if moved <= 0 {
        return 0;
    }
    let tax = (world.levers.tax_rate.max(0.0) * moved as f32).round() as i64;
    let tax = if owner.is_some() && tax > 0 { ownership::pay(world, owner, None, tax, Flow::Tax) } else { 0 };
    if crate::systems::wages::on(world) {
        if let Some(c) = owner.and_then(|o| world.comp_mut::<Corp>(o)) {
            c.rev_today += moved - tax;
        }
    }
    ownership::credit(world, b, moved);
    moved
}

/// The World's coins left today (it refills at midnight).
fn world_purse(world: &World) -> i64 {
    world.outside.faction(WORLD_ACCOUNT).map_or(0, |f| f.treasury.max(0))
}

/// Sell up to `want` units at Market `m` to the book (its surplus, what the
/// World can pay): real units off the shelf, a sale on the Market's books,
/// revenue to its owner. Returns the units sold.
pub fn sell_food_at(world: &mut World, m: EntityId, want: u32) -> u32 {
    let mut units = want.min(food_surplus(world, m));
    if units == 0 {
        return 0;
    }
    let tenths = world.price_tenths_at(m).max(1);
    let purse = world_purse(world);
    while units > 0 && cost_of(units, tenths) > purse {
        units -= 1;
    }
    if units == 0 {
        return 0;
    }
    let paid = pay_owner(world, m, cost_of(units, tenths));
    if paid <= 0 {
        return 0;
    }
    if let Some(bd) = world.comp_mut::<Building>(m) {
        bd.stock_food -= units;
    }
    if let Some(mk) = world.comp_mut::<Market>(m) {
        mk.sales_today += units;
    }
    world.ambient.today.food_bought += units;
    world.ambient.today.paid_food += paid;
    units
}

/// The visits venue `v` may take from the book now: open, priced, at most
/// `visit_share` of its own 7-day mean daily visits a day (none before the
/// ring holds a day); a NoodleBar only with a meal on the shelf.
fn visit_room(world: &World, v: EntityId, taken: u32) -> u32 {
    let Some(bd) = world.comp::<Building>(v) else { return 0 };
    let Some(venue) = bd.venue.as_ref() else { return 0 };
    if venue.visits.is_empty() || !crate::systems::leisure::open(world, v) {
        return 0;
    }
    if crate::systems::leisure::price_of(world, v) <= 0 {
        return 0;
    }
    let mean = f64::from(venue.visits.iter().map(|&x| u32::from(x)).sum::<u32>()) / venue.visits.len() as f64;
    let cap = (f64::from(world.config.outside.ambient.visit_share.max(0.0)) * mean).floor() as u32;
    let room = cap.saturating_sub(taken);
    if bd.kind == BuildingKind::NoodleBar {
        room.min(bd.stock_food)
    } else {
        room
    }
}

/// Up to `want` ledger visits at venue `v` (no bodies, no seats): its price
/// each, a NoodleBar's meal off its shelf. Returns the visits made.
fn visit_at(world: &mut World, v: EntityId, want: u32) -> u32 {
    let taken = ambient_book(world).and_then(|b| b.venue_today.get(&v).copied()).unwrap_or(0);
    let price = crate::systems::leisure::price_of(world, v);
    let mut n = want.min(visit_room(world, v, taken));
    if price <= 0 {
        return 0;
    }
    let purse = world_purse(world);
    n = n.min(u32::try_from(purse / price).unwrap_or(u32::MAX));
    if n == 0 {
        return 0;
    }
    let paid = pay_owner(world, v, price * i64::from(n));
    if paid <= 0 {
        return 0;
    }
    if world.comp::<Building>(v).is_some_and(|bd| bd.kind == BuildingKind::NoodleBar) {
        world.take_stock(v, Good::Food, n);
    }
    if let Some(b) = book_mut(world) {
        *b.venue_today.entry(v).or_insert(0) += n;
    }
    world.ambient.today.visits_made += n;
    world.ambient.today.paid_visits += paid;
    n
}

/// The buildings of `kinds` standing in district `d` first (ascending id),
/// then the rest by Manhattan door distance from the district's centroid
/// (ties ascending id).
fn sellers(world: &World, d: usize, kinds: &[BuildingKind]) -> Vec<EntityId> {
    let Some(dist) = world.districts.get(d) else { return Vec::new() };
    let centre = dist.centroid;
    let mut near: Vec<EntityId> = Vec::new();
    let mut far: Vec<(u32, EntityId)> = Vec::new();
    for &k in kinds {
        for &b in world.buildings_of_kind(k) {
            if !standing(world, b) {
                continue;
            }
            if world.district_of_building(b).index() == d {
                near.push(b);
            } else if let Some(bd) = world.comp::<Building>(b) {
                far.push((bd.door.manhattan(centre), b));
            }
        }
    }
    near.sort_unstable();
    far.sort_unstable();
    near.extend(far.into_iter().map(|(_, b)| b));
    near
}

/// The day's demand per district: food units and venue visits, each the
/// Ambient estimate × `capture` × appetite, capped city-wide (per capita)
/// and apportioned back to the districts: food, the district's Ambient
/// adults × yesterday's meals per simulated resident (the aggregate's own
/// estimate, read fresh); visits, its adults × yesterday's simulated venue
/// visits per resident.
fn demand_today(world: &World, appetite: f32) -> (Vec<u32>, Vec<u32>) {
    let cfg = &world.config.outside.ambient;
    let k = f64::from(cfg.capture.max(0.0)) * f64::from(appetite.max(0.0));
    // Yesterday's row (the 23:59 close precedes this midnight; the Ambient
    // aggregates' own food estimate is refreshed later in the same tick).
    let last = world.stats.history.back();
    let mpr = last.map_or(0.0, |r| {
        let h = &r.health;
        f64::from(h.meals_real + h.meals_synth + h.meals_dive + h.meals_alms) / f64::from(r.population.max(1))
    });
    let vpr = last.map_or(0.0, |r| f64::from(r.living.visits.iter().sum::<u32>()) / f64::from(r.population.max(1)));
    let food: Vec<f64> = world.ambient.districts.iter().map(|a| f64::from(a.adults) * mpr * k).collect();
    let visits: Vec<f64> = world.ambient.districts.iter().map(|a| f64::from(a.adults) * vpr * k).collect();
    let total = |v: &[f64], cap: u32| -> u32 { (v.iter().sum::<f64>().round() as u64).min(u64::from(cap)) as u32 };
    let (tf, tv) = (total(&food, cfg.food_cap), total(&visits, cfg.visit_cap));
    (apportion(tf, &food), apportion(tv, &visits))
}

/// The day opens (the first hour of a new day): the appetite walks, today's
/// demand is fixed, yesterday's venue counts go.
fn open_day(world: &mut World) {
    let (seed, day) = (world.seed(), world.day());
    let z = crate::systems::econ::hash_normal(seed, PURPOSE_AMBIENT_APPETITE, day, 0);
    let cfg = world.config.outside.ambient.clone();
    let appetite = {
        let Some(b) = book_mut(world) else { return };
        b.walk = 1.0 + (b.walk - 1.0) * (1.0 - cfg.revert) + cfg.sd * z;
        let raw = b.walk.clamp(cfg.appetite_min, cfg.appetite_max);
        b.appetite = b.appetite_pin.unwrap_or(raw);
        b.appetite
    };
    let (food, visits) = demand_today(world, appetite);
    world.ambient.today.food_wanted = food.iter().sum();
    world.ambient.today.visits_wanted = visits.iter().sum();
    if let Some(b) = book_mut(world) {
        b.due_day = day;
        b.food_due = food;
        b.visit_due = visits;
        b.venue_today.clear();
    }
}

/// One selling hour: each district's share of its due, at its own sellers
/// first. Unsold demand is lost (the households went elsewhere).
fn sell_hour(world: &mut World, hour: u8) {
    let cfg = world.config.outside.ambient.clone();
    let window = |w: [u8; 2]| -> Option<(u32, u32)> {
        (hour >= w[0] && hour < w[1]).then(|| (u32::from(w[1] - w[0]), u32::from(hour - w[0])))
    };
    let Some(book) = ambient_book(world) else { return };
    let (food_due, visit_due) = (book.food_due.clone(), book.visit_due.clone());
    if let Some((n, k)) = window(cfg.food_hours) {
        for (d, &due) in food_due.iter().enumerate() {
            let mut q = hour_share(due, n, k);
            if q == 0 {
                continue;
            }
            for m in sellers(world, d, &[BuildingKind::Market]) {
                q -= sell_food_at(world, m, q);
                if q == 0 {
                    break;
                }
            }
        }
    }
    if let Some((n, k)) = window(cfg.visit_hours) {
        for (d, &due) in visit_due.iter().enumerate() {
            let mut q = hour_share(due, n, k);
            if q == 0 {
                continue;
            }
            for v in sellers(world, d, &BuildingKind::LEISURE) {
                q -= visit_at(world, v, q);
                if q == 0 {
                    break;
                }
            }
        }
    }
}

/// The tick (after `living::run`): on the hour, the book opens its day if
/// it has not, then sells the hour's share.
pub fn run(world: &mut World) {
    if !ambient_on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    ensure_book(world);
    if ambient_book(world).is_some_and(|b| b.due_day != world.day()) {
        open_day(world);
    }
    let hour = (world.tick_of_day() / TICKS_PER_HOUR as u16) as u8;
    sell_hour(world, hour);
}

/// The day's close (`ambient::close_day`, 23:59): the counters into the rings.
pub fn close_day(world: &mut World) {
    let t = world.ambient.today.clone();
    let Some(b) = book_mut(world) else { return };
    fn push<T>(ring: &mut std::collections::VecDeque<T>, v: T) {
        if ring.len() >= crate::econ::BOOK_DAYS {
            ring.pop_front();
        }
        ring.push_back(v);
    }
    push(&mut b.units, t.food_bought);
    push(&mut b.visits, t.visits_made);
    push(&mut b.paid, t.paid_food + t.paid_visits);
}

/// `SetAmbientAppetite`: pin (or release) the appetite.
pub fn pin_appetite(world: &mut World, mult: Option<f32>) -> Result<String, String> {
    if !ambient_on(world) {
        return Err("SetAmbientAppetite: no Ambient book (no Ambient flats, or the outside is off)".to_string());
    }
    ensure_book(world);
    let Some(b) = book_mut(world) else { return Err("SetAmbientAppetite: no World account".to_string()) };
    b.appetite_pin = mult.map(|m| m.max(0.0));
    if let Some(m) = b.appetite_pin {
        b.appetite = m;
    }
    Ok(match mult {
        Some(m) => format!("World appetite for Ambient spending pinned at {m:.2}"),
        None => "World appetite for Ambient spending released to the walk".to_string(),
    })
}
