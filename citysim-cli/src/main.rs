//! Headless runner and calibration.
//!
//! ```text
//! citysim-cli run --days N --seed S [--speed 1000] [--report]
//!                 [--lever "day=90:release_reserve=1500"]... [--save-at T]
//!                 [--load FILE] [--force-lod full|coarse|stat] [--population N]
//!                 [--map FILE]
//! citysim-cli calibrate [--days 30] [--agents 500] [--seeds 3] [--map assets/map.txt]
//!                       [--out assets/stat_table.toml] [--straight-lines]
//! ```
//!
//! `--map` overrides `[world] map` (the M10 256 x 192 map by default).
//! `calibrate` runs the city scaled to `--agents` (`Config::scaled_to`) with
//! no gangs, so a 500-agent world on the 2,000 map keeps its proportions, and
//! writes the v2 table (M10): 24 rows by phase, lawfulness and hunger bucket,
//! with the hourly outcome mix and the actor- and victim-side crime rolls.
//!
//! Exit code 0 on completion, 101 on panic.

use std::path::PathBuf;
use std::time::Instant;

mod shadow;

use clap::{Parser, Subcommand, ValueEnum};

use citysim::{save, stats, Config, Lod, PlayerCommand, Posture, World, TICKS_PER_DAY};

#[derive(Parser)]
#[command(name = "citysim-cli", about = "Living City Simulator headless runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a city headless for N days.
    Run(RunArgs),
    /// Calibrate the Statistical-LOD table (M7).
    Calibrate(CalibrateArgs),
    /// Follow individual residents through their days (diaries).
    Shadow(shadow::ShadowArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    /// In-game days to simulate.
    #[arg(long)]
    days: u64,
    /// World seed.
    #[arg(long)]
    seed: u64,
    /// Accepted for parity with the app; headless runs always go flat out.
    #[arg(long, default_value_t = 1000)]
    speed: u32,
    /// Print DailyStats as CSV, one row per day, header first.
    #[arg(long)]
    report: bool,
    /// Print every event as `tick<TAB>kind<TAB>text` to stderr.
    #[arg(long)]
    events: bool,
    /// With `--events`: a daily `Diag` line per Market (owner, price in
    /// tenths, yesterday's sales, stock) and per corp (treasury, closing,
    /// buildings, niches, levels, order, sold contracts).
    #[arg(long)]
    diag: bool,
    /// Schedule a lever: `day=<D>:<lever>=<value>`. Repeatable.
    #[arg(long = "lever", value_name = "SPEC")]
    levers: Vec<String>,
    /// Write `saves/<seed>-<tick>.ron` when this tick is reached.
    #[arg(long, value_name = "TICK")]
    save_at: Option<u64>,
    /// Resume from a save instead of a fresh world.
    #[arg(long, value_name = "FILE")]
    load: Option<PathBuf>,
    /// Force every agent to one LOD.
    #[arg(long, value_enum)]
    force_lod: Option<ForceLod>,
    /// Directory for `--save-at`.
    #[arg(long, default_value = "saves")]
    saves_dir: PathBuf,
    /// Starting population, overriding the config (ignored with `--load`).
    #[arg(long)]
    population: Option<u32>,
    /// Map file, overriding `[world] map` (ignored with `--load`).
    #[arg(long, value_name = "FILE")]
    map: Option<PathBuf>,
    /// Run the 300-resident v1 city (`Config::v1_profile`) instead of the
    /// configured one (ignored with `--load`).
    #[arg(long)]
    v1_profile: bool,
    /// The Statistical tier's policy, overriding `[lod] policy`: `table` or
    /// `mlp` (experiment; ignored with `--load`).
    #[arg(long, value_name = "POLICY")]
    stat_policy: Option<String>,
    /// M14 V46: every M14 section off (`Config::virt_off`): no plane, Labs,
    /// ICE or tech caps; the M13 city.
    #[arg(long)]
    virt_off: bool,
    /// M15 W46: every M15 section off (`Config::word_off`): no pools,
    /// exchange, hearing, kin channel or reputation; the M14 city.
    #[arg(long)]
    word_off: bool,
}

/// An absolute map path for `config.world.map` (`Config::asset` joins it onto
/// the assets directory, which an absolute path replaces).
fn map_path(p: &std::path::Path) -> Result<String, String> {
    std::fs::canonicalize(p)
        .map(|a| a.to_string_lossy().into_owned())
        .map_err(|e| format!("--map {}: {e}", p.display()))
}

#[derive(Copy, Clone, ValueEnum)]
enum ForceLod {
    Full,
    Coarse,
    Stat,
}

impl From<ForceLod> for Lod {
    fn from(f: ForceLod) -> Lod {
        match f {
            ForceLod::Full => Lod::Full,
            ForceLod::Coarse => Lod::Coarse,
            ForceLod::Stat => Lod::Statistical,
        }
    }
}

#[derive(clap::Args)]
struct CalibrateArgs {
    #[arg(long, default_value_t = 30)]
    days: u64,
    #[arg(long, default_value_t = 500)]
    agents: u32,
    #[arg(long, default_value = "assets/map.txt")]
    map: PathBuf,
    #[arg(long, default_value = "assets/stat_table.toml")]
    out: PathBuf,
    /// Cities run (seeds 1000, 1001, ...), tallied together.
    #[arg(long, default_value_t = 3)]
    seeds: u64,
    /// The first seed (`--rows-csv` runs split the seeds over processes).
    #[arg(long, default_value_t = 1000)]
    first_seed: u64,
    /// Print each city's day-end health (price, Treasury, stocks, thefts).
    #[arg(long)]
    health: bool,
    /// Timed straight-line walks (the M7 shortcut). Off by default since M10:
    /// on the 256 x 192 map they starve the city and cut its crime threefold.
    #[arg(long)]
    straight_lines: bool,
    /// Also write one CSV row per Full agent-hour (the extended features at
    /// the hour's start, the outcome and the rolls that fired): the training
    /// data of the learned-policy experiment (`systems::stat_policy`).
    #[arg(long, value_name = "FILE")]
    rows_csv: Option<PathBuf>,
}

/// A scheduled lever: a ready command, or a god command naming a gang by its
/// index in `World::gangs()`, resolved to entity ids when it fires (the
/// command log records the resolved command, so replays need no resolving).
#[derive(Clone, Debug)]
enum Lever {
    Cmd(PlayerCommand),
    KillLeader(usize),
    JailGang(usize, u32),
    KillGang(usize),
    FundGang(usize, i64),
    SeizeGang(usize),
    /// M11: `breakup=<corp slot>` (the seeding row, 0-based).
    BreakUp(u8),
    // M11 god levers, a corp by its seeding slot (docs/GOD_SCENARIOS_V2.md).
    FundCorp(u8, i64),
    BankruptCorp(u8),
    SeizeCorp(u8, SeizeTo),
    KillExec(u8),
    KillStaff(u8),
    CorpOrder(u8, citysim::CorpOrder, Option<citysim::Niche>, u32),
    Strike(u8),
    /// M12: `stance=<district>:crackdown<gang index>`.
    StanceCrackdown(u8, usize),
    /// M12 god: `split_gang=<gang index>`.
    SplitGang(usize),
    /// M12 god: `derelict=<building index>`.
    Derelict(u32),
    /// M12 god: `buy_building=<buyer>:<building index>:<price>`.
    BuyBuilding(Buyer, u32, i64),
    /// M13 god: `grant_asset=<agent index>:<kind>:<tier>`.
    GrantAsset(u32, citysim::AssetKind, u8),
    /// M13 god: `wreck=<asset index>`.
    Wreck(u32),
    /// M13 god: `brick=<corp slot>` or `brick=agent:<index>`.
    Brick(Lender),
    /// M13 god: `chase=<agent index>`.
    Chase(u32),
    /// M14 god: `wipe_data=<building index>`.
    WipeData(u32),
    /// M14 god: `set_tech=<corp slot>:<track>:<tier>`.
    SetTech(u8, citysim::virt::Track, u8),
    /// M14 god: `grant_data=<corp slot>|gang<i>:<track>:<units>`.
    GrantData(Faction, citysim::virt::Track, u32),
    /// M14 god: `grant_deck=<agent index>:<tier>`.
    GrantDeck(u32, u8),
    /// M14 god: `set_ice=<building index>:<tier>`.
    SetIce(u32, u8),
    /// M14 god: `fry=<agent index>`.
    Fry(u32),
    /// M14 god: `run_now=<agent index>:<building index|corp<slot>>:<data|wipe|ledger|door>`.
    RunNow(u32, RunTarget, citysim::virt::Purpose),
    /// M14 god (phase 5): `wipe_corp_data=<corp slot>`.
    WipeCorpData(u8),
    /// M14 god (phase 5): `grant_decks=<district>:<n>:<tier>`.
    GrantDecks(u8, u16, u8),
    /// M14 god (phase 5): `set_corp_ice=<corp slot>:<tier>`.
    SetCorpIce(u8, u8),
    /// M15 god (W42): `plant_rumour=<agent>:<deed>:<object agent|none>:<district>:<reach>`.
    PlantRumour(u32, citysim::word::Deed, Option<u32>, u8, f32),
    /// M15 god (W42): `set_reputation=<agent>:<dread|standing|honour|heat>:<v>:<days>`.
    SetReputation(u32, citysim::word::Axis, f32, u16),
    /// M15 god (W42, phase 2): `grant_skill=<agent>:<skill>:<v>` or
    /// `grant_skill=dregs<n>:<skill>:<v>[:suit]` (the n lowest-wealth Dregs).
    GrantSkill(SkillTarget, citysim::word::SocialSkill, f32, bool),
    /// M15 god (W42, phase 2): `set_creed=<gang i>:purist|none`.
    SetCreed(usize, Option<citysim::word::Creed>),
}

/// Who `grant_skill=` names: an agent by index, or the n poorest Dregs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SkillTarget {
    Agent(u32),
    Dregs(u32),
}

/// The target of `run_now`: a building by entity index, or a corp by slot
/// (for a `ledger` run).
#[derive(Clone, Copy, Debug)]
enum RunTarget {
    Building(u32),
    Corp(u8),
}

/// A corp by its seeding slot or a gang by its index (`grant_data`).
#[derive(Clone, Copy, Debug)]
enum Faction {
    Corp(u8),
    Gang(usize),
}

/// Whose financed implants `brick` bricks: a corp by its seeding slot or an agent.
#[derive(Clone, Copy, Debug)]
enum Lender {
    Corp(u8),
    Agent(u32),
}

/// The living agent with this entity index.
fn agent_at(world: &World, index: u32) -> Result<citysim::EntityId, String> {
    world.citizens().into_iter().find(|c| c.index == index).ok_or_else(|| format!("no agent {index}"))
}

/// `moto|car|truck|flyer|arms|legs|nerves|eyes|skin|robot|pack|bridge`.
fn parse_asset_kind(s: &str) -> Option<citysim::AssetKind> {
    use citysim::{AssetKind, Slot};
    Some(match s.to_ascii_lowercase().as_str() {
        "moto" | "motorcycle" | "bike" => AssetKind::Motorcycle,
        "car" => AssetKind::Car,
        "truck" => AssetKind::Truck,
        "flyer" => AssetKind::Flyer,
        "arms" => AssetKind::Implant(Slot::Arms),
        "legs" => AssetKind::Implant(Slot::Legs),
        "nerves" => AssetKind::Implant(Slot::Nerves),
        "eyes" => AssetKind::Implant(Slot::Eyes),
        "skin" => AssetKind::Implant(Slot::Skin),
        "robot" => AssetKind::Robot,
        "pack" => AssetKind::Pack,
        "bridge" => AssetKind::Bridge,
        _ => return None,
    })
}

/// `moto|car|truck|flyer|implant|robot|pack|bridge` (`chrome` = implant).
fn parse_asset_class(s: &str) -> Option<citysim::AssetClass> {
    use citysim::AssetClass;
    Some(match s.to_ascii_lowercase().as_str() {
        "moto" | "motorcycle" | "bike" => AssetClass::Motorcycle,
        "car" => AssetClass::Car,
        "truck" => AssetClass::Truck,
        "flyer" => AssetClass::Flyer,
        "implant" | "chrome" => AssetClass::Implant,
        "robot" => AssetClass::Robot,
        "pack" => AssetClass::Pack,
        "bridge" => AssetClass::Bridge,
        _ => return None,
    })
}

/// `on|off` (also `1|0`, `true|false`).
fn on_off(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" => Some(true),
        "off" | "0" | "false" => Some(false),
        _ => None,
    }
}

/// Who `buy_building` buys for: `city`, `gang<i>`, `corp<slot>` or an agent's entity index.
#[derive(Clone, Copy, Debug)]
enum Buyer {
    City,
    Gang(usize),
    Corp(u8),
    Agent(u32),
}

/// The live building with this entity index.
fn building_at(world: &World, index: u32) -> Result<citysim::EntityId, String> {
    world
        .with::<citysim::Building>()
        .into_iter()
        .find(|b| b.index == index)
        .ok_or_else(|| format!("no building {index}"))
}

/// Who `seize_corp` hands the buildings to.
#[derive(Clone, Copy, Debug)]
enum SeizeTo {
    Corp(u8),
    City,
    Gang(usize),
}

/// The corp seeded in `slot`, if it still exists.
fn corp_in_slot(world: &World, slot: u8) -> Result<citysim::EntityId, String> {
    world
        .corps()
        .into_iter()
        .find(|&c| world.comp::<citysim::Corp>(c).is_some_and(|cc| cc.slot == Some(slot)))
        .ok_or_else(|| format!("no corp in slot {slot}"))
}

impl Lever {
    /// Every command a lever stands for (one, or a `dregs<n>` form's n).
    fn resolve_all(&self, world: &World) -> Result<Vec<PlayerCommand>, String> {
        match *self {
            Lever::GrantSkill(SkillTarget::Dregs(n), skill, value, suit) => {
                // The n lowest-wealth living Dreg adults (no Home), ties the lower index.
                let mut dregs: Vec<(i64, citysim::EntityId)> = world
                    .citizens()
                    .into_iter()
                    .filter(|&a| citysim::systems::law::living(world, a))
                    .filter(|&a| citysim::systems::demography::is_adult(world, a))
                    .filter(|&a| citysim::systems::classes::class_of(world, a) == citysim::components::Class::Dreg)
                    .map(|a| (world.comp::<citysim::components::Wallet>(a).map_or(0, |w| w.coins), a))
                    .collect();
                dregs.sort();
                if dregs.is_empty() {
                    return Err("no Dregs".into());
                }
                Ok(dregs
                    .into_iter()
                    .take(n as usize)
                    .map(|(_, agent)| PlayerCommand::GrantSkill { agent, skill, value, suit })
                    .collect())
            }
            _ => self.resolve(world).map(|c| vec![c]),
        }
    }

    fn resolve(&self, world: &World) -> Result<PlayerCommand, String> {
        let gang = |i: usize| world.gangs().get(i).copied().ok_or_else(|| format!("no gang {i}"));
        Ok(match *self {
            Lever::Cmd(ref c) => c.clone(),
            Lever::KillLeader(i) => {
                let g = gang(i)?;
                let leader = world.comp::<citysim::Gang>(g).and_then(|g| g.leader);
                PlayerCommand::KillAgent(leader.ok_or_else(|| format!("gang {i} has no leader"))?)
            }
            Lever::JailGang(i, days) => PlayerCommand::JailGang { gang: gang(i)?, days },
            Lever::KillGang(i) => PlayerCommand::KillGang(gang(i)?),
            Lever::FundGang(i, amount) => PlayerCommand::FundGang { gang: gang(i)?, amount },
            Lever::SeizeGang(i) => PlayerCommand::SeizeGangTreasury(gang(i)?),
            Lever::BreakUp(slot) => PlayerCommand::BreakUp(corp_in_slot(world, slot)?),
            Lever::FundCorp(slot, amount) => PlayerCommand::FundCorp { corp: corp_in_slot(world, slot)?, amount },
            Lever::BankruptCorp(slot) => PlayerCommand::BankruptCorp(corp_in_slot(world, slot)?),
            Lever::SeizeCorp(slot, to) => PlayerCommand::SeizeBuildings {
                owner: Some(corp_in_slot(world, slot)?),
                to: match to {
                    SeizeTo::Corp(s) => Some(corp_in_slot(world, s)?),
                    SeizeTo::City => None,
                    SeizeTo::Gang(i) => Some(gang(i)?),
                },
            },
            Lever::KillExec(slot) => PlayerCommand::KillExec(corp_in_slot(world, slot)?),
            Lever::KillStaff(slot) => PlayerCommand::KillStaff(corp_in_slot(world, slot)?),
            Lever::CorpOrder(slot, order, niche, days) => {
                PlayerCommand::SetCorpOrder { corp: corp_in_slot(world, slot)?, order, niche, days }
            }
            Lever::Strike(slot) => PlayerCommand::StrikeNow(corp_in_slot(world, slot)?),
            Lever::StanceCrackdown(d, i) => PlayerCommand::SetStance {
                district: citysim::DistrictId(d),
                stance: Some(citysim::Stance::Crackdown(gang(i)?)),
            },
            Lever::SplitGang(i) => PlayerCommand::SplitGang(gang(i)?),
            Lever::Derelict(b) => PlayerCommand::Derelict(building_at(world, b)?),
            Lever::BuyBuilding(buyer, b, price) => PlayerCommand::BuyBuilding {
                buyer: match buyer {
                    Buyer::City => None,
                    Buyer::Gang(i) => Some(gang(i)?),
                    Buyer::Corp(s) => Some(corp_in_slot(world, s)?),
                    Buyer::Agent(a) => Some(
                        world.citizens().into_iter().find(|c| c.index == a).ok_or_else(|| format!("no agent {a}"))?,
                    ),
                },
                building: building_at(world, b)?,
                price,
            },
            Lever::GrantAsset(a, kind, tier) => PlayerCommand::GrantAsset { agent: agent_at(world, a)?, kind, tier },
            Lever::Wreck(i) => PlayerCommand::Wreck(
                world
                    .with::<citysim::Asset>()
                    .into_iter()
                    .find(|a| a.index == i)
                    .ok_or_else(|| format!("no asset {i}"))?,
            ),
            Lever::Brick(Lender::Corp(s)) => PlayerCommand::Brick(corp_in_slot(world, s)?),
            Lever::Brick(Lender::Agent(a)) => PlayerCommand::Brick(agent_at(world, a)?),
            Lever::Chase(a) => PlayerCommand::Chase(agent_at(world, a)?),
            Lever::WipeData(b) => PlayerCommand::WipeData(building_at(world, b)?),
            Lever::SetTech(s, track, tier) => PlayerCommand::SetTech { corp: corp_in_slot(world, s)?, track, tier },
            Lever::GrantDeck(a, tier) => PlayerCommand::GrantDeck { agent: agent_at(world, a)?, tier },
            Lever::SetIce(b, tier) => PlayerCommand::SetIce { building: building_at(world, b)?, tier },
            Lever::Fry(a) => PlayerCommand::Fry(agent_at(world, a)?),
            Lever::WipeCorpData(s) => PlayerCommand::WipeCorpData(corp_in_slot(world, s)?),
            Lever::GrantDecks(d, n, tier) => PlayerCommand::GrantDecks { district: citysim::DistrictId(d), n, tier },
            Lever::SetCorpIce(s, tier) => PlayerCommand::SetCorpIce { corp: corp_in_slot(world, s)?, tier },
            Lever::PlantRumour(a, deed, o, d, reach) => PlayerCommand::PlantRumour {
                about: agent_at(world, a)?,
                deed,
                object: match o {
                    Some(o) => Some(agent_at(world, o)?),
                    None => None,
                },
                district: citysim::DistrictId(d),
                reach,
            },
            Lever::SetReputation(a, axis, value, days) => {
                PlayerCommand::SetReputation { who: agent_at(world, a)?, axis, value, days }
            }
            Lever::GrantSkill(SkillTarget::Agent(a), skill, value, suit) => {
                PlayerCommand::GrantSkill { agent: agent_at(world, a)?, skill, value, suit }
            }
            Lever::GrantSkill(SkillTarget::Dregs(_), ..) => return Err("dregs<n> resolves to many".into()),
            Lever::SetCreed(i, creed) => PlayerCommand::SetCreed { gang: gang(i)?, creed },
            Lever::RunNow(a, t, purpose) => PlayerCommand::RunNow {
                agent: agent_at(world, a)?,
                target: match t {
                    RunTarget::Building(b) => building_at(world, b)?,
                    RunTarget::Corp(s) => corp_in_slot(world, s)?,
                },
                purpose,
            },
            Lever::GrantData(f, track, units) => PlayerCommand::GrantData {
                faction: match f {
                    Faction::Corp(s) => corp_in_slot(world, s)?,
                    Faction::Gang(i) => gang(i)?,
                },
                track,
                units,
            },
        })
    }
}

/// `day=90:release_reserve=1500` → `(tick, lever)`. God levers name a gang
/// by index: `kill_leader=0`, `jail_gang=0:60`, `kill_gang=0`,
/// `fund_gang=1:10000`, `seize_gang=0`, `fire_guards=1`, `treasury=-50000`;
/// M11 names a corp by its seeding slot: `breakup=0`, and the corp god levers
/// `fund_corp=0:200000`, `bankrupt_corp=0`, `seize_corp=3:gang0` (or
/// `:city`, `:<slot>`), `kill_exec=0`, `kill_staff=0`,
/// `corp_order=3:Squeeze:30` (or `1:Squeeze:Housing:30`), `strike=0`,
/// `wipe_corps=1`. M12 (plan D42): `guard_weight=<district>:<weight>`,
/// `stance=<district>:sweep|patrol|cordon|withdrawn|crackdown<gang index>|auto`,
/// `sanitation=12`, `sanitation_weight=<district>:<weight>`,
/// `curfew=<district>:on|off`, `riot_response=crush|disperse|contain|auto`;
/// M12 god levers `riot=<district>`, `litter=<district>:<level 0..1>`,
/// `split_gang=<gang index>`, `derelict=<building index>`,
/// `buy_building=<city|gang<i>|corp<slot>|agent index>:<building index>:<price>`.
/// M13 (plan D48): `stims_legal=on|off`, `asset_tax=car:0.5` (a class:
/// `moto|car|truck|flyer|implant|robot|pack|bridge`), `impound=on|off`;
/// M13 god levers `grant_asset=<agent index>:<kind>:<tier>` (kinds
/// `moto|car|truck|flyer|arms|legs|nerves|eyes|skin|robot|pack|bridge`),
/// `wreck=<asset index>`, `chrome_everyone=<tier>`,
/// `flood_stims=<district>:<n>`, `brick=<corp slot>|agent:<index>`,
/// `chase=<agent index>`. M14 god levers (plan V42, phase 1):
/// `wipe_data=<building index>`, `set_tech=<corp slot>:<chrome|deck|industry>:<tier>`,
/// `grant_data=<corp slot>|gang<i>:<chrome|deck|industry>:<units>`. M14
/// levers (phase 4): `city_ice=<0..3>` (the Treasury's and Precinct's ICE),
/// `data_tax=<0..1>` (an extra share of Data sales to the Treasury),
/// `hack_sentence=intrusion|data_theft:<days>`; god `grant_deck=<agent
/// index>:<tier>`, `set_ice=<building index>:<tier>`, `fry=<agent index>`,
/// `run_now=<agent index>:<building index|corp<slot>>:<data|wipe|ledger|door>`;
/// phase 5's plurals `wipe_corp_data=<corp slot>`,
/// `grant_decks=<district>:<n>:<tier>`, `set_corp_ice=<corp slot>:<tier>`.
/// M15 god levers (plan W42, phase 1):
/// `plant_rumour=<agent>:<deed>:<object agent|none>:<district>:<reach>`,
/// `set_reputation=<agent>:<dread|standing|honour|heat>:<value>:<days>`;
/// phase 2: `grant_skill=<agent>:<skill>:<v>`,
/// `grant_skill=dregs<n>:<skill>:<v>:suit`, `set_creed=<gang i>:purist|none`.
fn parse_lever(spec: &str) -> Result<(u64, Lever), String> {
    let (day_part, cmd_part) =
        spec.split_once(':').ok_or_else(|| format!("{spec}: expected day=<D>:<lever>=<value>"))?;
    let day: u64 = day_part
        .strip_prefix("day=")
        .ok_or_else(|| format!("{spec}: expected day=<D>"))?
        .parse()
        .map_err(|e| format!("{spec}: bad day: {e}"))?;
    let (name, value) = cmd_part.split_once('=').ok_or_else(|| format!("{spec}: expected <lever>=<value>"))?;
    let num = |what: &str| value.parse::<f64>().map_err(|e| format!("{spec}: bad {what}: {e}"));
    let idx = |v: &str| v.parse::<usize>().map_err(|e| format!("{spec}: bad gang index: {e}"));
    let slot = |v: &str| v.parse::<u8>().map_err(|e| format!("{spec}: bad corp slot: {e}"));
    // `<gang index>:<n>`
    let pair = || -> Result<(usize, i64), String> {
        let (g, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <gang index>:<n>"))?;
        Ok((idx(g)?, n.parse::<i64>().map_err(|e| format!("{spec}: bad number: {e}"))?))
    };
    let god = match name {
        "kill_leader" => Some(Lever::KillLeader(idx(value)?)),
        "kill_gang" => Some(Lever::KillGang(idx(value)?)),
        "seize_gang" => Some(Lever::SeizeGang(idx(value)?)),
        "jail_gang" => {
            let (g, d) = pair()?;
            Some(Lever::JailGang(g, u32::try_from(d).map_err(|e| format!("{spec}: bad days: {e}"))?))
        }
        "fund_gang" => {
            let (g, n) = pair()?;
            Some(Lever::FundGang(g, n))
        }
        "breakup" => Some(Lever::BreakUp(slot(value)?)),
        "fund_corp" => {
            let (s, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <corp slot>:<coins>"))?;
            Some(Lever::FundCorp(slot(s)?, n.parse::<i64>().map_err(|e| format!("{spec}: bad coins: {e}"))?))
        }
        "bankrupt_corp" => Some(Lever::BankruptCorp(slot(value)?)),
        "seize_corp" => {
            let (s, to) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <corp slot>:<to>"))?;
            let to = match to {
                "city" => SeizeTo::City,
                t if t.starts_with("gang") => SeizeTo::Gang(idx(&t[4..])?),
                t => SeizeTo::Corp(slot(t)?),
            };
            Some(Lever::SeizeCorp(slot(s)?, to))
        }
        "stance" => {
            let (d, st) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<stance>"))?;
            let d = d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?;
            let st = st.to_ascii_lowercase();
            match st.strip_prefix("crackdown") {
                Some(g) => Some(Lever::StanceCrackdown(d, idx(g)?)),
                None => {
                    let stance = match st.as_str() {
                        "auto" => None,
                        "patrol" => Some(citysim::Stance::Patrol),
                        "sweep" => Some(citysim::Stance::Sweep),
                        "cordon" => Some(citysim::Stance::Cordon),
                        "withdrawn" => Some(citysim::Stance::Withdrawn),
                        _ => {
                            return Err(format!(
                                "{spec}: stance must be auto|patrol|sweep|cordon|withdrawn|crackdown<gang index>"
                            ))
                        }
                    };
                    Some(Lever::Cmd(PlayerCommand::SetStance { district: citysim::DistrictId(d), stance }))
                }
            }
        }
        "split_gang" => Some(Lever::SplitGang(idx(value)?)),
        "derelict" => Some(Lever::Derelict(value.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?)),
        "buy_building" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [who, b, price] = parts[..] else {
                return Err(format!("{spec}: expected <buyer>:<building index>:<price>"));
            };
            let buyer = match who {
                "city" => Buyer::City,
                w if w.starts_with("gang") => Buyer::Gang(idx(&w[4..])?),
                w if w.starts_with("corp") => Buyer::Corp(slot(&w[4..])?),
                w => Buyer::Agent(w.parse::<u32>().map_err(|e| format!("{spec}: bad buyer: {e}"))?),
            };
            Some(Lever::BuyBuilding(
                buyer,
                b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?,
                price.parse::<i64>().map_err(|e| format!("{spec}: bad price: {e}"))?,
            ))
        }
        "grant_asset" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, k, t] = parts[..] else {
                return Err(format!("{spec}: expected <agent index>:<kind>:<tier>"));
            };
            Some(Lever::GrantAsset(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                parse_asset_kind(k).ok_or_else(|| format!("{spec}: unknown asset kind {k}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "wreck" => Some(Lever::Wreck(value.parse::<u32>().map_err(|e| format!("{spec}: bad asset: {e}"))?)),
        "brick" => Some(Lever::Brick(match value.strip_prefix("agent:") {
            Some(a) => Lender::Agent(a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?),
            None => Lender::Corp(slot(value)?),
        })),
        "chase" => Some(Lever::Chase(value.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?)),
        "wipe_data" => Some(Lever::WipeData(value.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?)),
        "set_tech" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [s, t, n] = parts[..] else {
                return Err(format!("{spec}: expected <corp slot>:<track>:<tier>"));
            };
            Some(Lever::SetTech(
                slot(s)?,
                citysim::virt::Track::parse(t).ok_or_else(|| format!("{spec}: unknown track {t}"))?,
                n.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "grant_data" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [f, t, n] = parts[..] else {
                return Err(format!("{spec}: expected <corp slot>|gang<i>:<track>:<units>"));
            };
            let faction = match f.strip_prefix("gang") {
                Some(g) => Faction::Gang(idx(g)?),
                None => Faction::Corp(slot(f)?),
            };
            Some(Lever::GrantData(
                faction,
                citysim::virt::Track::parse(t).ok_or_else(|| format!("{spec}: unknown track {t}"))?,
                n.parse::<u32>().map_err(|e| format!("{spec}: bad units: {e}"))?,
            ))
        }
        "grant_deck" => {
            let (a, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <agent index>:<tier>"))?;
            Some(Lever::GrantDeck(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "set_ice" => {
            let (b, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <building index>:<tier>"))?;
            Some(Lever::SetIce(
                b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "fry" => Some(Lever::Fry(value.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?)),
        "wipe_corp_data" => Some(Lever::WipeCorpData(slot(value)?)),
        "grant_decks" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [d, n, t] = parts[..] else {
                return Err(format!("{spec}: expected <district>:<n>:<tier>"));
            };
            Some(Lever::GrantDecks(
                d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
                n.parse::<u16>().map_err(|e| format!("{spec}: bad count: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "set_corp_ice" => {
            let (s, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <corp slot>:<tier>"))?;
            Some(Lever::SetCorpIce(slot(s)?, t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?))
        }
        "run_now" => {
            use citysim::virt::Purpose;
            let parts: Vec<&str> = value.split(':').collect();
            let [a, t, p] = parts[..] else {
                return Err(format!("{spec}: expected <agent index>:<building index|corp<slot>>:<purpose>"));
            };
            let target = match t.strip_prefix("corp") {
                Some(s) => RunTarget::Corp(slot(s)?),
                None => RunTarget::Building(t.parse::<u32>().map_err(|e| format!("{spec}: bad target: {e}"))?),
            };
            let purpose = match p.to_ascii_lowercase().as_str() {
                "data" => Purpose::Data { wipe: false },
                "wipe" => Purpose::Data { wipe: true },
                "ledger" => Purpose::Ledger,
                "door" => Purpose::Door,
                _ => return Err(format!("{spec}: purpose must be data|wipe|ledger|door")),
            };
            Some(Lever::RunNow(a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?, target, purpose))
        }
        "plant_rumour" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, d, o, di, r] = parts[..] else {
                return Err(format!("{spec}: expected <agent>:<deed>:<object agent|none>:<district>:<reach>"));
            };
            let deed = citysim::word::Deed::parse(d).ok_or_else(|| format!("{spec}: unknown deed {d}"))?;
            let object = if o.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(o.parse::<u32>().map_err(|e| format!("{spec}: bad object: {e}"))?)
            };
            Some(Lever::PlantRumour(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                deed,
                object,
                di.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
                r.parse::<f32>().map_err(|e| format!("{spec}: bad reach: {e}"))?,
            ))
        }
        "set_reputation" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, x, v, d] = parts[..] else {
                return Err(format!("{spec}: expected <agent>:<dread|standing|honour|heat>:<v>:<days>"));
            };
            let axis = citysim::word::Axis::parse(x).ok_or_else(|| format!("{spec}: unknown axis {x}"))?;
            Some(Lever::SetReputation(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                axis,
                v.parse::<f32>().map_err(|e| format!("{spec}: bad value: {e}"))?,
                d.parse::<u16>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            ))
        }
        "grant_skill" => {
            let parts: Vec<&str> = value.split(':').collect();
            let (who, sk, v, suit) = match parts[..] {
                [w, s, v] => (w, s, v, false),
                [w, s, v, "suit"] => (w, s, v, true),
                _ => return Err(format!("{spec}: expected <agent|dregs<n>>:<skill>:<v>[:suit]")),
            };
            let skill = citysim::word::SocialSkill::parse(sk).ok_or_else(|| format!("{spec}: unknown skill {sk}"))?;
            let target = match who.strip_prefix("dregs") {
                Some(n) => SkillTarget::Dregs(n.parse::<u32>().map_err(|e| format!("{spec}: bad dregs count: {e}"))?),
                None => SkillTarget::Agent(who.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?),
            };
            if suit && matches!(target, SkillTarget::Agent(_)) {
                return Err(format!("{spec}: :suit goes with dregs<n>"));
            }
            Some(Lever::GrantSkill(
                target,
                skill,
                v.parse::<f32>().map_err(|e| format!("{spec}: bad value: {e}"))?,
                suit,
            ))
        }
        "set_creed" => {
            let (g, c) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <gang>:purist|none"))?;
            let creed = match c.to_ascii_lowercase().as_str() {
                "purist" => Some(citysim::word::Creed::Purist),
                "none" => None,
                _ => return Err(format!("{spec}: unknown creed {c}")),
            };
            Some(Lever::SetCreed(g.parse::<usize>().map_err(|e| format!("{spec}: bad gang: {e}"))?, creed))
        }
        "kill_exec" => Some(Lever::KillExec(slot(value)?)),
        "kill_staff" => Some(Lever::KillStaff(slot(value)?)),
        "strike" => Some(Lever::Strike(slot(value)?)),
        // `<slot>:<Order>:<days>` or `<slot>:<Order>:<Niche>:<days>`.
        "corp_order" => {
            let parts: Vec<&str> = value.split(':').collect();
            let (s, o, n, d) = match parts[..] {
                [s, o, d] => (s, o, None, d),
                [s, o, n, d] => (s, o, Some(n), d),
                _ => return Err(format!("{spec}: expected <corp slot>:<Order>[:<Niche>]:<days>")),
            };
            let order = citysim::CorpOrder::ALL
                .into_iter()
                .find(|x| x.to_string().eq_ignore_ascii_case(o))
                .ok_or_else(|| format!("{spec}: unknown corp order {o}"))?;
            let niche = match n {
                Some(n) => Some(
                    citysim::Niche::ALL
                        .into_iter()
                        .find(|x| x.label().eq_ignore_ascii_case(n))
                        .ok_or_else(|| format!("{spec}: unknown niche {n}"))?,
                ),
                None => None,
            };
            Some(Lever::CorpOrder(
                slot(s)?,
                order,
                niche,
                d.parse::<u32>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            ))
        }
        _ => None,
    };
    if let Some(l) = god {
        return Ok((day * TICKS_PER_DAY, l));
    }
    let cmd = match name {
        "release_reserve" => PlayerCommand::ReleaseReserve { amount: num("amount")? as u32 },
        "tax_rate" => PlayerCommand::SetTaxRate(num("rate")? as f32),
        "sentence_mult" => PlayerCommand::SetSentenceMult(num("multiplier")? as f32),
        "guard_count" => PlayerCommand::SetGuardCount(num("count")? as u8),
        "immigration_per_week" => PlayerCommand::SetImmigrationPerWeek(num("count")? as u8),
        "dole_per_day" => PlayerCommand::SetDolePerDay(num("coins")? as u8),
        "law_posture" => PlayerCommand::SetLawPosture(match value.to_ascii_lowercase().as_str() {
            "auto" => None,
            "patrol" => Some(Posture::Patrol),
            "crackdown" => Some(Posture::Crackdown),
            "garrison" => Some(Posture::Garrison),
            _ => return Err(format!("{spec}: law_posture must be Auto|Patrol|Crackdown|Garrison")),
        }),
        "fire_guards" => PlayerCommand::FireAllGuards,
        "guard_weight" => {
            let (d, wt) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<weight>"))?;
            PlayerCommand::SetGuardWeight {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                weight: wt.parse::<f32>().map_err(|e| format!("{spec}: bad weight: {e}"))?,
            }
        }
        "sanitation" => PlayerCommand::SetSanitation(num("count")?.clamp(0.0, 255.0) as u8),
        "sanitation_weight" => {
            let (d, wt) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<weight>"))?;
            PlayerCommand::SetSanitationWeight {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                weight: wt.parse::<f32>().map_err(|e| format!("{spec}: bad weight: {e}"))?,
            }
        }
        "curfew" => {
            let (d, on) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:on|off"))?;
            let on = match on.to_ascii_lowercase().as_str() {
                "on" | "1" | "true" => true,
                "off" | "0" | "false" => false,
                _ => return Err(format!("{spec}: curfew must be on|off")),
            };
            PlayerCommand::SetCurfew {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                on,
            }
        }
        // M13 D48: `stims_legal=on|off`, `asset_tax=car:0.5`, `impound=on|off`,
        // god `chrome_everyone=<tier>`, `flood_stims=<district>:<n>`.
        "stims_legal" => {
            PlayerCommand::SetStimsLegal(on_off(value).ok_or_else(|| format!("{spec}: stims_legal must be on|off"))?)
        }
        "asset_tax" => {
            let (k, r) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <class>:<rate>"))?;
            PlayerCommand::SetAssetTax {
                kind: parse_asset_class(k).ok_or_else(|| format!("{spec}: unknown asset class {k}"))?,
                rate: r.parse::<f32>().map_err(|e| format!("{spec}: bad rate: {e}"))?,
            }
        }
        "impound" => PlayerCommand::SetImpound(on_off(value).ok_or_else(|| format!("{spec}: impound must be on|off"))?),
        "chrome_everyone" => {
            PlayerCommand::ChromeEveryone { tier: value.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))? }
        }
        "flood_stims" => {
            let (d, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<n>"))?;
            PlayerCommand::FloodStims {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                n: n.parse::<u32>().map_err(|e| format!("{spec}: bad doses: {e}"))?,
            }
        }
        "riot_response" => PlayerCommand::SetRiotResponse(match value.to_ascii_lowercase().as_str() {
            "auto" => None,
            "contain" => Some(citysim::RiotResponse::Contain),
            "disperse" => Some(citysim::RiotResponse::Disperse),
            "crush" => Some(citysim::RiotResponse::Crush),
            _ => return Err(format!("{spec}: riot_response must be auto|contain|disperse|crush")),
        }),
        "riot" => PlayerCommand::Riot(citysim::DistrictId(
            value.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
        )),
        "litter" => {
            let (d, l) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<level>"))?;
            PlayerCommand::Litter {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                level: l.parse::<f32>().map_err(|e| format!("{spec}: bad level: {e}"))?,
            }
        }
        "treasury" => PlayerCommand::SetTreasury(value.parse::<i64>().map_err(|e| format!("{spec}: bad coins: {e}"))?),
        // M11: `city_rent=0/1/2` (Sump/Mid/Spire), `rent_cap=3` or `rent_cap=none`, `no_city_evictions=1`.
        "city_rent" => {
            let parts: Vec<i64> = value
                .split('/')
                .map(|v| v.parse::<i64>().map_err(|e| format!("{spec}: bad rent: {e}")))
                .collect::<Result<_, _>>()?;
            let rent: [i64; 3] =
                parts.try_into().map_err(|_| format!("{spec}: city_rent takes three values, Sump/Mid/Spire"))?;
            PlayerCommand::SetCityRent(rent)
        }
        "rent_cap" => PlayerCommand::SetRentCap(match value {
            "none" => None,
            v => Some(v.parse::<i64>().map_err(|e| format!("{spec}: bad cap: {e}"))?),
        }),
        "no_city_evictions" => PlayerCommand::NoCityEvictions(num("flag")? != 0.0),
        "wipe_corps" => PlayerCommand::WipeTreasuries,
        // M14 V42: `city_ice=<0..3>`, `data_tax=<0..1>`, `hack_sentence=intrusion|data_theft:<days>`.
        "city_ice" => PlayerCommand::SetCityIce(value.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?),
        "data_tax" => PlayerCommand::SetDataTax(num("rate")? as f32),
        "hack_sentence" => {
            let (c, d) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <crime>:<days>"))?;
            let crime = match c.to_ascii_lowercase().as_str() {
                "intrusion" => citysim::Crime::Intrusion,
                "data_theft" | "datatheft" => citysim::Crime::DataTheft,
                _ => return Err(format!("{spec}: crime must be intrusion|data_theft")),
            };
            PlayerCommand::SetHackSentence {
                crime,
                days: d.parse::<u16>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            }
        }
        other => return Err(format!("{spec}: unknown lever {other}")),
    };
    Ok((day * TICKS_PER_DAY, Lever::Cmd(cmd)))
}

fn run(args: RunArgs) -> Result<(), String> {
    let mut levers: Vec<(u64, Lever)> = args.levers.iter().map(|s| parse_lever(s)).collect::<Result<_, _>>()?;
    levers.sort_by_key(|(t, _)| *t);

    let mut config = Config::load();
    if args.v1_profile {
        config = config.v1_profile();
    }
    config.lod.force = args.force_lod.map(Lod::from);
    if let Some(n) = args.population {
        config.world.population = n;
    }
    if let Some(m) = &args.map {
        config.world.map = map_path(m)?;
    }
    if let Some(p) = &args.stat_policy {
        config.lod.policy = p.clone();
    }
    if args.virt_off {
        config = config.virt_off();
    }
    if args.word_off {
        config = config.word_off();
    }
    let mut world = match &args.load {
        Some(path) => {
            let mut w = save::load_from_file(path)?;
            w.config.lod.force = config.lod.force;
            w.config.assets_dir = config.assets_dir;
            if args.virt_off {
                // M14 review: a save with runs in progress would leave its
                // seated runners `JackedIn` for good (no run step pops with
                // the plane off): dump them and drop the orders first.
                if !w.runs.is_empty() || !w.run_orders.is_empty() {
                    eprintln!(
                        "note: --virt-off on a save with {} run(s) and {} order(s) in progress: dumped",
                        w.runs.len(),
                        w.run_orders.len()
                    );
                }
                citysim::systems::virt::plane_off(&mut w);
                w.config = w.config.clone().virt_off();
            }
            if args.word_off {
                w.config = w.config.clone().word_off();
            }
            w
        }
        None => World::new(args.seed, config),
    };
    let _ = args.speed;

    // Levers are absolute ticks. After a `--load`, anything already in the
    // past is in the save's command log; firing it again would double-apply.
    let start_tick = world.tick;
    let end_tick = start_tick + args.days * TICKS_PER_DAY;
    let (stale, live): (Vec<_>, Vec<_>) = levers.into_iter().partition(|(t, _)| *t < start_tick);
    for (t, cmd) in &stale {
        eprintln!("warning: lever {cmd:?} at tick {t} is before the start tick {start_tick}; skipped");
    }
    let levers = live;
    if let Some(t) = args.save_at {
        if t < start_tick || t > end_tick {
            return Err(format!("--save-at {t} is outside this run's tick range {start_tick}..={end_tick}"));
        }
    }
    let save_if_due = |world: &World| -> Result<(), String> {
        if args.save_at == Some(world.tick) {
            let path = save::save_path(&args.saves_dir, world.seed(), world.tick);
            save::save_to_file(world, &path).map_err(|e| format!("save {}: {e}", path.display()))?;
            eprintln!("saved {}", path.display());
        }
        Ok(())
    };

    if args.report {
        println!("{}", stats::CSV_HEADER);
    }

    let mut next_lever = 0;
    let mut last_event_tick: Option<u64> = None;
    // M12 fix pass (throughput): `--events` writes through one buffer flushed
    // once a day; an unbuffered line per event (1,000-1,500 a day, mostly
    // PlanAborted) cost a fifth of a day's ticks and made the dip days.
    use std::io::Write;
    let mut events_out = std::io::BufWriter::with_capacity(1 << 16, std::io::stderr());
    save_if_due(&world)?;
    while world.tick < end_tick {
        let day_start = Instant::now();
        // Chunk to the next day boundary so a mid-day `--load` still reports its first day.
        let day_end = ((world.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY).min(end_tick);
        let chunk_ticks = day_end - world.tick;
        while world.tick < day_end {
            while next_lever < levers.len() && levers[next_lever].0 <= world.tick {
                match levers[next_lever].1.resolve_all(&world) {
                    Ok(cmds) => {
                        for cmd in cmds {
                            world.push_command(cmd);
                        }
                    }
                    Err(e) => {
                        eprintln!("warning: lever {:?} at tick {}: {e}; skipped", levers[next_lever].1, world.tick)
                    }
                }
                next_lever += 1;
            }
            citysim::tick(&mut world);
            if args.events {
                // The story ring and the plan-abort debug ring, merged by tick.
                let fresh_of = |ring: &'_ std::collections::VecDeque<citysim::Event>| {
                    ring.iter()
                        .rev()
                        .take_while(|e| last_event_tick.is_none_or(|t| e.tick > t))
                        .cloned()
                        .collect::<Vec<_>>()
                };
                let mut both = fresh_of(&world.events);
                both.extend(fresh_of(&world.debug_events));
                both.sort_by_key(|e| std::cmp::Reverse(e.tick));
                let fresh: Vec<_> = both.iter().map(|e| (e.tick, e.kind, e.text.clone())).collect();
                for (tick, kind, text) in fresh.into_iter().rev() {
                    let _ = writeln!(events_out, "{tick}	{kind:?}	{text}");
                    last_event_tick = Some(tick);
                }
            }
            save_if_due(&world)?;
        }
        let _ = events_out.flush();
        if args.diag && args.events && world.tick % TICKS_PER_DAY == 0 {
            print_diag(&world);
        }
        let secs = day_start.elapsed().as_secs_f32().max(1e-9);
        if args.report && world.tick % TICKS_PER_DAY == 0 {
            if let Some(row) = world.stats.history.back_mut() {
                row.ticks_per_sec = chunk_ticks as f32 / secs;
                println!("{}", row.csv_row());
            }
        }
    }
    Ok(())
}

/// `--diag`: one `Diag` event line per Market and per corp at the day's start.
fn print_diag(world: &World) {
    let tick = world.tick;
    for &m in world.buildings_of_kind(citysim::BuildingKind::Market) {
        let owner = world.owner_label(world.owner_of(m));
        let sold = world.comp::<citysim::Market>(m).and_then(|mk| mk.sales.back().copied()).unwrap_or(0);
        let stock = world.comp::<citysim::Building>(m).map_or(0, |b| b.stock_food);
        eprintln!(
            "{tick}	Diag	market {} owner={owner} tenths={} sold={sold} stock={stock}",
            m.index,
            world.price_tenths_at(m)
        );
    }
    for c in world.corps() {
        let Some(cc) = world.comp::<citysim::Corp>(c) else { continue };
        let niches: Vec<String> = cc.niches.iter().map(|n| format!("{n}:{:.2}", cc.level(*n))).collect();
        let sold = citysim::systems::corp_brain::sold_contracts(world, cc, c);
        eprintln!(
            "{tick}	Diag	corp {} treasury={} closing={} buildings={} niches={} order={:?} contracts={sold}/{}",
            cc.name.replace(' ', "_"),
            cc.treasury,
            cc.closing,
            cc.buildings.len(),
            niches.join(","),
            cc.order,
            cc.contracts.len()
        );
    }
}

/// Which of the five Statistical outcomes an executor state counts toward.
fn exec_category(exec: &citysim::ExecState, night: bool) -> usize {
    use citysim::ActionKind as A;
    match exec {
        citysim::ExecState::Use { kind, .. } => match kind {
            A::EatFromInventory | A::EatAtHome | A::BuyFood | A::StealFood(_) | A::Forage | A::Beg => 0,
            k if k.is_work() => 1,
            A::HaulToMarket | A::CollectWage | A::CollectDole => 1,
            A::Chat | A::Drink | A::Flirt | A::Propose => 2,
            A::Sleep => 3,
            // Resting through the night is sleep for the table's purposes
            // (energy is restored); by day Rest is the idle filler.
            A::Rest if night => 3,
            _ => 4,
        },
        _ => 4,
    }
}

/// Below the Full Earn goal's savings line (`SAVINGS_DAYS` meals of coins).
fn below_savings(world: &World, id: citysim::EntityId) -> bool {
    let price = world.local(id, citysim::BuildingKind::Market).map_or(1, |m| world.price_for(m, id));
    world
        .comp::<citysim::Wallet>(id)
        .is_some_and(|w| w.coins < citysim::goap::world_state::SAVINGS_DAYS.saturating_mul(price))
}

/// Per-agent tallies for the hour in progress.
#[derive(Default, Clone)]
struct HourTally {
    /// The table row the agent's hour falls in (lawfulness and hunger at the hour's start).
    row: usize,
    /// Ticks per exec category.
    ticks: [u16; 5],
    /// steal, flirt, robbed, assaulted, killed.
    extra: [u16; 5],
    /// Had a courtship candidate at the hour's start (`p_flirt` is per such hour).
    courting: bool,
    /// New edges this hour with anyone but a co-worker (`p_meet`).
    met: u16,
    /// Chats begun this hour, and those with a housemate (`p_chat_home`).
    chats: u16,
    chats_home: u16,
    /// `stat_policy::features` at the hour's start (`--rows-csv` only).
    features: Option<[f32; citysim::systems::stat_policy::N_FEATURES]>,
}

/// Build a Full-only world (seed 1000, straight-line walks, gangless, the
/// city scaled to `--agents`), run it, and write the v2 table: per row
/// (phase x lawfulness bucket x hunger bucket, at the start of each
/// agent-hour) the frequency of the dominant exec category, and of hours in
/// which the agent stole, flirted (or proposed), was robbed, assaulted or
/// killed.
fn calibrate(args: CalibrateArgs) -> Result<(), String> {
    use citysim::{
        Brain, Corpse, DeathCause, EventKind, MemoryKind, Needs, Personality, StatRow, StatTable, STAT_ROWS,
    };
    use std::collections::BTreeMap;
    let mut config = Config::load().calibration_city(args.agents);
    config.world.map = map_path(&args.map)?;
    config.lod.force = Some(Lod::Full);
    config.exec.straight_line_paths = args.straight_lines;
    // Only the bucket edges are read from this header.
    let header = StatTable {
        version: 2,
        seed: 1000,
        days: args.days,
        agents: args.agents,
        lawfulness_edges: [0.3, 0.7],
        hunger_edge: 0.4,
        p_dole_day: 1.0,
        p_theft_caught: None,
        rows: Vec::new(),
    };
    let mut counts = [[0u64; 5]; STAT_ROWS];
    let mut extra = [[0u64; 5]; STAT_ROWS];
    let mut denom = [0u64; STAT_ROWS];
    let mut court_denom = [0u64; STAT_ROWS];
    // New non-co-worker edge ends per row, for `p_meet`.
    let mut met = [0u64; STAT_ROWS];
    let mut chats = [[0u64; 2]; STAT_ROWS];
    let (mut dole_days, mut dole_taken) = (0u64, 0u64);
    // Every Theft, every Arrest, and the arrests of a thief (`p_theft_caught`).
    let (mut thefts_all, mut arrests, mut theft_arrests) = (0u64, 0u64, 0u64);
    let with_rows = args.rows_csv.is_some();
    let mut rows_out = match &args.rows_csv {
        Some(path) => {
            use std::io::Write;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut w = std::io::BufWriter::new(f);
            let names = citysim::systems::stat_policy::FEATURE_NAMES.join(",");
            writeln!(
                w,
                "seed,tick,agent,row,{names},outcome,courting,steal,flirt,robbed,assaulted,killed,met,chats,chats_home"
            )
            .map_err(|e| e.to_string())?;
            Some(w)
        }
        None => None,
    };
    let open_hour = |world: &World, hour: &mut BTreeMap<citysim::EntityId, HourTally>| {
        hour.clear();
        for id in world.citizens() {
            if !world.has::<Brain>(id) {
                continue;
            }
            let law = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
            let hunger = world.comp::<Needs>(id).map_or(1.0, |n| n.hunger);
            let courting = citysim::systems::social::known_candidate(world, id, 0.3).is_some();
            let features = with_rows.then(|| citysim::systems::stat_policy::features(world, id));
            hour.insert(
                id,
                HourTally { row: header.index(world.phase(), law, hunger), courting, features, ..Default::default() },
            );
        }
    };
    let total_ticks = args.days * TICKS_PER_DAY;
    let t0 = Instant::now();
    // Several seeds, one tally: a single 30-day city varies by a quarter in
    // its theft rate from seed to seed (M10).
    for seed in args.first_seed..args.first_seed + args.seeds {
        let mut world = World::new(seed, config.clone());
        let mut hour: BTreeMap<citysim::EntityId, HourTally> = BTreeMap::new();
        let mut hour_start = world.tick;
        let mut poor_today: (u64, Vec<citysim::EntityId>) = (u64::MAX, Vec::new());
        let mut cursor = world.next_event_id;
        open_hour(&world, &mut hour);
        let mut known: std::collections::BTreeSet<_> = world.edges.keys().copied().collect();
        for _ in 0..total_ticks {
            citysim::tick(&mut world);
            let night = world.phase() == citysim::DayPhase::Night;
            let just = world.tick - 1;
            for (&id, t) in hour.iter_mut() {
                let Some(brain) = world.comp::<Brain>(id) else { continue };
                t.ticks[exec_category(&brain.exec, night)] += 1;
                if let citysim::ExecState::Use {
                    kind: citysim::ActionKind::Flirt | citysim::ActionKind::Propose,
                    started,
                    ..
                } = brain.exec
                {
                    if started == just {
                        t.extra[1] += 1;
                    }
                }
                if let citysim::ExecState::Use { kind: citysim::ActionKind::Chat, started, .. } = brain.exec {
                    if started == just {
                        let home = |x| world.comp::<citysim::Household>(x).and_then(|h| h.home);
                        let partner = brain.current_step().and_then(|s| s.target);
                        t.chats += 1;
                        t.chats_home += u16::from(home(id).is_some() && partner.is_some_and(|p| home(p) == home(id)));
                    }
                }
            }
            // New ring entries since the cursor.
            let first = world.events.front().map_or(0, |e| e.id);
            for e in world.events.iter().skip(cursor.saturating_sub(first) as usize) {
                let hit = |k: usize, who: Option<&citysim::EntityId>, hour: &mut BTreeMap<_, HourTally>| {
                    if let Some(t) = who.and_then(|w| hour.get_mut(w)) {
                        t.extra[k] += 1;
                    }
                };
                match e.kind {
                    // Every theft but a starving one (the Statistical eat path's
                    // desperation theft models those; M10, replaces D29's
                    // lawless-and-hungry rows).
                    EventKind::Arrest => {
                        arrests += 1;
                        let suspect = e.actors.get(1).copied();
                        let thief = world
                            .crime_reports()
                            .iter()
                            .any(|r| Some(r.suspect) == suspect && r.crime == citysim::Crime::Theft);
                        theft_arrests += u64::from(thief);
                    }
                    EventKind::Theft => {
                        thefts_all += 1;
                        let thief = e.actors.first().copied();
                        if thief.is_some_and(|a| world.comp::<Needs>(a).is_some_and(|n| n.hunger > 0.0)) {
                            hit(0, thief.as_ref(), &mut hour);
                        }
                    }
                    EventKind::Assault => hit(3, e.actors.get(1), &mut hour),
                    EventKind::Death => {
                        let violent = e
                            .actors
                            .first()
                            .and_then(|&a| world.comp::<Corpse>(a))
                            .is_some_and(|c| c.cause == DeathCause::Violence);
                        if violent {
                            hit(4, e.actors.first(), &mut hour);
                        }
                    }
                    _ => {}
                }
            }
            cursor = world.next_event_id;
            if world.tick.is_multiple_of(citysim::TICKS_PER_HOUR) {
                // Close the hour: first-hand robberies, then every agent of the
                // hour (the dead included) into its row.
                for (&id, t) in hour.iter_mut() {
                    let robbed = world.comp::<citysim::Memory>(id).is_some_and(|m| {
                        m.entries
                            .iter()
                            .any(|e| e.kind == MemoryKind::WasRobbed && !e.second_hand && e.tick >= hour_start)
                    });
                    if robbed {
                        t.extra[2] += 1;
                    }
                }
                // The hour's first meetings: edges that were not there at its
                // start. Co-workers are left out (the Statistical shift
                // drifts its co-workers itself).
                let employer = |id| world.comp::<citysim::Job>(id).and_then(|j| j.employer);
                for &(a, b) in world.edges.keys().filter(|k| !known.contains(k)) {
                    if employer(a).is_some() && employer(a) == employer(b) {
                        continue;
                    }
                    for x in [a, b] {
                        if let Some(t) = hour.get_mut(&x) {
                            t.met += 1;
                        }
                    }
                }
                known = world.edges.keys().copied().collect();
                for (&id, t) in hour.iter() {
                    let c = t.ticks;
                    let dominant =
                        if c[0] > 0 { 0 } else { (1..5).max_by_key(|&k| (c[k], std::cmp::Reverse(k))).unwrap_or(4) };
                    if let (Some(w), Some(f)) = (rows_out.as_mut(), t.features.as_ref()) {
                        use std::io::Write;
                        let mut line = format!("{seed},{hour_start},{},{}", id.index, t.row);
                        for v in f {
                            line.push_str(&format!(",{v}"));
                        }
                        let x = t.extra.map(|n| n.min(1));
                        line.push_str(&format!(
                            ",{dominant},{},{},{},{},{},{},{},{},{}\n",
                            u8::from(t.courting),
                            x[0],
                            x[1],
                            x[2],
                            x[3],
                            x[4],
                            t.met,
                            t.chats,
                            t.chats_home
                        ));
                        w.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
                    }
                    counts[t.row][dominant] += 1;
                    denom[t.row] += 1;
                    court_denom[t.row] += u64::from(t.courting);
                    met[t.row] += u64::from(t.met);
                    chats[t.row][0] += u64::from(t.chats);
                    chats[t.row][1] += u64::from(t.chats_home);
                    for (k, (sum, &n)) in extra[t.row].iter_mut().zip(&t.extra).enumerate() {
                        // A flirt counts in the hours it had a candidate for.
                        if k != 1 || t.courting {
                            *sum += u64::from(n.min(1));
                        }
                    }
                }
                hour_start = world.tick;
                open_hour(&world, &mut hour);
                // The Work phase opens: which jobless adults are below their
                // savings line (the Full Earn goal's gate) today?
                if world.phase() == citysim::DayPhase::Work && poor_today.0 != world.day() {
                    poor_today = (world.day(), Vec::new());
                    for id in world.citizens() {
                        if world.has::<Brain>(id)
                            && !world.has::<citysim::Job>(id)
                            && citysim::systems::demography::is_adult(&world, id)
                            && below_savings(&world, id)
                        {
                            poor_today.1.push(id);
                        }
                    }
                }
            }
            // Day end: did the poor collect the dole?
            if let Some(r) =
                world.stats.history.back().filter(|_| args.health && world.tick.is_multiple_of(TICKS_PER_DAY))
            {
                let homes = world
                    .buildings_of_kind(citysim::BuildingKind::Home)
                    .iter()
                    .filter(|&&h| {
                        world.comp::<citysim::Building>(h).is_some_and(|b| {
                            b.occupants
                                .iter()
                                .any(|&o| world.comp::<citysim::Household>(o).is_some_and(|hh| hh.home == Some(h)))
                        })
                    })
                    .count();
                let farms: u32 = world
                    .buildings_of_kind(citysim::BuildingKind::Farm)
                    .iter()
                    .filter_map(|&f| world.comp::<citysim::Building>(f))
                    .map(|b| b.stock_food)
                    .sum();
                eprintln!(
                    "seed {seed} day {}: price {} treasury {} warehouse {} market {} pantry {} farms {farms} employed {} thefts {} starved {} occupied homes {homes}",
                    r.day, r.price, r.treasury, r.food_warehouse, r.food_market, r.food_pantry, r.employed, r.thefts, r.deaths_starvation
                );
            }
            if world.tick.is_multiple_of(TICKS_PER_DAY) {
                let day = world.day() - 1;
                if poor_today.0 == day {
                    for &id in &poor_today.1 {
                        dole_days += 1;
                        dole_taken += u64::from(world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day == Some(day)));
                    }
                }
            }
        }
    }
    // The five independent rolls are pooled over the two hunger buckets of a
    // phase and lawfulness: a Statistical agent eats as soon as it is hungry
    // and spends a sixth of a Full agent's hours in the hungry bucket, so
    // hunger-conditioned crime rates would not transfer.
    // `p_flirt` is per hour with a courtship candidate (a Full agent only
    // flirts when it has one, and the Statistical roll needs one too).
    let pooled = |i: usize| -> ([u64; 5], u64, u64) {
        let (a, b) = (i & !1, i | 1);
        let x: [u64; 5] = std::array::from_fn(|k| extra[a][k] + extra[b][k]);
        (x, denom[a] + denom[b], court_denom[a] + court_denom[b])
    };
    let mut rows: Vec<StatRow> = (0..STAT_ROWS)
        .map(|i| {
            let n = denom[i].max(1) as f32;
            let c = counts[i];
            let (x, pn, cn) = pooled(i);
            let (pn, cn) = (pn.max(1) as f32, cn.max(1) as f32);
            StatRow {
                label: StatTable::label(i),
                p_eat: c[0] as f32 / n,
                p_work: c[1] as f32 / n,
                p_social: c[2] as f32 / n,
                p_sleep: c[3] as f32 / n,
                p_steal: x[0] as f32 / pn,
                p_flirt: x[1] as f32 / cn,
                p_robbed: x[2] as f32 / pn,
                p_assaulted: x[3] as f32 / pn,
                p_killed: x[4] as f32 / pn,
                // Each Statistical meeting gives both agents an edge: half the
                // ends per agent-hour, pooled over hunger.
                p_meet: {
                    let (a, b) = (i & !1, i | 1);
                    (met[a] + met[b]) as f32 / 2.0 / pn
                },
                p_chat: {
                    let (a, b) = (i & !1, i | 1);
                    (chats[a][0] + chats[b][0]) as f32 / pn
                },
                p_chat_home: {
                    let (a, b) = (i & !1, i | 1);
                    let all = chats[a][0] + chats[b][0];
                    if all == 0 {
                        0.0
                    } else {
                        (chats[a][1] + chats[b][1]) as f32 / all as f32
                    }
                },
            }
        })
        .collect();
    // An empty row borrows the same phase and lawfulness row's other hunger bucket.
    for i in 0..STAT_ROWS {
        if denom[i] == 0 {
            let twin = i ^ 1;
            rows[i] = if denom[twin] > 0 {
                StatRow { label: StatTable::label(i), ..rows[twin].clone() }
            } else {
                StatRow { label: StatTable::label(i), ..Default::default() }
            };
        }
    }
    if let Some(mut w) = rows_out {
        use std::io::Write;
        w.flush().map_err(|e| e.to_string())?;
    }
    let p_dole_day = (dole_taken as f32 / dole_days.max(1) as f32).min(1.0);
    let p_theft_caught = Some((theft_arrests as f32 / thefts_all.max(1) as f32).min(1.0));
    eprintln!("thefts {thefts_all}, arrests {arrests}, of a thief {theft_arrests}");
    let table = StatTable { rows, p_dole_day, p_theft_caught, ..header };
    let body = toml::to_string(&table).map_err(|e| e.to_string())?;
    let text = format!(
        "# generated by calibrate v2: seeds {}..={}, {} days, {} agents, map {}, gangless, {} walks, DO NOT EDIT\n{body}",
        args.first_seed,
        args.first_seed + args.seeds - 1,
        args.days,
        args.agents,
        args.map.display(),
        if args.straight_lines { "straight-line" } else { "pathed" }
    );
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&args.out, text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    eprintln!(
        "calibrated {} agents over {} days in {:.1}s -> {} (agent-hours per row: {:?})",
        args.agents,
        args.days,
        t0.elapsed().as_secs_f32(),
        args.out.display(),
        denom
    );
    eprintln!("new non-co-worker edge ends per row {met:?}");
    eprintln!("chats (all, with a housemate) per row {chats:?}");
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(args) => run(args),
        Command::Calibrate(args) => calibrate(args),
        Command::Shadow(args) => shadow::shadow(args),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citysim::{AssetClass, AssetKind, DistrictId, Slot};

    fn cmd(spec: &str) -> PlayerCommand {
        match parse_lever(spec).expect("parses") {
            (_, Lever::Cmd(c)) => c,
            (_, other) => panic!("{spec}: expected a plain command, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_stims_legal() {
        assert_eq!(cmd("day=30:stims_legal=on"), PlayerCommand::SetStimsLegal(true));
        assert_eq!(cmd("day=30:stims_legal=off"), PlayerCommand::SetStimsLegal(false));
        assert!(parse_lever("day=30:stims_legal=maybe").is_err());
    }

    #[test]
    fn test_parse_asset_tax() {
        let (t, _) = parse_lever("day=5:asset_tax=car:0.5").unwrap();
        assert_eq!(t, 5 * TICKS_PER_DAY);
        assert_eq!(cmd("day=5:asset_tax=car:0.5"), PlayerCommand::SetAssetTax { kind: AssetClass::Car, rate: 0.5 });
        assert_eq!(
            cmd("day=5:asset_tax=chrome:1"),
            PlayerCommand::SetAssetTax { kind: AssetClass::Implant, rate: 1.0 }
        );
        assert!(parse_lever("day=5:asset_tax=boat:1").is_err());
        assert!(parse_lever("day=5:asset_tax=car").is_err());
    }

    #[test]
    fn test_parse_impound() {
        assert_eq!(cmd("day=1:impound=off"), PlayerCommand::SetImpound(false));
        assert_eq!(cmd("day=1:impound=on"), PlayerCommand::SetImpound(true));
        assert!(parse_lever("day=1:impound=2").is_err());
    }

    #[test]
    fn test_parse_grant_asset() {
        match parse_lever("day=2:grant_asset=17:arms:2").unwrap().1 {
            Lever::GrantAsset(17, AssetKind::Implant(Slot::Arms), 2) => {}
            other => panic!("{other:?}"),
        }
        match parse_lever("day=2:grant_asset=3:flyer:1").unwrap().1 {
            Lever::GrantAsset(3, AssetKind::Flyer, 1) => {}
            other => panic!("{other:?}"),
        }
        assert!(parse_lever("day=2:grant_asset=3:tank:1").is_err());
        assert!(parse_lever("day=2:grant_asset=3:car").is_err());
    }

    #[test]
    fn test_parse_wreck() {
        assert!(matches!(parse_lever("day=2:wreck=2100").unwrap().1, Lever::Wreck(2100)));
        assert!(parse_lever("day=2:wreck=x").is_err());
    }

    #[test]
    fn test_parse_chrome_everyone() {
        assert_eq!(cmd("day=10:chrome_everyone=2"), PlayerCommand::ChromeEveryone { tier: 2 });
        assert!(parse_lever("day=10:chrome_everyone=-1").is_err());
    }

    #[test]
    fn test_parse_flood_stims() {
        assert_eq!(cmd("day=10:flood_stims=7:500"), PlayerCommand::FloodStims { district: DistrictId(7), n: 500 });
        assert!(parse_lever("day=10:flood_stims=7").is_err());
    }

    #[test]
    fn test_parse_brick() {
        assert!(matches!(parse_lever("day=10:brick=8").unwrap().1, Lever::Brick(Lender::Corp(8))));
        assert!(matches!(parse_lever("day=10:brick=agent:42").unwrap().1, Lever::Brick(Lender::Agent(42))));
        assert!(parse_lever("day=10:brick=agent:x").is_err());
    }

    #[test]
    fn test_parse_virt_god_levers() {
        assert!(matches!(parse_lever("day=3:wipe_data=812"), Ok((_, Lever::WipeData(812)))));
        assert!(matches!(
            parse_lever("day=3:set_tech=8:chrome:2"),
            Ok((_, Lever::SetTech(8, citysim::virt::Track::Chrome, 2)))
        ));
        assert!(matches!(
            parse_lever("day=3:grant_data=gang1:deck:500"),
            Ok((_, Lever::GrantData(Faction::Gang(1), citysim::virt::Track::Deck, 500)))
        ));
        assert!(parse_lever("day=3:set_tech=8:psionics:2").is_err());
    }

    #[test]
    fn test_parse_virt_levers_phase_4() {
        use citysim::virt::Purpose;
        assert!(matches!(parse_lever("day=3:city_ice=3").unwrap().1, Lever::Cmd(PlayerCommand::SetCityIce(3))));
        assert!(matches!(
            parse_lever("day=3:data_tax=0.25").unwrap().1,
            Lever::Cmd(PlayerCommand::SetDataTax(t)) if (t - 0.25).abs() < 1e-6
        ));
        assert!(matches!(
            parse_lever("day=3:hack_sentence=intrusion:9").unwrap().1,
            Lever::Cmd(PlayerCommand::SetHackSentence { crime: citysim::Crime::Intrusion, days: 9 })
        ));
        assert!(matches!(
            parse_lever("day=3:hack_sentence=data_theft:20").unwrap().1,
            Lever::Cmd(PlayerCommand::SetHackSentence { crime: citysim::Crime::DataTheft, days: 20 })
        ));
        assert!(parse_lever("day=3:hack_sentence=theft:20").is_err());
        assert!(matches!(parse_lever("day=3:grant_deck=77:3").unwrap().1, Lever::GrantDeck(77, 3)));
        assert!(matches!(parse_lever("day=3:set_ice=812:2").unwrap().1, Lever::SetIce(812, 2)));
        assert!(matches!(parse_lever("day=3:fry=77").unwrap().1, Lever::Fry(77)));
        assert!(matches!(
            parse_lever("day=3:run_now=77:812:wipe").unwrap().1,
            Lever::RunNow(77, RunTarget::Building(812), Purpose::Data { wipe: true })
        ));
        assert!(matches!(
            parse_lever("day=3:run_now=77:corp3:ledger").unwrap().1,
            Lever::RunNow(77, RunTarget::Corp(3), Purpose::Ledger)
        ));
        assert!(parse_lever("day=3:run_now=77:812:hax").is_err());
    }

    #[test]
    fn test_parse_virt_god_plurals_phase_5() {
        assert!(matches!(parse_lever("day=20:wipe_corp_data=8").unwrap().1, Lever::WipeCorpData(8)));
        assert!(matches!(parse_lever("day=45:grant_decks=6:10:3").unwrap().1, Lever::GrantDecks(6, 10, 3)));
        assert!(matches!(parse_lever("day=45:set_corp_ice=6:0").unwrap().1, Lever::SetCorpIce(6, 0)));
        assert!(parse_lever("day=45:grant_decks=6:10").is_err());
        assert!(parse_lever("day=45:set_corp_ice=x:0").is_err());
        assert!(matches!(
            parse_lever("day=10:plant_rumour=12:killed:none:3:1.0").unwrap().1,
            Lever::PlantRumour(12, citysim::word::Deed::Killed, None, 3, _)
        ));
        assert!(matches!(
            parse_lever("day=10:set_reputation=12:dread:0.9:5").unwrap().1,
            Lever::SetReputation(12, citysim::word::Axis::Dread, _, 5)
        ));
        assert!(parse_lever("day=10:plant_rumour=12:kissed:none:3:1.0").is_err());
        assert!(matches!(
            parse_lever("day=5:grant_skill=12:persuasion:1.0").unwrap().1,
            Lever::GrantSkill(SkillTarget::Agent(12), citysim::word::SocialSkill::Persuasion, _, false)
        ));
        assert!(matches!(
            parse_lever("day=5:grant_skill=dregs10:persuasion:1.0:suit").unwrap().1,
            Lever::GrantSkill(SkillTarget::Dregs(10), citysim::word::SocialSkill::Persuasion, _, true)
        ));
        assert!(matches!(
            parse_lever("day=5:set_creed=1:purist").unwrap().1,
            Lever::SetCreed(1, Some(citysim::word::Creed::Purist))
        ));
        assert!(parse_lever("day=5:set_creed=1:monk").is_err());
        assert!(parse_lever("day=5:grant_skill=12:charisma:1.0").is_err());
        assert!(parse_lever("day=3:city_ice=x").is_err());
    }

    #[test]
    fn test_parse_chase() {
        assert!(matches!(parse_lever("day=10:chase=99").unwrap().1, Lever::Chase(99)));
        assert!(parse_lever("day=10:chase=").is_err());
    }
}
