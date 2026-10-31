//! Meshing (runs on the async compute pool): a chunk's ground tiles, its massing boxes, the elevated layers.
//! Positions are Bevy (x, z, -y) relative to an origin (the chunk's SW corner for chunk meshes).

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh, PrimitiveTopology},
    prelude::*,
};

use crate::data::*;

/// World (x east, y north, z up) -> Bevy (x, z, -y).
#[inline]
pub fn bv(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, -y)
}

#[derive(Default)]
pub struct Mb {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub uv1: Vec<[f32; 2]>,
    pub uv0: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
    with_uv1: bool,
    /// UV_0 of the next quads (massing: window style, crown flag)
    pub cur_uv0: [f32; 2],
}

impl Mb {
    pub fn with_uv1() -> Mb {
        Mb { with_uv1: true, ..default() }
    }

    /// A quad p0 p1 p2 p3 (around its edge, either winding); faces `n`.
    pub fn quad(&mut self, p: [Vec3; 4], n: Vec3, c: [f32; 4], d: [f32; 2]) {
        self.quad_n(p, [n; 4], n, c, d);
    }

    /// A quad with per-vertex normals `vn`; `n` = the face direction (winding).
    pub fn quad_n(&mut self, p: [Vec3; 4], vn: [Vec3; 4], n: Vec3, c: [f32; 4], d: [f32; 2]) {
        let base = self.pos.len() as u32;
        for (q, m) in p.iter().zip(vn) {
            self.pos.push(q.to_array());
            self.nrm.push(m.to_array());
            self.col.push(c);
            if self.with_uv1 {
                self.uv1.push(d);
                self.uv0.push(self.cur_uv0);
            }
        }
        let f = (p[1] - p[0]).cross(p[2] - p[0]) + (p[2] - p[0]).cross(p[3] - p[0]);
        if f.dot(n) >= 0.0 {
            self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    /// An axis-aligned box in Bevy coords (min, max), without its bottom face.
    pub fn bx(&mut self, a: Vec3, b: Vec3, c: [f32; 4], d: [f32; 2]) {
        let (x0, y0, z0, x1, y1, z1) = (a.x, a.y, a.z, b.x, b.y, b.z);
        let v = |x, y, z| Vec3::new(x, y, z);
        self.quad([v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)], Vec3::Y, c, d);
        self.quad([v(x0, y0, z0), v(x0, y1, z0), v(x0, y1, z1), v(x0, y0, z1)], Vec3::NEG_X, c, d);
        self.quad([v(x1, y0, z0), v(x1, y1, z0), v(x1, y1, z1), v(x1, y0, z1)], Vec3::X, c, d);
        self.quad([v(x0, y0, z0), v(x1, y0, z0), v(x1, y1, z0), v(x0, y1, z0)], Vec3::NEG_Z, c, d);
        self.quad([v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)], Vec3::Z, c, d);
    }

    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    pub fn tris(&self) -> usize {
        self.idx.len() / 3
    }

    pub fn into_mesh(self) -> Mesh {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        if self.with_uv1 {
            // (bevy_pbr's fragment reads UV_0 whenever any UV set is present: it must exist anyway)
            m.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv0);
            m.insert_attribute(Mesh::ATTRIBUTE_UV_1, self.uv1);
        }
        m.insert_indices(Indices::U32(self.idx));
        m
    }
}

fn lin(hex: u32) -> [f32; 4] {
    let c = LinearRgba::from(Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8));
    [c.red, c.green, c.blue, 1.0]
}

fn scale(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, 1.0]
}

/// Ground colours (SPEC §PAL 3.1 albedos; nothing over #6A6A72).
pub fn class_colour(k: u8) -> [f32; 4] {
    match k {
        // one colour for natural and open land: the urban mask is ragged at tile scale, two tones read as a
        // checkerboard (review p3 r2)
        NATURAL | OPEN => lin(0x22281F),
        PLATFORM => lin(0x2A2A31),
        CARRIAGE => lin(0x1A1A20),
        MEDIAN => lin(0x3A3A42),
        SIDEWALK => lin(0x34343C),
        PED => lin(0x3A3A42),
        STAIRS => lin(0x4A4A52),
        _ => lin(0x0E1218),
    }
}

fn wall_colour(k: u8) -> [f32; 4] {
    match k {
        NATURAL | OPEN => lin(0x2B2620),
        _ => scale(lin(0x3A3A42), 0.85),
    }
}

/// The ground of a chunk. `step` = tiles per cell (1 full detail; 4 for the overview: a cell takes its corner
/// tiles' outer corners and its centre tile's class). Tops (flat runs merged) + a vertical quad on every cell edge
/// whose two sides differ (each chunk owns its W and S edges; the E / N ones belong to the neighbour).
pub fn ground_mesh(g: &Ground, tile: f32, step: i32) -> Mb {
    let mut mb = Mb::default();
    let n = g.n / step;
    let cs = tile * step as f32;
    // cell (ci, cj): class + corners, for ci, cj in -1 .. n (one apron cell: only its edge toward us is read)
    let cell = |ci: i32, cj: i32| -> (u8, [f32; 4]) {
        if step == 1 {
            let k = g.at(ci, cj);
            return (g.class[k], g.corners[k]);
        }
        let i0 = (ci * step).clamp(-1, g.n);
        let j0 = (cj * step).clamp(-1, g.n);
        let i1 = (ci * step + step - 1).clamp(-1, g.n);
        let j1 = (cj * step + step - 1).clamp(-1, g.n);
        let mid = g.at(((ci * step) + step / 2).clamp(-1, g.n), ((cj * step) + step / 2).clamp(-1, g.n));
        let sw = g.corners[g.at(i0, j0)][0];
        let se = g.corners[g.at(i1, j0)][1];
        let nw = g.corners[g.at(i0, j1)][2];
        let ne = g.corners[g.at(i1, j1)][3];
        (g.class[mid], [sw, se, nw, ne])
    };
    let flat = |c: &[f32; 4]| c[1] == c[0] && c[2] == c[0] && c[3] == c[0];
    // the height at grid corner (gi, gj), gi / gj in -1 ..= n (from the cell whose SW, SE, NW or NE it is), and
    // whether that cell is natural / open land
    let hg = |gi: i32, gj: i32| -> (bool, f32) {
        let (ci, ex) = if gi < n { (gi.max(-1), 0) } else { (n - 1, 1) };
        let (cj, ey) = if gj < n { (gj.max(-1), 0) } else { (n - 1, 2) };
        let (k, c) = cell(ci, cj);
        (k == NATURAL || k == OPEN, c[ex + ey])
    };
    // a smooth vertex normal over +-3 corners (the terrain's fine fBm under a low light read as tile noise: review
    // p3 r3), or None where a sample falls on a platform / road (their heights tilted the normal: lit wedges at
    // platform corners, review p3 r4): the face normal then
    let corner_n = |gi: i32, gj: i32| -> Option<Vec3> {
        let (a, b) = ((gi - 3).max(-1), (gi + 3).min(n));
        let (c, d) = ((gj - 3).max(-1), (gj + 3).min(n));
        let s = [hg(b, gj), hg(a, gj), hg(gi, d), hg(gi, c)];
        if s.iter().any(|x| !x.0) {
            return None;
        }
        let dzdx = (s[0].1 - s[1].1) / ((b - a) as f32 * cs);
        let dzdy = (s[2].1 - s[3].1) / ((d - c) as f32 * cs);
        // world (-dz/dx, -dz/dy, 1) -> Bevy (x, z, -y)
        Some(Vec3::new(-dzdx, 1.0, dzdy).normalize())
    };
    for cj in 0..n {
        let y0 = cj as f32 * cs;
        let y1 = y0 + cs;
        let mut ci = 0;
        while ci < n {
            let (k, c) = cell(ci, cj);
            if k == SEA {
                ci += 1;
                continue;
            }
            let x0 = ci as f32 * cs;
            let col = class_colour(k);
            if flat(&c) {
                let mut cj2 = ci + 1;
                while cj2 < n {
                    let (k2, c2) = cell(cj2, cj);
                    if k2 != k || !flat(&c2) || c2[0] != c[0] {
                        break;
                    }
                    cj2 += 1;
                }
                let x1 = cj2 as f32 * cs;
                mb.quad([bv(x0, y0, c[0]), bv(x1, y0, c[0]), bv(x1, y1, c[0]), bv(x0, y1, c[0])], Vec3::Y, col, [0.0; 2]);
                ci = cj2;
                continue;
            }
            let p = [bv(x0, y0, c[0]), bv(x0 + cs, y0, c[1]), bv(x0 + cs, y1, c[3]), bv(x0, y1, c[2])];
            // the two corner normals SW (p1-p0 x p3-p0) and NE (p3-p2 x p1-p2): both point up (the second term had its
            // operands swapped and cancelled the first's up component: /code-review s22)
            let mut nn = (p[1] - p[0]).cross(p[3] - p[0]) + (p[3] - p[2]).cross(p[1] - p[2]);
            if nn.y < 0.0 {
                nn = -nn;
            }
            if k == NATURAL || k == OPEN {
                // smooth vertex normals from the corner grid (per-tile facets read as a 2 m checkerboard: review p3 r1)
                let f = nn.normalize_or(Vec3::Y);
                let vn = [corner_n(ci, cj), corner_n(ci + 1, cj), corner_n(ci + 1, cj + 1), corner_n(ci, cj + 1)].map(|v| v.unwrap_or(f));
                mb.quad_n(p, vn, f, col, [0.0; 2]);
            } else {
                mb.quad(p, nn.normalize_or(Vec3::Y), col, [0.0; 2]);
            }
            ci += 1;
        }
    }
    const EPS: f32 = 0.005;
    for cj in 0..n {
        for ci in 0..n {
            let (kb, b) = cell(ci, cj);
            let x = ci as f32 * cs;
            let y = cj as f32 * cs;
            // west edge: the west cell's E side (SE, NE) vs this cell's W side (SW, NW)
            let (ka, a) = cell(ci - 1, cj);
            if !(ka == SEA && kb == SEA) && ((a[1] - b[0]).abs() > EPS || (a[3] - b[2]).abs() > EPS) {
                let hi_a = a[1] + a[3] > b[0] + b[2];
                let col = wall_colour(if hi_a { ka } else { kb });
                let nrm = if hi_a { Vec3::X } else { Vec3::NEG_X };
                mb.quad([bv(x, y, a[1]), bv(x, y + cs, a[3]), bv(x, y + cs, b[2]), bv(x, y, b[0])], nrm, col, [0.0; 2]);
            }
            // south edge: the south cell's N side (NW, NE) vs this cell's S side (SW, SE)
            let (ka, a) = cell(ci, cj - 1);
            if !(ka == SEA && kb == SEA) && ((a[2] - b[0]).abs() > EPS || (a[3] - b[1]).abs() > EPS) {
                let hi_a = a[2] + a[3] > b[0] + b[1];
                let col = wall_colour(if hi_a { ka } else { kb });
                // world +y (north) is Bevy -z
                let nrm = if hi_a { Vec3::NEG_Z } else { Vec3::Z };
                mb.quad([bv(x, y, a[2]), bv(x + cs, y, a[3]), bv(x + cs, y, b[1]), bv(x, y, b[0])], nrm, col, [0.0; 2]);
            }
        }
    }
    mb
}

/// Ground height under a world point inside this chunk (the tile's lowest corner), if it is land.
pub fn ground_z(g: &Ground, tile: f32, x: f32, y: f32) -> Option<f32> {
    let cm = g.n as f32 * tile;
    let i = ((x - g.cx as f32 * cm) / tile).floor() as i32;
    let j = ((y - g.cy as f32 * cm) / tile).floor() as i32;
    if i < 0 || j < 0 || i >= g.n || j >= g.n {
        return None;
    }
    let k = g.at(i, j);
    let c = g.corners[k];
    Some(c.iter().copied().fold(f32::MAX, f32::min).max(0.0))
}

/// Massing boxes -> one mesh for the window material. Per vertex: COLOR = (lit share, warm share, pattern seed,
/// upper floor height), UV_1 = (base z, ground floor height). `origin` = the chunk's SW corner (world m).
/// `boxes`: the chunk's `boxes` or its placed lots' `gen` (both index `m.buildings`).
pub fn massing_mesh(m: &ChunkMass, boxes: &[BoxRec], origin: Vec2) -> Mb {
    let mut mb = Mb::with_uv1();
    for (x0, y0, x1, y1, z0, z1, bi, part) in boxes {
        let b = &m.buildings[*bi];
        let c = [b.lit, b.warm, b.seed as f32, b.floor];
        let d = [b.base, b.ground];
        // UV_0 = (window style, crown band: a landmark's lit crown)
        // (2 = no windows: roof plant rooms)
        mb.cur_uv0 = [
            b.style as f32,
            if b.landmark && part == "crown" {
                1.0
            } else if part == "plant" {
                2.0
            } else {
                0.0
            },
        ];
        let (x0, x1, y0, y1) = (x0 - origin.x, x1 - origin.x, y0 - origin.y, y1 - origin.y);
        // Bevy: x = x, y = z, z = -y -> min corner (x0, z0, -y1), max (x1, z1, -y0)
        mb.bx(Vec3::new(x0, *z0, -y1), Vec3::new(x1, *z1, -y0), c, d);
    }
    mb
}

fn layer_colour(kind: &str) -> [f32; 4] {
    match kind {
        "expressway" | "ramp" => lin(0x3A3A42),
        "rail" => lin(0x34343C),
        "station" => lin(0x45454D),
        "walkway" => lin(0x4A4A52),
        "escalator" => lin(0x2E4A4C),
        "funicular" => lin(0x5A3A26),
        _ => lin(0x3A3A42),
    }
}

/// A deck piece (L3 fades it as a whole): its mesh, its running-surface and underside triangles (world, Bevy) for
/// the occlusion rays, and their box.
pub struct DeckPiece {
    pub mb: Mb,
    pub tris: Vec<[Vec3; 3]>,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// Pieces of a deck ribbon are cut at vertices once they reach this length (m): a slab-sized fade would pop.
pub const PIECE_M: f32 = 48.0;

/// The elevated layers: deck ribbons (top, underside, both sides) in pieces of <= ~PIECE_M, and one mesh of the
/// pylons and portal beams. World origin.
pub fn layer_parts(l: &Layers) -> (Vec<DeckPiece>, Mb) {
    let mut pieces = vec![];
    let mut solid = Mb::default();
    for el in &l.elements {
        let col = layer_colour(&el.kind);
        // the deck's running surface: asphalt on road decks, dark ballast on the rail (review p3 r1: decks read as roofs)
        let top_col = match el.kind.as_str() {
            "expressway" | "ramp" => lin(0x1A1A20),
            "rail" => lin(0x22201E),
            _ => col,
        };
        let pts = &el.pts;
        let hw = el.width / 2.0;
        // lateral offsets per vertex (mitred by the averaged tangent)
        let mut sides = Vec::with_capacity(pts.len());
        for k in 0..pts.len() {
            let a = Vec2::new(pts[k.saturating_sub(1)][0], pts[k.saturating_sub(1)][1]);
            let b = Vec2::new(pts[(k + 1).min(pts.len() - 1)][0], pts[(k + 1).min(pts.len() - 1)][1]);
            let t = (b - a).normalize_or(Vec2::X);
            let nrm = Vec2::new(-t.y, t.x);
            let p = Vec2::new(pts[k][0], pts[k][1]);
            sides.push((p + nrm * hw, p - nrm * hw, pts[k][2]));
        }
        let mut cur = DeckPiece { mb: Mb::default(), tris: vec![], lo: Vec3::MAX, hi: Vec3::MIN };
        let mut run = 0.0;
        let nseg = pts.len().saturating_sub(1);
        for k in 0..nseg {
            let (l0, r0, z0) = sides[k];
            let (l1, r1, z1) = sides[k + 1];
            let (u0, u1) = (z0 - el.thickness, z1 - el.thickness);
            let mb = &mut cur.mb;
            let top = [bv(l0.x, l0.y, z0), bv(r0.x, r0.y, z0), bv(r1.x, r1.y, z1), bv(l1.x, l1.y, z1)];
            mb.quad(top, Vec3::Y, top_col, [0.0; 2]);
            let bot = [bv(l0.x, l0.y, u0), bv(r0.x, r0.y, u0), bv(r1.x, r1.y, u1), bv(l1.x, l1.y, u1)];
            mb.quad(bot, Vec3::NEG_Y, scale(col, 0.7), [0.0; 2]);
            let dir = Vec2::new(pts[k + 1][0] - pts[k][0], pts[k + 1][1] - pts[k][1]).normalize_or(Vec2::X);
            let nl = Vec2::new(-dir.y, dir.x);
            let side = |a: Vec2, b: Vec2, n: Vec2, mb: &mut Mb| {
                mb.quad([bv(a.x, a.y, u0), bv(b.x, b.y, u1), bv(b.x, b.y, z1), bv(a.x, a.y, z0)], bv(n.x, n.y, 0.0), col, [0.0; 2]);
            };
            side(l0, l1, nl, mb);
            side(r0, r1, -nl, mb);
            for q in [top, bot] {
                cur.tris.push([q[0], q[1], q[2]]);
                cur.tris.push([q[0], q[2], q[3]]);
                for v in q {
                    cur.lo = cur.lo.min(v);
                    cur.hi = cur.hi.max(v);
                }
            }
            // deck ends
            for (kk, at_end) in [(0usize, k == 0), (pts.len() - 1, k + 1 == nseg)] {
                if !at_end {
                    continue;
                }
                let (l, r, z) = sides[kk];
                let j = if kk == 0 { 1 } else { kk - 1 };
                // outward: from the neighbouring vertex through the end
                let d = (Vec2::new(pts[kk][0], pts[kk][1]) - Vec2::new(pts[j][0], pts[j][1])).normalize_or(Vec2::X);
                let u = z - el.thickness;
                cur.mb.quad([bv(l.x, l.y, u), bv(r.x, r.y, u), bv(r.x, r.y, z), bv(l.x, l.y, z)], bv(d.x, d.y, 0.0), col, [0.0; 2]);
            }
            run += Vec2::new(pts[k + 1][0] - pts[k][0], pts[k + 1][1] - pts[k][1]).length();
            if run >= PIECE_M || k + 1 == nseg {
                let next = DeckPiece { mb: Mb::default(), tris: vec![], lo: Vec3::MAX, hi: Vec3::MIN };
                pieces.push(std::mem::replace(&mut cur, next));
                run = 0.0;
            }
        }
        let pc = scale(col, 0.8);
        for p in &el.pylons {
            let [x0, y0, x1, y1] = p.rect;
            solid.bx(Vec3::new(x0, p.z0, -y1), Vec3::new(x1, p.z1, -y0), pc, [0.0; 2]);
        }
        for b in &el.beams {
            let a = Vec2::from(b.a);
            let c = Vec2::from(b.b);
            let t = (c - a).normalize_or(Vec2::X);
            let nn = Vec2::new(-t.y, t.x) * b.w / 2.0;
            let (a0, a1, c0, c1) = (a + nn, a - nn, c + nn, c - nn);
            let q = |p: Vec2, z: f32| bv(p.x, p.y, z);
            solid.quad([q(a0, b.z1), q(a1, b.z1), q(c1, b.z1), q(c0, b.z1)], Vec3::Y, pc, [0.0; 2]);
            solid.quad([q(a0, b.z0), q(a1, b.z0), q(c1, b.z0), q(c0, b.z0)], Vec3::NEG_Y, pc, [0.0; 2]);
            solid.quad([q(a0, b.z0), q(c0, b.z0), q(c0, b.z1), q(a0, b.z1)], bv(nn.x, nn.y, 0.0).normalize_or(Vec3::X), pc, [0.0; 2]);
            solid.quad([q(a1, b.z0), q(c1, b.z0), q(c1, b.z1), q(a1, b.z1)], -bv(nn.x, nn.y, 0.0).normalize_or(Vec3::X), pc, [0.0; 2]);
        }
    }
    (pieces.into_iter().filter(|p| !p.mb.is_empty()).collect(), solid)
}

/// Pylon and beam boxes (world, Bevy min / max): occluders for the rays.
pub fn layer_boxes(l: &Layers) -> Vec<(Vec3, Vec3)> {
    let mut v = vec![];
    for el in &l.elements {
        for p in &el.pylons {
            let [x0, y0, x1, y1] = p.rect;
            v.push((Vec3::new(x0, p.z0, -y1), Vec3::new(x1, p.z1, -y0)));
        }
        for b in &el.beams {
            let (x0, x1) = (b.a[0].min(b.b[0]) - b.w / 2.0, b.a[0].max(b.b[0]) + b.w / 2.0);
            let (y0, y1) = (b.a[1].min(b.b[1]) - b.w / 2.0, b.a[1].max(b.b[1]) + b.w / 2.0);
            v.push((Vec3::new(x0, b.z0, -y1), Vec3::new(x1, b.z1, -y0)));
        }
    }
    v
}

/// Massing boxes of a chunk (world, Bevy min / max): occluders for the rays.
pub fn massing_boxes(boxes: &[BoxRec]) -> Vec<(Vec3, Vec3)> {
    boxes.iter().map(|(x0, y0, x1, y1, z0, z1, _, _)| (Vec3::new(*x0, *z0, -*y1), Vec3::new(*x1, *z1, -*y0))).collect()
}

/// Ray vs triangle (Moller-Trumbore), either side: distance if within (0, max_t).
pub fn ray_tri(o: Vec3, d: Vec3, t: &[Vec3; 3], max_t: f32) -> Option<f32> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let tt = e2.dot(q) * inv;
    (tt > 1e-3 && tt < max_t).then_some(tt)
}
