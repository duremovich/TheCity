use super::*;

// ---------------------------------------------------------------------------
// The network
// ---------------------------------------------------------------------------

/// The tiles of a polyline, waypoint to waypoint (an L when a pair is not
/// axis-aligned: x first), the first waypoint included.
pub(super) fn track(waypoints: &[[u16; 2]]) -> Vec<TilePos> {
    let mut out = Vec::new();
    let Some(&[x0, y0]) = waypoints.first() else { return out };
    let (mut x, mut y) = (i32::from(x0), i32::from(y0));
    out.push(TilePos::new(x0, y0));
    for &[bx, by] in &waypoints[1..] {
        let (bx, by) = (i32::from(bx), i32::from(by));
        while x != bx {
            x += (bx - x).signum();
            out.push(TilePos::new(x as u16, y as u16));
        }
        while y != by {
            y += (by - y).signum();
            out.push(TilePos::new(x as u16, y as u16));
        }
    }
    out
}

/// S1 (`OPEN-309`): is `t` on a marked crossing's zebra (a map `X` rect,
/// grown by `[transit] stop_zebra_margin_tiles`: the import's downsampled
/// rect and the painted zebra are a tile apart, `OPEN-316`)?
fn on_zebra(world: &World, t: TilePos) -> bool {
    let m = world.config.transit.stop_zebra_margin_tiles;
    let (x, y) = (u32::from(t.x), u32::from(t.y));
    world.map.crossings.iter().any(|c| {
        x + m >= u32::from(c.a.x) && x <= u32::from(c.b.x) + m && y + m >= u32::from(c.a.y) && y <= u32::from(c.b.y) + m
    })
}

/// The nearest walkable Road tile to `p` within `SNAP_RADIUS` (Manhattan),
/// ties the lower `(y, x)`; `p` itself when it is one. S1 (`OPEN-309`):
/// never a crossing's zebra (the Mid-Levels and Hillside lines' shared stop
/// at 823,619 stood its riders on one).
fn snap(world: &World, p: TilePos) -> Option<TilePos> {
    let (px, py) = (i32::from(p.x), i32::from(p.y));
    for r in 0..=SNAP_RADIUS {
        let mut best: Option<TilePos> = None;
        for dy in -r..=r {
            let rest = r - dy.abs();
            for dx in [-rest, rest] {
                let (x, y) = (px + dx, py + dy);
                if !world.map.in_bounds(x, y) {
                    continue;
                }
                let t = TilePos::new(x as u16, y as u16);
                if world.map.tile_at(t) != TileKind::Road || on_zebra(world, t) {
                    continue;
                }
                if best.is_none_or(|b| (t.y, t.x) < (b.y, b.x)) {
                    best = Some(t);
                }
                if rest == 0 {
                    break;
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}

/// Derive the stops and lines from `[transit]` and the map (T3, T5), then
/// the table over the open lines. A no-op with transit off.
pub fn build(world: &mut World) {
    world.transit.net = Net::default();
    if !enabled(world) {
        return;
    }
    let cfg = world.config.transit.clone();
    let tracks: Vec<Vec<TilePos>> = cfg.lines.iter().map(|l| track(&l.waypoints)).collect();
    let mut on_lines: BTreeMap<TilePos, BTreeSet<usize>> = BTreeMap::new();
    for (l, t) in tracks.iter().enumerate() {
        for &p in t {
            on_lines.entry(p).or_default().insert(l);
        }
    }
    let spacing = cfg.stop_spacing.max(1) as usize;
    let mut index: BTreeMap<TilePos, u16> = BTreeMap::new();
    let mut stops: Vec<TilePos> = Vec::new();
    let mut lines = Vec::new();
    for (l, (row, t)) in cfg.lines.iter().zip(&tracks).enumerate() {
        let waypoints: BTreeSet<TilePos> = row.waypoints.iter().map(|&[x, y]| TilePos::new(x, y)).collect();
        // The view milestone V9 (OPEN-255): a line with stations stops at
        // them (the map's other waypoints are bends; at a minute a bend the
        // island's lines ran at twice the walk), one without at every
        // waypoint and every `stop_spacing` tiles.
        let stations: BTreeSet<TilePos> = row
            .stations
            .iter()
            .filter_map(|&k| row.waypoints.get(usize::from(k)))
            .map(|&[x, y]| TilePos::new(x, y))
            .collect();
        let mut line = NetLine { name: row.name.clone(), ..NetLine::default() };
        let mut last_pos: Option<usize> = None;
        let mut at_track: Vec<usize> = Vec::new();
        // The ride and the dwells so far, unrounded: each offset is its
        // ceiling (a stretch's own ceiling cost every stop a whole minute).
        let mut exact = 0.0f64;
        for (i, &p) in t.iter().enumerate() {
            let shared = on_lines.get(&p).is_some_and(|s| s.len() > 1 || !s.contains(&l));
            let called = if stations.is_empty() {
                i % spacing == 0 || waypoints.contains(&p)
            } else {
                i == 0 || stations.contains(&p)
            };
            let is_stop = called || i + 1 == t.len() || shared;
            if !is_stop {
                continue;
            }
            let Some(at) = snap(world, p) else { continue };
            let s = *index.entry(at).or_insert_with(|| {
                stops.push(at);
                u16::try_from(stops.len() - 1).unwrap_or(u16::MAX)
            });
            if line.stops.last() == Some(&s) {
                continue;
            }
            let off = match (last_pos, line.offsets.last()) {
                (Some(j), Some(&prev)) => {
                    exact += (i - j) as f64 * f64::from(cfg.ride_ticks_per_tile) + f64::from(cfg.dwell_ticks);
                    (exact.ceil() as u32).max(prev + 1)
                }
                _ => 0,
            };
            line.stops.push(s);
            line.offsets.push(off);
            at_track.push(i);
            last_pos = Some(i);
        }
        line.total = line.offsets.last().copied().unwrap_or(0);
        line.seg = at_track
            .windows(2)
            .zip(line.stops.windows(2))
            .map(|(i, s)| seg_way(stops[usize::from(s[0])], &t[i[0]..=i[1]], stops[usize::from(s[1])]))
            .collect();
        lines.push(line);
    }
    let (w, h) = (world.map.w(), world.map.h());
    let (cells, cands) = near_index(w, h, &stops);
    let net = &mut world.transit.net;
    net.near_cells = cells;
    net.near_cands = cands;
    net.cells_w = w.div_ceil(NEAR_CELL);
    net.map_w = w;
    net.map_h = h;
    world.transit.net.platforms = platforms(world, &stops);
    world.transit.net.lines = lines;
    world.transit.net.stops = stops;
    let open = open_lines(world);
    rebuild_table(world, open);
}

/// S1 (`OPEN-309`): each stop's waiting place on its station's deck: the
/// platform deck point (a `P platform` end) nearest the stop tile within
/// `[transit] platform_reach_tiles` (ties the lower node), and the way up to
/// it: the stairs or lift (a `P stairs`/`P lift` from the way, level 0, to
/// a platform point) whose foot is nearest along the deck (its run plus the
/// platform's), then the platform's points to the waiting place. `None`: no
/// deck in reach (a tram stop), or no way down from it.
fn platforms(world: &World, stops: &[TilePos]) -> Vec<Option<Platform>> {
    use crate::map::PortalKind as K;
    type Node = (TilePos, u8);
    let reach = world.config.transit.platform_reach_tiles;
    let portals = &world.map.portals;
    if reach == 0 || portals.is_empty() {
        return vec![None; stops.len()];
    }
    let mut adj: BTreeMap<Node, Vec<(Node, u32)>> = BTreeMap::new();
    for p in portals.iter().filter(|p| p.kind == K::Platform && p.a.1 > 0 && p.b.1 > 0) {
        adj.entry(p.a).or_default().push((p.b, u32::from(p.tiles)));
        adj.entry(p.b).or_default().push((p.a, u32::from(p.tiles)));
    }
    // A platform point's ways down: (foot, the run).
    let mut downs: BTreeMap<Node, (TilePos, u32)> = BTreeMap::new();
    for p in portals.iter().filter(|p| matches!(p.kind, K::Stairs | K::Lift)) {
        for (top, foot) in [(p.a, p.b), (p.b, p.a)] {
            if top.1 > 0 && foot.1 == 0 && adj.contains_key(&top) {
                let run = u32::from(p.tiles);
                if downs.get(&top).is_none_or(|&(f, r)| (run, foot.0) < (r, f)) {
                    downs.insert(top, (foot.0, run));
                }
            }
        }
    }
    stops
        .iter()
        .map(|&s| {
            let (&wait, _) =
                adj.iter().filter(|(n, _)| n.0.manhattan(s) <= reach).min_by_key(|(n, _)| (n.0.manhattan(s), **n))?;
            // Dijkstra over the deck from the waiting place.
            let mut dist: BTreeMap<Node, (u32, Node)> = BTreeMap::new();
            let mut open = BinaryHeap::new();
            dist.insert(wait, (0, wait));
            open.push(std::cmp::Reverse((0u32, wait)));
            while let Some(std::cmp::Reverse((d, n))) = open.pop() {
                if dist.get(&n).is_some_and(|&(e, _)| e < d) {
                    continue;
                }
                for &(m, c) in adj.get(&n).into_iter().flatten() {
                    let nd = d + c;
                    if dist.get(&m).is_none_or(|&(e, _)| nd < e) {
                        dist.insert(m, (nd, n));
                        open.push(std::cmp::Reverse((nd, m)));
                    }
                }
            }
            let (top, foot) = dist
                .iter()
                .filter_map(|(n, &(d, _))| downs.get(n).map(|&(f, run)| (d + run, *n, f)))
                .min()
                .map(|(_, n, f)| (n, f))?;
            let mut way = vec![top];
            let mut at = top;
            while at != wait {
                at = dist.get(&at)?.1;
                way.push(at);
            }
            Some(Platform { foot, way })
        })
        .collect()
}

/// `Net::near`'s index: per `NEAR_CELL` cell (row-major), the offsets into
/// the candidates, and the candidates: every stop whose nearest point of the
/// cell is no farther than the `NEAR`-th smallest of the stops' farthest
/// points of it. A stop past that bound is beaten, on every tile of the
/// cell, by `NEAR` stops strictly nearer, so the cell's candidates hold each
/// of its tiles' `NEAR` nearest (ties included).
fn near_index(w: usize, h: usize, stops: &[TilePos]) -> (Vec<u32>, Vec<u16>) {
    let (cw, ch) = (w.div_ceil(NEAR_CELL), h.div_ceil(NEAR_CELL));
    let mut cells = Vec::with_capacity(cw * ch + 1);
    let mut cands = Vec::new();
    let k = NEAR.min(stops.len());
    let mut far: Vec<u32> = Vec::with_capacity(stops.len());
    cells.push(0);
    for cy in 0..ch {
        for cx in 0..cw {
            if k > 0 {
                let (x0, y0) = ((cx * NEAR_CELL) as i64, (cy * NEAR_CELL) as i64);
                let (x1, y1) = (((cx + 1) * NEAR_CELL).min(w) as i64 - 1, ((cy + 1) * NEAR_CELL).min(h) as i64 - 1);
                let span = |v: i64, lo: i64, hi: i64| -> (u32, u32) {
                    let near = if v < lo {
                        lo - v
                    } else if v > hi {
                        v - hi
                    } else {
                        0
                    };
                    (near as u32, (v - lo).abs().max((v - hi).abs()) as u32)
                };
                let reach = |s: &TilePos| {
                    let (nx, fx) = span(i64::from(s.x), x0, x1);
                    let (ny, fy) = span(i64::from(s.y), y0, y1);
                    (nx + ny, fx + fy)
                };
                far.clear();
                far.extend(stops.iter().map(|s| reach(s).1));
                far.select_nth_unstable(k - 1);
                let bound = far[k - 1];
                for (i, s) in stops.iter().enumerate() {
                    if reach(s).0 <= bound {
                        cands.push(u16::try_from(i).unwrap_or(u16::MAX));
                    }
                }
            }
            cells.push(cands.len() as u32);
        }
    }
    (cells, cands)
}

/// The lines open now (god `CutLine` is the only closure of a whole line).
pub(super) fn open_lines(world: &World) -> Vec<bool> {
    let tick = world.tick;
    (0..world.transit.net.lines.len())
        .map(|l| {
            u8::try_from(l).ok().is_none_or(|l| world.transit.line_closed.get(&l).is_none_or(|&until| until <= tick))
        })
        .collect()
}

/// A path's first leg in `rebuild_table`: line, direction, board position.
type FirstLeg = (u8, bool, u16);

/// The way between two calls: the stop tile, the track's corners from the
/// first call's track tile to the second's, the next stop tile (repeats
/// dropped).
fn seg_way(from: TilePos, track: &[TilePos], to: TilePos) -> Vec<TilePos> {
    let mut out = vec![from];
    let dir = |a: TilePos, b: TilePos| {
        ((i32::from(b.x) - i32::from(a.x)).signum(), (i32::from(b.y) - i32::from(a.y)).signum())
    };
    for (i, &p) in track.iter().enumerate() {
        let corner = i == 0 || i + 1 == track.len() || dir(track[i - 1], p) != dir(p, track[i + 1]);
        if corner && out.last() != Some(&p) {
            out.push(p);
        }
    }
    if out.last() != Some(&to) {
        out.push(to);
    }
    out
}

/// The all-pairs table over the open lines: Dijkstra from every stop over
/// `(line, position)` nodes, ride edges along a line, `transfer_ticks`
/// between two lines' nodes at one stop. Ties keep the first path found
/// (the heap orders by `(ticks, node)`: deterministic).
fn rebuild_table(world: &mut World, open: Vec<bool>) {
    // The island pass phase 6: with trains, a transfer also waits for the
    // next train (half a headway on the mean), so the table's ticks (the
    // structural `commute_ticks`) are the ride's expected value.
    let transfer =
        world.config.transit.transfer_ticks + if trains_on(world) { world.config.transit.headway / 2 } else { 0 };
    let net = &world.transit.net;
    let n = net.stops.len();
    // Nodes: (line, position) of the open lines.
    let mut nodes: Vec<(usize, usize)> = Vec::new();
    let mut at_stop: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut node_of: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (l, line) in net.lines.iter().enumerate() {
        if !open.get(l).copied().unwrap_or(false) {
            continue;
        }
        for (k, &s) in line.stops.iter().enumerate() {
            node_of.insert((l, k), nodes.len());
            at_stop[usize::from(s)].push(nodes.len());
            nodes.push((l, k));
        }
    }
    let mut table = vec![NO_HOP; n * n];
    for src in 0..n {
        // The ticks, the first leg, the first leg's last call (set at the
        // first transfer).
        let mut dist: Vec<(u32, Option<FirstLeg>, Option<u16>)> = vec![(UNREACHABLE, None, None); nodes.len()];
        let mut heap = BinaryHeap::new();
        for &v in &at_stop[src] {
            dist[v] = (0, None, None);
            heap.push(std::cmp::Reverse((0u32, v)));
        }
        while let Some(std::cmp::Reverse((d, v))) = heap.pop() {
            if d > dist[v].0 {
                continue;
            }
            let (first, off) = (dist[v].1, dist[v].2);
            let (l, k) = nodes[v];
            let line = &net.lines[l];
            let mut relax =
                |to: usize, cost: u32, first: Option<FirstLeg>, off: Option<u16>, heap: &mut BinaryHeap<_>| {
                    let nd = d.saturating_add(cost);
                    if nd < dist[to].0 {
                        dist[to] = (nd, first, off);
                        heap.push(std::cmp::Reverse((nd, to)));
                    }
                };
            let lu = u8::try_from(l).unwrap_or(u8::MAX);
            let ku = u16::try_from(k).unwrap_or(u16::MAX);
            if k + 1 < line.stops.len() {
                let to = node_of[&(l, k + 1)];
                relax(to, line.offsets[k + 1] - line.offsets[k], first.or(Some((lu, true, ku))), off, &mut heap);
            }
            if k > 0 {
                let to = node_of[&(l, k - 1)];
                relax(to, line.offsets[k] - line.offsets[k - 1], first.or(Some((lu, false, ku))), off, &mut heap);
            }
            // A transfer only once riding (at the source it changes nothing).
            if first.is_some() {
                let s = usize::from(line.stops[k]);
                for &w in &at_stop[s] {
                    if w != v {
                        relax(w, transfer, first, off.or(Some(ku)), &mut heap);
                    }
                }
            }
        }
        for t in 0..n {
            if t == src {
                continue;
            }
            let best = at_stop[t]
                .iter()
                .filter_map(|&v| dist[v].1.map(|f| (dist[v].0, f, dist[v].2.unwrap_or(nodes[v].1 as u16))))
                .min_by_key(|&(d, _, _)| d);
            if let Some((ticks, (line, forward, pos), off)) = best {
                table[src * n + t] = Hop { ticks, line, forward, pos, off };
            }
        }
    }
    world.transit.net.table = table;
    world.transit.net.open = open;
}

/// Build the network if it is missing, and rebuild the table when a line
/// opened or closed since it was built.
pub fn ensure_net(world: &mut World) {
    if !enabled(world) {
        return;
    }
    if world.transit.net.stops.is_empty() && !world.config.transit.lines.is_empty() {
        build(world);
        return;
    }
    let open = open_lines(world);
    if open != world.transit.net.open {
        rebuild_table(world, open);
    }
}
