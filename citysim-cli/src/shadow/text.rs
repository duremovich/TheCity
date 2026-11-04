use super::*;

// ---------------------------------------------------------------------------
// Names and places
// ---------------------------------------------------------------------------

pub(super) fn clock(tick: Tick) -> String {
    let t = tick % TICKS_PER_DAY;
    format!("d{} {:02}:{:02}", tick / TICKS_PER_DAY, t / TICKS_PER_HOUR, t % TICKS_PER_HOUR)
}

/// The view milestone V9 (OPEN-255): the El lines of `id` this tick, against
/// the ride seen last: the ride filed (the walk to the platform), each leg
/// boarded, the alighting.
pub(super) fn el_lines(
    world: &World,
    id: EntityId,
    seen: &mut Option<((Tick, u8), u8, bool, citysim::components::TilePos)>,
    now: Tick,
) -> Vec<String> {
    let net = &world.transit.net;
    let stop = |s: u16| {
        let tile = net.stops[usize::from(s)];
        format!("the stop at ({}, {}) in {}", tile.x, tile.y, world.district(world.district_of(tile)).name)
    };
    let line_name = |l: u8| net.lines.get(usize::from(l)).map_or("?".to_string(), |l| l.name.clone());
    let mut out = Vec::new();
    let Some(ride) = world.rides.get(&id) else {
        if let Some((_, _, true, tile)) = seen.take() {
            let d = world.district(world.district_of(tile)).name.clone();
            out.push(format!("EL off at the stop at ({}, {}) in {d}, walking on", tile.x, tile.y));
        }
        return out;
    };
    let key = (ride.plan_started, ride.plan_step);
    let aboard = matches!(
        world.comp::<Brain>(id).map(|b| &b.exec),
        Some(ExecState::Ride { depart, .. }) if now >= *depart
    );
    if seen.is_none_or(|s| s.0 != key) {
        let legs = ride.legs.len().max(1);
        out.push(format!(
            "EL ride to {}: on foot to {}, the {} at {}, off at {} at {} ({legs} leg{}, {})",
            place_of_target(world, &ride.target),
            stop(ride.board),
            line_name(ride.line),
            hm(ride.depart),
            stop(ride.alight),
            hm(ride.arrive),
            if legs == 1 { "" } else { "s" },
            if ride.dodge { "jumping the gate" } else { "paying the fare" }
        ));
        *seen = Some((key, u8::MAX, false, net.stops[usize::from(ride.alight)]));
    }
    if let Some(s) = seen.as_mut() {
        if aboard && (!s.2 || s.1 != ride.leg) {
            let stops = ride.legs.get(usize::from(ride.leg)).map(|l| l.from.abs_diff(l.to));
            out.push(format!(
                "EL aboard the {} at {}{}",
                line_name(ride.line),
                stop(ride.board),
                stops.map_or(String::new(), |n| format!(", {n} stop{} on", if n == 1 { "" } else { "s" }))
            ));
        }
        s.1 = ride.leg;
        s.2 = s.2 || aboard;
    }
    out
}

/// A `GoTo` target as the diary names it: its building, else its tile.
fn place_of_target(world: &World, target: &citysim::exec::GotoTarget) -> String {
    match target.building {
        Some(b) => bname(world, b),
        None => format!("({}, {})", target.tile.x, target.tile.y),
    }
}

pub(super) fn hm(tick: Tick) -> String {
    let t = tick % TICKS_PER_DAY;
    format!("{:02}:{:02}", t / TICKS_PER_HOUR, t % TICKS_PER_HOUR)
}

/// A building as `Label#index`.
pub(super) fn bname(world: &World, b: EntityId) -> String {
    match world.comp::<Building>(b) {
        Some(bl) => format!("{}#{}", bl.label.as_deref().unwrap_or_else(|| bl.kind.label()), b.index),
        None => world.name_of(b),
    }
}

/// A duration as "1 h", "1 h 20" or "38 min".
fn span(t: u64) -> String {
    match (t / 60, t % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02}"),
    }
}

/// Round 4 T2 (OPEN-232): the `[family]` line's why for a child's place
/// today: the school run (walked or by the El), or why it is home on a
/// school day (no School in reach, full, the purse), too young, no school
/// today. Empty in a Camp or on the street.
pub(super) fn school_why(world: &World, c: EntityId, now: u64) -> String {
    use citysim::components::{Routine, SchoolMiss};
    use citysim::systems::ages;
    let Some(k) = world.comp::<Child>(c) else { return String::new() };
    let cfg = &world.config.ages;
    let door = world
        .comp::<citysim::Household>(c)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door);
    let roll = |s: EntityId| {
        let on = world.jobs2.ages.rolls.get(&s).copied().unwrap_or(0);
        format!("{on} of {} on the roll", ages::roll_cap(world, s))
    };
    match (k.routine, k.school_miss) {
        (Routine::School, _) => {
            let Some((s, d)) = k.school.zip(door) else { return String::new() };
            let Some(sd) = world.comp::<Building>(s).map(|b| b.door) else { return String::new() };
            let (t, el) = ages::school_trip(world, d, sd);
            if el {
                format!(" ({} by the El, the fares paid)", span(t))
            } else {
                format!(" ({} walk)", span(t))
            }
        }
        (Routine::Camp | Routine::Street, _) => String::new(),
        (_, Some(SchoolMiss::Far)) => format!(": no School within {}", span(cfg.school_trip_ticks)),
        (_, Some(SchoolMiss::Full)) => match k.school {
            Some(s) if ages::roll_cap(world, s) == 0 => format!(": {} has no teacher", nm(world, s)),
            Some(s) => format!(": {} is full, {}", nm(world, s), roll(s)),
            None => ": every School in reach is full".to_string(),
        },
        (_, Some(SchoolMiss::Purse)) => {
            let s = k.school;
            let el =
                s.zip(door).and_then(|(s, d)| world.comp::<Building>(s).map(|b| ages::school_trip(world, d, b.door).1));
            let fee = if world.levers.public_school { 0 } else { cfg.school_fee };
            let fares = if el == Some(true) { world.config.transit.fare * cfg.school_el_fares } else { 0 };
            let at = s.map_or("School".to_string(), |s| nm(world, s));
            // Round 7 W5 (`OPEN-237`): the purse the midnight pick read
            // (`school_parent_48165 d22 09:30`: "beyond the purse" with the
            // morning's 10c in it; at midnight 4c, the rent's week kept).
            let purse = match k.school_purse {
                Some((held, kept)) if kept > 0 => {
                    format!("the purse at midnight ({held}c in hand, {kept}c of it kept for the rent and meals)")
                }
                Some((held, _)) => format!("the purse at midnight ({held}c in hand)"),
                None => "the purse".to_string(),
            };
            match (fee, fares) {
                (_, 0) => format!(": the {fee}c fee at {at} was beyond {purse}"),
                (0, f) => format!(": the El's {f}c of fares to {at} were beyond {purse}"),
                (fee, f) => format!(": the {fee}c fee and the El's {f}c of fares to {at} were beyond {purse}"),
            }
        }
        (_, None) => {
            let yrs = world.comp::<Identity>(c).map_or(0, |i| i.age_years());
            if yrs < cfg.school_from_years {
                format!(", under {} for School", cfg.school_from_years)
            } else if (now / 1440) % 7 >= 5 {
                ", no School today".to_string()
            } else {
                String::new()
            }
        }
    }
}

/// An agent as `Name#index`, a building, a gang or a corp by name.
pub(super) fn nm(world: &World, id: EntityId) -> String {
    if id == EntityId::NONE {
        return "?".into();
    }
    if let Some(i) = world.comp::<Identity>(id) {
        return format!("{}#{}", i.name, id.index);
    }
    if let Some(g) = world.comp::<Gang>(id) {
        return format!("gang {}", g.name);
    }
    if let Some(c) = world.comp::<Corp>(id) {
        return format!("corp {}", c.name);
    }
    if world.has::<Building>(id) {
        return place_of_building(world, id);
    }
    world.name_of(id)
}

pub(super) fn place_of_building(world: &World, b: EntityId) -> String {
    let d = world.district_of_building(b);
    format!("{} ({})", bname(world, b), world.district_name(d))
}

/// The city milestone C6a: where on its structure a building stands: ", floors 1-30", ", floor 0 of Block#77" (a
/// tenancy); "" for a building with no span (the 2k city).
pub(super) fn floors_of(world: &World, b: EntityId) -> String {
    let s = citysim::systems::floors::span_text(world, b);
    if s.is_empty() {
        return s;
    }
    match world.comp::<Building>(b).and_then(|bd| bd.within) {
        Some(h) => format!(", {s} of {}", bname(world, h)),
        None => format!(", {s}"),
    }
}

pub(super) fn place(world: &World, id: EntityId) -> String {
    match world.comp::<Position>(id) {
        // C6a: the storey he stands on, in a building with floors.
        Some(p) if p.building.is_some_and(|b| world.comp::<Building>(b).is_some_and(|bd| bd.span.is_some())) => {
            let b = p.building.unwrap_or(EntityId::NONE);
            let host = world
                .comp::<Building>(b)
                .and_then(|bd| bd.within)
                .map_or(String::new(), |h| format!(" of {}", bname(world, h)));
            format!("{}, floor {}{host}", place_of_building(world, b), p.floor)
        }
        Some(p) => match p.building {
            Some(b) => place_of_building(world, b),
            None => format!("street ({})", world.district_name(world.district_of(p.tile))),
        },
        None => "nowhere".into(),
    }
}

pub(super) fn owner_name(world: &World, b: EntityId) -> Option<String> {
    let o = world.comp::<Building>(b)?.owner?;
    Some(nm(world, o))
}

pub(super) fn money(c: i64) -> String {
    format!("{c}c")
}

pub(super) fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
