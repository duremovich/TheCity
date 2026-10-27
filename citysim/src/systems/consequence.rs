//! The pressure pass phase 5a (docs/PRESSURE_PASS.md § Phase 5): consequence,
//! the non-gang half. What happens next, so a started story is picked up.
//! Under `[consequence]` (absent: none of it):
//!
//! - **Witnesses react.** A civilian who notes a crime (not a petty one) does
//!   one thing that fits him, one weighted draw: reports (today's SawCrime
//!   memory and the walk to the Hall, `w_report x lawfulness`), flees (a
//!   violent crime: the culprit an Enemy, safety gone, so the Flee goal
//!   takes him), gossips (a crime that is a deed: his closest people hear
//!   it at once, `gossip.rs`'s rumour, so the culprit's name moves), or, the
//!   lawless and broke, keeps it to sell: a blackmail. The culprit sees who
//!   saw him on `learn_p` (an edge, which the Hit market's Silence reads).
//! - **Blackmail** is a later demand in person (a `Visit`, `VisitWhy::
//!   Blackmail`): the culprit pays (fear, and the lawful have more to lose)
//!   or refuses; refused, he silences the witness (a beating, an Assault or
//!   a Murder) or the witness goes to the law. A paid demand comes back.
//! - **Hits escalate.** A Hit or Beat a person bought that expires (or
//!   fails) unfilled: the buyer reposts it higher, goes himself (a Hunt, the
//!   cooldown lifted), or lets it go and the grudge cools; his diary shows
//!   the choice (`EventKind::HitEscalated`).
//! - **Jail is a place.** At sentencing the prisoner learns who reported
//!   him (a Betrayed grudge on a civilian reporter). Each day in the cells a
//!   prisoner may take a member cellmate's offer (a join), get a Fixer's
//!   name from a regular (a regular there too), fight a cellmate he
//!   dislikes, or borrow from one (an edge debt); a jailed worker's workday
//!   counts as an absence on phase 1's count (fired at `fire_absences`).
//!   Release writes the changed life: the post lost, the debt, the grudge,
//!   the gang, the name.
//! - **The faith on screen.** Every creed's place (a cult's or the Purists'
//!   chapel, a church) holds a service once a week at its hashed day and
//!   hour; the members (a church: the followers of its district) are called
//!   to it (a `Visit` to the building) and a congregant may bring an
//!   unchurched friend. An attendee hears the preacher, puts coins on the
//!   plate, a guest may convert, a congregant may walk out (a cult's leaver
//!   takes the row's apostasy grudge; a Purist's the preacher's
//!   `svc_heretic_w`): hunted as a heretic past `[hunt] hunt_min`.
//!
//! Statistical mirror: witness reactions happen where crimes are seen (on
//! screen; the off-screen crime rolls have no witnesses, as before); the
//! demands and the services are `Visit`s, which a Statistical visitor is
//! credited at the hour; the hit's choice, the cells and the release are
//! tier-blind passes.

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Crime, Gang, GangMember, Identity, Job, Lod, MemoryEntry, MemoryKind, Needs,
    Personality, Position, RelKind, Sentence,
};
use crate::config::ConsequenceCfg;
use crate::contract::{Broker, Contract, ContractKind, ContractStatus, PostExtra, Posting, Target};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::household::{PendingVisit, VisitWhy};
use crate::systems::ownership::{self, Flow};
use crate::word::{Creed, GrudgeCause, HuntPhase, HuntState, HuntWhy};
use crate::world::World;

const P_REACT: u64 = 0x0c05_0001;
const P_LEARN: u64 = 0x0c05_0002;
const P_HOUR: u64 = 0x0c05_0003;
const P_PAY: u64 = 0x0c05_0004;
const P_SILENCE: u64 = 0x0c05_0005;
const P_HIT: u64 = 0x0c05_0006;
const P_CELL: u64 = 0x0c05_0007;
const P_SVC_DAY: u64 = 0x0c05_0008;
const P_GUEST: u64 = 0x0c05_0009;
const P_CONVERT: u64 = 0x0c05_000a;
const P_WALKOUT: u64 = 0x0c05_000b;
/// How often (ticks) a body arrived at its visit is looked for.
const ARRIVAL_TICKS: u64 = 10;

/// A witness keeping a culprit's crime to sell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blackmail {
    pub witness: EntityId,
    pub culprit: EntityId,
    pub crime: Crime,
    /// The day of the next demand.
    pub next_day: u64,
    /// Demands paid.
    pub demands: u8,
    /// Demands tried and not made (the visit failed), since the last made.
    pub tries: u8,
    /// Coins paid so far.
    pub paid: i64,
}

/// Today's service at a creed's place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Service {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preacher: Option<EntityId>,
    pub creed: String,
    /// A cult's or the Purists' gang (none for a church).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gang: Option<EntityId>,
    /// The creed row (a cult or a church), none for the Purists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u8>,
    pub hour: u8,
    pub called: u16,
    pub came: u16,
}

/// A prisoner's run in the cells, for the release's line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Inside {
    pub since: u64,
    /// The post held going in: (role label, employer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<(String, Option<EntityId>)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporter: Option<EntityId>,
    /// Coins borrowed in the cells: (lender, coins).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owed: Vec<(EntityId, i64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixer: Option<EntityId>,
    pub fights: u8,
    pub missed: u8,
}

/// `World.consequence`: saved while it holds anything.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blackmail: Vec<Blackmail>,
    /// (buyer, target, reposts so far).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reposts: Vec<(EntityId, EntityId, u8)>,
    /// Today's services: place -> the service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<(EntityId, Service)>,
    /// Today's guests at a service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guests: Vec<EntityId>,
    /// Prisoners in the cells: prisoner -> the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inside: Vec<(EntityId, Inside)>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.blackmail.is_empty()
            && self.reposts.is_empty()
            && self.services.is_empty()
            && self.guests.is_empty()
            && self.inside.is_empty()
    }

    fn inside_mut(&mut self, id: EntityId) -> Option<&mut Inside> {
        self.inside.iter_mut().find(|(p, _)| *p == id).map(|(_, x)| x)
    }
}

pub fn cfg(world: &World) -> Option<&ConsequenceCfg> {
    world.config.consequence.as_ref()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

#[derive(Clone, Copy)]
struct Traits {
    lawful: f32,
    greed: f32,
    pride: f32,
    social: f32,
    courage: f32,
    loyalty: f32,
}

fn traits(world: &World, id: EntityId) -> Traits {
    world.comp::<Personality>(id).map_or(
        Traits { lawful: 0.5, greed: 0.5, pride: 0.5, social: 0.5, courage: 0.5, loyalty: 0.5 },
        |p| Traits {
            lawful: p.lawfulness,
            greed: p.greed,
            pride: p.pride,
            social: p.sociability,
            courage: p.courage,
            loyalty: p.loyalty,
        },
    )
}

fn living(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
}

fn person(world: &World, id: EntityId) -> bool {
    world.has::<Identity>(id) && world.has::<Brain>(id) && living(world, id)
}

fn free(world: &World, id: EntityId) -> bool {
    person(world, id) && crate::systems::contracts::free_adult(world, id)
}

/// A gang's name (`World::name_of` reads people and buildings).
fn gang_name(world: &World, g: EntityId) -> String {
    world.comp::<Gang>(g).map_or_else(|| world.name_of(g), |x| x.name.clone())
}

/// (he, him, his) for `id`.
fn pro(world: &World, id: EntityId) -> (&'static str, &'static str, &'static str) {
    match world.comp::<Identity>(id).map(|i| i.sex) {
        Some(crate::components::Sex::Female) => ("she", "her", "her"),
        _ => ("he", "him", "his"),
    }
}

fn place_of(world: &World, id: EntityId) -> (Option<EntityId>, String) {
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    (here, here.map_or_else(|| "on the street".to_string(), |b| format!("at {}", world.name_of(b))))
}

/// Draw one of `w` (weights, all >= 0) on `u` in `0..1`; `None` when all are 0.
fn pick(w: &[f32], u: f32) -> Option<usize> {
    let total: f32 = w.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = u * total;
    for (i, &v) in w.iter().enumerate() {
        if x < v {
            return Some(i);
        }
        x -= v;
    }
    w.iter().rposition(|&v| v > 0.0)
}

/// A visit due today within `from..to`; `false` with the visits off (the
/// caller resolves at once) or the visitor busy.
fn schedule(world: &mut World, who: EntityId, kin: EntityId, why: VisitWhy, from: u8, to: u8) -> bool {
    if !crate::systems::household::on(world) || world.visits.contains_key(&who) {
        return false;
    }
    let span = to.saturating_sub(from).max(1);
    let u = hash(world, P_HOUR, who, world.day() ^ key(kin));
    let hour = from + ((u * f32::from(span)) as u8).min(span - 1);
    world.visits.insert(who, PendingVisit { kin, why, hour });
    true
}

/// The Visit's urgency for the two calls (0 without the section).
pub fn urgency(world: &World, why: VisitWhy) -> f32 {
    cfg(world).map_or(0.0, |c| match why {
        VisitWhy::Blackmail => c.bm_urgency,
        _ => c.svc_urgency,
    })
}

/// Every `ARRIVAL_TICKS` (from `living::run`): a body come to its service's
/// place, or to the culprit's building, is credited at once (the Visit's
/// Rest runs on; a Sleep that cut the Rest lost the credit). The pending
/// visits only (a handful), never a scan of the city.
pub fn arrivals(world: &mut World) {
    if cfg(world).is_none() || !world.tick.is_multiple_of(ARRIVAL_TICKS) || world.visits.is_empty() {
        return;
    }
    let tod = world.tick_of_day();
    let due: Vec<(EntityId, PendingVisit)> = world
        .visits
        .iter()
        .filter(|(_, v)| matches!(v.why, VisitWhy::Service | VisitWhy::Blackmail))
        .filter(|(_, v)| u16::from(v.hour) * crate::time::TICKS_PER_HOUR as u16 <= tod)
        .map(|(&id, &v)| (id, v))
        .collect();
    for (id, v) in due {
        if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
            continue;
        }
        let here = world.comp::<Position>(id).and_then(|p| p.building);
        let there = match v.why {
            VisitWhy::Service => Some(v.kin),
            _ => world.comp::<Position>(v.kin).and_then(|p| p.building),
        };
        if here.is_none() || here != there {
            continue;
        }
        world.visits.remove(&id);
        match v.why {
            VisitWhy::Service => attended(world, id, v.kin, true),
            _ => blackmailed(world, id, v.kin, true),
        }
    }
}

/// Midnight (LivingEarly, after the visits due are dealt): the demands due,
/// today's services called, the cells' day.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    blackmail_daily(world, &c);
    services_daily(world, &c);
    cells_daily(world, &c);
    let reposts = std::mem::take(&mut world.consequence.reposts);
    world.consequence.reposts = reposts.into_iter().filter(|&(b, t, _)| living(world, b) && living(world, t)).collect();
}

// ---------------------------------------------------------------------------
// Witnesses
// ---------------------------------------------------------------------------

/// `law::raise_crime_mult`: civilian `w` noted `actor`'s `crime` (the
/// Witness event written). One reaction; the culprit may see him.
pub fn witnessed(world: &mut World, w: EntityId, actor: EntityId, crime: Crime, object: Option<EntityId>) {
    let Some(c) = cfg(world).cloned() else { return };
    if crate::systems::law::is_petty(crime) || !person(world, actor) || !person(world, w) {
        return;
    }
    if !crate::systems::demography::is_adult(world, w) || object == Some(w) {
        return;
    }
    // His own crew's work is not his to sell or tell.
    if world.gang_of(w).is_some() && world.gang_of(w) == world.gang_of(actor) {
        return;
    }
    let t = traits(world, w);
    let violent = matches!(crime, Crime::Murder | Crime::Assault | Crime::Abduction | Crime::Manslaughter);
    let deed = crate::systems::gossip::deed_of_crime(crime);
    let bm_ok = t.lawful <= c.bm_lawful_max
        && world.purse(Some(w)) <= c.bm_broke_coins
        && world.consequence.blackmail.len() < c.bm_max_open
        && !world.consequence.blackmail.iter().any(|b| b.witness == w || (b.culprit == w && b.witness == actor));
    let weights = [
        c.w_report * t.lawful,
        if violent { c.w_flee * (1.0 - t.courage) } else { 0.0 },
        if deed.is_some() { c.w_gossip * t.social } else { 0.0 },
        if bm_ok { c.w_blackmail * (1.0 - t.lawful) * (0.5 + t.greed) } else { 0.0 },
    ];
    let u = hash(world, P_REACT, w, world.tick ^ (key(actor) << 7));
    let (wn, an) = (world.name_of(w), world.name_of(actor));
    let label = crime.label();
    match pick(&weights, u) {
        Some(1) => {
            crate::systems::social::make_enemy(world, w, actor, c.flee_affinity);
            if let Some(n) = world.comp_mut::<Needs>(w) {
                n.safety = 0.0;
            }
            let text = format!("{wn} saw {an}'s {label} and ran");
            world.push_event(EventKind::WitnessReacted, &[w, actor], text);
            ownership::jobs2_row(world).wr_flee += 1;
        }
        Some(2) => {
            if let Some(d) = deed {
                let told = tell(world, &c, w, actor, d, object, crime);
                if told > 0 {
                    let his = pro(world, w).2;
                    let text = format!("{wn} told {told} of {his} people what {an} did (the {label})");
                    world.push_event(EventKind::WitnessReacted, &[w, actor], text);
                    ownership::jobs2_row(world).wr_gossip += 1;
                }
            }
        }
        Some(3) => {
            let next_day = world.day() + u64::from(c.bm_wait_days);
            world.consequence.blackmail.push(Blackmail {
                witness: w,
                culprit: actor,
                crime,
                next_day,
                demands: 0,
                tries: 0,
                paid: 0,
            });
            let him = pro(world, w).1;
            let text = format!("{wn} saw {an}'s {label} and kept it to {him}self: it is worth something");
            world.push_event(EventKind::WitnessReacted, &[w, actor], text);
            ownership::jobs2_row(world).wr_blackmail += 1;
        }
        _ => ownership::jobs2_row(world).wr_report += 1,
    }
    // The culprit sees who saw him (the pair's edge sours; the Silence pass reads it).
    // The pressure pass R4 (OPEN-219): no line of its own (781 a month at
    // 20k, one a witness a blow); the soured edge and the counter carry it.
    if hash(world, P_LEARN, actor, world.tick ^ key(w)) < c.learn_p {
        crate::systems::social::adjust(world, actor, w, -0.1, 0.0);
        ownership::jobs2_row(world).wr_learned += 1;
    }
}

/// The gossip: `w`'s closest people (Friend, Family, Spouse, Parent, best
/// affinity first) hear the deed at once, hops 1. Returns how many heard.
fn tell(
    world: &mut World,
    c: &ConsequenceCfg,
    w: EntityId,
    actor: EntityId,
    deed: crate::word::Deed,
    object: Option<EntityId>,
    crime: Crime,
) -> u32 {
    let mut near: Vec<(f32, EntityId)> = world
        .neighbours(w)
        .filter(|&o| o != actor && Some(o) != object && world.has::<Brain>(o))
        .filter_map(|o| {
            world
                .edge(w, o)
                .filter(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse | RelKind::Parent))
                .map(|e| (e.affinity, o))
        })
        .collect();
    near.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let hop = world.config.gossip.hop_salience;
    let salience = crate::systems::law::crime_salience(crime);
    let now = world.tick;
    let mut told = 0;
    for (_, o) in near.into_iter().take(usize::from(c.gossip_tell)) {
        let trust = world.edge(o, w).map_or(0.0, |e| e.trust);
        let entry = MemoryEntry {
            subject: Some(actor),
            salience: salience * hop,
            valence: -salience * hop,
            second_hand: true,
            deed: Some(deed),
            object,
            hops: 1,
            conf: 0.5 + 0.5 * trust,
            ..MemoryEntry::blank(MemoryKind::Rumour, now)
        };
        let out = crate::systems::memory::hear_entry(world, o, entry);
        if matches!(
            out,
            crate::systems::memory::HeardInsert::Inserted | crate::systems::memory::HeardInsert::Contradicted
        ) {
            told += 1;
            world.stats.current.word.rumours_heard += 1;
            if world.edge(o, actor).is_some() {
                let sev = world.config.gossip.deed_sev.get(deed);
                crate::systems::social::adjust(world, o, actor, -0.1 * sev, 0.0);
            }
        }
    }
    told
}

/// ReportCrime's reader: `w` is keeping `culprit`'s crime to sell.
pub fn keeps_quiet(world: &World, w: EntityId, culprit: EntityId) -> bool {
    world.consequence.blackmail.iter().any(|b| b.witness == w && b.culprit == culprit)
}

// ---------------------------------------------------------------------------
// Blackmail
// ---------------------------------------------------------------------------

fn blackmail_daily(world: &mut World, c: &ConsequenceCfg) {
    let today = world.day();
    let mut i = 0;
    while i < world.consequence.blackmail.len() {
        let b = world.consequence.blackmail[i].clone();
        let done = b.demands >= c.bm_max_demands;
        if done || b.tries >= c.bm_tries || !living(world, b.witness) || !living(world, b.culprit) {
            world.consequence.blackmail.remove(i);
            if done && living(world, b.witness) && living(world, b.culprit) {
                let text = format!(
                    "{} keeps {}'s secret now: {}c paid for the {} in all",
                    world.name_of(b.witness),
                    world.name_of(b.culprit),
                    b.paid,
                    b.crime.label()
                );
                world.push_event(EventKind::Blackmail, &[b.witness, b.culprit], text);
            }
            continue;
        }
        let ready = b.next_day <= today
            && free(world, b.witness)
            && free(world, b.culprit)
            && !world.visits.contains_key(&b.witness);
        if ready {
            world.consequence.blackmail[i].tries += 1;
            if !schedule(world, b.witness, b.culprit, VisitWhy::Blackmail, c.bm_hour_from, c.bm_hour_to)
                && !crate::systems::household::on(world)
            {
                blackmailed(world, b.witness, b.culprit, false);
            }
        }
        i += 1;
    }
}

/// The demand (a `Visit` arrived, or a Statistical one at its hour).
pub fn blackmailed(world: &mut World, w: EntityId, culprit: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(i) = world.consequence.blackmail.iter().position(|b| b.witness == w && b.culprit == culprit) else {
        return;
    };
    let b = world.consequence.blackmail[i].clone();
    let purse = world.purse(Some(culprit)).max(0);
    let demand = ((purse as f32 * c.bm_demand_share).round() as i64).clamp(c.bm_demand_min, c.bm_demand_max);
    let (here, place) = place_of(world, culprit);
    let (wn, cn) = (world.name_of(w), world.name_of(culprit));
    let how = if walked { format!("found {cn}") } else { format!("caught {cn} (off screen)") };
    let again = if b.demands > 0 { " again" } else { "" };
    let (he, him, _) = pro(world, w);
    let head =
        format!("{wn} {how} {place}{again}: {he} saw the {} and wants {demand}c to keep it quiet", b.crime.label());
    let mut actors = vec![w, culprit];
    actors.extend(here);
    ownership::jobs2_row(world).bm_demands += 1;
    let t = traits(world, culprit);
    let p_pay = (c.bm_pay_base + c.bm_pay_fear * (1.0 - t.courage) + c.bm_pay_lawful * t.lawful
        - c.bm_pay_pride * t.pride)
        .clamp(0.0, 1.0);
    let draw = world.day() ^ key(w);
    if purse >= demand && hash(world, P_PAY, culprit, draw) < p_pay {
        let paid = ownership::pay(world, Some(culprit), Some(w), demand, Flow::Bribe);
        let today = world.day();
        if let Some(x) = world.consequence.blackmail.get_mut(i) {
            x.demands += 1;
            x.tries = 0;
            x.paid += paid;
            x.next_day = today + u64::from(c.bm_interval_days);
        }
        crate::systems::grudges::add(world, culprit, w, GrudgeCause::Robbed, 0.3, 0);
        world.push_event(EventKind::Blackmail, &actors, format!("{head}: {cn} paid {paid}c"));
        let row = ownership::jobs2_row(world);
        row.bm_paid += 1;
        row.bm_coins += paid;
        return;
    }
    world.consequence.blackmail.remove(i);
    let why = if purse < demand { "had nothing like it".to_string() } else { format!("told {him} where to go") };
    let silence = hash(world, P_SILENCE, culprit, draw) < c.bm_silence_w * (1.0 - t.lawful) * (0.5 + t.courage);
    if silence {
        let tile = world.comp::<Position>(w).map_or_else(Default::default, |p| p.tile);
        let mods = crate::systems::law::FightMods { kill_mult: c.bm_kill_mult, ..Default::default() };
        let (winner, loser, died) = crate::systems::law::resolve_fight_mods(world, culprit, w, mods);
        let end = if died {
            format!(", {} died", world.name_of(loser))
        } else if winner == w {
            format!("; {wn} won and went to the law")
        } else {
            format!("; {cn} won and {wn} will keep quiet now")
        };
        world.push_event(EventKind::Blackmail, &actors, format!("{head}: {cn} {why} and went for {him}{end}"));
        ownership::jobs2_row(world).bm_silenced += 1;
        if living(world, culprit) {
            let murder = died && loser == w;
            let kind = if murder { Crime::Murder } else { Crime::Assault };
            if murder {
                world.push_event(EventKind::Murder, &[culprit, w], format!("{cn} killed {wn}"));
            }
            crate::systems::law::raise_crime_on(world, culprit, (!murder).then_some(w), Some(w), kind, tile);
        }
        if living(world, w) {
            crate::systems::grudges::add(world, w, culprit, GrudgeCause::Assaulted, 0.5, 0);
            if winner == w && living(world, culprit) {
                crate::systems::law::file_report(world, b.crime, culprit, Some(w));
            }
        }
        return;
    }
    crate::systems::law::file_report(world, b.crime, culprit, Some(w));
    crate::systems::grudges::add(world, culprit, w, GrudgeCause::Betrayed, c.snitch_grudge, 0);
    world.push_event(EventKind::Blackmail, &actors, format!("{head}: {cn} {why}; {wn} went to the law"));
    ownership::jobs2_row(world).bm_reported += 1;
}

// ---------------------------------------------------------------------------
// Hits
// ---------------------------------------------------------------------------

/// `contracts::set_status`: an organic Hit or Beat a person bought closed
/// `status` (Expired or Failed) unfilled. The buyer's choice.
pub fn hit_unfilled(world: &mut World, ct: &Contract, status: ContractStatus) {
    let Some(c) = cfg(world).cloned() else { return };
    if !matches!(ct.kind, ContractKind::Hit | ContractKind::Beat) {
        return;
    }
    let (Some(b), Target::Agent(t)) = (ct.buyer, ct.target) else { return };
    if ct.agent != Some(b) || !free(world, b) || !crate::systems::hunt::target_ok(world, t) {
        return;
    }
    let grudge = crate::systems::grudges::grudge_on(world, b, t).filter(|g| g.settled.is_none());
    let g = grudge.as_ref().map_or(0.5, |x| x.weight);
    let n = world.consequence.reposts.iter().find(|&&(x, y, _)| x == b && y == t).map_or(0, |x| x.2);
    let price = ((ct.price as f32 * c.hit_repost_mult).ceil() as i64).max(ct.price + 1);
    let deposit = crate::systems::hitmarket::deposit_of(world, price);
    let repost_ok = n < c.hit_repost_max && world.purse(Some(b)) >= deposit && crate::systems::hitmarket::room(world);
    let self_ok = grudge.is_some()
        && g >= c.hit_self_min
        && crate::systems::hunt::hunter_ok(world, b)
        && !world.hunts.contains_key(&b)
        && world.hunts.len() < world.config.hunt.max_hunts;
    let tr = traits(world, b);
    let weights = [
        if repost_ok { c.hit_w_repost * (0.5 + g) } else { 0.0 },
        if self_ok { c.hit_w_self * tr.courage * g } else { 0.0 },
        c.hit_w_giveup * (1.5 - g).max(0.0),
    ];
    let (bn, tn) = (world.name_of(b), world.name_of(t));
    let kind = ct.kind.label();
    let lapse = if status == ContractStatus::Failed { "failed" } else { "found no taker" };
    match pick(&weights, hash(world, P_HIT, b, ct.id)) {
        Some(0) => {
            let broker = ct
                .broker
                .filter(|&f| crate::systems::contracts::fixer_open(world, f))
                .or_else(|| crate::systems::contracts::nearest_fixer(world, b));
            let posting = Posting {
                buyer: Some(b),
                agent: Some(b),
                kind: ct.kind,
                target: ct.target,
                broker,
                deadline_days: world.config.contracts.deadline_days.get(ct.kind),
                origin: ct.origin,
                price: Some(price),
            };
            let extra = PostExtra {
                deposit: (deposit < price).then_some(deposit),
                motive: ct.motive,
                why: Some(format!("raised from {}c", ct.price)),
            };
            if crate::systems::contracts::post_with(world, posting, extra).is_ok() {
                match world.consequence.reposts.iter_mut().find(|x| x.0 == b && x.1 == t) {
                    Some(x) => x.2 += 1,
                    None => world.consequence.reposts.push((b, t, 1)),
                }
                let at = broker.map(|f| format!(" to {}", world.name_of(f))).unwrap_or_default();
                let text = format!(
                    "{bn} went back{at} and raised the price on {tn}: the {kind} {lapse} at {}c, now {price}c",
                    ct.price
                );
                let actors = [b, t, broker.unwrap_or(EntityId::NONE)];
                world.push_event(EventKind::HitEscalated, &actors, text);
                ownership::jobs2_row(world).hit_reposted += 1;
                return;
            }
            give_up(world, &c, b, t, kind, lapse, ct.price);
        }
        Some(1) => go_himself(world, b, t, g, kind, lapse),
        _ => give_up(world, &c, b, t, kind, lapse, ct.price),
    }
}

fn give_up(world: &mut World, c: &ConsequenceCfg, b: EntityId, t: EntityId, kind: &str, lapse: &str, price: i64) {
    let mut cooled = None;
    if let Some(g) = world.comp_mut::<crate::word::Grudges>(b) {
        if let Some(x) = g.list.iter_mut().find(|x| x.target == t && x.settled.is_none()) {
            let was = x.weight;
            x.weight *= c.hit_giveup_decay;
            cooled = Some((was, x.weight));
        }
    }
    let (bn, tn) = (world.name_of(b), world.name_of(t));
    let tail = cooled.map_or_else(String::new, |(a, z)| format!("; the grudge cooled {a:.2} -> {z:.2}"));
    let text = format!("{bn} let it go: the {kind} on {tn} {lapse} at {price}c{tail}");
    world.push_event(EventKind::HitEscalated, &[b, t], text);
    ownership::jobs2_row(world).hit_giveup += 1;
}

fn go_himself(world: &mut World, b: EntityId, t: EntityId, weight: f32, kind: &str, lapse: &str) {
    if world.comp::<Brain>(b).is_some_and(|x| x.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, b, Lod::Coarse);
    }
    let chain = crate::systems::grudges::grudge_on(world, b, t).map_or(0, |g| g.chain);
    let gap = crate::systems::hunt::might_gap(world, b, t);
    let now = world.tick;
    world.hunts.insert(
        b,
        HuntState {
            target: t,
            grudge_target: t,
            chain,
            since: now,
            phase: HuntPhase::Ask,
            venue: None,
            intel: None,
            stakeout_until: None,
            deceived: false,
            why: HuntWhy::Goal,
            liar: None,
            weight,
            gap,
        },
    );
    if let Some(x) = world.comp_mut::<Brain>(b) {
        x.cooldowns.remove(&crate::components::GoalKind::Hunt);
    }
    world.stats.current.word.hunts += 1;
    let (bn, tn) = (world.name_of(b), world.name_of(t));
    let (he, him, _) = pro(world, b);
    let text = format!("{bn} stopped waiting: the {kind} on {tn} {lapse}, so {he} went after {tn} {him}self");
    world.push_event(EventKind::HitEscalated, &[b, t], text);
    world.push_event(EventKind::HuntStarted, &[b, t], format!("{bn} went looking for {tn}"));
    crate::systems::hunt::reindex(world);
    ownership::jobs2_row(world).hit_self += 1;
}

// ---------------------------------------------------------------------------
// Jail
// ---------------------------------------------------------------------------

/// `law::sentence`, before the job is touched: the run begins; the
/// prisoner learns who reported him (a civilian: a Betrayed grudge).
pub fn on_sentenced(world: &mut World, who: EntityId, crime: Crime) {
    let Some(c) = cfg(world).cloned() else { return };
    if !person(world, who) {
        return;
    }
    let reporter = world
        .crime_reports()
        .iter()
        .rev()
        .filter(|r| r.suspect == who)
        .filter_map(|r| r.witness)
        .find(|&w| w != who && person(world, w) && !crate::systems::law::is_guard(world, w));
    let job = world.comp::<Job>(who).filter(|j| j.employer.is_some()).map(|j| (j.role.label().to_string(), j.employer));
    let day = world.day();
    world.consequence.inside.retain(|(p, _)| *p != who);
    world.consequence.inside.push((who, Inside { since: day, job, reporter, ..Default::default() }));
    if let Some(r) = reporter {
        crate::systems::grudges::add(world, who, r, GrudgeCause::Betrayed, c.snitch_grudge, 0);
        let him = pro(world, who).1;
        let text = format!(
            "{} learned who put {him} inside: {} reported {him} for the {}",
            world.name_of(who),
            world.name_of(r),
            crime.label()
        );
        world.push_event(EventKind::Cells, &[who, r], text);
        ownership::jobs2_row(world).snitch_grudges += 1;
    }
}

/// `law::release`: the life he comes out to.
pub fn on_released(world: &mut World, who: EntityId) {
    if cfg(world).is_none() {
        return;
    }
    let Some(i) = world.consequence.inside.iter().position(|(p, _)| *p == who) else { return };
    let (_, x) = world.consequence.inside.remove(i);
    let mut parts: Vec<String> = Vec::new();
    if let Some((role, employer)) = &x.job {
        let still = world.comp::<Job>(who).is_some_and(|j| j.employer == *employer);
        if !still {
            let at = employer.map(|e| format!(" at {}", world.name_of(e))).unwrap_or_default();
            parts.push(format!("the post as {role}{at} gone"));
        }
    }
    for &(l, n) in &x.owed {
        let left = crate::systems::underwater::owed(world, who, l);
        if left > 0 {
            parts.push(format!("{left}c owed to {} (of {n}c borrowed inside)", world.name_of(l)));
        }
    }
    if let Some(r) = x.reporter.filter(|&r| crate::systems::grudges::holds(world, who, r, 0.0)) {
        parts.push(format!("a grudge on {}, who reported {}", world.name_of(r), pro(world, who).1));
    }
    if let Some(g) = x.joined.filter(|&g| world.gang_of(who) == Some(g)) {
        parts.push(format!("one of {} now", gang_name(world, g)));
    }
    if let Some(f) = x.fixer {
        parts.push(format!("a name at {}", world.name_of(f)));
    }
    if x.fights > 0 {
        let n = x.fights;
        parts.push(format!("{n} fight{} in the cells behind {}", if n == 1 { "" } else { "s" }, pro(world, who).1));
    }
    if parts.is_empty() {
        return;
    }
    let text = format!("{} came out to a changed life: {}", world.name_of(who), parts.join("; "));
    world.push_event(EventKind::Cells, &[who], text);
    ownership::jobs2_row(world).released_changed += 1;
}

/// A Fixer `m` knows: one he owns, works at, or is a regular of.
fn fixer_known(world: &World, m: EntityId) -> Option<EntityId> {
    let mine = world.comp::<Job>(m).and_then(|j| j.employer);
    world.buildings_of_kind(BuildingKind::Fixer).iter().copied().find(|&f| {
        world.owner_of(f) == Some(m)
            || mine == Some(f)
            || world.comp::<Broker>(f).is_some_and(|k| k.regulars.contains_key(&m))
    })
}

fn regular_anywhere(world: &World, p: EntityId) -> bool {
    world
        .buildings_of_kind(BuildingKind::Fixer)
        .iter()
        .any(|&f| world.comp::<Broker>(f).is_some_and(|k| k.regulars.contains_key(&p)))
}

fn cells_daily(world: &mut World, c: &ConsequenceCfg) {
    let today = world.day();
    // Prisoners gone (dead, broken out) drop their run.
    let runs: Vec<EntityId> = world.consequence.inside.iter().map(|x| x.0).collect();
    for p in runs {
        if !world.has::<Sentence>(p) || !living(world, p) {
            world.consequence.inside.retain(|x| x.0 != p);
        }
    }
    let mut cells: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for &p in world.sentenced() {
        if !person(world, p) {
            continue;
        }
        if let Some(b) = world.comp::<Position>(p).and_then(|x| x.building) {
            cells.entry(b).or_default().push(p);
        }
    }
    for (jail, prisoners) in cells {
        let mut events = 0u16;
        for &p in &prisoners {
            if !living(world, p) || !world.has::<Sentence>(p) {
                continue;
            }
            jail_absence(world, p);
            if events >= c.cell_events_max || prisoners.len() < 2 {
                continue;
            }
            if cell_day(world, c, jail, p, &prisoners, today) {
                events += 1;
            }
        }
    }
}

/// A jailed worker's workday: an absence on phase 1's count.
fn jail_absence(world: &mut World, p: EntityId) {
    let Some(job) = world.comp::<Job>(p).cloned() else { return };
    let key = job.next_shift_key(world.tick);
    if !crate::exec::routine::workday_of(world, p, &job, key) {
        return;
    }
    let Some((count, window, fired)) = crate::systems::misery::jail_absence(world, p) else { return };
    let mut actors = vec![p];
    actors.extend(job.employer);
    let text = format!(
        "{} missed the shift as {} in the cells (absence {count} in {window} days{})",
        world.name_of(p),
        job.role.label(),
        if fired { ": dismissed" } else { "" }
    );
    world.push_event(EventKind::Cells, &actors, text);
    if let Some(x) = world.consequence.inside_mut(p) {
        x.missed = x.missed.saturating_add(1);
    }
    let row = ownership::jobs2_row(world);
    row.jail_absences += 1;
    if fired {
        row.jail_fired += 1;
    }
}

/// One prisoner's day among his cellmates: at most one thing happens.
fn cell_day(world: &mut World, c: &ConsequenceCfg, jail: EntityId, p: EntityId, mates: &[EntityId], day: u64) -> bool {
    let t = traits(world, p);
    let others: Vec<EntityId> = mates.iter().copied().filter(|&m| m != p && living(world, m)).collect();
    if others.is_empty() {
        return false;
    }
    let pn = world.name_of(p);
    let roll = |world: &World, salt: u64| hash(world, P_CELL, p, day ^ (salt << 48));
    // A fight with a cellmate he dislikes.
    let dislike = |world: &World, m: EntityId| -> Option<&'static str> {
        if crate::systems::grudges::holds(world, p, m, 0.0) {
            return Some("an old grudge");
        }
        if let (Some(a), Some(b)) = (world.gang_of(p), world.gang_of(m)) {
            if a != b {
                return Some("the other colours");
            }
        }
        world.edge(p, m).filter(|e| e.affinity < c.cell_fight_affinity).map(|_| "bad blood")
    };
    if roll(world, 1) < c.cell_fight_p * (0.5 + t.pride) {
        if let Some((m, why)) = others.iter().find_map(|&m| dislike(world, m).map(|w| (m, w))) {
            let mods = crate::systems::law::FightMods { kill_mult: c.cell_kill_mult, ..Default::default() };
            let (winner, loser, died) = crate::systems::law::resolve_fight_mods(world, p, m, mods);
            if living(world, loser) {
                crate::systems::grudges::add(world, loser, winner, GrudgeCause::Assaulted, 0.4, 0);
            }
            let end = if died { format!(", {} died", world.name_of(loser)) } else { String::new() };
            let text = format!(
                "{pn} went for {} in the cells of {} ({why}); {} won{end}",
                world.name_of(m),
                world.name_of(jail),
                world.name_of(winner)
            );
            world.push_event(EventKind::Cells, &[p, m, jail], text);
            for x in [p, m] {
                if let Some(r) = world.consequence.inside_mut(x) {
                    r.fights = r.fights.saturating_add(1);
                }
            }
            ownership::jobs2_row(world).cell_fights += 1;
            return true;
        }
    }
    // A member cellmate's offer.
    if world.gang_of(p).is_none() && !crate::systems::founding::is_exec(world, p) {
        let m = crate::systems::misery::misery(world, p);
        if roll(world, 2) < c.cell_recruit_p * (1.0 - t.lawful) * (0.5 + m) {
            let max = world.config.gangs.max_members;
            let now = world.tick;
            let offer = others.iter().copied().find_map(|o| {
                let g = world.comp::<GangMember>(o)?.gang;
                let gang = world.comp::<Gang>(g)?;
                let open =
                    gang.members.len() < max && !gang.is_sacked(now) && crate::systems::creeds::tolerated(world, g, p);
                open.then_some((o, g))
            });
            if let Some((o, g)) = offer {
                crate::systems::gang::enlist(world, p, g);
                let text = format!("{} brought {pn} into {} in the cells", world.name_of(o), gang_name(world, g));
                world.push_event(EventKind::Cells, &[p, o, g], text);
                if let Some(r) = world.consequence.inside_mut(p) {
                    r.joined = Some(g);
                }
                ownership::jobs2_row(world).cell_recruits += 1;
                return true;
            }
        }
    }
    // A Fixer's name.
    if roll(world, 3) < c.cell_contact_p && !regular_anywhere(world, p) {
        if let Some((o, f)) = others.iter().copied().find_map(|o| fixer_known(world, o).map(|f| (o, f))) {
            let now = world.tick;
            if let Some(k) = world.comp_mut::<Broker>(f) {
                k.regulars.insert(p, now);
            }
            let text = format!("{} gave {pn} a name in the cells: ask at {}", world.name_of(o), world.name_of(f));
            world.push_event(EventKind::Cells, &[p, o, f], text);
            if let Some(r) = world.consequence.inside_mut(p) {
                r.fixer = Some(f);
            }
            ownership::jobs2_row(world).cell_contacts += 1;
            return true;
        }
    }
    // A loan from a cellmate.
    if world.purse(Some(p)) < c.cell_loan_broke && roll(world, 4) < c.cell_loan_p {
        let lender = others.iter().copied().find(|&o| {
            world.purse(Some(o)) >= 2 * c.cell_loan_coins && world.edge(p, o).is_none_or(|e| e.affinity >= 0.0)
        });
        if let Some(o) = lender {
            let lent = ownership::pay(world, Some(o), Some(p), c.cell_loan_coins, Flow::Finance);
            if lent > 0 {
                crate::systems::underwater::owe(world, p, o, lent);
                let text = format!("{} lent {pn} {lent}c in the cells", world.name_of(o));
                world.push_event(EventKind::Cells, &[p, o], text);
                if let Some(r) = world.consequence.inside_mut(p) {
                    r.owed.push((o, lent));
                }
                ownership::jobs2_row(world).cell_loans += 1;
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// The faith on screen
// ---------------------------------------------------------------------------

/// The place's service day this week (a hash of the place, a weekday) and hour.
fn service_slot(world: &World, c: &ConsequenceCfg, place: EntityId) -> (u64, u8) {
    let u = hash(world, P_SVC_DAY, place, 0);
    let wd = ((u * 7.0) as u64).min(6);
    let span = c.svc_hour_to.saturating_sub(c.svc_hour_from).max(1);
    let v = hash(world, P_SVC_DAY, place, 1);
    (wd, c.svc_hour_from + ((v * f32::from(span)) as u8).min(span - 1))
}

fn services_daily(world: &mut World, c: &ConsequenceCfg) {
    world.consequence.services.clear();
    world.consequence.guests.clear();
    if !crate::systems::household::on(world) {
        return;
    }
    let today = world.day();
    // The creeds' gangs (a cult row, or the Purists) at their chapels.
    for g in world.gang_list().to_vec() {
        let Some(gang) = world.comp::<Gang>(g) else { continue };
        let Some(creed) = gang.creed else { continue };
        let place = gang.hideout;
        if place == EntityId::NONE
            || gang.is_sacked(world.tick)
            || world.comp::<Building>(place).is_none_or(|b| b.demolished || b.derelict)
        {
            continue;
        }
        let (wd, hour) = service_slot(world, c, place);
        if today % 7 != wd {
            continue;
        }
        let members: Vec<EntityId> = gang.members.iter().copied().filter(|&m| free(world, m)).collect();
        if members.is_empty() {
            continue;
        }
        let preacher = gang.leader.filter(|&l| free(world, l)).or_else(|| members.first().copied());
        let (label, row) = match creed {
            Creed::Config(i) => (crate::systems::faith::label(world, i), Some(i)),
            Creed::Purist => (gang.name.clone(), None),
        };
        call(world, c, place, preacher, label, Some(g), row, hour, &members);
    }
    // The churches: the followers of their district.
    if crate::systems::faith::on(world) {
        let churches: Vec<(EntityId, u8)> = world.jobs2.faith.churches.iter().map(|(&b, &i)| (b, i)).collect();
        for (b, i) in churches {
            if world.comp::<Building>(b).is_none_or(|x| x.demolished) {
                continue;
            }
            let (wd, hour) = service_slot(world, c, b);
            if today % 7 != wd {
                continue;
            }
            let d = world.district_of_building(b);
            let flock: Vec<EntityId> = world
                .jobs2
                .faith
                .followers
                .iter()
                .filter(|(_, &x)| x == i)
                .map(|(&a, _)| a)
                .filter(|&a| crate::systems::gossip::home_district(world, a) == Some(d) && free(world, a))
                .collect();
            if flock.is_empty() {
                continue;
            }
            // The eldest of the flock not called elsewhere today preaches (a
            // lay preacher; ties the lower id): a district with two churches
            // has two preachers.
            let preacher = flock
                .iter()
                .copied()
                .filter(|a| !world.visits.contains_key(a))
                .max_by_key(|&a| (world.comp::<Identity>(a).map_or(0, |x| x.age_years()), std::cmp::Reverse(a)));
            let label = crate::systems::faith::label(world, i);
            call(world, c, b, preacher, label, None, Some(i), hour, &flock);
        }
    }
}

/// Call the congregation (and the preacher, first) to `place` for today.
#[allow(clippy::too_many_arguments)]
fn call(
    world: &mut World,
    c: &ConsequenceCfg,
    place: EntityId,
    preacher: Option<EntityId>,
    creed: String,
    gang: Option<EntityId>,
    row: Option<u8>,
    hour: u8,
    flock: &[EntityId],
) {
    let visit_hour = hour.saturating_sub(c.svc_lead_hours);
    let day = world.day();
    let mut called = 0u16;
    let mut order: Vec<EntityId> = Vec::with_capacity(flock.len());
    order.extend(preacher);
    order.extend(flock.iter().copied().filter(|&a| Some(a) != preacher));
    let door = world.comp::<Building>(place).map(|b| b.door);
    for a in order {
        if usize::from(called) >= c.svc_cap {
            break;
        }
        if world.visits.contains_key(&a) {
            continue;
        }
        // Past the reach the walk would eat the evening (the preacher goes anyway).
        let far = |world: &World, a: EntityId| {
            let from = crate::systems::distance::set_out(world, a);
            from.zip(door).is_some_and(|(f, d)| crate::systems::distance::travel(world, f, d) > c.svc_reach_ticks)
        };
        if Some(a) != preacher && far(world, a) {
            continue;
        }
        world.visits.insert(a, PendingVisit { kin: place, why: VisitWhy::Service, hour: visit_hour });
        called += 1;
        // A congregant brings an unchurched friend.
        if Some(a) != preacher && hash(world, P_GUEST, a, day) < c.svc_guest_p && usize::from(called) < c.svc_cap {
            let mut near: Vec<(f32, EntityId)> = world
                .neighbours(a)
                .filter_map(|o| {
                    world
                        .edge(a, o)
                        .filter(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse))
                        .map(|e| (e.affinity, o))
                })
                .collect();
            near.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
            let guest = near.into_iter().map(|x| x.1).find(|&o| {
                free(world, o)
                    && !far(world, o)
                    && !world.visits.contains_key(&o)
                    && crate::systems::faith::creed_of(world, o).is_none()
                    && world.gang_of(o).and_then(|g| world.comp::<Gang>(g)).is_none_or(|g| g.creed.is_none())
            });
            if let Some(o) = guest {
                world.visits.insert(o, PendingVisit { kin: place, why: VisitWhy::Service, hour: visit_hour });
                world.consequence.guests.push(o);
                called += 1;
            }
        }
    }
    if called == 0 {
        return;
    }
    world.consequence.services.push((place, Service { preacher, creed, gang, row, hour, called, came: 0 }));
    ownership::jobs2_row(world).svc_held += 1;
}

/// An attendee arrived (a `Visit` to the place, or a Statistical one at its hour).
pub fn attended(world: &mut World, who: EntityId, place: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(s) = world.consequence.services.iter().find(|x| x.0 == place).map(|x| x.1.clone()) else { return };
    let pl = world.name_of(place);
    let hour = s.hour;
    if Some(who) == s.preacher {
        let text = format!(
            "{} preached {} at {pl} at {hour}:00 ({} called){}",
            world.name_of(who),
            s.creed,
            s.called,
            if walked { "" } else { " (off screen)" }
        );
        world.push_event(EventKind::Service, &[who, place], text);
        return;
    }
    if let Some(x) = world.consequence.services.iter_mut().find(|x| x.0 == place) {
        x.1.came += 1;
    }
    ownership::jobs2_row(world).svc_attended += 1;
    let belonging = world.comp::<Needs>(who).map_or(1.0, |n| n.belonging);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        n.belonging = (n.belonging + c.svc_gain).min(1.0);
    }
    let an = world.name_of(who);
    let preacher = s.preacher.filter(|&p| living(world, p));
    if let Some(p) = preacher {
        crate::systems::social::interacted(world, who, p);
    }
    let head = match preacher {
        Some(p) => format!("{an} heard {} preach {} at {pl}", world.name_of(p), s.creed),
        None => format!("{an} sat through the {} service at {pl}", s.creed),
    };
    let head = if walked { head } else { format!("{head} (off screen)") };
    let mut actors = vec![who];
    actors.extend(preacher);
    actors.push(place);
    let guest = world.consequence.guests.contains(&who);
    let day = world.day();
    let m = crate::systems::misery::misery(world, who);
    let t = traits(world, who);
    if guest {
        ownership::jobs2_row(world).svc_guests += 1;
        let p = c.svc_convert_p * (0.5 + m + (1.0 - belonging));
        if hash(world, P_CONVERT, who, day) < p {
            let took = match (s.row, s.gang) {
                (Some(i), _) => {
                    crate::systems::faith::convert(world, who, i);
                    crate::systems::faith::creed_of(world, who) == Some(i)
                }
                (None, Some(g)) => {
                    let open = world.comp::<Gang>(g).is_some_and(|x| x.members.len() < world.config.gangs.max_members)
                        && crate::systems::creeds::tolerated(world, g, who);
                    if open && world.gang_of(who).is_none() {
                        crate::systems::gang::enlist(world, who, g);
                    }
                    world.gang_of(who) == Some(g)
                }
                (None, None) => false,
            };
            if took {
                ownership::jobs2_row(world).svc_converts += 1;
                let text = format!("{head} as a guest, and took it up (misery {m:.2}, belonging {belonging:.2})");
                world.push_event(EventKind::Service, &actors, text);
                return;
            }
        }
        world.push_event(EventKind::Service, &actors, format!("{head} as a guest, and went home unmoved"));
        return;
    }
    // The plate.
    let purse = world.purse(Some(who)).max(0);
    let mut plate = String::new();
    if purse > 0 {
        let amount = ((purse as f32 * c.svc_tithe_share).round() as i64).clamp(1, c.svc_tithe_max.max(1));
        let moved = match s.gang {
            Some(g) => ownership::pay(world, Some(who), Some(g), amount, Flow::Tithe),
            None => ownership::tithe_in(world, who, place, amount),
        };
        if moved > 0 {
            ownership::jobs2_row(world).svc_tithe += moved;
            plate = format!(", put {moved}c on the plate");
        }
    }
    // Walking out.
    let p_out = c.svc_walkout_p * (1.5 - t.loyalty) * (0.5 + m);
    if hash(world, P_WALKOUT, who, day) < p_out {
        walk_out(world, &c, who, &s, preacher, &head, &actors, m, t.loyalty);
        return;
    }
    world.push_event(EventKind::Service, &actors, format!("{head}{plate}"));
}

#[allow(clippy::too_many_arguments)]
fn walk_out(
    world: &mut World,
    c: &ConsequenceCfg,
    who: EntityId,
    s: &Service,
    preacher: Option<EntityId>,
    head: &str,
    actors: &[EntityId],
    misery: f32,
    loyalty: f32,
) {
    let mut hunted = None;
    match s.gang {
        Some(g) if world.gang_of(who) == Some(g) => {
            crate::systems::gang::leave(world, who, "walked out of the service");
            // A cult row's leaver took the leader's apostasy grudge
            // (`faith::on_leave`); a Purist's (or a row without one) takes
            // the preacher's.
            let row_w = s.row.and_then(|i| crate::systems::faith::row(world, i)).map_or(0.0, |r| r.apostasy_w);
            let leader = world.comp::<Gang>(g).and_then(|x| x.leader);
            if row_w > 0.0 {
                hunted = leader;
            } else if let Some(p) = preacher.filter(|&p| p != who) {
                if crate::systems::grudges::add(world, p, who, GrudgeCause::Apostasy, c.svc_heretic_w, 0) {
                    world.jobs2.faith.apostates.insert(who);
                    hunted = Some(p);
                }
            }
        }
        _ => {
            world.jobs2.faith.followers.remove(&who);
            let row_w = s.row.and_then(|i| crate::systems::faith::row(world, i)).map_or(0.0, |r| r.apostasy_w);
            if row_w > 0.0 {
                if let Some(p) = preacher.filter(|&p| p != who) {
                    if crate::systems::grudges::add(world, p, who, GrudgeCause::Apostasy, row_w, 0) {
                        world.jobs2.faith.apostates.insert(who);
                        hunted = Some(p);
                    }
                }
            }
        }
    }
    let tail = match hunted {
        Some(h) => {
            let w = crate::systems::grudges::grudge_on(world, h, who).map_or(0.0, |g| g.weight);
            format!("; {} named {} a heretic (grudge {w:.2})", world.name_of(h), pro(world, who).1)
        }
        None => String::new(),
    };
    let text = format!("{head} and walked out on {} (loyalty {loyalty:.2}, misery {misery:.2}){tail}", s.creed);
    let mut a = actors.to_vec();
    a.extend(hunted.filter(|h| !a.contains(h)));
    world.push_event(EventKind::Service, &a, text);
    ownership::jobs2_row(world).svc_walkouts += 1;
}
