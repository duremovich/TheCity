//! The tram cars (P7 F, spec/city/P7_PLAN.md): `pipeline/city/street/tram.py` writes `out/street/tram.json`, per
//! line its loop (points every 2 m: x, y, z of the rail, cumulative u, the wire's height over the rail), stops and
//! timetable ([t, u] over one period). The client runs `cars` cars `period / cars` s apart on it: a car's u at
//! time t is the timetable's u at t + k * period / cars, its position / heading the loop's at u. Cars are HK
//! double-deckers as boxes (lit windows on both decks, a destination board at each end: the car reverses at
//! the termini without turning), a pantograph up to the wire. Shown only within the street ring of the focus
//! (the chunks' rails and wire are there); no light entities (the windows are emissive). P8: the bodies are the
//! fleet's `tram` variants (vehicles.rs) when it has them, the boxes below otherwise; the pantograph stays here.

use bevy::{camera::primitives::Aabb, prelude::*};
use serde::Deserialize;

use crate::block::hex;
use crate::frame::to_bevy;
use crate::street::{Acc, Street};
use crate::vehicles::Fleet;

#[derive(Deserialize, Clone, Debug)]
pub struct CarDims {
    pub len: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LineRec {
    pub way: String,
    pub pts: Vec<[f32; 3]>,
    pub u: Vec<f32>,
    #[serde(default)]
    pub wire: Vec<f32>,
    pub tt: Vec<[f32; 2]>,
    pub period: f32,
    pub cars: usize,
    pub car: CarDims,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct TramFile {
    #[serde(default)]
    lines: Vec<LineRec>,
}

impl LineRec {
    /// u at time t (the timetable wraps over the period)
    pub fn u_at(&self, t: f32) -> f32 {
        let t = t.rem_euclid(self.period);
        let k = self.tt.partition_point(|p| p[0] <= t).clamp(1, self.tt.len() - 1);
        let (a, b) = (self.tt[k - 1], self.tt[k]);
        if b[0] <= a[0] { a[1] } else { a[1] + (t - a[0]) / (b[0] - a[0]) * (b[1] - a[1]) }
    }

    /// (position on the rail, heading from +x, the wire's height over the rail) at u
    pub fn at(&self, u: f32) -> (Vec3, f32, f32) {
        let l = *self.u.last().unwrap();
        let u = u.rem_euclid(l);
        let mut k = self.u.partition_point(|x| *x <= u).clamp(1, self.u.len() - 1);
        // a zero-length piece (the loop's closing point) has no heading: use the next real one
        while k + 1 < self.u.len() && self.u[k] - self.u[k - 1] < 1e-4 {
            k += 1;
        }
        let (a, b) = (Vec3::from_array(self.pts[k - 1]), Vec3::from_array(self.pts[k]));
        let span = (self.u[k] - self.u[k - 1]).max(1e-4);
        let f = ((u - self.u[k - 1]) / span).clamp(0.0, 1.0);
        let w = match (self.wire.get(k - 1), self.wire.get(k)) {
            (Some(x), Some(y)) => x + f * (y - x),
            _ => 5.9,
        };
        (a.lerp(b, f), (b.y - a.y).atan2(b.x - a.x), w)
    }
}

/// The liveries (HK trams are mostly green, many wrapped in a single colour of advert)
const LIVERIES: [&str; 4] = ["#1E6B3A", "#1E6B3A", "#8C1D1D", "#1F3F7A"];
/// Window / board emissive scales: low (AgX turns a brighter emissive white; s30 checkpoint shots)
const WIN_EMIT: f32 = 1.1;
const BOARD_EMIT: f32 = 4.0;
/// The pantograph's base on the roof (along the car, from its centre)
const PANTO_AT: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Part {
    Body,
    Cream,
    Under,
    Roof,
    Window,
    Board,
    Head,
}

/// The car's boxes in its own frame (city axes: x along, y across, z up from the rail), per part.
fn car_parts(d: &CarDims) -> Vec<(Part, Vec3, Vec3)> {
    let (hl, hw, h) = (d.len / 2.0, d.w / 2.0, d.h);
    let b = |x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32| (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1));
    let mut out = vec![];
    let mut put = |p: Part, (a, c): (Vec3, Vec3)| out.push((p, a, c));
    put(Part::Under, b(-hl + 1.0, -hw + 0.15, 0.15, hl - 1.0, hw - 0.15, 0.6));
    put(Part::Body, b(-hl, -hw, 0.6, hl, hw, 2.4));
    put(Part::Cream, b(-hl - 0.01, -hw - 0.01, 2.4, hl + 0.01, hw + 0.01, 2.6));
    put(Part::Body, b(-hl, -hw, 2.6, hl, hw, h - 0.3));
    put(Part::Roof, b(-hl + 0.2, -hw + 0.1, h - 0.3, hl - 0.2, hw - 0.1, h));
    // windows: panes proud of both sides (the lower deck's leave the doors' bays dark), the end windows
    let panes = |n: usize, z0: f32, z1: f32, skip: &[usize]| -> Vec<(f32, f32, f32, f32)> {
        let pitch = (d.len - 1.6) / n as f32;
        (0..n)
            .filter(|i| !skip.contains(i))
            .map(|i| {
                let x0 = -hl + 0.8 + i as f32 * pitch + 0.1;
                (x0, x0 + pitch - 0.2, z0, z1)
            })
            .collect()
    };
    for (x0, x1, z0, z1) in panes(6, 1.2, 2.15, &[1, 4]).into_iter().chain(panes(7, 3.0, 3.85, &[])) {
        put(Part::Window, b(x0, -hw - 0.02, z0, x1, -hw, z1));
        put(Part::Window, b(x0, hw, z0, x1, hw + 0.02, z1));
    }
    for s in [-1.0f32, 1.0] {
        let e = s * hl;
        let (o0, o1) = if s > 0.0 { (e, e + 0.02) } else { (e - 0.02, e) };
        put(Part::Window, b(o0, -hw + 0.25, 1.3, o1, hw - 0.25, 2.2));
        put(Part::Window, b(o0, -hw + 0.25, 3.0, o1, hw - 0.25, 3.7));
        // the destination board over the upper deck's end window, the headlight under the lower one
        put(Part::Board, b(o0 - 0.01 * s, -0.7, 3.8, o1 + 0.01 * s, 0.7, 4.15));
        put(Part::Head, b(o0 - 0.01 * s, -0.2, 0.8, o1 + 0.01 * s, 0.2, 1.0));
    }
    out
}

/// A part's mesh: city-frame boxes emitted in Bevy coordinates (the car's root is rotated about Bevy +Y).
fn part_mesh(boxes: &[(Vec3, Vec3)]) -> Option<(Mesh, Aabb)> {
    let mut acc = Acc::default();
    for (a, b) in boxes {
        acc.cuboid(*a, *b);
    }
    acc.mesh()
}

#[derive(Component)]
pub struct TramCar {
    pub line: usize,
    pub k: usize,
}

/// The pantograph's strut (scaled to the wire) and head bar (at the wire).
#[derive(Component)]
pub struct Panto {
    bar: bool,
    /// The roof height it stands on (the box car's h or the fleet model's)
    base: f32,
}

#[derive(Resource, Default)]
pub struct Trams {
    pub lines: Vec<LineRec>,
    loaded: bool,
    /// Cars shown (in the ring) last frame, for the hud / benches
    pub shown: usize,
}

fn setup(
    mut commands: Commands,
    driven: Option<Res<crate::SimDrives>>,
    street: Option<Res<Street>>,
    fleet: Option<Res<Fleet>>,
    mut trams: ResMut<Trams>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(st), Some(fleet)) = (street, fleet) else { return };
    if trams.loaded || !fleet.ready {
        return;
    }
    trams.loaded = true;
    if std::env::var("CV_NO_TRAM").is_ok() || driven.is_some() {
        return;
    }
    let path = st.dir.join("tram.json");
    let Some(f) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<TramFile>(&t).map_err(|e| warn!("{}: {e}", path.display())).ok())
    else {
        return;
    };
    let emit = |c: &str, k: f32| LinearRgba::from(hex(c)) * k;
    let lit = |c: &str, k: f32| StandardMaterial { base_color: Color::BLACK, emissive: emit(c, k), ..default() };
    let plain = |c: Color, r: f32| StandardMaterial { base_color: c, perceptual_roughness: r, ..default() };
    let cream = mats.add(plain(hex("#E8DEC0"), 0.5));
    let under = mats.add(plain(Color::srgb(0.04, 0.04, 0.045), 0.8));
    let roof = mats.add(plain(Color::srgb(0.28, 0.29, 0.3), 0.6));
    let window = mats.add(lit("#FFC070", WIN_EMIT));
    let board = mats.add(lit("#FF9A20", BOARD_EMIT));
    let head = mats.add(lit("#FFF6E8", BOARD_EMIT * 2.0));
    let metal = mats.add(plain(Color::srgb(0.12, 0.12, 0.12), 0.5));
    let bodies: Vec<_> = LIVERIES.iter().map(|c| mats.add(plain(hex(c), 0.45))).collect();
    // a pantograph: the strut over a unit height (scaled per frame to the wire), the bar 1.6 m across
    let strut = part_mesh(&[(Vec3::new(-0.3, -0.04, 0.0), Vec3::new(0.3, 0.04, 1.0))]).unwrap();
    let bar = part_mesh(&[(Vec3::new(-0.06, -0.8, -0.04), Vec3::new(0.06, 0.8, 0.0))]).unwrap();
    let (strut, bar) = (meshes.add(strut.0), meshes.add(bar.0));
    for (li, ln) in f.lines.iter().enumerate() {
        let parts = car_parts(&ln.car);
        let mut per: Vec<(Part, Handle<Mesh>, Aabb)> = vec![];
        for p in [Part::Body, Part::Cream, Part::Under, Part::Roof, Part::Window, Part::Board, Part::Head] {
            let boxes: Vec<(Vec3, Vec3)> = parts.iter().filter(|x| x.0 == p).map(|x| (x.1, x.2)).collect();
            if let Some((m, a)) = part_mesh(&boxes) {
                per.push((p, meshes.add(m), a));
            }
        }
        for k in 0..ln.cars {
            let root = commands
                .spawn((Transform::IDENTITY, Visibility::Hidden, TramCar { line: li, k }, Name::new(format!("tram_{li}_{k}"))))
                .id();
            let mut roof_z = ln.car.h;
            if let Some(mi) = fleet.pick("tram", (li * 1000 + k) as u64) {
                let m = &fleet.models[mi];
                m.spawn_parts(&mut commands, root);
                // the roof under the boards (the model's top is its destination board's frame at most)
                roof_z = m.rec.size[2];
            } else {
                for (p, m, a) in &per {
                    let mat = match p {
                        Part::Body => bodies[k % bodies.len()].clone(),
                        Part::Cream => cream.clone(),
                        Part::Under => under.clone(),
                        Part::Roof => roof.clone(),
                        Part::Window => window.clone(),
                        Part::Board => board.clone(),
                        Part::Head => head.clone(),
                    };
                    commands.spawn((Mesh3d(m.clone()), MeshMaterial3d(mat), *a, Transform::IDENTITY, ChildOf(root)));
                }
            }
            commands.spawn((
                Mesh3d(strut.clone()),
                MeshMaterial3d(metal.clone()),
                Transform::from_translation(to_bevy(Vec3::new(PANTO_AT, 0.0, roof_z))),
                Panto { bar: false, base: roof_z },
                ChildOf(root),
            ));
            commands.spawn((
                Mesh3d(bar.clone()),
                MeshMaterial3d(metal.clone()),
                Transform::from_translation(to_bevy(Vec3::new(PANTO_AT, 0.0, roof_z + 1.0))),
                Panto { bar: true, base: roof_z },
                ChildOf(root),
            ));
        }
        info!(
            "tram {}: {} cars ({}), period {:.0} s, loop {:.0} m",
            ln.way,
            ln.cars,
            if fleet.of_kind("tram").is_empty() { "boxes" } else { "fleet models" },
            ln.period,
            ln.u.last().unwrap_or(&0.0)
        );
    }
    trams.lines = f.lines;
}

fn run_cars(
    time: Res<Time>,
    street: Option<Res<Street>>,
    mut trams: ResMut<Trams>,
    mut cars: Query<(&TramCar, &mut Transform, &mut Visibility, &Children), Without<Panto>>,
    mut pantos: Query<(&Panto, &mut Transform), Without<TramCar>>,
) {
    let Some(st) = street else { return };
    if trams.lines.is_empty() {
        return;
    }
    let t = time.elapsed_secs();
    let reach = (st.ring as f32 + 0.5) * st.chunk_m;
    let mut shown = 0;
    for (c, mut tf, mut vis, children) in cars.iter_mut() {
        let ln = &trams.lines[c.line];
        let u = ln.u_at(t + c.k as f32 * ln.period / ln.cars as f32);
        let (p, h, wire) = ln.at(u);
        let near = (p.truncate() - st.focus).abs().max_element() < reach;
        vis.set_if_neq(if near { Visibility::Inherited } else { Visibility::Hidden });
        if !near {
            continue;
        }
        shown += 1;
        tf.translation = to_bevy(p);
        tf.rotation = Quat::from_rotation_y(h);
        for ch in children.iter() {
            if let Ok((pa, mut pt)) = pantos.get_mut(ch) {
                let up = (wire - pa.base).max(0.05);
                if pa.bar {
                    pt.translation = to_bevy(Vec3::new(PANTO_AT, 0.0, pa.base + up));
                } else {
                    pt.scale = Vec3::new(1.0, up, 1.0);
                }
            }
        }
    }
    trams.shown = shown;
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Trams>().add_systems(Update, (setup, run_cars).chain().after(crate::street::update_street));
}
