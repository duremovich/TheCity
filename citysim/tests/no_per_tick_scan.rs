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
];

#[test]
fn test_no_unmarked_citizen_scans_in_per_tick_systems() {
    let mut bad = Vec::new();
    for file in FILES {
        let path = format!("{}/src/{file}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if !(code.contains(".citizens()") || code.contains("with::<Brain>()")) {
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
