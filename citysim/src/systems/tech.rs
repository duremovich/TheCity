//! M14 Data, Labs and the tech tree (docs/M14_VIRT.md § 4, § 5; plan phase
//! 1.5, V16, V19, V21-V23, V26, V35, V39).
//!
//! [`run`] is the daily pass at midnight (tick order: after `assets`):
//! relink, Lab production from the shift ledger, ICE upkeep, research
//! upkeep and decay, research, the expiries and the rolls. Every pass walks
//! nodes in `NodeId` order and corps ascending; nothing runs per tick. The
//! rest is event-driven: tier changes ([`gain_tier`], [`lose_tier`],
//! [`on_tier_change`]), wipes ([`wipe_store`]) and the gates' reads
//! ([`street_tier`], [`track_of`]).

use rand::Rng;

use crate::components::{AssetKind, Building, BuildingKind, Corp, CorpOrder, CorpShock, Job, Role, Skills};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::systems::virt;
use crate::time::TICKS_PER_DAY;
use crate::virt::{NodeId, NodeKind, Tech, Track};
use crate::world::World;

/// Plan V35: the hacking draws' stream key (`SimRng::keyed`), so neither
/// the world stream nor any agent's is touched.
pub const HACK_KEY: u64 = 0x4AC4 << 44;

/// `ice_spend` window (plan V27).
const SPEND_DAYS: usize = 30;

// ---------------------------------------------------------------------------
// The hacking skill (V35)
// ---------------------------------------------------------------------------

/// `hack_seed_scale × U(0,1)^3` from `id`'s keyed stream at the current
/// tick (the tick, as the Body's draw, keeps a reused index from repeating
/// its predecessor's draw).
pub fn draw_hacking(world: &World, id: EntityId) -> f32 {
    let mut r = world.rng.keyed(HACK_KEY ^ (u64::from(id.index) << 20) ^ world.tick);
    let u: f32 = r.random();
    world.config.decks.hack_seed_scale * u * u * u
}

/// A newborn's hacking: half the parents' mean, half its own draw.
pub fn child_hacking(world: &World, id: EntityId, mother: EntityId, father: EntityId) -> f32 {
    let parents: Vec<f32> = [mother, father]
        .iter()
        .filter_map(|&p| world.comp::<Skills>(p).map(|s| s.hacking))
        .filter(|&h| h >= 0.0)
        .collect();
    let own = draw_hacking(world, id);
    if parents.is_empty() {
        return own;
    }
    let mean = parents.iter().sum::<f32>() / parents.len() as f32;
    (0.5 * mean + 0.5 * own).clamp(0.0, 1.0)
}

/// Set `id`'s hacking from its keyed draw.
pub fn give_hacking(world: &mut World, id: EntityId, hacking: f32) {
    if let Some(s) = world.comp_mut::<Skills>(id) {
        s.hacking = hacking;
    }
}

// ---------------------------------------------------------------------------
// Seeds (V21, the Seeding table)
// ---------------------------------------------------------------------------

/// A corp's opening tree: `[tech] seed` by name (else `[1, 1, 1]`), focus by
/// its first niche (`focus_by_niche`).
pub fn seeded_tech(world: &World, corp: EntityId) -> Tech {
    let cfg = &world.config.tech;
    let Some(c) = world.comp::<Corp>(corp) else { return Tech::default() };
    let tiers = cfg.seed.get(&c.name).copied().unwrap_or([1, 1, 1]).map(|t| t.clamp(1, 3));
    let focus = c.niches.iter().next().map_or(Track::Industry, |&n| cfg.focus_by_niche.for_niche(n));
    Tech::seeded(tiers, focus)
}

/// Seed every corp's tree (`World::new`) or every unset one (a pre-M14
/// save, `migrate_legacy`). No RNG.
pub fn seed_corps(world: &mut World, only_unset: bool) {
    for c in world.corps() {
        if only_unset && world.comp::<Corp>(c).is_some_and(|cc| !cc.tech.is_unset()) {
            continue;
        }
        let t = seeded_tech(world, c);
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.tech = t;
        }
    }
}

// ---------------------------------------------------------------------------
// Tracks and gates (V22, V23)
// ---------------------------------------------------------------------------

/// The track that gates an asset kind: implants Chrome; robots, bridges
/// (and M14's decks and cameras) Deck; vehicles and packs Industry.
pub fn track_of(kind: AssetKind) -> Track {
    match kind {
        AssetKind::Implant(_) => Track::Chrome,
        AssetKind::Robot | AssetKind::Bridge => Track::Deck,
        AssetKind::Motorcycle | AssetKind::Car | AssetKind::Truck | AssetKind::Flyer | AssetKind::Pack => {
            Track::Industry
        }
    }
}

/// V22: the street tier of a track: the highest tier any living corp holds
/// in it, and that corp (highest tier, ties the lower id); `(1, None)` with
/// no corps.
pub fn street_tier(world: &World, track: Track) -> (u8, Option<EntityId>) {
    world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).filter(|cc| !cc.tech.is_unset()).map(|cc| (cc.tech.tier_of(track), c)))
        .min_by_key(|&(t, c)| (std::cmp::Reverse(t), c))
        .map_or((1, None), |(t, c)| (t, Some(c)))
}

// ---------------------------------------------------------------------------
// The daily pass (V39)
// ---------------------------------------------------------------------------

/// Midnight, with the plane on: relink, production, ICE upkeep, research
/// upkeep and decay, research, the expiries and the rolls.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 || !virt::enabled(world) {
        return;
    }
    virt::relink(world);
    produce(world);
    ice_upkeep(world);
    for c in world.corps() {
        research_upkeep(world, c);
        if world.comp::<Corp>(c).is_some_and(|cc| cc.order == CorpOrder::Research) {
            research(world, c);
        }
    }
    expire(world);
    roll(world);
}

/// V16: every Lab adds `round(data_per_shift × (0.5 + hacking))` per
/// Researcher shift worked yesterday (the shift ledger, so every LOD tier
/// produces the same) to its focus track, capped at `store_cap`.
pub fn produce(world: &mut World) {
    let cfg = world.config.data.clone();
    let mut by_lab: std::collections::BTreeMap<EntityId, u32> = Default::default();
    for &r in world.workers(Role::Researcher) {
        let Some(j) = world.comp::<Job>(r) else { continue };
        let Some(lab) = j.employer else { continue };
        let yesterday = j.shift_key_at(world.tick.saturating_sub(1));
        if j.last_shift_day != Some(yesterday) {
            continue;
        }
        let hacking = world.comp::<Skills>(r).map_or(0.0, |s| s.hacking.max(0.0));
        *by_lab.entry(lab).or_default() += (cfg.data_per_shift * (0.5 + hacking)).round().max(0.0) as u32;
    }
    for (lab, units) in by_lab {
        let Some(focus) =
            world.comp::<Building>(lab).filter(|b| b.kind == BuildingKind::Lab && !b.demolished).map(|b| b.focus)
        else {
            continue;
        };
        let Some(n) = virt::node_of_building(world, lab) else { continue };
        let track = focus.unwrap_or_default();
        let Some(node) = world.virt.node_mut(n) else { continue };
        let slot = &mut node.store.units[track.index()];
        let added = units.min(cfg.store_cap.saturating_sub(*slot));
        *slot += added;
        world.stats.current.virt.data_made += added;
    }
}

/// V26: per alive node with ICE, `ice_upkeep[ice]` to the City
/// (`Flow::IceUpkeep`): a corp always pays (it may go negative); a gang
/// or an agent pays in full or not at all (arrears + 1); the city's own
/// nodes are a ledger entry only.
pub fn ice_upkeep(world: &mut World) {
    for i in 0..world.virt.nodes.len() {
        let n = NodeId(i as u16);
        if !world.virt.nodes[i].alive {
            continue;
        }
        let Some(ice) = virt::profile(world, n).map(|p| p.ice).filter(|&ice| ice > 0) else { continue };
        let cost = world.config.ice.upkeep(ice);
        let owner = virt::owner_of(world, n);
        let paid = match ownership::owner_kind(world, owner) {
            OwnerKind::City => {
                ownership::ledger_only(world, Flow::IceUpkeep, cost);
                true
            }
            OwnerKind::Corp(_) => {
                ownership::charge(world, owner, None, cost, Flow::IceUpkeep);
                true
            }
            OwnerKind::Gang(_) | OwnerKind::Agent(_) => {
                world.purse(owner) >= cost && ownership::pay(world, owner, None, cost, Flow::IceUpkeep) == cost
            }
        };
        if let Some(p) = virt::profile_mut(world, n) {
            p.ice_arrears = if paid { 0 } else { p.ice_arrears.saturating_add(1) };
        }
    }
    virt::bump_epoch(world);
}

/// V21: per track at tier 2 or more, `upkeep_coins[tier]` (`Flow::Research`)
/// and `upkeep_data[tier]` units from the holding: both available, both are
/// paid and the lapse resets; short on either, nothing is paid and the
/// lapse grows; at `decay_days` the track drops a tier. Tier 1 never decays.
pub fn research_upkeep(world: &mut World, corp: EntityId) {
    for track in Track::ALL {
        let Some(tier) = world.comp::<Corp>(corp).map(|c| c.tech.tier_of(track)) else { return };
        if tier < 2 {
            continue;
        }
        let (coins, units) = (world.config.tech.upkeep_coins_at(tier), world.config.tech.upkeep_data_at(tier));
        let ok = world.purse(Some(corp)) >= coins && virt::holding(world, corp, track) >= units;
        if ok {
            ownership::charge(world, Some(corp), None, coins, Flow::Research);
            take_data(world, corp, track, units);
        }
        let decay = world.config.tech.decay_days;
        let Some(c) = world.comp_mut::<Corp>(corp) else { return };
        let lapse = &mut c.tech.lapse[track.index()];
        *lapse = if ok { 0 } else { lapse.saturating_add(1) };
        let lapsed = *lapse >= decay;
        if lapsed {
            lose_tier(world, corp, track, "lapse");
        }
    }
}

/// V21: under `Research`, up to `research_rate` units of the focus track's
/// holding above the backup reserve (`upkeep_data[tier] × backup_days`) go
/// into `progress`; at `tier_cost[next]` with `tier_coins[next]` in the
/// treasury the corp pays (`Flow::Research`) and gains the tier. A track at
/// tier 3 has no next tier: nothing is spent.
pub fn research(world: &mut World, corp: EntityId) {
    let cfg = world.config.tech.clone();
    let Some(focus) = world.comp::<Corp>(corp).map(|c| c.tech.focus) else { return };
    let tier = world.comp::<Corp>(corp).map_or(1, |c| c.tech.tier_of(focus));
    let next = usize::from(tier) + 1;
    let (Some(&cost), Some(&price)) = (cfg.tier_cost.get(next), cfg.tier_coins.get(next)) else { return };
    let reserve = cfg.upkeep_data_at(tier) * world.config.data.backup_days;
    let spare = virt::holding(world, corp, focus).saturating_sub(reserve);
    let units = take_data(world, corp, focus, cfg.research_rate.min(spare));
    world.stats.current.virt.research_spent += units;
    let progress = world.comp_mut::<Corp>(corp).map_or(0, |c| {
        c.tech.progress[focus.index()] += units;
        c.tech.progress[focus.index()]
    });
    if progress >= cost && world.purse(Some(corp)) >= price {
        ownership::charge(world, Some(corp), None, price, Flow::Research);
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.tech.progress[focus.index()] -= cost;
        }
        gain_tier(world, corp, focus);
    }
}

/// Midnight expiries: alarms, hacks (node and building), and the faction
/// databases' old sightings; run orders past their `expires`.
fn expire(world: &mut World) {
    let now = world.tick;
    let mut touched = false;
    for i in 0..world.virt.nodes.len() {
        let node = &mut world.virt.nodes[i];
        if node.alarm_until.is_some_and(|t| t <= now) {
            node.alarm_until = None;
            touched = true;
        }
        if node.hacked.is_some_and(|(_, t)| t <= now) {
            node.hacked = None;
            touched = true;
        }
    }
    let hacked: Vec<EntityId> = world.virt.of_building.keys().copied().collect();
    for b in hacked {
        if let Some(bd) = world.comp_mut::<Building>(b) {
            if bd.hacked.is_some_and(|(_, t)| t <= now) {
                bd.hacked = None;
                touched = true;
            }
        }
    }
    let horizon = now.saturating_sub(u64::from(world.config.db.sighting_days) * TICKS_PER_DAY);
    for db in world.db.values_mut() {
        while db.sightings.front().is_some_and(|s| s.tick < horizon) {
            db.sightings.pop_front();
        }
    }
    world.run_orders.retain(|_, o| o.expires > now);
    if touched {
        virt::bump_epoch(world);
    }
}

/// Roll each corp's `ice_spend` (30 days).
fn roll(world: &mut World) {
    for c in world.corps() {
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            let today = std::mem::take(&mut cc.ice_spend_today);
            cc.ice_spend.push_back(today);
            while cc.ice_spend.len() > SPEND_DAYS {
                cc.ice_spend.pop_front();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tier changes (V21, V23)
// ---------------------------------------------------------------------------

/// One tier up (at most 3): progress kept past the cost, `TechGained`.
pub fn gain_tier(world: &mut World, corp: EntityId, track: Track) {
    let Some(c) = world.comp_mut::<Corp>(corp) else { return };
    let t = &mut c.tech.tier[track.index()];
    if *t >= 3 {
        return;
    }
    *t += 1;
    let tier = *t;
    c.tech.lapse[track.index()] = 0;
    let name = c.name.clone();
    world.stats.current.virt.tech_gained += 1;
    world.push_event(EventKind::TechGained, &[corp], format!("{name} reached {track} {tier}"));
    on_tier_change(world, corp, track);
}

/// One tier down (never below 1): progress and lapse reset, `TechLost`,
/// `CorpShock::TechLost`.
pub fn lose_tier(world: &mut World, corp: EntityId, track: Track, why: &str) {
    let Some(c) = world.comp_mut::<Corp>(corp) else { return };
    let t = &mut c.tech.tier[track.index()];
    if *t <= 1 {
        c.tech.lapse[track.index()] = 0;
        return;
    }
    let lost = *t;
    *t -= 1;
    c.tech.progress[track.index()] = 0;
    c.tech.lapse[track.index()] = 0;
    let name = c.name.clone();
    world.stats.current.virt.tech_lost += 1;
    world.push_event(EventKind::TechLost, &[corp], format!("{name} lost {track} {lost} ({why})"));
    ownership::push_corp_shock(world, corp, CorpShock::TechLost);
    on_tier_change(world, corp, track);
}

/// Set a tier outright (god `SetTech`), one step at a time through
/// [`gain_tier`] / [`lose_tier`].
pub fn set_tier(world: &mut World, corp: EntityId, track: Track, tier: u8) {
    let tier = tier.clamp(1, 3);
    while let Some(cur) = world.comp::<Corp>(corp).map(|c| c.tech.tier_of(track)) {
        if cur < tier {
            gain_tier(world, corp, track);
        } else if cur > tier {
            lose_tier(world, corp, track, "god");
        } else {
            break;
        }
    }
}

/// V23: every agent holding an asset this corp made in this track is
/// re-kitted (its effective tier moved); route trees go stale (ICE caps).
/// O(assets), on the event only.
pub fn on_tier_change(world: &mut World, corp: EntityId, track: Track) {
    let mut agents: Vec<EntityId> = Vec::new();
    for a in crate::systems::assets::all_assets(world) {
        let Some(x) = world.comp::<crate::components::Asset>(a) else { continue };
        if x.maker == Some(corp) && track_of(x.kind) == track {
            agents.extend([x.loc.holder(), x.owner, x.keeper].into_iter().flatten());
        }
    }
    agents.sort_unstable();
    agents.dedup();
    for a in agents {
        crate::systems::assets::rekit(world, a);
    }
    virt::bump_epoch(world);
}

/// V23: the maker corp is gone: every agent holding something it made is
/// re-kitted (it now runs at `orphan_cap`). A no-op with the plane off.
pub fn on_maker_gone(world: &mut World, corp: EntityId) {
    if !virt::enabled(world) {
        return;
    }
    for track in Track::ALL {
        on_tier_change(world, corp, track);
    }
}

// ---------------------------------------------------------------------------
// Data (V19, V21)
// ---------------------------------------------------------------------------

/// Take up to `units` of `track` from `faction`'s stores: its Labs first
/// (ascending), then its other nodes. Returns the units taken.
pub fn take_data(world: &mut World, faction: EntityId, track: Track, units: u32) -> u32 {
    if units == 0 {
        return 0;
    }
    let is_lab = |w: &World, kind: NodeKind| match kind {
        NodeKind::Building(b) => w.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lab),
        _ => false,
    };
    let mine: Vec<(bool, usize)> = world
        .virt
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.alive && n.owner == Some(faction) && n.store.get(track) > 0)
        .map(|(i, n)| (!is_lab(world, n.kind), i))
        .collect();
    let mut order = mine;
    order.sort_unstable();
    let mut left = units;
    for (_, i) in order {
        if left == 0 {
            break;
        }
        let slot = &mut world.virt.nodes[i].store.units[track.index()];
        let take = left.min(*slot);
        *slot -= take;
        left -= take;
    }
    units - left
}

/// V19: a node's store to zero (`DataWiped`, `stats.data_wiped`); then, per
/// track the wipe touched, a corp owner whose remaining holding is under
/// `upkeep_data[tier] × backup_days` drops that track a tier at once.
pub fn wipe_store(world: &mut World, n: NodeId, by: Option<EntityId>) -> u32 {
    let Some(node) = world.virt.node_mut(n) else { return 0 };
    let before = std::mem::take(&mut node.store);
    let owner = node.owner;
    let total = before.total();
    world.stats.current.virt.data_wiped += total;
    let what = virt::node_label(world, n);
    let text = match by {
        Some(r) => format!("{what}'s Data was wiped by {}", world.name_of(r)),
        None => format!("{what}'s Data was wiped"),
    };
    world.push_event(
        EventKind::DataWiped,
        &[by.unwrap_or(EntityId::NONE), owner.unwrap_or(EntityId::NONE)],
        format!("{text} ({total} units)"),
    );
    let Some(corp) = owner.filter(|&o| world.has::<Corp>(o)) else { return total };
    let backup = world.config.data.backup_days;
    for track in Track::ALL {
        if before.get(track) == 0 {
            continue;
        }
        let tier = world.comp::<Corp>(corp).map_or(1, |c| c.tech.tier_of(track));
        if tier < 2 {
            continue;
        }
        if virt::holding(world, corp, track) < world.config.tech.upkeep_data_at(tier) * backup {
            lose_tier(world, corp, track, "wipe");
        }
    }
    total
}

/// God `GrantData`: `units` into the faction's first Lab (ascending), else
/// its Hideout's node. Returns the node, or why not.
pub fn grant_data(world: &mut World, faction: EntityId, track: Track, units: u32) -> Result<NodeId, String> {
    if !virt::enabled(world) {
        return Err("the plane is off".into());
    }
    let lab = world.virt.nodes.iter().enumerate().find_map(|(i, n)| {
        let NodeKind::Building(b) = n.kind else { return None };
        (n.alive
            && n.owner == Some(faction)
            && world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Lab))
        .then_some(NodeId(i as u16))
    });
    let n = lab
        .or_else(|| world.hideout_of(faction).and_then(|h| virt::node_of_building(world, h)))
        .ok_or_else(|| format!("{} has no Lab or Hideout node", world.owner_label(Some(faction))))?;
    if let Some(node) = world.virt.node_mut(n) {
        node.store.units[track.index()] = node.store.units[track.index()].saturating_add(units);
    }
    Ok(n)
}

/// V21 focus reset (the Research order): the track with the largest rival
/// tier gap (rivals share a niche; ties the track order), else the focus by
/// the first niche.
pub fn reset_focus(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let rivals: Vec<EntityId> = world
        .corps()
        .into_iter()
        .filter(|&r| r != corp)
        .filter(|&r| world.comp::<Corp>(r).is_some_and(|rc| rc.niches.iter().any(|n| c.niches.contains(n))))
        .collect();
    let gap = |t: Track| -> i32 {
        let best = rivals
            .iter()
            .filter_map(|&r| world.comp::<Corp>(r).map(|rc| i32::from(rc.tech.tier_of(t))))
            .max()
            .unwrap_or(0);
        best - i32::from(c.tech.tier_of(t))
    };
    let best = Track::ALL
        .into_iter()
        .map(|t| (gap(t), t))
        .filter(|&(g, _)| g > 0)
        .max_by_key(|&(g, t)| (g, std::cmp::Reverse(t)));
    let focus = match best {
        Some((_, t)) => t,
        None => seeded_tech(world, corp).focus,
    };
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.tech.focus = focus;
    }
}

/// V31 inputs: `(tech_gap, max_lapse)`: the largest rival tier lead in the
/// focus track ÷ 2, and the largest `lapse ÷ decay_days`.
pub fn research_inputs(world: &World, corp: EntityId) -> (f32, f32) {
    let Some(c) = world.comp::<Corp>(corp) else { return (0.0, 0.0) };
    let focus = c.tech.focus;
    let rival_best = world
        .corps()
        .into_iter()
        .filter(|&r| r != corp)
        .filter_map(|r| world.comp::<Corp>(r))
        .filter(|rc| rc.niches.iter().any(|n| c.niches.contains(n)))
        .map(|rc| rc.tech.tier_of(focus))
        .max()
        .unwrap_or(0);
    let gap = (f32::from(rival_best) - f32::from(c.tech.tier_of(focus))).max(0.0) / 2.0;
    let decay = f32::from(world.config.tech.decay_days.max(1));
    let lapse = c.tech.lapse.iter().map(|&l| f32::from(l) / decay).fold(0.0f32, f32::max);
    (gap.clamp(0.0, 1.0), lapse.clamp(0.0, 1.0))
}

/// A faction's Lab nodes (alive), ascending.
pub fn labs_of(world: &World, faction: EntityId) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Lab)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(faction) && !bd.demolished))
        .collect()
}
