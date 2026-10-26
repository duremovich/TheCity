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

const PURPOSE_VISIT: u64 = 0x5649_0001;
const PURPOSE_PAYER: u64 = 0x5649_0002;
const PURPOSE_ASSAULT: u64 = 0x5649_0003;
const PURPOSE_SWEEP: u64 = 0x5649_0004;
/// Lives at work phase 4 (§ 2.3): the client among the bodies near a pitch.
const PURPOSE_CLIENT: u64 = 0x5649_0005;

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
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl ViceState {
    pub fn is_default(&self) -> bool {
        *self == ViceState::default()
    }
}

/// A Vice venue kind.
pub fn is_vice_kind(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::StripClub | BuildingKind::Brothel)
}

/// A visit's price at a Vice venue by its tier (`venue_price`'s base).
pub fn visit_price(world: &World, kind: BuildingKind, tier: u8) -> i64 {
    let t = usize::from(tier.min(2));
    match kind {
        BuildingKind::StripClub => world.config.vice.strip_price[t],
        BuildingKind::Brothel => world.config.vice.brothel_price[t],
        _ => 0,
    }
}

/// The fun a visit gives (`leisure::gain_at`).
pub fn gain(world: &World, kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::StripClub => world.config.vice.gain_strip,
        BuildingKind::Brothel => world.config.vice.gain_brothel,
        _ => 0.0,
    }
}

/// The ladder's weight on a Brothel: `1 + intimacy_pull × (1 − intimacy)`
/// (1 elsewhere).
pub fn score_mult(world: &World, id: EntityId, kind: BuildingKind) -> f32 {
    if kind != BuildingKind::Brothel {
        return 1.0;
    }
    let i = world.comp::<Needs>(id).map_or(0.5, |n| n.intimacy);
    1.0 + world.config.vice.intimacy_pull * (1.0 - i).clamp(0.0, 1.0)
}

/// The tipped worker on duty at `b` (the lowest id): a `tipped` row's
/// staff on shift now, inside it (a Statistical one counts as there), not held.
pub fn tipped_on_duty(world: &World, b: EntityId) -> Option<EntityId> {
    tipped_for(world, b, EntityId::NONE)
}

/// Shadow V4 bug 7: the tipped worker on duty a visit's tip goes to, turn
/// about by the visitor and the hour (every tip went to the lowest id on
/// duty: 56 c against 0 c on the same shifts).
fn tipped_for(world: &World, b: EntityId, visitor: EntityId) -> Option<EntityId> {
    let tod = world.tick_of_day();
    let on_duty: Vec<EntityId> = world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| {
            world.comp::<Job>(s).is_some_and(|j| {
                j.on_shift(tod) && crate::systems::trades::row_of(world, j.role).is_some_and(|r| r.tipped)
            }) && !world.has::<Sentence>(s)
                && world.comp::<Brain>(s).is_some_and(|br| {
                    br.lod == Lod::Statistical || world.comp::<Position>(s).is_some_and(|p| p.building == Some(b))
                })
        })
        .collect();
    if on_duty.is_empty() {
        return None;
    }
    let turn = u64::from(visitor.index) + world.tick / crate::time::TICKS_PER_HOUR;
    Some(on_duty[(turn % on_duty.len() as u64) as usize])
}

fn add_intimacy(world: &mut World, id: EntityId, gain: f32) {
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.intimacy = (n.intimacy + gain).min(1.0);
    }
}

/// A paid venue visit done (`leisure::on_complete`'s `Enjoy`, the
/// Statistical evening): at a StripClub or a Brothel, the counts, the
/// Brothel's intimacy and the tip; at the Lounge with a companion on duty,
/// a companion's visit (intimacy, the tip).
pub fn on_visit(world: &mut World, visitor: EntityId, b: EntityId, price: i64) {
    if !on(world) {
        return;
    }
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let worker = tipped_for(world, b, visitor);
    match kind {
        BuildingKind::StripClub => {
            let row = ownership::jobs2_row(world);
            row.strip_visits += 1;
            row.vice_revenue += price;
        }
        // Jobs & lives phase 5: counted by `dollhouse::on_visit`; no tip.
        BuildingKind::Dollhouse => return,
        BuildingKind::Brothel => {
            let row = ownership::jobs2_row(world);
            row.brothel_visits += 1;
            row.vice_revenue += price;
            add_intimacy(world, visitor, world.config.vice.intimacy_gain);
        }
        BuildingKind::Lounge if worker.is_some() => {
            ownership::jobs2_row(world).companion_visits += 1;
            add_intimacy(world, visitor, world.config.vice.intimacy_gain);
        }
        // Shadow V4 bug 7: a Club's dancer is tipped too (it fell through).
        BuildingKind::Club if worker.is_some() => {}
        _ => return,
    }
    // Lives at work phase 1 (§ 2.1): the tip goes to the server the
    // encounter names (`service::serve`), not the turn-about worker.
    if crate::systems::service::on(world) {
        return;
    }
    let Some(w) = worker.filter(|&w| w != visitor) else { return };
    let tip = ((price as f32 * world.config.vice.tip_share).round() as i64).max(1);
    ownership::pay(world, Some(visitor), Some(w), tip, Flow::Tip);
}

// ---------------------------------------------------------------------------
// The street (on the phase-2 Pitch)
// ---------------------------------------------------------------------------

/// The district's pimp, free and alive (his gang's protection).
fn pimp_of(world: &World, d: DistrictId) -> Option<EntityId> {
    let p = *world.jobs2.vice.pimps.get(&(d.index() as u16))?;
    (crate::systems::law::living(world, p) && !world.has::<Sentence>(p) && world.gang_of(p).is_some()).then_some(p)
}

/// `streetwork::close_the_day`: a street pitch under a pimp pays his gang
/// `pimp_cut` of its day instead of the holding gang's `cut_frac`.
///
/// Lives at work phase 4b (§ 2.3): with `[service] street` on, the cut is
/// the pimp's (`StreetCut::pimp`): `close_the_day` pays him and he passes
/// `pimp_pass_up` of it to the gang the same tick (`pass_up`).
pub fn street_cut(world: &World, p: &Pitch) -> Option<StreetCut> {
    if !on(world) || !is_street_pitch(world, p) {
        return None;
    }
    let pimp = pimp_of(world, p.district)?;
    let gang = world.gang_of(pimp)?;
    let to_pimp = crate::systems::service::street_on(world);
    Some(StreetCut { gang, pimp: to_pimp.then_some(pimp), frac: world.config.vice.pimp_cut })
}

/// `street_cut`'s answer: the gang, the pimp when the cut is his (the
/// street switch on), the share of the pitch's day.
#[derive(Clone, Copy, Debug)]
pub struct StreetCut {
    pub gang: EntityId,
    pub pimp: Option<EntityId>,
    pub frac: f32,
}

/// Lives at work phase 4b (§ 2.3): the pimp, paid his cut of a street
/// pitch's day, passes `pimp_pass_up` of it to his gang (`Flow::StreetCut`,
/// the same tick). Returns the coins passed up.
pub fn pass_up(world: &mut World, pimp: EntityId, gang: EntityId, paid: i64) -> i64 {
    let up = (paid as f32 * world.config.service.pimp_pass_up).round() as i64;
    if up <= 0 {
        return 0;
    }
    let passed = ownership::pay(world, Some(pimp), Some(gang), up, Flow::StreetCut);
    ownership::jobs2_row(world).pimp_pass_up += passed;
    passed
}

/// Lives at work phase 4b (§ 2.3): the pimp of district `d` is present at
/// the pitch on `spot` now: his body (Full or Coarse) out within
/// `pimp_tiles` of it, or, a Statistical pimp (no Job, so no
/// `shift_attended`: the deviation), credited while he follows his gang's
/// order (`gang::following_order`). A presence marks `pimp_seen`.
pub fn pimp_present(world: &mut World, d: DistrictId, spot: TilePos) -> bool {
    let Some(pimp) = pimp_of(world, d) else { return false };
    let here = match world.comp::<Brain>(pimp).map(|b| b.lod) {
        Some(Lod::Statistical) => crate::systems::gang::following_order(world, pimp).is_some(),
        Some(_) => world
            .comp::<Position>(pimp)
            .is_some_and(|p| p.building.is_none() && p.tile.manhattan(spot) <= world.config.service.pimp_tiles),
        None => false,
    };
    if here {
        let today = world.day();
        world.jobs2.vice.pimp_seen.insert(d.index() as u16, today);
    }
    here
}

/// The district a named pimp works (his `pimps` entry), if `id` is one.
pub fn pimp_district(world: &World, id: EntityId) -> Option<DistrictId> {
    let d = world.jobs2.vice.pimps.iter().find(|&(_, &p)| p == id).map(|(&d, _)| d)?;
    world.districts.get(usize::from(d)).map(|x| x.id)
}

/// Lives at work phase 4b (§ 2.3): the street worker whose pitch is the
/// pimp's turn this hour: his district's street pitches in their hours, by
/// owner, the hour's index of them (the round, in turn). `None` off, for a
/// non-pimp, out of the pitch hours.
pub fn round_pitch(world: &World, id: EntityId) -> Option<EntityId> {
    if !on(world) || !crate::systems::service::street_on(world) {
        return None;
    }
    let d = pimp_district(world, id)?;
    pimp_of(world, d).filter(|&p| p == id)?;
    let due: Vec<EntityId> = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.district == d && is_street_pitch(world, p) && crate::systems::streetwork::due(world, p))
        .map(|p| p.owner)
        .collect();
    if due.is_empty() {
        return None;
    }
    let hour = (world.tick / TICKS_PER_HOUR) as usize;
    Some(due[hour % due.len()])
}

/// `gang::gang_work_target` for a pimp's body: the worker of his round's
/// pitch this hour (the GangWork goal's target; the plan is `pimp_plan`).
pub fn pimp_target(world: &World, id: EntityId) -> Option<EntityId> {
    if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
        return None;
    }
    round_pitch(world, id)
}

/// GangWork's scripted plan for a pimp's body in pitch hours: `GoTo(Spot)`
/// (the round's pitch) then `Collect` (an hour at it, `collect_ticks`).
pub fn pimp_plan(world: &mut World, id: EntityId) -> Option<crate::goap::Plan> {
    use crate::components::{ActionInstance, GoalKind};
    use crate::goap::{ActionKind, LocationKey, Plan};
    let worker = pimp_target(world, id)?;
    let spot = world.jobs2.pitches.get(&worker)?.spot;
    world.unwind.insert(
        id,
        crate::living::UnwindPick {
            rung: crate::living::Rung::Free,
            act: ActionKind::Collect,
            venue: None,
            spot: Some(spot),
            score: 0.0,
            stay_in: false,
        },
    );
    let step = |action, target| ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::Collect, Some(worker))];
    Some(Plan { goal: GoalKind::GangWork, target: Some(worker), steps, started_tick: world.tick })
}

/// A pimp's `Collect` at `worker`'s pitch (`leisure::can_start`): `Some`
/// when this is the pimp's collection (his district's street pitch as the
/// target), then whether he stands out within `pimp_tiles` of it.
pub fn collect_can_start(world: &World, id: EntityId, target: Option<EntityId>) -> Option<bool> {
    let worker = target?;
    let p = world.jobs2.pitches.get(&worker)?;
    if !is_street_pitch(world, p) || pimp_district(world, id) != Some(p.district) {
        return None;
    }
    let r = world.config.service.pimp_tiles;
    Some(world.comp::<Position>(id).is_some_and(|x| x.building.is_none() && x.tile.manhattan(p.spot) <= r))
}

/// The pimp's `Collect` runs the hour at the pitch (presence is what
/// protects it); `None` for any other Collect (and off).
pub fn collect_ticks(world: &World, id: EntityId) -> Option<crate::time::Tick> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    pimp_district(world, id).map(|_| TICKS_PER_HOUR)
}

/// A pimp's `Collect` ends (`leisure::on_complete`): the collection, a
/// `Staff` encounter with the worker (`"pimp"`'s row), `pimp_collections`.
/// `false` when it was not the pimp's (the leader's Collect runs on).
pub fn pimp_collect(world: &mut World, id: EntityId, target: Option<EntityId>) -> bool {
    let Some(worker) = target else { return false };
    let Some(p) = world.jobs2.pitches.get(&worker).cloned() else { return false };
    if !is_street_pitch(world, &p) || pimp_district(world, id) != Some(p.district) {
        return false;
    }
    // The pre-view pass phase 4 (P3, OPEN-120): the collection needs both
    // bodies: the worker at his pitch (`streetwork::working`: a body within
    // `pitch_reach` of the spot, a Statistical one on the hour's draw), else
    // no "collected" line in either diary (the next hour's pitch is tried).
    if crate::systems::presence::on(world) && !crate::systems::streetwork::working(world, &p) {
        crate::systems::presence::missed(world);
        return true;
    }
    collection(world, id, worker, p.district);
    true
}

fn collection(world: &mut World, pimp: EntityId, worker: EntityId, d: DistrictId) {
    ownership::jobs2_row(world).pimp_collections += 1;
    let kind = crate::components::ServeKind::Staff;
    crate::systems::service::encounter_post(world, "pimp", pimp, worker, None, d, kind, 0);
    cut_at_meeting(world, pimp, worker);
}

/// The pre-view pass phase 4 (P3, OPEN-119): with `[presence]` on and a
/// body on either side, the pimp's cut is taken when he meets the worker:
/// `pimp_cut` of the take since the last collection (`Pitch.cut_paid`),
/// worker -> pimp (`Flow::StreetCut`), `pimp_pass_up` of it to the gang.
/// A Statistical-only pair keeps the day's close (`street_cut_at_close`).
fn cut_at_meeting(world: &mut World, pimp: EntityId, worker: EntityId) {
    if !crate::systems::presence::on(world) || !body_pair(world, pimp, worker) {
        return;
    }
    let Some(p) = world.jobs2.pitches.get(&worker).cloned() else { return };
    let Some(c) = street_cut(world, &p) else { return };
    let due = p.take_today - p.cut_paid;
    if due <= 0 {
        return;
    }
    let cut = (due as f32 * c.frac).round() as i64;
    let to = c.pimp.unwrap_or(c.gang);
    let paid = ownership::pay(world, Some(worker), Some(to), cut, Flow::StreetCut);
    if let Some(x) = world.jobs2.pitches.get_mut(&worker) {
        x.cut_paid = p.take_today;
        x.cut_to = Some(c.gang);
    }
    let row = ownership::jobs2_row(world);
    row.street_cut += paid;
    row.pimp_cut += paid;
    row.cut_at_meet += 1;
    if to != c.gang {
        pass_up(world, to, c.gang, paid);
    }
}

/// Either side a body (Full or Coarse).
fn body_pair(world: &World, a: EntityId, b: EntityId) -> bool {
    crate::systems::presence::is_body(world, a) || crate::systems::presence::is_body(world, b)
}

/// `streetwork::close_the_day`: does the pimp's cut of this pitch still
/// settle at the day's close? Off, or a Statistical-only pair (the mirror);
/// with a body on either side it was taken at the collections.
pub fn street_cut_at_close(world: &World, p: &Pitch, c: &StreetCut) -> bool {
    !crate::systems::presence::on(world) || !body_pair(world, c.pimp.unwrap_or(c.gang), p.owner)
}

/// A Statistical pimp credited present collects at the pitch whose turn
/// the hour is (`round_pitch`), as a body's `Collect` would.
fn stat_collection(world: &mut World, p: &Pitch) {
    let Some(pimp) = pimp_of(world, p.district) else { return };
    if world.comp::<Brain>(pimp).is_none_or(|b| b.lod != Lod::Statistical) {
        return;
    }
    if round_pitch(world, pimp) == Some(p.owner) {
        collection(world, pimp, p.owner, p.district);
    }
}

/// A Pitch of a `vice` row (the street worker's).
pub fn is_street_pitch(world: &World, p: &Pitch) -> bool {
    world.config.trade(p.trade).is_some_and(|r| r.vice && r.workplace.is_empty())
}

/// `streetwork::close_the_day`'s sweep of a street pitch: never with
/// `street_legal` on; in a `Sweep` district on `p_street_sweep`; with
/// `patrol_sweep` on (phase 5's carry-over), in a district the law patrols
/// or cracks down on with guards allocated, on `p_patrol_sweep` × the law's
/// alertness (M16b's attention). Files `Solicitation` while under the day's
/// cap. Returns whether it is swept (the caller fines and closes it, under
/// `[jobs2] sweeps_per_day`).
pub fn sweep_street(world: &mut World, p: &Pitch) -> bool {
    if world.levers.vice.street_legal {
        return false;
    }
    let d = world.districts.get(p.district.index());
    let sweep = d.is_some_and(|d| d.stance == Stance::Sweep);
    let u = hash(world, PURPOSE_SWEEP, key(p.owner), world.day());
    let swept = if sweep {
        u < world.config.vice.p_street_sweep
    } else if world.config.vice.patrol_sweep
        && d.is_some_and(|d| matches!(d.stance, Stance::Patrol | Stance::Crackdown(_)) && d.guards > 0)
    {
        u < world.config.vice.p_patrol_sweep * crate::systems::attention::alertness_mult(world, None)
    } else {
        false
    };
    if !swept {
        return false;
    }
    if !sweep {
        ownership::jobs2_row(world).patrol_sweeps += 1;
    }
    let cap = crate::systems::weapons::per_capita_day(world, world.config.vice.solicit_per_1000_day);
    if world.jobs2.vice.solicit_today < cap {
        world.jobs2.vice.solicit_today += 1;
        ownership::jobs2_row(world).solicitation += 1;
        crate::systems::law::file_report(world, crate::components::Crime::Solicitation, p.owner, None);
    }
    true
}

/// Hourly: each working street pitch's visit roll and, unprotected, its
/// assault roll. O(street pitches).
fn street_hour(world: &mut World) {
    let cfg = world.config.vice.clone();
    let hour = world.tick / TICKS_PER_HOUR;
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        if !is_street_pitch(world, &p) || !crate::systems::streetwork::working(world, &p) {
            continue;
        }
        let street = crate::systems::service::street_on(world);
        let mut client = None;
        if hash(world, PURPOSE_VISIT, key(o), hour) < cfg.p_street_visit {
            // Lives at work phase 4 (§ 2.3): a body near the pitch first.
            let near = if street { client_near(world, &p, hour) } else { None };
            if near.is_some() {
                ownership::jobs2_row(world).street_clients_near += 1;
            }
            if let Some(payer) = near.or_else(|| street_payer(world, &p, hour)) {
                let paid = ownership::pay(world, Some(payer), Some(o), cfg.street_price, Flow::Leisure);
                if paid > 0 {
                    add_intimacy(world, payer, cfg.intimacy_gain);
                    if let Some(x) = world.jobs2.pitches.get_mut(&o) {
                        x.take_today += paid;
                    }
                    let row = ownership::jobs2_row(world);
                    row.street_visits += 1;
                    row.street_take += paid;
                    if street {
                        client = Some(payer);
                        let kind = crate::components::ServeKind::Client;
                        crate::systems::service::encounter(world, o, payer, None, p.district, kind, 0);
                    }
                }
            }
        }
        // Lives at work phase 4b (§ 2.3): the pimp's presence, not his
        // freedom; a Statistical pimp's round is credited here.
        let pimp = if street {
            let here = pimp_present(world, p.district, p.spot);
            if here {
                stat_collection(world, &p);
            }
            here
        } else {
            pimp_of(world, p.district).is_some()
        };
        let protected = pimp || world.comp::<Kit>(o).is_some_and(|k| k.weapon > 0);
        if protected {
            continue;
        }
        let fear = world.districts.get(p.district.index()).map_or(0.0, |d| d.fear);
        if hash(world, PURPOSE_ASSAULT, key(o), hour) < cfg.p_assault_street * (0.5 + fear) {
            street_assault(world, o, &p, hour, client);
        }
    }
}

/// Lives at work phase 4 (§ 2.3, the client): a body out near the pitch
/// (within `client_tiles` of its spot, Full or Coarse, in no building):
/// an adult, free, not the worker, with the visit's wants and twice the
/// price (`street_payer`'s rule), the lowest hash of the hour (D6).
fn client_near(world: &mut World, p: &Pitch, hour: u64) -> Option<EntityId> {
    let r = world.config.service.client_tiles;
    let near = world.agents_near(p.spot, r, None);
    let cfg = &world.config.vice;
    near.into_iter()
        .filter(|&id| {
            id != p.owner
                && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
                && world.comp::<Position>(id).is_some_and(|x| x.building.is_none())
                && crate::systems::demography::is_adult(world, id)
                && !world.has::<Sentence>(id)
                && crate::systems::law::living(world, id)
                && world.comp::<Needs>(id).is_some_and(|n| n.intimacy < cfg.street_intimacy && n.hunger >= 0.5)
                && world.comp::<Wallet>(id).is_some_and(|w| w.coins >= 2 * cfg.street_price)
        })
        .map(|id| (hash(world, PURPOSE_CLIENT, key(p.owner) ^ key(id), hour).to_bits(), id))
        .min()
        .map(|(_, id)| id)
}

/// A resident adult of the pitch's district (three draws by a hash) with
/// the price, `intimacy` under `street_intimacy` and `hunger` at 0.5 or
/// more (fed first), not the worker.
fn street_payer(world: &World, p: &Pitch, hour: u64) -> Option<EntityId> {
    let res = &world.districts.get(p.district.index())?.residents;
    if res.is_empty() {
        return None;
    }
    let cfg = &world.config.vice;
    // The pre-view pass phase 4 (P3): a body pays only in person
    // (`client_near`); the drawn payer is an off-screen resident.
    let presence = crate::systems::presence::on(world);
    (0..3u64).find_map(|k| {
        let u = hash(world, PURPOSE_PAYER, key(p.owner), hour * 4 + k);
        let id = res[((u * res.len() as f32) as usize).min(res.len() - 1)];
        (id != p.owner
            && !(presence && crate::systems::presence::is_body(world, id))
            && crate::systems::demography::is_adult(world, id)
            && !world.has::<Sentence>(id)
            // A hungry body buys food first (no visit under 0.5 hunger).
            && world.comp::<Needs>(id).is_some_and(|n| n.intimacy < cfg.street_intimacy && n.hunger >= 0.5)
            && world.comp::<Wallet>(id).is_some_and(|w| w.coins >= 2 * cfg.street_price))
        .then_some(id)
    })
}

/// An unprotected street pitch's assault: a lawless resident of the
/// district (the first by a hash, else none) takes the night's coins in
/// the worker's wallet; the worker's memories and safety.
///
/// Lives at work phase 4 (§ 2.3): the hour's `client`, when lawless, is the
/// actor (the robbery is one of the client's outcomes); else the hashed pick.
fn street_assault(world: &mut World, o: EntityId, p: &Pitch, hour: u64, client: Option<EntityId>) {
    let lawless =
        |w: &World, a: EntityId| w.comp::<crate::components::Personality>(a).is_some_and(|x| x.lawfulness < 0.3);
    let by_client = client.filter(|&c| lawless(world, c) && !world.has::<Sentence>(c));
    let actor = match by_client {
        Some(c) => Some(c),
        None => {
            let Some(res) = world.districts.get(p.district.index()).map(|d| d.residents.clone()) else { return };
            if res.is_empty() {
                return;
            }
            let u = hash(world, PURPOSE_PAYER, key(o), hour * 4 + 3);
            let start = ((u * res.len() as f32) as usize).min(res.len() - 1);
            (0..res.len()).map(|k| res[(start + k) % res.len()]).find(|&a| {
                a != o
                    && crate::systems::demography::is_adult(world, a)
                    && !world.has::<Sentence>(a)
                    && lawless(world, a)
            })
        }
    };
    let Some(a) = actor else { return };
    let take = p.take_today.min(world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0)));
    if take > 0 {
        if let Some(w) = world.comp_mut::<Wallet>(o) {
            w.coins -= take;
        }
        if let Some(w) = world.comp_mut::<Wallet>(a) {
            w.coins += take;
        }
    }
    world.remember(o, MemoryKind::WasRobbed, Some(a), 0.6, -0.6, false);
    world.remember(o, MemoryKind::Lost, Some(a), 0.6, -0.6, false);
    if let Some(n) = world.comp_mut::<Needs>(o) {
        n.safety = (n.safety - 0.4).max(0.0);
    }
    ownership::jobs2_row(world).street_assaults += 1;
    let at = world.district_name(p.district).to_string();
    let text =
        format!("{} assaulted {} at a street pitch in {at} ({take} coins taken)", world.name_of(a), world.name_of(o));
    world.push_event(EventKind::Assault, &[a, o], text);
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
    name_pimps(world);
    close_brothels(world);
}

/// The gang with the most territory (held Homes) in each district with a
/// street pitch (ties the lower id) names its pimp: the sitting one while he
/// is free and a member, else its loyal member of lowest id who deals no
/// Stims; none for a district no gang has a foot in.
fn name_pimps(world: &mut World) {
    let street = crate::systems::service::street_on(world);
    let today = world.day();
    let absent_days = u64::from(world.config.service.pimp_absent_days.max(1));
    let mut seen: BTreeMap<u16, u64> = BTreeMap::new();
    let mut want: BTreeMap<u16, EntityId> = BTreeMap::new();
    let mut held: BTreeMap<DistrictId, (usize, std::cmp::Reverse<EntityId>)> = BTreeMap::new();
    for g in world.gangs() {
        for (d, n) in crate::systems::gang::held_districts(world, g) {
            let e = held.entry(d).or_insert((0, std::cmp::Reverse(g)));
            if (n, std::cmp::Reverse(g)) > *e {
                *e = (n, std::cmp::Reverse(g));
            }
        }
    }
    let mut districts: Vec<DistrictId> = Vec::new();
    for p in world.jobs2.pitches.values() {
        if is_street_pitch(world, p) && !districts.contains(&p.district) {
            districts.push(p.district);
        }
    }
    districts.sort_unstable();
    for d in districts {
        let Some(&(n, std::cmp::Reverse(g))) = held.get(&d) else { continue };
        if n == 0 {
            continue;
        }
        let k = d.index() as u16;
        let sitting_any = world.jobs2.vice.pimps.get(&k).copied();
        // Lives at work phase 4b (§ 2.3): absent from every pitch of his
        // `pimp_absent_days`, the sitting pimp loses the district.
        let absent = street
            && sitting_any.is_some()
            && world.jobs2.vice.pimp_seen.get(&k).is_some_and(|&s| today.saturating_sub(s) >= absent_days);
        let sitting = sitting_any.filter(|&p| {
            !absent && world.gang_of(p) == Some(g) && crate::systems::law::living(world, p) && !world.has::<Sentence>(p)
        });
        let fit = |m: EntityId| {
            crate::systems::law::living(world, m)
                && !world.has::<Sentence>(m)
                && crate::systems::gang::following_order(world, m).is_some()
                && !crate::systems::stims::is_dealer(world, m)
                && !(absent && sitting_any == Some(m))
        };
        let pick = sitting.or_else(|| {
            let members = &world.comp::<Gang>(g)?.members;
            // Lives at work phase 4b (§ 2.3): the round is a night's work, so
            // a member with no Job first (a day job's sleeper is no pimp).
            let free = if street { members.iter().copied().find(|&m| fit(m) && !world.has::<Job>(m)) } else { None };
            free.or_else(|| members.iter().copied().find(|&m| fit(m)))
        });
        if let Some(p) = pick {
            want.insert(k, p);
            // A newly named pimp's days of grace start today.
            if street {
                let last = world.jobs2.vice.pimp_seen.get(&k).copied().filter(|_| sitting == Some(p));
                seen.insert(k, last.unwrap_or(today));
            }
        }
    }
    world.jobs2.vice.pimps = want;
    world.jobs2.vice.pimp_seen = seen;
}

/// D7: an unlicensed Brothel closes a day in a Sweep or a Crackdown
/// district (a licensed one is shut only as a front of the gang a
/// Crackdown targets, `leisure::fronts_daily`).
fn close_brothels(world: &mut World) {
    if world.levers.vice.brothels_licensed {
        return;
    }
    let now = world.tick;
    for b in world.buildings_of_kind(BuildingKind::Brothel).to_vec() {
        let d = world.district_of_building(b);
        let stance = world.districts.get(d.index()).map(|x| x.stance);
        let shut = matches!(stance, Some(Stance::Crackdown(_) | Stance::Sweep));
        if shut && !world.is_closed(b) {
            if let Some(bd) = world.comp_mut::<Building>(b) {
                bd.closed_until = Some(now + TICKS_PER_DAY);
            }
            ownership::jobs2_row(world).brothel_closed += 1;
        }
    }
}

/// `World::new`, after the industries: each copy of `[vice] corp_rows`
/// takes the Vice niche and opens `strip_clubs` StripClubs and `brothels`
/// Brothels on the Lots left (the venue tier rule). No RNG, no coins.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.vice.clone();
    let copies = world.lot_copies();
    for j in 0..copies {
        for &r in &cfg.corp_rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            if let Some(cc) = world.comp_mut::<Corp>(c) {
                cc.niches.insert(Niche::Vice);
            }
            for (kind, n) in [(BuildingKind::StripClub, cfg.strip_clubs), (BuildingKind::Brothel, cfg.brothels)] {
                for _ in 0..n {
                    seed_venue(world, kind, Some(c));
                }
            }
        }
    }
}

/// A seeded venue of `kind` for `owner` on a vacant Lot (`jobs::seed_lot`).
pub(crate) fn seed_venue(world: &mut World, kind: BuildingKind, owner: Option<EntityId>) -> Option<EntityId> {
    let lot = crate::systems::jobs::seed_lot(world, kind)?;
    crate::systems::founding::build_on_lot(world, lot, kind, owner).ok()?;
    let at = world.districts.get(world.district_of_building(lot).index()).map_or("?", |x| x.name.as_str());
    let who = world.owner_label(owner);
    let text = format!("{who} opened a {} in {at} (seeded; Lot {})", kind.label(), lot.index);
    let mut actors: Vec<EntityId> = owner.into_iter().collect();
    actors.push(lot);
    world.push_event(EventKind::Founded, &actors, text);
    Some(lot)
}

/// The day's end (`systems::stats`): the vice group's snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let venues = [BuildingKind::StripClub, BuildingKind::Brothel]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .count() as u32;
    let street = world.jobs2.pitches.values().filter(|p| is_street_pitch(world, p)).count() as u32;
    let pimps = world.jobs2.vice.pimps.len() as u32;
    let row = ownership::jobs2_row(world);
    row.vice_venues = venues;
    row.street_pitches = street;
    row.pimps = pimps;
}
