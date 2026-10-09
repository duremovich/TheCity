//! The Real economy phase 1 (docs/ECONOMY_V2.md § 3; plan E4-E12): the
//! World account's book per good. Each midnight (`living::run`, where L2's
//! `export_daily` ran): yesterday's counters roll into the 30-day rings,
//! the appetite walks (a pure hash, E7), the account refills to
//! `treasury_ref` both ways (E6), and the Parts and Data legs sell (E10).
//! Food sells at the haul (`economy::haul`, E9). A sale of `n` units pays
//! the closed-form sum of the sloped tranche bids, capped at the day's cap
//! (E8); the ask (E11) is what a Market pays to import. Everything is a
//! ledger abstraction: counters on the outside account and coins crossed
//! by `ownership::cross_in`.

use crate::components::{Building, BuildingKind, Good};
use crate::econ::{WorldBook, PURPOSE_APPETITE};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{ExportGood, WORLD_ACCOUNT};
use crate::systems::econ;
use crate::systems::ownership::{self, Flow};
use crate::world::World;

/// Days of the rings the fill ratio and the unfilled order read (E18, E19).
pub const FILL_DAYS: usize = 7;

pub fn on(world: &World) -> bool {
    econ::market_on(world)
}

/// The World buys and sells: the market on, `SetExport` open (L2's lever,
/// set from `[export] enabled` or the market at seed) and not `CloseWorld`.
pub fn open(world: &World) -> bool {
    on(world) && world.levers.export_open && !world.econ.world_closed
}

/// E47: a Market may import: the market on and not `CloseWorld` (`SetExport`
/// closes the World's buying only; the ceiling, E12, follows `open`).
pub fn imports_open(world: &World) -> bool {
    on(world) && !world.econ.world_closed
}

pub fn book(world: &World, good: ExportGood) -> Option<&WorldBook> {
    world.outside.faction(WORLD_ACCOUNT).and_then(|f| f.books.get(&good))
}

pub(crate) fn book_mut(world: &mut World, good: ExportGood) -> Option<&mut WorldBook> {
    world.outside.faction_mut(WORLD_ACCOUNT).and_then(|f| f.books.get_mut(&good))
}

fn market(world: &World) -> f32 {
    world.outside.faction(WORLD_ACCOUNT).map_or(1.0, |f| f.market)
}

/// Today's appetite for a good (1.0 before the book exists).
pub fn appetite(world: &World, good: ExportGood) -> f32 {
    book(world, good).map_or(1.0, |b| b.appetite)
}

/// E8: `cap_pin`, else `round(cap × appetite)`.
pub fn cap_today(world: &World, good: ExportGood) -> u32 {
    if let Some(c) = book(world, good).and_then(|b| b.cap_pin) {
        return c;
    }
    let cap = world.config.world_market.good(good).cap;
    (cap as f32 * appetite(world, good)).round().max(0.0) as u32
}

/// The first unit's reference bid: L2's `SetExportPrice` pin, else `bid_ref`.
fn bid_ref(world: &World, good: ExportGood) -> f64 {
    match world.outside.export.price.get(&good) {
        Some(&p) => p as f64,
        None => f64::from(world.config.world_market.good(good).bid_ref),
    }
}

/// The bid curve's intercept and slope per unit today: `p(q) = max(floor, a − d q)`.
fn curve(world: &World, good: ExportGood) -> (f64, f64, f64) {
    let g = world.config.world_market.good(good);
    let a = bid_ref(world, good) * f64::from(market(world)) * f64::from(appetite(world, good));
    let cap = cap_today(world, good);
    let d = if cap == 0 { 0.0 } else { a * f64::from(g.slope) / f64::from(cap) };
    (a, d, f64::from(g.bid_floor))
}

/// E8: the bid for the `q`-th unit sold today (`q` = `bought_today` before it).
pub fn bid(world: &World, good: ExportGood, q: u32) -> f64 {
    let (a, d, floor) = curve(world, good);
    (a - d * f64::from(q)).max(floor)
}

/// The marginal bid: the next unit's.
pub fn marginal_bid(world: &World, good: ExportGood) -> f64 {
    bid(world, good, book(world, good).map_or(0, |b| b.bought_today))
}

/// Σ `bid(q)` for `q` in `[from, from + n)`, in closed form (the arithmetic
/// series clipped at the floor), rounded once.
pub fn tranche(world: &World, good: ExportGood, from: u32, n: u32) -> i64 {
    if n == 0 {
        return 0;
    }
    let (a, d, floor) = curve(world, good);
    let n_f = f64::from(n);
    let b = f64::from(from);
    // Units priced on the line: q ≤ (a − floor) / d.
    let on_line = if d <= 0.0 {
        if a >= floor {
            n_f
        } else {
            0.0
        }
    } else {
        let q_max = ((a - floor) / d).floor();
        (q_max - b + 1.0).clamp(0.0, n_f)
    };
    let linear = on_line * a - d * on_line * (2.0 * b + on_line - 1.0) / 2.0;
    (linear + (n_f - on_line) * floor).round() as i64
}

/// How many of `n` units from `from` fetch at least `min_price` (E10: a Fab
/// keeps its Parts for the city's `parts_price` below it).
pub fn units_at_least(world: &World, good: ExportGood, from: u32, n: u32, min_price: f64) -> u32 {
    let (a, d, floor) = curve(world, good);
    if floor >= min_price {
        return n;
    }
    if d <= 0.0 {
        return if a >= min_price { n } else { 0 };
    }
    let q_max = ((a - min_price) / d).floor();
    (q_max - f64::from(from) + 1.0).clamp(0.0, f64::from(n)) as u32
}

/// What the World would pay for `n` units now: the units inside today's
/// cap and the coins (E8).
pub fn quote(world: &World, good: ExportGood, n: u32) -> (u32, i64) {
    let Some(b) = book(world, good) else { return (0, 0) };
    let room = cap_today(world, good).saturating_sub(b.bought_today);
    let units = n.min(room);
    (units, tranche(world, good, b.bought_today, units))
}

/// E8: sell `n` units of `good` from `building` (credited) for `owner`: the
/// coins cross in (`Flow::Export`), the owner pays the sale's tax
/// explicitly as L2, the book counts the units. Returns `(units, paid)`.
pub fn sell(
    world: &mut World,
    building: Option<EntityId>,
    owner: Option<EntityId>,
    good: ExportGood,
    n: u32,
) -> (u32, i64) {
    if !open(world) || n == 0 {
        return (0, 0);
    }
    let (mut units, mut paid) = quote(world, good, n);
    let purse = world.outside.faction(WORLD_ACCOUNT).map_or(0, |f| f.treasury.max(0));
    let from = book(world, good).map_or(0, |b| b.bought_today);
    // The account refills daily and is never short in practice; if it is,
    // the sale shrinks to what it holds.
    while units > 0 && paid > purse {
        let fit = ((i128::from(units) * i128::from(purse)) / i128::from(paid.max(1))) as u32;
        units = fit.min(units - 1);
        paid = tranche(world, good, from, units);
    }
    if units == 0 || paid <= 0 {
        return (0, 0);
    }
    let moved = ownership::cross_in(world, WORLD_ACCOUNT, owner, paid, Flow::Export);
    let tax = (world.levers.tax_rate * moved as f32).round() as i64;
    if owner.is_some() && tax > 0 {
        ownership::pay(world, owner, None, tax, Flow::Tax);
    }
    if let Some(b) = building {
        ownership::credit(world, b, moved);
    }
    if let Some(bk) = book_mut(world, good) {
        bk.bought_today += units;
        bk.paid_today += moved;
    }
    *world.outside.export.sold.entry(good).or_default() += u64::from(units);
    *world.outside.export.paid.entry(good).or_default() += moved;
    (units, moved)
}

/// E11: the ask, `round_up(bid_ref × market × (1 + spread) × ask_mult × the good's ask_mult)`.
pub fn ask(world: &World, good: ExportGood) -> i64 {
    let g = world.config.world_market.good(good);
    let mult = f64::from(world.config.world_market.ask_mult) * f64::from(book(world, good).map_or(1.0, |b| b.ask_mult));
    (bid_ref(world, good) * f64::from(market(world)) * (1.0 + f64::from(g.spread)) * mult).ceil().max(0.0) as i64
}

/// E5: the customs rate (a god's `SetCustoms` pin first).
pub fn customs_rate(world: &World) -> f32 {
    world.econ.customs_pin.unwrap_or(world.config.world_market.customs_rate).max(0.0)
}

/// A Market's import booked on the World's side (`sold_today`, `charged_today`).
pub fn note_sold(world: &mut World, good: ExportGood, units: u32, coins: i64) {
    if let Some(b) = book_mut(world, good) {
        b.sold_today += units;
        b.charged_today += coins;
    }
}

/// E19: the fill ratio, `Σ bought ÷ Σ cap_today` over the last `FILL_DAYS` days (0 with no history).
pub fn fill(world: &World, good: ExportGood) -> f32 {
    let Some(b) = book(world, good) else { return 0.0 };
    let bought = WorldBook::sum_u32(&b.bought, FILL_DAYS);
    let caps = WorldBook::sum_u32(&b.caps, FILL_DAYS);
    if caps == 0 {
        0.0
    } else {
        bought as f32 / caps as f32
    }
}

/// Plan Seeding (E4): the World account at `[world_market] treasury_ref`
/// (`minted` += it, so the identity's base is taken after seeding) and a
/// book per good at appetite 1.0. No RNG.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    ensure(world);
}

/// The account and its books, created on first use (a loaded save made
/// with the market off and flipped on: a god scenario).
fn ensure(world: &mut World) {
    let treasury_ref = world.config.world_market.treasury_ref;
    crate::systems::outside::ensure_world_account(world, treasury_ref);
    let Some(f) = world.outside.faction_mut(WORLD_ACCOUNT) else { return };
    f.treasury_ref = treasury_ref;
    let refill = treasury_ref - f.treasury;
    f.treasury += refill;
    for g in ExportGood::ALL {
        f.books.entry(g).or_default();
    }
    world.outside.minted += refill;
}

/// E7: one day of a good's appetite walk: `walk = 1 + (walk − 1)(1 − revert) + sd × z`;
/// `appetite = pin, else clamp(walk + season[good][season])` (the season a
/// level each day, not accumulated: the plan's deviation).
pub fn appetite_step(
    cfg: &crate::config::WorldMarketCfg,
    good: ExportGood,
    book: &mut WorldBook,
    season: usize,
    z: f32,
) {
    let gc = cfg.good(good);
    book.walk = 1.0 + (book.walk - 1.0) * (1.0 - cfg.revert) + cfg.sd * z;
    let raw = (book.walk + gc.season[season]).clamp(cfg.appetite_min, cfg.appetite_max);
    book.appetite = book.appetite_pin.unwrap_or(raw);
}

/// The day's close (E44), called from `systems::stats::run` right after the
/// day row rolls at 23:59: one `Exported` per good with the mean bid, the
/// counters into the 30-day rings. At 23:59 and not midnight because the
/// midnight imports (`economy::run`) run before `living::run` in the tick,
/// and the day's snapshot must see the counters whole.
pub fn close_day(world: &mut World) {
    if !on(world) {
        return;
    }
    for g in ExportGood::ALL {
        let cap = cap_today(world, g);
        let Some(b) = book(world, g) else { continue };
        let (n, paid) = (b.bought_today, b.paid_today);
        if n > 0 {
            let mean = paid as f64 / f64::from(n);
            world.push_event(
                EventKind::Exported,
                &[],
                format!("the World bought {n} {} for {paid} (mean bid {mean:.2})", g.label()),
            );
        }
        if let Some(b) = book_mut(world, g) {
            b.roll(cap);
        }
    }
}

/// The midnight pass (E6, E7, E10, E44).
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    ensure(world);
    let cfg = world.config.world_market.clone();
    let (seed, day, season) = (world.seed(), world.day(), world.season().index());
    // 1. The day closed at 23:59 (`close_day`, from the stats roll: the
    // midnight imports in `economy::run` precede this pass in the tick).
    // 2. The appetite walk (E7): a pure hash per (day, good); the season is a level, not accumulated.
    for g in ExportGood::ALL {
        let z = econ::hash_normal(seed, PURPOSE_APPETITE, day, g as u64);
        let Some(b) = book_mut(world, g) else { continue };
        let old = b.appetite;
        appetite_step(&cfg, g, b, season, z);
        let new = b.appetite;
        let far = |a: f32| (a - 1.0).abs() >= 0.25;
        if far(new) != far(old) {
            let text = if far(new) {
                format!(
                    "the World {} {}: appetite {new:.2}",
                    if new > 1.0 { "wants" } else { "has little use for" },
                    g.label()
                )
            } else {
                format!("the World's appetite for {} is back near normal: {new:.2}", g.label())
            };
            world.push_event(EventKind::AppetiteShift, &[], text);
        }
    }
    // 3. The refill, both ways (E6): `minted` cumulated is the city's net trade.
    if let Some(f) = world.outside.faction_mut(WORLD_ACCOUNT) {
        let delta = f.treasury_ref - f.treasury;
        f.treasury += delta;
        f.income_today = 0;
        world.outside.minted += delta;
    }
    if !open(world) {
        return;
    }
    // 4. Parts (E10): Fabs above `[export] parts_floor` while the bid holds the
    // city's `parts_price`, then the Recycler's scrap Parts (city-owned:
    // the Treasury until phase 3a's till), ascending id.
    let parts_floor = world.config.export.parts_floor;
    let parts_price = world.config.assets.parts_price.max(0) as f64;
    let mut sellers: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Fab)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect();
    sellers.sort_unstable();
    // Real economy phase 3c (E10, E40): the work camps' Parts, after the Fabs
    // and before the Recycler (none stands with `[camp]` off).
    sellers.extend(crate::systems::camp::standing_camps(world));
    if let Some(r) = world.building_of_kind(BuildingKind::Cemetery) {
        sellers.push(r);
    }
    for b in sellers {
        let have = world.stock(b, Good::Parts).saturating_sub(parts_floor);
        if have == 0 {
            continue;
        }
        let from = book(world, ExportGood::Parts).map_or(0, |bk| bk.bought_today);
        let (room, _) = quote(world, ExportGood::Parts, have);
        let units = units_at_least(world, ExportGood::Parts, from, room, parts_price);
        if units == 0 {
            continue;
        }
        let owner = world.owner_of(b);
        let (n, _) = sell(world, Some(b), owner, ExportGood::Parts, units);
        world.take_stock(b, Good::Parts, n);
    }
    // 5. Data (E10): Labs' stores above `[export] data_floor`, from the fullest track (L2's leg).
    let data_floor = world.config.export.data_floor;
    let mut labs = world.buildings_of_kind(BuildingKind::Lab).to_vec();
    labs.sort_unstable();
    for b in labs {
        let Some(n) = crate::systems::virt::node_of_building(world, b) else { continue };
        let Some(store) = world.virt.node(n).map(|x| x.store.clone()) else { continue };
        let units = store.total().saturating_sub(data_floor);
        if units == 0 {
            continue;
        }
        let owner = world.owner_of(b);
        let (sold, _) = sell(world, Some(b), owner, ExportGood::Data, units);
        let mut take = sold;
        if let Some(node) = world.virt.node_mut(n) {
            while take > 0 {
                let Some(i) = (0..3).max_by_key(|&i| (node.store.units[i], std::cmp::Reverse(i))) else { break };
                let t = node.store.units[i].min(take);
                if t == 0 {
                    break;
                }
                node.store.units[i] -= t;
                take -= t;
            }
        }
    }
}
