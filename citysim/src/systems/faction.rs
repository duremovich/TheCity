//! The faction brain. Each gang scores its five standing orders daily (with
//! hysteresis) and at once when pending shocks add up, then issues the best.
//! Orders bias members' goals (`utility::goals`); the brain never acts.

use crate::components::{
    Building, BuildingKind, Corp, Gang, Household, LawShock, LobbyHold, MemoryKind, Order, OrderScore, Personality,
    Sentence,
};
use crate::config::GangsCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::world::World;

/// Everything `score_orders` reads, gathered once per rescoring.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderInputs {
    /// Unclaimed Homes (nobody holds a count-3 claim) nearer this gang's Hideout than the rival's.
    pub frontier: usize,
    /// Inhabited Homes nearer this gang's Hideout than the rival's.
    pub frontier_total: usize,
    /// Homes the rival holds.
    pub rival_territory: usize,
    /// Headcounts, excluding the jailed.
    pub own: usize,
    pub rival: usize,
    /// Arrests and deaths within `heat_days` ÷ own, clamped to 1.
    pub heat: f32,
    /// The rival's treasury.
    pub prize: i64,
    /// A grudge shock is pending, or `retaliate_until` has not passed (a
    /// sack sets it past `sacked_until`, so the grievance outlives the sack).
    pub grudge: bool,
    pub greed: f32,
    pub courage: f32,
    pub pride: f32,
    /// Off the raid cooldown and not sacked.
    pub raid_ready: bool,
    pub rival_exists: bool,
    pub sacked: bool,
    /// M9: members in the Jail, whether the boss is among them, whether the
    /// breakout cooldown has passed (and the gang is not sacked), and whether
    /// the law holds the Jail in force (`Posture::Garrison`).
    pub jailed: usize,
    pub boss_jailed: bool,
    pub breakout_ready: bool,
    pub garrison: bool,
    pub loyalty: f32,
    /// M11 D33: how far the richest corp's treasury is past `[corps]
    /// hoard_heat` (0..1), and which corp; Contest's flat gains
    /// `hoard_tilt × hoard` (0 with no hoarder, so the M8 brain is unchanged).
    pub hoard: f32,
    pub hoard_corp: Option<EntityId>,
    pub hoard_tilt: f32,
    /// M12 D38: how covered the rival Hideout's district is (Raid,
    /// Retaliate): 1 under a `Crackdown(self)` or `Cordon` stance there,
    /// else `(coverage − 0.5) ÷ 1.5` clamped to 0..0.95. 0 with `[law]
    /// district_beats` off (the M11 brain).
    pub target_cover: f32,
    /// M12 D38: the same for the Jail's district (BreakOut); 1 under
    /// Garrison. Plan deviation: one field per target, since BreakOut and
    /// the raids aim at different districts.
    pub jail_cover: f32,
}

/// M12 D38: the cover over `gang`'s target under `order` (the Jail for
/// BreakOut, the rival Hideout otherwise): 1 when the target is the Jail
/// under Garrison, or its district's stance is `Crackdown(gang)` or
/// `Cordon`; else `((cov − 0.5) ÷ 1.5).clamp(0, 0.95)`, with a district
/// without Homes (the Civic) reading coverage 1.0, so coverage alone never
/// shuts the gate. 0 with `[law] district_beats` off or no target.
pub fn target_cover(world: &World, gang: EntityId, order: Order) -> f32 {
    use crate::components::Stance;
    if !world.config.law.district_beats {
        return 0.0;
    }
    let jail = order.target_is_jail();
    let target = if jail {
        world.building_of_kind(BuildingKind::Jail)
    } else {
        world.rival_of(gang).and_then(|r| world.hideout_of(r))
    };
    let Some(target) = target else { return 0.0 };
    if jail && crate::systems::law::garrisoned(world) {
        return 1.0;
    }
    let d = world.district(world.district_of_building(target));
    if d.stance == Stance::Crackdown(gang) || d.stance == Stance::Cordon {
        return 1.0;
    }
    let cov = if d.homes.is_empty() { 1.0 } else { d.coverage };
    ((cov - 0.5) / 1.5).clamp(0.0, 0.95)
}

/// D33: `(clamp((richest corp treasury − hoard_heat) ÷ hoard_heat, 0, 1), that
/// corp)`; `(0, None)` with no corp past the heat.
pub fn hoard(world: &World) -> (f32, Option<EntityId>) {
    let heat = world.config.corps.hoard_heat;
    if heat <= 0 {
        return (0.0, None);
    }
    let richest = world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (cc.treasury, c)))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    match richest {
        Some((t, c)) if t > heat => (((t - heat) as f32 / heat as f32).clamp(0.0, 1.0), Some(c)),
        _ => (0.0, None),
    }
}

/// `own / max(rival, 1)` mapped onto `[0, 1]` as `ratio / 2`: a Logistic mid of
/// `min_ratio / 2` sits where the spec's un-normalised `min_ratio` would.
fn ratio_x(own: usize, rival: usize) -> f32 {
    (own as f32 / rival.max(1) as f32 / 2.0).clamp(0.0, 1.0)
}

/// Product of the outputs plus a flat term; `None` when a gate is shut.
fn score(order: Order, cs: Vec<Consideration>, flat: f32) -> Option<OrderScore> {
    if cs.iter().any(|c| c.output <= 0.0) {
        return None;
    }
    let raw: f32 = cs.iter().map(|c| c.output).product();
    Some(OrderScore { order, score: raw + flat, considerations: cs })
}

/// Score every order, best first. LieLow always scores, so the result is never empty.
pub fn score_orders(i: &OrderInputs, cfg: &GangsCfg) -> Vec<OrderScore> {
    let f = &cfg.order_flat;
    let frontier_frac = i.frontier as f32 / i.frontier_total.max(1) as f32;
    let calm = 1.0 - i.heat;
    let weakness = ((i.rival as f32 / i.own.max(1) as f32) / 2.0).clamp(0.0, 1.0);
    let prize_x = (i.prize as f32 / 200.0).clamp(0.0, 1.0);
    let jailed_frac = i.jailed as f32 / (i.jailed + i.own).max(1) as f32;
    let scored = [
        score(
            Order::Expand,
            vec![
                Consideration::new("frontier open", can(i.frontier > 0), GATE),
                Consideration::new("frontier frac", frontier_frac, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-heat", calm, Curve::Linear { m: 0.8, b: 0.2 }),
                Consideration::new("greed", i.greed, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.expand,
        ),
        score(
            Order::Contest,
            vec![
                Consideration::new("rival territory", can(i.rival_territory > 0), GATE),
                Consideration::new(
                    "ratio",
                    ratio_x(i.own, i.rival),
                    Curve::Logistic { k: 12.0, mid: cfg.contest_min_ratio / 2.0 },
                ),
                Consideration::new("1-heat", calm, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("greed", i.greed, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("pride", i.pride, Curve::Linear { m: 0.4, b: 0.6 }),
            ],
            f.contest + i.hoard_tilt * i.hoard,
        ),
        score(
            Order::Raid,
            vec![
                Consideration::new(
                    "raid ready",
                    can(i.raid_ready && i.rival_exists && i.prize >= cfg.raid_min_prize),
                    GATE,
                ),
                Consideration::new(
                    "ratio",
                    ratio_x(i.own, i.rival),
                    Curve::Logistic { k: 16.0, mid: cfg.raid_min_ratio / 2.0 },
                ),
                Consideration::new("prize", prize_x, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("1-heat", calm, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("open target", can(i.target_cover < 1.0), GATE),
                Consideration::new("target cover", 1.0 - i.target_cover, Curve::Linear { m: 0.8, b: 0.2 }),
            ],
            f.raid,
        ),
        score(
            Order::Retaliate,
            vec![
                // Also off the raid cooldown (the spec exempts Retaliate): a
                // grudge a night is a feud, and the cooldown paces it.
                Consideration::new("grudge", can(i.grudge && i.rival_exists && i.raid_ready), GATE),
                Consideration::new("pride", i.pride, Curve::Linear { m: 0.9, b: 0.1 }),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("open target", can(i.target_cover < 1.0), GATE),
                Consideration::new("target cover", 1.0 - i.target_cover, Curve::Linear { m: 0.8, b: 0.2 }),
            ],
            f.retaliate,
        ),
        score(
            Order::LieLow,
            vec![
                Consideration::new("heat", i.heat, Curve::Logistic { k: 10.0, mid: 0.55 }),
                Consideration::new("weakness", weakness, Curve::Linear { m: 0.6, b: 0.4 }),
            ],
            f.lielow,
        ),
        score(
            Order::BreakOut,
            vec![
                Consideration::new(
                    "convicts",
                    can(i.jailed > 0 && i.breakout_ready && i.own >= cfg.breakout_min_members),
                    GATE,
                ),
                Consideration::new("jailed frac", jailed_frac, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("boss inside", can(i.boss_jailed), Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.8, b: 0.2 }),
                Consideration::new("loyalty", i.loyalty, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-garrison", can(!i.garrison), Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("open target", can(i.jail_cover < 1.0), GATE),
                Consideration::new("target cover", 1.0 - i.jail_cover, Curve::Linear { m: 0.8, b: 0.2 }),
            ],
            f.breakout,
        ),
    ];
    let mut out: Vec<OrderScore> = scored.into_iter().flatten().collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.order.cmp(&b.order)));
    out
}

/// The order to switch to, if the best beats the current one by `hysteresis`.
/// An unavailable current order scores zero, so anything replaces it.
pub fn choose(scores: &[OrderScore], current: Order, hysteresis: f32) -> Option<Order> {
    let best = scores.first()?;
    let current_score = scores.iter().find(|s| s.order == current).map_or(0.0, |s| s.score);
    (best.order != current && best.score > current_score + hysteresis).then_some(best.order)
}

/// The next `hour:00` at least two hours away.
pub fn next_muster(now: Tick, hour: u16) -> Tick {
    let day_start = now - now % TICKS_PER_DAY;
    let today = day_start + Tick::from(hour) * TICKS_PER_HOUR;
    if today >= now + 2 * TICKS_PER_HOUR {
        today
    } else {
        today + TICKS_PER_DAY
    }
}

/// `heat_log` entries within `heat_days` ÷ headcount, clamped to 1: the
/// fraction of the gang hit lately. The whole roster is the denominator (the
/// spec says the fit count): with half the gang in the Jail the fit count
/// doubled the heat and every gang lay low for good.
pub fn heat(world: &World, gang: EntityId) -> f32 {
    let Some(g) = world.comp::<Gang>(gang) else { return 0.0 };
    let window = world.config.gangs.heat_days * TICKS_PER_DAY;
    let now = world.tick;
    let recent = g.heat_log.iter().filter(|&&(t, _)| now.saturating_sub(t) < window).count();
    (recent as f32 / g.members.len().max(1) as f32).clamp(0.0, 1.0)
}

/// Gather the inputs, or `None` when the gang has no leader fit to decide
/// (none, or jailed): the standing order then stays as it is.
pub fn gather_inputs(world: &World, gang: EntityId) -> Option<OrderInputs> {
    let g = world.comp::<Gang>(gang)?;
    let leader = g.leader.filter(|&l| !world.has::<Sentence>(l))?;
    let p = world.comp::<Personality>(leader)?;
    let now = world.tick;
    let cfg = &world.config.gangs;
    let rival = world.rival_of(gang);
    let rg = rival.and_then(|r| world.comp::<Gang>(r));
    let own_door = world.comp::<Building>(g.hideout).map(|b| b.door);
    let rival_door = rg.and_then(|r| world.comp::<Building>(r.hideout)).map(|b| b.door);

    // Inhabited Homes on our side of the midline, and how many are still unclaimed.
    let mut residents = vec![0u16; world.building.len()];
    for id in world.citizens() {
        if let Some(h) = world.comp::<Household>(id).and_then(|h| h.home) {
            if let Some(n) = residents.get_mut(h.index as usize) {
                *n += 1;
            }
        }
    }
    let (mut frontier, mut frontier_total) = (0, 0);
    for &h in world.buildings_by_kind.get(&BuildingKind::Home).map(|v| v.as_slice()).unwrap_or(&[]) {
        let Some(b) = world.comp::<Building>(h) else { continue };
        if b.demolished || residents.get(h.index as usize).is_none_or(|&n| n == 0) {
            continue;
        }
        let ours = match (own_door, rival_door) {
            (Some(o), Some(r)) => b.door.manhattan(o) <= b.door.manhattan(r),
            _ => true,
        };
        if !ours {
            continue;
        }
        frontier_total += 1;
        if crate::systems::gang::holder_of(world, h).is_none() {
            frontier += 1;
        }
    }
    let own = crate::systems::gang::fit_headcount(world, gang);
    let rival_count = rival.map_or(0, |r| crate::systems::gang::fit_headcount(world, r));
    let cooldown = cfg.raid_cooldown_days * TICKS_PER_DAY;
    let breakout_cooldown = cfg.breakout_cooldown_days * TICKS_PER_DAY;
    let jailed = crate::systems::gang::jailed_headcount(world, gang);
    let (hoard, hoard_corp) = hoard(world);
    Some(OrderInputs {
        frontier,
        frontier_total,
        rival_territory: rg.map_or(0, |r| r.territory.len()),
        own,
        rival: rival_count,
        heat: heat(world, gang),
        prize: rg.map_or(0, |r| r.treasury),
        grudge: g.shocks.iter().any(|s| s.is_grudge()) || g.retaliate_until.is_some_and(|t| t > now),
        greed: p.greed,
        courage: p.courage,
        pride: p.pride,
        raid_ready: g.last_raid_tick.is_none_or(|t| now.saturating_sub(t) >= cooldown) && !g.is_sacked(now),
        rival_exists: rg.is_some(),
        sacked: g.is_sacked(now),
        jailed,
        boss_jailed: g.boss.is_some_and(|b| world.has::<Sentence>(b)),
        breakout_ready: g.last_breakout_tick.is_none_or(|t| now.saturating_sub(t) >= breakout_cooldown)
            && !g.is_sacked(now),
        garrison: crate::systems::law::garrisoned(world),
        loyalty: p.loyalty,
        hoard,
        hoard_corp,
        hoard_tilt: world.config.corps.hoard_tilt,
        target_cover: target_cover(world, gang, Order::Raid),
        jail_cover: target_cover(world, gang, Order::BreakOut),
    })
}

/// A departed raid has this long to reach the rival door before it is
/// written off and the brain rescores again.
pub const RAID_MARCH_TICKS: Tick = 6 * TICKS_PER_HOUR;

/// Score the orders and switch when the best clears `hysteresis`. A change
/// logs `OrderChanged`; Raid and Retaliate schedule the next muster. A raid
/// that has departed is not second-guessed until it resolves or fizzles.
/// Returns whether the orders were scored (the caller consumes the pending
/// shocks only then: a shock that lands mid-march waits for the brawl).
pub fn rescore(world: &mut World, gang: EntityId, hysteresis: f32) -> bool {
    let now = world.tick;
    if let Some(t) = world.comp::<Gang>(gang).and_then(|g| g.raid_at).filter(|&t| t <= now) {
        if now < t + RAID_MARCH_TICKS {
            return false; // marching
        }
        let name = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.raid_at = None;
            g.retaliate_until = None;
        }
        world.push_event(EventKind::Raid, &[gang], format!("{name}'s raid fizzled: nobody reached the rival door"));
    }
    let Some(inputs) = gather_inputs(world, gang) else {
        // Leaderless, or the leader is in the Jail: nothing new is issued and
        // a planned raid is off. (M9's jailbreak is what fixes that.)
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.raid_at = None;
        }
        return true;
    };
    let cfg = world.config.gangs.clone();
    let scores = score_orders(&inputs, &cfg);
    let Some((current, name)) = world.comp::<Gang>(gang).map(|g| (g.order, g.name.clone())) else { return false };
    let next = choose(&scores, current, hysteresis);
    let best_score = scores.first().map_or(0.0, |s| s.score);
    let current_score = scores.iter().find(|s| s.order == current).map_or(0.0, |s| s.score);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.order_trace = scores;
    }
    let Some(order) = next else { return true };
    let muster = order.is_raid().then(|| next_muster(now, cfg.raid_muster_hour));
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.order = order;
        g.order_since = now;
        g.raid_at = muster;
        if order == Order::Retaliate {
            g.retaliate_until = Some(now + cfg.retaliate_days * TICKS_PER_DAY);
        }
    }
    let why = if hysteresis == 0.0 { "shock" } else { "daily" };
    world.push_event(
        EventKind::OrderChanged,
        &[gang],
        format!("{name}: {current:?} -> {order:?} ({why}, {best_score:.2} vs {current_score:.2})"),
    );
    true
}

/// M9 bribery. What a bribe costs today: `bribe_base + bribe_per_guard × guards`.
pub fn bribe_price(world: &World) -> i64 {
    let cfg = &world.config.law;
    cfg.bribe_base + cfg.bribe_per_guard * crate::systems::law_brain::guards(world).len() as i64
}

/// The bribe score, or `None` when the gate is shut (no Crackdown on this
/// gang, no captain, a bribe still holds, or the treasury cannot pay):
/// `(1 − L.pride)` × `captain.greed` × `heat`, each through its curve.
pub fn bribe_score(world: &World, gang: EntityId) -> Option<Vec<Consideration>> {
    let g = world.comp::<Gang>(gang)?;
    let law = world.law()?;
    let captain = law.captain.filter(|&c| crate::systems::law::is_guard(world, c))?;
    let now = world.tick;
    if !crate::systems::law::cracking_down_on(world, gang)
        || g.bribe_until.is_some_and(|t| t > now)
        || g.treasury < bribe_price(world)
    {
        return None;
    }
    let leader = g.leader.filter(|&l| !world.has::<Sentence>(l))?;
    let lp = world.comp::<Personality>(leader)?;
    let cp = world.comp::<Personality>(captain)?;
    Some(vec![
        Consideration::new("1-pride", 1.0 - lp.pride, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("captain greed", cp.greed, Curve::Linear { m: 0.8, b: 0.2 }),
        Consideration::new("heat", heat(world, gang), Curve::Linear { m: 0.5, b: 0.5 }),
    ])
}

/// M11 D21: who offers a bribe.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Payer {
    Gang(EntityId),
    Corp(EntityId),
}

/// M11 D21: what a bribe buys. A gang pays the law to look away; a corp
/// (`Lobby`) pays it to crack down on a gang.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BribeAsk {
    LookAway,
    Crackdown(EntityId),
}

/// Daily, after the law has rescored: a gang under Crackdown may pay the
/// captain. A captain of lawfulness ≥ `incorruptible` refuses (no coins
/// move, the crackdown hardens, `LawShock::BribeRefused`); otherwise the
/// price moves from the treasury to the captain's wallet, the captain
/// drifts (`TookBribe`), the gang is immune to Crackdown for `bribe_days`
/// and the law rethinks at once. Either way the gang does not try again
/// until `bribe_until`; only a taken bribe sets `paid_until`. Returns whether a bribe was offered.
pub fn consider_bribe(world: &mut World, gang: EntityId) -> bool {
    let Some(cs) = bribe_score(world, gang) else { return false };
    let score: f32 = cs.iter().map(|c| c.output).product();
    if score < world.config.law.bribe_threshold {
        return false;
    }
    offer_bribe(world, Payer::Gang(gang), BribeAsk::LookAway, score)
}

/// M11 D21: one bribe offer to the captain, by a gang or a corp. The same
/// price (`bribe_price`), the same `incorruptible` refusal (`hardened_until`,
/// `LawShock::BribeRefused`), the same `Bribe` event (actors `[payer,
/// captain, leader or exec]`), drift and rethink. A gang's taken bribe buys
/// `paid_until` and clears any corp's `Law.lobby` (the gang out-bids); a
/// corp's buys `Law.lobby` (a forced Crackdown on the gang) and never
/// touches the gang's `bribe_until`. The payer must hold the price (a gang's
/// is checked by `bribe_score`). Returns whether an offer was made.
pub fn offer_bribe(world: &mut World, payer: Payer, ask: BribeAsk, score: f32) -> bool {
    let cfg = world.config.law.clone();
    let now = world.tick;
    let Some(captain) = world.law().and_then(|l| l.captain) else { return false };
    let price = bribe_price(world);
    let hold = now + cfg.bribe_days * TICKS_PER_DAY;
    let (payer_id, pname, decider) = match payer {
        Payer::Gang(g) => match world.comp::<Gang>(g) {
            Some(gg) => (g, gg.name.clone(), gg.leader),
            None => return false,
        },
        Payer::Corp(c) => match world.comp::<Corp>(c) {
            Some(cc) if cc.treasury >= price => (c, cc.name.clone(), cc.exec),
            _ => return false,
        },
    };
    let refused = world.comp::<Personality>(captain).is_some_and(|p| p.lawfulness >= cfg.incorruptible);
    match payer {
        Payer::Gang(g) => {
            if let Some(gg) = world.comp_mut::<Gang>(g) {
                gg.bribe_until = Some(hold);
            }
        }
        Payer::Corp(c) => {
            if let Some(cc) = world.comp_mut::<Corp>(c) {
                cc.lobby_until = Some(hold);
            }
        }
    }
    let mut actors = vec![payer_id, captain];
    actors.extend(decider);
    let cname = world.name_of(captain);
    let what = match ask {
        BribeAsk::LookAway => "to look away".to_string(),
        BribeAsk::Crackdown(g) => {
            format!(
                "for a crackdown on {}",
                world.comp::<Gang>(g).map_or_else(|| "a gang".to_string(), |g| g.name.clone())
            )
        }
    };
    if refused {
        if let Some(l) = world.law_mut() {
            l.hardened_until = Some(hold);
        }
        crate::systems::law_brain::push_shock(world, LawShock::BribeRefused);
        let text = match ask {
            BribeAsk::LookAway => format!("{pname} offered captain {cname} {price} coins; refused ({score:.2})"),
            BribeAsk::Crackdown(_) => {
                format!("{pname} offered captain {cname} {price} coins {what}; refused ({score:.2})")
            }
        };
        world.push_event(EventKind::Bribe, &actors, text);
        return true;
    }
    crate::systems::ownership::pay(world, Some(payer_id), Some(captain), price, crate::systems::ownership::Flow::Bribe);
    match (payer, ask) {
        (Payer::Gang(g), _) => {
            if let Some(gg) = world.comp_mut::<Gang>(g) {
                gg.paid_until = Some(hold);
            }
            // The gang out-bids a corp's bought crackdown.
            if let Some(l) = world.law_mut() {
                l.lobby = None;
            }
        }
        (Payer::Corp(c), BribeAsk::Crackdown(gang)) => {
            if let Some(l) = world.law_mut() {
                l.lobby = Some(LobbyHold { corp: c, gang, until: hold });
            }
        }
        (Payer::Corp(_), BribeAsk::LookAway) => {}
    }
    if let Some(p) = world.comp_mut::<Personality>(captain) {
        p.drift(Drift::TookBribe);
    }
    world.remember(captain, MemoryKind::Paid, None, 0.5, 0.3, false);
    world.push_event(
        EventKind::Bribe,
        &actors,
        format!("{pname} paid captain {cname} {price} coins {what} ({score:.2})"),
    );
    crate::systems::law_brain::rethink(world);
    true
}

/// An immediate rescoring with no hysteresis; the pending shocks are consumed
/// when the orders were scored (not while a raid is marching).
pub fn rethink(world: &mut World, gang: EntityId) {
    if rescore(world, gang, 0.0) {
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.shocks.clear();
        }
    }
}
