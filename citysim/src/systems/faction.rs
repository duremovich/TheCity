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
    /// M12 D38: how covered the rival Hideout's district is (Raid; Retaliate
    /// reads `retaliate_cover`): 1 under a `Crackdown(self)` or `Cordon` stance there,
    /// else `(coverage − 0.5) ÷ 1.5` clamped to 0..0.95. 0 with `[law]
    /// district_beats` off (the M11 brain).
    pub target_cover: f32,
    /// M14 V34: the same over the Retaliate target: the gang a pending
    /// `Shock::Hacked` names (`raid::hack_grudge`), else `raid::raid_rival`
    /// (a standing Retaliate's named gang, else the rival: `target_cover`).
    pub retaliate_cover: f32,
    /// M12 D38: the same for the Jail's district (BreakOut); 1 under
    /// Garrison. Plan deviation: one field per target, since BreakOut and
    /// the raids aim at different districts.
    pub jail_cover: f32,
    /// M12 D38 (phase 3): derelict Blocks in the gang's districts (its
    /// Hideout's and its held Homes') it does not hold yet.
    pub derelicts: usize,
    /// M12 D38: districts with `control == Gang(self)`, and districts that
    /// are Contested or held by a gang with no fit members (Expand's ground).
    pub districts_held: usize,
    pub open_districts: usize,
    /// M12 D39: the hoarding corp's richest building in (or next to) a
    /// district this gang holds, and the prize of a won raid on it.
    pub corp_prize: Option<(EntityId, i64)>,
    /// M12 D39: the cover over that building's district, and the private
    /// guards on its side (the corp Raid's ratio).
    pub corp_cover: f32,
    pub corp_guards: usize,
    /// M12 D39: corp raids are on (`[gangs] corp_raid_cap > 0`): the hoard
    /// tilt lands on the corp Raid, not on Contest (M11 D33).
    pub corp_raids: bool,
    /// M13 D36: a Clinic stands (somewhere to sell or install the take).
    pub clinic_exists: bool,
    /// M13 D36: `chrome::harvest_target`: the most valuable visible chrome
    /// on the gang's ground, and its value.
    pub harvest_target: Option<(EntityId, i64)>,
    /// M13 D36: the cover over the Harvest target's district (`target_cover`'s rule).
    pub harvest_cover: f32,
    /// M13 D36: the leader's lawfulness.
    pub lawfulness: f32,
    /// M13 D36: the gang's treasury ÷ `[corps] hoard_heat`, clamped 0..1.
    pub treasury_x: f32,
    /// M14 V30: the member with a deck and the best hacking (ties the lower
    /// id); `None` with the plane off.
    pub runner: Option<EntityId>,
    /// M14 V30: the best target's EV ÷ `[corps] hoard_heat` (0..1) and its
    /// `p_success` (0 unless it clears `min_route_p`), from one search at
    /// the Hideout node.
    pub virt_ev: f32,
    pub virt_p: f32,
    /// M14 V30: a traced run on our nodes named someone lately.
    pub hacked: bool,
}

/// M12 D38: the cover over `gang`'s target under `order` (the Jail for
/// BreakOut; under the gang's standing Raid its corp target if it has one;
/// the rival Hideout otherwise). See [`cover_of`]. 0 with no target.
pub fn target_cover(world: &World, gang: EntityId, order: Order) -> f32 {
    let target = if order.target_is_jail() {
        world.building_of_kind(BuildingKind::Jail)
    } else if order == Order::Raid {
        crate::systems::raid::corp_target(world, gang)
            .or_else(|| world.rival_of(gang).and_then(|r| world.hideout_of(r)))
    } else {
        // M14 V34: a standing Retaliate's named gang (`raid::raid_rival`).
        crate::systems::raid::raid_rival(world, gang).and_then(|r| world.hideout_of(r))
    };
    target.map_or(0.0, |t| cover_of(world, gang, t))
}

/// M12 D38: the cover over a target building for `gang`: 1 when it is the
/// Jail under Garrison, or its district's stance is `Crackdown(gang)` or
/// `Cordon`; else `((cov − 0.5) ÷ 1.5).clamp(0, 0.95)`, with a district
/// without Homes (the Civic) reading coverage 1.0, so coverage alone never
/// shuts the gate. 0 with `[law] district_beats` off.
pub fn cover_of(world: &World, gang: EntityId, target: EntityId) -> f32 {
    use crate::components::Stance;
    if !world.config.law.district_beats {
        return 0.0;
    }
    let jail = world.comp::<Building>(target).is_some_and(|b| b.kind == BuildingKind::Jail);
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
    Some(OrderScore { order, score: raw + flat, considerations: cs, corp_target: false })
}

/// Score every order, best first. LieLow always scores, so the result is never empty.
pub fn score_orders(i: &OrderInputs, cfg: &GangsCfg) -> Vec<OrderScore> {
    let f = &cfg.order_flat;
    let frontier_frac = i.frontier as f32 / i.frontier_total.max(1) as f32;
    let calm = 1.0 - i.heat;
    let weakness = ((i.rival as f32 / i.own.max(1) as f32) / 2.0).clamp(0.0, 1.0);
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
            f.contest + if i.corp_raids { 0.0 } else { i.hoard_tilt * i.hoard },
        ),
        raid_score(i, cfg, false),
        score(
            Order::Retaliate,
            vec![
                // Also off the raid cooldown (the spec exempts Retaliate): a
                // grudge a night is a feud, and the cooldown paces it.
                Consideration::new(
                    "grudge",
                    can(i.grudge && i.rival_exists && i.raid_ready && i.own >= cfg.raid_min_members),
                    GATE,
                ),
                Consideration::new("pride", i.pride, Curve::Linear { m: 0.9, b: 0.1 }),
                Consideration::new("courage", i.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("open target", can(i.retaliate_cover < 1.0), GATE),
                Consideration::new("target cover", 1.0 - i.retaliate_cover, Curve::Linear { m: 0.8, b: 0.2 }),
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
            Order::Squat,
            vec![
                Consideration::new("derelicts", can(i.derelicts > 0), GATE),
                Consideration::new(
                    "derelict count",
                    (i.derelicts as f32 / 3.0).min(1.0),
                    Curve::Linear { m: 0.5, b: 0.5 },
                ),
                Consideration::new("1-heat", calm, Curve::Linear { m: 0.8, b: 0.2 }),
                Consideration::new("greed", i.greed, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.squat,
        ),
        // M13 D36 (spec § 3 table).
        score(
            Order::Harvest,
            vec![
                Consideration::new(
                    "crew, clinic, target",
                    can(i.own >= 3 && i.clinic_exists && i.harvest_target.is_some()),
                    GATE,
                ),
                Consideration::new("greed", i.greed, Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }),
                Consideration::new("1-lawfulness", 1.0 - i.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-U(treasury/hoard_heat)", i.treasury_x, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("1-heat", calm, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-target cover", 1.0 - i.harvest_cover, Curve::Linear { m: 0.8, b: 0.2 }),
            ],
            f.harvest,
        ),
        // M14 V30 (spec § 6 table).
        score(
            Order::VirtRaid,
            vec![
                Consideration::new("runner & target", can(i.runner.is_some() && i.virt_p > 0.0), GATE),
                Consideration::new("greed", i.greed, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("U(EV/hoard_heat)", i.virt_ev, Curve::Logistic { k: 8.0, mid: 0.3 }),
                Consideration::new("1-U(treasury/hoard_heat)", 1.0 - i.treasury_x, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("1-heat", calm, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("hacked", if i.hacked { 1.0 } else { 0.0 }, Curve::Linear { m: 0.5, b: 0.5 }),
            ],
            f.virt_raid,
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
    // M12 D39: Raid is scored for the rival and for the corp prize; the better stays.
    if let Some(corp) = raid_score(i, cfg, true) {
        match out.iter_mut().find(|s| s.order == Order::Raid) {
            Some(rival) if rival.score >= corp.score => {}
            Some(rival) => *rival = corp,
            None => out.push(corp),
        }
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.order.cmp(&b.order)));
    out
}

/// One Raid variant's score: the rival Hideout (M8), or (`corp`) the corp
/// building of `corp_prize` (M12 D39), whose prize term reads the corp
/// prize, whose ratio reads the private guards on its side, whose cover
/// reads its district, and whose flat gains `hoard_tilt × hoard`. A corp
/// variant is marked `corp_target` (its trace also shows a "corp target"
/// consideration, always 1, for the panel).
fn raid_score(i: &OrderInputs, cfg: &GangsCfg, corp: bool) -> Option<OrderScore> {
    let f = &cfg.order_flat;
    let calm = 1.0 - i.heat;
    let (ready, opponents, prize, cover, flat) = if corp {
        let (_, value) = i.corp_prize?;
        (value >= cfg.raid_min_prize, i.corp_guards, value, i.corp_cover, f.raid + i.hoard_tilt * i.hoard)
    } else {
        (i.rival_exists && i.prize >= cfg.raid_min_prize, i.rival, i.prize, i.target_cover, f.raid)
    };
    let prize_x = (prize as f32 / 200.0).clamp(0.0, 1.0);
    let mut cs = vec![
        Consideration::new("raid ready", can(i.raid_ready && ready && i.own >= cfg.raid_min_members), GATE),
        Consideration::new(
            "ratio",
            ratio_x(i.own, opponents),
            Curve::Logistic { k: 16.0, mid: cfg.raid_min_ratio / 2.0 },
        ),
        Consideration::new("prize", prize_x, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("courage", i.courage, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("1-heat", calm, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("open target", can(cover < 1.0), GATE),
        Consideration::new("target cover", 1.0 - cover, Curve::Linear { m: 0.8, b: 0.2 }),
    ];
    if corp {
        cs.push(Consideration::new("corp target", 1.0, Curve::Linear { m: 0.0, b: 1.0 }));
    }
    score(Order::Raid, cs, flat).map(|s| OrderScore { corp_target: corp, ..s })
}

/// D39: does this Raid score (the best Raid entry) aim at the corp building?
pub fn is_corp_raid(s: &OrderScore) -> bool {
    s.order == Order::Raid && s.corp_target
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
    // M12 D38: the districts this gang holds, and the open ones (Contested,
    // or held by a gang with nobody fit), each with Homes.
    let (held, open) = district_ground(world, gang);
    let districts_held = held.iter().filter(|&&x| x).count();
    let open_districts = open.iter().filter(|&&x| x).count();
    let by_district = world.districts.len() > 1 && districts_held > 0;
    let (mut frontier, mut frontier_total) = (0, 0);
    for &h in world.buildings_by_kind.get(&BuildingKind::Home).map(|v| v.as_slice()).unwrap_or(&[]) {
        let Some(b) = world.comp::<Building>(h) else { continue };
        if b.demolished || b.derelict || residents.get(h.index as usize).is_none_or(|&n| n == 0) {
            continue;
        }
        // M12 D38: with a district held, Expand's frontier is the held and
        // open districts; a gang holding none (or a one-district city) keeps
        // the M8 midline, so a fresh gang still expands.
        let ours = if by_district {
            let d = world.district_of(b.door).index();
            held.get(d).copied().unwrap_or(false) || open.get(d).copied().unwrap_or(false)
        } else {
            match (own_door, rival_door) {
                (Some(o), Some(r)) => b.door.manhattan(o) <= b.door.manhattan(r),
                _ => true,
            }
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
    let corp_raids = cfg.corp_raid_cap > 0;
    // D39 reading: "a district held by this gang" = one where it holds Homes
    // (as a split reads it); with control alone only the city's biggest gang
    // ever had a prize (seed 42: one gang, 38 days in 120).
    let mut turf = vec![false; world.districts.len()];
    for &h in &g.territory {
        if let Some(t) = turf.get_mut(world.district_of_building(h).index()) {
            *t = true;
        }
    }
    let corp_prize = if corp_raids { corp_prize(world, &turf, hoard_corp) } else { None };
    let corp_cover = corp_prize.map_or(0.0, |(b, _)| cover_of(world, gang, b));
    // Fix pass: the guards the gang can expect at the door are the ones the
    // corp posts there (`raid::posted_guards`' cap), not its whole roster.
    let corp_guards = corp_prize
        .map_or(0, |(b, _)| crate::systems::raid::private_guards_of(world, b).len().min(cfg.corp_raid_posted));
    let rival_hq = rival.and_then(|r| world.hideout_of(r));
    // M14 V34: the Retaliate target (a hacker named by a pending shock, a
    // standing Retaliate's named gang, else the rival).
    let retaliate_hq = crate::systems::raid::hack_grudge(world, gang)
        .or_else(|| crate::systems::raid::raid_rival(world, gang))
        .and_then(|r| world.hideout_of(r));
    let jail = world.building_of_kind(BuildingKind::Jail);
    // M13 D36: the Harvest inputs (the target is cached on the gang by `rescore`).
    let clinic_exists = world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .any(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict));
    let harvest_target = if clinic_exists { crate::systems::chrome::harvest_target(world, gang) } else { None };
    let harvest_cover = harvest_target
        .and_then(|(t, _)| world.comp::<crate::components::Position>(t))
        .map_or(0.0, |p| tile_cover(world, gang, p.tile));
    let heat_ref = world.config.corps.hoard_heat.max(1);
    // M14 V30: the gang's runner and best target (nothing with the plane off).
    let virt = crate::systems::virt::gang_virt_inputs(world, gang, corp_prize.map(|(b, _)| b));
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
        target_cover: rival_hq.map_or(0.0, |h| cover_of(world, gang, h)),
        retaliate_cover: retaliate_hq.map_or(0.0, |h| cover_of(world, gang, h)),
        jail_cover: jail.map_or(0.0, |j| cover_of(world, gang, j)),
        derelicts: crate::systems::gang::squat_targets(world, gang).len(),
        districts_held,
        open_districts,
        corp_prize,
        corp_cover,
        corp_guards,
        corp_raids,
        clinic_exists,
        harvest_target,
        harvest_cover,
        lawfulness: p.lawfulness,
        treasury_x: (g.treasury as f32 / heat_ref as f32).clamp(0.0, 1.0),
        runner: virt.0,
        virt_ev: virt.1,
        virt_p: virt.2,
        hacked: virt.3,
    })
}

/// M13 D36: [`cover_of`]'s rule for a district by tile (the Harvest target's).
fn tile_cover(world: &World, gang: EntityId, tile: crate::components::TilePos) -> f32 {
    use crate::components::Stance;
    if !world.config.law.district_beats {
        return 0.0;
    }
    let d = world.district(world.district_of(tile));
    if d.stance == Stance::Crackdown(gang) || d.stance == Stance::Cordon {
        return 1.0;
    }
    let cov = if d.homes.is_empty() { 1.0 } else { d.coverage };
    ((cov - 0.5) / 1.5).clamp(0.0, 0.95)
}

/// M12 D38: per district, held by `gang` (`control == Gang(gang)`), and open
/// (Contested, or held by a gang with no fit members); only districts with
/// Homes count.
pub fn district_ground(world: &World, gang: EntityId) -> (Vec<bool>, Vec<bool>) {
    use crate::components::Controller;
    let mut held = vec![false; world.districts.len()];
    let mut open = vec![false; world.districts.len()];
    for (i, d) in world.districts.iter().enumerate() {
        if d.homes.is_empty() {
            continue;
        }
        match d.control {
            Controller::Gang(g) if g == gang => held[i] = true,
            Controller::Gang(g) => open[i] = crate::systems::gang::fit_headcount(world, g) == 0,
            Controller::Contested => open[i] = true,
            _ => {}
        }
    }
    (held, open)
}

/// M12 D39: the hoarding corp's building with the highest 7-day revenue
/// (ties lower id) in a district `held` marks (the gang's turf) or next to one, with the
/// prize a won raid on it takes (`raid::corp_prize_value`).
pub fn corp_prize(world: &World, held: &[bool], hoard_corp: Option<EntityId>) -> Option<(EntityId, i64)> {
    let corp = hoard_corp?;
    let mask: u16 = held.iter().enumerate().filter(|(_, &h)| h).fold(0, |m, (i, _)| m | (1 << i));
    if mask == 0 {
        return None;
    }
    let near = |d: usize| mask & (1 << d) != 0 || world.district_adjacent.get(d).is_some_and(|&adj| adj & mask != 0);
    let buildings = world.comp::<Corp>(corp).map(|c| c.buildings.clone()).unwrap_or_default();
    let best = buildings
        .into_iter()
        .filter_map(|b| world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict).map(|bd| (b, bd)))
        .filter(|(_, bd)| near(world.district_of(bd.door).index()))
        .map(|(b, bd)| (bd.revenue.iter().sum::<i64>(), b))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))?;
    let value = crate::systems::raid::corp_prize_value(world, corp);
    Some((best.1, value))
}

/// M16 hook (docs/M16_CONTRACTS.md, attention and distraction), left by the
/// M12 fix pass: how alert a faction (a gang, a corp; `None` = the City's
/// law) is, a multiplier on its detection and response rolls. M12 reads it
/// in the riot response, the corp-raid response (the posted guards) and the
/// stance's sweep rolls. Always 1.0 until M16 gives factions attention.
pub fn alertness_mult(_world: &World, _faction: Option<EntityId>) -> f32 {
    1.0
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
    // M13 D36: the Harvest target, cached for the members' GangWork.
    let harvest = inputs.harvest_target.map(|(t, _)| t);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.harvest_target = harvest;
    }
    let Some((current, name)) = world.comp::<Gang>(gang).map(|g| (g.order, g.name.clone())) else { return false };
    let next = choose(&scores, current, hysteresis);
    let best_score = scores.first().map_or(0.0, |s| s.score);
    let current_score = scores.iter().find(|s| s.order == current).map_or(0.0, |s| s.score);
    let corp_raid_best = scores.iter().find(|s| s.order == Order::Raid).is_some_and(is_corp_raid);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.order_trace = scores;
    }
    let Some(order) = next else { return true };
    let muster = order.is_raid().then(|| next_muster(now, cfg.raid_muster_hour));
    // M12 D39: a Raid chosen for the corp prize names the corp building.
    let corp_target = inputs.corp_prize.map(|(b, _)| b).filter(|_| order == Order::Raid && corp_raid_best);
    // M14 V34: a Retaliate chosen on a pending `Shock::Hacked { by }` naming
    // another gang fights that gang (with three gangs or more it need not be
    // the rival); read before the caller consumes the shocks.
    let retaliate_on = if order == Order::Retaliate { crate::systems::raid::hack_grudge(world, gang) } else { None };
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.order = order;
        g.order_since = now;
        g.raid_at = muster;
        g.raid_target = corp_target;
        g.retaliate_on = retaliate_on;
        if order == Order::Retaliate {
            g.retaliate_until = Some(now + cfg.retaliate_days * TICKS_PER_DAY);
        }
    }
    // M14 V32: the Raid prelude, timed off the corp raid's muster.
    if let (Some(t), Some(at)) = (corp_target, muster) {
        crate::systems::virt::raid_prelude(world, gang, t, at);
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
