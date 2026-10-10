//! The Hit market (docs/HIT_MARKET.md; plan `~/.claude/plans/hit-market.md`):
//! organic demand for Hits and Beats.
//!
//! A game abstraction over M16a's contract records: a "want" is a daily
//! check over a store (the grudge store, the factions' purses) that ends in
//! `contracts::post_with`; the price is what the board's own candidates
//! would accept (`contracts::market_price`, mode A's score solved for the
//! price); the buyer pays with its wallet and the wallets of kin who share
//! its grudge (`Flow::Pool`), half down and the rest owed at fulfilment
//! (`contracts::stiff` when short). Every roll is a keyed draw on
//! `rng::CONTRACT_KEY` (`ContractNs::Want`); nothing here runs per tick.
//!
//! Phase 1: the price, the means, credit and Revenge. Phase 3: the gang and
//! corp passes. Phase 2 (silence, debt, the walk to the Fixer) waits.

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{Brain, GoalKind, Personality, RelKind, Wallet};
use crate::contract::{ContractKind, ContractNs, Motive, Origin, PostExtra, Posting, Refusal, Target};
use crate::entity::EntityId;
use crate::systems::contracts;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::word::GrudgeCause;
use crate::world::World;

/// H12: does `holder` pay rather than go himself: too weak to win
/// (`might_gap < hire_gap`, M16a's rule) or, with the market on, too
/// cowardly (`courage < hire_courage`).
pub fn hires(world: &World, holder: EntityId, target: EntityId) -> bool {
    if crate::systems::hunt::might_gap(world, holder, target) < world.config.contracts.hire_gap {
        return true;
    }
    world.config.hitmarket.enabled
        && world.comp::<Personality>(holder).is_some_and(|p| p.courage < world.config.hitmarket.hire_courage)
}

/// H11: organic records live now.
pub fn open_organic(world: &World) -> usize {
    world.contracts.values().filter(|c| c.is_live() && !c.motive.is_none()).count()
}

/// H11: organic records posted since the day began.
pub fn posted_today(world: &World) -> usize {
    let start = world.tick - world.tick % TICKS_PER_DAY;
    world.contracts.values().filter(|c| !c.motive.is_none() && c.posted >= start).count()
}

/// H11: may one more organic record be posted now?
pub fn room(world: &World) -> bool {
    let h = &world.config.hitmarket;
    open_organic(world) < h.max_open && posted_today(world) < h.posts_per_day
}

/// H4: the deposit an agent buyer puts down on `price`.
pub fn deposit_of(world: &World, price: i64) -> i64 {
    ((price as f32 * world.config.hitmarket.deposit_share).ceil() as i64).clamp(1, price.max(1))
}

fn wallet(world: &World, id: EntityId) -> i64 {
    world.comp::<Wallet>(id).map_or(0, |w| w.coins.max(0))
}

/// Decision 6: the chance a sharing relative chips in, from its loyalty to
/// the buyer (the edge's affinity, `-1..=1`) and its own lawfulness:
/// `(0.5 + 0.5 × affinity) × (1.5 − lawfulness)`, clamped to `0..=1`.
pub fn chip_p(affinity: f32, lawfulness: f32) -> f32 {
    ((0.5 + 0.5 * affinity.clamp(-1.0, 1.0)) * (1.5 - lawfulness.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

/// H3, decision 6: the relatives of `holder` who share its grudge on
/// `target` (Spouse, Parent, Family; Friends too for a revenge want: a
/// living adult with an unsettled grudge `≥ grudges.grudge_min` on the same
/// target) and roll to chip in, with what each would put in: `kin_share`
/// of its wallet. Ascending id. The refusals are counted.
pub fn kin_offers(world: &mut World, holder: EntityId, target: EntityId) -> SmallVec<[(EntityId, i64); 4]> {
    let min = world.config.grudges.grudge_min;
    let share = world.config.hitmarket.kin_share;
    let mut kin: Vec<(EntityId, f32)> = world
        .neighbours(holder)
        .filter_map(|o| world.edge(holder, o).map(|e| (o, e.kind, e.affinity)))
        .filter(|&(_, k, _)| matches!(k, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend))
        .filter(|&(o, _, _)| o != target && crate::systems::law::living(world, o))
        .filter(|&(o, _, _)| crate::systems::demography::is_adult(world, o))
        .filter(|&(o, _, _)| crate::systems::grudges::holds(world, o, target, min))
        .map(|(o, _, a)| (o, a))
        .collect();
    kin.sort_by_key(|k| k.0);
    let day = world.day();
    let mut out: SmallVec<[(EntityId, i64); 4]> = SmallVec::new();
    for (o, affinity) in kin {
        let amount = (wallet(world, o) as f32 * share).floor() as i64;
        if amount <= 0 {
            continue;
        }
        let law = world.comp::<Personality>(o).map_or(0.5, |p| p.lawfulness);
        let key = (u64::from(holder.index) << 32) | u64::from(o.index);
        let roll: f32 = world.rng.contract(ContractNs::Want, day, key).random();
        if roll < chip_p(affinity, law) {
            out.push((o, amount));
        } else {
            world.stats.current.hit.hm_refused += 1;
        }
    }
    out
}

/// Words for the event text: who the grudge is for.
fn revenge_why(world: &World, holder: EntityId, target: EntityId) -> Option<String> {
    let g = crate::systems::grudges::grudge_on(world, holder, target)?;
    match g.cause {
        GrudgeCause::KilledKin(o) | GrudgeCause::KilledFriend(o) | GrudgeCause::Inherited(o) => {
            Some(format!("for {}", world.name_of(o)))
        }
        GrudgeCause::Betrayed => Some("for a betrayal".to_string()),
        GrudgeCause::Robbed => Some("for a robbery".to_string()),
        GrudgeCause::Assaulted => Some("for a beating".to_string()),
        _ => None,
    }
}

/// The Hit market (H1-H5, H8, H10-H12) for a grudge holder: replaces
/// M16a's `try_hire`. A holder who hires (H12), under the day's brake and
/// with no live revenge record of its own on the target, prices the job at
/// the market (brokered with the nearest Fixer in reach, else direct),
/// puts `deposit_share` down from its wallet and, when short, from kin who
/// share the grudge and chip in, and posts. Short of the deposit: nothing
/// moves (`hm_unaffordable`). Tier-neutral: a Statistical holder posts at
/// once (H10). The Hunt cools for `hunt_cooldown_days` on a post.
pub fn try_revenge(world: &mut World, holder: EntityId, target: EntityId, weight: f32) -> bool {
    if !hires(world, holder, target) || !room(world) {
        return false;
    }
    let dup = world.by_party.get(&holder).is_some_and(|l| {
        l.iter().any(|id| {
            world.contracts.get(id).is_some_and(|c| {
                c.is_live() && c.buyer == Some(holder) && c.target == Target::Agent(target) && c.kind.violent()
            })
        })
    });
    if dup {
        return false;
    }
    let kind = if weight >= world.config.hunt.lethal_min { ContractKind::Hit } else { ContractKind::Beat };
    let tgt = Target::Agent(target);
    let broker = contracts::nearest_fixer(world, holder);
    let price = contracts::organic_price(world, kind, &tgt, Some(holder), Some(holder), broker);
    let deposit = deposit_of(world, price);
    let own = wallet(world, holder);
    let mut pooled: SmallVec<[(EntityId, i64); 4]> = SmallVec::new();
    if own < deposit {
        let mut need = deposit - own;
        for (o, amount) in kin_offers(world, holder, target) {
            if need <= 0 {
                break;
            }
            let give = amount.min(need);
            pooled.push((o, give));
            need -= give;
        }
        if need > 0 {
            world.stats.current.hit.hm_unaffordable += 1;
            return false;
        }
    }
    for &(o, give) in &pooled {
        let moved = ownership::pay(world, Some(o), Some(holder), give, Flow::Pool);
        world.stats.current.hit.hm_pooled += moved;
    }
    let posting = Posting {
        buyer: Some(holder),
        agent: Some(holder),
        kind,
        target: tgt,
        broker,
        deadline_days: world.config.contracts.deadline_days.get(kind),
        origin: Origin::Hunt,
        price: Some(price),
    };
    let extra = PostExtra {
        deposit: (deposit < price).then_some(deposit),
        motive: Motive::Revenge,
        why: revenge_why(world, holder, target),
    };
    let r = contracts::post_with(world, posting, extra);
    let Ok(id) = r else {
        if matches!(r, Err(Refusal::MaxOpen)) {
            world.stats.current.contract.contracts_refused_full += 1;
        }
        // Nothing was bought: the chip-ins go back.
        for &(o, give) in &pooled {
            ownership::pay(world, Some(holder), Some(o), give, Flow::Pool);
            world.stats.current.hit.hm_pooled -= give;
        }
        return false;
    };
    // H5: who paid knows.
    for &(o, _) in &pooled {
        contracts::hear_hired(world, id, o);
    }
    let until = world.tick + Tick::from(world.config.hunt.hunt_cooldown_days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(holder) {
        b.cooldowns.insert(GoalKind::Hunt, until);
    }
    true
}

// ---------------------------------------------------------------------------
// Phase 3: the gangs' and corps' business (H18, H19)
// ---------------------------------------------------------------------------

/// H18 (plan D11): `victim` was wronged by the rival corp `by` (a hostile
/// buyout, a poached hand, an undercut): kept on `Corp.last_wronged`.
pub fn wronged(world: &mut World, victim: EntityId, by: EntityId, how: crate::contract::Wrong) {
    if victim == by || !world.has::<crate::components::Corp>(by) {
        return;
    }
    let now = world.tick;
    if let Some(c) = world.comp_mut::<crate::components::Corp>(victim) {
        c.last_wronged = Some((by, now, how));
    }
}

/// An organic record by `buyer` on `target` live now, or closed within
/// `repost_days` (the faction waits before buying the same head again).
fn bought_lately(world: &World, buyer: EntityId, target: EntityId) -> bool {
    let wait = Tick::from(world.config.hitmarket.repost_days) * TICKS_PER_DAY;
    let now = world.tick;
    world.contracts.values().any(|c| {
        c.buyer == Some(buyer)
            && c.target == Target::Agent(target)
            && !c.motive.is_none()
            && (c.is_live() || c.closed.is_some_and(|t| now.saturating_sub(t) < wait))
    })
}

/// A faction's organic post at the market price, paid in full from its
/// purse when `budget` covers it. True when posted.
#[allow(clippy::too_many_arguments)]
fn faction_post(
    world: &mut World,
    buyer: EntityId,
    agent: Option<EntityId>,
    kind: ContractKind,
    target: EntityId,
    broker: Option<EntityId>,
    origin: Origin,
    budget: i64,
    motive: Motive,
    why: Option<String>,
) -> bool {
    if !room(world) || bought_lately(world, buyer, target) {
        return false;
    }
    let tgt = Target::Agent(target);
    let price = contracts::organic_price(world, kind, &tgt, Some(buyer), agent, broker);
    if price > budget || world.purse(Some(buyer)) < price {
        world.stats.current.hit.hm_unaffordable += 1;
        return false;
    }
    let extra = PostExtra { deposit: None, motive, why };
    contracts::brain_post_with(world, Some(buyer), agent, kind, tgt, broker, origin, Some(price), extra).is_ok()
}

/// H19, daily: each gang with a leader buys at most one head a day, the
/// first of: its open vendettas' rival gang leaders (`Rival`; the whole
/// `gang_spend` share when it is the weaker side, half otherwise), else the
/// heaviest Hunt-eligible grudge its leader holds on an outsider
/// (`Revenge`, by weight). Brokered at an open Fixer in a held district,
/// else direct to its own members.
pub fn gang_pass(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    for g in world.gangs() {
        let Some((leader, members, order)) =
            world.comp::<crate::components::Gang>(g).and_then(|x| x.leader.map(|l| (l, x.members.len(), x.order)))
        else {
            continue;
        };
        if !crate::systems::law::living(world, leader) {
            continue;
        }
        let treasury = world.purse(Some(g));
        if treasury <= 0 {
            continue;
        }
        let share = world.config.hitmarket.gang_spend;
        let broker = contracts::fixer_in_held(world, g);
        let origin = Origin::GangOrder(order);
        let mut rivals: Vec<EntityId> = world
            .vendettas
            .iter()
            .filter_map(|v| {
                if v.a == g {
                    Some(v.b)
                } else if v.b == g {
                    Some(v.a)
                } else {
                    None
                }
            })
            .filter(|&r| world.has::<crate::components::Gang>(r))
            .collect();
        rivals.sort_unstable();
        rivals.dedup();
        let mut posted = false;
        for r in rivals {
            let Some(head) = contracts::head_of(world, r) else { continue };
            let theirs = world.comp::<crate::components::Gang>(r).map_or(0, |x| x.members.len()).max(1);
            let weaker = (members as f32 / theirs as f32) < world.config.contracts.hire_ratio;
            let budget = (treasury as f32 * share * if weaker { 1.0 } else { 0.5 }) as i64;
            let why = Some(format!("in the feud with {}", world.owner_label(Some(r))));
            if faction_post(world, g, Some(leader), ContractKind::Hit, head, broker, origin, budget, Motive::Rival, why)
            {
                posted = true;
                break;
            }
        }
        if posted {
            continue;
        }
        let Some((t, weight, _)) = crate::systems::hunt::heaviest_eligible(world, leader) else { continue };
        if crate::systems::grudges::member_of(world, t, g) {
            continue;
        }
        let kind = if weight >= world.config.hunt.lethal_min { ContractKind::Hit } else { ContractKind::Beat };
        let budget = (treasury as f32 * share) as i64;
        let why = revenge_why(world, leader, t);
        faction_post(world, g, Some(leader), kind, t, broker, origin, budget, Motive::Revenge, why);
    }
}

/// Decision 4: corp Rival Hits on execs posted in the last 365 days.
fn corp_hits_this_year(world: &mut World) -> usize {
    let year = 365 * TICKS_PER_DAY;
    let now = world.tick;
    while world.hm_corp_hits.front().is_some_and(|&t| now.saturating_sub(t) >= year) {
        world.hm_corp_hits.pop_front();
    }
    world.hm_corp_hits.len()
}

/// H18, decision 4, daily: a corp whose exec's lawfulness is under
/// `dirty_beat` buys its rival corp's exec: a rival is the last corp to
/// wrong it (`Corp.last_wronged`) within `rival_window_days`, or a corp in
/// an open vendetta with it. A Hit under `[contracts] dirty_lobby`, else a
/// Beat; at most one Rival record per corp per `corp_cooldown_days`, Hits
/// at most `corp_year_cap` a year city-wide; paid in full from `treasury ÷
/// corp_reserve_mult`, brokered at the open Fixer nearest the target.
pub fn corp_pass(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    let now = world.tick;
    let h = world.config.hitmarket.clone();
    let window = Tick::from(h.rival_window_days) * TICKS_PER_DAY;
    let cooldown = Tick::from(h.corp_cooldown_days) * TICKS_PER_DAY;
    for c in world.corps() {
        let Some(corp) = world.comp::<crate::components::Corp>(c) else { continue };
        let (order, wronged) = (corp.order, corp.last_wronged);
        let Some(exec) = contracts::head_of(world, c) else { continue };
        let law = world.comp::<Personality>(exec).map_or(1.0, |p| p.lawfulness);
        if law >= h.dirty_beat {
            continue;
        }
        let recent = world
            .contracts
            .values()
            .any(|x| x.buyer == Some(c) && x.motive == Motive::Rival && now.saturating_sub(x.posted) < cooldown);
        if recent {
            continue;
        }
        let mut rivals: Vec<(EntityId, String)> = Vec::new();
        if let Some((by, _, how)) = wronged.filter(|&(_, t, _)| now.saturating_sub(t) < window) {
            rivals.push((by, format!("over {}", how.label())));
        }
        let mut feuds: Vec<EntityId> = world
            .vendettas
            .iter()
            .filter_map(|v| {
                if v.a == c {
                    Some(v.b)
                } else if v.b == c {
                    Some(v.a)
                } else {
                    None
                }
            })
            .filter(|&r| world.has::<crate::components::Corp>(r))
            .collect();
        feuds.sort_unstable();
        rivals.extend(feuds.into_iter().map(|r| (r, "in the feud".to_string())));
        let kind = if law < world.config.contracts.dirty_lobby { ContractKind::Hit } else { ContractKind::Beat };
        if kind == ContractKind::Hit && corp_hits_this_year(world) >= h.corp_year_cap as usize {
            continue;
        }
        let budget = (world.purse(Some(c)) as f32 / h.corp_reserve_mult.max(1.0)) as i64;
        for (r, why) in rivals {
            if !world.has::<crate::components::Corp>(r) {
                continue;
            }
            let Some(head) = contracts::head_of(world, r) else { continue };
            let Some(at) = world.comp::<crate::components::Position>(head).map(|p| p.tile) else { continue };
            let broker = contracts::nearest_fixer_to(world, at);
            let why = Some(format!("{why} with {}", world.owner_label(Some(r))));
            let origin = Origin::CorpOrder(order);
            if faction_post(world, c, Some(exec), kind, head, broker, origin, budget, Motive::Rival, why) {
                if kind == ContractKind::Hit {
                    world.hm_corp_hits.push_back(now);
                }
                break;
            }
        }
    }
}
