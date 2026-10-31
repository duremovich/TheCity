//! Kit buildings (P6, spec/city/P6_PLAN.md): a building assembled from its kit stream (`<bldg>.kit`, written by
//! pipeline/buildings/kit.py through pipeline/blender/building/kitrec.py) and its block's atlas (`block.json`,
//! `atlas.png`, `mr.png`) instead of a glb.
//!
//! The stream holds the building's faces per look-dev node (building frame, z up) with a material key per face,
//! each node's game group (`vis`: Main / Hide / Cut) and whether it stays without interiors (`ext`), and the
//! lights as the glb ships them. Off the main thread (AsyncComputeTaskPool) the faces are stamped into one mesh
//! per (floor, group, material kind), the way export.py's gamify merges a glb; `poll_kits` spawns them with the
//! glb path's components (`GlNode` per (floor, group) with mesh children, `GlLight` on light children of light
//! nodes), so the cutaway, the light rules and occlusion run unchanged. `KitReady` fires on the building root
//! when it is spawned (occlusion's material swap listens to it as to a glb's `WorldInstanceReady`).
//!
//! Interiors are lazy (CITY_SPEC §PERF): `KitRef::interiors == false` stamps only the `ext` nodes (shells,
//! facade, signs, props, the roof slab); with a `KitRing` resource the client re-stamps buildings as the focus
//! moves (interiors inside the chunk ring around it, shells outside), otherwise the `KitRef` choice stands.

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    image::{ImageLoaderSettings, ImageSampler},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::city::{Bldg, City, CutKind, GlLight, GlNode, LM_PER_W, ReadyBuildings, Role};
use crate::frame::to_bevy;

/// A kit building's sources (assets-relative) and whether its interior nodes are stamped.
#[derive(Clone, Debug)]
pub struct KitRef {
    pub kit: String,
    pub block: String,
    pub interiors: bool,
}

/// Fired on a kit building's root once its nodes are spawned.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct KitReady {
    pub entity: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MatKind {
    Opaque,
    Emit,
    Glass,
    Cutout,
}

#[derive(Deserialize, Clone, Debug)]
struct MatRec {
    kind: String,
    #[serde(default)]
    rect: Option<[f32; 4]>,
    #[serde(default)]
    image: bool,
    #[serde(default)]
    rgb: Option<[f32; 3]>,
    #[serde(default)]
    rough: Option<f32>,
    #[serde(default)]
    alpha: Option<f32>,
}

#[derive(Deserialize, Clone, Debug)]
struct BlockJson {
    size: f32,
    s_max: f32,
    atlas: String,
    mr: String,
    mats: HashMap<String, MatRec>,
}

/// A block's atlas info (shared by its buildings' stamping tasks) and its four materials.
pub struct KitBlock {
    info: Arc<BlockJson>,
    mats: HashMap<MatKind, Handle<StandardMaterial>>,
}

#[derive(Resource, Default)]
pub struct KitBlocks(pub HashMap<String, KitBlock>);

struct NodeMesh {
    floor: i32,
    cut: CutKind,
    kind: MatKind,
    mesh: Mesh,
    aabb: Aabb,
}

#[derive(Clone)]
struct KitLight {
    name: String,
    role: Role,
    shadow: bool,
    floor: i32,
    interior_floor: i32,
    energy_w: f32,
    range: f32,
    radius: f32,
    color: Color,
    at: Vec3,
    dir: Vec3,
    spot: Option<(f32, f32)>,
    priority: f32,
    down: bool,
    cone: Option<(f32, f32)>,
}

struct Built {
    meshes: Vec<NodeMesh>,
    lights: Vec<KitLight>,
    faces: usize,
}

/// Stamping tasks in flight: (building root, building index, with interiors, task).
#[derive(Resource, Default)]
pub struct KitTasks(Vec<(Entity, usize, bool, Task<Result<Built, String>>)>);

/// A kit building's detail: interiors stamped or not, a re-stamp in flight, its lights spawned (once).
#[derive(Component, Debug)]
pub struct KitState {
    pub interiors: bool,
    pub pending: bool,
    pub spawned: bool,
    /// its lights are spawned
    pub lights: bool,
    /// a stamp failed: counted as ready (benches do not wait on it), never re-stamped
    pub failed: bool,
}

/// Lazy interiors (CITY_SPEC §PERF): with this resource, kit buildings within `ring` chunks of the focus's chunk
/// (`chunk_m`, the host's streaming grid in the city frame) carry interiors; the others are re-stamped as shells
/// (the `ext` nodes). Without it every building keeps the detail its `KitRef` asked for.
/// Lights are lazy too: a building's lights exist while its root is within `light_m` of the focus (+ 16 m
/// hysteresis); far lights cannot reach the game view (ranges <= ~20 m) and cost per entity every frame.
#[derive(Resource, Clone, Copy, Debug)]
pub struct KitRing {
    pub chunk_m: f32,
    pub ring: i32,
    pub light_m: f32,
}

/// Kit streaming (P8 B-A, spec/city/P8_PLAN.md): with this resource, kit buildings exist only in the chunks
/// around the focus. Chunks within `radius` (Chebyshev, in chunks of `chunk_m`, around the focus's chunk) load
/// nearest first while fewer than `max_tasks` stamps are in flight; loaded chunks beyond `keep` (> radius: the
/// hysteresis) despawn their buildings (meshes, lights, twins go with the roots). `setup_city` leaves the managed
/// buildings to it. A host shows its stand-in (cityview: the lot's massing) until `chunk_ready`.
#[derive(Resource)]
pub struct KitStream {
    pub chunk_m: f32,
    pub radius: i32,
    pub keep: i32,
    pub max_tasks: usize,
    by_chunk: HashMap<(i32, i32), Vec<usize>>,
    managed: std::collections::HashSet<usize>,
    loaded: HashMap<(i32, i32), Vec<Entity>>,
    ready: std::collections::HashSet<usize>,
    want: Vec<(i32, i32)>,
    pub stats: KitStreamStats,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct KitStreamStats {
    /// chunks with kit buildings loaded, buildings spawned now, and over the run: chunks loaded / unloaded
    pub chunks: usize,
    pub buildings: usize,
    pub loads: u64,
    pub unloads: u64,
    /// buildings spawned (first stamps) over the run, and this frame (the bench's kit-frame split)
    pub spawned: u64,
    pub spawned_frame: usize,
    /// main-thread ms this frame in kit_stream + poll_kits (the bench splits stalls by it)
    pub main_ms: f32,
}

impl KitStream {
    /// `chunks`: (building index, its chunk) for every kit building the stream manages (a host keys a building by
    /// its lot centre, as the massing chunks do).
    pub fn new(chunk_m: f32, radius: i32, keep: i32, max_tasks: usize, chunks: impl IntoIterator<Item = (usize, (i32, i32))>) -> KitStream {
        let mut by_chunk: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, c) in chunks {
            by_chunk.entry(c).or_default().push(i);
        }
        let managed = by_chunk.values().flatten().copied().collect();
        KitStream {
            chunk_m,
            radius,
            keep: keep.max(radius),
            max_tasks,
            by_chunk,
            managed,
            loaded: HashMap::new(),
            ready: Default::default(),
            want: vec![],
            stats: KitStreamStats::default(),
        }
    }

    /// Buildings the stream manages.
    pub fn total(&self) -> usize {
        self.managed.len()
    }

    pub fn manages(&self, idx: usize) -> bool {
        self.managed.contains(&idx)
    }

    /// Every building of the chunk is spawned (or failed): the host's stand-in may go. A chunk without kit
    /// buildings is never ready (nothing replaces its stand-in).
    pub fn chunk_ready(&self, c: (i32, i32)) -> bool {
        self.loaded.contains_key(&c) && self.by_chunk.get(&c).is_some_and(|v| v.iter().all(|i| self.ready.contains(i)))
    }

    /// Everything wanted is ready (a bench waits on this).
    pub fn idle(&self) -> bool {
        self.want.iter().all(|c| self.chunk_ready(*c))
    }
}

/// The host's buildings are all ready: every glb building and, with a stream, every wanted kit chunk (without one,
/// every kit building).
pub fn buildings_ready(city: &City, ready: &ReadyBuildings, ks: Option<&KitStream>) -> bool {
    match ks {
        Some(ks) => ready.0 >= city.bldgs.len() - ks.total() && ks.idle(),
        None => ready.0 >= city.bldgs.len(),
    }
}

/// Kit stats for the hud / benches: buildings spawned, faces stamped, meshes spawned.
#[derive(Resource, Default, Debug)]
pub struct KitStats {
    pub buildings: usize,
    pub faces: usize,
    pub meshes: usize,
}

/// The block's materials (loaded once per block) and the building's stamping task.
fn start(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    blocks: &mut KitBlocks,
    root_dir: &Path,
    k: &KitRef,
) -> Task<Result<Built, String>> {
    if !blocks.0.contains_key(&k.block) {
        let path = root_dir.join(&k.block);
        let txt = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let info: BlockJson = serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let dir = Path::new(&k.block).parent().map(Path::to_path_buf).unwrap_or_default();
        let atlas: Handle<Image> = asset_server
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| s.sampler = ImageSampler::nearest())
            .load(dir.join(&info.atlas));
        let mr: Handle<Image> = asset_server
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| {
                s.sampler = ImageSampler::nearest();
                s.is_srgb = false;
            })
            .load(dir.join(&info.mr));
        let glass = info.mats.values().find(|m| m.kind == "glass");
        let mut mats = HashMap::new();
        // export.game_materials: Opaque = atlas base + MR, Emit = black base, the atlas as emission x s_max,
        // Glass = #0E1218 alpha 0.18 rough 0.05 blended, Cutout = the atlas with its alpha as a mask, rough 0.8
        mats.insert(
            MatKind::Opaque,
            materials.add(StandardMaterial {
                base_color_texture: Some(atlas.clone()),
                metallic_roughness_texture: Some(mr.clone()),
                metallic: 1.0,
                perceptual_roughness: 1.0,
                ..two_sided()
            }),
        );
        mats.insert(
            MatKind::Emit,
            materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::rgb(info.s_max, info.s_max, info.s_max),
                emissive_texture: Some(atlas.clone()),
                perceptual_roughness: 0.4,
                ..two_sided()
            }),
        );
        let (g_rgb, g_rough, g_alpha) = glass.map_or(([0.0045, 0.0060, 0.0091], 0.05, 0.18), |g| {
            (g.rgb.unwrap_or([0.0045, 0.0060, 0.0091]), g.rough.unwrap_or(0.05), g.alpha.unwrap_or(0.18))
        });
        mats.insert(
            MatKind::Glass,
            materials.add(StandardMaterial {
                base_color: Color::linear_rgba(g_rgb[0], g_rgb[1], g_rgb[2], g_alpha),
                perceptual_roughness: g_rough,
                alpha_mode: AlphaMode::Blend,
                ..two_sided()
            }),
        );
        mats.insert(
            MatKind::Cutout,
            materials.add(StandardMaterial {
                base_color_texture: Some(atlas.clone()),
                perceptual_roughness: 0.8,
                alpha_mode: AlphaMode::Mask(0.5),
                ..two_sided()
            }),
        );
        blocks.0.insert(k.block.clone(), KitBlock { info: Arc::new(info), mats });
    }
    let info = blocks.0[&k.block].info.clone();
    let path: PathBuf = root_dir.join(&k.kit);
    restamp(info, path, k.interiors)
}

fn restamp(info: Arc<BlockJson>, path: PathBuf, interiors: bool) -> Task<Result<Built, String>> {
    AsyncComputeTaskPool::get().spawn(async move { stamp(&path, &info, interiors) })
}

/// Blender exports every building material `doubleSided` and bevy_gltf then turns culling off (shading and
/// shadow casting): single-sided pieces (awnings, shutters, decals, sign faces) cast shadows from both sides.
fn two_sided() -> StandardMaterial {
    StandardMaterial { double_sided: true, cull_mode: None, ..default() }
}

// ---------------------------------------------------------------------------------------------- the stream

struct Stream {
    h: Value,
    fsize: Vec<u8>,
    fmat: Vec<u16>,
    fv0: Vec<u32>,
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
}

fn read_stream(path: &Path) -> Result<Stream, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // v2 / v3 (kitfile.py): zlib blob, quantised positions; v3 adds the kit modules (expanded here)
    let v2 = data.len() >= 12 && (&data[..8] == b"GLKIT002" || &data[..8] == b"GLKIT003");
    if data.len() < 12 || !(v2 || &data[..8] == b"GLKIT001") {
        return Err(format!("{}: not a kit stream", path.display()));
    }
    let n = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let mut h: Value = serde_json::from_slice(&data[12..12 + n]).map_err(|e| format!("{}: {e}", path.display()))?;
    // v2 (kitfile.py): the blob is zlib; positions are i32 running differences of round(pos * Q), fv0 is derived
    let blob: Vec<u8> = if v2 {
        use std::io::Read;
        let mut out = vec![];
        flate2::read::ZlibDecoder::new(&data[12 + n..]).read_to_end(&mut out).map_err(|e| format!("{}: {e}", path.display()))?;
        out
    } else {
        data[12 + n..].to_vec()
    };
    let arr = |name: &str| -> Result<&[u8], String> {
        let a = h["arrays"][name].as_array().ok_or(format!("{}: no array {name}", path.display()))?;
        let off = a[0].as_u64().unwrap_or(0) as usize;
        let count = a[1].as_u64().unwrap_or(0) as usize;
        let width = match a[2].as_str().unwrap_or("") {
            "u8" => 1,
            "u16" => 2,
            _ => 4,
        };
        let end = off + count * width;
        if end > blob.len() {
            return Err(format!("{}: array {name} past the end", path.display()));
        }
        Ok(&blob[off..end])
    };
    let fsize = arr("fsize")?.to_vec();
    let u16s = |b: &[u8]| -> Vec<u16> { b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect() };
    let f32s = |b: &[u8]| -> Vec<f32> { b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect() };
    let fmat = u16s(arr("fmat")?);
    let q = h["quantum"].as_f64().unwrap_or(8192.0);
    // i32 running differences of round(p * Q) -> points (f64: the module points are placed before the f32 cast)
    let undelta = |b: &[u8]| -> Vec<[f64; 3]> {
        let mut acc = [0i64; 3];
        b.chunks_exact(12)
            .map(|c| {
                let mut p = [0f64; 3];
                for k in 0..3 {
                    acc[k] += i32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap()) as i64;
                    p[k] = acc[k] as f64 / q;
                }
                p
            })
            .collect()
    };
    let pos: Vec<[f32; 3]> = if v2 {
        undelta(arr("posq")?).iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect()
    } else {
        f32s(arr("pos")?).chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
    };
    let uv: Vec<[f32; 2]> = f32s(arr("uv")?).chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    let starts = |sizes: &[u8]| -> Vec<u32> {
        let mut at = 0u32;
        sizes
            .iter()
            .map(|&n| {
                let v = at;
                at += n as u32;
                v
            })
            .collect()
    };
    let fv0 = if v2 { starts(&fsize) } else { arr("fv0")?.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() };
    if h.get("modules").is_none() {
        return Ok(Stream { h, fsize, fmat, fv0, pos, uv });
    }
    // v3: per node its own faces, then each instance's module faces placed by the instance frame (kitfile.expand)
    let msize = arr("msize")?.to_vec();
    let mmat = u16s(arr("mmat")?);
    let mpos = undelta(arr("mposq")?);
    let mv0 = starts(&msize);
    let imod: Vec<u32> = arr("imod")?.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    let ipos = f32s(arr("ipos")?);
    let iyaw = f32s(arr("iyaw")?);
    let mods: Vec<(usize, usize)> = h["modules"]
        .as_array()
        .ok_or("modules")?
        .iter()
        .map(|m| (m["f0"].as_u64().unwrap_or(0) as usize, m["nf"].as_u64().unwrap_or(0) as usize))
        .collect();
    let mut s = Stream { h: Value::Null, fsize: vec![], fmat: vec![], fv0: vec![], pos: vec![], uv: vec![] };
    let bad = || format!("{}: a module / instance index past the end", path.display());
    for nd in h["nodes"].as_array_mut().ok_or("nodes")?.iter_mut() {
        let first = s.fsize.len();
        let (f0, nf) = (nd["f0"].as_u64().unwrap_or(0) as usize, nd["nf"].as_u64().unwrap_or(0) as usize);
        for f in f0..f0 + nf {
            let (v0, k) = (*fv0.get(f).ok_or_else(bad)? as usize, *fsize.get(f).ok_or_else(bad)? as usize);
            s.pos.extend_from_slice(pos.get(v0..v0 + k).ok_or_else(bad)?);
            s.uv.extend_from_slice(uv.get(v0..v0 + k).ok_or_else(bad)?);
            s.fsize.push(k as u8);
            s.fmat.push(fmat[f]);
        }
        let (i0, ni) = (nd["i0"].as_u64().unwrap_or(0) as usize, nd["ni"].as_u64().unwrap_or(0) as usize);
        for i in i0..i0 + ni {
            let &(g0, gn) = mods.get(*imod.get(i).ok_or_else(bad)? as usize).ok_or_else(bad)?;
            let o = ipos.get(3 * i..3 * i + 3).ok_or_else(bad)?;
            let (sn, cs) = (*iyaw.get(i).ok_or_else(bad)? as f64).sin_cos();
            for g in g0..g0 + gn {
                let (v0, k) = (*mv0.get(g).ok_or_else(bad)? as usize, *msize.get(g).ok_or_else(bad)? as usize);
                for p in mpos.get(v0..v0 + k).ok_or_else(bad)? {
                    s.pos.push([
                        (p[0] * cs - p[1] * sn + o[0] as f64) as f32,
                        (p[0] * sn + p[1] * cs + o[1] as f64) as f32,
                        (p[2] + o[2] as f64) as f32,
                    ]);
                    s.uv.push([0.0; 2]);
                }
                s.fsize.push(k as u8);
                s.fmat.push(mmat[g]);
            }
        }
        nd["f0"] = (first as u64).into();
        nd["nf"] = ((s.fsize.len() - first) as u64).into();
    }
    s.fv0 = starts(&s.fsize);
    s.h = h;
    Ok(s)
}

#[derive(Default)]
struct Acc {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    lo: Vec3,
    hi: Vec3,
}

fn cut_of(vis: &str) -> CutKind {
    match vis {
        "Hide" => CutKind::HideOnCut,
        "Cut" => CutKind::CutOnly,
        _ => CutKind::Main,
    }
}

fn kind_of(k: &str) -> MatKind {
    match k {
        "emit" => MatKind::Emit,
        "glass" => MatKind::Glass,
        "cutout" => MatKind::Cutout,
        _ => MatKind::Opaque,
    }
}

/// Triangles of a planar polygon (indices into `v`): a fan when convex, ear clipping otherwise (stair
/// stringers, sawtooth caps).
fn triangulate(v: &[Vec3], n: Vec3, out: &mut Vec<[usize; 3]>) {
    let k = v.len();
    if k < 3 {
        return;
    }
    let convex = (0..k).all(|i| (v[(i + 1) % k] - v[i]).cross(v[(i + 2) % k] - v[(i + 1) % k]).dot(n) >= -1e-9);
    if convex {
        for i in 1..k - 1 {
            out.push([0, i, i + 1]);
        }
        return;
    }
    let mut rest: Vec<usize> = (0..k).collect();
    let mut guard = 0;
    while rest.len() > 3 && guard < 4 * k * k {
        guard += 1;
        let m = rest.len();
        let mut clipped = false;
        for i in 0..m {
            let (a, b, c) = (rest[(i + m - 1) % m], rest[i], rest[(i + 1) % m]);
            if (v[b] - v[a]).cross(v[c] - v[b]).dot(n) <= 1e-12 {
                continue;
            }
            let inside = rest.iter().any(|&p| {
                p != a && p != b && p != c && {
                    let s = |p0: Vec3, p1: Vec3| (p1 - p0).cross(v[p] - p0).dot(n);
                    s(v[a], v[b]) > 1e-12 && s(v[b], v[c]) > 1e-12 && s(v[c], v[a]) > 1e-12
                }
            });
            if inside {
                continue;
            }
            out.push([a, b, c]);
            rest.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    for i in 1..rest.len().saturating_sub(1) {
        out.push([rest[0], rest[i], rest[i + 1]]);
    }
}

fn stamp(path: &Path, info: &BlockJson, interiors: bool) -> Result<Built, String> {
    let s = read_stream(path)?;
    let keys: Vec<String> = s.h["mats"].as_array().ok_or("no mats")?.iter().map(|k| k.as_str().unwrap_or("").to_string()).collect();
    // per material key: kind, atlas rect (px, top-down) or none (glass), textured (a sign image)
    let mk: Vec<(MatKind, Option<[f32; 4]>, bool)> = keys
        .iter()
        .map(|k| info.mats.get(k).map_or((MatKind::Opaque, None, false), |m| (kind_of(&m.kind), m.rect, m.image)))
        .collect();
    let mut groups: HashMap<(i32, CutKind, MatKind), Acc> = HashMap::new();
    let mut faces = 0;
    let mut tris = vec![];
    for nd in s.h["nodes"].as_array().ok_or("no nodes")? {
        if !interiors && !nd["ext"].as_bool().unwrap_or(true) {
            continue;
        }
        let mut floor = nd["floor"].as_i64().unwrap_or(0) as i32;
        let mut cut = cut_of(nd["vis"].as_str().unwrap_or("Main"));
        if !interiors {
            // a shell building (outside the interior ring) is never cut: its normal view (Main + Hide) merges
            // into one mesh per material kind, the Cut stubs go
            if cut == CutKind::CutOnly {
                continue;
            }
            (floor, cut) = (0, CutKind::Main);
        }
        let f0 = nd["f0"].as_u64().unwrap_or(0) as usize;
        let nf = nd["nf"].as_u64().unwrap_or(0) as usize;
        for f in f0..f0 + nf {
            let (kind, rect, image) = mk[s.fmat[f] as usize];
            let v0 = s.fv0[f] as usize;
            let n = s.fsize[f] as usize;
            let pb: Vec<Vec3> = (v0..v0 + n).map(|i| Vec3::from(s.pos[i])).collect();
            // Newell normal (building frame), the face's winding is the recorded one (outward)
            let mut nb = Vec3::ZERO;
            for i in 0..n {
                let (a, b) = (pb[i], pb[(i + 1) % n]);
                nb += Vec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
            }
            let nb = nb.normalize_or_zero();
            let acc = groups.entry((floor, cut, kind)).or_insert_with(|| Acc { lo: Vec3::splat(f32::MAX), hi: Vec3::splat(f32::MIN), ..default() });
            let base = acc.pos.len() as u32;
            let nv = to_bevy(nb).to_array();
            for (j, p) in pb.iter().enumerate() {
                let q = to_bevy(*p);
                acc.lo = acc.lo.min(q);
                acc.hi = acc.hi.max(q);
                acc.pos.push(q.to_array());
                acc.nrm.push(nv);
                // atlas UV (glTF / Bevy: v down from the top): a swatch's centre, a sign image's own UV (Blender
                // convention, v up) mapped into its rect; glass has no texture
                let uv = match rect {
                    None => [0.0, 0.0],
                    Some([x, y, w, h]) if image => {
                        let [u, v] = s.uv[v0 + j];
                        [(x + u.clamp(0.0, 1.0) * w) / info.size, (y + (1.0 - v.clamp(0.0, 1.0)) * h) / info.size]
                    }
                    Some([x, y, w, h]) => [(x + 0.5 * w) / info.size, (y + 0.5 * h) / info.size],
                };
                acc.uv.push(uv);
            }
            tris.clear();
            triangulate(&pb, nb, &mut tris);
            for t in tris.iter() {
                acc.idx.extend([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
            }
            faces += 1;
        }
    }
    let mut meshes = vec![];
    for ((floor, cut, kind), acc) in groups {
        if acc.idx.is_empty() {
            continue;
        }
        let aabb = Aabb::from_min_max(acc.lo, acc.hi);
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, acc.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, acc.nrm);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, acc.uv);
        mesh.insert_indices(Indices::U32(acc.idx));
        meshes.push(NodeMesh { floor, cut, kind, mesh, aabb });
    }
    meshes.sort_by_key(|m| (m.floor, m.cut as u8, m.kind as u8));
    let mut lights = vec![];
    for l in s.h["lights"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
        let f = |k: &str| l[k].as_f64().unwrap_or(0.0) as f32;
        let v3 = |k: &str| {
            let a = l[k].as_array();
            a.map_or(Vec3::ZERO, |a| Vec3::new(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32))
        };
        let pair = |k: &str| l[k].as_array().and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
        let c = v3("color");
        lights.push(KitLight {
            name: l["name"].as_str().unwrap_or("").to_string(),
            role: match l["role"].as_str().unwrap_or("") {
                "shadowed" => Role::Shadowed,
                "interior" => Role::Interior,
                _ => Role::Unshadowed,
            },
            shadow: l["shadow"].as_bool().unwrap_or(false),
            floor: l["floor"].as_i64().unwrap_or(0) as i32,
            interior_floor: l["interior_floor"].as_i64().unwrap_or(-1) as i32,
            energy_w: f("energy_w"),
            range: f("range"),
            radius: f("radius"),
            // bevy_gltf 0.19 reads KHR light colours as sRGB (loader/mod.rs: srgb_from_array, the spec says linear);
            // the look was tuned on the glb path, so the kit matches the loader
            color: Color::srgb(c.x, c.y, c.z),
            at: to_bevy(v3("at")),
            dir: to_bevy(v3("dir")),
            spot: pair("spot"),
            priority: l["priority"].as_f64().unwrap_or(1.0) as f32,
            down: l["fixture"].as_str().is_none_or(|f| f != "omni"),
            cone: pair("cone_deg").map(|(i, o)| (i.to_radians(), o.to_radians())),
        });
    }
    Ok(Built { meshes, lights, faces })
}

// ---------------------------------------------------------------------------------------------- spawning

/// Spawns finished buildings: a node per (floor, group) with one mesh child per material kind, the lights.
#[allow(clippy::too_many_arguments)]
pub fn poll_kits(
    mut commands: Commands,
    mut tasks: ResMut<KitTasks>,
    mut meshes: ResMut<Assets<Mesh>>,
    blocks: Res<KitBlocks>,
    city: Res<City>,
    mut ready: ResMut<ReadyBuildings>,
    mut stats: ResMut<KitStats>,
    mut states: Query<(&mut KitState, Option<&Children>)>,
    old_nodes: Query<(), With<GlNode>>,
    real: Res<Time<Real>>,
    ring: Option<Res<KitRing>>,
    focus: Res<crate::frame::Focus>,
    mut stream: Option<ResMut<KitStream>>,
) {
    // a streamed walk finishes many stamps at once: spawn at most INSERT_MAX per frame (the rest stay unpolled)
    let mut inserted = 0;
    let t0 = std::time::Instant::now();
    if let Some(s) = stream.as_mut() {
        s.stats.spawned_frame = 0;
    }
    let insert_max: usize = std::env::var("CV_KIT_INSERT").ok().and_then(|v| v.parse().ok()).unwrap_or(INSERT_MAX);
    let mut i = 0;
    while i < tasks.0.len() {
        if stream.is_some() && inserted >= insert_max {
            break;
        }
        let Some(res) = block_on(poll_once(&mut tasks.0[i].3)) else {
            i += 1;
            continue;
        };
        let (root, idx, interiors, _) = tasks.0.swap_remove(i);
        let Ok((mut st, kids)) = states.get_mut(root) else { continue };
        st.pending = false;
        let built = match res {
            Ok(b) => b,
            Err(e) => {
                error!("kit building {idx}: {e}");
                st.failed = true;
                if !st.spawned {
                    st.spawned = true;
                    match stream.as_mut() {
                        Some(s) => {
                            s.ready.insert(idx);
                        }
                        None => ready.0 += 1,
                    }
                }
                continue;
            }
        };
        let kind = &city.kinds[city.bldgs[idx].kind];
        let Some(kr) = kind.kit.as_ref() else { continue };
        let mats = &blocks.0[&kr.block].mats;
        // a re-stamp (lazy interiors) replaces the mesh nodes; the lights stay
        for &c in kids.map(|k| &k[..]).unwrap_or(&[]) {
            if old_nodes.contains(c) {
                commands.entity(c).despawn();
            }
        }
        st.interiors = interiors;
        let first = !st.spawned;
        st.spawned = true;
        let mut nodes: HashMap<(i32, CutKind), Entity> = HashMap::new();
        for m in built.meshes {
            let node = *nodes.entry((m.floor, m.cut)).or_insert_with(|| {
                commands
                    .spawn((
                        Transform::IDENTITY,
                        Visibility::default(),
                        GlNode { bldg: idx, floor: m.floor, cut: m.cut, lod: 0, data: false },
                        ChildOf(root),
                    ))
                    .id()
            });
            commands.spawn((Mesh3d(meshes.add(m.mesh)), MeshMaterial3d(mats[&m.kind].clone()), m.aabb, Transform::IDENTITY, ChildOf(node)));
            stats.meshes += 1;
        }
        if first {
            // without a ring every light lives; with one, the lights of a building near the focus spawn with its
            // meshes (as a glb's: the shadow allocator's hysteresis keeps whichever lights came first), kit_ring
            // spawns / drops them as the focus moves
            let lights = KitLights(Arc::new(built.lights));
            let near = ring.as_ref().is_none_or(|r| (city.bldgs[idx].transform.translation - focus.at).with_y(0.0).length() < r.light_m);
            if near && std::env::var("GL_KIT_NO_LIGHTS").is_err() {
                spawn_lights(&mut commands, root, idx, &lights.0);
                st.lights = true;
            }
            commands.entity(root).insert(lights);
        }
        stats.faces += built.faces;
        inserted += 1;
        if let Some(s) = stream.as_mut().filter(|_| first) {
            s.ready.insert(idx);
            s.stats.spawned += 1;
            s.stats.spawned_frame += 1;
        } else if first {
            stats.buildings += 1;
            ready.0 += 1;
            let kits = city.kinds.iter().filter(|k| k.kit.is_some()).count();
            if stats.buildings == kits {
                info!("kit: {kits} buildings ready at {:.2} s (wall since start), {} faces, {} meshes", real.elapsed_secs(), stats.faces, stats.meshes);
            }
        }
        commands.trigger(KitReady { entity: root });
    }
    if let Some(s) = stream.as_mut() {
        s.stats.main_ms += t0.elapsed().as_secs_f32() * 1000.0;
    }
}

/// A kit building's lights (kept on the root; spawned while it is near the focus when a `KitRing` is set).
#[derive(Component, Clone)]
pub struct KitLights(Arc<Vec<KitLight>>);

/// The node of a kit building's light (despawned with its twin when the building leaves the light radius).
#[derive(Component)]
pub struct KitLightNode;

fn spawn_lights(commands: &mut Commands, root: Entity, idx: usize, lights: &[KitLight]) {
    for l in lights.iter().cloned() {
            let up = if l.dir.normalize_or_zero().y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
            let node = commands
                .spawn((Transform::from_translation(l.at).looking_to(l.dir, up), Visibility::default(), KitLightNode, ChildOf(root)))
                .id();
            let intensity = l.energy_w * LM_PER_W;
            let gl = GlLight {
                bldg: idx,
                name: l.name,
                role: l.role,
                export_shadow: l.shadow,
                floor: l.floor,
                interior_floor: l.interior_floor,
                energy_w: l.energy_w,
                color: l.color,
                intensity,
                range: l.range,
                radius: l.radius,
                spot: l.spot,
                priority: l.priority,
                down: l.down,
                cone: l.cone,
            };
            let mut e = commands.spawn((Transform::IDENTITY, Visibility::default(), ChildOf(node)));
            match l.spot {
                Some((inner, outer)) => e.insert((
                    SpotLight {
                        color: l.color,
                        intensity,
                        range: l.range,
                        radius: l.radius,
                        inner_angle: inner,
                        outer_angle: outer,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    gl,
                )),
                None => e.insert((
                    PointLight { color: l.color, intensity, range: l.range, radius: l.radius, shadow_maps_enabled: false, ..default() },
                    gl,
                )),
            };
    }
}

/// GL_DUMP_LIGHTS=FILE: once every building is ready (+GL_DUMP_LIGHTS_FRAME frames, default 30), every managed light's world state as JSON
/// lines, sorted by name (the glb vs kit comparison: the same light set must come out the same).
#[allow(clippy::type_complexity)]
fn dump_lights(
    city: Res<City>,
    ready: Res<ReadyBuildings>,
    stream: Option<Res<KitStream>>,
    q: Query<(&GlLight, &GlobalTransform, Option<&PointLight>, Option<&SpotLight>, &Visibility, Option<&crate::lighting::Twin>), Without<crate::lighting::MovingLight>>,
    twins: Query<(&GlobalTransform, &SpotLight, &Visibility)>,
    mut frames: Local<u32>,
    mut done: Local<bool>,
) {
    let Ok(path) = std::env::var("GL_DUMP_LIGHTS") else { return };
    if *done || !buildings_ready(&city, &ready, stream.as_deref()) {
        return;
    }
    *frames += 1;
    if *frames < std::env::var("GL_DUMP_LIGHTS_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(30) {
        return;
    }
    *done = true;
    let mut rows: Vec<String> = q
        .iter()
        .map(|(l, gt, p, s, vis, twin)| {
            let (kind, inten, range, radius, col, shadow, ang) = match (p, s) {
                (Some(p), _) => ("point", p.intensity, p.range, p.radius, p.color, p.shadow_maps_enabled, (0.0, 0.0)),
                (_, Some(s)) => ("spot", s.intensity, s.range, s.radius, s.color, s.shadow_maps_enabled, (s.inner_angle, s.outer_angle)),
                _ => ("none", 0.0, 0.0, 0.0, Color::BLACK, false, (0.0, 0.0)),
            };
            let c = col.to_linear();
            let t = gt.translation();
            let f = gt.forward();
            let tw = twin.and_then(|t| twins.get(t.0).ok()).map_or(String::from("null"), |(tg, ts, tv)| {
                let tf = tg.forward();
                let tt = tg.translation();
                format!("{{\"at\":[{:.3},{:.3},{:.3}],\"fwd\":[{:.3},{:.3},{:.3}],\"range\":{:.2},\"shadow\":{},\"vis\":\"{:?}\"}}", tt.x, tt.y, tt.z, tf.x, tf.y, tf.z, ts.range, ts.shadow_maps_enabled, tv)
            });
            format!(
                "{{\"name\":\"{}\",\"kind\":\"{kind}\",\"at\":[{:.3},{:.3},{:.3}],\"fwd\":[{:.3},{:.3},{:.3}],\"intensity\":{:.1},\"range\":{:.2},\"radius\":{:.3},\"color\":[{:.4},{:.4},{:.4}],\"shadow\":{shadow},\"angles\":[{:.4},{:.4}],\"vis\":\"{:?}\",\"role\":\"{:?}\",\"twin\":{tw}}}",
                l.name, t.x, t.y, t.z, f.x, f.y, f.z, inten, range, radius, c.red, c.green, c.blue, ang.0, ang.1, vis, l.role
            )
        })
        .collect();
    rows.sort();
    std::fs::write(&path, rows.join("
")).ok();
    info!("dumped {} lights to {path}", rows.len());
}

/// The kit systems + resources (added by `GlClientPlugin`).
pub fn plugin(app: &mut App) {
    app.init_resource::<KitTasks>()
        .init_resource::<RingFocus>()
        .init_resource::<KitBlocks>()
        .init_resource::<KitStats>()
        // before the cutaway: nodes spawned this frame get their cut visibility this frame (no flash of the Cut stubs)
        .add_systems(Update, (kit_stream, kit_ring.after(kit_stream), poll_kits.after(kit_ring).before(crate::city::apply_cutaway), dump_lights));
}

/// Finished stamps spawned per frame at most while streaming (mesh uploads + entity spawns on the main thread).
const INSERT_MAX: usize = 12;

/// How far (m) the focus must be inside a new chunk before the interior ring follows it.
const RING_MARGIN: f32 = 16.0;

/// The ring's focus chunk, shared by kit_ring and kit_stream (/code-review: a streamed building's interiors were
/// decided from the raw focus chunk, kit_ring's sticky one then re-stamped it near a chunk edge).
#[derive(Resource, Default)]
pub struct RingFocus(pub Option<(i32, i32)>);

/// The focus chunk is sticky: it changes only once the focus is RING_MARGIN m inside the new chunk, so a walk along
/// a chunk edge does not re-stamp a whole row of buildings back and forth (each re-stamp re-reads the stream).
fn ring_focus(ring: &KitRing, at: Vec3, sticky: &mut RingFocus) -> (i32, i32) {
    let fp = crate::frame::from_bevy(at);
    let raw = ((fp.x / ring.chunk_m).floor() as i32, (fp.y / ring.chunk_m).floor() as i32);
    let inside = |c: (i32, i32)| {
        let (x0, y0) = (c.0 as f32 * ring.chunk_m, c.1 as f32 * ring.chunk_m);
        (fp.x - x0).min(x0 + ring.chunk_m - fp.x).min(fp.y - y0).min(y0 + ring.chunk_m - fp.y)
    };
    let f = match sticky.0 {
        Some(old) if old != raw && inside(raw) < RING_MARGIN && (old.0 - raw.0).abs() <= 1 && (old.1 - raw.1).abs() <= 1 => old,
        _ => raw,
    };
    sticky.0 = Some(f);
    f
}

/// Lazy interiors: every 15 frames, buildings whose wanted detail (inside the focus's chunk ring or not) differs
/// from what they carry get a re-stamp (one in flight per building).
#[allow(clippy::too_many_arguments)]
fn kit_ring(
    ring: Option<Res<KitRing>>,
    focus: Res<crate::frame::Focus>,
    city: Res<City>,
    blocks: Res<KitBlocks>,
    mut tasks: ResMut<KitTasks>,
    mut q: Query<(Entity, &Bldg, &GlobalTransform, &mut KitState, Option<&KitLights>, Option<&Children>)>,
    light_nodes: Query<(), With<KitLightNode>>,
    mut commands: Commands,
    mut frame: Local<u32>,
    mut sticky: ResMut<RingFocus>,
) {
    let Some(ring) = ring else { return };
    *frame = frame.wrapping_add(1);
    if *frame % 15 != 0 {
        return;
    }
    let chunk = |p: Vec3| {
        let c = crate::frame::from_bevy(p);
        ((c.x / ring.chunk_m).floor() as i32, (c.y / ring.chunk_m).floor() as i32)
    };
    let f = ring_focus(&ring, focus.at, &mut sticky);
    let no_lights = std::env::var("GL_KIT_NO_LIGHTS").is_ok();
    for (e, b, gt, mut st, lights, kids) in q.iter_mut() {
        if !st.spawned {
            continue;
        }
        let d = (gt.translation() - focus.at).with_y(0.0).length();
        let want_l = !no_lights && d < ring.light_m + if st.lights { 16.0 } else { 0.0 };
        if want_l != st.lights {
            if want_l {
                if let Some(ls) = lights {
                    spawn_lights(&mut commands, e, b.idx, &ls.0);
                }
            } else {
                for &c in kids.map(|k| &k[..]).unwrap_or(&[]) {
                    if light_nodes.contains(c) {
                        commands.entity(c).despawn();
                    }
                }
            }
            st.lights = want_l;
        }
        if st.pending || st.failed {
            continue;
        }
        let c = chunk(gt.translation());
        let want = (c.0 - f.0).abs() <= ring.ring && (c.1 - f.1).abs() <= ring.ring;
        if want == st.interiors {
            continue;
        }
        let Some(kr) = city.kinds[city.bldgs[b.idx].kind].kit.as_ref() else { continue };
        let Some(blk) = blocks.0.get(&kr.block) else { continue };
        st.pending = true;
        tasks.0.push((e, b.idx, want, restamp(blk.info.clone(), city.root.join(&kr.kit), want)));
    }
}

/// Spawns a kit building's root and queues its stamping (setup_city).
#[allow(clippy::too_many_arguments)]
pub fn spawn_building(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    blocks: &mut KitBlocks,
    tasks: &mut KitTasks,
    root_dir: &Path,
    idx: usize,
    transform: Transform,
    name: String,
    k: &KitRef,
) -> Entity {
    let root = commands
        .spawn((transform, Visibility::default(), Bldg { idx }, Name::new(name), KitState { interiors: k.interiors, pending: true, spawned: false, lights: false, failed: false }))
        .id();
    let t = start(asset_server, materials, blocks, root_dir, k);
    tasks.0.push((root, idx, k.interiors, t));
    root
}

/// Kit streaming (`KitStream`): unloads chunks beyond `keep`, then starts the wanted chunks nearest the focus
/// first while the stamping tasks in flight stay under `max_tasks` (a chunk starts whole).
#[allow(clippy::too_many_arguments)]
fn kit_stream(
    ks: Option<ResMut<KitStream>>,
    ring: Option<Res<KitRing>>,
    focus: Res<crate::frame::Focus>,
    city: Res<City>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blocks: ResMut<KitBlocks>,
    mut tasks: ResMut<KitTasks>,
    mut sticky: ResMut<RingFocus>,
    mut commands: Commands,
) {
    let Some(mut ks) = ks else { return };
    let t0 = std::time::Instant::now();
    let ks = &mut *ks;
    let fp = crate::frame::from_bevy(focus.at);
    let cm = ks.chunk_m;
    let f = ((fp.x / cm).floor() as i32, (fp.y / cm).floor() as i32);
    let cheb = |c: (i32, i32)| (c.0 - f.0).abs().max((c.1 - f.1).abs());
    let gone: Vec<(i32, i32)> = ks.loaded.keys().filter(|c| cheb(**c) > ks.keep).copied().collect();
    let mut dropped = std::collections::HashSet::new();
    for c in gone {
        for e in ks.loaded.remove(&c).unwrap_or_default() {
            commands.entity(e).despawn();
            dropped.insert(e);
        }
        for i in &ks.by_chunk[&c] {
            ks.ready.remove(i);
        }
        ks.stats.unloads += 1;
    }
    if !dropped.is_empty() {
        // a despawned root's stamp is dropped (dropping a Task cancels it)
        tasks.0.retain(|t| !dropped.contains(&t.0));
    }
    // interiors as kit_ring decides them: its sticky focus chunk, its chunk size
    let rf = ring.as_ref().map(|r| (**r, ring_focus(r, focus.at, &mut sticky)));
    let mut want: Vec<(i32, (i32, i32))> = ks.by_chunk.keys().filter(|c| cheb(**c) <= ks.radius).map(|c| (cheb(*c), *c)).collect();
    want.sort();
    ks.want = want.iter().map(|w| w.1).collect();
    for (_, c) in want {
        if ks.loaded.contains_key(&c) {
            continue;
        }
        if tasks.0.len() >= ks.max_tasks {
            break;
        }
        let mut roots = vec![];
        for &idx in &ks.by_chunk[&c] {
            let p = &city.bldgs[idx];
            let kind = &city.kinds[p.kind];
            let Some(kr) = kind.kit.as_ref() else { continue };
            let mut kr = kr.clone();
            // interiors by the building's own chunk, as kit_ring decides them (it re-stamps as the focus moves)
            if let Some((r, rf)) = rf {
                let b = crate::frame::from_bevy(p.transform.translation);
                let bc = ((b.x / r.chunk_m).floor() as i32, (b.y / r.chunk_m).floor() as i32);
                kr.interiors = (bc.0 - rf.0).abs() <= r.ring && (bc.1 - rf.1).abs() <= r.ring;
            }
            roots.push(spawn_building(
                &mut commands,
                &asset_server,
                &mut materials,
                &mut blocks,
                &mut tasks,
                &city.root,
                idx,
                p.transform,
                format!("bldg{idx}_{}", kind.id),
                &kr,
            ));
        }
        ks.loaded.insert(c, roots);
        ks.stats.loads += 1;
    }
    ks.stats.chunks = ks.loaded.len();
    ks.stats.buildings = ks.loaded.values().map(Vec::len).sum();
    ks.stats.main_ms = t0.elapsed().as_secs_f32() * 1000.0;
}
