//! M15 § 7 (plan phase 4, W36-W41): the Feeds. A Feed is a building whose
//! Reporters turn the day's most newsworthy pool entries into story
//! records: each story is posted back into every district pool the Feed
//! covers at the Feed's reach, tagged with the Feed and a slant, so the
//! agents who draw it hold a heard entry carrying `press` (W38). A corp
//! holding `Spin` pays Feeds to run a known misdeed of a rival (a plant)
//! and to drop stories about itself (a bury); every corp pays ads. All of
//! it is counters, coin transfers and one keyed roll per story: no event
//! here is more than a record about other records.
//!
//! Runs inside the word's midnight chain (`systems::word`), after the
//! pools decay and leak and before the hearing, so a story is read the
//! same night. Nothing runs per agent per tick.

use std::collections::BTreeMap;

use rand::Rng;
use smallvec::SmallVec;

use crate::components::{Building, BuildingKind, Corp, Job, Personality, Role};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::{self, Tick, TICKS_PER_DAY};
use crate::utility::curves::{can, Curve, GATE};
use crate::utility::Consideration;
use crate::word::{Deed, DeedRef, FeedState, PoolEntry, Story, WordNs};
use crate::world::World;

/// The Feeds' stories ring (`World::stories`).
pub const STORY_RING: usize = 256;

/// The city's Feed (W41): owner `None`, wages from the Treasury.
pub const CIVIC_WIRE: &str = "Civic Wire";

/// W41: the opening Feeds, `(name, owner corp by name or the city, district)`.
const SEED_FEEDS: [(&str, Option<&str>, &str); 2] =
    [(CIVIC_WIRE, None, "Civic"), ("Nutrix Now", Some("Nutrix"), "Mid West")];

/// A story's run within this many days of the same deed is skipped (W37),
/// and a Spin corp buries stories this recent.
const RECENT_DAYS: u64 = 3;

/// A misdeed older than this is not news to plant (W40).
const PLANT_MAX_AGE_DAYS: u64 = 14;

/// The Feeds are live: the word's master switch and `[news] enabled`.
pub fn on(world: &World) -> bool {
    world.config.gossip.enabled && world.config.news.enabled
}

/// Every standing Feed, ascending.
pub fn all_feeds(world: &World) -> Vec<EntityId> {
    world
        .buildings_of_kind(BuildingKind::Feed)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Feed && !bd.demolished))
        .collect()
}

/// Is `feed` the city's (owner `None`)? The Civic Wire publishes under any licence.
pub fn is_city_feed(world: &World, feed: EntityId) -> bool {
    world.comp::<Building>(feed).is_some_and(|b| b.owner.is_none())
}

/// Living, licensed Feeds, ascending: every standing Feed, or only the
/// city's while the press licence is off (spec § 9).
pub fn feeds(world: &World) -> Vec<EntityId> {
    let licence = world.levers.press_licence;
    all_feeds(world).into_iter().filter(|&f| licence || is_city_feed(world, f)).collect()
}

/// A Feed's state, if it has one.
pub fn feed_state(world: &World, feed: EntityId) -> Option<&FeedState> {
    world.comp::<Building>(feed).and_then(|b| b.feed.as_ref())
}

/// The district bitset a Feed covers (spec § 7): every district when its
/// owner is a corp or the city, else its own and the adjacent ones.
pub fn covers_for(world: &World, feed: EntityId) -> u16 {
    let n = world.districts.len().min(16);
    let all: u16 = if n >= 16 { u16::MAX } else { (1u16 << n) - 1 };
    let owner = world.comp::<Building>(feed).and_then(|b| b.owner);
    if owner.is_none_or(|o| world.has::<Corp>(o)) {
        return all;
    }
    let d = world.district_of_building(feed).index();
    let adj = world.district_adjacent.get(d).copied().unwrap_or(0);
    (adj | (1u16 << d.min(15))) & all
}

/// A default name for a Feed founded in play: "{owner's last name} Feed",
/// or "{corp} Feed".
fn default_name(world: &World, owner: Option<EntityId>) -> String {
    match owner {
        None => CIVIC_WIRE.to_string(),
        Some(o) if world.has::<Corp>(o) => format!("{} Feed", world.owner_label(Some(o))),
        Some(o) => {
            let full = world.name_of(o);
            format!("{} Feed", full.split_whitespace().last().unwrap_or("The"))
        }
    }
}

/// Make `lot` (already built as a Feed) a Feed with a name.
fn open_feed(world: &mut World, feed: EntityId, name: String) {
    let reach = world.config.news.reach_base;
    let covers = covers_for(world, feed);
    if let Some(b) = world.comp_mut::<Building>(feed) {
        b.feed = Some(FeedState { name, reach, covers, buried: Default::default(), stories_today: 0 });
    }
}

/// W41: last in `World::new` (after `virt::seed_labs`): the Civic Wire on
/// the vacant Lot nearest the Civic centroid (owner `None`) and Nutrix Now
/// on the one nearest the Mid West centroid (owner Nutrix); a district
/// with no Lot left takes the Lot nearest its centroid anywhere. Each opens
/// three Reporter vacancies (`build_on_lot`). No RNG. A no-op with the news off.
pub fn seed_feeds(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    if !on(world) {
        return built;
    }
    for (name, corp_name, district) in SEED_FEEDS {
        let owner = match corp_name {
            Some(cn) => {
                match world.corps().into_iter().find(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.name == cn)) {
                    Some(c) => Some(c),
                    None => continue,
                }
            }
            None => None,
        };
        let Some((d, centroid)) = world.districts.iter().find(|x| x.name == district).map(|x| (x.id, x.centroid))
        else {
            continue;
        };
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter_map(|l| {
                world.comp::<Building>(l).map(|b| (world.district_of_building(l) != d, b.door.manhattan(centroid), l))
            })
            .min()
            .map(|(_, _, l)| l);
        let Some(lot) = lot else { continue };
        if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Feed, owner).is_err() {
            continue;
        }
        open_feed(world, lot, name.to_string());
        built.push(lot);
        let at = world.districts.get(world.district_of_building(lot).index()).map_or("?", |x| x.name.as_str());
        let who = world.owner_label(owner);
        let text = format!("{who} opened {name} (Feed) in {at} (seeded for {district}; Lot {})", lot.index);
        let mut actors: SmallVec<[EntityId; 2]> = SmallVec::new();
        if let Some(o) = owner {
            actors.push(o);
        }
        actors.push(lot);
        world.push_event(EventKind::Founded, &actors, text);
    }
    // Review fix: seeded only once a Feed stands, so a city with no Lot at
    // seed gets its Feeds from `migrate` on a later load.
    world.feeds_seeded = !built.is_empty();
    built
}

/// W41: a save from before the Feeds (news on, never seeded) gets them on load.
pub fn migrate(world: &mut World) {
    if on(world) && !world.feeds_seeded && all_feeds(world).is_empty() {
        seed_feeds(world);
        crate::systems::virt::relink(world);
    }
}

/// The spec's `news`: `deed_sal × (0.3 + standing(actor) + standing(object))
/// × 0.8^(age days)`.
pub fn newsworthiness(world: &World, e: &PoolEntry, now: Tick) -> f32 {
    let sal = world.config.gossip.deed_sal.get(e.deed);
    let st = |id: Option<EntityId>| id.map_or(0.0, |x| crate::systems::reputation::rep(world, x).standing);
    let age = now.saturating_sub(e.tick) / TICKS_PER_DAY;
    sal * (0.3 + st(e.actor) + st(e.object)) * 0.8f32.powi(age.min(60) as i32)
}

/// A story's slant (plan deviation: the spec fixes only a plant's, −1):
/// `−deed_sev` for a harmful deed, `+min(1, 2 × honour_w)` for an
/// honourable one, else 0.
pub fn organic_slant(world: &World, deed: Deed) -> f32 {
    let sev = world.config.gossip.deed_sev.get(deed);
    if sev > 0.0 {
        return -sev.min(1.0);
    }
    let h = world.config.reputation.honour_w.get(deed);
    if h > 0.0 {
        (2.0 * h).min(1.0)
    } else {
        0.0
    }
}

/// A misdeed (W40): `honour_w < 0` or `dread_w > 0`.
fn is_misdeed(world: &World, deed: Deed) -> bool {
    world.config.reputation.honour_w.get(deed) < 0.0 || world.config.reputation.dread_w.get(deed) > 0.0
}

/// Whom a Spin corp may plant against: its niche rivals, their execs, and
/// the gangs in an open vendetta with it, ascending.
fn plant_targets(world: &World, corp: EntityId) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    let Some(c) = world.comp::<Corp>(corp) else { return out };
    for &n in &c.niches {
        for r in crate::systems::corp_brain::rivals_in(world, corp, n) {
            out.push(r);
            if let Some(e) = world.comp::<Corp>(r).and_then(|rc| rc.exec) {
                out.push(e);
            }
        }
    }
    for v in &world.vendettas {
        let other = if v.a == corp {
            v.b
        } else if v.b == corp {
            v.a
        } else {
            continue;
        };
        if world.has::<crate::components::Gang>(other) {
            out.push(other);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Every rival misdeed the corp's exec and employees know of (W40), with
/// its `news` score, best first (ties: older tick, then the deed's key).
/// One scan of their `entries ∪ heard`: O(employees × 32).
fn known_misdeeds(world: &World, corp: EntityId) -> Vec<(PoolEntry, f32)> {
    let targets = plant_targets(world, corp);
    if targets.is_empty() {
        return Vec::new();
    }
    let now = world.tick;
    let horizon = now.saturating_sub(PLANT_MAX_AGE_DAYS * TICKS_PER_DAY);
    let mut seen: BTreeMap<DeedKey, PoolEntry> = BTreeMap::new();
    for m in crate::systems::reputation::members_of(world, corp) {
        let Some(mem) = world.comp::<crate::components::Memory>(m) else { continue };
        for (e, r) in crate::systems::memory::deeds(m, mem) {
            let Some(actor) = r.actor else { continue };
            if e.tick < horizon || targets.binary_search(&actor).is_err() || !is_misdeed(world, r.deed) {
                continue;
            }
            let key = (r.deed, actor, r.object, time::day(e.tick));
            seen.entry(key).or_insert_with(|| PoolEntry {
                deed: r.deed,
                actor: Some(actor),
                object: r.object,
                tick: e.tick,
                hops: 1,
                reach: 0.0,
                hole: None,
                story: None,
                kin: SmallVec::new(),
                told: SmallVec::new(),
                district: crate::systems::gossip::talk_district(world, actor),
                press: 0,
            });
        }
    }
    let mut out: Vec<(PoolEntry, f32)> =
        seen.into_values().map(|e| (newsworthiness(world, &e, now), e)).map(|(s, e)| (e, s)).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.tick.cmp(&b.0.tick)));
    out
}

/// W40: the best rival misdeed the corp's people know, and its `news`.
pub fn best_known_misdeed(world: &World, corp: EntityId) -> Option<(PoolEntry, f32)> {
    known_misdeeds(world, corp).into_iter().next()
}

/// W40: did a licensed Feed run a story about the corp's people in the
/// last three days (one a Spin would bury)?
pub fn buryable(world: &World, corp: EntityId) -> bool {
    let horizon = world.tick.saturating_sub(RECENT_DAYS * TICKS_PER_DAY);
    let live = feeds(world);
    world
        .stories
        .iter()
        .rev()
        .take_while(|s| s.tick >= horizon)
        .any(|s| s.paid_by != Some(corp) && live.contains(&s.feed) && about(world, s.deed_actor(), corp))
}

/// Did `feed` run this deed (by its own actor, distorted or not) within
/// `RECENT_DAYS`?
fn ran_recently(world: &World, feed: EntityId, deed: Deed, actor: EntityId, object: Option<EntityId>) -> bool {
    let horizon = world.tick.saturating_sub(RECENT_DAYS * TICKS_PER_DAY);
    world
        .stories
        .iter()
        .rev()
        .take_while(|s| s.tick >= horizon)
        .any(|s| s.feed == feed && s.deed == deed && s.deed_actor() == actor && s.object == object)
}

/// Is `id` the faction `f` or one of its people (a gang's member, a corp's
/// exec or staff, a city guard or the captain for the Law)?
fn about(world: &World, id: EntityId, f: EntityId) -> bool {
    crate::systems::grudges::member_of(world, id, f)
}

/// The factions the Civic Wire buries today: the `CensorStories` lever's,
/// and the Law while the captain's lawfulness is below `censor_lawfulness`.
fn censored(world: &World) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = world.levers.censored.iter().copied().collect();
    let low = world
        .law()
        .and_then(|l| l.captain)
        .and_then(|c| world.comp::<Personality>(c))
        .is_some_and(|p| p.lawfulness < world.config.news.censor_lawfulness);
    if low {
        if let Some(j) = world.building_of_kind(BuildingKind::Jail).filter(|&j| world.has::<crate::components::Law>(j))
        {
            out.push(j);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// A Feed's Reporters, ascending; and those who worked a shift yesterday.
fn reporters(world: &World, feed: EntityId) -> (Vec<EntityId>, usize) {
    let now = world.tick;
    let staff: Vec<EntityId> = world
        .workers(Role::Reporter)
        .iter()
        .copied()
        .filter(|&r| world.comp::<Job>(r).is_some_and(|j| j.employer == Some(feed)))
        .collect();
    let worked = staff
        .iter()
        .filter(|&&r| {
            world.comp::<Job>(r).is_some_and(|j| {
                j.last_shift_day.is_some() && j.last_shift_day == Some(j.shift_key_at(now.saturating_sub(1)))
            })
        })
        .count();
    (staff, worked)
}

/// The owner's standing for a Feed's reach: a corp's or an agent's; the
/// city reads the Law's (plan deviation: the city has no reputation).
fn owner_standing(world: &World, feed: EntityId) -> f32 {
    match world.comp::<Building>(feed).and_then(|b| b.owner) {
        Some(o) => crate::systems::reputation::rep(world, o).standing,
        None => world
            .building_of_kind(BuildingKind::Jail)
            .map_or(0.0, |j| crate::systems::reputation::rep(world, j).standing),
    }
}

/// The share of `amount` the news tax diverts to the Treasury.
fn news_tax_of(world: &World, amount: i64) -> i64 {
    ((amount as f32) * world.levers.news_tax.clamp(0.0, 1.0)).round() as i64
}

/// A payment to a Feed's owner (`flow` Ads or Plant): a corp or the city
/// pays the full amount; the news tax's share goes to the Treasury.
fn pay_feed(world: &mut World, payer: EntityId, feed: EntityId, amount: i64, flow: Flow) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let owner = world.comp::<Building>(feed).and_then(|b| b.owner);
    if owner == Some(payer) {
        return 0;
    }
    let tax = if owner.is_some() { news_tax_of(world, amount).min(amount) } else { 0 };
    let paid = ownership::charge(world, Some(payer), owner, amount - tax, flow);
    let taxed = if tax > 0 { ownership::charge(world, Some(payer), None, tax, Flow::Tax) } else { 0 };
    if owner.is_some() {
        ownership::credit(world, feed, amount - tax);
    }
    paid + taxed
}

/// W37 Step 1: each Feed's reach (`reach_base + reach_per_reporter ×
/// reporters who worked yesterday + 0.3 × owner standing`, clamped) and
/// cover, recomputed daily; expired buries dropped; a Feed built in play
/// gets its name.
fn refresh(world: &mut World) {
    let cfg = world.config.news.clone();
    let now = world.tick;
    for f in all_feeds(world) {
        if feed_state(world, f).is_none() {
            let owner = world.comp::<Building>(f).and_then(|b| b.owner);
            let name = default_name(world, owner);
            open_feed(world, f, name);
        }
        let (_, worked) = reporters(world, f);
        let reach =
            (cfg.reach_base + cfg.reach_per_reporter * worked as f32 + 0.3 * owner_standing(world, f)).clamp(0.0, 1.0);
        let covers = covers_for(world, f);
        if let Some(s) = world.comp_mut::<Building>(f).and_then(|b| b.feed.as_mut()) {
            s.reach = reach;
            s.covers = covers;
            s.stories_today = 0;
            s.buried.retain(|&(_, until)| until > now);
        }
    }
}

/// A deed's key across the pools: `(deed, actor, object, day)`.
type DeedKey = (Deed, EntityId, Option<EntityId>, u64);

/// A story the day's Spin paid for: `(feed, deed, payer)`.
type Plant = (EntityId, PoolEntry, EntityId);

/// W40 (orchestrator deviation: a side spend, not a corp-wide order):
/// the Spin score of a corp, the spec's considerations: GATE `can(a
/// licensed Feed ∧ cash ≥ 0.3 ∧ treasury above the fleet reserve ∧ work)`
/// (work: a misdeed to plant or a story to bury), `honour drop` (the fall
/// from its 14-day high ÷ 0.2) Linear{0.6, 0.4}, `rival misdeed` (best
/// `news`) Linear{0.5, 0.5}, Street `unrest` Linear{0.4, 0.6}, the exec's
/// `greed` Linear{0.4, 0.6}; the product plus `order_flat.spin`, or `None`
/// with the gate shut.
pub fn spin_considerations(world: &World, corp: EntityId) -> (Vec<Consideration>, Option<f32>) {
    let (cs, score, _) = spin_scored(world, corp);
    (cs, score)
}

/// The rival misdeeds a corp's people know, best first (`known_misdeeds`).
type Misdeeds = Vec<(PoolEntry, f32)>;

/// `spin_considerations` and, with the gate open, the misdeeds it read
/// (handed to the day's plant so they are not scanned twice). The cheap
/// gates (a licensed Feed, cash, the fleet reserve) come first: a corp
/// that fails them never scans its people's memories.
fn spin_scored(world: &World, corp: EntityId) -> (Vec<Consideration>, Option<f32>, Misdeeds) {
    let Some(c) = world.comp::<Corp>(corp) else { return (Vec::new(), None, Vec::new()) };
    let cash = crate::systems::corp_brain::cash_of(c);
    let reserve = crate::systems::corps::fleet_reserve(world, corp);
    if feeds(world).is_empty() || cash < 0.3 || c.treasury <= reserve {
        return (vec![Consideration::new("feed, cash, reserve & work", 0.0, GATE)], None, Vec::new());
    }
    let misdeeds = known_misdeeds(world, corp);
    let best = misdeeds.first().map_or(0.0, |(_, s)| s.clamp(0.0, 1.0));
    let work = best > 0.0 || buryable(world, corp);
    let honour = crate::systems::reputation::rep(world, corp).honour;
    let high = c.honour_hist.iter().copied().fold(honour, f32::max);
    let greed = c.exec.and_then(|e| world.comp::<Personality>(e)).map_or(0.5, |p| p.greed);
    let cs = vec![
        Consideration::new("feed, cash, reserve & work", can(work), GATE),
        Consideration::new("honour drop", ((high - honour) / 0.2).clamp(0.0, 1.0), Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("rival misdeed", best, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new(
            "unrest",
            crate::systems::corp_brain::street_unrest(world),
            Curve::Linear { m: 0.4, b: 0.6 },
        ),
        Consideration::new("greed", greed, Curve::Linear { m: 0.4, b: 0.6 }),
    ];
    if cs.iter().any(|x| x.output <= 0.0) {
        return (cs, None, Vec::new());
    }
    let score = cs.iter().map(|x| x.output).product::<f32>() + world.config.corps.order_flat.spin;
    (cs, Some(score), misdeeds)
}

/// W40 (orchestrator deviation): at midnight each corp's Spin score; a
/// corp spins (`Corp.spin_since`) while it is at least `[news] spin_min`,
/// beside whatever corp-wide order it holds. Returns the spinners'
/// known misdeeds for the day's plant.
pub fn rescore_spin(world: &mut World) -> BTreeMap<EntityId, Misdeeds> {
    let now = world.tick;
    let min = world.config.news.spin_min;
    let mut out = BTreeMap::new();
    for corp in world.corps() {
        let (trace, score, misdeeds) = spin_scored(world, corp);
        let on = score.is_some_and(|s| s >= min);
        if on {
            out.insert(corp, misdeeds);
        }
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.spin_score = score.unwrap_or(0.0);
            c.spin_trace = trace;
            c.spin_since = match (on, c.spin_since) {
                (true, Some(t)) => Some(t),
                (true, None) => Some(now),
                (false, _) => None,
            };
        }
    }
    out
}

/// Would `feed` drop a story about `actor` (and `object`)? An actor it was
/// paid to bury, or, on the Civic Wire, anyone of a censored faction:
/// organic stories and plants pass the same filter (spec § 7, § 9).
fn silenced(world: &World, feed: EntityId, censor: &[EntityId], actor: EntityId, object: Option<EntityId>) -> bool {
    if feed_state(world, feed).is_some_and(|s| s.buried.iter().any(|&(a, _)| a == actor)) {
        return true;
    }
    is_city_feed(world, feed)
        && censor.iter().any(|&f| about(world, actor, f) || object.is_some_and(|o| about(world, o, f)))
}

/// W40: every spinning corp plants its best known rival misdeed on
/// the licensed Feed with the largest reach it does not own (its own at
/// cost 0 when it owns the only one), paying `plant_price × reach`, and
/// pays `bury_price × reach` per Feed that ran a story about it, its exec
/// or its staff in the last three days, burying those actors for
/// `bury_days`. No fabrication: only deeds its people hold.
fn spin(world: &mut World, mut known: BTreeMap<EntityId, Misdeeds>) -> Vec<Plant> {
    let mut plants: Vec<Plant> = Vec::new();
    let cfg = world.config.news.clone();
    let now = world.tick;
    let live = feeds(world);
    if live.is_empty() {
        return plants;
    }
    let spinners: Vec<EntityId> = world
        .corps()
        .into_iter()
        .filter(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.spin_since.is_some()))
        .collect();
    let censor = censored(world);
    // M15 phase 5: one plant per deed a day. Every spinner read the same
    // pools, so three corps paid to run the same story about the same man.
    let mut taken: Vec<(Deed, EntityId, Option<EntityId>, u64)> = Vec::new();
    for corp in spinners {
        let reach_of = |w: &World, f: EntityId| feed_state(w, f).map_or(0.0, |s| s.reach);
        // The plant: the largest reach not owned (ties lower id), else its own.
        let mut ranked: Vec<(bool, f32, EntityId)> =
            live.iter().map(|&f| (world.owner_of(f) == Some(corp), reach_of(world, f), f)).collect();
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
        if let Some(&(_, reach, feed)) = ranked.first() {
            // The misdeeds `rescore_spin` read; a plant passes the Feed's
            // buried and censor filters, as an organic story does.
            let pick = known.remove(&corp).unwrap_or_default().into_iter().find(|(e, _)| {
                e.actor.is_some_and(|a| {
                    !taken.contains(&(e.deed, a, e.object, time::day(e.tick)))
                        && !ran_recently(world, feed, e.deed, a, e.object)
                        && !silenced(world, feed, &censor, a, e.object)
                })
            });
            if let Some((e, _)) = pick {
                let own = world.owner_of(feed) == Some(corp);
                let price = if own { 0 } else { (cfg.plant_price as f32 * reach).round() as i64 };
                let cash = world.comp::<Corp>(corp).map_or(0, |c| c.treasury);
                if price == 0 || cash >= price {
                    if let Some(a) = e.actor {
                        taken.push((e.deed, a, e.object, time::day(e.tick)));
                    }
                    pay_feed(world, corp, feed, price, Flow::Plant);
                    let (cn, fname) = (world.owner_label(Some(corp)), feed_name(world, feed));
                    let actor = e.actor.map_or_else(String::new, |a| crate::systems::grudges::label(world, a));
                    world.push_event(
                        EventKind::Planted,
                        &[corp, feed],
                        format!("{cn} paid {fname} to run {} by {actor} ({price}¢)", e.deed.label()),
                    );
                    world.stats.current.word.planted += 1;
                    plants.push((feed, e, corp));
                }
            }
        }
        // The buries: every Feed that ran a story about the corp's people lately.
        let horizon = now.saturating_sub(RECENT_DAYS * TICKS_PER_DAY);
        let mut per_feed: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        for s in world.stories.iter().filter(|s| s.tick >= horizon && s.paid_by != Some(corp)) {
            let who = s.deed_actor();
            if about(world, who, corp) && live.contains(&s.feed) {
                let v = per_feed.entry(s.feed).or_default();
                if !v.contains(&who) {
                    v.push(who);
                }
            }
        }
        for (feed, actors) in per_feed {
            let fresh: Vec<EntityId> = actors
                .into_iter()
                .filter(|a| feed_state(world, feed).is_none_or(|s| !s.buried.iter().any(|&(x, _)| x == *a)))
                .collect();
            if fresh.is_empty() {
                continue;
            }
            let own = world.owner_of(feed) == Some(corp);
            let price = if own { 0 } else { (cfg.bury_price as f32 * reach_of(world, feed)).round() as i64 };
            let cash = world.comp::<Corp>(corp).map_or(0, |c| c.treasury);
            if price > 0 && cash < price {
                continue;
            }
            pay_feed(world, corp, feed, price, Flow::Plant);
            let until = now + u64::from(cfg.bury_days) * TICKS_PER_DAY;
            if let Some(s) = world.comp_mut::<Building>(feed).and_then(|b| b.feed.as_mut()) {
                for &a in &fresh {
                    s.buried.push_back((a, until));
                }
            }
            let (cn, fname, who) = (
                world.owner_label(Some(corp)),
                feed_name(world, feed),
                crate::systems::grudges::label(world, fresh[0]),
            );
            let more = if fresh.len() > 1 { format!(" and {} more", fresh.len() - 1) } else { String::new() };
            world.push_event(
                EventKind::Buried,
                &[corp, feed],
                format!("{cn} paid {fname} to drop stories about {who}{more} ({price}¢)"),
            );
            world.stats.current.word.buried += 1;
        }
    }
    plants
}

/// A Feed's display name.
pub fn feed_name(world: &World, feed: EntityId) -> String {
    feed_state(world, feed).map_or_else(|| world.name_of(feed), |s| s.name.clone())
}

/// W37: the stories. Each licensed Feed scores the named entries (`hops ≤
/// 2`) of its covered pools by `newsworthiness`, skipping actors it was
/// paid to bury (and, on the Civic Wire, the censored factions) and deeds
/// it ran in the last three days, and runs the top `stories_per_day` plus
/// every plant: one roll each on the Story stream for the Feed's
/// distortion (`distort_base × (1 − mean reporter knowledge)`), then posted
/// into every covered pool at `reach × story_reach`, hops 1, tagged with
/// the Feed and the slant, logged in `World::stories` and as a `Story`.
fn run_stories(world: &mut World, plants: Vec<Plant>) {
    let cfg = world.config.news.clone();
    let now = world.tick;
    let day = world.day();
    let censor = censored(world);
    for feed in feeds(world) {
        let Some(st) = feed_state(world, feed).cloned() else { continue };
        let owner = world.comp::<Building>(feed).and_then(|b| b.owner);
        let mut cands: BTreeMap<DeedKey, (f32, PoolEntry)> = BTreeMap::new();
        for (d, pool) in world.rumours.iter().enumerate() {
            if d >= 16 || st.covers & (1u16 << d) == 0 {
                continue;
            }
            for e in &pool.entries {
                let Some(actor) = e.actor else { continue };
                if e.hops > 2 || !world.is_alive(actor) {
                    continue;
                }
                if silenced(world, feed, &censor, actor, e.object) {
                    continue;
                }
                // Plan deviation: a Feed never runs its own owner's deeds
                // (or its people's) unpaid.
                if owner.is_some_and(|o| about(world, actor, o)) {
                    continue;
                }
                if ran_recently(world, feed, e.deed, actor, e.object) {
                    continue;
                }
                let score = newsworthiness(world, e, now);
                let key = (e.deed, actor, e.object, time::day(e.tick));
                match cands.get(&key) {
                    Some((s, _)) if *s >= score => {}
                    _ => {
                        cands.insert(key, (score, e.clone()));
                    }
                }
            }
        }
        let mut ranked: Vec<(f32, PoolEntry)> = cands.into_values().collect();
        // Best first; ties the older deed, then the actor's index.
        ranked.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(a.1.tick.cmp(&b.1.tick))
                .then(a.1.actor.map_or(0, |x| x.index).cmp(&b.1.actor.map_or(0, |x| x.index)))
        });
        let mut runs: Vec<(PoolEntry, f32, Option<EntityId>)> = ranked
            .into_iter()
            .take(usize::from(cfg.stories_per_day))
            .map(|(_, e)| {
                let slant = organic_slant(world, e.deed);
                (e, slant, None)
            })
            .collect();
        for (_, e, payer) in plants.iter().filter(|(f, _, _)| *f == feed) {
            runs.push((e.clone(), -1.0, Some(*payer)));
        }
        if runs.is_empty() {
            continue;
        }
        let (staff, _) = reporters(world, feed);
        let k = if staff.is_empty() {
            0.0
        } else {
            staff.iter().map(|&r| world.comp::<crate::components::Skills>(r).map_or(0.0, |s| s.knowledge)).sum::<f32>()
                / staff.len() as f32
        };
        let mut rng = world.rng.word(WordNs::Story, day, u64::from(feed.index));
        let reach = (st.reach * world.config.news.story_reach).clamp(0.0, 1.0);
        let fname = st.name.clone();
        let mut ran = 0u8;
        for (e, slant, payer) in runs {
            let mut r = DeedRef { deed: e.deed, actor: e.actor, object: e.object };
            let p = world.config.gossip.distort_base * (1.0 - k).clamp(0.0, 1.0);
            let roll: f32 = rng.random();
            if roll < p {
                // The Feed names the actor's gang instead (W8's first swap).
                if let Some(g) = r.actor.and_then(|a| world.gang_of(a)).filter(|&g| Some(g) != r.actor) {
                    r.actor = Some(g);
                    world.stats.current.word.distorted += 1;
                }
            }
            let Some(actor) = r.actor else { continue };
            let press = (slant * 100.0).round().clamp(-100.0, 100.0) as i8;
            for d in 0..world.rumours.len().min(16) {
                if st.covers & (1u16 << d) == 0 {
                    continue;
                }
                let entry = PoolEntry {
                    deed: r.deed,
                    actor: Some(actor),
                    object: r.object,
                    tick: e.tick,
                    hops: 1,
                    reach,
                    hole: None,
                    story: Some(feed),
                    kin: SmallVec::new(),
                    told: SmallVec::new(),
                    district: crate::components::DistrictId(d as u8),
                    press,
                };
                crate::systems::gossip::post(world, crate::components::DistrictId(d as u8), entry);
            }
            let id = world.next_story_id;
            world.next_story_id = world.next_story_id.wrapping_add(1);
            if world.stories.len() >= STORY_RING {
                world.stories.pop_front();
            }
            world.stories.push_back(Story {
                id,
                feed,
                deed: r.deed,
                actor,
                source: e.actor.filter(|&a| a != actor),
                object: r.object,
                tick: now,
                slant,
                paid_by: payer,
            });
            let obj = r.object.map_or_else(String::new, |o| format!(" {}", crate::systems::grudges::label(world, o)));
            let paid = payer.map_or_else(String::new, |p| format!(" (paid by {})", world.owner_label(Some(p))));
            let text =
                format!("{fname}: {} {}{obj}{paid}", crate::systems::grudges::label(world, actor), r.deed.label());
            world.push_event(EventKind::Story, &[feed, actor, r.object.unwrap_or(EntityId::NONE)], text);
            world.stats.current.word.stories += 1;
            ran = ran.saturating_add(1);
        }
        if let Some(s) = world.comp_mut::<Building>(feed).and_then(|b| b.feed.as_mut()) {
            s.stories_today = ran;
        }
    }
}

/// W37 ads: each living corp with `cash ≥ 0.3` pays `ad_rate` a day, split across the
/// licensed Feeds by reach (largest remainder, ties the lower id); the
/// Civic Wire's share goes to the Treasury, a corp's own Feed's share stays
/// home. `Flow::Ads`, taxed as owner revenue, plus the news tax.
fn ads(world: &mut World) {
    let rate = world.config.news.ad_rate;
    if rate <= 0 {
        return;
    }
    let live = feeds(world);
    let reaches: Vec<(EntityId, f32)> =
        live.iter().map(|&f| (f, feed_state(world, f).map_or(0.0, |s| s.reach))).filter(|&(_, r)| r > 0.0).collect();
    let total: f32 = reaches.iter().map(|&(_, r)| r).sum();
    if total <= 0.0 {
        return;
    }
    // The split is the same for every corp: whole coins plus the largest remainders.
    let mut split: Vec<(EntityId, i64, f32)> = reaches
        .iter()
        .map(|&(f, r)| {
            let exact = rate as f32 * r / total;
            (f, exact.floor() as i64, exact - exact.floor())
        })
        .collect();
    let mut left = rate - split.iter().map(|s| s.1).sum::<i64>();
    let mut order: Vec<usize> = (0..split.len()).collect();
    order.sort_by(|&a, &b| split[b].2.total_cmp(&split[a].2).then(split[a].0.cmp(&split[b].0)));
    for i in order {
        if left <= 0 {
            break;
        }
        split[i].1 += 1;
        left -= 1;
    }
    // Plan deviation: ads are a spend for a corp that can afford them
    // (`cash ≥ 0.3`, Spin's bar). Charged to every corp, the city's thin
    // incorporated Holdings went under in weeks: over seeds 42-47 the ads
    // alone took the day-120 Virt nodes from 43.7 to 39.5 and bankruptcies
    // from 36 to 47.
    let payers: Vec<EntityId> = world
        .corps()
        .into_iter()
        .filter(|&c| world.comp::<Corp>(c).is_some_and(|cc| crate::systems::corp_brain::cash_of(cc) >= 0.3))
        .collect();
    for corp in payers {
        for &(f, amount, _) in &split {
            pay_feed(world, corp, f, amount, Flow::Ads);
        }
    }
}

/// W37/W40, inside the word's midnight chain: reach and cover, the Spin
/// plants and buries, the stories, the ads.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    refresh(world);
    let known = rescore_spin(world);
    let plants = spin(world, known);
    run_stories(world, plants);
    ads(world);
}
