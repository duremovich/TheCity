//! The follow-cam on a recording (the view milestone V6, `OPEN-270`): `--sim FILE` where FILE is a shadow's
//! recording (`citysim-cli shadow --record`: `<out>/rec/<arch>_<index>_d<day>.simrec.gz`, `city-simrec/0.1`, one
//! pick's ring for one diary day). A thread reads the file a line a tick, applies each delta
//! (`citysim::view::RecDecoder`) and publishes the frame into the same two-slot buffer the live sim thread fills
//! (`simthread::Shared`), so the follow, the HUD's caption (the recorded `view::caption`) and `--bench one --shots`
//! work as on `--save`, on the shadow's own world: a diary line and the shot at its minute agree (no re-run, no
//! determinism needed).
//!
//! The pace: `--rate` (default the recorded world's walk-matched rate), `--from TICK` runs flat out to that frame;
//! a bench's hold reads flat out to the shot's frame and waits. A shot at diary minute `T` shows frame `T + 1`
//! (`citysim::view`'s module doc). A shot or `--from` outside the file's day is an error (exit 2), as is a
//! `--follow` that is not the recording's pick. At the file's end free-running holds the last frame.

use crate::simthread::{Published, Shared, Static, camera_line, fatal, first_hold};
use citysim::view::{REC_SCHEMA, RecDecoder, RecFrame, RecHeader, ViewFrame};
use flate2::read::GzDecoder;
use std::io::{BufRead, BufReader, Lines, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub struct RecArgs {
    pub path: PathBuf,
    pub follow: Option<u32>,
    pub rate: Option<f64>,
    pub from: Option<u64>,
    pub shots: Vec<u64>,
}

/// A gzip file (a recording) rather than the JSON fixture (`city-simview/0.1`, sim.rs's `Sim::load`).
pub fn is_recording(p: &Path) -> bool {
    let mut magic = [0u8; 2];
    std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && magic == [0x1f, 0x8b]
}

pub fn spawn(a: RecArgs) -> Arc<Shared> {
    let shared = Arc::new(Shared::new(first_hold(&a.shots)));
    let sh = shared.clone();
    std::thread::Builder::new().name("rec".into()).spawn(move || run(a, sh)).expect("spawn the recording thread");
    shared
}

type Src = Lines<BufReader<GzDecoder<std::fs::File>>>;

/// The next tick's frame and caption; `None` at the file's end.
fn next(src: &mut Src, dec: &mut RecDecoder, path: &Path) -> Option<(ViewFrame, String)> {
    let line = src.next()?.unwrap_or_else(|e| fatal(format!("--sim {}: {e}", path.display())));
    let r: RecFrame = serde_json::from_str(&line).unwrap_or_else(|e| fatal(format!("--sim {}: a frame: {e}", path.display())));
    Some(dec.apply(r))
}

fn run(a: RecArgs, sh: Arc<Shared>) {
    let t0 = Instant::now();
    let file = std::fs::File::open(&a.path).unwrap_or_else(|e| fatal(format!("--sim {}: {e}", a.path.display())));
    let mut src: Src = BufReader::new(GzDecoder::new(file)).lines();
    let head = src.next().and_then(|l| l.ok()).unwrap_or_else(|| fatal(format!("--sim {}: no header line", a.path.display())));
    let h: RecHeader = serde_json::from_str(&head).unwrap_or_else(|e| fatal(format!("--sim {}: the header: {e}", a.path.display())));
    if h.schema != REC_SCHEMA {
        fatal(format!("--sim {}: schema {} (this cityview plays {REC_SCHEMA})", a.path.display(), h.schema));
    }
    if let Some(ix) = a.follow.filter(|&ix| ix != h.subject.index) {
        fatal(format!("--follow {ix}: the recording's pick is {} #{}", h.name, h.subject.index));
    }
    // the file's frames: the diary day's minutes, frame T + 1 for minute T
    let tpd = citysim::TICKS_PER_DAY;
    let (lo, hi) = (h.day * tpd, h.day * tpd + tpd - 1);
    if let Some(&t) = a.shots.iter().find(|&&t| t < lo || t > hi) {
        fatal(format!("--shots {t}: outside the recording's diary day {} (minutes {lo}..={hi})", h.day));
    }
    if let Some(t) = a.from.filter(|&t| t <= lo || t > hi + 1) {
        fatal(format!("--from {t}: outside the recording's frames {}..={}", lo + 1, hi + 1));
    }
    let mut dec = RecDecoder::default();
    let Some(mut cur) = next(&mut src, &mut dec, &a.path) else { fatal(format!("--sim {}: no frames", a.path.display())) };
    if let Some(&t) = a.shots.first().filter(|&&t| t + 1 < cur.0.tick) {
        fatal(format!("--shots {t}: the recording starts at frame {} (minute {})", cur.0.tick, cur.0.tick - 1));
    }
    let rate = a.rate.unwrap_or(h.rate);
    sh.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
    *sh.stat.lock().unwrap() = Some(Static {
        lines: h.lines.clone(),
        tile_m: h.tile_m as f32,
        north_m: h.north_m as f32,
        // a recording carries no crossings (city-simrec/0.1): no signal heads in a replay (OPEN-260's leftovers)
        crossings: Vec::new(),
        stir_tiles: 0,
    });
    eprintln!(
        "cityview: playing {} ({} {} #{}, diary day {}, from frame {}) at {rate:.4} ticks/s ({:.1} s)",
        a.path.display(),
        h.arch,
        h.name,
        h.subject.index,
        h.day,
        cur.0.tick,
        t0.elapsed().as_secs_f64()
    );
    let from = a.from.unwrap_or(cur.0.tick);
    let mut seq = 0u64;
    let mut next_at = Instant::now();
    let mut ended = false;
    let mut first = true;
    loop {
        let hold = sh.hold.load(Ordering::Relaxed);
        let tick = cur.0.tick;
        sh.tick.store(tick, Ordering::Relaxed);
        let paced = hold == u64::MAX && tick >= from;
        let target = if hold == u64::MAX { from } else { hold };
        // publish the frame we stand on: every paced one, flat out the two before the hold (the lerp's pair)
        if first || paced || tick + 1 >= target {
            first = false;
            if !paced && tick >= target {
                eprintln!("cityview: frame {tick} (diary {}) {}", citysim::view::diary_minute(tick), cur.1);
            }
            seq += 1;
            let caption = format!("{}\n{}", cur.1, camera_line(&cur.0, h.view));
            let mut s = sh.slots.lock().unwrap();
            // T2: a frame a tick at the recording's rate (its ways ahead are the recorded tick's)
            let from_tick = s.latest.as_ref().map_or(tick.saturating_sub(1), |l| l.frame.tick);
            // (flat out, span 0: the frame at once, a bench's shot its tick exactly)
            let span_s = if paced { (tick.saturating_sub(from_tick).max(1) as f64 / rate.max(1e-9)) as f32 } else { 0.0 };
            let p = Arc::new(Published {
                frame: cur.0.clone(),
                subject: h.subject,
                caption,
                at: Instant::now(),
                seq,
                from_tick,
                span_s,
                clocked: false,
                strides: Default::default(),
            });
            s.prev = s.latest.take();
            s.latest = Some(p);
            next_at = next_at.max(Instant::now());
        }
        // wait for the next step: the hold reached, the pace, the end
        loop {
            let hold = sh.hold.load(Ordering::Relaxed);
            if tick >= hold || ended {
                if ended && hold != u64::MAX && tick < hold {
                    fatal(format!("the recording ends at frame {tick}, before the hold {hold}"));
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            let paced = hold == u64::MAX && tick >= from;
            if paced && rate > 0.0 {
                let now = Instant::now();
                if now < next_at {
                    std::thread::sleep((next_at - now).min(Duration::from_millis(20)));
                    continue;
                }
                next_at = next_at.max(now) + Duration::from_secs_f64(1.0 / rate);
            } else if paced {
                // --rate 0: hold the frame
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            break;
        }
        match next(&mut src, &mut dec, &a.path) {
            Some(f) => cur = f,
            None => {
                eprintln!("cityview: the recording ends at frame {}", cur.0.tick);
                ended = true;
            }
        }
    }
}
