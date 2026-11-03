//! M18 P4 (consequences, docs/M18_PLAYER.md): what the city knows of the
//! body the human drives and what it does about it, read from sim state for
//! the client's panels. No rule here writes anything: the memories naming
//! the body (`Memory` entries and the heard store, through
//! `memory::deed_of`), the district talk about its deeds (`World::rumours`),
//! the grudges on it (`Grudges`), the Hunts after it (`World::hunts`), the
//! contract records on it (`World::contracts`), its reputation (the daily
//! `reputation::rep`) and, at the end, its `Life` and `Corpse`.
//!
//! A full scan reads every body's memory (20k bodies: a few ms), so the client
//! calls it at its own cadence (`[view] news_scan_minutes`), not each tick.

use crate::components::{Corpse, Memory, MemoryEntry, MemoryKind};
use crate::contract::{ContractStatus, Target};
use crate::entity::EntityId;
use crate::systems::{grudges, memory, reputation};
use crate::time::Tick;
use crate::word::Grudges;
use crate::world::World;

/// How a knower came by what it knows.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum How {
    /// Was there (the deed seen, the meeting had).
    Saw,
    /// Told by someone who was there.
    Told,
    /// Rumour, this many tellings out.
    Rumour(u8),
    /// A Feed ran the story.
    Feed,
}

impl How {
    pub fn label(self) -> String {
        match self {
            How::Saw => "was there".into(),
            How::Told => "told by a witness".into(),
            How::Rumour(n) => format!("rumour, {n} tellings out"),
            How::Feed => "from a Feed".into(),
        }
    }
}

/// One body that knows something of the player: the weightiest thing, how it
/// heard, when, and how many things it holds.
#[derive(Clone, Debug)]
pub struct Knower {
    pub who: EntityId,
    pub name: String,
    pub line: String,
    pub how: How,
    pub tick: Tick,
    /// Knows a deed the player did (not just a meeting).
    pub deed: bool,
    pub count: usize,
}

/// Someone (or something) after the player: a grudge, a Hunt, a record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct After {
    /// What it is, for a client's diff: (0 grudge, 1 hunt, 2 record) and its key (holder index or record id).
    pub key: (u8, u64),
    pub line: String,
    pub since: Tick,
}

/// What the city knows of the player now.
#[derive(Clone, Debug, Default)]
pub struct Known {
    /// Most weighty first (a deed of the player's before a meeting, then the newest).
    pub knowers: Vec<Knower>,
    /// Grudges, Hunts and records on the player.
    pub after: Vec<After>,
    /// The district talk about the player's deeds (a line a deed).
    pub talk: Vec<String>,
    /// The daily reputation: dread, standing, honour, heat, known by.
    pub rep: [f32; 4],
    pub known_by: u16,
}

fn how_of(e: &MemoryEntry) -> How {
    if e.press != 0 {
        return How::Feed;
    }
    match memory::hops_of(e) {
        0 => How::Saw,
        1 => How::Told,
        n => How::Rumour(n),
    }
}

/// A meeting's line (a memory naming the player that is no deed).
fn met_line(kind: MemoryKind) -> String {
    match kind {
        MemoryKind::Socialised => "talked with you".into(),
        MemoryKind::Served => "dealt with you over a counter".into(),
        MemoryKind::Fought | MemoryKind::Bout => "fought you".into(),
        MemoryKind::Won => "beat you in a fight".into(),
        MemoryKind::Persuaded => "was worked on by you".into(),
        MemoryKind::Courted => "was courted by you".into(),
        MemoryKind::Rejected => "turned you down".into(),
        MemoryKind::Sighting => "heard where you were".into(),
        MemoryKind::MetInJail => "met you inside".into(),
        k => format!("{k:?}"),
    }
}

/// A party of a deed as `holder` tells it to the player: "you", "them" (the
/// holder itself), a name, or `None` when nobody is named.
fn party(world: &World, id: EntityId, holder: Option<EntityId>, o: Option<EntityId>) -> Option<String> {
    let o = o?;
    Some(if o == id {
        "you".into()
    } else if Some(o) == holder {
        "them".into()
    } else {
        grudges::label(world, o)
    })
}

/// A deed as a phrase: "you assaulted them", "Vale Singh robbed you", "you
/// were arrested" (an Arrested deed's actor is the one taken).
pub fn phrase(world: &World, id: EntityId, holder: Option<EntityId>, r: &crate::word::DeedRef) -> String {
    let a = party(world, id, holder, r.actor).unwrap_or_else(|| "someone".into());
    let o = party(world, id, holder, r.object);
    match (r.deed, o) {
        (crate::word::Deed::Arrested, _) => format!("{a} {} arrested", if a == "you" { "were" } else { "was" }),
        (d, Some(o)) => format!("{a} {} {o}", d.label()),
        (d, None) => format!("{a} {}", d.label()),
    }
}

/// What the city knows of `id` now: at most `cap` knowers (the weightiest).
pub fn known(world: &World, id: EntityId, cap: usize) -> Known {
    let mut knowers = Vec::new();
    for holder in world.with::<Memory>() {
        if holder == id {
            continue;
        }
        let Some(m) = world.comp::<Memory>(holder) else { continue };
        // the best entry: (a deed of the player's, |valence| x salience, tick)
        let mut best: Option<(bool, f32, &MemoryEntry, String)> = None;
        let mut count = 0usize;
        for e in m.entries.iter().chain(m.heard.iter()) {
            let line = match memory::deed_of(holder, e) {
                Some(d) if d.actor == Some(id) => {
                    Some((true, format!("knows {}", phrase(world, id, Some(holder), &d))))
                }
                Some(d) if d.object == Some(id) => {
                    Some((false, format!("knows {}", phrase(world, id, Some(holder), &d))))
                }
                Some(_) => None,
                None if e.subject == Some(id) => Some((false, met_line(e.kind))),
                None => None,
            };
            let Some((deed, line)) = line else { continue };
            count += 1;
            let w = e.valence.abs() * e.salience;
            let better = best
                .as_ref()
                .is_none_or(|(bd, bw, be, _)| (deed, w) > (*bd, *bw) || ((deed, w) == (*bd, *bw) && e.tick > be.tick));
            if better {
                best = Some((deed, w, e, line));
            }
        }
        if let Some((deed, _, e, line)) = best {
            knowers.push(Knower {
                who: holder,
                name: world.name_of(holder),
                line,
                how: how_of(e),
                tick: e.tick,
                deed,
                count,
            });
        }
    }
    knowers.sort_by(|a, b| b.deed.cmp(&a.deed).then(b.tick.cmp(&a.tick)).then(a.who.cmp(&b.who)));
    knowers.truncate(cap);
    let r = reputation::rep(world, id);
    Known {
        knowers,
        after: after(world, id),
        talk: talk(world, id),
        rep: [r.dread, r.standing, r.honour, r.heat],
        known_by: r.known_by,
    }
}

/// The grudges, Hunts and live records on `id`.
pub fn after(world: &World, id: EntityId) -> Vec<After> {
    let mut out = Vec::new();
    for holder in world.with::<Grudges>() {
        let Some(g) = world.comp::<Grudges>(holder) else { continue };
        for x in g.list.iter().filter(|x| x.target == id && x.settled.is_none() && x.weight > 0.0) {
            out.push(After {
                key: (0, u64::from(holder.index)),
                line: format!(
                    "{} holds a grudge on you ({}, {:.2})",
                    world.name_of(holder),
                    grudges::cause_label(x.cause),
                    x.weight
                ),
                since: x.since,
            });
        }
    }
    for (&h, s) in world.hunts.iter().filter(|(_, s)| s.target == id) {
        let phase = match s.phase {
            crate::word::HuntPhase::Ask => "asking around for you",
            crate::word::HuntPhase::Watch => "watching for you",
        };
        out.push(After {
            key: (1, u64::from(h.index)),
            line: format!("{} is hunting you ({phase})", world.name_of(h)),
            since: s.since,
        });
    }
    for c in world.contracts.values() {
        // a guard on oneself, or a record of one's own, is nobody after you
        if !matches!(c.target, Target::Agent(t) if t == id)
            || !matches!(c.status, ContractStatus::Open | ContractStatus::Taken)
            || c.kind == crate::contract::ContractKind::Guard
            || c.buyer == Some(id)
        {
            continue;
        }
        let by = c.buyer.map_or("the city".to_string(), |b| grudges::label(world, b));
        let taker = match (c.status, c.taker) {
            (ContractStatus::Taken, Some(t)) => format!(", taken by {}", grudges::label(world, t)),
            _ => ", open".to_string(),
        };
        out.push(After {
            key: (2, c.id),
            line: format!("a {} on you, {}c, from {by}{taker}", c.kind.label(), c.price),
            since: c.posted,
        });
    }
    out.sort_by_key(|a| a.key);
    out
}

/// The district talk naming `id` as a deed's actor or object.
pub fn talk(world: &World, id: EntityId) -> Vec<String> {
    let mut out = Vec::new();
    for (i, pool) in world.rumours.iter().enumerate() {
        for e in pool.entries.iter().filter(|e| e.actor == Some(id) || e.object == Some(id)) {
            let d = u8::try_from(i).unwrap_or(u8::MAX);
            let what = phrase(world, id, None, &e.deed_ref());
            out.push(format!(
                "{}: \"{what}\" (day {}, {:.0}% of the district talking)",
                world.district_name(crate::components::DistrictId(d)),
                crate::time::day(e.tick),
                e.reach * 100.0
            ));
        }
    }
    out
}

/// The end of a life, for the death screen: the cause, who did it, the day,
/// the age, and the biography's lines (newest first, `story::lines`).
#[derive(Clone, Debug)]
pub struct Death {
    pub line: String,
    pub life: Vec<String>,
}

/// `Some` once `id` is a corpse.
pub fn death(world: &World, id: EntityId) -> Option<Death> {
    let c = world.comp::<Corpse>(id)?;
    let by =
        c.killer.filter(|&k| k != id).map(|k| format!(" at the hands of {}", world.name_of(k))).unwrap_or_default();
    let line = format!(
        "{} died of {:?}{by}, day {} {}, aged {}",
        world.name_of(id),
        c.cause,
        crate::time::day(c.died_tick),
        crate::time::clock(c.died_tick),
        crate::systems::ages::years(world, id)
    );
    let life = crate::story::lines(world, id).iter().map(|l| format!("{}: {}", l.when(), l.text())).collect();
    Some(Death { line, life })
}
