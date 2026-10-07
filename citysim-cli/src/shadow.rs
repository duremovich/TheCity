//! `citysim-cli shadow`: follow individual residents through their days.
//!
//! ```text
//! citysim-cli shadow --seed S --days N [--start-day D] [--pick a,b,..]
//!                    [--agent INDEX]... [--count K] --out DIR
//! citysim-cli shadow --seed S --start-day D --list
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
    #[arg(long)]
    pub seed: u64,
    /// Shadowed days (counted from `--start-day`).
    #[arg(long, default_value_t = 7)]
    pub days: u64,
    /// The day the picks are chosen and pinned; the world runs normally until then.
    #[arg(long, default_value_t = 0)]
    pub start_day: u64,
    /// Comma-separated archetypes: gang_member, gang_leader, ripperdoc, homeless, ceo, exec, guard,
    /// worker, runner, purist, reporter, child, dealer.
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
}

const ARCHETYPES: [&str; 13] = [
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
];

/// Activity classes of the time-use table, in print order.
const CLASSES: [&str; 9] = ["sleep", "work", "eat", "social", "travel", "crime", "idle", "jail", "other"];

// ---------------------------------------------------------------------------
// Candidates
// ---------------------------------------------------------------------------

fn adult_alive(world: &World, id: EntityId) -> bool {
    world.has::<Brain>(id) && world.has::<Identity>(id) && !world.has::<Child>(id) && !world.has::<Corpse>(id)
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
        "runner" => adults().filter(|&a| world.comp::<Kit>(a).is_some_and(|k| k.deck.is_some())).collect(),
        "purist" => adults().filter(|&a| world.has::<GangMember>(a) && is_purist(world, a)).collect(),
        "reporter" => return Err("reporter: Role::Reporter does not exist at this commit; skipped".into()),
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
        "dealer" => {
            let today = world.day();
            let recent: BTreeSet<EntityId> =
                world.deal_log.values().filter(|&&(_, d)| d + 3 >= today).map(|&(a, _)| a).collect();
            let hit: Vec<EntityId> = adults().filter(|a| recent.contains(a) && world.has::<GangMember>(*a)).collect();
            if hit.is_empty() {
                adults().filter(|&a| world.has::<GangMember>(a)).collect()
            } else {
                hit
            }
        }
        other => return Err(format!("unknown archetype {other:?} (known: {})", ARCHETYPES.join(", "))),
    };
    out.sort();
    out.dedup();
    Ok(out)
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fisher-Yates keyed on the seed and the archetype name.
fn shuffled(mut v: Vec<EntityId>, seed: u64, arch: &str) -> Vec<EntityId> {
    let mut s =
        seed ^ arch.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    for i in (1..v.len()).rev() {
        let j = (splitmix(&mut s) % (i as u64 + 1)) as usize;
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
        | A::Register => "work",
        A::EatFromInventory | A::EatAtHome | A::BuyFood | A::Forage | A::StoreFood => "eat",
        A::Chat | A::Drink | A::Flirt | A::Propose | A::AskAround => "social",
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
        Some(A::CollectWage) => "wage",
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
    }
    for e in dbg {
        if e.actors.first() != Some(&id) {
            continue;
        }
        // "A -> B": interrupted; "A failed at X: reason".
        let label = if e.text.contains(" failed at ") { "PLAN FAILED" } else { "plan interrupted" };
        t.log(e.tick, "plan", format!("{label}: {}", e.text));
    }
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
    *t.use_by_day.entry(day).or_default().entry(act).or_insert(0) += 1;
    *t.day_places.entry(here.clone()).or_insert(0) += 1;
    *t.total_places.entry(here.clone()).or_insert(0) += 1;

    if let Some(b) = world.comp::<Brain>(id) {
        if !t.pinned_lod_noted && b.lod == Lod::Full {
            t.pinned_lod_noted = true;
            t.log(now, "info", "at Full LOD (pinned)".into());
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
                        "needs hunger {:.2} energy {:.2} safety {:.2} wealth {:.2} belonging {:.2} intimacy {:.2} mood {:.2}{} | {:?} | {:?} | {}",
                        n.hunger, n.energy, n.safety, n.wealth, n.belonging, n.intimacy, m.value, body, b.lod, b.current_goal, here
                    ),
                );
            }
        }
    }

    // Money.
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
                let owner = bld.and_then(|b| owner_name(world, b));
                let cp = owner.map_or(String::new(), |o| format!("; place owner {o}"));
                let ev =
                    if event_ctx.is_empty() { String::new() } else { format!("; events: {}", event_ctx.join(" | ")) };
                t.log(
                    now,
                    "money",
                    format!("{}{} ({kind}), purse {}{cp}{ev}", if d > 0 { "+" } else { "" }, money(d), money(w.coins)),
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
    if !args.list && args.out.is_none() {
        return Err("--out <dir> is required (or use --list)".into());
    }
    let mut world = World::new(args.seed, Config::load());
    let start_tick = args.start_day * TICKS_PER_DAY;
    while world.tick < start_tick {
        citysim::tick(&mut world);
    }

    if args.list {
        println!("seed {} day {}: candidates per archetype", args.seed, args.start_day);
        for a in ARCHETYPES {
            match candidates(&world, a) {
                Ok(v) => println!("  {a:<12} {}", v.len()),
                Err(e) => println!("  {a:<12} n/a ({e})"),
            }
        }
        return Ok(());
    }

    // Picks.
    let mut picked: Vec<(String, EntityId)> = Vec::new();
    let mut taken: BTreeSet<EntityId> = BTreeSet::new();
    if let Some(list) = &args.pick {
        for arch in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match candidates(&world, arch) {
                Err(e) => eprintln!("note: {e}"),
                Ok(c) if c.is_empty() => {
                    eprintln!("note: no {arch} candidates on seed {} at day {}", args.seed, args.start_day)
                }
                Ok(c) => {
                    let ordered = if arch == "ceo" { c } else { shuffled(c, args.seed, arch) };
                    let mut n = 0;
                    for id in ordered {
                        if n >= args.count {
                            break;
                        }
                        if taken.insert(id) {
                            picked.push((arch.to_string(), id));
                            n += 1;
                        }
                    }
                    if n < args.count {
                        eprintln!("note: only {n} distinct {arch} candidate(s) (asked for {})", args.count);
                    }
                }
            }
        }
    }
    for &ix in &args.agents {
        match world.citizens().into_iter().find(|c| c.index == ix) {
            Some(id) if world.has::<Identity>(id) && !world.has::<Corpse>(id) => {
                if taken.insert(id) {
                    picked.push(("agent".into(), id));
                }
            }
            _ => eprintln!("note: agent {ix} is not alive at day {}", args.start_day),
        }
    }
    if picked.is_empty() {
        return Err("nothing to shadow: no --pick candidates and no live --agent".into());
    }
    let dir = args.out.clone().unwrap_or_default();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    world.shadow_notes = Some(Vec::new());
    let mut tracks: Vec<Track> = Vec::new();
    for (arch, id) in &picked {
        let name = world.comp::<Identity>(*id).map_or("?".into(), |i| i.name.clone());
        let mut t = Track::new(*id, arch, name, start_tick);
        t.header = header(&world, &t);
        t.last_place = place(&world, *id);
        eprintln!("shadowing {arch}: {} (#{})", t.name, id.index);
        tracks.push(t);
    }

    let end_tick = start_tick + args.days * TICKS_PER_DAY;
    while world.tick < end_tick {
        for t in &tracks {
            if let Some(b) = world.comp_mut::<Brain>(t.id) {
                b.pinned = true;
            }
        }
        let cursor = world.next_event_id;
        if let Some(n) = world.shadow_notes.as_mut() {
            n.clear();
        }
        citysim::tick(&mut world);
        let now = world.tick - 1;
        let mut events: Vec<&Event> = world.events.iter().rev().take_while(|e| e.id >= cursor).collect();
        events.reverse();
        let dbg: Vec<&Event> = world.debug_events.iter().rev().take_while(|e| e.tick == now).collect();
        let notes: Vec<ShadowNote> = world.shadow_notes.clone().unwrap_or_default();
        for t in &mut tracks {
            observe(&world, t, now, &events, &dbg, &notes);
        }
    }
    let end = world.tick;
    let mut total = 0u64;
    for t in &mut tracks {
        // The last day's summary and any open action.
        let now = end;
        close_action(&world, t, now, "run ended");
        flush_day(t, now);
        let (md, jl) = write_diary(&world, t, &dir, end)?;
        total += md + jl;
        println!("{}_{}: {} ({} KB md, {} KB jsonl)", t.arch, t.id.index, t.name, md / 1024, jl / 1024);
    }
    println!("wrote {} diaries to {} ({} KB)", tracks.len(), dir.display(), total / 1024);
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

    #[test]
    fn test_unknown_archetype_is_an_error_not_a_panic() {
        let world = World::new(42, Config::load());
        assert!(candidates(&world, "astronaut").is_err());
        assert!(candidates(&world, "reporter").is_err());
    }
}
