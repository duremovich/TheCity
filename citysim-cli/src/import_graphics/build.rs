use super::*;

/// The import itself: the map text and the report.
pub fn build(args: &ImportArgs) -> Result<(String, Report), String> {
    let src = &args.src;
    if !src.is_dir() {
        return Err(format!("--src {}: not a directory", src.display()));
    }
    let k = args.downsample.max(1);
    let mut rep = Report { residents: args.residents, ..Report::default() };

    // S3c: the sim export, file by file; the S3b inputs where one is absent.
    let sim_dir = src.join("sim");
    let sim_lots = if sim_dir.join("lots_sim.json").is_file() {
        let l = read_sim_lots(&sim_dir, &mut rep)?;
        rep.inputs.push(format!(
            "lots: sim/lots_sim.json ({} records, {} from layouts): floors, use, flats, shop units, doors, civic sites",
            l.len(),
            l.values().filter(|r| r.layout).count()
        ));
        Some(l)
    } else {
        rep.inputs.push(
            "lots: lots.json floor ranges and bldg/<lot>/sim_layout.toml (no sim/lots_sim.json: the S3b fallback)"
                .into(),
        );
        None
    };
    let terrain = if sim_dir.join("terrain_sim.json").is_file() {
        let t = Terrain::read(&sim_dir, &mut rep)?;
        rep.inputs.push(format!(
            "terrain: sim/terrain_sim.json ({} x {} at {} m): sea Water, mountain Mountain",
            t.nx, t.ny, t.tile_m
        ));
        Some(t)
    } else {
        rep.inputs.push(
            "terrain: none (no sim/terrain_sim.json: every tile off the blocks and ways is Water, the S3b fallback)"
                .into(),
        );
        None
    };
    let rail_json = if sim_dir.join("rail.json").is_file() {
        let r = read_json(&sim_dir, "rail.json")?;
        check_schema(&r, "sim/rail.json", &mut rep)?;
        rep.inputs
            .push("rail: sim/rail.json (lines and named stations; the timetable stays the sim's [transit])".into());
        Some(r)
    } else {
        rep.inputs.push("rail: elevated.json rail elements (no sim/rail.json: the S3b fallback)".into());
        None
    };
    let mut raw_portals: Vec<RawPortal> = Vec::new();
    if sim_dir.join("walkways_sim.json").is_file() {
        raw_portals = read_walkways(&sim_dir, &mut rep)?;
        rep.inputs.push(format!(
            "walkways: sim/walkways_sim.json ({} portals; the P lines, C7: not the vehicle ramps)",
            raw_portals.len()
        ));
    }
    // C5: where the export came from: TheCityGraphics' commit (its `out/`
    // is generated and gitignored, so the commit is the code that made it)
    // and the export's seed.
    let sha = std::process::Command::new("git")
        .arg("-C")
        .arg(src)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let seed = rep.export_seed;
    rep.inputs.insert(
        0,
        format!(
            "TheCityGraphics {}, export seed {}",
            sha.as_deref().unwrap_or("(no git commit at --src)"),
            seed.map_or("(none)".to_string(), |s| s.to_string())
        ),
    );

    let districts_json = read_json(src, "districts.json")?;
    let blocks_json = read_json(src, "blocks.json")?;
    let lots_json = read_json(src, "lots.json")?;
    let roads_json = read_json(src, "roads.json")?;
    let elevated_json = read_json(src, "elevated.json")?;
    let tile_src = lots_json.get("tile").and_then(|t| t.as_f64()).unwrap_or(2.0);
    let s = tile_src * f64::from(k);
    rep.tile_m = s;

    // The city's extent: terrain.json's size_m, else every block and way.
    let size = match read_json(src, "terrain.json") {
        Ok(t) => {
            let ctx = "terrain.json summary";
            let a = arr(get(&t, "summary", "terrain.json")?, "size_m", ctx)?;
            if a.len() != 2 {
                return Err(format!("{ctx}: size_m is not [w, h]"));
            }
            [0.0, 0.0, num(&a[0], ctx)?, num(&a[1], ctx)?]
        }
        Err(e) => {
            rep.warnings.push(format!("{e}; the extent is the blocks' and ways' bounding box"));
            let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for (file, v, list) in [("blocks.json", &blocks_json, "blocks"), ("roads.json", &roads_json, "ways")] {
                for (i, x) in arr(v, list, file)?.iter().enumerate() {
                    let r = field_rect(x, "rect_m", &format!("{file} {list}[{i}]"))?;
                    b = [b[0].min(r[0]), b[1].min(r[1]), b[2].max(r[2]), b[3].max(r[3])];
                }
            }
            b
        }
    };
    let win = match &args.crop {
        None => size,
        Some(c) => {
            let v: Vec<f64> = c
                .split(',')
                .map(|x| x.trim().parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|e| format!("--crop {c}: {e}"))?;
            let r: [f64; 4] = v.try_into().map_err(|_| format!("--crop {c}: expected x0,y0,x1,y1"))?;
            if !(r[0] < r[2] && r[1] < r[3]) {
                return Err(format!("--crop {c}: empty window"));
            }
            r
        }
    };
    let (w, h) = (((win[2] - win[0]) / s).ceil() as usize, ((win[3] - win[1]) / s).ceil() as usize);
    if w < 8 || h < 8 || w > citysim::map::MAX_SIDE || h > citysim::map::MAX_SIDE {
        return Err(format!("the map would be {w} x {h} tiles; each side must be 8..={}", citysim::map::MAX_SIDE));
    }
    let f = Frame { x0: win[0], y1: win[3], s, w, h };
    rep.w = w;
    rep.h = h;
    let at = |x: usize, y: usize| y * w + x;

    // --- districts --------------------------------------------------------------
    let rows = arr(&districts_json, "rows", "districts.json")?;
    let mut drow: BTreeMap<String, (String, Zone)> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        let ctx = format!("districts.json rows[{i}]");
        let id = text(r, "id", &ctx)?.to_string();
        let name = text(r, "name", &ctx)?.to_string();
        let z = text(r, "zone", &ctx)?;
        let zone = zone_letter(z).unwrap_or_else(|| {
            rep.warnings.push(format!("{ctx}: zone {z:?} unknown to the sim; read as Mid"));
            Zone::Mid
        });
        drow.insert(id, (name, zone));
    }
    // Superblocks seed the district grid (multi-source BFS fills the rest).
    let mut dgrid: Vec<u16> = vec![u16::MAX; w * h];
    let mut queue = VecDeque::new();
    let mut used_ids: Vec<String> = Vec::new();
    for (i, sb) in arr(&districts_json, "superblocks", "districts.json")?.iter().enumerate() {
        let ctx = format!("districts.json superblocks[{i}]");
        let d = text(sb, "district", &ctx)?;
        if !drow.contains_key(d) {
            return Err(format!("{ctx}: district {d:?} has no row"));
        }
        let r = field_rect(sb, "rect_m", &ctx)?;
        if !overlaps(r, win) {
            continue;
        }
        let idx = match used_ids.iter().position(|x| x == d) {
            Some(p) => p,
            None => {
                used_ids.push(d.to_string());
                used_ids.len() - 1
            }
        } as u16;
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                if dgrid[at(x, y)] == u16::MAX {
                    dgrid[at(x, y)] = idx;
                    queue.push_back((x, y));
                }
            }
        }
    }
    if used_ids.is_empty() {
        return Err("districts.json: no superblock lies in the window".into());
    }
    // A district with no superblock of its own (D7 The Stack: its block says
    // D7, its superblock D1) takes its blocks' tiles.
    let superblock_ids = used_ids.clone();
    for (i, b) in arr(&blocks_json, "blocks", "blocks.json")?.iter().enumerate() {
        let ctx = format!("blocks.json blocks[{i}]");
        let Some(d) = b.get("district").and_then(|d| d.as_str()) else { continue };
        if superblock_ids.iter().any(|x| x == d) || !drow.contains_key(d) {
            continue;
        }
        let r = field_rect(b, "rect_m", &ctx)?;
        if !overlaps(r, win) {
            continue;
        }
        let idx = match used_ids.iter().position(|x| x == d) {
            Some(p) => p,
            None => {
                used_ids.push(d.to_string());
                used_ids.len() - 1
            }
        } as u16;
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                if dgrid[at(x, y)] != idx {
                    dgrid[at(x, y)] = idx;
                    queue.push_back((x, y));
                }
            }
        }
    }
    // Sort the districts by their row order (D1, D2, ...), then remap.
    let order: Vec<String> =
        rows.iter().filter_map(|r| r.get("id").and_then(|i| i.as_str()).map(String::from)).collect();
    let mut sorted = used_ids.clone();
    sorted.sort_by_key(|d| order.iter().position(|o| o == d).unwrap_or(usize::MAX));
    if sorted.len() > citysim::MAX_DISTRICTS {
        return Err(format!("{} districts; the sim holds {}", sorted.len(), citysim::MAX_DISTRICTS));
    }
    let remap: Vec<u16> = used_ids.iter().map(|d| sorted.iter().position(|x| x == d).unwrap_or(0) as u16).collect();
    for v in dgrid.iter_mut() {
        if *v != u16::MAX {
            *v = remap[usize::from(*v)];
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        let d = dgrid[at(x, y)];
        for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
            if nx < w && ny < h && dgrid[at(nx, ny)] == u16::MAX {
                dgrid[at(nx, ny)] = d;
                queue.push_back((nx, ny));
            }
        }
    }
    let districts: Vec<MapDistrict> = sorted
        .iter()
        .map(|d| {
            let (name, zone) = drow[d].clone();
            MapDistrict { name: format!("{d} {name}"), zone, x_from: 0, x_to: w as u16 }
        })
        .collect();
    for (d, (name, _)) in &drow {
        if !sorted.contains(d) {
            rep.unmapped.push(format!("district {d} {name}: no superblock in the window (not generated yet?)"));
        }
    }
    rep.districts = districts.iter().map(|d| format!("{} ({})", d.name, d.zone.letter())).collect();
    let zone_of_d: Vec<Zone> = districts.iter().map(|d| d.zone).collect();

    // --- tiles: water, land, roads ------------------------------------------------
    let mut tiles = vec!['~'; w * h];
    // S3c: the terrain first (each sim tile the most common class of its
    // samples, one per their tile; ties urban, then mountain), then the
    // blocks and ways over it.
    if let Some(t) = &terrain {
        let sub = ((s / t.tile_m).round() as usize).max(1);
        for ty in 0..h {
            for tx in 0..w {
                let mut n = [0usize; 3];
                for j in 0..sub {
                    for i in 0..sub {
                        let x = f.x0 + (tx as f64 + (i as f64 + 0.5) / sub as f64) * s;
                        let y = f.y1 - (ty as f64 + (j as f64 + 0.5) / sub as f64) * s;
                        n[usize::from(t.at(x, y))] += 1;
                    }
                }
                let c = [URBAN, MOUNTAIN, SEA].into_iter().fold(URBAN, |b, c| {
                    if n[usize::from(c)] > n[usize::from(b)] {
                        c
                    } else {
                        b
                    }
                });
                tiles[at(tx, ty)] = match c {
                    URBAN => '.',
                    MOUNTAIN => 'm',
                    _ => '~',
                };
            }
        }
        rep.mountain_tiles = tiles.iter().filter(|&&c| c == 'm').count();
        rep.sea_tiles = tiles.iter().filter(|&&c| c == '~').count();
    }
    // S3c: with the terrain, only the blocks are laid over its Mountain (a
    // superblock's or a green's rect is a bounding box, often up the hill).
    let land = |r: [f64; 4], tiles: &mut Vec<char>, over_hills: bool| {
        let (x0, y0, x1, y1) = f.round(r);
        for y in y0..y1 {
            for x in x0..x1 {
                if over_hills || tiles[at(x, y)] != 'm' {
                    tiles[at(x, y)] = '.';
                }
            }
        }
    };
    for (i, sb) in arr(&districts_json, "superblocks", "districts.json")?.iter().enumerate() {
        land(field_rect(sb, "rect_m", &format!("districts.json superblocks[{i}]"))?, &mut tiles, false);
    }
    for list in ["blocks", "open", "greens_m", "leftover_m", "voids_m"] {
        let Some(items) = blocks_json.get(list).and_then(|v| v.as_array()) else {
            rep.warnings.push(format!("blocks.json: no \"{list}\" list"));
            continue;
        };
        for (i, it) in items.iter().enumerate() {
            let ctx = format!("blocks.json {list}[{i}]");
            let r = if it.is_array() { rect_m(it, &ctx)? } else { field_rect(it, "rect_m", &ctx)? };
            land(r, &mut tiles, list == "blocks");
        }
    }
    let ways = arr(&roads_json, "ways", "roads.json")?;
    let mut by_cls: BTreeMap<String, usize> = BTreeMap::new();
    // A rear alley inside a block is built over when the block's parts are
    // too thin for the tile size (`whole` below).
    let (mut alley, mut street) = (vec![false; w * h], vec![false; w * h]);
    for (i, wy) in ways.iter().enumerate() {
        let ctx = format!("roads.json ways[{i}]");
        let r = field_rect(wy, "rect_m", &ctx)?;
        let cls = text(wy, "cls", &ctx)?;
        *by_cls.entry(cls.to_string()).or_default() += 1;
        let (x0, y0, x1, y1) = f.road(r);
        for y in y0..y1 {
            for x in x0..x1 {
                tiles[at(x, y)] = '=';
                if cls == "alley" {
                    alley[at(x, y)] = true;
                } else {
                    street[at(x, y)] = true;
                }
            }
        }
    }
    let road = |tiles: &Vec<char>, x: usize, y: usize| tiles[at(x, y)] == '=';
    // An island has no road on the map's edge: carve a pier from the nearest
    // road to each side, so migrants have a way in (`Map::edge_roads`).
    let has_edge = (0..w).any(|x| road(&tiles, x, 0) || road(&tiles, x, h - 1))
        || (0..h).any(|y| road(&tiles, 0, y) || road(&tiles, w - 1, y));
    if !has_edge {
        let roads: Vec<(usize, usize)> =
            (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| road(&tiles, x, y)).collect();
        if roads.is_empty() {
            return Err("no road in the window".into());
        }
        for side in ["west", "east", "north", "south"] {
            let dist = |p: &(usize, usize)| match side {
                "west" => p.0,
                "east" => w - 1 - p.0,
                "north" => p.1,
                _ => h - 1 - p.1,
            };
            let Some(&(x, y)) = roads.iter().min_by_key(|p| (dist(p), p.1, p.0)) else { break };
            let path: Vec<(usize, usize)> = match side {
                "west" => (0..x).map(|xx| (xx, y)).collect(),
                "east" => (x + 1..w).map(|xx| (xx, y)).collect(),
                "north" => (0..y).map(|yy| (x, yy)).collect(),
                _ => (y + 1..h).map(|yy| (x, yy)).collect(),
            };
            for (px, py) in path {
                tiles[at(px, py)] = '=';
            }
            rep.piers += 1;
        }
    }

    // --- lots and layouts ---------------------------------------------------------
    let lots_list = arr(&lots_json, "lots", "lots.json")?;
    let mut lots_by_block: BTreeMap<String, Vec<Lot>> = BTreeMap::new();
    // C5: each block's district (`D4 The Stacks`), for the units-by-district table.
    let mut block_d: BTreeMap<String, String> = BTreeMap::new();
    for b in arr(&blocks_json, "blocks", "blocks.json")? {
        if let (Some(id), Some(d)) = (b.get("id").and_then(Value::as_str), b.get("district").and_then(Value::as_str)) {
            let name = drow.get(d).map_or(String::new(), |(n, _)| format!(" {n}"));
            block_d.insert(id.to_string(), format!("{d}{name}"));
        }
    }
    for (i, l) in lots_list.iter().enumerate() {
        let ctx = format!("lots.json lots[{i}]");
        let id = text(l, "id", &ctx)?;
        let ctx = format!("{ctx} ({id})");
        let rect = field_rect(l, "rect_m", &ctx)?;
        if !inside(rect, win) {
            continue;
        }
        rep.lots += 1;
        let block = text(l, "block", &ctx)?.to_string();
        let front = text(l, "front", &ctx)?.chars().next().unwrap_or('S');
        // S3c: the export's record replaces the floor range and the layout read.
        if let Some(sim) = &sim_lots {
            let rec = sim
                .get(id)
                .ok_or_else(|| format!("sim/lots_sim.json lots: no record for lot {id} (lots.json lots[{i}])"))?;
            if rec.layout {
                rep.lots_with_layout += 1;
            }
            rep.flats += rec.flats as usize;
            rep.shop_units += rec.shops;
            // C5: 0.2's levels and cores, counted (C6a and C7 map them).
            rep.levels += rec.levels.len();
            rep.plate_levels += rec.levels.iter().filter(|x| x.plate).count();
            rep.cores_lift += rec.cores.iter().filter(|c| c.lift).count();
            rep.cores_flight += rec.cores.iter().filter(|c| !c.lift).count();
            let d = block_d.get(&block).cloned().unwrap_or_else(|| "-".into());
            let row = rep.units_by_district.entry(d).or_default();
            for lv in &rec.levels {
                for (k, &n) in lv.units.iter().enumerate() {
                    rep.level_units[k] += u64::from(n);
                    row[k] += u64::from(n);
                }
            }
            if rec.works {
                row[4] += 1;
            }
            let lot = Lot {
                id: id.to_string(),
                rect,
                front,
                floors: rec.floors.max(1),
                flats: rec.flats as usize,
                shops: rec.shops,
                layout: rec.layout,
                residential: rec.use_ == Use::Residential,
                door_off: None,
                sim: Some(rec.clone()),
            };
            lots_by_block.entry(block).or_default().push(lot);
            continue;
        }
        let fl = arr(l, "floors", &ctx)?;
        if fl.len() != 2 {
            return Err(format!("{ctx}: \"floors\" is not [lo, hi]"));
        }
        let (lo, hi) = (num(&fl[0], &ctx)?.max(1.0) as u32, num(&fl[1], &ctx)?.max(1.0) as u32);
        let seed = get(l, "seed", &ctx)?.as_u64().ok_or_else(|| format!("{ctx}: \"seed\" is not an integer"))?;
        // Massing floors are not in lots.json (only the district's range):
        // a deterministic pick in the range from the lot's seed.
        let mut lot = Lot {
            id: id.to_string(),
            rect,
            front,
            floors: lo + (seed % u64::from(hi.saturating_sub(lo) + 1)) as u32,
            flats: 0,
            shops: 0,
            layout: false,
            residential: true,
            door_off: None,
            sim: None,
        };
        let layout = src.join("bldg").join(id).join("sim_layout.toml");
        if layout.is_file() {
            let (storeys, flats, shops, residential, door_off) = read_layout(&layout)?;
            lot.floors = storeys;
            lot.flats = flats;
            lot.shops = shops;
            lot.residential = residential;
            lot.door_off = door_off;
            lot.layout = true;
            rep.lots_with_layout += 1;
            rep.flats += flats;
            rep.shop_units += shops;
        }
        lots_by_block.entry(block).or_default().push(lot);
    }
    if let Some(sim) = &sim_lots {
        if sim.len() != lots_list.len() {
            rep.warnings.push(format!(
                "sim/lots_sim.json holds {} records, lots.json {} lots (records past lots.json are ignored)",
                sim.len(),
                lots_list.len()
            ));
        }
    }

    // --- buildings: block parts cut at lot boundaries ---------------------------
    let mut occupied = vec![false; w * h];
    let mut segs: Vec<Seg> = Vec::new();
    for (i, b) in arr(&blocks_json, "blocks", "blocks.json")?.iter().enumerate() {
        let ctx = format!("blocks.json blocks[{i}]");
        let id = text(b, "id", &ctx)?;
        let ctx = format!("{ctx} ({id})");
        let brect = field_rect(b, "rect_m", &ctx)?;
        if !inside(brect, win) {
            continue;
        }
        let parts: Vec<[f64; 4]> = match b.get("parts_m").and_then(|p| p.as_array()) {
            Some(p) if !p.is_empty() => p
                .iter()
                .enumerate()
                .map(|(j, r)| rect_m(r, &format!("{ctx} parts_m[{j}]")))
                .collect::<Result<_, _>>()?,
            _ => vec![brect],
        };
        // Parts thinner than 3 tiles (the Sump's 12-16 m lots past 4 m
        // tiles): the whole block is one part, over its rear alleys.
        let whole = parts.len() > 1
            && parts.iter().any(|p| {
                let (a, b, c, d) = f.round(*p);
                c.saturating_sub(a) < 3 || d.saturating_sub(b) < 3
            });
        let parts = if whole { vec![brect] } else { parts };
        let block_lots = lots_by_block.get(id).cloned().unwrap_or_default();
        let vats_block =
            b.get("district").and_then(|d| d.as_str()).and_then(|d| drow.get(d)).is_some_and(|(_, z)| *z == Zone::Vats);
        for part in parts {
            rep.parts += 1;
            let lots: Vec<&Lot> = block_lots
                .iter()
                .filter(|l| {
                    let (cx, cy) = ((l.rect[0] + l.rect[2]) / 2.0, (l.rect[1] + l.rect[3]) / 2.0);
                    cx >= part[0] && cx < part[2] && cy >= part[1] && cy < part[3]
                })
                .collect();
            if lots.is_empty() {
                continue;
            }
            let (px0, py0, px1, py1) = f.round(part);
            if px1.saturating_sub(px0) < 3 || py1.saturating_sub(py0) < 3 {
                rep.dropped_small += 1;
                rep.lots_dropped += lots.len();
                continue;
            }
            // The part's front: its lots' most common front.
            let mut fronts: BTreeMap<char, usize> = BTreeMap::new();
            for l in &lots {
                *fronts.entry(l.front).or_default() += 1;
            }
            let front = fronts.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c))).map_or('S', |(c, _)| *c);
            let along_x = matches!(front, 'N' | 'S');
            // Cuts along the frontage at lot edges, every segment >= 3 tiles.
            let mut edges: Vec<usize> = lots
                .iter()
                .map(|l| {
                    let (_, y0, x1, _) = f.round(l.rect);
                    if along_x {
                        x1
                    } else {
                        y0
                    }
                })
                .collect();
            edges.sort_unstable();
            edges.dedup();
            let (a0, a1) = if along_x { (px0, px1) } else { (py0, py1) };
            let mut cuts = vec![a0];
            // The Vats' godowns stay whole (a Vat Farm needs 60 interior
            // tiles). S3c: with the export, a Vats part is cut between two
            // lots each big enough for a Farm on its own (at 2 m tiles most
            // vat farms are; at 8 m none, and the parts stay whole).
            if vats_block {
                let depth = if along_x { py1 - py0 } else { px1 - px0 };
                let span = |l: &Lot| {
                    let (x0, y0, x1, y1) = f.round(l.rect);
                    if along_x {
                        (x0, x1)
                    } else {
                        (y0, y1)
                    }
                };
                // C9a: a stretch is cut off once it is big enough for a
                // Farm and the rest of the part still is (the lots of a
                // stretch together, not each alone: at 4 m tiles most of the
                // Vats' works lots are under a Farm, so the 100k city's 600
                // Farms found 227 Vats cuts and took the Spire's towers).
                let farm_sized = |len: usize| len.saturating_sub(2) * depth.saturating_sub(2) >= 60;
                let mut last = a0;
                edges.retain(|&e| {
                    let before: Vec<&&Lot> = lots.iter().filter(|l| span(l).1 == e).collect();
                    let after: Vec<&&Lot> = lots.iter().filter(|l| span(l).0 == e).collect();
                    let keep = !before.is_empty()
                        && !after.is_empty()
                        && before.iter().chain(&after).all(|l| l.sim.is_some())
                        && e >= last
                        && farm_sized(e - last)
                        && farm_sized(a1.saturating_sub(e));
                    if keep {
                        last = e;
                    }
                    keep
                });
            }
            for e in edges {
                let last = *cuts.last().unwrap_or(&a0);
                if e >= last + 3 && e + 3 <= a1 {
                    cuts.push(e);
                }
            }
            cuts.push(a1);
            for c in cuts.windows(2) {
                let (s0, s1) = (c[0], c[1]);
                let (mut x0, mut y0, mut x1, mut y1) = if along_x { (s0, py0, s1, py1) } else { (px0, s0, px1, s1) };
                // Shrink off road and taken tiles, edge by edge.
                let bad = |x: usize, y: usize| {
                    let i = at(x, y);
                    (tiles[i] == '=' && !(whole && alley[i] && !street[i])) || occupied[i]
                };
                // The edge with the largest share of bad tiles goes first (a
                // road along one side is one edge, not a column at a time).
                for _ in 0..8 {
                    if x1 <= x0 + 2 || y1 <= y0 + 2 {
                        break;
                    }
                    let share = |n: usize, len: usize| n as f64 / len as f64;
                    let (hw, hh) = (x1 - x0, y1 - y0);
                    let edges = [
                        share((y0..y1).filter(|&y| bad(x0, y)).count(), hh),
                        share((y0..y1).filter(|&y| bad(x1 - 1, y)).count(), hh),
                        share((x0..x1).filter(|&x| bad(x, y0)).count(), hw),
                        share((x0..x1).filter(|&x| bad(x, y1 - 1)).count(), hw),
                    ];
                    let (e, top) =
                        edges.iter().enumerate().fold((0, 0.0), |b, (i, &v)| if v > b.1 { (i, v) } else { b });
                    if top == 0.0 {
                        break;
                    }
                    match e {
                        0 => x0 += 1,
                        1 => x1 -= 1,
                        2 => y0 += 1,
                        _ => y1 -= 1,
                    }
                }
                let members: Vec<&&Lot> = lots
                    .iter()
                    .filter(|l| {
                        let (cx, cy) = ((l.rect[0] + l.rect[2]) / 2.0, (l.rect[1] + l.rect[3]) / 2.0);
                        let (tx, ty) = (((cx - f.x0) / s) as usize, ((f.y1 - cy) / s) as usize);
                        if along_x {
                            (s0..s1).contains(&tx)
                        } else {
                            (s0..s1).contains(&ty)
                        }
                    })
                    .collect();
                // C9a: a piece (no lot centre in it) is a slice of the lots
                // whose span it overlaps along the cut, each by its share of
                // the lot's length: it stands their storeys (flats by that
                // share, the flats' storeys, their lifts), not a ground floor
                // with no flat (C8b: `exec_7467` on the ground of a 59-storey
                // piece). The owning cut keeps its flats (a few hundred
                // counted twice at 100k; the households follow the flats).
                let cover: Vec<(&Lot, f64)> = if members.is_empty() {
                    lots.iter()
                        .filter_map(|l| {
                            let (lx0, ly0, lx1, ly1) = f.round(l.rect);
                            let (a, b) = if along_x { (lx0, lx1) } else { (ly0, ly1) };
                            let over = b.min(s1).saturating_sub(a.max(s0));
                            (over > 0 && b > a).then(|| (*l, over as f64 / (b - a) as f64))
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                if x1 < x0 + 3 || y1 < y0 + 3 {
                    rep.dropped_small += 1;
                    rep.lots_dropped += members.len();
                    continue;
                }
                if (y0..y1).any(|y| (x0..x1).any(|x| bad(x, y))) {
                    rep.dropped_overlap += 1;
                    rep.lots_dropped += members.len();
                    continue;
                }
                let rect = Rect { x: x0 as u16, y: y0 as u16, w: (x1 - x0) as u16, h: (y1 - y0) as u16 };
                // The door: the front side first, from its middle out (or the
                // layout's street door on a one-lot cut), then the other sides.
                let hint = match (members.as_slice(), k) {
                    ([one], 1) if one.layout && front == 'S' => one.door_off.map(|o| x0 + o.min(x1 - x0 - 1)),
                    ([one], 1) if one.layout && front == 'N' => one.door_off.map(|o| x1 - 1 - o.min(x1 - x0 - 1)),
                    _ => None,
                };
                let side_tiles = |side: char| -> Vec<((usize, usize), (isize, isize))> {
                    let (n, mid) = match side {
                        'N' | 'S' => (x1 - x0, (x0 + x1) / 2),
                        _ => (y1 - y0, (y0 + y1) / 2),
                    };
                    let mut order: Vec<usize> = Vec::new();
                    if let Some(hx) = hint.filter(|_| side == front) {
                        order.push(hx);
                    }
                    for d in 0..n {
                        for v in [mid as isize - d as isize, mid as isize + d as isize] {
                            if v >= 0 {
                                order.push(v as usize);
                            }
                        }
                    }
                    order
                        .into_iter()
                        .filter_map(|v| match side {
                            'S' if v > x0 && v + 1 < x1 => Some(((v, y1 - 1), (0, 1))),
                            'N' if v > x0 && v + 1 < x1 => Some(((v, y0), (0, -1))),
                            'E' if v > y0 && v + 1 < y1 => Some(((x1 - 1, v), (1, 0))),
                            'W' if v > y0 && v + 1 < y1 => Some(((x0, v), (-1, 0))),
                            _ => None,
                        })
                        .collect()
                };
                // S3c: the members' (else the part's) lots, for the export's fields.
                let src_lots: Vec<&Lot> =
                    if members.is_empty() { lots.clone() } else { members.iter().map(|l| **l).collect() };
                // S3c: their street doors first: a wall tile (not a corner)
                // whose outside is the `out` tile, else the road-facing wall
                // tile nearest any `out` tile.
                let outs: Vec<(usize, usize)> = src_lots
                    .iter()
                    .filter_map(|l| l.sim.as_ref())
                    .flat_map(|r| r.doors_out.iter())
                    .filter_map(|&(x, y)| f.tile(x, y))
                    .collect();
                let mut door = None;
                if !outs.is_empty() {
                    let mut cands = wall_doors((x0, y0, x1, y1), w, h);
                    cands.retain(|&(_, (ox, oy))| road(&tiles, ox, oy));
                    let dist = |c: &((usize, usize), (usize, usize))| {
                        outs.iter().map(|o| c.1 .0.abs_diff(o.0) + c.1 .1.abs_diff(o.1)).min().unwrap_or(usize::MAX)
                    };
                    if let Some(c) = outs.iter().find_map(|o| cands.iter().find(|c| c.1 == *o)) {
                        door = Some(c.0);
                        rep.doors_exact += 1;
                    } else if let Some(c) = cands.iter().min_by_key(|c| dist(c)) {
                        door = Some(c.0);
                        rep.doors_near += 1;
                    }
                }
                let searched = door.is_none() && sim_lots.is_some();
                for side in std::iter::once(front).chain(['S', 'N', 'E', 'W'].into_iter().filter(|&c| c != front)) {
                    if door.is_some() {
                        break;
                    }
                    for ((dx, dy), (ox, oy)) in side_tiles(side) {
                        let (nx, ny) = (dx as isize + ox, dy as isize + oy);
                        if nx >= 0
                            && ny >= 0
                            && (nx as usize) < w
                            && (ny as usize) < h
                            && road(&tiles, nx as usize, ny as usize)
                        {
                            door = Some((dx, dy));
                            break;
                        }
                    }
                    if door.is_some() {
                        break;
                    }
                }
                let Some(door) = door else {
                    rep.dropped_no_road += 1;
                    rep.lots_dropped += members.len();
                    continue;
                };
                if searched {
                    rep.doors_searched += 1;
                }
                for y in y0..y1 {
                    for x in x0..x1 {
                        occupied[at(x, y)] = true;
                    }
                }
                let floors = if members.is_empty() {
                    // C9a: the storeys of the lots it is a slice of.
                    cover
                        .iter()
                        .map(|(l, _)| l.floors)
                        .max()
                        .or_else(|| lots.iter().map(|l| l.floors).max())
                        .unwrap_or(1)
                } else {
                    let sum: u32 = members.iter().map(|l| l.floors).sum();
                    (f64::from(sum) / members.len() as f64).round() as u32
                };
                let (cx, cy) =
                    (usize::from(rect.x) + usize::from(rect.w) / 2, usize::from(rect.y) + usize::from(rect.h) / 2);
                // S3c: the export's use, ground, flats and civic name.
                let recs: Vec<&SimLot> = src_lots.iter().filter_map(|l| l.sim.as_ref()).collect();
                let civic = members
                    .iter()
                    .filter_map(|l| l.sim.as_ref().and_then(|r| r.civic.clone()))
                    .min_by_key(|c| (civic_rank(c), c.clone()));
                let use_ = if recs.is_empty() {
                    None
                } else if civic.is_some() {
                    Some(Use::Civic)
                } else {
                    let mut n: BTreeMap<Use, usize> = BTreeMap::new();
                    for r in recs.iter().filter(|r| r.civic.is_none()) {
                        *n.entry(r.use_).or_default() += 1;
                    }
                    n.iter()
                        .max_by_key(|(u, c)| (**c, std::cmp::Reverse(**u)))
                        .map_or(Some(Use::Civic), |(u, _)| Some(*u))
                };
                // C6a: the members' units by storey (0.2's levels; none in a
                // 0.1 export), ground first, and their flats' storeys.
                let mut units: Vec<Unit> = Vec::new();
                let mut flat_span: Option<(u8, u8)> = None;
                let mut lot_outs: Vec<Vec<(usize, usize)>> = Vec::new();
                for (j, l) in members.iter().enumerate() {
                    let Some(r) = l.sim.as_ref() else {
                        lot_outs.push(Vec::new());
                        continue;
                    };
                    lot_outs.push(r.doors_out.iter().filter_map(|&(x, y)| f.tile(x, y)).collect());
                    for lv in &r.levels {
                        let level = u8::try_from(lv.i).unwrap_or(u8::MAX);
                        for (kind, n) in
                            [(UNIT_SHOP, lv.units[1]), (UNIT_WORKSHOP, lv.units[2]), (UNIT_OFFICE, lv.units[3])]
                        {
                            units.extend(std::iter::repeat_n(Unit { level, kind, lot: j }, n as usize));
                        }
                        // C9a: a tower's lobby (the Spire's ground, the
                        // export's `use: lobby`) is a unit, where the Spire's
                        // Lots stand (its Hqs and Lounges) under the flats;
                        // before, the Spire had no unit and its Lots took the
                        // towers whole (20k: 218 of its 286 cuts).
                        if lv.use_ == "lobby" {
                            units.push(Unit { level, kind: UNIT_OFFICE, lot: j });
                        }
                        if lv.units[0] > 0 {
                            flat_span = Some(flat_span.map_or((level, level), |(a, b)| (a.min(level), b.max(level))));
                        }
                    }
                    if r.works {
                        units.push(Unit { level: 0, kind: UNIT_WORKS, lot: j });
                    }
                }
                units.sort_by_key(|u| (u.level, u.kind, u.lot));
                // C9a: a piece's flats' storeys are its cover's.
                for r in cover.iter().filter_map(|(l, _)| l.sim.as_ref()) {
                    for lv in r.levels.iter().filter(|lv| lv.units[0] > 0) {
                        let level = u8::try_from(lv.i).unwrap_or(u8::MAX);
                        flat_span = Some(flat_span.map_or((level, level), |(a, b)| (a.min(level), b.max(level))));
                    }
                }
                // C7: the members' lifts (the export's lift cores, from the
                // ground); C9a: a piece's, its cover's.
                let lifts: Vec<u32> = members
                    .iter()
                    .map(|l| **l)
                    .chain(cover.iter().map(|(l, _)| *l))
                    .filter_map(|l| l.sim.as_ref())
                    .flat_map(|r| r.cores.iter().filter(|c| c.lift).map(|c| c.floors.1))
                    .collect();
                let lift = lifts
                    .iter()
                    .max()
                    .map(|&top| (u8::try_from(lifts.len()).unwrap_or(u8::MAX), u8::try_from(top).unwrap_or(u8::MAX)));
                segs.push(Seg {
                    units,
                    flat_span,
                    lift,
                    lot_outs,
                    rect,
                    door,
                    district: usize::from(dgrid[at(cx, cy)]),
                    floors: floors.clamp(1, 255),
                    shops: members.iter().map(|l| l.shops).sum(),
                    residential: members.iter().all(|l| l.residential),
                    use_,
                    workshop: !recs.is_empty() && recs.iter().filter(|r| r.workshop).count() * 2 >= recs.len(),
                    flats: members.iter().map(|l| l.flats as u32).sum::<u32>()
                        + cover.iter().map(|(l, share)| (l.flats as f64 * share).round() as u32).sum::<u32>(),
                    civic,
                    lots: members.iter().map(|l| (l.id.clone(), l.rect)).collect(),
                    own_lot: !members.is_empty(),
                });
            }
        }
    }
    // A door's outside tile can be a rear alley a whole-block cut built
    // over later: such a door moves to the nearest wall tile still facing an
    // open road (else the cut is dropped).
    let mut keep_seg = vec![true; segs.len()];
    for (j, sg) in segs.iter_mut().enumerate() {
        let r = sg.rect;
        let (x0, y0) = (usize::from(r.x), usize::from(r.y));
        let cands: Vec<((usize, usize), (usize, usize))> =
            wall_doors((x0, y0, x0 + usize::from(r.w), y0 + usize::from(r.h)), w, h)
                .into_iter()
                .filter(|&(_, (ox, oy))| road(&tiles, ox, oy) && !occupied[at(ox, oy)])
                .collect();
        if cands.iter().any(|c| c.0 == sg.door) {
            continue;
        }
        let d = sg.door;
        match cands.iter().min_by_key(|c| c.0 .0.abs_diff(d.0) + c.0 .1.abs_diff(d.1)) {
            Some(c) => {
                sg.door = c.0;
                rep.doors_moved += 1;
            }
            None => {
                keep_seg[j] = false;
                rep.dropped_no_road += 1;
            }
        }
    }
    let mut j = 0;
    segs.retain(|_| {
        j += 1;
        keep_seg[j - 1]
    });
    rep.segments = segs.len();
    if segs.is_empty() {
        return Err("no lot became a building (the window holds no block with lots?)".into());
    }

    // --- kinds: the generator's mix per 1,000 residents --------------------------
    let per_k = f64::from(args.residents) / 1000.0;
    let target = |per: f64| ((per * per_k).round() as usize).max(1);
    let mut kind: Vec<Option<BuildingKind>> = vec![None; segs.len()];
    let zone = |i: usize| zone_of_d.get(segs[i].district).copied().unwrap_or(Zone::Mid);
    let area = |i: usize| usize::from(segs[i].rect.w) * usize::from(segs[i].rect.h);
    let interior = |i: usize| (usize::from(segs[i].rect.w) - 2) * (usize::from(segs[i].rect.h) - 2);
    let pos = |i: usize| (segs[i].rect.y, segs[i].rect.x);
    // Take `n` of the free candidates, stage by stage (each a filter; S3c's
    // use-led picks), within a stage the preferred zones first, spread by
    // stride over the city (sorted by position) or the largest first.
    let pick = |kind: &mut Vec<Option<BuildingKind>>,
                rep: &mut Report,
                want: BuildingKind,
                n: usize,
                prefer: &[Zone],
                stages: &[&dyn Fn(usize) -> bool],
                largest: bool| {
        let mut got = 0;
        for ok in stages {
            let mut tiers: Vec<Vec<usize>> = prefer
                .iter()
                .map(|z| (0..segs.len()).filter(|&i| kind[i].is_none() && zone(i) == *z && ok(i)).collect())
                .collect();
            tiers.push((0..segs.len()).filter(|&i| kind[i].is_none() && !prefer.contains(&zone(i)) && ok(i)).collect());
            for mut c in tiers {
                if got >= n {
                    break;
                }
                c.retain(|&i| kind[i].is_none());
                if largest {
                    c.sort_by_key(|&i| (std::cmp::Reverse(area(i)), pos(i)));
                } else {
                    c.sort_by_key(|&i| pos(i));
                }
                let m = (n - got).min(c.len());
                let picks: Vec<usize> =
                    if largest { c[..m].to_vec() } else { (0..m).map(|j| c[j * c.len() / m]).collect() };
                for i in picks {
                    kind[i] = Some(want);
                    got += 1;
                }
            }
        }
        if got < n {
            rep.shortfall.push(format!("{want:?}: {got} of {n}"));
        }
    };
    // The city milestone C6a: a use dealt into a unit of a cut, not a whole
    // cut (`free` holds each cut's units left, `uses` what stands on them).
    // As `pick` (stages, the preferred zones first, spread by stride), one a
    // cut a round, rounds until `n` or no candidate has a unit left; a cut
    // dealt whole (`kind`) takes none. Returns how many it dealt.
    let mut free: Vec<Vec<Unit>> = segs.iter().map(|sg| sg.units.clone()).collect();
    let mut uses: Vec<Vec<(BuildingKind, Unit)>> = vec![Vec::new(); segs.len()];
    let pick_units = |kind: &Vec<Option<BuildingKind>>,
                      free: &mut Vec<Vec<Unit>>,
                      uses: &mut Vec<Vec<(BuildingKind, Unit)>>,
                      want: BuildingKind,
                      n: usize,
                      prefer: &[Zone],
                      stages: &[&dyn Fn(usize) -> bool]|
     -> usize {
        let mut got = 0;
        for ok in stages {
            let mut tiers: Vec<Vec<usize>> =
                prefer.iter().map(|z| (0..segs.len()).filter(|&i| zone(i) == *z && ok(i)).collect()).collect();
            tiers.push((0..segs.len()).filter(|&i| !prefer.contains(&zone(i)) && ok(i)).collect());
            for mut c in tiers {
                c.sort_by_key(|&i| pos(i));
                while got < n {
                    c.retain(|&i| kind[i].is_none() && !free[i].is_empty());
                    if c.is_empty() {
                        break;
                    }
                    let m = (n - got).min(c.len());
                    let picks: Vec<usize> = (0..m).map(|j| c[j * c.len() / m]).collect();
                    for i in picks {
                        if let Some(u) = take_unit(&mut free[i], want) {
                            uses[i].push((want, u));
                            got += 1;
                        }
                    }
                }
            }
        }
        got
    };
    use BuildingKind as K;
    use Zone as Z;
    let any = |_: usize| true;
    let farm_ok = |i: usize| interior(i) >= 60;
    let by_use = segs.iter().any(|sg| sg.use_.is_some());
    if !by_use {
        // S3b: no export, the generator's mix by district.
        pick(&mut kind, &mut rep, K::Jail, 1, &[Z::Civic], &[&any], true);
        pick(&mut kind, &mut rep, K::Hall, 1, &[Z::Civic], &[&any], true);
        pick(&mut kind, &mut rep, K::Warehouse, 1, &[Z::Vats], &[&any], true);
        pick(&mut kind, &mut rep, K::Cemetery, 1, &[Z::Vats], &[&any], false);
        pick(&mut kind, &mut rep, K::Farm, target(6.0), &[Z::Vats], &[&farm_ok], true);
        pick(&mut kind, &mut rep, K::SecurityOffice, target(1.0), &[Z::Vats], &[&any], false);
        pick(&mut kind, &mut rep, K::Hideout, target(1.0), &[Z::Sump], &[&any], false);
        // Shop cuts first; too few shop units: the rest on any block.
        let shop_ok = |i: usize| segs[i].shops > 0 || !segs[i].residential;
        for (kd, per) in [(K::Market, 1.5), (K::Bar, 1.5)] {
            pick(&mut kind, &mut rep, kd, target(per), &[Z::Mid, Z::Sump], &[&shop_ok, &any], false);
        }
        pick(&mut kind, &mut rep, K::Lot, target(80.0), &[], &[&any], false);
    } else {
        // S3c: the export's use leads; the counts stay the generator's mix.
        let segs = &segs;
        // The perf pass phase 10 (OPEN-182): a venue stands on its own lot
        // (`lot=` names it alone), so a cut holding no lot centre, a piece of
        // a lot another cut holds, is dealt no venue: it takes the kind the
        // S3c rule gives a cut nobody dealt (`rest` below: a Home, or a Lot
        // for an industrial, civic or flat-less commercial piece).
        for (i, sg) in segs.iter().enumerate() {
            if !sg.own_lot {
                kind[i] = Some(match sg.use_ {
                    Some(Use::Residential | Use::ShopHouse) => K::Home,
                    Some(Use::Commercial) if sg.flats > 0 => K::Home,
                    _ => K::Lot,
                });
                rep.pieces += 1;
            }
        }
        let is = |u: Use| move |i: usize| segs[i].use_ == Some(u);
        let (vat, factory, godown) = (is(Use::VatFarm), is(Use::Factory), is(Use::Godown));
        let (shop_house, commercial) = (is(Use::ShopHouse), is(Use::Commercial));
        let industrial = |i: usize| segs[i].use_.is_some_and(Use::industrial);
        // The named civic sites: city_hall the Hall, depot the Reserve Depot,
        // crematorium the Recycler; the precinct in a Civic district (else
        // the first) the Precinct (the Jail), every other precinct a
        // Security Office.
        let mut precincts: Vec<usize> = Vec::new();
        for (i, sg) in segs.iter().enumerate() {
            let Some(c) = sg.civic.as_deref() else { continue };
            let kd = match c {
                "city_hall" => K::Hall,
                "depot" => K::Warehouse,
                "crematorium" => K::Cemetery,
                p if p.starts_with("precinct") => {
                    precincts.push(i);
                    continue;
                }
                other => {
                    rep.warnings.push(format!("civic site {other:?}: no sim kind; read as a Lot"));
                    continue;
                }
            };
            if kind.contains(&Some(kd)) {
                rep.warnings.push(format!("civic site {c:?}: a second {kd:?}; read as a Lot"));
            } else {
                kind[i] = Some(kd);
            }
        }
        precincts.sort_by_key(|&i| (zone(i) != Z::Civic, segs[i].district, pos(i)));
        for (j, &i) in precincts.iter().enumerate() {
            kind[i] = Some(if j == 0 { K::Jail } else { K::SecurityOffice });
        }
        // A site the window or the cuts lost: the S3b rule, with a warning.
        for (kd, prefer, largest) in [
            (K::Jail, Z::Civic, true),
            (K::Hall, Z::Civic, true),
            (K::Warehouse, Z::Vats, true),
            (K::Cemetery, Z::Vats, false),
        ] {
            if !kind.contains(&Some(kd)) {
                rep.warnings.push(format!("no named civic site became the {kd:?}: placed by the S3b rule"));
                pick(&mut kind, &mut rep, kd, 1, &[prefer], &[&any], largest);
            }
        }
        let farm_vat = |i: usize| vat(i) && farm_ok(i);
        let farm_ind = |i: usize| industrial(i) && farm_ok(i);
        // Then a cut nobody lives in (commercial, civic), before any Block;
        // C6a: not an office stack (its offices are units the Lots take: at
        // 100k the Farms had taken D2's towers whole).
        let farm_open = |i: usize| {
            farm_ok(i)
                && !segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse))
                && !segs[i].units.iter().any(|u| u.kind == UNIT_OFFICE)
        };
        let farm_desk = |i: usize| farm_ok(i) && !segs[i].units.iter().any(|u| u.kind == UNIT_OFFICE);
        let n_farms = target(6.0);
        // The cuts nobody lives in first, the largest first (the shortfall,
        // if any, is the last stage's to report).
        let mut quiet = Report::default();
        pick(&mut kind, &mut quiet, K::Farm, n_farms, &[Z::Vats], &[&farm_vat, &farm_ind, &farm_open], true);
        let mut farms = kind.iter().filter(|x| **x == Some(K::Farm)).count();
        // C9a: past the cuts nobody lives in, a Farm takes the homes that
        // give up the fewest flats, zone by zone where the farms belong (the
        // Vats, the Sump, the Mid; `FARM_HOME_ZONES`), never the Spire's
        // towers (at 100k the largest-first stages had taken 218 of D5's
        // cuts, the largest lots on the island, and the Spire housed nobody).
        for z in FARM_HOME_ZONES {
            farms += fewest_flats(&mut kind, &uses, segs, K::Farm, n_farms.saturating_sub(farms), &|i| {
                zone(i) == z && farm_desk(i)
            });
        }
        // No last resort: a Farm takes no office stack, no tower lobby and no
        // Spire home; what the island cannot place is a shortfall (100k).
        if farms < n_farms {
            rep.shortfall
                .push(format!("Farm: {farms} of {n_farms} (the farm-sized cuts off the Spire and the office stacks)"));
        }
        // C6a: from here the uses go into units (a Security Office in an
        // office or a works floor, a Hideout in the alley's workshop, the
        // venues on the shop fronts, the Lots in the office stacks), so the
        // cuts they took before stay Homes; a use with no unit left in its
        // stage takes a whole cut as before (the fewest flats first).
        let n_uses = |kind: &Vec<Option<BuildingKind>>, uses: &Vec<Vec<(BuildingKind, Unit)>>, k: BuildingKind| {
            kind.iter().filter(|x| **x == Some(k)).count()
                + uses.iter().map(|u| u.iter().filter(|x| x.0 == k).count()).sum::<usize>()
        };
        let n_so = target(1.0).saturating_sub(n_uses(&kind, &uses, K::SecurityOffice));
        if n_so > 0 {
            let got = pick_units(
                &kind,
                &mut free,
                &mut uses,
                K::SecurityOffice,
                n_so,
                &[Z::Vats],
                &[&factory, &godown, &industrial, &any],
            );
            if got < n_so {
                pick(&mut kind, &mut rep, K::SecurityOffice, n_so - got, &[Z::Vats], &[&industrial, &any], false);
            }
        }
        let alley = |i: usize| shop_house(i) && segs[i].workshop;
        let shopfront = |i: usize| shop_house(i) && !segs[i].workshop;
        let n_hide = target(1.0);
        let got = pick_units(&kind, &mut free, &mut uses, K::Hideout, n_hide, &[Z::Sump], &[&alley, &shop_house, &any]);
        if got < n_hide {
            pick(&mut kind, &mut rep, K::Hideout, n_hide - got, &[Z::Sump], &[&alley, &shop_house, &any], false);
        }
        // Track B (OPEN-136): the Markets go where the households live, as the
        // generator puts a borough's food beside its homes: one a district
        // first, the rest by the district's household slots (largest
        // remainder; a cut's slots are its flats' share of the households, at
        // least one, as dealt below); within a district shop fronts,
        // commercial cuts, shop-houses, then (C6a) any unit there, then the
        // cuts with the fewest flats. What a district cannot place goes by
        // the S3c rule.
        let n_markets = target(1.5);
        let lives = |i: usize| {
            kind[i].is_none()
                && segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse | Use::Commercial))
        };
        let all_flats: u64 = (0..segs.len()).filter(|&i| lives(i)).map(|i| u64::from(segs[i].flats)).sum();
        let want_hh = u64::from(args.residents.div_ceil(RESIDENTS_PER_SLOT));
        let mut slots_of_d: BTreeMap<usize, u64> = BTreeMap::new();
        for i in (0..segs.len()).filter(|&i| lives(i)) {
            let slots = (want_hh * u64::from(segs[i].flats)).checked_div(all_flats).unwrap_or(0).max(1);
            *slots_of_d.entry(segs[i].district).or_default() += slots;
        }
        // One a district first (when there are enough), the rest by share.
        let one_each = n_markets >= slots_of_d.len();
        let rest_n = if one_each { n_markets - slots_of_d.len() } else { n_markets };
        let mut deal: BTreeMap<usize, usize> = slots_of_d.keys().map(|&d| (d, usize::from(one_each))).collect();
        for (d, n) in largest_remainder(&slots_of_d, rest_n) {
            *deal.entry(d).or_default() += n;
        }
        for (d, n) in deal.into_iter().filter(|&(_, n)| n > 0) {
            let in_d = |i: usize| segs[i].district == d;
            let (sf, cm, sh) = (
                |i: usize| in_d(i) && shopfront(i),
                |i: usize| in_d(i) && commercial(i),
                |i: usize| in_d(i) && shop_house(i),
            );
            let got = pick_units(&kind, &mut free, &mut uses, K::Market, n, &[], &[&sf, &cm, &sh, &in_d]);
            fewest_flats(&mut kind, &uses, segs, K::Market, n.saturating_sub(got), &|i| in_d(i) && segs[i].flats > 0);
        }
        let have_markets = n_uses(&kind, &uses, K::Market);
        let more = n_markets.saturating_sub(have_markets);
        let got = pick_units(
            &kind,
            &mut free,
            &mut uses,
            K::Market,
            more,
            &[Z::Sump, Z::Mid],
            &[&shopfront, &commercial, &any],
        );
        if got < more {
            pick(
                &mut kind,
                &mut rep,
                K::Market,
                more - got,
                &[Z::Sump, Z::Mid],
                &[&shopfront, &commercial, &any],
                false,
            );
        }
        let n_bars = target(1.5);
        let got = pick_units(
            &kind,
            &mut free,
            &mut uses,
            K::Bar,
            n_bars,
            &[Z::Mid, Z::Sump],
            &[&commercial, &shopfront, &any],
        );
        if got < n_bars {
            pick(
                &mut kind,
                &mut rep,
                K::Bar,
                n_bars - got,
                &[Z::Mid, Z::Sump],
                &[&commercial, &shopfront, &any],
                false,
            );
        }
        // Lots: the leftover industrial, civic and vacant cuts nothing stands
        // on, whole; then (C6a) the generator's Lots into units, zone by zone.
        for (i, sg) in segs.iter().enumerate() {
            if kind[i].is_none()
                && uses[i].is_empty()
                && sg.use_.is_some_and(|u| u.industrial() || matches!(u, Use::Civic | Use::Vacant))
            {
                kind[i] = Some(K::Lot);
            }
        }
        // Whole-cut Lots never past the generator's Lots to Blocks (80 to
        // 200): a coarse import has too few cuts for both, and the Blocks
        // come first. A Lot in a unit takes no Block (C6a).
        let whole_lots = kind.iter().filter(|x| **x == Some(K::Lot)).count();
        let housing = (0..segs.len())
            .filter(|&i| {
                kind[i].is_none()
                    && segs[i].use_.is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse | Use::Commercial))
            })
            .count();
        let mut room = ((housing as f64 * 0.4 - whole_lots as f64) / 1.4).max(0.0).floor() as usize;
        let want_lots = target(80.0);
        let mut budget = want_lots.saturating_sub(n_uses(&kind, &uses, K::Lot));
        let no_flats = |i: usize| commercial(i) && segs[i].flats == 0;
        // Track B (OPEN-136): first each zone's share of the generator's Lots
        // (`GEN_LOTS_BY_ZONE`, less the leftovers already there), so the
        // Spire has the Lots its Hqs and Lounges need (`founding::tier_ok`)
        // and the Mid its Gunsmiths and Schools: in the zone (C6a) the units
        // of commercial cuts without flats, commercial, shop-houses, any;
        // then the whole cuts with the fewest flats (the households follow
        // the flats, so a small tower gives up its slot, not its people),
        // within the room. The rest as before, anywhere.
        let in_zone_lots = |kind: &Vec<Option<BuildingKind>>, uses: &Vec<Vec<(BuildingKind, Unit)>>, zs: &[Zone]| {
            (0..segs.len())
                .filter(|&i| zs.contains(&zone(i)))
                .map(|i| usize::from(kind[i] == Some(K::Lot)) + uses[i].iter().filter(|x| x.0 == K::Lot).count())
                .sum::<usize>()
        };
        // C6a: a zone's Lots are dealt over its districts by their units
        // (the sites: D2's office stacks are the Civic Core's work, the Vats'
        // works floors theirs), a zone with none (the Spire) by its
        // household slots; the rest of the Lots anywhere by the household
        // slots (as the Markets: the work goes where the people live).
        let deal_lots = |kind: &mut Vec<Option<BuildingKind>>,
                         free: &mut Vec<Vec<Unit>>,
                         uses: &mut Vec<Vec<(BuildingKind, Unit)>>,
                         room: &mut usize,
                         zs: Option<&[Zone]>,
                         n: usize|
         -> usize {
            let in_z = |i: usize| zs.is_none_or(|zs| zs.contains(&zone(i)));
            let mut w: BTreeMap<usize, u64> = BTreeMap::new();
            if zs.is_some() {
                for i in (0..segs.len()).filter(|&i| in_z(i) && kind[i].is_none()) {
                    *w.entry(segs[i].district).or_default() += free[i].len() as u64;
                }
            }
            if w.values().all(|&v| v == 0) {
                w = slots_of_d
                    .iter()
                    .filter(|(&d, _)| zone_of_d.get(d).is_some_and(|z| zs.is_none_or(|zs| zs.contains(z))))
                    .map(|(&d, &n)| (d, n))
                    .collect();
            }
            let mut got = 0;
            for (d, nd) in largest_remainder(&w, n) {
                let in_d = |i: usize| segs[i].district == d && in_z(i);
                let (nf, cm, sh) = (
                    |i: usize| in_d(i) && no_flats(i),
                    |i: usize| in_d(i) && commercial(i),
                    |i: usize| in_d(i) && shop_house(i),
                );
                let g = pick_units(kind, free, uses, K::Lot, nd, &[], &[&sh, &cm, &nf, &in_d]);
                let homes = |i: usize| {
                    in_d(i)
                        && segs[i]
                            .use_
                            .is_some_and(|u| matches!(u, Use::Residential | Use::ShopHouse | Use::Commercial))
                };
                let more = fewest_flats(kind, uses, segs, K::Lot, (nd - g).min(*room), &homes);
                *room -= more;
                got += g + more;
            }
            got
        };
        for &(zs, per) in GEN_LOTS_BY_ZONE {
            let have = in_zone_lots(&kind, &uses, zs);
            let n = target(per).saturating_sub(have).min(budget);
            budget -= deal_lots(&mut kind, &mut free, &mut uses, &mut room, Some(zs), n);
        }
        if budget > 0 {
            budget -= deal_lots(&mut kind, &mut free, &mut uses, &mut room, None, budget);
        }
        if budget > 0 {
            let got = pick_units(
                &kind,
                &mut free,
                &mut uses,
                K::Lot,
                budget,
                &[],
                &[&shop_house, &commercial, &no_flats, &any],
            );
            budget -= got;
        }
        if budget > 0 {
            rep.shortfall.push(format!(
                "Lot: {} of {want_lots} (no unit left, and more whole cuts would take Blocks past the generator's 80 Lots to 200 Blocks)",
                want_lots - budget
            ));
        }
    }
    // The rest: Blocks where people live, open Lots elsewhere (S3b: in the
    // Vats and Civic; S3c: commercial cuts without flats).
    let rest = |i: usize| match segs[i].use_ {
        None if matches!(zone(i), Z::Vats | Z::Civic) => K::Lot,
        None => K::Home,
        Some(Use::Residential | Use::ShopHouse) => K::Home,
        Some(Use::Commercial) if segs[i].flats > 0 => K::Home,
        Some(_) => K::Lot,
    };
    // The city milestone C6a: each cut's host and its tenancies. A cut dealt
    // whole stands as its kind; one where people live (the S3c rule's Home)
    // is a Home on its flats' storeys holding the uses dealt into its units
    // as tenancies; one where nobody lives, its lowest use the host and the
    // rest tenancies on it.
    let mut kinds: Vec<BuildingKind> = Vec::with_capacity(segs.len());
    let mut host_span: Vec<Option<(u8, u8)>> = Vec::with_capacity(segs.len());
    let mut tenancies: Vec<Vec<(BuildingKind, Unit)>> = Vec::with_capacity(segs.len());
    for i in 0..segs.len() {
        let mut ts = std::mem::take(&mut uses[i]);
        let (k, span) = match kind[i] {
            Some(k) => (k, (k == K::Home).then_some(segs[i].flat_span).flatten()),
            None if ts.is_empty() || rest(i) == K::Home => {
                let k = rest(i);
                (k, (k == K::Home).then_some(segs[i].flat_span).flatten())
            }
            None => {
                let (k, u) = ts.remove(0);
                (k, Some((u.level, u.level)))
            }
        };
        kinds.push(k);
        host_span.push(span);
        tenancies.push(ts);
    }
    // Home floor slots (a household each, `RESIDENTS_PER_SLOT` residents).
    // S3b: a Home's floors. S3c: the `--residents` households (one a slot)
    // dealt over the Homes by their flats (largest remainder), at least one
    // slot a Home; World::new deals households by stride over the slots, so
    // a tower holds its flats' share and the beds come to at least N.
    let homes: Vec<usize> = (0..segs.len()).filter(|&i| kinds[i] == K::Home).collect();
    let mut slots: Vec<u32> = segs.iter().map(|sg| sg.floors).collect();
    if by_use && !homes.is_empty() {
        let want = u64::from(args.residents.div_ceil(RESIDENTS_PER_SLOT));
        let total: u64 = homes.iter().map(|&i| u64::from(segs[i].flats)).sum();
        rep.home_flats = total;
        let n = homes.len() as u64;
        let mut rems: Vec<(u64, usize)> = Vec::with_capacity(homes.len());
        let mut dealt = 0u64;
        for (j, &i) in homes.iter().enumerate() {
            let p = want * u64::from(segs[i].flats);
            // No flats anywhere: an even deal.
            let (q, r) = match (p.checked_div(total), p.checked_rem(total)) {
                (Some(q), Some(r)) => (q, r),
                _ => (want / n + u64::from((j as u64) < want % n), 0),
            };
            slots[i] = u32::try_from(q).unwrap_or(u32::MAX - 1);
            dealt += q;
            rems.push((r, i));
        }
        rems.sort_by_key(|&(r, i)| (std::cmp::Reverse(r), i));
        for &(_, i) in rems.iter().take(want.saturating_sub(dealt) as usize) {
            slots[i] += 1;
        }
        for &i in &homes {
            slots[i] = slots[i].max(1);
        }
        rep.slots_capped = homes.iter().filter(|&&i| slots[i] > 255).count();
    }

    let mut out_b: Vec<OutB> = Vec::new();
    let mut doors: Vec<(usize, usize)> = Vec::new();
    // C6a: per entry, its cut and whether it is a tenancy (its host is the
    // cut's host entry, the first of the cut's entries).
    let mut entry_of: Vec<(usize, bool)> = Vec::new();
    for (i, sg) in segs.iter().enumerate() {
        let z = zone(i);
        let kd = kinds[i];
        let floors = if kd == K::Home { slots[i].clamp(1, 255) as u8 } else { 1 };
        if kd == K::Home {
            rep.home_floors += usize::from(floors);
        }
        let n_units = u16::try_from(sg.units.len()).unwrap_or(u16::MAX);
        let extra = if by_use {
            BuildingExtra {
                flats: (kd == K::Home).then_some(sg.flats),
                storeys: Some(u16::try_from(sg.floors).unwrap_or(u16::MAX)),
                lot: door_lot(&f, sg),
                span: host_span[i],
                units: (n_units > 0).then_some(n_units),
                lift: sg.lift,
                ..BuildingExtra::default()
            }
        } else {
            BuildingExtra { lot: door_lot(&f, sg), ..BuildingExtra::default() }
        };
        if extra.lot.is_some() {
            rep.lots_named += 1;
        }
        let dname = districts.get(sg.district).map_or_else(|| "-".to_string(), |d| d.name.clone());
        let u = sg.use_.map_or("-".to_string(), |u| format!("{u:?}"));
        let count = |kd: BuildingKind, rep: &mut Report| {
            *rep.kinds.entry(kd).or_default() += 1;
            *rep.kind_use.entry((kd, u.clone())).or_default() += 1;
            *rep.dealt_by_district.entry(dname.clone()).or_default().entry(kd).or_default() += 1;
        };
        count(kd, &mut rep);
        rep.hosts += usize::from(!tenancies[i].is_empty());
        out_b.push((kd, sg.rect, tier_of(z), floors, extra.clone()));
        doors.push(sg.door);
        entry_of.push((i, false));
        // Its tenancies: each on its unit's storey, through its lot's street
        // door where that is a wall tile of the cut facing an open road (not
        // the host's), else the host's door.
        let (x0, y0) = (usize::from(sg.rect.x), usize::from(sg.rect.y));
        let walls: Vec<((usize, usize), (usize, usize))> =
            wall_doors((x0, y0, x0 + usize::from(sg.rect.w), y0 + usize::from(sg.rect.h)), w, h)
                .into_iter()
                .filter(|&(d, (ox, oy))| d != sg.door && road(&tiles, ox, oy) && !occupied[at(ox, oy)])
                .collect();
        for &(tk, unit) in &tenancies[i] {
            let own = sg
                .lot_outs
                .get(unit.lot)
                .and_then(|outs| outs.iter().find_map(|o| walls.iter().find(|c| c.1 == *o).map(|c| c.0)));
            let lot = sg.lots.get(unit.lot).map(|l| l.0.clone());
            count(tk, &mut rep);
            rep.tenancies += 1;
            rep.tenancy_doors += usize::from(own.is_some());
            out_b.push((
                tk,
                sg.rect,
                tier_of(z),
                1,
                BuildingExtra {
                    storeys: Some(u16::try_from(sg.floors).unwrap_or(u16::MAX)),
                    lot,
                    span: Some((unit.level, unit.level)),
                    door: own.map(|(x, y)| (x as u16, y as u16)),
                    // the host's index, set once the order is known
                    within: Some(0),
                    ..BuildingExtra::default()
                },
            ));
            doors.push(own.unwrap_or(sg.door));
            entry_of.push((i, true));
        }
    }

    // --- stamp ------------------------------------------------------------------
    let hosting: Vec<bool> = (0..segs.len()).map(|i| !tenancies[i].is_empty()).collect();
    for (((kd, r, ..), &(dx, dy)), &(i, tenancy)) in out_b.iter().zip(&doors).zip(&entry_of) {
        if !tenancy {
            let (x0, y0, x1, y1) = (
                usize::from(r.x),
                usize::from(r.y),
                usize::from(r.x) + usize::from(r.w),
                usize::from(r.y) + usize::from(r.h),
            );
            for y in y0..y1 {
                for x in x0..x1 {
                    let edge = y == y0 || y + 1 == y1 || x == x0 || x + 1 == x1;
                    tiles[at(x, y)] = match (kd, edge) {
                        // C6a: a Lot that hosts tenancies is a walled structure.
                        (K::Lot, true) if hosting[i] => '#',
                        (K::Lot, _) => '.',
                        (_, true) => '#',
                        (K::Farm, false) => 'f',
                        _ => '.',
                    };
                }
            }
        }
        tiles[at(dx, dy)] = 'D';
    }
    // File order: kind, then row, then column (the shipped map's order); a
    // cut's entries keep their order among equals (stable).
    let korder = |kd: BuildingKind| BuildingKind::ALL.iter().position(|&x| x == kd).unwrap_or(usize::MAX);
    let mut order: Vec<usize> = (0..out_b.len()).collect();
    order.sort_by_key(|&e| (korder(out_b[e].0), out_b[e].1.y, out_b[e].1.x));
    let mut at_line = vec![0u32; out_b.len()];
    for (line, &e) in order.iter().enumerate() {
        at_line[e] = line as u32;
    }
    let mut host_line = vec![0u32; segs.len()];
    for (e, &(i, tenancy)) in entry_of.iter().enumerate() {
        if !tenancy {
            host_line[i] = at_line[e];
        }
    }
    for (e, &(i, tenancy)) in entry_of.iter().enumerate() {
        if tenancy {
            out_b[e].4.within = Some(host_line[i]);
        }
    }
    let mut sorted: Vec<Option<OutB>> = out_b.into_iter().map(Some).collect();
    let out_b: Vec<OutB> = order.iter().map(|&e| sorted[e].take().expect("each entry once")).collect();

    // --- El lines ---------------------------------------------------------------
    let mut lines: Vec<TransitLineCfg> = Vec::new();
    let mut comments: Vec<String> = vec![
        format!(
            "imported from TheCityGraphics by citysim-cli import-graphics (Scale S3c): downsample {k}, {s} m tiles, crop {}, employers and beds for {} residents",
            args.crop.as_deref().unwrap_or("none"),
            args.residents
        ),
        format!("inputs: {}", rep.inputs.join("; ")),
        "docs/IMPORT_GRAPHICS.md: the mapping and what is not mapped".into(),
    ];
    let elements = arr(&elevated_json, "elements", "elevated.json")?;
    let mut unmapped_el: BTreeMap<String, usize> = BTreeMap::new();
    for (i, e) in elements.iter().enumerate() {
        let kd = text(e, "kind", &format!("elevated.json elements[{i}]"))?;
        // C7: the walkways and the escalator are the P lines (walkways_sim.json) when it is read.
        let portal = !raw_portals.is_empty() && (kd == "walkway" || kd == "escalator");
        if kd != "rail" && kd != "station" && kd != "funicular" && !portal {
            *unmapped_el.entry(kd.to_string()).or_default() += 1;
        }
    }
    if let Some(r) = &rail_json {
        // S3c: sim/rail.json, each line by its name with its stations kept
        // as waypoints (a waypoint is a stop); its service is not read (the
        // sim's `[transit]` headway and dwell run every line).
        for (i, l) in arr(r, "lines", "sim/rail.json")?.iter().enumerate() {
            let ctx = format!("sim/rail.json lines[{i}]");
            let name = text(l, "name", &ctx)?;
            let pts = points(l, "pts", &ctx)?;
            let u: Vec<f64> = arr(l, "u", &ctx)?.iter().map(|v| num(v, &ctx)).collect::<Result<_, _>>()?;
            if u.len() != pts.len() {
                return Err(format!("{ctx}: {} pts but {} u", pts.len(), u.len()));
            }
            let mut keep = BTreeSet::new();
            let mut names = Vec::new();
            for (j, st) in arr(l, "stops", &ctx)?.iter().enumerate() {
                let c = format!("{ctx} stops[{j}]");
                let su = num(get(st, "u", &c)?, &format!("{c} \"u\""))?;
                names.push(text(st, "name", &c)?.to_string());
                let at = (0..u.len()).min_by(|&a, &b| (u[a] - su).abs().total_cmp(&(u[b] - su).abs())).unwrap_or(0);
                keep.insert(at);
            }
            match line_waypoints(&f, &pts, &keep) {
                Some((wp, stations)) => {
                    comments.push(format!(
                        "El line {name}: stations {} (sim/rail.json; headway {} s, {} s at night, {} m/s there; the sim's [transit] timetable runs it)",
                        names.join(", "),
                        l.get("headway_s").and_then(Value::as_f64).unwrap_or(0.0),
                        l.get("night_headway_s").and_then(Value::as_f64).unwrap_or(0.0),
                        l.get("speed_mps").and_then(Value::as_f64).unwrap_or(0.0)
                    ));
                    lines.push(TransitLineCfg { name: name.to_string(), waypoints: wp, in_flight: false, stations });
                }
                None => rep.warnings.push(format!("{ctx} ({name}): under two track points in the window")),
            }
        }
        rep.unmapped.push(
            "sim/rail.json service (headway_s, night_headway_s, service_hours, dwell_s, speed_mps, trains, cars): the sim's [transit] timetable runs every line".into(),
        );
    } else {
        let mut stations: Vec<(f64, f64)> = Vec::new();
        for (i, e) in elements.iter().enumerate() {
            let ctx = format!("elevated.json elements[{i}]");
            if text(e, "kind", &ctx)? == "station" {
                let p = points(e, "pts", &ctx)?;
                if !p.is_empty() {
                    let n = p.len() as f64;
                    stations.push((p.iter().map(|q| q.0).sum::<f64>() / n, p.iter().map(|q| q.1).sum::<f64>() / n));
                }
            }
        }
        for (i, e) in elements.iter().enumerate() {
            let ctx = format!("elevated.json elements[{i}]");
            if text(e, "kind", &ctx)? != "rail" {
                continue;
            }
            let id = text(e, "id", &ctx)?;
            let pts = points(e, "pts", &ctx)?;
            if pts.len() < 2 {
                continue;
            }
            // Each station's nearest track point is kept (a waypoint is a stop).
            let keep: BTreeSet<usize> = stations
                .iter()
                .map(|&(sx, sy)| {
                    (0..pts.len())
                        .min_by(|&a, &b| {
                            let da = (pts[a].0 - sx).powi(2) + (pts[a].1 - sy).powi(2);
                            let db = (pts[b].0 - sx).powi(2) + (pts[b].1 - sy).powi(2);
                            da.total_cmp(&db)
                        })
                        .unwrap_or(0)
                })
                .collect();
            match line_waypoints(&f, &pts, &keep) {
                Some((wp, stations)) => {
                    lines.push(TransitLineCfg { name: format!("El {id}"), waypoints: wp, in_flight: false, stations })
                }
                None => rep.warnings.push(format!("{ctx} ({id}): under two track points in the window")),
            }
        }
    }
    // The pre-view pass phase 5 (P11): the lines in flight go after the
    // export's (the God's line rows keep their numbers).
    let mut flight: Vec<TransitLineCfg> = Vec::new();
    // The pre-view pass phase 5 (P11): the funicular (the Peak's tram up the
    // ridge) is a line between its two stair portals (its ends), in flight
    // (`L+`) until the phase lands. The escalator stays unmapped: a moving
    // walkway at a walk's pace on a ground the sim reads as flat.
    for (i, e) in elements.iter().enumerate() {
        let ctx = format!("elevated.json elements[{i}]");
        if text(e, "kind", &ctx)? != "funicular" {
            continue;
        }
        let pts = points(e, "pts", &ctx)?;
        if pts.len() < 2 {
            continue;
        }
        let keep: BTreeSet<usize> = [0, pts.len() - 1].into_iter().collect();
        match line_waypoints(&f, &pts, &keep) {
            Some((wp, stations)) => {
                flight.push(TransitLineCfg { name: "Funicular".into(), waypoints: wp, in_flight: true, stations })
            }
            None => rep.warnings.push(format!("{ctx} (funicular): under two track points in the window")),
        }
    }
    for (kd, n) in &unmapped_el {
        rep.unmapped.push(format!("elevated.json: {n} \"{kd}\" element(s) (no sim counterpart)"));
    }
    // The tram (street/tram.json): its loop out to the terminus is a line,
    // named after its street (street/names.json) where that file names it.
    let street_names = read_json(&src.join("street"), "names.json").ok();
    match read_json(&src.join("street"), "tram.json") {
        Err(e) => rep.warnings.push(format!("{e}; no tram line")),
        Ok(t) => {
            for (i, l) in arr(&t, "lines", "tram.json")?.iter().enumerate() {
                let ctx = format!("tram.json lines[{i}]");
                let way = l.get("way").and_then(|v| v.as_str()).unwrap_or("tram");
                let street = street_names.as_ref().and_then(|n| n.get(way)).and_then(Value::as_str).unwrap_or(way);
                let pts = points(l, "pts", &ctx)?;
                let u: Vec<f64> = arr(l, "u", &ctx)?.iter().map(|v| num(v, &ctx)).collect::<Result<_, _>>()?;
                if u.len() != pts.len() {
                    return Err(format!("{ctx}: {} pts but {} u", pts.len(), u.len()));
                }
                let mut stops: Vec<(f64, String)> = Vec::new();
                for (j, st) in arr(l, "stops", &ctx)?.iter().enumerate() {
                    let a = st
                        .as_array()
                        .filter(|a| a.len() >= 3)
                        .ok_or_else(|| format!("{ctx} stops[{j}]: not [u, dwell, name]"))?;
                    stops.push((num(&a[0], &ctx)?, a[2].as_str().unwrap_or("").to_string()));
                }
                // The far terminus (the loop starts at one): out on one track, back on the other.
                let end_u = stops
                    .iter()
                    .find(|(su, n)| n == "terminus" && *su > 0.0)
                    .map_or(u.last().copied().unwrap_or(0.0), |s| s.0);
                let n = u.iter().position(|&x| x > end_u).unwrap_or(u.len());
                let half = &pts[..n.max(2).min(pts.len())];
                let keep: BTreeSet<usize> = stops
                    .iter()
                    .filter(|(su, _)| *su <= end_u)
                    .map(|(su, _)| u.iter().position(|&x| x >= *su).unwrap_or(0).min(half.len() - 1))
                    .collect();
                match line_waypoints(&f, half, &keep) {
                    Some((wp, stations)) => lines.push(TransitLineCfg {
                        name: format!("Tram {street}"),
                        waypoints: wp,
                        in_flight: false,
                        stations,
                    }),
                    None => rep.warnings.push(format!("{ctx}: under two tram points in the window")),
                }
            }
        }
    }
    lines.append(&mut flight);
    rep.lines = lines.iter().map(|l| format!("{} ({} waypoints)", l.name, l.waypoints.len())).collect();

    // --- what the sim does not read ---------------------------------------------
    for (cls, n) in &by_cls {
        if !matches!(cls.as_str(), "street" | "arterial" | "lane" | "alley" | "promenade" | "tram") {
            rep.unmapped.push(format!("roads.json: {n} \"{cls}\" ways read as plain Road (no grade or stairs)"));
        }
    }
    rep.unmapped.push(
        "lots.json: platform z, under_deck, pylons, frontage classes, walkway_join (the sim's ground is flat)".into(),
    );
    if sim_lots.is_some() {
        rep.unmapped.push(
            "lots_sim.json levels: a use dealt into a unit is a tenancy on its storey (C6a); the vacant units are the host's units= count, not entities; a workplace keeps one floor of places (its storeys are written as storeys=)".into(),
        );
    } else {
        rep.unmapped.push("lots.json floors: the district's range, not the lot's massing (a seeded pick in it)".into());
    }
    if terrain.is_none() {
        rep.unmapped
            .push("terrain: sea and mountain both read as Water (outside every block, way and superblock)".into());
    }
    rep.unmapped.push("roads.json nodes/edges, traffic.json lanes: vehicles are not simulated on roads".into());
    rep.unmapped.push(
        "sim_layout.toml rooms, portals, slots: a Home is 5 residents a floor slot; flats and shop units are counted, rooms are not placed"
            .into(),
    );

    let zones: Vec<char> =
        dgrid.iter().map(|&d| zone_of_d.get(usize::from(d)).copied().unwrap_or(Z::Mid).letter()).collect();
    let grid: Vec<u8> = dgrid.iter().map(|&d| d as u8).collect();
    let extra: Vec<BuildingExtra> = out_b.iter().map(|b| b.4.clone()).collect();
    rep.lift_hosts = extra.iter().filter(|e| e.lift.is_some()).count();
    // C7: the walkways as portals, on the final tiles.
    let portals = map_portals(&f, &tiles, &raw_portals, sim_lots.as_ref(), &mut rep);
    // V7: the carriageways and the marked crossings, on the final tiles.
    let (carriage, crossings) = if src.join("chunks").join("index.json").is_file() {
        read_crossings(src, &f, k, tile_src, &tiles, &roads_json, &mut rep)?
    } else {
        rep.warnings.push("no chunks/index.json: no carriageways or crossings (V7)".into());
        (Vec::new(), Vec::new())
    };
    let text_out = MapText {
        w,
        h,
        tiles,
        zones,
        buildings: out_b.into_iter().map(|(kd, r, t, fl, _)| (kd, r, t, fl)).collect(),
        districts,
        district_grid: grid,
        lines,
        portals,
        carriage,
        crossings,
        comments,
        extra,
    }
    .to_text(true);

    // Self-check: the sim's own parser must accept it.
    let check = text_out.clone();
    let parsed = std::panic::catch_unwind(move || citysim::Map::parse(&check));
    if let Err(e) = parsed {
        let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
        return Err(format!("the imported map does not parse: {}", msg.unwrap_or_default()));
    }
    Ok((text_out, rep))
}
