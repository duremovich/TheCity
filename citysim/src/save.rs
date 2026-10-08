//! Whole-world serialisation with serde + RON. Everything on `World` is
//! saved except the flow-field cache, which is rebuilt lazily.

use std::path::{Path, PathBuf};

use crate::world::World;

/// The save format: bumped when a load must run a one-off migration on
/// older saves (`World::save_version`, 0 before the field existed).
/// 1: M15 (The Unplugged, corp competence); every save from M14 or
/// earlier reads 0.
pub const SAVE_VERSION: u8 = 1;

/// Compact RON of the whole world. Same world state ⇒ same bytes.
pub fn to_ron(world: &World) -> String {
    ron::to_string(world).expect("World is always serialisable")
}

pub fn from_ron(text: &str) -> Result<World, ron::error::SpannedError> {
    let mut world: World = ron::from_str(text)?;
    // M15 W44: a pre-M15 save (format 0) has no skill means, no Unplugged
    // and no corp competence. Review fix: the version, not a sentinel on
    // `skill_means`, so a world whose means are zero is not re-seeded on
    // every load.
    let pre_m15 = world.save_version < 1;
    world.rebuild_indices();
    world.migrate_legacy();
    // M14 V44: a pre-M14 save with the plane on gets its plane and ICE.
    crate::systems::virt::migrate(&mut world);
    // M15 W44: a pre-M15 save with the word on gets The Unplugged and its
    // corps' opening competence.
    if pre_m15 {
        crate::systems::creeds::migrate(&mut world);
        crate::systems::competence::seed(&mut world);
    }
    // M15 W41: a save from before the Feeds (news on) gets the opening two.
    crate::systems::news::migrate(&mut world);
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
