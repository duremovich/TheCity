//! Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1, § 2.1, § 2.2's
//! ripperdoc and orderly): the **service encounter**, the one mechanism
//! that puts a person on the other side of every counter. Under `[lives]
//! enabled` and `[service] venues` (`CITYSIM_LIVES_OFF=1`): off, every
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
//! `World::service_room`) whose `hash(PURPOSE_SERVE, place, served, hour)`
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

/// The regulars a server keeps (§ 1.1: the oldest falls off).
pub const REGULARS_CAP: usize = 8;

/// A server's room this hour (`World::service_room`, not saved): the hour
/// and the encounters taken in it (`per_hour` is the cap).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

/// Lives at work phase 3 is live: the encounter on and `[service] civic`
/// (`CITYSIM_CIVIC_OFF=1`): the civic counterparts, the near round, tied
/// housing and the parent's answer to a taken child.
pub fn civic_on(world: &World) -> bool {
    on(world) && world.config.service.civic
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
    row_for(world, world.comp::<Job>(id)?.role)
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
    let hour = world.tick / TICKS_PER_HOUR;
    let pick = servers(world, place, kind)
        .into_iter()
        .filter(|&(s, cap)| world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap))
        .map(|(s, _)| (hash(world, PURPOSE_SERVE, key(place) ^ key(served), hour ^ key(s)), s))
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
    }
    if civic_on(world) {
        civic_hour(world);
    }
}

/// Does `s`'s job start a shift this hour (the roster's and the headcount's
/// hour, as the madam's `staff_hour`)?
fn first_hour(world: &World, s: EntityId) -> bool {
    let tod = world.tick_of_day();
    world
        .comp::<Job>(s)
        .is_some_and(|j| j.shifts.iter().any(|&(start, _)| tod >= start && u32::from(tod) < u32::from(start) + 60))
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
        for (s, kids) in roster {
            for k in kids {
                arrive(world, k, s, ServeKind::Pupil, 0);
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
fn taught(world: &mut World, teacher: EntityId, child: EntityId, school: EntityId, valence: f32) {
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
        let _ = school;
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
        world.remember(tenant, MemoryKind::Served, Some(server), salience, -0.3, false);
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
    for t in tenants {
        serve(world, id, t, block, ServeKind::Tenant, 0);
    }
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
    for &j in &on_duty {
        world.remember(j, MemoryKind::Served, None, salience, -0.3, false);
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
                    && world.comp::<Job>(s).is_some_and(|j| {
                        j.shifts.iter().any(|&(start, _)| tod >= start && u32::from(tod) < u32::from(start) + 60)
                    })
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

/// One encounter (§ 1.2's five steps). Returns whether a regular was made.
pub fn serve(world: &mut World, server: EntityId, served: EntityId, place: EntityId, kind: ServeKind, coins: i64) {
    let Some(row) = row_of_agent(world, server).cloned() else { return };
    let kind_label = kind.label();
    let place_kind = world.comp::<Building>(place).map(|b| b.kind);
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
    let tipped = if row_tipped(world, &row.key) { Some(server) } else { tipped_pick(world, place, served) };
    if kind == ServeKind::Visit && coins > 0 {
        if let Some(t) = tipped.filter(|&t| t != served) {
            let tip = ((coins as f32 * world.config.vice.tip_share).round() as i64).max(1);
            let paid = ownership::pay(world, Some(served), Some(t), tip, Flow::Tip);
            // § 2.1: the madam's cut of a tip at her house.
            if paid > 0 && place_kind == Some(BuildingKind::Brothel) {
                let madam = servers(world, place, ServeKind::Staff).first().map(|&(m, _)| m);
                if let Some(m) = madam.filter(|&m| m != t) {
                    let cut = (paid as f32 * world.config.service.madam_cut).round() as i64;
                    if cut > 0 {
                        ownership::pay(world, Some(t), Some(m), cut, Flow::MadamCut);
                    }
                }
            }
        }
    }
    // Lives at work phase 3 (§ 2.2): the civic arms.
    let mut outcome = Outcome::Done;
    match kind {
        ServeKind::Pupil => taught(world, server, served, place, row.valence),
        ServeKind::Tenant => outcome = tenant(world, server, served, place),
        _ => {}
    }
    // 2. Memories, bodies only (`remember` drops a Statistical agent's and a chip's).
    world.remember(server, MemoryKind::Served, Some(served), salience, row.valence, false);
    world.remember(served, MemoryKind::Served, Some(server), salience, row.valence, false);
    // 3. The regulars ledger and the edge.
    let regular = note_regular(world, server, served, &row);
    // 4. The skill.
    train(world, server, row.skill, row.skill_gain);
    // 5. The event.
    if regular {
        outcome = Outcome::Regular;
    }
    if row.event_on.contains(&outcome.cfg()) {
        let (who, whom, at) = (world.name_of(server), world.name_of(served), world.name_of(place));
        let (kind, text) = match outcome {
            Outcome::Regular => (EventKind::Served, format!("{whom} is now a regular of {who} at {at}")),
            Outcome::Notice => (EventKind::Notice, format!("{who} served notice on {whom} at {at}")),
            _ => (EventKind::Served, format!("{who} served {whom} ({kind_label}) at {at}")),
        };
        world.push_event(kind, &[server, served, place], text);
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
    let staff = door_staff(world, b);
    if staff.is_empty() {
        return;
    }
    let u = hash(world, PURPOSE_DOOR, key(id), world.tick / TICKS_PER_HOUR);
    let bouncer = staff[((u * staff.len() as f32) as usize).min(staff.len() - 1)];
    let Some(row) = row_of_agent(world, bouncer).cloned() else { return };
    let salience = world.config.service.served_salience;
    world.remember(bouncer, MemoryKind::Served, Some(id), salience, -0.1, false);
    world.remember(id, MemoryKind::Served, Some(bouncer), salience, -0.3, false);
    if row.event_on.contains(&ServeOutcome::Refused) {
        let text =
            format!("{} turned {} away at the door of {}", world.name_of(bouncer), world.name_of(id), world.name_of(b));
        world.push_event(EventKind::Served, &[bouncer, id, b], text);
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
    let salience = world.config.service.served_salience;
    world.remember(bouncer, MemoryKind::Served, Some(attacker), salience, -0.1, false);
    world.remember(attacker, MemoryKind::Served, Some(bouncer), salience, -0.4, false);
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

/// The regulars kept for `server` (the inspector, the diary).
pub fn regulars_of(world: &World, server: EntityId) -> &[Regular] {
    world.regulars.get(&server).map_or(&[], SmallVec::as_slice)
}
