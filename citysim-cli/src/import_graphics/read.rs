use super::*;

// ---------------------------------------------------------------------------
// JSON access with messages that name the file, the record and the field.
// ---------------------------------------------------------------------------

pub(super) fn read_json(dir: &Path, name: &str) -> Result<Value, String> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", path.display()))
}

pub(super) fn get<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("{ctx}: missing field \"{key}\""))
}

pub(super) fn arr<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a Vec<Value>, String> {
    get(v, key, ctx)?.as_array().ok_or_else(|| format!("{ctx}: \"{key}\" is not a list"))
}

pub(super) fn text<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a str, String> {
    get(v, key, ctx)?.as_str().ok_or_else(|| format!("{ctx}: \"{key}\" is not a string"))
}

pub(super) fn num(v: &Value, ctx: &str) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("{ctx}: {v} is not a number"))
}

/// `[x0, y0, x1, y1]` metres.
pub(super) fn rect_m(v: &Value, ctx: &str) -> Result<[f64; 4], String> {
    let a = v.as_array().filter(|a| a.len() == 4).ok_or_else(|| format!("{ctx}: not a [x0, y0, x1, y1] rect"))?;
    let r = [num(&a[0], ctx)?, num(&a[1], ctx)?, num(&a[2], ctx)?, num(&a[3], ctx)?];
    if !(r[0] <= r[2] && r[1] <= r[3]) {
        return Err(format!("{ctx}: rect {r:?} has x0 > x1 or y0 > y1"));
    }
    Ok(r)
}

pub(super) fn field_rect(v: &Value, key: &str, ctx: &str) -> Result<[f64; 4], String> {
    rect_m(get(v, key, ctx)?, &format!("{ctx} \"{key}\""))
}

/// `[[x, y, ...], ...]` points.
pub(super) fn points(v: &Value, key: &str, ctx: &str) -> Result<Vec<(f64, f64)>, String> {
    arr(v, key, ctx)?
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let c = format!("{ctx} \"{key}\"[{i}]");
            let a = p.as_array().filter(|a| a.len() >= 2).ok_or_else(|| format!("{c}: not a point"))?;
            Ok((num(&a[0], &c)?, num(&a[1], &c)?))
        })
        .collect()
}

/// A whole number field.
pub(super) fn uint(v: &Value, key: &str, ctx: &str) -> Result<u64, String> {
    get(v, key, ctx)?.as_u64().ok_or_else(|| format!("{ctx}: \"{key}\" is not a whole number"))
}

/// A `[x, y, ...]` point field.
pub(super) fn point(v: &Value, key: &str, ctx: &str) -> Result<(f64, f64), String> {
    let c = format!("{ctx} \"{key}\"");
    let a = get(v, key, ctx)?.as_array().filter(|a| a.len() >= 2).ok_or_else(|| format!("{c}: not a point"))?;
    Ok((num(&a[0], &c)?, num(&a[1], &c)?))
}

/// Warn when a sim export file's `schema` is not the one this adapter reads.
pub(super) fn check_schema(v: &Value, file: &str, rep: &mut Report) -> Result<(), String> {
    let s = text(v, "schema", file)?;
    if s != SIM_SCHEMA {
        rep.warnings.push(format!("{file}: schema {s:?}, this adapter reads {SIM_SCHEMA:?}; reading it anyway"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The sim export (`<src>/sim/`, S3c).
// ---------------------------------------------------------------------------

/// A lot's `use` in `lots_sim.json`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Use {
    ShopHouse,
    Commercial,
    Residential,
    Factory,
    VatFarm,
    Godown,
    Civic,
    Vacant,
}

impl Use {
    fn parse(s: &str) -> Option<Use> {
        Some(match s {
            "shop_house" => Use::ShopHouse,
            "commercial" => Use::Commercial,
            "residential" => Use::Residential,
            "factory" => Use::Factory,
            "vat_farm" => Use::VatFarm,
            "godown" => Use::Godown,
            "civic" => Use::Civic,
            "vacant" => Use::Vacant,
            _ => return None,
        })
    }

    pub(super) fn industrial(self) -> bool {
        matches!(self, Use::Factory | Use::VatFarm | Use::Godown)
    }
}

/// One lot's record in `sim/lots_sim.json`.
#[derive(Clone, Debug)]
pub(super) struct SimLot {
    pub(super) floors: u32,
    pub(super) use_: Use,
    /// `ground == "workshop"` (an alley-only lot's ground floor).
    pub(super) workshop: bool,
    /// C5: `ground == "works"` (an industrial lot's works floor: one site).
    pub(super) works: bool,
    pub(super) flats: u32,
    pub(super) shops: usize,
    /// Each door's `out` point: the centre of the road tile outside it.
    pub(super) doors_out: Vec<(f64, f64)>,
    pub(super) layout: bool,
    pub(super) civic: Option<String>,
    /// C5 (`city-sim/0.2`): the walking surface at the lot (metres over
    /// the sea; 0 in a 0.1 export), its levels and its cores. Carried for
    /// C6a (tenancies by floor) and C7 (climbs); the map does not hold them yet.
    #[allow(dead_code)]
    z_m: f64,
    pub(super) levels: Vec<SimLevel>,
    pub(super) cores: Vec<SimCore>,
}

/// C5: one of a lot's `levels` (SIM_NOTES "The sim export", 0.2).
#[derive(Clone, Debug)]
#[allow(dead_code)] // carried for C6a / C7: z, height, area and use are read by nothing yet
pub(super) struct SimLevel {
    pub(super) i: u32,
    z: f64,
    h: f64,
    area_m2: f64,
    pub(super) use_: String,
    /// Flats, shops, workshops, offices.
    pub(super) units: [u32; 4],
    /// A tower's plate floor.
    pub(super) plate: bool,
}

/// C5: one of a lot's `cores`: a `flight` (stairs) or `lift` chain.
#[derive(Clone, Debug)]
#[allow(dead_code)] // carried for C7's climbs: floors, stops and roof are read by nothing yet
pub(super) struct SimCore {
    pub(super) lift: bool,
    pub(super) floors: (u32, u32),
    stops: u32,
    roof: bool,
}

/// C5: a lot's `levels` and `cores` (absent in a 0.1 export: empty).
fn read_levels(r: &Value, ctx: &str) -> Result<(Vec<SimLevel>, Vec<SimCore>), String> {
    let mut levels = Vec::new();
    if let Some(ls) = r.get("levels") {
        let ls = ls.as_array().ok_or_else(|| format!("{ctx}: \"levels\" is not a list"))?;
        for (j, l) in ls.iter().enumerate() {
            let c = format!("{ctx} levels[{j}]");
            let units = get(l, "units", &c)?;
            let unit =
                |k: &str| units.get(k).and_then(Value::as_u64).map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX));
            levels.push(SimLevel {
                i: u32::try_from(uint(l, "i", &c)?).unwrap_or(u32::MAX),
                z: num(get(l, "z", &c)?, &format!("{c} \"z\""))?,
                h: num(get(l, "h", &c)?, &format!("{c} \"h\""))?,
                area_m2: num(get(l, "area_m2", &c)?, &format!("{c} \"area_m2\""))?,
                use_: text(l, "use", &c)?.to_string(),
                units: [unit("flats"), unit("shops"), unit("workshops"), unit("offices")],
                plate: l.get("plate").and_then(Value::as_bool).unwrap_or(false),
            });
        }
    }
    let mut cores = Vec::new();
    if let Some(cs) = r.get("cores") {
        let cs = cs.as_array().ok_or_else(|| format!("{ctx}: \"cores\" is not a list"))?;
        for (j, k) in cs.iter().enumerate() {
            let c = format!("{ctx} cores[{j}]");
            let kind = text(k, "kind", &c)?;
            if kind != "flight" && kind != "lift" {
                return Err(format!("{c}: kind {kind:?} is not flight or lift"));
            }
            let fl = arr(k, "floors", &c)?;
            if fl.len() != 2 {
                return Err(format!("{c}: \"floors\" is not [lo, hi]"));
            }
            let f = |v: &Value| num(v, &c).map(|x| x.max(0.0) as u32);
            cores.push(SimCore {
                lift: kind == "lift",
                floors: (f(&fl[0])?, f(&fl[1])?),
                stops: u32::try_from(uint(k, "stops", &c)?).unwrap_or(u32::MAX),
                roof: k.get("roof").and_then(Value::as_bool).unwrap_or(false),
            });
        }
    }
    Ok((levels, cores))
}

/// C7: one end of a `walkways_sim.json` portal.
#[derive(Clone, Debug)]
struct RawEnd {
    kind: String,
    id: String,
    at: (f64, f64),
    level: u8,
}

/// C7: one `walkways_sim.json` portal as read.
#[derive(Clone, Debug)]
pub(super) struct RawPortal {
    kind: String,
    a: RawEnd,
    b: RawEnd,
    length_m: f64,
    vehicle: bool,
    /// `(from hour, to hour, a to b)`.
    hours: Vec<(u8, u8, bool)>,
}

/// C5: `sim/walkways_sim.json`'s portals by kind and their summed length
/// (each end's `kind`, `at` and `level` are checked); C7: the portals read,
/// for the map's `P` lines (`map_portals`).
pub(super) fn read_walkways(dir: &Path, rep: &mut Report) -> Result<Vec<RawPortal>, String> {
    const F: &str = "sim/walkways_sim.json";
    let v = read_json(dir, "walkways_sim.json")?;
    check_schema(&v, F, rep)?;
    let ps = arr(&v, "portals", F)?;
    let mut out = Vec::with_capacity(ps.len());
    for (i, p) in ps.iter().enumerate() {
        let ctx = format!("{F} portals[{i}]");
        let kind = text(p, "kind", &ctx)?;
        text(p, "id", &ctx)?;
        let mut ends = Vec::with_capacity(2);
        for end in ["a", "b"] {
            let e = get(p, end, &ctx)?;
            let c = format!("{ctx} {end}");
            let ek = text(e, "kind", &c)?.to_string();
            let level = u8::try_from(uint(e, "level", &c)?).unwrap_or(u8::MAX);
            let at = arr(e, "at", &c)?;
            if at.len() != 3 {
                return Err(format!("{c}: \"at\" is not [x, y, z]"));
            }
            let id = e.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            ends.push(RawEnd { kind: ek, id, at: (num(&at[0], &c)?, num(&at[1], &c)?), level });
        }
        let length_m = num(get(p, "length_m", &ctx)?, &format!("{ctx} \"length_m\""))?;
        rep.portal_m += length_m;
        *rep.portals.entry(kind.to_string()).or_default() += 1;
        let mut hours = Vec::new();
        if let Some(hs) = p.get("hours").and_then(Value::as_array) {
            for h in hs {
                let w = h
                    .as_array()
                    .filter(|w| w.len() == 3)
                    .ok_or_else(|| format!("{ctx}: an hours window is not [h0, h1, dir]"))?;
                let (h0, h1) = (num(&w[0], &ctx)?, num(&w[1], &ctx)?);
                let ab = match w[2].as_str() {
                    Some("ab") => true,
                    Some("ba") => false,
                    _ => return Err(format!("{ctx}: an hours window's direction is not ab or ba")),
                };
                hours.push((h0.round().clamp(0.0, 24.0) as u8, h1.round().clamp(0.0, 24.0) as u8, ab));
            }
        }
        let b = ends.pop().ok_or_else(|| format!("{ctx}: no end b"))?;
        let a = ends.pop().ok_or_else(|| format!("{ctx}: no end a"))?;
        out.push(RawPortal {
            kind: kind.to_string(),
            a,
            b,
            length_m,
            vehicle: p.get("vehicle").and_then(Value::as_bool).unwrap_or(false),
            hours,
        });
    }
    Ok(out)
}

/// C7: a deck segment longer than this many tiles is cut into pieces of at
/// most this many (deck nodes between), so a walker's hop along it is a
/// step of a few tiles like any other (the escalator's 378 m is 24 hops).
pub(super) const PIECE_TILES: u16 = 4;

/// C7: the nearest walkable tile (`. = D m`) to `t` within `r` tiles (the
/// lowest index on a tie), `None` past it.
fn snap_walkable(tiles: &[char], w: usize, h: usize, t: (usize, usize), r: usize) -> Option<(usize, usize)> {
    let walk = |x: usize, y: usize| matches!(tiles[y * w + x], '.' | '=' | 'D' | 'm');
    let mut best: Option<(usize, usize, usize)> = None;
    for y in t.1.saturating_sub(r)..=(t.1 + r).min(h - 1) {
        for x in t.0.saturating_sub(r)..=(t.0 + r).min(w - 1) {
            if walk(x, y) {
                let d = x.abs_diff(t.0) + y.abs_diff(t.1);
                if d <= r && best.is_none_or(|(bd, by, bx)| (d, y, x) < (bd, by, bx)) {
                    best = Some((d, y, x));
                }
            }
        }
    }
    best.map(|(_, y, x)| (x, y))
}

/// C7: `walkways_sim.json`'s portals as the map's `P` lines: an end on a way
/// (or a deck at level 0) the walkable tile at or nearest its point; an end
/// on a lot (a landing, its floor's level) the street outside the lot's
/// door, at level 0, the walk to it added to the run (the climb down through
/// the lot is the sim's, by the levels); a deck's or a station's end the
/// tile under its point at its level. The vehicle ramps are not mapped; a
/// deck-to-deck segment past [`PIECE_TILES`] is cut into pieces.
pub(super) fn map_portals(
    f: &Frame,
    tiles: &[char],
    raw: &[RawPortal],
    sim_lots: Option<&BTreeMap<String, SimLot>>,
    rep: &mut Report,
) -> Vec<citysim::map::MapPortal> {
    use citysim::map::{MapPortal, PortalKind};
    let (w, h) = (f.w, f.h);
    let mut out: Vec<MapPortal> = Vec::new();
    let skip = |rep: &mut Report, why: &str| *rep.portals_skipped.entry(why.to_string()).or_default() += 1;
    for p in raw {
        if p.vehicle {
            skip(rep, "vehicle ramp");
            continue;
        }
        let Some(kind) = PortalKind::parse(&p.kind) else {
            skip(rep, &format!("kind {}", p.kind));
            continue;
        };
        // An end: its tile, its level, the extra run to it (tiles).
        let end = |e: &RawEnd| -> Option<((usize, usize), u8, u16)> {
            let t = f.tile(e.at.0, e.at.1)?;
            match e.kind.as_str() {
                "lot" => {
                    let rec = sim_lots.and_then(|s| s.get(&e.id));
                    let door = rec
                        .and_then(|r| {
                            r.doors_out
                                .iter()
                                .filter_map(|&(x, y)| f.tile(x, y))
                                .min_by_key(|d| d.0.abs_diff(t.0) + d.1.abs_diff(t.1))
                        })
                        .and_then(|d| snap_walkable(tiles, w, h, d, 2))
                        .or_else(|| snap_walkable(tiles, w, h, t, 6))?;
                    let run = u16::try_from(door.0.abs_diff(t.0) + door.1.abs_diff(t.1)).unwrap_or(u16::MAX);
                    Some((door, 0, run))
                }
                _ if e.kind == "way" || e.level == 0 => snap_walkable(tiles, w, h, t, 3).map(|s| (s, 0, 0)),
                _ => Some((t, e.level, 0)),
            }
        };
        let (Some(a), Some(b)) = (end(&p.a), end(&p.b)) else {
            skip(rep, &format!("{} end off the map or no street near", p.kind));
            continue;
        };
        if a.0 == b.0 && a.1 == b.1 {
            skip(rep, &format!("{} ends on one tile", p.kind));
            continue;
        }
        *rep.portals_mapped.entry(p.kind.clone()).or_default() += 1;
        let run = ((p.length_m / f.s).ceil() as u16).saturating_add(a.2).saturating_add(b.2).max(1);
        let tp = |t: (usize, usize)| citysim::TilePos::new(t.0 as u16, t.1 as u16);
        let deck = a.1 > 0 && b.1 > 0;
        let n = if deck { run.div_ceil(PIECE_TILES).max(1) } else { 1 };
        let mut prev = (tp(a.0), a.1);
        let mut left = run;
        for k in 1..=n {
            let u = f64::from(k) / f64::from(n);
            let next = if k == n {
                (tp(b.0), b.1)
            } else {
                let x = a.0 .0 as f64 + (b.0 .0 as f64 - a.0 .0 as f64) * u;
                let y = a.0 .1 as f64 + (b.0 .1 as f64 - a.0 .1 as f64) * u;
                let l = f64::from(a.1) + (f64::from(b.1) - f64::from(a.1)) * u;
                (citysim::TilePos::new(x.round() as u16, y.round() as u16), l.round().max(1.0) as u8)
            };
            let piece = if k == n { left } else { (run / n).max(1) };
            left = left.saturating_sub(piece);
            if next != prev {
                out.push(MapPortal { kind, a: prev, b: next, tiles: piece.max(1), hours: p.hours.clone() });
            }
            prev = next;
        }
    }
    rep.portal_lines = out.len();
    out
}

/// `sim/lots_sim.json`'s lots by id.
pub(super) fn read_sim_lots(dir: &Path, rep: &mut Report) -> Result<BTreeMap<String, SimLot>, String> {
    const F: &str = "sim/lots_sim.json";
    let v = read_json(dir, "lots_sim.json")?;
    check_schema(&v, F, rep)?;
    rep.export_seed = v.get("seed").and_then(Value::as_u64);
    let lots = get(&v, "lots", F)?.as_object().ok_or_else(|| format!("{F}: \"lots\" is not an object"))?;
    let mut out = BTreeMap::new();
    let mut odd: BTreeMap<String, usize> = BTreeMap::new();
    for (id, r) in lots {
        let ctx = format!("{F} lots.{id}");
        let u = text(r, "use", &ctx)?;
        let use_ = Use::parse(u).unwrap_or_else(|| {
            *odd.entry(format!("use {u:?} (read as vacant)")).or_default() += 1;
            Use::Vacant
        });
        let ground = text(r, "ground", &ctx)?;
        if !matches!(ground, "shop" | "workshop" | "lobby" | "works" | "civic" | "none") {
            *odd.entry(format!("ground {ground:?}")).or_default() += 1;
        }
        let source = text(r, "source", &ctx)?;
        if !matches!(source, "layout" | "estimate") {
            *odd.entry(format!("source {source:?}")).or_default() += 1;
        }
        let mut doors_out = Vec::new();
        for (j, d) in arr(r, "doors_m", &ctx)?.iter().enumerate() {
            let c = format!("{ctx} doors_m[{j}]");
            point(d, "at", &c)?;
            doors_out.push(point(d, "out", &c)?);
        }
        let civic = match r.get("civic") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(format!("{ctx}: \"civic\" is not a string")),
        };
        let (levels, cores) = read_levels(r, &ctx)?;
        let z_m = match r.get("z_m") {
            None => 0.0,
            Some(z) => num(z, &format!("{ctx} \"z_m\""))?,
        };
        let rec = SimLot {
            z_m,
            levels,
            cores,
            floors: u32::try_from(uint(r, "floors", &ctx)?).unwrap_or(u32::MAX),
            use_,
            workshop: ground == "workshop",
            works: ground == "works",
            flats: u32::try_from(uint(r, "flats", &ctx)?).unwrap_or(u32::MAX),
            shops: usize::try_from(uint(r, "shop_units", &ctx)?).unwrap_or(usize::MAX),
            doors_out,
            layout: source == "layout",
            civic,
        };
        out.insert(id.clone(), rec);
    }
    for (what, n) in odd {
        rep.warnings.push(format!("{F}: {n} lot(s) with an unknown {what}"));
    }
    // C5: 0.2's `totals` against the levels and cores read.
    if let Some(t) = v.get("totals") {
        let n = |p: &[&str]| p.iter().try_fold(t, |v, k| v.get(*k)).and_then(Value::as_u64).unwrap_or(0);
        let sum = |f: &dyn Fn(&SimLot) -> u64| out.values().map(f).sum::<u64>();
        let unit = |k: usize| sum(&|l: &SimLot| l.levels.iter().map(|x| u64::from(x.units[k])).sum());
        let checks = [
            ("levels", n(&["levels"]), sum(&|l: &SimLot| l.levels.len() as u64)),
            ("units.flats", n(&["units", "flats"]), unit(0)),
            ("units.shops", n(&["units", "shops"]), unit(1)),
            ("units.workshops", n(&["units", "workshops"]), unit(2)),
            ("units.offices", n(&["units", "offices"]), unit(3)),
            (
                "cores.flight",
                n(&["cores", "flight"]),
                sum(&|l: &SimLot| l.cores.iter().filter(|c| !c.lift).count() as u64),
            ),
            ("cores.lift", n(&["cores", "lift"]), sum(&|l: &SimLot| l.cores.iter().filter(|c| c.lift).count() as u64)),
        ];
        for (what, want, got) in checks {
            if want != got {
                rep.warnings.push(format!("{F}: totals.{what} {want}, the lots hold {got}"));
            }
        }
    }
    Ok(out)
}
