//! One direction field per building door, built by Dijkstra from the door
//! with the same entry costs A* uses, so descending the field reproduces an
//! optimal A* path cost in O(1) per step.
//!
//! M10: a field stores one direction byte per tile (49 KB at 256 x 192) and
//! no cost array after the build; fields are built lazily and kept in an LRU
//! ([`FlowCache`]). A field is a pure function of the map and its door, so
//! eviction never changes results.

use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use crate::components::TilePos;
use crate::entity::EntityId;
use crate::map::Map;

/// Direction bytes: 0 = none (unreachable, or the door itself); 1..=4 =
/// W, E, N, S, the order of `Map::neighbours4`.
const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

fn dir_byte(from: TilePos, to: TilePos) -> u8 {
    let d = (i32::from(to.x) - i32::from(from.x), i32::from(to.y) - i32::from(from.y));
    DIRS.iter().position(|&x| x == d).map_or(0, |i| i as u8 + 1)
}

/// Per tile: the direction toward the door. Index is `y * w + x`.
#[derive(Clone, Debug, Default)]
pub struct FlowField {
    pub next: Vec<u8>,
    /// Map width (row stride of `next`).
    pub w: u16,
    pub door: Option<TilePos>,
}

impl FlowField {
    fn idx(&self, p: TilePos) -> usize {
        usize::from(p.y) * usize::from(self.w) + usize::from(p.x)
    }

    /// Build the field for `door`.
    pub fn build(map: &Map, door: TilePos) -> FlowField {
        let n = map.w() * map.h();
        let mut f = FlowField { next: vec![0; n], w: map.w() as u16, door: Some(door) };
        let mut cost = vec![f32::INFINITY; n];
        let mut open = BinaryHeap::new();
        cost[f.idx(door)] = 0.0;
        open.push(Reverse((OrderedFloat(0.0f32), door)));
        while let Some(Reverse((OrderedFloat(c), cur))) = open.pop() {
            if c > cost[f.idx(cur)] {
                continue;
            }
            // Travelling from n to cur enters cur, so n's cost = cost(cur) + dist(cur).
            let enter_cur = map.tile_at(cur).move_cost();
            for nb in map.neighbours4(cur) {
                if !map.walkable(nb) {
                    continue;
                }
                let nc = c + enter_cur;
                let i = f.idx(nb);
                if nc < cost[i] {
                    cost[i] = nc;
                    f.next[i] = dir_byte(nb, cur);
                    open.push(Reverse((OrderedFloat(nc), nb)));
                }
            }
        }
        f
    }

    /// The next tile toward the door from `p`.
    pub fn step(&self, p: TilePos) -> Option<TilePos> {
        let b = *self.next.get(self.idx(p))?;
        if b == 0 {
            return None;
        }
        let (dx, dy) = DIRS[usize::from(b - 1)];
        Some(TilePos { x: (i32::from(p.x) + dx) as u8, y: (i32::from(p.y) + dy) as u8 })
    }

    /// Full descent from `p` to the door, excluding `p`, including the door.
    pub fn descend(&self, p: TilePos) -> Vec<TilePos> {
        let mut out = Vec::new();
        let mut cur = p;
        while let Some(n) = self.step(cur) {
            out.push(n);
            cur = n;
            if out.len() > self.next.len() {
                break; // cannot happen on a well-formed field; guards against loops
            }
        }
        out
    }
}

/// Flow fields by building, with a use stamp for LRU eviction. Not saved.
#[derive(Clone, Debug, Default)]
pub struct FlowCache {
    fields: BTreeMap<EntityId, (FlowField, u64)>,
    clock: u64,
}

impl FlowCache {
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn contains(&self, b: EntityId) -> bool {
        self.fields.contains_key(&b)
    }

    pub fn clear(&mut self) {
        self.fields.clear();
    }

    /// The cached field for `b`, stamped as just used.
    pub fn get(&mut self, b: EntityId) -> Option<&FlowField> {
        self.clock += 1;
        let clock = self.clock;
        self.fields.get_mut(&b).map(|(f, stamp)| {
            *stamp = clock;
            &*f
        })
    }

    /// Insert a field, evicting the least recently used past `cap` (min 1).
    pub fn insert(&mut self, b: EntityId, field: FlowField, cap: usize) {
        while self.fields.len() >= cap.max(1) && !self.fields.contains_key(&b) {
            let oldest = self.fields.iter().min_by_key(|(&id, (_, stamp))| (*stamp, id)).map(|(&id, _)| id);
            match oldest {
                Some(id) => {
                    self.fields.remove(&id);
                }
                None => break,
            }
        }
        self.clock += 1;
        self.fields.insert(b, (field, self.clock));
    }
}
