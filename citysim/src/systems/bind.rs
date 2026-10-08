//! M10 holes and the binder (`docs/M10_SCALE.md` § 3).
//!
//! A Statistical agent's hourly table samples what is done *to* it (robbed,
//! beaten, killed) without drawing who did it: the crime is a `Hole` until
//! something needs the answer. The daily pass binds consequential holes (a
//! killing, or a gang member or guard as victim) by the next morning and any
//! hole past `[bind] hole_ttl_days`; a promoted victim's holes bind at once
//! (`lod::set_lod` queues them); later the inspector binds on demand.
//!
//! A binding is a pure function of `(world seed, hole id)` and the candidates'
//! traces: the hole's own RNG stream (`SimRng::hole`) is never stored, so call
//! order, save/load and repetition draw the same numbers. The draws, in order:
//! the Unknown roll, the candidate, the witness roll, the witness.

use rand::Rng;

use crate::components::{
    trace_flags, Bound, BuildingKind, DayTrace, Hole, HoleId, HoleKind, LawShock, Lod, MemoryKind, Personality, Trace,
    Wallet, Zone,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{self, TICKS_PER_DAY};
use crate::world::World;

/// Insert a hole, index it under the victim, enforce the per-victim cap
/// (oldest goes Unknown without a draw), count it.
pub fn open_hole(world: &mut World, hole: Hole) -> HoleId {
    let (id, victim) = (hole.id, hole.victim);
    // M12 D7: the district's crime counter.
    crate::systems::districts::note_crime_in(world, hole.district);
    // M12 D17: its litter on a street tile of the district drawn from the
    // hole's own key (a killing's is the death's, `kill_by`).
    let amount = match hole.kind {
        crate::components::HoleKind::Robbed => Some((6, 0)),
        crate::components::HoleKind::Assaulted => Some((12, 1)),
        crate::components::HoleKind::Killed | crate::components::HoleKind::Abducted => None,
    };
    if let Some((a, r)) = amount {
        crate::systems::litter::deposit_in_district(world, hole.district, a, r, hole.id);
    }
    // M15 W6: the crime goes into the hole's district's pool unnamed.
    crate::systems::gossip::post_hole(world, &hole);
    world.holes.insert(id, hole);
    let list = world.holes_by_agent.entry(victim).or_default();
    if let Err(i) = list.binary_search(&id) {
        list.insert(i, id);
    }
    world.stats.current.holes_opened += 1;
    let cap = world.config.bind.max_open_per_agent.max(1);
    while let Some(oldest) =
        world.holes_by_agent.get(&victim).filter(|l| l.len() > cap).and_then(|l| l.first().copied())
    {
        expire(world, oldest);
    }
    id
}

/// Take a hole out of both indices.
fn take(world: &mut World, id: HoleId) -> Option<Hole> {
    let hole = world.holes.remove(&id)?;
    if let Some(list) = world.holes_by_agent.get_mut(&hole.victim) {
        list.retain(|h| *h != id);
        if list.is_empty() {
            world.holes_by_agent.remove(&hole.victim);
        }
    }
    Some(hole)
}

/// The trace entry an agent has for `day`, or, for today (not yet written),
/// one built from live state.
fn trace_on(world: &World, id: EntityId, trace: &Trace, day: u64) -> Option<DayTrace> {
    trace.on_day(day).or_else(|| {
        (day == world.day() && crate::systems::law::living(world, id))
            .then(|| crate::systems::stats::live_day_trace(world, id))
    })
}

/// Who had a trace entry on a day, with it, ascending by id: one walk of
/// the Trace store per day and drain instead of one per bind (M10 review:
/// a promotion wave binds hundreds of holes in one tick). Past days only:
/// a day's trace is fixed once written, while today's is built from live
/// state that a bind can change (a witnessed actor's promotion), so today
/// is walked afresh every time.
#[derive(Default)]
struct DayPools {
    days: std::collections::BTreeMap<u64, Vec<(EntityId, DayTrace)>>,
}

impl DayPools {
    fn walk(world: &World, day: u64) -> Vec<(EntityId, DayTrace)> {
        world
            .with::<Trace>()
            .into_iter()
            .filter_map(|id| world.comp::<Trace>(id).and_then(|t| trace_on(world, id, t, day)).map(|t| (id, t)))
            .collect()
    }

    fn with_day<R>(&mut self, world: &World, day: u64, f: impl FnOnce(&[(EntityId, DayTrace)]) -> R) -> R {
        if day >= world.day() {
            return f(&Self::walk(world, day));
        }
        f(self.days.entry(day).or_insert_with(|| Self::walk(world, day)))
    }
}

/// Weighted candidates for a hole, ascending by id (pub for tests):
/// alive and free that day per their trace, an adult with a Personality now,
/// not the victim or the victim's spouse. Weight exactly
/// `(1 − l)^lawfulness_power × (1 + gang_claim_mult·g·c) × (1 + enemy_mult·e) × (1 + statistical_mult·s)`,
/// times (M12 D3) 1 in the hole's district, `same_zone_weight` elsewhere in
/// its zone, `other_zone_weight` off the zone. A trace or hole from before
/// M12 with no district reads its zone only (`same_zone_weight` 1.0 is M11).
pub fn candidates(world: &World, hole: &Hole) -> Vec<(EntityId, f64)> {
    DayPools::default().with_day(world, time::day(hole.tick), |pool| candidates_in(world, hole, pool))
}

fn candidates_in(world: &World, hole: &Hole, pool: &[(EntityId, DayTrace)]) -> Vec<(EntityId, f64)> {
    let cfg = &world.config.bind;
    let claim_gang = hole.home.and_then(|h| world.comp::<crate::components::Building>(h)).and_then(|b| b.claim);
    let claim_gang = claim_gang.map(|c| c.gang);
    let mut out = Vec::new();
    for &(id, t) in pool {
        if id == hole.victim || Some(id) == hole.spouse {
            continue;
        }
        // L2 (plan L27): a faction hole binds only to that faction's
        // members (the episode agent for an episode's), a riot hole only to
        // that riot's rioters; an empty pool binds Unknown.
        if hole.faction.is_some_and(|f| !crate::systems::grudges::member_of(world, id, f)) {
            continue;
        }
        if hole.source == Some(crate::ledger::ViolenceSource::Riot)
            && !crate::systems::fviolence::rioter_in(world, id, hole.riot)
        {
            continue;
        }
        if !t.has(trace_flags::ALIVE) || t.has(trace_flags::JAILED) {
            continue;
        }
        let Some(p) = world.comp::<Personality>(id) else { continue };
        let l = f64::from(p.lawfulness);
        let g = t.has(trace_flags::GANG);
        // M13 D37: only a gang member abducts for parts.
        if hole.kind == HoleKind::Abducted && !g {
            continue;
        }
        let c = claim_gang.is_some() && claim_gang == world.gang_of(id);
        let e = world.enemies.get(&hole.victim).is_some_and(|s| s.contains(&id));
        let s = t.has(trace_flags::STATISTICAL_ALL_DAY);
        let unlawful = 1.0 - l;
        // Squared exactly at the default, so it matches the old `powi(2)` bit for bit.
        let lawless =
            if cfg.lawfulness_power == 2.0 { unlawful * unlawful } else { unlawful.powf(cfg.lawfulness_power) };
        let mut w = lawless
            * (1.0 + cfg.gang_claim_mult * f64::from(u8::from(g && c)))
            * (1.0 + cfg.enemy_mult * f64::from(u8::from(e)))
            * (1.0 + cfg.statistical_mult * f64::from(u8::from(s)));
        if t.zone != hole.zone {
            w *= cfg.other_zone_weight;
        } else if t.district != hole.district || hole.district.is_unset() {
            w *= cfg.same_zone_weight;
        }
        // M13 D37: the chromed are the likelier culprits of a beating or a
        // killing (a branch: a bare candidate's weight is untouched).
        if matches!(hole.kind, HoleKind::Assaulted | HoleKind::Killed) {
            if let Some(m) = crate::systems::chrome::bind_weight(world, id) {
                w *= m;
            }
        }
        // M15 W34: the feared are blamed (a branch: no word, no term).
        if world.config.gossip.enabled {
            let dread = crate::systems::reputation::rep(world, id).dread;
            if dread > 0.0 {
                w *= 1.0 + f64::from(world.config.reputation.bind_dread_w * dread);
            }
        }
        if w > 0.0 {
            out.push((id, w));
        }
    }
    out
}

/// Guard presence in a zone yesterday, normalised by Homes (M10 D33):
/// `(hours_z / homes_z) / (hours_all / homes_all)` clamped to
/// `coverage_min..=coverage_max`; a zone without Homes reads the max, no
/// guard hours anywhere reads 1. M12 D3: the hours are the district watch
/// summed over the zone's districts (`zone_watch` is no longer written), so
/// the M11 binder (`[bind] district_coverage = false`) reads the same number.
pub fn zone_law_coverage(world: &World, zone: Zone) -> f32 {
    let cfg = &world.config.bind;
    let mut hours = [0u32; 5];
    for (d, &h) in world.districts.iter().zip(world.district_watch.yesterday.iter()) {
        hours[d.zone.index()] += h;
    }
    let hours_all: u32 = hours.iter().sum();
    if hours_all == 0 {
        return 1.0;
    }
    let mut homes = [0u32; 5];
    for &h in world.buildings_of_kind(BuildingKind::Home) {
        if let Some(b) = world.comp::<crate::components::Building>(h).filter(|b| !b.demolished) {
            homes[world.map.zone(b.door).index()] += 1;
        }
    }
    let homes_all: u32 = homes.iter().sum();
    let (hz, nz) = (hours[zone.index()], homes[zone.index()]);
    if nz == 0 || homes_all == 0 {
        return cfg.coverage_max;
    }
    let cov = (hz as f32 / nz as f32) / (hours_all as f32 / homes_all as f32);
    cov.clamp(cfg.coverage_min, cfg.coverage_max)
}

/// M12 D3: guard presence in a district yesterday, the `zone_law_coverage`
/// formula over the district's standing Homes and `district_watch`:
/// `(hours_d / homes_d) / (hours_all / homes_all)` clamped to
/// `coverage_min..=coverage_max`; a district without Homes reads the max,
/// no guard hours anywhere reads 1. Cached daily in `District::coverage`;
/// phase 1 has no reader but the aggregates.
pub fn district_coverage(world: &World, d: crate::components::DistrictId) -> f32 {
    let cfg = &world.config.bind;
    let hours = &world.district_watch.yesterday;
    let hours_all: u32 = hours.iter().sum();
    if hours_all == 0 {
        return 1.0;
    }
    let homes_all: usize = world.districts.iter().map(|x| x.homes.len()).sum();
    let nd = world.districts.get(d.index()).map_or(0, |x| x.homes.len());
    let hd = hours.get(d.index()).copied().unwrap_or(0);
    if nd == 0 || homes_all == 0 {
        return cfg.coverage_max;
    }
    let cov = (hd as f32 / nd as f32) / (hours_all as f32 / homes_all as f32);
    cov.clamp(cfg.coverage_min, cfg.coverage_max)
}

/// Attribute a hole. Deterministic in `(world seed, hole id)` and the traces.
/// `None` if no such hole is open.
pub fn bind(world: &mut World, id: HoleId) -> Option<Bound> {
    bind_in(world, id, &mut DayPools::default())
}

/// `bind` drawing its candidates and witnesses from `pools`.
fn bind_in(world: &mut World, id: HoleId, pools: &mut DayPools) -> Option<Bound> {
    // 1. Out of the indices.
    let hole = take(world, id)?;
    // 2. The hole's own stream.
    let mut rng = world.rng.hole(id);
    let cfg = world.config.bind.clone();
    // 3. Unknown first (D11), else a weighted draw over the candidates.
    let day = time::day(hole.tick);
    let mut bound = Bound::Unknown;
    // M16a (plan C19): a contract record's hole is pre-bound to its
    // `faction` (the taker, or the target for a taker's death): the Unknown
    // and candidate draws are made and discarded, so the witness roll reads
    // the offset an ordinary hole binding one actor reads.
    let contract = matches!(hole.source, Some(crate::ledger::ViolenceSource::Contract(_)));
    let unknown = if contract {
        let _ = rng.random::<f64>();
        let _ = rng.random::<f64>();
        if let Some(f) = hole.faction.filter(|&f| prebound_ok(world, f, day)) {
            bound = Bound::Actor(f);
        }
        true
    } else {
        rng.random::<f64>() < cfg.p_unknown
    };
    if !unknown {
        let cands = pools.with_day(world, day, |pool| candidates_in(world, &hole, pool));
        let total: f64 = cands.iter().map(|&(_, w)| w).sum();
        if total > 0.0 {
            let u = rng.random::<f64>() * total;
            let mut acc = 0.0;
            let mut pick = cands.last().map(|&(c, _)| c);
            for &(c, w) in &cands {
                acc += w;
                if u < acc {
                    pick = Some(c);
                    break;
                }
            }
            if let Some(c) = pick {
                bound = Bound::Actor(c);
            }
        }
    }
    // 4. The witness roll is always drawn (the spec's "bound witness decision").
    // M12 D3: the district's cached coverage; the pool is the district's.
    let by_district = cfg.district_coverage && !hole.district.is_unset();
    let coverage =
        if by_district { world.district(hole.district).coverage } else { zone_law_coverage(world, hole.zone) };
    // M12 D18: a dirty street sees less.
    let dirt =
        if by_district && crate::systems::litter::enabled(world) { world.district(hole.district).litter } else { 0.0 };
    let mut p = cfg.p_witness * coverage * (1.0 - 0.3 * dirt);
    // M15 W28: × the law's competence multiplier (1 when off).
    let m = crate::systems::competence::law_mult(world);
    if m != 1.0 {
        p *= m;
    }
    let witnessed = rng.random::<f32>() < p;
    let mut witness = None;
    if let (true, Bound::Actor(actor)) = (witnessed, bound) {
        // M15 W24: with street silence the pool skips who would keep quiet.
        let silence = world.config.gossip.enabled && world.config.hunt.street_silence;
        let pool: Vec<EntityId> = pools.with_day(world, day, |pool| {
            pool.iter()
                .filter(|&&(w, t)| w != actor && w != hole.victim && t.has(trace_flags::ALIVE))
                .filter(|&&(_, t)| if by_district { t.district == hole.district } else { t.zone == hole.zone })
                .map(|&(w, _)| w)
                .filter(|&w| crate::systems::law::living(world, w) && crate::systems::demography::is_adult(world, w))
                .filter(|&w| !silence || !crate::systems::law::silent_witness(world, w, actor, Some(hole.victim)))
                .collect()
        });
        if !pool.is_empty() {
            witness = Some(pool[rng.random_range(0..pool.len())]);
        }
    }
    let victim = hole.victim;
    if let Bound::Actor(actor) = bound {
        // 5. The actor's side, and the victim's memories get their subject.
        match hole.kind {
            HoleKind::Assaulted | HoleKind::Killed => {
                world.remember(actor, MemoryKind::Fought, Some(victim), 0.7, -0.5, false);
                world.remember(actor, MemoryKind::Won, Some(victim), 0.6, 0.4, false);
            }
            // M13 D37: the abductee's coins go to the actor as a robbery's do.
            HoleKind::Robbed | HoleKind::Abducted => {
                if let Some(w) = world.comp_mut::<Wallet>(actor) {
                    w.coins += hole.loot;
                }
            }
        }
        let kinds: &[MemoryKind] = match hole.kind {
            HoleKind::Robbed => &[MemoryKind::WasRobbed],
            HoleKind::Assaulted | HoleKind::Killed => &[MemoryKind::Fought, MemoryKind::Lost],
            HoleKind::Abducted => &[],
        };
        let mut named: smallvec::SmallVec<[crate::word::DeedRef; 2]> = smallvec::SmallVec::new();
        if let Some(m) = world.comp_mut::<crate::components::Memory>(victim) {
            for e in m.entries.iter_mut() {
                if kinds.contains(&e.kind) && e.tick == hole.tick && e.subject.is_none() {
                    e.subject = Some(actor);
                    if let Some(r) = crate::systems::memory::deed_of(victim, e) {
                        named.push(r);
                    }
                }
            }
        }
        // M15 W15 (plan deviation: the binder's rename is a call site too):
        // the victim now knows who did it.
        for r in named {
            crate::systems::grudges::on_learn(world, victim, &r, 1.0, 0);
        }
        // M15 W6: the hole's pool entries and held rumours take the name.
        crate::systems::gossip::name_hole(world, &hole, actor);
        // Only a living victim gets the Enemy edge: a corpse's edges are
        // dropped when it is freed, and nobody should come to hate the dead.
        if crate::systems::law::living(world, victim) {
            crate::systems::social::robbed_by(world, victim, actor);
        }
        // 6. A witness reports it: a cold case on the normal arrest path.
        if let Some(w) = witness {
            let crime = hole.kind.crime();
            world.remember_crime(w, actor, crime, crate::systems::law::crime_salience(crime), Some(victim));
            crate::systems::law::file_report(world, crime, actor, Some(w));
            if world.comp::<crate::components::Brain>(actor).is_some_and(|b| b.lod == Lod::Statistical) {
                crate::systems::lod::set_lod(world, actor, Lod::Coarse);
            }
        }
        // 7. Consequences (D31).
        // The death itself was shocked at `kill_by`, with no killer named
        // (`by_rival: false`): naming a rival only upgrades it.
        if hole.kind == HoleKind::Killed {
            if let (Some(gang), Some(theirs)) = (hole.gang, world.gang_of(actor)) {
                if theirs != gang {
                    crate::systems::gang::upgrade_kill_shock(world, gang);
                }
            }
        }
        if hole.kind == HoleKind::Assaulted && crate::systems::law::is_guard(world, victim) {
            crate::systems::law_brain::push_shock(world, LawShock::GuardBeaten);
        }
    }
    // M13 D37: the limbo chrome goes to the actor's gang, or is destroyed.
    if hole.kind == HoleKind::Abducted {
        let actor = match bound {
            Bound::Actor(a) => Some(a),
            Bound::Unknown => None,
        };
        crate::systems::chrome::settle_limbo(world, hole.id, actor);
    }
    // 8. The ring entry names the actor (or says nobody ever will).
    rewrite_event(world, &hole, bound);
    crate::events::life_bound(world, &hole, bound);
    // 9. Counters, 10. the Attributed event.
    match bound {
        Bound::Actor(_) => world.stats.current.holes_bound += 1,
        Bound::Unknown => world.stats.current.holes_unknown += 1,
    }
    // M16a (plan C19): a contract hole's binding is its own count (the
    // probe `contract_hole_wrong`: bound to anyone but its faction, 0).
    if contract {
        let c = &mut world.stats.current.contract;
        c.contract_holes += 1;
        c.contract_hole_wrong += u32::from(matches!(bound, Bound::Actor(a) if Some(a) != hole.faction));
    }
    // L2 (L27): a faction hole's binding; an actor outside its faction or
    // riot is `fv_bound_wrong` (the gate asserts 0).
    if hole.source.is_some() && !contract {
        let wrong = match bound {
            Bound::Actor(a) => {
                hole.faction.is_some_and(|f| !crate::systems::grudges::member_of(world, a, f))
                    || (hole.source == Some(crate::ledger::ViolenceSource::Riot)
                        && !crate::systems::fviolence::rioter_in(world, a, hole.riot))
            }
            Bound::Unknown => false,
        };
        let l = &mut world.stats.current.living;
        match bound {
            Bound::Actor(_) => l.fv_bound += 1,
            Bound::Unknown => l.fv_unknown += 1,
        }
        l.fv_bound_wrong += u32::from(wrong);
    }
    attributed_event(world, &hole, bound, witness);
    Some(bound)
}

/// M16a (plan C19): a pre-bound actor binds while living and not jailed
/// on the hole's day (else the hole is Unknown).
fn prebound_ok(world: &World, f: EntityId, day: u64) -> bool {
    if !crate::systems::law::living(world, f) {
        return false;
    }
    let jailed = world
        .comp::<crate::components::Trace>(f)
        .and_then(|t| t.on_day(day))
        .is_some_and(|t| t.has(trace_flags::JAILED));
    !jailed
}

fn rewrite_event(world: &mut World, hole: &Hole, bound: Bound) {
    let actor_name = match bound {
        Bound::Actor(a) => Some(world.name_of(a)),
        Bound::Unknown => None,
    };
    if let Some(e) = world.event_mut(hole.event_id) {
        match (bound, actor_name) {
            (Bound::Actor(a), Some(name)) => {
                if let Some(slot) = e.actors.first_mut() {
                    *slot = a;
                }
                e.text.push_str(&format!(" (laid to {name})"));
            }
            _ => e.text.push_str(" (never named)"),
        }
    }
}

fn attributed_event(world: &mut World, hole: &Hole, bound: Bound, witness: Option<EntityId>) {
    let day = time::day(hole.tick);
    let victim = world.name_of(hole.victim);
    let place = world.district_name(hole.district).to_string();
    let what = format!("the {place} {} of {victim} on day {day}", hole.kind.noun());
    match bound {
        Bound::Actor(a) => {
            let seen = witness.map(|w| format!(", seen by {}", world.name_of(w))).unwrap_or_default();
            // L2 (plan L35): a faction hole names the faction.
            let of = hole
                .faction
                .filter(|&f| f != a)
                .map(|f| {
                    let name = crate::systems::grudges::label(world, f);
                    match name.strip_prefix("The ") {
                        Some(rest) => format!(" (a {rest} member)"),
                        None => format!(" (a {name} member)"),
                    }
                })
                .unwrap_or_default();
            let text = format!("{what} is laid to {}{of}{seen}", world.name_of(a));
            world.push_event(EventKind::Attributed, &[a, hole.victim], text);
        }
        Bound::Unknown => {
            world.push_event(EventKind::Attributed, &[hole.victim], format!("{what} will never be solved"));
        }
    }
}

/// An agent leaving the world (`World::remove_agent`) takes its open victim
/// holes with it: each closes as Unknown and is counted, with no draw, no
/// event and no biography (both go with the agent).
pub fn drop_victim_holes(world: &mut World, victim: EntityId) {
    let ids = world.holes_by_agent.get(&victim).cloned().unwrap_or_default();
    for id in ids {
        if let Some(hole) = take(world, id) {
            world.stats.current.holes_unknown += 1;
            if hole.kind == HoleKind::Abducted {
                crate::systems::chrome::settle_limbo(world, id, None);
            }
        }
    }
    world.holes_by_agent.remove(&victim);
}

/// Close a hole as Unknown without a draw (the per-victim cap).
pub fn expire(world: &mut World, id: HoleId) {
    let Some(hole) = take(world, id) else { return };
    if hole.kind == HoleKind::Abducted {
        crate::systems::chrome::settle_limbo(world, id, None);
    }
    rewrite_event(world, &hole, Bound::Unknown);
    crate::events::life_bound(world, &hole, Bound::Unknown);
    world.stats.current.holes_unknown += 1;
    attributed_event(world, &hole, Bound::Unknown, None);
}

/// Daily pass at `tick_of_day == 0`: every consequential hole from before
/// today, then every hole past `hole_ttl_days`, each in ascending id order.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    // L2 (plan L26): faction violence off screen; its holes open now and
    // bind at the next midnight, as the hourly table's.
    crate::systems::fviolence::daily(world);
    let now = world.tick;
    let day_start = time::day(now) * TICKS_PER_DAY;
    let due: Vec<HoleId> =
        world.holes.values().filter(|h| h.consequential && h.tick < day_start).map(|h| h.id).collect();
    let mut pools = DayPools::default();
    for id in due {
        bind_in(world, id, &mut pools);
    }
    let ttl = world.config.bind.hole_ttl_days * TICKS_PER_DAY;
    let old: Vec<HoleId> = world.holes.values().filter(|h| h.tick + ttl <= now).map(|h| h.id).collect();
    for id in old {
        bind_in(world, id, &mut pools);
    }
}

/// Promotion binds queued by `lod::set_lod`; drained at the end of
/// `lod::run`, oldest first. A bind may promote a witnessed actor and queue
/// more; the loop runs until the queue is empty.
pub fn drain_queue(world: &mut World) {
    let mut pools = DayPools::default();
    while !world.bind_queue.is_empty() {
        let mut ids = std::mem::take(&mut world.bind_queue);
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            bind_in(world, id, &mut pools);
        }
    }
}

/// Bind every open hole, oldest first (tests, end-of-run parity).
pub fn bind_all(world: &mut World) {
    let ids: Vec<HoleId> = world.holes.keys().copied().collect();
    let mut pools = DayPools::default();
    for id in ids {
        bind_in(world, id, &mut pools);
    }
}
