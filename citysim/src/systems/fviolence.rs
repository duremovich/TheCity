//! L2 (plan L23-L25, "Ledger maths"): faction violence's ledger. Phase 3
//! builds the actor side: the on-screen members' acts (`note_act`) and
//! member-days (`daily_actor`) per `(Order(o), district, Member)` cell, and
//! the Statistical GangWork day that reads them as per-member-day rates
//! (`stat_gang_day`). Phase 4 adds the victim side: on-screen victims and
//! body exposure per `(source, district, victim class)` (`note_victim`,
//! `tally_exposure`), the sources touching each district (`rebuild_active`,
//! `active_now`), and the midnight pass (`daily`) that rolls every
//! Statistical adult of a touched district once against the summed rates.
//!
//! What this is: counters and seeded dice rolls on keyed streams
//! (`WordNs::GangHour`, `WordNs::FViolence`); "extortion", "claim", "deal",
//! "killed" and "beaten" are game actions and hole records between
//! fictional agents of a simulated city.

use rand::Rng;

use crate::components::{Brain, DistrictId, HoleKind, Household, Lod, Order, Position, Sentence};
use crate::entity::EntityId;
use crate::ledger::{ActKind, ActiveSource, VictimClass, ViolenceSource, RATE_DAYS};
use crate::world::World;

/// The district a member's acts and member-days are filed under: its
/// Home's, else its gang's Hideout's.
pub fn member_district(world: &World, id: EntityId) -> Option<DistrictId> {
    if let Some(h) = world.comp::<Household>(id).and_then(|h| h.home) {
        return Some(world.district_of_building(h));
    }
    world.gang_of(id).and_then(|g| world.hideout_of(g)).map(|h| world.district_of_building(h))
}

/// L25 (actor side): a body member following its gang's order did one of
/// the order's acts (an extortion, a claim blow, a deal shift): counted in
/// `(Order(o), its district, Member)`. Statistical acts are not counted
/// (the ledger is what bodies do).
pub fn note_act(world: &mut World, actor: EntityId, kind: ActKind) {
    if world.comp::<Brain>(actor).is_none_or(|b| b.lod == Lod::Statistical) {
        return;
    }
    let Some(o) = crate::systems::gang::following_order(world, actor) else { return };
    let Some(d) = member_district(world, actor) else { return };
    let cell = world.order_rates.cell_mut((ViolenceSource::Order(o), d, VictimClass::Member));
    let a = &mut cell.acts_today[kind as usize];
    *a = a.saturating_add(1);
}

/// Midnight, before the hour's LOD assignment: one member-day for every
/// free member that held a body at any assignment yesterday and follows
/// its gang's order (under `[lod] budget`), then every cell's windows roll
/// and empty cells go (under the budget or `[fviolence]`).
pub fn daily_actor(world: &mut World) {
    let yesterday = world.day().saturating_sub(1);
    let mut days: Vec<(ViolenceSource, DistrictId)> = Vec::new();
    // Phase 4: with the budget off the windows still roll for the victim side.
    let gangs = world.gangs();
    for gang in gangs {
        let members = world.comp::<crate::components::Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
        for m in members {
            if world.has::<Sentence>(m) || world.comp::<Brain>(m).is_none_or(|b| b.body_day != Some(yesterday)) {
                continue;
            }
            let Some(o) = crate::systems::gang::following_order(world, m) else { continue };
            let Some(d) = member_district(world, m) else { continue };
            days.push((ViolenceSource::Order(o), d));
        }
    }
    for (s, d) in days {
        let cell = world.order_rates.cell_mut((s, d, VictimClass::Member));
        cell.member_today = cell.member_today.saturating_add(1);
    }
    for cell in world.order_rates.cells.values_mut() {
        cell.roll(RATE_DAYS);
    }
    world.order_rates.cells.retain(|_, c| !c.is_empty());
    hands_daily_actor(world, yesterday);
}

// ---------------------------------------------------------------------------
// The island pass phase 10 (OPEN-185): the off-screen hands
// ---------------------------------------------------------------------------

/// The hands' keyed draws (`SimRng::keyed`: below 2^60, apart from the
/// other `*_KEY` prefixes).
pub const HANDS_KEY: u64 = 0xF4A0 << 44;

/// `[fviolence.hands]` is on (the key present).
pub fn hands_on(world: &World) -> bool {
    world.config.fviolence.hands.is_some()
}

/// The order a gang member's acts count under (`source_of`'s): the order
/// he follows, else his gang's raid order.
fn hand_order(world: &World, id: EntityId) -> Option<Order> {
    crate::systems::gang::following_order(world, id).or_else(|| {
        world
            .gang_of(id)
            .and_then(|g| world.comp::<crate::components::Gang>(g))
            .map(|g| g.order)
            .filter(|o| o.is_raid())
    })
}

/// The hands ledger's source: Job's cell is Expand's (`rebuild_active`).
fn hand_source(s: ViolenceSource) -> ViolenceSource {
    match s {
        ViolenceSource::Order(Order::Job) => ViolenceSource::Order(Order::Expand),
        s => s,
    }
}

/// A free off-screen hand at the midnight pass: Statistical now and at no
/// assignment yesterday (so nowhere on screen the day the deed is laid to),
/// living, adult, and not jailed, emigrating or pinned.
pub fn free_hand(world: &World, id: EntityId) -> bool {
    let yesterday = world.day().saturating_sub(1);
    let Some(b) = world.comp::<Brain>(id) else { return false };
    b.lod == Lod::Statistical
        && !b.emigrating
        && !b.pinned
        && b.body_day.is_none_or(|d| d < yesterday)
        && world.is_alive(id)
        && !world.has::<Sentence>(id)
        && crate::systems::demography::is_adult(world, id)
}

/// Midnight (from `daily_actor`): yesterday's member-days per `(source,
/// faction)`: every free gang member that held a body yesterday under its
/// order (`hand_order`), and every member of a faction in an open vendetta
/// that did; then the windows roll and empty cells go.
fn hands_daily_actor(world: &mut World, yesterday: u64) {
    if !hands_on(world) {
        return;
    }
    let bodied = |w: &World, m: EntityId| {
        !w.has::<Sentence>(m) && w.comp::<Brain>(m).is_some_and(|b| b.body_day == Some(yesterday))
    };
    let mut days: Vec<crate::ledger::HandsKey> = Vec::new();
    for gang in world.gangs() {
        let members = world.comp::<crate::components::Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
        for m in members {
            if !bodied(world, m) {
                continue;
            }
            if let Some(o) = hand_order(world, m) {
                days.push((hand_source(ViolenceSource::Order(o)), gang));
            }
        }
    }
    let feuding: std::collections::BTreeSet<EntityId> = world.vendettas.iter().flat_map(|v| [v.a, v.b]).collect();
    if !feuding.is_empty() {
        for lod in [Lod::Full, Lod::Coarse, Lod::Statistical] {
            for &m in world.tier(lod) {
                if !bodied(world, m) {
                    continue;
                }
                for &f in &feuding {
                    if crate::systems::grudges::member_of(world, m, f) {
                        days.push((ViolenceSource::Vendetta, f));
                    }
                }
            }
        }
    }
    for key in days {
        let c = world.order_rates.hands_mut(key);
        c.member_today = c.member_today.saturating_add(1);
    }
    for c in world.order_rates.hands.values_mut() {
        c.roll(RATE_DAYS);
    }
    world.order_rates.hands.retain(|_, c| !c.is_empty());
}

/// The actor side's rate per member-day of `(source, faction)`, Killed,
/// Assaulted, Robbed, Abducted: `(Σ hits[k] + prior[k] × prior_days) ÷
/// (Σ member_days + prior_days)` over the last `rate_days` days (the
/// victim side's `cell_rates` shape, `act_rate`'s maths). Zeros when off.
pub fn hand_rate(world: &World, key: crate::ledger::HandsKey) -> [f32; 4] {
    let Some(h) = world.config.fviolence.hands.as_ref() else { return [0.0; 4] };
    let days = world.config.fviolence.rate_days;
    let pd = h.prior_days.max(0.0);
    let cell = world.order_rates.hands.get(&key);
    std::array::from_fn(|k| {
        let (hits, members) = cell.map_or((0, 0), |c| c.sums(k, days));
        let denom = members as f32 + pd;
        if denom <= 0.0 {
            return 0.0;
        }
        ((hits as f32 + h.prior[k] * pd) / denom).clamp(0.0, 1.0)
    })
}

/// A source's hands: an order's or a vendetta side's faction, a riot.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PoolKey {
    Faction(ViolenceSource, EntityId),
    Riot(u32),
}

/// The pool of an active source (an episode's is the agent's own).
pub fn pool_key(s: &ActiveSource) -> Option<PoolKey> {
    match s.source {
        ViolenceSource::Riot => s.riot.map(PoolKey::Riot),
        src => s.faction.map(|f| PoolKey::Faction(hand_source(src), f)),
    }
}

/// One source's off-screen hands today: the free Statistical members (in
/// ascending id, spent ones dropped as they strike) and the day's budget
/// of hits per kind.
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub hands: Vec<EntityId>,
    pub budget: [u32; 4],
}

fn key_hash(k: &PoolKey) -> u64 {
    let (tag, a, b) = match *k {
        PoolKey::Faction(ViolenceSource::Order(o), f) => (1u64, o as u64, u64::from(f.index)),
        PoolKey::Faction(ViolenceSource::Vendetta, f) => (2, 0, u64::from(f.index)),
        PoolKey::Faction(ViolenceSource::Episode, f) => (3, 0, u64::from(f.index)),
        PoolKey::Faction(_, f) => (4, 0, u64::from(f.index)),
        PoolKey::Riot(r) => (5, 0, u64::from(r)),
    };
    (tag << 56) ^ (a << 40) ^ b
}

/// A keyed draw of the hands pass: `purpose` (1 a budget's rounding, 2 a
/// hand's pick) on `(day, a, b)`.
fn hands_draw(world: &World, purpose: u64, a: u64, b: u64) -> rand_chacha::ChaCha8Rng {
    let h = crate::rng::splitmix64(
        (purpose << 58) ^ world.day().rotate_left(44) ^ crate::rng::splitmix64(a) ^ b.rotate_left(20),
    );
    world.rng.keyed(HANDS_KEY | (h & ((1 << 44) - 1)))
}

/// The midnight pass's pools for the sources `all`: an order's hands are
/// its gang's free members under it, a vendetta side's its faction's free
/// members, a riot's its free rioters (live, or the roster the tally kept),
/// an episode's the agent while free. An order's or a vendetta's budget is
/// `n × hand_rate` per kind, rounded on a keyed draw (unbiased), at most
/// `n`; a riot's and an episode's is `n` (each hand strikes at most once).
pub fn build_pools(world: &World, all: &[ActiveSource]) -> std::collections::BTreeMap<PoolKey, Pool> {
    use std::collections::BTreeMap;
    let mut pools: BTreeMap<PoolKey, Pool> = BTreeMap::new();
    for s in all {
        if let Some(k) = pool_key(s) {
            pools.entry(k).or_default();
        }
    }
    let feuding: Vec<EntityId> = pools
        .keys()
        .filter_map(|k| match *k {
            PoolKey::Faction(ViolenceSource::Vendetta, f) => Some(f),
            _ => None,
        })
        .collect();
    let mut feud_hands: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    if !feuding.is_empty() {
        let mut ids: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
        ids.sort_unstable();
        for id in ids {
            if !free_hand(world, id) {
                continue;
            }
            for &f in &feuding {
                if crate::systems::grudges::member_of(world, id, f) {
                    feud_hands.entry(f).or_default().push(id);
                }
            }
        }
    }
    let keys: Vec<PoolKey> = pools.keys().copied().collect();
    for k in keys {
        let mut hands: Vec<EntityId> = match k {
            PoolKey::Faction(ViolenceSource::Order(o), g) => world
                .comp::<crate::components::Gang>(g)
                .map(|x| x.members.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|&m| {
                    free_hand(world, m)
                        && hand_order(world, m).map(|x| hand_source(ViolenceSource::Order(x)))
                            == Some(ViolenceSource::Order(o))
                })
                .collect(),
            PoolKey::Faction(ViolenceSource::Vendetta, f) => feud_hands.get(&f).cloned().unwrap_or_default(),
            PoolKey::Faction(ViolenceSource::Episode, a) => {
                if free_hand(world, a) {
                    vec![a]
                } else {
                    Vec::new()
                }
            }
            PoolKey::Faction(_, _) => Vec::new(),
            PoolKey::Riot(r) => {
                let live = world.riots.iter().find(|x| x.id == r).map(|x| x.rioters.clone());
                let roster = || world.fv_tally.riot_rosters.get(&r).map(|(_, m)| m.clone()).unwrap_or_default();
                live.unwrap_or_else(roster).into_iter().filter(|&m| free_hand(world, m)).collect()
            }
        };
        hands.sort_unstable();
        hands.dedup();
        let n = hands.len() as u32;
        let budget = match k {
            PoolKey::Faction(src @ (ViolenceSource::Order(_) | ViolenceSource::Vendetta), f) if n > 0 => {
                let rate = hand_rate(world, (src, f));
                let mut rng = hands_draw(world, 1, key_hash(&k), 0);
                std::array::from_fn(|kind| {
                    let x = f64::from(n) * f64::from(rate[kind]);
                    let u: f64 = rng.random();
                    let whole = x.floor();
                    ((whole as u32) + u32::from(u < x - whole)).min(n)
                })
            }
            _ => [n; 4],
        };
        if let Some(p) = pools.get_mut(&k) {
            *p = Pool { hands, budget };
        }
    }
    pools
}

/// The pass's hit by source `s` of kind `kind` on `victim`: a hand from its
/// pool not spent today (a keyed pick), its pool's budget for the kind
/// taken. `None`: the source has no hand or no budget left (the hit is not
/// made).
fn take_hand(
    world: &World,
    pools: &mut std::collections::BTreeMap<PoolKey, Pool>,
    spent: &std::collections::BTreeSet<EntityId>,
    s: &ActiveSource,
    kind: usize,
    victim: EntityId,
) -> Option<EntityId> {
    let k = pool_key(s)?;
    let p = pools.get_mut(&k)?;
    p.hands.retain(|h| !spent.contains(h));
    if p.budget[kind] == 0 || p.hands.is_empty() {
        return None;
    }
    let i = hands_draw(world, 2, key_hash(&k), u64::from(victim.index)).random_range(0..p.hands.len());
    p.budget[kind] -= 1;
    Some(p.hands.remove(i))
}

/// Has source `s` a hand and a budget left for any kind (a source with
/// none is not rolled).
fn has_hands(
    pools: &std::collections::BTreeMap<PoolKey, Pool>,
    spent: &std::collections::BTreeSet<EntityId>,
    s: &ActiveSource,
) -> bool {
    pool_key(s)
        .and_then(|k| pools.get(&k))
        .is_some_and(|p| p.budget.iter().any(|&b| b > 0) && p.hands.iter().any(|h| !spent.contains(h)))
}

/// The prior of an act per member-day (`[gangs] stat_*_prior`).
fn prior(world: &World, kind: ActKind) -> f32 {
    let g = &world.config.gangs;
    match kind {
        ActKind::Extort => g.stat_extort_prior,
        ActKind::Claim => g.stat_claim_prior,
        ActKind::Deal => g.stat_deal_prior,
    }
}

/// Ledger maths, actor side: `p = (Σ acts + prior × prior_days) ÷
/// (Σ member_days + prior_days)` per member-day over the cell's window.
pub fn act_rate(world: &World, o: Order, d: DistrictId, kind: ActKind) -> f32 {
    let days = world.config.gangs.stat_prior_days.max(0.0);
    let p0 = prior(world, kind);
    let (acts, members) = world
        .order_rates
        .cells
        .get(&(ViolenceSource::Order(o), d, VictimClass::Member))
        .map_or((0, 0), |c| (c.acts_sum(kind), c.member_days_sum()));
    let denom = members as f32 + days;
    if denom <= 0.0 {
        return p0;
    }
    ((acts as f32 + p0 * days) / denom).clamp(0.0, 1.0)
}

/// The act a Statistical member's GangWork day would be: a deal shift for
/// a dealer whose gang holds a batch, a claim blow under Squat, an
/// extortion under Expand, Contest and VirtRaid; none otherwise.
fn day_act(world: &World, id: EntityId, o: Order) -> Option<ActKind> {
    let stims = crate::systems::stims::on(world);
    if stims && crate::systems::stims::is_dealer(world, id) {
        let stock = world
            .gang_of(id)
            .and_then(|g| world.hideout_of(g))
            .map_or(0, |h| world.stock(h, crate::components::Good::Stims));
        if stock >= world.config.stims.deal_batch {
            return Some(ActKind::Deal);
        }
    }
    match o {
        Order::Squat => Some(ActKind::Claim),
        Order::Expand | Order::Contest | Order::VirtRaid => Some(ActKind::Extort),
        _ => None,
    }
}

/// L23: the Statistical GangWork day, from `lod::stat_work`'s jobless
/// branch in the Work phase. A free Statistical member following its
/// gang's order, its task not done today: one draw on `WordNs::GangHour`
/// keyed `(day, id.index)` (the same draw every hour of the day, so one
/// trial a day) against its cell's rate; a hit acts at once (no walk, no
/// witness: a Statistical agent has no eyes) and, on the same stream at
/// the table's `p_theft_caught`, an extortion or a deal is reported and the
/// member gets a body for the arrest path, as `stat_theft`. A hit with no
/// target today waits for the next hour.
pub fn stat_gang_day(world: &mut World, id: EntityId) {
    use crate::components::Crime;
    if !world.has::<crate::components::GangMember>(id) || world.has::<Sentence>(id) {
        return;
    }
    let today = world.day();
    if world.comp::<Brain>(id).is_none_or(|b| b.gang_task_day == Some(today) || b.lod != Lod::Statistical) {
        return;
    }
    let Some(gang) = world.gang_of(id) else { return };
    let Some(o) = crate::systems::gang::following_order(world, id) else { return };
    let Some(kind) = day_act(world, id, o) else { return };
    let Some(d) = member_district(world, id) else { return };
    let p = act_rate(world, o, d, kind);
    let mut rng = world.rng.word(crate::word::WordNs::GangHour, today, u64::from(id.index));
    let u: f32 = rng.random();
    if u >= p {
        return;
    }
    let caught_u: f64 = rng.random();
    let crime = match kind {
        ActKind::Deal => {
            // The shift's doses sell through the Statistical buyers' pass
            // (`stims::stat_addicts` reads `deal_log`; Lives at work phase
            // 4b, a corner's in `corner_log`).
            match crate::systems::stims::deal_site(world, gang) {
                Some(crate::systems::stims::DealSite::Venue(bar)) => {
                    world.deal_log.insert(bar, (id, today));
                }
                Some(crate::systems::stims::DealSite::Corner(t)) => {
                    world.corner_log.insert(t, (id, today));
                }
                None => return,
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.gang_task_day = Some(today);
            }
            world.stats.current.budget.stat_deals += 1;
            Some(Crime::Dealing)
        }
        ActKind::Claim => {
            let Some((b, _)) = crate::systems::gang::gang_work_target(world, id) else { return };
            if !crate::systems::street::is_derelict(world, b) {
                return;
            }
            crate::systems::gang::squat_claim_with(world, id, b, false);
            world.stats.current.budget.stat_claims += 1;
            None
        }
        ActKind::Extort => {
            let Some(home) = crate::systems::gang::stat_extort_target(world, id, o) else { return };
            let taken = crate::systems::gang::stat_extort(world, id, home);
            world.stats.current.budget.stat_extorts += 1;
            // Pressure pass F2: a door that gave nothing is no Extortion to catch.
            (taken > 0).then_some(Crime::Extortion)
        }
    };
    let tick = world.tick;
    let Some(crime) = crime else {
        if let Some(log) = world.shadow_notes.as_mut() {
            log.push(crate::word::ShadowNote::StatGang { tick, id, act: kind, caught: false });
        }
        return;
    };
    let p_caught = world
        .stat_table
        .as_ref()
        .and_then(|t| t.p_theft_caught)
        .map_or(world.config.crime.stat_theft_caught_p, f64::from);
    if let Some(log) = world.shadow_notes.as_mut() {
        log.push(crate::word::ShadowNote::StatGang { tick, id, act: kind, caught: caught_u < p_caught });
    }
    if caught_u < p_caught {
        crate::systems::law::file_report(world, crime, id, None);
        crate::systems::lod::set_lod(world, id, Lod::Coarse);
    }
}

// ---------------------------------------------------------------------------
// Phase 4: the victim side (plan L24-L28, "Ledger maths"; spec § 3)
// ---------------------------------------------------------------------------

/// The ledger's index of a hole kind: Killed 0, Assaulted 1, Robbed 2, Abducted 3.
pub fn kind_index(kind: HoleKind) -> usize {
    match kind {
        HoleKind::Killed => 0,
        HoleKind::Assaulted => 1,
        HoleKind::Robbed => 2,
        HoleKind::Abducted => 3,
    }
}

const KINDS: [HoleKind; 4] = [HoleKind::Killed, HoleKind::Assaulted, HoleKind::Robbed, HoleKind::Abducted];

/// A victim's class: a guard (public or private) is the Watch, a gang
/// member a Member, anyone else a Civilian; with violence fix 1 a civilian
/// with no Home is Street (`lod` promotes the fresh evictees to bodies, so
/// what they suffer fighting the gangs over squats would otherwise be
/// learned as every housed civilian's rate).
pub fn victim_class(world: &World, id: EntityId) -> VictimClass {
    if crate::systems::law::is_guard(world, id) {
        VictimClass::Watch
    } else if world.has::<crate::components::GangMember>(id) {
        VictimClass::Member
    } else if crate::systems::fixes::street_class(world) && world.comp::<Household>(id).is_none_or(|h| h.home.is_none())
    {
        VictimClass::Street
    } else {
        VictimClass::Civilian
    }
}

/// A civilian for the parity tallies and the `kill_rate_*_civ` columns:
/// Civilian or (fix 1) Street.
fn is_civ(class: VictimClass) -> bool {
    matches!(class, VictimClass::Civilian | VictimClass::Street)
}

fn is_body(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
}

/// L25: what drove an on-screen act by `actor` against `victim`: `None` for
/// a Hunt (excluded on purpose: a Hunt keeps both sides on screen); the
/// gang's order when the actor's plan is `GangWork` or `Raid` under it;
/// an open vendetta between a faction of the actor's and one of the
/// victim's; a live riot the actor belongs to; an episode. With the acting
/// faction (the episode agent for an episode) and the riot.
pub fn source_of(
    world: &World,
    actor: EntityId,
    victim: EntityId,
) -> Option<(ViolenceSource, Option<EntityId>, Option<u32>)> {
    use crate::components::GoalKind;
    if world.hunts.contains_key(&actor) {
        return None;
    }
    // M16a (plan C19): a live contract job on the victim is not counted
    // (the Hunt precedent): the ledger never doubles it off screen.
    if crate::systems::contracts::live_job_on(world, actor, victim) {
        return None;
    }
    let goal = world.comp::<Brain>(actor)?.plan_goal();
    if matches!(goal, Some(GoalKind::GangWork | GoalKind::Raid)) {
        if let Some(g) = world.gang_of(actor) {
            let raid = world.comp::<crate::components::Gang>(g).map(|x| x.order).filter(|o| o.is_raid());
            if let Some(o) = crate::systems::gang::following_order(world, actor).or(raid) {
                return Some((ViolenceSource::Order(o), Some(g), None));
            }
        }
    }
    for v in &world.vendettas {
        for (mine, theirs) in [(v.a, v.b), (v.b, v.a)] {
            if crate::systems::grudges::member_of(world, actor, mine)
                && crate::systems::grudges::member_of(world, victim, theirs)
            {
                return Some((ViolenceSource::Vendetta, Some(mine), None));
            }
        }
    }
    if let Some(&r) = world.rioter_of.get(&actor) {
        return Some((ViolenceSource::Riot, None, Some(r)));
    }
    if world.episodes.contains(&actor) {
        return Some((ViolenceSource::Episode, Some(actor), None));
    }
    None
}

/// L25: an on-screen Assault, Theft with a victim, Abduction (from
/// `law::raise_crime_on`) or killing (from `World::kill_by`, before the
/// victim is unlinked). Counted in `(source, the victim's district, its
/// class)` when both sides hold bodies (exposure counts bodies only, L40),
/// the act has a source and the victim is not of the acting faction.
pub fn note_victim(world: &mut World, actor: EntityId, victim: EntityId, kind: HoleKind) {
    if actor == victim || !is_body(world, actor) || !is_body(world, victim) {
        return;
    }
    let Some((source, faction, _)) = source_of(world, actor, victim) else { return };
    if faction.is_some_and(|f| crate::systems::grudges::member_of(world, victim, f)) {
        return;
    }
    let Some(tile) = world.comp::<Position>(victim).map(|p| p.tile) else { return };
    let d = world.district_of(tile);
    let class = victim_class(world, victim);
    let k = kind_index(kind);
    // The island pass phase 10: the acting faction's on-screen hit, the
    // actor side of its hands' rate (orders and vendettas).
    if let (true, Some(f), ViolenceSource::Order(_) | ViolenceSource::Vendetta) = (hands_on(world), faction, source) {
        let c = world.order_rates.hands_mut((hand_source(source), f));
        c.hits_today[k] = c.hits_today[k].saturating_add(1);
    }
    let cell = world.order_rates.cell_mut((source, d, class));
    cell.victims_today[k] = cell.victims_today[k].saturating_add(1);
    if is_civ(class) && k <= 1 {
        world.fv_tally.body_civ_victims += 1;
    }
}

/// From `World::kill_by`, before anything is unlinked: a violent death's
/// tier tally (the `kill_rate_*` columns) and, with a killer, the ledger.
pub fn note_death(world: &mut World, victim: EntityId, cause: crate::components::DeathCause, killer: Option<EntityId>) {
    if cause != crate::components::DeathCause::Violence {
        return;
    }
    let body = is_body(world, victim);
    let civ = is_civ(victim_class(world, victim));
    let t = &mut world.fv_tally.day_kills;
    t[usize::from(!body)] += 1;
    if civ {
        t[2 + usize::from(!body)] += 1;
    }
    if let Some(k) = killer {
        note_victim(world, k, victim, HoleKind::Killed);
    }
}

/// The districts where a faction holds members' Homes: a gang's members, a
/// corp's exec and employees, the Law's city guards and captain.
fn home_districts(world: &World, f: EntityId) -> std::collections::BTreeSet<DistrictId> {
    let mut out = std::collections::BTreeSet::new();
    let mut add = |id: EntityId| {
        if let Some(h) = world.comp::<Household>(id).and_then(|h| h.home) {
            out.insert(world.district_of_building(h));
        }
    };
    if let Some(g) = world.comp::<crate::components::Gang>(f) {
        g.members.iter().for_each(|&m| add(m));
    } else if let Some(c) = world.comp::<crate::components::Corp>(f) {
        if let Some(e) = c.exec {
            add(e);
        }
        for role in world.roles() {
            for &w in world.workers(role) {
                if world
                    .comp::<crate::components::Job>(w)
                    .and_then(|j| j.employer)
                    .and_then(|e| world.corp_of_building(e))
                    == Some(f)
                {
                    add(w);
                }
            }
        }
    } else if world.has::<crate::components::Law>(f) {
        for &g in world.guards() {
            if crate::systems::law::is_city_guard(world, g) {
                add(g);
            }
        }
        if let Some(c) = world.law().and_then(|l| l.captain) {
            add(c);
        }
    }
    out
}

/// Spec "Where a source touches", the order and vendetta part (rebuilt at
/// 01:00, after the midnight rescore): a gang's order over its held
/// districts (Expand, Squat, Harvest, VirtRaid, LieLow), the rival's for
/// Contest, the door's district for a raid order while its muster is set;
/// a god `FactionStrike` (Contest on its district); each side of a
/// vendetta over every district where both sides hold members' Homes
/// (striking only the other side's members, `may_strike`).
pub fn rebuild_active(world: &mut World) {
    use crate::components::{Gang, Order};
    let now = world.tick;
    let mut out: Vec<ActiveSource> = Vec::new();
    let src = |source, district, faction| ActiveSource { source, district, faction, riot: None, episode: None };
    for g in world.gangs() {
        let Some((order, raid_at, strike)) = world.comp::<Gang>(g).map(|x| (x.order, x.raid_at, x.strike)) else {
            continue;
        };
        let districts: Vec<DistrictId> = match order {
            Order::Contest => world
                .rival_of(g)
                .map(|r| crate::systems::gang::held_districts(world, r).into_iter().map(|(d, _)| d).collect())
                .unwrap_or_default(),
            o if o.is_raid() => raid_at
                .and(crate::systems::raid::gang_target(world, g))
                .map(|b| vec![world.district_of_building(b)])
                .unwrap_or_default(),
            _ => crate::systems::gang::held_districts(world, g).into_iter().map(|(d, _)| d).collect(),
        };
        // M16a (plan C26, deviation): a gang under Job keeps Expand's cell
        // over its held districts (its members outside the crew work
        // Expand's GangWork; the job's own violence is the record's).
        let cell = if order == Order::Job { Order::Expand } else { order };
        for d in districts {
            out.push(src(ViolenceSource::Order(cell), d, Some(g)));
        }
        if let Some((d, _)) = strike.filter(|&(_, t)| t > now) {
            out.push(src(ViolenceSource::Order(Order::Contest), d, Some(g)));
        }
    }
    let pairs: Vec<(EntityId, EntityId)> = world.vendettas.iter().map(|v| (v.a, v.b)).collect();
    for (a, b) in pairs {
        let (da, db) = (home_districts(world, a), home_districts(world, b));
        for &d in da.intersection(&db) {
            out.push(src(ViolenceSource::Vendetta, d, Some(a)));
            out.push(src(ViolenceSource::Vendetta, d, Some(b)));
        }
    }
    // A faction in two feuds is one vendetta source per district (its
    // victims are any of its enemies' members, `may_strike`).
    out.sort_unstable();
    out.dedup();
    world.fv_active = out;
}

/// The sources touching the city now: `fv_active` plus the live riots
/// (their district) and the episodes (the agent's district).
pub fn active_now(world: &World) -> Vec<ActiveSource> {
    let mut out = world.fv_active.clone();
    for r in &world.riots {
        out.push(ActiveSource {
            source: ViolenceSource::Riot,
            district: r.district,
            faction: None,
            riot: Some(r.id),
            episode: None,
        });
    }
    for &a in &world.episodes {
        if let Some(t) = world.comp::<Position>(a).map(|p| p.tile) {
            out.push(ActiveSource {
                source: ViolenceSource::Episode,
                district: world.district_of(t),
                faction: Some(a),
                riot: None,
                episode: Some(a),
            });
        }
    }
    out
}

/// May source `s` strike `id`: never one of the acting faction's own; a
/// vendetta side only the members of a faction it feuds with (what counts
/// on screen, `source_of`).
fn may_strike(world: &World, s: &ActiveSource, id: EntityId) -> bool {
    use crate::systems::grudges::member_of;
    let Some(f) = s.faction else { return true };
    if member_of(world, id, f) {
        return false;
    }
    if s.source != ViolenceSource::Vendetta {
        return true;
    }
    world.vendettas.iter().any(|v| (v.a == f && member_of(world, id, v.b)) || (v.b == f && member_of(world, id, v.a)))
}

/// The day's live riots and episodes (`FvTally::day_sources`), with fix 2
/// the hours each was live (`FvTally::source_hours`), and the riots'
/// rosters (`FvTally::riot_rosters`), from the hourly tally.
fn remember_live(world: &mut World, all: &[ActiveSource]) {
    let now = world.tick;
    let hours = crate::systems::fixes::short_sources(world);
    for s in all.iter().filter(|s| is_short(s.source)) {
        if !world.fv_tally.day_sources.contains(s) {
            world.fv_tally.day_sources.push(*s);
        }
        if hours {
            let t = &mut world.fv_tally.source_hours;
            match t.iter_mut().find(|(x, _)| x == s) {
                Some((_, h)) => *h += 1,
                None => t.push((*s, 1)),
            }
        }
    }
    for r in &world.riots {
        let e = world.fv_tally.riot_rosters.entry(r.id).or_default();
        e.0 = now;
        for &m in &r.rioters {
            if !e.1.contains(&m) {
                e.1.push(m);
            }
        }
    }
}

/// L27: is `id` a rioter of riot `riot`: live (`World::rioter_of`) or on
/// the roster the hourly tally kept after the riot ended.
pub fn rioter_in(world: &World, id: EntityId, riot: Option<u32>) -> bool {
    let Some(r) = riot else { return false };
    world.rioter_of.get(&id) == Some(&r) || world.fv_tally.riot_rosters.get(&r).is_some_and(|(_, m)| m.contains(&id))
}

/// The sources of `all` touching `d` that may strike `id` (`may_strike`).
fn touching<'a>(world: &World, all: &'a [ActiveSource], d: DistrictId, id: EntityId) -> Vec<&'a ActiveSource> {
    all.iter().filter(|s| s.district == d && may_strike(world, s, id)).collect()
}

/// Ledger maths, exposure: once an hour after the assignment, every body
/// adult (not sentenced) adds one body-hour to `(source, its district, its
/// class)` for each source touching that district (not one of its own
/// faction's). Bodies only (L40); O(bodies × sources there).
pub fn tally_exposure(world: &mut World) {
    let all = active_now(world);
    remember_live(world, &all);
    if all.is_empty() {
        return;
    }
    let mut adds: std::collections::BTreeMap<crate::ledger::CellKey, u32> = std::collections::BTreeMap::new();
    let mut civ_hours = 0u64;
    for id in world.bodies() {
        if world.has::<Sentence>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else { continue };
        let d = world.district_of(tile);
        let srcs = touching(world, &all, d, id);
        if srcs.is_empty() {
            continue;
        }
        let class = victim_class(world, id);
        for s in srcs {
            *adds.entry((s.source, d, class)).or_default() += 1;
        }
        if is_civ(class) {
            civ_hours += 1;
        }
    }
    for (key, n) in adds {
        let cell = world.order_rates.cell_mut(key);
        cell.exposure_today = cell.exposure_today.saturating_add(n);
    }
    world.fv_tally.body_civ_hours += civ_hours;
}

/// A source's prior per agent-day (`[fviolence] prior`), Killed,
/// Assaulted, Robbed, Abducted; `Order(Harvest)`'s Abducted is
/// `harvest_abducted`; times the victim class's `class_mult` (phase 5).
fn prior_of(world: &World, source: ViolenceSource, class: VictimClass) -> [f32; 4] {
    let m = &world.config.fviolence.class_mult;
    let mult = match class {
        VictimClass::Civilian => m.civilian,
        VictimClass::Member => m.member,
        VictimClass::Watch => m.watch,
        // Violence fix 1: the civilian prior times `vf_street_prior_mult`.
        VictimClass::Street => m.civilian.map(|x| x * world.config.life.vf_street_prior_mult.max(0.0)),
    };
    let p = source_prior(world, source);
    std::array::from_fn(|k| p[k] * mult[k])
}

fn source_prior(world: &World, source: ViolenceSource) -> [f32; 4] {
    let p = &world.config.fviolence.prior;
    match source {
        ViolenceSource::Order(crate::components::Order::Harvest) => {
            let mut r = p.order;
            r[3] = p.harvest_abducted;
            r
        }
        ViolenceSource::Order(_) => p.order,
        ViolenceSource::Vendetta => p.vendetta,
        ViolenceSource::Riot => p.riot,
        ViolenceSource::Episode => p.episode,
        // M16a (plan C19): a contract record's holes never enter the ledger.
        ViolenceSource::Contract(_) => [0.0; 4],
    }
}

/// Ledger maths, victim side: per agent-day,
/// `rate[k] = fv_mult × (Σ victims[k] + prior[k] × prior_weight) ÷ (Σ exposure ÷ 24 + prior_weight)`
/// over the cell's last `rate_days` days.
pub fn cell_rates(world: &World, source: ViolenceSource, d: DistrictId, class: VictimClass) -> [f32; 4] {
    let cfg = &world.config.fviolence;
    let days = cfg.rate_days;
    let (victims, exposure) = world.order_rates.cells.get(&(source, d, class)).map_or(([0u64; 4], 0u64), |c| {
        let v = std::array::from_fn(|k| c.victims[k].iter().rev().take(days).map(|&x| u64::from(x)).sum());
        (v, c.exposure.iter().rev().take(days).map(|&x| u64::from(x)).sum())
    });
    let prior = prior_of(world, source, class);
    let pw = cfg.prior_weight.max(0.0);
    let denom = exposure as f32 / 24.0 + pw;
    std::array::from_fn(|k| {
        if denom <= 0.0 {
            return 0.0;
        }
        (cfg.fv_mult * (victims[k] as f32 + prior[k] * pw) / denom).clamp(0.0, 1.0)
    })
}

/// A source that ends within hours (a riot, an episode): the midnight pass
/// reads it from `day_sources`, and fix 2 scales it by its live hours.
fn is_short(source: ViolenceSource) -> bool {
    matches!(source, ViolenceSource::Riot | ViolenceSource::Episode)
}

/// Violence fix 2: the share of the day short source `s` was live, its
/// hourly tallies / 24 (at least one: a source live at midnight but not yet
/// tallied), at most 1. Exposure is learned per 24 body-hours, so a 6-hour
/// riot rolled as a full day over-applies its rate about 4x.
pub fn live_share(hours: &[(ActiveSource, u32)], s: &ActiveSource) -> f32 {
    let h = hours.iter().find(|(x, _)| x == s).map_or(0, |&(_, h)| h);
    (h.max(1) as f32 / 24.0).min(1.0)
}

/// L26, L28: the daily pass, at the top of `bind::run`'s midnight branch.
/// Each Statistical adult (ascending; not jailed, emigrating, pinned, nor a
/// victim of a hole opened since the last midnight) in a district a source
/// touches (its Home's, else its tile's) draws `u_K, u_A, u_R, u_Ab, u_src`
/// on `WordNs::FViolence` keyed `(day, id.index)` against its class's
/// rates summed over the sources (not of its faction): Killed, else
/// Assaulted, else Robbed, else Abducted (a Kit at `harvest_min_visible`
/// with chrome; `Order(Harvest)`'s prior). At most one hit; the source by
/// `u_src` over the sources' shares of that kind; `day_cap` per kind
/// city-wide (`fv_capped` counts the hits it stops). A hit is
/// `lod::stat_hit` or `chrome::abduct_offscreen` with the source.
pub fn daily(world: &mut World) {
    // The day's riots and episodes, live now or earlier today.
    let mut all = active_now(world);
    for s in std::mem::take(&mut world.fv_tally.day_sources) {
        if !all.contains(&s) {
            all.push(s);
        }
    }
    // Fix 2: the share of the day each riot and episode was live.
    let hours = std::mem::take(&mut world.fv_tally.source_hours);
    let short = crate::systems::fixes::short_sources(world);
    let ttl = (world.config.bind.hole_ttl_days + 1) * crate::time::TICKS_PER_DAY;
    let now = world.tick;
    world.fv_tally.riot_rosters.retain(|_, (t, _)| *t + ttl > now);
    if all.is_empty() {
        return;
    }
    let today = world.day();
    // Holes opened after the last midnight's pass (strictly: the pass's own
    // holes carry that midnight's tick).
    let since = world.tick.saturating_sub(crate::time::TICKS_PER_DAY);
    let cap = world.config.fviolence.day_cap;
    let min_visible = world.config.chrome.harvest_min_visible;
    // The island pass phase 10 (OPEN-185): each source strikes only with
    // its off-screen hands, one hit per hand a day, within its budget.
    let hands = hands_on(world);
    let mut pools = if hands { build_pools(world, &all) } else { Default::default() };
    let mut spent: std::collections::BTreeSet<EntityId> = std::collections::BTreeSet::new();
    let mut ids: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    ids.sort_unstable();
    let mut hits = [0u32; 4];
    for id in ids {
        let Some(b) = world.comp::<Brain>(id) else { continue };
        if b.lod != Lod::Statistical || b.emigrating || b.pinned || world.has::<Sentence>(id) {
            continue;
        }
        if !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let recent = world
            .holes_by_agent
            .get(&id)
            .is_some_and(|hs| hs.iter().any(|h| world.holes.get(h).is_some_and(|x| x.tick > since)));
        if recent {
            continue;
        }
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let d = match home {
            Some(h) => world.district_of_building(h),
            None => match world.comp::<Position>(id) {
                Some(p) => world.district_of(p.tile),
                None => continue,
            },
        };
        let mut srcs: Vec<ActiveSource> = touching(world, &all, d, id).into_iter().copied().collect();
        if srcs.is_empty() {
            continue;
        }
        let class = victim_class(world, id);
        if hands {
            // A touched agent-day all the same (the parity tally below);
            // only the sources with a hand left roll.
            if is_civ(class) {
                world.fv_tally.stat_civ_days += 1;
            }
            srcs.retain(|s| has_hands(&pools, &spent, s));
            if srcs.is_empty() {
                continue;
            }
        }
        let rates: Vec<[f32; 4]> = srcs
            .iter()
            .map(|s| {
                let r = cell_rates(world, s.source, d, class);
                if short && is_short(s.source) {
                    let f = live_share(&hours, s);
                    r.map(|x| x * f)
                } else {
                    r
                }
            })
            .collect();
        let sum = |k: usize| rates.iter().map(|r| r[k]).sum::<f32>();
        let mut rng = world.rng.word(crate::word::WordNs::FViolence, today, u64::from(id.index));
        let u: [f32; 5] = std::array::from_fn(|_| rng.random());
        if is_civ(class) && !hands {
            world.fv_tally.stat_civ_days += 1;
        }
        let kitted =
            world.comp::<crate::components::Kit>(id).is_some_and(|k| k.visible >= min_visible && k.chrome_value > 0);
        let kind = if u[0] < sum(0) {
            0
        } else if u[1] < sum(1) {
            1
        } else if u[2] < sum(2) {
            2
        } else if kitted && u[3] < sum(3) {
            3
        } else {
            continue;
        };
        if hits[kind] >= cap.get(kind) {
            world.stats.current.living.fv_capped += 1;
            continue;
        }
        // The source by its share of the kind's summed rate.
        let total = sum(kind);
        let mut x = u[4] * total;
        let mut pick = srcs.len() - 1;
        for (i, r) in rates.iter().enumerate() {
            if x < r[kind] {
                pick = i;
                break;
            }
            x -= r[kind];
        }
        let src = srcs[pick];
        // The island pass phase 10: the hit spends one of the source's
        // free hands, or is not made.
        let hand = if hands {
            match take_hand(world, &mut pools, &spent, &src, kind, id) {
                Some(h) => Some(h),
                None => {
                    world.stats.current.living.fv_hands_capped += 1;
                    continue;
                }
            }
        } else {
            None
        };
        let done = if kind == 3 {
            crate::systems::chrome::abduct_offscreen(world, id, Some(&src))
        } else {
            crate::systems::lod::stat_hit(world, id, KINDS[kind], Some(&src))
        };
        if !done {
            // Rare (a victim with no tile): the hand and the budget are
            // not given back.
            continue;
        }
        if let Some(h) = hand {
            prebind(world, id, KINDS[kind], h);
            spent.insert(h);
        }
        hits[kind] += 1;
        let l = &mut world.stats.current.living;
        match kind {
            0 => l.fv_killed += 1,
            1 => l.fv_assaulted += 1,
            2 => l.fv_robbed += 1,
            _ => l.fv_abducted += 1,
        }
        if is_civ(class) && kind <= 1 {
            world.fv_tally.stat_civ_hits += 1;
        }
    }
}

/// The island pass phase 10: the pass's hole on `victim` (opened this
/// tick) is pre-bound to the hand that made it (`Hole::prebound`, bound as
/// a contract record's, `bind::prebound_ok`), and the hand's day is spent
/// (a gang member's GangWork day, `stat_gang_day`).
fn prebind(world: &mut World, victim: EntityId, kind: HoleKind, hand: EntityId) {
    let id = crate::components::hole_id(world.tick, victim, kind);
    if let Some(h) = world.holes.get_mut(&id) {
        h.prebound = Some(hand);
    }
    let today = world.day();
    if world.has::<crate::components::GangMember>(hand) {
        if let Some(b) = world.comp_mut::<Brain>(hand) {
            b.gang_task_day = Some(today);
        }
    }
}

/// The day's `kill_rate_*` columns (end of day): violent deaths per 1,000
/// agent-days by the victim's tier at death, all adults and civilians,
/// over today's free adults by tier (the sentenced apart); the day's
/// tallies reset.
pub fn snapshot(world: &mut World) {
    let mut n = [0u32; 4];
    for (i, lod) in [Lod::Full, Lod::Coarse, Lod::Statistical].into_iter().enumerate() {
        for &id in world.tier(lod) {
            if world.has::<Sentence>(id) || !crate::systems::demography::is_adult(world, id) {
                continue;
            }
            let stat = usize::from(i == 2);
            n[stat] += 1;
            if is_civ(victim_class(world, id)) {
                n[2 + stat] += 1;
            }
        }
    }
    let k = std::mem::take(&mut world.fv_tally.day_kills);
    let rate = |i: usize| k[i] as f32 * 1000.0 / n[i].max(1) as f32;
    let l = &mut world.stats.current.living;
    l.kill_rate_body = rate(0);
    l.kill_rate_stat = rate(1);
    l.kill_rate_body_civ = rate(2);
    l.kill_rate_stat_civ = rate(3);
}

/// L37 god `FactionStrike`: the gang's order pinned to Contest for `days`
/// and the district touched off screen until then (`Gang.strike`).
pub fn faction_strike(world: &mut World, gang: EntityId, district: DistrictId, days: u64) -> bool {
    use crate::components::{Gang, Order};
    let now = world.tick;
    let until = now + days * crate::time::TICKS_PER_DAY;
    let Some(g) = world.comp_mut::<Gang>(gang) else { return false };
    g.strike = Some((district, until));
    if g.order != Order::Contest {
        g.order = Order::Contest;
        g.order_since = now;
        g.raid_at = None;
    }
    let s = ActiveSource {
        source: ViolenceSource::Order(Order::Contest),
        district,
        faction: Some(gang),
        riot: None,
        episode: None,
    };
    if !world.fv_active.contains(&s) {
        world.fv_active.push(s);
    }
    true
}
