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
fn has_room(world: &World, id: EntityId, b: EntityId) -> bool {
    // Plan L19, L30: the live Seat (and Bed) reservations of others count
    // (`World::reserved_at`: Scale S4 perf, the per-building index; it was
    // one pass over every reservation per ladder walk).
    world.comp::<Building>(b).is_some_and(|bd| {
        bd.occupants.contains(&id) || bd.occupants.len() + world.reserved_at(b, id) < usize::from(bd.capacity)
    })
}

/// Plan L19: the door policy. A Lounge refuses a Dreg or a dress below 2; a
/// Club refuses `rep.heat ≥ door_heat` when the strongest Host on shift has
/// courage above that heat (no draw).
pub fn door_ok(world: &World, id: EntityId, b: EntityId) -> bool {
    // Jobs & lives phase 1 (§ 2): a site still being built is shut, and a
    // dark district's night venues (all but the noodle stalls) are closed.
    if crate::systems::industry::closed_site(world, b) {
        return false;
    }
    if crate::systems::industry::dark_at(world, b)
        && kind_of(world, b).is_some_and(|k| k.is_leisure() && k != BuildingKind::NoodleBar)
    {
        return false;
    }
    // The pressure pass phase 2 (OPEN-176): a bouncer on duty turns a
    // visitor with a cause away (drunk, a brawler, a rival's colours, rags).
    if crate::systems::customers::door_cause(world, id, b).is_some() {
        return false;
    }
    // The pressure pass R6a (OPEN-217): a rough client barred by the madam.
    if crate::systems::vice::barred(world, id, b) {
        return false;
    }
    // Round 4 T4 (the under-18 rule): no vice house takes a client under 18,
    // a bouncer on the door or not (`ages::is_protected`).
    if kind_of(world, b).is_some_and(|k| crate::systems::vice::is_vice_kind(k) || k == BuildingKind::Dollhouse)
        && crate::systems::ages::is_protected(world, id)
    {
        return false;
    }
    match kind_of(world, b) {
        Some(BuildingKind::Lounge) => {
            crate::systems::classes::class_of(world, id) != Class::Dreg
                && crate::systems::reputation::appearance_of(world, id).dress >= 2
        }
        // Jobs & lives phase 5: open while a contracted worker is on shift.
        Some(BuildingKind::Dollhouse) => crate::systems::dollhouse::staffed_now(world, b),
        Some(BuildingKind::Club) => {
            let heat = crate::systems::reputation::rep(world, id).heat;
            if heat < world.config.leisure.door_heat {
                return true;
            }
            let tod = world.tick_of_day();
            // Jobs and room J13 (`service`): the trade staff on duty at the
            // door (a fighting trade: the Bouncers) stand with the Hosts.
            let bouncer = ownership::staff_at(world, b)
                .into_iter()
                .filter(|&s| world.comp::<Job>(s).is_some_and(|j| j.role == Role::Host && j.on_shift(tod)))
                .chain(crate::systems::trades::door_staff(world, b))
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

/// The pre-view pass phase 3 (OPEN-121): the door as a patron choosing a
/// venue knows it. With `[shift.counter]` on, a Club's door is the
/// bouncer's call made at the door (`door_ok` there, `service::refused`),
/// not one the patron reads from across the city: the pick passes the
/// Club's heat rule and only passes over a venue that turned `id` away
/// within `[exec] refused_ticks`. Off: `door_ok` (the bouncer's verdict
/// read at the pick, so the door never refused anyone).
pub fn door_seen(world: &World, id: EntityId, b: EntityId) -> bool {
    // The pressure pass phase 2: every door a bouncer keeps is his call at
    // the door (`[customers]`), not the Club's alone.
    let kept = kind_of(world, b).is_some_and(|k| {
        k == BuildingKind::Club
            || (crate::systems::customers::cfg(world).is_some() && crate::systems::customers::kept_kind(world, k))
    });
    if !crate::systems::shift::counter_on(world) || !kept {
        return door_ok(world, id, b);
    }
    let turned = world
        .comp::<Brain>(id)
        .and_then(|br| br.refused)
        .is_some_and(|(r, t)| r == b && world.tick.saturating_sub(t) < world.config.exec.refused_ticks.max(1));
    !turned && !crate::systems::industry::closed_site(world, b) && !crate::systems::industry::dark_at(world, b)
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
    here(world, id).and_then(|b| kind_of(world, b)).is_some_and(|k| matches!(k, BuildingKind::Club | BuildingKind::Den))
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
    if here(world, id).and_then(|b| kind_of(world, b)) == Some(BuildingKind::Bar) {
        add_fun(world, id, world.config.leisure.gain.drink);
    }
}

/// The fun an `Enjoy` (or a meal, a table) at `b` gives.
pub(crate) fn gain_at(world: &World, b: EntityId) -> f32 {
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
        // Jobs & lives phase 4: the Vice venues and the Boutique.
        Some((k @ (BuildingKind::StripClub | BuildingKind::Brothel), _)) => crate::systems::vice::gain(world, k),
        Some((BuildingKind::Boutique, _)) => world.config.fashion.gain_boutique,
        // Jobs & lives phase 5.
        Some((BuildingKind::Dollhouse, _)) => world.config.vice.gain_dollhouse,
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
pub(crate) fn pay_venue(world: &mut World, from: EntityId, b: EntityId, amount: i64, flow: Flow) -> i64 {
    let owner = world.owner_of(b);
    let paid = ownership::pay(world, Some(from), owner, amount, flow);
    if paid <= 0 {
        return 0;
    }
    ownership::credit(world, b, paid);
    note_paid(world, from, b, paid);
    let tax = world.levers.leisure_tax;
    if tax > 0.0 && owner.is_some() {
        let extra = (tax * paid as f32).round() as i64;
        ownership::pay(world, owner, None, extra, Flow::Tax);
    }
    book_take(world, b, paid, flow == Flow::Gamble);
    paid
}

/// A paid visit of `paid` coins by `payer` at `b`, by its kind
/// (`CoinProbe::venue_paid`; a body's in `venue_unmirrored` too).
pub(crate) fn note_paid(world: &mut World, payer: EntityId, b: EntityId, paid: i64) {
    if paid <= 0 {
        return;
    }
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let e = world.probe.venue_paid.entry(format!("{kind:?}")).or_default();
    e.0 += 1;
    e.1 += paid;
    note_unmirrored(world, payer, kind, 1);
}

/// OPEN-106: a body's visit (`lod::kept_body`) at a venue of `kind`, `+1`
/// or `-1` on a refund (`CoinProbe::venue_unmirrored`).
fn note_unmirrored(world: &mut World, payer: EntityId, kind: BuildingKind, delta: i64) {
    if crate::systems::lod::kept_body(world, payer) {
        let e = world.probe.venue_unmirrored.entry(format!("{kind:?}")).or_default();
        *e = e.saturating_add_signed(delta);
    }
}

/// OPEN-106: a world-side pass's paid visit by `payer` at `b`, both tiers
/// alike (`classpay::status_daily`): out of `calibrate`'s `p_venue` (a
/// body's is already counted out by `note_paid`).
pub(crate) fn note_world_side(world: &mut World, payer: EntityId, b: EntityId) {
    if crate::systems::lod::kept_body(world, payer) {
        return;
    }
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    *world.probe.venue_unmirrored.entry(format!("{kind:?}")).or_default() += 1;
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

/// The pressure pass R6a (OPEN-217): an off-screen visitor of the pit `b`
/// in the bet window with `gamble_rung_courage` and a coin stakes on
/// tonight's bout as a body's `Gamble` does (`Flow::Gamble` into the
/// house, the bet settled at `bout_hour`), at most `pit_bettors_max` a
/// pit. Returns the stake placed (0 none). Nothing with `[trade_verbs]` absent.
fn bout_bet(world: &mut World, id: EntityId, b: EntityId) -> i64 {
    let Some(max) = world.config.trade_verbs.as_ref().map(|c| c.pit_bettors_max) else { return 0 };
    let brave = world.comp::<Personality>(id).is_some_and(|p| p.courage >= world.config.leisure.gamble_rung_courage);
    if !bet_window(world) || !brave {
        return 0;
    }
    let placed = world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).map_or(usize::MAX, |v| v.bets.len());
    let left = spendable(world, id).min(coins(world, id));
    if placed >= max as usize || left < 1 {
        return 0;
    }
    let stake = stake_of(world, left);
    let paid = pay_venue(world, id, b, stake, Flow::Gamble);
    if paid <= 0 {
        return 0;
    }
    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
        v.bets.push((id, paid, false));
    }
    paid
}

/// R6a: is the bet window open at the time of day `tod` (a body's arrival)?
fn bet_window_at(world: &World, tod: u64) -> bool {
    let cfg = &world.config.leisure;
    let h = ((tod % TICKS_PER_DAY) / TICKS_PER_HOUR) as u16;
    h < cfg.bout_hour && cfg.bout_hour.saturating_sub(cfg.bet_window_hours) <= h && !world.levers.ban_gambling
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
        // Phase 7b's probe: a won round is a visit too (no coins in).
        if let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) {
            world.probe.venue_paid.entry(format!("{kind:?}")).or_default().0 += 1;
            note_unmirrored(world, id, kind, 1);
        }
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
    /// F3 (OPEN-190): the stay-in fallback's own-Home HangOut.
    stay_in: bool,
}

pub(crate) fn rung_of(world: &World, b: EntityId, price: i64) -> Rung {
    match kind_of(world, b) {
        Some(BuildingKind::Lounge) => Rung::High,
        // Jobs & lives phase 5: the Dollhouse on the high rung.
        Some(BuildingKind::Dollhouse) => Rung::High,
        Some(BuildingKind::Club) if price >= 12 => Rung::High,
        Some(BuildingKind::Club | BuildingKind::FightPit | BuildingKind::Den) => Rung::Mid,
        // Jobs & lives phase 4: the Vice venues on the mid rung; a Spire
        // Boutique on the high one.
        Some(BuildingKind::StripClub | BuildingKind::Brothel) => Rung::Mid,
        Some(BuildingKind::Boutique) => {
            if world.comp::<Building>(b).is_some_and(|bd| bd.tier >= 2) {
                Rung::High
            } else {
                Rung::Mid
            }
        }
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
/// The pressure pass R6a (OPEN-217): the lonely with `[customers]
/// client_coins` reach the Dollhouse below the high rung's purse (it was
/// the execs' and the top tenth's alone: 25 visits in 30 days at 20k, the
/// joytoys' shifts empty). Nothing with `[trade_verbs]` absent.
fn lonely_client(world: &World, id: EntityId, b: EntityId) -> bool {
    world.config.trade_verbs.is_some()
        && kind_of(world, b) == Some(BuildingKind::Dollhouse)
        && crate::systems::customers::lonely_wants(world, id)
}

fn high_ok(world: &World, id: EntityId) -> bool {
    crate::systems::life::exec_corp(world, id).is_some()
        || (world.wealth_p90 > 0 && coins(world, id) >= world.wealth_p90)
}

/// The walk's factor (`life::travel_factor`; 1 with the life pass off).
fn travel(world: &World, tiles: u32) -> f32 {
    crate::systems::life::travel_factor(world, crate::systems::life::walk_ticks(world, tiles))
}

/// The island pass phase 2 (`[distance.free]`): a spot's travel estimate
/// from `from` (`distance::travel`), `None` off.
fn spot_ticks(world: &World, from: TilePos, tile: TilePos) -> Option<Tick> {
    crate::systems::distance::free(world).map(|_| crate::systems::distance::travel(world, from, tile))
}

/// The walk's factor to a spot at `tile`: the travel estimate's with
/// `[distance.free]` on, else the Manhattan walk's (the base).
fn spot_travel(world: &World, from: TilePos, tile: TilePos) -> f32 {
    match spot_ticks(world, from, tile) {
        Some(t) => crate::systems::life::travel_factor(world, t),
        None => travel(world, tile.manhattan(from)),
    }
}

/// A spot's distance for the cap and the ties: the travel estimate on, the
/// Manhattan tiles off.
fn spot_dist(world: &World, from: TilePos, tile: TilePos) -> u64 {
    spot_ticks(world, from, tile).unwrap_or_else(|| u64::from(tile.manhattan(from)))
}

/// The spot cap in `spot_dist`'s unit: `spot_cap_ticks` on, else
/// `[life] spot_walk_cap_tiles`.
fn spot_cap(world: &World) -> u64 {
    crate::systems::distance::free(world).map_or(u64::from(world.config.life.spot_walk_cap_tiles), |f| f.spot_cap_ticks)
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
    // Jobs & lives phase 4: a Brothel weighed by the visitor's intimacy, a
    // Boutique by its status (a branch: the older kinds score as before).
    if let Some(k) = kind_of(world, b).filter(|&k| matches!(k, BuildingKind::Brothel | BuildingKind::Boutique)) {
        s *= crate::systems::vice::score_mult(world, id, k) * crate::systems::fashion::score_mult(world, id, k);
    }
    // The pressure pass phase 2 (OPEN-176): the lonely and the miserable
    // are drawn to a venue whose companion, host or dancer is on duty.
    s *= crate::systems::customers::client_mult(world, id, b);
    let share = price.max(1) as f32 / purse.max(1) as f32;
    s * (1.0 - greed * share).max(0.05)
}

/// The kinds the ladder offers, cheap to dear (Jobs & lives phase 4's
/// three last: none stands with their switches off).
const LADDER: [BuildingKind; 11] = [
    BuildingKind::Bar,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
    BuildingKind::Club,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Lounge,
    BuildingKind::StripClub,
    BuildingKind::Brothel,
    BuildingKind::Boutique,
    BuildingKind::Dollhouse,
];

/// Where a paid visit starts from: a Full agent where it stands (`At`), or
/// a Statistical worker's day (`Day`): the Manhattan rectangle its Home's
/// door and its workplace's door span, every shortest walk between the two
/// inside it (Jobs & lives phase 7b: the Full tier's paid visits start at
/// home, at work or on the street between, 73 % of a worker's within the
/// walk cap of that rectangle on the calibration city).
#[derive(Clone, Copy, Debug)]
enum Origin {
    At(TilePos),
    Day(TilePos, TilePos),
}

impl Origin {
    /// The origin's tile nearest `t` (the rectangle's clamp).
    fn near(self, t: TilePos) -> TilePos {
        match self {
            Origin::At(p) => p,
            Origin::Day(a, b) => {
                TilePos { x: t.x.clamp(a.x.min(b.x), a.x.max(b.x)), y: t.y.clamp(a.y.min(b.y), a.y.max(b.y)) }
            }
        }
    }

    /// Manhattan tiles from the origin to `t`.
    fn dist(self, t: TilePos) -> u32 {
        self.near(t).manhattan(t)
    }
}

impl Origin {
    /// The origin's box, `(lo, hi)` corners (a point for `At`).
    fn bounds(self) -> (TilePos, TilePos) {
        match self {
            Origin::At(p) => (p, p),
            Origin::Day(a, b) => {
                (TilePos { x: a.x.min(b.x), y: a.y.min(b.y) }, TilePos { x: a.x.max(b.x), y: a.y.max(b.y) })
            }
        }
    }
}

/// The perf pass phase 2 (the venue index): the venues of `kind` (those of
/// `buildings_of_kind` with a Building) handed to `test` nearest first by
/// `(door distance from `from`, id)`, up to the first that passes and no
/// further than `max_d` tiles: the sequence the old collect-and-sort of
/// every venue of the kind handed to its `find`, read off the kind's grid
/// (`World::kind_grid`) ring by ring.
fn venue_by_distance(
    world: &World,
    kind: BuildingKind,
    from: Origin,
    max_d: u32,
    test: impl FnMut(u32, EntityId) -> bool,
) -> Option<(u32, EntityId)> {
    let (lo, hi) = from.bounds();
    world.kind_grid(kind).first_by_distance(lo, hi, max_d, |door| from.dist(door), test)
}

/// The most tiles from the origin a venue's door can be and still be within
/// `reach` ticks' walk for `id` (`exec::walk_reach_tiles`; `u32::MAX`
/// unbounded).
fn reach_tiles(world: &World, id: EntityId, reach: Option<u64>) -> u32 {
    let limit = (world.map.w() + world.map.h()) as u32;
    reach.map_or(u32::MAX, |r| crate::exec::walk_reach_tiles(world, id, r, limit))
}

/// The nearest open venue of `kind` `id` may enter from `from` (door
/// Manhattan, ties the lower id), restricted to `district` when given, and
/// `None` past `max_d` tiles (the perf pass phase 2: the callers drop a pick
/// past the walk cap, and no door further than `max_d` is within it).
fn nearest_venue(
    world: &World,
    id: EntityId,
    kind: BuildingKind,
    from: Origin,
    district: Option<DistrictId>,
    max_d: u32,
) -> Option<(u32, EntityId)> {
    // Scale S4 perf: the distances first, then the tests nearest first
    // until one passes (the door policy reads the staff, the room the
    // reservations), not the tests over every venue of the kind: the same
    // pick. The perf pass phase 2: the nearest first off the kind's grid.
    // Shadow V4 bug 18: not the agent's own workplace (staff spent their
    // evening at their own Brothel or pit, then waited up to 10 h there for
    // the shift).
    let own = world.comp::<crate::components::Job>(id).and_then(|j| j.employer);
    venue_by_distance(world, kind, from, max_d, |_, b| {
        Some(b) != own
            && standing(world, b)
            && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
            && district.is_none_or(|d| world.district_of_building(b) == d)
            && open(world, b)
            && door_seen(world, id, b)
            && has_room(world, id, b)
    })
}

/// The paid rungs open to `id` from `from` with `purse` to spend. Day gaps
/// 2 (2026-10-11): with `reach`, a venue whose walk (`exec::timed_walk`,
/// from the origin's nearest tile) is longer is off the ladder (the Full
/// agent's pick; since Jobs & lives phase 7b the Statistical evening's
/// too, from its day). With `district`, its venue of a kind in reach first,
/// else the nearest of the kind anywhere.
fn paid_candidates(
    world: &World,
    id: EntityId,
    from: Origin,
    purse: i64,
    district: Option<DistrictId>,
    reach: Option<u64>,
) -> Vec<Cand> {
    let mut out = Vec::new();
    if purse < 1 {
        return out;
    }
    let drank = crate::utility::goals::drank_today(world, id);
    let hungry = world.comp::<Needs>(id).is_some_and(|n| n.hunger < 0.5);
    let high = high_ok(world, id);
    let max_d = reach_tiles(world, id, reach);
    for kind in LADDER {
        if kind == BuildingKind::Bar && drank {
            continue;
        }
        if kind == BuildingKind::NoodleBar && !hungry {
            continue;
        }
        let in_reach = |&(_, b): &(u32, EntityId)| {
            reach.is_none_or(|r| {
                world
                    .comp::<Building>(b)
                    .is_some_and(|bd| crate::exec::timed_walk(world, id, from.near(bd.door), bd.door) <= r)
            })
        };
        let pick = nearest_venue(world, id, kind, from, district, max_d)
            .filter(in_reach)
            .or_else(|| district.and_then(|_| nearest_venue(world, id, kind, from, None, max_d)));
        let Some((tiles, b)) = pick.filter(in_reach) else { continue };
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
        if rung == Rung::High && !high && !lonely_client(world, id, b) {
            continue;
        }
        let score = paid_score(world, id, b, rung, price, purse, tiles);
        out.push(Cand { rung, act: act_of(kind), venue: Some(b), spot: None, score, stay_in: false });
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
    let reach = world.config.leisure.venue_walk_cap_ticks;
    let mut cands = paid_candidates(world, id, Origin::At(from), purse, None, Some(reach));
    // Spec § 2 (stat_daily's rule, kept on screen for parity): a hungry
    // agent's evening is the NoodleBar's meal when one is in reach.
    if let Some(c) = cands.iter().find(|c| c.act == ActionKind::EatOut) {
        return Some(pick_of(c));
    }
    let g = &world.config.leisure.gain;
    if let Some(t) = watch_tile(world, from) {
        let score = g.watch * travel(world, t.manhattan(from));
        cands.push(Cand {
            rung: Rung::Free,
            act: ActionKind::HangOut,
            venue: None,
            spot: Some(t),
            score,
            stay_in: false,
        });
    }
    // Lives at work phase 2 (§ 3.1): a Home spot carries its Home as the
    // pick's venue (the HangOut runs inside it).
    let (spot, past_cap) = if scored {
        let (best, past) = spot_pick(world, id, from);
        let best = best.map(|(s, social)| {
            let home = match s.kind {
                SpotKind::Home(h) => Some(h),
                _ => None,
            };
            (s.tile, social, home, false)
        });
        (best, past)
    } else {
        // The island pass phase 2: on, the nearest is `None` only with no
        // spot within the cap.
        let near = nearest_spot(world, id, from).map(|(t, social)| (t, social, None, false));
        let past = near.is_none();
        (near, past)
    };
    // Island fix round F3 (OPEN-190): with no spot within the cap, the
    // stay-in: the own Home's HangOut, scored at `stay_in.fun_hour`.
    let spot = spot.or_else(|| {
        if !past_cap {
            return None;
        }
        stay_in_home(world, id).map(|(h, t)| (t, 0.0, Some(h), true))
    });
    if let Some((t, social, home, stay_in)) = spot {
        let hours = world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        // L2 shadow fixes item 5: a lone hour is not a full hour's fun (11
        // of 15 HangOuts were "nobody there"); company scales it up.
        let lone = world.config.life.spot_lone_fun;
        let company = lone + (1.0 - lone) * social.clamp(0.0, 1.0);
        let fun = match crate::systems::distance::stay_in(world).filter(|_| stay_in) {
            Some(c) => c.fun_hour * hours,
            None => g.hangout_hour * hours * company,
        };
        let score = (fun + 0.1 * social.min(1.0)) * spot_travel(world, from, t);
        cands.push(Cand { rung: Rung::Free, act: ActionKind::HangOut, venue: home, spot: Some(t), score, stay_in });
    }
    let best = cands.iter().max_by(|a, b| {
        a.score.total_cmp(&b.score).then_with(|| b.venue.cmp(&a.venue)).then_with(|| b.spot.cmp(&a.spot))
    })?;
    Some(pick_of(best))
}

fn pick_of(c: &Cand) -> UnwindPick {
    UnwindPick { rung: c.rung, act: c.act, venue: c.venue, spot: c.spot, score: c.score, stay_in: c.stay_in }
}

// ---------------------------------------------------------------------------
// Spots (plan L15, L16)
// ---------------------------------------------------------------------------

/// Plan L15, midnight (and once lazily): each district's spots, ≤ 24: the
/// street outside the doors of its Bars, Markets, Clubs and NoodleBars, each
/// gang's Hideout door, and in a Sump district `barrels_per_sump` fire
/// barrels on street tiles within 6 of a Home door (`splitmix64`).
pub fn spots_daily(world: &mut World) {
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
            .map(|&ix| TilePos { x: (ix as usize % w) as u16, y: (ix as usize / w) as u16 })
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
    // L2 shadow fixes item 6: not the street just fled.
    if crate::systems::fixes::avoided(world, id, s.tile) {
        return false;
    }
    match s.kind {
        SpotKind::Hideout(g) => world.gang_of(id) == Some(g),
        _ => true,
    }
}

/// The spots of `id`'s district (its tile's), and of its Home's; its own
/// Home (Lives at work phase 2) and, for the scored pick, its friends'.
fn spots_near(world: &World, id: EntityId, from: TilePos) -> Vec<Spot> {
    spots_near_with(world, id, from, false)
}

fn spots_near_with(world: &World, id: EntityId, from: TilePos, with_friends: bool) -> Vec<Spot> {
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
    // Lives at work phase 2 (§ 3.1, § 3.2): the agent's own Home and the
    // Homes of friends due home, within the walk cap (nothing off).
    let cap = world.config.life.spot_walk_cap_tiles;
    for (h, _) in crate::systems::household::home_spots(world, id, from, cap, with_friends) {
        if let Some(bd) = world.comp::<Building>(h) {
            let tile = world.outside_door(bd);
            if !out.iter().any(|s| s.tile == tile) {
                out.push(Spot { tile, kind: SpotKind::Home(h) });
            }
        }
    }
    out
}

/// The nearest spot `id` may use (think time), with a 0 social score.
fn nearest_spot(world: &World, id: EntityId, from: TilePos) -> Option<(TilePos, f32)> {
    // The island pass phase 2: by the travel estimate, none past the cap
    // (the own Home's never is).
    if crate::systems::distance::free(world).is_some() {
        let cap = spot_cap(world);
        let own = home_of(world, id);
        return spots_near(world, id, from)
            .into_iter()
            .map(|s| (spot_dist(world, from, s.tile), s))
            .filter(|(d, s)| *d <= cap || matches!(s.kind, SpotKind::Home(h) if Some(h) == own))
            .map(|(d, s)| (d, s.tile))
            .min()
            .map(|(_, t)| (t, 0.0));
    }
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
    best_spot_kind(world, id, from).map(|(s, social)| (s.tile, social))
}

/// [`best_spot`] with the spot's kind (a Home spot's HangOut runs inside it).
pub fn best_spot_kind(world: &World, id: EntityId, from: TilePos) -> Option<(Spot, f32)> {
    spot_pick(world, id, from).0
}

/// [`best_spot_kind`], and whether there was no spot within the cap
/// (`[distance.free]` on: the stay-in's condition, island fix round F3).
fn spot_pick(world: &World, id: EntityId, from: TilePos) -> (Option<(Spot, f32)>, bool) {
    let free_on = crate::systems::distance::free(world).is_some();
    let mut spots = spots_near_with(world, id, from, true);
    if spots.is_empty() {
        return (None, free_on);
    }
    // L2 shadow fixes item 5: a spot past `spot_walk_cap_tiles` is not worth
    // the walk (195 min at midnight to an empty corner): within the cap,
    // else the nearest alone.
    let cap = spot_cap(world);
    // Lives at work phase 2 (§ 3.1): the own Home is never past the cap.
    let own = home_of(world, id);
    // Lives at work phase 2 (§ 3.3): after an empty leave, only spots with
    // company now (none: no HangOut).
    // The stay-in's test (island fix round F3) is the travel alone, over
    // every spot before that filter: none within the cap.
    let none_within = |all: &[Spot]| free_on && !all.iter().any(|s| spot_dist(world, from, s.tile) <= cap);
    let mut before: Option<Vec<Spot>> = None;
    if crate::systems::household::company_only(world, id) {
        before = Some(spots.clone());
        spots.retain(|s| {
            let home = match s.kind {
                SpotKind::Home(h) => Some(h),
                _ => None,
            };
            crate::systems::household::company_at(world, id, s.tile, home)
        });
        if spots.is_empty() {
            return (None, before.as_deref().is_some_and(none_within));
        }
    }
    // The island pass phase 2 (`[distance.free]`): the cap is on the travel
    // estimate, and with none within there is no spot (an hour's company is
    // not walked 4 h for; island d30: 2,204 Unwind spot legs of 4 h or more).
    let dist: Vec<(Spot, u64)> = spots.iter().map(|&s| (s, spot_dist(world, from, s.tile))).collect();
    let near: Vec<(Spot, u64)> = dist
        .iter()
        .copied()
        .filter(|&(s, d)| d <= cap || matches!(s.kind, SpotKind::Home(h) if Some(h) == own))
        .collect();
    let spots: Vec<(Spot, u64)> = if !near.is_empty() {
        near
    } else if free_on {
        return (None, before.as_deref().is_none_or(none_within));
    } else {
        dist.iter().copied().min_by_key(|&(s, d)| (d, s.tile)).into_iter().collect()
    };
    let enemies = crate::systems::fixes::enemy_tiles(world, id);
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
    let best = spots
        .iter()
        .map(|(s, d)| {
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
            // L2 shadow fixes item 5: the hour's company on the corner now
            // (strangers too, a little), less a strong term per Enemy near
            // it (a runner went back 18 times to the street where six beat
            // him).
            let others = present.map_or(0, |v| v.iter().filter(|&&o| o != id).count());
            social += (0.1 * others as f32).min(0.5);
            social -= cfg_life_penalty(world) * crate::systems::fixes::enemies_near(&enemies, s.tile) as f32;
            // Lives at work phase 2 (§ 3.1, § 3.3, § 3.4): the household term
            // of a Home spot, the empty-spot memory, the carer's away mult
            // (each 1 or 0 off).
            let own_home = matches!(s.kind, SpotKind::Home(h) if Some(h) == home_of(world, id));
            if let SpotKind::Home(h) = s.kind {
                social += crate::systems::household::home_term(world, id, h);
            }
            let mult = crate::systems::household::empty_mult(world, id, s.tile)
                * crate::systems::household::away_mult(world, id, own_home);
            ((0.5 + social).max(0.0) * spot_travel(world, from, s.tile) * mult, *d, *s, social)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.tile.cmp(&a.2.tile)))
        .map(|(_, _, s, social)| (s, social));
    (best, false)
}

/// Island fix round F3 (OPEN-190; I1's stay-in, Dylan's D1 B): with
/// `[distance.free.stay_in]` and the household on, a standing Home (the
/// caller has found no spot within `spot_cap_ticks`): the Home and its
/// outside door: the person stays in, and the
/// evening runs inside it (its flatmates and Block-mates, else alone).
fn stay_in_home(world: &World, id: EntityId) -> Option<(EntityId, TilePos)> {
    crate::systems::distance::stay_in(world)?;
    if !crate::systems::household::on(world) {
        return None;
    }
    let h = home_of(world, id)?;
    world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (h, world.outside_door(b)))
}

fn home_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Household>(id).and_then(|h| h.home)
}

// ---------------------------------------------------------------------------
// Unwind: considerations and plans (plan L14, L16)
// ---------------------------------------------------------------------------

/// `[life] spot_enemy_penalty` (L2 shadow fixes item 5).
fn cfg_life_penalty(world: &World) -> f32 {
    world.config.life.spot_enemy_penalty
}

fn adult_free(world: &World, id: EntityId) -> bool {
    crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
}

/// Spec § 2: Unwind is satisfied at `fun_satisfied` (and off for a child,
/// a prisoner, or with leisure off).
pub fn unwind_satisfied(world: &World, id: EntityId) -> bool {
    !adult_free(world, id)
        || world.comp::<Needs>(id).is_none_or(|n| n.fun >= world.config.needs.fun_satisfied)
            // The pressure pass phase 2: the lonely with coins look for company.
            && !crate::systems::customers::lonely_wants(world, id)
}

/// Day gaps 1 (2026-10-11): `job`'s shift is being worked now: on shift on
/// a working day (the agent's own: a guard's staggered rest day) that no
/// strike struck. A rest day's shift hours are free.
pub fn working_now(world: &World, id: EntityId, job: &Job) -> bool {
    let key = job.shift_key_at(world.tick);
    job.on_shift(world.tick_of_day())
        && crate::exec::routine::workday_of(world, id, job, key)
        && job.struck_shift != Some(key)
}

/// The agent's off hours. Day gaps 1 (2026-10-11): a job holder's are its
/// own: not working a shift now, and the next workday shift not due (a
/// rest day, a shift already worked or struck) or further off than the
/// walk to work plus `[leisure] shift_lead_ticks`. The wall clock's
/// Evening and Night left night crews, the venues' evening staff, night
/// guards and rest days at Unwind x 0.3 < Idle, at home for 6-18 h. The
/// jobless and execs keep the Evening and Night (an exec once the office
/// has shut).
pub fn off_hours(world: &World, id: EntityId) -> bool {
    if let Some(j) = world.comp::<Job>(id) {
        if working_now(world, id, j) {
            return false;
        }
        let key = j.next_shift_key(world.tick);
        if !crate::exec::routine::workday_of(world, id, j, key)
            || j.last_shift_day == Some(key)
            || j.struck_shift == Some(key)
        {
            return true;
        }
        let Some(door) = j.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door) else {
            let until = u64::from(j.ticks_until_shift(world.tick_of_day()));
            return until > world.config.leisure.shift_lead_ticks;
        };
        let until = u64::from(j.ticks_until_shift(world.tick_of_day()));
        // Transit T16: the trip as it would be made to arrive by the lead
        // (`transit::trip_due`, the departure gate's).
        return !crate::systems::transit::trip_due(world, id, door, until, world.config.leisure.shift_lead_ticks);
    }
    if !matches!(world.phase(), DayPhase::Evening | DayPhase::Night) {
        return false;
    }
    crate::systems::life::exec_corp(world, id).is_none() || crate::systems::life::exec_shift_over(world)
}

/// Plan L14 (spec § 2): `U(fun)` Logistic{8, 0.5}; off hours ([`off_hours`]:
/// the agent's own), else `gate_or(0.3)`; the best rung's score (gain × the walk ×
/// pride on the mid and high rungs × courage at the FightPit and Den ×
/// greed-weighted affordability). `None`: no rung reachable.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if unwind_satisfied(world, id) {
        return None;
    }
    // Shadow V4 bug 2: no leisure in a day worker's small hours (woken at
    // 23:30, it stood on a corner at 03:00; a soft 0.3 gate still beat Idle
    // with nothing else awake: the re-shadow read 606 such HangOuts against
    // 638): the night is Idle's, and Idle at Home at night is bed.
    if crate::exec::routine::day_worker_night(world, id) {
        return None;
    }
    let n = world.comp::<Needs>(id)?;
    // Hunger fix item 5 (P5f): a worker on shift stays at work (day gaps
    // 1: a shift being worked; a rest day's shift hours are free).
    if crate::systems::fixes::shift_first(world) && world.comp::<Job>(id).is_some_and(|j| working_now(world, id, j)) {
        return None;
    }
    let pick = choice(world, id, false)?;
    // The pressure pass phase 2 (OPEN-176): a lonely body whose pick is a
    // companion's, host's or dancer's venue unwinds for the company.
    let fun_u = Curve::Logistic { k: 8.0, mid: 0.5 }.eval(urgency(n.fun));
    let need = fun_u.max(crate::systems::customers::lonely_need(world, id, pick.venue));
    let cs = vec![
        Consideration::raw("U(fun) or lonely", urgency(n.fun), need),
        Consideration::new("off hours", can(off_hours(world, id)), gate_or(0.3)),
        Consideration::raw("rung", pick.score, (0.5 + pick.score).min(1.0)),
        // Deviation (measured): food before fun. Without it a hungry body
        // took the evening at a spot or a venue over its meal (fun decay off:
        // seed 42/43 starvation 22/16; on: 23-45; e8355d4 12/25).
        Consideration::new("fed", n.hunger, Curve::Logistic { k: 12.0, mid: 0.4 }),
    ];
    // L2 shadow fixes item 3: the tired go to bed, not to a corner.
    let mut cs = cs;
    if let Some(c) = crate::utility::goals::energy_brake(world, n) {
        cs.push(c);
    }
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
        // Lives at work phase 2 (§ 3.1): a HangOut at a Home (one's own, or
        // a friend's) walks to the Home and runs inside it.
        (Some(h), Some(_)) if pick.act == ActionKind::HangOut => {
            if Some(h) == home_of(world, id) {
                steps.push(step(ActionKind::GoTo(LocationKey::Home), None));
            } else {
                steps.push(step(ActionKind::GoTo(LocationKey::TargetHome), Some(h)));
            }
            steps.push(step(ActionKind::HangOut, None));
        }
        (Some(v), _) => {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(v)));
            let kind = kind_of(world, v);
            let purse = spendable(world, id) - price_of(world, v);
            if kind == Some(BuildingKind::Den) && purse >= 3 && !crate::utility::goals::drank_today(world, id) {
                steps.push(step(ActionKind::Drink, Some(v)));
            }
            steps.push(step(pick.act, Some(v)));
            // R6a (OPEN-217): the window read at the body's arrival, not at
            // the plan (a pit planned at 20:30 with a walk into the window
            // took no bet).
            let window = match world.config.trade_verbs.as_ref() {
                Some(_) => {
                    let walk = world
                        .comp::<Position>(id)
                        .zip(world.comp::<Building>(v))
                        .map_or(0, |(p, bd)| crate::exec::timed_walk(world, id, p.tile, bd.door));
                    bet_window_at(world, u64::from(world.tick_of_day()) + walk)
                }
                None => bet_window(world),
            };
            if kind == Some(BuildingKind::FightPit) && window && purse >= 1 {
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
    // Lives at work phase 2 (§ 3.1): a Home has no seats to hold.
    let at_home = pick.act == ActionKind::HangOut;
    world.unwind.insert(id, pick);
    // Plan L30: with the LOD budget on, a Seat at the venue is held from the
    // plan to the arrival (`unwind_plan` installs without `reserve_for`).
    if let Some(v) = target.filter(|_| !at_home) {
        let expires = world.tick + world.config.exec.reservation_ttl;
        world.reserve(id, crate::exec::ReservationKind::Seat { building: v }, expires);
    }
    Some(Plan { goal: GoalKind::Unwind, target, steps, started_tick: world.tick })
}

/// Plan L16: a Socialise with nobody to talk to here goes to a HangOut
/// when broke (< 2 coins) or when the best spot outscores the local Bar's
/// walk-weighted 1.0. `None`: the planner as before.
pub fn hangout_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if crate::systems::social::best_colocated_partner(world, id, -1.0, false).is_some() {
        return None;
    }
    let from = origin(world, id)?;
    let (spot, social) = best_spot_kind(world, id, from)?;
    let tile = spot.tile;
    let spot_score = (0.5 + social) * spot_travel(world, from, tile);
    let bar = world.local(id, BuildingKind::Bar).and_then(|b| crate::systems::life::tiles_to(world, id, b));
    let bar_score = bar.map_or(0.0, |t| travel(world, t));
    if coins(world, id) >= 2 && spot_score < bar_score {
        return None;
    }
    // Lives at work phase 2 (§ 3.1): a Home spot's HangOut runs inside the
    // Home (the pick's `venue`); the walk is to the Home itself.
    let home = match spot.kind {
        SpotKind::Home(h) => Some(h),
        _ => None,
    };
    let pick = UnwindPick {
        rung: Rung::Free,
        act: ActionKind::HangOut,
        venue: home,
        spot: Some(tile),
        score: spot_score,
        stay_in: false,
    };
    let steps = match home {
        Some(h) if Some(h) == home_of(world, id) => {
            vec![step(ActionKind::GoTo(LocationKey::Home), None), step(ActionKind::HangOut, None)]
        }
        Some(h) => vec![step(ActionKind::GoTo(LocationKey::TargetHome), Some(h)), step(ActionKind::HangOut, None)],
        None => vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::HangOut, None)],
    };
    world.unwind.insert(id, pick);
    Some(Plan { goal: GoalKind::Socialise, target: None, steps, started_tick: world.tick })
}

/// Plan L16: `already_satisfied(Socialise)`'s "no drink possible" clause is
/// dropped when a spot is within `hangout_reach`.
pub fn spot_in_reach(world: &World, id: EntityId) -> bool {
    origin(world, id).is_some_and(|from| {
        // The island pass phase 2: on the travel estimate, within the spot cap.
        if crate::systems::distance::free(world).is_some() {
            let cap = spot_cap(world);
            return spots_near(world, id, from).iter().any(|s| spot_dist(world, from, s.tile) <= cap);
        }
        spots_near(world, id, from).iter().any(|s| s.tile.manhattan(from) <= world.config.leisure.hangout_reach)
    })
}

/// The island pass phase 2 (`[distance.free]`): the Bar a Socialise would
/// walk to (`World::local`) is past `spot_cap_ticks` from where the body
/// sets out, so no drink is possible for it (the base walked to the nearest
/// Bar at any distance once no spot was left: 1,809 Socialise Bar legs on
/// the island at d30, 43 % at 2 h or more). `false` off.
pub fn bar_past_cap(world: &World, id: EntityId) -> bool {
    let Some(f) = crate::systems::distance::free(world) else { return false };
    let Some(from) = origin(world, id) else { return false };
    world
        .local(id, BuildingKind::Bar)
        .and_then(|b| world.comp::<Building>(b))
        .is_some_and(|bd| crate::systems::distance::travel(world, from, bd.door) > f.spot_cap_ticks)
}

/// `LocationKey::Spot`: the spot of the agent's pick.
pub fn spot_of(world: &World, id: EntityId) -> Option<TilePos> {
    // Jobs & lives phase 2: the Pitch goal's street tile (its stall, or the
    // labour corner at dawn).
    if world.comp::<crate::components::Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Pitch) {
        return crate::systems::streetwork::street_tile(world, id);
    }
    // Lives at work phase 4: a cab driver's leg (the rider's door, the work door).
    // Review fix 2: only in the Work plan (another goal's plan reads its own spot).
    if crate::systems::streetwork::driving(world, id) {
        if let Some(t) = crate::systems::streetwork::fare_tile(world, id) {
            return Some(t);
        }
    }
    // Round 3 S5 (OPEN-217): a cab driver standing by drives to his rank.
    if let Some(t) = crate::systems::streetwork::rank_spot(world, id) {
        return Some(t);
    }
    world.unwind.get(&id).and_then(|p| p.spot)
}

/// Plan L14 (spec § 2): `Earn` reads `U(fun)` as a second term on Beg and
/// Scavenge when fun is below 0.3 and the purse below the cheap rung (the
/// two actions' plan cost, `PlanCtx::fun_term`; review fix: not the goal).
pub fn earn_fun_term(world: &World, id: EntityId) -> Option<f32> {
    let fun = world.comp::<Needs>(id)?.fun;
    (fun < 0.3 && coins(world, id) < 2).then(|| urgency(fun))
}

// ---------------------------------------------------------------------------
// Execution (plan L14, L15, L18)
// ---------------------------------------------------------------------------

/// Plan L14: a leisure step may start (money is re-checked here, so a
/// refusal moves none).
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
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
        // Lives at work phase 2 (§ 3.1): a HangOut runs inside the Home it was picked for.
        ActionKind::HangOut => {
            world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
                || crate::systems::household::hanging_at_home(world, id).is_some()
        }
        ActionKind::Preach => world.comp::<Position>(id).is_some_and(|p| p.building.is_none()),
        ActionKind::Collect => {
            // Lives at work phase 4b (§ 2.3): a pimp's collection at a pitch.
            if let Some(ok) = crate::systems::vice::collect_can_start(world, id, target) {
                return ok;
            }
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
            if paid > 0 {
                if let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) {
                    let e = world.probe.venue_paid.entry(format!("{kind:?}")).or_default();
                    e.0 = e.0.saturating_sub(1);
                    e.1 -= paid;
                    note_unmirrored(world, id, kind, -1);
                }
            }
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

/// L2 shadow fixes item 4: an abandoned paid step keeps its purchase (no
/// refund): a meal out is eaten, an entry gives its fun for the share of
/// the visit had; a bet stands.
pub fn abort_paid(world: &mut World, id: EntityId, kind: ActionKind, started: Tick) {
    let b = world.comp::<Brain>(id).and_then(|b| b.current_step()).and_then(|s| s.target).or_else(|| here(world, id));
    match kind {
        ActionKind::EatOut => {
            let (units, _) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            if units > 0 {
                let cfg = world.config.needs.clone();
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    crate::needs::eat(n, &cfg);
                }
                crate::systems::health::ate(world, id, crate::components::MealKind::Real);
                world.mark_day(id, crate::components::trace_flags::ATE);
                world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            }
        }
        ActionKind::Enjoy => {
            world.pending_purchase.remove(&id);
            if let Some(b) = b {
                let share = (world.tick.saturating_sub(started) as f32 / 90.0).min(1.0);
                let gain = gain_at(world, b) * share;
                add_fun(world, id, gain);
            }
        }
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
            phase4_visit(world, id, b);
            // Lives at work phase 1 (§ 2.1): the visit queues at the venue's counter.
            let price = price_of(world, b);
            crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, price);
        }
        ActionKind::EatOut => {
            let (units, _) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            let Some(b) = target else { return };
            if units > 0 {
                // Lives at work phase 1 (§ 2.1): the meal is the cook's.
                let price = price_of(world, b);
                crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, price);
                crate::systems::bins::venue_meal(world, b);
                let cfg = world.config.needs.clone();
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    crate::needs::eat(n, &cfg);
                }
                crate::systems::health::ate(world, id, crate::components::MealKind::Real);
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
                // Lives at work phase 1 (§ 2.1): the table game names its croupier.
                crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, 0);
            }
        }
        ActionKind::HangOut => hangout_done(world, id, started),
        ActionKind::Preach => {
            preach_done(world, id);
            leave_spot(world, id);
        }
        // Lives at work phase 4b (§ 2.3): a pimp's collection, else the leader's.
        ActionKind::Collect if !crate::systems::vice::pimp_collect(world, id, target) => {
            collect_step(world, id, target)
        }
        _ => {}
    }
}

/// Jobs & lives phase 4: a paid venue visit's vice and fashion terms (the
/// tip, intimacy, status, a Boutique's clothing; nothing with both off).
fn phase4_visit(world: &mut World, id: EntityId, b: EntityId) {
    if !crate::systems::vice::on(world) && !crate::systems::fashion::on(world) {
        return;
    }
    // Jobs & lives phase 5: a Dollhouse's visit, an Arcade's unit (nothing off).
    crate::systems::dollhouse::on_visit(world, b, price_of(world, b));
    if kind_of(world, b) == Some(BuildingKind::Arcade) {
        crate::systems::braindance::on_arcade_visit(world, b);
    }
    let price = price_of(world, b);
    let high = rung_of(world, b, price) == Rung::High;
    // The pressure pass phase 2: a lonely client's visit (counted before
    // the visit's intimacy lands).
    if crate::systems::customers::lonely(world, id) > 0.0 && crate::systems::customers::worker_on(world, b) {
        ownership::jobs2_row(world).lonely_visits += 1;
    }
    crate::systems::vice::on_visit(world, id, b, price);
    crate::systems::fashion::on_visit(world, id, b, high);
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
    // Lives at work phase 2 (§ 3.1): inside a Home the company is the
    // household present, not the street registry.
    let at_home = crate::systems::household::hanging_at_home(world, id);
    let others: Vec<EntityId> = match at_home {
        Some(h) => crate::systems::household::residents_inside(world, h, id)
            .into_iter()
            .filter(|&o| world.has::<Brain>(o))
            .collect(),
        None => tile
            .and_then(|t| world.hangouts.get(&t))
            .map(|v| v.iter().copied().filter(|&o| o != id && world.has::<Brain>(o)).collect())
            .unwrap_or_default(),
    };
    let mut gain = world.config.leisure.gain.hangout_hour * hours.max(0.5);
    // Island fix round F3 (OPEN-190): the stay-in's hour (the own Home
    // past the spot cap) is `stay_in.fun_hour` alone, and never less in
    // company; it is not a spot left empty.
    let stay_in =
        world.unwind.get(&id).is_some_and(|p| p.stay_in) && at_home.is_some_and(|h| Some(h) == home_of(world, id));
    let stay_cfg = crate::systems::distance::stay_in(world).filter(|_| stay_in).cloned();
    if let Some(c) = &stay_cfg {
        ownership::jobs2_row(world).stay_in_hangouts += 1;
        gain = gain.max(c.fun_hour * hours.max(0.5));
    }
    // L2 shadow fixes item 5: an hour alone on the corner is `spot_lone_fun`
    // of one in company.
    if let (true, Some(c)) = (others.is_empty(), &stay_cfg) {
        gain = c.fun_hour * hours.max(0.5);
    } else if others.is_empty() {
        gain *= world.config.life.spot_lone_fun;
        // Lives at work phase 2 (§ 3.3): a spot left early is remembered empty.
        if world.tick.saturating_sub(started) < world.config.leisure.hangout_ticks {
            // Review fix 6: a Home's spot is keyed by its outside door (the
            // tile `SpotKind::Home` is scored by), not the interior tile.
            let spot = match at_home {
                Some(h) => world.comp::<Building>(h).map(|b| world.outside_door(b)),
                None => tile,
            };
            if let Some(t) = spot {
                crate::systems::household::remember_empty(world, id, t);
            }
            // The own Home left empty is counted apart (the fix round's device).
            if at_home.is_some_and(|h| Some(h) == home_of(world, id)) {
                ownership::jobs2_row(world).home_empty_leaves += 1;
            }
        }
    }
    if let Some(h) = at_home {
        let own = Some(h) == home_of(world, id);
        let row = ownership::jobs2_row(world);
        if own {
            row.home_hangouts += 1;
        } else {
            row.friend_hangouts += 1;
        }
    }
    add_fun(world, id, gain);
    let chat = world.config.needs.chat_belonging;
    add_belonging(world, id, chat);
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
    crate::systems::gossip::exchange(world, id, p, crate::systems::gossip::Venue::Chat);
    if world.edge(id, p).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent)) {
        let today = world.day();
        world.last_met.insert(pair(id, p), today);
    }
    // Lives at work phase 2 (§ 3.1): an evening at home with the spouse is
    // counted; street dice indoors only under `home_dice`.
    if at_home.is_some() {
        if world.spouse_of(id) == Some(p) {
            ownership::jobs2_row(world).home_with_spouse += 1;
        }
        if !world.config.household.home_dice {
            return;
        }
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
/// on the world stream, L31). At `bout_hour` each open FightPit pairs the
/// two Fighters on shift longest since their last bout (Shadow V4 bug 17:
/// the two fittest fought six nights running while the rest sat; ties the
/// fitter, then the lower id): the loser loses 0.3
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
        // L2 shadow fixes item 9: a working day's shift, and in the pit (a
        // body inside, or a Statistical one credited by the clock): a
        // fighter won a bout while wandering Mid East on her rest day.
        let present = |f: EntityId| {
            world.comp::<Job>(f).is_some_and(|j| crate::exec::routine::is_workday(j.shift_key_at(world.tick)))
                && world.comp::<Brain>(f).is_some_and(|b| {
                    b.lod == Lod::Statistical || world.comp::<Position>(f).is_some_and(|p| p.building == Some(pit))
                })
        };
        let last_bout = |f: EntityId| {
            world
                .comp::<crate::components::Memory>(f)
                .and_then(|m| m.entries.iter().filter(|e| e.kind == MemoryKind::Bout).map(|e| e.tick).max())
                .unwrap_or(0)
        };
        let mut fighters: Vec<(f32, EntityId, u64)> = ownership::staff_at(world, pit)
            .into_iter()
            .filter(|&f| world.comp::<Job>(f).is_some_and(|j| j.role == Role::Fighter && j.on_shift(tod)))
            .filter(|&f| crate::systems::law::living(world, f) && !world.has::<Sentence>(f))
            .filter(|&f| present(f))
            .map(|f| (crate::systems::law::fighting(world, f), f, last_bout(f)))
            .collect();
        fighters.sort_by(|a, b| a.2.cmp(&b.2).then(b.0.total_cmp(&a.0)).then(a.1.cmp(&b.1)));
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
        // L2 shadow fixes item 9: a bout is sanctioned, not an assault (its
        // `Lost` was read as Assaulted: rumours, news Stories, dread 0.82,
        // a best friend turned Enemy).
        world.remember(winner, MemoryKind::Bout, Some(loser), 0.4, 0.3, false);
        world.remember(loser, MemoryKind::Bout, Some(winner), 0.5, -0.4, false);
        if let Some(s) = world.comp_mut::<crate::components::Skills>(winner) {
            s.fighting = (s.fighting + 0.01).min(1.0);
        }
        let mut paid_out = 0;
        for (who, stake, _) in bets {
            // Lives at work phase 1 (§ 2.1): the bettors are the bout's served.
            crate::systems::service::arrive(world, who, pit, crate::components::ServeKind::Attendee, stake);
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

/// Jobs & lives phase 7b: the paid rungs open to `id` on its evening, one
/// venue per kind: from its day (its Home's door, or for a worker the
/// rectangle to its workplace's door; its Home's district first), a venue
/// within the Full ladder's `venue_walk_cap_ticks` of it, affordable from
/// the spendable purse (`stat_daily`'s, and `calibrate`'s eligibility for
/// `StatTable::p_venue`). Uncapped, the off-screen evening reached the
/// city's few vice venues from any district (Lounge visits 6.6x, StripClub
/// 2.6x the Full tier's on the calibration city), drained the purse, and
/// the food bought after it was synth.
fn evening_cands(world: &World, id: EntityId) -> Vec<Cand> {
    let Some((day_from, district)) = evening_day(world, id) else { return Vec::new() };
    let reach = Some(world.config.leisure.venue_walk_cap_ticks);
    paid_candidates(world, id, day_from, spendable(world, id), Some(district), reach)
}

/// The agent's day (`evening_cands`): its Home's door (else its tile), or
/// for a worker the rectangle to its workplace's door; and the Home's
/// district.
fn evening_day(world: &World, id: EntityId) -> Option<(Origin, DistrictId)> {
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let from = home
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| world.outside_door(b))
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))?;
    let work = world
        .comp::<crate::components::Job>(id)
        .and_then(|j| j.employer)
        .and_then(|e| world.comp::<Building>(e))
        .map(|bd| world.outside_door(bd));
    Some((work.map_or(Origin::At(from), |w| Origin::Day(from, w)), world.district_of(from)))
}

/// Jobs & lives phase 7b: the venues of `id`'s evening for `p_venue`, one
/// per kind of the ladder: the nearest standing venue of the kind to its
/// day (its Home's district first) within the Full ladder's walk cap that
/// is open, lets the agent in (the door policy, the room, a Den's table) and
/// is affordable, the high rung only for those who reach it.
fn evening_venues(world: &World, id: EntityId) -> Vec<(BuildingKind, EntityId)> {
    let purse = spendable(world, id);
    if purse < 1 {
        return Vec::new();
    }
    let Some((day_from, district)) = evening_day(world, id) else { return Vec::new() };
    let cap = world.config.leisure.venue_walk_cap_ticks;
    let high = high_ok(world, id);
    let mut out = Vec::new();
    let max_d = reach_tiles(world, id, Some(cap));
    for kind in LADDER {
        // The perf pass phase 2: the lowest `(other district, door distance,
        // id)` over the venues that pass, as the old sort of them all had it:
        // the nearest that passes in the Home's district, else the nearest
        // that passes outside it, each off the kind's grid.
        let pass = |b: EntityId| {
            standing(world, b)
                && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
                // Phase-8 review: the Full ladder's gates (a closed,
                // swept, dark, raided, unstaffed or unbuilt venue takes
                // nobody; the door policy; the room; a shut table).
                && open(world, b)
                && door_seen(world, id, b)
                && has_room(world, id, b)
                && (kind != BuildingKind::Den || table_open(world, b))
                && world
                    .comp::<Building>(b)
                    .is_some_and(|bd| crate::exec::timed_walk(world, id, day_from.near(bd.door), bd.door) <= cap)
        };
        let Some((_, b)) = venue_by_distance(world, kind, day_from, max_d, |_, b| {
            world.district_of_building(b) == district && pass(b)
        })
        .or_else(|| {
            venue_by_distance(world, kind, day_from, max_d, |_, b| world.district_of_building(b) != district && pass(b))
        }) else {
            continue;
        };
        let price = price_of(world, b);
        if (!high && rung_of(world, b, price) == Rung::High && !lonely_client(world, id, b))
            || (kind != BuildingKind::Bar && 2 * price > purse)
        {
            continue;
        }
        out.push((kind, b));
    }
    out
}

/// The venue kinds of [`evening_venues`] (`calibrate`'s eligible agent-days
/// per kind).
pub fn evening_kinds(world: &World, id: EntityId) -> Vec<BuildingKind> {
    evening_venues(world, id).into_iter().map(|(k, _)| k).collect()
}

/// The `StatTable::p_venue` key of a venue kind.
pub fn venue_key(kind: BuildingKind) -> String {
    format!("{kind:?}")
}

const PURPOSE_STAT_VENUE: u64 = 0x4c45_0017;

/// Plan L17, 21:00: the Statistical evening. With the table's `p_venue`
/// (Jobs & lives phase 7b), every Statistical adult takes each paid rung of
/// [`evening_cands`] on the kind's learned rate `p` (the Full tier's paid
/// visits of the kind per eligible agent-day): `floor(p)` visits and one
/// more on the fraction (a pure hash of the agent, the day and the kind),
/// while the purse affords them; an adult below
/// `fun_satisfied` who took none has the free rung (`free_stat`). Without
/// it, every adult below `fun_satisfied`, ascending, takes the best rung
/// `choice` would, the free one when no paid rung outscores an hour's
/// HangOut. A visit pays as on screen, gains the rung's fun and adds a
/// visit; a table game's draw on `WordNs::Leisure` keyed `(day, id)`.
pub fn stat_daily(world: &mut World) {
    let day = world.day();
    let ids: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    let g = world.config.leisure.gain.clone();
    if let Some(pv) = world.stat_table.as_ref().and_then(|t| t.p_venue.clone()) {
        stat_learned_evening(world, &ids, &pv);
        return;
    }
    for id in ids {
        if unwind_satisfied(world, id) {
            continue;
        }
        let cands = evening_cands(world, id);
        let pick = cands.iter().find(|c| c.act == ActionKind::EatOut).cloned().or_else(|| {
            cands.iter().max_by(|a, b| a.score.total_cmp(&b.score).then_with(|| b.venue.cmp(&a.venue))).cloned()
        });
        // The free rung's on-screen score (an hour's HangOut, no walk).
        let free_score = g.hangout_hour * world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        let Some(c) = pick.filter(|c| c.score > free_score) else {
            add_fun(world, id, g.free_stat);
            continue;
        };
        let Some(b) = c.venue else { continue };
        if queue_at_counter(world, id, b, c.act, None, 0) {
            continue;
        }
        let mut rng = world.rng.word(WordNs::Leisure, day, u64::from(id.index));
        stat_visit(world, id, b, c.act, None, &mut rng);
    }
}

/// The pre-view pass phase 3 (OPEN-145): an off-screen visit drawn into a
/// counter body's shift (`counter_due`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CounterVisit {
    /// The slot's tick (a multiple of `counter_settle_window` minutes).
    pub at: Tick,
    pub who: EntityId,
    pub venue: EntityId,
    pub act: ActionKind,
    /// The evening's purse the visit was rolled on (`stat_visit`'s `evening`).
    pub evening: Option<i64>,
    /// The day it was rolled and its ordinal that day (the visit's stream).
    pub day: u64,
    pub k: u32,
}

const PURPOSE_COUNTER: u64 = 0x4c45_0018;

/// The pre-view pass phase 3 (OPEN-145, P3): `b`'s counter bodies, the
/// staff of a `counter` shape that are bodies (not Statistical), living,
/// not held: ascending.
pub fn counter_bodies(world: &World, b: EntityId) -> Vec<EntityId> {
    world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| {
            // Round 4 T4 (OPEN-217): a serving body of `[verbs] night_rows`
            // (a dancer, a club hand) draws the venue's off-screen visits
            // into its shift as a counter body does (a Club with only a body
            // Host took its whole evening in one 21:00 batch).
            (world
                .comp::<crate::components::Job>(s)
                .is_some_and(|j| crate::systems::shift::shape_of(world, j.role) == crate::config::ShiftShape::Counter)
                || crate::systems::service::night_row(world, s, false))
                && world.comp::<Brain>(s).is_some_and(|br| br.lod != Lod::Statistical)
                && !world.has::<crate::components::Sentence>(s)
                && crate::systems::law::living(world, s)
        })
        .collect()
}

/// The pre-view pass phase 3 (OPEN-145): the 21:00 evening's visit `k` of
/// `id` at `b`, when `b` has a counter body, is drawn (purpose-keyed: the
/// agent, the venue, the day, `k`) into a `counter_settle_window` slot of
/// the next 24 hours inside the bodies' shifts and queued (`counter_due`
/// pays and serves it then, so the day cook has the day's customers and a
/// barkeep serves them as they come, not in one 21:00 batch). `true` when
/// queued; with no counter body the visit is the evening's, as before (an
/// off-screen venue keeps its economy: the mirror).
fn queue_at_counter(
    world: &mut World,
    id: EntityId,
    b: EntityId,
    act: ActionKind,
    evening: Option<i64>,
    k: u32,
) -> bool {
    let Some(window) = world.config.shift.as_ref().and_then(|c| c.counter.as_ref()).map(|c| c.counter_settle_window)
    else {
        return false;
    };
    let bodies = counter_bodies(world, b);
    if bodies.is_empty() {
        return false;
    }
    // The union of the bodies' shift minutes over the next 24 hours.
    let now = world.tick;
    let day0 = now - now % TICKS_PER_DAY;
    let mut spans: Vec<(Tick, Tick)> = Vec::new();
    for &s in &bodies {
        let Some(j) = world.comp::<crate::components::Job>(s) else { continue };
        for &(a, e) in &j.shifts {
            for d in [day0, day0 + TICKS_PER_DAY] {
                let (lo, hi) = ((d + Tick::from(a)).max(now), (d + Tick::from(e)).min(now + TICKS_PER_DAY));
                if lo < hi {
                    spans.push((lo, hi));
                }
            }
        }
    }
    spans.sort_unstable();
    let mut union: Vec<(Tick, Tick)> = Vec::new();
    for (lo, hi) in spans {
        match union.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => union.push((lo, hi)),
        }
    }
    let total: Tick = union.iter().map(|&(lo, hi)| hi - lo).sum();
    if total == 0 {
        return false;
    }
    let day = world.day();
    let u = crate::systems::econ::hash_unit(
        world.seed(),
        PURPOSE_COUNTER,
        u64::from(id.index) << 24 ^ u64::from(b.index),
        day * 64 + u64::from(k),
    );
    let mut off = ((u * total as f32) as Tick).min(total - 1);
    let mut at = now;
    for &(lo, hi) in &union {
        if off < hi - lo {
            at = lo + off;
            break;
        }
        off -= hi - lo;
    }
    let w = Tick::from(window.max(1));
    let at = (at - at % w).max(now);
    world.counter_due.push(CounterVisit { at, who: id, venue: b, act, evening, day, k });
    crate::systems::ownership::jobs2_row(world).counter_queued += 1;
    true
}

/// The pre-view pass phase 3 (OPEN-145): every `counter_settle_window`
/// minutes, the queued visits due now are made (`stat_visit`: paid, the
/// fun, the visit, and the encounter at the counter, `service::arrive`,
/// served by whoever is behind it). A visitor no longer Statistical (a body
/// now, planning its own evenings) or dead drops its visit.
pub fn counter_due(world: &mut World) {
    if world.counter_due.is_empty() {
        return;
    }
    let w = world.config.shift.as_ref().and_then(|c| c.counter.as_ref()).map_or(1, |c| c.counter_settle_window.max(1));
    if !world.tick.is_multiple_of(Tick::from(w)) {
        return;
    }
    let now = world.tick;
    let (due, rest): (Vec<CounterVisit>, Vec<CounterVisit>) =
        std::mem::take(&mut world.counter_due).into_iter().partition(|v| v.at <= now);
    world.counter_due = rest;
    for v in due {
        if world.comp::<Brain>(v.who).is_none_or(|b| b.lod != Lod::Statistical)
            || !crate::systems::law::living(world, v.who)
            || !world.has::<Building>(v.venue)
            || !open(world, v.venue)
        {
            continue;
        }
        let mut rng = world.rng.word(WordNs::Leisure, v.day, u64::from(v.who.index) ^ (u64::from(v.k) + 1) << 40);
        stat_visit(world, v.who, v.venue, v.act, v.evening, &mut rng);
    }
}

/// An adult's evening: its purse and its venues (`stat_learned_evening`).
type EveningPlan = (EntityId, i64, Vec<(BuildingKind, EntityId)>);

/// Jobs & lives phase 7b: the learned evening (`StatTable::p_venue`, the
/// Full tier's paid visits of each kind per adult-day). The tier's adults
/// and each one's evening venues (`evening_venues`, read before anyone
/// pays) are counted; the kind's visits due are its rate times the adults,
/// shared over the adults eligible for it: each takes `r = rate x adults /
/// eligible` visits (`floor(r)` and one more on the fraction, a pure hash
/// of the agent, the day and the kind), in a keyed order, while the
/// evening's purse affords the rung. So the tier pays the kinds as often as
/// the Full tier does, at the venues its people live and work by. An adult
/// below `fun_satisfied` who took none has the free rung.
fn stat_learned_evening(world: &mut World, ids: &[EntityId], pv: &std::collections::BTreeMap<String, f32>) {
    let day = world.day();
    let free = world.config.leisure.gain.free_stat;
    let plan: Vec<EveningPlan> = ids
        .iter()
        .copied()
        .filter(|&id| crate::systems::demography::is_adult(world, id))
        .map(|id| (id, spendable(world, id), evening_venues(world, id)))
        .collect();
    let adults = plan.len() as f32;
    let mut eligible = [0u32; LADDER.len()];
    for (_, _, venues) in &plan {
        for &(k, _) in venues {
            if let Some(i) = LADDER.iter().position(|&x| x == k) {
                eligible[i] += 1;
            }
        }
    }
    for (id, evening, mut venues) in plan {
        // In a keyed order (the ladder's order priced the dear rungs out
        // of a purse the cheap ones had spent).
        venues.sort_by_key(|&(k, _)| {
            let k = LADDER.iter().position(|&x| x == k).unwrap_or(0) as u64;
            splitmix64(world.seed() ^ PURPOSE_STAT_VENUE ^ (u64::from(id.index) << 20) ^ (day << 4) ^ k)
        });
        let mut rng = world.rng.word(WordNs::Leisure, day, u64::from(id.index));
        let mut went = false;
        for (kind, b) in venues {
            let i = LADDER.iter().position(|&k| k == kind).unwrap_or(0);
            // The pressure pass R6a (OPEN-217): the learned table has no
            // Dollhouse row (learned before the chip had clients: no
            // off-screen visit ever), so its rate is `[trade_verbs]`'s.
            let rate = pv
                .get(&venue_key(kind))
                .copied()
                .or_else(|| {
                    (kind == BuildingKind::Dollhouse)
                        .then(|| world.config.trade_verbs.as_ref().map(|c| c.dollhouse_p_venue))
                        .flatten()
                })
                .unwrap_or(0.0)
                .max(0.0);
            let mut r = rate * adults / eligible[i].max(1) as f32;
            // R6a: the few eligible for the Dollhouse (the lonely with the
            // purse, the rich) take at most a visit a night (three at 21:00).
            if kind == BuildingKind::Dollhouse && world.config.trade_verbs.is_some() {
                r = r.min(1.0);
            }
            let u = crate::systems::econ::hash_unit(
                world.seed(),
                PURPOSE_STAT_VENUE,
                u64::from(id.index),
                day * 16 + i as u64,
            );
            let n = r.floor() as u32 + u32::from(u < r.fract());
            for k in 0..n {
                // The pre-view pass phase 3 (OPEN-145): a venue with a
                // counter body serves the visit in the body's shift.
                if queue_at_counter(world, id, b, act_of(kind), Some(evening), (i as u32) << 8 | k) {
                    went = true;
                    continue;
                }
                if !stat_visit(world, id, b, act_of(kind), Some(evening), &mut rng) {
                    break;
                }
                went = true;
            }
        }
        if !went && !unwind_satisfied(world, id) {
            add_fun(world, id, free);
        }
    }
}

/// One off-screen paid visit of `id` at `b` (`stat_daily`): false when
/// nothing was paid (an emptied shelf, an empty purse). The rung must be
/// affordable from `evening` (the learned path: the purse the evening
/// opened with, as `evening_venues` read it; each rolled visit is one the
/// Full tier makes on its own hour's purse, not one the evening's earlier
/// visits priced out), else from the purse now; the coins come out of the
/// purse now.
fn stat_visit(
    world: &mut World,
    id: EntityId,
    b: EntityId,
    act: ActionKind,
    evening: Option<i64>,
    rng: &mut impl Rng,
) -> bool {
    let now = spendable(world, id);
    let purse = evening.unwrap_or(now);
    let price = price_of(world, b);
    if now < 1 || (act != ActionKind::Drink && 2 * price > purse) {
        return false;
    }
    // The pre-view pass phase 3 (OPEN-121, the mirror): the off-screen
    // visitor meets the same door a body does (`door_seen` picked it).
    if crate::systems::shift::counter_on(world) && !door_ok(world, id, b) {
        crate::systems::service::refused(world, id, b);
        return false;
    }
    // Phase 5: the gate's probe (not a CSV column): the evening's gross
    // spend, entries, meals, drinks and stakes.
    let mut gross = 0i64;
    match act {
        ActionKind::Drink => {
            let owner = world.owner_of(b);
            let paid = ownership::pay(world, Some(id), owner, 2, Flow::Drink);
            ownership::credit(world, b, paid);
            note_paid(world, id, b, paid);
            gross += paid;
            add_fun(world, id, gain_at(world, b));
            add_belonging(world, id, 0.15);
            world.remember(id, MemoryKind::Socialised, None, 0.2, 0.3, false);
            // Jobs & lives phase 5 (§ 5.3): a black unit from the Bar's dealer (nothing off).
            crate::systems::braindance::bar_visit(world, id, b, false);
        }
        ActionKind::Gamble => {
            // The stake from the purse now, as a Full round's.
            let stake = stake_of(world, now);
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
                return false;
            }
            let paid = pay_venue(world, id, b, price, Flow::Leisure);
            if paid <= 0 {
                return false;
            }
            gross += paid;
            world.take_stock(b, crate::components::Good::Food, 1);
            let cfg = world.config.needs.clone();
            if let Some(n) = world.comp_mut::<Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
            // Jobs & lives phase 7b: the meal as on screen (`on_complete`'s
            // EatOut): the venue's scraps, a real meal, the day's mark.
            crate::systems::bins::venue_meal(world, b);
            crate::systems::health::ate(world, id, crate::components::MealKind::Real);
            world.mark_day(id, crate::components::trace_flags::ATE);
            add_fun(world, id, gain_at(world, b));
            crate::systems::jobs::note_visit(world, b);
        }
        _ => {
            gross += pay_venue(world, id, b, price, Flow::Leisure);
            add_fun(world, id, gain_at(world, b));
            add_belonging(world, id, belonging_at(world, b));
            // A FightPit evening off screen bets as a table (the
            // Leisure stream), when brave enough and in funds. Not on the
            // learned path (phase 7b: the Full tier bet at a pit once in
            // three calibration cities against 1,320 pit visits).
            // The pressure pass R6a (OPEN-217): in the bet window the brave
            // back a fighter in tonight's bout (`bout_bet`), both paths; the
            // table game above only stood in for it (V10: every bout "bets
            // paid 0": the bettors were the Full tier's, who plan the
            // pit before the window opens).
            let bet = if kind_of(world, b) == Some(BuildingKind::FightPit) { bout_bet(world, id, b) } else { 0 };
            if bet > 0 {
                gross += bet;
            } else if evening.is_none()
                && kind_of(world, b) == Some(BuildingKind::FightPit)
                && !world.levers.ban_gambling
            {
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
            phase4_visit(world, id, b);
        }
    }
    world.stats.current.living.stat_spend += gross;
    // Lives at work phase 1 (§ 2.1): the off-screen visit queues at the
    // same counter a body's does (the mirror by construction).
    if gross > 0 {
        crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, gross);
    }
    gross > 0
}

// ---------------------------------------------------------------------------
// Fronts, the leader and the creed (plan L20)
// ---------------------------------------------------------------------------

/// A gang's standing fronts (owned by it, `front_of` it), ascending.
pub fn fronts_of(world: &World, gang: EntityId) -> Vec<EntityId> {
    // Jobs & lives phase 4: the Vice venues front too (none stands off).
    let mut out: Vec<EntityId> = [
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::Club,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Dollhouse,
    ]
    .iter()
    .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
    .filter(|&b| {
        standing(world, b)
            && world
                .comp::<Building>(b)
                .is_some_and(|bd| bd.owner == Some(gang) && bd.venue.as_ref().is_some_and(|v| v.front_of == Some(gang)))
    })
    .collect();
    out.sort_unstable();
    out
}

/// Plan L20: the coins a gang keeps back from the daily stipends for the
/// leader's Collect (`tribute_share` of the week's tribute and take).
pub fn set_aside(world: &World, gang: EntityId) -> i64 {
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
    if coins <= 0 {
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
    // Fronts on the city's deed that still name this gang close too.
    let mut named: Vec<EntityId> = fronts_of(world, gang);
    for k in [
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Dollhouse,
    ] {
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
    // Jobs & lives phase 4 (§ 5.1): with `[vice]` on a gang builds a
    // StripClub or a Brothel front too.
    // Jobs & lives phase 5 (§ 5.2): with the chip on, a Dollhouse front too.
    let kinds: &[BuildingKind] = if crate::systems::dollhouse::on(world) {
        &[
            BuildingKind::FightPit,
            BuildingKind::Den,
            BuildingKind::StripClub,
            BuildingKind::Brothel,
            BuildingKind::Dollhouse,
        ]
    } else if crate::systems::vice::on(world) {
        &[BuildingKind::FightPit, BuildingKind::Den, BuildingKind::StripClub, BuildingKind::Brothel]
    } else {
        &[BuildingKind::FightPit, BuildingKind::Den]
    };
    for &kind in kinds {
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
                    crate::systems::industry::pay_build(world, Some(gang), cost, Some(b), true);
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

/// L2 shadow fixes item 15: the start tick of the leader's running Lead
/// plan (`None` with the fixes off or no Lead under way).
fn lead_started(world: &World, gang: EntityId) -> Option<Tick> {
    let leader = world.comp::<Gang>(gang)?.leader?;
    world.comp::<Brain>(leader)?.plan.as_ref().filter(|p| p.goal == GoalKind::Lead).map(|p| p.started_tick)
}

/// Plan L20: the leader's Collect is due (the weekday, not done today).
/// L2 shadow fixes item 15: a round started on the collect day stays due
/// past midnight until its Collect runs.
pub(crate) fn collect_due(world: &World, gang: EntityId) -> bool {
    // Pressure pass F2 (OPEN-118's "Collected 0 to 0 members"): a week with
    // nothing to share is no Collect; the tribute waits for the next one.
    if collect_pool(world, gang) <= 0 {
        return false;
    }
    let today = world.day();
    let weekday = world.config.leisure.collect_weekday % 7;
    if let Some(t) = lead_started(world, gang) {
        let day = t / TICKS_PER_DAY;
        if day % 7 == weekday && world.comp::<Gang>(gang).is_some_and(|g| g.collected_day.is_none_or(|c| c < day)) {
            return true;
        }
    }
    today % 7 == weekday && world.comp::<Gang>(gang).is_some_and(|g| g.collected_day != Some(today))
}

/// Plan L20: a Call is due the evening after an order change. L2 shadow
/// fixes item 15: it stays due for a Lead plan's life (the leader walked
/// 330 min to the Hideout and dropped the Lead on the doorstep at night).
pub(crate) fn call_due(world: &World, gang: EntityId) -> bool {
    let Some(g) = world.comp::<Gang>(gang) else { return false };
    let pending = g.order_since > 0
        && g.called_for != Some(g.order_since)
        && world.tick.saturating_sub(g.order_since) < TICKS_PER_DAY;
    pending && (world.phase() == DayPhase::Evening || lead_started(world, gang).is_some())
}

/// Plan L20: the leader's `Lead` gate (`None`: not the leader, or nothing due).
pub fn lead_considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if world.has::<Sentence>(id) {
        return None;
    }
    // The pre-view pass phase 6 (P14): a lieutenant sent for to court.
    if crate::systems::leaders::attend_due(world, id) {
        return Some((vec![Consideration::raw("sent for to court", 1.0, 0.6)], 0.0));
    }
    let gang = is_leader(world, id)?;
    // The pre-view pass phase 6 (P13-P15): the round, court, an order to give.
    let due = collect_due(world, gang) || call_due(world, gang) || crate::systems::leaders::due(world, gang, id);
    if !due {
        return None;
    }
    Some((vec![Consideration::raw("collect or call", 1.0, 0.6)], 0.0))
}

/// `already_satisfied(Lead)`.
pub fn lead_satisfied(world: &World, id: EntityId) -> bool {
    if crate::systems::leaders::attend_due(world, id) {
        return false;
    }
    is_leader(world, id)
        .is_none_or(|g| !collect_due(world, g) && !call_due(world, g) && !crate::systems::leaders::due(world, g, id))
}

/// Plan L20: the round: `GoTo(front) → Collect` per front on a collect day,
/// then `GoTo(Hideout) → Collect` (the pay-out and the Call).
pub fn lead_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    // The pre-view pass phase 6 (P14): a lieutenant's walk to court.
    if crate::systems::leaders::attend_due(world, id) {
        return crate::systems::leaders::attend_plan(world, id);
    }
    let gang = is_leader(world, id)?;
    let hideout = world.hideout_of(gang)?;
    // The pre-view pass phase 6 (P13-P15): an order walked to a lieutenant,
    // court, or the round, whichever is due (the Collect and Call ride along).
    if let Some(plan) = crate::systems::leaders::plan(world, id, gang, hideout) {
        return Some(plan);
    }
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
        // The pre-view pass phase 6 (P13): a round stop collects in person.
        if let Some(b) = target {
            crate::systems::leaders::round_stop(world, id, gang, b);
        }
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

/// Plan L20: the coins a Collect shares, `tribute_share × (the week's
/// tribute + the fronts' take)`, net of the fronts' payroll the stipends
/// keep back (review fix), at most the treasury.
fn collect_pool(world: &World, gang: EntityId) -> i64 {
    let take: i64 = fronts_of(world, gang)
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()))
        .map(|v| v.take_week.max(0))
        .sum();
    let Some(g) = world.comp::<Gang>(gang) else { return 0 };
    let week = g.tribute_week.max(0) + take;
    let pool = (world.config.leisure.tribute_share * week as f32).floor() as i64;
    let keep = 2 * owner_payroll(world, Some(gang));
    pool.min((g.treasury - keep).max(0))
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
    let pool = collect_pool(world, gang);
    let mut members: Vec<EntityId> = crate::systems::gang::leader_ranking(world, gang);
    let mut rest: Vec<EntityId> = g.members.iter().copied().filter(|m| !members.contains(m)).collect();
    rest.sort_unstable();
    members.extend(rest);
    let mut paid_members = 0u32;
    let mut paid_total = 0i64;
    // Lives at work phase 6 (§ 4.5): the cut off the top, the leader's
    // `leader_cut` and the lieutenants' `lieutenant_cut` shared, before the
    // equal shares (none off).
    let mut pool = pool;
    let mut cut_note = String::new();
    if pool > 0 && crate::systems::careers::lieutenants_on(world) {
        let lts: Vec<EntityId> = members
            .iter()
            .copied()
            .filter(|&m| world.comp::<crate::components::GangMember>(m).is_some_and(|x| x.rank == 1))
            .collect();
        let boss = world.comp::<Gang>(gang).and_then(|g| g.leader).filter(|l| members.contains(l));
        let base = pool;
        let (mut to_leader, mut to_lts) = (0i64, 0i64);
        if let Some(l) = boss {
            let c = (world.config.gangs.leader_cut * base as f32).floor() as i64;
            to_leader = ownership::pay(world, Some(gang), Some(l), c.min(pool), Flow::Tribute);
            pool -= to_leader;
        }
        if !lts.is_empty() {
            let c = ((world.config.gangs.lieutenant_cut * base as f32).floor() as i64).min(pool);
            let (share, extra) = (c / lts.len() as i64, c % lts.len() as i64);
            for (i, &m) in lts.iter().enumerate() {
                let amount = share + i64::from((i as i64) < extra);
                if amount > 0 {
                    to_lts += ownership::pay(world, Some(gang), Some(m), amount, Flow::Tribute);
                }
            }
            pool -= to_lts;
        }
        if to_leader > 0 || to_lts > 0 {
            let row = ownership::jobs2_row(world);
            row.leader_cut_coins += to_leader;
            row.lieutenant_cut_coins += to_lts;
            let who = boss.map_or_else(|| "the leader".to_string(), |l| world.name_of(l));
            cut_note = format!("; off the top {to_leader} to {who}, {to_lts} to {} lieutenants", lts.len());
        }
    }
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
    // Pressure pass F2 (OPEN-118): an empty Collect (the pool spent between
    // the plan and the Hideout) closes the week without a line.
    if paid_total == 0 && cut_note.is_empty() {
        return 0;
    }
    let who = leader.map_or_else(|| "the leader".to_string(), |l| world.name_of(l));
    let text = format!(
        "{who} collected for {}: {paid_total} to {paid_members} members (the week's tribute and take {week}{cut_note})",
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
/// Jobs & lives phase 6: a cult's members preach too, under any order but a
/// raid (Shadow V4 bug 13: under Expand only, the Choir at LieLow, Squat or
/// Harvest never preached).
pub fn preacher(world: &World, id: EntityId) -> bool {
    let Some(gang) = world.gang_of(id) else { return false };
    let order = world.comp::<Gang>(gang).map(|g| g.order);
    let purist = crate::systems::creeds::is_purist(world, gang) && order == Some(crate::components::Order::Expand);
    let cult = crate::systems::faith::cult_preacher(world, gang) && order.is_some_and(|o| !o.is_raid());
    (purist || cult)
        && u64::from(id.index % 100) < (world.config.leisure.preach_share * 100.0) as u64
        && world.comp::<Brain>(id).is_none_or(|b| b.gang_task_day != Some(world.day()))
}

/// The busiest spot (most hanging out now, then the nearest to `from`, then
/// the lower tile) of the gang's held districts, else of its Hideout's
/// district. Pressure pass F2: the preacher's own district first when the
/// gang holds it, and ties by distance (with nobody hanging out anywhere the
/// lowest tile of every held district won: a preacher in D4 set out for a
/// corner of the map 2-3 h away, 17 plans and no Preach in a week).
fn busiest_spot(world: &World, id: EntityId, gang: EntityId, from: TilePos) -> Option<TilePos> {
    let mut ds: Vec<DistrictId> =
        crate::systems::gang::held_districts(world, gang).into_iter().map(|(d, _)| d).collect();
    let here = world.district_of(from);
    if ds.contains(&here) {
        ds = vec![here];
    }
    if ds.is_empty() {
        ds.push(world.district_of_building(world.hideout_of(gang)?));
    }
    // Pressure pass round 3, S1 (OPEN-221): a spot past `[distance.choice]
    // gang_work_ticks` of the preacher is no pulpit (V11: a cultist's five
    // sermons, each walk cut, no Preach).
    let cap = crate::systems::distance::choice(world).map(|c| c.gang_work_ticks);
    let rides = std::cell::OnceCell::new();
    let rides = || *rides.get_or_init(|| crate::systems::distance::rider(world, id));
    ds.iter()
        .filter_map(|d| world.spots.get(d.index()))
        .flat_map(|v| v.iter())
        .filter(|s| !matches!(s.kind, SpotKind::Hideout(g) if g != gang))
        .filter(|s| {
            cap.is_none_or(|c| crate::systems::distance::trip_by(world, from, s.tile, rides, Some(world.tick)) <= c)
        })
        .map(|s| (world.hangouts.get(&s.tile).map_or(0, |v| v.len()), s.tile.manhattan(from), s.tile))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)))
        .map(|(_, _, t)| t)
}

/// Plan L20: Preach's GangWork: `GoTo(Spot) → Preach` at the busiest spot.
pub fn preach_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !preacher(world, id) {
        return None;
    }
    let gang = world.gang_of(id)?;
    let from = world.comp::<Position>(id)?.tile;
    let tile = busiest_spot(world, id, gang, from)?;
    world.unwind.insert(
        id,
        UnwindPick {
            rung: Rung::Free,
            act: ActionKind::Preach,
            venue: None,
            spot: Some(tile),
            score: 0.0,
            stay_in: false,
        },
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
    // Jobs & lives phase 6: a cult's success converts the listener.
    crate::systems::faith::on_preached(world, gang, t);
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
