//! Whole-world serialisation with serde + RON. Everything on `World` is
//! saved except the flow-field cache, which is rebuilt lazily.

use std::path::{Path, PathBuf};

use crate::world::World;

/// The save format (`World::save_version`): bump it when the saved shape changes.
/// The history is `git log -L '/pub const SAVE_VERSION/,+1:citysim/src/save.rs'`.
pub const SAVE_VERSION: u16 = 135;

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
    // The city milestone C7: the portal graph (not saved).
    crate::systems::levels::init_portals(&mut world);
    // M12 D1: the district grid and adjacency are not saved.
    crate::systems::districts::rebuild(&mut world);
    // Transit: the derived network (not saved).
    crate::systems::transit::build(&mut world);
    // M17 phase 2: a save from before the parents builds their accounts
    // (minted, so the identity carries over; nothing with `[outside]` off).
    if world.save_version < 22 {
        crate::systems::outside::seed_parents(&mut world);
    }
    // M17 phase 4: a parent from before the weapons gets its edge, Data and tech ceiling.
    if world.save_version < 24 {
        crate::systems::parents::migrate_v24(&mut world);
    }
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
