//! Player commands and stored lever values. Every command is applied at the
//! start of the next tick and recorded with its tick, so a save plus its
//! command log replays deterministically.

use serde::{Deserialize, Serialize};

use crate::components::{Brain, Building, BuildingKind, Household, Position, Rect, TileKind, TilePos, Wallet};
use crate::config::Config;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::map::Map;
use crate::world::World;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlayerCommand {
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
    /// `0..=30`, default 10.
    pub guard_count: u8,
    /// `0..=10`, default 2.
    pub immigration_per_week: u8,
    /// `0..=10`, default 3.
    pub dole_per_day: u8,
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
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
                self.levers.guard_count = (*n).min(30);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Guard count set to {}", self.levers.guard_count),
                );
            }
            PlayerCommand::SetImmigrationPerWeek(n) => {
                self.levers.immigration_per_week = (*n).min(10);
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
            PlayerCommand::Arrest(who) => match crate::systems::law::player_arrest(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Arrested {name}"));
                }
                Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Arrest: {e}")),
            },
            PlayerCommand::Release(who) => match crate::systems::law::player_release(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Released {name}"));
                }
                Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Release: {e}")),
            },
            PlayerCommand::DemolishHome(home) => match self.cmd_demolish_home(*home) {
                Ok(n) => self.push_event(
                    EventKind::PlayerAction,
                    &[*home],
                    format!("Demolished Home#{} ({n} residents made homeless)", home.index),
                ),
                Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*home], format!("DemolishHome: {e}")),
            },
            PlayerCommand::BuildHome { rect } => match self.cmd_build_home(*rect) {
                Ok((id, housed)) => self.push_event(
                    EventKind::PlayerAction,
                    &[id],
                    format!("Built Home#{} at ({}, {}); {housed} moved in", id.index, rect.x, rect.y),
                ),
                Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("BuildHome: {e}")),
            },
        }
    }

    /// Tiles become Ground, residents are homeless (one event each), the
    /// building is marked demolished and dropped from the kind index.
    fn cmd_demolish_home(&mut self, home: EntityId) -> Result<usize, String> {
        let Some(b) = self.comp::<Building>(home) else { return Err("no such building".into()) };
        if b.kind != BuildingKind::Home {
            return Err(format!("{:?} is not a Home", b.kind));
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
        let residents: Vec<EntityId> = self
            .citizens()
            .into_iter()
            .filter(|&c| self.comp::<Household>(c).and_then(|h| h.home) == Some(home))
            .collect();
        for &r in &residents {
            if let Some(h) = self.comp_mut::<Household>(r) {
                h.home = None;
            }
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
        if !Map::in_bounds(i32::from(rect.x), i32::from(rect.y)) || !Map::in_bounds(x1 - 1, y1 - 1) {
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
                    return Err(format!("overlaps {:?}#{}", bd.kind, b.index));
                }
            }
        }
        for y in (i32::from(rect.y) - 1)..=y1 {
            for x in (i32::from(rect.x) - 1)..=x1 {
                if Map::in_bounds(x, y) && self.map.tile_at(TilePos { x: x as u8, y: y as u8 }) == TileKind::Water {
                    return Err("adjacent to Water".into());
                }
            }
        }
        let cost = self.config.economy.build_home_cost;
        let coins = self.treasury().map_or(0, |t| t.coins);
        if coins < cost {
            return Err(format!("Treasury has {coins}, needs {cost}"));
        }
        if let Some(t) = self.treasury_mut() {
            t.coins -= cost;
        }
        let door = TilePos { x: rect.x + rect.w / 2, y: rect.y + rect.h - 1 };
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
                extort_count: 0,
                child_food_debt: 0.0,
                rect,
                door,
                stock_food: 0,
                capacity,
                owner: None,
                occupants: Vec::new(),
                demolished: false,
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
            if let Some(hh) = self.comp_mut::<Household>(h) {
                hh.home = Some(id);
            }
        }
        Ok((id, housed))
    }

    fn cmd_release_reserve(&mut self, amount: u32) {
        let (Some(wh), Some(mk)) =
            (self.building_of_kind(BuildingKind::Warehouse), self.building_of_kind(BuildingKind::Market))
        else {
            self.push_event(EventKind::PlayerActionFailed, &[], "ReleaseReserve: no Warehouse or Market");
            return;
        };
        let market_cap = self.config.buildings.market.stock_cap;
        let available = self.comp::<Building>(wh).map_or(0, |b| b.stock_food);
        let room = self.comp::<Building>(mk).map_or(0, |b| market_cap.saturating_sub(b.stock_food));
        let moved = amount.min(available).min(room);
        if moved == 0 {
            self.push_event(EventKind::PlayerActionFailed, &[], "ReleaseReserve: nothing to move");
            return;
        }
        if let Some(b) = self.comp_mut::<Building>(wh) {
            b.stock_food -= moved;
        }
        if let Some(b) = self.comp_mut::<Building>(mk) {
            b.stock_food += moved;
        }
        self.push_event(
            EventKind::PlayerAction,
            &[],
            format!("Released {moved} food from the Warehouse to the Market"),
        );
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
