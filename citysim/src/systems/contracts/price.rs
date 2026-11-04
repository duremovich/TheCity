use super::*;

/// C12: `risk_for(cand)` against a target whose defenders sum to `s_def`.
pub(super) fn risk_against(world: &World, kind: ContractKind, cand: EntityId, s_def: f32) -> f32 {
    if kind.fights() {
        1.0 - missions::estimate_against(world, &[cand], s_def)
    } else {
        0.1
    }
}

/// C12: `risk_for(cand)` on record `c`.
pub fn risk_for(world: &World, c: &Contract, cand: EntityId) -> f32 {
    if c.kind.fights() {
        1.0 - missions::estimate(world, &[cand], &c.target)
    } else {
        0.1
    }
}

fn skill_for(world: &World, kind: ContractKind, cand: EntityId) -> f32 {
    match kind {
        ContractKind::Hit | ContractKind::Beat | ContractKind::Guard => crate::systems::combat::power(world, cand),
        ContractKind::Locate => world.comp::<Skills>(cand).map_or(0.0, |s| s.stealth),
        ContractKind::Persuade | ContractKind::Spin => world.comp::<Skills>(cand).map_or(0.0, |s| s.persuasion),
        ContractKind::Trauma => crate::systems::combat::power(world, cand),
        // M16b phase 2.
        ContractKind::Abduct | ContractKind::Rescue | ContractKind::Escort | ContractKind::Deliver => {
            crate::systems::combat::power(world, cand)
        }
        ContractKind::Steal => world.comp::<Skills>(cand).map_or(0.0, |s| s.stealth.max(s.hacking)),
        // M16b phase 4.
        ContractKind::Tag => world.comp::<Skills>(cand).map_or(0.0, |s| s.stealth),
        ContractKind::Vote => world.comp::<Skills>(cand).map_or(0.0, |s| s.intimidation),
        ContractKind::Retake => crate::systems::combat::power(world, cand),
    }
}

/// C12: `skill × (1 − risk_for) × (0.5 + 0.5 × honour)`.
pub fn fit(world: &World, c: &Contract, cand: EntityId, s_def: f32) -> f32 {
    let honour = crate::systems::reputation::rep(world, cand).honour;
    skill_for(world, c.kind, cand) * (1.0 - risk_against(world, c.kind, cand, s_def)) * (0.5 + 0.5 * honour)
}

/// `U(x) = x ÷ (1 + x)` (plan C13: the spec writes U without defining it).
fn u_of(x: f32) -> f32 {
    let x = x.max(0.0);
    x / (1.0 + x)
}

/// C13 mode A's considerations for `who` on record `c` (without the gate);
/// `urgency` is `1 − time_left ÷ total` (1.0 for `accept_score`).
pub(super) fn mode_a(world: &World, who: EntityId, c: &Contract, risk: f32, urgency_v: f32) -> Vec<Consideration> {
    // M16b: a coerced job pays nothing; the threat behind it weighs as the
    // kind's list price.
    // M16b phase 2: a gang's unpaid order to its own (price 0) weighs as
    // a coerced job does.
    let (price, coerced) = match c.terms {
        Terms::Coerced(_) => (world.config.contracts.price_base.get(c.kind), true),
        Terms::Pay { .. } if c.price == 0 => (world.config.contracts.price_base.get(c.kind), true),
        Terms::Pay { .. } => (c.price, false),
    };
    let cs = mode_a_at(world, who, c.kind, c.brokered(), c.buyer, price, risk, urgency_v);
    // M16b: under a threat neither conscience nor nerve is asked.
    if coerced {
        return cs.into_iter().filter(|x| x.name != "1-lawfulness" && x.name != "courage").collect();
    }
    cs
}

/// The Hit market (decision 7): a gun's weekly-wage floor, `max(50, 0.5 ×
/// 7 × the city's median daily wage)` with the market on (M16a's flat 50
/// off).
fn week_wage_floor(world: &World) -> i64 {
    let h = &world.config.hitmarket;
    if !h.enabled {
        return 50;
    }
    let follow = (h.wage_floor_mult * 7.0 * world.median_wage as f32).round() as i64;
    h.wage_floor_min.max(follow)
}

/// Mode A's considerations at a given `price` (the Hit market's `ask`
/// solves this for the price; `mode_a` reads the record's own).
#[allow(clippy::too_many_arguments)]
pub(super) fn mode_a_at(
    world: &World,
    who: EntityId,
    kind: ContractKind,
    brokered: bool,
    buyer: Option<EntityId>,
    price: i64,
    risk: f32,
    urgency_v: f32,
) -> Vec<Consideration> {
    let p = world.comp::<Personality>(who);
    let mut cs = vec![
        price_term(price, week_wage_of(world, who)),
        Consideration::new("1-risk", 1.0 - risk, Curve::Linear { m: 0.7, b: 0.3 }),
    ];
    if !brokered {
        let honour = buyer.map_or(0.5, |b| crate::systems::reputation::rep(world, b).honour);
        cs.push(Consideration::new("buyer honour", honour, Curve::Linear { m: 0.5, b: 0.5 }));
    }
    if kind.illegal() {
        let law = p.map_or(0.5, |p| p.lawfulness);
        cs.push(Consideration::new("1-lawfulness", 1.0 - law, Curve::Linear { m: 0.6, b: 0.4 }));
    }
    if kind.fights() {
        let courage = p.map_or(0.5, |p| p.courage);
        cs.push(Consideration::new("courage", courage, Curve::Linear { m: 0.5, b: 0.5 }));
    }
    cs.push(Consideration::new("urgency", urgency_v, Curve::Linear { m: 0.5, b: 0.5 }));
    cs
}

/// Mode A's price term: `U(price ÷ week wage)` through Logistic{8, 0.3}
/// (the first consideration of [`mode_a_at`]).
fn price_term(price: i64, week_wage: f32) -> Consideration {
    Consideration::new("U(price/week wage)", u_of(price as f32 / week_wage), Curve::Logistic { k: 8.0, mid: 0.3 })
}

/// A gun's week wage as mode A reads it: `max(7 × wage, the floor)`.
fn week_wage_of(world: &World, who: EntityId) -> f32 {
    let wage = world.comp::<Job>(who).map_or(0, |j| j.wage_per_day);
    (7 * wage).max(week_wage_floor(world)) as f32
}

/// C13: mode A's raw product without the gate, urgency 1.0: what a
/// candidate's acceptance reads at any tier (`Personality`, `Skills`,
/// `Kit`, `Job`, reputation), so a Statistical taker accepts as a Full one.
pub fn accept_score(world: &World, who: EntityId, id: ContractId) -> f32 {
    let Some(c) = world.contracts.get(&id) else { return 0.0 };
    let risk = risk_for(world, c, who);
    mode_a(world, who, c, risk, 1.0).iter().map(|x| x.output).product()
}

/// The Hit market's ceiling on an ask: past it a gun is not for hire.
pub(super) const ASK_CAP: i64 = 1_000_000;

/// The Hit market (docs/HIT_MARKET.md H2): the lowest whole price at which
/// `gun` would accept a `kind` record on `target` (mode A's product at
/// urgency 1 reaching `accept_min`, exactly as matching reads it), against
/// defenders summing `s_def`. `None` when no price will do (the other terms
/// cap the product under `accept_min`). The product rises with the price,
/// so a bisection finds the boundary exactly (no float inversion to drift).
pub fn ask_against(
    world: &World,
    gun: EntityId,
    kind: ContractKind,
    brokered: bool,
    buyer: Option<EntityId>,
    s_def: f32,
) -> Option<i64> {
    let min = world.config.contracts.accept_min;
    let risk = risk_against(world, kind, gun, s_def);
    // Every term but the price's is fixed: their outputs once, multiplied
    // in mode A's order after the price term's (the same f32 operations as
    // the product over the full list, so the boundary is bit-identical).
    let rest: SmallVec<[f32; 6]> =
        mode_a_at(world, gun, kind, brokered, buyer, 1, risk, 1.0).iter().skip(1).map(|x| x.output).collect();
    let week_wage = week_wage_of(world, gun);
    let score = |price: i64| -> f32 { rest.iter().fold(1.0f32 * price_term(price, week_wage).output, |a, &o| a * o) };
    if score(ASK_CAP) < min {
        return None;
    }
    if score(1) >= min {
        return Some(1);
    }
    // Invariant: score(lo) < min <= score(hi).
    let (mut lo, mut hi) = (1i64, ASK_CAP);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if score(mid) >= min {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// [`ask_against`] on `target`'s expected defenders.
pub fn ask(
    world: &World,
    gun: EntityId,
    kind: ContractKind,
    target: &Target,
    brokered: bool,
    buyer: Option<EntityId>,
) -> Option<i64> {
    let s_def = missions::defender_strength(world, target);
    ask_against(world, gun, kind, brokered, buyer, s_def)
}

/// The Hit market (H1): the market price of a `kind` record on `target`:
/// the ask of the `ask_rank`-th cheapest eligible candidate (the broker's
/// regulars when brokered, the buyer's own people when direct), at least
/// `price_floor[kind]`. `None` when fewer than `ask_rank` candidates would
/// take it at any price (the caller falls back to `quote`).
pub fn market_price(
    world: &World,
    kind: ContractKind,
    target: &Target,
    buyer: Option<EntityId>,
    agent: Option<EntityId>,
    broker: Option<EntityId>,
) -> Option<i64> {
    let mut cands: Vec<EntityId> = match broker {
        Some(b) => world.comp::<Broker>(b).map(|k| k.regulars.keys().copied().collect()).unwrap_or_default(),
        None => {
            let mut v = buyer.map(|b| direct_pool(world, b)).unwrap_or_default();
            if direct_gated(world, kind) {
                v.retain(|&a| gun_gate(world, a));
            }
            v
        }
    };
    cands.sort_unstable();
    cands.dedup();
    let probe = record_of(0, buyer, agent, kind, *target, broker, Origin::God, 0, 0);
    let s_def = missions::defender_strength(world, target);
    let mut asks: Vec<i64> = cands
        .into_iter()
        .filter(|&a| eligible(world, &probe, a))
        .filter_map(|a| ask_against(world, a, kind, broker.is_some(), buyer, s_def))
        .collect();
    let rank = world.config.hitmarket.ask_rank.max(1);
    if asks.len() < rank {
        return None;
    }
    asks.sort_unstable();
    Some(asks[rank - 1].max(world.config.hitmarket.price_floor.get(kind)).max(1))
}

/// The Hit market (H1): what an agent, gang or corp pays for a Hit or a
/// Beat: the market price, else `quote` (hard targets keep the old price,
/// Dylan's decision 1). With the market off: `quote`.
pub fn organic_price(
    world: &World,
    kind: ContractKind,
    target: &Target,
    buyer: Option<EntityId>,
    agent: Option<EntityId>,
    broker: Option<EntityId>,
) -> i64 {
    // M16b phase 2: an abduction and a rescue are priced at the guns' asks too.
    if world.config.hitmarket.enabled && kind.fights() {
        if let Some(p) = market_price(world, kind, target, buyer, agent, broker) {
            return p;
        }
    }
    quote(world, kind, target, broker)
}

/// The Hit market's reach (gaps pass): an agent buyer's Hit or Beat that
/// would go direct (no open Fixer within `hire_reach_tiles` of its Home)
/// and finds fewer than `ask_rank` asks among its own people goes to the
/// open Fixer nearest its Home door at any distance, at that Fixer's market
/// price × (1 + `reach_premium`): `(fixer, price)`. `None` with the market
/// off, for a faction buyer, when the direct market prices it, or when no
/// Fixer is open or its regulars price it neither (the caller keeps the
/// direct record at `quote`).
pub fn reach_offer(world: &World, kind: ContractKind, target: &Target, buyer: EntityId) -> Option<(EntityId, i64)> {
    if !world.config.hitmarket.enabled || !kind.violent() || !world.has::<Identity>(buyer) {
        return None;
    }
    if market_price(world, kind, target, Some(buyer), Some(buyer), None).is_some() {
        return None;
    }
    let f = nearest_fixer_to(world, home_or_here(world, buyer)?)?;
    let p = market_price(world, kind, target, Some(buyer), Some(buyer), Some(f))?;
    Some((f, (p as f32 * (1.0 + world.config.hitmarket.reach_premium.max(0.0))).ceil() as i64))
}
