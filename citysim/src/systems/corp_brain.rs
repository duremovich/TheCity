//! The corp brain (docs/M11_OWNERSHIP.md § 5, plan "The corp brain"). Each
//! corp scores its seven standing orders per niche daily (with hysteresis)
//! and at once when pending shocks add up, takes the best `(order, niche)`,
//! then acts on it daily. Mirrors `faction` (the gangs) and `law_brain`.
//! Nothing here runs per agent per tick: the per-tick work is one flag test.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{
    Building, BuildingKind, Corp, CorpOrder, CorpOrderScore, CorpShock, Gang, Job, Market, Niche, Personality, Role,
    Wallet,
};
use crate::config::CorpsCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::utility::curves::{can, Curve, GATE, SQUARE};
use crate::utility::Consideration;
use crate::world::World;

/// Everything one niche contributes to a corp's scoring.
#[derive(Clone, Debug, PartialEq)]
pub struct NicheInputs {
    /// Food: own Market sales ÷ opening stock over 7 days; Housing: occupancy
    /// of owned Blocks; Security: contracts ÷ `security_guards` (D16).
    pub demand: f32,
    /// This corp's share of the niche (D16).
    pub share: f32,
    pub own_price: f32,
    /// The lowest `price_level` among rivals in the niche (own when none).
    pub rival_price: f32,
    pub rivals: usize,
    /// `(rival corp, its cheapest niche building, that building's value)`
    /// for the weakest rival in the red or under 0.3 cash.
    pub weakest: Option<(EntityId, EntityId, i64)>,
    /// Vacant Lots when the corp can pay `found_cost` for the niche's kind.
    pub lots: usize,
    /// `round(value × acquire_premium)` for `weakest`.
    pub offer: i64,
    /// D32: every owned Block already at the rent cap (Squeeze cannot bite).
    pub at_cap: bool,
}

/// Gathered once per rescoring.
#[derive(Clone, Debug, PartialEq)]
pub struct CorpInputs {
    pub cash: f32,
    pub flow: f32,
    pub losses: f32,
    pub unrest: f32,
    pub greed: f32,
    pub courage: f32,
    pub lawfulness: f32,
    pub treasury: i64,
    pub cooldown_ok: bool,
    pub lobby_ready: bool,
    pub culprit: Option<EntityId>,
    pub niches: BTreeMap<Niche, NicheInputs>,
}

/// The building kinds that make up a niche.
pub fn niche_kinds(n: Niche) -> &'static [BuildingKind] {
    match n {
        Niche::Food => &[BuildingKind::Farm, BuildingKind::Market, BuildingKind::Bar],
        Niche::Housing => &[BuildingKind::Home],
        Niche::Security => &[BuildingKind::SecurityOffice],
    }
}

/// The niche a building kind belongs to.
pub fn niche_of_kind(kind: BuildingKind) -> Option<Niche> {
    Niche::ALL.into_iter().find(|&n| niche_kinds(n).contains(&kind))
}

/// What `Grow` builds in a niche (a second Security Office is never built).
pub fn build_kind(n: Niche) -> Option<BuildingKind> {
    match n {
        Niche::Food => Some(BuildingKind::Bar),
        Niche::Housing => Some(BuildingKind::Home),
        Niche::Security => None,
    }
}

/// The Street class's unrest (§ 7), the `1-unrest` term of Squeeze.
pub fn street_unrest(world: &World) -> f32 {
    crate::systems::classes::street_unrest(world)
}

/// A Security corp's sold contracts: its guard work on its own buildings is
/// no sale (it inflated its share and its demand).
pub fn sold_contracts(world: &World, c: &Corp, corp: EntityId) -> usize {
    c.contracts.iter().filter(|&&(b, _)| world.owner_of(b) != Some(corp)).count()
}

/// Every corp's share of a niche (D16), one pass. A niche with nothing in it
/// (no sales, no occupied Blocks, no contracts) gives everyone 0, and so
/// does one too young or too thin to be a market yet: Food before every
/// Market has its seven days of sales (on day 1 the one Market that sold on
/// day 0 read as a monopoly), Security below `security_guards` contracts
/// (the first contract signed was a monopoly).
pub fn shares(world: &World, niche: Niche) -> BTreeMap<EntityId, f32> {
    let mut own: BTreeMap<EntityId, f32> = BTreeMap::new();
    let mut total = 0.0f32;
    match niche {
        Niche::Food => {
            let markets = world.buildings_of_kind(BuildingKind::Market);
            if markets.iter().any(|&m| world.comp::<Market>(m).is_some_and(|mk| mk.sales.len() < 7)) {
                return BTreeMap::new();
            }
            for &m in markets {
                let sold: u32 = world.comp::<Market>(m).map_or(0, |mk| mk.sales.iter().sum());
                total += sold as f32;
                if let Some(c) = world.corp_of_building(m) {
                    *own.entry(c).or_default() += sold as f32;
                }
            }
        }
        Niche::Housing => {
            for &h in world.buildings_of_kind(BuildingKind::Home) {
                let occupied =
                    world.comp::<Building>(h).is_some_and(|b| !b.demolished) && !world.residents_of(h).is_empty();
                if !occupied {
                    continue;
                }
                total += 1.0;
                if let Some(c) = world.corp_of_building(h) {
                    *own.entry(c).or_default() += 1.0;
                }
            }
        }
        Niche::Security => {
            for c in world.corps() {
                let n = world.comp::<Corp>(c).map_or(0, |cc| sold_contracts(world, cc, c));
                total += n as f32;
                if n > 0 {
                    own.insert(c, n as f32);
                }
            }
            if total < world.config.corps.security_guards as f32 {
                return BTreeMap::new();
            }
        }
    }
    if total <= 0.0 {
        return BTreeMap::new();
    }
    own.into_iter().map(|(c, n)| (c, (n / total).clamp(0.0, 1.0))).collect()
}

/// The treasury as of the last midnight close: upkeep is charged in one
/// lump at midnight while revenue comes in all day, so the live treasury
/// swings by a day's upkeep and a shock rescore at noon would undo the
/// midnight choice. The brain reads the closed books.
pub fn closed_treasury(c: &Corp) -> i64 {
    c.treasury - c.cashflow_today
}

/// `treasury ÷ treasury_ref` at the last close, at most 2.
pub fn cash_of(c: &Corp) -> f32 {
    (closed_treasury(c) as f32 / c.treasury_ref.max(1) as f32).min(2.0)
}

/// Corps (other than `corp`) in a niche, ascending.
pub fn rivals_in(world: &World, corp: EntityId, n: Niche) -> Vec<EntityId> {
    world
        .corps()
        .into_iter()
        .filter(|&c| c != corp && world.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&n)))
        .collect()
}

/// A corp's buildings of a niche, ascending.
pub fn niche_buildings(world: &World, corp: EntityId, n: Niche) -> Vec<EntityId> {
    let kinds = niche_kinds(n);
    world
        .comp::<Corp>(corp)
        .map(|c| {
            c.buildings
                .iter()
                .copied()
                .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && kinds.contains(&bd.kind)))
                .collect()
        })
        .unwrap_or_default()
}

/// The daily wage bill a corp owes: its employees' wages and its exec's.
pub fn wage_bill(world: &World, corp: EntityId) -> i64 {
    let Some(c) = world.comp::<Corp>(corp) else { return 0 };
    let mut bill = if c.exec.is_some() { world.config.economy.wage_exec } else { 0 };
    for role in Role::ALL {
        for &a in world.workers(role) {
            let Some(j) = world.comp::<Job>(a) else { continue };
            if j.employer.is_some_and(|e| c.buildings.binary_search(&e).is_ok()) {
                bill += j.wage_per_day;
            }
        }
    }
    bill
}

/// The gang behind the most loss coins in the 14-day log (ties lower id).
fn culprit(world: &World, c: &Corp) -> Option<EntityId> {
    let horizon = world.tick.saturating_sub(14 * TICKS_PER_DAY);
    let mut by_gang: BTreeMap<EntityId, i64> = BTreeMap::new();
    for l in c.loss_log.iter().filter(|l| l.tick >= horizon) {
        if let Some(g) = l.gang.filter(|&g| world.has::<Gang>(g)) {
            *by_gang.entry(g).or_default() += l.coins.max(1);
        }
    }
    by_gang.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(g, _)| g)
}

/// Gather the inputs, or `None` for a missing corp.
pub fn gather_inputs(world: &World, corp: EntityId) -> Option<CorpInputs> {
    let c = world.comp::<Corp>(corp)?;
    let cfg = &world.config.corps;
    let now = world.tick;
    let bill = wage_bill(world, corp).max(1);
    let mean_flow =
        if c.cashflow.is_empty() { 0.0 } else { c.cashflow.iter().sum::<i64>() as f32 / c.cashflow.len() as f32 };
    let horizon = now.saturating_sub(14 * TICKS_PER_DAY);
    let lost: i64 = c.loss_log.iter().filter(|l| l.tick >= horizon).map(|l| l.coins.max(0)).sum();
    let p = c.exec.and_then(|e| world.comp::<Personality>(e));
    let lots_total = crate::systems::founding::vacant_lots(world).len();
    let cap = usize::from(world.config.buildings.home.capacity).max(1);
    let mut niches = BTreeMap::new();
    for &n in &c.niches {
        let share_map = shares(world, n);
        let share = share_map.get(&corp).copied().unwrap_or(0.0);
        let own_buildings = niche_buildings(world, corp, n);
        let demand = match n {
            Niche::Food => {
                let (mut sold, mut stock) = (0u32, 0u32);
                for &b in &own_buildings {
                    if let Some(m) = world.comp::<Market>(b) {
                        sold += m.sales.iter().sum::<u32>();
                        stock += m.stock_hist.iter().sum::<u32>();
                    }
                }
                sold as f32 / stock.max(1) as f32
            }
            Niche::Housing => {
                let residents: usize = own_buildings.iter().map(|&h| world.residents_of(h).len()).sum();
                residents as f32 / (own_buildings.len() * cap).max(1) as f32
            }
            Niche::Security => sold_contracts(world, c, corp) as f32 / cfg.security_guards.max(1) as f32,
        }
        .clamp(0.0, 1.0);
        let rivals = rivals_in(world, corp, n);
        let own_price = c.level(n);
        let rival_price = rivals
            .iter()
            .filter_map(|&r| world.comp::<Corp>(r).map(|rc| rc.level(n)))
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(own_price);
        // The weakest rival: the lowest cash among those in the red or under
        // 0.3 cash with a spare niche building for sale (ties lower id). A
        // hostile bid takes a branch, never a rival's last building in the
        // niche: that goes only in bankruptcy (Arasaka bought Militech's only
        // Security Office on day 35 and held a monopoly from then on).
        let mut weak: Vec<(f32, EntityId)> = rivals
            .iter()
            .filter_map(|&r| world.comp::<Corp>(r).map(|rc| (cash_of(rc), rc.negative_since.is_some(), r)))
            .filter(|&(cash, neg, _)| neg || cash < 0.3)
            .map(|(cash, _, r)| (cash, r))
            .collect();
        weak.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let weakest = weak.into_iter().find_map(|(_, r)| {
            let theirs = niche_buildings(world, r, n);
            if theirs.len() < 2 {
                return None;
            }
            theirs
                .into_iter()
                .filter_map(|b| {
                    let v = world.comp::<Building>(b).map_or(0, |bd| ownership::value(world, bd.kind));
                    (v > 0).then_some((v, b))
                })
                .min()
                .map(|(v, b)| (r, b, v))
        });
        let offer = weakest.map_or(0, |(_, _, v)| (v as f32 * cfg.acquire_premium).round() as i64);
        let lots = match build_kind(n).and_then(|k| crate::systems::founding::found_cost(world, k)) {
            Some(cost) if c.treasury >= cost => lots_total,
            _ => 0,
        };
        let at_cap = n == Niche::Housing
            && world.levers.rent_cap.is_some_and(|cap| {
                !own_buildings.is_empty()
                    && own_buildings.iter().all(|&h| {
                        let tier = world.comp::<Building>(h).map_or(0, |b| usize::from(b.tier.min(2)));
                        (own_price * world.config.rent.base[tier] as f32).round() as i64 >= cap
                    })
            });
        niches.insert(
            n,
            NicheInputs { demand, share, own_price, rival_price, rivals: rivals.len(), weakest, lots, offer, at_cap },
        );
    }
    let acquire_cd = cfg.acquire_cooldown_days * TICKS_PER_DAY;
    Some(CorpInputs {
        cash: cash_of(c),
        flow: (mean_flow / bill as f32).clamp(-1.0, 1.0),
        losses: (lost as f32 / closed_treasury(c).max(1) as f32).clamp(0.0, 1.0),
        unrest: street_unrest(world),
        greed: p.map_or(0.5, |p| p.greed),
        courage: p.map_or(0.5, |p| p.courage),
        lawfulness: p.map_or(0.5, |p| p.lawfulness),
        treasury: c.treasury,
        cooldown_ok: c.last_acquisition_tick.is_none_or(|t| now.saturating_sub(t) >= acquire_cd),
        lobby_ready: c.treasury >= cfg.lobby_min_treasury && c.lobby_until.is_none_or(|t| t <= now),
        culprit: culprit(world, c),
        niches,
    })
}

/// Product of the outputs plus a flat term; `None` when a gate is shut.
fn score(order: CorpOrder, niche: Niche, cs: Vec<Consideration>, flat: f32) -> Option<CorpOrderScore> {
    if cs.iter().any(|c| c.output <= 0.0) {
        return None;
    }
    let raw: f32 = cs.iter().map(|c| c.output).product();
    Some(CorpOrderScore { order, niche, score: raw + flat, considerations: cs })
}

/// The four niche orders (Grow, Squeeze, Undercut, Acquire) for one niche.
pub fn score_niche(i: &CorpInputs, n: Niche, cfg: &CorpsCfg) -> Vec<CorpOrderScore> {
    let f = &cfg.order_flat;
    let Some(ni) = i.niches.get(&n) else { return Vec::new() };
    let squeeze_share = if ni.at_cap { 0.0 } else { ni.share };
    [
        score(
            CorpOrder::Grow,
            n,
            vec![
                Consideration::new(
                    "lots & cash",
                    can(ni.lots > 0 && i.cash >= 0.5 && (n != Niche::Housing || ni.demand >= cfg.grow_min_occupancy)),
                    GATE,
                ),
                Consideration::new("demand", ni.demand, Curve::Linear { m: 0.8, b: 0.2 }),
                Consideration::new("flow", i.flow, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("greed", i.greed, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.grow,
        ),
        score(
            CorpOrder::Squeeze,
            n,
            vec![
                Consideration::new("share", squeeze_share, Curve::Logistic { k: 8.0, mid: 0.4 }),
                Consideration::new("1-flow", 1.0 - i.flow, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("greed", i.greed, SQUARE),
                Consideration::new("1-unrest", 1.0 - i.unrest, Curve::Linear { m: 0.6, b: 0.4 }),
            ],
            f.squeeze,
        ),
        score(
            CorpOrder::Undercut,
            n,
            vec![
                Consideration::new("rivals & cash", can(ni.rivals > 0 && i.cash >= 0.4), GATE),
                Consideration::new("1-share", 1.0 - ni.share, Curve::Linear { m: 0.8, b: 0.2 }),
                Consideration::new(
                    "rival price gap",
                    (ni.rival_price - ni.own_price).clamp(0.0, 1.0),
                    Curve::Linear { m: 0.5, b: 0.5 },
                ),
                Consideration::new("flow", i.flow, Curve::Linear { m: 0.4, b: 0.6 }),
            ],
            f.undercut,
        ),
        score(
            CorpOrder::Acquire,
            n,
            vec![
                Consideration::new(
                    "weak rival",
                    can(ni.weakest.is_some() && i.treasury >= ni.offer && i.cooldown_ok),
                    GATE,
                ),
                Consideration::new("cash", i.cash, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("greed", i.greed, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-share", 1.0 - ni.share, Curve::Linear { m: 0.4, b: 0.6 }),
            ],
            f.acquire,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The corp-wide orders (Secure, Hunker, Lobby). They score the same in every
/// niche, so they are scored once, in the first (D47: ties resolve to it).
fn score_corp_wide(i: &CorpInputs, n: Niche, cfg: &CorpsCfg) -> Vec<CorpOrderScore> {
    let f = &cfg.order_flat;
    [
        score(
            CorpOrder::Secure,
            n,
            vec![
                Consideration::new("losses", i.losses, Curve::Logistic { k: 8.0, mid: 0.1 }),
                Consideration::new("cash", i.cash, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.3, b: 0.7 }),
            ],
            f.secure,
        ),
        score(
            CorpOrder::Hunker,
            n,
            vec![
                Consideration::new("1-cash", 1.0 - i.cash, Curve::Logistic { k: 6.0, mid: 0.6 }),
                Consideration::new("1-flow", 1.0 - i.flow, Curve::Linear { m: 0.6, b: 0.4 }),
            ],
            f.hunker,
        ),
        score(
            CorpOrder::Lobby,
            n,
            vec![
                Consideration::new("lobby ready", can(i.lobby_ready && i.culprit.is_some()), GATE),
                Consideration::new("losses", i.losses, Curve::Linear { m: 0.9, b: 0.1 }),
                Consideration::new("1-lawfulness", 1.0 - i.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
            ],
            f.lobby,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Every `(order, niche)` score, best first (ties: order, then niche).
pub fn score_all(i: &CorpInputs, cfg: &CorpsCfg) -> Vec<CorpOrderScore> {
    let mut out: Vec<CorpOrderScore> = i.niches.keys().flat_map(|&n| score_niche(i, n, cfg)).collect();
    if let Some(&first) = i.niches.keys().next() {
        out.extend(score_corp_wide(i, first, cfg));
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.order.cmp(&b.order)).then(a.niche.cmp(&b.niche)));
    out
}

/// Secure, Hunker and Lobby are scored once for the whole corp.
fn corp_wide(order: CorpOrder) -> bool {
    matches!(order, CorpOrder::Secure | CorpOrder::Hunker | CorpOrder::Lobby)
}

/// Does a score row stand for the order `current`? An unset niche matches
/// any, and so does a corp-wide order (it is scored in whichever niche is
/// first, which moves when the corp's niches change: the old niche scored
/// 0 and logged a spurious switch).
fn matches(s: &CorpOrderScore, current: (CorpOrder, Option<Niche>)) -> bool {
    s.order == current.0 && (corp_wide(s.order) || current.1.is_none_or(|n| n == s.niche))
}

/// The score of the standing `(order, niche)`.
fn current_score(scores: &[CorpOrderScore], current: (CorpOrder, Option<Niche>)) -> f32 {
    scores.iter().find(|s| matches(s, current)).map_or(0.0, |s| s.score)
}

/// The pair to switch to, if the best beats the standing one by `hysteresis`.
/// An unavailable standing order scores zero, so anything replaces it.
pub fn choose(
    scores: &[CorpOrderScore],
    current: (CorpOrder, Option<Niche>),
    hysteresis: f32,
) -> Option<(CorpOrder, Niche)> {
    let best = scores.first()?;
    let same = matches(best, current);
    (!same && best.score > current_score(scores, current) + hysteresis).then_some((best.order, best.niche))
}

/// Queue a shock (the per-tick check rescores once they add up).
pub fn push_shock(world: &mut World, corp: EntityId, shock: CorpShock) {
    ownership::push_corp_shock(world, corp, shock);
}

/// Score and switch when the best clears `hysteresis`. D22: entering
/// Squeeze cuts Housing's eviction days or Food's wages; leaving restores
/// them. Logs `CorpOrder` with both orders, the niche and the reason.
pub fn rescore(world: &mut World, corp: EntityId, hysteresis: f32, why: &str) {
    let Some(inputs) = gather_inputs(world, corp) else { return };
    let cfg = world.config.corps.clone();
    let scores = score_all(&inputs, &cfg);
    let Some((current, cur_niche, name)) = world.comp::<Corp>(corp).map(|c| (c.order, c.order_niche, c.name.clone()))
    else {
        return;
    };
    let next = choose(&scores, (current, cur_niche), hysteresis);
    let best = scores.first().map_or(0.0, |s| s.score);
    let cur = current_score(&scores, (current, cur_niche));
    let now = world.tick;
    let evict_days = world.config.rent.evict_days;
    let Some(c) = world.comp_mut::<Corp>(corp) else { return };
    c.order_trace = scores;
    // A god pin (`PlayerCommand::SetCorpOrder`) holds the order.
    if c.pinned_until.is_some_and(|t| now < t) {
        return;
    }
    let Some((order, niche)) = next else { return };
    if current == CorpOrder::Squeeze {
        c.wage_mult = 1.0;
        c.evict_days_override = None;
    }
    if order == CorpOrder::Squeeze {
        match niche {
            Niche::Housing => c.evict_days_override = Some(evict_days.saturating_sub(1).max(2)),
            Niche::Food => c.wage_mult = 0.9,
            Niche::Security => {}
        }
    }
    c.order = order;
    c.order_niche = Some(niche);
    c.order_since = now;
    let old = match cur_niche {
        Some(n) => format!("{current} in {n}"),
        None => current.to_string(),
    };
    world.push_event(
        EventKind::CorpOrder,
        &[corp],
        format!("{name}: {old} -> {order} in {niche} ({why}, {best:.2} vs {cur:.2})"),
    );
    // Lobby is one payment: it is made when the order is taken (a shock
    // rescore at noon would otherwise lapse at midnight before it acted).
    if order == CorpOrder::Lobby && why != "daily" {
        act(world, corp);
    }
}

/// An immediate rescoring with no hysteresis; the pending shocks are consumed.
pub fn rethink(world: &mut World, corp: EntityId) {
    rescore(world, corp, 0.0, "shock");
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.shocks.clear();
    }
}

/// Round a price level to two decimals (0.1 steps stay exact in the CSV).
fn tidy(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}

fn set_level(world: &mut World, corp: EntityId, n: Niche, level: f32) {
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.price_level.insert(n, tidy(level));
    }
}

/// D17: top every owned niche building's vacancies up to full staff.
fn staff_up(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let buildings = c.buildings.clone();
    let mut employed: BTreeMap<EntityId, usize> = BTreeMap::new();
    for role in Role::ALL {
        for &a in world.workers(role) {
            if let Some(e) = world.comp::<Job>(a).and_then(|j| j.employer) {
                if buildings.binary_search(&e).is_ok() {
                    *employed.entry(e).or_default() += 1;
                }
            }
        }
    }
    for b in buildings {
        let Some(kind) = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.kind) else { continue };
        let Some(role) = ownership::role_for(kind) else { continue };
        let staff = full_staff(world, kind);
        let open = world.vacancies.get(&b).map_or(0, |v| v.len());
        let have = employed.get(&b).copied().unwrap_or(0) + open;
        if have < staff {
            world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, staff - have));
        }
    }
}

/// A niche building's full staff (D17): `[buildings] <kind>.staff`, a
/// Security Office's `[corps] security_guards`.
pub fn full_staff(world: &World, kind: BuildingKind) -> usize {
    if kind == BuildingKind::SecurityOffice {
        world.config.corps.security_guards as usize
    } else {
        world.config.buildings.for_kind(kind).staff as usize
    }
}

/// D17 Hunker: no open vacancies beyond half staff (every hunkering corp
/// closes them at once). A building with fewer than half its staff keeps
/// the vacancies that bring it back to half: Hunker is the quiet default,
/// and closing every vacancy left a Farm whose Vat Techs died or were
/// jailed unworked for good.
fn hunker_vacancies(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let buildings = c.buildings.clone();
    let mut employed: BTreeMap<EntityId, usize> = BTreeMap::new();
    for role in Role::ALL {
        for &a in world.workers(role) {
            if let Some(e) = world.comp::<Job>(a).and_then(|j| j.employer) {
                if buildings.binary_search(&e).is_ok() {
                    *employed.entry(e).or_default() += 1;
                }
            }
        }
    }
    for b in buildings {
        let Some(open) = world.vacancies.get(&b).map(|v| v.len()) else { continue };
        let kind = world.comp::<Building>(b).map(|bd| bd.kind);
        let half = kind.map_or(0, |k| full_staff(world, k).div_ceil(2));
        let keep = half.saturating_sub(employed.get(&b).copied().unwrap_or(0)).min(open);
        if keep == 0 {
            world.vacancies.remove(&b);
        } else if let Some(v) = world.vacancies.get_mut(&b) {
            v.truncate(keep);
        }
    }
}

/// D17 Hunker layoffs: one firing per building per day, the newest hire
/// first (ties higher id), while above half staff; never at a Farm (see
/// below).
fn hunker_staff(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let buildings = c.buildings.clone();
    let mut staff: BTreeMap<EntityId, Vec<(crate::time::Tick, EntityId)>> = BTreeMap::new();
    for role in Role::ALL {
        for &a in world.workers(role) {
            if let Some(j) = world.comp::<Job>(a) {
                if let Some(e) = j.employer.filter(|e| buildings.binary_search(e).is_ok()) {
                    staff.entry(e).or_default().push((j.hired_tick, a));
                }
            }
        }
    }
    for (b, mut list) in staff {
        let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { continue };
        // Vat Techs are the city's food: a Food corp that laid them off in
        // Winter (one per Farm a day) took employment from 213 to 138 and the
        // price to 7 on seed 42. Hunker cuts every other staff.
        if kind == BuildingKind::Farm && world.config.corps.hunker_spares_farms {
            continue;
        }
        let full = full_staff(world, kind);
        if full == 0 || list.len() <= full / 2 {
            continue;
        }
        list.sort();
        let Some(&(_, who)) = list.last() else { continue };
        let role = world.comp::<Job>(who).map(|j| j.role);
        world.abort_plan(who);
        world.remove::<Job>(who);
        let name = world.name_of(who);
        let what = role.map_or("worker", |r| r.label());
        let cname = world.owner_label(Some(corp));
        world.push_event(EventKind::Fire, &[who, b], format!("{name} laid off as {what} by {cname} (hunkering)"));
    }
}

/// The vacant Lot nearest any owned building (door to door, ties lower id).
fn lot_near(world: &World, corp: EntityId) -> Option<EntityId> {
    let doors: Vec<crate::components::TilePos> = world
        .comp::<Corp>(corp)?
        .buildings
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).map(|bd| bd.door))
        .collect();
    crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter_map(|l| {
            let d = world.comp::<Building>(l)?.door;
            doors.iter().map(|&o| o.manhattan(d)).min().map(|dist| (dist, l))
        })
        .min()
        .map(|(_, l)| l)
}

fn grow(world: &mut World, corp: EntityId, n: Niche, i: &CorpInputs) {
    let now = world.tick;
    let cd = world.config.corps.grow_cooldown_days * TICKS_PER_DAY;
    let ready = world.comp::<Corp>(corp).is_some_and(|c| c.last_build_tick.is_none_or(|t| now.saturating_sub(t) >= cd));
    let kind = build_kind(n);
    let cost = kind.and_then(|k| crate::systems::founding::found_cost(world, k));
    let lots = i.niches.get(&n).map_or(0, |ni| ni.lots);
    if let (true, Some(kind), Some(cost), true) = (ready, kind, cost, lots > 0) {
        if world.purse(Some(corp)) >= cost {
            if let Some(lot) = lot_near(world, corp) {
                // Paid once the building stands (a failed build cost nothing).
                if crate::systems::founding::build_on_lot(world, lot, kind, Some(corp)).is_ok() {
                    ownership::pay(world, Some(corp), None, cost, Flow::Found);
                    if let Some(c) = world.comp_mut::<Corp>(corp) {
                        c.last_build_tick = Some(now);
                    }
                    let (cname, what) = (world.owner_label(Some(corp)), world.name_of(lot));
                    world.push_event(
                        EventKind::Founded,
                        &[corp, lot],
                        format!("{cname} built {what} on a Lot for {cost} (growing in {n})"),
                    );
                }
            }
        }
    }
    staff_up(world, corp);
}

fn squeeze(world: &mut World, corp: EntityId, n: Niche) {
    let cfg = &world.config.corps;
    let cap =
        if crate::systems::corps::is_monopoly(world, corp, n) { cfg.monopoly_markup_cap } else { cfg.squeeze_cap };
    let level = world.comp::<Corp>(corp).map_or(1.0, |c| c.level(n));
    set_level(world, corp, n, (level + 0.1).min(cap));
}

fn undercut(world: &mut World, corp: EntityId, n: Niche, i: &CorpInputs) {
    let floor = world.config.corps.undercut_floor;
    let level = world.comp::<Corp>(corp).map_or(1.0, |c| c.level(n));
    let rival = i.niches.get(&n).map_or(level, |ni| ni.rival_price);
    let target = floor.max(rival - 0.1);
    if level > target {
        set_level(world, corp, n, (level - 0.1).max(target));
    }
}

fn acquire(world: &mut World, corp: EntityId, n: Niche, i: &CorpInputs) {
    let Some(ni) = i.niches.get(&n) else { return };
    let Some((seller, building, _)) = ni.weakest else { return };
    if !(i.cooldown_ok && world.purse(Some(corp)) >= ni.offer) {
        return;
    }
    if crate::systems::corps::acquire(world, corp, building, ni.offer, "hostile") {
        let now = world.tick;
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.last_acquisition_tick = Some(now);
        }
        push_shock(world, seller, CorpShock::BuildingLost);
    }
}

/// D24: contract up to `secure_per_day` owned buildings with a recent loss,
/// newest loss first; a Security corp covers its own, the rest buy from the
/// cheapest Security corp with room.
fn secure(world: &mut World, corp: EntityId) {
    let per_day = world.config.corps.secure_per_day;
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let horizon = world.tick.saturating_sub(14 * TICKS_PER_DAY);
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    for l in c.loss_log.iter().rev().filter(|l| l.tick >= horizon) {
        let Some(b) = l.building else { continue };
        if !seen.insert(b) {
            continue;
        }
        let ok = world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(corp) && !bd.demolished)
            && world.comp::<Building>(b).is_some_and(|bd| bd.secured_by.is_none());
        if ok {
            targets.push(b);
        }
        if targets.len() >= per_day {
            break;
        }
    }
    let own_security = c.niches.contains(&Niche::Security);
    for b in targets {
        let seller = if own_security { Some(corp) } else { crate::systems::corps::cheapest_seller(world, Some(corp)) };
        if let Some(s) = seller {
            crate::systems::corps::buy_contract(world, b, s);
        }
    }
}

fn hunker(world: &mut World, corp: EntityId, i: &CorpInputs) {
    if let Some(c) = world.comp::<Corp>(corp) {
        let levels: Vec<(Niche, f32)> = c.price_level.iter().map(|(&n, &l)| (n, l)).collect();
        for (n, l) in levels {
            let next = if l > 1.0 { (l - 0.1).max(1.0) } else { (l + 0.1).min(1.0) };
            set_level(world, corp, n, next);
        }
    }
    // Cutting staff is for a corp losing money over a week at least: Hunker
    // is also the quiet default (flat 0.15) and the seed order, and a corp
    // that laid off a Vat Tech a day from day 0 starved the city by Winter.
    // Open vacancies close at once (item 13: the gate had hidden that).
    hunker_vacancies(world, corp);
    let evidence = world.comp::<Corp>(corp).map_or(0, |c| c.cashflow.len());
    let losing = i.flow < 0.0 && evidence >= 7;
    if losing {
        hunker_staff(world, corp);
    }
    // Guard contracts are cut by the same evidence (phase 5): the quiet
    // default cancelled every contract a Secure had bought within days, so
    // the Security corps never kept a client and both bled out.
    if !losing {
        return;
    }
    let bought: Vec<EntityId> = world
        .comp::<Corp>(corp)
        .map(|c| {
            c.buildings
                .iter()
                .copied()
                .filter(|&b| world.comp::<Building>(b).and_then(|bd| bd.secured_by).is_some_and(|s| s != corp))
                .collect()
        })
        .unwrap_or_default();
    for b in bought {
        crate::systems::corps::end_contract(world, b, "cancelled: hunkering");
    }
}

fn lobby(world: &mut World, corp: EntityId, i: &CorpInputs) {
    let Some(gang) = i.culprit else { return };
    if !i.lobby_ready {
        return;
    }
    let score = world
        .comp::<Corp>(corp)
        .and_then(|c| c.order_trace.iter().find(|s| s.order == CorpOrder::Lobby).map(|s| s.score))
        .unwrap_or(0.0);
    crate::systems::faction::offer_bribe(
        world,
        crate::systems::faction::Payer::Corp(corp),
        crate::systems::faction::BribeAsk::Crackdown(gang),
        score,
    );
}

/// The standing order's effect, daily after the rescore.
pub fn act(world: &mut World, corp: EntityId) {
    let Some((order, niche)) = world.comp::<Corp>(corp).map(|c| (c.order, c.order_niche)) else { return };
    let Some(i) = gather_inputs(world, corp) else { return };
    let n = niche.or_else(|| i.niches.keys().next().copied());
    match (order, n) {
        (CorpOrder::Grow, Some(n)) => grow(world, corp, n, &i),
        (CorpOrder::Squeeze, Some(n)) => squeeze(world, corp, n),
        (CorpOrder::Undercut, Some(n)) => undercut(world, corp, n, &i),
        (CorpOrder::Acquire, Some(n)) => acquire(world, corp, n, &i),
        (CorpOrder::Secure, _) => secure(world, corp),
        (CorpOrder::Hunker, _) => hunker(world, corp, &i),
        (CorpOrder::Lobby, _) => lobby(world, corp, &i),
        _ => {}
    }
}

/// Per tick: a corp whose pending shocks reached the threshold rescores at
/// once (one flag test otherwise). At midnight, after the law and the gangs:
/// every corp rescores with hysteresis, then each acts, then `corps::daily`
/// (contracts, shares, bankruptcy, monopolies).
pub fn run(world: &mut World) {
    if world.tick_of_day() == 0 {
        world.corp_rethink = false;
        let corps = world.corps();
        if corps.is_empty() {
            return;
        }
        let h = world.config.corps.hysteresis;
        for &c in &corps {
            rescore(world, c, h, "daily");
            if let Some(cc) = world.comp_mut::<Corp>(c) {
                cc.shocks.clear();
            }
        }
        for &c in &corps {
            act(world, c);
        }
        crate::systems::corps::daily(world);
        return;
    }
    if !world.corp_rethink {
        return;
    }
    world.corp_rethink = false;
    let threshold = world.config.corps.shock_severity_rethink;
    for c in world.corps() {
        let pending: f32 = world.comp::<Corp>(c).map_or(0.0, |cc| cc.shocks.iter().map(|s| s.severity()).sum());
        if pending >= threshold {
            rethink(world, c);
        }
    }
}

/// Living agents' wallets (bankruptcy buyers).
pub(crate) fn wallets(world: &World) -> Vec<(i64, EntityId)> {
    world
        // scan-ok: rare: a bankruptcy
        .citizens()
        .into_iter()
        .filter(|&a| world.has::<crate::components::Brain>(a))
        .filter_map(|a| world.comp::<Wallet>(a).map(|w| (w.coins, a)))
        .collect()
}
