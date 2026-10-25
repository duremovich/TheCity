//! M17 phase 5 (docs/M17_OUTSIDE.md § 5, § 6): the outside as a place you
//! can be sent, at the Ledger LOD, and the one boogeyman.
//!
//! **Abroad.** A mission on a parent's asset (a depot, a data centre, a
//! convoy) is a ledger record (`Outside.abroad`), posted by a god or by a
//! parent's Campaign. Its crew is away (`Outside.away`) for `abroad_days`:
//! demoted to Statistical, skipped by the hourly table, by the Hunts and by
//! the contract board. On `back_at` one keyed draw settles it, `p =
//! logistic(strike_k × (S_crew ÷ def − 1))`; each crew member dies abroad
//! with `death_p × (1 − p)` (buried there); the survivors walk back in at an
//! edge road. The draw also makes `cast_size` transient NPCs, records never
//! entities, hash-derived from the mission and their index; one whose
//! disposition toward the crew passes `promote_friend` or `promote_enemy`
//! is promoted on a roll under `promote_p` and arrives as an immigrant
//! within `promote_delay_days` with that edge to the crew's leader (an
//! Enemy holds a grudge the Hunt reads).
//!
//! **The Blackwall.** A successful run on an uplink (only a deck at
//! `blackwall.cap` reaches one) rolls `breach_p`, never twice inside
//! `cooldown_days`; the breach lands at the next midnight. **Traces**: the
//! outside writes a sighting of whoever breached its uplink or killed one
//! of its Outsiders into its branches' databases.
//!
//! Every roll is a keyed hash (`econ::hash_unit`); nothing runs per agent
//! per tick (the daily pass walks the missions and the promoted; the
//! `is_away` checks read an empty map in a city with no mission).

use crate::components::{Corp, DeathCause, Identity, Lod, Personality, Position, RelKind, Sex};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{Abroad, CastMember, CastRole, OutsideAsset, OutsideId, Promoted};
use crate::systems::econ::hash_unit;
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::word::GrudgeCause;
use crate::world::World;

/// `econ::hash_unit` purposes (phase 5).
const PURPOSE_ABROAD: u64 = 0x0E17_0006;
const PURPOSE_ABROAD_DEATH: u64 = 0x0E17_0007;
const PURPOSE_CAST: u64 = 0x0E17_0008;
const PURPOSE_BREACH: u64 = 0x0E17_0009;

/// Is `id` away on a mission abroad. O(log n) on a map that is empty in a
/// city with no mission.
pub fn is_away(world: &World, id: EntityId) -> bool {
    !world.outside.away.is_empty() && world.outside.away.contains_key(&id)
}

fn parent_name(world: &World, id: OutsideId) -> String {
    world.outside.faction(id).map_or_else(String::new, |f| f.name.clone())
}

/// A living megacorp parent with a parent record.
fn living_parent(world: &World, id: OutsideId) -> bool {
    world.outside.faction(id).is_some_and(|f| !f.dead && f.parent.is_some())
}

/// Post a mission abroad on `on`'s `asset`: the crew (free adults not
/// away and working no live record, `crew_max` at most, the leader first)
/// leaves at once. The mission's id, or why not.
pub fn post(
    world: &mut World,
    buyer: Option<EntityId>,
    on: OutsideId,
    asset: OutsideAsset,
    crew: &[EntityId],
) -> Result<u32, String> {
    if !crate::systems::parents::parents_on(world) {
        return Err("Abroad: the parents are off".into());
    }
    if !living_parent(world, on) {
        return Err(format!("Abroad: no living parent {on}"));
    }
    let cfg = world.config.outside.abroad.clone();
    let mut team: Vec<EntityId> = Vec::new();
    for &a in crew {
        if team.len() >= cfg.crew_max || team.contains(&a) {
            continue;
        }
        if crate::systems::contracts::free_adult(world, a) && !crate::systems::contracts::holds_body(world, a) {
            team.push(a);
        }
    }
    if team.is_empty() {
        return Err("Abroad: no crew free to go".into());
    }
    let now = world.tick;
    let id = world.outside.abroad_next;
    world.outside.abroad_next += 1;
    for &a in &team {
        world.outside.away.insert(a, id);
        world.hunts.remove(&a);
        world.abort_plan(a);
        world.remove_from_building(a);
        crate::systems::lod::set_lod(world, a, Lod::Statistical);
    }
    crate::systems::hunt::reindex(world);
    world.outside.abroad.push(Abroad {
        id,
        buyer,
        on,
        asset,
        crew: team.clone(),
        since: now,
        back_at: now + u64::from(cfg.abroad_days) * TICKS_PER_DAY,
    });
    world.outside.today.abroad_posted += 1;
    let leader = world.name_of(team[0]);
    let text = format!(
        "{leader} and {} more left the city for a job on {}'s {}",
        team.len() - 1,
        parent_name(world, on),
        asset.label()
    );
    world.push_event(EventKind::AbroadDeparted, &team, text);
    Ok(id)
}

/// The midnight pass (from `systems::outside::run`): the missions due come
/// back and are settled; the promoted due arrive; a breach due lands.
pub fn daily(world: &mut World) {
    let now = world.tick;
    let due: Vec<Abroad> = world.outside.abroad.iter().filter(|m| m.back_at <= now).cloned().collect();
    world.outside.abroad.retain(|m| m.back_at > now);
    for m in due {
        resolve(world, &m);
    }
    arrivals(world);
    if let Some((on, runner, patron)) = world.outside.breach_due.take() {
        breach(world, on, runner, patron);
    }
    let living: Vec<EntityId> =
        world.outside.promoted_living.iter().copied().filter(|&a| crate::systems::law::living(world, a)).collect();
    world.outside.promoted_living = living;
}

/// The logistic of the plan's strike: `1 / (1 + e^(−k (x − 1)))`.
fn strike_p(k: f32, ratio: f32) -> f32 {
    1.0 / (1.0 + (-k * (ratio - 1.0)).exp())
}

/// A mission's odds: the crew's summed `raid::strength` against the asset's
/// defence at the parent's edge.
pub fn odds(world: &World, m: &Abroad) -> f32 {
    let cfg = &world.config.outside.abroad;
    let s: f32 = m
        .crew
        .iter()
        .filter(|&&a| crate::systems::law::living(world, a))
        .map(|&a| crate::systems::raid::strength(world, a))
        .sum();
    let edge = world.outside.faction(m.on).and_then(|f| f.parent.as_ref()).map_or(1.0, |p| p.edge);
    let def = cfg.def[asset_index(m.asset)] * (0.5 + 0.5 * edge);
    strike_p(cfg.strike_k, s / def.max(1e-3))
}

fn asset_index(a: OutsideAsset) -> usize {
    match a {
        OutsideAsset::Depot => 0,
        OutsideAsset::DataCentre => 1,
        OutsideAsset::Convoy => 2,
    }
}

/// Settle a mission: the draw, the asset's effect, deaths abroad, the
/// survivors home, the cast and its promotions, `AbroadResolved`.
fn resolve(world: &mut World, m: &Abroad) {
    let cfg = world.config.outside.abroad.clone();
    let seed = world.seed();
    let p = odds(world, m);
    let won = living_parent(world, m.on) && hash_unit(seed, PURPOSE_ABROAD, u64::from(m.id), 0) < p;
    let pname = parent_name(world, m.on);
    let what = if won { effect(world, m) } else { "came back empty-handed".to_string() };
    // Deaths abroad, then the survivors home.
    let mut dead = Vec::new();
    for &a in &m.crew {
        world.outside.away.remove(&a);
        if !crate::systems::law::living(world, a) {
            continue;
        }
        let u = hash_unit(seed, PURPOSE_ABROAD_DEATH, u64::from(m.id), u64::from(a.index));
        if u < cfg.death_p * (1.0 - p) {
            dead.push(a);
            world.kill_by(a, DeathCause::Abroad, None);
            // Buried abroad: no body lies in the city.
            let now = world.tick;
            if let Some(c) = world.comp_mut::<crate::components::Corpse>(a) {
                c.buried = true;
                c.buried_tick = Some(now);
            }
            crate::systems::assets::on_buried(world, a);
            continue;
        }
        come_home(world, a);
    }
    world.outside.today.abroad_done += 1;
    world.outside.today.abroad_won += u32::from(won);
    world.outside.today.abroad_dead += dead.len() as u32;
    // The cast, and its promotions.
    let leader = m.crew.iter().copied().find(|&a| crate::systems::law::living(world, a));
    let promoted = cast(world, m, won, leader);
    let lost = if dead.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = dead.iter().map(|&a| world.name_of(a)).collect();
        format!("; {} died abroad", names.join(", "))
    };
    let met = if promoted > 0 { format!("; {promoted} they met will come") } else { String::new() };
    let text = format!(
        "the job on {pname}'s {} (odds {p:.2}): {}{what}{lost}{met}",
        m.asset.label(),
        if won { "done: " } else { "" }
    );
    world.push_event(EventKind::AbroadResolved, &m.crew, text);
}

/// A won mission's effect on the parent and the buyer.
fn effect(world: &mut World, m: &Abroad) -> String {
    let cfg = world.config.outside.abroad.clone();
    match m.asset {
        OutsideAsset::Depot => {
            let living: Vec<EntityId> =
                m.crew.iter().copied().filter(|&a| crate::systems::law::living(world, a)).collect();
            let crew_take = ((cfg.loot as f64) * f64::from(cfg.crew_share)).floor() as i64;
            let each = crew_take / living.len().max(1) as i64;
            let mut moved = 0;
            for &a in &living {
                moved += ownership::cross_in(world, m.on, Some(a), each, Flow::Payout);
            }
            let to = m.buyer.filter(|&b| world.is_alive(b));
            moved +=
                ownership::cross_in(world, m.on, to.or(living.first().copied()), cfg.loot - crew_take, Flow::Payout);
            format!("{moved} coins taken")
        }
        OutsideAsset::DataCentre => {
            let payload = crate::systems::parents::take_abroad(world, m.on, cfg.data * 3);
            let units: u32 = payload.iter().sum();
            let lab = m
                .buyer
                .map(|b| crate::systems::tech::labs_of(world, b))
                .and_then(|v| v.first().copied())
                .and_then(|l| crate::systems::virt::node_of_building(world, l));
            // M17 review: no netrun (no `UplinkBreached`, no uplink counters);
            // the edge falls only when the Data left the parent.
            match lab.and_then(|n| world.virt.node_mut(n)) {
                Some(node) => {
                    for (u, x) in node.store.units.iter_mut().zip(payload) {
                        *u += x;
                    }
                    crate::systems::parents::cut_edge(world, m.on, units);
                    format!("{units} Data taken")
                }
                None => {
                    crate::systems::parents::return_abroad(world, m.on, payload);
                    "the Data had nowhere to go and stayed abroad".to_string()
                }
            }
        }
        OutsideAsset::Convoy => {
            crate::systems::parents::crew_sanction(world, m.on, cfg.convoy_days, cfg.convoy_cut);
            "the convoy burned".to_string()
        }
    }
}

/// A crew member back: Coarse, at the edge road nearest where they stood.
fn come_home(world: &mut World, a: EntityId) {
    crate::systems::lod::set_lod(world, a, Lod::Coarse);
    let from = world.comp::<Position>(a).map(|p| p.tile).unwrap_or_default();
    if let Some(t) = crate::systems::demography::nearest_edge_road(world, from) {
        world.remove_from_building(a);
        if let Some(p) = world.comp_mut::<Position>(a) {
            p.tile = t;
        }
    }
}

/// The mission's `cast_size` NPCs (records), and the promotions among them.
/// Returns how many were promoted.
fn cast(world: &mut World, m: &Abroad, won: bool, leader: Option<EntityId>) -> u32 {
    let cfg = world.config.outside.abroad.clone();
    let seed = world.seed();
    let honour = leader.map_or(0.5, |l| crate::systems::reputation::rep(world, l).honour);
    let today = world.day();
    let mut n = 0;
    for i in 0..cfg.cast_size {
        let h = |k: u64| hash_unit(seed, PURPOSE_CAST, u64::from(m.id) << 16 | u64::from(i), k);
        let sex = if h(0) < 0.5 { Sex::Male } else { Sex::Female };
        let pick =
            |t: &[String], u: f32| t.get(((u * t.len() as f32) as usize).min(t.len().saturating_sub(1))).cloned();
        let first = pick(world.names.first_names(sex), h(1)).unwrap_or_else(|| "Nameless".into());
        let last = pick(world.names.last_names(), h(2)).unwrap_or_default();
        let role = [CastRole::Guard, CastRole::Clerk, CastRole::Runner, CastRole::Fixer, CastRole::Bystander]
            [((h(3) * 5.0) as usize).min(4)];
        let personality = [h(4), h(5), h(6), h(7), h(8), h(9)];
        let base = if won { 0.3 } else { -0.3 };
        let disposition = (base + (honour - 0.5) + (h(10) - 0.5) * 1.6).clamp(-1.0, 1.0);
        let member = CastMember {
            seed: crate::rng::splitmix64(seed ^ (u64::from(m.id) << 16 | u64::from(i))),
            name: format!("{first} {last}").trim().to_string(),
            role,
            personality,
            disposition,
        };
        let rel = if disposition >= cfg.promote_friend {
            RelKind::Friend
        } else if disposition <= cfg.promote_enemy {
            RelKind::Enemy
        } else {
            continue;
        };
        let Some(toward) = leader else { continue };
        let pending = world.outside.promoted.len() + world.outside.promoted_living.len();
        if pending >= cfg.promoted_max as usize || h(11) >= cfg.promote_p {
            continue;
        }
        let arrive_on = today + 1 + (h(12) * cfg.promote_delay_days.max(1) as f32) as u64;
        world.outside.promoted.push(Promoted { cast: member, toward, rel, arrive_on });
        world.outside.today.promoted += 1;
        n += 1;
    }
    n
}

/// The promoted whose day has come arrive as immigrants with their stored
/// name and personality and their edge to `toward` (an Enemy with a grudge
/// the Hunt reads). One whose `toward` is dead arrives with no edge.
fn arrivals(world: &mut World) {
    let today = world.day();
    let due: Vec<Promoted> = world.outside.promoted.iter().filter(|p| p.arrive_on <= today).cloned().collect();
    world.outside.promoted.retain(|p| p.arrive_on > today);
    for p in due {
        let door = world.comp::<Position>(p.toward).map(|x| x.tile);
        let home = crate::systems::demography::emptiest_home_near(world, door, 1);
        let tile = door.and_then(|d| crate::systems::demography::nearest_edge_road(world, d));
        let a = crate::systems::demography::spawn_outsider(world, home, tile);
        world.stats.current.immigrants += 1;
        if let Some(id) = world.comp_mut::<Identity>(a) {
            id.name = p.cast.name.clone();
        }
        if let Some(x) = world.comp_mut::<Personality>(a) {
            let q = p.cast.personality;
            *x = Personality {
                lawfulness: q[0],
                greed: q[1],
                pride: q[2],
                sociability: q[3],
                courage: q[4],
                loyalty: q[5],
            };
        }
        let alive = crate::systems::law::living(world, p.toward);
        if alive {
            let now = world.tick;
            let e = world.edge_entry(a, p.toward);
            e.kind = p.rel;
            e.affinity = if p.rel == RelKind::Friend { 0.6 } else { -0.6 };
            e.last_interaction = now;
            world.note_kin(a, p.toward);
            if p.rel == RelKind::Enemy {
                crate::systems::grudges::add(world, a, p.toward, GrudgeCause::Abroad, 0.7, 0);
            }
        }
        world.outside.promoted_living.push(a);
        world.outside.today.cast_arrived += 1;
        let (who, to) = (world.name_of(a), world.name_of(p.toward));
        let how = match (alive, p.rel) {
            (false, _) => format!("came looking for {to}, who is dead"),
            (true, RelKind::Friend) => format!("came to the city to find {to}, a friend from a job abroad"),
            _ => format!("came to the city after {to}, an enemy from a job abroad"),
        };
        let actors = if alive { vec![a, p.toward] } else { vec![a] };
        world.push_event(EventKind::CastArrived, &actors, format!("{who} {how}"));
    }
}

/// A god's `PostAbroad`.
pub fn god_post(
    world: &mut World,
    buyer: Option<EntityId>,
    on: OutsideId,
    asset: OutsideAsset,
    crew: &[EntityId],
) -> Result<String, String> {
    let id = post(world, buyer, on, asset, crew)?;
    Ok(format!("God: mission {id} abroad on {}'s {}", parent_name(world, on), asset.label()))
}

/// § 2 Campaign's last resort: a mission on the rival's depot by
/// `campaign_crew_min` or more of the parent's own Outsiders (not brothers),
/// its first corp branch the buyer of record. True when one left.
pub fn campaign_post(world: &mut World, id: OutsideId, rival: OutsideId) -> bool {
    let cfg = world.config.outside.abroad.clone();
    let crew: Vec<EntityId> = world
        .outside
        .outsiders
        .iter()
        .filter(|(&a, o)| o.from == id && !o.brother && crate::systems::contracts::free_adult(world, a))
        .map(|(&a, _)| a)
        .take(cfg.crew_max)
        .collect();
    if crew.len() < cfg.campaign_crew_min {
        return false;
    }
    let buyer = world
        .outside
        .faction(id)
        .and_then(|f| f.parent.as_ref())
        .and_then(|p| p.branches.iter().copied().find(|&b| world.has::<Corp>(b)));
    post(world, buyer, rival, OutsideAsset::Depot, &crew).is_ok()
}

// ---------------------------------------------------------------------------
// The Blackwall and the traces (§ 6)
// ---------------------------------------------------------------------------

/// From `parents::uplink_landed`: a run that landed on `on`'s uplink. The
/// outside traces the runner into its branches' databases, and a run at
/// the Blackwall's cap rolls a breach (the uplink admits no lower deck).
pub fn on_uplink_run(world: &mut World, on: OutsideId, runner: EntityId, patron: Option<EntityId>) {
    if runner == EntityId::NONE {
        return;
    }
    trace(world, on, runner);
    let cfg = world.config.outside.blackwall.clone();
    let now = world.tick;
    let cooled =
        world.outside.last_breach.is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.cooldown_days) * TICKS_PER_DAY);
    let u = hash_unit(world.seed(), PURPOSE_BREACH, u64::from(runner.index), now);
    if cooled && u < cfg.breach_p && world.outside.breach_due.is_none() {
        // It lands at the next midnight, outside the run's own resolution.
        world.outside.breach_due = Some((on, runner, patron));
    }
}

/// A sighting of `who` at `trace_conf` in each of `on`'s branches' databases.
pub fn trace(world: &mut World, on: OutsideId, who: EntityId) {
    let conf = world.config.outside.blackwall.trace_conf;
    let tile = world.comp::<Position>(who).map(|p| p.tile).unwrap_or_default();
    let tick = world.tick;
    let branches: Vec<EntityId> = world
        .outside
        .faction(on)
        .and_then(|f| f.parent.as_ref())
        .map_or_else(Vec::new, |p| p.branches.iter().copied().filter(|&b| world.is_alive(b)).collect());
    for b in branches {
        crate::systems::virt::record_sighting(
            world,
            Some(b),
            crate::virt::Sighting { who, tile, tick, confidence: conf, relayed: false },
        );
    }
}

/// From `World::kill_by`: one of a parent's Outsiders killed by a known
/// killer is traced. Nothing with no Outsider.
pub fn on_death(world: &mut World, dead: EntityId, killer: Option<EntityId>) {
    if world.outside.outsiders.is_empty() {
        return;
    }
    let Some(k) = killer.filter(|&k| k != dead) else { return };
    let Some(from) = world.outside.outsiders.get(&dead).map(|o| o.from) else { return };
    trace(world, from, k);
}

/// A Blackwall breach on `on`'s uplink: the parent's Data abroad and the
/// patron's Lab stores wiped, the runner flatlined, every node of the
/// patron's alarmed for `alarm_days`. `BlackwallBreach`.
pub fn breach(world: &mut World, on: OutsideId, runner: EntityId, patron: Option<EntityId>) {
    let cfg = world.config.outside.blackwall.clone();
    let now = world.tick;
    world.outside.last_breach = Some(now);
    world.outside.today.breaches += 1;
    if let Some(p) = world.outside.faction_mut(on).and_then(|f| f.parent.as_mut()) {
        p.data.units = [0; 3];
    }
    let alarm = now + u64::from(cfg.alarm_days) * TICKS_PER_DAY;
    if let Some(pt) = patron {
        for n in 0..world.virt.nodes.len() {
            let nid = crate::virt::NodeId(n as u16);
            if crate::systems::virt::owner_of(world, nid) != Some(pt) {
                continue;
            }
            let lab = match world.virt.node(nid).map(|x| x.kind) {
                Some(crate::virt::NodeKind::Building(b)) => world
                    .comp::<crate::components::Building>(b)
                    .is_some_and(|b| b.kind == crate::components::BuildingKind::Lab),
                _ => false,
            };
            if let Some(node) = world.virt.node_mut(nid) {
                if lab {
                    node.store.units = [0; 3];
                }
                node.alarm_until = Some(node.alarm_until.map_or(alarm, |t| t.max(alarm)));
            }
        }
    }
    let pname = parent_name(world, on);
    let who = if runner == EntityId::NONE { "a runner".to_string() } else { world.name_of(runner) };
    let text = format!("{who} hit the Blackwall behind {pname}'s uplink: everything it touched is gone");
    world.push_event(EventKind::BlackwallBreach, &[runner, patron.unwrap_or(EntityId::NONE)], text);
    if runner != EntityId::NONE && crate::systems::law::living(world, runner) {
        world.kill_by(runner, DeathCause::Flatline, None);
    }
}

/// God `BlackwallBreach`: a breach on `on`'s uplink now (no runner).
pub fn god_breach(world: &mut World, on: OutsideId, patron: Option<EntityId>) -> Result<String, String> {
    if !living_parent(world, on) {
        return Err(format!("BlackwallBreach: no living parent {on}"));
    }
    breach(world, on, EntityId::NONE, patron);
    Ok(format!("God: the Blackwall behind {}", parent_name(world, on)))
}

/// `SetStateGuards`: the State's guards at most (`None`: the config's), and
/// at once down to it.
pub fn set_state_guards(world: &mut World, cap: Option<u16>) -> Result<String, String> {
    world.outside.state_guards_cap = cap;
    let max = crate::systems::parents::state_cap(world);
    world.outside.state_guards = world.outside.state_guards.min(max);
    Ok(match cap {
        Some(c) => format!("The State's guards capped at {c}"),
        None => "The State's guards uncapped".to_string(),
    })
}
