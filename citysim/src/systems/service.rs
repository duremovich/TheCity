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
    }
    let hour = world.tick / TICKS_PER_HOUR;
    // OPEN-111: the hash is the pair's, not the hour's, so a patron gets
    // the same server whenever that server is on duty with room (a regular
    // accumulates on one ledger); still never the lowest id (D6).
    let pick = servers(world, place, kind)
        .into_iter()
        .filter(|&(s, cap)| world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap))
        .map(|(s, _)| (hash(world, PURPOSE_SERVE, key(place) ^ key(served), key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    match pick {
        Some((_, server)) => {
            let r = world.service_room.entry(server).or_default();
            if r.hour != hour {
                *r = Room { hour, taken: 0 };
            }
            r.taken += 1;
            serve(world, server, served, place, kind, coins);
        }
        None => ownership::jobs2_row(world).served_waited += 1,
    }
}

/// The hourly pass (from `living::run`): the room ledger pruned of past
/// hours, then the madam's `Staff` encounters of the hour.
pub fn hourly(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let _t = crate::prof::scope!("service::hourly");
    let hour = world.tick / TICKS_PER_HOUR;
    world.service_room.retain(|_, r| r.hour == hour);
    staff_hour(world);
    if world.tick_of_day() == 0 {
        world.notices_today.clear();
        // The pressure pass R4: a round cut short (the shift's end, a
        // demotion) tells its notices at midnight.
        let open: Vec<EntityId> = world.notice_fold.keys().copied().collect();
        for s in open {
            notices_folded(world, s, None);
        }
    }
    if civic_on(world) {
        civic_hour(world);
    }
    if street_on(world) {
        lift_hour(world);
    }
    if tower_on(world) {
        meeting_hour(world);
        walk_in_hour(world);
        crate::systems::news::stat_beats(world);
    }
}

/// Lives at work phase 4 (§ 2.3, loader): at the last hour of a loader's
/// shift, each loader present at his Yard (a body inside, a Statistical one
/// credited by the clock: § 1.2) lifts the shift's crates (`lift`). One
/// world-side pass for both tiers.
fn lift_hour(world: &mut World) {
    let now = world.tick;
    let mut due = Vec::new();
    for &y in world.buildings_of_kind(BuildingKind::FreightYard) {
        for &s in world.staff_of(y) {
            let last = world
                .comp::<Job>(s)
                .and_then(|j| j.shift_end(now))
                .is_some_and(|e| e > now && e - now <= TICKS_PER_HOUR);
            if last && row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Crate) && present(world, s, y) {
                due.push((y, s));
            }
        }
    }
    for (y, s) in due {
        let n = crate::systems::industry::shift_lifts(world, y);
        lift(world, s, y, n);
    }
}

/// Does `s`'s job start a shift this hour (the roster's and the headcount's
/// hour, as the madam's `staff_hour`; `Job::first_hour`)?
fn first_hour(world: &World, s: EntityId) -> bool {
    let tod = world.tick_of_day();
    world.comp::<Job>(s).is_some_and(|j| j.first_hour(tod))
}

/// Lives at work phase 3 (§ 2.2, teacher and camp warden): at the first
/// hour of a server's shift, the School's roster (every child whose place
/// today is this School, `ages::place_of`, one pass over the children) and
/// the Camp's headcount (its children), one `Pupil`/`Inmate` encounter a
/// child. Children have no `Brain`: no memory on their side, the ledger
/// names them. Civic only.
fn civic_hour(world: &mut World) {
    let schools: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::School)
        .iter()
        .copied()
        .filter(|&s| servers(world, s, ServeKind::Pupil).iter().any(|&(t, _)| first_hour(world, t)))
        .collect();
    if !schools.is_empty() {
        let mut roster: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
        // scan-ok: once a day per School shift: the children
        for k in world.with::<Child>() {
            if !world.is_alive(k) || world.has::<Corpse>(k) {
                continue;
            }
            if let Some(crate::systems::ages::Place::School(s)) = crate::systems::ages::place_of(world, k) {
                if schools.contains(&s) {
                    roster.entry(s).or_default().push(k);
                }
            }
        }
        let hour = world.tick / TICKS_PER_HOUR;
        for (s, kids) in roster {
            let n = kids.len();
            for k in kids {
                arrive(world, k, s, ServeKind::Pupil, 0);
            }
            // S6 (OPEN-224): the class, one event a teacher (the pupils
            // taught this hour; the rest of the roster waited, untaught).
            let classes: Vec<(EntityId, u32)> = servers(world, s, ServeKind::Pupil)
                .into_iter()
                .filter_map(|(t, _)| {
                    world
                        .service_room
                        .get(&t)
                        .filter(|r| r.hour == hour && r.taken > 0)
                        .map(|r| (t, u32::from(r.taken)))
                })
                .collect();
            let taught: u32 = classes.iter().map(|&(_, m)| m).sum();
            let left = (n as u32).saturating_sub(taught);
            for (t, m) in classes {
                let (who, at) = (world.name_of(t), world.name_of(s));
                let rest = if left > 0 { format!(" ({left} more on the roll, no room)") } else { String::new() };
                let pupils = if m == 1 { "1 pupil".to_string() } else { format!("{m} pupils") };
                world.push_event(EventKind::Served, &[t, s], format!("{who} taught {pupils} at {at}{rest}"));
            }
        }
    }
    for camp in crate::systems::camp::standing_camps(world) {
        if !servers(world, camp, ServeKind::Inmate).iter().any(|&(t, _)| first_hour(world, t)) {
            continue;
        }
        let kids: Vec<EntityId> = crate::systems::camp::children_of(world, camp).to_vec();
        if kids.is_empty() {
            continue;
        }
        ownership::jobs2_row(world).camp_headcounts += 1;
        for k in kids {
            arrive(world, k, camp, ServeKind::Inmate, 0);
        }
    }
}

/// § 2.2 (teacher): a taught School day: `teacher_floor + (1 −
/// teacher_floor) × the teacher's knowledge` onto `Child.edu_taught`
/// (`ages::on_mature` reads it under civic), and the parents who are bodies
/// at home meet the teacher at the door (a first meeting at a hashed
/// `0.1 + 0.3u`, `interacted` after; a `Served` memory each side; no walk).
fn taught(world: &mut World, teacher: EntityId, child: EntityId, valence: f32) {
    let floor = world.config.service.teacher_floor;
    let know = world.comp::<crate::components::Skills>(teacher).map_or(0.0, |s| s.knowledge);
    if let Some(c) = world.comp_mut::<Child>(child) {
        c.edu_taught += floor + (1.0 - floor) * know;
    }
    ownership::jobs2_row(world).taught_days += 1;
    let Some(home) = world.comp::<Household>(child).and_then(|h| h.home) else { return };
    let salience = world.config.service.served_salience;
    for p in crate::systems::camp::parents_of(world, child) {
        let at_home = world.comp::<Position>(p).is_some_and(|x| x.building == Some(home))
            && world.comp::<Brain>(p).is_some_and(|b| b.lod != Lod::Statistical);
        if !at_home || p == teacher {
            continue;
        }
        if world.edge(p, teacher).is_some() {
            crate::systems::social::interacted(world, p, teacher);
        } else {
            let u = hash(world, PURPOSE_REGULAR, key(teacher), key(p));
            crate::systems::social::first_meeting(world, p, teacher, 0.1 + 0.3 * u);
        }
        world.remember(p, MemoryKind::Served, Some(teacher), salience, valence, false);
        world.remember(teacher, MemoryKind::Served, Some(p), salience, valence, false);
    }
}

/// § 2.2 (super): the stop's encounter with a tenant: in arrears, a
/// `Notice` (once a tenant a day; a `Served` memory at −0.3; the event
/// when the row names it); a whole coin of rent due and the purse holding
/// it, the rent settled at the door (`ownership::settle_rent`, no arrears
/// clock: the midnight settle moved earlier, the same `Flow::Rent`).
fn tenant(world: &mut World, server: EntityId, tenant: EntityId, block: EntityId) -> Outcome {
    let Some((arrears, due)) = world.comp::<Household>(tenant).map(|h| (h.arrears, h.rent_due.floor() as i64)) else {
        return Outcome::Done;
    };
    let day = world.day();
    let mut outcome = Outcome::Done;
    if arrears > 0 && world.notices_today.insert((tenant, day)) {
        outcome = Outcome::Notice;
        let salience = world.config.service.served_salience;
        let v = world.config.service.tune.notice;
        world.remember(tenant, MemoryKind::Served, Some(server), salience, v, false);
        ownership::jobs2_row(world).notices += 1;
    }
    if due >= 1 && world.purse(Some(tenant)) >= due {
        let before = world.comp::<Household>(tenant).map_or(0.0, |h| h.rent_due);
        ownership::settle_rent(world, tenant, block, false);
        let after = world.comp::<Household>(tenant).map_or(0.0, |h| h.rent_due);
        let paid = (before - after).round() as i64;
        if paid > 0 {
            ownership::jobs2_row(world).door_rent += paid;
        }
    }
    outcome
}

/// § 2.2 (super): the round's arrival at a Block (`trades::round_plan` for
/// a body, `lod::stat_work` for a Statistical super at the shift's first
/// hour): every adult resident but the super is served `Tenant` by the
/// super himself (his row's queue is the round, not his post). Civic only.
pub fn super_stop(world: &mut World, id: EntityId, block: EntityId) {
    if !civic_on(world) || !row_of_agent(world, id).is_some_and(|r| r.kind == ServeKind::Tenant) {
        return;
    }
    ownership::jobs2_row(world).super_stops += 1;
    let tenants: Vec<EntityId> = world
        .residents_of(block)
        .iter()
        .copied()
        .filter(|&t| {
            t != id
                && world.has::<Brain>(t)
                && crate::systems::demography::is_adult(world, t)
                && crate::systems::law::living(world, t)
        })
        .collect();
    // The pre-view pass phase 4 (P3, OPEN-120): only a tenant present and
    // awake is served (a body in the Block, a Statistical one on the home
    // draw); the rest wait for the next round (`presence_missed`).
    let presence = crate::systems::presence::on(world);
    let key = u64::from(id.index);
    // The pressure pass R4 (OPEN-219): the stop's notices go into the
    // round's one line (`notices_folded`), not a line a tenant.
    world.notice_fold.entry(id).or_default();
    for t in tenants {
        if presence && !crate::systems::presence::met_at_home(world, t, block, key) {
            crate::systems::presence::missed(world);
            continue;
        }
        serve(world, id, t, block, ServeKind::Tenant, 0);
    }
}

/// The pressure pass R4 (OPEN-219): the round's one `Notice` line, at its
/// end (`trades::round_plan`, `lod`'s Statistical round; `stops` the
/// round's Blocks) or at midnight for a round cut short (`None`): the count
/// and the first `[noise] notice_names` tenants; every tenant noticed is an
/// actor (his diary keeps the line). Nothing when the round noticed nobody.
pub fn notices_folded(world: &mut World, id: EntityId, stops: Option<usize>) {
    let Some(tenants) = world.notice_fold.remove(&id) else { return };
    if tenants.is_empty() {
        return;
    }
    let names = usize::from(world.config.noise.notice_names);
    let shown: Vec<String> = tenants.iter().take(names).map(|&t| world.name_of(t)).collect();
    let more = tenants.len().saturating_sub(shown.len());
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    let round = match stops {
        Some(n) => format!("the round of {n} Blocks"),
        None => "a round cut short".to_string(),
    };
    let n = tenants.len();
    let who = world.name_of(id);
    let s = if n == 1 { "" } else { "s" };
    let text = format!("{who} served notice on {n} tenant{s} in arrears on {round} ({}{tail})", shown.join(", "));
    let mut actors = vec![id];
    actors.extend(tenants);
    world.push_event(EventKind::Notice, &actors, text);
}

/// § 2.2 (jailer): a breakout past the jailers on duty: a `Served` memory
/// at −0.3 each (subject none), the first named in the `Jailbreak` text
/// (`raid::breach`). Civic only.
pub fn jailers_passed(world: &mut World, jail: EntityId) -> Option<EntityId> {
    if !civic_on(world) {
        return None;
    }
    let on_duty = servers_on_duty(world, jail, ServeKind::Inmate);
    let salience = world.config.service.served_salience;
    let v = world.config.service.tune.jailbreak;
    for &j in &on_duty {
        world.remember(j, MemoryKind::Served, None, salience, v, false);
    }
    on_duty.first().copied()
}

/// § 2.1 (madam): at the first hour of a worker's shift at a place with a
/// `Staff`-kind server on duty, the worker is served (the madam's house).
fn staff_hour(world: &mut World) {
    let tod = world.tick_of_day();
    let places: Vec<EntityId> = world
        .config
        .serves
        .iter()
        .filter(|r| r.kind == ServeKind::Staff)
        .flat_map(|_| world.buildings_of_kind(BuildingKind::Brothel).iter().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for place in places {
        if servers(world, place, ServeKind::Staff).is_empty() {
            continue;
        }
        let workers: Vec<EntityId> = world
            .staff_of(place)
            .iter()
            .copied()
            .filter(|&s| {
                row_of_agent(world, s).is_some_and(|r| r.kind != ServeKind::Staff)
                    && world.comp::<Job>(s).is_some_and(|j| j.first_hour(tod))
                    && present(world, s, place)
            })
            .collect();
        for w in workers {
            arrive(world, w, place, ServeKind::Staff, 0);
        }
    }
}

/// The skill a row trains.
fn train(world: &mut World, server: EntityId, skill: TradeSkill, gain: f32) {
    if gain <= 0.0 {
        return;
    }
    let Some(s) = world.comp_mut::<crate::components::Skills>(server) else { return };
    let slot = match skill {
        TradeSkill::None => return,
        TradeSkill::Farming => &mut s.farming,
        TradeSkill::Fighting => &mut s.fighting,
        TradeSkill::Persuasion => &mut s.persuasion,
        TradeSkill::Knowledge => &mut s.knowledge,
        TradeSkill::Hacking => &mut s.hacking,
    };
    *slot = (*slot + gain).min(1.0);
}

/// One encounter (§ 1.2's five steps) at a place.
pub fn serve(world: &mut World, server: EntityId, served: EntityId, place: EntityId, kind: ServeKind, coins: i64) {
    // The pre-view pass phase 3 (OPEN-145): the device rows, a venue's
    // customers by day and in the 21:00 hour (the Statistical evening).
    if kind == ServeKind::Visit && crate::systems::shift::counter_on(world) {
        let hour = world.tick_of_day() / TICKS_PER_HOUR as u16;
        let row = ownership::jobs2_row(world);
        row.served_day += u32::from((6..18).contains(&hour));
        row.served_21h += u32::from(hour == 21);
    }
    let d = world.district_of_building(place);
    encounter(world, server, served, Some(place), d, kind, coins);
    // The pressure pass R6a (OPEN-217): a Brothel visit is the madam's booking.
    if kind == ServeKind::Visit {
        crate::systems::vice::booked(world, server, served, place, coins);
        // Round 3 S5 (OPEN-217): a rough client at a kept door, the bouncer's.
        crate::systems::customers::rough_at_door(world, server, served, place);
    }
}

/// Lives at work phase 4 (§ 2.3): the encounter, at a place or (a street
/// pitch, a corner, a rickshaw's stand: a tile, not a Building) in a
/// district alone; the event names the district when there is no place.
/// `serve` is the placed form. A server with no trade row (the pimp, the
/// dealer) is read by its gang post's row (`row_of_agent`).
pub fn encounter(
    world: &mut World,
    server: EntityId,
    served: EntityId,
    place: Option<EntityId>,
    district: crate::components::DistrictId,
    kind: ServeKind,
    coins: i64,
) {
    let Some(row) = row_of_agent(world, server).cloned() else { return };
    encounter_row(world, row, server, served, place, district, kind, coins);
}

/// Lives at work phase 4b (§ 2.3): the encounter of a gang post with no
/// trade row, read by its `[[serves]]` key (`"pimp"`, `"dealer"`): a gang
/// member holding a Job meets his collection or his buyer as the post, not
/// as his trade. Nothing when the key has no row.
#[allow(clippy::too_many_arguments)]
pub fn encounter_post(
    world: &mut World,
    post: &str,
    server: EntityId,
    served: EntityId,
    place: Option<EntityId>,
    district: crate::components::DistrictId,
    kind: ServeKind,
    coins: i64,
) {
    let Some(row) = world.config.serves.iter().find(|r| r.key == post).cloned() else { return };
    encounter_row(world, row, server, served, place, district, kind, coins);
}

#[allow(clippy::too_many_arguments)]
fn encounter_row(
    world: &mut World,
    row: ServesCfg,
    server: EntityId,
    served: EntityId,
    place: Option<EntityId>,
    district: crate::components::DistrictId,
    kind: ServeKind,
    coins: i64,
) {
    let kind_label = kind.label();
    let place_kind = place.and_then(|b| world.comp::<Building>(b)).map(|b| b.kind);
    // 1. The arm: the counters and the re-routed coins.
    {
        let r = ownership::jobs2_row(world);
        r.served[kind as usize] += 1;
        if kind == ServeKind::Visit && place_kind == Some(BuildingKind::NoodleBar) {
            r.cooked += 1;
        }
    }
    let salience = world.config.service.served_salience;
    // The tip (§ 2.1): to the server when its trade row is tipped, else to
    // the tipped worker on duty the hash names (a Club's Host serves the
    // visit, the dancer is tipped: the flow main paid, re-routed, D4).
    let tipped =
        if row_tipped(world, &row.key) { Some(server) } else { place.and_then(|b| tipped_pick(world, b, served)) };
    if kind == ServeKind::Visit && coins > 0 {
        if let Some(t) = tipped.filter(|&t| t != served) {
            let tip = ((coins as f32 * world.config.vice.tip_share).round() as i64).max(1);
            let paid = ownership::pay(world, Some(served), Some(t), tip, Flow::Tip);
            // § 2.1, 8d (OPEN-123): a tip at her own house is noted on the
            // worker's shift; the madam's cut of the total settles once at
            // `shift_end` (per tip, 0.3 of a 1 c tip rounded to nothing).
            if let (true, Some(BuildingKind::Brothel), Some(place)) = (paid > 0, place_kind, place) {
                if let Some(j) = world.comp_mut::<Job>(t).filter(|j| j.employer == Some(place)) {
                    j.shift_tips += paid;
                }
            }
        }
    }
    // Lives at work phase 3 (§ 2.2): the civic arms.
    let mut outcome = Outcome::Done;
    match (kind, place) {
        (ServeKind::Pupil, Some(_)) => taught(world, server, served, row.valence),
        (ServeKind::Tenant, Some(p)) => outcome = tenant(world, server, served, p),
        _ => {}
    }
    // 2. Memories, bodies only (`remember` drops a Statistical agent's and a chip's).
    // The pressure pass R4 (OPEN-219): a super's call at a door with nothing
    // in it (the row's zero valence, no notice: `tenant` wrote the notice's
    // own) is no memory either side (V10: ~40 "called on" lines a round).
    let blank_call = kind == ServeKind::Tenant && row.valence == 0.0;
    // The pressure pass S6 (OPEN-224): a pupil is no memory of the
    // teacher's (a child has none of its own; a roster of 30 a morning
    // buried the teacher's memory, and the diary read the pupil as "a
    // pupil's parent"): the day's class is one event (`civic_hour`).
    let pupil = kind == ServeKind::Pupil && world.has::<Child>(served);
    if !blank_call && !pupil {
        world.remember(server, MemoryKind::Served, Some(served), salience, row.valence, false);
        world.remember(served, MemoryKind::Served, Some(server), salience, row.valence, false);
    }
    // 3. The regulars ledger and the edge.
    let regular = note_regular(world, server, served, &row);
    // 4. The skill.
    train(world, server, row.skill, row.skill_gain);
    // 5. The event.
    if regular {
        outcome = Outcome::Regular;
    }
    if row.event_on.contains(&outcome.cfg()) {
        // The pressure pass R4: a notice on the round is folded into its line.
        if outcome == Outcome::Notice {
            if let Some(f) = world.notice_fold.get_mut(&server) {
                f.push(served);
                return;
            }
        }
        let (who, whom) = (world.name_of(server), world.name_of(served));
        let at = match place {
            Some(p) => world.name_of(p),
            None => format!("a street in {}", world.district_name(district)),
        };
        let (kind, text) = match outcome {
            Outcome::Regular => (EventKind::Served, format!("{whom} is now a regular of {who} at {at}")),
            Outcome::Notice => (EventKind::Notice, format!("{who} served notice on {whom} at {at}")),
            _ => (EventKind::Served, format!("{who} served {whom} ({kind_label}) at {at}")),
        };
        let actors: Vec<EntityId> = [Some(server), Some(served), place].into_iter().flatten().collect();
        world.push_event(kind, &actors, text);
    }
}

/// The ledger: `visits + 1` within `regular_days`, else a fresh count; at
/// exactly `regular_after` with no edge, an Acquaintance at a hashed
/// affinity (never the world stream); with an edge, `interacted`.
fn note_regular(world: &mut World, server: EntityId, served: EntityId, row: &ServesCfg) -> bool {
    let day = world.day();
    let today = u16::try_from(day).unwrap_or(u16::MAX);
    let window = u16::from(row.regular_days);
    let list = world.regulars.entry(server).or_default();
    let visits = match list.iter_mut().find(|r| r.who == served) {
        Some(r) => {
            r.visits = if today.saturating_sub(r.last_day) > window { 1 } else { r.visits.saturating_add(1) };
            r.last_day = today;
            r.visits
        }
        None => {
            if list.len() >= REGULARS_CAP {
                // The oldest falls off (ties: the first).
                let (i, _) = list.iter().enumerate().min_by_key(|(i, r)| (r.last_day, *i)).expect("non-empty");
                list.remove(i);
            }
            list.push(Regular { who: served, visits: 1, last_day: today });
            1
        }
    };
    if world.edge(server, served).is_some() {
        crate::systems::social::interacted(world, server, served);
        return false;
    }
    if row.regular_after > 0 && visits == row.regular_after {
        // The affinity of a regular's acquaintance: a hashed `0.1..0.4`.
        let u = hash(world, PURPOSE_REGULAR, key(server), key(served));
        crate::systems::social::first_meeting(world, server, served, 0.1 + 0.3 * u);
        ownership::jobs2_row(world).regulars_made += 1;
        return true;
    }
    false
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

/// The door staff of `b` on duty (a fighting trade on shift, present).
fn door_staff(world: &World, b: EntityId) -> Vec<EntityId> {
    crate::systems::trades::door_staff(world, b).into_iter().filter(|&s| present(world, s, b)).collect()
}

/// § 2.1 (bouncer): `id` was turned away at `b`'s door (the exec's failed
/// start of a venue step inside it): `Refused`, an event when the bouncer's
/// row names it, a memory each side, the counter. Nothing off.
pub fn refused(world: &mut World, id: EntityId, b: EntityId) {
    if !on(world) {
        return;
    }
    ownership::jobs2_row(world).door_refusals += 1;
    // The pre-view pass phase 3 (OPEN-121): turned away, the patron's next
    // pick passes this door over for `[exec] refused_ticks` (`leisure::door_seen`).
    if crate::systems::shift::counter_on(world) {
        let tick = world.tick;
        if let Some(br) = world.comp_mut::<crate::components::Brain>(id) {
            br.refused = Some((b, tick));
        }
    }
    let staff = door_staff(world, b);
    if staff.is_empty() {
        return;
    }
    let u = hash(world, PURPOSE_DOOR, key(id), world.tick / TICKS_PER_HOUR);
    let bouncer = staff[((u * staff.len() as f32) as usize).min(staff.len() - 1)];
    let Some(row) = row_of_agent(world, bouncer).cloned() else { return };
    let salience = world.config.service.served_salience;
    let (vb, vr) = (world.config.service.tune.refused_bouncer, world.config.service.tune.refused);
    world.remember(bouncer, MemoryKind::Served, Some(id), salience, vb, false);
    world.remember(id, MemoryKind::Served, Some(bouncer), salience, vr, false);
    // The pressure pass phase 2 (OPEN-176): the cause named, and the scene
    // a drunk or a brawler makes of it.
    let why = crate::systems::customers::cause(world, id, b);
    if why == Some("barred") {
        ownership::jobs2_row(world).door_barred += 1;
    }
    if world.config.verbs.is_some() {
        let c = why.unwrap_or("no cause").to_string();
        crate::systems::vice::door_note(world, bouncer, b, |n| {
            n.refused += 1;
            match n.causes.iter_mut().find(|(k, _)| *k == c) {
                Some((_, k)) => *k += 1,
                None => n.causes.push((c, 1)),
            }
        });
    }
    if row.event_on.contains(&ServeOutcome::Refused) {
        let because = why.map(|w| format!(" ({w})")).unwrap_or_default();
        let text = format!(
            "{} turned {} away at the door of {}{because}",
            world.name_of(bouncer),
            world.name_of(id),
            world.name_of(b)
        );
        world.push_event(EventKind::Served, &[bouncer, id, b], text);
    }
    if let Some(line) = why.and_then(|w| crate::systems::customers::scene(world, id, bouncer, b, w)) {
        world.push_event(EventKind::Brawl, &[id, bouncer, b], line);
    }
}

/// § 2.1 (bouncer): an Assault raised inside a kept venue on a door shift
/// is `Ejected` on `p_eject × (bouncer fighting − attacker fighting)`: the
/// attacker's plan aborted and the attacker put at the door. Returns
/// whether it was thrown out. Nothing off.
pub fn maybe_eject(world: &mut World, attacker: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(b) = world.comp::<Position>(attacker).and_then(|p| p.building) else { return false };
    let staff = door_staff(world, b);
    let Some((bouncer, strength)) = staff
        .iter()
        .map(|&s| (s, crate::systems::law::fighting(world, s)))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
    else {
        return false;
    };
    let edge = (strength - crate::systems::law::fighting(world, attacker)).max(0.0);
    let p = world.config.service.p_eject * edge;
    if hash(world, PURPOSE_EJECT, key(attacker), world.tick) >= p {
        return false;
    }
    world.abort_plan(attacker);
    world.leave_building(attacker);
    ownership::jobs2_row(world).ejections += 1;
    if world.config.verbs.is_some() {
        crate::systems::vice::door_note(world, bouncer, b, |n| n.thrown += 1);
    }
    let salience = world.config.service.served_salience;
    let (vb, va) = (world.config.service.tune.eject_bouncer, world.config.service.tune.ejected);
    world.remember(bouncer, MemoryKind::Served, Some(attacker), salience, vb, false);
    world.remember(attacker, MemoryKind::Served, Some(bouncer), salience, va, false);
    let names_it = row_of_agent(world, bouncer).is_some_and(|r| r.event_on.contains(&ServeOutcome::Ejected));
    if names_it {
        let text = format!("{} threw {} out of {}", world.name_of(bouncer), world.name_of(attacker), world.name_of(b));
        world.push_event(EventKind::Served, &[bouncer, attacker, b], text);
    }
    true
}

/// The world-side shift-end rule (§ 2.1, cook; both tiers: `exec::actions::
/// end_shift` and `lod::stat_work`): a staffed shift of a serving row is
/// counted, and a Cook takes one staff meal from the NoodleBar's stock.
/// Nothing off.
pub fn shift_end(world: &mut World, id: EntityId) {
    if !on(world) {
        return;
    }
    let Some(j) = world.comp::<Job>(id).cloned() else { return };
    if row_for(world, j.role).is_some() {
        ownership::jobs2_row(world).serve_shifts += 1;
    }
    if j.shift_tips > 0 {
        madam_settles(world, id, &j);
    }
    // R6a: the madam's night of bookings, one line.
    crate::systems::vice::bookings_told(world, id);
    // Round 3 S5: the bouncer's night at the door, one line.
    crate::systems::vice::door_told(world, id);
    // Lives at work phase 4 (§ 2.3): the courier's undelivered parcels back
    // to the Yard (the loader's lifts are `lift_hour`'s).
    if street_on(world) {
        crate::systems::streetwork::parcels_back(world, id);
    }
    if !world.config.service.staff_meal || j.role != Role::Cook {
        return;
    }
    let Some(bar) =
        j.employer.filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::NoodleBar))
    else {
        return;
    };
    if world.take_stock(bar, crate::components::Good::Food, 1) == 0 {
        return;
    }
    let cfg = world.config.needs.clone();
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    crate::systems::health::ate(world, id, crate::components::MealKind::Real);
    world.mark_day(id, crate::components::trace_flags::ATE);
    ownership::jobs2_row(world).staff_meals += 1;
}

/// 8d (OPEN-123, § 2.1): the madam's cut settles at the worker's shift end,
/// `[service] madam_cut` of the shift's tips at the house, rounded once, to
/// the house's madam on duty (none on duty, or the worker herself: no cut).
/// The tally clears either way. Both tiers: a Statistical worker's tips are
/// paid by the same `encounter_row` and her shift ends in `lod::stat_work`.
fn madam_settles(world: &mut World, id: EntityId, j: &Job) {
    if let Some(job) = world.comp_mut::<Job>(id) {
        job.shift_tips = 0;
    }
    let Some(place) = j.employer else { return };
    // The house's madam on duty, else any madam it employs (the cut is the
    // house's take; her shift can end before her worker's).
    let madam = servers(world, place, ServeKind::Staff).first().map(|&(m, _)| m).or_else(|| {
        world
            .staff_of(place)
            .iter()
            .copied()
            .find(|&s| row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Staff))
    });
    let Some(m) = madam.filter(|&m| m != id) else { return };
    let cut = (j.shift_tips as f32 * world.config.service.madam_cut).round() as i64;
    if cut > 0 {
        ownership::pay(world, Some(id), Some(m), cut, Flow::MadamCut);
    }
}

/// Lives at work phase 4 (§ 2.3, loader; `lift_hour`): the shift's `n`
/// lifts at `yard` are the loader's (`Crate`, no served person: `arrive` cannot place
/// one): `served[Crate]` and `lifts` by `n`, one `Served` memory with no
/// subject (the diary's "loaded crates for <owner>"), the skill. Street only.
pub fn lift(world: &mut World, loader: EntityId, yard: EntityId, n: u32) {
    if !street_on(world) || n == 0 || !world.has::<Building>(yard) {
        return;
    }
    let Some(row) = row_of_agent(world, loader).cloned() else { return };
    {
        let r = ownership::jobs2_row(world);
        r.served[ServeKind::Crate as usize] += n;
        r.lifts += n;
    }
    let salience = world.config.service.served_salience;
    world.remember(loader, MemoryKind::Served, None, salience, row.valence, false);
    train(world, loader, row.skill, row.skill_gain * n as f32);
}

/// The regulars kept for `server` (the inspector, the diary).
pub fn regulars_of(world: &World, server: EntityId) -> &[Regular] {
    world.regulars.get(&server).map_or(&[], SmallVec::as_slice)
}

// ---------------------------------------------------------------------------
// Lives at work phase 5 (§ 2.4, § 2.2's civic clerk): the tower and the brokers
// ---------------------------------------------------------------------------

/// A paper buyer queued at midnight for a Fixer's open hours
/// (`World::walk_ins`, saved while pending): the forger the midnight pass
/// named (paid as `gray::papers_daily` paid him), the tier and the day queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkIn {
    pub buyer: EntityId,
    pub fixer: EntityId,
    pub forger: EntityId,
    pub tier: u8,
    pub day: u64,
    /// The pre-view pass phase 4 (P3, OPEN-119): the order is paid at the
    /// handover, not at midnight (written only when set).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unpaid: bool,
}

/// Is `id` a graded member of staff (its post graded, or its base grade)?
fn graded(world: &World, id: EntityId) -> bool {
    world
        .comp::<Job>(id)
        .is_some_and(|j| j.grade >= 1 || crate::systems::classpay::base_grade(world, j.employer, j.role) >= 1)
}

/// § 6: the exec at the table: alive and free; a body inside the Hq, a
/// Statistical exec credited in the office hours (the caller's clock).
fn exec_present(world: &World, exec: EntityId, hq: EntityId) -> bool {
    if !crate::systems::law::living(world, exec) || world.has::<Sentence>(exec) || world.has::<Job>(exec) {
        return false;
    }
    match world.comp::<Brain>(exec).map(|b| b.lod) {
        Some(Lod::Statistical) => true,
        Some(_) => world.comp::<Position>(exec).is_some_and(|p| p.building == Some(hq)),
        None => false,
    }
}

/// § 2.4 (exec): each hour of the office shift on a workday, each corp's
/// exec at its Hq (`life::hq_of`) meets up to `meeting_attendees` of the
/// graded staff present there (the lowest `hash(PURPOSE_TABLE, exec,
/// staff, hour)`): an `Attendee` encounter through the `exec` row, both
/// ways. The table is kept for the decision (`World::meeting_tables`).
fn meeting_hour(world: &mut World) {
    let tod = world.tick_of_day();
    let (s, e) = world.config.life.exec_shift;
    if tod < s || tod >= e || !crate::exec::routine::is_workday(world.day() as i64) {
        return;
    }
    let hour = world.tick / TICKS_PER_HOUR;
    let cap = usize::from(world.config.service.meeting_attendees);
    for corp in world.corps() {
        let Some(exec) = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec) else { continue };
        let Some(hq) = crate::systems::life::office_of(world, corp, exec) else { continue };
        if !exec_present(world, exec, hq) {
            continue;
        }
        let mut staff: Vec<(f32, EntityId)> = world
            .staff_of(hq)
            .iter()
            .copied()
            .filter(|&a| a != exec && graded(world, a) && present(world, a, hq))
            .map(|a| (hash(world, PURPOSE_TABLE, key(exec) ^ key(a), hour), a))
            .collect();
        staff.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        staff.truncate(cap);
        if staff.is_empty() {
            continue;
        }
        {
            let r = ownership::jobs2_row(world);
            r.meetings_held += 1;
            r.meeting_attendees += staff.len() as u32;
        }
        let d = world.district_of_building(hq);
        for &(_, a) in &staff {
            encounter_post(world, "exec", exec, a, Some(hq), d, ServeKind::Attendee, 0);
        }
        world.meeting_tables.insert(corp, staff.into_iter().map(|(_, a)| a).collect());
    }
}

/// § 2.4 (exec), from `corp_brain::rescore` after `attention::tilt`: when
/// the two best orders lie within `decision_margin` and one is expansive,
/// the other defensive, the corp's exec (alive and free) picks: the
/// expansive on `hash(PURPOSE_DECIDE, exec, day) < 0.5 + 0.5 × (courage −
/// loyalty)`, else the defensive; the picked one gains the margin and the
/// scores are re-sorted (stable, best first). A hash of the exec's traits,
/// the same in any tier (§ 6). Returns the exec when he made the call.
pub fn decide(world: &World, corp: EntityId, scores: &mut [crate::components::CorpOrderScore]) -> Option<EntityId> {
    if !tower_on(world) || scores.len() < 2 {
        return None;
    }
    let cfg = &world.config.service;
    let margin = cfg.decision_margin;
    if margin <= 0.0 || scores[0].score - scores[1].score > margin {
        return None;
    }
    let class = |o: crate::components::CorpOrder| {
        if cfg.expansive.contains(&o) {
            Some(true)
        } else if cfg.defensive.contains(&o) {
            Some(false)
        } else {
            None
        }
    };
    let (exp, def) = match (class(scores[0].order), class(scores[1].order)) {
        (Some(true), Some(false)) => (0, 1),
        (Some(false), Some(true)) => (1, 0),
        _ => return None,
    };
    let exec = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec)?;
    if !crate::systems::law::living(world, exec) || world.has::<Sentence>(exec) {
        return None;
    }
    let p = world
        .comp::<crate::components::Personality>(exec)
        .map_or(0.5, |t| (0.5 + 0.5 * (t.courage - t.loyalty)).clamp(0.0, 1.0));
    let pick = if hash(world, PURPOSE_DECIDE, key(exec), world.day()) < p { exp } else { def };
    scores[pick].score += margin;
    scores.sort_by(|a, b| b.score.total_cmp(&a.score));
    Some(exec)
}

/// § 2.4 (exec): the call changed the corp's outcome: `decisions`, an
/// `EventKind::Served` naming the table (the corp's last Meeting), each
/// attendee's `Served` memory of the exec and the `Decided` deed posted
/// (§ 3.6: the attendees' gossip).
pub fn decided(world: &mut World, corp: EntityId, exec: EntityId, what: String) {
    ownership::jobs2_row(world).decisions += 1;
    let table: Vec<EntityId> = world
        .meeting_tables
        .get(&corp)
        .map(|t| t.iter().copied().filter(|&a| crate::systems::law::living(world, a)).collect())
        .unwrap_or_default();
    let names: Vec<String> = table.iter().map(|&a| world.name_of(a)).collect();
    let at = if names.is_empty() { "nobody".to_string() } else { names.join(", ") };
    let corp_name = world.comp::<crate::components::Corp>(corp).map_or_else(|| world.name_of(corp), |c| c.name.clone());
    let text = format!("{} decided {corp_name} would {what}; at the table: {at}", world.name_of(exec));
    let mut actors = vec![exec, corp];
    actors.extend(table.iter().copied());
    world.push_event(EventKind::Served, &actors, text);
    let salience = world.config.service.served_salience;
    for &a in &table {
        world.remember(a, MemoryKind::Served, Some(exec), salience, 0.0, false);
    }
    let d =
        crate::systems::life::office_of(world, corp, exec).map(|h| world.district_of_building(h)).unwrap_or_default();
    crate::systems::gossip::post_deed(world, d, crate::word::Deed::Decided, Some(exec), Some(corp));
}

/// The Fixer's open hours now: a member of its staff on shift this hour on
/// a workday (the forger's and the Fixer's day shift).
fn fixer_open_now(world: &World, fixer: EntityId) -> bool {
    let tod = world.tick_of_day();
    world.staff_of(fixer).iter().any(|&s| {
        world.comp::<Job>(s).is_some_and(|j| {
            j.on_shift(tod) && crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick))
        })
    })
}

/// § 2.4 (fixer): the owner on duty without a post: alive, free, holding
/// no Job; a body inside his Fixer, a Statistical owner credited in its
/// open hours (deviation, as the pimp's: § 6's `shift_attended` needs a Job).
fn owner_present(world: &World, fixer: EntityId) -> Option<EntityId> {
    let o = world.owner_of(fixer)?;
    if !world.has::<Brain>(o)
        || world.has::<Job>(o)
        || world.has::<Sentence>(o)
        || !crate::systems::law::living(world, o)
    {
        return None;
    }
    let here = match world.comp::<Brain>(o).map(|b| b.lod) {
        Some(Lod::Statistical) => fixer_open_now(world, fixer),
        _ => world.comp::<Position>(o).is_some_and(|p| p.building == Some(fixer)),
    };
    here.then_some(o)
}

/// § 2.4 (fixer): who serves `served` at `fixer` now: the on-duty `Client`
/// servers (the Fixer's staff, the forger) with room this hour by the
/// lowest `hash(PURPOSE_FIXER, …)` (D6), else the owner present. `None`:
/// nobody on duty.
fn fixer_server(world: &World, fixer: EntityId, served: EntityId) -> Option<EntityId> {
    let hour = world.tick / TICKS_PER_HOUR;
    servers(world, fixer, ServeKind::Client)
        .into_iter()
        .filter(|&(s, cap)| s != served && world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap))
        .map(|(s, _)| (hash(world, PURPOSE_FIXER, key(fixer) ^ key(served), hour ^ key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, s)| s)
        .or_else(|| owner_present(world, fixer).filter(|&o| o != served))
}

/// The fixer a party off the hours is credited to (§ 2.4: "credited
/// world-side at that hour"; the posts come at midnight and the takes at
/// the match hour, outside the day shift): the Fixer's `Client` staff
/// alive and free by the lowest `hash(PURPOSE_FIXER, …)`, else the owner
/// alive and free. Review fix 7: a staff server with room left this hour
/// (his row's `per_hour`, as the on-duty pick reads it); all full, the
/// owner (whose post has no room ledger), else nobody (the contract
/// stands, the encounter is not credited).
fn fixer_book(world: &World, fixer: EntityId, party: EntityId) -> Option<EntityId> {
    let hour = world.tick / TICKS_PER_HOUR;
    let free = |s: EntityId| s != party && crate::systems::law::living(world, s) && !world.has::<Sentence>(s);
    let room = |s: EntityId, cap: u8| world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap);
    world
        .staff_of(fixer)
        .iter()
        .copied()
        .filter(|&s| {
            free(s) && row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Client && room(s, r.per_hour))
        })
        .map(|s| (hash(world, PURPOSE_FIXER, key(fixer) ^ key(party), hour ^ key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, s)| s)
        // The owner on his own post here is one of the staff above (full).
        .or_else(|| {
            world.owner_of(fixer).filter(|&o| {
                world.has::<Brain>(o) && free(o) && world.comp::<Job>(o).is_none_or(|j| j.employer != Some(fixer))
            })
        })
}

/// The `Client` encounter at the Fixer: a staff server through his row
/// (the room counted), the owner through the `Fixer` row.
fn fixer_meet(world: &mut World, server: EntityId, served: EntityId, fixer: EntityId, coins: i64) {
    let d = world.district_of_building(fixer);
    let staff = world.comp::<Job>(server).is_some_and(|j| j.employer == Some(fixer));
    if staff {
        let hour = world.tick / TICKS_PER_HOUR;
        let r = world.service_room.entry(server).or_default();
        if r.hour != hour {
            *r = Room { hour, taken: 0 };
        }
        // Both pickers leave room under the cap; saturating, never wrapping.
        r.taken = r.taken.saturating_add(1);
        encounter(world, server, served, Some(fixer), d, ServeKind::Client, coins);
    } else {
        encounter_post(world, "Fixer", server, served, Some(fixer), d, ServeKind::Client, coins);
    }
}

/// § 2.4 (fixer, `contracts::post_with` and `accept`): a brokered post or
/// take is a `Client` encounter with the fixer on duty (`contract_encounters`):
/// a body inside the Fixer meets him there, a party elsewhere (an order, a
/// coerced post, a Statistical take) is credited world-side at the hour, to
/// the fixer on duty or, off the open hours (the corps post at midnight,
/// the takes come at the match hour), to the Fixer's book ([`fixer_book`]).
/// Tower only.
pub fn contract_party(world: &mut World, party: EntityId, fixer: EntityId) {
    contract_party_at(world, party, fixer, true);
}

/// The pressure pass phase 3 (OPEN-157): a contract's party sent to the
/// Fixer in its open hours (`VisitWhy::Broker`) is there: the encounter now
/// if a server is on duty (else it is missed, not sent again).
pub fn broker_arrived(world: &mut World, party: EntityId, fixer: EntityId) {
    ownership::jobs2_row(world).broker_walks += 1;
    contract_party_at(world, party, fixer, false);
}

/// OPEN-157: send `party` to `fixer` in person at its next open hour today
/// (the staff's first shift hour, or the next hour if later), when the
/// visits are on and the party has no other visit due. `false`: not sent.
fn send_broker(world: &mut World, party: EntityId, fixer: EntityId) -> bool {
    if !world.config.protection.as_ref().is_some_and(|c| c.broker_visits)
        || !crate::systems::household::on(world)
        || world.visits.contains_key(&party)
        || world.comp::<Building>(fixer).is_none_or(|b| b.demolished || b.derelict)
    {
        return false;
    }
    let Some(open) = fixer_first_hour(world, fixer) else { return false };
    let now = u8::try_from(u64::from(world.tick_of_day()) / TICKS_PER_HOUR).unwrap_or(23);
    let hour = open.max(now.saturating_add(1));
    if hour >= 23 {
        return false;
    }
    let v =
        crate::systems::household::PendingVisit { kin: fixer, why: crate::systems::household::VisitWhy::Broker, hour };
    world.visits.insert(party, v);
    true
}

fn contract_party_at(world: &mut World, party: EntityId, fixer: EntityId, may_send: bool) {
    if !tower_on(world) || party == EntityId::NONE || !world.has::<Brain>(party) {
        return;
    }
    // The pre-view pass phase 4 (P3): the encounter needs both parties: a
    // server on duty in the Fixer and a body party inside it (a Statistical
    // one is credited at the hour); the book credit off the hours only
    // between two Statistical parties (the mirror), never to a body asleep.
    if crate::systems::presence::on(world) {
        let body = |x: EntityId| crate::systems::presence::is_body(world, x);
        let here = !body(party) || crate::systems::presence::body_at(world, party, fixer);
        let server = fixer_server(world, fixer, party)
            .filter(|_| here)
            .or_else(|| fixer_book(world, fixer, party).filter(|&s| !body(s) && !body(party)));
        let Some(server) = server else {
            // The pressure pass phase 3 (OPEN-157): the party comes in
            // person in the open hours (both tiers: a Statistical party is
            // credited at the visit's hour).
            if may_send && send_broker(world, party, fixer) {
                return;
            }
            crate::systems::presence::missed(world);
            return;
        };
        ownership::jobs2_row(world).contract_encounters += 1;
        fixer_meet(world, server, party, fixer, 0);
        return;
    }
    let Some(server) = fixer_server(world, fixer, party).or_else(|| fixer_book(world, fixer, party)) else { return };
    ownership::jobs2_row(world).contract_encounters += 1;
    fixer_meet(world, server, party, fixer, 0);
}

/// Is the walk-in's buyer walking to the Fixer (a body with the `Papers`
/// visit pending)? Its sale is made at the arrival ([`walk_in_arrived`]).
fn walking_in(world: &World, w: &WalkIn) -> bool {
    world.comp::<Brain>(w.buyer).is_some_and(|b| b.lod != Lod::Statistical)
        && world
            .visits
            .get(&w.buyer)
            .is_some_and(|v| v.kin == w.fixer && v.why == crate::systems::household::VisitWhy::Papers)
}

/// The walk-in's pickup, if a server is on duty: the order was paid at
/// midnight (`gray::papers_daily`: the coins and `fake_sold` as main's, so
/// the money circuit moves at main's tick); now `Papers` are inserted (their
/// protection starts here), an encounter with the server and a `Served`
/// event. `Some(true)` handed over, `Some(false)` nobody on duty (it
/// waits), `None` the pickup is off (the buyer gone, jailed or papered, the
/// Fixer gone: `walk_ins_dropped`; the forger keeps the coins).
fn walk_in_sale(world: &mut World, w: WalkIn) -> Option<bool> {
    if !crate::systems::law::living(world, w.buyer)
        || world.has::<Sentence>(w.buyer)
        || world.jobs2.gray.papers.contains_key(&w.buyer)
        || world.comp::<Building>(w.fixer).is_none_or(|b| b.demolished || b.derelict)
    {
        ownership::jobs2_row(world).walk_ins_dropped += 1;
        return None;
    }
    // The pre-view pass phase 4 (P3): a body buyer is handed his papers in
    // the Fixer, not across the city (he waits for his walk).
    if crate::systems::presence::on(world)
        && crate::systems::presence::is_body(world, w.buyer)
        && !crate::systems::presence::body_at(world, w.buyer, w.fixer)
    {
        return Some(false);
    }
    let Some(server) = fixer_server(world, w.fixer, w.buyer) else { return Some(false) };
    let tier = w.tier.max(1);
    let price = world.config.gray.id_price[usize::from(tier - 1)];
    let forger = if crate::systems::law::living(world, w.forger) { w.forger } else { server };
    // The pre-view pass phase 4 (P3, OPEN-119): paid here, at the handover:
    // the forger's share to him, the owner's cut to the server, who holds it
    // for the owner's collection in person (`collected`).
    if w.unpaid {
        if world.purse(Some(w.buyer)) < price {
            ownership::jobs2_row(world).walk_ins_dropped += 1;
            return None;
        }
        let cut = (price as f32 * world.config.gray.fixer_cut).round() as i64;
        ownership::pay(world, Some(w.buyer), Some(forger), price - cut, Flow::Gray);
        let held = ownership::pay(world, Some(w.buyer), Some(server), cut, Flow::Gray);
        if held > 0 {
            world.fixer_owed.entry(w.fixer).or_default().push((server, held));
        }
        let row = ownership::jobs2_row(world);
        row.fake_sold += 1;
        row.papers_handover += 1;
    }
    world.jobs2.gray.papers.insert(w.buyer, crate::systems::gray::Papers { tier, fake: true, forger: Some(forger) });
    ownership::jobs2_row(world).walk_ins_served += 1;
    fixer_meet(world, server, w.buyer, w.fixer, price);
    let text =
        format!("{} handed {} papers at {}", world.name_of(server), world.name_of(w.buyer), world.name_of(w.fixer));
    world.push_event(EventKind::Served, &[server, w.buyer, w.fixer], text);
    Some(true)
}

/// § 2.4 (fixer): each hour, the walk-ins not walking are served by the
/// Fixer's server on duty (a Statistical buyer credited at the hour a
/// body's walk would land in: the Fixer's open hours); unserved, they wait.
fn walk_in_hour(world: &mut World) {
    if world.walk_ins.is_empty() {
        return;
    }
    let pending = std::mem::take(&mut world.walk_ins);
    let mut keep = Vec::with_capacity(pending.len());
    for w in pending {
        if walking_in(world, &w) {
            keep.push(w);
            continue;
        }
        if walk_in_sale(world, w) == Some(false) {
            keep.push(w);
        }
    }
    keep.append(&mut world.walk_ins);
    world.walk_ins = keep;
}

/// § 2.4 (fixer, `household::credit`): the walking buyer is at the Fixer:
/// the sale now, if a server is on duty (else the walk-in waits for the
/// hour's pass, the walk spent).
pub fn walk_in_arrived(world: &mut World, buyer: EntityId, fixer: EntityId) {
    let Some(w) = world.walk_ins.iter().copied().find(|w| w.buyer == buyer && w.fixer == fixer) else { return };
    if walk_in_sale(world, w) != Some(false) {
        world.walk_ins.retain(|x| x.buyer != buyer);
    }
}

/// § 2.4 (fixer, `gray::papers_daily` at midnight, the order paid): queue
/// `buyer` for the pickup in `fixer`'s open hours (once: a buyer already
/// queued keeps his place).
pub fn queue_walk_in(world: &mut World, buyer: EntityId, fixer: EntityId, forger: EntityId, tier: u8) {
    if world.walk_ins.iter().any(|w| w.buyer == buyer) {
        return;
    }
    let day = world.day();
    let unpaid = crate::systems::presence::on(world);
    world.walk_ins.push(WalkIn { buyer, fixer, forger, tier, day, unpaid });
}

/// Is `buyer` queued for a walk-in?
pub fn walk_in_pending(world: &World, buyer: EntityId) -> bool {
    world.walk_ins.iter().any(|w| w.buyer == buyer)
}

/// Midnight (`gray::papers_daily`, after the queue): the walk-ins pending
/// from an earlier day are counted waited; each body queued is given
/// today's walk (a `Visit` with `VisitWhy::Papers` at the Fixer's first
/// shift hour) when the household pass is on and no other visit is due.
pub fn walk_ins_daily(world: &mut World) {
    let day = world.day();
    let waited = world.walk_ins.iter().filter(|w| w.day < day).count() as u32;
    ownership::jobs2_row(world).walk_ins_waited += waited;
    if !crate::systems::household::on(world) {
        return;
    }
    let due: Vec<WalkIn> = world.walk_ins.clone();
    for w in due {
        if world.comp::<Brain>(w.buyer).is_none_or(|b| b.lod == Lod::Statistical) || world.visits.contains_key(&w.buyer)
        {
            continue;
        }
        let Some(hour) = fixer_first_hour(world, w.fixer) else { continue };
        let v = crate::systems::household::PendingVisit {
            kin: w.fixer,
            why: crate::systems::household::VisitWhy::Papers,
            hour,
        };
        world.visits.insert(w.buyer, v);
    }
}

/// The first hour of `fixer`'s staff's earliest shift (its open hours' start).
pub fn fixer_first_hour(world: &World, fixer: EntityId) -> Option<u8> {
    world
        .staff_of(fixer)
        .iter()
        .filter_map(|&s| world.comp::<Job>(s))
        .flat_map(|j| j.shifts.iter().map(|&(a, _)| a))
        .min()
        .map(|m| u8::try_from(u64::from(m) / TICKS_PER_HOUR).unwrap_or(0))
}

/// § 2.4 (fixer, `household::credit`): the owner collected the week's cut
/// at his Fixer in person (`fixer_collections`): a `Client` encounter with
/// each member of staff present (the `Fixer` row), an event.
pub fn collected(world: &mut World, owner: EntityId, fixer: EntityId) {
    ownership::jobs2_row(world).fixer_collections += 1;
    let staff: Vec<EntityId> =
        world.staff_of(fixer).iter().copied().filter(|&s| s != owner && present(world, s, fixer)).collect();
    let d = world.district_of_building(fixer);
    for &s in &staff {
        encounter_post(world, "Fixer", owner, s, Some(fixer), d, ServeKind::Client, 0);
    }
    // The pre-view pass phase 4 (P3, OPEN-119): the cut held by the staff
    // present is handed to the owner now (the rest waits for them).
    if let Some(owed) = world.fixer_owed.remove(&fixer) {
        let mut left = Vec::new();
        for (holder, coins) in owed {
            if staff.contains(&holder) {
                ownership::pay(world, Some(holder), Some(owner), coins, Flow::Gray);
            } else if crate::systems::law::living(world, holder) {
                left.push((holder, coins));
            }
        }
        if !left.is_empty() {
            world.fixer_owed.insert(fixer, left);
        }
    }
    let text = format!("{} collected the week's cut at {} in person", world.name_of(owner), world.name_of(fixer));
    world.push_event(EventKind::Served, &[owner, fixer], text);
}

/// § 2.2 (civic clerk, `weapons::daily`'s licence sale): the clerk the
/// licence is sold by: among the civic clerks on the books at a standing
/// Hall, alive and free, the lowest `hash(PURPOSE_CLERK, buyer, day)`. The
/// sale runs at midnight, when no clerk is on shift: deviation, the desk is
/// named, not the hour's presence. `None` off or with no clerk.
pub fn licence_clerk(world: &World, buyer: EntityId) -> Option<(EntityId, EntityId)> {
    if !tower_on(world) {
        return None;
    }
    let role = world.config.trade_role("civic_clerk")?;
    let day = world.day();
    world
        .buildings_of_kind(BuildingKind::Hall)
        .iter()
        .copied()
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| !b.demolished))
        .flat_map(|h| world.staff_of(h).iter().map(move |&c| (c, h)))
        .filter(|&(c, _)| {
            world.comp::<Job>(c).is_some_and(|j| j.role == role)
                && crate::systems::law::living(world, c)
                && !world.has::<Sentence>(c)
        })
        .map(|(c, h)| (hash(world, PURPOSE_CLERK, key(buyer) ^ key(c), day), c, h))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, c, h)| (c, h))
}

/// § 2.2 (civic clerk): the licence sold by `clerk` at `hall` to `buyer`
/// (`licences_named`): a `Client` encounter (the fee) through the clerk's row.
pub fn licence_sold(world: &mut World, clerk: EntityId, hall: EntityId, buyer: EntityId, fee: i64) {
    ownership::jobs2_row(world).licences_named += 1;
    serve(world, clerk, buyer, hall, ServeKind::Client, fee);
}

/// § 2.2 (civic clerk, `gray::papers_daily`'s bribe): the clerk on the take
/// is named: a `Client` encounter (the bribe; the clerk's `Served` memory is
/// the on-the-take record), `bribes_named`. Tower only.
pub fn bribe_taken(world: &mut World, clerk: EntityId, buyer: EntityId, bribe: i64) {
    if !tower_on(world) {
        return;
    }
    ownership::jobs2_row(world).bribes_named += 1;
    let place = world.comp::<Job>(clerk).and_then(|j| j.employer);
    let d = place.map(|p| world.district_of_building(p)).unwrap_or_default();
    encounter(world, clerk, buyer, place, d, ServeKind::Client, bribe);
}

/// Lives at work phase 5: the weekday a Fixer's owner collects in person.
const PURPOSE_COLLECT: u64 = 0x5e52_000a;

/// Midnight (`living::run`, after `gray::daily`): the tower's day: the
/// walk-ins' walks and waits ([`walk_ins_daily`]) and the owners' weekly
/// collections: a Fixer's owner who holds no Job and is not inside it,
/// alive, free and adult, visits it on a hashed weekday (`hash(
/// PURPOSE_COLLECT, owner, week)`) at its first shift hour (a `Visit` with
/// `VisitWhy::Collect`; a Statistical owner credited at the hour by
/// `household::hourly`), unless another visit is due. Nothing off.
pub fn tower_daily(world: &mut World) {
    if !tower_on(world) {
        return;
    }
    walk_ins_daily(world);
    if !crate::systems::household::on(world) {
        return;
    }
    let day = world.day();
    let week = day / 7;
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    for f in fixers {
        if world.comp::<Building>(f).is_none_or(|b| b.demolished || b.derelict) {
            continue;
        }
        let Some(o) = world.owner_of(f) else { continue };
        if !world.has::<Brain>(o)
            || world.has::<Job>(o)
            || world.has::<Sentence>(o)
            || !crate::systems::law::living(world, o)
            || !crate::systems::demography::is_adult(world, o)
            || world.comp::<Brain>(o).is_some_and(|b| b.emigrating)
            || world.visits.contains_key(&o)
            || world.comp::<Position>(o).is_some_and(|p| p.building == Some(f))
        {
            continue;
        }
        let u = hash(world, PURPOSE_COLLECT, key(o) ^ key(f), week);
        if week * 7 + ((u * 7.0) as u64).min(6) != day {
            continue;
        }
        let Some(hour) = fixer_first_hour(world, f) else { continue };
        let v =
            crate::systems::household::PendingVisit { kin: f, why: crate::systems::household::VisitWhy::Collect, hour };
        world.visits.insert(o, v);
    }
}
