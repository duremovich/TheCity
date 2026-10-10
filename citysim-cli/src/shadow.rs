//! `citysim-cli shadow`: follow individual residents through their days.
//!
//! ```text
//! citysim-cli shadow --seed S --days N [--start-day D] [--pick a,b,..]
//!                    [--agent INDEX]... [--count K] --out DIR
//! citysim-cli shadow --seed S --start-day D --list
//! citysim-cli shadow --assert [--out DIR]
//! ```
//!
//! An observation tool. The world runs normally to `--start-day`; the chosen
//! agents are then pinned (`Brain.pinned`) so they live at Full LOD, and each
//! tick their state is diffed to write a diary (`<archetype>_<index>.md` and
//! `.jsonl`). The only sim hook is `World::shadow_notes` (social moves and
//! gossip tellings), a no-op unless turned on here. Nothing here draws RNG.

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
    /// worker_friday, super, bouncer, dancer, night_shift, orderly, camp_warden, fixer, gun (a child
    /// is unpinnable: no Brain).
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
}

const ARCHETYPES: [&str; 27] = [
    "gang_member",
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
];

/// Activity classes of the time-use table, in print order.
const CLASSES: [&str; 11] =
    ["sleep", "work", "eat", "social", "leisure", "travel", "crime", "idle", "jail", "stat", "other"];

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
}

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
            shift: None,
            shifts: Vec::new(),
            wages: Vec::new(),
            refunds: 0,
            work_plan: None,
            commute_aborts: 0,
            bouts: 0,
            bouts_present: 0,
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
        return "travel";
    }
    let Some(step) = b.plan.as_ref().and_then(|_| b.current_step()) else { return "idle" };
    use ActionKind as A;
    match step.action {
        A::Sleep | A::CheckIn => "sleep",
        A::Rest if b.plan_goal() == Some(GoalKind::Sleep) => "sleep",
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
        | A::Collect => "work",
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
    let owned: Vec<String> = world
        .buildings_by_kind
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

fn goal_line(world: &World, b: &Brain) -> String {
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
    let _ = world;
    s
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

fn mem_text(world: &World, e: &MemoryEntry) -> String {
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
        flush_day(t, now);
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
    }
    let mut flow_ctx: Vec<String> = Vec::new();
    for n in notes {
        match n {
            ShadowNote::Move { tick, m, out } if m.actor == id || m.target == id => {
                let other = if m.actor == id { m.target } else { m.actor };
                t.contact(world, other);
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
                let e = t.flow_kinds.entry(format!("{flow:?}")).or_insert((0, 0, 0));
                e.0 += out;
                e.1 += inn;
                e.2 += 1;
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
                        format!("TRIBUTE received +{coins}c from {who} (the weekly Collect pay-out)"),
                    );
                }
                let sign = if inn > 0 { "+" } else { "-" };
                flow_ctx.push(format!(
                    "{flow:?} {sign}{}{} {} {who}",
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
                } else if t.pinned && b.lod != Lod::Full {
                    " (sentencing sets Coarse until the next hourly assignment re-pins it)"
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
                        goal_line(world, b)
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
                let text = mem_text(world, e);
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
    let tod = (now % TICKS_PER_DAY) as u16;
    match world.comp::<Job>(id) {
        Some(j) if j.on_shift(tod) => {
            let s = t.shift.get_or_insert_with(|| ShiftObs {
                start: now,
                end: now,
                len: 0,
                work: 0,
                workday: citysim::exec::routine::workday_of(world, id, j, j.shift_key_at(now)),
                whole: j.shifts.iter().any(|&(s, _)| s == tod),
                free: true,
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
    let end_place = t.last_place.clone();
    let tgt = o.target.map_or(String::new(), |x| format!(" [target {}]", nm(world, x)));
    if o.action == ActionKind::HangOut {
        // V2: who was at the spot, the known contacts named.
        let others: Vec<EntityId> = std::mem::take(&mut t.hang_others).into_iter().collect();
        t.hangouts += 1;
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
        } else {
            format!("{} -> {}", o.start_place, end_place)
        }
    } else {
        end_place
    };
    t.log(o.start, "action", format!("ACTION {:?} @ {loc}, {dur} min ({status}){tgt}", o.action));
}

fn flush_day(t: &mut Track, now: Tick) {
    let day = t.cur_day;
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
        "DAY SUMMARY d{day}: {} | net {}{} | places: {}",
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
            "- #{} {} on {} for {} ({role}, {:?}, {:?}, posted {}{})",
            c.id,
            c.kind.label(),
            nm(world, c.target.id()),
            money(c.price),
            c.status,
            c.render,
            clock(c.posted),
            c.broker.map(|b| format!(" at {}", bname(world, b))).unwrap_or_default()
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
    if !args.list && args.out.is_none() {
        return Err("--out <dir> is required (or use --list or --assert)".into());
    }
    let mut world = start_world(args.seed, args.start_day);

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
    let mut tracks = follow(&mut world, &picked, args.days, !args.no_pin, true);
    let end = world.tick;
    let mut total = 0u64;
    for t in &mut tracks {
        let (md, jl) = write_diary(&world, t, &dir, end)?;
        total += md + jl;
        println!("{}_{}: {} ({} KB md, {} KB jsonl)", t.arch, t.id.index, t.name, md / 1024, jl / 1024);
    }
    println!("wrote {} diaries to {} ({} KB)", tracks.len(), dir.display(), total / 1024);
    Ok(())
}

/// The world of `seed` run normally to the start of `start_day`.
fn start_world(seed: u64, start_day: u64) -> World {
    let mut world = World::new(seed, Config::load());
    let start_tick = start_day * TICKS_PER_DAY;
    while world.tick < start_tick {
        citysim::tick(&mut world);
    }
    world
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
        for arch in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
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
                    let ordered = if arch == "ceo" { c } else { shuffled(c, seed, arch) };
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
        flush_day(t, end);
        end_sleep_block(t, end);
    }
    tracks
}

// ---------------------------------------------------------------------------
// The behaviour tier (`shadow --assert`; docs/TESTING.md)
// ---------------------------------------------------------------------------

/// The behaviour tier's windows: 7 days from day 19 (a Friday, so
/// `worker_friday` starts on its Friday) with every archetype, and from day
/// 90 (the gangs full) with the gang archetypes: the V2 shadow pass's
/// pinned windows (docs/SHADOW_V2.md), on each of `BEHAVIOUR_SEEDS`.
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
}

impl Agg {
    fn add(&mut self, t: &Track) {
        self.agents += 1;
        self.alive_min += u64::from(t.alive_min);
        self.travel_min += u64::from(t.total_use.get("travel").copied().unwrap_or(0));
        self.sleep_min += u64::from(t.total_use.get("sleep").copied().unwrap_or(0));
        self.work_min += u64::from(t.work_min);
        self.energy0_min += u64::from(t.energy0_min);
        let mut by_night: BTreeMap<u64, u32> = BTreeMap::new();
        for &(end, len) in &t.sleep_blocks {
            let night = end.saturating_sub(TICKS_PER_DAY / 2) / TICKS_PER_DAY;
            let e = by_night.entry(night).or_insert(0);
            *e = (*e).max(len);
        }
        self.nights.extend(by_night.values());
        for s in t.shifts.iter().filter(|s| s.whole && s.workday && s.free) {
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

    /// Pool another agent's sums into these.
    fn merge(&mut self, o: &Agg) {
        self.agents += o.agents;
        self.alive_min += o.alive_min;
        self.travel_min += o.travel_min;
        self.sleep_min += o.sleep_min;
        self.work_min += o.work_min;
        self.energy0_min += o.energy0_min;
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
}

impl Metric {
    /// A share of shifts or bouts: pooled over the sample (one agent's two
    /// shifts are no rate). Every other metric is a per-agent rate,
    /// trimmed-mean averaged.
    fn pooled(self) -> bool {
        matches!(self, Metric::ShiftsWorkedShare | Metric::PaidOfWorked | Metric::WagedWorkdays | Metric::BoutsAttended)
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

const METRICS: [Metric; 13] = [
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
];

/// A bound: the value must be at least (`Min`) or at most (`Max`) it.
#[derive(Clone, Copy)]
enum Bound {
    Min(f64),
    Max(f64),
}

/// One behaviour check: (window start day, archetype, metric, bound, and
/// the lowest and highest value the row read on the calibration ensemble:
/// main and seven config butterflies, the noise a city change that is not a
/// behaviour change makes). Bounds sit a cushion outside that range: they
/// catch a regression in "acts like a person", they are not targets.
type Check = (u64, &'static str, Metric, Bound, f64, f64);

const CHECKS: &[Check] = &[
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
    (19, "ceo", Metric::WalkHDay, Bound::Max(5.9), 4.15, 4.90),
    (19, "ceo", Metric::LongestSleepH, Bound::Min(4.3), 4.86, 5.14),
    (19, "ceo", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "ceo", Metric::KnownMetWeek, Bound::Min(0.7), 1.33, 2.62),
    (19, "ceo", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "ceo", Metric::CommuteAbortsDay, Bound::Max(0.61), 0.50, 0.56),
    (19, "club_staff", Metric::WalkHDay, Bound::Max(5.5), 3.97, 4.41),
    (19, "club_staff", Metric::LongestSleepH, Bound::Min(4.4), 4.93, 5.06),
    (19, "club_staff", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "club_staff", Metric::KnownMetWeek, Bound::Min(5.1), 8.60, 9.81),
    (19, "club_staff", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "club_staff", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "club_staff", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "club_staff", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 1.00),
    (19, "cook", Metric::WalkHDay, Bound::Max(5.5), 3.90, 4.47),
    (19, "cook", Metric::LongestSleepH, Bound::Min(4.6), 5.16, 5.24),
    (19, "cook", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.01),
    (19, "cook", Metric::KnownMetWeek, Bound::Min(5.9), 9.92, 10.46),
    (19, "cook", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "cook", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "cook", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "cook", Metric::WagedWorkdays, Bound::Min(0.87), 0.97, 0.99),
    (19, "fabber", Metric::WalkHDay, Bound::Max(6.6), 5.20, 5.53),
    (19, "fabber", Metric::LongestSleepH, Bound::Min(4.5), 5.09, 5.26),
    (19, "fabber", Metric::Energy0HWeek, Bound::Max(2.0), 0.00, 0.00),
    (19, "fabber", Metric::KnownMetWeek, Bound::Min(5.5), 9.18, 11.80),
    (19, "fabber", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "fabber", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "fabber", Metric::PaidOfWorked, Bound::Min(0.88), 0.98, 1.00),
    (19, "fabber", Metric::WagedWorkdays, Bound::Min(0.88), 0.98, 1.00),
    (19, "fighter", Metric::WalkHDay, Bound::Max(7.6), 5.35, 6.55),
    (19, "fighter", Metric::LongestSleepH, Bound::Min(5.0), 5.56, 6.02),
    (19, "fighter", Metric::Energy0HWeek, Bound::Max(2.5), 0.13, 0.41),
    (19, "fighter", Metric::KnownMetWeek, Bound::Min(3.0), 5.15, 6.55),
    (19, "fighter", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "fighter", Metric::CommuteAbortsDay, Bound::Max(0.08), 0.00, 0.03),
    (19, "fighter", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "fighter", Metric::WagedWorkdays, Bound::Min(0.84), 0.94, 1.00),
    (19, "fighter", Metric::BoutsAttended, Bound::Min(0.75), 1.00, 1.00),
    (19, "gang_leader", Metric::WalkHDay, Bound::Max(8.6), 5.71, 7.40),
    (19, "gang_leader", Metric::LongestSleepH, Bound::Min(3.7), 4.24, 5.64),
    (19, "gang_leader", Metric::Energy0HWeek, Bound::Max(3.2), 0.01, 1.12),
    (19, "gang_leader", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "gang_leader", Metric::CommuteAbortsDay, Bound::Max(0.24), 0.02, 0.19),
    (19, "gang_leader", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "gang_leader", Metric::WagedWorkdays, Bound::Min(0.86), 0.96, 0.96),
    (19, "gang_member", Metric::WalkHDay, Bound::Max(6.8), 4.74, 5.73),
    (19, "gang_member", Metric::LongestSleepH, Bound::Min(4.1), 4.68, 5.06),
    (19, "gang_member", Metric::Energy0HWeek, Bound::Max(2.2), 0.00, 0.14),
    (19, "gang_member", Metric::KnownMetWeek, Bound::Min(3.9), 6.50, 13.40),
    (19, "gang_member", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "gang_member", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "guard", Metric::WalkHDay, Bound::Max(9.6), 7.16, 8.32),
    (19, "guard", Metric::LongestSleepH, Bound::Min(4.6), 5.15, 5.45),
    (19, "guard", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.01),
    (19, "guard", Metric::KnownMetWeek, Bound::Min(7.1), 11.95, 14.48),
    (19, "guard", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "guard", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "guard", Metric::PaidOfWorked, Bound::Min(0.9), 1.00, 1.00),
    (19, "guard", Metric::WagedWorkdays, Bound::Min(0.84), 0.94, 0.98),
    (19, "homeless", Metric::WalkHDay, Bound::Max(6.6), 5.05, 5.53),
    (19, "homeless", Metric::LongestSleepH, Bound::Min(4.8), 5.36, 5.54),
    (19, "homeless", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.04),
    (19, "homeless", Metric::KnownMetWeek, Bound::Min(6.0), 10.00, 11.85),
    (19, "homeless", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "homeless", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
    (19, "homeless", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "homeless", Metric::WagedWorkdays, Bound::Min(0.82), 0.92, 0.94),
    (19, "reporter", Metric::WalkHDay, Bound::Max(9.0), 5.85, 7.79),
    (19, "reporter", Metric::LongestSleepH, Bound::Min(4.9), 5.40, 5.64),
    (19, "reporter", Metric::Energy0HWeek, Bound::Max(5.0), 0.89, 2.94),
    (19, "reporter", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "reporter", Metric::CommuteAbortsDay, Bound::Max(0.42), 0.21, 0.37),
    (19, "reporter", Metric::WagedWorkdays, Bound::Min(0.21), 0.31, 0.41),
    (19, "reporter", Metric::WorkHDay, Bound::Min(1.0), 2.08, 3.01),
    (19, "ripperdoc", Metric::WalkHDay, Bound::Max(5.5), 3.57, 4.42),
    (19, "ripperdoc", Metric::LongestSleepH, Bound::Min(4.2), 4.78, 5.22),
    (19, "ripperdoc", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.04),
    (19, "ripperdoc", Metric::KnownMetWeek, Bound::Min(5.8), 9.75, 17.67),
    (19, "ripperdoc", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "ripperdoc", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (19, "ripperdoc", Metric::PaidOfWorked, Bound::Min(0.69), 0.79, 0.93),
    (19, "ripperdoc", Metric::WagedWorkdays, Bound::Min(0.68), 0.78, 0.93),
    (19, "sweeper", Metric::WalkHDay, Bound::Max(8.8), 6.74, 7.60),
    (19, "sweeper", Metric::LongestSleepH, Bound::Min(4.0), 4.52, 4.72),
    (19, "sweeper", Metric::Energy0HWeek, Bound::Max(2.4), 0.06, 0.37),
    (19, "sweeper", Metric::KnownMetWeek, Bound::Min(7.0), 11.68, 14.03),
    (19, "sweeper", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "sweeper", Metric::CommuteAbortsDay, Bound::Max(0.25), 0.08, 0.20),
    (19, "sweeper", Metric::PaidOfWorked, Bound::Min(0.89), 0.99, 1.00),
    (19, "sweeper", Metric::WagedWorkdays, Bound::Min(0.81), 0.91, 0.95),
    (19, "worker", Metric::WalkHDay, Bound::Max(7.5), 6.00, 6.50),
    (19, "worker", Metric::LongestSleepH, Bound::Min(4.6), 5.10, 5.30),
    (19, "worker", Metric::Energy0HWeek, Bound::Max(2.3), 0.06, 0.28),
    (19, "worker", Metric::KnownMetWeek, Bound::Min(3.3), 5.62, 7.49),
    (19, "worker", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "worker", Metric::CommuteAbortsDay, Bound::Max(0.09), 0.02, 0.04),
    (19, "worker", Metric::PaidOfWorked, Bound::Min(0.88), 0.98, 1.00),
    (19, "worker", Metric::WagedWorkdays, Bound::Min(0.82), 0.92, 0.97),
    (19, "worker_friday", Metric::WalkHDay, Bound::Max(6.9), 4.98, 5.87),
    (19, "worker_friday", Metric::LongestSleepH, Bound::Min(4.3), 4.85, 5.09),
    (19, "worker_friday", Metric::Energy0HWeek, Bound::Max(2.1), 0.00, 0.07),
    (19, "worker_friday", Metric::KnownMetWeek, Bound::Min(4.5), 7.54, 9.96),
    (19, "worker_friday", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (19, "worker_friday", Metric::CommuteAbortsDay, Bound::Max(0.12), 0.02, 0.07),
    (19, "worker_friday", Metric::PaidOfWorked, Bound::Min(0.87), 0.97, 0.99),
    (19, "worker_friday", Metric::WagedWorkdays, Bound::Min(0.81), 0.91, 0.95),
    (90, "gang_leader", Metric::WalkHDay, Bound::Max(9.0), 5.07, 7.80),
    (90, "gang_leader", Metric::LongestSleepH, Bound::Min(4.7), 5.24, 6.35),
    (90, "gang_leader", Metric::Energy0HWeek, Bound::Max(3.1), 0.11, 1.10),
    (90, "gang_leader", Metric::KnownMetWeek, Bound::Min(2.3), 3.89, 10.78),
    (90, "gang_leader", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "gang_leader", Metric::CommuteAbortsDay, Bound::Max(0.13), 0.00, 0.08),
    (90, "gang_leader", Metric::PaidOfWorked, Bound::Min(0.78), 0.88, 0.88),
    (90, "gang_leader", Metric::WagedWorkdays, Bound::Min(0.76), 0.86, 0.87),
    (90, "gang_member", Metric::WalkHDay, Bound::Max(8.2), 6.03, 7.13),
    (90, "gang_member", Metric::LongestSleepH, Bound::Min(5.0), 5.54, 5.88),
    (90, "gang_member", Metric::Energy0HWeek, Bound::Max(2.3), 0.02, 0.29),
    (90, "gang_member", Metric::KnownMetWeek, Bound::Min(4.3), 7.33, 10.38),
    (90, "gang_member", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "gang_member", Metric::CommuteAbortsDay, Bound::Max(0.05), 0.00, 0.00),
    (90, "homeless", Metric::WalkHDay, Bound::Max(7.3), 5.26, 6.29),
    (90, "homeless", Metric::LongestSleepH, Bound::Min(5.5), 6.07, 6.29),
    (90, "homeless", Metric::Energy0HWeek, Bound::Max(2.7), 0.24, 0.70),
    (90, "homeless", Metric::KnownMetWeek, Bound::Min(6.5), 10.87, 12.49),
    (90, "homeless", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "homeless", Metric::CommuteAbortsDay, Bound::Max(0.08), 0.00, 0.03),
    (90, "homeless", Metric::PaidOfWorked, Bound::Min(0.87), 0.97, 1.00),
    (90, "homeless", Metric::WagedWorkdays, Bound::Min(0.74), 0.84, 0.93),
    (90, "purist", Metric::WalkHDay, Bound::Max(8.1), 4.94, 6.97),
    (90, "purist", Metric::LongestSleepH, Bound::Min(4.7), 5.28, 5.78),
    (90, "purist", Metric::Energy0HWeek, Bound::Max(2.3), 0.00, 0.29),
    (90, "purist", Metric::KnownMetWeek, Bound::Min(6.3), 10.58, 16.61),
    (90, "purist", Metric::RefundsWeek, Bound::Max(1.0), 0.00, 0.00),
    (90, "purist", Metric::CommuteAbortsDay, Bound::Max(0.06), 0.00, 0.01),
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
    let world = start_world(seed, day);
    let jobs: Vec<(&str, Result<Vec<EntityId>, String>)> =
        list.split(',').map(|arch| (arch, sample(&world, seed, arch))).collect();
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
    let mut failures = Vec::new();
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
        let (ok, rule) = match bound {
            Bound::Min(b) => (v >= b, format!(">= {b}")),
            Bound::Max(b) => (v <= b, format!("<= {b}")),
        };
        let line = format!("d{day} {arch} {m:?} {v:.2} {rule} (calibration range {lo:.2}-{hi:.2})");
        let _ = writeln!(text, "{} {line}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures.push(line);
        }
    }
    Ok(BehaviourReport { text, failures })
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
    }
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
        ] {
            assert!(text.contains(needle), "diary lacks {needle:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The behaviour tier (docs/TESTING.md): the archetypes' diary metrics
    /// against `CHECKS`. `#[ignore]` (two windows on three seeds, each
    /// archetype in its own clone: ~1-1.5 min release, ~3 GB):`cargo test --release -p citysim-cli -- --ignored behaviour`,
    /// or `citysim-cli shadow --assert`.
    #[test]
    #[ignore]
    fn test_behaviour_tier() {
        let r = behaviour(None).expect("the behaviour windows run");
        eprint!("{}", r.text);
        assert!(r.failures.is_empty(), "behaviour tier failures: {:?}", r.failures);
    }

    #[test]
    fn test_unknown_archetype_is_an_error_not_a_panic() {
        let world = World::new(42, Config::load());
        assert!(candidates(&world, "astronaut").is_err());
        assert!(candidates(&world, "reporter").is_ok());
    }
}
