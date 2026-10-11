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
//! corp passes. Phase 2: the walk to the Fixer (`GoalKind::Commission`, a
//! want in `World::wants`), Silence, Debt, Protect, cold cases and
//! recanting (the law's teeth), and the buyer after.

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{
    ActionInstance, Brain, Crime, GoalKind, Lod, Memory, MemoryEntry, MemoryKind, Personality, RelKind, Wallet,
};
use crate::contract::{
    Broker, Contract, ContractId, ContractKind, ContractNs, DebtClock, Motive, Origin, PostExtra, Posting, Refusal,
    Target, Want,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{FailReason, StepResult};
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::contracts;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::word::{Deed, GrudgeCause};
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

/// H3: the relatives of `holder` who share its grudge on `target`
/// (Spouse, Parent, Family, Friend: a living adult with an unsettled
/// grudge `≥ grudges.grudge_min` on the same target) with the edge's
/// affinity, ascending id.
fn sharing_kin(world: &World, holder: EntityId, target: EntityId) -> Vec<(EntityId, f32)> {
    let min = world.config.grudges.grudge_min;
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
    kin
}

/// H3, decision 6: the relatives of `holder` who share its grudge on
/// `target` (Spouse, Parent, Family; Friends too for a revenge want: a
/// living adult with an unsettled grudge `≥ grudges.grudge_min` on the same
/// target) and roll to chip in, with what each would put in: `kin_share`
/// of its wallet. Ascending id. The refusals are counted.
pub fn kin_offers(world: &mut World, holder: EntityId, target: EntityId) -> SmallVec<[(EntityId, i64); 4]> {
    let share = world.config.hitmarket.kin_share;
    let kin = sharing_kin(world, holder, target);
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
        // The dead's name while its entity stands; once the body is gone
        // (despawned: `name_of` reads `#id`) the deed alone.
        GrudgeCause::KilledKin(o) | GrudgeCause::KilledFriend(o) | GrudgeCause::Inherited(o) => {
            if world.has::<crate::components::Identity>(o) {
                Some(format!("for {}", world.name_of(o)))
            } else {
                Some("for a killing".to_string())
            }
        }
        GrudgeCause::Betrayed => Some("for a betrayal".to_string()),
        GrudgeCause::Robbed => Some("for a robbery".to_string()),
        GrudgeCause::Assaulted => Some("for a beating".to_string()),
        _ => None,
    }
}

/// The Hit market (H1-H5, H8, H10-H12) for a grudge holder: replaces
/// M16a's `try_hire`. A holder who hires (H12) buys a Hit or a Beat on the
/// target by weight through [`agent_buy`] (kin who share the grudge chip
/// in). The Hunt cools for `hunt_cooldown_days` on a post or a want (D8:
/// no Hunt on the target while the want stands).
pub fn try_revenge(world: &mut World, holder: EntityId, target: EntityId, weight: f32) -> bool {
    if !hires(world, holder, target) {
        return false;
    }
    let kind = if weight >= world.config.hunt.lethal_min { ContractKind::Hit } else { ContractKind::Beat };
    agent_buy(world, holder, target, kind, Motive::Revenge, weight, None)
}

/// D8 (phase 4 fix): `buyer` is paying for a Hit or a Beat on `target`: a
/// want on it, or a live organic record it bought. A buyer does not hunt
/// the head it paid for while the record stands (the diaries: a buyer's
/// Hunt cooled for `hunt_cooldown_days` at the post, then hunted the target
/// with the gun still on it; one adopted through the hire path had its
/// cooldown shortened to the planner's `goal_cooldown_ticks` the same tick
/// and killed the target herself 3 hours after paying).
pub fn paying_for(world: &World, buyer: EntityId, target: EntityId) -> bool {
    world.wants.get(&buyer).is_some_and(|w| w.target == target)
        || world.by_party.get(&buyer).is_some_and(|l| {
            l.iter().any(|id| {
                world.contracts.get(id).is_some_and(|c| {
                    c.is_live() && !c.motive.is_none() && c.buyer == Some(buyer) && c.target == Target::Agent(target)
                })
            })
        })
}

/// A live organic record of `buyer`'s on `target`, or one closed within
/// `repost_days` (the buyer waits before buying the same head again).
fn buying(world: &World, buyer: EntityId, target: EntityId) -> bool {
    let wait = Tick::from(world.config.hitmarket.repost_days) * TICKS_PER_DAY;
    let now = world.tick;
    world.by_party.get(&buyer).is_some_and(|l| {
        l.iter().any(|id| {
            world.contracts.get(id).is_some_and(|c| {
                c.buyer == Some(buyer)
                    && c.target == Target::Agent(target)
                    && c.kind.violent()
                    && (c.is_live() || (!c.motive.is_none() && c.closed.is_some_and(|t| now.saturating_sub(t) < wait)))
            })
        })
    })
}

/// H9: does `id`'s body walk the city (Full or Coarse)?
fn walks(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
}

/// Words for the event text, by motive.
fn why_of(world: &World, buyer: EntityId, target: EntityId, motive: Motive) -> Option<String> {
    match motive {
        Motive::Revenge => revenge_why(world, buyer, target),
        Motive::Silence => silence_report(world, buyer, target).map(|c| format!("over the {}", c.label())),
        Motive::Debt => Some(format!("over {} owed", owed_to(world, buyer, target) + arrears_to(world, buyer, target))),
        _ => None,
    }
}

/// The Hit market's agent buyer (H4, H5, H9-H11; phase 2): `buyer` buys a
/// `kind` record on `target` for `motive`. `at`: the Fixer it stands at
/// (the `Commission` step), else `None` (the daily passes). Under the
/// day's brake, not buying that head already: priced at the market
/// (brokered at the Fixer in reach, else direct), `deposit_share` down
/// from its wallet topped up, for Revenge, by kin who share the grudge and
/// chip in. A buyer whose body walks (Full or Coarse) with a Fixer in
/// reach and no `at` writes a want instead (H9: the walk is the errand; no
/// coin moves, the kin are asked at the Fixer). With no Fixer in reach the
/// post is direct, at once; a Statistical buyer posts at once (H10).
/// Short of the deposit nothing moves (`hm_unaffordable`). True when
/// posted or wanted, or a want on this head already stands (a want on
/// another head: false). A Revenge buyer's Hunt cools (D8).
pub fn agent_buy(
    world: &mut World,
    buyer: EntityId,
    target: EntityId,
    kind: ContractKind,
    motive: Motive,
    weight: f32,
    at: Option<EntityId>,
) -> bool {
    if at.is_none() {
        if let Some(w) = world.wants.get(&buyer) {
            // Already on the way to a Fixer: one errand at a time. The
            // errand stands for this head (any motive, any kind: the buyer
            // is paying for violence on it, as `paying_for` reads); a want
            // on another head buys nothing here (review fix: it reported a
            // hire that never happened).
            return w.target == target;
        }
    }
    if !room(world) || buying(world, buyer, target) {
        return false;
    }
    let tgt = Target::Agent(target);
    let broker = at.or_else(|| contracts::nearest_fixer(world, buyer));
    let price = contracts::organic_price(world, kind, &tgt, Some(buyer), Some(buyer), broker);
    let deposit = deposit_of(world, price);
    let own = wallet(world, buyer);
    let kin = motive == Motive::Revenge;
    if let (None, Some(fixer), true) = (at, broker, walks(world, buyer)) {
        // H9: the want waits for the walk; the kin are counted, not asked.
        let could = own + if kin { kin_potential(world, buyer, target) } else { 0 };
        if could < deposit {
            world.stats.current.hit.hm_unaffordable += 1;
            return false;
        }
        let since = world.tick;
        world.wants.insert(buyer, Want { target, kind, motive, weight, since, fixer });
        world.stats.current.hit.hm_wants += 1;
    } else {
        let mut pooled: SmallVec<[(EntityId, i64); 4]> = SmallVec::new();
        if own < deposit {
            let mut need = deposit - own;
            if kin {
                for (o, amount) in kin_offers(world, buyer, target) {
                    if need <= 0 {
                        break;
                    }
                    let give = amount.min(need);
                    pooled.push((o, give));
                    need -= give;
                }
            }
            if need > 0 {
                world.stats.current.hit.hm_unaffordable += 1;
                return false;
            }
        }
        for &(o, give) in &pooled {
            let moved = ownership::pay(world, Some(o), Some(buyer), give, Flow::Pool);
            world.stats.current.hit.hm_pooled += moved;
        }
        let posting = Posting {
            buyer: Some(buyer),
            agent: Some(buyer),
            kind,
            target: tgt,
            broker,
            deadline_days: world.config.contracts.deadline_days.get(kind),
            origin: Origin::Hunt,
            price: Some(price),
        };
        let extra = PostExtra {
            deposit: (deposit < price).then_some(deposit),
            motive,
            why: why_of(world, buyer, target, motive),
        };
        let r = contracts::post_with(world, posting, extra);
        let Ok(id) = r else {
            if matches!(r, Err(Refusal::MaxOpen)) {
                world.stats.current.contract.contracts_refused_full += 1;
            }
            // Nothing was bought: the chip-ins go back.
            for &(o, give) in &pooled {
                ownership::pay(world, Some(buyer), Some(o), give, Flow::Pool);
                world.stats.current.hit.hm_pooled -= give;
            }
            return false;
        };
        // H5: who paid knows.
        for &(o, _) in &pooled {
            contracts::hear_hired(world, id, o);
        }
        if at.is_some() {
            world.stats.current.hit.hm_commissions += 1;
        }
    }
    if kin {
        let until = world.tick + Tick::from(world.config.hunt.hunt_cooldown_days) * TICKS_PER_DAY;
        if let Some(b) = world.comp_mut::<Brain>(buyer) {
            b.cooldowns.insert(GoalKind::Hunt, until);
        }
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
pub fn bought_lately(world: &World, buyer: EntityId, target: EntityId) -> bool {
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
    // Phase 2 (D12): yesterday's untaken Protect records go to a Fixer.
    rebroker_protect(world);
    let silence = silenceable(world);
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
        // Phase 2: Protect first (a member's witness), then the feuds.
        let budget = (treasury as f32 * share) as i64;
        if protect(world, g, leader, budget, order, &silence) {
            continue;
        }
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
/// `dirty_beat` buys its rival corp's exec: a rival is a corp in an open
/// vendetta with it, else the last corp to wrong it (`Corp.last_wronged`)
/// within `rival_window_days` (the spec's Rival row: the vendetta first). A Hit under `[contracts] dirty_lobby`, else a
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
        // The spec's Rival row (review fix): the open vendettas first, then
        // the latest wrong (a feuding corp that also wronged us is tried
        // once, as a feud).
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
        feuds.dedup();
        let mut rivals: Vec<(EntityId, String)> = feuds.iter().map(|&r| (r, "in the feud".to_string())).collect();
        if let Some((by, _, how)) = wronged.filter(|&(_, t, _)| now.saturating_sub(t) < window) {
            if !feuds.contains(&by) {
                rivals.push((by, format!("over {}", how.label())));
            }
        }
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

// ---------------------------------------------------------------------------
// Phase 2: the walk to the Fixer (H9), Silence (H16), Debt, Protect (D12),
// the law's teeth (H14, H15) and the buyer after (H17)
// ---------------------------------------------------------------------------

/// H3 before the roll: what the relatives who share `holder`'s grudge on
/// `target` would put in at most (the want's check; they are asked at the
/// Fixer, `kin_offers`).
fn kin_potential(world: &World, holder: EntityId, target: EntityId) -> i64 {
    let share = world.config.hitmarket.kin_share;
    sharing_kin(world, holder, target).into_iter().map(|(o, _)| (wallet(world, o) as f32 * share).floor() as i64).sum()
}

/// Is the want still a want: the buyer free, the target living, the motive
/// standing (the grudge unsettled at `hunt_min`, the report open, the debt
/// on the clock).
fn still_wants(world: &World, id: EntityId, w: &Want) -> bool {
    if !contracts::free_adult(world, id) || !crate::systems::law::living(world, w.target) {
        return false;
    }
    match w.motive {
        // D8: a Hunt on the target by other means (a god's) ends the want.
        Motive::Revenge => {
            crate::systems::grudges::grudge_on(world, id, w.target)
                .is_some_and(|g| g.weight >= world.config.hunt.hunt_min)
                && world.hunts.get(&id).is_none_or(|h| h.target != w.target)
        }
        Motive::Silence => silence_report(world, id, w.target).is_some(),
        Motive::Debt => world.hm_debts.contains_key(&(id, w.target)),
        _ => true,
    }
}

/// H9: the Commission goal: a want held, its Fixer open, the buyer free.
/// The motive's weight → Linear{0.8, 0.2}; `1 − lawfulness` →
/// Linear{0.5, 0.5}; flat `commission_flat`. A map lookup first (every
/// think).
pub fn commission_considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if world.wants.is_empty() {
        return None;
    }
    let w = world.wants.get(&id)?;
    if !contracts::fixer_open(world, w.fixer) || !contracts::free_adult(world, id) {
        return None;
    }
    let p = world.comp::<Personality>(id)?;
    let cs = vec![
        Consideration::new("holds a want", can(true), GATE),
        Consideration::new("motive weight", w.weight, Curve::Linear { m: 0.8, b: 0.2 }),
        Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    Some((cs, world.config.hitmarket.commission_flat))
}

/// H9: the Commission goal's scripted plan (`plan::plan_for`'s bypass;
/// `None` cools the goal): `GoTo(Fixer) → Commission`.
pub fn commission_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let f = world.wants.get(&id)?.fixer;
    if !contracts::fixer_open(world, f) {
        return None;
    }
    let step = |action, target| ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Seller), Some(f)), step(ActionKind::Commission, Some(f))];
    Some(Plan { goal: GoalKind::Commission, target: Some(f), steps, started_tick: world.tick })
}

/// H9: `Commission` completes at the Fixer: the want is priced, paid and
/// posted there (`agent_buy` with `at`), or dropped when it no longer
/// stands or cannot be paid.
pub fn on_commission(world: &mut World, id: EntityId, fixer: Option<EntityId>) -> StepResult {
    let Some(w) = world.wants.remove(&id) else { return StepResult::Done };
    let Some(f) = fixer.filter(|&f| contracts::fixer_open(world, f)) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    if still_wants(world, id, &w) {
        agent_buy(world, id, w.target, w.kind, w.motive, w.weight, Some(f));
    }
    StepResult::Done
}

/// H8, daily: a want that no longer stands, whose Fixer closed or that is
/// `want_days` old lapses; a buyer demoted to Statistical posts at once
/// (H10). Heaviest first.
pub fn want_sweep(world: &mut World) {
    if world.wants.is_empty() {
        return;
    }
    let now = world.tick;
    let days = Tick::from(world.config.hitmarket.want_days) * TICKS_PER_DAY;
    let mut list: Vec<(EntityId, Want)> = world.wants.iter().map(|(&id, w)| (id, w.clone())).collect();
    list.sort_by(|a, b| b.1.weight.total_cmp(&a.1.weight).then(a.0.cmp(&b.0)));
    for (id, w) in list {
        let stale = now.saturating_sub(w.since) >= days || !contracts::fixer_open(world, w.fixer);
        if stale || !still_wants(world, id, &w) {
            world.wants.remove(&id);
            continue;
        }
        if !walks(world, id) {
            world.wants.remove(&id);
            agent_buy(world, id, w.target, w.kind, w.motive, w.weight, None);
        }
    }
}

/// Silence and Protect: the crimes a suspect (or its gang) silences a
/// witness over, the kind bought and the motive's weight.
fn silence_terms(crime: Crime) -> Option<(ContractKind, f32)> {
    match crime {
        Crime::Murder => Some((ContractKind::Hit, 1.0)),
        Crime::Abduction => Some((ContractKind::Beat, 0.9)),
        Crime::Assault => Some((ContractKind::Beat, 0.7)),
        Crime::GrandTheft => Some((ContractKind::Beat, 0.6)),
        _ => None,
    }
}

/// H16 (plan D9): `suspect` knows `witness`: they share an edge (any kind),
/// or the witness lives in a district the suspect's gang holds (the street
/// tells). Nobody else is a silencing target.
pub fn knows_witness(world: &World, suspect: EntityId, witness: EntityId) -> bool {
    if world.edge(suspect, witness).is_some() {
        return true;
    }
    let Some(g) = world.gang_of(suspect) else { return false };
    let Some(d) = crate::systems::gossip::home_district(world, witness) else { return false };
    crate::systems::gang::held_districts(world, g).iter().any(|&(x, _)| x == d)
}

/// The crime of the open report naming `suspect` that `witness` filed,
/// among the silenced kinds (the most weighty).
fn silence_report(world: &World, suspect: EntityId, witness: EntityId) -> Option<Crime> {
    world
        .crime_reports()
        .iter()
        .filter(|r| !r.resolved && r.suspect == suspect && r.witness == Some(witness))
        .filter_map(|r| silence_terms(r.crime).map(|(_, w)| (w, r.crime)))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// The open reports a suspect could silence: `(weight, suspect, witness,
/// kind)`, the witness living and no guard, the suspect a free adult who
/// knows the witness (H16); one row per pair, the weightiest crime;
/// heaviest first, then ids.
fn silenceable(world: &World) -> Vec<(f32, EntityId, EntityId, ContractKind)> {
    let mut best: std::collections::BTreeMap<(EntityId, EntityId), (f32, ContractKind)> =
        std::collections::BTreeMap::new();
    // scan-ok: daily, the open reports.
    for r in world.crime_reports().iter().filter(|r| !r.resolved) {
        let (Some(w), Some((kind, weight))) = (r.witness, silence_terms(r.crime)) else { continue };
        if w == r.suspect || !crate::systems::law::living(world, w) || crate::systems::law::is_guard(world, w) {
            continue;
        }
        let e = best.entry((r.suspect, w)).or_insert((weight, kind));
        if weight > e.0 {
            *e = (weight, kind);
        }
    }
    let mut out: Vec<(f32, EntityId, EntityId, ContractKind)> = best
        .into_iter()
        .filter(|&((s, w), _)| contracts::free_adult(world, s) && knows_witness(world, s, w))
        .map(|((s, w), (weight, kind))| (weight, s, w, kind))
        .collect();
    out.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    out
}

/// A live organic Hit or Beat on `target` (any buyer): that head is being
/// bought already.
fn on_the_board(world: &World, target: EntityId) -> bool {
    world.by_party.get(&target).is_some_and(|l| {
        l.iter().any(|id| {
            world.contracts.get(id).is_some_and(|c| {
                c.is_live() && c.kind.violent() && !c.motive.is_none() && c.target == Target::Agent(target)
            })
        })
    })
}

/// Silence (§ 2, H16), daily: a free suspect under `silence_lawfulness`
/// with an open Murder (a Hit), Abduction, Assault or Grand Theft (a Beat)
/// report whose witness it knows buys the witness ([`agent_buy`]: means
/// and credit, no kin). A gang member's reports are its gang's to buy
/// (Protect). Heaviest first.
pub fn silence_pass(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    let max = world.config.hitmarket.silence_lawfulness;
    for (weight, s, w, kind) in silenceable(world) {
        if world.gang_of(s).is_some() || on_the_board(world, w) {
            continue;
        }
        if world.comp::<Personality>(s).is_none_or(|p| p.lawfulness >= max) {
            continue;
        }
        agent_buy(world, s, w, kind, Motive::Silence, weight, None);
    }
}

/// What `debtor` owes `creditor` on their edge (0 when nothing).
fn owed_to(world: &World, creditor: EntityId, debtor: EntityId) -> i64 {
    let Some(e) = world.edge(creditor, debtor) else { return 0 };
    let (lo, _) = crate::components::edge_key(creditor, debtor);
    let owed = if lo == debtor { e.debt } else { -e.debt };
    i64::from(owed.max(0))
}

/// What `debtor` is behind on the financed assets `lender` lent it, in
/// arrears of `assets.repo_days` or more (the Debt pass's finance source):
/// `per_day × arrears` each, at most what remains (0 when nothing).
fn arrears_to(world: &World, lender: EntityId, debtor: EntityId) -> i64 {
    let repo = world.config.assets.repo_days;
    // scan-ok: at a Debt post only, the assets.
    world
        .with::<crate::components::Asset>()
        .into_iter()
        .filter_map(|a| world.comp::<crate::components::Asset>(a))
        .filter(|x| x.owner == Some(debtor))
        .filter_map(|x| x.finance.as_ref())
        .filter(|f| f.lender == Some(lender) && f.arrears >= repo)
        .map(|f| (f.per_day * i64::from(f.arrears)).min(f.remaining).max(0))
        .sum()
}

/// Debt (§ 2), daily: every edge debt of `debt_min` or more (one unordered
/// pass over the edges), and every financed asset in arrears of
/// `assets.repo_days` or more still on its owner (nothing was
/// repossessed) whose lender is an agent or a gang, is a debt on the clock
/// (`World::hm_debts`, first seen; the clock drops when the debt does).
/// After `debt_days` a creditor under `debt_lawfulness` (an agent through
/// [`agent_buy`], a gang from `gang_spend` × treasury) buys a Beat on the
/// debtor; when a Debt Beat on the debtor is `debt_hit_days` old and the
/// debt stands, a Hit.
pub fn debt_pass(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    let h = world.config.hitmarket.clone();
    let mut owed: Vec<(EntityId, EntityId)> = Vec::new();
    // scan-ok: daily, one unordered pass over the edges (sorted below).
    for (&(a, b), e) in world.edges.iter_mut_unordered() {
        if e.debt >= h.debt_min {
            owed.push((b, a));
        } else if e.debt <= -h.debt_min {
            owed.push((a, b));
        }
    }
    let repo = world.config.assets.repo_days;
    // scan-ok: daily, the assets.
    for a in world.with::<crate::components::Asset>() {
        let Some(x) = world.comp::<crate::components::Asset>(a) else { continue };
        let (Some(f), Some(owner)) = (x.finance.as_ref(), x.owner) else { continue };
        if f.arrears < repo || !world.has::<crate::components::Identity>(owner) {
            continue;
        }
        let Some(l) = f.lender else { continue };
        if world.has::<crate::components::Identity>(l) || world.has::<crate::components::Gang>(l) {
            owed.push((l, owner));
        }
    }
    owed.sort_unstable();
    owed.dedup();
    let now = world.tick;
    let keep: std::collections::BTreeSet<(EntityId, EntityId)> = owed.iter().copied().collect();
    world.hm_debts.retain(|k, _| keep.contains(k));
    for &k in &owed {
        world.hm_debts.entry(k).or_insert(DebtClock { seen: now, beat: None });
    }
    let age = Tick::from(h.debt_days) * TICKS_PER_DAY;
    let wait = Tick::from(h.debt_hit_days) * TICKS_PER_DAY;
    for (c, d) in owed {
        let Some(clock) = world.hm_debts.get(&(c, d)).copied() else { continue };
        if now.saturating_sub(clock.seen) < age || !crate::systems::law::living(world, d) {
            continue;
        }
        let (kind, weight) = match clock.beat {
            None => (ContractKind::Beat, 0.6),
            Some(t) if now.saturating_sub(t) >= wait => (ContractKind::Hit, 0.9),
            Some(_) => continue,
        };
        if world.has::<crate::components::Gang>(c) {
            let Some((leader, order)) =
                world.comp::<crate::components::Gang>(c).and_then(|g| g.leader.map(|l| (l, g.order)))
            else {
                continue;
            };
            if !crate::systems::law::living(world, leader) || crate::systems::grudges::member_of(world, d, c) {
                continue;
            }
            let budget = (world.purse(Some(c)) as f32 * h.gang_spend) as i64;
            let broker = contracts::fixer_in_held(world, c);
            let why = Some("over a debt".to_string());
            faction_post(world, c, Some(leader), kind, d, broker, Origin::GangOrder(order), budget, Motive::Debt, why);
            continue;
        }
        // Review fix: a dead creditor keeps its edges, a jailed or cuffed one
        // its wallet; only a living free adult buys (as Silence's suspect).
        if !contracts::free_adult(world, c) {
            continue;
        }
        if world.comp::<Personality>(c).is_none_or(|p| p.lawfulness >= h.debt_lawfulness) {
            continue;
        }
        agent_buy(world, c, d, kind, Motive::Debt, weight, None);
    }
}

/// Protect (§ 2, D12), from `gang_pass`: a gang buys the witness of one of
/// its members' open reports (the member knows the witness, H16), a Beat
/// (a Hit for Murder), direct to its own members first. True when posted.
fn protect(
    world: &mut World,
    g: EntityId,
    leader: EntityId,
    budget: i64,
    order: crate::components::Order,
    cands: &[(f32, EntityId, EntityId, ContractKind)],
) -> bool {
    for &(_, s, w, kind) in cands {
        if world.gang_of(s) != Some(g) || on_the_board(world, w) || crate::systems::grudges::member_of(world, w, g) {
            continue;
        }
        let why = Some(format!("for {}", world.name_of(s)));
        if faction_post(world, g, Some(leader), kind, w, None, Origin::GangOrder(order), budget, Motive::Protect, why) {
            return true;
        }
    }
    false
}

/// Plan D12, daily: a gang's direct Protect record nobody of its own took
/// at the last `match_hour` moves to the open Fixer in a district the gang
/// holds: the price into escrow from the gang's purse, the broker set (one
/// record, not two), a log line.
fn rebroker_protect(world: &mut World) {
    let now = world.tick;
    let ids: Vec<ContractId> = world
        .contracts
        .values()
        .filter(|c| c.motive == Motive::Protect && c.is_open() && c.broker.is_none())
        .filter(|c| now.saturating_sub(c.posted) >= TICKS_PER_DAY)
        .map(|c| c.id)
        .collect();
    for id in ids {
        let Some((g, price)) = world.contracts.get(&id).and_then(|c| c.buyer.map(|b| (b, c.price))) else { continue };
        let Some(f) = contracts::fixer_in_held(world, g) else { continue };
        if world.purse(Some(g)) < price {
            continue;
        }
        let moved = ownership::escrow_in(world, Some(g), id, price);
        if moved < price {
            ownership::escrow_out(world, id, Some(g), moved, Flow::Escrow);
            continue;
        }
        if let Some(c) = world.contracts.get_mut(&id) {
            c.broker = Some(f);
            c.renege = false;
        }
        if let Some(k) = world.comp_mut::<Broker>(f) {
            k.book.push(id);
        }
        if let Some(owner) = world.owner_of(f).filter(|&o| world.has::<crate::components::Identity>(o)) {
            contracts::hear_hired(world, id, owner);
        }
        let text = format!("no taker among its own: re-posted at {}", world.name_of(f));
        contracts::note(world, id, text);
    }
}

/// H14, daily: an open report whose witness is dead (or gone) `cold_days`
/// after the death, with no city guard holding a `SawCrime` of its suspect
/// and crime, goes cold: resolved, `CaseCold`. No witness, no case.
pub fn cold_cases(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    let now = world.tick;
    let wait = Tick::from(world.config.hitmarket.cold_days) * TICKS_PER_DAY;
    let mut cold: Vec<(usize, EntityId, EntityId, Crime)> = Vec::new();
    // scan-ok: daily, the reports.
    for (i, r) in world.crime_reports().iter().enumerate() {
        let Some(w) = r.witness.filter(|_| !r.resolved) else { continue };
        if crate::systems::law::living(world, w) {
            continue;
        }
        let died = world.comp::<crate::components::Corpse>(w).map_or(0, |c| c.died_tick);
        if now.saturating_sub(died) >= wait {
            cold.push((i, r.suspect, w, r.crime));
        }
    }
    if cold.is_empty() {
        return;
    }
    let guards = crate::systems::law_brain::guards(world);
    let seen = |s: EntityId, crime: Crime| {
        guards.iter().any(|&g| {
            world.comp::<crate::components::Memory>(g).is_some_and(|m| {
                m.entries
                    .iter()
                    .any(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(s) && e.crime == Some(crime))
            })
        })
    };
    cold.retain(|&(_, s, _, crime)| !seen(s, crime));
    for &(i, s, w, crime) in &cold {
        if let Some(r) = world.reports_mut().get_mut(i) {
            r.resolved = true;
        }
        world.stats.current.hit.hm_cold += 1;
        let who = if world.has::<crate::components::Identity>(w) { world.name_of(w) } else { "the witness".into() };
        let text = format!("the {} case on {} went cold: {who} is dead", crime.label(), world.name_of(s));
        world.push_event(EventKind::CaseCold, &[s, w], text);
    }
}

/// The phase 2 agent passes, from `contracts::daily` after the hire pass
/// (the wants' sweep runs before it): Silence, then Debt.
pub fn agent_passes(world: &mut World) {
    if !world.config.hitmarket.enabled {
        return;
    }
    silence_pass(world);
    debt_pass(world);
}

/// H15, H17 and the Debt clock, from `contracts::set_status` when an
/// organic record is done (`c` as it stood before): the placing agent
/// learns the deed first-hand and remembers it (`Commissioned`, relief for
/// Revenge); a Debt Beat starts the Hit's clock; a Silence or Protect Beat
/// on a report's witness may make it recant (`recant_p × (1 − courage)`,
/// never a guard).
pub fn on_fulfilled(world: &mut World, c: &Contract) {
    if !world.config.hitmarket.enabled || c.motive.is_none() {
        return;
    }
    let target = c.target.id();
    let now = world.tick;
    // H17: the buyer after.
    if let Some(a) = c.agent.filter(|&a| world.has::<Memory>(a) && crate::systems::law::living(world, a)) {
        let deed = if c.kind == ContractKind::Hit { Deed::Killed } else { Deed::Assaulted };
        let sal = world.config.gossip.deed_sal.get(deed);
        let e = MemoryEntry {
            subject: c.taker,
            salience: sal,
            valence: 0.0,
            deed: Some(deed),
            object: Some(target),
            hops: 0,
            conf: 1.0,
            ..MemoryEntry::blank(MemoryKind::Rumour, now)
        };
        crate::systems::memory::hear_entry(world, a, e);
        let val = if c.motive == Motive::Revenge { world.config.hitmarket.closure_val } else { 0.0 };
        world.remember(a, MemoryKind::Commissioned, Some(target), sal.max(0.5), val, false);
    }
    if c.kind != ContractKind::Beat {
        return;
    }
    let Some(buyer) = c.buyer else { return };
    if c.motive == Motive::Debt {
        if let Some(k) = world.hm_debts.get_mut(&(buyer, target)) {
            k.beat = Some(now);
        }
        return;
    }
    if !matches!(c.motive, Motive::Silence | Motive::Protect)
        || !crate::systems::law::living(world, target)
        || crate::systems::law::is_guard(world, target)
    {
        return;
    }
    let courage = world.comp::<Personality>(target).map_or(0.5, |p| p.courage);
    let p = world.config.hitmarket.recant_p * (1.0 - courage.clamp(0.0, 1.0));
    let ours =
        |s: EntityId| s == buyer || (world.has::<crate::components::Gang>(buyer) && world.gang_of(s) == Some(buyer));
    let reports: Vec<(usize, EntityId, Crime)> = world
        .crime_reports()
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.resolved && r.witness == Some(target) && ours(r.suspect))
        .map(|(i, r)| (i, r.suspect, r.crime))
        .collect();
    let day = world.day();
    for (i, s, crime) in reports {
        let key = (c.id << 16) ^ i as u64;
        let roll: f32 = world.rng.contract(ContractNs::Recant, day, key).random();
        if roll >= p {
            continue;
        }
        if let Some(r) = world.reports_mut().get_mut(i) {
            r.resolved = true;
        }
        world.stats.current.hit.hm_recanted += 1;
        let text = format!("{} took back the {} report on {}", world.name_of(target), crime.label(), world.name_of(s));
        world.push_event(EventKind::Recanted, &[target, s], text);
    }
}
