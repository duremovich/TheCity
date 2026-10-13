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

use crate::components::{Building, Corp, Gang, Law, Personality, RelKind, Sentence, Skills, OUTSIDE_PARENT_BASE};
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
    world.outside.csv_parents = u8::try_from(world.outside.parents().count()).unwrap_or(u8::MAX);
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
        book_day(world, id);
    }
    tours(world);
    found_parents(world);
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
    // A corp branch that died lost its component: a branch neither a gang
    // nor the Law was a corp.
    let had_corp = p.branches.iter().any(|&b| !world.has::<Gang>(b) && !world.has::<Law>(b));
    let has_corp = keep.iter().any(|&b| world.has::<Corp>(b));
    let now = world.tick;
    if let Some(p) = parent_mut(world, id) {
        p.branches = keep;
        if had_corp && !has_corp {
            p.branch_died_at = Some(now);
        }
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
        // An emptied gang waits on its refound, not a subsidy.
        if g.emptied && g.members.is_empty() {
            return None;
        }
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
    let mut push_scored = |order: OutsideOrder, cs: &[Consideration], flat: f32| {
        if cs.iter().all(|c| c.output > 0.0) {
            let raw: f32 = cs.iter().map(|c| c.output).product();
            trace.push((order, compensate(raw, cs.len()) + flat + h(order)));
        }
    };
    push_scored(OutsideOrder::Subsidise, &subsidise, cfg.flat_subsidise);
    // Phase 3 (§ 2's table): Reinforce, Retake, FoundBranch (megacorps),
    // Abandon (megacorps and syndicates).
    let now = world.tick;
    let persona = p.persona;
    let loss = p.branch_loss;
    let megacorp = f.kind == OutsideKind::Megacorp;
    let corp_branch = p.branches.iter().any(|&b| world.has::<Corp>(b));
    if megacorp {
        let recent_t = now.saturating_sub(u64::from(cfg.recent_days) * TICKS_PER_DAY);
        let recent = p.lost.iter().any(|l| l.at >= recent_t)
            || p.branches
                .iter()
                .any(|&b| world.comp::<Corp>(b).and_then(|c| c.raided_at).is_some_and(|t| t >= recent_t));
        let cooled = p
            .last_reinforce
            .is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.reinforce_cooldown_days) * TICKS_PER_DAY);
        let room = living_outsiders(world, id) < cfg.outsiders_cap as usize;
        let reinforce = [
            Consideration::raw(
                "a recent shock",
                0.0,
                can(recent
                    && cooled
                    && room
                    && corp_branch
                    && !p.embargoed
                    && f.treasury >= (cfg.body_cost + cfg.body_fund) * i64::from(cfg.bodies_per_call)),
            ),
            Consideration::new("loss", loss, Curve::Logistic { k: 8.0, mid: 0.3 }),
            Consideration::new("pride", persona.pride, Curve::Linear { m: 0.6, b: 0.4 }),
            Consideration::new("attitude", att, Curve::Linear { m: 0.5, b: 0.5 }),
            Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ];
        push_scored(OutsideOrder::Reinforce, &reinforce, cfg.flat_reinforce);
        let window_t = now.saturating_sub(u64::from(cfg.retake_window_days) * TICKS_PER_DAY);
        let target = p.lost.iter().any(|l| l.at >= window_t && retake_holder(world, id, l.building).is_some());
        let retake = [
            Consideration::raw(
                "a lost building",
                0.0,
                can(target && corp_branch && !p.embargoed && wealth >= cfg.retake_wealth_min),
            ),
            Consideration::new("loss", loss, Curve::Linear { m: 0.7, b: 0.3 }),
            Consideration::new("pride", persona.pride, Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }),
            Consideration::new("exposure", p.exposure, Curve::Linear { m: 0.5, b: 0.5 }),
            Consideration::new("attitude", att, Curve::Linear { m: 0.6, b: 0.4 }),
        ];
        push_scored(OutsideOrder::Retake, &retake, cfg.flat_retake);
        let abandoned_ok = p.abandoned_until.is_none_or(|t| t <= now);
        let since = p.branch_died_at.map_or(0.0, |t| now.saturating_sub(t) as f32 / (14.0 * TICKS_PER_DAY as f32));
        let found = [
            Consideration::raw(
                "no branch",
                0.0,
                can(!corp_branch && abandoned_ok && !p.embargoed && f.treasury >= 3 * cfg.branch_seed),
            ),
            Consideration::new("exposure", p.exposure, Curve::Linear { m: 0.7, b: 0.3 }),
            Consideration::new("pride", persona.pride, Curve::Linear { m: 0.6, b: 0.4 }),
            Consideration::new("days without", since, Curve::Logistic { k: 8.0, mid: 0.5 }),
            Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ];
        push_scored(OutsideOrder::FoundBranch, &found, cfg.flat_found_branch);
    }
    let spent30 = sum_last(&p.ledger.spent, RING_DAYS);
    let burn = sat(spent30 as f32 / (30.0 * f.base_income.max(1) as f32));
    let abandon_cs = [
        Consideration::raw("a tired parent", 0.0, can(!p.branches.is_empty() && p.attitude < cfg.abandon_attitude)),
        Consideration::new("spent", burn, Curve::Logistic { k: 8.0, mid: 0.5 }),
        Consideration::new("greed", persona.greed, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("1 - exposure", 1.0 - p.exposure, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("1 - attitude", 1.0 - att, Curve::Linear { m: 0.7, b: 0.3 }),
    ];
    push_scored(OutsideOrder::Abandon, &abandon_cs, cfg.flat_abandon);
    trace.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let best = trace[0].0;
    let pinned = p.pinned_until.is_some_and(|t| t > now);
    let held = !pinned && world.tick.saturating_sub(p.order_since) >= u64::from(cfg.min_hold_days) * TICKS_PER_DAY;
    // An order whose gate closed (no weak branch left) ends at once.
    let still = pinned || trace.iter().any(|t| t.0 == current);
    let next = if best != current && (held || !still) { best } else { current };
    let name = f.name.clone();
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
    match next {
        OutsideOrder::Subsidise => {
            if let Some((b, _, closing, r)) = weakest {
                subsidise_branch(world, id, b, closing, r);
            }
        }
        OutsideOrder::Reinforce if next != current => {
            start_reinforce(world, id);
        }
        OutsideOrder::Retake => retake_step(world, id),
        OutsideOrder::FoundBranch => {
            found_branch(world, id);
        }
        OutsideOrder::Abandon if next != current => abandon(world, id),
        _ => {}
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
    let gap = remit_gap(world, id);
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    // Phase 3: an emptied gang waiting on its refound is no lost value (the
    // refound reads the attitude).
    let live: Vec<EntityId> = p
        .branches
        .iter()
        .copied()
        .filter(|&b| world.comp::<Gang>(b).is_none_or(|g| !(g.emptied && g.members.is_empty())))
        .collect();
    let value: i64 = live.iter().map(|&b| branch_value(world, b)).sum();
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
    let all_waiting = live.is_empty() && kind == OutsideKind::Syndicate;
    if !all_waiting {
        push(&mut l.branch_value, value);
    }
    let peak = l.branch_value.iter().copied().max().unwrap_or(0);
    let loss = if kind == OutsideKind::State || all_waiting { 0.0 } else { branch_loss(value, peak, gap) };
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
    let t = o.today.clone();
    cols.outsiders_living = o.outsiders.keys().filter(|&&a| world.is_alive(a)).count() as u32;
    cols.outsiders_arrived = t.arrived;
    cols.outsiders_left = t.left;
    cols.settled = t.settled;
    cols.reinforcements = t.reinforcements;
    cols.buybacks = t.buybacks;
    cols.retakes_posted = t.retakes_posted;
    cols.retakes_won = t.retakes_won;
    cols.branches_founded = t.branches_founded;
    cols.parents_founded = t.parents_founded;
    cols.abandoned = t.abandoned;
    cols.brothers_arrived = t.brothers;
    cols.gangs_refounded = t.refounded;
    cols.state_guards = u32::from(o.state_guards);
    cols.state_grants = t.grants;
    cols.mandates = t.mandates;
    cols.withdrawals = t.withdrawals;
    // The CSV's account slots are the seeded parents (the header is fixed;
    // a parent founded later is in the panel, not the CSV).
    let slots = usize::from(o.csv_parents);
    for f in o.parents().take(slots) {
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
    world.outside.today = Default::default();
    world.outside.inbound_mark = world.outside.inbound;
    world.outside.outbound_mark = world.outside.outbound;
    Some(cols)
}

// ---------------------------------------------------------------------------
// Phase 3: bodies (docs/M17_OUTSIDE.md § 2, § 4; "Revised 2026-10-12").
//
// Agents from abroad are spawned through demography's migrant arrival (the
// World's coins and the arrival savings cross in as any migrant's), carry a
// side-table record (`Outside.outsiders`, not a component) and leave by
// demography's emigration path at the end of their tour. A Retake is a
// contract record on the M16 board, struck as one draw over the missions'
// strength estimate (a ledger record, as Tag and Vote). Every coin a parent
// moves crosses through `ownership::{cross_in, cross_out}`; a cost paid
// abroad is booked out of `minted`. No stream draw but `arrive`'s (M11's
// immigrant draws, in order) and the contract's keyed outcome.
// ---------------------------------------------------------------------------

/// `econ::hash_unit` purpose of an Outsider's lawfulness.
const PURPOSE_OUTSIDER: u64 = 0x0E17_0003;

/// Phase 3: the share of a parent's corp branches a whole week in the red
/// (Σ of the last 7 days' cashflow below 0). Sporadic remittances and a
/// bad day or two read no loss.
pub fn remit_gap(world: &World, id: OutsideId) -> f32 {
    let Some(p) = parent(world, id) else { return 0.0 };
    let corps: Vec<&Corp> = p.branches.iter().filter_map(|&b| world.comp::<Corp>(b)).collect();
    if corps.is_empty() {
        return 0.0;
    }
    let red =
        corps.iter().filter(|c| c.cashflow.len() >= 7 && c.cashflow.iter().rev().take(7).sum::<i64>() < 0).count();
    red as f32 / corps.len() as f32
}

/// A parent's living Outsiders (not its brothers).
fn living_outsiders(world: &World, id: OutsideId) -> usize {
    world.outside.outsiders.iter().filter(|(&a, o)| o.from == id && !o.brother && world.is_alive(a)).count()
}

/// A parent's corp branches, ascending.
fn corp_branches(world: &World, id: OutsideId) -> Vec<EntityId> {
    parent(world, id).map_or_else(Vec::new, |p| p.branches.iter().copied().filter(|&b| world.has::<Corp>(b)).collect())
}

/// Pay a cost abroad: out of the parent's treasury and out of `minted`
/// (the coins leave existence with the outside economy).
fn pay_abroad(world: &mut World, id: OutsideId, cost: i64) -> bool {
    let Some(f) = world.outside.faction_mut(id) else { return false };
    if cost <= 0 {
        return true;
    }
    if f.treasury < cost {
        return false;
    }
    f.treasury -= cost;
    world.outside.minted -= cost;
    true
}

/// Coins from a parent into a branch: `Flow::ParentFund`, capital (a corp's
/// day of trading does not count it). Returns the coins moved.
fn fund(world: &mut World, id: OutsideId, branch: EntityId, amount: i64) -> i64 {
    let moved = ownership::cross_in(world, id, Some(branch), amount, Flow::ParentFund);
    if moved > 0 {
        if let Some(f) = world.outside.faction_mut(id) {
            f.income_today += moved;
        }
        if let Some(c) = world.comp_mut::<Corp>(branch) {
            c.cashflow_today -= moved;
        }
    }
    moved
}

/// One agent from abroad at `door` (the edge road nearest it, a bed in the
/// Block with room nearest it), fighting at `outsider_skill`, its
/// lawfulness the parent's ± 0.2 (a keyed hash).
fn spawn_from_abroad(world: &mut World, id: OutsideId, door: crate::components::TilePos) -> EntityId {
    let home = crate::systems::demography::emptiest_home_near(world, Some(door), 1);
    let tile = crate::systems::demography::nearest_edge_road(world, door);
    let a = crate::systems::demography::spawn_outsider(world, home, tile);
    let cfg = world.config.outside.parents.clone();
    let law = parent(world, id).map_or(0.5, |p| p.persona.lawfulness);
    let u = crate::systems::econ::hash_unit(world.seed(), PURPOSE_OUTSIDER, u64::from(a.index), u64::from(id));
    if let Some(p) = world.comp_mut::<Personality>(a) {
        p.lawfulness = (law + (u - 0.5) * 0.4).clamp(0.0, 1.0);
    }
    if let Some(s) = world.comp_mut::<Skills>(a) {
        s.fighting = s.fighting.max(cfg.outsider_skill);
    }
    a
}

/// § 2 Reinforce: an episode of `bodies_per_call` bodies starts (a cohort,
/// the cooldown's clock). False inside the cooldown or with no corp branch.
pub fn start_reinforce(world: &mut World, id: OutsideId) -> bool {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return false };
    let cooled = p
        .last_reinforce
        .is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.reinforce_cooldown_days) * TICKS_PER_DAY);
    if !cooled || corp_branches(world, id).is_empty() {
        return false;
    }
    let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    if let Some(p) = parent_mut(world, id) {
        p.reinforce_left = cfg.bodies_per_call;
        p.last_reinforce = Some(now);
        p.cohort_next += 1;
    }
    world.outside.today.reinforcements += 1;
    world.push_event(EventKind::Reinforced, &[], format!("{name} sends its people to the city"));
    true
}

/// The branch building a body goes to: the corp branches' staffed kinds,
/// fewest staff for its places first (ties the lower id).
fn post_for(world: &World, id: OutsideId) -> Option<(EntityId, crate::components::Role)> {
    let mut best: Option<(u64, EntityId, crate::components::Role)> = None;
    for c in corp_branches(world, id) {
        let Some(corp) = world.comp::<Corp>(c) else { continue };
        let buildings = corp.buildings.clone();
        let staff = ownership::staff_by_building(world, &buildings);
        for b in buildings {
            let Some(kind) = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| bd.kind)
            else {
                continue;
            };
            let Some(role) = ownership::role_for(kind) else { continue };
            let places = crate::systems::jobs::places_of(world, b, role).max(1) as u64;
            let have = staff.get(&b).map_or(0, |v| v.len()) as u64;
            let ratio = have * 1000 / places;
            if best.is_none_or(|(r, x, _)| ratio < r || (ratio == r && b < x)) {
                best = Some((ratio, b, role));
            }
        }
    }
    best.map(|(_, b, r)| (b, r))
}

/// Send up to `n` bodies now (`outsiders_cap` holds): each a hire at a
/// branch post with the cohort's Acquaintance edges; `body_cost` abroad and
/// `body_fund` into the branch each. Returns the bodies sent.
fn send_bodies(world: &mut World, id: OutsideId, n: u32) -> u32 {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let cohort = parent(world, id).map_or(0, |p| p.cohort_next);
    let mut sent: Vec<EntityId> = Vec::new();
    for _ in 0..n {
        if living_outsiders(world, id) >= cfg.outsiders_cap as usize {
            break;
        }
        let Some((b, role)) = post_for(world, id) else { break };
        let Some(branch) = world.owner_of(b) else { break };
        let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) else { break };
        if world.outside.faction(id).is_none_or(|f| f.treasury < cfg.body_cost + cfg.body_fund) {
            break;
        }
        pay_abroad(world, id, cfg.body_cost);
        let a = spawn_from_abroad(world, id, door);
        crate::systems::demography::floor_role_skill(world, a, role);
        crate::systems::demography::hire(world, a, b, role);
        if let Some(v) = world.vacancies.get_mut(&b) {
            if let Some(i) = v.iter().position(|&r| r == role) {
                v.remove(i);
            }
            if v.is_empty() {
                world.vacancies.remove(&b);
            }
        }
        fund(world, id, branch, cfg.body_fund);
        let tour_until = now + u64::from(cfg.tour_days) * TICKS_PER_DAY;
        world.outside.outsiders.insert(a, Outsider { from: id, arrived: now, tour_until, cohort, brother: false });
        for &o in &sent {
            for (x, y) in [(a, o), (o, a)] {
                let e = world.edge_entry(x, y);
                e.affinity = e.affinity.max(0.4);
                e.last_interaction = now;
            }
        }
        sent.push(a);
    }
    let k = sent.len() as u32;
    if k > 0 {
        world.outside.today.arrived += k;
        let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
        world.push_event(EventKind::OutsidersArrived, &sent, format!("{k} of {name}'s people arrived in the city"));
    }
    k
}

/// A Reinforce episode's day: up to `bodies_per_day` of what is left.
pub fn reinforce_step(world: &mut World, id: OutsideId) -> u32 {
    let cfg = world.config.outside.parents.clone();
    let left = parent(world, id).map_or(0, |p| p.reinforce_left);
    let n = send_bodies(world, id, left.min(cfg.bodies_per_day));
    if let Some(p) = parent_mut(world, id) {
        // A body that could not come (the cap, no post, no coins) ends the episode.
        p.reinforce_left = if n < left.min(cfg.bodies_per_day) { 0 } else { p.reinforce_left - n };
    }
    n
}

/// God `ForceReinforce`: `bodies` now, the cooldown and the daily caps
/// ignored, `outsiders_cap` not.
pub fn force_reinforce(world: &mut World, id: OutsideId, bodies: u32) -> Result<String, String> {
    let now = world.tick;
    let name = world.outside.faction(id).map(|f| f.name.clone()).ok_or(format!("ForceReinforce: no parent {id}"))?;
    let Some(p) = parent_mut(world, id) else { return Err(format!("ForceReinforce: {name} is no parent")) };
    p.cohort_next += 1;
    p.last_reinforce = Some(now);
    world.outside.today.reinforcements += 1;
    let n = send_bodies(world, id, bodies);
    Ok(format!("{name} sent {n} of {bodies} people at once"))
}

/// God `SetOutsideOrder`: the order pinned for `days` (a Reinforce armed).
pub fn set_order(world: &mut World, id: OutsideId, order: OutsideOrder, days: u32) -> Result<String, String> {
    let now = world.tick;
    let name = world.outside.faction(id).map(|f| f.name.clone()).ok_or(format!("SetOutsideOrder: no parent {id}"))?;
    let Some(p) = parent_mut(world, id) else { return Err(format!("SetOutsideOrder: {name} is no parent")) };
    p.order = order;
    p.order_since = now;
    p.pinned_until = Some(now + u64::from(days) * TICKS_PER_DAY);
    if order == OutsideOrder::Reinforce {
        p.last_reinforce = None;
        start_reinforce(world, id);
    }
    Ok(format!("{name}'s order pinned to {} for {days} days", order.label()))
}

/// The daily tour pass: the dead and the gone drop off the table; an
/// Outsider past `tour_until` with a Spouse or a child settles (stays,
/// loses the record), else emigrates.
pub fn tours(world: &mut World) {
    let now = world.tick;
    let all: Vec<(EntityId, Outsider)> = world.outside.outsiders.iter().map(|(&a, o)| (a, o.clone())).collect();
    for (a, o) in all {
        if !world.is_alive(a) || !world.has::<crate::components::Identity>(a) {
            world.outside.outsiders.remove(&a);
            continue;
        }
        if o.brother || now < o.tour_until {
            continue;
        }
        world.outside.outsiders.remove(&a);
        let kin = world.spouses.contains_key(&a) || !crate::systems::demography::children_of_agent(world, a).is_empty();
        let name = world.name_of(a);
        if kin {
            world.outside.today.settled += 1;
            world.push_event(EventKind::Settled, &[a], format!("{name}'s tour ended; they stay for their family"));
        } else {
            crate::systems::demography::start_emigrating(world, a, "tour over");
            world.outside.today.left += 1;
            world.push_event(EventKind::OutsiderLeft, &[a], format!("{name}'s tour ended; they leave the city"));
        }
    }
}

/// The holder a lost building's buy-back or Retake deals with: its owner
/// when not a branch of `id` (`Some(None)`: the city, a squat or a
/// foreclosure), `None` when the building is back, gone or derelict.
fn retake_holder(world: &World, id: OutsideId, b: EntityId) -> Option<Option<EntityId>> {
    let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished)?;
    let owner = bd.owner;
    if owner.is_some_and(|o| world.comp::<Corp>(o).is_some_and(|c| c.parent == Some(id))) {
        return None;
    }
    Some(owner)
}

/// § 2: the branches' buildings against the last midnight's: a building
/// gone from them (and standing) is lost; a lost one back with a branch, or
/// past `retake_window_days`, drops off the list.
pub fn note_losses(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let mut current: Vec<EntityId> = corp_branches(world, id)
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|x| x.buildings.clone()))
        .flatten()
        .collect();
    current.sort_unstable();
    let Some(p) = parent(world, id) else { return };
    let mut lost = p.lost.clone();
    for &b in &p.held {
        if current.binary_search(&b).is_err()
            && !lost.iter().any(|l| l.building == b)
            && world.comp::<Building>(b).is_some_and(|bd| !bd.demolished)
        {
            lost.push(LostBuilding { building: b, at: now, refusals: 0, contract: None, hit_posted: false });
        }
    }
    let window = u64::from(cfg.retake_window_days) * TICKS_PER_DAY;
    lost.retain(|l| now.saturating_sub(l.at) < window && retake_holder(world, id, l.building).is_some());
    if let Some(p) = parent_mut(world, id) {
        p.held = current;
        p.lost = lost;
    }
}

/// The corp seller's rule (M11, a simple form): a corp sells a building
/// back when it is short of cash or greedy; an agent when greedy or poorer
/// than the building; a gang never; the city always.
fn accepts_offer(world: &World, holder: Option<EntityId>, value: i64) -> bool {
    match ownership::owner_kind(world, holder) {
        ownership::OwnerKind::City => true,
        ownership::OwnerKind::Gang(_) => false,
        ownership::OwnerKind::Corp(c) => world.comp::<Corp>(c).is_some_and(|x| {
            x.closing < x.treasury_ref / 2
                || x.exec.and_then(|e| world.comp::<Personality>(e)).is_some_and(|p| p.greed >= 0.5)
        }),
        ownership::OwnerKind::Agent(a) => {
            world.comp::<Personality>(a).is_some_and(|p| p.greed >= 0.5) || world.purse(Some(a)) < value
        }
    }
}

/// § 2 Retake's daily action on the newest lost building: a buy-back at
/// `retake_premium × value` (funded by the parent) while its holder has
/// refused fewer than `refusals_to_force` offers, then a `Retake` record
/// (first to the branch's own Outsiders), a Hit on a raider gang's leader
/// when the parent's lawfulness is low, and a Guard on each remaining
/// branch building. Nothing under the Embargo.
pub fn retake_step(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let Some(p) = parent(world, id) else { return };
    if p.embargoed {
        return;
    }
    let lawfulness = p.persona.lawfulness;
    let Some(i) = (0..p.lost.len()).rev().find(|&i| retake_holder(world, id, p.lost[i].building).is_some()) else {
        return;
    };
    let entry = p.lost[i].clone();
    let b = entry.building;
    let Some(holder) = retake_holder(world, id, b) else { return };
    let Some(branch) = corp_branches(world, id).into_iter().next() else { return };
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let value = ownership::value(world, kind).max(1);
    if let Some(c) = entry.contract {
        // A record stands (or ran): nothing new until it closes.
        if world.contracts.get(&c).is_some_and(|x| {
            matches!(x.status, crate::contract::ContractStatus::Open | crate::contract::ContractStatus::Taken)
        }) {
            return;
        }
        if let Some(p) = parent_mut(world, id) {
            p.lost[i].contract = None;
        }
        return;
    }
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    let (bname, hname) = (world.name_of(b), world.owner_label(holder));
    if entry.refusals < cfg.refusals_to_force {
        let price = (f64::from(cfg.retake_premium) * value as f64).round() as i64;
        world.outside.today.buybacks += 1;
        if accepts_offer(world, holder, value) {
            let got = fund(world, id, branch, price);
            if got >= price && crate::systems::corps::acquire(world, branch, b, price, "bought back") {
                world.push_event(
                    EventKind::BuyBackOffered,
                    &[branch, b],
                    format!("{pname} bought {bname} back from {hname} for {price}"),
                );
                return;
            }
        }
        world.push_event(
            EventKind::BuyBackOffered,
            &[branch, b],
            format!("{hname} refused {pname}'s offer of {price} for {bname}"),
        );
        if let Some(p) = parent_mut(world, id) {
            p.lost[i].refusals += 1;
        }
        return;
    }
    // Force: the Retake on the board, the parent's coins behind it.
    let target = crate::contract::Target::Building(b);
    let quote = crate::systems::contracts::quote(world, crate::contract::ContractKind::Retake, &target, None);
    fund(world, id, branch, quote);
    let door = world.comp::<Building>(b).map(|bd| bd.door).unwrap_or_default();
    let broker = crate::systems::contracts::nearest_fixer_to(world, door);
    let exec = world.comp::<Corp>(branch).and_then(|c| c.exec);
    let r = crate::systems::contracts::brain_post_with(
        world,
        Some(branch),
        exec,
        crate::contract::ContractKind::Retake,
        target,
        broker,
        crate::contract::Origin::Parent,
        Some(quote),
        crate::contract::PostExtra::default(),
    );
    let Ok(cid) = r else { return };
    if let Some(p) = parent_mut(world, id) {
        p.lost[i].contract = Some(cid);
    }
    // First to the branch's own: its strongest free Outsider, the others as crew.
    let mut own: Vec<EntityId> = world
        .outside
        .outsiders
        .iter()
        .filter(|(&a, o)| o.from == id && !o.brother && crate::systems::contracts::free_adult(world, a))
        .map(|(&a, _)| a)
        .collect();
    crate::systems::raid::by_strength(world, &mut own);
    if let Some((&lead, rest)) = own.split_first() {
        let max = usize::from(world.config.fixers.crew_max.saturating_sub(1));
        let crew: Vec<EntityId> = rest.iter().copied().take(max).collect();
        crate::systems::contracts::accept(world, cid, lead, &crew);
    }
    // A raider gang's leader, when the parent is lawless enough.
    if let ownership::OwnerKind::Gang(g) = ownership::owner_kind(world, holder) {
        if lawfulness < cfg.hit_lawfulness && !entry.hit_posted {
            if let Some(l) = world.comp::<Gang>(g).and_then(|x| x.leader) {
                let t = crate::contract::Target::Agent(l);
                let q = crate::systems::contracts::quote(world, crate::contract::ContractKind::Hit, &t, broker);
                fund(world, id, branch, q);
                let _ = crate::systems::contracts::brain_post_with(
                    world,
                    Some(branch),
                    exec,
                    crate::contract::ContractKind::Hit,
                    t,
                    broker,
                    crate::contract::Origin::Parent,
                    Some(q),
                    crate::contract::PostExtra::default(),
                );
                if let Some(p) = parent_mut(world, id) {
                    p.lost[i].hit_posted = true;
                }
            }
        }
    }
    // A Guard on each remaining branch building.
    let remaining: Vec<EntityId> = world.comp::<Corp>(branch).map(|c| c.buildings.clone()).unwrap_or_default();
    for rb in remaining {
        let t = crate::contract::Target::Building(rb);
        let q = crate::systems::contracts::quote(world, crate::contract::ContractKind::Guard, &t, None);
        fund(world, id, branch, q);
        crate::systems::contracts::post_secure(world, branch, rb);
    }
}

/// A won Retake (or a god's): the branch buys the building from its holder
/// at its value (funded by the parent: a forced sale), the record settles.
pub fn retake_win(world: &mut World, cid: crate::contract::ContractId) -> bool {
    let Some(c) = world.contracts.get(&cid).cloned() else { return false };
    let crate::contract::Target::Building(b) = c.target else { return false };
    let Some(branch) = c.buyer.filter(|&x| world.has::<Corp>(x)) else { return false };
    let Some(id) = world.comp::<Corp>(branch).and_then(|x| x.parent) else { return false };
    let Some(kind) = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.kind) else { return false };
    let value = ownership::value(world, kind).max(1);
    let holder = world.owner_of(b);
    let short = (value - world.purse(Some(branch))).max(0);
    if short > 0 {
        fund(world, id, branch, short);
    }
    if !crate::systems::corps::acquire(world, branch, b, value, "retaken") {
        return false;
    }
    let text = format!(
        "{} took {} back from {}",
        world.owner_label(Some(branch)),
        world.name_of(b),
        world.owner_label(holder)
    );
    if c.status == crate::contract::ContractStatus::Taken {
        crate::systems::contracts::settle(world, cid, crate::systems::contracts::Settle::Fulfilled, &text);
    } else if c.status == crate::contract::ContractStatus::Open {
        crate::systems::contracts::settle(world, cid, crate::systems::contracts::Settle::Cancelled, &text);
    }
    world.push_event(EventKind::BranchRetook, &[branch, b], text);
    true
}

/// The Retake's strike (`contracts::resolve_ledger`): the crew's summed
/// strength against the building's posted guards and, when a gang holds
/// it, its strongest members (up to `crew_max`); one keyed draw at the
/// missions' `p_win`. A win forces the sale; a loss is a failed attempt.
pub fn resolve_retake(world: &mut World, id: crate::contract::ContractId, lead: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let crate::contract::Target::Building(b) = c.target else { return };
    let Some(pid) = c.buyer.and_then(|x| world.comp::<Corp>(x)).and_then(|x| x.parent) else {
        crate::systems::contracts::fail_attempt(world, id, "the branch is gone");
        return;
    };
    if retake_holder(world, pid, b).is_none() {
        crate::systems::contracts::settle(world, id, crate::systems::contracts::Settle::Cancelled, "already back");
        return;
    }
    let mut party: Vec<EntityId> = vec![lead];
    party.extend(c.crew.iter().copied().filter(|&m| m != lead && crate::systems::contracts::free_adult(world, m)));
    let s_crew: f32 = party.iter().map(|&a| crate::systems::raid::strength(world, a)).sum();
    let mut defenders = crate::systems::raid::posted_guards(world, b);
    if let ownership::OwnerKind::Gang(g) = ownership::owner_kind(world, world.owner_of(b)) {
        let mut m: Vec<EntityId> = world.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
        crate::systems::raid::by_strength(world, &mut m);
        defenders.extend(m.into_iter().take(usize::from(world.config.fixers.crew_max)));
    }
    let s_def: f32 = defenders.iter().map(|&a| crate::systems::raid::strength(world, a)).sum::<f32>()
        + crate::systems::missions::MEDIAN_STRENGTH;
    let p = crate::systems::missions::p_win(world, s_crew, s_def);
    use rand::Rng;
    let win = world.rng.contract(crate::contract::ContractNs::Outcome, id, u64::from(c.attempts)).random::<f32>() < p;
    if win {
        if !retake_win(world, id) {
            crate::systems::contracts::fail_attempt(world, id, "the sale did not go through");
        }
    } else {
        crate::systems::contracts::fail_attempt(world, id, "the strike on the building was lost");
    }
}

/// § 2 FoundBranch: a corp named for the brand (" II", " III" past a dead
/// one), its template's niches, `branch_seed` from abroad, a grace from
/// upkeep and, when a Lot and the coins allow, its first building. The next
/// Reinforce comes with no cooldown.
pub fn found_branch(world: &mut World, id: OutsideId) -> Option<EntityId> {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let f = world.outside.faction(id)?;
    if f.kind != OutsideKind::Megacorp || !corp_branches(world, id).is_empty() || f.treasury < cfg.branch_seed {
        return None;
    }
    let brand = f.parent.as_ref()?.branch_name.clone();
    let rows = world.config.corps.row_count();
    let row = (0..rows).find(|&i| world.config.corps.template_of(i) == brand)?;
    let niches = world.config.corps.niches_of(row);
    let taken = world
        .corps()
        .into_iter()
        .filter(|&c| world.comp::<Corp>(c).is_some_and(|x| x.name.starts_with(&brand)))
        .count();
    let numerals = ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    let name = format!("{brand} {}", numerals.get(taken).copied().unwrap_or("X"));
    let corp = ownership::spawn_corp(world, name.clone(), niches.clone(), 0, None);
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.parent = Some(id);
        c.treasury_ref = cfg.branch_seed.max(1);
        c.closing = 0;
        c.upkeep_grace_until = Some(now + u64::from(cfg.upkeep_grace_days) * TICKS_PER_DAY);
    }
    fund(world, id, corp, cfg.branch_seed);
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.closing = c.treasury;
    }
    // The first building, on the lowest Lot, when the seed covers it.
    if let Some(&n) = niches.iter().next() {
        if let Some(kind) = crate::systems::corp_brain::build_kind_for(world, corp, n) {
            let cost = crate::systems::founding::found_cost(world, kind).unwrap_or(i64::MAX);
            if world.purse(Some(corp)) >= cost {
                if let Some(lot) = crate::systems::founding::vacant_lots(world).into_iter().next() {
                    if crate::systems::founding::build_on_lot(world, lot, kind, Some(corp)).is_ok() {
                        ownership::pay(world, Some(corp), None, cost, Flow::Found);
                    }
                }
            }
        }
    }
    if let Some(p) = parent_mut(world, id) {
        p.branches.push(corp);
        p.branches.sort_unstable();
        p.branch_died_at = None;
        p.last_reinforce = None;
    }
    world.outside.today.branches_founded += 1;
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    world.push_event(EventKind::BranchFounded, &[corp], format!("{pname} founded {name} in the city"));
    Some(corp)
}

/// § 2: a local corp that has held `found_parent_treasury` and a niche
/// share of `found_parent_share` for `found_parent_days` founds a parent:
/// `found_parent_frac` of its treasury crosses abroad as the account's
/// treasury (nothing minted), its income and upkeep rates on it, the exec's
/// persona, exposure 0.5. At most `max_accounts` accounts.
pub fn found_parents(world: &mut World) {
    let cfg = world.config.outside.parents.clone();
    let mut streak = std::mem::take(&mut world.outside.found_streak);
    let mut due: Vec<EntityId> = Vec::new();
    for c in world.corps() {
        let Some(x) = world.comp::<Corp>(c).filter(|x| x.parent.is_none()) else { continue };
        let share = x.share_hist.values().filter_map(|v| v.back().copied()).fold(0.0f32, f32::max);
        if x.treasury >= cfg.found_parent_treasury && share >= cfg.found_parent_share {
            let n = streak.entry(c).or_insert(0);
            *n += 1;
            if *n > cfg.found_parent_days {
                due.push(c);
            }
        } else {
            streak.remove(&c);
        }
    }
    streak.retain(|&c, _| world.has::<Corp>(c));
    world.outside.found_streak = streak;
    for c in due {
        if world.outside.factions.len() >= cfg.max_accounts {
            break;
        }
        let Some((name, treasury, exec)) = world.comp::<Corp>(c).map(|x| (x.name.clone(), x.treasury, x.exec)) else {
            continue;
        };
        let persona = exec
            .and_then(|e| world.comp::<Personality>(e))
            .map(|p| Persona { greed: p.greed, pride: p.pride, lawfulness: p.lawfulness })
            .unwrap_or_default();
        let id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
        world.outside.next_id = id + 1;
        let moved_target = (f64::from(cfg.found_parent_frac) * treasury as f64).round() as i64;
        world.outside.factions.push(OutsideFaction {
            id,
            name: format!("{name} Holdings"),
            kind: OutsideKind::Megacorp,
            treasury: 0,
            treasury_ref: moved_target.max(1),
            base_income: (f64::from(cfg.parent_income_rate) * moved_target as f64).round() as i64,
            base_upkeep: (f64::from(cfg.parent_upkeep_rate) * moved_target as f64).round() as i64,
            income_today: 0,
            market: 1.0,
            dead: false,
            books: Default::default(),
            ambient: None,
            parent: Some(Parent {
                persona,
                exposure: 0.5,
                rep: 0.5,
                health: 1.0,
                branches: vec![c],
                branch_name: name.clone(),
                ..Default::default()
            }),
        });
        let moved = ownership::cross_out(world, Some(c), id, moved_target, Flow::Remit, true);
        if let Some(x) = world.comp_mut::<Corp>(c) {
            x.parent = Some(id);
            x.cashflow_today += moved;
        }
        world.outside.found_streak.remove(&c);
        world.outside.today.parents_founded += 1;
        world.push_event(
            EventKind::ParentFounded,
            &[c],
            format!("{name} moved {moved} abroad and founded {name} Holdings"),
        );
    }
}

/// § 2 Abandon: every branch cut loose, the Outsiders' tours ended, no
/// branch refounded for `abandon_days`.
pub fn abandon(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return };
    let branches = p.branches.clone();
    for &b in &branches {
        if let Some(c) = world.comp_mut::<Corp>(b) {
            c.parent = None;
        }
    }
    for o in world.outside.outsiders.values_mut().filter(|o| o.from == id) {
        o.tour_until = o.tour_until.min(now);
        o.brother = false;
    }
    if let Some(p) = parent_mut(world, id) {
        p.branches.clear();
        p.held.clear();
        p.lost.clear();
        p.reinforce_left = 0;
        p.order = OutsideOrder::Abandon;
        p.order_since = now;
        p.abandoned_until = Some(now + u64::from(cfg.abandon_days) * TICKS_PER_DAY);
    }
    world.outside.today.abandoned += 1;
    let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    world.push_event(EventKind::Abandoned, &branches, format!("{name} cut its branches in the city loose"));
}

/// A gang's syndicate (a living parent of kind Syndicate holding it).
fn syndicate_of(world: &World, gang: EntityId) -> Option<OutsideId> {
    if !crate::systems::outside::on(world) {
        return None;
    }
    world
        .outside
        .factions
        .iter()
        .find(|f| {
            f.kind == OutsideKind::Syndicate && !f.dead && f.parent.as_ref().is_some_and(|p| p.branches.contains(&gang))
        })
        .map(|f| f.id)
}

/// Does `gang` have a living syndicate (M12's bootstrap re-formation is
/// replaced by the syndicate's refound).
pub fn has_syndicate(world: &World, gang: EntityId) -> bool {
    syndicate_of(world, gang).is_some()
}

/// § 4 brothers: a decapitation (or a sack) of a syndicate's gang
/// schedules `brothers` bodies `brother_delay_days` on, unless inside the
/// cooldown or the syndicate cannot pay. True when scheduled.
pub fn on_decapitation(world: &mut World, gang: EntityId, killer: Option<EntityId>) -> bool {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(id) = syndicate_of(world, gang) else { return false };
    let Some(f) = world.outside.faction(id) else { return false };
    let Some(p) = f.parent.as_ref() else { return false };
    let cooled =
        p.last_brothers.is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.syndicate_cooldown_days) * TICKS_PER_DAY);
    let cost = cfg.brother_cost * i64::from(cfg.brothers);
    if !cooled || p.embargoed || f.treasury < cost || p.brothers_due.iter().any(|d| d.gang == gang) {
        return false;
    }
    if let Some(p) = parent_mut(world, id) {
        p.brothers_due.push(BrothersDue {
            gang,
            at: now + u64::from(cfg.brother_delay_days) * TICKS_PER_DAY,
            killer,
            refound: false,
        });
        p.last_brothers = Some(now);
    }
    true
}

/// A syndicate's midnight: a gang branch's leader dead or jailed since the
/// last midnight is a decapitation; an emptied gang past `refound_days`
/// with a fond enough syndicate (and the coins) is refounded; the due
/// brothers arrive (members at once, an Enemy edge to a known killer).
pub fn syndicate_daily(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return };
    let gangs: Vec<EntityId> = p.branches.iter().copied().filter(|&g| world.has::<Gang>(g)).collect();
    let prev = p.leaders.clone();
    let attitude = p.attitude;
    let window = u64::from(cfg.refound_window_days) * TICKS_PER_DAY;
    let recent_refounds = p.refounds.iter().filter(|&&t| now.saturating_sub(t) < window).count();
    // Decapitations.
    for &(g, old) in &prev {
        let Some(l) = old else { continue };
        let now_leader = world.comp::<Gang>(g).and_then(|x| x.leader);
        if now_leader != Some(l) && (!world.is_alive(l) || world.has::<Sentence>(l)) {
            on_decapitation(world, g, None);
        }
    }
    // Refounds.
    for &g in &gangs {
        let Some(x) = world.comp::<Gang>(g) else { continue };
        if !(x.emptied && x.members.is_empty()) {
            continue;
        }
        let waited =
            x.empty_since.is_some_and(|t| now.saturating_sub(t) >= u64::from(cfg.refound_days) * TICKS_PER_DAY);
        let need = cfg.refound_attitude + if recent_refounds > 0 { 0.3 } else { 0.0 };
        let Some(f) = world.outside.faction(id) else { continue };
        let due_already = f.parent.as_ref().is_some_and(|p| p.brothers_due.iter().any(|d| d.gang == g));
        if waited && attitude >= need && f.treasury >= cfg.refound_cost && !due_already {
            if let Some(p) = parent_mut(world, id) {
                p.brothers_due.push(BrothersDue { gang: g, at: now, killer: None, refound: true });
            }
        }
    }
    // Arrivals.
    let due: Vec<BrothersDue> = parent(world, id).map_or_else(Vec::new, |p| p.brothers_due.clone());
    let mut keep = Vec::new();
    for d in due {
        if d.at > now {
            keep.push(d);
            continue;
        }
        if !world.has::<Gang>(d.gang) {
            continue;
        }
        let cost = if d.refound { cfg.refound_cost } else { cfg.brother_cost * i64::from(cfg.brothers) };
        if !pay_abroad(world, id, cost) {
            continue;
        }
        let door =
            world.hideout_of(d.gang).and_then(|h| world.comp::<Building>(h).map(|bd| bd.door)).unwrap_or_default();
        let mut arrived = Vec::new();
        for _ in 0..cfg.brothers {
            let a = spawn_from_abroad(world, id, door);
            crate::systems::gang::enlist(world, a, d.gang);
            world
                .outside
                .outsiders
                .insert(a, Outsider { from: id, arrived: now, tour_until: u64::MAX, cohort: 0, brother: true });
            if let Some(k) = d.killer.filter(|&k| world.is_alive(k)) {
                let e = world.edge_entry(a, k);
                e.kind = RelKind::Enemy;
                e.affinity = -0.6;
                e.last_interaction = now;
            }
            arrived.push(a);
        }
        let n = arrived.len() as u32;
        world.outside.today.brothers += n;
        let (pname, gname) =
            (world.outside.faction(id).map_or_else(String::new, |f| f.name.clone()), world.owner_label(Some(d.gang)));
        if d.refound {
            world.outside.today.refounded += 1;
            if let Some(p) = parent_mut(world, id) {
                p.refounds.push(now);
                p.refounds.retain(|&t| now.saturating_sub(t) < window);
            }
            world.push_event(EventKind::GangRefounded, &arrived, format!("{pname} sent {n} to refound {gname}"));
        } else {
            world.push_event(EventKind::BrothersArrived, &arrived, format!("{n} brothers from {pname} joined {gname}"));
        }
    }
    let leaders: Vec<(EntityId, Option<EntityId>)> =
        gangs.iter().map(|&g| (g, world.comp::<Gang>(g).and_then(|x| x.leader))).collect();
    if let Some(p) = parent_mut(world, id) {
        p.brothers_due = keep;
        p.leaders = leaders;
    }
}

/// § 4 the State's heat: `0.4 U(murders 7 d ÷ heat_murders) + 0.3 U(riots
/// 30 d) + 0.3 × the hottest gang's heat` (M15).
pub fn city_heat(world: &World) -> f32 {
    let cfg = &world.config.outside.parents;
    let h = &world.stats.history;
    let murders: u32 = h.iter().rev().take(7).map(|r| r.deaths_violence).sum();
    let riots: u32 = h.iter().rev().take(30).map(|r| r.riots).sum();
    let gang_heat =
        world.gangs().into_iter().map(|g| crate::systems::reputation::rep(world, g).heat).fold(0.0f32, f32::max);
    0.4 * sat(murders as f32 / cfg.heat_murders.max(1e-3)) + 0.3 * sat(riots as f32) + 0.3 * gang_heat
}

/// § 4 the State's table: Hold, Fund, Mandate, Withdraw, best first. Fund
/// scores at the cap too (its action caps), so a hot city keeps its guards.
pub fn state_scores(
    cfg: &crate::config::ParentsCfg,
    heat: f32,
    corrupt: f32,
    wealth: f32,
    guards: u16,
    _max: u16,
) -> Vec<(OutsideOrder, f32)> {
    let pride = cfg.factions.iter().find(|r| r.kind == OutsideKind::State).map_or(0.5, |r| r.persona[1]);
    let mut out = vec![(OutsideOrder::Hold, cfg.flat_hold)];
    let mut add = |o: OutsideOrder, cs: &[Consideration], flat: f32| {
        if cs.iter().all(|c| c.output > 0.0) {
            let raw: f32 = cs.iter().map(|c| c.output).product();
            out.push((o, compensate(raw, cs.len()) + flat));
        }
    };
    add(
        OutsideOrder::Fund,
        &[
            Consideration::new("heat", heat, Curve::Logistic { k: 8.0, mid: 0.4 }),
            Consideration::new("1 - corrupt", 1.0 - corrupt, Curve::Linear { m: 0.6, b: 0.4 }),
            Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ],
        cfg.flat_fund,
    );
    add(
        OutsideOrder::Mandate,
        &[
            Consideration::raw("hot", 0.0, can(heat >= cfg.mandate_heat)),
            Consideration::new("heat", heat, Curve::Linear { m: 0.8, b: 0.2 }),
            Consideration::new("pride", pride, Curve::Linear { m: 0.5, b: 0.5 }),
        ],
        cfg.flat_mandate,
    );
    add(
        OutsideOrder::Withdraw,
        &[
            Consideration::raw("guards", 0.0, can(guards > 0)),
            Consideration::new("1 - heat", 1.0 - heat, Curve::Linear { m: 0.7, b: 0.3 }),
            Consideration::new("corrupt", corrupt, Curve::Logistic { k: 8.0, mid: 0.5 }),
        ],
        cfg.flat_withdraw,
    );
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

fn state_id(world: &World) -> Option<OutsideId> {
    world.outside.factions.iter().find(|f| f.kind == OutsideKind::State && f.parent.is_some()).map(|f| f.id)
}

/// A week of the State's guards' wages into the Treasury (`Flow::Grant`).
fn grant_week(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let amount = i64::from(world.outside.state_guards) * cfg.grant_wage * 7;
    if amount <= 0 {
        return;
    }
    let moved = ownership::cross_in(world, id, None, amount, Flow::Grant);
    if moved > 0 {
        if let Some(f) = world.outside.faction_mut(id) {
            f.income_today += moved;
        }
        if let Some(p) = parent_mut(world, id) {
            p.ledger.spent_today += moved;
        }
        world.outside.today.grants += 1;
        world.push_event(EventKind::StateGrant, &[], format!("the State granted {moved} for its guards"));
    }
}

/// The State's order's action (also a god's, through the tests).
pub fn state_act(world: &mut World, order: OutsideOrder) {
    let cfg = world.config.outside.parents.clone();
    let Some(id) = state_id(world) else { return };
    let now = world.tick;
    match order {
        OutsideOrder::Fund => {
            world.outside.state_guards = (world.outside.state_guards + cfg.state_guards_step).min(cfg.state_guards_max);
            grant_week(world, id);
        }
        OutsideOrder::Mandate => {
            world.outside.mandate_until = Some(now + u64::from(cfg.mandate_days) * TICKS_PER_DAY);
            world.outside.today.mandates += 1;
            world.push_event(EventKind::StateMandate, &[], "the State mandated a harder posture".to_string());
        }
        OutsideOrder::Withdraw => {
            world.outside.state_guards = world.outside.state_guards.saturating_sub(cfg.state_guards_step);
            world.outside.today.withdrawals += 1;
            world.push_event(EventKind::StateWithdrew, &[], "the State withdrew some of its guards".to_string());
        }
        _ => {}
    }
}

/// The State's midnight: the heat, the captain's honour, the table with
/// hysteresis and the minimum hold; Fund raises each day it holds,
/// Withdraw lowers, a Mandate runs once; a week's grant each seventh
/// midnight while guards stand. A negative treasury forces Withdraw.
fn state_daily(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let heat = city_heat(world);
    let corrupt = world
        .building_of_kind(crate::components::BuildingKind::Jail)
        .map_or(0.0, |j| 1.0 - crate::systems::reputation::rep(world, j).honour);
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    let wealth = sat(f.treasury as f32 / f.treasury_ref.max(1) as f32);
    let current = p.order;
    let mut scores = state_scores(&cfg, heat, corrupt, wealth, world.outside.state_guards, cfg.state_guards_max);
    for s in &mut scores {
        if s.0 == current {
            s.1 += cfg.hysteresis;
        }
    }
    if mandate_active(world) {
        scores.retain(|s| s.0 != OutsideOrder::Mandate);
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let held = now.saturating_sub(p.order_since) >= u64::from(cfg.min_hold_days) * TICKS_PER_DAY;
    let still = scores.iter().any(|s| s.0 == current);
    let mut next = match scores.first() {
        Some(&(best, _)) if best != current && (held || !still) => best,
        _ => current,
    };
    if f.treasury < 0 && world.outside.state_guards > 0 {
        next = OutsideOrder::Withdraw;
    }
    let name = f.name.clone();
    if let Some(p) = parent_mut(world, id) {
        p.order_trace = scores;
        if next != current {
            p.order = next;
            p.order_since = now;
        }
    }
    if next != current {
        world.push_event(EventKind::OutsideOrder, &[], format!("{name}: {} -> {}", current.label(), next.label()));
    }
    match next {
        OutsideOrder::Fund if world.outside.state_guards < cfg.state_guards_max => state_act(world, OutsideOrder::Fund),
        OutsideOrder::Mandate if !mandate_active(world) => state_act(world, OutsideOrder::Mandate),
        OutsideOrder::Withdraw if world.outside.state_guards > 0 => state_act(world, OutsideOrder::Withdraw),
        _ => {
            if world.day().is_multiple_of(7) {
                grant_week(world, id);
            }
        }
    }
}

/// The State's Mandate runs.
pub fn mandate_active(world: &World) -> bool {
    crate::systems::outside::on(world) && world.outside.mandate_until.is_some_and(|t| t > world.tick)
}

/// The law's posture scores under a Mandate: Crackdown and Garrison +
/// `mandate_bias`, re-sorted (as `attention::tilt`).
pub fn mandate_tilt(world: &World, scores: &mut [crate::components::PostureScore]) {
    if !mandate_active(world) {
        return;
    }
    let bias = world.config.outside.parents.mandate_bias;
    for s in scores.iter_mut() {
        if matches!(s.posture, crate::components::Posture::Crackdown | crate::components::Posture::Garrison) {
            s.score += bias;
        }
    }
    scores.sort_by(|a, b| b.score.total_cmp(&a.score));
}

/// A parent's branch founded with no building yet, inside its upkeep
/// grace (M11's bankruptcy waits on it; nothing with the parents off).
pub fn in_founding_grace(world: &World, c: &Corp, now: crate::time::Tick) -> bool {
    parents_on(world)
        && c.parent.is_some_and(|p| world.outside.faction(p).is_some_and(|f| f.parent.is_some()))
        && c.upkeep_grace_until.is_some_and(|t| t > now)
}
