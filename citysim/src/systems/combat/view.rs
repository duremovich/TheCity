use super::*;

// ---------------------------------------------------------------------------
// C4: the fight HUD and the wounds panel (read by cityview)
// ---------------------------------------------------------------------------

/// One body in the player's fight, as the HUD shows it.
#[derive(Clone, Debug)]
pub struct FightMember {
    pub id: EntityId,
    pub name: String,
    pub stance: &'static str,
    /// In the duel now (the front).
    pub front: bool,
    /// What the eye sees: a limp, a bad arm, bleeding, a robot.
    pub shows: String,
    /// Close above its break line (its morale and the numbers).
    pub wavering: bool,
}

/// C4: the player's fight for the HUD (`fight_view`).
#[derive(Clone, Debug, Default)]
pub struct FightView {
    /// The player's side (0 the starter's).
    pub mine: usize,
    pub sides: [Vec<FightMember>; 2],
    pub stance: Option<Stance>,
    pub weapon: String,
    pub holstered: bool,
    pub cover: f32,
    pub fatigue: f32,
    pub blood: f32,
    /// The player's own wounds, exactly.
    pub wounds: Vec<String>,
    /// The morale hints ("he's wavering", "they outnumber you").
    pub hints: Vec<String>,
    /// C5: the player's body is under its break line (shaking).
    pub shaking: bool,
    /// C5: the fist fight's orders that do nothing are hidden: Take cover
    /// (melee ignores cover: nobody on either side has a gun) and Holster
    /// (bare hands).
    pub guns: bool,
    /// The chrome to hand (a Sandevistan's charge, a Kerenzikov's dodges).
    pub chrome: Vec<String>,
    /// An Attack row for each foe standing: (the foe, its name, the odds of
    /// the duel with it alone, the whole fight's odds going for it).
    pub odds: Vec<(EntityId, String, f32, f32)>,
    /// A Help row: the downed of the fight.
    pub downed: Vec<(EntityId, String)>,
    pub beats: u32,
    /// Orders waiting for the next beat.
    pub pending: Vec<String>,
}

/// What another body's wounds show: a limp, a bad arm, bleeding.
fn shows(world: &World, x: EntityId) -> String {
    if crate::systems::robots::is_robot(world, x) {
        return "a robot".into();
    }
    let mut v: Vec<&str> = carried_note(world, x).into_iter().collect();
    if world.comp::<Wounds>(x).is_some_and(|w| w.list.iter().any(|y| y.bleeding)) {
        v.push("bleeding");
    }
    if crate::systems::wounded::is_down(world, x) {
        v.push("down");
    }
    v.join(", ")
}

/// A fighter's margin over its break line, with the numbers.
fn margin(world: &World, f: &Fighter, own: f32, theirs: f32) -> f32 {
    let c = &world.config.combat;
    if !f.morale.is_finite() {
        return f32::INFINITY;
    }
    f.morale + c.numbers_morale * (own.max(1.0) / theirs.max(1.0)).ln() - (c.break_at + c.break_shift[f.stance.i()])
}

/// A body's own wounds in words, exactly (the healing, seen to or not; the
/// marks).
pub fn wound_lines(world: &World, id: EntityId) -> Vec<String> {
    let c = &world.config.combat;
    let Some(w) = world.comp::<Wounds>(id) else { return vec![] };
    let now = world.tick;
    // C5: alike wounds folded ("body grazed x8"), the longest heal first, worst first
    let mut rows: Vec<(Severity, String, f32, u32)> = Vec::new();
    for x in &w.list {
        let days = heal_at(x, c).saturating_sub(now) as f32 / TICKS_PER_DAY as f32;
        let seen = if x.treated { "seen to" } else { "not seen to" };
        let bleed = if x.bleeding { ", bleeding" } else { "" };
        let what = format!("{} {}{bleed}, {seen}", x.region.label(), x.severity.label());
        match rows.iter_mut().find(|r| r.1 == what) {
            Some(r) => {
                r.2 = r.2.max(days);
                r.3 += 1;
            }
            None => rows.push((x.severity, what, days, 1)),
        }
    }
    rows.sort_by_key(|a| std::cmp::Reverse(a.0));
    let mut out: Vec<String> = rows
        .into_iter()
        .map(|(_, what, days, n)| {
            let n = if n > 1 { format!(" x{n}") } else { String::new() };
            format!("{what}{n}: heals in {days:.1} days")
        })
        .collect();
    for m in &w.marks {
        out.push(format!("{}: {:?} for good", m.0.label(), m.1).to_lowercase());
    }
    out
}

/// C4: the player's fight now (`None`: not in a live one).
pub fn fight_view(world: &World, id: EntityId) -> Option<FightView> {
    let eid = world.combat.engaged.get(&id)?;
    let e = world.combat.engagements.get(eid)?;
    let mine = usize::from(!e.sides[0].contains(&id));
    let live = |x: EntityId| e.duel.as_ref().and_then(|d| d.f.iter().chain(d.aid.iter().flatten()).find(|f| f.id == x));
    let fronts = e.duel.as_ref().map(|d| [d.f[0].id, d.f[1].id]);
    let standing = [0, 1].map(|k| e.sides[k].len() as f32);
    let c = &world.config.combat;
    let mut v = FightView { mine, beats: e.beats, ..Default::default() };
    for k in 0..2 {
        for &x in &e.sides[k] {
            let f = live(x);
            let stance = f.map(|f| f.stance).unwrap_or_else(|| stance_for(world, x, k == 0, &e.mods));
            let wavering = f.is_some_and(|f| margin(world, f, standing[k], standing[1 - k]) < c.hint_waver);
            v.sides[k].push(FightMember {
                id: x,
                name: world.name_of(x),
                stance: stance.label(),
                front: fronts.is_some_and(|fr| fr[k] == x),
                shows: if x == id { String::new() } else { shows(world, x) },
                wavering,
            });
        }
    }
    let me = live(id);
    let beat = e.duel.as_ref().map_or(0, |d| d.beat);
    v.stance = Some(me.map(|f| f.stance).unwrap_or_else(|| stance_for(world, id, mine == 0, &e.mods)));
    let kind = me.map_or_else(|| kind_of(world, id), |f| f.kind);
    v.weapon = kind_row(world, kind).filter(|_| kind > 0).map_or("bare hands".to_string(), |r| r.id.clone());
    if let Some(f) = me.filter(|f| f.ready_at > beat) {
        v.weapon.push_str(&format!(" (ready in {} beats)", f.ready_at - beat));
    }
    v.holstered = orders_of(world, id).is_some_and(|o| o.holster);
    v.guns = e.duel.as_ref().is_some_and(|d| d.f.iter().chain(d.aid.iter().flatten()).any(|f| ranged(world, f.kind)))
        || ranged(world, kind_of(world, id));
    v.cover =
        me.map_or_else(|| world.comp::<Position>(id).map_or(0.0, |p| cover_at(world, p.tile, p.building)), |f| f.cover);
    v.fatigue = me.map_or_else(|| fatigue_of(world, id), |f| f.fatigue);
    v.blood = world.comp::<Wounds>(id).map_or(1.0, |w| w.blood);
    v.wounds = wound_lines(world, id);
    // The hints: your nerve, the numbers, theirs.
    let (own, other) = (standing[mine], standing[1 - mine]);
    if let Some(f) = me {
        let m = margin(world, f, own, other);
        if m < 0.0 {
            // C5: the human's body never breaks on its nerve: it shakes.
            let (a, e) = (shaken(c, c.shake_attack, -m), shaken(c, c.shake_evade, -m));
            v.hints.push(format!(
                "you're shaking: your nerve is gone (blows x{a:.2}, dodging x{e:.2}); flee or yield when you choose"
            ));
            v.shaking = true;
        } else if m < c.hint_waver {
            v.hints.push("you're wavering".into());
        }
    }
    if other > own {
        v.hints.push(format!("they outnumber you ({other:.0} to {own:.0})"));
    } else if own > other {
        v.hints.push(format!("you outnumber them ({own:.0} to {other:.0})"));
    }
    for m in &v.sides[1 - mine] {
        if m.wavering {
            v.hints.push(format!("{} is wavering", m.name));
        }
    }
    if v.fatigue > 0.5 {
        v.hints.push("you're tiring".into());
    }
    // The chrome to hand.
    match me {
        Some(f) => {
            if f.sandy > 0 {
                v.chrome.push(if f.sandy_left > 0 {
                    format!("Sandevistan firing ({} beats)", f.sandy_left)
                } else if f.sandy_armed {
                    "Sandevistan charged (fires at your first strike)".into()
                } else {
                    "Sandevistan spent".into()
                });
            }
            if f.keren_p > 0.0 {
                v.chrome.push(format!("Kerenzikov: {} dodge charges", f.keren));
            }
            if f.gorilla {
                v.chrome.push("gorilla arms".into());
            }
        }
        None => v.chrome = chrome_lines(world, id),
    }
    for m in &v.sides[1 - mine] {
        let o = odds(world, id, m.id);
        v.odds.push((m.id, m.name.clone(), duel_odds(world, id, m.id), o.p));
    }
    v.downed = e
        .seen
        .iter()
        .copied()
        .filter(|&x| x != id && crate::systems::wounded::is_down(world, x))
        .map(|x| (x, world.name_of(x)))
        .collect();
    v.pending =
        world.combat.orders.get(&id).map(|o| o.pending.iter().map(|x| x.label(world)).collect()).unwrap_or_default();
    Some(v)
}

/// C4: the chrome a body carries into a fight, in words (outside one).
pub fn chrome_lines(world: &World, id: EntityId) -> Vec<String> {
    let c = &world.config.combat;
    let mut out = Vec::new();
    if let Some((_, t)) = chrome_tier(world, id, Slot::Nerves) {
        if t >= c.sandy_tier {
            let cd = world.combat.sandy_cd.get(&id).copied().filter(|&u| u > world.tick);
            out.push(match cd {
                Some(u) => format!("Sandevistan cooling ({} min)", u - world.tick),
                None => "Sandevistan charged".into(),
            });
        }
        out.push("Kerenzikov".into());
    }
    if let Some((_, t)) = chrome_tier(world, id, Slot::Arms) {
        out.push(if t >= c.mantis_tier { "mantis blades".into() } else { "gorilla arms".into() });
    }
    out
}

/// C4: the wounds panel outside a fight: the wounds and their healing, the
/// blood, and where they can be seen to at what price (the Treat goal's
/// pick, or why nobody will).
#[derive(Clone, Debug, Default)]
pub struct WoundsView {
    pub wounds: Vec<String>,
    pub blood: f32,
    pub care: Vec<String>,
    /// A wound not seen to (the Treat goal has work).
    pub untreated: bool,
}

pub fn wounds_view(world: &World, id: EntityId) -> Option<WoundsView> {
    let w = world.comp::<Wounds>(id)?;
    if w.list.is_empty() && w.marks.is_empty() {
        return None;
    }
    let mut v = WoundsView { wounds: wound_lines(world, id), blood: w.blood, ..Default::default() };
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |x| x.coins);
    if let Some(worst) = worst_untreated(world, id) {
        v.untreated = true;
        let (patch, back) = crate::systems::customers::patch_prices(world, id);
        let grave = worst.index() > world.config.combat.back_room_max;
        v.care.push(format!(
            "worst not seen to: {}; a Clinic's patch-up {patch}c, the back room's {back}c{}; you have {coins}c",
            worst.label(),
            if grave { " (it won't take a Grave)" } else { "" }
        ));
        match crate::systems::customers::care_choice(world, id) {
            Some((b, how, price)) => {
                v.care.push(format!("the nearest that will see you: {} ({how:?}, {price}c)", world.name_of(b)));
            }
            None => {
                let why = crate::systems::customers::untreated_why(world, id)
                    .unwrap_or_else(|| "nobody will see to it now".into());
                v.care.push(format!("nobody will see to it: {why}"));
            }
        }
    } else if !w.list.is_empty() {
        v.care.push("all seen to: they heal on their own".into());
    }
    Some(v)
}

/// C4: a beat in words for the player's story log (`me` reads "you"); a
/// robot is dented, not wounded. Also `true` when it is a turn of the fight
/// (a break, a joiner, a grave hit) rather than a blow.
pub fn blow_line(world: &World, b: &BeatLog, me: EntityId) -> (String, bool) {
    let who = |x: EntityId| if x == me { "you".to_string() } else { world.name_of(x) };
    let (by, on) = (who(b.by), who(b.on));
    let s = |v: &str, you: &str| if b.by == me { you.to_string() } else { v.to_string() };
    match b.blow {
        Blow::Miss => (format!("{by} {} at {on}: a miss", s("swings", "swing")), false),
        Blow::Hit(r, v) => {
            let (part, harm) = if crate::systems::robots::is_robot(world, b.on) {
                (robot_part(r), robot_harm(v))
            } else {
                (r.label(), v.label())
            };
            let whose = if b.on == me { "your".to_string() } else { format!("{on}'s") };
            (format!("{by} {} {whose} {part}: {harm}", s("hits", "hit")), v == Severity::Grave)
        }
        Blow::Broke(e) => {
            let what = match e {
                End::Fled => s("breaks and runs", "break and run"),
                End::Yielded => s("yields", "yield"),
                End::KnockedOut => s("is knocked out", "are knocked out"),
                End::Downed => s("goes down bleeding", "go down bleeding"),
                End::Dead => s("is killed", "are killed"),
                End::Wrecked => s("is wrecked", "are wrecked"),
                End::Won => s("wins", "win"),
            };
            (format!("{by} {what}"), true)
        }
        Blow::Joined => (format!("{by} {} in on {on}'s side", s("jumps", "jump")), true),
        Blow::Cover(c) => (format!("{by} {} cover ({c}%)", s("takes", "take")), false),
        Blow::Stuck => {
            (format!("{by} {} to break away from {on}, and {}", s("tries", "try"), s("can't", "can't")), false)
        }
        Blow::Refused => (format!("{on} won't take {}'s yield", if b.by == me { "your".into() } else { by }), true),
        Blow::Drew(true) => (format!("{by} {}", s("draws", "draw")), false),
        Blow::Drew(false) => (format!("{by} {}", s("holsters", "holster")), false),
        Blow::Stance(st) => (format!("{by} {} {}", s("goes", "go"), st.order_label()), false),
    }
}

/// C4: the beats of `world.tick - 1` (the tick just run) of the live
/// engagements and those that ended in it, each with the bodies who were in
/// its fight: the view's replay.
pub fn last_beats(world: &World) -> Vec<(Vec<EntityId>, Vec<BeatLog>)> {
    let t = world.tick.saturating_sub(1);
    let mut out = Vec::new();
    let of = |log: &std::collections::VecDeque<BeatLog>| -> Vec<BeatLog> {
        log.iter().filter(|b| b.tick == t).copied().collect()
    };
    for e in world.combat.engagements.values() {
        let b = of(&e.log);
        if !b.is_empty() {
            out.push((e.seen.clone(), b));
        }
    }
    for (seen, log) in &world.combat.ended {
        let b = of(log);
        if !b.is_empty() {
            out.push((seen.clone(), b));
        }
    }
    out
}

/// C4: the stance the human chose for the body's fights (`None`: its own).
pub fn stance_pref(world: &World, id: EntityId) -> Option<Stance> {
    orders_of(world, id).and_then(|o| o.stance)
}

/// C4: the human holstered the body's weapon (it fights bare-handed).
pub fn holstered(world: &World, id: EntityId) -> bool {
    orders_of(world, id).is_some_and(|o| o.holster)
}
