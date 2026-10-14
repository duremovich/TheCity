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
            can(!p.embargoed
                && !state_sanctioned(world, id)
                && f.treasury > 0
                && weakest.is_some()
                && cash < cfg.subsidise_below),
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
    let rival = if megacorp { rival_of(world, id) } else { None };
    let margin = {
        let n = p.ledger.income.len().min(7) as i64;
        let inc = sum_last(&p.ledger.income, 7);
        let up = f.base_upkeep.max(1) * n.max(1);
        if n == 0 {
            0.0
        } else {
            (inc - up) as f32 / up as f32
        }
    };
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
                    && !state_sanctioned(world, id)
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
        // Phase 4 (§ 3): Sanction and Campaign against the least-regarded rival.
        if let Some((r, regard)) = rival.filter(|_| corp_branch) {
            let ill = (1.0 - regard) / 2.0;
            let sanctioning = world.outside.sanctions.iter().any(|s| s.by == Some(id) && s.until > now);
            let sanction_cs = [
                Consideration::raw("a rival", 0.0, can(regard < cfg.sanction_regard && margin >= 0.0 && !sanctioning)),
                Consideration::new("1 - regard", ill, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("pride", persona.pride, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("margin", margin.clamp(0.0, 1.0), Curve::Linear { m: 0.5, b: 0.5 }),
            ];
            push_scored(OutsideOrder::Sanction, &sanction_cs, cfg.flat_sanction);
            let campaign_cs = [
                // A disliked rival only (the plan's table opens it on any
                // rival; every healthy parent campaigned).
                Consideration::raw(
                    "a rival",
                    0.0,
                    can(regard < cfg.sanction_regard
                        && margin >= cfg.campaign_margin
                        && uplink_node(world, r).is_some()),
                ),
                Consideration::new("1 - regard", ill, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1 - lawfulness", 1.0 - persona.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("greed", persona.greed, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
            ];
            push_scored(OutsideOrder::Campaign, &campaign_cs, cfg.flat_campaign);
        }
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
        if !pinned || p.rival.is_none() {
            p.rival = rival.map(|r| r.0);
        }
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
        OutsideOrder::Sanction => {
            let mine = world.outside.sanctions.iter().any(|s| s.by == Some(id) && s.until > now);
            if let (Some(r), false) = (parent(world, id).and_then(|p| p.rival), mine) {
                let cfg = world.config.outside.parents.clone();
                sanction(world, Some(id), r, cfg.sanction_days, cfg.sanction_cut);
            }
        }
        OutsideOrder::Campaign => campaign_step(world, id),
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
    // Phase 4 (economic warfare): the districts its corp branches control.
    let held_districts = {
        let corps = corp_branches(world, id);
        world
            .districts
            .iter()
            .filter(|d| matches!(d.control, crate::components::Controller::Corp(c) if corps.contains(&c)))
            .count() as i64
    };
    let cut = sanction_cut(world, id);
    let sanction_costs: i64 =
        world.outside.sanctions.iter().filter(|s| s.by == Some(id) && s.until > now).count() as i64 * cfg.sanction_cost;
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
    push(&mut l.districts, held_districts);
    let peak = l.branch_value.iter().copied().max().unwrap_or(0);
    let d_peak = l.districts.iter().copied().max().unwrap_or(0);
    // Phase 4: districts lost since the 30-day peak add 0.3 of their share.
    let d_term = if d_peak > 0 { 0.3 * (d_peak - held_districts).max(0) as f32 / d_peak as f32 } else { 0.0 };
    let loss =
        if kind == OutsideKind::State || all_waiting { 0.0 } else { (branch_loss(value, peak, gap) + d_term).min(1.0) };
    p.branch_loss = loss;
    p.rep = (p.rep + cfg.rep_rate * (target - p.rep)).clamp(0.0, 1.0);
    // Phase 4: the edge recovers while every track holds `data_floor`; Data
    // comes in from abroad.
    if p.data.units.iter().all(|&u| u >= cfg.data_floor) {
        p.edge += cfg.edge_recover * (1.0 - p.edge);
    }
    for (u, d) in p.data.units.iter_mut().zip(cfg.data_per_day) {
        *u += d;
    }
    p.edge = p.edge.clamp(cfg.edge_min, cfg.edge_max);
    let cut = 1.0 - (1.0 - cut) * if p.embargoed { 1.0 - cfg.embargo_cut } else { 1.0 };
    let income = parent_income(base_income, market, p.edge, p.rep, p.exposure, loss, cut);
    push(&mut p.ledger.income, income);
    let (inc7, n7) = (sum_last(&p.ledger.income, 7), p.ledger.income.len().min(7) as i64);
    let (rem7, spent7) = (sum_last(&p.ledger.remitted, 7), sum_last(&p.ledger.spent, 7));
    let delta = income - upkeep - sanction_costs;
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
    // The State never fails (its income tracks its upkeep).
    let failing = n7 >= 7 && inc7 < week_upkeep && kind != OutsideKind::State;
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
    // Phase 4: death by its creditors (income under upkeep `death_days`, or
    // a treasury under 0 `insolvent_days`). The State never dies.
    // Deviation: the creditors call in a failing parent only once its reserves
    // are under its `treasury_ref` (a parent fat on its branches' remittances
    // rides out a thin month: a founded parent, Nutrix Holdings, died on a
    // rich treasury and took the city's largest Food corp with it).
    let reserves = world.outside.faction(id).is_some_and(|f| f.treasury < f.treasury_ref);
    let Some(p) = parent(world, id) else { return };
    let failed =
        reserves && p.failing_since.is_some_and(|t| now.saturating_sub(t) >= u64::from(cfg.death_days) * TICKS_PER_DAY);
    let broke =
        p.insolvent_since.is_some_and(|t| now.saturating_sub(t) >= u64::from(cfg.insolvent_days) * TICKS_PER_DAY);
    if kind != OutsideKind::State && (failed || broke) {
        let _ = kill_parent(world, id, if failed { "its creditors called it in" } else { "insolvent" });
        return;
    }
    sync_uplink(world, id);
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
    cols.uplink_runs = i64::from(t.uplink_runs);
    cols.uplink_breached = i64::from(t.uplink_breached);
    cols.data_shipped = i64::from(t.data_shipped);
    cols.sanctions_new = i64::from(t.sanctions);
    cols.sanctions_active = o.sanctions.iter().filter(|s| s.until > world.tick).count() as i64;
    cols.campaigns = i64::from(t.campaigns);
    cols.parents_dead = i64::from(t.parents_dead);
    cols.scrip_out = o.parents().filter_map(|f| f.parent.as_ref()).map(|p| p.scrip_out).sum();
    cols.scrip_paid = t.scrip_paid;
    cols.scrip_spent = t.scrip_spent;
    cols.scrip_converted = t.scrip_converted;
    cols.scrip_wiped = t.scrip_wiped;
    cols.parents_in = t.parents_in;
    cols.parents_out = t.parents_out;
    cols.successions = t.successions;
    cols.civil_wars = t.wars;
    cols.wars_active = world
        .gang_list()
        .iter()
        .filter(|&&g| world.comp::<crate::components::Gang>(g).is_some_and(|x| x.war.is_some()))
        .count() as u32;
    cols.war_loser_dead = t.war_dead;
    cols.war_loser_fled = t.war_fled;
    cols.war_loser_splinter = t.war_splinter;
    cols.war_loser_jailed = t.war_jailed;
    cols.abroad_posted = t.abroad_posted;
    cols.abroad_done = t.abroad_done;
    cols.abroad_won = t.abroad_won;
    cols.abroad_dead = t.abroad_dead;
    cols.away = o.away.len() as u32;
    cols.promoted = t.promoted;
    cols.cast_arrived = t.cast_arrived;
    cols.breaches = t.breaches;
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
            edge: p.edge,
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
/// `econ::hash_unit` purpose of the brothers' roll on a gutted gang (phase 4b).
const PURPOSE_BROTHERS: u64 = 0x0E17_0005;

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
    // M17 review: every spend is booked (attitude and Abandon read it).
    if let Some(p) = parent_mut(world, id) {
        p.ledger.spent_today += cost;
    }
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
        // M17 review: every spend is booked (attitude and Abandon read it).
        if let Some(p) = parent_mut(world, id) {
            p.ledger.spent_today += moved;
        }
    }
    moved
}

/// M17 review: coins `fund` sent for a post that failed go back abroad (as
/// much as the branch still holds), the booking undone.
fn unfund(world: &mut World, id: OutsideId, branch: EntityId, amount: i64) {
    let back = amount.min(world.purse(Some(branch)).max(0));
    if back <= 0 {
        return;
    }
    let moved = ownership::cross_out(world, Some(branch), id, back, Flow::ParentFund, true);
    if moved > 0 {
        if let Some(f) = world.outside.faction_mut(id) {
            f.income_today -= moved;
        }
        if let Some(c) = world.comp_mut::<Corp>(branch) {
            c.cashflow_today += moved;
        }
        if let Some(p) = parent_mut(world, id) {
            p.ledger.spent_today -= moved;
        }
    }
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
            // M17 review: the sale fell through: the coins go back abroad.
            unfund(world, id, branch, got);
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
    let sent = fund(world, id, branch, quote);
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
    let Ok(cid) = r else {
        unfund(world, id, branch, sent);
        return;
    };
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
                let sent = fund(world, id, branch, q);
                let posted = crate::systems::contracts::brain_post_with(
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
                if posted.is_err() {
                    unfund(world, id, branch, sent);
                }
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
    // M17 review: the numeral counts the branches this parent founded (a
    // living count repeated "II" after a death, and the 20k "Arasaka 2"
    // copies inflated it).
    let taken = f.parent.as_ref().map_or(0, |p| p.founded as usize);
    let numerals = ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    let name = match numerals.get(taken) {
        Some(n) => format!("{brand} {n}"),
        None => format!("{brand} {}", taken + 2),
    };
    if let Some(p) = parent_mut(world, id) {
        p.founded += 1;
    }
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
                        crate::systems::industry::pay_build(world, Some(corp), cost, Some(lot), true);
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
        found_parent_for(world, c);
    }
}

/// A local corp founds its parent: `found_parent_frac` of its treasury
/// moved abroad into a new megacorp account it is the branch of. The
/// account's id, or `None` for no corp.
fn found_parent_for(world: &mut World, c: EntityId) -> Option<OutsideId> {
    let cfg = world.config.outside.parents.clone();
    {
        let (name, treasury, exec) = world.comp::<Corp>(c).map(|x| (x.name.clone(), x.treasury, x.exec))?;
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
                edge: 1.0,
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
        Some(id)
    }
}

/// God `FoundParent`: `corp` founds its parent now, whatever its treasury
/// and share (not past `max_accounts`, not a branch already).
pub fn god_found_parent(world: &mut World, corp: EntityId) -> Result<String, String> {
    if !parents_on(world) {
        return Err("FoundParent: the parents are off".into());
    }
    let x = world.comp::<Corp>(corp).ok_or("FoundParent: no corp".to_string())?;
    if x.parent.is_some() {
        return Err(format!("FoundParent: {} has a parent", x.name));
    }
    // M17 review: a parent founded on nothing would starve (its income and
    // upkeep scale with the coins it moves abroad): the natural threshold.
    let need = world.config.outside.parents.found_parent_treasury;
    if x.treasury < need {
        return Err(format!("FoundParent: {} holds {} (needs {need})", x.name, x.treasury));
    }
    if world.outside.factions.len() >= world.config.outside.parents.max_accounts {
        return Err("FoundParent: no account left".into());
    }
    let id = found_parent_for(world, corp).ok_or("FoundParent: no corp".to_string())?;
    let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    Ok(format!("God: {name} founded"))
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
    // Phase 4: its scrip redeemed at health, or wiped below `scrip_par_min`.
    let h = parent(world, id).map_or(0.0, |p| p.health);
    if h < cfg.scrip_par_min {
        wipe_scrip(world, id);
    } else {
        let (paid, _) = redeem_all(world, id, h);
        world.outside.today.scrip_spent += paid;
    }
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

/// Jobs & lives phase 3: a gang's living syndicate (`None` with the
/// outside off or none), which ships its crates.
pub fn syndicate_id(world: &World, gang: EntityId) -> Option<OutsideId> {
    syndicate_of(world, gang)
}

/// Jobs & lives phase 3: a parent's treasury abroad (0 for none).
pub fn treasury_abroad(world: &World, id: OutsideId) -> i64 {
    world.outside.faction(id).map_or(0, |f| f.treasury)
}

/// Jobs & lives phase 3: a parent pays a cost abroad (a crate shipped in
/// kind): out of its treasury and out of `minted`, booked as spent.
pub fn spend_abroad(world: &mut World, id: OutsideId, cost: i64) -> bool {
    pay_abroad(world, id, cost)
}

/// Does `gang` have a living syndicate (M12's bootstrap re-formation is
/// replaced by the syndicate's refound).
pub fn has_syndicate(world: &World, gang: EntityId) -> bool {
    syndicate_of(world, gang).is_some()
}

/// God `SendBrothers`: the gang's syndicate sends its brothers at the next
/// midnight, the cooldown ignored (the syndicate pays).
pub fn send_brothers(world: &mut World, gang: EntityId) -> Result<String, String> {
    let gname = world.owner_label(Some(gang));
    let id = syndicate_of(world, gang).ok_or(format!("SendBrothers: {gname} has no syndicate"))?;
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let f = world.outside.faction(id).ok_or("SendBrothers: no account".to_string())?;
    let pname = f.name.clone();
    if f.treasury < cfg.brother_cost * i64::from(cfg.brothers) {
        return Err(format!("SendBrothers: {pname} cannot pay"));
    }
    let p = parent_mut(world, id).ok_or(format!("SendBrothers: {pname} is no parent"))?;
    if p.brothers_due.iter().any(|d| d.gang == gang) {
        return Err(format!("SendBrothers: brothers already due for {gname}"));
    }
    p.brothers_due.push(BrothersDue { gang, at: now, killer: None, refound: false });
    p.last_brothers = Some(now);
    Ok(format!("{pname} will send {} brothers to {gname} at midnight", cfg.brothers))
}

/// § 4 brothers: a syndicate's gang gutted by a sack schedules `brothers`
/// bodies `brother_delay_days` on, unless inside the cooldown or the
/// syndicate cannot pay. True when scheduled.
pub fn schedule_brothers(world: &mut World, gang: EntityId, killer: Option<EntityId>) -> bool {
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

/// A syndicate's midnight: a gang branch sacked since the last midnight
/// with fewer than `brothers_gutted_below` free members gets brothers on a
/// keyed roll under `brothers_p` (phase 4b: a decapitation is the gang's
/// own succession, `systems::succession`); an emptied gang past
/// `refound_days` with a fond enough syndicate (and the coins) is
/// refounded; the due brothers arrive (members at once, an Enemy edge to a
/// known killer).
pub fn syndicate_daily(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return };
    let gangs: Vec<EntityId> = p.branches.iter().copied().filter(|&g| world.has::<Gang>(g)).collect();
    let attitude = p.attitude;
    let window = u64::from(cfg.refound_window_days) * TICKS_PER_DAY;
    let recent_refounds = p.refounds.iter().filter(|&&t| now.saturating_sub(t) < window).count();
    // A gang gutted by a sack (an emptied one waits on its refound).
    let sacked_for = world.config.gangs.sacked_days * TICKS_PER_DAY;
    for &g in &gangs {
        let sacked_at = world
            .comp::<Gang>(g)
            .filter(|x| !x.members.is_empty())
            .and_then(|x| x.sacked_until)
            .map(|u| u.saturating_sub(sacked_for));
        if !sacked_at.is_some_and(|t| t <= now && now - t < TICKS_PER_DAY)
            || crate::systems::gang::fit_headcount(world, g) >= cfg.brothers_gutted_below
        {
            continue;
        }
        let u =
            crate::systems::econ::hash_unit(world.seed(), PURPOSE_BROTHERS, u64::from(g.index), now / TICKS_PER_DAY);
        if u < cfg.brothers_p {
            schedule_brothers(world, g, None);
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

/// The State's guards at most: `state_guards_max`, under the city's
/// `SetStateGuards` cap when one is set.
pub fn state_cap(world: &World) -> u16 {
    let max = world.config.outside.parents.state_guards_max;
    world.outside.state_guards_cap.map_or(max, |c| c.min(max))
}

/// The State's order's action (also a god's, through the tests).
pub fn state_act(world: &mut World, order: OutsideOrder) {
    let cfg = world.config.outside.parents.clone();
    let Some(id) = state_id(world) else { return };
    let now = world.tick;
    match order {
        OutsideOrder::Fund => {
            world.outside.state_guards = (world.outside.state_guards + cfg.state_guards_step).min(state_cap(world));
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
    // Phase 4: in a hot city (`mandate_heat`), a megacorp parent whose
    // branch is this hot is sanctioned (deviation: the plan's table
    // sanctioned on the branch's heat alone, and M15's corp heat sits near
    // the threshold on most days: Militech was sanctioned on day 3).
    let city_hot = city_heat(world) >= cfg.mandate_heat;
    let hot: Vec<OutsideId> = world
        .outside
        .parents()
        .filter(|f| f.kind == OutsideKind::Megacorp && !f.dead)
        .filter(|f| {
            f.parent.as_ref().is_some_and(|p| {
                p.branches.iter().any(|&b| crate::systems::reputation::rep(world, b).heat >= cfg.state_sanction_heat)
            })
        })
        .map(|f| f.id)
        .filter(|&x| city_hot && !state_sanctioned(world, x))
        .collect();
    for x in hot {
        sanction(world, None, x, cfg.sanction_days, cfg.sanction_cut);
    }
    let heat = city_heat(world);
    let corrupt = world
        .building_of_kind(crate::components::BuildingKind::Jail)
        .map_or(0.0, |j| 1.0 - crate::systems::reputation::rep(world, j).honour);
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    let wealth = sat(f.treasury as f32 / f.treasury_ref.max(1) as f32);
    let current = p.order;
    let mut scores = state_scores(&cfg, heat, corrupt, wealth, world.outside.state_guards, state_cap(world));
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
        OutsideOrder::Fund if world.outside.state_guards < state_cap(world) => state_act(world, OutsideOrder::Fund),
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

// ---------------------------------------------------------------------------
// Phase 4: the weapons that reach the ledger, and scrip (docs/M17_OUTSIDE.md
// § 3-4, "Revised 2026-10-12").
//
// The uplink is a node of the M14 plane (`NodeKind::Uplink`): a window on
// the parent's Data and treasury, behind a tier-3 Trunk from its branch's
// Ledger; its ICE is the parent's. A run on it lands abroad: a theft cuts
// the parent's edge, a wipe zeroes its Data in the touched tracks and drops
// a tier there (its branches' effective tiers are capped by the parent's,
// `tech_ceiling`), a Ledger run siphons its treasury. Sanctions cut income
// (and the World's buying from the sanctioned branch); a parent under its
// upkeep for `death_days` dies, its corp branches go bankrupt and its scrip
// is wiped. Scrip is a share of a branch's wages whose coins cross to the
// parent at issue: it is not coins, so `econ::identity` never sees it; it
// becomes coins again only by crossing back from the parent (spent at the
// issuer's buildings, converted at a Fixer, redeemed under `BanScrip`).
// ---------------------------------------------------------------------------

/// The living megacorp parents with a corp branch, and that branch (the
/// uplink's anchor): `virt::wanted`'s and `links`'s source.
pub fn wanted_uplinks(world: &World) -> Vec<(OutsideId, EntityId)> {
    if !parents_on(world) {
        return Vec::new();
    }
    world
        .outside
        .parents()
        .filter(|f| f.kind == OutsideKind::Megacorp && !f.dead)
        .filter_map(|f| corp_branches(world, f.id).first().map(|&b| (f.id, b)))
        .collect()
}

/// A parent's alive uplink node.
pub fn uplink_node(world: &World, id: OutsideId) -> Option<crate::virt::NodeId> {
    world
        .virt
        .nodes
        .iter()
        .position(|n| n.alive && n.kind == crate::virt::NodeKind::Uplink(id))
        .map(|i| crate::virt::NodeId(i as u16))
}

/// An uplink's ICE: `uplink_ice` capped by the parent's Deck tier.
pub fn uplink_ice(world: &World, id: OutsideId) -> u8 {
    let ice = world.config.outside.parents.uplink_ice;
    match parent(world, id).map(|p| p.tech[crate::virt::Track::Deck.index()]) {
        Some(t) if t > 0 => ice.min(t),
        _ => ice,
    }
}

/// The ceiling a branch's parent puts on its tier in `t` (3: none).
pub fn tech_ceiling(world: &World, corp: &Corp, t: crate::virt::Track) -> u8 {
    if !parents_on(world) {
        return 3;
    }
    corp.parent.and_then(|p| parent(world, p)).map(|p| p.tech[t.index()]).filter(|&x| x > 0).unwrap_or(3)
}

/// The uplink node's store mirrors the parent's Data.
fn sync_uplink(world: &mut World, id: OutsideId) {
    let Some(data) = parent(world, id).map(|p| p.data.clone()) else { return };
    if let Some(n) = uplink_node(world, id) {
        if let Some(node) = world.virt.node_mut(n) {
            if node.store != data {
                node.store = data;
                crate::systems::virt::bump_epoch(world);
            }
        }
    }
}

/// Take up to `budget` units from the parent's Data, the largest track
/// first (ties the lower track), as `act_begin` takes from a store.
pub fn take_abroad(world: &mut World, id: OutsideId, budget: u32) -> [u32; 3] {
    let mut out = [0u32; 3];
    let Some(p) = parent_mut(world, id) else { return out };
    let mut left = budget;
    let mut order: Vec<(std::cmp::Reverse<u32>, usize)> =
        p.data.units.iter().enumerate().map(|(i, &u)| (std::cmp::Reverse(u), i)).collect();
    order.sort_unstable();
    for (_, i) in order {
        let got = p.data.units[i].min(left);
        p.data.units[i] -= got;
        out[i] += got;
        left -= got;
    }
    sync_uplink(world, id);
    out
}

/// A payload that had nowhere to go goes back abroad.
pub fn return_abroad(world: &mut World, id: OutsideId, payload: [u32; 3]) {
    if let Some(p) = parent_mut(world, id) {
        for (u, x) in p.data.units.iter_mut().zip(payload) {
            *u += x;
        }
    }
    sync_uplink(world, id);
}

/// A run on an uplink lands: a theft cuts the edge by `edge_per_unit` a
/// unit; a wipe zeroes the touched tracks, drops a tier where it is 2 or
/// more and cuts `edge_wipe`; a Ledger run crosses `min(uplink_frac ×
/// treasury, uplink_cap)` to the patron (`Flow::Hack`).
pub fn uplink_landed(
    world: &mut World,
    id: OutsideId,
    purpose: crate::virt::Purpose,
    units: u32,
    patron: Option<EntityId>,
    runner: EntityId,
) {
    let cfg = world.config.outside.parents.clone();
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    let who = if runner == EntityId::NONE { "a runner".to_string() } else { world.name_of(runner) };
    world.outside.today.uplink_runs += 1;
    let text = match purpose {
        crate::virt::Purpose::Data { wipe: false } => {
            if let Some(p) = parent_mut(world, id) {
                p.edge = (p.edge - cfg.edge_per_unit * units as f32).clamp(cfg.edge_min, cfg.edge_max);
            }
            format!("{who} stole {units} Data through {pname}'s uplink")
        }
        crate::virt::Purpose::Data { wipe: true } => {
            let mut dropped = Vec::new();
            if let Some(p) = parent_mut(world, id) {
                for t in crate::virt::Track::ALL {
                    if p.data.units[t.index()] == 0 {
                        continue;
                    }
                    p.data.units[t.index()] = 0;
                    if p.tech[t.index()] >= 2 {
                        p.tech[t.index()] -= 1;
                        dropped.push(t.label());
                    }
                }
                p.edge = (p.edge - cfg.edge_wipe).clamp(cfg.edge_min, cfg.edge_max);
            }
            sync_uplink(world, id);
            crate::systems::virt::bump_epoch(world);
            format!(
                "{who} wiped {pname}'s Data abroad (tiers lost: {})",
                if dropped.is_empty() { "none".into() } else { dropped.join(", ") }
            )
        }
        crate::virt::Purpose::Ledger => {
            let treasury = world.outside.faction(id).map_or(0, |f| f.treasury.max(0));
            let take = ((f64::from(cfg.uplink_frac) * treasury as f64).floor() as i64).min(cfg.uplink_cap);
            let moved = ownership::cross_in(world, id, patron, take, Flow::Hack);
            if let Some(f) = world.outside.faction_mut(id) {
                f.income_today += moved;
            }
            format!("{who} siphoned {moved} from {pname}'s ledger abroad")
        }
        _ => return,
    };
    world.outside.today.uplink_breached += 1;
    let actors = [runner, patron.unwrap_or(EntityId::NONE)];
    world.push_event(EventKind::UplinkBreached, &actors, text);
    // Phase 5: the outside traces the runner; the Blackwall may answer.
    crate::systems::abroad::on_uplink_run(world, id, runner, patron);
}

/// `units` of Data gone from `id` abroad: its edge cut by `edge_per_unit` a unit.
pub fn cut_edge(world: &mut World, id: OutsideId, units: u32) {
    let cfg = world.config.outside.parents.clone();
    if let Some(p) = parent_mut(world, id) {
        p.edge = (p.edge - cfg.edge_per_unit * units as f32).clamp(cfg.edge_min, cfg.edge_max);
    }
}

/// God `ForceUplinkRun`: the run's effect now, no runner (a theft's units
/// at a tier-3 deck's take, to the patron's first Lab when it has one).
pub fn force_uplink_run(
    world: &mut World,
    id: OutsideId,
    purpose: crate::virt::Purpose,
    patron: Option<EntityId>,
) -> Result<String, String> {
    let Some(f) = world.outside.faction(id).filter(|f| f.parent.is_some() && !f.dead) else {
        return Err(format!("ForceUplinkRun: no living parent {id}"));
    };
    let name = f.name.clone();
    let mut units = 0;
    if purpose == (crate::virt::Purpose::Data { wipe: false }) {
        let budget = world.config.data.steal_units.get(2).copied().unwrap_or(0);
        let payload = take_abroad(world, id, budget);
        units = payload.iter().sum();
        let lab = patron
            .map(|x| crate::systems::tech::labs_of(world, x))
            .and_then(|v| v.first().copied())
            .and_then(|l| crate::systems::virt::node_of_building(world, l));
        if let Some(n) = lab {
            if let Some(node) = world.virt.node_mut(n) {
                for (u, x) in node.store.units.iter_mut().zip(payload) {
                    *u += x;
                }
            }
        }
    }
    uplink_landed(world, id, purpose, units, patron, EntityId::NONE);
    Ok(format!("God: a run on {name}'s uplink ({purpose:?})"))
}

/// § 3 sanctions: `by` (a rival parent; `None` the city or the State) cuts
/// `on`'s income by `cut` for `days`.
pub fn sanction(world: &mut World, by: Option<OutsideId>, on: OutsideId, days: u32, cut: f32) {
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.outside.sanctions.push(crate::outside::Sanction { by, on, until, cut: cut.clamp(0.0, 1.0), crew: false });
    world.outside.today.sanctions += 1;
    let name = |w: &World, x: OutsideId| w.outside.faction(x).map_or_else(String::new, |f| f.name.clone());
    let text = match by {
        Some(b) => format!("{} sanctioned {} for {days} days", name(world, b), name(world, on)),
        None => format!("the State sanctioned {} for {days} days", name(world, on)),
    };
    world.push_event(EventKind::Sanctioned, &[], text);
}

/// M17 review: a crew abroad burned `on`'s convoy: its income cut by `cut`
/// for `days`, the crew's sanction (no State's: subsidies and bodies still
/// cross, and a State sanction can still come).
pub fn crew_sanction(world: &mut World, on: OutsideId, days: u32, cut: f32) {
    let until = world.tick + u64::from(days) * TICKS_PER_DAY;
    world.outside.sanctions.push(crate::outside::Sanction {
        by: None,
        on,
        until,
        cut: cut.clamp(0.0, 1.0),
        crew: true,
    });
    world.outside.today.sanctions += 1;
    let name = world.outside.faction(on).map_or_else(String::new, |f| f.name.clone());
    world.push_event(
        EventKind::Sanctioned,
        &[],
        format!("a crew burned {name}'s convoy abroad: its income cut for {days} days"),
    );
}

/// The combined cut of the sanctions standing on `id`: `1 − Π(1 − cut)`.
pub fn sanction_cut(world: &World, id: OutsideId) -> f32 {
    let now = world.tick;
    1.0 - world.outside.sanctions.iter().filter(|s| s.on == id && s.until > now).map(|s| 1.0 - s.cut).product::<f32>()
}

/// The State's (or the city's) sanction stands on `id`: no subsidy or body crosses.
pub fn state_sanctioned(world: &World, id: OutsideId) -> bool {
    let now = world.tick;
    world.outside.sanctions.iter().any(|s| s.on == id && s.by.is_none() && !s.crew && s.until > now)
}

/// The cut on a corp's sales to the World (a sanction on its parent).
pub fn world_cut_for(world: &World, owner: Option<EntityId>) -> f32 {
    if !parents_on(world) {
        return 0.0;
    }
    owner.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.parent).map_or(0.0, |p| sanction_cut(world, p))
}

/// God `SanctionParent`.
pub fn god_sanction(world: &mut World, id: OutsideId, days: u32, cut: f32) -> Result<String, String> {
    if parent(world, id).is_none() {
        return Err(format!("SanctionParent: no parent {id}"));
    }
    sanction(world, None, id, days, cut);
    Ok(format!("God: sanctioned parent {id} for {days} days"))
}

/// God `SetParentEdge`.
pub fn set_edge(world: &mut World, id: OutsideId, edge: f32) -> Result<String, String> {
    let cfg = world.config.outside.parents.clone();
    let Some(p) = parent_mut(world, id) else { return Err(format!("SetParentEdge: no parent {id}")) };
    p.edge = edge.clamp(cfg.edge_min, cfg.edge_max);
    Ok(format!("God: parent {id}'s edge set to {:.2}", p.edge))
}

/// God `WipeParentTreasury`: the treasury absorbed abroad (`minted −= it`).
pub fn wipe_treasury(world: &mut World, id: OutsideId) -> Result<String, String> {
    let Some(f) = world.outside.faction_mut(id).filter(|f| f.parent.is_some()) else {
        return Err(format!("WipeParentTreasury: no parent {id}"));
    };
    let t = std::mem::take(&mut f.treasury);
    world.outside.minted -= t;
    Ok(format!("God: parent {id}'s treasury of {t} wiped"))
}

/// A holder's scrip of `issuer`.
pub fn scrip_of(world: &World, agent: EntityId, issuer: OutsideId) -> i64 {
    world.outside.scrip.get(&agent).map_or(0, |v| v.iter().filter(|&&(i, _)| i == issuer).map(|&(_, n)| n).sum())
}

fn add_scrip(world: &mut World, agent: EntityId, issuer: OutsideId, delta: i64) {
    let v = world.outside.scrip.entry(agent).or_default();
    match v.iter_mut().find(|(i, _)| *i == issuer) {
        Some(e) => e.1 += delta,
        None => v.push((issuer, delta)),
    }
    v.retain(|&(_, n)| n > 0);
    if v.is_empty() {
        world.outside.scrip.remove(&agent);
    }
    if let Some(p) = parent_mut(world, issuer) {
        p.scrip_out = (p.scrip_out + delta).max(0);
    }
}

/// Scrip's rate: par while the issuer's health holds `scrip_par_min`, else its health.
fn scrip_rate(world: &World, issuer: OutsideId) -> f32 {
    let par = world.config.outside.parents.scrip_par_min;
    parent(world, issuer).map_or(0.0, |p| if p.health >= par { 1.0 } else { p.health.max(0.0) })
}

/// The issuer whose scrip a wage from `payer` pays: a corp branch of a
/// living megacorp parent, scrip on and not banned.
fn issuer_of(world: &World, payer: Option<EntityId>) -> Option<OutsideId> {
    let cfg = &world.config.outside.parents;
    if !cfg.scrip_enabled || world.outside.scrip_banned || !parents_on(world) {
        return None;
    }
    let p = payer.and_then(|x| world.comp::<Corp>(x)).and_then(|c| c.parent)?;
    world.outside.faction(p).filter(|f| f.kind == OutsideKind::Megacorp && !f.dead && f.parent.is_some()).map(|f| f.id)
}

/// `ownership::transfer`'s wage hook: `scrip_wage_share` of a wage `from` a
/// parented corp branch `to` an agent is scrip; its coins cross from the
/// agent to the parent at issue.
pub fn scrip_wage(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, moved: i64) {
    let Some(issuer) = issuer_of(world, from) else { return };
    let Some(a) = to.filter(|&a| world.has::<crate::components::Wallet>(a)) else { return };
    let share = (f64::from(world.config.outside.parents.scrip_wage_share) * moved as f64).round() as i64;
    if share <= 0 {
        return;
    }
    let crossed = ownership::cross_out(world, Some(a), issuer, share, Flow::Scrip, false);
    if crossed <= 0 {
        return;
    }
    if let Some(f) = world.outside.faction_mut(issuer) {
        f.income_today -= crossed;
    }
    add_scrip(world, a, issuer, crossed);
    world.outside.today.scrip_paid += crossed;
}

/// `ownership::transfer`'s spending hook: an agent paying food or rent to a
/// building owner that is a branch of an issuer whose scrip it holds pays
/// scrip first: the parent redeems it into the agent's purse at its rate
/// (par, or health below `scrip_par_min`), then the coins move as usual.
pub fn scrip_spend(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, amount: i64) {
    if amount <= 0 || world.outside.scrip.is_empty() {
        return;
    }
    let Some(a) = from else { return };
    let Some(issuer) = to.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.parent) else { return };
    let held = scrip_of(world, a, issuer);
    if held <= 0 || world.outside.faction(issuer).is_none_or(|f| f.dead) {
        return;
    }
    let rate = scrip_rate(world, issuer);
    if rate <= 0.0 {
        return;
    }
    let used = (((amount as f64) / f64::from(rate)).ceil() as i64).min(held);
    let coins = ((used as f64) * f64::from(rate)).floor() as i64;
    let moved = ownership::cross_in(world, issuer, Some(a), coins, Flow::Scrip);
    if moved <= 0 {
        return;
    }
    if let Some(f) = world.outside.faction_mut(issuer) {
        f.income_today += moved;
    }
    let spent = if moved < coins { ((moved as f64) / f64::from(rate)).ceil() as i64 } else { used };
    add_scrip(world, a, issuer, -spent.min(held));
    world.outside.today.scrip_spent += moved;
}

/// § 4 conversion at a Fixer (deviation: a daily pass, not a walk): a holder
/// with `convert_min` scrip and under three meals' coins converts at the
/// issuer's health, `fixer_cut` to the nearest open Fixer's owner (the
/// city's when none), both from the parent's treasury; one day in seven
/// per holder (`all` in tests).
pub fn convert_pass(world: &mut World, all: bool) {
    let cfg = world.config.outside.parents.clone();
    let day = world.day();
    let meal = world.mean_price().max(1);
    let fixer = crate::systems::contracts::open_fixers(world).first().copied();
    let owner = fixer.and_then(|f| world.owner_of(f));
    let holders: Vec<(EntityId, Vec<(OutsideId, i64)>)> =
        world.outside.scrip.iter().map(|(&a, v)| (a, v.clone())).collect();
    for (a, held) in holders {
        if !all && !(u64::from(a.index) + day).is_multiple_of(7) {
            continue;
        }
        if world.purse(Some(a)) >= 3 * meal {
            continue;
        }
        for (issuer, units) in held {
            if units < cfg.convert_min || world.outside.faction(issuer).is_none_or(|f| f.dead) {
                continue;
            }
            let h = parent(world, issuer).map_or(0.0, |p| p.health.clamp(0.0, 1.0));
            let to_holder = (units as f64 * f64::from(h) * f64::from(1.0 - cfg.fixer_cut)).floor() as i64;
            let to_fixer = (units as f64 * f64::from(h) * f64::from(cfg.fixer_cut)).floor() as i64;
            let m1 = ownership::cross_in(world, issuer, Some(a), to_holder, Flow::Scrip);
            let m2 = ownership::cross_in(world, issuer, owner, to_fixer, Flow::Scrip);
            if let Some(f) = world.outside.faction_mut(issuer) {
                f.income_today += m1 + m2;
            }
            add_scrip(world, a, issuer, -units);
            world.outside.today.scrip_converted += m1;
        }
    }
}

/// Every holder's scrip of `issuer` redeemed at `rate` (or wiped at 0).
fn redeem_all(world: &mut World, issuer: OutsideId, rate: f32) -> (i64, i64) {
    let holders: Vec<(EntityId, i64)> = world
        .outside
        .scrip
        .iter()
        .filter_map(|(&a, v)| v.iter().find(|&&(i, _)| i == issuer).map(|&(_, n)| (a, n)))
        .collect();
    let (mut paid, mut units) = (0, 0);
    for (a, n) in holders {
        let coins = (n as f64 * f64::from(rate)).floor() as i64;
        if coins > 0 && world.is_alive(a) {
            let m = ownership::cross_in(world, issuer, Some(a), coins, Flow::Scrip);
            if let Some(f) = world.outside.faction_mut(issuer) {
                f.income_today += m;
            }
            paid += m;
        }
        units += n;
        add_scrip(world, a, issuer, -n);
    }
    (paid, units)
}

/// § 4 the wipe: every holder's scrip of `issuer` gone; each district's
/// unrest rises by `scrip_unrest × wiped there ÷ its residents' coins`.
fn wipe_scrip(world: &mut World, issuer: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let holders: Vec<(EntityId, i64)> = world
        .outside
        .scrip
        .iter()
        .filter_map(|(&a, v)| v.iter().find(|&&(i, _)| i == issuer).map(|&(_, n)| (a, n)))
        .collect();
    if holders.is_empty() {
        return;
    }
    let mut by_district: BTreeMap<usize, i64> = BTreeMap::new();
    let mut total = 0;
    for (a, n) in holders {
        if let Some(h) = world.comp::<crate::components::Household>(a).and_then(|h| h.home) {
            *by_district.entry(world.district_of_building(h).index()).or_insert(0) += n;
        }
        total += n;
        add_scrip(world, a, issuer, -n);
    }
    for (d, wiped) in by_district {
        let coins: i64 =
            world.districts.get(d).map_or(0, |x| x.residents.iter().map(|&r| world.purse(Some(r)).max(0)).sum());
        if let Some(x) = world.districts.get_mut(d) {
            x.unrest = (x.unrest + cfg.scrip_unrest * wiped as f32 / coins.max(1) as f32).min(1.0);
        }
    }
    world.outside.today.scrip_wiped += total;
    let name = world.outside.faction(issuer).map_or_else(String::new, |f| f.name.clone());
    world.push_event(EventKind::ScripWiped, &[], format!("{name}'s scrip is worthless: {total} units wiped"));
}

/// `BanScrip`: no issuance; every holder redeemed at its issuer's rate.
pub fn ban_scrip(world: &mut World, on: bool) -> Result<String, String> {
    world.outside.scrip_banned = on;
    if !on {
        return Ok("Scrip allowed again".to_string());
    }
    let issuers: Vec<OutsideId> = world.outside.parents().map(|f| f.id).collect();
    let mut paid = 0;
    for i in issuers {
        let rate = scrip_rate(world, i);
        paid += redeem_all(world, i, rate).0;
    }
    world.outside.today.scrip_spent += paid;
    Ok(format!("Scrip banned; {paid} redeemed"))
}

/// § 3 death: `dead`, `ParentDied`; every corp branch bankrupt (M11's
/// estate rule), its gangs on their own, its Outsiders' tours ended, its
/// scrip wiped, its uplink dropped (a relink), its treasury absorbed abroad.
pub fn kill_parent(world: &mut World, id: OutsideId, why: &str) -> Result<String, String> {
    let now = world.tick;
    let Some(f) = world.outside.faction(id).filter(|f| f.parent.is_some() && !f.dead) else {
        return Err(format!("KillParent: no living parent {id}"));
    };
    if f.kind == OutsideKind::State {
        return Err("KillParent: the State never dies".to_string());
    }
    let name = f.name.clone();
    let branches = f.parent.as_ref().map(|p| p.branches.clone()).unwrap_or_default();
    world.push_event(EventKind::ParentDied, &branches, format!("{name} died abroad ({why})"));
    for &b in &branches {
        if world.has::<Corp>(b) {
            crate::systems::corps::bankrupt(world, b);
        }
    }
    for o in world.outside.outsiders.values_mut().filter(|o| o.from == id) {
        o.tour_until = o.tour_until.min(now);
        o.brother = false;
    }
    wipe_scrip(world, id);
    if let Some(f) = world.outside.faction_mut(id) {
        f.dead = true;
        let t = std::mem::take(&mut f.treasury);
        if let Some(p) = f.parent.as_mut() {
            p.branches.clear();
            p.reinforce_left = 0;
        }
        world.outside.minted -= t;
    }
    world.outside.today.parents_dead += 1;
    crate::systems::virt::mark_dirty(world);
    Ok(format!("{name} is dead"))
}

/// § 3: Data shipped to a branch whose tech upkeep would lapse for want of
/// it: `ship_data[tier]` units of the track into its first Lab.
pub fn ship_data(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    for c in corp_branches(world, id) {
        let Some(lab) = crate::systems::tech::labs_of(world, c).first().copied() else { continue };
        let Some(n) = crate::systems::virt::node_of_building(world, lab) else { continue };
        for t in crate::virt::Track::ALL {
            let (tier, lapse) = world.comp::<Corp>(c).map_or((1, 0), |x| (x.tech.tier_of(t), x.tech.lapse[t.index()]));
            if tier < 2 {
                continue;
            }
            let need = world.config.tech.upkeep_data_at(tier);
            if lapse == 0 && crate::systems::virt::holding(world, c, t) >= need {
                continue;
            }
            let want = cfg.ship_data.get(usize::from(tier) - 1).copied().unwrap_or(0);
            let units = parent(world, id).map_or(0, |p| p.data.get(t)).min(want);
            if units == 0 {
                continue;
            }
            if let Some(p) = parent_mut(world, id) {
                p.data.units[t.index()] -= units;
            }
            if let Some(node) = world.virt.node_mut(n) {
                node.store.units[t.index()] += units;
            }
            world.outside.today.data_shipped += 1;
            let (pname, lname) =
                (world.outside.faction(id).map_or_else(String::new, |f| f.name.clone()), world.name_of(lab));
            world.push_event(EventKind::DataShipped, &[c, lab], format!("{pname} shipped {units} {t} Data to {lname}"));
        }
    }
    sync_uplink(world, id);
}

/// A rival parent: another living megacorp parent whose corp branches
/// share a niche with this one's, the one its branches regard least (M15),
/// and that regard's value.
fn rival_of(world: &World, id: OutsideId) -> Option<(OutsideId, f32)> {
    let mine = corp_branches(world, id);
    let niches: std::collections::BTreeSet<crate::components::Niche> =
        mine.iter().filter_map(|&c| world.comp::<Corp>(c)).flat_map(|c| c.niches.iter().copied()).collect();
    world
        .outside
        .parents()
        .filter(|f| f.id != id && f.kind == OutsideKind::Megacorp && !f.dead)
        .filter_map(|f| {
            let theirs: Vec<EntityId> = corp_branches(world, f.id)
                .into_iter()
                .filter(|&c| world.comp::<Corp>(c).is_some_and(|x| x.niches.iter().any(|n| niches.contains(n))))
                .collect();
            let v = mine
                .iter()
                .flat_map(|&a| theirs.iter().map(move |&b| (a, b)))
                .map(|(a, b)| crate::systems::reputation::regard(world, a, b).value)
                .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |x| x.min(v))))?;
            Some((f.id, v))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}

/// § 3 Campaign's action, weekly from its start: a run on the rival's
/// uplink when a branch Lab has a tier-3 fleet deck and a Researcher;
/// else, when the parent is lawless (< 0.4), a Beat on the rival branch's
/// exec. `CampaignPosted` on an action.
pub fn campaign_step(world: &mut World, id: OutsideId) {
    let now = world.tick;
    let Some(p) = parent(world, id) else { return };
    if now.saturating_sub(p.order_since) % (7 * TICKS_PER_DAY) >= TICKS_PER_DAY {
        return;
    }
    let Some(rival) = p.rival else { return };
    let law = p.persona.lawfulness;
    let Some(target) = uplink_node(world, rival) else { return };
    let mut acted = None;
    for c in corp_branches(world, id) {
        let Some(run) = crate::systems::virt::corp_run(world, c) else { continue };
        let tier = run.deck.and_then(|d| world.comp::<crate::components::Asset>(d)).map_or(0, |a| a.tier);
        if tier < world.config.outside.parents.uplink_link_tier || world.run_orders.contains_key(&run.runner) {
            continue;
        }
        let order = crate::virt::RunOrder {
            patron: Some(c),
            purpose: crate::virt::Purpose::Data { wipe: law < 0.4 },
            target,
            chair: run.lab,
            not_before: now,
            expires: now + TICKS_PER_DAY,
            why: crate::virt::RunWhy::CorpOrder,
            mode: crate::virt::RunMode::Loud,
        };
        crate::systems::virt::give_order(world, run.runner, order);
        acted = Some(format!("a run on {}", crate::systems::virt::node_label(world, target)));
        break;
    }
    if acted.is_none() && law < 0.4 {
        let branch = corp_branches(world, id).into_iter().next();
        let exec = corp_branches(world, rival).into_iter().find_map(|c| world.comp::<Corp>(c).and_then(|x| x.exec));
        if let (Some(b), Some(e)) = (branch, exec.filter(|&e| crate::systems::law::living(world, e))) {
            let t = crate::contract::Target::Agent(e);
            let door = world.comp::<crate::components::Position>(e).map(|x| x.tile).unwrap_or_default();
            let broker = crate::systems::contracts::nearest_fixer_to(world, door);
            let q = crate::systems::contracts::quote(world, crate::contract::ContractKind::Beat, &t, broker);
            let sent = fund(world, id, b, q);
            let bexec = world.comp::<Corp>(b).and_then(|x| x.exec);
            let r = crate::systems::contracts::brain_post_with(
                world,
                Some(b),
                bexec,
                crate::contract::ContractKind::Beat,
                t,
                broker,
                crate::contract::Origin::Parent,
                Some(q),
                crate::contract::PostExtra::default(),
            );
            if r.is_ok() {
                acted = Some(format!("a Beat on {}", world.name_of(e)));
            } else {
                unfund(world, id, b, sent);
            }
        }
    }
    // Phase 5: no run and no Beat: a job abroad on the rival's depot by
    // the parent's own Outsiders.
    if acted.is_none() && crate::systems::abroad::campaign_post(world, id, rival) {
        acted = Some("a job on its depot abroad".to_string());
    }
    if let Some(what) = acted {
        world.outside.today.campaigns += 1;
        let (a, b) = (
            world.outside.faction(id).map_or_else(String::new, |f| f.name.clone()),
            world.outside.faction(rival).map_or_else(String::new, |f| f.name.clone()),
        );
        world.push_event(EventKind::CampaignPosted, &[], format!("{a}'s campaign against {b}: {what}"));
    }
}

/// A save from before phase 4: the parents' edge, Data and tech ceiling
/// from their rows (by name).
pub fn migrate_v24(world: &mut World) {
    if !parents_on(world) {
        return;
    }
    let rows = world.config.outside.parents.factions.clone();
    for f in world.outside.factions.iter_mut() {
        let Some(p) = f.parent.as_mut() else { continue };
        if p.edge > 0.0 {
            continue;
        }
        p.edge = 1.0;
        if let Some(r) = rows.iter().find(|r| r.name == f.name) {
            p.data = crate::virt::DataStore { units: r.data };
            p.tech = r.tech;
        }
    }
    crate::systems::virt::mark_dirty(world);
}
