//! Grid A* on 4-connected walkable tiles with per-tile entry costs and a
//! Manhattan heuristic × 0.7. Deterministic: ties break on `(f, g, tile)`.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use crate::components::TilePos;
use crate::map::Map;

pub const HEURISTIC_WEIGHT: f32 = 0.7;

/// A path from `start` (exclusive) to `goal` (inclusive), or `None` if
/// unreachable or the expansion cap is hit. The returned path's cost is the
/// sum of `move_cost` over every tile entered.
pub fn astar(map: &Map, start: TilePos, goal: TilePos, max_expansions: usize) -> Option<Vec<TilePos>> {
    if start == goal {
        return Some(Vec::new());
    }
    if !map.walkable(goal) {
        return None;
    }
    let h = |p: TilePos| p.manhattan(goal) as f32 * HEURISTIC_WEIGHT;

    let mut open = BinaryHeap::new();
    let mut best_g: BTreeMap<TilePos, f32> = BTreeMap::new();
    let mut parent: BTreeMap<TilePos, TilePos> = BTreeMap::new();
    best_g.insert(start, 0.0);
    open.push(Reverse((OrderedFloat(h(start)), OrderedFloat(0.0f32), start)));

    let mut expansions = 0usize;
    while let Some(Reverse((_, OrderedFloat(g), cur))) = open.pop() {
        if best_g.get(&cur).is_some_and(|&bg| g > bg) {
            continue; // stale entry
        }
        if cur == goal {
            let mut path = vec![goal];
            let mut p = goal;
            while let Some(&q) = parent.get(&p) {
                if q == start {
                    break;
                }
                path.push(q);
                p = q;
            }
            path.reverse();
            return Some(path);
        }
        expansions += 1;
        if expansions > max_expansions {
            return None;
        }
        for n in map.neighbours4(cur) {
            let kind = map.tile_at(n);
            if !kind.walkable() {
                continue;
            }
            let ng = g + kind.move_cost();
            if best_g.get(&n).is_none_or(|&bg| ng < bg) {
                best_g.insert(n, ng);
                parent.insert(n, cur);
                open.push(Reverse((OrderedFloat(ng + h(n)), OrderedFloat(ng), n)));
            }
        }
    }
    None
}

/// Sum of entry costs along a path.
pub fn path_cost(map: &Map, path: &[TilePos]) -> f32 {
    path.iter().map(|&p| map.tile_at(p).move_cost()).sum()
}
