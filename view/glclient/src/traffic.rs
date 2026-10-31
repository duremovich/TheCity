//! Traffic (P7 G, spec/city/P7_PLAN.md): `pipeline/city/street/traffic.py` writes `out/street/traffic.json`, the
//! keep-left lanes and the junction connectors (points with z, successors, conflicts, a turn's radius). The client
//! runs box cars on them within the street ring of the focus:
//! - a car follows its segment (lane or connector) by arc length; at a lane's end it takes a connector picked at
//!   random when it entered the lane (straight / left / right weights), at a connector's end its out lane;
//! - gap keeping: the speed is capped by what stops it GAP short of the car ahead (on its segment or the next);
//! - junctions: a car may enter a connector only when no conflicting connector is held and its out lane has room
//!   for it (don't block the box); it holds (reserves) the connector from RESERVE m out until its tail leaves it
//!   (and gives it back if the out lane fills before it enters); a car waiting WAIT_PRIO s at a line has priority
//!   over newer cars; one waiting REPLAN s for room takes another exit with room (a queue round a block);
//! - cars spawn on the ring's lanes at a density per class (filled when a lane comes into the ring, topped up at
//!   lane starts: sources and lanes entering from outside) and vanish past the ring or at a sink (a dead end);
//! - buses and minibuses keep to arterial / tram lanes and take only arcs of BIG_R m or more (their width).
//! `Sim` is plain data (no Bevy): cityview's headless `--traffic-trace` runs it for the gates (checks_traffic).
//! Bodies: the city fleet's glbs (vehicles.rs, P8) picked per car id from its kind's variants; wheels spin with
//! the distance driven and the front pair steers with the heading rate, hover cars float at their hover height;
//! a kind without a fleet model (or CV_BOX_TRAFFIC) keeps the P7 boxes. Head / tail lights are emissive (no light
//! entities).

use bevy::{camera::primitives::Aabb, prelude::*};
use serde::Deserialize;
use std::collections::HashMap;

use crate::block::hex;
use crate::frame::to_bevy;
use crate::street::{Acc, Street};
use crate::city::{GlLight, LM_PER_W, Role as LightRole};
use crate::lighting::MovingLight;
use crate::vehicles::{Fleet, Role};

#[derive(Deserialize, Clone, Debug)]
pub struct LaneRec {
    pub pts: Vec<[f32; 3]>,
    pub cls: String,
    pub next: Vec<usize>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ConnRec {
    pub pts: Vec<[f32; 3]>,
    pub out: usize,
    pub conf: Vec<usize>,
    /// conflicts with a bus on either side
    #[serde(default)]
    pub conf_big: Vec<usize>,
    pub turn: String,
    #[serde(default)]
    pub r: f32,
    /// a bus sweeps it clear
    #[serde(default)]
    pub big: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct TrafficFile {
    #[serde(default)]
    pub speed: HashMap<String, f32>,
    #[serde(default)]
    pub density: HashMap<String, f32>,
    #[serde(default)]
    pub lanes: Vec<LaneRec>,
    #[serde(default)]
    pub conns: Vec<ConnRec>,
}

/// A segment: a lane (0 .. lanes) or a connector (lanes ..).
pub struct Seg {
    pub pts: Vec<Vec3>,
    pub cum: Vec<f32>,
    pub len: f32,
    pub next: Vec<usize>,
    pub conf: Vec<usize>,
    pub conf_big: Vec<usize>,
    pub vmax: f32,
    pub conn: bool,
    pub turn: u8,
    pub r: f32,
    pub big: bool,
    pub cls: u8,
    pub source: bool,
    pub density: f32,
    pub lo: Vec2,
    pub hi: Vec2,
}

impl Seg {
    /// (point, unit direction) at arc length s (clamped to the segment)
    pub fn at(&self, s: f32) -> (Vec3, Vec3) {
        let s = s.clamp(0.0, self.len);
        let k = self.cum.partition_point(|c| *c <= s).clamp(1, self.cum.len() - 1);
        let (a, b) = (self.pts[k - 1], self.pts[k]);
        let span = (self.cum[k] - self.cum[k - 1]).max(1e-4);
        let f = (s - self.cum[k - 1]) / span;
        (a.lerp(b, f), (b - a).normalize_or(Vec3::X))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Car,
    Taxi,
    Van,
    Minibus,
    Bus,
    Bike,
}

impl Kind {
    /// (length, width, height) m
    pub fn dims(self) -> (f32, f32, f32) {
        match self {
            Kind::Car => (4.5, 1.8, 1.45),
            Kind::Taxi => (4.6, 1.75, 1.5),
            Kind::Van => (5.0, 1.9, 2.0),
            Kind::Minibus => (7.0, 2.1, 2.6),
            Kind::Bus => (12.0, 2.5, 4.3),
            Kind::Bike => (2.2, 0.9, 1.5),
        }
    }
    pub fn big(self) -> bool {
        matches!(self, Kind::Minibus | Kind::Bus)
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Car => "car",
            Kind::Taxi => "taxi",
            Kind::Van => "van",
            Kind::Minibus => "minibus",
            Kind::Bus => "bus",
            Kind::Bike => "bike",
        }
    }
}

/// P8: couriers (bikes) take 0.08 of the cars' share; a bike keeps its lane like a car (no splitting)
const KINDS: [(Kind, f32); 6] = [(Kind::Car, 0.37), (Kind::Taxi, 0.25), (Kind::Van, 0.12), (Kind::Minibus, 0.1), (Kind::Bus, 0.08), (Kind::Bike, 0.08)];
/// m from a car's front to the next car's tail it stops at
pub const GAP: f32 = 2.0;
/// m before a connector a car reserves it
const RESERVE: f32 = 12.0;
const ACC: f32 = 2.0;
const BRAKE: f32 = 4.0;
/// s a car waits at a line before it has the junction's priority (newer cars leave its connector's conflicts)
const WAIT_PRIO: f32 = 10.0;
/// s a car waits at a line for room on its planned exit before it takes another exit with room (gridlock)
const REPLAN: f32 = 8.0;
/// turn weights at a lane's end: straight, left, right
const TURN_W: [f32; 3] = [0.6, 0.25, 0.15];
/// m past the ring a car vanishes; m inside it a lane start counts as an entry
const EDGE: f32 = 20.0;
/// s of traffic run when the ring is first set (the view opens on settled traffic)
const PREROLL: f32 = 60.0;

#[derive(Clone, Debug)]
pub struct Car {
    pub id: u64,
    pub kind: Kind,
    pub seg: usize,
    /// the front's arc length on `seg`
    pub s: f32,
    pub v: f32,
    /// the segment entered before `seg` (the tail may still be on it)
    pub prev: Option<usize>,
    /// the next segment (picked on entering `seg`)
    pub plan: Option<usize>,
    /// connectors held (reserved / occupied) until the tail leaves them
    pub holds: Vec<usize>,
    /// s travelled on `seg` + earlier segments since the last junction (for the tail's release)
    pub since: f32,
    pub stopped: f32,
}

/// Switches for the gates' controls (checks_traffic: each must fail its gate).
#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    /// ignore the car ahead and the junction holds (overlaps)
    pub no_gap: bool,
    /// nobody moves (throughput)
    pub freeze: bool,
}

pub struct Sim {
    pub segs: Vec<Seg>,
    pub lanes: usize,
    pub cars: Vec<Car>,
    /// connector -> cars holding it (id, a bus or minibus)
    held: Vec<Vec<(u64, bool)>>,
    /// lane -> a bus / minibus can drive it and always leave it (an arterial / tram lane long enough with a big
    /// connector onto another such lane, or a sink): the greatest such set (Sim::new)
    big_ok: Vec<bool>,
    /// connector -> the oldest car waiting WAIT_PRIO at its line for it (id, a bus or minibus)
    want: Vec<Option<(u64, bool)>>,
    rng: u64,
    next_id: u64,
    /// the ring: centre, half size
    pub centre: Vec2,
    pub half: f32,
    active: Vec<bool>,
    spawn_t: f32,
    pub faults: Faults,
    /// counters: spawned, gone, connectors crossed
    pub spawned: u64,
    pub gone: u64,
    pub crossed: u64,
}

fn rand01(r: &mut u64) -> f32 {
    *r ^= *r << 13;
    *r ^= *r >> 7;
    *r ^= *r << 17;
    (*r >> 40) as f32 / (1u64 << 24) as f32
}

impl Sim {
    pub fn new(f: &TrafficFile, seed: u64) -> Sim {
        let cls_id = |c: &str| match c {
            "arterial" => 0u8,
            "tram" => 1,
            _ => 2,
        };
        let mut segs = vec![];
        let nl = f.lanes.len();
        let mut has_pred = vec![false; nl];
        for c in &f.conns {
            has_pred[c.out] = true;
        }
        let mk = |pts: &[[f32; 3]]| -> (Vec<Vec3>, Vec<f32>, Vec2, Vec2) {
            let p: Vec<Vec3> = pts.iter().map(|a| Vec3::from_array(*a)).collect();
            let mut cum = vec![0.0];
            for w in p.windows(2) {
                cum.push(cum.last().unwrap() + w[0].truncate().distance(w[1].truncate()));
            }
            let lo = p.iter().fold(Vec2::splat(f32::MAX), |a, b| a.min(b.truncate()));
            let hi = p.iter().fold(Vec2::splat(f32::MIN), |a, b| a.max(b.truncate()));
            (p, cum, lo, hi)
        };
        for (i, l) in f.lanes.iter().enumerate() {
            let (pts, cum, lo, hi) = mk(&l.pts);
            let len = *cum.last().unwrap();
            segs.push(Seg {
                pts,
                cum,
                len,
                next: l.next.iter().map(|c| nl + c).collect(),
                conf: vec![],
                conf_big: vec![],
                vmax: f.speed.get(&l.cls).copied().unwrap_or(9.0),
                conn: false,
                turn: 0,
                r: 0.0,
                big: true,
                cls: cls_id(&l.cls),
                source: !has_pred[i],
                density: f.density.get(&l.cls).copied().unwrap_or(0.7) / 100.0,
                lo,
                hi,
            });
        }
        for c in &f.conns {
            let (pts, cum, lo, hi) = mk(&c.pts);
            let len = *cum.last().unwrap();
            let turn = match c.turn.as_str() {
                "s" => 0,
                "l" => 1,
                _ => 2,
            };
            segs.push(Seg {
                pts,
                cum,
                len,
                next: vec![c.out],
                conf: c.conf.iter().map(|x| nl + x).collect(),
                conf_big: c.conf_big.iter().map(|x| nl + x).collect(),
                vmax: if turn == 0 { 10.0 } else { (1.6 * c.r).clamp(4.0, 8.0) },
                conn: true,
                turn,
                r: c.r,
                big: c.big,
                cls: 2,
                source: false,
                density: 0.0,
                lo,
                hi,
            });
        }
        let n = segs.len();
        // big_ok: start from every long-enough arterial / tram lane, drop lanes with no big way on until none drops
        let need = Kind::Bus.dims().0 + GAP + 0.5;
        let mut big_ok: Vec<bool> = (0..n).map(|i| i < nl && segs[i].cls <= 1 && segs[i].len >= need).collect();
        loop {
            let mut changed = false;
            for i in 0..nl {
                if big_ok[i] && !segs[i].next.is_empty() && !segs[i].next.iter().any(|c| segs[*c].big && big_ok[segs[*c].next[0]]) {
                    big_ok[i] = false;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Sim {
            big_ok,
            segs,
            lanes: nl,
            cars: vec![],
            held: vec![vec![]; n],
            want: vec![None; n],
            rng: seed | 1,
            next_id: 0,
            centre: Vec2::ZERO,
            half: 0.0,
            active: vec![false; n],
            spawn_t: 0.0,
            faults: Faults::default(),
            spawned: 0,
            gone: 0,
            crossed: 0,
        }
    }

    fn inside(&self, p: Vec2, margin: f32) -> bool {
        (p - self.centre).abs().max_element() <= self.half + margin
    }

    fn seg_active(&self, i: usize) -> bool {
        let s = &self.segs[i];
        let (lo, hi) = (self.centre - Vec2::splat(self.half), self.centre + Vec2::splat(self.half));
        s.hi.x >= lo.x && s.lo.x <= hi.x && s.hi.y >= lo.y && s.lo.y <= hi.y
    }

    fn pick_kind(&mut self, seg: usize) -> Kind {
        let big_ok = self.big_ok[seg];
        loop {
            let mut x = rand01(&mut self.rng);
            for (k, w) in KINDS {
                if x < w {
                    if k.big() && !big_ok {
                        break;
                    }
                    return k;
                }
                x -= w;
            }
        }
    }

    /// The next segment from `seg` for a car of `kind` (weighted by turn), None at a sink.
    fn pick_next(&mut self, seg: usize, kind: Kind) -> Option<usize> {
        // an exit lane long enough to hold it stopped (it enters only when it fits); a big vehicle: a connector with
        // a wide enough arc onto a lane it can always leave again (big_ok: it never strands at a line, /code-review
        // s30; a connector's next is its lane)
        let need = kind.dims().0 + GAP + 0.5;
        let ok = |n: usize| -> bool {
            let s = &self.segs[n];
            let lane = if s.conn { s.next[0] } else { n };
            self.segs[lane].len >= need && (!kind.big() || (s.conn && s.big && self.big_ok[lane]) || (!s.conn && self.big_ok[n]))
        };
        let nx: Vec<usize> = self.segs[seg].next.iter().copied().filter(|n| ok(*n)).collect();
        if nx.is_empty() {
            return None;
        }
        if !self.segs[seg].conn {
            let tot: f32 = nx.iter().map(|n| TURN_W[self.segs[*n].turn as usize]).sum();
            let mut x = rand01(&mut self.rng) * tot;
            for n in &nx {
                x -= TURN_W[self.segs[*n].turn as usize];
                if x <= 0.0 {
                    return Some(*n);
                }
            }
        }
        Some(nx[0])
    }

    fn spawn(&mut self, seg: usize, s: f32, v: f32) {
        let kind = self.pick_kind(seg);
        let plan = self.pick_next(seg, kind);
        self.next_id += 1;
        self.cars.push(Car { id: self.next_id, kind, seg, s, v, prev: None, plan, holds: vec![], since: s, stopped: 0.0 });
        self.spawned += 1;
    }

    /// Cars on `seg` sorted by s (front positions), with their lengths
    fn occupancy(&self) -> Vec<Vec<(f32, f32, usize)>> {
        let mut occ: Vec<Vec<(f32, f32, usize)>> = vec![vec![]; self.segs.len()];
        for (i, c) in self.cars.iter().enumerate() {
            let len = c.kind.dims().0;
            occ[c.seg].push((c.s, len, i));
            // the tail still on the previous segment occupies its end
            if let Some(p) = c.prev
                && c.s < len
            {
                occ[p].push((self.segs[p].len + c.s, len, i));
            }
        }
        for o in occ.iter_mut() {
            o.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        occ
    }

    /// Fill a lane that came into the ring at its density (random spacing, never closer than a car + GAP + 4).
    fn fill(&mut self, seg: usize) {
        // the part inside the ring only (a car placed outside would vanish at once)
        let len = self.segs[seg].len;
        let pitch = 1.0 / self.segs[seg].density.max(1e-3);
        let mut s = len - 1.0 - pitch * rand01(&mut self.rng);
        let there: Vec<(f32, f32)> = self.cars.iter().filter(|c| c.seg == seg).map(|c| (c.s, c.kind.dims().0)).collect();
        while s >= 13.0 {
            let clear = there.iter().all(|(cs, cl)| s < cs - cl - GAP - 1.0 || s - 12.0 - GAP - 1.0 > *cs);
            if clear && self.inside(self.segs[seg].at(s).0.truncate(), 0.0) {
                let v = self.segs[seg].vmax * 0.5;
                self.spawn(seg, s, v);
            }
            s -= (pitch * (0.5 + rand01(&mut self.rng))).max(13.0 + GAP);
        }
    }

    pub fn set_ring(&mut self, centre: Vec2, half: f32) {
        let first = self.half == 0.0;
        self.centre = centre;
        self.half = half;
        for i in 0..self.lanes {
            let a = self.seg_active(i);
            if a && !self.active[i] {
                self.fill(i);
            }
            self.active[i] = a;
        }
        if first {
            // a minute of traffic before the first frame (the fill's even spacing settles into queues and gaps)
            for _ in 0..(PREROLL / 0.05) as usize {
                self.step(0.05);
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.want.iter_mut().for_each(|w| *w = None);
        for c in &self.cars {
            if let Some(nx) = c.plan
                && self.segs[nx].conn
                && c.stopped > WAIT_PRIO
                && self.segs[c.seg].len - c.s < 3.0
                && !c.holds.contains(&nx)
            {
                let w = &mut self.want[nx];
                if w.is_none_or(|x| c.id < x.0) {
                    *w = Some((c.id, c.kind.big()));
                }
            }
        }
        let occ = self.occupancy();
        let n = self.cars.len();
        let mut new_v = vec![0.0f32; n];
        let mut commit = vec![0.0f32; self.segs.len()];
        for c in &self.cars {
            let need = c.kind.dims().0 + GAP;
            let lane = if self.segs[c.seg].conn { self.segs[c.seg].next[0] } else { c.seg };
            commit[lane] += need;
            for h in &c.holds {
                if *h != c.seg && Some(*h) != c.prev {
                    commit[self.segs[*h].next[0]] += need;
                }
            }
        }
        // speeds (sequential: a connector reserved here is seen by the cars after)
        for i in 0..n {
            let (cid, cseg, cs, cv, plan, kind) = {
                let c = &self.cars[i];
                (c.id, c.seg, c.s, c.v, c.plan, c.kind)
            };
            let (seg_len, seg_vmax) = (self.segs[cseg].len, self.segs[cseg].vmax);
            let len = kind.dims().0;
            let to_end = seg_len - cs;
            let mut gap = f32::MAX;
            if !self.faults.no_gap {
                // the car ahead on this segment
                if let Some(a) = occ[cseg].iter().find(|o| o.0 > cs + 1e-3 && o.2 != i) {
                    gap = a.0 - a.1 - cs;
                }
                if gap == f32::MAX
                    && let Some(nx) = plan
                {
                    if self.segs[nx].conn {
                        // a junction: no conflicting connector held (a bus's wider conflicts on either side), none
                        // wanted by an older waiting car, room on the out lane, the connector's start free
                        let big = kind.big();
                        let out = self.segs[nx].next[0];
                        let held = &self.held;
                        let busy = self.segs[nx].conf.iter().any(|k| held[*k].iter().any(|h| h.0 != cid))
                            || self.segs[nx].conf_big.iter().any(|k| held[*k].iter().any(|h| h.0 != cid && (h.1 || big)));
                        // yield to an older waiting car (a bus's wider conflicts on either side)
                        let yield_ = self.segs[nx].conf.iter().any(|k| self.want[*k].is_some_and(|w| w.0 < cid))
                            || self.segs[nx].conf_big.iter().any(|k| self.want[*k].is_some_and(|w| w.0 < cid && (w.1 || big)));
                        // room: the out lane holds this car stopped behind every car on it or committed to it (a
                        // short lane between two junctions never takes a car whose tail would stay in the box)
                        let free = |k: usize| self.segs[k].len - 0.5 - commit[k];
                        let self_room = occ[nx].first().map_or(f32::MAX, |o| o.0 - o.1);
                        let mine = self.cars[i].holds.contains(&nx);
                        // (a car holding nx is already in commit[out]: /code-review s30, the hold flip-flopped)
                        let own = if mine && cseg != nx { len + GAP } else { 0.0 };
                        let room = free(out) + own >= len + GAP && self_room > GAP;
                        if !mine && !room && self.cars[i].stopped > REPLAN && to_end < 3.0 {
                            // the exit stays full: another exit with room (a queue around a block gridlocks)
                            let alt = self.segs[cseg].next.iter().copied().find(|k| {
                                *k != nx && (!big || self.segs[*k].big) && free(self.segs[*k].next[0]) >= len + GAP
                            });
                            if let Some(k) = alt {
                                self.cars[i].plan = Some(k);
                                self.cars[i].stopped = 0.0;
                            }
                        }
                        if mine && !room {
                            // reserved, not entered, and the out lane filled: give the junction back, wait at the line
                            self.held[nx].retain(|h| h.0 != cid);
                            self.cars[i].holds.retain(|h| *h != nx);
                            gap = to_end - 0.5 + GAP;
                        } else if !mine && (busy || yield_ || !room) {
                            // stop at the line (the front 0.5 m short of the lane's end)
                            gap = to_end - 0.5 + GAP;
                        } else if !mine && to_end <= RESERVE {
                            self.held[nx].push((cid, big));
                            self.cars[i].holds.push(nx);
                            commit[out] += len + GAP;
                        }
                    }
                    if gap == f32::MAX {
                        // the first car on the next segment (and on the lane after a connector)
                        if let Some(a) = occ[nx].first() {
                            gap = to_end + a.0 - a.1;
                        } else if self.segs[nx].conn {
                            let out = self.segs[nx].next[0];
                            if let Some(a) = occ[out].first() {
                                gap = to_end + self.segs[nx].len + a.0 - a.1;
                            }
                        }
                    }
                }
            }
            // slow for the segment ahead (a turn's speed)
            let vmax = seg_vmax.min(plan.map_or(f32::MAX, |nx| (self.segs[nx].vmax.powi(2) + 2.0 * BRAKE * to_end.max(0.0)).sqrt()));
            let safe = (2.0 * BRAKE * (gap - GAP).max(0.0)).sqrt();
            let target = vmax.min(safe);
            let v = if target > cv { (cv + ACC * dt).min(target) } else { target };
            new_v[i] = if self.faults.freeze { 0.0 } else { v };
        }
        // move
        let mut gone = vec![false; n];
        {
            let (cars, segs, held) = (&mut self.cars, &self.segs, &mut self.held);
            for i in 0..n {
                let c = &mut cars[i];
                let v = new_v[i];
                c.v = v;
                c.stopped = if v < 0.1 { c.stopped + dt } else { 0.0 };
                c.s += v * dt;
                c.since += v * dt;
                while c.s > segs[c.seg].len {
                    let Some(nx) = c.plan else {
                        gone[i] = true;
                        break;
                    };
                    c.s -= segs[c.seg].len;
                    c.prev = Some(c.seg);
                    c.seg = nx;
                    if segs[nx].conn {
                        self.crossed += 1;
                        c.since = c.s;
                        if !c.holds.contains(&nx) {
                            c.holds.push(nx);
                            held[nx].push((c.id, c.kind.big()));
                        }
                    }
                    c.plan = None;
                }
                if gone[i] {
                    continue;
                }
                // release the connectors the tail has left (a reserved one ahead stays)
                let len = c.kind.dims().0;
                let (seg_now, s_now, prev, id) = (c.seg, c.s, c.prev, c.id);
                c.holds.retain(|h| {
                    let still = *h == seg_now || (Some(*h) == prev && s_now < len) || segs[seg_now].next.contains(h);
                    if !still {
                        held[*h].retain(|x| x.0 != id);
                    }
                    still
                });
                let p = segs[c.seg].at(c.s).0.truncate();
                if (p - self.centre).abs().max_element() > self.half + EDGE {
                    gone[i] = true;
                }
            }
        }
        // plans for the cars that changed segment
        for i in 0..n {
            if !gone[i] && self.cars[i].plan.is_none() {
                let (seg, kind) = (self.cars[i].seg, self.cars[i].kind);
                self.cars[i].plan = self.pick_next(seg, kind);
            }
        }
        for i in (0..n).rev() {
            if gone[i] {
                let c = self.cars.swap_remove(i);
                for h in c.holds {
                    self.held[h].retain(|x| x.0 != c.id);
                }
                self.gone += 1;
            }
        }
        // top up the entries to their density: a source lane at its start, a lane coming in from outside the ring
        // where it enters it
        self.spawn_t += dt;
        if self.spawn_t >= 0.5 {
            self.spawn_t = 0.0;
            let occ = self.occupancy();
            for i in 0..self.lanes {
                if !self.active[i] || self.segs[i].len < 20.0 {
                    continue;
                }
                if occ[i].len() as f32 >= self.segs[i].len * self.segs[i].density + 0.5 {
                    continue;
                }
                let at = if self.segs[i].source {
                    13.0
                } else if !self.inside(self.segs[i].pts[0].truncate(), -EDGE) {
                    // the first point inside the ring (4 m steps), not before a car's length
                    let mut s = 13.0;
                    while s < self.segs[i].len && !self.inside(self.segs[i].at(s).0.truncate(), 0.0) {
                        s += 4.0;
                    }
                    if s >= self.segs[i].len - 5.0 {
                        continue;
                    }
                    s
                } else {
                    continue;
                };
                // room: no car within a bus + GAP + 4 behind or ahead of the spot
                let clear = 12.0 + GAP + 4.0;
                if occ[i].iter().any(|o| o.0 > at - clear && o.0 - o.1 < at + clear) {
                    continue;
                }
                let v = self.segs[i].vmax * 0.6;
                self.spawn(i, at, v);
            }
        }
    }

    /// A car's footprint: (centre, heading from +x, length, width)
    pub fn pose(&self, c: &Car) -> (Vec3, f32, f32, f32) {
        let (len, w, _) = c.kind.dims();
        let seg = &self.segs[c.seg];
        let (front, _) = seg.at(c.s);
        let rear = if c.s >= len {
            seg.at(c.s - len).0
        } else if let Some(p) = c.prev {
            self.segs[p].at(self.segs[p].len + c.s - len).0
        } else {
            seg.at(0.0).0 - seg.at(0.0).1 * (len - c.s)
        };
        let d = front - rear;
        ((front + rear) * 0.5, d.y.atan2(d.x), len, w)
    }
}

// ------------------------------------------------------------------------------------------------ Bevy

#[derive(Resource)]
pub struct Traffic {
    pub sim: Sim,
    pub entities: HashMap<u64, CarEnt>,
    meshes: HashMap<Kind, Vec<(Handle<Mesh>, Handle<StandardMaterial>, Aabb)>>,
}

/// A drawn car: its root, its wheels (entity, pivot, radius, front) and the state the animation integrates.
pub struct CarEnt {
    pub root: Entity,
    /// The fleet variant (None = boxes)
    pub model: Option<usize>,
    wheels: Vec<(Entity, Transform, f32, bool)>,
    hover: f32,
    /// Wheel roll angle (rad) and the last heading (rad) for the steer
    roll: f32,
    heading: f32,
    wheelbase: f32,
    /// Tail lamp parts (entity, normal material, brake material) and whether they show braking
    tails: Vec<(Entity, Handle<StandardMaterial>, Handle<StandardMaterial>)>,
    braking: bool,
    last_v: f32,
    /// The real lights while the car is among the nearest (head spot, underglow point)
    lights: Vec<Entity>,
}

/// Real head lamps for the HEAD_N cars nearest the focus (a car keeps them while it ranks within HEAD_KEEP: no
/// flicker as cars swap places); the rest only glow (P8 V-D).
const HEAD_N: usize = 8;
const HEAD_KEEP: usize = 12;
/// A pair of head lamps as one spot (W, the lamps' LM_PER_W), its reach (m) and cone (deg, outer / inner)
const HEAD_W: f32 = 150.0;
const HEAD_RANGE: f32 = 30.0;
const HEAD_CONE: (f32, f32) = (34.0, 18.0);
/// A hover car's underglow (Gutterlight: an area light, 20 W, palette hue): a point under the body
const UNDER_W: f32 = 20.0;
/// Brake lamps on below this deceleration (m/s2) or this speed (standing)
const BRAKE_DEC: f32 = 1.0;
const BRAKE_V: f32 = 0.3;

/// A wheel of a traffic car (spun / steered by `run`).
#[derive(Component)]
pub struct TrafficWheel;

/// Stats for the hud / benches.
#[derive(Resource, Default, Debug, Clone, serde::Serialize)]
pub struct TrafficStats {
    pub cars: usize,
    /// Cars drawn with a fleet model (the rest are boxes)
    pub fleet: usize,
    /// Real car lights (head spots + underglows)
    pub lights: usize,
    #[serde(skip)]
    logged: u32,
}

#[derive(Component)]
pub struct TrafficCar;

pub fn load(path: &std::path::Path) -> Option<TrafficFile> {
    let t = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&t).map_err(|e| warn!("{}: {e}", path.display())).ok()
}

/// A kind's boxes in its own frame (city axes: x along from the centre, y across, z up), per material slot:
/// 0 body, 1 glass, 2 head lights, 3 tail lights, 4 roof sign / stripe.
fn kind_boxes(k: Kind) -> Vec<(usize, Vec3, Vec3)> {
    let (l, w, h) = k.dims();
    let (hl, hw) = (l / 2.0, w / 2.0);
    let b = |s: usize, x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32| (s, Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1));
    let mut out = vec![];
    match k {
        Kind::Car | Kind::Taxi => {
            out.push(b(0, -hl, -hw, 0.25, hl, hw, 0.85));
            out.push(b(0, -hl * 0.55, -hw + 0.08, 0.85, hl * 0.35, hw - 0.08, h));
            out.push(b(1, -hl * 0.5, -hw + 0.06, 0.95, hl * 0.3, hw - 0.06, h - 0.08));
            if k == Kind::Taxi {
                out.push(b(4, -0.15, -0.3, h, 0.15, 0.3, h + 0.15));
            }
        }
        Kind::Bike => {
            out.push(b(0, -hl, -hw * 0.4, 0.3, hl, hw * 0.4, 0.9));
            out.push(b(4, -hl * 0.9, -hw * 0.5, 0.9, -hl * 0.4, hw * 0.5, 1.3));
        }
        Kind::Van => {
            out.push(b(0, -hl, -hw, 0.3, hl, hw, h));
            out.push(b(1, hl - 0.9, -hw + 0.05, 1.1, hl + 0.01, hw - 0.05, h - 0.3));
        }
        Kind::Minibus => {
            out.push(b(0, -hl, -hw, 0.35, hl, hw, h));
            out.push(b(1, -hl + 0.5, -hw - 0.01, 1.3, hl - 0.4, hw + 0.01, h - 0.4));
            out.push(b(4, -hl, -hw - 0.02, 0.9, hl, hw + 0.02, 1.05));
        }
        Kind::Bus => {
            out.push(b(0, -hl, -hw, 0.35, hl, hw, h));
            out.push(b(1, -hl + 0.6, -hw - 0.01, 1.4, hl - 0.4, hw + 0.01, 2.2));
            out.push(b(1, -hl + 0.3, -hw - 0.01, 2.7, hl - 0.2, hw + 0.01, 3.7));
            out.push(b(4, -hl, -hw - 0.02, 2.3, hl, hw + 0.02, 2.55));
        }
    }
    out.push(b(2, hl, -hw + 0.15, 0.55, hl + 0.03, -hw + 0.45, 0.75));
    out.push(b(2, hl, hw - 0.45, 0.55, hl + 0.03, hw - 0.15, 0.75));
    out.push(b(3, -hl - 0.03, -hw + 0.15, 0.6, -hl, -hw + 0.4, 0.8));
    out.push(b(3, -hl - 0.03, hw - 0.4, 0.6, -hl, hw - 0.15, 0.8));
    out
}

fn setup(
    mut commands: Commands,
    street: Option<Res<Street>>,
    traffic: Option<Res<Traffic>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut done: Local<bool>,
) {
    if *done || traffic.is_some() {
        return;
    }
    let Some(st) = street else { return };
    *done = true;
    if std::env::var("CV_NO_TRAFFIC").is_ok() {
        return;
    }
    let Some(f) = load(&st.dir.join("traffic.json")) else { return };
    let sim = Sim::new(&f, 0x5eed);
    info!("traffic: {} lanes, {} connectors", f.lanes.len(), f.conns.len());
    let plain = |c: Color, r: f32| StandardMaterial { base_color: c, perceptual_roughness: r, ..default() };
    // low emissive scales: AgX turns a brighter emissive white (s30)
    let lit = |c: &str, k: f32| StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::from(hex(c)) * k, ..default() };
    let glass = mats.add(StandardMaterial { base_color: Color::srgb(0.04, 0.05, 0.06), perceptual_roughness: 0.15, ..default() });
    let head = mats.add(lit("#FFF4E0", 6.0));
    let tail = mats.add(lit("#FF2020", 3.0));
    let bus_win = mats.add(lit("#FFC070", 0.35));
    let mut out = HashMap::new();
    for k in [Kind::Car, Kind::Taxi, Kind::Van, Kind::Minibus, Kind::Bus, Kind::Bike] {
        let (body, extra) = match k {
            Kind::Car => (plain(Color::srgb(0.32, 0.33, 0.35), 0.35), plain(Color::WHITE, 0.5)),
            Kind::Taxi => (plain(hex("#B0101E"), 0.35), lit("#FFFFFF", 1.2)),
            Kind::Van => (plain(Color::srgb(0.75, 0.75, 0.73), 0.5), plain(Color::WHITE, 0.5)),
            Kind::Minibus => (plain(hex("#E8E2C8"), 0.45), plain(hex("#1E6B3A"), 0.5)),
            Kind::Bus => (plain(hex("#B71C1C"), 0.45), plain(hex("#E8E2C8"), 0.5)),
            Kind::Bike => (plain(Color::srgb(0.2, 0.2, 0.22), 0.5), plain(hex("#FF2E88"), 0.5)),
        };
        let (body, extra) = (mats.add(body), mats.add(extra));
        let boxes = kind_boxes(k);
        let mut parts = vec![];
        for slot in 0..5 {
            let mut acc = Acc::default();
            for (s, a, b) in &boxes {
                if *s == slot {
                    acc.cuboid(*a, *b);
                }
            }
            let Some((m, aabb)) = acc.mesh() else { continue };
            let mat = match slot {
                0 => body.clone(),
                1 if k == Kind::Bus => bus_win.clone(),
                1 => glass.clone(),
                2 => head.clone(),
                3 => tail.clone(),
                _ => extra.clone(),
            };
            parts.push((meshes.add(m), mat, aabb));
        }
        out.insert(k, parts);
    }
    commands.insert_resource(Traffic { sim, entities: HashMap::new(), meshes: out });
}

#[allow(clippy::too_many_arguments)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    street: Option<Res<Street>>,
    traffic: Option<ResMut<Traffic>>,
    fleet: Option<Res<Fleet>>,
    focus: Option<Res<crate::frame::Focus>>,
    mut stats: ResMut<TrafficStats>,
    mut q: Query<&mut Transform, (With<TrafficCar>, Without<TrafficWheel>)>,
    mut wq: Query<&mut Transform, (With<TrafficWheel>, Without<TrafficCar>)>,
) {
    let (Some(st), Some(mut tr), Some(fleet)) = (street, traffic, fleet) else { return };
    // the fleet first: a car spawned before it is in would stay a box until it leaves the ring
    if !fleet.ready {
        return;
    }
    let half = (st.ring as f32 + 0.5) * st.chunk_m;
    tr.sim.set_ring(st.focus, half);
    // fixed sub-steps (a long frame does not tunnel a car through the one ahead)
    let dt_frame = time.delta_secs().min(0.25);
    let mut t = dt_frame;
    while t > 1e-4 {
        let dt = t.min(0.05);
        tr.sim.step(dt);
        t -= dt;
    }
    let tr = &mut *tr;
    let live: std::collections::HashSet<u64> = tr.sim.cars.iter().map(|c| c.id).collect();
    tr.entities.retain(|id, e| {
        let keep = live.contains(id);
        if !keep {
            commands.entity(e.root).despawn();
        }
        keep
    });
    let now = time.elapsed_secs();
    for c in &tr.sim.cars {
        let (p, h, _, _) = tr.sim.pose(c);
        let mut tf = Transform::from_translation(to_bevy(p)).with_rotation(Quat::from_rotation_y(h));
        if let Some(e) = tr.entities.get_mut(&c.id) {
            if e.hover > 0.0 {
                // a slow bob, out of phase per car
                tf.translation.y += e.hover + 0.04 * (now * 1.7 + (c.id % 97) as f32).sin();
            }
            if let Ok(mut t) = q.get_mut(e.root) {
                *t = tf;
            }
            if !e.wheels.is_empty() && dt_frame > 0.0 {
                let r = e.wheels[0].2.max(0.1);
                e.roll = (e.roll + c.v * dt_frame / r) % std::f32::consts::TAU;
                let mut dh = h - e.heading;
                dh = (dh + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                // bicycle model: tan(steer) = wheelbase * yaw rate / v
                let steer = if c.v > 0.5 { (e.wheelbase * dh / dt_frame / c.v).atan().clamp(-0.6, 0.6) } else { 0.0 };
                for (we, base, _, front) in &e.wheels {
                    if let Ok(mut wt) = wq.get_mut(*we) {
                        // the axle is the pivot's local X (glTF X); the steer turns about the car's up (+Y)
                        let st = if *front { Quat::from_rotation_y(steer) } else { Quat::IDENTITY };
                        *wt = Transform { translation: base.translation, rotation: st * base.rotation * Quat::from_rotation_x(-e.roll), scale: base.scale };
                    }
                }
            }
            e.heading = h;
            if dt_frame > 0.0 {
                let braking = (e.last_v - c.v) / dt_frame > BRAKE_DEC || c.v < BRAKE_V;
                if braking != e.braking {
                    e.braking = braking;
                    for (te, normal, brake) in &e.tails {
                        commands.entity(*te).insert(MeshMaterial3d(if braking { brake.clone() } else { normal.clone() }));
                    }
                }
                e.last_v = c.v;
            }
            continue;
        }
        let model = fleet.pick(c.kind.name(), c.id);
        let root = commands.spawn((tf, Visibility::Inherited, TrafficCar, Name::new(format!("{}_{}", c.kind.name(), c.id)))).id();
        let mut ent = CarEnt { root, model, wheels: vec![], hover: 0.0, roll: 0.0, heading: h, wheelbase: c.kind.dims().0 * 0.6, tails: vec![], braking: false, last_v: c.v, lights: vec![] };
        if let Some(mi) = model {
            let m = &fleet.models[mi];
            ent.hover = m.rec.hover;
            let (mut fz, mut rz) = (f32::MIN, f32::MAX);
            for (part, e) in m.parts.iter().zip(m.spawn_parts(&mut commands, root)) {
                if let Some(b) = fleet.brake.get(&part.mat.id()) {
                    ent.tails.push((e, part.mat.clone(), b.clone()));
                }
                if let Role::Wheel { front } = part.role {
                    commands.entity(e).insert(TrafficWheel);
                    ent.wheels.push((e, part.tf, part.radius, front));
                    fz = fz.max(part.tf.translation.x);
                    rz = rz.min(part.tf.translation.x);
                }
            }
            if fz > rz {
                ent.wheelbase = fz - rz;
            }
        } else {
            for (m, mat, aabb) in &tr.meshes[&c.kind] {
                commands.spawn((Mesh3d(m.clone()), MeshMaterial3d(mat.clone()), *aabb, Transform::IDENTITY, ChildOf(root)));
            }
        }
        tr.entities.insert(c.id, ent);
    }
    // real lights for the cars nearest the view's focus (not the ring's chunk centre)
    let focus = focus.map(|f| crate::frame::from_bevy(f.at).truncate()).unwrap_or(tr.sim.centre);
    let mut ranked: Vec<(f32, u64)> = tr.sim.cars.iter().map(|c| (tr.sim.pose(c).0.truncate().distance(focus), c.id)).collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let rank: HashMap<u64, usize> = ranked.iter().enumerate().map(|(i, (_, id))| (*id, i)).collect();
    let mut lit = 0;
    for (id, e) in tr.entities.iter_mut() {
        let r = rank.get(id).copied().unwrap_or(usize::MAX);
        if !e.lights.is_empty() && r >= HEAD_KEEP {
            for l in e.lights.drain(..) {
                commands.entity(l).despawn();
            }
        }
        lit += !e.lights.is_empty() as usize;
    }
    for (_, id) in ranked.iter().take(HEAD_N) {
        // only a car that will get a light competes (a model without head / underglow sockets never does)
        let wants = tr.entities.get(id).and_then(|e| e.lights.is_empty().then_some(e.model).flatten()).is_some_and(|mi| {
            let m = &fleet.models[mi];
            m.head.is_some() || (m.under.is_some() && m.under_color.is_some())
        });
        if !wants {
            continue;
        }
        // at the cap a car in the top HEAD_N takes the lights of the worst-ranked holder (a kept one, rank >= HEAD_N)
        if lit >= HEAD_N + 2 {
            let worst = tr.entities.iter().filter(|(_, e)| !e.lights.is_empty()).max_by_key(|(i, _)| rank.get(*i).copied().unwrap_or(usize::MAX)).map(|(i, _)| *i);
            let Some(w) = worst.filter(|w| rank.get(w).copied().unwrap_or(usize::MAX) >= HEAD_N) else { break };
            for l in tr.entities.get_mut(&w).unwrap().lights.drain(..) {
                commands.entity(l).despawn();
            }
            lit -= 1;
        }
        let Some(e) = tr.entities.get_mut(id) else { continue };
        let Some(m) = e.model.map(|i| &fleet.models[i]) else { continue };
        lit += 1; // (wants: at least one light below)
        if let Some(hp) = m.head {
            let at = hp + Vec3::X * 0.1;
            let (outer, inner) = (HEAD_CONE.0.to_radians(), HEAD_CONE.1.to_radians());
            let colour = Color::srgb(1.0, 0.93, 0.8);
            let lm = HEAD_W * LM_PER_W;
            let l = commands
                .spawn((
                    SpotLight { color: colour, intensity: lm, range: HEAD_RANGE, radius: 0.1, outer_angle: outer, inner_angle: inner, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(at).looking_at(at + Vec3::new(10.0, -2.0, 0.0), Vec3::Y),
                    head_gl(&format!("car_head_{id}"), colour, lm, HEAD_RANGE, Some((inner, outer)), HEAD_W),
                    MovingLight,
                    ChildOf(e.root),
                ))
                .id();
            e.lights.push(l);
        }
        if let (Some(up), Some(col)) = (m.under, m.under_color) {
            let lm = UNDER_W * LM_PER_W;
            let l = commands
                .spawn((
                    PointLight { color: col, intensity: lm, range: 4.0, radius: 0.3, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(up),
                    head_gl(&format!("car_under_{id}"), col, lm, 4.0, None, UNDER_W),
                    MovingLight,
                    ChildOf(e.root),
                ))
                .id();
            e.lights.push(l);
        }
    }
    stats.lights = tr.entities.values().map(|e| e.lights.len()).sum();
    stats.cars = tr.sim.cars.len();
    stats.fleet = tr.entities.values().filter(|e| e.model.is_some()).count();
    // a line every 10 s (the benches' logs)
    let bucket = time.elapsed_secs() as u32 / 10 + 1;
    if bucket != stats.logged {
        stats.logged = bucket;
        info!("traffic: {} cars ({} fleet models, {} lights) around ({:.0}, {:.0}), {} spawned, {} junction crossings", stats.cars, stats.fleet, stats.lights, tr.sim.centre.x, tr.sim.centre.y, tr.sim.spawned, tr.sim.crossed);
    }
}

/// A car light's GlLight: unshadowed; with `MovingLight` never a shadow candidate (the allocator's budget is the
/// buildings' and lamps').
fn head_gl(name: &str, color: Color, lm: f32, range: f32, spot: Option<(f32, f32)>, w: f32) -> GlLight {
    GlLight {
        bldg: usize::MAX,
        name: name.into(),
        role: LightRole::Unshadowed,
        export_shadow: false,
        floor: 0,
        interior_floor: -1,
        energy_w: w,
        color,
        intensity: lm,
        range,
        radius: 0.1,
        spot,
        priority: 0.0,
        down: false,
        cone: None,
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<TrafficStats>().add_systems(Update, (setup, run).chain().after(crate::street::update_street).before(crate::lighting::apply_lights));
}

// ------------------------------------------------------------------------------------------------ trace

/// Run the sim headless around `centre` for `secs` (dt 0.05) and write a trace for checks_traffic: every
/// second the cars' footprints (x, y, heading, length, width, kind, segment, speed) and the counters.
pub fn trace(path_in: &std::path::Path, out: &std::path::Path, centre: Vec2, half: f32, secs: f32, faults: Faults) -> std::io::Result<()> {
    let f = load(path_in).ok_or_else(|| std::io::Error::other("no traffic.json"))?;
    let mut sim = Sim::new(&f, 0x5eed);
    sim.faults = faults;
    sim.set_ring(centre, half);
    let mut frames = vec![];
    let dt = 0.05;
    let steps_per_s = (1.0 / dt) as usize;
    let mut stuck_max: f32 = 0.0;
    for s in 0..(secs as usize) {
        for _ in 0..steps_per_s {
            sim.step(dt);
        }
        let cars: Vec<serde_json::Value> = sim
            .cars
            .iter()
            .map(|c| {
                let (p, h, l, w) = sim.pose(c);
                stuck_max = stuck_max.max(c.stopped);
                serde_json::json!([p.x, p.y, h, l, w, c.kind.name(), c.seg, c.v, c.stopped, c.id])
            })
            .collect();
        // the junction state (debugging a jam): connector -> holders, connector -> the waiting car it is wanted by
        let held: Vec<serde_json::Value> = sim.held.iter().enumerate().filter(|(_, h)| !h.is_empty())
            .map(|(k, h)| serde_json::json!([k - sim.lanes, h.iter().map(|x| x.0).collect::<Vec<_>>()])).collect();
        let want: Vec<serde_json::Value> = sim.want.iter().enumerate().filter_map(|(k, w)| w.map(|w| serde_json::json!([k - sim.lanes, w.0]))).collect();
        frames.push(serde_json::json!({"t": s + 1, "cars": cars, "held": held, "want": want}));
    }
    let doc = serde_json::json!({
        "schema": "city-traffic-trace/0.1",
        "centre": [centre.x, centre.y], "half": half, "secs": secs,
        "lanes": sim.lanes,
        "spawned": sim.spawned, "gone": sim.gone, "crossed": sim.crossed,
        "frames": frames,
    });
    std::fs::write(out, serde_json::to_string(&doc).unwrap())
}
