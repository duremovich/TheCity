use super::*;

/// The chunk files' ground classes (`pipeline/city/chunks.py`): carriageway and median.
const GROUND_CARRIAGE: u8 = 4;
const GROUND_MEDIAN: u8 = 5;

/// V7 (OPEN-260): the vehicle ways' carriageways from the ground raster the
/// client draws (`out/chunks/<cx>_<cy>.ground.bin`, 2 m tiles), and the
/// zebras (`out/street/<cx>_<cy>.json` crossings of kind `zebra`; a `mouth`,
/// the side street's dropped kerb across the main road's sidewalk, is a
/// walk tile in the raster and costs nothing) as `X` lines, so what is
/// drawn is what is walked. A sim tile is carriageway when every source
/// tile under it is carriageway or median and each of its 4 neighbours is
/// a road tile (the outermost road tile stays a walk: a door never opens
/// onto the carriageway, and a narrow street's sidewalk folded into its lot
/// keeps a walk tile on the road). A zebra's tiles are the carriageway
/// tiles half or more of whose source tiles' centres lie in its rect; its
/// junction (the signal's key) is the centre of the overlap of the two ways
/// its id names (`A|B#z0`), else its own centre.
#[allow(clippy::too_many_arguments)]
pub(super) fn read_crossings(
    src: &Path,
    f: &Frame,
    k: u32,
    tile_src: f64,
    tiles: &[char],
    roads_json: &Value,
    rep: &mut Report,
) -> Result<(Vec<bool>, Vec<MapCrossing>), String> {
    let index = read_json(&src.join("chunks"), "index.json")?;
    let ctx = "chunks/index.json";
    let n = get(&index, "chunk_tiles", ctx)?.as_u64().ok_or_else(|| format!("{ctx}: chunk_tiles"))? as usize;
    let apron = get(&index, "apron", ctx)?.as_u64().ok_or_else(|| format!("{ctx}: apron"))? as usize;
    let gt = get(&index, "grid_tiles", ctx)?.as_array().ok_or_else(|| format!("{ctx}: grid_tiles"))?;
    let (gnx, gny) = (
        gt.first().and_then(|v| v.as_u64()).ok_or_else(|| format!("{ctx}: grid_tiles[0]"))? as usize,
        gt.get(1).and_then(|v| v.as_u64()).ok_or_else(|| format!("{ctx}: grid_tiles[1]"))? as usize,
    );
    let win = n + 2 * apron;
    // the source raster, row 0 the south
    let mut ground = vec![0u8; gnx * gny];
    let dir = src.join("chunks");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.ends_with(".ground.bin"))
        .collect();
    names.sort();
    for name in &names {
        let path = dir.join(name);
        let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let want = 16 + win * win * 8 + win * win;
        if b.len() != want || &b[..4] != b"CGND" {
            return Err(format!("{}: not a CGND v1 ground chunk of {win} x {win}", path.display()));
        }
        let cx = i32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        let cy = i32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        let cls = &b[16 + win * win * 8..];
        for j in apron..apron + n {
            for i in apron..apron + n {
                let gx = cx as i64 * n as i64 + (i - apron) as i64;
                let gy = cy as i64 * n as i64 + (j - apron) as i64;
                if gx >= 0 && gy >= 0 && (gx as usize) < gnx && (gy as usize) < gny {
                    ground[gy as usize * gnx + gx as usize] = cls[j * win + i];
                }
            }
        }
        rep.ground_chunks += 1;
    }
    let (w, h) = (f.w, f.h);
    let k = k as usize;
    // the source tiles under sim tile (x, y): column gx0.., row gy0.. (south up)
    let sub = |x: usize, y: usize| -> (i64, i64) {
        let gx0 = ((f.x0 + x as f64 * f.s) / tile_src).round() as i64;
        let gy0 = ((f.y1 - (y + 1) as f64 * f.s) / tile_src).round() as i64;
        (gx0, gy0)
    };
    let cls_at = |gx: i64, gy: i64| -> u8 {
        if gx < 0 || gy < 0 || gx as usize >= gnx || gy as usize >= gny {
            0
        } else {
            ground[gy as usize * gnx + gx as usize]
        }
    };
    let road = |x: usize, y: usize| tiles[y * w + x] == '=';
    let mut carriage = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            if !road(x, y) {
                continue;
            }
            let (gx0, gy0) = sub(x, y);
            let all = (0..k as i64).all(|dy| {
                (0..k as i64).all(|dx| matches!(cls_at(gx0 + dx, gy0 + dy), GROUND_CARRIAGE | GROUND_MEDIAN))
            });
            let ringed = x > 0
                && y > 0
                && x + 1 < w
                && y + 1 < h
                && road(x - 1, y)
                && road(x + 1, y)
                && road(x, y - 1)
                && road(x, y + 1);
            carriage[y * w + x] = all && ringed;
        }
    }
    rep.carriage_tiles = carriage.iter().filter(|&&c| c).count();
    // the ways' raw rects by id (the junctions)
    let mut way_rect: BTreeMap<String, [f64; 4]> = BTreeMap::new();
    for (i, wy) in arr(roads_json, "ways", "roads.json")?.iter().enumerate() {
        let c = format!("roads.json ways[{i}]");
        way_rect.insert(text(wy, "id", &c)?.to_string(), field_rect(wy, "rect_m", &c)?);
    }
    let sdir = src.join("street");
    let mut snames: Vec<String> = std::fs::read_dir(&sdir)
        .map_err(|e| format!("{}: {e}", sdir.display()))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| {
            n.strip_suffix(".json")
                .and_then(|b| b.split_once('_'))
                .is_some_and(|(a, b)| a.parse::<i32>().is_ok() && b.parse::<i32>().is_ok())
        })
        .collect();
    snames.sort();
    let mut seen: BTreeSet<(TilePos, TilePos)> = BTreeSet::new();
    let mut out: Vec<MapCrossing> = Vec::new();
    let mut read: BTreeSet<String> = BTreeSet::new();
    for name in &snames {
        let v = read_json(&sdir, name)?;
        let Some(list) = v.get("crossings").and_then(|c| c.as_array()) else { continue };
        for (i, c) in list.iter().enumerate() {
            let ctx = format!("street/{name} crossings[{i}]");
            if text(c, "kind", &ctx)? != "zebra" {
                continue;
            }
            let id = text(c, "id", &ctx)?.to_string();
            let r = field_rect(c, "rect", &ctx)?;
            let walk_x = text(c, "walk", &ctx)? == "x";
            if !read.insert(format!("{id}{r:?}")) {
                continue;
            }
            rep.zebras_read += 1;
            // the sim tiles the rect touches
            let (tx0, ty0, tx1, ty1) = f.round([r[0] - f.s, r[1] - f.s, r[2] + f.s, r[3] + f.s]);
            let (mut a, mut b) = (TilePos::new(u16::MAX, u16::MAX), TilePos::new(0, 0));
            for y in ty0..ty1 {
                for x in tx0..tx1 {
                    if !carriage[y * w + x] {
                        continue;
                    }
                    let (gx0, gy0) = sub(x, y);
                    let mut inside = 0;
                    for dy in 0..k as i64 {
                        for dx in 0..k as i64 {
                            let cx = (gx0 + dx) as f64 * tile_src + tile_src / 2.0;
                            let cy = (gy0 + dy) as f64 * tile_src + tile_src / 2.0;
                            if r[0] <= cx && cx <= r[2] && r[1] <= cy && cy <= r[3] {
                                inside += 1;
                            }
                        }
                    }
                    if inside * 2 >= k * k {
                        a = TilePos::new(a.x.min(x as u16), a.y.min(y as u16));
                        b = TilePos::new(b.x.max(x as u16), b.y.max(y as u16));
                    }
                }
            }
            if a.x > b.x {
                rep.zebras_dropped += 1;
                continue;
            }
            if !seen.insert((a, b)) {
                continue;
            }
            let ways = id.split('#').next().unwrap_or("");
            let sq = ways.split_once('|').and_then(|(wa, wb)| {
                let (ra, rb) = (way_rect.get(wa)?, way_rect.get(wb)?);
                Some([ra[0].max(rb[0]), ra[1].max(rb[1]), ra[2].min(rb[2]), ra[3].min(rb[3])])
            });
            let centre = match sq {
                Some(q) if q[0] <= q[2] && q[1] <= q[3] => ((q[0] + q[2]) / 2.0, (q[1] + q[3]) / 2.0),
                _ => ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0),
            };
            let junction = f.tile(centre.0, centre.1).map(|(x, y)| TilePos::new(x as u16, y as u16)).unwrap_or(a);
            out.push(MapCrossing { a, b, walk_x, junction });
        }
    }
    out.sort_by_key(|c| (c.a.y, c.a.x, c.b.y, c.b.x));
    rep.crossings = out.len();
    Ok((carriage, out))
}
