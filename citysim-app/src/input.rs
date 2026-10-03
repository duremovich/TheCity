//! Keyboard and mouse: pan, zoom, select, time controls, save/load.

use macroquad::prelude::*;

use citysim::{save, Brain, Lod, PlayerCommand, Position, Speed, World, TICKS_PER_HOUR};

use crate::camera::{PAN_TILES_PER_SEC, ZOOM_PER_NOTCH};
use crate::ui::HUD_H;
use crate::App;

pub fn handle(app: &mut App, world: &mut World) {
    let dt = get_frame_time();
    let mouse = Vec2::from(mouse_position());

    // --- pan: WASD or left-drag below the HUD ------------------------------
    let mut pan = Vec2::ZERO;
    if is_key_down(KeyCode::W) {
        pan.y -= 1.0;
    }
    if is_key_down(KeyCode::S) {
        pan.y += 1.0;
    }
    if is_key_down(KeyCode::A) {
        pan.x -= 1.0;
    }
    if is_key_down(KeyCode::D) {
        pan.x += 1.0;
    }
    if pan != Vec2::ZERO {
        app.camera.pan(pan.normalize() * PAN_TILES_PER_SEC * dt);
        app.follow = false;
    }
    if is_mouse_button_down(MouseButton::Left) && mouse.y > HUD_H {
        let delta = mouse_delta_position();
        if delta != Vec2::ZERO {
            // mouse_delta_position is in normalised [-1, 1] screen units
            let px = vec2(delta.x * screen_width() * 0.5, delta.y * screen_height() * 0.5);
            app.camera.pan(px / app.camera.px_per_tile);
            app.follow = false;
        }
    }

    // --- zoom about the cursor ----------------------------------------------
    let (_, wheel) = mouse_wheel();
    if wheel != 0.0 && mouse.y > HUD_H {
        let factor = if wheel > 0.0 { ZOOM_PER_NOTCH } else { 1.0 / ZOOM_PER_NOTCH };
        app.camera.zoom_about(mouse, factor);
    }

    // --- select: click on a Full agent's tile --------------------------------
    if is_mouse_button_pressed(MouseButton::Left) && mouse.y > HUD_H {
        if let Some(tile) = app.camera.tile_at(mouse) {
            app.selected = world.citizens().into_iter().find(|&id| {
                world.comp::<Position>(id).is_some_and(|p| p.tile == tile)
                    && world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Full)
            });
        }
    }
    if is_key_pressed(KeyCode::Escape) {
        app.selected = None;
        app.follow = false;
    }
    if is_key_pressed(KeyCode::F) {
        app.follow = app.selected.is_some() && !app.follow;
    }
    if app.follow {
        if let Some(p) = app.selected.and_then(|id| world.comp::<Position>(id)) {
            app.camera.centre_on(p.tile);
        }
    }

    // --- time controls -------------------------------------------------------
    if is_key_pressed(KeyCode::Space) {
        app.paused = !app.paused;
    }
    let speed_keys =
        [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7];
    for (key, speed) in speed_keys.into_iter().zip(Speed::ALL) {
        if is_key_pressed(key) && app.speed != speed {
            app.speed = speed;
            app.cmds.push(PlayerCommand::SetSpeed(speed));
        }
    }
    if is_key_pressed(KeyCode::Period) {
        step(app, world, 1);
    }
    if is_key_pressed(KeyCode::Comma) {
        step(app, world, TICKS_PER_HOUR);
    }

    // --- save / load ---------------------------------------------------------
    if is_key_pressed(KeyCode::F5) {
        let path = save::save_path(&app.saves_dir, world.seed(), world.tick);
        match save::save_to_file(world, &path) {
            Ok(()) => app.notify(format!("Saved {}", path.display())),
            Err(e) => app.notify(format!("Save failed: {e}")),
        }
    }
    if is_key_pressed(KeyCode::F9) {
        match save::newest_save(&app.saves_dir, world.seed()) {
            Some(path) => match save::load_from_file(&path) {
                Ok(w) => {
                    *world = w;
                    app.selected = None;
                    app.acc = 0.0;
                    app.notify(format!("Loaded {}", path.display()));
                }
                Err(e) => app.notify(format!("Load failed: {e}")),
            },
            None => app.notify(format!("No save for seed {} in {}", world.seed(), app.saves_dir.display())),
        }
    }
}

/// Stepping while paused calls `tick()` directly.
fn step(app: &mut App, world: &mut World, ticks: u64) {
    app.paused = true;
    world.push_commands(&mut app.cmds);
    for _ in 0..ticks {
        citysim::tick(world);
    }
}
