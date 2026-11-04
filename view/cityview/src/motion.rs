//! Motion V1 (the films' fix pass, docs/TESTING.md "Filming motion"; CLAUDE.md: at every speed the player sees time
//! pass, and no body ever teleports): what a body's drawn place, facing and clip do between the sim's places.
//! sim.rs lerps each body's tile along its way (the base, a tile centre line) and picks the place's offset (a lane's
//! jitter while it walks, a crowd's slot while it stands, `Sim::slots`); this layer, per drawn body, keyed by id:
//!
//! - **The offset eases**: a body's offset moves toward the one sim.rs asks for at `[view] motion_offset_m_s` (x the
//!   square root of the play speed: a step aside, readable at 60x), so a walker stopping steps into its crowd slot and a crowd re-slotting shuffles over; the view never
//!   jumps a body because it changed its mind about the offset (P1: 5-8 m jumps at the first real tick pair).
//! - **The guard**: a drawn place that would step past `motion_jump_k` x the faster of the walk and its own last
//!   speed a frame (+ `motion_jump_floor_m`) stays where it was drawn and closes the gap at `motion_catch_k` x the walk (a re-plan, the
//!   click way handing back to the lerp, a pair begun off the last one's end); past `motion_snap_m` (or sim.rs's
//!   "no way" jump, the sim's own placement; every body while the sim runs flat out to a hold, a script's line or
//!   `--from`) it is drawn as the jump it is.
//! - **The facing turns** at `turn_deg_s` x the square root of the play speed toward the way it walks (its base's
//!   way; a body sidestepping into its slot faces on; standing, the sim's heading), never a 180 deg snap in a frame.
//! - **The clip follows the ground**: walking (the drawn speed past `walk_on_m_s`, back to the idle under
//!   `walk_off_m_s`) the walk clip plays at the drawn speed over `walk_clip_m_s`, clamped to `walk_clip_rate_min` ..
//!   `walk_clip_rate_max` (a 60x walk would cycle the legs 60 times a second); a body stood still idles.
//!
//! A body not drawn in a frame (indoors out of the cut, aboard, out of the ring) loses its track: it comes back fresh.
//! `CV_MOTION_DEBUG=INDEX,INDEX` logs those bodies' tracks every frame (base, offset, raw, speed, gap, snap).

use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
struct Track {
    /// The offset drawn now (metres off the base), eased toward the one asked
    off: Vec2,
    /// Last frame's base + offset, and the place drawn (the two differ by the gap the guard closes)
    raw: Vec2,
    at: Vec2,
    err: Vec2,
    /// The raw place's own speed (m/s, last frame's step)
    vel: Vec2,
    /// The base alone (metres, and tile units: the click way's rejoin)
    base: Vec2,
    base_t: [f32; 2],
    yaw: f32,
    /// The drawn ground speed (m/s, eased) and the clip it reads as
    speed: f32,
    walking: bool,
    seen: u64,
}

/// One body's draw this frame.
#[derive(Clone, Copy, Debug)]
pub struct Drawn {
    pub at: Vec2,
    pub yaw: f32,
    pub walking: bool,
    pub clip_rate: f32,
}

/// The per-body tracks (sim.rs `Sim::motion`).
#[derive(Default)]
pub struct Motion {
    tracks: HashMap<u64, Track>,
    frame: u64,
    dt: f32,
    speed_x: f32,
    /// This frame's jumps absorbed and drawn as jumps (the HUD's)
    pub absorbed: usize,
    pub snapped: usize,
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI
}

impl Motion {
    /// A frame begins: `dt` real seconds since the last (0 on a film's frames between captures: nothing moves), the
    /// play speed now (a multiple of 1x; the pace's scale for the eases, at least 1).
    pub fn begin(&mut self, dt: f32, speed_x: f32) {
        self.frame += 1;
        self.dt = dt.max(0.0);
        self.speed_x = speed_x.max(1.0);
        self.absorbed = 0;
        self.snapped = 0;
    }

    /// The frame is over: the bodies not drawn in it lose their tracks.
    pub fn end(&mut self) {
        let f = self.frame;
        self.tracks.retain(|_, t| t.seen == f);
    }

    /// Body `id` drawn this frame: `base` its lerped place on the tile centre line (metres; `base_t` the same in
    /// tiles), `want` the offset sim.rs asks for, `heading` the sim's (a standing body faces it), `jump` the sim's own
    /// placement (drawn as one).
    pub fn draw(
        &mut self,
        id: u64,
        base: Vec2,
        base_t: [f32; 2],
        want: Vec2,
        heading: f32,
        jump: bool,
        c: &citysim::config::ViewCfg,
    ) -> Drawn {
        let (dt, sx, frame) = (self.dt, self.speed_x, self.frame);
        let walk = c.walk_m_s as f32 * sx;
        let Some(t) = self.tracks.get_mut(&id) else {
            let at = base + want;
            self.tracks.insert(
                id,
                Track {
                    off: want,
                    raw: at,
                    at,
                    err: Vec2::ZERO,
                    vel: Vec2::ZERO,
                    base,
                    base_t,
                    yaw: heading,
                    speed: 0.0,
                    walking: false,
                    seen: frame,
                },
            );
            return Drawn { at, yaw: heading, walking: false, clip_rate: 1.0 };
        };
        t.seen = frame;
        let along = base - t.base;
        t.base = base;
        t.base_t = base_t;
        // the offset eases toward the one asked
        let d = want - t.off;
        let step = c.motion_offset_m_s * sx.sqrt() * dt;
        t.off = if d.length() <= step { want } else { t.off + d.normalize() * step };
        let raw = base + t.off;
        // the guard: a step of the raw place past `motion_jump_k` x the faster of the walk and its own last speed (a
        // walker the sim moves fast is no jump; a stop is none either) is kept off the drawn one, the drawn body where
        // it was, the gap closing at a walk
        let leap = raw.distance(t.raw);
        let mut snap = jump || leap > c.motion_snap_m;
        let absorb = !snap && leap > c.motion_jump_k * walk.max(t.vel.length()) * dt + c.motion_jump_floor_m;
        if absorb {
            t.err = t.at - raw;
            self.absorbed += 1;
        }
        if dt > 0.0 {
            t.vel = (raw - t.raw) / dt;
        }
        // (a gap grown past the snap over many frames, a flat-out run's, is a jump too)
        snap |= t.err.length() > c.motion_snap_m;
        if snap {
            t.err = Vec2::ZERO;
            t.vel = Vec2::ZERO;
            self.snapped += usize::from(leap > 0.5);
        }
        let close = c.motion_catch_k * walk * dt;
        let e = t.err.length();
        t.err = if e <= close { Vec2::ZERO } else { t.err * (1.0 - close / e) };
        let at = raw + t.err;
        // the ground speed drawn, eased (a film's 0-dt frames keep the last; a jump is no speed)
        let moved = if snap { Vec2::ZERO } else { at - t.at };
        // (a body stood still idles at once: a 60x walk's speed eased down took over a second, the walk in place)
        if snap {
            t.speed = 0.0;
        } else if dt > 0.0 {
            let v = moved.length() / dt;
            let k = 1.0 - (-dt * std::f32::consts::LN_2 / c.motion_speed_half_life_s.max(1e-3)).exp();
            t.speed = if v < c.walk_off_m_s { v } else { t.speed + (v - t.speed) * k };
        }
        t.walking = if t.walking { t.speed > c.walk_off_m_s } else { t.speed > c.walk_on_m_s };
        // the facing turns toward the way walked (the base's: a body stepping into its crowd slot or closing a gap
        // sidesteps facing on); standing, the sim's heading; walking with no step this frame, as it was
        let goal = match (t.walking, along.length_squared() > 1e-8) {
            (true, true) => -along.x.atan2(along.y),
            (false, true) => t.yaw,
            (true, false) if moved.length_squared() > 1e-8 => t.yaw,
            _ => heading,
        };
        let turn = wrap(goal - t.yaw);
        let max = c.turn_deg_s.to_radians() * sx.sqrt() * dt;
        t.yaw = wrap(t.yaw + turn.clamp(-max, max));
        if std::env::var("CV_MOTION_DEBUG").is_ok_and(|v| v.split(',').any(|x| x.parse() == Ok(id & 0xFFFF_FFFF))) {
            eprintln!(
                "motion {}: frame {frame} dt {dt:.3} sx {sx:.1} base {base:?} want {want:?} off {:?} raw {raw:?} leap {leap:.2} vel {:?} err {:?} at {at:?} snap {snap} jump {jump}",
                id & 0xFFFF_FFFF,
                t.off,
                t.vel,
                t.err
            );
        }
        t.raw = raw;
        t.at = at;
        let clip_rate =
            if t.walking { (t.speed / c.walk_clip_m_s.max(0.1)).clamp(c.walk_clip_rate_min, c.walk_clip_rate_max) } else { 1.0 };
        Drawn { at, yaw: t.yaw, walking: t.walking, clip_rate }
    }

    /// The base (tiles) body `id` was drawn on last frame.
    pub fn base_of(&self, id: u64) -> Option<[f32; 2]> {
        self.tracks.get(&id).map(|t| t.base_t)
    }
}

/// Motion V1 (docs/TIME_MODEL.md § 5): a click's new way (`tiles`, from the sim's place at the click) as the drawn
/// body walks it from where it is drawn (`from`, tiles): from there straight to the way's next point after the
/// nearest one within `near` tiles, else to the way's start. The sim's place at a click is up to a tick ahead of the
/// drawn body (the lerp runs a tick behind), so the way walked from its start snapped the body forward, and a
/// re-click mid-walk (the drawn body then ahead) snapped it back.
pub fn rejoin(tiles: &[[f32; 2]], from: [f32; 2], near: f32) -> Vec<[f32; 2]> {
    let p = Vec2::from(from);
    let mut best: Option<(f32, usize)> = None;
    for (i, w) in tiles.windows(2).enumerate() {
        let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
        let ab = b - a;
        let l2 = ab.length_squared();
        let k = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let d = (a + ab * k).distance(p);
        if d <= near && best.is_none_or(|x| d < x.0) {
            best = Some((d, i + 1));
        }
    }
    let rest = match best {
        Some((_, next)) => &tiles[next..],
        None => tiles,
    };
    std::iter::once(from).chain(rest.iter().copied()).collect()
}
