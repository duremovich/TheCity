//! `citysim-cli shadow`: follow individual residents through their days.
//!
//! ```text
//! citysim-cli shadow --seed S --days N [--start-day D] [--pick a,b,..]
//!                    [--agent INDEX]... [--count K] [--population N]
//!                    [--map FILE] --out DIR
//! citysim-cli shadow --seed S --start-day D [--population N] [--map FILE] --list
//! citysim-cli shadow --assert [--out DIR]
//! citysim-cli shadow --lives [--population 20000] [--out DIR]
//! ```
//!
//! An observation tool. The world runs normally to `--start-day`; the chosen
//! agents are then pinned (`Brain.pinned`) so they live at Full LOD, and each
//! tick their state is diffed to write a diary (`<archetype>_<index>.md` and
//! `.jsonl`). The only sim hook is `World::shadow_notes` (social moves and
//! gossip tellings), a no-op unless turned on here. Nothing here draws RNG.
//!
//! `--lives` is the Lives at work phase 0 baseline (docs/LIVES_AT_WORK.md
//! § 9 row 0, § 13): the `--assert` recipe (day 19, 7 days, seeds 42-44
//! pooled, up to `BEHAVIOUR_SAMPLE` free agents per archetype and seed, each
//! archetype in its own clone) over every pinnable archetype, printing one
//! `PROBE lives <city> <archetype>:` line per archetype and an `all` row:
//! idle hours a day; spouse meetings a week, two ways; HangOuts and the
//! empty ones; `Served`-able shifts. With `--population N` it is instead
//! one window of the generated city (seed 42, day 20, 7 days; V4's 20k
//! recipe), every archetype pinned together in one world, `LIVES_COUNT_BIG`
//! each (a 20k world is too large to clone per archetype). Never asserted.
//!
//! The lives signals, and what a Statistical agent looks like to them:
//! - *spouse named*: a social move (`ShadowNote::Move`) between the pick and
//!   its spouse, or a HangOut of the pick with the spouse at the spot
//!   (`world.hangouts`): what the V4 critics read in the diaries. A
//!   Statistical spouse makes no moves and stands at no spot: invisible.
//! - *spouse co-located*: an hourly sample where the pick and the spouse are
//!   in the same building, or both outdoors within `COLOC_TILES`, and
//!   neither is at a sleep-class step. A Statistical spouse's `Position` is
//!   where its demotion snapped it (`lod::snap_to_phase_door`: home at night,
//!   the workplace by day) and it has no step, so it counts as awake there;
//!   the share of co-located hours with a Statistical spouse is printed.
//! - *empty HangOut*: a HangOut step closed with nobody else at the tile in
//!   `world.hangouts` (bodies only; a Statistical agent is never at a spot).
//! - *Served-able shift*: a whole, free workday shift of a pick with a `Job`
//!   at a building, with either (1) a non-staff body inside the workplace
//!   (`Building.occupants` minus `World::staff_of`) at any hourly sample of
//!   the shift, or (2) an event of a `SERVED_SOURCE_KINDS` kind during the
//!   shift naming the workplace or an actor standing inside it. A
//!   Statistical visitor (`leisure::stat_visit`, `lod::stat_eat`, the
//!   Statistical shop) moves no `Position` and pushes no event: it is
//!   invisible to both, a known limit of this baseline (phase 1 makes the
//!   encounter itself visible). It does bump the venue's
//!   `Venue.visits_today`, so a third column reads that tally over the
//!   shift's hours (leisure venues only) and the share with any of the three.
//!   Phase 1's encounter is the fourth signal: `met` is the share of shifts
//!   in which the pick gained a `Served` memory, `enc/shift` the mean count
//!   (phase 0 read 0 for both; a Statistical pick keeps no such memory, but
//!   the picks are pinned Full).
//!
//! Shadow V6 (docs/plans/shadow_v6.md, phase 0): the camera lens. Each
//! diary opens with a camera block, and a window writes `camera.tsv` (one
//! row per pick and day): the minutes by place (Home, work, another
//! building, the street, a vehicle, held, Statistical, Coarse); plans
//! started and aborted (by the `PlanAborted` reason); jumps (a tile change
//! larger than the walk/ride allowance accrued in travel states since the
//! last change, plus `CAM_JUMP_SLACK`); the longest unchanged stretch (tile,
//! building, step and exec state), overall and awake; and settlements (a
//! flow note or a `Served` memory naming a person) with the counterpart not
//! co-located, Statistical, or the pick asleep. V5 bug 20's labels: a
//! `Served` line names its kind and side (read from the regulars ledger and
//! the pair's `[[serves]]` rows; the memory itself carries neither), a
//! courier's parcels are logged as handed over, the Camp's headcount is
//! never "a regular", the school fee is `SchoolFee` (not `Contract`), and
//! the Meeting is its own time-use class. Still read-only, still no RNG.
//!
//! Shadow V7 (docs/plans/shadow_v7.md, phase 1), for the pre-view pass's
//! new actions: `Break`, `Unload` and `Court` are work (they were `other`),
//! and so is any leg of a shift's shape plan; the escort van, a cab
//! driver's fare leg and a rider aboard a body cab are a vehicle, their
//! jump allowance the cab's timed speed (they were a street walk at 1 tile
//! a tick); `camera.tsv` gains the longest work-class run on one step, the
//! distinct work tiles and the GoTo steps over 120 min, per day; and
//! `--lives --population N` reads `--map`.
//!
//! Shadow V8 (the island pass's gate): a pick aboard a vehicle in motion
//! (`motion::vehicle_of`: the car, cab, flyer, exec ride, van or El train of
//! ISLAND_PASS I3) is in a vehicle, and the vehicle's own move each minute
//! is jump allowance (V7's allowances were the old timed speeds: the El at
//! `1 / ride_ticks_per_tile` read a train's 7 tiles a minute as a jump, a
//! cab rider with an Idle exec as a street walk).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use citysim::systems::classes::class_of;
use citysim::systems::reputation::rep;
use citysim::word::ShadowNote;
use citysim::{
    ActionKind, Body, Brain, Building, BuildingKind, Child, Config, Corp, Corpse, EntityId, Event, EventKind,
    ExecState, Gang, GangMember, GoalKind, Household, Identity, Inventory, Job, Kit, Lod, Memory, MemoryEntry,
    MemoryKind, Mood, Needs, Personality, Position, RelKind, Role, Sentence, Sex, Skills, Tick, Wallet, World,
    TICKS_PER_DAY, TICKS_PER_HOUR,
};

mod behaviour;
mod cam;
mod diary;
mod fold;
mod lines;
mod lives;
mod observe;
mod pick;
mod text;
mod track;

pub use behaviour::*;
use cam::*;
use diary::*;
use fold::*;
use lines::*;
pub use lives::*;
use observe::*;
use pick::*;
use text::*;
use track::*;

#[derive(clap::Args)]
pub struct ShadowArgs {
    /// World seed.
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Shadowed days (counted from `--start-day`).
    #[arg(long, default_value_t = 7)]
    pub days: u64,
    /// The day the picks are chosen and pinned; the world runs normally until then.
    #[arg(long, default_value_t = 0)]
    pub start_day: u64,
    /// Comma-separated archetypes: gang_member, gang_leader, ripperdoc, homeless, ceo, exec, guard,
    /// worker, runner, purist, reporter, child, dealer, cook, club_staff, fighter, fabber, sweeper,
    /// worker_friday, super, bouncer, dancer, night_shift, orderly, camp_warden, fixer, gun, sick,
    /// buyer, and (Jobs & lives, diaries only) joytoy, street_worker, pimp, vendor, day_labourer,
    /// cabbie, courier, office_worker, arms_dealer, forger, elder, loader, builder, foreman, madam,
    /// companion, black_doc, cultist, teacher, scav, bereaved, marked (a child is unpinnable: no
    /// Brain). `arch:N`
    /// picks N of that archetype instead of `--count` (Shadow V7).
    #[arg(long)]
    pub pick: Option<String>,
    /// Shadow this entity index (repeatable).
    #[arg(long = "agent", value_name = "INDEX")]
    pub agents: Vec<u32>,
    /// How many agents per archetype.
    #[arg(long, default_value_t = 1)]
    pub count: usize,
    /// Output directory for the diaries.
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Print the candidate count per archetype at `--start-day` and exit.
    #[arg(long)]
    pub list: bool,
    /// V2: observe without pinning, so the diary shows the Statistical (and held) stretches and
    /// what the stand-in did.
    #[arg(long)]
    pub no_pin: bool,
    /// The behaviour tier (docs/TESTING.md): run the fixed windows (days 19 and 90, 7 days,
    /// seeds 42-44 pooled, up to 30 agents per archetype and seed), print each archetype's
    /// metrics and judge them against the bounds in `CHECKS`; exit non-zero on a failure.
    /// `--out` keeps the diaries (5 per archetype and seed).
    #[arg(long)]
    pub assert: bool,
    /// Lives at work phase 0: the `--assert` windows over every pinnable archetype, printing
    /// `PROBE lives` lines (idle hours, spouse meetings, empty HangOuts, Served-able shifts);
    /// with `--population N`, one window of the generated city instead (seed 42, day 20).
    #[arg(long)]
    pub lives: bool,
    /// Shadow in a generated city of this many residents (`core_20k`'s recipe: `gen:<N>`,
    /// `Config::scaled_to(N)`, 50 Ambient flats a slot) instead of the shipped 2,000 city.
    #[arg(long, value_name = "N")]
    pub population: Option<u32>,
    /// Shadow V6: a map file overriding `[world] map`, as `run --map` (with `--population`, the
    /// scaled config on this map instead of the generated one). Not read by `--assert`; read by
    /// `--lives --population N` (Shadow V7: the island's window).
    #[arg(long, value_name = "FILE")]
    pub map: Option<PathBuf>,
    /// The city milestone C2: write the world's save at the start of each of these days
    /// (comma-separated, inside `--start-day ..= --start-day + --days`) to `<out>/saves/<seed>-<tick>.ron`,
    /// so the follow-cam (`cityview --save FILE --follow INDEX`) watches the diary's world.
    #[arg(long, value_name = "DAYS", value_delimiter = ',')]
    pub save_at: Vec<u64>,
    /// The view milestone V6 (OPEN-270): record the picks' rings a tick to `<out>/rec/<arch>_<index>_d<day>.simrec.gz`
    /// (`city-simrec/0.1`, `record.rs`) for cityview `--sim FILE` to play. Bare: every pick; else `all` or
    /// archetypes and entity indices, comma-separated.
    #[arg(long, value_name = "PICKS", num_args = 0..=1, default_missing_value = "all")]
    pub record: Option<String>,
    /// V6: with one pick recorded, the shadow sets the view on it each tick (`PlayerCommand::SetView`, the
    /// follow-cam's ring, `[view] ring_half_tiles`), so the camera ring (V5) holds its surroundings Full as the live
    /// follow-cam does. It changes the shadow's world (the ring's bodies Full) and costs their ticks.
    #[arg(long)]
    pub record_view: bool,
    /// The view milestone V6: start from this save (its world, its config) instead of building one; the window
    /// starts at the save's day (`--start-day`, when given, must be it or later: the world runs on to it).
    /// `--population` and `--map` are the save's.
    #[arg(long, value_name = "FILE")]
    pub from_save: Option<PathBuf>,
}

const ARCHETYPES: [&str; 61] = [
    "gang_member",
    // The pressure pass phase 5b: a member of a gang whose standing order
    // marches (a raid, a retaliation, a breakout) at the start day.
    "raider",
    "gang_leader",
    "ripperdoc",
    "homeless",
    "ceo",
    "exec",
    "guard",
    "worker",
    "runner",
    "purist",
    "reporter",
    "child",
    "dealer",
    "cook",
    "club_staff",
    "fighter",
    "fabber",
    "sweeper",
    "worker_friday",
    // Jobs and room P3 (J14): the second roles, by trade.
    "super",
    "bouncer",
    "dancer",
    "night_shift",
    "orderly",
    "camp_warden",
    // M16a phase 5 (5.3): a Fixer's owner (or its hired Fixer), and a gun (an agent holding a
    // contract or one fulfilled in the last 7 days). No `CHECKS` rows yet: measured on the
    // behaviour tier after the pick rework.
    "fixer",
    "gun",
    // Hunger as sickness (docs/HUNGER_SICKNESS.md § 9): malnutrition >= 0.5 or a cancer. Diaries
    // only (no `CHECKS` rows); empty with `[sickness]` off.
    "sick",
    // The Hit market phase 4: an agent holding a want (on the walk to the Fixer) or the placing
    // agent of an organic Hit or Beat live or posted in the 7 days before today. Diaries only
    // (no `CHECKS` rows: the tier is recalibrated separately).
    "buyer",
    // Jobs & lives phase 8 (docs/JOBS_AND_LIVES.md § 12): the milestone's lives. Diaries only (no
    // `CHECKS` rows until the eight-run calibration ensemble). A trade by its `[[trades]]` key
    // (posts and pitches); the rest by state.
    "joytoy",
    "street_worker",
    "pimp",
    "vendor",
    "day_labourer",
    "cabbie",
    "courier",
    "office_worker",
    "arms_dealer",
    "forger",
    "elder",
    "loader",
    "builder",
    "foreman",
    "madam",
    "companion",
    "black_doc",
    "cultist",
    "teacher",
    "scav",
    // The pressure pass phase 1 (OPEN-178): the lowest-mood free adults, lowest first (not shuffled).
    "miserable",
    // The pressure pass phase 6: owners of person-owned businesses with staff to pay, the
    // lowest till against a week's wage bill first (not shuffled).
    "owner_broke",
    // The pressure pass phase 5a: an adult serving a sentence on the start day (the cells, the
    // release into a changed life).
    "jailed",
    // Pressure pass round 2, R1: a repeat fare dodger at the start day (caught at the gate
    // twice or more inside the window, or banned from the El).
    "dodger",
    // The pressure pass R4 (OPEN-219): a parent of a child held in a standing Camp (the weekly
    // answer: a visit, a buy-out, a break-out).
    "camp_parent",
    // Round 5 U2 (OPEN-231): a parent whose child at home went unfed last night (the ladder's
    // next rung: a warning, a second warning's take, or a take with no free parent), and a
    // jobless parent of a Camp child (the week's answer against a search day).
    "neglect_parent",
    "camp_jobless",
    // The pressure pass S6 (OPEN-224): a working parent whose child is on a
    // School's roll at the start day or earlier in the childhood (the seeded city's children).
    "school_parent",
    // Round 4 T5 (OPEN-217): a jobless adult inside the search at the start day (a post lost
    // in the last `[search] search_days`), the freshest first (not shuffled).
    "jobless",
    // Round 8 X2 (OPEN-247): an adult mourning a death in the household at the start day (the
    // freshest first), and an adult with a taken Hit or Beat on him (lying low first).
    "bereaved",
    "marked",
];

/// Activity classes of the time-use table, in print order.
/// Shadow V6 (V5 bug 20): the Meeting is its own class (it read as `other`).
const CLASSES: [&str; 12] =
    ["sleep", "work", "eat", "social", "leisure", "travel", "crime", "idle", "jail", "stat", "meeting", "other"];

// ---------------------------------------------------------------------------
// Candidates
// ---------------------------------------------------------------------------

fn adult_alive(world: &World, id: EntityId) -> bool {
    world.has::<Brain>(id) && world.has::<Identity>(id) && !world.has::<Child>(id) && !world.has::<Corpse>(id)
}

/// V2: free on the start day: not jailed, not emigrating.
fn is_free(world: &World, id: EntityId) -> bool {
    !world.has::<Sentence>(id) && !world.comp::<Brain>(id).is_some_and(|b| b.emigrating)
}

fn is_purist(world: &World, id: EntityId) -> bool {
    world.gang_of(id).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.creed == Some(citysim::word::Creed::Purist))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn shadow(args: ShadowArgs) -> Result<(), String> {
    if args.assert {
        let report = behaviour(args.out.as_deref())?;
        print!("{}", report.text);
        return if report.failures.is_empty() {
            Ok(())
        } else {
            Err(format!("behaviour tier: {} failure(s): {:?}", report.failures.len(), report.failures))
        };
    }
    if args.lives {
        print!("{}", lives(args.out.as_deref(), args.population, args.map.as_deref())?);
        return Ok(());
    }
    if !args.list && args.out.is_none() {
        return Err("--out <dir> is required (or use --list or --assert)".into());
    }
    let mut args = args;
    let mut world = match &args.from_save {
        Some(p) => {
            if args.population.is_some() || args.map.is_some() {
                return Err("--from-save: --population and --map are the save's".into());
            }
            let t0 = std::time::Instant::now();
            let mut w = citysim::save::load_from_file(p)?;
            eprintln!("loaded {} at tick {} ({:.1} s)", p.display(), w.tick, t0.elapsed().as_secs_f64());
            let day = w.tick.div_ceil(TICKS_PER_DAY);
            if args.start_day != 0 && args.start_day < day {
                return Err(format!("--start-day {} is before the save's day {day}", args.start_day));
            }
            args.start_day = args.start_day.max(day);
            args.seed = w.seed();
            while w.tick < args.start_day * TICKS_PER_DAY {
                citysim::tick(&mut w);
            }
            w
        }
        None => start_world(args.seed, args.start_day, args.population, args.map.as_deref())?,
    };

    if args.list {
        println!("seed {} day {}: candidates per archetype", args.seed, args.start_day);
        for a in ARCHETYPES {
            match candidates(&world, a) {
                Ok(v) if a == "child" => {
                    println!("  {a:<13} {} (UNPINNABLE: a child has no Brain, so no Full LOD; not picked)", v.len())
                }
                Ok(v) => {
                    let free = v.iter().filter(|&&id| is_free(&world, id)).count();
                    println!("  {a:<13} {} ({free} free: not jailed, not emigrating)", v.len());
                }
                Err(e) => println!("  {a:<13} n/a ({e})"),
            }
        }
        let wd = args.start_day % 7;
        let friday = (world.config.leisure.collect_weekday + 6) % 7;
        println!(
            "start day {} is weekday {wd}; worker_friday wants weekday {friday} (the day before [leisure] collect_weekday {}): {}",
            args.start_day,
            world.config.leisure.collect_weekday,
            if wd == friday { "yes".to_string() } else { "no, pick --start-day with day % 7 == that weekday".to_string() }
        );
        return Ok(());
    }

    let picked = pick(&world, args.seed, args.start_day, args.pick.as_deref(), args.count, &args.agents);
    // Round 8 X6 (OPEN-217): the archetypes nobody does (an error once the rest are written).
    type Picks = Vec<(String, EntityId)>;
    let (empty, picked): (Picks, Picks) = picked.into_iter().partition(|(a, _)| a.starts_with(EMPTY_PICK));
    let empty: Vec<String> = empty.into_iter().map(|(a, _)| a[EMPTY_PICK.len()..].to_string()).collect();
    if picked.is_empty() {
        return Err("nothing to shadow: no --pick candidates and no live --agent".into());
    }
    let dir = args.out.clone().unwrap_or_default();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // V4: the pins sit inside `[lod] max_full`, so more picks than the cap
    // left the overflow at Coarse all week; the cap grows by the pick count,
    // keeping its slots for the rest of the city. Not in `behaviour`, whose
    // bounds were measured under the shipped cap.
    if !args.no_pin {
        world.config.lod.max_full += picked.len();
    }
    let end_day = args.start_day + args.days;
    if let Some(d) = args.save_at.iter().find(|&&d| d < args.start_day || d > end_day) {
        return Err(format!("--save-at {d} is outside the window's days {}..={end_day}", args.start_day));
    }
    let saves = (!args.save_at.is_empty()).then(|| Saves {
        ticks: args.save_at.iter().map(|d| d * TICKS_PER_DAY).collect(),
        dir: dir.join("saves"),
        seed: args.seed,
    });
    let mut recorder = match &args.record {
        Some(sel) => Some(crate::record::Recorder::new(&world, &dir, &crate::record::selected(sel, &picked)?)?),
        None if args.record_view => return Err("--record-view needs --record".into()),
        None => None,
    };
    if let Some(r) = recorder.as_mut().filter(|_| args.record_view) {
        r.set_view()?;
    }
    let mut tracks =
        follow_saving(&mut world, &picked, args.days, !args.no_pin, true, saves.as_ref(), recorder.as_mut())?;
    if let Some(r) = recorder {
        for (path, frames, raw, bytes) in r.finish()? {
            println!("recorded {}: {frames} frames, {} KB ({} KB of JSON)", path.display(), bytes / 1024, raw / 1024);
        }
    }
    let end = world.tick;
    let mut total = 0u64;
    let mut tsv = format!("{CAMERA_TSV_HEADER}\n");
    for t in &mut tracks {
        let (md, jl) = write_diary(&world, t, &dir, end)?;
        total += md + jl;
        camera_rows(t, &mut tsv);
        println!("{}_{}: {} ({} KB md, {} KB jsonl)", t.arch, t.id.index, t.name, md / 1024, jl / 1024);
    }
    let tp = dir.join("camera.tsv");
    std::fs::write(&tp, &tsv).map_err(|e| format!("{}: {e}", tp.display()))?;
    println!("wrote {} diaries to {} ({} KB)", tracks.len(), dir.display(), total / 1024);
    if !empty.is_empty() {
        return Err(format!("no candidates for: {} (see the error lines above)", empty.join(", ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_diary_has_every_section() {
        let dir = std::env::temp_dir().join(format!("citysim_shadow_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let args = ShadowArgs {
            seed: 42,
            days: 2,
            start_day: 0,
            pick: Some("worker".into()),
            agents: vec![],
            count: 1,
            out: Some(dir.clone()),
            list: false,
            no_pin: false,
            assert: false,
            lives: false,
            population: None,
            map: None,
            save_at: vec![],
            record: None,
            record_view: false,
            from_save: None,
        };
        shadow(args).expect("shadow runs");
        let md = std::fs::read_dir(&dir)
            .expect("out dir")
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|x| x == "md"))
            .expect("a markdown diary");
        let text = std::fs::read_to_string(md.path()).expect("diary reads");
        for needle in [
            "**Who**",
            "**Personality**",
            "**Skills**",
            "**Home**",
            "**Job**",
            "**Wallet**",
            "**Reputation**",
            "**Relationships**",
            "## Day 0",
            "## Day 1",
            "[goal] GOAL",
            "runners-up",
            "[plan] PLAN",
            "[action] ACTION",
            "[needs] needs hunger",
            "DAY SUMMARY d0",
            "DAY SUMMARY d1",
            "## Whole-run summary",
            "### Time use (hours)",
            "### Top places",
            "### Top contacts",
            "### Money",
            "### Notable events",
            // Shadow V6: the camera block.
            "## Camera (Shadow V6)",
            "| d0 |",
            "| d1 |",
        ] {
            assert!(text.contains(needle), "diary lacks {needle:?}");
        }
        // Shadow V6: the window's camera.tsv, a row per pick and day.
        let tsv = std::fs::read_to_string(dir.join("camera.tsv")).expect("camera.tsv");
        let rows: Vec<&str> = tsv.lines().collect();
        assert_eq!(rows.first().copied(), Some(CAMERA_TSV_HEADER));
        assert_eq!(rows.len(), 3, "a header and two days: {tsv}");
        let cols = CAMERA_TSV_HEADER.split('\t').count();
        assert!(rows.iter().all(|r| r.split('\t').count() == cols), "ragged camera.tsv: {tsv}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Shadow V6: the `PlanAborted` reasons, as `exec`, `plan`, `think` and
    /// `lod` write them.
    #[test]
    fn test_abort_reasons() {
        assert_eq!(abort_reason("Some(Work) failed at GoTo(TargetHome): Timeout"), "Timeout");
        assert_eq!(abort_reason("Some(Sleep) failed at Sleep: BuildingFull (bed: Home)"), "BuildingFull");
        assert_eq!(abort_reason("Work unplannable (no feasible action)"), "Unplannable");
        assert_eq!(abort_reason("Visit unplannable: NoPlan"), "Unplannable");
        assert_eq!(abort_reason("Work LodDemotion"), "LodDemotion");
        assert_eq!(abort_reason("Work -> Eat"), "Interrupted");
    }

    /// The behaviour tier (docs/TESTING.md): the archetypes' diary metrics
    /// against `CHECKS`. `#[ignore]` (two windows on three seeds, each
    /// archetype in its own clone: ~1-1.5 min release, ~3 GB):`cargo test --release -p citysim-cli -- --ignored behaviour`,
    /// or `citysim-cli shadow --assert`.
    #[test]
    #[ignore = "tier: merge"]
    fn test_behaviour_tier() {
        let r = behaviour(None).expect("the behaviour windows run");
        eprint!("{}", r.text);
        assert!(r.failures.is_empty(), "behaviour tier failures: {:?}", r.failures);
    }

    /// Shadow V4 bug 23: the picks name the trade's people: a cancer past
    /// stage 0, a free pimp, an open pitch's postless owner.
    #[test]
    fn test_picks_skip_stage_zero_jailed_pimp_and_closed_pitch() {
        let mut world = World::new(42, Config::load());
        let free: Vec<EntityId> = world
            .citizens()
            .into_iter()
            .filter(|&a| adult_alive(&world, a) && !world.has::<Job>(a) && !world.has::<citysim::Sentence>(a))
            .collect();
        let (s0, s1, pimp, vend) = (free[0], free[1], free[2], free[3]);
        for (a, stage) in [(s0, 0.0), (s1, 0.3)] {
            if !world.has::<citysim::Health>(a) {
                world.insert(a, citysim::Health::default());
            }
            let h = world.comp_mut::<citysim::Health>(a).expect("health");
            h.malnutrition = 0.0;
            h.cancer = Some(citysim::components::Cancer { stage, since: 0, diagnosed: false });
        }
        let sick = candidates(&world, "sick").expect("sick");
        assert!(!sick.contains(&s0) && sick.contains(&s1), "a stage-0 cancer is no sick pick");
        world.jobs2.vice.pimps.insert(0, pimp);
        let until = world.tick + TICKS_PER_DAY;
        world.insert(pimp, citysim::Sentence { until_tick: until, crime: citysim::Crime::Assault });
        assert!(!candidates(&world, "pimp").expect("pimp").contains(&pimp), "a jailed pimp");
        let Role::Trade(t) = world.config.trade_role("vendor").expect("the vendor row") else { panic!("a trade") };
        let spot = world.comp::<citysim::Position>(vend).expect("pos").tile;
        let p = citysim::components::Pitch {
            spot,
            district: world.district_of(spot),
            trade: t,
            owner: vend,
            since: 0,
            take_today: 0,
            take_week: 0,
            cut_to: None,
            stock: 0,
            closed_until: Some(until),
            dry_days: 0,
            jobs_today: 0,
            cut_paid: 0,
        };
        world.jobs2.pitches.insert(vend, p);
        assert!(!candidates(&world, "vendor").expect("vendor").contains(&vend), "a pitch closed tonight");
        world.jobs2.pitches.get_mut(&vend).expect("pitch").closed_until = None;
        assert!(candidates(&world, "vendor").expect("vendor").contains(&vend), "open, it is picked");
    }

    #[test]
    fn test_unknown_archetype_is_an_error_not_a_panic() {
        let world = World::new(42, Config::load());
        assert!(candidates(&world, "astronaut").is_err());
        assert!(candidates(&world, "reporter").is_ok());
    }
}
