use super::*;

pub(super) fn guard_draw(world: &World, id: EntityId) -> u8 {
    let Some(job) = world.comp::<Job>(id).filter(|j| j.role == crate::components::Role::Guard) else { return 0 };
    let (share, t) = if crate::systems::law::job_is_private_guard(world, job) {
        match job.employer.and_then(|e| world.jobs2.arms.offices.get(&e)) {
            Some(&x) => x,
            None => return 0,
        }
    } else {
        world.jobs2.arms.law
    };
    if t == 0 || share <= 0.0 {
        return 0;
    }
    if hash(world, PURPOSE_GUARD, key(id), 0) < share {
        t
    } else {
        0
    }
}

/// The fight's tiers `(a, b)` when either side is armed (`None`: the
/// unarmed roll, exactly as before).
pub fn fight_tiers(world: &World, a: EntityId, b: EntityId) -> Option<(u8, u8)> {
    if !on(world) {
        return None;
    }
    let (ta, tb) = (tier(world, a), tier(world, b));
    (ta > 0 || tb > 0).then_some((ta, tb))
}

/// A raid's pairing strength and a Hunt's might per tier (0 unarmed).
pub fn might(world: &World, id: EntityId) -> f32 {
    if !on(world) {
        return 0.0;
    }
    let t = tier(world, id);
    if t == 0 {
        0.0
    } else {
        world.config.weapons.might_w * f32::from(t)
    }
}

/// After a fight (switch on): the counts, the winner's weapon's wear, and
/// the loser's carried weapons to the winner (a guard seizes them for the
/// Precinct; a robot takes nothing).
pub fn after_fight(world: &mut World, winner: EntityId, loser: EntityId, died: bool, tiers: Option<(u8, u8)>) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("weapons::after_fight");
    let row = ownership::jobs2_row(world);
    row.fights += 1;
    if died {
        row.fight_deaths += 1;
    }
    if tiers.is_some() {
        row.armed_fights += 1;
        if died {
            row.armed_deaths += 1;
        }
    }
    if tiers.is_none() {
        return;
    }
    // The winner's weapon wears.
    let wear = world.config.weapons.wear_per_fight;
    if wear > 0 {
        if let Some(w) = best_of(world, &carried_by(world, winner)) {
            let c = world.comp::<Asset>(w).map_or(0, |x| x.condition.saturating_sub(wear));
            if let Some(x) = world.comp_mut::<Asset>(w) {
                x.condition = c;
            }
            if c == 0 {
                assets::wreck(world, w, "worn out in a fight");
                world.jobs2.arms.index.remove(&w);
            }
        }
    }
    let taken = carried_by(world, loser);
    if taken.is_empty() || !world.has::<Brain>(winner) {
        return;
    }
    if crate::systems::law::is_guard(world, winner) {
        seize(world, loser, &taken);
        return;
    }
    for a in taken {
        let p = prov(world, ProvKind::Stolen, Some(winner));
        hand_over(world, a, Some(winner), AssetLoc::Carried(winner), p, true);
    }
}

/// The law takes `items` from `from` into the Precinct's stock (the city's).
fn seize(world: &mut World, from: EntityId, items: &[EntityId]) {
    let Some(p) = precinct(world) else { return };
    for &a in items {
        let pv = prov(world, ProvKind::Seized, Some(p));
        hand_over(world, a, None, AssetLoc::Stock(p), pv, false);
        ownership::jobs2_row(world).seized += 1;
    }
    world.jobs2.arms.licences.remove(&from);
}

/// `law::jail_suspect`: an arrest takes every weapon the suspect carries.
pub fn confiscate(world: &mut World, suspect: EntityId) {
    if !on(world) {
        return;
    }
    let items = carried_by(world, suspect);
    if !items.is_empty() {
        seize(world, suspect, &items);
    }
}

// ---------------------------------------------------------------------------
// Carrying, licences and the sightings (§ 4)
// ---------------------------------------------------------------------------

/// Licensed by the post (guards, Security staff) or the Clerk's register,
/// or holding a forged licence (Round 5 U5: the Gunsmith takes it).
pub fn licensed(world: &World, id: EntityId) -> bool {
    if world.jobs2.arms.licences.contains(&id) || world.jobs2.gray.licences.contains_key(&id) {
        return true;
    }
    world.comp::<Job>(id).is_some_and(|j| {
        j.role == crate::components::Role::Guard
            || j.employer
                .and_then(|e| world.comp::<Building>(e))
                .is_some_and(|b| matches!(b.kind, BuildingKind::SecurityOffice | BuildingKind::Gunsmith))
    })
}

/// A reason to want and carry a weapon: lawless, hunted or hunting, on a
/// contract's run, working a Pitch, robbed or beaten within `robbed_days`.
pub(super) fn has_reason(world: &World, id: EntityId) -> bool {
    let cfg = &world.config.weapons;
    if world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < cfg.want_lawless) {
        return true;
    }
    if world.hunted_by.contains_key(&id) || world.hunts.contains_key(&id) || world.contract_runs.contains_key(&id) {
        return true;
    }
    if world.jobs2.pitches.contains_key(&id) {
        return true;
    }
    // The pressure pass phase 2 (OPEN-176): the frightened (low safety).
    if crate::systems::customers::frightened(world, id) {
        return true;
    }
    let since = world.tick.saturating_sub(u64::from(cfg.robbed_days) * TICKS_PER_DAY);
    world.comp::<crate::components::Memory>(id).is_some_and(|m| {
        m.entries.iter().any(|e| e.tick >= since && matches!(e.kind, MemoryKind::WasRobbed | MemoryKind::Lost))
    })
}

pub(super) fn class_index(world: &World, id: EntityId) -> usize {
    match crate::systems::classes::class_of(world, id) {
        Class::Corp => 0,
        Class::Street => 1,
        Class::Dreg => 2,
    }
}

/// A free, living adult with a Body (no corpse, no sentence).
pub(super) fn free_adult(world: &World, id: EntityId) -> bool {
    world.has::<Identity>(id)
        && world.has::<Brain>(id)
        && !world.has::<Corpse>(id)
        && !world.has::<Sentence>(id)
        && crate::systems::demography::is_adult(world, id)
}

/// The concealment contest: the guard's chance to notice a carrier whose
/// stealth tier and pack tier hide a weapon of concealment term `conceal`
/// (its kind's, `combat::conceal_of`; C1, in place of the tier's `tier − 1`)
/// from a guard of `sight_tier`: `1 − contest_p(stealth + pack − conceal, sight, step)`.
pub fn notice_p(stealth: f32, pack: u8, conceal: f32, sight_tier: u8, step: f32) -> f32 {
    let hide = f32::from(crate::systems::security::thief_tier(stealth)) + f32::from(pack) - conceal;
    1.0 - crate::systems::security::contest_p(hide, f32::from(sight_tier.max(1)), step)
}

/// A guard's sight tier: 1, plus his Eyes' tier.
pub(super) fn sight_tier(world: &World, g: EntityId) -> u8 {
    let per = world.config.chrome.eyes_sight.max(1);
    let extra = world.comp::<Kit>(g).map_or(0, |k| k.sight) / per;
    1 + extra.min(2)
}

fn pack_tier(world: &World, id: EntityId) -> u8 {
    assets::assets_at(world, id)
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a))
        .filter(|x| x.kind == AssetKind::Pack && x.loc == AssetLoc::Carried(id) && x.condition > 0)
        .map(|x| x.tier)
        .max()
        .unwrap_or(0)
}

/// Hourly: every unlicensed carrier a guard sees rolls the concealment
/// contest (`crackdown_rolls` times where the law cracks down on the
/// carrier's gang); a notice files `UnlicensedArms` while under the day's cap.
pub fn sight_carriers(world: &mut World) {
    let cap = per_capita_day(world, world.config.weapons.ua_per_1000_day);
    if world.jobs2.arms.ua_today >= cap {
        return;
    }
    let mut carriers: Vec<EntityId> = world
        .jobs2
        .arms
        .index
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a))
        .filter_map(|x| match x.loc {
            AssetLoc::Carried(h) => Some(h),
            _ => None,
        })
        .collect();
    carriers.sort_unstable();
    carriers.dedup();
    if carriers.is_empty() {
        return;
    }
    let sight = world.config.crime.sight;
    let guards: Vec<(EntityId, crate::components::TilePos, Option<EntityId>, u32)> = world
        .guards()
        .iter()
        .filter_map(|&g| {
            let extra = world.comp::<Kit>(g).map_or(0, |k| u32::from(k.sight));
            world.comp::<Position>(g).map(|p| (g, p.tile, p.building, extra))
        })
        .collect();
    let hour = world.tick / TICKS_PER_HOUR;
    let step = world.config.weapons.conceal_step;
    for c in carriers {
        if world.jobs2.arms.ua_today >= cap {
            break;
        }
        // Round 5 U5 (OPEN-217): a forged licence is shown when the weapon
        // is noticed (`gray::licence_looked_at`), not waved through.
        let forged = world.jobs2.gray.licences.contains_key(&c);
        if !free_adult(world, c) || (licensed(world, c) && !forged) {
            continue;
        }
        let t = world.comp::<Kit>(c).map_or(0, |k| k.weapon);
        if t == 0 {
            continue;
        }
        let Some(pc) = world.comp::<Position>(c) else { continue };
        let (tile, building) = (pc.tile, pc.building);
        let Some(&(g, ..)) = guards.iter().find(|&&(g, gt, gb, extra)| {
            g != c && ((gb.is_some() && gb == building) || crate::systems::law::chebyshev(gt, tile) <= sight + extra)
        }) else {
            continue;
        };
        let stealth = crate::systems::law::stealth(world, c);
        let conceal = crate::systems::combat::conceal_of(world, c, t);
        let p = notice_p(stealth, pack_tier(world, c), conceal, sight_tier(world, g), step);
        // Jobs & lives phase 4 (§ 9): a dress-0 body in the Spire (1 off).
        let m = crate::systems::fashion::sighting_mult(world, c);
        let p = if m != 1.0 { (p * m).min(1.0) } else { p };
        let rolls = match world.gang_of(c) {
            Some(gang) if crate::systems::law::cracking_down_on(world, gang) => {
                world.config.weapons.crackdown_rolls.max(1)
            }
            _ => 1,
        };
        let seen = (0..rolls).any(|r| hash(world, PURPOSE_SIGHT, key(c), hour * 4 + u64::from(r)) < p);
        if !seen {
            continue;
        }
        if forged && !crate::systems::gray::licence_looked_at(world, c, g) {
            continue;
        }
        world.jobs2.arms.ua_today += 1;
        ownership::jobs2_row(world).ua_reports += 1;
        crate::systems::law::file_report(world, crate::components::Crime::UnlicensedArms, c, Some(g));
    }
}
