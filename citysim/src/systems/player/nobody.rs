use super::*;

/// One row of "pick a nobody": the body, its kind, and a one-line read of
/// who it is (age, district, coins, job or none, the ties that matter).
#[derive(Clone, Debug)]
pub struct Nobody {
    pub id: EntityId,
    pub kind: &'static str,
    pub line: String,
}

/// The kinds a nobody is picked by, one each, in this order.
const NOBODY_KINDS: [&str; 7] = [
    "sleeps rough",
    "squats",
    "waits at the labour corner",
    "out of work",
    "works for scraps",
    "in debt",
    "sick and broke",
];

/// Coins `id` owes `o` on their edge (negative: `o` owes `id`).
fn owes(world: &World, id: EntityId, o: EntityId) -> i64 {
    let d = world.edge(id, o).map_or(0, |e| i64::from(e.debt));
    if id < o {
        d
    } else {
        -d
    }
}

/// A free broke adult the human may take: 18 to `[slice] nobody_age_max`,
/// at most `nobody_coins`, nobody's exec, guard or player, not held (in a
/// cell, cuffs, a fight, knocked out, down, leaving). Statistical is fine:
/// the camera ring makes it Full.
fn nobody_ok(world: &World, id: EntityId) -> bool {
    let c = &world.config.slice;
    let Some(i) = world.comp::<Identity>(id) else { return false };
    let Some(b) = world.comp::<Brain>(id) else { return false };
    demography::is_adult(world, id)
        && u64::from(i.age_days) <= u64::from(c.nobody_age_max) * crate::time::DAYS_PER_YEAR
        && !world.has::<crate::components::Corpse>(id)
        && b.player.is_none()
        && b.cuffed_by.is_none()
        && !b.emigrating
        && !world.has::<crate::components::Sentence>(id)
        && !crate::systems::combat::engaged(world, id)
        && !crate::systems::combat::knocked_out(world, id)
        && !crate::systems::wounded::is_down(world, id)
        && !crate::systems::founding::is_exec(world, id)
        && !crate::systems::law::is_guard(world, id)
        && world.purse(Some(id)) <= c.nobody_coins
        // P5b: nobodies hold no steady job and no gang post
        && !world.has::<crate::components::GangMember>(id)
        && world.comp::<Job>(id).is_none_or(|j| j.wage_per_day <= c.nobody_wage_max)
}

fn nobody_kind(world: &World, id: EntityId, k: usize) -> bool {
    let job = world.has::<Job>(id);
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let squat = world.has::<Squatter>(id);
    match k {
        0 => home.is_none() && !squat && !job,
        1 => squat,
        2 => !job && crate::systems::streetwork::corner_ok(world, id),
        3 => !job && home.is_some(),
        4 => job,
        5 => world.neighbours(id).any(|o| owes(world, id, o) > 0),
        _ => world.comp::<crate::components::Health>(id).is_some_and(|h| {
            h.malnutrition >= 0.5 || h.sick_days != 0 || h.cancer.as_ref().is_some_and(|c| c.stage > 0.0)
        }),
    }
}

/// Where `id` lives or stands (its home's door, else its tile).
fn nobody_tile(world: &World, id: EntityId) -> Option<TilePos> {
    let home = world.comp::<Household>(id).and_then(|h| h.home).and_then(|h| world.comp::<Building>(h)).map(|b| b.door);
    home.or_else(|| world.comp::<Position>(id).map(|p| p.tile))
}

/// "Pick a nobody": up to `[slice] nobody_count` broke adults of the
/// running city, one per kind (sleeps rough, squats, the labour corner, out
/// of work, a gang's runner, the working poor, in debt, sick), the Sump's
/// first, then by index. Nobody under 18 (P5: the slice never possesses a
/// child).
pub fn nobodies(world: &World) -> Vec<Nobody> {
    let mut cands: Vec<(bool, EntityId)> = world
        .citizens()
        .into_iter()
        .filter(|&id| nobody_ok(world, id))
        .map(|id| {
            let sump = nobody_tile(world, id).is_some_and(|t| world.map.zone(t) == crate::components::Zone::Sump);
            (!sump, id)
        })
        .collect();
    cands.sort();
    let district = |id: EntityId| nobody_tile(world, id).map(|t| world.district_of(t));
    let mut out: Vec<Nobody> = Vec::new();
    for (k, kind) in NOBODY_KINDS.iter().enumerate() {
        if out.len() >= world.config.slice.nobody_count {
            break;
        }
        // P5b: a district with room on the list first (the Sump's first), else the first
        let per = world.config.slice.nobody_per_district;
        let fresh = |id: EntityId| out.iter().filter(|n| district(n.id) == district(id)).count() < per;
        let open = |id: EntityId| !out.iter().any(|n| n.id == id) && nobody_kind(world, id, k);
        let pick =
            cands.iter().find(|(_, id)| open(*id) && fresh(*id)).or_else(|| cands.iter().find(|(_, id)| open(*id)));
        if let Some(&(_, id)) = pick {
            out.push(Nobody { id, kind, line: nobody_line(world, id) });
        }
    }
    out
}

/// P5b: "Kowloon Steps (the Sump)" for a tile: its district, and its zone when the name does not say it.
pub fn area_of(world: &World, t: TilePos) -> String {
    use crate::components::Zone;
    let zone = match world.map.zone(t) {
        Zone::Spire => "the Spire",
        Zone::Civic => "Civic",
        Zone::Vats => "the Vats",
        Zone::Mid => "the Mid",
        Zone::Sump => "the Sump",
    };
    let d = world.district_name(world.district_of(t)).to_string();
    if d.contains(zone.trim_start_matches("the ")) {
        d
    } else {
        format!("{d} ({zone})")
    }
}

/// P5b: a building in plain words: "a block in Kowloon Steps (the Sump)" (a Feed goes by its name).
pub fn building_phrase(world: &World, b: EntityId) -> String {
    let Some(x) = world.comp::<Building>(b) else { return world.name_of(b) };
    if let Some(f) = x.feed.as_ref() {
        return f.name.clone();
    }
    format!("a {} in {}", x.kind.label().to_lowercase(), area_of(world, x.door))
}

/// P5b: the story log's text with every `Kind#N` building name put in plain words (`building_phrase`).
pub fn plain_buildings(world: &World, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(h) = rest.find('#') {
        let (pre, post) = rest.split_at(h);
        let label_len = pre.chars().rev().take_while(|c| c.is_ascii_alphabetic()).count();
        let (head, label) = pre.split_at(pre.len() - label_len);
        let digits: String = post[1..].chars().take_while(char::is_ascii_digit).collect();
        let found = if label.is_empty() || digits.is_empty() {
            None
        } else {
            let n: u32 = digits.parse().unwrap_or(u32::MAX);
            world
                .buildings_by_kind()
                .iter()
                .filter(|(k, _)| k.label() == label)
                .flat_map(|(_, v)| v.iter())
                .find(|b| b.index == n)
                .copied()
        };
        match found {
            Some(b) => {
                out.push_str(head);
                out.push_str(&building_phrase(world, b));
            }
            None => {
                out.push_str(pre);
                out.push('#');
                out.push_str(&digits);
            }
        }
        rest = &post[1 + digits.len()..];
    }
    out.push_str(rest);
    out
}

/// A nobody's one-line read: "Name, 34, Kowloon Steps (the Sump): 4c, no
/// job, sleeps rough; kin: X; owes Y 12c; a grudge on Z".
pub fn nobody_line(world: &World, id: EntityId) -> String {
    use crate::components::RelKind;
    let age = world.comp::<Identity>(id).map_or(0, |i| u64::from(i.age_days) / crate::time::DAYS_PER_YEAR);
    let at = nobody_tile(world, id);
    let place = at.map_or_else(|| "nowhere".to_string(), |t| area_of(world, t));
    let job = match world.comp::<Job>(id) {
        Some(j) => match j.employer {
            Some(e) => format!("{} at {}", j.role.label(), building_phrase(world, e)),
            None => j.role.label().to_string(),
        },
        None => "no job".to_string(),
    };
    let bed = if let Some(s) = world.comp::<Squatter>(id) {
        format!("squats in {}", building_phrase(world, s.building))
    } else if let Some(h) = world.comp::<Household>(id).and_then(|h| h.home) {
        format!("sleeps in {}", building_phrase(world, h))
    } else {
        "sleeps rough".to_string()
    };
    let mut ties: Vec<String> = Vec::new();
    if let Some(g) = world.comp::<crate::components::GangMember>(id) {
        let gang =
            world.comp::<crate::components::Gang>(g.gang).map_or_else(|| world.name_of(g.gang), |x| x.name.clone());
        ties.push(format!("runs for {gang}"));
    }
    let mut near: Vec<EntityId> = world.neighbours(id).filter(|&o| world.has::<Identity>(o)).collect();
    near.sort();
    let mut kin: Vec<String> = Vec::new();
    for &o in &near {
        let Some(e) = world.edge(id, o) else { continue };
        match e.kind {
            RelKind::Spouse => ties.push(format!("married to {}", world.name_of(o))),
            RelKind::Parent | RelKind::Family if kin.len() < 3 => kin.push(world.name_of(o)),
            _ => {}
        }
    }
    if !kin.is_empty() {
        ties.push(format!("kin: {}", kin.join(", ")));
    }
    for &o in &near {
        let d = owes(world, id, o);
        if d > 0 && ties.len() < 5 {
            ties.push(format!("owes {} {d}c", world.name_of(o)));
        } else if d < 0 && ties.len() < 5 {
            ties.push(format!("is owed {}c by {}", -d, world.name_of(o)));
        }
    }
    if let Some(g) = world.comp::<crate::word::Grudges>(id).and_then(|g| {
        g.list.iter().filter(|x| x.settled.is_none()).max_by(|a, b| a.weight.total_cmp(&b.weight)).cloned()
    }) {
        ties.push(format!("a grudge on {}", world.name_of(g.target)));
    }
    if let Some(&o) = near.iter().find(|&&o| world.edge(id, o).is_some_and(|e| e.kind == RelKind::Enemy)) {
        if ties.len() < 6 {
            ties.push(format!("an enemy: {}", world.name_of(o)));
        }
    }
    let ties = if ties.is_empty() { "nobody close".to_string() } else { ties.join("; ") };
    format!("{}, {age}, {place}: {}c, {job}, {bed}; {ties}", world.name_of(id), world.purse(Some(id)))
}

/// "Arrive fresh": an immigrant onto a Sump street (beside the door of a
/// Sump Market, Mission or Bar, picked keyed on the tick), homeless, with
/// the arrival's coins (`demography::arrive_at`) and nobody who knows them.
pub fn arrive_fresh(world: &mut World) -> EntityId {
    let mut tiles: Vec<TilePos> = [BuildingKind::Market, BuildingKind::Mission, BuildingKind::Bar]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter_map(|b| world.comp::<Building>(b).filter(|x| !x.demolished).map(|x| x.door))
        .filter(|&d| world.map.zone(d) == crate::components::Zone::Sump)
        .filter_map(|d| {
            let (x, y) = (i32::from(d.x), i32::from(d.y));
            [(0, 1), (1, 0), (0, -1), (-1, 0)].iter().find_map(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                let t = TilePos { x: u16::try_from(nx).ok()?, y: u16::try_from(ny).ok()? };
                (usize::from(t.x) < world.map.w() && usize::from(t.y) < world.map.h() && world.is_street(t))
                    .then_some(t)
            })
        })
        .collect();
    tiles.sort();
    tiles.dedup();
    let tile = if tiles.is_empty() {
        None
    } else {
        use rand::Rng;
        let i = world.rng.keyed(0x5_1ACE ^ world.tick).random_range(0..tiles.len());
        Some(tiles[i])
    };
    demography::arrive_at(world, tile)
}
