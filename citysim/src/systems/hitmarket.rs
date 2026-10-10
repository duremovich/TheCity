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
