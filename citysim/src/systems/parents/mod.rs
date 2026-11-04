//! M17 phase 2 (docs/M17_OUTSIDE.md, "Revised 2026-10-12" and § 1-2): the
//! ledger and the parents. Outside accounts nobody in the city can walk to:
//! a megacorp brand's parent (every copy of the brand is a branch), a
//! syndicate behind each seeded gang, and the State behind the Law. Each
//! midnight a parent's branches send coins home (a corp's share of a
//! profitable day, a gang's weekly tribute; the city's tariff keeps a cut),
//! the parent books its income abroad and its upkeep (the outside's own
//! creation of money, into `Outside.minted`), its health, reputation and
//! attitude move, and it decides between Hold and Subsidise (the M11 brain
//! template: considerations, hysteresis, a minimum hold). A subsidy is a
//! capital move into a branch's treasury: never revenue (`wage_rev` does not
//! read it) and never a wallet. Every coin crosses through
//! `ownership::{cross_in, cross_out}`, so `econ::identity` holds.
//!
//! O(accounts + branches) a day; nothing per agent; no stream draw (the
//! market walk is a pure hash). Nothing at all with `[outside]` off.

use std::collections::BTreeMap;

use crate::components::{Building, Corp, Gang, Law, Personality, RelKind, Skills, OUTSIDE_PARENT_BASE};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{
    BrothersDue, LostBuilding, OutsideFaction, OutsideId, OutsideKind, OutsideOrder, Outsider, Parent, Persona,
    WORLD_ACCOUNT,
};
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::utility::curves::{can, Curve};
use crate::utility::{compensate, Consideration};
use crate::world::World;

mod books;
mod campaign;
mod founding;
mod outsiders;
mod scrip;
mod state;
mod syndicates;

pub use books::*;
pub use campaign::*;
pub use founding::*;
pub use outsiders::*;
pub use scrip::*;
pub use state::*;
pub use syndicates::*;

/// `econ::hash_normal` purpose of a parent's market walk.
const PURPOSE_PARENT_MARKET: u64 = 0x0E17_0002;
/// Days of a parent's rings.
const RING_DAYS: usize = crate::econ::BOOK_DAYS;

/// The parents run: `[outside]` on and the accounts seeded.
pub fn parents_on(world: &World) -> bool {
    crate::systems::outside::on(world) && world.outside.parents().next().is_some()
}

/// The saturating unit `x ÷ (1 + x)` (M11).
fn sat(x: f32) -> f32 {
    let x = x.max(0.0);
    x / (1.0 + x)
}

/// `World::new` (last) and the load of a pre-M17 save: the accounts from
/// `[outside.parents] factions`, each treasury minted (the identity's base
/// moves with it), `Corp.parent` re-pointed at the brand's account. No RNG.
/// Nothing with `[outside]` off or once seeded.
pub fn seed_parents(world: &mut World) {
    if !crate::systems::outside::on(world) || world.outside.parents().next().is_some() {
        return;
    }
    let treasury_ref = world.config.world_market.treasury_ref;
    crate::systems::outside::ensure_world_account(world, treasury_ref);
    let cfg = world.config.outside.parents.clone();
    let syndicates: Vec<usize> =
        cfg.factions.iter().enumerate().filter(|(_, r)| r.kind == OutsideKind::Syndicate).map(|(i, _)| i).collect();
    // The Hideout gangs (a creed's gang, The Unplugged, has no syndicate).
    let mut gangs: Vec<EntityId> = world
        .gang_list()
        .iter()
        .copied()
        .filter(|&g| world.comp::<Gang>(g).is_some_and(|x| x.creed.is_none()))
        .collect();
    gangs.sort_unstable();
    let corps = world.corps();
    let per_capita = world.config.per_capita();
    for (ri, row) in cfg.factions.iter().enumerate() {
        let branches: Vec<EntityId> = match row.kind {
            OutsideKind::Megacorp => {
                let mut v: Vec<EntityId> = corps
                    .iter()
                    .copied()
                    .filter(|&c| {
                        world
                            .comp::<Corp>(c)
                            .and_then(|x| x.slot)
                            .is_some_and(|s| world.config.corps.template_of(usize::from(s)) == row.branch)
                    })
                    .collect();
                v.sort_unstable();
                v
            }
            // A seeded Hideout gang `i` (ascending id) is the branch of
            // syndicate row `i mod n`: the shipped city's two gangs are its
            // two rows; at 20k the generated gangs are dealt over the rows.
            OutsideKind::Syndicate => match syndicates.iter().position(|&r| r == ri) {
                Some(k) => {
                    gangs.iter().enumerate().filter(|(i, _)| i % syndicates.len() == k).map(|(_, &g)| g).collect()
                }
                None => Vec::new(),
            },
            OutsideKind::State => world
                .building_of_kind(crate::components::BuildingKind::Jail)
                .filter(|&j| world.has::<Law>(j))
                .into_iter()
                .collect(),
            OutsideKind::World => continue,
        };
        let k = match row.kind {
            OutsideKind::State => per_capita.round().max(1.0) as i64,
            _ => branches.len().max(1) as i64,
        };
        let id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
        world.outside.next_id = id + 1;
        let treasury = row.treasury * k;
        world.outside.minted += treasury;
        for &c in &branches {
            if let Some(cc) = world.comp_mut::<Corp>(c) {
                cc.parent = Some(id);
                cc.outside_treasury = 0;
            }
        }
        world.outside.factions.push(OutsideFaction {
            id,
            name: row.name.clone(),
            kind: row.kind,
            treasury,
            treasury_ref: treasury.max(1),
            base_income: row.income * k,
            base_upkeep: row.upkeep * k,
            income_today: 0,
            market: 1.0,
            dead: false,
            books: Default::default(),
            ambient: None,
            parent: Some(Parent {
                persona: Persona { greed: row.persona[0], pride: row.persona[1], lawfulness: row.persona[2] },
                exposure: row.exposure,
                rep: 0.5,
                attitude: 0.0,
                health: 1.0,
                branches,
                branch_name: row.branch.clone(),
                edge: 1.0,
                data: crate::virt::DataStore { units: row.data },
                tech: row.tech,
                ..Default::default()
            }),
        });
    }
    // The megacorp rows no account claimed keep no stale M11 hook.
    for c in corps {
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            if cc.parent.is_some_and(|p| (OUTSIDE_PARENT_BASE..WORLD_ACCOUNT).contains(&p)) {
                cc.parent = None;
            }
        }
    }
    world.outside.tariff = cfg.tariff.clamp(0.0, 1.0);
    world.outside.csv_parents = u8::try_from(world.outside.parents().count()).unwrap_or(u8::MAX);
    // Phase 4: the uplinks join the plane now (appended: no node id moves).
    crate::systems::virt::relink(world);
}

/// § 1 step 2: `base × market × edge × (0.8 + 0.4 rep) × (1 − exposure ×
/// loss) × (1 − cut)`, rounded; `cut` the sanctions' (and the city's
/// Embargo's) combined cut.
pub fn parent_income(base: i64, market: f32, edge: f32, rep: f32, exposure: f32, loss: f32, cut: f32) -> i64 {
    let x = base as f64
        * f64::from(market)
        * f64::from(edge.max(0.0))
        * f64::from(0.8 + 0.4 * rep.clamp(0.0, 1.0))
        * f64::from(1.0 - exposure.clamp(0.0, 1.0) * loss.clamp(0.0, 1.0))
        * f64::from(1.0 - cut.clamp(0.0, 1.0));
    x.round() as i64
}

/// § 1 step 2: `0.6 × (1 − value ÷ peak) + 0.4 × gap`, the gap the share
/// of the branches a week in the red ([`remit_gap`]; phase 3: sporadic
/// remittances no longer read as loss). The district term arrives with
/// economic warfare (phase 4).
pub fn branch_loss(value: i64, peak: i64, gap: f32) -> f32 {
    let held = if peak > 0 { 1.0 - (value.max(0) as f32 / peak as f32).min(1.0) } else { 0.0 };
    (0.6 * held + 0.4 * gap.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

fn parent(world: &World, id: OutsideId) -> Option<&Parent> {
    world.outside.faction(id).and_then(|f| f.parent.as_ref())
}

fn parent_mut(world: &mut World, id: OutsideId) -> Option<&mut Parent> {
    world.outside.faction_mut(id).and_then(|f| f.parent.as_mut())
}

fn push<T>(ring: &mut std::collections::VecDeque<T>, v: T) {
    if ring.len() >= RING_DAYS {
        ring.pop_front();
    }
    ring.push_back(v);
}

fn sum_last(ring: &std::collections::VecDeque<i64>, n: usize) -> i64 {
    ring.iter().rev().take(n).sum()
}

/// The parents' midnight (`outside::run`, after `living::run`): each
/// account's branches pruned, the remittances home, the order and its
/// action, then the books.
pub fn daily(world: &mut World) {
    if !parents_on(world) {
        return;
    }
    let ids: Vec<OutsideId> = world.outside.parents().filter(|f| !f.dead).map(|f| f.id).collect();
    for &id in &ids {
        prune(world, id);
        note_losses(world, id);
        remit(world, id);
        match world.outside.faction(id).map(|f| f.kind) {
            Some(OutsideKind::State) => state_daily(world, id),
            Some(OutsideKind::Syndicate) => {
                syndicate_daily(world, id);
                decide(world, id);
            }
            _ => decide(world, id),
        }
        if parent(world, id).is_some_and(|p| p.reinforce_left > 0) {
            reinforce_step(world, id);
        }
        if world.outside.faction(id).is_some_and(|f| f.kind == OutsideKind::Megacorp && !f.dead) {
            ship_data(world, id);
        }
        if world.outside.faction(id).is_some_and(|f| !f.dead) {
            book_day(world, id);
        }
    }
    tours(world);
    found_parents(world);
    // Phase 4: expired sanctions go; the dead's scrip holdings go; conversions.
    let now = world.tick;
    world.outside.sanctions.retain(|s| s.until > now);
    let gone: Vec<EntityId> = world.outside.scrip.keys().copied().filter(|&a| !world.is_alive(a)).collect();
    for a in gone {
        let held = world.outside.scrip.remove(&a).unwrap_or_default();
        for (i, n) in held {
            if let Some(p) = parent_mut(world, i) {
                p.scrip_out = (p.scrip_out - n).max(0);
            }
        }
    }
    convert_pass(world, false);
}
