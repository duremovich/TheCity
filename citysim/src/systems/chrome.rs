//! M13 phase 3: chrome at the Clinic (docs/M13_ASSETS.md § 3, plan D31-D37,
//! D43, D44, D46): installing and treating at a Clinic, sanity and
//! cyberpsychotic episodes, stripping and ripping bodies, the Harvest
//! order's abductions (on screen and off) and its `Abducted` holes.
//!
//! Everything here is a dice contest or a state change between fictional
//! agents. Nothing runs per tick: the sanity pass, the episode roll and the
//! off-screen abductions are daily (from `assets::run`, plan D9), episodes
//! end hourly, the rest is event-driven by actions.

use rand::Rng;

use crate::components::{
    Asset, AssetKind, AssetLoc, Body, Brain, Building, BuildingKind, Controller, Corpse, Gang, GangMember, Hole,
    HoleKind, Kit, Lod, MemoryKind, Personality, Position, ShopPick, Slot, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::assets;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::TICKS_PER_HOUR;
use crate::world::World;

/// `[assets] enabled`: chrome reads nothing while assets are off.
fn on(world: &World) -> bool {
    world.config.assets.enabled
}

// ---------------------------------------------------------------------------
// Reading the body
// ---------------------------------------------------------------------------

/// The implants installed in `body` (a living agent or a corpse), ascending.
pub fn installed(world: &World, body: EntityId) -> Vec<EntityId> {
    assets::assets_at(world, body)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind.is_implant() && x.loc == AssetLoc::Installed(body)))
        .collect()
}

/// The implant in `slot`, if any.
pub fn implant_in(world: &World, body: EntityId, slot: Slot) -> Option<EntityId> {
    installed(world, body)
        .into_iter()
        .find(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Implant(slot)))
}

/// `install_fee[tier − 1]`.
pub fn install_fee(world: &World, tier: u8) -> i64 {
    let i = usize::from(tier.saturating_sub(1));
    world.config.chrome.install_fee.get(i).copied().unwrap_or(0)
}

/// In an episode now (D33).
pub fn in_episode(world: &World, agent: EntityId) -> bool {
    let now = world.tick;
    world.comp::<Body>(agent).and_then(|b| b.episode_until).is_some_and(|t| t > now)
}

/// Below `edgy` sanity (D33).
pub fn edgy(world: &World, agent: EntityId) -> bool {
    on(world) && world.comp::<Body>(agent).is_some_and(|b| b.sanity < world.config.chrome.edgy)
}

/// D33: the body's mood term, derived on read: `−edgy_mood` below `edgy`,
/// plus (phase 4, D39) `+stim_mood` within `stim_hours` of a dose or
/// `−withdrawal_mood` in withdrawal. 0 for a calm, clean body or assets off.
pub fn body_bias(world: &World, agent: EntityId) -> f32 {
    let edgy = if edgy(world, agent) { -world.config.chrome.edgy_mood } else { 0.0 };
    let stims = crate::systems::stims::mood_terms(world, agent);
    if stims != 0.0 {
        edgy + stims
    } else {
        edgy
    }
}

/// D34: the Treat goal's eligibility: sanity below `treat_below`.
pub fn wants_treatment(world: &World, agent: EntityId) -> bool {
    on(world) && world.comp::<Body>(agent).is_some_and(|b| b.sanity < world.config.chrome.treat_below)
}

/// D33: the Fight goal's flat gains `edgy_fight` below `edgy` or (D39) in
/// withdrawal (once for either).
pub fn fight_flat(world: &World, agent: EntityId) -> f32 {
    if edgy(world, agent) || crate::systems::stims::in_withdrawal(world, agent) {
        world.config.chrome.edgy_fight
    } else {
        0.0
    }
}

fn district_label(world: &World, agent: EntityId) -> String {
    let d = world.comp::<Position>(agent).map(|p| world.district_of(p.tile));
    d.map_or_else(|| "the city".to_string(), |d| world.district_name(d).to_string())
}

// ---------------------------------------------------------------------------
// The Clinic: install, therapy, uninstall (D34)
// ---------------------------------------------------------------------------

/// The Clinic buys an implant back at `buyback_frac × value` (`Flow::Sale`,
/// the Clinic's owner to the implant's owner, refused when the owner cannot
/// pay): it goes to the Clinic's stock, owned by the Clinic's owner, any
/// finance settled to its lender out of the price first. Returns the coins
/// the seller kept.
fn buy_back(world: &mut World, clinic: EntityId, implant: EntityId) -> Option<i64> {
    let x = world.comp::<Asset>(implant).cloned()?;
    let buyer = world.owner_of(clinic);
    let price = (world.config.chrome.buyback_frac * x.value as f32).round().max(0.0) as i64;
    if !matches!(ownership::owner_kind(world, buyer), OwnerKind::City | OwnerKind::Corp(_))
        && world.purse(buyer) < price
    {
        return None;
    }
    let mut kept = ownership::charge(world, buyer, x.owner, price, Flow::Sale);
    // A financed implant pays off its lender from the sale (what is left of
    // the plan beyond the price is forgiven: the lender takes the metal's worth).
    if let Some(f) = x.finance.as_ref() {
        let due = f.remaining.min(kept).max(0);
        if due > 0 && x.owner != f.lender {
            ownership::pay(world, x.owner, f.lender, due, Flow::Finance);
            kept -= due;
        }
    }
    assets::set_owner(world, implant, buyer);
    assets::set_loc(world, implant, AssetLoc::Stock(clinic));
    if let Some(m) = world.comp_mut::<Asset>(implant) {
        m.finance = None;
        m.upkeep_arrears = 0;
        m.bricked = false;
        m.keeper = None;
    }
    Some(kept)
}

/// After an implant goes in: sanity − `install_shock`, the `Installed`
/// event, `chrome_installs`.
fn installed_now(world: &mut World, agent: EntityId, clinic: Option<EntityId>, implant: EntityId) {
    let shock = world.config.chrome.install_shock;
    if let Some(b) = world.comp_mut::<Body>(agent) {
        b.sanity = (b.sanity - shock).clamp(0.0, 1.0);
    }
    world.stats.current.chrome_installs += 1;
    let (who, what) = (world.name_of(agent), world.name_of(implant));
    let at = clinic.map_or_else(|| "a back room".to_string(), |c| world.name_of(c));
    let gang = world.gang_of(agent).and_then(|g| world.comp::<Gang>(g)).map(|g| g.name.clone());
    let text = match gang {
        Some(g) => format!("{who} had {what} installed at {at} (gang: {g})"),
        None => format!("{who} had {what} installed at {at}"),
    };
    let actors: Vec<EntityId> = [Some(agent), clinic, Some(implant)].into_iter().flatten().collect();
    world.push_event(EventKind::Installed, &actors, text);
    // M15 W31: a Purist member now above tolerance is cast out.
    crate::systems::creeds::on_install(world, agent);
}

/// D34: buy `pick` (an implant, new or used) at `clinic` and install it in
/// the same action. An occupied slot is bought back first (only once the
/// sale is affordable). `Flow::Asset` and the finance through
/// `assets::buy`.
pub fn buy_install(world: &mut World, agent: EntityId, clinic: EntityId, pick: &ShopPick) -> Result<EntityId, String> {
    let kind = match pick.used.and_then(|u| world.comp::<Asset>(u)) {
        Some(x) => x.kind,
        None => pick.kind,
    };
    let AssetKind::Implant(slot) = kind else { return assets::buy(world, agent, clinic, pick) };
    let old = implant_in(world, agent, slot);
    let a = assets::buy(world, agent, clinic, pick)?;
    if let Some(o) = old {
        if buy_back(world, clinic, o).is_none() {
            // The Clinic cannot pay for the old one: it comes out for nothing.
            let owner = world.owner_of(clinic);
            assets::set_owner(world, o, owner);
            assets::set_loc(world, o, AssetLoc::Stock(clinic));
        }
    }
    installed_now(world, agent, Some(clinic), a);
    Ok(a)
}

/// D34: install a gang's implant (`implant`, in its Hideout's stock) at
/// `clinic` for `fee` (`Flow::Asset` to the Clinic's owner): the gang pays
/// when it can, else the agent. An occupied slot is bought back first. The
/// implant becomes the agent's.
pub fn install(
    world: &mut World,
    agent: EntityId,
    clinic: EntityId,
    implant: EntityId,
    fee: i64,
) -> Result<(), String> {
    let x = world.comp::<Asset>(implant).cloned().ok_or("no such implant")?;
    let AssetKind::Implant(slot) = x.kind else { return Err("not an implant".into()) };
    if !matches!(x.loc, AssetLoc::Stock(_)) {
        return Err("the implant is not in stock".into());
    }
    let to = world.owner_of(clinic);
    let gang = x.owner.filter(|&o| world.has::<Gang>(o));
    let payer = if gang.is_some_and(|g| world.purse(Some(g)) >= fee) {
        gang
    } else if world.purse(Some(agent)) >= fee {
        Some(agent)
    } else {
        return Err(format!("{} cannot pay the fee", world.name_of(agent)));
    };
    let paid = ownership::pay(world, payer, to, fee, Flow::Asset);
    ownership::credit(world, clinic, paid);
    if let Some(o) = implant_in(world, agent, slot) {
        if buy_back(world, clinic, o).is_none() {
            assets::set_owner(world, o, to);
            assets::set_loc(world, o, AssetLoc::Stock(clinic));
        }
    }
    assets::set_keeper(world, implant, None);
    assets::set_owner(world, implant, Some(agent));
    assets::set_loc(world, implant, AssetLoc::Installed(agent));
    if let Some(m) = world.comp_mut::<Asset>(implant) {
        m.finance = None;
    }
    installed_now(world, agent, Some(clinic), implant);
    Ok(())
}

/// D34: Therapy's price at `clinic`: `round(therapy_price × level)`.
pub fn therapy_price(world: &World, clinic: EntityId) -> i64 {
    (world.config.chrome.therapy_price as f32 * assets::seller_level(world, clinic)).round() as i64
}

/// D34: Therapy at `clinic`: the price in full (`Flow::Treatment` to the
/// Clinic's owner), sanity + `therapy_gain`, the `Treated` event.
pub fn therapy(world: &mut World, agent: EntityId, clinic: EntityId) -> bool {
    let price = therapy_price(world, clinic);
    if world.purse(Some(agent)) < price {
        return false;
    }
    let to = world.owner_of(clinic);
    let paid = ownership::pay(world, Some(agent), to, price, Flow::Treatment);
    ownership::credit(world, clinic, paid);
    let gain = world.config.chrome.therapy_gain;
    if let Some(b) = world.comp_mut::<Body>(agent) {
        b.sanity = (b.sanity + gain).clamp(0.0, 1.0);
    }
    world.stats.current.treatments += 1;
    let (who, at) = (world.name_of(agent), world.name_of(clinic));
    world.push_event(EventKind::Treated, &[agent, clinic], format!("{who} took Therapy at {at}"));
    true
}

/// D34: the Clinic buys the agent's highest-load implant back (the highest
/// tier, ties the lower id).
pub fn uninstall(world: &mut World, agent: EntityId, clinic: EntityId) -> bool {
    let pick = installed(world, agent)
        .into_iter()
        .filter_map(|a| world.comp::<Asset>(a).map(|x| (std::cmp::Reverse(x.tier), a)))
        .min()
        .map(|(_, a)| a);
    let Some(a) = pick else { return false };
    if buy_back(world, clinic, a).is_none() {
        return false;
    }
    let (who, what, at) = (world.name_of(agent), world.name_of(a), world.name_of(clinic));
    world.push_event(EventKind::Installed, &[agent, clinic], format!("{who} had the {what} taken out at {at}"));
    true
}

/// D44: the implant in a gang's Hideout stock reserved for `agent`
/// (`Asset.keeper`), if the agent is still a member.
pub fn reserved_implant(world: &World, agent: EntityId) -> Option<EntityId> {
    let gang = world.gang_of(agent)?;
    let h = world.hideout_of(gang)?;
    assets::assets_at(world, h).iter().copied().find(|&a| {
        world.comp::<Asset>(a).is_some_and(|x| {
            x.kind.is_implant() && x.loc == AssetLoc::Stock(h) && x.keeper == Some(agent) && x.owner == Some(gang)
        })
    })
}

/// D44 (phase 3), daily after the bike: the gang puts its chrome on its
/// members. Each unreserved implant in the Hideout's stock (its take from
/// rips and scavs, ascending) is reserved for the strongest member (highest
/// fighting, ties the lower id) with that slot empty and nothing reserved;
/// then, with the treasury at `gang_chrome_floor`, one Arms T1 is bought at
/// the nearest Clinic into the stock for the strongest member without Arms.
/// A member installs its reservation (`keeper`) at a Clinic for
/// `install_fee` (the Shop goal's install offer).
pub fn gang_arms(world: &mut World, gang: EntityId) {
    if !on(world) {
        return;
    }
    let Some(h) = world.hideout_of(gang) else { return };
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    // Stale reservations (a member who left or died) are released.
    for a in assets::assets_at(world, h).to_vec() {
        let stale = world
            .comp::<Asset>(a)
            .and_then(|x| x.keeper.filter(|_| x.kind.is_implant()))
            .is_some_and(|k| members.binary_search(&k).is_err() || !world.has::<Brain>(k));
        if stale {
            assets::set_keeper(world, a, None);
        }
    }
    let strongest = |world: &World, slot: Slot| {
        members
            .iter()
            .copied()
            .filter(|&m| world.has::<Brain>(m) && !world.has::<crate::components::Sentence>(m))
            .filter(|&m| implant_in(world, m, slot).is_none() && reserved_implant(world, m).is_none())
            .map(|m| (std::cmp::Reverse(ordered_float::OrderedFloat(crate::systems::law::fighting(world, m))), m))
            .min()
            .map(|(_, m)| m)
    };
    // The take first.
    let stock: Vec<(EntityId, Slot)> = assets::assets_at(world, h)
        .iter()
        .copied()
        .filter_map(|a| {
            let x = world.comp::<Asset>(a)?;
            let AssetKind::Implant(slot) = x.kind else { return None };
            (x.loc == AssetLoc::Stock(h) && x.keeper.is_none() && x.owner == Some(gang) && x.condition > 0)
                .then_some((a, slot))
        })
        .collect();
    for (a, slot) in stock {
        if let Some(m) = strongest(world, slot) {
            assets::set_keeper(world, a, Some(m));
        }
    }
    // Then one bought, while the treasury holds the floor.
    if world.purse(Some(gang)) < world.config.chrome.gang_chrome_floor {
        return;
    }
    let Some(member) = strongest(world, Slot::Arms) else { return };
    let from = world.comp::<Building>(h).map(|b| b.door).unwrap_or_default();
    let Some(clinic) = assets::nearest_seller(world, BuildingKind::Clinic, from, false) else { return };
    let pick = ShopPick { kind: AssetKind::Implant(Slot::Arms), tier: 1, used: None, upgrade: false };
    let note = format!("for {}", world.name_of(member));
    if let Ok(a) = assets::buy_noted(world, gang, clinic, &pick, Some(&note)) {
        assets::set_loc(world, a, AssetLoc::Stock(h));
        assets::set_keeper(world, a, Some(member));
    }
}

// ---------------------------------------------------------------------------
// Sanity and episodes (D33)
// ---------------------------------------------------------------------------

/// D33, daily from `assets::run` (ascending): sanity moves toward `1 −
/// Kit.load` by at most `sanity_drift`, less `unmedicated_drift` per
/// implant whose upkeep is unpaid; below `psycho` a living agent not yet in
/// an episode rolls `episode_base × (psycho − sanity) ÷ psycho` on the
/// world stream. A bare body at sanity 1 is untouched (no write, no roll).
pub fn sanity_daily(world: &mut World) {
    let cfg = world.config.chrome.clone();
    let mut rolls = Vec::new();
    // scan-ok: daily: sanity
    for id in world.with::<Body>() {
        if !world.has::<Brain>(id) {
            continue;
        }
        let load = world.comp::<Kit>(id).map_or(0.0, |k| k.load);
        let unpaid = if load > 0.0 {
            installed(world, id)
                .into_iter()
                .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.upkeep_arrears > 0))
                .count()
        } else {
            0
        };
        let Some(b) = world.comp_mut::<Body>(id) else { continue };
        let target = (1.0 - load).clamp(0.0, 1.0);
        if b.sanity != target || unpaid > 0 {
            let step = (target - b.sanity).clamp(-cfg.sanity_drift, cfg.sanity_drift);
            b.sanity = (b.sanity + step - cfg.unmedicated_drift * unpaid as f32).clamp(0.0, 1.0);
        }
        if b.sanity < cfg.psycho && b.episode_until.is_none() {
            rolls.push((id, b.sanity));
        }
    }
    let psycho = cfg.psycho.max(1e-6);
    for (id, sanity) in rolls {
        // Phase 4 (plan 4.3): × `stim_episode_mult` within 12 h of a dose.
        let mut p = cfg.episode_base * (cfg.psycho - sanity) / psycho;
        if crate::systems::stims::used_within(world, id, crate::systems::stims::EPISODE_STIM_HOURS) {
            p *= cfg.stim_episode_mult;
        }
        let p = f64::from(p.clamp(0.0, 1.0));
        if world.rng.world().random_bool(p) {
            start_episode(world, id);
        }
    }
}

/// D33: the agent goes berserk for `episode_hours`: the `Episode` event,
/// every body within `crime.sight` remembers it, the law gets an Assault
/// report and a sighting (it hunts the agent), and a Statistical agent is
/// promoted to Coarse (it ranks LOD class 2 while the episode lasts).
pub fn start_episode(world: &mut World, agent: EntityId) {
    if !crate::systems::law::living(world, agent) {
        return;
    }
    let until = world.tick + u64::from(world.config.chrome.episode_hours.max(1)) * TICKS_PER_HOUR;
    let sanity = world.comp::<Body>(agent).map_or(0.0, |b| b.sanity);
    if let Some(b) = world.comp_mut::<Body>(agent) {
        b.episode_until = Some(until);
    }
    world.episodes.insert(agent);
    world.stats.current.episodes += 1;
    if world.comp::<Brain>(agent).is_some_and(|b| b.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, agent, Lod::Coarse);
    }
    // The goal is pinned (Fight) at the next think: drop what it was doing.
    if world.comp::<Brain>(agent).is_some_and(|b| b.plan.is_some()) {
        world.abort_plan(agent);
    }
    if let Some(b) = world.comp_mut::<Brain>(agent) {
        b.current_goal = None;
    }
    let text =
        format!("{} went berserk in {} (sanity {sanity:.2})", world.name_of(agent), district_label(world, agent));
    world.push_event(EventKind::Episode, &[agent], text);
    let Some(tile) = world.comp::<Position>(agent).map(|p| p.tile) else { return };
    let r = world.config.crime.sight;
    for w in world.bodies() {
        if w != agent && world.comp::<Position>(w).is_some_and(|p| crate::systems::law::chebyshev(p.tile, tile) <= r) {
            world.remember(w, MemoryKind::SawEpisode, Some(agent), 0.6, -0.6, false);
        }
    }
    crate::systems::law::file_report(world, crate::components::Crime::Assault, agent, None);
    let now = world.tick;
    world.last_seen.insert(agent, (tile, now));
}

/// D33: the episode ends (`why`: "spent", "arrested", "dead"). A survivor
/// gains sanity +0.1, remembers it, and leaves its gang when its loyalty is
/// below 0.5.
pub fn end_episode(world: &mut World, agent: EntityId, why: &str) {
    let was = world.episodes.remove(&agent);
    let had = world.comp::<Body>(agent).is_some_and(|b| b.episode_until.is_some());
    if let Some(b) = world.comp_mut::<Body>(agent) {
        b.episode_until = None;
    }
    if !was && !had {
        return;
    }
    let alive = crate::systems::law::living(world, agent);
    if alive {
        if let Some(b) = world.comp_mut::<Body>(agent) {
            b.sanity = (b.sanity + 0.1).clamp(0.0, 1.0);
        }
        world.remember(agent, MemoryKind::Episode, None, 0.9, -0.8, false);
    }
    let text = format!("{} came down ({why})", world.name_of(agent));
    world.push_event(EventKind::Episode, &[agent], text);
    if alive && world.has::<GangMember>(agent) && world.comp::<Personality>(agent).is_some_and(|p| p.loyalty < 0.5) {
        crate::systems::gang::leave(world, agent, "episode");
    }
}

/// The law ended an episode (a cuffing, or the berserker killed resisting).
pub fn ended_by_law(world: &mut World, agent: EntityId, why: &str) {
    if world.episodes.contains(&agent) || in_episode(world, agent) {
        world.stats.current.episodes_by_law += 1;
        end_episode(world, agent, why);
    }
}

/// Hourly (from `assets::run`): episodes past `episode_until`, and the
/// dead, end.
pub fn episodes_hourly(world: &mut World) {
    let now = world.tick;
    for a in world.episodes.iter().copied().collect::<Vec<_>>() {
        if !crate::systems::law::living(world, a) {
            end_episode(world, a, "dead");
        } else if world.comp::<Body>(a).and_then(|b| b.episode_until).is_none_or(|t| t <= now) {
            end_episode(world, a, "spent");
        }
    }
}

/// D33: an episode's quarry: the nearest living body within `crime.sight`
/// (Chebyshev; ties the lower id), not the agent.
pub fn episode_target(world: &World, agent: EntityId) -> Option<EntityId> {
    let tile = world.comp::<Position>(agent)?.tile;
    let r = world.config.crime.sight;
    world
        .bodies()
        .into_iter()
        .filter(|&o| o != agent && world.has::<Brain>(o))
        .filter_map(|o| {
            let d = crate::systems::law::chebyshev(world.comp::<Position>(o)?.tile, tile);
            (d <= r).then_some((d, o))
        })
        .min()
        .map(|(_, o)| o)
}

/// D33: the kill multiplier of an episode's `Attack`.
pub fn attack_kill_mult(world: &World, agent: EntityId) -> f32 {
    if in_episode(world, agent) {
        world.config.chrome.berserk_kill_mult
    } else {
        1.0
    }
}

/// D33: a guard's contested arrest of a berserker kills at `psycho_kill`
/// (× M12's `crush_kill_mult` while the district is under a Crush).
pub fn arrest_kill_mult(world: &World, suspect: EntityId) -> f32 {
    let base = world.config.crime.fight_death_p as f32;
    let mut m = if base > 0.0 { world.config.chrome.psycho_kill / base } else { 1.0 };
    if let Some(tile) = world.comp::<Position>(suspect).map(|p| p.tile) {
        let now = world.tick;
        if world.district(world.district_of(tile)).crush_until.is_some_and(|t| t > now) {
            m *= world.config.riots.crush_kill_mult;
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Strip, Rip, Loot (D35)
// ---------------------------------------------------------------------------

/// Kin, spouse, or a fellow member of the dead: never loots it.
fn close_to(world: &World, agent: EntityId, dead: EntityId) -> bool {
    use crate::components::RelKind;
    let kin =
        world.edge(agent, dead).is_some_and(|e| matches!(e.kind, RelKind::Family | RelKind::Spouse | RelKind::Parent));
    let gang = world.gang_of(agent);
    let member = gang.is_some_and(|g| world.comp::<Gang>(g).is_some_and(|x| x.members.binary_search(&dead).is_ok()));
    kin || member || world.spouse_of(agent) == Some(dead)
}

/// May `agent` strip corpse `c`: unsettled, unstripped, unburied, not kin,
/// spouse or (a dead member stays on the roster until the gang notices) a
/// fellow member's.
pub fn may_loot(world: &World, agent: EntityId, c: EntityId) -> bool {
    world.comp::<Corpse>(c).is_some_and(|k| !k.settled && !k.stripped && !k.buried) && !close_to(world, agent, c)
}

/// D35: the Loot goal's gate and target: the nearest entry of
/// `loot_corpses` within `loot_reach` (Manhattan; ties the lower id) the
/// agent may strip.
pub fn loot_target(world: &World, agent: EntityId) -> Option<EntityId> {
    if !on(world) || world.loot_corpses.is_empty() {
        return None;
    }
    let tile = world.comp::<Position>(agent)?.tile;
    let reach = world.config.chrome.loot_reach;
    world
        .loot_corpses
        .iter()
        .copied()
        .filter_map(|c| {
            let d = world.comp::<Position>(c)?.tile.manhattan(tile);
            (d <= reach && may_loot(world, agent, c)).then_some((d, c))
        })
        .min()
        .map(|(_, c)| c)
}

/// D35: may `agent` rip chrome (a member, or `courage ≥ rip_courage`).
pub fn may_rip(world: &World, agent: EntityId) -> bool {
    world.has::<GangMember>(agent)
        || world.comp::<Personality>(agent).is_some_and(|p| p.courage >= world.config.assets.rip_courage)
}

/// D35: strip corpse `c`: its coins to the wallet, its goods up to the
/// agent's capacity, its packs carried; a Theft raised; an heir who sees it
/// remembers `Stripped`; the body is settled.
pub fn strip(world: &mut World, agent: EntityId, c: EntityId) -> bool {
    if !may_loot(world, agent, c) || !crate::systems::law::near(world, agent, c, 1) {
        return false;
    }
    let Some(corpse) = world.comp_mut::<Corpse>(c) else { return false };
    let loot = std::mem::take(&mut corpse.loot);
    corpse.stripped = true;
    if loot.coins > 0 {
        world.purse_add(Some(agent), loot.coins);
    }
    assets::give_goods(world, agent, loot.food, loot.stims, loot.parts);
    for p in assets::assets_at(world, c).to_vec() {
        if world.comp::<Asset>(p).is_some_and(|x| x.loc == AssetLoc::Carried(c)) {
            assets::set_owner(world, p, Some(agent));
            assets::set_loc(world, p, AssetLoc::Carried(agent));
        }
    }
    let tile = world.comp::<Position>(c).map_or_else(Default::default, |p| p.tile);
    crate::systems::law::raise_crime(world, agent, None, crate::components::Crime::Theft, tile);
    let r = world.config.crime.sight;
    for h in crate::systems::demography::coin_heirs(world, c) {
        if world.has::<Brain>(h)
            && world.comp::<Position>(h).is_some_and(|p| crate::systems::law::chebyshev(p.tile, tile) <= r)
        {
            world.remember(h, MemoryKind::Stripped, Some(agent), 0.7, -0.7, false);
            // M15 W35: a witnessed strip of a close one's body leaves a grudge.
            let r = crate::word::DeedRef { deed: crate::word::Deed::Stripped, actor: Some(agent), object: Some(c) };
            crate::systems::grudges::on_learn(world, h, &r, 1.0, 0);
        }
    }
    world.stats.current.stripped += 1;
    world.stats.current.stripped_window += 1;
    let (who, dead) = (world.name_of(agent), world.name_of(c));
    world.push_event(
        EventKind::Stripped,
        &[agent, c],
        format!("{who} stripped the body of {dead} ({} coins)", loot.coins),
    );
    assets::settle_corpse(world, c);
    true
}

/// D35/D36: rip every implant out of `body` (a corpse, or a live abductee
/// at the Hideout). A member's take goes to its gang's Hideout stock
/// (stolen); anyone else's is broken for `parts_per.implant` Parts each
/// into its inventory. A live abductee is then killed (`p_rip_kill`) or
/// released at sanity 0.1. Returns the implants taken.
pub fn rip(world: &mut World, agent: EntityId, body: EntityId) -> usize {
    let implants = installed(world, body);
    let gang = world.gang_of(agent);
    let hideout = gang.and_then(|g| world.hideout_of(g));
    let per = world.config.assets.parts_per.implant;
    // M15 W31: a Purist destroys what it rips: Parts to the Recycler.
    let purist = gang.is_some_and(|g| crate::systems::creeds::is_purist(world, g));
    let recycler = if purist {
        let from = world.comp::<Position>(agent).map(|p| p.tile).unwrap_or_default();
        world.nearest_of_kind(crate::components::BuildingKind::Cemetery, from)
    } else {
        None
    };
    for &a in &implants {
        match (gang, hideout) {
            _ if purist => {
                assets::despawn(world, a);
                if let Some(r) = recycler.and_then(|r| world.comp_mut::<crate::components::Building>(r)) {
                    let i = crate::components::Good::Parts as usize - 1;
                    r.stock_goods[i] = r.stock_goods[i].saturating_add(per);
                }
            }
            (Some(g), Some(h)) => assets::into_gang_stock(world, a, g, h),
            _ => {
                assets::despawn(world, a);
                assets::give_goods(world, agent, 0, 0, u16::try_from(per).unwrap_or(u16::MAX));
            }
        }
    }
    let n = implants.len();
    if n > 0 {
        world.stats.current.harvests += 1;
        let who = world.name_of(agent);
        let text = format!("{who} ripped {n} implants from {}", world.name_of(body));
        world.push_event(EventKind::Harvested, &[agent, body], text);
    }
    // A live abductee: killed on the table, or let go.
    if crate::systems::law::living(world, body) {
        let escorted = world.comp::<Brain>(agent).and_then(|b| b.escorting) == Some(body);
        if escorted {
            if let Some(b) = world.comp_mut::<Brain>(agent) {
                b.escorting = None;
            }
        }
        if let Some(b) = world.comp_mut::<Brain>(body) {
            b.cuffed_by = None;
            b.abducted_by = None;
        }
        let p = f64::from(world.config.chrome.p_rip_kill.clamp(0.0, 1.0));
        if world.rng.world().random_bool(p) {
            let tile = world.comp::<Position>(body).map_or_else(Default::default, |p| p.tile);
            world.kill_by(body, crate::components::DeathCause::Violence, Some(agent));
            crate::systems::law::raise_crime_on(world, agent, None, Some(body), crate::components::Crime::Murder, tile);
        } else {
            if let Some(b) = world.comp_mut::<Body>(body) {
                b.sanity = 0.1;
            }
            world.remember(body, MemoryKind::Abducted, Some(agent), 1.0, -1.0, false);
            world.leave_building(body);
        }
    } else if world.comp::<Brain>(agent).and_then(|b| b.carrying_corpse) == Some(body) {
        // A body dragged home is put down at the Hideout.
        if let Some(b) = world.comp_mut::<Brain>(agent) {
            b.carrying_corpse = None;
        }
    }
    n
}

// ---------------------------------------------------------------------------
// Harvest (D36) and the Abducted hole (D37)
// ---------------------------------------------------------------------------

/// Districts a gang may harvest in: held by it, or Contested.
fn harvest_ground(world: &World, gang: EntityId) -> Vec<bool> {
    world
        .districts
        .iter()
        .map(|d| matches!(d.control, Controller::Contested) || d.control == Controller::Gang(gang))
        .collect()
}

/// D36: the living non-member adult with the highest `Kit.chrome_value`
/// among `Kit.visible ≥ harvest_min_visible` whose Home or tile is in a
/// district the gang holds or that is Contested (ties the lower id), with
/// that value. O(agents with a Kit): called once per rescore and cached on
/// the gang.
pub fn harvest_target(world: &World, gang: EntityId) -> Option<(EntityId, i64)> {
    if !on(world) {
        return None;
    }
    let min = world.config.chrome.harvest_min_visible;
    let ground = harvest_ground(world, gang);
    let in_ground = |t: crate::components::TilePos| ground.get(world.district_of(t).index()).copied().unwrap_or(false);
    let mut best: Option<(i64, std::cmp::Reverse<EntityId>)> = None;
    // M14 V30: freelance runners the gang's database sighted are candidates
    // wherever they are and whatever shows.
    let sighted: Vec<EntityId> = world
        .db
        .get(&gang)
        .map(|db| {
            db.sightings.iter().filter(|s| !s.relayed).map(|s| s.who).filter(|&w| world.gang_of(w).is_none()).collect()
        })
        .unwrap_or_default();
    // scan-ok: per gang rescore (daily, and on shocks)
    for id in world.with::<Kit>() {
        let Some(k) = world.comp::<Kit>(id) else { continue };
        let seen = !sighted.is_empty() && sighted.contains(&id);
        if (k.visible < min && !seen) || k.chrome_value <= 0 {
            continue;
        }
        if !crate::systems::law::living(world, id)
            || world.gang_of(id) == Some(gang)
            || world.has::<crate::components::Sentence>(id)
            || !crate::systems::demography::is_adult(world, id)
        {
            continue;
        }
        // M15 W32: nobody harvests the feared.
        if crate::systems::reputation::rep(world, id).dread >= world.config.reputation.harvest_dread_max {
            continue;
        }
        let home = world
            .comp::<crate::components::Household>(id)
            .and_then(|h| h.home)
            .and_then(|h| world.comp::<Building>(h))
            .map(|b| b.door);
        let tile = world.comp::<Position>(id).map(|p| p.tile);
        if !seen && !home.is_some_and(in_ground) && !tile.is_some_and(in_ground) {
            continue;
        }
        let key = (k.chrome_value, std::cmp::Reverse(id));
        if best.is_none_or(|b| key > b) {
            best = Some(key);
        }
    }
    best.map(|(v, std::cmp::Reverse(id))| (id, v))
}

/// The gang's cached Harvest target, still a valid one.
pub fn gang_harvest_target(world: &World, gang: EntityId) -> Option<EntityId> {
    let t = world.comp::<Gang>(gang)?.harvest_target?;
    let ok = crate::systems::law::living(world, t)
        && world.gang_of(t) != Some(gang)
        && !world.has::<crate::components::Sentence>(t)
        && world.comp::<Brain>(t).is_some_and(|b| b.cuffed_by.is_none());
    ok.then_some(t)
}

/// D36: `agent` tries to drag `victim` off: a fight with the fighting of
/// fellow members within 2 tiles on the abductor's side. A win cuffs the
/// victim to the abductor (the M9 escort drag); a victim killed in the
/// fight is carried as a corpse. Either way an Abduction is raised.
pub fn abduct(world: &mut World, agent: EntityId, victim: EntityId) -> bool {
    if !crate::systems::law::living(world, victim) || !crate::systems::law::near(world, agent, victim, 1) {
        return false;
    }
    let bonus: f32 = world
        .gang_of(agent)
        .and_then(|g| world.comp::<Gang>(g))
        .map(|g| g.members.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&m| {
            m != agent && crate::systems::law::living(world, m) && crate::systems::law::near(world, agent, m, 2)
        })
        .map(|m| crate::systems::law::fighting(world, m))
        .sum();
    let mods = crate::systems::law::FightMods { kill_mult: 1.0, a_bonus: bonus, kill_p: None };
    let (winner, _, died) = crate::systems::law::resolve_fight_mods(world, agent, victim, mods);
    let tile = world.comp::<Position>(agent).map_or_else(Default::default, |p| p.tile);
    if crate::systems::law::living(world, agent) {
        crate::systems::law::raise_crime(world, agent, Some(victim), crate::components::Crime::Abduction, tile);
    }
    if winner != agent || !crate::systems::law::living(world, agent) {
        return false;
    }
    if died {
        if let Some(b) = world.comp_mut::<Brain>(agent) {
            b.carrying_corpse = Some(victim);
        }
    } else {
        world.abort_plan(victim);
        if let Some(b) = world.comp_mut::<Brain>(victim) {
            b.cuffed_by = Some(agent);
            b.abducted_by = Some(agent);
            b.current_goal = None;
        }
        if let Some(b) = world.comp_mut::<Brain>(agent) {
            b.escorting = Some(victim);
        }
    }
    world.stats.current.abductions += 1;
    let text =
        format!("{} abducted {} in {}", world.name_of(agent), world.name_of(victim), district_label(world, agent));
    world.push_event(EventKind::Abducted, &[agent, victim], text);
    true
}

/// Is `victim` an abductee being dragged by `agent`?
pub fn dragging(world: &World, agent: EntityId) -> Option<EntityId> {
    let v = world.comp::<Brain>(agent)?.escorting?;
    (world.comp::<Brain>(v)?.abducted_by == Some(agent)).then_some(v)
}

/// The off-screen abduction of `id` (D37). `src` (L2 plan L28): the
/// faction-violence source of `fviolence::daily`'s Harvest cell (the hole
/// carries it and is filed in its district); `None`: M13's roll.
pub fn abduct_offscreen(world: &mut World, id: EntityId, src: Option<&crate::ledger::ActiveSource>) -> bool {
    let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else { return false };
    let tick = world.tick;
    let (zone, district) = match src {
        Some(s) => (world.district(s.district).zone, s.district),
        None => (world.map.zone(tile), world.district_of(tile)),
    };
    let place = world.district_name(district).to_string();
    let hid = crate::components::hole_id(tick, id, HoleKind::Abducted);
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins.max(0));
    if let Some(w) = world.comp_mut::<Wallet>(id) {
        w.coins -= coins;
    }
    for a in installed(world, id) {
        assets::set_loc(world, a, AssetLoc::Limbo(hid));
    }
    let hole = Hole {
        id: hid,
        kind: HoleKind::Abducted,
        victim: id,
        zone,
        district,
        tick,
        event_id: 0,
        consequential: true,
        spouse: world.spouse_of(id),
        loot: coins,
        home: world.comp::<crate::components::Household>(id).and_then(|h| h.home),
        gang: world.gang_of(id),
        source: src.map(|s| s.source),
        faction: src.and_then(|s| s.faction),
        riot: src.and_then(|s| s.riot),
    };
    let name = world.name_of(id);
    let ev = world.push_event(
        EventKind::Abducted,
        &[EntityId::NONE, id],
        format!("{name} was abducted in {place} (abductors unknown)"),
    );
    world.kill_by(id, crate::components::DeathCause::Violence, None);
    world.stats.current.deaths_violence_offscreen += 1;
    world.stats.current.abductions += 1;
    if let Some(c) = world.comp_mut::<Corpse>(id) {
        c.stripped = true;
    }
    crate::systems::bind::open_hole(world, Hole { event_id: ev, ..hole });
    true
}

/// D37: a bound `Abducted` hole's limbo implants go to the actor's gang's
/// Hideout stock (stolen); with no gang (or `actor` `None`: Unknown) they
/// are destroyed.
pub fn settle_limbo(world: &mut World, hole: crate::components::HoleId, actor: Option<EntityId>) {
    let Some(list) = world.limbo.get(&hole).cloned() else { return };
    let gang = actor.and_then(|a| world.gang_of(a));
    let hideout = gang.and_then(|g| world.hideout_of(g));
    for a in list {
        match (gang, hideout) {
            (Some(g), Some(h)) => assets::into_gang_stock(world, a, g, h),
            _ => assets::despawn(world, a),
        }
    }
    if gang.is_some() {
        world.stats.current.harvests += 1;
        let who = actor.map_or_else(String::new, |a| world.name_of(a));
        let text = format!("{who} ripped the abductee's implants (hole {hole})");
        world.push_event(EventKind::Harvested, &[actor.unwrap_or(EntityId::NONE)], text);
    }
}

// ---------------------------------------------------------------------------
// The Statistical reading (D46)
// ---------------------------------------------------------------------------

/// D46: the hour's `(p_killed, p_assaulted, p_robbed)` multipliers for a
/// Statistical agent's Kit, `None` for a bare Kit (the table's own
/// probabilities, bit for bit).
pub fn stat_multipliers(world: &World, id: EntityId) -> Option<(f32, f32)> {
    let k = world.comp::<Kit>(id).filter(|k| !k.is_bare())?;
    let dodge_w = world.config.vehicles.dodge_w;
    let harm = (1.0 - dodge_w * k.reflex).max(0.0) * (1.0 - k.armour).max(0.0);
    let robbed = 1.0 + world.config.lod.flash_w * k.flash;
    Some((harm, robbed))
}

/// D37: the binder's Assaulted/Killed candidate weight `1 + chrome_bind_w ×
/// Kit.fighting`, `None` when it is 1 (no Arms, or the knob at 0).
pub fn bind_weight(world: &World, id: EntityId) -> Option<f64> {
    let f = world.comp::<Kit>(id).map_or(0.0, |k| k.fighting);
    let w = world.config.lod.chrome_bind_w;
    (f != 0.0 && w != 0.0).then(|| 1.0 + f64::from(w) * f64::from(f))
}

/// Statistical chrome (D43): `pick` bought remotely at `clinic` and
/// installed in place, the shock applied.
pub fn stat_install(world: &mut World, agent: EntityId, clinic: EntityId, pick: &ShopPick) -> bool {
    buy_install(world, agent, clinic, pick).is_ok()
}
