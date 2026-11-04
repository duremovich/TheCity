//! M18 P5h: housing as an errand (docs/M18_PLAYER.md § "P5h housing as an
//! errand", Dylan 2026-11-03: "let it be a task that can be completed early
//! ... the mechanism for NPCs too, to keep it consistent").
//!
//! A homeless adult's [`GoalKind::FindHome`] walks to a letting desk and
//! applies (`ActionKind::Apply`); on acceptance the applicant pays the
//! deposit (`[service] move_in_rent_days` of rent, everyone) and moves in,
//! with a homeless spouse and their children. `ownership::rehouse` (a
//! daily pass that housed anyone holding 21 days of rent) is retired.
//! Off camera a Statistical applicant rolls the same rules at midnight
//! ([`daily`]), with no walk. The world build's day-0 allocation stays.
//!
//! - **The desk** ([`desk_of`]): the Home's super (a tied resident) at the
//!   Block; else the owner's: the city's at the nearest Civic Hall, a corp's
//!   at its office ([`corp_desk`]: its Hq or most senior working building; a
//!   pure housing corp lets from the Block's lobby), a gang's at its
//!   Hideout, a private owner's at the Block.
//! - **The verdict** ([`judge`]), by the landlord's kind: the city keeps a
//!   list (`[housing] list_days`, then a place); a corp refuses a record, no
//!   job or no address history; a gang asks nothing, its protection rides on
//!   the rent (`ownership::rent_for` x `1 + gang_protection`) and the tenant
//!   is known to the gang (an edge with its leader); a private owner asks
//!   the deposit only. A refusal is remembered for `refusal_days`.
//! - **The capsule** (the first rung): a Capsule Hotel booking
//!   (`street::book`) is a capsule held: renewed at the 08:00 checkout while
//!   the night is paid ([`checkout`]; a missed night puts the guest out), with
//!   a nightly chance the locker is robbed ([`lockers`], 03:00). No deposit,
//!   no address. The world build opens a city rack per `hostel_residents`
//!   residents in the districts of `hostel_zones` ([`seed_hostels`]).
//! - **The player**: [`desk_places`] lists a desk's places for the menu
//!   (`player::Verb::Apply`), [`tenure_line`] is the HUD's place line.
//!   Safe-idle autopilot never runs FindHome (not a `safe_goal`) and never
//!   starts an Apply (`ActionKind::safe_idle_ok`); a held capsule's night is
//!   paid at checkout whoever holds the wheel.
//!
//! No child applies: children follow their parent. No coin made or lost:
//! the deposit is `Flow::Rent` to the owner, a robbed locker's coins go to a
//! fellow guest (else the owner).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{ActionInstance, GoalKind};
use crate::components::{
    Brain, Building, BuildingKind, Child, Household, Job, Lod, Position, Sentence, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{FailReason, StepResult};
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::ownership::{self, OwnerKind};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::world::World;

/// The locker roll's purpose key.
const PURPOSE_LOCKER: u64 = 0x5035_4801;
/// The thief's pick among the racks' guests.
const PURPOSE_THIEF: u64 = 0x5035_4802;

/// The errand's state (`World::housing`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HousingState {
    /// The city's list: applicant -> the tick his place is his.
    #[serde(default)]
    pub list: BTreeMap<EntityId, Tick>,
    /// (applicant, landlord) -> (the refusal's tick, its reason); the city's
    /// key is `EntityId::NONE`, every corp's one key `CORPS` (a corp's rule is
    /// every corp's).
    #[serde(default)]
    pub refused: BTreeMap<(EntityId, EntityId), (Tick, Refusal)>,
    /// Capsule holders: guest -> the Capsule Hotel (renewed at checkout while paid).
    #[serde(default)]
    pub capsules: BTreeMap<EntityId, EntityId>,
    /// The tick each body last lost a Home of its own (the address history a corp reads).
    #[serde(default)]
    pub left_home: BTreeMap<EntityId, Tick>,
    /// The cheapest deposit of a Home with room today (the goal's cheap gate; midnight).
    #[serde(default)]
    pub min_deposit: i64,
}

impl HousingState {
    /// A body gone (dead, emigrated): its rows go.
    pub fn forget(&mut self, id: EntityId) {
        self.list.remove(&id);
        self.capsules.remove(&id);
        self.left_home.remove(&id);
        self.refused.retain(|&(a, _), _| a != id);
    }
}

/// Why a landlord says no (or why the applicant does not ask).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The Home has no room for the household.
    NoRoom,
    /// The deposit is more than the applicant holds.
    Deposit(i64),
    /// This landlord evicted him (`[rent] refuse_days`).
    Evicted,
    /// On the city's list: days left.
    Listed(u64),
    /// A corp: a sentence served or an open warrant within `corp_record_days`.
    Record,
    /// A corp: no job.
    NoJob,
    /// A corp: no Home of his own within `corp_address_days`.
    NoAddress,
}

impl Refusal {
    /// The reason in the log's words.
    pub fn why(self) -> String {
        match self {
            Refusal::NoRoom => "no room".into(),
            Refusal::Deposit(d) => format!("the deposit is {d}c"),
            Refusal::Evicted => "they evicted you".into(),
            Refusal::Listed(d) => format!("on the city's list, a place in {d} days"),
            Refusal::Record => "a record".into(),
            Refusal::NoJob => "no job".into(),
            Refusal::NoAddress => "no address".into(),
        }
    }

    /// A landlord's rule, learned only at the desk (the applicant walks).
    fn at_desk(self) -> bool {
        matches!(self, Refusal::Record | Refusal::NoJob | Refusal::NoAddress)
    }
}

/// The desk's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// A place: the deposit paid, moved in.
    Accept,
    /// The city: the name goes on the list.
    Join,
    Refuse(Refusal),
}

/// The errand is on (`[housing] enabled`).
pub fn on(world: &World) -> bool {
    world.config.housing.enabled
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// The landlord's kind of building `b` (`ownership::owner_kind`, which
/// asserts on a dead owner: a dead private owner's Block is let as a
/// private owner's, the deposit only).
fn owner_kind_of(world: &World, b: EntityId) -> OwnerKind {
    match world.owner_of(b) {
        Some(o)
            if !(world.has::<crate::components::Corp>(o)
                || world.has::<crate::components::Gang>(o)
                || world.has::<Wallet>(o)) =>
        {
            OwnerKind::Agent(o)
        }
        owner => ownership::owner_kind(world, owner),
    }
}

/// The landlord's key in the refusal book (the city: `NONE`).
/// The corps' one key: a corp's refusal is a rule every corp keeps (a
/// record, a job, an address), so the applicant does not walk the round of
/// a dozen subsidiaries' desks the same afternoon (the shadow read: 14
/// refusals for one record in a day).
const CORPS: EntityId = EntityId { index: u32::MAX - 1, generation: u32::MAX };

fn landlord(world: &World, home: EntityId) -> EntityId {
    match owner_kind_of(world, home) {
        OwnerKind::Corp(_) => CORPS,
        _ => world.owner_of(home).unwrap_or(EntityId::NONE),
    }
}

fn door(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// A Home a homeless household may be let: standing, no company bed.
fn lettable(world: &World, h: EntityId) -> bool {
    standing(world, h)
        && world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Home)
        && crate::systems::company::open_to_all(world, h)
}

/// The deposit a mover pays at `home`: `[service] move_in_rent_days` of its rent.
pub fn deposit(world: &World, home: EntityId) -> i64 {
    i64::from(world.config.service.move_in_rent_days).max(1) * ownership::rent_for(world, home)
}

/// A corp's letting desk: its Hq, else its most senior working building
/// (an office before a counter), the lower id; `None`: a pure housing
/// corp (it lets from the Block's lobby).
pub fn corp_desk(world: &World, corp: EntityId) -> Option<EntityId> {
    const RANK: [BuildingKind; 8] = [
        BuildingKind::Hq,
        BuildingKind::SecurityOffice,
        BuildingKind::Lab,
        BuildingKind::Feed,
        BuildingKind::Clinic,
        BuildingKind::Hotel,
        BuildingKind::Bar,
        BuildingKind::Market,
    ];
    let c = world.comp::<crate::components::Corp>(corp)?;
    c.buildings
        .iter()
        .copied()
        .filter(|&b| standing(world, b))
        .filter_map(|b| {
            let k = world.comp::<Building>(b)?.kind;
            RANK.iter().position(|&r| r == k).map(|r| (r, b))
        })
        .min()
        .map(|(_, b)| b)
}

/// The Home's super: a resident tied to it (`ownership::tied_here`).
fn has_super(world: &World, home: EntityId) -> bool {
    world.residents_of(home).iter().any(|&r| ownership::tied_here(world, r, home))
}

/// Where one applies for `home` (see the module).
pub fn desk_of(world: &World, home: EntityId) -> Option<EntityId> {
    if has_super(world, home) {
        return Some(home);
    }
    match owner_kind_of(world, home) {
        OwnerKind::City => {
            let at = door(world, home)?;
            world
                .buildings_of_kind(BuildingKind::Hall)
                .iter()
                .copied()
                .filter(|&b| standing(world, b))
                .filter_map(|b| door(world, b).map(|d| (d.manhattan(at), b)))
                .min()
                .map(|(_, b)| b)
        }
        OwnerKind::Corp(c) => corp_desk(world, c).or(Some(home)),
        OwnerKind::Gang(g) => world.hideout_of(g).filter(|&h| standing(world, h)).or(Some(home)),
        OwnerKind::Agent(_) => Some(home),
    }
}

/// The desks' hours.
pub fn desk_hours(world: &World) -> bool {
    let c = &world.config.housing;
    let h = (world.tick_of_day() / TICKS_PER_HOUR as u16) as u8;
    h >= c.desk_open && h < c.desk_close
}

/// The applicant and who moves with him: a homeless spouse, their homeless children.
fn movers(world: &World, a: EntityId) -> (Option<EntityId>, Vec<EntityId>) {
    let spouse = world
        .spouse_of(a)
        .filter(|&s| world.has::<Brain>(s) && world.comp::<Household>(s).is_some_and(|h| h.home.is_none()));
    let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
        .into_iter()
        .filter(|&c| world.has::<Child>(c) && world.comp::<Household>(c).is_some_and(|h| h.home.is_none()))
        .collect();
    (spouse, kids)
}

/// A record a corp reads: a sentence served within `corp_record_days`, or an open warrant.
fn has_record(world: &World, a: EntityId) -> bool {
    if crate::systems::law::wanted(world, a) {
        return true;
    }
    let at = crate::systems::misery::record_at(world, a);
    at > 0 && world.tick < at + world.config.housing.corp_record_days * TICKS_PER_DAY
}

/// An address within `corp_address_days`: a Home of his own now or lately.
fn has_address(world: &World, a: EntityId) -> bool {
    if world.comp::<Household>(a).is_some_and(|h| h.home.is_some()) {
        return true;
    }
    let days = world.config.housing.corp_address_days * TICKS_PER_DAY;
    world.housing.left_home.get(&a).is_some_and(|&t| world.tick < t + days)
}

/// What the landlord of `home` says to `a` (who needs `need` beds) now.
pub fn judge(world: &World, a: EntityId, home: EntityId, need: usize) -> Verdict {
    let Some(b) = world.comp::<Building>(home) else { return Verdict::Refuse(Refusal::NoRoom) };
    if !lettable(world, home) || world.residents_of(home).len() + need > usize::from(b.capacity) {
        return Verdict::Refuse(Refusal::NoRoom);
    }
    let dep = deposit(world, home);
    if world.comp::<Wallet>(a).map_or(0, |w| w.coins) < dep {
        return Verdict::Refuse(Refusal::Deposit(dep));
    }
    let owner = world.owner_of(home);
    let now = world.tick;
    let refuse = world.config.rent.refuse_days * TICKS_PER_DAY;
    if world.comp::<Household>(a).and_then(|h| h.evicted_by).is_some_and(|(o, t)| o == owner && now < t + refuse) {
        return Verdict::Refuse(Refusal::Evicted);
    }
    match owner_kind_of(world, home) {
        OwnerKind::City => match world.housing.list.get(&a) {
            None => Verdict::Join,
            Some(&t) if now < t => Verdict::Refuse(Refusal::Listed((t - now).div_ceil(TICKS_PER_DAY))),
            Some(_) => Verdict::Accept,
        },
        OwnerKind::Corp(_) => {
            if has_record(world, a) {
                Verdict::Refuse(Refusal::Record)
            } else if !world.has::<Job>(a) {
                Verdict::Refuse(Refusal::NoJob)
            } else if !has_address(world, a) {
                Verdict::Refuse(Refusal::NoAddress)
            } else {
                Verdict::Accept
            }
        }
        OwnerKind::Gang(_) | OwnerKind::Agent(_) => Verdict::Accept,
    }
}

/// A refusal from this landlord still on the book (`refusal_days`).
pub fn refused_by(world: &World, a: EntityId, home: EntityId) -> Option<(Tick, Refusal)> {
    let days = world.config.housing.refusal_days * TICKS_PER_DAY;
    world.housing.refused.get(&(a, landlord(world, home))).copied().filter(|&(t, _)| world.tick < t + days)
}

/// May `a` apply for anything now: a free homeless adult staying in the city.
pub fn applicant(world: &World, a: EntityId) -> bool {
    world.comp::<Household>(a).is_some_and(|h| h.home.is_none())
        && crate::systems::demography::is_adult(world, a)
        && world.comp::<Brain>(a).is_some_and(|b| !b.emigrating && b.cuffed_by.is_none())
        && !world.has::<Sentence>(a)
        && crate::systems::law::living(world, a)
}

/// Where the applicant stands (outside his building's door).
fn origin(world: &World, a: EntityId) -> Option<TilePos> {
    let p = world.comp::<Position>(a)?;
    Some(match p.building.and_then(|b| world.comp::<Building>(b)) {
        Some(b) => world.outside_door(b),
        None => p.tile,
    })
}

/// The NPC's pick: the nearest Home it would ask about (room for the
/// household, the deposit in hand, no landlord's refusal on the book, the
/// city's list not waiting), with its desk in reach. What a landlord's
/// rules say (a record, no job, no address) is learned at the desk.
pub fn pick(world: &World, a: EntityId) -> Option<(EntityId, EntityId)> {
    let from = origin(world, a)?;
    let (spouse, kids) = movers(world, a);
    let need = 1 + usize::from(spouse.is_some()) + kids.len();
    let reach = world.config.housing.reach_tiles;
    let mut near: Vec<(u32, EntityId)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter_map(|h| door(world, h).map(|d| (d.manhattan(from), h)))
        .filter(|&(d, _)| d <= reach)
        .collect();
    near.sort_unstable();
    near.into_iter().map(|(_, h)| h).find_map(|h| {
        if refused_by(world, a, h).is_some() {
            return None;
        }
        match judge(world, a, h, need) {
            Verdict::Refuse(r) if !r.at_desk() => return None,
            _ => {}
        }
        let desk = desk_of(world, h)?;
        // T1 (`OPEN-304`): the desk by its trip (the walk or the El), not its
        // tiles: at 3.5 tiles a minute the Civic Hall's 400 tiles were a
        // 2-hour walk, and a desk past the cap is ridden to or passed over.
        let cap = world.config.housing.desk_reach_ticks;
        door(world, desk).filter(|&d| crate::systems::distance::trip(world, a, from, d) <= cap)?;
        Some((h, desk))
    })
}

/// The FindHome goal's considerations: an applicant in the desks' hours
/// holding the cheapest deposit, not on a shift; urgency the days homeless.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || !applicant(world, id) || !desk_hours(world) {
        return None;
    }
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    if coins < world.housing.min_deposit {
        return None;
    }
    let c = &world.config.housing;
    let days = world
        .comp::<Household>(id)
        .and_then(|h| h.homeless_since)
        .map_or(0.5, |t| (world.tick.saturating_sub(t)) as f32 / TICKS_PER_DAY as f32 / c.urgency_days.max(0.1));
    let on_shift = world.comp::<Job>(id).is_some_and(|j| crate::utility::goals::shift_reachable(world, id, j));
    let cs = vec![
        Consideration::new("homeless", can(true), GATE),
        Consideration::new("days homeless", days.min(1.0), Curve::Linear { m: c.curve.0, b: c.curve.1 }),
        Consideration::new("not on shift", can(!on_shift), GATE),
    ];
    Some((cs, c.flat))
}

/// The FindHome goal's scripted plan: `GoTo(desk) -> Apply(home)`; `None`
/// (nothing to ask about) cools the goal.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let (home, desk) = pick(world, id)?;
    Some(plan_for(world, id, home, desk))
}

/// The walk to `desk` and the Apply for `home` (the NPC's and the player's).
pub fn plan_for(world: &World, id: EntityId, home: EntityId, desk: EntityId) -> Plan {
    let step = |action, target| ActionInstance { action, target, tile: None };
    let mut steps = Vec::new();
    if world.comp::<Position>(id).and_then(|p| p.building) != Some(desk) {
        steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(desk)));
    }
    steps.push(step(ActionKind::Apply, Some(home)));
    Plan { goal: GoalKind::FindHome, target: Some(desk), steps, started_tick: world.tick }
}

/// The Apply's start: inside the desk for `home`, in the desks' hours.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    match kind {
        ActionKind::Apply => target.is_some_and(|h| {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            here.is_some() && here == desk_of(world, h) && desk_hours(world)
        }),
        _ => crate::exec::actions::can_start(world, id, kind, None),
    }
}

/// `Apply` completes at the desk: the verdict, written.
pub fn on_apply(world: &mut World, id: EntityId, home: Option<EntityId>) -> StepResult {
    let Some(home) = home else { return StepResult::Failed(FailReason::NoSuchPlace) };
    if !applicant(world, id) {
        return StepResult::Done;
    }
    let desk = world.comp::<Position>(id).and_then(|p| p.building);
    apply(world, id, home, desk);
    StepResult::Done
}

/// The application decided (a body at the desk, or a Statistical one's roll
/// with `desk` `None`): the move-in, the list, or the refusal on the book.
pub fn apply(world: &mut World, a: EntityId, home: EntityId, desk: Option<EntityId>) -> Verdict {
    let (spouse, kids) = movers(world, a);
    let need = 1 + usize::from(spouse.is_some()) + kids.len();
    let verdict = judge(world, a, home, need);
    let name = world.name_of(a);
    let place = world.name_of(home);
    let label = world.owner_label(world.owner_of(home));
    // a Block that lets itself (a super, a private owner, a housing corp's lobby) is named once
    let at = desk.filter(|&d| d != home).map_or_else(String::new, |d| format!(" at {}", world.name_of(d)));
    let mut actors = vec![a, home];
    actors.extend(desk.filter(|&d| d != home));
    match verdict {
        Verdict::Accept => move_in(world, a, home, spouse, kids),
        Verdict::Join => {
            let days = world.config.housing.list_days;
            world.housing.list.insert(a, world.tick + days * TICKS_PER_DAY);
            world.push_event(
                EventKind::Listed,
                &actors,
                format!("{name} put their name on the city's list{at}: a place in {days} days"),
            );
        }
        Verdict::Refuse(r) => {
            // A landlord's rule goes on the book (not asked again for
            // `refusal_days`); a place gone meanwhile, a deposit short, is not.
            if r.at_desk() {
                world.housing.refused.insert((a, landlord(world, home)), (world.tick, r));
            }
            world.push_event(
                EventKind::HomeRefused,
                &actors,
                format!("{name} asked for a place at {place}{at} and was refused by {label}: {}", r.why()),
            );
        }
    }
    verdict
}

/// Accepted: the deposit paid, the household moved in (the spouse with
/// him, the children to the door), `Housed`; a gang's tenant is known to it.
fn move_in(world: &mut World, a: EntityId, home: EntityId, spouse: Option<EntityId>, kids: Vec<EntityId>) {
    let days = i64::from(world.config.service.move_in_rent_days).max(1);
    let dep = deposit(world, home);
    ownership::move_in_deposit(world, a, home, days);
    world.housing.list.remove(&a);
    let place = world.name_of(home);
    let owner = world.owner_of(home);
    let label = world.owner_label(owner);
    let gang = match owner_kind_of(world, home) {
        OwnerKind::Gang(g) => Some(g),
        _ => None,
    };
    let rent = ownership::rent_for(world, home);
    let terms = match gang {
        Some(_) => format!("{dep}c down, {rent}c a day with the protection on it"),
        None => format!("{dep}c down, {rent}c a day"),
    };
    let tick = world.tick;
    for m in std::iter::once(a).chain(spouse) {
        world.set_home(m, Some(home));
        let name = world.name_of(m);
        world.push_event(EventKind::Housed, &[m, home], format!("{name} moved into {place} ({label}; {terms})"));
        world.stats.current.housed += 1;
    }
    // A gang's tenant is known to it: an edge with its leader.
    if let Some(leader) = gang.and_then(|g| world.comp::<crate::components::Gang>(g)).and_then(|g| g.leader) {
        if leader != a && world.has::<Brain>(leader) {
            crate::systems::social::interacted(world, a, leader);
        }
    }
    let Some(d) = door(world, home) else { return };
    for c in kids {
        world.set_home(c, Some(home));
        if let Some(p) = world.comp_mut::<Position>(c) {
            p.tile = d;
            p.building = Some(home);
            p.entered = tick;
        }
        crate::systems::floors::settle(world, c);
    }
}

/// Midnight (`ownership::run`, where `rehouse` ran): the cheapest deposit,
/// the books pruned, and the Statistical applicants' rolls (the same pick
/// and verdict, no walk).
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let now = world.tick;
    let c = world.config.housing.clone();
    world.housing.min_deposit = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| lettable(world, h))
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| world.residents_of(h).len() < usize::from(b.capacity)))
        .map(|h| deposit(world, h))
        .min()
        .unwrap_or(0);
    let keep = c.refusal_days.max(1) * TICKS_PER_DAY;
    world.housing.refused.retain(|_, &mut (t, _)| now < t + keep);
    let addr = c.corp_address_days * TICKS_PER_DAY;
    world.housing.left_home.retain(|_, &mut t| now < t + addr);
    let gone: Vec<EntityId> = world
        .housing
        .list
        .keys()
        .copied()
        .filter(|&a| !world.comp::<Household>(a).is_some_and(|h| h.home.is_none()))
        .collect();
    for a in gone {
        world.housing.list.remove(&a);
    }
    let min = world.housing.min_deposit;
    let stat: Vec<EntityId> = world
        // scan-ok: daily: the Statistical applicants (the rehouse pass's scan)
        .citizens()
        .into_iter()
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical && !b.possessed()))
        .filter(|&a| applicant(world, a))
        .filter(|&a| world.comp::<Wallet>(a).map_or(0, |w| w.coins) >= min)
        .collect();
    for a in stat {
        if !applicant(world, a) {
            continue; // housed as someone's spouse earlier in this pass
        }
        if let Some((home, desk)) = pick(world, a) {
            apply(world, a, home, Some(desk));
        }
    }
}

/// The capsule's nightly pay at the 08:00 checkout (`living::run` hourly):
/// each holder still homeless and booked there pays tonight's capsule
/// (`street::hotel_price`, `Flow::Hotel`) and keeps it to the next 08:00,
/// or is put out of it (`CapsuleLost`).
pub fn checkout(world: &mut World) {
    if !on(world) || world.housing.capsules.is_empty() {
        return;
    }
    let now = world.tick;
    if world.tick_of_day() != (crate::systems::street::CHECKOUT_HOUR * TICKS_PER_HOUR) as u16 {
        return;
    }
    let held: Vec<(EntityId, EntityId)> = world.housing.capsules.iter().map(|(&a, &h)| (a, h)).collect();
    for (a, h) in held {
        let booked = world.hotel_beds.get(&a).is_some_and(|&(x, _)| x == h);
        let homeless = world.comp::<Household>(a).is_some_and(|x| x.home.is_none());
        if !booked || !homeless || !crate::systems::law::living(world, a) || world.has::<Sentence>(a) {
            world.housing.capsules.remove(&a);
            continue;
        }
        let price = crate::systems::street::hotel_price(world, h);
        let coins = world.comp::<Wallet>(a).map_or(0, |w| w.coins);
        if !crate::systems::street::is_hotel(world, h) || coins < price {
            world.housing.capsules.remove(&a);
            world.hotel_beds.remove(&a);
            let (name, place) = (world.name_of(a), world.name_of(h));
            let why = if coins < price {
                format!("the night unpaid ({price}c, {coins}c in hand)")
            } else {
                "the racks shut".into()
            };
            world.push_event(EventKind::CapsuleLost, &[a, h], format!("{name} lost their capsule at {place}: {why}"));
            continue;
        }
        let owner = world.owner_of(h);
        let paid = ownership::pay(world, Some(a), owner, price, ownership::Flow::Hotel);
        ownership::credit(world, h, paid);
        world.hotel_beds.insert(a, (h, crate::systems::street::checkout_after(now)));
        world.stats.current.hotel_nights += 1;
    }
}

/// 03:00 (`street::nightly`): each capsule holder's locker rolls
/// `locker_rob_p`; robbed, `locker_rob_share` of his coins go to a fellow
/// guest of the racks (hashed), else to the owner (`LockerRobbed`).
pub fn lockers(world: &mut World) {
    if !on(world) || world.housing.capsules.is_empty() {
        return;
    }
    let c = world.config.housing.clone();
    let day = world.day();
    let held: Vec<(EntityId, EntityId)> = world.housing.capsules.iter().map(|(&a, &h)| (a, h)).collect();
    for &(a, h) in &held {
        if !world.hotel_beds.get(&a).is_some_and(|&(x, until)| x == h && until > world.tick) {
            continue;
        }
        if crate::systems::econ::hash_unit(world.seed(), PURPOSE_LOCKER, key(a), day) >= c.locker_rob_p {
            continue;
        }
        let have = world.comp::<Wallet>(a).map_or(0, |w| w.coins.max(0));
        let n = (have as f32 * c.locker_rob_share).floor() as i64;
        if n <= 0 {
            continue;
        }
        let guests: Vec<EntityId> =
            held.iter().filter(|&&(g, x)| x == h && g != a && world.has::<Wallet>(g)).map(|&(g, _)| g).collect();
        let thief = (!guests.is_empty()).then(|| {
            let r = crate::systems::econ::hash_unit(world.seed(), PURPOSE_THIEF, key(a), day);
            guests[((r * guests.len() as f32) as usize).min(guests.len() - 1)]
        });
        if let Some(w) = world.comp_mut::<Wallet>(a) {
            w.coins -= n;
        }
        match thief {
            Some(t) => {
                if let Some(w) = world.comp_mut::<Wallet>(t) {
                    w.coins += n;
                }
            }
            None => {
                let owner = world.owner_of(h);
                // the coins go back through the owner's purse (no coin made or lost)
                if let Some(w) = world.comp_mut::<Wallet>(a) {
                    w.coins += n;
                }
                ownership::pay(world, Some(a), owner, n, ownership::Flow::Hotel);
            }
        }
        world.remember(a, crate::components::MemoryKind::WasRobbed, None, 0.4, -0.5, false);
        let (name, place) = (world.name_of(a), world.name_of(h));
        let mut actors = vec![a, h];
        actors.extend(thief);
        world.push_event(
            EventKind::LockerRobbed,
            &actors,
            format!("{name}'s locker at {place} was robbed in the night: {n}c gone"),
        );
    }
}

/// The world build (after `street::seed_hotels`): a city Capsule Hotel
/// per `hostel_residents` residents (floored; none in a city smaller than
/// that), dealt round the districts of `hostel_zones` in id order, each on
/// the vacant lot nearest its district's centre.
pub fn seed_hostels(world: &mut World) {
    let c = world.config.housing.clone();
    if !on(world) || !crate::systems::street::enabled(world) || c.hostel_residents == 0 {
        return;
    }
    let mut left = world.citizens().len() / c.hostel_residents;
    let districts: Vec<(crate::components::DistrictId, TilePos)> =
        world.districts.iter().filter(|d| c.hostel_zones.contains(&d.zone)).map(|d| (d.id, d.centroid)).collect();
    if districts.is_empty() {
        return;
    }
    let mut stuck = 0;
    for (d, centroid) in districts.iter().copied().cycle() {
        if left == 0 || stuck >= districts.len() {
            break;
        }
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| world.district_of_building(l) == d)
            .filter_map(|l| door(world, l).map(|x| (x.manhattan(centroid), l)))
            .min()
            .map(|(_, l)| l);
        let built =
            lot.is_some_and(|l| crate::systems::founding::build_on_lot(world, l, BuildingKind::Hotel, None).is_ok());
        if !built {
            stuck += 1;
            continue;
        }
        stuck = 0;
        left -= 1;
        let lot = lot.unwrap_or(EntityId::NONE);
        let what = world.name_of(lot);
        world.push_event(EventKind::Founded, &[lot], format!("the city opened a capsule hostel ({what})"));
    }
}

/// The places a desk lets, for the player's menu: the lettable Homes whose
/// desk is `desk`, nearest the body first, at most `menu_rows` (the
/// city's: one row while the list is to join).
pub fn desk_places(world: &World, id: EntityId, desk: EntityId) -> Vec<EntityId> {
    if !on(world) || !standing(world, desk) {
        return Vec::new();
    }
    let Some(kind) = world.comp::<Building>(desk).map(|b| b.kind) else { return Vec::new() };
    let pool: Vec<EntityId> = match kind {
        BuildingKind::Hall => world
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .filter(|&h| world.owner_of(h).is_none() && !has_super(world, h))
            .collect(),
        BuildingKind::Home => vec![desk],
        _ => match owner_kind_of(world, desk) {
            OwnerKind::Corp(c) if corp_desk(world, c) == Some(desk) => world
                .comp::<crate::components::Corp>(c)
                .map(|cc| cc.buildings.iter().copied().filter(|&h| !has_super(world, h)).collect())
                .unwrap_or_default(),
            OwnerKind::Gang(g) if world.hideout_of(g) == Some(desk) => world
                .buildings_of_kind(BuildingKind::Home)
                .iter()
                .copied()
                .filter(|&h| world.owner_of(h) == Some(g) && !has_super(world, h))
                .collect(),
            _ => Vec::new(),
        },
    };
    let from = origin(world, id).unwrap_or_default();
    let (spouse, kids) = movers(world, id);
    let need = 1 + usize::from(spouse.is_some()) + kids.len();
    let mut near: Vec<(u32, EntityId)> = pool
        .into_iter()
        .filter(|&h| lettable(world, h) && desk_of(world, h) == Some(desk))
        .filter(|&h| {
            world.comp::<Building>(h).is_some_and(|b| world.residents_of(h).len() + need <= usize::from(b.capacity))
        })
        .filter_map(|h| door(world, h).map(|d| (d.manhattan(from), h)))
        .collect();
    near.sort_unstable();
    let mut out: Vec<EntityId> = near.into_iter().map(|(_, h)| h).collect();
    // The city's list is joined once: one row until the place is due.
    if kind == BuildingKind::Hall && world.housing.list.get(&id).is_none_or(|&t| world.tick < t) {
        out.truncate(1);
    }
    out.truncate(world.config.housing.menu_rows.max(1));
    out
}

/// The player's row for `home` (`player::Verb::Apply`): its label, or why not.
pub fn row(world: &World, id: EntityId, home: EntityId) -> Result<String, String> {
    if !applicant(world, id) {
        return Err("you have a home".into());
    }
    let (spouse, kids) = movers(world, id);
    let need = 1 + usize::from(spouse.is_some()) + kids.len();
    let place = world.name_of(home);
    let rent = ownership::rent_for(world, home);
    let dep = deposit(world, home);
    let what = if world.comp::<Building>(home).is_some_and(|b| b.tier == 0) { "a room" } else { "a flat" };
    if let Some((t, r)) = refused_by(world, id, home) {
        let ago = (world.tick.saturating_sub(t)) / TICKS_PER_DAY;
        let when = if ago == 0 { "today".to_string() } else { format!("{ago} days ago") };
        return Err(format!("{what} at {place}: refused {when} ({})", r.why()));
    }
    match judge(world, id, home, need) {
        Verdict::Join => Ok(format!(
            "Put your name on the city's list (a place in {} days, then {dep}c down, {rent}c a day)",
            world.config.housing.list_days
        )),
        Verdict::Accept => Ok(format!("Take {what} at {place} ({dep}c down, {rent}c a day)")),
        Verdict::Refuse(r) if r.at_desk() => {
            let asks = match owner_kind_of(world, home) {
                OwnerKind::Corp(_) => "; they ask for a job, no record and an address",
                _ => "",
            };
            Ok(format!("Apply for {what} at {place} ({dep}c down, {rent}c a day{asks})"))
        }
        Verdict::Refuse(Refusal::Listed(d)) => Err(format!("On the city's list: a place in {d} days")),
        Verdict::Refuse(r) => Err(format!("{what} at {place}: {}", r.why())),
    }
}

/// The HUD's place line: the body's tenure.
pub fn tenure_line(world: &World, id: EntityId) -> String {
    let list = world.housing.list.get(&id).map(|&t| {
        if world.tick < t {
            format!("; on the city's list, {} days to go", (t - world.tick).div_ceil(TICKS_PER_DAY))
        } else {
            "; the city's list: your place is due at the Hall".to_string()
        }
    });
    let list = list.unwrap_or_default();
    if let Some(h) = world.comp::<Household>(id).and_then(|h| h.home) {
        let place = world.name_of(h);
        let label = world.owner_label(world.owner_of(h));
        let rent = ownership::rent_for(world, h);
        let what = if world.comp::<Building>(h).is_some_and(|b| b.tier == 0) { "a room" } else { "a flat" };
        return format!("lives: {what} at {place} ({label}, {rent}c a day)");
    }
    if let Some(&h) = world.housing.capsules.get(&id) {
        let price = crate::systems::street::hotel_price(world, h);
        return format!("lives: a capsule at {} ({price}c a night, paid at 08:00){list}", world.name_of(h));
    }
    if let Some(h) = crate::systems::street::booked_hotel(world, id) {
        return format!("lives: a cot at {} tonight{list}", world.name_of(h));
    }
    if let Some(s) = world.comp::<crate::components::Squatter>(id) {
        return format!("lives: squats at {}{list}", world.name_of(s.building));
    }
    format!("lives: sleeps rough{list}")
}

/// A play script's target (`bld near hostel|corp|city`): the nearest
/// Capsule Hotel with a free bed, the nearest corp letting desk with a
/// place, the nearest Civic Hall.
pub fn script_target(world: &World, id: EntityId, what: &str) -> Option<EntityId> {
    let from = origin(world, id)?;
    let nearest = |bs: Vec<EntityId>| {
        bs.into_iter().filter_map(|b| door(world, b).map(|d| (d.manhattan(from), b))).min().map(|(_, b)| b)
    };
    match what {
        "hostel" => nearest(
            world
                .buildings_of_kind(BuildingKind::Hotel)
                .iter()
                .copied()
                .filter(|&h| {
                    crate::systems::street::is_hotel(world, h) && crate::systems::street::free_beds(world, h) > 0
                })
                .collect(),
        ),
        "city" => nearest(
            world.buildings_of_kind(BuildingKind::Hall).iter().copied().filter(|&b| standing(world, b)).collect(),
        ),
        "corp" => nearest(
            world
                .corps()
                .into_iter()
                .filter_map(|c| corp_desk(world, c))
                .filter(|&d| !desk_places(world, id, d).is_empty())
                .collect(),
        ),
        _ => None,
    }
}
