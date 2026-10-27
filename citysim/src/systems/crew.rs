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
    let Some((t, at)) = crate::systems::raid::muster_trip(world, m, true) else { return true };
    if t <= c.duty_reach_ticks {
        return true;
    }
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

fn missed(world: &mut World, c: &CrewCfg, m: EntityId, d: &Duty) {
    let day = world.day() as u32;
    let window = c.miss_window_days;
    // R5 (OPEN-214), the attrition judged: a court summons missed at work is
    // excused (the standing, not the colours).
    if c.court_work_excused && d.kind == DutyKind::Court && d.chose_job {
        book(world, m).standing -= c.miss_standing;
        let text = format!(
            "{} missed {}'s court summons at {} (at work: excused)",
            world.name_of(m),
            gang_name(world, d.gang),
            world.name_of(d.place)
        );
        world.push_event(EventKind::Crew, &[m, d.gang], text);
        ownership::jobs2_row(world).crew_duty_missed += 1;
        return;
    }
    let count = {
        let b = book(world, m);
        b.misses.retain(|&x| x + window > day);
        b.misses.push(day);
        b.standing -= c.miss_standing;
        b.misses.len() as u32
    };
    let why = if d.chose_job { " (at work)" } else { "" };
    let text = format!(
        "{} missed {}'s {} at {}{why} ({count} in {window} days)",
        world.name_of(m),
        gang_name(world, d.gang),
        d.kind.label(),
        world.name_of(d.place)
    );
    world.push_event(EventKind::Crew, &[m, d.gang], text);
    ownership::jobs2_row(world).crew_duty_missed += 1;
    if count >= c.colours_misses {
        if let Some(b) = world.crew.books.get_mut(&m) {
            b.misses.clear();
        }
        send_reckoning(world, c, m, d.gang, CallWhy::Colours(count));
    }
}

/// The gang's toughest free member, not `not`, with no errand.
fn enforcer(world: &World, gang: EntityId, not: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    g.members
        .iter()
        .copied()
        .filter(|&m| m != not && crate::systems::contracts::free_adult(world, m))
        .filter(|&m| !world.visits.contains_key(&m) && !world.crew.calls.contains_key(&m))
        .map(|m| (std::cmp::Reverse(ordered_float::OrderedFloat(law::fighting(world, m))), m))
        .min()
        .map(|(_, m)| m)
}

/// Send the toughest to `target` (a person Visit at the next hour), or with
/// nobody to send, take the colours where he stands.
fn send_reckoning(world: &mut World, c: &CrewCfg, target: EntityId, gang: EntityId, why: CallWhy) {
    let Some(e) = enforcer(world, gang, target) else {
        out_of_colours(world, target, gang, why, None);
        return;
    };
    let call = Call { target, gang, why };
    let hour = (world.tick_of_day() / TICKS_PER_HOUR as u16 + 1) as u8;
    if !crate::systems::household::on(world) || hour >= 23 {
        reckoning(world, c, e, call, false);
        return;
    }
    world.visits.insert(e, PendingVisit { kin: target, why: VisitWhy::Reckoning, hour });
    world.crew.calls.insert(e, call);
}

/// The reckoning's Visit arrived (`household::credit`), or it is met off
/// screen at its hour.
pub fn reckoned(world: &mut World, enforcer: EntityId, target: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(call) = world.crew.calls.remove(&enforcer) else { return };
    if call.target != target {
        return;
    }
    reckoning(world, &c, enforcer, call, walked);
}

fn reckoning(world: &mut World, c: &CrewCfg, enforcer: EntityId, call: Call, walked: bool) {
    let target = call.target;
    if world.gang_of(target) != Some(call.gang) || !law::living(world, target) || world.has::<Sentence>(target) {
        return;
    }
    if !law::living(world, enforcer) || world.has::<Sentence>(enforcer) {
        out_of_colours(world, target, call.gang, call.why, None);
        return;
    }
    let mods = law::FightMods { kill_mult: c.beat_kill_mult, ..Default::default() };
    let (winner, loser, died) = law::resolve_fight_mods(world, enforcer, target, mods);
    let (en, tn) = (world.name_of(enforcer), world.name_of(target));
    let how = if walked { "came for" } else { "caught up with" };
    let fight = if winner == enforcer {
        if died {
            format!("{en} {how} {tn} and beat them to death")
        } else {
            format!("{en} {how} {tn} and beat them")
        }
    } else {
        format!("{en} {how} {tn}, who fought back and won")
    };
    let kind = if died && loser == target { EventKind::Murder } else { EventKind::Assault };
    world.push_event(kind, &[enforcer, target], fight);
    let tile = world.comp::<Position>(target).or_else(|| world.comp::<Position>(enforcer)).map(|p| p.tile);
    if let Some(t) = tile {
        let crime = if died && loser == target { Crime::Murder } else { Crime::Assault };
        if law::living(world, enforcer) {
            law::raise_crime_on(world, enforcer, (!died).then_some(target), Some(target), crime, t);
        }
    }
    if law::living(world, target) {
        let by = (winner == enforcer).then_some(enforcer);
        out_of_colours(world, target, call.gang, call.why, by);
    }
}

fn out_of_colours(world: &mut World, target: EntityId, gang: EntityId, why: CallWhy, by: Option<EntityId>) {
    let reason = match why {
        CallWhy::Talked => "talked to the law: thrown out".to_string(),
        CallWhy::Colours(n) => format!("lost the colours: {n} duties missed"),
    };
    let gname = gang_name(world, gang);
    let by_text = by.map_or_else(String::new, |b| format!(", beaten out by {}", world.name_of(b)));
    let text = format!("{} is out of {gname} ({reason}{by_text})", world.name_of(target));
    let mut actors = vec![target, gang];
    actors.extend(by);
    world.push_event(EventKind::Crew, &actors, text);
    {
        let row = ownership::jobs2_row(world);
        match why {
            CallWhy::Talked => row.crew_talker_beaten += 1,
            CallWhy::Colours(_) => row.crew_colours_lost += 1,
        }
    }
    world.crew.books.remove(&target);
    crate::systems::gang::leave(world, target, &reason);
    // R5 (OPEN-214): beaten out is a leaver's cooldown, as walking away is
    // (a shadow pick beaten out at 11:54 joined the same gang at 17:45).
    let until = world.tick + world.config.living.desist_cooldown_days * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<crate::components::Brain>(target) {
        b.cooldowns.insert(crate::components::GoalKind::JoinGang, until);
    }
}

/// A jailed member's talk, once a sentence of `talk_min_days` or more: a
/// report he files on a gang-mate (the leader first) for the gang's trade.
fn talks(world: &mut World, c: &CrewCfg) {
    let now = world.tick;
    let mut talkers = Vec::new();
    for gang in world.gangs() {
        let Some(g) = world.comp::<Gang>(gang) else { continue };
        for &m in &g.members {
            let Some(s) = world.comp::<Sentence>(m) else { continue };
            if matches!(s.crime, Crime::Vagrancy | Crime::FareEvasion) {
                continue;
            }
            if s.until_tick.saturating_sub(now) < u64::from(c.talk_min_days) * TICKS_PER_DAY {
                continue;
            }
            if world.crew.books.get(&m).is_some_and(|b| b.talk_rolled) {
                continue;
            }
            talkers.push((m, gang));
        }
    }
    for (m, gang) in talkers {
        book(world, m).talk_rolled = true;
        let loyalty = world.comp::<Personality>(m).map_or(0.5, |p| p.loyalty);
        let p = c.talk_p * (1.0 - loyalty).max(0.0);
        if hash(world, PURPOSE_TALK, m, now / TICKS_PER_DAY) >= p {
            continue;
        }
        let (leader, members) =
            world.comp::<Gang>(gang).map(|g| (g.leader, g.members.clone())).unwrap_or((None, Vec::new()));
        let named = leader
            .filter(|&l| l != m)
            .or_else(|| members.iter().copied().find(|&x| x != m && !world.has::<Sentence>(x)));
        let Some(named) = named else { continue };
        book(world, m).talked = true;
        law::file_report(world, Crime::Extortion, named, Some(m));
        let text = format!(
            "{} talked in the cells: named {} of {} for Extortion",
            world.name_of(m),
            world.name_of(named),
            gang_name(world, gang)
        );
        world.push_event(EventKind::Crew, &[m, named, gang], text);
        ownership::jobs2_row(world).crew_talked += 1;
    }
}

fn post_gang(world: &mut World, c: &CrewCfg, gang: EntityId) {
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (order, leader, raid_at, members) = (g.order, g.leader, g.raid_at, g.members.clone());
    let Some(hideout) = world.hideout_of(gang) else { return };
    if world.comp::<Gang>(gang).is_some_and(|g| g.is_sacked(world.tick)) {
        return;
    }
    let mut free: Vec<EntityId> =
        members.iter().copied().filter(|&m| Some(m) != leader && free_for_duty(world, m)).collect();
    // A fair rota: a keyed shuffle a day.
    let day = world.day();
    free.sort_by(|&a, &b| {
        hash(world, PURPOSE_PICK, a, day).total_cmp(&hash(world, PURPOSE_PICK, b, day)).then(a.cmp(&b))
    });
    // Court: the body leader summons by name.
    if let (Some(_), Some(lc)) = (crate::systems::leaders::body_leader(world, gang), world.config.leaders.clone()) {
        let mut n = 0;
        let mut i = 0;
        while i < free.len() && n < usize::from(c.court_summons) {
            let m = free[i];
            if in_reach(world, c, m, hideout) {
                free.remove(i);
                post(world, c, m, gang, DutyKind::Court, hideout, u8::try_from(lc.court_hour).unwrap_or(20));
                n += 1;
            } else {
                i += 1;
            }
        }
    }
    // The order's duty.
    let window_end = world.tick + TICKS_PER_DAY;
    let raid_today = order.is_raid() && raid_at.is_some_and(|t| t < window_end);
    if raid_today {
        let hour = raid_at.map_or(c.guard_hour, |t| ((t % TICKS_PER_DAY) / TICKS_PER_HOUR) as u8);
        // Pressure pass round 3, S1 (OPEN-221): the leader too says so when
        // he leaves the march to his crew.
        if let Some(l) = leader.filter(|&l| free_for_duty(world, l)) {
            muster_reach(world, c, l, gang);
        }
        for m in std::mem::take(&mut free) {
            if muster_reach(world, c, m, gang) {
                post(world, c, m, gang, DutyKind::Muster, hideout, hour);
            }
        }
    } else {
        let kind = match order {
            Order::Contest | Order::Retaliate => Some(DutyKind::Guard),
            Order::Expand | Order::Squat | Order::Harvest | Order::VirtRaid | Order::Job => Some(DutyKind::Watch),
            _ => None,
        };
        if let Some(kind) = kind {
            let slots = paid_slots(world, c, gang, kind, duty_slots(c, members.len()));
            let mut n = 0;
            let mut i = 0;
            while i < free.len() && n < slots {
                let m = free[i];
                let place = if kind == DutyKind::Guard { Some(hideout) } else { watch_place(world, c, gang, m) };
                match place.filter(|&p| in_reach(world, c, m, p)) {
                    Some(p) => {
                        free.remove(i);
                        let hour = if kind == DutyKind::Guard { c.guard_hour } else { c.watch_hour };
                        post(world, c, m, gang, kind, p, hour);
                        n += 1;
                    }
                    None => i += 1,
                }
            }
        }
    }
    // Rest days: the crew.
    for m in free {
        if works_today(world, m) || hash(world, PURPOSE_CREW, m, day) >= c.crew_p {
            continue;
        }
        let Some(p) = crew_place(world, c, gang, m) else { continue };
        post(world, c, m, gang, DutyKind::Crew, p, c.crew_hour);
    }
}

/// R5 (OPEN-214): a raid ordered after midnight's duties were posted, its
/// muster before the next midnight: the members free and holding no duty
/// today are called to it now (V10: a Raid ordered at 06:58 scored ~0.3 on
/// courage and loyalty alone, with no muster posted).
pub fn raid_ordered(world: &mut World, gang: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (leader, raid_at, members) = (g.leader, g.raid_at, g.members.clone());
    let Some(at) = raid_at.filter(|_| g.order.is_raid()) else { return };
    let midnight = (world.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    if at >= midnight {
        return;
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let hour = ((at % TICKS_PER_DAY) / TICKS_PER_HOUR) as u8;
    let today = world.day();
    for m in members {
        if Some(m) == leader || !free_for_duty(world, m) {
            continue;
        }
        if world.crew.duties.get(&m).is_some_and(|d| d.day == today) {
            continue;
        }
        if !muster_reach(world, &c, m, gang) {
            continue;
        }
        post(world, &c, m, gang, DutyKind::Muster, hideout, hour);
    }
}

/// R5 (OPEN-214): the watch or guard slots a gang can pay today (its
/// treasury over `pay_reserve`, `duty_pay` a slot), at most `slots`. A gang
/// that can pay none stands its duty down (told once, and again when it
/// pays): V10's "unpaid: the gang is broke" every night on the same watch.
fn paid_slots(world: &mut World, c: &CrewCfg, gang: EntityId, kind: DutyKind, slots: usize) -> usize {
    let spare = (world.purse(Some(gang)) - c.pay_reserve).max(0);
    let can = if c.duty_pay > 0 { usize::try_from(spare / c.duty_pay).unwrap_or(usize::MAX) } else { slots };
    let n = slots.min(can);
    let today = world.day();
    let was = world.crew.stood_down.contains_key(&gang);
    let gname = gang_name(world, gang);
    let mut actors = vec![gang];
    actors.extend(world.comp::<Gang>(gang).and_then(|g| g.leader));
    if n == 0 && slots > 0 && !was {
        world.crew.stood_down.insert(gang, today);
        let text = format!(
            "{gname} stood its {} down: the treasury ({}c) cannot pay a {}c duty",
            kind.label(),
            world.purse(Some(gang)),
            c.duty_pay
        );
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_stood_down += 1;
    } else if n > 0 && was {
        world.crew.stood_down.remove(&gang);
        let text = format!("{gname} can pay a {} again: {n} posted", kind.label());
        world.push_event(EventKind::Crew, &actors, text);
    }
    n
}

/// R5 (OPEN-214): the Raid goal's duty flat: `muster_flat` for a member
/// posted to today's muster who did not choose the shift; 0 otherwise.
pub fn muster_flat(world: &World, id: EntityId) -> f32 {
    let Some(c) = cfg(world) else { return 0.0 };
    match world.crew.duties.get(&id) {
        Some(d) if d.kind == DutyKind::Muster && !d.chose_job && world.gang_of(id) == Some(d.gang) => c.muster_flat,
        _ => 0.0,
    }
}

/// R5 (OPEN-214): `plan` holds against anything but danger or starving
/// (`fixes::committed`): a recruit's try-out (the walk there and the half
/// hour; V10: a 254-min walk, then Sleep at the door 14 minutes in), and a
/// raid's muster until the march (V10: both musters cut by Sleep, 0
/// brawls), unless the member chose the shift over it at midnight. A try-out
/// gives way to the shift (`winner` Work), a raid only to the chosen one.
pub fn holds(world: &World, id: EntityId, plan: &crate::goap::Plan, winner: crate::components::GoalKind) -> bool {
    if !on(world) {
        return false;
    }
    match plan.goal {
        crate::components::GoalKind::JoinGang => {
            !world.has::<GangMember>(id) && winner != crate::components::GoalKind::Work
        }
        crate::components::GoalKind::Raid => {
            crate::systems::raid::raid_pending(world, id)
                && !crate::systems::raid::raid_done(world, id)
                && !world.crew.duties.get(&id).is_some_and(|d| d.kind == DutyKind::Muster && d.chose_job)
        }
        _ => false,
    }
}

/// R5 (OPEN-214): a full door does not keep the crew out: a member into his
/// own gang's Hideout, a member to today's duty place, a recruit to his
/// try-out. The Hideout's capacity is its beds; V10's musters, courts and
/// rest nights at a full Hideout failed `BuildingFull` at the door.
pub fn capacity_exempt(world: &World, agent: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    if let Some(g) = world.gang_of(agent) {
        if world.hideout_of(g) == Some(b) || world.crew.duties.get(&agent).is_some_and(|d| d.place == b) {
            return true;
        }
    }
    world
        .comp::<crate::components::Brain>(agent)
        .and_then(|br| br.plan.as_ref())
        .is_some_and(|p| p.goal == crate::components::GoalKind::JoinGang && p.target == Some(b))
}

/// A crew Visit arrived (`household::credit`), or a Statistical one at its
/// hour: the duty kept, the pay, the evening's company, the welcome.
pub fn arrived(world: &mut World, id: EntityId, place: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(d) = world.crew.duties.get(&id).cloned() else { return };
    if d.place != place || d.kept || world.gang_of(id) != Some(d.gang) {
        return;
    }
    if let Some(x) = world.crew.duties.get_mut(&id) {
        x.kept = true;
    }
    let gname = gang_name(world, d.gang);
    let pname = world.name_of(place);
    let crew = company(world, id, d.gang, place);
    let names = names_of(world, &crew);
    match d.kind {
        DutyKind::Watch | DutyKind::Guard => {
            let pay = pay(world, &c, id, d.gang);
            let verb = if d.kind == DutyKind::Watch { "stood watch at" } else { "stood guard at" };
            let with = if crew.is_empty() { String::new() } else { format!(" with {names}") };
            let text = format!("{} {verb} {pname} for {gname}{with} ({pay})", world.name_of(id));
            world.push_event(EventKind::Crew, &[id, d.gang, place], text);
            ownership::jobs2_row(world).crew_duty_kept += 1;
        }
        DutyKind::Court => {
            let today = world.day() as u32;
            let over = world.crew.failed_courts.get(&d.gang).is_some_and(|v| v.contains(&today));
            let text = if over {
                format!("{} came to {gname}'s court at {pname} too late: it was over", world.name_of(id))
            } else {
                format!("{} answered {gname}'s summons to court at {pname}", world.name_of(id))
            };
            world.push_event(EventKind::Crew, &[id, d.gang, place], text);
            ownership::jobs2_row(world).crew_duty_kept += 1;
        }
        DutyKind::Crew => {
            let text = if crew.is_empty() {
                format!("{} went to the crew at {pname} on a day off; nobody else came", world.name_of(id))
            } else {
                format!("{} spent a day off with the crew at {pname}: {names} there", world.name_of(id))
            };
            for &o in &crew {
                crate::systems::social::interacted(world, id, o);
            }
            if !crew.is_empty() {
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.belonging = (n.belonging + 0.3).min(1.0);
                }
            }
            let mut actors = vec![id, d.gang, place];
            actors.extend(crew.iter().copied());
            world.push_event(EventKind::Crew, &actors, text);
            ownership::jobs2_row(world).crew_rest_nights += 1;
        }
        DutyKind::Welcome => welcome(world, &c, id, d.gang, place, &crew, walked),
        DutyKind::Muster => {}
    }
}

/// The gang-mates at `place` now: bodies inside; with none, the members
/// whose own duty there today was kept (the off-screen crew).
fn company(world: &World, id: EntityId, gang: EntityId, place: EntityId) -> Vec<EntityId> {
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    members
        .into_iter()
        .filter(|&m| m != id)
        .filter(|&m| {
            presence::body_at(world, m, place) || world.crew.duties.get(&m).is_some_and(|d| d.place == place && d.kept)
        })
        .take(4)
        .collect()
}

fn names_of(world: &World, ids: &[EntityId]) -> String {
    ids.iter().map(|&x| world.name_of(x)).collect::<Vec<_>>().join(", ")
}

/// The duty's pay, from the treasury above `pay_reserve`, less the bail
/// owed. Returns the line's words.
fn pay(world: &mut World, c: &CrewCfg, id: EntityId, gang: EntityId) -> String {
    let spare = world.purse(Some(gang)) - c.pay_reserve;
    if spare < c.duty_pay || c.duty_pay <= 0 {
        return "unpaid: the gang is broke".to_string();
    }
    let owed = world.crew.books.get(&id).map_or(0, |b| b.owed);
    let kept = owed.min(c.duty_pay);
    if kept > 0 {
        if let Some(b) = world.crew.books.get_mut(&id) {
            b.owed -= kept;
        }
    }
    let paid = ownership::pay(world, Some(gang), Some(id), c.duty_pay - kept, Flow::Tribute);
    ownership::jobs2_row(world).crew_duty_pay += paid;
    crate::systems::gang::note_paid(world, id);
    if kept > 0 {
        format!("paid {paid}c, {kept}c kept against his bail")
    } else {
        format!("paid {paid}c")
    }
}

/// Round 3 S2 (OPEN-222): a member who sat the muster to its departure
/// (`raid::depart`) kept it: a march cut short on the way (Sleep, a
/// street cut) is no miss (`raider_39544` mustered 120 min and was marked
/// "missed" when only the brawl's present counted).
pub fn sat_muster(world: &mut World, gang: EntityId, m: EntityId) {
    if !on(world) {
        return;
    }
    if let Some(d) = world.crew.duties.get_mut(&m) {
        if d.gang == gang && d.kind == DutyKind::Muster {
            d.kept = true;
        }
    }
}

/// The raid's muster kept by those at the door (`raid::resolve`).
pub fn mustered(world: &mut World, gang: EntityId, present: &[EntityId]) {
    if !on(world) {
        return;
    }
    let today = world.day();
    world.crew.raided_day.insert(gang, today);
    for &m in present {
        if let Some(d) = world.crew.duties.get_mut(&m) {
            if d.gang == gang && d.kind == DutyKind::Muster {
                d.kept = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Raids write a result
// ---------------------------------------------------------------------------

/// What a raid's brawl left (`raid::brawl`, `raid::corp_brawl`).
pub struct RaidLeft<'a> {
    pub gang: EntityId,
    /// The defending gang (a rival Hideout), `None` at a corp's door.
    pub rival: Option<EntityId>,
    pub place: &'a str,
    pub raiders: &'a [EntityId],
    pub defenders: &'a [EntityId],
    pub raiders_standing: &'a [EntityId],
    pub defenders_standing: &'a [EntityId],
    pub won: bool,
    pub coins: i64,
    pub food: u32,
}

/// The raid's result line for every raider and defender, the wounded named,
/// the crews' grudges, the repelled raid answered, the next orders.
pub fn raid_result(world: &mut World, r: RaidLeft) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    let gname = gang_name(world, r.gang);
    let rname = r.rival.map_or_else(|| r.place.to_string(), |x| gang_name(world, x));
    let beaten_def: Vec<EntityId> = r.defenders.iter().copied().filter(|d| !r.defenders_standing.contains(d)).collect();
    let beaten_raid: Vec<EntityId> = r.raiders.iter().copied().filter(|x| !r.raiders_standing.contains(x)).collect();
    let mut wounded = Vec::new();
    let mut dead = Vec::new();
    for &x in beaten_def.iter().chain(beaten_raid.iter()) {
        if !world.has::<crate::components::Brain>(x) {
            continue;
        }
        if law::living(world, x) {
            wounded.push(x);
        } else {
            dead.push(x);
        }
    }
    // The crews' grudges: the beaten on the other gang, the losing leader
    // on the winner.
    for &d in &beaten_def {
        if law::living(world, d) {
            crate::systems::grudges::add(world, d, r.gang, crate::word::GrudgeCause::Assaulted, c.raid_grudge, 0);
        }
    }
    if let Some(rival) = r.rival {
        for &x in &beaten_raid {
            if law::living(world, x) {
                crate::systems::grudges::add(world, x, rival, crate::word::GrudgeCause::Assaulted, c.raid_grudge, 0);
            }
        }
        let (loser, winner) = if r.won { (rival, r.gang) } else { (r.gang, rival) };
        if let Some(l) = world.comp::<Gang>(loser).and_then(|g| g.leader) {
            crate::systems::grudges::add(
                world,
                l,
                winner,
                crate::word::GrudgeCause::Assaulted,
                c.raid_leader_grudge,
                0,
            );
        }
        // A repelled raid is answered.
        if !r.won {
            if let Some(g) = world.comp_mut::<Gang>(rival) {
                g.retaliate_until = Some(now + c.repelled_retaliate_days * TICKS_PER_DAY);
                if g.order != Order::Retaliate {
                    g.retaliate_on = Some(r.gang);
                }
            }
            crate::systems::gang::push_shock(world, rival, Shock::Raided);
            crate::systems::faction::rethink(world, rival);
        }
    }
    // The wounded: the lost fight is in their memory (phase 2's care reads
    // it); the line names where the gang-hurt go.
    let wounded_text =
        if wounded.is_empty() { String::new() } else { format!("; wounded: {}", names_of(world, &wounded)) };
    let dead_text = if dead.is_empty() { String::new() } else { format!("; dead: {}", names_of(world, &dead)) };
    let took = match (r.won, r.rival.is_some()) {
        (true, true) => format!("; {gname} took {}c and {} food from the Hideout", r.coins, r.food),
        (true, false) => format!("; the till smashed, {}c and {} food taken", r.coins, r.food),
        (false, _) => "; nothing taken".to_string(),
    };
    let next = |w: &World, g: EntityId| {
        w.comp::<Gang>(g).map_or_else(String::new, |x| {
            let o = x.pending_order.as_ref().map_or(x.order, |p| p.order);
            format!("{} next: {o:?}", x.name)
        })
    };
    let mut nexts = vec![next(world, r.gang)];
    if let Some(rv) = r.rival {
        nexts.push(next(world, rv));
    }
    let verdict = if r.won { format!("{gname} won") } else { format!("{rname} held") };
    let text = format!(
        "Raid on {}: {verdict} ({} raiders, {} defenders){took}{wounded_text}{dead_text}; {}",
        r.place,
        r.raiders.len(),
        r.defenders.len(),
        nexts.join(", ")
    );
    let mut actors: Vec<EntityId> = r.raiders.iter().chain(r.defenders.iter()).copied().collect();
    actors.sort_unstable();
    actors.dedup();
    let fought = actors.clone();
    actors.push(r.gang);
    actors.extend(r.rival);
    world.push_event(EventKind::Raid, &actors, text);
    let summary =
        if r.won { format!("{gname} won{took}{wounded_text}") } else { format!("{rname} held{wounded_text}") };
    world.crew.last_raid.insert(r.gang, (now, format!("{} ({summary})", r.place), fought));
    let row = ownership::jobs2_row(world);
    row.crew_raid_results += 1;
    row.crew_raid_wounded += wounded.len() as u32;
}

/// A marcher at the door after the brawl was resolved (`raid::resolve`
/// found no raid standing): the result as he finds it, within two hours.
pub fn late_to_raid(world: &mut World, actor: EntityId, gang: EntityId) {
    if !on(world) {
        return;
    }
    let Some((t, what, fought)) = world.crew.last_raid.get(&gang).cloned() else { return };
    if world.tick.saturating_sub(t) > 2 * TICKS_PER_HOUR || fought.binary_search(&actor).is_ok() {
        return;
    }
    let text = format!("{} reached {what} after it was over", world.name_of(actor));
    world.push_event(EventKind::Raid, &[actor, gang], text);
}

// ---------------------------------------------------------------------------
// Court's quorum and the gang that cannot muster
// ---------------------------------------------------------------------------

/// Court ended at the Hideout (`leaders::court_done`): the summoned there
/// make the quorum, else the court fails with the reasons.
pub fn court_held(world: &mut World, gang: EntityId, leader: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(hideout) = world.hideout_of(gang) else { return };
    let today = world.day();
    let summoned: Vec<(EntityId, Duty)> = world
        .crew
        .duties
        .iter()
        .filter(|(_, d)| d.gang == gang && d.kind == DutyKind::Court && d.day == today)
        .map(|(&m, d)| (m, d.clone()))
        .collect();
    if summoned.is_empty() {
        return;
    }
    let mut came = Vec::new();
    let mut reasons = Vec::new();
    for (m, d) in &summoned {
        if d.kept || presence::body_at(world, *m, hideout) {
            came.push(*m);
            if let Some(x) = world.crew.duties.get_mut(m) {
                x.kept = true;
            }
        } else {
            let why = if world.has::<Sentence>(*m) {
                "in the cells"
            } else if d.chose_job {
                "at work"
            } else if world.gang_of(*m) != Some(gang) {
                "gone"
            } else {
                "did not come"
            };
            reasons.push(format!("{} ({why})", world.name_of(*m)));
        }
    }
    let lname = world.name_of(leader);
    if came.len() < usize::from(c.court_quorum) {
        let fails = {
            let day = today as u32;
            let w = c.fold_window_days;
            let v = world.crew.failed_courts.entry(gang).or_default();
            v.retain(|&x| x + w > day);
            v.push(day);
            v.len()
        };
        let text = format!(
            "{lname}'s court at {} failed: nobody came (summoned {}; {fails} failed in {} days)",
            world.name_of(hideout),
            reasons.join(", "),
            c.fold_window_days
        );
        world.push_event(EventKind::Crew, &[leader, gang, hideout], text);
        ownership::jobs2_row(world).crew_court_failed += 1;
    } else {
        let absent = if reasons.is_empty() { String::new() } else { format!("; absent {}", reasons.join(", ")) };
        let text = format!("{lname} held court at {}: {} came{absent}", world.name_of(hideout), names_of(world, &came));
        let mut actors = vec![leader, gang, hideout];
        actors.extend(came.iter().copied());
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_court_quorum += 1;
    }
}

/// A gang with `fold_fails` failed courts in the window takes a rival's
/// offer or folds.
fn fold_check(world: &mut World, c: &CrewCfg, gang: EntityId, day: u32) {
    let fails =
        world.crew.failed_courts.get(&gang).map_or(0, |v| v.iter().filter(|&&x| x + c.fold_window_days > day).count());
    if (fails as u32) < c.fold_fails.max(1) {
        return;
    }
    world.crew.failed_courts.remove(&gang);
    let Some(g) = world.comp::<Gang>(gang) else { return };
    if g.members.is_empty() {
        return;
    }
    let (leader, creed, members) = (g.leader, g.creed, g.members.clone());
    let pride = leader.and_then(|l| world.comp::<Personality>(l)).map_or(0.5, |p| p.pride);
    let buyer = if creed.is_some() { None } else { absorber(world, gang) };
    let wa = buyer.map_or(0.0, |_| c.w_absorb * (1.5 - pride).max(0.0));
    let wf = c.w_fold * (0.5 + pride);
    let u = hash(world, PURPOSE_FOLD, gang, u64::from(day)) * (wa + wf);
    let gname = gang_name(world, gang);
    if let (Some(b), true) = (buyer, u < wa) {
        let bname = gang_name(world, b);
        let offer = leader.map_or(0, |l| ownership::pay(world, Some(b), Some(l), c.absorb_offer, Flow::Tribute));
        let chest = world.purse(Some(gang)).max(0);
        let moved = ownership::pay(world, Some(gang), Some(b), chest, Flow::Tribute);
        let reason = format!("absorbed into {bname}");
        // The leader last: his colours change with the rest.
        let mut order: Vec<EntityId> = members.iter().copied().filter(|&m| Some(m) != leader).collect();
        order.extend(leader);
        for m in &order {
            crate::systems::gang::leave(world, *m, &reason);
            if law::living(world, *m) {
                crate::systems::gang::enlist(world, *m, b);
            }
        }
        let lname = leader.map_or_else(|| "nobody".to_string(), |l| world.name_of(l));
        let text = format!(
            "{gname} could not muster and took {bname}'s offer: {} members and {moved}c went over, {offer}c to {lname}",
            order.len()
        );
        let mut actors = vec![gang, b];
        actors.extend(order.iter().copied());
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_absorbed += 1;
    } else {
        let chest = world.purse(Some(gang)).max(0);
        let each = if members.is_empty() { 0 } else { chest / members.len() as i64 };
        if each > 0 {
            for &m in &members {
                ownership::pay(world, Some(gang), Some(m), each, Flow::Tribute);
            }
        }
        let mut order: Vec<EntityId> = members.iter().copied().filter(|&m| Some(m) != leader).collect();
        order.extend(leader);
        for m in &order {
            crate::systems::gang::leave(world, *m, "folded");
        }
        let text = format!("{gname} could not muster and folded: {} members walk, {each}c each", order.len());
        let mut actors = vec![gang];
        actors.extend(order.iter().copied());
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_folded += 1;
    }
}

/// The gang that would take `gang` in: its rival if bigger, else the
/// biggest gang with no creed whose Hideout is nearest.
fn absorber(world: &World, gang: EntityId) -> Option<EntityId> {
    let own = world.comp::<Gang>(gang)?.members.len();
    let at = world.hideout_of(gang).and_then(|h| world.comp::<Building>(h)).map(|b| b.door)?;
    let ok = |w: &World, g: EntityId| {
        g != gang && w.comp::<Gang>(g).is_some_and(|x| x.creed.is_none() && x.members.len() >= own.max(2))
    };
    if let Some(r) = world.rival_of(gang).filter(|&r| ok(world, r)) {
        return Some(r);
    }
    world
        .gangs()
        .into_iter()
        .filter(|&g| ok(world, g))
        .filter_map(|g| {
            let d = world.hideout_of(g).and_then(|h| world.comp::<Building>(h)).map(|b| b.door)?;
            Some((d.manhattan(at), g))
        })
        .min()
        .map(|(_, g)| g)
}

// ---------------------------------------------------------------------------
// Release and return
// ---------------------------------------------------------------------------

/// `gang::on_member_arrested`: the sentence's start.
pub fn on_arrested(world: &mut World, id: EntityId) {
    if !on(world) || world.gang_of(id).is_none() {
        return;
    }
    let now = world.tick;
    let b = book(world, id);
    b.jailed_at = Some(now);
    b.talked = false;
    b.talk_rolled = false;
}

/// The gang paid `coins` of bail for `id` (`protection::daily`).
pub fn note_bail(world: &mut World, id: EntityId, coins: i64) {
    if cfg(world).is_some_and(|c| c.bail_debt) && coins > 0 {
        book(world, id).owed += coins;
    }
}

/// Phase 5a's hook: `id` talked to the law inside (a jail contact).
pub fn note_talked(world: &mut World, id: EntityId) {
    if on(world) && world.gang_of(id).is_some() {
        book(world, id).talked = true;
    }
}

/// He talked: his own flag, or a report he filed names a gang-mate since
/// his sentence began.
fn talked(world: &World, id: EntityId, gang: EntityId) -> bool {
    let b = world.crew.books.get(&id);
    if b.is_some_and(|b| b.talked) {
        return true;
    }
    let since = b.and_then(|b| b.jailed_at).unwrap_or(0);
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    world
        .crime_reports()
        .iter()
        .any(|r| r.witness == Some(id) && r.tick >= since && r.suspect != id && members.contains(&r.suspect))
}

/// `gang::on_member_released`: the gang's side of the release. A talker is
/// met by the toughest; the rest walk to the Hideout to be welcomed.
pub fn on_released(world: &mut World, id: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(gang) = world.gang_of(id) else { return };
    if talked(world, id, gang) {
        send_reckoning(world, &c, id, gang, CallWhy::Talked);
        return;
    }
    // A night in the cells is no homecoming.
    let now = world.tick;
    let served = world.crew.books.get(&id).and_then(|b| b.jailed_at).map_or(0, |t| now.saturating_sub(t));
    if served < TICKS_PER_DAY {
        if let Some(b) = world.crew.books.get_mut(&id) {
            b.jailed_at = None;
            b.talk_rolled = false;
        }
        return;
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let hour = (world.tick_of_day() / TICKS_PER_HOUR as u16) as u8 + c.welcome_after_hours;
    let day = world.day();
    let duty = Duty { gang, kind: DutyKind::Welcome, place: hideout, hour, day, chose_job: false, kept: false };
    world.crew.duties.insert(id, duty);
    if !crate::systems::household::on(world) || hour >= 22 {
        arrived(world, id, hideout, false);
        return;
    }
    world.visits.insert(id, PendingVisit { kin: hideout, why: VisitWhy::Crew, hour });
}

fn welcome(
    world: &mut World,
    c: &CrewCfg,
    id: EntityId,
    gang: EntityId,
    place: EntityId,
    crew: &[EntityId],
    walked: bool,
) {
    let now = world.tick;
    let served =
        world.crew.books.get(&id).and_then(|b| b.jailed_at).map_or(0, |t| now.saturating_sub(t) / TICKS_PER_DAY);
    let owed = world.crew.books.get(&id).map_or(0, |b| b.owed);
    let rank_before = world.comp::<GangMember>(id).map_or(0, |x| x.rank);
    if served >= u64::from(c.talk_min_days) {
        book(world, id).standing += c.served_standing;
        crate::systems::gang::recompute_leader(world, gang);
    }
    if let Some(b) = world.crew.books.get_mut(&id) {
        b.jailed_at = None;
        b.talk_rolled = false;
    }
    let rank_after = world.comp::<GangMember>(id).map_or(0, |x| x.rank);
    for &o in crew {
        crate::systems::social::interacted(world, id, o);
    }
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.belonging = (n.belonging + 0.5).min(1.0);
    }
    world.remember(id, crate::components::MemoryKind::Socialised, Some(gang), 0.6, 0.4, false);
    let gname = gang_name(world, gang);
    let who = if crew.is_empty() {
        if walked {
            " (nobody there to meet them)".to_string()
        } else {
            String::new()
        }
    } else {
        format!(" by {}", names_of(world, crew))
    };
    let debt = if owed > 0 { format!("; owes {owed}c for the bail") } else { String::new() };
    let rank = if rank_after > rank_before {
        if rank_after >= 2 {
            "; he leads now".to_string()
        } else {
            "; made lieutenant".to_string()
        }
    } else if served >= u64::from(c.talk_min_days) {
        format!("; {served} days served, nothing said inside")
    } else {
        String::new()
    };
    let text = format!("{} welcomed back to {gname} at {}{who}{debt}{rank}", world.name_of(id), world.name_of(place));
    let mut actors = vec![id, gang, place];
    actors.extend(crew.iter().copied());
    world.push_event(EventKind::Crew, &actors, text);
    let row = ownership::jobs2_row(world);
    row.crew_welcomes += 1;
    if rank_after > rank_before {
        row.crew_promoted += 1;
    }
}
