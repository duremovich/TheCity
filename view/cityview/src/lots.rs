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
            out.lots.push(Lot { id: l.id, rect: l.rect_m, z_m: s.z_m, zs });
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
    /// (the most overlap first), else the tallest overlapping lot (`over`).
    pub fn place(&self, r: [f32; 4], f: f32) -> Option<Placed> {
        let under = self.under(r);
        let need = f.round().max(0.0) as usize;
        let pick = under
            .iter()
            .find(|(i, _, _)| self.lots[*i].floors() > need)
            .or_else(|| under.iter().max_by(|a, b| self.lots[a.0].floors().cmp(&self.lots[b.0].floors()).then(b.1.total_cmp(&a.1))))?;
        let lot = &self.lots[pick.0];
        Some(Placed { lot: pick.0, z: lot.z_at(f), area: pick.2, over: lot.floors() <= need })
    }
}
