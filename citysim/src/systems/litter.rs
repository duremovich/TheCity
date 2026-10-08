//! M12 litter (docs/M12_DISTRICTS.md § 3, plan D16-D19): one byte per tile,
//! written only by events (a crime, a brawl, a death, an eviction, a
//! derelict, a squat, a rough sleeper), decayed and averaged per district
//! once a day, cleaned by the city's sweepers, owners and the ruling gang.
//!
//! Nothing here runs per agent per tick: a deposit is one event, the decay
//! and the district means are one pass over the grid at midnight, and the
//! movement delay (`exec::litter_delay`) is one byte read on a step already
//! being taken. Flow fields never change; only the rubble hook (255, off in
//! M12) touches walkability.

use rand::Rng;

use crate::components::{Building, DistrictId, TilePos};
use crate::entity::EntityId;
use crate::world::World;

/// The rubble value (the `Damage` hook, D19). Deposits stop at 254.
pub const RUBBLE: u8 = 255;
/// Deposits saturate here: litter never becomes rubble.
pub const MAX_LITTER: u8 = 254;

/// The band of a litter value: 0 clean (0-31), 1 littered (32-95), 2 trashed
/// (96-191), 3 heaped (192-254); rubble reads 3.
pub fn band(v: u8) -> usize {
    match v {
        0..=31 => 0,
        32..=95 => 1,
        96..=191 => 2,
        _ => 3,
    }
}

/// The band's name, for the panel.
pub fn band_label(b: usize) -> &'static str {
    ["clean", "littered", "trashed", "heaped"][b.min(3)]
}

/// Litter is on (`[litter] enabled`) and the grid exists.
pub fn enabled(world: &World) -> bool {
    world.config.litter.enabled && !world.litter.is_empty()
}

fn index(world: &World, p: TilePos) -> usize {
    usize::from(p.y) * world.map.w() + usize::from(p.x)
}

/// The litter on a tile (0 off the grid).
pub fn at(world: &World, p: TilePos) -> u8 {
    world.litter.get(index(world, p)).copied().unwrap_or(0)
}

/// Fix pass (phase 3 review): where litter from an event at `tile` lands.
/// A street tile is itself; an event inside a building (`inside`: an indoor
/// theft, a death in a Home, a raid at its door) lands at that building's
/// outside door, so an indoor event still marks the street. M12 review: no
/// building scan; with no building named, a tile next to the street (a door)
/// lands on its first street neighbour (`neighbours4` order). Anything else
/// (a wall, a demolished Home's ground) is left as it is and the deposit's
/// street filter decides.
pub fn street_anchor(world: &World, tile: TilePos, inside: Option<EntityId>) -> TilePos {
    if world.is_street(tile) {
        return tile;
    }
    if let Some(bd) = inside.and_then(|b| world.comp::<Building>(b)).filter(|bd| !bd.demolished) {
        return world.outside_door(bd);
    }
    world.map.neighbours4(tile).find(|&n| world.is_street(n)).unwrap_or(tile)
}

/// M12 review: the street tiles (walkable, inside no building) within
/// Chebyshev `r` of `centre`, row by row, each with its ring `k`.
pub fn street_tiles_around(world: &World, centre: TilePos, r: u8) -> impl Iterator<Item = (TilePos, i32)> + '_ {
    let (w, h) = (world.map.w() as i32, world.map.h() as i32);
    let r = i32::from(r);
    let (cx, cy) = (i32::from(centre.x), i32::from(centre.y));
    (-r..=r)
        .flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(move |&(dx, dy)| {
            let (x, y) = (cx + dx, cy + dy);
            x >= 0 && y >= 0 && x < w && y < h
        })
        .map(move |(dx, dy)| (TilePos { x: (cx + dx) as u8, y: (cy + dy) as u8 }, dx.abs().max(dy.abs())))
        .filter(move |&(p, _)| world.is_street(p))
}

/// D16: `amount` at `tile` (a street tile, or a door's: see
/// [`street_anchor`]); [`deposit_near`] names the building an indoor event
/// happened in.
pub fn deposit(world: &mut World, tile: TilePos, amount: u8, r: u8) {
    deposit_near(world, tile, None, amount, r);
}

/// D16: `amount` at `tile` (mapped to the street by [`street_anchor`] with
/// the building `inside`, if the event happened in one), scaled by
/// `[litter] deposit_mult` (fix pass; capped at 254), spread to Chebyshev
/// radius `r` with half the amount at the rim (`round(amount × (1 − k ÷
/// 2r))` at ring `k`); only street tiles (walkable, inside no building) take
/// it, saturating at 254. Rubble is left as it is.
pub fn deposit_near(world: &mut World, tile: TilePos, inside: Option<EntityId>, amount: u8, r: u8) {
    if !enabled(world) || amount == 0 {
        return;
    }
    let tile = street_anchor(world, tile, inside);
    let amount = (f32::from(amount) * world.config.litter.deposit_mult.max(0.0)).min(f32::from(MAX_LITTER));
    let rr = i32::from(r);
    let adds: Vec<(usize, u16)> = street_tiles_around(world, tile, r)
        .map(|(p, k)| {
            let add = if rr == 0 { amount } else { amount * (1.0 - k as f32 / (2 * rr) as f32) };
            (index(world, p), add.round().clamp(0.0, 255.0) as u16)
        })
        .collect();
    for (i, add) in adds {
        if let Some(v) = world.litter.get_mut(i) {
            if *v != RUBBLE {
                *v = (u16::from(*v) + add).min(u16::from(MAX_LITTER)) as u8;
            }
        }
    }
}

/// D17: a Statistical crime's litter lands on a street tile of its district
/// drawn from `SimRng::keyed(key)` (the hole's id), so neither the world
/// stream nor an agent's moves and a replay lands it on the same tile.
pub fn deposit_in_district(world: &mut World, d: DistrictId, amount: u8, r: u8, key: u64) {
    if !enabled(world) {
        return;
    }
    let tile = {
        let Some(dist) = world.districts.get(d.index()) else { return };
        if dist.streets.is_empty() {
            return;
        }
        let k = world.rng.keyed(key).random_range(0..dist.streets.len());
        let i = dist.streets[k] as usize;
        let w = world.map.w();
        TilePos { x: (i % w) as u8, y: (i / w) as u8 }
    };
    deposit(world, tile, amount, r);
}

/// Midnight: every tile in 1..=254 loses `[litter] decay`.
pub fn decay(world: &mut World) {
    let d = world.config.litter.decay;
    if d == 0 {
        return;
    }
    for v in world.litter.iter_mut() {
        if *v != 0 && *v != RUBBLE {
            *v = v.saturating_sub(d);
        }
    }
}

/// Midnight: each district's litter, the share of its street tiles at or
/// above `[litter] visible` (32, the littered band; fix pass, phase 3
/// review: the mean over ~30k street tiles pinned every district near 0).
/// 0.30 reads "three street tiles in ten are visibly littered or worse".
pub fn district_means(world: &mut World) {
    let visible = world.config.litter.visible.max(1);
    let shares: Vec<f32> = world
        .districts
        .iter()
        .map(|d| {
            if d.streets.is_empty() {
                return 0.0;
            }
            let dirty =
                d.streets.iter().filter(|&&i| world.litter.get(i as usize).is_some_and(|&v| v >= visible)).count();
            dirty as f32 / d.streets.len() as f32
        })
        .collect();
    for (d, m) in world.districts.iter_mut().zip(shares) {
        d.litter = m;
    }
}

/// Remove up to `units` from `tiles` (grid indices), dirtiest first (ties
/// the lower index): each tile is zeroed in turn, the last one partly.
/// Rubble costs `rubble_clean_mult` units a point. Returns the units used.
fn clean_tiles(world: &mut World, mut tiles: Vec<u32>, units: u32) -> u32 {
    let mult = world.config.litter.rubble_clean_mult.max(1);
    tiles.retain(|&i| world.litter.get(i as usize).is_some_and(|&v| v > 0));
    tiles.sort_by_key(|&i| (std::cmp::Reverse(world.litter[i as usize]), i));
    let mut left = units;
    let mut rubble_cleared = Vec::new();
    for i in tiles {
        if left == 0 {
            break;
        }
        let v = world.litter[i as usize];
        let per = if v == RUBBLE { mult } else { 1 };
        let cost = u32::from(v) * per;
        if cost <= left {
            left -= cost;
            world.litter[i as usize] = 0;
            if v == RUBBLE {
                rubble_cleared.push(i);
            }
        } else {
            if v == RUBBLE {
                // Rubble goes in one piece or not at all.
                continue;
            }
            world.litter[i as usize] = v - left as u8;
            left = 0;
        }
    }
    let w = world.map.w();
    for i in rubble_cleared {
        let p = TilePos { x: (i as usize % w) as u8, y: (i as usize / w) as u8 };
        unblock(world, p);
    }
    units - left
}

/// D23: the sweepers' credit: `units` off district `d`'s dirtiest street tiles.
pub fn clean_dirtiest(world: &mut World, d: DistrictId, units: u32) -> u32 {
    if units == 0 {
        return 0;
    }
    let tiles = world.districts.get(d.index()).map(|x| x.streets.clone()).unwrap_or_default();
    clean_tiles(world, tiles, units)
}

/// L2 L8 (pure): district `d`'s dirtiest street tile, ties nearest its
/// centroid, then the lower grid index; with no litter, the street tile
/// nearest the centroid.
pub fn dirtiest_tile(world: &World, d: DistrictId) -> Option<TilePos> {
    let dist = world.districts.get(d.index())?;
    let w = world.map.w();
    let centroid = dist.centroid;
    dist.streets
        .iter()
        .map(|&i| {
            let p = TilePos { x: (i as usize % w) as u8, y: (i as usize / w) as u8 };
            let v = world.litter.get(i as usize).copied().unwrap_or(0);
            (std::cmp::Reverse(v), p.manhattan(centroid), i, p)
        })
        .min()
        .map(|(_, _, _, p)| p)
}

/// D24: `units` off the street tiles within Chebyshev `r` of `door`.
pub fn clean_around(world: &mut World, door: TilePos, r: u8, units: u32) -> u32 {
    if units == 0 {
        return 0;
    }
    let tiles: Vec<u32> = street_tiles_around(world, door, r).map(|(p, _)| index(world, p) as u32).collect();
    clean_tiles(world, tiles, units)
}

/// The litter units within Chebyshev `r` of `door` (the owner's bill).
pub fn units_around(world: &World, door: TilePos, r: u8) -> u32 {
    street_tiles_around(world, door, r).map(|(p, _)| u32::from(at(world, p))).sum()
}

fn unblock(world: &mut World, p: TilePos) {
    if world.map.blocked.remove(&p) {
        world.invalidate_flow_fields();
    }
}

/// D19, the `Damage` hook (off in M12): only with `[litter] rubble_blocks`,
/// rubble is written (255, the tile blocked) or cleared (0, unblocked), and
/// the flow fields are dropped as on any wall change. Otherwise a no-op.
pub fn set_rubble(world: &mut World, tile: TilePos, on: bool) {
    if !world.config.litter.rubble_blocks || world.litter.is_empty() {
        return;
    }
    let i = index(world, tile);
    let Some(v) = world.litter.get_mut(i) else { return };
    if on {
        *v = RUBBLE;
        world.map.blocked.insert(tile);
        world.invalidate_flow_fields();
    } else {
        *v = 0;
        unblock(world, tile);
    }
}

/// The litter grid in a save: `(run, value)` pairs (the grid is mostly
/// zero runs: tens of pairs, not 48 KB).
pub mod rle {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        let mut runs: Vec<(u32, u8)> = Vec::new();
        for &b in v {
            match runs.last_mut() {
                Some((n, x)) if *x == b => *n += 1,
                _ => runs.push((1, b)),
            }
        }
        runs.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let runs = Vec::<(u32, u8)>::deserialize(d)?;
        let mut out = Vec::with_capacity(runs.iter().map(|&(n, _)| n as usize).sum());
        for (n, b) in runs {
            out.extend(std::iter::repeat_n(b, n as usize));
        }
        Ok(out)
    }
}
