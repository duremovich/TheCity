use super::*;

/// A scheduled lever: a ready command, or a god command naming a gang by its
/// index in `World::gangs()`, resolved to entity ids when it fires (the
/// command log records the resolved command, so replays need no resolving).
#[derive(Clone, Debug)]
pub(super) enum Lever {
    Cmd(PlayerCommand),
    KillLeader(usize),
    JailGang(usize, u32),
    KillGang(usize),
    FundGang(usize, i64),
    SeizeGang(usize),
    /// M11: `breakup=<corp slot>` (the seeding row, 0-based).
    BreakUp(u8),
    // M11 god levers, a corp by its seeding slot (docs/history/GOD_SCENARIOS_V2.md).
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
    /// M15 god (W42, phase 3): `declare_vendetta=<faction>:<faction>:<w>`.
    DeclareVendetta(FactionRef, FactionRef, f32),
    /// M15 god (W42, phase 3): `kill_friend=<agent|leader<g>>:<agent|member<g>>`.
    KillFriend(Who, Who),
    /// M15 god (W42, phase 3): `hunt=<agent>:<agent>`.
    Hunt(u32, u32),
    /// M15 lever (W42, phase 4): `censor=gang<i>|corp<slot>|law[:off]`.
    Censor(FactionRef, bool),
    /// L2 god (L37): `open_venue=<kind>:<district>:<city|corp<slot>|gang<i>>`.
    OpenVenue(citysim::BuildingKind, u8, Option<FactionRef>),
    /// L2 god (L37, phase 4): `faction_strike=<gang i>:<district>:<days>`.
    FactionStrike(usize, u8, u64),
    /// M16a god (plan C39): `post_contract=<buyer>:<hit|beat|guard|locate>:<agent i|b<building i>>:<price>:<0|1>:<days>`
    /// (`buyer` an agent index, `corp<slot>`, `gang<i>` or `city`; price 0 = quoted; days 0 = the kind's).
    PostContract(Buyer, citysim::contract::ContractKind, ContractTarget, i64, bool, u16),
    /// M16a god (plan C39): `take_contract=<id>:<agent i>` (gang and corp takers are phase 2).
    TakeContract(u64, u32),
    /// Real economy (plan E47): `donate=<mission building i>:<coins>`.
    Donate(u32, i64),
    /// M16b god (phase 4, spec § 8): `hostage=`, `wound=`, `subscribe=`,
    /// `tag_agent=`, `coerce=`, `call_vote=` (see `parse_lever`).
    M16b(M16bLever),
}

/// M16b phase 4's god levers, by entity index, gang index or corp slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum M16bLever {
    Hostage { captive: u32, holder: Option<FactionRef>, at: u32, demand: DemandSpec },
    Wound(u32),
    Subscribe(u32, u8),
    TagAgent(u32, FactionRef, u8),
    Coerce(u32, u32, LevSpec, DemandSpec),
    CallVote(u8),
}

/// A demand on the command line: `coins<n>`, `weekly<n>`, `release<agent>`,
/// `vote<corp slot>` (for the coercing agent), or none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DemandSpec {
    None,
    Coins(i64),
    Weekly(i64),
    Release(u32),
    Vote(u8),
}

/// A leverage on the command line: `threat` or `debt<n>`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LevSpec {
    Threat,
    Debt(i64),
}

pub(super) fn parse_demand(v: &str) -> Result<DemandSpec, String> {
    let num = |s: &str| s.parse::<i64>().map_err(|e| format!("bad demand {v}: {e}"));
    if v.is_empty() || v == "none" {
        return Ok(DemandSpec::None);
    }
    if let Some(n) = v.strip_prefix("coins") {
        return Ok(DemandSpec::Coins(num(n)?));
    }
    if let Some(n) = v.strip_prefix("weekly") {
        return Ok(DemandSpec::Weekly(num(n)?));
    }
    if let Some(n) = v.strip_prefix("release") {
        return n.parse::<u32>().map(DemandSpec::Release).map_err(|e| format!("bad demand {v}: {e}"));
    }
    if let Some(n) = v.strip_prefix("vote") {
        return n.parse::<u8>().map(DemandSpec::Vote).map_err(|e| format!("bad demand {v}: {e}"));
    }
    Err(format!("demand must be none|coins<n>|weekly<n>|release<agent>|vote<slot>, got {v}"))
}

impl DemandSpec {
    fn resolve(self, world: &World, agent: citysim::EntityId) -> Result<Option<citysim::contract::Demand>, String> {
        use citysim::contract::Demand;
        Ok(match self {
            DemandSpec::None => None,
            DemandSpec::Coins(amount) => Some(Demand::Coins { amount, weekly: false }),
            DemandSpec::Weekly(amount) => Some(Demand::Coins { amount, weekly: true }),
            DemandSpec::Release(a) => Some(Demand::Release(agent_at(world, a)?)),
            DemandSpec::Vote(s) => Some(Demand::Vote { corp: corp_in_slot(world, s)?, for_: agent }),
        })
    }
}

impl M16bLever {
    fn resolve(self, world: &World) -> Result<PlayerCommand, String> {
        Ok(match self {
            M16bLever::Hostage { captive, holder, at, demand } => {
                let captive = agent_at(world, captive)?;
                PlayerCommand::Hostage {
                    captive,
                    holder: match holder {
                        Some(f) => Some(f.resolve(world)?),
                        None => None,
                    },
                    at: building_at(world, at)?,
                    demand: demand.resolve(world, captive)?,
                }
            }
            M16bLever::Wound(a) => PlayerCommand::Wound { agent: agent_at(world, a)?, by: None },
            M16bLever::Subscribe(a, s) => {
                PlayerCommand::Subscribe { agent: agent_at(world, a)?, corp: corp_in_slot(world, s)? }
            }
            M16bLever::TagAgent(a, f, tier) => {
                PlayerCommand::TagAgent { agent: agent_at(world, a)?, owner: f.resolve(world)?, tier }
            }
            M16bLever::Coerce(a, o, lev, demand) => {
                let agent = agent_at(world, a)?;
                PlayerCommand::Coerce {
                    agent,
                    over: agent_at(world, o)?,
                    leverage: match lev {
                        LevSpec::Threat => citysim::contract::Leverage::Threat,
                        LevSpec::Debt(amount) => citysim::contract::Leverage::Debt { amount },
                    },
                    demand: demand.resolve(world, agent)?.ok_or_else(|| "coerce needs a demand".to_string())?,
                }
            }
            M16bLever::CallVote(s) => PlayerCommand::CallVote(corp_in_slot(world, s)?),
        })
    }
}

/// A contract record's target on the command line: an agent's or a
/// building's entity index (`b<i>`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ContractTarget {
    Agent(u32),
    Building(u32),
}

/// A faction on the command line: `gang<i>`, `corp<slot>` or `law`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FactionRef {
    Gang(usize),
    Corp(u8),
    Law,
}

/// An agent on the command line: an entity index, `leader<g>` (gang g's
/// leader) or `member<g>` (gang g's best fighter, ties the lower id).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Who {
    Agent(u32),
    Leader(usize),
    Member(usize),
}

pub(super) fn parse_faction(v: &str) -> Result<FactionRef, String> {
    if v.eq_ignore_ascii_case("law") {
        return Ok(FactionRef::Law);
    }
    if let Some(g) = v.strip_prefix("gang") {
        return g.parse::<usize>().map(FactionRef::Gang).map_err(|e| format!("bad gang index {g}: {e}"));
    }
    if let Some(c) = v.strip_prefix("corp") {
        return c.parse::<u8>().map(FactionRef::Corp).map_err(|e| format!("bad corp slot {c}: {e}"));
    }
    Err(format!("faction must be gang<i>|corp<slot>|law, got {v}"))
}

pub(super) fn parse_who(v: &str) -> Result<Who, String> {
    if let Some(g) = v.strip_prefix("leader") {
        return g.parse::<usize>().map(Who::Leader).map_err(|e| format!("bad gang index {g}: {e}"));
    }
    if let Some(g) = v.strip_prefix("member") {
        return g.parse::<usize>().map(Who::Member).map_err(|e| format!("bad gang index {g}: {e}"));
    }
    v.parse::<u32>().map(Who::Agent).map_err(|e| format!("bad agent {v}: {e}"))
}

impl FactionRef {
    fn resolve(self, world: &World) -> Result<citysim::EntityId, String> {
        match self {
            FactionRef::Gang(i) => world.gangs().get(i).copied().ok_or_else(|| format!("no gang {i}")),
            FactionRef::Corp(s) => corp_in_slot(world, s),
            FactionRef::Law => world
                .building_of_kind(citysim::BuildingKind::Jail)
                .filter(|&j| world.has::<citysim::components::Law>(j))
                .ok_or_else(|| "no Law".to_string()),
        }
    }
}

impl Who {
    fn resolve(self, world: &World) -> Result<citysim::EntityId, String> {
        let gang = |i: usize| world.gangs().get(i).copied().ok_or_else(|| format!("no gang {i}"));
        match self {
            Who::Agent(a) => agent_at(world, a),
            Who::Leader(i) => world
                .comp::<citysim::Gang>(gang(i)?)
                .and_then(|g| g.leader)
                .ok_or_else(|| format!("gang {i} has no leader")),
            Who::Member(i) => {
                let g = world.comp::<citysim::Gang>(gang(i)?).ok_or_else(|| format!("no gang {i}"))?;
                g.members
                    .iter()
                    .copied()
                    .filter(|&m| citysim::systems::law::living(world, m))
                    .map(|m| (citysim::systems::combat::power(world, m), m))
                    .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
                    .map(|(_, m)| m)
                    .ok_or_else(|| format!("gang {i} has no living member"))
            }
        }
    }
}

/// Who `grant_skill=` names: an agent by index, or the n poorest Dregs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SkillTarget {
    Agent(u32),
    Dregs(u32),
}

/// The target of `run_now`: a building by entity index, or a corp by slot
/// (for a `ledger` run).
#[derive(Clone, Copy, Debug)]
pub(super) enum RunTarget {
    Building(u32),
    Corp(u8),
}

/// A corp by its seeding slot or a gang by its index (`grant_data`).
#[derive(Clone, Copy, Debug)]
pub(super) enum Faction {
    Corp(u8),
    Gang(usize),
}

/// Whose financed implants `brick` bricks: a corp by its seeding slot or an agent.
#[derive(Clone, Copy, Debug)]
pub(super) enum Lender {
    Corp(u8),
    Agent(u32),
}

/// The living agent with this entity index.
fn agent_at(world: &World, index: u32) -> Result<citysim::EntityId, String> {
    world.citizens().into_iter().find(|c| c.index == index).ok_or_else(|| format!("no agent {index}"))
}

/// `moto|car|truck|flyer|arms|legs|nerves|eyes|skin|robot|pack|bridge`.
pub(super) fn parse_asset_kind(s: &str) -> Option<citysim::AssetKind> {
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
pub(super) fn parse_asset_class(s: &str) -> Option<citysim::AssetClass> {
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
pub(super) fn on_off(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" => Some(true),
        "off" | "0" | "false" => Some(false),
        _ => None,
    }
}

/// Who `buy_building` buys for: `city`, `gang<i>`, `corp<slot>` or an agent's entity index.
#[derive(Clone, Copy, Debug)]
pub(super) enum Buyer {
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
pub(super) enum SeizeTo {
    Corp(u8),
    City,
    Gang(usize),
}

/// The corp seeded in `slot`, if it still exists.
fn corp_in_slot(world: &World, slot: u8) -> Result<citysim::EntityId, String> {
    world
        .corps()
        .into_iter()
        .find(|&c| world.comp::<citysim::Corp>(c).is_some_and(|cc| cc.slot == Some(u16::from(slot))))
        .ok_or_else(|| format!("no corp in slot {slot}"))
}

impl Lever {
    /// Every command a lever stands for (one, or a `dregs<n>` form's n).
    pub(super) fn resolve_all(&self, world: &World) -> Result<Vec<PlayerCommand>, String> {
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
            Lever::DeclareVendetta(a, b, weight) => {
                PlayerCommand::DeclareVendetta { a: a.resolve(world)?, b: b.resolve(world)?, weight }
            }
            Lever::KillFriend(of, by) => PlayerCommand::KillFriend { of: of.resolve(world)?, by: by.resolve(world)? },
            Lever::Hunt(h, t) => PlayerCommand::Hunt { hunter: agent_at(world, h)?, target: agent_at(world, t)? },
            Lever::Censor(f, on) => PlayerCommand::CensorStories { faction: f.resolve(world)?, on },
            Lever::OpenVenue(kind, d, owner) => PlayerCommand::OpenVenue {
                kind,
                district: citysim::DistrictId(d),
                owner: match owner {
                    Some(f) => Some(f.resolve(world)?),
                    None => None,
                },
            },
            Lever::FactionStrike(i, d, days) => {
                PlayerCommand::FactionStrike { gang: gang(i)?, district: citysim::DistrictId(d), days }
            }
            Lever::PostContract(buyer, kind, target, price, brokered, deadline_days) => PlayerCommand::PostContract {
                buyer: match buyer {
                    Buyer::City => None,
                    Buyer::Gang(i) => Some(gang(i)?),
                    Buyer::Corp(s) => Some(corp_in_slot(world, s)?),
                    Buyer::Agent(a) => Some(agent_at(world, a)?),
                },
                kind,
                target: match target {
                    ContractTarget::Agent(a) => citysim::contract::Target::Agent(agent_at(world, a)?),
                    ContractTarget::Building(b) => citysim::contract::Target::Building(building_at(world, b)?),
                },
                price,
                brokered,
                deadline_days,
            },
            Lever::TakeContract(contract, taker) => {
                PlayerCommand::TakeContract { contract, taker: agent_at(world, taker)? }
            }
            Lever::Donate(b, amount) => PlayerCommand::Donate { mission: building_at(world, b)?, amount },
            Lever::M16b(m) => m.resolve(world)?,
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
