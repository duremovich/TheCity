use super::*;

// ---------------------------------------------------------------------------
// Decks: gang buys (V30), corp fleet decks (V31), the snapshot (V43)
// ---------------------------------------------------------------------------

/// V24: the sellers of decks (Clinics and Security Offices), ascending.
pub fn deck_sellers(world: &World) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .chain(world.buildings_of_kind(BuildingKind::SecurityOffice))
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect();
    v.sort_unstable();
    v
}

/// The dearest new deck up to `max_tier` within `budget` across the deck
/// sellers (`can_sell`, priced at the seller's level), ties the cheaper
/// price, then nearest to `from`, then the lower id: `(seller, tier, price)`.
pub fn deck_offer(world: &World, from: TilePos, budget: i64, max_tier: u8) -> Option<(EntityId, u8, i64)> {
    type Key = (u8, std::cmp::Reverse<i64>, std::cmp::Reverse<u32>, std::cmp::Reverse<EntityId>);
    let mut best: Option<Key> = None;
    for s in deck_sellers(world) {
        let level = crate::systems::assets::seller_level(world, s);
        let dist = world.comp::<Building>(s).map_or(u32::MAX, |b| b.door.manhattan(from));
        for tier in 1..=max_tier.min(3) {
            let Some(list) = crate::systems::assets::list_price(world, AssetKind::Deck, tier) else { continue };
            let price = (list as f32 * level).round() as i64;
            if price > budget || !crate::systems::assets::can_sell(world, s, AssetKind::Deck, tier) {
                continue;
            }
            let key = (tier, std::cmp::Reverse(price), std::cmp::Reverse(dist), std::cmp::Reverse(s));
            if best.is_none_or(|b| key > b) {
                best = Some(key);
            }
        }
    }
    best.map(|(tier, std::cmp::Reverse(price), _, std::cmp::Reverse(s))| (s, tier, price))
}

/// V30, the gang buy step (after the bike and the Arms): one deck a day
/// while the treasury holds `gang_deck_floor` and the gang owns fewer than
/// `gang_decks_max`: the dearest tier priced within `treasury − floor`, for
/// the best-hacking member without one (at `hack_min` or better; ties the
/// lower id), owned by the gang, carried and kept by the member.
pub fn gang_deck(world: &mut World, gang: EntityId) {
    if !world.config.assets.enabled {
        return;
    }
    let floor = world.config.decks.gang_deck_floor;
    let treasury = world.purse(Some(gang));
    // The pressure pass phase 5b: over the gang's commitments, one a week.
    if !crate::systems::crew::gear_spare(world, gang, floor) {
        return;
    }
    let owned = crate::systems::assets::assets_of(world, Some(gang))
        .iter()
        .filter(|&&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Deck))
        .count();
    if owned >= world.config.decks.gang_decks_max as usize {
        return;
    }
    let min = world.config.hack.hack_min;
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let member = members
        .iter()
        .copied()
        .filter(|&m| world.has::<Brain>(m) && !world.has::<crate::components::Sentence>(m))
        .filter(|&m| world.comp::<Kit>(m).is_some_and(|k| k.deck.is_none()))
        .filter_map(|m| world.comp::<Skills>(m).map(|s| (s.hacking, m)))
        .filter(|&(h, _)| h >= min)
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, m)| m);
    let Some(member) = member else { return };
    let from = world.hideout_of(gang).and_then(|h| world.comp::<Building>(h)).map(|b| b.door).unwrap_or_default();
    let Some((seller, tier, _)) = deck_offer(world, from, treasury - floor, 3) else { return };
    let pick = crate::components::ShopPick { kind: AssetKind::Deck, tier, used: None, upgrade: false };
    let note = format!("for {}", world.name_of(member));
    if let Ok(a) = crate::systems::assets::buy_noted(world, gang, seller, &pick, Some(&note)) {
        crate::systems::assets::set_keeper(world, a, Some(member));
        crate::systems::assets::set_loc(world, a, AssetLoc::Carried(member));
    }
}

/// V31 (phase 2 part), daily under `Research`: one tier-1 fleet deck for
/// each of the corp's Labs with none posted (plan deviation: the tier is
/// left to phase 3's `VirtRaid`; a dearer pick spent 1,080 of Zetatech's day
/// 0 on two T3 decks, it then could not pay its Garage's truck imports and
/// the M13 gate's Farm trucks fell from 8 to 4), posted at the Lab
/// (`Posted`, not `Parked`: M13's street theft never sees it). Phase 3's `VirtRaid` lends it.
pub fn fleet_decks(world: &mut World, corp: EntityId) {
    if !world.config.assets.enabled {
        return;
    }
    for lab in crate::systems::tech::labs_of(world, corp) {
        let has = crate::systems::assets::assets_at(world, lab).iter().any(|&a| {
            world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Deck && x.loc == AssetLoc::Posted(lab))
        });
        if has {
            continue;
        }
        let budget = world.purse(Some(corp));
        let from = world.comp::<Building>(lab).map(|b| b.door).unwrap_or_default();
        let Some((seller, tier, _)) = deck_offer(world, from, budget, 1) else { return };
        let pick = crate::components::ShopPick { kind: AssetKind::Deck, tier, used: None, upgrade: false };
        let note = format!("for {}", world.name_of(lab));
        match crate::systems::assets::buy_noted(world, corp, seller, &pick, Some(&note)) {
            Ok(a) => crate::systems::assets::set_loc(world, a, AssetLoc::Posted(lab)),
            Err(_) => return,
        }
    }
}

/// V43: decks owned (any owner).
pub fn decks_owned(world: &World) -> u32 {
    world
        .assets_by_owner
        .values()
        .flatten()
        .filter(|&&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Deck))
        .count() as u32
}

/// V18: an arrest confiscates the Data on the suspect's deck (destroyed).
pub fn confiscate_data(world: &mut World, agent: EntityId) {
    if let Some(d) = world.comp::<Kit>(agent).and_then(|k| k.deck) {
        if let Some(a) = world.comp_mut::<Asset>(d) {
            a.data = [0; 3];
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 3: orders and targets (plan V13, V27, V28, V30-V33, V37, V38)
// ---------------------------------------------------------------------------

/// Days a traced run that named someone keeps a gang's `hacked` input up.
const HACKED_DAYS: Tick = 7;

/// V13: write `order` for `runner`, promote a Statistical runner to Coarse
/// at once, and re-kit (a corp order lends the Lab's fleet deck).
pub fn give_order(world: &mut World, runner: EntityId, order: RunOrder) {
    world.run_orders.insert(runner, order);
    if world.comp::<Brain>(runner).is_some_and(|b| b.lod == crate::components::Lod::Statistical) {
        crate::systems::lod::set_lod(world, runner, crate::components::Lod::Coarse);
    }
    crate::systems::assets::rekit(world, runner);
}

/// V30: a gang's members able to run, best hacking first (ties the lower
/// id): a deck in the Kit, `hacking ≥ hack_min`, free, not on a run.
pub fn gang_runners(world: &World, gang: EntityId) -> Vec<EntityId> {
    let min = world.config.hack.hack_min;
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let mut v: Vec<(f32, EntityId)> = g
        .members
        .iter()
        .copied()
        .filter(|&m| crate::systems::law::living(world, m) && !world.has::<crate::components::Sentence>(m))
        .filter(|&m| !world.runner_of.contains_key(&m))
        .filter(|&m| world.comp::<Kit>(m).is_some_and(|k| k.deck.is_some() && k.deck_tier > 0))
        .filter_map(|m| world.comp::<Skills>(m).map(|s| (s.hacking, m)))
        .filter(|&(h, _)| h >= min)
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter().map(|(_, m)| m).collect()
}

/// V30: who a traced run on the gang's nodes named lately (`Some(None)`: a
/// pending `Shock::Hacked` naming nobody).
fn gang_hacked(world: &World, gang: EntityId) -> Option<Option<EntityId>> {
    let g = world.comp::<Gang>(gang)?;
    let window = HACKED_DAYS * TICKS_PER_DAY;
    if let Some((by, _)) = g.hacked_by.filter(|&(_, t)| world.tick.saturating_sub(t) < window) {
        return Some(Some(by));
    }
    g.shocks.iter().any(|s| matches!(s, crate::components::Shock::Hacked { .. })).then_some(None)
}

/// V30: a gang runner's candidates: rival Hideout stores and corp Labs
/// (steal), the hoarding corp's Ledger (a deck at the Trunk tier), the
/// robot posted at the corp-raid prize building, and a wipe of whoever a
/// trace named (a corp's richest Lab, a gang's Hideout store).
fn gang_candidates(
    world: &World,
    gang: EntityId,
    deck_eff: u8,
    mode: RunMode,
    prize: Option<EntityId>,
) -> SmallVec<[(NodeId, Purpose, i64); 32]> {
    let mut out: SmallVec<[(NodeId, Purpose, i64); 32]> = SmallVec::new();
    let (take, _, _) = mode_terms(world, mode);
    let price = world.config.data.data_price;
    let units = steal_units(world, deck_eff);
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive && n.owner != Some(gang)) {
        let NodeKind::Building(b) = n.kind else { continue };
        let kind = world.comp::<Building>(b).map(|bd| bd.kind);
        let fits = match n.owner_kind {
            OwnerTag::Gang => kind == Some(BuildingKind::Hideout),
            OwnerTag::Corp => kind == Some(BuildingKind::Lab),
            OwnerTag::City => false,
        };
        if !fits || n.store.total() == 0 {
            continue;
        }
        let got = (n.store.total().min(units) as f32 * take).round() as i64;
        if got > 0 {
            out.push((NodeId(i as u16), Purpose::Data { wipe: false }, got * price));
        }
    }
    if deck_eff >= world.config.virt.trunk_links_tier {
        if let Some(c) = crate::systems::faction::hoard(world).1 {
            if let Some(l) = ledger_of(world, c) {
                let got = (ledger_take(world, world.purse(Some(c)), deck_eff) as f32 * take).round() as i64;
                if got > 0 {
                    out.push((l, Purpose::Ledger, got));
                }
            }
        }
    }
    if let Some(b) = prize {
        if let (Some(r), Some(n)) = (crate::systems::robots::powered_robot(world, b), node_of_building(world, b)) {
            let v = world.comp::<Asset>(r).map_or(0, |x| x.value) / 2;
            out.push((n, Purpose::Robot(r), v.max(1)));
        }
    }
    match gang_hacked(world, gang) {
        Some(Some(by)) if world.has::<Corp>(by) => {
            let lab = crate::systems::tech::labs_of(world, by)
                .into_iter()
                .filter_map(|l| node_of_building(world, l))
                .filter_map(|n| world.virt.node(n).map(|x| (x.store.total(), std::cmp::Reverse(n))))
                .max()
                .filter(|&(u, _)| u > 0);
            if let Some((u, std::cmp::Reverse(n))) = lab {
                out.push((n, Purpose::Data { wipe: true }, i64::from(u) * price));
            }
        }
        Some(Some(by)) if world.has::<Gang>(by) && by != gang => {
            if let Some(n) = world.hideout_of(by).and_then(|h| node_of_building(world, h)) {
                let u = world.virt.node(n).map_or(0, |x| x.store.total());
                if u > 0 {
                    out.push((n, Purpose::Data { wipe: true }, i64::from(u) * price));
                }
            }
        }
        _ => {}
    }
    out
}

/// One ranked target: `(EV × p, node, purpose, EV, p_success)`.
type Ranked = (f32, NodeId, Purpose, i64, f32);

/// V6/V30: the candidates read off one search from `from`, at `p_success ≥
/// min_route_p`, best `EV × p` first (ties the lower `NodeId`).
pub(super) fn rank(
    world: &World,
    from: NodeId,
    o: &Odds,
    deck_eff: u8,
    cands: &[(NodeId, Purpose, i64)],
) -> Vec<Ranked> {
    let tree = routes_from(world, from, deck_eff, o.att, o.patron);
    let min = world.config.virt.min_route_p;
    let mut v: Vec<Ranked> = cands
        .iter()
        .filter(|&&(n, _, _)| n != from)
        .filter_map(|&(n, p, ev)| route_to(world, &tree, from, n, o).map(|r| (n, p, ev, r.p_success)))
        .filter(|&(_, _, _, ps)| ps >= min)
        .map(|(n, p, ev, ps)| (ev as f32 * ps, n, p, ev, ps))
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    v
}

/// V30's VirtRaid inputs for a gang (see [`gang_virt_inputs`]).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct GangVirtInputs {
    /// The gang's best runner (`None`: nobody can run).
    pub runner: Option<EntityId>,
    /// The best target's EV ÷ `hoard_heat` (0..1) and its `p_success`.
    pub ev: f32,
    pub p: f32,
    /// A traced run on the gang's nodes named someone lately.
    pub hacked: bool,
    /// M14 review: the grudge's wipe (the store of whoever a trace named)
    /// is among the best runner's targets at `p_success ≥ min_route_p`.
    pub grudge: bool,
}

/// V30 inputs for the best target of the gang's best runner from its
/// Hideout node (one search), and whether the grudge's wipe is reachable.
pub fn gang_virt_inputs(world: &World, gang: EntityId, prize: Option<EntityId>) -> GangVirtInputs {
    let hacked = gang_hacked(world, gang).is_some();
    let none = GangVirtInputs { hacked, ..Default::default() };
    let Some(runner) = gang_runners(world, gang).first().copied() else { return none };
    let none = GangVirtInputs { runner: Some(runner), ..none };
    let Some(from) = world.hideout_of(gang).and_then(|h| node_of_building(world, h)) else { return none };
    let deck_eff = world.comp::<Kit>(runner).map_or(0, |k| k.deck_tier);
    let mode = mode_of(world, runner);
    let o = odds_of(world, runner, deck_eff, Some(gang), mode);
    let cands = gang_candidates(world, gang, deck_eff, mode, prize);
    let ranked = rank(world, from, &o, deck_eff, &cands);
    let heat = world.config.corps.hoard_heat.max(1) as f32;
    let grudge = ranked.iter().any(|t| t.2 == Purpose::Data { wipe: true });
    match ranked.first() {
        Some(&(_, _, _, ev, p)) => GangVirtInputs { ev: (ev as f32 / heat).clamp(0.0, 1.0), p, grudge, ..none },
        None => none,
    }
}

/// V30, daily in the gang economy: tier-1 ICE on the Hideout node while its
/// store is worth `ice_value_steps[0]`; then, under `VirtRaid`, up to
/// `virt_runners` runners each get a `RunOrder` (chair the Hideout, patron
/// the gang) on a distinct target, best first.
pub fn gang_daily(world: &mut World, gang: EntityId) {
    let Some(hideout) = world.hideout_of(gang) else { return };
    let Some(from) = node_of_building(world, hideout) else { return };
    let worth = value_at_risk(world, from);
    let step = world.config.ice.ice_value_steps.first().copied().unwrap_or(i64::MAX);
    if worth >= step && profile(world, from).is_some_and(|p| p.ice == 0) {
        install_ice(world, Some(gang), from);
    }
    if world.comp::<Gang>(gang).is_none_or(|g| g.order != crate::components::Order::VirtRaid) {
        return;
    }
    let prize = crate::systems::raid::corp_target(world, gang);
    let now = crate::systems::midnight::stamp(world);
    let mut taken: Vec<NodeId> = Vec::new();
    let max = world.config.hack.virt_runners as usize;
    let runners: Vec<EntityId> =
        gang_runners(world, gang).into_iter().filter(|r| !world.run_orders.contains_key(r)).take(max).collect();
    for runner in runners {
        let deck_eff = world.comp::<Kit>(runner).map_or(0, |k| k.deck_tier);
        let mode = mode_of(world, runner);
        let o = odds_of(world, runner, deck_eff, Some(gang), mode);
        let cands = gang_candidates(world, gang, deck_eff, mode, prize);
        // M14 review: the grudge's wipe first (a wipe is a candidate only
        // on a grudge), then the best `EV × p`.
        let ranked = rank(world, from, &o, deck_eff, &cands);
        let free = |t: &&Ranked| !taken.contains(&t.1);
        let wipe = ranked.iter().filter(free).find(|t| t.2 == Purpose::Data { wipe: true });
        let pick = wipe.or_else(|| ranked.iter().find(free)).copied();
        let Some((_, target, purpose, _, _)) = pick else { continue };
        taken.push(target);
        let order = RunOrder {
            patron: Some(gang),
            purpose,
            target,
            chair: hideout,
            not_before: now,
            expires: now + TICKS_PER_DAY,
            why: RunWhy::GangOrder,
            mode,
        };
        give_order(world, runner, order);
    }
}

/// V32, when M12's Raid names a corp building and the muster: the gang's
/// best runner gets a `Robot` run (a powered robot posted there) or a `Door`
/// run (ICE ≥ 1 or a contract; not within `door_cooldown_days` of the last
/// DoorOpen), from the Hideout chair, `door_lead_hours` before the muster.
pub fn raid_prelude(world: &mut World, gang: EntityId, target: EntityId, raid_at: Tick) {
    let Some(n) = node_of_building(world, target) else { return };
    let Some(hideout) = world.hideout_of(gang) else { return };
    let robot = crate::systems::robots::powered_robot(world, target);
    let (ice, contract, last) = world
        .comp::<Building>(target)
        .map_or((0, false, None), |b| (b.security.ice, b.secured_by.is_some(), b.last_door_open));
    let cool = Tick::from(world.config.ice.door_cooldown_days) * TICKS_PER_DAY;
    let door_ok = (ice >= 1 || contract) && last.is_none_or(|t| world.tick.saturating_sub(t) >= cool);
    let purpose = match robot {
        Some(r) => Purpose::Robot(r),
        None if door_ok => Purpose::Door,
        None => return,
    };
    let Some(runner) = gang_runners(world, gang).first().copied() else { return };
    let lead = Tick::from(world.config.hack.door_lead_hours) * TICKS_PER_HOUR;
    let order = RunOrder {
        patron: Some(gang),
        purpose,
        target: n,
        chair: hideout,
        not_before: raid_at.saturating_sub(lead).max(world.tick),
        expires: raid_at + Tick::from(world.config.hack.door_hours) * TICKS_PER_HOUR,
        why: RunWhy::Prelude,
        mode: RunMode::Quiet,
    };
    give_order(world, runner, order);
}

/// V37, the first member out of the muster: a member with a deck and
/// `hacking ≥ stream_min` inside the Hideout and not marching takes an
/// `Overwatch` run; `Gang.stream_by`. A corp raid leaving through an open
/// door logs it (the V32 gate pair).
pub fn on_raid_departed(world: &mut World, gang: EntityId) {
    let now = world.tick;
    if let Some(t) = crate::systems::raid::corp_target(world, gang) {
        if hack_live(world, t, crate::virt::HackEffect::DoorOpen) {
            let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
            let text = format!("{gname}'s raid departs for {} through open doors", world.name_of(t));
            world.push_event(EventKind::Raid, &[gang, t], text);
        }
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let min = world.config.hack.stream_min;
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let streamer = members.into_iter().find(|&m| {
        crate::systems::law::living(world, m)
            && !world.runner_of.contains_key(&m)
            && !world.run_orders.contains_key(&m)
            && world.comp::<Kit>(m).is_some_and(|k| k.deck.is_some())
            && world.comp::<Skills>(m).is_some_and(|s| s.hacking >= min)
            && world.comp::<Position>(m).is_some_and(|p| p.building == Some(hideout))
            && world.comp::<Brain>(m).is_some_and(|b| b.current_goal != Some(GoalKind::Raid))
    });
    let Some(m) = streamer else { return };
    let order = RunOrder {
        patron: Some(gang),
        purpose: Purpose::Overwatch(gang),
        target: node_of_building(world, hideout).unwrap_or_default(),
        chair: hideout,
        not_before: now,
        expires: now + crate::systems::faction::march_ticks(world),
        why: RunWhy::Overwatch,
        mode: RunMode::Quiet,
    };
    give_order(world, m, order);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.stream_by = Some(m);
    }
}
