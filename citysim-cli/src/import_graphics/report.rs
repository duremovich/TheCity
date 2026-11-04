use super::*;

/// What the import read, wrote and could not map.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub w: usize,
    pub h: usize,
    pub tile_m: f64,
    pub lots: usize,
    pub lots_with_layout: usize,
    pub parts: usize,
    pub segments: usize,
    pub dropped_small: usize,
    pub dropped_no_road: usize,
    pub dropped_overlap: usize,
    pub lots_dropped: usize,
    pub flats: usize,
    pub shop_units: usize,
    pub kinds: BTreeMap<BuildingKind, usize>,
    pub home_floors: usize,
    pub shortfall: Vec<String>,
    pub districts: Vec<String>,
    pub lines: Vec<String>,
    pub piers: usize,
    pub unmapped: Vec<String>,
    pub warnings: Vec<String>,
    /// S3c: which inputs were read (the sim export or the S3b fallbacks).
    pub inputs: Vec<String>,
    /// S3c: the residents the employers and beds are sized for.
    pub residents: u32,
    /// S3c: doors placed on their `doors_m` `out` tile, on the nearest
    /// road-facing wall tile to it, or by the S3b side search.
    pub doors_exact: usize,
    pub doors_near: usize,
    pub doors_searched: usize,
    /// Doors moved off an outside tile a later cut built over.
    pub doors_moved: usize,
    /// The perf pass phase 10: `B` lines that name their door's lot (`lot=`).
    pub lots_named: usize,
    /// The perf pass phase 10: cuts holding no lot centre (no `lot=`, no venue).
    pub pieces: usize,
    /// S3c: flats in the Homes (their estimate) and Home floor slots capped at 255.
    pub home_flats: u64,
    pub slots_capped: usize,
    /// S3c: sim tiles read as Mountain and as sea from `terrain_sim.json`
    /// (before blocks and roads are laid over them).
    pub mountain_tiles: usize,
    pub sea_tiles: usize,
    /// S3c: buildings by kind and their cut's `use` (`-` without the export).
    pub kind_use: BTreeMap<(BuildingKind, String), usize>,
    /// C5 (`city-sim/0.2`): the lots' levels read, plate floors among them,
    /// and their units by kind: flats, shops, workshops, offices.
    pub levels: usize,
    pub plate_levels: usize,
    pub level_units: [u64; 4],
    /// C5: stair (`flight`) and `lift` cores read.
    pub cores_flight: usize,
    pub cores_lift: usize,
    /// C5: `walkways_sim.json` portals by kind, and their metres.
    pub portals: BTreeMap<String, usize>,
    pub portal_m: f64,
    /// C7: the portals mapped (`P` lines) by kind, the pieces a long deck
    /// segment was cut into, and the portals not mapped by why.
    pub portals_mapped: BTreeMap<String, usize>,
    pub portal_lines: usize,
    pub portals_skipped: BTreeMap<String, usize>,
    /// C7: the hosts given a `lift=`.
    pub lift_hosts: usize,
    /// V7: the ground chunks read, the carriageway tiles, the zebras read
    /// (deduplicated by their sim tiles), mapped as `X` lines, and dropped
    /// (no carriageway tile under them at the sim's tile size).
    pub ground_chunks: usize,
    pub carriage_tiles: usize,
    pub zebras_read: usize,
    pub crossings: usize,
    pub zebras_dropped: usize,
    /// C5 (for C6): the units each district's lots offer (flats, shops,
    /// workshops, offices, `works` grounds), by its `districts.json` id and
    /// name, beside the buildings this import dealt there by kind.
    pub units_by_district: BTreeMap<String, [u64; 5]>,
    pub dealt_by_district: BTreeMap<String, BTreeMap<BuildingKind, usize>>,
    /// C5: `lots_sim.json`'s `seed` (TheCityGraphics' city seed).
    pub export_seed: Option<u64>,
    /// The city milestone C6a: cuts that host tenancies, the tenancies, and
    /// the tenancies through their own lot's street door.
    pub hosts: usize,
    pub tenancies: usize,
    pub tenancy_doors: usize,
}

/// The generator's per-1,000 mix (docs/SCALE_20K.md § S3b), for the report.
const GEN_MIX: [(BuildingKind, f64); 7] = [
    (BuildingKind::Home, 200.0),
    (BuildingKind::Farm, 6.0),
    (BuildingKind::Market, 1.5),
    (BuildingKind::Bar, 1.5),
    (BuildingKind::Hideout, 1.0),
    (BuildingKind::SecurityOffice, 1.0),
    (BuildingKind::Lot, 80.0),
];

/// Track B (OPEN-136): the generator's 80 Lots per 1,000 by zone
/// (`mapgen`'s borough: the Spire's 10 + 10, the Mid's and the Civic row's 66,
/// the Sump's 16 + 8, the Vats' 50 per 2,000; `gen:20000` measures Spire
/// 200, Mid 638 + Civic 22, Sump 240, Vats 500), dealt zone by zone in this
/// order before the rest of the Lots go anywhere.
pub(super) const GEN_LOTS_BY_ZONE: &[(&[Zone], f64)] =
    &[(&[Zone::Spire], 10.0), (&[Zone::Mid, Zone::Civic], 33.0), (&[Zone::Sump], 12.0), (&[Zone::Vats], 25.0)];

/// `n` dealt over the keys by their weights (largest remainder, ties the
/// lower key); an even deal when every weight is 0. Keys with no share are
/// left out.
pub(super) fn largest_remainder(weights: &BTreeMap<usize, u64>, n: usize) -> Vec<(usize, usize)> {
    let total: u64 = weights.values().sum();
    let len = weights.len() as u64;
    if len == 0 {
        return Vec::new();
    }
    let n64 = n as u64;
    let mut out: Vec<(usize, usize, u64)> = weights
        .iter()
        .enumerate()
        .map(|(j, (&k, &w))| match ((n64 * w).checked_div(total), (n64 * w).checked_rem(total)) {
            (Some(q), Some(r)) => (k, q as usize, r),
            _ => (k, (n64 / len + u64::from((j as u64) < n64 % len)) as usize, 0),
        })
        .collect();
    let dealt: usize = out.iter().map(|o| o.1).sum();
    let mut order: Vec<usize> = (0..out.len()).collect();
    order.sort_by_key(|&j| (std::cmp::Reverse(out[j].2), out[j].0));
    for &j in order.iter().take(n.saturating_sub(dealt)) {
        out[j].1 += 1;
    }
    out.into_iter().filter(|o| o.1 > 0).map(|o| (o.0, o.1)).collect()
}

/// Make `n` of the free cuts passing `ok` a `want`, the fewest flats first
/// (ties by position); returns how many it made. C6a: a cut with a use dealt
/// into its units is not free.
pub(super) fn fewest_flats(
    kind: &mut [Option<BuildingKind>],
    uses: &[Vec<(BuildingKind, Unit)>],
    segs: &[Seg],
    want: BuildingKind,
    n: usize,
    ok: &dyn Fn(usize) -> bool,
) -> usize {
    let mut c: Vec<usize> = (0..segs.len()).filter(|&i| kind[i].is_none() && uses[i].is_empty() && ok(i)).collect();
    c.sort_by_key(|&i| (segs[i].flats, segs[i].rect.y, segs[i].rect.x));
    let m = n.min(c.len());
    for &i in &c[..m] {
        kind[i] = Some(want);
    }
    m
}

impl Report {
    /// Kind counts per 1,000 residents beside the generator's, one line each.
    pub fn mix_lines(&self) -> Vec<String> {
        let per_k = f64::from(self.residents.max(1)) / 1000.0;
        GEN_MIX
            .iter()
            .map(|&(k, gen)| {
                let n = self.kinds.get(&k).copied().unwrap_or(0);
                format!("{:<15} {n:>6}  {:>7.2} per 1,000 (generator {gen})", format!("{k:?}"), n as f64 / per_k)
            })
            .collect()
    }

    pub fn print(&self) {
        for i in &self.inputs {
            println!("input: {i}");
        }
        println!(
            "map {} x {} tiles ({} m a tile); {} lots read ({} with a sim_layout), {} block parts",
            self.w, self.h, self.tile_m, self.lots, self.lots_with_layout, self.parts
        );
        println!(
            "buildings: {} segments kept; dropped {} too small, {} with no road at any side, {} overlapping ({} lots in dropped parts or segments)",
            self.segments, self.dropped_small, self.dropped_no_road, self.dropped_overlap, self.lots_dropped
        );
        println!(
            "flats read {}, shop units {}; Homes hold {} flats; Home floor slots {} ({} residents at {RESIDENTS_PER_SLOT} a slot; {} capped at 255)",
            self.flats,
            self.shop_units,
            self.home_flats,
            self.home_floors,
            self.home_floors * RESIDENTS_PER_SLOT as usize,
            self.slots_capped
        );
        println!(
            "doors: {} on their out tile, {} on the nearest road-facing wall tile, {} by the side search; {} moved off a built-over alley; {} name their lot (lot=); {} cuts hold no lot centre (no lot=, no venue)",
            self.doors_exact, self.doors_near, self.doors_searched, self.doors_moved, self.lots_named, self.pieces
        );
        println!(
            "terrain: {} Mountain tiles, {} sea tiles before the blocks and roads",
            self.mountain_tiles, self.sea_tiles
        );
        if self.levels > 0 {
            let [fl, sh, ws, of] = self.level_units;
            println!(
                "levels (city-sim/0.2; their units hold the tenancies, the cores carried for C7): {} levels ({} tower plate floors); units flats {fl}, shops {sh}, workshops {ws}, offices {of}; cores {} flight, {} lift",
                self.levels, self.plate_levels, self.cores_flight, self.cores_lift
            );
        }
        if self.ground_chunks > 0 {
            println!(
                "crossings (V7, out/chunks ground + out/street zebras): {} ground chunks, {} carriageway tiles; {} zebras read, {} X lines, {} dropped (no carriageway tile under them)",
                self.ground_chunks, self.carriage_tiles, self.zebras_read, self.crossings, self.zebras_dropped
            );
        }
        if !self.portals.is_empty() {
            let by: Vec<String> = self.portals.iter().map(|(k, n)| format!("{k} {n}")).collect();
            println!(
                "walkways (sim/walkways_sim.json): {} portals, {:.0} m: {}",
                self.portals.values().sum::<usize>(),
                self.portal_m,
                by.join(", ")
            );
            let by: Vec<String> = self.portals_mapped.iter().map(|(k, n)| format!("{k} {n}")).collect();
            let skip: Vec<String> = self.portals_skipped.iter().map(|(k, n)| format!("{k} {n}")).collect();
            println!(
                "walkways mapped (C7): {} portals as {} P lines (deck segments cut at {PIECE_TILES} tiles): {}; not mapped: {}",
                self.portals_mapped.values().sum::<usize>(),
                self.portal_lines,
                by.join(", "),
                if skip.is_empty() { "none".to_string() } else { skip.join(", ") }
            );
        }
        if self.lift_hosts > 0 {
            println!("lifts (C7): {} hosts carry lift= (their lots' lift cores)", self.lift_hosts);
        }
        if !self.units_by_district.is_empty() {
            let per_k = f64::from(self.residents) / 1000.0;
            let lot_mix = GEN_MIX.iter().find(|m| m.0 == BuildingKind::Lot).map_or(0.0, |m| m.1);
            let all_mix: f64 = GEN_MIX.iter().filter(|m| m.0 != BuildingKind::Home).map(|m| m.1).sum();
            println!(
                "units offered by district (C6's input) against what {} residents want: {:.0} Lots ({lot_mix} per 1,000), {:.0} employers in all ({all_mix} per 1,000)",
                self.residents,
                lot_mix * per_k,
                all_mix * per_k
            );
            println!(
                "  {:<22} {:>8} {:>6} {:>9} {:>7} {:>6} {:>7}   {:>6} {:>9}",
                "district", "flats", "shops", "workshops", "offices", "works", "sites", "Lots", "employers"
            );
            let mut tot = [0u64; 7];
            for (d, u) in &self.units_by_district {
                let sites = u[1] + u[2] + u[3] + u[4];
                let dealt = self.dealt_by_district.get(d);
                let lots = dealt.and_then(|m| m.get(&BuildingKind::Lot)).copied().unwrap_or(0) as u64;
                let emp = dealt
                    .map_or(0, |m| m.iter().filter(|(k, _)| **k != BuildingKind::Home).map(|(_, n)| *n).sum::<usize>())
                    as u64;
                for (t, v) in tot.iter_mut().zip([u[0], u[1], u[2], u[3], u[4], sites, lots]) {
                    *t += v;
                }
                println!(
                    "  {d:<22} {:>8} {:>6} {:>9} {:>7} {:>6} {sites:>7}   {lots:>6} {emp:>9}",
                    u[0], u[1], u[2], u[3], u[4]
                );
            }
            let emp_all: usize = self.kinds.iter().filter(|(k, _)| **k != BuildingKind::Home).map(|(_, n)| *n).sum();
            println!(
                "  {:<22} {:>8} {:>6} {:>9} {:>7} {:>6} {:>7}   {:>6} {emp_all:>9}",
                "all", tot[0], tot[1], tot[2], tot[3], tot[4], tot[5], tot[6]
            );
        }
        println!(
            "tenancies (C6a): {} uses on {} host cuts ({} through their own lot's street door); {} buildings in all on {} cuts",
            self.tenancies,
            self.hosts,
            self.tenancy_doors,
            self.segments + self.tenancies,
            self.segments
        );
        for (k, n) in &self.kinds {
            println!("  {:<15} {n:>6}", format!("{k:?}"));
        }
        if self.kind_use.keys().any(|(_, u)| u != "-") {
            println!("kinds by their cut's use:");
            for ((k, u), n) in &self.kind_use {
                println!("  {:<15} {u:<12} {n:>6}", format!("{k:?}"));
            }
        }
        println!("mix for {} residents:", self.residents);
        for l in self.mix_lines() {
            println!("  {l}");
        }
        for s in &self.shortfall {
            println!("  short: {s}");
        }
        println!("districts: {}", self.districts.join(", "));
        println!("El lines: {}", self.lines.join(", "));
        println!("edge piers carved: {}", self.piers);
        for u in &self.unmapped {
            println!("  unmapped: {u}");
        }
        for wn in &self.warnings {
            println!("  warning: {wn}");
        }
    }
}
