//! M12 the street rung (docs/M12_DISTRICTS.md § 2 "Vagrancy" and § 4).
//!
//! Phase 2 is the nightly Vagrancy sweep (plan D15): at 03:00, every rough
//! sleeper rolls against the law of the district they lie in. Phase 3 adds
//! Hotels, derelicts and squats to the same nightly pass (D22).
//!
//! O(residents) once a night; one `rng.world()` draw per rough sleeper, in
//! ascending id order.

use rand::Rng;

use crate::components::{
    Brain, BuildingKind, Crime, DistrictId, Household, Job, Lod, Position, Sentence, Stance, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::world::World;

/// The tick of day of the nightly pass (03:00).
pub const NIGHTLY_TOD: u16 = 180;
/// `District::vagrancy_log` keeps at most this many.
pub const VAGRANCY_LOG_CAP: usize = 64;

/// D22 (phase 2 body: Vagrancy only). Called by `districts::run` at 03:00
/// when `[law] district_beats` is on.
pub fn nightly(world: &mut World) {
    vagrancy(world);
}

/// Homeless adults with a Brain who are free (no Sentence, not cuffed) and
/// not emigrating, ascending. Phase 3 extends it: not booked in a Hotel,
/// not squatting.
pub fn rough_sleepers(world: &World) -> Vec<EntityId> {
    // scan-ok: nightly: rough sleepers
    world
        .citizens()
        .into_iter()
        .filter(|&a| world.comp::<Household>(a).is_some_and(|h| h.home.is_none()))
        .filter(|&a| world.comp::<Brain>(a).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating))
        .filter(|&a| !world.has::<Sentence>(a) && crate::systems::demography::is_adult(world, a))
        .collect()
}

/// A fine or a Vagrancy sentence in district `d`: the counter and the log.
pub fn note_vagrancy(world: &mut World, d: DistrictId) {
    world.stats.current.vagrancy += 1;
    let now = world.tick;
    if let Some(x) = world.districts.get_mut(d.index()) {
        if x.vagrancy_log.len() >= VAGRANCY_LOG_CAP {
            x.vagrancy_log.pop_front();
        }
        x.vagrancy_log.push_back(now);
    }
}

/// D15: the sweep probability for a rough sleeper in `d`.
pub fn vagrancy_p(world: &World, d: DistrictId) -> f32 {
    let cfg = &world.config.law;
    let dist = world.district(d);
    let sweep = if dist.stance == Stance::Sweep { cfg.sweep_mult } else { 1.0 };
    let curfew = if world.levers.curfew.get(d.index()).copied().unwrap_or(false) { cfg.curfew_mult } else { 1.0 };
    (cfg.vagrancy_base * dist.coverage * sweep * curfew).clamp(0.0, 1.0)
}

/// The on-shift, free city guard standing in `d` nearest `tile` (Manhattan,
/// ties the lower id); any tier.
fn nearest_guard_in(world: &World, d: DistrictId, tile: TilePos) -> Option<EntityId> {
    let tod = world.tick_of_day();
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| crate::systems::law::is_city_guard(world, g) && !world.has::<Sentence>(g))
        .filter(|&g| world.comp::<Job>(g).is_some_and(|j| j.on_shift(tod)))
        .filter_map(|g| world.comp::<Position>(g).map(|p| (g, p.tile)))
        .filter(|&(_, t)| world.district_of(t) == d)
        .min_by_key(|&(g, t)| (t.manhattan(tile), g))
        .map(|(g, _)| g)
}

/// D15: the nightly Vagrancy pass. Each rough sleeper not inside a building
/// (a Statistical one wherever it stands) rolls `vagrancy_p` of the district
/// it lies in. A hit: one who can pay the fine pays it to the Treasury; a
/// broke Statistical vagrant is jailed for the night if the Precinct has a
/// free cell (never at the expense of a convict: a full Precinct moves them
/// on); a broke Full or Coarse vagrant is reported by the nearest on-shift
/// city guard in the district (or by nobody: a sweep) and the arrest path
/// chases them. Also writes `District.rough` (rough sleepers per district).
pub fn vagrancy(world: &mut World) {
    let sleepers = rough_sleepers(world);
    let n = world.districts.len();
    let mut rough = vec![0u16; n];
    let mut rolls: Vec<(EntityId, TilePos, DistrictId)> = Vec::new();
    for a in sleepers {
        let Some(pos) = world.comp::<Position>(a) else { continue };
        let tile = pos.tile;
        let d = world.district_of(tile);
        if let Some(r) = rough.get_mut(d.index()) {
            *r = r.saturating_add(1);
        }
        let statistical = world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical);
        if pos.building.is_some() && !statistical {
            continue;
        }
        rolls.push((a, tile, d));
    }
    for (x, r) in world.districts.iter_mut().zip(&rough) {
        x.rough = *r;
    }
    let fine = world.config.law.vagrancy_fine;
    let capacity = usize::from(world.config.buildings.jail.capacity);
    for (a, tile, d) in rolls {
        let p = vagrancy_p(world, d);
        let roll: f32 = world.rng.world().random();
        if roll >= p {
            continue;
        }
        let name = world.name_of(a);
        let place = world.district_name(d).to_string();
        let coins = world.comp::<Wallet>(a).map_or(0, |w| w.coins);
        if fine > 0 && coins >= fine {
            let paid =
                crate::systems::ownership::pay(world, Some(a), None, fine, crate::systems::ownership::Flow::Fine);
            note_vagrancy(world, d);
            world.push_event(EventKind::Vagrancy, &[a], format!("{name} fined {paid} for sleeping rough in {place}"));
            continue;
        }
        let statistical = world.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical);
        if statistical {
            let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { continue };
            if world.sentenced().len() >= capacity {
                continue;
            }
            let until = world.tick + crate::systems::law::sentence_ticks(world, Crime::Vagrancy);
            world.vagrancy_places.insert(a, d);
            crate::systems::law::sentence(world, a, Crime::Vagrancy, until, jail);
            continue;
        }
        let witness = nearest_guard_in(world, d, tile);
        world.vagrancy_places.insert(a, d);
        crate::systems::law::file_report(world, Crime::Vagrancy, a, witness);
        let now = world.tick;
        world.last_seen.insert(a, (tile, now));
    }
}
