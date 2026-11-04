use super::*;

// ---------------------------------------------------------------------------
// Seeding (the plan's Seeding table, V26, V47)
// ---------------------------------------------------------------------------

/// Last in `World::new`, after `assets::seed_sellers`: each row's Lab on
/// the vacant Lot of its district nearest the centroid (Lot door
/// Manhattan, ties lower id), owned by the row's corp, `focus` set, four
/// Researcher vacancies posted (`build_on_lot`), `seed_lab_grant` to the
/// owner out of nothing (V47), then a relink and `seed_store` Data in the
/// focus track. Returns the Labs built, in row order. A no-op with the
/// plane off.
pub fn seed_labs(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    let copies = world.lot_copies();
    // M17 phase 5: the seeded Labs are `[corps] seed_labs` (owner corp by
    // its row's template, Scale S4: copy `j` goes to the template's `j`-th
    // copy; district by role: the Spire, the Civic, the Vats, Mid West on the
    // shipped map; focus), keyed by row, never by name.
    let sites = world.config.corps.seed_labs.clone();
    for (copy, site) in (0..copies).flat_map(|j| sites.iter().map(move |r| (j, r))) {
        let (seat, focus) = (Seat::new(site.zone, 0.0), site.track);
        // Review finding 7: copy `j` is the row's `j`th corp (none: skipped,
        // never another copy's owner).
        let Some(template) = world.config.corps.row_template(site.row) else { continue };
        let Some(&corp) = world.corps_of_template(template).get(copy) else {
            continue;
        };
        let corp_name = world.comp::<Corp>(corp).map_or_else(String::new, |c| c.name.clone());
        let Some(d) = crate::systems::districts::seat_district(world, seat, copy, copies) else {
            continue;
        };
        let Some((district, centroid)) = world.districts.get(d.index()).map(|x| (x.name.clone(), x.centroid)) else {
            continue;
        };
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter(|&l| world.district_of_building(l) == d)
            .filter_map(|l| world.comp::<Building>(l).map(|b| (b.door.manhattan(centroid), l)))
            .min()
            .map(|(_, l)| l);
        let Some(lot) = lot else { continue };
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Lab, Some(corp)).is_err() {
            continue;
        }
        if let Some(bd) = world.comp_mut::<Building>(lot) {
            bd.focus = Some(focus);
        }
        let grant = world.config.tech.seed_lab_grant;
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.treasury += grant;
            c.closing = c.treasury;
        }
        built.push(lot);
        let rect = world.comp::<Building>(lot).map(|b| b.rect);
        let text = format!(
            "{corp_name} opened {} ({focus}) in {district} (seeded; Lot {} {rect:?})",
            world.name_of(lot),
            lot.index
        );
        world.push_event(EventKind::Founded, &[corp, lot], text);
    }
    relink(world);
    let store = world.config.data.seed_store;
    for &lab in &built {
        let focus = world.comp::<Building>(lab).and_then(|b| b.focus).unwrap_or_default();
        if let Some(n) = node_of_building(world, lab) {
            if let Some(node) = world.virt.node_mut(n) {
                node.store.units[focus.index()] = store;
            }
        }
    }
    built
}

/// V26: the opening ICE (`ice_maker = None`, the city's own): building
/// nodes `ice_seed[kind]`, corp Ledgers one tier per `ice_value_steps`
/// step their treasury reaches, the Treasury and the Precinct
/// `levers.city_ice`. Not an event (`ice_raised` counts installs only).
pub fn seed_ice(world: &mut World) {
    let jail = world.building_of_kind(BuildingKind::Jail);
    let hall = world.building_of_kind(BuildingKind::Hall);
    let city_ice = world.levers.city_ice;
    let steps = world.config.ice.ice_value_steps.clone();
    for i in 0..world.virt.nodes.len() {
        let n = NodeId(i as u16);
        let Some(node) = world.virt.node(n).filter(|x| x.alive) else { continue };
        let ice = match node.kind {
            NodeKind::Public(_) => continue,
            NodeKind::Building(b) if Some(b) == jail => city_ice,
            NodeKind::Building(b) => {
                world.comp::<Building>(b).map_or(0, |bd| world.config.ice.ice_seed.for_kind(bd.kind))
            }
            NodeKind::Ledger(c) if Some(c) == hall => city_ice,
            NodeKind::Ledger(c) => {
                let t = world.purse(Some(c));
                steps.iter().filter(|&&s| t >= s).count() as u8
            }
            // M17 phase 4: an uplink's ICE is its parent's (`ice_eff`).
            NodeKind::Uplink(_) => continue,
        };
        if let Some(p) = profile_mut(world, n) {
            *p = SecurityProfile { ice: ice.min(3), ice_maker: None, ice_arrears: 0 };
        }
    }
    bump_epoch(world);
}

// ---------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------

/// The alive node of building `b` (a Lab, a Hideout, a faction building, the Precinct).
pub fn node_of_building(world: &World, b: EntityId) -> Option<NodeId> {
    world.virt.of_building.get(&b).copied()
}

/// District `d`'s Public node.
pub fn public_of(world: &World, d: DistrictId) -> NodeId {
    world.virt.public_of.get(d.index()).copied().unwrap_or_default()
}

/// A faction's Ledger node (a corp; the Hall for the Treasury).
pub fn ledger_of(world: &World, owner: EntityId) -> Option<NodeId> {
    world.virt.ledger_of.get(&owner).copied()
}

/// The node's faction (`None` = the city).
pub fn owner_of(world: &World, n: NodeId) -> Option<EntityId> {
    world.virt.node(n).and_then(|x| x.owner)
}

/// The building a node's ICE lives on: its building, or the Hall for the
/// Treasury. `None` for a Public node or a corp Ledger.
pub(super) fn profile_building(world: &World, n: NodeId) -> Option<EntityId> {
    match world.virt.node(n)?.kind {
        NodeKind::Public(_) | NodeKind::Uplink(_) => None,
        NodeKind::Building(b) => Some(b),
        NodeKind::Ledger(c) => (!world.has::<Corp>(c)).then_some(c),
    }
}

/// V25: where a node's ICE lives (a building, `Corp.ledger_ice`, the Hall).
pub fn profile(world: &World, n: NodeId) -> Option<&SecurityProfile> {
    match world.virt.node(n)?.kind {
        NodeKind::Public(_) => None,
        NodeKind::Ledger(c) if world.has::<Corp>(c) => world.comp::<Corp>(c).map(|x| &x.ledger_ice),
        _ => profile_building(world, n).and_then(|b| world.comp::<Building>(b)).map(|bd| &bd.security),
    }
}

pub fn profile_mut(world: &mut World, n: NodeId) -> Option<&mut SecurityProfile> {
    match world.virt.node(n)?.kind {
        NodeKind::Public(_) => None,
        NodeKind::Ledger(c) if world.has::<Corp>(c) => world.comp_mut::<Corp>(c).map(|x| &mut x.ledger_ice),
        _ => {
            let b = profile_building(world, n)?;
            world.comp_mut::<Building>(b).map(|bd| &mut bd.security)
        }
    }
}

/// A corp's tier in a track; a maker that is gone reads `orphan_cap`, an
/// unset tree (a pre-M14 save before migration) reads 3 (uncapped).
pub fn maker_tier(world: &World, maker: EntityId, t: Track) -> u8 {
    match world.comp::<Corp>(maker) {
        Some(c) if c.tech.is_unset() => 3,
        // M17 phase 4: a parent's tech ceiling caps its branches.
        Some(c) => c.tech.tier_of(t).min(crate::systems::parents::tech_ceiling(world, c, t)),
        None => world.config.tech.orphan_cap,
    }
}

/// V25: 0 when its upkeep is `ice_off_days` behind or a `DoorOpen` hack is
/// live on its building; else `min(ice, the maker's Deck tier)` (the city's
/// own ICE, `ice_maker = None`, is uncapped).
pub fn ice_eff(world: &World, n: NodeId) -> u8 {
    // M17 phase 4: an uplink's ICE is `uplink_ice` capped by its parent's Deck tier.
    if let Some(NodeKind::Uplink(p)) = world.virt.node(n).map(|x| x.kind) {
        return crate::systems::parents::uplink_ice(world, p);
    }
    let Some(p) = profile(world, n) else { return 0 };
    if p.ice == 0 || p.ice_arrears >= world.config.ice.ice_off_days {
        return 0;
    }
    let door_open = profile_building(world, n)
        .and_then(|b| world.comp::<Building>(b))
        .and_then(|bd| bd.hacked)
        .is_some_and(|(e, until)| e == crate::virt::HackEffect::DoorOpen && until > world.tick);
    if door_open {
        return 0;
    }
    // Jobs & lives phase 1 (§ 2): ICE needs power; a dark district's is off.
    if profile_building(world, n).is_some_and(|b| crate::systems::industry::dark_at(world, b)) {
        return 0;
    }
    match p.ice_maker {
        None => p.ice,
        Some(m) => p.ice.min(maker_tier(world, m, Track::Deck)),
    }
}

/// V7: what a runner meets: `ice_eff + 1` while the node's alarm stands.
pub fn def(world: &World, n: NodeId) -> u8 {
    let alarm = world.virt.node(n).and_then(|x| x.alarm_until).is_some_and(|t| t > world.tick);
    ice_eff(world, n) + u8::from(alarm)
}

/// V27: a Lab's or Hideout's `Σ store × data_price`, a Ledger's purse, any
/// other building's `ownership::value`. A Public node risks nothing.
///
/// Plan deviation (phase 3 fix round): the plan adds the value of the
/// assets posted at an "other" building. Runs only target Labs, Hideouts,
/// Ledgers and robots, so a Farm's posted truck lifted its target to 2 and
/// Secure spent 600 on ICE nobody contests: the Food corps' truck money in
/// days 0-30 (the M13 gate's truck bullets). A robbed building is still
/// raised through the loss term of [`ice_target`].
pub fn value_at_risk(world: &World, n: NodeId) -> i64 {
    let Some(node) = world.virt.node(n) else { return 0 };
    match node.kind {
        NodeKind::Public(_) => 0,
        NodeKind::Uplink(_) => i64::from(node.store.total()) * world.config.data.data_price,
        NodeKind::Ledger(c) if world.has::<Corp>(c) => world.purse(Some(c)),
        NodeKind::Ledger(_) => world.purse(None),
        NodeKind::Building(b) => {
            let Some(bd) = world.comp::<Building>(b) else { return 0 };
            if matches!(bd.kind, BuildingKind::Lab | BuildingKind::Hideout) {
                return i64::from(node.store.total()) * world.config.data.data_price;
            }
            ownership::value(world, bd.kind)
        }
    }
}

/// V27: one tier per `ice_value_steps` step the value at risk reaches,
/// plus (plan deviation, phase 3) one per Virt loss on the node in the
/// owner's last 14 days, at most two, capped at 3: a robbed Lab already at
/// its value target still gets raised (phase 5's "raised within 7 days").
pub fn ice_target(world: &World, n: NodeId) -> u8 {
    let v = value_at_risk(world, n);
    let base = world.config.ice.ice_value_steps.iter().filter(|&&s| v >= s).count();
    let horizon = world.tick.saturating_sub(14 * TICKS_PER_DAY);
    let robbed = owner_of(world, n)
        .and_then(|o| world.comp::<Corp>(o))
        .map_or(0, |c| c.virt_losses.iter().filter(|&&(t, x)| x == n && t >= horizon).count());
    (base + robbed.min(2)).min(3) as u8
}

/// A faction's holding in a track: the sum of `store[t]` over its alive nodes.
pub fn holding(world: &World, faction: EntityId, t: Track) -> u32 {
    world.virt.nodes.iter().filter(|n| n.alive && n.owner == Some(faction)).map(|n| n.store.get(t)).sum()
}

// ---------------------------------------------------------------------------
// ICE installs (V26)
// ---------------------------------------------------------------------------

/// The Security corp selling ICE at `tier`: Deck tier at least `tier`, the
/// lowest `ice_price[tier] × level(Security)`, ties the lower id.
fn ice_seller(world: &World, tier: u8) -> Option<(i64, EntityId)> {
    let base = world.config.ice.price(tier)?;
    world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (c, cc)))
        .filter(|(_, cc)| cc.niches.contains(&Niche::Security) && !cc.tech.is_unset())
        .filter(|(_, cc)| cc.tech.tier_of(Track::Deck) >= tier)
        .map(|(c, cc)| ((base as f32 * cc.level(Niche::Security)).round() as i64, c))
        .min()
}

/// V26: raise `n`'s ICE one tier, paid by `owner`: self-installed at
/// `self_install_frac` of the cheapest seller's price (to the City,
/// `Flow::Import`) when the owner is a corp whose own Deck tier suffices,
/// else bought from the cheapest Security corp that can sell the tier
/// (`Flow::Contract`). `ice_maker` is the installer. A gang or an agent
/// pays in full or not at all. `IceRaised`, `Corp.ice_spend_today`,
/// `stats.ice_spend`. False when nothing was installed.
/// V26: what raising `n` one tier costs `owner`: `(cost, payee, maker,
/// flow, tier)`; `None` at ICE 3, without a profile, or with no seller.
#[allow(clippy::type_complexity)]
pub(super) fn ice_quote(
    world: &World,
    owner: Option<EntityId>,
    n: NodeId,
) -> Option<(i64, Option<EntityId>, Option<EntityId>, Flow, u8)> {
    let cur = profile(world, n).map(|p| p.ice)?;
    if cur >= 3 {
        return None;
    }
    let tier = cur + 1;
    let base = world.config.ice.price(tier)?;
    let seller = ice_seller(world, tier);
    let own_deck = owner.and_then(|o| world.comp::<Corp>(o)).map_or(0, |c| c.tech.tier_of(Track::Deck));
    if own_deck >= tier {
        let cheapest = seller.map_or(base, |(p, _)| p);
        let cost = (cheapest as f32 * world.config.ice.self_install_frac).round() as i64;
        Some((cost, None, owner, Flow::Import, tier))
    } else {
        let (price, s) = seller?;
        Some((price, Some(s), Some(s), Flow::Contract, tier))
    }
}

pub fn install_ice(world: &mut World, owner: Option<EntityId>, n: NodeId) -> bool {
    let Some((cost, payee, maker, flow, tier)) = ice_quote(world, owner, n) else { return false };
    let full = matches!(ownership::owner_kind(world, owner), OwnerKind::City | OwnerKind::Corp(_));
    if !full && world.purse(owner) < cost {
        return false;
    }
    if flow == Flow::Import {
        // Real economy (plan E5): a self-installed tier's import.
        ownership::import(world, owner, cost, ownership::ImportWhy::Ice);
    } else {
        ownership::charge(world, owner, payee, cost, flow);
    }
    if let Some(p) = profile_mut(world, n) {
        p.ice = tier;
        p.ice_maker = maker;
        p.ice_arrears = 0;
    }
    if let Some(c) = owner.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.ice_spend_today += cost;
    }
    world.stats.current.virt.ice_spend += cost;
    world.stats.current.virt.ice_raised += 1;
    bump_epoch(world);
    let what = node_label(world, n);
    let (who, by) = (world.owner_label(owner), world.owner_label(maker));
    let building = profile_building(world, n).unwrap_or(EntityId::NONE);
    world.push_event(
        EventKind::IceRaised,
        &[owner.unwrap_or(EntityId::NONE), building],
        format!("{who} raised {what} to ICE {tier} ({by})"),
    );
    true
}

/// V27 Hunker: lower `n`'s ICE one tier (no refund). `IceLowered`.
pub fn lower_ice(world: &mut World, n: NodeId) -> bool {
    let Some(p) = profile_mut(world, n).filter(|p| p.ice > 0) else { return false };
    p.ice -= 1;
    let tier = p.ice;
    world.stats.current.virt.ice_lowered += 1;
    bump_epoch(world);
    let owner = owner_of(world, n);
    let what = node_label(world, n);
    let who = world.owner_label(owner);
    let building = profile_building(world, n).unwrap_or(EntityId::NONE);
    world.push_event(
        EventKind::IceLowered,
        &[owner.unwrap_or(EntityId::NONE), building],
        format!("{who} lowered {what} to ICE {tier}"),
    );
    true
}

/// M14 V42 `SetCityIce`: the Treasury's and the Precinct's ICE moved to
/// `levers.city_ice` one tier at a time (`install_ice` pays the Security
/// corp's price from the Treasury; `lower_ice` refunds nothing). An install
/// that finds no seller sets the tier directly (the city's own, free). The
/// plane off: nothing to do.
pub fn set_city_ice(world: &mut World) {
    virt_relink_if_dirty(world);
    let want = world.levers.city_ice.min(3);
    let mut nodes: SmallVec<[NodeId; 2]> = SmallVec::new();
    nodes.extend(world.building_of_kind(BuildingKind::Hall).and_then(|h| ledger_of(world, h)));
    nodes.extend(world.building_of_kind(BuildingKind::Jail).and_then(|j| node_of_building(world, j)));
    for n in nodes {
        for _ in 0..3 {
            let Some(cur) = profile(world, n).map(|p| p.ice) else { break };
            if cur == want {
                break;
            }
            if cur > want {
                if !lower_ice(world, n) {
                    break;
                }
            } else if !install_ice(world, None, n) {
                if let Some(p) = profile_mut(world, n) {
                    p.ice = cur + 1;
                    p.ice_maker = None;
                    p.ice_arrears = 0;
                }
                bump_epoch(world);
            }
        }
    }
}

/// `relink` when the plane's index is stale (a lever read before the first
/// tick).
fn virt_relink_if_dirty(world: &mut World) {
    if world.virt_dirty || world.virt.nodes.is_empty() {
        relink(world);
    }
}

/// M14 V42 god `Fry`: resolved at once, at command time. The seated agent
/// loses a contest at the next node of its route that is contested (ICE and
/// not its patron's own; the target once past the hops), logged like a
/// natural loss, with no dodge, no trace and a certain fry; `lose` then
/// rolls the kill on the run's own stream (`p_flatline` at ICE 3 with the
/// gap forced to `flatline_gap`, else `p_fry_kill[ice]`).
pub fn god_fry(world: &mut World, agent: EntityId) -> Result<String, String> {
    let id = *world.runner_of.get(&agent).ok_or("not jacked in")?;
    let mut r = world.runs.get(&id).cloned().ok_or("not jacked in")?;
    if matches!(r.purpose, Purpose::Overwatch(_)) {
        return Err("an overwatch stream meets no ICE".into());
    }
    let n = if r.phase == RunPhase::Hop {
        let ahead = r.route.iter().skip(usize::from(r.at) + 1).copied();
        ahead.chain([r.target]).find(|&x| ice_eff(world, x) > 0 && contested(world, x, r.patron)).unwrap_or(r.target)
    } else {
        r.target
    };
    if ice_eff(world, n) == 0 || !contested(world, n, r.patron) {
        return Err(format!("no contest at {}", node_label(world, n)));
    }
    let gap = (f32::from(def(world, n)) - r.att).max(world.config.ice.flatline_gap);
    let (_, draws) = contest_draws(world, r.id, r.contests, gap);
    let rolls = LossRolls { gap, fry: 0.0, dodge: 2.0, kill: draws.kill, trace: 2.0 };
    let what = node_label(world, n);
    let name = world.name_of(agent);
    r.log.push((world.tick, n, false));
    r.contests = r.contests.saturating_add(1);
    lose(world, r, n, rolls);
    Ok(format!("fried {name} in {what}"))
}

/// M14 V42 god `RunNow`: a `RunOrder { why: God }` for an agent with a deck
/// (promoted to Coarse); the Hack goal takes a standing order up at once and
/// the contest rules apply. `target` is a building (or, for a Ledger, a
/// building or corp whose owner's Ledger is meant).
pub fn god_run_now(world: &mut World, agent: EntityId, target: EntityId, purpose: Purpose) -> Result<String, String> {
    if !crate::systems::law::living(world, agent) || !world.has::<Brain>(agent) {
        return Err("no such agent".into());
    }
    if world.runner_of.contains_key(&agent) {
        return Err("already jacked in".into());
    }
    if !world.comp::<Kit>(agent).is_some_and(|k| k.deck.is_some() && k.deck_tier > 0) {
        return Err("no deck".into());
    }
    virt_relink_if_dirty(world);
    let node = match purpose {
        Purpose::Ledger => {
            let owner = if world.has::<Corp>(target) { Some(target) } else { world.owner_of(target) };
            owner.and_then(|o| ledger_of(world, o)).ok_or("the target has no Ledger")?
        }
        _ => node_of_building(world, target).ok_or("the target has no node")?,
    };
    // Phase 5: a Door on a building with a powered robot posted takes the
    // robot, as the Raid prelude does (V32).
    let purpose = match (purpose, crate::systems::robots::powered_robot(world, target)) {
        (Purpose::Door, Some(r)) => Purpose::Robot(r),
        (p, _) => p,
    };
    let patron = None;
    let chair = chair_for(world, agent, patron).ok_or("no chair")?.building;
    let now = world.tick;
    let order = RunOrder {
        patron,
        purpose,
        target: node,
        chair,
        not_before: now,
        expires: now + TICKS_PER_DAY,
        why: RunWhy::God,
        mode: mode_of(world, agent),
    };
    give_order(world, agent, order);
    Ok(format!("ordered {} onto {}", world.name_of(agent), node_label(world, node)))
}

/// "Vat Farm#12", "Zetatech's Ledger", "the Treasury", "Spire Public".
pub fn node_label(world: &World, n: NodeId) -> String {
    let Some(node) = world.virt.node(n) else { return format!("node {}", n.0) };
    match node.kind {
        NodeKind::Public(d) => format!("{} Public", world.district(d).name),
        NodeKind::Building(b) => world.name_of(b),
        NodeKind::Ledger(c) if world.has::<Corp>(c) => format!("{}'s Ledger", world.owner_label(Some(c))),
        NodeKind::Ledger(_) => "the Treasury".to_string(),
        NodeKind::Uplink(p) => {
            format!("{}'s uplink", world.outside.faction(p).map_or("an outside parent", |f| f.name.as_str()))
        }
    }
}
