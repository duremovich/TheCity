//! Player commands and stored lever values. Every command is applied at the
//! start of the next tick and recorded with its tick, so a save plus its
//! command log replays deterministically.

use serde::{Deserialize, Serialize};

use crate::components::{
    Brain, Building, BuildingKind, Crime, DeathCause, Gang, Household, Job, Position, Posture, Rect, Sentence,
    TileKind, TilePos, Wallet,
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
            | PlayerCommand::SetTreasury(_) => {
                let _ = match self.cmd_god(cmd) {
                    Ok((actors, text)) => self.push_event(EventKind::PlayerAction, &actors, format!("God: {text}")),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("God: {e}")),
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
                    // An escort in progress ends: the suspect walks.
                    if let Some(s) = self.comp_mut::<Brain>(g).and_then(|b| b.escorting.take()) {
                        if let Some(sb) = self.comp_mut::<Brain>(s) {
                            sb.cuffed_by = None;
                        }
                    }
                    self.abort_plan(g);
                    // As `law::reconcile_guards` dismisses: no vacancy (job
                    // search would rehire the whole watch the next morning;
                    // reconcile_guards rehires up to the lever, five a day),
                    // but a Fire event, so it reaches the biography.
                    self.remove::<Job>(g);
                    let name = self.name_of(g);
                    self.push_event(EventKind::Fire, &[g], format!("{name} dismissed from the guard"));
                }
                law_brain::recompute_captain(self);
                Ok((guards, "dismissed every guard".to_string()))
            }
            PlayerCommand::SetTreasury(coins) => {
                let t = self.treasury_mut().ok_or("SetTreasury: no Civic Hall")?;
                t.coins = coins;
                Ok((vec![], format!("set the Treasury to {coins}")))
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
            },
        );
        self.buildings_by_kind.entry(BuildingKind::Home).or_default().push(id);
        self.invalidate_flow_fields();
        let homeless: Vec<EntityId> = self
            .citizens()
            .into_iter()
            .filter(|&c| matches!(self.comp::<Household>(c), Some(Household { home: None })))
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
