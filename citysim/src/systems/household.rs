//! Lives at work phase 2 (docs/LIVES_AT_WORK.md § 3): the evening and the
//! household. Under `[household] enabled` (under the `[lives]` master;
//! `[household] enabled = false`): off, every entry point is a no-op, no spot
//! is added, no state is written and the save is byte-identical to main.
//!
//! - **Home as a spot** (§ 3.1, § 3.2; `leisure::spots_near`): the agent's
//!   own Home and up to `visit_hosts` Friend/Family contacts' Homes whose
//!   host is due home are HangOut spots; `best_spot` adds `home_w` per
//!   household member home now and `child_w` per child at home
//!   ([`home_term`]); the HangOut runs inside and its partner is the
//!   resident present of highest affinity ([`home_partner`]).
//! - **Leaving an empty spot** (§ 3.3): a HangOut alone after
//!   `empty_leave_ticks` ends early ([`empty_leave`]); the tile goes into
//!   `Brain.empty_spots` and scores × `empty_penalty` for
//!   `empty_memory_hours` ([`empty_mult`]).
//! - **The household** (§ 3.4): [`reunite`] at midnight (before
//!   `ownership`'s re-housing) moves an unhoused spouse in where there is
//!   room; [`on_birth`] sets `Household.newborn_until` on both parents and
//!   picks the carer, whose away spots and night goals score ×
//!   `newborn_away_mult` ([`damped`], [`away_mult`]) while both parents'
//!   energy decays × `newborn_energy_mult` ([`energy_mult`]) and the Home
//!   feeds the newborn through the pantry ([`daily`]); the household's
//!   business (a building the agent or the spouse owns) is offered to its jobless adults first
//!   ([`owner_operator`]), its heir waits for it ([`business_holders`])
//!   and its members are laid off last ([`is_member_of_owner`]).
//! - **Visits** (§ 3.5): [`visits_due`] at midnight posts at most one
//!   `GoalKind::Visit` a day an agent (a kin or spouse downed, assaulted or
//!   arrested in the last day; an elder parent not met for
//!   `elder_visit_days`), `World::visits`; a Full visitor walks the scripted
//!   plan `GoTo(the kin's building) → Rest` ([`plan`]) and is credited at
//!   its end ([`visit_done`]); a Statistical one is credited at the visit's
//!   hour ([`hourly`]). The Camp answer is phase 3's.
//! - **Deeds** (§ 3.6): `Deed::Employed`, `Deed::LaidOff` and `Deed::Born`
//!   posted where the events are (`demography::hire`, `economy::dismiss_as`,
//!   [`on_birth`]); `Deed::Married` already existed.
//!
//! The Statistical mirror (§ 6): reunite, the newborn flags and the visits
//! are midnight and hourly rules for every tier; the home evening moves the
//! Full tier's chat and social rates, learned at the gate's one regen (D14).
//! No world-stream draw: the carer's tie and the visit's hour are hashed.

use serde::{Deserialize, Serialize};

use crate::components::{
    ActionInstance, Brain, Building, BuildingKind, Child, Corpse, Crime, GoalKind, Household, Identity, Job, Lod,
    MemoryKind, Personality, Position, RelKind, Routine, Sentence, ServeKind, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, DAYS_PER_YEAR, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::word::Deed;
use crate::world::World;

/// The carer's tie (both parents employed at the same wage).
const PURPOSE_CARER: u64 = 0x4855_0001;
/// The visit's hour of the day.
const PURPOSE_VISIT_HOUR: u64 = 0x4855_0002;
/// Lives at work phase 3 (§ 3.5): the day of the week a parent answers for
/// a Camp child, on (parent, week).
const PURPOSE_ANSWER: u64 = 0x4855_0003;

/// Lives at work phase 3 (§ 3.5): a parent's answer to a taken child, chosen
/// at midnight from the household's state (`answer_of`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampAnswer {
    /// The weekly visit: an encounter with the warden, a `last_met`.
    Visit,
    /// The fee to the Camp's owner (`Flow::Buyout`) and the child home.
    BuyOut,
    /// A contest at the Camp door against the wardens present.
    BreakOut,
}

/// Why a visit is due (`World::visits`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisitWhy {
    /// A kin or spouse downed, assaulted or arrested in the last day.
    Hurt,
    /// An elder parent not met for `elder_visit_days`.
    Elder,
    /// Lives at work phase 3 (§ 3.5): a child in a Camp: the parent's answer.
    Camp(CampAnswer),
    /// Lives at work phase 5 (§ 2.4): a walk-in paper buyer to the Fixer
    /// (`kin` is the Fixer building; `service::walk_ins_daily`).
    Papers,
    /// Lives at work phase 5 (§ 2.4, § 3.4's owner-operator): a Fixer's owner
    /// without a post collects the week's cut in person (`kin` is the Fixer).
    Collect,
    /// The pressure pass phase 6 (`underwater::confronted`): a worker owed
    /// wages goes to the owner (`kin`) in person to demand them.
    Confront,
    /// Phase 6 (`underwater::dunned`): a loan's collector calls on the
    /// borrower (`kin`) behind on his installments.
    Dun,
    /// The pressure pass phase 5a (`consequence::blackmailed`): a witness
    /// goes to the culprit (`kin`) in person to name the price of silence.
    Blackmail,
    /// Phase 5a (`consequence::attended`): a creed's service at its place
    /// (`kin` is the chapel or the church).
    Service,
}

impl VisitWhy {
    /// Phase 5's visits go to a building, not to a person.
    pub fn to_building(self) -> bool {
        matches!(self, VisitWhy::Papers | VisitWhy::Collect | VisitWhy::Service)
    }
}

/// A visit due today (`World::visits`, saved while pending; cleared at midnight).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingVisit {
    /// Who is visited (the walk goes to its building).
    pub kin: EntityId,
    pub why: VisitWhy,
    /// The hour of the day the visit is made (a Statistical visitor is
    /// credited then; a Full one scores the goal from then).
    pub hour: u8,
}

/// Phase 2 is live: `[jobs2] enabled`, `[lives] enabled` and `[household] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.lives.enabled && world.config.household.enabled
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

fn home_of(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Household>(id).and_then(|h| h.home)
}

fn is_home(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Home)
}

/// Bodies of `home`'s household inside it now, `except` left out.
pub fn residents_inside(world: &World, home: EntityId, except: EntityId) -> Vec<EntityId> {
    inside(world, home, except).collect()
}

/// Whether any body of `home`'s household but `except` is inside it now
/// (the per-tick question of `empty_leave`, no Vec).
pub fn any_inside(world: &World, home: EntityId, except: EntityId) -> bool {
    inside(world, home, except).next().is_some()
}

fn inside(world: &World, home: EntityId, except: EntityId) -> impl Iterator<Item = EntityId> + '_ {
    world
        .residents_of(home)
        .iter()
        .copied()
        .filter(move |&r| r != except && world.comp::<Position>(r).is_some_and(|p| p.building == Some(home)))
}

// ---------------------------------------------------------------------------
// § 3.1, § 3.2: the home spot
// ---------------------------------------------------------------------------

/// The Homes `id` may hang out in: its own, when there is company to be had
/// there (`company_due`), at any distance; and (`with_friends`) up to
/// `visit_hosts` Friend/Family (never Rival/Enemy) contacts' Homes whose
/// host's habit at `now + 60` is that Home, within `cap` tiles of `from`.
/// Returns `(home, is_own)`. The friends' read the contacts' habits (14 days
/// of Trace each), so only the scored pick asks for them. Nothing off.
pub fn home_spots(world: &World, id: EntityId, from: TilePos, cap: u32, with_friends: bool) -> Vec<(EntityId, bool)> {
    if !on(world) {
        return Vec::new();
    }
    let mut out: Vec<(EntityId, bool)> = Vec::new();
    let door = |h: EntityId| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| world.outside_door(b));
    // The own Home at any distance (the travel term weighs the walk; the
    // agent goes home in the end), only with company to be had: a single
    // resident with nobody due keeps the corner and the bar.
    if let Some(h) = home_of(world, id) {
        if door(h).is_some() && company_due(world, id, h) {
            out.push((h, true));
        }
    }
    let max = usize::from(world.config.household.visit_hosts);
    if max == 0 || !with_friends {
        return out;
    }
    let now = world.tick;
    let mut hosts: Vec<(f32, EntityId, EntityId)> = world
        .neighbours(id)
        .filter(|&c| world.has::<Brain>(c) && !world.has::<Sentence>(c))
        .filter_map(|c| {
            let e = world.edge(id, c)?;
            if !matches!(e.kind, RelKind::Friend | RelKind::Family) || e.affinity <= 0.0 {
                return None;
            }
            let h = home_of(world, c)?;
            if out.iter().any(|&(o, _)| o == h) {
                return None;
            }
            let due_home = crate::systems::hunt::habit(world, c, now + TICKS_PER_HOUR).building == Some(h);
            (due_home && door(h).is_some_and(|d| d.manhattan(from) <= cap)).then_some((e.affinity, c, h))
        })
        .collect();
    hosts.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, _, h) in hosts.into_iter().take(max) {
        if !out.iter().any(|&(o, _)| o == h) {
            out.push((h, false));
        }
    }
    out
}

/// § 3.1: whether `home` holds company for `id` now or within the hour: a
/// household body inside or a child at home (`home_term` > 0), or another
/// resident with a moving body (Full or Coarse) whose habit at `now + 60` is
/// the Home (the spouse due back). A Statistical resident is left out of the
/// "due" clause: its `Position` is a demotion snapshot, so a habit that says
/// "home at 20:00" never puts a body inside (measured: 1,700 of 1,750 adults
/// "due" at 19:00 on day 19, 35 with a body inside an hour later). Whether a
/// Statistical spouse counts as home by the clock is the mirror question for
/// the gate. The same rule the friends' Homes follow; the own Home is never
/// offered empty.
pub fn company_due(world: &World, id: EntityId, home: EntityId) -> bool {
    if home_term(world, id, home) > 0.0 {
        return true;
    }
    let at = world.tick + TICKS_PER_HOUR;
    world.residents_of(home).iter().any(|&r| {
        r != id
            && world.comp::<Brain>(r).is_some_and(|b| b.lod != Lod::Statistical)
            && crate::systems::hunt::habit(world, r, at).building == Some(home)
    })
}

/// § 3.1's household term for a Home spot: `home_w` per household member
/// home now (bodies inside, the agent aside) plus `child_w` per child at
/// home (routine not `Camp`/`Street`).
pub fn home_term(world: &World, id: EntityId, home: EntityId) -> f32 {
    let cfg = &world.config.household;
    let mut adults = 0u32;
    let mut children = 0u32;
    for r in residents_inside(world, home, id) {
        match world.comp::<Child>(r) {
            Some(c) => children += u32::from(!matches!(c.routine, Routine::Camp | Routine::Street)),
            None => adults += 1,
        }
    }
    cfg.home_w * adults as f32 + cfg.child_w * children as f32
}

/// § 3.1: the HangOut partner inside `home`: the resident present of
/// highest affinity (ties the lower id), bodies only.
pub fn home_partner(world: &World, id: EntityId, home: EntityId) -> Option<EntityId> {
    residents_inside(world, home, id)
        .into_iter()
        .filter(|&r| world.has::<Brain>(r))
        .map(|r| (world.edge(id, r).map_or(f32::NEG_INFINITY, |e| e.affinity), r))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, r)| r)
}

/// Is `id`'s running HangOut inside a Home (`UnwindPick.venue` the home)?
pub fn hanging_at_home(world: &World, id: EntityId) -> Option<EntityId> {
    let here = world.comp::<Position>(id).and_then(|p| p.building)?;
    let venue = world.unwind.get(&id).and_then(|p| p.venue)?;
    (venue == here && is_home(world, here)).then_some(here)
}

// ---------------------------------------------------------------------------
// § 3.3: leaving an empty spot
// ---------------------------------------------------------------------------

/// § 3.3: a HangOut `started` at `started` with nobody else there after
/// `empty_leave_ticks` ends early (the caller returns `Done`). Nothing off.
pub fn empty_leave(world: &World, id: EntityId, started: Tick) -> bool {
    if !on(world) || world.tick.saturating_sub(started) < Tick::from(world.config.household.empty_leave_ticks) {
        return false;
    }
    // Island fix round F3 (OPEN-190): the stay-in runs alone by design.
    if world.unwind.get(&id).is_some_and(|p| p.stay_in) {
        return false;
    }
    match hanging_at_home(world, id) {
        Some(h) => !any_inside(world, h, id),
        None => world
            .comp::<Position>(id)
            .and_then(|p| world.hangouts.get(&p.tile))
            .is_none_or(|v| !v.iter().any(|&o| o != id)),
    }
}

/// § 3.3: `id` left `tile` empty now: remembered for `empty_memory_hours`
/// (the oldest of four falls off); counted.
pub fn remember_empty(world: &mut World, id: EntityId, tile: TilePos) {
    if !on(world) {
        return;
    }
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        if b.empty_spots.len() >= b.empty_spots.inline_size() {
            b.empty_spots.remove(0);
        }
        b.empty_spots.push((tile, now));
    }
    ownership::jobs2_row(world).empty_leaves += 1;
}

/// § 3.3: having left a spot empty within `empty_memory_hours`, `id` goes
/// only where there is company now (a body at the spot, a resident inside
/// the Home); the first build re-planned a lonely corner at once and the
/// city's HangOuts doubled with three in four left empty (the lives probe
/// read 0.27 -> 0.73): nobody waited, so nobody met.
pub fn company_only(world: &World, id: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let window = Tick::from(world.config.household.empty_memory_hours) * TICKS_PER_HOUR;
    let now = world.tick;
    world.comp::<Brain>(id).is_some_and(|b| b.empty_spots.iter().any(|&(_, at)| now.saturating_sub(at) < window))
}

/// § 3.3: is there company at `tile` now (a body hanging out there), or
/// inside `home` (a resident)?
pub fn company_at(world: &World, id: EntityId, tile: TilePos, home: Option<EntityId>) -> bool {
    match home {
        Some(h) => any_inside(world, h, id),
        None => world.hangouts.get(&tile).is_some_and(|v| v.iter().any(|&o| o != id)),
    }
}

/// § 3.3: the factor a remembered empty spot's score takes (1 otherwise).
pub fn empty_mult(world: &World, id: EntityId, tile: TilePos) -> f32 {
    if !on(world) {
        return 1.0;
    }
    let cfg = &world.config.household;
    let window = Tick::from(cfg.empty_memory_hours) * TICKS_PER_HOUR;
    let now = world.tick;
    let remembered = world
        .comp::<Brain>(id)
        .is_some_and(|b| b.empty_spots.iter().any(|&(t, at)| t == tile && now.saturating_sub(at) < window));
    if remembered {
        cfg.empty_penalty
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// § 3.4: the household
// ---------------------------------------------------------------------------

/// Midnight, before `ownership`'s re-housing: a married pair living apart
/// (one housed, the other with no Home) moves the unhoused spouse into the
/// housed one's Home when it has room; with no room the pair waits for
/// `rehouse` (deviation: no queue exists; `rehouse` already moves a
/// homeless pair together). Nothing off.
pub fn reunite(world: &mut World) {
    if !on(world) {
        return;
    }
    let pairs: Vec<(EntityId, EntityId)> = world.spouses.iter().filter(|(a, b)| a < b).map(|(&a, &b)| (a, b)).collect();
    for (a, b) in pairs {
        let (housed, out) = match (home_of(world, a), home_of(world, b)) {
            (Some(h), None) => (h, b),
            (None, Some(h)) => (h, a),
            _ => continue,
        };
        if !world.has::<Brain>(out)
            || world.has::<Sentence>(out)
            || world.comp::<Brain>(out).is_some_and(|br| br.emigrating)
            || !crate::systems::law::living(world, out)
        {
            continue;
        }
        let room = world.comp::<Building>(housed).is_some_and(|bd| {
            !bd.demolished && !bd.derelict && world.residents_of(housed).len() < usize::from(bd.capacity)
        });
        if !room {
            continue;
        }
        world.set_home(out, Some(housed));
        let (name, place) = (world.name_of(out), world.name_of(housed));
        let spouse = world.name_of(if out == a { b } else { a });
        world.push_event(EventKind::Housed, &[out, housed], format!("{name} moved into {place} with {spouse}"));
        world.stats.current.housed += 1;
        ownership::jobs2_row(world).reunited += 1;
    }
}

/// A Birth (`demography::spawn_child`): both parents hold the newborn for
/// `newborn_days`; the carer is the parent without a Job, else the lower
/// wage, else the mother (a hashed tie); the `Born` deed is talked about
/// in the home's district. Nothing off.
pub fn on_birth(world: &mut World, child: EntityId, mother: EntityId, father: EntityId) {
    if !on(world) {
        return;
    }
    let until = world.tick + Tick::from(world.config.household.newborn_days) * TICKS_PER_DAY;
    let wage = |w: &World, p: EntityId| w.comp::<Job>(p).map(|j| j.wage_per_day);
    let carer = match (wage(world, mother), wage(world, father)) {
        (None, _) => mother,
        (Some(_), None) => father,
        (Some(m), Some(f)) if m < f => mother,
        (Some(m), Some(f)) if f < m => father,
        _ => {
            if hash(world, PURPOSE_CARER, key(mother), key(father)) < 0.5 {
                mother
            } else {
                father
            }
        }
    };
    for p in [mother, father] {
        if let Some(h) = world.comp_mut::<Household>(p) {
            h.newborn_until = Some(until);
            h.newborn_carer = p == carer;
        }
    }
    ownership::jobs2_row(world).carers += 1;
    let d = crate::systems::gossip::talk_district(world, mother);
    crate::systems::gossip::post_deed(world, d, Deed::Born, Some(mother), Some(child));
}

/// A parent holding a newborn now.
pub fn new_parent(world: &World, id: EntityId) -> bool {
    on(world) && world.comp::<Household>(id).and_then(|h| h.newborn_until).is_some_and(|t| t > world.tick)
}

/// The newborn's carer now.
pub fn carer(world: &World, id: EntityId) -> bool {
    new_parent(world, id) && world.comp::<Household>(id).is_some_and(|h| h.newborn_carer)
}

/// § 3.4: a new parent's energy decays × `newborn_energy_mult` (`None` otherwise).
pub fn energy_mult(world: &World, id: EntityId) -> Option<f32> {
    new_parent(world, id).then_some(world.config.household.newborn_energy_mult)
}

/// Is it the night (`night_from`..`night_to` hours of the day)?
fn night(world: &World) -> bool {
    let h = u8::try_from(world.tick_of_day() / TICKS_PER_HOUR as u16).unwrap_or(0);
    let cfg = &world.config.household;
    if cfg.night_from <= cfg.night_to {
        h >= cfg.night_from && h < cfg.night_to
    } else {
        h >= cfg.night_from || h < cfg.night_to
    }
}

/// § 3.4: the carer's night goals (any but Work, Sleep, Eat and Flee
/// between `night_from` and `night_to`) score × `newborn_away_mult`.
pub fn damped(world: &World, id: EntityId, goal: GoalKind) -> bool {
    !matches!(goal, GoalKind::Work | GoalKind::Sleep | GoalKind::Eat | GoalKind::Flee)
        && night(world)
        && carer(world, id)
}

/// § 3.4: the carer's away spots (a spot not its Home) score × this (1 otherwise).
pub fn away_mult(world: &World, id: EntityId, own_home: bool) -> f32 {
    if !own_home && carer(world, id) {
        world.config.household.newborn_away_mult
    } else {
        1.0
    }
}

/// § 3.4: has `b` taken coins within `abandon_days` (today, or the last days)?
fn business_live(world: &World, b: EntityId) -> bool {
    let days = usize::from(world.config.household.abandon_days);
    world
        .comp::<Building>(b)
        .is_some_and(|bd| bd.revenue_today > 0 || bd.revenue.iter().rev().take(days).any(|&r| r > 0))
}

/// § 3.4 (`demography::job_search`): every adult who waits for a live
/// household business, in one pass over the buildings (the hiring pool
/// reads this set, not a scan per candidate). Empty off.
pub fn business_holders(world: &World) -> std::collections::BTreeSet<EntityId> {
    let mut out = std::collections::BTreeSet::new();
    if !on(world) {
        return out;
    }
    for b in world.with::<Building>() {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        if bd.demolished || bd.derelict || bd.kind == BuildingKind::Home {
            continue;
        }
        let Some(owner) = bd.owner.filter(|&o| world.has::<Brain>(o)) else { continue };
        if !business_live(world, b) {
            continue;
        }
        out.insert(owner);
        if let Some(sp) = world.spouse_of(owner) {
            out.insert(sp);
        }
    }
    out
}

/// § 3.4 (`demography::job_search`): the jobless adults of an owner's
/// household take the business's open posts first. Returns the hires made
/// (each vacancy closed by the caller). Nothing off.
pub fn owner_operator(world: &World, employer: EntityId, role: crate::components::Role) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let owner = world.comp::<Building>(employer).and_then(|bd| bd.owner).filter(|&o| world.has::<Brain>(o))?;
    // Review fix 1: the same gate as every other hire (`demography::pick_candidate`):
    // the role's age band, the retired and the frail-exited (`careers::admits`),
    // and a quitter's rehire block (`life::quit_blocks`).
    let band = crate::systems::careers::hire_band(world, role);
    let members: Vec<EntityId> = std::iter::once(owner).chain(world.spouse_of(owner)).collect();
    members.into_iter().find(|&m| {
        crate::systems::law::living(world, m)
            && crate::systems::demography::is_adult(world, m)
            && !world.has::<Job>(m)
            && !world.has::<Sentence>(m)
            && crate::systems::life::exec_corp(world, m).is_none()
            && world.comp::<Brain>(m).is_some_and(|b| !b.emigrating)
            && !crate::systems::life::quit_blocks(world, m, employer)
            && crate::systems::careers::admits(world, m, band)
            && crate::systems::dollhouse::hire_ok(world, role)
    })
}

/// § 3.4 (`wages`'s layoff order): is `who` of the household (the agent or
/// its spouse) that owns `b`?
pub fn is_member_of_owner(world: &World, who: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(owner) = world.comp::<Building>(b).and_then(|bd| bd.owner).filter(|&o| world.has::<Brain>(o)) else {
        return false;
    };
    who == owner || world.spouse_of(owner) == Some(who)
}

/// Midnight (from `living::run`): the visits due today, and the newborns'
/// keep through the pantry (`newborn_cost_per_day` on the Home's child
/// food debt, once a Home). Nothing off.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let cost = world.config.household.newborn_cost_per_day;
    let now = world.tick;
    // Review fix 11: every parent holding a newborn (a widowed or unmarried
    // one too), not only the `spouses` map's; the set keeps one charge a Home.
    let homes: std::collections::BTreeSet<EntityId> = world
        .with::<Household>()
        .into_iter()
        .filter(|&p| world.comp::<Household>(p).and_then(|h| h.newborn_until).is_some_and(|t| t > now))
        .filter_map(|p| home_of(world, p))
        .collect();
    for h in homes {
        if let Some(b) = world.comp_mut::<Building>(h) {
            b.child_food_debt += cost;
        }
    }
    visits_due(world);
}

// ---------------------------------------------------------------------------
// § 3.5: visits with a purpose
// ---------------------------------------------------------------------------

/// The kin of `id`: its spouse and its Family/Parent edges.
fn kin_of(world: &World, id: EntityId) -> Vec<(EntityId, RelKind)> {
    // The perf pass phase 6: the kin candidates, not every neighbour.
    world
        .kin_candidates(id)
        .filter_map(|c| {
            let e = world.edge(id, c)?;
            matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent).then_some((c, e.kind))
        })
        .collect()
}

/// Midnight: at most one visit a day an adult. A kin or spouse downed
/// (`World::wounded`), assaulted or arrested in the last day (the visitor's
/// own first-hand `SawCrime` with the kin as object, or a heard
/// `Assaulted`/`Arrested` rumour about it), else an elder parent (over
/// `[ages] frail_from_years`) not met for `elder_visit_days`. World-side:
/// O(adults × kin). Nothing off.
pub fn visits_due(world: &mut World) {
    world.visits.clear();
    if !on(world) {
        return;
    }
    let now = world.tick;
    let day = world.day();
    let since = now.saturating_sub(TICKS_PER_DAY);
    let frail = u32::try_from(u64::from(world.config.ages.frail_from_years) * DAYS_PER_YEAR).unwrap_or(u32::MAX);
    let elder_days = u64::from(world.config.household.elder_visit_days);
    let mut due: Vec<(EntityId, PendingVisit)> = Vec::new();
    // Lives at work phase 3 (§ 3.5): the parents' answers to their Camp
    // children due today (and the neglect tally).
    let camp_due = camp_answers_due(world);
    // scan-ok: daily: the visits due
    for id in world.citizens() {
        if !world.has::<Brain>(id)
            || world.has::<Sentence>(id)
            || !crate::systems::demography::is_adult(world, id)
            || !crate::systems::law::living(world, id)
            || world.comp::<Brain>(id).is_some_and(|b| b.emigrating)
        {
            continue;
        }
        let mut pick: Option<(EntityId, VisitWhy)> = None;
        for (k, rel) in kin_of(world, id) {
            if !world.has::<Brain>(k) || !crate::systems::law::living(world, k) || k == id {
                continue;
            }
            let downed = world.wounded.get(&k).is_some_and(|r| r.since >= since || r.rise_at.is_some_and(|t| t > now));
            let hurt = downed
                || world.comp::<crate::components::Memory>(id).is_some_and(|m| {
                    m.entries.iter().any(|e| {
                        e.tick >= since
                            && e.kind == MemoryKind::SawCrime
                            && e.object == Some(k)
                            && e.crime == Some(crate::components::Crime::Assault)
                    }) || m.heard.iter().any(|e| {
                        e.tick >= since
                            && e.object == Some(k)
                            && matches!(e.deed, Some(Deed::Assaulted | Deed::Arrested))
                    })
                });
            if hurt {
                pick = Some((k, VisitWhy::Hurt));
                break;
            }
            if rel == RelKind::Parent && pick.is_none() {
                let elder = world.comp::<Identity>(k).is_some_and(|i| i.age_days >= frail)
                    && crate::systems::demography::is_child_of(world, id, k);
                let unmet = world.last_met.get(&pair(id, k)).is_none_or(|&d| day.saturating_sub(d) >= elder_days);
                if elder && unmet {
                    pick = Some((k, VisitWhy::Elder));
                }
            }
        }
        if pick.is_none() {
            if let Some(&(child, answer)) = camp_due.get(&id) {
                pick = Some((child, VisitWhy::Camp(answer)));
            }
        }
        if let Some((kin, why)) = pick {
            let cfg = &world.config.household;
            let span = u32::from(cfg.visit_hour_to.saturating_sub(cfg.visit_hour_from)).max(1);
            let u = hash(world, PURPOSE_VISIT_HOUR, key(id), day);
            let hour = cfg.visit_hour_from + ((u * span as f32) as u8).min(span as u8 - 1);
            due.push((id, PendingVisit { kin, why, hour }));
        }
    }
    for (id, v) in due {
        world.visits.insert(id, v);
    }
}

fn pair(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Is the visit's kin still there to see: a Brain alive for a kin or an
/// elder; for a Camp child (no Brain), alive, no corpse, housed in a Camp.
fn kin_there(world: &World, v: PendingVisit) -> bool {
    match v.why {
        VisitWhy::Camp(_) => camp_of(world, v.kin).is_some(),
        VisitWhy::Papers | VisitWhy::Collect | VisitWhy::Service => {
            world.comp::<Building>(v.kin).is_some_and(|b| !b.demolished && !b.derelict)
        }
        _ => world.has::<Brain>(v.kin) && crate::systems::law::living(world, v.kin),
    }
}

/// The Camp `child` is housed in, if any.
fn camp_of(world: &World, child: EntityId) -> Option<EntityId> {
    if !world.is_alive(child) || world.has::<Corpse>(child) {
        return None;
    }
    world
        .comp::<Household>(child)
        .and_then(|h| h.home)
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp))
}

// ---------------------------------------------------------------------------
// § 3.5 (phase 3): the parent's answer to a taken child
// ---------------------------------------------------------------------------

/// Midnight (civic only): the parents' answers due today, once a week a
/// parent (the day of the week hashed on (parent, week), `PURPOSE_ANSWER`),
/// for each standing Camp's children; a parent with two taken children
/// answers for the lower id. The neglect tally beside it: a parent of a
/// Camp child with no `last_met` with it in `neglect_days`, counted a day
/// (printed, never a camp-take term: Dylan's answer 4).
fn camp_answers_due(world: &mut World) -> std::collections::BTreeMap<EntityId, (EntityId, CampAnswer)> {
    let mut due = std::collections::BTreeMap::new();
    if !crate::systems::service::civic_on(world) {
        return due;
    }
    let cfg = world.config.service.clone();
    let day = world.day();
    let week = day / 7;
    let mut neglect = 0u32;
    for camp in crate::systems::camp::standing_camps(world) {
        let kids: Vec<EntityId> = crate::systems::camp::children_of(world, camp).to_vec();
        for k in kids {
            for p in crate::systems::camp::parents_of(world, k) {
                if !world.has::<Brain>(p)
                    || world.has::<Sentence>(p)
                    || !crate::systems::demography::is_adult(world, p)
                    || world.comp::<Brain>(p).is_some_and(|b| b.emigrating)
                {
                    continue;
                }
                let met = world.last_met.get(&pair(p, k)).copied();
                if met.is_none_or(|d| day.saturating_sub(d) >= u64::from(cfg.neglect_days)) {
                    neglect += 1;
                }
                let u = hash(world, PURPOSE_ANSWER, key(p), week);
                let on_day = week * 7 + ((u * 7.0) as u64).min(6);
                if on_day != day || due.contains_key(&p) {
                    continue;
                }
                let answer = answer_of(world, p, &cfg);
                due.insert(p, (k, answer));
            }
        }
    }
    if neglect > 0 {
        ownership::jobs2_row(world).camp_neglect += neglect;
    }
    due
}

/// § 3.5: buy out when the purse holds `buyout_fee` and the Home's pantry
/// `buyout_food_days` of the child's food; else break out when lawless and
/// brave (lawfulness < `breakout_lawfulness`, courage ≥ `breakout_courage`,
/// `[service.tune]`) or a gang member with
/// `breakout_crew` of his gang alive and free; else visit.
fn answer_of(world: &World, p: EntityId, cfg: &crate::config::ServiceCfg) -> CampAnswer {
    let purse = world.purse(Some(p));
    let pantry = home_of(world, p).and_then(|h| world.comp::<Building>(h)).map_or(0.0, |b| b.stock_food as f32);
    if purse >= cfg.buyout_fee && pantry >= cfg.buyout_food_days * crate::systems::demography::CHILD_FOOD_PER_DAY {
        return CampAnswer::BuyOut;
    }
    let t = &cfg.tune;
    let lawless = world
        .comp::<Personality>(p)
        .is_some_and(|x| x.lawfulness < t.breakout_lawfulness && x.courage >= t.breakout_courage);
    let crew = world.gang_of(p).is_some_and(|g| crew_of(world, g, p).len() >= usize::from(cfg.breakout_crew));
    if lawless || crew {
        CampAnswer::BreakOut
    } else {
        CampAnswer::Visit
    }
}

/// The free, living members of gang `g` but `p`, ascending.
fn crew_of(world: &World, g: EntityId, p: EntityId) -> Vec<EntityId> {
    world
        .comp::<crate::components::Gang>(g)
        .map(|x| x.members.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&m| m != p && !world.has::<Sentence>(m) && crate::systems::law::living(world, m))
        .collect()
}

/// The answer made at the Camp (a body's walk done, or a Statistical
/// parent's hour): the visit, the buy-out or the break-out. The `last_met`
/// and the belonging are the visit's in every arm (the parent was there).
fn camp_answer(world: &mut World, id: EntityId, child: EntityId, answer: CampAnswer, walked: bool) {
    let today = world.day();
    world.last_met.insert(pair(id, child), today);
    let gain = world.config.household.visit_belonging;
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        n.belonging = (n.belonging + gain).min(1.0);
    }
    let Some(camp) = camp_of(world, child) else { return };
    match answer {
        CampAnswer::Visit => camp_visit(world, id, child, camp),
        CampAnswer::BuyOut => {
            let fee = world.config.service.buyout_fee;
            let owner = world.owner_of(camp);
            let paid = ownership::pay(world, Some(id), owner, fee, Flow::Buyout);
            if paid >= fee {
                child_home(
                    world,
                    id,
                    child,
                    camp,
                    EventKind::CampBuyout,
                    &format!("bought {{child}} out of {{camp}} for {fee}"),
                );
                ownership::jobs2_row(world).camp_buyouts += 1;
            } else {
                // The purse moved since midnight: the coins back, a visit instead.
                if paid > 0 {
                    ownership::refund(world, id, owner, paid, Flow::Buyout);
                }
                camp_visit(world, id, child, camp);
            }
        }
        CampAnswer::BreakOut => breakout(world, id, child, camp, walked),
    }
}

/// § 3.5 (visit): a `Served` memory with the warden on duty, if any (the
/// visit is not queued at his counter: his row serves children), the
/// counter, the event.
fn camp_visit(world: &mut World, id: EntityId, child: EntityId, camp: EntityId) {
    let salience = world.config.service.served_salience;
    if let Some(&w) = crate::systems::service::servers_on_duty(world, camp, ServeKind::Inmate).first() {
        world.remember(id, MemoryKind::Served, Some(w), salience, -0.1, false);
        world.remember(w, MemoryKind::Served, Some(id), salience, 0.0, false);
    }
    ownership::jobs2_row(world).camp_visits += 1;
    let text =
        format!("{} visited {} at {} (a Camp child)", world.name_of(id), world.name_of(child), world.name_of(camp));
    world.push_event(EventKind::Visited, &[id, child, camp], text);
}

/// The child out of the Camp and home (`camp::release`, then the parent's
/// Home with room, else the street as `release` leaves it), the event.
fn child_home(world: &mut World, id: EntityId, child: EntityId, camp: EntityId, kind: EventKind, what: &str) {
    crate::systems::camp::release(world, child, crate::systems::camp::ReleaseWhy::BoughtOut);
    let home = crate::systems::camp::send_home(world, child, id);
    let (name, cn, camp_name) = (world.name_of(id), world.name_of(child), world.name_of(camp));
    let to = home.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
    let text = format!("{name} {}, to {to}", what.replace("{child}", &cn).replace("{camp}", &camp_name));
    world.push_event(kind, &[id, child, camp], text);
}

/// § 3.5 (break-out): the parent and his gang's free members at the door
/// (a body's within `raid_gather_radius`; a Statistical parent's the first
/// `breakout_crew`) against the wardens present and the Camp's robots,
/// `raid::fight_out`. Won: the child home and an `Abduction` raised on the
/// parent at the door; lost: the strongest warden standing arrests a body
/// (`law::arrest` takes any arrester adjacent), else the Abduction is
/// filed with the parent as suspect and the warrant runs (deviation: a
/// Statistical parent has no body at the door).
fn breakout(world: &mut World, id: EntityId, child: EntityId, camp: EntityId, walked: bool) {
    let Some(door) = world.comp::<Building>(camp).map(|b| b.door) else { return };
    let cfg = world.config.service.clone();
    let radius = world.config.gangs.raid_gather_radius;
    let mut raiders = vec![id];
    if let Some(g) = world.gang_of(id) {
        let crew = crew_of(world, g, id);
        let cap = usize::from(cfg.breakout_crew);
        if walked {
            raiders.extend(crew.into_iter().filter(|&m| {
                world.comp::<Position>(m).is_some_and(|p| crate::systems::law::chebyshev(p.tile, door) <= radius)
            }));
        } else {
            raiders.extend(crew.into_iter().take(cap));
        }
    }
    crate::systems::raid::by_strength(world, &mut raiders);
    let mut defenders = crate::systems::service::servers_on_duty(world, camp, ServeKind::Inmate);
    crate::systems::raid::robots_first(world, camp, &mut defenders);
    let witness = defenders.first().copied();
    let _tally = crate::systems::raid::fight_out(world, &mut raiders, &mut defenders, door, "the Camp", 1.0, None);
    if defenders.is_empty() {
        child_home(world, id, child, camp, EventKind::CampBreakout, "broke {child} out of {camp}");
        if crate::systems::law::living(world, id) {
            crate::systems::law::raise_crime_on(world, id, None, Some(child), Crime::Abduction, door);
            crate::systems::law::file_report(world, Crime::Abduction, id, witness);
        }
        ownership::jobs2_row(world).camp_breakouts += 1;
        return;
    }
    if !crate::systems::law::living(world, id) {
        return;
    }
    let cuffed =
        walked && defenders.iter().any(|&w| world.has::<Brain>(w) && crate::systems::law::arrest(world, w, id));
    if !cuffed {
        crate::systems::law::file_report(world, Crime::Abduction, id, defenders.first().copied());
    }
}

// ---------------------------------------------------------------------------
// § 2.2 (phase 3): tied housing
// ---------------------------------------------------------------------------

/// Midnight (`ownership::run`, after `reunite`, before `rehouse`; civic
/// only): a tied trade's worker (`[[trades]] tied_housing`) whose Home's
/// owner is not his employer's owner moves, with his co-resident spouse and
/// children, into the standing Block of that owner in the post's district
/// nearest the post with room for them all (never evicting: room or
/// nothing); `Housed` as `rehouse` posts it. Rent-free there
/// (`ownership::tied_here`).
pub fn tied_housing(world: &mut World) {
    if !crate::systems::service::civic_on(world) {
        return;
    }
    // scan-ok: daily: the tied trades' workers
    let workers: Vec<(EntityId, EntityId)> = world
        .citizens()
        .into_iter()
        .filter_map(|a| {
            let j = world.comp::<Job>(a)?;
            let row = crate::systems::trades::row_of(world, j.role)?;
            row.tied_housing.then_some((a, j.employer?))
        })
        .collect();
    for (a, post) in workers {
        if !world.has::<Brain>(a)
            || world.has::<Sentence>(a)
            || !crate::systems::law::living(world, a)
            || world.comp::<Brain>(a).is_some_and(|b| b.emigrating)
        {
            continue;
        }
        let Some(owner) = world.owner_of(post) else { continue };
        let home = home_of(world, a);
        if home.is_some_and(|h| world.owner_of(h) == Some(owner)) {
            continue;
        }
        // A household already housed by a co-resident's landlord stays: two
        // supers of two landlords married each other and the first build
        // moved the couple between the two landlords' Blocks every midnight.
        if home.is_some_and(|h| world.residents_of(h).iter().any(|&r| r != a && ownership::tied_here(world, r, h))) {
            continue;
        }
        let mut movers = vec![a];
        if let Some(h) = home {
            for &r in world.residents_of(h) {
                if r != a && (world.spouse_of(a) == Some(r) || crate::systems::demography::is_child_of(world, r, a)) {
                    movers.push(r);
                }
            }
        }
        let d = world.district_of_building(post);
        let Some(from) = world.comp::<Building>(post).map(|b| b.door) else { continue };
        let pick = world
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .filter(|&b| world.owner_of(b) == Some(owner) && world.district_of_building(b) == d)
            // The island pass phase 3a (I6): a company bed is its staff's only.
            .filter(|&b| crate::systems::company::open_to_all(world, b))
            .filter(|&b| {
                world.comp::<Building>(b).is_some_and(|bd| {
                    !bd.demolished
                        && !bd.derelict
                        && world.residents_of(b).len() + movers.len() <= usize::from(bd.capacity)
                })
            })
            .min_by_key(|&b| (world.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(from)), b));
        let Some(to) = pick else { continue };
        for &m in &movers {
            world.set_home(m, Some(to));
            if let Some(h) = world.comp_mut::<Household>(m) {
                h.rent_due = 0.0;
                h.arrears = 0;
            }
        }
        let (name, place) = (world.name_of(a), world.name_of(to));
        let with = movers.len() - 1;
        world.push_event(
            EventKind::Housed,
            &[a, to],
            format!("{name} moved into {place}, the landlord's Block, as its super (with {with} of the household)"),
        );
        world.stats.current.housed += 1;
        ownership::jobs2_row(world).tied_housed += 1;
    }
}

/// The visit `id` owes today, once its hour has come.
pub fn visit_now(world: &World, id: EntityId) -> Option<PendingVisit> {
    let v = *world.visits.get(&id)?;
    (u16::from(v.hour) * TICKS_PER_HOUR as u16 <= world.tick_of_day()).then_some(v)
}

/// Where a Full visitor walks for `v`: the building itself for a building
/// visit, else the building the kin is in now (a kin on the street, or a
/// Statistical one with no building, has nowhere to walk to: the plan's own
/// rule, so the goal never wins a think it cannot plan; Shadow V5 bug 3).
fn visit_place(world: &World, v: PendingVisit) -> Option<EntityId> {
    if v.why.to_building() {
        return Some(v.kin);
    }
    world.comp::<Position>(v.kin).and_then(|p| p.building)
}

/// The Visit goal's considerations: a visit due and its hour come, a kin
/// still there to see and a place to walk to (the plan's rule), and not on
/// a shift (§ 1.3: no shift is interrupted; Shadow V5 bug 3: the Visit beat
/// Work at 1.08 and cut the shift); the flat `visit_flat`.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) {
        return None;
    }
    let v = visit_now(world, id)?;
    if !kin_there(world, v) || visit_place(world, v).is_none() {
        return None;
    }
    let on_shift =
        world.comp::<crate::components::Job>(id).is_some_and(|j| crate::utility::goals::shift_reachable(world, id, j));
    let u = &world.config.household.visit_urgency;
    let urgency = match v.why {
        VisitWhy::Hurt => u.hurt,
        VisitWhy::Elder => u.elder,
        VisitWhy::Camp(_) => u.camp,
        VisitWhy::Papers => u.papers,
        VisitWhy::Collect => u.collect,
        VisitWhy::Confront | VisitWhy::Dun => crate::systems::underwater::urgency(world, v.why),
        VisitWhy::Blackmail | VisitWhy::Service => crate::systems::consequence::urgency(world, v.why),
    };
    let cs = vec![
        Consideration::new("a visit due", can(true), GATE),
        Consideration::new("the reason", urgency, Curve::Linear { m: u.curve.0, b: u.curve.1 }),
        Consideration::new("not on shift", can(!on_shift), GATE),
    ];
    Some((cs, world.config.household.visit_flat))
}

/// The Visit's scripted plan: `GoTo(the kin's building) → Rest` (the
/// visiting hour is a Rest at the place: the Clinic's ward, the Home's
/// room, the Precinct's hall), the kin the plan's target. A visit due with
/// nowhere to walk is dropped for the day, not retried at every cooldown
/// (Shadow V5 bug 3: 312 "could not plan" lines in 26 diaries).
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let v = visit_now(world, id)?;
    if visit_place(world, v).is_none() {
        world.visits.remove(&id);
        return None;
    }
    let step = |action, target| ActionInstance { action, target, tile: None };
    // Lives at work phase 5 (§ 2.4): the Fixer's walk-in and the owner's
    // collection walk to a building (the exec's office walk's shape).
    if v.why.to_building() {
        if world.comp::<Position>(id).and_then(|p| p.building) == Some(v.kin) {
            let steps = vec![step(ActionKind::Rest, None)];
            return Some(Plan { goal: GoalKind::Visit, target: Some(v.kin), steps, started_tick: world.tick });
        }
        let steps = vec![step(ActionKind::GoTo(LocationKey::Seller), Some(v.kin)), step(ActionKind::Rest, None)];
        return Some(Plan { goal: GoalKind::Visit, target: Some(v.kin), steps, started_tick: world.tick });
    }
    let steps = vec![step(ActionKind::GoTo(LocationKey::Victim), Some(v.kin)), step(ActionKind::Rest, None)];
    Some(Plan { goal: GoalKind::Visit, target: Some(v.kin), steps, started_tick: world.tick })
}

/// The Visit's own start checks: the walk as any walk; the Rest only at
/// the kin's building.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind) -> bool {
    match kind {
        ActionKind::Rest => {
            let kin = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.target);
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            // Lives at work phase 5: a building visit rests inside it.
            if kin.is_some_and(|k| world.has::<Building>(k)) {
                return here.is_some() && here == kin;
            }
            here.is_some() && kin.and_then(|k| world.comp::<Position>(k)).is_some_and(|p| p.building == here)
        }
        _ => crate::exec::actions::can_start(world, id, kind, None),
    }
}

/// The credit a visit gives both sides: `interacted`, `last_met`, the
/// visitor's belonging (`visit_belonging`), the counter, an event.
fn credit(world: &mut World, id: EntityId, v: PendingVisit, walked: bool) {
    // Lives at work phase 3 (§ 3.5): a Camp child has no edge table and no
    // Brain: its arm writes the `last_met` and the belonging itself.
    if let VisitWhy::Camp(answer) = v.why {
        camp_answer(world, id, v.kin, answer, walked);
        return;
    }
    // Lives at work phase 5 (§ 2.4): the walk-in's sale, the owner's collection.
    match v.why {
        VisitWhy::Papers => return crate::systems::service::walk_in_arrived(world, id, v.kin),
        VisitWhy::Collect => return crate::systems::service::collected(world, id, v.kin),
        // The pressure pass phase 6: the stiffed worker's scene, the collector's call.
        VisitWhy::Confront => return crate::systems::underwater::confronted(world, id, v.kin, walked),
        VisitWhy::Dun => return crate::systems::underwater::dunned(world, id, v.kin, walked),
        // The pressure pass phase 5a: the blackmailer's demand, the service.
        VisitWhy::Blackmail => return crate::systems::consequence::blackmailed(world, id, v.kin, walked),
        VisitWhy::Service => return crate::systems::consequence::attended(world, id, v.kin, walked),
        _ => {}
    }
    crate::systems::social::interacted(world, id, v.kin);
    let today = world.day();
    world.last_met.insert(pair(id, v.kin), today);
    let gain = world.config.household.visit_belonging;
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        n.belonging = (n.belonging + gain).min(1.0);
    }
    let row = ownership::jobs2_row(world);
    match v.why {
        VisitWhy::Hurt => row.kin_visits += 1,
        VisitWhy::Elder => row.elder_visits += 1,
        VisitWhy::Camp(_)
        | VisitWhy::Papers
        | VisitWhy::Collect
        | VisitWhy::Confront
        | VisitWhy::Dun
        | VisitWhy::Blackmail
        | VisitWhy::Service => {}
    }
    let place = world.comp::<Position>(v.kin).and_then(|p| p.building);
    let at = place.map_or_else(|| "where they were".to_string(), |b| world.name_of(b));
    let text = format!(
        "{} visited {} at {at}{}",
        world.name_of(id),
        world.name_of(v.kin),
        match (v.why, walked) {
            (VisitWhy::Hurt, _) => " (hurt)",
            (VisitWhy::Elder, _) => " (an elder parent)",
            (VisitWhy::Camp(_), _) => " (a Camp child)",
            (
                VisitWhy::Papers
                | VisitWhy::Collect
                | VisitWhy::Confront
                | VisitWhy::Dun
                | VisitWhy::Blackmail
                | VisitWhy::Service,
                _,
            ) => "",
        }
    );
    let actors: Vec<EntityId> = [Some(id), Some(v.kin), place].into_iter().flatten().collect();
    world.push_event(EventKind::Visited, &actors, text);
}

/// A Full visitor's plan ended at the kin's side (the Rest done): the credit.
pub fn visit_done(world: &mut World, id: EntityId) {
    let Some(v) = world.visits.remove(&id) else { return };
    credit(world, id, v, true);
}

/// Hourly (from `living::run`): a Statistical visitor is credited at its
/// visit's hour, as a body's walk would be. Nothing off.
pub fn hourly(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) || world.visits.is_empty() {
        return;
    }
    let hour = u8::try_from(world.tick_of_day() / TICKS_PER_HOUR as u16).unwrap_or(0);
    let legs_on = crate::exec::legs::on(world);
    let stat: Vec<(EntityId, PendingVisit)> = world
        .visits
        .iter()
        // Pre-view pass phase 5 (`[legs]`): a Camp visitor demoted mid-ride, after its
        // hour, is credited at the next hour (a ridden leg outlasts the hour; the
        // Camp answer of a Coarse parent was lost to the demotion).
        .filter(|(_, v)| v.hour == hour || (legs_on && v.hour < hour && matches!(v.why, VisitWhy::Camp(_))))
        .filter(|(&id, _)| world.comp::<Brain>(id).is_some_and(|b| b.lod == crate::components::Lod::Statistical))
        .map(|(&id, &v)| (id, v))
        .collect();
    for (id, v) in stat {
        if kin_there(world, v) {
            credit(world, id, v, false);
        }
        world.visits.remove(&id);
    }
}

/// The lives probe's and inspector's reading of a Home spot (for the diary).
pub fn home_label(world: &World, home: EntityId, own: bool) -> String {
    format!("{}{}", world.name_of(home), if own { " (home)" } else { " (a friend's place)" })
}
