//! The block file and its navmesh (pipeline/blender/building/block.py writes both; GAME_DATA.md "Blocks").
//! Everything here is in the block frame (x east, y north, z up; y = 0 the street line); `frame::to_bevy`.

use bevy::prelude::*;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

#[derive(Deserialize, Clone, Debug)]
pub struct BlockFile {
    pub block: BlockHead,
    pub lots: Vec<LotDef>,
    #[serde(default)]
    pub lamps: Vec<LampDef>,
    #[serde(default)]
    pub crossings: Vec<CrossingDef>,
    #[serde(default)]
    pub props: Vec<PropDef>,
    #[serde(default)]
    pub grade: Vec<GradeDef>,
    #[serde(default)]
    pub signs: Vec<SignDef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BlockHead {
    pub id: String,
    pub length: f32,
    pub lot_w: f32,
    pub sidewalk: f32,
    pub road: f32,
    pub kerb: f32,
    /// The far edge's low rail: thickness, height
    pub rail: [f32; 2],
    /// assets/-relative paths
    pub nav: String,
    pub sim: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LotDef {
    pub bldg: String,
    pub x: f32,
    pub round: String,
    /// assets/-relative
    pub glb: String,
    pub lights: String,
    /// Floors incl. the roof (the cut steps through 0..floors-1)
    pub floors: i32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LampDef {
    pub id: String,
    pub side: String,
    pub pole: [f32; 2],
    pub pole_size: f32,
    pub pole_h: f32,
    pub head: [f32; 3],
    pub colour: String,
    /// Blender spot watts (the client's lumens: x 683, as-if-point)
    pub watts: f32,
    pub cone_deg: f32,
    pub blend: f32,
    pub range: f32,
    pub radius: f32,
    pub shadow_priority: f32,
    pub fixture: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CrossingDef {
    pub id: String,
    pub x0: f32,
    pub x1: f32,
    pub ramp: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PropDef {
    pub id: String,
    pub kind: String,
    pub rect: [f32; 4],
    pub h: f32,
    /// A lit face toward the road
    pub emissive: Option<String>,
    /// A shadowless glow in front of it, Blender watts (x 683 lm)
    pub glow_w: Option<f32>,
}

/// A street sign panel on a lot's street face (block data): emissive panel, an area light toward the street, a
/// wash back onto the wall.
#[derive(Deserialize, Clone, Debug)]
pub struct SignDef {
    pub id: String,
    pub x: [f32; 2],
    pub z: [f32; 2],
    /// The panel's face plane (y < 0: just off the street face)
    pub y: f32,
    pub colour: String,
    /// Blender emission strength
    pub strength: f32,
    pub light_w: f32,
    pub wash_w: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct GradeDef {
    pub id: String,
    pub colour: String,
    pub watts: f32,
    pub size: f32,
    pub at: [f32; 3],
    pub aim: [f32; 3],
}

impl BlockFile {
    pub fn load(path: &Path) -> BlockFile {
        let txt = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        toml::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }
}

/// "#RRGGBB" -> sRGB colour.
pub fn hex(s: &str) -> Color {
    Srgba::hex(s).map(Color::from).unwrap_or(Color::WHITE)
}

/// One convex nav poly (block frame).
#[derive(Clone)]
pub struct NavPoly {
    /// Index into the block's lots (None = the street)
    pub lot: Option<usize>,
    /// The building's floor index (len(floors) = its roof); 0 on the street
    pub level: i32,
    pub kind: String,
    pub room: String,
    pub verts: Vec<Vec3>,
    pub centroid: Vec3,
    /// Neighbour, shared edge midpoint (x, y)
    pub nbrs: Vec<(usize, Vec2)>,
}

/// The block's navmesh: every building's polys + the street's, joined (nav.json).
#[derive(Clone)]
pub struct BlockNav {
    pub polys: Vec<NavPoly>,
    /// Sidewalk ends (where the street goes on): poly, point
    pub spawns: Vec<(usize, Vec3)>,
}

fn v3(q: &Value) -> Vec3 {
    let q = q.as_array().unwrap();
    Vec3::new(q[0].as_f64().unwrap() as f32, q[1].as_f64().unwrap() as f32, q[2].as_f64().unwrap() as f32)
}

impl BlockNav {
    pub fn load(path: &Path) -> BlockNav {
        let txt = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let v: Value = serde_json::from_str(&txt).unwrap();
        let lots: Vec<String> = v["lots"].as_array().unwrap().iter().map(|l| l["bldg"].as_str().unwrap().to_string()).collect();
        let mut polys: Vec<NavPoly> = v["polys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let verts: Vec<Vec3> = p["verts"].as_array().unwrap().iter().map(v3).collect();
                let centroid = verts.iter().copied().sum::<Vec3>() / verts.len() as f32;
                let b = p["bldg"].as_str().unwrap_or("");
                NavPoly {
                    lot: lots.iter().position(|l| l == b),
                    level: p["level"].as_i64().unwrap_or(0) as i32,
                    kind: p["kind"].as_str().unwrap_or("").to_string(),
                    room: p["room"].as_str().unwrap_or("").to_string(),
                    verts,
                    centroid,
                    nbrs: vec![],
                }
            })
            .collect();
        // ids are the array order (block.py numbers them so); check rather than assume
        for (i, p) in v["polys"].as_array().unwrap().iter().enumerate() {
            assert_eq!(p["id"].as_u64(), Some(i as u64), "{}: poly ids must be the array order", path.display());
        }
        for e in v["edges"].as_array().unwrap() {
            let (a, b) = (e["a"].as_u64().unwrap() as usize, e["b"].as_u64().unwrap() as usize);
            let s = e["seg"].as_array().unwrap();
            let p = |k: usize| {
                let q = s[k].as_array().unwrap();
                Vec2::new(q[0].as_f64().unwrap() as f32, q[1].as_f64().unwrap() as f32)
            };
            let mid = (p(0) + p(1)) / 2.0;
            polys[a].nbrs.push((b, mid));
            polys[b].nbrs.push((a, mid));
        }
        let spawns = v["spawns"]
            .as_array()
            .map(|a| a.iter().map(|s| (s["poly"].as_u64().unwrap() as usize, v3(&s["at"]))).collect())
            .unwrap_or_default();
        BlockNav { polys, spawns }
    }

    /// Height of poly `i` at (x, y): fan triangles + barycentric, else the nearest vertex's z.
    pub fn z_at(&self, i: usize, p: Vec2) -> f32 {
        let v = &self.polys[i].verts;
        for k in 1..v.len().saturating_sub(1) {
            let (a, b, c) = (v[0], v[k], v[k + 1]);
            let (a2, b2, c2) = (a.truncate(), b.truncate(), c.truncate());
            let d = (b2 - a2).perp_dot(c2 - a2);
            if d.abs() < 1e-6 {
                continue;
            }
            let w1 = (p - a2).perp_dot(c2 - a2) / d;
            let w2 = (b2 - a2).perp_dot(p - a2) / d;
            let w0 = 1.0 - w1 - w2;
            if w0 >= -0.02 && w1 >= -0.02 && w2 >= -0.02 {
                return w0 * a.z + w1 * b.z + w2 * c.z;
            }
        }
        v.iter().min_by(|a, b| (a.truncate() - p).length().total_cmp(&(b.truncate() - p).length())).unwrap().z
    }
}
