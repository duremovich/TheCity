use super::*;

/// The tier of the fleet deck a corp's VirtRaid buys when its Lab has none
/// (plan deviation: phase 2's tier-1 fleet decks cannot clear `min_route_p`
/// against a rival Lab at ICE 2).
const RAID_DECK_TIER: u8 = 2;

/// A corp's run for the day (V31).
#[derive(Clone, Debug)]
pub struct CorpRun {
    pub lab: EntityId,
    /// The fleet deck posted at the Lab; `None`: the act buys one first.
    pub deck: Option<EntityId>,
    pub runner: EntityId,
    pub target: NodeId,
    pub purpose: Purpose,
    pub p: f32,
}

/// The fleet deck posted at `lab` (condition > 0), the lowest id.
pub fn fleet_deck_at(world: &World, lab: EntityId) -> Option<EntityId> {
    crate::systems::assets::assets_at(world, lab).iter().copied().find(|&a| {
        world
            .comp::<Asset>(a)
            .is_some_and(|x| x.kind == AssetKind::Deck && x.loc == AssetLoc::Posted(lab) && x.condition > 0)
    })
}

/// V31: the corp's run for today, if it has a Lab with a fleet deck, a
/// Researcher there and a niche rival holding Data in its focus (the rival
/// with the most, its Lab with the most in that track): a wipe when the
/// rival's tier there exceeds its own and `1 − lawfulness ≥ wipe_min`, else
/// a steal. Plan deviation: the runner is the best Researcher employed at
/// the Lab, on shift or not (the act runs at midnight).
pub fn corp_run(world: &World, corp: EntityId) -> Option<CorpRun> {
    let c = world.comp::<Corp>(corp)?;
    let focus = c.tech.focus;
    let staff = |lab: EntityId| {
        world
            .workers(crate::components::Role::Researcher)
            .iter()
            .copied()
            .filter(|&r| world.comp::<crate::components::Job>(r).and_then(|j| j.employer) == Some(lab))
            .filter(|&r| crate::systems::law::living(world, r) && !world.has::<crate::components::Sentence>(r))
            .filter(|&r| !world.runner_of.contains_key(&r))
            .filter_map(|r| world.comp::<Skills>(r).map(|s| (s.hacking, r)))
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, r)| r)
    };
    // A Lab with a fleet deck first, else any staffed Lab (ascending).
    let labs = crate::systems::tech::labs_of(world, corp);
    let (lab, deck, runner) = labs
        .iter()
        .find_map(|&l| Some((l, Some(fleet_deck_at(world, l)?), staff(l)?)))
        .or_else(|| labs.iter().find_map(|&l| Some((l, None, staff(l)?))))?;
    let from = node_of_building(world, lab)?;
    let rival = world
        .corps()
        .into_iter()
        .filter(|&r| r != corp)
        .filter(|&r| world.comp::<Corp>(r).is_some_and(|rc| rc.niches.iter().any(|n| c.niches.contains(n))))
        .map(|r| (holding(world, r, focus), std::cmp::Reverse(r)))
        .filter(|&(h, _)| h > 0)
        .max()
        .map(|(_, std::cmp::Reverse(r))| r)?;
    let target = crate::systems::tech::labs_of(world, rival)
        .into_iter()
        .filter_map(|l| node_of_building(world, l))
        .filter_map(|n| world.virt.node(n).map(|x| (x.store.get(focus), std::cmp::Reverse(n))))
        .filter(|&(u, _)| u > 0)
        .max()
        .map(|(_, std::cmp::Reverse(n))| n)?;
    let law = c.exec.and_then(|e| world.comp::<Personality>(e)).map_or(0.5, |p| p.lawfulness);
    let behind = maker_tier(world, rival, focus) > c.tech.tier_of(focus);
    let wipe = behind && 1.0 - law >= world.config.hack.wipe_min;
    let deck_eff = deck
        .and_then(|d| world.comp::<Asset>(d))
        .map_or(RAID_DECK_TIER, |x| crate::systems::assets::eff_tier_of(world, x).max(1));
    let mode = if wipe { RunMode::Loud } else { RunMode::Quiet };
    let o = odds_of(world, runner, deck_eff, Some(corp), mode);
    let tree = routes_from(world, from, deck_eff, o.att, Some(corp));
    let p = route_to(world, &tree, from, target, &o).map_or(0.0, |r| r.p_success);
    Some(CorpRun { lab, deck, runner, target, purpose: Purpose::Data { wipe }, p })
}

/// V31, daily under `VirtRaid`: one `RunOrder` from the Lab chair (the
/// fleet deck is lent through the Kit while the order stands). The order's
/// one spend: a fleet deck at `RAID_DECK_TIER` (the tier `corp_run` scored;
/// no order when only a lower one fits) for a Lab without one,
/// paid from above the fleet reserve ([`fleet_reserve`], as Secure's ICE
/// and cameras), logged "for {Lab} (VirtRaid)".
pub fn corp_virt_raid(world: &mut World, corp: EntityId) {
    let Some(run) = corp_run(world, corp) else { return };
    if world.run_orders.contains_key(&run.runner) {
        return;
    }
    // The order equips its Lab: a fleet deck up to `RAID_DECK_TIER`.
    if run.deck.is_none() {
        let from = world.comp::<Building>(run.lab).map(|b| b.door).unwrap_or_default();
        let reserve = fleet_reserve(world, corp);
        let budget = world.purse(Some(corp)) - reserve;
        // M14 review: `corp_run` scored the run at `RAID_DECK_TIER`; when only
        // a lower tier fits the budget (or is sold), the order is not given.
        let Some((seller, tier, _)) = deck_offer(world, from, budget, RAID_DECK_TIER) else { return };
        if tier < RAID_DECK_TIER {
            return;
        }
        let pick = crate::components::ShopPick { kind: AssetKind::Deck, tier, used: None, upgrade: false };
        let note = format!("for {} (VirtRaid)", world.name_of(run.lab));
        match crate::systems::assets::buy_noted(world, corp, seller, &pick, Some(&note)) {
            Ok(a) => crate::systems::assets::set_loc(world, a, AssetLoc::Posted(run.lab)),
            Err(_) => return,
        }
    }
    let now = crate::systems::midnight::stamp(world);
    let wipe = matches!(run.purpose, Purpose::Data { wipe: true });
    let order = RunOrder {
        patron: Some(corp),
        purpose: run.purpose,
        target: run.target,
        chair: run.lab,
        not_before: now,
        expires: now + TICKS_PER_DAY,
        why: RunWhy::CorpOrder,
        mode: if wipe { RunMode::Loud } else { RunMode::Quiet },
    };
    give_order(world, run.runner, order);
}

/// The corp's alive nodes (building nodes and its Ledger), ascending.
fn corp_nodes(world: &World, corp: EntityId) -> Vec<NodeId> {
    world
        .virt
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.alive && n.owner == Some(corp) && !matches!(n.kind, NodeKind::Public(_)))
        .map(|(i, _)| NodeId(i as u16))
        .collect()
}

/// Phase 3 fix round: can `corp` pay `cost` for a Secure spend (ICE, a
/// camera) while its purse holds `mult × cost` and keeps M13's fleet-buy
/// reserve (`treasury_ref / 4`, the reserve the Data budget keeps) after
/// it? Without the reserve Secure's ICE and cameras ate the Food corps'
/// truck money in days 0-30 (the M13 gate's truck bullets).
pub fn secure_affords(world: &World, corp: EntityId, cost: i64, mult: f32) -> bool {
    let purse = world.purse(Some(corp));
    let reserve = fleet_reserve(world, corp);
    purse as f32 >= mult * cost as f32 && purse - cost >= reserve
}

/// V27 Secure (after the contract and robot pass): raise ICE one tier on
/// up to `secure_per_day` owned nodes with `ice_target − ice_eff > 0`:
/// nodes with a Virt loss in 14 days first (newest first), then the
/// largest gap (ties the lower id). Nodes the corp cannot afford
/// ([`secure_affords`]) are skipped before the `secure_per_day` take.
pub fn secure_ice(world: &mut World, corp: EntityId) {
    let per_day = world.config.corps.secure_per_day;
    let horizon = world.tick.saturating_sub(14 * TICKS_PER_DAY);
    let recent: Vec<NodeId> = world
        .comp::<Corp>(corp)
        .map(|c| c.virt_losses.iter().rev().filter(|&&(t, _)| t >= horizon).map(|&(_, n)| n).collect())
        .unwrap_or_default();
    let gap = |w: &World, n: NodeId| i32::from(ice_target(w, n)) - i32::from(ice_eff(w, n));
    let mut order: Vec<NodeId> = Vec::new();
    for n in recent {
        if !order.contains(&n) && owner_of(world, n) == Some(corp) && gap(world, n) > 0 {
            order.push(n);
        }
    }
    let mut rest: Vec<(std::cmp::Reverse<i32>, NodeId)> = corp_nodes(world, corp)
        .into_iter()
        .filter(|n| !order.contains(n))
        .map(|n| (std::cmp::Reverse(gap(world, n)), n))
        .filter(|&(std::cmp::Reverse(g), _)| g > 0)
        .collect();
    rest.sort_unstable();
    order.extend(rest.into_iter().map(|(_, n)| n));
    // Plan deviation: M13 D42's cash rule for Secure's robots holds for its
    // ICE (the treasury keeps `robot_cash_mult` x the price): with the loss
    // term in `ice_target`, Militech bought ICE 3 for its robbed Lab from
    // Arasaka (2,000) on day 7 and went bankrupt on day 11 (HEAD: 89).
    let mult = world.config.robots.robot_cash_mult;
    let mut raised = 0;
    for n in order {
        if raised >= per_day {
            break;
        }
        let Some((cost, ..)) = ice_quote(world, Some(corp), n) else { continue };
        if !secure_affords(world, corp, cost, mult) {
            continue;
        }
        if install_ice(world, Some(corp), n) {
            raised += 1;
        }
    }
}

/// Days a Virt loss keeps its node on [`harden_robbed`]'s list (spec: "the
/// node that was robbed last week").
const HARDEN_DAYS: Tick = 7;

/// Phase 5 (calibration; plan V27 deviation): whatever its order, a corp
/// raises ICE one tier a day on a node it was robbed on: first a node with
/// a Virt loss since the last midnight (the newest first) while it is below
/// ICE 3 (the robbery is the evidence, so the value target does not bind:
/// a drained Lab's value falls with its store), then a node with a loss
/// within `HARDEN_DAYS` still below its `ice_target` (the loss term lifts
/// it). Paid only from above M13's fleet reserve (`treasury_ref / 4`), and
/// without Secure's cash multiple; under Secure it follows Secure's own pass
/// ([`secure_ice`], whose value target can leave a drained Lab alone). Before it, ICE moved only under Secure, which a
/// robbed corp seldom held (the losses consideration reads a Lab's 90-coin
/// theft against a treasury of thousands): on seeds 42-44 a Virt loss saw
/// its node's ICE rise within 7 days in 3 of 11 episodes.
pub fn harden_robbed(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let now = world.tick;
    let horizon = now.saturating_sub(HARDEN_DAYS * TICKS_PER_DAY);
    let fresh = now.saturating_sub(TICKS_PER_DAY);
    let reserve = fleet_reserve(world, corp);
    let mut recent: Vec<(bool, NodeId)> = Vec::new();
    for &(t, n) in c.virt_losses.iter().rev().filter(|&&(t, _)| t >= horizon) {
        if !recent.iter().any(|&(_, m)| m == n) {
            recent.push((t >= fresh, n));
        }
    }
    // Fresh losses first (stable: newest first within each group).
    recent.sort_by_key(|&(f, _)| !f);
    for (f, n) in recent {
        if owner_of(world, n) != Some(corp) {
            continue;
        }
        let want = if f { 3 } else { ice_target(world, n) };
        if ice_eff(world, n) >= want {
            continue;
        }
        let Some((cost, ..)) = ice_quote(world, Some(corp), n) else { continue };
        if world.purse(Some(corp)) - cost >= reserve && install_ice(world, Some(corp), n) {
            return;
        }
    }
}

/// A camera posted at `b` (condition > 0, upkeep paid), the lowest id.
pub fn camera_at(world: &World, b: EntityId) -> Option<EntityId> {
    crate::systems::assets::assets_at(world, b).iter().copied().find(|&a| {
        world.comp::<Asset>(a).is_some_and(|x| {
            x.kind == AssetKind::Camera && x.loc == AssetLoc::Posted(b) && x.condition > 0 && x.upkeep_arrears == 0
        })
    })
}

/// V27/V28 Secure: a tier-1 camera for the owned building (lowest id) with
/// no sensor (no robot, no camera) and `ice_target ≥ 1`, from the cheapest
/// Security Office that can sell it, while the treasury holds twice the
/// price and keeps the fleet reserve after it ([`secure_affords`]).
pub fn buy_camera(world: &mut World, corp: EntityId) {
    if !world.config.assets.enabled {
        return;
    }
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let want = c.buildings.iter().copied().find(|&b| {
        world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
            && crate::systems::robots::posted_robot(world, b).is_none()
            && camera_at(world, b).is_none()
            && node_of_building(world, b).is_some_and(|n| ice_target(world, n) >= 1)
    });
    let Some(b) = want else { return };
    let Some(list) = crate::systems::assets::list_price(world, AssetKind::Camera, 1) else { return };
    let office = world
        .buildings_of_kind(BuildingKind::SecurityOffice)
        .iter()
        .copied()
        .filter(|&o| crate::systems::assets::can_sell(world, o, AssetKind::Camera, 1))
        .map(|o| ((list as f32 * crate::systems::assets::seller_level(world, o)).round() as i64, o))
        .min();
    let Some((price, office)) = office else { return };
    if !secure_affords(world, corp, price, 2.0) {
        return;
    }
    let pick = crate::components::ShopPick { kind: AssetKind::Camera, tier: 1, used: None, upgrade: false };
    let note = format!("for Secure ({})", world.name_of(b));
    if let Ok(a) = crate::systems::assets::buy_noted(world, corp, office, &pick, Some(&note)) {
        crate::systems::assets::set_loc(world, a, AssetLoc::Posted(b));
    }
}

/// Days of upkeep a Hunker shed must save to repay re-buying the tier
/// (the corp's `ice_spend` window).
const HUNKER_PAYBACK_DAYS: i64 = 30;

/// V27 Hunker: while the purse is below the fleet reserve (`treasury_ref /
/// 4`), lower ICE one tier on an owned node whose tier saves, over
/// `HUNKER_PAYBACK_DAYS`, at least what re-installing it would cost
/// (`ice_price × self_install_frac`, the cheapest rebuy): the most excess
/// over `ice_target` first, then the largest saving, ties the lower id.
///
/// Plan deviation (phase 3 fix round): the plan sheds the node with the
/// most excess over `ice_target`, cash or not. With the loss term in
/// `ice_target` (14 days) and a Ledger's target following its purse, a
/// target fell, Hunker shed the tier and Secure bought it back days later:
/// on seed 42 Greenline's seeded Ledger ICE 2 was shed on day 1 (saving 1
/// coin a day), re-bought for 600 on day 5 and shed again on day 8, the
/// money its day-5 truck took at HEAD. `ice_target` now governs buys only;
/// at today's `ice_upkeep` [0, 0, 1, 2] no shed repays itself, so Hunker
/// keeps its ICE until phase 5 gives upkeep teeth.
pub fn hunker_ice(world: &mut World, corp: EntityId) {
    let reserve = fleet_reserve(world, corp);
    if world.purse(Some(corp)) >= reserve {
        return;
    }
    let cfg = &world.config.ice;
    let pick = corp_nodes(world, corp)
        .into_iter()
        .filter_map(|n| {
            let ice = profile(world, n).map(|p| p.ice).filter(|&i| i > 0)?;
            let saving = cfg.upkeep(ice) - cfg.upkeep(ice - 1);
            let rebuy = (cfg.price(ice).unwrap_or(0) as f32 * cfg.self_install_frac).round() as i64;
            let excess = i32::from(ice) - i32::from(ice_target(world, n));
            (saving > 0 && saving * HUNKER_PAYBACK_DAYS >= rebuy).then_some((excess, saving, std::cmp::Reverse(n)))
        })
        .max();
    if let Some((_, _, std::cmp::Reverse(n))) = pick {
        lower_ice(world, n);
    }
}

/// V33: a live hack of `effect` on building `b`.
pub fn hack_live(world: &World, b: EntityId, effect: crate::virt::HackEffect) -> bool {
    world.comp::<Building>(b).and_then(|bd| bd.hacked).is_some_and(|(e, until)| e == effect && until > world.tick)
}

/// V32/V33: the building's robot sensors are off (Blind, or DoorOpen: a
/// robot posted at an open door neither defends nor senses).
pub fn robot_sensors_off(world: &World, b: EntityId) -> bool {
    hack_live(world, b, crate::virt::HackEffect::Blind) || hack_live(world, b, crate::virt::HackEffect::DoorOpen)
}

/// V33: the building's cameras are off (Blind only; DoorOpen opens the
/// door and leaves the cameras on).
pub fn cameras_off(world: &World, b: EntityId) -> bool {
    hack_live(world, b, crate::virt::HackEffect::Blind)
}

/// V28: a sighting into `owner`'s database (`None` = the city), capped at
/// `db_cap`; a corp's or the city's sighting of an agent with an open
/// report refreshes `last_seen` (the law reads it).
pub fn record_sighting(world: &mut World, owner: Option<EntityId>, s: crate::virt::Sighting) {
    let cap = world.config.db.db_cap.max(1);
    let db = world.db.entry(owner.unwrap_or(EntityId::NONE)).or_default();
    db.sightings.push_back(s);
    // M15 W12: the oldest relayed street sighting goes before any trace or
    // camera sighting (none exist with the word off: the M14 rule).
    while db.sightings.len() > cap {
        let i = db.sightings.iter().position(|x| x.relayed).unwrap_or(0);
        db.sightings.remove(i);
    }
    world.stats.current.virt.sightings += 1;
    let lawful = owner.is_none_or(|o| world.has::<Corp>(o));
    if lawful && crate::systems::law::wanted(world, s.who) {
        let newer = world.last_seen().get(&s.who).is_none_or(|&(_, t)| t <= s.tick);
        if newer {
            world.set_last_seen(s.who, (s.tile, s.tick));
        }
    }
}

/// V28, from `law::raise_crime`: a Theft, Shakedown or Grand Theft inside a
/// building with a camera (not Blind) rolls the thief's tier against the
/// camera's effective tier on the world stream (`security::contest_p`, one
/// draw), the camera's side scaled by the owner's `alertness_mult` (the M16
/// hook every detection roll reads; at 1.0 the draw is `security::contest`'s).
/// The camera wins: a sighting (confidence the camera's odds) and, for a
/// corp or the city, a report with no witness.
pub fn camera_sense(world: &mut World, actor: EntityId, crime: crate::components::Crime, b: EntityId) {
    use crate::components::Crime;
    if !matches!(crime, Crime::Theft | Crime::Extortion | Crime::GrandTheft) {
        return;
    }
    let Some(cam) = camera_at(world, b) else { return };
    if cameras_off(world, b) {
        return;
    }
    let cam_tier = world.comp::<Asset>(cam).map_or(1, |x| crate::systems::assets::eff_tier_of(world, x));
    let thief = crate::systems::security::thief_tier(crate::systems::law::stealth(world, actor));
    let step = contest_step(world);
    let owner = world.owner_of(b);
    let p_thief = crate::systems::security::contest_p(f32::from(thief), f32::from(cam_tier), step);
    let alert = alertness_mult(world, owner);
    // The thief slips by on `roll < p_thief` (M13's draw); alertness widens
    // or narrows the camera's side from the top. At exactly 1.0 the compare
    // is against `p_thief` itself, so `1 − (1 − p)` cannot differ from M13's
    // draw by an ulp.
    let roll: f32 = world.rng.world().random();
    #[allow(clippy::float_cmp)]
    let slips = if alert == 1.0 { roll < p_thief } else { roll < 1.0 - ((1.0 - p_thief) * alert).clamp(0.0, 1.0) };
    if slips {
        return;
    }
    let confidence = 1.0 - p_thief;
    let tile = world.comp::<Position>(actor).map(|p| p.tile).unwrap_or_default();
    if owner.is_none_or(|o| world.has::<Corp>(o)) {
        crate::systems::law::file_report(world, crime, actor, None);
    }
    let tick = world.tick;
    record_sighting(world, owner, crate::virt::Sighting { who: actor, tile, tick, confidence, relayed: false });
}
