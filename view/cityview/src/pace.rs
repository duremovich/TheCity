//! The frame cap (Dylan 2026-11-03): at most `[view] fps_cap` frames a real second (`--fps N` overrides; 0 and the
//! benches run uncapped). The window presents without vsync (`PresentMode::AutoNoVsync`) and winit runs continuously,
//! so without this the client drew as fast as the GPU let it, the CPU past what a frame needs taken from the sim
//! thread. The sim paces itself by the wall clock (simthread.rs), so the cap never slows the city.

use bevy::prelude::*;
use std::time::{Duration, Instant};

/// The cap's period and the next frame's deadline.
#[derive(Resource)]
pub struct FramePace {
    period: Duration,
    next: Instant,
}

impl FramePace {
    /// `None` when uncapped (`fps` 0).
    pub fn new(fps: u16) -> Option<FramePace> {
        (fps > 0).then(|| FramePace { period: Duration::from_secs_f64(1.0 / f64::from(fps)), next: Instant::now() })
    }
}

/// In `Last`: sleep out what is left of this frame's period. The deadline advances by the period, not from now, so a
/// frame that finishes early does not push the next one later (30 a second on average); a frame that ran a whole
/// period over resyncs to now rather than racing to catch up.
pub fn pace(mut p: ResMut<FramePace>) {
    let now = Instant::now();
    if p.next > now {
        std::thread::sleep(p.next - now);
    }
    let now = Instant::now();
    let period = p.period;
    p.next = if now > p.next + period { now + period } else { p.next + period };
}
