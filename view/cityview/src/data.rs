//! The generator's chunk files (`assets/city/<city>/out/chunks/`, pipeline/city/chunks.py): the index, a
//! chunk's ground tiles (`.ground.bin`), its massing (`.json`) and the elevated layers (`layers.json`).
//! World frame: x east, y north, z up, metres, origin at the island bbox SW corner (CITY_SPEC §GRID).

use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Clone, Debug)]
pub struct Index {
    pub city: String,
    pub seed: i64,
    pub tile: f32,
    pub chunk_m: f32,
    pub apron: i32,
    pub size_m: [f32; 2],
    pub chunks: Vec<ChunkRec>,
    pub pins: Vec<Pin>,
    pub views: std::collections::BTreeMap<String, [f32; 2]>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ChunkRec {
    pub c: [i32; 2],
    pub ground: bool,
    pub boxes: usize,
    /// Ground (land) z .. the highest roof, m (view culling)
    pub zr: [f32; 2],
    /// The massing boxes' plan extent (they may overhang the chunk), m
    #[serde(default)]
    pub bb: Option<[f32; 4]>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Pin {
    pub block: String,
    /// World point of the block frame's origin (the first lot's SW corner on its platform)
    pub origin_m: [f32; 3],
}

impl Index {
    pub fn load(dir: &Path) -> Index {
        let p = dir.join("index.json");
        let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e} (run the generator first)", p.display()));
        serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Bldg {
    pub base: f32,
    pub ground: f32,
    pub floor: f32,
    pub lit: f32,
    pub warm: f32,
    pub seed: u32,
    /// Window grammar (massing.py STYLE)
    #[serde(default)]
    pub style: i32,
    #[serde(default)]
    pub landmark: bool,
    /// The city lot it stands on (a generated building replaces its massing: `PLACED`)
    #[serde(default)]
    pub lot: String,
}

/// The lots whose massing a placed building replaces (set once at startup; the chunk builder threads read it).
pub static PLACED: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();

/// A kit building placed in the city (P6: pipeline/buildings/kit.py writes out/bldg/kitplaced_<block>.json).
#[derive(Deserialize, Clone, Debug)]
pub struct KitPlacedRec {
    pub lot: String,
    pub id: String,
    /// assets-relative: the kit stream and its block's block.json (atlas + materials)
    pub kit: String,
    pub block: String,
    pub floors: i32,
    pub at: [f32; 3],
    pub yaw: f32,
    pub x: [f32; 2],
    /// The lot rect (city metres; kit.py since P8 B-A): its centre keys the building's chunk, as the massing's
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
}

impl KitPlacedRec {
    /// The chunk of the lot centre (the massing chunk holding its boxes); the root's corner without a rect.
    pub fn chunk(&self, chunk_m: f32) -> (i32, i32) {
        let (x, y) = self.rect.map_or((self.at[0], self.at[1]), |r| ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0));
        ((x / chunk_m).floor() as i32, (y / chunk_m).floor() as i32)
    }
}

/// Every out/bldg/kitplaced_*.json of the city (sorted by file name).
pub fn kit_placed(city_dir: &Path) -> Vec<KitPlacedRec> {
    let mut files: Vec<_> = std::fs::read_dir(city_dir.join("out/bldg"))
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    files.retain(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("kitplaced_") && n.ends_with(".json")));
    files.sort();
    let mut out = vec![];
    for p in files {
        let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let v: Vec<KitPlacedRec> = serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        out.extend(v);
    }
    out
}

/// [x0, y0, x1, y1, z0, z1, building index, part]
pub type BoxRec = (f32, f32, f32, f32, f32, f32, usize, String);

#[derive(Deserialize, Clone, Debug, Default)]
pub struct ChunkMass {
    pub buildings: Vec<Bldg>,
    pub boxes: Vec<BoxRec>,
    /// The placed (generated) lots' boxes, split off `boxes` at load: the stand-in while their kits stream in
    #[serde(skip)]
    pub standin: Vec<BoxRec>,
}

impl ChunkMass {
    pub fn load(path: &Path) -> Option<ChunkMass> {
        // errors are logged: a schema change must not stream an empty city silently (/code-review s22)
        let txt = std::fs::read_to_string(path).map_err(|e| bevy::log::error!("{}: {e}", path.display())).ok()?;
        let mut m: ChunkMass = serde_json::from_str(&txt).map_err(|e| bevy::log::error!("{}: {e}", path.display())).ok()?;
        // a placed (generated) building replaces its lot's massing boxes (and their occlusion rays): they move to
        // `standin`, shown only while the chunk's kit buildings are not in (P8 B-A streaming)
        if let Some(placed) = PLACED.get().filter(|s| !s.is_empty()) {
            let b = &m.buildings;
            let (standin, keep): (Vec<BoxRec>, Vec<BoxRec>) =
                m.boxes.drain(..).partition(|bx| b.get(bx.6).is_some_and(|bd| placed.contains(&bd.lot)));
            m.boxes = keep;
            m.standin = standin;
        }
        Some(m)
    }
}

/// Ground classes (chunks.py CLASSES).
pub const SEA: u8 = 0;
pub const NATURAL: u8 = 1;
pub const OPEN: u8 = 2;
pub const PLATFORM: u8 = 3;
pub const CARRIAGE: u8 = 4;
pub const MEDIAN: u8 = 5;
pub const SIDEWALK: u8 = 6;
pub const PED: u8 = 7;
pub const STAIRS: u8 = 8;

/// A chunk's ground tiles with a 1-tile apron: `w` x `w` tiles, row-major S -> N, each with its corner heights
/// (SW, SE, NW, NE; m) and class. Tile (i, j) of the chunk is at apron index (i + apron, j + apron).
#[derive(Clone, Debug)]
pub struct Ground {
    pub cx: i32,
    pub cy: i32,
    pub n: i32,
    pub w: i32,
    pub corners: Vec<[f32; 4]>,
    pub class: Vec<u8>,
}

impl Ground {
    pub fn load(path: &Path, apron: i32) -> Option<Ground> {
        let b = std::fs::read(path).map_err(|e| bevy::log::error!("{}: {e}", path.display())).ok()?;
        if b.len() < 16 || &b[0..4] != b"CGND" {
            bevy::log::error!("{}: not a CGND ground file", path.display());
            return None;
        }
        let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        let i32_at = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let (ver, n, cx, cy) = (u16_at(4), u16_at(6) as i32, i32_at(8), i32_at(12));
        if ver != 1 {
            bevy::log::error!("{}: ground version {ver}, expected 1", path.display());
            return None;
        }
        let w = n + 2 * apron;
        let nt = (w * w) as usize;
        let need = 16 + nt * 8 + nt;
        if b.len() != need {
            bevy::log::error!("{}: {} bytes, expected {need}", path.display(), b.len());
            return None;
        }
        let mut corners = Vec::with_capacity(nt);
        for k in 0..nt {
            let o = 16 + k * 8;
            let z = |q: usize| (u16_at(o + q * 2) as f32 - 5000.0) / 100.0;
            corners.push([z(0), z(1), z(2), z(3)]);
        }
        let class = b[16 + nt * 8..].to_vec();
        Some(Ground { cx, cy, n, w, corners, class })
    }

    /// Apron index of chunk tile (i, j); i, j in -apron .. n + apron.
    #[inline]
    pub fn at(&self, i: i32, j: i32) -> usize {
        let a = (self.w - self.n) / 2;
        ((j + a) * self.w + (i + a)) as usize
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Layers {
    pub elements: Vec<Element>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Element {
    pub kind: String,
    pub width: f32,
    pub thickness: f32,
    pub pts: Vec<[f32; 3]>,
    pub pylons: Vec<Pylon>,
    pub beams: Vec<Beam>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Pylon {
    pub rect: [f32; 4],
    pub z0: f32,
    pub z1: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Beam {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub z0: f32,
    pub z1: f32,
    pub w: f32,
}

impl Layers {
    pub fn load(dir: &Path) -> Layers {
        let p = dir.join("layers.json");
        let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }
}
