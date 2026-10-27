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
    /// companion, black_doc, cultist, teacher, scav (a child is unpinnable: no Brain). `arch:N`
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
}

const ARCHETYPES: [&str; 56] = [
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
    // The pressure pass S6 (OPEN-224): a working parent whose child is on a
    // School's roll at the start day or earlier in the childhood (the seeded city's children).
    "school_parent",
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

/// Every candidate of an archetype, ascending by index (the CEO list is by
/// treasury, richest first). `Err` for an unknown archetype or a role the
/// build does not have.
fn candidates(world: &World, arch: &str) -> Result<Vec<EntityId>, String> {
    let all = world.citizens();
    let adults = || all.iter().copied().filter(|&a| adult_alive(world, a));
    let by_role = |roles: &[Role]| -> Vec<EntityId> {
        adults().filter(|&a| world.comp::<Job>(a).is_some_and(|j| roles.contains(&j.role))).collect()
    };
    let mut out: Vec<EntityId> = match arch {
        "gang_member" => adults()
            .filter(|&a| {
                world.has::<GangMember>(a)
                    && world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.leader != Some(a))
            })
            .collect(),
        "gang_leader" => adults()
            .filter(|&a| world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.leader == Some(a)))
            .collect(),
        "raider" => adults()
            .filter(|&a| {
                world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| {
                    g.leader != Some(a)
                        && (g.order.is_raid() || g.pending_order.as_ref().is_some_and(|p| p.order.is_raid()))
                })
            })
            .collect(),
        "ripperdoc" => {
            let owners: BTreeSet<EntityId> = world
                .buildings_of_kind(BuildingKind::Clinic)
                .iter()
                .filter_map(|&b| world.comp::<Building>(b).and_then(|b| b.owner))
                .collect();
            adults()
                .filter(|a| owners.contains(a) || world.comp::<Job>(*a).is_some_and(|j| j.role == Role::Ripperdoc))
                .collect()
        }
        "homeless" => adults().filter(|&a| world.comp::<Household>(a).is_some_and(|h| h.home.is_none())).collect(),
        // Shadow V4 bug 23: a cancer past stage 0 (it took one at 0.00).
        "sick" => adults()
            .filter(|&a| {
                world
                    .comp::<citysim::Health>(a)
                    .is_some_and(|h| h.malnutrition >= 0.5 || h.cancer.as_ref().is_some_and(|c| c.stage > 0.0))
            })
            .collect(),
        "ceo" => {
            let mut corps: Vec<(i64, EntityId, EntityId)> = world
                .corps()
                .into_iter()
                .filter_map(|c| world.comp::<Corp>(c).and_then(|k| k.exec.map(|e| (k.treasury, c, e))))
                .filter(|&(_, _, e)| adult_alive(world, e))
                .collect();
            corps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            return Ok(corps.into_iter().map(|(_, _, e)| e).collect());
        }
        "exec" => world
            .corps()
            .into_iter()
            .filter_map(|c| world.comp::<Corp>(c).and_then(|k| k.exec))
            .filter(|&e| adult_alive(world, e))
            .collect(),
        "guard" => world.guards().iter().copied().filter(|&a| adult_alive(world, a)).collect(),
        "worker" => adults().filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role != Role::Guard)).collect(),
        // V2: a deck is not a run. A `JackedIn` event naming the agent in the 7 days before today
        // (or a run seated now) qualifies; an agent with a deck and no runs does not.
        "runner" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut ran: BTreeSet<EntityId> = world
                .events
                .iter()
                .rev()
                .take_while(|e| e.tick >= from)
                .filter(|e| e.kind == EventKind::JackedIn)
                .filter_map(|e| e.actors.first().copied())
                .collect();
            ran.extend(world.runner_of.keys().copied());
            adults().filter(|a| ran.contains(a)).collect()
        }
        "cook" => by_role(&[Role::Cook]),
        "club_staff" => by_role(&[Role::Host, Role::Attendant, Role::Croupier, Role::Concierge]),
        "fighter" => by_role(&[Role::Fighter]),
        "fabber" => by_role(&[Role::Fabber]),
        "sweeper" => by_role(&[Role::Sanitation]),
        // Jobs and room P3: a trade by its `[[trades]]` key (the night shift: the Night
        // Stockers and Night Porters); none when the config lacks the row.
        "night_shift" => {
            let roles: Vec<Role> =
                ["night_stocker", "night_porter"].iter().filter_map(|k| world.config.trade_role(k)).collect();
            by_role(&roles)
        }
        "super" | "bouncer" | "dancer" | "orderly" | "camp_warden" => {
            let roles: Vec<Role> = world.config.trade_role(arch).into_iter().collect();
            by_role(&roles)
        }
        // Jobs & lives phase 8: a trade's post holders, and (for a pitch trade) the agents
        // working its pitch; the joytoy is the chip trade's.
        "street_worker" | "vendor" | "day_labourer" | "courier" | "office_worker" | "forger" | "loader" | "builder"
        | "madam" | "companion" | "black_doc" | "teacher" | "cabbie" | "joytoy" => {
            let key = match arch {
                "cabbie" => "cab_driver",
                "joytoy" => "chip_host",
                k => k,
            };
            let Some(role) = world.config.trade_role(key) else { return Ok(Vec::new()) };
            let mut v = by_role(&[role]);
            if let Role::Trade(t) = role {
                // Shadow V4 bug 23: a pitch open (not closed that midnight) and
                // its owner free and postless (a closed pitch was picked).
                let now = world.tick;
                v.extend(
                    world
                        .jobs2
                        .pitches
                        .iter()
                        .filter(|(_, p)| p.trade == t && p.closed_until.is_none_or(|c| c <= now))
                        .map(|(&a, _)| a)
                        .filter(|&a| !world.has::<Job>(a) && !world.has::<citysim::Sentence>(a)),
                );
            }
            // Shadow V4 bug 23: at 00:00 no day post is held; the day
            // labourers are those hired off the corner in the last 7 days.
            if arch == "day_labourer" {
                let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
                let label = world.config.trade(match role {
                    Role::Trade(t) => t,
                    _ => return Ok(Vec::new()),
                });
                let label = label.map(|r| format!("hired as {}", r.label)).unwrap_or_default();
                v.extend(
                    world
                        .events
                        .iter()
                        .rev()
                        .take_while(|e| e.tick >= from)
                        .filter(|e| e.kind == EventKind::Hire && !label.is_empty() && e.text.contains(&label))
                        .filter_map(|e| e.actors.first().copied()),
                );
                v.sort_unstable();
                v.dedup();
            }
            v.into_iter().filter(|&a| adult_alive(world, a)).collect()
        }
        // Shadow V4 bug 23: a free pimp (a jailed one spent the week inside).
        "pimp" => world
            .jobs2
            .vice
            .pimps
            .values()
            .copied()
            .filter(|&a| adult_alive(world, a) && !world.has::<citysim::Sentence>(a))
            .collect(),
        "foreman" => {
            let Some(role) = world.config.trade_role("builder") else { return Ok(Vec::new()) };
            let yards: BTreeSet<EntityId> =
                by_role(&[role]).into_iter().filter_map(|a| world.comp::<Job>(a).and_then(|j| j.employer)).collect();
            yards.into_iter().filter_map(|y| citysim::systems::industry::foreman(world, y, None)).collect()
        }
        // A gang dealer whose gang keeps weapons in its Hideout (the dealer over goods).
        "arms_dealer" => {
            let mut dealers: BTreeSet<EntityId> = world.dealers.values().flatten().copied().collect();
            dealers.extend(world.deal_log.values().map(|&(a, _)| a));
            adults()
                .filter(|a| dealers.contains(a))
                // Shadow V4 bug 23: a surplus to sell (the armoury's target
                // kept back), not any stock.
                .filter(|&a| world.gang_of(a).is_some_and(|g| citysim::systems::weapons::has_surplus(world, g)))
                .collect()
        }
        "elder" => {
            let from = world.config.ages.frail_from_years;
            adults().filter(|&a| world.comp::<Identity>(a).is_some_and(|i| i.age_years() >= from)).collect()
        }
        // A member of a gang under a `[faith]` cult row.
        "cultist" => adults()
            .filter(|&a| {
                world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).and_then(|g| g.creed).is_some_and(|c| match c {
                    citysim::word::Creed::Config(i) => {
                        world.config.faith.creeds.get(usize::from(i)).is_some_and(|r| r.cult)
                    }
                    citysim::word::Creed::Purist => false,
                })
            })
            .collect(),
        // A member of a gang typed with organ targets (the scavs).
        "scav" => adults()
            .filter(|&a| world.has::<GangMember>(a))
            .filter(|&a| world.gang_of(a).is_some_and(|g| citysim::systems::gray::organ_targets(world, g)))
            .collect(),
        // V2: the diary should start on the weekday before `[leisure] collect_weekday` (Friday on
        // the shipped config: weekday 5 of `day % 7`, the last workday before the rest day 6).
        "worker_friday" => adults()
            .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role != Role::Guard))
            .filter(|&a| class_of(world, a) == citysim::Class::Street)
            .collect(),
        "purist" => adults().filter(|&a| world.has::<GangMember>(a) && is_purist(world, a)).collect(),
        "reporter" => adults().filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == Role::Reporter)).collect(),
        "child" => {
            let kids: Vec<EntityId> = all
                .iter()
                .copied()
                .filter(|&a| world.has::<Child>(a) && world.has::<Identity>(a) && !world.has::<Corpse>(a))
                .collect();
            // Infants do nothing readable: prefer a child old enough to have a life.
            let grown: Vec<EntityId> = kids
                .iter()
                .copied()
                .filter(|&a| world.comp::<Identity>(a).is_some_and(|i| i.age_years() >= 5))
                .collect();
            if grown.is_empty() {
                kids
            } else {
                grown
            }
        }
        // V2: one candidate per dealer, not per Bar: every dealer in a `deal_log` row (a Bar's
        // last dealer, within 7 days, Statistical hits included) and every dealer registered at a
        // Bar now. No gang-member fallback: a "dealer" who never dealt is no dealer.
        "dealer" => {
            let today = world.day();
            let mut dealers: BTreeSet<EntityId> =
                world.deal_log.values().filter(|&&(_, d)| d + 7 >= today).map(|&(a, _)| a).collect();
            dealers.extend(world.dealers.values().flatten().copied());
            adults().filter(|a| dealers.contains(a) && world.has::<GangMember>(*a)).collect()
        }
        // M16a 5.3: whoever runs a Fixer's office: its owner (an agent) or a hired Fixer.
        "fixer" => {
            let owners: BTreeSet<EntityId> = world
                .buildings_of_kind(BuildingKind::Fixer)
                .iter()
                .filter_map(|&b| world.comp::<Building>(b).and_then(|b| b.owner))
                .collect();
            adults()
                .filter(|a| owners.contains(a) || world.comp::<Job>(*a).is_some_and(|j| j.role == Role::Fixer))
                .collect()
        }
        // M16a 5.3: an agent holding a taken contract (its taker or a crew member) or one that
        // fulfilled a contract in the 7 days before today (closed records stay 30 days).
        "gun" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut guns: BTreeSet<EntityId> = BTreeSet::new();
            for c in world.contracts.values() {
                let held = c.status == citysim::contract::ContractStatus::Taken;
                let done =
                    c.status == citysim::contract::ContractStatus::Fulfilled && c.closed.is_some_and(|t| t >= from);
                if held || done {
                    guns.extend(c.taker);
                    guns.extend(c.crew.iter().copied());
                }
            }
            adults().filter(|a| guns.contains(a)).collect()
        }
        // The Hit market phase 4: a want's holder, or the placing agent of an organic record
        // (a motive set) still live or posted in the 7 days before today.
        "buyer" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut buyers: BTreeSet<EntityId> = world.wants.keys().copied().collect();
            for c in world.contracts.values().filter(|c| !c.motive.is_none()) {
                if c.status.is_live() || c.posted >= from {
                    buyers.extend(c.agent);
                }
            }
            adults().filter(|a| buyers.contains(a)).collect()
        }
        // The pressure pass phase 1 (OPEN-178): the adults below `[misery] mood_line` (any adult
        // below 0 without the section); `pick` orders them lowest mood first.
        "miserable" => {
            let line = world.config.misery.as_ref().map_or(0.0, |c| c.mood_line);
            adults().filter(|&a| world.comp::<citysim::components::Mood>(a).is_some_and(|m| m.value < line)).collect()
        }
        "owner_broke" => citysim::systems::underwater::owners_by_till(world).into_iter().map(|(_, o)| o).collect(),
        "jailed" => adults().filter(|&a| world.has::<Sentence>(a)).collect(),
        "dodger" => adults()
            .filter(|a| world.fares.book.get(a).is_some_and(|r| r.caught.len() >= 2 || r.banned_until.is_some()))
            .collect(),
        "school_parent" => {
            // The pressure pass S6 (OPEN-224): a parent with a job whose child
            // is on a School's roll today or has been this childhood (a start
            // day after the rest day still holds the rest day's Play).
            let mut v = Vec::new();
            for &k in all.iter() {
                if !world.has::<Child>(k) || world.has::<Corpse>(k) {
                    continue;
                }
                let at_school = matches!(
                    citysim::systems::ages::place_of(world, k),
                    Some(citysim::systems::ages::Place::School(_))
                );
                if at_school || world.comp::<Child>(k).is_some_and(|c| c.edu_days > 0) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            v.retain(|&a| world.has::<Brain>(a) && world.has::<Job>(a) && !world.has::<Sentence>(a));
            v
        }
        "camp_parent" => {
            let mut v = Vec::new();
            for camp in citysim::systems::camp::standing_camps(world) {
                for &k in citysim::systems::camp::children_of(world, camp) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            v.retain(|&a| world.has::<Brain>(a) && !world.has::<Sentence>(a) && !world.has::<Corpse>(a));
            v
        }
        other => return Err(format!("unknown archetype {other:?} (known: {})", ARCHETYPES.join(", "))),
    };
    out.sort();
    out.dedup();
    Ok(out)
}

/// Fisher-Yates keyed on the seed and the archetype name: the library's
/// `rng::splitmix64` over an FNV key and the swap index (no second copy of
/// the hash constants here).
fn shuffled(mut v: Vec<EntityId>, seed: u64, arch: &str) -> Vec<EntityId> {
    let key =
        seed ^ arch.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    for i in (1..v.len()).rev() {
        let j = (citysim::rng::splitmix64(key ^ ((i as u64) << 32)) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

// ---------------------------------------------------------------------------
// Names and places
// ---------------------------------------------------------------------------

fn clock(tick: Tick) -> String {
    let t = tick % TICKS_PER_DAY;
    format!("d{} {:02}:{:02}", tick / TICKS_PER_DAY, t / TICKS_PER_HOUR, t % TICKS_PER_HOUR)
}

fn hm(tick: Tick) -> String {
    let t = tick % TICKS_PER_DAY;
    format!("{:02}:{:02}", t / TICKS_PER_HOUR, t % TICKS_PER_HOUR)
}

/// A building as `Label#index`.
fn bname(world: &World, b: EntityId) -> String {
    match world.comp::<Building>(b) {
        Some(bl) => format!("{}#{}", bl.label.as_deref().unwrap_or_else(|| bl.kind.label()), b.index),
        None => world.name_of(b),
    }
}

/// An agent as `Name#index`, a building, a gang or a corp by name.
fn nm(world: &World, id: EntityId) -> String {
    if id == EntityId::NONE {
        return "?".into();
    }
    if let Some(i) = world.comp::<Identity>(id) {
        return format!("{}#{}", i.name, id.index);
    }
    if let Some(g) = world.comp::<Gang>(id) {
        return format!("gang {}", g.name);
    }
    if let Some(c) = world.comp::<Corp>(id) {
        return format!("corp {}", c.name);
    }
    if world.has::<Building>(id) {
        return place_of_building(world, id);
    }
    world.name_of(id)
}

fn place_of_building(world: &World, b: EntityId) -> String {
    let d = world.district_of_building(b);
    format!("{} ({})", bname(world, b), world.district_name(d))
}

fn place(world: &World, id: EntityId) -> String {
    match world.comp::<Position>(id) {
        Some(p) => match p.building {
            Some(b) => place_of_building(world, b),
            None => format!("street ({})", world.district_name(world.district_of(p.tile))),
        },
        None => "nowhere".into(),
    }
}

fn owner_name(world: &World, b: EntityId) -> Option<String> {
    let o = world.comp::<Building>(b)?.owner?;
    Some(nm(world, o))
}

fn money(c: i64) -> String {
    format!("{c}c")
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

// ---------------------------------------------------------------------------
// Diary state
// ---------------------------------------------------------------------------

struct Entry {
    tick: Tick,
    seq: u32,
    cat: &'static str,
    text: String,
}

struct OpenAction {
    key: (Tick, u8),
    action: ActionKind,
    target: Option<EntityId>,
    start: Tick,
    start_place: String,
    /// The pressure pass F1: the body's tile at the step's start (a GoTo's
    /// line gives the Manhattan tiles it moved).
    start_tile: Option<citysim::components::TilePos>,
    /// The perf pass phase 1 (OPEN-195): the step's plan goal (an idle Rest
    /// is one outside a Work plan).
    goal: Option<GoalKind>,
}

struct Track {
    id: EntityId,
    arch: String,
    name: String,
    header: String,
    entries: Vec<Entry>,
    seq: u32,
    died: Option<Tick>,
    // diff state
    last_goal: Option<GoalKind>,
    plan_started: Option<Tick>,
    open: Option<OpenAction>,
    last_place: String,
    last_building: Option<EntityId>,
    cooldowns: BTreeMap<GoalKind, Tick>,
    wallet: Option<i64>,
    inv: Option<(u32, u32, u16, u16)>,
    /// Hunger as sickness: the day's meal tally last seen (a meal logged on a rise).
    tally: Option<citysim::MealTally>,
    mem_seen: BTreeSet<(Tick, MemoryKind, Option<EntityId>, bool)>,
    mem_primed: bool,
    edges_primed: bool,
    edges: BTreeMap<EntityId, (f32, RelKind)>,
    sentence: Option<Tick>,
    gang_order: Option<String>,
    following: Option<String>,
    grudges: BTreeSet<(EntityId, Tick)>,
    hunt: Option<EntityId>,
    rep: [f32; 4],
    class: String,
    // tallies
    cur_day: u64,
    day_use: BTreeMap<&'static str, u32>,
    day_places: BTreeMap<String, u32>,
    day_net: i64,
    total_use: BTreeMap<&'static str, u32>,
    use_by_day: BTreeMap<u64, BTreeMap<&'static str, u32>>,
    total_places: BTreeMap<String, u32>,
    total_net: i64,
    flows: BTreeMap<String, (i64, u32)>,
    contacts: BTreeMap<EntityId, (String, u32)>,
    event_kinds: BTreeMap<String, u32>,
    notable: Vec<(Tick, String)>,
    pinned_lod_noted: bool,
    last_action: Option<ActionKind>,
    // V2: the L2 life
    pinned: bool,
    last_lod: Option<Lod>,
    last_pick: Option<String>,
    holes_seen: BTreeSet<u64>,
    hang_others: BTreeSet<EntityId>,
    hangouts: u32,
    hang_known: BTreeSet<EntityId>,
    /// Flow name -> (coins out, coins in, count), from the sim's flow notes.
    flow_kinds: BTreeMap<String, (i64, i64, u32)>,
    tribute_in: i64,
    gossip_about: u32,
    stat_notes: Vec<(Tick, String)>,
    stat_minutes: u32,
    held_settles: u32,
    held_minutes: u64,
    // The behaviour tier (`--assert`, docs/TESTING.md): the diary's numbers as metrics.
    alive_min: u32,
    work_min: u32,
    sleep_run: u32,
    /// `(end tick, minutes)` of every finished block of sleep-class minutes.
    sleep_blocks: Vec<(Tick, u32)>,
    energy0_min: u32,
    /// Hunger as sickness: free minutes starving (hunger < `[life]
    /// starving_hunger`), and meals by kind (synth includes a Mission's).
    starve_min: u32,
    meals: u32,
    meals_synth: u32,
    meals_dive: u32,
    /// The job's shift under way, and the shifts seen whole.
    shift: Option<ShiftObs>,
    shifts: Vec<ShiftObs>,
    /// Ticks of every wage paid to the agent (`Flow::Wage`, `Flow::ExecWage`).
    wages: Vec<Tick>,
    /// Coins handed back to the agent by `ownership::refund` (a purchase undone).
    refunds: u32,
    /// The Work goal under way: a work-class minute reached in it.
    work_plan: Option<bool>,
    /// Work goals dropped before a single work-class minute (a commute turned back).
    commute_aborts: u32,
    /// Bouts naming the agent, and those with the agent inside the pit.
    bouts: u32,
    bouts_present: u32,
    // Lives at work phase 0 (`--lives`; the module header names the signals).
    /// The spouse at the window's start (`World::spouse_of`); a marriage or a
    /// death inside the window does not move it.
    spouse: Option<EntityId>,
    /// Lives at work phase 7 (§ 5.3): the Home's tier at the last minute
    /// alive (`None` homeless).
    home_tier: Option<u8>,
    /// Hours (tick / 60) in which a social move or a HangOut named the spouse.
    spouse_named_hours: BTreeSet<Tick>,
    /// Hourly samples with the spouse alive; of those co-located and both awake;
    /// of those with the spouse Statistical (its position a demotion snapshot).
    spouse_samples: u32,
    spouse_coloc: u32,
    spouse_coloc_stat: u32,
    /// HangOuts closed with nobody else at the tile.
    hang_empty: u32,
    /// The perf pass phase 1 (OPEN-195): idle Rests cut within `SHORT_REST_MIN`.
    short_rests: u32,
    /// `SERVED_SOURCE_KINDS` events seen at the workplace during a shift, by kind.
    source_kinds: BTreeMap<String, u32>,
    /// Shadow V6: the camera lens.
    cam: Cam,
}

/// One shift of the agent's job, as observed.
#[derive(Clone, Copy)]
struct ShiftObs {
    start: Tick,
    end: Tick,
    len: u32,
    work: u32,
    /// A working day of the job (`routine::workday_of`), not a rest day.
    workday: bool,
    /// Opened at the shift's first minute (not mid-shift at the window's start).
    whole: bool,
    /// Never jailed during the shift.
    free: bool,
    // Lives at work phase 0: who was on the other side of the counter.
    /// The job's building at the shift's first minute.
    employer: Option<EntityId>,
    /// The agent owns the employer building: an owner-operator's post, which pays no wage by design
    /// (an owner pays the staff, not themselves), so the wage metrics skip it.
    own_post: bool,
    /// Hourly samples during the shift, and the non-staff bodies inside the
    /// workplace summed over them.
    samples: u32,
    nonstaff: u32,
    /// `SERVED_SOURCE_KINDS` events at the workplace during the shift.
    source_events: u32,
    /// The workplace's `Venue.visits_today` rise over the shift's hours
    /// (leisure venues only; a Statistical visit shows here and nowhere else).
    venue_visits: u32,
    last_visits: Option<u16>,
    /// Lives at work phase 1: `Served` memories the agent gained during the
    /// shift (the encounter itself, visible to neither body nor event).
    encounters: u32,
    /// Lives at work phase 5: an exec's office hours (`[life] exec_shift`
    /// at his corp's `life::hq_of`), read by `--lives` as a shift (an exec
    /// holds no Job); left out of the behaviour tier's shift metrics.
    office: bool,
}

/// Lives at work phase 0: two agents outdoors this close (Manhattan tiles)
/// at an hourly sample are co-located, as two inside one building are.
const COLOC_TILES: u32 = 2;

/// Shadow V6: the camera's places, in print order (`cam_place`).
const CAM_PLACES: [&str; 8] = ["home", "work", "building", "street", "vehicle", "held", "stat", "coarse"];
const CAM_HOME: usize = 0;
const CAM_WORK: usize = 1;
const CAM_BUILDING: usize = 2;
const CAM_STREET: usize = 3;
const CAM_VEHICLE: usize = 4;
const CAM_HELD: usize = 5;
const CAM_STAT: usize = 6;
const CAM_COARSE: usize = 7;
/// Shadow V6: tiles a position may change by beyond the travel allowance
/// before it is a jump (stepping out of a door, entering a building rect).
const CAM_JUMP_SLACK: f32 = 2.0;

/// Shadow V6: one day of the camera block.
#[derive(Default, Clone)]
struct CamDay {
    place_min: [u32; 8],
    plans: u32,
    aborts: BTreeMap<String, u32>,
    jumps: u32,
    /// Jumps with the pick Full and free at both ends (a body teleporting).
    jumps_body: u32,
    max_jump: u32,
    first_jump: Option<Tick>,
    /// The longest unchanged stretch begun this day: minutes, start, class, place.
    still: Option<(u32, Tick, &'static str, String)>,
    /// The same, among stretches that are not sleep.
    still_awake: Option<(u32, Tick, &'static str, String)>,
    /// Flow notes and `Served` memories naming a person.
    settles: u32,
    /// Of those, with the counterpart a body elsewhere.
    settles_apart: u32,
    /// With the counterpart Statistical (off screen).
    settles_stat: u32,
    /// With the pick asleep.
    settles_asleep: u32,
    /// Shadow V7 (tool): the longest run of work-class minutes on one plan
    /// step begun this day (the shift's longest single work action).
    work_run_max: u32,
    /// The distinct tiles the pick stood on at work-class minutes this day.
    work_tiles: BTreeSet<citysim::TilePos>,
    /// GoTo steps begun this day that ran over `CAM_LONG_GOTO_MIN` minutes.
    goto_long: u32,
}

/// Shadow V7: a GoTo step longer than this (minutes) is a long leg (PREVIEW_PASS § 3 phase 5's row).
const CAM_LONG_GOTO_MIN: u64 = 120;

/// The perf pass phase 1 (OPEN-195): an idle Rest this short or shorter
/// (minutes) is a cut one (`--lives`' `short_rest/d`; the loop cut at 1).
const SHORT_REST_MIN: u64 = 5;

/// The key of an unchanged stretch: tile, building, step, exec state.
type StillKey = (citysim::TilePos, Option<EntityId>, Option<ActionKind>, u8);

/// Shadow V6: the camera's running state.
#[derive(Default)]
struct Cam {
    days: BTreeMap<u64, CamDay>,
    /// The position at the last tile change, its tick, and its place text.
    anchor: Option<(citysim::TilePos, Tick, String)>,
    /// Tiles of travel allowance accrued since the anchor.
    allow: f32,
    /// The allowance a tick in the current exec state accrues.
    speed: f32,
    prev_lod: Option<Lod>,
    prev_held: bool,
    /// The unchanged stretch under way: its key, start, class and place.
    still_key: Option<StillKey>,
    still_start: Tick,
    still_act: &'static str,
    still_place: String,
    /// A courier's yard's pending parcels at the last tick (V5 bug 20).
    parcels: Vec<citysim::systems::streetwork::Parcel>,
    parcels_handed: u32,
    /// Shadow V7: the work-class run under way: its plan step key and start.
    work_run: Option<((Tick, u8), Tick)>,
    /// Shadow V8: the vehicle in motion the pick rode at the last minute
    /// (the island pass's I3, `motion::vehicle_of`) and its tile then.
    veh: Option<(EntityId, citysim::TilePos)>,
}

/// Lives at work phase 0: the event kinds that are a service encounter's
/// source today (docs/LIVES_AT_WORK.md § 1.2's list, as far as the city
/// writes an event for it: a sale, an install, a treatment, a stabilise, a
/// cure, a licence, a table or a bout, a Mission's day, a gift). No event
/// exists for a visit's completion, a tip, a cab fare or a detox.
const SERVED_SOURCE_KINDS: &[EventKind] = &[
    EventKind::AssetBought,
    EventKind::Installed,
    EventKind::Treated,
    EventKind::Rescued,
    EventKind::Cured,
    EventKind::LicenceSold,
    EventKind::Gambled,
    EventKind::Bout,
    EventKind::MissionServed,
    EventKind::Donated,
    EventKind::TraumaSave,
];

impl Track {
    fn new(id: EntityId, arch: &str, name: String, start: Tick) -> Track {
        Track {
            id,
            arch: arch.into(),
            name,
            header: String::new(),
            entries: Vec::new(),
            seq: 0,
            died: None,
            last_goal: None,
            plan_started: None,
            open: None,
            last_place: String::new(),
            last_building: None,
            cooldowns: BTreeMap::new(),
            wallet: None,
            tally: None,
            inv: None,
            mem_seen: BTreeSet::new(),
            mem_primed: false,
            edges_primed: false,
            edges: BTreeMap::new(),
            sentence: None,
            gang_order: None,
            following: None,
            grudges: BTreeSet::new(),
            hunt: None,
            rep: [0.0; 4],
            class: String::new(),
            cur_day: start / TICKS_PER_DAY,
            day_use: BTreeMap::new(),
            day_places: BTreeMap::new(),
            day_net: 0,
            total_use: BTreeMap::new(),
            use_by_day: BTreeMap::new(),
            total_places: BTreeMap::new(),
            total_net: 0,
            flows: BTreeMap::new(),
            contacts: BTreeMap::new(),
            event_kinds: BTreeMap::new(),
            notable: Vec::new(),
            pinned_lod_noted: false,
            last_action: None,
            pinned: true,
            last_lod: None,
            last_pick: None,
            holes_seen: BTreeSet::new(),
            hang_others: BTreeSet::new(),
            hangouts: 0,
            hang_known: BTreeSet::new(),
            flow_kinds: BTreeMap::new(),
            tribute_in: 0,
            gossip_about: 0,
            stat_notes: Vec::new(),
            stat_minutes: 0,
            held_settles: 0,
            held_minutes: 0,
            alive_min: 0,
            work_min: 0,
            sleep_run: 0,
            sleep_blocks: Vec::new(),
            energy0_min: 0,
            starve_min: 0,
            meals: 0,
            meals_synth: 0,
            meals_dive: 0,
            shift: None,
            shifts: Vec::new(),
            wages: Vec::new(),
            refunds: 0,
            work_plan: None,
            commute_aborts: 0,
            bouts: 0,
            bouts_present: 0,
            spouse: None,
            home_tier: None,
            spouse_named_hours: BTreeSet::new(),
            spouse_samples: 0,
            spouse_coloc: 0,
            spouse_coloc_stat: 0,
            hang_empty: 0,
            short_rests: 0,
            source_kinds: BTreeMap::new(),
            cam: Cam::default(),
        }
    }

    fn log(&mut self, tick: Tick, cat: &'static str, text: String) {
        self.seq += 1;
        self.entries.push(Entry { tick, seq: self.seq, cat, text });
    }

    fn contact(&mut self, world: &World, other: EntityId) {
        if other == self.id || other == EntityId::NONE || !world.has::<Identity>(other) {
            return;
        }
        let e = self.contacts.entry(other).or_insert_with(|| (nm(world, other), 0));
        e.1 += 1;
    }
}

/// The activity class of the agent this tick.
fn activity(world: &World, id: EntityId) -> &'static str {
    if world.has::<Sentence>(id) {
        // Pressure pass round 3, S3: lights-out in the cells is sleep.
        if citysim::systems::law::asleep_in_cell(world, id) {
            return "sleep";
        }
        return "jail";
    }
    let Some(b) = world.comp::<Brain>(id) else { return "idle" };
    // V2: a Statistical stand-in (an unpinned run) has no step to classify.
    if b.lod == Lod::Statistical {
        return "stat";
    }
    if matches!(b.exec, ExecState::JackedIn { .. }) {
        return "crime";
    }
    if matches!(b.exec, ExecState::Goto { .. } | ExecState::GotoTimed { .. } | ExecState::Fly { .. }) {
        // Lives at work phase 3 (§ 2.2): a round leg on shift is the work.
        // Lives at work phase 5 (§ 2.4): a reporter's beat leg, likewise.
        // The pre-view pass phase 2 (OPEN-137): a stations shift's walk
        // across the room between its spots, likewise.
        if b.plan_goal() == Some(GoalKind::Work)
            && (citysim::systems::trades::on_round(world, id)
                || citysim::systems::news::on_beat(world, id)
                || b.current_step().is_some_and(|s| s.action == ActionKind::GoTo(citysim::LocationKey::Station))
                // Shadow V7 (tool): any leg of a shape plan (a haul trip,
                // the walk out to the break) is on shift too.
                || citysim::systems::shift::scripted(world, id))
        {
            return "work";
        }
        return "travel";
    }
    let Some(step) = b.plan.as_ref().and_then(|_| b.current_step()) else { return "idle" };
    use ActionKind as A;
    match step.action {
        A::Sleep | A::CheckIn => "sleep",
        A::Rest if b.plan_goal() == Some(GoalKind::Sleep) => "sleep",
        // Lives at work phase 4 (§ 2.3): a cab driver standing by at the Garage.
        A::Rest if b.plan_goal() == Some(GoalKind::Work) && citysim::systems::streetwork::standing_by(world, id) => {
            "work"
        }
        A::Rest => "idle",
        A::FarmWork
        | A::HaulToMarket
        | A::ClerkWork
        | A::BartendWork
        | A::CollectWage
        | A::SellFood
        | A::PatrolLeg
        | A::Arrest
        | A::Escort
        | A::GuardJail
        | A::TendGraves
        | A::BuryCorpse
        | A::CarryCorpse
        | A::ReportCrime
        | A::Deal
        | A::PickUp
        | A::SellData
        | A::Register
        | A::FabWork
        | A::Sweep
        | A::Collect
        // Shadow V7 (tool): the pre-view pass's scripted steps (a stations
        // shift's break, a haul's unload, the leader's court) were `other`.
        | A::Break
        | A::Unload
        | A::Court => "work",
        A::Enjoy | A::Gamble | A::EatOut | A::HangOut => "leisure",
        A::EatFromInventory | A::EatAtHome | A::BuyFood | A::Forage | A::StoreFood => "eat",
        A::Chat | A::Drink | A::Flirt | A::Propose | A::AskAround | A::Preach => "social",
        A::StealFood(_)
        | A::Attack
        | A::Extort
        | A::SplitLoot
        | A::Muster
        | A::Brawl
        | A::StealVehicle
        | A::Strip
        | A::Rip
        | A::Abduct
        | A::Fence
        | A::HideFromLaw
        | A::JackIn
        | A::BuyStims
        | A::UseStim
        | A::StakeOut => "crime",
        A::Wander | A::Beg | A::CollectDole => "idle",
        A::GoTo(_) => "travel",
        A::Meeting => "meeting",
        _ => "other",
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn header(world: &World, t: &Track) -> String {
    let id = t.id;
    let mut h = String::new();
    let _ = writeln!(h, "# {} ({}) - {}#{}", t.name, t.arch, t.arch, id.index);
    let _ = writeln!(h);
    let Some(ident) = world.comp::<Identity>(id) else { return h };
    let sex = if ident.sex == Sex::Male { "male" } else { "female" };
    let _ = writeln!(h, "- **Who**: {} y {}, class {}", ident.age_years(), sex, class_of(world, id).label());
    let _ = writeln!(h, "- **Where**: {}", place(world, id));
    if let Some(p) = world.comp::<Personality>(id) {
        let _ = writeln!(
            h,
            "- **Personality**: lawfulness {:.2}, greed {:.2}, pride {:.2}, sociability {:.2}, courage {:.2}, loyalty {:.2}",
            p.lawfulness, p.greed, p.pride, p.sociability, p.courage, p.loyalty
        );
    }
    if let Some(s) = world.comp::<Skills>(id) {
        let _ = writeln!(
            h,
            "- **Skills**: stealth {:.2}, fighting {:.2}, farming {:.2}, hacking {:.2} | persuasion {:.2}, intimidation {:.2}, knowledge {:.2}, deception {:.2}",
            s.stealth, s.fighting, s.farming, s.hacking, s.persuasion, s.intimidation, s.knowledge, s.deception
        );
    }
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let _ = writeln!(
        h,
        "- **Home**: {}",
        match home {
            Some(b) => place_of_building(world, b),
            None => "none (homeless)".into(),
        }
    );
    if let Some(j) = world.comp::<Job>(id) {
        let _ = writeln!(
            h,
            "- **Job**: {} at {} (wage {}/day, shifts {:?})",
            j.role.label(),
            j.employer.map_or("?".into(), |e| nm(world, e)),
            j.wage_per_day,
            j.shifts
        );
    } else {
        let _ = writeln!(h, "- **Job**: none");
    }
    // The pressure pass F1: the doors and the structural commute between them.
    let door = |b: EntityId| world.comp::<Building>(b).map(|bd| bd.door);
    let home_door = home.and_then(door);
    let work_door = world.comp::<Job>(id).and_then(|j| j.employer).and_then(door);
    if let (Some(hd), Some(wd)) = (home_door, work_door) {
        let _ = writeln!(
            h,
            "- **Commute**: Home door ({}, {}) to work door ({}, {}): {} tiles, travel {} min",
            hd.x,
            hd.y,
            wd.x,
            wd.y,
            hd.manhattan(wd),
            citysim::systems::distance::travel(world, hd, wd)
        );
    }
    let owned: Vec<String> = world
        .buildings_by_kind()
        .values()
        .flatten()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bl| bl.owner == Some(id)))
        .map(|b| bname(world, b))
        .collect();
    if !owned.is_empty() {
        let _ = writeln!(h, "- **Owns**: {}", owned.join(", "));
    }
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<Gang>(g).map(|c| (g, c))) {
        let rank = world.comp::<GangMember>(id).map_or(0, |m| m.rank);
        let role = if g.1.leader == Some(id) { "LEADER" } else { "member" };
        let _ = writeln!(
            h,
            "- **Gang**: {} ({role}, rank {rank}), creed {:?}, order {:?}, treasury {}, {} members",
            g.1.name,
            g.1.creed,
            g.1.order,
            money(g.1.treasury),
            g.1.members.len()
        );
    }
    if let Some(c) = world.corp_of_agent(id).and_then(|c| world.comp::<Corp>(c).map(|k| (c, k))) {
        let exec = if c.1.exec == Some(id) { "EXEC" } else { "staff" };
        let _ =
            writeln!(h, "- **Corp**: {} ({exec}), treasury {}, order {:?}", c.1.name, money(c.1.treasury), c.1.order);
    }
    let wallet = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let inv = world.comp::<Inventory>(id).map(|i| format!("food {}, stims {}, parts {}", i.food, i.stims, i.parts));
    let _ = writeln!(h, "- **Wallet**: {} | inventory: {}", money(wallet), inv.unwrap_or_default());
    // Pressure pass round 2, R1: the fare record (caught at the gate, the El's ban).
    if let Some(r) = world.fares.book.get(&id) {
        let ban = r.banned_until.filter(|&t| t > world.tick).map_or_else(
            || "not banned".to_string(),
            |t| format!("banned from the El until d{}", t / citysim::time::TICKS_PER_DAY),
        );
        let _ = writeln!(h, "- **Fares**: caught at the gate {} time(s) lately, {ban}", r.caught.len());
    }
    if let Some(hl) = world.comp::<citysim::Health>(id) {
        let _ = writeln!(h, "- **Health**: {}", health_line(hl));
    }
    if let Some(k) = world.comp::<Kit>(id) {
        let assets: Vec<String> =
            world.assets_by_owner.get(&id).map(|v| v.iter().map(|&a| world.name_of(a)).collect()).unwrap_or_default();
        let _ = writeln!(
            h,
            "- **Kit**: fighting {:.2}, reflex {:.2}, strength {:.2}, armour {:.2}, chrome {} (value {}), vehicle {}, deck tier {} | assets: {}",
            k.fighting,
            k.reflex,
            k.strength,
            k.armour,
            k.chrome,
            k.chrome_value,
            k.vehicle.map_or("none".into(), |v| world.name_of(v)),
            k.deck_tier,
            if assets.is_empty() { "none".into() } else { assets.join(", ") }
        );
    }
    if let Some(b) = world.comp::<Body>(id) {
        let _ = writeln!(h, "- **Body**: sanity {:.2}, addiction {:.2}", b.sanity, b.addiction);
    }
    let r = rep(world, id);
    let _ = writeln!(
        h,
        "- **Reputation**: dread {:.2}, standing {:.2}, honour {:.2}, heat {:.2}, known by {}",
        r.dread, r.standing, r.honour, r.heat, r.known_by
    );
    // Key relationships: spouse, kin, enemies, rivals, then the warmest.
    let mut es: Vec<(EntityId, f32, f32, RelKind, i32)> = world
        .neighbours(id)
        .filter_map(|o| world.edge(id, o).map(|e| (o, e.affinity, e.trust, e.kind, e.debt)))
        .collect();
    es.sort_by(|a, b| {
        let strong = |k: RelKind| {
            matches!(k, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Enemy | RelKind::Rival)
        };
        strong(b.3).cmp(&strong(a.3)).then(b.1.abs().total_cmp(&a.1.abs())).then(a.0.cmp(&b.0))
    });
    let _ = writeln!(h, "- **Relationships** ({} edges), top:", es.len());
    for (o, aff, trust, kind, debt) in es.into_iter().take(10) {
        let _ = writeln!(
            h,
            "  - {} [{:?}] affinity {:+.2}, trust {:.2}{}",
            nm(world, o),
            kind,
            aff,
            trust,
            if debt != 0 { format!(", debt {debt}") } else { String::new() }
        );
    }
    let _ = writeln!(h);
    h
}

// ---------------------------------------------------------------------------
// Per-tick observation
// ---------------------------------------------------------------------------

fn flow_kind(action: Option<ActionKind>, midnight: bool) -> &'static str {
    use ActionKind as A;
    if midnight {
        return "daily pass (wage/rent/upkeep/tax)";
    }
    match action {
        // L1: a shift's wage is paid as the shift ends.
        Some(
            A::CollectWage
            | A::ClerkWork
            | A::FarmWork
            | A::BartendWork
            | A::TendGraves
            | A::GuardJail
            | A::FabWork
            | A::Sweep,
        ) => "wage",
        Some(A::Enjoy) => "venue entry",
        Some(A::EatOut) => "venue meal",
        Some(A::Gamble) => "gambling",
        Some(A::Collect) => "collect (tribute/fronts)",
        Some(A::Scavenge) => "scavenging",
        Some(A::Meeting) => "office",
        Some(A::BuyFood) => "food purchase",
        Some(A::SellFood | A::HaulToMarket) => "food sale",
        Some(A::Fence) => "fenced goods",
        Some(A::Beg) => "begging",
        Some(A::CollectDole) => "dole",
        Some(A::Extort | A::SplitLoot | A::Strip) => "loot/extortion/split",
        Some(A::BuyAsset) => "asset purchase",
        Some(A::BuyStims | A::UseStim) => "stims purchase",
        Some(A::Deal) => "stims sale",
        Some(A::Install | A::Therapy | A::Detox | A::Uninstall) => "clinic",
        Some(A::CheckIn) => "hotel bed",
        Some(A::Register) => "founding fee",
        Some(A::Drink) => "bar tab",
        Some(A::SellData) => "data sale",
        Some(A::UpgradeDeck) => "deck upgrade",
        Some(A::Arrest | A::Escort) => "law",
        _ => "other",
    }
}

fn goal_line(world: &World, id: EntityId, b: &Brain) -> String {
    let Some(tr) = b.last_think.as_ref() else { return "(no think trace)".into() };
    let chosen = b.current_goal;
    let mut s = String::new();
    if let Some(top) = tr.goals.first() {
        if Some(top.goal) != chosen {
            let _ = write!(s, "(trace winner {:?} deferred by an uninterruptible step) ", top.goal);
        }
    }
    let score = tr.goals.iter().find(|g| Some(g.goal) == chosen);
    match score {
        Some(g) => {
            let _ = write!(s, "score {:.3}", g.score);
        }
        None => {
            let _ = write!(s, "score ?");
        }
    }
    let runners: Vec<String> = tr
        .goals
        .iter()
        .filter(|g| Some(g.goal) != chosen)
        .take(3)
        .map(|g| format!("{:?} {:.3}", g.goal, g.score))
        .collect();
    let _ = write!(s, "; runners-up: {}", if runners.is_empty() { "none".into() } else { runners.join(", ") });
    if let Some(g) = score {
        let cs: Vec<String> =
            g.considerations.iter().map(|c| format!("{} {:.2}->{:.2}", c.name, c.input, c.output)).collect();
        let _ = write!(s, "; because {}", cs.join(", "));
    }
    if chosen != Some(GoalKind::Work) {
        if let Some(why) = work_blocked(world, id) {
            let _ = write!(s, "; Work: {why}");
        }
    }
    s
}

/// The perf pass phase 1 (OPEN-195): why the Work goal cannot run, for the
/// goal line: an exec whose corp has no office (`life::hq_of` skips Homes
/// and Lots: a housing corp's exec never plans Work), a post with no
/// workplace, or the Work goal's refusals (`routine::work_refused`).
fn work_blocked(world: &World, id: EntityId) -> Option<&'static str> {
    use citysim::systems::life;
    let Some(job) = world.comp::<Job>(id) else {
        let corp = life::exec_corp(world, id)?;
        return life::office_of(world, corp, id).is_none().then_some("no Hq");
    };
    if job.employer.is_none() {
        Some("no workplace")
    } else if world.has::<Sentence>(id) {
        Some("sentenced")
    } else if citysim::systems::health::sick(world, id) {
        Some("sick")
    } else if job.days_unpaid >= 7 {
        Some("unpaid a week")
    } else {
        None
    }
}

fn plan_line(world: &World, b: &Brain) -> Option<String> {
    let p = b.plan.as_ref()?;
    let steps: Vec<String> = p
        .steps
        .iter()
        .map(|s| match s.target {
            Some(t) if !matches!(s.action, ActionKind::GoTo(_)) => format!("{:?}->{}", s.action, nm(world, t)),
            _ => format!("{:?}", s.action),
        })
        .collect();
    Some(format!(
        "{:?}{} [{}]",
        p.goal,
        p.target.map_or(String::new(), |t| format!(" on {}", nm(world, t))),
        steps.join(" > ")
    ))
}

fn mem_text(world: &World, holder: EntityId, e: &MemoryEntry) -> String {
    let who = e.subject.map(|s| nm(world, s));
    match (e.kind, e.deed) {
        (MemoryKind::Rumour, Some(d)) => format!(
            "heard a rumour: {:?} by {} on {} (hops {}, conf {:.2})",
            d,
            who.unwrap_or_else(|| "someone unnamed".into()),
            e.object.map_or("?".into(), |o| nm(world, o)),
            e.hops,
            e.conf
        ),
        (MemoryKind::Sighting, _) => format!(
            "sighting of {}{}",
            who.unwrap_or_else(|| "?".into()),
            e.at.map_or(String::new(), |a| format!(" at {}", nm(world, a)))
        ),
        // Lives at work phase 1: the encounter, either side; "a regular" when
        // the holder's ledger (a server's) or the other's (a served's server)
        // holds the pair at two or more visits.
        // Lives at work phase 4 (§ 2.3): a loader's lifts name no person.
        (MemoryKind::Served, _)
            if e.subject.is_none()
                && world
                    .comp::<citysim::Job>(holder)
                    .and_then(|j| citysim::systems::service::row_for(world, j.role))
                    .is_some_and(|r| r.kind == citysim::components::ServeKind::Crate) =>
        {
            let yard = world.comp::<citysim::Job>(holder).and_then(|j| j.employer);
            format!("loaded the shift's crates for {}", world.owner_label(yard.and_then(|y| world.owner_of(y))))
        }
        // Shadow V6 (V5 bug 20): the kind and the side.
        (MemoryKind::Served, _) => served_text(world, holder, e),
        _ => format!(
            "memory {:?}{} (salience {:.2}, valence {:+.2}{}{})",
            e.kind,
            who.map_or(String::new(), |w| format!(" of {w}")),
            e.salience,
            e.valence,
            e.crime.map_or(String::new(), |c| format!(", {c:?}")),
            if e.second_hand { ", second-hand" } else { "" }
        ),
    }
}

/// Everything one tick can add to `t`. `now` is the tick that just ran.
fn observe(world: &World, t: &mut Track, now: Tick, events: &[&Event], dbg: &[&Event], notes: &[ShadowNote]) {
    let id = t.id;
    let day = now / TICKS_PER_DAY;
    // Day rollover: close the finished day.
    if day != t.cur_day {
        flush_day(world, t, now);
        t.cur_day = day;
        // Reputation and class at the day's start.
        if world.has::<Identity>(id) && !world.has::<Corpse>(id) {
            let r = rep(world, id);
            let cur = [r.dread, r.standing, r.honour, r.heat];
            let cls = class_of(world, id).label().to_string();
            if cur.iter().zip(t.rep.iter()).any(|(a, b)| (a - b).abs() >= 0.02) || cls != t.class {
                t.log(
                    now,
                    "rep",
                    format!(
                        "reputation dread {:.2} standing {:.2} honour {:.2} heat {:.2}; class {}",
                        cur[0], cur[1], cur[2], cur[3], cls
                    ),
                );
                t.rep = cur;
                t.class = cls;
            }
        }
    }

    // Events naming the agent (story ring by id cursor).
    let action_now: Option<ActionKind> = world.comp::<Brain>(id).and_then(|b| b.current_step().map(|s| s.action));
    let prev_action = std::mem::replace(&mut t.last_action, action_now);
    // Shadow V6: a courier's parcels handed over (before the round's event).
    cam_parcels(world, t, now);
    let mut event_ctx: Vec<String> = Vec::new();
    for e in events {
        if !e.actors.contains(&id) {
            continue;
        }
        *t.event_kinds.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
        let others: Vec<EntityId> = e.actors.iter().copied().filter(|&a| a != id && world.has::<Identity>(a)).collect();
        for o in others {
            t.contact(world, o);
        }
        let text = format!("{:?}: {}", e.kind, e.text);
        if !matches!(e.kind, EventKind::Witness) {
            t.notable.push((e.tick, text.clone()));
        }
        event_ctx.push(format!("{:?} \"{}\"", e.kind, e.text));
        t.log(e.tick, "event", text);
        // Shadow V6 (V5 bug 20): the courier's round is the parcel round.
        if e.kind == EventKind::SuperRound && e.actors.first() == Some(&id) && is_courier(world, id) {
            let line = format!(
                "the round was the courier's parcel round: {} parcel(s) handed over in the window so far, {} of the yard's still pending",
                t.cam.parcels_handed,
                t.cam.parcels.len()
            );
            t.log(e.tick, "parcel", line);
        }
        if matches!(e.kind, EventKind::Death) && e.actors.first() == Some(&id) && t.died.is_none() {
            t.died = Some(e.tick);
        }
        // `[winner, loser, pit]`: a fighter's bout, and whether she was inside the pit for it.
        if e.kind == EventKind::Bout && e.actors.iter().take(2).any(|&a| a == id) {
            t.bouts += 1;
            let pit = e.actors.get(2).copied();
            if pit.is_some() && world.comp::<Position>(id).and_then(|p| p.building) == pit {
                t.bouts_present += 1;
            }
        }
    }
    for e in dbg {
        if e.actors.first() != Some(&id) {
            continue;
        }
        // "A -> B": interrupted; "A failed at X: reason".
        let label = if e.text.contains(" failed at ") { "PLAN FAILED" } else { "plan interrupted" };
        t.log(e.tick, "plan", format!("{label}: {}", e.text));
        *t.cam.days.entry(e.tick / TICKS_PER_DAY).or_default().aborts.entry(abort_reason(&e.text)).or_insert(0) += 1;
    }
    let mut flow_ctx: Vec<String> = Vec::new();
    // Shadow V4 bug 23: the tick's largest Flow on this agent (the money
    // section buckets by it; the running step read midnight sales as the
    // daily pass and a mid-shift purchase as a wage).
    let mut tick_flow: Option<(i64, String)> = None;
    for n in notes {
        match n {
            ShadowNote::Move { tick, m, out } if m.actor == id || m.target == id => {
                let other = if m.actor == id { m.target } else { m.actor };
                t.contact(world, other);
                if t.spouse == Some(other) {
                    t.spouse_named_hours.insert(*tick / TICKS_PER_HOUR);
                }
                let dir = if m.actor == id { "to" } else { "from" };
                t.log(
                    *tick,
                    "move",
                    format!(
                        "social move {:?} {dir} {} (stake {:?}): {} p={:.2}{}{}",
                        m.kind,
                        nm(world, other),
                        m.stake,
                        if out.success { "SUCCESS" } else { "failed" },
                        out.p,
                        if out.refused { ", refused to meet" } else { "" },
                        if out.backlash { ", BACKLASH" } else { "" }
                    ),
                );
            }
            ShadowNote::Told { tick, from, to, r, heard } if *from == id || *to == id => {
                let other = if *from == id { *to } else { *from };
                t.contact(world, other);
                let verb = if *from == id { "told" } else { "heard from" };
                t.log(
                    *tick,
                    "gossip",
                    format!(
                        "{verb} {}: {:?} by {} on {}{}",
                        nm(world, other),
                        r.deed,
                        r.actor.map_or("someone unnamed".into(), |a| nm(world, a)),
                        r.object.map_or("?".into(), |o| nm(world, o)),
                        if *heard { "" } else { " (already known / dropped)" }
                    ),
                );
            }
            // V2: gossip told ABOUT the agent (as actor or object), heard by others.
            ShadowNote::Told { tick, from, to, r, heard } if r.actor == Some(id) || r.object == Some(id) => {
                t.gossip_about += 1;
                let role = if r.actor == Some(id) { "as the doer" } else { "as the object" };
                t.log(
                    *tick,
                    "gossip",
                    format!(
                        "GOSSIP ABOUT ME ({role}): {} told {}: {:?} by {} on {}{}",
                        nm(world, *from),
                        nm(world, *to),
                        r.deed,
                        r.actor.map_or("someone unnamed".into(), |a| nm(world, a)),
                        r.object.map_or("?".into(), |o| nm(world, o)),
                        if *heard { "" } else { " (already known / dropped)" }
                    ),
                );
            }
            ShadowNote::Flow { tick, from, to, coins, flow, refund } if *from == Some(id) || *to == Some(id) => {
                let other = if *from == Some(id) { *to } else { *from };
                let who = other.map_or("the City".to_string(), |o| nm(world, o));
                // A refund is already oriented owner -> agent: the agent's `in`.
                let (out, inn) = if *from == Some(id) { (*coins, 0) } else { (0, *coins) };
                let label = flow_label(world, *flow, *from, *to, *coins);
                let e = t.flow_kinds.entry(label.clone()).or_insert((0, 0, 0));
                e.0 += out;
                e.1 += inn;
                e.2 += 1;
                if tick_flow.as_ref().is_none_or(|(c, _)| *coins > *c) {
                    tick_flow = Some((*coins, label.clone()));
                }
                if let Some(o) = other {
                    let dir = if *from == Some(id) { "to" } else { "from" };
                    cam_settle(world, t, *tick, o, &format!("{label} {coins}c {dir}"));
                }
                use citysim::systems::ownership::Flow as F;
                if *to == Some(id) && !*refund && matches!(flow, F::Wage | F::ExecWage) && *coins > 0 {
                    t.wages.push(*tick);
                }
                if *to == Some(id) && *refund {
                    t.refunds += 1;
                }
                if matches!(flow, citysim::systems::ownership::Flow::Tribute) && *to == Some(id) {
                    t.tribute_in += *coins;
                    t.log(
                        *tick,
                        "tribute",
                        format!("TRIBUTE received +{coins}c from {who} (the daily stipend or the weekly Collect)"),
                    );
                }
                let sign = if inn > 0 { "+" } else { "-" };
                flow_ctx.push(format!(
                    "{label} {sign}{}{} {} {who}",
                    coins,
                    if *refund { " (refund)" } else { "" },
                    if sign == "+" { "from" } else { "to" }
                ));
            }
            ShadowNote::StatGang { tick, id: who, act, caught } if *who == id => {
                let text = format!(
                    "STATISTICAL GangWork day hit: {act:?}{} (no body, no walk, no witness)",
                    if *caught { ", reported (a body for the arrest path)" } else { "" }
                );
                t.stat_notes.push((*tick, text.clone()));
                t.log(*tick, "stat", text);
            }
            ShadowNote::Settled { tick, id: who, ticks } if *who == id => {
                t.held_settles += 1;
                t.held_minutes += u64::from(*ticks);
                t.log(
                    *tick,
                    "jail",
                    format!(
                        "cell settlement (the hold, or the promotion out of it): {ticks} min of jailed decay settled"
                    ),
                );
            }
            _ => {}
        }
    }

    let alive = world.has::<Brain>(id) || (world.has::<Child>(id) && !world.has::<Corpse>(id));
    if !alive {
        // Dead (or gone): close the open action once and stop.
        if world.has::<Corpse>(id) && t.died.is_none() {
            t.died = Some(now);
            t.log(now, "event", "died (corpse present)".into());
        }
        close_action(world, t, now, "agent died");
        end_sleep_block(t, now);
        return;
    }

    // Place and time use.
    let here = place(world, id);
    let bld = world.comp::<Position>(id).and_then(|p| p.building);
    if here != t.last_place {
        if !t.last_place.is_empty() {
            t.log(now, "move", format!("now at {here} (from {})", t.last_place));
        }
        t.last_place = here.clone();
    }
    t.last_building = bld;
    let act = activity(world, id);
    *t.day_use.entry(act).or_insert(0) += 1;
    *t.total_use.entry(act).or_insert(0) += 1;
    // A held prisoner classes as "jail" but is just as much a stand-in.
    if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
        t.stat_minutes += 1;
    }
    *t.use_by_day.entry(day).or_default().entry(act).or_insert(0) += 1;
    *t.day_places.entry(here.clone()).or_insert(0) += 1;
    *t.total_places.entry(here.clone()).or_insert(0) += 1;
    behaviour_minute(world, t, now, act);
    lives_minute(world, t, now, act, events);
    cam_minute(world, t, now, act, &here);

    if let Some(b) = world.comp::<Brain>(id) {
        if !t.pinned_lod_noted && b.lod == Lod::Full {
            t.pinned_lod_noted = true;
            t.log(now, "info", "at Full LOD (pinned)".into());
        }
        // V2: LOD changes. A pinned pick must stay Full, jailed or not (L1's rule under L2's held
        // prisoners); an unpinned run shows its Statistical and held stretches.
        if t.last_lod != Some(b.lod) {
            if let Some(prev) = t.last_lod {
                let jailed = world.has::<Sentence>(id);
                // Sentencing sets Coarse; the hourly assignment re-pins to Full (up to 59 min
                // later). Statistical is the held tier: a pinned prisoner must never reach it.
                let warn = if t.pinned && b.lod == Lod::Statistical {
                    " **PIN BROKEN: a pinned pick became Statistical (held)**"
                } else if t.pinned && b.lod != Lod::Full && jailed {
                    " (sentencing sets Coarse until the next hourly assignment re-pins it)"
                } else if t.pinned && b.lod != Lod::Full {
                    " **PIN NOT HONOURED: Coarse with no sentence**"
                } else {
                    ""
                };
                t.log(
                    now,
                    "lod",
                    format!(
                        "LOD {prev:?} -> {:?}{}{warn}",
                        b.lod,
                        if jailed && b.lod == Lod::Statistical { " (held in the cells)" } else { "" }
                    ),
                );
            }
            t.last_lod = Some(b.lod);
        }
        // V2: the Unwind pick (rung, satisfier, venue or spot).
        let pick = world.unwind.get(&id).map(|p| {
            format!(
                "rung {:?}, {:?}{}{} (score {:.2})",
                p.rung,
                p.act,
                p.venue.map_or(String::new(), |v| format!(" at {}", nm(world, v))),
                p.spot.map_or(String::new(), |s| format!(" at spot ({},{})", s.x, s.y)),
                p.score
            )
        });
        if pick != t.last_pick {
            if let Some(p) = &pick {
                t.log(now, "unwind", format!("UNWIND pick: {p}"));
            }
            t.last_pick = pick;
        }
        // V2: who is at the HangOut spot (the registry empties as the step completes).
        if action_now == Some(ActionKind::HangOut) {
            if let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) {
                if let Some(list) = world.hangouts.get(&tile) {
                    t.hang_others.extend(list.iter().copied().filter(|&o| o != id));
                }
            }
            // Lives at work phase 2 (§ 3.1): a HangOut inside a Home keeps the
            // household present as its company (the registry holds hangers only).
            if let Some(h) = citysim::systems::household::hanging_at_home(world, id) {
                t.hang_others.extend(citysim::systems::household::residents_inside(world, h, id));
            }
        }
        // Goal choice.
        if b.current_goal != t.last_goal {
            if let Some(g) = b.current_goal {
                t.log(
                    now,
                    "goal",
                    format!(
                        "GOAL {:?}{}: {}",
                        g,
                        t.last_goal.map_or(String::new(), |o| format!(" (was {o:?})")),
                        goal_line(world, id, b)
                    ),
                );
            }
            t.last_goal = b.current_goal;
        }
        // Cooldowns: a failed or unplannable goal cools.
        for (&g, &until) in &b.cooldowns {
            if until > now && t.cooldowns.get(&g) != Some(&until) {
                t.log(
                    now,
                    "plan",
                    format!("goal {g:?} cooled until {} (could not plan or plan failed twice)", clock(until)),
                );
            }
        }
        t.cooldowns = b.cooldowns.clone();
        // Plans.
        let ps = b.plan.as_ref().map(|p| p.started_tick);
        if ps != t.plan_started {
            if ps.is_some() {
                t.cam.days.entry(day).or_default().plans += 1;
            }
            if let Some(line) = plan_line(world, b) {
                t.log(now, "plan", format!("PLAN {line}"));
            }
        }
        // Actions.
        let key =
            b.plan.as_ref().filter(|p| usize::from(b.plan_step) < p.steps.len()).map(|p| (p.started_tick, b.plan_step));
        let open_key = t.open.as_ref().map(|o| o.key);
        if key != open_key {
            let status =
                if t.open.is_some() && key.is_some_and(|k| Some(k.0) == t.plan_started) { "done" } else { "ended" };
            close_action(world, t, now, status);
            if let (Some(k), Some(step)) = (key, b.current_step()) {
                t.open = Some(OpenAction {
                    key: k,
                    action: step.action,
                    target: step.target,
                    start: now,
                    start_place: here.clone(),
                    start_tile: world.comp::<Position>(id).map(|p| p.tile),
                    goal: b.plan.as_ref().map(|p| p.goal),
                });
            }
        }
        t.plan_started = ps;
        // Sentence.
        let sent = world.comp::<Sentence>(id).map(|s| s.until_tick);
        if sent != t.sentence {
            match (t.sentence, sent) {
                (None, Some(until)) => {
                    let crime = world.comp::<Sentence>(id).map(|s| s.crime);
                    t.log(now, "law", format!("JAILED for {crime:?} until {}", clock(until)));
                }
                (Some(_), None) => t.log(now, "law", "released from jail".into()),
                _ => {}
            }
            t.sentence = sent;
        }
        // Gang orders.
        let gord = world.gang_of(id).and_then(|g| world.comp::<Gang>(g)).map(|g| format!("{:?}", g.order));
        if gord != t.gang_order {
            if let Some(o) = &gord {
                t.log(now, "order", format!("gang standing order is now {o}"));
            }
            t.gang_order = gord;
        }
        let fo = b.following_order.map(|o| format!("{o:?}"));
        if fo != t.following {
            match &fo {
                Some(o) => t.log(now, "order", format!("following gang order {o}")),
                None if t.following.is_some() => t.log(now, "order", "no longer following an order".into()),
                None => {}
            }
            t.following = fo;
        }
        if b.dazed_until.is_some_and(|u| u > now) && b.dazed_until != Some(0) && t.last_goal.is_some() {
            // quiet: dazed is visible in the activity class.
        }
        // Needs every two hours.
        if now.is_multiple_of(2 * TICKS_PER_HOUR) {
            if let (Some(n), Some(m)) = (world.comp::<Needs>(id), world.comp::<Mood>(id)) {
                let body = world
                    .comp::<Body>(id)
                    .map(|b| format!(" sanity {:.2} addiction {:.2}", b.sanity, b.addiction))
                    .unwrap_or_default();
                t.log(
                    now,
                    "needs",
                    format!(
                        "needs hunger {:.2} energy {:.2} safety {:.2} wealth {:.2} belonging {:.2} intimacy {:.2} fun {:.2} mood {:.2}{} | {:?} | {:?} | {}",
                        n.hunger, n.energy, n.safety, n.wealth, n.belonging, n.intimacy, n.fun, m.value, body, b.lod, b.current_goal, here
                    ),
                );
            }
        }
    }

    // The pressure pass S6 (OPEN-224): a child has no diary, so a parent's
    // reads the children once a day (09:30, the school day begun): where
    // each is today and the Home's pantry.
    if now % TICKS_PER_DAY == 9 * TICKS_PER_HOUR + 30 {
        let kids: Vec<EntityId> = citysim::systems::demography::children_of_agent(world, id)
            .into_iter()
            .filter(|&c| world.has::<Child>(c))
            .collect();
        if !kids.is_empty() {
            use citysim::systems::ages::Place;
            let parts: Vec<String> = kids
                .iter()
                .map(|&c| {
                    let yrs = world.comp::<citysim::Identity>(c).map_or(0, |i| i.age_years());
                    let at = match citysim::systems::ages::place_of(world, c) {
                        Some(Place::School(s)) => format!("at {}", nm(world, s)),
                        Some(Place::Pitch(_)) => "at the pitch (chores)".to_string(),
                        Some(Place::Home) => "at home (play)".to_string(),
                        Some(Place::Street) => "on the street".to_string(),
                        Some(Place::Camp) => {
                            let camp = world.comp::<citysim::Household>(c).and_then(|h| h.home);
                            camp.map_or("in a Camp".to_string(), |b| format!("in {}", nm(world, b)))
                        }
                        None => "nowhere known".to_string(),
                    };
                    format!("{} ({yrs} y) {at}", nm(world, c))
                })
                .collect();
            let pantry = world
                .comp::<citysim::Household>(id)
                .and_then(|h| h.home)
                .and_then(|h| world.comp::<Building>(h))
                .map_or(String::new(), |b| format!("; the pantry holds {}", b.stock_food));
            t.log(now, "family", format!("my children: {}{pantry}", parts.join(", ")));
        }
    }

    // Hunger as sickness: each meal by kind (the tally rises), and the
    // meters with the needs line.
    if let Some(hl) = world.comp::<citysim::Health>(id) {
        let now_t = hl.today;
        if let Some(prev) = t.tally {
            let rises = [
                (now_t.real > prev.real, "ate a REAL meal"),
                (now_t.synth > prev.synth, "ate SYNTH"),
                (now_t.dive > prev.dive, "ate scraps from a BIN"),
                (now_t.alms > prev.alms, "ate a Mission's SYNTH meal"),
            ];
            for (up, what) in rises {
                if up {
                    t.log(now, "meal", format!("{what} | {}", health_line(hl)));
                }
            }
            let (real, synth, dive, alms) = (
                now_t.real.saturating_sub(prev.real),
                now_t.synth.saturating_sub(prev.synth),
                now_t.dive.saturating_sub(prev.dive),
                now_t.alms.saturating_sub(prev.alms),
            );
            t.meals += u32::from(real) + u32::from(synth) + u32::from(dive) + u32::from(alms);
            t.meals_synth += u32::from(synth) + u32::from(alms);
            t.meals_dive += u32::from(dive);
        }
        t.tally = Some(now_t);
    }
    // Money.
    let mut money_logged = false;
    if let Some(w) = world.comp::<Wallet>(id) {
        if let Some(prev) = t.wallet {
            let d = w.coins - prev;
            if d != 0 {
                let midnight = now % TICKS_PER_DAY == TICKS_PER_DAY - 1 || now.is_multiple_of(TICKS_PER_DAY);
                // A step that changed this tick paid at its completion (wage, dole) or
                // at its start (a purchase): try the finished step first.
                let kind = match flow_kind(prev_action, midnight) {
                    "other" => flow_kind(action_now, midnight),
                    k => k,
                };
                // L1: a guard's shift clock pays at the shift's end, whatever the step.
                let shift_end = world.comp::<citysim::components::Job>(id).is_some_and(|j| {
                    let tod = (now % TICKS_PER_DAY) as u16;
                    let last = if tod == 0 { 1439 } else { tod - 1 };
                    j.on_shift(last) && !j.on_shift(tod)
                });
                let kind = if kind == "other" && d > 0 && shift_end { "wage" } else { kind };
                // L1: the dole paid in place at 09:00.
                let kind = if kind == "other"
                    && d > 0
                    && now % TICKS_PER_DAY == 540
                    && world.comp::<citysim::components::Job>(id).is_none()
                {
                    "dole"
                } else {
                    kind
                };
                // Shadow V4 bug 23: a Flow on the books names the money.
                let kind: String = tick_flow.as_ref().map_or_else(|| kind.to_string(), |(_, f)| f.clone());
                let kind = kind.as_str();
                let owner = bld.and_then(|b| owner_name(world, b));
                let cp = owner.map_or(String::new(), |o| format!("; place owner {o}"));
                let ev =
                    if event_ctx.is_empty() { String::new() } else { format!("; events: {}", event_ctx.join(" | ")) };
                let fl = if flow_ctx.is_empty() { String::new() } else { format!("; flows: {}", flow_ctx.join(", ")) };
                money_logged = true;
                if world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
                    t.stat_notes.push((
                        now,
                        format!("stand-in purse {}{} ({kind}){fl}", if d > 0 { "+" } else { "" }, money(d)),
                    ));
                }
                t.log(
                    now,
                    "money",
                    format!(
                        "{}{} ({kind}), purse {}{cp}{ev}{fl}",
                        if d > 0 { "+" } else { "" },
                        money(d),
                        money(w.coins)
                    ),
                );
                t.day_net += d;
                t.total_net += d;
                let f = t.flows.entry(kind.to_string()).or_insert((0, 0));
                f.0 += d;
                f.1 += 1;
            }
        }
        t.wallet = Some(w.coins);
    }
    if !money_logged && !flow_ctx.is_empty() {
        t.log(now, "money", format!("(net 0c) flows: {}", flow_ctx.join(", ")));
    }
    // V2: off-screen holes against the agent (a Statistical victim's crimes, actor not yet drawn).
    // `Hole.source`/`faction`/`riot` come from the daily off-screen faction pass (L2 phase 4).
    if let Some(hs) = world.holes_by_agent.get(&id) {
        for hid in hs.iter() {
            if !t.holes_seen.insert(*hid) {
                continue;
            }
            if let Some(h) = world.holes.get(hid).filter(|h| h.tick + 2 >= now) {
                t.log(
                    h.tick,
                    "hole",
                    format!(
                        "OFF-SCREEN {:?} against me in {}{}{}{}{}{}",
                        h.kind,
                        world.district_name(h.district),
                        if h.loot > 0 { format!(" (loot {}c)", h.loot) } else { String::new() },
                        h.gang.map_or(String::new(), |g| format!(", my gang {}", nm(world, g))),
                        h.source.map_or(String::new(), |s| format!(", source {s:?}")),
                        h.faction.map_or(String::new(), |f| format!(", by {}", nm(world, f))),
                        h.riot.map_or(String::new(), |r| format!(", riot #{r}"))
                    ),
                );
            }
        }
    }
    // Inventory.
    if let Some(i) = world.comp::<Inventory>(id) {
        let cur = (i.food, i.stolen_food, i.stims, i.parts);
        if let Some(p) = t.inv {
            if p != cur {
                t.log(
                    now,
                    "inv",
                    format!(
                        "inventory food {}->{} (stolen {}->{}), stims {}->{}, parts {}->{}",
                        p.0, cur.0, p.1, cur.1, p.2, cur.2, p.3, cur.3
                    ),
                );
            }
        }
        t.inv = Some(cur);
    }
    let mut mem_msgs: Vec<String> = Vec::new();
    let mut edge_msgs: Vec<String> = Vec::new();
    // Memories.
    if let Some(m) = world.comp::<Memory>(id) {
        let prime = !t.mem_primed;
        t.mem_primed = true;
        for (e, heard) in m.entries.iter().map(|e| (e, false)).chain(m.heard.iter().map(|e| (e, true))) {
            let key = (e.tick, e.kind, e.subject, heard);
            if t.mem_seen.insert(key) && !prime {
                // Lives at work phase 1: an encounter on shift (the fourth Served-able signal).
                if e.kind == MemoryKind::Served && !heard {
                    if let Some(s) = t.shift.as_mut() {
                        s.encounters += 1;
                    }
                }
                if let Some(s) = e.subject {
                    if matches!(
                        e.kind,
                        MemoryKind::Socialised
                            | MemoryKind::Fought
                            | MemoryKind::Won
                            | MemoryKind::Lost
                            | MemoryKind::Bout
                            | MemoryKind::Courted
                    ) {
                        t.contact(world, s);
                    }
                }
                let text = mem_text(world, id, e);
                // Shadow V6: an encounter with the counterpart elsewhere.
                if e.kind == MemoryKind::Served && !heard {
                    if let Some(s) = e.subject {
                        cam_settle(world, t, now, s, &format!("memory: {text}"));
                    }
                }
                // Rumours heard already appear as gossip lines when told in person.
                let old = if now.saturating_sub(e.tick) > 60 {
                    format!(" [deed from {}]", clock(e.tick))
                } else {
                    String::new()
                };
                mem_msgs.push(format!("{text}{old}"));
            }
        }
    }
    // Edges.
    let mut seen: BTreeSet<EntityId> = BTreeSet::new();
    let ns: Vec<EntityId> = world.neighbours(id).collect();
    for o in ns {
        let Some(e) = world.edge(id, o) else { continue };
        seen.insert(o);
        match t.edges.get(&o).copied() {
            None => {
                if t.edges_primed {
                    edge_msgs.push(format!(
                        "new relationship with {}: {:?} affinity {:+.2}",
                        nm(world, o),
                        e.kind,
                        e.affinity
                    ));
                }
                t.edges.insert(o, (e.affinity, e.kind));
            }
            Some((aff, kind)) => {
                if kind != e.kind {
                    edge_msgs.push(format!(
                        "{} is now {:?} (was {:?}), affinity {:+.2}",
                        nm(world, o),
                        e.kind,
                        kind,
                        e.affinity
                    ));
                    t.edges.insert(o, (e.affinity, e.kind));
                } else if (e.affinity - aff).abs() >= 0.1 {
                    edge_msgs.push(format!(
                        "affinity to {} {:+.2} -> {:+.2} ({:?})",
                        nm(world, o),
                        aff,
                        e.affinity,
                        e.kind
                    ));
                    t.edges.insert(o, (e.affinity, e.kind));
                }
            }
        }
    }
    t.edges.retain(|k, _| seen.contains(k));
    log_grouped(t, now, "memory", mem_msgs, "memories");
    log_grouped(t, now, "edge", edge_msgs, "relationship changes");
    t.edges_primed = true;
    // Grudges and Hunts.
    if let Some(Some(gs)) = world.grudges.get(id.index as usize) {
        for g in &gs.list {
            if t.grudges.insert((g.target, g.since)) {
                t.log(
                    now,
                    "grudge",
                    format!(
                        "GRUDGE against {} ({:?}, weight {:.2}, chain {})",
                        nm(world, g.target),
                        g.cause,
                        g.weight,
                        g.chain
                    ),
                );
            }
        }
    }
    let hunt = world.hunts.get(&id).map(|h| h.target);
    if hunt != t.hunt {
        match (hunt, world.hunts.get(&id)) {
            (Some(tg), Some(h)) => {
                t.log(now, "grudge", format!("HUNT begins on {} ({:?}, phase {:?})", nm(world, tg), h.why, h.phase))
            }
            _ => t.log(now, "grudge", "hunt over".into()),
        }
        t.hunt = hunt;
    }
}

/// The behaviour tier's per-minute tallies (sleep blocks, energy 0, shifts,
/// the Work plan's commute). Reads only; no RNG.
fn behaviour_minute(world: &World, t: &mut Track, now: Tick, act: &'static str) {
    let id = t.id;
    t.alive_min += 1;
    t.home_tier = world
        .comp::<citysim::components::Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<citysim::Building>(h))
        .map(|b| b.tier);
    if act == "work" {
        t.work_min += 1;
    }
    if act == "sleep" {
        t.sleep_run += 1;
    } else {
        end_sleep_block(t, now);
    }
    let jailed = world.has::<Sentence>(id);
    // Free minutes only: a pinned prisoner is kept at Full in the cells, which the
    // city never does (a sentenced agent is held Statistical, `lod::held_on`).
    if !jailed && world.comp::<Needs>(id).is_some_and(|n| n.energy <= 0.001) {
        t.energy0_min += 1;
    }
    let starving = world.config.life.starving_hunger;
    if !jailed && world.comp::<Needs>(id).is_some_and(|n| n.hunger < starving) {
        t.starve_min += 1;
    }
    let tod = (now % TICKS_PER_DAY) as u16;
    // Lives at work phase 5: an exec's office hours are his shift for `--lives`.
    let (os, oe) = world.config.life.exec_shift;
    let office = (!world.has::<Job>(id) && tod >= os && tod < oe)
        .then(|| citysim::systems::life::exec_corp(world, id))
        .flatten();
    match (world.comp::<Job>(id), office) {
        (Some(j), _) if j.on_shift(tod) => {
            let s = t.shift.get_or_insert_with(|| ShiftObs {
                start: now,
                end: now,
                len: 0,
                work: 0,
                workday: citysim::exec::routine::workday_of(world, id, j, j.shift_key_at(now)),
                whole: j.shifts.iter().any(|&(s, _)| s == tod),
                free: true,
                employer: j.employer,
                own_post: j.employer.is_some_and(|e| world.owner_of(e) == Some(id)),
                samples: 0,
                nonstaff: 0,
                source_events: 0,
                venue_visits: 0,
                last_visits: None,
                encounters: 0,
                office: false,
            });
            s.len += 1;
            s.work += u32::from(act == "work");
            s.free &= !jailed;
            s.end = now;
        }
        (None, Some(corp)) => {
            let day = (now / TICKS_PER_DAY) as i64;
            let s = t.shift.get_or_insert_with(|| ShiftObs {
                start: now,
                end: now,
                len: 0,
                work: 0,
                workday: citysim::exec::routine::is_workday(day),
                whole: tod == os,
                free: true,
                employer: citysim::systems::life::hq_of(world, corp, id),
                own_post: false,
                samples: 0,
                nonstaff: 0,
                source_events: 0,
                venue_visits: 0,
                last_visits: None,
                encounters: 0,
                office: true,
            });
            s.len += 1;
            s.work += u32::from(act == "work");
            s.free &= !jailed;
            s.end = now;
        }
        _ => {
            if let Some(s) = t.shift.take() {
                t.shifts.push(s);
            }
        }
    }
    let Some(b) = world.comp::<Brain>(id) else { return };
    // A run of the Work goal (successive Work plans included: the walk and the
    // wait at the door, then the shift's own plan) that ends before a single
    // work-class minute is a commute turned back.
    if b.current_goal == Some(GoalKind::Work) {
        let reached = t.work_plan.get_or_insert(false);
        *reached |= act == "work";
    } else if let Some(reached) = t.work_plan.take() {
        t.commute_aborts += u32::from(!reached);
    }
}

fn end_sleep_block(t: &mut Track, now: Tick) {
    if t.sleep_run > 0 {
        t.sleep_blocks.push((now, t.sleep_run));
        t.sleep_run = 0;
    }
}

/// Lives at work phase 0 (`--lives`): the spouse and the counter, read once
/// an hour (plus the tick's events at the workplace). Reads only; no RNG.
/// Runs after `behaviour_minute`, so `t.shift` is this minute's.
fn lives_minute(world: &World, t: &mut Track, now: Tick, act: &'static str, events: &[&Event]) {
    let id = t.id;
    // The tick's source events at the workplace (every tick: an event is one minute).
    if let Some(s) = t.shift.as_mut() {
        if let Some(wp) = s.employer {
            for e in events.iter().filter(|e| SERVED_SOURCE_KINDS.contains(&e.kind)) {
                let at_work = e.actors.contains(&wp)
                    || e.actors.iter().any(|&a| {
                        a != id && a != EntityId::NONE && world.comp::<Position>(a).and_then(|p| p.building) == Some(wp)
                    });
                if at_work {
                    s.source_events += 1;
                    *t.source_kinds.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
                }
            }
        }
    }
    if !now.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    // The counter: non-staff bodies inside the workplace, and the venue's visit tally.
    if let Some(s) = t.shift.as_mut() {
        if let Some(bd) = s.employer.and_then(|wp| world.comp::<Building>(wp)) {
            let staff = world.staff_of(s.employer.unwrap_or(EntityId::NONE));
            s.samples += 1;
            s.nonstaff += bd.occupants.iter().filter(|&&o| o != id && staff.binary_search(&o).is_err()).count() as u32;
            if let Some(v) = bd.venue.as_ref() {
                // `visits_today` resets at midnight: a fall is a new day's count.
                let cur = v.visits_today;
                if let Some(prev) = s.last_visits {
                    s.venue_visits += u32::from(if cur >= prev { cur - prev } else { cur });
                }
                s.last_visits = Some(cur);
            }
        }
    }
    // The spouse: co-located and both awake.
    let Some(sp) = t.spouse else { return };
    if !world.has::<Brain>(sp) || world.has::<Corpse>(sp) {
        return;
    }
    t.spouse_samples += 1;
    let (Some(a), Some(b)) = (world.comp::<Position>(id), world.comp::<Position>(sp)) else { return };
    let together = match (a.building, b.building) {
        (Some(x), Some(y)) => x == y,
        (None, None) => a.tile.manhattan(b.tile) <= COLOC_TILES,
        _ => false,
    };
    let sp_act = activity(world, sp);
    if together && act != "sleep" && sp_act != "sleep" {
        t.spouse_coloc += 1;
        t.spouse_coloc_stat += u32::from(sp_act == "stat");
    }
}

/// Log up to three lines, then one line counting the rest (a gang-wide feud
/// adjustment touches dozens of edges in one minute).
fn log_grouped(t: &mut Track, now: Tick, cat: &'static str, msgs: Vec<String>, noun: &str) {
    let n = msgs.len();
    for m in msgs.into_iter().take(3) {
        t.log(now, cat, m);
    }
    if n > 3 {
        t.log(now, cat, format!("... and {} more {noun} in the same minute", n - 3));
    }
}

fn close_action(world: &World, t: &mut Track, now: Tick, status: &str) {
    let Some(o) = t.open.take() else { return };
    let dur = now.saturating_sub(o.start);
    // The perf pass phase 1 (OPEN-195): an idle Rest cut within minutes.
    if o.action == ActionKind::Rest && o.goal != Some(GoalKind::Work) && dur <= SHORT_REST_MIN {
        t.short_rests += 1;
    }
    let end_place = t.last_place.clone();
    let tgt = o.target.map_or(String::new(), |x| format!(" [target {}]", nm(world, x)));
    if o.action == ActionKind::HangOut {
        // V2: who was at the spot, the known contacts named.
        let others: Vec<EntityId> = std::mem::take(&mut t.hang_others).into_iter().collect();
        t.hangouts += 1;
        t.hang_empty += u32::from(others.is_empty());
        if t.spouse.is_some_and(|s| others.contains(&s)) {
            t.spouse_named_hours.insert(o.start / TICKS_PER_HOUR);
        }
        let mut parts: Vec<String> = Vec::new();
        let mut known = 0;
        let mut strangers = 0;
        for &x in &others {
            match world.edge(t.id, x) {
                Some(e) => {
                    known += 1;
                    t.hang_known.insert(x);
                    t.contact(world, x);
                    parts.push(format!("{} (known, {:?}, affinity {:+.2})", nm(world, x), e.kind, e.affinity));
                }
                None => strangers += 1,
            }
        }
        if strangers > 0 {
            parts.push(format!("{strangers} stranger(s)"));
        }
        t.log(
            o.start,
            "hangout",
            format!(
                "HANGOUT at {} for {dur} min ({status}): {} there{}",
                t.last_place,
                if others.is_empty() { "nobody".to_string() } else { format!("{} other(s)", others.len()) },
                if parts.is_empty() { String::new() } else { format!(": {} [{known} known]", parts.join(", ")) }
            ),
        );
    }
    let loc = if matches!(o.action, ActionKind::GoTo(_)) {
        if o.start_place == end_place {
            end_place.to_string()
        } else if status == "done" {
            format!("{} -> {}", o.start_place, end_place)
        } else {
            // OPEN-200: an interrupted leg's end is where it was cut, not
            // where it was going (the target is printed after).
            format!("{} -> cut at {}", o.start_place, end_place)
        }
    } else {
        end_place
    };
    if matches!(o.action, ActionKind::GoTo(_)) && dur > CAM_LONG_GOTO_MIN {
        t.cam.days.entry(o.start / TICKS_PER_DAY).or_default().goto_long += 1;
    }
    // The pressure pass F1: a GoTo's Manhattan tiles from its start.
    let tiles = match (o.action, o.start_tile, world.comp::<Position>(t.id).map(|p| p.tile)) {
        (ActionKind::GoTo(_), Some(a), Some(b)) => format!(", {} tiles", a.manhattan(b)),
        _ => String::new(),
    };
    t.log(o.start, "action", format!("ACTION {:?} @ {loc}, {dur} min{tiles} ({status}){tgt}", o.action));
}

fn flush_day(world: &World, t: &mut Track, now: Tick) {
    let day = t.cur_day;
    // The pressure pass R4 (OPEN-219): the shiftless rest day (weekday 6, a
    // guard's staggered one) is marked, so its zero work does not read as a bug.
    let rest = rest_day_note(world, t.id, day);
    let tick = day * TICKS_PER_DAY + TICKS_PER_DAY - 1;
    let _ = now;
    let hours: Vec<String> = CLASSES
        .iter()
        .filter_map(|c| t.day_use.get(c).filter(|&&m| m > 0).map(|&m| format!("{c} {:.1}h", f64::from(m) / 60.0)))
        .collect();
    let mut pl: Vec<(&String, &u32)> = t.day_places.iter().collect();
    pl.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let places: Vec<String> = pl.iter().filter(|(_, &m)| m >= 5).take(8).map(|(p, m)| format!("{p} {m}m")).collect();
    let text = format!(
        "DAY SUMMARY d{day}{rest}: {} | net {}{} | places: {}",
        if hours.is_empty() { "no activity".into() } else { hours.join(", ") },
        if t.day_net > 0 { "+" } else { "" },
        money(t.day_net),
        if places.is_empty() { "none".into() } else { places.join("; ") }
    );
    t.log(tick, "summary", text);
    t.day_use.clear();
    t.day_places.clear();
    t.day_net = 0;
}

/// The pressure pass R4: " (rest day: no shift)" when `id` holds a job with
/// no shift on `day` (weekday 6, a guard's staggered day), " (the city's rest
/// day)" for the jobless on weekday 6, else nothing.
fn rest_day_note(world: &World, id: EntityId, day: u64) -> &'static str {
    let key = i64::try_from(day).unwrap_or(i64::MAX);
    match world.comp::<Job>(id) {
        Some(j) if !citysim::exec::routine::workday_of(world, id, j, key) => " (rest day: no shift)",
        Some(_) => "",
        None if !citysim::exec::routine::is_workday(key) => " (the city's rest day)",
        None => "",
    }
}

// ---------------------------------------------------------------------------
// Shadow V6: the camera lens and V5 bug 20's labels
// ---------------------------------------------------------------------------

/// The camera's place of `id` this minute (an index into `CAM_PLACES`):
/// held (a sentence), Statistical, Coarse, a vehicle (the El or a flyer,
/// boarding included, or a walk with a vehicle under the agent), then by
/// the building: Home, work (the employer, an exec's Hq, or a work-class or
/// Meeting step anywhere), another building, else the street.
fn cam_place(world: &World, id: EntityId, act: &str) -> usize {
    if world.has::<Sentence>(id) {
        return CAM_HELD;
    }
    if let Some(b) = world.comp::<Brain>(id) {
        match b.lod {
            Lod::Statistical => return CAM_STAT,
            Lod::Coarse => return CAM_COARSE,
            Lod::Full => {}
        }
        let driving = world.comp::<Kit>(id).is_some_and(|k| k.driving.is_some());
        match b.exec {
            ExecState::Fly { .. } | ExecState::Ride { .. } => return CAM_VEHICLE,
            ExecState::Goto { .. } if driving => return CAM_VEHICLE,
            _ => {}
        }
        if pass_vehicle(world, id).is_some() || citysim::systems::motion::vehicle_of(world, id).is_some() {
            return CAM_VEHICLE;
        }
    }
    let Some(bl) = world.comp::<Position>(id).and_then(|p| p.building) else { return CAM_STREET };
    if world.comp::<Household>(id).and_then(|h| h.home) == Some(bl) {
        return CAM_HOME;
    }
    let employer = world.comp::<Job>(id).and_then(|j| j.employer).or_else(|| {
        citysim::systems::life::exec_corp(world, id).and_then(|c| citysim::systems::life::hq_of(world, c, id))
    });
    if employer == Some(bl) || act == "work" || act == "meeting" {
        CAM_WORK
    } else {
        CAM_BUILDING
    }
}

/// The exec state as a small code (part of an unchanged stretch's key).
fn exec_code(e: &ExecState) -> u8 {
    match e {
        ExecState::Idle => 0,
        ExecState::Goto { .. } => 1,
        ExecState::GotoTimed { .. } => 2,
        ExecState::Use { .. } => 3,
        ExecState::Wait { .. } => 4,
        ExecState::Fly { .. } => 5,
        ExecState::Ride { .. } => 6,
        ExecState::JackedIn { .. } => 7,
    }
}

fn exec_name(e: &ExecState) -> &'static str {
    match e {
        ExecState::Idle => "Idle",
        ExecState::Goto { .. } => "Goto",
        ExecState::GotoTimed { .. } => "GotoTimed (Coarse walk)",
        ExecState::Use { .. } => "Use",
        ExecState::Wait { .. } => "Wait",
        ExecState::Fly { .. } => "Fly",
        ExecState::Ride { .. } => "Ride (the El)",
        ExecState::JackedIn { .. } => "JackedIn",
    }
}

/// Chebyshev tiles a tick in this exec state may cover: a walk 1 (a
/// walker's step is 8 or fewer quarter ticks), a drive `[vehicles]
/// max_steps_per_tick`, a flyer `1 / flyer_ticks_per_tile`, the El `1 /
/// ride_ticks_per_tile` (the track is no shorter than the Chebyshev
/// distance), a Coarse timed walk 1 (generous: it is timed at 1.5 walk
/// ticks a Manhattan tile); anything else 0.
fn exec_speed(world: &World, id: EntityId, e: &ExecState) -> f32 {
    if let Some(v) = pass_vehicle(world, id) {
        return v;
    }
    match e {
        ExecState::Goto { .. } if world.comp::<Kit>(id).is_some_and(|k| k.driving.is_some()) => {
            f32::from(world.config.vehicles.max_steps_per_tick.max(1))
        }
        ExecState::Goto { .. } | ExecState::GotoTimed { .. } => 1.0,
        ExecState::Fly { .. } => 1.0 / world.config.vehicles.flyer_ticks_per_tile.max(0.01),
        ExecState::Ride { .. } => 1.0 / world.config.transit.ride_ticks_per_tile.max(0.01),
        _ => 0.0,
    }
}

/// Shadow V7 (tool): the pre-view pass's timed vehicle legs of a Full body,
/// which V6's lens read as a street walk at 1 tile a tick: the escort van
/// (an `Escort` step's `GotoTimed`, `exec::escort_leg`; its suspect, cuffed,
/// rides along), a cab driver's fare leg (`GotoTimed` on a fare,
/// `exec::fare_segment`) and a rider aboard a body cab (`Jobs2State.
/// passengers`, the ride under way). Returns the leg's tiles a tick, the
/// cab's timed speed (`streetwork::cab_ticks`: `move_ticks_full` x the
/// Car's `timed_mult` a Manhattan tile). `None`: not one of these.
fn pass_vehicle(world: &World, id: EntityId) -> Option<f32> {
    let b = world.comp::<Brain>(id)?;
    let timed = |x: EntityId| {
        world.comp::<Brain>(x).is_some_and(|bx| matches!(bx.exec, ExecState::GotoTimed { .. }) && bx.lod == Lod::Full)
    };
    let van = |x: EntityId| {
        timed(x)
            && world.comp::<Brain>(x).and_then(|bx| bx.current_step()).is_some_and(|s| s.action == ActionKind::Escort)
    };
    let fare = timed(id) && citysim::systems::streetwork::on_fare(world, id);
    let aboard = world.jobs2.passengers.get(&id).is_some_and(|&d| {
        world.jobs2.cab_rides.iter().any(|r| r.body && r.rider == id && r.driver == d && r.stage >= 1)
    });
    let cuffed = b.cuffed_by.is_some_and(van);
    if !(van(id) || fare || aboard || cuffed) {
        return None;
    }
    // The pressure pass R2: the vans drive the road at its own speed
    // (`vehicles::road_ticks`), faster than the cab's Coarse blend.
    let car = citysim::components::AssetKind::Car;
    let m = citysim::systems::vehicles::timed_mult_of(world, car).min(world.config.vehicles.mult_of(car, 0));
    Some(1.0 / (world.config.exec.move_ticks_full.max(1) as f32 * m).max(0.01))
}

/// Shadow V8: a vehicle in motion's own move over the last two ticks (its
/// route's place then and now; a driven car `[vehicles] max_steps_per_tick`
/// a tick). Generous by a tick, so a boarding minute is never a jump for
/// the vehicle's pace alone.
fn veh_step(world: &World, v: EntityId) -> f32 {
    use citysim::systems::{motion, transit};
    let Some(m) = world.motions.get(&v) else { return 0.0 };
    let t = world.tick;
    let at = |k: Tick| match &m.route {
        motion::Route::Timed { way, depart, arrive } => Some(motion::timed_pos(way, *depart, *arrive, k)),
        motion::Route::Line { line, train } => {
            transit::train_at(world, *line, u64::from(*train), k).and_then(|a| transit::train_tile(world, *line, a))
        }
        motion::Route::Driven => None,
    };
    match (at(t.saturating_sub(2)), at(t)) {
        (Some(a), Some(b)) => chebyshev(a, b) as f32,
        _ => 2.0 * f32::from(world.config.vehicles.max_steps_per_tick.max(1)),
    }
}

fn chebyshev(a: citysim::TilePos, b: citysim::TilePos) -> u32 {
    u32::from(a.x.abs_diff(b.x)).max(u32::from(a.y.abs_diff(b.y)))
}

/// Two agents in one building, or both outdoors within `COLOC_TILES`.
fn colocated(world: &World, a: EntityId, b: EntityId) -> bool {
    let (Some(x), Some(y)) = (world.comp::<Position>(a), world.comp::<Position>(b)) else { return false };
    match (x.building, y.building) {
        (Some(p), Some(q)) => p == q,
        (None, None) => x.tile.manhattan(y.tile) <= COLOC_TILES,
        _ => false,
    }
}

/// The camera's minute: the place, the jump check, the unchanged stretch.
/// Reads only; no RNG.
fn cam_minute(world: &World, t: &mut Track, now: Tick, act: &'static str, here: &str) {
    let id = t.id;
    let place = cam_place(world, id, act);
    t.cam.days.entry(now / TICKS_PER_DAY).or_default().place_min[place] += 1;
    let Some(pos) = world.comp::<Position>(id).cloned() else { return };
    let brain = world.comp::<Brain>(id);
    // Shadow V7: the work-class run on one plan step, and the work tiles.
    let step_key = brain.and_then(|b| b.plan.as_ref().map(|p| (p.started_tick, b.plan_step)));
    match step_key.filter(|_| act == "work") {
        Some(k) => {
            let start = match t.cam.work_run {
                Some((rk, s)) if rk == k => s,
                _ => now,
            };
            t.cam.work_run = Some((k, start));
            let len = u32::try_from(now - start + 1).unwrap_or(u32::MAX);
            let d = t.cam.days.entry(start / TICKS_PER_DAY).or_default();
            d.work_run_max = d.work_run_max.max(len);
            t.cam.days.entry(now / TICKS_PER_DAY).or_default().work_tiles.insert(pos.tile);
        }
        None => t.cam.work_run = None,
    }
    let lod = brain.map(|b| b.lod);
    let held = world.has::<Sentence>(id);
    // The jump: the allowance accrued in the state the tick ran under. Inside
    // a building the body is at its door (the camera draws the building; a
    // step in through the door lands on an interior tile).
    let at = pos.building.and_then(|b| world.comp::<Building>(b)).map_or(pos.tile, |b| b.door);
    t.cam.allow += t.cam.speed;
    // Shadow V8: a rider aboard a vehicle in motion (a car, cab, flyer, van
    // or El train, `systems/motion.rs`) is where the vehicle is, so the
    // vehicle's own move since the last minute is allowance. A snap onto
    // the vehicle at boarding still reads as a jump; the vehicle's own
    // continuity is the `run --legs` `#ride` device's to judge.
    // `Motion::pos` is where `motion::advance` set its riders (the route's
    // place at the tick's own time; `motion::pos_of` reads the timetable
    // at `world.tick` and runs a tick ahead of the riders).
    let veh = citysim::systems::motion::vehicle_of(world, id).and_then(|v| world.motions.get(&v).map(|m| (v, m.pos)));
    match (veh, t.cam.veh) {
        (Some((v, p)), Some((pv, pp))) if v == pv => t.cam.allow += chebyshev(pp, p) as f32,
        // The boarding minute: the vehicle may already have pulled away
        // (a train departs the tick its rider boards), so its own step is
        // allowance too.
        (Some((v, _)), _) => t.cam.allow += veh_step(world, v),
        _ => {}
    }
    t.cam.veh = veh;
    let mut jump_line: Option<String> = None;
    match t.cam.anchor.clone() {
        None => t.cam.anchor = Some((at, now, here.to_string())),
        Some((tile, since, from)) if tile != at => {
            let dist = chebyshev(tile, at);
            if dist as f32 > t.cam.allow + CAM_JUMP_SLACK {
                let body = t.cam.prev_lod == Some(Lod::Full) && lod == Some(Lod::Full) && !t.cam.prev_held && !held;
                let d = t.cam.days.entry(now / TICKS_PER_DAY).or_default();
                d.jumps += 1;
                d.jumps_body += u32::from(body);
                d.max_jump = d.max_jump.max(dist);
                d.first_jump.get_or_insert(now);
                let lods = match (t.cam.prev_lod, lod) {
                    (Some(a), Some(b)) if a != b => format!("LOD {a:?} -> {b:?}"),
                    (_, Some(b)) => format!("LOD {b:?}"),
                    _ => "no LOD".into(),
                };
                let jail = match (t.cam.prev_held, held) {
                    (false, true) => ", sentenced",
                    (true, false) => ", released",
                    (true, true) => ", held",
                    _ => "",
                };
                let step = brain.and_then(|b| b.current_step()).map_or("no step".into(), |s| format!("{:?}", s.action));
                jump_line = Some(format!(
                    "JUMP{} {dist} tiles in {} min (allowance {:.0}): {from} -> {here}; {lods}{jail}; exec {}, {step}",
                    if body { " (a Full body)" } else { "" },
                    now.saturating_sub(since),
                    t.cam.allow + CAM_JUMP_SLACK,
                    brain.map_or("none", |b| exec_name(&b.exec)),
                ));
            }
            t.cam.anchor = Some((at, now, here.to_string()));
            t.cam.allow = 0.0;
        }
        Some(_) => {}
    }
    t.cam.speed = brain.map_or(0.0, |b| exec_speed(world, id, &b.exec));
    t.cam.prev_lod = lod;
    t.cam.prev_held = held;
    if let Some(l) = jump_line {
        t.log(now, "camera", l);
    }
    // The unchanged stretch.
    let key: StillKey = (
        pos.tile,
        pos.building,
        brain.and_then(|b| b.current_step().map(|s| s.action)),
        brain.map_or(u8::MAX, |b| exec_code(&b.exec)),
    );
    if t.cam.still_key != Some(key) {
        close_still(t, now);
        t.cam.still_key = Some(key);
        t.cam.still_start = now;
        t.cam.still_act = act;
        t.cam.still_place = here.to_string();
    }
}

/// Close the unchanged stretch under way at `now` (its minutes credit the
/// day it began).
fn close_still(t: &mut Track, now: Tick) {
    if t.cam.still_key.take().is_none() {
        return;
    }
    let len = u32::try_from(now.saturating_sub(t.cam.still_start)).unwrap_or(u32::MAX);
    let rec = (len, t.cam.still_start, t.cam.still_act, t.cam.still_place.clone());
    let awake = t.cam.still_act != "sleep";
    let d = t.cam.days.entry(t.cam.still_start / TICKS_PER_DAY).or_default();
    if d.still.as_ref().is_none_or(|s| len > s.0) {
        d.still = Some(rec.clone());
    }
    if awake && d.still_awake.as_ref().is_none_or(|s| len > s.0) {
        d.still_awake = Some(rec);
    }
}

/// A settlement naming a person (a flow note or a `Served` memory): counted,
/// and logged when the counterpart is not a body beside the pick, or the
/// pick is asleep (V5 bugs 6 and 7: the encounter narrates a settlement the
/// body was not present for).
fn cam_settle(world: &World, t: &mut Track, now: Tick, other: EntityId, what: &str) {
    if other == t.id || other == EntityId::NONE || !world.has::<Identity>(other) {
        return;
    }
    let asleep = activity(world, t.id) == "sleep";
    let stat = world.comp::<Brain>(other).is_some_and(|b| b.lod == Lod::Statistical);
    let apart = !stat && !colocated(world, t.id, other);
    let d = t.cam.days.entry(now / TICKS_PER_DAY).or_default();
    d.settles += 1;
    d.settles_apart += u32::from(apart);
    d.settles_stat += u32::from(stat);
    d.settles_asleep += u32::from(asleep);
    if apart || stat || asleep {
        let where_ = if stat {
            "Statistical (off screen)".to_string()
        } else if apart {
            format!("at {}", place(world, other))
        } else {
            "beside me".to_string()
        };
        let me = if asleep { format!("; I am asleep at {}", place(world, t.id)) } else { String::new() };
        // The pre-view pass phase 4 (P3): an encounter with an off-screen
        // party, the pick awake, is narrated `[off]`; a body elsewhere or the
        // pick asleep is the bug (SETTLED APART).
        let tag = if stat && !asleep { "[off]" } else { "SETTLED APART:" };
        t.log(now, "camera", format!("{tag} {what}; {} {where_}{me}", nm(world, other)));
    }
}

/// A `PlanAborted` text's reason: the `FailReason` of a failed step, else
/// how the plan ended (an interrupt, unplannable, a LOD demotion).
fn abort_reason(text: &str) -> String {
    if let Some(i) = text.find(" failed at ") {
        if let Some(j) = text[i..].find(": ") {
            let r: String = text[i + j + 2..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if !r.is_empty() {
                return r;
            }
        }
        return "Failed".into();
    }
    if text.contains("unplannable") {
        "Unplannable".into()
    } else if text.contains("LodDemotion") {
        "LodDemotion".into()
    } else if text.contains(" -> ") {
        "Interrupted".into()
    } else {
        "Other".into()
    }
}

/// A flow's name in the diary: the sim's `Flow`, but the school fee (a
/// `Flow::Contract` of `[ages] school_fee` from a person to a School's
/// owner or the city) is `SchoolFee` (V5 bug 20).
fn flow_label(
    world: &World,
    flow: citysim::systems::ownership::Flow,
    from: Option<EntityId>,
    to: Option<EntityId>,
    coins: i64,
) -> String {
    use citysim::systems::ownership::Flow as F;
    let school_owner =
        |o: Option<EntityId>| world.buildings_of_kind(BuildingKind::School).iter().any(|&b| world.owner_of(b) == o);
    if flow == F::Contract
        && coins == world.config.ages.school_fee
        && from.is_some_and(|f| world.has::<Identity>(f))
        && school_owner(to)
    {
        return "SchoolFee".into();
    }
    format!("{flow:?}")
}

/// The `[[serves]]` row `id` serves by, as `service::row_of_agent` (private
/// there) reads it: the Job's row, else a postless pitch owner's trade row;
/// a gang post (the pimp, the dealer) by its key when neither.
fn serve_row(world: &World, id: EntityId) -> Option<&citysim::config::ServesCfg> {
    let by_key = |k: &str| world.config.serves.iter().find(|r| r.key == k);
    let row = match world.comp::<Job>(id) {
        Some(j) => citysim::systems::service::row_for(world, j.role),
        None => world.jobs2.pitches.get(&id).and_then(|p| world.config.trade(p.trade)).and_then(|t| by_key(&t.id)),
    };
    row.or_else(|| {
        if citysim::systems::life::exec_corp(world, id).is_some() {
            by_key("exec")
        } else if world.jobs2.vice.pimps.values().any(|&p| p == id) {
            by_key("pimp")
        } else if world.dealers.values().flatten().any(|&d| d == id) {
            by_key("dealer")
        } else {
            None
        }
    })
}

/// V5 bug 20: a `Served` memory as its kind and side. The memory carries
/// neither, so the server is read from the regulars ledger (the encounter
/// stamps the server's row for the served today), the pimp from his
/// district's street pitch, else from the pair's rows (the door, the
/// Camp's or Jail's count, a corp's table: the encounters with no ledger;
/// a super's stop, whose Block can overflow the ledger's cap in one tick).
/// The Camp's and the Jail's count is never "a regular".
fn served_text(world: &World, holder: EntityId, e: &MemoryEntry) -> String {
    use citysim::components::ServeKind as K;
    let Some(o) = e.subject else { return format!("served someone unnamed (valence {:+.2})", e.valence) };
    let today = u16::try_from(world.day()).unwrap_or(u16::MAX);
    let fresh = |server: EntityId, guest: EntityId| {
        citysim::systems::service::regulars_of(world, server).iter().any(|r| r.who == guest && r.last_day == today)
    };
    let is_pimp = |p: EntityId| world.jobs2.vice.pimps.values().any(|&x| x == p);
    let pitch = |w: EntityId| world.jobs2.pitches.contains_key(&w);
    let is_exec =
        |x: EntityId| world.corps().into_iter().any(|c| world.comp::<Corp>(c).and_then(|k| k.exec) == Some(x));
    let row_kind = |x: EntityId| serve_row(world, x).map(|r| (r.kind, r.key.clone()));
    let unled = |x: EntityId| row_kind(x).is_some_and(|(k, _)| matches!(k, K::Door | K::Inmate | K::Tenant));
    // (the server, the row's kind and key)
    let side: Option<(EntityId, K, String)> = if is_pimp(holder) && pitch(o) {
        Some((holder, K::Staff, "pimp".into()))
    } else if is_pimp(o) && pitch(holder) {
        Some((o, K::Staff, "pimp".into()))
    } else if fresh(holder, o) {
        row_kind(holder).map(|(k, key)| (holder, k, key))
    } else if fresh(o, holder) {
        row_kind(o).map(|(k, key)| (o, k, key))
    } else if is_exec(o) {
        Some((o, K::Attendee, "exec".into()))
    } else if unled(o) {
        row_kind(o).map(|(k, key)| (o, k, key))
    } else if unled(holder) {
        row_kind(holder).map(|(k, key)| (holder, k, key))
    } else if let Some((K::Pupil, key)) = row_kind(o) {
        // The pressure pass S6 (OPEN-224): the teacher met at the door by a
        // parent with a row of his own (no ledger on the parent's side).
        Some((o, K::Pupil, key))
    } else if let Some((K::Pupil, key)) = row_kind(holder) {
        Some((holder, K::Pupil, key))
    } else {
        match (row_kind(holder), row_kind(o)) {
            (Some((k, key)), None) => Some((holder, k, key)),
            (None, Some((k, key))) => Some((o, k, key)),
            _ => None,
        }
    };
    let who = nm(world, o);
    let Some((server, kind, key)) = side else {
        let rk = |x: EntityId| serve_row(world, x).map_or("none".to_string(), |r| r.key.clone());
        return format!(
            "served-pair with {who} (kind and side unknown: my row {}, theirs {}) (valence {:+.2})",
            rk(holder),
            rk(o),
            e.valence
        );
    };
    let mine = server == holder;
    let text = match (kind, mine) {
        (K::Visit, true) => format!("served {who} (a customer, at the {key}'s counter)"),
        (K::Visit, false) => format!("was served by {who} (as a customer; {who} the {key})"),
        (K::Patient, true) => format!("treated {who} (a patient)"),
        (K::Patient, false) => format!("was treated by {who} (as a patient; the {key})"),
        // The pressure pass S6 (OPEN-224): V11's "a pupil's parent" was the
        // pupil (an infant on the roll). A pupil now writes no memory; the
        // parent is named with the child on today's roll here.
        (K::Pupil, true) if world.has::<Child>(o) => format!("taught {who} (a pupil)"),
        (K::Pupil, true) => {
            let school = world.comp::<Job>(holder).and_then(|j| j.employer);
            let kid = citysim::systems::demography::children_of_agent(world, o).into_iter().find(|&c| {
                school.is_some()
                    && citysim::systems::ages::place_of(world, c) == school.map(citysim::systems::ages::Place::School)
            });
            match kid {
                Some(c) => format!("met {who} at the door (the parent of {}, a pupil)", nm(world, c)),
                None => format!("met {who} at the door (a parent; no child of theirs on today's roll)"),
            }
        }
        (K::Pupil, false) => format!("met {who} at the door (my child's teacher)"),
        (K::Inmate, true) if key == "camp_warden" => format!("counted {who} (the Camp's headcount)"),
        (K::Inmate, true) => format!("fed and counted {who} (an inmate)"),
        (K::Inmate, false) if pitch(holder) || world.has::<Sentence>(holder) => {
            format!("was counted by {who} (as an inmate; the {key})")
        }
        (K::Inmate, false) => format!("visited the Camp or the cells, {who} on duty (the {key})"),
        (K::Tenant, true) => format!("called on {who} (a tenant, on the round)"),
        (K::Tenant, false) => format!("was called on by {who} (as a tenant; my super)"),
        (K::Fare, true) => format!("took {who} as a fare (the {key})"),
        (K::Fare, false) => format!("rode with {who} (as a fare; the {key})"),
        (K::Parcel, true) => format!("handed a parcel to {who} (the recipient)"),
        (K::Parcel, false) => format!("took a parcel from {who} (the courier)"),
        (K::Client, true) => format!("served {who} (a client, the {key})"),
        (K::Client, false) => format!("was served by {who} (as a client; the {key})"),
        (K::Door, true) => format!("kept the door on {who} (a patron)"),
        (K::Door, false) => format!("met {who} at the door (the bouncer)"),
        (K::Staff, true) if key == "pimp" => format!("collected from {who} (my street worker)"),
        (K::Staff, false) if key == "pimp" => format!("was collected by {who} (my pimp)"),
        (K::Staff, true) => format!("oversaw {who} (a co-worker, the house's staff)"),
        (K::Staff, false) => format!("was overseen by {who} (a co-worker, the {key})"),
        (K::Buyer, true) => format!("sold to {who} (a buyer, the {key})"),
        (K::Buyer, false) => format!("bought from {who} (as a buyer; the {key})"),
        (K::Attendee, true) if key == "exec" => format!("met {who} at the table (an attendee)"),
        (K::Attendee, false) if key == "exec" => format!("sat at {who}'s table (a Meeting)"),
        (K::Attendee, true) => format!("fought before {who} (a bettor)"),
        (K::Attendee, false) => format!("watched {who} fight (as a bettor)"),
        (K::Source, true) => format!("interviewed {who} (a source)"),
        (K::Source, false) => format!("was interviewed by {who} (as a source; the reporter)"),
        (K::Crate, _) => format!("loaded crates with {who}"),
        (K::Broker, true) => format!("brokered for {who}"),
        (K::Broker, false) => format!("was brokered by {who}"),
    };
    let regular = kind != K::Inmate
        && citysim::systems::service::regulars_of(world, server)
            .iter()
            .any(|r| r.who == if mine { o } else { holder } && r.visits >= 2);
    format!("{text}{} (valence {:+.2})", if regular { ", a regular" } else { "" }, e.valence)
}

/// A courier pick's parcels handed over this tick: those of the yard gone
/// from the pending list while the pick stands in their stop (V5 bug 20).
fn is_courier(world: &World, id: EntityId) -> bool {
    world
        .comp::<Job>(id)
        .and_then(|j| citysim::systems::service::row_for(world, j.role))
        .is_some_and(|r| r.kind == citysim::components::ServeKind::Parcel)
}

fn cam_parcels(world: &World, t: &mut Track, now: Tick) {
    let id = t.id;
    let job = world.comp::<Job>(id);
    if !is_courier(world, id) {
        t.cam.parcels.clear();
        return;
    }
    let yard = job.and_then(|j| j.employer);
    let cur: Vec<citysim::systems::streetwork::Parcel> =
        world.jobs2.parcels.iter().copied().filter(|p| Some(p.yard) == yard).collect();
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let handed: Vec<citysim::systems::streetwork::Parcel> =
        t.cam.parcels.iter().copied().filter(|p| !cur.contains(p) && here == Some(p.at)).collect();
    for p in handed {
        t.cam.parcels_handed += 1;
        let line = format!(
            "PARCEL handed over at {}: from {} for {} (fee {})",
            place_of_building(world, p.at),
            nm(world, p.from),
            nm(world, p.to),
            money(p.fee)
        );
        t.log(now, "parcel", line);
    }
    t.cam.parcels = cur;
}

fn hours(m: u32) -> String {
    format!("{:.1}", f64::from(m) / 60.0)
}

/// The diary's camera block (Shadow V6), after the header.
fn camera_block(t: &Track) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "## Camera (Shadow V6)\n");
    let _ = writeln!(
        md,
        "Hours by place; plans started and aborted; jumps (a tile change beyond the walk/ride allowance; body: Full and \
         free at both ends) and the largest; the longest unchanged stretch (tile, building, step, exec) begun that day, \
         all and awake; settlements naming a person (a flow or a Served memory: apart = the counterpart a body \
         elsewhere, stat = Statistical, asleep = the pick asleep).\n"
    );
    let _ = writeln!(
        md,
        "| day | {} | plans | aborts | jumps (body) | max jump | still | awake still | settles (apart/stat/asleep) |",
        CAM_PLACES.join(" | ")
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut notes: Vec<String> = Vec::new();
    for (day, d) in &t.cam.days {
        let _ = write!(md, "| d{day} |");
        for m in d.place_min {
            let _ = write!(md, " {} |", hours(m));
        }
        let aborts: u32 = d.aborts.values().sum();
        let still = d.still.as_ref().map_or("-".into(), |s| format!("{}m {}", s.0, s.2));
        let awake = d.still_awake.as_ref().map_or("-".into(), |s| format!("{}m {}", s.0, s.2));
        let _ = writeln!(
            md,
            " {} | {aborts} | {} ({}) | {} | {still} | {awake} | {} ({}/{}/{}) |",
            d.plans, d.jumps, d.jumps_body, d.max_jump, d.settles, d.settles_apart, d.settles_stat, d.settles_asleep
        );
        let reasons: Vec<String> = d.aborts.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let mut n = format!("d{day}: aborts {}", if reasons.is_empty() { "none".into() } else { reasons.join(", ") });
        if let Some(s) = &d.still_awake {
            let _ = write!(n, "; awake still from {} at {}", clock(s.1), s.3);
        }
        if let Some(j) = d.first_jump {
            let _ = write!(n, "; first jump {}", clock(j));
        }
        // Shadow V7: the shift's shape and the long legs.
        if d.work_run_max > 0 {
            let _ = write!(n, "; longest work action {} min on {} work tile(s)", d.work_run_max, d.work_tiles.len());
        }
        if d.goto_long > 0 {
            let _ = write!(n, "; {} GoTo leg(s) over {CAM_LONG_GOTO_MIN} min", d.goto_long);
        }
        notes.push(n);
    }
    let _ = writeln!(md);
    for n in notes {
        let _ = writeln!(md, "- {n}");
    }
    if t.cam.parcels_handed > 0 {
        let _ = writeln!(md, "- parcels handed over in the window: {}", t.cam.parcels_handed);
    }
    let _ = writeln!(md);
    md
}

/// `camera.tsv`'s header (one row per pick and day).
const CAMERA_TSV_HEADER: &str = "arch\tindex\tname\tday\thome_h\twork_h\tbuilding_h\tstreet_h\tvehicle_h\theld_h\tstat_h\t\
coarse_h\tplans\taborts\taborts_by_reason\tjumps\tjumps_body\tmax_jump\tfirst_jump\tstill_min\tstill_start\tstill_class\t\
awake_still_min\tawake_still_start\tawake_still_class\tawake_still_place\tsettles\tsettles_apart\tsettles_stat\t\
settles_asleep\twork_longest_min\twork_tiles\tgoto_over_120";

fn camera_rows(t: &Track, out: &mut String) {
    for (day, d) in &t.cam.days {
        let _ = write!(out, "{}\t{}\t{}\t{day}", t.arch, t.id.index, t.name);
        for m in d.place_min {
            let _ = write!(out, "\t{}", hours(m));
        }
        let reasons: Vec<String> = d.aborts.iter().map(|(k, n)| format!("{k}:{n}")).collect();
        let st = |s: &Option<(u32, Tick, &'static str, String)>| {
            s.as_ref().map_or("0\t-\t-".to_string(), |s| format!("{}\t{}\t{}", s.0, clock(s.1), s.2))
        };
        let awake_place = d.still_awake.as_ref().map_or("-".to_string(), |s| s.3.replace('\t', " "));
        let _ = writeln!(
            out,
            "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{awake_place}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            d.plans,
            d.aborts.values().sum::<u32>(),
            if reasons.is_empty() { "-".into() } else { reasons.join(",") },
            d.jumps,
            d.jumps_body,
            d.max_jump,
            d.first_jump.map_or("-".into(), clock),
            st(&d.still),
            st(&d.still_awake),
            d.settles,
            d.settles_apart,
            d.settles_stat,
            d.settles_asleep,
            d.work_run_max,
            d.work_tiles.len(),
            d.goto_long
        );
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// M16a 5.3: the agent's contracts (bought, placed, held or worked, records
/// still on the books: closed ones stay `closed_keep_days`), its regular
/// stamps at the Fixers and the heat of any Fixer it owns, at the run's end.
fn contracts_section(world: &World, id: EntityId, md: &mut String) {
    use citysim::contract::Broker;
    let _ = writeln!(md, "\n### Contracts (M16a)\n");
    let mine: Vec<&citysim::contract::Contract> = world
        .contracts
        .values()
        .filter(|c| c.buyer == Some(id) || c.agent == Some(id) || c.taker == Some(id) || c.crew.contains(&id))
        .collect();
    if mine.is_empty() {
        let _ = writeln!(md, "- records: none");
    }
    for c in mine {
        let role = if c.taker == Some(id) {
            "taker"
        } else if c.crew.contains(&id) {
            "crew"
        } else {
            "buyer"
        };
        let _ = writeln!(
            md,
            "- #{} {} on {} for {} ({role}, {:?}, {:?}, posted {}{}{})",
            c.id,
            c.kind.label(),
            nm(world, c.target.id()),
            money(c.price),
            c.status,
            c.render,
            clock(c.posted),
            c.broker.map(|b| format!(" at {}", bname(world, b))).unwrap_or_default(),
            // The Hit market: an organic record's motive and deposit.
            if c.motive.is_none() {
                String::new()
            } else if c.deposit > 0 {
                format!(", {}, {} down", c.motive.label(), money(c.deposit))
            } else {
                format!(", {}", c.motive.label())
            }
        );
    }
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    let stamps: Vec<String> = fixers
        .iter()
        .filter_map(|&f| world.comp::<Broker>(f).and_then(|k| k.regulars.get(&id)).map(|&t| (f, t)))
        .map(|(f, t)| format!("{} (last {})", bname(world, f), clock(t)))
        .collect();
    let _ = writeln!(md, "- regular at: {}", if stamps.is_empty() { "none".into() } else { stamps.join(", ") });
    for f in fixers {
        if world.comp::<Building>(f).is_some_and(|b| b.owner == Some(id)) {
            if let Some(k) = world.comp::<Broker>(f) {
                let _ = writeln!(
                    md,
                    "- owns {}: heat {:.2}, book {}, regulars {}, closed until {}",
                    bname(world, f),
                    k.heat,
                    k.book.len(),
                    k.regulars.len(),
                    k.closed_until.map_or("-".into(), clock)
                );
            }
        }
    }
}

fn write_diary(world: &World, t: &mut Track, dir: &std::path::Path, end: Tick) -> Result<(u64, u64), String> {
    t.entries.sort_by_key(|e| (e.tick, e.seq));
    let stem = format!("{}_{}", t.arch, t.id.index);
    let mut md = t.header.clone();
    md.push('\n');
    md.push_str(&camera_block(t));
    let mut jl = String::new();
    let mut day_open: Option<u64> = None;
    // Summary lines print last in their day.
    let mut by_day: BTreeMap<u64, Vec<&Entry>> = BTreeMap::new();
    for e in &t.entries {
        by_day.entry(e.tick / TICKS_PER_DAY).or_default().push(e);
    }
    for (day, es) in &by_day {
        let _ = day_open.insert(*day);
        let _ = writeln!(md, "## Day {day}\n");
        let (sum, rest): (Vec<&&Entry>, Vec<&&Entry>) = es.iter().partition(|e| e.cat == "summary");
        for e in rest.iter().chain(sum.iter()) {
            let line = match e.cat {
                "summary" => format!("**{}**", e.text),
                _ => format!("- `{}` [{}] {}", clock(e.tick), e.cat, e.text),
            };
            let _ = writeln!(md, "{line}");
            let _ = writeln!(
                jl,
                "{{\"tick\":{},\"day\":{},\"time\":{},\"cat\":{},\"text\":{}}}",
                e.tick,
                e.tick / TICKS_PER_DAY,
                json_str(&hm(e.tick)),
                json_str(e.cat),
                json_str(&e.text)
            );
        }
        let _ = writeln!(md);
    }
    // Whole-run summary.
    let _ = writeln!(md, "## Whole-run summary\n");
    if let Some(d) = t.died {
        let _ = writeln!(md, "**Died at {}.** The diary stops there.\n", clock(d));
    } else if !world.has::<Brain>(t.id) && !world.has::<Child>(t.id) {
        let _ = writeln!(md, "(No longer a living agent at the end of the run.)\n");
    }
    let _ = writeln!(md, "### Time use (hours)\n");
    let _ = write!(md, "| day |");
    for c in CLASSES {
        let _ = write!(md, " {c} |");
    }
    let _ = writeln!(md);
    let _ = write!(md, "|---|");
    for _ in CLASSES {
        let _ = write!(md, "---|");
    }
    let _ = writeln!(md);
    for (d, row) in &t.use_by_day {
        let _ = write!(md, "| d{d} |");
        for c in CLASSES {
            let _ = write!(md, " {:.1} |", f64::from(row.get(c).copied().unwrap_or(0)) / 60.0);
        }
        let _ = writeln!(md);
    }
    let _ = write!(md, "| **total** |");
    for c in CLASSES {
        let _ = write!(md, " {:.1} |", f64::from(t.total_use.get(c).copied().unwrap_or(0)) / 60.0);
    }
    let _ = writeln!(md, "\n");
    let _ = writeln!(md, "### Top places (minutes)\n");
    let mut pl: Vec<(&String, &u32)> = t.total_places.iter().collect();
    pl.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (p, m) in pl.into_iter().take(10) {
        let _ = writeln!(md, "- {p}: {m}");
    }
    let _ = writeln!(md, "\n### Top contacts (interactions)\n");
    let mut cs: Vec<&(String, u32)> = t.contacts.values().collect();
    cs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if cs.is_empty() {
        let _ = writeln!(md, "- none recorded");
    }
    for (n, c) in cs.into_iter().take(10) {
        let _ = writeln!(md, "- {n}: {c}");
    }
    let _ = writeln!(md, "\n### Money\n");
    let _ = writeln!(md, "- net {}{}", if t.total_net > 0 { "+" } else { "" }, money(t.total_net));
    for (k, (sum, n)) in &t.flows {
        let _ = writeln!(md, "- {k}: {}{} over {n} flow(s)", if *sum > 0 { "+" } else { "" }, money(*sum));
    }
    let _ = writeln!(md, "\n### By flow kind (the sim's own labels; out / in / count)\n");
    if t.flow_kinds.is_empty() {
        let _ = writeln!(md, "- none recorded");
    }
    for (k, (o, i, n)) in &t.flow_kinds {
        let _ = writeln!(md, "- {k}: out {}, in {}, x{n}", money(*o), money(*i));
    }
    let out_of = |names: &[&str]| -> i64 { names.iter().filter_map(|k| t.flow_kinds.get(*k)).map(|v| v.0).sum() };
    let in_of = |names: &[&str]| -> i64 { names.iter().filter_map(|k| t.flow_kinds.get(*k)).map(|v| v.1).sum() };
    let _ = writeln!(md, "\n### Leisure and the street (L2)\n");
    let _ = writeln!(
        md,
        "- fun activities (Enjoy, Gamble, EatOut, HangOut): {:.1} h",
        f64::from(t.total_use.get("leisure").copied().unwrap_or(0)) / 60.0
    );
    let _ = writeln!(
        md,
        "- coins spent: leisure {} (venues {}, gambling {} less {} won back), drink {}, food {}, rent {}",
        // Street dice are gambling too: stakes lost out, winnings back.
        money(out_of(&["Leisure", "Gamble", "StreetDice"]) - in_of(&["Leisure", "Gamble", "GambleWin", "StreetDice"])),
        money(out_of(&["Leisure"]) - in_of(&["Leisure"])),
        money(out_of(&["Gamble", "StreetDice"]) - in_of(&["Gamble"])),
        money(in_of(&["GambleWin", "StreetDice"])),
        money(out_of(&["Drink"])),
        money(out_of(&["Food"]) - in_of(&["Food"])),
        money(out_of(&["Rent"]))
    );
    let _ = writeln!(md, "- HangOuts: {}, distinct known contacts met there: {}", t.hangouts, t.hang_known.len());
    let _ = writeln!(md, "- tribute received (Flow::Tribute): {}", money(t.tribute_in));
    let _ = writeln!(md, "- gossip told about me: {} telling(s)", t.gossip_about);
    contracts_section(world, t.id, &mut md);
    if t.held_settles > 0 || t.stat_minutes > 0 || !t.stat_notes.is_empty() {
        let _ = writeln!(md, "\n### Statistical stretches (what the stand-in did)\n");
        let _ = writeln!(
            md,
            "- {:.1} h Statistical; {} held-cell settlement(s) covering {:.1} h of jailed decay",
            f64::from(t.stat_minutes) / 60.0,
            t.held_settles,
            t.held_minutes as f64 / 60.0
        );
        for (tk, text) in t.stat_notes.iter().take(40) {
            let _ = writeln!(md, "- `{}` {text}", clock(*tk));
        }
        if t.stat_notes.len() > 40 {
            let _ = writeln!(md, "- ... and {} more", t.stat_notes.len() - 40);
        }
    }
    let _ = writeln!(md, "\n### Notable events\n");
    let kinds: Vec<String> = t.event_kinds.iter().map(|(k, n)| format!("{k} x{n}")).collect();
    let _ = writeln!(md, "- counts: {}", if kinds.is_empty() { "none".into() } else { kinds.join(", ") });
    for (tk, text) in t.notable.iter().take(25) {
        let _ = writeln!(md, "- `{}` {}", clock(*tk), text);
    }
    if t.notable.len() > 25 {
        let _ = writeln!(md, "- ... and {} more", t.notable.len() - 25);
    }
    let _ = writeln!(md, "\n(run ended {})", clock(end));
    let mdp = dir.join(format!("{stem}.md"));
    let jp = dir.join(format!("{stem}.jsonl"));
    std::fs::write(&mdp, &md).map_err(|e| format!("{}: {e}", mdp.display()))?;
    std::fs::write(&jp, &jl).map_err(|e| format!("{}: {e}", jp.display()))?;
    Ok((md.len() as u64, jl.len() as u64))
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
    let mut world = start_world(args.seed, args.start_day, args.population, args.map.as_deref())?;

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
    let mut tracks = follow(&mut world, &picked, args.days, !args.no_pin, true);
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
    Ok(())
}

/// The world of `seed` run normally to the start of `start_day`. Shadow V6:
/// `map` overrides `[world] map` as `run --map` does (with `population`,
/// the scaled config on that map instead of the generated one).
fn start_world(
    seed: u64,
    start_day: u64,
    population: Option<u32>,
    map: Option<&std::path::Path>,
) -> Result<World, String> {
    let mut config = match population {
        Some(n) => {
            let mut c = Config::load().scaled_to(n);
            c.world.map = format!("gen:{n}");
            c.ambient.flats_per_slot = 50.0;
            c
        }
        None => Config::load(),
    };
    if let Some(m) = map {
        config.world.map = crate::map_path(m)?;
    }
    let mut world = World::new(seed, config);
    let start_tick = start_day * TICKS_PER_DAY;
    while world.tick < start_tick {
        citysim::tick(&mut world);
    }
    Ok(world)
}

/// The picks: `count` per archetype of the comma-separated `list` (free
/// agents first, shuffled by the seed; the CEO list by treasury), then every
/// live `--agent` index.
fn pick(
    world: &World,
    seed: u64,
    start_day: u64,
    list: Option<&str>,
    count: usize,
    agents: &[u32],
) -> Vec<(String, EntityId)> {
    let mut picked: Vec<(String, EntityId)> = Vec::new();
    let mut taken: BTreeSet<EntityId> = BTreeSet::new();
    if let Some(list) = list {
        for tok in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            // Shadow V7 (tool): `arch:N` picks N of this archetype instead of `--count`.
            let (arch, count) = match tok.split_once(':') {
                Some((a, n)) => (a, n.parse::<usize>().unwrap_or(count)),
                None => (tok, count),
            };
            match candidates(world, arch) {
                Err(e) => eprintln!("note: {e}"),
                Ok(c) if c.is_empty() => eprintln!("note: no {arch} candidates on seed {seed} at day {start_day}"),
                Ok(_) if arch == "child" => {
                    eprintln!("note: child is unpinnable (no Brain, no Full LOD); not picked (see --list)")
                }
                Ok(c) => {
                    if arch == "worker_friday" {
                        let friday = (world.config.leisure.collect_weekday + 6) % 7;
                        if start_day % 7 != friday {
                            eprintln!(
                                "note: worker_friday: day {start_day} is weekday {}, not the Friday (weekday {friday}); the diary does not start on the Friday",
                                start_day % 7
                            );
                        }
                    }
                    let ordered = if arch == "ceo" {
                        c
                    } else if arch == "owner_broke" {
                        let by = citysim::systems::underwater::owners_by_till(world);
                        eprintln!(
                            "note: owner_broke: {} owners with staff; the lowest till {:.2} weeks of wages",
                            by.len(),
                            by.first().map_or(0.0, |x| x.0)
                        );
                        by.into_iter().map(|(_, o)| o).filter(|o| c.contains(o)).collect()
                    } else if arch == "miserable" {
                        let mood = |a: &EntityId| world.comp::<citysim::components::Mood>(*a).map_or(0.0, |m| m.value);
                        let mut c = c;
                        c.sort_by(|a, b| mood(a).total_cmp(&mood(b)).then(a.cmp(b)));
                        let adults = world.citizens().into_iter().filter(|&a| adult_alive(world, a)).count();
                        eprintln!(
                            "note: miserable: {} of {adults} adults under the misery line; the lowest mood {:+.2}",
                            c.len(),
                            c.first().map_or(0.0, mood)
                        );
                        c
                    } else {
                        shuffled(c, seed, arch)
                    };
                    // L1 (the V1 tool note): free agents first; a jailed pick's
                    // week is the cells (V1's dealer and leader were inside).
                    let (free, jailed): (Vec<EntityId>, Vec<EntityId>) =
                        ordered.into_iter().partition(|&id| is_free(world, id));
                    let ordered: Vec<EntityId> = free.into_iter().chain(jailed).collect();
                    let mut n = 0;
                    for id in ordered {
                        if n >= count {
                            break;
                        }
                        if taken.insert(id) {
                            picked.push((arch.to_string(), id));
                            n += 1;
                        }
                    }
                    if n < count {
                        eprintln!("note: only {n} distinct {arch} candidate(s) (asked for {count})");
                    }
                }
            }
        }
    }
    for &ix in agents {
        match world.citizens().into_iter().find(|c| c.index == ix) {
            Some(id) if world.has::<Identity>(id) && !world.has::<Corpse>(id) => {
                if taken.insert(id) {
                    picked.push(("agent".into(), id));
                }
            }
            _ => eprintln!("note: agent {ix} is not alive at day {start_day}"),
        }
    }
    picked
}

/// Follow the picks for `days` from the world's tick, pinned unless
/// `pin` is false; the tracks come back closed (the last day flushed).
fn follow(world: &mut World, picked: &[(String, EntityId)], days: u64, pin: bool, verbose: bool) -> Vec<Track> {
    let start_tick = world.tick;
    world.shadow_notes = Some(Vec::new());
    let mut tracks: Vec<Track> = Vec::new();
    for (arch, id) in picked {
        let name = world.comp::<Identity>(*id).map_or("?".into(), |i| i.name.clone());
        let mut t = Track::new(*id, arch, name, start_tick);
        t.pinned = pin;
        t.spouse = world.spouse_of(*id);
        t.header = header(world, &t);
        t.last_place = place(world, *id);
        if verbose {
            eprintln!("shadowing {arch}: {} (#{})", t.name, id.index);
        }
        tracks.push(t);
    }
    let end_tick = start_tick + days * TICKS_PER_DAY;
    while world.tick < end_tick {
        if pin {
            for t in &tracks {
                if let Some(b) = world.comp_mut::<Brain>(t.id) {
                    b.pinned = true;
                }
            }
        }
        let cursor = world.next_event_id;
        if let Some(n) = world.shadow_notes.as_mut() {
            n.clear();
        }
        citysim::tick(world);
        let now = world.tick - 1;
        let mut events: Vec<&Event> = world.events.iter().rev().take_while(|e| e.id >= cursor).collect();
        events.reverse();
        let dbg: Vec<&Event> = world.debug_events.iter().rev().take_while(|e| e.tick == now).collect();
        let notes: Vec<ShadowNote> = world.shadow_notes.clone().unwrap_or_default();
        for t in &mut tracks {
            observe(world, t, now, &events, &dbg, &notes);
        }
    }
    let end = world.tick;
    for t in &mut tracks {
        // The last day's summary and any open action.
        close_action(world, t, end, "run ended");
        flush_day(world, t, end);
        end_sleep_block(t, end);
        close_still(t, end);
    }
    tracks
}

// ---------------------------------------------------------------------------
// The behaviour tier (`shadow --assert`; docs/TESTING.md)
// ---------------------------------------------------------------------------

/// The behaviour tier's windows: 7 days from day 19 (a Friday, so
/// `worker_friday` starts on its Friday) with every archetype, and from day
/// 90 (the gangs full) with the gang archetypes: the V2 shadow pass's
/// pinned windows (docs/history/SHADOW_V2.md), on each of `BEHAVIOUR_SEEDS`.
///
/// Each archetype is measured in its own clone of the window's world, with
/// up to `BEHAVIOUR_SAMPLE` of its free candidates pinned (every one in a
/// small pool), and a row pools the three seeds: it reads the archetype,
/// not five people on one city, and another archetype's pins never move
/// it. The first `BEHAVIOUR_COUNT` of each sample are the diaries (`--out`),
/// the human-readable judge.
///
/// The diary seed (`--out` writes its diaries under `d<day>`, the others under `s<seed>_d<day>`).
pub const BEHAVIOUR_SEED: u64 = 42;
/// The seeds pooled into every row: a gang's trajectory or the Law's posture on one seed is one
/// sample of the city, not the archetype.
const BEHAVIOUR_SEEDS: [u64; 3] = [BEHAVIOUR_SEED, 43, 44];
const BEHAVIOUR_DAYS: u64 = 7;
const BEHAVIOUR_COUNT: usize = 5;
/// Agents measured per archetype and window: under `[lod] max_full` (50), so
/// every pinned agent holds a Full slot.
const BEHAVIOUR_SAMPLE: usize = 30;
/// The share of agents dropped at each end before a per-agent metric is
/// averaged (a trimmed mean): one agent's 6 h commute or week at energy 0
/// does not carry a row; a regression shared by most of them does.
const BEHAVIOUR_TRIM: f64 = 0.1;
/// A share (paid of worked, waged workdays, bouts attended) is read over at
/// least this many shifts or bouts, else it is a SKIP: a share of five shifts
/// is one person's week.
const BEHAVIOUR_MIN_SHARE: u32 = 20;
/// A row is read over at least this many agents (the seeds pooled), else it
/// is a SKIP: never a smaller sample than the 5-pick tier judged.
const BEHAVIOUR_MIN_AGENTS: usize = 5;
const BEHAVIOUR_WINDOWS: [(u64, &str); 2] = [
    (
        19,
        "gang_member,gang_leader,ripperdoc,homeless,ceo,guard,worker,runner,purist,dealer,reporter,cook,club_staff,fighter,fabber,sweeper,worker_friday,super,bouncer,dancer,night_shift,orderly,camp_warden",
    ),
    (90, "gang_member,gang_leader,dealer,runner,purist,homeless"),
];

/// One agent's numbers in one window, or (merged) an archetype's (sums, then rates).
#[derive(Clone, Default)]
struct Agg {
    agents: u32,
    alive_min: u64,
    travel_min: u64,
    sleep_min: u64,
    work_min: u64,
    energy0_min: u64,
    starve_min: u64,
    meals: u32,
    meals_synth: u32,
    meals_dive: u32,
    /// The longest sleep block of each agent-night (noon to noon), minutes.
    nights: Vec<u32>,
    /// Whole workday shifts while free; of those worked (half the shift at work-class
    /// steps); paid (a wage by two hours after the end); worked and paid.
    shifts: u32,
    worked: u32,
    paid: u32,
    worked_paid: u32,
    hangouts: u32,
    known_met: u32,
    refunds: u32,
    commute_aborts: u32,
    bouts: u32,
    bouts_present: u32,
    // Lives at work phase 0 (`--lives`): read by `lives_report` only.
    idle_min: u64,
    /// Married picks (a spouse at the window's start), their alive minutes,
    /// and those that never met the spouse by each signal.
    married: u32,
    married_min: u64,
    never_named: u32,
    never_coloc: u32,
    spouse_named_hours: u32,
    spouse_samples: u32,
    spouse_coloc: u32,
    spouse_coloc_stat: u32,
    hang_empty: u32,
    /// The perf pass phase 1 (OPEN-195): idle Rests cut within `SHORT_REST_MIN`.
    short_rests: u32,
    /// Whole free workday shifts at a building; Served-able by a body or an
    /// event; by any of the three signals; the hourly samples and the
    /// non-staff bodies summed; the venue tally; the source events by kind.
    served_shifts: u32,
    servedable: u32,
    servedable_any: u32,
    shift_samples: u64,
    nonstaff: u64,
    venue_visits: u64,
    /// Phase 1: shifts with an encounter, and the encounters.
    shifts_met: u32,
    encounters: u64,
    source_kinds: BTreeMap<String, u32>,
    /// Phase 7 (§ 5.3): picks housed at the window's end, and of them in the
    /// Spire (tier 2).
    housed: u32,
    spire: u32,
}

impl Agg {
    fn add(&mut self, t: &Track) {
        self.add_lives(t);
        self.agents += 1;
        self.alive_min += u64::from(t.alive_min);
        self.travel_min += u64::from(t.total_use.get("travel").copied().unwrap_or(0));
        self.sleep_min += u64::from(t.total_use.get("sleep").copied().unwrap_or(0));
        self.work_min += u64::from(t.work_min);
        self.energy0_min += u64::from(t.energy0_min);
        self.starve_min += u64::from(t.starve_min);
        self.meals += t.meals;
        self.meals_synth += t.meals_synth;
        self.meals_dive += t.meals_dive;
        let mut by_night: BTreeMap<u64, u32> = BTreeMap::new();
        for &(end, len) in &t.sleep_blocks {
            let night = end.saturating_sub(TICKS_PER_DAY / 2) / TICKS_PER_DAY;
            let e = by_night.entry(night).or_insert(0);
            *e = (*e).max(len);
        }
        self.nights.extend(by_night.values());
        for s in t.shifts.iter().filter(|s| s.whole && s.workday && s.free && !s.office && !s.own_post) {
            let worked = s.work * 2 >= s.len;
            let paid = t.wages.iter().any(|&w| w >= s.start && w <= s.end + 2 * TICKS_PER_HOUR);
            self.shifts += 1;
            self.worked += u32::from(worked);
            self.paid += u32::from(paid);
            self.worked_paid += u32::from(worked && paid);
        }
        self.hangouts += t.hangouts;
        self.known_met += t.hang_known.len() as u32;
        self.refunds += t.refunds;
        self.commute_aborts += t.commute_aborts;
        self.bouts += t.bouts;
        self.bouts_present += t.bouts_present;
    }

    /// Lives at work phase 0: the `--lives` tallies of one agent.
    fn add_lives(&mut self, t: &Track) {
        self.idle_min += u64::from(t.total_use.get("idle").copied().unwrap_or(0));
        if t.spouse.is_some() {
            self.married += 1;
            self.married_min += u64::from(t.alive_min);
            self.never_named += u32::from(t.spouse_named_hours.is_empty());
            self.never_coloc += u32::from(t.spouse_coloc == 0);
            self.spouse_named_hours += t.spouse_named_hours.len() as u32;
            self.spouse_samples += t.spouse_samples;
            self.spouse_coloc += t.spouse_coloc;
            self.spouse_coloc_stat += t.spouse_coloc_stat;
        }
        self.hang_empty += t.hang_empty;
        self.short_rests += t.short_rests;
        self.housed += u32::from(t.home_tier.is_some());
        self.spire += u32::from(t.home_tier.is_some_and(|x| x >= 2));
        for s in t.shifts.iter().filter(|s| s.whole && s.workday && s.free && s.employer.is_some()) {
            let served = s.nonstaff > 0 || s.source_events > 0;
            self.served_shifts += 1;
            self.servedable += u32::from(served);
            self.servedable_any += u32::from(served || s.venue_visits > 0);
            self.shift_samples += u64::from(s.samples);
            self.nonstaff += u64::from(s.nonstaff);
            self.venue_visits += u64::from(s.venue_visits);
            self.shifts_met += u32::from(s.encounters > 0);
            self.encounters += u64::from(s.encounters);
        }
        for (k, n) in &t.source_kinds {
            *self.source_kinds.entry(k.clone()).or_insert(0) += n;
        }
    }

    /// Pool another agent's sums into these.
    fn merge(&mut self, o: &Agg) {
        self.idle_min += o.idle_min;
        self.housed += o.housed;
        self.spire += o.spire;
        self.married += o.married;
        self.married_min += o.married_min;
        self.never_named += o.never_named;
        self.never_coloc += o.never_coloc;
        self.spouse_named_hours += o.spouse_named_hours;
        self.spouse_samples += o.spouse_samples;
        self.spouse_coloc += o.spouse_coloc;
        self.spouse_coloc_stat += o.spouse_coloc_stat;
        self.hang_empty += o.hang_empty;
        self.short_rests += o.short_rests;
        self.served_shifts += o.served_shifts;
        self.servedable += o.servedable;
        self.servedable_any += o.servedable_any;
        self.shift_samples += o.shift_samples;
        self.nonstaff += o.nonstaff;
        self.venue_visits += o.venue_visits;
        self.shifts_met += o.shifts_met;
        self.encounters += o.encounters;
        for (k, n) in &o.source_kinds {
            *self.source_kinds.entry(k.clone()).or_insert(0) += n;
        }
        self.agents += o.agents;
        self.alive_min += o.alive_min;
        self.travel_min += o.travel_min;
        self.sleep_min += o.sleep_min;
        self.work_min += o.work_min;
        self.energy0_min += o.energy0_min;
        self.starve_min += o.starve_min;
        self.meals += o.meals;
        self.meals_synth += o.meals_synth;
        self.meals_dive += o.meals_dive;
        self.nights.extend_from_slice(&o.nights);
        self.shifts += o.shifts;
        self.worked += o.worked;
        self.paid += o.paid;
        self.worked_paid += o.worked_paid;
        self.hangouts += o.hangouts;
        self.known_met += o.known_met;
        self.refunds += o.refunds;
        self.commute_aborts += o.commute_aborts;
        self.bouts += o.bouts;
        self.bouts_present += o.bouts_present;
    }

    fn agent_days(&self) -> f64 {
        self.alive_min as f64 / TICKS_PER_DAY as f64
    }

    /// The metric's value; `None` when the window gave it nothing to read
    /// (no shifts, no nights, no bouts).
    fn get(&self, m: Metric) -> Option<f64> {
        let days = self.agent_days();
        if days <= 0.0 {
            return None;
        }
        let ratio = |a: u32, b: u32| (b > 0).then(|| f64::from(a) / f64::from(b));
        match m {
            Metric::WalkHDay => Some(self.travel_min as f64 / 60.0 / days),
            Metric::SleepHDay => Some(self.sleep_min as f64 / 60.0 / days),
            Metric::LongestSleepH => (!self.nights.is_empty())
                .then(|| self.nights.iter().map(|&n| f64::from(n)).sum::<f64>() / 60.0 / self.nights.len() as f64),
            Metric::Energy0HWeek => Some(self.energy0_min as f64 / 60.0 / days * 7.0),
            Metric::WorkHDay => Some(self.work_min as f64 / 60.0 / days),
            Metric::ShiftsWorkedShare => ratio(self.worked, self.shifts),
            Metric::PaidOfWorked => ratio(self.worked_paid, self.worked),
            Metric::WagedWorkdays => ratio(self.paid, self.shifts),
            Metric::HangoutsWeek => Some(f64::from(self.hangouts) / days * 7.0),
            Metric::KnownMetWeek => Some(f64::from(self.known_met) / days * 7.0),
            Metric::RefundsWeek => Some(f64::from(self.refunds) / days * 7.0),
            Metric::CommuteAbortsDay => Some(f64::from(self.commute_aborts) / days),
            Metric::BoutsAttended => ratio(self.bouts_present, self.bouts),
            Metric::StarveHWeek => Some(self.starve_min as f64 / 60.0 / days * 7.0),
            Metric::SynthShare => ratio(self.meals_synth, self.meals),
            Metric::DivesWeek => Some(f64::from(self.meals_dive) / days * 7.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Metric {
    /// Travel-class hours a day.
    WalkHDay,
    /// Sleep-class hours a day.
    SleepHDay,
    /// The mean over agent-nights (noon to noon) of the night's longest sleep block, hours.
    LongestSleepH,
    /// Hours at energy 0 per agent-week (free minutes: a pinned prisoner's cell is the pin's artefact).
    Energy0HWeek,
    /// Work-class hours a day.
    WorkHDay,
    /// Whole workday shifts (free) with half the shift at work-class steps.
    ShiftsWorkedShare,
    /// Worked shifts paid a wage by two hours after the shift's end.
    PaidOfWorked,
    /// Whole workday shifts (free) with a wage.
    WagedWorkdays,
    /// HangOut steps per agent-week.
    HangoutsWeek,
    /// Distinct known contacts met at a HangOut per agent-week.
    KnownMetWeek,
    /// Purchases refunded (a started buy undone) per agent-week.
    RefundsWeek,
    /// Work goals dropped before any work-class minute (a commute turned back), per agent-day.
    CommuteAbortsDay,
    /// Bouts with the fighter inside the pit, of the bouts naming her.
    BoutsAttended,
    /// Hunger as sickness (printed; no bound yet): free hours starving
    /// (hunger < `starving_hunger`) per agent-week.
    StarveHWeek,
    /// Synth (and Mission) meals of all meals, pooled (zero with `[sickness]` off).
    SynthShare,
    /// Bin meals per agent-week (zero with `[sickness]` off).
    DivesWeek,
}

impl Metric {
    /// A share of shifts or bouts: pooled over the sample (one agent's two
    /// shifts are no rate). Every other metric is a per-agent rate,
    /// trimmed-mean averaged.
    fn pooled(self) -> bool {
        matches!(
            self,
            Metric::ShiftsWorkedShare
                | Metric::PaidOfWorked
                | Metric::WagedWorkdays
                | Metric::BoutsAttended
                | Metric::SynthShare
        )
    }
}

/// One archetype in one window: each sampled agent's numbers, and their pool.
#[derive(Default)]
struct Row {
    agents: Vec<Agg>,
    pooled: Agg,
}

impl Row {
    fn add(&mut self, t: &Track) {
        let mut a = Agg::default();
        a.add(t);
        self.pooled.merge(&a);
        self.agents.push(a);
    }

    fn extend(&mut self, o: &Row) {
        self.pooled.merge(&o.pooled);
        self.agents.extend(o.agents.iter().cloned());
    }

    /// The row's value of `m`: the pooled share, or the trimmed mean of the
    /// per-agent rates over the agents alive a whole day of the window.
    /// `judged`: `None` under `BEHAVIOUR_MIN_AGENTS` agents or (a share)
    /// `BEHAVIOUR_MIN_SHARE` shifts or bouts; the printed tables read every row.
    fn get(&self, m: Metric, judged: bool) -> Option<f64> {
        let floor = |n: usize, min: usize| !judged || n >= min;
        if m.pooled() {
            let base = match m {
                Metric::PaidOfWorked => self.pooled.worked,
                Metric::BoutsAttended => self.pooled.bouts,
                _ => self.pooled.shifts,
            };
            let enough =
                floor(self.agents.len(), BEHAVIOUR_MIN_AGENTS) && floor(base as usize, BEHAVIOUR_MIN_SHARE as usize);
            return if enough { self.pooled.get(m) } else { None };
        }
        let v: Vec<f64> =
            self.agents.iter().filter(|a| a.alive_min >= TICKS_PER_DAY).filter_map(|a| a.get(m)).collect();
        if !floor(v.len(), BEHAVIOUR_MIN_AGENTS) {
            return None;
        }
        trimmed_mean(v, BEHAVIOUR_TRIM)
    }
}

/// The mean of `v` without its `floor(n x trim)` lowest and highest values.
fn trimmed_mean(mut v: Vec<f64>, trim: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let k = (v.len() as f64 * trim).floor() as usize;
    let kept = &v[k..v.len() - k];
    Some(kept.iter().sum::<f64>() / kept.len() as f64)
}

const METRICS: [Metric; 16] = [
    Metric::WalkHDay,
    Metric::SleepHDay,
    Metric::LongestSleepH,
    Metric::Energy0HWeek,
    Metric::WorkHDay,
    Metric::ShiftsWorkedShare,
    Metric::PaidOfWorked,
    Metric::WagedWorkdays,
    Metric::HangoutsWeek,
    Metric::KnownMetWeek,
    Metric::RefundsWeek,
    Metric::CommuteAbortsDay,
    Metric::BoutsAttended,
    Metric::StarveHWeek,
    Metric::SynthShare,
    Metric::DivesWeek,
];

/// A bound: the value must be at least (`Min`) or at most (`Max`) it.
#[derive(Clone, Copy)]
enum Bound {
    Min(f64),
    Max(f64),
}

/// One behaviour check: (window start day, archetype, metric, bound, and
/// the lowest and highest value the row read on the calibration ensemble (a printed FINDING band, see `COLLAPSE`):
/// main and seven config butterflies, the noise a city change that is not a
/// behaviour change makes). Bounds sit a cushion outside that range: they
/// catch a regression in "acts like a person", they are not targets.
type Check = (u64, &'static str, Metric, Bound, f64, f64);

/// Doctrine 1 (Dylan, 2026-10-22): the tier asserts broken-system signals, it does not police calibration.
/// A `CHECKS` row's own bound and range are the calibration band: printed as a `FINDING`, never failing.
/// Only a metric listed here asserts, against the bound here (where only a broken mechanism lands), and
/// only when the row's pooled n reaches `BEHAVIOUR_ASSERT_MIN_N`. One line of rationale per kind.
const COLLAPSE: &[(Metric, Bound)] = &[
    // Wages: under half of worked shifts (or workdays) paid is a dead pay path, not one broke owner.
    (Metric::PaidOfWorked, Bound::Min(0.5)),
    (Metric::WagedWorkdays, Bound::Min(0.5)),
    // Social: known contacts met at a HangOut near 0 a week is a dead social mechanism.
    (Metric::KnownMetWeek, Bound::Min(0.05)),
    // Travel: 20 h or more a day walking is a stuck commute or a lost plan, not a long day.
    (Metric::WalkHDay, Bound::Max(20.0)),
    // Body: energy at 0 for most of a 168 h week (> 84 h) is a person who never recovers.
    (Metric::Energy0HWeek, Bound::Max(84.0)),
    // Commute: more than one Work goal dropped before any work per agent-day is a broken planner (the CEO's
    // office-hours goal flickers at 0.5-0.9 by design, so the bound sits above it).
    (Metric::CommuteAbortsDay, Bound::Max(1.0)),
    // Core action: the reporter's work hours under half an hour a day, the role's job not happening.
    (Metric::WorkHDay, Bound::Min(0.5)),
    // Absolute, as before: a purchase refund is a bug (<= 1 a week); the fighter is in the pit for 3 bouts in 4.
    (Metric::RefundsWeek, Bound::Max(1.0)),
    (Metric::BoutsAttended, Bound::Min(0.75)),
];

/// A row whose pooled n (agents over the seeds) is under this prints and never asserts: one person's week.
const BEHAVIOUR_ASSERT_MIN_N: usize = 10;

const CHECKS: &[Check] = &[
    // Re-calibrated 2026-10-11 on a6887f1 (after the housing pre-roll near jobs, grocery runs, workplaces capped
    // by room with night crews, vagrancy at work and the Hit market) by the same rules: main and seven
    // butterflies (`stat_violence_mult` 0.98, 0.99, 1.01, 1.02, 1.03, `max_coarse` 149, 151), the shipped
    // values being 1.0 and 150. d19 gang_leader PaidOfWorked/WagedWorkdays (read on 2 and 3 of 8 runs) and
    // d90 gang_leader PaidOfWorked (6 of 8) kept their bound and range; a row read on 7 of 8 was re-derived.
    // Re-derived 2026-10-10 after the flip (wages, no net, sickness, shanties the shipped default) by
    // the same rules: the tier on the new default and the same seven butterflies; the >30 % moves
    // are listed in the flip commit for review. d90 gang_leader PaidOfWorked read on 6 of 8 runs
    // (under 20 shifts on two): its bound and range kept.
    // Calibrated 2026-10-09 on d11e397: the tier run on main and on seven config butterflies
    // (`[lod] stat_violence_mult` 0.98, 0.99, 1.01, 1.02, 1.03, `max_coarse` 149, 151: the same rules,
    // a different week for every pick), the range each row read beside its bound. Cushions outside
    // the range: walking + max(1 h, 15 %), the longest nightly block - 0.5 h, energy 0 + 2 h a week,
    // known contacts met x 0.6, commute aborts + 0.05 a day, paid of worked and waged workdays - 0.1,
    // the reporter's work hours x 0.5; refunds <= 1 a week and the fighter inside the pit for 3 bouts
    // in 4 as before. A Min is rounded down, a Max up (to 0.01 for aborts and shares, else 0.1).
    // Not judged: d19 reporter PaidOfWorked (under 20 worked shifts on every run) and d90 runner
    // (3-10 agents over the three seeds, an archetype defined by a run in the last 7 days: its rows
    // moved more under the butterflies than any injected regression moved a row).
    // Not judged since Lives at work phase 4a (2026-10-18): d19 ripperdoc PaidOfWorked and WagedWorkdays.
    // The 18-21 picks share three clinic owners; the unpaid shifts are all at the two small clinics whose
    // owners run out of coin (Ripperdoc#515, #546), so the pooled share reads one owner's month. Phase 4a
    // read 0.73 / 0.69, its street switch off 0.89 / 0.85, `cab_shifts` off 0.86 / 0.83, the worlds
    // diverging from tick 588; unpaid-wage quits by Ripperdocs and Orderlies over 26 days, seeds 42-50:
    // 64 on, 55 off, 59 cab_shifts off (42-44 on is high, 45-50 on is the lowest). Workers going unpaid
    // by a broke owner is the harsh city, not a broken system (doctrine 1).
    (19, "ceo", Metric::WalkHDay, Bound::Max(5.5), 3.83, 4.42),
    (19, "ceo", Metric::LongestSleepH, Bound::Min(4.3), 4.81, 5.16),
    (19, "ceo", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    // Jobs & lives phase 7b (2026-10-15): this row alone re-derived on the phase-7 city with the
    // regenerated stat table (the eight-run ensemble: main 2.57; stat_violence_mult 0.98 1.32, 0.99
    // 1.42, 1.01 2.21, 1.02 2.50, 1.03 1.83; max_coarse 149 1.59, 151 2.05): 1.32 x 0.6 = 0.79,
    // rounded down. It read 0.77-2.00 across the milestone's phases against the old 1.3 (2.29-4.71).
    // Shadow V4 (2026-10-17, bug 5's known reason): the CEO holds office at its Hq every workday
    // (the office was the nearest Vat Farm or none, and the exec's day leaked into the street):
    // 0.55 pooled (0.12 / 0.89 / 1.22 by seed), the range widened by it, 0.55 x 0.6 rounded down.
    // Shadow V4 review 7 (2026-10-17, its known reason): no office in a corp's Home or Lot (bug 5's
    // unranked fallback had seated housing corps' execs among their tenants): 0.22 pooled (0.00 /
    // 0.89 / 0.12 by seed) against 0.59 with the fallback on the same tree, 0.22 x 0.6 rounded down.
    (19, "ceo", Metric::KnownMetWeek, Bound::Min(0.13), 0.22, 2.57),
    (19, "ceo", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    // An exec never does a work-class action, so every run of the office-hours
    // Work goal counts here: the row reads goal flicker (2026-10-11: 0.63 after
    // the own-shift off hours, more Idle -> Work switches, the same day). Not
    // the office gate's estimate: with it read against the deadline (review
    // item 6) the row reads 0.58, and 0.57 with that change alone undone;
    // 0.52-0.63 over the round's tables, so the 0.62 bound would flip on noise.
    // Shadow V4 (2026-10-17, bug 5's known reason): an office day at the Hq every workday, so
    // more office runs: 0.84 pooled (0.73 / 0.86 / 0.76), the range widened by it, + 0.05.
    (19, "ceo", Metric::CommuteAbortsDay, Bound::Max(0.89), 0.52, 0.84),
    (19, "club_staff", Metric::WalkHDay, Bound::Max(5.6), 3.95, 4.53),
    (19, "club_staff", Metric::LongestSleepH, Bound::Min(4.0), 4.54, 4.76),
    (19, "club_staff", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "club_staff", Metric::KnownMetWeek, Bound::Min(3.2), 5.42, 7.03),
    (19, "club_staff", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "club_staff", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "club_staff", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "club_staff", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 1.00),
    (19, "cook", Metric::WalkHDay, Bound::Max(5.1), 3.71, 4.02),
    (19, "cook", Metric::LongestSleepH, Bound::Min(4.3), 4.81, 4.89),
    (19, "cook", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "cook", Metric::KnownMetWeek, Bound::Min(3.4), 5.83, 6.88),
    (19, "cook", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "cook", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "cook", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "cook", Metric::WagedWorkdays, Bound::Min(0.89), 0.99, 1.00),
    (19, "fabber", Metric::WalkHDay, Bound::Max(5.8), 4.45, 4.79),
    (19, "fabber", Metric::LongestSleepH, Bound::Min(4.2), 4.75, 4.88),
    (19, "fabber", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "fabber", Metric::KnownMetWeek, Bound::Min(0.9), 1.62, 2.88),
    (19, "fabber", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "fabber", Metric::CommuteAbortsDay, Bound::Max(0.07), 0.00, 0.02),
    (19, "fabber", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "fabber", Metric::WagedWorkdays, Bound::Min(0.83), 0.93, 0.95),
    (19, "fighter", Metric::WalkHDay, Bound::Max(6.9), 4.77, 5.84),
    (19, "fighter", Metric::LongestSleepH, Bound::Min(4.5), 5.02, 5.34),
    (19, "fighter", Metric::Energy0HWeek, Bound::Max(2.2), 0.01, 0.12),
    // Lives at work gate (2026-10-19): this row re-derived on the gate city (the eight-run ensemble:
    // shipped 2.55; stat_violence_mult 0.98 2.95, 0.99 2.60, 1.01 3.05, 1.02 3.05, 1.03 3.00; max_coarse
    // 149 3.00, 151 2.70): 2.55 x 0.6 = 1.53, rounded down. Cause: phase 2's own-Home HangOut moved free
    // time indoors with flatmates (5.55 -> 3.46 at phase 2) and the 8a regen echoes it (OPEN-103, closed
    // by the street-life feature row).
    (19, "fighter", Metric::KnownMetWeek, Bound::Min(1.5), 2.55, 3.05),
    (19, "fighter", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "fighter", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "fighter", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "fighter", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 0.99),
    (19, "fighter", Metric::BoutsAttended, Bound::Min(0.75), 1.00, 1.00),
    (19, "gang_leader", Metric::WalkHDay, Bound::Max(10.1), 4.43, 8.72),
    (19, "gang_leader", Metric::LongestSleepH, Bound::Min(4.0), 4.59, 5.61),
    (19, "gang_leader", Metric::Energy0HWeek, Bound::Max(2.8), 0.00, 0.77),
    (19, "gang_leader", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "gang_leader", Metric::CommuteAbortsDay, Bound::Max(0.26), 0.00, 0.21),
    (19, "gang_leader", Metric::PaidOfWorked, Bound::Min(0.85), 0.95, 1.00),
    (19, "gang_leader", Metric::WagedWorkdays, Bound::Min(0.64), 0.74, 1.00),
    (19, "gang_member", Metric::WalkHDay, Bound::Max(7.1), 5.55, 6.04),
    (19, "gang_member", Metric::LongestSleepH, Bound::Min(4.4), 4.90, 5.35),
    (19, "gang_member", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.03),
    // OPEN-111 (2026-10-19): this row re-derived on the fix tree (the eight-run ensemble: shipped 5.35, pick 58;
    // stat_violence_mult 0.98 4.23 (57), 0.99 4.23 (57), 1.01 7.07 (71), 1.02 7.07 (71), 1.03 6.64 (75);
    // max_coarse 149 7.34 (80), 151 6.45 (85)): 4.23 x 0.6 = 2.54, rounded down. Cause: the venues' evening staff
    // and the stable server move who is free on d19 (pick sizes 57-85, seed 42's 22 -> 9), a thin pick moved by
    // the milestone's design (OPEN-109; Dylan's 2026-10-19 rule: re-derive social-contact rows the milestone moved).
    (19, "gang_member", Metric::KnownMetWeek, Bound::Min(2.5), 4.23, 7.34),
    (19, "gang_member", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "gang_member", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "guard", Metric::WalkHDay, Bound::Max(11.4), 9.43, 9.89),
    (19, "guard", Metric::LongestSleepH, Bound::Min(4.6), 5.15, 5.25),
    (19, "guard", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "guard", Metric::KnownMetWeek, Bound::Min(2.8), 4.78, 6.94),
    (19, "guard", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "guard", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "guard", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "guard", Metric::WagedWorkdays, Bound::Min(0.86), 0.96, 0.98),
    (19, "homeless", Metric::WalkHDay, Bound::Max(6.6), 4.97, 5.53),
    (19, "homeless", Metric::LongestSleepH, Bound::Min(5.1), 5.60, 5.82),
    (19, "homeless", Metric::Energy0HWeek, Bound::Max(2.3), 0.03, 0.23),
    // Lives at work phase 5 (2026-10-18): this row alone re-derived on the phase-5 city (the eight-run
    // ensemble: shipped 5.16; stat_violence_mult 0.98 6.00, 0.99 6.00, 1.01 5.61, 1.02 6.44, 1.03 5.45;
    // max_coarse 149 7.47, 151 7.24): 5.16 x 0.6 = 3.10, rounded down. The row drifted across phases 1-5
    // from 10.8-12.5 to about 7 on main (7.00) and 5.2 here: drift across the milestone's
    // phases, not a single change (no other row moved in any of the eight runs bar max_coarse 149's three).
    (19, "homeless", Metric::KnownMetWeek, Bound::Min(3.0), 5.16, 7.47),
    (19, "homeless", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "homeless", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    // M17 phase 4b: restored to the pre-phase-3 bounds. Phase 3 loosened them to 0.54 / 0.51 for the
    // brothers' larger gangs (the shipped city read 0.64 / 0.61, 0.91 / 0.92 with the brothers off);
    // with the brothers a rare sack-gutting event the shipped city reads 0.84 / 0.80.
    (19, "homeless", Metric::PaidOfWorked, Bound::Min(0.7), 0.80, 0.94),
    // Shadow V4 (2026-10-17, the regenerated stat table): 0.59 pooled (0.44 / 0.75 / 0.62), the
    // off-screen run to day 19 housing more of the poor (11 homeless picks on seed 42, 16 before)
    // and one of the few left a new Bartender at a Bar that paid him nothing (city wages paid on
    // 42-47 x 120 rose 1-7 % over ce5afe6): the range widened by it, - 0.1.
    (19, "homeless", Metric::WagedWorkdays, Bound::Min(0.49), 0.59, 0.88),
    (19, "reporter", Metric::WalkHDay, Bound::Max(5.9), 4.03, 4.85),
    (19, "reporter", Metric::LongestSleepH, Bound::Min(4.4), 4.90, 5.27),
    (19, "reporter", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.01),
    (19, "reporter", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "reporter", Metric::CommuteAbortsDay, Bound::Max(0.19), 0.01, 0.14),
    (19, "reporter", Metric::WagedWorkdays, Bound::Min(0.64), 0.74, 0.83),
    (19, "reporter", Metric::WorkHDay, Bound::Min(2.8), 5.79, 6.17),
    (19, "ripperdoc", Metric::WalkHDay, Bound::Max(5.5), 3.72, 4.44),
    (19, "ripperdoc", Metric::LongestSleepH, Bound::Min(4.0), 4.52, 4.94),
    (19, "ripperdoc", Metric::Energy0HWeek, Bound::Max(2.2), 0.00, 0.11),
    // M17 phase 2 (one-row re-calibration, TESTING.md): the parents moved the city's trajectory and the
    // ten-agent ripperdoc row read 2.78 known met and 0.05 aborts on 42-44 (2.90 / 0.00 on c6d6ebe;
    // 5.23 / 0.03 and 4.30 / 0.01 under the 1.01 and 0.99 stat_violence_mult butterflies with the parents
    // on): the ranges widened by the new reading, the cushions as the rule.
    (19, "ripperdoc", Metric::KnownMetWeek, Bound::Min(1.6), 2.78, 9.47),
    (19, "ripperdoc", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "ripperdoc", Metric::CommuteAbortsDay, Bound::Max(0.10), 0.00, 0.05),
    // Not judged since the pre-view pass phase 5 (2026-10-21): d19 sweeper WalkHDay (a printed value, no bound).
    // `[legs] sweep_by_ride` (OPEN-156) reads the way back to the Recycler by the ride, so a shift begun far
    // from it is no longer the old stand on the beat (`sweep_stands`) but a commute to the Recycler (the round
    // must end there, P8): 7.60 h/day with it, 6.23 with it off, 6.06 with `[legs]` off. The extra
    // travel minutes are the in-shift ones with no round route (the commute to the post, ridden or walked:
    // 44k -> 93k of +54k over the tier's sweepers; the walk to the first stop +4.6k, the way back +6.7k, the stops +1.2k). Moved for a
    // known reason, the bullet demoted, the bound not widened (CLAUDE.md doctrine 2).
    (19, "sweeper", Metric::LongestSleepH, Bound::Min(3.5), 4.00, 4.08),
    (19, "sweeper", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "sweeper", Metric::KnownMetWeek, Bound::Min(2.2), 3.75, 4.67),
    (19, "sweeper", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "sweeper", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "sweeper", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "sweeper", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 1.00),
    (19, "worker", Metric::WalkHDay, Bound::Max(6.9), 5.37, 5.83),
    (19, "worker", Metric::LongestSleepH, Bound::Min(4.4), 4.97, 5.18),
    (19, "worker", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "worker", Metric::KnownMetWeek, Bound::Min(1.0), 1.74, 4.93),
    (19, "worker", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "worker", Metric::CommuteAbortsDay, Bound::Max(0.08), 0.00, 0.03),
    (19, "worker", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "worker", Metric::WagedWorkdays, Bound::Min(0.82), 0.92, 0.96),
    (19, "worker_friday", Metric::WalkHDay, Bound::Max(6.3), 4.42, 5.24),
    (19, "worker_friday", Metric::LongestSleepH, Bound::Min(3.5), 4.09, 4.39),
    (19, "worker_friday", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "worker_friday", Metric::KnownMetWeek, Bound::Min(2.6), 4.47, 5.55),
    (19, "worker_friday", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "worker_friday", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "worker_friday", Metric::PaidOfWorked, Bound::Min(0.86), 0.96, 0.99),
    (19, "worker_friday", Metric::WagedWorkdays, Bound::Min(0.85), 0.95, 0.97),
    (90, "gang_leader", Metric::WalkHDay, Bound::Max(9.9), 6.10, 8.53),
    (90, "gang_leader", Metric::LongestSleepH, Bound::Min(4.4), 4.99, 6.08),
    (90, "gang_leader", Metric::Energy0HWeek, Bound::Max(3.1), 0.12, 1.05),
    (90, "gang_leader", Metric::KnownMetWeek, Bound::Min(0.1), 0.25, 6.38),
    (90, "gang_leader", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "gang_leader", Metric::CommuteAbortsDay, Bound::Max(0.18), 0.00, 0.13),
    // Jobs & lives phase-8 review (2026-10-17): this row alone re-derived on the review's city with
    // the eight-run ensemble (main 0.77; stat_violence_mult 0.98 1.00, 0.99 1.00, 1.01 0.74, 1.02
    // 0.85, 1.03 SKIP (16 shifts); max_coarse 149 0.96, 151 0.97; read on 7 of 8): 0.74 - 0.1 =
    // 0.64. The misses were leaders on side jobs at an owner short of coins (unpaid, then a quit).
    (90, "gang_leader", Metric::PaidOfWorked, Bound::Min(0.64), 0.74, 1.00),
    (90, "gang_leader", Metric::WagedWorkdays, Bound::Min(0.59), 0.69, 0.97),
    (90, "gang_member", Metric::WalkHDay, Bound::Max(7.8), 5.35, 6.70),
    (90, "gang_member", Metric::LongestSleepH, Bound::Min(5.2), 5.78, 6.09),
    (90, "gang_member", Metric::Energy0HWeek, Bound::Max(2.7), 0.19, 0.70),
    (90, "gang_member", Metric::KnownMetWeek, Bound::Min(2.3), 3.94, 5.91),
    (90, "gang_member", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "gang_member", Metric::CommuteAbortsDay, Bound::Max(0.07), 0.00, 0.02),
    (90, "homeless", Metric::WalkHDay, Bound::Max(6.1), 4.37, 5.01),
    (90, "homeless", Metric::LongestSleepH, Bound::Min(5.6), 6.15, 6.52),
    (90, "homeless", Metric::Energy0HWeek, Bound::Max(3.8), 1.23, 1.79),
    // Lives at work gate (2026-10-19): this row re-derived on the gate city (the eight-run ensemble:
    // shipped 3.73; stat_violence_mult 0.98 3.74, 0.99 4.48, 1.01 4.84, 1.02 4.16, 1.03 2.91; max_coarse
    // 149 4.13, 151 5.06): 2.91 x 0.6 = 1.75, rounded down. Cause: phase 2's own-Home HangOut and the 8a
    // regen (OPEN-103, closed by the street-life feature row).
    (90, "homeless", Metric::KnownMetWeek, Bound::Min(1.7), 2.91, 5.06),
    (90, "homeless", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    // Lives at work gate 8d (2026-10-19): re-derived on the gate city (the eight-run ensemble on the 8d
    // fix round's stat table: shipped 0.07; stat_violence_mult 0.98 0.04, 0.99 0.04, 1.01 0.04, 1.02
    // 0.03, 1.03 0.04; max_coarse 149 0.08, 151 0.04): 0.08 + 0.05 = 0.13. The milestone moved it off the
    // old 0.00-0.02 before the regen (8c main 0.06, the 8d tree 0.04 on the old table); not the deposit
    // (`move_in_rent_days` 0 reads 0.09). The highs are one seed's commuters (seed 44: 0.24, 0.20 of
    // 16-19 picks; the seeds read 0.00-0.24 over the ensemble).
    (90, "homeless", Metric::CommuteAbortsDay, Bound::Max(0.13), 0.03, 0.08),
    (90, "homeless", Metric::PaidOfWorked, Bound::Min(0.68), 0.78, 0.92),
    (90, "homeless", Metric::WagedWorkdays, Bound::Min(0.43), 0.53, 0.69),
    (90, "purist", Metric::WalkHDay, Bound::Max(7.1), 4.39, 6.08),
    (90, "purist", Metric::LongestSleepH, Bound::Min(5.0), 5.51, 6.22),
    (90, "purist", Metric::Energy0HWeek, Bound::Max(2.6), 0.01, 0.56),
    (90, "purist", Metric::KnownMetWeek, Bound::Min(1.0), 1.73, 10.06),
    (90, "purist", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "purist", Metric::CommuteAbortsDay, Bound::Max(0.07), 0.00, 0.02),
];

/// One archetype's measured sample in one window of one seed (`None`: no free candidate).
type SampleRun = Result<(u64, u64, String, Option<Row>), String>;

/// What `behaviour` found: the printed table and the failed checks.
pub struct BehaviourReport {
    pub text: String,
    pub failures: Vec<String>,
}

/// The behaviour tier's sample of `arch`: its free candidates in the
/// shadow's pick order (shuffled by the seed; the CEOs by treasury), at most
/// `BEHAVIOUR_SAMPLE` of them.
fn sample(world: &World, seed: u64, arch: &str) -> Result<Vec<EntityId>, String> {
    let c = candidates(world, arch)?;
    let ordered = if arch == "ceo" { c } else { shuffled(c, seed, arch) };
    Ok(ordered.into_iter().filter(|&id| is_free(world, id)).take(BEHAVIOUR_SAMPLE).collect())
}

/// One window of one seed: the world run to `day` once, then each
/// archetype's sample followed in its own clone of it (a thread each); the
/// first `BEHAVIOUR_COUNT` of a sample write their diaries under
/// `out/d<day>` (seed 42) or `out/s<seed>_d<day>`.
fn behaviour_window(seed: u64, day: u64, list: &'static str, out: Option<&std::path::Path>) -> Vec<SampleRun> {
    let archs: Vec<&str> = list.split(',').collect();
    window_runs(seed, day, &archs, out)
}

/// `behaviour_window`'s body over an archetype list (shared with `--lives`).
fn window_runs(seed: u64, day: u64, archs: &[&str], out: Option<&std::path::Path>) -> Vec<SampleRun> {
    let world = start_world(seed, day, None, None).expect("the shipped map");
    let jobs: Vec<(&str, Result<Vec<EntityId>, String>)> =
        archs.iter().map(|&arch| (arch, sample(&world, seed, arch))).collect();
    std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .into_iter()
            .map(|(arch, ids)| {
                let mut w = world.clone();
                s.spawn(move || -> SampleRun {
                    let ids = ids?;
                    if ids.is_empty() {
                        return Ok((seed, day, arch.to_string(), None));
                    }
                    let picked: Vec<(String, EntityId)> = ids.iter().map(|&id| (arch.to_string(), id)).collect();
                    let mut tracks = follow(&mut w, &picked, BEHAVIOUR_DAYS, true, false);
                    if let Some(dir) = out {
                        let sub = if seed == BEHAVIOUR_SEED { format!("d{day}") } else { format!("s{seed}_d{day}") };
                        let dir = dir.join(sub);
                        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                        for t in tracks.iter_mut().take(BEHAVIOUR_COUNT) {
                            write_diary(&w, t, &dir, w.tick)?;
                        }
                    }
                    let mut row = Row::default();
                    for t in &tracks {
                        row.add(t);
                    }
                    Ok((seed, day, arch.to_string(), Some(row)))
                })
            })
            .collect();
        hs.into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err(format!("a seed {seed} d{day} behaviour sample panicked"))))
            .collect()
    })
}

/// Run the behaviour tier's windows (each archetype's sample in its own
/// thread), write the diaries under `out` if given, and judge `CHECKS`.
pub fn behaviour(out: Option<&std::path::Path>) -> Result<BehaviourReport, String> {
    let seeds = BEHAVIOUR_SEEDS;
    let runs: Vec<Vec<SampleRun>> = std::thread::scope(|s| {
        let hs: Vec<_> = seeds
            .iter()
            .flat_map(|&seed| BEHAVIOUR_WINDOWS.iter().map(move |&(day, list)| (seed, day, list)))
            .map(|(seed, day, list)| s.spawn(move || behaviour_window(seed, day, list, out)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| vec![Err("a behaviour window panicked".into())])).collect()
    });
    let mut aggs: BTreeMap<(u64, String), Row> = BTreeMap::new();
    let mut per_seed: BTreeMap<(u64, String, u64), Row> = BTreeMap::new();
    for r in runs.into_iter().flatten() {
        if let (seed, day, arch, Some(row)) = r? {
            aggs.entry((day, arch.clone())).or_default().extend(&row);
            per_seed.insert((day, arch, seed), row);
        }
    }
    let mut text = String::new();
    let _ = writeln!(
        text,
        "behaviour tier: seeds {seeds:?} pooled, {BEHAVIOUR_DAYS} days, up to {BEHAVIOUR_SAMPLE} free agents per archetype \
         and seed (each archetype in its own clone, pinned at Full); shares pooled (>= {BEHAVIOUR_MIN_SHARE} shifts or \
         bouts), rates a {:.0} % trimmed mean",
        BEHAVIOUR_TRIM * 100.0
    );
    let _ = write!(text, "{:<4} {:<14} {:>2}", "day", "archetype", "n");
    for m in METRICS {
        let _ = write!(text, " {:>9}", short(m));
    }
    let _ = writeln!(text);
    for ((day, arch), a) in &aggs {
        row_line(&mut text, *day, arch, a);
    }
    // Which seed carried a row: read before the diaries of a failure.
    let _ = writeln!(text, "per seed (archetype@seed):");
    for ((day, arch, seed), a) in &per_seed {
        row_line(&mut text, *day, &format!("{arch}@{seed}"), a);
    }
    let (mut failures, mut asserted, mut findings, mut missed) = (Vec::new(), 0usize, 0usize, 0usize);
    for &(day, arch, m, bound, lo, hi) in CHECKS {
        let Some(a) = aggs.get(&(day, arch.to_string())) else {
            let _ = writeln!(text, "SKIP d{day} {arch} {m:?}: no candidate in the window");
            continue;
        };
        let Some(v) = a.get(m, true) else {
            let _ = writeln!(
                text,
                "SKIP d{day} {arch} {m:?}: too little to read ({} agents, {} shifts, {} bouts; the floors are \
                 {BEHAVIOUR_MIN_AGENTS} agents, {BEHAVIOUR_MIN_SHARE} shifts or bouts for a share)",
                a.agents.len(),
                a.pooled.shifts,
                a.pooled.bouts
            );
            continue;
        };
        let holds = |bd: Bound, v: f64| match bd {
            Bound::Min(b) => (v >= b, format!(">= {b}")),
            Bound::Max(b) => (v <= b, format!("<= {b}")),
        };
        let n = a.agents.len();
        let (cal_ok, cal_rule) = holds(bound, v);
        let cal = format!("calibration {cal_rule} {} (range {lo:.2}-{hi:.2})", if cal_ok { "ok" } else { "MISS" });
        let collapse = COLLAPSE.iter().find(|(cm, _)| *cm == m).map(|&(_, cb)| cb);
        match collapse {
            Some(cb) if n >= BEHAVIOUR_ASSERT_MIN_N => {
                let (ok, rule) = holds(cb, v);
                let line = format!("d{day} {arch} {m:?} {v:.2} n {n} collapse {rule}; {cal}");
                let _ = writeln!(text, "{} {line}", if ok { "PASS" } else { "FAIL" });
                asserted += 1;
                if !ok {
                    failures.push(line);
                }
                if !cal_ok {
                    let _ =
                        writeln!(text, "FINDING d{day} {arch} {m:?} {v:.2} n {n} (asserted row, collapse ok); {cal}");
                    missed += 1;
                }
            }
            _ => {
                let why = if collapse.is_some() {
                    format!("n under {BEHAVIOUR_ASSERT_MIN_N}, not asserted")
                } else {
                    "no collapse rule".to_string()
                };
                let _ = writeln!(text, "FINDING d{day} {arch} {m:?} {v:.2} n {n} ({why}); {cal}");
                findings += 1;
                if !cal_ok {
                    missed += 1;
                }
            }
        }
    }
    let _ = writeln!(
        text,
        "behaviour tier: {asserted} rows asserted (collapse bounds), {findings} rows printed only as FINDING, {missed} \
         rows outside their calibration range (FINDING lines), {} failed",
        failures.len()
    );
    Ok(BehaviourReport { text, failures })
}

// ---------------------------------------------------------------------------
// Lives at work phase 0 (`shadow --lives`; docs/LIVES_AT_WORK.md § 9 row 0)
// ---------------------------------------------------------------------------

/// The generated city's window (V4's 20k recipe: `--seed 42 --start-day 20
/// --days 7`): every archetype pinned together in one world, this many each.
const LIVES_BIG_SEED: u64 = 42;
const LIVES_BIG_DAY: u64 = 20;
const LIVES_COUNT_BIG: usize = 5;

/// The lives baseline: the 2k windows (`--assert`'s first window over every
/// pinnable archetype, the seeds pooled) or, with `population`, one window of
/// the generated city. Returns the `PROBE lives` lines.
pub fn lives(
    out: Option<&std::path::Path>,
    population: Option<u32>,
    map: Option<&std::path::Path>,
) -> Result<String, String> {
    let archs: Vec<&str> = ARCHETYPES.iter().copied().filter(|&a| a != "child").collect();
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let (city, window) = match population {
        Some(n) => {
            // Shadow V7 (tool): `--map` is read here too (the island at 20k).
            let mut world = start_world(LIVES_BIG_SEED, LIVES_BIG_DAY, Some(n), map)?;
            let list = archs.join(",");
            let picked = pick(&world, LIVES_BIG_SEED, LIVES_BIG_DAY, Some(&list), LIVES_COUNT_BIG, &[]);
            if picked.is_empty() {
                return Err("lives: no candidates in the generated city".into());
            }
            // As `shadow --pick`: the pins sit inside `[lod] max_full`, grown by the pick count.
            world.config.lod.max_full += picked.len();
            let mut tracks = follow(&mut world, &picked, BEHAVIOUR_DAYS, true, false);
            if let Some(dir) = out {
                let dir = dir.join(format!("lives_{n}_d{LIVES_BIG_DAY}"));
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let end = world.tick;
                for t in &mut tracks {
                    write_diary(&world, t, &dir, end)?;
                }
            }
            for t in &tracks {
                rows.entry(t.arch.clone()).or_default().add(t);
            }
            let stem = map.and_then(|m| m.file_stem()).map(|m| format!("@{}", m.to_string_lossy()));
            (
                format!("{}k{}", n / 1000, stem.unwrap_or_default()),
                format!(
                    "seed {LIVES_BIG_SEED}, day {LIVES_BIG_DAY}, {BEHAVIOUR_DAYS} days, {} picks of {} archetypes pinned \
                     together in one world (up to {LIVES_COUNT_BIG} each)",
                    picked.len(),
                    archs.len()
                ),
            )
        }
        None => {
            let day = BEHAVIOUR_WINDOWS[0].0;
            // One seed at a time: a clone of the city per archetype is ~48 worlds.
            for &seed in &BEHAVIOUR_SEEDS {
                for r in window_runs(seed, day, &archs, out) {
                    if let (_, _, arch, Some(row)) = r? {
                        rows.entry(arch).or_default().extend(&row);
                    }
                }
            }
            (
                "2k".to_string(),
                format!(
                    "seeds {BEHAVIOUR_SEEDS:?} pooled, day {day}, {BEHAVIOUR_DAYS} days, up to {BEHAVIOUR_SAMPLE} free \
                     agents per archetype and seed, each archetype in its own clone (pinned at Full)"
                ),
            )
        }
    };
    let mut text = String::new();
    let _ = writeln!(
        text,
        "PROBE lives {city}: {window}; rates per agent-day or agent-week pooled over the row's agents; a spouse row \
         reads the married picks only; shifts are whole free workday shifts at a building; rows under \
         {BEHAVIOUR_MIN_AGENTS} agents are one person's week; `all` pools the rows (an agent picked for two \
         archetypes counts in both)"
    );
    let mut all = Row::default();
    for (arch, row) in &rows {
        all.extend(row);
        lives_line(&mut text, &city, arch, &row.pooled);
    }
    lives_line(&mut text, &city, "all", &all.pooled);
    Ok(text)
}

/// One `PROBE lives` row: the archetype's pooled numbers.
fn lives_line(text: &mut String, city: &str, label: &str, a: &Agg) {
    let days = a.agent_days();
    let per_day = |n: f64| if days > 0.0 { n / days } else { 0.0 };
    let share = |n: u32, d: u32| if d > 0 { f64::from(n) / f64::from(d) } else { 0.0 };
    let married_days = a.married_min as f64 / TICKS_PER_DAY as f64;
    let per_married_week = |n: f64| if married_days > 0.0 { n / married_days * 7.0 } else { 0.0 };
    let mut kinds: Vec<(&String, &u32)> = a.source_kinds.iter().collect();
    kinds.sort_by(|x, y| y.1.cmp(x.1).then(x.0.cmp(y.0)));
    let kinds: Vec<String> = kinds.iter().take(3).map(|(k, n)| format!("{k} {n}")).collect();
    let _ = writeln!(
        text,
        "PROBE lives {city} {label}: n {} agent-days {days:.1} | idle_h/d {:.2} short_rest/d {:.2} | married {} never_named {} never_coloc {} \
         named/wk {:.2} coloc_h/wk {:.2} (stat-spouse {:.2}) | hang/wk {:.2} empty {:.2} ({}/{}) | shifts {} servedable \
         {:.2} any {:.2} bodies/h {:.2} venue_visits/shift {:.2} met {:.2} enc/shift {:.2} | travel_h/wk {:.2}          spire {}/{} | kinds: {}",
        a.agents,
        per_day(a.idle_min as f64 / 60.0),
        per_day(f64::from(a.short_rests)),
        a.married,
        a.never_named,
        a.never_coloc,
        per_married_week(f64::from(a.spouse_named_hours)),
        per_married_week(f64::from(a.spouse_coloc)),
        share(a.spouse_coloc_stat, a.spouse_coloc),
        per_day(f64::from(a.hangouts)) * 7.0,
        share(a.hang_empty, a.hangouts),
        a.hang_empty,
        a.hangouts,
        a.served_shifts,
        share(a.servedable, a.served_shifts),
        share(a.servedable_any, a.served_shifts),
        if a.shift_samples > 0 { a.nonstaff as f64 / a.shift_samples as f64 } else { 0.0 },
        if a.served_shifts > 0 { a.venue_visits as f64 / f64::from(a.served_shifts) } else { 0.0 },
        share(a.shifts_met, a.served_shifts),
        if a.served_shifts > 0 { a.encounters as f64 / f64::from(a.served_shifts) } else { 0.0 },
        per_day(a.travel_min as f64 / 60.0) * 7.0,
        a.spire,
        a.housed,
        if kinds.is_empty() { "none".to_string() } else { kinds.join(", ") }
    );
}

/// One printed row of the behaviour table (every value, the floors aside).
fn row_line(text: &mut String, day: u64, label: &str, a: &Row) {
    let _ = write!(text, "d{day:<3} {label:<14} {:>2}", a.agents.len());
    for m in METRICS {
        match a.get(m, false) {
            Some(v) => {
                let _ = write!(text, " {v:>9.2}");
            }
            None => {
                let _ = write!(text, " {:>9}", "-");
            }
        }
    }
    let _ = writeln!(text);
}

fn short(m: Metric) -> &'static str {
    match m {
        Metric::WalkHDay => "walk_h/d",
        Metric::SleepHDay => "sleep_h/d",
        Metric::LongestSleepH => "longest_h",
        Metric::Energy0HWeek => "e0_h/wk",
        Metric::WorkHDay => "work_h/d",
        Metric::ShiftsWorkedShare => "worked",
        Metric::PaidOfWorked => "paid/wkd",
        Metric::WagedWorkdays => "waged_wd",
        Metric::HangoutsWeek => "hang/wk",
        Metric::KnownMetWeek => "known/wk",
        Metric::RefundsWeek => "refund/wk",
        Metric::CommuteAbortsDay => "abort/d",
        Metric::BoutsAttended => "bouts_at",
        Metric::StarveHWeek => "starve_h/wk",
        Metric::SynthShare => "synth%",
        Metric::DivesWeek => "dive/wk",
    }
}

/// Hunger as sickness: the meters on one line (the header and meal lines).
fn health_line(h: &citysim::Health) -> String {
    let cancer = h.cancer.map_or(String::from("no cancer"), |c| {
        format!("cancer stage {:.2}{}", c.stage, if c.diagnosed { " (diagnosed)" } else { " (silent)" })
    });
    format!(
        "malnutrition {:.2} | toxicity {:.2} | {cancer} | sick days {}/14{}",
        h.malnutrition,
        h.toxicity,
        h.sick_days.count_ones(),
        if h.sick_today { " | OFF SICK TODAY" } else { "" }
    )
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
