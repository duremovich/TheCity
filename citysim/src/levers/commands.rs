use super::*;

impl World {
    /// Tiles become Ground, residents are homeless (one event each), the
    /// building is marked demolished and dropped from the kind index.
    pub(super) fn cmd_demolish_home(&mut self, home: EntityId) -> Result<usize, String> {
        let Some(b) = self.comp::<Building>(home) else { return Err("no such building".into()) };
        if b.kind != BuildingKind::Home {
            return Err(format!("{} is not a Block", b.kind.label()));
        }
        if b.demolished {
            return Err("already demolished".into());
        }
        // The city milestone C6a: a structure's floors go with it; the
        // street-off razing would clear a host's tenancies' tiles too.
        let razes = !self.config.street.enabled;
        if razes && (b.within.is_some() || !crate::systems::floors::tenancies_of(self, home).is_empty()) {
            return Err("its structure holds other tenancies".into());
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
                p.floor = 0;
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
                    p.floor = 0;
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
        if let Some(v) = self.buildings_by_kind_mut().get_mut(&BuildingKind::Home) {
            v.retain(|&h| h != home);
        }
        self.invalidate_flow_fields();
        crate::systems::districts::rebuild(self);
        Ok(residents.len())
    }

    /// A 4×4..6×6 rect of Ground, clear of other buildings and of Water,
    /// paid from the Treasury: walls on the perimeter, a door mid-south,
    /// capacity 6; the lowest-index homeless move in.
    pub(super) fn cmd_build_home(&mut self, rect: Rect) -> Result<(EntityId, usize), String> {
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
                if self.map.in_bounds(x, y) && self.map.tile_at(TilePos { x: x as u16, y: y as u16 }) == TileKind::Water
                {
                    return Err("adjacent to Water".into());
                }
            }
        }
        let door = TilePos { x: rect.x + rect.w / 2, y: rect.y + rect.h - 1 };
        let outside = (i32::from(door.x), i32::from(door.y) + 1);
        if !self.map.in_bounds(outside.0, outside.1) {
            return Err("the door would open off the map".into());
        }
        let outside_tile = TilePos { x: outside.0 as u16, y: outside.1 as u16 };
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
                security: Default::default(),
                focus: None,
                hacked: None,
                last_door_open: None,
                label: None,
                feed: None,
                venue: None,
                charity: None,
                camp: None,
                input_accum: 0.0,
                floors: 1,
                floor_days: 0,
                synth_food: 0,
                bin: 0,
                bin_accum: 0.0,
                spoil_carry: 0.0,
                bin_diver: None,
                bin_regular: None,
                under_construction_until: None,
                stock_x: [0; 3],
                within: None,
                span: None,
                units: 0,
                floor_rent: 0,
                floor_arrears: 0,
                hours: None,
                lift: None,
            },
        );
        self.buildings_by_kind_mut().entry(BuildingKind::Home).or_default().push(id);
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
    pub(super) fn cmd_release_reserve(&mut self, amount: u32) {
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

    pub(super) fn cmd_grant_coins(&mut self, agent: EntityId, amount: i64) {
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

impl World {
    /// M16b phase 4 (spec § 8): the lever and the god commands of 16b.
    /// `Ok` the event's text, `Err` the failure's.
    pub(super) fn apply_m16b(&mut self, cmd: &PlayerCommand) -> Result<String, String> {
        use crate::contract::{CoercionWhy, Demand, Leverage};
        match cmd {
            PlayerCommand::SetPrisonPermit(on) => {
                self.levers.prison_permit = *on;
                Ok(if *on {
                    "Lever: Prisons permitted".to_string()
                } else {
                    "Lever: no new Prison, and no new captive in one".to_string()
                })
            }
            PlayerCommand::Hostage { captive, holder, at, demand } => {
                if !crate::systems::captives::on(self) {
                    return Err("Hostage: captives are off".to_string());
                }
                if !self.has::<crate::components::Building>(*at) {
                    return Err("Hostage: no such building".to_string());
                }
                let over = match demand {
                    Some(Demand::Release(_)) => self.law().and_then(|l| l.captain),
                    _ => None,
                };
                // Review finding 2: the hand-over agent is the holder's head,
                // a holding person itself, else its strongest free member;
                // a holder-less hold has none (`EntityId::NONE`: nobody
                // rolls the demand, nobody kills at the hold's end).
                let agent = match holder {
                    None => EntityId::NONE,
                    Some(h) => {
                        let member = || {
                            self.comp::<crate::components::Gang>(*h).and_then(|g| {
                                g.members
                                    .iter()
                                    .copied()
                                    .filter(|&m| crate::systems::contracts::free_adult(self, m))
                                    .map(|m| (crate::systems::combat::power(self, m), m))
                                    .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
                                    .map(|(_, m)| m)
                            })
                        };
                        let person = self.has::<crate::components::Identity>(*h).then_some(*h);
                        match crate::systems::contracts::head_of(self, *h).or(person).or_else(member) {
                            Some(a) => a,
                            None => return Err("Hostage: the holder has nobody to hand over".to_string()),
                        }
                    }
                };
                if agent == *captive {
                    return Err("Hostage: the captive cannot hold itself".to_string());
                }
                if crate::systems::captives::take(self, *captive, *holder, agent, *at, *demand, over) {
                    Ok(format!("God: {} held at {}", self.name_of(*captive), self.name_of(*at)))
                } else {
                    Err("Hostage: refused (quota, capacity, permit or already held)".to_string())
                }
            }
            PlayerCommand::Wound { agent, by } => {
                if !crate::systems::wounded::on(self) || !crate::systems::law::living(self, *agent) {
                    return Err("Wound: the wounded are off or the agent is gone".to_string());
                }
                if crate::systems::wounded::is_down(self, *agent) {
                    return Err("Wound: already down".to_string());
                }
                crate::systems::wounded::down(self, *agent, *by);
                Ok(format!("God: {} went down", self.name_of(*agent)))
            }
            PlayerCommand::Subscribe { agent, corp } => {
                if crate::systems::wounded::subscribe_with(self, *agent, *agent, *corp) {
                    Ok(format!(
                        "God: {} subscribed to {}'s Trauma crew",
                        self.name_of(*agent),
                        self.owner_label(Some(*corp))
                    ))
                } else {
                    Err("Subscribe: refused".to_string())
                }
            }
            PlayerCommand::TagAgent { agent, owner, tier } => {
                crate::systems::tags::god_tag(self, *agent, *owner, *tier)
                    .map(|id| format!("God: tag #{id} on {}", self.name_of(*agent)))
            }
            PlayerCommand::Coerce { agent, over, leverage, demand } => {
                if !crate::systems::leverage::on(self) {
                    return Err("Coerce: leverage is off".to_string());
                }
                if !crate::systems::law::living(self, *over) {
                    return Err("Coerce: the coerced is gone".to_string());
                }
                let why = match (leverage, demand) {
                    (_, Demand::Vote { .. }) => CoercionWhy::Vote,
                    (Leverage::Hostage { .. }, _) => CoercionWhy::Hostage,
                    (Leverage::Secret { .. }, _) => CoercionWhy::Blackmail,
                    _ => CoercionWhy::Other,
                };
                let holder = self.gang_of(*agent).or(self.corp_of_agent(*agent)).filter(|_| why != CoercionWhy::Vote);
                let holder = holder.or(Some(*agent));
                let id =
                    crate::systems::leverage::force(self, holder, *agent, *over, *leverage, *demand, why, None, None);
                Ok(format!("God: coercion #{id} of {} by {}", self.name_of(*over), self.name_of(*agent)))
            }
            PlayerCommand::CallVote(corp) => crate::systems::governance::call_vote(self, *corp)
                .map(|w| format!("God: {}'s board voted, chair {}", self.owner_label(Some(*corp)), self.name_of(w))),
            PlayerCommand::ExposeSecret(id) => {
                let Some(c) = self.coercions.get(id).cloned() else {
                    return Err("ExposeSecret: no such coercion".to_string());
                };
                let Leverage::Secret { deed, actor, object, .. } = c.leverage else {
                    return Err("ExposeSecret: not a secret".to_string());
                };
                let d = crate::systems::gossip::home_district(self, actor)
                    .unwrap_or_else(|| crate::systems::gossip::talk_district(self, actor));
                let mult = 1.0 / self.config.gossip.reach0.get(deed).max(0.01);
                crate::systems::gossip::post_deed_at(self, d, deed, Some(actor), object, mult);
                Ok(format!("God: {}'s secret is out", self.name_of(actor)))
            }
            _ => Err("not a 16b command".to_string()),
        }
    }
}
