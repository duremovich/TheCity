//! One direction field per building door, built by Dijkstra from the door
//! with the same entry costs A* uses, so descending the field reproduces an
//! optimal A* path cost in O(1) per step.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::components::TilePos;
use crate::map::{Map, MAP_H, MAP_W};

/// Per tile: the next tile toward the door, or `None` when unreachable (or
/// at the door itself). Index is `y * MAP_W + x`.
#[derive(Clone, Debug, Default)]
pub struct FlowField {
    pub next: Vec<Option<TilePos>>,
    /// Cost from each tile to the door (sum of entered-tile costs).
    pub cost: Vec<f32>,
    pub door: Option<TilePos>,
}

impl FlowField {
    pub fn empty() -> Self {
        FlowField { next: vec![None; MAP_W * MAP_H], cost: vec![f32::INFINITY; MAP_W * MAP_H], door: None }
    }

    fn idx(p: TilePos) -> usize {
        usize::from(p.y) * MAP_W + usize::from(p.x)
    }

    /// Build the field for `door`.
    pub fn build(map: &Map, door: TilePos) -> FlowField {
        let mut f = FlowField::empty();
        f.door = Some(door);
        let mut open = BinaryHeap::new();
        f.cost[FlowField::idx(door)] = 0.0;
        open.push(Reverse((OrderedFloat(0.0f32), door)));
        while let Some(Reverse((OrderedFloat(c), cur))) = open.pop() {
            if c > f.cost[FlowField::idx(cur)] {
                continue;
            }
            // Travelling from n to cur enters cur, so n's cost = cost(cur) + dist(cur).
            let enter_cur = map.tile_at(cur).move_cost();
            for n in map.neighbours4(cur) {
                if !map.walkable(n) {
                    continue;
                }
                let nc = c + enter_cur;
                let i = FlowField::idx(n);
                if nc < f.cost[i] {
                    f.cost[i] = nc;
                    f.next[i] = Some(cur);
                    open.push(Reverse((OrderedFloat(nc), n)));
                }
            }
        }
        f
    }

    /// The next tile toward the door from `p`.
    pub fn step(&self, p: TilePos) -> Option<TilePos> {
        self.next[FlowField::idx(p)]
    }

    pub fn cost_from(&self, p: TilePos) -> f32 {
        self.cost[FlowField::idx(p)]
    }

    /// Full descent from `p` to the door, excluding `p`, including the door.
    pub fn descend(&self, p: TilePos) -> Vec<TilePos> {
        let mut out = Vec::new();
        let mut cur = p;
        while let Some(n) = self.step(cur) {
            out.push(n);
            cur = n;
            if out.len() > MAP_W * MAP_H {
                break; // cannot happen on a well-formed field; guards against loops
            }
        }
        out
    }
}
