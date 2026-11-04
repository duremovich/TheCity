use super::*;

/// Names a folded line gives before "and N more" (round 3 S7, OPEN-225).
pub(super) const FOLD_NAMES: usize = 3;

/// Round 3 S7 (OPEN-225): a crowd event's shape. One minute of a raid wrote
/// 260 `Witness` lines, ten `Assault`s by one raider and a `Report` and
/// `WitnessReacted` per witness. The lines of one shape in one minute fold
/// into the first: `{before}{count}{after} (names)`, `{}` in `after` the
/// summed count (a gossip's "told K of his people").
pub(super) struct FoldShape {
    before: String,
    after: String,
    pub(super) name: String,
    pub(super) sum: u32,
}

impl FoldShape {
    pub(super) fn same(&self, o: &FoldShape) -> bool {
        self.before == o.before && self.after == o.after
    }
}

pub(super) struct Fold {
    pub(super) tick: Tick,
    pub(super) kind: EventKind,
    pub(super) shape: FoldShape,
    pub(super) names: Vec<String>,
    pub(super) sum: u32,
    /// Where the first line went: the diary entry, the notable list, the money line's events.
    pub(super) entry: usize,
    pub(super) notable: Option<usize>,
    pub(super) ctx: usize,
}

impl Fold {
    pub(super) fn text(&self) -> String {
        let n = self.names.len();
        // Y3: one name every time is one target hit n times (a hit's pairings
        // all land on the mark): the attacker and the victim, once each.
        if n > 1 && self.sum == 0 && self.names.iter().all(|x| *x == self.names[0]) {
            return format!("{:?}: {}{} {n} times{}", self.kind, self.shape.before, self.names[0], self.shape.after);
        }
        let shown: Vec<&str> = self.names.iter().take(FOLD_NAMES).map(String::as_str).collect();
        let more = n.saturating_sub(shown.len());
        let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
        let after = self.shape.after.replace("{}", &self.sum.to_string());
        format!("{:?}: {}{n}{after} ({}{tail})", self.kind, self.shape.before, shown.join(", "))
    }
}

/// The fold shape of a crowd event, `None` for a line that stands alone (a
/// death, a blackmail, anything not of a crowd's shapes).
pub(super) fn fold_shape(e: &Event) -> Option<FoldShape> {
    let t = e.text.as_str();
    let shape = |before: String, after: String, name: &str, sum: u32| {
        Some(FoldShape { before, after, name: name.to_string(), sum })
    };
    match e.kind {
        // "W saw A commit C".
        EventKind::Witness => {
            t.split_once(" saw ").and_then(|(w, rest)| shape(String::new(), format!(" saw {rest}"), w, 0))
        }
        // "W reported A for C".
        EventKind::Report => {
            t.split_once(" reported ").and_then(|(w, rest)| shape(String::new(), format!(" reported {rest}"), w, 0))
        }
        // "W saw A's C and ran"; "W told K of his people what A did (the C)".
        EventKind::WitnessReacted if t.ends_with(" and ran") => {
            t.split_once(" saw ").and_then(|(w, rest)| shape(String::new(), format!(" saw {rest}"), w, 0))
        }
        EventKind::WitnessReacted => {
            let (w, rest) = t.split_once(" told ")?;
            let (k, rest) = rest.split_once(" of ")?;
            let k: u32 = k.parse().ok()?;
            let (_, what) = rest.split_once(" people ")?;
            shape(String::new(), format!(" told {{}} of their people {what}"), w, k)
        }
        // A brawl's pairing, "R attacked D at P" (one that killed stands alone).
        EventKind::Assault if !t.ends_with(" and died") => {
            let (r, rest) = t.split_once(" attacked ")?;
            let (d, place) = rest.split_once(" at ")?;
            shape(format!("{r} attacked "), format!(" at {place}"), d, 0)
        }
        _ => None,
    }
}

/// Round 4 T7 (OPEN-233): the petty crimes (`law::is_petty`) by their
/// labels, each its own hourly fold key.
const PETTY: [&str; 4] = ["petty:Fare Evasion", "petty:Vagrancy", "petty:Unlicensed Arms", "petty:Solicitation"];

/// Round 4 T7: a `Witness` line with the pick the witness of a petty crime,
/// as its hourly fold key.
pub(super) fn petty_seen(e: &Event, me: EntityId) -> Option<&'static str> {
    if e.kind != EventKind::Witness || e.actors.first() != Some(&me) {
        return None;
    }
    PETTY.iter().copied().find(|k| k.strip_prefix("petty:").is_some_and(|c| e.text.ends_with(&format!(" commit {c}"))))
}

/// Round 4 T7 (OPEN-233): how an event line folds. A standing fact (its
/// class and key) is told on its first day and on the change, the days it
/// was told again listed on the first line; a day fold (its key) folds the
/// day's lines of one chase or one face at one door into the first.
pub(super) enum Refold {
    Standing(&'static str, String),
    Day(String),
}

pub(super) fn refold_of(e: &Event, me: EntityId) -> Option<Refold> {
    let t = e.text.as_str();
    match e.kind {
        // `distance`'s trade reach, `crew::muster_reach`, `crew`'s duty pick:
        // the same sentence at 00:07 daily (V12: 4-6 times a life).
        EventKind::Crew if t.contains(" stays off ") && t.contains("'s trade today: ") => {
            Some(Refold::Standing("stays_off", t.to_string()))
        }
        EventKind::Crew if t.contains(" sat out ") && t.contains("'s raid: ") => {
            Some(Refold::Standing("sat_out", t.to_string()))
        }
        EventKind::Crew if t.contains(" chose the shift as ") => {
            let key = t.rsplit_once(" (").map_or(t, |(k, _)| k);
            Some(Refold::Standing("chose_shift", key.to_string()))
        }
        // `camp`'s quiet headcount (T1), the wardens' names aside.
        EventKind::Custody if t.contains("; nobody in or out since the last count") => {
            let key = t.split_once(" counted ").map_or(t, |(_, k)| k);
            Some(Refold::Standing("count", key.to_string()))
        }
        // `law::chase_dropped` (T3): per guard and suspect in the guard's
        // diary, per suspect in anyone else's.
        EventKind::Pursuit => {
            let (g, rest) = t.split_once(" gave up the chase of ")?;
            let (s, _) = rest.split_once(" (")?;
            Some(Refold::Day(if e.actors.first() == Some(&me) {
                format!("chase:{g}:{s}")
            } else {
                format!("chased:{s}")
            }))
        }
        // `service::refused` (T4): one face at one door.
        EventKind::Served if t.contains(" away at the door of ") || t.contains(" away again at the door of ") => {
            let (_, rest) = t.split_once(" turned ")?;
            let (x, rest) = rest.split_once(" away")?;
            let (_, door) = rest.split_once(" at the door of ")?;
            let door = door.split([' ', ':']).next().unwrap_or(door);
            Some(Refold::Day(format!("door:{x}:{door}")))
        }
        _ => None,
    }
}

/// Round 4 T7: what a line adds to its day fold (the reason, the name).
fn absorb(f: &mut DayFold, e: &Event) {
    let t = e.text.as_str();
    match e.kind {
        EventKind::Pursuit => {
            let Some((g, rest)) = t.split_once(" gave up the chase of ") else { return };
            f.name(g);
            let why = rest.split_once(" (").and_then(|(_, w)| w.split_once(')')).map_or("", |x| x.0);
            f.note(why.split(':').next().unwrap_or(why));
            if let Some((_, to)) = rest.split_once(": handed to ") {
                f.note(&format!("handed to {}", to.split(" on shift").next().unwrap_or(to)));
            }
        }
        EventKind::Served => {
            let head = t.split(": barred through ").next().unwrap_or(t);
            let why = head.rsplit_once(" (").and_then(|(_, w)| w.split_once(')')).map_or("no cause", |x| x.0);
            f.note(why);
            if let Some((_, till)) = t.split_once(": barred through ") {
                f.last = format!("barred through {till}");
            }
        }
        _ => {}
    }
}

/// Round 4 T7: a fold's first line, rewritten.
pub(super) fn render(f: &DayFold, kind: EventKind) -> String {
    match kind {
        EventKind::Pursuit if f.key.starts_with("chase:") => {
            let (g, s) = f.key["chase:".len()..].split_once(':').unwrap_or(("", ""));
            format!("Pursuit: {g} gave up the chase of {s} {} times today ({})", f.count, f.tally_text())
        }
        EventKind::Pursuit => {
            let s = &f.key["chased:".len()..];
            format!("Pursuit: {} chases of {s} given up today (by {}; {})", f.count, f.names_text(), f.tally_text())
        }
        EventKind::Served => {
            let last = if f.last.is_empty() { String::new() } else { format!("; {}", f.last) };
            format!("{} (turned away {} times today: {}{last})", f.base, f.count, f.tally_text())
        }
        _ => {
            let n = f.days.len();
            format!("{} (the same on {n} more day{}: {})", f.base, if n == 1 { "" } else { "s" }, days_text(&f.days))
        }
    }
}

/// Round 4 T7: fold `e` into an earlier line; false when it opens its own.
pub(super) fn fold_into(t: &mut Track, r: &Refold, e: &Event) -> bool {
    let day = e.tick / TICKS_PER_DAY;
    let f = match r {
        Refold::Standing(class, key) => match t.standing.get_mut(class) {
            Some(f) if f.key == *key => {
                if f.day != day && !f.days.contains(&day) {
                    f.days.push(day);
                }
                f
            }
            _ => return false,
        },
        Refold::Day(key) => match t.day_folds.get_mut(key) {
            Some(f) if f.day == day => {
                f.count += 1;
                absorb(f, e);
                f
            }
            _ => return false,
        },
    };
    let (entry, notable, text) = (f.entry, f.notable, render(f, e.kind));
    t.refold_line(entry, notable, &text);
    true
}

/// Round 4 T7: `e`, just logged at `entry`, opens its fold.
pub(super) fn open_fold(t: &mut Track, r: Refold, e: &Event, entry: usize, notable: Option<usize>, text: String) {
    let day = e.tick / TICKS_PER_DAY;
    match r {
        Refold::Standing(class, key) => {
            t.standing.insert(class, DayFold::new(key, day, entry, notable, text));
        }
        Refold::Day(key) => {
            let mut f = DayFold::new(key.clone(), day, entry, notable, text);
            absorb(&mut f, e);
            t.day_folds.insert(key, f);
        }
    }
}

pub(super) fn close_action(world: &World, t: &mut Track, now: Tick, status: &str) {
    let Some(o) = t.open.take() else { return };
    let dur = now.saturating_sub(o.start);
    // The perf pass phase 1 (OPEN-195): an idle Rest cut within minutes.
    if o.action == ActionKind::Rest && o.goal != Some(GoalKind::Work) && dur <= SHORT_REST_MIN {
        t.short_rests += 1;
    }
    let end_place = t.last_place.clone();
    let tgt = o.target.map_or(String::new(), |x| format!(" [target {}]", nm(world, x)));
    if o.action == ActionKind::HangOut {
        // V2: who was at the spot, the known contacts named.
        let others: Vec<EntityId> = std::mem::take(&mut t.hang_others).into_iter().collect();
        t.hangouts += 1;
        t.hang_empty += u32::from(others.is_empty());
        if t.spouse.is_some_and(|s| others.contains(&s)) {
            t.spouse_named_hours.insert(o.start / TICKS_PER_HOUR);
        }
        let mut parts: Vec<String> = Vec::new();
        let mut known = 0;
        let mut strangers = 0;
        for &x in &others {
            match world.edge(t.id, x) {
                Some(e) => {
                    known += 1;
                    t.hang_known.insert(x);
                    t.contact(world, x);
                    parts.push(format!("{} (known, {:?}, affinity {:+.2})", nm(world, x), e.kind, e.affinity));
                }
                None => strangers += 1,
            }
        }
        if strangers > 0 {
            parts.push(format!("{strangers} stranger(s)"));
        }
        t.log(
            o.start,
            "hangout",
            format!(
                "HANGOUT at {} for {dur} min ({status}): {} there{}",
                t.last_place,
                if others.is_empty() { "nobody".to_string() } else { format!("{} other(s)", others.len()) },
                if parts.is_empty() { String::new() } else { format!(": {} [{known} known]", parts.join(", ")) }
            ),
        );
    }
    let loc = if matches!(o.action, ActionKind::GoTo(_)) {
        if o.start_place == end_place {
            end_place.to_string()
        } else if status == "done" {
            format!("{} -> {}", o.start_place, end_place)
        } else {
            // OPEN-200: an interrupted leg's end is where it was cut, not
            // where it was going (the target is printed after).
            format!("{} -> cut at {}", o.start_place, end_place)
        }
    } else {
        end_place
    };
    if matches!(o.action, ActionKind::GoTo(_)) && dur > CAM_LONG_GOTO_MIN {
        t.cam.days.entry(o.start / TICKS_PER_DAY).or_default().goto_long += 1;
    }
    // The pressure pass F1: a GoTo's Manhattan tiles from its start.
    let tiles = match (o.action, o.start_tile, world.comp::<Position>(t.id).map(|p| p.tile)) {
        (ActionKind::GoTo(_), Some(a), Some(b)) => format!(", {} tiles", a.manhattan(b)),
        _ => String::new(),
    };
    t.log(o.start, "action", format!("ACTION {:?} @ {loc}, {dur} min{tiles} ({status}){tgt}", o.action));
}

/// Round 3 S7 (OPEN-225): the hour's quiet lines, each at its minute while
/// they are few (`FOLD_NAMES` + 1), else one line a key at its first minute.
pub(super) fn flush_hourly(t: &mut Track) {
    let buf = std::mem::take(&mut t.hourly);
    if buf.len() <= FOLD_NAMES + 1 {
        for q in buf {
            t.log(q.tick, q.cat, q.line);
        }
        return;
    }
    let mut keys: Vec<&'static str> = Vec::new();
    for q in &buf {
        if !keys.contains(&q.key) {
            keys.push(q.key);
        }
    }
    for k in keys {
        let g: Vec<&Quiet> = buf.iter().filter(|q| q.key == k).collect();
        if g.len() == 1 {
            t.log(g[0].tick, g[0].cat, g[0].line.clone());
            continue;
        }
        let shown: Vec<&str> = g.iter().take(FOLD_NAMES).map(|q| q.name.as_str()).collect();
        let more = g.len() - shown.len();
        let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
        let (n, names) = (g.len(), format!("{}{tail}", shown.join(", ")));
        let text = match k {
            "new" => format!("{n} new Acquaintances this hour ({names})"),
            "warmer" => format!("{n} Acquaintances warmer this hour ({names})"),
            "colder" => format!("{n} Acquaintances colder this hour ({names})"),
            "cells" => format!("met {n} in the cells this hour ({names})"),
            k if k.starts_with("petty:") => format!("Witness: saw {n} commit {} this hour ({names})", &k[6..]),
            _ => format!("GOSSIP ABOUT ME: {n} retellings this hour, every hearer knew already (told by {names})"),
        };
        t.log(g[0].tick, g[0].cat, text);
    }
}

pub(super) fn flush_day(world: &World, t: &mut Track, now: Tick) {
    flush_hourly(t);
    let day = t.cur_day;
    // The pressure pass R4 (OPEN-219): the shiftless rest day (weekday 6, a
    // guard's staggered one) is marked, so its zero work does not read as a bug.
    let rest = rest_day_note(world, t.id, day);
    let tick = day * TICKS_PER_DAY + TICKS_PER_DAY - 1;
    let _ = now;
    let hours: Vec<String> = CLASSES
        .iter()
        .filter_map(|c| t.day_use.get(c).filter(|&&m| m > 0).map(|&m| format!("{c} {:.1}h", f64::from(m) / 60.0)))
        .collect();
    let mut pl: Vec<(&String, &u32)> = t.day_places.iter().collect();
    pl.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let places: Vec<String> = pl.iter().filter(|(_, &m)| m >= 5).take(8).map(|(p, m)| format!("{p} {m}m")).collect();
    let text = format!(
        "DAY SUMMARY d{day}{rest}: {} | net {}{} | places: {}",
        if hours.is_empty() { "no activity".into() } else { hours.join(", ") },
        if t.day_net > 0 { "+" } else { "" },
        money(t.day_net),
        if places.is_empty() { "none".into() } else { places.join("; ") }
    );
    t.log(tick, "summary", text);
    t.day_use.clear();
    t.day_places.clear();
    t.day_net = 0;
}

/// The pressure pass R4: " (rest day: no shift)" when `id` holds a job with
/// no shift on `day` (weekday 6, a guard's staggered day), " (the city's rest
/// day)" for the jobless on weekday 6, else nothing.
pub(super) fn rest_day_note(world: &World, id: EntityId, day: u64) -> &'static str {
    let key = i64::try_from(day).unwrap_or(i64::MAX);
    match world.comp::<Job>(id) {
        Some(j) if !citysim::exec::routine::workday_of(world, id, j, key) => " (rest day: no shift)",
        Some(_) => "",
        None if !citysim::exec::routine::is_workday(key) => " (the city's rest day)",
        None => "",
    }
}
