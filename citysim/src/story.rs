//! Biographies (M10 phase 4): the lines of an agent's Story tab.
//!
//! `lines` merges the agent's `Life` (events, one template per `LifeKind`)
//! with runs of days from their `Trace` (five or more days sharing a
//! summary key fold into one line), newest first. It lives here, not in the
//! app, so it is testable without egui: the app only draws `StoryLine`s.
//! Full and Statistical agents read alike; the only difference is a victim
//! hole that is still open, which reads "an unknown assailant" and carries a
//! `find_out` hole id for the Bind command.

use crate::components::{trace_flags, DayTrace, HoleId, Life, LifeEvent, LifeKind, Trace, Zone};
use crate::entity::EntityId;
use crate::time::{self, Tick, TICKS_PER_DAY};
use crate::world::World;

/// A trace run needs at least this many days to earn a line.
pub const MIN_RUN_DAYS: usize = 5;

/// One piece of a line: plain text, or an agent the app draws as a link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Span {
    Text(String),
    Agent(EntityId, String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryLine {
    /// Sort key: the event's tick, or the start of a run's last day.
    pub tick: Tick,
    /// First day, and last day (equal for an event).
    pub first_day: u64,
    pub last_day: u64,
    pub spans: Vec<Span>,
    /// An open victim hole behind this line: the app offers `Bind(hole)`.
    pub find_out: Option<HoleId>,
    /// A folded trace run rather than a life event.
    pub run: bool,
}

impl StoryLine {
    /// "day 34" or "days 12-20".
    pub fn when(&self) -> String {
        if self.first_day == self.last_day {
            format!("day {}", self.first_day)
        } else {
            format!("days {}-{}", self.first_day, self.last_day)
        }
    }

    /// The body as plain text.
    pub fn text(&self) -> String {
        self.spans
            .iter()
            .map(|s| match s {
                Span::Text(t) | Span::Agent(_, t) => t.as_str(),
            })
            .collect()
    }
}

/// What a run of days shares. Priority: jailed, homeless, hungry, quiet.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum RunKey {
    Jailed,
    Homeless,
    Hungry(Zone),
    Quiet,
}

fn run_key(t: DayTrace) -> Option<RunKey> {
    if !t.has(trace_flags::ALIVE) {
        None
    } else if t.has(trace_flags::JAILED) {
        Some(RunKey::Jailed)
    } else if t.has(trace_flags::HOMELESS) {
        Some(RunKey::Homeless)
    } else if t.hunger == 0 {
        Some(RunKey::Hungry(t.zone))
    } else if t.has(trace_flags::SLEPT_AT_HOME) && t.mood >= 2 {
        Some(RunKey::Quiet)
    } else {
        None
    }
}

fn run_text(key: RunKey, n: usize) -> String {
    match key {
        RunKey::Jailed => format!("{n} days inside"),
        RunKey::Homeless => format!("{n} days on the street"),
        RunKey::Hungry(zone) if (7..14).contains(&n) => format!("a hungry week in the {zone}"),
        RunKey::Hungry(zone) => format!("{n} hungry days in the {zone}"),
        RunKey::Quiet => format!("{n} quiet days at home"),
    }
}

/// The folded runs of a trace, oldest first: `(first_day, last_day, text)`.
fn runs(trace: &Trace) -> Vec<(u64, u64, String)> {
    let n = trace.days.len() as u64;
    if n == 0 {
        return Vec::new();
    }
    let first = (trace.last_day + 1).saturating_sub(n);
    let mut out = Vec::new();
    let mut cur: Option<(RunKey, u64, usize)> = None; // key, first day, length
    let close = |cur: &mut Option<(RunKey, u64, usize)>, out: &mut Vec<(u64, u64, String)>| {
        if let Some((key, start, len)) = cur.take() {
            if len >= MIN_RUN_DAYS {
                out.push((start, start + len as u64 - 1, run_text(key, len)));
            }
        }
    };
    for (i, t) in trace.days.iter().enumerate() {
        let day = first + i as u64;
        match (run_key(*t), cur) {
            (Some(k), Some((ck, start, len))) if k == ck => cur = Some((ck, start, len + 1)),
            (k, _) => {
                close(&mut cur, &mut out);
                cur = k.map(|k| (k, day, 1));
            }
        }
    }
    close(&mut cur, &mut out);
    out
}

fn text(s: impl Into<String>) -> Span {
    Span::Text(s.into())
}

/// The district a victim-side entry happened in, by name: the open hole's,
/// else the victim's trace on that day (M12 D5).
fn district_of(world: &World, id: EntityId, e: &LifeEvent) -> Option<String> {
    let d = match e.hole.and_then(|h| world.holes.get(&h)) {
        Some(h) => h.district,
        None => world.comp::<Trace>(id)?.on_day(time::day(e.tick))?.district,
    };
    (!d.is_unset()).then(|| world.district_name(d).to_string())
}

/// One life event as spans. `unknown` is what stands for a missing name.
fn body(world: &World, id: EntityId, e: &LifeEvent) -> Vec<Span> {
    let who = |fallback: &str| match e.other {
        Some(o) => Span::Agent(o, world.name_of(o)),
        None => text(fallback),
    };
    let in_zone = || district_of(world, id, e).map(|d| format!(" in {d}")).unwrap_or_default();
    // Still open: the player can find out. Closed unbound: nobody ever will.
    let open = e.hole.is_some_and(|h| world.holes.contains_key(&h));
    let unknown = if open { "an unknown assailant" } else { "someone who was never found" };
    match e.kind {
        LifeKind::Born => match e.other {
            Some(_) => vec![text("born to "), who("")],
            None => vec![text("born")],
        },
        LifeKind::Married => vec![text("married "), who("someone")],
        LifeKind::Widowed => match e.other {
            Some(_) => vec![text("widowed when "), who(""), text(" died")],
            None => vec![text("widowed")],
        },
        LifeKind::Hired => vec![text("took a job")],
        LifeKind::Fired => vec![text("lost the job")],
        LifeKind::Quit => vec![text("quit the job")],
        LifeKind::Paid => vec![text("was paid")],
        LifeKind::Starving => vec![text("was starving")],
        LifeKind::Robbed => vec![text(format!("was robbed{} by ", in_zone())), who(unknown)],
        LifeKind::Assaulted => vec![text(format!("was beaten{} by ", in_zone())), who(unknown)],
        LifeKind::Stole => vec![text("stole food")],
        LifeKind::RobbedSomeone => match e.other {
            Some(_) => vec![text("robbed "), who("")],
            None => vec![text("robbed a household")],
        },
        LifeKind::AssaultedSomeone => vec![text("beat "), who("someone")],
        LifeKind::Killed => vec![text(format!("was killed{} by ", in_zone())), who(unknown)],
        LifeKind::KilledSomeone => vec![text("killed "), who("someone")],
        LifeKind::Died => vec![text("died")],
        LifeKind::Arrested => match e.other {
            Some(_) => vec![text("was arrested by "), who("")],
            None => vec![text("was arrested")],
        },
        LifeKind::Released => vec![text("was released from the Precinct")],
        LifeKind::Escaped => vec![text("broke out of the Precinct")],
        LifeKind::JoinedGang => vec![text("joined a gang")],
        LifeKind::LeftGang => vec![text("left the gang")],
        LifeKind::Betrayed => vec![text("betrayed the gang")],
        LifeKind::Evicted => vec![text("ended up homeless")],
        LifeKind::Housed => vec![text("found a home")],
        LifeKind::Founded => vec![text("opened a business")],
        LifeKind::Incorporated => vec![text("incorporated a company")],
        LifeKind::Immigrated => vec![text("arrived in the city")],
        LifeKind::Buried => match e.other {
            Some(_) => vec![text("was laid to rest by "), who("")],
            None => vec![text("was laid to rest")],
        },
        LifeKind::Witnessed => match e.other {
            Some(_) => vec![text("saw "), who(""), text(" commit a crime")],
            None => vec![text("saw a crime")],
        },
    }
}

/// The biography of `id`, newest first. Works for the dead (their `Life` and
/// `Trace` outlive them) and for any tier.
pub fn lines(world: &World, id: EntityId) -> Vec<StoryLine> {
    let mut out: Vec<StoryLine> = Vec::new();
    if let Some(life) = world.comp::<Life>(id) {
        for e in &life.events {
            let day = time::day(e.tick);
            out.push(StoryLine {
                tick: e.tick,
                first_day: day,
                last_day: day,
                spans: body(world, id, e),
                find_out: e.hole.filter(|h| world.holes.contains_key(h)),
                run: false,
            });
        }
    }
    if let Some(trace) = world.comp::<Trace>(id) {
        for (first, last, t) in runs(trace) {
            out.push(StoryLine {
                tick: last * TICKS_PER_DAY,
                first_day: first,
                last_day: last,
                spans: vec![text(t)],
                find_out: None,
                run: true,
            });
        }
    }
    out.sort_by_key(|l| std::cmp::Reverse(l.tick));
    out
}
