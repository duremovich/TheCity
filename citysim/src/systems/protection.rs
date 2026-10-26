//! The pressure pass phase 3 (docs/PRESSURE_PASS.md § Phase 3; OPEN-160,
//! OPEN-162): protection bites, under `[protection]` (absent: M16b's
//! protection as it was).
//!
//! **Held ground** ([`ground`]): the districts a gang's turf Homes stand in,
//! and (`hideout_ground`) its Hideout's district from day 0. The weekly pass
//! (`leverage::protection_pass`, each gang on its weekday) leans on up to
//! `lean_per_district` businesses on that ground whatever the gang's order
//! (not while it lies low), for a levy sized to the business ([`levy`]).
//!
//! **Refusal bites**: a lean the owner refuses is recorded standing with
//! its misses at `punish_misses`; a week missed (a round stop that met
//! nobody, the till short at the midnight collection) counts a miss
//! ([`miss`]). At `punish_misses` the gang's toughest free member is sent
//! to the business in person (a `Visit`, `VisitWhy::Punish`, between
//! `punish_hour_from..to`; a Statistical enforcer is credited at the hour).
//! There ([`punished`]): the first time the till is smashed (or the stall
//! kicked over: `smash_weeks` of the levy taken from the purse, the business shut
//! `smash_close_days`), the second the owner is beaten if he is there (else
//! the till again), from the third a fire (shut `fire_close_days`). Each
//! raises the crime, leaves the owner's grudge on the enforcer and his
//! report if lawful, and the enforcer keeps `enforcer_cut` of the take.
//!
//! **The owner's choice** after, one weighted draw: pay up now, report the
//! enforcer and the leader, go to a rival gang with ground in the district
//! for cover (the hold ends, the rival's protection starts, the holder's
//! leader takes a grudge on the rival's and the gang a `HomeFlippedAgainst`
//! shock: a turf war's spark), or (with `[underwater]`) sell to the gang as
//! a front or shut the business.
//!
//! **The gang's purse**: the protection paid joins the week's tribute the
//! leader's Collect shares out (`leverage::pay_week`); daily, a gang with
//! the coins over `bail_reserve` bails out a jailed member (`bail_per_day`
//! a day of the sentence left, to the city, `Flow::Fine`). The daily
//! stipend (`gang::daily_economy`) goes only to the jobless members
//! (`stipend_jobless_only`; the employed wait for the Collect) and only
//! out of the treasury above `stipend_reserve`.
//!
//! **A rival's front** leaned on and punished: no choice, a `Trespass` shock
//! on the gang that owns it and its leader's grudge on the enforcer.
//!
//! **OPEN-157** (`service::contract_party`): a contract's party the Fixer
//! cannot meet now is sent there in person at its next open hour (a
//! `VisitWhy::Broker` visit) instead of the encounter being missed.
//!
//! Statistical mirror: the pass, the misses and the bail are midnight or
//! round reads, tier-blind; the punishment is a `Visit`, credited to a
//! Statistical enforcer at its hour.

use crate::components::{Building, BuildingKind, DistrictId, Gang, Personality, Position, Sentence, Shock};
use crate::config::ProtectionCfg;
use crate::contract::{CoercionId, CoercionStatus, CoercionWhy, Demand};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::household::{PendingVisit, VisitWhy};
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::systems::{law, presence};
use crate::time::TICKS_PER_DAY;
use crate::world::World;

const PURPOSE_HOUR: u64 = 0x5054_0001;
const PURPOSE_CHOICE: u64 = 0x5054_0002;
const PURPOSE_OWNER: u64 = 0x5054_0003;

pub fn cfg(world: &World) -> Option<&ProtectionCfg> {
    world.config.protection.as_ref()
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

/// The districts a gang's protection reaches: its turf's, and with
/// `hideout_ground` its Hideout's; ascending, deduplicated.
pub fn ground(world: &World, gang: EntityId) -> Vec<DistrictId> {
    let mut out: Vec<DistrictId> =
        crate::systems::gang::held_districts(world, gang).into_iter().map(|(d, _)| d).collect();
    if cfg(world).is_some_and(|c| c.hideout_ground) {
        if let Some(h) = world.hideout_of(gang).filter(|&h| world.has::<Building>(h)) {
            let d = world.district_of_building(h);
            if let Err(i) = out.binary_search(&d) {
                out.insert(i, d);
            }
        }
    }
    out
}

/// The weekly levy on a business taking `week` coins a week: `levy_share`
/// of it, at most `cap`; `None` under `levy_min`.
pub fn levy(world: &World, week: i64, cap: i64) -> Option<i64> {
    let c = cfg(world)?;
    let x = ((c.levy_share * week as f32).floor() as i64).min(cap);
    (x >= c.levy_min).then_some(x)
}

/// The enforcer's Visit urgency (0 without the section).
pub fn urgency(world: &World) -> f32 {
    cfg(world).map_or(0.0, |c| c.punish_urgency)
}

/// The gang's toughest free member off shift at `hour` (one on shift only
/// when nobody else is), its leader only when nobody else is free, never
/// `not` (the coerced, a member himself), nor one with a visit due.
fn enforcer(world: &World, gang: EntityId, not: EntityId, hour: u8) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    let tod = u16::from(hour) * crate::time::TICKS_PER_HOUR as u16;
    let mut best: Option<(bool, bool, f32, EntityId)> = None;
    for &m in &g.members {
        if m == not || !crate::systems::contracts::free_adult(world, m) || world.visits.contains_key(&m) {
            continue;
        }
        let busy = world.comp::<crate::components::Job>(m).is_some_and(|j| j.on_shift(tod));
        let key = (g.leader == Some(m), busy, law::fighting(world, m), m);
        let better = best.is_none_or(|b| (key.0, key.1, -key.2, key.3) < (b.0, b.1, -b.2, b.3));
        if better {
            best = Some(key);
        }
    }
    best.map(|x| x.3)
}

/// A week missed on protection `id` (`why`: nobody met, the till short):
/// the count goes up, and at `punish_misses` the enforcer is sent.
pub fn miss(world: &mut World, id: CoercionId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(x) = world.coercions.get_mut(&id) else { return };
    if !x.is_active() || x.why != CoercionWhy::Protection {
        return;
    }
    x.missed = x.missed.saturating_add(1);
    let missed = x.missed;
    ownership::jobs2_row(world).pr_missed += 1;
    if missed >= c.punish_misses {
        send(world, &c, id);
    }
}

/// A lean refused: the hold stands with its misses at `punish_misses`, and
/// the enforcer is sent.
pub fn refused(world: &mut World, id: CoercionId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(x) = world.coercions.get_mut(&id) else { return };
    x.missed = c.punish_misses;
    x.refusing = true;
    let x = x.clone();
    let (Some(g), Some(b)) = (x.holder, x.building) else { return };
    ownership::jobs2_row(world).pr_refused += 1;
    let text = format!(
        "{} refused {}'s protection on {}: {} will not pay",
        world.name_of(x.over),
        gang_name(world, g),
        world.name_of(b),
        world.name_of(x.over)
    );
    world.push_event(EventKind::Refused, &[x.over, x.agent, b], text);
    send(world, &c, id);
}

/// Send the gang's enforcer to the business (a Visit at an open hour), or
/// with the visits off, punish at once.
fn send(world: &mut World, c: &ProtectionCfg, id: CoercionId) {
    let Some(x) = world.coercions.get(&id).cloned() else { return };
    let (Some(g), Some(b)) = (x.holder, x.building) else { return };
    // One enforcer at a time on a business.
    if world.visits.values().any(|v| v.kin == b && v.why == VisitWhy::Punish) {
        return;
    }
    let span = c.punish_hour_to.saturating_sub(c.punish_hour_from).max(1);
    let u = hash(world, PURPOSE_HOUR, b, world.day() ^ u64::from(g.index));
    let hour = c.punish_hour_from + ((u * f32::from(span)) as u8).min(span - 1);
    let Some(who) = enforcer(world, g, x.over, hour) else { return };
    if !crate::systems::household::on(world) {
        punished(world, who, b, false);
        return;
    }
    world.visits.insert(who, PendingVisit { kin: b, why: VisitWhy::Punish, hour });
}

/// Is `who` at business `b` now: a body inside, else (off screen) a keyed
/// draw at `[leaders] owner_stat_p`.
fn there(world: &World, who: EntityId, b: EntityId) -> bool {
    if !law::living(world, who) || world.has::<Sentence>(who) {
        return false;
    }
    if presence::is_body(world, who) {
        return presence::body_at(world, who, b);
    }
    let p = world.config.leaders.as_ref().map_or(0.5, |l| l.owner_stat_p);
    hash(world, PURPOSE_OWNER, who, world.day() ^ (u64::from(b.index) << 20)) < p
}

/// The enforcer's call at business `b` (a Visit arrived, or a Statistical
/// one at its hour): the punishment the gang's standing protection there
/// has earned, then the owner's choice.
pub fn punished(world: &mut World, who: EntityId, b: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(gang) = world.gang_of(who) else { return };
    let Some(x) = world
        .coercions
        .values()
        .find(|x| {
            x.is_active()
                && x.why == CoercionWhy::Protection
                && x.holder == Some(gang)
                && x.building == Some(b)
                && x.missed >= c.punish_misses
        })
        .cloned()
    else {
        return;
    };
    if !law::living(world, who) || world.has::<Sentence>(who) {
        return;
    }
    let owner = x.over;
    let payer = x.payer.unwrap_or(owner);
    let gname = gang_name(world, gang);
    let bname = world.name_of(b);
    let how = if walked { "" } else { " (off screen)" };
    let tile = world
        .comp::<Position>(who)
        .filter(|_| walked)
        .map(|p| p.tile)
        .or_else(|| world.comp::<Building>(b).map(|bd| bd.door))
        .unwrap_or_default();
    let stall =
        world.comp::<Building>(b).is_some_and(|bd| matches!(bd.kind, BuildingKind::Market | BuildingKind::NoodleBar));
    let owner_here = there(world, owner, b);
    let level = x.punished;
    let mut what;
    let mut assault = false;
    let mut murder = false;
    if level >= 2 {
        // A fire: the business shut for the repairs.
        let until = world.tick + u64::from(c.fire_close_days) * TICKS_PER_DAY;
        if let Some(bd) = world.comp_mut::<Building>(b) {
            bd.closed_until = Some(bd.closed_until.unwrap_or(0).max(until));
        }
        ownership::jobs2_row(world).pr_fire += 1;
        what = "the fire";
        let text = format!(
            "{} set {bname} on fire for {gname}{how}: {} would not pay; shut {} days",
            world.name_of(who),
            world.name_of(owner),
            c.fire_close_days
        );
        world.push_event(EventKind::Punished, &[who, owner, b, gang], text);
    } else if level == 1 && owner_here {
        // The owner beaten.
        let mods = law::FightMods { kill_mult: c.beat_kill_mult, ..Default::default() };
        let (winner, loser, died) = law::resolve_fight_mods(world, who, owner, mods);
        ownership::jobs2_row(world).pr_beat += 1;
        what = "the beating";
        assault = true;
        murder = died && loser == owner;
        let tail = if winner != who {
            format!(" ({} fought back and won)", world.name_of(winner))
        } else if murder {
            ", to death".to_string()
        } else {
            String::new()
        };
        let text = format!(
            "{} beat {} at {bname} for {gname}'s protection{how}{tail}",
            world.name_of(who),
            world.name_of(owner)
        );
        world.push_event(EventKind::Punished, &[who, owner, b, gang], text);
    } else {
        // The till smashed (the stall kicked over): `smash_weeks` of the
        // levy, what the till holds of it.
        let purse = world.purse(Some(payer)).max(0);
        let week = match x.demand {
            Demand::Coins { amount, .. } => amount,
            _ => 0,
        };
        let take = ((c.smash_weeks * week as f32).floor() as i64).min(purse);
        let cut = (c.enforcer_cut * take as f32).floor() as i64;
        let mut took = 0;
        if take > 0 {
            took += ownership::pay(world, Some(payer), Some(who), cut, Flow::Robbery);
            took += ownership::pay(world, Some(payer), Some(gang), take - cut, Flow::Robbery);
            ownership::note_loss(world, b, took, Some(who));
        }
        let until = world.tick + u64::from(c.smash_close_days) * TICKS_PER_DAY;
        if let Some(bd) = world.comp_mut::<Building>(b) {
            bd.closed_until = Some(bd.closed_until.unwrap_or(0).max(until));
        }
        ownership::jobs2_row(world).pr_smash += 1;
        what = if stall { "the stall kicked over" } else { "the smashed till" };
        let verb = if stall { "kicked over the stall at" } else { "smashed the till at" };
        let text = format!(
            "{} {verb} {bname} for {gname}{how}: took {took}c ({} kept {cut}c), shut {} day(s); {} owes the week",
            world.name_of(who),
            world.name_of(who),
            c.smash_close_days,
            world.name_of(owner)
        );
        world.push_event(EventKind::Punished, &[who, owner, b, gang], text);
        if level == 1 {
            what = if stall { "the stall kicked over again" } else { "the till smashed again" };
        }
    }
    let kind = if murder {
        crate::components::Crime::Murder
    } else if assault {
        crate::components::Crime::Assault
    } else {
        crate::components::Crime::Extortion
    };
    if murder {
        let t = format!("{} killed {}", world.name_of(who), world.name_of(owner));
        world.push_event(EventKind::Murder, &[who, owner], t);
    }
    law::raise_crime_on(world, who, (assault && !murder).then_some(owner), Some(owner), kind, tile);
    if let Some(y) = world.coercions.get_mut(&x.id) {
        y.punished = y.punished.saturating_add(1);
        y.missed = 0;
    }
    if !law::living(world, owner) {
        return;
    }
    let cause = if assault { crate::word::GrudgeCause::Assaulted } else { crate::word::GrudgeCause::Robbed };
    crate::systems::grudges::add(world, owner, who, cause, c.grudge, 0);
    let lawful = world.comp::<Personality>(owner).map_or(0.5, |p| p.lawfulness);
    if level >= 1 && lawful >= c.victim_report_lawful {
        law::file_report(world, kind, who, Some(owner));
    }
    choose(world, &c, x.id, who, what);
}

/// The owner's choice after a punishment (see the module).
fn choose(world: &mut World, c: &ProtectionCfg, id: CoercionId, enforcer: EntityId, what: &str) {
    let Some(x) = world.coercions.get(&id).cloned() else { return };
    let (Some(gang), Some(b)) = (x.holder, x.building) else { return };
    let owner = x.over;
    if world.has::<Sentence>(owner) {
        return;
    }
    let payer = x.payer.unwrap_or(owner);
    // A rival gang's front: no choice to make, a score to settle (the turf
    // war's spark: a Trespass shock on the gang that owns it, its leader's
    // grudge on the enforcer).
    if let OwnerKind::Gang(r) = ownership::owner_kind(world, Some(payer)) {
        crate::systems::gang::push_shock(world, r, Shock::Trespass { by: Some(gang) });
        crate::systems::grudges::add(world, owner, enforcer, crate::word::GrudgeCause::Robbed, c.cover_grudge, 0);
        ownership::jobs2_row(world).pr_rival += 1;
        let text = format!(
            "{} will answer {} for {what} at {}",
            gang_name(world, r),
            gang_name(world, gang),
            world.name_of(b)
        );
        world.push_event(EventKind::Punished, &[owner, gang, b, r], text);
        return;
    }
    let agent_owned = matches!(ownership::owner_kind(world, Some(payer)), OwnerKind::Agent(_));
    let p = world.comp::<Personality>(owner).cloned();
    let (courage, lawful) = p.as_ref().map_or((0.5, 0.5), |p| (p.courage, p.lawfulness));
    let misery = crate::systems::misery::misery(world, owner);
    let rival = if agent_owned { rival_for(world, gang, b) } else { None };
    let w_pay = c.w_pay * (1.5 - courage).max(0.0);
    let w_report = c.w_report * lawful;
    let w_rival = if rival.is_some() { c.w_rival * (0.5 + courage) } else { 0.0 };
    let w_sell = if agent_owned && crate::systems::underwater::cfg(world).is_some() {
        c.w_sell * (0.25 + misery) * f32::from(x.punished.max(1))
    } else {
        0.0
    };
    let total = w_pay + w_report + w_rival + w_sell;
    if total <= 0.0 {
        return;
    }
    let u = hash(world, PURPOSE_CHOICE, owner, world.day() ^ u64::from(b.index)) * total;
    let gname = gang_name(world, gang);
    let (oname, bname) = (world.name_of(owner), world.name_of(b));
    if u < w_pay {
        if let Some(y) = world.coercions.get_mut(&id) {
            y.refusing = false;
        }
        let paid = crate::systems::leverage::collect_in_person(world, id);
        ownership::jobs2_row(world).pr_pay += 1;
        let text = if paid > 0 {
            format!("{oname} paid {gname} {paid}c after {what} at {bname}")
        } else {
            format!("{oname} swore to pay {gname} after {what} at {bname}, with nothing in the till")
        };
        world.push_event(EventKind::Punished, &[owner, gang, b], text);
    } else if u < w_pay + w_report {
        if let Some(y) = world.coercions.get_mut(&id) {
            y.refusing = true;
        }
        let kind = crate::components::Crime::Extortion;
        law::file_report(world, kind, enforcer, Some(owner));
        if x.agent != enforcer && law::living(world, x.agent) {
            law::file_report(world, kind, x.agent, Some(owner));
        }
        ownership::jobs2_row(world).pr_report += 1;
        let named = if x.agent == enforcer {
            world.name_of(enforcer)
        } else {
            format!("{} and {}", world.name_of(enforcer), world.name_of(x.agent))
        };
        let text = format!("{oname} went to the law after {what} at {bname}: reported {named} of {gname}");
        world.push_event(EventKind::Punished, &[owner, gang, b], text);
    } else if u < w_pay + w_report + w_rival {
        let Some((r, leader)) = rival else { return };
        let Demand::Coins { amount, .. } = x.demand else { return };
        crate::systems::leverage::finish(
            world,
            id,
            CoercionStatus::Broken,
            &format!("{oname} went to {} for cover", gang_name(world, r)),
        );
        crate::systems::leverage::cover(world, r, leader, owner, payer, b, amount);
        ownership::jobs2_row(world).pr_rival += 1;
        // The spark: the holder lost a business to the rival.
        crate::systems::gang::push_shock(world, gang, Shock::HomeFlippedAgainst);
        if let Some(l) = world.comp::<Gang>(gang).and_then(|g| g.leader) {
            crate::systems::grudges::add(world, l, leader, crate::word::GrudgeCause::Robbed, c.cover_grudge, 0);
        }
        let text = format!(
            "{oname} went to {} for cover from {gname} after {what} at {bname}: {} takes {amount}c a week now",
            gang_name(world, r),
            world.name_of(leader)
        );
        world.push_event(EventKind::Punished, &[owner, gang, b, r, leader], text);
    } else {
        let Some(front) = crate::systems::underwater::forced_out(world, b, owner, gang) else { return };
        ownership::jobs2_row(world).pr_sell += 1;
        let text = if front {
            format!("{oname} gave up {bname} to {gname} after {what}: a front now")
        } else {
            format!("{oname} shut {bname} after {what}: {gname} will not have it")
        };
        world.push_event(EventKind::Punished, &[owner, gang, b], text);
    }
}

/// A rival that would cover `b` from `gang`: another gang with a free
/// leader whose ground takes in `b`'s district, the one with the most
/// members (ties the lower id).
fn rival_for(world: &World, gang: EntityId, b: EntityId) -> Option<(EntityId, EntityId)> {
    let d = world.district_of_building(b);
    world
        .gangs()
        .into_iter()
        .filter(|&r| r != gang)
        .filter_map(|r| {
            let g = world.comp::<Gang>(r)?;
            let l = g.leader.filter(|&l| crate::systems::contracts::free_adult(world, l))?;
            ground(world, r).binary_search(&d).is_ok().then_some((g.members.len(), std::cmp::Reverse(r), l))
        })
        .max()
        .map(|(_, std::cmp::Reverse(r), l)| (r, l))
}

/// Daily (after the gang's economy): the bail, then the treasuries' snapshot.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    for gang in world.gangs() {
        let Some(g) = world.comp::<Gang>(gang) else { continue };
        let mut jailed: Vec<(u64, EntityId)> = g
            .members
            .iter()
            .filter_map(|&m| world.comp::<Sentence>(m).map(|s| (s.until_tick.saturating_sub(now), m)))
            .filter(|&(left, _)| {
                left >= u64::from(c.bail_min_days) * TICKS_PER_DAY && left <= u64::from(c.bail_max_days) * TICKS_PER_DAY
            })
            .collect();
        if jailed.is_empty() {
            continue;
        }
        // The leader first, then the shortest sentence.
        let leader = g.leader;
        jailed.sort_unstable_by_key(|&(left, m)| (Some(m) != leader, left, m));
        for (left, m) in jailed {
            let days = left.div_ceil(TICKS_PER_DAY) as i64;
            let bail = c.bail_per_day * days;
            let spare = world.comp::<Gang>(gang).map_or(0, |g| g.treasury)
                - c.bail_reserve
                - crate::systems::leisure::set_aside(world, gang);
            if spare < bail {
                break;
            }
            let paid = ownership::pay(world, Some(gang), None, bail, Flow::Fine);
            if paid < bail {
                break;
            }
            let text =
                format!("{} paid {paid}c bail for {} ({days} days left)", gang_name(world, gang), world.name_of(m));
            world.push_event(EventKind::Punished, &[m, gang], text);
            let row = ownership::jobs2_row(world);
            row.pr_bail += 1;
            row.pr_bail_coins += paid;
            law::release(world, m, false);
        }
    }
    let mut t: Vec<i64> = world.gangs().into_iter().filter_map(|g| world.comp::<Gang>(g).map(|x| x.treasury)).collect();
    t.sort_unstable();
    let row = ownership::jobs2_row(world);
    row.gang_treasury_med = t.get(t.len() / 2).copied().unwrap_or(0);
    row.gang_treasury_max = t.last().copied().unwrap_or(0);
}
