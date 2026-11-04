//! Scale S3b: the TheCityGraphics adapter (docs/IMPORT_GRAPHICS.md).
//!
//! `citysim-cli import-graphics --src <isle_m/out> --out <map> [--downsample k]
//! [--residents N] [--crop x0,y0,x1,y1]` reads the graphics pipeline's
//! generated city (read only: nothing is ever written under `--src`) and
//! writes the sim's own map text: roads from the street ways, buildings from
//! the blocks' platform parts cut at lot boundaries (floors and flats from the
//! per-lot `sim_layout.toml` where one exists), districts from the
//! superblocks, El lines from the elevated rail (and the tram, when its file
//! exists). Their format still evolves: a missing file or field is an error
//! naming the file, the record and the field, never a panic.
//!
//! Scale S3c: their sim export (`<src>/sim/`, schema `city-sim/0.1`,
//! SIM_NOTES "The sim export") is preferred where present, file by file:
//! `lots_sim.json` (per-lot floors, use, flats, shop units, street doors,
//! the named civic sites), `terrain_sim.json` (sea, urban land and mountain:
//! the hills become walkable Mountain), `rail.json` (the Terrace Line and its
//! named stations). Without it the S3b inputs stand; the report says which
//! were read.
//!
//! The city milestone C5: schema `city-sim/0.2`. `rail.json` holds every El
//! line (the Terrace, Hillside, Mid-Levels and Vats lines; the authored
//! `--lines` file retired), the tram is imported once from `tram.json`, and
//! 0.2's new fields (a lot's `z_m`, `levels`, `cores`; `walkways_sim.json`'s
//! portals) are read, checked and counted in the report.
//!
//! The city milestone C6a: the levels' units hold the uses. The employers,
//! venues and Hideouts are dealt into a cut's units (shops, workshops,
//! offices, a works floor), each a tenancy `B` line on the cut's host (`in=`,
//! `span=`, its own `door=` where its lot's street door is on the cut's
//! wall), so the cuts they took go back to Homes (docs/IMPORT_GRAPHICS.md).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::Value;

use citysim::components::TilePos;
use citysim::components::{BuildingKind, Rect, Zone};
use citysim::config::TransitLineCfg;
use citysim::map::{BuildingExtra, MapCrossing, MapDistrict, MapText};

mod build;
mod crossings;
mod layout;
mod read;
mod report;

pub use build::*;
use crossings::*;
use layout::*;
use read::*;
pub use report::*;

/// The sim export's schema this adapter reads (`<src>/sim/*.json`).
pub const SIM_SCHEMA: &str = "city-sim/0.2";

/// Residents a Home's floor slot (a household) holds: the 2k city's
/// `[world] residents_per_home`, the generator's convention.
const RESIDENTS_PER_SLOT: u32 = citysim::mapgen::RESIDENTS_PER_HOME;

#[derive(clap::Args, Clone, Debug)]
pub struct ImportArgs {
    /// The pipeline's output directory (`assets/city/isle_m/out`).
    #[arg(long)]
    pub src: PathBuf,
    /// The map file to write.
    #[arg(long)]
    pub out: PathBuf,
    /// Sim tiles per graphics tile along each axis (1: their 2 m tiles).
    #[arg(long, default_value_t = 1)]
    pub downsample: u32,
    /// Residents the employers are sized for (the generator's mix per 1,000).
    #[arg(long, default_value_t = 20_000)]
    pub residents: u32,
    /// Import only this window, in metres of their frame (x east, y north):
    /// `x0,y0,x1,y1`.
    #[arg(long)]
    pub crop: Option<String>,
}

/// `import-graphics`: build, write, report.
pub fn import(args: ImportArgs) -> Result<(), String> {
    let (text, rep) = build(&args)?;
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&args.out, &text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    println!("{}: {} bytes", args.out.display(), text.len());
    rep.print();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("citysim-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("tmp");
        d
    }

    fn args(src: &Path) -> ImportArgs {
        ImportArgs { src: src.to_path_buf(), out: src.join("out.txt"), downsample: 1, residents: 1_000, crop: None }
    }

    /// A tiny city in their format: one Sump superblock, one block with two
    /// parts either side of an alley, a street on each long side.
    fn write_city(d: &Path, lots_field: &str) {
        let districts = r#"{"rows":[{"id":"D1","name":"The Sump","zone":"Sump"}],
            "superblocks":[{"district":"D1","rect_m":[0,0,80,60]}]}"#;
        let blocks = r#"{"blocks":[{"id":"b0","district":"D1","rect_m":[0,12,80,48],
            "parts_m":[[0,12,80,28],[0,30,80,48]]}],"open":[],"greens_m":[],"leftover_m":[],"voids_m":[]}"#;
        let mut lots = Vec::new();
        for i in 0..10 {
            let x = f64::from(i) * 8.0;
            lots.push(format!(
                r#"{{"id":"l{i}","block":"b0","front":"S","{lots_field}":[{x},12,{},28],"floors":[4,12],"seed":{i}}}"#,
                x + 8.0
            ));
            lots.push(format!(
                r#"{{"id":"m{i}","block":"b0","front":"N","rect_m":[{x},30,{},48],"floors":[4,12],"seed":{}}}"#,
                x + 8.0,
                i + 10
            ));
        }
        let lots = format!(r#"{{"tile":2.0,"lots":[{}]}}"#, lots.join(","));
        let roads = r#"{"ways":[{"cls":"street","rect_m":[0,0,80,12]},{"cls":"alley","rect_m":[0,28,80,30]},
            {"cls":"street","rect_m":[0,48,80,60]},{"cls":"street","rect_m":[0,0,4,60]}]}"#;
        let elevated = r#"{"elements":[{"id":"rail","kind":"rail","pts":[[2,6,40],[78,6,40]]},
            {"id":"escalator","kind":"escalator","pts":[[2,2,0],[3,3,0]]}]}"#;
        let terrain = r#"{"summary":{"size_m":[80,60]}}"#;
        for (n, t) in [
            ("districts.json", districts),
            ("blocks.json", blocks),
            ("lots.json", lots.as_str()),
            ("roads.json", roads),
            ("elevated.json", elevated),
            ("terrain.json", terrain),
        ] {
            std::fs::write(d.join(n), t).expect("write");
        }
    }

    #[test]
    fn test_import_tiny_city() {
        let d = tmp("tiny");
        write_city(&d, "rect_m");
        let (text, rep) = build(&args(&d)).expect("imports");
        let m = citysim::Map::parse(&text);
        assert_eq!((m.w(), m.h()), (40, 30));
        assert_eq!(m.districts.len(), 1);
        assert_eq!(m.lines.len(), 1);
        assert!(rep.segments >= 4, "{rep:?}");
        assert!(m.count_kind(BuildingKind::Home) + m.count_kind(BuildingKind::Lot) > 0);
        assert!(!m.edge_roads().is_empty());
        assert!(rep.unmapped.iter().any(|u| u.contains("escalator")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn test_import_missing_field_is_an_error_not_a_panic() {
        let d = tmp("missing");
        write_city(&d, "rect");
        let err = build(&args(&d)).expect_err("lots without rect_m");
        assert!(err.contains("lots.json lots[0] (l0): missing field \"rect_m\""), "{err}");
        std::fs::remove_file(d.join("roads.json")).expect("rm");
        let err = build(&args(&d)).expect_err("no roads.json");
        assert!(err.contains("roads.json"), "{err}");
        let err = build(&ImportArgs { src: d.join("nope"), ..args(&d) }).expect_err("no dir");
        assert!(err.contains("not a directory"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// S3c: the tiny city with their sim export (`sim/`): the hills north of
    /// the city (outside every block and way) are Mountain, the named civic
    /// site is the Hall, the Homes carry their flats, the rail line its name.
    fn write_sim(d: &Path, schema: &str, drop_use_of: Option<&str>) {
        std::fs::write(d.join("terrain.json"), r#"{"summary":{"size_m":[80,70]}}"#).expect("write");
        let sim = d.join("sim");
        std::fs::create_dir_all(&sim).expect("mkdir");
        let mut lots = Vec::new();
        for i in 0..10 {
            let x = f64::from(i) * 8.0 + 4.0;
            let (use_s, ground, flats, civic) =
                if i == 0 { ("civic", "civic", 0, r#","civic":"city_hall""#) } else { ("shop_house", "shop", 3, "") };
            lots.push(format!(
                r#""m{i}":{{"floors":5,"use":"{use_s}","ground":"{ground}","flats":{flats},"shop_units":1,"source":"estimate","doors_m":[{{"at":[{x},48],"out":[{x},49],"side":"N"}}]{civic}}}"#
            ));
            let use_field =
                if drop_use_of == Some(&format!("l{i}")) { String::new() } else { r#""use":"residential","#.into() };
            lots.push(format!(
                r#""l{i}":{{"floors":6,{use_field}"ground":"lobby","flats":{},"shop_units":0,"source":"estimate","doors_m":[{{"at":[{x},12],"out":[{x},11],"side":"S"}}]}}"#,
                4 + i
            ));
        }
        let lots = format!(r#"{{"schema":"{schema}","lots":{{{}}}}}"#, lots.join(","));
        let rows: Vec<String> =
            (0..35).map(|r| if r < 5 { "\"40m\"".to_string() } else { "\"40u\"".to_string() }).collect();
        let terrain = format!(r#"{{"schema":"{schema}","tile_m":2,"nx":40,"ny":35,"rows":[{}]}}"#, rows.join(","));
        let rail = format!(
            r#"{{"schema":"{schema}","lines":[{{"name":"Test Line","pts":[[2,6,40],[40,6,40],[78,6,40]],"u":[0,38,76],
            "stops":[{{"name":"West","u":0}},{{"name":"East","u":76}}],"headway_s":240}}]}}"#
        );
        for (n, t) in [("lots_sim.json", lots), ("terrain_sim.json", terrain), ("rail.json", rail)] {
            std::fs::write(sim.join(n), t).expect("write");
        }
    }

    #[test]
    fn test_import_reads_the_sim_export() {
        let d = tmp("sim");
        write_city(&d, "rect_m");
        write_sim(&d, "city-sim/0.9", None);
        let (text, rep) = build(&args(&d)).expect("imports");
        let m = citysim::Map::parse(&text);
        assert_eq!((m.w(), m.h()), (40, 35));
        assert_eq!(m.tiles().iter().filter(|&&t| t == citysim::TileKind::Mountain).count(), 200, "{rep:?}");
        assert!(m.walkable(citysim::TilePos::new(10, 2)));
        assert_eq!(m.lines.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Test Line"]);
        assert_eq!(m.count_kind(BuildingKind::Hall), 1);
        assert!(m.buildings.iter().any(|b| b.kind == BuildingKind::Home && b.flats.is_some_and(|f| f > 0)));
        assert!(rep.doors_exact > 0, "{rep:?}");
        assert!(rep.inputs.iter().any(|i| i.starts_with("lots: sim/lots_sim.json")));
        assert!(rep.warnings.iter().any(|w| w.contains("schema \"city-sim/0.9\"")), "{:?}", rep.warnings);
        // A record missing a field fails naming the file, the record and the field.
        write_sim(&d, SIM_SCHEMA, Some("l3"));
        let err = build(&args(&d)).expect_err("a lot without use");
        assert!(err.contains("sim/lots_sim.json lots.l3: missing field \"use\""), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Track B (OPEN-136): the tiny city's south row (the `l` lots, all
    /// residential towers) is a Spire district. Before, a Lot only came from
    /// commercial cuts and shop-houses, so a Spire of towers had none and the
    /// megacorps' Hqs (`founding::tier_ok`: the Spire only) never stood; now
    /// the Spire takes its share of the generator's Lots from its smallest
    /// towers and keeps Homes. A Market stands (one a district when there
    /// are enough; at 200 residents the mix is one).
    #[test]
    fn test_import_deals_lots_and_markets_by_zone() {
        let d = tmp("zones");
        write_city(&d, "rect_m");
        write_sim(&d, SIM_SCHEMA, None);
        let districts = r#"{"rows":[{"id":"D1","name":"The Sump","zone":"Sump"},{"id":"D2","name":"The Spire","zone":"Spire"}],
            "superblocks":[{"district":"D1","rect_m":[0,30,80,70]},{"district":"D2","rect_m":[0,0,80,29]}]}"#;
        std::fs::write(d.join("districts.json"), districts).expect("write");
        let (text, rep) = build(&ImportArgs { residents: 200, ..args(&d) }).expect("imports");
        let m = citysim::Map::parse(&text);
        let spire = |b: &citysim::map::MapBuilding| m.zone(b.door) == Zone::Spire;
        let count =
            |k: BuildingKind, inside: bool| m.buildings.iter().filter(|b| b.kind == k && spire(b) == inside).count();
        assert!(count(BuildingKind::Lot, true) >= 1, "no Spire Lot: {rep:?}");
        assert!(count(BuildingKind::Home, true) >= 1, "the Spire lost every Home: {rep:?}");
        assert!(count(BuildingKind::Market, true) + count(BuildingKind::Market, false) >= 1, "{rep:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
