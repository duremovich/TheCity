//! `shadow --record` (the view milestone V6, `OPEN-270`): the picks' rings a tick, for the follow-cam to play.
//!
//! After each tick of the shadow's window, every recorded pick's ring (`[view] record_half_tiles` about the pick,
//! `citysim::view::subject_tile`) is snapshotted from the one world in one pass (`view::snapshot_many`) and written
//! as a `city-simrec/0.1` line (`view::RecFrame`: a delta on the pick's last frame, with the HUD's caption) to
//! `<out>/rec/<arch>_<index>_d<day>.simrec.gz`, one gzip'd file per pick per diary day (the day of the minute the
//! frame shows, `frame.tick - 1`; a new file starts with a full frame). The shadow sets no view: the ring is the
//! pick's surroundings as the shadow's world has them (the pick pinned Full, the rest at the tier the city gives
//! them), so recording changes nothing in the diary's world. The El trains are the ring's (the live view draws
//! every train in the city; a recording keeps the ones in the ring and the pick's own). cityview `--sim FILE` plays
//! a file (`view/cityview/src/recplay.rs`). Read-only, no RNG.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use citysim::view::{self, RecEncoder, RecHeader, REC_SCHEMA};
use citysim::{time, Corpse, EntityId, World};
use flate2::write::GzEncoder;
use flate2::Compression;

/// Bytes through, counted (the raw JSON's size beside the gzip'd file's).
struct Counting<W: Write> {
    w: W,
    n: u64,
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let k = self.w.write(buf)?;
        self.n += k as u64;
        Ok(k)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}

struct Open {
    day: u64,
    path: PathBuf,
    w: Counting<GzEncoder<BufWriter<File>>>,
    enc: RecEncoder,
    frames: u64,
}

struct Pick {
    id: EntityId,
    arch: String,
    name: String,
    open: Option<Open>,
    /// Stopped (dead or placeless): nothing more is written.
    done: bool,
}

pub struct Recorder {
    dir: PathBuf,
    half: u16,
    level: u32,
    picks: Vec<Pick>,
    /// Each file closed: (path, frames, raw JSON bytes, file bytes).
    pub written: Vec<(PathBuf, u64, u64, u64)>,
    /// `--record-view`: the one recorded pick the shadow's view follows.
    view: Option<EntityId>,
    /// Wall time in `after_tick` (the snapshots, the encoding, the gzip).
    secs: f64,
    /// Of it, in `view::snapshot_many`.
    snap_secs: f64,
    /// The walkers' ways, kept across ticks.
    ways: view::WayCache,
}

/// `--record`'s selection: `all`, or archetypes and entity indices, comma-separated.
pub fn selected(sel: &str, picked: &[(String, EntityId)]) -> Result<Vec<(String, EntityId)>, String> {
    if sel.trim() == "all" {
        return Ok(picked.to_vec());
    }
    let mut out = vec![];
    for tok in sel.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let hits: Vec<&(String, EntityId)> =
            picked.iter().filter(|(a, id)| a == tok || tok.parse::<u32>().is_ok_and(|ix| ix == id.index)).collect();
        if hits.is_empty() {
            return Err(format!("--record {tok}: no pick of that archetype or index"));
        }
        for h in hits {
            if !out.contains(h) {
                out.push(h.clone());
            }
        }
    }
    Ok(out)
}

impl Recorder {
    pub fn new(world: &World, dir: &Path, picks: &[(String, EntityId)]) -> Result<Recorder, String> {
        let dir = dir.join("rec");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let level = world.config.view.record_gzip_level;
        if level > 9 {
            return Err(format!("[view] record_gzip_level {level} is outside 0..=9"));
        }
        Ok(Recorder {
            dir,
            half: world.config.view.record_half_tiles,
            level,
            picks: picks
                .iter()
                .map(|(arch, id)| Pick {
                    id: *id,
                    arch: arch.clone(),
                    name: world.name_of(*id),
                    open: None,
                    done: false,
                })
                .collect(),
            written: vec![],
            view: None,
            secs: 0.0,
            snap_secs: 0.0,
            ways: view::WayCache::default(),
        })
    }

    /// `--record-view`: the view follows the one recorded pick (an error with more than one).
    pub fn set_view(&mut self) -> Result<(), String> {
        match self.picks.as_slice() {
            [p] => {
                eprintln!("record: the view follows {} #{} (its ring Full, the camera ring's)", p.name, p.id.index);
                self.view = Some(p.id);
                Ok(())
            }
            ps => {
                Err(format!("--record-view needs one recorded pick ({} are recorded: name one in --record)", ps.len()))
            }
        }
    }

    /// Before a tick: `--record-view`'s view on the pick (the follow-cam's ring, `[view] ring_half_tiles`).
    pub fn before_tick(&self, world: &mut World) {
        let Some(id) = self.view else { return };
        let ring = view::subject_tile(world, id)
            .map(|t| view::ring_about(t, world.config.view.ring_half_tiles, world.map.w(), world.map.h()));
        world.set_view(ring);
    }

    fn close(&mut self, o: Open) -> Result<(), String> {
        let bufw = o.w.w.finish().map_err(|e| format!("{}: {e}", o.path.display()))?;
        bufw.into_inner().map_err(|e| format!("{}: {e}", o.path.display()))?;
        let bytes = std::fs::metadata(&o.path).map_err(|e| format!("{}: {e}", o.path.display()))?.len();
        self.written.push((o.path, o.frames, o.w.n, bytes));
        Ok(())
    }

    /// After a tick: each recorded pick's ring as the world has it now, a line of its day's file.
    pub fn after_tick(&mut self, world: &World) -> Result<(), String> {
        let t0 = std::time::Instant::now();
        let r = self.record(world);
        self.secs += t0.elapsed().as_secs_f64();
        r
    }

    fn record(&mut self, world: &World) -> Result<(), String> {
        let day = time::day(world.tick.saturating_sub(1));
        let (mw, mh) = (world.map.w(), world.map.h());
        let mut live = vec![];
        let mut rings = vec![];
        let mut stopped = vec![];
        for (k, p) in self.picks.iter_mut().enumerate() {
            if p.done {
                continue;
            }
            match view::subject_tile(world, p.id).filter(|_| !world.has::<Corpse>(p.id)) {
                Some(t) => {
                    live.push(k);
                    rings.push(view::ring_about(t, self.half, mw, mh));
                }
                None => {
                    eprintln!(
                        "record: {} #{} has no place at tick {}: its recording stops",
                        p.name, p.id.index, world.tick
                    );
                    p.done = true;
                    stopped.extend(p.open.take());
                }
            }
        }
        for o in stopped {
            self.close(o)?;
        }
        let t0 = std::time::Instant::now();
        let frames = view::snapshot_many_cached(world, &rings, Some(&mut self.ways));
        self.snap_secs += t0.elapsed().as_secs_f64();
        for (k, mut f) in live.into_iter().zip(frames) {
            // the El trains in the ring only (the snapshot holds every train, wherever it is: the live view's whole
            // city, two fifths of a recording's bytes), and the pick's own vehicle
            let mine = f.agent(self.picks[k].id).and_then(|a| a.aboard);
            let ring = f.ring;
            f.vehicles.retain(|v| ring.contains(v.t) || Some(v.id) == mine);
            if self.picks[k].open.as_ref().is_some_and(|o| o.day != day) {
                let o = self.picks[k].open.take().expect("open");
                self.close(o)?;
            }
            if self.picks[k].open.is_none() {
                let p = &self.picks[k];
                let path = self.dir.join(format!("{}_{}_d{day}.simrec.gz", p.arch, p.id.index));
                let file = File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let mut w = Counting { w: GzEncoder::new(BufWriter::new(file), Compression::new(self.level)), n: 0 };
                let header = RecHeader {
                    schema: REC_SCHEMA.into(),
                    seed: world.seed(),
                    map_size: [mw, mh],
                    subject: p.id,
                    name: p.name.clone(),
                    arch: p.arch.clone(),
                    day,
                    tile_m: world.config.view.tile_m,
                    north_m: world.config.view.north_m,
                    half: self.half,
                    rate: view::walk_matched_rate(world),
                    lines: world.map.lines.iter().map(|l| l.name.clone()).collect(),
                    minute: "a frame's tick is World.tick after the tick ran: frame T + 1 shows the state the diary \
                             stamps T (citysim::view's module doc)"
                        .into(),
                    view: self.view == Some(p.id),
                };
                serde_json::to_writer(&mut w, &header).map_err(|e| e.to_string())?;
                w.write_all(b"\n").map_err(|e| format!("{}: {e}", path.display()))?;
                self.picks[k].open = Some(Open { day, path, w, enc: RecEncoder::default(), frames: 0 });
            }
            let caption = view::caption(world, &f, self.picks[k].id);
            let o = self.picks[k].open.as_mut().expect("open");
            let r = o.enc.encode(&f, caption);
            serde_json::to_writer(&mut o.w, &r).map_err(|e| e.to_string())?;
            o.w.write_all(b"\n").map_err(|e| format!("{}: {e}", o.path.display()))?;
            o.frames += 1;
        }
        Ok(())
    }

    /// Close every open file; one line per file written.
    pub fn finish(mut self) -> Result<Vec<(PathBuf, u64, u64, u64)>, String> {
        let open: Vec<Open> = self.picks.iter_mut().filter_map(|p| p.open.take()).collect();
        for o in open {
            self.close(o)?;
        }
        eprintln!(
            "record: {:.1} s recording {} picks ({:.1} s of it in the snapshots)",
            self.secs,
            self.picks.len(),
            self.snap_secs
        );
        Ok(self.written)
    }
}
