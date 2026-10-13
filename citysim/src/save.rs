//! Whole-world serialisation with serde + RON. Everything on `World` is
//! saved except the flow-field cache, which is rebuilt lazily.

use std::path::{Path, PathBuf};

use crate::world::World;

/// The save format, bumped when the saved shape changes (`World::save_version`).
/// 4: the Real economy (the World's books, `wage_rev` and the revenue windows,
/// Missions, Camps, the till). Older saves are not loaded: the legacy
/// migrations retired with the off switches (2026-10-09). 5: Jobs and room P2
/// (`Building.floors`, `floor_days`, `MapBuilding.floors`; serde defaults).
/// 6: Jobs and room P3 (J32): `Role::Trade` by key (`Trade("night_porter")`,
/// resolved against the save's own `Config::trades`; an unknown key fails
/// the load) and the `trades` rows in the saved config. 7: Hunger as
/// sickness (`World.health`, `Inventory.synth`, `Building.synth_food`,
/// `Market.synth_frac`, `Charity.synth_frac`, `DayRow.health`; serde
/// defaults, none written with `[sickness]` off).
/// 8: the Hit market (docs/HIT_MARKET.md H22): `Contract.deposit`,
/// `Contract.motive`, `Corp.last_wronged`, `World::hm_corp_hits`,
/// `World::wants`, `World::hm_debts`, `[hitmarket]` in the saved config
/// (serde defaults).
/// 9: the Hit market's gaps pass: `World::hm_stiffs` (the stiffed gun's
/// grace) and `[hitmarket] reach_premium`, `lawful_wait`, `queue_hours` in
/// the saved config (serde defaults).
/// 10: P10 (docs/P10_POLICY.md): the policy levers `Levers.{levy,
/// school_meals, reserve_release}`, `CityBudget.{dry_days, short_today}`
/// and `[policy]` in the saved config (serde defaults: the shipped no-net
/// market city); the saved config's retired `[economy2] wages` /
/// `no_safety_net`, `[sickness] enabled`, `[street] shanties` and
/// `[demography] school_meals` keys are ignored on load. An old save's dole
/// and public works levers load as they were saved; its levy and school
/// meals load off (an old dole-city save resumes under the market wage law).
/// 11: transit, the El (docs/TRANSIT.md): `World::transit` (closures, the day's
/// load, `rode`, the crews), `World::rides`, `DayRow.transit`,
/// `StatTable.p_dodge_caught` and `[transit]` in the saved config (serde
/// defaults, none written with the El off; a pre-transit save loads with
/// its saved config, so the El stays off for it).
/// 12: Scale S2 (docs/SCALE_20K.md): `TilePos`, `Rect`, `Building.capacity`
/// / `full_capacity`, the `guard_count` / `immigration_per_week` /
/// `sanitation_count` levers, `District.guards` / `sweepers` and
/// `FeedState.covers` widened (RON writes numbers, so an older save's
/// values read as they were); `MAX_DISTRICTS` 12 -> 32, the per-district
/// arrays (`Levers.guard_weight`, `stance_pin`, `curfew`,
/// `sanitation_weight`, `DistrictWatch`, `EconCols.d_*`) read a shorter
/// saved tuple padded (the weights with 1.0, the rest with their default);
/// the packed `DayTrace` keeps a district under 16 where it was.
/// 13: Scale S3b (docs/SCALE_20K.md): the map's grids are run-length strings
/// (`Map` saves `tiles_rle` / `zones_rle`: the whole map 1.3 MB for the
/// 3,000 x 1,875 import against ~62 MB of enum names, 61 KB for the 2k map
/// against ~550 KB), plus the map's own `districts`, `district_rle` and
/// `lines` (serde defaults). A save before 13 carries `tiles` / `zones` as
/// one enum name per tile and loads as it did (`map::MapRepr`).
/// 14: Scale S5 (docs/SCALE_20K.md § S5): `World::ambient` (the Ambient
/// Homes, the district aggregates, the donors, the counters), `[ambient]` in
/// the saved config (both skipped when empty or default: a world without
/// Ambient flats saves as before), `DayRow.ambient`, `EventKind::Materialised`.
/// 15: the Scale review fixes: `[scale] fitted` in the saved config (a
/// fitted config is not fitted again; an older save's reads `false`, so a
/// replay from it fits once more, as before), `Corp.slot` a `u16` (RON
/// writes numbers: an older save reads as it was), `World::landmark_base`
/// (the big-map landmarks' tiles while a closure left them older than the
/// map's; skipped when `None`, always on the 2,000 city), and `[transit]
/// staff_per_line` uncapped when missing (an older config keeps its crews).
/// 16: M16b phase 1, leverage (docs/M16_CONTRACTS.md § 4):
/// `World::{coercions, next_coercion, lev_tried, fake_jobs}`,
/// `DayRow.leverage`, `[leverage]` and the `persuade` keys in the saved
/// config (serde defaults: an older save loads with no coercions); the
/// appended `ContractKind::Persuade`, `Terms::Coerced`, `Origin::Court`,
/// `Crime::{Fraud, Blackmail}`, `Deed::Blackmailed` and their events.
/// 17: M16b phase 2, captives and the new kinds: `World::{captives,
/// refusals, steal_runs, feed_shamed}`, `Contract.hold`,
/// `Story.{fabricated, exposed}`, `DayRow.captive`, `[hostage]` and the
/// phase-2 kind keys in the saved config (serde defaults: an older save
/// loads with nobody held); the appended `BuildingKind::Prison`, the six
/// `ContractKind`s, `Target::{Carry, Story}`, `Origin::{Bury, Spin}`,
/// `Leverage::Hostage`, `Demand::{Sell, Release}`, `Deed::Rescued`,
/// `ActionKind::HandOver`, `RunWhy::Contract` and the four events.
/// 18: M16b phase 3, the wounded: `World::wounded`, `DayRow.wounded`,
/// `[wounded]`, `[trauma]` and the `trauma` kind keys in the saved config
/// (serde defaults: an older save loads with nobody down); the appended
/// `GoalKind::Rescue`, `ActionKind::{CarryWounded, Stabilise}`,
/// `ContractKind::Trauma`, `ContractStatus::Standing`,
/// `Origin::Subscription`, `Flow::Trauma` and the six events.
/// 19: M16b phase 4, boards, tags and attention: `Governance::Board`'s
/// `chair`, `last_vote`, `votes`; `World::{tags, next_tag, gang_scanners,
/// attention}`; `Levers.prison_permit`; `DayRow.gov`; `[governance]`,
/// `[tags]`, `[attention]` and the `tag`/`vote` kind keys in the saved
/// config (serde defaults); the appended `ContractKind::{Tag, Vote}`,
/// `Origin::Board`, `Demand::Vote`, `CoercionWhy::Vote`, the five events
/// and the eight god commands.
pub const SAVE_VERSION: u8 = 19;

/// Compact RON of the whole world. Same world state ⇒ same bytes.
pub fn to_ron(world: &World) -> String {
    ron::to_string(world).expect("World is always serialisable")
}

pub fn from_ron(text: &str) -> Result<World, ron::error::SpannedError> {
    // J32: a `Trade("key")` resolves against the trades of the config this
    // save carries (read before any Job): none until that config is read.
    crate::components::TradeId::set_keys(Vec::new());
    let mut world: World = ron::from_str(text)?;
    // Review finding 10: the landmarks the saved world searched with.
    let base = world.landmark_base.as_ref().map(|b| b.0.clone()).filter(|t| t.len() == world.map.tiles().len());
    world.flow_fields.set_landmark_base(base);
    world.resize_stores();
    world.rebuild_indices();
    // M12 D1: the district grid and adjacency are not saved.
    crate::systems::districts::rebuild(&mut world);
    // Transit: the derived network (not saved).
    crate::systems::transit::build(&mut world);
    world.save_version = SAVE_VERSION;
    world.reload_names();
    Ok(world)
}

/// `saves/<seed>-<tick>.ron`.
pub fn save_path(dir: &Path, seed: u64, tick: u64) -> PathBuf {
    dir.join(format!("{seed}-{tick}.ron"))
}

pub fn save_to_file(world: &World, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, to_ron(world))
}

pub fn load_from_file(path: &Path) -> Result<World, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    from_ron(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The save with the highest tick for `seed` in `dir`, if any.
pub fn newest_save(dir: &Path, seed: u64) -> Option<PathBuf> {
    let prefix = format!("{seed}-");
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(rest) = name.strip_prefix(&prefix).and_then(|r| r.strip_suffix(".ron")) else { continue };
        let Ok(tick) = rest.parse::<u64>() else { continue };
        if best.as_ref().is_none_or(|(t, _)| tick > *t) {
            best = Some((tick, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}
