//! Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1, § 2.1, § 2.2's
//! ripperdoc and orderly): the **service encounter**, the one mechanism
//! that puts a person on the other side of every counter. Under `[lives]
//! enabled` and `[service] venues` (`[lives] enabled = false`): off, every
//! entry point is a no-op, no queue fills, no ledger is written and the
//! save is byte-identical to main.
//!
//! An encounter is `(server, served, place, kind)`. The flows the city
//! already has bring the served to the place through the one-line hook
//! [`arrive`] (a venue visit's completion, a Clinic sale or treatment, a
//! stabilisation, a bout's bettors, a Brothel worker's shift for the madam).
//! The encounter resolves at the arrival's minute (deviation from § 1.2's
//! hourly drain, see docs/LIVES_AT_WORK.md § 14: presence is read when the
//! visitor is there, so a shift boundary or the Statistical evening's
//! one-hour batch loses no tip): the served goes to the on-duty server with
//! room left this hour (its `[[serves]]` row's `per_hour`, the ledger
//! `World::service_room`) whose `hash(PURPOSE_SERVE, place, served, server)`
//! is lowest (D6: never the lowest id), FIFO by arrival; with nobody on duty
//! or every counter full the arrival is the "waited" outcome, counted.
//! [`hourly`] (from `living::run` after `vice::hourly`) prunes the room
//! ledger and queues the madam's `Staff` encounters. [`serve`] applies the
//! row's arm: a `Served` memory on both
//! sides (bodies only: `World::remember` drops it for a Statistical agent
//! and under a chip), the regulars ledger (both tiers; an Acquaintance
//! edge at `regular_after` visits within `regular_days`, `interacted`
//! after), the server's `skill_gain`, the tip to the server named (so
//! `vice::tipped_for`'s turn-about is superseded) and the madam's cut of
//! it, and an `EventKind::Served` for the outcomes the row's `event_on`
//! names (§ 1.4: a refusal, an ejection, a companion's new regular).
//!
//! **Presence** (§ 1.2): a server is on duty when its `Job` is on shift and
//! a workday, it is not held, and it is a body inside the place or a
//! Statistical agent counted present by `lod::shift_attended` on its role's
//! learned attendance.
//!
//! **The Statistical mirror** (§ 6): the pass is world-side and tier-blind;
//! the hooks fire from the same functions for both tiers (`stat_visit` and
//! `on_complete`, `chrome::installed_now`, `stims::detox`, …); the memories
//! carry mood weight 0 (`mood::memory_term` skips `Served`). No world-stream
//! draw: every roll is `econ::hash_unit` on a purpose of this file.
//!
//! Also here: the door refusal ([`refused`], from the exec's failed start at
//! a venue door), the bouncer's ejection ([`maybe_eject`], from the Attack's
//! completion inside a kept venue) and the shift-end rule ([`shift_end`]:
//! the cook's staff meal for every tier, the staffed-shift count).

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::components::{
    Brain, Building, BuildingKind, Child, Corpse, Household, Job, Lod, MemoryKind, Position, Role, Sentence, ServeKind,
};
use crate::config::{ServeOutcome, ServesCfg, TradeSkill};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_HOUR};
use crate::world::World;

mod encounter;
mod hours;
mod meetings;
mod walkins;

pub use encounter::*;
pub use hours::*;
pub use meetings::*;
pub use walkins::*;

/// The server a served goes to among those with room (D6).
const PURPOSE_SERVE: u64 = 0x5e52_0001;
/// A regular's first-meeting affinity.
const PURPOSE_REGULAR: u64 = 0x5e52_0002;
/// The bouncer's ejection roll.
const PURPOSE_EJECT: u64 = 0x5e52_0003;
/// The bouncer a refusal is read as (among the door staff).
const PURPOSE_DOOR: u64 = 0x5e52_0004;
/// The tipped worker a visit's tip goes to when the server is not tipped.
const PURPOSE_TIP: u64 = 0x5e52_0005;
/// Lives at work phase 5: the attendees of a Meeting hour (§ 2.4).
const PURPOSE_TABLE: u64 = 0x5e52_0006;
/// Lives at work phase 5: the exec's call between the two best orders.
const PURPOSE_DECIDE: u64 = 0x5e52_0007;
/// Lives at work phase 5: the Fixer's server a walk-in or a contract party goes to.
const PURPOSE_FIXER: u64 = 0x5e52_0008;
/// Lives at work phase 5: the clerk a licence is sold by.
const PURPOSE_CLERK: u64 = 0x5e52_0009;

/// The regulars a server keeps (§ 1.1: the oldest falls off).
pub const REGULARS_CAP: usize = 8;

/// A server's room this hour (`World::service_room`, saved since OPEN-113):
/// the hour and the encounters taken in it (`per_hour` is the cap).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    pub hour: Tick,
    pub taken: u8,
}

/// One of a server's regulars (`World::regulars`, saved).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Regular {
    pub who: EntityId,
    pub visits: u8,
    /// The day of the last encounter.
    pub last_day: u16,
}

/// The encounter's outcome, as `[[serves]] event_on` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Refused,
    Ejected,
    Regular,
    /// Lives at work phase 3 (§ 2.2, super): a tenant in arrears served notice.
    Notice,
}

/// Phase 1 is live: `[jobs2] enabled`, `[lives] enabled` and `[service] venues`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.lives.enabled && world.config.service.venues
}

/// OPEN-111, Shadow V5 bug 1: the shift a legacy venue `role` is hired onto
/// at `employer` when `[service] venues` is on and the role is in `[service]
/// evening_staff`. A venue trades from the day into the evening (no venue
/// keeps hours; 73 of 100 Bar visits in V5's diaries came before 18:00), so
/// its staff of the role split between `[world] shift_day` and `[leisure]
/// evening_shift`: the thinner of the two among the role's staff there now,
/// the evening on a tie (a venue of one keeps the evening, the drinking's
/// peak and the Statistical evening). Seeding and `demography::hire` alike;
/// a Statistical server is credited by the same `Job::shifts`, so the
/// mirror follows. `None`: the role's shift as before.
pub fn venue_shift(world: &World, employer: Option<EntityId>, role: Role) -> Option<Vec<(u16, u16)>> {
    let cfg = &world.config;
    if !(cfg.jobs2.enabled && cfg.lives.enabled && cfg.service.venues && cfg.service.evening_staff.contains(&role)) {
        return None;
    }
    let (day, eve) = (&cfg.world.shift_day, &cfg.leisure.evening_shift);
    let (mut on_day, mut on_eve) = (0usize, 0usize);
    for &s in employer.map_or(&[][..], |b| world.staff_of(b)) {
        match world.comp::<Job>(s).filter(|j| j.role == role) {
            Some(j) if j.shifts == *day => on_day += 1,
            Some(j) if j.shifts == *eve => on_eve += 1,
            _ => {}
        }
    }
    Some(if on_day < on_eve { day.clone() } else { eve.clone() })
}

/// The shifts a `[service] evening_staff` role works at its venues taken
/// together (`wages::site_shifts`' room count): the day and the evening.
pub fn venue_shifts_all(cfg: &crate::config::Config, role: Role) -> Option<Vec<(u16, u16)>> {
    (cfg.jobs2.enabled && cfg.lives.enabled && cfg.service.venues && cfg.service.evening_staff.contains(&role))
        .then(|| cfg.world.shift_day.iter().chain(cfg.leisure.evening_shift.iter()).copied().collect())
}

/// Lives at work phase 3 is live: the encounter on and `[service] civic`
/// (`[service] civic = false`): the civic counterparts, the near round, tied
/// housing and the parent's answer to a taken child.
pub fn civic_on(world: &World) -> bool {
    on(world) && world.config.service.civic
}

/// Lives at work phase 4 is live: the encounter on, `[jobs2] street` (the
/// pitches and the rounds exist) and `[service] street`: the cab roll shared by both tiers and the
/// driver's body, the parcel, the lift, the client, the pimp's round, the
/// dealer's corner and regulars.
pub fn street_on(world: &World) -> bool {
    on(world) && crate::systems::streetwork::on(world) && world.config.service.street
}

/// Lives at work phase 5 is live: the encounter on and `[service] tower`
/// (`[service] tower = false`): the exec's Meeting and decision, the reporter's
/// beat and byline, the Fixer's walk-in and contract encounters, the
/// owner's weekly collection, the civic clerk named.
pub fn tower_on(world: &World) -> bool {
    on(world) && world.config.service.tower
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// The `[[serves]]` row of a role: a trade by its id, a legacy role by its name.
pub fn row_for(world: &World, role: Role) -> Option<&ServesCfg> {
    let rows = &world.config.serves;
    match role.trade() {
        Some(t) => {
            let id = &world.config.trade(t)?.id;
            rows.iter().find(|r| &r.key == id)
        }
        None => {
            let name = format!("{role:?}");
            rows.iter().find(|r| r.key == name)
        }
    }
}

/// A `[[serves]]` key whose trade row takes tips (`[[trades]] tipped`).
fn row_tipped(world: &World, key: &str) -> bool {
    world.config.trades.iter().any(|t| t.id == key && t.tipped)
}

/// The row of `id`'s job, if it serves.
fn row_of_agent(world: &World, id: EntityId) -> Option<&ServesCfg> {
    match world.comp::<Job>(id) {
        Some(j) => row_for(world, j.role),
        // Lives at work phase 4 (§ 2.3): a Pitch owner (the street worker,
        // the rickshaw puller) holds no Job: the row of his pitch's trade.
        None if street_on(world) => {
            let t = world.jobs2.pitches.get(&id)?.trade;
            let key = &world.config.trade(t)?.id;
            world.config.serves.iter().find(|r| &r.key == key)
        }
        None => None,
    }
}

/// Round 4 T4 (OPEN-217): `id`'s serving row is one of `[verbs]
/// night_rows` (the dancer, the club staff: the night told, the venue's
/// off-screen visits drawn into the shift); with `sets`, of `set_rows`.
pub fn night_row(world: &World, id: EntityId, sets: bool) -> bool {
    let Some(v) = world.config.verbs.as_ref() else { return false };
    let Some(row) = row_of_agent(world, id) else { return false };
    let keys = if sets { &v.set_rows } else { &v.night_rows };
    row.kind == ServeKind::Visit && keys.contains(&row.key)
}

/// The tipped worker on duty at `place` a visit's tip goes to when the
/// encounter's server is not tipped: the lowest hash among the present
/// holders of a tipped `Visit` row (room not counted: a tip is coins, not
/// time).
fn tipped_pick(world: &World, place: EntityId, served: EntityId) -> Option<EntityId> {
    let hour = world.tick / TICKS_PER_HOUR;
    world
        .staff_of(place)
        .iter()
        .copied()
        .filter(|&s| {
            row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Visit && row_tipped(world, &r.key))
                && present(world, s, place)
        })
        .map(|s| (hash(world, PURPOSE_TIP, key(place) ^ key(served), hour ^ key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, s)| s)
}

/// § 1.2's presence: on shift today, not held, a body inside the place or a
/// Statistical agent credited by `lod::shift_attended`.
pub fn present(world: &World, s: EntityId, place: EntityId) -> bool {
    let Some(j) = world.comp::<Job>(s) else { return false };
    if j.employer != Some(place)
        || !j.on_shift(world.tick_of_day())
        || !crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick))
        || world.has::<Sentence>(s)
        || !crate::systems::law::living(world, s)
    {
        return false;
    }
    match world.comp::<Brain>(s).map(|b| b.lod) {
        Some(Lod::Statistical) => crate::systems::lod::stat_attend(world, s, j.role)
            .is_none_or(|p| crate::systems::lod::shift_attended(world, s, j, p)),
        Some(_) => world.comp::<Position>(s).is_some_and(|p| p.building == Some(place)),
        None => false,
    }
}

/// The on-duty servers of `place` whose row serves `kind`, ascending (the
/// inspector's and the probes' reading of "who is behind the counter").
pub fn servers_on_duty(world: &World, place: EntityId, kind: ServeKind) -> Vec<EntityId> {
    servers(world, place, kind).into_iter().map(|(s, _)| s).collect()
}

/// The on-duty servers of `place` whose row serves `kind`, ascending.
fn servers(world: &World, place: EntityId, kind: ServeKind) -> Vec<(EntityId, u8)> {
    world
        .staff_of(place)
        .iter()
        .copied()
        .filter_map(|s| {
            let row = row_of_agent(world, s)?;
            (row.kind == kind && present(world, s, place)).then_some((s, row.per_hour))
        })
        .collect()
}

/// Round 3 S4 (OPEN-216): the on-duty server of `place` [`arrive`] would
/// hand `served` to now (room left this hour, the pair's lowest hash);
/// `None` with none (or off).
pub fn server_for(world: &World, place: EntityId, kind: ServeKind, served: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    pick_server(world, place, kind, served)
}

/// Round 7 W4 (OPEN-245; `[care] install_rows`): who cuts `served`'s chrome
/// at `place`: a server of a cutting row (`install_rows`, the Ripperdoc, the
/// back room's doc) on duty there, one with room this hour first, the pair's
/// lowest hash; else a cutting row's holder on a working shift there by the
/// clock (the counter `assets::seller_open` opened on); never the patient
/// himself (a doctor's own chrome is a colleague's, or nobody's named). Empty rows: the
/// patient queue's server ([`server_for`]). V15: U6's body-first pick handed
/// a Clinic's installs to the orderly at the triage (36 a week, unpaid).
pub fn install_server(world: &World, place: EntityId, served: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let rows = crate::systems::care::cfg(world).map(|c| c.install_rows.as_slice()).unwrap_or_default();
    if rows.is_empty() {
        return pick_server(world, place, ServeKind::Patient, served);
    }
    // Never his own chrome (a doctor's own install is a colleague's work).
    let cuts = |s: EntityId| s != served && row_of_agent(world, s).is_some_and(|r| rows.iter().any(|k| k == &r.key));
    let by_hash = |pool: Vec<EntityId>| {
        pool.into_iter()
            .map(|s| (hash(world, PURPOSE_SERVE, key(place) ^ key(served), key(s)), s))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, s)| s)
    };
    let on_duty: Vec<EntityId> =
        world.staff_of(place).iter().copied().filter(|&s| cuts(s) && present(world, s, place)).collect();
    let roomy: Vec<EntityId> = on_duty.iter().copied().filter(|&s| has_room(world, s)).collect();
    if let Some(s) = by_hash(roomy).or_else(|| by_hash(on_duty)) {
        return Some(s);
    }
    let tod = world.tick_of_day();
    let clock: Vec<EntityId> = world
        .staff_of(place)
        .iter()
        .copied()
        .filter(|&s| {
            cuts(s)
                && !world.has::<Sentence>(s)
                && crate::systems::law::living(world, s)
                && world.comp::<Job>(s).is_some_and(|j| {
                    j.employer == Some(place)
                        && j.on_shift(tod)
                        && crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick))
                })
        })
        .collect();
    by_hash(clock)
}

/// Round 7 W4: `served` is served at `place` by `server` (the one who did
/// the work, named already), taking one of his hour's room: [`arrive`] with
/// the server fixed.
pub fn arrive_by(world: &mut World, server: EntityId, served: EntityId, place: EntityId, kind: ServeKind, coins: i64) {
    if !on(world) || served == EntityId::NONE || !world.has::<Building>(place) {
        return;
    }
    let hour = world.tick / TICKS_PER_HOUR;
    let r = world.service_room.entry(server).or_default();
    if r.hour != hour {
        *r = Room { hour, taken: 0 };
    }
    r.taken = r.taken.saturating_add(1);
    if kind == ServeKind::Patient && crate::systems::presence::is_body(world, server) {
        ownership::jobs2_row(world).clinic_body += 1;
    }
    serve(world, server, served, place, kind, coins);
}

/// The on-duty server of `place` with room this hour whose pair hash with
/// `served` is lowest (D6, OPEN-111). Round 4 T4 (OPEN-217; `[verbs]
/// door_body_first`): a visit goes to a body of `[verbs] night_rows` on the
/// floor with room first (the camera's dancer or club hand; at 100k a
/// venue's off-screen colleagues, present by the clock, took all but one
/// in twenty: "a room of 22, nobody to serve").
fn pick_server(world: &World, place: EntityId, kind: ServeKind, served: EntityId) -> Option<EntityId> {
    let hour = world.tick / TICKS_PER_HOUR;
    let free: Vec<EntityId> = servers(world, place, kind)
        .into_iter()
        .filter(|&(s, cap)| world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap))
        .map(|(s, _)| s)
        .collect();
    let first = kind == ServeKind::Visit && world.config.verbs.as_ref().is_some_and(|v| v.door_body_first);
    let bodies: Vec<EntityId> = if first {
        free.iter()
            .copied()
            .filter(|&s| {
                world.comp::<Brain>(s).is_some_and(|b| b.lod != Lod::Statistical) && night_row(world, s, false)
            })
            .collect()
    } else {
        Vec::new()
    };
    // Round 5 U6 (OPEN-217; `[care] clinic_body_first`): a patient goes to
    // a doctor's body on duty with room first (the Clinic's off-screen
    // colleagues, present by the clock, took the pair hash: V13's 100k
    // ripperdocs saw 0 and 1 patients a week).
    let bodies = if bodies.is_empty()
        && kind == ServeKind::Patient
        && crate::systems::care::cfg(world).is_some_and(|c| c.clinic_body_first)
    {
        free.iter().copied().filter(|&s| world.comp::<Brain>(s).is_some_and(|b| b.lod != Lod::Statistical)).collect()
    } else {
        bodies
    };
    let pool = if bodies.is_empty() { free } else { bodies };
    pool.into_iter()
        .map(|s| (hash(world, PURPOSE_SERVE, key(place) ^ key(served), key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, s)| s)
}

/// Round 3 S4 (OPEN-216): `s` has room this hour for one more of his
/// row's `per_hour` (no row: room).
pub fn has_room(world: &World, s: EntityId) -> bool {
    let Some(per_hour) = row_of_agent(world, s).map(|r| r.per_hour) else { return true };
    let hour = world.tick / TICKS_PER_HOUR;
    world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < per_hour)
}

/// Round 3 S4: a walk-in seen by `s` at `place` while he is not on duty
/// there (a back room's doc on call: [`arrive`] serves only the on-duty)
/// takes one of his hour's room.
pub fn take_room_on_call(world: &mut World, s: EntityId, place: EntityId) {
    if !on(world) || present(world, s, place) {
        return;
    }
    let hour = world.tick / TICKS_PER_HOUR;
    let r = world.service_room.entry(s).or_default();
    if r.hour != hour {
        *r = Room { hour, taken: 0 };
    }
    r.taken = r.taken.saturating_add(1);
}

/// The one-line hook: `served` arrives at `place` for `kind`, with the
/// coins its source moved, and is served now by the on-duty server with
/// room left this hour whose hash is lowest (D6), or waits (counted).
/// Nothing off.
pub fn arrive(world: &mut World, served: EntityId, place: EntityId, kind: ServeKind, coins: i64) {
    if !on(world) || served == EntityId::NONE || !world.has::<Building>(place) {
        return;
    }
    // Round 3 S5 (OPEN-217): the door's ledger (every arrival, served or
    // waiting).
    if kind == ServeKind::Visit {
        crate::systems::customers::door_visit(world, served, place);
        // Round 4 T4 (OPEN-217): the room of the serving bodies on duty.
        crate::systems::vice::night_room(world, place);
    }
    // Round 7 W6 (OPEN-217): the Clinic's day of patients.
    if kind == ServeKind::Patient {
        crate::systems::care::note_seen(world, place);
    }
    let hour = world.tick / TICKS_PER_HOUR;
    // OPEN-111: the hash is the pair's, not the hour's, so a patron gets
    // the same server whenever that server is on duty with room (a regular
    // accumulates on one ledger); still never the lowest id (D6).
    match pick_server(world, place, kind, served) {
        Some(server) => {
            let r = world.service_room.entry(server).or_default();
            if r.hour != hour {
                *r = Room { hour, taken: 0 };
            }
            r.taken += 1;
            if kind == ServeKind::Patient && crate::systems::presence::is_body(world, server) {
                ownership::jobs2_row(world).clinic_body += 1;
            }
            serve(world, server, served, place, kind, coins);
        }
        None => ownership::jobs2_row(world).served_waited += 1,
    }
}

impl Outcome {
    fn cfg(self) -> ServeOutcome {
        match self {
            Outcome::Done => ServeOutcome::Done,
            Outcome::Refused => ServeOutcome::Refused,
            Outcome::Ejected => ServeOutcome::Ejected,
            Outcome::Regular => ServeOutcome::Regular,
            Outcome::Notice => ServeOutcome::Notice,
        }
    }
}
