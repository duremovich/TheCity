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

/// Seed every corp's tree (`World::new`), or every unset one. No RNG.
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
        AssetKind::Robot | AssetKind::Bridge | AssetKind::Deck | AssetKind::Camera => Track::Deck,
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
    if world.tick_of_day() != 0 {
        return;
    }
    // Phase 3: a new day's Data budget for every corp.
    for c in world.corps() {
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.data_bought_today = 0;
        }
    }
    virt::relink(world);
    produce(world);
    ice_upkeep(world);
    for c in world.corps() {
        research_upkeep(world, c);
        if world.comp::<Corp>(c).is_some_and(|cc| cc.order == CorpOrder::Research) {
            research(world, c);
            sell_spare(world, c);
        }
    }
    gang_sales(world);
    // V38: the Statistical hack pass, after production and the sales.
    virt::stat_pass(world);
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
    for (lab, mut units) in by_lab {
        // M15 W28: × the owner's competence multiplier (1 when off).
        if let Some(c) = world.corp_of_building(lab) {
            let m = crate::systems::competence::comp_mult(world, c);
            if m != 1.0 {
                units = (units as f32 * m).round().max(0.0) as u32;
            }
        }
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
        // Real economy phase 2 (plan E13): inputs to the World per Data unit made.
        let per = world.config.economy2.input_per_data;
        crate::systems::wages::produce_inputs(world, lab, added, per);
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
                // Phase 5: a corp's ICE spend (the panel's and the gate's
                // 30-day window) counts its upkeep as well as its installs.
                if let Some(c) = owner.and_then(|o| world.comp_mut::<Corp>(o)) {
                    c.ice_spend_today += cost;
                }
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
///
/// Phase 5 (calibration, the handoff's "upkeep teeth"): the coins count as
/// available only while paying them leaves M13's fleet reserve
/// (`treasury_ref / 4`, the floor every M14 corp spend keeps) in the
/// treasury. A corp sliding toward bankruptcy stops paying its licences
/// before anything else and lapses: the spec's "a corp that let its Lab
/// budget slide", without the upkeep itself pushing anyone under. With no
/// coins due the floor is 0 (a corp in the red still lapses, as before):
/// the shipped `upkeep_coins` is 0 (every coin cost Zetatech, at
/// break-even, about a day of life; see `assets/config.toml`), so the rule
/// waits for an economy that carries licences.
pub fn research_upkeep(world: &mut World, corp: EntityId) {
    for track in Track::ALL {
        let Some(tier) = world.comp::<Corp>(corp).map(|c| c.tech.tier_of(track)) else { return };
        if tier < 2 {
            continue;
        }
        let (coins, units) = (world.config.tech.upkeep_coins_at(tier), world.config.tech.upkeep_data_at(tier));
        let floor = if coins > 0 { crate::systems::corps::fleet_reserve(world, corp) } else { 0 };
        let ok = world.purse(Some(corp)) - coins >= floor && virt::holding(world, corp, track) >= units;
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
    // V31: a lapsed corp order returns its lent fleet deck (re-kit).
    let lent: Vec<EntityId> = world
        .run_orders
        .iter()
        .filter(|(_, o)| o.expires <= now && o.why == crate::virt::RunWhy::CorpOrder)
        .map(|(&a, _)| a)
        .collect();
    // V37 (M14 review): an Overwatch order that lapsed untaken ends its stream.
    let watch: Vec<(EntityId, EntityId)> = world
        .run_orders
        .iter()
        .filter(|(a, o)| o.expires <= now && !world.runner_of.contains_key(a))
        .filter_map(|(&a, o)| match o.purpose {
            crate::virt::Purpose::Overwatch(g) => Some((g, a)),
            _ => None,
        })
        .collect();
    for (g, a) in watch {
        virt::clear_stream(world, g, a);
    }
    world.run_orders.retain(|_, o| o.expires > now);
    for a in lent {
        crate::systems::assets::rekit(world, a);
    }
    // V33: turned robots come back.
    for a in crate::systems::assets::all_assets(world) {
        if let Some(x) = world.comp_mut::<crate::components::Asset>(a) {
            if x.turned.is_some_and(|(_, t)| t <= now) {
                x.turned = None;
            }
        }
    }
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
/// focus track ÷ 2, and the largest `lapse ÷ decay_days`. VirtRaid reads
/// this gap; Research reads [`research_gap`].
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

/// M14 phase 5 (deviation from the spec's Research score): Research's tech
/// gap, `(best − own tier in focus) ÷ 2` clamped to 0..=1, where `best` is
/// the niche rivals' best and, with `[tech] research_gap_street`, also the
/// street tier ([`street_tier`]: the best living corp's tier in the track).
/// In the spec only niche rivals count, and the only rival lead in the city
/// at seed is Militech's Deck 2 against Arasaka's 3, so no other corp ever
/// had a reason to research.
pub fn research_gap(world: &World, corp: EntityId) -> f32 {
    let (niche_gap, _) = research_inputs(world, corp);
    if !world.config.tech.research_gap_street {
        return niche_gap;
    }
    let Some(c) = world.comp::<Corp>(corp) else { return niche_gap };
    let focus = c.tech.focus;
    let street = street_tier(world, focus).0;
    let gap = ((f32::from(street) - f32::from(c.tech.tier_of(focus))) / 2.0).clamp(0.0, 1.0);
    gap.max(niche_gap)
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

/// Where `corp` keeps Data of `track` (V17, a sale's units; a corp run's
/// payload): its Lab of that focus, else its first Lab (ascending).
pub fn data_lab_for(world: &World, corp: EntityId, track: Track) -> Option<EntityId> {
    let labs = labs_of(world, corp);
    labs.iter()
        .copied()
        .find(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.focus == Some(track)))
        .or_else(|| labs.first().copied())
}

// ---------------------------------------------------------------------------
// The Data market (V17, V29, V30; phase 2)
// ---------------------------------------------------------------------------

/// Is `corp` a Tech-niche corp (M13 D17)?
fn is_tech(world: &World, corp: EntityId) -> bool {
    world.comp::<Corp>(corp).is_some_and(|c| c.niches.contains(&crate::components::Niche::Tech))
}

/// Does `corp` buy Data in `track` (V17 plus the phase 5 calibration)? A
/// Tech-niche corp owning a Lab buys every track (the spec); any corp
/// buys the track one of its Labs researches (`Building.focus`), for its
/// own tree. Phase 5: with Zetatech the only buyer the market died with
/// Zetatech's treasury (below its reserve from day ~50, bankrupt day
/// 89-101 with the plane on or off) and `data_sold` read 20/0/0 on seeds
/// 42-44 while gangs moved 600+ units Hideout to Hideout after day 80.
pub fn buys_track(world: &World, corp: EntityId, track: Track) -> bool {
    let labs = labs_of(world, corp);
    (is_tech(world, corp) && !labs.is_empty())
        || labs.iter().any(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.focus == Some(track)))
}

/// V17: a unit's price at `buyer`: `data_price × level(Tech)`, at least 1.
pub fn data_unit_price(world: &World, buyer: EntityId) -> i64 {
    let level = world.comp::<Corp>(buyer).map_or(1.0, |c| c.level(crate::components::Niche::Tech));
    ((world.config.data.data_price as f32 * level).round() as i64).max(1)
}

/// Phase 3 (procurement budget): what `corp` may still spend on Data
/// today: `buy_budget_frac × closing treasury` less today's purchases, and
/// never below `treasury_ref / 4` of treasury (M13's fleet-buy reserve).
pub fn data_budget(world: &World, corp: EntityId) -> i64 {
    let Some(c) = world.comp::<Corp>(corp) else { return 0 };
    let cap = (world.config.data.buy_budget_frac * c.closing.max(0) as f32).floor() as i64 - c.data_bought_today;
    let reserve = world.purse(Some(corp)) - crate::systems::corps::fleet_reserve(world, corp);
    cap.min(reserve).max(0)
}

/// Can `corp` pay for a Data unit now, within its budget (V17)?
fn can_buy_data(world: &World, corp: EntityId) -> bool {
    let price = data_unit_price(world, corp);
    world.purse(Some(corp)) >= price && data_budget(world, corp) >= price
}

/// V17: the Data buyer for `track`: the corp buying the track
/// ([`buys_track`]) with the lowest holding in it that can pay for a unit
/// within its procurement budget ([`data_budget`]; ties the lower id), never
/// `seller`; `only` restricts it to one corp (an agent selling at that
/// corp's Lab).
fn data_buyer(world: &World, track: Track, seller: EntityId, only: Option<EntityId>) -> Option<EntityId> {
    world
        .corps()
        .into_iter()
        .filter(|&c| c != seller && only.is_none_or(|o| o == c) && buys_track(world, c, track))
        .filter(|&c| can_buy_data(world, c))
        .map(|c| (virt::holding(world, c, track), c))
        .min()
        .map(|(_, c)| c)
}

/// V17: `seller` (an agent's wallet, a gang's or a corp's treasury) sells up
/// to `units` of `track`: the buyer pays `data_unit_price` a unit for as
/// many as its treasury covers (`charge`, `Flow::Data`, taxed), the units go
/// to its Lab of that focus (else its first Lab). Returns the units sold;
/// the caller takes them from where they were. No buyer, no sale.
pub fn sell_data(world: &mut World, seller: EntityId, track: Track, units: u32, only: Option<EntityId>) -> u32 {
    if units == 0 {
        return 0;
    }
    let Some(buyer) = data_buyer(world, track, seller, only) else { return 0 };
    let price = data_unit_price(world, buyer);
    let funds = world.purse(Some(buyer)).min(data_budget(world, buyer)).max(0);
    let afford = u32::try_from(funds / price).unwrap_or(u32::MAX);
    let n = units.min(afford);
    if n == 0 {
        return 0;
    }
    let Some(node) = data_lab_for(world, buyer, track).and_then(|b| virt::node_of_building(world, b)) else {
        return 0;
    };
    let coins = i64::from(n) * price;
    ownership::charge(world, Some(buyer), Some(seller), coins, Flow::Data);
    if let Some(c) = world.comp_mut::<Corp>(buyer) {
        c.data_bought_today += coins;
    }
    // V42 `SetDataTax`: an extra share of the sale to the Treasury, from the
    // seller's take (0 by default).
    let extra = (coins as f32 * world.levers.data_tax).floor() as i64;
    if extra > 0 {
        ownership::pay(world, Some(seller), None, extra, Flow::Tax);
    }
    if let Some(x) = world.virt.node_mut(node) {
        x.store.units[track.index()] = x.store.units[track.index()].saturating_add(n);
    }
    world.stats.current.virt.data_sold += n;
    let text = format!(
        "{} sold {n} {track} Data to {} for {coins}",
        world.owner_label(Some(seller)),
        world.owner_label(Some(buyer))
    );
    world.push_event(EventKind::DataSold, &[seller, buyer], text);
    n
}

/// V29: is `b` a corp's Lab whose owner buys Data in some track
/// ([`buys_track`]: a Tech corp's, or a Lab with a focus): where a runner
/// sells Data.
pub fn is_data_buyer_lab(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| {
        bd.kind == BuildingKind::Lab
            && !bd.demolished
            && bd.owner.is_some_and(|o| world.has::<Corp>(o) && Track::ALL.into_iter().any(|t| buys_track(world, o, t)))
    })
}

/// V29 (`LocationKey::DataBuyer`): the open buyer Lab nearest the agent
/// (Manhattan, ties the lower id) whose owner buys a track the agent's deck
/// holds and can pay for a unit.
pub fn data_buyer_lab(world: &World, agent: EntityId) -> Option<EntityId> {
    let tile = world.comp::<crate::components::Position>(agent)?.tile;
    let held = world
        .comp::<crate::components::Kit>(agent)
        .and_then(|k| k.deck)
        .and_then(|d| world.comp::<crate::components::Asset>(d))
        .map_or([0; 3], |a| a.data);
    world
        .buildings_of_kind(BuildingKind::Lab)
        .iter()
        .copied()
        .filter(|&b| is_data_buyer_lab(world, b) && !world.is_closed(b))
        .filter(|&b| {
            world.owner_of(b).is_some_and(|o| {
                can_buy_data(world, o) && Track::ALL.into_iter().any(|t| held[t.index()] > 0 && buys_track(world, o, t))
            })
        })
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(tile), b)))
        .min()
        .map(|(_, b)| b)
}

/// V29 `SellData`: the Data on the agent's deck, track by track, to the
/// corp owning the Lab the agent stands in. Returns the units sold.
pub fn sell_deck_data(world: &mut World, agent: EntityId) -> u32 {
    let Some(deck) = world.comp::<crate::components::Kit>(agent).and_then(|k| k.deck) else { return 0 };
    let here = world.comp::<crate::components::Position>(agent).and_then(|p| p.building);
    let Some(buyer) = here.filter(|&b| is_data_buyer_lab(world, b)).and_then(|b| world.owner_of(b)) else {
        return 0;
    };
    let mut sold = 0;
    for track in Track::ALL {
        let held = world.comp::<crate::components::Asset>(deck).map_or(0, |a| a.data[track.index()]);
        let n = sell_data(world, agent, track, held, Some(buyer));
        if let Some(a) = world.comp_mut::<crate::components::Asset>(deck) {
            a.data[track.index()] -= n;
        }
        sold += n;
    }
    sold
}

/// V30: each gang's Hideout store above `gang_data_keep` (per track) is
/// offered daily through V17.
fn gang_sales(world: &mut World) {
    let keep = world.config.data.gang_data_keep;
    for g in world.gangs() {
        let Some(n) = world.hideout_of(g).and_then(|h| virt::node_of_building(world, h)) else { continue };
        for track in Track::ALL {
            let spare = world.virt.node(n).map_or(0, |x| x.store.get(track)).saturating_sub(keep);
            let sold = sell_data(world, g, track, spare, None);
            if let Some(x) = world.virt.node_mut(n) {
                x.store.units[track.index()] -= sold;
            }
        }
    }
}

/// V17: a corp under `Research` sells the Data it holds in the tracks it
/// does not research, above each track's backup reserve.
fn sell_spare(world: &mut World, corp: EntityId) {
    let Some((focus, tiers)) = world.comp::<Corp>(corp).map(|c| (c.tech.focus, c.tech.tier)) else { return };
    for track in Track::ALL.into_iter().filter(|&t| t != focus) {
        let tier = tiers[track.index()];
        let reserve = world.config.tech.upkeep_data_at(tier) * world.config.data.backup_days;
        let spare = virt::holding(world, corp, track).saturating_sub(reserve);
        let sold = sell_data(world, corp, track, spare, None);
        take_data(world, corp, track, sold);
    }
}
