//! Player commands and stored lever values. Every command is applied at the
//! start of the next tick and recorded with its tick, so a save plus its
//! command log replays deterministically.

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Corp, CorpOrder, CorpShock, Crime, DeathCause, Gang, Household, Niche, Position,
    Posture, Rect, Sentence, TileKind, TilePos, Wallet,
};
use crate::config::Config;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::world::World;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlayerCommand {
    /// Attribute an open hole now (the inspector's "find out"). A missing
    /// hole does nothing. Logged, so a replay binds at the same tick.
    Bind(crate::components::HoleId),
    ReleaseReserve {
        amount: u32,
    },
    SetTaxRate(f32),
    SetSentenceMult(f32),
    SetGuardCount(u8),
    SetImmigrationPerWeek(u8),
    GrantCoins {
        agent: EntityId,
        amount: i64,
    },
    Arrest(EntityId),
    Release(EntityId),
    DemolishHome(EntityId),
    BuildHome {
        rect: Rect,
    },
    Pin(EntityId, bool),
    /// Issued by the app whenever the view rect changes; makes LOD replayable.
    SetView(Option<Rect>),
    /// `0..=10`, default 3.
    SetDolePerDay(u8),
    /// App-only; logged so replays know the speed, no sim effect.
    SetSpeed(Speed),
    /// M9: pin the law's posture, or (`None`) hand it back to the captain.
    SetLawPosture(Option<Posture>),
    /// M12 D42: a district's allocation weight multiplier (`0.0..=5.0`; 0 withdraws the law).
    SetGuardWeight {
        district: crate::components::DistrictId,
        weight: f32,
    },
    /// M12 D42: pin a district's stance, or (`None`) hand it back to the captain.
    SetStance {
        district: crate::components::DistrictId,
        stance: Option<crate::components::Stance>,
    },
    // --- God commands (docs/VISION.md "How we test: god scenarios"). They
    // break the world's rules on purpose: money from nowhere, death without a
    // killer, sentences without a crime. Logged and replayed like the rest.
    /// Kill an agent: cause Violence, no killer.
    KillAgent(EntityId),
    /// Jail an agent for `days` (no crime, no report, no capacity check); a
    /// prisoner's sentence is extended to at least that.
    JailAgent {
        who: EntityId,
        days: u32,
    },
    /// Free a prisoner (as `Release`, but named for the god set).
    FreeAgent(EntityId),
    /// Add coins to a gang's treasury, out of thin air.
    FundGang {
        gang: EntityId,
        amount: i64,
    },
    /// A gang's treasury moves into the city Treasury.
    SeizeGangTreasury(EntityId),
    /// Kill every living member of a gang.
    KillGang(EntityId),
    /// Jail every member of a gang for `days` (extending sentences already running).
    JailGang {
        gang: EntityId,
        days: u32,
    },
    /// Dismiss every guard; `guard_count` is untouched, so the law re-hires.
    FireAllGuards,
    /// Set the city Treasury to exactly this many coins (may be negative).
    SetTreasury(i64),
    // --- M11 levers (docs/M11_OWNERSHIP.md § 8) ---
    /// Rent on city-owned Blocks per tier (Sump, Mid, Spire), each `0..=20`.
    SetCityRent([i64; 3]),
    /// A ceiling on every Block's rent; `None` lifts it.
    SetRentCap(Option<i64>),
    /// The city buys a building at its value from its owner.
    Nationalise(EntityId),
    /// Treasury -> a corp's treasury.
    Subsidise {
        corp: EntityId,
        amount: i64,
    },
    /// The city never evicts while set.
    NoCityEvictions(bool),
    /// Split a corp's monopoly niche in two (D31); refused without one.
    BreakUp(EntityId),
    // --- M11 god commands (docs/GOD_SCENARIOS_V2.md): corps, out of the rules.
    /// Add coins to a corp's treasury, out of thin air.
    FundCorp {
        corp: EntityId,
        amount: i64,
    },
    /// The corp's treasury goes to -1, `negative_since` is back-dated past
    /// `bankrupt_days`, and it goes bankrupt at once (`corps::bankrupt`), so
    /// a day's takings cannot save it before the midnight pass.
    BankruptCorp(EntityId),
    /// Every building of `owner` (`None` = the city) moves to `to` (an
    /// agent, gang, corp or `None` = the city). A corp losing buildings takes
    /// one `BuildingLost`, as when a rival's Acquire takes one.
    SeizeBuildings {
        owner: Option<EntityId>,
        to: Option<EntityId>,
    },
    /// Kill a corp's exec (Violence, no killer).
    KillExec(EntityId),
    /// Kill every non-exec employee of a corp (Violence, no killer).
    KillStaff(EntityId),
    /// Pin a corp's order for `days` (the daily and shock rescores keep the
    /// trace but do not switch). `niche` defaults to the order's current
    /// niche, else the corp's first.
    SetCorpOrder {
        corp: EntityId,
        order: CorpOrder,
        #[serde(default)]
        niche: Option<Niche>,
        days: u32,
    },
    /// The corp's non-exec workers walk out of their next shift, as a Street
    /// strike does (D35), whatever the unrest; the strike cooldown is untouched.
    StrikeNow(EntityId),
    /// Every corp's treasury to 0.
    WipeTreasuries,
    // --- M12 levers (docs/M12_DISTRICTS.md § 7, plan D42) ---
    /// The city's Sanitation headcount (`0..=60`).
    SetSanitation(u8),
    /// A district's sweeper weight multiplier (`0.0..=5.0`; 0 sends no sweepers).
    SetSanitationWeight {
        district: crate::components::DistrictId,
        weight: f32,
    },
    /// A district curfew: Vagrancy × `curfew_mult`, submission + `curfew_fear`,
    /// residents' happiness − 0.05 in the aggregate.
    SetCurfew {
        district: crate::components::DistrictId,
        on: bool,
    },
    /// Pin the law's riot response, or (`None`) hand it back to the captain.
    SetRiotResponse(Option<crate::components::RiotResponse>),
    // --- M12 god commands (docs/GOD_SCENARIOS_V3.md): districts, out of the rules.
    /// A riot in the district at the next muster hour, unrest, streak,
    /// cooldown and caps ignored (one eligible rioter is enough).
    Riot(crate::components::DistrictId),
    /// Every street tile of the district to `round(level × 254)`.
    Litter {
        district: crate::components::DistrictId,
        level: f32,
    },
    /// Split a gang by the D36 rule minus the roll and the character tests
    /// (lieutenant strength, loyalty); refused with the reason when the
    /// structure fails (no lieutenant, one held district, the cap, no site).
    SplitGang(EntityId),
    /// A Block, Bar or Hotel goes derelict now.
    Derelict(EntityId),
    /// `buyer` (an agent, gang or corp; `None` = the city) buys `building`
    /// for `price` (paid to the owner, from nowhere when the city sells to
    /// itself); a derelict is restored to the buyer.
    BuyBuilding {
        buyer: Option<EntityId>,
        building: EntityId,
        price: i64,
    },
    // --- M13 god commands (plan D48; phase 1 for tests, the CLI in phase 5).
    /// Give an agent a new asset, free and without import (`assets::grant`).
    GrantAsset {
        agent: EntityId,
        kind: crate::components::AssetKind,
        tier: u8,
    },
    /// Wreck an asset now (an implant fails instead).
    Wreck(EntityId),
    /// M13 D40/D48 (phase 4): Markets sell Stims legally, or stop.
    SetStimsLegal(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speed {
    X1,
    X2,
    X4,
    X8,
    X32,
    X128,
    X1000,
}

impl Speed {
    pub const ALL: [Speed; 7] = [Speed::X1, Speed::X2, Speed::X4, Speed::X8, Speed::X32, Speed::X128, Speed::X1000];

    pub fn multiplier(self) -> u32 {
        match self {
            Speed::X1 => 1,
            Speed::X2 => 2,
            Speed::X4 => 4,
            Speed::X8 => 8,
            Speed::X32 => 32,
            Speed::X128 => 128,
            Speed::X1000 => 1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levers {
    /// `0.0..=0.3`, default 0.05.
    pub tax_rate: f32,
    /// `0.5..=3.0`, default 1.0.
    pub sentence_mult: f32,
    /// `0..=60`, default 36 (M10, 2,000 residents; v1 `0..=30`, 10).
    pub guard_count: u8,
    /// `0..=30`, default 13 (M10; v1 `0..=10`, 2).
    pub immigration_per_week: u8,
    /// `0..=10`, default 3.
    pub dole_per_day: u8,
    /// M11: rent on city-owned Blocks per tier (initially `[rent] base`).
    #[serde(default)]
    pub city_rent: [i64; 3],
    /// M11: a ceiling on any Block's rent.
    #[serde(default)]
    pub rent_cap: Option<i64>,
    /// M11: the city never evicts.
    #[serde(default)]
    pub no_city_evictions: bool,
    /// M12 D10: a multiplier on each district's allocation weight (1.0; 0
    /// abandons the district: no guards, stance Withdrawn).
    #[serde(default = "default_guard_weight")]
    pub guard_weight: [f32; crate::components::MAX_DISTRICTS],
    /// M12 D12: the player's stance pin per district; `None` = the captain.
    #[serde(default)]
    pub stance_pin: [Option<crate::components::Stance>; crate::components::MAX_DISTRICTS],
    /// M12 (curfew lands with the levers in phase 5; Vagrancy reads it now).
    #[serde(default)]
    pub curfew: [bool; crate::components::MAX_DISTRICTS],
    /// M12 D23: city Sanitation workers (`[levers] sanitation_count`).
    #[serde(default)]
    pub sanitation_count: u8,
    /// M12 D23: a multiplier on each district's sweeper weight (1.0).
    #[serde(default = "default_guard_weight")]
    pub sanitation_weight: [f32; crate::components::MAX_DISTRICTS],
    /// M12 D42: the player's riot-response pin; `None` = the captain (D34).
    #[serde(default)]
    pub riot_response: Option<crate::components::RiotResponse>,
    /// M13 D40: Markets restock and sell Stims legally.
    #[serde(default)]
    pub stims_legal: bool,
    /// M13 D12: the city impounds vehicles with unpaid upkeep.
    #[serde(default = "default_true")]
    pub impound: bool,
    /// M13 D48: upkeep is charged × `1 + rate`, per `AssetClass`.
    #[serde(default)]
    pub asset_tax: [f32; 8],
}

fn default_true() -> bool {
    true
}

fn default_guard_weight() -> [f32; crate::components::MAX_DISTRICTS] {
    [1.0; crate::components::MAX_DISTRICTS]
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.overlaps(&b)
}

impl Levers {
    pub fn from_config(cfg: &Config) -> Levers {
        Levers {
            tax_rate: cfg.levers.tax_rate,
            sentence_mult: cfg.levers.sentence_mult,
            guard_count: cfg.levers.guard_count,
            immigration_per_week: cfg.levers.immigration_per_week,
            dole_per_day: cfg.levers.dole_per_day,
            city_rent: cfg.rent.base,
            rent_cap: None,
            no_city_evictions: false,
            guard_weight: default_guard_weight(),
            stance_pin: [None; crate::components::MAX_DISTRICTS],
            curfew: [false; crate::components::MAX_DISTRICTS],
            sanitation_count: cfg.levers.sanitation_count,
            sanitation_weight: default_guard_weight(),
            riot_response: None,
            stims_legal: cfg.levers.stims_legal,
            impound: true,
            asset_tax: [0.0; 8],
        }
    }
}

impl World {
    /// Queue a command for the start of the next tick.
    pub fn push_command(&mut self, cmd: PlayerCommand) {
        self.command_queue.push(cmd);
    }

    /// Move every command out of `cmds` into the queue, in order.
    pub fn push_commands(&mut self, cmds: &mut Vec<PlayerCommand>) {
        self.command_queue.append(cmds);
    }

    /// Drain the queue, apply each command in order, log it.
    pub fn apply_commands(&mut self) {
        let queue = std::mem::take(&mut self.command_queue);
        for cmd in queue {
            self.apply_command(&cmd);
            self.command_log.push((self.tick, cmd));
        }
    }

    fn apply_command(&mut self, cmd: &PlayerCommand) {
        match cmd {
            PlayerCommand::Bind(hole) => {
                crate::systems::bind::bind(self, *hole);
            }
            PlayerCommand::ReleaseReserve { amount } => self.cmd_release_reserve(*amount),
            PlayerCommand::SetTaxRate(r) => {
                self.levers.tax_rate = r.clamp(0.0, 0.3);
                self.push_event(EventKind::PlayerAction, &[], format!("Tax rate set to {:.2}", self.levers.tax_rate));
            }
            PlayerCommand::SetSentenceMult(m) => {
                self.levers.sentence_mult = m.clamp(0.5, 3.0);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Sentence multiplier set to {:.2}", self.levers.sentence_mult),
                );
            }
            PlayerCommand::SetGuardCount(n) => {
                self.levers.guard_count = (*n).min(60);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Guard count set to {}", self.levers.guard_count),
                );
            }
            PlayerCommand::SetImmigrationPerWeek(n) => {
                self.levers.immigration_per_week = (*n).min(30);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Immigration set to {} per week", self.levers.immigration_per_week),
                );
            }
            PlayerCommand::SetDolePerDay(n) => {
                self.levers.dole_per_day = (*n).min(10);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Dole set to {} per day", self.levers.dole_per_day),
                );
            }
            PlayerCommand::GrantCoins { agent, amount } => self.cmd_grant_coins(*agent, *amount),
            PlayerCommand::Pin(agent, pinned) => {
                if let Some(brain) = self.comp_mut::<Brain>(*agent) {
                    brain.pinned = *pinned;
                    let text = if *pinned { "Pinned" } else { "Unpinned" };
                    self.push_event(EventKind::PlayerAction, &[*agent], format!("{text} {}", self.name_of(*agent)));
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[*agent], "Pin: no such agent");
                }
            }
            PlayerCommand::SetView(rect) => {
                self.view_rect = *rect;
            }
            PlayerCommand::SetSpeed(_) => {}
            PlayerCommand::SetLawPosture(p) => {
                if let Some(l) = self.law_mut() {
                    l.pinned = *p;
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetLawPosture: no Precinct");
                    return;
                }
                crate::systems::law_brain::rescore(self, 0.0, "pinned");
                let text = match p {
                    Some(p) => format!("Law posture pinned to {p}"),
                    None => "Law posture handed back to the captain".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetGuardWeight { district, weight } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetGuardWeight: no such district");
                    return;
                }
                self.levers.guard_weight[i] = weight.clamp(0.0, 5.0);
                let text =
                    format!("Guard weight in {} set to {:.2}", self.district_name(*district), weight.clamp(0.0, 5.0));
                self.push_event(EventKind::PlayerAction, &[], text);
                crate::systems::law_brain::redeal(self, "lever");
            }
            PlayerCommand::SetStance { district, stance } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetStance: no such district");
                    return;
                }
                if let Some(crate::components::Stance::Crackdown(g)) = stance {
                    if !self.has::<Gang>(*g) {
                        self.push_event(EventKind::PlayerActionFailed, &[], "SetStance: no such gang");
                        return;
                    }
                }
                self.levers.stance_pin[i] = *stance;
                let name = self.district_name(*district).to_string();
                let text = match stance {
                    Some(s) => {
                        format!("Stance in {name} pinned to {}", crate::systems::law_brain::stance_label(self, *s))
                    }
                    None => format!("Stance in {name} handed back to the captain"),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
                crate::systems::law_brain::redeal(self, "pinned");
            }
            PlayerCommand::SetSanitation(n) => {
                self.levers.sanitation_count = (*n).min(60);
                let text = format!("Sanitation headcount set to {}", self.levers.sanitation_count);
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetSanitationWeight { district, weight } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetSanitationWeight: no such district");
                    return;
                }
                let w = weight.clamp(0.0, 5.0);
                self.levers.sanitation_weight[i] = w;
                let text = format!("Sanitation weight in {} set to {w:.2}", self.district_name(*district));
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCurfew { district, on } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetCurfew: no such district");
                    return;
                }
                self.levers.curfew[i] = *on;
                self.districts[i].curfew = *on;
                let name = self.district_name(*district).to_string();
                let text = if *on { format!("Curfew in {name}") } else { format!("Curfew in {name} lifted") };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetStimsLegal(on) => {
                self.levers.stims_legal = *on;
                let text = if *on { "Stims legal: Markets stock and sell them" } else { "Stims banned again" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetRiotResponse(r) => {
                self.levers.riot_response = *r;
                let text = match r {
                    Some(r) => format!("Riot response pinned to {r:?}"),
                    None => "Riot response handed back to the captain".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::Arrest(who) => match crate::systems::law::player_arrest(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Arrested {name}"));
                }
                Err(e) => {
                    self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Arrest: {e}"));
                }
            },
            PlayerCommand::Release(who) => match crate::systems::law::player_release(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Released {name}"));
                }
                Err(e) => {
                    self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Release: {e}"));
                }
            },
            PlayerCommand::KillAgent(_)
            | PlayerCommand::JailAgent { .. }
            | PlayerCommand::FreeAgent(_)
            | PlayerCommand::FundGang { .. }
            | PlayerCommand::SeizeGangTreasury(_)
            | PlayerCommand::KillGang(_)
            | PlayerCommand::JailGang { .. }
            | PlayerCommand::FireAllGuards
            | PlayerCommand::SetTreasury(_)
            | PlayerCommand::FundCorp { .. }
            | PlayerCommand::BankruptCorp(_)
            | PlayerCommand::SeizeBuildings { .. }
            | PlayerCommand::KillExec(_)
            | PlayerCommand::KillStaff(_)
            | PlayerCommand::SetCorpOrder { .. }
            | PlayerCommand::StrikeNow(_)
            | PlayerCommand::WipeTreasuries
            | PlayerCommand::Riot(_)
            | PlayerCommand::Litter { .. }
            | PlayerCommand::SplitGang(_)
            | PlayerCommand::Derelict(_)
            | PlayerCommand::BuyBuilding { .. }
            | PlayerCommand::GrantAsset { .. }
            | PlayerCommand::Wreck(_) => {
                let _ = match self.cmd_god(cmd) {
                    Ok((actors, text)) => self.push_event(EventKind::PlayerAction, &actors, format!("God: {text}")),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("God: {e}")),
                };
            }
            PlayerCommand::SetCityRent(rent) => {
                self.levers.city_rent = rent.map(|r| r.clamp(0, 20));
                let [a, b, c] = self.levers.city_rent;
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("City rent set to {a}/{b}/{c} per Block per day"),
                );
            }
            PlayerCommand::SetRentCap(cap) => {
                self.levers.rent_cap = cap.map(|c| c.max(0));
                let text = match self.levers.rent_cap {
                    Some(c) => format!("Rent capped at {c} per Block per day"),
                    None => "Rent cap lifted".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::NoCityEvictions(on) => {
                self.levers.no_city_evictions = *on;
                let text = if *on { "The city stops evicting" } else { "The city evicts again" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::Nationalise(b) => {
                let _ = match crate::systems::ownership::nationalise(self, *b) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[*b], text),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*b], format!("Nationalise: {e}")),
                };
            }
            PlayerCommand::Subsidise { corp, amount } => {
                let _ = match crate::systems::ownership::subsidise(self, *corp, *amount) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[*corp], text),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*corp], format!("Subsidise: {e}")),
                };
            }
            PlayerCommand::BreakUp(corp) => {
                let _ = match crate::systems::corps::break_up(self, *corp) {
                    Ok(s) => {
                        let text =
                            format!("Broke up {} into {}", self.owner_label(Some(*corp)), self.owner_label(Some(s)));
                        self.push_event(EventKind::PlayerAction, &[*corp, s], text)
                    }
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*corp], format!("BreakUp: {e}")),
                };
            }
            PlayerCommand::DemolishHome(home) => {
                let _ = match self.cmd_demolish_home(*home) {
                    Ok(n) => self.push_event(
                        EventKind::PlayerAction,
                        &[*home],
                        format!("Demolished Block#{} ({n} residents made homeless)", home.index),
                    ),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*home], format!("DemolishHome: {e}")),
                };
            }
            PlayerCommand::BuildHome { rect } => {
                let _ = match self.cmd_build_home(*rect) {
                    Ok((id, housed)) => self.push_event(
                        EventKind::PlayerAction,
                        &[id],
                        format!("Built Block#{} at ({}, {}); {housed} moved in", id.index, rect.x, rect.y),
                    ),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("BuildHome: {e}")),
                };
            }
        }
    }

    /// The god commands: `Ok((actors, event text))` or why nothing happened.
    fn cmd_god(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{law, law_brain};
        let gang_name = |w: &World, g: EntityId| w.comp::<Gang>(g).map(|g| g.name.clone()).ok_or("no such gang");
        match *cmd {
            PlayerCommand::KillAgent(who) => {
                if !self.is_alive(who) || !self.has::<Brain>(who) {
                    return Err("KillAgent: no such agent".into());
                }
                let name = self.name_of(who);
                self.kill_by(who, DeathCause::Violence, None);
                Ok((vec![who], format!("struck {name} dead")))
            }
            PlayerCommand::JailAgent { who, days } => {
                let name = self.name_of(who);
                self.god_jail(who, days)?;
                Ok((vec![who], format!("jailed {name} for {days} days")))
            }
            PlayerCommand::FreeAgent(who) => {
                law::player_release(self, who).map_err(|e| format!("FreeAgent: {e}"))?;
                Ok((vec![who], format!("freed {}", self.name_of(who))))
            }
            PlayerCommand::FundGang { gang, amount } => {
                let name = gang_name(self, gang)?;
                if let Some(g) = self.comp_mut::<Gang>(gang) {
                    g.treasury += amount;
                }
                Ok((vec![gang], format!("gave {name} {amount} coins")))
            }
            PlayerCommand::SeizeGangTreasury(gang) => {
                let name = gang_name(self, gang)?;
                let coins = self.comp_mut::<Gang>(gang).map_or(0, |g| std::mem::take(&mut g.treasury));
                if let Some(t) = self.treasury_mut() {
                    t.coins += coins;
                }
                Ok((vec![gang], format!("seized {coins} coins from {name}")))
            }
            PlayerCommand::KillGang(gang) => {
                let name = gang_name(self, gang)?;
                let members = self.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
                for &m in &members {
                    self.kill_by(m, DeathCause::Violence, None);
                }
                Ok((vec![gang], format!("killed all {} of {name}", members.len())))
            }
            PlayerCommand::JailGang { gang, days } => {
                let name = gang_name(self, gang)?;
                let members = self.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
                let jailed = members.into_iter().filter(|&m| self.god_jail(m, days).is_ok()).count();
                Ok((vec![gang], format!("jailed {jailed} of {name} for {days} days")))
            }
            PlayerCommand::FireAllGuards => {
                let guards = law_brain::guards(self);
                for &g in &guards {
                    // As `law::reconcile_guards` dismisses: no vacancy (job
                    // search would rehire the whole watch the next morning;
                    // reconcile_guards rehires up to the lever, five a day),
                    // but a Fire event, so it reaches the biography. An
                    // escort in progress ends: the suspect walks.
                    let text = format!("{} dismissed from the guard", self.name_of(g));
                    crate::systems::economy::dismiss(self, g, None, text);
                }
                law_brain::recompute_captain(self);
                Ok((guards, "dismissed every guard".to_string()))
            }
            PlayerCommand::SetTreasury(coins) => {
                let t = self.treasury_mut().ok_or("SetTreasury: no Civic Hall")?;
                t.coins = coins;
                Ok((vec![], format!("set the Treasury to {coins}")))
            }
            PlayerCommand::Riot(_)
            | PlayerCommand::Litter { .. }
            | PlayerCommand::SplitGang(_)
            | PlayerCommand::Derelict(_)
            | PlayerCommand::BuyBuilding { .. } => self.cmd_god_district(cmd),
            PlayerCommand::GrantAsset { agent, kind, tier } => {
                let a =
                    crate::systems::assets::grant(self, agent, kind, tier).map_err(|e| format!("GrantAsset: {e}"))?;
                Ok((vec![agent, a], format!("granted {} a {}", self.name_of(agent), self.name_of(a))))
            }
            PlayerCommand::Wreck(a) => {
                if !self.has::<crate::components::Asset>(a) {
                    return Err("Wreck: no such asset".into());
                }
                let what = self.name_of(a);
                crate::systems::assets::wreck(self, a, "by god");
                Ok((vec![a], format!("wrecked the {what}")))
            }
            _ => self.cmd_god_corp(cmd),
        }
    }

    /// The M12 god commands on districts (docs/GOD_SCENARIOS_V3.md, plan D42).
    fn cmd_god_district(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{gang, litter, ownership, riot, street};
        let check_district = |w: &World, d: crate::components::DistrictId, what: &str| {
            if d.index() < w.districts.len() {
                Ok(w.district_name(d).to_string())
            } else {
                Err(format!("{what}: no such district"))
            }
        };
        match *cmd {
            PlayerCommand::Riot(d) => {
                let name = check_district(self, d, "Riot")?;
                riot::start(self, d, true).map_err(|e| format!("Riot in {name}: {e}"))?;
                Ok((vec![], format!("raised a riot in {name}")))
            }
            PlayerCommand::Litter { district, level } => {
                let name = check_district(self, district, "Litter")?;
                if self.litter.is_empty() {
                    return Err("Litter: litter is off".into());
                }
                let v = (level.clamp(0.0, 1.0) * 254.0).round() as u8;
                let tiles = self.districts[district.index()].streets.clone();
                for &i in &tiles {
                    if let Some(t) = self.litter.get_mut(i as usize) {
                        // Rubble (the Damage hook) stays rubble.
                        if *t != litter::RUBBLE {
                            *t = v;
                        }
                    }
                }
                litter::district_means(self);
                Ok((vec![], format!("littered {} street tiles of {name} to {v}", tiles.len())))
            }
            PlayerCommand::SplitGang(g) => {
                let name = self.comp::<Gang>(g).map(|x| x.name.clone()).ok_or("SplitGang: no such gang")?;
                let s = gang::split(self, g, None, false).map_err(|e| format!("SplitGang {name}: {e}"))?;
                let sn = self.comp::<Gang>(s).map_or_else(String::new, |x| x.name.clone());
                Ok((vec![g, s], format!("split {name}: the {sn}")))
            }
            PlayerCommand::Derelict(b) => {
                let what = self.name_of(b);
                let Some(bd) = self.comp::<Building>(b) else { return Err("Derelict: no such building".into()) };
                if bd.demolished || bd.derelict {
                    return Err(format!("Derelict: {what} is already gone or derelict"));
                }
                if !street::can_go_derelict(bd.kind) {
                    return Err(format!("Derelict: a {} cannot go derelict", bd.kind.label()));
                }
                if !street::make_derelict(self, b, "by god") {
                    return Err(format!("Derelict: {what} refused"));
                }
                Ok((vec![b], format!("made {what} derelict")))
            }
            PlayerCommand::BuyBuilding { buyer, building, price } => {
                let Some(bd) = self.comp::<Building>(building) else {
                    return Err("BuyBuilding: no such building".into());
                };
                if bd.demolished {
                    return Err("BuyBuilding: demolished".into());
                }
                let (owner, derelict) = (bd.owner, bd.derelict);
                if buyer.is_some_and(|t| !(self.has::<Wallet>(t) || self.has::<Gang>(t) || self.has::<Corp>(t))) {
                    return Err("BuyBuilding: the buyer is not an agent, gang or corp".into());
                }
                if owner == buyer && !derelict {
                    return Err("BuyBuilding: the buyer owns it".into());
                }
                let price = price.max(0);
                // As every M11 flow: the buyer pays, the owner (or the city) is paid.
                let paid = if owner == buyer {
                    0
                } else {
                    ownership::charge(self, buyer, owner, price, ownership::Flow::Sale)
                };
                let what = self.name_of(building);
                let (from, to) = (self.owner_label(owner), self.owner_label(buyer));
                if derelict {
                    street::restore(self, building, buyer, "bought by god");
                } else {
                    crate::systems::corps::move_building(self, building, buyer);
                    if buyer.is_none() {
                        crate::systems::corps::end_contract(self, building, "bought by the city");
                    }
                }
                if let Some(c) = owner.filter(|&o| self.has::<Corp>(o) && Some(o) != buyer) {
                    ownership::push_corp_shock(self, c, CorpShock::BuildingLost);
                }
                let actors: Vec<EntityId> = [Some(building), owner, buyer].into_iter().flatten().collect();
                Ok((actors, format!("{to} bought {what} from {from} for {paid}")))
            }
            _ => Err("not a god command".into()),
        }
    }

    /// The M11 god commands on corps (docs/GOD_SCENARIOS_V2.md).
    fn cmd_god_corp(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{classes, corps, ownership};
        let corp_name = |w: &World, c: EntityId| w.comp::<Corp>(c).map(|c| c.name.clone()).ok_or("no such corp");
        match *cmd {
            PlayerCommand::FundCorp { corp, amount } => {
                let name = corp_name(self, corp)?;
                if let Some(c) = self.comp_mut::<Corp>(corp) {
                    c.treasury += amount;
                }
                Ok((vec![corp], format!("gave {name} {amount} coins")))
            }
            PlayerCommand::BankruptCorp(corp) => {
                let name = corp_name(self, corp)?;
                let days = self.config.corps.bankrupt_days + 1;
                let back = self.tick.saturating_sub(days * crate::time::TICKS_PER_DAY);
                if let Some(c) = self.comp_mut::<Corp>(corp) {
                    c.treasury = -1;
                    c.negative_since = Some(back);
                }
                corps::bankrupt(self, corp);
                Ok((vec![corp], format!("bankrupted {name}")))
            }
            PlayerCommand::SeizeBuildings { owner, to } => {
                if to.is_some_and(|t| !(self.has::<Wallet>(t) || self.has::<Gang>(t) || self.has::<Corp>(t))) {
                    return Err("SeizeBuildings: the new owner is not an agent, gang or corp".into());
                }
                if owner == to {
                    return Err("SeizeBuildings: same owner".into());
                }
                let owned: Vec<EntityId> = self
                    .with::<Building>()
                    .into_iter()
                    .filter(|&b| self.comp::<Building>(b).is_some_and(|bd| bd.owner == owner && !bd.demolished))
                    .collect();
                if owned.is_empty() {
                    return Err(format!("SeizeBuildings: {} owns nothing", self.owner_label(owner)));
                }
                let (from_label, to_label) = (self.owner_label(owner), self.owner_label(to));
                for &b in &owned {
                    corps::move_building(self, b, to);
                    if to.is_none() {
                        // The city does not keep a corp's guard contract.
                        corps::end_contract(self, b, "seized by the city");
                    }
                }
                if let Some(c) = owner.filter(|&o| self.has::<Corp>(o)) {
                    ownership::push_corp_shock(self, c, CorpShock::BuildingLost);
                }
                let actors: Vec<EntityId> = owner.into_iter().chain(to).collect();
                Ok((actors, format!("gave all {} buildings of {from_label} to {to_label}", owned.len())))
            }
            PlayerCommand::KillExec(corp) => {
                let name = corp_name(self, corp)?;
                let exec = self.comp::<Corp>(corp).and_then(|c| c.exec);
                let exec = exec.filter(|&e| self.is_alive(e) && self.has::<Brain>(e));
                let exec = exec.ok_or_else(|| format!("KillExec: {name} has no living exec"))?;
                let who = self.name_of(exec);
                self.kill_by(exec, DeathCause::Violence, None);
                Ok((vec![corp, exec], format!("struck {who}, exec of {name}, dead")))
            }
            PlayerCommand::KillStaff(corp) => {
                let name = corp_name(self, corp)?;
                let staff = self.comp::<Corp>(corp).map(|c| classes::employees(self, c)).unwrap_or_default();
                for &a in &staff {
                    self.kill_by(a, DeathCause::Violence, None);
                }
                Ok((vec![corp], format!("killed all {} employees of {name}", staff.len())))
            }
            PlayerCommand::SetCorpOrder { corp, order, niche, days } => {
                let name = corp_name(self, corp)?;
                let (now, evict_days) = (self.tick, self.config.rent.evict_days);
                let c = self.comp_mut::<Corp>(corp).ok_or("no such corp")?;
                let niche = match niche {
                    Some(n) if !c.niches.contains(&n) => return Err(format!("SetCorpOrder: {name} is not in {n}")),
                    Some(n) => n,
                    None => c
                        .order_niche
                        .filter(|n| c.niches.contains(n))
                        .or_else(|| c.niches.iter().next().copied())
                        .ok_or("SetCorpOrder: the corp has no niche")?,
                };
                // As `corp_brain::rescore` switches (D22).
                if c.order == CorpOrder::Squeeze {
                    c.wage_mult = 1.0;
                    c.evict_days_override = None;
                }
                if order == CorpOrder::Squeeze {
                    match niche {
                        Niche::Housing => c.evict_days_override = Some(evict_days.saturating_sub(1).max(2)),
                        Niche::Food => c.wage_mult = 0.9,
                        Niche::Security | Niche::Tech => {}
                    }
                }
                c.order = order;
                c.order_niche = Some(niche);
                c.order_since = now;
                c.pinned_until = Some(now + u64::from(days) * crate::time::TICKS_PER_DAY);
                Ok((vec![corp], format!("pinned {name} to {order} in {niche} for {days} days")))
            }
            PlayerCommand::StrikeNow(corp) => {
                let name = corp_name(self, corp)?;
                let strikers = self.comp::<Corp>(corp).map(|c| classes::employees(self, c)).unwrap_or_default();
                if strikers.is_empty() {
                    return Err(format!("StrikeNow: {name} has no workers"));
                }
                classes::walk_out(self, &strikers);
                self.stats.current.strikes += 1;
                let text = format!("{} workers of {name} walk out (by god)", strikers.len());
                self.push_event(EventKind::Strike, &[corp], text);
                crate::systems::corp_brain::push_shock(self, corp, CorpShock::Strike);
                Ok((vec![corp], format!("called a strike at {name}")))
            }
            PlayerCommand::WipeTreasuries => {
                let all = self.corps();
                for &c in &all {
                    if let Some(cc) = self.comp_mut::<Corp>(c) {
                        cc.treasury = 0;
                    }
                }
                let n = all.len();
                Ok((all, format!("wiped the treasuries of {n} corps")))
            }
            _ => Err("not a god command".into()),
        }
    }

    /// A god sentence: no report, no capacity check. A prisoner's sentence is
    /// extended instead. It is filed as Assault: a full Jail makes room by
    /// freeing the Theft prisoner with the longest sentence, which a long
    /// god sentence for Theft always was.
    fn god_jail(&mut self, who: EntityId, days: u32) -> Result<(), String> {
        if !self.is_alive(who) || !self.has::<Brain>(who) {
            return Err("JailAgent: no such agent".into());
        }
        let until = self.tick + u64::from(days.max(1)) * crate::time::TICKS_PER_DAY;
        // Already inside: the sentence runs to whichever end is later.
        if let Some(s) = self.comp_mut::<Sentence>(who) {
            s.until_tick = s.until_tick.max(until);
            s.crime = s.crime.max(Crime::Assault);
            return Ok(());
        }
        let jail = self.building_of_kind(BuildingKind::Jail).ok_or("JailAgent: no Precinct")?;
        crate::systems::law::sentence(self, who, Crime::Assault, until, jail);
        Ok(())
    }

    /// Tiles become Ground, residents are homeless (one event each), the
    /// building is marked demolished and dropped from the kind index.
    fn cmd_demolish_home(&mut self, home: EntityId) -> Result<usize, String> {
        let Some(b) = self.comp::<Building>(home) else { return Err("no such building".into()) };
        if b.kind != BuildingKind::Home {
            return Err(format!("{} is not a Block", b.kind.label()));
        }
        if b.demolished {
            return Err("already demolished".into());
        }
        // M11 D44: only the city's own Blocks.
        if b.owner.is_some() {
            return Err(format!("owned by {}; nationalise it first", self.owner_label(b.owner)));
        }
        // M12 D26: with the street on, a demolished Block stands derelict at
        // half capacity (squatters may move in), its walls left as they are.
        if self.config.street.enabled {
            if b.derelict {
                return Err("already derelict".into());
            }
            let residents: Vec<EntityId> = self.residents_of(home).to_vec();
            let cap = b.capacity;
            for &r in &residents {
                let name = self.name_of(r);
                self.push_event(EventKind::Homeless, &[r], format!("{name} is homeless"));
            }
            crate::systems::street::make_derelict(self, home, "demolished");
            if let Some(bd) = self.comp_mut::<Building>(home) {
                bd.capacity = cap / 2;
            }
            return Ok(residents.len());
        }
        let (rect, door, occupants) = (b.rect, b.door, b.occupants.clone());
        let outside = self.outside_door(b);
        for o in occupants {
            self.abort_plan(o);
            self.remove_from_building(o);
            if let Some(p) = self.comp_mut::<Position>(o) {
                p.tile = outside;
                p.building = None;
            }
        }
        let residents: Vec<EntityId> = self.residents_of(home).to_vec();
        for &r in &residents {
            self.set_home(r, None);
            // Children living there stand outside too.
            if self.comp::<Position>(r).is_some_and(|p| p.building == Some(home)) {
                if let Some(p) = self.comp_mut::<Position>(r) {
                    p.tile = outside;
                    p.building = None;
                }
            }
            let name = self.name_of(r);
            self.push_event(EventKind::Homeless, &[r], format!("{name} is homeless"));
        }
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                self.map.set_tile(TilePos { x, y }, TileKind::Ground);
            }
        }
        let _ = door;
        if let Some(b) = self.comp_mut::<Building>(home) {
            b.demolished = true;
            b.occupants.clear();
            b.stock_food = 0;
        }
        if let Some(v) = self.buildings_by_kind.get_mut(&BuildingKind::Home) {
            v.retain(|&h| h != home);
        }
        self.invalidate_flow_fields();
        crate::systems::districts::rebuild(self);
        Ok(residents.len())
    }

    /// A 4×4..6×6 rect of Ground, clear of other buildings and of Water,
    /// paid from the Treasury: walls on the perimeter, a door mid-south,
    /// capacity 6; the lowest-index homeless move in.
    fn cmd_build_home(&mut self, rect: Rect) -> Result<(EntityId, usize), String> {
        if !(4..=6).contains(&rect.w) || !(4..=6).contains(&rect.h) {
            return Err(format!("rect must be 4x4 to 6x6, got {}x{}", rect.w, rect.h));
        }
        let (x1, y1) = (i32::from(rect.x) + i32::from(rect.w), i32::from(rect.y) + i32::from(rect.h));
        if !self.map.in_bounds(i32::from(rect.x), i32::from(rect.y)) || !self.map.in_bounds(x1 - 1, y1 - 1) {
            return Err("rect is off the map".into());
        }
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                let t = TilePos { x, y };
                if self.map.tile_at(t) != TileKind::Ground {
                    return Err(format!("({x}, {y}) is {:?}, not Ground", self.map.tile_at(t)));
                }
            }
        }
        for b in self.with::<Building>() {
            if let Some(bd) = self.comp::<Building>(b) {
                if !bd.demolished && rects_overlap(bd.rect, rect) {
                    return Err(format!("overlaps {}#{}", bd.kind.label(), b.index));
                }
            }
        }
        for y in (i32::from(rect.y) - 1)..=y1 {
            for x in (i32::from(rect.x) - 1)..=x1 {
                if self.map.in_bounds(x, y) && self.map.tile_at(TilePos { x: x as u8, y: y as u8 }) == TileKind::Water {
                    return Err("adjacent to Water".into());
                }
            }
        }
        let door = TilePos { x: rect.x + rect.w / 2, y: rect.y + rect.h - 1 };
        let outside = (i32::from(door.x), i32::from(door.y) + 1);
        if !self.map.in_bounds(outside.0, outside.1) {
            return Err("the door would open off the map".into());
        }
        let outside_tile = TilePos { x: outside.0 as u8, y: outside.1 as u8 };
        if !matches!(self.map.tile_at(outside_tile), TileKind::Ground | TileKind::Road) {
            return Err(format!("the door would open onto {:?}", self.map.tile_at(outside_tile)));
        }
        let cost = self.config.economy.build_home_cost;
        let coins = self.treasury().map_or(0, |t| t.coins);
        if coins < cost {
            return Err(format!("Treasury has {coins}, needs {cost}"));
        }
        if let Some(t) = self.treasury_mut() {
            t.coins -= cost;
        }
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                let t = TilePos { x, y };
                let kind = if t == door {
                    TileKind::Door
                } else if rect.on_perimeter(t) {
                    TileKind::Wall
                } else {
                    TileKind::Ground
                };
                self.map.set_tile(t, kind);
            }
        }
        let capacity = self.config.buildings.home.capacity;
        let id = self.spawn();
        self.insert(
            id,
            Building {
                kind: BuildingKind::Home,
                production_accum: 0.0,
                claim: None,
                child_food_debt: 0.0,
                rect,
                door,
                stock_food: 0,
                capacity,
                owner: None,
                occupants: Vec::new(),
                demolished: false,
                tier: 1,
                rent_per_day: self.levers.city_rent[1],
                revenue_today: 0,
                revenue: std::collections::VecDeque::new(),
                secured_by: None,
                derelict: false,
                empty_since: None,
                closed_until: None,
                full_capacity: None,
                stock_goods: [0; 2],
                asset_sales_today: 0,
                asset_sales: std::collections::VecDeque::new(),
            },
        );
        self.buildings_by_kind.entry(BuildingKind::Home).or_default().push(id);
        self.invalidate_flow_fields();
        crate::systems::districts::rebuild(self);
        let homeless: Vec<EntityId> = self
            .citizens()
            .into_iter()
            .filter(|&c| matches!(self.comp::<Household>(c), Some(Household { home: None, .. })))
            .take(usize::from(capacity))
            .collect();
        let housed = homeless.len();
        for h in homeless {
            self.set_home(h, Some(id));
        }
        Ok((id, housed))
    }

    /// Split `amount` evenly over every Market (remainder to the lowest id),
    /// each share capped by that Market's room and by what the Warehouse holds
    /// (M10 D22).
    fn cmd_release_reserve(&mut self, amount: u32) {
        let markets: Vec<EntityId> = self
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .copied()
            .filter(|&m| self.comp::<Building>(m).is_some_and(|b| !b.demolished))
            .collect();
        let Some(wh) = self.building_of_kind(BuildingKind::Warehouse).filter(|_| !markets.is_empty()) else {
            self.push_event(EventKind::PlayerActionFailed, &[], "ReleaseReserve: no Reserve Depot or Street Market");
            return;
        };
        let market_cap = self.config.buildings.market.stock_cap;
        let mut available = self.comp::<Building>(wh).map_or(0, |b| b.stock_food);
        let n = markets.len() as u32;
        let mut moved = 0;
        // Each round splits what is left evenly over the Markets that still
        // have room (remainder to the lowest ids); a Market that fills drops
        // out and the next round shares its overflow among the rest.
        let mut left = amount;
        loop {
            let open: Vec<(EntityId, u32)> = markets
                .iter()
                .map(|&mk| (mk, self.comp::<Building>(mk).map_or(0, |b| market_cap.saturating_sub(b.stock_food))))
                .filter(|&(_, room)| room > 0)
                .collect();
            if left == 0 || available == 0 || open.is_empty() {
                break;
            }
            let k = open.len() as u32;
            let mut round = 0;
            for (i, &(mk, room)) in open.iter().enumerate() {
                let share = left / k + u32::from((i as u32) < left % k);
                let m = share.min(room).min(available);
                if m == 0 {
                    continue;
                }
                available -= m;
                round += m;
                if let Some(b) = self.comp_mut::<Building>(mk) {
                    b.stock_food += m;
                }
            }
            if round == 0 {
                break;
            }
            left -= round;
            moved += round;
        }
        if moved == 0 {
            self.push_event(EventKind::PlayerActionFailed, &[], "ReleaseReserve: nothing to move");
            return;
        }
        if let Some(b) = self.comp_mut::<Building>(wh) {
            b.stock_food -= moved;
        }
        let to = if n == 1 { "the Street Market".to_string() } else { format!("{n} Street Markets") };
        self.push_event(EventKind::PlayerAction, &[], format!("Released {moved} food from the Reserve Depot to {to}"));
    }

    fn cmd_grant_coins(&mut self, agent: EntityId, amount: i64) {
        if amount <= 0 || !self.has::<Wallet>(agent) {
            self.push_event(EventKind::PlayerActionFailed, &[agent], "GrantCoins: bad amount or no such agent");
            return;
        }
        let treasury = self.treasury().map_or(0, |t| t.coins);
        if treasury < amount {
            self.push_event(
                EventKind::PlayerActionFailed,
                &[agent],
                format!("GrantCoins: Treasury has {treasury}, needs {amount}"),
            );
            return;
        }
        if let Some(t) = self.treasury_mut() {
            t.coins -= amount;
        }
        if let Some(w) = self.comp_mut::<Wallet>(agent) {
            w.coins += amount;
        }
        let name = self.name_of(agent);
        self.push_event(EventKind::PlayerAction, &[agent], format!("Granted {amount} coins to {name}"));
    }
}
