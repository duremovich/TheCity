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
    Sentence, Stance, Wallet,
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
pub fn street_cut(world: &World, p: &Pitch) -> Option<(EntityId, f32)> {
    if !on(world) || !is_street_pitch(world, p) {
        return None;
    }
    let pimp = pimp_of(world, p.district)?;
    Some((world.gang_of(pimp)?, world.config.vice.pimp_cut))
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
        if hash(world, PURPOSE_VISIT, key(o), hour) < cfg.p_street_visit {
            if let Some(payer) = street_payer(world, &p, hour) {
                let paid = ownership::pay(world, Some(payer), Some(o), cfg.street_price, Flow::Leisure);
                if paid > 0 {
                    add_intimacy(world, payer, cfg.intimacy_gain);
                    if let Some(x) = world.jobs2.pitches.get_mut(&o) {
                        x.take_today += paid;
                    }
                    let row = ownership::jobs2_row(world);
                    row.street_visits += 1;
                    row.street_take += paid;
                }
            }
        }
        let protected = pimp_of(world, p.district).is_some() || world.comp::<Kit>(o).is_some_and(|k| k.weapon > 0);
        if protected {
            continue;
        }
        let fear = world.districts.get(p.district.index()).map_or(0.0, |d| d.fear);
        if hash(world, PURPOSE_ASSAULT, key(o), hour) < cfg.p_assault_street * (0.5 + fear) {
            street_assault(world, o, &p, hour);
        }
    }
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
    (0..3u64).find_map(|k| {
        let u = hash(world, PURPOSE_PAYER, key(p.owner), hour * 4 + k);
        let id = res[((u * res.len() as f32) as usize).min(res.len() - 1)];
        (id != p.owner
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
fn street_assault(world: &mut World, o: EntityId, p: &Pitch, hour: u64) {
    let Some(res) = world.districts.get(p.district.index()).map(|d| d.residents.clone()) else { return };
    if res.is_empty() {
        return;
    }
    let u = hash(world, PURPOSE_PAYER, key(o), hour * 4 + 3);
    let start = ((u * res.len() as f32) as usize).min(res.len() - 1);
    let actor = (0..res.len()).map(|k| res[(start + k) % res.len()]).find(|&a| {
        a != o
            && crate::systems::demography::is_adult(world, a)
            && !world.has::<Sentence>(a)
            && world.comp::<crate::components::Personality>(a).is_some_and(|x| x.lawfulness < 0.3)
    });
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
        let sitting = world.jobs2.vice.pimps.get(&k).copied().filter(|&p| {
            world.gang_of(p) == Some(g) && crate::systems::law::living(world, p) && !world.has::<Sentence>(p)
        });
        let pick = sitting.or_else(|| {
            world.comp::<Gang>(g).and_then(|x| {
                x.members.iter().copied().find(|&m| {
                    crate::systems::law::living(world, m)
                        && !world.has::<Sentence>(m)
                        && crate::systems::gang::following_order(world, m).is_some()
                        && !crate::systems::stims::is_dealer(world, m)
                })
            })
        });
        if let Some(p) = pick {
            want.insert(k, p);
        }
    }
    world.jobs2.vice.pimps = want;
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
    let copies = world.seed_copies();
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
