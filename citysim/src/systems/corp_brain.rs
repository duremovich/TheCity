//! The corp brain (docs/M11_OWNERSHIP.md § 5, plan "The corp brain"). Each
//! corp scores its seven standing orders per niche daily (with hysteresis)
//! and at once when pending shocks add up, takes the best `(order, niche)`,
//! then acts on it daily. Mirrors `faction` (the gangs) and `law_brain`.
//! Nothing here runs per agent per tick: the per-tick work is one flag test.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{
    Building, BuildingKind, Corp, CorpOrder, CorpOrderScore, CorpShock, Gang, Job, Market, Niche, Personality, Wallet,
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
    /// M14 V31 (default: the plane off, no Research row).
    pub virt: VirtInputs,
}

/// M14 V31: the Research order's inputs, gathered only with the plane on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VirtInputs {
    /// `[virt] enabled`: without it Research is not scored at all.
    pub enabled: bool,
    /// `(max rival tier - own tier in focus) / 2`, rivals sharing a niche.
    pub tech_gap: f32,
    /// The largest `lapse / decay_days`.
    pub max_lapse: f32,
    /// M14 phase 5: Research's gap (`tech::research_gap`, the street tier
    /// with `[tech] research_gap_street`) and the floor under its lapse term.
    pub research_gap: f32,
    pub lapse_floor: f32,
    /// M14 phase 5: `[tech] research_min_days` of cashflow on the books.
    pub research_ready: bool,
    pub has_lab: bool,
    /// Vacant Lots while the treasury holds `found_cost.lab`.
    pub lab_lots: usize,
    /// Phase 3: the corp has a run for today (`virt::corp_run`): a Lab with
    /// a Researcher and a niche rival's Lab holding Data in focus. The Lab
    /// need not hold a fleet deck yet: the VirtRaid act buys one (tier
    /// `RAID_DECK_TIER`, above the fleet reserve) before the order goes out.
    pub run_ready: bool,
    /// Phase 3: `p_success` of the day's run against the niche rival's Lab
    /// (0 below `min_route_p`).
    pub virt_p: f32,
    /// Phase 3: the share of the first niche (VirtRaid's `1 - share`).
    pub share: f32,
}

/// The building kinds that make up a niche.
pub fn niche_kinds(n: Niche) -> &'static [BuildingKind] {
    match n {
        Niche::Food => &[BuildingKind::Farm, BuildingKind::Market, BuildingKind::Bar],
        Niche::Housing => &[BuildingKind::Home],
        Niche::Security => &[BuildingKind::SecurityOffice],
        // M13 D17.
        Niche::Tech => &[BuildingKind::Clinic, BuildingKind::Garage],
    }
}

/// The niche a building kind belongs to.
pub fn niche_of_kind(kind: BuildingKind) -> Option<Niche> {
    Niche::ALL.into_iter().find(|&n| niche_kinds(n).contains(&kind))
}

/// What `Grow` builds in a niche (a second Security Office is never built).
/// M13 D17: Tech builds a Garage (phase 2: the only Tech kind `found_cost`
/// prices; phase 3 picks the kind with fewer per capita).
pub fn build_kind(n: Niche) -> Option<BuildingKind> {
    match n {
        Niche::Food => Some(BuildingKind::Bar),
        Niche::Housing => Some(BuildingKind::Home),
        Niche::Security => None,
        Niche::Tech => Some(BuildingKind::Garage),
    }
}

/// Standing buildings of `kind`, and the per-capita target
/// `population ÷ residents_per_<kind>` (`founding::choose_kind`'s).
fn tech_count_target(world: &World, kind: BuildingKind) -> (f32, f32) {
    let per = match kind {
        BuildingKind::Clinic => world.config.corps.residents_per_clinic,
        _ => world.config.corps.residents_per_garage,
    };
    let target = crate::systems::founding::living(world) as f32 / per.max(1) as f32;
    let count = world
        .buildings_of_kind(kind)
        .iter()
        .filter(|&&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .count();
    (count as f32, target)
}

/// M13 D17/D44 (phase 3): what a Tech corp builds: the kind with fewer per
/// capita (`count ÷ target`; ties to the Garage), each only while
/// foundable.
pub fn tech_build_kind(world: &World) -> BuildingKind {
    let ratio = |k: BuildingKind| {
        let (count, target) = tech_count_target(world, k);
        count / target.max(1e-3)
    };
    let clinic_ok = crate::systems::founding::found_cost(world, BuildingKind::Clinic).is_some();
    let garage_ok = crate::systems::founding::found_cost(world, BuildingKind::Garage).is_some();
    match (clinic_ok, garage_ok) {
        (true, true) if ratio(BuildingKind::Clinic) < ratio(BuildingKind::Garage) => BuildingKind::Clinic,
        (true, false) => BuildingKind::Clinic,
        _ => BuildingKind::Garage,
    }
}

/// The kind a corp grows a niche with (Tech: [`tech_build_kind`]).
pub fn build_kind_in(world: &World, n: Niche) -> Option<BuildingKind> {
    match n {
        Niche::Tech => Some(tech_build_kind(world)),
        _ => build_kind(n),
    }
}

/// M13 D17: the city has fewer of the kind Tech would build than
/// `population ÷ residents_per_<kind>` (the target `founding::choose_kind`
/// reads).
pub fn tech_room(world: &World) -> bool {
    let (count, target) = tech_count_target(world, tech_build_kind(world));
    count < target
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
        // M13 D42: robots sold and in service count as contracts do.
        Niche::Security => {
            for c in world.corps() {
                let n = world.comp::<Corp>(c).map_or(0, |cc| sold_contracts(world, cc, c))
                    + crate::systems::robots::sold_in_service(world, c);
                total += n as f32;
                if n > 0 {
                    own.insert(c, n as f32);
                }
            }
            if total < world.config.corps.security_guards as f32 {
                return BTreeMap::new();
            }
        }
        // M13 D17: asset sales over 7 days, per building (empty until every
        // Tech building has its 7 days of history).
        Niche::Tech => {
            let tech: Vec<EntityId> = niche_kinds(Niche::Tech)
                .iter()
                .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
                .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
                .collect();
            if tech.is_empty()
                || tech.iter().any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.asset_sales.len() < 7))
            {
                return BTreeMap::new();
            }
            for b in tech {
                let sold: u32 =
                    world.comp::<Building>(b).map_or(0, |bd| bd.asset_sales.iter().map(|&s| u32::from(s)).sum());
                total += sold as f32;
                if let Some(c) = world.corp_of_building(b) {
                    *own.entry(c).or_default() += sold as f32;
                }
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
    let exec = if c.exec.is_some() { world.config.economy.wage_exec } else { 0 };
    exec + ownership::employees_of(world, corp)
        .into_iter()
        .filter_map(|a| world.comp::<Job>(a).map(|j| j.wage_per_day))
        .sum::<i64>()
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
            // M13 D17: own 7-day sales ÷ (7 × tech_demand_ref × own Tech buildings).
            Niche::Tech => {
                let sold: u32 = own_buildings
                    .iter()
                    .filter_map(|&b| world.comp::<Building>(b))
                    .map(|bd| bd.asset_sales.iter().map(|&s| u32::from(s)).sum::<u32>())
                    .sum();
                let reference = 7.0 * world.config.shop.tech_demand_ref.max(1e-3) * own_buildings.len().max(1) as f32;
                sold as f32 / reference
            }
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
        let lots = match build_kind_in(world, n).and_then(|k| crate::systems::founding::found_cost(world, k)) {
            // M13 D17 (phase 2): Tech grows only while its kind is under the
            // per-capita target `choose_kind` uses (`population ÷
            // residents_per_garage`); without it Zetatech built two idle
            // Garages in the first month on the corps' fleet orders.
            Some(_) if n == Niche::Tech && !tech_room(world) => 0,
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
    let virt = if world.config.virt.enabled {
        let (tech_gap, max_lapse) = crate::systems::tech::research_inputs(world, corp);
        let lab_cost = cfg.found_cost.lab;
        let run = crate::systems::virt::corp_run(world, corp);
        VirtInputs {
            enabled: true,
            tech_gap,
            max_lapse,
            research_gap: crate::systems::tech::research_gap(world, corp),
            lapse_floor: world.config.tech.research_lapse_floor.clamp(0.0, 1.0),
            research_ready: c.cashflow.len() >= world.config.tech.research_min_days as usize,
            has_lab: !crate::systems::tech::labs_of(world, corp).is_empty(),
            lab_lots: if lab_cost > 0 && c.treasury >= lab_cost { lots_total } else { 0 },
            run_ready: run.is_some(),
            virt_p: run.as_ref().map(|r| r.p).filter(|&p| p >= world.config.virt.min_route_p).unwrap_or(0.0),
            share: niches.values().next().map_or(0.0, |n: &NicheInputs| n.share),
        }
    } else {
        VirtInputs::default()
    };
    Some(CorpInputs {
        cash: cash_of(c),
        flow: (mean_flow / bill as f32).clamp(-1.0, 1.0),
        losses: (lost as f32 / closed_treasury(c).max(1) as f32)
            .clamp(0.0, 1.0)
            .max(at_risk_share(world, corp) * world.config.law.private_fill_weight)
            .max(raided_floor(world, c)),
        unrest: street_unrest(world),
        greed: p.map_or(0.5, |p| p.greed),
        courage: p.map_or(0.5, |p| p.courage),
        lawfulness: p.map_or(0.5, |p| p.lawfulness),
        treasury: c.treasury,
        cooldown_ok: c.last_acquisition_tick.is_none_or(|t| now.saturating_sub(t) >= acquire_cd),
        lobby_ready: c.treasury >= cfg.lobby_min_treasury && c.lobby_until.is_none_or(|t| t <= now),
        culprit: culprit(world, c),
        niches,
        virt,
    })
}

/// M12 fix pass: a corp raided (or looted by a riot) within `[corps]
/// raided_days` reads `losses` of at least `raided_losses`, so the shock
/// lands on Secure (or Lobby against the gang) whatever the coins were
/// against its treasury.
fn raided_floor(world: &World, c: &Corp) -> f32 {
    let cfg = &world.config.corps;
    match c.raided_at {
        Some(t) if world.tick.saturating_sub(t) < cfg.raided_days * TICKS_PER_DAY => cfg.raided_losses,
        _ => 0.0,
    }
}

/// M12 D14: a building is at risk where the law is thin: its district's
/// coverage is below `[law] private_fill_coverage` (only with `[law]
/// district_beats`; a district without Homes reads `coverage_max`).
pub fn thinly_covered(world: &World, b: EntityId) -> bool {
    world.config.law.district_beats
        && world.district(world.district_of_building(b)).coverage < world.config.law.private_fill_coverage
}

/// M12 D14: the share of the corp's standing buildings at risk (0 with
/// district beats off), the Secure input `losses` floors at it times
/// `private_fill_weight`.
pub fn at_risk_share(world: &World, corp: EntityId) -> f32 {
    if !world.config.law.district_beats {
        return 0.0;
    }
    let Some(c) = world.comp::<Corp>(corp) else { return 0.0 };
    let standing: Vec<EntityId> =
        c.buildings.iter().copied().filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished)).collect();
    if standing.is_empty() {
        return 0.0;
    }
    standing.iter().filter(|&&b| thinly_covered(world, b)).count() as f32 / standing.len() as f32
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
        // M14 V31: only with the plane on (no row at all with it off).
        i.virt
            .enabled
            .then(|| {
                score(
                    CorpOrder::Research,
                    n,
                    vec![
                        Consideration::new(
                            "lab & cash",
                            can((i.virt.has_lab || i.virt.lab_lots > 0) && i.cash >= 0.3 && i.virt.research_ready),
                            GATE,
                        ),
                        Consideration::new("tech gap", i.virt.research_gap, Curve::Linear { m: 0.6, b: 0.4 }),
                        {
                            // M14 phase 5: the lapse term floored at `[tech] research_lapse_floor`.
                            let mut c =
                                Consideration::new("max lapse", i.virt.max_lapse, Curve::Logistic { k: 8.0, mid: 0.4 });
                            c.output = c.output.max(i.virt.lapse_floor);
                            c
                        },
                        Consideration::new("cash", i.cash, Curve::Linear { m: 0.5, b: 0.5 }),
                        Consideration::new("greed", i.greed, Curve::Linear { m: 0.4, b: 0.6 }),
                    ],
                    f.research,
                )
            })
            .flatten(),
        // M14 V31 (phase 3).
        i.virt
            .enabled
            .then(|| {
                score(
                    CorpOrder::VirtRaid,
                    n,
                    vec![
                        Consideration::new("staffed Lab & target", can(i.virt.run_ready && i.virt.virt_p > 0.0), GATE),
                        Consideration::new("tech gap", i.virt.tech_gap, Curve::Linear { m: 0.7, b: 0.3 }),
                        Consideration::new(
                            "1-lawfulness",
                            1.0 - i.lawfulness,
                            Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 },
                        ),
                        Consideration::new("1-share", 1.0 - i.virt.share, Curve::Linear { m: 0.5, b: 0.5 }),
                        Consideration::new("cash", i.cash, Curve::Linear { m: 0.4, b: 0.6 }),
                    ],
                    f.virt_raid,
                )
            })
            .flatten(),
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
    matches!(
        order,
        CorpOrder::Secure | CorpOrder::Hunker | CorpOrder::Lobby | CorpOrder::Research | CorpOrder::VirtRaid
    )
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
            Niche::Security | Niche::Tech => {}
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

/// The margin a shock rescore needs to replace the standing order: scores
/// this close are a tie, and the standing order wins exact ties only.
///
/// M14 phase 5: with the Research change Nutrix scored Research and Secure
/// at the same value and flipped between them on every shock from day 47.
/// A real margin (the daily `[corps] hysteresis`) also moved plane-off
/// shock rescores at margins 0.04-0.09, an M11 rule change, so only ties
/// (a constant, not a knob: M14 review).
pub const SHOCK_HYSTERESIS: f32 = 1e-6;

/// An immediate rescoring in which the standing order wins exact ties
/// ([`SHOCK_HYSTERESIS`]); the pending shocks are consumed.
pub fn rethink(world: &mut World, corp: EntityId) {
    rescore(world, corp, SHOCK_HYSTERESIS, "shock");
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
    let employed: BTreeMap<EntityId, usize> =
        ownership::staff_by_building(world, &buildings).into_iter().map(|(b, v)| (b, v.len())).collect();
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
    let employed: BTreeMap<EntityId, usize> =
        ownership::staff_by_building(world, &buildings).into_iter().map(|(b, v)| (b, v.len())).collect();
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
    let staff: BTreeMap<EntityId, Vec<(crate::time::Tick, EntityId)>> = ownership::staff_by_building(world, &buildings)
        .into_iter()
        .map(|(b, v)| (b, v.into_iter().filter_map(|a| world.comp::<Job>(a).map(|j| (j.hired_tick, a))).collect()))
        .collect();
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
        let name = world.name_of(who);
        let what = role.map_or("worker", |r| r.label());
        let cname = world.owner_label(Some(corp));
        // No vacancy: Hunker is shrinking, and `hunker_vacancies` would close it.
        let text = format!("{name} laid off as {what} by {cname} (hunkering)");
        crate::systems::economy::dismiss(world, who, Some(b), text);
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
    let kind = build_kind_in(world, n);
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
    // M14 V31 (phase 2): a fleet deck posted at each Lab without one.
    crate::systems::virt::fleet_decks(world, corp);
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
/// cheapest Security corp with room. M13 D42: a building with a robot
/// posted needs neither; a robot is bought instead of a contract when it is
/// cheaper over `robot_horizon_days` (`robots::consider_robot`).
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
            && world.comp::<Building>(b).is_some_and(|bd| bd.secured_by.is_none())
            && crate::systems::robots::posted_robot(world, b).is_none();
        if ok {
            targets.push(b);
        }
        if targets.len() >= per_day {
            break;
        }
    }
    // M12 D14: then the unsecured buildings where the law is thinnest
    // (lowest district coverage first, ties the lower id).
    if targets.len() < per_day && world.config.law.district_beats {
        let mut thin: Vec<(f32, EntityId)> = c
            .buildings
            .iter()
            .copied()
            .filter(|b| !targets.contains(b))
            .filter(|&b| {
                world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && bd.secured_by.is_none())
                    && crate::systems::robots::posted_robot(world, b).is_none()
                    && thinly_covered(world, b)
            })
            .map(|b| (world.district(world.district_of_building(b)).coverage, b))
            .collect();
        thin.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        targets.extend(thin.into_iter().take(per_day - targets.len()).map(|(_, b)| b));
    }
    let own_security = c.niches.contains(&Niche::Security);
    for b in targets {
        // M13 D42: a robot instead of a contract when it is cheaper over the horizon.
        if crate::systems::robots::consider_robot(world, corp, b) {
            continue;
        }
        let seller = if own_security { Some(corp) } else { crate::systems::corps::cheapest_seller(world, Some(corp)) };
        if let Some(s) = seller {
            crate::systems::corps::buy_contract(world, b, s);
        }
    }
    // M14 V27 (phase 3): ICE by the largest gap (Virt losses first), a camera.
    crate::systems::virt::secure_ice(world, corp);
    crate::systems::virt::buy_camera(world, corp);
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
    // M14 V27 (phase 3): shed one tier of ICE beyond the value at risk.
    crate::systems::virt::hunker_ice(world, corp);
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
        (CorpOrder::Research, _) => research(world, corp),
        (CorpOrder::VirtRaid, _) => crate::systems::virt::corp_virt_raid(world, corp),
        _ => {}
    }
}

/// M14 V31 (phase 1 part), daily under `Research` (the research itself is
/// `tech::research` at midnight): reset the focus to the track with the
/// largest rival tier gap (else the niche's); build a Lab in the focus on
/// the Lot nearest an owned building when it owns none there
/// (`found_cost.lab`, `Flow::Found`); staff every building up.
fn research(world: &mut World, corp: EntityId) {
    if !world.config.virt.enabled {
        return;
    }
    crate::systems::tech::reset_focus(world, corp);
    let Some(focus) = world.comp::<Corp>(corp).map(|c| c.tech.focus) else { return };
    let has = crate::systems::tech::labs_of(world, corp)
        .iter()
        .any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.focus == Some(focus)));
    let cost = world.config.corps.found_cost.lab;
    // M14 phase 5: a Lab only after `[tech] research_build_days` of Research
    // (a one-day flip into Research, as every corp's day-0 pick, builds none).
    let held = world.comp::<Corp>(corp).map_or(0, |c| world.tick.saturating_sub(c.order_since));
    let committed = held >= u64::from(world.config.tech.research_build_days) * TICKS_PER_DAY;
    if !has && committed && cost > 0 && world.purse(Some(corp)) >= cost {
        if let Some(lot) = lot_near(world, corp) {
            if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Lab, Some(corp)).is_ok() {
                ownership::pay(world, Some(corp), None, cost, Flow::Found);
                if let Some(bd) = world.comp_mut::<Building>(lot) {
                    bd.focus = Some(focus);
                }
                let now = world.tick;
                if let Some(c) = world.comp_mut::<Corp>(corp) {
                    c.last_build_tick = Some(now);
                }
                let (cname, what) = (world.owner_label(Some(corp)), world.name_of(lot));
                world.push_event(
                    EventKind::Founded,
                    &[corp, lot],
                    format!("{cname} built {what} ({focus}) on a Lot for {cost} (researching)"),
                );
            }
        }
    }
    staff_up(world, corp);
    // M14 V31: one fleet deck per Lab without one (the plan's Research row;
    // phase 2 placed it under Grow, where it stays).
    crate::systems::virt::fleet_decks(world, corp);
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
            // M14 phase 5: the node robbed last week hardens under any order.
            crate::systems::virt::harden_robbed(world, c);
            // M13 D44: one fleet purchase (or a Hunker sale) inside the order.
            crate::systems::vehicles::corp_fleet(world, c);
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
