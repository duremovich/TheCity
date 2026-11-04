//! Jobs & lives phase 4, vice (docs/JOBS_AND_LIVES.md § 5.1). Under
//! `[vice] enabled` (itself under `[jobs2] enabled`): off, every entry point
//! is a no-op, no venue of its kinds stands and no state is written.
//!
//! An economic and legal simulation of venues and labour, in systems terms
//! only: venues, shifts, visits, prices, tips, cuts, licences, sweeps,
//! fines, coins, safety and reputation numbers.
//!
//! - **Venues.** The StripClub and the Brothel stand on the leisure ladder
//!   (`leisure::paid_candidates`) as `Enjoy` satisfiers of `fun`, a
//!   Brothel's visit also of `intimacy`; the Lounge's on-duty companion
//!   makes its visit a companion's visit (`intimacy` too). A visit pays the
//!   venue's price to its owner (`Flow::Leisure`, the L2 path) and a tip of
//!   `tip_share` of it, customer -> the tipped worker on duty (`Flow::Tip`,
//!   untaxed). The Statistical evening and the Ambient book treat them as
//!   venues. Seeded per copy of `[vice] corp_rows` (those rows take the Vice
//!   niche); gangs build them as fronts (`leisure::fronts_daily`).
//! - **The street.** A `street_worker` Pitch (the phase-2 Pitch; a `vice`
//!   row with no workplace) works the night: on `p_street_visit` an hour a
//!   resident of the district under `street_intimacy` pays `street_price`
//!   (`Flow::Leisure`). The gang holding a district with street pitches
//!   names a pimp (its loyal member of lowest id, no dealer): those pitches
//!   pay the gang `pimp_cut` of their day (`Flow::StreetCut`) instead of
//!   `[jobs2] cut_frac`. Unprotected (no pimp, the worker unarmed), a
//!   working pitch-hour rolls `p_assault_street × (0.5 + fear)`: a resident
//!   of the district takes the night's coins (an Assault; the worker's
//!   `WasRobbed` and `Lost` memories, the weapon's want).
//! - **The law.** With `street_legal` off a street pitch in a `Sweep`
//!   district is swept on `p_street_sweep` (the phase-2 fine and closing)
//!   and files `Solicitation` (petty: a night in the cells), capped a day per
//!   capita (R3). A licensed Brothel closes only under a Crackdown in its
//!   district; an unlicensed one in a Sweep or a Crackdown.
//!
//! Every coin moves between purses. Hourly work is O(street pitches); the
//! midnight pass O(gangs + Brothels).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Corp, DistrictId, Gang, Job, Kit, Lod, MemoryKind, Needs, Niche, Pitch, Position,
    Sentence, Stance, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::{TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

mod brothels;
mod pimps;
mod street;
mod visits;

pub use brothels::*;
pub use pimps::*;
pub use street::*;
pub use visits::*;

const PURPOSE_VISIT: u64 = 0x5649_0001;
const PURPOSE_PAYER: u64 = 0x5649_0002;
const PURPOSE_ASSAULT: u64 = 0x5649_0003;
const PURPOSE_SWEEP: u64 = 0x5649_0004;
/// Lives at work phase 4 (§ 2.3): the client among the bodies near a pitch.
const PURPOSE_CLIENT: u64 = 0x5649_0005;
/// R6a: a booked client turning rough.
const PURPOSE_ROUGH: u64 = 0x5649_0006;
/// Round 5 U6: the pimp's booked client, drawn from the district's residents.
const PURPOSE_BOOK: u64 = 0x5649_0007;

/// Phase 4's vice is live: `[jobs2] enabled` and `[vice] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::industry::on(world) && world.config.vice.enabled
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

/// Phase 4's state on `Jobs2State` (empty, and not saved, with the switch off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViceState {
    /// District index -> its pimp (the holding gang's member).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pimps: BTreeMap<u16, EntityId>,
    /// `Solicitation` reports filed today (R3's cap).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub solicit_today: u32,
    /// Lives at work phase 4b (§ 2.3): district index -> the last day its
    /// pimp was present at one of its pitches (a body within `pimp_tiles`, a
    /// Statistical one credited); `name_pimps` drops him `pimp_absent_days`
    /// on. Written only with `[service] street` on.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pimp_seen: BTreeMap<u16, u64>,
    /// The pressure pass R6a (OPEN-217): a street worker (`key`) -> the tick
    /// her pimp's body last collected at her pitch (`round_pitch` walks to
    /// the pitch owed, then the one longest unvisited). Pruned at midnight.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pimp_visits: BTreeMap<u64, u64>,
    /// R6a: a madam (`key`) -> her night's bookings, told at her shift's end.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bookings: BTreeMap<u64, Bookings>,
    /// R6a: a client (`key`) barred from a house after a rough booking ->
    /// (the house, the day the bar lifts).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub barred: BTreeMap<u64, (EntityId, u64)>,
    /// Round 3 S5 (OPEN-217): a bouncer (`key`) -> his night at the door
    /// (the customer ledger), told at his shift's end (`door_told`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub door_nights: BTreeMap<u64, DoorNight>,
    /// Round 4 T4 (OPEN-217): a serving body of `[verbs] night_rows` (`key`)
    /// -> its night (the room, the served, the tips, a dancer's sets), told
    /// at the shift's end (`night_told`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub serve_nights: BTreeMap<u64, ServeNight>,
    /// Round 4 T7 (OPEN-233): a face turned away at a kept door (`key`) ->
    /// (the door, the tick, the day a return's bar there lifts or 0), kept
    /// `[verbs] door_return_ticks` or while barred (`turned_again`, `barred`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub turned: BTreeMap<u64, Vec<(EntityId, u64, u64)>>,
    /// Round 6 V6 (OPEN-217): a Fighter (`key`) -> the tick of his last
    /// bout, the pit's card (`leisure::bouts` read it from the `Bout`
    /// memory, which an off-screen fighter's eight slots can drop: V14's
    /// 100k fighters stood in the pit at 23:00 a week with no bout). Pruned
    /// at the bout to the living Fighters.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bouts: BTreeMap<u64, u64>,
    /// Round 7 W6 (OPEN-217): the pimps (`key`) who made a collection since
    /// the last midnight (the named pimp-day's count, `pimp_round_days`).
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub collected: std::collections::BTreeSet<u64>,
}

/// Round 6 V6: the tick of `fighter`'s last bout on the card (0: none).
pub fn last_bout(world: &World, fighter: EntityId) -> u64 {
    world.jobs2.vice.bouts.get(&key(fighter)).copied().unwrap_or(0)
}

/// Round 6 V6: `fighter` fought tonight.
pub fn note_bout(world: &mut World, fighter: EntityId) {
    let tick = world.tick;
    world.jobs2.vice.bouts.insert(key(fighter), tick);
}

/// Round 6 V6: the pit's book pruned to the living Fighters.
pub fn keep_bouts(world: &mut World) {
    if world.jobs2.vice.bouts.is_empty() {
        return;
    }
    let keep: std::collections::BTreeSet<u64> = world
        .workers(crate::components::Role::Fighter)
        .iter()
        .copied()
        .filter(|&f| crate::systems::law::living(world, f))
        .map(key)
        .collect();
    world.jobs2.vice.bouts.retain(|k, _| keep.contains(k));
}

/// Round 4 T4 (OPEN-217): a serving body's night at its venue: the room
/// (every visit that arrived there), whom it served (the regulars among
/// them), the tips it took, and a dancer's sets (the set in progress apart).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServeNight {
    pub venue: EntityId,
    pub room: u32,
    pub served: u32,
    /// The served, first come (at most a line's names and a few more).
    pub faces: Vec<EntityId>,
    /// The regulars among the served (the ledger's `regular_after`).
    pub regulars: Vec<EntityId>,
    pub tips: u32,
    pub coins: i64,
    pub sets: u32,
    /// The sets danced to an empty room (no line of their own).
    pub empty_sets: u32,
    pub set_room: u32,
    pub set_tips: u32,
    pub set_coins: i64,
    pub set_faces: Vec<EntityId>,
}

impl Default for ServeNight {
    fn default() -> Self {
        ServeNight {
            venue: EntityId::NONE,
            room: 0,
            served: 0,
            faces: Vec::new(),
            regulars: Vec::new(),
            tips: 0,
            coins: 0,
            sets: 0,
            empty_sets: 0,
            set_room: 0,
            set_tips: 0,
            set_coins: 0,
            set_faces: Vec::new(),
        }
    }
}

/// The names kept on a night's ledger (a line's names and the "N more").
const NIGHT_FACES: usize = 12;

/// Round 4 T4: a body of `[verbs] night_rows` at its own venue `b` now.
fn night_body(world: &World, id: EntityId, b: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|br| br.lod != Lod::Statistical)
        && world.comp::<Job>(id).is_some_and(|j| j.employer == Some(b))
        && crate::systems::service::night_row(world, id, false)
}

fn night_note(world: &mut World, id: EntityId, b: EntityId, f: impl FnOnce(&mut ServeNight)) {
    let n = world.jobs2.vice.serve_nights.entry(key(id)).or_default();
    if n.venue != b {
        *n = ServeNight { venue: b, ..ServeNight::default() };
    }
    f(n);
}

/// Round 4 T4 (`service::arrive`): a visit arrived at `b`: the room of
/// every serving body of `night_rows` on duty there.
pub fn night_room(world: &mut World, b: EntityId) {
    if world.config.verbs.is_none() {
        return;
    }
    let tod = world.tick_of_day();
    let on: Vec<EntityId> = world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| {
            world.comp::<Job>(s).is_some_and(|j| j.on_shift(tod))
                && night_body(world, s, b)
                && world.comp::<Position>(s).is_some_and(|p| p.building == Some(b))
        })
        .collect();
    for s in on {
        let set = on_set(world, s);
        night_note(world, s, b, |n| {
            n.room += 1;
            n.set_room += u32::from(set);
        });
    }
}

/// Round 7 W5 (`OPEN-237`): `id` is on the stage now (a stations block,
/// `ClerkWork`, in progress): a set's room and tips are those taken in it,
/// not the rest before it (`dancer_17424 d23 18:00`, 20k: the client barred
/// at 18:00, during her rest before the set, was a tipper "at" the 18:05 set).
fn on_set(world: &World, id: EntityId) -> bool {
    world
        .comp::<Brain>(id)
        .is_some_and(|b| matches!(b.exec, crate::exec::ExecState::Use { kind: crate::goap::ActionKind::ClerkWork, .. }))
}

/// Round 4 T4 (`service::encounter_row`): `server` served `served` at `b`
/// (`regular`: a regular of his ledger now).
pub fn night_served(world: &mut World, server: EntityId, served: EntityId, b: EntityId, regular: bool) {
    if world.config.verbs.is_none() || !night_body(world, server, b) {
        return;
    }
    night_note(world, server, b, |n| {
        n.served += 1;
        if n.faces.len() < NIGHT_FACES && !n.faces.contains(&served) {
            n.faces.push(served);
        }
        if regular && n.regulars.len() < NIGHT_FACES && !n.regulars.contains(&served) {
            n.regulars.push(served);
        }
    });
}

/// Round 4 T4 (`service::encounter_row`): `worker` took a tip of `paid`
/// from `from` at `b`.
pub fn night_tip(world: &mut World, worker: EntityId, from: EntityId, b: EntityId, paid: i64) {
    if paid <= 0 || world.config.verbs.is_none() || !night_body(world, worker, b) {
        return;
    }
    let set = on_set(world, worker);
    night_note(world, worker, b, |n| {
        n.tips += 1;
        n.coins += paid;
        if !set {
            return;
        }
        n.set_tips += 1;
        n.set_coins += paid;
        if n.set_faces.len() < NIGHT_FACES && !n.set_faces.contains(&from) {
            n.set_faces.push(from);
        }
    });
}

/// `names` of a line: the first `[verbs] night_names`, then "and N more".
pub fn named(world: &World, ids: &[EntityId], more: usize) -> String {
    let cap = world.config.verbs.as_ref().map_or(3, |v| usize::from(v.night_names.max(1)));
    let mut out: Vec<String> = ids.iter().take(cap).map(|&a| world.name_of(a)).collect();
    let rest = more.max(ids.len()).saturating_sub(cap);
    if rest > 0 {
        out.push(format!("{rest} more"));
    }
    match out.len() {
        0 => String::new(),
        1 => out.remove(0),
        _ => {
            let last = out.pop().unwrap_or_default();
            format!("{} and {last}", out.join(", "))
        }
    }
}

/// Round 4 T4 (`exec::actions`, a stations block's end): a body of `[verbs]
/// set_rows` (the dancer) closed a block begun at `started` at her venue:
/// a set on the stage, one line (the room, the tips, the tippers), the set
/// folded into the night. Round 7 W5 (`OPEN-237`): also a block cut short
/// (`exec::actions::on_abort`); a block that is no set here clears the set's
/// tally, so it never carries into the next set (`dancer_74279 d21 20:32`: the
/// rough client barred at 20:32 tipped "at" the 23:31 set, his tip from the set
/// the Flee cut).
pub fn set_done(world: &mut World, id: EntityId, started: crate::time::Tick) {
    let Some(b) = world.comp::<Job>(id).and_then(|j| j.employer) else { return };
    if world.config.verbs.is_none() {
        return;
    }
    if !night_body(world, id, b)
        || !crate::systems::service::night_row(world, id, true)
        || world.comp::<Position>(id).is_none_or(|p| p.building != Some(b))
    {
        if let Some(s) = world.jobs2.vice.serve_nights.get_mut(&key(id)) {
            s.set_room = 0;
            s.set_tips = 0;
            s.set_coins = 0;
            s.set_faces.clear();
        }
        return;
    }
    let mins = world.tick.saturating_sub(started);
    let mut n = ServeNight::default();
    night_note(world, id, b, |s| {
        s.sets += 1;
        s.empty_sets += u32::from(s.set_room == 0);
        n = s.clone();
        s.set_room = 0;
        s.set_tips = 0;
        s.set_coins = 0;
        s.set_faces.clear();
    });
    crate::systems::ownership::jobs2_row(world).sets_danced += 1;
    // A set to an empty room is the night line's count, not a line.
    if n.set_room == 0 {
        return;
    }
    let room = match n.set_room {
        1 => "to a room of one".to_string(),
        k => format!("to a room of {k}"),
    };
    let tips = if n.set_tips == 0 {
        ", no tips".to_string()
    } else {
        let s = if n.set_tips == 1 { "" } else { "s" };
        format!(", {} tip{s}, {}c ({})", n.set_tips, n.set_coins, named(world, &n.set_faces, n.set_faces.len()))
    };
    let text = format!("{} danced a set at {} ({mins} min) {room}{tips}", world.name_of(id), world.name_of(b));
    world.push_event(EventKind::Served, &[id, b], text);
}

/// Round 4 T4 (`service::shift_end`): a serving body's night told in one
/// line, the ledger cleared: a dancer's sets and tips, a club hand's floor
/// (served, the regulars, the room). A dead night is a night too.
pub fn night_told(world: &mut World, id: EntityId) {
    let n = world.jobs2.vice.serve_nights.remove(&key(id));
    let Some(b) = world.comp::<Job>(id).and_then(|j| j.employer) else { return };
    if world.config.verbs.is_none() || !night_body(world, id, b) {
        return;
    }
    let n = n.filter(|n| n.venue == b).unwrap_or_default();
    let name = world.name_of(id);
    let at = world.name_of(b);
    let regs = if n.regulars.is_empty() {
        String::new()
    } else {
        format!("; regulars {}", named(world, &n.regulars, n.regulars.len()))
    };
    let text = if crate::systems::service::night_row(world, id, true) {
        let tips = if n.tips == 0 {
            "no tips".to_string()
        } else {
            format!("{} tip{}, {}c", n.tips, if n.tips == 1 { "" } else { "s" }, n.coins)
        };
        let empty = if n.empty_sets > 0 { format!(" ({} to an empty room)", n.empty_sets) } else { String::new() };
        format!("{name} danced {} sets{empty} at {at} tonight to a room of {}: {tips}{regs}", n.sets, n.room)
    } else if n.served == 0 {
        format!("{name} worked the floor of {at} tonight: a room of {}, nobody to serve", n.room)
    } else {
        format!(
            "{name} worked the floor of {at} tonight: a room of {}, served {} ({}){regs}",
            n.room,
            n.served,
            named(world, &n.faces, n.served as usize)
        )
    };
    world.push_event(EventKind::Served, &[id, b], text);
    crate::systems::ownership::jobs2_row(world).serve_nights += 1;
}

/// Round 3 S5 (OPEN-217): a bouncer's night at his door: the visits served
/// inside, the turned away (and the commonest cause), the thrown out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DoorNight {
    pub venue: EntityId,
    pub let_in: u32,
    pub refused: u32,
    pub causes: Vec<(String, u32)>,
    pub thrown: u32,
}

impl Default for DoorNight {
    fn default() -> Self {
        DoorNight { venue: EntityId::NONE, let_in: 0, refused: 0, causes: Vec::new(), thrown: 0 }
    }
}

/// Round 3 S5: note on `bouncer`'s door night at `venue` (`f` edits it).
pub fn door_note(world: &mut World, bouncer: EntityId, venue: EntityId, f: impl FnOnce(&mut DoorNight)) {
    let n = world.jobs2.vice.door_nights.entry(key(bouncer)).or_default();
    if n.venue != venue {
        *n = DoorNight { venue, ..DoorNight::default() };
    }
    f(n);
}

/// Round 3 S5 (`service::shift_end`): the bouncer's night told in one line
/// ("kept the door of X tonight: N let in, M turned away (drunk 3), K thrown
/// out"), the tally cleared. Bouncers worked 12 nights with no door line in
/// their diaries (V11): a quiet door is a door too.
pub fn door_told(world: &mut World, bouncer: EntityId) {
    let Some(n) = world.jobs2.vice.door_nights.remove(&key(bouncer)) else { return };
    // A body's diary; the off-screen door's night is cleared untold.
    let body =
        world.comp::<crate::components::Brain>(bouncer).is_some_and(|b| b.lod != crate::components::Lod::Statistical);
    if !body || n.let_in + n.refused + n.thrown == 0 {
        return;
    }
    let mut causes = n.causes.clone();
    causes.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    let why: Vec<String> = causes.iter().take(2).map(|(c, k)| format!("{c} {k}")).collect();
    let refused = if n.refused == 0 {
        "nobody turned away".to_string()
    } else {
        format!("{} turned away ({})", n.refused, why.join(", "))
    };
    let thrown = if n.thrown > 0 { format!(", {} thrown out", n.thrown) } else { String::new() };
    let text = format!(
        "{} kept the door of {} tonight: {} let in, {refused}{thrown}",
        world.name_of(bouncer),
        world.name_of(n.venue),
        n.let_in
    );
    world.push_event(EventKind::Served, &[bouncer, n.venue], text);
}

/// R6a (OPEN-217): a madam's night of bookings at her house.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bookings {
    pub house: EntityId,
    /// Clients booked and the coins their visits paid.
    pub clients: u32,
    pub coins: i64,
    /// The workers booked, with their count tonight.
    pub workers: Vec<(EntityId, u32)>,
    /// Rough clients thrown out tonight.
    pub rough: u32,
}

impl Default for Bookings {
    fn default() -> Self {
        Bookings { house: EntityId::NONE, clients: 0, coins: 0, workers: Vec::new(), rough: 0 }
    }
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl ViceState {
    pub fn is_default(&self) -> bool {
        *self == ViceState::default()
    }
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Hourly (from `living::run`): the street's visits and assault rolls.
pub fn hourly(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let _t = crate::prof::scope!("vice::hourly");
    street_hour(world);
}

/// Midnight (from `living::run`, before the street's day closes): the
/// day's cap reset, the pimps named, the Brothels the law closes.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("vice::daily");
    world.jobs2.vice.solicit_today = 0;
    // R6a: the round's marks last a night.
    let since = world.tick.saturating_sub(TICKS_PER_DAY);
    world.jobs2.vice.pimp_visits.retain(|_, t| *t >= since);
    // Round 7 W6 (OPEN-217): the day's named pimps and those who collected.
    let named: Vec<EntityId> = world.jobs2.vice.pimps.values().copied().collect();
    let collected = std::mem::take(&mut world.jobs2.vice.collected);
    let ran = named.iter().filter(|&&p| collected.contains(&key(p))).count() as u32;
    let row = ownership::jobs2_row(world);
    row.pimp_named_days += named.len() as u32;
    row.pimp_round_days += ran;
    name_pimps(world);
    close_brothels(world);
}
