//! The pressure pass phase 5b (docs/PRESSURE_PASS.md § Phase 5, the gang
//! half): **consequence on the gang's side**, under `[crew]` (absent: the
//! gangs as phase 3 left them).
//!
//! **Raids write a result** ([`raid_result`], from `raid::brawl` and
//! `raid::corp_brawl`): one line every raider and defender reads (the winner,
//! the wounded by name, the dead, the coins and food taken, the till), the
//! beaten's grudges on the other crew and the losing leader's on the winning
//! gang, and the next order: a repelled raid is answered (the defenders'
//! gang may retaliate on the raiders, `Shock::Raided`, a rethink at once).
//! The wounded carry a `Lost` memory, so phase 2's care sends them to a
//! Clinic or the back room.
//!
//! **Duty** ([`daily`], the LivingEarly midnight after the visits due): a
//! gang posts today's duty from its order: under a raid order every member
//! to the muster; under Contest or Retaliate a guard at the Hideout at
//! `guard_hour`; under Expand (and the street orders) a watch on held ground
//! (a protected business, a front, else the Hideout) at `watch_hour`; under
//! LieLow none. The leader holding court summons `court_summons` members by
//! name. Each duty is a `Visit` (`VisitWhy::Crew`) to the place at the hour;
//! a kept watch or guard pays `duty_pay` from the treasury (less what the
//! member owes for his bail). A member whose shift falls on the duty
//! chooses (loyalty, misery, rank, the gang's purse): the gang skips the
//! shift (`misery::skip_for`, an absence toward the firing), the shift is
//! a missed duty. Missed duties take standing, and `colours_misses` in
//! `miss_window_days` take the colours: the gang's toughest goes to him
//! (a `VisitWhy::Reckoning`) and beats him out.
//!
//! **Court's quorum** ([`court_held`], from `leaders::court_done`): fewer
//! than `court_quorum` of the summoned there and the court fails with the
//! reasons; `fold_fails` failed courts in `fold_window_days` and the gang
//! takes a rival's offer (absorbed) or folds.
//!
//! **Release and return** ([`on_released`]): a member who talked inside (a
//! report he filed names a gang-mate; [`daily`] rolls the talk once a
//! sentence) is met by the toughest and beaten out; the rest walk to the
//! Hideout and are welcomed (the crew there named, the bail as a debt, the
//! time served as standing, which the ladder reads: a new rank).
//!
//! **Rest days**: a member with no shift today goes to the crew on
//! `crew_p` (the Hideout under a fighting order, the gang's own venue
//! under Expand) at `crew_hour`, and the line names who else came.
//!
//! **The treasury** (phase 3's untraced sink): the gear the gang buys daily
//! (an Arms implant whenever it held `[chrome] gang_chrome_floor`, a bike,
//! a deck) spent every treasury down to the floor. [`gear_spare`]: gear
//! comes out of what is left over the gang's commitments (a week of duty
//! pay, two days of its fronts' payroll, the bail reserve) and at most one
//! piece a week.
//!
//! Statistical mirror: the duty, the court summons, the welcome and the
//! reckoning are `Visit`s, credited to a Statistical visitor at its hour.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{Building, Crime, Gang, GangMember, Job, Needs, Order, Personality, Position, Sentence, Shock};
use crate::config::CrewCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::household::{PendingVisit, VisitWhy};
use crate::systems::ownership::{self, Flow};
use crate::systems::{law, presence};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

mod custody;
mod raids;
mod reckoning;

pub use custody::*;
pub use raids::*;
pub use reckoning::*;

const PURPOSE_CHOOSE: u64 = 0x4357_0001;
const PURPOSE_TALK: u64 = 0x4357_0002;
const PURPOSE_FOLD: u64 = 0x4357_0003;
const PURPOSE_CREW: u64 = 0x4357_0004;
const PURPOSE_PICK: u64 = 0x4357_0005;

pub fn cfg(world: &World) -> Option<&CrewCfg> {
    world.config.crew.as_ref()
}

pub fn on(world: &World) -> bool {
    cfg(world).is_some()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

fn gang_name(world: &World, g: EntityId) -> String {
    world.comp::<Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone())
}

/// What a duty is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DutyKind {
    /// The raid's muster (no Visit: the Raid goal walks it; kept at the door).
    Muster,
    /// A watch on held ground (a protected business, a front, the Hideout).
    Watch,
    /// A guard at the Hideout (Contest, Retaliate).
    Guard,
    /// Summoned to court by name.
    Court,
    /// A rest day with the crew (no miss).
    Crew,
    /// Out of the cells, back to the Hideout (no miss).
    Welcome,
    /// V8 (`OPEN-264`): a Hideout founded on a floor, walked into by the
    /// members it is nearest (`vertical::found`; no miss).
    MoveIn,
}

impl DutyKind {
    fn label(self) -> &'static str {
        match self {
            DutyKind::Muster => "muster",
            DutyKind::Watch => "watch",
            DutyKind::Guard => "guard",
            DutyKind::Court => "court summons",
            DutyKind::Crew => "night with the crew",
            DutyKind::Welcome => "welcome back",
            DutyKind::MoveIn => "move into the new Hideout",
        }
    }

    /// A duty whose absence is a miss.
    fn owed(self) -> bool {
        matches!(self, DutyKind::Muster | DutyKind::Watch | DutyKind::Guard | DutyKind::Court)
    }
}

/// Today's duty of one member.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Duty {
    pub gang: EntityId,
    pub kind: DutyKind,
    pub place: EntityId,
    pub hour: u8,
    pub day: u64,
    /// The shift won the choice: no walk, a missed duty.
    #[serde(default)]
    pub chose_job: bool,
    #[serde(default)]
    pub kept: bool,
}

/// A member's book with his gang.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Book {
    /// Days of missed duties (the window's).
    pub misses: Vec<u32>,
    /// The ladder's term (`gang::leader_scores` reads `standing_w x` it).
    pub standing: f32,
    /// Bail the gang paid for him, not yet worked off.
    pub owed: i64,
    /// He talked inside this sentence.
    pub talked: bool,
    /// The talk was rolled for this sentence.
    pub talk_rolled: bool,
    /// When this sentence began (`gang::on_member_arrested`).
    pub jailed_at: Option<Tick>,
}

/// Why the gang's toughest is sent to a member.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallWhy {
    /// He talked to the law inside.
    Talked,
    /// He missed this many duties in the window.
    Colours(u32),
}

/// A reckoning on its way (the enforcer's errand).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub target: EntityId,
    pub gang: EntityId,
    pub why: CallWhy,
}

/// `World.crew`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub duties: BTreeMap<EntityId, Duty>,
    pub books: BTreeMap<EntityId, Book>,
    /// A gang's failed courts (days), the window's.
    pub failed_courts: BTreeMap<EntityId, Vec<u32>>,
    /// Enforcer -> his errand.
    pub calls: BTreeMap<EntityId, Call>,
    /// A gang's last gear purchase (the week's one), by week.
    pub gear_week: BTreeMap<EntityId, u64>,
    /// A gang's last raid that reached its door (a muster nobody went on is
    /// no miss), by day.
    pub raided_day: BTreeMap<EntityId, u64>,
    /// A gang's last raid result (tick, the verdict, who was in it), for
    /// the marchers who reach the door after it.
    pub last_raid: BTreeMap<EntityId, (Tick, String, Vec<EntityId>)>,
    /// R5 (OPEN-214): a gang whose treasury cannot pay a watch or a guard
    /// stood its duty down on this day (told once; told again when it pays).
    pub stood_down: BTreeMap<EntityId, u64>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.duties.is_empty()
            && self.books.is_empty()
            && self.failed_courts.is_empty()
            && self.calls.is_empty()
            && self.gear_week.is_empty()
            && self.raided_day.is_empty()
            && self.last_raid.is_empty()
            && self.stood_down.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The ladder and the treasury
// ---------------------------------------------------------------------------

/// The ladder's standing term of `m` (`gang::leader_scores`; 0 off).
pub fn standing_score(world: &World, m: EntityId) -> f32 {
    let Some(c) = cfg(world) else { return 0.0 };
    c.standing_w * world.crew.books.get(&m).map_or(0.0, |b| b.standing)
}

fn book(world: &mut World, m: EntityId) -> &mut Book {
    world.crew.books.entry(m).or_default()
}

/// The coins a gang has promised before gear: a week of its posted duty
/// pay, two days of its fronts' payroll, the bail reserve.
pub fn commitments(world: &World, gang: EntityId) -> i64 {
    let Some(c) = cfg(world) else { return 0 };
    let members = world.comp::<Gang>(gang).map_or(0, |g| g.members.len());
    let slots = duty_slots(c, members) as i64;
    let bail = world.config.protection.as_ref().map_or(0, |p| p.bail_reserve);
    7 * slots * c.duty_pay + 2 * crate::systems::leisure::owner_payroll(world, Some(gang)) + bail
}

/// The gear purchases' gate (`gang::daily_economy`): with `[crew]` on, a
/// gang buys gear only from what is left over its commitments and at most
/// one piece a week (`Gang`'s last purchase day, kept here). `true`: buy.
pub fn gear_spare(world: &World, gang: EntityId, floor: i64) -> bool {
    if !on(world) {
        return world.purse(Some(gang)) >= floor;
    }
    if world.crew.gear_week.get(&gang) == Some(&(world.day() / 7)) {
        return false;
    }
    world.purse(Some(gang)) - commitments(world, gang) >= floor
}

/// A piece of gear was bought today (the week's one).
pub fn gear_bought(world: &mut World, gang: EntityId) {
    if !on(world) {
        return;
    }
    let week = world.day() / 7;
    world.crew.gear_week.insert(gang, week);
}

fn duty_slots(c: &CrewCfg, members: usize) -> usize {
    if members == 0 {
        return 0;
    }
    ((members as f32 * c.duty_share).ceil() as usize).clamp(1, usize::from(c.duty_max))
}

// ---------------------------------------------------------------------------
// Duty
// ---------------------------------------------------------------------------

/// `m`'s job and its shift today covering `hour`.
fn shift_on(world: &World, m: EntityId, hour: u8) -> Option<Job> {
    let job = world.comp::<Job>(m)?.clone();
    job.employer?;
    let tod = u16::from(hour) * TICKS_PER_HOUR as u16;
    let key = job.next_shift_key(world.tick);
    (job.on_shift(tod) && crate::exec::routine::workday_of(world, m, &job, key)).then_some(job)
}

/// `m` works a shift today.
fn works_today(world: &World, m: EntityId) -> bool {
    let Some(job) = world.comp::<Job>(m) else { return false };
    if job.employer.is_none() {
        return false;
    }
    let key = job.next_shift_key(world.tick);
    let start = world.tick + Tick::from(job.ticks_until_shift(world.tick_of_day()));
    start < world.tick + TICKS_PER_DAY && crate::exec::routine::workday_of(world, m, job, key)
}

/// The duty place within reach of `m` (from his Home door; none past
/// `duty_reach_ticks`).
fn in_reach(world: &World, c: &CrewCfg, m: EntityId, place: EntityId) -> bool {
    let Some(to) = world.comp::<Building>(place).map(|b| b.door) else { return false };
    let from = crate::systems::contracts::home_door(world, m)
        .or_else(|| world.comp::<Position>(m).map(|p| p.tile))
        .unwrap_or(to);
    // S1 (OPEN-221): the member's own trip (the walk without the fare).
    crate::systems::distance::trip_ahead(world, m, from, to) <= c.duty_reach_ticks
}

/// Pressure pass round 3, S1 (OPEN-221): today's muster within `m`'s
/// reach (`raid::muster_trip` from his Home door, `duty_reach_ticks`), else
/// he is not posted to it and says so (a Crew line): V11's leader walked
/// 330 and 630 min to his own muster, both cut, then "missed". True off.
fn muster_reach(world: &mut World, c: &CrewCfg, m: EntityId, gang: EntityId) -> bool {
    // Round 5 U7 (`OPEN-222`): a member already on his Raid plan is in the
    // raid (the march past the muster): no muster to post him to and no
    // "sat out" line (V13: "sat out the raid" at 00:07 while he marched on
    // the Precinct, `raider_84195 d21`).
    if world.comp::<crate::components::Brain>(m).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::Raid)
    {
        return false;
    }
    let Some((t, at)) = crate::systems::raid::muster_trip(world, m, true) else { return true };
    if t <= c.duty_reach_ticks {
        ownership::jobs2_row(world).raid_posted += 1;
        return true;
    }
    ownership::jobs2_row(world).raid_sat_out += 1;
    let text = format!(
        "{} sat out {}'s raid: the muster at {at} is {} from home",
        world.name_of(m),
        gang_name(world, gang),
        crate::systems::distance::hours(t)
    );
    world.push_event(EventKind::Crew, &[m, gang], text);
    ownership::jobs2_row(world).choice_muster_out += 1;
    false
}

/// Round 8 X4 (`OPEN-241`): the place a gang's raid duty names, its muster
/// (`raid::pick_muster`) when one was chosen, else the Hideout.
fn muster_at(world: &World, gang: EntityId, hideout: EntityId) -> EntityId {
    world
        .comp::<Gang>(gang)
        .and_then(|g| g.raid_muster)
        .filter(|&b| world.comp::<crate::components::Building>(b).is_some_and(|bd| !bd.demolished))
        .unwrap_or(hideout)
}

/// The order's watch place for `m`: the nearest to his Home within reach of
/// the businesses the gang holds protection on, its fronts and its Hideout.
fn watch_place(world: &World, c: &CrewCfg, gang: EntityId, m: EntityId) -> Option<EntityId> {
    let mut places: Vec<EntityId> = world
        .coercions
        .values()
        .filter(|x| x.holder == Some(gang) && x.is_active() && x.why == crate::contract::CoercionWhy::Protection)
        .filter_map(|x| x.building)
        .collect();
    places.extend(crate::systems::leisure::fronts_of(world, gang));
    places.extend(world.hideout_of(gang));
    places.sort_unstable();
    places.dedup();
    let from = crate::systems::contracts::home_door(world, m).or_else(|| world.comp::<Position>(m).map(|p| p.tile))?;
    places
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (crate::systems::distance::travel(world, from, bd.door), b)))
        .filter(|&(t, _)| t <= c.duty_reach_ticks)
        .min()
        .map(|(_, b)| b)
}

/// The crew's rest-day place under the order, the first within `m`'s
/// reach: the gang's own venues (under Expand and the street orders), the
/// Hideout, then the crew's local in his district (the Bar there nearest
/// the Hideout, where the gang-mates of his district meet too); none under
/// LieLow.
fn crew_place(world: &World, c: &CrewCfg, gang: EntityId, m: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    let hideout = world.hideout_of(gang)?;
    let mut places = Vec::new();
    match g.order {
        Order::LieLow => return None,
        Order::Expand | Order::Squat | Order::Job | Order::Harvest | Order::VirtRaid => {
            places.extend(crate::systems::leisure::fronts_of(world, gang));
        }
        _ => {}
    }
    places.push(hideout);
    let door = world.comp::<Building>(hideout).map(|b| b.door)?;
    if let Some(home) = crate::systems::contracts::home_door(world, m) {
        let d = world.district_of(home);
        let local = world
            .buildings_of_kind(crate::components::BuildingKind::Bar)
            .iter()
            .copied()
            .filter_map(|b| world.comp::<Building>(b).map(|bd| (b, bd)))
            .filter(|(_, bd)| !bd.demolished && !bd.derelict && world.district_of(bd.door) == d)
            .map(|(b, bd)| (bd.door.manhattan(door), b))
            .min()
            .map(|(_, b)| b);
        places.extend(local);
    }
    places.into_iter().find(|&p| in_reach(world, c, m, p))
}

/// `m` free to take a duty: a free adult, no visit pending, not the
/// leader's own round.
fn free_for_duty(world: &World, m: EntityId) -> bool {
    crate::systems::contracts::free_adult(world, m)
        && !world.visits.contains_key(&m)
        && !world.crew.calls.contains_key(&m)
        && !crate::systems::wounded::is_down(world, m)
}

/// The hour `m` sets out to be at `place` by `hour` (his trip's estimate
/// from Home, whole hours, rounded up).
fn set_out(world: &World, m: EntityId, place: EntityId, hour: u8) -> u8 {
    let Some(to) = world.comp::<Building>(place).map(|b| b.door) else { return hour };
    let Some(from) = crate::systems::contracts::home_door(world, m) else { return hour };
    let lead = crate::systems::distance::trip_ahead(world, m, from, to).div_ceil(TICKS_PER_HOUR);
    hour.saturating_sub(u8::try_from(lead).unwrap_or(hour))
}

/// V8 (`OPEN-264`): `m` walks into his gang's new Hideout `place` (founded on a
/// floor tonight) at `hour`: a Visit (the walk and the climb his), no miss.
/// False when he is not free for it.
pub fn post_move_in(world: &mut World, m: EntityId, gang: EntityId, place: EntityId, hour: u8) -> bool {
    if !on(world) || !free_for_duty(world, m) || world.crew.duties.contains_key(&m) {
        return false;
    }
    let day = world.day();
    let hour = set_out(world, m, place, hour);
    let duty = Duty { gang, kind: DutyKind::MoveIn, place, hour, day, chose_job: false, kept: false };
    world.visits.insert(m, PendingVisit { kin: place, why: VisitWhy::Crew, hour });
    world.crew.duties.insert(m, duty);
    true
}

/// Post one duty: the choice against the shift, then the Visit.
fn post(world: &mut World, c: &CrewCfg, m: EntityId, gang: EntityId, kind: DutyKind, place: EntityId, hour: u8) {
    let day = world.day();
    let duty_hour = hour;
    let hour = if kind == DutyKind::Muster { hour } else { set_out(world, m, place, hour) };
    let mut duty = Duty { gang, kind, place, hour, day, chose_job: false, kept: false };
    // The shift that falls on the walk there or the duty itself.
    if let Some(job) = shift_on(world, m, duty_hour).or_else(|| shift_on(world, m, hour)) {
        let p = world.comp::<Personality>(m).map_or(0.5, |p| p.loyalty);
        let misery = crate::systems::misery::misery(world, m);
        let lt = world.comp::<GangMember>(m).is_some_and(|x| x.rank >= 1);
        let unpaid = world.purse(Some(gang)) - c.pay_reserve < c.duty_pay;
        let chance = (c.choose_base
            + c.choose_loyalty * (p - 0.5)
            + c.choose_misery * misery
            + if lt { c.choose_rank } else { 0.0 }
            - if unpaid { c.choose_unpaid } else { 0.0 })
        .clamp(0.05, 0.95);
        let gname = gang_name(world, gang);
        let pname = world.name_of(place);
        if hash(world, PURPOSE_CHOOSE, m, day ^ (u64::from(kind as u8) << 32)) < chance {
            let moment = format!(" for {gname}'s {} at {pname}", kind.label());
            if !crate::systems::misery::skip_for(world, m, &moment) {
                crate::systems::classes::walk_out(world, &[m]);
            }
            ownership::jobs2_row(world).crew_chose_gang += 1;
        } else {
            duty.chose_job = true;
            let text = format!(
                "{} chose the shift as {} over {gname}'s {} at {pname} ({:.0}% the gang's way)",
                world.name_of(m),
                job.role.label(),
                kind.label(),
                chance * 100.0
            );
            let mut actors = vec![m, gang];
            actors.extend(job.employer);
            world.push_event(EventKind::Crew, &actors, text);
            ownership::jobs2_row(world).crew_chose_job += 1;
        }
    }
    if kind.owed() {
        ownership::jobs2_row(world).crew_duty_posted += 1;
    }
    if !duty.chose_job && kind != DutyKind::Muster {
        world.visits.insert(m, PendingVisit { kin: place, why: VisitWhy::Crew, hour });
    }
    world.crew.duties.insert(m, duty);
}

/// The LivingEarly midnight (after the visits due): yesterday's duties
/// closed (the misses, the colours), the talk inside, the reckonings left
/// over, today's duties posted, the gangs that cannot muster.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let today = world.day();
    let day = today as u32;
    // 1. Yesterday's reckonings whose walk never came off: met where he is.
    let left: Vec<(EntityId, Call)> = std::mem::take(&mut world.crew.calls).into_iter().collect();
    for (e, call) in left {
        reckoning(world, &c, e, call, false);
    }
    // 2. Yesterday's duties (a reckoning sent now walks today).
    let old = std::mem::take(&mut world.crew.duties);
    for (m, d) in old {
        // V8: a move-in posted by tonight's founding (`vertical::found`) is today's.
        if d.kind == DutyKind::MoveIn && d.day == today {
            // The visits due (`household::visits_due`) cleared its walk: posted again.
            world.visits.entry(m).or_insert(PendingVisit { kin: d.place, why: VisitWhy::Crew, hour: d.hour });
            world.crew.duties.insert(m, d);
            continue;
        }
        if !d.kind.owed() {
            continue;
        }
        if d.kept {
            book(world, m).standing += c.kept_standing;
            continue;
        }
        if world.gang_of(m) != Some(d.gang) || !law::living(world, m) || world.has::<Sentence>(m) {
            continue;
        }
        // Round 4 T3 (OPEN-222): a member who leads the gang by midnight
        // owes it no duty (the leader is posted none; `raider_70326` took
        // the post on d23 and was "missed ... guard" for the round he led).
        if world.comp::<Gang>(d.gang).is_some_and(|g| g.leader == Some(m)) {
            continue;
        }
        // A muster that never marched (called off, the order changed) is no miss.
        if d.kind == DutyKind::Muster && world.crew.raided_day.get(&d.gang).is_none_or(|&x| x < d.day) {
            continue;
        }
        missed(world, &c, m, &d);
    }
    // 3. The talk inside, once a sentence.
    talks(world, &c);
    // 4. Today's duties, gang by gang.
    for gang in world.gangs() {
        post_gang(world, &c, gang);
    }
    // 5. The gangs that cannot muster.
    for gang in world.gangs() {
        fold_check(world, &c, gang, day);
    }
}
