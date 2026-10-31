//! Gutterlight client rules (session 17), for any Bevy client over the game data (engine/lighttest is the bench):
//!
//! - `block`: the block file (`assets/blocks/<id>/block.toml`) and its navmesh (`nav.json`), as written by
//!   `pipeline/blender/building/block.py`;
//! - `city`: placed blocks -> the lots' building glbs, the street (sidewalks, road, kerbs, crossings, rail),
//!   lamps, props, grade lights; the glTF `gl_*` extras -> `GlNode` / `GlLight`; the cutaway (GAME_DATA.md);
//! - `lighting`: the session-16 light rules (DECISIONS "Session 16"): a per-frame shadow allocator over a budget
//!   of shadow views, downward fixtures as spots, room fills only on the cut floor, volumetric shafts on the
//!   shadowed set; the other policies stay for the bench;
//! - `occlusion`: camera occlusion (CITY_SPEC §CAM): see-through volumes, L3 fades, the L1 cut driver,
//!   silhouettes (`OcclusionPlugin`, a host adds it and feeds its own ray tests);
//! - `kit`: P6 kit buildings stamped from a kit stream + the block atlas (no glb), with the glb path's components
//!   (`GlNode`, `GlLight`), so the cutaway, light rules and occlusion apply unchanged;
//! - `vehicles`: P8 the city fleet (generated vehicle glbs flattened into shared parts for traffic / tram);
//! - `street`: P7 street dressing per city chunk (lamps + their lazy lights, zebra paint) from out/street/;
//! - `frame`: block / building frame (x east, y north, z up) <-> Bevy (x, z, -y), the SPEC 5.1 camera.
//!
//! Insert a `city::City` and a `frame::Focus` before Startup, add `GlClientPlugin`.

// Bevy systems take their params as arguments and query tuples as types: the two lints every Bevy crate allows.
// The traffic sim and the kit decoder walk parallel arrays by index (needless_range_loop would split them).
#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::needless_range_loop)]

pub mod block;
pub mod city;
pub mod frame;
pub mod kit;
pub mod lighting;
pub mod occlusion;
pub mod street;
pub mod traffic;
pub mod tram;
pub mod vehicles;

use bevy::prelude::*;

pub struct GlClientPlugin;

impl Plugin for GlClientPlugin {
    fn build(&self, app: &mut App) {
        kit::plugin(app);
        street::plugin(app);
        vehicles::plugin(app);
        tram::plugin(app);
        traffic::plugin(app);
        app.init_resource::<lighting::LightSettings>()
            .init_resource::<lighting::AllocReset>()
            .init_resource::<lighting::LightStats>()
            .init_resource::<city::ReadyBuildings>()
            .add_systems(Startup, city::setup_city)
            .add_systems(
                Update,
                (
                    city::apply_cutaway,
                    lighting::make_twins,
                    lighting::apply_lights.after(lighting::make_twins),
                    lighting::apply_features,
                    frame::sync_camera,
                ),
            );
    }
}
