//! The view milestone V4: a storey's z from TheCityGraphics' export, not from a formula. The sim export
//! (`out/sim/lots_sim.json`, schema city-sim/0.2, SIM_NOTES "lots_sim.json") gives every lot its platform z (`z_m`)
//! and per storey its z over it (`levels[i].z`, the kit's own 4 m + 3 m tenant stack, D6's 4.5 m, D2's 6 m ground,
//! the 3.5 m estimates); `out/lots.json` gives each lot's rect in city metres.
//!
//! A sim building (a cut of a block: one lot or several, its `rect` in tiles) is matched to the lots its rect
//! overlaps; a body on storey `f` stands on the overlapping lot that has that storey (the most overlap first), at
//! `z_m + levels[f].z`, inside that lot's part of the rect. Before V4 the view stood it at the ground under its drawn
//! point plus 4 m + 3 m a storey: at Home#824 the point fell on the next lot's platform (13 m higher) and the body
//! stood over that lot's roof (`office_worker_42443`, V3's z 234 where the export says 221).
//!
//! V10: a storey stands on the lot's massing box that holds it (the chunks' `boxes`, the kit building's own frame:
//! the podium, then the tower's plate, P9's square at the podium's back corner), not on the whole lot. Before V10 a
//! body on a tower storey stood anywhere in the middle 60 % of the lot (20 x 24 m) while the plate is 14 x 14 m at its
//! back corner: Nerys Underhill (#24590, floor 27 of sb00_05_r0c10_p1_l1) stood on the plate's east wall line, and
//! the kit's own cutaway (glclient::occlusion::own_cut) looked her floor up from a box she was not in.
//!
//! Missing files (an old asset tree) leave the table empty: the caller falls back to the formula.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize)]
struct LotsFile {
    lots: Vec<LotRec>,
}

#[derive(Deserialize)]
struct LotRec {
    id: String,
    rect_m: [f32; 4],
}

#[derive(Deserialize)]
struct SimFile {
    lots: HashMap<String, SimLot>,
}

#[derive(Deserialize)]
struct SimLot {
    #[serde(default)]
    z_m: f32,
    #[serde(default)]
    levels: Vec<SimLevel>,
}

#[derive(Deserialize)]
struct SimLevel {
    z: f32,
    h: f32,
}

pub struct Lot {
    pub id: String,
    /// x0, y0, x1, y1 (city metres)
    pub rect: [f32; 4],
    pub z_m: f32,
    /// Each storey's floor over `z_m`, and the roof (the last storey's top) at the end
    pub zs: Vec<f32>,
    /// V10: the lot's massing boxes (x0, y0, x1, y1, z0, z1; city metres, absolute z) from the chunks: the
    /// storeys' footprints (a tower's plate over its podium); empty (an old asset tree): the lot's rect
    pub boxes: Vec<[f32; 6]>,
}

impl Lot {
    pub fn floors(&self) -> usize {
        self.zs.len().saturating_sub(1)
    }

    /// The absolute z of storey `f` (a climb's fraction lerped between two storeys; past the top: the roof).
    pub fn z_at(&self, f: f32) -> f32 {
        let f = f.max(0.0);
        let i = (f.floor() as usize).min(self.zs.len() - 1);
        let j = (i + 1).min(self.zs.len() - 1);
        self.z_m + self.zs[i] + (self.zs[j] - self.zs[i]) * (f - i as f32).min(1.0)
    }

    pub fn roof(&self) -> f32 {
        self.z_m + self.zs.last().copied().unwrap_or(0.0)
    }
}

/// Where a body on a storey of a building stands: the lot, its z, and the lot's part of the building's rect.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub lot: usize,
    pub z: f32,
    /// x0, y0, x1, y1 (metres): the building's rect cut to the lot's
    pub area: [f32; 4],
    /// The storey is past the lot's top (the sim's floor count disagrees with the export's)
    pub over: bool,
    /// V10: the lift and the stairs (x, y metres): the middle of the top storey's footprint (a tower's plate), so the
    /// core stands in every storey the climb passes (the podium holds the plate) and does not jump at the setback
    pub core: [f32; 2],
}

#[derive(Default)]
pub struct Lots {
    pub lots: Vec<Lot>,
    /// 64 m cells -> lots overlapping them
    grid: HashMap<(i32, i32), Vec<u32>>,
}

const CELL: f32 = 64.0;

impl Lots {
    /// `chunk_dir` is `out/chunks`: the files are `out/lots.json` and `out/sim/lots_sim.json`.
    pub fn load(chunk_dir: &Path) -> Lots {
        let Some(out) = chunk_dir.parent() else { return Lots::default() };
        let read =
            |p: &Path| std::fs::read_to_string(p).map_err(|e| warn!("lots: {}: {e} (storeys by the 4 m + 3 m formula)", p.display())).ok();
        let (Some(a), Some(b)) = (read(&out.join("lots.json")), read(&out.join("sim").join("lots_sim.json"))) else {
            return Lots::default();
        };
        let lf: LotsFile = match serde_json::from_str(&a) {
            Ok(v) => v,
            Err(e) => {
                warn!("lots: lots.json: {e}");
                return Lots::default();
            }
        };
        let sf: SimFile = match serde_json::from_str(&b) {
            Ok(v) => v,
            Err(e) => {
                warn!("lots: lots_sim.json: {e}");
                return Lots::default();
            }
        };
        let boxes = lot_boxes(chunk_dir);
        let mut out = Lots::default();
        for l in lf.lots {
            let Some(s) = sf.lots.get(&l.id) else { continue };
            if s.levels.is_empty() {
                continue;
            }
            let mut zs: Vec<f32> = s.levels.iter().map(|v| v.z).collect();
            let top = s.levels.last().unwrap();
            zs.push(top.z + top.h);
            let i = out.lots.len() as u32;
            let [x0, y0, x1, y1] = l.rect_m;
            for cx in (x0 / CELL).floor() as i32..=(x1 / CELL).floor() as i32 {
                for cy in (y0 / CELL).floor() as i32..=(y1 / CELL).floor() as i32 {
                    out.grid.entry((cx, cy)).or_default().push(i);
                }
            }
            let boxes = boxes.get(&l.id).cloned().unwrap_or_default();
            out.lots.push(Lot { id: l.id, rect: l.rect_m, z_m: s.z_m, zs, boxes });
        }
        info!("lots: {} lots with storeys (lots_sim.json)", out.lots.len());
        out
    }

    /// The lots `r` (x0, y0, x1, y1 metres) overlaps, the most overlap first: (lot, overlap m^2, the overlap).
    pub fn under(&self, r: [f32; 4]) -> Vec<(usize, f32, [f32; 4])> {
        let mut seen: Vec<u32> = vec![];
        for cx in (r[0] / CELL).floor() as i32..=(r[2] / CELL).floor() as i32 {
            for cy in (r[1] / CELL).floor() as i32..=(r[3] / CELL).floor() as i32 {
                if let Some(v) = self.grid.get(&(cx, cy)) {
                    seen.extend(v);
                }
            }
        }
        seen.sort_unstable();
        seen.dedup();
        let mut out: Vec<(usize, f32, [f32; 4])> = seen
            .into_iter()
            .filter_map(|i| {
                let l = &self.lots[i as usize].rect;
                let o = [r[0].max(l[0]), r[1].max(l[1]), r[2].min(l[2]), r[3].min(l[3])];
                let a = (o[2] - o[0]) * (o[3] - o[1]);
                (o[2] > o[0] && o[3] > o[1]).then_some((i as usize, a, o))
            })
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    /// A body on storey `f` of the building whose rect (metres) is `r`: the overlapping lot that has the storey
    /// (the most overlap first), else the tallest overlapping lot (`over`); V10: on the part of it that storey's
    /// massing box covers (`slab`: `[view] storey_slab_m`).
    pub fn place(&self, r: [f32; 4], f: f32, slab: f32) -> Option<Placed> {
        let under = self.under(r);
        let need = f.round().max(0.0) as usize;
        let pick = under
            .iter()
            .find(|(i, _, _)| self.lots[*i].floors() > need)
            .or_else(|| under.iter().max_by(|a, b| self.lots[a.0].floors().cmp(&self.lots[b.0].floors()).then(b.1.total_cmp(&a.1))))?;
        let lot = &self.lots[pick.0];
        let z = lot.z_at(f);
        let top = lot.footprint(pick.2, lot.z_at(lot.floors().saturating_sub(1) as f32), slab);
        let core = [(top[0] + top[2]) / 2.0, (top[1] + top[3]) / 2.0];
        Some(Placed { lot: pick.0, z, area: lot.footprint(pick.2, z, slab), over: lot.floors() <= need, core })
    }
}

impl Lot {
    /// V10: the part of `area` (metres) a storey whose floor is at `z` stands on: the lot's massing box holding that
    /// floor (its slab, `slab` m, inside the box's height) cut to `area`, else the box itself (the sim's rect misses
    /// the plate); no box holds it (an old asset tree, the roof): `area`.
    fn footprint(&self, area: [f32; 4], z: f32, slab: f32) -> [f32; 4] {
        let Some(b) = self.boxes.iter().filter(|b| z + slab >= b[4] && z + slab < b[5]).max_by(|a, b| {
            let o = |r: &[f32; 6]| (r[2].min(area[2]) - r[0].max(area[0])).max(0.0) * (r[3].min(area[3]) - r[1].max(area[1])).max(0.0);
            o(a).total_cmp(&o(b))
        }) else {
            return area;
        };
        let o = [b[0].max(area[0]), b[1].max(area[1]), b[2].min(area[2]), b[3].min(area[3])];
        if o[2] > o[0] && o[3] > o[1] { o } else { [b[0], b[1], b[2], b[3]] }
    }
}

#[derive(Deserialize)]
struct ChunkBoxes {
    buildings: Vec<ChunkBldg>,
    /// x0, y0, x1, y1, z0, z1, building index, part
    boxes: Vec<(f32, f32, f32, f32, f32, f32, usize, String)>,
}

#[derive(Deserialize)]
struct ChunkBldg {
    #[serde(default)]
    lot: String,
}

/// V10: every chunk's massing boxes by lot (the roof's plant rooms left out: no storey stands in them).
fn lot_boxes(chunk_dir: &Path) -> HashMap<String, Vec<[f32; 6]>> {
    let mut out: HashMap<String, Vec<[f32; 6]>> = HashMap::new();
    let Ok(dir) = std::fs::read_dir(chunk_dir) else { return out };
    for e in dir.filter_map(|e| e.ok()) {
        let p = e.path();
        let is_chunk = p.extension().is_some_and(|x| x == "json")
            && p.file_stem()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.split_once('_').is_some_and(|(a, b)| a.parse::<i32>().is_ok() && b.parse::<i32>().is_ok()));
        if !is_chunk {
            continue;
        }
        let Some(m) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<ChunkBoxes>(&t).ok()) else { continue };
        for (x0, y0, x1, y1, z0, z1, b, part) in m.boxes {
            if part == "plant" {
                continue;
            }
            if let Some(bd) = m.buildings.get(b).filter(|bd| !bd.lot.is_empty()) {
                out.entry(bd.lot.clone()).or_default().push([x0, y0, x1, y1, z0, z1]);
            }
        }
    }
    out
}
