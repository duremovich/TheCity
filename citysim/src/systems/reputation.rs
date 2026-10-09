//! M15 § 2 (plan phase 1.8, W13, W14, W48): reputation, rebuilt at
//! midnight from what is known (every living adult's deed memories and the
//! district pools), never from what happened (`Life`, holes, the event
//! ring). Four numbers per living adult and per faction (gang, corp, the
//! Law), the faction x faction regard matrix, and the watched killers'
//! `known_by` seven days on. Phase 1: computed and saved, read by nobody.

use std::collections::BTreeMap;

use smallvec::SmallVec;

use crate::components::{Appearance, Building, Class, Corp, Gang, GangMember, Job, Law, Memory, Niche, Role};
use crate::entity::EntityId;
use crate::systems::memory;
use crate::time::{self, TICKS_PER_DAY};
use crate::word::{Audience, Creed, Deed, Reputation, Taste};
use crate::world::World;

/// `1 − e^(−D ÷ scale)`.
pub fn dread_of(d: f32, scale: f32) -> f32 {
    1.0 - (-d / scale.max(1e-6)).exp()
}

/// `clamp(0.5 + H ÷ (2 × scale), 0, 1)`.
pub fn honour_of(h: f32, scale: f32) -> f32 {
    (0.5 + h / (2.0 * scale.max(1e-6))).clamp(0.0, 1.0)
}

/// `max(1 − e^(−X ÷ scale), law_heat)`.
pub fn heat_of(x: f32, scale: f32, law_heat: f32) -> f32 {
    (1.0 - (-x / scale.max(1e-6)).exp()).max(law_heat).clamp(0.0, 1.0)
}

/// `pos × (0.4 + 0.6 × fame)`, `fame = 1 − e^(−known_by ÷ fame_scale)`.
pub fn standing_of(pos: f32, known_by: u16, fame_scale: f32) -> f32 {
    let fame = 1.0 - (-f32::from(known_by) / fame_scale.max(1e-6)).exp();
    (pos * (0.4 + 0.6 * fame)).clamp(0.0, 1.0)
}

/// The weight one held deed memory adds: `weight × conf × hop_w[min(hops, 3)]`.
pub fn memory_weight(world: &World, e: &crate::components::MemoryEntry) -> f32 {
    let cfg = &world.config.reputation;
    memory::weight(e, world.tick, cfg.half_life_days) * e.conf * cfg.hop_w[usize::from(memory::hops_of(e).min(3))]
}

/// A reputation, the default (nothing known) when absent or when `id` is
/// not the live entity at its index (a stale id never reads its slot's
/// new owner).
pub fn rep(world: &World, id: EntityId) -> Reputation {
    if !world.is_alive(id) {
        return Reputation::default();
    }
    world.reputation.get(id.index as usize).and_then(|r| r.clone()).unwrap_or_default()
}

/// `known_by` of an agent or faction (0 when absent or stale).
pub fn known_by(world: &World, id: EntityId) -> u16 {
    if !world.is_alive(id) {
        return 0;
    }
    world.reputation.get(id.index as usize).and_then(Option::as_ref).map_or(0, |r| r.known_by)
}

/// W14: how faction `a` regards faction `b`; neutral when absent.
pub fn regard(world: &World, a: EntityId, b: EntityId) -> crate::word::Regard {
    world.regard.get(&(a, b)).cloned().unwrap_or_default()
}

/// W30 stub: the stored `Appearance`, else a class's default dress (Corp
/// 2, Street 1, Dreg 0), no chrome, the gang's or the employer corp's colours.
pub fn appearance_of(world: &World, id: EntityId) -> Appearance {
    if let Some(a) = world.comp::<Appearance>(id) {
        return a.clone();
    }
    let dress = match crate::systems::classes::class_of(world, id) {
        Class::Corp => 2,
        Class::Street => 1,
        Class::Dreg => 0,
    };
    let colours = world.gang_of(id).or_else(|| world.corp_of_agent(id));
    Appearance { dress, chrome: 0, colours, dress_pin: None }
}

/// W30: whose eyes an agent sees with: its class (Corp, Street, Dreg),
/// overridden by gang membership (a Purist gang's members: `Purist`).
pub fn audience_of(world: &World, id: EntityId) -> Audience {
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<Gang>(g)) {
        return if g.creed == Some(Creed::Purist) { Audience::Purist } else { Audience::Gang };
    }
    match crate::systems::classes::class_of(world, id) {
        Class::Corp => Audience::Corp,
        Class::Street => Audience::Street,
        Class::Dreg => Audience::Dreg,
    }
}

/// The audience's taste weights (`[taste]`).
pub fn taste_weights(world: &World, a: Audience) -> Taste {
    let t = &world.config.taste;
    match a {
        Audience::Corp => t.corp,
        Audience::Street => t.street,
        Audience::Dreg => t.dreg,
        Audience::Gang => t.gang,
        Audience::Purist => t.purist,
    }
}

/// Are `a` and `b` rival colours to the observer's gang (`rival_of`)?
fn rival_colours(world: &World, observer: EntityId, colours: Option<EntityId>) -> bool {
    let (Some(g), Some(c)) = (world.gang_of(observer), colours) else { return false };
    world.rival_of(g) == Some(c)
}

/// W30, spec § 2: how `observer` sees `target`: `dress × (2 × dress_t ÷ 3
/// − 1) + chrome × min(chrome_t ÷ chrome_ref, 1) + own_colours × [same
/// colours] + rival_colours × [rival colours]`, clamped −1..1. A Purist
/// audience reads `chrome = −1` for a target above `creed_tolerance`
/// (`Kit.visible`) and nothing for one within it (the tolerance rule).
pub fn taste(world: &World, observer: EntityId, target: EntityId) -> f32 {
    let aud = audience_of(world, observer);
    let w = taste_weights(world, aud);
    let app = appearance_of(world, target);
    let dress = w.dress * (2.0 * f32::from(app.dress) / 3.0 - 1.0);
    let chrome = if aud == Audience::Purist {
        let visible = world.comp::<crate::components::Kit>(target).map_or(app.chrome, |k| k.visible);
        if visible > world.config.creeds.creed_tolerance {
            -1.0
        } else {
            0.0
        }
    } else {
        let r = f32::from(world.config.taste.chrome_ref.max(1));
        w.chrome * (f32::from(app.chrome) / r).min(1.0)
    };
    let mine = world.gang_of(observer).or_else(|| world.corp_of_agent(observer));
    let own = if app.colours.is_some() && app.colours == mine { w.own_colours } else { 0.0 };
    let rival = if rival_colours(world, observer, app.colours) { w.rival_colours } else { 0.0 };
    (dress + chrome + own + rival).clamp(-1.0, 1.0)
}

/// W14's taste term of `opinion`: the observer's audience on a faction's
/// colours (its own: `own_colours`; the observer's gang's rival:
/// `rival_colours`).
pub fn colours_taste(world: &World, observer: EntityId, faction: EntityId) -> f32 {
    let w = taste_weights(world, audience_of(world, observer));
    let mine = world.gang_of(observer).or_else(|| world.corp_of_agent(observer));
    let own = if mine == Some(faction) { w.own_colours } else { 0.0 };
    let rival = if rival_colours(world, observer, Some(faction)) { w.rival_colours } else { 0.0 };
    (own + rival).clamp(-1.0, 1.0)
}

/// Every live faction: gangs, corps, then the Law (the Jail's entity), ascending within each.
pub fn factions(world: &World) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = world.gang_list().to_vec();
    out.extend(world.corps());
    if let Some(jail) = world.building_of_kind(crate::components::BuildingKind::Jail).filter(|&j| world.has::<Law>(j)) {
        out.push(jail);
    }
    out
}

/// A faction's members: a gang's roster, a corp's staff and exec, the Law's guards.
pub fn members_of(world: &World, faction: EntityId) -> Vec<EntityId> {
    if let Some(g) = world.comp::<Gang>(faction) {
        return g.members.clone();
    }
    if let Some(c) = world.comp::<Corp>(faction) {
        let mut v = crate::systems::ownership::employees_of(world, faction);
        if let Some(e) = c.exec.filter(|e| !v.contains(e)) {
            v.push(e);
            v.sort_unstable();
        }
        return v;
    }
    if world.has::<Law>(faction) {
        return crate::systems::law_brain::guards(world);
    }
    Vec::new()
}

/// W38: the agent's ties the press term reads: its employer corp and that
/// corp's exec, its gang, and its Home's owner (a corp or a landlord).
pub fn press_ties(world: &World, agent: EntityId) -> SmallVec<[EntityId; 4]> {
    let mut out: SmallVec<[EntityId; 4]> = SmallVec::new();
    if let Some(c) = world.corp_of_agent(agent) {
        out.push(c);
        if let Some(e) = world.comp::<Corp>(c).and_then(|cc| cc.exec).filter(|&e| e != agent) {
            out.push(e);
        }
    }
    if let Some(g) = world.gang_of(agent) {
        out.push(g);
    }
    if let Some(o) = world
        .comp::<crate::components::Household>(agent)
        .and_then(|h| h.home)
        .and_then(|h| world.owner_of(h))
        .filter(|&o| o != agent && !out.contains(&o))
    {
        out.push(o);
    }
    out
}

/// W38: Σ over the agent's heard story entries whose actor is one of its
/// ties (`press_ties`) of `slant × conf` (the entry's `press ÷ 100`).
pub fn press_of(world: &World, agent: EntityId) -> f32 {
    let Some(m) = world.comp::<Memory>(agent) else { return 0.0 };
    if !m.heard.iter().any(|e| e.press != 0) {
        return 0.0;
    }
    let ties = press_ties(world, agent);
    m.heard
        .iter()
        .filter(|e| e.press != 0 && e.subject.is_some_and(|s| ties.contains(&s)))
        .map(|e| f32::from(e.press) / 100.0 * e.conf)
        .sum()
}

/// W14: an agent's opinion of a faction, on demand: `0.4 × mean affinity
/// to its members the agent has edges with + 0.3 × own deed memories about
/// it (valence-weighted) + 0.2 × (honour − 0.5) × 2 + 0.1 × taste (W30:
/// `colours_taste`, 0 with moves off) + press_w × press` (W38: when the
/// faction is one of the agent's ties), plus `own_bias` for the employer
/// and the gang, clamped −1..1.
pub fn opinion(world: &World, agent: EntityId, faction: EntityId) -> f32 {
    let members = members_of(world, faction);
    let affs: Vec<f32> = members.iter().filter_map(|&m| world.edge(agent, m).map(|e| e.affinity)).collect();
    let aff = if affs.is_empty() { 0.0 } else { affs.iter().sum::<f32>() / affs.len() as f32 };
    let own: f32 = world
        .comp::<Memory>(agent)
        .map(|m| {
            memory::deeds(agent, m)
                .filter(|(_, r)| {
                    r.actor.is_some_and(|a| a == faction || members.binary_search(&a).is_ok())
                        || r.object.is_some_and(|o| o == faction || members.binary_search(&o).is_ok())
                })
                .map(|(e, _)| e.valence * e.conf)
                .sum::<f32>()
        })
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let honour = rep(world, faction).honour;
    let press = if press_ties(world, agent).contains(&faction) {
        world.config.news.press_w * rep(world, agent).press
    } else {
        0.0
    };
    let taste = colours_taste(world, agent, faction);
    let bias = if world.gang_of(agent) == Some(faction) || world.corp_of_agent(agent) == Some(faction) {
        world.config.reputation.own_bias
    } else {
        0.0
    };
    (0.4 * aff + 0.3 * own + 0.2 * (honour - 0.5) * 2.0 + 0.1 * taste + press + bias).clamp(-1.0, 1.0)
}

/// Per-actor accumulators of one rebuild.
struct Acc {
    d: Vec<f32>,
    h: Vec<f32>,
    x: Vec<f32>,
    /// Per actor, the summed weight per deed (`Deed::ALL` order).
    by_deed: Vec<[f32; Deed::ALL.len()]>,
    known: Vec<u16>,
}

impl Acc {
    fn new(n: usize) -> Acc {
        Acc {
            d: vec![0.0; n],
            h: vec![0.0; n],
            x: vec![0.0; n],
            by_deed: vec![[0.0; Deed::ALL.len()]; n],
            known: vec![0; n],
        }
    }

    /// Accumulates on the actor's index only while `actor` is the live
    /// entity there (generation included): a rumour of a despawned agent
    /// must not land on the immigrant who reused its index.
    fn add(&mut self, world: &World, actor: EntityId, deed: Deed, w: f32) {
        let i = actor.index as usize;
        if i >= self.d.len() || !world.is_alive(actor) {
            return;
        }
        let cfg = &world.config.reputation;
        self.d[i] += w * cfg.dread_w.get(deed);
        self.h[i] += w * cfg.honour_w.get(deed);
        self.x[i] += w * cfg.heat_w.get(deed);
        self.by_deed[i][deed.index()] += w;
    }

    fn get<T: Copy + Default>(v: &[T], id: EntityId) -> T {
        v.get(id.index as usize).copied().unwrap_or_default()
    }
}

/// W13: the daily rebuild (O(memory entries + pool entries)). Per living
/// adult holder ascending, per deed memory with a named actor that is not
/// the holder: `w = weight × conf × hop_w[min(hops, 3)]` into the actor's
/// D, H and X; each pool entry adds `reach × pool_w × hop_w[hops]`;
/// `known_by` counts the distinct living holders of any memory whose
/// subject is the agent (spec). Then the axes of every living adult, every
/// gang, corp and the Law, the regard matrix, and the W48 sample.
pub fn rebuild(world: &mut World) {
    let cfg = world.config.reputation.clone();
    let now = world.tick;
    let n = world.alive.len();
    let mut acc = Acc::new(n);
    // scan-ok: daily (the word's midnight chain).
    let citizens = world.citizens();
    let adults: Vec<EntityId> =
        citizens.iter().copied().filter(|&id| crate::systems::demography::is_adult(world, id)).collect();
    for &h in &adults {
        let Some(m) = world.comp::<Memory>(h) else { continue };
        let mut subjects: SmallVec<[EntityId; 32]> = SmallVec::new();
        for e in m.entries.iter().chain(m.heard.iter()) {
            if let Some(s) = e.subject.filter(|&s| s != h) {
                if !subjects.contains(&s) {
                    subjects.push(s);
                }
            }
        }
        for s in subjects.into_iter().filter(|&s| world.is_alive(s)) {
            if let Some(k) = acc.known.get_mut(s.index as usize) {
                *k = k.saturating_add(1);
            }
        }
        let deeds: SmallVec<[(Deed, EntityId, f32); 32]> = memory::deeds(h, m)
            .filter_map(|(e, r)| r.actor.filter(|&a| a != h).map(|a| (r.deed, a, memory_weight(world, e))))
            .collect();
        for (deed, a, w) in deeds {
            acc.add(world, a, deed, w);
        }
    }
    let pool_adds: Vec<(Deed, EntityId, f32)> = world
        .rumours
        .iter()
        .flat_map(|p| p.entries.iter())
        .filter_map(|e| e.actor.map(|a| (e.deed, a, e.reach * cfg.pool_w * cfg.hop_w[usize::from(e.hops.min(3))])))
        .collect();
    for (deed, a, w) in pool_adds {
        acc.add(world, a, deed, w);
    }

    // The law's own records: 30 days of sentences.
    let horizon = now.saturating_sub(30 * TICKS_PER_DAY);
    while world.arrest_log.front().is_some_and(|&(t, _)| t < horizon) {
        world.arrest_log.pop_front();
    }
    let mut arrests: BTreeMap<EntityId, u32> = BTreeMap::new();
    for &(_, a) in &world.arrest_log {
        *arrests.entry(a).or_default() += 1;
    }

    // Standing's position terms.
    let execs = crate::systems::classes::exec_set(world);
    let mut owned: BTreeMap<EntityId, u32> = BTreeMap::new();
    for id in world.with::<Building>() {
        if let Some(o) = world.comp::<Building>(id).filter(|b| !b.demolished).and_then(|b| b.owner) {
            *owned.entry(o).or_default() += 1;
        }
    }
    let mut leaders: BTreeMap<EntityId, f32> = BTreeMap::new();
    for g in world.gang_list().to_vec() {
        let ranking = crate::systems::gang::leader_ranking(world, g);
        if let Some(l) = world.comp::<Gang>(g).and_then(|x| x.leader) {
            leaders.insert(l, 0.7);
        }
        if let Some(&lt) = ranking.get(1) {
            leaders.entry(lt).or_insert(0.5);
        }
    }
    let captain = world.law().and_then(|l| l.captain);
    let mut out: Vec<(EntityId, Reputation)> = Vec::with_capacity(adults.len() + 32);
    for &a in &adults {
        let kb = Acc::get(&acc.known, a);
        let wanted = crate::systems::law::wanted(world, a);
        let law_heat = (if wanted { cfg.heat_wanted } else { 0.0 })
            .max((cfg.heat_arrest * arrests.get(&a).copied().unwrap_or(0) as f32).min(1.0));
        let class = crate::systems::classes::class_in(world, a, &execs);
        let mut pos: f32 = 0.0;
        if execs.contains(&a) {
            pos = pos.max(0.9);
        }
        if let Some(&k) = owned.get(&a) {
            pos = pos.max((0.5 + 0.1 * k as f32).min(0.8));
        }
        if let Some(&l) = leaders.get(&a) {
            pos = pos.max(l);
        } else if world.has::<GangMember>(a) {
            pos = pos.max(0.3);
        }
        if captain == Some(a) {
            pos = pos.max(0.7);
        } else if crate::systems::law::is_guard(world, a) {
            pos = pos.max(0.35);
        }
        if world.has::<Job>(a) {
            pos = pos.max(0.2);
        }
        if class == Class::Dreg {
            pos = pos.max(0.05);
        }
        let dress = world.comp::<Appearance>(a).map_or_else(|| appearance_of(world, a).dress, |x| x.dress);
        pos += 0.2 * f32::from(dress.min(3)) / 3.0;
        let r = Reputation {
            dread: dread_of(Acc::get(&acc.d, a), cfg.dread_scale),
            standing: standing_of(pos, kb, cfg.fame_scale),
            honour: honour_of(Acc::get(&acc.h, a), cfg.honour_scale),
            heat: heat_of(Acc::get(&acc.x, a), cfg.heat_scale, law_heat),
            known_by: kb,
            top: top_of(&acc, a),
            pinned: None,
            press: press_of(world, a),
        };
        out.push((a, r));
    }
    let by_id: BTreeMap<EntityId, Reputation> = out.iter().cloned().collect();
    let get_r = |id: EntityId| by_id.get(&id).cloned().unwrap_or_default();

    // Gangs: dread from their members' mean dread and the deeds told of the
    // gang itself; honour the members' mean; heat the M9 pressure or the
    // members' mean heat; standing from turf and treasury. M15 phase 5: the
    // spec's three most feared (hottest) members read 0.9-1.0 for every
    // gang by day 30 (each gang has three known killers), so the brains'
    // fear and pressure terms saw no difference between gangs; the mean
    // reads how violent (wanted) the gang is per head, which spreads them.
    let reports = crate::systems::law_brain::reports_by_gang(world);
    let full = crate::systems::law_brain::crackdown_reports(world).max(1) as f32;
    let homes = world.buildings_of_kind(crate::components::BuildingKind::Home).len().max(1) as f32;
    let hoard = world.config.corps.hoard_heat.max(1) as f32;
    for g in world.gang_list().to_vec() {
        let Some(gg) = world.comp::<Gang>(g) else { continue };
        let members: Vec<Reputation> = gg.members.iter().map(|&m| get_r(m)).collect();
        let mean = |f: fn(&Reputation) -> f32| {
            if members.is_empty() {
                0.0
            } else {
                members.iter().map(f).sum::<f32>() / members.len() as f32
            }
        };
        let honour = if members.is_empty() { 0.5 } else { mean(|r| r.honour) };
        let pressure = reports.get(&g).copied().unwrap_or(0) as f32 / full;
        let standing =
            (0.3 + 0.4 * gg.territory.len() as f32 / homes + 0.3 * gg.treasury.max(0) as f32 / hoard).clamp(0.0, 1.0);
        let r = Reputation {
            dread: 0.5 * mean(|r| r.dread) + 0.5 * dread_of(Acc::get(&acc.d, g), cfg.dread_scale),
            standing,
            honour,
            heat: pressure.max(mean(|r| r.heat)).clamp(0.0, 1.0),
            known_by: Acc::get(&acc.known, g),
            top: top_of(&acc, g),
            pinned: None,
            press: 0.0,
        };
        out.push((g, r));
    }
    // Corps: standing from niche share and cash; honour from the corp's own
    // deeds and its exec's at 0.3; dread from its deeds and its private
    // guards' killings; heat from its deeds and its staff's arrests.
    let mut shares: BTreeMap<Niche, BTreeMap<EntityId, f32>> = BTreeMap::new();
    for niche in [Niche::Food, Niche::Housing, Niche::Security, Niche::Tech] {
        shares.insert(niche, crate::systems::corp_brain::shares(world, niche));
    }
    for c in world.corps() {
        let Some(cc) = world.comp::<Corp>(c) else { continue };
        let share = if cc.niches.is_empty() {
            0.0
        } else {
            cc.niches.iter().map(|n| shares.get(n).and_then(|s| s.get(&c)).copied().unwrap_or(0.0)).sum::<f32>()
                / cc.niches.len() as f32
        };
        let cash = (cc.treasury.max(0) as f32 / cc.treasury_ref.max(1) as f32).min(2.0);
        let exec_honour = cc.exec.map_or(0.5, |e| get_r(e).honour);
        let staff = crate::systems::ownership::employees_of(world, c);
        let kill = cfg.dread_w.killed;
        let guard_d: f32 = staff
            .iter()
            .filter(|&&s| world.comp::<Job>(s).is_some_and(|j| j.role == Role::Guard))
            .map(|&s| Acc::get(&acc.by_deed, s)[Deed::Killed.index()] * kill)
            .sum();
        let arrest_x: f32 =
            staff.iter().map(|&s| Acc::get(&acc.by_deed, s)[Deed::Arrested.index()] * cfg.heat_w.arrested).sum();
        // The Real economy E42: a camp that could not feed takes standing off its owner.
        let scandal = crate::systems::camp::scandal_penalty(world, Some(c));
        let r = Reputation {
            dread: dread_of(Acc::get(&acc.d, c) + guard_d, cfg.dread_scale),
            standing: (0.5 * share + 0.5 * cash / 2.0 + scandal).clamp(0.0, 1.0),
            honour: 0.7 * honour_of(Acc::get(&acc.h, c), cfg.honour_scale) + 0.3 * exec_honour,
            heat: heat_of(Acc::get(&acc.x, c) + arrest_x, cfg.heat_scale, 0.0),
            known_by: Acc::get(&acc.known, c),
            top: top_of(&acc, c),
            pinned: None,
            press: 0.0,
        };
        out.push((c, r));
    }
    // The Law: dread from its guards' killings and beatings; no known
    // bribes yet (honour 0.5); standing fixed at 0.8.
    if let Some(jail) = world.building_of_kind(crate::components::BuildingKind::Jail).filter(|&j| world.has::<Law>(j)) {
        let guards = crate::systems::law_brain::guards(world);
        let d: f32 = guards
            .iter()
            .map(|&g| {
                let b = Acc::get(&acc.by_deed, g);
                b[Deed::Killed.index()] * cfg.dread_w.killed + b[Deed::Assaulted.index()] * cfg.dread_w.assaulted
            })
            .sum();
        // The Real economy E42: the city's camp likewise (the service is the Law's).
        let scandal = crate::systems::camp::scandal_penalty(world, None);
        out.push((
            jail,
            Reputation {
                dread: dread_of(d, cfg.dread_scale),
                standing: (0.8 + scandal).clamp(0.0, 1.0),
                honour: 0.5,
                heat: 0.0,
                known_by: Acc::get(&acc.known, jail),
                top: top_of(&acc, jail),
                pinned: None,
                press: 0.0,
            },
        ));
    }

    // Write: god pins hold until their tick; everyone else not rebuilt loses theirs.
    let mut keep = vec![false; world.reputation.len()];
    for (id, mut r) in out {
        let i = id.index as usize;
        if i >= world.reputation.len() {
            continue;
        }
        if let Some((pin, until)) = world.reputation[i].as_ref().and_then(|old| old.pinned) {
            if now < until {
                r.set_axes(pin);
                r.pinned = Some((pin, until));
            }
        }
        world.reputation[i] = Some(r);
        keep[i] = true;
    }
    for (i, k) in keep.into_iter().enumerate() {
        if !k {
            world.reputation[i] = None;
        }
    }
    rebuild_regard(world);
    sample_kill_watch(world, &acc.known);
    // W40: each corp's honour at this midnight, 14 kept (Spin's honour drop).
    for c in world.corps() {
        let h = rep(world, c).honour;
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.honour_hist.push_back(h);
            while cc.honour_hist.len() > 14 {
                cc.honour_hist.pop_front();
            }
        }
    }
}

/// The four largest per-deed contributions of an actor, biggest first.
fn top_of(acc: &Acc, id: EntityId) -> SmallVec<[(Deed, f32); 4]> {
    let b = Acc::get(&acc.by_deed, id);
    let mut v: SmallVec<[(Deed, f32); Deed::ALL.len()]> =
        Deed::ALL.iter().map(|&d| (d, b[d.index()])).filter(|&(_, w)| w > 0.0).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().take(4).collect()
}

/// W14: every ordered pair of live factions: `value = clamp(Σ_{m ∈ A}
/// opinion_part(m, B) ÷ |A| − vendetta(A, B) + 0.3 × (B.honour − 0.5), −1,
/// 1)` with `opinion_part` the m-weighted valence of m's deed memories
/// whose actor or object is in B (a deed by B against an A member counts
/// double; vendettas are phase 3's); `fear = clamp(B.dread − A.dread +
/// 0.5, 0, 1)`.
pub fn rebuild_regard(world: &mut World) {
    let fs = factions(world);
    let mut member_of: BTreeMap<EntityId, SmallVec<[EntityId; 3]>> = BTreeMap::new();
    let mut rosters: Vec<Vec<EntityId>> = Vec::with_capacity(fs.len());
    for &f in &fs {
        let ms = members_of(world, f);
        for &m in &ms {
            member_of.entry(m).or_default().push(f);
        }
        rosters.push(ms);
    }
    let index: BTreeMap<EntityId, usize> = fs.iter().enumerate().map(|(i, &f)| (f, i)).collect();
    let k = fs.len();
    let mut sum = vec![0.0f32; k * k];
    let in_faction = |x: EntityId| -> SmallVec<[usize; 3]> {
        let mut v: SmallVec<[usize; 3]> =
            member_of.get(&x).map(|fs| fs.iter().filter_map(|f| index.get(f).copied()).collect()).unwrap_or_default();
        if let Some(&i) = index.get(&x) {
            v.push(i);
        }
        v
    };
    for (ai, roster) in rosters.iter().enumerate() {
        for &m in roster {
            let Some(mem) = world.comp::<Memory>(m) else { continue };
            for (e, r) in memory::deeds(m, mem) {
                let v = e.valence * e.conf;
                if v == 0.0 {
                    continue;
                }
                let objects_a = r.object.is_some_and(|o| in_faction(o).contains(&ai));
                if let Some(a) = r.actor {
                    for bi in in_faction(a) {
                        if bi != ai {
                            sum[ai * k + bi] += if objects_a { 2.0 * v } else { v };
                        }
                    }
                }
                if let Some(o) = r.object {
                    for bi in in_faction(o) {
                        if bi != ai {
                            sum[ai * k + bi] += v;
                        }
                    }
                }
            }
        }
    }
    let reps: Vec<Reputation> = fs.iter().map(|&f| rep(world, f)).collect();
    world.regard.clear();
    for ai in 0..k {
        let size = rosters[ai].len().max(1) as f32;
        for bi in 0..k {
            if ai == bi {
                continue;
            }
            let value = (sum[ai * k + bi] / size + 0.3 * (reps[bi].honour - 0.5)).clamp(-1.0, 1.0);
            let fear = (reps[bi].dread - reps[ai].dread + 0.5).clamp(0.0, 1.0);
            world.regard.insert((fs[ai], fs[bi]), crate::word::Regard { value, fear });
        }
    }
}

/// W48: the `known_by` of every watched killer seven days on: appended to
/// `kill_known`, their median into `known_by_killers_median` (−1 when none);
/// a killer since despawned (its index maybe reused) is not sampled.
fn sample_kill_watch(world: &mut World, known: &[u16]) {
    let today = time::day(world.tick);
    let mut sample: Vec<u16> = world
        .kill_watch
        .iter()
        .filter(|&&(t, a)| today.saturating_sub(time::day(t)) == 7 && world.is_alive(a))
        .map(|&(_, a)| known.get(a.index as usize).copied().unwrap_or(0))
        .collect();
    while world.kill_watch.front().is_some_and(|&(t, _)| today.saturating_sub(time::day(t)) > 7) {
        world.kill_watch.pop_front();
    }
    world.kill_known.extend_from_slice(&sample);
    sample.sort_unstable();
    world.stats.current.word.known_by_killers_median =
        if sample.is_empty() { -1.0 } else { f32::from(sample[sample.len() / 2]) };
}
