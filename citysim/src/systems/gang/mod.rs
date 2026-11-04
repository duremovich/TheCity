//! The gangs: recruitment into one of several gangs, joining and leaving,
//! ranks and leaders, heat and shocks for the faction brain, extortion and
//! Home claims, loot splitting, fencing, the daily economy, betrayal.
//!
//! The brain that issues orders lives in `faction`; raids in `raid`.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Claim, DistrictId, Gang, GangMember, GoalKind, Household, Identity, Job, Memory,
    MemoryKind, Needs, Order, Personality, Position, Sentence, Shock, TilePos, Wallet, Zone,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::systems::{faction, law, social};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

mod leaders;
mod recruit;
mod split;
mod squats;
mod work;

pub use leaders::*;
pub use recruit::*;
pub use split::*;
pub use squats::*;
pub use work::*;

/// A claim at this count holds the Home: it is territory.
pub const CLAIM_HELD: u8 = 3;
/// `heat_log` keeps at most this many arrests and deaths.
pub const HEAT_LOG_CAP: usize = 64;

// ---------------------------------------------------------------------------
// Per tick: the brain; daily: leaders, tribute, stipend, disbanding, betrayal
// ---------------------------------------------------------------------------

pub fn run(world: &mut World) {
    let daily = crate::systems::midnight::due(world, crate::systems::midnight::Mid::Gang);
    let hysteresis = world.config.gangs.hysteresis;
    let threshold = world.config.gangs.shock_severity_rethink;
    // Pressure pass round 6, V2 (TURF § 1): the members' hours on the street
    // and the corner, then at the gangs' midnight the cells' holders (before
    // the brains read `rival_of`, the neighbour).
    crate::systems::turf::hourly(world);
    if daily {
        crate::prof::timed!("gang::turf_daily", crate::systems::turf::daily(world));
        // The city milestone C8: the district Hideouts, the lifts held.
        crate::prof::timed!("gang::vertical_daily", crate::systems::vertical::daily(world));
    }
    for gang in world.gangs() {
        // M12 D36: a decapitation this tick may split the gang. M17 phase
        // 4b: the succession first (a promotion, or a civil war instead of
        // the split check).
        if let Some(old) = world.comp_mut::<Gang>(gang).and_then(|g| g.split_check.take()) {
            if !crate::systems::succession::decide(world, gang, old) {
                let _ = split(world, gang, Some(old), true);
            }
        }
        if daily {
            crate::prof::timed!("gang::succession_daily", crate::systems::succession::daily(world, gang));
            crate::prof::timed!("gang::recompute_leader", recompute_leader(world, gang));
            // The law has just rescored (it runs before this system): a gang
            // under Crackdown may buy its way out before choosing its order.
            crate::prof::timed!("gang::consider_bribe", faction::consider_bribe(world, gang));
        }
        let pending: f32 = world.comp::<Gang>(gang).map_or(0.0, |g| g.shocks.iter().map(|s| s.severity()).sum());
        // M16a (plan C26, phase 2): a gang whose job settled rescores at once.
        let job_done = world.comp::<Gang>(gang).is_some_and(|g| g.order == Order::Job)
            && !crate::systems::contracts::job_live(world, gang);
        // Perf pass 7d (OPEN-211): the next order is already chosen and held for the giving in person; the
        // same rescore every tick only re-derives it (an O(N) `gather_inputs` each). Rescore at the daily
        // tick or on a shock rethink, as the other orders do.
        let held = job_done && world.comp::<Gang>(gang).is_some_and(|g| g.pending_order.is_some());
        if held {
            if daily || pending >= threshold {
                faction::rethink(world, gang);
            }
        } else if job_done {
            faction::rethink(world, gang);
        } else if daily {
            if crate::prof::timed!("gang::rescore_daily", faction::rescore(world, gang, hysteresis)) {
                if let Some(g) = world.comp_mut::<Gang>(gang) {
                    g.shocks.clear();
                }
            }
        } else if pending >= threshold {
            faction::rethink(world, gang);
        }
    }
    // The pre-view pass phase 6 (P15): the waiting orders given in person.
    crate::systems::leaders::run(world);
    if daily {
        crate::prof::timed!("gang::daily_economy", daily_economy(world));
        // The pressure pass phase 3: the bail, the treasuries' snapshot.
        crate::prof::timed!("gang::protection_daily", crate::systems::protection::daily(world));
        crate::prof::timed!("gang::desist", desist(world));
    }
}

/// L2 phase 5: a gang stipend or tribute reached `id` today.
pub fn note_paid(world: &mut World, id: EntityId) {
    let today = world.day();
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.gang_paid_day = Some(today);
    }
}

/// L2 phase 5 (the year runs' diagnosis: the roster cap had no exit; four
/// gangs filled to `max_members` by ~day 150 and the violence followed):
/// gang desistance. Daily, every ordinary member (not the leader nor
/// `leader_ranking`'s two lieutenants; not while its gang musters a raid
/// or a BreakOut; not jailed, cuffed, hunting or hunted) walks away with
/// `p = desist_base x` the terms `[living]` names (employed, unpaid,
/// settled, aged, lately jailed; a feud brakes it), one draw from a
/// `splitmix64` hash of the seed, the member and the day (no stream). A
/// leaver keeps its grudges and memories and cools `JoinGang` for
/// `desist_cooldown_days`.
pub fn desist(world: &mut World) {
    let cfg = world.config.living.clone();
    if cfg.desist_base <= 0.0 {
        return;
    }
    let today = world.day();
    let now = world.tick;
    let seed = world.seed();
    let mut leaving: Vec<EntityId> = Vec::new();
    // Dealers with a deal in the last 7 days. `deal_log` keeps only the
    // latest dealer per venue, so a second dealer at the same bar that week
    // is missed (undercounted: it rolls without the brake).
    let dealers: std::collections::BTreeSet<EntityId> =
        world.deal_log.values().filter(|&&(_, d)| d + 7 >= today).map(|&(m, _)| m).collect();
    // Members keeping an asset their gang owns (the gang's Arms implants and decks).
    let mut kept: std::collections::BTreeSet<EntityId> = std::collections::BTreeSet::new();
    // scan-ok: daily, assets only
    for a in world.with::<crate::components::Asset>() {
        let Some(x) = world.comp::<crate::components::Asset>(a) else { continue };
        if let (Some(k), Some(o)) = (x.keeper, x.owner) {
            if world.comp::<Gang>(o).is_some() {
                kept.insert(k);
            }
        }
    }
    for gang in world.gangs() {
        let Some(g) = world.comp::<Gang>(gang) else { continue };
        if (g.order.is_raid() || g.order == Order::BreakOut) && g.raid_at.is_some() {
            continue;
        }
        let mut core: Vec<EntityId> = g.leader.into_iter().collect();
        core.extend(leader_ranking(world, gang).into_iter().filter(|&m| Some(m) != g.leader).take(2));
        let feud_gang = world.vendettas.iter().any(|v| v.a == gang || v.b == gang);
        for &m in &g.members {
            if core.contains(&m) || world.has::<Sentence>(m) {
                continue;
            }
            let Some(b) = world.comp::<Brain>(m) else { continue };
            if b.cuffed_by.is_some() || world.hunts.contains_key(&m) || world.hunted_by.contains_key(&m) {
                continue;
            }
            // Pressure pass F3: a recruit's initiation holds him (`[misery] recruit_tenure_days`).
            if world.comp::<GangMember>(m).is_some_and(|x| crate::systems::misery::recruit_holds(world, x.joined_tick))
            {
                continue;
            }
            let mut p = cfg.desist_base;
            if world.comp::<Job>(m).is_some_and(|j| j.paid_once) {
                p *= cfg.desist_employed;
            }
            let joined = world.comp::<GangMember>(m).map_or(0, |x| x.joined_tick / TICKS_PER_DAY);
            let paid = b.gang_paid_day.unwrap_or(joined).max(joined);
            if today.saturating_sub(paid) >= cfg.desist_unpaid_days {
                p *= cfg.desist_unpaid;
            }
            let sump_home = world
                .comp::<Household>(m)
                .and_then(|h| h.home)
                // Review fix: the district's zone, as `leisure::spots_daily` reads it.
                .map(|h| {
                    world.districts.get(world.district_of_building(h).index()).is_some_and(|d| d.zone == Zone::Sump)
                });
            if world.spouse_of(m).is_some() || sump_home == Some(false) {
                p *= cfg.desist_settled;
            }
            if world.comp::<Identity>(m).is_some_and(|i| i.age_years() >= cfg.desist_age) {
                p *= cfg.desist_aged;
            }
            // Released from a sentence in the last 30 days: the biography's
            // `Released` row (a `WasArrested` memory is written at the
            // arrest as well as at the release; an escape writes neither).
            let lately_released = world.life.get(m.index as usize).and_then(|l| l.as_ref()).is_some_and(|l| {
                l.events.iter().any(|e| {
                    e.kind == crate::components::LifeKind::Released && now.saturating_sub(e.tick) <= 30 * TICKS_PER_DAY
                })
            });
            if lately_released {
                p *= cfg.desist_jailed;
            }
            let grudge_on_rival = world.comp::<crate::word::Grudges>(m).is_some_and(|gr| {
                gr.list.iter().any(|x| {
                    x.settled.is_none() && world.gang_of(x.target).is_some_and(|tg| tg != gang)
                        || (x.settled.is_none() && x.target != gang && world.comp::<Gang>(x.target).is_some())
                })
            });
            if feud_gang || grudge_on_rival {
                p *= cfg.desist_feud;
            }
            // Investment and income hold a member: an asset the gang owns
            // in its keeping (the gang's Arms, its deck) or a deal this week.
            if kept.contains(&m) || dealers.contains(&m) {
                p *= cfg.desist_invested;
            }
            // Jobs & lives phase 6 (§ 8): a cult holds its members (1 off).
            let hold = crate::systems::faith::desist_mult(world, gang);
            if hold != 1.0 {
                p *= hold;
            }
            let h = crate::rng::splitmix64(seed ^ 0x6465_7369_7374 ^ u64::from(m.index) ^ today.rotate_left(32));
            let u = (h >> 11) as f32 / (1u64 << 53) as f32;
            if u < p {
                leaving.push(m);
            }
        }
    }
    let until = now + cfg.desist_cooldown_days * TICKS_PER_DAY;
    for m in leaving {
        leave(world, m, "walked away");
        if let Some(b) = world.comp_mut::<Brain>(m) {
            b.cooldowns.insert(GoalKind::JoinGang, until);
        }
    }
}

fn daily_economy(world: &mut World) {
    let today = world.day();
    // scan-ok: daily
    for id in world.citizens() {
        if let Some(b) = world.comp_mut::<Brain>(id) {
            if b.gang_task_day != Some(today) {
                b.loot_today = 0;
            }
        }
    }
    let stipend = world.config.social.gang_stipend;
    let now = world.tick;
    // Pressure pass round 6, V2 (TURF § 2): with `[turf]` the cells' Homes
    // pay their holder (`turf::tribute`), not the claimed Homes alone.
    let turf = crate::systems::turf::on(world);
    if turf {
        crate::systems::turf::tribute(world);
    }
    for gang in world.gangs() {
        // Territory tribute: 2 coins/day per Home when the occupants can pay.
        let territory =
            if turf { Vec::new() } else { world.comp::<Gang>(gang).map(|g| g.territory.clone()).unwrap_or_default() };
        for home in territory {
            // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
            let residents: Vec<EntityId> = world
                .residents_of(home)
                .iter()
                .copied()
                .filter(|&r| !crate::systems::ages::is_protected(world, r))
                .collect();
            let mut owed = 2;
            for r in residents {
                if owed == 0 {
                    break;
                }
                let coins = world.comp::<Wallet>(r).map_or(0, |w| w.coins);
                let pay = owed.min(coins.max(0));
                if pay > 0 {
                    // Shadow V4 bug 14: on the books (`Flow::Coerced`, a
                    // tribute under the gang's hold), not a bare wallet write.
                    owed -= crate::systems::ownership::pay(
                        world,
                        Some(r),
                        Some(gang),
                        pay,
                        crate::systems::ownership::Flow::Coerced,
                    );
                }
            }
            // L2 L20: the week's tribute, for the leader's Collect.
            crate::systems::leisure::note_tribute(world, gang, 2 - owed);
        }
        // Stipend while the treasury holds it, leader first; none while sacked.
        let sacked = world.comp::<Gang>(gang).is_some_and(|g| g.is_sacked(now));
        if !sacked {
            let (mut members, leader) =
                world.comp::<Gang>(gang).map(|g| (g.members.clone(), g.leader)).unwrap_or_default();
            if let Some(l) = leader {
                members.retain(|&m| m != l);
                members.insert(0, l);
            }
            // L2 L20: the week's Collect is set aside (0 with leisure off).
            let set_aside = crate::systems::leisure::set_aside(world, gang);
            // The pressure pass phase 3: the employed wait for the Collect.
            let jobless_only = world.config.protection.as_ref().is_some_and(|c| c.stipend_jobless_only);
            let reserve = world.config.protection.as_ref().map_or(0, |c| c.stipend_reserve);
            for m in members {
                if !world.comp::<Gang>(gang).is_some_and(|g| g.treasury - set_aside - reserve >= stipend) {
                    break;
                }
                if jobless_only && world.comp::<Job>(m).is_some_and(|j| j.employer.is_some()) {
                    continue;
                }
                // Shadow V4 bug 14: the stipend on the books (`Flow::Tribute`,
                // the gang's pay to its members; it was a bare wallet write
                // and the diaries read it as a "daily pass").
                crate::systems::ownership::pay(
                    world,
                    Some(gang),
                    Some(m),
                    stipend,
                    crate::systems::ownership::Flow::Tribute,
                );
                note_paid(world, m);
            }
        }
        // L2 L20: fronts (Crackdown closures, a new front under Expand).
        crate::systems::leisure::fronts_daily(world, gang);
        // The Real economy E28: a gang under Expand with a front gives weekly.
        crate::systems::charity::gang_gift(world, gang);
        // M12 D40: an empty gang's claims lapse after `empty_claims_days`.
        clear_claims_if_empty(world, gang);
        // Disband after 30 days with no members: treasury and territory go, the name stays.
        let empty_since =
            world.comp::<Gang>(gang).and_then(|g| if g.members.is_empty() { g.empty_since } else { None });
        if empty_since.is_some_and(|since| now.saturating_sub(since) >= 30 * TICKS_PER_DAY) {
            let territory = world.comp::<Gang>(gang).map(|g| g.territory.clone()).unwrap_or_default();
            for home in territory {
                if let Some(b) = world.comp_mut::<Building>(home) {
                    b.claim = None;
                }
            }
            if let Some(g) = world.comp_mut::<Gang>(gang) {
                g.treasury = 0;
                g.territory.clear();
                g.empty_since = Some(now);
            }
        }
        // Betrayal: a member with loyalty < 0.3 and an open warrant on themselves.
        let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
        for m in members {
            let disloyal = world.comp::<Personality>(m).is_some_and(|p| p.loyalty < 0.3);
            let wanted = law::wanted(world, m);
            if let Some(b) = world.comp_mut::<Brain>(m) {
                b.betraying = disloyal && wanted;
            }
        }
        // M13 D38: the cook, before the chop and the purchases.
        crate::systems::stims::cook(world, gang);
        // M13 D27: the chop shop, then D44: a bike for a member.
        crate::systems::vehicles::chop_daily(world, gang);
        let before = world.purse(Some(gang));
        crate::systems::vehicles::gang_bikes(world, gang);
        if world.purse(Some(gang)) < before {
            crate::systems::crew::gear_bought(world, gang);
        }
        // M15 W31: a Purist gang buys no chrome and no decks.
        let purist = crate::systems::creeds::is_purist(world, gang);
        // M13 D44 (phase 3): then an Arms implant for the strongest member
        // (Jobs & lives phase 6: not by a cult that tolerates no chrome).
        // The pressure pass phase 5b: gear comes from what is left over the
        // gang's commitments, one piece a week (`crew::gear_spare`; the daily
        // purchase at the floor was the treasuries' sink).
        let spent = world.purse(Some(gang));
        if !purist && !crate::systems::faith::no_chrome(world, gang) {
            crate::systems::chrome::gang_arms(world, gang);
        }
        // M14 V30: then a deck for the best hacker.
        if !purist && world.purse(Some(gang)) == spent {
            crate::systems::virt::gang_deck(world, gang);
        }
        if world.purse(Some(gang)) < spent {
            crate::systems::crew::gear_bought(world, gang);
        }
        // M14 V30 (phase 3): Hideout ICE, then VirtRaid's run orders.
        crate::systems::virt::gang_daily(world, gang);
    }
}

/// A betrayer filed a report naming the leader: they leave, every remaining
/// member becomes their enemy, and their own warrant is resolved.
pub fn betrayal_filed(world: &mut World, betrayer: EntityId) {
    let Some(gang) = world.gang_of(betrayer) else { return };
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    leave(world, betrayer, "betrayal");
    for m in members.into_iter().filter(|&m| m != betrayer) {
        social::make_enemy(world, m, betrayer, -0.7);
    }
    for r in world.reports_mut().iter_mut().filter(|r| r.suspect == betrayer) {
        r.resolved = true;
    }
    if let Some(b) = world.comp_mut::<Brain>(betrayer) {
        b.betraying = false;
    }
    let name = world.name_of(betrayer);
    world.push_event(EventKind::Betrayal, &[betrayer], format!("{name} betrayed the gang"));
    let d = crate::systems::gossip::talk_district(world, betrayer);
    crate::systems::gossip::post_deed(world, d, crate::word::Deed::Betrayed, Some(betrayer), Some(gang));
}

// ---------------------------------------------------------------------------
// M12 D36, D40: splits and the hydra rule
// ---------------------------------------------------------------------------

/// D40, daily: a gang empty for `empty_claims_days` (0 = off) loses every
/// claim and its territory at once, not at the 30-day disband.
pub fn clear_claims_if_empty(world: &mut World, gang: EntityId) {
    let days = world.config.gangs.empty_claims_days;
    let now = world.tick;
    let due = world.comp::<Gang>(gang).is_some_and(|g| {
        days > 0
            && g.members.is_empty()
            && !g.claims_cleared
            && g.empty_since.is_some_and(|t| now.saturating_sub(t) >= days * TICKS_PER_DAY)
    });
    if !due {
        return;
    }
    let mut lost = 0usize;
    for kind in [BuildingKind::Home, BuildingKind::Bar, BuildingKind::Hotel] {
        for b in world.buildings_of_kind(kind).to_vec() {
            if let Some(bd) = world.comp_mut::<Building>(b) {
                if bd.claim.is_some_and(|c| c.gang == gang) {
                    bd.claim = None;
                    lost += 1;
                }
            }
        }
    }
    let name = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let held = world.comp::<Gang>(gang).map_or(0, |g| g.territory.len());
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.territory.clear();
        g.claims_cleared = true;
    }
    if lost > 0 || held > 0 {
        world.push_event(
            EventKind::GangLeave,
            &[gang],
            format!("{name} lost its claims on {lost} Blocks ({held} held): nobody left to hold them"),
        );
    }
}

/// D36: the districts a gang holds Homes in, `(district, held Homes)`,
/// ascending by district.
pub fn held_districts(world: &World, gang: EntityId) -> Vec<(DistrictId, usize)> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let mut counts: std::collections::BTreeMap<DistrictId, usize> = std::collections::BTreeMap::new();
    for &h in &g.territory {
        *counts.entry(world.district_of_building(h)).or_default() += 1;
    }
    counts.into_iter().collect()
}
