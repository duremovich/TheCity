//! One direction field per building door, built by Dijkstra from the door.
//! Built in M1; the type exists now so `World` can hold the cache.

use crate::map::{MAP_H, MAP_W};

/// Per tile: the step toward the door as `(dx, dy)` in `{-1, 0, 1}`, or
/// `None` for unreachable tiles. Index is `y * MAP_W + x`.
#[derive(Clone, Debug, Default)]
pub struct FlowField {
    pub step: Vec<Option<(i8, i8)>>,
}

impl FlowField {
    pub fn empty() -> Self {
        FlowField { step: vec![None; MAP_W * MAP_H] }
    }
}
