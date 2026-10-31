//! `citysim-cli export-view` (the city milestone C2): a save in, `city-simview/0.1` frames out
//! (TheCityGraphics `spec/integration/SIM_VIEW_CONTRACT.md` § 2's file form; `citysim::view` is the snapshot, this is
//! its JSON). The bridge converter `engine/simsnap` moved in: it loads the save, sets the view to the camera ring
//! (`PlayerCommand::SetView`), pins every body in it (`view::pin_ring`, OPEN-149's stopgap) unless `--no-pin`, runs
//! `--ticks` ticks and writes a frame a tick (the last `--keep` of them). `--follow INDEX` re-centres the ring on
//! that body every tick; `--at X,Y` (city metres) holds it; neither: the 256 m cell with the most bodies on the
//! street. It makes the view tier's fixture (`view/cityview/fixtures/`, docs/TESTING.md). Run from the repo root
//! (the save reloads its name tables from `assets/`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use citysim::components::{Position, Rect, TilePos};
use citysim::view::{self, ViewAct, ViewFrame, ViewRoute};
use citysim::{save, time, EntityId, World};
use serde_json::{json, Value};

#[derive(clap::Args)]
pub struct ExportViewArgs {
    /// The save to load.
    #[arg(long, value_name = "FILE")]
    save: PathBuf,
    /// The `city-simview/0.1` JSON to write.
    #[arg(long, value_name = "FILE")]
    out: PathBuf,
    /// Ticks to run (a frame after each).
    #[arg(long, default_value_t = 60)]
    ticks: u64,
    /// Write only the last N frames (the fixture keeps one).
    #[arg(long)]
    keep: Option<usize>,
    /// The ring's centre in city metres, `X,Y` (x east, y north).
    #[arg(long, value_name = "X,Y")]
    at: Option<String>,
    /// Follow this entity index: the ring re-centred on it every tick.
    #[arg(long, value_name = "INDEX")]
    follow: Option<u32>,
    /// The ring's half side in tiles (default `[view] ring_half_tiles`).
    #[arg(long)]
    half: Option<u16>,
    /// Pin nobody (the ring's bodies keep the tier the city gives them).
    #[arg(long)]
    no_pin: bool,
}

/// Sim tiles (x east, y south) <-> city metres (x east, y north): x = (tx + 0.5) * tile_m, y = north - (ty + 0.5) *
/// tile_m (IMPORT_GRAPHICS's mapping, x0 = 0).
struct Frame {
    s: f64,
    north: f64,
}

impl Frame {
    fn tile(&self, x: f64, y: f64) -> TilePos {
        TilePos::new((x / self.s).floor().max(0.0) as u16, ((self.north - y) / self.s).floor().max(0.0) as u16)
    }

    fn metres(&self, t: TilePos) -> (f64, f64) {
        ((f64::from(t.x) + 0.5) * self.s, self.north - (f64::from(t.y) + 0.5) * self.s)
    }
}

fn id(e: EntityId) -> Value {
    json!([e.index, e.generation])
}

fn tp(t: TilePos) -> Value {
    json!([t.x, t.y])
}

fn rect(r: &Rect) -> Value {
    json!([r.x, r.y, r.w, r.h])
}

/// The 256 m cell with the most bodies out on the street (simsnap's `--auto`).
fn busiest(world: &World, f: &Frame) -> Result<TilePos, String> {
    let cell = ((256.0 / f.s) as u16).max(1);
    let mut n: std::collections::BTreeMap<(u16, u16), u32> = Default::default();
    for e in world.bodies() {
        if let Some(p) = world.comp::<Position>(e).filter(|p| p.building.is_none()) {
            *n.entry((p.tile.x / cell, p.tile.y / cell)).or_default() += 1;
        }
    }
    let ((cx, cy), c) = n
        .iter()
        .max_by_key(|(k, v)| (**v, std::cmp::Reverse(**k)))
        .map(|(k, v)| (*k, *v))
        .ok_or("no bodies on the street")?;
    let t = TilePos::new(cx * cell + cell / 2, cy * cell + cell / 2);
    let (x, y) = f.metres(t);
    eprintln!("auto: cell {cx},{cy} ({c} bodies on the street) at {x:.0},{y:.0} m");
    Ok(t)
}

/// The subject's place: its frame's, else its vehicle's, else its walk's, else its `Position`.
pub fn subject_tile(world: &World, e: EntityId) -> Option<TilePos> {
    if let Some(v) = citysim::systems::motion::vehicle_of(world, e) {
        if let Some(m) = world.motions.get(&v) {
            return Some(m.pos);
        }
    }
    citysim::exec::walk_place(world, e).map(|p| p.tile)
}

fn act_x(a: &ViewAct) -> Value {
    match a {
        ViewAct::Idle | ViewAct::Child => Value::Null,
        ViewAct::Walk { next, step_at } => json!({"next": next.map(tp), "step_at": step_at}),
        ViewAct::Timed { arrive } => json!({"arrive": arrive}),
        ViewAct::Use { until, .. } => json!({"until": until}),
        ViewAct::Wait { until } => json!({"until": until}),
        ViewAct::Fly { from, depart, arrive } => json!({"from": tp(*from), "depart": depart, "arrive": arrive}),
        ViewAct::Ride { depart, arrive } => json!({"depart": depart, "arrive": arrive}),
        ViewAct::JackedIn { since } => json!({"since": since}),
    }
}

/// One frame of the file form (`SIM_VIEW_CONTRACT.md` § 2), plus `ring` and each body's and vehicle's `ahead`.
pub fn frame_json(f: &ViewFrame) -> Value {
    let agents: Vec<Value> = f
        .agents
        .iter()
        .map(|a| {
            json!({
                "id": id(a.id),
                "t": tp(a.t),
                "b": a.b.map(id),
                "floor": a.floor,
                "lod": a.lod.map_or("Statistical".to_string(), |l| format!("{l:?}")),
                "act": a.act.word(),
                "x": act_x(&a.act),
                "goal": a.goal.map(|g| format!("{g:?}")),
                "step": a.step.map(|s| format!("{s:?}")),
                "facing": a.facing.map(|h| h.round()),
                "ahead": a.ahead.iter().map(|t| tp(*t)).collect::<Vec<_>>(),
                "aboard": a.aboard.map(id),
                "cuffed": a.cuffed.map(id),
                "sex": format!("{:?}", a.sex),
                "age": a.age,
                "dress": a.dress,
                "chrome": a.chrome,
                "colours": a.colours.map(id),
                "weapon": a.weapon,
                "deck": a.deck,
                "food": a.food,
            })
        })
        .collect();
    let vehicles: Vec<Value> = f
        .vehicles
        .iter()
        .map(|v| {
            let route = match &v.route {
                ViewRoute::Driven => json!({"type": "driven"}),
                ViewRoute::Timed { way, depart, arrive } => json!({
                    "type": "timed", "way": way.iter().map(|t| tp(*t)).collect::<Vec<_>>(), "depart": depart, "arrive": arrive
                }),
                ViewRoute::Line { line, train, at } => {
                    json!({"type": "line", "line": line, "train": train, "at": at.map(|a| format!("{a:?}"))})
                }
            };
            json!({
                "id": id(v.id),
                "ride": format!("{:?}", v.ride),
                "kind": format!("{:?}", v.kind),
                "asset": v.asset,
                "t": tp(v.t),
                "facing": v.facing.map(|h| h.round()),
                "ahead": v.ahead.iter().map(|t| tp(*t)).collect::<Vec<_>>(),
                "driver": v.driver.map(id),
                "riders": v.riders.iter().map(|r| id(*r)).collect::<Vec<_>>(),
                "route": route,
                "van_run": v.van_run.as_ref().map(|(p, arrive)| json!({
                    "prisoners": p.iter().map(|x| id(*x)).collect::<Vec<_>>(), "arrive": arrive
                })),
            })
        })
        .collect();
    json!({
        "tick": f.tick,
        "day": f.day,
        "clock": f.clock,
        "dark": f.dark,
        "season": format!("{:?}", f.season),
        "weather": Value::Null,
        "ring": rect(&f.ring),
        "agents": agents,
        "vehicles": vehicles,
        "buildings": f.buildings.iter().map(|b| json!([id(b.id), b.occupants, b.demolished, b.derelict, b.closed])).collect::<Vec<_>>(),
        "corpses": f.corpses.iter().map(|(c, t)| json!([id(*c), tp(*t)])).collect::<Vec<_>>(),
        "events": f.events.iter().map(|e| json!({
            "id": e.id, "tick": e.tick, "kind": format!("{:?}", e.kind),
            "actors": e.actors.iter().map(|a| id(*a)).collect::<Vec<_>>(), "t": e.t.map(tp), "text": e.text,
        })).collect::<Vec<_>>(),
    })
}

pub fn export_view(args: ExportViewArgs) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let mut world = save::load_from_file(&args.save)?;
    eprintln!("loaded {} at tick {} ({:.1} s)", args.save.display(), world.tick, t0.elapsed().as_secs_f64());
    if args.ticks == 0 {
        return Err("--ticks must be 1 or more (a frame is written after each tick)".into());
    }
    if args.keep.is_some_and(|k| k == 0 || k as u64 > args.ticks) {
        return Err(format!("--keep {} is outside 1..={} (--ticks)", args.keep.unwrap_or(0), args.ticks));
    }
    let cfg = world.config.view.clone();
    let f = Frame { s: cfg.tile_m, north: cfg.north_m };
    let half = args.half.unwrap_or(cfg.ring_half_tiles);
    let (w, h) = (world.map.w(), world.map.h());
    let subject = match args.follow {
        Some(ix) => {
            let e = world
                .entities()
                .find(|e| e.index == ix)
                .ok_or_else(|| format!("--follow {ix}: no living entity with that index at tick {}", world.tick))?;
            subject_tile(&world, e).ok_or_else(|| format!("--follow {ix}: entity {e:?} has no place"))?;
            Some(e)
        }
        None => None,
    };
    let fixed = match (&args.at, subject) {
        (Some(_), Some(_)) => return Err("--at and --follow are exclusive".into()),
        (Some(s), None) => {
            let (x, y) = s.split_once(',').ok_or("--at X,Y")?;
            let (x, y): (f64, f64) =
                (x.trim().parse().map_err(|_| "--at X,Y")?, y.trim().parse().map_err(|_| "--at X,Y")?);
            let t = f.tile(x, y);
            if usize::from(t.x) >= w || usize::from(t.y) >= h {
                return Err(format!("--at {x},{y}: tile {t:?} is off the {w} x {h} map"));
            }
            Some(t)
        }
        (None, Some(_)) => None,
        (None, None) => Some(busiest(&world, &f)?),
    };
    let ring_now = |world: &World| -> TilePos {
        fixed
            .or_else(|| subject.and_then(|e| subject_tile(world, e)))
            .unwrap_or(TilePos::new((w / 2) as u16, (h / 2) as u16))
    };
    let grow = |r: Rect, m: u16| -> Rect {
        let x = r.x.saturating_sub(m);
        let y = r.y.saturating_sub(m);
        Rect {
            x,
            y,
            w: r.w.saturating_add(r.x - x).saturating_add(m),
            h: r.h.saturating_add(r.y - y).saturating_add(m),
        }
    };
    let first = view::ring_about(ring_now(&world), half, w, h);
    let mut pinned = BTreeSet::new();
    if let Some(e) = subject {
        if let Some(b) = world.comp_mut::<citysim::Brain>(e) {
            b.pinned = true;
        }
    }
    let mut frames = vec![];
    let mut pins = 0;
    for _ in 0..args.ticks {
        let ring = view::ring_about(ring_now(&world), half, w, h);
        world.set_view(Some(ring));
        if !args.no_pin {
            pins += view::pin_ring(&mut world, ring, grow(ring, cfg.pin_margin_tiles), &mut pinned);
        }
        citysim::tick(&mut world);
        let ring = view::ring_about(ring_now(&world), half, w, h);
        frames.push(view::snapshot(&world, ring));
    }
    let keep = args.keep.unwrap_or(frames.len());
    let frames = &frames[frames.len() - keep..];
    let lods = |fr: &ViewFrame| {
        let mut c: std::collections::BTreeMap<String, usize> = Default::default();
        for a in &fr.agents {
            *c.entry(a.lod.map_or("Child".into(), |l| format!("{l:?}"))).or_default() += 1;
        }
        c
    };
    eprintln!(
        "ring {first:?} ({pins} pinned over the run); first frame lods {:?}, last {:?}",
        lods(&frames[0]),
        lods(&frames[keep - 1])
    );
    let buildings: Vec<Value> = view::buildings_in(&world, grow(first, half))
        .into_iter()
        .map(|b| {
            json!({
                "id": id(b.id), "kind": format!("{:?}", b.kind), "rect": rect(&b.rect), "door": tp(b.door),
                "floors": b.floors, "tier": b.tier, "capacity": b.capacity, "owner": b.owner.map(id),
                "within": b.within.map(id), "span": b.span.map(|(lo, hi)| [lo, hi]), "label": b.label,
            })
        })
        .collect();
    let lines: Vec<Value> = world
        .map
        .lines
        .iter()
        .map(|l| json!({"name": l.name, "in_flight": l.in_flight, "waypoints": l.waypoints}))
        .collect();
    let (cx, cy) = f.metres(TilePos::new(first.x + first.w / 2, first.y + first.h / 2));
    let doc = json!({
        "schema": "city-simview/0.1",
        "source": {"save": args.save.display().to_string(), "seed": world.seed(), "map_size": [w, h], "subject": subject.map(id)},
        "frame": {"tile_m": f.s, "x0": 0.0, "y1": f.north, "axes": "tiles x east, y south; metres x = x0 + (tx + 0.5) * tile_m, y = y1 - (ty + 0.5) * tile_m"},
        "time": {"ticks_per_hour": time::TICKS_PER_HOUR, "ticks_per_day": time::TICKS_PER_DAY, "tick_is": "one game minute"},
        "ring": {"rect": rect(&first), "centre_m": [cx, cy], "pinned": pins},
        "buildings": buildings,
        "lines": lines,
        "frames": frames.iter().map(frame_json).collect::<Vec<_>>(),
    });
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&args.out, &text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    eprintln!(
        "wrote {} ({} frames, {} KB, {:.1} s)",
        args.out.display(),
        keep,
        text.len() / 1024,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
