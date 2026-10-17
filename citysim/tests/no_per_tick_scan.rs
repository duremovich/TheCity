//! M10: no per-tick system may scan every citizen. Per-tick code reads the
//! tier and role indices on the world (`bodies`, `guards`, `workers`); a call
//! to `citizens()` or `with::<Brain>()` in the per-tick files must carry a
//! `scan-ok:` marker naming why it is not per tick (daily, hourly, an event,
//! or a deliberate exception) on the same line or the line above.

const FILES: &[&str] = &[
    "needs.rs",
    "systems/think.rs",
    "exec/mod.rs",
    "exec/actions.rs",
    "goap/actions.rs",
    "goap/world_state.rs",
    "utility/goals.rs",
    "systems/social.rs",
    "systems/law.rs",
    "systems/lod.rs",
    "systems/plan.rs",
    "systems/gang.rs",
    "systems/raid.rs",
    "systems/assets.rs",
    // M13 phase 2.
    "systems/vehicles.rs",
    "systems/security.rs",
    // M13 phase 3.
    "systems/chrome.rs",
    // M13 phase 4.
    "systems/stims.rs",
    "systems/robots.rs",
    // M14 phase 1.
    "virt.rs",
    "systems/virt.rs",
    "systems/tech.rs",
    // M15 phase 1.
    "word.rs",
    "systems/word.rs",
    "systems/gossip.rs",
    "systems/reputation.rs",
    // M15 phase 2.
    "systems/moves.rs",
    "systems/competence.rs",
    "systems/creeds.rs",
    // M15 phase 3.
    "systems/grudges.rs",
    "systems/hunt.rs",
    // M15 phase 4.
    "systems/news.rs",
    // L2 phase 1.
    "systems/living.rs",
    "systems/jobs.rs",
    "systems/budget.rs",
    "systems/outside.rs",
    // L2 phase 3.
    "systems/fviolence.rs",
    // L2 phase 2.
    "systems/leisure.rs",
    // L2 phase 4 (the daily pass runs from the binder's midnight).
    "systems/bind.rs",
    // M16a phase 1 (plan C43).
    "contract.rs",
    "systems/contracts.rs",
    "systems/missions.rs",
    // The Real economy phase 1 (daily passes from `living::run`).
    "systems/econ.rs",
    "systems/world_market.rs",
    // The Real economy phase 3b/3c.
    "systems/charity.rs",
    "systems/camp.rs",
    // The Real economy phase 2 (the wage and staffing passes from `living::run`).
    "systems/wages.rs",
    // M16b phase 4.
    "systems/governance.rs",
    "systems/tags.rs",
    "systems/attention.rs",
    // Jobs & lives phase-8 review: the daily passes that scanned the city
    // for a role or an asset kind read the indices (`workers`, the asset
    // store) instead; `.entities()` is checked here too (`ENTITY_FILES`).
    "systems/gray.rs",
    "systems/faith.rs",
    "systems/braindance.rs",
];

/// Files where a scan of every entity (`.entities()`) also needs a marker.
const ENTITY_FILES: &[&str] = &["systems/gray.rs", "systems/faith.rs", "systems/braindance.rs"];

#[test]
fn test_no_unmarked_citizen_scans_in_per_tick_systems() {
    let mut bad = Vec::new();
    for file in FILES {
        let path = format!("{}/src/{file}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let entity_scan = ENTITY_FILES.contains(file) && code.contains(".entities()");
            if !(code.contains(".citizens()") || code.contains("with::<Brain>()") || entity_scan) {
                continue;
            }
            let marked = line.contains("scan-ok:") || (i > 0 && lines[i - 1].contains("scan-ok:"));
            if !marked {
                bad.push(format!("{file}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "unmarked per-tick scans (use world.bodies()/guards()/workers() or mark scan-ok):\n{}",
        bad.join("\n")
    );
}

/// Phase-8 review: every hashed roll's purpose constant (`const PURPOSE_*:
/// u64`) across `src/` is unique, so two systems never read the same draw
/// (on d2eea01 `fashion::PURPOSE_MUG` and `faith::PURPOSE_ATTEND` were both
/// 0x4641_0001).
#[test]
fn test_hash_purposes_are_unique() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).expect("src") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    files.sort();
    let mut seen: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut dup = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).expect("read");
        for line in text.lines() {
            let l = line.trim();
            let Some(rest) = l.strip_prefix("const PURPOSE_").or_else(|| l.strip_prefix("pub const PURPOSE_")) else {
                continue;
            };
            let Some((name, value)) = rest.split_once(": u64 = ") else { continue };
            let value = value.trim_end_matches(';').replace('_', "").to_lowercase();
            let here =
                format!("{}::PURPOSE_{name}", f.file_name().map_or(String::new(), |n| n.to_string_lossy().into()));
            if let Some(prev) = seen.insert(value.clone(), here.clone()) {
                dup.push(format!("{value}: {prev} and {here}"));
            }
        }
    }
    assert!(dup.is_empty(), "shared hash purposes: {dup:?}");
}
