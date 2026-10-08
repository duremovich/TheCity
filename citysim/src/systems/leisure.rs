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

use std::collections::BTreeMap;

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, Class, Corp, DistrictId, Gang, GangMember, GoalKind, Household, Job,
    Lod, MemoryKind, Needs, Personality, Position, RelKind, Role, Sentence, TilePos, Wallet, Zone,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{ExecState, ReservationKind};
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::living::{Rung, Spot, SpotKind, UnwindPick};
use crate::rng::splitmix64;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::{self, DayPhase, Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, gate_or, urgency, Curve};
use crate::utility::Consideration;
use crate::word::WordNs;
use crate::world::World;

/// Spots kept per district (plan L15).
const MAX_SPOTS: usize = 24;
/// Contacts a spot is scored for (plan L16).
const MAX_CONTACTS: usize = 6;
/// Tiles of a Sump street from a Home door a fire barrel may stand on.
const BARREL_REACH: u32 = 6;

/// `[living] enabled && [leisure] enabled` (plan L5, L13).
pub fn on(world: &World) -> bool {
    world.config.living.enabled && world.config.leisure.enabled
}

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
            // The homeless save for a Sump Block's re-housing deposit first.
            keep += world.config.rent.rehouse_coins_mult * world.config.rent.base[0].max(1);
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
fn owner_payroll(world: &World, owner: Option<EntityId>) -> i64 {
    const STAFFED: [BuildingKind; 4] =
        [BuildingKind::Bar, BuildingKind::Clinic, BuildingKind::Garage, BuildingKind::Hotel];
    // Review fix (perf): one allocation-free pass over the staffed kinds'
    // lists (`owned_of_kind`'s filter, same order); an owner of nothing,
    // nearly every agent, never reaches the staff scan.
    let owned = |b: EntityId| world.comp::<Building>(b).is_some_and(|bd| bd.owner == owner && !bd.demolished);
    let mut bill = 0;
    for kind in STAFFED.into_iter().chain(BuildingKind::LEISURE) {
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
    if !on(world) {
        return;
    }
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
    if !on(world) {
        return;
    }
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
    if !on(world) {
        return 0.0;
    }
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
// Venues: the door, the price, the visit (plan L19)
// ---------------------------------------------------------------------------

fn night(tod: u16) -> bool {
    !(360..1320).contains(&tod)
}

/// Plan L19: a venue is open: standing, not closed (a riot, a Crackdown,
/// `CloseLeisure`), not under a night curfew (22:00-06:00), and a Den only
/// while gambling is legal.
pub fn open(world: &World, b: EntityId) -> bool {
    let Some(bd) = world.comp::<Building>(b) else { return false };
    if bd.demolished || bd.derelict || world.is_closed(b) {
        return false;
    }
    let d = world.district_of_building(b);
    if world.levers.curfew.get(d.index()).copied().unwrap_or(false) && night(world.tick_of_day()) {
        return false;
    }
    !(bd.kind == BuildingKind::Den && world.levers.ban_gambling)
}

/// Room for `id`: inside already, or fewer occupants than the capacity.
/// `held` is [`held_by_others`] for `id`.
fn has_room(world: &World, id: EntityId, b: EntityId, held: &BTreeMap<EntityId, usize>) -> bool {
    // Plan L19, L30: the live Seat (and Bed) reservations of others count.
    world.comp::<Building>(b).is_some_and(|bd| {
        bd.occupants.contains(&id) || bd.occupants.len() + held.get(&b).copied().unwrap_or(0) < usize::from(bd.capacity)
    })
}

/// `World::reserved_at(b, exclude)` for every building at once (review fix,
/// perf: one pass over the reservations per ladder walk, not one per
/// venue): live `Bed` and `Seat` reservations by holders other than
/// `exclude`, per building, a holder already inside it not counted.
fn held_by_others(world: &World, exclude: EntityId) -> BTreeMap<EntityId, usize> {
    let tick = world.tick;
    let mut out: BTreeMap<EntityId, usize> = BTreeMap::new();
    for (&h, rs) in &world.reservations {
        if h == exclude {
            continue;
        }
        let inside = world.comp::<Position>(h).and_then(|p| p.building);
        for r in rs {
            if let ReservationKind::Bed { home: b } | ReservationKind::Seat { building: b } = r.kind {
                if r.expires > tick && inside != Some(b) {
                    *out.entry(b).or_default() += 1;
                }
            }
        }
    }
    out
}

/// Plan L19: the door policy. A Lounge refuses a Dreg or a dress below 2; a
/// Club refuses `rep.heat ≥ door_heat` when the strongest Host on shift has
/// courage above that heat (no draw).
pub fn door_ok(world: &World, id: EntityId, b: EntityId) -> bool {
    match kind_of(world, b) {
        Some(BuildingKind::Lounge) => {
            crate::systems::classes::class_of(world, id) != Class::Dreg
                && crate::systems::reputation::appearance_of(world, id).dress >= 2
        }
        Some(BuildingKind::Club) => {
            let heat = crate::systems::reputation::rep(world, id).heat;
            if heat < world.config.leisure.door_heat {
                return true;
            }
            let tod = world.tick_of_day();
            let bouncer = ownership::staff_at(world, b)
                .into_iter()
                .filter(|&s| world.comp::<Job>(s).is_some_and(|j| j.role == Role::Host && j.on_shift(tod)))
                .max_by(|&x, &y| {
                    crate::systems::law::fighting(world, x)
                        .total_cmp(&crate::systems::law::fighting(world, y))
                        .then(y.cmp(&x))
                });
            !bouncer.is_some_and(|h| crate::systems::law::courage(world, h) > heat)
        }
        _ => true,
    }
}

/// The venue's price today (`Venue.price`; a Bar's drink is 2).
pub fn price_of(world: &World, b: EntityId) -> i64 {
    let Some(bd) = world.comp::<Building>(b) else { return 0 };
    if bd.kind == BuildingKind::Bar {
        return 2;
    }
    bd.venue.as_ref().map_or(0, |v| v.price)
}

/// L2 L14: inside a Club or Den with leisure on (a Drink there).
pub fn drink_venue(world: &World, id: EntityId) -> bool {
    on(world)
        && here(world, id)
            .and_then(|b| kind_of(world, b))
            .is_some_and(|k| matches!(k, BuildingKind::Club | BuildingKind::Den))
}

/// A Drink at a Club or Den: the visit is the venue's (a Bar's is not),
/// unless the drink is part of an Unwind whose main step counts it.
pub fn note_drink(world: &mut World, id: EntityId) {
    if !drink_venue(world, id) {
        return;
    }
    add_fun(world, id, world.config.leisure.gain.drink);
    let unwinding = world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Unwind);
    if !unwinding {
        if let Some(b) = here(world, id) {
            crate::systems::jobs::note_visit(world, b);
        }
    }
}

/// A Bar drink also lifts `fun` (the cheap rung) with leisure on.
pub fn drink_fun(world: &mut World, id: EntityId) {
    if on(world) && here(world, id).and_then(|b| kind_of(world, b)) == Some(BuildingKind::Bar) {
        add_fun(world, id, world.config.leisure.gain.drink);
    }
}

/// The fun an `Enjoy` (or a meal, a table) at `b` gives.
fn gain_at(world: &World, b: EntityId) -> f32 {
    let g = &world.config.leisure.gain;
    match world.comp::<Building>(b).map(|bd| (bd.kind, bd.tier)) {
        Some((BuildingKind::Club, 2)) => g.club_spire,
        Some((BuildingKind::Club, _)) => g.club,
        Some((BuildingKind::Arcade, _)) => g.arcade,
        Some((BuildingKind::NoodleBar, _)) => g.noodle_bar,
        Some((BuildingKind::FightPit, _)) => g.fight_pit,
        Some((BuildingKind::Den, _)) => g.den,
        Some((BuildingKind::Lounge, _)) => g.lounge,
        Some((BuildingKind::Bar, _)) => g.drink,
        _ => 0.0,
    }
}

/// Belonging an `Enjoy` gives (spec: Club +0.2, Lounge +0.3).
fn belonging_at(world: &World, b: EntityId) -> f32 {
    match kind_of(world, b) {
        Some(BuildingKind::Club) => 0.2,
        Some(BuildingKind::Lounge) => 0.3,
        _ => 0.0,
    }
}

/// A payment into a venue (`Flow::Leisure` or `Flow::Gamble`): `pay` and
/// `credit`, the `SetLeisureTax` surcharge (plan L37), the front's take.
fn pay_venue(world: &mut World, from: EntityId, b: EntityId, amount: i64, flow: Flow) -> i64 {
    let owner = world.owner_of(b);
    let paid = ownership::pay(world, Some(from), owner, amount, flow);
    if paid <= 0 {
        return 0;
    }
    ownership::credit(world, b, paid);
    let tax = world.levers.leisure_tax;
    if tax > 0.0 && owner.is_some() {
        let extra = (tax * paid as f32).round() as i64;
        ownership::pay(world, owner, None, extra, Flow::Tax);
    }
    book_take(world, b, paid, flow == Flow::Gamble);
    paid
}

/// Undo `pay_venue`'s `SetLeisureTax` surcharge on `paid` (plan L37): the
/// Treasury hands `owner` back the surcharge (`ownership::refund_tax`).
fn refund_surcharge(world: &mut World, owner: Option<EntityId>, paid: i64) {
    let tax = world.levers.leisure_tax;
    if tax > 0.0 && owner.is_some() && paid > 0 {
        let extra = (tax * paid as f32).round() as i64;
        ownership::refund_tax(world, owner, extra);
    }
}

/// The house's take: `take_today` (a table's margin) and, on a front its
/// gang owns, `take_week` (review fix: a seeded front on the city's deed
/// books no week, its coins reached the Treasury).
fn book_take(world: &mut World, b: EntityId, delta: i64, table: bool) {
    let Some(bd) = world.comp_mut::<Building>(b) else { return };
    let gang_owned = bd.venue.as_ref().and_then(|v| v.front_of).is_some_and(|g| bd.owner == Some(g));
    if let Some(v) = bd.venue.as_mut() {
        if table {
            v.take_today += delta;
        }
        if gang_owned {
            v.take_week += delta;
        }
    }
}

/// The house pays a win (`Flow::GambleWin`, untaxed): a corp or the city in
/// full (`charge`), an agent or a gang what it holds (`pay`). A house short
/// of the win shuts its table for the day.
fn house_pays(world: &mut World, b: EntityId, to: EntityId, amount: i64, gross: i64) -> i64 {
    let owner = world.owner_of(b);
    let paid = ownership::charge(world, owner, Some(to), amount, Flow::GambleWin);
    let today = world.day();
    book_take(world, b, -paid, true);
    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
        if paid < amount {
            v.table_shut_day = Some(today);
        }
    }
    ownership::credit(world, b, -paid);
    // Deviation: a win is big at `big_win` or more of the gross return (the
    // stake back and the win): with the spec's `stake_cap` 20 nothing ever
    // paid more than 40, so `> big_win` (40) could never fire.
    let gross = gross - (amount - paid);
    if gross >= world.config.leisure.big_win {
        let text = format!("{} won {gross} at {}", world.name_of(to), world.name_of(b));
        world.push_event(EventKind::Gambled, &[to, b], text);
    }
    paid
}

/// Spec § 1: a stake is `min(stake_frac × coins, stake_cap)`, at least 1
/// and at most what the purse holds.
pub fn stake_of(world: &World, coins: i64) -> i64 {
    let cfg = &world.config.leisure;
    ((cfg.stake_frac * coins as f32).floor() as i64).min(cfg.stake_cap).max(1).min(coins.max(0))
}

fn table_open(world: &World, b: EntityId) -> bool {
    let today = world.day();
    world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).is_none_or(|v| v.table_shut_day != Some(today))
}

/// A FightPit takes bets in the `bet_window_hours` before `bout_hour`.
fn bet_window(world: &World) -> bool {
    let cfg = &world.config.leisure;
    let h = world.tick_of_day() / TICKS_PER_HOUR as u16;
    // Review fix: saturating, so a `bout_hour` below the window cannot wrap.
    h < cfg.bout_hour && cfg.bout_hour.saturating_sub(cfg.bet_window_hours) <= h && !world.levers.ban_gambling
}

/// Plan L18: one table game at the house `b`: the draw on `WordNs::Gamble`
/// keyed `(tick, id)` (or `stream` when given), a loss to the house, a win
/// paid by it. Returns the agent's net coins.
fn play_table(world: &mut World, id: EntityId, b: EntityId, stake: i64, u: f32) -> i64 {
    let p_win = 0.5 - world.config.leisure.house_edge;
    if u < p_win {
        house_pays(world, b, id, stake, 2 * stake)
    } else {
        -pay_venue(world, id, b, stake, Flow::Gamble)
    }
}

// ---------------------------------------------------------------------------
// The rung ladder (plan L14, L17)
// ---------------------------------------------------------------------------

/// A satisfier on offer to one agent.
#[derive(Clone, Debug)]
struct Cand {
    rung: Rung,
    act: ActionKind,
    venue: Option<EntityId>,
    spot: Option<TilePos>,
    score: f32,
}

fn rung_of(world: &World, b: EntityId, price: i64) -> Rung {
    match kind_of(world, b) {
        Some(BuildingKind::Lounge) => Rung::High,
        Some(BuildingKind::Club) if price >= 12 => Rung::High,
        Some(BuildingKind::Club | BuildingKind::FightPit | BuildingKind::Den) => Rung::Mid,
        _ => Rung::Cheap,
    }
}

fn act_of(kind: BuildingKind) -> ActionKind {
    match kind {
        BuildingKind::Bar => ActionKind::Drink,
        BuildingKind::NoodleBar => ActionKind::EatOut,
        BuildingKind::Den => ActionKind::Gamble,
        _ => ActionKind::Enjoy,
    }
}

/// Spec § 2: execs and the top wealth decile reach the high rung.
fn high_ok(world: &World, id: EntityId) -> bool {
    crate::systems::life::exec_corp(world, id).is_some()
        || (world.wealth_p90 > 0 && coins(world, id) >= world.wealth_p90)
}

/// The walk's factor (`life::travel_factor`; 1 with the life pass off).
fn travel(world: &World, tiles: u32) -> f32 {
    if !crate::systems::life::on(world) {
        return 1.0;
    }
    crate::systems::life::travel_factor(world, crate::systems::life::walk_ticks(world, tiles))
}

/// A paid candidate's score: `gain × travel × pride (mid, high) × courage
/// (FightPit, Den) × (1 − greed × price ÷ purse)`.
fn paid_score(world: &World, id: EntityId, b: EntityId, rung: Rung, price: i64, purse: i64, tiles: u32) -> f32 {
    let p = world.comp::<Personality>(id);
    let (pride, courage, greed) = p.map_or((0.5, 0.5, 0.5), |p| (p.pride, p.courage, p.greed));
    let mut s = gain_at(world, b) * travel(world, tiles);
    if matches!(rung, Rung::Mid | Rung::High) {
        s *= 1.0 + 0.5 * pride;
    }
    if matches!(kind_of(world, b), Some(BuildingKind::FightPit | BuildingKind::Den)) {
        let c = 0.5 + courage;
        s *= if courage >= world.config.leisure.gamble_rung_courage { c } else { 0.5 * c };
    }
    let share = price.max(1) as f32 / purse.max(1) as f32;
    s * (1.0 - greed * share).max(0.05)
}

/// The kinds the ladder offers, cheap to dear.
const LADDER: [BuildingKind; 7] = [
    BuildingKind::Bar,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
    BuildingKind::Club,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Lounge,
];

/// The nearest open venue of `kind` `id` may enter from `from` (door
/// Manhattan, ties the lower id), restricted to `district` when given.
fn nearest_venue(
    world: &World,
    id: EntityId,
    kind: BuildingKind,
    from: TilePos,
    district: Option<DistrictId>,
    held: &BTreeMap<EntityId, usize>,
) -> Option<(u32, EntityId)> {
    world
        .buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| {
            standing(world, b)
                && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
        })
        .filter(|&b| district.is_none_or(|d| world.district_of_building(b) == d))
        .filter(|&b| open(world, b) && door_ok(world, id, b) && has_room(world, id, b, held))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
}

/// The paid rungs open to `id` from `from` with `purse` to spend.
fn paid_candidates(world: &World, id: EntityId, from: TilePos, purse: i64, district: Option<DistrictId>) -> Vec<Cand> {
    let mut out = Vec::new();
    if purse < 1 {
        return out;
    }
    let drank = crate::utility::goals::drank_today(world, id);
    let hungry = world.comp::<Needs>(id).is_some_and(|n| n.hunger < 0.5);
    let high = high_ok(world, id);
    let held = held_by_others(world, id);
    for kind in LADDER {
        if kind == BuildingKind::Bar && drank {
            continue;
        }
        if kind == BuildingKind::NoodleBar && !hungry {
            continue;
        }
        let pick = nearest_venue(world, id, kind, from, district, &held)
            .or_else(|| district.and_then(|_| nearest_venue(world, id, kind, from, None, &held)));
        let Some((tiles, b)) = pick else { continue };
        let price = price_of(world, b);
        // The rung's affordability (spec § 2's brake): at most half the purse above
        // the reserve goes on one evening.
        if price <= 0 || 2 * price > purse {
            continue;
        }
        if kind == BuildingKind::NoodleBar && world.comp::<Building>(b).is_none_or(|bd| bd.stock_food == 0) {
            continue;
        }
        if kind == BuildingKind::Den && !table_open(world, b) {
            continue;
        }
        let rung = rung_of(world, b, price);
        if rung == Rung::High && !high {
            continue;
        }
        let score = paid_score(world, id, b, rung, price, purse, tiles);
        out.push(Cand { rung, act: act_of(kind), venue: Some(b), spot: None, score });
    }
    out
}

/// The free show: a live raid's door or a FightPit's bout within
/// `watch_tiles` of `from` (the street outside it).
fn watch_tile(world: &World, from: TilePos) -> Option<TilePos> {
    let reach = world.config.leisure.watch_tiles;
    let mut best: Option<(u32, TilePos)> = None;
    let mut consider = |t: TilePos| {
        let d = t.manhattan(from);
        if d <= reach && best.is_none_or(|(bd, bt)| (d, t) < (bd, bt)) {
            best = Some((d, t));
        }
    };
    for &g in world.gang_list() {
        if world.comp::<Gang>(g).is_some_and(|x| x.raid_at.is_some()) {
            if let Some(t) = crate::systems::raid::gang_target(world, g)
                .and_then(|b| world.comp::<Building>(b))
                .map(|bd| world.outside_door(bd))
            {
                consider(t);
            }
        }
    }
    let hour = world.tick_of_day() / TICKS_PER_HOUR as u16;
    if hour + 1 >= world.config.leisure.bout_hour && hour <= world.config.leisure.bout_hour {
        for &b in world.buildings_of_kind(BuildingKind::FightPit) {
            if open(world, b) {
                if let Some(bd) = world.comp::<Building>(b) {
                    consider(world.outside_door(bd));
                }
            }
        }
    }
    best.map(|(_, t)| t)
}

/// The walk origin of `id` (outside its building, else its tile).
fn origin(world: &World, id: EntityId) -> Option<TilePos> {
    crate::exec::walk_origin(world, id)
}

/// Plan L14: the rung `id` would take now, scored as Unwind scores it:
/// the paid venues (nearest open of each kind), the free show and the best
/// HangOut spot. `scored` runs the full spot scoring (`hunt::habit` over
/// the contacts, at plan time); unscored picks the nearest spot (think).
pub fn choice(world: &World, id: EntityId, scored: bool) -> Option<UnwindPick> {
    let from = origin(world, id)?;
    let purse = spendable(world, id);
    let mut cands = paid_candidates(world, id, from, purse, None);
    // Spec § 2 (stat_daily's rule, kept on screen for parity): a hungry
    // agent's evening is the NoodleBar's meal when one is in reach.
    if let Some(c) = cands.iter().find(|c| c.act == ActionKind::EatOut) {
        return Some(pick_of(c));
    }
    let g = &world.config.leisure.gain;
    if let Some(t) = watch_tile(world, from) {
        let score = g.watch * travel(world, t.manhattan(from));
        cands.push(Cand { rung: Rung::Free, act: ActionKind::HangOut, venue: None, spot: Some(t), score });
    }
    let spot = if scored { best_spot(world, id, from) } else { nearest_spot(world, id, from) };
    if let Some((t, social)) = spot {
        let hours = world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        let score = (g.hangout_hour * hours + 0.1 * social.min(1.0)) * travel(world, t.manhattan(from));
        cands.push(Cand { rung: Rung::Free, act: ActionKind::HangOut, venue: None, spot: Some(t), score });
    }
    let best = cands.iter().max_by(|a, b| {
        a.score.total_cmp(&b.score).then_with(|| b.venue.cmp(&a.venue)).then_with(|| b.spot.cmp(&a.spot))
    })?;
    Some(pick_of(best))
}

fn pick_of(c: &Cand) -> UnwindPick {
    UnwindPick { rung: c.rung, act: c.act, venue: c.venue, spot: c.spot, score: c.score }
}

// ---------------------------------------------------------------------------
// Spots (plan L15, L16)
// ---------------------------------------------------------------------------

/// Plan L15, midnight (and once lazily): each district's spots, ≤ 24: the
/// street outside the doors of its Bars, Markets, Clubs and NoodleBars, each
/// gang's Hideout door, and in a Sump district `barrels_per_sump` fire
/// barrels on street tiles within 6 of a Home door (`splitmix64`).
pub fn spots_daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let n = world.districts.len();
    let mut all: Vec<Vec<Spot>> = vec![Vec::new(); n];
    for kind in [BuildingKind::Bar, BuildingKind::Market, BuildingKind::Club, BuildingKind::NoodleBar] {
        for &b in world.buildings_of_kind(kind) {
            if !standing(world, b) {
                continue;
            }
            let Some(bd) = world.comp::<Building>(b) else { continue };
            let d = world.district_of_building(b).index();
            if let Some(v) = all.get_mut(d) {
                v.push(Spot { tile: world.outside_door(bd), kind: SpotKind::Door(b) });
            }
        }
    }
    for &g in world.gang_list() {
        let Some(h) = world.hideout_of(g) else { continue };
        let Some(bd) = world.comp::<Building>(h).filter(|bd| !bd.demolished) else { continue };
        let d = world.district_of_building(h).index();
        if let Some(v) = all.get_mut(d) {
            v.push(Spot { tile: world.outside_door(bd), kind: SpotKind::Hideout(g) });
        }
    }
    let w = world.map.w();
    let k_max = world.config.leisure.barrels_per_sump as u64;
    for (i, d) in world.districts.iter().enumerate() {
        if d.zone != Zone::Sump || k_max == 0 {
            continue;
        }
        let doors: Vec<TilePos> = d
            .homes
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| b.door))
            .collect();
        let near: Vec<TilePos> = d
            .streets
            .iter()
            .map(|&ix| TilePos { x: (ix as usize % w) as u8, y: (ix as usize / w) as u8 })
            .filter(|&t| doors.iter().any(|&dd| dd.manhattan(t) <= BARREL_REACH))
            .collect();
        if near.is_empty() {
            continue;
        }
        let mut taken: Vec<TilePos> = Vec::new();
        for k in 0..k_max {
            let ix = (splitmix64(world.seed() ^ (i as u64).rotate_left(17) ^ (k + 1).rotate_left(41))
                % near.len() as u64) as usize;
            let t = near[ix];
            if !taken.contains(&t) {
                taken.push(t);
            }
        }
        if let Some(v) = all.get_mut(i) {
            v.extend(taken.into_iter().map(|t| Spot { tile: t, kind: SpotKind::Barrel }));
        }
    }
    for v in &mut all {
        v.sort_by_key(|s| (s.tile, s.kind));
        v.dedup_by_key(|s| s.tile);
        v.truncate(MAX_SPOTS);
    }
    world.spots = all;
}

/// May `id` hang out at `s` (a Hideout's door is its members' only)?
fn spot_ok(world: &World, id: EntityId, s: &Spot) -> bool {
    match s.kind {
        SpotKind::Hideout(g) => world.gang_of(id) == Some(g),
        _ => true,
    }
}

/// The spots of `id`'s district (its tile's), and of its Home's.
fn spots_near(world: &World, id: EntityId, from: TilePos) -> Vec<Spot> {
    let mut ds: SmallVec<[DistrictId; 2]> = SmallVec::new();
    ds.push(world.district_of(from));
    if let Some(d) = world.comp::<Household>(id).and_then(|h| h.home).map(|h| world.district_of_building(h)) {
        if !ds.contains(&d) {
            ds.push(d);
        }
    }
    let mut out: Vec<Spot> = Vec::new();
    for d in ds {
        if let Some(v) = world.spots.get(d.index()) {
            out.extend(v.iter().copied().filter(|s| spot_ok(world, id, s)));
        }
    }
    out
}

/// The nearest spot `id` may use (think time), with a 0 social score.
fn nearest_spot(world: &World, id: EntityId, from: TilePos) -> Option<(TilePos, f32)> {
    spots_near(world, id, from).into_iter().map(|s| (s.tile.manhattan(from), s.tile)).min().map(|(_, t)| (t, 0.0))
}

/// Plan L16: `id`'s top contacts (≤ 6): kin and Friends first, then
/// gang-mates, by affinity.
fn contacts(world: &World, id: EntityId) -> Vec<(EntityId, f32, bool)> {
    let gang = world.gang_of(id);
    let mut v: Vec<(u8, f32, EntityId, bool)> = world
        .neighbours(id)
        .filter(|&c| world.has::<Brain>(c) && !world.has::<Sentence>(c))
        .filter_map(|c| {
            let e = world.edge(id, c)?;
            let kin = matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent);
            let rank = match e.kind {
                RelKind::Spouse | RelKind::Family | RelKind::Parent => 0,
                RelKind::Friend => 1,
                _ if gang.is_some() && world.gang_of(c) == gang => 2,
                _ => return None,
            };
            (e.affinity > 0.0).then_some((rank, e.affinity, c, kin))
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
    v.truncate(MAX_CONTACTS);
    v.into_iter().map(|(_, a, c, kin)| (c, a, kin)).collect()
}

fn pair(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Plan L16: the best spot for `id` and its social score: `Σ affinity ×
/// P(contact there within the hour)` (`hunt::habit` at `now + 60`: its
/// building's outside door 1.0, its district 0.5; present now +1.0), a kin
/// term `kin_w × min(7, days since they met) × P`, a Call's `call_w` on the
/// own Hideout's spot, times the walk; ties the nearer, then the lower tile.
pub fn best_spot(world: &World, id: EntityId, from: TilePos) -> Option<(TilePos, f32)> {
    let spots = spots_near(world, id, from);
    if spots.is_empty() {
        return None;
    }
    let now = world.tick;
    let today = world.day();
    let cs = contacts(world, id);
    let habits: Vec<(EntityId, f32, bool, Option<TilePos>, DistrictId)> = cs
        .iter()
        .map(|&(c, a, kin)| {
            let i = crate::systems::hunt::habit(world, c, now + TICKS_PER_HOUR);
            let door = i.building.and_then(|b| world.comp::<Building>(b)).map(|bd| world.outside_door(bd));
            (c, a, kin, door, world.district_of(i.tile))
        })
        .collect();
    let gang = world.gang_of(id);
    let call = gang.and_then(|g| world.comp::<Gang>(g)).and_then(|g| g.call_until).is_some_and(|t| t > now);
    let cfg = &world.config.leisure;
    spots
        .iter()
        .map(|s| {
            let sd = world.district_of(s.tile);
            let present = world.hangouts.get(&s.tile);
            let mut social = 0.0f32;
            for &(c, a, kin, door, d) in &habits {
                let mut p = if door == Some(s.tile) {
                    1.0
                } else if d == sd {
                    0.5
                } else {
                    0.0
                };
                if present.is_some_and(|v| v.contains(&c)) {
                    p += 1.0;
                }
                social += a * p;
                if kin && p > 0.0 {
                    let days = world.last_met.get(&pair(id, c)).map_or(7, |&d| today.saturating_sub(d)).min(7);
                    social += cfg.kin_w * days as f32 * p.min(1.0);
                }
            }
            if call
                && matches!(s.kind, SpotKind::Hideout(g) if Some(g) == gang)
                && s.tile.manhattan(from) <= cfg.call_tiles
            {
                social += cfg.call_w;
            }
            let d = s.tile.manhattan(from);
            ((0.5 + social) * travel(world, d), d, s.tile, social)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)))
        .map(|(_, _, t, social)| (t, social))
}

// ---------------------------------------------------------------------------
// Unwind: considerations and plans (plan L14, L16)
// ---------------------------------------------------------------------------

fn adult_free(world: &World, id: EntityId) -> bool {
    crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// Spec § 2: Unwind is satisfied at `fun_satisfied` (and off for a child,
/// a prisoner, or with leisure off).
pub fn unwind_satisfied(world: &World, id: EntityId) -> bool {
    !on(world)
        || !adult_free(world, id)
        || world.comp::<Needs>(id).is_none_or(|n| n.fun >= world.config.needs.fun_satisfied)
}

/// Off shift in the Evening or Night (an exec once the office has shut).
fn off_hours(world: &World, id: EntityId) -> bool {
    let phase = world.phase();
    if !matches!(phase, DayPhase::Evening | DayPhase::Night) {
        return false;
    }
    if let Some(j) = world.comp::<Job>(id) {
        return !j.on_shift(world.tick_of_day());
    }
    crate::systems::life::exec_corp(world, id).is_none() || crate::systems::life::exec_shift_over(world)
}

/// Plan L14 (spec § 2): `U(fun)` Logistic{8, 0.5}; Evening and Night off
/// shift, else `gate_or(0.3)`; the best rung's score (gain × the walk ×
/// pride on the mid and high rungs × courage at the FightPit and Den ×
/// greed-weighted affordability). `None`: no rung reachable.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if unwind_satisfied(world, id) {
        return None;
    }
    let n = world.comp::<Needs>(id)?;
    let pick = choice(world, id, false)?;
    let cs = vec![
        Consideration::new("U(fun)", urgency(n.fun), Curve::Logistic { k: 8.0, mid: 0.5 }),
        Consideration::new("off hours", can(off_hours(world, id)), gate_or(0.3)),
        Consideration::raw("rung", pick.score, (0.5 + pick.score).min(1.0)),
        // Deviation (measured): food before fun. Without it a hungry body
        // took the evening at a spot or a venue over its meal (fun decay off:
        // seed 42/43 starvation 22/16; on: 23-45; e8355d4 12/25).
        Consideration::new("fed", n.hunger, Curve::Logistic { k: 12.0, mid: 0.4 }),
    ];
    Some((cs, 0.0))
}

fn step(action: ActionKind, target: Option<EntityId>) -> ActionInstance {
    ActionInstance { action, target, tile: None }
}

/// The steps of a pick: the walk, then the satisfier (a Den's drink first,
/// a FightPit's bet in the bet window).
fn steps_for(world: &World, id: EntityId, pick: &UnwindPick) -> Vec<ActionInstance> {
    let mut steps = Vec::new();
    match (pick.venue, pick.spot) {
        (Some(v), _) => {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(v)));
            let kind = kind_of(world, v);
            let purse = spendable(world, id) - price_of(world, v);
            if kind == Some(BuildingKind::Den) && purse >= 3 && !crate::utility::goals::drank_today(world, id) {
                steps.push(step(ActionKind::Drink, Some(v)));
            }
            steps.push(step(pick.act, Some(v)));
            if kind == Some(BuildingKind::FightPit) && bet_window(world) && purse >= 1 {
                let brave = world
                    .comp::<Personality>(id)
                    .is_some_and(|p| p.courage >= world.config.leisure.gamble_rung_courage);
                if brave {
                    steps.push(step(ActionKind::Gamble, Some(v)));
                }
            }
        }
        (None, Some(_)) => {
            steps.push(step(ActionKind::GoTo(LocationKey::Spot), None));
            steps.push(step(pick.act, None));
        }
        (None, None) => {}
    }
    steps
}

/// Plan L14: the Unwind plan from the pick made now (deviation: the pick is
/// computed again at plan time with the spots scored, and cached in
/// `World::unwind` for the executor, rather than carried from the think).
pub fn unwind_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if unwind_satisfied(world, id) {
        return None;
    }
    let pick = choice(world, id, true)?;
    let steps = steps_for(world, id, &pick);
    if steps.is_empty() {
        return None;
    }
    let target = pick.venue;
    world.unwind.insert(id, pick);
    // Plan L30: with the LOD budget on, a Seat at the venue is held from the
    // plan to the arrival (`unwind_plan` installs without `reserve_for`).
    if let Some(v) = target.filter(|_| crate::systems::lod::budget_on(world)) {
        let expires = world.tick + world.config.exec.reservation_ttl;
        world.reserve(id, crate::exec::ReservationKind::Seat { building: v }, expires);
    }
    Some(Plan { goal: GoalKind::Unwind, target, steps, started_tick: world.tick })
}

/// Plan L16: a Socialise with nobody to talk to here goes to a HangOut
/// when broke (< 2 coins) or when the best spot outscores the local Bar's
/// walk-weighted 1.0. `None`: the planner as before.
pub fn hangout_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !on(world) || crate::systems::social::best_colocated_partner(world, id, -1.0, false).is_some() {
        return None;
    }
    let from = origin(world, id)?;
    let (tile, social) = best_spot(world, id, from)?;
    let spot_score = (0.5 + social) * travel(world, tile.manhattan(from));
    let bar = world.local(id, BuildingKind::Bar).and_then(|b| crate::systems::life::tiles_to(world, id, b));
    let bar_score = bar.map_or(0.0, |t| travel(world, t));
    if coins(world, id) >= 2 && spot_score < bar_score {
        return None;
    }
    let pick =
        UnwindPick { rung: Rung::Free, act: ActionKind::HangOut, venue: None, spot: Some(tile), score: spot_score };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::HangOut, None)];
    world.unwind.insert(id, pick);
    Some(Plan { goal: GoalKind::Socialise, target: None, steps, started_tick: world.tick })
}

/// Plan L16: `already_satisfied(Socialise)`'s "no drink possible" clause is
/// dropped when a spot is within `hangout_reach`.
pub fn spot_in_reach(world: &World, id: EntityId) -> bool {
    on(world)
        && origin(world, id).is_some_and(|from| {
            spots_near(world, id, from).iter().any(|s| s.tile.manhattan(from) <= world.config.leisure.hangout_reach)
        })
}

/// `LocationKey::Spot`: the spot of the agent's pick.
pub fn spot_of(world: &World, id: EntityId) -> Option<TilePos> {
    world.unwind.get(&id).and_then(|p| p.spot)
}

/// Plan L14 (spec § 2): `Earn` reads `U(fun)` as a second term on Beg and
/// Scavenge when fun is below 0.3 and the purse below the cheap rung (the
/// two actions' plan cost, `PlanCtx::fun_term`; review fix: not the goal).
pub fn earn_fun_term(world: &World, id: EntityId) -> Option<f32> {
    if !on(world) {
        return None;
    }
    let fun = world.comp::<Needs>(id)?.fun;
    (fun < 0.3 && coins(world, id) < 2).then(|| urgency(fun))
}

// ---------------------------------------------------------------------------
// Execution (plan L14, L15, L18)
// ---------------------------------------------------------------------------

/// Plan L14: a leisure step may start (money is re-checked here, so a
/// refusal moves none).
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    if !on(world) {
        return false;
    }
    let inside = here(world, id);
    let at_venue = |b: EntityId| inside == Some(b) && open(world, b) && door_ok(world, id, b);
    match kind {
        ActionKind::Enjoy => {
            let Some(b) = target else { return false };
            at_venue(b) && price_of(world, b) > 0 && coins(world, id) >= price_of(world, b)
        }
        ActionKind::EatOut => {
            let Some(b) = target else { return false };
            at_venue(b)
                && kind_of(world, b) == Some(BuildingKind::NoodleBar)
                && world.comp::<Building>(b).is_some_and(|bd| bd.stock_food > 0)
                && coins(world, id) >= price_of(world, b).max(1)
        }
        ActionKind::Gamble => {
            let Some(b) = target else { return false };
            let ok_kind = match kind_of(world, b) {
                Some(BuildingKind::Den) => table_open(world, b),
                Some(BuildingKind::FightPit) => bet_window(world),
                _ => false,
            };
            at_venue(b) && ok_kind && coins(world, id) >= 1
        }
        ActionKind::HangOut | ActionKind::Preach => world.comp::<Position>(id).is_some_and(|p| p.building.is_none()),
        ActionKind::Collect => {
            let Some(gang) = world.gang_of(id) else { return false };
            world.comp::<Gang>(gang).is_some_and(|g| g.leader == Some(id)) && inside.is_some() && inside == target
        }
        _ => false,
    }
}

/// Plan L14, at start (money moves now): the entry or the meal (refunded on
/// abort through `World::pending_purchase`), the table game or the bet, the
/// HangOut registry.
pub fn on_start(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>) {
    match kind {
        ActionKind::Enjoy | ActionKind::EatOut => {
            let Some(b) = target else { return };
            let price = price_of(world, b);
            let paid = pay_venue(world, id, b, price, Flow::Leisure);
            let mut units = 0;
            if kind == ActionKind::EatOut && world.take_stock(b, crate::components::Good::Food, 1) == 1 {
                units = 1;
            }
            world.pending_purchase.insert(id, (units, paid));
        }
        ActionKind::Gamble => {
            let Some(b) = target else { return };
            let stake = stake_of(world, spendable(world, id).max(1).min(coins(world, id)));
            if kind_of(world, b) == Some(BuildingKind::FightPit) {
                let paid = pay_venue(world, id, b, stake, Flow::Gamble);
                if paid > 0 {
                    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
                        v.bets.push((id, paid, false));
                    }
                }
            } else {
                let now = world.tick;
                let u: f32 = world.rng.word(WordNs::Gamble, now, u64::from(id.index)).random();
                play_table(world, id, b, stake, u);
            }
        }
        ActionKind::HangOut | ActionKind::Preach => {
            let Some(t) = world.comp::<Position>(id).map(|p| p.tile) else { return };
            let list = world.hangouts.entry(t).or_default();
            if !list.contains(&id) {
                list.push(id);
            }
        }
        _ => {}
    }
}

/// Leave the HangOut registry (a step's end, an abort; review fix: also a
/// death and an emigration, which drop the Brain without `abort_plan`).
pub fn leave_spot(world: &mut World, id: EntityId) {
    if world.hangouts.is_empty() {
        return;
    }
    let t = world.comp::<Position>(id).map(|p| p.tile);
    let mut emptied = None;
    if let Some(list) = t.and_then(|t| world.hangouts.get_mut(&t)) {
        list.retain(|x| *x != id);
        if list.is_empty() {
            emptied = t;
        }
    } else {
        for list in world.hangouts.values_mut() {
            list.retain(|x| *x != id);
        }
        world.hangouts.retain(|_, l| !l.is_empty());
    }
    if let Some(t) = emptied {
        world.hangouts.remove(&t);
    }
}

/// Plan L14: an abandoned Enjoy or meal is refunded (`ownership::refund`),
/// the meal's unit back in stock; a HangOut leaves the registry.
pub fn on_abort(world: &mut World, id: EntityId, kind: ActionKind) {
    match kind {
        ActionKind::Enjoy | ActionKind::EatOut => {
            let Some((units, paid)) = world.pending_purchase.remove(&id) else { return };
            let b = world
                .comp::<Brain>(id)
                .and_then(|b| b.current_step())
                .and_then(|s| s.target)
                .or_else(|| here(world, id));
            let Some(b) = b else { return };
            let owner = world.owner_of(b);
            ownership::refund(world, id, owner, paid, Flow::Leisure);
            ownership::credit(world, b, -paid);
            book_take(world, b, -paid, false);
            // Review fix: the `SetLeisureTax` surcharge comes back too.
            refund_surcharge(world, owner, paid);
            if units > 0 {
                world.add_stock(b, crate::components::Good::Food, units);
            }
        }
        ActionKind::HangOut | ActionKind::Preach => leave_spot(world, id),
        _ => {}
    }
}

/// Plan L14, L15, L20: completion effects.
pub fn on_complete(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>, started: Tick) {
    match kind {
        ActionKind::Enjoy => {
            world.pending_purchase.remove(&id);
            let Some(b) = target else { return };
            let gain = gain_at(world, b);
            add_fun(world, id, gain);
            add_belonging(world, id, belonging_at(world, b));
            world.remember(id, MemoryKind::Enjoyed, None, 0.2, 0.3, false);
            crate::systems::jobs::note_visit(world, b);
        }
        ActionKind::EatOut => {
            let (units, _) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            let Some(b) = target else { return };
            if units > 0 {
                let cfg = world.config.needs.clone();
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    crate::needs::eat(n, &cfg);
                }
                world.mark_day(id, crate::components::trace_flags::ATE);
                world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            }
            add_fun(world, id, gain_at(world, b));
            crate::systems::jobs::note_visit(world, b);
        }
        ActionKind::Gamble => {
            let Some(b) = target else { return };
            // A FightPit's bettor had the pit's evening already (its Enjoy).
            if kind_of(world, b) == Some(BuildingKind::Den) {
                add_fun(world, id, gain_at(world, b));
                world.remember(id, MemoryKind::Enjoyed, None, 0.2, 0.3, false);
                crate::systems::jobs::note_visit(world, b);
            }
        }
        ActionKind::HangOut => hangout_done(world, id, started),
        ActionKind::Preach => {
            preach_done(world, id);
            leave_spot(world, id);
        }
        ActionKind::Collect => collect_step(world, id, target),
        _ => {}
    }
}

/// Plan L15: a HangOut ends: fun by the hour, `chat_belonging`, and one
/// exchange with the co-hanger of highest affinity (else the first present,
/// ties the lower id): `social::interacted` (a first meeting if strangers;
/// its noise from a `splitmix64` hash), the legacy `social::gossip` when it
/// is the live path, `gossip::exchange(.., Chat)`; kin note the day they
/// met; street dice between two bored co-hangers.
fn hangout_done(world: &mut World, id: EntityId, started: Tick) {
    let tile = world.comp::<Position>(id).map(|p| p.tile);
    let hours = world.tick.saturating_sub(started) as f32 / TICKS_PER_HOUR as f32;
    let gain = world.config.leisure.gain.hangout_hour * hours.max(0.5);
    add_fun(world, id, gain);
    let chat = world.config.needs.chat_belonging;
    add_belonging(world, id, chat);
    let others: Vec<EntityId> = tile
        .and_then(|t| world.hangouts.get(&t))
        .map(|v| v.iter().copied().filter(|&o| o != id && world.has::<Brain>(o)).collect())
        .unwrap_or_default();
    leave_spot(world, id);
    let known = others.iter().filter(|&&o| world.edge(id, o).is_some()).count();
    let row = &mut world.stats.current.living;
    // Running mean of known co-hangers per completed HangOut.
    let n = row.hangouts as f32;
    row.hangout_contacts_mean = (row.hangout_contacts_mean * n + known as f32) / (n + 1.0);
    row.hangouts += 1;
    let partner = others
        .iter()
        .copied()
        .map(|o| (o, world.edge(id, o).map_or(f32::NEG_INFINITY, |e| e.affinity)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(o, _)| o);
    let Some(p) = partner else { return };
    if world.edge(id, p).is_none() {
        let (sa, sb) = (
            world.comp::<Personality>(id).map_or(0.5, |x| x.sociability),
            world.comp::<Personality>(p).map_or(0.5, |x| x.sociability),
        );
        let (a, b) = pair(id, p);
        let h = splitmix64(world.seed() ^ world.tick.rotate_left(13) ^ (u64::from(a.index) << 32 | u64::from(b.index)));
        let u1 = ((h >> 40) as f32 + 0.5) / (1u64 << 24) as f32;
        let u2 = ((h & 0xFF_FFFF) as f32 + 0.5) / (1u64 << 24) as f32;
        let look = crate::systems::social::first_look(world, id, p);
        let aff = crate::systems::social::first_affinity_with(sa, sb, u1, u2, look);
        crate::systems::social::first_meeting(world, id, p, aff);
    } else {
        crate::systems::social::interacted(world, id, p);
    }
    let legacy = !world.config.gossip.enabled || world.config.gossip.legacy_second_hand;
    if legacy {
        crate::systems::social::gossip(world, id, p);
    }
    crate::systems::gossip::exchange(world, id, p, crate::systems::gossip::Venue::Chat);
    if world.edge(id, p).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent)) {
        let today = world.day();
        world.last_met.insert(pair(id, p), today);
    }
    street_dice(world, id, p);
}

/// Spec § 2: street dice: two co-hangers with fun below 0.5 and a coin each
/// play for 1 coin (`WordNs::Gamble`, key `(tick, a << 32 | b)`; agent to
/// agent, `Flow::StreetDice`).
fn street_dice(world: &mut World, a: EntityId, b: EntityId) {
    let bored = |w: &World, x: EntityId| w.comp::<Needs>(x).is_some_and(|n| n.fun < 0.5) && coins(w, x) >= 1;
    if !bored(world, a) || !bored(world, b) {
        return;
    }
    let (lo, hi) = pair(a, b);
    let u: f32 = world.rng.word(WordNs::Gamble, world.tick, u64::from(lo.index) << 32 | u64::from(hi.index)).random();
    let (winner, loser) = if u < 0.5 { (lo, hi) } else { (hi, lo) };
    ownership::pay(world, Some(loser), Some(winner), 1, Flow::StreetDice);
    for x in [a, b] {
        add_fun(world, x, 0.05);
    }
}

/// Plan L15: who hangs out where, from the bodies' running steps (on load).
pub fn rebuild_hangouts(world: &mut World) {
    world.hangouts.clear();
    let mut map: std::collections::BTreeMap<TilePos, SmallVec<[EntityId; 8]>> = Default::default();
    for id in world.bodies() {
        let running = world
            .comp::<Brain>(id)
            .is_some_and(|b| matches!(b.exec, ExecState::Use { kind: ActionKind::HangOut | ActionKind::Preach, .. }));
        if !running {
            continue;
        }
        if let Some(t) = world.comp::<Position>(id).map(|p| p.tile) {
            map.entry(t).or_default().push(id);
        }
    }
    world.hangouts = map;
}

// ---------------------------------------------------------------------------
// The nightly bout (plan L18)
// ---------------------------------------------------------------------------

/// Plan L18 (deviation: the bout is resolved here with `resolve_fight`'s
/// `p_win` and no death, its draw on `WordNs::Bout`; `resolve_fight` rolls
/// on the world stream, L31). At `bout_hour` each open FightPit pairs its two
/// fittest Fighters on shift (ties the lower id): the loser loses 0.3
/// energy and safety and remembers `Lost`; tonight's bets settle (a bettor
/// on the winner's side is paid twice the stake by the house); the sides
/// are drawn on the same stream per bettor by courage. A pit without two
/// Fighters refunds its bets.
pub fn bouts(world: &mut World) {
    let day = world.day();
    let tod = world.tick_of_day();
    for pit in world.buildings_of_kind(BuildingKind::FightPit).to_vec() {
        // Review fix: a pit that fell refunds its stranded bets too.
        let bets =
            world.comp_mut::<Building>(pit).and_then(|bd| bd.venue.as_mut()).map(|v| std::mem::take(&mut v.bets));
        let bets = bets.unwrap_or_default();
        let mut fighters: Vec<(f32, EntityId)> = ownership::staff_at(world, pit)
            .into_iter()
            .filter(|&f| world.comp::<Job>(f).is_some_and(|j| j.role == Role::Fighter && j.on_shift(tod)))
            .filter(|&f| crate::systems::law::living(world, f) && !world.has::<Sentence>(f))
            .map(|f| (crate::systems::law::fighting(world, f), f))
            .collect();
        fighters.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        if fighters.len() < 2 || !open(world, pit) || !standing(world, pit) {
            let owner = world.owner_of(pit);
            for (who, stake, _) in bets {
                if world.has::<Wallet>(who) {
                    ownership::refund(world, who, owner, stake, Flow::Gamble);
                    ownership::credit(world, pit, -stake);
                    book_take(world, pit, -stake, true);
                    // Review fix: the bet's `SetLeisureTax` surcharge comes back, as `on_abort`.
                    refund_surcharge(world, owner, stake);
                }
            }
            continue;
        }
        let (a, b) = (fighters[0].1, fighters[1].1);
        let (fa, fb) = (fighters[0].0, fighters[1].0);
        let (ca, cb) = (crate::systems::law::courage(world, a), crate::systems::law::courage(world, b));
        let p_win = (0.5 + 0.4 * (fa - fb) + 0.1 * (ca - cb)).clamp(0.1, 0.9);
        let mut rng = world.rng.word(WordNs::Bout, day, u64::from(pit.index));
        let a_wins = rng.random::<f32>() < p_win;
        let (winner, loser) = if a_wins { (a, b) } else { (b, a) };
        if let Some(n) = world.comp_mut::<Needs>(loser) {
            n.energy = (n.energy - 0.3).max(0.0);
            n.safety = (n.safety - 0.3).max(0.0);
        }
        world.remember(winner, MemoryKind::Won, Some(loser), 0.4, 0.3, false);
        world.remember(loser, MemoryKind::Lost, Some(winner), 0.5, -0.4, false);
        if let Some(s) = world.comp_mut::<crate::components::Skills>(winner) {
            s.fighting = (s.fighting + 0.01).min(1.0);
        }
        let mut paid_out = 0;
        for (who, stake, _) in bets {
            let courage = world.comp::<Personality>(who).map_or(0.5, |p| p.courage);
            let backs_a = rng.random::<f32>() < 0.3 + 0.4 * courage;
            if backs_a == a_wins && world.has::<Wallet>(who) {
                paid_out += house_pays(world, pit, who, 2 * stake, 2 * stake);
            }
        }
        let text = format!(
            "{} beat {} at {} (bets paid {paid_out})",
            world.name_of(winner),
            world.name_of(loser),
            world.name_of(pit)
        );
        world.push_event(EventKind::Bout, &[winner, loser, pit], text);
    }
}

// ---------------------------------------------------------------------------
// The Statistical evening (plan L17)
// ---------------------------------------------------------------------------

/// Plan L17, 21:00: every Statistical adult below `fun_satisfied`,
/// ascending, takes the rung `choice` would (from its Home's door, its
/// Home's district first): pays as on screen, gains the rung's fun, adds a
/// visit; a table game's draw on `WordNs::Leisure` keyed `(day, id)`; the
/// free rung adds `free_stat`.
pub fn stat_daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let day = world.day();
    let ids: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    for id in ids {
        if unwind_satisfied(world, id) {
            continue;
        }
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let from = home
            .and_then(|h| world.comp::<Building>(h))
            .map(|b| world.outside_door(b))
            .or_else(|| world.comp::<Position>(id).map(|p| p.tile));
        let Some(from) = from else { continue };
        let district = Some(world.district_of(from));
        let purse = spendable(world, id);
        let cands = paid_candidates(world, id, from, purse, district);
        let pick = cands.iter().find(|c| c.act == ActionKind::EatOut).cloned().or_else(|| {
            cands.iter().max_by(|a, b| a.score.total_cmp(&b.score).then_with(|| b.venue.cmp(&a.venue))).cloned()
        });
        let g = world.config.leisure.gain.clone();
        // The free rung's on-screen score (an hour's HangOut, no walk).
        let free_score = g.hangout_hour * world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        let Some(c) = pick.filter(|c| c.score > free_score) else {
            add_fun(world, id, g.free_stat);
            continue;
        };
        let Some(b) = c.venue else { continue };
        let mut rng = world.rng.word(WordNs::Leisure, day, u64::from(id.index));
        // Phase 5: the gate's probe (not a CSV column): the evening's gross
        // spend, entries, meals, drinks and stakes.
        let mut gross = 0i64;
        match c.act {
            ActionKind::Drink => {
                let owner = world.owner_of(b);
                let paid = ownership::pay(world, Some(id), owner, 2, Flow::Drink);
                ownership::credit(world, b, paid);
                gross += paid;
                add_fun(world, id, gain_at(world, b));
                add_belonging(world, id, 0.15);
                world.remember(id, MemoryKind::Socialised, None, 0.2, 0.3, false);
            }
            ActionKind::Gamble => {
                let stake = stake_of(world, purse);
                let u: f32 = rng.random();
                play_table(world, id, b, stake, u);
                gross += stake;
                add_fun(world, id, gain_at(world, b));
                crate::systems::jobs::note_visit(world, b);
            }
            ActionKind::EatOut => {
                // Review fix: the shelf first (emptied earlier in this pass,
                // there is no meal: no pay, no fun, no visit), then the pay.
                if world.comp::<Building>(b).is_none_or(|bd| bd.stock_food == 0) {
                    continue;
                }
                let price = price_of(world, b);
                let paid = pay_venue(world, id, b, price, Flow::Leisure);
                if paid <= 0 {
                    continue;
                }
                gross += paid;
                world.take_stock(b, crate::components::Good::Food, 1);
                let cfg = world.config.needs.clone();
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    crate::needs::eat(n, &cfg);
                }
                add_fun(world, id, gain_at(world, b));
                crate::systems::jobs::note_visit(world, b);
            }
            _ => {
                let price = price_of(world, b);
                gross += pay_venue(world, id, b, price, Flow::Leisure);
                add_fun(world, id, gain_at(world, b));
                add_belonging(world, id, belonging_at(world, b));
                // A FightPit evening off screen bets as a table (the
                // Leisure stream), when brave enough and in funds.
                if kind_of(world, b) == Some(BuildingKind::FightPit) && !world.levers.ban_gambling {
                    let brave = world
                        .comp::<Personality>(id)
                        .is_some_and(|p| p.courage >= world.config.leisure.gamble_rung_courage);
                    let left = spendable(world, id);
                    if brave && left >= 1 {
                        let stake = stake_of(world, left);
                        let u: f32 = rng.random();
                        play_table(world, id, b, stake, u);
                        gross += stake;
                    }
                }
                world.remember(id, MemoryKind::Enjoyed, None, 0.2, 0.3, false);
                crate::systems::jobs::note_visit(world, b);
            }
        }
        world.stats.current.living.stat_spend += gross;
    }
}

// ---------------------------------------------------------------------------
// Fronts, the leader and the creed (plan L20)
// ---------------------------------------------------------------------------

/// A gang's standing fronts (owned by it, `front_of` it), ascending.
pub fn fronts_of(world: &World, gang: EntityId) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = [BuildingKind::FightPit, BuildingKind::Den, BuildingKind::Club]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&b| {
            standing(world, b)
                && world.comp::<Building>(b).is_some_and(|bd| {
                    bd.owner == Some(gang) && bd.venue.as_ref().is_some_and(|v| v.front_of == Some(gang))
                })
        })
        .collect();
    out.sort_unstable();
    out
}

/// Plan L20: the coins a gang keeps back from the daily stipends for the
/// leader's Collect (`tribute_share` of the week's tribute and take).
pub fn set_aside(world: &World, gang: EntityId) -> i64 {
    if !on(world) {
        return 0;
    }
    let take: i64 = fronts_of(world, gang)
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()))
        .map(|v| v.take_week.max(0))
        .sum();
    let week = world.comp::<Gang>(gang).map_or(0, |g| g.tribute_week.max(0)) + take;
    // Measured deviation: two days of its fronts' wages too (the stipends
    // left a front's Fighters and Croupiers unpaid; 18 unpaid quits).
    (world.config.leisure.tribute_share * week as f32).floor() as i64 + 2 * owner_payroll(world, Some(gang))
}

/// Plan L20: territory tribute credited today (`gang::daily_economy`).
pub fn note_tribute(world: &mut World, gang: EntityId, coins: i64) {
    if !on(world) || coins <= 0 {
        return;
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.tribute_week += coins;
    }
}

/// Plan L20, daily after the stipends: a Crackdown closes the gang's fronts
/// (in the districts it targets) for `crackdown_close_days`; a gang under
/// Expand with the treasury for a FightPit (a Sump district) or a Den
/// builds one, ≤ `fronts_max`, on a vacant Lot in a held district, else
/// refits a derelict there.
pub fn fronts_daily(world: &mut World, gang: EntityId) {
    if !on(world) || !crate::systems::jobs::on(world) {
        return;
    }
    // Fronts on the city's deed that still name this gang close too.
    let mut named: Vec<EntityId> = fronts_of(world, gang);
    for k in [BuildingKind::FightPit, BuildingKind::Den] {
        for &b in world.buildings_of_kind(k) {
            if standing(world, b)
                && !named.contains(&b)
                && world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).is_some_and(|v| v.front_of == Some(gang))
            {
                named.push(b);
            }
        }
    }
    let named_count = named.len() as u32;
    if crate::systems::law::cracking_down_on(world, gang) {
        use crate::components::{Posture, Stance};
        let city = world.law().is_some_and(|l| l.posture == Posture::Crackdown && l.target == Some(gang));
        let until = world.tick + world.config.leisure.crackdown_close_days * TICKS_PER_DAY;
        let now = world.tick;
        for b in named {
            let d = world.district_of_building(b);
            let hit = city || world.districts.get(d.index()).is_some_and(|x| x.stance == Stance::Crackdown(gang));
            if hit && !world.is_closed(b) {
                if let Some(bd) = world.comp_mut::<Building>(b) {
                    bd.closed_until = Some(until.max(now + 1));
                }
            }
        }
    }
    let cfg = world.config.leisure.clone();
    let Some(g) = world.comp::<Gang>(gang) else { return };
    // Review fix: the cap counts every front naming the gang, the seeded
    // ones still on the city's deed too.
    if g.order != crate::components::Order::Expand || named_count >= cfg.fronts_max || g.members.is_empty() {
        return;
    }
    let treasury = g.treasury;
    let held: Vec<DistrictId> = crate::systems::gang::held_districts(world, gang).into_iter().map(|(d, _)| d).collect();
    if held.is_empty() {
        return;
    }
    for kind in [BuildingKind::FightPit, BuildingKind::Den] {
        let Some(cost) = crate::systems::founding::found_cost(world, kind) else { continue };
        // Measured deviation: and a week of the front's full staff's wages
        // on top (a front built on the found cost alone left its Fighters
        // unpaid: 14 unpaid quits on seed 43).
        let payroll = world.config.buildings.for_kind(kind).staff as i64
            * ownership::role_for(kind).map_or(0, |r| world.config.economy.wage(r));
        if treasury < cost + 7 * payroll {
            continue;
        }
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter_map(|l| {
                let bd = world.comp::<Building>(l)?;
                let tier = crate::systems::founding::door_tier(world, bd.door);
                let d = world.district_of_building(l);
                (crate::systems::founding::tier_ok(kind, tier) && held.contains(&d)).then_some(l)
            })
            .min();
        let built = match lot {
            Some(l) => match crate::systems::founding::build_on_lot(world, l, kind, Some(gang)) {
                Ok(b) => {
                    ownership::pay(world, Some(gang), None, cost, Flow::Found);
                    Some(b)
                }
                Err(_) => None,
            },
            None => {
                let derelict = crate::systems::street::derelicts(world).into_iter().find(|&x| {
                    world.comp::<Building>(x).is_some_and(|bd| {
                        bd.kind == BuildingKind::Home
                            && crate::systems::founding::tier_ok(kind, bd.tier)
                            && held.contains(&world.district_of_building(x))
                    })
                });
                derelict.and_then(|x| crate::systems::founding::refit(world, x, kind, Some(gang)).ok())
            }
        };
        let Some(b) = built else { continue };
        let price = {
            let (tier, owner) = world.comp::<Building>(b).map_or((0, None), |bd| (bd.tier, bd.owner));
            crate::systems::jobs::venue_price(world, b, kind, tier, owner)
        };
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.front_of = Some(gang);
            v.price = price;
        }
        let at =
            world.districts.get(world.district_of_building(b).index()).map_or("?", |x| x.name.as_str()).to_string();
        let text = format!("{} opened a {} front in {at}", world.owner_label(Some(gang)), kind.label());
        world.push_event(EventKind::Founded, &[gang, b], text);
        crate::systems::virt::relink(world);
        return;
    }
}

/// `stims::deal_bar` with leisure on: the gang's open front (or a Club in
/// its most-held district) nearest that district's centroid.
pub fn deal_venue(world: &World, gang: EntityId, d: DistrictId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let centroid = world.district(d).centroid;
    let front = fronts_of(world, gang)
        .into_iter()
        .filter(|&b| open(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(centroid), b)))
        .min()
        .map(|(_, b)| b);
    front.or_else(|| {
        world
            .buildings_of_kind(BuildingKind::Club)
            .iter()
            .copied()
            .filter(|&b| standing(world, b) && open(world, b) && world.district_of_building(b) == d)
            .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(centroid), b)))
            .min()
            .map(|(_, b)| b)
    })
}

fn is_leader(world: &World, id: EntityId) -> Option<EntityId> {
    let gang = world.gang_of(id)?;
    world.comp::<Gang>(gang).filter(|g| g.leader == Some(id)).map(|_| gang)
}

/// Plan L20: the leader's Collect is due (the weekday, not done today).
fn collect_due(world: &World, gang: EntityId) -> bool {
    let today = world.day();
    today % 7 == world.config.leisure.collect_weekday % 7
        && world.comp::<Gang>(gang).is_some_and(|g| g.collected_day != Some(today))
}

/// Plan L20: a Call is due the evening after an order change.
fn call_due(world: &World, gang: EntityId) -> bool {
    let Some(g) = world.comp::<Gang>(gang) else { return false };
    g.order_since > 0
        && g.called_for != Some(g.order_since)
        && world.tick.saturating_sub(g.order_since) < TICKS_PER_DAY
        && world.phase() == DayPhase::Evening
}

/// Plan L20: the leader's `Lead` gate (`None`: not the leader, or nothing due).
pub fn lead_considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || world.has::<Sentence>(id) {
        return None;
    }
    let gang = is_leader(world, id)?;
    let due = collect_due(world, gang) || call_due(world, gang);
    if !due {
        return None;
    }
    Some((vec![Consideration::raw("collect or call", 1.0, 0.6)], 0.0))
}

/// `already_satisfied(Lead)`.
pub fn lead_satisfied(world: &World, id: EntityId) -> bool {
    !on(world) || is_leader(world, id).is_none_or(|g| !collect_due(world, g) && !call_due(world, g))
}

/// Plan L20: the round: `GoTo(front) → Collect` per front on a collect day,
/// then `GoTo(Hideout) → Collect` (the pay-out and the Call).
pub fn lead_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !on(world) {
        return None;
    }
    let gang = is_leader(world, id)?;
    let hideout = world.hideout_of(gang)?;
    let mut steps = Vec::new();
    if collect_due(world, gang) {
        for f in fronts_of(world, gang) {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(f)));
            steps.push(step(ActionKind::Collect, Some(f)));
        }
    } else if !call_due(world, gang) {
        return None;
    }
    steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(hideout)));
    steps.push(step(ActionKind::Collect, Some(hideout)));
    Some(Plan { goal: GoalKind::Lead, target: Some(hideout), steps, started_tick: world.tick })
}

/// A `Collect` step: at a front the week is booked (the coins reached the
/// treasury at each payment); at the Hideout the Call and the pay-out.
fn collect_step(world: &mut World, id: EntityId, target: Option<EntityId>) {
    let Some(gang) = is_leader(world, id) else { return };
    if target.is_none() || target != world.hideout_of(gang) {
        return;
    }
    if call_due(world, gang) {
        call(world, gang, id);
    }
    if collect_due(world, gang) {
        payout(world, gang, Some(id));
    }
}

fn call(world: &mut World, gang: EntityId, leader: EntityId) {
    let until = world.tick + world.config.leisure.call_hours * TICKS_PER_HOUR;
    let Some(g) = world.comp_mut::<Gang>(gang) else { return };
    g.call_until = Some(until);
    g.called_for = Some(g.order_since);
    let text = format!("{} called {} to the Hideout", world.name_of(leader), world.owner_label(Some(gang)));
    world.push_event(EventKind::Called, &[leader, gang], text);
}

/// Plan L20: `tribute_share × (the week's territory tribute + the fronts'
/// take)` from the treasury to the members, equal shares, highest rank
/// first (the leader's ranking, then the rest by id); `Flow::Tribute`.
/// Resets the week. Returns the members paid.
pub fn payout(world: &mut World, gang: EntityId, leader: Option<EntityId>) -> u32 {
    let today = world.day();
    let fronts = fronts_of(world, gang);
    let take: i64 = fronts
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()))
        .map(|v| v.take_week.max(0))
        .sum();
    let Some(g) = world.comp::<Gang>(gang) else { return 0 };
    let week = g.tribute_week.max(0) + take;
    let pool = (world.config.leisure.tribute_share * week as f32).floor() as i64;
    // Review fix: net of the fronts' payroll the stipends keep back.
    let keep = 2 * owner_payroll(world, Some(gang));
    let pool = pool.min((g.treasury - keep).max(0));
    let mut members: Vec<EntityId> = crate::systems::gang::leader_ranking(world, gang);
    let mut rest: Vec<EntityId> = g.members.iter().copied().filter(|m| !members.contains(m)).collect();
    rest.sort_unstable();
    members.extend(rest);
    let mut paid_members = 0u32;
    let mut paid_total = 0i64;
    if pool > 0 && !members.is_empty() {
        let n = members.len() as i64;
        let share = pool / n;
        let extra = pool % n;
        for (i, m) in members.iter().enumerate() {
            let amount = share + i64::from((i as i64) < extra);
            if amount <= 0 {
                continue;
            }
            let p = ownership::pay(world, Some(gang), Some(*m), amount, Flow::Tribute);
            if p > 0 {
                crate::systems::gang::note_paid(world, *m);
                paid_members += 1;
                paid_total += p;
            }
        }
    }
    for b in fronts {
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.take_week = 0;
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.tribute_week = 0;
        g.collected_day = Some(today);
    }
    world.stats.current.living.collected += paid_members;
    let who = leader.map_or_else(|| "the leader".to_string(), |l| world.name_of(l));
    let text = format!(
        "{who} collected for {}: {paid_total} to {paid_members} members (the week's tribute and take {week})",
        world.owner_label(Some(gang))
    );
    let mut actors: Vec<EntityId> = leader.into_iter().collect();
    actors.push(gang);
    world.push_event(EventKind::Collected, &actors, text);
    paid_members
}

/// Plan L20: a leader who is not a body collects (and calls) without the
/// walk, from `living::run`'s 18:00 hour.
pub fn stat_leaders(world: &mut World) {
    for gang in world.gangs() {
        let Some(leader) = world.comp::<Gang>(gang).and_then(|g| g.leader) else { continue };
        let body = world.comp::<Brain>(leader).is_some_and(|b| b.lod != Lod::Statistical);
        if body && !world.has::<Sentence>(leader) {
            continue;
        }
        if call_due(world, gang) {
            call(world, gang, leader);
        }
        if collect_due(world, gang) {
            payout(world, gang, Some(leader));
        }
    }
}

/// Plan L20: a Purist gang's preacher today: under Expand, `preach_share`
/// of the members by `id.index % 100`, a busiest spot in a held district.
pub fn preacher(world: &World, id: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(gang) = world.gang_of(id) else { return false };
    crate::systems::creeds::is_purist(world, gang)
        && world.comp::<Gang>(gang).is_some_and(|g| g.order == crate::components::Order::Expand)
        && u64::from(id.index % 100) < (world.config.leisure.preach_share * 100.0) as u64
        && world.comp::<Brain>(id).is_none_or(|b| b.gang_task_day != Some(world.day()))
}

/// The busiest spot (most hanging out now, then the lower tile) of the
/// gang's held districts, else of its Hideout's district.
fn busiest_spot(world: &World, gang: EntityId) -> Option<TilePos> {
    let mut ds: Vec<DistrictId> =
        crate::systems::gang::held_districts(world, gang).into_iter().map(|(d, _)| d).collect();
    if ds.is_empty() {
        ds.push(world.district_of_building(world.hideout_of(gang)?));
    }
    ds.iter()
        .filter_map(|d| world.spots.get(d.index()))
        .flat_map(|v| v.iter())
        .filter(|s| !matches!(s.kind, SpotKind::Hideout(g) if g != gang))
        .map(|s| (world.hangouts.get(&s.tile).map_or(0, |v| v.len()), s.tile))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, t)| t)
}

/// Plan L20: Preach's GangWork: `GoTo(Spot) → Preach` at the busiest spot.
pub fn preach_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !preacher(world, id) {
        return None;
    }
    let gang = world.gang_of(id)?;
    let tile = busiest_spot(world, gang)?;
    world.unwind.insert(
        id,
        UnwindPick { rung: Rung::Free, act: ActionKind::Preach, venue: None, spot: Some(tile), score: 0.0 },
    );
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::Preach, None)];
    Some(Plan { goal: GoalKind::GangWork, target: None, steps, started_tick: world.tick })
}

/// Plan L20: a Preach ends: a Persuade (M15 `moves::resolve`, stake the
/// gang) on the co-hanger of highest affinity (else the first present);
/// success leaves the heard `Persuaded` entry and `+preach_opinion` on the
/// listener's edge to the gang's leader, `Preached`. The day's GangWork is
/// done either way.
fn preach_done(world: &mut World, id: EntityId) {
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.gang_task_day = Some(today);
    }
    let Some(gang) = world.gang_of(id) else { return };
    let tile = world.comp::<Position>(id).map(|p| p.tile);
    let listener = tile
        .and_then(|t| world.hangouts.get(&t))
        .map(|v| {
            v.iter()
                .copied()
                .filter(|&o| o != id && world.has::<Brain>(o) && world.gang_of(o) != Some(gang))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|o| (o, world.edge(id, o).map_or(f32::NEG_INFINITY, |e| e.affinity)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(o, _)| o);
    let Some(t) = listener else { return };
    if !crate::systems::moves::on(world) {
        return;
    }
    let m = crate::word::SocialMove {
        actor: id,
        target: t,
        kind: crate::word::MoveKind::Persuade,
        stake: crate::word::Stake::Join(gang),
    };
    let out = crate::systems::moves::resolve(world, &m);
    if !out.success {
        return;
    }
    if let Some(l) = world.comp::<Gang>(gang).and_then(|g| g.leader).filter(|&l| l != t) {
        let tick = world.tick;
        let gain = world.config.leisure.preach_opinion;
        let e = world.edge_entry(t, l);
        e.affinity = (e.affinity + gain).min(1.0);
        e.last_interaction = tick;
    }
    world.stats.current.living.preached += 1;
    let text = format!("{} preached to {}", world.name_of(id), world.name_of(t));
    world.push_event(EventKind::Preached, &[id, t], text);
}

// ---------------------------------------------------------------------------
// The hand-over of the seeded venues (phase 1 deviation)
// ---------------------------------------------------------------------------

/// The seeded venues on the city's deed pass to their heir (a front's gang,
/// a Club's or the Lounge's megacorp) once they earn: from day 14, each of
/// the last 7 days' revenue covered a day of the staff's wages and the
/// kind's upkeep, and the heir's purse holds a week of both.
pub fn hand_over(world: &mut World) {
    let mut moves: Vec<(EntityId, EntityId)> = Vec::new();
    for kind in BuildingKind::LEISURE {
        for &b in world.buildings_of_kind(kind) {
            let Some(bd) = world.comp::<Building>(b).filter(|bd| bd.owner.is_none() && !bd.demolished && !bd.derelict)
            else {
                continue;
            };
            let Some(v) = bd.venue.as_ref() else { continue };
            let Some(heir) = v.heir.or(v.front_of) else { continue };
            if !(world.has::<Gang>(heir) || world.has::<Corp>(heir)) || bd.revenue.len() < 7 {
                continue;
            }
            // Every one of the last 7 days covered the day's wages and
            // upkeep, past the opening fortnight (measured: the day-1 burst
            // of full purses passed a 7-day sum, then the venues' takings
            // fell and the heirs' Cooks, Bartenders and Fighters went unpaid).
            let worst: i64 = bd.revenue.iter().rev().take(7).copied().min().unwrap_or(0);
            let wages: i64 = ownership::staff_at(world, b)
                .into_iter()
                .filter_map(|s| world.comp::<Job>(s).map(|j| j.wage_per_day))
                .sum();
            let upkeep = {
                let t = world.comp::<Building>(b).map_or(0, |x| x.tier);
                world.config.corps.upkeep.for_building(kind, t)
            };
            // ... and the heir holds a week of them (a gang at its stipends' floor
            // took a pit whose Fighters then went unpaid).
            if world.day() >= 14 && worst >= wages + upkeep && world.purse(Some(heir)) >= 7 * (wages + upkeep) {
                moves.push((b, heir));
            }
        }
    }
    for (b, heir) in moves {
        crate::systems::corps::move_building(world, b, Some(heir));
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.heir = None;
            // Review fix: the city-era take is the Treasury's, not the week's.
            v.take_week = 0;
        }
        let text = format!("{} took over {} (it earns)", world.owner_label(Some(heir)), world.name_of(b));
        world.push_event(EventKind::Founded, &[heir, b], text);
    }
}

// ---------------------------------------------------------------------------
// The daily and hourly passes (plan L36)
// ---------------------------------------------------------------------------

/// Midnight: the spots, the wealth decile, stale picks pruned, the hand-over,
/// the fun columns of the closing day.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    spots_daily(world);
    let mut purses: Vec<i64> =
        world.with::<Wallet>().iter().filter_map(|&a| world.comp::<Wallet>(a)).map(|w| w.coins).collect();
    purses.sort_unstable();
    world.wealth_p90 = purses.get(purses.len() * 9 / 10).copied().unwrap_or(0).max(1);
    let keep: Vec<EntityId> = world
        .unwind
        .keys()
        .copied()
        .filter(|&id| {
            world
                .comp::<Brain>(id)
                .and_then(|b| b.plan_goal())
                .is_some_and(|g| matches!(g, GoalKind::Unwind | GoalKind::Socialise | GoalKind::GangWork))
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
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if world.spots.is_empty() {
        spots_daily(world);
    }
    sample_density(world);
    let hour = world.tick_of_day() / TICKS_PER_HOUR as u16;
    if hour == 18 {
        stat_leaders(world);
    }
    if hour == world.config.leisure.bout_hour {
        bouts(world);
    }
    if world.tick_of_day() == 1260 {
        stat_daily(world);
    }
}

/// `d{i}_street_density`: HangOut and venue occupants per district, the
/// day's hourly mean.
fn sample_density(world: &mut World) {
    let n = crate::stats::DISTRICT_SLOTS;
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
    if !on(world) {
        return (0.0, 0.0);
    }
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
