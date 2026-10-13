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

use crate::components::{Corp, Gang, Law, OUTSIDE_PARENT_BASE};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{OutsideFaction, OutsideId, OutsideKind, OutsideOrder, Parent, Persona, WORLD_ACCOUNT};
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::utility::curves::{can, Curve};
use crate::utility::{compensate, Consideration};
use crate::world::World;

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
}

/// § 1 step 2: `base × market × (0.8 + 0.4 rep) × (1 − exposure × loss)`,
/// times `1 − embargo_cut` under the city's Embargo, rounded.
pub fn parent_income(
    base: i64,
    market: f32,
    rep: f32,
    exposure: f32,
    loss: f32,
    embargo_cut: f32,
    embargoed: bool,
) -> i64 {
    let mut x = base as f64
        * f64::from(market)
        * f64::from(0.8 + 0.4 * rep.clamp(0.0, 1.0))
        * f64::from(1.0 - exposure.clamp(0.0, 1.0) * loss.clamp(0.0, 1.0));
    if embargoed {
        x *= f64::from(1.0 - embargo_cut.clamp(0.0, 1.0));
    }
    x.round() as i64
}

/// § 1 step 2 (phase 2: buildings and remittances; the district term
/// arrives with economic warfare, phase 4): `0.6 × (1 − value ÷ peak) + 0.4
/// × remit_gap`, `remit_gap = 1 − remitted_7d ÷ (remitted_30d ÷ days × 7)`
/// once a week of books exists and anything was sent.
pub fn branch_loss(value: i64, peak: i64, remitted_7d: i64, remitted_30d: i64, days: usize) -> f32 {
    let held = if peak > 0 { 1.0 - (value.max(0) as f32 / peak as f32).min(1.0) } else { 0.0 };
    let gap = if days >= 7 && remitted_30d > 0 {
        let weekly = remitted_30d as f32 / days as f32 * 7.0;
        (1.0 - remitted_7d.max(0) as f32 / weekly).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (0.6 * held + 0.4 * gap).clamp(0.0, 1.0)
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
    for id in ids {
        prune(world, id);
        remit(world, id);
        decide(world, id);
        book_day(world, id);
    }
}

/// Branches that died or left drop off the list.
fn prune(world: &mut World, id: OutsideId) {
    let Some(p) = parent(world, id) else { return };
    let keep: Vec<EntityId> = p
        .branches
        .iter()
        .copied()
        .filter(|&b| {
            world.is_alive(b)
                && (world.comp::<Corp>(b).is_some_and(|c| c.parent == Some(id))
                    || world.has::<Gang>(b)
                    || world.has::<Law>(b))
        })
        .collect();
    if let Some(p) = parent_mut(world, id) {
        p.branches = keep;
    }
}

/// § 1 step 4: the branches send coins home. A corp branch whose closing
/// is at least `remit_floor × treasury_ref` sends `remit_share` of
/// yesterday's positive cashflow; a syndicate's gang, on the week's first
/// midnight, `tribute_share` of its treasury above `tribute_floor`. The
/// Treasury keeps the tariff's share (`Flow::Tax`); the rest crosses out
/// (`Flow::Remit`). Both are capital: neither is a trading loss. Nothing
/// under the city's Embargo.
pub fn remit(world: &mut World, id: OutsideId) {
    let Some(p) = parent(world, id) else { return };
    if p.embargoed {
        return;
    }
    let branches = p.branches.clone();
    let cfg = world.config.outside.parents.clone();
    let tariff = world.outside.tariff.clamp(0.0, 1.0);
    let weekly = world.day().is_multiple_of(7);
    let (mut sent, mut kept) = (0i64, 0i64);
    for b in branches {
        let amount = if let Some(c) = world.comp::<Corp>(b) {
            let cf = c.cashflow.back().copied().unwrap_or(0);
            if cf <= 0 || (c.closing as f64) < f64::from(cfg.remit_floor) * c.treasury_ref as f64 {
                continue;
            }
            ((f64::from(cfg.remit_share) * cf as f64).round() as i64).min(c.treasury.max(0))
        } else if let Some(g) = world.comp::<Gang>(b) {
            if !weekly {
                continue;
            }
            (f64::from(cfg.tribute_share) * (g.treasury - cfg.tribute_floor).max(0) as f64).round() as i64
        } else {
            continue;
        };
        if amount <= 0 {
            continue;
        }
        let cut = (f64::from(tariff) * amount as f64).round() as i64;
        let taxed = if cut > 0 { ownership::pay(world, Some(b), None, cut, Flow::Tax) } else { 0 };
        let moved = ownership::cross_out(world, Some(b), id, amount - taxed, Flow::Remit, false);
        if let Some(c) = world.comp_mut::<Corp>(b) {
            // Capital: the corp's day of trading is not the poorer for it.
            c.cashflow_today += taxed + moved;
        }
        sent += moved;
        kept += taxed;
    }
    if let Some(f) = world.outside.faction_mut(id) {
        // `cross_out` books an account's inflows as `income_today` (the
        // World's reading); a parent's income is its own (the ledger).
        f.income_today -= sent;
    }
    if let Some(p) = parent_mut(world, id) {
        p.ledger.remitted_today += sent;
        p.ledger.tariff_today += kept;
    }
}

/// A branch's `cash`: a corp's closing ÷ its `treasury_ref`, a gang's
/// treasury ÷ `gang_cash_ref`; the Law none.
fn cash_of(world: &World, b: EntityId) -> Option<(f32, i64, i64)> {
    if let Some(c) = world.comp::<Corp>(b) {
        let r = c.treasury_ref.max(1);
        return Some((c.closing as f32 / r as f32, c.closing, r));
    }
    if let Some(g) = world.comp::<Gang>(b) {
        let r = world.config.outside.parents.gang_cash_ref.max(1);
        return Some((g.treasury as f32 / r as f32, g.treasury, r));
    }
    None
}

/// § 2: the considerations table (phase 2: Hold and Subsidise), rescored
/// each midnight with hysteresis and a minimum hold, then the order's
/// action. The State holds (its orders are phase 3's).
pub fn decide(world: &mut World, id: OutsideId) {
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    if f.kind == OutsideKind::State {
        return;
    }
    let cfg = world.config.outside.parents.clone();
    let weakest: Option<(EntityId, f32, i64, i64)> = p
        .branches
        .iter()
        .filter_map(|&b| cash_of(world, b).map(|(cash, closing, r)| (b, cash, closing, r)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let wealth = sat(f.treasury as f32 / f.treasury_ref.max(1) as f32);
    let att = (p.attitude + 1.0) / 2.0;
    let current = p.order;
    let h = |o: OutsideOrder| if o == current { cfg.hysteresis } else { 0.0 };
    let mut trace: Vec<(OutsideOrder, f32)> = vec![(OutsideOrder::Hold, cfg.flat_hold + h(OutsideOrder::Hold))];
    let cash = weakest.map_or(1.0, |w| w.1);
    let subsidise = [
        Consideration::raw(
            "a weak branch",
            cash,
            can(!p.embargoed && f.treasury > 0 && weakest.is_some() && cash < cfg.subsidise_below),
        ),
        Consideration::new("1 - cash", 1.0 - cash, Curve::Linear { m: 0.8, b: 0.2 }),
        Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("attitude", att, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("exposure", p.exposure, Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    if subsidise.iter().all(|c| c.output > 0.0) {
        let raw: f32 = subsidise.iter().map(|c| c.output).product();
        trace.push((
            OutsideOrder::Subsidise,
            compensate(raw, subsidise.len()) + cfg.flat_subsidise + h(OutsideOrder::Subsidise),
        ));
    }
    trace.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let best = trace[0].0;
    let held = world.tick.saturating_sub(p.order_since) >= u64::from(cfg.min_hold_days) * TICKS_PER_DAY;
    // An order whose gate closed (no weak branch left) ends at once.
    let still = trace.iter().any(|t| t.0 == current);
    let next = if best != current && (held || !still) { best } else { current };
    let name = f.name.clone();
    let now = world.tick;
    if let Some(p) = parent_mut(world, id) {
        p.order_trace = trace;
        if next != current {
            p.order = next;
            p.order_since = now;
        }
        p.order_target = if next == OutsideOrder::Subsidise { weakest.map(|w| w.0) } else { None };
    }
    if next != current {
        world.push_event(EventKind::OutsideOrder, &[], format!("{name}: {} -> {}", current.label(), next.label()));
    }
    if next == OutsideOrder::Subsidise {
        if let Some((b, _, closing, r)) = weakest {
            subsidise_branch(world, id, b, closing, r);
        }
    }
}

/// Subsidise's daily action: `min(subsidy_cap_day, subsidise_to × ref −
/// closing, max_spend_frac × treasury)` into the weakest branch
/// (`Flow::ParentFund`, capital: a corp's cashflow does not count it).
fn subsidise_branch(world: &mut World, id: OutsideId, b: EntityId, closing: i64, r: i64) {
    let cfg = world.config.outside.parents.clone();
    let Some(f) = world.outside.faction(id) else { return };
    let to = (f64::from(cfg.subsidise_to) * r as f64).round() as i64 - closing;
    let spend = (f64::from(cfg.max_spend_frac) * f.treasury.max(0) as f64).round() as i64;
    let amount = cfg.subsidy_cap_day.min(to).min(spend);
    if amount <= 0 {
        return;
    }
    let moved = ownership::cross_in(world, id, Some(b), amount, Flow::ParentFund);
    if moved <= 0 {
        return;
    }
    if let Some(f) = world.outside.faction_mut(id) {
        f.income_today += moved;
    }
    if let Some(c) = world.comp_mut::<Corp>(b) {
        c.cashflow_today -= moved;
    }
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    let bname = world.owner_label(Some(b));
    if let Some(p) = parent_mut(world, id) {
        p.ledger.spent_today += moved;
        p.ledger.subsidies_today += 1;
    }
    world.push_event(EventKind::Subsidised, &[b], format!("{pname} sent {moved} to {bname}"));
}

/// A branch's value: a corp's buildings at `ownership::value`, a gang's
/// members at `member_value`; the Law none.
fn branch_value(world: &World, b: EntityId) -> i64 {
    if let Some(c) = world.comp::<Corp>(b) {
        return c
            .buildings
            .iter()
            .filter_map(|&x| world.comp::<crate::components::Building>(x))
            .map(|bd| ownership::value(world, bd.kind))
            .sum();
    }
    if let Some(g) = world.comp::<Gang>(b) {
        return g.members.len() as i64 * world.config.outside.parents.member_value;
    }
    0
}

/// § 1 steps 1-3 and 5-8: today's counters into the rings; the market walk;
/// `branch_loss`; reputation; income abroad less upkeep (minted); health;
/// attitude; the failure flags.
fn book_day(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let (seed, day, now) = (world.seed(), world.day(), world.tick);
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    let value: i64 = p.branches.iter().map(|&b| branch_value(world, b)).sum();
    // Reputation's target from what the city knows of the branches (M15).
    let reps: Vec<crate::word::Reputation> =
        p.branches.iter().map(|&b| crate::systems::reputation::rep(world, b)).collect();
    let target = if reps.is_empty() {
        p.rep
    } else if f.kind == OutsideKind::State {
        reps.iter().map(|r| r.honour).sum::<f32>() / reps.len() as f32
    } else {
        let n = reps.len() as f32;
        0.5 * reps.iter().map(|r| r.honour).sum::<f32>() / n
            + 0.3 * reps.iter().map(|r| r.standing).sum::<f32>() / n
            + 0.2 * (1.0 - reps.iter().map(|r| r.heat).sum::<f32>() / n)
    };
    let z = crate::systems::econ::hash_normal(seed, PURPOSE_PARENT_MARKET, u64::from(id), day);
    let name = f.name.clone();
    let kind = f.kind;
    let Some(f) = world.outside.faction_mut(id) else { return };
    f.market = (1.0 + (f.market - 1.0) * (1.0 - cfg.market_revert) + cfg.market_sd * z).clamp(0.7, 1.3);
    let (market, base_income, upkeep) = (f.market, f.base_income, f.base_upkeep.max(1));
    let Some(p) = f.parent.as_mut() else { return };
    let l = &mut p.ledger;
    push(&mut l.remitted, std::mem::take(&mut l.remitted_today));
    push(&mut l.spent, std::mem::take(&mut l.spent_today));
    l.tariff_last = std::mem::take(&mut l.tariff_today);
    l.subsidies_last = std::mem::take(&mut l.subsidies_today);
    push(&mut l.branch_value, value);
    let peak = l.branch_value.iter().copied().max().unwrap_or(0);
    let loss = if kind == OutsideKind::State {
        0.0
    } else {
        branch_loss(value, peak, sum_last(&l.remitted, 7), sum_last(&l.remitted, RING_DAYS), l.remitted.len())
    };
    p.branch_loss = loss;
    p.rep = (p.rep + cfg.rep_rate * (target - p.rep)).clamp(0.0, 1.0);
    let income = parent_income(base_income, market, p.rep, p.exposure, loss, cfg.embargo_cut, p.embargoed);
    push(&mut p.ledger.income, income);
    let (inc7, n7) = (sum_last(&p.ledger.income, 7), p.ledger.income.len().min(7) as i64);
    let (rem7, spent7) = (sum_last(&p.ledger.remitted, 7), sum_last(&p.ledger.spent, 7));
    let delta = income - upkeep;
    f.treasury += delta;
    f.income_today = income;
    let (treasury, tref) = (f.treasury, f.treasury_ref.max(1));
    let Some(p) = f.parent.as_mut() else { return };
    let week_upkeep = 7 * upkeep;
    let inc_week = if n7 > 0 { inc7 * 7 / n7 } else { income * 7 };
    let margin = (inc_week - week_upkeep) as f32 / week_upkeep as f32;
    p.health = ((0.5 + 2.0 * margin).clamp(0.0, 1.0) * (treasury as f32 / (0.25 * tref as f32)).clamp(0.0, 1.0))
        .clamp(0.0, 1.0);
    let remit_term = if p.exposure > 0.0 && base_income > 0 {
        cfg.att_remit * sat(rem7 as f32 / (7.0 * base_income as f32 * p.exposure))
    } else {
        0.0
    };
    let cost_term = if base_income > 0 { cfg.att_cost * sat(spent7 as f32 / (7.0 * base_income as f32)) } else { 0.0 };
    p.attitude = (p.attitude * (1.0 - cfg.att_decay) + remit_term - cost_term - cfg.att_loss * loss).clamp(-1.0, 1.0);
    let failing = n7 >= 7 && inc7 < week_upkeep;
    let newly_failing = failing && p.failing_since.is_none();
    p.failing_since = if failing { p.failing_since.or(Some(now)) } else { None };
    p.insolvent_since = if treasury < 0 { p.insolvent_since.or(Some(now)) } else { None };
    world.outside.minted += delta;
    if newly_failing {
        world.push_event(
            EventKind::ParentFailing,
            &[],
            format!("{name}: income {inc7} under its upkeep {week_upkeep} for a week"),
        );
    }
}

/// `Embargo`: the city's sanction on a parent while set.
pub fn set_embargo(world: &mut World, id: OutsideId, on: bool) -> Result<String, String> {
    let name = world.outside.faction(id).map(|f| f.name.clone());
    let Some(p) = parent_mut(world, id) else { return Err(format!("Embargo: no parent {id}")) };
    p.embargoed = on;
    let name = name.unwrap_or_default();
    Ok(if on { format!("Embargo on {name}") } else { format!("Embargo on {name} lifted") })
}

/// The day's parents group (`systems::stats::snapshot`, 23:59): the last
/// midnight's flows, today's crossings (every account), each parent's
/// state. `None` with the parents off.
pub fn close_day(world: &mut World) -> Option<crate::stats::OutsideCols> {
    if !parents_on(world) {
        return None;
    }
    let o = &world.outside;
    let mut cols = crate::stats::OutsideCols {
        crossed_in: o.inbound - o.inbound_mark,
        crossed_out: o.outbound - o.outbound_mark,
        ..Default::default()
    };
    for f in o.parents() {
        let Some(p) = f.parent.as_ref() else { continue };
        let l = &p.ledger;
        cols.remit += l.remitted.back().copied().unwrap_or(0);
        cols.parent_fund += l.spent.back().copied().unwrap_or(0);
        cols.failing += u32::from(p.failing_since.is_some());
        cols.accounts.push(crate::stats::ParentCols {
            treasury: f.treasury,
            income: l.income.back().copied().unwrap_or(0),
            health: p.health,
            attitude: p.attitude,
            rep: p.rep,
            branch_loss: p.branch_loss,
            order: p.order.label().to_string(),
        });
    }
    let (tariff, subsidies) = world
        .outside
        .factions
        .iter()
        .filter_map(|f| f.parent.as_ref())
        .fold((0, 0), |a, p| (a.0 + p.ledger.tariff_last, a.1 + p.ledger.subsidies_last));
    cols.tariff = tariff;
    cols.subsidies = subsidies;
    world.outside.inbound_mark = world.outside.inbound;
    world.outside.outbound_mark = world.outside.outbound;
    Some(cols)
}
